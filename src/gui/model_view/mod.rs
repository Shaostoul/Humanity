//! The 3D model viewer (docs/design/model-viewer.md): a `.glb` or `.stl` file, fetched when the
//! person presses View in 3D, read on a worker, drawn with the app's own GPU, turned and zoomed
//! with the mouse. Used by the Files page's middle pane (files/viewer.rs) and the chat's 3D window
//! ([`draw_window`]); both fit it into their room with [`Viewer::show_fit`], the line under the view
//! and the Close button laid out first (BUG-251). The website mirrors it in web/shared/model-view.js.

pub mod camera;
pub mod gpu;
pub mod load;

use crate::gui::theme::Theme;
use camera::Orbit;
use egui::{RichText, Ui};
use humanity_core::i18n::{t, tf};
use load::Model;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// The button beside a `.glb` or `.stl` file.
pub const VIEW_IN_3D: &str = "View in 3D";
/// The button that closes the view.
pub const CLOSE: &str = "Close the 3D view";
/// Shown under the view where the app's graphics cannot draw it (and covered by the picture where
/// they can).
pub const NO_GRAPHICS: &str = "The 3D view needs the app's graphics; Download the file to see it.";

/// View in 3D's tooltip, with the file's size.
pub fn view_tip(size: &str) -> String {
    format!("Look at it here and turn it with the mouse. It is fetched from the server first ({size}).")
}

/// Fetch a model's bytes from `url`, with this session's upload token in `header` for a
/// members-only item (never in the address), at most [`load::MAX_MODEL_BYTES`]. A big model on a
/// slow line takes as long as it needs while bytes keep coming: only a connection that will not
/// open, or a minute of silence, ends it.
pub fn fetch(url: &str, header: Option<(&str, &str)>) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .build();
    let mut req = agent.get(url);
    if let Some((name, value)) = header {
        req = req.set(name, value);
    } else if let Some(token) = crate::net::upload_auth::token_for(url) {
        // A members-only room's file answers its readers alone (BUG-282).
        req = req.set(crate::net::upload_auth::HEADER, &token);
    }
    let resp = req.call().map_err(|e| fetch_failed(e))?;
    let max = load::MAX_MODEL_BYTES;
    if let Some(declared) = resp.header("Content-Length").and_then(|v| v.trim().parse::<usize>().ok()) {
        if declared > max {
            return Err(load::too_large_words(declared as i64));
        }
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), max as u64 + 1), &mut bytes).map_err(|e| fetch_failed(e))?;
    if bytes.len() > max {
        return Err(load::too_large_words(bytes.len() as i64));
    }
    Ok(bytes)
}

/// The words for a fetch that did not work (both apps say the same).
pub fn fetch_failed(reason: impl std::fmt::Display) -> String {
    format!("This model could not be fetched: {reason}.")
}

/// Numbers for each open viewer and each model read, so the GPU side knows which is which.
static NEXT: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// What the loading worker reports.
enum Progress {
    Reading,
    Done(Result<Model, String>),
}

enum Stage {
    Fetching(Receiver<Progress>),
    Reading(Receiver<Progress>),
    Ready { model: Arc<Model>, key: u64 },
    Failed(String),
}

/// One open viewer: the file it shows, how far loading is, and the camera.
pub struct Viewer {
    id: u64,
    /// The file's own name, as shown.
    pub name: String,
    size: i64,
    stage: Stage,
    orbit: Option<Orbit>,
}

impl Viewer {
    /// Open a viewer for `name` (`size` bytes, as the page knows it), whose bytes `fetch` gets on a
    /// worker thread. Over the size limit, it opens with the words saying so and fetches nothing.
    pub fn open(name: &str, size: i64, fetch: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static) -> Viewer {
        let stage = if size > load::MAX_MODEL_BYTES as i64 {
            Stage::Failed(load::too_large_words(size))
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            let file = name.to_string();
            std::thread::Builder::new()
                .name("model_view_load".into())
                .spawn(move || {
                    let result = fetch().and_then(|bytes| {
                        let _ = tx.send(Progress::Reading);
                        load::load(&file, &bytes)
                    });
                    let _ = tx.send(Progress::Done(result));
                })
                .map(|_| Stage::Fetching(rx))
                .unwrap_or_else(|e| Stage::Failed(fetch_failed(e)))
        };
        Viewer { id: next_id(), name: name.to_string(), size, stage, orbit: None }
    }

