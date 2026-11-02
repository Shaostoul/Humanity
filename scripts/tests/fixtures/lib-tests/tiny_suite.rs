// A tiny test program standing in for the library's (scripts/tests/lib-tests-policy.test.js).
//   TINY_FLAKE_MARKER=<file>  flaky_once fails the first time (it creates the file), then passes
//   TINY_REAL_FAIL=1          always_fails fails every time
//   TINY_RUN_LOG=<file>       every run appends one line, so a test can see whether it ran
//   TINY_SILENT_HANG=1        hangs_silently_when_asked reports its result, then its thread never exits
//   HUMANITY_TEST_GPU_BUSY    set by lib-tests.js beside the operator's GPU game: gpu_device_test stands
//                             down and writes its name to HUMANITY_TEST_GPU_SKIPPED, as src/test_gpu.rs does

use std::fs::OpenOptions;
use std::io::Write;

fn note(what: &str) {
    if let Ok(p) = std::env::var("TINY_RUN_LOG") {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
            // one write call per line, so tests running at once cannot interleave their lines
            let _ = f.write_all(format!("{what}\n").as_bytes());
        }
    }
}

#[test]
fn always_ok() {
    note("always_ok");
    assert_eq!(2 + 2, 4);
}

#[test]
fn flaky_once() {
    note("flaky_once");
    if let Ok(p) = std::env::var("TINY_FLAKE_MARKER") {
        if !std::path::Path::new(&p).exists() {
            std::fs::write(&p, "failed once").unwrap();
            panic!("timing: the neighbour test held the port (a made-up flake)");
        }
    }
}

#[test]
fn always_fails() {
    note("always_fails");
    if std::env::var("TINY_REAL_FAIL").as_deref() == Ok("1") {
        panic!("a real failure: 2 + 2 is not 5");
    }
}

/// Stands in for a test that opens a GPU device: it asks first, the way src/test_gpu.rs answers.
#[test]
fn gpu_device_test() {
    let busy = std::env::var("HUMANITY_TEST_GPU_BUSY").unwrap_or_default();
    if !busy.trim().is_empty() {
        if let Ok(p) = std::env::var("HUMANITY_TEST_GPU_SKIPPED") {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
                let _ = f.write_all(b"gpu_device_test\n");
            }
        }
        return;
    }
    note("gpu_device_test");
}

/// The 2026-11-01 hang, made on purpose: the test reports its result, then its thread cannot exit
/// (a thread-local whose destructor never returns, where the GPU driver's per-thread cleanup most
/// likely stuck). libtest joins a finished test's thread before it prints anything more, so the
/// whole run goes silent, its 60-second warnings included.
#[test]
fn hangs_silently_when_asked() {
    struct Never;
    impl Drop for Never {
        fn drop(&mut self) {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
    }
    thread_local! {
        static NEVER: Never = const { Never };
    }
    if std::env::var("TINY_SILENT_HANG").as_deref() == Ok("1") {
        NEVER.with(|_| {});
    }
}
