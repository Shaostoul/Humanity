//! The Music page's controls: What plays (let the game choose, with its options, or the person's
//! choice), Tune (key, tempo, lead voices, each part's level and mute) and Your music (favourites,
//! "my music" for a mood, share codes, export); Explore is explore.rs. Every word by key, every
//! control with a hint that says what it changes (docs/design/music-page.md sections 4 to 8).
//!
//! The choice is the person's own settings (`MusicSettings`, config.json), the one state the score,
//! this page and Settings > Music read (section 4). What plays and Keep write it directly; Explore
//! and Tune change the page's [`Shown`], which `draw` writes back through [`apply`](super::apply),
//! so a mood, a seed or a change of tuning becomes the person's choice and plays everywhere.

use std::sync::Arc;

use egui::{Color32, RichText};
use humanity_core::i18n::t;
use humanity_music::favourite::{from_share_code, share_code, Favourite, SavedTune, ShareError};
use humanity_music::tune::{Tune, PIECE, ROLES};

use super::{back_to_composed, export_dir, export_file_name, genre_label, mood_words, open_favourite, role_colour, role_words, voice_words, ExportJob, ExportStatus, Shown, EXPORT_MINUTES};
use crate::gui::pages::dev_music::GenreChoice;
use crate::gui::theme::Theme;
use crate::gui::widgets::{with_hint, Hint};
use crate::gui::{widgets, GuiState};

/// The genres offered: the engine's list (the music's own data, community genres included), else
/// the default alone, for the first moment of a run before the data is read (engine/music.rs,
/// BUG-273).
pub(super) fn genres_of(state: &GuiState) -> Vec<GenreChoice> {
    if state.music_dev.readout.genres.is_empty() {
        vec![GenreChoice { id: humanity_music::library::DEFAULT_GENRE.to_string(), words: "music.genre.mothership_calm".to_string(), about_words: "music.genre.mothership_calm_about".to_string(), ..Default::default() }]
    } else {
        state.music_dev.readout.genres.clone()
    }
}

/// A genre's words from the list offered (its key, else its plain name, else its id).
fn listed_words(genres: &[GenreChoice], id: &str) -> String {
    genres.iter().find(|g| g.id == id).map(|g| genre_label(&g.id, &g.words, &g.name)).unwrap_or_else(|| id.replace('_', " "))
}

