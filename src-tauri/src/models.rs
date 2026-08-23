use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Local,
    GoogleDocs,
    Notion,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::GoogleDocs => "google_docs",
            Self::Notion => "notion",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "local" => Some(Self::Local),
            "google_docs" => Some(Self::GoogleDocs),
            "notion" => Some(Self::Notion),
            _ => None,
        }
    }
}

/// A normalized document written into the shared index by a source connector.
#[derive(Debug, Clone)]
pub struct DocumentRecord {
    pub id: String,
    pub source_id: String,
    pub source_kind: SourceKind,
    pub external_id: String,
    pub title: String,
    pub uri: String,
    pub body: String,
    pub mtime: Option<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub id: String,
    pub kind: SourceKind,
    pub name: String,
    pub root_path: Option<String>,
    pub enabled: bool,
    pub last_sync_at: Option<String>,
    pub doc_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub source_id: String,
    pub source_kind: SourceKind,
    pub title: String,
    pub uri: String,
    pub snippet: String,
    pub rank: f64,
    pub mtime: Option<String>,
    /// 0 = direct hit, >=1 found via outbound link chain
    #[serde(default)]
    pub depth: u32,
    /// Parent document title when depth > 0
    #[serde(default)]
    pub via: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeepSearchStats {
    pub enabled: bool,
    pub max_depth: u32,
    pub seeds: usize,
    pub links_found: usize,
    pub links_followed: usize,
    pub fetched: usize,
    pub linked_hits: usize,
    pub skipped_existing: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub deep: DeepSearchStats,
}

/// All user-supplied search filters, passed to the backend as one invoked object.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query: String,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub source_kind: Option<String>,
    pub mode: Option<String>,
    pub scope: Option<String>,
    pub sort: Option<String>,
    pub deep: Option<bool>,
    pub deep_depth: Option<u32>,
    pub deep_link_limit: Option<usize>,
    pub deep_fetch_missing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub document_count: i64,
    pub source_count: i64,
    pub local_doc_count: i64,
    pub google_doc_count: i64,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub source_id: String,
    pub indexed: usize,
    pub removed: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncErrorKind {
    AccessDenied,
    Network,
    Parse,
    MissingSource,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub running: bool,
    pub phase: String,
    pub source_id: Option<String>,
    pub source_name: Option<String>,
    pub visited: usize,
    pub scanned: usize,
    pub processed: usize,
    pub indexed: usize,
    pub removed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub error_kind: Option<SyncErrorKind>,
    pub message: Option<String>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            running: false,
            phase: "idle".into(),
            source_id: None,
            source_name: None,
            visited: 0,
            scanned: 0,
            processed: 0,
            indexed: 0,
            removed: 0,
            skipped: 0,
            errors: 0,
            error_kind: None,
            message: None,
        }
    }
}

/// A selectable local root (drive letter on Windows, mount path elsewhere).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDriveInfo {
    pub path: String,
    pub label: String,
}
