use crate::error::{AppError, AppResult};
use crate::models::{DocumentRecord, SourceKind};
use crate::sources::{hash_text, SourceConnector};
use docx_rs::{
    read_docx, DocumentChild, ParagraphChild, RunChild, TableCellContent, TableChild, TableRowChild,
};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "markdown", "log", "csv", "pdf", "docx"];
// ponytail: fixed ceiling keeps whole-drive scans bounded; make it per-source only if real files exceed it.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Parse candidates in bounded parallel batches: PDF/DOCX extraction is the
/// CPU-heavy part, directory walking stays sequential, and memory stays capped
/// at one batch of extracted documents in flight.
const PARSE_BATCH: usize = 64;

pub struct LocalFolderSource {
    pub source_id: String,
    pub root: PathBuf,
    /// When set, cooperative cancel checked while walking files.
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub progress: Option<Arc<dyn Fn(usize, usize) + Send + Sync>>,
    pub errors: Option<Arc<Mutex<Vec<String>>>>,
    pub known_mtimes: Option<Arc<HashMap<String, Option<String>>>>,
    pub seen_external_ids: Option<Arc<Mutex<Vec<String>>>>,
    pub scan_complete: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub unchanged: Option<Arc<std::sync::atomic::AtomicUsize>>,
    pub document_handler: Option<Arc<dyn Fn(DocumentRecord) -> AppResult<()> + Send + Sync>>,
}

impl LocalFolderSource {
    pub fn new(source_id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            source_id: source_id.into(),
            root: root.into(),
            cancel: None,
            progress: None,
            errors: None,
            known_mtimes: None,
            seen_external_ids: None,
            scan_complete: None,
            unchanged: None,
            document_handler: None,
        }
    }

    pub fn with_cancel(mut self, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_progress(mut self, progress: Arc<dyn Fn(usize, usize) + Send + Sync>) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn with_errors(mut self, errors: Arc<Mutex<Vec<String>>>) -> Self {
        self.errors = Some(errors);
        self
    }

    pub fn with_known_mtimes(mut self, mtimes: Arc<HashMap<String, Option<String>>>) -> Self {
        self.known_mtimes = Some(mtimes);
        self
    }

    pub fn with_seen_external_ids(mut self, ids: Arc<Mutex<Vec<String>>>) -> Self {
        self.seen_external_ids = Some(ids);
        self
    }

    pub fn with_scan_complete(mut self, complete: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.scan_complete = Some(complete);
        self
    }

    pub fn with_unchanged(mut self, unchanged: Arc<std::sync::atomic::AtomicUsize>) -> Self {
        self.unchanged = Some(unchanged);
        self
    }

    pub fn with_document_handler(
        mut self,
        handler: Arc<dyn Fn(DocumentRecord) -> AppResult<()> + Send + Sync>,
    ) -> Self {
        self.document_handler = Some(handler);
        self
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    }

    fn record_error(&self, message: String) {
        if let Some(errors) = &self.errors {
            if let Ok(mut errors) = errors.lock() {
                if errors.len() < 100 {
                    errors.push(message);
                }
            }
        }
    }
}

