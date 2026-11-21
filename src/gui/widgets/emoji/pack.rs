//! Rung 2: the pack's pictures, drawn by resvg (2026-11-20, docs/design/emoji.md).
//!
//! Each emoji the text shows is drawn from its SVG in `assets/emoji/<set>/` (the same files the
//! website serves: `scripts/sync-web-root.sh` copies `assets/` to the web root) at the exact pixel
//! size the line asks for (`inline::with_pictures`: 1.2 em times the screen's scale), into an egui
//! texture. resvg with usvg and tiny-skia was chosen and its licences quoted in
//! `docs/reference/findings/2026-11-20-svg-rasteriser.md`. The desktop never fetches a picture from
//! another site: the pack ships beside the exe (`scripts/lib/release-files.js`).
//!
//! THE OUTLINE (2026-11-20, `outline.rs`): each picture is drawn with the theme's outline laid
//! beneath it (on the dark theme, a faint light edge so OpenMoji's black lines still show), so it
//! costs nothing per frame.
//!
//! REUSE (CLAUDE.md, the no-caches rule). A drawn picture is kept IN MEMORY ONLY, keyed by the
//! picture, its pixel size (which already carries the theme's font scale and the screen's scale), its
//! outline (so a theme change draws it again) and the pack's version, so it is drawn once per emoji,
//! size and outline, never per frame. The version is bumped
//! by a watcher of `assets/emoji/` on any change there, which drops every kept picture and reloads the
//! list (`inline::source`), so a kept picture cannot be older than its file. Nothing is written to
//! disk. The reaction panel's pictures are drawn ahead by a worker, a few at a time (`warm.rs`).
//! With the Dev page's "Turn off all reuse" (`HUMANITY_NO_REUSE=1`) every ask reads and draws
//! the file again, and the list is read again on a short schedule (`list_due`).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use egui::mutex::RwLock;
use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use humanity_core::hot_reload::watcher::FileWatcher;

use super::inline::{install, EmojiPictures, EmojiSource};
use super::outline::{self, Outline};
use super::EmojiIndex;

/// The pack's folder beside the exe (`assets/emoji/`).
pub fn pack_dir() -> PathBuf {
    humanity_core::storage::assets_dir().join("emoji")
}

/// Draw one SVG `px` pixels square, its picture scaled to fit and centred (as a browser draws an
/// `<img>` of it in a square box), with `outline` laid beneath it (`outline::apply`; nothing for
/// `Outline::NONE`), into an image egui can upload.
pub fn rasterise(svg: &[u8], px: u32, outline: Outline) -> Result<ColorImage, String> {
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
    outline::apply(pixmap.data_mut(), px as usize, px as usize, outline);
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

/// A kept picture's key: set folder, file name, pixel size and outline (`outline.rs`), so a new
/// size or a theme change draws the picture again.
type Key = (String, String, u32, Outline);

/// One picture to draw ahead of need (`warm.rs`): the parts of its key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WarmKey {
    pub set: String,
    pub file: String,
    pub px: u32,
    pub outline: Outline,
}

impl WarmKey {
    fn key(&self) -> Key {
        (self.set.clone(), self.file.clone(), self.px, self.outline)
    }
}

/// A picture the worker drew ahead, waiting to become a texture on a frame (None: it could not
/// be drawn, kept as "none" like a failed draw).
struct Waiting {
    key: Key,
    version: u64,
    image: Option<ColorImage>,
}

/// What every handle on one context's pictures shares: the frame's and the worker's.
struct Shared {
    dir: PathBuf,
    textures: Arc<RwLock<egui::epaint::TextureManager>>,
    /// reuse: site: each drawn picture, keyed by set, file, pixel size and outline and tagged with
    /// the pack's version; with the switch off every ask reads and draws its file again.
    kept: Mutex<HashMap<Key, Kept>>,
    /// reuse: site: pictures drawn ahead (`warm.rs`), each tagged with the pack's version, waiting
    /// for a frame to upload them; emptied, and nothing drawn ahead, with the switch off
    /// (`stop_ahead`), and one drawn for a pack that changed since is let go.
    waiting: Mutex<VecDeque<Waiting>>,
    /// reuse: site: the pack version and sizes the worker was last handed a list for, so the list
    /// is not made again every frame; cleared with the switch off (`stop_ahead`).
    asked: Mutex<Option<(u64, [u32; 5])>>,
    /// Bumped for each new list (and to stop): a worker drawing an older one stops.
    generation: AtomicU64,
    /// How many pictures were drawn on the asking thread (the frame), and by the worker ahead.
    drawn: AtomicUsize,
    drawn_ahead: AtomicUsize,
    /// When these pictures were installed (start-up, or the pack read again).
    installed: Instant,
}

