use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::google_auth;
use crate::google_links::{self, GoogleWatchItem, ImportLinksReport};
use crate::google_prefs::{self, GooglePrefsStatus, GoogleSyncMode};
use crate::models::{
    DeepSearchStats, IndexStats, LocalDriveInfo, SearchHit, SearchQuery, SearchResponse,
    SourceInfo, SourceKind, SyncErrorKind, SyncReport, SyncStatus,
};
use crate::notion_prefs;
use crate::sources::google_docs::GoogleDocsSource;
use crate::sources::local::LocalFolderSource;
use crate::sources::SourceConnector;
use once_cell::sync::Lazy;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Callback sink that forwards sync-status snapshots to the UI layer.
type StatusBroadcaster = Arc<OnceLock<Box<dyn Fn(&SyncStatus) + Send + Sync>>>;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    /// Dedicated read-only WAL connection: searches/previews keep working
    /// while the main handle is mid-write during a long sync.
    pub(crate) read_db: Arc<Mutex<Connection>>,
    /// Cooperative cancel for long local scans / multi-source sync.
    pub cancel_sync: Arc<AtomicBool>,
    /// Whole-state guard so tray + UI syncs never run overlapping DB writes.
    pub sync_in_progress: Arc<AtomicBool>,
    pub sync_status: Arc<Mutex<SyncStatus>>,
    /// Set during setup; forwards sync-status snapshots to the UI. Kept as a
    /// plain callback so this module never drags the GUI stack into test
    /// binaries (AppHandle<Wry> monomorphization would link tao/wry).
    pub(crate) status_broadcaster: StatusBroadcaster,
    /// File watcher asks for a watch-root rebuild through this flag.
    pub(crate) watch_refresh: Arc<AtomicBool>,
}

// Cheap clone: every field is an Arc handle onto the same shared state, so the
// watcher thread can hold its own copy without touching command signatures.
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            read_db: Arc::clone(&self.read_db),
            cancel_sync: Arc::clone(&self.cancel_sync),
            sync_in_progress: Arc::clone(&self.sync_in_progress),
            sync_status: Arc::clone(&self.sync_status),
            status_broadcaster: Arc::clone(&self.status_broadcaster),
            watch_refresh: Arc::clone(&self.watch_refresh),
        }
    }
}

struct SyncRunGuard<'a>(&'a AtomicBool);

impl Drop for SyncRunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub static APP_DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    directories::ProjectDirs::from("com", "SearchDoc", "SearchDoc")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".searchdoc-data"))
});

const INDEX_FILE: &str = "index.db";
const PENDING_RESTORE_FILE: &str = "index.restore.pending";

/// After this many indexed+removed documents in one sync, merge FTS segments
/// automatically so search latency does not drift as churn accumulates.
const AUTO_FTS_OPTIMIZE_THRESHOLD: usize = 200;

impl AppState {
    pub fn init() -> AppResult<Self> {
        std::fs::create_dir_all(APP_DATA_DIR.as_path())?;
        apply_pending_restore(APP_DATA_DIR.as_path())?;
        let db_path = APP_DATA_DIR.join(INDEX_FILE);
        let db = Database::open(&db_path)?;

        // WAL lets one writer and many readers coexist; searches run here.
        let read_conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        read_conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let read_db = Arc::new(Mutex::new(read_conn));

        let google_name = if google_auth::auth_status()
            .map(|s| s.connected)
            .unwrap_or(false)
        {
            "Google Docs"
        } else {
            "Google Docs（未连接）"
        };

        db.upsert_source(
            google_auth::google_source_id(),
            SourceKind::GoogleDocs,
            google_name,
            None,
            true,
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            read_db,
            cancel_sync: Arc::new(AtomicBool::new(false)),
            sync_in_progress: Arc::new(AtomicBool::new(false)),
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            status_broadcaster: Arc::new(OnceLock::new()),
            watch_refresh: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn request_cancel_sync(&self) {
        self.cancel_sync.store(true, Ordering::SeqCst);
    }

    /// Called once from setup so background threads can push status to the UI.
    pub fn set_status_broadcaster(&self, f: Box<dyn Fn(&SyncStatus) + Send + Sync>) {
        let _ = self.status_broadcaster.set(f);
    }

    /// Ask the file watcher to reload watch roots (sources changed).
    pub fn request_watch_refresh(&self) {
        self.watch_refresh.store(true, Ordering::Release);
    }

    pub(crate) fn take_watch_refresh(&self) -> bool {
        self.watch_refresh.swap(false, Ordering::AcqRel)
    }

    pub fn clear_cancel_sync(&self) {
        self.cancel_sync.store(false, Ordering::SeqCst);
    }

    pub fn is_cancel_requested(&self) -> bool {
        self.cancel_sync.load(Ordering::Relaxed)
    }

    /// Claim the global sync lock and always release it when the run leaves scope.
    fn begin_sync(&self) -> Option<SyncRunGuard<'_>> {
        (!self.sync_in_progress.swap(true, Ordering::AcqRel))
            .then_some(SyncRunGuard(&self.sync_in_progress))
    }

    pub fn is_sync_running(&self) -> bool {
        self.sync_in_progress.load(Ordering::Acquire)
    }

    pub fn sync_status(&self) -> AppResult<SyncStatus> {
        self.sync_status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| AppError::msg("sync status lock poisoned"))
    }

    fn update_sync_status(&self, update: impl FnOnce(&mut SyncStatus)) {
        let snapshot = if let Ok(mut status) = self.sync_status.lock() {
            update(&mut status);
            Some(status.clone())
        } else {
            None
        };
        // Push transitions straight to the UI; the frontend still keeps a slow
        // polling fallback in case an event is missed.
        if let Some(snapshot) = snapshot {
            if let Some(broadcast) = self.status_broadcaster.get() {
                broadcast(&snapshot);
            }
        }
    }

    pub fn list_sources(&self) -> AppResult<Vec<SourceInfo>> {
        let db = self
            .db
            .lock()
            .map_err(|_| AppError::msg("db lock poisoned"))?;
        db.list_sources()
    }

