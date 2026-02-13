#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

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
                // Wait a few seconds before checking
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Ok(updater) = handle.updater().check().await {
                    if let Some(update) = updater {
                        // Download and install silently
                        let _ = update.download_and_install(|_, _| {}, || {}).await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Humanity");
}
