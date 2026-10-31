//! What a notice leads to, and the cues beside it (rung 2, part 2, 2026-10-31;
//! docs/design/notifications.md, section 3 items 1 and 4).
//!
//! - **A click on an operating-system notice opens its conversation.** Each notice carries a token
//!   that names nothing: this run's id and a number ([`Targets::token`]). The run keeps the last
//!   [`KEPT`] numbers' conversations in memory, so a click maps back to one ([`Targets::resolve`])
//!   and opens it the way the in-app list does (`gui/notice_panel.rs` `open_entry`). The operating
//!   system keeps a notice's words and its token (Windows in its notification database), so the
//!   token holds no key, no address and no name. A token from another run, or one past the kept
//!   numbers, opens the window only: what it named is gone with the list it belonged to.
//! - **The cues that need no sound** ([`cues`]): in the world with the window in front, a short
//!   banner at the top of the screen (a toast may sit behind a full-screen game, and a focused
//!   window gets no toast); with the window behind, the taskbar button flashes. Both only for what
//!   THE rule let through, and neither while the window is hidden in the tray (there is no
//!   taskbar button then, and the toast is the cue). The banner's words are the notice's
//!   (`decide::words`), so it shows the message only when "What a notice shows" does.

use std::collections::VecDeque;

use super::list::Entry;

/// How many notices' conversations a run keeps for their clicks.
pub const KEPT: usize = 64;

/// The token's first part: which shape of token this is.
const SCHEME: &str = "hos1";

/// How long the in-world banner stays, in seconds.
pub const BANNER_SECS: f64 = 6.0;

/// This run's notices and the conversations they lead to. Never saved.
#[derive(Debug, Default)]
pub struct Targets {
    /// This run's id: a random number, so a token from another run never opens this run's entry.
    run: String,
    next: u64,
    kept: VecDeque<(u64, Entry)>,
}

/// What a click on a notice asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Click {
    /// Open this conversation (and the window).
    Open(Entry),
    /// Show the window: the test notice, the "still running" notice, or a notice whose
    /// conversation this run no longer keeps.
    Window,
}

impl Targets {
    /// Targets for the run `run` (a lib test names its own; the app's comes from [`run_id`]).
    pub fn with_run(run: &str) -> Targets {
        Targets { run: run.to_string(), ..Default::default() }
    }

    fn run(&mut self) -> &str {
        if self.run.is_empty() {
            self.run = run_id().to_string();
        }
        &self.run
    }

    /// A token for a notice that opens `e`'s conversation.
    pub fn token(&mut self, e: Entry) -> String {
        let n = self.next;
        self.next += 1;
        self.kept.push_back((n, e));
        while self.kept.len() > KEPT {
            self.kept.pop_front();
        }
        format!("{SCHEME}.{}.{n}", self.run())
    }

    /// The newest token [`Self::token`] made, if any (the dev IPC reports it).
    pub fn last_token(&self) -> Option<String> {
        let run = if self.run.is_empty() { run_id() } else { self.run.as_str() };
        self.kept.back().map(|(n, _)| format!("{SCHEME}.{run}.{n}"))
    }

    /// A token for a notice that only shows the window (the test notice, the tray's own).
    pub fn window_token(&mut self) -> String {
        format!("{SCHEME}.{}.w", self.run())
    }

    /// What the notice whose token is `launch` leads to.
    pub fn resolve(&self, launch: &str) -> Click {
        let mut parts = launch.trim().splitn(3, '.');
        let (Some(SCHEME), Some(run), Some(n)) = (parts.next(), parts.next(), parts.next()) else {
            return Click::Window;
        };
        if run != self.run || self.run.is_empty() {
            return Click::Window;
        }
        let Ok(n) = n.parse::<u64>() else { return Click::Window };
        self.kept.iter().find(|(k, _)| *k == n).map(|(_, e)| Click::Open(e.clone())).unwrap_or(Click::Window)
    }
}

/// This run's id, made once: 16 hex digits of a random number.
pub fn run_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| format!("{:016x}", rand::random::<u64>()))
}

/// Where the window is, for [`cues`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Window {
    /// It has the keyboard focus.
    pub focused: bool,
    /// It is hidden in the tray (closed with "keeps running in the tray" on).
    pub hidden: bool,
    /// The person is playing in the world (no page open over it).
    pub in_world: bool,
}

/// The cues beside a notice the rule let through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cues {
    /// The short banner at the top of the screen in the world.
    pub banner: bool,
    /// The taskbar button's flash.
    pub flash: bool,
}

/// The cues for a notice the rule let through, given where the window is. Pure.
pub fn cues(w: Window) -> Cues {
    if w.hidden {
        return Cues::default();
    }
    Cues { banner: w.focused && w.in_world, flash: !w.focused }
}

/// The in-world banner showing now: the notice's words, and when it came.
#[derive(Debug, Clone, PartialEq)]
pub struct Banner {
    pub title: String,
    /// Empty when the notice says only its title.
    pub body: String,
    pub at: std::time::Instant,
}

impl Banner {
    /// How opaque it is now.
    pub fn opacity(&self) -> f32 {
        opacity_after(self.at.elapsed().as_secs_f64())
    }
}

/// How opaque a banner `age` seconds old is: 1 while shown, fading over its last second, 0 once
/// gone.
pub fn opacity_after(age: f64) -> f32 {
    if !(0.0..BANNER_SECS).contains(&age) {
        return 0.0;
    }
    ((BANNER_SECS - age).min(1.0)) as f32
}
