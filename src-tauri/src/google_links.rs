use crate::app_state::APP_DATA_DIR;
use crate::error::{AppError, AppResult};
use crate::file_store;
use crate::google_auth;
use crate::models::{DocumentRecord, SourceKind};
use crate::sources::google_docs::{export_doc_text, fetch_drive_file_meta};
use crate::sources::hash_text;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

static WATCHLIST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleWatchItem {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WatchlistFile {
    items: Vec<GoogleWatchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLinksReport {
    pub parsed: usize,
    pub added: usize,
    pub already_tracked: usize,
    pub invalid: Vec<String>,
    pub fetched: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub watchlist: Vec<GoogleWatchItem>,
}

pub fn watchlist_path() -> PathBuf {
    APP_DATA_DIR.join("google_watchlist.json")
}

pub fn load_watchlist() -> AppResult<Vec<GoogleWatchItem>> {
    let path = watchlist_path();
    Ok(file_store::read_json::<WatchlistFile>(&path)?
        .unwrap_or_default()
        .items)
}

pub fn save_watchlist(items: &[GoogleWatchItem]) -> AppResult<()> {
    std::fs::create_dir_all(APP_DATA_DIR.as_path())?;
    let file = WatchlistFile {
        items: items.to_vec(),
    };
    file_store::write_json(&watchlist_path(), &file)
}

pub fn parse_google_doc_ids(input: &str) -> (Vec<(String, String)>, Vec<String>) {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();
    let mut invalid = Vec::new();

    for raw in
        input.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|' | '\n' | '\r'))
    {
        let token = raw
            .trim()
            .trim_matches(|c| matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']'));
        if token.is_empty() {
            continue;
        }

        if let Some(id) = extract_doc_id(token) {
            if seen.insert(id.clone()) {
                let url = if token.starts_with("http://") || token.starts_with("https://") {
                    token.to_string()
                } else {
                    format!("https://docs.google.com/document/d/{id}/edit")
                };
                pairs.push((id, url));
            }
        } else if token.contains("docs.google")
            || token.contains("drive.google")
            || token.len() > 12
        {
            invalid.push(token.to_string());
        }
    }

    (pairs, invalid)
}

pub fn extract_doc_id(token: &str) -> Option<String> {
    if let Some(rest) = token
        .split("/document/d/")
        .nth(1)
        .or_else(|| token.split("/document/u/0/d/").nth(1))
        .or_else(|| token.split("/document/u/1/d/").nth(1))
    {
        let id = rest
            .split('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("");
        if is_plausible_id(id) {
            return Some(id.to_string());
        }
    }

    if let Some(idx) = token.find("id=") {
        let rest = &token[idx + 3..];
        let id = rest
            .split('&')
            .next()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("");
        if is_plausible_id(id) {
            return Some(id.to_string());
        }
    }

    if let Some(rest) = token.split("/file/d/").nth(1) {
        let id = rest
            .split('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("");
        if is_plausible_id(id) {
            return Some(id.to_string());
        }
    }

    if is_plausible_id(token) && !token.contains('/') && !token.contains('.') {
        return Some(token.to_string());
    }

    None
}

fn is_plausible_id(id: &str) -> bool {
    let id = id.trim();
    id.len() >= 20
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn extract_google_doc_links_from_text(text: &str) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    // Prefer scanning URL-like tokens first.
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(
                c,
                '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']' | '，' | '。' | '；' | '、'
            ) || c.is_whitespace()
        });
        if token.is_empty() {
            continue;
        }
        if !(token.contains("docs.google")
            || token.contains("drive.google")
            || token.contains("/document/d/")
            || token.contains("id="))
        {
            continue;
        }
        if let Some(id) = extract_doc_id(token) {
            if seen.insert(id.clone()) {
                let url = if token.starts_with("http") {
                    token.to_string()
                } else {
                    format!("https://docs.google.com/document/d/{id}/edit")
                };
                out.push((id, url));
            }
        }
    }

    // Also catch URLs stuck to CJK without spaces: ...看https://docs.google.com/...
    let mut rest = text;
    while let Some(idx) = rest.find("https://docs.google.com/") {
        let slice = &rest[idx..];
        let end = slice
            .find(|c: char| {
                c.is_whitespace() || matches!(c, ')' | ']' | '>' | '"' | '\'' | '，' | '。')
            })
            .unwrap_or(slice.len().min(200));
        let token = &slice[..end];
        if let Some(id) = extract_doc_id(token) {
            if seen.insert(id.clone()) {
                out.push((id, token.to_string()));
            }
        }
        rest = &slice[end.max(1)..];
    }

    out
}

pub fn add_links_to_watchlist(input: &str) -> AppResult<(Vec<String>, usize, usize, Vec<String>)> {
    let _guard = WATCHLIST_LOCK
        .lock()
        .map_err(|_| AppError::msg("watchlist lock poisoned"))?;
    let (pairs, invalid) = parse_google_doc_ids(input);
    if pairs.is_empty() {
        return Err(AppError::msg(if invalid.is_empty() {
            "没有识别到有效的 Google Docs 链接或 ID".into()
        } else {
            format!("无法解析链接：{}", invalid.join("；"))
        }));
    }

    let mut watchlist = load_watchlist()?;
    let existing: HashSet<String> = watchlist.iter().map(|i| i.id.clone()).collect();
    let mut added = 0usize;
    let mut already = 0usize;
    let now = chrono::Utc::now().to_rfc3339();
    let mut ids = Vec::new();

    for (id, url) in pairs {
        ids.push(id.clone());
        if existing.contains(&id) {
            already += 1;
        } else {
            watchlist.push(GoogleWatchItem {
                id,
                url,
                title: None,
                added_at: now.clone(),
            });
            added += 1;
        }
    }
    save_watchlist(&watchlist)?;
    Ok((ids, added, already, invalid))
}

