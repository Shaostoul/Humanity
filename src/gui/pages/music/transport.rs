//! THE TRANSPORT BAR, AT THE BOTTOM (docs/design/music-page.md section 28.5, version 3; the operator,
//! 2026-11-21: "Bottom would probably be best for phone users and most consistent across all other
//! media players like VLC." and "Let's add icons for the buttons too.").
//!
//! Along the bottom of the Music page on every tab, always in view: the seek bar with the position and
//! the length across the top of the bar (a station has no end, and says so), then Previous, Play or
//! Pause, Next, Shuffle and Repeat, what plays in words, Like, Up next and the volume. Every button is
//! an icon drawn as shapes (`widgets::transport_icons`), named in words for assistive technology and
//! with its hint. Next and Previous go through the ONE queue for every kind of music (queue.rs): Up
//! next, then the station. Up next opens the list above the bar ([`up_next_panel`]).

use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use humanity_core::i18n::t;

use super::queue::{Item, Repeat};
use super::{genre_label, mood_words, now_playing_words, score_words, Shown};
use crate::gui::theme::Theme;
use crate::gui::widgets::transport_icons::{IconButton, TransportIcon};
use crate::gui::widgets::{self, Button, ButtonSize, Hint};
use crate::gui::GuiState;

/// The seek bar's egui id (a test reads where it was drawn).
pub const SEEK_ID: &str = "music_t_seek";
/// The volume slider's egui id.
pub const VOLUME_ID: &str = "music_t_volume";

/// The ids of the bar's buttons (`IconButton::new`'s), in the order they are drawn.
pub const PREV: &str = "music_t_prev";
pub const PLAY: &str = "music_t_play";
pub const NEXT: &str = "music_t_next";
pub const SHUFFLE: &str = "music_t_shuffle";
pub const REPEAT: &str = "music_t_repeat";
pub const LIKE: &str = "music_t_like";
pub const UP_NEXT: &str = "music_t_up_next";
pub const MUTE: &str = "music_t_mute";

/// The egui id an icon button of the bar is drawn under.
pub fn button_id(name: &str) -> egui::Id {
    egui::Id::new(("icon_button", name))
}

/// How long the endless round is, minutes (a composed piece's length when "A new piece" is Never).
pub fn endless(state: &GuiState) -> f32 {
    state.music_dev.readout.lib.as_ref().map(|l| l.endless_minutes).unwrap_or(8.0)
}

/// A genre's words from the data (its key, else its plain name, else its id).
fn genre_words(state: &GuiState, id: &str) -> String {
    match state.music_dev.readout.lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == id)) {
        Some(g) => genre_label(&g.id, &g.words, &g.name),
        None => id.replace('_', " "),
    }
}

/// An item in words: a written piece's title, "Electro swing, seed 4242", "Lo-fi station, Mothership
/// calm".
pub fn item_words(state: &GuiState, item: &Item) -> String {
    let lib = state.music_dev.readout.lib.clone();
    match item {
        Item::Written { id } => super::files::entry_of(state, id).map(|e| score_words(&e).0).unwrap_or_else(|| id.replace('_', " ")),
        Item::Composed { genre, seed, .. } => t("music_page.item_composed").replace("{{genre}}", &genre_words(state, genre)).replace("{{seed}}", &seed.to_string()),
        Item::Station { genre, mood, .. } => t("music_page.item_station").replace("{{genre}}", &genre_words(state, genre)).replace("{{mood}}", &mood_words(lib.as_deref(), mood)),
    }
}

/// An item and its kind in words ("Ode to Joy, a written piece").
pub fn item_with_kind(state: &GuiState, item: &Item) -> String {
    t("music_page.item_with_kind").replace("{{what}}", &item_words(state, item)).replace("{{kind}}", humanity_core::i18n::t_dyn(item.kind_key()))
}

/// What the bar says plays: the sentence that says what plays and who chose it, the very words
/// Settings > Music shows (`now_playing_words`, one function for both pages); while the danger breaks
/// in, that it does and that your music comes back. (Up next names each item with its kind.)
pub fn what_plays(state: &GuiState) -> String {
    let r = &state.music_dev.readout;
    if r.danger {
        return t("music_page.q_danger").to_string();
    }
    now_playing_words(&state.settings.music, r)
}

