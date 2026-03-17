#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::Manager as _;
use tauri_plugin_updater::UpdaterExt;

/// Tracks whether an update is available (version string).
struct UpdateState(Mutex<Option<String>>);

/// JS injected before every page runs.
/// F12            → open DevTools
/// Ctrl+Shift+Del → clear all SW caches and hard-reload (fixes stale pages after a deploy)
const INIT_SCRIPT: &str = r#"
(function () {
    if (window.__HOS_APP_INIT__) return;
    window.__HOS_APP_INIT__ = true;
    document.addEventListener('keydown', async function (e) {
        // F12 — open DevTools
        if (e.key === 'F12') {
            e.preventDefault();
            window.__TAURI__?.core?.invoke('open_devtools').catch(() => {});
        }
        // Ctrl+Shift+Delete — clear all caches and hard-reload
        if (e.ctrlKey && e.shiftKey && e.key === 'Delete') {
            e.preventDefault();
            try {
                const names = await caches.keys();
                await Promise.all(names.map(n => caches.delete(n)));
                const regs = await navigator.serviceWorker.getRegistrations();
                await Promise.all(regs.map(r => r.unregister()));
            } catch (_) {}
            location.reload(true);
        }
    });
})();
"#;

/// Called from JS (F12) to open the WebView DevTools panel.
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

/// Called from JS when user clicks the download/update button.
/// Downloads and installs the update, then restarts the app.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    let updater = app.updater().map_err(|e| format!("Updater error: {e}"))?;
    let update = updater.check().await
        .map_err(|e| format!("Check failed: {e}"))?
        .ok_or_else(|| "No update available".to_string())?;

    let version = update.version.clone();
    update.download_and_install(|_, _| {}, || {}).await
        .map_err(|e| format!("Install failed: {e}"))?;

    Ok(version)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(UpdateState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![open_devtools, install_update])
        .setup(|app| {
            let version = app.config().version.clone().unwrap_or_else(|| "dev".to_string());

            // Build the main window — loads united-humanity.us, injects keyboard shortcuts
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External("https://united-humanity.us".parse().unwrap()),
            )
            .title(format!("Humanity — v{version}"))
            .inner_size(1200.0, 800.0)
            .min_inner_size(400.0, 300.0)
            .devtools(true)
            .initialization_script(INIT_SCRIPT)
            .build()?;

            // ── Background update check: notify webview if update exists ──
            let handle = app.handle().clone();
            let win_clone = window.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                match handle.updater() {
                    Ok(updater) => {
                        match updater.check().await {
                            Ok(Some(update)) => {
                                let new_version = update.version.clone();
                                eprintln!("Update available: v{new_version}");

                                // Store version in state.
                                if let Some(state) = handle.try_state::<UpdateState>() {
                                    *state.0.lock().unwrap() = Some(new_version.clone());
                                }

                                // Notify webview — shell.js will light up the download button.
                                let js = format!(
                                    "window.__HOS_UPDATE_READY = true; window.__HOS_UPDATE_VERSION = '{}';",
                                    new_version
                                );
                                let _ = win_clone.eval(&js);

                                // Update title bar.
                                let current = handle.config().version.clone().unwrap_or_default();
                                let _ = win_clone.set_title(&format!(
                                    "Humanity — v{current} (v{new_version} ready)"
                                ));
                            }
                            Ok(None) => eprintln!("App is up to date"),
                            Err(e) => eprintln!("Update check failed: {e}"),
                        }
                    }
                    Err(e) => eprintln!("Updater init error: {e}"),
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Humanity");
}
