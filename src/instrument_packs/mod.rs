//! The free instrument library (2026-11-06; docs/design/music.md rung 6, the operator's "let's
//! build up our free library" and "in the settings we can have a section for filling out the
//! instrument list, kind of like we do with the star catalog and skybox").
//!
//! Real recorded instruments, from public-domain (CC0) sample libraries, for the live score:
//!
//! - **The core set ships inside the app**: a small piano, a harp, bells, a vibraphone, glass,
//!   string pads (`data/music/instruments/*.ron` and `data/music/samples/<id>/`).
//! - **Downloadable packs by family** (keys, strings, mallets, guitars, percussion): one archive
//!   each, published as GitHub release assets like the star catalog's tiers, listed with their
//!   size and SHA-256 in `data/music/packs.ron`, installed from Settings > Audio > Instruments
//!   into `<data>/music/packs/<id>/`, and removed from there.
//! - **The music always plays**: for each part of the score (piano, harp, strings, bells...) the
//!   engine takes the best instrument installed ([`library::InstrumentLibrary::choose`]) and,
//!   when there is none, that part's synthesised voice from `data/music/parts.ron`.
//!
//! Everything here is built by `just instrument-packs` (scripts/instrument-packs.js) from the raw
//! libraries, whose downloads are recorded in docs/reference/instrument-sources.md.
//!
//! **Not a cache.** An installed pack is data the person chose to install, the way a program is
//! installed: it is never worked out again from anything, it is checked against its published
//! SHA-256 before it lands, and removing it changes what plays (that part falls back to the next
//! instrument or to synthesis), so nothing stale can hide in it. What is kept in memory is decoded
//! sample audio: a score holds the recordings it was built with, and [`samples::SampleBank`], the
//! path the music decodes through (engine/music.rs's build, BUG-267), keeps them between builds
//! while the Dev page's "turn off all reuse" switch is on; off, every build (one a phrase) decodes
//! every file again and the bank keeps nothing.
//!
//! The website does not play music yet (music.md rung 9 compiles the score to WebAssembly), so it
//! has no Instruments list; when it plays music, it mirrors this section.

/// `data/music/packs.ron` (the packs, their sizes and checksums) and `data/music/parts.ron` (the
/// parts of the score and each one's synthesised stand-in).
pub mod catalog;
/// An instrument definition (`data/music/instruments/<id>.ron`), the zones of its samples and the
/// playback rate that tunes each one to A = 432 Hz or 440 Hz.
pub mod definition;
/// Downloading, checking, installing and removing a pack.
pub mod install;
/// Every instrument available now (the core set and the installed packs), and the best for a part.
pub mod library;
/// Decoded sample audio and a voice that plays it.
pub mod samples;
/// The bridge into the live score: the best recording of each part its roles name, decoded into the
/// engine's sample bank and levelled to the synthesised voice it replaces (2026-11-06).
pub mod score;
/// Reading a pack archive (plain ustar), refusing anything that could write outside its folder.
pub mod tar;
/// The Settings section's state, and the frame step that runs its downloads and removals.
pub mod ui;
/// Wave sets: single-cycle waves for the synthesiser, kept as plain PCM (2026-11-13, AKWF).
pub mod waves;

#[cfg(test)]
mod install_tests;
#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod data_tests;
