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
//!   * Nothing here is feature-gated: `renderer` compiles in the relay build
//!     too, so this module stays std + wgpu only (no GUI, no `engine::`).
//!   * Everything is cheap when nobody is watching: the page calls `watch()`
//!     each frame it draws, and the expensive inventory sampling (allocator
//!     report, working-set syscall, mesh buffer walk) only runs when that
//!     watch is fresh.

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
}

impl Snapshot {
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
        s.last_frame_start = Some(now);
        s.has_frames = true;
        let accum = std::mem::take(&mut s.cpu_accum);
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
    }
}

/// Whether `HUMANITY_FRAME_COSTS` asked for the JSON drop (read once).
fn dump_enabled() -> bool {
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
    let s = snapshot();
    let col = |list: &[Entry]| -> serde_json::Value {
        serde_json::Value::Object(
            list.iter()
                .map(|e| (e.id.to_string(), serde_json::json!(e.value)))
                .collect(),
        )
    };
    let body = serde_json::json!({
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
    });
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write("debug/frame_costs.json", body.to_string());
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

// ────────────────────────────── GPU timestamps ──────────────────────────────

/// How many render passes can be timed per frame. Each takes two timestamp
/// slots (begin + end). 32 is far above the ~12 passes the engine submits.
const MAX_TIMED_PASSES: usize = 32;

/// wgpu timestamp-query ring. One query set is enough because we resolve the
/// PREVIOUS frame's writes at the start of the next frame and read the result
/// asynchronously: at no point does the CPU wait on the GPU.
pub struct GpuTimers {
    query_set: wgpu::QuerySet,
    /// Destination of `resolve_query_set` (GPU-side, not mappable).
    resolve_buf: wgpu::Buffer,
    /// Mappable copy the CPU reads from.
    read_buf: wgpu::Buffer,
    /// Nanoseconds per timestamp tick, from `Queue::get_timestamp_period`.
    period_ns: f32,
    state: Mutex<TimerState>,
    /// Set by the map callback when `read_buf` is ready to read.
    ready: Arc<AtomicBool>,
}

#[derive(Default)]
struct TimerState {
    /// Passes timed during the frame in progress: (id, begin slot index).
    pending: Vec<(&'static str, u32)>,
    /// Passes whose timestamps are resolving on the GPU right now.
    in_flight: Vec<(&'static str, u32)>,
    /// Next free slot index in the query set.
    next: u32,
    /// Whether `read_buf` is currently mapped by us.
    mapped: bool,
}

impl GpuTimers {
    /// Build the query set, or `None` when the device was created without
    /// `TIMESTAMP_QUERY` (the caller then reports the CPU fallback).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let count = (MAX_TIMED_PASSES * 2) as u32;
        let bytes = count as u64 * 8;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Frame Cost Timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame Cost Query Resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame Cost Query Readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve_buf,
            read_buf,
            period_ns: queue.get_timestamp_period(),
            state: Mutex::new(TimerState::default()),
            ready: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Timestamp writes to hand a render pass descriptor. Returns `None` once
    /// the frame's slots are exhausted (that pass is simply untimed).
    pub fn writes(&self, id: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let mut st = self.state.lock().ok()?;
        if st.next as usize + 2 > MAX_TIMED_PASSES * 2 {
            return None;
        }
        let begin = st.next;
        st.next += 2;
        st.pending.push((id, begin));
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }

    /// Frame boundary: publish whatever finished, then kick off the resolve of
    /// the frame that just ended. Never blocks - `Maintain::Poll` only drains
    /// callbacks that are already complete.
    pub fn begin_frame(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let _ = device.poll(wgpu::Maintain::Poll);
        let mut st = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        // 1. Harvest a completed readback into the store.
        if self.ready.swap(false, Ordering::Acquire) {
            let mut samples: Vec<(&'static str, f32)> = Vec::new();
            {
                let view = self.read_buf.slice(..).get_mapped_range();
                // Read each 8-byte tick by hand rather than casting the slice:
                // a mapped range carries no alignment guarantee, and a
                // bytemuck cast would panic on an unaligned one.
                let tick = |i: usize| -> Option<u64> {
                    let o = i * 8;
                    view.get(o..o + 8)
                        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                };
                for (id, begin) in st.in_flight.iter() {
                    let (b, e) = (*begin as usize, *begin as usize + 1);
                    if let (Some(t0), Some(t1)) = (tick(b), tick(e)) {
                        if t1 >= t0 {
                            let ns = (t1 - t0) as f64 * self.period_ns as f64;
                            samples.push((id, (ns / 1.0e6) as f32));
                        }
                    }
                }
            }
            self.read_buf.unmap();
            st.mapped = false;
            st.in_flight.clear();
            // Published as ONE frame so same-id passes sum and absent passes
            // decay (see `publish_gpu_frame`).
            publish_gpu_frame(&samples);
        }

        // 2. Resolve the frame that just ended, if the previous readback is done.
        let pending = std::mem::take(&mut st.pending);
        let used = st.next;
        st.next = 0;
        if st.in_flight.is_empty() && !st.mapped && !pending.is_empty() && used > 0 {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Cost Resolve"),
            });
            enc.resolve_query_set(&self.query_set, 0..used, &self.resolve_buf, 0);
            enc.copy_buffer_to_buffer(
                &self.resolve_buf,
                0,
                &self.read_buf,
                0,
                used as u64 * 8,
            );
            queue.submit(std::iter::once(enc.finish()));
            let flag = self.ready.clone();
            self.read_buf.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });
            st.mapped = true;
            st.in_flight = pending;
        }
    }
}

