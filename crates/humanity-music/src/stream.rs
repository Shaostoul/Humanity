//! Streaming long recordings from disk while a note plays (docs/design/music.md section 23,
//! "Memory", built 2026-11-14).
//!
//! A full instrument is far too big to hold decoded: the full Steinway is 2,687 s of samples, about
//! 490 MiB as 16-bit stereo. So, as disk-streaming samplers do, a recording longer than a few seconds
//! keeps only its ATTACK in memory (its first half second, the zone's `frames`), and the rest is
//! decoded just in time on a worker thread into a ring buffer for each sounding note, ahead of the
//! play head. The reference is sfizz (the open SFZ player, BSD-2-Clause; its `src/sfizz/Config.h`,
//! read 2026-11-13): `preloadSize { 8192 }` frames kept of every file, `numBackgroundThreads { 4 }`
//! loading the rest, `loadInRam { false }`, a file promise per voice. Ours keeps more of each file
//! (half a second: the worker shares one thread with nothing else and Windows sleeps in 15.6 ms
//! steps) in one thread, and streams into a fixed ring per voice rather than a growing buffer.
//!
//! The parts:
//! - [`StreamPool`]: a fixed set of rings, made once (allocated here, never on the audio thread). A
//!   note claims one with a compare-and-swap ([`StreamLease::open`]), reads from it, and gives it
//!   back when it ends ([`StreamLease`]'s drop). Each ring has ONE writer (the worker) and ONE
//!   reader (the note), so it needs no lock: the worker publishes how much it has written with a
//!   release store, the note publishes how much it has read the same way.
//! - [`Service`]: the worker's pass (open what was asked for, top every ring up, let go of what
//!   ended), run on its own thread by [`start_worker`], or by a test one pass at a time.
//! - [`SourceOpener`] and [`FrameSource`]: where the frames come from (files, through
//!   [`crate::decode::Reader`], in [`FileOpener`]; a test's own frames in memory).
//!
//! When the stream cannot keep up (the disk stalls, or every ring is in use: the budget, set in
//! `data/music/streaming.ron`), the note does not stop dead (a click): the voice sees that the frames
//! it has will run out within a block and a fade, and fades out over the frames it still has
//! (`voice.rs`, the streamed sampler), so a note the budget cannot stream plays its attack and a
//! release. Nothing is written to disk and nothing outlives the note: a ring is refilled from the
//! file for every note, so there is nothing that could go stale (CLAUDE.md, the no-caches rule).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// How long recordings are streamed (`data/music/streaming.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamSettings {
    /// Off: every recording is decoded whole (as before 2026-11-14).
    pub enabled: bool,
    /// A recording longer than this, seconds, is streamed; a shorter one is kept whole.
    pub stream_longer_than_s: f32,
    /// The attack kept in memory of a streamed recording, seconds.
    pub attack_s: f32,
    /// How many notes can stream at once (each holds a ring): a note beyond them plays its attack
    /// and fades.
    pub streams: usize,
    /// Each ring's length, seconds of 48 kHz audio (rounded up to a power of two frames).
    pub ring_s: f32,
    /// The fade a note is given when its frames will run out, milliseconds.
    pub fade_ms: f32,
    /// How long the worker sleeps between passes, milliseconds.
    pub poll_ms: f32,
}

impl Default for StreamSettings {
    fn default() -> Self {
        StreamSettings { enabled: true, stream_longer_than_s: 2.0, attack_s: 0.5, streams: 32, ring_s: 0.5, fade_ms: 20.0, poll_ms: 4.0 }
    }
}

impl StreamSettings {
    /// Parse `data/music/streaming.ron`.
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| format!("streaming.ron: {e}"))
    }
}

/// Where a stream's frames come from, positioned where the note needs them.
pub trait FrameSource: Send {
    /// Append the next frames (any number: a codec packet's worth) to `out`. False at the end.
    fn read(&mut self, out: &mut Vec<[i16; 2]>) -> Result<bool, String>;
}

/// Opens a source by the id a zone carries.
pub trait SourceOpener: Send + Sync {
    /// Source `source`, positioned at frame `start` (its first `start` frames are the zone's attack,
    /// already in memory).
    fn open(&self, source: u32, start: u64) -> Result<Box<dyn FrameSource>, String>;
}

// A ring's states. Only the note moves a ring FREE -> CLAIMED -> REQUESTED and to CLOSING; only the
// worker moves it REQUESTED -> STREAMING and CLOSING -> FREE.
const FREE: u8 = 0;
const CLAIMED: u8 = 1;
const REQUESTED: u8 = 2;
const STREAMING: u8 = 3;
const CLOSING: u8 = 4;

