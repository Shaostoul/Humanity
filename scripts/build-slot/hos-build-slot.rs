//! hos-build-slot: one memory budget for the whole machine, for HumanityOS's own crate.
//!
//! WHY (2026-10-22, the build-speed review; operator: "Less waste = more productivity").
//! Memory, not CPU, limits this PC. One compile of our crate (humanity_engine, about 600k
//! lines in one crate) peaks near 10 GB of private memory, and several sessions share the
//! machine, each with its own worktree and its own build. Nothing made them take turns, so
//! they started together and Windows ran out of commit memory: out-of-memory kills were seen
//! 13, 11 and 21 times on 2026-10-20, 21 and 22 (Win32 error 1455, exit 0xC0000409,
//! "memory allocation of N bytes failed", "LLVM ERROR: out of memory", E0786 invalid
//! metadata). Each kill threw away minutes of compiling, and agents wrote six home-made
//! "build when free" waiters of their own.
//!
//! The page file went from 8 GB to 100 GB on 2026-10-23 (commit limit about 132 GB), so
//! commit rarely runs out now. Physical memory is still 32 GB, and six 10 GB compiles in
//! 32 GB do not fail any more, they page: every one of them crawls and so does the
//! operator's PC. So the slot keys on FREE PHYSICAL MEMORY (with commit as the second
//! limit), and the out-of-memory retry stays as the safety net.
//!
//! WHAT IT DOES. Cargo runs this in front of rustc for the workspace's own units only
//! (`[build] rustc-workspace-wrapper` in C:\Humanity\.cargo\config.toml, installed by
//! `just build-slot`), so the 500 dependency crates are never gated. For each compile:
//!   1. ADMIT. It waits until the machine has room for the compile's expected peak plus a
//!      margin, counting what the compiles already admitted may still grow by, and the needs
//!      of compiles that started waiting before it (first come, first served; a smaller one
//!      may pass a bigger one only when both fit). The room is the smaller of the free
//!      physical memory (what a compile can get without paging; no paging is allowed by
//!      default) and the free commit memory (what an allocation can get at all). It logs the
//!      wait and gives up waiting after a ceiling, compiling anyway with a loud line.
//!   2. RUN rustc, passing its output straight through to cargo line by line (cargo needs
//!      the artifact lines as they come). Error diagnostics are held back until rustc exits.
//!   3. RETRY. When rustc died of memory (an out-of-memory signature in its output, or an
//!      exit code Windows uses for it), it waits for more room and compiles again, up to
//!      three attempts, with a loud line each time. The held-back errors of a retried attempt
//!      are dropped, so a compile that succeeded on its second try shows no errors.
//!   4. LEARN. The peak of every successful compile (the whole process tree, the linker
//!      included, read from a Windows job object) is written to a ledger, and the expected
//!      peak of each kind of compile is the largest of its last ten, plus 10%.
//!
//! Admission is decided under a lock file and recorded in files named after the process,
//! checked by process id AND start time (Windows reuses ids), the pattern of
//! scripts/lib/boot-lock.js (std's File::lock is newer than some toolchains this may be built with).
//! Everything lives in %LOCALAPPDATA%\HumanityOS\build-slot\ (HOS_BUILD_SLOT_DIR names
//! another folder; the tests use their own).
//!
//! Turn it off for one command: HUMANITY_BUILD_SLOT=off (it then only runs rustc).
//!
//! Other commands (no rustc path first):
//!   hos-build-slot --status   who holds room, who waits, the memory and the budget
//!   hos-build-slot --mem      the memory figures, key=value (scripts read these)
//!   hos-build-slot --source   the SHA-256 of the source this exe was built from
//!   hos-build-slot --measure <program> <args...>
//!                             run a whole command (a cargo build) and print its wall time,
//!                             CPU seconds and peak memory of its whole process tree: the A/B
//!                             measure on a machine other builds share
//!
//! Environment (all optional):
//!   HUMANITY_BUILD_SLOT=off          no gating at all
//!   HOS_BUILD_SLOT_DIR=<dir>         the slot's folder
//!   HOS_BUILD_SLOT_MARGIN_MB=2048    room kept free beyond the expected peak
//!   HOS_BUILD_SLOT_PAGING_MB=0       how far past free physical memory a compile may page
//!   HOS_BUILD_SLOT_MAX_WAIT_MIN=60   the ceiling on one wait
//!   HOS_BUILD_SLOT_POLL_MS=2000      how often a waiting compile looks again
//!   HOS_BUILD_SLOT_ATTEMPTS=3        attempts per compile (1 = never retry)
//!   HOS_BUILD_SLOT_PEAK_MB=<n>       use this expected peak for every unit (tests)
//!   HOS_BUILD_SLOT_FAKE_MEM_FILE=<f> read "avail_commit_mb avail_phys_mb" from a file (tests)
//!   HOS_BUILD_SLOT_TEST_OOM_MB=<n>   cap the FIRST attempt's process tree at n MB (job
//!                                    object), so a real rustc really runs out of memory:
//!                                    the retry proof
//!
//! Built standalone with plain rustc, std only, like the lints (scripts/build-slot.js).
//! Its unit tests: rustc --test scripts/build-slot/hos-build-slot.rs (scripts/tests/build-slot.test.js).

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SRC_SHA256: &str = match option_env!("HOS_BUILD_SLOT_SRC_SHA256") {
    Some(s) => s,
    None => "unknown",
};

const MB: u64 = 1024 * 1024;

// ── what kind of compile this is ────────────────────────────────────────────────

/// One compile cargo asked for: the crate and a kind key the peaks are learned under.
#[derive(Debug, Clone, PartialEq)]
struct Unit {
    crate_name: String,
    /// e.g. "lib-test-native", "lib-check-relay", "bin-release-native", "other-build-native"
    kind: String,
}

/// Which compile the arguments describe, or None for a call that is not a compile (cargo's
/// `-vV` and `--print` probes, the `___` target-info probe): those pass straight through.
fn classify(args: &[String]) -> Option<Unit> {
    let mut crate_name: Option<String> = None;
    let mut emit = String::new();
    let mut test = false;
    let mut opt = String::from("0");
    let mut native = false;
    let mut relay = false;
    let mut print = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = args.get(i + 1).map(|s| s.as_str());
        match a {
            "--crate-name" => {
                crate_name = next.map(String::from);
                i += 2;
                continue;
            }
            "--emit" => {
                if let Some(v) = next {
                    emit = v.to_string();
                }
                i += 2;
                continue;
            }
            "--test" => test = true,
            "-C" => {
                if let Some(v) = next {
                    if let Some(o) = v.strip_prefix("opt-level=") {
                        opt = o.to_string();
                    }
                }
                i += 2;
                continue;
            }
            "--cfg" => {
                match next {
                    Some("feature=\"native\"") => native = true,
                    Some("feature=\"relay\"") => relay = true,
                    _ => {}
                }
                i += 2;
                continue;
            }
            _ => {
                if let Some(v) = a.strip_prefix("--emit=") {
                    emit = v.to_string();
                } else if let Some(v) = a.strip_prefix("-Copt-level=") {
                    opt = v.to_string();
                } else if a.starts_with("--print") {
                    print = true;
                }
            }
        }
        i += 1;
    }
    if print {
        return None;
    }
    let name = crate_name?;
    if name == "___" {
        return None;
    }
    let check = !emit.is_empty() && !emit.split(',').any(|e| e == "link");
    let mode = if check {
        "check"
    } else if test {
        "test"
    } else if opt != "0" {
        "release"
    } else {
        "build"
    };
    let target = match name.as_str() {
        "humanity_engine" => "lib",
        "HumanityOS" => "bin",
        "build_script_build" => "buildscript",
        _ => "other",
    };
    let features = if native {
        "native"
    } else if relay {
        "relay"
    } else {
        "none"
    };
    Some(Unit { crate_name: name.clone(), kind: format!("{target}-{mode}-{features}") })
}

