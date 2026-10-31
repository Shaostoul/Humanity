//! Lay out only what is on screen (2026-10-31, docs/design/sim-frame-cost.md, "After the two
//! fixes").
//!
//! A long list inside a scroll area used to lay out every one of its rows on every draw, though a
//! person only ever sees a screenful of them. The Inventory page's Garden section laid out all of
//! the showcase's ~2,000 crop tiles each time it was drawn (`cpu.garden.page`, 5.4 to 6.5 ms a
//! draw on a busy machine, on the main screen and on the Inventory wall screen's turn), and a farm
//! five times the size would have cost five times as much just to look at (infinite-of-x).
//!
//! The standard technique for a list whose rows are not all the same height: a block that lies
//! wholly outside the visible part of its scroll area is not laid out at all. The space it took the
//! last time it WAS laid out is taken instead, so everything below it lands where it always did and
//! the scroll bar keeps its length and position exactly. A block never laid out before is laid out
//! (nothing is estimated), so the first draw of a page costs what it always did and every draw after
//! it costs what is on screen.
//!
//! The remembered sizes are data kept across frames, so CLAUDE.md's reuse rule applies in full:
//! - TAGGED. Each size is stored with a key the caller computes from everything that decides that
//!   block's size (its rows, the words in them, whether it is open), and this module mixes in what
//!   decides every block's size: where the block starts and how wide it may be. The caller adds the
//!   page-wide part once a frame with [`style_key`] (text sizes, the font's own measurements, the
//!   spacing, pixels per point). A changed key lays the block out again in the same frame, so a
//!   stale size is never used.
//! - SWITCHED. With the Dev page's "Turn off all reuse" (`crate::reuse`) off, nothing is skipped
//!   and nothing is remembered: every block is laid out every frame, exactly as before.
//!
//! Also never skipped: a sizing pass (egui measuring a window at its smallest), and any frame in
//! which a widget holds keyboard focus. Tab moves focus to the next widget laid out that frame, so
//! with rows skipped a keyboard user could not Tab down a list; while focus is held everything is
//! laid out, at the old cost, and Tab walks every row and scrolls each into view as before. (Clicks
//! do not hold focus in egui; Tab and a typing field do.) The app does not build egui's AccessKit
//! tree; if it ever does, a screen reader must see every row too, so add that condition here.
//!
//! The sizes live in the egui context's own memory (temporary data, never saved to disk), so each
//! wall screen and the main screen keep their own: the same page is a different width on each.

use egui::{Id, Rect, Ui};
use std::hash::Hasher;

/// What a block looked like the last time it was laid out: the key it was laid out under and the
/// rectangle it took, relative to where it started.
#[derive(Clone, Copy, Debug)]
struct Seen {
    key: u64,
    rect: Rect,
}

#[cfg(test)]
thread_local! {
    /// A test's own setting, so a test can compare skipping with no skipping without flipping
    /// the process-wide switch under the other tests running beside it.
    static TEST_OVERRIDE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// For tests: force skipping on (`Some(true)`), off (`Some(false)`), or back to the Dev page's
/// switch (`None`) on this thread.
#[cfg(test)]
pub fn set_test_override(on: Option<bool>) {
    TEST_OVERRIDE.with(|c| c.set(on));
}

/// May this module skip blocks at all? Off with the Dev page's "Turn off all reuse".
pub fn enabled() -> bool {
    #[cfg(test)]
    if let Some(on) = TEST_OVERRIDE.with(|c| c.get()) {
        return on;
    }
    crate::reuse::enabled()
}

/// May blocks in `ui` be skipped THIS frame? Not in a sizing pass (widths are at their smallest
/// there, so its sizes are not the real ones) and not while a widget holds keyboard focus (see the
/// module notes).
fn enabled_here(ui: &Ui) -> bool {
    enabled() && !ui.is_sizing_pass() && ui.ctx().memory(|m| m.focused().is_none())
}

/// A small fast hasher for these keys (the FxHash mix, the one rustc uses for its own tables). A
/// key is worked out for every block every frame, including the ones skipped, so it has to cost far
/// less than laying the block out. Collisions only matter between two looks of the SAME block, one
/// frame apart, so a 64-bit mix is plenty. It is also a `fmt::Write`, so a number can be hashed
/// exactly as it is printed (`write!(h, "{x:.1}")`) without building a String.
#[derive(Clone, Copy, Debug)]
pub struct KeyHasher(u64);

impl Default for KeyHasher {
    fn default() -> Self {
        KeyHasher(0x2f6b_8a5c_d3e1_4907)
    }
}

impl KeyHasher {
    const K: u64 = 0x517c_c1b7_2722_0a95;

    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::K);
    }
}