/// One ring and what it streams. Frames are kept packed, left in the low 16 bits, as atomics: a
/// relaxed load of one is a plain load, and the reader never sees a frame the writer has not
/// published (it reads only below `written`, loaded with acquire).
struct Ring {
    state: AtomicU8,
    source: AtomicU32,
    start: AtomicU64,
    /// Frames written since the stream began (the worker's).
    written: AtomicU64,
    /// Frames the note is done with (the note's).
    read: AtomicU64,
    /// The frame count at the end of the source, once it has ended (u64::MAX until then).
    end: AtomicU64,
    /// The source failed: no more frames will come, and it did not end (the note fades).
    failed: AtomicBool,
    frames: Box<[AtomicU32]>,
}

#[inline]
fn pack(f: [i16; 2]) -> u32 {
    (f[0] as u16 as u32) | ((f[1] as u16 as u32) << 16)
}

#[inline]
fn unpack(x: u32) -> [i16; 2] {
    [x as u16 as i16, (x >> 16) as u16 as i16]
}

/// The rings, shared by the notes (the audio thread) and the worker.
pub struct StreamPool {
    rings: Box<[Ring]>,
    mask: u64,
    fade_s: f32,
    poll: std::time::Duration,
    /// The worker, to wake for an offline render waiting on it (never woken from the audio thread).
    worker: std::sync::Mutex<Option<std::thread::Thread>>,
}

impl std::fmt::Debug for StreamPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamPool({} rings of {} frames)", self.rings.len(), self.mask + 1)
    }
}

impl StreamPool {
    /// `settings.streams` rings of `settings.ring_s` (allocated here, once).
    pub fn new(settings: &StreamSettings) -> Arc<StreamPool> {
        let len = ((settings.ring_s.max(0.05) * 48000.0) as usize).next_power_of_two();
        let rings = (0..settings.streams.max(1))
            .map(|_| Ring {
                state: AtomicU8::new(FREE),
                source: AtomicU32::new(0),
                start: AtomicU64::new(0),
                written: AtomicU64::new(0),
                read: AtomicU64::new(0),
                end: AtomicU64::new(u64::MAX),
                failed: AtomicBool::new(false),
                frames: (0..len).map(|_| AtomicU32::new(0)).collect(),
            })
            .collect();
        Arc::new(StreamPool { rings, mask: len as u64 - 1, fade_s: settings.fade_ms.max(1.0) / 1000.0, poll: std::time::Duration::from_secs_f32(settings.poll_ms.clamp(0.5, 50.0) / 1000.0), worker: std::sync::Mutex::new(None) })
    }

    /// The memory the rings hold, bytes.
    pub fn memory(&self) -> usize {
        self.rings.len() * (self.mask as usize + 1) * 4
    }

    /// How many rings there are.
    pub fn streams(&self) -> usize {
        self.rings.len()
    }

    /// How many are streaming (or asked for) now.
    pub fn in_use(&self) -> usize {
        self.rings.iter().filter(|r| r.state.load(Ordering::Relaxed) != FREE).count()
    }

    /// Each ring's length, frames.
    pub fn ring_frames(&self) -> u64 {
        self.mask + 1
    }

    /// The fade a note is given when its frames will run out, in frames at `rate`.
    pub fn fade_frames(&self, rate: f32) -> u32 {
        ((self.fade_s * rate) as u32).max(1)
    }

    /// Wake the worker now (an offline render waiting for frames; never called on the audio thread,
    /// which only ever waits for the worker's next pass).
    pub fn wake(&self) {
        if let Some(t) = self.worker.lock().ok().and_then(|g| g.clone()) {
            t.unpark();
        }
    }

    fn close(&self, i: usize) {
        self.rings[i].state.store(CLOSING, Ordering::Release);
    }
}

/// What a streamed zone carries: the pool its notes stream through and its source there. Its attack
/// is the zone's `frames`; the stream starts where they end.
#[derive(Clone)]
pub struct ZoneStream {
    pub pool: Arc<StreamPool>,
    pub source: u32,
    /// The whole recording's length in frames, when its file says.
    pub frames: Option<u64>,
}

impl std::fmt::Debug for ZoneStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZoneStream(source {}, {:?} frames)", self.source, self.frames)
    }
}

/// A note's hold on one ring, given back when it is dropped. Opening, reading and dropping it are
/// lock-free and allocate nothing (the audio thread's rule).
pub struct StreamLease {
    pool: Arc<StreamPool>,
    ring: usize,
}

