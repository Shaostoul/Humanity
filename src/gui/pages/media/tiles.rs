//! The Media page's grid: the filters above it, each tile (what it shows, and its actions), and
//! the Remove confirmation.
//!
//! WHAT A TILE SHOWS ([`show_of`]), in order, the first that applies:
//! 1. Hidden: the protected setup leaves it out, either because it is from someone who is not a
//!    friend (the preset's own line, as in the chat) or because it carries a content label (the
//!    design: "The protected setup always hides labelled items").
//! 2. Label: a labelled item shows its labels instead of its picture until it is clicked.
//! 3. Ask: a picture from someone who is not a friend waits for a click, as in the chat
//!    (gui/pages/chat/attach_view.rs; everything on this page comes from this server, so there is
//!    no other-website case).
//! 4. TooBig: a picture larger than this app shows in place (the image cache's 16 MB) says so,
//!    and opens its card, where Download takes it whatever its size.
//! 5. Load: the picture itself (until increment 4's previews), or the kind's mark.
//!
//! A click on a Label or Ask tile is remembered in the image cache's "allowed" set, as the chat's
//! click-to-load is: for this session, one item at a time. A picture that could not be loaded
//! says so and tries again on a click; a members-only one is also asked for again whenever the
//! session's upload token changes (media.rs `note_token`), so a refusal never outlives the token
//! it was given to.
//!
//! EVERY ACTION NAMES THE ITEM'S FILE (the relay's W5): `Item::reference`; an item whose file the
//! server did not name is not acted on, and the page says why.

use egui::{Color32, RichText, Rounding, Sense, Stroke, Vec2};

use super::api::{self, Item};
use super::{kinds, signed_in_here, signer_or_say, spawn, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::image_cache::{ImageStatus, MAX_IMAGE_BYTES, PRIVATE_KEY_PREFIX};
use crate::gui::GuiState;

/// A tile's size, and its picture's.
pub(crate) const TILE_W: f32 = 220.0;
const THUMB_H: f32 = 150.0;

/// The line instead of a labelled item's picture while the protected setup is on.
pub(crate) const LABEL_HIDDEN_LINE: &str = "Hidden: the protected setup leaves out items with a content label.";
/// The second line of a labelled tile.
pub(crate) const CLICK_TO_SHOW: &str = "Click to show.";
/// What a picture that could not be loaded says.
pub(crate) const COULD_NOT_LOAD: &str = "This picture could not be loaded. Click to try again.";

/// What a picture too large to show in place says.
pub(crate) fn too_big_words(item: &Item) -> String {
    format!(
        "This picture is {}, larger than this app shows in place ({}). Click to open it and download it.",
        api::size_words(item.size_bytes),
        api::size_words(MAX_IMAGE_BYTES as i64)
    )
}

/// What a tile shows in place of its picture, or the picture.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Show {
    /// Left out by the protected setup, with the line to say so.
    Hidden(String),
    /// Its labels' names, until clicked.
    Label(Vec<String>),
    /// From someone who is not a friend: a click first.
    Ask,
    /// A picture larger than the image cache shows: its card instead.
    TooBig,
    /// The picture (or the kind's mark).
    Load,
}

/// The name an item's picture (and its click) goes by in the image cache: the full address of a
/// public picture, or, for a members-only one, a name made from the server and the item number
/// under the private prefix, so the session's upload token is never a texture's name, the
/// viewer's title or something Copy URL could copy.
pub(crate) fn cache_key(state: &GuiState, item: &Item) -> String {
    if item.members_only() {
        let h = blake3::hash(format!("{}\n{}", state.media.server, item.id).as_bytes());
        format!("{PRIVATE_KEY_PREFIX}media-{}", &h.to_hex()[..32])
    } else {
        api::file_url(&super::base_of(state), item)
    }
}

/// The names of `item`'s labels, as the server lists them (its id when the server sent no name).
pub(crate) fn label_names(state: &GuiState, item: &Item) -> Vec<String> {
    item.labels
        .iter()
        .map(|id| {
            state.media.list.labels.iter().find(|l| &l.id == id).map(|l| l.label.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone())
        })
        .collect()
}

/// Whether `item` is a picture too large for the image cache to show in place.
pub(crate) fn too_big(item: &Item) -> bool {
    item.kind == "picture" && item.size_bytes > MAX_IMAGE_BYTES as i64
}