/// Bytes a texture occupies, mip levels and array layers included. Good enough
/// for an inventory: it is the logical size, not the driver's padded footprint,
/// which is exactly why the pie also shows the allocator's untracked remainder.
pub fn texture_bytes(tex: &wgpu::Texture) -> u64 {
    let size = tex.size();
    let block = tex.format().block_copy_size(None).unwrap_or(4) as u64;
    let (bw, bh) = tex.format().block_dimensions();
    let mut total = 0u64;
    for mip in 0..tex.mip_level_count() {
        let w = (size.width >> mip).max(1) as u64;
        let h = (size.height >> mip).max(1) as u64;
        let d = (size.depth_or_array_layers >> mip).max(1) as u64;
        let blocks_w = w.div_ceil(bw as u64);
        let blocks_h = h.div_ceil(bh as u64);
        total += blocks_w * blocks_h * d * block;
    }
    total
}

// ───────────────────────── Renderer-side plumbing ─────────────────────────
// A child module can see its parent's private items, so the inventory walk
// lives HERE (reading `Renderer`'s private buffers directly) instead of adding
// a hundred lines to `renderer/mod.rs`, which sits under a file-size ratchet.

use super::Renderer;

impl Renderer {
    /// Timestamp writes for one render pass, or `None` when the adapter has no
    /// timestamp queries. Call sites read:
    ///     timestamp_writes: self.pass_timer("gpu.celestial"),
    pub(crate) fn pass_timer(
        &self,
        id: &'static str,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.gpu_timers.as_ref().and_then(|t| t.writes(id))
    }

    /// Frame boundary. Called from the two surface-acquire entry points, which
    /// every frame passes through exactly once: closes the CPU accumulation,
    /// resolves last frame's GPU timestamps, and (only while the Performance
    /// page is open, at most once a second) walks the memory inventory.
    ///
    /// `world` is false for the UI-only path (a page is open, nothing but a
    /// surface clear happens); those frames are discarded rather than published,
    /// so opening the Performance page does not decay the numbers you opened it
    /// to read.
    pub(crate) fn frame_costs_begin(&self, world: bool) {
        if world {
            begin_frame();
        } else {
            discard_frame();
        }
        if let Some(t) = self.gpu_timers.as_ref() {
            t.begin_frame(&self.device, &self.queue);
        }
        // Dev rig hook: `HUMANITY_FRAME_COSTS=1` makes a headless probe run
        // behave as if the Performance page were open and drop the numbers to
        // `debug/frame_costs.json` once a second - the same machine-readable
        // JSON-drop convention as `debug/boot_timing.json`, so a scripted or AI
        // session can READ a real breakdown instead of eyeballing a screenshot.
        if dump_enabled() {
            watch();
        }
        if !watched() {
            return;
        }
        let due = match self.inventory_sampled.lock() {
            Ok(mut last) => {
                let due = last.map_or(true, |t: Instant| t.elapsed().as_secs_f32() > 1.0);
                if due {
                    *last = Some(Instant::now());
                }
                due
            }
            Err(_) => false,
        };
        if due {
            self.sample_memory_inventory();
            if dump_enabled() {
                dump_json();
            }
        }
    }

