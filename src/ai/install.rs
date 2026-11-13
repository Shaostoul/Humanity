//! Downloading, checking, installing and removing AI (docs/design/embeddable-ai.md, 5.2, 5.3 and 6;
//! rung 2, 2026-11-09). Driven by Settings > AI on a background thread; progress through a shared
//! [`ProgressHandle`] the section draws.
//!
//! On disk, all under the `ai` folder in the person's own files ([`super::paths::ai_root`]):
//! - `downloads/<file>.part` while a file arrives. **Resumable**: a later Get asks the server only for
//!   the bytes still missing (an HTTP `Range` request) and goes on hashing from where it stopped; a
//!   server that ignores the request sends the whole file again, which is simply started over.
//! - every file is **checked against the catalog** (its size, then its SHA-256) as soon as it is whole;
//!   a mismatch is DELETED and said in words (5.2: "The download did not match what we published, so it
//!   was deleted. Try again later.");
//! - **parts** (a file of 2 GiB or more is published in parts, 6.1): each part is checked, then they are
//!   joined in order and the joined file is checked again against the whole file's hash;
//! - `engine/<version>/`: the AI engine, unpacked from its checked zip (src/ai/zip.rs) into
//!   `<version>.installing/` and renamed into place only when complete;
//! - `models/<id>/`: the model file and its licence and credit README, the same way.
//!
//! **The engine comes with the first model, never alone** (5.2): [`install_model`] fetches it when it is
//! missing. **Removing**: [`remove_model`] deletes the model's folder and any half-downloaded files of
//! it, and the engine with the last model (5.3); [`remove_all`] deletes the whole `ai` folder. The
//! running engine must be stopped first (Windows keeps a running program's files and a loaded model's
//! file open), which the Settings section does.
//!
//! Nothing here is a cache: an installed file is what the person chose to install, checked against the
//! signed catalog, and removing it changes what the app can do.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use super::catalog::{AiCatalog, ModelEntry, Part};

/// The engine program's file name inside its folder.
pub use super::paths::ENGINE_EXE;

/// Where a job is.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Connecting,
    /// Fetching this file (`resumed_from` bytes were already on disk).
    Downloading { file: String, resumed_from: u64 },
    /// Checking a file's SHA-256.
    Checking { file: String },
    /// Joining the parts of a large file, and checking the whole.
    Joining { file: String },
    /// Unpacking the engine, or moving a model into its folder.
    Installing,
    Done,
    Failed(Failure),
}

/// Why a job stopped, so the section can say it in plain words (by key) with the detail after.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// The server could not be reached or refused (detail: the error). The part already fetched is kept.
    Network(String),
    /// A file arrived but its size or SHA-256 is not the catalog's: it was deleted.
    Damaged(String),
    /// The disk refused (detail: the error).
    Disk(String),
    /// The engine's zip could not be unpacked.
    BadZip(String),
    /// The person pressed Stop. The part already fetched is kept, so Get resumes.
    Stopped,
}

impl Failure {
    /// The key of the words saying it.
    pub fn words_key(&self) -> &'static str {
        match self {
            Failure::Network(_) => "ai.download.network",
            Failure::Damaged(_) => "ai.download.damaged",
            Failure::Disk(_) => "ai.download.disk",
            Failure::BadZip(_) => "ai.download.bad_zip",
            Failure::Stopped => "ai.download.stopped",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Failure::Network(d) | Failure::Damaged(d) | Failure::Disk(d) | Failure::BadZip(d) => d,
            Failure::Stopped => "",
        }
    }
}

/// A job's progress: bytes so far and in all (every file of the job), and the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub stage: Stage,
}

/// Shared between the job's thread and the section. `stop` asks the job to stop between reads.
#[derive(Debug)]
pub struct Job {
    pub progress: Mutex<Progress>,
    pub stop: AtomicBool,
}

pub type ProgressHandle = Arc<Job>;

pub fn new_job(total: u64) -> ProgressHandle {
    Arc::new(Job { progress: Mutex::new(Progress { done: 0, total, stage: Stage::Connecting }), stop: AtomicBool::new(false) })
}

