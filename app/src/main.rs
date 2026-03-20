#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod storage;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager as _;
use tauri_plugin_updater::UpdaterExt;

use storage::{LocalStorage, LocalStorageState, StorageStats, SyncConfig, DetectedDrive};

// ── State types ──────────────────────────────────────────────────────────────

/// Tracks whether a binary (Tauri) update is available.
struct UpdateState(Mutex<Option<String>>);

/// Tracks whether a web asset sync found changes (triggers "reload?" prompt).
struct WebSyncState(Mutex<bool>);

// ── Web manifest schema ──────────────────────────────────────────────────────

/// Manifest listing every web asset with its SHA-256 hash.
/// Served by the relay at /api/web-manifest and stored locally as manifest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebManifest {
    /// Semver string for the web assets (e.g. "0.10.0")
    version: String,
    /// Map of relative path → hex-encoded SHA-256 hash
    files: HashMap<String, String>,
}

// ── Init script ──────────────────────────────────────────────────────────────

/// JS injected before every page runs.
/// - F12            → open DevTools
/// - Ctrl+Shift+Del → clear all SW caches and hard-reload
/// - Intercepts ALL external link clicks → opens in system browser
/// - window.open() override → opens in system browser
const INIT_SCRIPT: &str = r#"
(function () {
    if (window.__HOS_APP_INIT__) return;
    window.__HOS_APP_INIT__ = true;

    // Mark that we're running inside the desktop app
    window.__HOS_DESKTOP__ = true;

    // Local-first storage is available via Tauri commands
    window.__HOS_HAS_LOCAL_STORAGE = true;

    // Remote server base URL — all API/WebSocket calls route here
    // since Tauri serves local files, not a web server
    var SERVER = 'https://united-humanity.us';
    window.__HOS_SERVER = SERVER;

    // Override fetch() so /api/ calls go through a Tauri command (Rust-side HTTP).
    // This completely bypasses CORS — the browser never makes a cross-origin request.
    var _origFetch = window.fetch.bind(window);
    window.fetch = function(input, init) {
        var url = (typeof input === 'string') ? input : (input && input.url) ? input.url : null;
        var isApi = url && (url.startsWith('/api') ||
            ((url.startsWith('tauri://') || url.startsWith('https://tauri.localhost')) && url.indexOf('/api') !== -1));

        if (isApi) {
            var apiPath = url.startsWith('/api') ? url : url.substring(url.indexOf('/api'));
            var method = (init && init.method) ? init.method : 'GET';
            var body = (init && init.body) ? (typeof init.body === 'string' ? init.body : JSON.stringify(init.body)) : null;
            var headers = {};
            if (init && init.headers) {
                if (init.headers instanceof Headers) {
                    init.headers.forEach(function(v, k) { headers[k] = v; });
                } else if (typeof init.headers === 'object') {
                    headers = init.headers;
                }
            }
            return window.__TAURI__.core.invoke('api_proxy', {
                path: apiPath, method: method, body: body, headers: headers
            }).then(function(result) {
                return new Response(result.body, {
                    status: result.status,
                    headers: { 'Content-Type': result.content_type || 'application/json' }
                });
            });
        }
        return _origFetch(input, init);
    };

    // Override WebSocket to rewrite relative /ws paths to the remote server.
    var _origWS = window.WebSocket;
    window.WebSocket = function(url, protocols) {
        if (url && (url.indexOf('tauri.localhost') !== -1 || url.indexOf('localhost') !== -1)) {
            url = url.replace(/wss?:\/\/[^\/]+/, 'wss://united-humanity.us');
        }
        return new _origWS(url, protocols);
    };
    window.WebSocket.prototype = _origWS.prototype;
    window.WebSocket.CONNECTING = _origWS.CONNECTING;
    window.WebSocket.OPEN = _origWS.OPEN;
    window.WebSocket.CLOSING = _origWS.CLOSING;
    window.WebSocket.CLOSED = _origWS.CLOSED;

    // Helper: open URL in system browser via Tauri command
    function openExternal(url) {
        if (window.__TAURI__?.core?.invoke) {
            window.__TAURI__.core.invoke('open_external_url', { url: url }).catch(function(err) {
                console.error('Tauri open_external_url failed:', err);
            });
            return true;
        }
        return false;
    }

    // Check if URL is external (not our app — local protocol or united-humanity.us)
    function isExternal(href) {
        try {
            var url = new URL(href, location.origin);
            return url.hostname &&
                   url.hostname !== 'united-humanity.us' &&
                   url.hostname !== 'localhost' &&
                   url.hostname !== '127.0.0.1' &&
                   url.hostname !== 'tauri.localhost' &&
                   url.protocol !== 'tauri:';
        } catch (_) {
            return false;
        }
    }

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

    // Rewrite extensionless paths to .html for Tauri static file serving.
    // Nginx does this automatically on the VPS, but Tauri serves files literally.
    // e.g. /tasks → /tasks.html, /chat → /chat/index.html (already has index.html)
    function rewriteForTauri(href) {
        try {
            var url = new URL(href, location.origin);
            var p = url.pathname;
            // Skip if already has extension, is root, or ends with /
            if (p === '/' || p.indexOf('.') !== -1 || p.endsWith('/')) return href;
            // Skip known directory routes (they have index.html inside)
            if (p === '/chat') return href + '/index.html';
            // All other extensionless paths → append .html
            url.pathname = p + '.html';
            return url.href;
        } catch(_) { return href; }
    }

    // Intercept ALL clicks on links — rewrite paths + open external in system browser
    document.addEventListener('click', function (e) {
        var link = e.target.closest('a[href]');
        if (!link) return;

        var href = link.getAttribute('href');
        if (!href || href === '#' || href.startsWith('javascript:')) return;

        // Resolve relative URLs
        var fullUrl;
        try { fullUrl = new URL(href, location.origin).href; } catch(_) { return; }

        if (isExternal(fullUrl)) {
            e.preventDefault();
            e.stopPropagation();
            e.stopImmediatePropagation();
            openExternal(fullUrl);
            return false;
        }

        // Rewrite extensionless internal links for Tauri static serving
        var rewritten = rewriteForTauri(fullUrl);
        if (rewritten !== fullUrl) {
            e.preventDefault();
            location.href = rewritten;
            return false;
        }

        // Also catch target="_blank" links to our own domain (prevent new window)
        if (link.target === '_blank') {
            e.preventDefault();
            location.href = fullUrl;
        }
    }, true);

    // Override window.open to redirect external URLs to system browser
    var originalOpen = window.open;
    window.open = function(url) {
        if (url && isExternal(url)) {
            openExternal(url);
            return null;
        }
        if (url) location.href = url;
        return null;
    };
})();
"#;

