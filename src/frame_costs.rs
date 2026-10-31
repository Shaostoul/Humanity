//! Live per-system resource measurement (resource budgets, increment 1 -
//! MEASURE; see `docs/design/resource-budgets.md`).
//!
//! "You cannot allocate what you cannot measure." This module is the one place
//! the engine deposits what a frame ACTUALLY cost, split by pass / stage /
//! allocation bucket, so the Performance page can draw honest pies over it and
//! (increment 2) the operator can hand each system a share of the machine.
//!
//! Four measurements, four pies:
//!   * GPU  - milliseconds per render pass, from wgpu TIMESTAMP_QUERY when the
//!            adapter has it, resolved ONE FRAME LATE so the pipeline is never
//!            stalled. When the feature is missing we fall back to CPU-side
//!            pass-submission time and SAY SO in the pie's footer.
//!   * CPU  - milliseconds per frame stage (pass submission, buffer uploads,
//!            and whatever else calls `stage()`), accumulated per frame.
//!   * VRAM - TRACKED allocations (patch arena, meshes, textures, particles,
//!            render targets), plus what the wgpu allocator reports overall so
//!            the untracked remainder is visible instead of pretended away.
//!   * RAM  - process working set, read once a second while somebody is
//!            looking, via the `memory-stats` crate already in the tree.
//!
//! Design notes:
//!   * ONE process-global store behind a `Mutex`. The alternative was new
//!     fields on `GuiState` + `EngineState`, which are shared wiring files that
//!     three parallel agents edit; telemetry is genuinely process-global, so a
//!     static keeps this increment free of merge hazards.
//!   * Every number is smoothed with an exponential moving average, otherwise
//!     the pie strobes and is unreadable.
//!   * Nothing here is feature-gated and nothing here touches the GPU: it is
//!     std only (no GUI, no `engine::`), so every crate can record into it.
//!   * Everything is cheap when nobody is watching: the page calls `watch()`
//!     each frame it draws, and the expensive inventory sampling (allocator
//!     report, working-set syscall, mesh buffer walk) only runs when that
//!     watch is fresh.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was `renderer::frame_costs`, so the ECS runner (each
//! system's `cpu.system.*` time) and the farming system's stages reached up into the renderer
//! to record a CPU time. The store and its recording calls are std only; what reads the GPU
//! (the timestamp ring, the per-view pass ids, the VRAM walk) is `renderer::gpu_costs`, which
//! publishes into this store.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Smoothing factor for the per-frame numbers: each frame contributes 15% of
/// the displayed value. Fast enough to see a spike, slow enough to read.
const EMA_ALPHA: f32 = 0.15;

/// How the GPU column was obtained, so the UI can be honest about it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GpuTiming {
    /// Real GPU timestamps (wgpu `TIMESTAMP_QUERY`), resolved a frame late.
    Timestamps,
    /// The adapter has no timestamp queries: these are CPU-side pass
    /// submission times, which do NOT include GPU execution.
    CpuFallback,
}

/// One named measurement, ready for the pie.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// Stable id the registry's `sources` list refers to, e.g. `gpu.celestial`.
    pub id: &'static str,
    /// Milliseconds (GPU/CPU entries) or bytes (VRAM/RAM entries, as f64).
    pub value: f64,
}

/// An immutable copy of the latest measurements, handed to the GUI.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub gpu: Vec<Entry>,
    pub cpu: Vec<Entry>,
    pub vram: Vec<Entry>,
    pub ram: Vec<Entry>,
    /// Smoothed wall-clock frame time (ms) measured between frame starts.
    pub frame_ms: f32,
    pub gpu_timing: GpuTiming,
    /// Whether any frame has been measured yet (false before world entry).
    pub has_frames: bool,
    /// How many passes asked for a timestamp slot pair in the last WORLD frame
    /// and were refused because the ring (`MAX_TIMED_PASSES`) was full. Such a
    /// pass is simply untimed, which on the pie looks like "free"; the
    /// Performance page prints this count so that silence is never mistaken
    /// for a measurement. Exact (not smoothed), frozen on UI-only frames like
    /// the rest of the GPU column.
    pub gpu_untimed_passes: u32,
}

/// The `gpu.*` ids whose pass has NO same-named `cpu.*` stage. On an adapter
/// without timestamp queries a `gpu.x` row reads `cpu.x` (`Snapshot::value`);
/// for these ids there is nothing to read, so the row would show 0 and look
/// free. The Performance page consults this list to say "not measurable on
/// this adapter" instead. Kept in lockstep with the source tree by
/// `gui::pages::performance::tests::no_twin_list_matches_the_source_tree`,
/// which derives the same set by scanning `src/` for recorded ids: add a
/// `cpu.x` stage (or a new twin-less `gpu.x` pass) and that test names the
/// drift.
pub const GPU_IDS_WITHOUT_CPU_TWIN: &[&str] = &[
    // The UI-only frame's surface clear and the main egui pass: submitted from
    // lib.rs's frame loop, no stage guard around them.
    "gpu.clear",
    "gpu.ui",
    // The star pass has a `cpu.stars` twin on the LIVE path, but these passes
    // never got one: the transparent celestial pass (ocean shells), the sun
    // shadow depth pass, the sky-view LUT refresh.
    "gpu.celestial_t",
    "gpu.shadow",
    "gpu.sky_view",
    // The whole cloud deck is encoded inside `render_celestial_onto`, under
    // `cpu.celestial`; its four (plus two calibration) passes have no
    // per-pass CPU twin.
    "gpu.cloud_light",
    "gpu.cloud_screen",
    "gpu.cloud_resolve",
    "gpu.cloud_composite",
    "gpu.cloud_profile",
    "gpu.cloud_profile_calib",
    // The fullscreen emission pass (the aurora above the cloud deck), also
    // encoded inside `render_celestial_onto` with no per-pass CPU twin.
    "gpu.aurora",
    // Dormant (nothing calls BloomPass::apply today) and twin-less.
    "gpu.bloom",
];

/// Whether `gpu.x` has a `cpu.x` stage the no-timestamp fallback can read.
pub fn gpu_id_has_cpu_twin(id: &str) -> bool {
    !GPU_IDS_WITHOUT_CPU_TWIN.contains(&id)
}

