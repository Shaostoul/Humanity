//! The app's one AI client, which Settings > AI and the Dev page's AI part drive, and the watchdog
//! that keeps its time while no page is drawing it.
//!
//! The engine is found each time [`client`] is asked: the installed one (`paths::installed_engine`),
//! unless the Dev page chose the developer's fake one for this run. While the engine is live a small
//! thread polls the client twice a second (a crash noticed, the idle stop kept, the reuse switch
//! passed on) and ends itself once the engine is not live; nothing in the frame waits on it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use super::client::{AiClient, ClientConfig};
use super::paths;

/// The Dev page's choice for this run: drive the developer's fake engine instead of the installed one.
static USE_DEV_FAKE: AtomicBool = AtomicBool::new(false);

/// The arguments the developer's engine is started with: the fake, with its default timing.
pub fn dev_fake_args() -> Vec<String> {
    vec!["--fake".into()]
}

/// Drive the developer's fake engine (true) or the installed one (false), for this run.
pub fn set_use_dev_fake(on: bool) {
    USE_DEV_FAKE.store(on, Ordering::Relaxed);
}

pub fn use_dev_fake() -> bool {
    USE_DEV_FAKE.load(Ordering::Relaxed)
}

fn state() -> &'static Mutex<AiClient> {
    static CLIENT: std::sync::OnceLock<Mutex<AiClient>> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| Mutex::new(AiClient::new(None, ClientConfig::default())))
}

/// The app's client, pointed at the engine to use now (it is found again on every call, so an engine
/// installed or removed shows at once: nothing about it is kept).
pub fn client() -> MutexGuard<'static, AiClient> {
    let mut c = state().lock().unwrap_or_else(|p| p.into_inner());
    let (engine, args) = if use_dev_fake() { (paths::dev_engine().map(|(p, _)| p), dev_fake_args()) } else { (paths::installed_engine(), Vec::new()) };
    if c.engine_path() != engine.as_deref() {
        c.config.args = args;
        c.set_engine(engine);
    }
    c
}

/// Start the watchdog if the engine is live and none runs. Pages call this after starting the engine.
pub fn watch() {
    static WATCHING: AtomicBool = AtomicBool::new(false);
    if WATCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new().name("ai-engine-watch".into()).spawn(|| {
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let mut c = state().lock().unwrap_or_else(|p| p.into_inner());
            c.poll();
            if !c.state().is_live() {
                break;
            }
        }
        WATCHING.store(false, Ordering::Release);
    });
    if spawned.is_err() {
        WATCHING.store(false, Ordering::Release);
    }
}