/// The arguments as text for classify(), with every `@file` replaced by the file's lines: cargo
/// hands rustc an argument file when the command line would be too long for Windows, and a
/// compile classified without it would pass ungated. The arguments rustc is started with are
/// the original ones, untouched (see main).
fn text_args(args: &[OsString]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let t = a.to_string_lossy().to_string();
        if let Some(file) = t.strip_prefix('@') {
            if let Ok(body) = fs::read(file) {
                out.extend(String::from_utf8_lossy(&body).lines().map(String::from));
                continue;
            }
        }
        out.push(t);
    }
    out
}

/// The expected peak, in MB, of a kind of compile nobody has measured on this machine yet.
/// Measured 2026-10-22 on the dev PC (32 GB RAM, Rust 1.88, debuginfo as Cargo.toml set it
/// that day); scripts/build-slot.js --status shows what the ledger has learned since.
fn seed_peak_mb(kind: &str) -> u64 {
    let mut parts = kind.split('-');
    let target = parts.next().unwrap_or("");
    let mode = parts.next().unwrap_or("");
    let features = parts.next().unwrap_or("");
    let lib = match mode {
        "check" => 6_000,
        "test" => 11_000,
        "release" => 9_000,
        _ => 10_000,
    };
    match target {
        "lib" => {
            if features == "native" {
                lib
            } else {
                lib * 6 / 10
            }
        }
        // the thin main.rs, but its link of the whole native exe can take gigabytes
        "bin" => 3_000,
        "buildscript" => 1_000,
        _ => 3_000,
    }
}

// ── output: what rustc says, what is held back, what means "out of memory" ─────────

/// An out-of-memory signature in one line of rustc's output, named for the log.
fn oom_line_signature(line: &str) -> Option<&'static str> {
    let l = line.to_ascii_lowercase();
    if l.contains("memory allocation of") && l.contains("failed") {
        return Some("\"memory allocation of N bytes failed\"");
    }
    if l.contains("llvm error: out of memory") {
        return Some("\"LLVM ERROR: out of memory\"");
    }
    if l.contains("os error 1455") || l.contains("paging file is too small") {
        return Some("Win32 error 1455 (the paging file is too small)");
    }
    if l.contains("os error 1450") {
        return Some("Win32 error 1450 (insufficient system resources)");
    }
    if l.contains("not enough memory resources") || l.contains("(os error 8)") || l.contains("(os error 14)") {
        return Some("Win32 error 8/14 (not enough memory)");
    }
    if l.contains("cannot allocate memory") {
        return Some("\"Cannot allocate memory\"");
    }
    if l.contains("e0786") {
        return Some("E0786 (invalid metadata files, which a memory-starved read produces)");
    }
    if l.contains("lnk1102") {
        return Some("LNK1102 (the linker ran out of memory)");
    }
    None
}

/// An exit code Windows gives a process that died of memory.
fn oom_exit_signature(code: Option<i32>) -> Option<&'static str> {
    match code.map(|c| c as u32) {
        Some(0xC000_0409) => Some("exit 0xC0000409 (STATUS_STACK_BUFFER_OVERRUN: how rustc aborts when an allocation fails)"),
        Some(0xC000_0017) => Some("exit 0xC0000017 (STATUS_NO_MEMORY)"),
        Some(0xC000_012D) => Some("exit 0xC000012D (STATUS_COMMITMENT_LIMIT)"),
        _ => None,
    }
}

/// Lines held back until rustc exits: error diagnostics (a retried attempt's errors must not
/// reach cargo) and any out-of-memory line. Everything else goes through at once: cargo
/// starts dependent compiles from rustc's artifact lines as they arrive.
fn hold_back(line: &str) -> bool {
    (line.contains("\"$message_type\":\"diagnostic\"") && line.contains("\"level\":\"error"))
        || oom_line_signature(line).is_some()
}

// ── the admission rule (pure, tested) ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct Mem {
    avail_commit_mb: u64,
    avail_phys_mb: u64,
    commit_limit_mb: u64,
    total_phys_mb: u64,
}

#[derive(Debug, Clone, Copy)]
struct Policy {
    margin_mb: u64,
    paging_mb: u64,
}

/// The rule's two numbers, from the environment or the defaults: 2 GB kept free beyond the
/// expected peak, and no paging. Paging is the waste this exists to stop (see the header).
fn policy() -> Policy {
    Policy { margin_mb: env_u64("HOS_BUILD_SLOT_MARGIN_MB", 2048), paging_mb: env_u64("HOS_BUILD_SLOT_PAGING_MB", 0) }
}

/// The room a new compile can count on: the smaller of free commit memory and free
/// physical memory plus the paging allowance, less what is already promised.
fn room_mb(mem: &Mem, policy: &Policy, promised_mb: u64) -> i64 {
    let physical = mem.avail_phys_mb + policy.paging_mb;
    let base = mem.avail_commit_mb.min(physical);
    base as i64 - promised_mb as i64
}

/// What an admitted compile may still grow by: its expected peak less what it holds now.
fn still_to_grow_mb(expect_mb: u64, current_mb: u64) -> u64 {
    expect_mb.saturating_sub(current_mb)
}

/// May a compile needing `need_mb` start now? `promised_mb` is what admitted compiles may
/// still grow by plus the needs of compiles waiting longer than this one.
fn fits(mem: &Mem, policy: &Policy, promised_mb: u64, need_mb: u64) -> bool {
    room_mb(mem, policy, promised_mb) >= (need_mb + policy.margin_mb) as i64
}

/// The expected peak from the ledger's observations for a kind (newest last): the largest
/// of the last ten plus 10%, never below half the seed. None when there are none.
fn learned_peak_mb(observed: &[u64], seed: u64) -> Option<u64> {
    if observed.is_empty() {
        return None;
    }
    let start = observed.len().saturating_sub(10);
    let max = *observed[start..].iter().max().unwrap();
    Some((max + max / 10).max(seed / 2))
}

// ── records: who holds room, who waits ────────────────────────────────────────────

/// A held-<pid>.txt or wait-<pid>.txt file: key=value lines.
#[derive(Debug, Clone, PartialEq, Default)]
struct Record {
    pid: u32,
    start: u64,
    child_pid: u32,
    kind: String,
    crate_name: String,
    mb: u64,
    since_ms: u64,
    cwd: String,
}

