//! Downloading, checking, installing and removing an instrument pack, the star catalog's way
//! (a background thread, progress the Settings section draws, a retry after a failure), with one
//! more guard the catalog's single file did not need: every pack is checked against its published
//! SHA-256 before anything is unpacked, and a damaged one is refused and deleted.
//!
//! On disk, in `<data>/music/packs/`:
//! - `<id>.download` while the archive arrives (deleted after);
//! - `<id>.installing/` while it is unpacked (renamed to `<id>/` only once complete);
//! - `<id>/`, the installed pack: `pack.ron`, `instruments/`, `samples/`.
//!
//! A pack is never half-installed where the music reads: either `<id>/` is the old pack, the new
//! one, or absent.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::catalog::PackEntry;

/// Where a download is.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Connecting,
    Downloading,
    /// Checking the SHA-256.
    Checking,
    /// Unpacking and moving into place.
    Installing,
    Done,
    Failed(Failure),
}

/// Why an install failed, so the Settings section can say it in plain words (by key) with the
/// technical detail after.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// The server could not be reached or refused (detail: the error).
    Network(String),
    /// The archive arrived, but its size or SHA-256 is not the published one.
    Damaged(String),
    /// The disk refused (detail: the error).
    Disk(String),
    /// The archive is not a pack of this catalog (detail: what was wrong).
    NotAPack(String),
    /// The pack is listed but not built yet (no checksum to check against).
    NotPublished,
}

impl Failure {
    pub fn detail(&self) -> &str {
        match self {
            Failure::Network(d) | Failure::Damaged(d) | Failure::Disk(d) | Failure::NotAPack(d) => d,
            Failure::NotPublished => "",
        }
    }
}

/// A download's progress: bytes so far, bytes in all (0 until known), and the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub stage: Stage,
}

pub type ProgressHandle = Arc<Mutex<Progress>>;

pub fn new_progress() -> ProgressHandle {
    Arc::new(Mutex::new(Progress { done: 0, total: 0, stage: Stage::Connecting }))
}

fn set(progress: &ProgressHandle, f: impl FnOnce(&mut Progress)) {
    if let Ok(mut p) = progress.lock() {
        f(&mut p);
    }
}

/// `pack.ron` at the top of every pack.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PackManifest {
    pub id: String,
    pub version: u32,
    pub instruments: Vec<String>,
}

/// An installed pack: its version and the bytes it takes on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledPack {
    pub version: u32,
    pub bytes: u64,
}

