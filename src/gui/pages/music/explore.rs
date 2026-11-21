//! EXPLORE MADE PLAIN (docs/design/music-page.md section 9; the operator, 2026-11-20: "We need to
//! find some way to make the explore section more intuitive. What each style means and mood is kind
//! of confusing"). A card for each genre (its name, ONE plain sentence of what it is from its data
//! file, its tempo and the instruments it plays) and each mood (its name, where it plays, its tempo),
//! the seed, and the pieces heard (kept across runs since rung 2: history.rs). Every card comes from the
//! music's data: a new genre or mood file gets its card with no code change (infinite-of-X). Clicking
//! a card chooses it; the choice plays from the next bar, everywhere (the page's one genre control is
//! these cards), in place of a written piece the person chose (BUG-343: a click on a card, chosen or
//! not, is a choice of composed music). Each card's LISTEN (rung 2, the operator's "Agree") plays 20
//! seconds of a piece in it and then what played before comes back, without changing the person's
//! choice.

use egui::{Color32, RichText, Stroke};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::favourite::explore_intensity;
use humanity_music::tune::Tune;
use humanity_music::MusicLibrary;

use super::history::Heard;
use super::{genre_label, mood_words, score_words, voice_words, Audition, Shown, AUDITION_S, RANDOM_SEED_MAX};
use crate::gui::pages::dev_music::{GenreChoice, MusicChoice};
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState};

/// A card's width on a wide page, points (as many columns as fit).
const CARD_W: f32 = 330.0;

/// One genre's card, as the data says it: its name, its plain sentence, its tempo, its instruments,
/// and its longer "about" (shown on the chosen card).
#[derive(Clone, Debug, PartialEq)]
pub struct GenreCard {
    pub id: String,
    pub name: String,
    pub plain: String,
    pub tempo: String,
    pub instruments: String,
    pub about: String,
}

/// A tempo range in words.
fn tempo_words((lo, hi): (f32, f32)) -> String {
    if (hi - lo).abs() < 1.0 {
        t("music_page.card_tempo_one").replace("{{bpm}}", &format!("{lo:.0}"))
    } else {
        t("music_page.card_tempo").replace("{{lo}}", &format!("{lo:.0}")).replace("{{hi}}", &format!("{hi:.0}"))
    }
}

/// The instruments a genre plays, by their words: its lead voices, its palette and its added parts
/// (a part's words where it names one), else the mood's own when the genre adds none (the ship's own
/// genre plays the mood as written).
fn instrument_words(lib: &MusicLibrary, g: &humanity_music::genre::Genre, mood: &str) -> String {
    let inst = |id: &str| lib.instrument_index(id).map(|i| t_dyn(&lib.instruments[i].words).to_string()).filter(|w| !w.is_empty() && !w.contains('.')).unwrap_or_else(|| id.replace('_', " "));
    let mut named: Vec<String> = Vec::new();
    let add = |named: &mut Vec<String>, w: String| {
        if !named.contains(&w) {
            named.push(w);
        }
    };
    for l in &g.leads {
        add(&mut named, l.part.as_deref().map(voice_words).unwrap_or_else(|| inst(&l.instrument)));
    }
    for p in &g.palette {
        add(&mut named, p.part.as_deref().map(voice_words).unwrap_or_else(|| inst(&p.instrument)));
    }
    for r in &g.roles_add {
        add(&mut named, r.part.as_deref().map(voice_words).unwrap_or_else(|| inst(&r.instrument)));
    }
    if named.is_empty() {
        if let Some(m) = lib.moods.iter().find(|m| m.id == mood).or(lib.moods.first()) {
            for r in &m.roles {
                add(&mut named, r.part.as_deref().map(voice_words).unwrap_or_else(|| inst(&r.instrument)));
            }
        }
    }
    named.truncate(6);
    named.join(", ")
}