impl Job {
    pub fn snapshot(&self) -> Progress {
        self.progress.lock().map(|p| p.clone()).unwrap_or(Progress { done: 0, total: 0, stage: Stage::Connecting })
    }

    fn set(&self, f: impl FnOnce(&mut Progress)) {
        if let Ok(mut p) = self.progress.lock() {
            f(&mut p);
        }
    }

    /// Is it still moving (not finished, not failed)?
    pub fn running(&self) -> bool {
        !matches!(self.snapshot().stage, Stage::Done | Stage::Failed(_))
    }
}

/// The folders under the `ai` folder.
pub fn downloads_dir(root: &Path) -> PathBuf {
    root.join("downloads")
}

pub fn engine_dir(root: &Path, version: &str) -> PathBuf {
    root.join("engine").join(version)
}

pub fn model_dir(root: &Path, id: &str) -> PathBuf {
    root.join("models").join(id)
}

/// The model file of an installed model.
pub fn model_file(root: &Path, m: &ModelEntry) -> PathBuf {
    model_dir(root, &m.id).join(&m.file)
}

fn bytes_in(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                bytes_in(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// What is installed under `root`, against the catalog.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Installed {
    /// The engine of the catalog's version, and the bytes its folder takes.
    pub engine: Option<u64>,
    /// Each installed model, by id, and the bytes its folder takes.
    pub models: BTreeMap<String, u64>,
    /// Bytes of half-finished downloads waiting to be resumed.
    pub partial: u64,
}

impl Installed {
    pub fn nothing(&self) -> bool {
        self.engine.is_none() && self.models.is_empty() && self.partial == 0
    }
}

/// Read what is installed: the engine (its program in its version's folder) and each catalog model
/// whose file is in its folder at the catalog's size. A model is checked byte for byte before each
/// load ([`verify`]), not here (this is read whenever the section draws).
pub fn installed(cat: &AiCatalog, root: &Path) -> Installed {
    let eng = engine_dir(root, &cat.engine.version);
    let engine = eng.join(ENGINE_EXE).is_file().then(|| bytes_in(&eng));
    let models = cat
        .models
        .iter()
        .filter(|m| std::fs::metadata(model_file(root, m)).map(|md| md.len() == m.bytes).unwrap_or(false))
        .map(|m| (m.id.clone(), bytes_in(&model_dir(root, &m.id))))
        .collect();
    Installed { engine, models, partial: bytes_in(&downloads_dir(root)) }
}

/// One file to fetch.
#[derive(Debug, Clone)]
struct Fetch {
    file: String,
    bytes: u64,
    sha256: String,
}

impl From<&Part> for Fetch {
    fn from(p: &Part) -> Self {
        Fetch { file: p.file.clone(), bytes: p.bytes, sha256: p.sha256.clone() }
    }
}

/// How many bytes Get will download for `model_id`: the model, its extras, and the engine when it is
/// not installed yet (5.2: "its size added to the total shown before the click").
pub fn download_total(cat: &AiCatalog, model: &ModelEntry, root: &Path) -> u64 {
    let engine = if installed(cat, root).engine.is_some() { 0 } else { cat.engine.bytes };
    engine + model.download_bytes()
}

/// Hash a file's first `n` bytes into `hasher` (the part already fetched, when a download resumes).
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

/// Fetch one file into `downloads/<file>.part`, resuming what is already there, then check it. Ok: the
/// path of the checked file (still named .part, its caller moves it). `done_before` is the job's bytes
/// before this file, so the bar counts the whole job.
fn fetch(base: &str, f: &Fetch, downloads: &Path, job: &Job, done_before: u64) -> Result<PathBuf, Failure> {
    std::fs::create_dir_all(downloads).map_err(|e| Failure::Disk(format!("{}: {e}", downloads.display())))?;
    let part = downloads.join(format!("{}.part", f.file));
    let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have > f.bytes {
        let _ = std::fs::remove_file(&part);
        have = 0;
    }
    let mut hasher = Sha256::new();
    if have > 0 && hash_prefix(&part, have, &mut hasher).is_err() {
        let _ = std::fs::remove_file(&part);
        have = 0;
        hasher = Sha256::new();
    }
    if have < f.bytes {
        let url = super::catalog::url(base, &f.file);
        job.set(|p| p.stage = Stage::Connecting);
        let mut req = ureq::get(&url).set("User-Agent", "HumanityOS");
        if have > 0 {
            req = req.set("Range", &format!("bytes={have}-"));
        }
        let resp = req.call().map_err(|e| Failure::Network(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(&part).map_err(|e| Failure::Disk(format!("{}: {e}", part.display())))?;
        let resumed = resp.status() == 206 && resp.header("Content-Range").is_some_and(|r| r.starts_with(&format!("bytes {have}-")));
        if resumed {
            file.seek(SeekFrom::Start(have)).map_err(|e| Failure::Disk(e.to_string()))?;
        } else {
            // The server sent the whole file (it ignored the Range, or there was nothing to resume).
            file.set_len(0).map_err(|e| Failure::Disk(e.to_string()))?;
            file.seek(SeekFrom::Start(0)).map_err(|e| Failure::Disk(e.to_string()))?;
            have = 0;
            hasher = Sha256::new();
        }
        job.set(|p| {
            p.stage = Stage::Downloading { file: f.file.clone(), resumed_from: have };
            p.done = done_before + have;
        });
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            if job.stop.load(Ordering::Relaxed) {
                return Err(Failure::Stopped);
            }
            let n = reader.read(&mut buf).map_err(|e| Failure::Network(format!("the download stopped: {e}")))?;
            if n == 0 {
                break;
            }
            if have + n as u64 > f.bytes {
                drop(file);
                let _ = std::fs::remove_file(&part);
                return Err(Failure::Damaged(format!("{}: more than its published {} bytes arrived", f.file, f.bytes)));
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Failure::Disk(format!("{}: {e}", part.display())))?;
            have += n as u64;
            job.set(|p| p.done = done_before + have);
        }
        file.flush().map_err(|e| Failure::Disk(e.to_string()))?;
    }
    job.set(|p| p.stage = Stage::Checking { file: f.file.clone() });
    if have != f.bytes {
        // Short: keep it, a later Get resumes from here.
        return Err(Failure::Network(format!("{}: {have} of its {} bytes arrived", f.file, f.bytes)));
    }
    let got = hex::encode(hasher.finalize());
    if got != f.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(Failure::Damaged(format!("{}: its SHA-256 is {got}, the published one is {}", f.file, f.sha256)));
    }
    Ok(part)
}

/// Fetch a file published whole or in parts; Ok: the checked whole file in `downloads/`.
fn fetch_whole(base: &str, name: &str, bytes: u64, sha256: &str, parts: &[Part], downloads: &Path, job: &Job, done_before: u64) -> Result<PathBuf, Failure> {
    if parts.is_empty() {
        return fetch(base, &Fetch { file: name.to_string(), bytes, sha256: sha256.to_string() }, downloads, job, done_before);
    }
    let mut paths = Vec::with_capacity(parts.len());
    let mut done = done_before;
    for p in parts {
        paths.push(fetch(base, &Fetch::from(p), downloads, job, done)?);
        done += p.bytes;
    }
    join_parts(&paths, name, bytes, sha256, downloads, job)
}

/// Join checked parts, in order, into `downloads/<name>.joined`, hashing as it goes; the whole must match
/// the whole file's hash (6.2: "after joining, the whole file is checked against the whole-file hash").
/// The parts are deleted once the whole is good; a bad whole is deleted with them.
pub fn join_parts(parts: &[PathBuf], name: &str, bytes: u64, sha256: &str, downloads: &Path, job: &Job) -> Result<PathBuf, Failure> {
    job.set(|p| p.stage = Stage::Joining { file: name.to_string() });
    let out = downloads.join(format!("{name}.joined"));
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&out).map_err(|e| Failure::Disk(format!("{}: {e}", out.display())))?);
        let mut buf = vec![0u8; 1 << 20];
        for p in parts {
            let mut r = std::fs::File::open(p).map_err(|e| Failure::Disk(format!("{}: {e}", p.display())))?;
            loop {
                let n = r.read(&mut buf).map_err(|e| Failure::Disk(e.to_string()))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                w.write_all(&buf[..n]).map_err(|e| Failure::Disk(e.to_string()))?;
                total += n as u64;
            }
        }
        w.flush().map_err(|e| Failure::Disk(e.to_string()))?;
    }
    let got = hex::encode(hasher.finalize());
    let bad = total != bytes || got != sha256;
    for p in parts {
        let _ = std::fs::remove_file(p);
    }
    if bad {
        let _ = std::fs::remove_file(&out);
        return Err(Failure::Damaged(format!("{name}: the joined parts are {total} bytes with SHA-256 {got}; the published whole is {bytes} bytes, {sha256}")));
    }
    Ok(out)
}

