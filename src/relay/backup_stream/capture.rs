//! The capturer: follows the live database's write-ahead log and turns each new run of
//! committed frames into sealed segment files in a local staging folder, from which the
//! uploader (mod.rs) sends them on. Also takes the snapshots a generation starts from.
//!
//! The one rule everything here keeps: **a frame never leaves the log before it is staged.**
//! SQLite's own automatic checkpoint is switched off while the stream runs
//! (`PRAGMA wal_autocheckpoint=0` on the writer); the checkpoints that do run are this
//! module's (after a capture, under the writer lock) and the relay's few explicit
//! `wal_checkpoint(TRUNCATE)` calls, which now go through `Storage::wal_truncate`, which
//! captures first. All of them hold the writer lock, so no transaction can slip in between the
//! capture and the checkpoint. When the shipper still cannot prove the log ran unbroken (the
//! relay was killed and restarted, another program checkpointed the file), it does not guess:
//! it starts a new generation from a fresh snapshot.
//!
//! Staging is the local half of crash safety: every object is written to a `.tmp` name, flushed
//! and renamed, and the state file is written after it. A crash between the two is repaired at
//! the next start from the staged segment itself, which records where in the log it ends.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use super::names::{self, Name};
use super::seal::{self, Kind, LogPoint, Manifest, Segment};
use super::wal;

/// The size of a snapshot chunk, and the size at which a run of frames is cut into another
/// segment. Every object stays well under nginx's 6 MB limit on /api/ (the HumanityOS server
/// destination) and under memory worth worrying about.
pub const CHUNK: usize = 4 * 1024 * 1024;
/// A checkpoint is run once this many frames are in the log, which is SQLite's own default.
pub const CHECKPOINT_FRAMES: u32 = 1000;

/// What the capturer remembers across restarts (`backup-stream/state.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub gen: String,
    /// The last segment staged in this generation (0: none yet).
    pub seq: u64,
    /// Where in the log the last capture ended; None when there was no log (everything was in
    /// the database file).
    pub pos: Option<LogPoint>,
    /// True when a checkpoint of ours has folded every frame of the current pass of the log
    /// into the database after they were all captured, so when SQLite starts the log over the
    /// shipper may follow it.
    pub lap_done: bool,
    /// The generation's first snapshot is staged.
    pub has_base: bool,
    pub last_snapshot_ms: u64,
    pub last_snapshot_seq: u64,
    pub bytes_since_snapshot: u64,
}

/// The marker a clean shutdown leaves (`backup-stream/clean-close.json`): with the log empty,
/// the database file alone is the state after segment `seq`. If the file is unchanged at the
/// next start, the generation simply continues and no new snapshot is needed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CleanClose {
    gen: String,
    seq: u64,
    db_len: u64,
    db_blake3: String,
}

/// What one capture did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Captured {
    Nothing,
    Staged { segments: u32, frames: u32, bytes: u64 },
    /// The log can no longer be shown to follow on from the last capture: a new generation is needed.
    Broken(String),
}

pub struct Capturer {
    pub db_path: PathBuf,
    wal_path: PathBuf,
    pub work: PathBuf,
    pub pending: PathBuf,
    key: [u8; 32],
    pub st: State,
    /// Why the next tick must start a new generation (None: it need not).
    pub needs_new_gen: Option<String>,
}

/// A snapshot to take outside the locks: a read transaction already open on the database at
/// exactly the point the snapshot stands for.
pub struct SnapJob {
    conn: Connection,
    gen: String,
    seq: u64,
    point: Option<LogPoint>,
    taken_ms: u64,
    work: PathBuf,
    pending: PathBuf,
    key: [u8; 32],
}

/// What a finished snapshot was.
#[derive(Debug, Clone)]
pub struct SnapInfo {
    pub gen: String,
    pub seq: u64,
    pub taken_ms: u64,
    pub db_len: u64,
    pub chunks: u32,
}

pub fn now_ms() -> u64 {
    crate::relay::storage::now_millis()
}

fn wal_path_for(db: &Path) -> PathBuf {
    let mut s = db.as_os_str().to_owned();
    s.push("-wal");
    PathBuf::from(s)
}

