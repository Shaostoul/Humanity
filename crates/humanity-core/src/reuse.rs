//! The one "turn off all reuse" switch (CLAUDE.md, the reuse rule, refined 2026-10-30).
//!
//! The operator's rule: nothing kept across runs during development, and data kept in memory
//! across frames only when it cannot go stale (rebuilt on a schedule, or tagged with a version its
//! source bumps), "and every one of them obeys a single 'turn off all reuse' switch in the Dev page.
//! When a bug smells like a cache, one click rules caches in or out." (the proposal he took, "I like
//! this idea, let's do it."). Every reuse in the game asks `enabled()` and, when it is false, works
//! everything out again instead: on every frame, or, where every frame would be far over a frame
//! (the home's rebuild measured 301 to 637 ms), on a short SCHEDULE, so nothing is older than the
//! schedule: the rule's other allowed form. A new reuse that does not ask is a bug.
//!
//! THE PATTERN. The keep-or-build decision is a pure function that takes `reuse: bool` (`keeps`
//! below for a reuse tagged with a version, or the site's own, such as
//! `engine::home_reuse::home_rebuild_due`), called with `enabled()` at the site, so a test drives
//! both arms without flipping the process-wide switch for every test.
//!
//! WHERE THE SITES ARE. Each reuse declares itself where it lives, with a comment on the field,
//! static or function that keeps it: `// reuse: site: <what the switch does here>`. A reuse-shaped
//! name that is not a reuse (a dirty flag that only asks for a full rebuild, a counter, data read
//! once per run) says why in any other `// reuse: <reason>` comment. The list of every site is
//! `grep -rn "reuse: site:" src crates`, and `tests/reuse_switch_lint.rs` (in `just lints`) fails a reuse-shaped field or static that
//! declares neither. (Until 2026-11-13 the sites were listed in this paragraph, and every agent
//! that added one appended here: it conflicted in two merges in one day. The lint now fails a site
//! list written back into this file.)
//!
//! The switch is the Dev tab's Performance section ("Turn off all reuse"), for this run only, and
//! `HUMANITY_NO_REUSE=1` for a rig or a scripted run.

use std::sync::atomic::{AtomicU8, Ordering};

const UNREAD: u8 = 0;
const ON: u8 = 1;
const OFF: u8 = 2;

/// Unread until the first question, when `HUMANITY_NO_REUSE` is read once.
static STATE: AtomicU8 = AtomicU8::new(UNREAD);

/// May a reuse keep what it worked out earlier? True unless the switch is off.
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => {
            let off = std::env::var_os("HUMANITY_NO_REUSE").is_some_and(|v| v == "1");
            if off {
                log::info!("Reuse: OFF for this run (HUMANITY_NO_REUSE=1): everything is worked out again every frame");
            }
            STATE.store(if off { OFF } else { ON }, Ordering::Relaxed);
            !off
        }
    }
}

/// May a reuse TAGGED WITH A VERSION keep what it built for `have` now that its source says
/// `want`? Only while the two match exactly and reuse is on. `reuse` is `enabled()` at the call
/// site (`keeps(self.key, key, reuse::enabled())`); it is a parameter so a test can drive both
/// arms without flipping the switch for every test in the process.
pub fn keeps<K: PartialEq>(have: Option<K>, want: K, reuse: bool) -> bool {
    reuse && have == Some(want)
}

/// Turn every reuse on or off, from the Dev page.
pub fn set_enabled(on: bool) {
    STATE.store(if on { ON } else { OFF }, Ordering::Relaxed);
    log::info!(
        "Reuse: {} (Dev > Performance)",
        if on { "ON: reused data is kept between frames again" } else { "OFF: everything is worked out again every frame" }
    );
}

#[cfg(test)]
mod tests {
    /// The switch flips both ways and every reader sees it at once.
    #[test]
    fn the_switch_turns_every_reuse_off_and_on() {
        super::set_enabled(false);
        assert!(!super::enabled());
        super::set_enabled(true);
        assert!(super::enabled());
    }

    /// A version-tagged reuse keeps what it built only for the same version and only with reuse
    /// on; with reuse off it builds again even for the version it has (so the check can fail).
    /// Seen red 2026-11-13 with `keeps` ignoring `reuse`: "reuse off: built again for the same
    /// version".
    #[test]
    fn with_reuse_off_a_versioned_reuse_builds_again_for_the_same_version() {
        assert!(!super::keeps(Some(7u32), 7, false), "reuse off: built again for the same version");
        assert!(super::keeps(Some(7u32), 7, true), "reuse on: kept for the same version");
        assert!(!super::keeps(Some(6u32), 7, true), "a new version: built again");
        assert!(!super::keeps(None, 7u32, true), "nothing built yet: built");
    }
}
