//! Surface + render-target LIFECYCLE: everything that creates, reconfigures,
//! acquires or resizes the things a frame is drawn INTO.
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1319) under the file-size
//! ratchet, which mod.rs had outgrown badly: 5,652 lines against a 3,883
//! budget. Four clusters came out in that pass; this is the smallest and the
//! most self-contained.
//!
//! WHY THIS CLUSTER. Every function here is about the SWAPCHAIN and the two
//! offscreen targets sized to match it (the depth buffer and the scene
//! texture), and none of it knows what a scene is. The swapchain is
//! configured (`set_vsync`, `apply_pending_surface_config`), resized
//! (`resize`, which rebuilds both targets), asked about (`aspect_ratio`,
//! `surface_format`, `surface_size`) and acquired (`acquire_surface`,
//! `acquire_surface_cleared`). `create_scene_texture` and
//! `create_depth_texture` are the two constructors that `resize`, `init` and
//! `capture.rs` all share. One job, one file.
//!
//! BUG-077 LIVES HERE, and is why the present-mode change is DEFERRED rather
//! than applied where it is asked for: reconfiguring the surface while a
//! frame's swapchain view is still alive makes DXGI's ResizeBuffers refuse,
//! and wgpu's default fatal handler then ends the process. `set_vsync`
//! records the wanted mode; `apply_pending_surface_config` applies it at the
//! START of the next frame. The two pure helpers that decide it
//! (`vsync_present_mode`, `pending_mode_change`) and their unit tests moved
//! here with it, and mod.rs re-exports them so `renderer::vsync_present_mode`
//! still resolves for any future caller.
//!
//! NO RE-EXPORT SHIM IS NEEDED for the methods, for the same reason
//! `materials.rs` and `capture.rs` needed none: they are inherent methods on
//! `Renderer`, resolved by receiver type rather than by module path, so every
//! call site in the crate keeps working untouched. A child module also sees
//! its parent's private items, so `surface`, `config`, `pending_present_mode`,
//! `depth_texture`, `scene_texture` and `bloom` stay private and are reached
//! from here.

use super::{frame_costs, Renderer};

impl Renderer {
    /// Apply the Settings VSync toggle (v0.909 - the toggle used to save a
    /// value nothing read). AutoVsync caps at the monitor refresh;
    /// AutoNoVsync uncaps (mailbox/immediate as the platform allows).
    ///
    /// DEFERRED, not immediate (BUG-077, 2026-09-18): this is called from
    /// the settings-apply block at the TAIL of the frame arm, while that
    /// frame's swapchain `TextureView` is still in scope. Reconfiguring the
    /// surface there makes DXGI's ResizeBuffers refuse (an outstanding
    /// back-buffer reference, 0x887A0001), wgpu reports "window is in use"
    /// and its default fatal handler ends the process. With `vsync: true`
    /// nothing ever reconfigured, so nobody saw it; with `vsync: false` in
    /// the config the app died on its FIRST frame, menu or world. The mode
    /// is recorded here and applied by `apply_pending_surface_config` at the
    /// start of the next frame, before the surface texture is acquired.
    pub fn set_vsync(&mut self, on: bool) {
        let mode = vsync_present_mode(on);
        self.pending_present_mode = pending_mode_change(self.config.present_mode, mode);
    }