/// Decide what `item`'s tile shows (see the file header).
pub(crate) fn show_of(state: &GuiState, item: &Item) -> Show {
    let own = !item.uploader_key.is_empty() && item.uploader_key == state.profile_public_key;
    if crate::engine::protected::hides_pictures_from(state, &item.uploader_key) {
        let line = state.protected.preset.as_ref().map(|p| p.picture_hidden_line.clone()).unwrap_or_default();
        return Show::Hidden(line);
    }
    if !item.labels.is_empty() && crate::engine::protected::is_on(state) && !own {
        return Show::Hidden(LABEL_HIDDEN_LINE.to_string());
    }
    let allowed = state.image_cache.is_allowed(&cache_key(state, item));
    if !item.labels.is_empty() && !allowed {
        return Show::Label(label_names(state, item));
    }
    if item.kind == "picture" && !own && !allowed && !crate::engine::protected::picture_friend(state, &item.uploader_key) {
        return Show::Ask;
    }
    if too_big(item) {
        return Show::TooBig;
    }
    Show::Load
}

/// Ask the image cache for `item`'s picture: a public one by its address, a members-only one
/// through the members' file route with this session's upload token in a header, on a worker
/// thread. A picture too large to show is never asked for.
pub(crate) fn request_picture(state: &mut GuiState, item: &Item) {
    let key = cache_key(state, item);
    if too_big(item) || !matches!(state.image_cache.status(&key), ImageStatus::Idle) {
        return;
    }
    if item.members_only() {
        let token = crate::gui::pages::chat::upload_token_here(state);
        let url = api::file_url(&super::base_of(state), item);
        let header = api::token_header(item, token.as_deref());
        state.image_cache.request_bytes(&key, move || {
            let header = header.as_ref().map(|(name, value)| (*name, value.as_str()));
            crate::gui::widgets::image_cache::download_bytes_with(&url, MAX_IMAGE_BYTES, header)
        });
    } else {
        state.image_cache.request(&key);
    }
}

/// The filters row: category, kind, person, My picks, and the search box.
pub(crate) fn draw_filters(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let before = state.media.filters.clone();
    let signed_in = signed_in_here(state);
    ui.horizontal_wrapped(|ui| {
        let m = &mut state.media;
        let cat_name = m.filters.category.and_then(|c| m.list.categories.iter().find(|x| x.id == c)).map(|c| c.name.clone()).unwrap_or_else(|| "All categories".into());
        egui::ComboBox::from_id_salt("media_category").selected_text(cat_name).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.category, None, "All categories");
            for c in &m.list.categories {
                ui.selectable_value(&mut m.filters.category, Some(c.id), &c.name);
            }
        });
        let kind_name = m.filters.kind.as_deref().and_then(kinds::kind).map(|k| k.label.clone()).unwrap_or_else(|| "All kinds".into());
        egui::ComboBox::from_id_salt("media_kind").selected_text(kind_name).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.kind, None, "All kinds");
            for k in kinds::kinds() {
                ui.selectable_value(&mut m.filters.kind, Some(k.id.clone()), &k.label);
            }
        });
        let person = m.filters.by.as_ref().and_then(|k| m.people.iter().find(|(key, _)| key == k)).map(|(_, n)| n.clone()).unwrap_or_else(|| "Everyone".into());
        egui::ComboBox::from_id_salt("media_person").selected_text(person).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.by, None, "Everyone");
            for (key, name) in &m.people {
                ui.selectable_value(&mut m.filters.by, Some(key.clone()), name);
            }
        });
        if signed_in {
            ui.checkbox(&mut m.filters.my_picks, "My picks").on_hover_text("Only the items you picked.");
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut m.search).hint_text("Search titles").desired_width(180.0));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter || widgets::Button::secondary("Search").show(ui, theme) {
            m.filters.q = m.search.trim().to_string();
        }
    });
    if state.media.filters != before {
        super::reload_list(state);
    }
}

/// The grid of `items`, as many to a row as fit.
pub(crate) fn draw_grid(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, items: &[Item]) {
    let gap = theme.spacing_md;
    let per_row = ((ui.available_width() + gap) / (TILE_W + gap)).floor().max(1.0) as usize;
    for row in items.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for item in row {
                draw_tile(ui, theme, state, item);
            }
        });
        ui.add_space(gap);
    }
}

