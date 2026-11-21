//! The reaction panel's pictures drawn ahead of need, a few at a time (2026-11-20, docs/design/emoji.md
//! "Drawn ahead, gradually").
//!
//! Each picture is drawn the first time it is asked for (`pack.rs`), so the first opening of the
//! chat's ∞ reaction panel drew up to 42 pictures in one frame (about 36 ms, a visible hitch, once a
//! run). The operator asked about that hitch, and then (2026-11-20): "Just remember we don't want to
//! precache all at once. We want to do it gradually so the app doesn't hang."
//!
//! So after start-up a worker thread draws the panel's pictures (the quick row beside the Þ at the
//! body text's size, the ∞ panel at the button text's size, with the theme's outline) ONE AT A TIME
//! with a gap after each ([`GAP_MS`]), never a burst. Each finished picture waits in memory, and the
//! frame turns at most [`UPLOADS_PER_FRAME`] of them into textures, within [`FRAME_BUDGET_MS`] of the
//! frame, so the uploads are spread out too. Nothing waits for the worker: a picture asked for before
//! it is ready is drawn then, as before (and a drawn-ahead one waiting is used rather than drawn
//! twice). The worker starts [`START_AFTER_S`] after the pictures were installed, out of start-up's way.
//!
//! REUSE (CLAUDE.md, the no-caches rule): the drawn-ahead pictures are kept pictures like any other,
//! in memory only, tagged with the pack's version (a picture drawn for a pack that changed is let
//! go) and keyed by size and outline. With the Dev page's "Turn off all reuse" nothing is drawn
//! ahead, since every ask draws its picture again anyway.

use std::time::Duration;

use super::outline::{self, Outline};
use super::pack::{pack_version, PackPictures, WarmKey};
use super::{inline, pieces, EmojiIndex, Piece};

/// Seconds after the pictures were installed (start-up) before anything is drawn ahead.
pub const START_AFTER_S: f32 = 2.0;
/// The worker's rest after each picture it draws, in milliseconds: at 120 frames a second, about one
/// picture every two frames, and the worker never holds a core.
pub const GAP_MS: u64 = 15;
/// At most this many drawn-ahead pictures become textures in one frame.
pub const UPLOADS_PER_FRAME: usize = 4;
/// The frame's budget for the emoji work of drawing ahead, in milliseconds (uploading what the worker
/// finished; the drawing itself is on the worker). Stated in docs/design/emoji.md with the measured
/// worst frame.
pub const FRAME_BUDGET_MS: f64 = 2.0;

/// The pictures the chat's reactions show for `ctx` now: each reaction in the palette
/// (`data/reactions.json`) that the pack draws, at the quick row's size (the body text,
/// `body_size`) and the ∞ panel's (egui's button text style), with `tokens`' outline. The same
/// sizes the chat asks for (`pages::chat::rows`), through the same size rule (`inline::picture_px`).
pub fn reaction_pictures(ctx: &egui::Context, index: &EmojiIndex, body_size: f32, tokens: outline::Tokens) -> Vec<WarmKey> {
    reaction_pictures_at(index, Sizes::of(ctx, body_size), tokens)
}

/// The text sizes the reactions are shown at, and the screen's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// The quick row's (the theme's body text).
    pub body: f32,
    /// The ∞ panel's (egui's button text style).
    pub button: f32,
    pub pixels_per_point: f32,
}

impl Sizes {
    pub fn of(ctx: &egui::Context, body_size: f32) -> Sizes {
        let button = ctx.style().text_styles.get(&egui::TextStyle::Button).map(|f| f.size).unwrap_or(body_size);
        Sizes { body: body_size, button, pixels_per_point: ctx.pixels_per_point() }
    }
}

/// [`reaction_pictures`] for given sizes.
pub fn reaction_pictures_at(index: &EmojiIndex, sizes: Sizes, tokens: outline::Tokens) -> Vec<WarmKey> {
    let (body_size, button_size, ppp) = (sizes.body, sizes.button, sizes.pixels_per_point);
    let outline: Outline = tokens.at(ppp);
    let mut out: Vec<WarmKey> = Vec::new();
    for (list, size) in [(humanity_core::reactions::top(), body_size), (humanity_core::reactions::all(), button_size)] {
        let px = inline::picture_px(size, ppp);
        for emoji in list {
            // As the chat shows it: U+FE0F stripped (`rows.rs`).
            let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
            for piece in pieces(&clean, index) {
                if let Piece::Picture { set, file, .. } = piece {
                    let key = WarmKey { set, file, px, outline };
                    if !out.contains(&key) {
                        out.push(key);
                    }
                }
            }
        }
    }
    out
}

/// What one frame's drawing-ahead did, for the tests and the measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Step {
    /// A new list was handed to the worker this frame.
    pub started: bool,
    /// Drawn-ahead pictures turned into textures this frame.
    pub uploaded: usize,
}

/// Once a frame, from the app's main frame (`engine::frame_ui_run`): hand the reaction panel's
/// pictures to the worker when they change (start-up, a new font size or screen scale, a theme
/// change, a changed pack) and upload a few that it finished.
pub fn frame(ctx: &egui::Context, body_size: f32) -> Step {
    frame_with(ctx, body_size, START_AFTER_S, humanity_core::reuse::enabled())
}

/// [`frame`] with its start delay and the reuse switch as parameters, for the tests.
pub fn frame_with(ctx: &egui::Context, body_size: f32, start_after_s: f32, reuse: bool) -> Step {
    // The installed pictures as they are (`inline::installed`): reading the pack's list again when
    // it is due is the text's job (`inline::source`), not this step's.
    let Some(src) = inline::installed(ctx) else { return Step::default() };
    let Some(pictures) = src.pictures.as_pack() else { return Step::default() };
    let version = pack_version();
    if !reuse {
        pictures.stop_ahead();
        return Step::default();
    }
    if pictures.age_s() < start_after_s {
        return Step::default();
    }
    let tokens = outline::tokens(ctx);
    let sizes = Sizes::of(ctx, body_size);
    // What the list depends on; the worker gets a new list only when one of these changes.
    let asked = [sizes.body.to_bits(), sizes.button.to_bits(), sizes.pixels_per_point.to_bits(), u32::from_le_bytes(tokens.rgba), tokens.width_pt.to_bits()];
    let started = pictures.ensure_ahead(version, asked, || reaction_pictures_at(&src.index, sizes, tokens));
    let uploaded = pictures.upload_ahead(version, UPLOADS_PER_FRAME, Duration::from_secs_f64(FRAME_BUDGET_MS / 1000.0));
    Step { started, uploaded }
}

/// Is every picture in `wanted` kept, ready to draw with no work on the frame?
pub fn all_ready(pictures: &PackPictures, wanted: &[WarmKey]) -> bool {
    let version = pack_version();
    wanted.iter().all(|k| pictures.is_kept(k, version))
}
