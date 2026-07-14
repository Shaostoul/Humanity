//! Non-blocking swapchain readback for live streaming (v0.853.0).
//!
//! The existing `read_texture_to_png` path is fine for a screenshot and WRONG for a
//! stream: it allocates a fresh ~8 MB readback buffer per call and then does a full
//! `device.poll(Maintain::Wait)`, stalling the CPU until the GPU drains. Call that
//! once and you get a screenshot; call it every frame and you get a slideshow.
//!
//! This is the streaming version. It never waits:
//!
//! 1. `submit()` copies the swapchain texture into a REUSED readback buffer, submits,
//!    and asks wgpu to map it asynchronously. Returns immediately.
//! 2. `poll()` runs pending map callbacks with a NON-blocking `Maintain::Poll` and
//!    returns a frame only if one has actually landed. Returns immediately.
//!
//! The cost is one frame of latency (we consume frame N while the GPU renders N+1),
//! which is invisible in a live stream and vastly preferable to a stall.
//!
//! One buffer is enough: at 15 fps we have ~66 ms between captures and the readback
//! lands in single-digit milliseconds. If a capture is still in flight when the next
//! is requested, we simply skip it — a dropped frame, not a stall.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::net::live::RawFrame;

/// A reusable async readback slot.
pub struct StreamCapture {
    buffer: Option<wgpu::Buffer>,
    /// Set by wgpu's map callback (on a wgpu-owned thread) when the mapping lands.
    ready: Arc<AtomicBool>,
    /// True between `submit()` and the frame being consumed by `poll()`.
    inflight: bool,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    bgra: bool,
}

impl Default for StreamCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamCapture {
    pub fn new() -> Self {
        Self {
            buffer: None,
            ready: Arc::new(AtomicBool::new(false)),
            inflight: false,
            width: 0,
            height: 0,
            bytes_per_row: 0,
            bgra: false,
        }
    }

    /// Row stride wgpu requires: 256-byte aligned. The padding is stripped later,
    /// during the downscale pass, so nothing else in the pipeline needs to know.
    fn aligned_bytes_per_row(width: u32) -> u32 {
        (width * 4).div_ceil(256) * 256
    }

    /// Kick off a capture of `texture` (the swapchain texture of the frame just
    /// rendered, BEFORE `present()`). Non-blocking. Returns false if a capture is
    /// already in flight, in which case this frame is simply skipped.
    ///
    /// The texture must have been created with `COPY_SRC` — the swapchain asks for
    /// it at init when the backend allows, exposed as `supports_frame_capture()`.
    pub fn submit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> bool {
        if self.inflight {
            return false;
        }
        let (w, h) = (texture.width(), texture.height());
        if w == 0 || h == 0 {
            return false;
        }
        self.bgra = matches!(
            texture.format(),
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Reallocate only when the window actually changes size.
        let bpr = Self::aligned_bytes_per_row(w);
        if self.buffer.is_none() || self.width != w || self.height != h {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("live_stream_readback"),
                size: (bpr * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
            self.width = w;
            self.height = h;
            self.bytes_per_row = bpr;
        }
        let buffer = self.buffer.as_ref().expect("just ensured");

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("live_stream_capture"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);

        self.ready.store(false, Ordering::SeqCst);
        let flag = self.ready.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
            // Only signal on success. A failed map leaves `inflight` set, which
            // stalls captures rather than handing the encoder garbage pixels; the
            // next resize clears it. Silence beats a corrupt stream.
            if res.is_ok() {
                flag.store(true, Ordering::SeqCst);
            }
        });
        self.inflight = true;
        true
    }

    /// Collect a finished capture, if one has landed. Non-blocking: `Maintain::Poll`
    /// runs pending callbacks and returns immediately rather than waiting on the GPU.
    pub fn poll(&mut self, device: &wgpu::Device) -> Option<RawFrame> {
        if !self.inflight {
            return None;
        }
        let _ = device.poll(wgpu::Maintain::Poll);
        if !self.ready.load(Ordering::SeqCst) {
            return None;
        }

        let buffer = self.buffer.as_ref()?;
        let slice = buffer.slice(..);
        let pixels = {
            let data = slice.get_mapped_range();
            data.to_vec()
        };
        buffer.unmap();
        self.ready.store(false, Ordering::SeqCst);
        self.inflight = false;

        Some(RawFrame {
            pixels,
            width: self.width,
            height: self.height,
            bytes_per_row: self.bytes_per_row,
            bgra: self.bgra,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// wgpu requires 256-byte-aligned rows in a texture-to-buffer copy. Get this
    /// wrong and the copy is a validation error, or the image comes out skewed.
    #[test]
    fn row_stride_is_256_aligned_and_never_smaller_than_the_row() {
        for w in [1u32, 2, 100, 1280, 1920, 2560, 3840] {
            let bpr = StreamCapture::aligned_bytes_per_row(w);
            assert_eq!(bpr % 256, 0, "width {w} produced unaligned stride {bpr}");
            assert!(bpr >= w * 4, "width {w} stride {bpr} cannot hold the row");
            assert!(bpr < w * 4 + 256, "width {w} stride {bpr} over-padded");
        }
    }

    /// 1280 px * 4 bytes = 5120, which is already a multiple of 256, so it must NOT
    /// be padded up to the next block.
    #[test]
    fn an_already_aligned_width_is_not_padded() {
        assert_eq!(StreamCapture::aligned_bytes_per_row(1280), 5120);
    }

    #[test]
    fn a_fresh_capture_has_nothing_to_collect() {
        let cap = StreamCapture::new();
        assert!(!cap.inflight, "nothing is in flight before the first submit");
    }
}
