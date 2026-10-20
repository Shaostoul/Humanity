// Child of pages/games.rs (#[path]): a game's details, at the right of the library while a game is
// chosen, and its screenshot shown large. Drawn from what the page's worker read (games/), so
// nothing here reads a file: the screenshots are shrunk for drawing by widgets/thumbnails.rs on
// its own workers.
//
// Top to bottom: its banner slot (games/art.rs: Steam's cached banner with the game's lettering
// over it, else a plain panel with its name and its program's icon), where it came
// from and Close; why it cannot start, when it cannot; Play, the store's own button and Show in
// folder (Rename and Remove for a game added by hand); its numbers (time played, last played, size
// on the disk, last updated, version, the folder it is in); its achievements, unlocked first with
// the day; the person's screenshots; its Workshop mods; and its links, in Steam and on the web.

use egui::{Rect, RichText, Sense, Stroke, Vec2};

use super::{Act, GamesState, ShowAll};
use crate::games::links::{self, Opens};
use crate::games::{Game, State, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cover_tile, icons, shelf, thumbnails};
use crate::gui::GuiState;

/// The details' width when they open (the person can drag their edge).
pub(super) const DETAILS_WIDTH: f32 = 440.0;
/// The banner slot's shape (Steam's library banner is about 3.1 to 1).
const HERO_ASPECT: f32 = 3.1;
/// How many achievements, screenshots and mods show before "Show all".
const FIRST_ACHIEVEMENTS: usize = 8;
const FIRST_SCREENSHOTS: usize = 9;
const FIRST_MODS: usize = 6;
/// At most this many screenshots are drawn even with Show all (the rest: Show in folder).
const MOST_SCREENSHOTS: usize = 60;
/// Screenshots to a row.
const SHOTS_ACROSS: usize = 3;

/// The section headings.
pub(super) const ABOUT: &str = "About";
pub(super) const ACHIEVEMENTS: &str = "Achievements";
pub(super) const SCREENSHOTS: &str = "Screenshots";
pub(super) const MODS: &str = "Mods";
pub(super) const LINKS: &str = "Links";
/// Said under the mods: why they are numbers.
pub(super) const MODS_NOTE: &str = "Steam keeps each mod's name on its own servers, so here each is shown by its number. Open in Steam shows its page.";
/// Said under the achievements when which are unlocked is not known.
pub(super) const ACHIEVEMENTS_UNKNOWN: &str = "Steam has not saved which of these you have on this computer.";
/// A hidden achievement's description, until it is unlocked.
pub(super) const HIDDEN: &str = "Hidden: its description shows once you unlock it.";
/// Over the web links.
pub(super) const ON_THE_WEB: &str = "On the web";
pub(super) const IN_STEAM: &str = "In Steam";

/// The details, when a game is chosen. Returns what was clicked.
pub(super) fn panel(ctx: &egui::Context, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let page = &state.games;
    let Some(game) = page.selected_game() else { return acts };
    // The details panel the Tools page shares (widgets/shelf.rs), its contents clear of its scroll
    // bar since 2026-10-19 (the operator's library at 2560 wide: dates and the banner touched it).
    shelf::details_panel(ctx, theme, "games_details", DETAILS_WIDTH, |ui| {
        body(ui, theme, page, game, &mut acts);
    });
    acts
}

/// A section's heading, a note beside it, and a line under it (the shelf's).
fn heading(ui: &mut egui::Ui, theme: &Theme, text: &str, note: Option<&str>) {
    shelf::section_heading(ui, theme, text, note);
}