impl SourceConnector for LocalFolderSource {
    fn scan(&self) -> AppResult<Vec<DocumentRecord>> {
        if !self.root.exists() {
            if let Some(complete) = &self.scan_complete {
                complete.store(false, std::sync::atomic::Ordering::Release);
            }
            return Err(AppError::msg(format!(
                "local folder does not exist: {}",
                self.root.display()
            )));
        }

        let mut docs = Vec::new();
        let mut worklist: Vec<PathBuf> = Vec::new();
        let mut seen = 0usize;
        let mut processed = 0usize;
        for entry in WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !should_skip_under(&self.root, entry.path()))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    if let Some(complete) = &self.scan_complete {
                        complete.store(false, std::sync::atomic::Ordering::Release);
                    }
                    self.record_error(format!("目录遍历失败: {err}"));
                    continue;
                }
            };
            seen += 1;
            if let Some(progress) = &self.progress {
                progress(seen, processed);
            }
            if seen.is_multiple_of(64) && self.is_cancelled() {
                return Err(AppError::msg("已取消同步"));
            }
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if should_skip_under(&self.root, path) {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            if self.is_cancelled() {
                return Err(AppError::msg("已取消同步"));
            }

            let external_id = path.to_string_lossy().to_string();
            if let Some(seen) = &self.seen_external_ids {
                if let Ok(mut seen) = seen.lock() {
                    seen.push(external_id.clone());
                }
            }
            let metadata = fs::metadata(path).ok();
            let current_mtime = metadata
                .as_ref()
                .and_then(|meta| meta.modified().ok())
                .and_then(system_time_to_rfc3339);
            if self
                .known_mtimes
                .as_ref()
                .and_then(|known| known.get(&external_id))
                .is_some_and(|known| known == &current_mtime)
            {
                // ponytail: mtime-only fast path; persist size too if preserved timestamps cause stale hits.
                processed += 1;
                if let Some(unchanged) = &self.unchanged {
                    unchanged.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                continue;
            }

            if metadata
                .as_ref()
                .is_some_and(|metadata| metadata.len() > MAX_FILE_BYTES)
            {
                processed += 1;
                self.record_error(format!("{}: 文件超过 64 MiB，已跳过", path.display()));
                continue;
            }

            worklist.push(path.to_path_buf());
        }

        // Phase 2: parse in bounded parallel batches. PDF/DOCX extraction is
        // CPU-bound; the document handler (DB write) stays on this thread so
        // writes remain serialized and handler errors abort the scan exactly
        // like the sequential version did.
        for batch in worklist.chunks(PARSE_BATCH) {
            if self.is_cancelled() {
                return Err(AppError::msg("已取消同步"));
            }
            let source_id = &self.source_id;
            let parsed: Vec<_> = batch
                .par_iter()
                .map(|path| read_document(source_id, path))
                .collect();
            for (path, result) in batch.iter().zip(parsed) {
                match result {
                    Ok(doc) => {
                        if let Some(handler) = &self.document_handler {
                            handler(doc)?;
                        } else {
                            docs.push(doc);
                        }
                    }
                    Err(err) => {
                        log::warn!("skip {}: {err}", path.display());
                        self.record_error(format!("{}: {err}", path.display()));
                    }
                }
                processed += 1;
            }
            if let Some(progress) = &self.progress {
                progress(seen, processed);
            }
        }

        if self.is_cancelled() {
            return Err(AppError::msg("已取消同步"));
        }
        if let Some(progress) = &self.progress {
            progress(seen, processed);
        }

        Ok(docs)
    }
}

fn should_skip_under(root: &Path, path: &Path) -> bool {
    should_skip(path.strip_prefix(root).unwrap_or(path))
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        let n = name.as_ref();
        // Dev / VCS noise
        if matches!(
            n,
            "node_modules"
                | ".git"
                | "target"
                | "dist"
                | ".venv"
                | "venv"
                | "__pycache__"
                | ".next"
                | ".cache"
                | ".turbo"
                | "bower_components"
        ) {
            return true;
        }
        // Windows system / junk when indexing a whole drive
        if matches!(
            n,
            "Windows"
                | "Program Files"
                | "Program Files (x86)"
                | "ProgramData"
                | "$Recycle.Bin"
                | "System Volume Information"
                | "Recovery"
                | "Config.Msi"
                | "AppData"
                | "Intel"
                | "PerfLogs"
                | "MSOCache"
        ) {
            return true;
        }
        // Hidden / system-ish dot dirs (except common user content roots we might want)
        if n.starts_with('.') && n != ".config" {
            return true;
        }
        false
    })
}