fn dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_bytes(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// Read an installed pack's manifest.
pub fn manifest(pack_dir: &Path) -> Result<PackManifest, String> {
    let path = pack_dir.join("pack.ron");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every installed pack under `packs_dir`, by id (a folder with a readable `pack.ron` naming
/// itself). Leftovers of an interrupted install (`*.download`, `*.installing`) are not packs.
pub fn installed_packs(packs_dir: &Path) -> BTreeMap<String, InstalledPack> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(packs_dir) else { return out };
    for e in entries.flatten() {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        if !path.is_dir() || name.contains('.') {
            continue;
        }
        if let Ok(m) = manifest(&path) {
            if m.id == name {
                out.insert(name, InstalledPack { version: m.version, bytes: dir_bytes(&path) });
            }
        }
    }
    out
}

fn sibling(packs_dir: &Path, id: &str, ext: &str) -> PathBuf {
    packs_dir.join(format!("{id}.{ext}"))
}

/// Download `entry` from `url` into `packs_dir` and install it, reporting into `progress`. Ends in
/// `Stage::Done` or `Stage::Failed`, with nothing left behind but the installed pack (or, after a
/// failure, whatever was installed before).
pub fn download_and_install(url: &str, entry: &PackEntry, packs_dir: &Path, progress: &ProgressHandle) -> Result<(), Failure> {
    let result = (|| {
        if !entry.is_built() {
            return Err(Failure::NotPublished);
        }
        std::fs::create_dir_all(packs_dir).map_err(|e| Failure::Disk(format!("{}: {e}", packs_dir.display())))?;
        let resp = ureq::get(url)
            .set("User-Agent", "HumanityOS")
            .call()
            .map_err(|e| Failure::Network(e.to_string()))?;
        let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(entry.bytes);
        set(progress, |p| {
            p.total = total;
            p.stage = Stage::Downloading;
        });
        let mut reader = resp.into_reader();
        install_from_reader(&mut reader, entry, packs_dir, progress)
    })();
    set(progress, |p| {
        p.stage = match &result {
            Ok(()) => Stage::Done,
            Err(f) => Stage::Failed(f.clone()),
        }
    });
    result
}

/// The install itself, from any reader of the archive's bytes: stream it to `<id>.download` while
/// hashing, refuse a size or SHA-256 that is not the catalog's, unpack into `<id>.installing/`,
/// check its `pack.ron`, then swap it in for `<id>/`.
pub fn install_from_reader(reader: &mut impl Read, entry: &PackEntry, packs_dir: &Path, progress: &ProgressHandle) -> Result<(), Failure> {
    let download = sibling(packs_dir, &entry.id, "download");
    let installing = sibling(packs_dir, &entry.id, "installing");
    let cleanup = || {
        let _ = std::fs::remove_file(&download);
        let _ = std::fs::remove_dir_all(&installing);
    };
    let result = (|| {
        std::fs::create_dir_all(packs_dir).map_err(|e| Failure::Disk(format!("{}: {e}", packs_dir.display())))?;
        let mut file = std::fs::File::create(&download).map_err(|e| Failure::Disk(format!("{}: {e}", download.display())))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        let mut done: u64 = 0;
        loop {
            let n = reader.read(&mut buf).map_err(|e| Failure::Network(format!("the download stopped: {e}")))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Failure::Disk(format!("{}: {e}", download.display())))?;
            done += n as u64;
            set(progress, |p| p.done = done);
            if done > entry.bytes {
                return Err(Failure::Damaged(format!("more than the published {} bytes arrived", entry.bytes)));
            }
        }
        drop(file);
        set(progress, |p| p.stage = Stage::Checking);
        if done != entry.bytes {
            return Err(Failure::Damaged(format!("{done} bytes arrived, the pack is {} bytes", entry.bytes)));
        }
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(&entry.sha256) {
            return Err(Failure::Damaged(format!("its SHA-256 is {got}, the published one is {}", entry.sha256)));
        }
        set(progress, |p| p.stage = Stage::Installing);
        let _ = std::fs::remove_dir_all(&installing);
        std::fs::create_dir_all(&installing).map_err(|e| Failure::Disk(format!("{}: {e}", installing.display())))?;
        let mut archive = std::io::BufReader::new(
            std::fs::File::open(&download).map_err(|e| Failure::Disk(format!("{}: {e}", download.display())))?,
        );
        super::tar::extract(&mut archive, &installing).map_err(Failure::NotAPack)?;
        let m = manifest(&installing).map_err(Failure::NotAPack)?;
        if m.id != entry.id || m.version != entry.version {
            return Err(Failure::NotAPack(format!(
                "it says it is {} version {}, the catalog lists {} version {}",
                m.id, m.version, entry.id, entry.version
            )));
        }
        let target = packs_dir.join(&entry.id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| Failure::Disk(format!("{}: {e}", target.display())))?;
        }
        std::fs::rename(&installing, &target).map_err(|e| Failure::Disk(format!("{}: {e}", target.display())))?;
        Ok(())
    })();
    cleanup();
    result
}

/// Remove an installed pack (and any leftovers of an interrupted install of it).
pub fn remove_pack(packs_dir: &Path, id: &str) -> Result<(), String> {
    super::tar::safe_path(id)?;
    let _ = std::fs::remove_file(sibling(packs_dir, id, "download"));
    let _ = std::fs::remove_dir_all(sibling(packs_dir, id, "installing"));
    let dir = packs_dir.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(())
}
