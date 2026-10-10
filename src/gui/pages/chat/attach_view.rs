//! Showing a file sent into a private conversation (10k of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): a `[[hum:file:v1]]` marker in a direct
//! message or a P2P group message, shown the same way in both.
//!
//! A picture (by its `mime`) is fetched, decrypted and drawn inline. A picture from someone who
//! is not a friend waits for a click (6.2's "Pictures and files from non-friends: click to
//! load"), and one whose ciphertext sits on another website waits for the click that names the
//! website (loading it shows that website this device's address). While the protected setup
//! hides non-friends' pictures nothing is fetched at all: the message row already shows the
//! preset's line instead (chat.rs, `protected::picture_hidden_line`). Anything else is a card
//! with its name and size and a Save button that fetches and decrypts only when pressed.
//!
//! Nothing here runs on the frame but drawing: the fetch, the decryption and the decoding run
//! on the image cache's worker threads (`ImageCache::request_bytes`, `save_with`), and the
//! decrypted bytes stay in memory (as the picture's pixels) unless the person presses Save. A
//! download or decryption that fails says "This file could not be opened." and nothing else:
//! never the ciphertext, never the error's own text.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use crate::gui::widgets::image_cache::{route_image, ImageCache, ImageRoute, ImageStatus, PRIVATE_KEY_PREFIX};
use crate::net::dm_pq::DmAttachment;

/// The one line shown when a private file cannot be fetched or decrypted.
pub(crate) const CANNOT_OPEN: &str = "This file could not be opened.";

/// The largest ciphertext fetched: the send cap plus AES-GCM's 16-byte tag. A marker pointing at
/// something bigger is refused before it is buffered.
const MAX_CIPHERTEXT: usize = ATTACH_MAX_BYTES as usize + 16;

/// Picture types drawn inline: what the image decoder reads. Any other type is a card.
const INLINE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Where the picture sits under the message text, and its width.
const INDENT: f32 = 40.0;
const THUMB_W: f32 = 240.0;

/// The private file a message carries, if it is one.
pub(crate) fn file_in(msg: &ChatMessage) -> Option<DmAttachment> {
    crate::net::dm_pq::parse_file_marker(&msg.content)
}

/// Whether the file is drawn as a picture rather than a card.
pub(crate) fn is_inline(att: &DmAttachment) -> bool {
    INLINE_TYPES.contains(&att.mime.to_ascii_lowercase().as_str())
}

/// The name a private file goes by in the image cache: a hash of its address, key and nonce. The
/// key is in it so a tampered marker can never reuse a picture another marker opened, and it is
/// hashed so the file's key is never a texture name or the image viewer's title.
pub(crate) fn cache_key(att: &DmAttachment) -> String {
    let h = blake3::hash(format!("{}\n{}\n{}", att.url, att.k, att.n).as_bytes());
    format!("{PRIVATE_KEY_PREFIX}{}", &h.to_hex()[..32])
}

/// How a private file is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Show {
    /// The protected setup leaves it out; the message row says so.
    Hidden,
    /// From someone who is not a friend: waits for a click.
    AskNotFriend,
    /// Its ciphertext sits on another website, named here: waits for a click.
    AskOtherWebsite(String),
    /// Fetched, decrypted and drawn now.
    Load,
}

/// What `draw` needs: the cache key, the full address of the ciphertext, and how it is shown.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub key: String,
    pub url: String,
    pub show: Show,
}

/// Decide how `att`, carried by `msg`, is shown. The protected setup's rule comes first and
/// nothing overrides it; then a click already given; then another website's address; then
/// whether the sender is a friend (the pictures rule's own test, `picture_friend`). Our own
/// files load by themselves.
pub(crate) fn plan(state: &GuiState, msg: &ChatMessage, att: &DmAttachment) -> Plan {
    let key = cache_key(att);
    // The server this message came through, as for any picture in a message.
    let server = if msg.server.is_empty() { state.server_url.as_str() } else { msg.server.as_str() };
    let route = route_image(&att.url, server);
    let url = route.url().to_string();
    let own = !msg.sender_key.is_empty() && msg.sender_key == state.profile_public_key;
    let show = if crate::engine::protected::hides_pictures_from(state, &msg.sender_key) {
        Show::Hidden
    } else if state.image_cache.is_allowed(&key) {
        Show::Load
    } else if let ImageRoute::AskFirst { host, .. } = route {
        Show::AskOtherWebsite(host)
    } else if !own && !crate::engine::protected::picture_friend(state, &msg.sender_key) {
        Show::AskNotFriend
    } else {
        Show::Load
    };
    Plan { key, url, show }
}

