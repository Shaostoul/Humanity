//! The notification-area (tray) icon (notifications rung 2, part 2, 2026-10-31;
//! docs/design/notifications.md, "The tray"). The operator, 2026-10-30: "Can we add the ability for
//! HOS to run in the taskbar? Then we could add to the right click menu a button for mute that
//! opens to the available options."
//!
//! - **The crate:** tauri-apps `tray-icon` 0.26 (MIT OR Apache-2.0), its menus through `muda`. On
//!   Windows its window procedure runs on the event loop's thread (the icon is made here, on it);
//!   on Linux it is a StatusNotifierItem over D-Bus (`ksni`, its own thread); on macOS it is made
//!   after winit's loop has started (the first frame), on the main thread, as AppKit requires.
//! - **Its events** (a menu click, a click on the icon) arrive through handlers that queue them and
//!   wake the event loop (`tray_window::wake`), so a click reaches a loop waiting with the window
//!   hidden. [`pump`] takes them once per frame, and once per tick while hidden: one atomic load
//!   when nothing happened.
//! - **The menu** is `notifications::tray::Look::menu`: Open HumanityOS, Mute everything (the data
//!   file's lengths, or the muted line and Unmute), Quit HumanityOS. Its Mute is Settings >
//!   Notifications > Mute everything itself (`Mutes::everything`).
//! - **The icon's look** follows the mutes and the in-app list's unseen count: a dot for notices not
//!   opened yet, the picture dimmed with a bell and a slash while everything is muted, the tooltip
//!   in words. Rebuilt only when the look changes, never per frame.
//! - **A click on a notice** (`notifications::toast::take_clicks`) opens its conversation through
//!   the in-app list's own path (`gui/notice_panel.rs` `open_entry`) and shows the window.
//!
//! **Who gets an icon:** a person's launch (a double-click, `just play`), with Settings' "Show
//! HumanityOS in the notification area" on (the default). An instance a script started (a rig, a
//! boot check: `launch_focus::launch_in_background`) gets none, so agents' boots never pile icons
//! into the operator's tray (a force-closed process leaves its icon behind until the pointer
//! passes over it), and it never answers a click on the operator's own notices. A rig that tests
//! the tray asks for it with `HUMANITY_TRAY=1`, and closes gracefully so the icon goes with it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::engine::state::EngineState;
use crate::notifications::{self as n, tray::Action, tray::Look, tray::MenuLine};

/// The environment variable a script sets to give its instance the tray anyway (a rig testing it).
pub(crate) const TRAY_ENV: &str = "HUMANITY_TRAY";

/// The tray's state on `EngineState`.
#[derive(Default)]
pub(crate) struct TrayState {
    /// The icon, while it shows.
    icon: Option<Shown>,
    /// The handlers and the click route are installed (once for the run).
    started: bool,
    /// The icon could not be made this run (logged once; Settings still shows the switch).
    failed: bool,
    /// The window is hidden in the tray (engine/tray_window.rs).
    pub(crate) hidden: bool,
    /// When the hidden window's tick next runs.
    pub(crate) next_tick: Option<std::time::Instant>,
    /// When it last ran (its step).
    pub(crate) last_tick: Option<std::time::Instant>,
    /// The person was told this run that HumanityOS stays in the tray.
    pub(crate) told: bool,
}

impl TrayState {
    /// The icon shows in the notification area.
    pub(crate) fn shown(&self) -> bool {
        self.icon.is_some()
    }
}

struct Shown {
    icon: tray_icon::TrayIcon,
    /// The app's icon, [`n::tray::ICON_PX`] square, straight RGBA: made once from the exe's own
    /// copy, which never changes while it runs.
    base: Vec<u8>,
    /// What the icon shows now, and the language its menu is in.
    look: Look,
    lang: &'static str,
}

/// What the handlers queued.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    /// A menu line was clicked: its id.
    Menu(String),
    /// The icon itself was clicked (left button) or double-clicked: open the window.
    Icon,
}

static PENDING: AtomicBool = AtomicBool::new(false);
static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
const EVENTS_KEPT: usize = 32;

