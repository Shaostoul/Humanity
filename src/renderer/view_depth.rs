//! The OFF-SCREEN VIEW depth buffer: which depth texture is current, the
//! window's or a view's.
//!
//! Extracted VERBATIM from `renderer/capture.rs` (2026-09-19) under the
//! file-size ratchet, which capture.rs had outgrown at 367 lines against a
//! 300 budget.
//!
//! WHY THIS IS THE CLUSTER. capture.rs is about getting pixels back OFF the
//! GPU and onto disk. Depth-buffer swapping is not that: it is what an
//! off-screen render needs BEFORE any pixels exist, and its other caller is
//! the in-world camera screens (`engine/ipc.rs`), which never capture
//! anything at all. The three functions here and the `ViewDepth` enum are a
//! closed set - they only ever talk to each other and to the two `Renderer`
//! depth fields - so nothing else moved and no signature changed.
//!
//! NO RE-EXPORT SHIM IS NEEDED for the methods: they are inherent methods on
//! `Renderer`, resolved by receiver type rather than module path, so
//! `engine/ipc.rs` and the screenshot path keep working untouched. The
//! `ViewDepth` enum DOES move path, because `renderer/mod.rs` names it as the
//! type of its `view_depth` field; that is the whole of this change outside
//! these two files.

use super::Renderer;

impl Renderer {
    /// Recreate the shared DEPTH buffer at an arbitrary size (v0.810). The hi-res
    /// offscreen capture re-runs the normal scene passes, which all bind
    /// `depth_view`, so the depth buffer must match the capture target's size for
    /// that one frame; the caller calls this again with the window size right
    /// after to restore. Deliberately does NOT reconfigure the swapchain (that
    /// belongs to the window) and does not touch scene_texture/bloom (they are
    /// not part of the live frame path).
    ///
    /// A request for the size the buffer already has is a no-op: a depth
    /// texture is a full-screen allocation, and re-creating one that already
    /// fits (a view rendered at the window's own size, or the restore after
    /// it) is pure GPU churn. The rung-3 camera screens render at 10 Hz
    /// through `begin_view_depth` / `end_view_depth` below, which never
    /// touch the window's buffer at all; this early-out is the belt to that
    /// braces for any caller still resizing in place.
    pub fn set_depth_target_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.depth_texture.width() == width && self.depth_texture.height() == height {
            return;
        }
        let (tex, view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
    }

    /// Bind a VIEW-SIZED depth buffer for the passes that follow, leaving the
    /// window's own depth buffer parked and untouched (in-world screens,
    /// rung 3). Every `render_*_onto` pass binds `self.depth_view`, so an
    /// off-screen view (a camera screen at 640 x 360, a hi-res screenshot at
    /// 7680 x 4320) needs a depth buffer of ITS size for the duration.
    /// Before this the path resized the shared buffer to the view and back
    /// to the window every time: two full-screen allocations per view,
    /// twenty a second for one camera at 10 Hz, half of them at window size.
    ///
    /// Now the renderer keeps ONE spare depth texture ([`ViewDepth`]). This
    /// swaps it in as the current one (creating it only when its size does
    /// not match `width` x `height`) and parks the window's texture in its
    /// place; [`end_view_depth`](Self::end_view_depth) swaps them back. At
    /// steady state a camera screen costs zero depth allocations per render,
    /// and the window's buffer is never recreated. Two camera screens of
    /// different sizes alternate re-creating the spare (still never the
    /// window's); a per-size cache is not worth its bookkeeping until a
    /// real home has that. Balanced calls only: a second `begin` before
    /// `end` is refused (logged) so the window's buffer can never be lost.
    pub fn begin_view_depth(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let spare = match std::mem::replace(&mut self.view_depth, ViewDepth::None) {
            ViewDepth::Active(t, v) => {
                // Unbalanced: a view is already active. Put things back and
                // refuse, rather than parking the wrong texture.
                log::warn!("begin_view_depth called while a view depth is already active; ignored");
                self.view_depth = ViewDepth::Active(t, v);
                return;
            }
            ViewDepth::Parked(t, v) if t.width() == width && t.height() == height => (t, v),
            // No spare, or a spare of the wrong size: make one that fits
            // (the old one, if any, is dropped here).
            _ => Self::create_depth_texture(&self.device, width, height),
        };
        let window_tex = std::mem::replace(&mut self.depth_texture, spare.0);
        let window_view = std::mem::replace(&mut self.depth_view, spare.1);
        self.view_depth = ViewDepth::Active(window_tex, window_view);
    }

    /// Restore the window's depth buffer after [`begin_view_depth`](Self::begin_view_depth).
    /// `retain` keeps the view-sized texture parked for the next view (the
    /// camera screens: the same size every 100 ms); `false` drops it (the
    /// hi-res screenshot: a one-off whose 8K depth buffer must not linger
    /// for the rest of the session). Without a matching `begin` this does
    /// nothing, so the window's buffer is never replaced by mistake.
    pub fn end_view_depth(&mut self, retain: bool) {
        let ViewDepth::Active(window_tex, window_view) = std::mem::replace(&mut self.view_depth, ViewDepth::None) else {
            return;
        };
        let view_tex = std::mem::replace(&mut self.depth_texture, window_tex);
        let view_view = std::mem::replace(&mut self.depth_view, window_view);
        if retain {
            self.view_depth = ViewDepth::Parked(view_tex, view_view);
        }
    }
}

/// The renderer's spare depth buffer for off-screen views (see
/// [`Renderer::begin_view_depth`]). Exactly one of the two textures, the
/// window's and the view's, is current (`depth_texture` / `depth_view`)
/// at any time; this holds the other.
pub(super) enum ViewDepth {
    /// No spare: the window's depth buffer is current and nothing is parked.
    None,
    /// A view-sized depth buffer is parked; the window's is current.
    Parked(wgpu::Texture, wgpu::TextureView),
    /// A view is being rendered: the WINDOW's depth buffer is parked here
    /// and the view-sized one is current.
    Active(wgpu::Texture, wgpu::TextureView),
}
