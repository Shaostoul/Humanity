//! What this computer has, for Settings > AI's plain notices (docs/design/embeddable-ai.md, 4.7, and
//! CLAUDE.md's hardware-notice rule, 2026-11-08: "We should make sure that we clearly notice and inform
//! users when they have a GPU or whatever that can't run something we're incorporating.").
//!
//! [`read`] asks the operating system each time it is called: total and free memory, processor cores
//! and threads, the processor features AI runtimes use (AVX2 above all), free space where the AI files
//! would go, and the graphics card the renderer recorded. Nothing is kept between calls (so nothing can
//! go stale, CLAUDE.md's no-caches rule); each read takes microseconds, and only Settings > AI and the
//! Dev page ask. Nothing read here leaves the computer.
//!
//! [`check`] compares one choice's needs (data/ai/choices.ron) with a reading and returns the notices
//! 4.7 lists, as data the page words; [`pretend_free_mb`] is the Dev page's "pretend this PC has {n} GB
//! free" (12.1), so every memory notice can be seen on a computer that has plenty.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::choices::Choice;

/// A graphics card, as the renderer recorded it at its start.
#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub name: String,
    /// It shares the computer's main memory (no memory of its own to measure against).
    pub integrated: bool,
    pub vram_mb: Option<u64>,
}

/// One reading of this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct Hardware {
    pub memory_total_mb: Option<u64>,
    /// Free memory now (or the Dev page's pretend figure, `pretending` says which).
    pub memory_free_mb: Option<u64>,
    pub pretending: bool,
    /// Physical cores, when the system says.
    pub cores: Option<u32>,
    /// Logical processors (threads).
    pub threads: u32,
    /// AVX2: Some(true or false) on an x86 processor, None on another kind (an ARM processor has its own
    /// fast instructions, which the runtimes use).
    pub avx2: Option<bool>,
    pub avx512: bool,
    pub gpu: Option<Gpu>,
    /// Free space in `folder`, the folder AI files would go to (or the nearest one that exists).
    pub disk_free_mb: Option<u64>,
    pub folder: Option<PathBuf>,
}

/// The Dev page's pretend free memory in MB; 0 is off.
static PRETEND_FREE_MB: AtomicU64 = AtomicU64::new(0);

/// Pretend this computer has `mb` free (None: tell the truth again). For this run only.
pub fn set_pretend_free_mb(mb: Option<u64>) {
    PRETEND_FREE_MB.store(mb.unwrap_or(0), Ordering::Relaxed);
}

/// The pretend figure, when the Dev page set one.
pub fn pretend_free_mb() -> Option<u64> {
    Some(PRETEND_FREE_MB.load(Ordering::Relaxed)).filter(|mb| *mb > 0)
}

/// Read this computer now.
pub fn read() -> Hardware {
    let (memory_total_mb, real_free) = memory();
    let pretend = pretend_free_mb();
    let folder = super::paths::ai_root().map(|p| nearest_existing(&p));
    let disk_free_mb = folder.as_deref().and_then(disk_free_mb);
    let gpu = crate::renderer::bounced_light_detail::adapter_memory().map(|a| Gpu {
        name: a.name.clone(),
        integrated: a.integrated,
        vram_mb: a.dedicated_bytes.map(|b| b / (1024 * 1024)),
    });
    #[cfg(target_arch = "x86_64")]
    let (avx2, avx512) = (Some(std::arch::is_x86_feature_detected!("avx2")), std::arch::is_x86_feature_detected!("avx512f"));
    #[cfg(not(target_arch = "x86_64"))]
    let (avx2, avx512) = (None, false);
    Hardware {
        memory_total_mb,
        memory_free_mb: pretend.or(real_free),
        pretending: pretend.is_some(),
        cores: physical_cores(),
        threads: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1),
        avx2,
        avx512,
        gpu,
        disk_free_mb,
        folder,
    }
}

/// The folder itself if it exists, else its nearest parent that does (free space is a property of the
/// drive, and the `ai` folder is not made until something is installed).
fn nearest_existing(p: &Path) -> PathBuf {
    let mut here = p;
    while !here.is_dir() {
        match here.parent() {
            Some(up) => here = up,
            None => break,
        }
    }
    here.to_path_buf()
}

#[cfg(windows)]
fn memory() -> (Option<u64>, Option<u64>) {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: `m` is ours, its length set as the call requires.
    match unsafe { GlobalMemoryStatusEx(&mut m) } {
        Ok(()) => (Some(m.ullTotalPhys / (1024 * 1024)), Some(m.ullAvailPhys / (1024 * 1024))),
        Err(_) => (None, None),
    }
}

