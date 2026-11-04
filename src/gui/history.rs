//! HISTORY: Back and Forward on the desktop, the way a web browser has them (navigation rung 4,
//! 2026-11-04; docs/design/navigation.md section 10). The operator, 2026-11-02: "We need to make it
//! so the back button on the mouse can take the user back to where they were like the internet
//! browser."
//!
//! A STEP is anything that has, or gets, its own address on the website (section 10.1): a page,
//! a Settings section, a Profile or Dev section, a chat channel or conversation, a Files place or
//! item, a Library document, a how-to, a Maps view. The history is a list of steps and a position
//! (section 10.2): going somewhere drops the steps ahead and adds the new one; Back moves the
//! position back one, Forward forward one; at most [`MAX_STEPS`], the oldest going first.
//!
//! IN MEMORY ONLY, for the session: nothing here is ever written to disk (CLAUDE.md's no-caches
//! rule, and nothing about where a person went is stored). A chat step holds the conversation's
//! channel id (`dm:<key>` for a conversation), which is why it must never be saved, logged or sent:
//! the website keeps the same step in `history.state`, never in the address (section 10.1).
//!
//! THE ONE WAY BETWEEN PLACES is here: [`GuiState::go`] (a page) and [`GuiState::go_to`] (a place
//! inside one), with [`How`] saying how the step was made. `tests/page_change_lint.rs` refuses an
//! assignment to `active_page` anywhere else, so a new page change cannot skip the history. The
//! world's own entry and exit ([`GuiState::enter_world`], [`GuiState::leave_world`]) and the title
//! screen ([`GuiState::show_title`]) are here too, and they are never steps (section 11): Play
//! leaves the history where it is, and coming out of the world returns to the step you were on.
//!
//! A step made INSIDE a page (a channel clicked, a document opened from a link) is seen rather than
//! told: [`GuiState::note_place`], once a frame, compares what the page shows with the step the
//! history is on, and adds a step when they differ. The Chat page alone writes its channel in 86
//! places; reading the result once is the one way none of them is missed. Settings is the
//! exception: its open section follows the scroll, and scrolling is not a step, so a section is a
//! step only when it is chosen (its sidebar, a link to it), through `go_to`.
//!
//! THE KEYS AND BUTTONS ([`frame`], called by lib.rs once a frame before the top menu): the
//! mouse's back and forward buttons (`PointerButton::Extra1` and `Extra2`: egui-winit's
//! `MouseButton::Back` and `Forward`, Windows' XBUTTON1 and XBUTTON2) and Alt+Left and Alt+Right
//! while no text box has the keyboard (egui's text box moves the cursor a word on Alt+arrows, so
//! there they keep that meaning); the top menu's two arrows (pages/escape_menu.rs). WITH A POPUP
//! OPEN, BACK CLOSES THE POPUP FIRST ([`close_popup`]), the way Android's back closes a dialog.
//! IN THE WORLD the side buttons do nothing: they are game buttons there, unbound until the
//! operator decides (his answer of 2026-11-03: "I'm not sure what to make them do yet").
//!
//! ESCAPE ([`GuiState::escape`], from lib.rs's key handler, section 10.4): the nearest thing open
//! closes first (lib.rs: the help panel, the menu's preview, then [`close_popup`]); then a page
//! opened FROM inside another goes back to that page; a page reached from the menu goes into the
//! world; in the world, the page you left comes back; on the title screen, nothing.

use crate::gui::pages::files::Place as FilesPlace;
use crate::gui::pages::maps::MapView;
use crate::gui::{GuiPage, GuiState, SettingsCategory};

/// The most steps the history keeps; the oldest goes first (section 10.2).
pub const MAX_STEPS: usize = 100;

