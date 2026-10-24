//! The dev MCP endpoint's link to the running game: the seam between the tools (tools.rs) and
//! the dev IPC (engine/ipc.rs). `GameLink` is what a tool may ask of the game; `LiveLink` is the
//! real one, and the tests hand in a double that plays the game's part on a temp folder.
//!
//! A request is not written to disk: it is handed to the IPC's one request funnel
//! (`take_request_text`), which takes it the next time its poller runs, exactly as it takes a
//! dropped `debug/<name>` file, so every line of handling from the text on is the same code a rig
//! drives. The answers (`*_done.json`, screenshots) are the files the handlers already write.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

/// What a tool may ask of the game.
pub(crate) trait GameLink: Send + Sync {
    /// Hand `body` to the dev IPC as the request `debug/<request>` (e.g. `showcase_request.json`)
    /// and return once the game has taken it and a frame has run since (so its handler is done).
    /// Err says plainly why not, within `timeout`.
    fn submit(&self, request: &str, body: &str, timeout: Duration) -> Result<(), String>;
    /// The game's working folder: its `debug/` (the IPC's answers) is under it.
    fn game_dir(&self) -> PathBuf;
    /// This launch's log, and the crash log kept across launches.
    fn run_log(&self) -> Option<PathBuf>;
    fn crash_log(&self) -> Option<PathBuf>;
    /// The Performance page's measurements, freshly sampled.
    fn frame_costs(&self) -> Value;
    /// Frames run since the game started.
    fn frames(&self) -> u64;
    /// Whether the game's server is still on this computer (no tool runs otherwise).
    fn server_local(&self) -> bool;
}

/// The running game.
pub(crate) struct LiveLink {
    game_dir: PathBuf,
    logs_dir: Option<PathBuf>,
}

impl LiveLink {
    pub(crate) fn new(game_dir: PathBuf, logs_dir: Option<PathBuf>) -> Self {
        LiveLink { game_dir, logs_dir }
    }
}

/// How often a waiting tool looks again.
const POLL: Duration = Duration::from_millis(5);

impl GameLink for LiveLink {
    fn submit(&self, request: &str, body: &str, timeout: Duration) -> Result<(), String> {
        // Named exactly as the pollers name their files ("debug/<name>"), so the funnel's path
        // comparison is the same one a dropped file meets.
        let path = PathBuf::from(format!("debug/{request}"));
        let ticket = super::inject(&path, body.to_string());
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(frame) = super::taken_frame(ticket) {
                // Taken in frame `frame`; its handler ran in that frame. The next frame starting
                // means the handler is finished and its answer written.
                while super::frames() <= frame {
                    if Instant::now() > deadline {
                        return Err(format!(
                            "the game took {request} but no frame has run since ({:.0} s): it may be stalled",
                            timeout.as_secs_f32()
                        ));
                    }
                    std::thread::sleep(POLL);
                }
                return Ok(());
            }
            if Instant::now() > deadline {
                if super::withdraw(ticket) {
                    return Err(format!(
                        "the game did not take {request} within {:.0} s: frames are not running (closed, stalled, or a long load), \
                         or this request's poller does not run in the game's current state (most run every frame; the camera \
                         needs the world loaded and answers so in camera_done)",
                        timeout.as_secs_f32()
                    ));
                }
                // Taken between the look and the withdraw: go round once more.
            }
            std::thread::sleep(POLL);
        }
    }

    fn game_dir(&self) -> PathBuf {
        self.game_dir.clone()
    }

    fn run_log(&self) -> Option<PathBuf> {
        self.logs_dir.as_ref().map(|d| d.join("run.log"))
    }

    fn crash_log(&self) -> Option<PathBuf> {
        self.logs_dir.as_ref().map(|d| d.join("crash.log"))
    }

    fn frame_costs(&self) -> Value {
        // "Somebody is looking" for a little over a second, so the inventory sampling (VRAM,
        // RAM) runs and the smoothed numbers are current (renderer/frame_costs.rs `watch`).
        let until = Instant::now() + Duration::from_millis(1200);
        while Instant::now() < until {
            crate::renderer::frame_costs::watch();
            std::thread::sleep(Duration::from_millis(100));
        }
        crate::renderer::frame_costs::snapshot_json()
    }

    fn frames(&self) -> u64 {
        super::frames()
    }

    fn server_local(&self) -> bool {
        super::server_local()
    }
}
