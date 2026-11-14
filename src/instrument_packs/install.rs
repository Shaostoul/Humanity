//! Downloading, checking, installing and removing an instrument pack, the star catalog's way
//! (a background thread, progress the Settings section draws, a retry after a failure), with one
//! more guard the catalog's single file did not need: every pack is checked against its published
//! SHA-256 before anything is unpacked, and a damaged one is refused and deleted.
//!
//! On disk, in `<data>/music/packs/` (and the model and garment texture packs' folders, which use
//! this same installer):
//! - `<id>.download` while the archive arrives (deleted after it installs or is refused; kept when
//!   the connection dropped, so the next download resumes from it: `download_and_install`);
//! - `<id>.installing/` while it is unpacked (renamed to `<id>/` only once complete);
//! - `<id>/`, the installed pack: `pack.ron`, `instruments/`, `samples/`.
//!
//! A pack is never half-installed where the music reads: either `<id>/` is the old pack, the new
//! one, or absent.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
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
    /// Its wave sets (`waves/<id>/`, 2026-11-13).
    #[serde(default)]
    pub waves: Vec<String>,
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

/// Hash a file's first `n` bytes into `hasher` (the part a stopped download left, when it resumes).
fn hash_prefix(path: &Path, n: u64, hasher: &mut Sha256) -> std::io::Result<()> {
    let mut f = std::fs::File::open(path)?;
    let mut left = n;
    let mut buf = vec![0u8; 1 << 20];
    while left > 0 {
        let want = buf.len().min(left as usize);
        let got = f.read(&mut buf[..want])?;
        if got == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the partial file is shorter than it was"));
        }
        hasher.update(&buf[..got]);
        left -= got as u64;
    }
    Ok(())
}

/// The size of the whole file the server holds, from its answer: a 206's `Content-Range` ("bytes
/// N-M/TOTAL") or a 200's `Content-Length`. None when it does not say.
fn server_total(resp: &ureq::Response) -> Option<u64> {
    if resp.status() == 206 {
        return resp.header("Content-Range").and_then(|r| r.rsplit('/').next()).and_then(|t| t.trim().parse::<u64>().ok());
    }
    resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok())
}

/// Download `entry` from `url` into `packs_dir` and install it, reporting into `progress`. Ends in
/// `Stage::Done` or `Stage::Failed`.
///
/// **Resumable (2026-11-10, the garment texture packs: a 2K set is hundreds of megabytes).** The
/// archive arrives in `<id>.download`. When the connection drops before the whole file is in, what
/// arrived is KEPT and the failure is `Network`: the next try asks the server only for the bytes still
/// missing (an HTTP `Range` request) and goes on hashing from where it stopped, the AI files' way
/// (`ai::install`); a server that ignores the request sends the whole file, which simply starts over.
/// What is refused and deleted: a server whose file is not the published size (`Damaged`, nothing to
/// resume), more bytes than published, and a whole file whose SHA-256 is not the published one. After
/// a refusal nothing is left but whatever was installed before.
pub fn download_and_install(url: &str, entry: &PackEntry, packs_dir: &Path, progress: &ProgressHandle) -> Result<(), Failure> {
    let result = (|| {
        if !entry.is_built() {
            return Err(Failure::NotPublished);
        }
        std::fs::create_dir_all(packs_dir).map_err(|e| Failure::Disk(format!("{}: {e}", packs_dir.display())))?;
        let download = sibling(packs_dir, &entry.id, "download");
        let discard = || {
            let _ = std::fs::remove_file(&download);
        };
        // What a download that stopped short left, hashed again (a file longer than the pack, or one
        // that cannot be read, starts over).
        let mut have = std::fs::metadata(&download).map(|m| m.len()).unwrap_or(0);
        if have > entry.bytes {
            discard();
            have = 0;
        }
        let mut hasher = Sha256::new();
        if have > 0 && hash_prefix(&download, have, &mut hasher).is_err() {
            discard();
            have = 0;
            hasher = Sha256::new();
        }
        if have < entry.bytes {
            let mut req = ureq::get(url).set("User-Agent", "HumanityOS");
            if have > 0 {
                req = req.set("Range", &format!("bytes={have}-"));
            }
            let resp = req.call().map_err(|e| Failure::Network(e.to_string()))?;
            if let Some(total) = server_total(&resp) {
                if total != entry.bytes {
                    discard();
                    return Err(Failure::Damaged(format!("the server's file is {total} bytes, the published pack is {} bytes", entry.bytes)));
                }
            }
            let resumed = resp.status() == 206 && resp.header("Content-Range").is_some_and(|r| r.starts_with(&format!("bytes {have}-")));
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&download)
                .map_err(|e| Failure::Disk(format!("{}: {e}", download.display())))?;
            if resumed {
                file.seek(SeekFrom::Start(have)).map_err(|e| Failure::Disk(e.to_string()))?;
            } else {
                // The whole file is coming (the server ignored the Range, or there was nothing to resume).
                file.set_len(0).map_err(|e| Failure::Disk(e.to_string()))?;
                file.seek(SeekFrom::Start(0)).map_err(|e| Failure::Disk(e.to_string()))?;
                have = 0;
                hasher = Sha256::new();
            }
            set(progress, |p| {
                p.total = entry.bytes;
                p.done = have;
                p.stage = Stage::Downloading;
            });
            let mut reader = resp.into_reader();
            let mut buf = vec![0u8; 1 << 16];
            loop {
                let n = match reader.read(&mut buf) {
                    Ok(n) => n,
                    // Dropped: keep what arrived, the next try resumes from it.
                    Err(e) => return Err(Failure::Network(format!("the download stopped after {have} of {} bytes ({e}); downloading again resumes from there", entry.bytes))),
                };
                if n == 0 {
                    break;
                }
                if have + n as u64 > entry.bytes {
                    drop(file);
                    discard();
                    return Err(Failure::Damaged(format!("more than the published {} bytes arrived", entry.bytes)));
                }
                hasher.update(&buf[..n]);
                file.write_all(&buf[..n]).map_err(|e| Failure::Disk(format!("{}: {e}", download.display())))?;
                have += n as u64;
                set(progress, |p| p.done = have);
            }
            file.flush().map_err(|e| Failure::Disk(e.to_string()))?;
        }
        if have != entry.bytes {
            // Short with the connection closed: kept, the next try resumes.
            return Err(Failure::Network(format!("{have} of the pack's {} bytes arrived; downloading again resumes from there", entry.bytes)));
        }
        set(progress, |p| p.stage = Stage::Checking);
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(&entry.sha256) {
            discard();
            return Err(Failure::Damaged(format!("its SHA-256 is {got}, the published one is {}", entry.sha256)));
        }
        let unpacked = unpack_checked(&download, entry, packs_dir, progress);
        discard();
        unpacked
    })();
    set(progress, |p| {
        p.stage = match &result {
            Ok(()) => Stage::Done,
            Err(f) => Stage::Failed(f.clone()),
        }
    });
    result
}

