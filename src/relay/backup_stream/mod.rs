//! Continuous backup of the relay's database (2026-10-22, PRIORITIES Tier 2 item 12).
//!
//! Goal: a relay never loses more than about a minute of data, wherever its disk dies.
//! Method (the one Litestream made standard, built here in Rust inside the relay so nothing
//! extra has to be installed or supervised): the database runs in WAL mode, so every committed
//! change is first appended to `relay.db-wal` as whole pages. Every few seconds the capturer
//! (`capture.rs`) reads the new committed frames, seals them (`seal.rs`: compressed, then
//! AES-256-GCM under a key derived from the relay's existing `data/backup.key`) and stages
//! them; the uploader below sends them, in order, to the destination the operator chose
//! (`dest.rs`: a folder, an S3-compatible bucket, or another HumanityOS server, `peer.rs` and
//! `host.rs`). Periodic full snapshots let a restore start somewhere other than the beginning,
//! and retention (`names.rs`) deletes what falls outside the history the operator keeps. A
//! restore (`restore.rs`) replays the frames onto a snapshot, to the newest state or to any
//! moment inside the kept history; the restore drill does exactly that into a scratch file and
//! compares it with the live database, never touching the live file.
//!
//! In the app: Server Settings > ADMIN > Backups (native) and the admin page's Backups panel
//! (web), both through `POST /api/admin/backups` (`api.rs`); every action is listed in
//! `data/admin/ops_registry.json`. Runbook: `docs/admin/continuous-backup.md`.

pub mod api;
pub mod capture;
pub mod dest;
pub mod host;
pub mod names;
pub mod peer;
pub mod restore;
pub mod s3;
pub mod seal;
pub mod wal;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::relay::storage::Storage;
use capture::{Captured, Capturer, SnapJob};
use dest::{Destination, DestinationConfig};
use names::{Catalog, Listed, Target};

/// The configuration file beside the database. It holds an S3 secret when there is one, so the
/// name ends in `.secrets.json` (git ignores that), it is private to the relay's user on unix,
/// and it travels with `backup.key` when a server moves: a restore needs both.
pub const CONFIG_FILE: &str = "backup-destination.secrets.json";
/// How often the capturer looks at the log, and the uploader sends what it staged.
pub const TICK: std::time::Duration = std::time::Duration::from_secs(10);
/// How often the destination is listed again, and retention applied.
const LIST_EVERY_MS: u64 = 10 * 60 * 1000;
/// The longest wait between retries when the destination keeps failing.
const MAX_BACKOFF_MS: u64 = 5 * 60 * 1000;

// ── Configuration ────────────────────────────────────────────────────────────

/// The saved settings (`backup-destination.secrets.json`).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub destination: Option<DestinationConfig>,
    /// How much history is kept: any moment this far back can be restored.
    #[serde(default = "default_retention")]
    pub retention_hours: u32,
    /// How often a fresh full snapshot is taken (restores replay at most this much).
    #[serde(default = "default_snapshot_every")]
    pub snapshot_every_hours: u32,
    /// An alert goes out when the newest copy at the destination is this far behind.
    #[serde(default = "default_alert_after")]
    pub alert_after_minutes: u32,
    /// The seed of this server's backup sending key: what another HumanityOS server keeping
    /// these backups knows it by. Kept here, not in the database, so a restore on a new machine
    /// can still prove who it is before the database exists. Never shown.
    #[serde(default)]
    pub sender_seed: String,
}

pub const RETENTION_RANGE: (u32, u32) = (1, 720);
pub const SNAPSHOT_RANGE: (u32, u32) = (1, 168);
pub const ALERT_RANGE: (u32, u32) = (1, 1440);

fn default_retention() -> u32 {
    72
}
fn default_snapshot_every() -> u32 {
    24
}
fn default_alert_after() -> u32 {
    5
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            destination: None,
            retention_hours: default_retention(),
            snapshot_every_hours: default_snapshot_every(),
            alert_after_minutes: default_alert_after(),
            sender_seed: String::new(),
        }
    }
}

impl std::fmt::Debug for StreamConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamConfig")
            .field("enabled", &self.enabled)
            .field("destination", &self.destination)
            .field("retention_hours", &self.retention_hours)
            .finish_non_exhaustive()
    }
}

impl StreamConfig {
    pub fn active(&self) -> bool {
        self.enabled && self.destination.is_some()
    }

    pub fn sender_seed_bytes(&self) -> Option<[u8; 32]> {
        let b = hex::decode(&self.sender_seed).ok()?;
        (b.len() == 32).then(|| {
            let mut a = [0u8; 32];
            a.copy_from_slice(&b);
            a
        })
    }

    /// The backup sending key's public half, hex: what another server lists to keep these backups.
    pub fn sending_key(&self) -> Option<String> {
        use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
        let seed = self.sender_seed_bytes()?;
        Some(hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
    }

    /// The settings as a client may see them.
    pub fn view(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled,
            "destination": self.destination.as_ref().map(|d| d.view()),
            "retention_hours": self.retention_hours,
            "snapshot_every_hours": self.snapshot_every_hours,
            "alert_after_minutes": self.alert_after_minutes,
        })
    }
}

pub fn load_config(data_dir: &Path) -> Option<StreamConfig> {
    let text = std::fs::read_to_string(data_dir.join(CONFIG_FILE)).ok()?;
    match serde_json::from_str(&text) {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::error!("continuous backup: {} is not readable ({e}); backup stays off until it is fixed or saved again in the app", CONFIG_FILE);
            None
        }
    }
}

pub fn save_config(data_dir: &Path, cfg: &StreamConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    write_file_atomic(&data_dir.join(CONFIG_FILE), text.as_bytes(), true)
}