/// How a step was made (section 10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// A menu click or a link to another page: Escape on it goes into the world.
    Beside,
    /// A page opened from inside another (the chat's cog to Server settings, Open in the Library,
    /// a notice to its page): Escape on it goes back to the page it was opened from.
    From,
    /// A place the app opens by itself (the first screen, a redirect, the Profile alias): it takes
    /// the place of the step the history is on instead of adding one.
    Replace,
}

/// Where inside a page a step is.
#[derive(Debug, Clone, PartialEq)]
pub enum At {
    /// The page itself (or a page with no places of its own).
    Page,
    /// A Settings section, chosen (its sidebar or a link to it).
    Settings(SettingsCategory),
    /// A Dev page section or a Profile (Real page) section, by id.
    Section(String),
    /// A chat channel or conversation on one server (`norm_server_url`). In memory only.
    Chat { server: String, channel: String },
    /// A Files place, and the item open in its panes.
    Files { place: FilesPlace, item: Option<i64> },
    /// A Library document by its slug, or `curriculum` or `dictionary`.
    Library(String),
    /// A how-to by its id.
    HowTo(String),
    /// A Maps view.
    Maps(MapView),
}

/// A place: a page, and where on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub page: GuiPage,
    pub at: At,
}

impl Place {
    /// A page itself.
    pub fn page(page: GuiPage) -> Self {
        Place { page, at: At::Page }
    }

    /// A Settings section.
    pub fn settings(section: SettingsCategory) -> Self {
        Place { page: GuiPage::Settings, at: At::Settings(section) }
    }
}

#[derive(Debug, Clone)]
struct Step {
    place: Place,
    /// Made with `How::From`: Escape backs out of it.
    from: bool,
}

/// The steps and the position (section 10.2), held in `GuiState::history`.
#[derive(Debug, Default)]
pub struct History {
    steps: Vec<Step>,
    pos: usize,
    /// The step the history is on was just made or returned to: the place the page settles on
    /// next frame is that step's (a page opened with its place set after the call, a document
    /// the Library picks on its first frame, a channel on another server), never a new step.
    settle: bool,
}

impl History {
    /// The step the history is on.
    pub fn current(&self) -> Option<&Place> {
        self.steps.get(self.pos).map(|s| &s.place)
    }

    /// The step Back would go to.
    pub fn previous(&self) -> Option<&Place> {
        self.pos.checked_sub(1).and_then(|p| self.steps.get(p)).map(|s| &s.place)
    }

    /// How many steps are kept, and which is the current one (from 0).
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    /// Every kept step's place, oldest first.
    pub fn places(&self) -> impl Iterator<Item = &Place> {
        self.steps.iter().map(|s| &s.place)
    }

    pub fn can_back(&self) -> bool {
        self.pos > 0 && !self.steps.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        self.pos + 1 < self.steps.len()
    }

    /// Add a step: the place you are already on adds nothing; `Replace` takes the current step's
    /// place; anything else drops the steps ahead and adds this one, the oldest going past
    /// [`MAX_STEPS`].
    fn record(&mut self, place: Place, how: How) {
        match self.steps.get_mut(self.pos) {
            Some(s) if how == How::Replace => s.place = place,
            Some(s) if s.place == place => {}
            _ => {
                self.steps.truncate(self.pos + 1);
                self.steps.push(Step { place, from: how == How::From });
                if self.steps.len() > MAX_STEPS {
                    self.steps.remove(0);
                }
                self.pos = self.steps.len() - 1;
            }
        }
    }

    /// The step Escape returns to (section 10.4, rule 2): the step before the visit to this page
    /// began, when that visit was opened FROM inside another page. A place moved to inside the
    /// page since (a document followed from a link) is part of the same visit.
    fn escape_target(&self) -> Option<usize> {
        let mut k = self.pos.min(self.steps.len().checked_sub(1)?);
        while k > 0 && !self.steps[k].from && self.steps[k - 1].place.page == self.steps[k].place.page {
            k -= 1;
        }
        (self.steps[k].from && k > 0).then(|| k - 1)
    }
}

