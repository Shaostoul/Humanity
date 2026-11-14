//! The Music page's controls: Explore (genre, mood, seed), Tune (key, tempo, lead voices, each
//! part's level and mute) and Keep (favourites, "my music" for a mood, share codes, export). Every
//! word by key, every control with a hint that says what it changes (docs/design/music-page.md
//! sections 5 to 8).

use std::sync::Arc;

use egui::{Color32, RichText};
use humanity_core::i18n::t;
use humanity_music::favourite::{from_share_code, share_code, Favourite, SavedTune, ShareError};
use humanity_music::tune::{Tune, PIECE, ROLES};

use super::{export_dir, export_file_name, mood_words, role_colour, role_words, voice_words, ExportJob, ExportStatus, EXPORT_MINUTES, RANDOM_SEED_MAX};
use crate::gui::pages::settings_music::genre_label;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState};

/// Explore: the genre, the mood and the seed.
pub(super) fn explore(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let genres = state.music_dev.readout.genres.clone();
    let settings_genre = state.settings.music.genre.clone();
    let p = &mut state.music_page;
    widgets::subsection_header(ui, theme, accent, t("music_page.explore"), "");
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("music_page.genre")).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            let current = p.genre_or(&settings_genre).to_string();
            for g in &genres {
                let selected = current == g.id;
                if ui.radio(selected, RichText::new(genre_label(&g.id, &g.words, &g.name)).color(theme.text_primary())).hint("music_page.genre_hint").clicked() && !selected {
                    p.genre = g.id.clone();
                    // Lead voices are the genre's own: another genre starts from the piece's.
                    p.tune.lead = [PIECE; ROLES];
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.genre_hint"));
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("music_page.mood")).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            let current = if p.mood.is_empty() { humanity_music::library::DEFAULT_MOOD.to_string() } else { p.mood.clone() };
            for m in lib.iter().flat_map(|l| l.moods.iter()) {
                let selected = current == m.id;
                if ui.radio(selected, RichText::new(mood_words(lib.as_deref(), &m.id)).color(theme.text_primary())).hint("music_page.mood_hint").clicked() && !selected {
                    p.mood = m.id.clone();
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.mood_hint"));
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("music_page.seed")).color(theme.text_secondary()));
        ui.horizontal(|ui| {
            if ui.button(t("music_page.previous")).hint("music_page.previous_hint").clicked() {
                let s = p.seed.saturating_sub(1).max(1);
                p.set_seed(s);
            }
            let r = ui.add(egui::TextEdit::singleline(&mut p.seed_text).desired_width(110.0)).hint("music_page.seed_box_hint");
            if r.changed() {
                if let Ok(s) = p.seed_text.trim().parse::<u64>() {
                    if s > 0 {
                        p.seed = s;
                    }
                }
            }
            if ui.button(t("music_page.next")).hint("music_page.next_hint").clicked() {
                let s = p.seed.saturating_add(1);
                p.set_seed(s);
            }
            if ui.button(t("music_page.random")).hint("music_page.random_hint").clicked() {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
                let s = humanity_music::piece::mix(now, p.seed) % RANDOM_SEED_MAX + 1;
                p.set_seed(s);
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.seed_hint"));
    });
}

/// Tune: the key, the tempo, the lead voices, and each part's level and mute.
pub(super) fn tune(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let settings_genre = state.settings.music.genre.clone();
    let sheet = state.music_page.sheet.as_ref().map(|s| s.sheet.clone());
    let p = &mut state.music_page;
    let genre = lib.as_deref().and_then(|l| l.genre_index(p.genre_or(&settings_genre)).map(|i| &l.genres[i]));
    widgets::subsection_header(ui, theme, accent, t("music_page.tune"), t("music_page.tune_note"));
    widgets::card(ui, theme, |ui| {
        // The key: a transposition, shown as the key it makes.
        let mut steps = p.tune.transpose as i32;
        let key_now = sheet.as_ref().and_then(|s| s.sections.first().map(|x| (x.key_pc, x.mode)));
        let label = match key_now {
            Some((pc, mode)) => t("music_page.key_value").replace("{{key}}", humanity_music::sheet::pc_name(pc, humanity_music::sheet::key_uses_flats(pc, mode))).replace("{{steps}}", &format!("{steps:+}")),
            None => format!("{steps:+}"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.key")).color(theme.text_secondary()));
            // The slider's own text names it for assistive technology (the a11y test).
            if widgets::slider(ui, theme, egui::Slider::new(&mut steps, -6..=6).show_value(false).text(RichText::new(label).color(theme.text_primary()))).hint("music_page.key_hint").changed() {
                p.tune.transpose = steps as i8;
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.key_hint"));
        // The tempo, within the genre's range (or a tenth either way of a one-tempo genre).
        let range = genre.and_then(|g| g.tempo_bpm).or_else(|| lib.as_deref().and_then(|l| l.moods.iter().find(|m| m.id == p.mood).or(l.moods.first()).map(|m| m.tempo_bpm))).unwrap_or((60.0, 140.0));
        let (lo, hi) = Tune::tempo_range(range);
        let drawn = sheet.as_ref().and_then(|s| s.sections.first().map(|x| x.tempo_bpm)).unwrap_or((lo + hi) * 0.5);
        let mut bpm = if p.tune.tempo_bpm > 0.0 { p.tune.tempo_bpm } else { drawn };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.tempo")).color(theme.text_secondary()));
            if widgets::slider(ui, theme, egui::Slider::new(&mut bpm, lo..=hi).step_by(1.0).suffix(t("music_page.unit_bpm")).text(t("music_page.tempo_name"))).hint("music_page.tempo_hint").changed() {
                p.tune.tempo_bpm = bpm.round();
            }
            if p.tune.tempo_bpm > 0.0 && ui.button(t("music_page.tempo_piece")).hint("music_page.tempo_piece_hint").clicked() {
                p.tune.tempo_bpm = 0.0;
            }
        });
        let one_tempo = (range.1 - range.0).abs() < range.0 * humanity_music::tune::NARROW_TEMPO_SPAN;
        widgets::setting_hint(ui, theme, hint, if one_tempo { t("music_page.tempo_one") } else { t("music_page.tempo_hint") });
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
                let shown = match p.tune.lead[k] {
                    PIECE => match &playing {
                        Some(v) => format!("{} ({v})", t("music_page.lead_piece")),
                        None => t("music_page.lead_piece").to_string(),
                    },
                    i => entries.iter().find(|e| e.0 == i as usize).map(|e| e.1.clone()).unwrap_or_default(),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(role_words(k)).color(role_colour(theme, k as u8)));
                    egui::ComboBox::from_id_salt(("music_page_lead", k)).selected_text(shown).show_ui(ui, |ui| {
                        ui.selectable_value(&mut p.tune.lead[k], PIECE, t("music_page.lead_piece")).hint("music_page.lead_piece_hint");
                        for (i, words) in &entries {
                            ui.selectable_value(&mut p.tune.lead[k], *i as u16, words).hint("music_page.lead_voice_hint");
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
        // Each part's level and mute.
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("music_page.parts")).color(theme.text_secondary()));
        let plays: [bool; ROLES] = std::array::from_fn(|k| sheet.as_ref().is_none_or(|s| s.notes.iter().any(|n| n.role as usize == k)));
        egui::Grid::new("music_page_parts").num_columns(3).spacing([theme.spacing_sm, theme.spacing_xs]).show(ui, |ui| {
            for k in 0..ROLES {
                let colour = if plays[k] { role_colour(theme, k as u8) } else { theme.text_muted() };
                widgets::slider(ui, theme, egui::Slider::new(&mut p.tune.level_db[k], -24.0..=6.0).step_by(0.5).suffix(t("music_page.unit_db")).text(RichText::new(role_words(k)).color(colour))).hint("music_page.level_hint");
                let mut muted = p.tune.mute & (1 << k) != 0;
                if ui.checkbox(&mut muted, t("music_page.mute")).hint("music_page.mute_hint").changed() {
                    p.tune.mute = if muted { p.tune.mute | 1 << k } else { p.tune.mute & !(1 << k) };
                }
                if plays[k] {
                    ui.label("");
                } else {
                    ui.label(RichText::new(t("music_page.part_silent")).size(theme.font_size_small).color(theme.text_muted()));
                }
                ui.end_row();
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.parts_hint"));
        if p.tune != Tune::default() && ui.button(t("music_page.reset_tune")).hint("music_page.reset_tune_hint").clicked() {
            p.tune = Tune::default();
        }
    });
}

/// Keep: name and keep the piece, the favourites with their buttons, "my music" for each mood, a
/// pasted share code, and where exports go.
pub(super) fn keep(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let settings_genre = state.settings.music.genre.clone();
    let p = &mut state.music_page;
    let mut changed = false;
    widgets::subsection_header(ui, theme, accent, t("music_page.keep"), "");
    widgets::card(ui, theme, |ui| {
        if let Some(e) = &p.fav_error {
            ui.label(RichText::new(t("music_page.fav_error").replace("{{error}}", e)).color(theme.warning()));
        }
        // Keep the piece playing on the page.
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("music_page.name")).color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut p.name_text).desired_width(220.0)).hint("music_page.name_hint");
            if ui.button(t("music_page.keep_button")).hint("music_page.keep_hint").clicked() {
                let genre = p.genre_or(&settings_genre).to_string();
                let gi = lib.as_deref().and_then(|l| l.genre_index(&genre));
                let tune = match (lib.as_deref(), gi) {
                    (Some(l), Some(g)) => SavedTune::from_tune(&p.tune, l, g),
                    _ => SavedTune::default(),
                };
                let name = if p.name_text.trim().is_empty() { t("music_page.default_name").replace("{{seed}}", &p.seed.to_string()) } else { p.name_text.trim().to_string() };
                let mood = if p.mood.is_empty() { humanity_music::library::DEFAULT_MOOD.to_string() } else { p.mood.clone() };
                Arc::make_mut(&mut p.favourites).add(Favourite { id: 0, name, genre, mood, seed: p.seed, tune });
                p.name_text.clear();
                changed = true;
            }
        });
        widgets::setting_hint(ui, theme, hint, t("music_page.keep_hint"));
        ui.add_space(theme.spacing_xs);
        // The favourites.
        let favs = p.favourites.favourites.clone();
        if favs.is_empty() {
            ui.label(RichText::new(t("music_page.favourites_none")).color(theme.text_muted()));
        }
        let mut open: Option<Favourite> = None;
        let mut remove: Option<u64> = None;
        let mut export: Option<Favourite> = None;
        for f in &favs {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&f.name).strong().color(theme.text_primary()));
                let gl = state_genre_words(&f.genre, lib.as_deref());
                ui.label(RichText::new(t("music_page.fav_line").replace("{{genre}}", &gl).replace("{{mood}}", &mood_words(lib.as_deref(), &f.mood)).replace("{{seed}}", &f.seed.to_string())).size(theme.font_size_small).color(theme.text_muted()));
                if ui.button(t("music_page.open")).hint("music_page.open_hint").clicked() {
                    open = Some(f.clone());
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
        }
        if let Some(f) = open {
            p.open_favourite(&f, lib.as_deref());
        }
        if let Some(id) = remove {
            Arc::make_mut(&mut p.favourites).remove(id);
            changed = true;
        }
        if let Some(f) = export {
            let path = p.export_dir_override.clone().unwrap_or_else(export_dir).join(export_file_name(&f, p.export_minutes));
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
        let dir = p.export_dir_override.clone().unwrap_or_else(export_dir);
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
                egui::ComboBox::from_id_salt(("music_page_my", m.id.as_str())).selected_text(shown).show_ui(ui, |ui| {
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
                        p.open_favourite(&f, lib.as_deref());
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