/// The configuration, with a sending key made if there was none (written at once, so the key
/// shown to an admin is the one that will sign).
pub fn config_with_sender(data_dir: &Path) -> StreamConfig {
    let mut cfg = load_config(data_dir).unwrap_or_default();
    if cfg.sender_seed_bytes().is_none() {
        cfg.sender_seed = hex::encode(rand::random::<[u8; 32]>());
        if let Err(e) = save_config(data_dir, &cfg) {
            tracing::error!("continuous backup: could not save the backup sending key: {e}");
        }
    }
    cfg
}

// ── Small helpers ────────────────────────────────────────────────────────────

/// The folder the capturer keeps its state and staged objects in.
pub fn work_dir(db_path: &Path) -> PathBuf {
    db_path.parent().unwrap_or(Path::new(".")).join("backup-stream")
}

/// The live database's own file, from SQLite itself.
pub fn live_db_path(db: &Storage) -> PathBuf {
    let p: String = db
        .with_conn(|c| c.query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |r| r.get(0)))
        .unwrap_or_default();
    PathBuf::from(p)
}

/// The folder the live database is in (`data/` on a standard install).
pub fn live_data_dir(db: &Storage) -> PathBuf {
    live_db_path(db).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// Write a file whole or not at all: a temporary file, flushed, then renamed over the name.
/// `private` makes it readable by the relay's own user only (unix).
pub fn write_file_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("no folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("making {}: {e}", parent.display()))?;
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = parent.join(format!("{name}.{:08x}.tmp", rand::random::<u32>()));
    let res = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)?;
        dest::sync_dir(parent);
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("writing {}: {e}", path.display()));
    }
    Ok(())
}

pub fn human_bytes(n: u64) -> String {
    let f = n as f64;
    if f >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", f / (1024.0 * 1024.0 * 1024.0))
    } else if f >= 1024.0 * 1024.0 {
        format!("{:.1} MB", f / (1024.0 * 1024.0))
    } else if f >= 1024.0 {
        format!("{:.0} KB", f / 1024.0)
    } else {
        format!("{n} bytes")
    }
}

/// Days since 1970-01-01 to (year, month, day) (Howard Hinnant's civil_from_days).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) to days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-10-22 14:02:11 UTC`
pub fn utc_text(ms: u64) -> String {
    if ms == 0 {
        return "never".into();
    }
    let s = ms / 1000;
    let (y, mo, d) = civil_from_days((s / 86_400) as i64);
    let t = s % 86_400;
    format!("{y:04}-{mo:02}-{d:02} {:02}:{:02}:{:02} UTC", t / 3600, (t % 3600) / 60, t % 60)
}

/// Unix milliseconds, or `YYYY-MM-DDTHH:MM:SSZ` (also with a space for the T, or without the Z).
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        return s.parse().ok();
    }
    let s = s.trim_end_matches('Z').trim_end_matches(" UTC");
    let (date, time) = s.split_once(['T', ' '])?;
    let mut dp = date.split('-');
    let (y, mo, d): (i64, u32, u32) = (dp.next()?.parse().ok()?, dp.next()?.parse().ok()?, dp.next()?.parse().ok()?);
    let mut tp = time.split(':');
    let (h, mi): (u64, u64) = (tp.next()?.parse().ok()?, tp.next()?.parse().ok()?);
    let sec: u64 = tp.next().map(|x| x.parse().ok()).unwrap_or(Some(0))?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    if days < 0 {
        return None;
    }
    Some((days as u64 * 86_400 + h * 3600 + mi * 60 + sec) * 1000)
}

fn ago(now: u64, then: u64) -> String {
    let s = now.saturating_sub(then) / 1000;
    if s < 90 {
        format!("{s} s ago")
    } else if s < 90 * 60 {
        format!("{} min ago", s / 60)
    } else if s < 48 * 3600 {
        format!("{} h ago", s / 3600)
    } else {
        format!("{} days ago", s / 86_400)
    }
}

// ── Shared state ─────────────────────────────────────────────────────────────

/// The result of the last restore drill.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DrillStatus {
    pub running: bool,
    pub started_ms: u64,
    pub finished_ms: u64,
    /// None until a drill has finished.
    pub ok: Option<bool>,
    pub words: String,
    pub restored_to_ms: u64,
    pub duration_ms: u64,
    pub db_bytes: u64,
}

/// What the Backups panel shows.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub enabled: bool,
    /// off | starting | up_to_date | catching_up | failing | behind | refused
    pub state: String,
    /// One plain sentence saying what is going on.
    pub words: String,
    pub generation: String,
    pub seq: u64,
    pub last_capture_ms: u64,
    /// The last time anything reached the destination.
    pub last_copy_ms: u64,
    pub pending_objects: u64,
    pub pending_bytes: u64,
    pub oldest_pending_ms: u64,
    /// The newest moment the destination can restore to; how much would be lost if this
    /// machine died now is the time since then.
    pub restorable_to_ms: u64,
    /// The oldest moment it can restore to.
    pub restorable_from_ms: u64,
    pub lag_secs: u64,
    pub last_error: String,
    pub failing_since_ms: u64,
    pub dest_objects: u64,
    pub dest_bytes: u64,
    pub dest_checked_ms: u64,
    pub snapshots: u64,
    pub generations: u64,
    pub last_snapshot_ms: u64,
    pub retention_hours: u32,
    pub snapshot_every_hours: u32,
    pub alert_after_minutes: u32,
    pub key_fingerprint: String,
    pub alert_active: bool,
    pub last_alert: String,
    pub last_alert_ms: u64,
    pub refused: String,
    pub drill: DrillStatus,
}

/// Requests from the app and the relay to the shipper.
#[derive(Debug, Default)]
pub struct Ctl {
    pub config: StreamConfig,
    /// Goes up whenever the configuration changes, so the loop rebuilds its destination.
    pub version: u64,
    pub force_snapshot: bool,
    pub closed: bool,
    pub enabled_ms: u64,
}

type AlertSink = Box<dyn Fn(&str, &str) + Send + Sync>;

