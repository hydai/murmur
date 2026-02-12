// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing_subscriber;

#[derive(Clone, serde::Serialize)]
struct OverlayToggleEvent {
    visible: bool,
}

#[tauri::command]
fn toggle_overlay(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_visible = window.is_visible().map_err(|e| e.to_string())?;

    if is_visible {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }

    Ok(!is_visible)
}

#[tauri::command]
fn get_status() -> String {
    "Ready".to_string()
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lt_tauri=debug,info")
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![toggle_overlay, get_status])
        .setup(|app| {
            // Try to register global shortcut for overlay toggle
            let handle = app.handle().clone();

            // Register the shortcut handler first
            if let Err(e) = app.global_shortcut().on_shortcut("Cmd+Shift+Space", move |_app, _shortcut, _event| {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = toggle_overlay(window);
                }
            }) {
                tracing::warn!("Failed to set up shortcut handler: {}", e);
            }

            // Try to register the global shortcut (non-fatal if it fails)
            match app.global_shortcut().register("Cmd+Shift+Space") {
                Ok(_) => tracing::info!("Global shortcut Cmd+Shift+Space registered successfully"),
                Err(e) => tracing::warn!("Failed to register global shortcut: {}. The app will still work, but you'll need to use the window controls.", e),
            }

            tracing::info!("Localtype started successfully");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
