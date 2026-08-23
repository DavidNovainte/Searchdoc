mod app_state;
mod db;
mod error;
mod file_store;
mod google_auth;
mod google_links;
mod google_prefs;
mod models;
mod notion_prefs;
mod shortcut_prefs;
mod sources;
mod update;
mod watcher;

use app_state::AppState;
use error::AppResult;
use models::{IndexStats, SearchResponse, SourceInfo, SyncReport};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[tauri::command(async)]
async fn search_documents(
    state: tauri::State<'_, AppState>,
    query: models::SearchQuery,
) -> AppResult<SearchResponse> {
    // Deep search performs blocking Google fetches; keep them off the async
    // runtime workers so unrelated commands never stall behind them.
    let app_state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app_state.search(query))
        .await
        .map_err(|err| error::AppError::msg(format!("搜索任务异常退出: {err}")))?
}

#[tauri::command]
fn list_sources(state: tauri::State<'_, AppState>) -> AppResult<Vec<SourceInfo>> {
    state.list_sources()
}

#[tauri::command]
fn get_stats(state: tauri::State<'_, AppState>) -> AppResult<IndexStats> {
    state.stats()
}

#[tauri::command(async)]
fn backup_index(state: tauri::State<'_, AppState>) -> AppResult<String> {
    state.backup_index()
}

#[tauri::command(async)]
fn optimize_index(state: tauri::State<'_, AppState>) -> AppResult<String> {
    state.optimize_index()
}

#[tauri::command]
fn get_global_shortcut() -> String {
    shortcut_prefs::load()
}

#[tauri::command(async)]
fn add_notion_source(
    state: tauri::State<'_, AppState>,
    token: String,
    database_id: String,
) -> AppResult<models::SourceInfo> {
    state.add_notion_database(token, database_id)
}

#[tauri::command]
fn get_notion_status(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    state.get_notion_status()
}

#[tauri::command]
fn get_app_version() -> String {
    update::current_version()
}

#[tauri::command(async)]
fn check_app_update() -> AppResult<update::UpdateInfo> {
    update::check_for_update()
}

/// Swap the global accelerator at runtime. On failure the previous binding is
/// restored so the app never ends up without a working hotkey.
#[tauri::command(async)]
fn apply_global_shortcut(app: tauri::AppHandle, shortcut: String) -> AppResult<String> {
    let trimmed = shortcut.trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > 64 {
        return Err(crate::error::AppError::msg(
            "快捷键格式无效（示例：Ctrl+Shift+Space 或 Alt+F9）",
        ));
    }
    let parsed = trimmed
        .parse::<Shortcut>()
        .map_err(|e| crate::error::AppError::msg(format!("无法识别快捷键 {trimmed}：{e}")))?;

    let gs = app.global_shortcut();
    gs.unregister_all()
        .map_err(|e| crate::error::AppError::msg(format!("解除旧快捷键失败：{e}")))?;
    match gs.register(parsed) {
        Ok(()) => {
            shortcut_prefs::save(&trimmed)?;
            log::info!("global shortcut changed to {trimmed}");
            Ok(trimmed)
        }
        Err(err) => {
            // Roll back so the previous hotkey keeps working.
            let prev = shortcut_prefs::load();
            if let Ok(prev_sc) = prev.parse::<Shortcut>() {
                let _ = gs.register(prev_sc);
            }
            Err(crate::error::AppError::msg(format!(
                "注册失败（可能被其他程序占用）：{err}"
            )))
        }
    }
}

#[tauri::command(async)]
fn schedule_index_restore(state: tauri::State<'_, AppState>, path: String) -> AppResult<String> {
    state.schedule_index_restore(path)
}

#[tauri::command(async)]
fn get_document_preview(
    state: tauri::State<'_, AppState>,
    document_id: String,
) -> AppResult<Option<String>> {
    state.get_document_preview(document_id)
}

#[tauri::command(async)]
fn add_local_folder(state: tauri::State<'_, AppState>, path: String) -> AppResult<SourceInfo> {
    state.add_local_folder(path)
}

#[tauri::command]
fn list_local_drives(state: tauri::State<'_, AppState>) -> AppResult<Vec<models::LocalDriveInfo>> {
    state.list_local_drives()
}