// ── Tauri commands ───────────────────────────────────────────────────────────

/// Called from JS (F12) to open the WebView DevTools panel.
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

/// Called from JS to open a URL in the system's default browser.
#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {e}"))
}

/// Proxy API response from Rust back to JS.
#[derive(Serialize)]
struct ApiProxyResponse {
    status: u16,
    body: String,
    content_type: String,
}

/// Proxies HTTP requests from the WebView through Rust, completely bypassing
/// CORS. The browser never makes a cross-origin request — Rust's reqwest
/// handles the HTTP call and returns the response body as a string.
#[tauri::command]
async fn api_proxy(
    path: String,
    method: String,
    body: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> Result<ApiProxyResponse, String> {
    let url = format!("https://united-humanity.us{}", path);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut req = match method.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };

    // Forward headers from JS (e.g. Content-Type, Authorization)
    if let Some(hdrs) = headers {
        for (k, v) in hdrs {
            req = req.header(&k, &v);
        }
    }

    // Attach body for POST/PUT/PATCH
    if let Some(b) = body {
        req = req.body(b);
    }

    let resp = req.send().await.map_err(|e| format!("Request failed: {e}"))?;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body_text = resp.text().await.map_err(|e| format!("Read body failed: {e}"))?;

    Ok(ApiProxyResponse {
        status,
        body: body_text,
        content_type,
    })
}

