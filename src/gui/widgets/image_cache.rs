//! Chat image cache and fetcher.
//!
//! Manages image downloads from the relay (or any URL) so chat messages that
//! reference `/uploads/<file>.jpg` can be rendered inline inside egui. The
//! cache runs blocking HTTP on a background thread pool — no tokio — and
//! streams decoded RGBA buffers back through an mpsc channel. The main
//! render loop calls `poll(ctx)` once per frame; any decoded images waiting
//! are uploaded to egui's texture store and become instantly available via
//! `get_texture(url)`.
//!
//! Design goals:
//! - No external async runtime required; the app is single-threaded egui.
//! - Fetch-on-demand via `request(url)`. Idempotent; duplicate requests are
//!   silently deduped.
//! - Status query: `status(url)` returns `Idle / Fetching / Ready(size) /
//!   Failed(err)` so the chat UI can show a spinner or an error pill.
//! - Downloads are orthogonal: `download(url, dest)` spawns a thread that
//!   writes the raw bytes to `dest` and posts a completion event.

use egui::{ColorImage, Context, TextureHandle};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;

/// What state an image URL is in.
#[derive(Debug, Clone)]
pub enum ImageStatus {
    Idle,
    Fetching,
    Ready { width: u32, height: u32 },
    Failed(String),
}