/// Download, check and unpack the catalog's engine into `engine/<version>/`, then delete every OTHER
/// engine version folder: this app only ever starts its catalog's version (`paths::installed_engine`),
/// so an older one would be gigabyte-free but dead weight that Remove all AI is the only way out of
/// (7.4; 2026-11-12, the first engine update, 1.0.0 to 1.1.0). The progress counts the engine as
/// the first bytes of the job.
fn install_engine(base: &str, cat: &AiCatalog, root: &Path, job: &Job) -> Result<(), Failure> {
    let e = &cat.engine;
    let downloads = downloads_dir(root);
    let zip_path = fetch_whole(base, &e.file, e.bytes, &e.sha256, &e.parts, &downloads, job, 0)?;
    job.set(|p| p.stage = Stage::Installing);
    let zip = std::fs::read(&zip_path).map_err(|err| Failure::Disk(format!("{}: {err}", zip_path.display())))?;
    let target = engine_dir(root, &e.version);
    let staging = target.with_extension("installing");
    let _ = std::fs::remove_dir_all(&staging);
    super::zip::unpack(&zip, &staging).map_err(|err| {
        let _ = std::fs::remove_dir_all(&staging);
        Failure::BadZip(err)
    })?;
    if !staging.join(ENGINE_EXE).is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(Failure::BadZip(format!("the zip has no {ENGINE_EXE}")));
    }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&staging, &target).map_err(|err| Failure::Disk(format!("{}: {err}", target.display())))?;
    let _ = std::fs::remove_file(&zip_path);
    log::info!("[AI] the AI engine {} is installed in {}", e.version, target.display());
    if let Ok(rd) = std::fs::read_dir(root.join("engine")) {
        for old in rd.flatten().map(|d| d.path()).filter(|p| p.is_dir() && *p != target) {
            match std::fs::remove_dir_all(&old) {
                Ok(()) => log::info!("[AI] removed the engine this app no longer starts: {}", old.display()),
                Err(err) => log::warn!("[AI] could not remove the old engine {}: {err}", old.display()),
            }
        }
    }
    Ok(())
}