impl Record {
    fn to_text(&self) -> String {
        format!(
            "pid={}\nstart={}\nchild_pid={}\nkind={}\ncrate={}\nmb={}\nsince_ms={}\ncwd={}\n",
            self.pid, self.start, self.child_pid, self.kind, self.crate_name, self.mb, self.since_ms, self.cwd
        )
    }
    fn parse(text: &str) -> Option<Record> {
        let mut r = Record::default();
        for line in text.lines() {
            let (k, v) = match line.split_once('=') {
                Some(x) => x,
                None => continue,
            };
            match k {
                "pid" => r.pid = v.parse().ok()?,
                "start" => r.start = v.parse().ok()?,
                "child_pid" => r.child_pid = v.parse().ok()?,
                "kind" => r.kind = v.to_string(),
                "crate" => r.crate_name = v.to_string(),
                "mb" => r.mb = v.parse().ok()?,
                "since_ms" => r.since_ms = v.parse().ok()?,
                "cwd" => r.cwd = v.to_string(),
                _ => {}
            }
        }
        if r.pid == 0 {
            None
        } else {
            Some(r)
        }
    }
}

// ── Windows ──────────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    pub type Handle = *mut c_void;

    #[repr(C)]
    pub struct MemoryStatusEx {
        pub length: u32,
        pub memory_load: u32,
        pub total_phys: u64,
        pub avail_phys: u64,
        pub total_page_file: u64,
        pub avail_page_file: u64,
        pub total_virtual: u64,
        pub avail_virtual: u64,
        pub avail_extended_virtual: u64,
    }

    #[repr(C)]
    pub struct ProcessMemoryCounters {
        pub cb: u32,
        pub page_fault_count: u32,
        pub peak_working_set_size: usize,
        pub working_set_size: usize,
        pub quota_peak_paged_pool_usage: usize,
        pub quota_paged_pool_usage: usize,
        pub quota_peak_non_paged_pool_usage: usize,
        pub quota_non_paged_pool_usage: usize,
        pub pagefile_usage: usize,
        pub peak_pagefile_usage: usize,
    }

    #[repr(C)]
    pub struct JobBasicLimit {
        pub per_process_user_time_limit: i64,
        pub per_job_user_time_limit: i64,
        pub limit_flags: u32,
        pub minimum_working_set_size: usize,
        pub maximum_working_set_size: usize,
        pub active_process_limit: u32,
        pub affinity: usize,
        pub priority_class: u32,
        pub scheduling_class: u32,
    }

    #[repr(C)]
    pub struct JobExtendedLimit {
        pub basic: JobBasicLimit,
        pub io: [u64; 6],
        pub process_memory_limit: usize,
        pub job_memory_limit: usize,
        pub peak_process_memory_used: usize,
        pub peak_job_memory_used: usize,
    }

    #[repr(C)]
    pub struct JobAccounting {
        pub total_user_time: i64,
        pub total_kernel_time: i64,
        pub this_period_user_time: i64,
        pub this_period_kernel_time: i64,
        pub total_page_fault_count: u32,
        pub total_processes: u32,
        pub active_processes: u32,
        pub total_terminated_processes: u32,
    }

    #[repr(C)]
    pub struct SystemTime {
        pub year: u16,
        pub month: u16,
        pub day_of_week: u16,
        pub day: u16,
        pub hour: u16,
        pub minute: u16,
        pub second: u16,
        pub milliseconds: u16,
    }

    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    pub const PROCESS_VM_READ: u32 = 0x0010;
    pub const STILL_ACTIVE: u32 = 259;
    pub const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    pub const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION: i32 = 1;
    pub const JOB_OBJECT_LIMIT_JOB_MEMORY: u32 = 0x0000_0200;
    pub const ERROR_ACCESS_DENIED: i32 = 5;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        pub fn CloseHandle(h: Handle) -> i32;
        pub fn GetExitCodeProcess(h: Handle, code: *mut u32) -> i32;
        pub fn GetProcessTimes(h: Handle, creation: *mut u64, exit: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
        pub fn K32GetProcessMemoryInfo(h: Handle, counters: *mut ProcessMemoryCounters, cb: u32) -> i32;
        pub fn CreateJobObjectW(attrs: *mut c_void, name: *const u16) -> Handle;
        pub fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        pub fn QueryInformationJobObject(job: Handle, class: i32, info: *mut c_void, len: u32, ret: *mut u32) -> i32;
        pub fn SetInformationJobObject(job: Handle, class: i32, info: *mut c_void, len: u32) -> i32;
        pub fn GetLocalTime(t: *mut SystemTime);
    }
}

/// The machine's memory now (or the test's fake figures).
fn memory() -> Mem {
    if let Ok(f) = env::var("HOS_BUILD_SLOT_FAKE_MEM_FILE") {
        let text = fs::read_to_string(&f).unwrap_or_default();
        let n: Vec<u64> = text.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        let commit = n.first().copied().unwrap_or(0);
        let phys = n.get(1).copied().unwrap_or(commit);
        return Mem { avail_commit_mb: commit, avail_phys_mb: phys, commit_limit_mb: commit, total_phys_mb: phys };
    }
    real_memory()
}