/// One tile: its picture (or what stands in for it), its title, who put it there, its badges, and
/// its actions.
pub(crate) fn draw_tile(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item) {
    let show = show_of(state, item);
    if show == Show::Load && item.kind == "picture" {
        request_picture(state, item);
    }
    egui::Frame::none()
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, if item.featured { theme.accent() } else { theme.border() }))
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.spacing_sm)
        .show(ui, |ui| {
            ui.set_width(TILE_W - 2.0 * theme.spacing_sm);
            ui.vertical(|ui| {
                draw_face(ui, theme, state, item, &show);
                ui.add_space(theme.spacing_xs);
                ui.add(egui::Label::new(RichText::new(item.display_title()).size(theme.font_size_body).color(theme.text_primary()).strong()).truncate());
                ui.add(egui::Label::new(RichText::new(format!("by {}", item.by_name())).size(theme.font_size_small).color(theme.text_secondary())).truncate());
                ui.horizontal_wrapped(|ui| {
                    let picks = if item.picks == 1 { "1 pick".to_string() } else { format!("{} picks", item.picks) };
                    ui.label(RichText::new(picks).size(theme.font_size_small).color(theme.text_muted()));
                    ui.label(RichText::new(api::size_words(item.size_bytes)).size(theme.font_size_small).color(theme.text_muted()));
                    if item.featured {
                        ui.label(RichText::new("Featured").size(theme.font_size_small).color(theme.accent()));
                    }
                    if item.kept {
                        ui.label(RichText::new(KEPT_BADGE).size(theme.font_size_small).color(theme.success()))
                            .on_hover_text("This server keeps it: it counts against the Media space, not against its uploader's share.");
                    }
                    if item.members_only() {
                        ui.label(RichText::new("Members only").size(theme.font_size_small).color(theme.info()));
                    }
                    for name in label_names(state, item) {
                        ui.label(RichText::new(name).size(theme.font_size_small).color(theme.warning()));
                    }
                });
                draw_actions(ui, theme, state, item);
            });
        });
}

/// The badge of an item the server keeps.
pub(crate) const KEPT_BADGE: &str = "Kept by the server";

/// The picture area: what [`show_of`] decided.
fn draw_face(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item, show: &Show) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, THUMB_H), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(theme.border_radius as u8), theme.bg_tertiary());
    let font = egui::FontId::proportional(theme.font_size_small);
    let two_lines = |first: &str, second: &str, color: Color32| {
        let galley = painter.layout(first.to_string(), font.clone(), color, width - 16.0);
        let top = rect.center().y - galley.size().y / 2.0 - theme.font_size_small / 2.0;
        painter.galley(egui::pos2(rect.left() + 8.0, top), galley, color);
        painter.text(egui::pos2(rect.left() + 8.0, rect.bottom() - 10.0 - theme.font_size_small / 2.0), egui::Align2::LEFT_CENTER, second, font.clone(), theme.text_muted());
    };
    let wrapped = |text: &str, color: Color32| {
        let galley = painter.layout(text.to_string(), font.clone(), color, width - 16.0);
        painter.galley(egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0), galley, color);
    };
    let key = cache_key(state, item);
    match show {
        Show::Hidden(line) => {
            wrapped(line, theme.text_muted());
            return;
        }
        Show::Label(names) => {
            two_lines(&names.join(", "), CLICK_TO_SHOW, theme.warning());
            if resp.clicked() {
                state.image_cache.allow(&key);
            }
        }
        Show::Ask => {
            let first = format!("{} is not your friend. Pictures show only when you choose.", item.by_name());
            two_lines(&first, CLICK_TO_SHOW, theme.text_primary());
            if resp.clicked() {
                state.image_cache.allow(&key);
            }
        }
        Show::TooBig => {
            wrapped(&too_big_words(item), theme.text_secondary());
            if resp.clicked() {
                super::open::open_card(state, item);
            }
        }
        Show::Load => {
            if item.kind == "picture" {
                match state.image_cache.status(&key) {
                    ImageStatus::Ready { width: w, height: h } => {
                        if let Some(tex) = state.image_cache.get_texture(&key) {
                            // Fit the whole picture inside the area, centred.
                            let aspect = w.max(1) as f32 / h.max(1) as f32;
                            let (mut dw, mut dh) = (rect.width(), rect.width() / aspect);
                            if dh > rect.height() {
                                dh = rect.height();
                                dw = dh * aspect;
                            }
                            let img = egui::Rect::from_center_size(rect.center(), Vec2::new(dw, dh));
                            let mut mesh = egui::Mesh::with_texture(tex.id());
                            mesh.add_rect_with_uv(img, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                            painter.add(egui::Shape::mesh(mesh));
                        }
                    }
                    ImageStatus::Failed(_) => {
                        wrapped(COULD_NOT_LOAD, theme.danger());
                        if resp.clicked() {
                            state.image_cache.retry(&key);
                            return;
                        }
                    }
                    ImageStatus::Fetching | ImageStatus::Idle => {
                        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Loading...", font, theme.text_muted());
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                    }
                }
            } else {
                let mark = egui::Rect::from_center_size(rect.center() - Vec2::new(0.0, 10.0), Vec2::splat(48.0));
                paint_mark(&painter, mark, kinds::mark(&item.kind), theme.text_secondary());
                painter.text(egui::pos2(rect.center().x, mark.bottom() + 14.0), egui::Align2::CENTER_CENTER, kinds::one_name(&item.kind), font, theme.text_muted());
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                super::open::open_item(state, item);
            }
        }
    }
}