/// What plays: let the game choose (with what it chooses by: places, a blend, a new piece every few
/// minutes), or the person's choice. Writes the person's settings.
pub(super) fn what_plays(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, shown: &Shown, accent: Color32) {
    let hint = state.settings.hint_display;
    let genres = genres_of(state);
    let lib = state.music_dev.readout.lib.clone();
    // "A new piece" choices (data/music/pieces.ron, 0 = Never), and the person's own value if it is
    // not among them (a hand-edited config), so it shows as chosen.
    let mut piece_choices = if state.music_dev.readout.piece_minutes.is_empty() { vec![4.0, 0.0] } else { state.music_dev.readout.piece_minutes.clone() };
    if !piece_choices.iter().any(|c| (c - state.settings.music.piece_minutes).abs() < 1e-3) {
        piece_choices.push(state.settings.music.piece_minutes);
    }
    let m = &mut state.settings.music;
    let mut dirty = false;
    widgets::subsection_header(ui, theme, accent, t("music_page.what_plays"), "");
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("music_page.what_plays_note")).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
        // Either choice here is a choice of composed music: it plays in place of a written piece
        // (BUG-343), even clicked when it is already chosen.
        if ui.radio(m.game_chooses, RichText::new(t("music_page.game_chooses")).color(theme.text_primary())).hint("music_page.game_chooses_hint").clicked() {
            if !m.game_chooses {
                m.game_chooses = true;
                dirty = true;
            }
            dirty |= back_to_composed(m);
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.game_chooses_hint"));
        // What the game chooses by, greyed while the person's choice plays.
        let game = m.game_chooses;
        ui.indent("music_page_game_options", |ui| {
            ui.add_enabled_ui(game, |ui| {
                ui.label(RichText::new(t("music_page.game_options")).size(theme.font_size_small).color(theme.text_muted()));
                dirty |= with_hint(ui, "music_page.places_pick_genre_hint", |ui| widgets::toggle(ui, theme, t("music_page.places_pick_genre"), &mut m.places_pick_genre));
                widgets::setting_hint(ui, theme, hint, t("music_page.places_pick_genre_hint"));
                // A second genre to blend with: a list that opens, so the one genre control stays
                // the only place the genres are laid out.
                let blend_shown = if m.blend_with.is_empty() { t("music_page.blend_none").to_string() } else { listed_words(&genres, &m.blend_with) };
                ui.horizontal(|ui| {
                    // Its words as the list's own label, so a screen reader names it (the a11y walk).
                    egui::ComboBox::new("music_page_blend", t("music_page.blend_with"))
                        .selected_text(blend_shown)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(m.blend_with.is_empty(), t("music_page.blend_none")).hint("music_page.blend_with_hint").clicked() && !m.blend_with.is_empty() {
                                m.blend_with.clear();
                                dirty = true;
                            }
                            for g in genres.iter().filter(|g| g.id != m.genre) {
                                let selected = m.blend_with == g.id;
                                if ui.selectable_label(selected, genre_label(&g.id, &g.words, &g.name)).hint("music_page.blend_with_hint").clicked() && !selected {
                                    m.blend_with = g.id.clone();
                                    dirty = true;
                                }
                            }
                        })
                        .response
                        .hint("music_page.blend_with_hint");
                });
                widgets::setting_hint(ui, theme, hint, t("music_page.blend_with_hint"));
                if !m.blend_with.is_empty() {
                    dirty |= with_hint(ui, "music_page.blend_hint", |ui| widgets::labeled_slider(ui, theme, t("music_page.blend"), &mut m.blend, 0.0..=1.0));
                    widgets::setting_hint(ui, theme, hint, t("music_page.blend_hint"));
                }
                // A new piece (docs/design/music.md section 7c): how often the next seed begins.
                ui.label(RichText::new(t("music_page.new_piece")).color(theme.text_secondary()));
                ui.horizontal_wrapped(|ui| {
                    for &minutes in &piece_choices {
                        let selected = (m.piece_minutes - minutes).abs() < 1e-3;
                        let words = if minutes <= 0.0 { t("music_page.new_piece_never").to_string() } else { t("music_page.new_piece_every").replace("{{minutes}}", &format!("{minutes:.0}")) };
                        if ui.radio(selected, RichText::new(words).color(theme.text_primary())).hint("music_page.new_piece_hint").clicked() && !selected {
                            m.piece_minutes = minutes;
                            dirty = true;
                        }
                    }
                });
                widgets::setting_hint(ui, theme, hint, t("music_page.new_piece_hint"));
                ui.label(RichText::new(t("music_page.new_piece_what")).size(theme.font_size_small).color(theme.text_muted()));
            });
        });
        ui.add_space(theme.spacing_xs);
        if ui.radio(!m.game_chooses, RichText::new(t("music_page.my_choice")).color(theme.text_primary())).hint("music_page.my_choice_hint").clicked() {
            if m.game_chooses {
                // What plays now becomes the person's choice, so the switch alone changes nothing heard.
                m.game_chooses = false;
                m.mood = shown.mood.clone();
                m.seed = shown.seed.max(1);
                m.tune = match lib.as_deref().and_then(|l| l.genre_index(&m.genre).map(|g| (l, g))) {
                    Some((l, g)) => SavedTune::from_tune(&shown.tune, l, g),
                    None => SavedTune::default(),
                };
                dirty = true;
            }
            dirty |= back_to_composed(m);
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.my_choice_hint"));
    });
    if dirty {
        state.settings_dirty = true;
    }
}