#[tauri::command(async)]
fn remove_source(state: tauri::State<'_, AppState>, source_id: String) -> AppResult<()> {
    state.remove_source(source_id)
}

#[tauri::command]
fn set_source_enabled(
    state: tauri::State<'_, AppState>,
    source_id: String,
    enabled: bool,
) -> AppResult<()> {
    state.set_source_enabled(source_id, enabled)
}

#[tauri::command(async)]
fn sync_source(state: tauri::State<'_, AppState>, source_id: String) -> AppResult<SyncReport> {
    state.sync_source(source_id)
}

#[tauri::command(async)]
fn sync_all(state: tauri::State<'_, AppState>) -> AppResult<Vec<SyncReport>> {
    state.sync_all()
}

#[tauri::command]
fn cancel_sync(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.request_cancel_sync();
    Ok(())
}

#[tauri::command]
fn get_sync_status(state: tauri::State<'_, AppState>) -> AppResult<models::SyncStatus> {
    state.sync_status()
}

#[tauri::command]
fn get_google_auth_status(
    state: tauri::State<'_, AppState>,
) -> AppResult<google_auth::GoogleAuthStatus> {
    state.google_auth_status()
}

#[tauri::command]
fn get_google_prefs(
    state: tauri::State<'_, AppState>,
) -> AppResult<google_prefs::GooglePrefsStatus> {
    state.google_prefs()
}

#[tauri::command]
fn set_google_sync_mode(
    state: tauri::State<'_, AppState>,
    mode: String,
) -> AppResult<google_prefs::GooglePrefsStatus> {
    state.set_google_sync_mode(mode)
}

#[tauri::command]
fn set_google_folder_filter(
    state: tauri::State<'_, AppState>,
    raw_text: String,
) -> AppResult<google_prefs::GooglePrefsStatus> {
    state.set_google_folder_filter(raw_text)
}

#[tauri::command]
fn clear_google_folder_filter(
    state: tauri::State<'_, AppState>,
) -> AppResult<google_prefs::GooglePrefsStatus> {
    state.clear_google_folder_filter()
}

