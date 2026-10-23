// A tiny test program standing in for the library's (scripts/tests/lib-tests-policy.test.js).
//   TINY_FLAKE_MARKER=<file>  flaky_once fails the first time (it creates the file), then passes
//   TINY_REAL_FAIL=1          always_fails fails every time
//   TINY_RUN_LOG=<file>       every run appends one line, so a test can see whether it ran

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