impl Snapshot {
    /// Whether `id` (a `gpu.*` id) is a real measurement in this snapshot:
    /// always with timestamp queries; in the CPU fallback only when the pass
    /// has a `cpu.*` twin to read. A row whose sources are all unmeasurable
    /// must say so rather than read 0 (the Clouds row on an adapter without
    /// `TIMESTAMP_QUERY` read exactly zero, silently, before this).
    pub fn gpu_measurable(&self, id: &str) -> bool {
        match self.gpu_timing {
            GpuTiming::Timestamps => true,
            GpuTiming::CpuFallback => gpu_id_has_cpu_twin(id),
        }
    }

    /// Look a value up by source id across every column. `gpu.*` ids fall back
    /// to the matching `cpu.*` id when the adapter has no timestamp queries,
    /// which is what makes one registry serve both paths.
    pub fn value(&self, id: &str) -> f64 {
        let find = |list: &Vec<Entry>, want: &str| {
            list.iter().find(|e| e.id == want).map(|e| e.value)
        };
        if let Some(v) = find(&self.gpu, id) {
            return v;
        }
        if let Some(v) = find(&self.cpu, id) {
            return v;
        }
        if let Some(v) = find(&self.vram, id) {
            return v;
        }
        if let Some(v) = find(&self.ram, id) {
            return v;
        }
        // Fallback rule: a `gpu.x` row with no GPU timestamp reads `cpu.x`.
        if self.gpu_timing == GpuTiming::CpuFallback {
            if let Some(rest) = id.strip_prefix("gpu.") {
                let alt = format!("cpu.{rest}");
                if let Some(v) = find(&self.cpu, &alt) {
                    return v;
                }
            }
        }
        0.0
    }
}

#[derive(Default)]
struct Store {
    gpu: Vec<Entry>,
    cpu: Vec<Entry>,
    /// This frame's CPU accumulation (several calls per id per frame are
    /// summed, then folded into `cpu` at the next frame boundary).
    cpu_accum: Vec<Entry>,
    vram: Vec<Entry>,
    ram: Vec<Entry>,
    frame_ms: f32,
    last_frame_start: Option<Instant>,
    timestamps: bool,
    has_frames: bool,
    /// Slot-pair refusals in the last world frame (see `Snapshot::gpu_untimed_passes`).
    gpu_untimed: u32,
    /// Each CPU id's largest single frame, and the longest frame, in this window and the one
    /// before (`note_worst`, `worst`).
    worst_cur: Vec<Entry>,
    worst_prev: Vec<Entry>,
    /// The spans' own CPU time (`close_span`): this frame's accumulation, its average, and its worst
    /// frames, beside `cpu_accum`, `cpu` and `worst_cur` / `worst_prev`.
    thread_accum: Vec<Entry>,
    thread: Vec<Entry>,
    thread_worst_cur: Vec<Entry>,
    thread_worst_prev: Vec<Entry>,
    frame_worst_cur: f32,
    frame_worst_prev: f32,
    worst_since: Option<Instant>,
}

/// How long one window of worst frames lasts. What is reported is the worst of this window and
/// the one before, so a spike stays visible for 5 to 10 s.
const WORST_WINDOW: Duration = Duration::from_secs(5);

/// Remember the worst frames (docs/design/sim-frame-cost.md, Stutter, 2026-10-30). Every `cpu.*`
/// value is an exponential average, which hides a job that costs 17 ms once a second: the average
/// barely moves while the player feels a hitch every second. So beside each average the largest
/// single-frame value is kept, over two windows of `WORST_WINDOW`. The clock is passed in so the
/// test can step it.
fn note_worst(s: &mut Store, now: Instant, frame_ms: Option<f32>, frame: &[Entry]) {
    if s.worst_since.map_or(true, |t| now.saturating_duration_since(t) >= WORST_WINDOW) {
        s.worst_prev = std::mem::take(&mut s.worst_cur);
        s.thread_worst_prev = std::mem::take(&mut s.thread_worst_cur);
        s.frame_worst_prev = s.frame_worst_cur;
        s.frame_worst_cur = 0.0;
        s.worst_since = Some(now);
    }
    if let Some(ms) = frame_ms {
        s.frame_worst_cur = s.frame_worst_cur.max(ms);
    }
    for e in frame {
        match s.worst_cur.iter_mut().find(|w| w.id == e.id) {
            Some(w) => w.value = w.value.max(e.value),
            None => s.worst_cur.push(Entry { id: e.id, value: e.value }),
        }
    }
}

/// Each span's own CPU time: its average and its worst frame in the last 5 to 10 s (`close_span`).
pub fn thread_times() -> (Vec<Entry>, Vec<Entry>) {
    let Ok(s) = store().lock() else { return (Vec::new(), Vec::new()) };
    let avg = s.thread.iter().map(|e| Entry { id: e.id, value: e.value }).collect();
    let mut worst: Vec<Entry> = s.thread_worst_prev.iter().map(|e| Entry { id: e.id, value: e.value }).collect();
    for e in &s.thread_worst_cur {
        match worst.iter_mut().find(|w| w.id == e.id) {
            Some(w) => w.value = w.value.max(e.value),
            None => worst.push(Entry { id: e.id, value: e.value }),
        }
    }
    (avg, worst)
}