impl Capturer {
    /// Open the capturer for the database at `db_path`, picking up where the last run left off
    /// when that can be proven, and otherwise asking for a new generation. Called under the
    /// writer lock, before anything else writes.
    pub fn open(db_path: &Path, key: [u8; 32]) -> Result<Self, String> {
        let work = super::work_dir(db_path);
        let pending = work.join("pending");
        std::fs::create_dir_all(&pending).map_err(|e| format!("making {}: {e}", pending.display()))?;
        let mut cap = Capturer {
            db_path: db_path.to_path_buf(),
            wal_path: wal_path_for(db_path),
            work: work.clone(),
            pending,
            key,
            st: State {
                gen: String::new(),
                seq: 0,
                pos: None,
                lap_done: false,
                has_base: false,
                last_snapshot_ms: 0,
                last_snapshot_seq: 0,
                bytes_since_snapshot: 0,
            },
            needs_new_gen: Some("first start".into()),
        };
        let saved: Option<State> = std::fs::read_to_string(work.join("state.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
        let marker: Option<CleanClose> = std::fs::read_to_string(work.join("clean-close.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
        let _ = std::fs::remove_file(work.join("clean-close.json"));
        let Some(mut st) = saved else { return Ok(cap) };
        cap.recover_from_pending(&mut st);
        cap.st = st;
        if !cap.st.has_base {
            cap.needs_new_gen = Some("the last generation never got its first snapshot".into());
            return Ok(cap);
        }
        // A clean shutdown: the database file is exactly the state after the last segment.
        if let Some(m) = marker {
            if m.gen == cap.st.gen && m.seq == cap.st.seq {
                match file_digest(db_path) {
                    Ok((len, hash)) if len == m.db_len && hash == m.db_blake3 => {
                        cap.st.pos = None;
                        cap.st.lap_done = true;
                        cap.needs_new_gen = None;
                        tracing::info!("continuous backup: resumed generation {} after a clean shutdown", cap.st.gen);
                        return Ok(cap);
                    }
                    _ => {}
                }
            }
            cap.needs_new_gen = Some("the database changed while the relay was stopped".into());
            return Ok(cap);
        }
        // An unclean stop: the log must still hold the exact point the last capture reached.
        if let Some(p) = cap.st.pos {
            if let Ok(Some(h)) = wal::read_header(&cap.wal_path) {
                if h.salt1 == p.salt1 && h.salt2 == p.salt2 && chain_at(&cap.wal_path, &h, p.frame) == Some((p.ck1, p.ck2)) {
                    cap.needs_new_gen = None;
                    tracing::info!("continuous backup: resumed generation {} at frame {}", cap.st.gen, p.frame);
                    return Ok(cap);
                }
            }
        }
        cap.needs_new_gen = Some("the relay stopped without a clean shutdown and its log no longer reaches the last copy".into());
        Ok(cap)
    }

    /// A segment or manifest staged after the state file was last written (a crash in between):
    /// read where it ends and take that as the state.
    fn recover_from_pending(&self, st: &mut State) {
        let dir = self.pending.join(&st.gen);
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        let mut newest: Option<(u64, String)> = None;
        let mut has_manifest = false;
        for e in rd.flatten() {
            let n = format!("{}/{}", st.gen, e.file_name().to_string_lossy());
            match names::parse(&n) {
                Name::Segment { seq, .. } if seq > st.seq && newest.as_ref().map_or(true, |(s, _)| seq > *s) => newest = Some((seq, n)),
                _ => {}
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(inner) = std::fs::read_dir(e.path()) {
                    has_manifest |= inner.flatten().any(|f| f.file_name().to_string_lossy().starts_with("manifest-"));
                }
            }
        }
        if has_manifest && !st.has_base {
            st.has_base = true;
        }
        if let Some((seq, name)) = newest {
            let read = std::fs::read(self.pending_path(&name)).ok().and_then(|b| seal::open(&self.key, &name, Kind::Segment, &b).ok()).and_then(|b| Segment::decode(&b).ok());
            if let Some(seg) = read {
                st.seq = seq;
                st.pos = Some(seg.end);
                st.lap_done = false;
                tracing::warn!("continuous backup: picked up segment {seq} staged just before the last stop");
            }
        }
    }

    pub fn pending_path(&self, name: &str) -> PathBuf {
        name.split('/').fold(self.pending.clone(), |p, c| p.join(c))
    }

    fn persist(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.st).map_err(|e| e.to_string())?;
        super::write_file_atomic(&self.work.join("state.json"), text.as_bytes(), true)
    }

    fn stage(&self, name: &str, kind: Kind, plain: &[u8]) -> Result<u64, String> {
        let sealed = seal::seal(&self.key, name, kind, plain);
        let len = sealed.len() as u64;
        super::write_file_atomic(&self.pending_path(name), &sealed, true)?;
        Ok(len)
    }

    /// Frames in the current pass of the log that have been captured.
    pub fn lap_frames(&self) -> u32 {
        self.st.pos.map(|p| p.frame).unwrap_or(0)
    }

    /// Stage every frame committed since the last capture. Call with the writer lock held.
    pub fn capture(&mut self) -> Result<Captured, String> {
        let hdr = wal::read_header(&self.wal_path).map_err(|e| format!("reading the log: {e}"))?;
        let (h, after, ck) = match (self.st.pos, hdr) {
            (_, None) => {
                if self.st.pos.is_none() || self.st.lap_done {
                    self.st.pos = None;
                    return Ok(Captured::Nothing);
                }
                // A log that held uncaptured-or-unfolded frames vanished without our checkpoint.
                return Ok(Captured::Broken("the log was emptied by something other than the backup".into()));
            }
            (None, Some(h)) => (h, 0, h.cksum),
            (Some(p), Some(h)) if p.salt1 == h.salt1 && p.salt2 == h.salt2 => (h, p.frame, (p.ck1, p.ck2)),
            (Some(p), Some(h)) => {
                if self.st.lap_done && h.salt1 == p.salt1.wrapping_add(1) {
                    (h, 0, h.cksum)
                } else {
                    return Ok(Captured::Broken("the log was started over by something other than the backup".into()));
                }
            }
        };
        let out = wal::read_committed(&self.wal_path, &h, after, ck).map_err(|e| format!("reading the log: {e}"))?;
        if out.frames.is_empty() {
            return Ok(Captured::Nothing);
        }
        let captured_ms = now_ms();
        let mut chain = ck;
        let mut index = after;
        let mut segments = 0u32;
        let mut bytes = 0u64;
        let frames = out.frames.len() as u32;
        let mut part: Vec<(u32, u32, Vec<u8>)> = Vec::new();
        let mut part_bytes = 0usize;
        let mut part_first = after + 1;
        let total = out.frames.len();
        for (i, f) in out.frames.into_iter().enumerate() {
            let mut fh = [0u8; 8];
            fh[..4].copy_from_slice(&f.pgno.to_be_bytes());
            fh[4..].copy_from_slice(&f.commit.to_be_bytes());
            chain = wal::checksum(h.big_endian, &f.data, wal::checksum(h.big_endian, &fh, chain));
            index += 1;
            part_bytes += f.data.len();
            let commit = f.commit;
            part.push((f.pgno, f.commit, f.data));
            let last = i + 1 == total;
            if last || part_bytes >= CHUNK {
                self.st.seq += 1;
                let seg = Segment {
                    page_size: h.page_size,
                    first_frame: part_first,
                    end: LogPoint { salt1: h.salt1, salt2: h.salt2, frame: index, ck1: chain.0, ck2: chain.1 },
                    captured_ms,
                    db_pages: if commit != 0 { commit } else { 0 },
                    ends_at_commit: commit != 0,
                    frames: std::mem::take(&mut part),
                };
                let name = names::segment_name(&self.st.gen, self.st.seq, captured_ms, seg.ends_at_commit);
                bytes += self.stage(&name, Kind::Segment, &seg.encode())?;
                self.st.pos = Some(seg.end);
                self.st.lap_done = false;
                self.st.bytes_since_snapshot += part_bytes as u64;
                self.persist()?;
                segments += 1;
                part_bytes = 0;
                part_first = index + 1;
            }
        }
        debug_assert_eq!(index, out.last_frame);
        Ok(Captured::Staged { segments, frames, bytes })
    }

    /// Note what a checkpoint just did. `(busy, log, done)` is the row `PRAGMA wal_checkpoint`
    /// returns. Only a checkpoint that folded every frame of the log, every one of them
    /// captured, lets the shipper follow SQLite when it starts the log over.
    pub fn after_checkpoint(&mut self, busy: i64, log: i64, done: i64) {
        if busy != 0 || log < 0 {
            return;
        }
        let truncated = std::fs::metadata(&self.wal_path).map(|m| m.len() == 0).unwrap_or(true);
        if truncated {
            self.st.pos = None;
            self.st.lap_done = true;
        } else if log == done && log as u32 == self.lap_frames() {
            self.st.lap_done = true;
        }
        let _ = self.persist();
    }

    /// Start a new generation at the log's current end. Under the writer lock: nothing commits
    /// between finding the end and opening the read transaction the snapshot is copied from.
    pub fn begin_generation(&mut self, writer: &Connection, why: &str) -> Result<SnapJob, String> {
        let point = match wal::read_header(&self.wal_path).map_err(|e| format!("reading the log: {e}"))? {
            None => None,
            Some(h) => {
                let out = wal::read_committed(&self.wal_path, &h, 0, h.cksum).map_err(|e| format!("reading the log: {e}"))?;
                Some(LogPoint { salt1: h.salt1, salt2: h.salt2, frame: out.last_frame, ck1: out.cksum.0, ck2: out.cksum.1 })
            }
        };
        // Leftovers of a generation that never got its snapshot are no use anywhere.
        if !self.st.gen.is_empty() && !self.st.has_base {
            let _ = std::fs::remove_dir_all(self.pending.join(&self.st.gen));
        }
        let now = now_ms();
        self.st = State {
            gen: names::new_generation(now),
            seq: 0,
            pos: point,
            lap_done: false,
            has_base: false,
            last_snapshot_ms: 0,
            last_snapshot_seq: 0,
            bytes_since_snapshot: 0,
        };
        // Fold what the snapshot will hold into the database now, so that if SQLite starts the
        // log over next, the shipper can follow it.
        if let Ok((busy, log, done)) = writer.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))) {
            if busy == 0 && log >= 0 && log == done && log as u32 == self.lap_frames() {
                self.st.lap_done = true;
            }
        }
        self.persist()?;
        self.needs_new_gen = None;
        tracing::info!("continuous backup: new generation {} ({why})", self.st.gen);
        self.snap_job(now)
    }

