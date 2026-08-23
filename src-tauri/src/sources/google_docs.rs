use crate::db::{Database, GOOGLE_UNCHANGED_HASH_PREFIX};
use crate::error::{AppError, AppResult};
use crate::google_auth;
use crate::models::{DocumentRecord, SourceKind};
use crate::sources::{hash_text, SourceConnector};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const DRIVE_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const MAX_DOCS_DEFAULT: usize = 200;

pub struct GoogleDocsSource {
    pub source_id: String,
    pub max_docs: usize,
    /// Optional DB handle for incremental skip by modifiedTime/hash fingerprint.
    pub db: Option<std::sync::Arc<Mutex<Database>>>,
    /// When non-empty, only Docs that are direct children of these folders (OR).
    pub folder_ids: Vec<String>,
    pub cancel: Option<Arc<AtomicBool>>,
    pub errors: Option<Arc<Mutex<Vec<String>>>>,
}

impl GoogleDocsSource {
    pub fn new(source_id: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            max_docs: MAX_DOCS_DEFAULT,
            db: None,
            folder_ids: Vec::new(),
            cancel: None,
            errors: None,
        }
    }

    pub fn with_db(mut self, db: std::sync::Arc<Mutex<Database>>) -> Self {
        self.db = Some(db);
        self
    }

    pub fn with_folder_ids(mut self, folder_ids: Vec<String>) -> Self {
        self.folder_ids = folder_ids;
        self
    }

    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_errors(mut self, errors: Arc<Mutex<Vec<String>>>) -> Self {
        self.errors = Some(errors);
        self
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .map(|cancel| cancel.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    fn finish_scan(
        &self,
        documents: Vec<DocumentRecord>,
        errors: Vec<String>,
    ) -> AppResult<Vec<DocumentRecord>> {
        if errors.is_empty() {
            return Ok(documents);
        }
        if let Some(target) = &self.errors {
            target
                .lock()
                .map_err(|_| AppError::msg("google scan errors lock poisoned"))?
                .extend(errors);
            Ok(documents)
        } else {
            Err(AppError::msg(format!(
                "Google Docs 同步失败：{}",
                errors.join("；")
            )))
        }
    }
}

impl SourceConnector for GoogleDocsSource {
    fn scan(&self) -> AppResult<Vec<DocumentRecord>> {
        if self.is_cancelled() {
            return Err(AppError::msg("已取消同步"));
        }
        let access_token = google_auth::get_valid_access_token()?;
        let files = list_google_docs(&access_token, self.max_docs, &self.folder_ids)?;
        let client = reqwest::blocking::Client::new();
        let mut docs = Vec::new();
        let mut errors = Vec::new();
        let mut skipped_unchanged = 0usize;

        for file in files {
            if self.is_cancelled() {
                return Err(AppError::msg("已取消同步"));
            }
            // Incremental: if fingerprint already matches modifiedTime, skip export.
            if let Some(db_lock) = &self.db {
                if let Ok(db) = db_lock.lock() {
                    if let Ok(Some((existing_hash, existing_mtime))) =
                        db.get_document_fingerprint(&self.source_id, &file.id)
                    {
                        if let (Some(existing), Some(remote)) =
                            (existing_mtime.as_ref(), file.modified_time.as_ref())
                        {
                            if existing == remote {
                                // Keep the document by re-emitting a lightweight stub?
                                // We need keep_ids for delete_missing; fetch minimal record from export skip:
                                // Use a placeholder body that won't be written if hash+mtime match.
                                // Safer: export only when mtime differs. For keep list we still need external_id.
                                docs.push(DocumentRecord {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    source_id: self.source_id.clone(),
                                    source_kind: SourceKind::GoogleDocs,
                                    external_id: file.id.clone(),
                                    title: file.name.clone(),
                                    uri: file.web_view_link.clone().unwrap_or_else(|| {
                                        format!(
                                            "https://docs.google.com/document/d/{}/edit",
                                            file.id
                                        )
                                    }),
                                    // Keep the ID for pruning without exporting the body.
                                    body: String::new(),
                                    mtime: file.modified_time.clone(),
                                    content_hash: format!(
                                        "{GOOGLE_UNCHANGED_HASH_PREFIX}{existing_hash}"
                                    ),
                                });
                                skipped_unchanged += 1;
                                continue;
                            }
                        }
                    }
                }
            }

            match export_doc_text(&client, &access_token, &file.id) {
                Ok(body) => {
                    let uri = file.web_view_link.clone().unwrap_or_else(|| {
                        format!("https://docs.google.com/document/d/{}/edit", file.id)
                    });
                    let content_hash = hash_text(&body);
                    docs.push(DocumentRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        source_id: self.source_id.clone(),
                        source_kind: SourceKind::GoogleDocs,
                        external_id: file.id.clone(),
                        title: file.name,
                        uri,
                        body,
                        mtime: file.modified_time,
                        content_hash,
                    });
                }
                Err(err) => {
                    errors.push(format!("{}: {err}", file.name));
                }
            }
            // Be gentle with Drive API free-tier quotas.
            thread::sleep(Duration::from_millis(80));
        }

        if !errors.is_empty() {
            log::warn!("google docs partial errors: {}", errors.join(" | "));
        }
        if skipped_unchanged > 0 {
            log::info!("google docs skipped unchanged: {skipped_unchanged}");
        }

        if self.is_cancelled() {
            return Err(AppError::msg("已取消同步"));
        }
        self.finish_scan(docs, errors)
    }
}