/// Every genre's card, in the data's order (the engine's list before the data is read: names only).
pub fn genre_cards(lib: Option<&MusicLibrary>, offered: &[GenreChoice], mood: &str) -> Vec<GenreCard> {
    let Some(lib) = lib else {
        return offered.iter().map(|g| GenreCard { id: g.id.clone(), name: genre_label(&g.id, &g.words, &g.name), plain: String::new(), tempo: String::new(), instruments: String::new(), about: genre_label("", &g.about_words, &g.about) }).collect();
    };
    let mood_tempo = lib.moods.iter().find(|m| m.id == mood).or(lib.moods.first()).map(|m| m.tempo_bpm).unwrap_or((60.0, 120.0));
    lib.genres
        .iter()
        .map(|g| GenreCard {
            id: g.id.clone(),
            name: genre_label(&g.id, &g.words, &g.name),
            plain: genre_label("", &g.plain_words, &g.plain),
            tempo: tempo_words(g.tempo_bpm.unwrap_or(mood_tempo)),
            instruments: t("music_page.card_instruments").replace("{{list}}", &instrument_words(lib, g, mood)),
            about: genre_label("", &g.about_words, &g.about),
        })
        .collect()
}

/// A mood's sentence of where it plays (its words by key, else a community file's plain words).
pub fn mood_where(m: &humanity_music::mood::Mood) -> String {
    genre_label("", &m.where_words, &m.where_plain)
}

/// How many cards fit across, and each card's width.
fn card_columns(ui: &egui::Ui, theme: &Theme, wide: bool) -> (usize, f32) {
    let avail = ui.available_width();
    let gap = theme.spacing_sm;
    let cols = if wide { (((avail + gap) / (CARD_W + gap)).floor() as usize).max(1) } else { 1 };
    (cols, ((avail - gap * (cols as f32 - 1.0)) / cols as f32).max(200.0))
}

/// Under a card: Listen (20 seconds of a piece in it, then back to what played), or while that card's
/// audition plays, how long is left and Stop. Some(true): Listen clicked; Some(false): Stop clicked.
fn listen_row(ui: &mut egui::Ui, theme: &Theme, words: &str, auditioning: Option<&str>, heard_s: Option<f64>) -> Option<bool> {
    let mut out = None;
    ui.horizontal(|ui| {
        if auditioning == Some(words) {
            let left = (AUDITION_S as f64 - heard_s.unwrap_or(0.0)).max(0.0).ceil() as u32;
            let line = if heard_s.is_some() { t("music_page.listen_left").replace("{{n}}", &left.to_string()) } else { t("music_page.listen_waiting").to_string() };
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.accent()));
            if widgets::Button::new(t("music_page.listen_stop")).size(widgets::ButtonSize::Small).hint("music_page.listen_stop_hint").show(ui, theme) {
                out = Some(false);
            }
        } else if widgets::Button::new(&t("music_page.listen").replace("{{n}}", &(AUDITION_S as u32).to_string())).size(widgets::ButtonSize::Small).hint("music_page.listen_hint").show(ui, theme) {
            out = Some(true);
        }
    });
    out
}

/// How long ago a piece began, in words: now, minutes, hours, days.
pub fn ago_words(secs: u64) -> String {
    let mins = secs / 60;
    if mins == 0 {
        t("music_page.heard_now").to_string()
    } else if mins < 60 {
        t("music_page.heard_minutes").replace("{{n}}", &mins.to_string())
    } else if mins < 60 * 48 {
        t("music_page.heard_hours").replace("{{n}}", &(mins / 60).to_string())
    } else {
        t("music_page.heard_days").replace("{{n}}", &(mins / (60 * 24)).to_string())
    }
}

/// One card: a clickable surface `w` wide, marked when chosen. True when clicked.
fn card(ui: &mut egui::Ui, theme: &Theme, w: f32, chosen: bool, hint: &'static str, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let inner = w - 2.0 * theme.card_padding;
    let resp = widgets::card_clickable(ui, theme, |ui| {
        ui.set_min_width(inner);
        ui.set_max_width(inner);
        // Top to bottom (a grid cell lays its contents out left to right).
        ui.vertical(|ui| add(ui));
    })
    .hint(hint);
    if chosen {
        ui.painter().rect_stroke(resp.rect, theme.border_radius, Stroke::new(2.0f32, theme.accent()), egui::epaint::StrokeKind::Inside);
    }
    resp.clicked()
}