/// Tune: the key, the tempo and the lead voices (each part's level, Mute and Solo are the mixer's:
/// mixer.rs).
pub(super) fn tune(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let sheet = state.music_page.sheet.as_ref().map(|s| s.sheet.clone());
    let genre = lib.as_deref().and_then(|l| l.genre_index(&sh.genre).map(|i| &l.genres[i]));
    let tu = &mut sh.tune;
    let mood_intensity = lib.as_deref().map(|l| humanity_music::favourite::explore_intensity(l, &sh.mood)).unwrap_or(0.3);
    widgets::subsection_header(ui, theme, accent, t("music_page.tune"), t("music_page.tune_note"));
    widgets::card(ui, theme, |ui| {
        // The tuning (rung 3a): A = 432 by default (the 2026-11-06 decision) or 440, for everything the
        // page plays, written pieces and every sound style included (Settings > Music keeps the same
        // choice).
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.tuning")).color(theme.text_secondary()));
            for tuning in humanity_music::tuning::Tuning::ALL {
                let selected = state.settings.music.tuning == tuning;
                if ui.radio(selected, RichText::new(humanity_core::i18n::t_dyn(tuning.words_key())).color(theme.text_primary())).hint("music_page.tuning_hint").clicked() && !selected {
                    state.settings.music.tuning = tuning;
                    state.settings_dirty = true;
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.tuning_hint"));
        // The key: a transposition, shown as the key it makes.
        let mut steps = tu.transpose as i32;
        let key_now = sheet.as_ref().and_then(|s| s.sections.first().map(|x| (x.key_pc, x.mode)));
        let label = match key_now {
            Some((pc, mode)) => t("music_page.key_value").replace("{{key}}", humanity_music::sheet::pc_name(pc, humanity_music::sheet::key_uses_flats(pc, mode))).replace("{{steps}}", &format!("{steps:+}")),
            None => format!("{steps:+}"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.key")).color(theme.text_secondary()));
            // The slider's own text names it for assistive technology (the a11y test).
            if widgets::slider(ui, theme, egui::Slider::new(&mut steps, -6..=6).show_value(false).text(RichText::new(label).color(theme.text_primary()))).hint("music_page.key_hint").changed() {
                tu.transpose = steps as i8;
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.key_hint"));
        // The tempo, within the genre's range (or a tenth either way of a one-tempo genre).
        let range = genre.and_then(|g| g.tempo_bpm).or_else(|| lib.as_deref().and_then(|l| l.moods.iter().find(|m| m.id == sh.mood).or(l.moods.first()).map(|m| m.tempo_bpm))).unwrap_or((60.0, 140.0));
        let (lo, hi) = Tune::tempo_range(range);
        let drawn = sheet.as_ref().and_then(|s| s.sections.first().map(|x| x.tempo_bpm)).unwrap_or((lo + hi) * 0.5);
        let mut bpm = if tu.tempo_bpm > 0.0 { tu.tempo_bpm } else { drawn };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.tempo")).color(theme.text_secondary()));
            if widgets::slider(ui, theme, egui::Slider::new(&mut bpm, lo..=hi).step_by(1.0).suffix(t("music_page.unit_bpm")).text(t("music_page.tempo_name"))).hint("music_page.tempo_hint").changed() {
                tu.tempo_bpm = bpm.round();
            }
            if tu.tempo_bpm > 0.0 && ui.button(t("music_page.tempo_piece")).hint("music_page.tempo_piece_hint").clicked() {
                tu.tempo_bpm = 0.0;
            }
        });
        let one_tempo = (range.1 - range.0).abs() < range.0 * humanity_music::tune::NARROW_TEMPO_SPAN;
        widgets::setting_hint(ui, theme, hint, if one_tempo { t("music_page.tempo_one") } else { t("music_page.tempo_hint") });
        // The swing (rung 3a): where the off-beats fall, from straight (half way through the beat) to
        // heavy (three quarters), else the genre's own.
        let own_swing = genre.and_then(|g| g.groove.as_ref().map(|x| x.swing)).unwrap_or(0.5).clamp(0.5, 0.75);
        let mut swing = if tu.swing >= 0.0 { tu.swing } else { own_swing };
        let swing_label = t("music_page.swing_value").replace("{{pct}}", &format!("{:.0}", swing * 100.0));
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.swing")).color(theme.text_secondary()));
            if widgets::slider(ui, theme, egui::Slider::new(&mut swing, 0.5..=0.75).step_by(0.01).show_value(false).text(RichText::new(swing_label).color(theme.text_primary()))).hint("music_page.swing_hint").changed() {
                tu.swing = (swing * 100.0).round() / 100.0;
            }
            if tu.swing >= 0.0 && ui.button(t("music_page.swing_own")).hint("music_page.swing_own_hint").clicked() {
                tu.swing = -1.0;
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.swing_hint"));
        // The intensity (rung 3a): how much is going on, 0 calm to 1 every layer the mood has, else
        // the mood's own (what Explore plays it at).
        let mut intensity = if tu.intensity >= 0.0 { tu.intensity } else { mood_intensity };
        let intensity_label = t("music_page.intensity_value").replace("{{pct}}", &format!("{:.0}", intensity * 100.0));
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.intensity")).color(theme.text_secondary()));
            if widgets::slider(ui, theme, egui::Slider::new(&mut intensity, 0.0..=1.0).step_by(0.05).show_value(false).text(RichText::new(intensity_label).color(theme.text_primary()))).hint("music_page.intensity_hint").changed() {
                tu.intensity = (intensity * 100.0).round() / 100.0;
            }
            if tu.intensity >= 0.0 && ui.button(t("music_page.intensity_own")).hint("music_page.intensity_own_hint").clicked() {
                tu.intensity = -1.0;
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.intensity_hint"));
        // The lead voices: each role the genre offers leads for.
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("music_page.lead")).color(theme.text_secondary()));
        let mut any = false;
        if let Some(g) = genre {
            for k in 0..ROLES {
                let entries: Vec<(usize, String)> = g.leads.iter().enumerate().filter(|(_, e)| e.role.index() == k).map(|(i, e)| (i, voice_words(e.part.as_deref().unwrap_or(&e.instrument)))).collect();
                if entries.is_empty() {
                    continue;
                }
                any = true;
                let playing = sheet.as_ref().map(|s| s.leads[k]).filter(|&i| i != PIECE).and_then(|i| entries.iter().find(|e| e.0 == i as usize)).map(|e| e.1.clone());
                let shown = match tu.lead[k] {
                    PIECE => match &playing {
                        Some(v) => format!("{} ({v})", t("music_page.lead_piece")),
                        None => t("music_page.lead_piece").to_string(),
                    },
                    i => entries.iter().find(|e| e.0 == i as usize).map(|e| e.1.clone()).unwrap_or_default(),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(role_words(k)).color(role_colour(theme, k as u8)));
                    egui::ComboBox::from_id_salt(("music_page_lead", k))
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut tu.lead[k], PIECE, t("music_page.lead_piece")).hint("music_page.lead_piece_hint");
                            for (i, words) in &entries {
                                ui.selectable_value(&mut tu.lead[k], *i as u16, words).hint("music_page.lead_voice_hint");
                            }
                        })
                        .response
                        .hint("music_page.lead_hint");
                });
            }
        }
        if !any {
            ui.label(RichText::new(t("music_page.no_leads")).size(theme.font_size_small).color(theme.text_muted()));
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.lead_hint"));
        if *tu != Tune::default() && ui.button(t("music_page.reset_tune")).hint("music_page.reset_tune_hint").clicked() {
            *tu = Tune::default();
        }
    });
}

