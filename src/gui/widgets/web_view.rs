//! The web view: draws a readable [`Page`] in egui with clickable links,
//! back/forward history, an editable URL row, and an "open in system
//! browser" escape hatch for pages the readable web cannot show.
//!
//! This widget owns the NAVIGATION state (history, the in-flight fetch, the
//! status line); the page it draws comes from `crate::web_reader`. Every
//! fetch runs on a background thread through `web_reader::spawn_fetch`; this
//! file only ever polls a channel, so a slow site can never freeze the app.
//!
//! LAYOUT APPROACH. Same as the markdown reader (`widgets/markdown.rs`): one
//! `horizontal_wrapped` per paragraph with zero item spacing, so a run of
//! styled inlines reflows like a single paragraph; links are clickable
//! labels in the accent color; headings scale with the theme's font-size
//! ladder; code sits in a card-colored frame; tables use `egui::Grid` with
//! the width shared evenly. Images go through the chat image cache, so they
//! download once and reuse the texture.
//!
//! TESTABILITY. `navigate` records the resolved URL in `queued` and the
//! fetch is only dispatched at the START of the next `show`, gated by
//! `fetch_enabled`. A headless test can therefore click a link and assert
//! the exact URL the view asked for without a socket being opened. The
//! rects of drawn links are recorded per frame for the same reason.

use egui::{Frame, Label, RichText, ScrollArea, Sense};
use std::sync::mpsc;

use crate::gui::theme::Theme;
use crate::gui::widgets::image_cache::{ImageCache, ImageStatus};
use crate::gui::widgets::{self, ButtonVariant};
use crate::web_reader::{self, Block, Inline, Page, ReadRules, WebError};

/// What the status line shows.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewStatus {
    /// No page and nothing happening.
    Idle,
    /// A fetch is in flight for this URL.
    Fetching(String),
    /// A page is shown (its URL is in the history).
    Ready,
    /// The last navigation failed; the page before it (if any) stays shown.
    Error(String),
}

/// What the caller learns from one `show`.
#[derive(Debug, Default)]
pub struct WebViewResponse {
    /// The "Sites" button was pressed: the caller should return to its
    /// bookmark list. The view keeps its history.
    pub wants_close: bool,
}

pub struct WebViewState {
    /// The editable URL field. Follows navigation; edited by the person.
    pub url_input: String,
    /// The page on screen, if any.
    pub page: Option<Page>,
    pub status: ViewStatus,
    /// Readability hints, from `data/web/readability.json`.
    pub rules: ReadRules,
    /// When false, `show` never dispatches a fetch (tests). Default true.
    pub fetch_enabled: bool,
    /// Draw the toolbar's "Sites" button (return to the Browser page's card
    /// list). The Browser page wants it; an in-world screen has no card
    /// list to return to and turns it off. Default true.
    pub show_sites_button: bool,
    /// Visited URLs; `pos` indexes the current one.
    history: Vec<String>,
    pos: usize,
    /// A navigation `navigate` accepted but `show` has not dispatched yet.
    queued: Option<String>,
    /// The background fetch, keyed by the URL it is for.
    inflight: Option<(String, mpsc::Receiver<Result<Page, WebError>>)>,
    /// True while the view has something to show (a page, a fetch or an
    /// error); the Browser page swaps to the view on this.
    open: bool,
    /// Scroll the page to the top on the next frame (new page arrived).
    scroll_top: bool,
    /// (href, rect) of every link drawn last frame. Tests click these.
    link_rects: Vec<(String, egui::Rect)>,
    /// History index of the page on screen (set when a page arrives), so a
    /// refused navigation can return to it without a re-fetch.
    shown_pos: Option<usize>,
    /// The embed gate's line for the current page (review pending, not
    /// listed, or why a navigation was refused), drawn under the status
    /// line. Set by `apply_embed_gate`.
    embed_note: Option<String>,
}

impl Default for WebViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebViewState {
    pub fn new() -> Self {
        Self {
            url_input: String::new(),
            page: None,
            status: ViewStatus::Idle,
            rules: ReadRules::default(),
            fetch_enabled: true,
            show_sites_button: true,
            history: Vec::new(),
            pos: 0,
            queued: None,
            inflight: None,
            open: false,
            scroll_top: false,
            link_rects: Vec::new(),
            shown_pos: None,
            embed_note: None,
        }
    }

