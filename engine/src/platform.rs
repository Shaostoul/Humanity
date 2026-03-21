//! Platform abstraction for native vs WASM.
//!
//! Game code calls these functions without caring which platform runs underneath.
//! On native: std::fs, std::time, println. On WASM: fetch, performance.now(), console.log.

/// Log a message through the platform's logging mechanism.
pub fn log_info(msg: &str) {
    log::info!("{}", msg);
}

/// Get the current time in seconds (monotonic).
#[cfg(feature = "native")]
pub fn current_time_secs() -> f64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Get the current time in seconds (from performance.now(), monotonic).
#[cfg(feature = "wasm")]
pub fn current_time_secs() -> f64 {
    web_sys::window()
        .expect("no global window")
        .performance()
        .expect("no performance API")
        .now()
        / 1000.0
}

/// Load asset bytes from disk (native) — synchronous.
#[cfg(feature = "native")]
pub fn load_asset_bytes_sync(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("Failed to load {}: {}", path, e))
}

/// Load asset bytes via fetch (WASM) — async only.
/// On WASM, use `load_asset_bytes_async` instead.
#[cfg(feature = "wasm")]
pub async fn load_asset_bytes_async(url: &str) -> Result<Vec<u8>, String> {
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, Response};

    let window = web_sys::window().ok_or("no global window")?;

    let mut opts = RequestInit::new();
    opts.method("GET");

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast failed".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP {} for {}", resp.status(), url));
    }

    let array_buffer = JsFuture::from(
        resp.array_buffer()
            .map_err(|e| format!("ArrayBuffer failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("ArrayBuffer await failed: {:?}", e))?;

    let buffer: ArrayBuffer = array_buffer
        .dyn_into()
        .map_err(|_| "ArrayBuffer cast failed".to_string())?;

    let uint8_array = Uint8Array::new(&buffer);
    Ok(uint8_array.to_vec())
}
