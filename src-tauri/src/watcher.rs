//! Filesystem watcher: keeps local source indexes fresh within seconds.
//!
//! Every enabled local source root is watched recursively. Bursts of change
//! events are debounced by a quiet period, then mapped back to their source
//! and synced incrementally - no more waiting for the five-minute sweep.

use crate::app_state::{AppState, APP_DATA_DIR};
use crate::models::SourceKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Quiet time after the last event before an incremental sync fires.
const QUIET_PERIOD: Duration = Duration::from_secs(2);
const POLL_TICK: Duration = Duration::from_millis(250);

/// Spawn the watcher thread. If watching cannot start on this platform the
/// thread simply exits; the periodic five-minute sweep still covers freshness.
pub fn spawn(state: AppState) {
    std::thread::spawn(move || {
        if let Err(err) = run(state) {
            log::error!("file watcher stopped: {err}");
        }
    });
}

fn run(state: AppState) -> Result<(), notify::Error> {
    let (tx, rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(tx)?;
    // normalized root path -> source id
    let mut watches: HashMap<String, String> = HashMap::new();
    let mut dirty: HashSet<String> = HashSet::new();
    let mut last_change: Option<Instant> = None;
    let mut need_rebuild = true;
    let data_dir_guard = strip_extended(&APP_DATA_DIR.to_string_lossy()).to_lowercase();

    loop {
        match rx.recv_timeout(POLL_TICK) {
            Ok(Ok(event)) => {
                // Reads fire access noise constantly while scanning; ignore it.
                if matches!(event.kind, EventKind::Access(_)) {
                    continue;
                }
                let mut hit = false;
                for path in &event.paths {
                    let target = strip_extended(&path.to_string_lossy());
                    if target.to_lowercase().starts_with(&data_dir_guard) {
                        continue; // never react to our own index database churn
                    }
                    if let Some(id) = map_event_path(&watches, path) {
                        dirty.insert(id);
                        hit = true;
                    }
                }
                if hit {
                    last_change = Some(Instant::now());
                }
            }
            Ok(Err(err)) => log::warn!("watcher event error: {err}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if need_rebuild && !state.is_sync_running() {
            need_rebuild = false;
            rebuild_watches(&state, &mut watcher, &mut watches);
        }
        if state.take_watch_refresh() {
            need_rebuild = true;
        }

        let quiet = last_change.is_some_and(|at| at.elapsed() >= QUIET_PERIOD);
        if quiet && !dirty.is_empty() && !state.is_sync_running() {
            let batch: Vec<String> = dirty.drain().collect();
            last_change = None;
            for source_id in batch {
                if state.is_sync_running() {
                    dirty.insert(source_id); // user/tray sync won; retry later
                    continue;
                }
                match state.sync_source(source_id.clone()) {
                    Ok(_) => {}
                    Err(err) => {
                        let msg = err.to_string();
                        if msg.contains("已有同步在运行") {
                            dirty.insert(source_id);
                        } else {
                            log::error!("watcher auto-sync failed ({source_id}): {msg}");
                            need_rebuild = true; // root may have vanished
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn rebuild_watches(
    state: &AppState,
    watcher: &mut RecommendedWatcher,
    watches: &mut HashMap<String, String>,
) {
    for old in watches.keys() {
        let _ = watcher.unwatch(Path::new(old));
    }
    watches.clear();
    let Ok(sources) = state.list_sources() else {
        return;
    };
    for src in sources {
        if src.kind != SourceKind::Local || !src.enabled {
            continue;
        }
        let Some(raw_root) = src.root_path.clone() else {
            continue;
        };
        let root = normalize_root(Path::new(&raw_root));
        match watcher.watch(Path::new(&root), RecursiveMode::Recursive) {
            Ok(()) => {
                watches.insert(root, src.id);
            }
            Err(err) => log::warn!("watcher: cannot watch {raw_root}: {err}"),
        }
    }
    log::info!("file watcher watching {} local root(s)", watches.len());
}

fn map_event_path(watches: &HashMap<String, String>, raw: &Path) -> Option<String> {
    let target = strip_extended(&raw.to_string_lossy());
    let lower = target.to_lowercase();
    watches
        .iter()
        .filter(|(root, _)| {
            let r = root.to_lowercase();
            lower == *r || lower.starts_with(&format!("{r}\\"))
        })
        .max_by_key(|(root, _)| root.split("\\").count())
        .map(|(_, id)| id.clone())
}

fn normalize_root(path: &Path) -> String {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    strip_extended(&canon.to_string_lossy())
}

/// Drop Windows extended-length prefixes so canonicalized roots and raw
/// event paths are directly comparable.
fn strip_extended(s: &str) -> String {
    let stripped = s
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| s.strip_prefix(r"\\?\").map(str::to_string))
        .unwrap_or_else(|| s.to_string());
    stripped.replace('/', "\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn watches(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(root, id)| (root.to_string(), id.to_string()))
            .collect()
    }

    #[test]
    fn strips_extended_length_prefixes_and_normalizes_separators() {
        assert_eq!(strip_extended(r"\\?\C:\notes\a.md"), r"C:\notes\a.md");
        assert_eq!(
            strip_extended(r"\\?\UNC\server\share\x.txt"),
            r"\\server\share\x.txt"
        );
        assert_eq!(
            strip_extended("C:/mixed/separators"),
            r"C:\mixed\separators"
        );
        assert_eq!(strip_extended(r"C:\plain\path"), r"C:\plain\path");
    }

    #[test]
    fn maps_event_paths_to_the_longest_matching_root() {
        let table = watches(&[(r"C:\data", "src-root"), (r"C:\data\work", "src-work")]);
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"C:\data\work\file.md")),
            Some("src-work".to_string())
        );
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"C:\data\other.md")),
            Some("src-root".to_string())
        );
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"D:\elsewhere.md")),
            None
        );
    }

    #[test]
    fn root_itself_and_case_insensitive_events_map_correctly() {
        let table = watches(&[(r"C:\Data", "src")]);
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"C:\data")),
            Some("src".to_string())
        );
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"c:\DATA\notes\a.md")),
            Some("src".to_string())
        );
    }

    #[test]
    fn similarly_named_roots_do_not_cross_match() {
        let table = watches(&[(r"C:\data", "src")]);
        assert_eq!(
            map_event_path(&table, &PathBuf::from(r"C:\database\f.md")),
            None
        );
    }
}
