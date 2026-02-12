#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Navigate the main window to the remote Humanity web app
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.navigate("https://united-humanity.us".parse().unwrap());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Humanity");
}