impl StreamLease {
    /// Claim a free ring for `zone`'s stream from frame `start` on. None when every ring is in use
    /// (the note plays its attack and fades).
    pub fn open(zone: &ZoneStream, start: u64) -> Option<StreamLease> {
        for (i, r) in zone.pool.rings.iter().enumerate() {
            if r.state.compare_exchange(FREE, CLAIMED, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                r.source.store(zone.source, Ordering::Relaxed);
                r.start.store(start, Ordering::Relaxed);
                r.written.store(0, Ordering::Relaxed);
                r.read.store(0, Ordering::Relaxed);
                r.end.store(u64::MAX, Ordering::Relaxed);
                r.failed.store(false, Ordering::Relaxed);
                r.state.store(REQUESTED, Ordering::Release);
                return Some(StreamLease { pool: zone.pool.clone(), ring: i });
            }
        }
        None
    }

    /// How many frames are ready from the stream's start, and whether that is all there will be (the
    /// source ended there).
    #[inline]
    pub fn ready(&self) -> (u64, bool) {
        let r = &self.pool.rings[self.ring];
        let end = r.end.load(Ordering::Acquire);
        let w = r.written.load(Ordering::Acquire);
        (w, end != u64::MAX && w >= end)
    }

    /// Whether the source failed (no more frames will come; the note fades).
    pub fn failed(&self) -> bool {
        self.pool.rings[self.ring].failed.load(Ordering::Acquire)
    }

    /// Frame `i` of the stream (from its start). Only below what [`StreamLease::ready`] gave, and not
    /// before what [`StreamLease::consumed`] let go.
    #[inline]
    pub fn frame(&self, i: u64) -> [i16; 2] {
        self.view().get(i)
    }

    /// The ring's frames, for a block's reads (looked up once, not per frame).
    #[inline]
    pub fn view(&self) -> RingView<'_> {
        RingView { frames: &self.pool.rings[self.ring].frames, mask: self.pool.mask }
    }

    /// The note is done with every frame before `upto`: the worker may write over them.
    #[inline]
    pub fn consumed(&self, upto: u64) {
        self.pool.rings[self.ring].read.store(upto, Ordering::Release);
    }

    /// The pool it streams through.
    pub fn pool(&self) -> &StreamPool {
        &self.pool
    }
}

/// A ring's frames, read by a note through its lease (the same rules as [`StreamLease::frame`]).
#[derive(Clone, Copy)]
pub struct RingView<'a> {
    frames: &'a [AtomicU32],
    mask: u64,
}

impl RingView<'_> {
    /// Frame `i` of the stream (from its start).
    #[inline]
    pub fn get(&self, i: u64) -> [i16; 2] {
        unpack(self.frames[(i & self.mask) as usize].load(Ordering::Relaxed))
    }
}

impl Drop for StreamLease {
    fn drop(&mut self) {
        self.pool.close(self.ring);
    }
}

/// A stream the worker has open.
struct Open {
    src: Box<dyn FrameSource>,
    pending: Vec<[i16; 2]>,
    at: usize,
}

/// The worker's side: one pass opens what notes asked for, tops every ring up and lets go of the
/// rings whose notes ended.
pub struct Service {
    pool: Arc<StreamPool>,
    opener: Arc<dyn SourceOpener>,
    open: Vec<Option<Open>>,
}

impl Service {
    pub fn new(pool: Arc<StreamPool>, opener: Arc<dyn SourceOpener>) -> Self {
        let open = (0..pool.rings.len()).map(|_| None).collect();
        Service { pool, opener, open }
    }

