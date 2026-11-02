//! May a test use the GPU right now? (2026-11-01.) Test builds only: the product cannot reach it.
//!
//! A few tests open a real GPU device (the present pass's round trips in
//! renderer/scene_target_tests.rs run in every full run; the rest are `#[ignore]`d and run
//! on purpose). While one of the operator's GPU-heavy games runs (scripts/lib/operator-apps.json,
//! or his own HumanityOS), such a test can hang the WHOLE test run, silently: on 2026-11-01 the
//! full lib tests froze at 3,852 of 5,584 for two hours beside helldivers2.exe, burning CPU, with
//! no "has been running for over 60 seconds" line, and passed in 232 s once the game closed. The
//! silence is libtest's: its main loop joins each finished test's thread BEFORE it prints that
//! test's result, and its 60-second warnings are printed from inside the same loop, so one test
//! thread that reports its result and then cannot exit (most likely the GPU driver's per-thread
//! cleanup, which Windows runs as the thread exits; nothing was dumped to prove it) stops every
//! line after it.
//!
//! So scripts/lib-tests.js looks for those programs before it runs the tests and, when one runs,
//! sets `HUMANITY_TEST_GPU_BUSY` to their names. A test that would open a device asks `may_use()`
//! FIRST, before it creates a `wgpu::Instance` (creating the instance already loads the driver),
//! and returns early when the answer is no. Each test that stood down is written, one name a
//! line, to the file `HUMANITY_TEST_GPU_SKIPPED` names, and lib-tests.js reads it back and says
//! loudly which tests did nothing and why, and does not count the run as a full pass of them:
//! libtest itself counts a test that returned early as passed. tests/gpu_test_gate_lint.rs fails
//! when a new instance is created in a test without asking first.
//!
//! Run outside lib-tests.js (a bare test program, CI, `cargo test`), neither variable is set and
//! every test may use the GPU, as before.

use std::io::Write;

/// The variable lib-tests.js sets to the names of the operator's GPU programs that are running.
pub(crate) const BUSY_VAR: &str = "HUMANITY_TEST_GPU_BUSY";
/// The variable naming the file each test that stood down appends its name to.
pub(crate) const SKIPPED_VAR: &str = "HUMANITY_TEST_GPU_SKIPPED";

/// May this test open a GPU device now? Call it before creating a `wgpu::Instance`; on `false`,
/// return without touching the GPU (the reason is already recorded and printed).
pub(crate) fn may_use() -> bool {
    let busy = std::env::var(BUSY_VAR).ok();
    let log = std::env::var(SKIPPED_VAR).ok();
    let thread = std::thread::current();
    // libtest names each test's thread after the test.
    let test = thread.name().unwrap_or("(a test on an unnamed thread)");
    decide(busy.as_deref(), log.as_deref().map(std::path::Path::new), test)
}

/// `may_use`, with what it reads from the environment handed in, so a test can try it without
/// setting a variable every test in the program would see.
fn decide(busy: Option<&str>, log: Option<&std::path::Path>, test: &str) -> bool {
    let Some(busy) = busy.map(str::trim).filter(|b| !b.is_empty()) else {
        return true;
    };
    if let Some(log) = log {
        // One write per line, so tests standing down at the same moment cannot interleave.
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = f.write_all(format!("{test}\n").as_bytes());
        }
    }
    println!("{test}: not run on the GPU, {busy} is using it (scripts/lib-tests.js, src/test_gpu.rs)");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_busy_program_means_the_gpu_may_be_used_and_nothing_is_written() {
        let dir = crate::test_temp::dir("test_gpu_free");
        let log = dir.path().join("skipped.txt");
        assert!(decide(None, Some(&log), "a::b"));
        assert!(decide(Some(""), Some(&log), "a::b"));
        assert!(decide(Some("  "), Some(&log), "a::b"));
        assert!(!log.exists(), "a test that ran must not be listed as skipped");
    }

    #[test]
    fn a_busy_program_stands_the_test_down_and_names_it_once_per_ask() {
        let dir = crate::test_temp::dir("test_gpu_busy");
        let log = dir.path().join("skipped.txt");
        assert!(!decide(Some("helldivers2.exe"), Some(&log), "renderer::x::tests::one"));
        assert!(!decide(Some("helldivers2.exe"), Some(&log), "renderer::x::tests::two"));
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text, "renderer::x::tests::one\nrenderer::x::tests::two\n");
        // Without a file to write to it still stands down.
        assert!(!decide(Some("nms.exe"), None, "renderer::x::tests::three"));
    }
}
