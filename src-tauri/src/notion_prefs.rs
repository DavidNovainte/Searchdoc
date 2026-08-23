use crate::app_state::APP_DATA_DIR;
use crate::error::{AppError, AppResult};
use std::fs;

const LEGACY_FILE_NAME: &str = "notion.json";
const KEYRING_SERVICE: &str = "com.searchdoc.notion";
const KEYRING_KEY: &str = "notion-token";

fn keyring_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_KEY)
        .map_err(|err| AppError::msg(format!("无法初始化系统凭据存储：{err}")))
}

/// Read the legacy plaintext token (pre-keyring installs) if present.
fn read_legacy_token() -> Option<String> {
    fs::read_to_string(APP_DATA_DIR.join(LEGACY_FILE_NAME))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("token")?.as_str().map(str::to_string))
        .filter(|t| !t.trim().is_empty())
}

fn remove_legacy_file() {
    let _ = fs::remove_file(APP_DATA_DIR.join(LEGACY_FILE_NAME));
}

/// Load the saved Notion integration token. Tokens live in the OS keyring;
/// a legacy plaintext file is migrated automatically on first read.
pub fn load_token() -> AppResult<Option<String>> {
    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(value) => {
            remove_legacy_file();
            if value.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
        Err(keyring::Error::NoEntry) => {
            if let Some(token) = read_legacy_token() {
                entry
                    .set_password(&token)
                    .map_err(|err| AppError::msg(format!("迁移 Notion 凭据失败：{err}")))?;
                remove_legacy_file();
                Ok(Some(token))
            } else {
                Ok(None)
            }
        }
        Err(err) => Err(AppError::msg(format!("无法读取系统凭据：{err}"))),
    }
}

/// Persist the Notion integration token into the OS keyring.
pub fn save_token(token: &str) -> AppResult<()> {
    keyring_entry()?
        .set_password(token)
        .map_err(|err| AppError::msg(format!("无法保存系统凭据：{err}")))?;
    remove_legacy_file();
    Ok(())
}
