//! The server strip (Media round 5, docs/design/media-round-5-plan.md 3.11, lane 1E,
//! 2026-10-12): a bar fixed under the top menu on the four pages whose content belongs to one
//! server (Media, Watch, Chat and Server Settings) that says which server that is, and offers
//! Switch. The operator's ask, in his words: "add like a fixed label just below the top menu
//! that says what server we're looking at. That way even if I scroll down the huge blue section
//! I can still see that I'm on united-humanity.us".
//!
//! WHAT IT SAYS: "{server name}  {host}" (decision 7 of the plan), with a dot, green while the
//! connection to it is up, and Switch at the right. The website draws the same strip with the
//! same words (web/shared/server-strip.js, lane 1F): the name is the one the server gives at
//! `/api/server-info` (its `name`), and with no name it is the host alone. The desktop app asks
//! for that name once per server, only once it is signed in there, and keeps it beside the
//! launcher's (`GuiState::server_info_cache`); until it has it, the name this person saved the
//! server under stands in, unless that is just its host, which the strip already shows.
//!
//! SWITCH lists this person's saved servers (`GuiState::chat_servers`, decision 6: the website,
//! which keeps nothing across sites, lists the servers its server is linked with instead), each
//! with its dot. Picking one does what clicking that server's row in Chat does, through the one
//! switch they share (`GuiState::switch_to_server`, gui/connections.rs): this server is parked
//! with its socket open, that one is brought back as it was or dialled. With uploads on their
//! way to this server it asks first, since a switch stops them where they are (the Media page's
//! `sync_server`); its records stay, and the Media page offers each again with Carry on when
//! you come back. "Manage servers" opens Chat, where the servers are added, ordered and forgotten.
//!
//! WHERE IT IS DRAWN: `dispatch::draw_tool_page` calls [`draw`] for the pages [`shown_on`] names
//! and no other, before the page draws. lib.rs adds the top menu's panel before that call, so egui
//! stacks this panel under the menu, above the page's own panels, across the whole width, and it
//! never scrolls with the page. An in-world screen showing one of the four pages shows it too.
//!
//! Colours are theme tokens only (tests/theme_token_lint.rs); the dots are painted circles, not
//! glyphs (tests/icon_glyph_lint.rs). What the strip remembers between frames (Switch's list
//! open, the question, the name being asked for) lives in egui's memory for its context, so no
//! field was added to `GuiState` for it.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use egui::{Align, Align2, Id, Layout, RichText, Sense};

use crate::gui::connections::Switched;
use crate::gui::pages::chat::norm_server_url;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// The strip's panel id: what egui stacks under the top menu, and what the tests look for.
pub(crate) const STRIP_ID: &str = "server_strip";

/// The pages that show the strip: the ones whose content is one server's.
pub(crate) const PAGES: [GuiPage; 4] = [GuiPage::Media, GuiPage::Watch, GuiPage::Chat, GuiPage::ServerSettings];

/// Every word the strip puts on screen. The website's strip says the same where the two do the
/// same thing (web/shared/server-strip.js `STRIP_WORDS`): Switch, Connected, and the label.
pub(crate) const SWITCH: &str = "Switch";
pub(crate) const SWITCH_HINT: &str = "Switch this app to another of your servers";
pub(crate) const CONNECTED: &str = "Connected";
pub(crate) const NO_SERVER: &str = "No server chosen";
pub(crate) const LIST_HEAD: &str = "Your servers";
pub(crate) const NO_SAVED: &str = "No saved servers yet. Add one with the + in Chat's server list.";
pub(crate) const CURRENT: &str = "(current)";
pub(crate) const MANAGE: &str = "Manage servers";
pub(crate) const MANAGE_HINT: &str = "Add, order and forget servers in Chat's server list.";
pub(crate) const QUESTION_TITLE: &str = "Switch servers?";
pub(crate) const STAY: &str = "Stay here";