/// Explore: the genre cards, the mood cards, the seed, and the pieces heard.
pub(super) fn explore(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, accent: Color32, wide: bool) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let offered = super::controls::genres_of(state);
    let (cols, w) = card_columns(ui, theme, wide);
    let gap = theme.spacing_sm;
    // The genres.
    widgets::subsection_header(ui, theme, accent, t("music_page.genres_title"), "");
    ui.label(RichText::new(t("music_page.genres_intro")).color(theme.text_secondary()));
    // While a written piece is the music, the cards are the way back to composed music (BUG-343).
    if let Some(title) = super::written_title(&state.settings.music, &state.music_dev.readout) {
        ui.label(RichText::new(t("music_page.explore_written_note").replace("{{title}}", &title)).color(theme.accent()));
    }
    ui.add_space(theme.spacing_xs);
    let cards = genre_cards(lib.as_deref(), &offered, &sh.mood);
    // A genre or mood card clicked, chosen or not: composed music plays in place of a written piece.
    let mut composed = false;
    // The audition playing (its card's words) and the seconds it has played; a card's Listen asks
    // for one, its Stop ends one (applied after the cards are drawn).
    let auditioning = state.music_page.audition.as_ref().map(|a| a.words.clone());
    let audition_s = state.music_dev.readout.audition_s;
    let mut listen: Option<Audition> = None;
    let mut stop = false;
    let intensity = |mood: &str| lib.as_deref().map(|l| explore_intensity(l, mood)).unwrap_or(0.3);
    egui::Grid::new("music_page_genre_cards").num_columns(cols).spacing([gap, gap]).show(ui, |ui| {
        for (i, c) in cards.iter().enumerate() {
            let chosen = sh.genre == c.id;
            let stand_ins = offered.iter().find(|g| g.id == c.id).map(|g| g.stand_ins.clone()).unwrap_or_default();
            let clicked = ui.vertical(|ui| {
            let clicked = card(ui, theme, w, chosen, "music_page.genre_card_hint", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&c.name).size(theme.font_size_heading).strong().color(theme.text_primary()));
                    if chosen {
                        ui.label(RichText::new(t("music_page.card_chosen")).size(theme.font_size_small).color(theme.accent()));
                    }
                });
                if !c.plain.is_empty() {
                    ui.label(RichText::new(&c.plain).color(theme.text_primary()));
                }
                if !c.tempo.is_empty() {
                    ui.label(RichText::new(&c.tempo).size(theme.font_size_small).color(theme.text_secondary()));
                }
                if !c.instruments.is_empty() {
                    ui.label(RichText::new(&c.instruments).size(theme.font_size_small).color(theme.text_secondary()));
                }
                if chosen {
                    if !c.about.is_empty() {
                        ui.label(RichText::new(&c.about).size(theme.font_size_small).color(theme.text_muted()));
                    }
                    // Which of its instruments play synthesised stand-ins until a free recording of
                    // them is installed (said plainly, never silently).
                    if !stand_ins.is_empty() {
                        let parts: Vec<String> = stand_ins
                            .iter()
                            .map(|s| match &s.pack {
                                Some(pack) => t("music_page.stand_in_from").replace("{{part}}", t_dyn(&s.part)).replace("{{pack}}", t_dyn(pack)),
                                None => t_dyn(&s.part).to_string(),
                            })
                            .collect();
                        ui.label(RichText::new(t("music_page.genre_stand_ins").replace("{{parts}}", &parts.join(", "))).size(theme.font_size_small).color(theme.text_muted()));
                    }
                }
            });
            match listen_row(ui, theme, &c.name, auditioning.as_deref(), audition_s) {
                Some(true) => listen = Some(Audition { choice: MusicChoice { genre: c.id.clone(), mood: Some(sh.mood.clone()), intensity: Some(intensity(&sh.mood)), seed: sh.seed.max(1), tune: Tune::default(), written: None, style: None }, words: c.name.clone() }),
                Some(false) => stop = true,
                None => {}
            }
            clicked
            })
            .inner;
            if clicked {
                if !chosen {
                    sh.genre = c.id.clone();
                }
                composed = true;
            }
            if (i + 1) % cols == 0 {
                ui.end_row();
            }
        }
    });
    ui.label(RichText::new(t("music_page.recordings_note")).size(theme.font_size_small).color(theme.text_muted()));
    // The moods.
    ui.add_space(theme.spacing_md);
    widgets::subsection_header(ui, theme, accent, t("music_page.moods_title"), "");
    ui.label(RichText::new(t("music_page.moods_intro")).color(theme.text_secondary()));
    ui.add_space(theme.spacing_xs);
    if let Some(l) = lib.as_deref() {
        egui::Grid::new("music_page_mood_cards").num_columns(cols).spacing([gap, gap]).show(ui, |ui| {
            for (i, m) in l.moods.iter().enumerate() {
                let chosen = sh.mood == m.id;
                let danger = l.intensity.bands.iter().any(|b| b.mood.as_deref() == Some(m.id.as_str()));
                let name = mood_words(Some(l), &m.id);
                let clicked = ui.vertical(|ui| {
                let clicked = card(ui, theme, w, chosen, "music_page.mood_card_hint", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(mood_words(Some(l), &m.id)).size(theme.font_size_heading).strong().color(theme.text_primary()));
                        if chosen {
                            ui.label(RichText::new(t("music_page.card_chosen")).size(theme.font_size_small).color(theme.accent()));
                        }
                    });
                    let place = mood_where(m);
                    if !place.is_empty() {
                        ui.label(RichText::new(place).color(theme.text_primary()));
                    }
                    ui.label(RichText::new(tempo_words(m.tempo_bpm)).size(theme.font_size_small).color(theme.text_secondary()));
                    if danger {
                        ui.label(RichText::new(t("music_page.mood_danger")).size(theme.font_size_small).color(theme.warning()));
                    }
                });
                match listen_row(ui, theme, &name, auditioning.as_deref(), audition_s) {
                    Some(true) => listen = Some(Audition { choice: MusicChoice { genre: sh.genre.clone(), mood: Some(m.id.clone()), intensity: Some(explore_intensity(l, &m.id)), seed: sh.seed.max(1), tune: Tune::default(), written: None, style: None }, words: name.clone() }),
                    Some(false) => stop = true,
                    None => {}
                }
                clicked
                })
                .inner;
                if clicked {
                    if !chosen {
                        sh.mood = m.id.clone();
                    }
                    composed = true;
                }
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
    }
    if composed && super::back_to_composed(&mut state.settings.music) {
        state.settings_dirty = true;
    }
    // A card's Listen is not a click on it: the audition goes back to what played, written or not.
    if let Some(a) = listen {
        state.music_page.audition = Some(a);
    } else if stop {
        state.music_page.audition = None;
    }
    // The sound styles written music plays in (music-page.md section 15a, rung 3a): a card each, from
    // the styles' data, chosen here for all written music (a piece's details choose for one piece).
    ui.add_space(theme.spacing_md);
    widgets::subsection_header(ui, theme, accent, t("music_page.styles_title"), "");
    ui.label(RichText::new(t("music_page.styles_intro")).color(theme.text_secondary()));
    ui.add_space(theme.spacing_xs);
    let styles = super::styles::styles_of(state);
    let mut pick: Option<String> = None;
    let style_cards: Vec<(String, String, String)> = std::iter::once((String::new(), t("music_page.style_as_written").to_string(), t("music_page.style_as_written_plain").to_string()))
        .chain(styles.iter().map(|s| (s.id.clone(), super::styles::style_name(s), super::styles::style_plain(s))))
        .collect();
    egui::Grid::new("music_page_style_cards").num_columns(cols).spacing([gap, gap]).show(ui, |ui| {
        for (i, (id, name, plain)) in style_cards.iter().enumerate() {
            let chosen = state.settings.music.style == *id;
            let clicked = card(ui, theme, w, chosen, "music_page.style_card_hint", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).size(theme.font_size_heading).strong().color(theme.text_primary()));
                    if chosen {
                        ui.label(RichText::new(t("music_page.card_chosen")).size(theme.font_size_small).color(theme.accent()));
                    }
                });
                if !plain.is_empty() {
                    ui.label(RichText::new(plain).color(theme.text_primary()));
                }
            });
            if clicked && !chosen {
                pick = Some(id.clone());
            }
            if (i + 1) % cols == 0 {
                ui.end_row();
            }
        }
    });
    if let Some(id) = pick {
        state.settings.music.style = id;
        state.settings_dirty = true;
    }
    // The seed.
    ui.add_space(theme.spacing_md);
    widgets::subsection_header(ui, theme, accent, t("music_page.seed"), "");
    widgets::card(ui, theme, |ui| seed_row(ui, theme, state, sh));
    widgets::setting_hint(ui, theme, hint, t("music_page.seed_hint"));
    // The pieces heard.
    ui.add_space(theme.spacing_md);
    heard(ui, theme, state, sh, accent);
}

