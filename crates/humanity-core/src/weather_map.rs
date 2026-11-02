//! The live weather map's size (v0.874): an equirectangular grid of 1440 x 720 cells, a
//! quarter of a degree each. The fetcher (`net::live_weather`, native-only) fills a grid this
//! size from NASA GIBS cloud fraction, and the renderer uploads it as a texture (its mip count,
//! `renderer::WEATHER_MAP_MIPS`, follows from the height).
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was defined on the renderer, because the renderer compiles
//! in every feature set while the fetcher is native-only, so the network side reached into the
//! renderer for two numbers. The core is under both and compiles everywhere.

/// Cells across: 360 degrees of longitude at a quarter degree.
pub const WEATHER_MAP_W: u32 = 1440;
/// Cells down: 180 degrees of latitude at a quarter degree.
pub const WEATHER_MAP_H: u32 = 720;