/// The longest frame (ms) and each CPU id's largest single-frame value in the last 5 to 10 s.
pub fn worst() -> (f32, Vec<Entry>) {
    let Ok(s) = store().lock() else { return (0.0, Vec::new()) };
    let mut out: Vec<Entry> = s.worst_prev.iter().map(|e| Entry { id: e.id, value: e.value }).collect();
    for e in &s.worst_cur {
        match out.iter_mut().find(|w| w.id == e.id) {
            Some(w) => w.value = w.value.max(e.value),
            None => out.push(Entry { id: e.id, value: e.value }),
        }
    }
    (s.frame_worst_cur.max(s.frame_worst_prev), out)
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// Last moment a UI surface asked for the numbers (microseconds since the
/// process epoch below). Sampling that costs anything is skipped unless this
/// is fresh, so a normal play session pays nothing for this module.
static LAST_WATCH_US: AtomicU64 = AtomicU64::new(0);

fn epoch() -> Instant {
    static E: OnceLock<Instant> = OnceLock::new();
    *E.get_or_init(Instant::now)
}

/// Called by the Performance page every frame it draws: "somebody is looking".
pub fn watch() {
    // `.max(1)` because 0 is the "never watched" sentinel, and the very first
    // call can land in the same microsecond the epoch is created.
    LAST_WATCH_US.store((epoch().elapsed().as_micros() as u64).max(1), Ordering::Relaxed);
}

/// True while a UI surface asked for the numbers within the last second.
pub fn watched() -> bool {
    let last = LAST_WATCH_US.load(Ordering::Relaxed);
    if last == 0 {
        return false;
    }
    let now = epoch().elapsed().as_micros() as u64;
    now.saturating_sub(last) < 1_000_000
}

/// Fold `value` into `list` under `id` with exponential smoothing.
fn ema_into(list: &mut Vec<Entry>, id: &'static str, value: f64) {
    if let Some(e) = list.iter_mut().find(|e| e.id == id) {
        e.value = e.value * (1.0 - EMA_ALPHA as f64) + value * EMA_ALPHA as f64;
    } else {
        list.push(Entry { id, value });
    }
}

/// Replace (no smoothing) - for inventories that are already exact.
fn set_into(list: &mut Vec<Entry>, id: &'static str, value: f64) {
    if let Some(e) = list.iter_mut().find(|e| e.id == id) {
        e.value = value;
    } else {
        list.push(Entry { id, value });
    }
}

/// Record a GPU pass duration in milliseconds. Single-sample entry point, used
/// by the headless UI snapshot to stage plausible numbers; the live renderer
/// path goes through `publish_gpu_frame`, which sums same-id passes first.
pub fn record_gpu(id: &'static str, ms: f32) {
    if let Ok(mut s) = store().lock() {
        ema_into(&mut s.gpu, id, ms as f64);
    }
}

/// Publish ONE frame's worth of resolved GPU pass durations.
///
/// Two properties the naive "record each sample as it is read" loop did not
/// have, and both are load-bearing for an honest pie:
///   * durations that share an id are SUMMED. Several passes legitimately
///     carry one id - the scene pass runs twice per frame, bloom is four
///     fullscreen passes - and folding each one separately through the EMA
///     published something between them instead of their total.
///   * an id that did NOT run this frame decays toward zero, the same rule the
///     CPU column already followed. Without it, turning god rays or SSAO off
///     left their last measured cost sitting in the pie forever.
pub(crate) fn publish_gpu_frame(samples: &[(&'static str, f32)]) {
    let mut totals: Vec<(&'static str, f32)> = Vec::new();
    for (id, ms) in samples {
        match totals.iter_mut().find(|(i, _)| i == id) {
            Some((_, acc)) => *acc += *ms,
            None => totals.push((*id, *ms)),
        }
    }
    if let Ok(mut s) = store().lock() {
        let ids: Vec<&'static str> = s.gpu.iter().map(|e| e.id).collect();
        for id in ids {
            let v = totals
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            ema_into(&mut s.gpu, id, v as f64);
        }
        for (id, v) in totals {
            if !s.gpu.iter().any(|e| e.id == id) {
                s.gpu.push(Entry { id, value: v as f64 });
            }
        }
    }
}

/// What to do with one frame's harvested GPU durations, given whose frame
/// they were. This is the GPU twin of the CPU side's `begin_frame` /
/// `discard_frame` split, and it is the whole fix for the Performance page
/// reading zeros:
///
///   * a WORLD frame publishes (sum same-id passes, decay absent ones), as
///     `publish_gpu_frame` describes;
///   * a UI-ONLY frame (a full-screen page is open, the engine only clears
///     the surface and draws egui) publishes NOTHING. Its samples are thrown
///     away and every `gpu.*` value stays frozen at the last world frame.
///
/// Before this split the harvest published unconditionally, so opening the
/// Performance page (itself a UI-only page) EMA'd every world pass toward
/// zero: 3.9 percent of its true value after 20 frames, 0.006 percent after
/// 60. The Clouds, In-world screens and World surface rows read about zero
/// exactly where the operator was told to read them. The UI-only passes
/// themselves (`gpu.clear`, `gpu.ui`) freeze too, deliberately: one rule for
/// the whole column, "the pie shows your last rendered world frame", the
/// same promise the CPU column and the page's own caption make.
pub(crate) fn publish_gpu_harvest(samples: &[(&'static str, f32)], world: bool) {
    if world {
        publish_gpu_frame(samples);
    }
    // UI-only: discard. Nothing to do; the store is untouched.
}

/// Record how many passes went untimed in the last world frame because the
/// timestamp ring was full. Exact, not smoothed: it is a count, and a single
/// refused frame is worth knowing about.
pub(crate) fn set_gpu_untimed(count: u32) {
    if let Ok(mut s) = store().lock() {
        s.gpu_untimed = count;
    }
}

/// Interned ids built from a runtime name (ECS system names, which are `&str`
/// borrowed from the system, not `&'static str`). Bounded by construction: the
/// engine registers its systems once at boot, so this map holds one entry per
/// system for the process lifetime and never grows again.
static INTERNED: OnceLock<Mutex<std::collections::HashMap<String, &'static str>>> =
    OnceLock::new();

/// Turn `prefix` + a runtime `name` into a stable measurement id.
///
/// The name is slugged (lowercase, non-alphanumeric runs become `_`, a trailing
/// "system" is dropped) so `FarmingSystem` measures as `cpu.system.farming`.
/// Two systems that slug the same simply share a row, which is honest: their
/// costs are summed under one name rather than one silently hiding the other.
pub fn intern_id(prefix: &str, name: &str) -> &'static str {
    let mut slug = String::with_capacity(name.len());
    let mut prev_us = true; // leading underscores are suppressed
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us {
            slug.push('_');
            prev_us = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    if slug.len() > "system".len() && slug.ends_with("system") {
        slug.truncate(slug.len() - "system".len());
        while slug.ends_with('_') {
            slug.pop();
        }
    }
    if slug.is_empty() {
        slug.push_str("unnamed");
    }
    let key = format!("{prefix}{slug}");
    let map = INTERNED.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut m = match map.lock() {
        Ok(m) => m,
        // A poisoned interner must not take the engine down over telemetry.
        Err(e) => e.into_inner(),
    };
    if let Some(id) = m.get(&key) {
        return id;
    }
    let leaked: &'static str = Box::leak(key.clone().into_boxed_str());
    m.insert(key, leaked);
    leaked
}

/// Accumulate a CPU stage duration into the frame in progress.
pub fn record_cpu(id: &'static str, dur: Duration) {
    if let Ok(mut s) = store().lock() {
        let ms = dur.as_secs_f64() * 1000.0;
        if let Some(e) = s.cpu_accum.iter_mut().find(|e| e.id == id) {
            e.value += ms;
        } else {
            s.cpu_accum.push(Entry { id, value: ms });
        }
    }
}

/// Set a tracked VRAM bucket, in bytes.
pub fn set_vram(id: &'static str, bytes: u64) {
    if let Ok(mut s) = store().lock() {
        set_into(&mut s.vram, id, bytes as f64);
    }
}

/// Add to a tracked VRAM bucket (creation-time accounting, e.g. textures).
pub fn add_vram(id: &'static str, bytes: u64) {
    if let Ok(mut s) = store().lock() {
        let cur = s.vram.iter().find(|e| e.id == id).map(|e| e.value).unwrap_or(0.0);
        set_into(&mut s.vram, id, cur + bytes as f64);
    }
}

/// Per-key contributions to a bucket (material index -> texture bytes), so a
/// REPLACED allocation replaces its old entry instead of double-counting it.
static KEYED: OnceLock<Mutex<std::collections::HashMap<(&'static str, u64), u64>>> =
    OnceLock::new();

/// Record `bytes` for `key` inside `bucket` and republish the bucket total.
/// Used for material textures, whose albedo can be swapped in place.
pub fn set_vram_keyed(bucket: &'static str, key: u64, bytes: u64) {
    let map = KEYED.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let total = {
        let mut m = match map.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        m.insert((bucket, key), bytes);
        m.iter().filter(|((b, _), _)| *b == bucket).map(|(_, v)| *v).sum::<u64>()
    };
    set_vram(bucket, total);
}

/// Set a RAM bucket, in bytes.
pub fn set_ram(id: &'static str, bytes: u64) {
    if let Ok(mut s) = store().lock() {
        set_into(&mut s.ram, id, bytes as f64);
    }
}

/// Override the smoothed frame time. The renderer derives this from real
/// frame-to-frame deltas in `begin_frame`; this setter exists for the headless
/// UI snapshot, which lays the page out without a render loop.
pub fn set_frame_ms(ms: f32) {
    if let Ok(mut s) = store().lock() {
        s.frame_ms = ms;
        s.has_frames = true;
    }
}

/// Record which way the GPU column was measured.
pub fn set_gpu_timing(timestamps: bool) {
    if let Ok(mut s) = store().lock() {
        s.timestamps = timestamps;
    }
}

/// Close the frame in progress: fold the CPU accumulation into the smoothed
/// values and update the frame-time clock. Called once per frame by the
/// renderer's surface acquisition (the one call site every frame passes
/// through exactly once).
pub fn begin_frame() {
    if let Ok(mut s) = store().lock() {
        let now = Instant::now();
        if let Some(prev) = s.last_frame_start {
            let ms = now.duration_since(prev).as_secs_f32() * 1000.0;
            // Ignore absurd gaps (window minimised, breakpoint, load screen).
            if ms > 0.0 && ms < 1000.0 {
                s.frame_ms = if s.frame_ms <= 0.0 {
                    ms
                } else {
                    s.frame_ms * (1.0 - EMA_ALPHA) + ms * EMA_ALPHA
                };
            }
        }
        let frame_ms = s.last_frame_start.map(|prev| now.duration_since(prev).as_secs_f32() * 1000.0).filter(|ms| *ms > 0.0 && *ms < 1000.0);
        s.last_frame_start = Some(now);
        s.has_frames = true;
        let accum = std::mem::take(&mut s.cpu_accum);
        note_worst(&mut s, now, frame_ms, &accum);
        // The spans' own CPU time: averaged like the stages, worst-tracked in the same windows.
        let thread_accum = std::mem::take(&mut s.thread_accum);
        let ids: Vec<&'static str> = s.thread.iter().map(|e| e.id).collect();
        for id in ids {
            let v = thread_accum.iter().find(|e| e.id == id).map(|e| e.value).unwrap_or(0.0);
            ema_into(&mut s.thread, id, v);
        }
        for e in &thread_accum {
            if !s.thread.iter().any(|x| x.id == e.id) {
                s.thread.push(Entry { id: e.id, value: e.value });
            }
            match s.thread_worst_cur.iter_mut().find(|w| w.id == e.id) {
                Some(w) => w.value = w.value.max(e.value),
                None => s.thread_worst_cur.push(Entry { id: e.id, value: e.value }),
            }
        }
        // Stages that did not run this frame decay toward zero, otherwise a
        // one-off cost (a single patch upload) would stick in the pie forever.
        let ids: Vec<&'static str> = s.cpu.iter().map(|e| e.id).collect();
        for id in ids {
            let v = accum.iter().find(|e| e.id == id).map(|e| e.value).unwrap_or(0.0);
            ema_into(&mut s.cpu, id, v);
        }
        for e in accum {
            if !s.cpu.iter().any(|x| x.id == e.id) {
                s.cpu.push(e);
            }
        }
    }
}

/// A frame that did NOT render the 3D world (a full-screen page is open, so
/// the engine only clears the surface). Its costs are meaningless as a budget -
/// no render pass ran - so we throw the partial accumulation away and leave the
/// published numbers FROZEN at the last real world frame. That is exactly what
/// the Performance page wants to show you: what your last rendered frame cost.
pub fn discard_frame() {
    if let Ok(mut s) = store().lock() {
        s.cpu_accum.clear();
        s.thread_accum.clear();
        // Drop the clock too: the gap spent sitting in a menu is not a frame.
        s.last_frame_start = None;
    }
}

/// A copy of the current measurements for the UI.
pub fn snapshot() -> Snapshot {
    let s = match store().lock() {
        Ok(s) => s,
        Err(_) => {
            return Snapshot {
                gpu: Vec::new(),
                cpu: Vec::new(),
                vram: Vec::new(),
                ram: Vec::new(),
                frame_ms: 0.0,
                gpu_timing: GpuTiming::CpuFallback,
                has_frames: false,
                gpu_untimed_passes: 0,
            }
        }
    };
    Snapshot {
        gpu: s.gpu.clone(),
        cpu: s.cpu.clone(),
        vram: s.vram.clone(),
        ram: s.ram.clone(),
        frame_ms: s.frame_ms,
        gpu_timing: if s.timestamps { GpuTiming::Timestamps } else { GpuTiming::CpuFallback },
        has_frames: s.has_frames,
        gpu_untimed_passes: s.gpu_untimed,
    }
}

/// Whether `HUMANITY_FRAME_COSTS` asked for the JSON drop (read once).
pub(crate) fn dump_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("HUMANITY_FRAME_COSTS")
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false)
    })
}

/// Write the current snapshot to `debug/frame_costs.json`. Dev tooling: this
/// is how a probe-rig run reports a REAL breakdown back to whoever launched it.
pub fn dump_json() {
    let body = snapshot_json();
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write("debug/frame_costs.json", body.to_string());
}

/// The current snapshot as the JSON `dump_json` writes: one shape for the rig's
/// file and the dev MCP's `game_frame_costs` (engine/mcp, 2026-10-22).
pub fn snapshot_json() -> serde_json::Value {
    let s = snapshot();
    let (worst_frame, worst_cpu) = worst();
    let (thread_avg, thread_worst) = thread_times();
    let col = |list: &[Entry]| -> serde_json::Value {
        serde_json::Value::Object(
            list.iter()
                .map(|e| (e.id.to_string(), serde_json::json!(e.value)))
                .collect(),
        )
    };
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "frame_ms": s.frame_ms,
        "gpu_timing": match s.gpu_timing {
            GpuTiming::Timestamps => "timestamps",
            GpuTiming::CpuFallback => "cpu_fallback",
        },
        "gpu_ms": col(&s.gpu),
        "cpu_ms": col(&s.cpu),
        "vram_bytes": col(&s.vram),
        "ram_bytes": col(&s.ram),
        // Passes refused a timestamp slot in the last world frame (0 = every
        // pass was timed). Non-zero means the `gpu_ms` column is missing
        // passes and MAX_TIMED_PASSES needs raising.
        "gpu_untimed_passes": s.gpu_untimed_passes,
        // The worst single frame in the last 5 to 10 s, beside the averages above (Stutter,
        // sim-frame-cost.md): a span whose worst is far above its average is a periodic spike.
        "frame_ms_worst": worst_frame,
        "cpu_worst_ms": col(&worst_cpu),
        // Each span's own CPU time (the thread's cycles, so OS pauses and waits are left out):
        // average and worst. Empty where the platform cannot count a thread's cycles.
        "thread_ms": col(&thread_avg),
        "thread_worst_ms": col(&thread_worst),
    })
}

/// Wipe every measurement (tests only; the live store is append-only).
#[cfg(test)]
pub fn reset_for_test() {
    if let Ok(mut s) = store().lock() {
        *s = Store::default();
    }
}

/// Serialise every test that touches the process-global store. Cargo runs lib
/// tests on many threads, and the ECS runner's timing test lives in another
/// module, so the lock has to be reachable from both.
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Scope timer: `let _t = frame_costs::stage("cpu.celestial");` at the top of a
/// block records its wall time into the CPU column when the guard drops. This
/// is the `boot_timer` pattern, per frame instead of per boot phase.
pub struct StageTimer {
    id: &'static str,
    t0: Instant,
}

impl StageTimer {
    pub fn new(id: &'static str) -> Self {
        Self { id, t0: Instant::now() }
    }
}

impl Drop for StageTimer {
    fn drop(&mut self) {
        record_cpu(self.id, self.t0.elapsed());
    }
}

/// Convenience constructor so call sites read `let _t = stage("cpu.x");`.
pub fn stage(id: &'static str) -> StageTimer {
    StageTimer::new(id)
}

// ── Spans: the frame tiled into stretches that do not overlap ──
//
// docs/design/sim-frame-cost.md, increment 0 (2026-10-30). About half of the
// CPU frame had no timer, so nobody could say where it went: the Garden data's
// cost was estimated at 4-9 ms and measured at 16-20. A span is one stretch of
// the RedrawRequested handler between two `span` calls placed at its top-level
// boundaries, so the spans add up to the frame by construction:
// `cpu.frame_total` ~= `cpu.fps_cap_sleep` + the sum of `cpu.span.*`. They
// are a family of their own, measured beside the stages (which they contain,
// `cpu.systems` inside `cpu.span.ecs` and so on), and never a pie row: the
// Performance page would count everything twice. One marker per boundary
// rather than a timer per stretch, so a stretch cannot be left untimed by a
// missing close and a new one is one line.

/// One open span: its id, when it opened, and the thread's CPU cycles then (None where the platform
/// cannot say).
type OpenSpan = (&'static str, Instant, Option<u64>);

thread_local! {
    /// The open span, and whether spans are recorded.
    static SPAN: std::cell::Cell<(Option<OpenSpan>, bool)> = const { std::cell::Cell::new((None, true)) };
}

// ── The spans' own CPU time (2026-10-30) ──
//
// A span's clock time includes everything that happened to the thread meanwhile: on a busy PC the
// OS pauses the game (it runs below normal priority while developing) partway through a span, and
// the first worst-frame readings showed nearly every span's worst jumping at once for that reason.
// So each span also counts the CPU cycles the thread actually used (Windows' QueryThreadCycleTime;
// none elsewhere, where the column stays empty), turned into milliseconds at the thread's own rate,
// which is learned as the HIGHEST cycles-per-millisecond seen over any span of at least half a
// millisecond: a stretch the thread ran through unpaused shows the true rate, a pause only lowers it.
// Waiting (a blocked present) shows as clock time without cycles too, so the gap between the two is
// "not working", whatever the reason, and a span's CPU-time worst is the code's own spike.

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThread() -> isize;
    fn QueryThreadCycleTime(thread: isize, cycles: *mut u64) -> i32;
}

/// The calling thread's CPU cycles so far.
fn thread_cycles() -> Option<u64> {
    #[cfg(windows)]
    {
        let mut c: u64 = 0;
        // SAFETY: GetCurrentThread returns a pseudo-handle that needs no closing, and `c` is a
        // valid place for the one u64 QueryThreadCycleTime writes.
        let ok = unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut c) };
        if ok != 0 {
            return Some(c);
        }
    }
    None
}

