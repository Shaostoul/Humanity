//! The web provider: a `web:<url>` screen shows the readable web view on a
//! wall (in-world screens, rung 6 integration; the core reader is
//! `src/web_reader/` and the widget is `src/gui/widgets/web_view.rs`, both
//! already used by the Browser page).
//!
//! What this file decides, and why:
//!
//! * The provider owns ONE `WebViewState` per screen: its own history, its
//!   own in-flight fetch, its own status line. Two wall screens showing two
//!   sites never share a page, and neither touches the Browser page's view.
//! * The screen navigates to its url ONCE, on the first frame drawn while
//!   in-app web reading is on. It is not re-fetched every frame (the view's
//!   own fetch runs on a background thread and the view polls it per frame,
//!   which is all the per-frame work there is). `navigated` is the guard,
//!   and `on_switch_navigates_once_not_every_frame` holds the line.
//! * When `AppConfig.readable_web` is OFF the screen draws a notice saying
//!   in-app web reading is off and where the switch is, and NEVER calls the
//!   view: no navigate, no `show`, so no fetch can be dispatched. That is
//!   the same promise the Browser page makes ("a person who never turns it
//!   on never has the app fetch a web page"), kept on the wall too.
//!   `off_switch_draws_a_notice_and_never_fetches` proves it through the
//!   view's own fetch state.
//! * The dev IPC learns the view's state through `status()` (url, title,
//!   fetching/ready/error), waits on `load_state()`, and clicks links by
//!   index through `link_rects()`; the click itself goes through the
//!   surface's normal event API, never a side path into the view.
//!
//! The frame is split like the surface itself: `tick` is GPU-free (it draws
//! through `ScreenCore::run_with`) so every test here runs without a
//! device; `frame` wraps `tick` in the surface's `run_and_render`.

use crate::gui::screen_surface::{LoadState, ScreenCore, ScreenProvider, ScreenSurface};
use crate::gui::theme::Theme;
use crate::gui::widgets::web_view::{ViewStatus, WebViewState};
use crate::gui::widgets::{self, AlertKind};
use crate::gui::GuiState;
use crate::web_reader::Block;

/// Where the switch lives, for the off notice. Matches the Browser page's
/// own button ("Turn on in-app reading (Settings > Privacy)") and the
/// Settings toggle's label, so the wall names the same control.
const SWITCH_LOCATION: &str = "Settings > Privacy";
const SWITCH_LABEL: &str = "Read websites inside HumanityOS";

pub struct WebProvider {
    /// The url from the data file (`web:<url>`), the screen's page.
    url: String,
    view: WebViewState,
    /// Set once the first navigation was issued (readable_web was on for a
    /// frame). The reason the wall fetches its page once, not per frame.
    navigated: bool,
    /// Whether the last frame drew the view (`Some(true)`), the off notice
    /// (`Some(false)`), or no frame has happened yet (`None`). Only the
    /// status report reads it.
    last_on: Option<bool>,
}

impl WebProvider {
    pub fn new(url: &str) -> Self {
        let mut view = WebViewState::new();
        // A wall has no card list to return to.
        view.show_sites_button = false;
        Self { url: url.to_string(), view, navigated: false, last_on: None }
    }

