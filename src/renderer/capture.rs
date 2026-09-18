//! Frame + texture CAPTURE: getting pixels back off the GPU and onto disk.
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1108) under the file-size
//! ratchet, which mod.rs was sitting exactly on (3,883 of 3,883) when the
//! shadow-cutout work needed room.
//!
//! WHY THIS CLUSTER. It is the one group in mod.rs with a single job that
//! nothing in the frame loop calls: the live screenshot command (`debug/
//! screenshot_request.json`), the hi-res offscreen capture and the probe rig
//! all enter here and nowhere else. Every function is a `&self` (or
//! target-sizing) helper over `device`/`queue`/`config`; none of them touch
//! the render passes, the material registry or any per-frame state, so moving
//! them changes no ordering and no borrow. The alternative candidates were
//! worse: the pass helpers (`render_*_onto`) ARE the frame loop, and the
//! target/depth plumbing is read by `resize` and the passes both.
//!
//! NO RE-EXPORT SHIM IS NEEDED, for the same reason `materials.rs` needed
//! none: these are inherent methods on `Renderer`, resolved by receiver type
//! rather than module path, so every call site in the crate keeps working
//! untouched. A child module also sees its parent's private items, so
//! `supports_frame_capture` (the FIELD), `config`, `depth_texture` and
//! `depth_view` stay private and are reached from here.
//!
//! The one project rule this file carries: a capture that silently ships a
//! bad file must never report ok. `read_texture_to_png` re-reads the PNG
//! header off disk and fails if the dimensions do not match the request.

use super::Renderer;

impl Renderer {
    /// Whether the swapchain surface was configured with `COPY_SRC`, i.e. whether
    /// `capture_current_frame` can succeed on this backend. (v0.639)
    pub fn supports_frame_capture(&self) -> bool {
        self.supports_frame_capture
    }

    /// Capture `texture` (the swapchain texture of the frame just rendered, BEFORE
    /// `present()`) to a PNG at `path` (v0.639, the live in-game screenshot command). Reuses the
    /// copy-texture-to-buffer-to-PNG technique `ui_snapshots.rs::render_page_png` already uses
    /// for offscreen snapshots, adapted for the live swapchain: the surface format is not
    /// necessarily `Rgba8*` (Windows/DX12 commonly configures `Bgra8UnormSrgb`), so a BGRA
    /// surface has its R/B channels swapped back before the `image` crate (which expects RGBA)
    /// writes the file. Returns a plain error string (not a panic) if this backend's swapchain
    /// doesn't support `COPY_SRC` -- checked once at `init` via `supports_frame_capture`.
    pub fn capture_current_frame(&self, texture: &wgpu::Texture, path: &std::path::Path) -> Result<(), String> {
        if !self.supports_frame_capture {
            return Err("swapchain surface has no COPY_SRC usage on this backend -- frame capture unavailable".to_string());
        }
        let (w, h) = (self.config.width, self.config.height);
        if w == 0 || h == 0 {
            return Err("zero-sized surface -- nothing to capture".to_string());
        }
        self.read_texture_to_png(texture, w, h, path)
    }

