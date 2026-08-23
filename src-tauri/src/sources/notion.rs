//! Notion database connector: queries a database, flattens page blocks into
//! searchable plain text.

use crate::error::{AppError, AppResult};
use crate::models::{DocumentRecord, SourceKind};
use crate::sources::hash_text;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";
/// Notion allows roughly 3 requests per second.
const THROTTLE: Duration = Duration::from_millis(350);
/// Bound nested-content fetching (toggle/callout children).
const MAX_NEST_DEPTH: usize = 1;

pub struct NotionSource {
    source_id: String,
    token: String,
    database_id: String,
    client: reqwest::blocking::Client,
    errors: Option<Arc<Mutex<Vec<String>>>>,
    cancel: Option<Arc<AtomicBool>>,
}

impl NotionSource {
    pub fn new(source_id: String, token: String, database_id: String) -> Self {
        Self {
            source_id,
            token,
            database_id,
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("notion http client"),
            errors: None,
            cancel: None,
        }
    }

    pub fn with_errors(mut self, errors: Arc<Mutex<Vec<String>>>) -> Self {
        self.errors = Some(errors);
        self
    }

    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
    }

    fn record_error(&self, message: String) {
        if let Some(errors) = &self.errors {
            if let Ok(mut errors) = errors.lock() {
                errors.push(message);
            }
        }
    }

    /// One throttled JSON API call with retry on 429 / 5xx. Honors
    /// Retry-After when Notion sends it, otherwise backs off exponentially.
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        json_body: Option<&Value>,
    ) -> AppResult<Value> {
        std::thread::sleep(THROTTLE);
        let url = format!("{API_BASE}/{path}");
        let mut attempt = 0;
        loop {
            if self.is_cancelled() {
                return Err(AppError::msg("\u{5df2}\u{53d6}\u{6d88}\u{540c}\u{6b65}"));
            }
            attempt += 1;
            let mut req = self
                .client
                .request(method.clone(), &url)
                .bearer_auth(&self.token)
                .header("Notion-Version", NOTION_VERSION);
            if let Some(body) = json_body {
                req = req.json(body);
            }
            let resp = req.send().map_err(|e| {
                AppError::msg(format!("Notion \u{8bf7}\u{6c42}\u{5931}\u{8d25}: {e}"))
            })?;
            let status = resp.status();
            if status.as_u16() == 429 || status.is_server_error() {
                if attempt >= 3 {
                    return Err(AppError::msg(format!(
                        "Notion \u{63a5}\u{53e3}\u{6301}\u{7eed}\u{5931}\u{8d25} ({status})"
                    )));
                }
                let wait = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_millis(600 << (attempt - 1)));
                log::warn!("notion api {status}; retrying ({attempt}/2) after {wait:?}");
                std::thread::sleep(wait);
                continue;
            }
            let value: Value = resp.json().map_err(|e| {
                AppError::msg(format!(
                    "Notion \u{54cd}\u{5e94}\u{89e3}\u{6790}\u{5931}\u{8d25} ({status}): {e}"
                ))
            })?;
            if !status.is_success() {
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                return Err(AppError::msg(format!(
                    "Notion \u{62d2}\u{7edd} ({status}): {message}"
                )));
            }
            return Ok(value);
        }
    }

    /// Fetch all children blocks of a page or block (paginated).
    fn fetch_children(&self, block_id: &str) -> AppResult<Vec<Value>> {
        let mut blocks = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut path = format!(
                "blocks/{}/children?page_size=100",
                urlencoding::encode(block_id)
            );
            if let Some(c) = &cursor {
                path.push_str(&format!("&start_cursor={}", urlencoding::encode(c)));
            }
            let resp = self.request(reqwest::Method::GET, &path, None)?;
            if let Some(results) = resp.get("results").and_then(Value::as_array) {
                blocks.extend(results.iter().cloned());
            }
            let has_more = resp
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            cursor = resp
                .get("next_cursor")
                .and_then(Value::as_str)
                .filter(|_| has_more)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(blocks)
    }

    /// Flatten one block into text, recursing into container children once.
    fn collect_block_text(&self, block: &Value, depth: usize, out: &mut Vec<String>) {
        let line = block_inline_text(block);
        if !line.is_empty() {
            out.push(line);
        }
        let has_children = block
            .get("has_children")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
        if has_children
            && depth < MAX_NEST_DEPTH
            && kind != "child_database"
            && kind != "child_page"
        {
            if let Some(id) = block.get("id").and_then(Value::as_str) {
                match self.fetch_children(id) {
                    Ok(children) => {
                        for child in &children {
                            self.collect_block_text(child, depth + 1, out);
                        }
                    }
                    Err(err) => {
                        self.record_error(format!("\u{5d4c}\u{5957}\u{5185}\u{5bb9}\u{83b7}\u{53d6}\u{5931}\u{8d25}: {err}"));
                    }
                }
            }
        }
    }

    fn fetch_page_text(&self, page_id: &str) -> AppResult<String> {
        let blocks = self.fetch_children(page_id)?;
        let mut lines = Vec::new();
        for block in &blocks {
            self.collect_block_text(block, 0, &mut lines);
        }
        Ok(lines.join("\n"))
    }
}

