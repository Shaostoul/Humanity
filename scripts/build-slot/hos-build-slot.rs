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
//!      default) and the free commit memory (what an allocation can get at all). With none of
//!      our compiles running, the first in line that fits on its own with a smaller margin
//!      starts, so the line never waits on room only its own head could free (2026-11-14; the
//!      rule and its reasons: decide()). It logs the wait and gives up waiting after a
//!      ceiling, compiling anyway with a loud line.
//!   2. RUN rustc, passing its output straight through to cargo line by line (cargo needs
//!      the artifact lines as they come). Error diagnostics are held back until rustc exits.
//!   3. RETRY. When rustc died of memory (an out-of-memory signature in its output, or an
//!      exit code Windows uses for it), it waits for more room and compiles again, up to
//!      three attempts, with a loud line each time. The held-back errors of a retried attempt
//!      are dropped, so a compile that succeeded on its second try shows no errors.
//!   4. LEARN. The peaks of every successful compile (the whole process tree, the linker
//!      included: its commit from a Windows job object, its working set summed over the job's
//!      processes) are written to a ledger, and the expected need of each kind of compile is
//!      the largest of its last ten, plus 10%: the working set against free physical memory,
//!      the commit against free commit memory (learned_need says why two, 2026-11-14).
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
//!   HOS_BUILD_SLOT_IDLE_MARGIN_MB=<margin/2>  the margin with none of our compiles running
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
        // The workspace's other crates (the crate split, rung 1, 2026-11-01): each a kind of its
        // own, named by the crate (`core-check-native`, `protocol-test-native`), so the ledger
        // learns the core's peak apart from the engine's and from the lints'. Until it has, the
        // seed is the one for "other" (seed_peak_mb).
        n if n.starts_with("humanity_") => n.trim_start_matches("humanity_"),
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
    /// the margin when none of our compiles is running (see decide)
    idle_margin_mb: u64,
}

/// The rule's numbers, from the environment or the defaults: 2 GB kept free beyond the
/// expected peak, and no paging. Paging is the waste this exists to stop (see the header).
/// With none of our compiles running, half the margin (see decide).
fn policy() -> Policy {
    let margin_mb = env_u64("HOS_BUILD_SLOT_MARGIN_MB", 2048);
    Policy { margin_mb, paging_mb: env_u64("HOS_BUILD_SLOT_PAGING_MB", 0), idle_margin_mb: env_u64("HOS_BUILD_SLOT_IDLE_MARGIN_MB", margin_mb / 2).min(margin_mb) }
}

/// What a compile needs at its peak, or what is promised to compiles: commit memory (what
/// Windows must promise it) and physical memory (what it holds in RAM).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Need {
    commit_mb: u64,
    phys_mb: u64,
}

impl Need {
    fn both(mb: u64) -> Need {
        Need { commit_mb: mb, phys_mb: mb }
    }
    fn plus(self, o: Need) -> Need {
        Need { commit_mb: self.commit_mb + o.commit_mb, phys_mb: self.phys_mb + o.phys_mb }
    }
}

/// The room a new compile can count on, for the lines a person reads: the smaller of free
/// commit memory and free physical memory plus the paging allowance, less what is promised.
fn room_mb(mem: &Mem, policy: &Policy, promised_mb: u64) -> i64 {
    let physical = mem.avail_phys_mb + policy.paging_mb;
    let base = mem.avail_commit_mb.min(physical);
    base as i64 - promised_mb as i64
}

/// What an admitted compile may still grow by: its expected peak less what it holds now.
fn still_to_grow_mb(expect_mb: u64, current_mb: u64) -> u64 {
    expect_mb.saturating_sub(current_mb)
}

/// Does a compile needing `need` fit now, `margin_mb` kept free beyond it? `promised` is what
/// admitted compiles may still grow by plus the needs of compiles ahead of it in line.
fn fits(mem: &Mem, policy: &Policy, promised: Need, need: Need, margin_mb: u64) -> bool {
    let phys_room = (mem.avail_phys_mb + policy.paging_mb) as i64 - promised.phys_mb as i64;
    let commit_room = mem.avail_commit_mb as i64 - promised.commit_mb as i64;
    phys_room >= (need.phys_mb + margin_mb) as i64 && commit_room >= (need.commit_mb + margin_mb) as i64
}

/// One compile in line for room, as every waiting compile sees it (its wait- record).
#[derive(Debug, Clone, Copy, PartialEq)]
struct InLine {
    pid: u32,
    since_ms: u64,
    need: Need,
}

/// What the rule says to one waiting compile.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Verdict {
    /// It fits beside what runs, with the full margin, after everyone ahead of it.
    Start,
    /// None of ours is running and it is the first in line that fits on its own (with the idle
    /// margin); `passed` compiles ahead of it cannot start even so.
    Alone { passed: usize },
    /// Wait and look again.
    Wait,
}