#[cfg(windows)]
fn real_memory() -> Mem {
    let mut s = sys::MemoryStatusEx {
        length: std::mem::size_of::<sys::MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    unsafe { sys::GlobalMemoryStatusEx(&mut s) };
    Mem {
        avail_commit_mb: s.avail_page_file / MB,
        avail_phys_mb: s.avail_phys / MB,
        commit_limit_mb: s.total_page_file / MB,
        total_phys_mb: s.total_phys / MB,
    }
}

#[cfg(not(windows))]
fn real_memory() -> Mem {
    // Not the machine this was built for: no gate (room is "plenty").
    Mem { avail_commit_mb: u64::MAX / 4, avail_phys_mb: u64::MAX / 4, commit_limit_mb: 0, total_phys_mb: 0 }
}

/// A process's start time (Windows FILETIME), or None when it cannot be read.
#[cfg(windows)]
fn start_time(pid: u32) -> Option<u64> {
    unsafe {
        let h = sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
        let ok = sys::GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        sys::CloseHandle(h);
        if ok == 0 {
            None
        } else {
            Some(c)
        }
    }
}
#[cfg(not(windows))]
fn start_time(_pid: u32) -> Option<u64> {
    None
}

/// Is the process a record names still the one that wrote it?
#[cfg(windows)]
fn alive(pid: u32, start: u64) -> bool {
    unsafe {
        let h = sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            // Gone (invalid parameter), or one we may not open (access denied): only the
            // second is alive.
            return io::Error::last_os_error().raw_os_error() == Some(sys::ERROR_ACCESS_DENIED);
        }
        let mut code = 0u32;
        let mut live = sys::GetExitCodeProcess(h, &mut code) != 0 && code == sys::STILL_ACTIVE;
        if live && start != 0 {
            let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
            if sys::GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                live = c == start; // another start time: the pid was reused
            }
        }
        sys::CloseHandle(h);
        live
    }
}
#[cfg(not(windows))]
fn alive(pid: u32, _start: u64) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// A process's private (commit) memory now, MB; 0 when it cannot be read.
#[cfg(windows)]
fn private_mb(pid: u32) -> u64 {
    if pid == 0 {
        return 0;
    }
    unsafe {
        let h = sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION | sys::PROCESS_VM_READ, 0, pid);
        if h.is_null() {
            return 0;
        }
        let mut pmc: sys::ProcessMemoryCounters = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<sys::ProcessMemoryCounters>() as u32;
        let ok = sys::K32GetProcessMemoryInfo(h, &mut pmc, pmc.cb);
        sys::CloseHandle(h);
        if ok == 0 {
            0
        } else {
            pmc.pagefile_usage as u64 / MB
        }
    }
}
#[cfg(not(windows))]
fn private_mb(_pid: u32) -> u64 {
    0
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// "23:58:01", local time, for lines a person reads.
#[cfg(windows)]
fn clock() -> String {
    let mut t = sys::SystemTime { year: 0, month: 0, day_of_week: 0, day: 0, hour: 0, minute: 0, second: 0, milliseconds: 0 };
    unsafe { sys::GetLocalTime(&mut t) };
    format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second)
}
#[cfg(not(windows))]
fn clock() -> String {
    let s = now_ms() / 1000;
    format!("{:02}:{:02}:{:02} UTC", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

fn gb(mb: u64) -> String {
    if mb < 1024 {
        format!("{mb} MB")
    } else {
        format!("{:.1} GB", mb as f64 / 1024.0)
    }
}

// ── the job object: measure the whole tree, and the retry proof's memory cap ────────

#[cfg(windows)]
struct Job(sys::Handle);

#[cfg(windows)]
impl Job {
    /// A new job holding `process` (the rustc just started; the linker it starts later
    /// joins it too). None when Windows refuses (then the peak is rustc's own).
    fn holding(process: sys::Handle, cap_mb: Option<u64>) -> Option<Job> {
        unsafe {
            let h = sys::CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let job = Job(h);
            if let Some(cap) = cap_mb {
                let mut info = job.query()?;
                info.basic.limit_flags |= sys::JOB_OBJECT_LIMIT_JOB_MEMORY;
                info.job_memory_limit = (cap * MB) as usize;
                sys::SetInformationJobObject(
                    h,
                    sys::JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of::<sys::JobExtendedLimit>() as u32,
                );
            }
            if sys::AssignProcessToJobObject(h, process) == 0 {
                return None;
            }
            Some(job)
        }
    }
    fn query(&self) -> Option<sys::JobExtendedLimit> {
        unsafe {
            let mut info: sys::JobExtendedLimit = std::mem::zeroed();
            let ok = sys::QueryInformationJobObject(
                self.0,
                sys::JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<sys::JobExtendedLimit>() as u32,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                None
            } else {
                Some(info)
            }
        }
    }
    fn peak_mb(&self) -> u64 {
        self.query().map(|i| i.peak_job_memory_used as u64 / MB).unwrap_or(0)
    }
    /// CPU seconds the whole tree used (user + kernel), the honest measure of a compile's cost
    /// on a machine that other builds share (wall time measures them too).
    fn cpu_s(&self) -> f64 {
        unsafe {
            let mut a: sys::JobAccounting = std::mem::zeroed();
            let ok = sys::QueryInformationJobObject(
                self.0,
                sys::JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
                &mut a as *mut _ as *mut _,
                std::mem::size_of::<sys::JobAccounting>() as u32,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                0.0
            } else {
                (a.total_user_time + a.total_kernel_time) as f64 / 1e7
            }
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe { sys::CloseHandle(self.0) };
    }
}

/// rustc's own peaks, (private commit MB, working set MB), from its handle (still valid
/// after the wait). The working set is what it actually held in physical memory.
#[cfg(windows)]
fn child_peaks_mb(child: &std::process::Child) -> (u64, u64) {
    use std::os::windows::io::AsRawHandle;
    unsafe {
        let mut pmc: sys::ProcessMemoryCounters = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<sys::ProcessMemoryCounters>() as u32;
        if sys::K32GetProcessMemoryInfo(child.as_raw_handle() as sys::Handle, &mut pmc, pmc.cb) == 0 {
            (0, 0)
        } else {
            (pmc.peak_pagefile_usage as u64 / MB, pmc.peak_working_set_size as u64 / MB)
        }
    }
}

// ── the slot folder ───────────────────────────────────────────────────────────────

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}

fn slot_dir() -> PathBuf {
    if let Ok(d) = env::var("HOS_BUILD_SLOT_DIR") {
        return PathBuf::from(d);
    }
    let base = env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| env::temp_dir());
    base.join("HumanityOS").join("build-slot")
}

struct Slot {
    dir: PathBuf,
    me: u32,
    my_start: u64,
}

impl Slot {
    fn open() -> io::Result<Slot> {
        let dir = slot_dir();
        fs::create_dir_all(&dir)?;
        let me = std::process::id();
        Ok(Slot { dir, me, my_start: start_time(me).unwrap_or(0) })
    }

    fn log(&self, line: &str) {
        let path = self.dir.join("build-slot.log");
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > 2 * MB {
                let _ = fs::rename(&path, self.dir.join("build-slot.old.log"));
            }
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{} {} pid {} {}", now_ms(), clock(), self.me, line);
        }
    }

    /// Write a record atomically (temp file, then rename over).
    fn write_record(&self, name: &str, rec: &Record) -> io::Result<()> {
        let tmp = self.dir.join(format!(".tmp-{}-{}", self.me, name));
        fs::write(&tmp, rec.to_text())?;
        fs::rename(&tmp, self.dir.join(name))
    }

    fn remove(&self, name: &str) {
        let _ = fs::remove_file(self.dir.join(name));
    }

    /// Every live record whose file name starts with `prefix` (stale ones are removed).
    fn live(&self, prefix: &str) -> Vec<Record> {
        let mut out = Vec::new();
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(_) => return out,
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(prefix) || !name.ends_with(".txt") {
                continue;
            }
            let rec = fs::read_to_string(e.path()).ok().and_then(|t| Record::parse(&t));
            match rec {
                Some(r) if alive(r.pid, r.start) => out.push(r),
                Some(_) => {
                    let _ = fs::remove_file(e.path());
                }
                None => {
                    // Unreadable: a writer between create and rename cannot leave one (the
                    // rename is atomic), so this is junk; remove it once it is a minute old.
                    let old = e
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|d| d.as_secs() > 60)
                        .unwrap_or(false);
                    if old {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        out
    }

    /// Take the short-lived admission lock (held for the milliseconds of one decision).
    fn lock(&self) -> AdmitLock {
        let path = self.dir.join("admit.lock");
        let rec = Record { pid: self.me, start: self.my_start, since_ms: now_ms(), ..Default::default() };
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    let _ = f.write_all(rec.to_text().as_bytes());
                    return AdmitLock { path: Some(path) };
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let holder = fs::read_to_string(&path).ok().and_then(|t| Record::parse(&t));
                    let age_s = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let stale = match &holder {
                        Some(h) => !alive(h.pid, h.start) || age_s > 30,
                        None => age_s > 5,
                    };
                    if stale {
                        // Rename first: of two processes that both judged it stale, only
                        // one rename succeeds, so a fresh lock is never deleted.
                        let away = self.dir.join(format!("admit.lock.stale-{}-{}", self.me, now_ms()));
                        if fs::rename(&path, &away).is_ok() {
                            let _ = fs::remove_file(&away);
                        }
                        continue;
                    }
                }
                Err(_) => {}
            }
            if Instant::now() > deadline {
                // Cannot get it in a minute: decide without it rather than hang a build.
                self.log("admission lock not free after 60 s; deciding without it");
                return AdmitLock { path: None };
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// The expected peak for a kind: the ledger's, or the seed.
    fn expected_mb(&self, kind: &str) -> u64 {
        if let Some(p) = env::var("HOS_BUILD_SLOT_PEAK_MB").ok().and_then(|v| v.parse().ok()) {
            return p;
        }
        let seed = seed_peak_mb(kind);
        let text = fs::read_to_string(self.dir.join("peaks.txt")).unwrap_or_default();
        let observed: Vec<u64> = text
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.len() >= 3 && f[1] == kind {
                    f[2].parse().ok()
                } else {
                    None
                }
            })
            .collect();
        learned_peak_mb(&observed, seed).unwrap_or(seed)
    }

    fn record_peak(&self, kind: &str, peak_mb: u64, secs: u64, ws_mb: u64, cpu_s: f64) {
        let path = self.dir.join("peaks.txt");
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            // epoch-seconds kind peak-commit-MB seconds peak-working-set-MB cpu-seconds
            let _ = writeln!(f, "{} {} {} {} {} {:.0}", now_ms() / 1000, kind, peak_mb, secs, ws_mb, cpu_s);
        }
        // Keep it short: the last 2,000 lines.
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > 200_000 {
                if let Ok(text) = fs::read_to_string(&path) {
                    let lines: Vec<&str> = text.lines().collect();
                    let keep = lines[lines.len().saturating_sub(2000)..].join("\n") + "\n";
                    let tmp = self.dir.join(format!(".tmp-{}-peaks", self.me));
                    if fs::write(&tmp, keep).is_ok() {
                        let _ = fs::rename(&tmp, &path);
                    }
                }
            }
        }
    }
}