    pub fn stats(&self) -> AppResult<IndexStats> {
        let db = self
            .db
            .lock()
            .map_err(|_| AppError::msg("db lock poisoned"))?;
        db.stats()
    }

    pub fn backup_index(&self) -> AppResult<String> {
        if self.is_sync_running() {
            return Err(AppError::msg("请等待同步完成后再备份索引"));
        }
        let db = self
            .db
            .lock()
            .map_err(|_| AppError::msg("db lock poisoned"))?;
        Ok(db.backup()?.to_string_lossy().to_string())
    }

    /// Manual「优化索引」: FTS optimize + VACUUM while blocking concurrent syncs.
    pub fn optimize_index(&self) -> AppResult<String> {
        let Some(_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        let elapsed = {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.compact()?
        };
        Ok(format!("索引已优化 · 耗时 {:.1} 秒", elapsed.as_secs_f32()))
    }

    pub fn schedule_index_restore(&self, path: String) -> AppResult<String> {
        if self.is_sync_running() {
            return Err(AppError::msg("请等待同步完成后再恢复索引"));
        }
        let source = PathBuf::from(path.trim());
        if !source.is_file() {
            return Err(AppError::msg("请选择有效的 SQLite 备份文件"));
        }
        Database::validate_backup(&source)?;

        let pending = APP_DATA_DIR.join(PENDING_RESTORE_FILE);
        let temp = APP_DATA_DIR.join("index.restore.pending.tmp");
        std::fs::copy(&source, &temp)?;
        Database::validate_backup(&temp)?;
        if pending.exists() {
            std::fs::remove_file(&pending)?;
        }
        std::fs::rename(&temp, &pending)?;
        Ok(pending.to_string_lossy().to_string())
    }

    pub fn get_document_preview(&self, document_id: String) -> AppResult<Option<String>> {
        let conn = self
            .read_db
            .lock()
            .map_err(|_| AppError::msg("read db lock poisoned"))?;
        Database::get_body_on(&conn, &document_id)
    }

    pub fn search(&self, q: SearchQuery) -> AppResult<SearchResponse> {
        let exact = matches!(
            q.mode.as_deref(),
            Some("exact") | Some("精准") | Some("precise")
        );
        let want_deep = q.deep.unwrap_or(false);
        let limit = normalize_search_limit(q.limit);
        // v1 caps: depth 1..=2, links 1..=24, seeds from top direct hits
        let max_depth = q.deep_depth.unwrap_or(1).clamp(1, 2);
        let link_limit = q.deep_link_limit.unwrap_or(12).clamp(1, 24);
        let fetch_missing = q.deep_fetch_missing.unwrap_or(false);
        let seed_limit = 8usize;

        let offset = q.offset.unwrap_or(0);
        let mut hits = {
            let conn = self
                .read_db
                .lock()
                .map_err(|_| AppError::msg("read db lock poisoned"))?;
            Database::search_on(
                &conn,
                &q.query,
                limit,
                offset,
                q.source_kind.as_deref(),
                q.mode.as_deref(),
                q.scope.as_deref(),
                q.sort.as_deref(),
            )?
        };

        let mut stats = DeepSearchStats {
            enabled: want_deep,
            max_depth: if want_deep { max_depth } else { 0 },
            ..DeepSearchStats::default()
        };

        if !want_deep || hits.is_empty() {
            return Ok(SearchResponse { hits, deep: stats });
        }

        let mut seen_ids: std::collections::HashSet<String> =
            hits.iter().map(|h| h.id.clone()).collect();
        let mut seen_external: std::collections::HashSet<String> = std::collections::HashSet::new();

        // frontier holds docs whose bodies we will scan for outbound links
        let mut frontier: Vec<SearchHit> = hits.iter().take(seed_limit).cloned().collect();
        stats.seeds = frontier.len();

        for depth in 1..=max_depth {
            if frontier.is_empty() {
                break;
            }

            // Collect unique outbound Google Docs links from frontier bodies.
            let mut linked_ids: Vec<(String, String)> = Vec::new(); // (external_id, parent_title)
            {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| AppError::msg("db lock poisoned"))?;
                for parent in &frontier {
                    if let Ok(Some(body)) = db.get_document_body_by_id(&parent.id) {
                        let mut per_doc = 0usize;
                        for (ext_id, _url) in
                            google_links::extract_google_doc_links_from_text(&body)
                        {
                            // soft per-doc fanout to avoid one noisy doc dominating
                            if per_doc >= 8 {
                                break;
                            }
                            if seen_external.insert(ext_id.clone()) {
                                linked_ids.push((ext_id, parent.title.clone()));
                                per_doc += 1;
                            }
                        }
                    }
                }
            }

            stats.links_found += linked_ids.len();
            if linked_ids.is_empty() {
                break;
            }

            // Remaining global budget
            let remaining = link_limit.saturating_sub(stats.links_followed);
            if remaining == 0 {
                break;
            }
            let linked_ids: Vec<(String, String)> =
                linked_ids.into_iter().take(remaining).collect();
            stats.links_followed += linked_ids.len();

            // Optionally fetch missing docs (only if connected + allowed)
            let missing: Vec<String> = {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| AppError::msg("db lock poisoned"))?;
                let mut missing = Vec::new();
                for (id, _) in &linked_ids {
                    if db.find_by_external_id(id)?.is_none() {
                        missing.push(id.clone());
                    }
                }
                missing
            };

            if !missing.is_empty() {
                if fetch_missing
                    && !self.is_sync_running()
                    && google_auth::auth_status()
                        .map(|s| s.connected)
                        .unwrap_or(false)
                {
                    let source_id = google_auth::google_source_id().to_string();
                    match google_links::fetch_documents_by_ids(&source_id, &missing, None) {
                        Ok((docs, errs)) => {
                            stats.fetched += docs.len();
                            stats.errors.extend(errs.into_iter().take(5));
                            let db = self
                                .db
                                .lock()
                                .map_err(|_| AppError::msg("db lock poisoned"))?;
                            for doc in &docs {
                                if let Err(err) = db.upsert_document(doc) {
                                    stats.errors.push(format!("{}: {err}", doc.uri));
                                }
                            }
                        }
                        Err(err) => stats.errors.push(err.to_string()),
                    }
                } else {
                    stats.skipped_existing += missing.len();
                }
            }

            // Evaluate linked docs; those that match become hits and next frontier.
            let mut next_frontier: Vec<SearchHit> = Vec::new();
            {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| AppError::msg("db lock poisoned"))?;
                for (ext_id, parent_title) in &linked_ids {
                    match db.find_by_external_id(ext_id) {
                        Ok(Some(mut hit)) => {
                            if seen_ids.contains(&hit.id) {
                                continue;
                            }
                            let body = db.get_document_body_by_id(&hit.id)?.unwrap_or_default();
                            if !Database::body_matches_query(&body, &q.query, exact) {
                                // Still allow deeper expansion from non-matching hubs? No —
                                // only expand matching docs to keep relevance.
                                continue;
                            }
                            hit.depth = depth;
                            hit.via = Some(parent_title.clone());
                            hit.snippet = Database::make_snippet(&body, &q.query);
                            // De-prioritize deeper hits slightly
                            hit.rank += 5.0 * depth as f64;
                            seen_ids.insert(hit.id.clone());
                            stats.linked_hits += 1;
                            next_frontier.push(hit.clone());
                            hits.push(hit);
                        }
                        Ok(None) => {
                            // not indexed and not fetched
                        }
                        Err(err) => stats.errors.push(err.to_string()),
                    }
                }
            }