    /// The embed placement gate (2026-09-25; docs/design/readable-web.md).
    /// Every host calls this right before `show`. A queued navigation to a
    /// site whose terms forbid being shown inside other software is
    /// REFUSED before it is fetched: it never leaves the machine, the view
    /// stays on the page it had, and the note says why. Otherwise the note
    /// is the review line for the page being shown or fetched.
    pub fn apply_embed_gate(&mut self, sites: &crate::web_reader::sites::WebSites) {
        use crate::web_reader::sites::EmbedVerdict;
        if let Some(q) = self.queued.clone() {
            let verdict = sites.embed_verdict(&q);
            if matches!(verdict, EmbedVerdict::Forbidden { .. }) {
                self.refuse_queued();
                self.embed_note = verdict.note();
                return;
            }
        }
        // A refusal note stays until the next navigation is queued.
        if self.queued.is_none() && self.embed_note.as_deref().map_or(false, |n| n.starts_with("Not shown inside")) {
            return;
        }
        // An allowed page whose licence asks for credit gets its credit line
        // (the condition most "allowed" decisions rest on); anything else
        // gets its review note, if it has one.
        self.embed_note = self.current_url().and_then(|u| {
            sites.embed_verdict(u).note().or_else(|| {
                sites.attribution_for(u).map(|a| format!("From {a}. Original page: {u}"))
            })
        });
    }

    /// Drop the queued navigation without dispatching it and put the view
    /// back on the page that was on screen. A `navigate` pushed the refused
    /// url onto the end of the history, so it is popped; a Back or Forward
    /// only moved the position, so the position is restored.
    fn refuse_queued(&mut self) {
        if self.queued.take().is_none() {
            return;
        }
        match self.shown_pos {
            Some(shown) if shown < self.history.len() => {
                if self.pos + 1 == self.history.len() && self.pos > shown {
                    self.history.pop();
                }
                self.pos = shown.min(self.history.len().saturating_sub(1));
                self.status = if self.page.is_some() { ViewStatus::Ready } else { ViewStatus::Idle };
            }
            _ => {
                // Nothing was ever shown: there is no page to return to.
                self.history.clear();
                self.pos = 0;
                self.status = ViewStatus::Idle;
            }
        }
        self.url_input = self.current_url().unwrap_or("").to_string();
    }

    /// The embed gate's line for the current page, if any.
    pub fn embed_note(&self) -> Option<&str> {
        self.embed_note.as_deref()
    }

    /// True while the view has something to show.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Return to the bookmark list. History is kept so Back still works
    /// when the view reopens.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// The URL of the page on screen (or being fetched).
    pub fn current_url(&self) -> Option<&str> {
        self.history.get(self.pos).map(String::as_str)
    }

