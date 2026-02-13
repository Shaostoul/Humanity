#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Navigate the main window to the remote Humanity web app
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.navigate("https://united-humanity.us".parse().unwrap());
            }

            // Check for updates in the background
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                match handle.updater() {
                    Ok(updater) => {
                        if let Ok(Some(update)) = updater.check().await {
                            let _ = update.download_and_install(
                                |downloaded, total| {
                                    let _ = (downloaded, total);
                                },
                                || {},
                            ).await;
                        }
                    }
                    Err(e) => eprintln!("Updater error: {e}"),
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Humanity");
}
