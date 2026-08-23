use crate::error::{AppError, AppResult};
use crate::models::{DocumentRecord, IndexStats, SearchHit, SourceInfo, SourceKind};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};

pub(crate) const GOOGLE_UNCHANGED_HASH_PREFIX: &str = "searchdoc-unchanged:";

pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sources (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                root_path TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_sync_at TEXT,
                config_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                source_id TEXT NOT NULL,
                source_kind TEXT NOT NULL,
                external_id TEXT NOT NULL,
                title TEXT NOT NULL,
                uri TEXT NOT NULL,
                body TEXT NOT NULL,
                mtime TEXT,
                content_hash TEXT NOT NULL,
                indexed_at TEXT NOT NULL,
                UNIQUE(source_id, external_id),
                FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
                title,
                body,
                content='documents',
                content_rowid='rowid',
                tokenize = 'unicode61'
            );

            CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
                INSERT INTO documents_fts(rowid, title, body)
                VALUES (new.rowid, new.title, new.body);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, title, body)
                VALUES ('delete', old.rowid, old.title, old.body);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_au AFTER UPDATE ON documents BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, title, body)
                VALUES ('delete', old.rowid, old.title, old.body);
                INSERT INTO documents_fts(rowid, title, body)
                VALUES (new.rowid, new.title, new.body);
            END;
            "#,
        )?;

        migrate(&conn)?;

        Ok(Self {
            conn,
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn upsert_source(
        &self,
        id: &str,
        kind: SourceKind,
        name: &str,
        root_path: Option<&str>,
        enabled: bool,
    ) -> AppResult<()> {
        self.conn.execute(
            r#"
            INSERT INTO sources (id, kind, name, root_path, enabled)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                name = excluded.name,
                root_path = excluded.root_path,
                enabled = excluded.enabled
            "#,
            params![id, kind.as_str(), name, root_path, enabled as i64],
        )?;
        Ok(())
    }

    pub fn list_sources(&self) -> AppResult<Vec<SourceInfo>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                s.id,
                s.kind,
                s.name,
                s.root_path,
                s.enabled,
                s.last_sync_at,
                (
                    SELECT COUNT(*) FROM documents d WHERE d.source_id = s.id
                ) AS doc_count
            FROM sources s
            ORDER BY s.kind, s.name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let kind_raw: String = row.get(1)?;
            Ok(SourceInfo {
                id: row.get(0)?,
                kind: SourceKind::parse(&kind_raw).unwrap_or(SourceKind::Local),
                name: row.get(2)?,
                root_path: row.get(3)?,
                enabled: row.get::<_, i64>(4)? == 1,
                last_sync_at: row.get(5)?,
                doc_count: row.get(6)?,
            })
        })?;

        let mut sources = Vec::new();
        for row in rows {
            sources.push(row?);
        }
        Ok(sources)
    }

    pub fn get_source(&self, id: &str) -> AppResult<Option<SourceInfo>> {
        self.conn
            .query_row(
                r#"
                SELECT
                    s.id,
                    s.kind,
                    s.name,
                    s.root_path,
                    s.enabled,
                    s.last_sync_at,
                    (
                        SELECT COUNT(*) FROM documents d WHERE d.source_id = s.id
                    ) AS doc_count
                FROM sources s
                WHERE s.id = ?1
                "#,
                params![id],
                |row| {
                    let kind_raw: String = row.get(1)?;
                    Ok(SourceInfo {
                        id: row.get(0)?,
                        kind: SourceKind::parse(&kind_raw).unwrap_or(SourceKind::Local),
                        name: row.get(2)?,
                        root_path: row.get(3)?,
                        enabled: row.get::<_, i64>(4)? == 1,
                        last_sync_at: row.get(5)?,
                        doc_count: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn set_source_enabled(&self, id: &str, enabled: bool) -> AppResult<()> {
        let changed = self.conn.execute(
            "UPDATE sources SET enabled = ?1 WHERE id = ?2",
            params![enabled as i64, id],
        )?;
        if changed == 0 {
            return Err(AppError::msg(format!("source not found: {id}")));
        }
        Ok(())
    }

    pub fn touch_source_sync(&self, id: &str, synced_at: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE sources SET last_sync_at = ?1 WHERE id = ?2",
            params![synced_at, id],
        )?;
        Ok(())
    }

    pub fn delete_source(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM sources WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Returns true when content was written/updated; false when unchanged and skipped.
    pub fn upsert_document(&self, doc: &DocumentRecord) -> AppResult<bool> {
        // Recent-mode scans use an explicit marker when modifiedTime proves a
        // Google document is unchanged. A real empty Google document must still
        // clear previously indexed text.
        if doc.source_kind == SourceKind::GoogleDocs
            && doc.body.is_empty()
            && doc.content_hash.starts_with(GOOGLE_UNCHANGED_HASH_PREFIX)
        {
            return Ok(false);
        }

        if let Some((existing_hash, existing_mtime)) =
            self.get_document_fingerprint(&doc.source_id, &doc.external_id)?
        {
            let hash_same = existing_hash == doc.content_hash;
            let mtime_same = match (&existing_mtime, &doc.mtime) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };
            if hash_same && mtime_same {
                return Ok(false);
            }
        }

        let indexed_at = chrono::Utc::now().to_rfc3339();
        // Space-separate CJK so unicode61 FTS can match multi-char Chinese queries.
        let title = cjk_expand(&doc.title);
        let body = cjk_expand(&doc.body);
        self.conn.execute(
            r#"
            INSERT INTO documents (
                id, source_id, source_kind, external_id, title, uri, body, mtime, content_hash, indexed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(source_id, external_id) DO UPDATE SET
                id = excluded.id,
                title = excluded.title,
                uri = excluded.uri,
                body = excluded.body,
                mtime = excluded.mtime,
                content_hash = excluded.content_hash,
                indexed_at = excluded.indexed_at,
                source_kind = excluded.source_kind
            "#,
            params![
                doc.id,
                doc.source_id,
                doc.source_kind.as_str(),
                doc.external_id,
                title,
                doc.uri,
                body,
                doc.mtime,
                doc.content_hash,
                indexed_at,
            ],
        )?;
        Ok(true)
    }

    pub fn get_document_fingerprint(
        &self,
        source_id: &str,
        external_id: &str,
    ) -> AppResult<Option<(String, Option<String>)>> {
        self.conn
            .query_row(
                r#"
                SELECT content_hash, mtime
                FROM documents
                WHERE source_id = ?1 AND external_id = ?2
                "#,
                params![source_id, external_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn list_document_mtimes(
        &self,
        source_id: &str,
    ) -> AppResult<std::collections::HashMap<String, Option<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT external_id, mtime FROM documents WHERE source_id = ?1")?;
        let rows = stmt.query_map(params![source_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut result = std::collections::HashMap::new();
        for row in rows {
            let (external_id, mtime) = row?;
            result.insert(external_id, mtime);
        }
        Ok(result)
    }

    pub fn get_document_body_by_id(&self, id: &str) -> AppResult<Option<String>> {
        Self::get_body_on(&self.conn, id)
    }

    pub(crate) fn get_body_on(conn: &Connection, id: &str) -> AppResult<Option<String>> {
        conn.query_row(
            "SELECT body FROM documents WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(AppError::from)
        .map(|opt| opt.map(|body| collapse_cjk_spaces(&body)))
    }

    pub fn find_by_external_id(&self, external_id: &str) -> AppResult<Option<SearchHit>> {
        self.conn
            .query_row(
                r#"
                SELECT id, source_id, source_kind, title, uri, mtime,
                       substr(body, 1, 240) AS preview
                FROM documents
                WHERE external_id = ?1
                LIMIT 1
                "#,
                params![external_id],
                |row| {
                    let kind_raw: String = row.get(2)?;
                    let title: String = row.get(3)?;
                    let preview: String = row.get(6)?;
                    Ok(SearchHit {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        source_kind: SourceKind::parse(&kind_raw).unwrap_or(SourceKind::Local),
                        title: collapse_cjk_spaces(&title),
                        uri: row.get(4)?,
                        snippet: collapse_cjk_spaces(&preview.replace('\n', " ")),
                        rank: 0.0,
                        mtime: row.get(5)?,
                        depth: 0,
                        via: None,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    /// Whether collapsed body contains query under fuzzy/exact rules (lightweight).
    pub fn body_matches_query(body: &str, query: &str, exact: bool) -> bool {
        let q = collapse_cjk_spaces(query.trim());
        if q.is_empty() {
            return false;
        }
        let hay = collapse_cjk_spaces(body);
        if exact {
            let compact_hay: String = hay.chars().filter(|c| !c.is_whitespace()).collect();
            let compact_q: String = q.chars().filter(|c| !c.is_whitespace()).collect();
            return !compact_q.is_empty() && compact_hay.contains(&compact_q);
        }
        // Fuzzy: all whitespace-separated tokens (and CJK chars) should appear.
        let tokens: Vec<String> = q
            .split_whitespace()
            .flat_map(|t| {
                if t.chars().any(is_cjk) {
                    t.chars()
                        .filter(|c| !c.is_whitespace())
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                } else {
                    vec![t.to_ascii_lowercase()]
                }
            })
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            return hay.to_ascii_lowercase().contains(&q.to_ascii_lowercase());
        }
        let hay_l = hay.to_ascii_lowercase();
        tokens.iter().all(|t| {
            if t.chars().any(is_cjk) {
                hay.contains(t.as_str())
            } else {
                hay_l.contains(&t.to_ascii_lowercase())
            }
        })
    }

    pub fn make_snippet(body: &str, query: &str) -> String {
        let hay = collapse_cjk_spaces(body);
        let q = collapse_cjk_spaces(query.trim());
        if q.is_empty() {
            let mut s: String = hay.chars().take(120).collect();
            if hay.chars().count() > 120 {
                s.push('…');
            }
            return s;
        }

        // Prefer continuous phrase window centered on full query.
        let chars: Vec<char> = hay.chars().collect();
        let q_chars: Vec<char> = q.chars().filter(|c| !c.is_whitespace()).collect();
        let compact: Vec<char> = chars
            .iter()
            .copied()
            .filter(|c| !c.is_whitespace())
            .collect();

        let mut found_compact = None;
        if !q_chars.is_empty() && compact.len() >= q_chars.len() {
            'outer: for i in 0..=(compact.len() - q_chars.len()) {
                if compact[i..i + q_chars.len()] == q_chars[..] {
                    found_compact = Some(i);
                    break 'outer;
                }
            }
        }

        if let Some(ci) = found_compact {
            // Map compact index back to original char index approximately.
            let mut seen = 0usize;
            let mut start_char = 0usize;
            for (i, ch) in chars.iter().enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                if seen == ci {
                    start_char = i;
                    break;
                }
                seen += 1;
            }
            let start = start_char.saturating_sub(36);
            let end = (start_char + q_chars.len() + 48).min(chars.len());
            let mut out = String::new();
            if start > 0 {
                out.push('…');
            }
            out.extend(chars[start..end].iter());
            if end < chars.len() {
                out.push('…');
            }
            out
        } else if let Some(i) = hay.find(&q) {
            let start = i.saturating_sub(40);
            let end = (i + q.chars().count() + 40).min(hay.chars().count());
            let snippet: String = hay
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
            let mut out = String::new();
            if start > 0 {
                out.push('…');
            }
            out.push_str(&snippet);
            if end < hay.chars().count() {
                out.push('…');
            }
            out
        } else {
            let mut s: String = hay.chars().take(120).collect();
            if hay.chars().count() > 120 {
                s.push('…');
            }
            s
        }
    }

    pub fn delete_document_by_external_id(
        &self,
        source_id: &str,
        external_id: &str,
    ) -> AppResult<usize> {
        let removed = self.conn.execute(
            "DELETE FROM documents WHERE source_id = ?1 AND external_id = ?2",
            params![source_id, external_id],
        )?;
        Ok(removed)
    }

    pub fn delete_missing_documents(
        &self,
        source_id: &str,
        keep_external_ids: &[String],
    ) -> AppResult<usize> {
        if keep_external_ids.is_empty() {
            let removed = self.conn.execute(
                "DELETE FROM documents WHERE source_id = ?1",
                params![source_id],
            )?;
            return Ok(removed);
        }

        // A temporary table avoids SQLite's bound-parameter ceiling on large drives.
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(
            "DROP TABLE IF EXISTS temp.searchdoc_keep_documents;
             CREATE TEMP TABLE searchdoc_keep_documents (
                 external_id TEXT PRIMARY KEY
             ) WITHOUT ROWID;",
        )?;
        {
            let mut insert = tx.prepare(
                "INSERT OR IGNORE INTO temp.searchdoc_keep_documents (external_id) VALUES (?1)",
            )?;
            for id in keep_external_ids {
                insert.execute(params![id])?;
            }
        }
        let removed = tx.execute(
            "DELETE FROM documents
             WHERE source_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM temp.searchdoc_keep_documents keep
                   WHERE keep.external_id = documents.external_id
               )",
            params![source_id],
        )?;
        tx.execute_batch("DROP TABLE temp.searchdoc_keep_documents;")?;
        tx.commit()?;
        Ok(removed)
    }

    /// Convenience wrapper on the main connection; production reads go
    /// through AppState's read-only handle calling `search_on` directly.
    #[cfg(test)]
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        source_kind: Option<&str>,
        mode: Option<&str>,
        scope: Option<&str>,
        sort: Option<&str>,
    ) -> AppResult<Vec<SearchHit>> {
        Self::search_on(&self.conn, query, limit, 0, source_kind, mode, scope, sort)
    }

    /// Same query against any connection, plus cursor pagination. Lets AppState
    /// serve reads from a dedicated read-only WAL connection while all writes
    /// keep flowing through the main handle.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_on(
        conn: &Connection,
        query: &str,
        limit: usize,
        offset: usize,
        source_kind: Option<&str>,
        mode: Option<&str>,
        scope: Option<&str>,
        sort: Option<&str>,
    ) -> AppResult<Vec<SearchHit>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let exact = matches!(mode, Some("exact") | Some("精准") | Some("precise"));
        let scope_norm = match scope {
            Some("title") | Some("标题") => "title",
            Some("body") | Some("正文") => "body",
            _ => "all",
        };
        let sort_mtime = matches!(
            sort,
            Some("mtime") | Some("time") | Some("最新") | Some("modified")
        );
        let match_query = build_fts_query(trimmed, exact, scope_norm);
        if match_query.is_empty() {
            return Ok(Vec::new());
        }

        let kind_filter = match source_kind {
            Some("local") | Some("google_docs") | Some("notion") => source_kind,
            _ => None,
        };

        let order_sql = if sort_mtime {
            "ORDER BY d.mtime IS NULL, d.mtime DESC, rank"
        } else {
            "ORDER BY rank"
        };

        // Exact / title scope: plain window; UI phrase-highlights. Fuzzy body: FTS snippet.
        let snip_sql = if exact || scope_norm == "title" {
            "substr(d.body, 1, 280)"
        } else {
            "snippet(documents_fts, 1, '⟦', '⟧', '…', 18)"
        };

        // Exact mode re-verifies the continuous phrase against the real body.
        // Fetching it inside the main query removes the former per-hit
        // get_document_body_by_id round-trip (N+1 at LIMIT=500).
        let body_col = if exact { ", d.body AS full_body" } else { "" };

        let sql = if kind_filter.is_some() {
            format!(
                r#"
            SELECT
                d.id,
                d.source_id,
                d.source_kind,
                d.title,
                d.uri,
                {snip_sql} AS snip,
                bm25(documents_fts) AS rank,
                d.mtime{body_col}
            FROM documents_fts
            JOIN documents d ON d.rowid = documents_fts.rowid
            JOIN sources s ON s.id = d.source_id
            WHERE documents_fts MATCH ?1
              AND s.enabled = 1
              AND d.source_kind = ?3
            {order_sql}
            LIMIT ?2 OFFSET ?4
            "#
            )
        } else {
            format!(
                r#"
            SELECT
                d.id,
                d.source_id,
                d.source_kind,
                d.title,
                d.uri,
                {snip_sql} AS snip,
                bm25(documents_fts) AS rank,
                d.mtime{body_col}
            FROM documents_fts
            JOIN documents d ON d.rowid = documents_fts.rowid
            JOIN sources s ON s.id = d.source_id
            WHERE documents_fts MATCH ?1
              AND s.enabled = 1
            {order_sql}
            LIMIT ?2 OFFSET ?3
            "#
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(SearchHit, Option<String>)> {
            let kind_raw: String = row.get(2)?;
            let title: String = row.get(3)?;
            let snippet: String = row.get::<_, String>(5)?.replace('\n', " ");
            // Only selected in exact mode (see body_col); stored bodies are
            // CJK-expanded, so collapse them exactly like get_document_body_by_id.
            let full_body: Option<String> = if exact {
                row.get::<_, Option<String>>(8)?
                    .map(|b| collapse_cjk_spaces(&b))
            } else {
                None
            };
            Ok((
                SearchHit {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    source_kind: SourceKind::parse(&kind_raw).unwrap_or(SourceKind::Local),
                    title: collapse_cjk_spaces(&title),
                    uri: row.get(4)?,
                    snippet: collapse_cjk_spaces(&snippet),
                    rank: row.get(6)?,
                    mtime: row.get(7)?,
                    depth: 0,
                    via: None,
                },
                full_body,
            ))
        };

        let rows = if let Some(kind) = kind_filter {
            stmt.query_map(
                params![match_query, limit as i64, kind, offset as i64],
                map_row,
            )?
        } else {
            stmt.query_map(params![match_query, limit as i64, offset as i64], map_row)?
        };

        let mut hits = Vec::new();
        for row in rows {
            let (mut hit, full_body) = row?;
            if exact {
                // Strict continuous phrase — scope-aware.
                let title = hit.title.clone();
                let body = full_body.unwrap_or_default();
                let ok = match scope_norm {
                    "title" => Self::body_matches_query(&title, trimmed, true),
                    "body" => Self::body_matches_query(&body, trimmed, true),
                    _ => {
                        Self::body_matches_query(&title, trimmed, true)
                            || Self::body_matches_query(&body, trimmed, true)
                    }
                };
                if !ok {
                    continue;
                }
                if scope_norm == "title" && Self::body_matches_query(&title, trimmed, true) {
                    hit.snippet = format!("标题命中 · {title}");
                    hit.rank -= 20.0;
                } else {
                    hit.snippet = Self::make_snippet(&body, trimmed);
                    hit.rank -= 10.0;
                }
            } else if scope_norm == "title" {
                hit.snippet = format!("标题 · {}", hit.title);
            }
            hits.push(hit);
        }
        if exact && !sort_mtime {
            hits.sort_by(|a, b| {
                a.rank
                    .partial_cmp(&b.rank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        Ok(hits)
    }

    pub fn stats(&self) -> AppResult<IndexStats> {
        let document_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
        let source_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))?;
        let local_doc_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE source_kind = 'local'",
            [],
            |row| row.get(0),
        )?;
        let google_doc_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE source_kind = 'google_docs'",
            [],
            |row| row.get(0),
        )?;

        Ok(IndexStats {
            document_count,
            source_count,
            local_doc_count,
            google_doc_count,
            db_path: self.path.to_string_lossy().to_string(),
        })
    }

    pub fn backup(&self) -> AppResult<PathBuf> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::msg("index database has no parent directory"))?;
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
        let backup = parent.join(format!("index-backup-{stamp}.db"));
        self.conn
            .execute("VACUUM INTO ?1", params![backup.to_string_lossy()])?;
        Ok(backup)
    }

    pub fn validate_backup(path: &Path) -> AppResult<()> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(AppError::msg(format!("备份校验失败：{check}")));
        }
        conn.query_row("SELECT COUNT(*) FROM sources", [], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(())
    }

    /// Merge FTS5 b-tree segments after heavy churn. Cheap enough to run
    /// automatically at the end of a large sync.
    pub fn optimize_fts(&self) -> AppResult<()> {
        self.conn
            .execute_batch("INSERT INTO documents_fts(documents_fts) VALUES('optimize');")?;
        Ok(())
    }

    /// Full compaction for the manual「优化索引」action:
    /// FTS optimize plus VACUUM to reclaim deleted pages. Returns elapsed time.
    pub fn compact(&self) -> AppResult<std::time::Duration> {
        let started = std::time::Instant::now();
        self.conn.execute_batch(
            "INSERT INTO documents_fts(documents_fts) VALUES('optimize'); VACUUM;",
        )?;
        Ok(started.elapsed())
    }
}

const SCHEMA_VERSION: i32 = 3;

/// Apply incremental, non-destructive schema migrations.
///
/// `PRAGMA user_version` records how far the schema has been upgraded. Future
/// changes must be added as guarded steps here (e.g. `if version < 3 { ... }`),
/// never as a destructive drop-and-rebuild — the previous approach threw away
/// every user's index on schema bumps.
///
/// Fresh installs start at 0; the idempotent `CREATE TABLE IF NOT EXISTS` DDL
/// above lays down the base schema regardless of version.
fn migrate(conn: &Connection) -> AppResult<()> {
    let mut version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 2 {
        // v1 -> v2: no base-table change was required for this bump — the old
        // eager rebuild only recreated identical tables and discarded the index.
        // Preserve existing documents/sources and just advance the version.
        version = 2;
    }

    if version < 3 {
        // v2 -> v3: "newest" ordering sorts on documents.mtime; previously every
        // sorted query scanned the full FTS join result. Non-destructive.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_documents_mtime ON documents(mtime)",
            [],
        )?;
        version = 3;
    }

    if version < SCHEMA_VERSION {
        version = SCHEMA_VERSION;
    }

    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{3000}'..='\u{303F}'
            | '\u{FF00}'..='\u{FFEF}'
    )
}

/// Insert spaces between CJK chars so FTS unicode61 treats them as tokens.
fn cjk_expand(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut prev_cjk = false;
    for ch in text.chars() {
        let cur_cjk = is_cjk(ch);
        if cur_cjk {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push(ch);
            prev_cjk = true;
        } else {
            if prev_cjk && !ch.is_whitespace() {
                out.push(' ');
            }
            out.push(ch);
            prev_cjk = false;
        }
    }
    out
}

fn collapse_cjk_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == ' ' && i > 0 && i + 1 < chars.len() && is_cjk(chars[i - 1]) && is_cjk(chars[i + 1])
        {
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn sanitize_query_token(token: &str) -> String {
    token
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .replace('"', " ")
        .trim()
        .to_string()
}

fn column_prefix(scope: &str) -> &'static str {
    match scope {
        "title" => "title:",
        "body" => "body:",
        _ => "",
    }
}

fn build_fts_query(input: &str, exact: bool, scope: &str) -> String {
    if exact {
        return build_exact_fts_query(input, scope);
    }

    // Support OR groups: `foo OR bar` / `foo | bar` (case-insensitive OR).
    let normalized = input
        .replace('|', " OR ")
        .split_whitespace()
        .map(|t| t.to_string())
        .collect::<Vec<_>>();
    let mut groups: Vec<Vec<String>> = vec![Vec::new()];
    for tok in normalized {
        if tok.eq_ignore_ascii_case("OR") {
            if groups.last().map(|g| !g.is_empty()).unwrap_or(false) {
                groups.push(Vec::new());
            }
            continue;
        }
        if let Some(last) = groups.last_mut() {
            last.push(tok);
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.is_empty() {
        return String::new();
    }

    let prefix = column_prefix(scope);
    let rendered: Vec<String> = groups
        .into_iter()
        .filter_map(|group| {
            let parts = build_fuzzy_and_clause(&group.join(" "), prefix);
            if parts.is_empty() {
                None
            } else {
                Some(parts)
            }
        })
        .collect();
    if rendered.is_empty() {
        return String::new();
    }
    if rendered.len() == 1 {
        rendered.into_iter().next().unwrap_or_default()
    } else {
        rendered
            .into_iter()
            .map(|g| format!("({g})"))
            .collect::<Vec<_>>()
            .join(" OR ")
    }
}

fn render_fuzzy_token(token: &str, column_prefix: &str) -> String {
    let cleaned = sanitize_query_token(token);
    if cleaned.is_empty() {
        return String::new();
    }
    let expanded = cjk_expand(&cleaned);
    if cleaned.chars().any(is_cjk) {
        expanded
            .split_whitespace()
            .map(|part| format!("{column_prefix}\"{part}\""))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        format!("{column_prefix}\"{cleaned}\"*")
    }
}

fn build_fuzzy_and_clause(input: &str, column_prefix: &str) -> String {
    // Fuzzy: AND of tokens; support NOT / -term exclusion (FTS5).
    // Examples: `roadmap NOT draft`, `架构 -废弃`
    let raw: Vec<&str> = input.split_whitespace().filter(|t| !t.is_empty()).collect();
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let tok = raw[i];
        let (negate, term) = if tok.eq_ignore_ascii_case("NOT") || tok == "-" {
            i += 1;
            if i >= raw.len() {
                break;
            }
            (true, raw[i])
        } else if let Some(rest) = tok.strip_prefix('-').filter(|r| !r.is_empty() && *r != "-") {
            (true, rest)
        } else if tok.eq_ignore_ascii_case("AND") {
            i += 1;
            continue;
        } else {
            (false, tok)
        };

        let rendered = render_fuzzy_token(term, column_prefix);
        if !rendered.is_empty() {
            if negate {
                // Unary NOT applies to the following term/phrase.
                parts.push(format!("NOT ({rendered})"));
            } else {
                parts.push(rendered);
            }
        }
        i += 1;
    }
    parts.join(" ")
}

fn build_exact_fts_query(input: &str, scope: &str) -> String {
    let cleaned = sanitize_query_token(input);
    if cleaned.is_empty() {
        return String::new();
    }
    let prefix = column_prefix(scope);

    // Treat the whole query as one phrase when possible.
    // CJK is stored char-tokenized, so phrase = ordered adjacent chars.
    if cleaned.chars().any(is_cjk) {
        let expanded = cjk_expand(&cleaned);
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        if parts.is_empty() {
            return String::new();
        }
        // FTS5 phrase: "t1 t2 t3" requires adjacent tokens in order.
        format!("{prefix}\"{}\"", parts.join(" "))
    } else {
        // Latin/other: exact phrase, no prefix wildcard.
        format!("{prefix}\"{cleaned}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DocumentRecord;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn indexes_and_searches_documents() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-test-{nanos}.db"));
        let db = Database::open(&path).expect("open db");
        db.upsert_source(
            "local-1",
            SourceKind::Local,
            "Notes",
            Some("C:/notes"),
            true,
        )
        .unwrap();
        let doc = DocumentRecord {
            id: "doc-1".into(),
            source_id: "local-1".into(),
            source_kind: SourceKind::Local,
            external_id: "C:/notes/a.md".into(),
            title: "welcome.md".into(),
            uri: "C:/notes/a.md".into(),
            body: "SearchDoc 支持片段预览与统一索引".into(),
            mtime: Some("2026-01-01T00:00:00Z".into()),
            content_hash: "abc".into(),
        };
        assert!(db.upsert_document(&doc).unwrap());
        assert!(
            !db.upsert_document(&doc).unwrap(),
            "unchanged doc should skip"
        );

        let hits = db
            .search("片段", 10, None, Some("fuzzy"), None, None)
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].snippet.contains("片段") || hits[0].title.contains("welcome"));
        let local_only = db
            .search("片段", 10, Some("local"), None, None, None)
            .unwrap();
        assert!(!local_only.is_empty());
        let google_only = db
            .search("片段", 10, Some("google_docs"), None, None, None)
            .unwrap();
        assert!(google_only.is_empty());

        let exact_hits = db
            .search("片段预览", 10, None, Some("exact"), None, None)
            .unwrap();
        assert!(!exact_hits.is_empty());
        let fuzzy_partial = db
            .search("预览片段", 10, None, Some("fuzzy"), None, None)
            .unwrap();
        // fuzzy may still match reordered CJK chars; exact phrase should be stricter on order
        let exact_reordered = db
            .search("预览片段", 10, None, Some("exact"), None, None)
            .unwrap();
        assert!(
            exact_reordered.len() <= fuzzy_partial.len(),
            "exact should not be looser than fuzzy"
        );

        // Exclusion: document has 片段 but not 不存在的词 — NOT should still hit.
        let not_hits = db
            .search("片段 NOT 不存在词xyz", 10, None, Some("fuzzy"), None, None)
            .unwrap();
        assert!(!not_hits.is_empty(), "NOT unknown term should keep match");

        // Dash exclusion form
        let dash_hits = db
            .search("片段 -不存在词xyz", 10, None, Some("fuzzy"), None, None)
            .unwrap();
        assert!(!dash_hits.is_empty(), "-term exclusion should keep match");

        let empty_doc = DocumentRecord {
            id: "doc-1-empty".into(),
            body: String::new(),
            mtime: Some("2026-01-02T00:00:00Z".into()),
            content_hash: "empty".into(),
            ..doc
        };
        assert!(db.upsert_document(&empty_doc).unwrap());
        assert_eq!(
            db.get_document_body_by_id("doc-1-empty").unwrap(),
            Some(String::new())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn google_empty_exports_clear_stale_text_but_skip_markers_do_not() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-google-empty-{nanos}.db"));
        let db = Database::open(&path).expect("open db");
        db.upsert_source(
            "google-1",
            SourceKind::GoogleDocs,
            "Google Docs",
            None,
            true,
        )
        .unwrap();
        let old = DocumentRecord {
            id: "google-old".into(),
            source_id: "google-1".into(),
            source_kind: SourceKind::GoogleDocs,
            external_id: "remote-1".into(),
            title: "Document".into(),
            uri: "https://docs.google.com/document/d/remote-1/edit".into(),
            body: "stale text".into(),
            mtime: Some("2026-01-01T00:00:00Z".into()),
            content_hash: "old-hash".into(),
        };
        assert!(db.upsert_document(&old).unwrap());

        let marker = DocumentRecord {
            id: "google-marker".into(),
            body: String::new(),
            content_hash: format!("{GOOGLE_UNCHANGED_HASH_PREFIX}old-hash"),
            ..old.clone()
        };
        assert!(!db.upsert_document(&marker).unwrap());
        assert_eq!(
            db.get_document_body_by_id("google-old").unwrap(),
            Some("stale text".into())
        );

        let empty = DocumentRecord {
            id: "google-empty".into(),
            body: String::new(),
            mtime: Some("2026-01-02T00:00:00Z".into()),
            content_hash: "empty-hash".into(),
            ..old
        };
        assert!(db.upsert_document(&empty).unwrap());
        assert_eq!(
            db.get_document_body_by_id("google-empty").unwrap(),
            Some(String::new())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn builds_not_clause_in_fts_query() {
        let q = build_fts_query("foo NOT bar", false, "all");
        assert!(q.contains("NOT"), "query={q}");
        assert!(q.contains("foo") || q.contains("\"foo\""), "query={q}");
        let q2 = build_fts_query("架构 -草稿", false, "body");
        assert!(q2.contains("NOT"), "query={q2}");
        assert!(q2.contains("body:"), "query={q2}");
    }

    #[test]
    fn creates_consistent_backup() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-backup-test-{nanos}.db"));
        let db = Database::open(&path).expect("open db");
        db.upsert_source("local-1", SourceKind::Local, "Notes", None, true)
            .unwrap();

        let backup = db.backup().expect("create backup");
        let restored = Database::open(&backup).expect("open backup");
        assert_eq!(restored.stats().unwrap().source_count, 1);

        drop(restored);
        drop(db);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(backup);
    }

    fn temp_db() -> (Database, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-m3-test-{nanos}.db"));
        let db = Database::open(&path).expect("open db");
        db.upsert_source(
            "local-1",
            SourceKind::Local,
            "Notes",
            Some("C:/notes"),
            true,
        )
        .unwrap();
        (db, path)
    }

    fn doc(id: &str, title: &str, body: &str, uri: &str, hash: &str) -> DocumentRecord {
        DocumentRecord {
            id: id.into(),
            source_id: "local-1".into(),
            source_kind: SourceKind::Local,
            external_id: uri.into(),
            title: title.into(),
            uri: uri.into(),
            body: body.into(),
            mtime: Some("2026-01-01T00:00:00Z".into()),
            content_hash: hash.into(),
        }
    }

    #[test]
    fn sorts_by_mtime_when_requested() {
        let (db, path) = temp_db();
        db.upsert_document(&doc(
            "old",
            "note old",
            "keyword alpha present",
            "C:/n/old.md",
            "h-old",
        ))
        .unwrap();
        let new = DocumentRecord {
            id: "new".into(),
            mtime: Some("2026-02-01T00:00:00Z".into()),
            ..doc(
                "new",
                "note new",
                "keyword alpha present",
                "C:/n/new.md",
                "h-new",
            )
        };
        db.upsert_document(&new).unwrap();

        let hits = db
            .search("keyword alpha", 10, None, None, None, None)
            .unwrap();
        assert_eq!(hits.len(), 2, "both docs should match");
        let newest = db
            .search("keyword alpha", 10, None, None, None, Some("mtime"))
            .unwrap();
        assert_eq!(newest[0].id, "new", "mtime sort should place newest first");
        assert_eq!(newest[1].id, "old");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn title_scope_matches_only_title_field() {
        let (db, path) = temp_db();
        db.upsert_document(&doc(
            "t",
            "独特标题词组",
            "正文不出现该词",
            "C:/n/t.md",
            "h-t",
        ))
        .unwrap();
        db.upsert_document(&doc(
            "b",
            "普通标题",
            "正文包含 独特标题词组",
            "C:/n/b.md",
            "h-b",
        ))
        .unwrap();

        let in_title = db
            .search("独特标题词组", 10, None, None, Some("title"), None)
            .unwrap();
        assert_eq!(in_title.len(), 1);
        assert_eq!(in_title[0].id, "t");

        let in_body = db
            .search("独特标题词组", 10, None, None, Some("body"), None)
            .unwrap();
        assert_eq!(in_body.len(), 1);
        assert_eq!(in_body[0].id, "b");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn or_syntax_matches_either_term() {
        let (db, path) = temp_db();
        db.upsert_document(&doc(
            "a",
            "a",
            "contains AlphaOne engine term",
            "C:/n/a.md",
            "h-a",
        ))
        .unwrap();
        db.upsert_document(&doc(
            "b",
            "b",
            "contains BetaTwo engine term",
            "C:/n/b.md",
            "h-b",
        ))
        .unwrap();

        let or = db
            .search("AlphaOne OR BetaTwo", 10, None, None, None, None)
            .unwrap();
        assert_eq!(or.len(), 2, "OR query should match both docs");
        let pipe = db
            .search("AlphaOne|BetaTwo", 10, None, None, None, None)
            .unwrap();
        assert_eq!(pipe.len(), 2, "pipe OR should match both docs");
        let and = db
            .search("AlphaOne BetaTwo", 10, None, None, None, None)
            .unwrap();
        assert!(and.is_empty(), "plain AND should match neither doc");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn delete_missing_documents_removes_only_stale() {
        let (db, path) = temp_db();
        db.upsert_document(&doc("d1", "one", "body one", "C:/n/1.md", "h1"))
            .unwrap();
        db.upsert_document(&doc("d2", "two", "body two", "C:/n/2.md", "h2"))
            .unwrap();
        db.upsert_document(&doc("d3", "three", "body three", "C:/n/3.md", "h3"))
            .unwrap();

        let removed = db
            .delete_missing_documents("local-1", &["C:/n/1.md".into(), "C:/n/3.md".into()])
            .unwrap();
        assert_eq!(removed, 1, "only the stale document should be removed");
        assert!(db.find_by_external_id("C:/n/1.md").unwrap().is_some());
        assert!(db.find_by_external_id("C:/n/2.md").unwrap().is_none());

        let all = db.delete_missing_documents("local-1", &[]).unwrap();
        assert_eq!(all, 2);
        assert_eq!(db.stats().unwrap().document_count, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn delete_missing_documents_handles_more_ids_than_sqlite_parameters() {
        let (db, path) = temp_db();
        db.upsert_document(&doc("kept", "kept", "body", "C:/n/kept.md", "h1"))
            .unwrap();
        db.upsert_document(&doc("stale", "stale", "body", "C:/n/stale.md", "h2"))
            .unwrap();
        let mut keep: Vec<String> = (0..33_000).map(|i| format!("C:/n/{i}.md")).collect();
        keep.push("C:/n/kept.md".into());

        assert_eq!(db.delete_missing_documents("local-1", &keep).unwrap(), 1);
        assert!(db.find_by_external_id("C:/n/kept.md").unwrap().is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn mixed_cjk_latin_fuzzy_search() {
        let (db, path) = temp_db();
        db.upsert_document(&doc(
            "mixed",
            "SearchDoc 说明",
            "SearchDoc 支持 片段预览 与统一索引",
            "C:/n/mixed.md",
            "h-mixed",
        ))
        .unwrap();

        let q1 = db
            .search("SearchDoc 片段", 10, None, None, None, None)
            .unwrap();
        assert_eq!(q1.len(), 1);
        let q2 = db.search("片段 预览", 10, None, None, None, None).unwrap();
        assert_eq!(q2.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn results_survive_reopen() {
        let (db, path) = temp_db();
        db.upsert_document(&doc(
            "keep",
            "持久化标题",
            "重启后仍能搜到的正文",
            "C:/n/keep.md",
            "h-keep",
        ))
        .unwrap();
        drop(db);

        let reopened = Database::open(&path).expect("reopen db");
        let hits = reopened
            .search("重启后仍能搜到", 10, None, None, None, None)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "keep");
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn make_snippet_returns_window_around_match() {
        let filler = "填充文本填充文本填充文本填充文本填充文本填充文本填充文本填充文本";
        let body = format!("{filler} SearchDoc 关键命中词 {filler}");
        let snip = Database::make_snippet(&body, "关键命中词");
        assert!(
            snip.contains("关键命中词"),
            "snippet should keep the match, got: {snip}"
        );
        assert!(
            snip.len() < body.len(),
            "snippet should be a trimmed window"
        );
    }

    #[test]
    #[ignore = "performance baseline"]
    fn search_baseline_for_5000_documents() {
        let (db, path) = temp_db();
        for i in 0..5_000 {
            db.upsert_document(&doc(
                &format!("perf-{i}"),
                &format!("note-{i}.md"),
                &format!("document {i} contains the searchable needle and context"),
                &format!("C:/n/perf-{i}.md"),
                &format!("hash-{i}"),
            ))
            .unwrap();
        }

        let started = std::time::Instant::now();
        let hits = db
            .search("searchable needle", 50, None, None, None, None)
            .unwrap();
        let elapsed = started.elapsed();
        eprintln!("search 5000 docs: {elapsed:?}, hits={}", hits.len());
        assert_eq!(hits.len(), 50);
        let _ = std::fs::remove_file(path);
    }
}
