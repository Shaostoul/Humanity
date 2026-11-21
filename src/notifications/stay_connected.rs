//! Staying connected while the phone app is closed (Android rung A4, 2026-11-20;
//! docs/design/platforms.md "Staying connected: the foreground service and UnifiedPush",
//! android/README.md).
//!
//! Android stops an app that is not on the screen, and with it the chat connection, so a phone
//! got no messages or notices while HumanityOS was in the background. The operator, 2026-11-20, on
//! how to keep it connected: "Whichever one you believe is best." The choice, compared in
//! platforms.md: a FOREGROUND SERVICE (`ChatService`, android/java/), which keeps the app's own
//! process and its own connection alive, so no extra app is needed and no third party sees that a
//! message arrived; UnifiedPush (a distributor app such as ntfy wakes the app) can come later as a
//! battery-saving choice. Android's price for a foreground service is a notification in the bar
//! for as long as it runs, so it is the person's choice: Settings, Notifications, "Stay connected
//! while HumanityOS is closed", OFF by default ([`LABEL`], [`HINT`]).
//!
//! What a notice says while the app is closed is decided where it always is
//! (`notifications::decide`, the "What a notice shows" choice): who it is from by default, never
//! the message unless the person chose that. On the locked screen of a phone with a screen lock (a
//! PIN, pattern or password; a swipe-only lock shows every notice whole, Android's rule) it shows
//! nothing at all unless the person turns on "Show a line on the locked screen" ([`LOCK_SCREEN_LABEL`], off by
//! default; the Android notice `Backend` in `toast.rs`), and even then the line hides who it is
//! from and the message only where the phone's own "hide sensitive content" setting is on, which
//! its hint says ([`LOCK_SCREEN_HINT`]). The connection stays the app's own, so sealed messages
//! stay sealed end to end.
//!
//! The desktop's counterpart is closing into the tray (`close_to_tray`); on a desktop everything
//! here does nothing. A web page cannot hold a connection while closed at all; the website's
//! notices come from Web Push, so the website has no such switch (docs/design/notifications.md).

/// The switch's words (data/i18n/en.json).
pub const LABEL: &str = "notifications.stay_connected";
/// Its hint: what it does, that a notification stays in the bar, that it uses some battery.
pub const HINT: &str = "notifications.stay_connected_hint";
/// The words when Android refused to start it.
pub const REFUSED: &str = "notifications.stay_connected_refused";
/// The words when Android is not letting the app show notices.
pub const BLOCKED: &str = "notifications.android_blocked";
/// The choice of a line on the locked screen (off by default) and its hint, which says the line
/// depends on the phone's own "hide sensitive content" setting.
pub const LOCK_SCREEN_LABEL: &str = "notifications.lock_screen_line";
pub const LOCK_SCREEN_HINT: &str = "notifications.lock_screen_line_hint";

