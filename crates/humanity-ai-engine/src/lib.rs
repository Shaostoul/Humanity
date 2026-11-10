//! The AI engine, `humanity-ai.exe` (docs/design/embeddable-ai.md, section 7; rung 1, 2026-11-09).
//!
//! A program of its own, started by the app only while an AI feature is in use, with its standard
//! input and output connected: it reads one message a line ([`humanity_ai_protocol::AppMessage`]) and
//! answers one a line ([`humanity_ai_protocol::EngineMessage`]). It opens no socket and listens on
//! nothing, so no firewall question ever arises and no other program on the computer can talk to it
//! (7.2). When the app goes, its standard input closes and the engine exits; the app's job object ends
//! it too, even if the app crashed.
//!
//! RUNG 1 HOLDS NO MODEL AND NO RUNTIME. Two modes:
//! - `--fake`: scripted lines and timing ([`fake`]), the stand-in every client test, Dev page button
//!   and rig uses (12.1). It also stands in for the reuse switch (a persona read again only when reuse
//!   is off) and for running out of memory (`--fake-load-mb`, against the job object's cap).
//! - without it: `hello` with no backends, and every model request refused in words
//!   ([`humanity_ai_protocol::ErrorCode::NoBackend`]). Rung 2 adds llama.cpp here.
//!
//! The engine never prints anything but messages on its standard output; notes for a person go to its
//! standard error, which the app writes into its own log.

pub mod args;
pub mod fake;
pub mod folder;
pub mod run;

pub use args::Options;
pub use run::run;

/// The engine's version, reported in its `hello` (the app compares it with the one it expects).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests;