    /// Go to `url`. The scheme gate runs here, so a `javascript:` link is
    /// refused with a status-line message and never reaches the network.
    /// Accepted URLs are pushed onto the history (dropping any forward
    /// entries) and fetched at the start of the next `show`.
    pub fn navigate(&mut self, url: &str) {
        self.open = true;
        match web_reader::check_url(url) {
            Ok(u) => {
                let u = u.to_string();
                // A fragment-only jump on the page already shown is a scroll,
                // which the reader does not do; re-fetching would only blink.
                if let Some(cur) = self.current_url() {
                    if same_document(cur, &u) && self.page.is_some() {
                        self.url_input = u;
                        return;
                    }
                }
                if !self.history.is_empty() {
                    self.history.truncate(self.pos + 1);
                }
                self.history.push(u.clone());
                self.pos = self.history.len() - 1;
                self.request(u);
            }
            Err(e) => self.status = ViewStatus::Error(e.to_string()),
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.pos > 0 && !self.history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.history.is_empty() && self.pos + 1 < self.history.len()
    }

    pub fn back(&mut self) {
        if self.can_go_back() {
            self.pos -= 1;
            let u = self.history[self.pos].clone();
            self.request(u);
        }
    }

    pub fn forward(&mut self) {
        if self.can_go_forward() {
            self.pos += 1;
            let u = self.history[self.pos].clone();
            self.request(u);
        }
    }

    /// Re-fetch the current page.
    pub fn reload(&mut self) {
        if let Some(u) = self.current_url().map(str::to_string) {
            self.request(u);
        }
    }

    fn request(&mut self, url: String) {
        self.url_input = url.clone();
        self.status = ViewStatus::Fetching(url.clone());
        self.queued = Some(url);
    }

    /// The navigation `navigate` accepted and `show` has not dispatched.
    /// Tests read this to prove a click asked for the right URL.
    pub fn queued_navigation(&self) -> Option<&str> {
        self.queued.as_deref()
    }

    /// (href, rect) of every link drawn on the last frame.
    pub fn link_rects(&self) -> &[(String, egui::Rect)] {
        &self.link_rects
    }

    /// True while a background fetch is running for this view. With
    /// `queued_navigation`, the complete picture of "did this view ask the
    /// network for anything": a view that never navigated has neither.
    pub fn fetch_in_flight(&self) -> bool {
        self.inflight.is_some()
    }

    /// How many pages the history holds. A wall screen that navigates to
    /// its page once must leave this at 1, however many frames it draws.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Dispatch a queued fetch and drain a finished one. Runs at the start of
    /// every `show`, so a result is on screen the frame after it arrives.
    fn pump(&mut self, ctx: &egui::Context) {
        if let Some(url) = self.queued.take() {
            if self.fetch_enabled {
                // A newer navigation supersedes an older in-flight one; its
                // result is dropped with the receiver.
                self.inflight = Some((url.clone(), web_reader::spawn_fetch(url, self.rules.clone())));
            } else {
                // Tests: keep it visible to `queued_navigation` instead.
                self.queued = Some(url);
            }
        }
        let finished = match &self.inflight {
            Some((_, rx)) => match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(WebError::Network("the fetch thread stopped without answering".to_string())))
                }
            },
            None => None,
        };
        if let Some(result) = finished {
            self.inflight = None;
            match result {
                Ok(page) => {
                    self.url_input = page.url.clone();
                    // The final URL after redirects is what history should
                    // hold, so Back/Forward and "open in system browser"
                    // agree with what is on screen.
                    if let Some(slot) = self.history.get_mut(self.pos) {
                        *slot = page.url.clone();
                    }
                    self.page = Some(page);
                    self.status = ViewStatus::Ready;
                    self.scroll_top = true;
                    self.shown_pos = Some(self.pos);
                }
                Err(e) => self.status = ViewStatus::Error(e.to_string()),
            }
        }
        if self.inflight.is_some() {
            // Poll again soon without waiting for mouse movement.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Draw the toolbar, status line and page. `disclosure`, when given, is
    /// shown above the page (the affiliate transparency line from
    /// `data/web/sites.json`).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        images: &mut ImageCache,
        disclosure: Option<&str>,
    ) -> WebViewResponse {
        self.pump(ui.ctx());
        let mut resp = WebViewResponse::default();
        self.link_rects.clear();

        // ── Toolbar: Sites | < | > | reload | [url] Go | Open in browser ──
        let mut go_to: Option<String> = None;
        ui.horizontal(|ui| {
            if self.show_sites_button && widgets::compact_button(ui, theme, "Sites", ButtonVariant::Secondary) {
                resp.wants_close = true;
            }
            let back = widgets::Button::secondary("Back")
                .size(widgets::ButtonSize::Small)
                .disabled(!self.can_go_back())
                .tooltip("Previous page")
                .show(ui, theme);
            if back {
                self.back();
            }
            let fwd = widgets::Button::secondary("Forward")
                .size(widgets::ButtonSize::Small)
                .disabled(!self.can_go_forward())
                .tooltip("Next page")
                .show(ui, theme);
            if fwd {
                self.forward();
            }
            if widgets::Button::secondary("Reload")
                .size(widgets::ButtonSize::Small)
                .disabled(self.current_url().is_none())
                .show(ui, theme)
            {
                self.reload();
            }
            // The URL field takes the room the buttons leave. Enter = Go.
            let reserve = 170.0;
            let field_w = (ui.available_width() - reserve).max(120.0);
            let field = ui.add_sized(
                egui::vec2(field_w, theme.input_height),
                egui::TextEdit::singleline(&mut self.url_input)
                    .hint_text("https://")
                    .font(egui::FontId::monospace(theme.font_size_small)),
            );
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if widgets::compact_button(ui, theme, "Go", ButtonVariant::Primary) || entered {
                go_to = Some(self.url_input.clone());
            }
            if widgets::Button::secondary("Open in browser")
                .size(widgets::ButtonSize::Small)
                .disabled(self.current_url().is_none())
                .tooltip("Open this page in your system browser (for pages that need JavaScript)")
                .show(ui, theme)
            {
                if let Some(u) = self.current_url() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(u));
                }
            }
        });
        if let Some(u) = go_to {
            self.navigate(&u);
        }

        // ── Status line ──
        let (status_text, status_color) = match &self.status {
            ViewStatus::Idle => ("Enter an address or pick a site.".to_string(), theme.text_muted()),
            ViewStatus::Fetching(u) => (format!("Fetching {u}"), theme.text_secondary()),
            ViewStatus::Ready => {
                let title = self.page.as_ref().map(|p| p.title.as_str()).unwrap_or("");
                (title.to_string(), theme.text_secondary())
            }
            ViewStatus::Error(e) => (e.clone(), theme.danger()),
        };
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(status_text).size(theme.font_size_small).color(status_color));
        if let Some(d) = disclosure {
            ui.label(RichText::new(d).size(theme.font_size_small).color(theme.warning()));
        }
        if let Some(n) = &self.embed_note {
            // A refusal reads as a warning; a review note is quieter.
            let color = if n.starts_with("Not shown inside") { theme.warning() } else { theme.text_muted() };
            ui.label(RichText::new(n).size(theme.font_size_small).color(color));
        }
        ui.add_space(theme.spacing_sm);

        // ── Page ──
        // The page is moved out for the duration of the draw so the
        // link-click handler can borrow `self` mutably afterwards.
        let page = self.page.take();
        let mut clicked: Option<String> = None;
        let mut rects: Vec<(String, egui::Rect)> = Vec::new();
        let mut scroll = ScrollArea::vertical().auto_shrink([false, false]).id_salt("web_view_page");
        if self.scroll_top {
            scroll = scroll.vertical_scroll_offset(0.0);
            self.scroll_top = false;
        }
        scroll.show(ui, |ui| {
            ui.set_width(ui.available_width());
            match &page {
                Some(p) => {
                    if let Some(n) = &p.notice {
                        widgets::alert(ui, theme, widgets::AlertKind::Info, n);
                    }
                    let mut painter = PageDraw { theme, images, clicked: &mut clicked, rects: &mut rects };
                    painter.blocks(ui, &p.blocks);
                    ui.add_space(theme.spacing_xl);
                }
                None => {
                    if matches!(self.status, ViewStatus::Idle) {
                        ui.label(
                            RichText::new(
                                "The readable web shows the text, links, images and tables of a page. \
                                 Pages that are only a JavaScript app will look empty here; use \
                                 \"Open in browser\" for those.",
                            )
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                    }
                }
            }
        });
        self.page = page;
        self.link_rects = rects;
        if let Some(href) = clicked {
            self.navigate(&href);
        }
        resp
    }
}

