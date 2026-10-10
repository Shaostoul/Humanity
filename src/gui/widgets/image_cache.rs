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
//! - A file encrypted for a private conversation (10k of
//!   docs/design/blocking-and-safe-mode.md) goes through `request_bytes` and
//!   `save_with` instead: the caller's job fetches and decrypts on the worker,
//!   so neither the download nor the decryption ever runs on the frame, and
//!   the decrypted bytes live only in memory until the person chooses Save.

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
    /// Keys whose `save_with` job is still running, so the Save button can
    /// say so and is not pressed twice.
    saving: HashSet<String>,
    /// Pictures from another website that the person clicked to load
    /// (`route_image` sends those to a click-to-load placeholder). This session
    /// only, one picture at a time, the way the web client's placeholder works.
    allowed: HashSet<String>,
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
            saving: HashSet::new(),
            allowed: HashSet::new(),
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

    /// The person clicked to load this picture from another website.
    pub fn allow(&mut self, url: &str) {
        self.allowed.insert(url.to_string());
    }

    /// Whether the person clicked to load this picture from another website.
    pub fn is_allowed(&self, url: &str) -> bool {
        self.allowed.contains(url)
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

    /// Load a picture whose bytes come from `load` rather than from a plain GET
    /// of a URL: a file encrypted for a private conversation, whose ciphertext
    /// must be fetched AND decrypted before there is anything to decode. `key`
    /// names it in this cache (`PRIVATE_KEY_PREFIX` plus a hash, never the
    /// file's key). `load` runs on a worker thread, the bytes are decoded there
    /// too, and only the pixels come back. Idempotent like `request`, and a
    /// failure is final for the session, also like `request`.
    pub fn request_bytes<F>(&mut self, key: &str, load: F)
    where
        F: FnOnce() -> Result<Vec<u8>, String> + Send + 'static,
    {
        if self.textures.contains_key(key) || self.fetching.contains(key) || self.errors.contains_key(key) {
            return;
        }
        self.fetching.insert(key.to_string());
        let key_owned = key.to_string();
        let tx = self.tx.clone();
        let max_pixels = self.max_pixels;
        thread::Builder::new()
            .name("image-open".to_string())
            .spawn(move || {
                let msg = match load().and_then(|bytes| decode_bytes(&bytes, max_pixels)) {
                    Ok((width, height, rgba)) => BgResult::Decoded { url: key_owned, width, height, rgba },
                    Err(err) => BgResult::Failed { url: key_owned, err },
                };
                let _ = tx.send(msg);
            })
            .ok();
    }

    /// Run `job` on a worker thread: it fetches, decrypts and writes a private
    /// file, returning where it went. Reported through `downloaded_path(key)`
    /// and `save_failed(key)`, the way `download` is. A second press while it
    /// runs does nothing; a press after a failure tries again.
    pub fn save_with<F>(&mut self, key: &str, job: F)
    where
        F: FnOnce() -> Result<std::path::PathBuf, String> + Send + 'static,
    {
        if self.saving.contains(key) {
            return;
        }
        self.errors.remove(&format!("dl:{key}"));
        self.saving.insert(key.to_string());
        let key_owned = key.to_string();
        let tx = self.tx.clone();
        thread::Builder::new()
            .name("file-save".to_string())
            .spawn(move || {
                let msg = match job() {
                    Ok(path) => BgResult::Downloaded { url: key_owned, path },
                    Err(err) => BgResult::DownloadFailed { url: key_owned, err },
                };
                let _ = tx.send(msg);
            })
            .ok();
    }

    /// Whether a `save_with` job for `key` is still running.
    pub fn is_saving(&self, key: &str) -> bool {
        self.saving.contains(key)
    }

    /// Whether the last `save_with` job for `key` failed.
    pub fn save_failed(&self, key: &str) -> bool {
        self.errors.contains_key(&format!("dl:{key}"))
    }

    /// Put an already decoded picture under `key`, for the snapshot fixtures,
    /// which have no server to fetch from. Never used by the app itself, which
    /// must not decode on the frame.
    #[cfg(test)]
    pub fn insert_for_test(&mut self, ctx: &Context, key: &str, bytes: &[u8]) -> Result<(), String> {
        let (width, height, rgba) = decode_bytes(bytes, self.max_pixels)?;
        let _ = self.tx.send(BgResult::Decoded { url: key.to_string(), width, height, rgba });
        self.poll(ctx);
        Ok(())
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
                    self.saving.remove(&url);
                    self.downloads.insert(url, path);
                }
                BgResult::DownloadFailed { url, err } => {
                    self.saving.remove(&url);
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
                // Same cap as the inline fetch: the Download button sits in
                // the viewer for an image that already displayed, so it was
                // already under this limit once.
                let msg = match download_bytes(&url_owned, MAX_IMAGE_BYTES) {
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

/// Largest image file the cache will download, in bytes (16 MB). Chat photos
/// are a few MB; a 4K screenshot PNG is around 10 MB. The cap exists so a
/// page opened in the readable web view (or a hostile chat message) cannot
/// point at a multi-gigabyte "image" and have the app buffer it whole before
/// the decoder ever sees it. `max_pixels` bounds the DECODED texture; this
/// bounds the bytes on the wire, which is a different thing.
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// Blocking HTTP GET via ureq + decode with the `image` crate.
fn fetch_and_decode(url: &str, max_pixels: u32) -> Result<(u32, u32, Vec<u8>), String> {
    decode_bytes(&download_bytes(url, MAX_IMAGE_BYTES)?, max_pixels)
}

/// Decode a picture's file bytes to RGBA pixels, downsampled to `max_pixels`.
fn decode_bytes(bytes: &[u8], max_pixels: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode: {e}"))?;
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

/// Blocking download of raw bytes from a URL, refusing anything over
/// `max_bytes`. Two checks, because a server may lie or say nothing:
///
/// 1. If the response declares a `Content-Length` above the cap, refuse
///    before reading a single body byte.
/// 2. Read through `Read::take(max_bytes + 1)`: if more than `max_bytes`
///    bytes arrive (no Content-Length, or a false one), refuse. The extra
///    byte is how "exceeded" is told apart from "exactly at the cap".
///
/// ureq follows up to five redirects on its own here; it only speaks http
/// and https, so a redirect cannot lead anywhere the readable-web scheme
/// gate would refuse. Production passes [`MAX_IMAGE_BYTES`]; the tests pass
/// a small cap so they can prove the refusal against a loopback server
/// without pushing 16 MB through a socket.
pub(crate) fn download_bytes(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| format!("GET {}: {}", url, e))?;
    if resp.status() != 200 {
        return Err(format!("GET {}: HTTP {}", url, resp.status()));
    }
    // Check 1: an honest server tells us up front.
    if let Some(declared) = resp.header("Content-Length").and_then(|v| v.trim().parse::<usize>().ok()) {
        if declared > max_bytes {
            return Err(format!(
                "GET {url}: image is {declared} bytes, over the {} MB limit",
                max_bytes / (1024 * 1024)
            ));
        }
    }
    // Check 2: the bytes themselves, whatever the header said.
    let mut reader = std::io::Read::take(resp.into_reader(), max_bytes as u64 + 1);
    let mut bytes = Vec::with_capacity(64 * 1024);
    std::io::Read::read_to_end(&mut reader, &mut bytes).map_err(|e| format!("read body: {e}"))?;
    if bytes.len() > max_bytes {
        return Err(format!("GET {url}: image exceeds the {} MB limit", max_bytes / (1024 * 1024)));
    }
    Ok(bytes)
}

#[cfg(test)]
mod download_cap_tests {
    use super::download_bytes;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-shot HTTP/1.1 server on a random loopback port that answers
    /// every request with the given status line, headers and body, then
    /// closes. Returns the URL to fetch. Nothing leaves the machine.
    fn one_shot_server(headers: &'static str, body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Read the request head (until the blank line) so the client
                // has finished sending before we answer.
                let mut buf = [0u8; 4096];
                let mut got = Vec::new();
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    got.extend_from_slice(&buf[..n]);
                    if got.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\n{headers}\r\n").as_bytes());
                // The client may hang up mid-body once its cap trips; that is
                // the point, so a broken pipe here is not an error.
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        format!("http://127.0.0.1:{port}/picture.png")
    }

    #[test]
    fn a_declared_length_over_the_cap_is_refused_before_the_body_is_read() {
        // The body is never sent past the cap; the header alone must refuse.
        let url = one_shot_server("Content-Length: 5000\r\nConnection: close\r\n", vec![b'x'; 5000]);
        let err = download_bytes(&url, 1024).expect_err("5000 declared bytes over a 1024 cap");
        assert!(err.contains("5000 bytes") && err.contains("limit"), "{err}");
    }

    #[test]
    fn a_body_that_streams_past_the_cap_with_no_length_is_refused() {
        // No Content-Length: HTTP/1.1 with Connection: close means "read until
        // the server hangs up", the shape a hostile server would use to hide
        // its size. The take() cap must still stop it.
        let url = one_shot_server("Connection: close\r\n", vec![b'y'; 3000]);
        let err = download_bytes(&url, 1024).expect_err("3000 streamed bytes over a 1024 cap");
        assert!(err.contains("exceeds") && err.contains("limit"), "{err}");
    }

    #[test]
    fn a_body_within_the_cap_comes_through_whole() {
        let url = one_shot_server("Content-Length: 1000\r\nConnection: close\r\n", vec![b'z'; 1000]);
        let bytes = download_bytes(&url, 1024).expect("1000 bytes under a 1024 cap");
        assert_eq!(bytes.len(), 1000);
        assert!(bytes.iter().all(|&b| b == b'z'));
    }

    #[test]
    fn exactly_at_the_cap_is_allowed_one_over_is_not() {
        let url = one_shot_server("Content-Length: 1024\r\nConnection: close\r\n", vec![b'a'; 1024]);
        assert_eq!(download_bytes(&url, 1024).expect("exactly the cap is fine").len(), 1024);
        let url = one_shot_server("Connection: close\r\n", vec![b'b'; 1025]);
        assert!(download_bytes(&url, 1024).is_err(), "one byte over the cap must be refused");
    }
}

/// The start of every `ImageCache` key that names a file encrypted for a
/// private conversation rather than a URL. The image viewer reads it to leave
/// out "Download" and "Copy URL", which would fetch or show the ciphertext.
pub const PRIVATE_KEY_PREFIX: &str = "private-file:";

/// Which pictures named in a message load by themselves (2026-10-09, defect
/// 3.7.6 of docs/design/blocking-and-safe-mode.md): the server's own, and no
/// other website's.
#[cfg(test)]
mod route_tests {
    use super::{extract_image_urls, route_image, ImageRoute};

    const OWN: &str = "https://united-humanity.us";

    fn asks(raw: &str, server: &str) -> String {
        match route_image(raw, server) {
            ImageRoute::AskFirst { host, .. } => host,
            ImageRoute::Load(url) => panic!("{raw} would load by itself from {url}"),
        }
    }

    /// Seen red 2026-10-09 with the own-server branch of `route_image` switched
    /// off (every picture asked first): the first assertion, `/uploads/a.png`
    /// on our own server, failed.
    #[test]
    fn the_servers_own_pictures_load_by_themselves() {
        let own = "https://united-humanity.us/uploads/a.png";
        assert_eq!(route_image("/uploads/a.png", OWN), ImageRoute::Load(own.into()));
        assert_eq!(route_image(own, OWN), ImageRoute::Load(own.into()));
        assert_eq!(route_image(own, "https://united-humanity.us/"), ImageRoute::Load(own.into()));
        // Host names are not case sensitive.
        assert!(matches!(route_image("https://United-Humanity.US/uploads/a.png", OWN), ImageRoute::Load(_)));
        // The socket address the app may hold instead, and a bare host.
        assert_eq!(route_image("/uploads/a.png", "wss://united-humanity.us/ws"), ImageRoute::Load(own.into()));
        assert_eq!(route_image("/uploads/a.png", "united-humanity.us"), ImageRoute::Load(own.into()));
        // A local server on its own port.
        assert_eq!(
            route_image("/uploads/a.png", "http://127.0.0.1:3210"),
            ImageRoute::Load("http://127.0.0.1:3210/uploads/a.png".into())
        );
        // A path that starts with two slashes is still a path on our server.
        assert!(matches!(route_image("//tracker.example/a.png", OWN), ImageRoute::Load(_)));
    }

    /// Seen red 2026-10-09 with `route_image` loading every picture, as the
    /// chat did before: "https://tracker.example/pixel.png would load by itself
    /// from https://tracker.example/pixel.png".
    #[test]
    fn any_other_website_waits_for_a_click_and_is_named() {
        assert_eq!(asks("https://tracker.example/pixel.png", OWN), "tracker.example");
        assert_eq!(asks("http://tracker.example/pixel.png", OWN), "tracker.example");
        // Lookalikes: a subdomain, another port, our name as a login part.
        assert_eq!(asks("https://cdn.united-humanity.us/a.png", OWN), "cdn.united-humanity.us");
        assert_eq!(asks("https://united-humanity.us:8443/a.png", OWN), "united-humanity.us:8443");
        assert_eq!(asks("https://united-humanity.us@tracker.example/a.png", OWN), "tracker.example");
        assert_eq!(asks("https://tracker.example\\@united-humanity.us/a.png", OWN), "tracker.example");
        assert_eq!(asks("https://united-humanity.us.tracker.example/a.png", OWN), "united-humanity.us.tracker.example");
        // Another server's picture is another website to a reader on this one.
        assert_eq!(asks("https://united-humanity.us/uploads/a.png", "http://127.0.0.1:3210"), "united-humanity.us");
        // Nothing the fetcher could load still never loads by itself.
        assert_eq!(asks("ftp://tracker.example/a.png", OWN), "another website");
    }

    /// Seen red 2026-10-09 both ways: with every picture loaded (the tracker's
    /// came back `Load`) and with every picture asked first (our own came
    /// back `AskFirst`).
    #[test]
    fn a_message_with_both_kinds_routes_each_picture_on_its_own() {
        let text = "look /uploads/cat.jpg and https://tracker.example/x.gif";
        let routes: Vec<ImageRoute> = extract_image_urls(text).iter().map(|u| route_image(u, OWN)).collect();
        assert_eq!(routes.len(), 2, "{routes:?}");
        assert!(matches!(routes[0], ImageRoute::Load(_)), "{routes:?}");
        assert!(matches!(&routes[1], ImageRoute::AskFirst { host, .. } if host == "tracker.example"), "{routes:?}");
    }
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

/// Where a picture named in a chat message may be loaded from (2026-10-09,
/// defect 3.7.6 of docs/design/blocking-and-safe-mode.md). Fetching a picture
/// shows the website that serves it this device's network address, and anyone
/// can post a link to a server they run to collect the addresses of everyone
/// who reads the message. The server the message came through already sees the
/// address, so its own pictures load straight away; any other website's wait
/// for a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRoute {
    /// Served by the person's own server: load it now.
    Load(String),
    /// Served by another website, named by `host`: show the click-to-load
    /// placeholder first.
    AskFirst { url: String, host: String },
}

impl ImageRoute {
    /// The full URL the picture is fetched from.
    pub fn url(&self) -> &str {
        match self {
            ImageRoute::Load(url) => url,
            ImageRoute::AskFirst { url, .. } => url,
        }
    }
}

/// Decide where a picture named in a message may load from. `raw` is what
/// `extract_image_urls` found; `server_url` is the server the message came
/// through, in whatever form the app holds it (`https://host`, `wss://host/ws`,
/// a bare `host:port`). A picture counts as the server's own only when its host
/// AND port are the server's: a subdomain, another port or a lookalike host is
/// another website. Hosts are compared after the URL parser that does the
/// fetching has read them, so `https://own.example@other.example/` is
/// `other.example`, as the fetch would see it.
pub fn route_image(raw: &str, server_url: &str) -> ImageRoute {
    let base = http_base(server_url);
    let url = resolve_url(raw, &base);
    let picture = url::Url::parse(&url)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"));
    let Some(picture) = picture else {
        // Nothing the fetcher could load, and nothing to name.
        return ImageRoute::AskFirst { url, host: "another website".to_string() };
    };
    let own = origin_of(&base);
    let theirs = picture
        .host_str()
        .map(|h| h.to_ascii_lowercase())
        .zip(picture.port_or_known_default());
    if own.is_some() && own == theirs {
        return ImageRoute::Load(url);
    }
    let host = match (picture.host_str(), picture.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        (None, _) => "another website".to_string(),
    };
    ImageRoute::AskFirst { url, host }
}

/// The web address a server's own files are served under: `wss://host/ws`
/// becomes `https://host`, `ws://` becomes `http://`, a bare `host:port`
/// becomes `https://host:port`, the same way the WebRTC manager's
/// `relay_base` is derived from the socket address in src/lib.rs.
fn http_base(server_url: &str) -> String {
    let s = server_url.trim().trim_end_matches('/');
    let s = s.strip_suffix("/ws").unwrap_or(s);
    if let Some(rest) = s.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = s.strip_prefix("ws://") {
        format!("http://{rest}")
    } else if s.starts_with("https://") || s.starts_with("http://") || s.is_empty() {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

/// A server's host (lower case) and port, or `None` when it has no host.
fn origin_of(base: &str) -> Option<(String, u16)> {
    let u = url::Url::parse(base).ok()?;
    Some((u.host_str()?.to_ascii_lowercase(), u.port_or_known_default()?))
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
