//! HumanityOS Native Desktop Application
//!
//! Entry point for the standalone desktop binary.
//! Launches the wgpu renderer with egui GUI and all game systems.

// Hide the console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "native")]
fn main() {
    humanity_engine::run();
}

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("This binary requires the 'native' feature. Build with: cargo build --features native");
}
