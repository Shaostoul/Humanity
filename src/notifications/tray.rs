//! The notification-area (tray) icon's look and menu, worked out from the state alone (rung 2,
//! part 2, 2026-10-31; docs/design/notifications.md, "The tray"). Pure: the engine hands the result
//! to the platform (`src/engine/tray.rs`), so every rule here is a lib test.
//!
//! - **What it shows:** the app's icon; with notices not opened yet, a dot in the unread colour at
//!   its top-right; while everything is muted, the picture dimmed and a bell with a slash at its
//!   bottom-right. The tooltip says the same in words ("Muted until 14:00", "Notices: 3 not opened
//!   yet"). The count is the in-app list's unseen count (`list::List::unseen`), kept as entries
//!   come and go, so reading it each frame costs nothing.
//! - **The menu:** Open HumanityOS; Mute everything, opening to the data file's mute lengths (the
//!   same as Mute server's), or, while muted, a line saying until when that cannot be clicked and
//!   Unmute; Quit HumanityOS. Its Mute is THE Mute everything of Settings > Notifications
//!   (`Mutes::everything`), nothing new.
//! - Every word is a key in `data/i18n/en.json`.

use super::data::File;
use super::mutes::Mutes;

/// The menu's ids, which the click comes back with.
pub const OPEN: &str = "open";
pub const QUIT: &str = "quit";
pub const UNMUTE: &str = "unmute";
/// The "Muted until 14:00" line (never clickable).
pub const MUTED_LINE: &str = "muted";
/// A mute length's id is this and the length's id in the data file ("mute:1h").
pub const MUTE_PREFIX: &str = "mute:";

/// The longest tooltip Windows shows (its `szTip` holds 128 UTF-16 units with the ending zero).
pub const TOOLTIP_UNITS: usize = 127;

/// What the icon has to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Look {
    /// Everything muted until this second (0: until unmuted); `None` when not muted.
    pub muted: Option<i64>,
    /// Notices not opened yet.
    pub unread: usize,
}

impl Look {
    /// The look at `now`, from the mutes and the list's unseen count.
    pub fn of(mutes: &Mutes, unread: usize, now: i64) -> Look {
        Look { muted: mutes.everything_muted(now), unread }
    }

    /// The tooltip: the app's name, then until when everything is muted, then how many notices
    /// wait, one line each, cut to what Windows shows.
    pub fn tooltip(&self) -> String {
        let mut lines = vec![super::toast::APP_NAME.to_string()];
        if let Some(until) = self.muted {
            lines.push(muted_line(until));
        }
        if self.unread > 0 {
            lines.push(crate::gui::widgets::notice_bell::label_for(self.unread));
        }
        cut_units(&lines.join("\n"), TOOLTIP_UNITS)
    }

    /// The menu's shape and words. It changes only when the mute does (or the language), so the
    /// engine rebuilds it only then.
    pub fn menu(&self, file: &File) -> Vec<MenuLine> {
        let t = humanity_core::i18n::t;
        let item = |id: &str, text: &str, enabled: bool| MenuLine::Item { id: id.to_string(), text: text.to_string(), enabled };
        let mute = match self.muted {
            Some(until) => vec![item(MUTED_LINE, &muted_line(until), false), item(UNMUTE, t("notifications.unmute"), true)],
            None => file
                .mute_for
                .iter()
                .map(|m| item(&format!("{MUTE_PREFIX}{}", m.id), humanity_core::i18n::t_dyn(&m.label), true))
                .collect(),
        };
        vec![
            item(OPEN, t("notifications.tray_open"), true),
            MenuLine::Separator,
            MenuLine::Submenu { text: t("notifications.mute_everything").to_string(), lines: mute },
            MenuLine::Separator,
            item(QUIT, t("notifications.tray_quit"), true),
        ]
    }

    /// The dot (notices waiting) and the muted mark.
    pub fn badges(&self) -> (bool, bool) {
        (self.unread > 0, self.muted.is_some())
    }
}

/// "Muted until 14:00", or "Muted until you unmute it".
pub fn muted_line(until: i64) -> String {
    humanity_core::i18n::tf("notifications.tray_muted", &[("until", &crate::gui::pages::settings::notifications::until_words(until))])
}

/// One line of the tray's menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuLine {
    Item { id: String, text: String, enabled: bool },
    Submenu { text: String, lines: Vec<MenuLine> },
    Separator,
}

/// What a click on a menu line asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show the window (and bring it forward).
    Open,
    /// Mute everything for this many seconds (0: until unmuted).
    MuteFor(u64),
    Unmute,
    /// Save and leave, as closing the window does today.
    Quit,
}

/// The action of the line whose id is `id`; `None` for the muted line, a mute length the data
/// file no longer has, or an id that is not ours.
pub fn action(id: &str, file: &File) -> Option<Action> {
    match id {
        OPEN => Some(Action::Open),
        QUIT => Some(Action::Quit),
        UNMUTE => Some(Action::Unmute),
        _ => {
            let length = id.strip_prefix(MUTE_PREFIX)?;
            file.mute_for.iter().find(|m| m.id == length).map(|m| Action::MuteFor(m.seconds))
        }
    }
}

/// Did a D-Bus tool's answer to `IsStatusNotifierHostRegistered` say yes? `busctl` prints `b true`,
/// `gdbus` `(<true>,)`, `dbus-send` `variant boolean true`. Anything else (false, an error, nothing)
/// is no (the engine's Linux check, `tray::icon_can_be_seen`). Pure.
pub fn sni_host_answer(out: &str) -> bool {
    let t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    t == "b true" || t.contains("<true>") || t.ends_with("boolean true")
}