fn body(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, game: &Game, acts: &mut Vec<Act>) {
    let width = ui.available_width();
    // The banner slot: a picture when the app may show one (games/art.rs), else the plain panel
    // with the game's name.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, (width / HERO_ASPECT).round().max(80.0)), Sense::hover());
    let art = &game.details.art;
    let banner = art.hero.as_deref().or(art.cover.as_deref());
    let picture = banner.and_then(|p| thumbnails::get(ui.ctx(), p, width));
    // Whether it HAS a picture, not whether it is read yet: the name's line under it is there from
    // the first frame, so nothing below moves when the picture comes in.
    let has_picture = banner.is_some();
    cover_tile::paint_cover(ui.painter(), theme, rect, picture, page.icon_of(game), &game.name, &game.id, theme.font_size_title, theme.border_radius_lg);
    // The game's own title lettering over its banner, as Steam's library shows it.
    if picture.is_some() {
        if let Some((logo, size)) = art.logo.as_deref().and_then(|p| thumbnails::get(ui.ctx(), p, width * 0.5)) {
            let pad = theme.spacing_md;
            let room = Vec2::new(rect.width() * 0.5, rect.height() * 0.6);
            let k = (room.x / size.x).min(room.y / size.y);
            let at = Rect::from_min_size(egui::pos2(rect.left() + pad, rect.bottom() - pad - size.y * k), size * k);
            ui.painter().with_clip_rect(rect).image(logo, at, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
    }
    ui.add_space(theme.spacing_sm);
    if has_picture {
        ui.label(RichText::new(&game.name).size(theme.font_size_title).color(theme.text_primary()));
    }

    ui.horizontal(|ui| {
        ui.label(RichText::new(game.store.name()).size(theme.font_size_small).color(theme.text_secondary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::ghost(super::CLOSE).size(widgets::ButtonSize::Small).tooltip("Close its details.").show(ui, theme) {
                acts.push(Act::CloseDetails);
            }
        });
    });
    if let Some(words) = super::why_not_words(game) {
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.warning()));
    }
    ui.add_space(theme.spacing_xs);
    actions(ui, theme, page, game, acts);
    about(ui, theme, game);
    achievements(ui, theme, page, game, acts);
    screenshots(ui, theme, page, game, acts);
    mods(ui, theme, page, game, acts);
    links_section(ui, theme, game, acts);
    ui.add_space(theme.spacing_lg);
}

/// Play, the store's own button, Show in folder; Rename and Remove for a game added by hand.
fn actions(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, game: &Game, acts: &mut Vec<Act>) {
    ui.horizontal_wrapped(|ui| {
        if game.play.is_some() && widgets::Button::primary(super::PLAY).size(widgets::ButtonSize::Large).tooltip(super::play_tip(game.store)).show(ui, theme) {
            acts.push(Act::Play(game.clone()));
        }
        if let Some(b) = game.store_button() {
            if widgets::Button::secondary(b.label).tooltip(b.tip).show(ui, theme) {
                acts.push(Act::Open(b.open));
            }
        }
        // With its program gone, the folder itself is what is there to show.
        let shown = if game.state == State::ProgramGone { game.folder.clone() } else { game.show.clone() };
        if game.folder_here && widgets::Button::ghost(super::SHOW_IN_FOLDER).tooltip("Show it in the computer's file manager.").show(ui, theme) {
            acts.push(Act::Open(crate::games::Launch::ShowInFolder(shown)));
        }
    });
    if game.store != Store::Added {
        return;
    }
    let editing = page.renaming.as_ref().filter(|(p, _)| super::same_file(p, &game.show)).map(|(_, n)| n.clone());
    ui.horizontal_wrapped(|ui| match editing {
        Some(typed) => {
            let mut typed = typed;
            let edit = ui.add(egui::TextEdit::singleline(&mut typed).desired_width(240.0).hint_text("Its name"));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if edit.changed() {
                acts.push(Act::StartRename(game.show.clone(), typed.clone()));
            }
            if widgets::Button::primary(super::SAVE).size(widgets::ButtonSize::Small).show(ui, theme) || enter {
                acts.push(Act::Rename(game.show.clone(), typed.clone()));
            }
            if widgets::Button::secondary(super::CANCEL).size(widgets::ButtonSize::Small).show(ui, theme) {
                acts.push(Act::CancelRename);
            }
        }
        None => {
            if widgets::Button::ghost(super::RENAME).size(widgets::ButtonSize::Small).tooltip("Change the name it shows here.").show(ui, theme) {
                acts.push(Act::StartRename(game.show.clone(), game.name.clone()));
            }
            if widgets::Button::ghost(super::REMOVE).size(widgets::ButtonSize::Small).tooltip("Take it off this list. The game itself is not touched.").show(ui, theme) {
                acts.push(Act::Remove(game.show.clone()));
            }
        }
    });
}