pub struct Shared {
    pub(crate) cap: Mutex<Option<Capturer>>,
    pub(crate) status: Mutex<Status>,
    pub(crate) ctl: Mutex<Ctl>,
    pub(crate) wake: tokio::sync::Notify,
    /// Held while objects are being sent or deleted, so the loop and a drill never send the
    /// same staged object twice at once.
    pub(crate) io: tokio::sync::Mutex<()>,
    /// What is known to be at the destination: the last listing plus what was sent since.
    pub(crate) known: Mutex<Option<Vec<Listed>>>,
    /// Tests replace the alert channel; None sends through `scripts/humanity-alert.js`.
    pub(crate) alert_sink: Mutex<Option<AlertSink>>,
}

/// The continuous-backup state a `Storage` carries. Cheap to clone (an `Arc`).
#[derive(Clone)]
pub struct Slot(pub(crate) Arc<Shared>);

impl Default for Slot {
    fn default() -> Self {
        Slot(Arc::new(Shared {
            cap: Mutex::new(None),
            status: Mutex::new(Status { state: "off".into(), ..Default::default() }),
            ctl: Mutex::new(Ctl::default()),
            wake: tokio::sync::Notify::new(),
            io: tokio::sync::Mutex::new(()),
            known: Mutex::new(None),
            alert_sink: Mutex::new(None),
        }))
    }
}

impl Slot {
    pub fn status(&self) -> Status {
        self.0.status.lock().unwrap().clone()
    }
    pub fn config(&self) -> StreamConfig {
        self.0.ctl.lock().unwrap().config.clone()
    }
    pub fn wake(&self) {
        self.0.wake.notify_one();
    }
    fn active(&self) -> bool {
        self.0.cap.lock().unwrap().is_some()
    }
}

/// Anything the shipper can reach the database through: the relay's state, or a bare storage in tests.
pub trait HasStorage: Send + Sync + 'static {
    fn storage(&self) -> &Storage;
}
impl HasStorage for crate::relay::relay::RelayState {
    fn storage(&self) -> &Storage {
        &self.db
    }
}
impl HasStorage for Storage {
    fn storage(&self) -> &Storage {
        self
    }
}

// ── The hooks on Storage ─────────────────────────────────────────────────────

/// What one capture tick produced.
#[derive(Default)]
pub struct TickOut {
    pub job: Option<SnapJob>,
    pub captured: Option<Captured>,
    pub error: Option<String>,
}

impl Storage {
    /// Start continuous backup at relay start, BEFORE anything else writes, when it is
    /// configured on: read the settings, open the capturer (which picks up the last run or asks
    /// for a new generation), and switch SQLite's own checkpoint off.
    pub fn backup_stream_install(&self) -> Result<(), String> {
        let data = live_data_dir(self);
        let cfg = load_config(&data).unwrap_or_default();
        let mut ctl = self.stream.0.ctl.lock().unwrap();
        ctl.config = cfg.clone();
        ctl.version += 1;
        drop(ctl);
        if !cfg.active() {
            self.stream.0.status.lock().unwrap().state = "off".into();
            return Ok(());
        }
        self.start_capturer(&cfg, false)
    }

    /// Open (or, with `fresh`, restart from nothing) the capturer, under the writer lock.
    fn start_capturer(&self, cfg: &StreamConfig, fresh: bool) -> Result<(), String> {
        let db_path = live_db_path(self);
        let data = db_path.parent().map(Path::to_path_buf).unwrap_or_default();
        let key = crate::relay::storage::backup_crypto::load_or_create_key(&data).ok_or("the backup key (data/backup.key) cannot be read or made")?;
        let res = self.with_conn(|conn| -> Result<(), String> {
            if fresh {
                let work = work_dir(&db_path);
                let _ = std::fs::remove_dir_all(work.join("pending"));
                let _ = std::fs::remove_file(work.join("state.json"));
                let _ = std::fs::remove_file(work.join("clean-close.json"));
            }
            let cap = Capturer::open(&db_path, seal::object_key(&key))?;
            conn.execute_batch("PRAGMA wal_autocheckpoint=0").map_err(|e| e.to_string())?;
            *self.stream.0.cap.lock().unwrap() = Some(cap);
            Ok(())
        });
        let mut st = self.stream.0.status.lock().unwrap();
        st.enabled = res.is_ok();
        st.key_fingerprint = seal::key_fingerprint(&key);
        st.retention_hours = cfg.retention_hours;
        st.snapshot_every_hours = cfg.snapshot_every_hours;
        st.alert_after_minutes = cfg.alert_after_minutes;
        match &res {
            Ok(()) => {
                st.state = "starting".into();
                st.words = "Starting: the first copy is being made.".into();
                self.stream.0.ctl.lock().unwrap().enabled_ms = capture::now_ms();
            }
            Err(e) => {
                st.state = "failing".into();
                st.last_error = e.clone();
                st.words = format!("Continuous backup could not start: {e}");
            }
        }
        drop(st);
        *self.stream.0.known.lock().unwrap() = None;
        self.stream.wake();
        res
    }

    /// Stop capturing: SQLite's own checkpoint comes back. Staged objects stay on disk.
    fn stop_capturer(&self) {
        self.with_conn(|conn| {
            *self.stream.0.cap.lock().unwrap() = None;
            let _ = conn.execute_batch("PRAGMA wal_autocheckpoint=1000");
        });
        let mut st = self.stream.0.status.lock().unwrap();
        *st = Status { state: "off".into(), words: "Continuous backup is off.".into(), drill: st.drill.clone(), ..Default::default() };
    }

