//! How-to: step-by-step procedures for real life, the game and this platform, as data
//! (docs/design/how-to.md; increment 1, 2026-10-23).
//!
//! A how-to is one JSON file, `data/how_to/<top category>/<id>.json`, in the shape
//! schemas/how_to.toml describes. This module is the model, the loader, the checker, the hash a
//! review binds to, and a person's runs. It compiles into the relay as well as the desktop app
//! (the relay will host shared how-tos in increment 8), so nothing here draws: the page is
//! src/gui/pages/how_to.rs, and the website's page web/pages/how-to.html reads the same files.
//!
//! - model.rs     the fields, refusing unknown ones
//! - check.rs     every gate, by code; tests/fixtures/how_to/broken.json shows each one red
//! - canonical.rs the hash: BLAKE3 of the canonical CBOR without the review block
//! - library.rs   loading a data folder, refused files kept with their reasons
//! - runs.rs      a person's runs, keyed by step id, written crash-safely
//! - view.rs      Short and Full, the visible steps, the current step, search
//! - time.rs      UTC times as text
//!
//! Gated on `relay` because the hash uses ciborium and blake3, which only that feature brings;
//! `native` includes `relay`, so the desktop app always has it.

pub mod canonical;
pub mod check;
pub mod library;
pub mod model;
pub mod runs;
pub mod time;
pub mod view;

#[cfg(test)]
mod tests;