    /// Tracked GPU allocations, by bucket, plus the driver's own total so the
    /// untracked remainder is visible. Runs at most once a second and only
    /// while somebody is looking at the numbers.
    fn sample_memory_inventory(&self) {
        // Terrain patch arena: the mega-buffers are allocated at full size, so
        // report both what is RESERVED and what is actually in use.
        let (arena_used, arena_reserved) = match self.patch_arena.as_ref() {
            Some(a) => {
                let vstride = std::mem::size_of::<super::mesh::Vertex>() as u64;
                let vused = (a.valloc.cap() - a.valloc.free_total()) as u64 * vstride;
                let iused = (a.ialloc.cap() - a.ialloc.free_total()) as u64 * 4;
                let reserved = a.vertex_buf.size()
                    + a.index_buf.size()
                    + a.instance_buf.size()
                    + a.indirect_buf.size()
                    + a.batch_buf.size();
                (vused + iused, reserved)
            }
            None => (0, 0),
        };
        set_vram("vram.patch_arena", arena_used);
        set_vram("vram.patch_arena_reserved", arena_reserved);

        // Classic meshes (homestead geometry, trees, props, unbatched patches).
        let mesh_bytes: u64 = self
            .meshes
            .iter()
            .map(|m| m.vertex_buffer.size() + m.index_buffer.size())
            .sum();
        set_vram("vram.meshes", mesh_bytes);

        // Engine-global textures and render targets.
        let targets = texture_bytes(&self.scene_texture)
            + texture_bytes(&self.depth_texture)
            + texture_bytes(&self.tree_atlas_texture)
            + texture_bytes(&self.water_fft_texture)
            + texture_bytes(&self.weather_map_tex)
            + texture_bytes(&self.atmo_trans_tex)
            + texture_bytes(&self.atmo_ms_tex);
        set_vram("vram.render_targets", targets);

        // Particles: the two billboard vertex pools plus the GPU-simulated
        // pool's draw buffer (its private state buffer lands in the untracked
        // remainder, which is exactly what that slice is for).
        let particles = self.particle_vb_alpha.as_ref().map_or(0, |b| b.size())
            + self.particle_vb_additive.as_ref().map_or(0, |b| b.size())
            + self.gpu_particles.as_ref().map_or(0, |p| p.vertex_buf.size());
        set_vram("vram.particles", particles);

        // Grass strand instances (near-field vegetation).
        let grass = self.grass_instance_buf.as_ref().map_or(0, |b| b.size())
            + self.grass_mesh.as_ref().map_or(0, |m| {
                m.vertex_buffer.size() + m.index_buffer.size()
            });
        set_vram("vram.grass", grass);

        // Per-frame uniform and light buffers.
        let uniforms = self.camera_buffer.size()
            + self.object_buffer.size()
            + self.lights_buffer.size()
            + self.tile_counts_buffer.size()
            + self.tile_indices_buffer.size()
            + self.light_camera_buffer.size()
            + self.shadow_uniform_buffer.size()
            + self.particle_frame_buffer.size()
            + self.dummy_instance_buf.size();
        set_vram("vram.uniforms", uniforms);

        // What the driver-side allocator says it holds overall. Not every
        // backend reports one; when it does, the Performance page shows the
        // difference from our tracked buckets as "untracked / driver".
        if let Some(rep) = self.device.generate_allocator_report() {
            set_vram("vram.driver_reserved", rep.total_reserved_bytes);
            set_vram("vram.driver_allocated", rep.total_allocated_bytes);
        }

        sample_process_memory();
    }
}

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

    /// REAL GPU verification of the timestamp path, headless (no window, no
    /// world): create a device with TIMESTAMP_QUERY, run three frames of a
    /// trivial render pass through `writes()`, and prove a duration comes back.
    ///
    /// This is the part `cargo check` cannot cover - query-set creation, the
    /// `timestamp_writes` descriptor, `resolve_query_set`, the readback map and
    /// the tick-to-millisecond maths are all runtime wgpu validation, which is
    /// exactly the failure class that shipped ten unbootable releases in
    /// v0.1029-v0.1038. Ignored by default because it needs a GPU adapter:
    ///   cargo test --features native --lib gpu_timestamps -- --ignored --nocapture
    #[test]
    #[ignore = "needs a GPU adapter; run explicitly (see the doc comment)"]
    fn gpu_timestamps_resolve_a_real_pass_duration() {
        let _g = serial();
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Some(a) => a,
            None => {
                println!("no GPU adapter; skipping");
                return;
            }
        };
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            println!("adapter has no TIMESTAMP_QUERY; the CPU fallback is the live path here");
            return;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("frame_costs timestamp test"),
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .expect("device with timestamp queries");
        let timers = GpuTimers::new(&device, &queue).expect("timers on a capable adapter");

        // A BIG clear, on purpose: an empty 256x256 pass finishes inside one
        // timestamp tick on this hardware and reads back as 0 ms, which proves
        // nothing. 4096x4096 is ~67 MB of writes, comfortably measurable.
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("timestamp test target"),
            size: wgpu::Extent3d { width: 4096, height: 4096, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        reset_for_test();
        for _ in 0..4 {
            timers.begin_frame(&device, &queue);
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("timestamp test pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: timers.writes("gpu.test_pass"),
                    occlusion_query_set: None,
                });
            }
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::Maintain::Wait);
        }
        // One more boundary so the last frame's readback is harvested.
        timers.begin_frame(&device, &queue);
        let _ = device.poll(wgpu::Maintain::Wait);
        timers.begin_frame(&device, &queue);

        let snap = snapshot();
        let recorded = snap.gpu.iter().any(|e| e.id == "gpu.test_pass");
        let v = snap.value("gpu.test_pass");
        println!("measured 4096x4096 clear pass: {v:.4} ms");
        // The load-bearing assertion is that the WHOLE path ran: query set,
        // pass timestamp_writes, resolve, readback map, tick maths.
        assert!(
            recorded,
            "no GPU duration was ever recorded - the timestamp path did not \
             resolve (query set, timestamp_writes, resolve_query_set, or the \
             readback map)"
        );
        assert!(
            v > 0.0 && v < 100.0,
            "expected a plausible GPU pass duration, got {v} ms"
        );
        reset_for_test();
    }
}