/// The seed: Previous, the box, Next and Random.
fn seed_row(ui: &mut egui::Ui, _theme: &Theme, state: &mut GuiState, sh: &mut Shown) {
    let p = &mut state.music_page;
    // The box shows the seed shown, and keeps what is being typed until it is a seed.
    if p.seed_shown != sh.seed {
        p.seed_text = sh.seed.to_string();
        p.seed_shown = sh.seed;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button(t("music_page.previous")).hint("music_page.previous_hint").clicked() {
            sh.seed = sh.seed.saturating_sub(1).max(1);
        }
        let r = ui.add(egui::TextEdit::singleline(&mut p.seed_text).desired_width(110.0)).hint("music_page.seed_box_hint");
        if r.changed() {
            if let Ok(s) = p.seed_text.trim().parse::<u64>() {
                if s > 0 {
                    sh.seed = s;
                    p.seed_shown = s;
                }
            }
        }
        if ui.button(t("music_page.next")).hint("music_page.next_hint").clicked() {
            sh.seed = sh.seed.saturating_add(1);
        }
        if ui.button(t("music_page.random")).hint("music_page.random_hint").clicked() {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
            sh.seed = humanity_music::piece::mix(now, sh.seed) % RANDOM_SEED_MAX + 1;
        }
    });
}