/// Fetch the ciphertext at `url` with `fetch` (capped) and decrypt it with the marker's key and
/// nonce. Any failure is an error; the caller shows `CANNOT_OPEN`, never the error.
pub(crate) fn open<F>(url: &str, k: &str, n: &str, fetch: F) -> Result<Vec<u8>, String>
where
    F: FnOnce(&str, usize) -> Result<Vec<u8>, String>,
{
    let ciphertext = fetch(url, MAX_CIPHERTEXT)?;
    crate::net::dm_pq::decrypt_attachment(&ciphertext, k, n)
}

/// A file name safe to write in the downloads folder: the sender chose it, so only its last part
/// is kept (a name like `..\..\x` cannot leave the folder), characters Windows refuses become
/// `_`, and an empty or reserved name becomes "attachment".
pub(crate) fn save_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c })
        .take(120)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if cleaned.is_empty() || reserved {
        "attachment".to_string()
    } else {
        cleaned
    }
}

/// Write `bytes` into `dir` under `name` made safe, never over an existing file: "x (2).pdf",
/// "x (3).pdf" and so on. Returns where it went.
pub(crate) fn save_to(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| format!("folder: {e}"))?;
    let name = save_name(name);
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name.as_str(), ""),
    };
    for n in 1..=999 {
        let candidate = if n == 1 { name.clone() } else { format!("{stem} ({n}){ext}") };
        let path = dir.join(candidate);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(bytes).map_err(|e| format!("write: {e}"))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("create: {e}")),
        }
    }
    Err("no free file name".to_string())
}

/// What Save does, on its worker thread: fetch with `fetch`, decrypt, and write the decrypted
/// file into `dir` (`save_to`). A file that cannot be opened writes nothing.
pub(crate) fn save_file<F>(url: &str, k: &str, n: &str, name: &str, dir: &std::path::Path, fetch: F) -> Result<std::path::PathBuf, String>
where
    F: FnOnce(&str, usize) -> Result<Vec<u8>, String>,
{
    let bytes = open(url, k, n, fetch)?;
    save_to(dir, name, &bytes)
}

