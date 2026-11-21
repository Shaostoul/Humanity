//! Rung 2: the pack's pictures, drawn by resvg (2026-11-20, docs/design/emoji.md).
//!
//! Each emoji the text shows is drawn from its SVG in `assets/emoji/<set>/` (the same files the
//! website serves: `scripts/sync-web-root.sh` copies `assets/` to the web root) at the exact pixel
//! size the line asks for (`inline::with_pictures`: 1.2 em times the screen's scale), into an egui
//! texture. resvg with usvg and tiny-skia was chosen and its licences quoted in
//! `docs/reference/findings/2026-11-20-svg-rasteriser.md`. The desktop never fetches a picture from
//! another site: the pack ships beside the exe (`scripts/lib/release-files.js`).
//!
//! REUSE (CLAUDE.md, the no-caches rule). A drawn picture is kept IN MEMORY ONLY, keyed by the
//! picture, its pixel size (which already carries the theme's font scale and the screen's scale) and
//! the pack's version, so it is drawn once per emoji and size, never per frame. The version is bumped
//! by a watcher of `assets/emoji/` on any change there, which drops every kept picture and reloads the
//! list (`inline::source`), so a kept picture cannot be older than its file. Nothing is written to
//! disk. With the Dev page's "Turn off all reuse" (`HUMANITY_NO_REUSE=1`) every ask reads and draws
//! the file again, and the list is read again on a short schedule (`list_due`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use egui::mutex::RwLock;
use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use humanity_core::hot_reload::watcher::FileWatcher;

use super::inline::{install, EmojiPictures, EmojiSource};
use super::EmojiIndex;

/// The pack's folder beside the exe (`assets/emoji/`).
pub fn pack_dir() -> PathBuf {
    humanity_core::storage::assets_dir().join("emoji")
}

/// Draw one SVG `px` pixels square, its picture scaled to fit and centred (as a browser draws an
/// `<img>` of it in a square box), into an image egui can upload.
pub fn rasterise(svg: &[u8], px: u32) -> Result<ColorImage, String> {
    use resvg::{tiny_skia, usvg};
    let px = px.clamp(1, 1024);
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(|e| format!("not a picture: {e}"))?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w > 0.0 && h > 0.0) {
        return Err("the picture has no size".into());
    }
    let scale = px as f32 / w.max(h);
    let dx = (px as f32 - w * scale) / 2.0;
    let dy = (px as f32 - h * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(px, px).ok_or("no room for the picture")?;
    resvg::render(&tree, tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, dx, dy), &mut pixmap.as_mut());
    // tiny-skia's pixels are premultiplied RGBA, as egui's texture wants them.
    Ok(ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data()))
}

/// How many times the pack's files changed this run: the version every kept picture and the
/// installed list carry. Bumped by the watcher of `assets/emoji/` (`watch`).
static PACK_VERSION: AtomicU64 = AtomicU64::new(0);
/// reuse: the watcher that bumps `PACK_VERSION`, started once a run; it keeps nothing it worked out.
static WATCHER: OnceLock<Option<Mutex<FileWatcher>>> = OnceLock::new();

/// Start watching the pack's folder (once a run; a second call does nothing).
fn watch(dir: &Path) {
    WATCHER.get_or_init(|| match FileWatcher::new(dir.to_path_buf()) {
        Ok(w) => Some(Mutex::new(w)),
        Err(e) => {
            // Without a watcher a changed picture would stay as it was drawn: say so once.
            log::warn!("Emoji: cannot watch {} ({e}); pictures changed there show after a restart, or at once with Dev > Turn off all reuse", dir.display());
            None
        }
    });
}

