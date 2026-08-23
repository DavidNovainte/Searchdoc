//! Lightweight release check against GitHub Releases. No signing or
//! auto-install: the app just tells the user a newer version exists and
//! opens the release page for a manual download.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::time::Duration;

/// GitHub "owner/repo" polled for releases.
pub const UPDATE_REPO: &str = "DavidNovainte/Searchdoc";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub notes: Option<String>,
    /// True when UPDATE_REPO is not configured yet.
    pub disabled: bool,
}

/// True when `latest` is strictly newer than `current` (numeric semver
/// triples; pre-release suffixes are stripped before comparing).
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn check_for_update() -> AppResult<UpdateInfo> {
    let current = current_version();
    if UPDATE_REPO.is_empty() {
        return Ok(UpdateInfo {
            current,
            latest: None,
            update_available: false,
            release_url: None,
            notes: None,
            disabled: true,
        });
    }

    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("searchdoc-update-check")
        .build()
        .map_err(|e| AppError::msg(format!("http client: {e}")))?;
    let resp = client.get(url).send().map_err(|e| {
        AppError::msg(format!(
            "\u{68c0}\u{67e5}\u{66f4}\u{65b0}\u{5931}\u{8d25}: {e}"
        ))
    })?;
    let status = resp.status();
    if status.as_u16() == 404 {
        // Repo has no releases yet.
        return Ok(UpdateInfo {
            current,
            latest: None,
            update_available: false,
            release_url: None,
            notes: None,
            disabled: false,
        });
    }
    let value: serde_json::Value = resp.json().map_err(|e| {
        AppError::msg(format!(
            "\u{66f4}\u{65b0}\u{4fe1}\u{606f}\u{89e3}\u{6790}\u{5931}\u{8d25} ({status}): {e}"
        ))
    })?;
    if !status.is_success() {
        return Err(AppError::msg(format!(
            "\u{68c0}\u{67e5}\u{66f4}\u{65b0}\u{88ab}\u{62d2}\u{7edd} ({status})"
        )));
    }

    let tag = value
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let latest = tag.trim_start_matches('v').to_string();
    let update_available = is_newer_version(&current, &latest);
    Ok(UpdateInfo {
        current,
        latest: if latest.is_empty() {
            None
        } else {
            Some(latest)
        },
        update_available,
        release_url: value
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        notes: value
            .get("body")
            .and_then(serde_json::Value::as_str)
            .map(|notes| notes.chars().take(500).collect()),
        disabled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_is_numeric_and_v_tolerant() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.9.9", "1.0.0"));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(!is_newer_version("0.2.0", "0.1.0"));
        assert!(!is_newer_version("0.2.0", "v0.2.0"));
        assert!(!is_newer_version("0.2.0", "garbage"));
        assert!(is_newer_version("1.0.0-beta", "1.0.1"));
    }
}