    /// The view, for tests that inject a page or read what was queued.
    pub fn view(&self) -> &WebViewState {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut WebViewState {
        &mut self.view
    }

    /// One frame, GPU-free: decide whether the switch is on, navigate on
    /// the first on-frame, and draw either the view or the off notice
    /// through the core. Returns the run's output for the GPU half.
    pub fn tick(&mut self, core: &mut ScreenCore, theme: &Theme, gui_state: &mut GuiState) -> egui::FullOutput {
        let on = gui_state.settings.readable_web;
        self.last_on = Some(on);
        if !on {
            // The view is not touched at all while the switch is off: no
            // `show`, so its queued navigation (if any) is never dispatched
            // and nothing leaves the machine.
            let url = self.url.clone();
            return core.run_with(gui_state, |ctx, _state| off_notice(ctx, theme, &url));
        }
        if !self.navigated {
            self.view.navigate(&self.url);
            self.navigated = true;
        }
        // The affiliate transparency line for the site the current page
        // belongs to, if the database carries a tag for it (none do yet).
        let disclosure: Option<String> = self
            .view
            .current_url()
            .and_then(|u| gui_state.web_sites.disclosure_for(u))
            .map(str::to_string);
        let view = &mut self.view;
        core.run_with(gui_state, |ctx, state| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(theme.bg_primary()).inner_margin(egui::Margin::same(theme.spacing_md as i8)))
                .show(ctx, |ui| {
                    // The swapped-in image cache is THIS screen's (see the
                    // texture-swap rule in screen_surface.rs), so page
                    // images decode into textures the screen's context owns.
                    view.show(ui, theme, &mut state.image_cache, disclosure.as_deref());
                });
            state.image_cache.poll(ctx);
        })
    }

    /// The title the status report carries: the page's first heading, else
    /// its `<title>` (the host when it had none), else the url itself.
    fn title(&self) -> String {
        if let Some(p) = &self.view.page {
            for b in &p.blocks {
                if let Block::Heading { inlines, .. } = b {
                    let t: String = inlines.iter().map(|i| i.text()).collect::<Vec<_>>().join("");
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
            if !p.title.trim().is_empty() {
                return p.title.clone();
            }
        }
        self.view.current_url().unwrap_or(&self.url).to_string()
    }

    /// One word (or "error: reason") for the status report.
    fn status_word(&self) -> String {
        match self.last_on {
            None => "unframed".to_string(),
            Some(false) => "off".to_string(),
            Some(true) => match &self.view.status {
                ViewStatus::Idle => "idle".to_string(),
                ViewStatus::Fetching(_) => "fetching".to_string(),
                ViewStatus::Ready => "ready".to_string(),
                ViewStatus::Error(e) => format!("error: {e}"),
            },
        }
    }
}

impl ScreenProvider for WebProvider {
    fn frame(
        &mut self,
        surface: &mut ScreenSurface,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
    ) {
        surface.run_and_render(device, queue, theme, gui_state, |core, theme, state| self.tick(core, theme, state));
    }

    fn kind(&self) -> &'static str {
        "web"
    }

    /// `{url, title, status}`: the url is the page on screen (or being
    /// fetched; the screen's own url before any navigation), the title is
    /// `title()`, the status one of "unframed", "off", "idle", "fetching",
    /// "ready" or "error: ...". The rig asserts on all three.
    fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "url": self.view.current_url().unwrap_or(&self.url),
            "title": self.title(),
            "status": self.status_word(),
        })
    }

    fn load_state(&self) -> LoadState {
        match self.last_on {
            // The off notice is static content; there is nothing to wait for
            // and the status field says "off" for the rig to read.
            None | Some(false) => LoadState::Static,
            Some(true) => match &self.view.status {
                ViewStatus::Fetching(_) => LoadState::Loading,
                ViewStatus::Ready => LoadState::Ready,
                ViewStatus::Error(e) => LoadState::Error(e.clone()),
                // Idle only before the first navigation; nothing pending.
                ViewStatus::Idle => LoadState::Static,
            },
        }
    }

    fn link_rects(&self) -> Vec<egui::Rect> {
        self.view.link_rects().iter().map(|(_, r)| *r).collect()
    }
}