/// THE ADMISSION RULE (pure; the queue simulation in the tests runs it). `line` is every
/// compile waiting for room, `me` among them; `running` how many of ours hold room now and
/// `growth` what they may still grow by.
///
/// 1. FIRST COME, FIRST SERVED. A compile starts when it fits, with the full margin, beside
///    what the running ones may still grow by AND the needs of every compile ahead of it in
///    line. A later compile passes an earlier one only when there is room for both.
/// 2. WITH NONE OF OURS RUNNING (2026-11-14). Then the rule above can deadlock: the head of
///    the line waits for memory that is not ours to free, and everyone behind it counts the
///    head's need as promised, so nothing runs at all. On 2026-11-14 a lib-test expected at
///    14.4 GB needed 16.4 GB with the margin, 16.3 GB was free, none of ours was compiling, 14
///    compiles waited behind it, and the head waited 17 minutes (then peaked at 10.7 GB). So,
///    with none of ours running, the FIRST compile in line that fits ON ITS OWN with the idle
///    margin (half the margin by default) starts:
///    - Usually that is the head itself. The smaller margin is safe there because the margin is
///      for what else may grow while it compiles, and none of ours can (everyone behind it
///      waits for it, rule 1), while its need is already the largest of its kind's last ten
///      peaks plus 10%. The out-of-memory retry stays the net under that.
///    - When even that leaves no room for the head (the rest of the machine holds the memory),
///      the first compile behind it that fits on its own goes instead, ONE at a time: once it
///      runs, rule 1 counts the head against everyone again. When it finishes, none of ours runs
///      again, so the head is looked at first, and it starts before anyone behind it the moment
///      the machine has room for it. Nobody behind it ever holds the head's room hostage.
fn decide(mem: &Mem, policy: &Policy, running: usize, growth: Need, line: &[InLine], me: u32) -> Verdict {
    let mut line: Vec<InLine> = line.to_vec();
    line.sort_by_key(|w| (w.since_ms, w.pid));
    let Some(pos) = line.iter().position(|w| w.pid == me) else {
        return Verdict::Wait;
    };
    let ahead = line[..pos].iter().fold(Need::default(), |a, w| a.plus(w.need));
    if fits(mem, policy, growth.plus(ahead), line[pos].need, policy.margin_mb) {
        return Verdict::Start;
    }
    if running > 0 {
        return Verdict::Wait;
    }
    let alone = |w: &InLine| fits(mem, policy, Need::default(), w.need, policy.idle_margin_mb);
    match line.iter().position(alone) {
        Some(first) if first == pos => Verdict::Alone { passed: pos },
        _ => Verdict::Wait,
    }
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

/// A peak no compile reaches (1 TB): a larger number in the ledger is a torn line, not a reading.
const PLAUSIBLE_PEAK_MB: u64 = 1024 * 1024;

/// One ledger line: epoch-seconds kind peak-commit-MB seconds peak-working-set-MB cpu-seconds.
/// The working set is the whole tree's since 2026-11-14 (TreeWatch); older lines hold rustc's
/// own, which leaves out the linker, and fall out of every kind's last ten within ten compiles
/// (until then the 3/4-of-commit floor in learned_need covers the linker kinds).
fn ledger_line(epoch_s: u64, kind: &str, peak_mb: u64, secs: u64, ws_mb: u64, cpu_s: f64) -> String {
    format!("{} {} {} {} {} {:.0}\n", epoch_s, kind, peak_mb, secs, ws_mb, cpu_s.max(0.0))
}

/// Appends a whole line in ONE write. `writeln!` on a File writes each piece of the format
/// separately, so two compiles finishing at the same moment interleaved their lines (2026-11-02:
/// 29 torn lines in a day, one of them a "peak" of 1,081,793,679,699 MB that held every
/// other-check-relay compile for the full 60-minute ceiling). One write to a file opened for
/// appending lands whole at the end.
fn append_ledger_line(path: &std::path::Path, line: &str) {
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// A ledger line's (commit peak, working-set peak) for `kind`, or None when the line is not
/// exactly the six fields `ledger_line` writes, with a plausible commit peak (the ledger may still
/// hold torn lines from before `append_ledger_line`, and a hand edit may leave anything). The
/// working set is 0 when it could not be read, and above the plausible peak it reads as 0 too.
fn ledger_peaks(line: &str, kind: &str) -> Option<(u64, u64)> {
    let f: Vec<&str> = line.split(' ').collect();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if f.len() != 6 || f[1] != kind || !digits(f[0]) || f[0].len() != 10 || !f[2..].iter().all(|s| digits(s)) {
        return None;
    }
    let commit = f[2].parse().ok().filter(|&mb| mb > 0 && mb < PLAUSIBLE_PEAK_MB)?;
    let ws = f[4].parse().ok().filter(|&mb| mb < PLAUSIBLE_PEAK_MB).unwrap_or(0);
    Some((commit, ws))
}

/// The expected need of a kind from its ledger lines (commit, working set; newest last), or the
/// seed for both when it has none (2026-11-14).
///
/// WHY TWO. The slot keys on free PHYSICAL memory (the header), but until 2026-11-14 the need it
/// compared against free physical memory was the COMMIT peak (the job object's figure), and a
/// compile commits more than it ever holds in RAM: over the last ten lib-test-native compiles
/// of 2026-11-14 rustc's peak working set was 82% of the tree's commit peak (8.9 against 10.8 GB
/// on average, 11.4 against 13.6 at the largest). So the 14.4 GB the deadlocked head was
/// expected to need (13.4 GB, the largest commit of its last ten, plus 10%) asked for about 2 GB
/// more physical memory than any of those compiles held; learned from the working set it is
/// 12.2 GB, which fitted the 16.3 GB free with the full margin. Commit is still checked, against
/// free commit memory, with the commit peak.
///
/// The physical need is never below 3/4 of the commit need: a working set measured while Windows
/// was trimming it under memory pressure reads low, and learning from it would admit more
/// compiles into the same pressure. (The ratio was 0.71 to 0.88 for every kind rustc dominates.)
/// Both are the largest of the last ten plus 10% (learned_peak_mb), so one full compile among
/// incremental ones (13.3 against 10.7 GB for a lib-test) still sets the need: which kind the
/// next compile will be is not known before it runs.
fn learned_need(observed: &[(u64, u64)], seed: u64) -> Need {
    let commits: Vec<u64> = observed.iter().map(|o| o.0).collect();
    let commit = learned_peak_mb(&commits, seed).unwrap_or(seed);
    let wss: Vec<u64> = observed.iter().map(|o| o.1).filter(|&ws| ws > 0).collect();
    let phys = match learned_peak_mb(&wss, seed) {
        Some(p) => p.max(commit * 3 / 4),
        None => commit,
    };
    Need { commit_mb: commit, phys_mb: phys }
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
    /// the expected commit peak, MB
    mb: u64,
    /// the expected physical (working set) peak, MB
    phys_mb: u64,
    since_ms: u64,
    cwd: String,
}

impl Record {
    fn to_text(&self) -> String {
        format!(
            "pid={}\nstart={}\nchild_pid={}\nkind={}\ncrate={}\nmb={}\nphys_mb={}\nsince_ms={}\ncwd={}\n",
            self.pid, self.start, self.child_pid, self.kind, self.crate_name, self.mb, self.phys_mb, self.since_ms, self.cwd
        )
    }
    fn need(&self) -> Need {
        Need { commit_mb: self.mb, phys_mb: self.phys_mb }
    }
    fn parse(text: &str) -> Option<Record> {
        let mut r = Record::default();
        let mut phys = None;
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
                "phys_mb" => phys = Some(v.parse().ok()?),
                "since_ms" => r.since_ms = v.parse().ok()?,
                "cwd" => r.cwd = v.to_string(),
                _ => {}
            }
        }
        // A record written by a wrapper from before the physical need (one still compiling while
        // the new one was installed) promises its commit need in both.
        r.phys_mb = phys.unwrap_or(r.mb);
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
    pub const JOB_OBJECT_BASIC_PROCESS_ID_LIST: i32 = 3;

    /// JOBOBJECT_BASIC_PROCESS_ID_LIST with room for 256 processes (a compile has two or three).
    #[repr(C)]
    pub struct JobProcessIdList {
        pub assigned: u32,
        pub in_list: u32,
        pub ids: [usize; 256],
    }
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

/// What a process holds now, MB: its private (commit) memory and its working set (physical
/// memory); zeros when it cannot be read.
#[cfg(windows)]
fn usage_mb(pid: u32) -> Need {
    if pid == 0 {
        return Need::default();
    }
    unsafe {
        let h = sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION | sys::PROCESS_VM_READ, 0, pid);
        if h.is_null() {
            return Need::default();
        }
        let mut pmc: sys::ProcessMemoryCounters = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<sys::ProcessMemoryCounters>() as u32;
        let ok = sys::K32GetProcessMemoryInfo(h, &mut pmc, pmc.cb);
        sys::CloseHandle(h);
        if ok == 0 {
            Need::default()
        } else {
            Need { commit_mb: pmc.pagefile_usage as u64 / MB, phys_mb: pmc.working_set_size as u64 / MB }
        }
    }
}
#[cfg(not(windows))]
fn usage_mb(_pid: u32) -> Need {
    Need::default()
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

/// A handle the watcher thread may carry (Windows handles are process-wide).
#[cfg(windows)]
struct SendHandle(sys::Handle);
#[cfg(windows)]
unsafe impl Send for SendHandle {}

/// The peak PHYSICAL memory of a compile's whole process tree (2026-11-14). A job object reports
/// the tree's peak commit, not its working set, and rustc's own working set leaves out the
/// linker it starts at the end (for bin-release-native the linker is most of it: 2.0 GB of
/// commit against rustc's own 0.6 GB working set). So a thread lists the job's processes every
/// 200 ms while the compile runs, opens each new one, and at the end each one's own peak working
/// set is read and the peaks are summed. The sum of the peaks is never below the peak of the
/// sum: it can over-count a tree whose processes peaked at different moments, never under-count.
#[cfg(windows)]
struct TreeWatch {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<Vec<SendHandle>>>,
}

#[cfg(windows)]
impl TreeWatch {
    /// Watch `job` until finish() (or drop). The job must outlive the watch: declare the watch
    /// after the job, so it drops (and joins its thread) first.
    fn start(job: &Job) -> TreeWatch {
        use std::sync::atomic::Ordering;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stop.clone();
        let job_h = SendHandle(job.0);
        let thread = std::thread::spawn(move || {
            let job_h = job_h;
            let mut seen: Vec<u32> = Vec::new();
            let mut handles: Vec<SendHandle> = Vec::new();
            loop {
                // Read the flag BEFORE listing, so the last pass still lists once after it is set.
                let last = flag.load(Ordering::Acquire);
                unsafe {
                    let mut list: sys::JobProcessIdList = std::mem::zeroed();
                    let ok = sys::QueryInformationJobObject(
                        job_h.0,
                        sys::JOB_OBJECT_BASIC_PROCESS_ID_LIST,
                        &mut list as *mut _ as *mut _,
                        std::mem::size_of::<sys::JobProcessIdList>() as u32,
                        std::ptr::null_mut(),
                    );
                    if ok != 0 {
                        for &id in list.ids.iter().take((list.in_list as usize).min(list.ids.len())) {
                            let pid = id as u32;
                            if pid != 0 && !seen.contains(&pid) {
                                seen.push(pid);
                                let h = sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION | sys::PROCESS_VM_READ, 0, pid);
                                if !h.is_null() {
                                    handles.push(SendHandle(h));
                                }
                            }
                        }
                    }
                }
                if last {
                    return handles;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        TreeWatch { stop, thread: Some(thread) }
    }

    /// Stop watching and return the summed peak working sets of every process seen, MB.
    fn finish(&mut self) -> u64 {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        let handles = self.thread.take().and_then(|t| t.join().ok()).unwrap_or_default();
        let mut bytes = 0u64;
        for h in handles {
            unsafe {
                let mut pmc: sys::ProcessMemoryCounters = std::mem::zeroed();
                pmc.cb = std::mem::size_of::<sys::ProcessMemoryCounters>() as u32;
                if sys::K32GetProcessMemoryInfo(h.0, &mut pmc, pmc.cb) != 0 {
                    bytes += pmc.peak_working_set_size as u64;
                }
                sys::CloseHandle(h.0);
            }
        }
        bytes / MB
    }
}

#[cfg(windows)]
impl Drop for TreeWatch {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.finish();
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
    fn expected(&self, kind: &str) -> Need {
        if let Some(p) = env::var("HOS_BUILD_SLOT_PEAK_MB").ok().and_then(|v| v.parse().ok()) {
            return Need::both(p);
        }
        let text = fs::read_to_string(self.dir.join("peaks.txt")).unwrap_or_default();
        let observed: Vec<(u64, u64)> = text.lines().filter_map(|l| ledger_peaks(l, kind)).collect();
        learned_need(&observed, seed_peak_mb(kind))
    }

    fn record_peak(&self, kind: &str, peak_mb: u64, secs: u64, ws_mb: u64, cpu_s: f64) {
        let path = self.dir.join("peaks.txt");
        append_ledger_line(&path, &ledger_line(now_ms() / 1000, kind, peak_mb, secs, ws_mb, cpu_s));
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

/// What admitted compiles may still grow by, each kind of memory against its own expected peak.
fn promised_by_holders(holders: &[Record], me: u32) -> Need {
    holders.iter().filter(|h| h.pid != me).fold(Need::default(), |a, h| {
        let now = usage_mb(h.child_pid);
        a.plus(Need { commit_mb: still_to_grow_mb(h.mb, now.commit_mb), phys_mb: still_to_grow_mb(h.phys_mb, now.phys_mb) })
    })
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

/// "14.4 GB (11.0 GB physical)": a need for the lines a person reads.
fn need_text(n: Need) -> String {
    if n.phys_mb == n.commit_mb {
        gb(n.commit_mb)
    } else {
        format!("{} physical ({} commit)", gb(n.phys_mb), gb(n.commit_mb))
    }
}

/// Wait until this compile fits, then hold its room. Never fails: past the ceiling it
/// proceeds anyway, saying so.
fn admit<'a>(slot: &'a Slot, unit: &Unit, need: Need, arrived_ms: u64, policy: &Policy) -> Held<'a> {
    let poll = Duration::from_millis(env_u64("HOS_BUILD_SLOT_POLL_MS", 2000).max(50));
    let ceiling = Duration::from_secs(env_u64("HOS_BUILD_SLOT_MAX_WAIT_MIN", 60) * 60);
    let wait_name = format!("wait-{}.txt", slot.me);
    let mine = Record {
        pid: slot.me,
        start: slot.my_start,
        child_pid: 0,
        kind: unit.kind.clone(),
        crate_name: unit.crate_name.clone(),
        mb: need.commit_mb,
        phys_mb: need.phys_mb,
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
            let holders: Vec<Record> = slot.live("held-").into_iter().filter(|h| h.pid != slot.me).collect();
            let waiters = slot.live("wait-");
            // The line: every waiting compile's record, this one's as it is now.
            let mut line: Vec<InLine> = waiters.iter().filter(|w| w.pid != slot.me).map(|w| InLine { pid: w.pid, since_ms: w.since_ms, need: w.need() }).collect();
            line.push(InLine { pid: slot.me, since_ms: arrived_ms, need });
            let growth = promised_by_holders(&holders, slot.me);
            let verdict = decide(&mem, policy, holders.len(), growth, &line, slot.me);
            let over_ceiling = t0.elapsed() >= ceiling;
            if verdict != Verdict::Wait || over_ceiling {
                let held = Record { since_ms: now_ms(), ..mine.clone() };
                let _ = slot.write_record(&format!("held-{}.txt", slot.me), &held);
                slot.remove(&wait_name);
                Ok((held, mem, growth, verdict, over_ceiling, holders.len()))
            } else {
                if !announced {
                    let _ = slot.write_record(&wait_name, &mine);
                }
                Err((mem, growth, holders, line))
            }
        };
        match decision {
            Ok((rec, mem, growth, verdict, over_ceiling, n_holders)) => {
                let waited = t0.elapsed().as_secs();
                if verdict == Verdict::Wait && over_ceiling {
                    let msg = format!(
                        "{} ({}): WAITED {} min, the ceiling, and the machine never had room for {} (+{} margin); compiling anyway. Room now {} with {} promised to {} other compile(s). Out of memory here is retried.",
                        unit.crate_name, unit.kind, waited / 60, need_text(need), gb(policy.margin_mb), gb(room_mb(&mem, policy, 0).max(0) as u64), need_text(growth), n_holders
                    );
                    say(&msg);
                    slot.log(&msg);
                } else if let Verdict::Alone { passed } = verdict {
                    let why = if passed == 0 {
                        format!("none of our compiles is running, so it starts with the idle margin ({}) instead of waiting for room only it could free", gb(policy.idle_margin_mb))
                    } else {
                        format!(
                            "the {passed} compile(s) ahead of it cannot start even with none of ours running (the rest of the machine holds the memory), so it goes first, alone; they are first in line again the moment it finishes"
                        )
                    };
                    let msg = format!(
                        "{} ({}): starting after {} s: {}. Needs {}; physical free {}, commit free {}.",
                        unit.crate_name, unit.kind, waited, why, need_text(need), gb(mem.avail_phys_mb), gb(mem.avail_commit_mb)
                    );
                    say(&msg);
                    slot.log(&msg);
                } else if announced {
                    let msg = format!("{} ({}): room for {} after waiting {} s; compiling.", unit.crate_name, unit.kind, need_text(need), waited);
                    say(&msg);
                    slot.log(&msg);
                } else {
                    slot.log(&format!(
                        "admit {} ({}) need {} MB physical, {} MB commit; commit free {} MB, physical free {} MB, promised {} MB physical",
                        unit.crate_name, unit.kind, need.phys_mb, need.commit_mb, mem.avail_commit_mb, mem.avail_phys_mb, growth.phys_mb
                    ));
                }
                return Held { slot, rec };
            }
            Err((mem, growth, holders, line)) => {
                if !announced || last_note.elapsed() >= Duration::from_secs(60) {
                    let others: Vec<String> = holders
                        .iter()
                        .map(|h| format!("{} ({}, {} of {})", h.crate_name, h.kind, gb(usage_mb(h.child_pid).phys_mb), gb(h.phys_mb)))
                        .collect();
                    let ahead = line.iter().filter(|w| (w.since_ms, w.pid) < (arrived_ms, slot.me)).fold(Need::default(), |a, w| a.plus(w.need));
                    let msg = format!(
                        "{} ({}): {} for memory: needs {} + {} margin; commit free {}, physical free {} (+{} paging allowed); {} promised to compiles already running, {} to {} waiting longer. Running: {}. Waiting: {}. Ceiling {} min; HUMANITY_BUILD_SLOT=off skips the wait.",
                        unit.crate_name,
                        unit.kind,
                        if announced { format!("still waiting ({} s)", t0.elapsed().as_secs()) } else { "WAITING".to_string() },
                        need_text(need),
                        gb(policy.margin_mb),
                        gb(mem.avail_commit_mb),
                        gb(mem.avail_phys_mb),
                        gb(policy.paging_mb),
                        need_text(growth),
                        need_text(ahead),
                        line.iter().filter(|w| (w.since_ms, w.pid) < (arrived_ms, slot.me)).count(),
                        if others.is_empty() { "none of ours".to_string() } else { others.join(", ") },
                        line.len(),
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
    /// the whole tree's peak working set (physical memory), MB, for the ledger (TreeWatch)
    ws_mb: u64,
    /// rustc's own peak working set, MB, for the log beside the tree's
    own_ws_mb: u64,
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
    // Declared after the job, so it drops (joining its thread) before the job closes.
    #[cfg(windows)]
    let mut watch = job.as_ref().map(TreeWatch::start);
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
    let (peak_mb, ws_mb, own_ws_mb, cpu_s) = {
        let (own, own_ws) = child_peaks_mb(&child);
        let tree_ws = watch.as_mut().map(|w| w.finish()).unwrap_or(0);
        (job.as_ref().map(|j| j.peak_mb()).unwrap_or(0).max(own), tree_ws.max(own_ws), own_ws, job.as_ref().map(|j| j.cpu_s()).unwrap_or(0.0))
    };
    #[cfg(not(windows))]
    let (peak_mb, ws_mb, own_ws_mb, cpu_s) = (0, 0, 0, 0.0);
    Ok(Outcome { code, oom: if status.success() { None } else { oom }, held_lines, peak_mb, ws_mb, own_ws_mb, cpu_s, secs: t0.elapsed().as_secs() })
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
    let mut need = slot.expected(&unit.kind);
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
            slot.log(&format!(
                "done {} ({}) in {} s (cpu {:.0} s), peak {} MB commit, working set {} MB (rustc alone {} MB), attempt {}",
                unit.crate_name, unit.kind, out.secs, out.cpu_s, out.peak_mb, out.ws_mb, out.own_ws_mb, attempt
            ));
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
                let more = |n: u64, seen: u64| n.max(seen) + n.max(seen) / 4;
                need = Need { commit_mb: more(need.commit_mb, out.peak_mb), phys_mb: more(need.phys_mb, out.ws_mb) };
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
    let mut waiters = slot.live("wait-");
    waiters.sort_by_key(|w| (w.since_ms, w.pid));
    println!("compiling       {}", holders.len());
    for h in &holders {
        let now = usage_mb(h.child_pid);
        println!(
            "  {} ({}) pid {} rustc {}: now {} of {} physical ({} of {} commit), for {} s, in {}",
            h.crate_name,
            h.kind,
            h.pid,
            h.child_pid,
            gb(now.phys_mb),
            gb(h.phys_mb),
            gb(now.commit_mb),
            gb(h.mb),
            now_ms().saturating_sub(h.since_ms) / 1000,
            h.cwd
        );
    }
    // The line in its order, and what the rule says to each one right now (none of them is
    // started by this: each decides for itself on its next look).
    let line: Vec<InLine> = waiters.iter().map(|w| InLine { pid: w.pid, since_ms: w.since_ms, need: w.need() }).collect();
    let growth = promised_by_holders(&holders, 0);
    println!("waiting         {} (first in line first)", waiters.len());
    for w in &waiters {
        let v = decide(&m, &policy, holders.len(), growth, &line, w.pid);
        println!("  {} ({}) pid {}: needs {}, waiting {} s, rule says {:?}, in {}", w.crate_name, w.kind, w.pid, need_text(w.need()), now_ms().saturating_sub(w.since_ms) / 1000, v, w.cwd);
    }
    let kinds = ["lib-check-native", "lib-build-native", "lib-test-native", "lib-release-native", "lib-check-relay", "lib-test-relay", "bin-release-native", "bin-build-native"];
    println!("expected peaks  (learned from this machine's ledger, else the seed)");
    for k in kinds {
        println!("  {:<22} {}", k, need_text(slot.expected(k)));
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

    /// A ledger line's commit peak for `kind`.
    fn ledger_peak(line: &str, kind: &str) -> Option<u64> {
        ledger_peaks(line, kind).map(|(commit, _)| commit)
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
        // The workspace's crates (rung 1 of the crate split): a kind each, named by the crate.
        let core = a("--crate-name humanity_core --edition=2021 crates\\humanity-core\\src\\lib.rs --crate-type lib --emit=dep-info,metadata -C embed-bitcode=no --cfg feature=\"native\"");
        assert_eq!(classify(&core).unwrap().kind, "core-check-native");
        let protocol = a("--crate-name humanity_protocol --edition=2021 crates\\humanity-protocol\\src\\lib.rs --emit=dep-info,link --test --cfg feature=\"test-support\"");
        assert_eq!(classify(&protocol).unwrap().kind, "protocol-test-none");
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
    // The defaults: 2 GB margin, no paging, half the margin with none of ours running.
    const P: Policy = Policy { margin_mb: 2048, paging_mb: 0, idle_margin_mb: 1024 };

    #[test]
    fn admission_keys_on_physical_memory_then_commit_and_counts_promises() {
        // Plenty of both: 11 GB + 2 margin fits in 24 GB of physical memory.
        assert!(fits(&mem(118_000, 24_000), &P, Need::both(0), Need::both(11_000), P.margin_mb));
        // Physical is the limit (the 100 GB page file): 12 GB free cannot take 11 + 2, though
        // commit has 118 GB. This is the thrash the slot stops.
        assert!(!fits(&mem(118_000, 12_000), &P, Need::both(0), Need::both(11_000), P.margin_mb));
        assert!(fits(&mem(118_000, 13_100), &P, Need::both(0), Need::both(11_000), P.margin_mb));
        // Commit is the limit (the old 8 GB page file): 12 GB of commit, 24 GB physical.
        assert!(!fits(&mem(12_000, 24_000), &P, Need::both(0), Need::both(11_000), P.margin_mb));
        // Promised room counts: 24 less 12 promised = 12, short of 13; less 10 = 14 fits.
        assert!(!fits(&mem(118_000, 24_000), &P, Need::both(12_000), Need::both(11_000), P.margin_mb));
        assert!(fits(&mem(118_000, 24_000), &P, Need::both(10_000), Need::both(11_000), P.margin_mb));
        // A paging allowance, when someone sets one, is counted on the physical side only.
        let paging = Policy { margin_mb: 2048, paging_mb: 8192, idle_margin_mb: 1024 };
        assert!(fits(&mem(118_000, 6_000), &paging, Need::both(0), Need::both(11_000), paging.margin_mb));
        assert!(!fits(&mem(6_000, 6_000), &paging, Need::both(0), Need::both(11_000), paging.margin_mb));
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

    /// The ten lib-test-native ledger lines before the 2026-11-14 deadlock (commit MB, rustc's
    /// working set MB), copied from this machine's peaks.txt, 14:04 that day.
    const LIB_TEST_2026_11_14: [(u64, u64); 10] = [
        (13_427, 11_375),
        (9_342, 7_277),
        (10_764, 8_929),
        (13_180, 11_069),
        (10_728, 8_894),
        (13_254, 11_179),
        (13_264, 11_173),
        (10_740, 8_911),
        (10_721, 8_885),
        (10_301, 8_366),
    ];

    /// The physical need is learned from the working set, not the commit (learned_need).
    /// RED on the learning before 2026-11-14 (the commit peak used as the physical need too):
    /// the head needed 14,769 MB of physical memory, and with the full margin it did not fit the
    /// 16,691 MB that was free with none of ours running (the deadlock's other half).
    #[test]
    fn the_physical_need_is_learned_from_the_working_set() {
        let need = learned_need(&LIB_TEST_2026_11_14, seed_peak_mb("lib-test-native"));
        // 13,427 + 10% = 14,769 MB commit (the "14.4 GB" of the log); 11,375 + 10% = 12,512 MB physical.
        assert_eq!(need, Need { commit_mb: 14_769, phys_mb: 12_512 });
        let that_moment = mem(114_000, 16_691);
        assert!(fits(&that_moment, &P, Need::default(), need, P.margin_mb), "12.2 GB + 2 GB margin fits 16.3 GB");
        assert!(!fits(&that_moment, &P, Need::default(), Need::both(need.commit_mb), P.margin_mb), "the old need, 14.4 GB as physical, did not");
        // Never below 3/4 of the commit need (a working set trimmed under pressure reads low).
        assert_eq!(learned_need(&[(12_000, 2_000)], 11_000), Need { commit_mb: 13_200, phys_mb: 9_900 });
        // No working set read (0): the commit need stands for both; no lines at all: the seed.
        assert_eq!(learned_need(&[(8_000, 0)], 11_000), Need::both(8_800));
        assert_eq!(learned_need(&[], 11_000), Need::both(11_000));
        // The ledger's own lines read back as (commit, working set).
        assert_eq!(ledger_peaks(ledger_line(1_793_683_300, "lib-test-native", 9_800, 312, 7_400, 640.0).trim_end(), "lib-test-native"), Some((9_800, 7_400)));
    }

    #[test]
    fn a_torn_ledger_line_is_not_a_peak() {
        // Two real torn lines from this machine's ledger, 2026-11-02. The first held every
        // other-check-relay compile for the full 60-minute ceiling ("needs 1162083054.4 GB").
        assert_eq!(ledger_peak("1793679699 other-check-relay 1081793679699  0other-check-relay 40  108 0 039", "other-check-relay"), None);
        assert_eq!(ledger_peak("1793654343 other-check-relay 107 0 179365434358 other-check-relay  1240 ", "other-check-relay"), None);
        assert_eq!(ledger_peak(" 0", "other-check-relay"), None);
        assert_eq!(ledger_peak("1793683300 other-check-relay 108 0 40 0", "other-check-relay"), Some(108));
        assert_eq!(ledger_peak("1793683300 other-check-relay 108 0 40 0", "lib-check-relay"), None);
        // Well-formed but impossible: no compile reaches a terabyte.
        assert_eq!(ledger_peak("1793683300 other-check-relay 1048576 0 40 0", "other-check-relay"), None);
        // What ledger_line writes reads back, a NaN CPU time included.
        let line = ledger_line(1_793_683_300, "lib-test-native", 9_800, 312, 7_400, 640.4);
        assert_eq!(ledger_peak(line.trim_end(), "lib-test-native"), Some(9_800));
        assert_eq!(ledger_peak(ledger_line(1_793_683_300, "x-y-z", 5, 1, 1, f64::NAN).trim_end(), "x-y-z"), Some(5));
    }

    #[test]
    fn ledger_lines_appended_together_never_tear() {
        // Eight writers at once, each with its own handle, the way separate compiles append.
        // Red with the old write (one write per piece of the format, as writeln! did): see the commit.
        let dir = std::env::temp_dir().join(format!("hos-build-slot-ledger-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("peaks.txt");
        let _ = std::fs::remove_file(&path);
        let writers: Vec<_> = (0..8u64)
            .map(|w| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..400u64 {
                        append_ledger_line(&path, &ledger_line(1_793_683_300 + i, "other-check-relay", 100 + w, i, 40, 1.0));
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let torn: Vec<&&str> = lines.iter().filter(|l| ledger_peak(l, "other-check-relay").is_none()).collect();
        assert!(torn.is_empty(), "{} torn of {}: {:?}", torn.len(), lines.len(), torn.iter().take(3).collect::<Vec<_>>());
        assert_eq!(lines.len(), 8 * 400);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_round_trip() {
        let r = Record { pid: 42, start: 133_000_000_000, child_pid: 7, kind: "lib-test-native".into(), crate_name: "humanity_engine".into(), mb: 11_000, phys_mb: 9_000, since_ms: 5, cwd: "C:\\Humanity".into() };
        assert_eq!(Record::parse(&r.to_text()), Some(r));
        assert_eq!(Record::parse("garbage"), None);
        // A record with no physical need (a wrapper from before it, still compiling while this
        // one was installed) promises its commit need as both.
        let old = Record::parse("pid=9\nmb=11000\nsince_ms=1\n").unwrap();
        assert_eq!(old.need(), Need::both(11_000));
    }

    // ── the queue, simulated ────────────────────────────────────────────────────────
    // A model of this machine and its line: every waiting compile looks again each poll (one
    // tick, 2 s), the decisions are taken one at a time under the lock and each sees what the
    // ones before it left, and a compile holds what it really uses (`uses`) from its start while
    // the rule counts what it may still grow by to its expected peak (`need`). The commit side
    // has the 100 GB page file, so physical memory is the limit, as it is on the dev PC.

    #[derive(Clone, Copy)]
    struct SimJob {
        arrive: u64,
        need: Need,
        uses: Need,
        ticks: u64,
    }

    /// The tick each job started at (None: not within `ticks`). `outside(t)`: the physical memory
    /// the rest of the machine uses at tick t. `newest_first`: the order the waiting compiles
    /// look in within a tick (arbitrary on the real machine; newest first is the order that would
    /// let a rule's newcomer jump the line).
    fn simulate(jobs: &[SimJob], outside: &dyn Fn(u64) -> u64, policy: &Policy, ticks: u64, newest_first: bool) -> Vec<Option<u64>> {
        const TOTAL_PHYS: u64 = 32_768;
        let mut started: Vec<Option<u64>> = vec![None; jobs.len()];
        for t in 0..ticks {
            let mut order: Vec<usize> = (0..jobs.len()).filter(|&i| jobs[i].arrive <= t && started[i].is_none()).collect();
            if newest_first {
                order.reverse();
            }
            for &i in &order {
                let running: Vec<usize> = (0..jobs.len()).filter(|&j| matches!(started[j], Some(s) if t < s + jobs[j].ticks)).collect();
                let used = running.iter().fold(Need::default(), |a, &j| a.plus(jobs[j].uses));
                let growth = running.iter().fold(Need::default(), |a, &j| {
                    a.plus(Need { commit_mb: still_to_grow_mb(jobs[j].need.commit_mb, jobs[j].uses.commit_mb), phys_mb: still_to_grow_mb(jobs[j].need.phys_mb, jobs[j].uses.phys_mb) })
                });
                let mem = Mem {
                    avail_commit_mb: 100_000 - used.commit_mb,
                    avail_phys_mb: TOTAL_PHYS.saturating_sub(outside(t) + used.phys_mb),
                    commit_limit_mb: 132_000,
                    total_phys_mb: TOTAL_PHYS,
                };
                let line: Vec<InLine> = (0..jobs.len())
                    .filter(|&j| jobs[j].arrive <= t && started[j].is_none())
                    .map(|j| InLine { pid: j as u32 + 1, since_ms: jobs[j].arrive * 2000, need: jobs[j].need })
                    .collect();
                if decide(&mem, policy, running.len(), growth, &line, i as u32 + 1) != Verdict::Wait {
                    started[i] = Some(t);
                }
            }
        }
        started
    }

    /// The 2026-11-14 deadlock, in its own numbers (the slot log, pid 16076): a lib-test expected
    /// to peak at 14.4 GB (14,746 MB) needed 16.4 GB with the margin, the machine had 16.3 GB
    /// (16,691 MB) free with NONE of our compiles running, and 14 small compiles (lints and the
    /// workspace's small crates, 1.5 GB each) waited behind it. Each of them counted the head's
    /// 14.4 GB as promised, so nothing ran, and the only thing that could have freed the head's
    /// room was the head. It waited 17 minutes and then peaked at 10.7 GB.
    ///
    /// RED on the rule before 2026-11-14 (first come, first served with the full margin, the head
    /// counted against everyone behind it), seen running this test that day:
    ///   "after 300 ticks (10 min) 0 of 15 compiles had started: [None, None, ... None]"
    #[test]
    fn the_queue_never_waits_on_room_only_its_own_head_could_free() {
        let mut jobs = vec![SimJob { arrive: 0, need: Need::both(14_746), uses: Need { commit_mb: 10_719, phys_mb: 8_885 }, ticks: 172 }];
        for k in 1..=14 {
            jobs.push(SimJob { arrive: k, need: Need::both(1_536), uses: Need::both(110), ticks: 2 });
        }
        let outside = |_t: u64| 32_768 - 16_691;
        for newest_first in [true, false] {
            let s = simulate(&jobs, &outside, &P, 300, newest_first);
            let n = s.iter().filter(|x| x.is_some()).count();
            assert_eq!(n, jobs.len(), "after 300 ticks (10 min) {n} of {} compiles had started: {s:?}", jobs.len());
            // The head went first, and at once: nothing of ours was running.
            assert_eq!(s[0], Some(0), "{s:?}");
        }
    }

    /// When the head cannot start even on an idle machine (the rest of the machine holds the
    /// memory), the compiles behind it go one at a time instead of waiting with it; the moment
    /// the machine has room for the head, it goes next: before any compile that starts after
    /// that moment, however many keep arriving behind it.
    ///
    /// RED on the rule before 2026-11-14: "small compile 1 (arrived at tick 1) never started: it
    /// waited behind a head that could not start" (nothing behind the head ever ran).
    #[test]
    fn behind_a_head_that_cannot_start_the_line_moves_and_the_head_goes_next() {
        let head = SimJob { arrive: 0, need: Need::both(14_746), uses: Need { commit_mb: 10_719, phys_mb: 8_885 }, ticks: 172 };
        let mut jobs = vec![head];
        // A small compile every 3 ticks for 20 minutes.
        for k in 0..200u64 {
            jobs.push(SimJob { arrive: 1 + 3 * k, need: Need::both(1_536), uses: Need::both(110), ticks: 2 });
        }
        // 13 GB free on an idle machine until tick 300, then 16 GB: room for the head only with
        // nothing of ours running.
        let outside = |t: u64| if t < 300 { 32_768 - 13_000 } else { 32_768 - 16_000 };
        for newest_first in [true, false] {
            let s = simulate(&jobs, &outside, &P, 1_200, newest_first);
            for (i, j) in jobs.iter().enumerate().skip(1) {
                if j.arrive < 290 {
                    let st = s[i].unwrap_or_else(|| panic!("small compile {i} (arrived at tick {}) never started: it waited behind a head that could not start", j.arrive));
                    assert!(st <= j.arrive + 10, "small compile {i} arrived at {} and started at {st}: {s:?}", j.arrive);
                }
            }
            let h = s[0].expect("the head started once the machine had room for it");
            assert!((300..=303).contains(&h), "the head went at tick {h}, not as soon as the compile running at tick 300 finished");
            let jumped: Vec<usize> = (1..jobs.len()).filter(|&i| matches!(s[i], Some(st) if st > 300 && st < h)).collect();
            assert!(jumped.is_empty(), "compiles {jumped:?} started after the room appeared and before the head (tick {h})");
            assert!(s.iter().all(|x| x.is_some()), "every compile ran in the end: {s:?}");
        }
    }

    #[test]
    fn seeds_cover_every_kind() {
        assert_eq!(seed_peak_mb("lib-test-native"), 11_000);
        assert!(seed_peak_mb("lib-test-relay") < seed_peak_mb("lib-test-native"));
        assert!(seed_peak_mb("buildscript-build-none") <= 1_000);
        assert!(seed_peak_mb("nonsense") > 0);
    }
}
