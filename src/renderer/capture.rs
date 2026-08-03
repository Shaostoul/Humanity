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
    pub fn set_depth_target_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let (tex, view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
    }

    /// Read a rendered texture (must have COPY_SRC and the swapchain's format)
    /// back to a PNG at `path` (v0.810; generalized from the v0.639 swapchain
    /// capture so the hi-res offscreen target uses the same proven path). After
    /// writing, the file's header is re-read and its dimensions must match
    /// `width` x `height` exactly, or this returns Err -- a capture that
    /// silently shipped a bad file must never report ok (project lesson).
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