/// The highest cycles per millisecond seen (bits of an f64; 0 = none yet).
static CYCLES_PER_MS: AtomicU64 = AtomicU64::new(0);

/// Close the stretch `open` at `now`: its clock time to `record_cpu`, and its CPU time (once the
/// rate is known) to the thread column.
fn close_span(open: OpenSpan, now: Instant, cycles_now: Option<u64>) {
    let (id, t0, c0) = open;
    let wall = now.duration_since(t0);
    record_cpu(id, wall);
    let (Some(c0), Some(c1)) = (c0, cycles_now) else { return };
    let cycles = c1.saturating_sub(c0) as f64;
    let wall_ms = wall.as_secs_f64() * 1000.0;
    let mut rate = f64::from_bits(CYCLES_PER_MS.load(Ordering::Relaxed));
    if wall_ms >= 0.5 && cycles / wall_ms > rate {
        rate = cycles / wall_ms;
        CYCLES_PER_MS.store(rate.to_bits(), Ordering::Relaxed);
    }
    if rate > 0.0 {
        record_thread(id, cycles / rate);
    }
}

/// Accumulate a span's CPU time (ms) into the frame in progress.
fn record_thread(id: &'static str, ms: f64) {
    if let Ok(mut s) = store().lock() {
        match s.thread_accum.iter_mut().find(|e| e.id == id) {
            Some(e) => e.value += ms,
            None => s.thread_accum.push(Entry { id, value: ms }),
        }
    }
}