            frontier = next_frontier;
        }

        hits.sort_by(|a, b| {
            a.depth.cmp(&b.depth).then_with(|| {
                a.rank
                    .partial_cmp(&b.rank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        hits.truncate(limit);
        Ok(SearchResponse { hits, deep: stats })
    }

    pub fn add_local_folder(&self, path: String) -> AppResult<SourceInfo> {
        let root = PathBuf::from(path.trim());
        if !root.exists() {
            return Err(AppError::msg(format!("路径不存在: {}", root.display())));
        }
        if !root.is_dir() {
            return Err(AppError::msg(format!(
                "请选择文件夹或磁盘根目录，而不是文件: {}",
                root.display()
            )));
        }

        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };

        // Normalize for display / uniqueness (Windows: keep drive letter casing).
        let path = root.to_string_lossy().to_string();

        // Avoid duplicate local roots.
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for src in db.list_sources()? {
                if src.kind == SourceKind::Local {
                    if let Some(existing) = src.root_path.as_ref() {
                        let a = PathBuf::from(existing);
                        if paths_same_root(&a, &root) {
                            return Err(AppError::msg(format!("该路径已在来源中：{existing}")));
                        }
                    }
                }
            }
        }

        let id = format!("local-{}", uuid::Uuid::new_v4());
        let name = local_root_display_name(&root);

        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.upsert_source(&id, SourceKind::Local, &name, Some(&path), true)?;
        }

        if let Err(error) = self.run_sync_source(id.clone()) {
            let cleanup = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?
                .delete_source(&id);
            if let Err(cleanup_error) = cleanup {
                return Err(AppError::msg(format!(
                    "{error}；清理未完成的来源失败：{cleanup_error}"
                )));
            }
            return Err(error);
        }

        let source = {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.get_source(&id)?
                .ok_or_else(|| AppError::msg("failed to load created source"))?
        };
        self.request_watch_refresh();
        Ok(source)
    }

    /// List fixed / removable drive roots (Windows) or mount-style roots.
    pub fn list_local_drives(&self) -> AppResult<Vec<LocalDriveInfo>> {
        Ok(enumerate_local_drives())
    }

    pub fn remove_source(&self, source_id: String) -> AppResult<()> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        let db = self
            .db
            .lock()
            .map_err(|_| AppError::msg("db lock poisoned"))?;
        if source_id == google_auth::google_source_id() {
            return Err(AppError::msg("Google Docs 源请用「断开连接」，而不是删除"));
        }
        db.delete_source(&source_id)?;
        self.request_watch_refresh();
        Ok(())
    }

    pub fn set_source_enabled(&self, source_id: String, enabled: bool) -> AppResult<()> {
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.set_source_enabled(&source_id, enabled)?;
        }
        self.request_watch_refresh();
        Ok(())
    }

    pub fn save_google_oauth_config(
        &self,
        client_id: String,
        client_secret: String,
    ) -> AppResult<google_auth::GoogleAuthStatus> {
        google_auth::save_client_config(client_id, client_secret)?;
        google_auth::auth_status()
    }

    pub fn google_auth_status(&self) -> AppResult<google_auth::GoogleAuthStatus> {
        google_auth::auth_status()
    }

    pub fn google_prefs(&self) -> AppResult<GooglePrefsStatus> {
        google_prefs::get_status()
    }

    pub fn set_google_sync_mode(&self, mode: String) -> AppResult<GooglePrefsStatus> {
        let _ = google_prefs::ensure_valid_mode(&mode)?;
        google_prefs::set_sync_mode(&mode)
    }

    pub fn set_google_folder_filter(&self, raw_text: String) -> AppResult<GooglePrefsStatus> {
        let (ids, invalid) = google_prefs::parse_folder_ids(&raw_text);
        if ids.is_empty() && !raw_text.trim().is_empty() {
            return Err(AppError::msg(format!(
                "无法解析文件夹链接或 ID{}",
                if invalid.is_empty() {
                    String::new()
                } else {
                    format!(
                        "：{}",
                        invalid.into_iter().take(3).collect::<Vec<_>>().join("；")
                    )
                }
            )));
        }
        google_prefs::set_folder_ids(ids)
    }

    pub fn clear_google_folder_filter(&self) -> AppResult<GooglePrefsStatus> {
        google_prefs::set_folder_ids(Vec::new())
    }

    pub fn connect_google(
        &self,
        app: &tauri::AppHandle,
    ) -> AppResult<google_auth::GoogleAuthStatus> {
        let sync_recent = google_prefs::current_mode()? == GoogleSyncMode::Recent;
        let _status = google_auth::connect_google_interactive(app)?;
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.upsert_source(
                google_auth::google_source_id(),
                SourceKind::GoogleDocs,
                "Google Docs",
                None,
                true,
            )?;
        }
        // Avoid surprising full Drive scans on first connect when mode is watchlist-only.
        if sync_recent {
            let _ = self.sync_source(google_auth::google_source_id().to_string());
        }
        google_auth::auth_status()
    }

    pub fn disconnect_google(&self) -> AppResult<google_auth::GoogleAuthStatus> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            // Clear indexed google docs.
            db.delete_missing_documents(google_auth::google_source_id(), &[])?;
        }
        let status = google_auth::disconnect_google()?;
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.upsert_source(
                google_auth::google_source_id(),
                SourceKind::GoogleDocs,
                "Google Docs（未连接）",
                None,
                true,
            )?;
        }
        Ok(status)
    }

    pub fn sync_source(&self, source_id: String) -> AppResult<SyncReport> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        self.run_sync_source(source_id)
    }

    fn run_sync_source(&self, source_id: String) -> AppResult<SyncReport> {
        self.clear_cancel_sync();
        self.update_sync_status(|status| {
            *status = SyncStatus {
                running: true,
                phase: "preparing".into(),
                source_id: Some(source_id.clone()),
                ..SyncStatus::default()
            };
        });
        let result = self.sync_source_inner(source_id);
        self.update_sync_status(|status| {
            status.running = false;
            status.phase = match &result {
                Ok(report) if report.errors.is_empty() => "completed",
                Ok(_) => "partial",
                Err(err) if err.to_string().contains("已取消") => "cancelled",
                Err(_) => "failed",
            }
            .into();
            status.message = result.as_ref().err().map(ToString::to_string);
            status.error_kind = match &result {
                Ok(report) => report
                    .errors
                    .first()
                    .map(|message| classify_sync_error(message)),
                Err(err) => Some(classify_sync_error(&err.to_string())),
            };
        });
        if let Ok(report) = &result {
            if report.indexed.saturating_add(report.removed) >= AUTO_FTS_OPTIMIZE_THRESHOLD {
                match self.db.lock() {
                    Ok(db) => match db.optimize_fts() {
                        Ok(()) => log::info!(
                            "auto FTS optimize ran after {} changes",
                            report.indexed + report.removed
                        ),
                        Err(err) => log::warn!("auto FTS optimize failed: {err}"),
                    },
                    Err(_) => log::warn!("db lock poisoned during auto FTS optimize"),
                }
            }
        }
        result
    }

    /// Validate Notion credentials, persist the token, and register the
    /// database as a searchable source.
    pub fn add_notion_database(&self, token: String, database_id: String) -> AppResult<SourceInfo> {
        let token = token.trim().to_string();
        let database_id = database_id.trim().to_string();
        if database_id.is_empty() {
            return Err(AppError::msg("请填写 Notion 数据库 ID"));
        }
        // Round-trip to Notion up front: catches bad tokens / ids / missing
        // integration access before anything is persisted.
        let title = crate::sources::notion::fetch_database_title(&token, &database_id)?;
        notion_prefs::save_token(&token)?;

        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for src in db.list_sources()? {
                if src.kind == SourceKind::Notion
                    && src.root_path.as_deref() == Some(database_id.as_str())
                {
                    return Err(AppError::msg(format!("该数据库已在来源中：{title}")));
                }
            }
            let id = format!("notion-{}", uuid::Uuid::new_v4());
            db.upsert_source(&id, SourceKind::Notion, &title, Some(&database_id), true)?;
            db.get_source(&id)?
                .ok_or_else(|| AppError::msg("来源创建失败"))
        }
    }

    /// Whether a Notion integration token has been saved.
    pub fn get_notion_status(&self) -> AppResult<bool> {
        Ok(notion_prefs::load_token()?.is_some())
    }

    fn sync_source_inner(&self, source_id: String) -> AppResult<SyncReport> {
        if self.is_cancel_requested() {
            return Err(AppError::msg("已取消同步"));
        }

        let source = {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            db.get_source(&source_id)?
                .ok_or_else(|| AppError::msg(format!("source not found: {source_id}")))?
        };

        self.update_sync_status(|status| {
            status.source_id = Some(source.id.clone());
            status.source_name = Some(source.name.clone());
            status.phase = "scanning".into();
            status.visited = 0;
            status.scanned = 0;
            status.processed = 0;
            status.indexed = 0;
            status.removed = 0;
            status.skipped = 0;
            status.errors = 0;
            status.message = None;
        });

        let mut errors = Vec::new();
        let google_mode = google_prefs::current_mode()?;
        let google_folder_ids = if google_mode == GoogleSyncMode::Recent {
            google_prefs::current_folder_ids()?
        } else {
            Vec::new()
        };
        let local_seen_external_ids = Arc::new(Mutex::new(Vec::new()));
        let local_scan_complete = Arc::new(AtomicBool::new(true));
        let local_written = Arc::new(AtomicUsize::new(0));
        let local_unchanged = Arc::new(AtomicUsize::new(0));
        let documents = match source.kind {
            SourceKind::Local => {
                let root = source
                    .root_path
                    .clone()
                    .ok_or_else(|| AppError::msg("local source missing root_path"))?;
                let status = self.sync_status.clone();
                let local_errors = Arc::new(Mutex::new(Vec::new()));
                let handler_db = self.db.clone();
                let handler_written = local_written.clone();
                let handler_unchanged = local_unchanged.clone();
                let connector = LocalFolderSource::new(source.id.clone(), root)
                    .with_cancel(self.cancel_sync.clone())
                    .with_errors(local_errors.clone())
                    .with_seen_external_ids(local_seen_external_ids.clone())
                    .with_scan_complete(local_scan_complete.clone())
                    .with_unchanged(local_unchanged.clone())
                    .with_document_handler(Arc::new(move |doc| {
                        let db = handler_db
                            .lock()
                            .map_err(|_| AppError::msg("db lock poisoned"))?;
                        if db.upsert_document(&doc)? {
                            handler_written.fetch_add(1, Ordering::Relaxed);
                        } else {
                            handler_unchanged.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(())
                    }))
                    .with_known_mtimes(Arc::new({
                        let db = self
                            .db
                            .lock()
                            .map_err(|_| AppError::msg("db lock poisoned"))?;
                        db.list_document_mtimes(&source.id)?
                    }))
                    .with_progress(Arc::new(move |visited, processed| {
                        if let Ok(mut status) = status.lock() {
                            status.visited = visited;
                            status.processed = processed;
                        }
                    }));
                let documents = match connector.scan() {
                    Ok(docs) => docs,
                    Err(err) => {
                        let msg = err.to_string();
                        if msg.contains("已取消") {
                            return Err(err);
                        }
                        local_scan_complete.store(false, Ordering::Release);
                        errors.push(msg);
                        Vec::new()
                    }
                };
                if let Ok(local_errors) = local_errors.lock() {
                    errors.extend(local_errors.iter().cloned());
                }
                documents
            }
            SourceKind::GoogleDocs => {
                match google_auth::auth_status() {
                    Ok(status) if !status.connected => {
                        errors.push("尚未连接 Google。请在「设置」完成 OAuth 连接。".into());
                        Vec::new()
                    }
                    Ok(_) => match google_mode {
                        GoogleSyncMode::WatchlistOnly => {
                            let watch = google_links::load_watchlist()?;
                            if watch.is_empty() {
                                errors.push(
                                    "当前为「仅观察列表」模式，但列表为空。请在「来源」添加 Docs 链接。".into(),
                                );
                                Vec::new()
                            } else {
                                let ids: Vec<String> = watch.iter().map(|w| w.id.clone()).collect();
                                match google_links::fetch_documents_by_ids(
                                    &source.id,
                                    &ids,
                                    Some(self.cancel_sync.as_ref()),
                                ) {
                                    Ok((docs, errs)) => {
                                        errors.extend(errs);
                                        // best-effort title refresh
                                        let titles: Vec<(String, String)> = docs
                                            .iter()
                                            .map(|d| (d.external_id.clone(), d.title.clone()))
                                            .collect();
                                        let _ = google_links::update_watch_titles(&titles);
                                        docs
                                    }
                                    Err(err) if err.to_string().contains("已取消") => {
                                        return Err(err);
                                    }
                                    Err(err) => {
                                        errors.push(err.to_string());
                                        Vec::new()
                                    }
                                }
                            }
                        }
                        GoogleSyncMode::Recent => {
                            if !google_folder_ids.is_empty() {
                                log::info!(
                                    "google docs folder filter: {} folder(s)",
                                    google_folder_ids.len()
                                );
                            }
                            let google_errors = Arc::new(Mutex::new(Vec::new()));
                            let connector = GoogleDocsSource::new(source.id.clone())
                                .with_db(self.db.clone())
                                .with_folder_ids(google_folder_ids.clone())
                                .with_cancel(self.cancel_sync.clone())
                                .with_errors(google_errors.clone());
                            let documents = match connector.scan() {
                                Ok(docs) => docs,
                                Err(err) if err.to_string().contains("已取消") => {
                                    return Err(err);
                                }
                                Err(err) => {
                                    errors.push(err.to_string());
                                    Vec::new()
                                }
                            };
                            if let Ok(google_errors) = google_errors.lock() {
                                errors.extend(google_errors.iter().cloned());
                            }
                            documents
                        }
                    },
                    Err(err) => {
                        errors.push(err.to_string());
                        Vec::new()
                    }
                }
            }
            SourceKind::Notion => match notion_prefs::load_token() {
                Ok(Some(token)) => {
                    let database_id = source
                        .root_path
                        .clone()
                        .ok_or_else(|| AppError::msg("notion source missing database_id"))?;
                    let notion_errors = Arc::new(Mutex::new(Vec::new()));
                    let connector = crate::sources::notion::NotionSource::new(
                        source.id.clone(),
                        token,
                        database_id,
                    )
                    .with_cancel(self.cancel_sync.clone())
                    .with_errors(notion_errors.clone());
                    let documents = match connector.scan() {
                        Ok(docs) => docs,
                        Err(err) if err.to_string().contains("已取消") => {
                            return Err(err);
                        }
                        Err(err) => {
                            errors.push(err.to_string());
                            Vec::new()
                        }
                    };
                    if let Ok(notion_errors) = notion_errors.lock() {
                        errors.extend(notion_errors.iter().cloned());
                    }
                    documents
                }
                Ok(None) => {
                    errors.push(
                        "尚未配置 Notion Integration Token。请在「设置 → 同步」保存。".into(),
                    );
                    Vec::new()
                }
                Err(err) => {
                    errors.push(err.to_string());
                    Vec::new()
                }
            },
        };

        let mut keep_ids: Vec<String> = documents.iter().map(|d| d.external_id.clone()).collect();
        if source.kind == SourceKind::Local {
            if let Ok(seen) = local_seen_external_ids.lock() {
                let existing: std::collections::HashSet<String> =
                    keep_ids.iter().cloned().collect();
                keep_ids.extend(seen.iter().filter(|id| !existing.contains(*id)).cloned());
            }
        } else if google_mode == GoogleSyncMode::Recent {
            // Recent Drive scans are capped; never prune explicitly tracked documents.
            let existing: std::collections::HashSet<String> = keep_ids.iter().cloned().collect();
            keep_ids.extend(
                google_links::load_watchlist()?
                    .into_iter()
                    .map(|item| item.id)
                    .filter(|id| !existing.contains(id)),
            );
        }
        let scanned = if source.kind == SourceKind::Local {
            local_seen_external_ids
                .lock()
                .map(|seen| seen.len())
                .unwrap_or_default()
        } else {
            documents.len()
        };

        self.update_sync_status(|status| {
            status.phase = "indexing".into();
            status.scanned = scanned;
            status.processed = scanned;
        });

        let mut written = local_written.load(Ordering::Relaxed);
        let mut unchanged = local_unchanged.load(Ordering::Relaxed);
        let removed = {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for doc in &documents {
                if self.is_cancel_requested() {
                    return Err(AppError::msg("已取消同步"));
                }
                match db.upsert_document(doc) {
                    Ok(true) => written += 1,
                    Ok(false) => unchanged += 1,
                    Err(err) => errors.push(format!("{}: {err}", doc.uri)),
                }
            }
            // Never purge after a partial scan or write failure. An unreadable
            // document is not proof that it was deleted.
            // ponytail: track failed local paths separately before allowing
            // partial local scans to prune unrelated stale documents.
            let purge_error_count = if source.kind == SourceKind::Local
                && local_scan_complete.load(Ordering::Acquire)
            {
                0
            } else {
                errors.len()
            };
            let should_replace = should_replace_after_scan(
                &source.kind,
                google_mode == GoogleSyncMode::Recent,
                !google_folder_ids.is_empty(),
                documents.len(),
                purge_error_count,
            );
            if self.is_cancel_requested() {
                return Err(AppError::msg("已取消同步"));
            }
            let removed = if should_replace {
                db.delete_missing_documents(&source.id, &keep_ids)?
            } else {
                0
            };
            if errors.is_empty() {
                let now = chrono::Utc::now().to_rfc3339();
                db.touch_source_sync(&source.id, &now)?;
            }
            removed
        };

        // `indexed` = newly written/updated docs; `skipped` = unchanged + hard errors.
        let report = SyncReport {
            source_id,
            indexed: written,
            removed,
            skipped: unchanged + errors.len(),
            errors,
        };
        self.update_sync_status(|status| {
            status.indexed = report.indexed;
            status.removed = report.removed;
            status.skipped = report.skipped;
            status.errors = report.errors.len();
        });
        Ok(report)
    }

    pub fn sync_all(&self) -> AppResult<Vec<SyncReport>> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        self.update_sync_status(|status| {
            *status = SyncStatus {
                running: true,
                phase: "preparing".into(),
                ..SyncStatus::default()
            };
        });
        let result = self.sync_all_inner();
        self.update_sync_status(|status| {
            status.running = false;
            status.phase = match &result {
                Ok(reports) if reports.iter().all(|report| report.errors.is_empty()) => "completed",
                Ok(_) => "partial",
                Err(err) if err.to_string().contains("已取消") => "cancelled",
                Err(_) => "failed",
            }
            .into();
            status.message = result.as_ref().err().map(ToString::to_string);
            status.error_kind = match &result {
                Ok(reports) => reports
                    .iter()
                    .find_map(|report| report.errors.first())
                    .map(|message| classify_sync_error(message)),
                Err(err) => Some(classify_sync_error(&err.to_string())),
            };
        });
        result
    }

    fn sync_all_inner(&self) -> AppResult<Vec<SyncReport>> {
        self.clear_cancel_sync();
        let sources = self.list_sources()?;
        let google_disconnected = matches!(
            google_auth::auth_status(),
            Ok(status) if !status.connected
        );
        let mut reports = Vec::new();
        for source in sources.into_iter().filter(|s| s.enabled) {
            if source.kind == SourceKind::GoogleDocs && google_disconnected {
                continue;
            }
            if self.is_cancel_requested() {
                return Err(AppError::msg("已取消同步"));
            }
            reports.push(self.sync_source_inner(source.id)?);
        }
        if reports.is_empty() {
            Err(AppError::msg("没有可同步的来源"))
        } else {
            Ok(reports)
        }
    }

    pub fn list_google_watchlist(&self) -> AppResult<Vec<GoogleWatchItem>> {
        google_links::load_watchlist()
    }

    pub fn import_google_links(&self, raw_text: String) -> AppResult<ImportLinksReport> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        self.clear_cancel_sync();
        let source_id = google_auth::google_source_id().to_string();
        let (ids, added, already, invalid) = google_links::add_links_to_watchlist(&raw_text)?;
        let (documents, mut errors) = google_links::fetch_documents_by_ids(
            &source_id,
            &ids,
            Some(self.cancel_sync.as_ref()),
        )?;

        let mut updated = 0usize;
        let mut skipped = 0usize;
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for doc in &documents {
                match db.upsert_document(doc) {
                    Ok(true) => updated += 1,
                    Ok(false) => skipped += 1,
                    Err(err) => errors.push(format!("{}: {err}", doc.uri)),
                }
            }
            if errors.is_empty() {
                let now = chrono::Utc::now().to_rfc3339();
                db.touch_source_sync(&source_id, &now)?;
            }
        }

        let titles: Vec<(String, String)> = documents
            .iter()
            .map(|d| (d.external_id.clone(), d.title.clone()))
            .collect();
        let watchlist = google_links::update_watch_titles(&titles)
            .unwrap_or_else(|_| google_links::load_watchlist().unwrap_or_default());

        Ok(ImportLinksReport {
            parsed: ids.len(),
            added,
            already_tracked: already,
            invalid,
            fetched: documents.len(),
            updated,
            skipped: skipped + errors.len(),
            errors,
            watchlist,
        })
    }

    pub fn sync_google_watchlist(&self) -> AppResult<ImportLinksReport> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        self.clear_cancel_sync();
        let source_id = google_auth::google_source_id().to_string();
        let watchlist = google_links::load_watchlist()?;
        if watchlist.is_empty() {
            return Err(AppError::msg("观察列表为空。请先粘贴 Google Docs 链接。"));
        }
        let ids: Vec<String> = watchlist.iter().map(|i| i.id.clone()).collect();
        let (documents, mut errors) = google_links::fetch_documents_by_ids(
            &source_id,
            &ids,
            Some(self.cancel_sync.as_ref()),
        )?;

        let mut updated = 0usize;
        let mut skipped = 0usize;
        {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for doc in &documents {
                match db.upsert_document(doc) {
                    Ok(true) => updated += 1,
                    Ok(false) => skipped += 1,
                    Err(err) => errors.push(format!("{}: {err}", doc.uri)),
                }
            }
            if errors.is_empty() {
                let now = chrono::Utc::now().to_rfc3339();
                db.touch_source_sync(&source_id, &now)?;
            }
        }

        let titles: Vec<(String, String)> = documents
            .iter()
            .map(|d| (d.external_id.clone(), d.title.clone()))
            .collect();
        let watchlist = google_links::update_watch_titles(&titles)?;

        Ok(ImportLinksReport {
            parsed: ids.len(),
            added: 0,
            already_tracked: ids.len(),
            invalid: Vec::new(),
            fetched: documents.len(),
            updated,
            skipped: skipped + errors.len(),
            errors,
            watchlist,
        })
    }

    pub fn remove_google_watch_ids(&self, ids: Vec<String>) -> AppResult<Vec<GoogleWatchItem>> {
        let Some(_sync_guard) = self.begin_sync() else {
            return Err(AppError::msg("已有同步在运行，请稍候再试"));
        };
        let list = google_links::remove_watch_ids(&ids)?;
        if !ids.is_empty() {
            let db = self
                .db
                .lock()
                .map_err(|_| AppError::msg("db lock poisoned"))?;
            for id in &ids {
                db.delete_document_by_external_id(google_auth::google_source_id(), id)?;
            }
        }
        Ok(list)
    }
}