/// Unpack a checked archive into `<id>.installing/`, check its `pack.ron`, then swap it in for
/// `<id>/`. The side folder is gone afterwards either way.
fn unpack_checked(archive_path: &Path, entry: &PackEntry, packs_dir: &Path, progress: &ProgressHandle) -> Result<(), Failure> {
    let installing = sibling(packs_dir, &entry.id, "installing");
    let result = (|| {
        set(progress, |p| p.stage = Stage::Installing);
        let _ = std::fs::remove_dir_all(&installing);
        std::fs::create_dir_all(&installing).map_err(|e| Failure::Disk(format!("{}: {e}", installing.display())))?;
        let mut archive = std::io::BufReader::new(std::fs::File::open(archive_path).map_err(|e| Failure::Disk(format!("{}: {e}", archive_path.display())))?);
        super::tar::extract(&mut archive, &installing).map_err(Failure::NotAPack)?;
        let m = manifest(&installing).map_err(Failure::NotAPack)?;
        if m.id != entry.id || m.version != entry.version {
            return Err(Failure::NotAPack(format!("it says it is {} version {}, the catalog lists {} version {}", m.id, m.version, entry.id, entry.version)));
        }
        let target = packs_dir.join(&entry.id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| Failure::Disk(format!("{}: {e}", target.display())))?;
        }
        std::fs::rename(&installing, &target).map_err(|e| Failure::Disk(format!("{}: {e}", target.display())))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&installing);
    result
}

/// Bytes of a download that stopped short, waiting to be resumed (0 when there is none).
pub fn partial_bytes(packs_dir: &Path, id: &str) -> u64 {
    std::fs::metadata(sibling(packs_dir, id, "download")).map(|m| m.len()).unwrap_or(0)
}

/// The install from an archive already in hand (any reader of its bytes; the tests' in-memory
/// archives): stream it to `<id>.download` while hashing, refuse a size or SHA-256 that is not the
/// catalog's, unpack into `<id>.installing/`, check its `pack.ron`, then swap it in for `<id>/`. A
/// reader cannot be asked for the rest, so nothing is kept to resume: `download_and_install` resumes.
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
        unpack_checked(&download, entry, packs_dir, progress)
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