    /// Apply a present-mode change recorded by `set_vsync`. Call once per
    /// frame BEFORE `acquire_surface` / `acquire_surface_cleared`, when no
    /// view of a swapchain texture can be alive (see BUG-077). A no-op when
    /// nothing is pending, which is every frame but the one after a toggle.
    pub fn apply_pending_surface_config(&mut self) {
        if let Some(mode) = self.pending_present_mode.take() {
            // Logged so a rig or a crash reader can see that the reconfigure
            // ran (the runtime proof of BUG-077 had to infer it from the
            // config save line; this line makes it direct).
            log::info!("[Surface] present mode {:?} -> {mode:?} (applied before the frame)", self.config.present_mode);
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Handle window/canvas resize.

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (tex, view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
        // Resize scene texture + bloom
        let fmt = self.config.format;
        let (st, sv) = Self::create_scene_texture(&self.device, width, height, fmt);
        self.scene_texture = st;
        self.scene_view = sv;
        if let Some(ref mut bloom) = self.bloom {
            bloom.resize(&self.device, width, height);
        }
    }

    /// Current surface aspect ratio.
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Surface texture format (needed by egui-wgpu renderer).
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Current surface dimensions.
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Acquire the surface texture and clear it with a solid color.
    /// Used when rendering UI-only frames (no 3D scene).
    pub fn acquire_surface_cleared(
        &self,
        clear_color: wgpu::Color,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        // UI-only frame: no world pass runs, so the budget numbers stay frozen
        // at the last rendered world frame (resource budgets increment 1).
        self.frame_costs_begin(false);
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Clear Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                timestamp_writes: self.pass_timer("gpu.clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok((output, view))
    }

    /// Acquire surface and clear to black, returning the texture for star + scene rendering.
    pub fn acquire_surface(&self) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        self.frame_costs_begin(true);
        // `cpu.present_wait`, half 1: acquiring the next swapchain image
        // blocks when the display has not released one yet (vsync, or the
        // driver's own frame queue). The other half is timed around
        // `present()` in the frame loop; both sum under the one id, so the
        // Performance page can tell "waiting for the display" from
        // "CPU-bound", which the 2026-09-18 measurement could not.
        let wait_t0 = std::time::Instant::now();
        let output = self.surface.get_current_texture()?;
        frame_costs::record_cpu("cpu.present_wait", wait_t0.elapsed());
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((output, view))
    }

    /// Create the off-screen scene texture (same format as surface, with TEXTURE_BINDING).
    ///
    /// `pub(super)` only because `init` (mod.rs) and the offscreen capture
    /// targets (capture.rs) build their own: it was private while it lived
    /// beside them in mod.rs, and this is the whole privacy delta of the move.
    pub(super) fn create_scene_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub(super) fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}

/// The present mode the Settings VSync switch means: on = capped at the
/// monitor refresh, off = uncapped (mailbox or immediate as the platform
/// allows). Pure, so the mapping is unit-tested without a device.
pub fn vsync_present_mode(on: bool) -> wgpu::PresentMode {
    if on { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync }
}

/// What `set_vsync` must record: `Some(want)` only when the surface is not
/// already in that mode, so a settings apply that changes nothing never
/// schedules a reconfigure (that was true of the old immediate path too, and
/// it is why `vsync: true` never crashed: it never reconfigured).
pub fn pending_mode_change(current: wgpu::PresentMode, want: wgpu::PresentMode) -> Option<wgpu::PresentMode> {
    if current == want { None } else { Some(want) }
}

#[cfg(test)]
mod vsync_deferral_tests {
    use super::*;

    /// BUG-077: the switch maps to the two auto modes and nothing else.
    #[test]
    fn vsync_switch_maps_to_the_auto_modes() {
        assert_eq!(vsync_present_mode(true), wgpu::PresentMode::AutoVsync);
        assert_eq!(vsync_present_mode(false), wgpu::PresentMode::AutoNoVsync);
    }

    /// A reconfigure is scheduled only on a real change: the first settings
    /// apply after boot (vsync true in the config, surface already AutoVsync)
    /// must schedule nothing, and a toggle to off must schedule exactly the
    /// off mode. The apply itself runs at the next frame's start, never in
    /// the frame arm, which is the whole fix.
    #[test]
    fn a_reconfigure_is_pending_only_when_the_mode_changes() {
        let vs = wgpu::PresentMode::AutoVsync;
        let no = wgpu::PresentMode::AutoNoVsync;
        assert_eq!(pending_mode_change(vs, vsync_present_mode(true)), None, "no change, nothing to apply");
        assert_eq!(pending_mode_change(vs, vsync_present_mode(false)), Some(no), "off is a change");
        assert_eq!(pending_mode_change(no, vsync_present_mode(true)), Some(vs), "back on is a change");
        assert_eq!(pending_mode_change(no, vsync_present_mode(false)), None);
    }
}