    /// One pass over every ring. Returns the frames written.
    pub fn pass(&mut self) -> u64 {
        let mut wrote = 0;
        let len = self.pool.mask + 1;
        for (i, r) in self.pool.rings.iter().enumerate() {
            match r.state.load(Ordering::Acquire) {
                REQUESTED => {
                    let (source, start) = (r.source.load(Ordering::Relaxed), r.start.load(Ordering::Relaxed));
                    match self.opener.open(source, start) {
                        Ok(src) => self.open[i] = Some(Open { src, pending: Vec::with_capacity(8192), at: 0 }),
                        Err(e) => {
                            log::warn!("[Music] a recording could not be streamed: {e}");
                            r.failed.store(true, Ordering::Release);
                        }
                    }
                    if r.state.compare_exchange(REQUESTED, STREAMING, Ordering::AcqRel, Ordering::Acquire).is_err() {
                        // The note ended before its stream opened.
                        self.open[i] = None;
                        r.state.store(FREE, Ordering::Release);
                        continue;
                    }
                }
                CLOSING => {
                    self.open[i] = None;
                    r.state.store(FREE, Ordering::Release);
                    continue;
                }
                STREAMING => {}
                _ => continue,
            }
            let Some(o) = self.open[i].as_mut() else { continue };
            let mut w = r.written.load(Ordering::Relaxed);
            let read = r.read.load(Ordering::Acquire);
            let mut space = len.saturating_sub(w.saturating_sub(read));
            let mut ended = false;
            while space > 0 {
                if o.at >= o.pending.len() {
                    o.pending.clear();
                    o.at = 0;
                    match o.src.read(&mut o.pending) {
                        Ok(true) => continue,
                        Ok(false) => {
                            ended = true;
                            break;
                        }
                        Err(e) => {
                            log::warn!("[Music] a streamed recording stopped: {e}");
                            r.failed.store(true, Ordering::Release);
                            break;
                        }
                    }
                }
                let take = ((o.pending.len() - o.at) as u64).min(space);
                for k in 0..take {
                    r.frames[((w + k) & self.pool.mask) as usize].store(pack(o.pending[o.at + k as usize]), Ordering::Relaxed);
                }
                o.at += take as usize;
                w += take;
                space -= take;
                wrote += take;
            }
            r.written.store(w, Ordering::Release);
            if ended {
                r.end.store(w, Ordering::Release);
            }
            if ended || r.failed.load(Ordering::Relaxed) {
                // Nothing more to read: the decoder (and its file) goes now, the ring when the note ends.
                self.open[i] = None;
            }
        }
        wrote
    }
}

/// Run the worker on its own thread until nothing but the worker holds `pool` (every score and the
/// bank that made it are gone).
pub fn start_worker(pool: &Arc<StreamPool>, opener: Arc<dyn SourceOpener>) -> std::io::Result<()> {
    let poll = pool.poll;
    let mine = pool.clone();
    let handle = std::thread::Builder::new().name("music-stream".into()).spawn(move || {
        let mut s = Service::new(mine, opener);
        // A pass fills every ring to the top, so one pass every few milliseconds keeps far ahead of
        // any note (a ring holds 0.68 s; a note reads at most twice its rate), and a request waits at
        // most one sleep (Windows rounds it up to 15.6 ms) against the attack's half second.
        while Arc::strong_count(&s.pool) > 1 {
            s.pass();
            std::thread::park_timeout(poll);
        }
    })?;
    if let Ok(mut g) = pool.worker.lock() {
        *g = Some(handle.thread().clone());
    }
    Ok(())
}

/// Recordings streamed from files: each path registered once, given an id a zone carries.
#[cfg(feature = "kira")]
#[derive(Default)]
pub struct FileOpener {
    paths: std::sync::Mutex<(Vec<std::path::PathBuf>, std::collections::HashMap<std::path::PathBuf, u32>)>,
}

#[cfg(feature = "kira")]
impl FileOpener {
    /// The id of `path`, registering it the first time (on the build's thread; the worker reads the
    /// list only when a note opens a stream, and the audio thread never touches it).
    pub fn register(&self, path: &std::path::Path) -> u32 {
        let mut g = self.paths.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&id) = g.1.get(path) {
            return id;
        }
        let id = g.0.len() as u32;
        g.0.push(path.to_path_buf());
        g.1.insert(path.to_path_buf(), id);
        id
    }

    fn path(&self, id: u32) -> Option<std::path::PathBuf> {
        self.paths.lock().unwrap_or_else(|e| e.into_inner()).0.get(id as usize).cloned()
    }
}

/// A file decoded from its start, its first `skip` frames let go (they are the attack the zone
/// holds): decoding from the start, rather than seeking, gives exactly the frames a whole-file decode
/// gives (a Vorbis packet depends on the one before it), at the cost of decoding half a second more.
#[cfg(feature = "kira")]
struct FileSource {
    reader: crate::decode::Reader,
    skip: u64,
}

#[cfg(feature = "kira")]
impl FrameSource for FileSource {
    fn read(&mut self, out: &mut Vec<[i16; 2]>) -> Result<bool, String> {
        loop {
            let before = out.len();
            if !self.reader.next_into(out)? {
                return Ok(false);
            }
            let got = (out.len() - before) as u64;
            if self.skip == 0 {
                return Ok(true);
            }
            let drop = got.min(self.skip);
            out.drain(before..before + drop as usize);
            self.skip -= drop;
            if out.len() > before {
                return Ok(true);
            }
        }
    }
}

#[cfg(feature = "kira")]
impl SourceOpener for FileOpener {
    fn open(&self, source: u32, start: u64) -> Result<Box<dyn FrameSource>, String> {
        let path = self.path(source).ok_or_else(|| format!("no streamed recording {source}"))?;
        Ok(Box::new(FileSource { reader: crate::decode::Reader::open(&path)?, skip: start }))
    }
}