/// Called from JS when user clicks the download/update button.
/// Downloads and installs the update, then restarts the app.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    let updater = app.updater().map_err(|e| format!("Updater error: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Check failed: {e}"))?
        .ok_or_else(|| "No update available".to_string())?;

    let version = update.version.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Install failed: {e}"))?;

    Ok(version)
}

// ── Local storage commands ───────────────────────────────────────────────────

/// Returns the names of all save slots on disk.
#[tauri::command]
fn list_saves(state: tauri::State<'_, LocalStorageState>) -> Vec<String> {
    let storage = state.0.lock().unwrap();
    storage.list_saves()
}

/// Creates a new empty save slot with stub JSON files.
#[tauri::command]
fn create_save(name: String, state: tauri::State<'_, LocalStorageState>) -> Result<(), String> {
    let storage = state.0.lock().unwrap();
    storage.create_save(&name)
}

/// Permanently removes a save slot and all its data.
#[tauri::command]
fn delete_save(name: String, state: tauri::State<'_, LocalStorageState>) -> Result<(), String> {
    let storage = state.0.lock().unwrap();
    storage.delete_save(&name)
}

/// Copies a save slot to an external directory (USB drive, backup location).
#[tauri::command]
fn export_save(
    name: String,
    target_path: String,
    state: tauri::State<'_, LocalStorageState>,
) -> Result<(), String> {
    let storage = state.0.lock().unwrap();
    let target = PathBuf::from(&target_path);
    storage.export_save(&name, &target)
}

/// Imports a save from an external directory, returns the assigned save name.
#[tauri::command]
fn import_save(
    source_path: String,
    state: tauri::State<'_, LocalStorageState>,
) -> Result<String, String> {
    let storage = state.0.lock().unwrap();
    let source = PathBuf::from(&source_path);
    storage.import_save(&source)
}

/// Scans for external/USB drives containing a HumanityOS folder.
#[tauri::command]
fn detect_drives(state: tauri::State<'_, LocalStorageState>) -> Vec<DetectedDrive> {
    let storage = state.0.lock().unwrap();
    storage.detect_external_drives()
}

/// Returns the current tiered sync configuration.
#[tauri::command]
fn get_sync_config(state: tauri::State<'_, LocalStorageState>) -> SyncConfig {
    let storage = state.0.lock().unwrap();
    storage.get_sync_config()
}

/// Updates which data categories sync to which server tiers.
#[tauri::command]
fn set_sync_config(
    config: SyncConfig,
    state: tauri::State<'_, LocalStorageState>,
) -> Result<(), String> {
    let storage = state.0.lock().unwrap();
    storage.set_sync_config(&config)
}

/// Returns the absolute path to the local data directory.
#[tauri::command]
fn get_data_dir(state: tauri::State<'_, LocalStorageState>) -> String {
    let storage = state.0.lock().unwrap();
    storage.data_dir().to_string_lossy().to_string()
}

/// Returns size and count statistics for the settings UI.
#[tauri::command]
fn get_storage_stats(state: tauri::State<'_, LocalStorageState>) -> StorageStats {
    let storage = state.0.lock().unwrap();
    storage.storage_stats()
}

/// Creates a timestamped backup snapshot, auto-rotates to keep last 5.
#[tauri::command]
fn create_backup(state: tauri::State<'_, LocalStorageState>) -> Result<String, String> {
    let storage = state.0.lock().unwrap();
    storage.create_backup(5)
}

/// Moves the entire data directory to a new location on disk.
#[tauri::command]
fn relocate_data(
    new_path: String,
    state: tauri::State<'_, LocalStorageState>,
) -> Result<(), String> {
    let mut storage = state.0.lock().unwrap();
    storage.relocate(PathBuf::from(new_path))
}

// ── Web sync logic ───────────────────────────────────────────────────────────

const MANIFEST_URL: &str = "https://united-humanity.us/api/web-manifest";
const ASSET_BASE_URL: &str = "https://united-humanity.us";