/// A file's size in plain words.
pub(crate) fn size_words(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", (bytes + 512) / 1024)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// A box under a message asking for a click before anything is fetched: `first` in the normal
/// colour, `second` muted. Returns true on the click. Shared with the click-to-load placeholder
/// for another website's picture (chat.rs `draw_click_to_load`).
pub(super) fn draw_ask_box(ui: &mut egui::Ui, theme: &Theme, row_bg: Color32, indent: f32, first: &str, second: &str) -> bool {
    let row_w = ui.available_width();
    let box_h = theme.font_size_small * 2.0 + 18.0;
    let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(row_w, box_h + 4.0), egui::Sense::click());
    ui.painter().rect_filled(row_rect, 0.0, row_bg);
    let box_w = (row_w - indent - 8.0).clamp(160.0, 480.0);
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(row_rect.left() + indent, row_rect.top() + 2.0),
        Vec2::new(box_w, box_h),
    );
    let hovered = resp.hovered();
    let fill = if hovered { theme.bg_tertiary() } else { row_bg };
    ui.painter().rect_filled(box_rect, Rounding::same(4), fill);
    ui.painter().rect_stroke(box_rect, Rounding::same(4), Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
    // Clipped to the box, so a long name cannot spill over the chat.
    let painter = ui.painter_at(box_rect.shrink(1.0));
    let line_h = theme.font_size_small + 4.0;
    let left = box_rect.left() + 8.0;
    let top = box_rect.top() + 6.0 + theme.font_size_small / 2.0;
    let font = egui::FontId::proportional(theme.font_size_small);
    painter.text(egui::pos2(left, top), egui::Align2::LEFT_CENTER, first, font.clone(), theme.text_primary());
    painter.text(egui::pos2(left, top + line_h), egui::Align2::LEFT_CENTER, second, font, theme.text_muted());
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// One line under a message, indented like its pictures.
fn draw_line(ui: &mut egui::Ui, theme: &Theme, row_bg: Color32, text: &str, color: Color32) {
    let row_w = ui.available_width();
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(row_w, theme.font_size_small + 10.0), egui::Sense::hover());
    ui.painter().rect_filled(row_rect, 0.0, row_bg);
    ui.painter_at(row_rect).text(
        egui::pos2(row_rect.left() + INDENT, row_rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(theme.font_size_small),
        color,
    );
}

/// Draw the private file `att` from `sender_name` under its message, as `plan` says. Borrows
/// only the cache and the viewer's slot, so the message list can keep its borrow of the
/// messages while this runs.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ui: &mut egui::Ui,
    theme: &Theme,
    cache: &mut ImageCache,
    viewer: &mut Option<String>,
    sender_name: &str,
    att: &DmAttachment,
    plan: &Plan,
    row_bg: Color32,
) {
    if plan.show == Show::Hidden {
        return; // the row already carries the protected setup's line; nothing is fetched
    }
    if is_inline(att) {
        match &plan.show {
            Show::AskNotFriend => {
                let first = format!("{sender_name} is not your friend. Pictures show only when you choose.");
                if draw_ask_box(ui, theme, row_bg, INDENT, &first, &format!("Click to show {}.", att.name)) {
                    cache.allow(&plan.key);
                }
                return;
            }
            Show::AskOtherWebsite(host) => {
                if draw_click_to_load(ui, theme, row_bg, INDENT, host) {
                    cache.allow(&plan.key);
                }
                return;
            }
            Show::Load | Show::Hidden => {}
        }
        if matches!(cache.status(&plan.key), ImageStatus::Idle) {
            let (url, k, n) = (plan.url.clone(), att.k.clone(), att.n.clone());
            cache.request_bytes(&plan.key, move || {
                open(&url, &k, &n, crate::gui::widgets::image_cache::download_bytes)
            });
        }
        match cache.status(&plan.key) {
            ImageStatus::Ready { width, height } => {
                let aspect = width as f32 / height.max(1) as f32;
                let thumb_h = (THUMB_W / aspect.max(0.1)).min(360.0).max(60.0);
                let row_w = ui.available_width();
                let (row_rect, resp) = ui.allocate_exact_size(Vec2::new(row_w, thumb_h + 4.0), egui::Sense::click());
                ui.painter().rect_filled(row_rect, 0.0, row_bg);
                if let Some(tex) = cache.get_texture(&plan.key) {
                    let img_rect = egui::Rect::from_min_size(
                        egui::pos2(row_rect.left() + INDENT, row_rect.top() + 2.0),
                        Vec2::new(THUMB_W, thumb_h),
                    );
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    mesh.add_rect_with_uv(img_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    ui.painter().add(egui::Shape::mesh(mesh));
                    ui.painter().rect_stroke(img_rect, Rounding::same(3), Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    *viewer = Some(plan.key.clone()); // the viewer shows no address for these
                }
            }
            ImageStatus::Fetching | ImageStatus::Idle => {
                draw_line(ui, theme, row_bg, &format!("Opening {}\u{2026}", att.name), theme.text_muted());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
            ImageStatus::Failed(_) => {
                draw_line(ui, theme, row_bg, CANNOT_OPEN, theme.danger());
                return;
            }
        }
    }
    draw_file_card(ui, theme, cache, att, plan, row_bg);
}

/// The card under a private file: its name, its size and Save, which fetches, decrypts and
/// writes it to the downloads folder on a worker thread, then says where it went.
fn draw_file_card(ui: &mut egui::Ui, theme: &Theme, cache: &mut ImageCache, att: &DmAttachment, plan: &Plan, row_bg: Color32) {
    let mut save = false;
    Frame::NONE
        .fill(row_bg)
        .inner_margin(egui::Margin { left: INDENT as i8, right: 8, top: 2, bottom: 4 })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::NONE
                .fill(theme.bg_card())
                .stroke(Stroke::new(1.0, theme.border()))
                .rounding(Rounding::same(4))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&att.name).size(theme.font_size_small).color(theme.text_primary()));
                        ui.label(RichText::new(size_words(att.size)).size(theme.font_size_small).color(theme.text_muted()));
                        if cache.is_saving(&plan.key) {
                            ui.label(RichText::new("Saving\u{2026}").size(theme.font_size_small).color(theme.text_muted()));
                            // Keep drawing until the worker reports, or "Saved to" waits for the mouse.
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                        } else if widgets::Button::secondary("Save").size(widgets::ButtonSize::Small).show(ui, theme) {
                            save = true;
                        }
                    });
                    if let Show::AskOtherWebsite(host) = &plan.show {
                        let note = format!("Saving this shows {host} your network address.");
                        ui.label(RichText::new(note).size(theme.font_size_small).color(theme.text_muted()));
                    }
                    if cache.save_failed(&plan.key) {
                        ui.label(RichText::new(CANNOT_OPEN).size(theme.font_size_small).color(theme.danger()));
                    } else if let Some(path) = cache.downloaded_path(&plan.key) {
                        let saved = format!("Saved to {}", path.display());
                        ui.label(RichText::new(saved).size(theme.font_size_small).color(theme.success()));
                    }
                });
        });
    if save {
        let (url, k, n, name) = (plan.url.clone(), att.k.clone(), att.n.clone(), att.name.clone());
        let dir = crate::gui::widgets::image_cache::default_downloads_dir();
        cache.save_with(&plan.key, move || {
            save_file(&url, &k, &n, &name, &dir, crate::gui::widgets::image_cache::download_bytes)
        });
    }
}

#[cfg(test)]
#[path = "attach_view_tests.rs"]
mod tests;