pub fn fetch_documents_by_ids(
    source_id: &str,
    ids: &[String],
    cancel: Option<&AtomicBool>,
) -> AppResult<(Vec<DocumentRecord>, Vec<String>)> {
    if cancel
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
    {
        return Err(AppError::msg("已取消同步"));
    }
    let status = google_auth::auth_status()?;
    if !status.connected {
        return Err(AppError::msg(
            "尚未连接 Google。请先在「来源」完成 OAuth 连接。",
        ));
    }

    let access_token = google_auth::get_valid_access_token()?;
    // Bounded per-request budget keeps a whole batch from hanging a search.
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| AppError::msg(format!("初始化 HTTP 客户端失败: {e}")))?;
    let mut documents = Vec::new();
    let mut errors = Vec::new();

    for id in ids {
        if cancel
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err(AppError::msg("已取消同步"));
        }
        match fetch_one(&client, &access_token, source_id, id) {
            Ok(doc) => documents.push(doc),
            Err(err) => errors.push(format!("{id}: {err}")),
        }
        thread::sleep(Duration::from_millis(60));
    }

    if cancel
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
    {
        return Err(AppError::msg("已取消同步"));
    }

    Ok((documents, errors))
}

fn fetch_one(
    client: &reqwest::blocking::Client,
    access_token: &str,
    source_id: &str,
    file_id: &str,
) -> AppResult<DocumentRecord> {
    let meta = fetch_drive_file_meta(client, access_token, file_id)?;
    if let Some(mime) = &meta.mime_type {
        if mime != "application/vnd.google-apps.document" {
            return Err(AppError::msg(format!(
                "暂只支持 Google Docs，当前类型为 {mime}"
            )));
        }
    }

    let body = export_doc_text(client, access_token, file_id)?;
    let uri = meta
        .web_view_link
        .unwrap_or_else(|| format!("https://docs.google.com/document/d/{file_id}/edit"));
    Ok(DocumentRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: source_id.to_string(),
        source_kind: SourceKind::GoogleDocs,
        external_id: file_id.to_string(),
        title: meta.name,
        uri,
        body: body.clone(),
        mtime: meta.modified_time,
        content_hash: hash_text(&body),
    })
}

pub fn remove_watch_ids(ids: &[String]) -> AppResult<Vec<GoogleWatchItem>> {
    let _guard = WATCHLIST_LOCK
        .lock()
        .map_err(|_| AppError::msg("watchlist lock poisoned"))?;
    let set: HashSet<_> = ids.iter().cloned().collect();
    let mut list = load_watchlist()?;
    list.retain(|i| !set.contains(&i.id));
    save_watchlist(&list)?;
    Ok(list)
}

pub fn update_watch_titles(titles: &[(String, String)]) -> AppResult<Vec<GoogleWatchItem>> {
    let _guard = WATCHLIST_LOCK
        .lock()
        .map_err(|_| AppError::msg("watchlist lock poisoned"))?;
    let mut list = load_watchlist()?;
    for item in &mut list {
        if let Some((_, title)) = titles.iter().find(|(id, _)| id == &item.id) {
            item.title = Some(title.clone());
        }
    }
    save_watchlist(&list)?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn parses_common_docs_urls() {
        let input = r#"
        https://docs.google.com/document/d/1abcDEF-ghij_KLMNOpqrsTUVWxyz1234567/edit
        https://docs.google.com/document/d/1abcDEF-ghij_KLMNOpqrsTUVWxyz1234567/edit?usp=sharing
        https://drive.google.com/open?id=1abcDEV-ghij_KLMNOpqrsTUVWxyz9999999
        1OnlyBareIdShouldBeLongEnough12345
        not-a-link
        "#;
        let (pairs, _invalid) = parse_google_doc_ids(input);
        assert!(pairs.len() >= 2);
        assert!(pairs.iter().any(|(id, _)| id.starts_with("1abcDEF")));
    }

    #[test]
    fn extracts_links_from_body_text() {
        let body = "参见https://docs.google.com/document/d/1abcDEF-ghij_KLMNOpqrsTUVWxyz1234567/edit 以及说明。";
        let links = extract_google_doc_links_from_text(body);
        assert_eq!(links.len(), 1);
        assert!(links[0]
            .0
            .starts_with("1abcDEF-ghij_KLMNOpqrsTUVWxyz1234567"));
    }

    #[test]
    fn cancellation_is_checked_before_google_auth() {
        let cancel = AtomicBool::new(true);
        let err = fetch_documents_by_ids("google-test", &[], Some(&cancel)).unwrap_err();
        assert!(err.to_string().contains("已取消"));
    }
}