/// Message posted back to the main thread from background workers.
enum BgResult {
    Decoded {
        url: String,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Failed {
        url: String,
        err: String,
    },
    Downloaded {
        url: String,
        path: std::path::PathBuf,
    },
    DownloadFailed {
        url: String,
        err: String,
    },
}

pub struct ImageCache {
    /// Textures that have finished loading and been uploaded to egui.
    textures: HashMap<String, TextureHandle>,
    /// URLs currently being fetched (to de-duplicate).
    fetching: HashSet<String>,
    /// Error messages for URLs that failed to load.
    errors: HashMap<String, String>,
    /// Last successful download path (path toast). Keyed by URL.
    downloads: HashMap<String, std::path::PathBuf>,
    /// Cap on max pixels per image to avoid blowing memory on a 10k×10k jpg.
    /// Default ~8 MP (3840×2160). Larger images are downsampled before upload.
    pub max_pixels: u32,
    /// Background-worker result channel.
    tx: mpsc::Sender<BgResult>,
    rx: mpsc::Receiver<BgResult>,
}

impl Default for ImageCache {
    fn default() -> Self { Self::new() }
}

impl ImageCache {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            textures: HashMap::new(),
            fetching: HashSet::new(),
            errors: HashMap::new(),
            downloads: HashMap::new(),
            max_pixels: 8_294_400, // 3840x2160
            tx,
            rx,
        }
    }

    /// Get the current status of an image URL.
    pub fn status(&self, url: &str) -> ImageStatus {
        if self.textures.contains_key(url) {
            let tex = &self.textures[url];
            let [w, h] = tex.size();
            ImageStatus::Ready { width: w as u32, height: h as u32 }
        } else if self.fetching.contains(url) {
            ImageStatus::Fetching
        } else if let Some(err) = self.errors.get(url) {
            ImageStatus::Failed(err.clone())
        } else {
            ImageStatus::Idle
        }
    }

    /// Return a reference to the loaded texture if ready.
    pub fn get_texture(&self, url: &str) -> Option<&TextureHandle> {
        self.textures.get(url)
    }

    /// Request an image fetch. No-op if already fetching or loaded.
    pub fn request(&mut self, url: &str) {
        if self.textures.contains_key(url) || self.fetching.contains(url) {
            return;
        }
        if self.errors.contains_key(url) {
            // Let retries clear the error so the user can try again by
            // re-opening the message, but for now: one error = give up to
            // avoid retry storms.
            return;
        }
        self.fetching.insert(url.to_string());
        let url_owned = url.to_string();
        let tx = self.tx.clone();
        let max_pixels = self.max_pixels;
        thread::Builder::new()
            .name("image-fetch".to_string())
            .spawn(move || {
                let result = fetch_and_decode(&url_owned, max_pixels);
                let msg = match result {
                    Ok((w, h, rgba)) => BgResult::Decoded { url: url_owned, width: w, height: h, rgba },
                    Err(e) => BgResult::Failed { url: url_owned, err: e },
                };
                let _ = tx.send(msg);
            })
            .ok();
    }

    /// Drain any completed background results and upload textures. Call this
    /// once per frame (cheap if nothing is ready). `ctx` is an egui Context.
    pub fn poll(&mut self, ctx: &Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                BgResult::Decoded { url, width, height, rgba } => {
                    self.fetching.remove(&url);
                    let pixels: Vec<egui::Color32> = rgba
                        .chunks_exact(4)
                        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                        .collect();
                    let image = ColorImage {
                        size: [width as usize, height as usize],
                        pixels,
                    };
                    let name = format!("chat_img:{}", url);
                    let handle = ctx.load_texture(&name, image, egui::TextureOptions::LINEAR);
                    self.textures.insert(url, handle);
                }
                BgResult::Failed { url, err } => {
                    self.fetching.remove(&url);
                    self.errors.insert(url, err);
                }
                BgResult::Downloaded { url, path } => {
                    self.downloads.insert(url, path);
                }
                BgResult::DownloadFailed { url, err } => {
                    self.errors.insert(format!("dl:{}", url), err);
                }
            }
        }
    }

    /// Start a download of `url` to `dest` (full path including filename).
    /// The download runs on a worker thread. Completion is reported via
    /// `downloaded_path(url)` on subsequent frames.
    pub fn download(&self, url: &str, dest: std::path::PathBuf) {
        let url_owned = url.to_string();
        let tx = self.tx.clone();
        thread::Builder::new()
            .name("image-download".to_string())
            .spawn(move || {
                let msg = match download_bytes(&url_owned) {
                    Ok(bytes) => {
                        if let Some(parent) = dest.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::write(&dest, &bytes) {
                            Ok(()) => BgResult::Downloaded { url: url_owned, path: dest },
                            Err(e) => BgResult::DownloadFailed { url: url_owned, err: e.to_string() },
                        }
                    }
                    Err(e) => BgResult::DownloadFailed { url: url_owned, err: e },
                };
                let _ = tx.send(msg);
            })
            .ok();
    }

    /// If a download completed for this URL, returns the path it was saved to.
    pub fn downloaded_path(&self, url: &str) -> Option<&std::path::Path> {
        self.downloads.get(url).map(|p| p.as_path())
    }
}

/// Blocking HTTP GET via ureq + decode with the `image` crate.
fn fetch_and_decode(url: &str, max_pixels: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = download_bytes(url)?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("decode: {e}"))?;
    let (w, h) = (img.width(), img.height());

    // Downsample if the raw image exceeds the cap (keeps memory bounded).
    let img = if w * h > max_pixels {
        let scale = (max_pixels as f32 / (w * h) as f32).sqrt();
        let nw = (w as f32 * scale).max(1.0) as u32;
        let nh = (h as f32 * scale).max(1.0) as u32;
        img.resize(nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((w, h, rgba.into_raw()))
}

/// Blocking download of raw bytes from a URL.
fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| format!("GET {}: {}", url, e))?;
    if resp.status() != 200 {
        return Err(format!("GET {}: HTTP {}", url, resp.status()));
    }
    let mut reader = resp.into_reader();
    let mut bytes = Vec::with_capacity(64 * 1024);
    std::io::Read::read_to_end(&mut reader, &mut bytes)
        .map_err(|e| format!("read body: {e}"))?;
    Ok(bytes)
}