    /// `PRAGMA wal_checkpoint(TRUNCATE)` for the relay's privacy scrubs (after a bulk delete),
    /// with the continuous backup's capture first, so no committed frame leaves the log before
    /// it is staged. Call with the writer connection, inside `with_conn`.
    pub(crate) fn wal_truncate(&self, conn: &rusqlite::Connection) {
        let mut g = self.stream.0.cap.lock().unwrap();
        if let Some(cap) = g.as_mut() {
            if cap.needs_new_gen.is_none() {
                match cap.capture() {
                    Ok(Captured::Broken(why)) => cap.needs_new_gen = Some(why),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("continuous backup: capture before a checkpoint failed: {e}"),
                }
            }
        }
        match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))) {
            Ok((busy, log, done)) => {
                if let Some(cap) = g.as_mut() {
                    cap.after_checkpoint(busy, log, done);
                }
            }
            Err(e) => tracing::warn!("checkpoint after a delete failed: {e}"),
        }
    }

    /// One capture tick (blocking): capture what is new, checkpoint when the log is long, and
    /// hand back a snapshot to take when one is due or a new generation must start.
    pub fn backup_stream_tick(&self) -> TickOut {
        let (every_ms, force) = {
            let mut ctl = self.stream.0.ctl.lock().unwrap();
            let f = std::mem::take(&mut ctl.force_snapshot);
            (ctl.config.snapshot_every_hours.max(1) as u64 * 3_600_000, f)
        };
        let mut out = TickOut::default();
        self.with_conn(|conn| {
            let mut g = self.stream.0.cap.lock().unwrap();
            let Some(cap) = g.as_mut() else { return };
            let new_gen = cap.needs_new_gen.clone().or_else(|| (!cap.st.has_base).then(|| "the generation has no snapshot yet".to_string()));
            if let Some(why) = new_gen {
                match cap.begin_generation(conn, &why) {
                    Ok(job) => out.job = Some(job),
                    Err(e) => out.error = Some(e),
                }
                return;
            }
            match cap.capture() {
                Ok(Captured::Broken(why)) => {
                    tracing::warn!("continuous backup: {why}; starting a new generation");
                    match cap.begin_generation(conn, &why) {
                        Ok(job) => out.job = Some(job),
                        Err(e) => out.error = Some(e),
                    }
                    return;
                }
                Ok(c) => out.captured = Some(c),
                Err(e) => {
                    out.error = Some(e);
                    return;
                }
            }
            if cap.lap_frames() >= capture::CHECKPOINT_FRAMES {
                if let Ok((busy, log, done)) = conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))) {
                    cap.after_checkpoint(busy, log, done);
                }
            }
            if force || cap.snapshot_due(every_ms, capture::now_ms()) {
                match cap.begin_snapshot() {
                    Ok(job) => out.job = Some(job),
                    Err(e) => out.error = Some(e),
                }
            }
        });
        if !matches!(out.captured, None | Some(Captured::Nothing)) || out.job.is_some() {
            self.stream.0.status.lock().unwrap().last_capture_ms = capture::now_ms();
        }
        out
    }

    /// A snapshot job finished.
    pub fn backup_stream_snapshot_done(&self, res: &Result<capture::SnapInfo, String>) {
        match res {
            Ok(info) => {
                if let Some(cap) = self.stream.0.cap.lock().unwrap().as_mut() {
                    cap.snapshot_done(info);
                }
                let mut st = self.stream.0.status.lock().unwrap();
                st.last_snapshot_ms = info.taken_ms;
                tracing::info!("continuous backup: staged a full copy ({}, {} chunks) for generation {}", human_bytes(info.db_len), info.chunks, info.gen);
            }
            Err(e) => {
                tracing::error!("continuous backup: the full copy failed: {e}");
                let mut st = self.stream.0.status.lock().unwrap();
                st.last_error = format!("the full copy failed: {e}");
            }
        }
    }

    /// Ask for a fresh snapshot now (Back up now). Returns the sentence for the admin when
    /// continuous backup is on.
    pub fn backup_stream_request_snapshot(&self) -> Option<String> {
        if !self.stream.active() {
            return None;
        }
        self.stream.0.ctl.lock().unwrap().force_snapshot = true;
        self.stream.wake();
        let to = self.stream.config().destination.map(|d| d.summary()).unwrap_or_default();
        Some(format!("A fresh full copy is on its way to {to}; the Backups panel shows when it has arrived."))
    }

    /// The last step of a clean shutdown: capture, empty the log, leave the marker that lets
    /// the next start continue this generation. Best effort.
    pub fn backup_stream_close(&self) {
        self.stream.0.ctl.lock().unwrap().closed = true;
        self.stream.wake();
        self.with_conn(|conn| {
            if let Some(cap) = self.stream.0.cap.lock().unwrap().as_mut() {
                if let Err(e) = cap.close_clean(conn) {
                    tracing::warn!("continuous backup: clean close: {e}");
                }
            }
        });
    }

    pub fn backup_stream_status(&self) -> Status {
        self.stream.status()
    }
}

// ── The loop ─────────────────────────────────────────────────────────────────

/// What the loop keeps between passes.
#[derive(Default)]
pub struct RunState {
    version: u64,
    dest: Option<Arc<Destination>>,
    owner_ok: bool,
    retry_at: u64,
    backoff: u64,
    next_list: u64,
}

/// The shipper: one task for the relay's lifetime. Wakes every TICK (or when asked), captures,
/// takes snapshots, sends what is staged, applies retention and keeps the status current.
pub async fn run<T: HasStorage>(host: Arc<T>) {
    let mut rs = RunState::default();
    loop {
        let slot = host.storage().stream.clone();
        tokio::select! {
            _ = slot.0.wake.notified() => {}
            _ = tokio::time::sleep(TICK) => {}
        }
        if slot.0.ctl.lock().unwrap().closed {
            break;
        }
        pass(&host, &mut rs).await;
    }
}

