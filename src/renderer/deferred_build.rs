//! A thing the renderer builds on a background thread and installs when it is
//! ready (startup speed, 2026-10-18).
//!
//! WHY IT EXISTS. On the 2026-10-18 boot rig (i7-8700K, RTX 4070, DX12 with
//! DXC, three boots of v0.1516.0) the app took 15.4 s to become playable, and
//! 9.1 s of that was `Pipeline::new` compiling twenty pipelines in parallel.
//! Eighteen of them were done in about 3 s. The other two, the cloud shell's
//! transparent PSO (8.99 s) and the near-field cloud screen march (8.23 s), are
//! the only programs that bake the volumetric cloud march, and the whole boot
//! sat waiting for them. Nothing on the way to a playable frame needs a cloud,
//! so those two now compile on a background thread that starts at the same
//! moment as the others, and the frame draws no clouds until they arrive.
//!
//! This is the cold path made shorter, NOT a cache: the operator does not want
//! a shader or pipeline cache during development ("every cache is a chance to
//! encounter a bug"), and nothing here is kept between boots.
//!
//! WHAT THIS FILE IS. The ready / not-ready holder, kept free of wgpu so its
//! one subtle rule can be tested without a GPU: an install from a shader hot
//! reload SUPERSEDES a background build still in flight. The reload's result is
//! installed at once, and the older build's result is dropped when it arrives,
//! because its receiver no longer exists. So a reload is never lost and nothing
//! is ever installed twice, whichever finishes first. The renderer's use is
//! `Pipeline::clouds` (src/renderer/pipeline.rs): `transparent_for(Cloud)` and
//! `cloud_screen_pipeline()` answer `None` until the pair is installed, and the
//! draw loops skip clouds on `None`.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// A value that is either installed, still being built elsewhere, or both
/// (an installed value with a newer build on its way never happens today:
/// every install path either waits or supersedes, but the type allows it).
pub struct Deferred<T> {
    ready: Option<T>,
    pending: Option<Pending<T>>,
}

/// A build in flight: where its result will arrive and when we began waiting.
struct Pending<T> {
    rx: Receiver<T>,
    started: Instant,
}

/// What one `poll` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Polled {
    /// Nothing was being built.
    Idle,
    /// A build is still running.
    StillWaiting,
    /// The build's result arrived and is now installed; `waited` is how long
    /// after the wait began (the build's start) it was installed.
    Installed { waited: Duration },
    /// The builder went away without sending anything (its thread panicked).
    /// Nothing is installed and nothing is pending any more.
    Lost,
}

impl<T> Deferred<T> {
    /// Waiting for a build whose result will arrive on `rx`, starting now.
    pub fn waiting(rx: Receiver<T>) -> Self {
        Self { ready: None, pending: Some(Pending { rx, started: Instant::now() }) }
    }

    /// Already built (the inline path: no threads, or a hot reload).
    pub fn installed(value: T) -> Self {
        Self { ready: Some(value), pending: None }
    }

    /// Never built and not being built: `ready()` stays `None` and `poll`
    /// answers `Idle` (the clouds under FXC, `CloudsWhen::Never`).
    pub fn empty() -> Self {
        Self { ready: None, pending: None }
    }

    /// The installed value, or `None` while it is still being built.
    pub fn ready(&self) -> Option<&T> {
        self.ready.as_ref()
    }

    /// Whether a build is still in flight.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The installed value, consuming the holder (and dropping any build in
    /// flight, whose result then has nowhere to go).
    pub fn into_ready(self) -> Option<T> {
        self.ready
    }

    /// Install `value` now and CANCEL any build still in flight: its receiver
    /// is dropped here, so its result, buffered or not yet sent, can never be
    /// installed over this one. Returns whether a build was superseded.
    pub fn install_now(&mut self, value: T) -> bool {
        self.ready = Some(value);
        self.pending.take().is_some()
    }

