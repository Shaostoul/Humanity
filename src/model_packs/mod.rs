//! Downloadable 3D model packs (2026-11-09; docs/design/simready-props.md, section 7).
//!
//! The operator, on where the converted NVIDIA SimReady models go: "Sure, let's make sure it's all
//! easy to access." So the home set ships inside the app (`assets/models/simready/`), and the
//! Medical Bay set is the first pack, reached the way the instrument packs and the star catalogue
//! are: from Settings > Graphics > Furniture models, with its size, its licence in plain words,
//! Download / Remove and the download's progress.
//!
//! - **The catalog** is `data/models/packs.ron`, written by `just simready` (scripts/simready/pack.js)
//!   in the same run as the archive, so its size and SHA-256 always match the file uploaded.
//! - **The archive** is a GitHub release asset under the non-`v*` tag `assets-simready-1`
//!   (uploading is the operator's step, docs/ai/release.md), checked against its SHA-256 before
//!   anything is unpacked, then installed into `<data>/models/packs/<id>/` by the instrument packs'
//!   own installer (`instrument_packs::install`: download, check, unpack into a side folder, swap in
//!   whole), so a pack is never half-installed where the models are read.
//! - **A model whose pack is not installed draws as its plain box**, the fallback every machine
//!   model already has (`engine::home_meshes::build_machine_bodies`): nothing is ever missing, only
//!   simpler. Installing or removing a pack rebuilds the home's bodies (`engine::furniture_detail`).
//!
//! **Not a cache.** An installed pack is data the person chose to install, checked against its
//! published SHA-256; it is never worked out again from anything, and removing it changes what is
//! drawn, so nothing stale can hide in it.

/// `data/models/packs.ron`: what ships in the app, and each pack with its file, size and checksum.
pub mod catalog;
/// The Settings section's state, and the frame step that runs its downloads and removals.
pub mod ui;

#[cfg(test)]
mod tests;
