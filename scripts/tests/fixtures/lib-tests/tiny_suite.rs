// A tiny test program standing in for the library's (scripts/tests/lib-tests-policy.test.js).
//   TINY_FLAKE_MARKER=<file>  flaky_once fails the first time (it creates the file), then passes
//   TINY_REAL_FAIL=1          always_fails fails every time
//   TINY_REAL_FAIL_IN=<text>  always_fails fails only in a program whose path holds <text> (one workspace
//                             member's program of several, 2026-11-01)
//   TINY_RUN_LOG=<file>       every run appends one line, so a test can see whether it ran
//   TINY_SILENT_HANG=1        hangs_silently_when_asked reports its result, then its thread never exits
//   HUMANITY_TEST_GPU_BUSY    set by lib-tests.js beside the operator's GPU game: gpu_device_test stands
//                             down and writes its name to HUMANITY_TEST_GPU_SKIPPED, as crates/humanity-core/src/test_gpu.rs does
//   TINY_INPUT=<file>         reads_its_input fails while the file holds a single-brace "{name}" (2026-11-21)
//   TINY_FIX_INPUT=1          ...and corrects the file to "{{name}}" as it fails: the word fixed mid-run
//   TINY_TOUCH=<file>         touches_a_file_when_asked gives the file a new modified time (a relinked program)
//   TINY_TOUCH_IN=<text>      ...only in a program whose path holds <text>
//   TINY_APPEND=<file>        touches_a_file_when_asked adds a line to the file (an edit made mid-run)
//   ignored_unless_asked      is #[ignore]d: a filter that matches only it runs nothing
//   TINY_EXIT_MARKER=<file>   exits_the_program_once_it_has_failed fails once, then ends the program with
//                             exit 0 before libtest reports it

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
    let here = std::env::var("TINY_REAL_FAIL_IN")
        .map_or(false, |s| std::env::current_exe().map_or(false, |p| p.to_string_lossy().contains(s.as_str())));
    if std::env::var("TINY_REAL_FAIL").as_deref() == Ok("1") || here {
        panic!("a real failure: 2 + 2 is not 5");
    }
}

/// A DETERMINISTIC failure that reads its input at run time, the way
/// i18n::tests::every_placeholder_is_double_braced reads data/i18n/en.json. On 2026-11-20 that test
/// failed on a `{name}` placeholder, its word was corrected while the suite was still running, the
/// rerun alone read the corrected file and passed, and lib-tests logged a real failure FLAKY.
/// TINY_FIX_INPUT=1 is that correction, made at the moment of failing so the timing is certain.
#[test]
fn reads_its_input() {
    note("reads_its_input");
    if let Ok(p) = std::env::var("TINY_INPUT") {
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        if text.contains("{name}") && !text.contains("{{name}}") {
            if std::env::var("TINY_FIX_INPUT").as_deref() == Ok("1") {
                std::fs::write(&p, "Hello {{name}}\n").unwrap();
            }
            panic!("words with a single-brace placeholder (write {{{{name}}}}): {text:?}");
        }
    }
}

/// Gives a file a new modified time from inside the run: another cargo command relinking a test
/// program while the suite runs, so the rerun would run a different build than the one that failed.
#[test]
fn touches_a_file_when_asked() {
    let here = std::env::var("TINY_TOUCH_IN")
        .map_or(true, |s| std::env::current_exe().map_or(false, |p| p.to_string_lossy().contains(s.as_str())));
    if let (Ok(p), true) = (std::env::var("TINY_TOUCH"), here) {
        let f = OpenOptions::new().write(true).open(&p).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
    }
    // TINY_APPEND=<file>: a line added to the file, as someone editing the checkout mid-run does.
    if let Ok(p) = std::env::var("TINY_APPEND") {
        let mut f = OpenOptions::new().create(true).append(true).open(&p).unwrap();
        let _ = f.write_all(b"edited while the suite ran\n");
    }
}

/// TINY_EXIT_MARKER=<file>: fails the first time (creating the file), and the next time ends the
/// whole program with exit code 0 before libtest can report it, the way code under test that calls
/// std::process::exit(0) would. Its rerun alone printed "running 1 test" and exited 0, which the
/// check before 2026-11-21 took for a pass: no test was ever reported as passing.
#[test]
fn exits_the_program_once_it_has_failed() {
    note("exits_the_program_once_it_has_failed");
    if let Ok(p) = std::env::var("TINY_EXIT_MARKER") {
        if std::path::Path::new(&p).exists() {
            std::process::exit(0);
        }
        std::fs::write(&p, "failed once").unwrap();
        panic!("the first time it fails; the next time it ends the program");
    }
}

/// An ignored test: a filter matching only it runs nothing unless --ignored is given.
#[test]
#[ignore]
fn ignored_unless_asked() {
    note("ignored_unless_asked");
}

/// Stands in for a test that opens a GPU device: it asks first, the way crates/humanity-core/src/test_gpu.rs answers.
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
        // As src/renderer/test_gpu.rs does for a test holding the GPU (2026-11-14): the holder in
        // the file lib-tests.js names, and the watchdog's line straight to stderr.
        if let (Ok(holder), Ok(file)) = (std::env::var("TINY_GPU_TURN_HOLDER"), std::env::var("HUMANITY_TEST_GPU_TURN")) {
            use std::io::Write;
            let _ = std::fs::write(file, format!("1763100000000 held {holder}\n"));
            let _ = std::io::stderr().write_all(format!("GPU TEST STILL RUNNING: {holder} has held the GPU for 60 s (src/renderer/test_gpu.rs)\n").as_bytes());
        }
        NEVER.with(|_| {});
    }
}