/// One pass of the loop. Public so tests can drive the shipper step by step.
pub async fn pass<T: HasStorage>(host: &Arc<T>, rs: &mut RunState) {
    let slot = host.storage().stream.clone();
    let (cfg, version) = {
        let c = slot.0.ctl.lock().unwrap();
        (c.config.clone(), c.version)
    };
    if !cfg.active() || !slot.active() {
        rs.dest = None;
        return;
    }
    if rs.version != version || rs.dest.is_none() {
        rs.version = version;
        rs.owner_ok = false;
        rs.retry_at = 0;
        rs.next_list = 0;
        rs.dest = match Destination::from_config(cfg.destination.as_ref().unwrap(), cfg.sender_seed_bytes().as_ref()) {
            Ok(d) => Some(Arc::new(d)),
            Err(e) => {
                set_refused(&slot, &format!("The destination cannot be used: {e}"));
                return;
            }
        };
    }
    let dest = rs.dest.clone().unwrap();
    let now = capture::now_ms();

    // 1. Capture, and take any snapshot that is due.
    let h = host.clone();
    let tick = tokio::task::spawn_blocking(move || h.storage().backup_stream_tick()).await.unwrap_or_default();
    if let Some(e) = &tick.error {
        slot.0.status.lock().unwrap().last_error = e.clone();
    }
    if let Some(job) = tick.job {
        let res = tokio::task::spawn_blocking(move || job.run()).await.unwrap_or_else(|e| Err(e.to_string()));
        host.storage().backup_stream_snapshot_done(&res);
    }

    // 2. Talk to the destination, unless it is in its back-off.
    if now >= rs.retry_at {
        let _io = slot.0.io.lock().await;
        let res = talk(host.storage(), &slot, &dest, &cfg, rs, now).await;
        match res {
            Ok(()) => {
                rs.backoff = 0;
                let mut st = slot.0.status.lock().unwrap();
                st.last_error.clear();
                st.failing_since_ms = 0;
                st.refused.clear();
            }
            Err(Talk::Refused(why)) => {
                set_refused(&slot, &why);
                rs.retry_at = now + MAX_BACKOFF_MS;
            }
            Err(Talk::Failed(e)) => {
                rs.backoff = if rs.backoff == 0 { 5_000 } else { (rs.backoff * 2).min(MAX_BACKOFF_MS) };
                rs.retry_at = now + rs.backoff;
                let mut st = slot.0.status.lock().unwrap();
                if st.failing_since_ms == 0 {
                    st.failing_since_ms = now;
                }
                st.last_error = e;
            }
        }
    }

    // 3. Status and alerts.
    refresh_status(host.storage(), &slot, &cfg, capture::now_ms(), rs.retry_at);
    alert_check(&slot, &cfg);
}

enum Talk {
    Refused(String),
    Failed(String),
}

/// The owner check (once per destination), a listing and retention (every LIST_EVERY_MS), and
/// sending everything staged, oldest name first.
async fn talk(db: &Storage, slot: &Slot, dest: &Destination, cfg: &StreamConfig, rs: &mut RunState, now: u64) -> Result<(), Talk> {
    let key = object_key_for(db).map_err(Talk::Failed)?;
    if !rs.owner_ok {
        check_owner(dest, &key, &server_fingerprint(db)).await?;
        rs.owner_ok = true;
    }
    if now >= rs.next_list || slot.0.known.lock().unwrap().is_none() {
        let list = dest.list().await.map_err(Talk::Failed)?;
        *slot.0.known.lock().unwrap() = Some(list);
        rs.next_list = now + LIST_EVERY_MS;
        prune(dest, slot, cfg, now).await.map_err(Talk::Failed)?;
    }
    upload_pending(db, slot, dest).await.map_err(Talk::Failed)?;
    Ok(())
}

fn object_key_for(db: &Storage) -> Result<[u8; 32], String> {
    let data = live_data_dir(db);
    let key = crate::relay::storage::backup_crypto::load_or_create_key(&data).ok_or("the backup key cannot be read")?;
    Ok(seal::object_key(&key))
}

/// This server's identity fingerprint (the federation key), the owner a destination records.
pub fn server_fingerprint(db: &Storage) -> String {
    db.get_or_create_server_keypair()
        .ok()
        .and_then(|(pk, _)| hex::decode(pk).ok())
        .map(|pk| peer::fingerprint(&pk))
        .unwrap_or_default()
}

/// A destination is used by one server only: the first use leaves an owner mark; a mark this
/// server's key cannot open, or one naming another server, stops everything before a single
/// object is mixed in.
async fn check_owner(dest: &Destination, key: &[u8; 32], fp: &str) -> Result<(), Talk> {
    match dest.get(names::OWNER).await.map_err(Talk::Failed)? {
        None => {
            let body = seal::owner_body(fp, capture::now_ms());
            dest.put(names::OWNER, seal::seal(key, names::OWNER, seal::Kind::Owner, &body)).await.map_err(Talk::Failed)
        }
        Some(b) => match seal::open(key, names::OWNER, seal::Kind::Owner, &b).and_then(|p| seal::owner_decode(&p)) {
            Err(_) => Err(Talk::Refused(
                "That destination already holds backups made with another backup key (another server's, or this one's before its key changed). Choose an empty folder or prefix; nothing was written there.".into(),
            )),
            Ok((other, _)) if other != fp => Err(Talk::Refused(format!(
                "That destination holds the backups of another server (…{}). Choose another folder or prefix; nothing was written there.",
                &other[other.len().saturating_sub(8)..]
            ))),
            Ok(_) => Ok(()),
        },
    }
}

/// Files waiting in the staging folder, oldest name first.
fn pending_files(pending: &Path) -> Vec<(String, PathBuf, u64)> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf, u64)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let full = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(&e.path(), &full, out),
                Ok(t) if t.is_file() && !name.ends_with(".tmp") => out.push((full, e.path(), e.metadata().map(|m| m.len()).unwrap_or(0))),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(pending, "", &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn pending_dir(db: &Storage) -> PathBuf {
    work_dir(&live_db_path(db)).join("pending")
}

/// Send every staged object in name order (each generation's segments, then its snapshots'
/// chunks, then their manifests), deleting each local file once the destination has it.
/// Stops at the first failure, so nothing is ever sent out of order.
pub async fn upload_pending(db: &Storage, slot: &Slot, dest: &Destination) -> Result<u64, String> {
    let pending = pending_dir(db);
    let mut sent = 0;
    for (name, path, size) in pending_files(&pending) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("reading the staged copy: {e}")),
        };
        dest.put(&name, bytes).await?;
        let _ = std::fs::remove_file(&path);
        let mut dir = path.parent().map(Path::to_path_buf);
        while let Some(d) = dir {
            if d == pending || std::fs::remove_dir(&d).is_err() {
                break;
            }
            dir = d.parent().map(Path::to_path_buf);
        }
        if let Some(k) = slot.0.known.lock().unwrap().as_mut() {
            k.retain(|l| l.name != name);
            k.push(Listed { name, size });
        }
        slot.0.status.lock().unwrap().last_copy_ms = capture::now_ms();
        sent += 1;
    }
    Ok(sent)
}

