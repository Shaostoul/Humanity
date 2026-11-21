//! When the 3D world is prepared: the person's choice (Settings > Graphics > General > Prepare the
//! 3D world, 2026-11-20) and this start's own times, which the Settings page shows. The engine's
//! half, which prepares the world, is src/engine/world_prep.rs (its module docs say why); this half
//! sits with the pages (the gui layer), which may not reach into the engine
//! (scripts/check-crate-layers.js).
//!
//! THE DEFAULT is only when Play is pressed, the operator's, 2026-11-20, verbatim: "I figure the all
//! at start could be an option, but I'd like to default to having to press play from now on. That way
//! we can test non-game stuff faster."

use std::sync::atomic::{AtomicU64, Ordering};

/// When the 3D world is prepared. The saved setting is [`Choice::id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// In the background, right after the first page frame.
    Background,
    /// Only when the person presses Play (or anything else enters the world): the default.
    OnPlay,
    /// At the start, before the pages: the order of every start before 2026-11-20.
    AtStart,
}

impl Choice {
    /// Every choice, in the order Settings shows them (the default first).
    pub(crate) const ALL: [Choice; 3] = [Choice::OnPlay, Choice::Background, Choice::AtStart];

    /// The saved setting's value, and its words' key part (`graphics.world_prep_choice.<id>`).
    pub(crate) fn id(self) -> &'static str {
        match self {
            Choice::Background => "background",
            Choice::OnPlay => "on_play",
            Choice::AtStart => "at_start",
        }
    }

    /// The choice a saved value names; anything else is the default, only when Play is pressed.
    pub(crate) fn from_id(s: &str) -> Choice {
        Self::ALL.into_iter().find(|c| c.id() == s.trim()).unwrap_or(Choice::OnPlay)
    }
}

/// The choice a run uses: a phone never builds the world (Play's Enter is a notice there); then
/// `HUMANITY_WORLD_PREP`, then `HUMANITY_PAGES_FIRST` (1: on Play, 0: at start, the old order), then
/// the saved setting.
pub(crate) fn effective(setting: &str, phone: bool, world_prep_env: Option<&str>, pages_first_env: Option<&str>) -> Choice {
    if phone {
        return Choice::OnPlay;
    }
    if let Some(v) = world_prep_env.map(str::trim).filter(|v| !v.is_empty()) {
        return Choice::from_id(v);
    }
    match pages_first_env.map(str::trim) {
        Some("1") => Choice::OnPlay,
        Some("0") => Choice::AtStart,
        _ => Choice::from_id(setting),
    }
}

static CHOICE: std::sync::OnceLock<Choice> = std::sync::OnceLock::new();

/// Decide this run's choice from the saved setting and the switches, once, before the renderer.
pub(crate) fn init(setting: &str) -> Choice {
    *CHOICE.get_or_init(|| {
        let env = |k: &str| std::env::var(k).ok();
        let c = effective(setting, humanity_core::platform::device::phone(), env("HUMANITY_WORLD_PREP").as_deref(), env("HUMANITY_PAGES_FIRST").as_deref());
        log::info!("[WorldPrep] the 3D world is prepared: {} (setting {:?})", c.id(), setting);
        c
    })
}

/// This run's choice ([`init`]; the default before it, which only a test sees).
pub(crate) fn choice() -> Choice {
    CHOICE.get().copied().unwrap_or(Choice::OnPlay)
}

/// This run's first page frame and world-ready times, ms after the window opened (0: not yet), for
/// Settings' "what it cost on this computer" line ([`measured`]). In memory only, this run's.
static PAGES_MS: AtomicU64 = AtomicU64::new(0);
static READY_MS: AtomicU64 = AtomicU64::new(0);

/// The first page frame was drawn this long after the window opened.
pub(crate) fn note_pages(ms: u64) {
    PAGES_MS.store(ms.max(1), Ordering::Relaxed);
}

/// The 3D world was ready this long after the window opened.
pub(crate) fn note_ready(ms: u64) {
    READY_MS.store(ms.max(1), Ordering::Relaxed);
}

/// This run's (first page frame, world ready) in ms after the window opened, each once known.
pub(crate) fn measured() -> (Option<u64>, Option<u64>) {
    let some = |v: u64| (v > 0).then_some(v);
    (some(PAGES_MS.load(Ordering::Relaxed)), some(READY_MS.load(Ordering::Relaxed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The setting's three choices, each named by its saved value, and the default for anything else.
    /// Seen red (2026-11-20): with `from_id` matching nothing and answering Background (the default
    /// before the operator moved it to Play that day), this test failed at the first round trip (left
    /// Background, right OnPlay), and so did the switches test below.
    #[test]
    fn the_settings_three_choices() {
        for c in Choice::ALL {
            assert_eq!(Choice::from_id(c.id()), c, "{c:?} round trip");
            assert_eq!(effective(c.id(), false, None, None), c, "{c:?} as the setting");
        }
        assert_eq!(Choice::from_id(""), Choice::OnPlay, "no setting: the default, only when Play is pressed (operator, 2026-11-20)");
        assert_eq!(Choice::from_id("bogus"), Choice::OnPlay);
        assert_eq!(Choice::ALL[0], Choice::OnPlay, "Settings lists the default first");
        assert_eq!(Choice::ALL.len(), 3);
    }

    /// The switches: a phone never builds the world, then HUMANITY_WORLD_PREP, then
    /// HUMANITY_PAGES_FIRST (1 on Play, 0 the old order), then the setting.
    #[test]
    fn the_switches_come_before_the_setting() {
        assert_eq!(effective("at_start", true, Some("background"), Some("0")), Choice::OnPlay, "a phone");
        assert_eq!(effective("at_start", false, Some("background"), None), Choice::Background);
        assert_eq!(effective("background", false, None, Some("1")), Choice::OnPlay);
        assert_eq!(effective("background", false, None, Some("0")), Choice::AtStart);
        assert_eq!(effective("on_play", false, Some(""), Some("")), Choice::OnPlay, "empty switches: the setting");
    }
}