/// Its numbers, each only when its store keeps it.
fn about(ui: &mut egui::Ui, theme: &Theme, game: &Game) {
    let d = &game.details;
    let mut rows: Vec<(&str, String)> = Vec::new();
    if let Some(m) = d.played_minutes.filter(|m| *m > 0) {
        rows.push(("Played", super::played_words(m)));
    }
    if let Some(t) = d.last_played {
        rows.push(("Last played", super::day_words(t, false)));
    }
    if let Some(s) = d.size {
        rows.push(("Size on disk", super::size_words(s)));
    }
    if let Some(t) = d.updated {
        rows.push(("Last updated", super::day_words(t, false)));
    }
    if let Some(v) = &d.version {
        rows.push(("Version", v.clone()));
    }
    rows.push(("Installed in", game.folder.display().to_string()));
    heading(ui, theme, ABOUT, None);
    shelf::facts(ui, theme, ("games_about", &game.id), &rows);
}

/// Its achievements: how many of how many, a bar, and the list (the unlocked first, with the day).
fn achievements(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, game: &Game, acts: &mut Vec<Act>) {
    let Some(a) = &game.details.achievements else { return };
    let count = match a.unlocked() {
        Some(n) => format!("{n} of {}", a.total()),
        None => format!("{}", a.total()),
    };
    heading(ui, theme, ACHIEVEMENTS, Some(&count));
    match a.unlocked() {
        Some(n) => cover_tile::thin_bar(ui, theme, n as f32 / a.total().max(1) as f32, ui.available_width()),
        None => {
            ui.label(RichText::new(ACHIEVEMENTS_UNKNOWN).size(theme.font_size_small).color(theme.text_muted()));
        }
    }
    ui.add_space(theme.spacing_sm);
    let shown = if page.show_all.achievements { a.list.len() } else { FIRST_ACHIEVEMENTS.min(a.list.len()) };
    for item in &a.list[..shown] {
        let unlocked = item.unlocked == Some(true);
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_body + theme.spacing_sm), Sense::hover());
            if unlocked {
                icons::paint_check(ui.painter(), icon, theme.success());
            } else {
                icons::paint_lock(ui.painter(), icon, theme.text_muted());
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let color = if unlocked { theme.text_primary() } else { theme.text_secondary() };
                    ui.label(RichText::new(&item.name).size(theme.font_size_body).color(color));
                    if let Some(at) = item.unlocked_at {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(super::day_words(at, true)).size(theme.font_size_small).color(theme.text_muted()));
                        });
                    }
                });
                let description = if item.hidden && !unlocked { HIDDEN } else { item.description.as_str() };
                if !description.is_empty() {
                    ui.add(egui::Label::new(RichText::new(description).size(theme.font_size_small).color(theme.text_muted())).wrap());
                }
            });
        });
        ui.add_space(theme.spacing_xs);
    }
    if a.list.len() > FIRST_ACHIEVEMENTS {
        let words = if page.show_all.achievements { "Show fewer".to_string() } else { format!("Show all {}", a.list.len()) };
        if widgets::Button::ghost(&words).size(widgets::ButtonSize::Small).show(ui, theme) {
            acts.push(Act::ShowAll(ShowAll { achievements: !page.show_all.achievements, ..page.show_all }));
        }
    }
}