struct AdmitLock {
    path: Option<PathBuf>,
}
impl Drop for AdmitLock {
    fn drop(&mut self) {
        if let Some(p) = &self.path {
            let _ = fs::remove_file(p);
        }
    }
}

/// The room this compile holds: removes its record when dropped (a hard kill leaves a
/// record whose process is gone, which the next reader removes).
struct Held<'a> {
    slot: &'a Slot,
    rec: Record,
}
impl<'a> Held<'a> {
    fn set_child(&mut self, pid: u32) {
        self.rec.child_pid = pid;
        let _ = self.slot.write_record(&format!("held-{}.txt", self.slot.me), &self.rec);
    }
}
impl<'a> Drop for Held<'a> {
    fn drop(&mut self) {
        self.slot.remove(&format!("held-{}.txt", self.slot.me));
    }
}

/// What admitted compiles may still grow by.
fn promised_by_holders(holders: &[Record], me: u32) -> u64 {
    holders.iter().filter(|h| h.pid != me).map(|h| still_to_grow_mb(h.mb, private_mb(h.child_pid))).sum()
}

/// A line for the person (or agent) watching the build. On STDOUT, deliberately: cargo
/// saves a unit's stderr lines and replays them every later time it finds the unit fresh,
/// so a wait printed on stderr came back on every no-op build as if it were happening
/// again (seen 2026-10-22). Cargo passes rustc's stdout through without saving it.
fn say(line: &str) {
    let mut e = io::stdout().lock();
    let _ = writeln!(e, "[build-slot {}] {}", clock(), line);
    let _ = e.flush();
}

/// Wait until this compile fits, then hold its room. Never fails: past the ceiling it
/// proceeds anyway, saying so.
fn admit<'a>(slot: &'a Slot, unit: &Unit, need_mb: u64, arrived_ms: u64, policy: &Policy) -> Held<'a> {
    let poll = Duration::from_millis(env_u64("HOS_BUILD_SLOT_POLL_MS", 2000).max(50));
    let ceiling = Duration::from_secs(env_u64("HOS_BUILD_SLOT_MAX_WAIT_MIN", 60) * 60);
    let wait_name = format!("wait-{}.txt", slot.me);
    let mine = Record {
        pid: slot.me,
        start: slot.my_start,
        child_pid: 0,
        kind: unit.kind.clone(),
        crate_name: unit.crate_name.clone(),
        mb: need_mb,
        since_ms: arrived_ms,
        cwd: env::current_dir().map(|p| p.display().to_string()).unwrap_or_default(),
    };
    let t0 = Instant::now();
    let mut announced = false;
    let mut last_note = Instant::now();
    loop {
        let decision = {
            let _lock = slot.lock();
            let mem = memory();
            let holders = slot.live("held-");
            let waiters = slot.live("wait-");
            let older: u64 = waiters
                .iter()
                .filter(|w| w.pid != slot.me && (w.since_ms, w.pid) < (arrived_ms, slot.me))
                .map(|w| w.mb)
                .sum();
            let promised = promised_by_holders(&holders, slot.me) + older;
            let over_ceiling = t0.elapsed() >= ceiling;
            if fits(&mem, policy, promised, need_mb) || over_ceiling {
                let held = Record { since_ms: now_ms(), ..mine.clone() };
                let _ = slot.write_record(&format!("held-{}.txt", slot.me), &held);
                slot.remove(&wait_name);
                Ok((held, mem, promised, over_ceiling, holders.len()))
            } else {
                if !announced {
                    let _ = slot.write_record(&wait_name, &mine);
                }
                Err((mem, promised, holders, waiters.len()))
            }
        };
        match decision {
            Ok((rec, mem, promised, over_ceiling, n_holders)) => {
                let waited = t0.elapsed().as_secs();
                if over_ceiling {
                    let msg = format!(
                        "{} ({}): WAITED {} min, the ceiling, and the machine never had room for {} (+{} margin); compiling anyway. Room now {} with {} promised to {} other compile(s). Out of memory here is retried.",
                        unit.crate_name, unit.kind, waited / 60, gb(need_mb), gb(policy.margin_mb), gb(room_mb(&mem, policy, 0).max(0) as u64), gb(promised), n_holders
                    );
                    say(&msg);
                    slot.log(&msg);
                } else if announced {
                    let msg = format!("{} ({}): room for {} after waiting {} s; compiling.", unit.crate_name, unit.kind, gb(need_mb), waited);
                    say(&msg);
                    slot.log(&msg);
                } else {
                    slot.log(&format!(
                        "admit {} ({}) need {} MB; commit free {} MB, physical free {} MB, promised {} MB",
                        unit.crate_name, unit.kind, need_mb, mem.avail_commit_mb, mem.avail_phys_mb, promised
                    ));
                }
                return Held { slot, rec };
            }
            Err((mem, promised, holders, n_waiters)) => {
                if !announced || last_note.elapsed() >= Duration::from_secs(60) {
                    let others: Vec<String> = holders
                        .iter()
                        .filter(|h| h.pid != slot.me)
                        .map(|h| format!("{} ({}, {} of {})", h.crate_name, h.kind, gb(private_mb(h.child_pid)), gb(h.mb)))
                        .collect();
                    let msg = format!(
                        "{} ({}): {} for memory: needs {} + {} margin; commit free {}, physical free {} (+{} paging allowed), {} promised to compiles already running or waiting longer. Running: {}. Waiting: {}. Ceiling {} min; HUMANITY_BUILD_SLOT=off skips the wait.",
                        unit.crate_name,
                        unit.kind,
                        if announced { format!("still waiting ({} s)", t0.elapsed().as_secs()) } else { "WAITING".to_string() },
                        gb(need_mb),
                        gb(policy.margin_mb),
                        gb(mem.avail_commit_mb),
                        gb(mem.avail_phys_mb),
                        gb(policy.paging_mb),
                        gb(promised),
                        if others.is_empty() { "none of ours".to_string() } else { others.join(", ") },
                        n_waiters,
                        ceiling.as_secs() / 60
                    );
                    say(&msg);
                    slot.log(&msg);
                    announced = true;
                    last_note = Instant::now();
                }
                std::thread::sleep(poll);
            }
        }
    }
}