    /// Largest texture edge this device supports (v0.810, hi-res screenshot capture).
    /// Queried live from the device limits so the capture path's size clamp never
    /// hardcodes a backend-specific number.
    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Create an offscreen color target for a one-frame hi-res capture (v0.810).
    /// Uses the SWAPCHAIN's format so every existing scene pipeline (they were all
    /// built against `surface_format`) renders to it unchanged, plus COPY_SRC for
    /// the PNG readback. Caller renders the normal passes to the returned view,
    /// then hands the texture to `read_texture_to_png`.
    pub fn create_capture_target(&self, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HiRes Capture Target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

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

impl Renderer {
    /// Read a rendered texture (must have COPY_SRC and the swapchain's format)
    /// back to a PNG at `path` (v0.810; generalized from the v0.639 swapchain
    /// capture so the hi-res offscreen target uses the same proven path). After
    /// writing, the file's header is re-read and its dimensions must match
    /// `width` x `height` exactly, or this returns Err -- a capture that
    /// silently shipped a bad file must never report ok (project lesson).
    /// Dump the octa cloud map's CURRENT ping to a PNG (dev forensics,
    /// v0.1246). The map is 4096^2 RGBA16F premultiplied; the dump writes a
    /// DOUBLE-WIDE image: left half = rgb (clamped), right half = alpha as
    /// grayscale, so one file answers both "what content" and "what
    /// coverage". This is the decisive instrument for the starburst
    /// forensics: fibres present in THIS image = baked into the accumulated
    /// content (temporal/march side); absent = display/sampling side.
    pub fn dump_cloud_map_png(&self, path: &std::path::Path) -> Result<(u32, u32), String> {
        fn f16_to_f32(bits: u16) -> f32 {
            let s = (bits >> 15) & 1;
            let e = ((bits >> 10) & 0x1f) as i32;
            let m = (bits & 0x3ff) as f32;
            let f = if e == 0 {
                m * (-24f32).exp2()
            } else if e == 31 {
                if m == 0.0 { f32::INFINITY } else { f32::NAN }
            } else {
                (1.0 + m / 1024.0) * ((e - 15) as f32).exp2()
            };
            if s == 1 { -f } else { f }
        }
        let ct = self
            .cloud_temporal
            .as_ref()
            .ok_or_else(|| "cloud temporal map not active".to_string())?;
        let tex = ct.cur_texture();
        let (w, h) = (tex.width(), tex.height());
        let bytes_per_row = ((w * 8 + 255) / 256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cloud_map_readback"),
            size: (bytes_per_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cloud_map_dump_encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let mut pixels = vec![0u8; (w * 2 * h * 4) as usize];
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let row_bytes = &data[start..start + (w * 8) as usize];
            for (x, px) in row_bytes.chunks_exact(8).enumerate() {
                let r = f16_to_f32(u16::from_le_bytes([px[0], px[1]]));
                let g = f16_to_f32(u16::from_le_bytes([px[2], px[3]]));
                let b = f16_to_f32(u16::from_le_bytes([px[4], px[5]]));
                let a = f16_to_f32(u16::from_le_bytes([px[6], px[7]]));
                let li = ((row * w * 2 + x as u32) * 4) as usize;
                pixels[li] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[li + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[li + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[li + 3] = 255;
                let ri = ((row * w * 2 + w + x as u32) * 4) as usize;
                let av = (a.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[ri] = av;
                pixels[ri + 1] = av;
                pixels[ri + 2] = av;
                pixels[ri + 3] = 255;
            }
        }
        drop(data);
        buffer.unmap();
        let img = image::RgbaImage::from_raw(w * 2, h, pixels)
            .ok_or_else(|| "cloud map pixel buffer size mismatch".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        img.save(path).map_err(|e| e.to_string())?;
        Ok((w * 2, h))
    }

    pub fn read_texture_to_png(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let (w, h) = (width, height);
        if w == 0 || h == 0 {
            return Err("zero-sized texture -- nothing to capture".to_string());
        }
        let bytes_per_row = ((w * 4 + 255) / 256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_capture_readback"),
            size: (bytes_per_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_capture_encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let row_bytes = &data[start..start + (w * 4) as usize];
            if bgra {
                for px in row_bytes.chunks_exact(4) {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            } else {
                pixels.extend_from_slice(row_bytes);
            }
        }
        drop(data);
        buffer.unmap();

        let img = image::RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| "captured pixel buffer size mismatch".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        img.save(path).map_err(|e| e.to_string())?;
        // Self-verify the written file (v0.810): decode the PNG header off disk and
        // require the actual dimensions to match the capture request before
        // reporting success. A writer that silently ships nothing (or a truncated
        // file) must surface as an error, never an ok:true.
        let (dw, dh) = image::image_dimensions(path)
            .map_err(|e| format!("wrote {} but could not verify it: {e}", path.display()))?;
        if (dw, dh) != (w, h) {
            return Err(format!(
                "PNG verification failed: requested {w}x{h} but {} decodes as {dw}x{dh}",
                path.display()
            ));
        }
        Ok(())
    }
}