/// Seconds as "m:ss" (or "h:mm:ss" past an hour).
pub fn time_words(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Where the music is in the piece playing and how long the piece is (None: it plays on, a station or
/// the game's piece with "A new piece" set to Never).
pub fn position(state: &GuiState) -> (f64, Option<f64>) {
    let r = &state.music_dev.readout;
    let pos = r.score.map(|s| s.seconds).unwrap_or(0.0);
    if let Some(p) = r.written_piece.as_ref() {
        return (pos, Some(p.timeline.length_s.max(0.1)));
    }
    let len = r.playing.as_ref().and_then(|p| p.choice.as_ref()).and_then(|c| c.length_s).map(f64::from);
    (pos, len)
}

/// Play the piece from the bar holding second `t` (the bar's start: a seek moves to a bar line, as a
/// click on the piano roll does).
pub fn seek_to_time(state: &mut GuiState, t: f64) {
    let Some(bar) = state.music_page.sheet.as_ref().map(|c| c.sheet.bars.partition_point(|&b| b <= t + 1e-6).saturating_sub(1)) else { return };
    state.music_page.seek_to(bar);
}

/// Next, the same for every kind (queue.rs). From the game's own music with no station of the person's
/// yet, the station of what plays (its genre and the game's mood) plays its next piece: "skip this
/// piece" gives one like it.
pub fn next(state: &mut GuiState) {
    let e = endless(state);
    let s = &state.settings.music;
    let q = &mut state.music_page.queue;
    if s.game_chooses && s.written.is_empty() && q.up_next.is_empty() && q.station.is_none() {
        let sh = super::shown(s, &state.music_dev.readout);
        q.station = Some(Item::Station { genre: sh.genre, mood: sh.mood, seed: sh.seed, tune: humanity_music::favourite::SavedTune::default() });
    }
    state.music_page.queue.next(&mut state.settings.music, e);
    state.settings_dirty = true;
    state.music_page.save_queue();
}

/// Previous, the same for every kind (queue.rs).
pub fn previous(state: &mut GuiState) {
    let e = endless(state);
    if state.music_page.queue.previous(&mut state.settings.music, e) {
        state.settings_dirty = true;
        state.music_page.save_queue();
    }
}

/// Shuffle on or off.
pub fn toggle_shuffle(state: &mut GuiState) {
    state.music_page.queue.shuffle = !state.music_page.queue.shuffle;
    state.music_page.save_queue();
}

/// Repeat: off, all, one, off.
pub fn cycle_repeat(state: &mut GuiState) {
    state.music_page.queue.repeat = state.music_page.queue.repeat.next();
    state.music_page.save_queue();
}

/// Play or pause the music everywhere (Settings > Music's own switch).
pub fn play_pause(state: &mut GuiState) {
    state.settings.music.music_on = !state.settings.music.music_on;
    state.settings_dirty = true;
}

/// "Add to Up next" for `item`, where it is listed (written pieces, favourites, pieces heard).
pub fn add_button(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: Item) {
    if Button::new(t("music_page.q_add")).size(ButtonSize::Small).hint("music_page.q_add_hint").show(ui, theme) {
        let words = item_words(state, &item);
        state.music_page.queue.add(item);
        state.music_page.queue_msg = Some(t("music_page.q_added").replace("{{what}}", &words));
        state.music_page.save_queue();
    }
}

/// The seek bar: the position and the length either side of a bar a click or a drag moves along (a
/// station's words in place of the length). Arrow keys move five seconds while it has the keyboard.
fn seek_bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, width: f32) {
    let (pos, len) = position(state);
    let small = theme.font_size_small;
    ui.horizontal(|ui| {
        ui.label(RichText::new(time_words(pos)).size(small).color(theme.text_secondary()));
        let Some(len) = len else {
            // A station, or the game's piece that plays on: no end to seek in, said in words.
            let minutes = super::queue::station_minutes(&state.settings.music);
            let words = if minutes > 0.0 { t("music_page.t_station_no_end").replace("{{n}}", &format!("{minutes:.0}")) } else { t("music_page.t_no_end").to_string() };
            ui.label(RichText::new(words).size(small).color(theme.text_muted()));
            return;
        };
        let w = (width - 2.0 * 64.0).max(120.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
        let resp = ui.interact(rect, egui::Id::new(SEEK_ID), Sense::click_and_drag());
        let frac = (pos / len).clamp(0.0, 1.0) as f32;
        let words = t("music_page.t_seek").to_string();
        resp.widget_info(|| egui::WidgetInfo::slider(true, frac as f64, &words));
        let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 6.0));
        let painter = ui.painter_at(rect.expand(8.0));
        // The rail in the sliders' own colour, so the part still to play reads on a dark card.
        painter.rect_filled(track, 3.0, theme.slider_track());
        // While dragged, where it would go; else where the music is.
        let shown = match resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            Some(p) => ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
            None => frac,
        };
        painter.rect_filled(Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * shown, track.bottom())), 3.0, theme.accent());
        painter.circle(Pos2::new(track.left() + track.width() * shown, track.center().y), 7.0, theme.accent(), Stroke::new(1.5_f32, theme.text_on_accent()));
        let to = if resp.clicked() || resp.drag_stopped() { resp.interact_pointer_pos().map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * len) } else { None };
        let keyed = if resp.has_focus() {
            let back = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
            let on = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight));
            if back { Some((pos - 5.0).max(0.0)) } else if on { Some((pos + 5.0).min(len)) } else { None }
        } else {
            None
        };
        let resp = resp.hint("music_page.t_seek_hint");
        widgets::focus_ring(ui, theme, &resp);
        if let Some(t) = to.or(keyed) {
            seek_to_time(state, t);
        }
        ui.label(RichText::new(time_words(len)).size(small).color(theme.text_secondary()));
    });
}