impl GuiState {
    /// THE way to another page (section 10.2). `Profile` is the Real page's alias and lands on
    /// it; the world and the title screen are never steps, so `None` and `MainMenu` go through
    /// [`enter_world`](Self::enter_world) and [`show_title`](Self::show_title).
    pub fn go(&mut self, page: GuiPage, how: How) {
        let page = if page == GuiPage::Profile { GuiPage::Real } else { page };
        match page {
            GuiPage::None => return self.enter_world(),
            GuiPage::MainMenu => return self.show_title(),
            _ => {}
        }
        self.note_page_left();
        self.active_page = page;
        // The page's own place as it stands; a page with none seen keeps the place the history
        // is already on for it (a menu click on Settings while in its Audio section).
        let at = match self.observed_at(page) {
            Some(at) => at,
            None => match self.history.current() {
                Some(c) if c.page == page => c.at.clone(),
                _ => At::Page,
            },
        };
        self.history.record(Place { page, at }, how);
        self.history.settle = true;
    }

    /// Go to a place inside a page (a Settings section from its sidebar or a link to it).
    pub fn go_to(&mut self, place: Place, how: How) {
        self.note_page_left();
        self.apply(&place);
        self.history.record(place, how);
        self.history.settle = true;
    }

    /// Back one step. False when there is none.
    pub fn back(&mut self) -> bool {
        if !self.history.can_back() {
            return false;
        }
        self.history.pos -= 1;
        self.return_to_current();
        true
    }

    /// Forward one step. False when there is none.
    pub fn forward(&mut self) -> bool {
        if !self.history.can_forward() {
            return false;
        }
        self.history.pos += 1;
        self.return_to_current();
        true
    }

    /// Escape's second rule: out of a page opened from inside another, back to that page. False
    /// when the page was reached from the menu (or nothing is behind it).
    pub fn back_out(&mut self) -> bool {
        match self.history.escape_target() {
            Some(i) => {
                self.history.pos = i;
                self.return_to_current();
                true
            }
            None => false,
        }
    }

    /// Escape, after the nearest open thing has had its chance to close (section 10.4): back out
    /// of a page opened from another, else from a page into the world, from the world to the page
    /// left, and on the title screen nothing.
    pub fn escape(&mut self) {
        match self.active_page {
            GuiPage::MainMenu => {}
            GuiPage::None => self.leave_world(),
            _ => {
                if !self.back_out() {
                    self.enter_world();
                }
            }
        }
    }

    /// Into the 3D world. Never a step: the history stays on the page you left.
    pub fn enter_world(&mut self) {
        self.note_page_left();
        self.active_page = GuiPage::None;
    }

    /// The page being left is the step the history is on, so Back (and Escape in the world)
    /// can return to it. It always is once a frame has noted it; a page set by hand (a test's
    /// starting page) and left the same frame is noted here.
    fn note_page_left(&mut self) {
        let left = self.active_page;
        if matches!(left, GuiPage::None | GuiPage::MainMenu) || self.history.current().map(|c| c.page) == Some(left) {
            return;
        }
        let at = self.observed_at(left).unwrap_or(At::Page);
        self.history.record(Place { page: left, at }, How::Beside);
    }

    /// Out of the world, back to the step you were on (the Chat page when there is none yet). What
    /// the page shows now (a channel changed from the world's own chat) is that step's place.
    pub fn leave_world(&mut self) {
        let page = self.history.current().map(|p| p.page).unwrap_or(GuiPage::Chat);
        if self.history.is_empty() {
            self.go(page, How::Replace);
            return;
        }
        self.active_page = page;
        self.history.settle = true;
    }

    /// The title screen: the first screen, never a step.
    pub fn show_title(&mut self) {
        self.active_page = GuiPage::MainMenu;
    }

    /// Show the step the history is now on.
    fn return_to_current(&mut self) {
        if let Some(place) = self.history.current().cloned() {
            self.apply(&place);
        }
        self.history.settle = true;
    }