/// Open the frame's first span: forget anything left open (a frame that
/// returned early), and record again (see `span_stop_recording`).
pub fn span_begin(id: &'static str) {
    let c = thread_cycles();
    SPAN.with(|s| s.set((Some((id, Instant::now(), c)), true)));
}

/// Close the open span, recording it, and open `id`.
pub fn span(id: &'static str) {
    let now = Instant::now();
    let c = thread_cycles();
    SPAN.with(|s| {
        let (open, recording) = s.get();
        if let (Some(o), true) = (open, recording) {
            close_span(o, now, c);
        }
        s.set((Some((id, now, c)), recording));
    });
}

/// Close the open span, recording it, and open none.
pub fn span_end() {
    let now = Instant::now();
    let c = thread_cycles();
    SPAN.with(|s| {
        let (open, recording) = s.get();
        if let (Some(o), true) = (open, recording) {
            close_span(o, now, c);
        }
        s.set((None, recording));
    });
}

/// From now until the next `span_begin`, record no span. For a frame that
/// does not draw the world (a page covers it): its accumulation is thrown
/// away at the surface acquire (`discard_frame`), and a span recorded AFTER
/// that would leak into the next world frame, the same rule `cpu.frame_total`
/// follows.
pub fn span_stop_recording() {
    SPAN.with(|s| {
        let (open, _) = s.get();
        s.set((open, false));
    });
}