    /// A viewer already holding `model` (the snapshot tests draw only a few frames).
    #[cfg(test)]
    pub fn with_model(name: &str, model: Model) -> Viewer {
        let orbit = Some(Orbit::fit(&model.bounds));
        Viewer { id: next_id(), name: name.to_string(), size: 0, stage: Stage::Ready { model: Arc::new(model), key: next_id() }, orbit }
    }

    /// Whether the model is in and ready to turn.
    pub fn ready(&self) -> bool {
        matches!(self.stage, Stage::Ready { .. })
    }

    /// Read what the worker reported.
    fn poll(&mut self) {
        loop {
            let rx = match &self.stage {
                Stage::Fetching(rx) | Stage::Reading(rx) => rx,
                _ => return,
            };
            match rx.try_recv() {
                Ok(Progress::Reading) => {
                    if let Stage::Fetching(rx) = std::mem::replace(&mut self.stage, Stage::Failed(String::new())) {
                        self.stage = Stage::Reading(rx);
                    }
                }
                Ok(Progress::Done(Ok(model))) => {
                    self.orbit = Some(Orbit::fit(&model.bounds));
                    self.stage = Stage::Ready { model: Arc::new(model), key: next_id() };
                    return;
                }
                Ok(Progress::Done(Err(words))) => {
                    self.stage = Stage::Failed(words);
                    return;
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.stage = Stage::Failed("The model could not be read.".to_string());
                    return;
                }
            }
        }
    }

    /// Draw the viewer `width` points wide, its view at [`preferred_canvas`] and the line under it
    /// (the words while it loads or if it cannot), with nothing after it.
    pub fn show(&mut self, ui: &mut Ui, theme: &Theme, width: f32) {
        self.show_fit(ui, theme, egui::vec2(width, f32::INFINITY), preferred_canvas(width), &mut |_| {});
    }

    /// The viewer and `foot` (its Close the 3D view) in `room`: `room.x` points wide, and together
    /// at most `room.y` tall. The line under the view and the foot are laid out FIRST and the view
    /// takes the height left, never more than `canvas_most` (BUG-251, operator 2026-11-04: "The
    /// close 3D view button at the bottom left of the file upload is pushed offscreen a bit." The
    /// view was sized from the width alone, the line and the button after it, so a short window, a
    /// long name or a large Font Size pushed the button past its panel's edge). While the model
    /// loads, or when it cannot be shown, the words, then the foot.
    pub fn show_fit(&mut self, ui: &mut Ui, theme: &Theme, room: egui::Vec2, canvas_most: f32, foot: &mut dyn FnMut(&mut Ui)) {
        self.poll();
        let size_words = crate::gui::pages::files::api::size_words(self.size);
        let (model, key) = match &self.stage {
            Stage::Fetching(_) => {
                ui.label(RichText::new(format!("Fetching {} ({size_words})...", self.name)).size(theme.font_size_body).color(theme.text_muted()));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                foot(ui);
                return;
            }
            Stage::Reading(_) => {
                ui.label(RichText::new("Reading the model...").size(theme.font_size_body).color(theme.text_muted()));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                foot(ui);
                return;
            }
            Stage::Failed(words) => {
                ui.label(RichText::new(words.as_str()).size(theme.font_size_body).color(theme.warning()));
                foot(ui);
                return;
            }
            Stage::Ready { model, key } => (model.clone(), *key),
        };
        let width = room.x.max(1.0);
        // The line under the view, laid out at the view's width, and the foot, measured where
        // nobody sees it: the view is given what they leave.
        let help = tf("model_view.help", &[("name", &self.name), ("triangles", &load::thousands(model.triangles))]);
        let help = ui.painter().layout(help, egui::FontId::proportional(theme.font_size_small), theme.text_muted(), width);
        let foot_height = measured_height(ui, width, foot);
        let gap = ui.spacing().item_spacing.y;
        let with_help = room.y - help.size().y - foot_height - 2.0 * gap;
        // A room too short for a view worth turning AND the line (a stacked pane on a short window)
        // keeps the view and Close: the line is how-to words, the view is the thing.
        let keep_help = with_help >= MIN_VIEW.min(canvas_most);
        let height = if keep_help { with_help } else { room.y - foot_height - gap }.min(canvas_most);
        if height >= 1.0 {
            self.canvas(ui, theme, &model, key, egui::vec2(width, height));
        }
        if keep_help {
            // The galley as measured, so the line takes exactly the room it was given.
            ui.add(egui::Label::new(help));
        }
        foot(ui);
    }