fn paths_same_root(a: &Path, b: &Path) -> bool {
    let na = normalize_path_key(a);
    let nb = normalize_path_key(b);
    na == nb
}

fn normalize_search_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(40).clamp(1, 500)
}

fn apply_pending_restore(data_dir: &Path) -> AppResult<()> {
    let pending = data_dir.join(PENDING_RESTORE_FILE);
    if !pending.exists() {
        return Ok(());
    }
    Database::validate_backup(&pending)?;

    let index = data_dir.join(INDEX_FILE);
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
    let rescue = data_dir.join(format!("index-before-restore-{stamp}.db"));
    let mut moved = Vec::new();
    for (current, saved) in std::iter::once((index.clone(), rescue.clone())).chain(
        ["-wal", "-shm"].into_iter().map(|suffix| {
            (
                sqlite_sidecar_path(&index, suffix),
                sqlite_sidecar_path(&rescue, suffix),
            )
        }),
    ) {
        if current.exists() {
            if let Err(error) = std::fs::rename(&current, &saved) {
                rollback_renames(&moved);
                return Err(error.into());
            }
            moved.push((current, saved));
        }
    }
    if let Err(err) = std::fs::rename(&pending, &index) {
        rollback_renames(&moved);
        return Err(err.into());
    }
    Ok(())
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn rollback_renames(moved: &[(PathBuf, PathBuf)]) {
    for (original, saved) in moved.iter().rev() {
        if saved.exists() {
            let _ = std::fs::rename(saved, original);
        }
    }
}

fn should_replace_after_scan(
    kind: &SourceKind,
    google_recent: bool,
    google_has_folder_filter: bool,
    documents: usize,
    errors: usize,
) -> bool {
    if errors > 0 {
        return false;
    }
    match kind {
        SourceKind::Local => true,
        SourceKind::GoogleDocs => google_recent && (!google_has_folder_filter || documents > 0),
        // A successful query always lists the database authoritatively.
        SourceKind::Notion => true,
    }
}

fn classify_sync_error(message: &str) -> SyncErrorKind {
    let text = message.to_ascii_lowercase();
    if text.contains("permission")
        || text.contains("access denied")
        || text.contains("权限")
        || text.contains("拒绝访问")
    {
        SyncErrorKind::AccessDenied
    } else if text.contains("network")
        || text.contains("timeout")
        || text.contains("http")
        || text.contains("google")
        || text.contains("notion")
    {
        SyncErrorKind::Network
    } else if text.contains("parse")
        || text.contains("extract")
        || text.contains("docx")
        || text.contains("pdf")
    {
        SyncErrorKind::Parse
    } else if text.contains("not found") || text.contains("不存在") {
        SyncErrorKind::MissingSource
    } else if text.contains("已取消") || text.contains("cancel") {
        SyncErrorKind::Cancelled
    } else {
        SyncErrorKind::Unknown
    }
}

fn normalize_path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('/', "\\");
    let mut t = s.trim_end_matches('\\').to_string();
    // Keep "C:" style as "C:\" for drive roots comparison consistency.
    if t.len() == 2 && t.as_bytes().get(1) == Some(&b':') {
        t.push('\\');
    }
    t.to_ascii_lowercase()
}

