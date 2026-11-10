//! Embeddable AI, rung 1: the door, with no model (docs/design/embeddable-ai.md, section 14 item 1;
//! built 2026-11-09).
//!
//! The operator, 2026-11-09: "AI needs to be optional, not required", and the model runtime is a
//! separate download too ("Agreed."). So this module holds NO model and NO runtime, and never will: what
//! runs a model is the AI engine, `humanity-ai.exe` (crates/humanity-ai-engine/), a program of its own
//! that the person downloads only by choosing AI in Settings > AI (rung 2), and that this app never
//! depends on (tests/no_ai_in_app_lint.rs holds both). What is here is the door (section 5.1):
//!
//! - [`paths`]: where the engine lives once chosen (`<your files>/ai/engine/<version>/`), and where the
//!   developer's fake engine is.
//! - [`client`]: starting the engine on demand with pipes (its standard input and output, one JSON
//!   message a line, crates/humanity-ai-protocol/; no socket and no listener), inside a Windows job
//!   object that ends it with the app and caps its memory, at below-normal priority; stopping it after
//!   a few idle minutes; and when it crashes, falling back at once, offering "Try again", and after two
//!   failures in a row starting it no more on its own (section 7.2).
//! - [`hardware`]: what this computer has (memory, processor cores and features, graphics card, free
//!   space), read each time Settings > AI draws, and the plain notices of section 4.7.
//! - [`choices`]: what each later AI choice will need (data/ai/choices.ron).
//! - [`shared`]: the app's one client, which Settings > AI and the Dev page's AI part drive.
//!
//! The server build never compiles any of it (`native` only): the relay never talks to an AI engine
//! and never learns that a person has one (section 8.1).

pub mod choices;
pub mod client;
pub mod hardware;
#[cfg(windows)]
pub mod job;
pub mod paths;
pub mod shared;

/// The client against the real fake engine; its `fake_engine` also serves the Dev > AI snapshot.
#[cfg(test)]
pub(crate) mod client_tests;
#[cfg(test)]
mod hardware_tests;