/// The bar. `sh` is the piece as the page shows it (Like keeps it). Wide: the seek bar, then one row
/// of the playing buttons at the left, what plays filling the middle, and Like, Up next and the volume
/// at the right (laid out from the right, so they never run off the edge). Narrow (a phone): what
/// plays, the seek bar, a row of buttons, then the volume on its own row.
pub(super) fn bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, wide: bool) {
    let small = theme.font_size_small;
    let size = if wide { 44.0 } else { 36.0 };
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        let width = ui.available_width();
        if !wide {
            ui.label(RichText::new(what_plays(state)).strong().color(theme.text_primary()));
        }
        seek_bar(ui, theme, state, width);
        if wide {
            ui.horizontal(|ui| {
                playing_buttons(ui, theme, state, size);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    volume(ui, theme, state, size - 8.0, true);
                    ui.add_space(theme.spacing_sm);
                    list_buttons(ui, theme, state, sh, size - 4.0, true);
                    ui.add_space(theme.spacing_md);
                    // What plays, in the room left between the two groups.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add_space(theme.spacing_md);
                        ui.add(egui::Label::new(RichText::new(what_plays(state)).strong().color(theme.text_primary())).truncate());
                    });
                });
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                playing_buttons(ui, theme, state, size);
                list_buttons(ui, theme, state, sh, size - 4.0, false);
            });
            ui.horizontal(|ui| volume(ui, theme, state, size - 6.0, false));
        }
        if let Some(m) = state.music_page.like_msg.clone() {
            ui.label(RichText::new(m).size(small).color(theme.accent()));
        }
        if state.settings.music.music_on && !state.music_dev.readout.device {
            ui.label(RichText::new(t("music_page.no_device")).size(small).color(theme.text_muted()));
        }
    });
}