/// Bring the engine up to the catalog's version while models stay as they are (Settings > AI's "An
/// update to the AI engine is ready", 7.4): the person's models were installed with an earlier app
/// whose catalog named an earlier engine. Ends in Done or Failed, as [`install_model`] does.
pub fn update_engine(base: &str, cat: &AiCatalog, root: &Path, job: &Job) -> Result<(), Failure> {
    let result = install_engine(base, cat, root, job).map(|()| {
        let _ = std::fs::remove_dir(downloads_dir(root));
    });
    job.set(|p| {
        p.stage = match &result {
            Ok(()) => Stage::Done,
            Err(f) => Stage::Failed(f.clone()),
        }
    });
    result
}

/// Download and install `model_id`, with the engine first when it is missing. Ends in Done or Failed.
pub fn install_model(base: &str, cat: &AiCatalog, model_id: &str, root: &Path, job: &Job) -> Result<(), Failure> {
    let result: Result<(), Failure> = (|| {
        let model = cat.model(model_id).ok_or_else(|| Failure::Disk(format!("{model_id} is not in the catalog")))?;
        let downloads = downloads_dir(root);
        let mut done = 0u64;
        if installed(cat, root).engine.is_none() {
            install_engine(base, cat, root, job)?;
            done += cat.engine.bytes;
        }
        let file = fetch_whole(base, &model.file, model.bytes, &model.sha256, &model.parts, &downloads, job, done)?;
        done += model.bytes;
        let mut extras = Vec::new();
        for x in &model.extras {
            extras.push((fetch(base, &Fetch { file: x.file.clone(), bytes: x.bytes, sha256: x.sha256.clone() }, &downloads, job, done)?, x.install_as.clone()));
            done += x.bytes;
        }
        job.set(|p| p.stage = Stage::Installing);
        let target = model_dir(root, &model.id);
        let staging = target.with_extension("installing");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| Failure::Disk(format!("{}: {e}", staging.display())))?;
        std::fs::rename(&file, staging.join(&model.file)).map_err(|e| Failure::Disk(format!("{}: {e}", staging.display())))?;
        for (from, name) in extras {
            std::fs::rename(&from, staging.join(name)).map_err(|e| Failure::Disk(format!("{}: {e}", staging.display())))?;
        }
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&staging, &target).map_err(|e| Failure::Disk(format!("{}: {e}", target.display())))?;
        let _ = std::fs::remove_dir(&downloads);
        log::info!("[AI] {} is installed in {}", model.model, target.display());
        Ok(())
    })();
    job.set(|p| {
        p.stage = match &result {
            Ok(()) => Stage::Done,
            Err(f) => Stage::Failed(f.clone()),
        }
    });
    result
}