/// Returns the writable directory where synced web assets are stored.
/// Falls under the app's local data dir so it survives updates.
fn sync_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .expect("failed to resolve app local data dir")
        .join("web-sync")
}

/// Reads the local manifest.json from the sync directory, if it exists.
fn read_local_manifest(sync_path: &std::path::Path) -> Option<WebManifest> {
    let manifest_path = sync_path.join("manifest.json");
    let data = std::fs::read_to_string(manifest_path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Writes the manifest to the sync directory.
fn write_local_manifest(sync_path: &std::path::Path, manifest: &WebManifest) {
    let manifest_path = sync_path.join("manifest.json");
    if let Ok(json) = serde_json::to_string_pretty(manifest) {
        let _ = std::fs::write(manifest_path, json);
    }
}

/// Compute SHA-256 of a byte slice, return hex string.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// One round of web asset sync. Returns the number of files updated, or an error.
async fn sync_web_assets(sync_path: &std::path::Path) -> Result<(usize, WebManifest), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    // 1. Fetch remote manifest
    let remote_manifest: WebManifest = client
        .get(MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Manifest fetch failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Manifest parse failed: {e}"))?;

    // 2. Load local manifest (if any)
    let local_manifest = read_local_manifest(sync_path);

    // 3. Diff: find files that are new or changed
    let local_files = local_manifest
        .as_ref()
        .map(|m| &m.files)
        .cloned()
        .unwrap_or_default();

    let mut changed: Vec<(&String, &String)> = Vec::new();
    for (path, remote_hash) in &remote_manifest.files {
        match local_files.get(path) {
            Some(local_hash) if local_hash == remote_hash => {
                // Also verify the file actually exists on disk
                if sync_path.join(path).exists() {
                    continue;
                }
            }
            _ => {}
        }
        changed.push((path, remote_hash));
    }

    if changed.is_empty() {
        return Ok((0, remote_manifest));
    }

    // 4. Download changed files
    let mut updated = 0;
    for (path, expected_hash) in &changed {
        let url = format!("{}/{}", ASSET_BASE_URL, path);
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Download {path} failed: {e}"))?;

        if !response.status().is_success() {
            eprintln!("web-sync: HTTP {} for {}", response.status(), path);
            continue;
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Read {path} failed: {e}"))?;

        // Verify hash
        let actual_hash = sha256_hex(&bytes);
        if &actual_hash != *expected_hash {
            eprintln!(
                "web-sync: hash mismatch for {path} (expected {expected_hash}, got {actual_hash})"
            );
            continue;
        }

        // Write to sync directory
        let dest = sync_path.join(path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&dest, &bytes)
            .map_err(|e| format!("Write {path} failed: {e}"))?;
        updated += 1;
    }

    // 5. Also remove files that are no longer in the remote manifest
    if let Some(ref local) = local_manifest {
        for old_path in local.files.keys() {
            if !remote_manifest.files.contains_key(old_path) {
                let _ = std::fs::remove_file(sync_path.join(old_path));
            }
        }
    }

    // 6. Save updated manifest
    write_local_manifest(sync_path, &remote_manifest);

    Ok((updated, remote_manifest))
}

/// Spawns the periodic web sync task. Runs first check after `initial_delay`,
/// then every `interval`.
fn spawn_web_sync(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let handle = app.clone();
    let win = window.clone();
    let sync_path = sync_dir(app);
    let _ = std::fs::create_dir_all(&sync_path);

    tauri::async_runtime::spawn(async move {
        let initial_delay = std::time::Duration::from_secs(30);
        let interval = std::time::Duration::from_secs(30 * 60); // 30 minutes

        tokio::time::sleep(initial_delay).await;

        loop {
            eprintln!("web-sync: Checking for updates...");

            match sync_web_assets(&sync_path).await {
                Ok((0, manifest)) => {
                    eprintln!("web-sync: Up to date (v{})", manifest.version);
                    // Inject web version even if no changes
                    let js = format!(
                        "window.__HOS_WEB_VERSION = '{}';",
                        manifest.version
                    );
                    let _ = win.eval(&js);
                }
                Ok((count, manifest)) => {
                    eprintln!("web-sync: {} files updated (v{})", count, manifest.version);

                    // Store sync-ready state
                    if let Some(state) = handle.try_state::<WebSyncState>() {
                        *state.0.lock().unwrap() = true;
                    }

                    // Notify webview
                    let js = format!(
                        "window.__HOS_WEB_VERSION = '{}'; \
                         window.__HOS_WEB_UPDATE_READY = true; \
                         window.__HOS_WEB_UPDATED_COUNT = {}; \
                         if (typeof window.__hosWebUpdateReady === 'function') window.__hosWebUpdateReady({});",
                        manifest.version, count, count
                    );
                    let _ = win.eval(&js);

                    // Update title bar
                    let app_ver = handle.config().version.clone().unwrap_or_default();
                    let _ = win.set_title(&format!(
                        "Humanity — v{app_ver} (update ready)"
                    ));
                }
                Err(e) => {
                    eprintln!("web-sync: {e}");
                }
            }

            tokio::time::sleep(interval).await;
        }
    });
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(UpdateState(Mutex::new(None)))
        .manage(WebSyncState(Mutex::new(false)))
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            install_update,
            open_external_url,
            api_proxy,
            list_saves,
            create_save,
            delete_save,
            export_save,
            import_save,
            detect_drives,
            get_sync_config,
            set_sync_config,
            get_data_dir,
            get_storage_stats,
            create_backup,
            relocate_data
        ])
        .setup(|app| {
            let version = app
                .config()
                .version
                .clone()
                .unwrap_or_else(|| "dev".to_string());

            // ── Initialize local-first storage ──
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let local_storage = LocalStorage::new(data_dir);
            if let Err(e) = local_storage.init() {
                eprintln!("storage: init failed: {e}");
            }
            app.manage(LocalStorageState(Mutex::new(local_storage)));

            // ── Build the main window — loads from local files ──
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title(format!("Humanity — v{version}"))
            .inner_size(1200.0, 800.0)
            .min_inner_size(400.0, 300.0)
            .devtools(true)
            .initialization_script(INIT_SCRIPT)
            .build()?;

            // Inject app version + desktop flag into the webview
            let ver_js = format!(
                "window.__HOS_APP_VERSION = '{}'; window.__HOS_DESKTOP__ = true;",
                version
            );
            let _ = window.eval(&ver_js);

            // ── Inject web version from local manifest (if already synced) ──
            let sync_path = sync_dir(&app.handle());
            if let Some(manifest) = read_local_manifest(&sync_path) {
                let js = format!("window.__HOS_WEB_VERSION = '{}';", manifest.version);
                let _ = window.eval(&js);
            }

            // ── Background binary update check ──
            {
                let handle = app.handle().clone();
                let win_clone = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    match handle.updater() {
                        Ok(updater) => match updater.check().await {
                            Ok(Some(update)) => {
                                let new_version = update.version.clone();
                                eprintln!("Update available: v{new_version}");

                                if let Some(state) = handle.try_state::<UpdateState>() {
                                    *state.0.lock().unwrap() = Some(new_version.clone());
                                }

                                let js = format!(
                                    "window.__HOS_UPDATE_READY = true; \
                                     window.__HOS_UPDATE_VERSION = '{}';",
                                    new_version
                                );
                                let _ = win_clone.eval(&js);

                                let current =
                                    handle.config().version.clone().unwrap_or_default();
                                let _ = win_clone.set_title(&format!(
                                    "Humanity — v{current} (v{new_version} ready)"
                                ));
                            }
                            Ok(None) => eprintln!("App is up to date"),
                            Err(e) => eprintln!("Update check failed: {e}"),
                        },
                        Err(e) => eprintln!("Updater init error: {e}"),
                    }
                });
            }

            // ── Background web asset sync ──
            spawn_web_sync(&app.handle(), &window);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Humanity");
}