/// The pack's pictures, drawn on first sight at each size and outline: rung 3's [`EmojiPictures`].
/// A handle: clones share the same pictures (the worker that draws ahead holds one).
#[derive(Clone)]
pub struct PackPictures {
    shared: Arc<Shared>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl PackPictures {
    /// Pictures from the set folders under `dir`, uploaded through `textures` (a context's own).
    pub fn new(dir: PathBuf, textures: Arc<RwLock<egui::epaint::TextureManager>>) -> PackPictures {
        PackPictures {
            shared: Arc::new(Shared {
                dir,
                textures,
                kept: Mutex::new(HashMap::new()),
                waiting: Mutex::new(VecDeque::new()),
                asked: Mutex::new(None),
                generation: AtomicU64::new(0),
                drawn: AtomicUsize::new(0),
                drawn_ahead: AtomicUsize::new(0),
                installed: Instant::now(),
            }),
        }
    }

    /// How many pictures were read and rasterised on the asking thread (the frame) so far.
    pub fn drawn(&self) -> usize {
        self.shared.drawn.load(Ordering::Relaxed)
    }

    /// How many pictures the worker drew ahead of need so far (`warm.rs`).
    pub fn drawn_ahead(&self) -> usize {
        self.shared.drawn_ahead.load(Ordering::Relaxed)
    }

    /// Seconds since these pictures were installed.
    pub fn age_s(&self) -> f32 {
        self.shared.installed.elapsed().as_secs_f32()
    }

    /// The picture's texture, drawn now unless one kept at this size, outline and pack `version`
    /// may be used, or one drawn ahead is waiting (`reuse` is `humanity_core::reuse::enabled()` at
    /// the trait's call, a parameter so a test drives both arms without flipping the switch for
    /// every test).
    pub fn picture_with(&self, set: &str, file: &str, px: u32, outline: Outline, reuse: bool, version: u64) -> Option<TextureId> {
        let key: Key = (set.to_string(), file.to_string(), px, outline);
        let now = Instant::now();
        let mut kept = lock(&self.shared.kept);
        if let Some(k) = kept.get_mut(&key) {
            if humanity_core::reuse::keeps(Some(k.version), version, reuse) {
                k.used = now;
                return k.texture.as_ref().map(|t| t.id());
            }
        }
        // A picture the worker drew ahead and the frame has not uploaded yet: used, not drawn again.
        let ahead = if reuse { self.take_waiting(&key, version) } else { None };
        let texture = match ahead {
            Some(image) => image.map(|image| self.upload(set, file, px, image)),
            None => self.draw(set, file, px, outline),
        };
        let id = texture.as_ref().map(|t| t.id());
        if kept.len() >= KEEP_MAX {
            kept.retain(|_, k| now.duration_since(k.used).as_secs_f32() < IDLE_S && k.version == version);
        }
        // Replacing an entry drops its old texture: egui frees it after this frame is painted.
        kept.insert(key, Kept { texture, version, used: now });
        id
    }

    /// Read and rasterise one picture into a new texture on the asking thread; None (logged) when
    /// it cannot be drawn, and the emoji stays the font's glyph.
    fn draw(&self, set: &str, file: &str, px: u32, outline: Outline) -> Option<TextureHandle> {
        self.shared.drawn.fetch_add(1, Ordering::Relaxed);
        match self.shared.read_and_draw(set, file, px, outline) {
            Ok(image) => Some(self.upload(set, file, px, image)),
            Err(e) => {
                log::warn!("Emoji: {set}/{file} could not be drawn ({e}); it shows as the font's glyph");
                None
            }
        }
    }

    /// Turn a drawn picture into a texture of this context.
    fn upload(&self, set: &str, file: &str, px: u32, image: ColorImage) -> TextureHandle {
        let id = self.shared.textures.write().alloc(format!("emoji {set}/{file} {px}px"), image.into(), TextureOptions::LINEAR);
        TextureHandle::new(self.shared.textures.clone(), id)
    }

    /// The drawn-ahead picture for `key` at `version`, taken off the waiting list, if it is there
    /// (Some(None): it could not be drawn).
    fn take_waiting(&self, key: &Key, version: u64) -> Option<Option<ColorImage>> {
        let mut waiting = lock(&self.shared.waiting);
        let at = waiting.iter().position(|w| w.key == *key && w.version == version)?;
        waiting.remove(at).map(|w| w.image)
    }

    /// Is the picture kept at pack `version`, ready to draw with no work?
    pub fn is_kept(&self, key: &WarmKey, version: u64) -> bool {
        lock(&self.shared.kept).get(&key.key()).is_some_and(|k| k.version == version)
    }

    /// Hand the worker `list()`'s pictures to draw ahead, unless it was already handed the list for
    /// this pack `version` and these sizes (`asked`, what the list depends on). True when a new
    /// list was handed over. The worker draws one picture at a time with a rest after each
    /// (`warm::GAP_MS`); nothing here waits for it.
    pub fn ensure_ahead(&self, version: u64, asked: [u32; 5], list: impl FnOnce() -> Vec<WarmKey>) -> bool {
        {
            let mut last = lock(&self.shared.asked);
            if *last == Some((version, asked)) {
                return false;
            }
            *last = Some((version, asked));
        }
        let generation = {
            let mut waiting = lock(&self.shared.waiting);
            waiting.clear();
            self.shared.generation.fetch_add(1, Ordering::Relaxed) + 1
        };
        let wanted: Vec<WarmKey> = list().into_iter().filter(|k| !self.is_kept(k, version)).collect();
        if wanted.is_empty() {
            return true;
        }
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new().name("emoji-ahead".into()).spawn(move || shared.draw_ahead(generation, version, wanted));
        if let Err(e) = spawned {
            // Nothing breaks: each picture is drawn when it is first asked for, as before.
            log::warn!("Emoji: could not start drawing the reaction pictures ahead ({e}); they are drawn when first shown");
        }
        true
    }

    /// Upload at most `max` of the pictures the worker finished, stopping once `budget` of this
    /// frame is spent; returns how many became textures. One drawn for an older pack is let go.
    pub fn upload_ahead(&self, version: u64, max: usize, budget: std::time::Duration) -> usize {
        let started = Instant::now();
        let mut n = 0;
        while n < max && started.elapsed() < budget {
            let Some(w) = lock(&self.shared.waiting).pop_front() else { break };
            if w.version != version {
                continue;
            }
            let mut kept = lock(&self.shared.kept);
            if kept.get(&w.key).is_some_and(|k| k.version == version) {
                continue; // asked for on a frame meanwhile and drawn then
            }
            let texture = w.image.map(|image| self.upload(&w.key.0, &w.key.1, w.key.2, image));
            kept.insert(w.key, Kept { texture, version, used: Instant::now() });
            n += 1;
        }
        n
    }

    /// Stop drawing ahead and let go of what waits (the reuse switch off).
    pub fn stop_ahead(&self) {
        let mut asked = lock(&self.shared.asked);
        let mut waiting = lock(&self.shared.waiting);
        if asked.is_none() && waiting.is_empty() {
            return;
        }
        *asked = None;
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        waiting.clear();
    }

    /// How many drawn-ahead pictures wait for a frame to upload them (for the tests).
    #[cfg(test)]
    pub fn waiting_len(&self) -> usize {
        lock(&self.shared.waiting).len()
    }
}

impl Shared {
    /// Read and rasterise one picture with its outline, on whichever thread asks.
    fn read_and_draw(&self, set: &str, file: &str, px: u32, outline: Outline) -> Result<ColorImage, String> {
        // Names come from the pack's own list; refuse anything that could leave its folder.
        if [set, file].iter().any(|s| s.is_empty() || s.contains(['/', '\\', '.'])) {
            return Err("not a name in the pack".into());
        }
        let path = self.dir.join(set).join(format!("{file}.svg"));
        std::fs::read(&path).map_err(|e| e.to_string()).and_then(|svg| rasterise(&svg, px, outline))
    }

