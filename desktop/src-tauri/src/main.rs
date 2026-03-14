#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_plugin_updater::UpdaterExt;

/// JS injected into every page load.
/// Ctrl+Shift+Delete clears all SW caches and hard-reloads — fixes stale pages after a deploy.
const INIT_SCRIPT: &str = r#"
(function () {
    if (window.__HOS_APP_INIT__) return;
    window.__HOS_APP_INIT__ = true;
    document.addEventListener('keydown', async function (e) {
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let version = app.config().version.clone().unwrap_or_else(|| "dev".to_string());

            // Build main window pointing at the live site, with keyboard shortcut injection
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External("https://united-humanity.us".parse().unwrap()),
            )
            .title(format!("Humanity — v{version}"))
            .inner_size(1200.0, 800.0)
            .min_inner_size(400.0, 300.0)
            .initialization_script(INIT_SCRIPT)
            .build()?;

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