impl Hasher for KeyHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut w = [0u8; 8];
            w[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(w));
        }
        // The length too, so "ab" + "c" and "a" + "bc" differ.
        self.add(bytes.len() as u64);
    }

    fn write_u8(&mut self, v: u8) {
        self.add(v as u64);
    }

    fn write_u32(&mut self, v: u32) {
        self.add(v as u64);
    }

    fn write_u64(&mut self, v: u64) {
        self.add(v);
    }

    fn write_usize(&mut self, v: usize) {
        self.add(v as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Write for KeyHasher {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

/// [`KeyHasher`] for a `HashMap` worked through every frame (`HashMap<K, V, KeyBuild>`), where
/// the standard hasher's resistance to crafted keys buys nothing: the keys are the game's own (crop
/// entities, grow-area ids) and the map is rebuilt or read once a frame per crop.
pub type KeyBuild = std::hash::BuildHasherDefault<KeyHasher>;

/// The page-wide part of every key, worked out once a frame by the caller: pixels per point, the
/// spacing egui lays widgets out with, and for each text size the page uses, the font's own line
/// height and two glyph widths (so a different font, or a different size, changes every key and
/// every block is laid out again). `other` takes the caller's own numbers that change sizes, such
/// as the theme's spacing values or the screen width a grid wraps by.
pub fn style_key(ui: &Ui, font_sizes: &[f32], other: &[f32]) -> u64 {
    let mut h = KeyHasher::default();
    h.write_u32(ui.ctx().pixels_per_point().to_bits());
    let s = ui.spacing();
    for v in [
        s.item_spacing.x,
        s.item_spacing.y,
        s.button_padding.x,
        s.button_padding.y,
        s.interact_size.x,
        s.interact_size.y,
        s.indent,
        s.icon_width,
        s.icon_width_inner,
        s.icon_spacing,
    ] {
        h.write_u32(v.to_bits());
    }
    ui.fonts(|f| {
        for &size in font_sizes {
            let id = egui::FontId::proportional(size);
            h.write_u32(size.to_bits());
            h.write_u32(f.row_height(&id).to_bits());
            h.write_u32(f.glyph_width(&id, 'm').to_bits());
            h.write_u32(f.glyph_width(&id, '0').to_bits());
        }
    });
    for v in other {
        h.write_u32(v.to_bits());
    }
    h.finish()
}

/// The caller's key mixed with where the block starts and how wide it may be.
fn placed_key(ui: &Ui, key: u64) -> u64 {
    let avail = ui.available_rect_before_wrap();
    let mut h = KeyHasher(key);
    h.write_u32(avail.min.x.to_bits());
    h.write_u32(avail.width().to_bits());
    h.finish()
}

/// Take the space of a block that is wholly off screen, without laying it out. True when it was
/// skipped (the caller draws nothing for it this frame); false when the caller must lay it out,
/// through [`measure`]: skipping is off, the block was never laid out under this key, or some of it
/// would be visible.
pub fn skip_if_unseen(ui: &mut Ui, id: Id, key: u64) -> bool {
    if !enabled_here(ui) {
        return false;
    }
    let Some(seen) = ui.data(|d| d.get_temp::<Seen>(id)) else { return false };
    if seen.key != placed_key(ui, key) {
        return false;
    }
    let start = ui.available_rect_before_wrap().min;
    let rect = seen.rect.translate(start.to_vec2());
    if ui.is_rect_visible(rect) {
        return false;
    }
    // Exactly what `Ui::scope` does with the rect its contents took: the cursor moves past it (and
    // the auto-id counter by one, as a scope's does, so the ids of everything after stay the same
    // whether this block was skipped or drawn).
    ui.advance_cursor_after_rect(rect);
    true
}

/// Lay out a block in a scope of its own and, when `key` is given and skipping is on, remember the
/// space it took under that key for [`skip_if_unseen`]. Pass `None` for a look that must not be
/// remembered (half way through an open/close animation, say): it is laid out and nothing is kept.
pub fn measure<R>(ui: &mut Ui, id: Id, key: Option<u64>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let start = ui.available_rect_before_wrap().min;
    let placed = key.map(|k| placed_key(ui, k));
    let remember = enabled_here(ui);
    let inner = ui.scope(add);
    if remember {
        if let Some(key) = placed {
            let rect = inner.response.rect.translate(-start.to_vec2());
            ui.data_mut(|d| d.insert_temp(id, Seen { key, rect }));
        }
    }
    inner.inner
}

/// The usual call: skip the block when it is off screen and its size is known for `key`, else lay
/// it out and remember its size (with no key: lay it out, remember nothing). Returns what `add`
/// returned, or `None` when it was skipped.
pub fn show<R>(ui: &mut Ui, id: Id, key: Option<u64>, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    if key.is_some_and(|k| skip_if_unseen(ui, id, k)) {
        return None;
    }
    Some(measure(ui, id, key, add))
}

/// Drop what was remembered for a block: a click inside it changed its size during the very
/// frame it was measured (a group's triangle, a tile opening a card), so the size measured is not
/// the one its key describes.
pub fn forget(ui: &Ui, id: Id) {
    ui.data_mut(|d| d.remove::<Seen>(id));
}
