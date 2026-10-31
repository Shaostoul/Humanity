//! A frame of pixels read back from the GPU for the live stream.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was `net::live`'s, and the renderer's stream capture
//! (`renderer::stream_capture`), which fills them, reached into the network side for it. The
//! renderer makes one and the live stream's worker (`net::live`) converts and sends it; plain
//! data, under both.

/// A raw frame handed over by the render thread. Pixels are still in the GPU's
/// layout (padded rows, possibly BGRA) because converting them is the worker's job,
/// not the render thread's.
pub struct RawFrame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub bgra: bool,
}