/// How wide Switch's list is, and each of its rows.
const LIST_WIDTH: f32 = 320.0;

/// Whether `page` shows the strip.
pub(crate) fn shown_on(page: GuiPage) -> bool {
    PAGES.contains(&page)
}

/// The host of a server's address: "united-humanity.us" for "https://united-humanity.us/", and
/// "127.0.0.1:9" for "http://127.0.0.1:9", a port included as the website's `location.host`
/// includes it. The address is returned whole when it names no scheme.
pub(crate) fn host_of(server: &str) -> String {
    let s = server.trim();
    let rest = s.split_once("://").map(|(_, rest)| rest).unwrap_or(s);
    rest.split('/').next().unwrap_or("").to_string()
}

/// The strip's words: "{server name}  {host}", or the host alone when there is no name (the
/// website's `stripLabel`, word for word).
pub(crate) fn strip_label(name: &str, host: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        host.to_string()
    } else {
        format!("{name}  {host}")
    }
}

/// Where the launcher keeps a saved server's `/api/server-info` (pages/showroom.rs keys it by the
/// saved server's id, `srv_<address>`), so either can use the other's answer.
fn info_key(server: &str) -> String {
    format!("srv_{server}")
}

/// The name this person saved `server` under, unless it is just the server's host (a server
/// saved by connecting to it is named after its host), which the strip already shows.
fn saved_name(state: &GuiState, server: &str) -> String {
    let host = host_of(server);
    state
        .chat_servers
        .iter()
        .find(|s| norm_server_url(&s.url) == server)
        .map(|s| s.name.trim().to_string())
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(&host))
        .unwrap_or_default()
}

/// The name the strip shows for `server` (a normalised address): the server's own, from its
/// `/api/server-info`, when the app has it; else the name this person saved it under, unless
/// that is just its host. Empty when there is neither: the strip shows the host alone.
pub(crate) fn server_name(state: &GuiState, server: &str) -> String {
    let own = state.server_info_cache.get(&info_key(server)).map(|i| i.name.trim().to_string()).unwrap_or_default();
    if !own.is_empty() {
        return own;
    }
    saved_name(state, server)
}

/// The uploads on their way to the Media page's server: each still sending (or checking, paused,
/// waiting or reconnecting), not being cancelled. An upload that is up and being published, or
/// that ended, is not stopped by a switch, so it is not counted.
pub(crate) fn uploads_on_their_way(state: &GuiState) -> usize {
    use crate::gui::pages::media::resumable::Phase;
    state
        .media
        .uploads
        .iter()
        .filter(|e| !e.cancelling && !matches!(e.run.phase, Phase::Done { .. } | Phase::Failed { .. } | Phase::Cancelled))
        .count()
}

/// What the strip asks before a switch while uploads are on their way to `host` (the plan's
/// sentence, which ends by naming the button the Media page offers them again with).
pub(crate) fn uploads_question(n: usize, host: &str) -> String {
    if n == 1 {
        format!("1 upload is on its way to {host}. Switching stops it where it is; it carries on when you come back and press Carry on.")
    } else {
        format!("{n} uploads are on their way to {host}. Switching stops them where they are; they carry on when you come back and press Carry on.")
    }
}

/// Whether the connection to the server the strip names is up.
fn connected(state: &GuiState) -> bool {
    state.ws_client.as_ref().is_some_and(|c| c.is_connected())
}

/// Whether a saved server's link is up: the active connection for the server the app is on, its
/// background link for any other.
fn link_up(state: &GuiState, server: &str, here: &str) -> bool {
    if server == here {
        connected(state)
    } else {
        state.connections.iter().any(|c| c.url == server && c.identified)
    }
}

/// A switch picked, waiting for the answer to the question: the server's address, the name the
/// question's button says, and the egui pass it was last on screen in.
#[derive(Clone)]
struct Asked {
    url: String,
    label: String,
    shown: u64,
}

