//! THE PICTURE'S OWN NAMEPLATE (2026-11-21, the copresence meeting's picture race).
//!
//! A screenshot request may name a drawn text, `{"find": "TestBotPlots"}`, and the done file then
//! says where THE CAPTURED FRAME drew it (`"find": {"found", "text", "matches", "rect_px",
//! "pos_px"}`, in window pixels, the main UI `find` verb's fields), from that frame's own egui
//! shapes. A rig that judges a figure under its name in a picture needs the name's place in the
//! picture, and a separate `find` before or after the request cannot give it: a window capture
//! waits for temporal anti-aliasing to settle (`taa::SETTLE_FRAMES`, 16 frames), which on a busy
//! machine took 1 to 3 s, and verify-copresence's walker turned at its line's end inside that
//! window. Its walker-first meeting then failed `meet_figure_visible` on a picture that showed the
//! walker plainly: the finds before and after the capture both caught it near 77.1 to 77.7 m, on
//! its way out and on its way back, while the picture caught it near 79 m, outside the box drawn
//! between them (runs 20261121-230950 and -232752: 0 and 29 teal pixels in the box, 824 and 827
//! in the picture).
//!
//! How it stays about one frame: the request is taken in the frame's tail (`ipc::
//! poll_screenshot_request`), after that frame's shapes were already painted, so a request with a
//! find is HELD at least one frame; every frame while it is pending, `scan` looks for the text in
//! the shapes the paint is about to tessellate (engine/frame_ui_paint.rs), and the same frame's
//! tail takes that answer (`frame_answer`), so a capture can only carry the answer of the frame it
//! captured. A frame whose UI did not run (no scan) holds the capture for the next one.

use crate::gui::screen_surface::FoundText;
use std::sync::Mutex;

/// The text the pending screenshot asked for, and this frame's answer (None until this frame's
/// paint scanned; `Some(None)` = scanned and not drawn).
struct Pending {
    text: String,
    this_frame: Option<Option<FoundText>>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// A screenshot request was taken: remember what it asks to find (None: nothing; a request without
/// a find clears any earlier one).
pub(crate) fn ask(text: Option<String>) {
    if let Ok(mut p) = PENDING.lock() {
        *p = text.filter(|t| !t.trim().is_empty()).map(|text| Pending { text, this_frame: None });
    }
}

/// Look for the pending text in this frame's shapes (the main egui pass's, before tessellation
/// consumes them). A no-op when no screenshot asks for a find, so it costs nothing on a normal frame.
pub(crate) fn scan(shapes: &[egui::epaint::ClippedShape]) {
    let Ok(mut p) = PENDING.lock() else { return };
    if let Some(p) = p.as_mut() {
        p.this_frame = Some(crate::gui::screen_surface::find_text_in_shapes(shapes, &p.text));
    }
}

/// What this frame's tail may capture with: `Ready(None)` when no find is pending, `Ready(Some(_))`
/// with this frame's answer, `NotScanned` when a find is pending and this frame's shapes were not
/// scanned (the request was taken this frame, or the UI did not run): then the capture waits a
/// frame. Called once per frame by `ipc::poll_screenshot_request`; it clears this frame's answer
/// either way, so a later frame never captures with an earlier frame's.
pub(crate) enum FrameAnswer {
    Ready(Option<FindAnswer>),
    NotScanned,
}

/// A frame's answer for the done file: the query and what that frame drew.
pub(crate) struct FindAnswer {
    query: String,
    found: Option<FoundText>,
}

pub(crate) fn frame_answer() -> FrameAnswer {
    let Ok(mut p) = PENDING.lock() else { return FrameAnswer::Ready(None) };
    match p.as_mut() {
        None => FrameAnswer::Ready(None),
        Some(pending) => match pending.this_frame.take() {
            None => FrameAnswer::NotScanned,
            Some(found) => FrameAnswer::Ready(Some(FindAnswer { query: pending.text.clone(), found })),
        },
    }
}

/// The capture was taken: the find is answered and no longer pending.
pub(crate) fn done() {
    if let Ok(mut p) = PENDING.lock() {
        *p = None;
    }
}

impl FindAnswer {
    /// The done file's `find` object, in window pixels (`ppp` = egui's pixels per point), with the
    /// fields of the main UI `find` verb (engine/ipc.rs `complete_ui_request`).
    pub(crate) fn json(&self, ppp: f32) -> serde_json::Value {
        match &self.found {
            Some(f) => {
                let (r, c) = (f.rect, f.rect.center());
                serde_json::json!({
                    "query": self.query,
                    "found": true,
                    "text": f.text,
                    "matches": f.matches,
                    "rect_px": [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp],
                    "pos_px": [c.x * ppp, c.y * ppp],
                })
            }
            None => serde_json::json!({ "query": self.query, "found": false, "matches": 0 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CAPTURE CARRIES ONLY ITS OWN FRAME'S ANSWER. The request is taken in a frame's tail, after
    /// that frame's paint, so the first frame has no answer and the capture waits; a frame that
    /// scanned answers for itself and its answer is gone by the next frame; a request without a
    /// find never waits. Seen red 2026-11-21 with `frame_answer` keeping the answer across frames
    /// (`this_frame.clone()` instead of `take()`): panicked at "an unscanned frame waits".
    #[test]
    fn a_capture_carries_only_its_own_frames_answer() {
        // One test owns the static (the module's tests run in this one function).
        ask(None);
        assert!(matches!(frame_answer(), FrameAnswer::Ready(None)), "no find: capture at once");
        ask(Some("TestBotPlots".into()));
        assert!(matches!(frame_answer(), FrameAnswer::NotScanned), "taken this frame: not yet scanned");
        scan(&[]);
        match frame_answer() {
            FrameAnswer::Ready(Some(a)) => {
                assert!(a.found.is_none());
                assert_eq!(a.json(2.0), serde_json::json!({ "query": "TestBotPlots", "found": false, "matches": 0 }));
            }
            _ => panic!("a scanned frame answers"),
        }
        assert!(matches!(frame_answer(), FrameAnswer::NotScanned), "an unscanned frame waits");
        done();
        assert!(matches!(frame_answer(), FrameAnswer::Ready(None)), "answered: nothing pending");
        ask(Some("   ".into()));
        assert!(matches!(frame_answer(), FrameAnswer::Ready(None)), "a blank find is no find");
    }
}
