//! What volume a new video starts at (2026-11-06). The operator: "Let's add the toggle option
//! maintain last used volume setting between app resets or don't. Another could be all videos start
//! at custom % volume. So I could set them at say 5% for every new video I start because, not
//! everyone balances audio the same."
//!
//! One choice for every video the app plays (a video posted in chat, the Media page's player, a
//! video on an in-world screen), in Settings > Audio, saved in the config:
//!
//! - **My last volume** (the default): a new video starts where the last video's volume was left,
//!   kept across restarts. Moving any video's volume bar sets it ([`note_volume`]).
//! - **A set volume**: every new video starts at the same chosen volume (5%, say), whatever the
//!   last one was turned to. Its volume bar still turns that one video up or down.
//!
//! Why two choices and a slider, not three ("last", "set", "full") or a toggle: full volume is the
//! set volume at 100% (its default), so a third choice would be a second way to say one thing; and a
//! "remember" toggle whose off state quietly means "use the slider" hides how the two relate, where
//! a choice names both. Mute is not remembered: a video that starts silent with no visible reason,
//! the morning after, reads as broken.
//!
//! The volume is the video's own, 0 to 1, on top of the app's Master (and, for a screen in the world,
//! the sound-effects bus and the distance). Held here, process-wide, the way the chat's timestamp
//! format is: the config sets it at load and reads it at save, Settings edits it, and every player
//! asks [`start_volume`] when a video starts. A moved volume marks it unsaved ([`take_unsaved`]),
//! which the engine turns into a settings save.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

/// Where a new video's volume comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartAt {
    /// The volume the last video was left at.
    Last,
    /// The chosen set volume.
    Set,
}

impl StartAt {
    /// The word the config keeps.
    pub fn as_str(self) -> &'static str {
        match self {
            StartAt::Last => "last",
            StartAt::Set => "set",
        }
    }

    /// From the config's word (anything else is the default, Last).
    pub fn from_config_str(s: &str) -> StartAt {
        if s == "set" {
            StartAt::Set
        } else {
            StartAt::Last
        }
    }
}

static START_AT: AtomicU8 = AtomicU8::new(0);
/// f32 bits: 1.0 (0x3F80_0000) until the config says otherwise.
static SET_VOLUME: AtomicU32 = AtomicU32::new(0x3F80_0000);
static LAST_VOLUME: AtomicU32 = AtomicU32::new(0x3F80_0000);
static UNSAVED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// A test's own choice (start at, set volume, last volume), on its own thread only, so tests
    /// running side by side do not see each other's ([`use_on_this_thread`]).
    static THIS_THREAD: Cell<Option<(StartAt, f32, f32)>> = const { Cell::new(None) };
}

fn clamp(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// The choice: where a new video's volume comes from, and the set volume.
pub fn choice() -> (StartAt, f32) {
    if let Some((at, set, _)) = THIS_THREAD.with(|c| c.get()) {
        return (at, set);
    }
    let at = if START_AT.load(Ordering::Relaxed) == 1 { StartAt::Set } else { StartAt::Last };
    (at, f32::from_bits(SET_VOLUME.load(Ordering::Relaxed)))
}

/// Make the choice (Settings, and the config at load).
pub fn set_choice(at: StartAt, set_volume: f32) {
    if let Some((_, _, last)) = THIS_THREAD.with(|c| c.get()) {
        THIS_THREAD.with(|c| c.set(Some((at, clamp(set_volume), last))));
        return;
    }
    START_AT.store(if at == StartAt::Set { 1 } else { 0 }, Ordering::Relaxed);
    SET_VOLUME.store(clamp(set_volume).to_bits(), Ordering::Relaxed);
}

/// The volume the last video was left at (0 to 1).
pub fn last_volume() -> f32 {
    if let Some((_, _, last)) = THIS_THREAD.with(|c| c.get()) {
        return last;
    }
    f32::from_bits(LAST_VOLUME.load(Ordering::Relaxed))
}

/// Put the last volume back as the config kept it (at load: not a change to save).
pub fn restore_last_volume(v: f32) {
    if let Some((at, set, _)) = THIS_THREAD.with(|c| c.get()) {
        THIS_THREAD.with(|c| c.set(Some((at, set, clamp(v)))));
        return;
    }
    LAST_VOLUME.store(clamp(v).to_bits(), Ordering::Relaxed);
}

/// The volume a video starting now starts at (0 to 1): the last volume, or the set one.
pub fn start_volume() -> f32 {
    match choice() {
        (StartAt::Last, _) => last_volume(),
        (StartAt::Set, set) => set,
    }
}

/// A person moved a video's volume to `v`: it is the last volume now, saved with the settings
/// (whichever choice is made, so switching back to "my last volume" finds it).
pub fn note_volume(v: f32) {
    let v = clamp(v);
    if (last_volume() - v).abs() < f32::EPSILON {
        return;
    }
    if let Some((at, set, _)) = THIS_THREAD.with(|c| c.get()) {
        THIS_THREAD.with(|c| c.set(Some((at, set, v))));
        return;
    }
    LAST_VOLUME.store(v.to_bits(), Ordering::Relaxed);
    UNSAVED.store(true, Ordering::Relaxed);
}

/// Whether a moved volume is waiting to be saved; clears it. The engine asks every frame.
pub fn take_unsaved() -> bool {
    UNSAVED.swap(false, Ordering::Relaxed)
}

/// Give this thread a choice of its own (tests): every function here then reads and writes that,
/// not the app's, until the thread ends. The app never calls it.
pub fn use_on_this_thread(at: StartAt, set_volume: f32, last_volume: f32) {
    THIS_THREAD.with(|c| c.set(Some((at, clamp(set_volume), clamp(last_volume)))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A set volume starts every new video there whatever the last one was turned to; "my last
    /// volume" follows the last moved bar; a move marks the settings unsaved once. RED first
    /// (2026-11-06): with `start_volume` answering the last volume whatever the choice, "a set 5%
    /// starts at 5%" failed (left: 0.8, right: 0.05).
    #[test]
    fn a_set_volume_starts_every_video_there_and_last_follows_the_bar() {
        use_on_this_thread(StartAt::Set, 0.05, 0.8);
        assert_eq!(start_volume(), 0.05, "a set 5% starts at 5%");
        note_volume(0.6);
        assert_eq!(start_volume(), 0.05, "moving a bar does not change the set volume");
        set_choice(StartAt::Last, 0.05);
        assert_eq!(start_volume(), 0.6, "my last volume: where the bar was left");
        set_choice(StartAt::Set, 7.0);
        assert_eq!(start_volume(), 1.0, "clamped to full");
        assert_eq!(StartAt::from_config_str("set"), StartAt::Set);
        assert_eq!(StartAt::from_config_str("anything"), StartAt::Last);
    }
}
