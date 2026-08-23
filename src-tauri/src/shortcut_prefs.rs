use crate::app_state::APP_DATA_DIR;
use crate::error::AppResult;
use std::fs;

/// Default global hotkey string (parsed by tauri-plugin-global-shortcut).
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+Space";

const FILE_NAME: &str = "shortcut.json";

/// Load the persisted accelerator, falling back to the default when the
/// config is missing or malformed.
pub fn load() -> String {
    fs::read_to_string(APP_DATA_DIR.join(FILE_NAME))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("shortcut")?.as_str().map(str::to_string))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string())
}

/// Persist the accelerator for the next launch.
pub fn save(shortcut: &str) -> AppResult<()> {
    let json = serde_json::json!({ "shortcut": shortcut });
    fs::write(
        APP_DATA_DIR.join(FILE_NAME),
        serde_json::to_string_pretty(&json)?,
    )?;
    Ok(())
}