/// The ask for a server's own name: every server asked in this run (so a server that does not
/// answer is not asked every frame), and the answer on its way.
#[derive(Clone, Default)]
struct NameAsk {
    asked: Vec<String>,
    pending: Option<(String, Arc<Mutex<Receiver<Result<crate::gui::ServerInfo, String>>>>)>,
}

fn open_id() -> Id {
    Id::new("server_strip_open")
}

fn asked_id() -> Id {
    Id::new("server_strip_asked")
}

fn name_ask_id() -> Id {
    Id::new("server_strip_name_ask")
}

/// Land the server's own name when its answer came, and ask for it once per server, only once
/// this app is signed in there (`ws_identified`), on a worker thread. The answer goes where the
/// launcher keeps them (`server_info_cache`). A server that did not answer keeps the name it was
/// saved under for the rest of the run.
fn learn_name(ctx: &egui::Context, state: &mut GuiState, server: &str) {
    let mut ask: NameAsk = ctx.data(|d| d.get_temp(name_ask_id())).unwrap_or_default();
    if let Some((asked_for, rx)) = ask.pending.clone() {
        let got = rx.lock().ok().map(|rx| rx.try_recv());
        match got {
            Some(Ok(Ok(info))) => {
                state.server_info_cache.insert(info_key(&asked_for), info);
                ask.pending = None;
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => {}
            // Refused, unreadable, or the worker gone: the saved name stands.
            _ => ask.pending = None,
        }
    }
    let known = state.server_info_cache.contains_key(&info_key(server));
    if ask.pending.is_none() && state.ws_identified && !server.is_empty() && !known && !ask.asked.iter().any(|s| s == server) {
        let api = format!("{}/api/server-info", crate::gui::widgets::image_cache::http_base(server));
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<crate::gui::ServerInfo, String> {
                // A time limit, so a server that never answers does not hold up the ask for the
                // next one for the rest of the run.
                let answer = ureq::get(&api).timeout(std::time::Duration::from_secs(15)).call().map_err(|e| e.to_string())?;
                let body = answer.into_string().map_err(|e| e.to_string())?;
                serde_json::from_str::<crate::gui::ServerInfo>(&body).map_err(|e| e.to_string())
            })();
            let _ = tx.send(result);
        });
        ask.asked.push(server.to_string());
        ask.pending = Some((server.to_string(), Arc::new(Mutex::new(rx))));
    }
    ctx.data_mut(|d| d.insert_temp(name_ask_id(), ask));
}

/// Draw the strip under the top menu, Switch's list while it is open, and the question while one
/// waits. Called by `dispatch::draw_tool_page` for the pages [`shown_on`] names.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let here = state.active_server();
    learn_name(ctx, state, &here);
    let host = host_of(&here);
    let label = if here.is_empty() { NO_SERVER.to_string() } else { strip_label(&server_name(state, &here), &host) };
    let name_len = if here.is_empty() { 0 } else { label.len().saturating_sub(host.len()) };
    let up = connected(state);
    let mut open: bool = ctx.data(|d| d.get_temp(open_id())).unwrap_or(false);

    let margin = egui::Margin::symmetric(theme.spacing_md as i8, theme.spacing_xs as i8);
    let mut switch_rect = egui::Rect::NOTHING;
    let mut switch_pressed = false;
    egui::TopBottomPanel::top(STRIP_ID)
        .frame(egui::Frame::NONE.fill(theme.bg_secondary()).inner_margin(margin))
        .resizable(false)
        .show(ctx, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let switch = ui.scope(|ui| {
                    widgets::Button::secondary(SWITCH).size(widgets::ButtonSize::Small).tooltip(SWITCH_HINT).show(ui, theme)
                });
                switch_pressed = switch.inner;
                switch_rect = switch.response.rect;
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // The dot: green while the connection is up; hovering it says how it is.
                    let (dot, dot_resp) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                    let colour = if up { theme.success() } else { theme.text_muted() };
                    ui.painter().circle_filled(dot.center(), 4.0, colour);
                    let status = if up { CONNECTED.to_string() } else { state.ws_status.clone() };
                    dot_resp.on_hover_text(status);
                    // "{server name}  {host}" as one line of text: the name brighter, the host muted.
                    let mut job = egui::text::LayoutJob::default();
                    let font = egui::FontId::proportional(theme.font_size_small);
                    let (name_part, host_part) = label.split_at(name_len);
                    job.append(name_part, 0.0, egui::TextFormat { font_id: font.clone(), color: theme.text_primary(), ..Default::default() });
                    job.append(host_part, 0.0, egui::TextFormat { font_id: font, color: theme.text_muted(), ..Default::default() });
                    ui.add(egui::Label::new(job).truncate());
                });
            });
        });
    if switch_pressed {
        open = !open;
    }

    if open {
        open = draw_list(ctx, theme, state, &here, switch_rect, switch_pressed);
    }
    ctx.data_mut(|d| d.insert_temp(open_id(), open));
    draw_question(ctx, theme, state);
}