/// Previous, Play or Pause (the larger round button), Next, Shuffle and Repeat.
fn playing_buttons(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, size: f32) {
    let on = state.settings.music.music_on;
    if IconButton::new(TransportIcon::Previous, PREV, t("music_page.t_prev")).hint("music_page.t_prev_hint").size(size).show(ui, theme).clicked() {
        previous(state);
    }
    let (icon, name, hint) = if on { (TransportIcon::Pause, t("music_page.t_pause"), "music_page.t_pause_hint") } else { (TransportIcon::Play, t("music_page.t_play"), "music_page.t_play_hint") };
    if IconButton::new(icon, PLAY, name).hint(hint).size(size + 8.0).primary(true).show(ui, theme).clicked() {
        play_pause(state);
    }
    if IconButton::new(TransportIcon::Next, NEXT, t("music_page.t_next")).hint("music_page.t_next_hint").size(size).show(ui, theme).clicked() {
        next(state);
    }
    ui.add_space(theme.spacing_sm);
    let shuffle = state.music_page.queue.shuffle;
    let (name, hint) = if shuffle { (t("music_page.t_shuffle_on"), "music_page.t_shuffle_on_hint") } else { (t("music_page.t_shuffle_off"), "music_page.t_shuffle_off_hint") };
    if IconButton::new(TransportIcon::Shuffle, SHUFFLE, name).hint(hint).size(size - 4.0).active(shuffle).show(ui, theme).clicked() {
        toggle_shuffle(state);
    }
    let repeat = state.music_page.queue.repeat;
    let (icon, name, hint) = match repeat {
        Repeat::Off => (TransportIcon::Repeat, t("music_page.t_repeat_off"), "music_page.t_repeat_off_hint"),
        Repeat::All => (TransportIcon::Repeat, t("music_page.t_repeat_all"), "music_page.t_repeat_all_hint"),
        Repeat::One => (TransportIcon::RepeatOne, t("music_page.t_repeat_one"), "music_page.t_repeat_one_hint"),
    };
    if IconButton::new(icon, REPEAT, name).hint(hint).size(size - 4.0).active(repeat != Repeat::Off).show(ui, theme).clicked() {
        cycle_repeat(state);
    }
}

/// Like and Up next (`rtl`: in a right-to-left run, so Up next is added first to sit rightmost).
fn list_buttons(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, size: f32, rtl: bool) {
    let up_next = |ui: &mut egui::Ui, state: &mut GuiState| {
        let open = state.music_page.up_next_open;
        if IconButton::new(TransportIcon::UpNext, UP_NEXT, t("music_page.t_up_next")).hint("music_page.t_up_next_hint").size(size).active(open).show(ui, theme).clicked() {
            state.music_page.up_next_open = !open;
        }
    };
    if rtl {
        up_next(ui, state);
    }
    let written = !state.settings.music.written.is_empty();
    let f = super::like::liked_favourite(state, sh);
    let liked = !written && super::like::already_liked(state, &f).is_some();
    let (icon, name, hint) = if written {
        (TransportIcon::Heart, t("music_page.t_like"), "music_page.like_written_hint")
    } else if liked {
        (TransportIcon::HeartFilled, t("music_page.t_liked"), "music_page.liked_hint")
    } else {
        (TransportIcon::Heart, t("music_page.t_like"), "music_page.like_hint")
    };
    if IconButton::new(icon, LIKE, name).hint(hint).size(size).active(liked).enabled(!written).show(ui, theme).clicked() {
        super::like::like(state, sh);
    }
    if !rtl {
        up_next(ui, state);
    }
}

/// The volume: the speaker (a click silences the music and a second brings it back to where it was)
/// and the Music Volume slider Settings > Audio keeps (`rtl`: the slider added first, so the speaker
/// sits before it on the screen).
fn volume(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, size: f32, rtl: bool) {
    let speaker = |ui: &mut egui::Ui, state: &mut GuiState| {
        let v = state.settings.music_volume;
        let (icon, name, hint) = if v <= 0.0 { (TransportIcon::SpeakerMuted, t("music_page.t_unmute"), "music_page.t_unmute_hint") } else { (TransportIcon::Speaker, t("music_page.t_mute"), "music_page.t_mute_hint") };
        if IconButton::new(icon, MUTE, name).hint(hint).size(size).show(ui, theme).clicked() {
            if v > 0.0 {
                state.music_page.volume_before_mute = v;
                state.settings.music_volume = 0.0;
            } else {
                state.settings.music_volume = if state.music_page.volume_before_mute > 0.0 { state.music_page.volume_before_mute } else { 0.7 };
            }
            state.settings_dirty = true;
        }
    };
    let slider = |ui: &mut egui::Ui, state: &mut GuiState| {
        let mut vol = state.settings.music_volume;
        ui.spacing_mut().slider_width = 140.0;
        // (In the right-to-left run its name sits before its rail; never wrapped in a layout of its own,
        // which would take the whole row's room and push the buttons onto the others.)
        let r = ui.push_id(VOLUME_ID, |ui| widgets::slider(ui, theme, egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false).text(t("music_page.t_volume")))).inner;
        if r.hint("music_page.t_volume_hint").changed() {
            state.settings.music_volume = vol;
            state.settings_dirty = true;
        }
    };
    if rtl {
        slider(ui, state);
        speaker(ui, state);
    } else {
        speaker(ui, state);
        slider(ui, state);
    }
}