impl super::SourceConnector for NotionSource {
    fn scan(&self) -> AppResult<Vec<DocumentRecord>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            if self.is_cancelled() {
                return Err(AppError::msg("\u{5df2}\u{53d6}\u{6d88}\u{540c}\u{6b65}"));
            }
            let mut body = json!({ "page_size": 100 });
            if let Some(c) = &cursor {
                body["start_cursor"] = json!(c);
            }
            let path = format!("databases/{}/query", urlencoding::encode(&self.database_id));
            let resp = self.request(reqwest::Method::POST, &path, Some(&body))?;

            let results = resp
                .get("results")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for page in &results {
                let page_id = page
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if page_id.is_empty() {
                    continue;
                }
                let title = title_from_properties(page.get("properties").unwrap_or(&Value::Null));
                let uri = page
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let mtime = page
                    .get("last_edited_time")
                    .and_then(Value::as_str)
                    .map(str::to_string);

                let body_text = match self.fetch_page_text(&page_id) {
                    Ok(text) => text,
                    Err(err) => {
                        self.record_error(format!("{title}: {err}"));
                        continue;
                    }
                };

                out.push(DocumentRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: self.source_id.clone(),
                    source_kind: SourceKind::Notion,
                    external_id: page_id,
                    title,
                    uri,
                    body: body_text.clone(),
                    mtime,
                    content_hash: hash_text(&body_text),
                });
            }

            let has_more = resp
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            cursor = resp
                .get("next_cursor")
                .and_then(Value::as_str)
                .filter(|_| has_more)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }
}

/// Best-effort database display title; also validates token + id on add.
pub fn fetch_database_title(token: &str, database_id: &str) -> AppResult<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::msg(format!("http client: {e}")))?;
    let resp = client
        .get(format!(
            "{API_BASE}/databases/{}",
            urlencoding::encode(database_id)
        ))
        .bearer_auth(token)
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .map_err(|e| AppError::msg(format!("Notion \u{8fde}\u{63a5}\u{5931}\u{8d25}: {e}")))?;
    let status = resp.status();
    let value: Value = resp.json().map_err(|e| {
        AppError::msg(format!(
            "Notion \u{54cd}\u{5e94}\u{89e3}\u{6790}\u{5931}\u{8d25}: {e}"
        ))
    })?;
    if !status.is_success() {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(AppError::msg(format!(
            "Notion \u{9a8c}\u{8bc1}\u{5931}\u{8d25} ({status}): {message}"
        )));
    }
    let title = value
        .get("title")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("plain_text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| format!("Notion {database_id}"));
    Ok(title)
}

/// Join a rich_text array into plain text.
fn rich_text_to_plain(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("plain_text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Inline text of a single block ("paragraph", headings, lists, code, ...).
fn block_inline_text(block: &Value) -> String {
    let kind = match block.get("type").and_then(Value::as_str) {
        Some(kind) => kind,
        None => return String::new(),
    };
    if matches!(kind, "child_database" | "child_page" | "unsupported") {
        return String::new();
    }
    let body = match block.get(kind) {
        Some(body) => body,
        None => return String::new(),
    };
    rich_text_to_plain(body.get("rich_text"))
}

/// Find the title property among a page's properties and flatten it.
pub(crate) fn title_from_properties(properties: &Value) -> String {
    if let Some(props) = properties.as_object() {
        for (_name, spec) in props {
            if spec.get("type").and_then(Value::as_str) == Some("title") {
                let title = rich_text_to_plain(spec.get("title"));
                if !title.trim().is_empty() {
                    return title;
                }
            }
        }
    }
    "\u{672a}\u{547d}\u{540d}".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_common_block_types() {
        let mk = |kind: &str, text: &str| {
            json!({
                "type": kind,
                kind: { "rich_text": [ { "plain_text": text } ] },
                "has_children": false,
            })
        };
        let blocks = [
            mk("paragraph", "\u{7b2c}\u{4e00}\u{6bb5}"),
            mk("heading_1", "\u{6807}\u{9898}"),
            mk("bulleted_list_item", "\u{5217}\u{8868}\u{9879}"),
            mk("to_do", "\u{5f85}\u{529e}"),
            mk("code", "let x = 1;"),
            mk("quote", "\u{5f15}\u{7528}"),
        ];
        let text = blocks
            .iter()
            .map(block_inline_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(text, "\u{7b2c}\u{4e00}\u{6bb5}\n\u{6807}\u{9898}\n\u{5217}\u{8868}\u{9879}\n\u{5f85}\u{529e}\nlet x = 1;\n\u{5f15}\u{7528}");
    }

    #[test]
    fn skips_child_pages_and_unsupported_blocks() {
        let blocks = [
            json!({ "type": "child_page", "child_page": { "title": "x" }, "has_children": true }),
            json!({ "type": "unsupported", "unsupported": {} }),
            json!({ "type": "paragraph", "paragraph": { "rich_text": [] }, "has_children": false }),
        ];
        let text = blocks
            .iter()
            .map(block_inline_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(text, "\n\n");
    }

    #[test]
    fn finds_title_property_among_mixed_properties() {
        let props = json!({
            "Status": { "type": "select", "select": { "name": "Done" } },
            "Name": { "type": "title", "title": [
                { "plain_text": "Road" },
                { "plain_text": "map" }
            ] },
            "Tags": { "type": "multi_select", "multi_select": [] }
        });
        assert_eq!(title_from_properties(&props), "Roadmap");
        assert_eq!(
            title_from_properties(&json!({})),
            "\u{672a}\u{547d}\u{540d}"
        );
    }
}