/// Apply retention at the destination.
async fn prune(dest: &Destination, slot: &Slot, cfg: &StreamConfig, now: u64) -> Result<(), String> {
    let current = slot.0.cap.lock().unwrap().as_ref().map(|c| c.st.gen.clone());
    let doomed = {
        let k = slot.0.known.lock().unwrap();
        let Some(list) = k.as_ref() else { return Ok(()) };
        names::plan_prune(&Catalog::from_listing(list), now, cfg.retention_hours as u64 * 3_600_000, current.as_deref())
    };
    for name in &doomed {
        dest.delete(name).await?;
        if let Some(k) = slot.0.known.lock().unwrap().as_mut() {
            k.retain(|l| &l.name != name);
        }
    }
    if !doomed.is_empty() {
        tracing::info!("continuous backup: retention removed {} old objects", doomed.len());
    }
    Ok(())
}

fn set_refused(slot: &Slot, why: &str) {
    let mut st = slot.0.status.lock().unwrap();
    st.refused = why.to_string();
    st.state = "refused".into();
    st.words = why.to_string();
}

/// Recompute the status from the staging folder and what is known to be at the destination.
pub fn refresh_status(db: &Storage, slot: &Slot, cfg: &StreamConfig, now: u64, retry_at: u64) {
    let pending = pending_files(&pending_dir(db));
    let (gen, seq) = slot.0.cap.lock().unwrap().as_ref().map(|c| (c.st.gen.clone(), c.st.seq)).unwrap_or_default();
    let cat = slot.0.known.lock().unwrap().as_ref().map(|l| Catalog::from_listing(l));
    let enabled_ms = slot.0.ctl.lock().unwrap().enabled_ms;
    let mut st = slot.0.status.lock().unwrap();
    st.enabled = true;
    st.generation = gen;
    st.seq = seq;
    st.retention_hours = cfg.retention_hours;
    st.snapshot_every_hours = cfg.snapshot_every_hours;
    st.alert_after_minutes = cfg.alert_after_minutes;
    st.pending_objects = pending.len() as u64;
    st.pending_bytes = pending.iter().map(|p| p.2).sum();
    st.oldest_pending_ms = pending
        .iter()
        .filter_map(|p| std::fs::metadata(&p.1).and_then(|m| m.modified()).ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .min()
        .unwrap_or(0);
    if let Some(cat) = &cat {
        st.dest_objects = cat.objects;
        st.dest_bytes = cat.bytes;
        st.dest_checked_ms = now;
        st.generations = cat.gens.len() as u64;
        let complete = cat.complete_snaps();
        st.snapshots = complete.len() as u64;
        st.restorable_from_ms = complete.first().map(|s| s.ms).unwrap_or(0);
        st.restorable_to_ms = names::plan_restore(cat, Target::Latest).map(|p| p.restores_to_ms).unwrap_or(0);
    }
    // How much would be lost if this machine died now: nothing waiting means nothing.
    let since = if st.restorable_to_ms > 0 { st.restorable_to_ms } else { enabled_ms };
    st.lag_secs = if st.pending_objects == 0 && st.restorable_to_ms > 0 { 0 } else { now.saturating_sub(since) / 1000 };
    let to = cfg.destination.as_ref().map(|d| d.summary()).unwrap_or_default();
    let alert_after = cfg.alert_after_minutes as u64 * 60;
    if !st.refused.is_empty() {
        st.state = "refused".into();
        st.words = st.refused.clone();
    } else if st.lag_secs > alert_after && (st.pending_objects > 0 || st.restorable_to_ms == 0) {
        st.state = "behind".into();
        st.words = format!(
            "Backups are {} behind (the alert is set at {} min){}.",
            ago_words(st.lag_secs),
            cfg.alert_after_minutes,
            if st.last_error.is_empty() { ": still sending".to_string() } else { format!(": {}", st.last_error) }
        );
    } else if !st.last_error.is_empty() {
        st.state = "failing".into();
        let retry = retry_at.saturating_sub(now) / 1000;
        st.words = format!("The last copy to {to} failed: {}. Trying again in {retry} s.", st.last_error);
    } else if st.pending_objects > 0 || st.restorable_to_ms == 0 {
        st.state = if st.restorable_to_ms == 0 { "starting".into() } else { "catching_up".into() };
        st.words = if st.restorable_to_ms == 0 {
            format!("Sending the first full copy to {to}: {} parts ({}) to go.", st.pending_objects, human_bytes(st.pending_bytes))
        } else {
            format!("Sending the latest changes to {to}: {} parts to go.", st.pending_objects)
        };
    } else {
        st.state = "up_to_date".into();
        st.words = format!("Every change is at {to}. Newest copy: {}.", ago(now, st.restorable_to_ms.max(st.last_copy_ms)));
    }
}

fn ago_words(secs: u64) -> String {
    if secs < 120 {
        format!("{secs} s")
    } else if secs < 7200 {
        format!("{} min", secs / 60)
    } else {
        format!("{} h", secs / 3600)
    }
}

/// An alert when backups fall behind or are refused, and one when they are back. Each fires
/// once per episode, never every pass. The words name no folder, bucket or address: alert
/// channels such as an ntfy topic can be readable by others.
fn alert_check(slot: &Slot, cfg: &StreamConfig) {
    let (bad, good, state) = {
        let st = slot.0.status.lock().unwrap();
        let failing_long = st.failing_since_ms > 0 && capture::now_ms().saturating_sub(st.failing_since_ms) > cfg.alert_after_minutes as u64 * 60_000;
        (st.state == "behind" || st.state == "refused" || failing_long, st.state == "up_to_date", st.state.clone())
    };
    let server = std::env::var("SERVER_NAME").unwrap_or_else(|_| "HumanityOS relay".into());
    let kind = cfg.destination.as_ref().map(|d| d.kind()).unwrap_or("destination");
    let mut st = slot.0.status.lock().unwrap();
    let msg = if bad && !st.alert_active {
        st.alert_active = true;
        Some((
            format!(
                "{server}: continuous backup to its {kind} is {} (alert after {} min). Open Server Settings > Backups for the reason.",
                if state == "refused" { "refused".to_string() } else { format!("{} behind", ago_words(st.lag_secs)) },
                cfg.alert_after_minutes
            ),
            "warn",
        ))
    } else if good && st.alert_active {
        st.alert_active = false;
        Some((format!("{server}: continuous backup to its {kind} has caught up."), "info"))
    } else {
        None
    };
    if let Some((m, sev)) = msg {
        st.last_alert = m.clone();
        st.last_alert_ms = capture::now_ms();
        drop(st);
        send_alert(slot, &m, sev);
    }
}

fn send_alert(slot: &Slot, msg: &str, sev: &str) {
    if let Some(sink) = slot.0.alert_sink.lock().unwrap().as_ref() {
        sink(msg, sev);
        return;
    }
    tracing::warn!("ALERT: {msg}");
    // The alert channels an admin set up (data/alert-channels.secrets.json), sent by the same
    // script the watchdog and the disk guard use. No script here (a desktop node): the log line
    // above and the Backups panel are all there is (docs/design/in-app-ops.md records the gap).
    let script = Path::new("scripts/humanity-alert.js");
    if !script.exists() {
        return;
    }
    match std::process::Command::new("node").arg(script).arg(msg).arg(sev).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => tracing::warn!("could not run the alert script: {e}"),
    }
}

