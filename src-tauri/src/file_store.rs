use crate::error::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "searchdoc-data".into());
    path.with_file_name(format!("{name}.{suffix}"))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<Option<T>> {
    let backup = sibling(path, "bak");
    if !path.exists() && !backup.exists() {
        return Ok(None);
    }

    match std::fs::read_to_string(path)
        .map_err(AppError::from)
        .and_then(|raw| serde_json::from_str(&raw).map_err(AppError::from))
    {
        Ok(value) => Ok(Some(value)),
        Err(primary_error) if backup.exists() => {
            let raw = std::fs::read_to_string(&backup)?;
            let value = serde_json::from_str(&raw)?;
            eprintln!(
                "recovered {} from {} after: {primary_error}",
                path.display(),
                backup.display()
            );
            let _ = std::fs::copy(&backup, path);
            Ok(Some(value))
        }
        Err(error) => Err(error),
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = sibling(path, "tmp");
    let backup = sibling(path, "bak");
    {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
        file.sync_all()?;
    }

    if backup.exists() {
        std::fs::remove_file(&backup)?;
    }
    if path.exists() {
        std::fs::rename(path, &backup)?;
    }
    if let Err(error) = std::fs::rename(&temp, path) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    Ok(())
}

pub fn remove_backup(path: &Path) -> AppResult<()> {
    let backup = sibling(path, "bak");
    if backup.exists() {
        std::fs::remove_file(backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_json, remove_backup, sibling, write_json};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Value {
        name: String,
    }

    #[test]
    fn corrupt_primary_recovers_previous_json() {
        let dir = std::env::temp_dir().join(format!("searchdoc-json-{}", uuid::Uuid::new_v4()));
        let path = dir.join("prefs.json");
        write_json(&path, &Value { name: "old".into() }).unwrap();
        write_json(&path, &Value { name: "new".into() }).unwrap();
        std::fs::write(&path, "broken").unwrap();

        let recovered: Value = read_json(&path).unwrap().unwrap();
        assert_eq!(recovered.name, "old");

        remove_backup(&path).unwrap();
        assert!(!sibling(&path, "bak").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