// ── running rustc ────────────────────────────────────────────────────────────────

struct Outcome {
    code: Option<i32>,
    oom: Option<&'static str>,
    held_lines: Vec<String>,
    peak_mb: u64,
    /// rustc's peak working set (physical memory), MB, for the ledger
    ws_mb: u64,
    /// CPU seconds of the whole tree (job accounting), for the ledger
    cpu_s: f64,
    secs: u64,
}

fn run_once(rustc: &OsStr, args: &[OsString], held: &mut Held, cap_mb: Option<u64>) -> io::Result<Outcome> {
    let t0 = Instant::now();
    let mut child = Command::new(rustc).args(args).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::piped()).spawn()?;
    held.set_child(child.id());
    #[cfg(windows)]
    let job = {
        use std::os::windows::io::AsRawHandle;
        Job::holding(child.as_raw_handle() as sys::Handle, cap_mb)
    };
    #[cfg(not(windows))]
    let _ = cap_mb;
    let mut held_lines = Vec::new();
    let mut oom = None;
    {
        let stderr = child.stderr.take().expect("stderr was piped");
        let mut reader = BufReader::new(stderr);
        let mut out = io::stderr();
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    if let Some(sig) = oom_line_signature(&line) {
                        oom.get_or_insert(sig);
                    }
                    if hold_back(&line) {
                        held_lines.push(line.into_owned());
                    } else {
                        let _ = out.write_all(&buf);
                        let _ = out.flush();
                    }
                }
                Err(_) => break,
            }
        }
    }
    let status = child.wait()?;
    let code = status.code();
    if oom.is_none() {
        oom = oom_exit_signature(code);
    }
    #[cfg(windows)]
    let (peak_mb, ws_mb, cpu_s) = {
        let (own, ws) = child_peaks_mb(&child);
        (job.as_ref().map(|j| j.peak_mb()).unwrap_or(0).max(own), ws, job.as_ref().map(|j| j.cpu_s()).unwrap_or(0.0))
    };
    #[cfg(not(windows))]
    let (peak_mb, ws_mb, cpu_s) = (0, 0, 0.0);
    Ok(Outcome { code, oom: if status.success() { None } else { oom }, held_lines, peak_mb, ws_mb, cpu_s, secs: t0.elapsed().as_secs() })
}

/// No gate: run rustc with this process's own output, return its exit code.
fn passthrough(rustc: &OsStr, args: &[OsString]) -> i32 {
    match Command::new(rustc).args(args).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("hos-build-slot: could not start {}: {e}", rustc.to_string_lossy());
            1
        }
    }
}

fn gated(rustc: &OsStr, args: &[OsString], unit: Unit) -> i32 {
    let slot = match Slot::open() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[build-slot] cannot open the slot folder {} ({e}); compiling without it", slot_dir().display());
            return passthrough(rustc, args);
        }
    };
    let policy = policy();
    let attempts = env_u64("HOS_BUILD_SLOT_ATTEMPTS", 3).max(1);
    let test_cap = env::var("HOS_BUILD_SLOT_TEST_OOM_MB").ok().and_then(|v| v.parse::<u64>().ok());
    let arrived = now_ms();
    let mut need = slot.expected_mb(&unit.kind);
    let mut last: Option<Outcome> = None;
    for attempt in 1..=attempts {
        let mut held = admit(&slot, &unit, need, arrived, &policy);
        let cap = if attempt == 1 { test_cap } else { None };
        if let Some(c) = cap {
            say(&format!("{} ({}): TEST: capping this first attempt at {} (HOS_BUILD_SLOT_TEST_OOM_MB), so it runs out of memory", unit.crate_name, unit.kind, gb(c)));
        }
        let out = match run_once(rustc, args, &mut held, cap) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("hos-build-slot: could not start {}: {e}", rustc.to_string_lossy());
                return 1;
            }
        };
        drop(held);
        if out.code == Some(0) {
            if out.peak_mb > 0 && cap.is_none() {
                slot.record_peak(&unit.kind, out.peak_mb, out.secs, out.ws_mb, out.cpu_s);
            }
            slot.log(&format!("done {} ({}) in {} s (cpu {:.0} s), peak {} MB (working set {} MB), attempt {}", unit.crate_name, unit.kind, out.secs, out.cpu_s, out.peak_mb, out.ws_mb, attempt));
            if attempt > 1 {
                say(&format!("{} ({}): attempt {} SUCCEEDED after the out-of-memory retry (peak {}).", unit.crate_name, unit.kind, attempt, gb(out.peak_mb)));
            }
            flush(&out.held_lines);
            return 0;
        }
        match out.oom {
            Some(sig) if attempt < attempts => {
                let msg = format!(
                    "{} ({}): OUT OF MEMORY on attempt {} of {}: {}, after {} s at a peak of {}. Waiting for more room, then compiling again (the {} held-back error line(s) of this attempt are dropped; they are in the slot log).",
                    unit.crate_name, unit.kind, attempt, attempts, sig, out.secs, gb(out.peak_mb), out.held_lines.len()
                );
                say(&msg);
                slot.log(&msg);
                for l in out.held_lines.iter().take(20) {
                    slot.log(&format!("  dropped: {}", l.trim_end().chars().take(400).collect::<String>()));
                }
                append_retry_ledger(&slot, &unit, attempt, sig, out.peak_mb);
                // Ask for more room next time: what it reached, or what was expected, plus a quarter.
                need = need.max(out.peak_mb) + need.max(out.peak_mb) / 4;
                last = Some(out);
            }
            Some(sig) => {
                let msg = format!("{} ({}): OUT OF MEMORY on the last attempt ({} of {}): {}. Giving up; the build fails as rustc failed.", unit.crate_name, unit.kind, attempt, attempts, sig);
                say(&msg);
                slot.log(&msg);
                append_retry_ledger(&slot, &unit, attempt, sig, out.peak_mb);
                flush(&out.held_lines);
                return out.code.unwrap_or(1);
            }
            None => {
                flush(&out.held_lines);
                return out.code.unwrap_or(1);
            }
        }
    }
    // Unreachable in practice (the loop returns); keep the compiler and a reader honest.
    last.map(|o| {
        flush(&o.held_lines);
        o.code.unwrap_or(1)
    })
    .unwrap_or(1)
}