    /// Take a snapshot inside the current generation, at the point the last capture reached.
    /// Call right after a capture, under the writer lock.
    pub fn begin_snapshot(&mut self) -> Result<SnapJob, String> {
        self.snap_job(now_ms())
    }

    fn snap_job(&self, taken_ms: u64) -> Result<SnapJob, String> {
        let conn = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|e| format!("opening the database to copy it: {e}"))?;
        conn.execute_batch("BEGIN").map_err(|e| format!("starting the copy: {e}"))?;
        // A read transaction really starts at its first read.
        conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get::<_, i64>(0)).map_err(|e| format!("starting the copy: {e}"))?;
        Ok(SnapJob {
            conn,
            gen: self.st.gen.clone(),
            seq: self.st.seq,
            point: self.st.pos,
            taken_ms,
            work: self.work.clone(),
            pending: self.pending.clone(),
            key: self.key,
        })
    }

    /// Record a finished snapshot (under the capturer lock only).
    pub fn snapshot_done(&mut self, info: &SnapInfo) {
        if info.gen != self.st.gen {
            return;
        }
        self.st.has_base = true;
        self.st.last_snapshot_ms = info.taken_ms;
        self.st.last_snapshot_seq = info.seq;
        self.st.bytes_since_snapshot = 0;
        let _ = self.persist();
    }

    /// A snapshot is due: the cadence passed, or more has changed since the last one than the
    /// whole database holds (a restore would replay more than it would copy).
    pub fn snapshot_due(&self, every_ms: u64, now: u64) -> bool {
        if !self.st.has_base {
            return false;
        }
        let db_len = std::fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0);
        now.saturating_sub(self.st.last_snapshot_ms) >= every_ms || self.st.bytes_since_snapshot > db_len.max(64 * 1024 * 1024)
    }

    /// The last step of a clean shutdown, under the writer lock: capture, fold the whole log
    /// into the database and empty it, and leave the marker that lets the next start continue
    /// this generation if the file is still the same then.
    pub fn close_clean(&mut self, writer: &Connection) -> Result<(), String> {
        if self.needs_new_gen.is_some() || !self.st.has_base {
            return Ok(());
        }
        if let Captured::Broken(_) = self.capture()? {
            return Ok(());
        }
        let row = writer
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))
            .map_err(|e| format!("checkpoint: {e}"))?;
        self.after_checkpoint(row.0, row.1, row.2);
        if self.st.pos.is_some() {
            return Ok(()); // the log could not be emptied (a reader held it): the next start makes a new generation
        }
        let (db_len, hash) = file_digest(&self.db_path).map_err(|e| format!("reading the database: {e}"))?;
        let m = CleanClose { gen: self.st.gen.clone(), seq: self.st.seq, db_len, db_blake3: hash };
        let text = serde_json::to_string(&m).map_err(|e| e.to_string())?;
        super::write_file_atomic(&self.work.join("clean-close.json"), text.as_bytes(), true)
    }
}