/// Read the process working set (and committed bytes) into the RAM column.
/// Uses the `memory-stats` crate already in the tree (Windows
/// `GetProcessMemoryInfo`, Linux `/proc`, macOS task_info) rather than adding
/// a new FFI shim; on a platform it cannot read, the values simply stay 0 and
/// the pie shows the "not available on this platform" note.
#[cfg(feature = "native")]
pub fn sample_process_memory() {
    if let Some(u) = memory_stats::memory_stats() {
        let resident = u.physical_mem as u64;
        let committed = u.virtual_mem as u64;
        set_ram("ram.resident", resident);
        set_ram("ram.committed_extra", committed.saturating_sub(resident));
    }
}

/// Relay/headless builds have no `memory-stats` dependency (it is native-only).
#[cfg(not(feature = "native"))]
pub fn sample_process_memory() {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store is process-global, so these tests must not run concurrently
    /// with each other (cargo runs lib tests on many threads by default), nor
    /// with the ECS runner's timing test, which writes the same store.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        super::test_lock()
    }

    /// The CPU column accumulates several calls within one frame and only
    /// publishes at the frame boundary - the property the pie relies on to
    /// show "milliseconds per frame" rather than "milliseconds per call".
    #[test]
    fn cpu_stages_accumulate_within_a_frame_then_publish() {
        let _g = serial();
        reset_for_test();
        begin_frame();
        record_cpu("cpu.test_stage", Duration::from_micros(500));
        record_cpu("cpu.test_stage", Duration::from_micros(500));
        // Not published yet: the frame is still open.
        assert_eq!(snapshot().value("cpu.test_stage"), 0.0);
        begin_frame();
        let v = snapshot().value("cpu.test_stage");
        // First publish is EMA from 0 toward 1.0 ms.
        assert!(v > 0.0 && v <= 1.0, "expected a smoothed value in (0, 1] ms, got {v}");
        reset_for_test();
    }

    /// Spans tile the frame: each marker closes the stretch before it, the
    /// stretches add up to the whole, and after `span_stop_recording` (a frame
    /// a page covers, past its acquire) nothing is recorded until the next
    /// frame's `span_begin`. Proven able to fail: with `span` recording
    /// regardless of the switch, the `cpu.span.test_c` assertion fires.
    #[test]
    fn spans_tile_the_frame_and_stop_when_told() {
        let _g = serial();
        reset_for_test();
        begin_frame();
        let t0 = Instant::now();
        span_begin("cpu.span.test_a");
        std::thread::sleep(Duration::from_millis(3));
        span("cpu.span.test_b");
        std::thread::sleep(Duration::from_millis(2));
        span_end();
        let whole = t0.elapsed().as_secs_f64() * 1000.0;
        // A stretch opened and closed while not recording leaves nothing.
        span_begin("cpu.span.test_c");
        span_stop_recording();
        std::thread::sleep(Duration::from_millis(1));
        span_end();
        let acc = |id: &str| {
            store().lock().unwrap().cpu_accum.iter().find(|e| e.id == id).map(|e| e.value)
        };
        let (a, b) = (acc("cpu.span.test_a").unwrap(), acc("cpu.span.test_b").unwrap());
        assert!(a >= 3.0 && b >= 2.0, "each stretch holds its own time: a {a} ms, b {b} ms");
        assert!(a + b <= whole + 0.01, "the stretches never add up to more than the whole: {a} + {b} > {whole}");
        assert!(acc("cpu.span.test_c").is_none(), "a span closed while not recording was recorded");
        reset_for_test();
    }

    /// A SPAN'S CPU TIME LEAVES OUT TIME THE THREAD DID NOT RUN (2026-10-30): a span spent working
    /// records about its clock time, a span spent asleep (as a thread the OS paused would be)
    /// records almost none, though its clock time is the same. Windows only (elsewhere the column
    /// is empty). Proven able to fail: with `close_span` recording the clock time into the thread
    /// column, the "asleep" assertion fires.
    #[cfg(windows)]
    #[test]
    fn a_spans_cpu_time_leaves_out_a_pause() {
        let _g = serial();
        reset_for_test();
        let spin = |ms: u64| {
            let t = Instant::now();
            let mut x = 0u64;
            while t.elapsed() < Duration::from_millis(ms) {
                x = std::hint::black_box(x.wrapping_add(1));
            }
        };
        // Twice working first, so the rate is learned from an unpaused stretch.
        span_begin("cpu.span.test_work");
        spin(4);
        span("cpu.span.test_work2");
        spin(4);
        span("cpu.span.test_sleep");
        std::thread::sleep(Duration::from_millis(8));
        span_end();
        let acc = |id: &str| store().lock().unwrap().thread_accum.iter().find(|e| e.id == id).map(|e| e.value).unwrap_or(0.0);
        let wall = |id: &str| store().lock().unwrap().cpu_accum.iter().find(|e| e.id == id).map(|e| e.value).unwrap_or(0.0);
        assert!(acc("cpu.span.test_work2") > 0.5 * wall("cpu.span.test_work2"), "working: {} of {} ms", acc("cpu.span.test_work2"), wall("cpu.span.test_work2"));
        assert!(acc("cpu.span.test_sleep") < 0.25 * wall("cpu.span.test_sleep"), "asleep: {} of {} ms", acc("cpu.span.test_sleep"), wall("cpu.span.test_sleep"));
        reset_for_test();
    }

    /// THE WORST FRAME IS KEPT BESIDE THE AVERAGE (Stutter, 2026-10-30): one 17 ms frame among
    /// 1 ms ones stays the reported worst for this window and the next, then is forgotten.
    /// Proven able to fail: with `note_worst` keeping the last value instead of the largest, the
    /// "a spike is the worst" assertion fires.
    #[test]
    fn the_worst_frame_is_kept_for_two_windows_then_forgotten() {
        let _g = serial();
        reset_for_test();
        let t0 = Instant::now();
        let mut s = Store::default();
        let f = |v: f64| vec![Entry { id: "cpu.span.test", value: v }];
        note_worst(&mut s, t0, Some(16.0), &f(1.0));
        note_worst(&mut s, t0 + Duration::from_millis(100), Some(33.0), &f(17.0));
        note_worst(&mut s, t0 + Duration::from_millis(200), Some(16.0), &f(1.0));
        let w = |s: &Store| {
            let a = s.worst_cur.iter().chain(s.worst_prev.iter()).filter(|e| e.id == "cpu.span.test").map(|e| e.value).fold(0.0, f64::max);
            (s.frame_worst_cur.max(s.frame_worst_prev), a)
        };
        assert_eq!(w(&s), (33.0, 17.0), "a spike is the worst, not the last frame");
        // The next window still shows it (worst of this window and the one before).
        note_worst(&mut s, t0 + WORST_WINDOW + Duration::from_millis(1), Some(16.0), &f(1.0));
        assert_eq!(w(&s), (33.0, 17.0), "still shown one window later");
        // Two windows on, it has gone.
        note_worst(&mut s, t0 + WORST_WINDOW * 2 + Duration::from_millis(2), Some(16.0), &f(1.0));
        assert_eq!(w(&s), (16.0, 1.0), "forgotten two windows later");
    }

    /// A stage that stops running must decay to zero instead of sticking in
    /// the pie forever (a one-off upload would otherwise look permanent).
    #[test]
    fn idle_cpu_stages_decay_toward_zero() {
        let _g = serial();
        reset_for_test();
        begin_frame();
        record_cpu("cpu.decay", Duration::from_millis(10));
        begin_frame();
        let first = snapshot().value("cpu.decay");
        for _ in 0..60 {
            begin_frame();
        }
        let later = snapshot().value("cpu.decay");
        assert!(later < first * 0.2, "stale stage did not decay: {first} -> {later}");
        reset_for_test();
    }

    /// A `gpu.*` row with no timestamp data must read the matching `cpu.*`
    /// stage, which is what lets ONE registry serve both measurement paths.
    #[test]
    fn gpu_rows_fall_back_to_cpu_stage_times() {
        let _g = serial();
        reset_for_test();
        set_gpu_timing(false);
        begin_frame();
        record_cpu("cpu.celestial", Duration::from_millis(4));
        begin_frame();
        let snap = snapshot();
        assert_eq!(snap.gpu_timing, GpuTiming::CpuFallback);
        assert!(snap.value("gpu.celestial") > 0.0, "gpu.* did not fall back to cpu.*");
        reset_for_test();
    }

    /// Byte inventories are exact, not smoothed: a 64 MB buffer must read back
    /// as 64 MB the first time it is reported.
    #[test]
    fn vram_buckets_are_exact() {
        let _g = serial();
        reset_for_test();
        set_vram("vram.test", 64 * 1024 * 1024);
        assert_eq!(snapshot().value("vram.test"), 67_108_864.0);
        add_vram("vram.test", 1024);
        assert_eq!(snapshot().value("vram.test"), 67_109_888.0);
        reset_for_test();
    }

    /// A UI-only frame must not decay the published numbers: opening the
    /// Performance page has to show the last WORLD frame, not a page of zeros
    /// one second later.
    #[test]
    fn ui_only_frames_freeze_rather_than_decay_the_numbers() {
        let _g = serial();
        reset_for_test();
        begin_frame();
        record_cpu("cpu.frozen", Duration::from_millis(5));
        begin_frame();
        let published = snapshot().value("cpu.frozen");
        assert!(published > 0.0);
        // 120 UI-only frames (two seconds of sitting on the page).
        for _ in 0..120 {
            record_cpu("cpu.frozen", Duration::from_millis(5));
            discard_frame();
        }
        assert_eq!(
            snapshot().value("cpu.frozen"),
            published,
            "UI-only frames must leave the published costs untouched"
        );
        reset_for_test();
    }

    /// The GPU twin of the test above. The harvest layer must publish a
    /// world frame's batch and DISCARD a UI-only frame's, so the world passes
    /// hold the value they had when the Performance page (a UI-only page)
    /// was opened. Before `publish_gpu_harvest` the harvest published
    /// unconditionally, and 60 UI-only frames (one second on the page) took
    /// every world pass to 0.006 percent of its value: this test fails on
    /// that code (proven by making the UI-only branch publish, 2026-09-18).
    #[test]
    fn ui_only_frames_freeze_the_gpu_column_too() {
        let _g = serial();
        reset_for_test();
        // A run of world frames so the EMA has settled on a real number.
        for _ in 0..40 {
            publish_gpu_harvest(&[("gpu.scene", 4.0), ("gpu.cloud_screen", 9.0)], true);
        }
        let scene = snapshot().value("gpu.scene");
        let clouds = snapshot().value("gpu.cloud_screen");
        assert!(scene > 3.9 && clouds > 8.9, "EMA did not settle: {scene} {clouds}");
        // 60 UI-only frames: only the clear and the egui pass ran.
        for _ in 0..60 {
            publish_gpu_harvest(&[("gpu.clear", 0.05), ("gpu.ui", 0.4)], false);
        }
        let snap = snapshot();
        assert_eq!(
            snap.value("gpu.scene"),
            scene,
            "a UI-only frame decayed a world pass; the Performance page would read zero"
        );
        assert_eq!(snap.value("gpu.cloud_screen"), clouds);
        // The UI-only passes freeze too: one rule for the column (the pie is
        // the last WORLD frame), so nothing from the page's own frames leaks in.
        assert_eq!(snap.value("gpu.ui"), 0.0, "a UI-only frame's own passes must not publish");
        reset_for_test();
    }


    /// In the CPU fallback a `gpu.*` id is only a measurement when a
    /// same-named `cpu.*` stage exists to read; the Clouds passes have none,
    /// so the page must say "not measurable" rather than show their 0.
    #[test]
    fn gpu_ids_without_a_cpu_twin_are_not_measurable_in_fallback() {
        let mk = |t| Snapshot {
            gpu: Vec::new(),
            cpu: Vec::new(),
            vram: Vec::new(),
            ram: Vec::new(),
            frame_ms: 0.0,
            gpu_timing: t,
            has_frames: true,
            gpu_untimed_passes: 0,
        };
        let fallback = mk(GpuTiming::CpuFallback);
        assert!(fallback.gpu_measurable("gpu.scene"), "gpu.scene has cpu.scene");
        assert!(!fallback.gpu_measurable("gpu.cloud_screen"), "the cloud passes have no twin");
        assert!(!fallback.gpu_measurable("gpu.shadow"));
        let real = mk(GpuTiming::Timestamps);
        assert!(real.gpu_measurable("gpu.cloud_screen"), "timestamps measure every pass");
        assert!(gpu_id_has_cpu_twin("gpu.screen_sky"), "cpu.screen_sky was added 2026-09-18");
        assert!(!gpu_id_has_cpu_twin("gpu.ui"));
    }

    /// Several passes can share one id in a single frame (the scene pass runs
    /// twice; bloom is four passes). Their durations must SUM, not average.
    #[test]
    fn gpu_passes_sharing_an_id_sum_within_a_frame() {
        let _g = serial();
        reset_for_test();
        publish_gpu_frame(&[("gpu.scene", 2.0), ("gpu.scene", 3.0), ("gpu.ssao", 1.0)]);
        let snap = snapshot();
        assert_eq!(snap.value("gpu.scene"), 5.0, "two scene passes must add up");
        assert_eq!(snap.value("gpu.ssao"), 1.0);
        reset_for_test();
    }

    /// A pass that stops running (SSAO turned off, god rays skipped at night)
    /// must decay to zero instead of leaving its last cost in the pie forever.
    #[test]
    fn gpu_passes_that_stop_running_decay_toward_zero() {
        let _g = serial();
        reset_for_test();
        publish_gpu_frame(&[("gpu.godrays", 4.0)]);
        let first = snapshot().value("gpu.godrays");
        assert!(first > 0.0);
        for _ in 0..60 {
            publish_gpu_frame(&[("gpu.celestial", 4.0)]);
        }
        let later = snapshot().value("gpu.godrays");
        assert!(later < first * 0.2, "a stopped pass did not decay: {first} -> {later}");
        reset_for_test();
    }

    /// Runtime names become stable, readable, reusable ids - the join between
    /// the ECS system list and the budget registry.
    #[test]
    fn interned_ids_are_stable_and_slugged() {
        assert_eq!(intern_id("cpu.system.", "FarmingSystem"), "cpu.system.farming");
        assert_eq!(intern_id("cpu.system.", "AISystem"), "cpu.system.ai");
        assert_eq!(intern_id("cpu.system.", "Interaction"), "cpu.system.interaction");
        assert_eq!(
            intern_id("cpu.system.", "Container Compatibility"),
            "cpu.system.container_compatibility"
        );
        // "System" alone must not slug away to nothing.
        assert_eq!(intern_id("cpu.system.", "System"), "cpu.system.system");
        // Same name in, same pointer out (the interner, not a fresh leak).
        let a = intern_id("cpu.system.", "WeatherSystem");
        let b = intern_id("cpu.system.", "weather");
        assert!(std::ptr::eq(a, b), "interning must reuse one allocation per id");
    }

    /// The watch flag gates every expensive sample; it must start cold.
    #[test]
    fn watch_flag_round_trips() {
        watch();
        assert!(watched(), "watch() should mark the store as observed");
    }
}