// ── Admin actions ────────────────────────────────────────────────────────────

/// The settings an admin sends from the Backups panel.
#[derive(Debug, Deserialize)]
pub struct SetRequest {
    pub enabled: bool,
    #[serde(default)]
    pub destination: Option<DestinationConfig>,
    #[serde(default = "default_retention")]
    pub retention_hours: u32,
    #[serde(default = "default_snapshot_every")]
    pub snapshot_every_hours: u32,
    #[serde(default = "default_alert_after")]
    pub alert_after_minutes: u32,
}

fn in_range(v: u32, (lo, hi): (u32, u32), what: &str) -> Result<u32, String> {
    if (lo..=hi).contains(&v) {
        Ok(v)
    } else {
        Err(format!("{what} goes from {lo} to {hi}."))
    }
}

/// Try a destination without saving it: write a sealed probe, read it back, delete it, and
/// check that no other server's backups are there. The answer says how long the round trip took.
pub async fn test_destination(db: &Storage, cfg: &DestinationConfig, sender_seed: Option<&[u8; 32]>) -> Result<String, String> {
    let dest = Destination::from_config(cfg, sender_seed)?;
    let key = object_key_for(db)?;
    let name = format!("probe-{:016x}", rand::random::<u64>());
    let body = format!("probe {}", capture::now_ms()).into_bytes();
    let sealed = seal::seal(&key, &name, seal::Kind::Probe, &body);
    let t = std::time::Instant::now();
    let fut = async {
        dest.put(&name, sealed).await?;
        let back = dest.get(&name).await?.ok_or("the test copy was not there when read back")?;
        dest.delete(&name).await?;
        if seal::open(&key, &name, seal::Kind::Probe, &back)? != body {
            return Err("the test copy came back different".to_string());
        }
        match dest.get(names::OWNER).await? {
            None => Ok(None),
            Some(b) => match seal::open(&key, names::OWNER, seal::Kind::Owner, &b).and_then(|p| seal::owner_decode(&p)) {
                Ok((fp, _)) if fp == server_fingerprint(db) => Ok(Some("it already holds this server's backups")),
                _ => Err("it already holds another server's backups (or ones made with another backup key): choose an empty folder or prefix".to_string()),
            },
        }
    };
    let note = tokio::time::timeout(std::time::Duration::from_secs(60), fut).await.map_err(|_| "no answer within a minute".to_string())??;
    Ok(format!(
        "{} works: a test copy went there and back in {} ms{}.",
        cfg.summary(),
        t.elapsed().as_millis(),
        note.map(|n| format!(", and {n}")).unwrap_or_default()
    ))
}

/// Save new settings from the Backups panel. A destination is tested before it is saved; a
/// changed destination starts a new generation there (and drops what was staged for the old one).
pub async fn apply_settings(db: &Storage, req: SetRequest) -> Result<String, String> {
    let data = live_data_dir(db);
    let saved = config_with_sender(&data);
    let retention_hours = in_range(req.retention_hours, RETENTION_RANGE, "History kept (hours)")?;
    let snapshot_every_hours = in_range(req.snapshot_every_hours, SNAPSHOT_RANGE, "A full copy every (hours)")?;
    let alert_after_minutes = in_range(req.alert_after_minutes, ALERT_RANGE, "Alert after (minutes)")?;
    // Turning it off without naming a destination keeps the saved one, ready for next time.
    let destination = match req.destination {
        Some(d) => Some(d.merged_with_saved(saved.destination.as_ref())),
        None if !req.enabled => saved.destination.clone(),
        None => None,
    };
    if req.enabled && destination.is_none() {
        return Err("Choose where the backups go before turning it on.".into());
    }
    let mut test_words = String::new();
    if req.enabled {
        test_words = test_destination(db, destination.as_ref().unwrap(), saved.sender_seed_bytes().as_ref()).await?;
    } else if let Some(d) = &destination {
        d.check()?;
    }
    let cfg = StreamConfig { enabled: req.enabled, destination, retention_hours, snapshot_every_hours, alert_after_minutes, sender_seed: saved.sender_seed.clone() };
    save_config(&data, &cfg)?;
    let changed_place = cfg.destination != saved.destination;
    let was_on = saved.active();
    {
        let mut ctl = db.stream.0.ctl.lock().unwrap();
        ctl.config = cfg.clone();
        ctl.version += 1;
    }
    tracing::info!("continuous backup: settings saved ({:?})", cfg);
    if !cfg.active() {
        db.stop_capturer();
        return Ok("Continuous backup is off. What was already sent stays where it is.".into());
    }
    if changed_place || !was_on || !db.stream.active() {
        db.start_capturer(&cfg, changed_place)?;
        return Ok(format!("{test_words} Continuous backup is on: the first full copy is being made now."));
    }
    db.stream.wake();
    Ok("Saved.".into())
}

