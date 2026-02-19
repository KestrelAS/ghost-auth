mod backup;
mod commands;
mod google_auth_proto;
mod import;
mod keystore;
mod pin;
mod storage;
mod sync;
mod sync_transport;
mod sync_ws;
mod totp;

use std::path::Path;
use std::sync::Mutex;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const LOG_RETENTION_DAYS: u64 = 7;

/// Delete log files older than `LOG_RETENTION_DAYS`.
/// Runs once at startup; failures are logged but never block the app.
fn cleanup_old_logs(log_dir: &Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(LOG_RETENTION_DAYS * 24 * 60 * 60);

    for entry in entries.flatten() {
        let path = entry.path();

        // Only consider files whose name starts with the log prefix
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.starts_with("ghost-auth.log.") => n.to_string(),
            _ => continue,
        };

        // Skip the current day's log (no date suffix means active log)
        if name == "ghost-auth.log" {
            continue;
        }

        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        if modified < cutoff {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!(file = %name, error = %e, "Failed to remove old log file");
            } else {
                tracing::debug!(file = %name, "Removed old log file");
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init());

    // Mobile-only plugins (crates gated behind #![cfg(mobile)])
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_barcode_scanner::init())
        .plugin(tauri_plugin_biometric::init())
        .plugin(tauri_plugin_edge_to_edge::init())
        .plugin(tauri_plugin_share_file::init());

    builder
        .setup(|app| {
            use tauri::Manager;
            let data_dir = app.path().app_data_dir().map_err(|e| {
                eprintln!("Failed to resolve app data directory: {e}");
                e
            })?;

            // Initialize tracing with both stdout and file logging
            let log_dir = data_dir.join("logs");
            let file_appender = tracing_appender::rolling::daily(&log_dir, "ghost-auth.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            let filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ghost_auth_lib=info"));
            let stdout_layer = fmt::layer().with_target(false);
            let file_layer = fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(non_blocking);

            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();

            // Keep the file writer guard alive for the app's lifetime
            app.manage(guard);

            tracing::info!(event = "app_started", "Ghost Auth started");

            let log_dir_clone = log_dir.clone();
            std::thread::spawn(move || cleanup_old_logs(&log_dir_clone));

            // Initialize storage on a background thread so the Keychain
            // syscall doesn't block the main thread (iOS watchdog).
            let data_dir_clone = data_dir.clone();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let _ = tx.send(storage::Storage::new(data_dir_clone));
            });
            let store = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| {
                    tracing::error!("Storage initialization timed out after 5s");
                    "Storage initialization timed out".to_string()
                })?
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to initialize encrypted storage");
                    e
                })?;
            app.manage(Mutex::new(store));
            app.manage(pin::PinManager::new(data_dir));
            app.manage(commands::SyncManager::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_accounts,
            commands::add_account,
            commands::add_account_manual,
            commands::edit_account,
            commands::delete_account,
            commands::reorder_accounts,
            commands::generate_code,
            commands::generate_all_codes,
            commands::has_pin,
            commands::set_pin,
            commands::verify_pin,
            commands::remove_pin,
            commands::export_backup,
            commands::import_backup,
            commands::import_backup_confirm,
            commands::save_backup_file,
            commands::verify_recovery_code,
            commands::has_recovery_codes,
            commands::get_export_accounts,
            commands::import_external_preview,
            commands::import_external_confirm,
            commands::sync_start,
            commands::sync_poll,
            commands::sync_join,
            commands::sync_confirm,
            commands::sync_cancel,
            commands::sync_history,
            commands::save_theme,
        ])
        .build(tauri::generate_context!())
        .expect("Fatal: failed to start Ghost Auth — check system logs for details")
        .run(|app, event| {
            match event {
                tauri::RunEvent::Exit => {
                    tracing::info!(event = "app_exiting", "Ghost Auth shutting down");
                }
                tauri::RunEvent::Resumed => {
                    tracing::info!(event = "app_resumed", "Ghost Auth resumed from background");
                }
                #[cfg(not(mobile))]
                tauri::RunEvent::Ready => {
                    // Apply the user's saved theme to the native window so the
                    // macOS title bar matches before the WebView finishes loading.
                    use tauri::Manager;
                    if let Ok(data_dir) = app.path().app_data_dir() {
                        if let Ok(saved) = std::fs::read_to_string(data_dir.join("theme")) {
                            let theme = match saved.trim() {
                                "dark" => tauri::Theme::Dark,
                                _ => tauri::Theme::Light,
                            };
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.set_theme(Some(theme));
                            }
                        }
                    }
                }
                _ => {}
            }
        });
}