fn read_document(source_id: &str, path: &Path) -> AppResult<DocumentRecord> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let body = match ext.as_str() {
        "pdf" => extract_pdf(path)?,
        "docx" => extract_docx(path)?,
        _ => read_plain_text(path)?,
    };

    let external_id = path.to_string_lossy().to_string();
    let title = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| external_id.clone());
    let mtime = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(system_time_to_rfc3339);
    let content_hash = hash_text(&body);
    let id = uuid::Uuid::new_v4().to_string();

    Ok(DocumentRecord {
        id,
        source_id: source_id.to_string(),
        source_kind: SourceKind::Local,
        external_id,
        title,
        uri: path.to_string_lossy().to_string(),
        body,
        mtime,
        content_hash,
    })
}

fn read_plain_text(path: &Path) -> AppResult<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(_) => Ok(String::from_utf8_lossy(&fs::read(path)?).into_owned()),
    }
}

fn extract_pdf(path: &Path) -> AppResult<String> {
    pdf_extract::extract_text(path).map_err(|e| AppError::msg(format!("pdf extract failed: {e}")))
}

fn extract_docx(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path)?;
    let docx = read_docx(&bytes).map_err(|e| AppError::msg(format!("docx read failed: {e}")))?;
    let mut out = String::new();

    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                let line = paragraph_text(p);
                if !line.is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&line);
                }
            }
            DocumentChild::Table(table) => {
                for row_child in &table.rows {
                    let TableChild::TableRow(row) = row_child;
                    let mut cells = Vec::new();
                    for cell_child in &row.cells {
                        let TableRowChild::TableCell(cell) = cell_child;
                        let mut cell_text = String::new();
                        for content in &cell.children {
                            if let TableCellContent::Paragraph(p) = content {
                                let t = paragraph_text(p.as_ref());
                                if !t.is_empty() {
                                    if !cell_text.is_empty() {
                                        cell_text.push(' ');
                                    }
                                    cell_text.push_str(&t);
                                }
                            }
                        }
                        if !cell_text.is_empty() {
                            cells.push(cell_text);
                        }
                    }
                    if !cells.is_empty() {
                        if !out.is_empty() {
                            out.push('\n');
                        }
                        out.push_str(&cells.join(" | "));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

fn paragraph_text(paragraph: &docx_rs::Paragraph) -> String {
    let mut line = String::new();
    for child in &paragraph.children {
        if let ParagraphChild::Run(run) = child {
            for rc in &run.children {
                if let RunChild::Text(t) = rc {
                    line.push_str(&t.text);
                }
            }
        }
    }
    line
}

fn system_time_to_rfc3339(time: SystemTime) -> Option<String> {
    let dt: chrono::DateTime<chrono::Utc> = time.into();
    Some(dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn file_parse_errors_are_reported_to_the_caller() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("searchdoc-local-scan-test-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.docx"), "not a docx archive").unwrap();

        let errors = Arc::new(Mutex::new(Vec::new()));
        let source = LocalFolderSource::new("local-test", &dir).with_errors(errors.clone());
        assert!(source.scan().unwrap().is_empty());
        assert_eq!(errors.lock().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_plain_text_file_returns_an_error() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("searchdoc-missing-{nanos}.txt"));

        assert!(read_plain_text(&path).is_err());
    }

    #[test]
    fn document_handler_streams_without_collecting_bodies() {
        let dir =
            std::env::temp_dir().join(format!("searchdoc-stream-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("one.txt"), "one").unwrap();
        std::fs::write(dir.join("two.md"), "two").unwrap();

        let handled = Arc::new(AtomicUsize::new(0));
        let handled_in_callback = handled.clone();
        let documents = LocalFolderSource::new("local-test", &dir)
            .with_document_handler(Arc::new(move |_| {
                handled_in_callback.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }))
            .scan()
            .unwrap();

        assert!(documents.is_empty());
        assert_eq!(handled.load(Ordering::Relaxed), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
