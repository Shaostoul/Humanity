//! The machine-wide boot ledger, the game's own half (2026-11-14).
//!
//! Every boot a developer's script starts appends ONE line to
//! `%LOCALAPPDATA%\HumanityOS\dev\boots.jsonl`, outside every repository, so the orchestrator can
//! see repeated or wasted boots across every worktree (`node scripts/boot-ledger.js` prints the
//! last day). The rigs' launcher writes the line for the boots it starts (scripts/lib/
//! game-launch.js spawnGame, and the dev MCP's game_boot through it) and tells the game so with
//! HUMANITY_BOOT_LEDGER; this half covers a quiet boot no launcher recorded (`just launch-bg`, a
//! script that starts the exe with HUMANITY_NO_FOCUS itself), written when the game exits.
//!
//! It is a LOG: the game appends to it and never reads it back, so it is not a cache (CLAUDE.md's
//! no-caches rule). A person's own launch (foreground, or a background launch nobody asked to be
//! quiet) writes nothing: the ledger counts developers' boots, not anyone's play.
//!
//! One line's fields, shared with scripts/lib/boot-ledger.js (its test pins the names):
//! t, end (UTC, ISO 8601), duration_s, checkout, worktree, script, rig, via, purpose, pid, exe,
//! hidden, low_impact, ended, writer.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Set by a launcher that writes this boot's line itself (its name); the game then writes none.
pub const LEDGER_OWNER_ENV: &str = "HUMANITY_BOOT_LEDGER";
/// A test's or a rig's own ledger file instead of the machine-wide one.
pub const LEDGER_FILE_ENV: &str = "HUMANITY_BOOT_LEDGER_FILE";
/// What a launcher says the boot is for (shown by scripts/boot-ledger.js).
pub const PURPOSE_ENV: &str = "HUMANITY_BOOT_PURPOSE";
/// The gate or tool that ran the script that started the boot (verify-runtime, the dev MCP).
pub const VIA_ENV: &str = "HUMANITY_BOOT_VIA";

/// `%LOCALAPPDATA%\HumanityOS\dev\boots.jsonl` (or HUMANITY_BOOT_LEDGER_FILE).
pub fn ledger_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(LEDGER_FILE_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("HumanityOS").join("dev").join("boots.jsonl"))
}

/// The short name the report groups by: the worktree's folder under `.claude\worktrees`, else the
/// checkout's own folder name (scripts/lib/boot-ledger.js worktreeName, the same rule).
pub fn worktree_name(checkout: &Path) -> String {
    let parts: Vec<String> = checkout.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    for w in parts.windows(3) {
        if w[0].eq_ignore_ascii_case(".claude") && w[1].eq_ignore_ascii_case("worktrees") {
            return w[2].clone();
        }
    }
    parts.last().cloned().unwrap_or_default()
}

/// The checkout an exe was started from: the nearest folder above it with a `.git` (a worktree's is
/// a file, the main checkout's a folder). None for an installed copy.
pub fn checkout_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().skip(1).find(|d| d.join(".git").exists()).map(Path::to_path_buf)
}

/// Unix seconds as ISO 8601 UTC ("2026-11-14T09:30:05Z"), with no date library.
pub fn iso_utc(unix_s: u64) -> String {
    let days = (unix_s / 86_400) as i64;
    let rem = unix_s % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Append one record. Never fails the caller: a ledger that cannot be written is logged and skipped.
pub fn append(path: &Path, record: &serde_json::Value) {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let line = format!("{record}\n");
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(line.as_bytes()) {
                log::warn!("boot ledger: could not write {}: {e}", path.display());
            }
        }
        Err(e) => log::warn!("boot ledger: could not open {}: {e}", path.display()),
    }
}

/// Held across `humanity_engine::run()` by src/main.rs: on drop (the game's exit, or a panic's
/// unwind) it writes this boot's line, if this boot is one the game records itself.
pub struct EngineBoot {
    t0: Instant,
    started_unix: u64,
}

impl EngineBoot {
    /// Note the start. Reads nothing that logs: the launch decision is made (and logged) by the game.
    pub fn start() -> Self {
        EngineBoot { t0: Instant::now(), started_unix: unix_now() }
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Whether the game writes this boot's line: a developer's quiet background boot that no launcher
/// already records. Pure, so it is tested.
pub fn engine_writes(background: bool, quiet_asked: bool, owner: Option<&str>) -> bool {
    background && quiet_asked && owner.map_or(true, |o| o.is_empty())
}

impl Drop for EngineBoot {
    fn drop(&mut self) {
        let owner = std::env::var(LEDGER_OWNER_ENV).ok();
        if !engine_writes(
            crate::engine::launch_focus::launch_in_background(),
            crate::engine::launch_focus::quiet_asked(),
            owner.as_deref(),
        ) {
            return;
        }
        let Some(path) = ledger_path() else { return };
        let exe = std::env::current_exe().ok();
        let checkout = exe.as_deref().and_then(checkout_of);
        let ended = if std::thread::panicking() { "panicked" } else { "closed" };
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let record = serde_json::json!({
            "t": iso_utc(self.started_unix),
            "end": iso_utc(unix_now()),
            "duration_s": (self.t0.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            "checkout": checkout.as_ref().map(|c| c.display().to_string()),
            "worktree": checkout.as_deref().map(worktree_name),
            "script": env("HUMANITY_BOOT_SCRIPT"),
            "rig": serde_json::Value::Null,
            "via": env(VIA_ENV),
            "purpose": env(PURPOSE_ENV),
            "pid": std::process::id(),
            "exe": exe.as_ref().map(|e| e.display().to_string()),
            "hidden": crate::engine::rig_window::hide_from_screen(),
            "low_impact": env("HUMANITY_LOW_IMPACT"),
            "ended": ended,
            "writer": "engine",
        });
        append(&path, &record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_writes_only_unrecorded_developer_boots() {
        assert!(engine_writes(true, true, None), "`just launch-bg`: quiet, background, no launcher");
        assert!(engine_writes(true, true, Some("")));
        assert!(!engine_writes(true, true, Some("spawnGame")), "a rig's launcher already writes it");
        assert!(!engine_writes(false, true, None), "a human launch writes nothing");
        assert!(!engine_writes(true, false, None), "a person's background launch writes nothing");
    }

    #[test]
    fn names_and_times() {
        assert_eq!(worktree_name(Path::new(r"C:\Humanity\.claude\worktrees\agent-abc")), "agent-abc");
        assert_eq!(worktree_name(Path::new(r"C:\Humanity")), "Humanity");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        // 2026-11-14T09:30:05Z, from `date -u -d @1794648605`.
        assert_eq!(iso_utc(1_794_648_605), "2026-11-14T09:30:05Z");
        // A leap day.
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    }
}