impl SnapJob {
    /// Copy the database as it stands in the open read transaction (SQLite's online backup, page
    /// for page, so the frames shipped after it apply to the same page numbers), cut the copy
    /// into chunks, seal and stage them, and stage the manifest last.
    pub fn run(self) -> Result<SnapInfo, String> {
        let tmp = self.work.join("snapshot.tmp.db");
        for side in ["", "-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{side}", tmp.display()));
        }
        {
            let mut dest = Connection::open(&tmp).map_err(|e| format!("making the copy: {e}"))?;
            let b = rusqlite::backup::Backup::new(&self.conn, &mut dest).map_err(|e| format!("starting the copy: {e}"))?;
            match b.step(-1).map_err(|e| format!("copying: {e}"))? {
                rusqlite::backup::StepResult::Done => {}
                other => return Err(format!("the copy did not finish ({other:?})")),
            }
        }
        let _ = self.conn.execute_batch("COMMIT");
        drop(self.conn);
        let res = stage_snapshot_file(&tmp, &self.pending, &self.key, &self.gen, self.seq, self.taken_ms, self.point);
        for side in ["", "-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{side}", tmp.display()));
        }
        res
    }
}

/// Chunk, seal and stage a database file as a snapshot of `gen` after segment `seq`.
pub fn stage_snapshot_file(file: &Path, pending: &Path, key: &[u8; 32], gen: &str, seq: u64, taken_ms: u64, point: Option<LogPoint>) -> Result<SnapInfo, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(file).map_err(|e| format!("reading the copy: {e}"))?;
    let db_len = f.metadata().map(|m| m.len()).map_err(|e| e.to_string())?;
    let mut head = [0u8; 18];
    f.read_exact(&mut head).map_err(|e| format!("reading the copy's header: {e}"))?;
    let raw = u16::from_be_bytes([head[16], head[17]]) as u32;
    let page_size = if raw == 1 { 65_536 } else { raw };
    drop(f);
    let mut f = std::fs::File::open(file).map_err(|e| format!("reading the copy: {e}"))?;
    let dir = names::snapshot_dir(gen, seq, taken_ms);
    let mut hashes = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let mut got = 0;
        while got < CHUNK {
            let n = f.read(&mut buf[got..]).map_err(|e| format!("reading the copy: {e}"))?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            break;
        }
        let chunk = &buf[..got];
        hashes.push(*blake3::hash(chunk).as_bytes());
        let name = names::chunk_name(&dir, hashes.len() as u32 - 1);
        let sealed = seal::seal(key, &name, Kind::SnapChunk, chunk);
        super::write_file_atomic(&path_in(pending, &name), &sealed, true)?;
        if got < CHUNK {
            break;
        }
    }
    let m = Manifest { page_size, db_len, chunk_size: CHUNK as u32, taken_ms, seq, point, chunk_hashes: hashes.clone() };
    let name = names::manifest_name(&dir, hashes.len() as u32);
    super::write_file_atomic(&path_in(pending, &name), &seal::seal(key, &name, Kind::SnapManifest, &m.encode()), true)?;
    Ok(SnapInfo { gen: gen.to_string(), seq, taken_ms, db_len, chunks: hashes.len() as u32 })
}

fn path_in(root: &Path, name: &str) -> PathBuf {
    name.split('/').fold(root.to_path_buf(), |p, c| p.join(c))
}

/// The checksum chain after frame `n` of the log's current pass (None if the log does not reach it).
fn chain_at(path: &Path, h: &wal::WalHeader, n: u32) -> Option<(u32, u32)> {
    if n == 0 {
        return Some(h.cksum);
    }
    let out = wal::read_committed(path, h, 0, h.cksum).ok()?;
    if out.last_frame < n {
        return None;
    }
    let mut ck = h.cksum;
    for f in out.frames.iter().take(n as usize) {
        let mut fh = [0u8; 8];
        fh[..4].copy_from_slice(&f.pgno.to_be_bytes());
        fh[4..].copy_from_slice(&f.commit.to_be_bytes());
        ck = wal::checksum(h.big_endian, &f.data, wal::checksum(h.big_endian, &fh, ck));
    }
    Some(ck)
}

/// Length and BLAKE3 hash of a file, read in pieces.
pub fn file_digest(path: &Path) -> std::io::Result<(u64, String)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut len = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        len += n as u64;
        h.update(&buf[..n]);
    }
    Ok((len, h.finalize().to_hex().to_string()))
}