/// The screen when in-app web reading is off: what it would show, and the
/// exact switch that turns it on. Plain text on the theme's panel, sized to
/// read from across the room.
fn off_notice(ctx: &egui::Context, theme: &Theme, url: &str) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_primary()).inner_margin(egui::Margin::same(theme.spacing_md as i8)))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                egui::RichText::new("In-app web reading is off")
                    .size(theme.font_size_title)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.add_space(theme.spacing_sm);
            ui.label(
                egui::RichText::new(format!("This screen shows {url} when it is on."))
                    .size(theme.font_size_body)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            widgets::alert(
                ui,
                theme,
                AlertKind::Info,
                &format!("Turn it on in {SWITCH_LOCATION}: \"{SWITCH_LABEL}\". Until then nothing is fetched."),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::screens::provider_for;
    use crate::gui::screen_surface::ScreenSource;
    use crate::gui::theme::load_theme;
    use crate::web_reader::{Inline, Page};

    const URL: &str = "https://united-humanity.us";

    fn core(theme: &Theme) -> ScreenCore {
        ScreenCore::new("web_screen", &format!("web:{URL}"), 1280, 720, theme)
    }

    /// The registry resolves the `web:` scheme to this provider and nothing
    /// else; the source string keeps the url whole.
    #[test]
    fn provider_for_resolves_the_web_scheme() {
        let src = ScreenSource::parse(&format!("web:{URL}/library#slug"));
        assert_eq!(src, ScreenSource::Web(format!("{URL}/library#slug")));
        let p = provider_for(&src).expect("the web scheme has a provider");
        assert_eq!(p.kind(), "web");
        assert_eq!(p.status()["url"], format!("{URL}/library#slug"), "before any frame the url is the screen's own");
        assert_eq!(p.status()["status"], "unframed");
        assert_eq!(p.load_state(), LoadState::Static);
        assert!(p.link_rects().is_empty());
    }

    /// THE OFF SWITCH. With `readable_web` off the provider draws the notice
    /// and the view is never asked for anything: no navigation queued, no
    /// fetch in flight, no history, not even opened. Fetching stays enabled
    /// on the view here on purpose: the proof is that nothing reached it,
    /// not that a test flag stopped it. Proven able to fail by making
    /// `tick` navigate regardless of the switch: `queued_navigation` is then
    /// `Some(URL)` and the first assertion fires.
    #[test]
    fn off_switch_draws_a_notice_and_never_fetches() {
        let theme = load_theme();
        let mut state = GuiState::default();
        assert!(!state.settings.readable_web, "a fresh GuiState has in-app web reading off");
        let mut core = core(&theme);
        let mut p = WebProvider::new(URL);
        assert!(p.view().fetch_enabled, "the view would fetch if asked; the test is that it is not asked");
        for _ in 0..3 {
            let out = p.tick(&mut core, &theme, &mut state);
            assert!(!out.shapes.is_empty(), "the off notice draws something");
        }
        assert!(p.view().queued_navigation().is_none(), "no navigation may be queued while the switch is off");
        assert!(!p.view().fetch_in_flight(), "no fetch may be in flight while the switch is off");
        assert_eq!(p.view().history_len(), 0);
        assert!(!p.view().is_open());
        assert_eq!(p.status()["status"], "off");
        assert_eq!(p.status()["url"], URL);
        assert_eq!(p.load_state(), LoadState::Static);
        // The notice names the switch so a person at the wall knows where
        // to go. Read back from the drawn shapes, the same way the rig's
        // `find` verb would.
        core.find_text(SWITCH_LABEL);
        p.tick(&mut core, &theme, &mut state);
        let found = core.take_found_text().flatten().expect("the notice names the switch");
        assert!(found.text.contains(SWITCH_LOCATION), "and where it lives: {:?}", found.text);
    }

    /// With the switch on, the screen navigates to its url exactly once:
    /// three frames later the history still holds one entry and the one
    /// queued navigation is the screen's url. (Fetching is disabled on the
    /// view so the test opens no socket; the queued slot then keeps the
    /// request visible, which is what the widget's own tests rely on.)
    /// Proven able to fail by dropping the `navigated` guard: history grows
    /// to 3 and the length assertion fires.
    #[test]
    fn on_switch_navigates_once_not_every_frame() {
        let theme = load_theme();
        let mut state = GuiState::default();
        state.settings.readable_web = true;
        let mut core = core(&theme);
        let mut p = WebProvider::new(URL);
        p.view_mut().fetch_enabled = false;
        for _ in 0..3 {
            p.tick(&mut core, &theme, &mut state);
        }
        assert_eq!(p.view().queued_navigation(), Some(format!("{URL}/").as_str()), "the screen asked for its own url");
        assert_eq!(p.view().history_len(), 1, "one navigation, not one per frame");
        assert!(p.view().is_open());
        assert_eq!(p.status()["status"], "fetching");
        assert_eq!(p.load_state(), LoadState::Loading);
        // Turning the switch off again stops the view being drawn at all:
        // a frame in the off state reports "off" and Static.
        state.settings.readable_web = false;
        p.tick(&mut core, &theme, &mut state);
        assert_eq!(p.status()["status"], "off");
        assert_eq!(p.load_state(), LoadState::Static);
    }

    /// A page injected as if the fetch had answered: status becomes ready,
    /// the title is the first heading (not the `<title>`), the link rects
    /// are exposed in drawing order, and A CLICK AT A LINK'S RECT THROUGH
    /// THE CORE'S UV EVENT API navigates the view to that link's href. This
    /// is the same path the dev IPC's `link` verb takes (rect -> uv ->
    /// `ScreenSurface::button`), so it is the headless twin of the rig's
    /// web check. Proven able to fail by clicking at v = 0.99 (below the
    /// page): `queued_navigation` stays at the screen's own url.
    #[test]
    fn a_click_at_a_link_rect_through_the_core_navigates_the_wall_view() {
        let theme = load_theme();
        let mut state = GuiState::default();
        state.settings.readable_web = true;
        let mut core = core(&theme);
        let mut p = WebProvider::new(URL);
        p.view_mut().fetch_enabled = false;
        // Frame 1 queues the screen's own navigation.
        p.tick(&mut core, &theme, &mut state);
        assert_eq!(p.view().queued_navigation(), Some(format!("{URL}/").as_str()));
        // The fetch "answers": a page with a heading and one link.
        let target = format!("{URL}/library");
        p.view_mut().page = Some(Page {
            url: format!("{URL}/"),
            title: "united-humanity.us".into(),
            blocks: vec![
                Block::Heading { level: 1, inlines: vec![Inline::Text("Welcome home".into())] },
                Block::Paragraph(vec![
                    Inline::Text("Read the ".into()),
                    Inline::Link { href: target.clone(), text: "Library".into() },
                    Inline::Text(" on the wall.".into()),
                ]),
            ],
            notice: None,
        });
        p.view_mut().status = ViewStatus::Ready;
        // Two frames so the scroll area and wrapped labels have rects.
        p.tick(&mut core, &theme, &mut state);
        p.tick(&mut core, &theme, &mut state);
        assert_eq!(p.status()["status"], "ready");
        assert_eq!(p.status()["title"], "Welcome home", "first heading, not the <title>");
        assert_eq!(p.load_state(), LoadState::Ready);
        let rects = p.link_rects();
        assert_eq!(rects.len(), 1, "one link on the page");
        let (w, h) = core.size();
        let uv = crate::engine::screens::link_uv(&rects, 0, (w, h)).expect("the link is on the surface");
        assert!(uv.0 > 0.0 && uv.0 < 1.0 && uv.1 > 0.0 && uv.1 < 1.0, "uv {uv:?}");

        // The click, exactly as the IPC issues it: move, press, release on
        // separate frames, all through the core.
        core.pointer_moved(uv);
        p.tick(&mut core, &theme, &mut state);
        core.button(uv, true);
        p.tick(&mut core, &theme, &mut state);
        core.button(uv, false);
        p.tick(&mut core, &theme, &mut state);
        assert_eq!(
            p.view().queued_navigation(),
            Some(target.as_str()),
            "the click at the link's rect must queue navigation to its href"
        );
        assert_eq!(p.view().history_len(), 2, "the link is a new history entry");
        assert_eq!(p.status()["status"], "fetching", "a new fetch is pending for the link");
        assert_eq!(p.status()["url"], target);
    }

    /// An error from the fetch is reported as `error: ...` and as
    /// `LoadState::Error`, so the rig's wait ends with the reason in hand.
    #[test]
    fn a_failed_fetch_reports_its_reason() {
        let theme = load_theme();
        let mut state = GuiState::default();
        state.settings.readable_web = true;
        let mut core = core(&theme);
        let mut p = WebProvider::new(URL);
        p.view_mut().fetch_enabled = false;
        p.tick(&mut core, &theme, &mut state);
        p.view_mut().status = ViewStatus::Error("Could not reach the site: no route".into());
        p.tick(&mut core, &theme, &mut state);
        assert_eq!(p.status()["status"], "error: Could not reach the site: no route");
        assert_eq!(p.load_state(), LoadState::Error("Could not reach the site: no route".into()));
        // No page: the title falls back to the url.
        assert_eq!(p.status()["title"], format!("{URL}/"));
    }
}