fn queue(e: Event) {
    if let Ok(mut q) = EVENTS.lock() {
        if q.len() >= EVENTS_KEPT {
            q.pop_front();
        }
        q.push_back(e);
    }
    PENDING.store(true, Ordering::SeqCst);
    crate::engine::tray_window::wake();
}

/// Something the tray or a notice asked for is waiting (one atomic load each).
pub(crate) fn pending() -> bool {
    PENDING.load(Ordering::SeqCst)
}

fn take_events() -> Vec<Event> {
    if !PENDING.swap(false, Ordering::SeqCst) {
        return Vec::new();
    }
    EVENTS.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
}

/// The environment variable a script ALSO sets to have its instance serve clicks on notices (the
/// Windows notice activator). Separate from [`TRAY_ENV`]: a rig with a tray beside the operator's
/// own game would register the same activator class, and Windows could hand HIS clicks to the rig.
pub(crate) const TRAY_CLICKS_ENV: &str = "HUMANITY_TRAY_CLICKS";

/// Does an instance serve clicks on notices? A person's launch does; a script's only when it names
/// [`TRAY_CLICKS_ENV`] too, never for [`TRAY_ENV`] alone (the critic's review of part 2,
/// 2026-10-31). Pure.
pub(crate) fn serves_clicks(person: bool, clicks_asked: bool) -> bool {
    person || clicks_asked
}

/// May this instance have a tray icon? A person's launch, or a script's that set [`TRAY_ENV`].
/// Read once.
pub(crate) fn allowed() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    *ALLOWED.get_or_init(|| {
        let asked = std::env::var(TRAY_ENV).map(|v| v == "1").unwrap_or(false);
        let person = !crate::engine::launch_focus::launch_in_background();
        if !person && !asked {
            log::info!("tray: none for this instance: a script launched it ({TRAY_ENV}=1 asks for one)");
        }
        person || asked
    })
}

/// Once for the run: the handlers that queue the tray's events, and clicks on notices turned on.
fn start() {
    tray_icon::menu::MenuEvent::set_event_handler(Some(|e: tray_icon::menu::MenuEvent| queue(Event::Menu(e.id.0))));
    tray_icon::TrayIconEvent::set_event_handler(Some(|e: tray_icon::TrayIconEvent| {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent as E};
        // Only a click opens the window; the pointer passing over the icon is not an event of ours.
        match e {
            E::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | E::DoubleClick { button: MouseButton::Left, .. } => queue(Event::Icon),
            _ => {}
        }
    }));
    let person = !crate::engine::launch_focus::launch_in_background();
    let clicks_asked = std::env::var(TRAY_CLICKS_ENV).map(|v| v == "1").unwrap_or(false);
    if serves_clicks(person, clicks_asked) {
        n::toast::enable_clicks(Box::new(crate::engine::tray_window::wake));
    } else {
        log::info!("notifications: a script's instance serves no clicks on notices ({TRAY_CLICKS_ENV}=1 asks for it)");
    }
}