    /// The view itself, `size` points: the person's hands on it (drag turns, right-drag or
    /// Shift-drag moves, scroll zooms, double-click starts over), and the GPU's picture of it.
    fn canvas(&mut self, ui: &mut Ui, theme: &Theme, model: &Arc<Model>, key: u64, size: egui::Vec2) {
        let viewer = self.id;
        let Some(orbit) = self.orbit.as_mut() else { return };
        let height = size.y;
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        // The person's hands on it.
        let shift = ui.input(|i| i.modifiers.shift);
        let delta = resp.drag_delta();
        if resp.dragged_by(egui::PointerButton::Secondary) || (resp.dragged_by(egui::PointerButton::Primary) && shift) {
            orbit.pan(delta.x, delta.y, height);
        } else if resp.dragged_by(egui::PointerButton::Primary) {
            orbit.turn(delta.x, delta.y);
        }
        if resp.hovered() {
            // Taken from the page, so the scroll zooms the model rather than scrolling the window.
            let scroll = ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta).y);
            if scroll != 0.0 {
                orbit.zoom(scroll);
            }
        }
        if resp.double_clicked() {
            orbit.reset();
        }

        // The words show only where nothing draws over them.
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::same(theme.border_radius as u8), theme.bg_card());
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, NO_GRAPHICS, egui::FontId::proportional(theme.font_size_small), theme.text_muted());
        let ppp = ui.ctx().pixels_per_point();
        let px = [(rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32];
        let view = orbit.view();
        let paint = gpu::Paint {
            viewer,
            model: model.clone(),
            model_key: key,
            view,
            view_proj: orbit.projection(rect.width() / rect.height()) * view,
            eye: orbit.eye(),
            size: px,
            background: egui::Rgba::from(theme.bg_card()).to_array(),
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, paint));
    }
}

/// The shortest view worth turning, in points: below it [`Viewer::show_fit`] leaves out the line
/// under the view rather than squeeze the view to nothing.
pub const MIN_VIEW: f32 = 64.0;

/// The view's height where nothing limits it, for a viewer `width` points wide.
pub fn preferred_canvas(width: f32) -> f32 {
    (width * 0.62).clamp(160.0, 480.0)
}

/// How tall `add` lays out `width` points wide, laid out once in a part of `ui` nobody sees (it is
/// not drawn and answers nothing), so what comes before it can be given the rest.
fn measured_height(ui: &mut Ui, width: f32, add: &mut dyn FnMut(&mut Ui)) -> f32 {
    let at = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, 100_000.0));
    let mut probe = ui.new_child(egui::UiBuilder::new().max_rect(at).layout(*ui.layout()).invisible());
    add(&mut probe);
    probe.min_rect().height()
}

/// The id of the chat's 3D window.
pub const CHAT_WINDOW_ID: &str = "chat_model_view";

/// The window a model posted in chat opens in (View in 3D on its card), with Close the 3D view:
/// its body held to the room a dialog has on this screen (`widgets::dialog::body_room`) and the
/// viewer fitted into it, so the button always sits inside the window at every window size and
/// Font Size (BUG-251).
pub fn draw_window(ctx: &egui::Context, theme: &Theme, model_view: &mut Option<Viewer>) {
    let Some(viewer) = model_view.as_mut() else { return };
    let mut open = true;
    let mut close = false;
    let title = viewer.name.clone();
    let room = crate::gui::widgets::dialog::body_room(ctx, theme);
    crate::gui::widgets::dialog(ctx, theme, CHAT_WINDOW_ID, &title, &mut open, |ui| {
        let width = ui.available_width().clamp(320.0, 720.0).min(room.x);
        viewer.show_fit(ui, theme, egui::vec2(width, room.y), preferred_canvas(width), &mut |ui| {
            close |= crate::gui::widgets::Button::secondary(CLOSE).tooltip(t("model_view.close_hint")).show(ui, theme);
        });
    });
    if !open || close {
        *model_view = None;
    }
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;
