//! HumanityOS Native Desktop Application
//!
//! Entry point for the standalone desktop binary.
//! Launches the wgpu renderer with egui GUI and all game systems.

#[cfg(feature = "native")]
fn main() {
    humanity_engine::run();
}

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("This binary requires the 'native' feature. Build with: cargo build --features native");
}