/// The person's screenshots: small, newest first; a click shows one large.
fn screenshots(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, game: &Game, acts: &mut Vec<Act>) {
    let Some(s) = &game.details.screenshots else { return };
    heading(ui, theme, SCREENSHOTS, Some(&s.total.to_string()));
    let gap = theme.spacing_sm;
    let cell_w = ((ui.available_width() - gap * (SHOTS_ACROSS as f32 - 1.0)) / SHOTS_ACROSS as f32).floor();
    let cell = Vec2::new(cell_w, (cell_w * 9.0 / 16.0).round());
    let shown = if page.show_all.screenshots { MOST_SCREENSHOTS.min(s.shots.len()) } else { FIRST_SCREENSHOTS.min(s.shots.len()) };
    for (row, chunk) in s.shots[..shown].chunks(SHOTS_ACROSS).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (col, shot) in chunk.iter().enumerate() {
                let index = row * SHOTS_ACROSS + col;
                let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                // Steam's own small copy when there is one: quicker to read than the picture.
                let source = shot.thumb.as_deref().unwrap_or(&shot.path);
                let picture = thumbnails::get(ui.ctx(), source, cell_w);
                paint_shot(ui, theme, rect, picture, resp.hovered());
                if resp.clicked() {
                    acts.push(Act::ShowShot(index));
                }
                resp.on_hover_text("Show it large");
            }
        });
        ui.add_space(gap);
    }
    ui.horizontal_wrapped(|ui| {
        if s.shots.len() > FIRST_SCREENSHOTS {
            let words = if page.show_all.screenshots { "Show fewer".to_string() } else { format!("Show all {}", s.total) };
            if widgets::Button::ghost(&words).size(widgets::ButtonSize::Small).show(ui, theme) {
                acts.push(Act::ShowAll(ShowAll { screenshots: !page.show_all.screenshots, ..page.show_all }));
            }
        }
        if widgets::Button::ghost(super::SHOW_IN_FOLDER).size(widgets::ButtonSize::Small).tooltip("Show the folder Steam keeps them in.").show(ui, theme) {
            acts.push(Act::Open(crate::games::Launch::ShowInFolder(s.folder.clone())));
        }
    });
    if page.show_all.screenshots && s.total > MOST_SCREENSHOTS {
        ui.label(RichText::new(format!("The newest {MOST_SCREENSHOTS} are shown here; Show in folder has them all.")).size(theme.font_size_small).color(theme.text_muted()));
    }
}

/// One screenshot in its cell: the picture cropped to fill, or a faint picture mark while it is read.
fn paint_shot(ui: &egui::Ui, theme: &Theme, rect: Rect, picture: Option<(egui::TextureId, Vec2)>, hovered: bool) {
    let corners = egui::CornerRadius::same(theme.border_radius as u8);
    match picture {
        Some((tex, size)) => {
            ui.painter().add(egui::epaint::RectShape::filled(rect, corners, egui::Color32::WHITE).with_texture(tex, cover_tile::fill_uv(size, rect.size())));
        }
        None => {
            ui.painter().rect_filled(rect, corners, theme.bg_tertiary());
            let mark = Rect::from_center_size(rect.center(), Vec2::splat(rect.height() * 0.4));
            icons::paint_picture(ui.painter(), mark, theme.text_muted());
        }
    }
    let stroke = if hovered { Stroke::new(2.0, theme.accent()) } else { Stroke::new(1.0, theme.border()) };
    ui.painter().rect_stroke(rect, corners, stroke, egui::StrokeKind::Inside);
}

/// Its Workshop mods: how many and how large, then each by its number, its size and its day.
fn mods(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, game: &Game, acts: &mut Vec<Act>) {
    let Some(w) = &game.details.workshop else { return };
    let n = w.items.len();
    let note = format!("Steam Workshop: {n} {}, {}", if n == 1 { "item" } else { "items" }, super::size_words(w.total_size));
    heading(ui, theme, MODS, Some(&note));
    let shown = if page.show_all.mods { n } else { FIRST_MODS.min(n) };
    for item in &w.items[..shown] {
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_body), Sense::hover());
            icons::paint_wrench(ui.painter(), icon.shrink(1.0), theme.text_muted());
            ui.label(RichText::new(format!("Item {}", item.id)).size(theme.font_size_body).color(theme.text_primary()));
            let mut facts = Vec::new();
            if let Some(size) = item.size {
                facts.push(super::size_words(size));
            }
            if let Some(t) = item.updated {
                facts.push(format!("updated {}", super::day_words(t, true)));
            }
            if !facts.is_empty() {
                ui.label(RichText::new(facts.join(", ")).size(theme.font_size_small).color(theme.text_muted()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(open) = links::workshop_item(&item.id) {
                    if widgets::Button::ghost("Open in Steam").size(widgets::ButtonSize::Small).tooltip("Shows its page in Steam.").show(ui, theme) {
                        acts.push(Act::Open(open));
                    }
                }
            });
        });
    }
    ui.horizontal_wrapped(|ui| {
        if n > FIRST_MODS {
            let words = if page.show_all.mods { "Show fewer".to_string() } else { format!("Show all {n}") };
            if widgets::Button::ghost(&words).size(widgets::ButtonSize::Small).show(ui, theme) {
                acts.push(Act::ShowAll(ShowAll { mods: !page.show_all.mods, ..page.show_all }));
            }
        }
        if w.folder_here && widgets::Button::ghost(super::SHOW_IN_FOLDER).size(widgets::ButtonSize::Small).tooltip("Show the folder Steam keeps its mods in.").show(ui, theme) {
            acts.push(Act::Open(crate::games::Launch::ShowInFolder(w.folder.clone())));
        }
    });
    ui.label(RichText::new(MODS_NOTE).size(theme.font_size_small).color(theme.text_muted()));
}