/// Scan a plain-text string for image URLs. Returns each URL substring we
/// find that has an image extension. Both absolute (http/https) and relative
/// (`/uploads/...`) are matched. Absolute URLs pass through; relative URLs
/// should be prefixed with the server URL by the caller.
pub fn extract_image_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_lowercase();
    // Simple token-based scan: iterate substrings that end in a recognized ext.
    for tok in text.split_whitespace() {
        let tok_lower = tok.to_lowercase();
        if is_image_url(&tok_lower) {
            out.push(strip_trailing_punct(tok).to_string());
        } else if tok_lower.contains("/uploads/") {
            // Sometimes the URL has embedded extension differences; be lenient
            // about known image extensions anywhere in the token.
            if IMAGE_EXTS.iter().any(|e| tok_lower.contains(e)) {
                out.push(strip_trailing_punct(tok).to_string());
            }
        }
    }
    // Also scan for bare /uploads/... paths on their own line (common in
    // messages produced by the upload flow).
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("/uploads/") && is_image_url(&trimmed.to_lowercase())
            && !out.iter().any(|u| u == trimmed)
        {
            out.push(trimmed.to_string());
        }
    }
    // Dedup preserving order.
    let mut seen = HashSet::new();
    out.retain(|u| seen.insert(u.clone()));
    let _ = lower; // suppress unused warning on lower
    out
}

const IMAGE_EXTS: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".gif"];

fn is_image_url(s_lower: &str) -> bool {
    IMAGE_EXTS.iter().any(|e| s_lower.ends_with(e))
        || IMAGE_EXTS.iter().any(|e| {
            // Also match `.jpg.jpg` or `.jpgpng.jpg` patterns sometimes
            // produced by the current uploader; match if any image ext is in
            // the last path segment.
            s_lower.rsplit('/').next().map_or(false, |seg| seg.contains(e))
        })
}

fn strip_trailing_punct(s: &str) -> &str {
    s.trim_end_matches(|c: char| matches!(c, '.' | ',' | ')' | ']' | '}' | '!' | '?'))
}

/// Resolve a URL from a message: absolute URLs pass through; relative paths
/// starting with `/` get the server URL prepended.
pub fn resolve_url(raw: &str, server_url: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.starts_with('/') {
        format!("{}{}", server_url.trim_end_matches('/'), raw)
    } else {
        raw.to_string()
    }
}

/// Return a copy of `text` with all image URLs removed, collapsing any
/// runs of whitespace left behind so the text reads cleanly. If the
/// original text was nothing but URLs, returns an empty string.
pub fn strip_image_urls(text: &str) -> String {
    let urls = extract_image_urls(text);
    if urls.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for url in &urls {
        out = out.replace(url, "");
    }
    // Collapse repeated whitespace / empty lines that the removal left behind.
    let mut cleaned = String::with_capacity(out.len());
    let mut last_was_newline = false;
    for ch in out.chars() {
        match ch {
            '\n' | '\r' => {
                if !last_was_newline {
                    cleaned.push('\n');
                    last_was_newline = true;
                }
            }
            ' ' | '\t' => {
                if !cleaned.is_empty() && !cleaned.ends_with(' ') && !cleaned.ends_with('\n') {
                    cleaned.push(' ');
                }
            }
            _ => {
                cleaned.push(ch);
                last_was_newline = false;
            }
        }
    }
    cleaned.trim().to_string()
}

/// Suggest a local filename for `url`. Strips query strings and uses the
/// last path segment, falling back to a timestamp-based name.
pub fn filename_from_url(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    let last = without_query.rsplit('/').next().unwrap_or("image");
    if last.is_empty() {
        format!(
            "image_{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        )
    } else {
        last.to_string()
    }
}

/// Default location for downloaded attachments: a `downloads/` folder
/// next to the running executable. Matches the user's ask for
/// `C:\Humanity\downloads\` when the exe is at the repo root.
pub fn default_downloads_dir() -> std::path::PathBuf {
    let fallback = std::path::PathBuf::from("downloads");
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("downloads")))
        .unwrap_or(fallback)
}