/// UP NEXT, opened from the bar: the item playing, Up next with Play now, Up, Down and Remove on each,
/// the station at the end, Clear, and how far Previous goes back.
pub(super) fn up_next_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let small = theme.font_size_small;
    let mut play: Option<usize> = None;
    let mut up: Option<usize> = None;
    let mut down: Option<usize> = None;
    let mut remove: Option<usize> = None;
    let mut clear = false;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.q_title")).size(theme.font_size_heading).strong().color(theme.text_primary()));
            if !state.music_page.queue.up_next.is_empty() && Button::new(t("music_page.q_clear")).size(ButtonSize::Small).hint("music_page.q_clear_hint").show(ui, theme) {
                clear = true;
            }
        });
        if let Some(e) = &state.music_page.queue_error {
            ui.label(RichText::new(t("music_page.q_error").replace("{{error}}", e)).color(theme.warning()));
        }
        if let Some(now) = state.music_page.queue.now.clone() {
            ui.label(RichText::new(t("music_page.q_now").replace("{{what}}", &item_with_kind(state, &now))).color(theme.text_secondary()));
        }
        let items = state.music_page.queue.up_next.clone();
        if items.is_empty() {
            ui.label(RichText::new(t("music_page.q_empty")).color(theme.text_muted()));
        }
        let n = items.len();
        for (i, item) in items.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{}. {}", i + 1, item_with_kind(state, item))).color(theme.text_primary()));
                if Button::new(t("music_page.q_play_now")).size(ButtonSize::Small).hint("music_page.q_play_now_hint").show(ui, theme) {
                    play = Some(i);
                }
                if i > 0 && Button::new(t("music_page.q_up")).size(ButtonSize::Small).hint("music_page.q_up_hint").show(ui, theme) {
                    up = Some(i);
                }
                if i + 1 < n && Button::new(t("music_page.q_down")).size(ButtonSize::Small).hint("music_page.q_down_hint").show(ui, theme) {
                    down = Some(i);
                }
                if Button::new(t("music_page.q_remove")).size(ButtonSize::Small).hint("music_page.q_remove_hint").show(ui, theme) {
                    remove = Some(i);
                }
            });
        }
        let station = state.music_page.queue.station_or_default(&state.settings.music);
        ui.label(RichText::new(t("music_page.q_station").replace("{{what}}", &item_words(state, &station))).color(theme.text_secondary()));
        let back = state.music_page.queue.history.len();
        ui.label(RichText::new(t("music_page.q_history").replace("{{n}}", &back.to_string())).size(small).color(theme.text_muted()));
        if let Some(m) = &state.music_page.queue_msg {
            ui.label(RichText::new(m).size(small).color(theme.accent()));
        }
    });
    let e = endless(state);
    let q = &mut state.music_page.queue;
    let mut changed = true;
    if clear {
        q.up_next.clear();
    } else if let Some(i) = up {
        q.move_item(i, i - 1);
    } else if let Some(i) = down {
        q.move_item(i, i + 1);
    } else if let Some(i) = remove {
        q.up_next.remove(i);
    } else if let Some(i) = play {
        let item = q.up_next.remove(i);
        q.play_now(item, &mut state.settings.music, e);
        state.settings_dirty = true;
    } else {
        changed = false;
    }
    if changed {
        state.music_page.save_queue();
    }
}