fn append_retry_ledger(slot: &Slot, unit: &Unit, attempt: u64, sig: &str, peak_mb: u64) {
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(slot.dir.join("oom-retries.txt")) {
        let cwd = env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
        let _ = writeln!(f, "{} {} {} attempt={} peak_mb={} cwd={} | {}", now_ms() / 1000, clock(), unit.kind, attempt, peak_mb, cwd, sig);
    }
}

fn flush(lines: &[String]) {
    let mut e = io::stderr().lock();
    for l in lines {
        let _ = e.write_all(l.as_bytes());
    }
    let _ = e.flush();
}

// ── the other commands ──────────────────────────────────────────────────────────

fn print_mem() {
    let m = memory();
    println!("avail_commit_mb={}", m.avail_commit_mb);
    println!("avail_phys_mb={}", m.avail_phys_mb);
    println!("commit_limit_mb={}", m.commit_limit_mb);
    println!("total_phys_mb={}", m.total_phys_mb);
}

fn print_status() -> i32 {
    let slot = match Slot::open() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot open {}: {e}", slot_dir().display());
            return 1;
        }
    };
    let policy = policy();
    let m = memory();
    println!("build slot      {}", slot.dir.display());
    println!("source sha256   {}", SRC_SHA256);
    println!(
        "memory          commit free {} of {}; physical free {} of {}; room {} (the smaller of commit free and physical free + {} paging)",
        gb(m.avail_commit_mb),
        gb(m.commit_limit_mb),
        gb(m.avail_phys_mb),
        gb(m.total_phys_mb),
        gb(room_mb(&m, &policy, 0).max(0) as u64),
        gb(policy.paging_mb)
    );
    let holders = slot.live("held-");
    let waiters = slot.live("wait-");
    println!("compiling       {}", holders.len());
    for h in &holders {
        println!(
            "  {} ({}) pid {} rustc {}: now {}, expected peak {}, for {} s, in {}",
            h.crate_name,
            h.kind,
            h.pid,
            h.child_pid,
            gb(private_mb(h.child_pid)),
            gb(h.mb),
            now_ms().saturating_sub(h.since_ms) / 1000,
            h.cwd
        );
    }
    println!("waiting         {}", waiters.len());
    for w in &waiters {
        println!("  {} ({}) pid {}: needs {}, waiting {} s, in {}", w.crate_name, w.kind, w.pid, gb(w.mb), now_ms().saturating_sub(w.since_ms) / 1000, w.cwd);
    }
    let kinds = ["lib-check-native", "lib-build-native", "lib-test-native", "lib-release-native", "lib-check-relay", "lib-test-relay", "bin-release-native", "bin-build-native"];
    println!("expected peaks  (learned from this machine's ledger, else the seed)");
    for k in kinds {
        println!("  {:<22} {}", k, gb(slot.expected_mb(k)));
    }
    0
}

/// --measure: the program runs with this process's own output; the numbers go to stderr last.
fn measure(cmd: &[OsString]) -> i32 {
    let Some(program) = cmd.first() else {
        eprintln!("hos-build-slot: --measure needs a program to run");
        return 2;
    };
    let t0 = Instant::now();
    let mut child = match Command::new(program).args(&cmd[1..]).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hos-build-slot: could not start {}: {e}", program.to_string_lossy());
            return 1;
        }
    };
    #[cfg(windows)]
    let job = {
        use std::os::windows::io::AsRawHandle;
        Job::holding(child.as_raw_handle() as sys::Handle, None)
    };
    let code = child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
    #[cfg(windows)]
    let (cpu, peak) = job.as_ref().map(|j| (j.cpu_s(), j.peak_mb())).unwrap_or((0.0, 0));
    #[cfg(not(windows))]
    let (cpu, peak) = (0.0, 0);
    let m = memory();
    eprintln!(
        "[measure] exit {code}, wall {:.0} s, cpu {:.0} s (the whole tree), peak memory {} MB (all its processes at once); free physical now {} MB",
        t0.elapsed().as_secs_f64(),
        cpu,
        peak,
        m.avail_phys_mb
    );
    code
}

fn main() {
    // OsStrings: an argument that is not valid Unicode (a path) must pass through, not panic.
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let first = match args.first() {
        Some(a) => a.to_string_lossy().to_string(),
        None => {
            eprintln!("hos-build-slot: cargo runs this as `hos-build-slot <rustc> <args...>` (rustc-workspace-wrapper). Commands: --status, --mem, --source.");
            std::process::exit(2);
        }
    };
    match first.as_str() {
        "--status" => std::process::exit(print_status()),
        "--mem" => {
            print_mem();
            return;
        }
        "--source" => {
            println!("{SRC_SHA256}");
            return;
        }
        "--measure" => std::process::exit(measure(&args[1..])),
        _ => {}
    }
    let rustc = args[0].as_os_str();
    let rest = &args[1..];
    let off = env::var("HUMANITY_BUILD_SLOT").map(|v| v.eq_ignore_ascii_case("off") || v == "0").unwrap_or(false);
    let code = match (off, classify(&text_args(rest))) {
        (false, Some(unit)) => gated(rustc, rest, unit),
        _ => passthrough(rustc, rest),
    };
    std::process::exit(code);
}