/// `s` cut to at most `units` UTF-16 units, never inside a character.
pub fn cut_units(s: &str, units: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        used += ch.len_utf16();
        if used > units {
            break;
        }
        out.push(ch);
    }
    out
}

// ── The icon's picture ───────────────────────────────────────────────────────────────────────────

/// How many pixels square the tray's picture is made (Windows shows it at 16 to 32 by the screen's
/// scale and shrinks it itself; the Linux tray and the macOS menu bar take any size).
pub const ICON_PX: u32 = 32;

/// The theme's colours the marks are drawn in, straight (not premultiplied) RGBA: the unread dot,
/// the ring and disc behind a mark, and the muted bell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkColours {
    pub dot: [u8; 4],
    pub backing: [u8; 4],
    pub bell: [u8; 4],
}

/// The bell's outline in a unit square: its crown flaring to the lip (the same shape the app draws,
/// `gui/widgets/icons.rs` `paint_bell`), the lip, the clapper and the slash, with the stroke's width.
const BELL_BODY: [(f32, f32); 7] = [(0.20, 0.72), (0.24, 0.42), (0.34, 0.22), (0.50, 0.14), (0.66, 0.22), (0.76, 0.42), (0.80, 0.72)];
const BELL_LIP: [(f32, f32); 2] = [(0.10, 0.74), (0.90, 0.74)];
const BELL_CLAPPER: ((f32, f32), f32) = ((0.50, 0.87), 0.08);
const BELL_SLASH: [(f32, f32); 2] = [(0.08, 0.08), (0.92, 0.94)];
const BELL_STROKE: f32 = 0.11;

/// The tray's picture for `look`: `base` (the app icon, `size` square, straight RGBA) with its
/// marks. Pure, so the picture is a lib test; drawn only when the look changes.
pub fn picture(base: &[u8], size: u32, look: &Look, c: &MarkColours) -> Vec<u8> {
    let mut px = base.to_vec();
    let (dot, muted) = look.badges();
    let s = size as f32;
    if muted {
        // Dimmed and drained of colour: a muted app reads as quiet even at 16 pixels.
        for p in px.chunks_exact_mut(4) {
            let grey = (p[0] as f32 * 0.30 + p[1] as f32 * 0.59 + p[2] as f32 * 0.11) * 0.6;
            for ch in p.iter_mut().take(3) {
                *ch = grey as u8;
            }
        }
        // The bell with a slash on a disc at the bottom-right corner.
        let (cx, cy, r) = (s * 0.70, s * 0.70, s * 0.30);
        disc(&mut px, size, (cx, cy), r, c.backing);
        let box_ = (cx - r * 0.72, cy - r * 0.72, r * 1.44);
        let to = |(x, y): (f32, f32)| (box_.0 + x * box_.2, box_.1 + y * box_.2);
        let w = (BELL_STROKE * box_.2).max(1.0);
        let body: Vec<(f32, f32)> = BELL_BODY.iter().copied().map(to).collect();
        for seg in body.windows(2) {
            stroke(&mut px, size, seg[0], seg[1], w, c.bell);
        }
        stroke(&mut px, size, to(BELL_LIP[0]), to(BELL_LIP[1]), w, c.bell);
        disc(&mut px, size, to(BELL_CLAPPER.0), BELL_CLAPPER.1 * box_.2, c.bell);
        stroke(&mut px, size, to(BELL_SLASH[0]), to(BELL_SLASH[1]), w, c.bell);
    }
    if dot {
        // The unread dot at the top-right corner, ringed so it reads on any taskbar.
        let (cx, cy, r) = (s * 0.76, s * 0.24, s * 0.22);
        disc(&mut px, size, (cx, cy), r + (s * 0.05).max(1.0), c.backing);
        disc(&mut px, size, (cx, cy), r, c.dot);
    }
    px
}

/// Paint `colour` over pixel `(x, y)` at `cover` (0..1).
fn blend(px: &mut [u8], size: u32, x: u32, y: u32, colour: [u8; 4], cover: f32) {
    let a = (cover.clamp(0.0, 1.0)) * colour[3] as f32 / 255.0;
    if a <= 0.0 {
        return;
    }
    let i = ((y * size + x) * 4) as usize;
    let dst_a = px[i + 3] as f32 / 255.0;
    let out_a = a + dst_a * (1.0 - a);
    for ch in 0..3 {
        let src = colour[ch] as f32;
        let dst = px[i + ch] as f32;
        let v = if out_a > 0.0 { (src * a + dst * dst_a * (1.0 - a)) / out_a } else { 0.0 };
        px[i + ch] = v.round().clamp(0.0, 255.0) as u8;
    }
    px[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// A filled disc, its edge smoothed over one pixel.
fn disc(px: &mut [u8], size: u32, (cx, cy): (f32, f32), r: f32, colour: [u8; 4]) {
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            blend(px, size, x, y, colour, r - d + 0.5);
        }
    }
}

/// A stroke from `a` to `b`, `w` wide with round ends, its edge smoothed over one pixel.
fn stroke(px: &mut [u8], size: u32, a: (f32, f32), b: (f32, f32), w: f32, colour: [u8; 4]) {
    let (ex, ey) = (b.0 - a.0, b.1 - a.1);
    let len2 = (ex * ex + ey * ey).max(1e-6);
    for y in 0..size {
        for x in 0..size {
            let (px_, py_) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = (((px_ - a.0) * ex + (py_ - a.1) * ey) / len2).clamp(0.0, 1.0);
            let (qx, qy) = (a.0 + t * ex - px_, a.1 + t * ey - py_);
            let d = (qx * qx + qy * qy).sqrt();
            blend(px, size, x, y, colour, w * 0.5 - d + 0.5);
        }
    }
}