#[tauri::command(async)]
fn save_google_oauth_config(
    state: tauri::State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> AppResult<google_auth::GoogleAuthStatus> {
    state.save_google_oauth_config(client_id, client_secret)
}

#[tauri::command(async)]
fn connect_google(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<google_auth::GoogleAuthStatus> {
    state.connect_google(&app)
}

#[tauri::command(async)]
fn disconnect_google(
    state: tauri::State<'_, AppState>,
) -> AppResult<google_auth::GoogleAuthStatus> {
    state.disconnect_google()
}

#[tauri::command]
fn list_google_watchlist(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<google_links::GoogleWatchItem>> {
    state.list_google_watchlist()
}

#[tauri::command(async)]
fn import_google_links(
    state: tauri::State<'_, AppState>,
    raw_text: String,
) -> AppResult<google_links::ImportLinksReport> {
    state.import_google_links(raw_text)
}

#[tauri::command(async)]
fn sync_google_watchlist(
    state: tauri::State<'_, AppState>,
) -> AppResult<google_links::ImportLinksReport> {
    state.sync_google_watchlist()
}

#[tauri::command(async)]
fn remove_google_watch_ids(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<Vec<google_links::GoogleWatchItem>> {
    state.remove_google_watch_ids(ids)
}

fn focus_main_window(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        focus_main_window(&window);
    }
}

fn launched_via_autostart() -> bool {
    std::env::args().any(|a| a == "--autostart")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            // File + stdout logging so packaged builds stay diagnosable.
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("searchdoc".to_string()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let state = AppState::init().map_err(|err| std::io::Error::other(err.to_string()))?;
            let emitter = app.handle().clone();
            state.set_status_broadcaster(Box::new(move |status| {
                let _ = tauri::Emitter::emit(&emitter, "sync-status", status);
            }));
            app.manage(state.clone());

            // Second-level incremental indexing: filesystem watcher keeps local
            // sources fresh between the five-minute fallback sweeps.
            log::info!(
                "SearchDoc started · data dir {}",
                app_state::APP_DATA_DIR.display()
            );
            watcher::spawn(state);

            // Keep the local index fresh while the app is open. The database
            // already skips unchanged documents by content hash.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(AUTO_SYNC_INTERVAL);
                if let Some(state) = handle.try_state::<AppState>() {
                    if !state.is_sync_running() {
                        let _ = state.sync_all();
                    }
                }
            });

            #[cfg(desktop)]
            {
                // Login item / startup registry. Args keep boot launches quiet (tray).
                app.handle().plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    Some(vec!["--autostart"]),
                ))?;

                // Close → hide to tray instead of quitting.
                if let Some(window) = app.get_webview_window("main") {
                    let window_for_event = window.clone();
                    window.on_window_event(move |event| {
                        if let WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = window_for_event.hide();
                        }
                    });
                }

                // Boot via autostart: stay in tray so login isn't blocked by a window.
                if launched_via_autostart() {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }

                // Global shortcut (user-configurable; persisted via shortcut_prefs).
                // The handler intentionally reacts to ANY registered shortcut of
                // ours, so runtime re-registration needs no handler swap.
                let configured = shortcut_prefs::load();
                let effective = match configured.parse::<Shortcut>() {
                    Ok(sc) => sc,
                    Err(_) => shortcut_prefs::DEFAULT_SHORTCUT
                        .parse::<Shortcut>()
                        .expect("built-in default shortcut must parse"),
                };
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, _sc, event| {
                            if event.state != ShortcutState::Pressed {
                                return;
                            }
                            show_main_window(app);
                        })
                        .build(),
                )?;
                app.global_shortcut().register(effective)?;
                log::info!("global shortcut registered: {configured}");

                // System tray
                let show_i =
                    MenuItem::with_id(app, "show", "显示 SearchDoc", true, None::<&str>)?;
                let sync_i =
                    MenuItem::with_id(app, "sync_all", "同步全部来源", true, None::<&str>)?;
                let sep = PredefinedMenuItem::separator(app)?;
                let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &sync_i, &sep, &quit_i])?;

                let mut tray_builder = TrayIconBuilder::new()
                    .menu(&menu)
                    .tooltip(format!("SearchDoc — {configured}"))
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => show_main_window(app),
                        "sync_all" => {
                            if let Some(state) = app.try_state::<AppState>() {
                                // Don't block the tray/UI thread on a long drive scan.
                                if state.is_sync_running() {
                                    return;
                                }
                                let handle = app.clone();
                                std::thread::spawn(move || {
                                    if let Some(state) = handle.try_state::<AppState>() {
                                        match state.sync_all() {
                                            Ok(reports) => {
                                                let total: usize = reports
                                                    .iter()
                                                    .map(|r| r.indexed)
                                                    .sum();
                                                log::info!(
                                                    "tray sync_all done: updated={total}, sources={}",
                                                    reports.len()
                                                );
                                            }
                                            Err(err) => {
                                                log::error!("tray sync_all failed: {err}")
                                            }
                                        }
                                    }
                                });
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main_window(tray.app_handle());
                        }
                    });

                if let Some(icon) = app.default_window_icon() {
                    tray_builder = tray_builder.icon(icon.clone());
                }

                let _tray = tray_builder.build(app)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_documents,
            list_sources,
            get_stats,
            backup_index,
            optimize_index,
            get_global_shortcut,
            apply_global_shortcut,
            add_notion_source,
            get_notion_status,
            get_app_version,
            check_app_update,
            schedule_index_restore,
            get_document_preview,
            add_local_folder,
            list_local_drives,
            remove_source,
            set_source_enabled,
            sync_source,
            sync_all,
            cancel_sync,
            get_sync_status,
            get_google_auth_status,
            get_google_prefs,
            set_google_sync_mode,
            set_google_folder_filter,
            clear_google_folder_filter,
            save_google_oauth_config,
            connect_google,
            disconnect_google,
            list_google_watchlist,
            import_google_links,
            sync_google_watchlist,
            remove_google_watch_ids
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        // Keep process alive when all windows are hidden (tray mode).
        if let RunEvent::ExitRequested { api, .. } = event {
            // Only allow exit via tray "quit" which calls app.exit.
            // Window close already prevents default; this guards edge cases.
            let _ = api;
        }
    });
}