    /// The worker: draw each picture of `wanted` that is not kept yet, one at a time, resting
    /// `warm::GAP_MS` after each, until the list ends, a newer list replaces it (`generation`) or
    /// the pack changes. Holds a lock only to look and to hand a picture over, never while drawing.
    fn draw_ahead(&self, generation: u64, version: u64, wanted: Vec<WarmKey>) {
        let current = |s: &Shared| s.generation.load(Ordering::Relaxed) == generation && PACK_VERSION.load(Ordering::Relaxed) == version;
        for want in wanted {
            if !current(self) {
                return;
            }
            let key = want.key();
            let have = lock(&self.kept).get(&key).is_some_and(|k| k.version == version);
            if have {
                continue;
            }
            let image = match self.read_and_draw(&want.set, &want.file, want.px, want.outline) {
                Ok(image) => Some(image),
                Err(e) => {
                    log::warn!("Emoji: {}/{} could not be drawn ({e}); it shows as the font's glyph", want.set, want.file);
                    None
                }
            };
            self.drawn_ahead.fetch_add(1, Ordering::Relaxed);
            {
                // Looked at under the waiting list's lock, which a new list or a stop also holds
                // while it bumps the generation, so nothing stale is handed over after them.
                let mut waiting = lock(&self.waiting);
                if !current(self) {
                    return;
                }
                waiting.push_back(Waiting { key, version, image });
            }
            std::thread::sleep(std::time::Duration::from_millis(super::warm::GAP_MS));
        }
    }
}

impl EmojiPictures for PackPictures {
    fn picture(&self, set: &str, file: &str, px: u32, outline: Outline) -> Option<TextureId> {
        self.picture_with(set, file, px, outline, humanity_core::reuse::enabled(), pack_version())
    }

    fn as_pack(&self) -> Option<&PackPictures> {
        Some(self)
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