/// The pack's version now, after taking in any change the watcher saw (never waits for it).
pub fn pack_version() -> u64 {
    if let Some(Some(w)) = WATCHER.get() {
        if let Ok(w) = w.try_lock() {
            if !w.poll_changes().is_empty() {
                PACK_VERSION.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    PACK_VERSION.load(Ordering::Relaxed)
}

/// What an installed list was read from: the pack's version then, and when.
#[derive(Debug, Clone, Copy)]
pub struct PackStamp {
    pub version: u64,
    pub read_at: Instant,
}

/// With the reuse switch off, how old the installed list may get before it is read again (seconds).
/// Not every ask: the list is read for every line of text, and reading it costs milliseconds.
pub const LIST_REUSE_OFF_S: f32 = 1.0;

/// Is the installed list read `age_s` seconds ago at `stamp` due to be read again? When the pack
/// changed since, and with reuse off (`reuse` false) once it is older than `LIST_REUSE_OFF_S`.
pub fn list_due(stamp: &PackStamp, version: u64, reuse: bool, age_s: f32) -> bool {
    stamp.version != version || (!reuse && age_s >= LIST_REUSE_OFF_S)
}

/// One kept picture: its texture (None when the file could not be drawn, so it is not read again
/// every frame), the pack version it was drawn at, and when it was last asked for.
struct Kept {
    texture: Option<TextureHandle>,
    version: u64,
    used: Instant,
}

/// Above this many kept pictures, the ones not asked for in `IDLE_S` are let go (each is a few KB;
/// sizes pile up when the Font Size or the window's scale changes).
const KEEP_MAX: usize = 2048;
const IDLE_S: f32 = 30.0;

/// The pack's pictures, drawn on first sight at each size: rung 3's [`EmojiPictures`].
pub struct PackPictures {
    dir: PathBuf,
    textures: Arc<RwLock<egui::epaint::TextureManager>>,
    /// reuse: site: each drawn picture, keyed by set, file and pixel size and tagged with the pack's
    /// version; with the switch off every ask reads and draws its file again.
    kept: Mutex<HashMap<(String, String, u32), Kept>>,
    /// How many pictures were drawn (read and rasterised), for the tests and the Dev page's counters.
    drawn: AtomicUsize,
}

impl PackPictures {
    /// Pictures from the set folders under `dir`, uploaded through `textures` (a context's own).
    pub fn new(dir: PathBuf, textures: Arc<RwLock<egui::epaint::TextureManager>>) -> PackPictures {
        PackPictures { dir, textures, kept: Mutex::new(HashMap::new()), drawn: AtomicUsize::new(0) }
    }

    /// How many pictures were read and rasterised so far.
    pub fn drawn(&self) -> usize {
        self.drawn.load(Ordering::Relaxed)
    }

    /// The picture's texture, drawn now unless one kept at this size and pack `version` may be used
    /// (`reuse` is `humanity_core::reuse::enabled()` at the trait's call, a parameter so a test drives
    /// both arms without flipping the switch for every test).
    pub fn picture_with(&self, set: &str, file: &str, px: u32, reuse: bool, version: u64) -> Option<TextureId> {
        let key = (set.to_string(), file.to_string(), px);
        let now = Instant::now();
        let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = kept.get_mut(&key) {
            if humanity_core::reuse::keeps(Some(k.version), version, reuse) {
                k.used = now;
                return k.texture.as_ref().map(|t| t.id());
            }
        }
        let texture = self.draw(set, file, px);
        let id = texture.as_ref().map(|t| t.id());
        if kept.len() >= KEEP_MAX {
            kept.retain(|_, k| now.duration_since(k.used).as_secs_f32() < IDLE_S && k.version == version);
        }
        // Replacing an entry drops its old texture: egui frees it after this frame is painted.
        kept.insert(key, Kept { texture, version, used: now });
        id
    }

    /// Read and rasterise one picture into a new texture; None (logged) when it cannot be drawn,
    /// and the emoji stays the font's glyph.
    fn draw(&self, set: &str, file: &str, px: u32) -> Option<TextureHandle> {
        self.drawn.fetch_add(1, Ordering::Relaxed);
        // Names come from the pack's own list; refuse anything that could leave its folder.
        if [set, file].iter().any(|s| s.is_empty() || s.contains(['/', '\\', '.'])) {
            return None;
        }
        let path = self.dir.join(set).join(format!("{file}.svg"));
        let image = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|svg| rasterise(&svg, px));
        match image {
            Ok(image) => {
                let id = self.textures.write().alloc(format!("emoji {set}/{file} {px}px"), image.into(), TextureOptions::LINEAR);
                Some(TextureHandle::new(self.textures.clone(), id))
            }
            Err(e) => {
                log::warn!("Emoji: {} could not be drawn ({e}); it shows as the font's glyph", path.display());
                None
            }
        }
    }
}

impl EmojiPictures for PackPictures {
    fn picture(&self, set: &str, file: &str, px: u32) -> Option<TextureId> {
        self.picture_with(set, file, px, humanity_core::reuse::enabled(), pack_version())
    }
}

/// The pack's list and pictures for `ctx` from the pack folder `dir`, or why not.
pub fn pack_source(ctx: &egui::Context, dir: &Path) -> Result<EmojiSource, String> {
    let version = pack_version();
    let index = EmojiIndex::load(&dir.join("index.json"))?;
    let pictures = PackPictures::new(dir.to_path_buf(), ctx.tex_manager());
    Ok(EmojiSource { index, pictures: Box::new(pictures), pack: Some(PackStamp { version, read_at: Instant::now() }) })
}

/// Give `ctx` the shipped pack's pictures (called for every egui context through
/// `gui::install_fonts`, so a wall screen draws the emoji the main window does). Without the pack
/// (a build that did not carry `assets/emoji/`) nothing is installed and every emoji stays the
/// system font's glyph, as before rung 2: said once in the log.
pub fn install_pack(ctx: &egui::Context) {
    let dir = pack_dir();
    watch(&dir);
    match pack_source(ctx, &dir) {
        Ok(source) => install(ctx, Some(Arc::new(source))),
        Err(e) => {
            static SAID: std::sync::Once = std::sync::Once::new();
            SAID.call_once(|| log::warn!("Emoji: no pictures ({e}); emoji show as the system font's glyphs"));
        }
    }
}

/// Is the installed list read at `stamp` due to be read again now (`list_due` with the switch and
/// the clock)? Asked by `inline::source` for a source that came from the pack.
pub(super) fn due(stamp: &PackStamp) -> bool {
    list_due(stamp, pack_version(), humanity_core::reuse::enabled(), stamp.read_at.elapsed().as_secs_f32())
}

/// Read the pack's list again and install it with fresh pictures (the old ones are let go); None,
/// and nothing installed, when it cannot be read.
pub(super) fn reload(ctx: &egui::Context) -> Option<Arc<EmojiSource>> {
    match pack_source(ctx, &pack_dir()) {
        Ok(source) => {
            let source = Arc::new(source);
            install(ctx, Some(source.clone()));
            Some(source)
        }
        Err(e) => {
            log::warn!("Emoji: the pack's list could not be read again ({e}); emoji show as the system font's glyphs");
            install(ctx, None);
            None
        }
    }
}
