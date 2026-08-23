use crate::app_state::APP_DATA_DIR;
use crate::error::{AppError, AppResult};
use crate::file_store;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static PREFS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoogleSyncMode {
    /// Scan recent Google Docs (shallow bulk sync).
    Recent,
    /// Only sync documents in the watchlist (pasted links).
    #[default]
    WatchlistOnly,
}

impl GoogleSyncMode {
    pub fn parse(value: &str) -> Self {
        match value {
            "recent" | "all" | "full" => Self::Recent,
            _ => Self::WatchlistOnly,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "最近文档浅同步",
            Self::WatchlistOnly => "仅观察列表",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePrefs {
    pub sync_mode: GoogleSyncMode,
    /// When non-empty and mode is Recent, only list Docs that are direct children
    /// of these Drive folders (OR). Empty = no folder filter (Drive-wide recent).
    #[serde(default)]
    pub folder_ids: Vec<String>,
}

impl Default for GooglePrefs {
    fn default() -> Self {
        Self {
            sync_mode: GoogleSyncMode::WatchlistOnly,
            folder_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleFolderItem {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePrefsStatus {
    pub sync_mode: GoogleSyncMode,
    pub sync_mode_label: String,
    pub folder_ids: Vec<String>,
    pub folders: Vec<GoogleFolderItem>,
    pub prefs_path: String,
}

fn prefs_path() -> PathBuf {
    APP_DATA_DIR.join("google_prefs.json")
}

pub fn load_prefs() -> AppResult<GooglePrefs> {
    let path = prefs_path();
    if !path.exists() {
        return Ok(GooglePrefs::default());
    }
    let mut prefs = file_store::read_json::<GooglePrefs>(&path)?.unwrap_or_default();
    prefs.folder_ids = normalize_folder_ids(prefs.folder_ids);
    Ok(prefs)
}

pub fn save_prefs(prefs: &GooglePrefs) -> AppResult<()> {
    std::fs::create_dir_all(APP_DATA_DIR.as_path())?;
    file_store::write_json(&prefs_path(), prefs)
}

fn normalize_folder_ids(ids: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        let id = id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        if !out.iter().any(|x: &String| x == &id) {
            out.push(id);
        }
    }
    out
}

pub fn get_status() -> AppResult<GooglePrefsStatus> {
    let prefs = load_prefs()?;
    let folders = prefs
        .folder_ids
        .iter()
        .map(|id| GoogleFolderItem {
            id: id.clone(),
            name: None,
        })
        .collect();
    Ok(GooglePrefsStatus {
        sync_mode: prefs.sync_mode,
        sync_mode_label: prefs.sync_mode.label().to_string(),
        folder_ids: prefs.folder_ids.clone(),
        folders,
        prefs_path: prefs_path().to_string_lossy().to_string(),
    })
}

pub fn set_sync_mode(mode: &str) -> AppResult<GooglePrefsStatus> {
    let _guard = PREFS_LOCK
        .lock()
        .map_err(|_| AppError::msg("google prefs lock poisoned"))?;
    let mut prefs = load_prefs()?;
    prefs.sync_mode = GoogleSyncMode::parse(mode);
    save_prefs(&prefs)?;
    get_status()
}

pub fn set_folder_ids(folder_ids: Vec<String>) -> AppResult<GooglePrefsStatus> {
    let _guard = PREFS_LOCK
        .lock()
        .map_err(|_| AppError::msg("google prefs lock poisoned"))?;
    let mut prefs = load_prefs()?;
    prefs.folder_ids = normalize_folder_ids(folder_ids);
    save_prefs(&prefs)?;
    get_status()
}

pub fn current_mode() -> AppResult<GoogleSyncMode> {
    Ok(load_prefs()?.sync_mode)
}

pub fn current_folder_ids() -> AppResult<Vec<String>> {
    Ok(load_prefs()?.folder_ids)
}

pub fn ensure_valid_mode(mode: &str) -> AppResult<GoogleSyncMode> {
    match mode {
        "recent" | "watchlist_only" => Ok(GoogleSyncMode::parse(mode)),
        _ => Err(AppError::msg(
            "无效的 Google 同步模式，请使用 recent 或 watchlist_only",
        )),
    }
}

/// Parse Drive folder ids from free-form text (URLs or bare ids).
pub fn parse_folder_ids(input: &str) -> (Vec<String>, Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
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
        if let Some(id) = extract_folder_id(token) {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        } else if token.contains("drive.google") || token.contains("folders/") || token.len() > 12 {
            invalid.push(token.to_string());
        }
    }

    (ids, invalid)
}

fn extract_folder_id(token: &str) -> Option<String> {
    if let Some(rest) = token.split("/folders/").nth(1) {
        let id = rest
            .split('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("");
        if is_plausible_drive_id(id) {
            return Some(id.to_string());
        }
    }

    // drive.google.com/open?id=...
    if let Some(idx) = token.find("id=") {
        let rest = &token[idx + 3..];
        let id = rest
            .split('&')
            .next()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("");
        if is_plausible_drive_id(id) {
            return Some(id.to_string());
        }
    }

    if is_plausible_drive_id(token) && !token.contains('/') && !token.contains('.') {
        return Some(token.to_string());
    }

    None
}

fn is_plausible_drive_id(id: &str) -> bool {
    let len = id.len();
    (10..=128).contains(&len)
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