fn local_root_display_name(root: &Path) -> String {
    // Windows drive root: C:\ → "本地磁盘 (C:)"
    if let Some(s) = root.to_str() {
        let t = s.trim_end_matches(['\\', '/']);
        if t.len() == 2 && t.as_bytes()[1] == b':' {
            let letter = t.chars().next().unwrap_or('?').to_ascii_uppercase();
            return format!("本地磁盘 ({letter}:)");
        }
    }
    root.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| root.to_string_lossy().to_string())
}

fn enumerate_local_drives() -> Vec<LocalDriveInfo> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        // A: … Z:
        for code in b'A'..=b'Z' {
            let letter = code as char;
            let path = format!("{letter}:\\");
            let p = PathBuf::from(&path);
            if p.is_dir() {
                out.push(LocalDriveInfo {
                    path,
                    label: format!("本地磁盘 ({letter}:)"),
                });
            }
        }
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/", "/home", "/Users", "/Volumes"] {
            let p = PathBuf::from(candidate);
            if p.is_dir() {
                out.push(LocalDriveInfo {
                    path: candidate.to_string(),
                    label: candidate.to_string(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        apply_pending_restore, classify_sync_error, normalize_search_limit,
        should_replace_after_scan, sqlite_sidecar_path, AppState, INDEX_FILE, PENDING_RESTORE_FILE,
    };
    use crate::db::Database;
    use crate::models::{DocumentRecord, SourceKind, SyncErrorKind, SyncStatus};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sync_errors_are_classified_by_cause() {
        assert_eq!(
            classify_sync_error("access denied"),
            SyncErrorKind::AccessDenied
        );
        assert_eq!(
            classify_sync_error("network timeout"),
            SyncErrorKind::Network
        );
        assert_eq!(
            classify_sync_error("docx parse failed"),
            SyncErrorKind::Parse
        );
        assert_eq!(
            classify_sync_error("source not found"),
            SyncErrorKind::MissingSource
        );
        assert_eq!(classify_sync_error("已取消同步"), SyncErrorKind::Cancelled);
    }

    #[test]
    fn search_limit_is_bounded_at_the_command_boundary() {
        assert_eq!(normalize_search_limit(None), 40);
        assert_eq!(normalize_search_limit(Some(0)), 1);
        assert_eq!(normalize_search_limit(Some(50_000)), 500);
    }

    #[test]
    fn partial_scan_never_purges_existing_index() {
        assert!(!should_replace_after_scan(
            &SourceKind::Local,
            false,
            false,
            1,
            1
        ));
        assert!(!should_replace_after_scan(
            &SourceKind::GoogleDocs,
            true,
            false,
            1,
            1
        ));
    }

    #[test]
    fn clean_local_scan_can_remove_deleted_files() {
        assert!(should_replace_after_scan(
            &SourceKind::Local,
            false,
            false,
            0,
            0
        ));
        assert!(should_replace_after_scan(
            &SourceKind::GoogleDocs,
            true,
            false,
            0,
            0
        ));
    }

    #[test]
    fn empty_filtered_google_scan_does_not_purge_existing_index() {
        assert!(!should_replace_after_scan(
            &SourceKind::GoogleDocs,
            true,
            true,
            0,
            0
        ));
        assert!(should_replace_after_scan(
            &SourceKind::GoogleDocs,
            true,
            true,
            1,
            0
        ));
    }

    #[test]
    fn rejected_sync_does_not_clear_active_cancellation() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-sync-lock-test-{nanos}.db"));
        let state = AppState {
            db: Arc::new(Mutex::new(Database::open(&path).unwrap())),
            read_db: Arc::new(Mutex::new(rusqlite::Connection::open(&path).unwrap())),
            cancel_sync: Arc::new(AtomicBool::new(true)),
            sync_in_progress: Arc::new(AtomicBool::new(true)),
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            status_broadcaster: Arc::new(std::sync::OnceLock::new()),
            watch_refresh: Arc::new(AtomicBool::new(false)),
        };

        assert!(state.sync_source("unused".into()).is_err());
        assert!(state.is_cancel_requested());

        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_error_keeps_that_file_but_removes_other_deleted_documents() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("searchdoc-partial-scan-test-{nanos}"));
        let root = dir.join("source");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("broken.docx"), "not a docx archive").unwrap();

        let root_path = root.to_string_lossy().to_string();
        let old_path = root.join("old.txt").to_string_lossy().to_string();
        let broken_path = root.join("broken.docx").to_string_lossy().to_string();
        let db = Database::open(dir.join("index.db")).unwrap();
        db.upsert_source(
            "local-test",
            SourceKind::Local,
            "Test",
            Some(&root_path),
            true,
        )
        .unwrap();
        db.upsert_document(&DocumentRecord {
            id: "old-doc".into(),
            source_id: "local-test".into(),
            source_kind: SourceKind::Local,
            external_id: old_path.clone(),
            title: "old.txt".into(),
            uri: old_path.clone(),
            body: "keep this indexed content".into(),
            mtime: None,
            content_hash: "old-hash".into(),
        })
        .unwrap();
        db.upsert_document(&DocumentRecord {
            id: "broken-doc".into(),
            source_id: "local-test".into(),
            source_kind: SourceKind::Local,
            external_id: broken_path.clone(),
            title: "broken.docx".into(),
            uri: broken_path.clone(),
            body: "previously readable content".into(),
            mtime: None,
            content_hash: "broken-old-hash".into(),
        })
        .unwrap();

        let state = AppState {
            db: Arc::new(Mutex::new(db)),
            read_db: Arc::new(Mutex::new(
                rusqlite::Connection::open(dir.join("index.db")).unwrap(),
            )),
            cancel_sync: Arc::new(AtomicBool::new(false)),
            sync_in_progress: Arc::new(AtomicBool::new(false)),
            sync_status: Arc::new(Mutex::new(SyncStatus::default())),
            status_broadcaster: Arc::new(std::sync::OnceLock::new()),
            watch_refresh: Arc::new(AtomicBool::new(false)),
        };
        let report = state.sync_source_inner("local-test".into()).unwrap();
        assert!(!report.errors.is_empty());
        assert_eq!(report.removed, 1);
        let db = state.db.lock().unwrap();
        assert!(db.find_by_external_id(&old_path).unwrap().is_none());
        assert!(db.find_by_external_id(&broken_path).unwrap().is_some());
        drop(db);

        drop(state);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pending_restore_replaces_index_and_keeps_rescue_copy() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("searchdoc-restore-test-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();

        let current = Database::open(dir.join(INDEX_FILE)).unwrap();
        current
            .upsert_source("old", SourceKind::Local, "Old", None, true)
            .unwrap();
        drop(current);

        let pending = Database::open(dir.join(PENDING_RESTORE_FILE)).unwrap();
        pending
            .upsert_source("new-1", SourceKind::Local, "New 1", None, true)
            .unwrap();
        pending
            .upsert_source("new-2", SourceKind::Local, "New 2", None, true)
            .unwrap();
        drop(pending);

        let old_index = dir.join(INDEX_FILE);
        std::fs::write(sqlite_sidecar_path(&old_index, "-wal"), "old wal").unwrap();
        std::fs::write(sqlite_sidecar_path(&old_index, "-shm"), "old shm").unwrap();

        apply_pending_restore(&dir).unwrap();
        let rescue = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("index-before-restore-") && name.ends_with(".db")
                })
            })
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&rescue, "-wal")).unwrap(),
            "old wal"
        );
        assert_eq!(
            std::fs::read_to_string(sqlite_sidecar_path(&rescue, "-shm")).unwrap(),
            "old shm"
        );
        let restored = Database::open(dir.join(INDEX_FILE)).unwrap();
        assert_eq!(restored.stats().unwrap().source_count, 2);
        drop(restored);
        let _ = std::fs::remove_dir_all(dir);
    }
}