    /// Check for the build's result without blocking: once a frame. On
    /// arrival the result replaces whatever was installed.
    pub fn poll(&mut self) -> Polled {
        let Some(p) = self.pending.as_ref() else {
            return Polled::Idle;
        };
        match p.rx.try_recv() {
            Ok(value) => {
                let waited = p.started.elapsed();
                self.pending = None;
                self.ready = Some(value);
                Polled::Installed { waited }
            }
            Err(TryRecvError::Empty) => Polled::StillWaiting,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                Polled::Lost
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    /// Not installed until the build's result is polled, installed after:
    /// the shape of `transparent_for(Cloud)` before and after the clouds
    /// arrive.
    #[test]
    fn nothing_is_ready_until_the_build_arrives_and_is_polled() {
        let (tx, rx) = channel::<&str>();
        let mut d = Deferred::waiting(rx);
        assert!(d.ready().is_none(), "nothing is installed while the build runs");
        assert!(d.is_pending());
        assert_eq!(d.poll(), Polled::StillWaiting);
        assert!(d.ready().is_none());
        tx.send("boot build").unwrap();
        // Sent but not yet polled: still not installed (the install happens
        // on the frame that polls, never in the middle of one).
        assert!(d.ready().is_none(), "a sent result is installed by poll, not by send");
        assert!(matches!(d.poll(), Polled::Installed { .. }));
        assert_eq!(d.ready(), Some(&"boot build"));
        assert!(!d.is_pending());
        assert_eq!(d.poll(), Polled::Idle, "nothing more to wait for");
    }

    /// THE RACE THIS TYPE EXISTS FOR. A shader hot reload compiles the pair
    /// itself and installs it while the boot's background build is still
    /// running. Whichever finishes first, the reload's pipeline must be the
    /// one installed afterwards, exactly once: the boot build's result,
    /// whether it arrives later or is already sitting unpolled in the
    /// channel, is never installed over it.
    ///
    /// Seen RED (2026-10-18), with `install_now` changed to leave the pending
    /// build in place (`self.pending.as_ref().is_some()` instead of
    /// `take()`): case (a) failed with `left: Some("boot build (stale
    /// module)")`, `right: Some("hot reload (new module)")`, the stale boot
    /// build overwriting the reload on the next poll; and with the cases in
    /// the other order, case (b) failed at "nothing is pending after a
    /// superseding install". Restored.
    #[test]
    fn a_reload_during_the_background_build_ends_with_exactly_its_own_pipeline() {
        // (a) The boot build finished BEFORE the reload, but no frame had
        // polled it yet: its result is sitting in the channel.
        let (tx, rx) = channel::<&str>();
        let mut d = Deferred::waiting(rx);
        tx.send("boot build (stale module)").unwrap();
        assert!(d.install_now("hot reload (new module)"));
        for _ in 0..3 {
            d.poll();
        }
        assert_eq!(d.ready(), Some(&"hot reload (new module)"), "the stale boot build must never be installed");
        assert!(!d.is_pending());

        // (b) The boot build finishes AFTER the reload.
        let (tx, rx) = channel::<&str>();
        let mut d = Deferred::waiting(rx);
        assert!(d.install_now("hot reload (new module)"), "the boot build was superseded");
        assert!(!d.is_pending(), "nothing is pending after a superseding install");
        // The boot build's send now has nowhere to go: its result is dropped
        // on its own thread (the real worker logs that it was superseded).
        assert!(tx.send("boot build (stale module)").is_err(), "the stale result has no receiver");
        assert_eq!(d.poll(), Polled::Idle);
        assert_eq!(d.ready(), Some(&"hot reload (new module)"));

        // (c) Two reloads in a row with nothing pending: each simply replaces
        // the last, and neither reports a superseded build.
        let mut d = Deferred::installed("boot (inline)");
        assert!(!d.install_now("reload 1"));
        assert!(!d.install_now("reload 2"));
        assert_eq!(d.ready(), Some(&"reload 2"));
        assert_eq!(d.poll(), Polled::Idle);
    }

    /// A builder that dies (its thread panicked) is reported once as Lost and
    /// leaves nothing installed and nothing pending, so the frame keeps
    /// skipping clouds instead of polling a dead channel forever.
    #[test]
    fn a_builder_that_dies_is_reported_once_and_leaves_nothing_installed() {
        let (tx, rx) = channel::<&str>();
        let mut d = Deferred::waiting(rx);
        drop(tx);
        assert_eq!(d.poll(), Polled::Lost);
        assert!(d.ready().is_none());
        assert!(!d.is_pending());
        assert_eq!(d.poll(), Polled::Idle, "Lost is reported once");
    }
}