/// Switch's list, under the button, right edges aligned: the saved servers, each with its dot,
/// the one the app is on marked "(current)"; then Manage servers. Returns whether it stays open:
/// it closes on a pick, on Manage servers, on Escape, and on a click anywhere else (never on the
/// click that opened it).
fn draw_list(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, here: &str, anchor: egui::Rect, just_opened: bool) -> bool {
    let mut keep_open = true;
    let mut picked: Option<(String, String)> = None;
    let locked = state.private_key_bytes.is_none();
    let servers: Vec<(String, String)> = state.chat_servers.iter().map(|s| (norm_server_url(&s.url), s.name.clone())).collect();
    let area = egui::Area::new(Id::new("server_strip_list"))
        .order(egui::Order::Foreground)
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(egui::pos2(anchor.right(), anchor.bottom() + 4.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(theme.bg_card()).show(ui, |ui| {
                ui.set_width(LIST_WIDTH);
                ui.label(RichText::new(LIST_HEAD).size(theme.font_size_small).color(theme.text_muted()));
                if locked {
                    ui.label(RichText::new(crate::gui::connections::UNLOCK_TO_SWITCH).size(theme.font_size_small).color(theme.warning()));
                }
                if servers.is_empty() {
                    ui.label(RichText::new(NO_SAVED).size(theme.font_size_small).color(theme.text_secondary()));
                }
                for (url, name) in &servers {
                    let current = url == here;
                    let host = host_of(url);
                    let shown = if name.trim().eq_ignore_ascii_case(&host) { "" } else { name.as_str() };
                    let row_label = strip_label(shown, &host);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, theme.row_height), Sense::hover());
                    // A stable id per server (docs/design/ui-system.md, interaction tests): a click
                    // pressed on one frame and released on the next lands on the same row.
                    let sense = if locked || current { Sense::hover() } else { Sense::click() };
                    let resp = ui.interact(rect, Id::new(("server_strip_pick", url.as_str())), sense);
                    if resp.hovered() && !locked && !current {
                        ui.painter().rect_filled(rect, theme.border_radius, theme.bg_tertiary());
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let dot = egui::pos2(rect.left() + 8.0, rect.center().y);
                    let colour = if link_up(state, url, here) { theme.success() } else { theme.text_muted() };
                    ui.painter().circle_filled(dot, 4.0, colour);
                    let text_colour = if locked { theme.text_muted() } else { theme.text_primary() };
                    let font = egui::FontId::proportional(theme.font_size_small);
                    // A long name is cut at the row's end, or before "(current)" on that row.
                    let room = if current { rect.with_max_x(rect.right() - 72.0) } else { rect };
                    let painter = ui.painter().with_clip_rect(room);
                    painter.text(egui::pos2(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, &row_label, font.clone(), text_colour);
                    if current {
                        ui.painter().text(egui::pos2(rect.right() - 6.0, rect.center().y), Align2::RIGHT_CENTER, CURRENT, font, theme.text_muted());
                    }
                    if resp.clicked() {
                        let label = if shown.is_empty() { host.clone() } else { shown.to_string() };
                        picked = Some((url.clone(), label));
                    }
                }
                ui.separator();
                if widgets::Button::ghost(MANAGE).size(widgets::ButtonSize::Small).tooltip(MANAGE_HINT).show(ui, theme) {
                    state.push_nav_to(GuiPage::Chat);
                    keep_open = false;
                }
            });
        });
    if let Some((url, label)) = picked {
        pick(ctx, state, &url, &label);
        keep_open = false;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        keep_open = false;
    }
    // A click outside the list and the button closes it; the opening click is on the button.
    if !just_opened && ctx.input(|i| i.pointer.any_click()) {
        let at = ctx.input(|i| i.pointer.interact_pos());
        if at.is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)) {
            keep_open = false;
        }
    }
    keep_open
}