/// Its links: in the store's own app, and on the web (each opened only on a click).
fn links_section(ui: &mut egui::Ui, theme: &Theme, game: &Game, acts: &mut Vec<Act>) {
    let all = links::links(game);
    if all.is_empty() {
        return;
    }
    heading(ui, theme, LINKS, None);
    let in_store: Vec<_> = all.iter().filter(|l| matches!(l.opens, Opens::InStore(_))).collect();
    let web: Vec<_> = all.iter().filter(|l| matches!(l.opens, Opens::Web(_))).collect();
    for (title, group) in [(IN_STEAM, in_store), (ON_THE_WEB, web)] {
        if group.is_empty() {
            continue;
        }
        ui.label(RichText::new(title).size(theme.font_size_small).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            for link in group {
                if widgets::Button::secondary(link.label).size(widgets::ButtonSize::Small).tooltip(link.tip).show(ui, theme) {
                    acts.push(match &link.opens {
                        Opens::InStore(l) => Act::Open(l.clone()),
                        Opens::Web(u) => Act::OpenWeb(u.clone()),
                    });
                }
            }
        });
        ui.add_space(theme.spacing_xs);
    }
}

/// A screenshot shown large, over the page: Previous, Next (and the arrow keys), Show in folder.
pub(super) fn viewer(ctx: &egui::Context, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let page = &state.games;
    let (Some(index), Some(game)) = (page.viewer, page.selected_game()) else { return acts };
    let Some(shots) = &game.details.screenshots else { return acts };
    let Some(shot) = shots.shots.get(index) else { return acts };
    let mut open = true;
    let title = format!("{}: screenshot {} of {}", game.name, index + 1, shots.shots.len());
    let screen = ctx.screen_rect().size();
    let max = Vec2::new((screen.x * 0.8).max(320.0), (screen.y * 0.7).max(200.0));
    widgets::dialog(ctx, theme, "games_screenshot", &title, &mut open, |ui| {
        let picture = thumbnails::get(ui.ctx(), &shot.path, max.x);
        let size = match picture {
            Some((_, s)) => {
                let k = (max.x / s.x).min(max.y / s.y);
                Vec2::new(s.x * k, s.y * k)
            }
            None => Vec2::new(max.x.min(960.0), max.x.min(960.0) * 9.0 / 16.0),
        };
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        paint_shot(ui, theme, rect, picture, false);
        ui.add_space(theme.spacing_sm);
        let (left, right) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        ui.horizontal(|ui| {
            if (widgets::Button::secondary("Previous").disabled(index == 0).show(ui, theme) || left) && index > 0 {
                acts.push(Act::ShowShot(index - 1));
            }
            if (widgets::Button::secondary("Next").disabled(index + 1 >= shots.shots.len()).show(ui, theme) || right) && index + 1 < shots.shots.len() {
                acts.push(Act::ShowShot(index + 1));
            }
            if widgets::Button::ghost(super::SHOW_IN_FOLDER).tooltip("Show it in the computer's file manager.").show(ui, theme) {
                acts.push(Act::Open(crate::games::Launch::ShowInFolder(shot.path.clone())));
            }
        });
    });
    if !open {
        acts.push(Act::CloseShot);
    }
    acts
}