/// Start or stop the stay-connected service to match `on`. False when Android refused to start
/// it (Android 12 and later refuse while the app is in the background; Settings says so). On a
/// desktop nothing, and true.
pub fn apply(on: bool) -> bool {
    #[cfg(target_os = "android")]
    {
        use humanity_core::i18n::t;
        use humanity_core::platform::android_jni as jni;
        let result = if on {
            jni::service_start(
                t("notifications.android_channel_connection"),
                t("notifications.android_ongoing_title"),
                t("notifications.android_ongoing_text"),
            )
        } else {
            jni::service_stop().map(|()| true)
        };
        match result {
            Ok(started) => {
                log::info!("stay connected: {}", if !on { "stopped" } else if started { "started" } else { "refused by Android" });
                started
            }
            Err(e) => {
                log::warn!("stay connected: {e}");
                false
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = on;
        true
    }
}

/// Ask Android to let the app show notices (Android 13 and later, where they are off until the
/// person allows them; the dialog is Android's own). Asked when the person turns staying connected
/// on, the moment its purpose is plain. Nothing on a desktop.
pub fn ask_for_notices() {
    #[cfg(target_os = "android")]
    {
        // The activity was handed to humanity-core by the entry point (src/android.rs).
        if let Err(e) = humanity_core::platform::android_jni::notices_ask() {
            log::warn!("notifications: could not ask Android to allow them: {e}");
        }
    }
}

/// Whether the phone lets the app show notices: `Some(false)` when Android blocks them (Settings
/// says how to allow them), `None` on a desktop.
pub fn notices_allowed() -> Option<bool> {
    #[cfg(target_os = "android")]
    {
        match humanity_core::platform::android_jni::notices_allowed() {
            Ok(allowed) => Some(allowed),
            Err(e) => {
                log::warn!("notifications: could not ask Android whether they are allowed: {e}");
                None
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off unless the person turns it on, a config.json from before it reads off, and its words
    /// say what Android will do: a notification in the bar, some battery. Seen red 2026-11-20 with
    /// the hint written "it uses some power": "the hint must say it uses some battery: ...".
    #[test]
    fn staying_connected_is_off_by_default_and_says_what_it_costs() {
        assert!(!crate::notifications::LocalSettings::default().stay_connected);
        let old: crate::notifications::LocalSettings = serde_json::from_str(r#"{"desktop":true,"tray":true}"#).unwrap();
        assert!(!old.stay_connected, "a config.json from before it: off");
        let en: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap()).unwrap();
        let words = |key: &str| en.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|v| v.as_str()).map(str::to_string);
        for key in [
            LABEL,
            HINT,
            REFUSED,
            BLOCKED,
            "notifications.android_channel_connection",
            "notifications.android_channel_messages",
            "notifications.android_ongoing_title",
            "notifications.android_ongoing_text",
            "notifications.android_locked_body",
        ] {
            assert!(words(key).is_some_and(|w| !w.is_empty()), "{key} is not in en.json");
        }
        let hint = words(HINT).unwrap().to_lowercase();
        assert!(hint.contains("notification in the bar"), "the hint must say a notification stays in the bar: {hint}");
        assert!(hint.contains("battery"), "the hint must say it uses some battery: {hint}");
        assert!(hint.contains("off unless you turn it on"), "the hint must say it is off by default: {hint}");
    }

    /// A locked phone with a screen lock shows nothing of a notice unless the person chose the line, a config.json
    /// from before the choice reads it off, and the choice's hint says the line depends on the
    /// phone's own "hide sensitive content" setting and that, with it off, the whole notice shows
    /// (the review's finding 1). The Java's side is held by `platform::android_java`'s
    /// `message_notices_show_nothing_on_a_locked_screen_by_default`. Seen red 2026-11-20 before
    /// the words were in en.json: "notifications.lock_screen_line is not in en.json".
    ///
    /// (The test that sat here, `on_a_desktop_nothing_happens`, called the desktop's stubs, which
    /// return constants, so it could not fail on anything that mattered; the review said so and
    /// it was removed on 2026-11-20.)
    #[test]
    fn a_locked_phone_shows_nothing_unless_the_person_chose_a_line() {
        assert!(!crate::notifications::LocalSettings::default().lock_screen_line);
        let old: crate::notifications::LocalSettings = serde_json::from_str(r#"{"desktop":true,"stay_connected":true}"#).unwrap();
        assert!(!old.lock_screen_line, "a config.json from before it: off");
        let en: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap()).unwrap();
        let words = |key: &str| en.pointer(&format!("/{}", key.replace('.', "/"))).and_then(|v| v.as_str()).map(str::to_string);
        assert!(words(LOCK_SCREEN_LABEL).is_some_and(|w| !w.is_empty()), "{LOCK_SCREEN_LABEL} is not in en.json");
        let hint = words(LOCK_SCREEN_HINT).unwrap_or_else(|| panic!("{LOCK_SCREEN_HINT} is not in en.json"));
        assert!(hint.contains("Off unless you turn it on"), "the hint must say it is off by default: {hint}");
        assert!(hint.contains("Hide sensitive content"), "the hint must name the phone's own setting: {hint}");
        assert!(hint.contains("shows the whole notice"), "the hint must say what shows when that setting is off: {hint}");
    }
}
