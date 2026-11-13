//! Embeddable AI, rungs 1 and 2: the door, with no model inside it (docs/design/embeddable-ai.md,
//! section 14 items 1 and 2; built 2026-11-09).
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
//! - [`choices`]: what each AI choice needs (data/ai/choices.ron).
//! - [`shared`]: the app's one client, which Settings > AI and the Dev page's AI part drive.
//!
//! RUNG 2 (2026-11-09) adds the room's front door, still holding no AI itself:
//! - [`catalog`]: every AI file the app may download, with its size and SHA-256, COMPILED IN from
//!   data/ai/catalog.ron (6.2), so the signed app vouches for every hash.
//! - [`install`]: downloading (resumable), checking (size, SHA-256, parts joined and checked whole),
//!   installing (the engine unpacked from its zip, [`zip`]) and removing (one model, or all AI) (5.2, 5.3).
//! - [`models`]: an installed model checked byte for byte before each load, and what the load asks for.
//!
//! RUNG 3 (2026-11-12), talking characters:
//! - [`talk`]: the project's character prompt, the fixed output shape (reply text and at most one
//!   action the game granted), its check, and the filter of 8.3; the talk card (src/gui/talk/) drives it.
//!
//! The server build never compiles any of it (`native` only): the relay never talks to an AI engine
//! and never learns that a person has one (section 8.1).

pub mod catalog;
pub mod choices;
pub mod client;
pub mod hardware;
pub mod install;
#[cfg(windows)]
pub mod job;
pub mod models;
pub mod paths;
pub mod shared;
pub mod talk;
pub mod zip;

/// The client against the real fake engine; its `fake_engine` also serves the Dev > AI snapshot.
#[cfg(test)]
pub(crate) mod client_tests;
#[cfg(test)]
mod hardware_tests;
/// The talk card's AI half: the persona, the fixed shape, the grants, the filter.
#[cfg(test)]
mod talk_tests;
/// Downloads from a server on this computer, resume, the hash refusal, parts, removal, the zip.
#[cfg(test)]
mod install_tests;