/// A server picked in Switch's list: switched at once, or, with uploads on their way to this
/// server, the question first.
fn pick(ctx: &egui::Context, state: &mut GuiState, url: &str, label: &str) {
    if uploads_on_their_way(state) > 0 {
        let asked = Asked { url: url.to_string(), label: label.to_string(), shown: ctx.cumulative_pass_nr() };
        ctx.data_mut(|d| d.insert_temp(asked_id(), asked));
        return;
    }
    switch(state, url, label);
}

/// The switch itself: the one Chat's rows make (gui/connections.rs). With no identity unlocked
/// it changes nothing and the status line says so ([`Switched::Locked`]); the list shows that line
/// and offers no row to pick then.
fn switch(state: &mut GuiState, url: &str, label: &str) {
    let _: Switched = state.switch_to_server(url, label);
}

/// The question, while one waits: the uploads that a switch would stop, and the two answers.
fn draw_question(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut asked) = ctx.data(|d| d.get_temp::<Asked>(asked_id())) else { return };
    let pass = ctx.cumulative_pass_nr();
    if pass > asked.shown + 1 {
        // Not on screen since: the person went to a page without the strip, which is leaving the
        // question unanswered. It is dropped, so no switch happens later out of the blue.
        ctx.data_mut(|d| d.remove::<Asked>(asked_id()));
        return;
    }
    let n = uploads_on_their_way(state);
    if n == 0 {
        // They ended while the question was on screen: nothing stops now, so the switch picked
        // goes ahead.
        ctx.data_mut(|d| d.remove::<Asked>(asked_id()));
        switch(state, &asked.url, &asked.label);
        return;
    }
    asked.shown = pass;
    ctx.data_mut(|d| d.insert_temp(asked_id(), asked.clone()));
    let host = host_of(&state.media.server);
    let mut open = true;
    let mut answer: Option<bool> = None;
    let go = format!("Switch to {}", asked.label);
    widgets::dialog(ctx, theme, "server_strip_question", QUESTION_TITLE, &mut open, |ui| {
        ui.set_max_width(420.0);
        ui.label(RichText::new(uploads_question(n, &host)).size(theme.font_size_body).color(theme.text_primary()));
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            if widgets::Button::primary(&go).show(ui, theme) {
                answer = Some(true);
            }
            if widgets::Button::secondary(STAY).show(ui, theme) {
                answer = Some(false);
            }
        });
    });
    if !open && answer.is_none() {
        answer = Some(false); // the dialog's close is Stay here
    }
    if let Some(go) = answer {
        ctx.data_mut(|d| d.remove::<Asked>(asked_id()));
        if go {
            switch(state, &asked.url, &asked.label);
        }
    }
}

#[cfg(test)]
#[path = "server_strip_tests.rs"]
mod tests;