/// The pieces heard (kept in the person's own file), newest first, each with Play again and Like, and
/// Clear to empty the list.
fn heard(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, accent: Color32) {
    let lib = state.music_dev.readout.lib.clone();
    let list = state.music_page.heard.clone();
    let mut play: Option<Heard> = None;
    let mut like: Option<Heard> = None;
    let mut clear = false;
    let now = super::history::now_s();
    widgets::subsection_header(ui, theme, accent, t("music_page.heard_title"), "");
    widgets::card(ui, theme, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.heard_intro")).color(theme.text_secondary()));
            if !list.is_empty() && widgets::Button::new(t("music_page.heard_clear")).size(widgets::ButtonSize::Small).hint("music_page.heard_clear_hint").show(ui, theme) {
                clear = true;
            }
        });
        if let Some(e) = &state.music_page.heard_error {
            ui.label(RichText::new(e).size(theme.font_size_small).color(theme.warning()));
        }
        if list.is_empty() {
            ui.label(RichText::new(t("music_page.heard_none")).color(theme.text_muted()));
        }
        for (i, h) in list.iter().enumerate() {
            let line = match &h.written {
                Some(id) => {
                    let title = lib.as_deref().and_then(|l| l.score(id)).map(|e| score_words(e).0).unwrap_or_else(|| id.replace('_', " "));
                    t("music_page.heard_written").replace("{{title}}", &title)
                }
                None => {
                    let genre = match lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == h.genre)) {
                        Some(g) => genre_label(&g.id, &g.words, &g.name),
                        None => h.genre.replace('_', " "),
                    };
                    t("music_page.heard_line").replace("{{genre}}", &genre).replace("{{mood}}", &mood_words(lib.as_deref(), &h.mood)).replace("{{seed}}", &h.seed.to_string())
                }
            };
            let ago = ago_words(now.saturating_sub(h.at));
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(line).color(if i == 0 { theme.text_primary() } else { theme.text_secondary() }));
                ui.label(RichText::new(ago).size(theme.font_size_small).color(theme.text_muted()));
                if ui.button(t("music_page.heard_play")).hint("music_page.heard_play_hint").clicked() {
                    play = Some(h.clone());
                }
                if h.written.is_none() && ui.button(t("music_page.like")).hint("music_page.like_hint").clicked() {
                    like = Some(h.clone());
                }
            });
        }
    });
    if clear {
        state.music_page.clear_heard();
    }
    let tune_of = |h: &Heard| lib.as_deref().and_then(|l| l.genre_index(&h.genre).map(|g| h.tune.to_tune(l, g))).unwrap_or_default();
    if let Some(h) = play {
        match &h.written {
            Some(id) => {
                state.settings.music.written = id.clone();
                state.settings_dirty = true;
            }
            None => {
                if super::back_to_composed(&mut state.settings.music) {
                    state.settings_dirty = true;
                }
                sh.genre = h.genre.clone();
                sh.mood = h.mood.clone();
                sh.seed = h.seed.max(1);
                sh.tune = tune_of(&h);
            }
        }
    }
    if let Some(h) = like {
        let tune = tune_of(&h);
        super::strip::like(state, &Shown { genre: h.genre, mood: h.mood, seed: h.seed, tune });
    }
}