/// Remove one model: its folder, any half-downloaded files of it, and, when no model is left, the
/// engine and the `ai` folder itself (5.3). Returns what it deleted.
pub fn remove_model(cat: &AiCatalog, root: &Path, model_id: &str) -> Result<Vec<PathBuf>, String> {
    let model = cat.model(model_id).ok_or_else(|| format!("{model_id} is not in the catalog"))?;
    let mut gone = Vec::new();
    let mut rm = |p: PathBuf| -> Result<(), String> {
        if p.is_dir() {
            std::fs::remove_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            gone.push(p);
        } else if p.exists() {
            std::fs::remove_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            gone.push(p);
        }
        Ok(())
    };
    let dir = model_dir(root, model_id);
    rm(dir.clone())?;
    rm(dir.with_extension("installing"))?;
    let downloads = downloads_dir(root);
    rm(downloads.join(format!("{}.part", model.file)))?;
    rm(downloads.join(format!("{}.joined", model.file)))?;
    for p in &model.parts {
        rm(downloads.join(format!("{}.part", p.file)))?;
    }
    for x in &model.extras {
        rm(downloads.join(format!("{}.part", x.file)))?;
    }
    let models_left = std::fs::read_dir(root.join("models")).map(|r| r.flatten().count()).unwrap_or(0);
    if models_left == 0 {
        rm(root.join("models"))?;
        rm(root.join("engine"))?;
        rm(downloads_dir(root))?;
        if std::fs::read_dir(root).map(|mut r| r.next().is_none()).unwrap_or(false) {
            rm(root.to_path_buf())?;
        }
    }
    Ok(gone)
}

/// Remove all AI: the whole `ai` folder (5.3).
pub fn remove_all(root: &Path) -> Result<(), String> {
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    }
    Ok(())
}

/// Check a file byte for byte against its size and SHA-256 (before each load, 6.2). Err in words.
pub fn verify(path: &Path, bytes: u64, sha256: &str) -> Result<(), String> {
    let len = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if len != bytes {
        return Err(format!("{} is {len} bytes, the catalog's is {bytes}", path.display()));
    }
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex::encode(hasher.finalize());
    if got != sha256 {
        return Err(format!("{}: SHA-256 {got}, the catalog's is {sha256}", path.display()));
    }
    Ok(())
}

/// The threads a brain gets (9.2): physical cores minus two, never more than four, at least one.
pub fn brain_threads(cores: Option<u32>, threads: u32) -> u32 {
    cores.unwrap_or(threads).saturating_sub(2).clamp(1, 4)
}
