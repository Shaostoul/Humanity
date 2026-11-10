//! The client against the real engine program in its fake mode (`humanity-ai --fake`, 12.1): started
//! on pipes in its job object, answering, crashing, refused, capped, idle, ended with the app.
//!
//! THE ENGINE IS BUILT HERE when it is missing or stale: one `cargo build` of
//! crates/humanity-ai-engine/ into target/ai-engine/debug/ per test run (it is not a member of the
//! app's workspace, so the app's own build never makes it). If that build fails, every test here FAILS
//! and says so in words; none passes on a missing engine.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use humanity_ai_protocol::{ErrorCode, ModelKind, ProtocolError, PROTOCOL};

use super::client::{AiClient, ClientConfig, ExitWhy, Fallback, Outcome, State};

/// The fake engine, built for this test run (or why it could not be).
pub(crate) fn fake_engine() -> PathBuf {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        let root = Path::new(env!("HUMANITY_REPO_ROOT"));
        let target = root.join("target").join("ai-engine");
        // `cargo` through rustup's proxy, run from the repo root, the way `just ai-engine` builds it:
        // the proxy reads rust-toolchain.toml there and hands the pinned toolchain to every compile
        // (RUSTUP_TOOLCHAIN). The toolchain's own cargo.exe (option_env!("CARGO")) skipped that, and
        // cargo compiles a registry crate from its own folder outside the repo, where rustup falls back
        // to the machine's default: serde came out of rustc 1.88 and our crates out of the pinned
        // 1.99.1, E0514, and every client test failed on main (2026-11-09, at the merge).
        let cargo = "cargo";
        let out = std::process::Command::new(cargo)
            .current_dir(root)
            .arg("build")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(root.join("crates").join("humanity-ai-engine").join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target)
            .output()
            .map_err(|e| format!("could not run cargo ({cargo}): {e}"))?;
        let exe = target.join("debug").join(super::paths::ENGINE_EXE);
        if !out.status.success() || !exe.is_file() {
            return Err(format!(
                "THE FAKE AI ENGINE IS NOT BUILT, and building it failed, so the client cannot be tested (run `just ai-engine` to see why):\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(exe)
    });
    match built {
        Ok(p) => p.clone(),
        Err(e) => panic!("{e}"),
    }
}

/// A client for the fake engine with these extra arguments (fast timing unless they say otherwise).
fn client(args: &[&str]) -> AiClient {
    let mut all = vec!["--fake".to_string(), "--fake-ms-prefill".into(), "10".into(), "--fake-ms-per-token".into(), "2".into()];
    all.extend(args.iter().map(|s| s.to_string()));
    AiClient::new(Some(fake_engine()), ClientConfig { args: all, ..ClientConfig::default() })
}

/// Poll until `pred` holds, or fail naming what was awaited and the state.
fn until(c: &mut AiClient, what: &str, pred: impl Fn(&AiClient) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        c.poll();
        if pred(c) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("waited 20 s for {what}; the state is {:?}", c.state());
}

fn running(c: &AiClient) -> bool {
    matches!(c.state(), State::Running { .. })
}

/// Load the fake's model and ask; the request's number.
fn ask(c: &mut AiClient) -> u64 {
    c.load("fake", "(fake)", "", ModelKind::Brain, 4096, 1);
    c.generate("fake", "You are the test persona.", "What is this place?", 64)
}

/// Poll until the request ends; its outcome.
fn outcome_of(c: &mut AiClient, request: u64) -> Outcome {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        c.poll();
        seen.extend(c.take_outcomes());
        if let Some(o) = seen.iter().find(|o| o.request() == request) {
            return o.clone();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("request {request} never ended; the state is {:?}", c.state());
}

/// The fake engine starts on demand (a request starts it), says hello as the fake, and its reply comes
/// whole, with its timing; the client counts it.
#[test]
fn a_request_starts_the_engine_and_is_answered() {
    let mut c = client(&[]);
    assert_eq!(c.state(), &State::Stopped, "nothing runs until something is asked");
    assert!(c.pid().is_none());
    let r = ask(&mut c);
    match outcome_of(&mut c, r) {
        Outcome::Answered { text, tokens, .. } => {
            assert!(text.starts_with("A real engine would answer"), "{text}");
            assert!(tokens > 5);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(c.state(), State::Running { hello, .. } if hello.fake && hello.version == super::paths::ENGINE_VERSION), "{:?}", c.state());
    assert_eq!((c.counters.answered, c.counters.fallbacks, c.failures()), (1, 0, 0));
}

/// The engine runs inside its job object at below-normal priority (7.2, 9.2). Seen red 2026-11-09 with
/// the `assign` call taken out of `spawn`: "(false, 16384)", in no job though the priority was right.
#[cfg(windows)]
#[test]
fn the_engine_runs_in_its_job_at_below_normal_priority() {
    let mut c = client(&[]);
    c.start();
    until(&mut c, "the hello", running);
    assert_eq!(c.containment(), Some((true, super::job::BELOW_NORMAL)), "in the job, below normal");
    let (now, peak) = c.memory_mb();
    assert!(now.is_some() && peak.is_some(), "its memory is readable: {now:?} {peak:?}");
}

/// When the job's handle closes, as it does when the app exits or crashes, the engine ends (kill on
/// close): nothing is left running behind the app. Seen red 2026-11-09 with KILL_ON_JOB_CLOSE left out
/// of the limits: "the engine outlived the job".
#[cfg(windows)]
#[test]
fn the_engine_ends_with_the_app() {
    let mut c = client(&[]);
    c.start();
    until(&mut c, "the hello", running);
    c.drop_job_for_test();
    assert!(c.process_ended_within(Duration::from_secs(5)), "the engine outlived the job");
}

/// A load past the job's memory cap is refused in words (out_of_memory), and the engine goes on; under
/// a higher cap the same load succeeds. Seen red 2026-11-09 with the cap left out of the limits (the
/// 300 MB load under a 64 MB cap succeeded): "waited 20 s for the refusal; the state is Running { .. }".
#[cfg(windows)]
#[test]
fn the_memory_cap_holds() {
    let mut c = client(&["--fake-load-mb", "300"]);
    c.config.memory_cap_mb = 64;
    c.load("fake", "(fake)", "", ModelKind::Brain, 4096, 1);
    until(&mut c, "the refusal", |c| c.last_error.is_some());
    assert_eq!(c.last_error.as_ref().map(|(code, _)| *code), Some(ErrorCode::OutOfMemory));
    assert!(!c.is_loaded("fake"));
    assert!(running(&c), "a refused load is not a crash: {:?}", c.state());

    let mut roomy = client(&["--fake-load-mb", "300"]);
    roomy.config.memory_cap_mb = 1024;
    roomy.load("fake", "(fake)", "", ModelKind::Brain, 4096, 1);
    until(&mut roomy, "the load", |c| c.is_loaded("fake"));
}

/// A crash while a request is held falls back AT ONCE; the next request starts the engine again; a
/// second failure in a row stops automatic restarts and says so; requests then fall back without
/// starting anything; Try again clears the count and starts it (7.2). Seen red 2026-11-09
/// with `fail` choosing Crashed every time: "two in a row: Crashed { why: Code(1) }".
#[test]
fn a_crash_falls_back_at_once_and_two_in_a_row_stop_restarts() {
    let mut c = client(&["--fake-ms-per-token", "300"]);
    let r1 = ask(&mut c);
    until(&mut c, "the reply under way", |c| c.partial(r1).is_some_and(|t| !t.is_empty()));
    c.kill_as_crash();
    assert_eq!(outcome_of(&mut c, r1), Outcome::Fallback { request: r1, why: Fallback::Crashed });
    assert!(matches!(c.state(), State::Crashed { .. }), "{:?}", c.state());
    assert_eq!(c.failures(), 1);

    let r2 = ask(&mut c);
    until(&mut c, "the second engine", running);
    c.kill_as_crash();
    assert_eq!(outcome_of(&mut c, r2), Outcome::Fallback { request: r2, why: Fallback::Crashed });
    assert!(matches!(c.state(), State::Halted { .. }), "two in a row: {:?}", c.state());
    assert_eq!(c.state().words_key(), "ai.state.halted");

    let r3 = c.generate("fake", "p", "q", 8);
    assert_eq!(c.take_outcomes(), vec![Outcome::Fallback { request: r3, why: Fallback::Halted }], "at once, without starting anything");
    assert!(c.pid().is_none());

    c.try_again();
    until(&mut c, "the engine after Try again", running);
    assert_eq!(c.failures(), 0);
    assert_eq!(c.counters.starts, 3, "two automatic starts and Try again's, no more");
}

/// An engine that dies on a message (here, every reply) is a crash with its exit code.
#[test]
fn an_engine_that_exits_is_a_crash_with_its_code() {
    let mut c = client(&["--fake-exit-on", "generate"]);
    let r = ask(&mut c);
    assert_eq!(outcome_of(&mut c, r), Outcome::Fallback { request: r, why: Fallback::Crashed });
    assert_eq!(c.state(), &State::Crashed { why: ExitWhy::Code(3) });
}

/// An engine speaking another protocol version is refused with words and ended, and never used until
/// Try again (7.3). Seen red 2026-11-09 with `on_unreadable` only logging: the client waited for a
/// hello it could never read, "request 1 never ended; the state is Starting { .. }".
#[test]
fn an_engine_of_another_protocol_is_refused_in_words() {
    let mut c = client(&["--fake-protocol", "2"]);
    c.config.hello_timeout = Duration::from_secs(30);
    let r = c.generate("fake", "p", "q", 8);
    assert_eq!(outcome_of(&mut c, r), Outcome::Fallback { request: r, why: Fallback::Refused });
    assert_eq!(c.state(), &State::Refused { error: ProtocolError::Version { theirs: 2, ours: PROTOCOL } });
    assert!(c.pid().is_none(), "ended");
    let again = c.generate("fake", "p", "q", 8);
    assert_eq!(c.take_outcomes(), vec![Outcome::Fallback { request: again, why: Fallback::Refused }]);
    assert!(c.pid().is_none(), "not started again on its own");
}

/// Stopping is not a failure, and an idle engine stops itself, also not a failure.
#[test]
fn stopping_and_idling_are_not_failures() {
    let mut c = client(&[]);
    c.start();
    until(&mut c, "the hello", running);
    c.stop();
    until(&mut c, "the stop", |c| c.state() == &State::Stopped);
    assert_eq!(c.failures(), 0);

    let mut idle = client(&[]);
    idle.config.idle_stop = Duration::from_millis(300);
    idle.start();
    until(&mut idle, "the hello", running);
    until(&mut idle, "the idle stop", |c| c.state() == &State::Stopped);
    assert_eq!((idle.failures(), idle.counters.crashes), (0, 0));
}

/// A request asked while the engine is stopping is kept, and a fresh engine answers it once the old one
/// is gone; it never falls back for the stop. Seen red 2026-11-09 with requests made while stopping
/// falling back at once: "answered after the restart: Fallback { request: 1, why: Crashed }".
#[test]
fn a_request_while_stopping_is_answered_by_a_fresh_engine() {
    let mut c = client(&[]);
    c.start();
    until(&mut c, "the hello", running);
    c.stop();
    let r = ask(&mut c);
    let o = outcome_of(&mut c, r);
    assert!(matches!(o, Outcome::Answered { .. }), "answered after the restart: {o:?}");
    assert_eq!((c.counters.starts, c.failures()), (2, 0));
}

/// The reuse switch reaches the engine after its hello and whenever it changes (9.6).
#[test]
fn the_reuse_switch_reaches_the_engine() {
    let mut c = client(&[]);
    c.start();
    until(&mut c, "the hello", running);
    let status_reuse = |c: &mut AiClient| -> bool {
        c.last_status = None;
        c.ask_status();
        until(c, "the status", |c| c.last_status.is_some());
        matches!(c.last_status, Some(humanity_ai_protocol::EngineMessage::Status { reuse: true, .. }))
    };
    humanity_core::reuse::set_enabled(false);
    c.poll();
    let off = status_reuse(&mut c);
    humanity_core::reuse::set_enabled(true);
    c.poll();
    let on = status_reuse(&mut c);
    assert_eq!((off, on), (false, true));
}

/// With no engine, a request falls back at once; a relative path is refused, never looked up.
#[test]
fn no_engine_and_a_relative_path_fall_back() {
    let mut none = AiClient::new(None, ClientConfig::default());
    assert_eq!(none.state(), &State::NotFound);
    let r = none.generate("x", "p", "q", 8);
    assert_eq!(none.take_outcomes(), vec![Outcome::Fallback { request: r, why: Fallback::NotFound }]);

    let mut relative = AiClient::new(Some(PathBuf::from(super::paths::ENGINE_EXE)), ClientConfig::default());
    relative.start();
    assert!(matches!(relative.state(), State::Crashed { why: ExitWhy::StartFailed(e) } if e.contains("not a full path")), "{:?}", relative.state());
    assert!(relative.pid().is_none());
}

/// The engine version the app expects is the engine crate's own.
#[test]
fn the_expected_engine_version_is_the_engine_crates() {
    let toml = std::fs::read_to_string(Path::new(env!("HUMANITY_REPO_ROOT")).join("crates/humanity-ai-engine/Cargo.toml")).expect("the engine's Cargo.toml");
    let version = toml.lines().find_map(|l| l.trim().strip_prefix("version = \"").and_then(|v| v.strip_suffix('"'))).expect("a version line");
    assert_eq!(version, super::paths::ENGINE_VERSION);
}