#[derive(Debug, Deserialize)]
struct DriveFileList {
    files: Option<Vec<DriveFile>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(rename = "modifiedTime")]
    modified_time: Option<String>,
    #[serde(rename = "webViewLink")]
    web_view_link: Option<String>,
}

fn build_docs_query(folder_ids: &[String]) -> String {
    let base = "mimeType = 'application/vnd.google-apps.document' and trashed = false";
    let folders: Vec<&str> = folder_ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if folders.is_empty() {
        return base.to_string();
    }
    // Direct children only (non-recursive). Multi-folder = OR.
    let parents = folders
        .iter()
        .map(|id| {
            // Escape single quotes in id (Drive q syntax).
            let escaped = id.replace('\'', "\\'");
            format!("'{escaped}' in parents")
        })
        .collect::<Vec<_>>()
        .join(" or ");
    format!("{base} and ({parents})")
}

fn list_google_docs(
    access_token: &str,
    max_docs: usize,
    folder_ids: &[String],
) -> AppResult<Vec<DriveFile>> {
    let client = reqwest::blocking::Client::new();
    let mut page_token: Option<String> = None;
    let mut out = Vec::new();
    let q = build_docs_query(folder_ids);

    loop {
        let mut url = url::Url::parse(DRIVE_FILES_URL).map_err(|e| AppError::msg(e.to_string()))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("q", &q);
            qp.append_pair("spaces", "drive");
            qp.append_pair("pageSize", "50");
            qp.append_pair(
                "fields",
                "nextPageToken, files(id, name, modifiedTime, webViewLink)",
            );
            qp.append_pair("orderBy", "modifiedTime desc");
            if let Some(token) = &page_token {
                qp.append_pair("pageToken", token);
            }
        }

        let resp = send_with_retry(|| {
            client
                .get(url.clone())
                .bearer_auth(access_token)
                .header("Accept", "application/json")
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(AppError::msg(format!(
                "列出 Google Docs 被拒绝 ({status}): {text}"
            )));
        }

        let parsed: DriveFileList = resp
            .json()
            .map_err(|e| AppError::msg(format!("解析 Docs 列表失败: {e}")))?;

        if let Some(files) = parsed.files {
            for f in files {
                out.push(f);
                if out.len() >= max_docs {
                    return Ok(out);
                }
            }
        }

        match parsed.next_page_token {
            Some(token) if !token.is_empty() => page_token = Some(token),
            _ => break,
        }
    }

    Ok(out)
}

/// Send a GET with exponential backoff on 429 / 5xx (Google quota and
/// transient hiccups). Up to 3 attempts: 400ms, then 800ms.
fn send_with_retry(
    build: impl Fn() -> reqwest::blocking::RequestBuilder,
) -> AppResult<reqwest::blocking::Response> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        let resp: reqwest::blocking::Response = build()
            .send()
            .map_err(|e| AppError::msg(format!("网络请求失败: {e}")))?;
        let status = resp.status();
        let retryable = status.as_u16() == 429 || status.is_server_error();
        if !retryable || attempt >= 3 {
            return Ok(resp);
        }
        let delay = std::time::Duration::from_millis(400 << (attempt - 1));
        log::warn!("google api {status}; retrying ({attempt}/2) after {delay:?}");
        std::thread::sleep(delay);
    }
}
pub fn export_doc_text(
    client: &reqwest::blocking::Client,
    access_token: &str,
    file_id: &str,
) -> AppResult<String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}/export?mimeType=text/plain",
        urlencoding::encode(file_id)
    );
    let resp = send_with_retry(|| client.get(url.clone()).bearer_auth(access_token))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(AppError::msg(format!("导出失败 ({status}): {text}")));
    }

    resp.text()
        .map_err(|e| AppError::msg(format!("读取导出正文失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_export_keeps_documents_and_reports_errors() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let source = GoogleDocsSource::new("google-test").with_errors(errors.clone());
        let documents = vec![DocumentRecord {
            id: "doc-1".into(),
            source_id: "google-test".into(),
            source_kind: SourceKind::GoogleDocs,
            external_id: "remote-1".into(),
            title: "Document".into(),
            uri: "https://docs.google.com/document/d/remote-1/edit".into(),
            body: "searchable text".into(),
            mtime: None,
            content_hash: "hash".into(),
        }];

        let result = source
            .finish_scan(documents, vec!["one document failed".into()])
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(errors.lock().unwrap().len(), 1);
    }
}

#[derive(Debug, Deserialize)]
pub struct DriveFileMeta {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    #[serde(rename = "modifiedTime")]
    pub modified_time: Option<String>,
    #[serde(rename = "webViewLink")]
    pub web_view_link: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

pub fn fetch_drive_file_meta(
    client: &reqwest::blocking::Client,
    access_token: &str,
    file_id: &str,
) -> AppResult<DriveFileMeta> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?fields=id,name,modifiedTime,webViewLink,mimeType",
        urlencoding::encode(file_id)
    );
    let resp = send_with_retry(|| {
        client
            .get(url.clone())
            .bearer_auth(access_token)
            .header("Accept", "application/json")
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(AppError::msg(format!(
            "获取文件元数据被拒绝 ({status}): {text}"
        )));
    }

    resp.json()
        .map_err(|e| AppError::msg(format!("解析文件元数据失败: {e}")))
}