/// Your music: the favourites (Like keeps one in a click; each here with Open, Rename, Copy code,
/// Export and Remove, the one Like marked outlined), "my music" for each mood, a pasted share code,
/// and where exports go. Opening a favourite or a code makes it the person's choice.
pub(super) fn keep(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let p = &mut state.music_page;
    let mut changed = false;
    let mut opened: Option<Favourite> = None;
    widgets::subsection_header(ui, theme, accent, t("music_page.keep"), "");
    widgets::card(ui, theme, |ui| {
        if let Some(e) = &p.fav_error {
            ui.label(RichText::new(t("music_page.fav_error").replace("{{error}}", e)).color(theme.warning()));
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.like_hint"));
        ui.add_space(theme.spacing_xs);
        // The favourites.
        let favs = p.favourites.favourites.clone();
        if favs.is_empty() {
            ui.label(RichText::new(t("music_page.favourites_none")).color(theme.text_muted()));
        }
        let mut remove: Option<u64> = None;
        let mut export: Option<Favourite> = None;
        let mut renamed: Option<(u64, String)> = None;
        for f in &favs {
            let marked = p.marked == Some(f.id);
            let row = ui.horizontal_wrapped(|ui| {
                match p.renaming.as_mut().filter(|(id, _)| *id == f.id) {
                    Some((_, text)) => {
                        let r = ui.add(egui::TextEdit::singleline(text).desired_width(240.0)).hint("music_page.rename_hint");
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (ui.button(t("music_page.rename_done")).hint("music_page.rename_done_hint").clicked() || enter) && !text.trim().is_empty() {
                            renamed = Some((f.id, text.trim().to_string()));
                        }
                    }
                    None => {
                        ui.label(RichText::new(&f.name).strong().color(if marked { accent } else { theme.text_primary() }));
                    }
                }
                let gl = state_genre_words(&f.genre, lib.as_deref());
                ui.label(RichText::new(t("music_page.fav_line").replace("{{genre}}", &gl).replace("{{mood}}", &mood_words(lib.as_deref(), &f.mood)).replace("{{seed}}", &f.seed.to_string())).size(theme.font_size_small).color(theme.text_muted()));
                if ui.button(t("music_page.open")).hint("music_page.open_hint").clicked() {
                    opened = Some(f.clone());
                }
                if p.renaming.as_ref().is_none_or(|(id, _)| *id != f.id) && ui.button(t("music_page.rename")).hint("music_page.rename_hint").clicked() {
                    p.renaming = Some((f.id, f.name.clone()));
                }
                if ui.button(t("music_page.copy_code")).hint("music_page.copy_code_hint").clicked() {
                    ui.ctx().copy_text(share_code(f));
                    p.share_msg = Some(t("music_page.code_copied").to_string());
                }
                let working = matches!(p.export_status, ExportStatus::Working { .. }) || p.export_request.is_some();
                if ui.add_enabled(!working, egui::Button::new(t("music_page.export"))).hint("music_page.export_hint").clicked() {
                    export = Some(f.clone());
                }
                if ui.button(t("music_page.remove")).hint("music_page.remove_hint").clicked() {
                    remove = Some(f.id);
                }
            });
            // The favourite Like just kept, or found already kept, outlined.
            if marked {
                ui.painter().rect_stroke(row.response.rect.expand(2.0), 4.0, egui::Stroke::new(2.0f32, accent), egui::epaint::StrokeKind::Outside);
            }
        }
        if let Some((id, name)) = renamed {
            if let Some(f) = Arc::make_mut(&mut p.favourites).favourites.iter_mut().find(|f| f.id == id) {
                f.name = name;
            }
            p.renaming = None;
            changed = true;
        }
        if let Some(id) = remove {
            Arc::make_mut(&mut p.favourites).remove(id);
            changed = true;
        }
        let dir = p.export_dir_override.clone().unwrap_or_else(export_dir);
        if let Some(f) = export {
            let path = dir.join(export_file_name(&f, p.export_minutes));
            p.export_status = ExportStatus::Working { name: f.name.clone() };
            p.export_request = Some(ExportJob { favourite: f, minutes: p.export_minutes, path });
        }
        // The export's length and where it stands.
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.export_length")).color(theme.text_secondary()));
            for m in EXPORT_MINUTES {
                let selected = (p.export_minutes - m).abs() < 1e-3;
                if ui.radio(selected, t("music_page.minutes").replace("{{n}}", &format!("{m:.0}"))).hint("music_page.export_length_hint").clicked() {
                    p.export_minutes = m;
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, &t("music_page.export_where").replace("{{dir}}", &dir.display().to_string()));
        match &p.export_status {
            ExportStatus::Idle => {}
            ExportStatus::Working { name } => {
                ui.label(RichText::new(t("music_page.export_working").replace("{{name}}", name)).color(theme.text_secondary()));
            }
            ExportStatus::Done { path } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(t("music_page.export_done").replace("{{path}}", &path.display().to_string())).color(theme.success()));
                    if ui.button(t("music_page.open_folder")).hint("music_page.open_folder_hint").clicked() {
                        open_folder(path.parent().unwrap_or(&dir));
                    }
                });
            }
            ExportStatus::Failed { why } => {
                ui.label(RichText::new(t("music_page.export_failed").replace("{{why}}", why)).color(theme.warning()));
            }
        }
        // "My music": a favourite for each mood.
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(t("music_page.my_music")).color(theme.text_secondary()));
        widgets::setting_hint(ui, theme, hint, t("music_page.my_music_hint"));
        egui::Grid::new("music_page_my_music").num_columns(2).spacing([theme.spacing_sm, theme.spacing_xs]).show(ui, |ui| {
            for m in lib.iter().flat_map(|l| l.moods.iter()) {
                ui.label(RichText::new(mood_words(lib.as_deref(), &m.id)).color(theme.text_primary()));
                let current = p.favourites.for_mood(&m.id).map(|f| f.id);
                let shown = current.and_then(|id| favs.iter().find(|f| f.id == id)).map(|f| f.name.clone()).unwrap_or_else(|| t("music_page.new_pieces").to_string());
                let mut pick = current;
                egui::ComboBox::from_id_salt(("music_page_my", m.id.as_str()))
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pick, None, t("music_page.new_pieces")).hint("music_page.new_pieces_hint");
                        for f in &favs {
                            ui.selectable_value(&mut pick, Some(f.id), &f.name).hint("music_page.my_music_pick_hint");
                        }
                    })
                    .response
                    .hint("music_page.my_music_hint");
                if pick != current {
                    Arc::make_mut(&mut p.favourites).assign(&m.id, pick);
                    changed = true;
                }
                ui.end_row();
            }
        });
        // A friend's share code.
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.paste")).color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut p.share_text).desired_width(320.0)).hint("music_page.paste_hint");
            if ui.button(t("music_page.paste_button")).hint("music_page.paste_button_hint").clicked() {
                let msg = match from_share_code(&p.share_text) {
                    Ok(f) if lib.as_deref().is_some_and(|l| l.genre_index(&f.genre).is_none()) => t("music_page.paste_genre_missing").replace("{{genre}}", &f.genre),
                    Ok(f) => {
                        let name = f.name.clone();
                        opened = Some(f.clone());
                        Arc::make_mut(&mut p.favourites).add(f);
                        changed = true;
                        p.share_text.clear();
                        t("music_page.paste_ok").replace("{{name}}", &name)
                    }
                    Err(ShareError::NotACode) => t("music_page.paste_not_code").to_string(),
                    Err(ShareError::Mistyped) => t("music_page.paste_mistyped").to_string(),
                    Err(ShareError::Garbled) => t("music_page.paste_garbled").to_string(),
                };
                p.share_msg = Some(msg);
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.paste_hint"));
        if let Some(m) = &p.share_msg {
            ui.label(RichText::new(m).size(theme.font_size_small).color(theme.text_secondary()));
        }
    });
    if changed {
        p.save_favourites();
    }
    if let Some(f) = opened {
        open_favourite(&mut state.settings.music, &f);
        state.settings_dirty = true;
    }
}

/// A genre's words from the data (its key, else its plain name, else its id).
fn state_genre_words(id: &str, lib: Option<&humanity_music::MusicLibrary>) -> String {
    match lib.and_then(|l| l.genres.iter().find(|g| g.id == id)) {
        Some(g) => genre_label(&g.id, &g.words, &g.name),
        None => id.replace('_', " "),
    }
}

/// Open a folder in the system's file browser.
fn open_folder(dir: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(dir).spawn();
    if let Err(e) = r {
        log::warn!("[Music] the folder {} did not open: {e}", dir.display());
    }
}
