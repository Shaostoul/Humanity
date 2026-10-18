//! Pictures on this computer shown small, for the file picker's Pictures view (2026-10-17), and
//! videos by one of their frames where the computer's own decoders can give one (Windows, 2026-10-18,
//! `media::system::still_of`): each is read and shrunk on a worker thread, never on the frame, then kept as a texture while it is in
//! use. Only what is on screen is asked for; at most [`WORKERS`] are read at once, the rest wait
//! their turn; at most [`KEPT`] are kept, the longest unused going first. A file over
//! [`MAX_BYTES`], or one the image reader cannot read, is drawn as its type instead.
//!
//! Kept per thread (the picker draws on the main thread; each test thread has its own).

use egui::{Context, TextureHandle, TextureId, Vec2};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// How many pictures are read at once.
pub const WORKERS: usize = 3;
/// How many shrunk pictures are kept.
pub const KEPT: usize = 120;
/// The largest file read for a preview.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// The sizes a preview is shrunk to (its longer side, in pixels): the smallest that covers the
/// space it is drawn in, so one picture across reads a large one and eight across small ones.
const SIZES: [u32; 5] = [128, 256, 512, 1024, 2048];

/// The picture kinds read for a preview: what the image reader built into the app reads.
pub fn is_picture(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"].iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// The video kinds a preview is asked of the computer's own decoders for.
fn is_video(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv"].iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Whether a file gets a preview: a picture, or a video where this computer can decode one of its
/// frames.
pub fn is_previewable(name: &str) -> bool {
    is_picture(name) || (is_video(name) && video_stills_here())
}

/// Whether this computer can give a video's frame (Windows with Media Foundation).
fn video_stills_here() -> bool {
    #[cfg(windows)]
    return crate::media::system::available();
    #[cfg(not(windows))]
    false
}

/// The size a preview `px` pixels across is read at.
pub fn size_for(px: f32) -> u32 {
    SIZES.iter().copied().find(|s| *s as f32 >= px).unwrap_or(SIZES[SIZES.len() - 1])
}

enum Slot {
    Waiting,
    Reading,
    Ready { tex: TextureHandle, size: Vec2, used: u64 },
    Failed,
}

type Key = (PathBuf, u32);
type Shrunk = (Key, Option<(u32, u32, Vec<u8>)>);

struct Thumbs {
    slots: HashMap<Key, Slot>,
    queue: VecDeque<Key>,
    reading: usize,
    tx: mpsc::Sender<Shrunk>,
    rx: mpsc::Receiver<Shrunk>,
    frame: u64,
}

impl Default for Thumbs {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Thumbs { slots: HashMap::new(), queue: VecDeque::new(), reading: 0, tx, rx, frame: 0 }
    }
}

thread_local! {
    static THUMBS: RefCell<Thumbs> = RefCell::new(Thumbs::default());
}

/// Read `path` and shrink it to fit `size` square, on a worker. None when it cannot be.
fn shrink(path: &Path, size: u32) -> Option<(u32, u32, Vec<u8>)> {
    // A video: one of its frames, through the computer's own decoders (any size of file: only the
    // frames up to it are read).
    #[cfg(windows)]
    if is_video(&path.file_name()?.to_string_lossy()) {
        let frame = crate::media::system::still_of(path)?;
        let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?;
        let small = image::DynamicImage::ImageRgba8(img).thumbnail(size, size).to_rgba8();
        return Some((small.width(), small.height(), small.into_raw()));
    }
    let len = std::fs::metadata(path).ok()?.len();
    if len > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let small = img.thumbnail(size, size).to_rgba8();
    Some((small.width(), small.height(), small.into_raw()))
}

impl Thumbs {
    /// Start reading what waits, while fewer than [`WORKERS`] are being read.
    fn start_waiting(&mut self) {
        while self.reading < WORKERS {
            let Some(key) = self.queue.pop_front() else { return };
            if !matches!(self.slots.get(&key), Some(Slot::Waiting)) {
                continue;
            }
            self.slots.insert(key.clone(), Slot::Reading);
            self.reading += 1;
            let tx = self.tx.clone();
            let started = std::thread::Builder::new().name("thumbnail".into()).spawn(move || {
                let shrunk = shrink(&key.0, key.1);
                let _ = tx.send((key, shrunk));
            });
            if started.is_err() {
                self.reading -= 1;
            }
        }
    }

    /// Take what the workers finished: each a texture, or a failure drawn as its type.
    fn take_finished(&mut self, ctx: &Context) {
        while let Ok((key, shrunk)) = self.rx.try_recv() {
            self.reading = self.reading.saturating_sub(1);
            let slot = match shrunk {
                Some((w, h, rgba)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    let tex = ctx.load_texture(format!("thumb:{}:{}", key.0.display(), key.1), image, egui::TextureOptions::LINEAR);
                    Slot::Ready { tex, size: Vec2::new(w as f32, h as f32), used: self.frame }
                }
                None => Slot::Failed,
            };
            self.slots.insert(key, slot);
        }
        // Keep at most KEPT: the longest unused go (their textures are freed with them).
        let ready = self.slots.values().filter(|s| matches!(s, Slot::Ready { .. })).count();
        if ready > KEPT {
            let mut used: Vec<(u64, Key)> = self
                .slots
                .iter()
                .filter_map(|(k, s)| if let Slot::Ready { used, .. } = s { Some((*used, k.clone())) } else { None })
                .collect();
            used.sort_by_key(|(u, _)| *u);
            for (_, k) in used.into_iter().take(ready - KEPT) {
                self.slots.remove(&k);
            }
        }
    }
}

/// The preview of the picture at `path`, about `px` pixels across: its texture and its size once
/// read, None while it is read (asked for here) or when it cannot be.
pub fn get(ctx: &Context, path: &Path, px: f32) -> Option<(TextureId, Vec2)> {
    THUMBS.with(|t| {
        let mut t = t.borrow_mut();
        t.take_finished(ctx);
        let key = (path.to_path_buf(), size_for(px));
        let frame = t.frame;
        let found = match t.slots.get_mut(&key) {
            Some(Slot::Ready { tex, size, used }) => {
                *used = frame;
                Some(Some((tex.id(), *size)))
            }
            Some(_) => Some(None),
            None => None,
        };
        let answer = match found {
            Some(answer) => answer,
            None => {
                t.slots.insert(key.clone(), Slot::Waiting);
                t.queue.push_back(key);
                None
            }
        };
        t.start_waiting();
        if t.reading > 0 || !t.queue.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        answer
    })
}

/// Whether the picture at `path` could not be read for a preview (drawn as its type instead).
pub fn failed(path: &Path, px: f32) -> bool {
    THUMBS.with(|t| matches!(t.borrow().slots.get(&(path.to_path_buf(), size_for(px))), Some(Slot::Failed)))
}

/// Mark a new frame, so what was drawn this frame counts as the most recently used.
pub fn next_frame() {
    THUMBS.with(|t| t.borrow_mut().frame += 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PICTURE IS SHRUNK ON A WORKER AND KEPT; A BROKEN ONE IS DRAWN AS ITS TYPE (2026-10-17): a
    /// real PNG comes back as a texture no larger than the size asked for, keeping its shape; a file
    /// named .png that is not one fails, and only picture kinds are read at all. Seen red 2026-10-17
    /// with `shrink` returning the picture at full size: "no larger than asked".
    #[test]
    fn a_picture_is_shrunk_on_a_worker_and_a_broken_one_fails() {
        let dir = crate::test_temp::dir("thumbs");
        let png = dir.join("wide.png");
        image::RgbaImage::from_fn(400, 200, |x, _| image::Rgba([(x % 255) as u8, 90, 160, 255])).save(&png).unwrap();
        let bad = dir.join("broken.png");
        std::fs::write(&bad, b"not a png").unwrap();
        let ctx = Context::default();
        let mut got = None;
        for _ in 0..400 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                got = get(ctx, &png, 100.0);
                let _ = get(ctx, &bad, 100.0);
            });
            if got.is_some() && failed(&bad, 100.0) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let (_, size) = got.expect("the picture is read");
        assert!(size.x <= 128.0 && size.y <= 128.0, "no larger than asked: {size:?}");
        assert!((size.x / size.y - 2.0).abs() < 0.05, "its shape kept: {size:?}");
        assert!(failed(&bad, 100.0), "a file that is not a picture fails, and is drawn as its type");
        assert!(is_picture("A.JPG") && !is_picture("clip.mp4") && !is_picture("model.glb"));
        assert_eq!((size_for(90.0), size_for(600.0), size_for(5000.0)), (128, 1024, 2048));
    }

    /// A VIDEO IS PREVIEWED BY ONE OF ITS FRAMES (2026-10-18, where Windows decodes it): the MP4
    /// fixture comes back no larger than asked, in its own shape (16:9), and shows the frame a tenth
    /// of the way in: the orange box on navy, not a blank. Seen red 2026-10-18 with videos left out
    /// of `shrink`: "the video's frame is read" (it failed, drawn as its type).
    #[cfg(windows)]
    #[test]
    fn a_video_is_previewed_by_one_of_its_frames() {
        if !crate::media::system::available() {
            println!("skipped: this copy of Windows has no Media Foundation");
            return;
        }
        let mp4 = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media/moving-box-h264-aac.mp4");
        assert!(is_previewable("clip.MP4") && !is_previewable("model.glb"));
        let shrunk = shrink(&mp4, 128);
        let (w, h, rgba) = shrunk.expect("the video's frame is read");
        assert!(w <= 128 && h <= 128 && (w as f32 / h as f32 - 16.0 / 9.0).abs() < 0.1, "no larger than asked, its shape kept: {w}x{h}");
        let orange = rgba.chunks_exact(4).filter(|p| p[0] > 200 && (110..215).contains(&p[1]) && p[2] < 90).count();
        let navy = rgba.chunks_exact(4).filter(|p| p[0] < 80 && p[2] > p[0] && p[2] < 120).count();
        assert!(orange > 20 && navy > orange, "the box on navy: {orange} orange, {navy} navy");
    }
}