/// Paint a kind's mark (data/media/kinds.json `mark`) with the shared icon painters, never a font
/// glyph (CLAUDE.md, icon_glyph_lint).
fn paint_mark(painter: &egui::Painter, rect: egui::Rect, mark: &str, color: Color32) {
    use crate::gui::widgets::icons;
    match mark {
        "play" => icons::paint_play(painter, rect, color),
        "box" => icons::paint_box(painter, rect, color),
        "picture" => icons::paint_picture(painter, rect, color),
        _ => icons::paint_scroll(painter, rect, color),
    }
}

/// The item's reference for an action, or None with the reason said on the page.
fn reference_or_say(state: &mut GuiState, item: &Item) -> Option<api::ItemRef> {
    match item.reference() {
        Ok(r) => Some(r),
        Err(words) => {
            state.media.status = words;
            None
        }
    }
}

/// A tile's buttons: Pick or Unpick (not on your own), Remove (yours, or any as an admin), Feature
/// or Unfeature (admins), Keep on the server or Stop keeping (admins and moderators), Report (not
/// on your own).
fn draw_actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item) {
    if !signed_in_here(state) {
        return;
    }
    let own = item.uploader_key == state.profile_public_key;
    let admin = super::is_admin(state);
    let keeper = super::may_keep(state);
    let small = widgets::ButtonSize::Small;
    ui.horizontal_wrapped(|ui| {
        if !own {
            let picked = item.picked_by_me;
            let label = if picked { "Unpick" } else { "Pick" };
            let tip = if picked { "Take it off your Picks." } else { "Add it to your Picks, the list on your profile. It stays theirs, credited to them." };
            if widgets::Button::secondary(label).size(small).tooltip(tip).show(ui, theme) {
                if let (Some(r), Some(signer)) = (reference_or_say(state, item), signer_or_say(state)) {
                    spawn(state, Job::Pick { id: item.id, on: !picked }, api::pick_request(&signer, &r, !picked));
                }
            }
        }
        if (own || admin) && widgets::Button::danger("Remove").size(small).show(ui, theme) {
            state.media.confirm_remove = Some(item.clone());
        }
        if admin {
            let label = if item.featured { "Unfeature" } else { "Feature" };
            if widgets::Button::secondary(label).size(small).tooltip("Featured items lead the Media page.").show(ui, theme) {
                if let (Some(r), Some(signer)) = (reference_or_say(state, item), signer_or_say(state)) {
                    spawn(state, Job::Feature { id: item.id, on: !item.featured }, api::feature_request(&signer, &r, !item.featured));
                }
            }
        }
        if keeper {
            let (label, tip) = if item.kept { (STOP_KEEPING, STOP_KEEPING_TIP) } else { (KEEP, KEEP_TIP) };
            if widgets::Button::secondary(label).size(small).tooltip(tip).show(ui, theme) {
                if let (Some(r), Some(signer)) = (reference_or_say(state, item), signer_or_say(state)) {
                    spawn(state, Job::Keep { id: item.id, on: !item.kept, from_admin: false }, api::keep_request(&signer, &r, !item.kept));
                }
            }
        }
        if !own && widgets::Button::ghost("Report").size(small).tooltip(REPORT_TIP).show(ui, theme) {
            crate::engine::report::open_for_media(state, item);
        }
    });
}

/// Keep on the server, and what it does.
pub(crate) const KEEP: &str = "Keep on the server";
pub(crate) const KEEP_TIP: &str = "This server carries it from now on: it stops counting against its uploader's share, so they can upload something new. It still counts against the Media space.";
pub(crate) const STOP_KEEPING: &str = "Stop keeping";
pub(crate) const STOP_KEEPING_TIP: &str = "It counts against its uploader's share again, even past it; nothing is deleted for that.";

/// What Report says when hovered.
pub(crate) const REPORT_TIP: &str = "Tell this server's admins and moderators about this item. They are not told who reported it.";

/// The Remove confirmation, naming what happens to the file (the item's own `keeps_file`).
pub(crate) fn draw_confirm_remove(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(item) = state.media.confirm_remove.clone() else { return };
    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    let words = api::remove_words(&item);
    widgets::dialog(ctx, theme, "media_confirm_remove", "Remove from Media", &mut open, |ui| {
        ui.label(RichText::new(item.display_title()).size(theme.font_size_body).color(theme.text_primary()).strong());
        ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_secondary()));
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            confirmed = widgets::Button::danger("Remove").show(ui, theme);
            cancelled = widgets::Button::secondary("Cancel").show(ui, theme);
        });
    });
    if confirmed {
        if let (Some(r), Some(signer)) = (reference_or_say(state, &item), signer_or_say(state)) {
            let req = api::remove_request(&signer, &api::RemoveTarget::Item(r));
            spawn(state, Job::Remove { id: Some(item.id), from_admin: false }, req);
        }
    }
    if confirmed || cancelled || !open {
        state.media.confirm_remove = None;
    }
}