    /// Open a place: its page, and its place on that page.
    fn apply(&mut self, place: &Place) {
        self.active_page = place.page;
        match &place.at {
            At::Page => {}
            At::Settings(c) => {
                self.settings.category = *c;
                self.settings.scroll_to_section = Some(*c);
            }
            At::Section(id) if place.page == GuiPage::Dev => self.active_dev_section = id.clone(),
            At::Section(id) => self.active_real_section = id.clone(),
            At::Chat { server, channel } => self.return_to_channel(server, channel),
            At::Files { place: p, item } => {
                crate::gui::pages::files::sync_server(self);
                self.files.place = *p;
                match item {
                    Some(id) => {
                        if self.files.item.as_ref().map(|i| i.id) != Some(*id) {
                            crate::gui::pages::files::item_view::open_by_number(self, *id);
                        }
                    }
                    None => {
                        // An edit with changes is never dropped by a step (item_view.rs `unsaved`).
                        if !crate::gui::pages::files::item_view::unsaved(self) {
                            self.files.item = None;
                        }
                    }
                }
            }
            At::Library(id) => crate::gui::pages::library::show_place(self, id),
            At::HowTo(id) => self.how_to.open = Some(id.clone()),
            At::Maps(v) => self.maps.view = *v,
        }
        if place.at == At::Page {
            match place.page {
                // The how-to list is the how-to page with nothing open.
                GuiPage::HowTo => self.how_to.open = None,
                // Settings with no section chosen is its top, as `/settings` with no `#` is.
                GuiPage::Settings => self.settings.scroll_to_section = Some(SettingsCategory::Account),
                _ => {}
            }
        }
    }

    /// A chat step: its server (switched to, when it is a saved one and not the one open), then
    /// its channel, the way a click on the channel's row opens it (chat/left_panel.rs).
    fn return_to_channel(&mut self, server: &str, channel: &str) {
        let here = crate::gui::pages::chat::norm_server_url(&self.connected_server_url);
        if !server.is_empty() && server != here {
            let saved = self
                .chat_servers
                .iter()
                .find(|s| crate::gui::pages::chat::norm_server_url(&s.url) == server)
                .map(|s| (s.url.clone(), s.name.clone()));
            match saved {
                Some((url, name)) => {
                    let _ = self.switch_to_server(&url, &name);
                }
                // A server no longer saved: the Chat page shows what it shows (and the step
                // settles on it).
                None => return,
            }
        }
        if self.chat_active_channel == channel {
            return;
        }
        if let Some(key) = channel.strip_prefix("dm:") {
            let key = key.to_string();
            crate::gui::pages::chat::open_dm_conversation(self, &key);
            return;
        }
        self.chat_active_channel = channel.to_string();
        self.chat_reply_to = None;
        self.chat_messages.clear();
        self.history_fetched = false;
        if let Some(c) = self.chat_channels.iter_mut().find(|c| c.id == channel) {
            c.unread = false;
        }
    }

    /// The place a page shows now, for the pages whose places are seen (`None`: Settings, whose
    /// section follows the scroll, and the pages with no places of their own).
    fn observed_at(&self, page: GuiPage) -> Option<At> {
        Some(match page {
            GuiPage::Chat => At::Chat {
                server: crate::gui::pages::chat::norm_server_url(&self.connected_server_url),
                channel: self.chat_active_channel.clone(),
            },
            GuiPage::Files => At::Files { place: self.files.place, item: self.files.item.as_ref().map(|i| i.id) },
            GuiPage::Library => crate::gui::pages::library::shown_place(self).map_or(At::Page, At::Library),
            GuiPage::HowTo => self.how_to.open.clone().map_or(At::Page, At::HowTo),
            GuiPage::Maps => At::Maps(self.maps.view),
            GuiPage::Dev => At::Section(self.active_dev_section.clone()),
            GuiPage::Real => At::Section(self.active_real_section.clone()),
            _ => return None,
        })
    }