// ── tests: rustc --test scripts/build-slot/hos-build-slot.rs ───────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        // split like a shell would for these fixtures (no spaces inside values)
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn classifies_cargos_real_command_lines() {
        // Shapes copied from the rustc command lines cargo 1.88 ran on 2026-10-22.
        let check = a("--crate-name humanity_engine --edition=2021 src\\lib.rs --error-format=json --crate-type lib --emit=dep-info,metadata -C embed-bitcode=no -C debuginfo=2 --cfg feature=\"default\" --cfg feature=\"native\" --cfg feature=\"relay\"");
        assert_eq!(classify(&check).unwrap().kind, "lib-check-native");
        let test = a("--crate-name humanity_engine --edition=2021 src\\lib.rs --emit=dep-info,link -C embed-bitcode=no -C debuginfo=2 --test --cfg feature=\"native\"");
        assert_eq!(classify(&test).unwrap().kind, "lib-test-native");
        let release = a("--crate-name humanity_engine src\\lib.rs --crate-type lib --emit=dep-info,metadata,link -C opt-level=3 --cfg feature=\"native\"");
        assert_eq!(classify(&release).unwrap().kind, "lib-release-native");
        let relay = a("--crate-name humanity_engine src\\lib.rs --emit=dep-info,link --test --cfg feature=\"relay\"");
        assert_eq!(classify(&relay).unwrap().kind, "lib-test-relay");
        let bin = a("--crate-name HumanityOS src\\main.rs --crate-type bin --emit=dep-info,link -C opt-level=3 --cfg feature=\"native\"");
        assert_eq!(classify(&bin).unwrap().kind, "bin-release-native");
        let bs = a("--crate-name build_script_build build.rs --crate-type bin --emit=dep-info,link");
        assert_eq!(classify(&bs).unwrap().kind, "buildscript-build-none");
        let lint = a("--crate-name emdash_lint tests\\emdash_lint.rs --emit=dep-info,link --test --cfg feature=\"native\"");
        assert_eq!(classify(&lint).unwrap().kind, "other-test-native");
    }

    #[test]
    fn an_argfile_is_read_for_classification() {
        let dir = std::env::temp_dir().join(format!("hos-build-slot-argfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("args");
        std::fs::write(&file, "--crate-name\nhumanity_engine\n--edition=2021\nsrc\\lib.rs\n--emit=dep-info,link\n--test\n--cfg\nfeature=\"native\"\n").unwrap();
        let args = vec![OsString::from(format!("@{}", file.display()))];
        assert_eq!(classify(&text_args(&args)).unwrap().kind, "lib-test-native");
        // an @argument that is not a readable file stays as it is
        assert_eq!(text_args(&[OsString::from("@nowhere-at-all")]), vec!["@nowhere-at-all".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probes_pass_straight_through() {
        assert_eq!(classify(&a("-vV")), None);
        assert_eq!(classify(&a("- --crate-name ___ --print=file-names --crate-type bin")), None);
        assert_eq!(classify(&a("--crate-name foo --print cfg")), None);
        assert_eq!(classify(&[]), None);
    }

    #[test]
    fn out_of_memory_signatures_are_recognised() {
        assert!(oom_line_signature("memory allocation of 1073741824 bytes failed").is_some());
        assert!(oom_line_signature("LLVM ERROR: out of memory").is_some());
        assert!(oom_line_signature("error: failed to map file: The paging file is too small for this operation to complete. (os error 1455)").is_some());
        assert!(oom_line_signature("{\"$message_type\":\"diagnostic\",\"message\":\"found invalid metadata files for crate `wgpu`\",\"code\":{\"code\":\"E0786\"},\"level\":\"error\"}").is_some());
        assert!(oom_line_signature("LINK : fatal error LNK1102: out of memory").is_some());
        assert_eq!(oom_exit_signature(Some(0xC000_0409u32 as i32)).is_some(), true);
        assert_eq!(oom_exit_signature(Some(-1073740791)).is_some(), true); // the same code, signed
        // Ordinary failures are not retried.
        assert!(oom_line_signature("error[E0308]: mismatched types").is_none());
        assert!(oom_line_signature("LINK : fatal error LNK1318: Unexpected PDB error; LIMIT (12)").is_none());
        assert!(oom_exit_signature(Some(1)).is_none());
        assert!(oom_exit_signature(Some(101)).is_none());
    }

    #[test]
    fn errors_are_held_back_and_artifacts_are_not() {
        assert!(hold_back("{\"$message_type\":\"diagnostic\",\"message\":\"mismatched types\",\"level\":\"error\",\"spans\":[]}"));
        assert!(!hold_back("{\"$message_type\":\"artifact\",\"artifact\":\"x.rmeta\",\"emit\":\"metadata\"}"));
        assert!(!hold_back("{\"$message_type\":\"diagnostic\",\"message\":\"unused\",\"level\":\"warning\"}"));
        assert!(hold_back("memory allocation of 8 bytes failed"));
    }

    fn mem(commit: u64, phys: u64) -> Mem {
        Mem { avail_commit_mb: commit, avail_phys_mb: phys, commit_limit_mb: 0, total_phys_mb: 0 }
    }
    // The defaults: 2 GB margin, no paging.
    const P: Policy = Policy { margin_mb: 2048, paging_mb: 0 };

    #[test]
    fn admission_keys_on_physical_memory_then_commit_and_counts_promises() {
        // Plenty of both: 11 GB + 2 margin fits in 24 GB of physical memory.
        assert!(fits(&mem(118_000, 24_000), &P, 0, 11_000));
        // Physical is the limit (the 100 GB page file): 12 GB free cannot take 11 + 2, though
        // commit has 118 GB. This is the thrash the slot stops.
        assert!(!fits(&mem(118_000, 12_000), &P, 0, 11_000));
        assert!(fits(&mem(118_000, 13_100), &P, 0, 11_000));
        // Commit is the limit (the old 8 GB page file): 12 GB of commit, 24 GB physical.
        assert!(!fits(&mem(12_000, 24_000), &P, 0, 11_000));
        // Promised room counts: 24 less 12 promised = 12, short of 13; less 10 = 14 fits.
        assert!(!fits(&mem(118_000, 24_000), &P, 12_000, 11_000));
        assert!(fits(&mem(118_000, 24_000), &P, 10_000, 11_000));
        // A paging allowance, when someone sets one, is counted on the physical side only.
        let paging = Policy { margin_mb: 2048, paging_mb: 8192 };
        assert!(fits(&mem(118_000, 6_000), &paging, 0, 11_000));
        assert!(!fits(&mem(6_000, 6_000), &paging, 0, 11_000));
    }

    #[test]
    fn a_running_compile_promises_only_what_it_may_still_grow_by() {
        assert_eq!(still_to_grow_mb(11_000, 1_000), 10_000);
        assert_eq!(still_to_grow_mb(11_000, 12_500), 0);
    }

    #[test]
    fn the_ledger_learns_the_largest_recent_peak() {
        assert_eq!(learned_peak_mb(&[], 10_000), None);
        assert_eq!(learned_peak_mb(&[8_000, 9_000], 10_000), Some(9_900));
        // only the last ten count
        let mut v = vec![20_000];
        v.extend(std::iter::repeat(6_000).take(10));
        assert_eq!(learned_peak_mb(&v, 10_000), Some(6_600));
        // never below half the seed
        assert_eq!(learned_peak_mb(&[1_000], 10_000), Some(5_000));
    }

    #[test]
    fn records_round_trip() {
        let r = Record { pid: 42, start: 133_000_000_000, child_pid: 7, kind: "lib-test-native".into(), crate_name: "humanity_engine".into(), mb: 11_000, since_ms: 5, cwd: "C:\\Humanity".into() };
        assert_eq!(Record::parse(&r.to_text()), Some(r));
        assert_eq!(Record::parse("garbage"), None);
    }

    #[test]
    fn seeds_cover_every_kind() {
        assert_eq!(seed_peak_mb("lib-test-native"), 11_000);
        assert!(seed_peak_mb("lib-test-relay") < seed_peak_mb("lib-test-native"));
        assert!(seed_peak_mb("buildscript-build-none") <= 1_000);
        assert!(seed_peak_mb("nonsense") > 0);
    }
}