#[cfg(target_os = "linux")]
fn memory() -> (Option<u64>, Option<u64>) {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb = |name: &str| text.lines().find(|l| l.starts_with(name)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).map(|k| k / 1024);
    (kb("MemTotal:"), kb("MemAvailable:"))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn memory() -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[cfg(windows)]
fn physical_cores() -> Option<u32> {
    use windows::Win32::System::SystemInformation::{GetLogicalProcessorInformation, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION};
    let mut bytes = 0u32;
    // SAFETY: a first call with no buffer asks for the size it needs (it fails, as documented).
    let _ = unsafe { GetLogicalProcessorInformation(None, &mut bytes) };
    let each = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
    if bytes == 0 {
        return None;
    }
    let mut buf = vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); (bytes as usize).div_ceil(each)];
    // SAFETY: the buffer holds at least `bytes` bytes of the entries the call writes.
    unsafe { GetLogicalProcessorInformation(Some(buf.as_mut_ptr()), &mut bytes) }.ok()?;
    let n = (bytes as usize / each).min(buf.len());
    Some(buf[..n].iter().filter(|e| e.Relationship == RelationProcessorCore).count() as u32).filter(|c| *c > 0)
}

#[cfg(target_os = "linux")]
fn physical_cores() -> Option<u32> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let mut seen = std::collections::BTreeSet::new();
    let (mut package, mut core) = (None, None);
    for line in text.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if let (Some(p), Some(c)) = (package.take(), core.take()) {
                seen.insert((p, c));
            }
            continue;
        }
        let mut kv = line.splitn(2, ':').map(str::trim);
        match (kv.next(), kv.next()) {
            (Some("physical id"), Some(v)) => package = Some(v.to_string()),
            (Some("core id"), Some(v)) => core = Some(v.to_string()),
            _ => {}
        }
    }
    Some(seen.len() as u32).filter(|c| *c > 0)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn physical_cores() -> Option<u32> {
    None
}

#[cfg(windows)]
fn disk_free_mb(folder: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = folder.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free = 0u64;
    // SAFETY: a NUL-terminated wide path that lives across the call; `free` is ours.
    unsafe { GetDiskFreeSpaceExW(windows::core::PCWSTR(wide.as_ptr()), Some(&mut free), None, None) }.ok()?;
    Some(free / (1024 * 1024))
}

#[cfg(not(windows))]
fn disk_free_mb(_folder: &Path) -> Option<u64> {
    // Rung 2, with the downloads, reads it on the other systems (statvfs); rung 1 only shows it.
    None
}

/// One of section 4.7's situations, for one choice on one reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// Everything it needs is here.
    Fits,
    /// Too little free memory while playing.
    Memory { need_mb: u64, free_mb: u64, total_mb: Option<u64> },
    /// Fewer processor cores than it wants: it would slow the game.
    Cores { need: u32, have: u32 },
    /// An x86 processor without AVX2: it works, slowly.
    NoAvx2,
    /// Not enough free space for its files.
    Disk { need_mb: u64, free_mb: u64, folder: PathBuf },
    /// The helper's graphics-card route outside the 3D world: whether this card holds the model.
    Gpu { name: String, vram_mb: u64, holds: bool },
}

impl Notice {
    /// A notice that something is short (everything but Fits and a card that holds it).
    pub fn is_warning(&self) -> bool {
        !matches!(self, Notice::Fits | Notice::Gpu { holds: true, .. })
    }
}

/// What a choice needs against what this computer has: every shortfall, in 4.7's order, then the
/// graphics-card line for a choice with that route; `[Fits]` first when nothing is short. A figure the
/// system did not report is never counted as short (the page says it is unknown).
pub fn check(choice: &Choice, hw: &Hardware) -> Vec<Notice> {
    let mut out = Vec::new();
    let n = &choice.needs;
    if let Some(free) = hw.memory_free_mb {
        if free < n.memory_mb {
            out.push(Notice::Memory { need_mb: n.memory_mb, free_mb: free, total_mb: hw.memory_total_mb });
        }
    }
    let cores = hw.cores.unwrap_or(hw.threads);
    if cores < n.cores {
        out.push(Notice::Cores { need: n.cores, have: cores });
    }
    if n.avx2 && hw.avx2 == Some(false) {
        out.push(Notice::NoAvx2);
    }
    if let (Some(free), Some(folder)) = (hw.disk_free_mb, &hw.folder) {
        if free < n.disk_mb {
            out.push(Notice::Disk { need_mb: n.disk_mb, free_mb: free, folder: folder.clone() });
        }
    }
    if out.is_empty() {
        out.push(Notice::Fits);
    }
    if let (Some(need), Some(gpu)) = (n.vram_mb, &hw.gpu) {
        if let (false, Some(vram)) = (gpu.integrated, gpu.vram_mb) {
            out.push(Notice::Gpu { name: gpu.name.clone(), vram_mb: vram, holds: vram >= need });
        }
    }
    out
}

/// Megabytes as gigabytes with one decimal, for the words ("2.9").
pub fn gb(mb: u64) -> String {
    format!("{:.1}", mb as f64 / 1024.0)
}