/// Run a restore drill: bring the destination up to the live database's current state, restore
/// from the destination into a scratch file, check it with SQLite's integrity check, and
/// compare everything in it with the live database at that same moment. The live database is
/// only ever read. Runs on its own task; the result appears in the status.
pub async fn drill<T: HasStorage>(host: Arc<T>) -> DrillStatus {
    let slot = host.storage().stream.clone();
    let started = capture::now_ms();
    {
        let mut st = slot.0.status.lock().unwrap();
        if st.drill.running {
            return st.drill.clone();
        }
        st.drill = DrillStatus { running: true, started_ms: started, words: "Restore drill running...".into(), ..Default::default() };
    }
    let result = drill_inner(&host).await;
    let finished = capture::now_ms();
    let d = match result {
        Ok((report, ms)) => DrillStatus {
            running: false,
            started_ms: started,
            finished_ms: finished,
            ok: Some(true),
            words: format!(
                "Passed: restored from the destination to {} ({}, the full copy of {} plus {} change runs), SQLite's integrity check is ok, and every table matches the live database at that moment. Took {:.1} s.",
                utc_text(report.restored_to_ms.max(ms)),
                human_bytes(report.db_bytes),
                utc_text(report.snapshot_ms),
                report.segments,
                (finished - started) as f64 / 1000.0
            ),
            restored_to_ms: report.restored_to_ms.max(ms),
            duration_ms: finished - started,
            db_bytes: report.db_bytes,
        },
        Err(e) => DrillStatus { running: false, started_ms: started, finished_ms: finished, ok: Some(false), words: format!("Failed: {e}"), duration_ms: finished - started, ..Default::default() },
    };
    tracing::info!("continuous backup: restore drill {}", d.words);
    slot.0.status.lock().unwrap().drill = d.clone();
    d
}

async fn drill_inner<T: HasStorage>(host: &Arc<T>) -> Result<(restore::Report, u64), String> {
    let db = host.storage();
    let slot = db.stream.clone();
    let cfg = slot.config();
    if !cfg.active() || !slot.active() {
        return Err("continuous backup is off: there is nothing to restore from".into());
    }
    // 1. Under the writer lock: stage everything committed, and open a read transaction on the
    //    live database at exactly that point (the state the restore must reproduce).
    let h = host.clone();
    let (gen, seq, reference) = tokio::task::spawn_blocking(move || -> Result<(String, u64, rusqlite::Connection), String> {
        let db = h.storage();
        let path = live_db_path(db);
        db.with_conn(|_conn| {
            let mut g = db.stream.0.cap.lock().unwrap();
            let cap = g.as_mut().ok_or("continuous backup is off")?;
            if cap.needs_new_gen.is_some() || !cap.st.has_base {
                return Err("a new full copy is being made right now: run the drill again in a minute".to_string());
            }
            if let Captured::Broken(why) = cap.capture()? {
                cap.needs_new_gen = Some(why);
                return Err("the backup is starting a new full copy: run the drill again in a minute".into());
            }
            let r = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|e| e.to_string())?;
            r.execute_batch("BEGIN").map_err(|e| e.to_string())?;
            r.query_row("SELECT count(*) FROM sqlite_schema", [], |x| x.get::<_, i64>(0)).map_err(|e| e.to_string())?;
            Ok((cap.st.gen.clone(), cap.st.seq, r))
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    // 2. The live database's digest at that point (off the writer lock: writes go on meanwhile).
    let (live_digest, live_ms) = tokio::task::spawn_blocking(move || {
        let d = restore::logical_digest(&reference);
        let _ = reference.execute_batch("COMMIT");
        (d, capture::now_ms())
    })
    .await
    .map_err(|e| e.to_string())?;
    let live_digest = live_digest?;
    // 3. Send everything staged up to that point.
    let dest = Destination::from_config(cfg.destination.as_ref().unwrap(), cfg.sender_seed_bytes().as_ref())?;
    {
        let _io = slot.0.io.lock().await;
        upload_pending(db, &slot, &dest).await.map_err(|e| format!("the destination did not take the latest changes: {e}"))?;
    }
    // 4. Restore from the destination into a scratch file and compare.
    let work = work_dir(&live_db_path(db)).join("drill");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let out = work.join("restored.db");
    let key = object_key_for(db)?;
    let report = restore::restore(&dest, &key, &out, Target::Seq(&gen, seq)).await;
    let res = match report {
        Ok(r) if r.seq != seq => {
            Err(format!("the destination reached only change run {} of {seq}{}", r.seq, r.stopped_early.map(|w| format!(" ({w})")).unwrap_or_default()))
        }
        Ok(r) => {
            let o = out.clone();
            let digest = tokio::task::spawn_blocking(move || {
                let c = rusqlite::Connection::open_with_flags(&o, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
                restore::logical_digest(&c)
            })
            .await
            .map_err(|e| e.to_string())??;
            if digest == live_digest {
                Ok((r, live_ms))
            } else {
                Err("the restored copy opens and passes SQLite's check, but its contents differ from the live database at the same moment".into())
            }
        }
        Err(e) => Err(e),
    };
    restore::remove_db_files(&out);
    let _ = std::fs::remove_dir_all(&work);
    res
}