/// Same page ignoring the fragment: `a#x` and `a#y` are one document.
fn same_document(a: &str, b: &str) -> bool {
    let strip = |s: &str| s.split('#').next().unwrap_or(s).to_string();
    strip(a) == strip(b)
}

/// Draws blocks. Holds the per-frame outputs (clicked link, link rects).
struct PageDraw<'a> {
    theme: &'a Theme,
    images: &'a mut ImageCache,
    clicked: &'a mut Option<String>,
    rects: &'a mut Vec<(String, egui::Rect)>,
}

impl<'a> PageDraw<'a> {
    fn blocks(&mut self, ui: &mut egui::Ui, blocks: &[Block]) {
        for b in blocks {
            self.block(ui, b);
        }
    }

    fn block(&mut self, ui: &mut egui::Ui, b: &Block) {
        let theme = self.theme;
        match b {
            Block::Heading { level, inlines } => {
                ui.add_space(if *level <= 2 { theme.spacing_md } else { theme.spacing_sm });
                let (size, color) = match level {
                    1 => (theme.font_size_title, theme.text_primary()),
                    2 => (theme.font_size_heading, theme.text_primary()),
                    3 => (theme.font_size_body, theme.accent()),
                    _ => (theme.font_size_body, theme.text_primary()),
                };
                self.inlines(ui, inlines, size, color, true);
                ui.add_space(theme.spacing_xs);
            }
            Block::Paragraph(inlines) => {
                self.inlines(ui, inlines, theme.font_size_small, theme.text_secondary(), false);
                ui.add_space(theme.spacing_sm);
            }
            Block::List { ordered, items } => {
                for (n, item) in items.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        ui.add_space(theme.spacing_sm + theme.spacing_md * item.depth as f32);
                        // U+00B7 is the confirmed-rendering bullet the
                        // markdown reader uses; numbers stay plain digits.
                        let marker = if *ordered { format!("{}.", n + 1) } else { "\u{00b7}".to_string() };
                        ui.label(RichText::new(marker).size(theme.font_size_small).color(theme.accent()));
                        ui.scope(|ui| {
                            ui.set_max_width(ui.available_width());
                            self.inlines(ui, &item.inlines, theme.font_size_small, theme.text_secondary(), false);
                        });
                    });
                }
                ui.add_space(theme.spacing_sm);
            }
            Block::Image { src, alt } => {
                self.image(ui, src, alt);
                ui.add_space(theme.spacing_sm);
            }
            Block::Code(text) => {
                Frame::none()
                    .fill(theme.bg_card())
                    .rounding(egui::Rounding::same(theme.border_radius as u8))
                    .stroke(egui::Stroke::new(1.0, theme.border()))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .monospace()
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            )
                            .wrap(),
                        );
                    });
                ui.add_space(theme.spacing_sm);
            }
            Block::Table { rows } => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
                // Share the width evenly and let cells wrap inside their
                // share, exactly as the markdown reader lays out its tables.
                let avail = ui.available_width();
                let col_w = ((avail - theme.spacing_sm * (cols as f32 + 1.0)) / cols as f32).max(60.0);
                let id = ui.next_auto_id();
                egui::Grid::new(id).striped(true).num_columns(cols).show(ui, |ui| {
                    for row in rows {
                        for c in 0..cols {
                            ui.scope(|ui| {
                                ui.set_max_width(col_w);
                                if let Some(cell) = row.get(c) {
                                    let (size, color) = if cell.header {
                                        (theme.font_size_small, theme.text_primary())
                                    } else {
                                        (theme.font_size_small, theme.text_secondary())
                                    };
                                    self.inlines(ui, &cell.inlines, size, color, cell.header);
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(theme.spacing_sm);
            }
            Block::Quote(inner) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(theme.spacing_sm);
                    ui.label(RichText::new("\u{2502}").color(theme.accent()));
                    ui.scope(|ui| {
                        ui.set_max_width(ui.available_width());
                        self.blocks(ui, inner);
                    });
                });
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }

    /// One wrapped run of styled text. Links are clickable and record their
    /// rects; a `"\n"` inside an inline ends the line.
    fn inlines(&mut self, ui: &mut egui::Ui, inlines: &[Inline], size: f32, color: egui::Color32, strong: bool) {
        let theme = self.theme;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for inl in inlines {
                match inl {
                    Inline::Link { href, text } => {
                        for (i, line) in text.split('\n').enumerate() {
                            if i > 0 {
                                ui.end_row();
                            }
                            if line.is_empty() {
                                continue;
                            }
                            let r = ui
                                .add(
                                    Label::new(RichText::new(line).size(size).underline().color(theme.accent()))
                                        .sense(Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(href);
                            self.rects.push((href.clone(), r.rect));
                            if r.clicked() {
                                *self.clicked = Some(href.clone());
                            }
                        }
                    }
                    other => {
                        let text = other.text();
                        for (i, line) in text.split('\n').enumerate() {
                            if i > 0 {
                                ui.end_row();
                            }
                            if line.is_empty() {
                                continue;
                            }
                            let mut rt = RichText::new(line).size(size).color(color);
                            rt = match other {
                                Inline::Strong(_) => rt.strong(),
                                Inline::Em(_) => rt.italics(),
                                Inline::Code(_) => rt.monospace().background_color(theme.bg_card()),
                                _ => rt,
                            };
                            if strong {
                                rt = rt.strong();
                            }
                            ui.add(Label::new(rt).wrap());
                        }
                    }
                }
            }
        });
    }

    /// An image through the shared cache: requested the first time its place
    /// on the page is inside the visible part of the scroll area, drawn when
    /// ready, a placeholder while loading, the alt text on failure.
    ///
    /// The "inside the visible part" rule is a privacy promise, not a
    /// performance trick: the Settings hint says only the page address and
    /// the images that scroll into view leave the machine. A page with fifty
    /// pictures below the fold must send nothing for them until the reader
    /// scrolls there. The headless test
    /// `web_view_requests_an_image_only_when_it_scrolls_into_view` in
    /// `ui_snapshots.rs` holds this line.
    fn image(&mut self, ui: &mut egui::Ui, src: &str, alt: &str) {
        let theme = self.theme;
        match self.images.status(src) {
            ImageStatus::Ready { .. } => {
                if let Some(tex) = self.images.get_texture(src) {
                    let [w, h] = tex.size();
                    let (w, h) = (w as f32, h as f32);
                    // Fit the reading column; never upscale a small image.
                    let scale = (ui.available_width() / w.max(1.0)).min(1.0);
                    ui.image((tex.id(), egui::vec2(w * scale, h * scale)));
                }
                if !alt.is_empty() {
                    ui.label(RichText::new(alt).size(theme.font_size_small).italics().color(theme.text_muted()));
                }
            }
            status @ (ImageStatus::Fetching | ImageStatus::Idle) => {
                let what = if alt.is_empty() { "image".to_string() } else { alt.to_string() };
                // The placeholder is laid out first so its rect is known;
                // that rect against the scroll area's clip rect is the
                // "has it scrolled into view" test.
                let placeholder = ui.label(
                    RichText::new(format!("Loading {what}..."))
                        .size(theme.font_size_small)
                        .italics()
                        .color(theme.text_muted()),
                );
                let on_screen = ui.is_rect_visible(placeholder.rect);
                if on_screen && matches!(status, ImageStatus::Idle) {
                    // First time on screen: this is the one GET for this image.
                    self.images.request(src);
                }
                // Poll for the decoded texture only while a fetch is actually
                // in flight or was just dispatched; an off-screen idle image
                // needs no repaint loop (scrolling repaints on its own).
                if on_screen || matches!(status, ImageStatus::Fetching) {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                }
            }
            ImageStatus::Failed(_) => {
                let what = if alt.is_empty() { "an image that could not be loaded" } else { alt };
                ui.label(RichText::new(format!("[{what}]")).size(theme.font_size_small).italics().color(theme.text_muted()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_refuses_blocked_schemes_without_queuing() {
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("javascript:alert(1)");
        assert!(v.queued_navigation().is_none(), "a blocked scheme must never be queued");
        assert!(matches!(&v.status, ViewStatus::Error(e) if e.contains("javascript")), "{:?}", v.status);
        assert!(v.is_open(), "the view opens to show the refusal");
    }

    #[test]
    fn history_back_and_forward_walk_the_visited_urls() {
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("https://a.example/");
        v.navigate("https://b.example/");
        v.navigate("https://c.example/");
        assert_eq!(v.current_url(), Some("https://c.example/"));
        assert!(v.can_go_back() && !v.can_go_forward());
        v.back();
        assert_eq!(v.current_url(), Some("https://b.example/"));
        assert_eq!(v.queued_navigation(), Some("https://b.example/"), "going back re-fetches that page");
        v.back();
        assert_eq!(v.current_url(), Some("https://a.example/"));
        assert!(!v.can_go_back());
        v.forward();
        assert_eq!(v.current_url(), Some("https://b.example/"));
        // A new navigation from the middle drops the forward entries.
        v.navigate("https://d.example/");
        assert!(!v.can_go_forward());
        assert_eq!(v.history, vec!["https://a.example/", "https://b.example/", "https://d.example/"]);
    }

    #[test]
    fn a_fragment_jump_on_the_shown_page_does_not_refetch() {
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("https://a.example/doc");
        v.queued = None; // pretend the fetch was dispatched and finished
        v.page = Some(Page { url: "https://a.example/doc".into(), ..Default::default() });
        v.navigate("https://a.example/doc#section-2");
        assert!(v.queued_navigation().is_none(), "same document: no fetch");
        assert_eq!(v.history.len(), 1);
    }

    fn gate_site(id: &str, url: &str, status: &str) -> crate::web_reader::sites::WebSite {
        use crate::web_reader::sites::*;
        WebSite {
            id: id.into(),
            name: id.into(),
            url: url.into(),
            category: "c".into(),
            description: String::new(),
            icon: String::new(),
            embed: WebSiteEmbed {
                status: status.into(),
                basis: "its terms, section 4".into(),
                terms_url: None,
                reviewed_on: None,
                reviewed_by: None,
                attribution: None,
            },
            affiliate: WebSiteAffiliate { program: None, tag: None, disclosure: String::new() },
            notes: String::new(),
        }
    }

    fn gate_db() -> crate::web_reader::sites::WebSites {
        crate::web_reader::sites::WebSites {
            categories: Vec::new(),
            sites: vec![
                gate_site("nope", "https://nope.example/", "forbidden"),
                gate_site("pending", "https://pending.example/", "needs_review"),
            ],
            own_domains: vec!["https://ours.example".into()],
        }
    }

    /// THE PLACEMENT GATE (2026-09-25). A link to a site whose terms forbid
    /// being shown inside other software is refused BEFORE it is fetched:
    /// the queued navigation is dropped, the view stays on the page it had
    /// (no re-fetch of that either), the refused url is not left in the
    /// history, and the note says why. Proven able to fail by skipping the
    /// refusal in apply_embed_gate: the queued navigation survives to the
    /// pump and the first assertion after the click fires.
    #[test]
    fn embed_gate_refuses_a_forbidden_link_before_it_is_fetched() {
        let db = gate_db();
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("https://pending.example/");
        // As if the fetch had answered.
        v.queued = None;
        v.page = Some(Page { url: "https://pending.example/".into(), ..Default::default() });
        v.status = ViewStatus::Ready;
        v.shown_pos = Some(v.pos);
        v.apply_embed_gate(&db);
        assert!(v.embed_note().unwrap().starts_with("Review pending"), "{:?}", v.embed_note());

        v.navigate("https://nope.example/article"); // a link click
        v.apply_embed_gate(&db);
        assert!(v.queued_navigation().is_none(), "a refused navigation is never dispatched");
        assert_eq!(v.current_url(), Some("https://pending.example/"), "back on the page that was shown");
        assert_eq!(v.history_len(), 1, "the refused url is not left in the history");
        assert!(matches!(v.status, ViewStatus::Ready), "{:?}", v.status);
        assert!(v.embed_note().unwrap().contains("the site's rule, not a law"));
        v.apply_embed_gate(&db);
        assert!(v.embed_note().unwrap().starts_with("Not shown inside"), "the reason stays up until the next navigation");

        v.navigate("https://ours.example/page");
        v.apply_embed_gate(&db);
        assert_eq!(v.queued_navigation(), Some("https://ours.example/page"), "an allowed navigation goes ahead");
        assert_eq!(v.embed_note(), None, "our own site carries no note");
    }

    /// An allowed site whose licence asks for credit gets its credit line
    /// under the status line, with the page it came from.
    #[test]
    fn an_allowed_site_carries_its_credit_line() {
        let mut db = gate_db();
        let mut wiki = gate_site("wiki", "https://wiki.example/", "allowed");
        wiki.embed.attribution = Some("Wiki, under CC BY-SA 4.0".into());
        db.sites.push(wiki);
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("https://wiki.example/Page");
        v.apply_embed_gate(&db);
        assert_eq!(
            v.embed_note(),
            Some("From Wiki, under CC BY-SA 4.0. Original page: https://wiki.example/Page")
        );
    }

    /// A refused FIRST navigation (nothing on screen yet) leaves an empty,
    /// idle view rather than a history pointing at the refused site.
    #[test]
    fn embed_gate_refuses_a_first_navigation_to_an_empty_view() {
        let mut v = WebViewState::new();
        v.fetch_enabled = false;
        v.navigate("https://nope.example/");
        v.apply_embed_gate(&gate_db());
        assert!(v.queued_navigation().is_none());
        assert_eq!(v.history_len(), 0);
        assert_eq!(v.current_url(), None);
        assert!(matches!(v.status, ViewStatus::Idle));
        assert!(v.embed_note().is_some());
    }
}