/// Can the person see the icon? Made, and on Linux a tray host registered with the
/// StatusNotifierWatcher: tray-icon's `ksni` path counts the icon made with no host at all (it
/// assumes one), so on a desktop with no tray (GNOME without its AppIndicator extension) the window
/// could hide with no icon anywhere and no way back. Asked when the window is closed (a process
/// start, as notices on Linux are: `busctl`, else `gdbus`, else `dbus-send`; none of them, or no
/// watcher, is no host). Elsewhere the icon that was made is seen.
pub(crate) fn icon_can_be_seen() -> bool {
    #[cfg(target_os = "linux")]
    {
        let seen = linux_tray_host();
        log::info!("tray: a tray host is {}registered on this desktop", if seen { "" } else { "NOT " });
        seen
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Is a StatusNotifierHost registered (Linux)? Through the first of three D-Bus command-line tools
/// present; none present is no.
#[cfg(target_os = "linux")]
fn linux_tray_host() -> bool {
    const W: &str = "org.kde.StatusNotifierWatcher";
    const PROP: &str = "IsStatusNotifierHostRegistered";
    let tries: [(&str, Vec<&str>); 3] = [
        ("busctl", vec!["--user", "get-property", W, "/StatusNotifierWatcher", W, PROP]),
        ("gdbus", vec!["call", "--session", "--dest", W, "--object-path", "/StatusNotifierWatcher", "--method", "org.freedesktop.DBus.Properties.Get", W, PROP]),
        ("dbus-send", vec!["--session", "--print-reply", "--dest=org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.freedesktop.DBus.Properties.Get", "string:org.kde.StatusNotifierWatcher", "string:IsStatusNotifierHostRegistered"]),
    ];
    for (tool, args) in tries {
        let out = std::process::Command::new(tool)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        match out {
            // The tool is there: its answer decides (a failure is no watcher, so no host).
            Ok(o) => return o.status.success() && n::tray::sni_host_answer(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => continue,
        }
    }
    false
}

/// Once per frame (from `frame_ws_poll::poll_relay_messages`) and once per tick while hidden: the
/// icon made or taken down as Settings says, its clicks and the notices' clicks acted on, and its
/// look brought up to date. Constant time when nothing changed.
pub(crate) fn pump(state: &mut EngineState) {
    if !allowed() {
        return;
    }
    if !state.tray.started {
        state.tray.started = true;
        start();
    }
    let want = state.gui_state.settings.notifications.tray;
    if want && state.tray.icon.is_none() && !state.tray.failed {
        make(state);
    } else if !want && state.tray.icon.is_some() {
        state.tray.icon = None;
        log::info!("tray: the icon was taken down (Settings)");
        // Without the icon there is no way back to a hidden window.
        if state.tray.hidden {
            crate::engine::tray_window::show(state, false);
        }
    }
    for e in take_events() {
        match e {
            Event::Icon => crate::engine::tray_window::show(state, true),
            Event::Menu(id) => match n::tray::action(&id, n::data::get()) {
                Some(a) => act(state, a),
                None => log::debug!("tray: menu line {id:?} does nothing"),
            },
        }
    }
    for launch in n::toast::take_clicks() {
        open_click(state, &launch);
    }
    refresh(state);
    poll_dev_request(state);
}

/// The dev IPC's door to the tray (`debug/tray_request.json`, answered in `debug/tray_done.json`),
/// for a rig: a real tray menu, notice or flash cannot be clicked by a script, so this does what
/// each click does, through the same code, and reports what the tray and the window now are.
/// `action`: `state`; `close` (the window's close button); `menu` with `id` (a menu line: `open`,
/// `mute:1h`, `unmute`, `quit`); `icon` (a click on the icon); `click` with `launch` (a click on a
/// notice, the token as Windows hands it back); `notice` (a made-up notice for `kind`, `sender`,
/// `room`, `text`, through THE rule, or past it with `force`, to see its surfaces); `voice_close`
/// (a stand-in room and call, then the close button, to see voice left; no microphone is opened);
/// `setting` with
/// any of `tray`, `close_to_tray`, `desktop` (booleans) and `show` (the rig's own sandbox settings).
const DEV_REQUEST: &str = "debug/tray_request.json";
const DEV_DONE: &str = "debug/tray_done.json";

fn poll_dev_request(state: &mut EngineState) {
    let Some(text) = crate::engine::ipc::take_request_text(std::path::Path::new(DEV_REQUEST)) else { return };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut extra = serde_json::json!({});
    match s("action").as_str() {
        "state" | "" => {}
        "close" => {
            if !crate::engine::tray_window::close_to_tray(state) {
                state.gui_state.quit_requested = true;
            }
        }
        "voice_close" => {
            // A rig's check that closing into the tray leaves voice, with no microphone ever
            // opened: a stand-in room and call are set and the window closed in the SAME call,
            // before any frame could start a voice session from them; whatever the close did not
            // take down is cleared here at once (the critic's review of part 2, 2026-10-31).
            let gs = &mut state.gui_state;
            gs.voice_active_room = Some("rig-voice-room".into());
            gs.call_active = Some(("rig-peer".into(), "Rig peer".into()));
            let hid = crate::engine::tray_window::close_to_tray(state);
            let gs = &mut state.gui_state;
            let left_by_close = gs.voice_active_room.is_none() && gs.call_active.is_none();
            if !left_by_close {
                crate::gui::pages::chat::leave_all_voice(gs);
            }
            extra = serde_json::json!({ "voice_close": { "hid": hid, "left_by_close": left_by_close } });
        }
        "menu" => match n::tray::action(&s("id"), n::data::get()) {
            Some(a) => act(state, a),
            None => extra = serde_json::json!({ "error": format!("no menu line {:?}", s("id")) }),
        },
        "icon" => crate::engine::tray_window::show(state, true),
        "click" => open_click(state, &s("launch")),
        "setting" => {
            // A rig's own sandbox settings: the tray, closing into it, desktop notices, what one shows.
            let l = &mut state.gui_state.settings.notifications;
            let b = |k: &str| v.get(k).and_then(|x| x.as_bool());
            if let Some(x) = b("tray") {
                l.tray = x;
            }
            if let Some(x) = b("close_to_tray") {
                l.close_to_tray = x;
            }
            if let Some(x) = b("desktop") {
                l.desktop = x;
            }
            if let Some(x) = v.get("show").and_then(|x| x.as_str()) {
                l.show = n::Show::from_id(x);
            }
            save_settings(state);
        }
        "notice" => {
            let server = crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url);
            let room = s("room");
            let kind = if s("kind").is_empty() { "mention".to_string() } else { s("kind") };
            let c = n::Candidate {
                conversation: if kind == "dm" || kind == "contact_request" { format!("dm:{}", s("sender")) } else { room.clone() },
                room: (!room.is_empty()).then(|| room.clone()),
                room_name: room,
                sender: s("sender"),
                text: s("text"),
                kind,
                server,
                ..Default::default()
            };
            let now = n::now_secs();
            let out = if v.get("force").and_then(|f| f.as_bool()).unwrap_or(false) {
                let l = &state.gui_state.settings.notifications;
                n::Outcome { why: n::Why::Notify, toast: l.desktop && !state.window_focused, sound: l.sound, list: true }
            } else {
                crate::engine::notify::outcome(state, &c, now)
            };
            let before = state.gui_state.notify.targets.last_token();
            crate::engine::notify::act_on(state, c, out, now);
            let token = state.gui_state.notify.targets.last_token().filter(|t| Some(t) != before.as_ref());
            extra = serde_json::json!({ "why": out.why.code(), "toast": out.toast, "launch": token, "banner": state.gui_state.notify.banner.is_some() });
        }
        other => extra = serde_json::json!({ "error": format!("unknown action {other:?}: state, close, voice_close, menu, icon, click, notice, setting") }),
    }
    let look = Look::of(&state.gui_state.settings.notifications.mutes, state.gui_state.notify.list.unseen(), n::now_secs());
    let mut done = serde_json::json!({
        "ok": true,
        "allowed": allowed(),
        "icon_shown": state.tray.shown(),
        "hidden": state.tray.hidden,
        "visible": state.window.is_visible(),
        "ms_since_last_frame": state.last_frame.elapsed().as_millis() as u64,
        "world_loaded": state.world_loaded,
        "page": format!("{:?}", state.gui_state.active_page),
        "quit_requested": state.gui_state.quit_requested,
        "close_to_tray": state.gui_state.settings.notifications.close_to_tray,
        "muted_until": look.muted,
        "unread": look.unread,
        "tooltip": look.tooltip(),
        "menu": format!("{:?}", look.menu(n::data::get())),
        "chat_channel": state.gui_state.chat_active_channel,
        "voice": {
            "room": state.gui_state.voice_active_room,
            "call": state.gui_state.call_active.as_ref().map(|c| c.0.clone()),
            "wanted": crate::gui::pages::chat::voice_wanted(&state.gui_state),
            "session_running": crate::net::voice::voice_session_running(),
            "mic_test": state.gui_state.mic_test_active,
        },
        "pending_notices": state.gui_state.pending_notices.len(),
    });
    if let (Some(d), Some(e)) = (done.as_object_mut(), extra.as_object()) {
        for (k, val) in e {
            d.insert(k.clone(), val.clone());
        }
    }
    let _ = std::fs::create_dir_all("debug");
    let tmp = format!("{DEV_DONE}.tmp");
    if std::fs::write(&tmp, done.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, DEV_DONE);
    }
}

/// Do what a menu line asked.
fn act(state: &mut EngineState, a: Action) {
    let now = n::now_secs();
    match a {
        Action::Open => crate::engine::tray_window::show(state, true),
        Action::MuteFor(seconds) => {
            state.gui_state.settings.notifications.mutes.mute_everything(now, seconds);
            log::info!("tray: everything muted ({seconds} s, 0 = until unmuted)");
            save_settings(state);
        }
        Action::Unmute => {
            state.gui_state.settings.notifications.mutes.unmute_everything();
            log::info!("tray: everything unmuted");
            save_settings(state);
        }
        Action::Quit => {
            log::info!("tray: Quit");
            state.gui_state.quit_requested = true;
        }
    }
}

/// Write the settings now: the frame's own save (`settings_dirty`) does not run while the window
/// is hidden, and a mute is a choice the person expects kept.
fn save_settings(state: &mut EngineState) {
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
}

/// A click on one of our notices: open its conversation, or only the window.
fn open_click(state: &mut EngineState, launch: &str) {
    match state.gui_state.notify.targets.resolve(launch) {
        n::clicks::Click::Open(entry) => {
            log::info!("notifications: a click opens a {} notice's conversation", entry.kind);
            crate::engine::tray_window::show(state, true);
            crate::gui::notice_panel::open_entry(&mut state.gui_state, &entry);
        }
        n::clicks::Click::Window => crate::engine::tray_window::show(state, true),
    }
}

/// Make the icon.
fn make(state: &mut EngineState) {
    let base = base_picture();
    let look = Look::of(&state.gui_state.settings.notifications.mutes, state.gui_state.notify.list.unseen(), n::now_secs());
    let picture = n::tray::picture(&base, n::tray::ICON_PX, &look, &colours(state));
    let icon = tray_icon::Icon::from_rgba(picture, n::tray::ICON_PX, n::tray::ICON_PX).ok();
    let built = tray_icon::TrayIconBuilder::new()
        .with_tooltip(look.tooltip())
        .with_menu(Box::new(menu_of(&look.menu(n::data::get()))))
        // A left click opens the window; the menu is the right click's.
        .with_menu_on_left_click(false);
    let built = match icon {
        Some(i) => built.with_icon(i),
        None => built,
    };
    match built.build() {
        Ok(icon) => {
            log::info!("tray: the icon shows in the notification area");
            state.tray.icon = Some(Shown { icon, base, look, lang: crate::i18n::language() });
        }
        Err(e) => {
            state.tray.failed = true;
            log::warn!("tray: the icon could not be made ({e}); closing the window will quit");
        }
    }
}

/// Bring the icon's picture, tooltip and menu up to date when its look (or the language) changed.
fn refresh(state: &mut EngineState) {
    let Some(shown) = state.tray.icon.as_ref() else { return };
    let look = Look::of(&state.gui_state.settings.notifications.mutes, state.gui_state.notify.list.unseen(), n::now_secs());
    let lang = crate::i18n::language();
    if look == shown.look && std::ptr::eq(lang, shown.lang) {
        return;
    }
    let menu_changed = look.muted != shown.look.muted || !std::ptr::eq(lang, shown.lang);
    let picture = n::tray::picture(&shown.base, n::tray::ICON_PX, &look, &colours(state));
    if let Ok(icon) = tray_icon::Icon::from_rgba(picture, n::tray::ICON_PX, n::tray::ICON_PX) {
        if let Err(e) = shown.icon.set_icon(Some(icon)) {
            log::debug!("tray: the picture was not changed ({e})");
        }
    }
    let _ = shown.icon.set_tooltip(Some(look.tooltip()));
    if menu_changed {
        shown.icon.set_menu(Some(Box::new(menu_of(&look.menu(n::data::get())))));
    }
    log::info!("tray: the icon now shows {} notice(s) not opened yet{}", look.unread, if look.muted.is_some() { ", everything muted" } else { "" });
    if let Some(shown) = state.tray.icon.as_mut() {
        shown.look = look;
        shown.lang = lang;
    }
}

/// The theme's colours for the icon's marks: the unread colour of the bell's count, the card behind.
fn colours(state: &EngineState) -> n::tray::MarkColours {
    let th = &state.theme;
    n::tray::MarkColours { dot: th.danger().to_srgba_unmultiplied(), backing: th.bg_card().to_srgba_unmultiplied(), bell: th.text_primary().to_srgba_unmultiplied() }
}

/// The app's icon (the exe's own copy, the window's icon) at the tray's size.
fn base_picture() -> Vec<u8> {
    const ICON: &[u8] = include_bytes!("../../assets/icon.png");
    let px = n::tray::ICON_PX;
    match image::load_from_memory(ICON) {
        Ok(img) => image::imageops::thumbnail(&img.to_rgba8(), px, px).into_raw(),
        Err(e) => {
            log::warn!("tray: the app icon could not be read ({e}); a plain square stands in");
            vec![128; (px * px * 4) as usize]
        }
    }
}

/// The platform menu for `lines`.
fn menu_of(lines: &[MenuLine]) -> tray_icon::menu::Menu {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    let menu = Menu::new();
    for line in lines {
        let added = match line {
            MenuLine::Item { id, text, enabled } => menu.append(&MenuItem::with_id(id.as_str(), text, *enabled, None)),
            MenuLine::Separator => menu.append(&PredefinedMenuItem::separator()),
            MenuLine::Submenu { text, lines } => {
                let sub = Submenu::new(text, true);
                for l in lines {
                    let r = match l {
                        MenuLine::Item { id, text, enabled } => sub.append(&MenuItem::with_id(id.as_str(), text, *enabled, None)),
                        MenuLine::Separator => sub.append(&PredefinedMenuItem::separator()),
                        // One level deep is all the tray's menu has.
                        MenuLine::Submenu { .. } => Ok(()),
                    };
                    if let Err(e) = r {
                        log::debug!("tray: a menu line was not added ({e})");
                    }
                }
                menu.append(&sub)
            }
        };
        if let Err(e) = added {
            log::debug!("tray: a menu line was not added ({e})");
        }
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A person's launch serves clicks on notices; a script's only when it names HUMANITY_TRAY_CLICKS,
    /// never for HUMANITY_TRAY alone, so a rig with a tray beside the operator's own game never
    /// registers the activator his notices' clicks would go to. Seen red 2026-10-31 with the rule
    /// as part 2 merged it (every instance with a tray served clicks): "a rig's tray alone serves
    /// no clicks".
    #[test]
    fn a_rigs_tray_alone_serves_no_clicks() {
        assert!(serves_clicks(true, false), "a person's launch");
        assert!(!serves_clicks(false, false), "a rig's tray alone serves no clicks");
        assert!(serves_clicks(false, true), "a rig that names it");
    }

    /// The Linux tray host's answer, from each of the three D-Bus tools: yes only for a yes. Seen
    /// red 2026-10-31 with the parser reading busctl's form alone ("gdbus").
    #[test]
    fn the_tray_hosts_answer_is_read_from_each_tool() {
        use crate::notifications::tray::sni_host_answer as yes;
        assert!(yes("b true\n"), "busctl");
        assert!(yes("(<true>,)\n"), "gdbus");
        assert!(yes("method return time=1 sender=:1.1 -> destination=:1.2 serial=3 reply_serial=2\n   variant       boolean true\n"), "dbus-send");
        for no in ["b false\n", "(<false>,)\n", "   variant       boolean false\n", "", "Error: no such name"] {
            assert!(!yes(no), "{no:?}");
        }
    }
}