    /// Once a frame on a page: a place moved to inside the page (a channel clicked, a document
    /// followed) is a step; the place a page settles on just after a step was made or returned to
    /// is that step's.
    pub fn note_place(&mut self) {
        let page = self.active_page;
        if matches!(page, GuiPage::None | GuiPage::MainMenu) {
            return;
        }
        let settle = std::mem::take(&mut self.history.settle);
        let Some(current) = self.history.current() else {
            self.go(page, How::Replace);
            self.history.settle = false;
            return;
        };
        if current.page != page {
            // Only a test sets a page by hand now (the lint holds the rest).
            self.go(page, How::Beside);
            self.history.settle = false;
            return;
        }
        let Some(at) = self.observed_at(page) else { return };
        if at == current.at {
            return;
        }
        let place = Place { page, at };
        if settle {
            self.history.record(place, How::Replace);
        } else {
            self.history.record(place, How::Beside);
        }
    }
}

/// Where the dialog drawn last says so (pass number, its id), and where a close is asked for.
const POPUP_SHOWN: &str = "history_popup_shown";
const POPUP_CLOSE: &str = "history_popup_close";

/// Called by a closable dialog each pass it is drawn (widgets/dialog.rs): true when Back or
/// Escape asked it to close, which it then does.
pub fn popup_drawn(ctx: &egui::Context, id: egui::Id) -> bool {
    let close = ctx.data_mut(|d| {
        if d.get_temp::<egui::Id>(egui::Id::new(POPUP_CLOSE)) == Some(id) {
            d.remove::<egui::Id>(egui::Id::new(POPUP_CLOSE));
            true
        } else {
            false
        }
    });
    if !close {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(POPUP_SHOWN), (pass, id)));
    }
    close
}

/// Close the nearest popup: an open drop-down, else the dialog drawn last (in the last pass).
/// True when there was one, so the Back or Escape that asked does nothing more.
pub fn close_popup(ctx: &egui::Context) -> bool {
    if ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
        return true;
    }
    let now = ctx.cumulative_pass_nr();
    let shown = ctx.data(|d| d.get_temp::<(u64, egui::Id)>(egui::Id::new(POPUP_SHOWN)));
    match shown {
        Some((pass, id)) if pass + 1 >= now => {
            ctx.data_mut(|d| {
                d.remove::<(u64, egui::Id)>(egui::Id::new(POPUP_SHOWN));
                d.insert_temp(egui::Id::new(POPUP_CLOSE), id);
            });
            true
        }
        _ => false,
    }
}

/// Back as every way of asking for it does it: a popup open closes first.
pub fn ask_back(ctx: &egui::Context, state: &mut GuiState) {
    if !close_popup(ctx) {
        state.back();
    }
}

/// Once a frame, before the top menu (lib.rs): the step a page moved to is noted, and the mouse's
/// side buttons and Alt+arrows go Back and Forward. On the title screen and in the world nothing
/// happens here: the side buttons are the game's there, unbound (section 11).
pub fn frame(ctx: &egui::Context, state: &mut GuiState) {
    if matches!(state.active_page, GuiPage::None | GuiPage::MainMenu) {
        return;
    }
    state.note_place();
    let (mouse_back, mouse_forward) =
        ctx.input(|i| (i.pointer.button_pressed(egui::PointerButton::Extra1), i.pointer.button_pressed(egui::PointerButton::Extra2)));
    // A text box with the keyboard keeps Alt+arrows as its word jumps (and the keys stay its).
    let typing = ctx.wants_keyboard_input();
    let (alt_back, alt_forward) = if typing {
        (false, false)
    } else {
        ctx.input_mut(|i| (i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft), i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight)))
    };
    if mouse_back || alt_back {
        ask_back(ctx, state);
    } else if mouse_forward || alt_forward {
        state.forward();
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
