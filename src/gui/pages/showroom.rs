//! Character showroom panel (v0.441/442). One orbiting-avatar scene, three modes:
//!   0 = the Play picker: WHO (left column) + WHERE (right column) + ONE Enter
//!       bar along the bottom that states the pairing it commits.
//!   1 = appearance editor: the in-world wetroom mirror, and the picker's
//!       "Edit look" (whose Done returns to the picker, not the world).
//!   2 = wardrobe (bedroom): equip cosmetics per slot, "Done".
//! The panel only edits `gui_state` (appearance / outfit / backdrop / selection /
//! confirm); the main loop applies it to the avatar mesh, camera, backdrop, and
//! save (edit-buffer-then-sync). Structure spec: docs/design/play-characters.md.
//!
//! THE LAYOUT (2026-11-02, docs/design/top-menu-and-game-pages.md section 5; the operator: "The
//! left and right bar areas are small so a lot of text is crammed vertically. We need to even
//! things out."). The columns are SHARES of the window (`column_widths`): WHO a fifth, WHERE about
//! two fifths, the stage the rest, each between a narrowest and a widest. The character stands in
//! the middle of the stage, not of the window: lib.rs shifts the camera's lens by `stage_shift`.
//! The WHERE column keeps its three coloured parts and uses the width: the homes and the servers
//! are small cards side by side (`pick_cards`), the selected server's card shows the few lines a
//! person choosing needs with the rest behind "Server details", and the kinds of home are cards
//! side by side (where_you_live.rs). Every word comes from data/i18n/en.json (`characters.*`).
//!
//! CHOOSE WHERE YOU LIVE (ship homes increment 6, lane 2B, 2026-10-12;
//! docs/design/ship-homes-increment-6-plan.md 3.9). Two places in the picker choose a home:
//! - the new-home card ("A new home", `NEW_HOME`): selected while your own home has no save yet,
//!   it shows the choice (`where_you_live::draw_choice`, every start from data/homes/starts.ron),
//!   the Enter bar names the home it makes ("Enter as Wanderer -> a new Cabin (solo)"), and Enter
//!   remembers the choice (`persistence::live_save::choose_new_home`), which `save_slot::follow` makes into
//!   your own home's save once lib.rs records the pairing;
//! - the card of the server the game is connected to (`draw_home_here`): it asks the server where
//!   you live there (`where_you_live::ask_where_you_live`) and shows its answer
//!   (`where_you_live::draw_server`): your home there, or the choice with each start's free count
//!   and a Choose button (`where_you_live::choose_on_server`), whose answer engine/save_slot.rs puts
//!   into that server's save.

use std::cell::RefCell;
use std::time::Instant;

use egui::{Context, RichText, ScrollArea};
use humanity_core::i18n::{t, tf, tn};

use crate::gui::pages::where_you_live as wyl;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cards, Hint};
use crate::gui::{GuiState, LauncherWhere};

// Wardrobe slots come from data/inventory/equipment_slots.json (v0.942
// infinite-of-x: was a hardcoded array duplicating that registry). The
// inventory page and this wardrobe now share the one slot list via
// gui_state.equipment_slots (id, label), loaded at startup.

/// Sentinel id for the VIRTUAL "server you are connected to right now" row in
/// the picker's Open Net card (v0.775). The app auto-connects to `server_url`
/// (united-humanity.us by default) for chat, but that live connection was never
/// shown here -- only explicitly-saved `chat_servers` were -- so the operator
/// saw "No servers yet" while actually connected. This id is not a real saved
/// server; `resolve_server` special-cases it to read `server_url` directly.
/// It is also the row a server pairing made by the probe's `{"pairing":"server"}` records
/// (engine/save_slot.rs `enter_pairing_gui`), so the picker preselects that pairing's row: one
/// constant for both, the picker's (it was engine/save_slot.rs's `CONNECTED_SERVER_ROW` too, until
/// the crate split's rung 0, part B).
pub(crate) const CONNECTED_SERVER_ID: &str = "__connected__";

/// How long a server's card waits for its `game_home` answer before it stops saying it is asking,
/// seconds. A relay from before increment 6 never answers; its card then says so.
const ANSWER_WAIT_SECS: f32 = 10.0;

// ── The layout's measures (2026-11-02) ───────────────────────────────────────────────────────────

/// The WHO column's share of the window's width, and the narrowest and widest it is laid out at:
/// at the operator's 2000 points it is 400, wide enough that its hint reads in two lines, and at
/// his window's real 2560 it is 512 (the widest, 520, is reached only past 2600 points).
pub const WHO_SHARE: f32 = 0.20;
const WHO_MIN: f32 = 260.0;
const WHO_MAX: f32 = 520.0;
/// The WHERE column's: at 2000 points it is 880, room for three kinds of home side by side.
pub const WHERE_SHARE: f32 = 0.44;
const WHERE_MIN: f32 = 400.0;
const WHERE_MAX: f32 = 1280.0;
/// The most of the window the two columns take together: the stage keeps the rest, at least a
/// quarter, so on a very small window the columns give up their share before the character
/// disappears behind them.
const COLUMNS_AT_MOST: f32 = 0.75;
/// The narrowest a home's or a server's card is, before its row loses a column.
const PICK_CARD_MIN: f32 = 200.0;
/// The room kept at the WHO column's foot for the Name field and Edit look, under the list of
/// characters (which scrolls on its own when there are more than fit above it).
const WHO_FOOT: f32 = 110.0;

/// The WHO and WHERE columns' widths for a window `screen_w` points wide.
pub fn column_widths(screen_w: f32) -> (f32, f32) {
    let who = (screen_w * WHO_SHARE).clamp(WHO_MIN, WHO_MAX);
    let where_ = (screen_w * WHERE_SHARE).clamp(WHERE_MIN, WHERE_MAX);
    let room = (screen_w * COLUMNS_AT_MOST).max(1.0);
    if who + where_ > room {
        let k = room / (who + where_);
        (who * k, where_ * k)
    } else {
        (who, where_)
    }
}

/// The card of the connected server: what it asked, and what is on its way.
#[derive(Default)]
struct HereCard {
    /// The server the card asked where you live (its normalized address) since the picker opened.
    asked: Option<String>,
    /// When a request or a Choose went out, until its answer comes (or `ANSWER_WAIT_SECS` pass).
    waiting_since: Option<Instant>,
    /// The answer as it stood when the request or the Choose went out: one that differs is its
    /// reply.
    before: Option<humanity_ship::starts::HomeAnswer>,
    /// A Choose that could not be sent, in the sentence that says why.
    note: Option<String>,
}

// Thread-local, the way where_you_live.rs keeps its draft: the picker is drawn on the one main
// thread, and a test's state stays its own.
thread_local! {
    /// The name of your own home's save (saves/offline_home.json) when the picker last scanned
    /// the saves (`load_homes`), None when it has none: the new-home row offers the choice only
    /// then (`save_slot::follow` makes the new home only for a save that does not exist yet).
    static OWN_HOME: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The connected server's card (`draw_home_here`).
    static HERE: RefCell<HereCard> = RefCell::new(HereCard::default());
}

/// Where the picker's parts were drawn on its last frame: the screen, the WHO and WHERE columns
/// (the editor's two columns in modes 1 and 2), and how tall the WHERE column's contents ran
/// against the room it had (taller means it scrolls). The layout tests read it
/// (showroom_tests.rs), and lib.rs centres the character in the stage between the columns
/// (`stage_shift`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub screen: egui::Rect,
    pub who: egui::Rect,
    pub where_: egui::Rect,
    pub where_content_h: f32,
    pub where_view_h: f32,
}

impl Default for Drawn {
    fn default() -> Self {
        Self { screen: egui::Rect::NOTHING, who: egui::Rect::NOTHING, where_: egui::Rect::NOTHING, where_content_h: 0.0, where_view_h: 0.0 }
    }
}

thread_local! {
    /// The picker's last frame (`Drawn`).
    static DRAWN: std::cell::Cell<Drawn> = std::cell::Cell::new(Drawn::default());
}

/// Where the picker's parts were drawn on its last frame.
pub fn drawn() -> Drawn {
    DRAWN.with(|d| d.get())
}

/// How far the middle of the stage (between the two columns) is from the middle of the window, as
/// a share of half the window's width: the camera's lens shift that stands the character in the
/// middle of the stage (lib.rs, `Camera::lens_shift_x`). 0 before anything is drawn.
pub fn stage_shift() -> f32 {
    shift_for(&drawn())
}

/// [`stage_shift`] for a given frame.
pub fn shift_for(d: &Drawn) -> f32 {
    let w = d.screen.width();
    if !(w > 1.0) || !d.who.is_positive() || !d.where_.is_positive() {
        return 0.0;
    }
    let mid = (d.who.right() + d.where_.left()) * 0.5;
    ((mid - d.screen.center().x) / (w * 0.5)).clamp(-1.0, 1.0)
}

/// The name of your own home's save, as the picker last scanned it (None: it has none).
fn own_home() -> Option<String> {
    OWN_HOME.with(|o| o.borrow().clone())
}

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Land any finished server-info fetch into the cache (v0.478).
    drain_server_info(state);
    if state.showroom_mode == 0 {
        draw_picker(ctx, theme, state);
    } else {
        draw_editor(ctx, theme, state);
    }
    // (No CentralPanel: the 3D avatar renders in the center gap between the columns.)
}

// ─────────────────────────────────────────────────────────────────────────
// Mode 0: the Play picker (WHO / WHERE / one Enter)
// ─────────────────────────────────────────────────────────────────────────

/// The pairing the bottom bar states and the Enter button commits.
struct Pairing {
    /// Display name of the WHO half.
    who: String,
    /// Display name of the WHERE half.
    world: String,
    /// True for a server world (shared), false for a local home (solo).
    shared: bool,
    /// Why this pairing cannot be entered yet, stated in the open. None = go.
    blocked: Option<String>,
}

fn draw_picker(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Rescan the saves once per opening; the openers clear the flag.
    if !state.launcher_homes_loaded {
        load_homes(state);
    }

    let pair = pairing(state);
    let how = if pair.shared { t("characters.shared") } else { t("characters.solo") };
    let sentence = tf("characters.enter_as", &[("who", &pair.who), ("world", &pair.world), ("how", &how)]);

    // The Enter bar is shown FIRST on purpose: egui panels claim space in the
    // order they are added, so a bottom panel added after the two side columns
    // would only span the gap between them instead of the window.
    egui::TopBottomPanel::bottom("showroom_enter_bar").show(ctx, |ui| {
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&sentence)
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::Button::primary(t("characters.enter"))
                    .disabled(pair.blocked.is_some())
                    .hint("characters.enter_hint")
                    .show(ui, theme)
                {
                    remember_new_home(state);
                    state.showroom_confirm = true;
                }
            });
        });
        // A disabled Enter always says WHY, on screen, not in a tooltip nobody
        // hovers (play-characters.md: honest disabled states, never hidden).
        if let Some(reason) = &pair.blocked {
            ui.label(
                RichText::new(reason)
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
        }
        ui.add_space(theme.spacing_sm);
    });

    let (who_w, where_w) = column_widths(ctx.screen_rect().width());

    // ── WHO: the characters you can be ──
    let who_rect = egui::SidePanel::left("showroom_select")
        .resizable(false)
        .exact_width(who_w)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            // A visible Back returns to the menu without entering the world --
            // the nav bar is hidden here, so Esc alone is not discoverable.
            // (v0.476.1)
            if widgets::Button::secondary(t("characters.back"))
                .hint("characters.back_hint")
                .show(ui, theme)
            {
                state.showroom_cancel = true;
            }
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(t("characters.who"))
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.text_primary()),
            );
            draw_who_column(ui, theme, state);
        })
        .response
        .rect;

    // ── WHERE: the worlds that character can enter ──
    let where_side = egui::SidePanel::right("showroom_details")
        .resizable(false)
        .exact_width(where_w)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(t("characters.where"))
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.text_primary()),
            );
            hint(ui, theme, t("characters.where_line"));
            ui.add_space(theme.spacing_sm);
            draw_where_column(ui, theme, state)
        });
    let (where_content_h, where_view_h) = where_side.inner;
    DRAWN.with(|d| {
        d.set(Drawn { screen: ctx.screen_rect(), who: who_rect, where_: where_side.response.rect, where_content_h, where_view_h })
    });
}

/// Rescan the saves directory into the picker's (character, home) rows.
///
/// This reads the files itself instead of calling `persistence::list_saves`,
/// which returns only (world name, timestamp): the WHO column needs the
/// character name and the home summary needs the design, and both already sit
/// in the very file list_saves parses and throws away. Rows are deduped by
/// world name (the saves dir holds `offline_home.json` AND `auto_save.json`,
/// which carry the SAME `WorldSave.name`), newest kept, so a row name is a
/// usable id.
fn load_homes(state: &mut GuiState) {
    let dir = humanity_systems::persistence::saves_dir();
    // Your own home's save, if it has one (the new-home row offers the choice only while it has
    // none), and the connected server's card asks the server afresh each time the picker opens.
    let own = humanity_systems::persistence::slot_path(&dir, &humanity_systems::persistence::SaveSlot::Offline);
    let own = own.exists().then(|| humanity_systems::persistence::load_world(&own).map(|s| s.name).unwrap_or_default());
    OWN_HOME.with(|o| *o.borrow_mut() = own);
    HERE.with(|h| *h.borrow_mut() = HereCard::default());
    let mut homes: Vec<crate::gui::LauncherHome> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(save) = humanity_systems::persistence::load_world(&path) else {
                continue;
            };
            if save.name.trim().is_empty() {
                continue;
            }
            match homes.iter_mut().find(|h| h.world == save.name) {
                Some(existing) => {
                    if save.timestamp > existing.timestamp {
                        existing.character = save.character_name.clone();
                        existing.design = save.start.clone();
                        existing.timestamp = save.timestamp;
                    }
                }
                None => homes.push(crate::gui::LauncherHome {
                    world: save.name.clone(),
                    character: save.character_name.clone(),
                    design: save.start.clone(),
                    timestamp: save.timestamp,
                }),
            }
        }
    }
    // Most recently played first, so the fallback preselect lands on the home
    // you were last in.
    homes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    state.launcher_homes = homes;
    state.launcher_homes_loaded = true;
    preselect(state);
}

/// Preselect the picker: the last pairing when its rows still exist, else the
/// most recent home paired with itself, else the first-run "new character into
/// a new home" pair. A row that vanished from disk never survives the fallback.
fn preselect(state: &mut GuiState) {
    let newest = state.launcher_homes.first().map(|h| h.world.clone());
    let last_char = state.launcher_last_character.clone();
    let last_where = state.launcher_last_world.clone();
    // "" is the new-character row, which always exists; any other WHO must
    // still have its save on disk.
    let char_ok = last_char.is_empty() || state.launcher_homes.iter().any(|h| h.world == last_char);
    if char_ok {
        if let Some(id) = last_where.strip_prefix("server:") {
            if !id.is_empty() {
                state.launcher_who = last_char;
                state.launcher_where_kind = LauncherWhere::Server;
                state.launcher_selected_server = Some(id.to_string());
                state.launcher_selected_world = String::new();
                return;
            }
        }
        if let Some(world) = last_where.strip_prefix("home:") {
            // The fresh-homestead row only survives while you still have no
            // home: entering it creates one, and that save is the real WHERE.
            let row_ok = if world.is_empty() {
                newest.is_none()
            } else {
                state.launcher_homes.iter().any(|h| h.world == world)
            };
            if row_ok {
                state.launcher_who = last_char;
                state.launcher_where_kind = LauncherWhere::Home;
                state.launcher_selected_world = world.to_string();
                return;
            }
        }
    }
    let world = newest.unwrap_or_default();
    state.launcher_who = world.clone();
    state.launcher_where_kind = LauncherWhere::Home;
    state.launcher_selected_world = world;
}

/// WHO column: the hint, then one card per local character (its name, and the home it lives in)
/// and "+ New character", in a list that scrolls on its own when there are more than fit, then
/// the inline Name field and the door to the appearance editor at the column's foot. A character
/// IS its save file until the character/world split, so a card is keyed by the name of the home
/// it lives in (docs/design/character-and-save-custody.md).
fn draw_who_column(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let homes = state.launcher_homes.clone();
    let who = state.launcher_who.clone();
    let live_name = display_name(&state.character_name);

    // Deferred so the card closures never mutate state while it is snapshotted.
    let mut pick: Option<String> = None;
    let mut new_character = false;
    let mut edit_look = false;

    hint(ui, theme, t("characters.who_hint"));
    ui.add_space(theme.spacing_sm);

    // The characters: as many as there are (infinite of X), in a list of their own that scrolls,
    // so the Name field and Edit look stay in view however many there are.
    let list_h = (ui.available_height() - WHO_FOOT).max(theme.font_size_body * 4.0);
    let list = ScrollArea::vertical().id_salt("showroom_who_list").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing_sm;
        for home in &homes {
            let is_sel = who == home.world;
            // The selected card reads the LIVE name buffer, so an inline rename
            // shows up in the list as you type it.
            let name = if is_sel { live_name.clone() } else { display_name(&home.character) };
            let lives = tf("characters.lives_in", &[("home", &home.world)]);
            if pick_card(ui, theme, ("who", &home.world), is_sel, "characters.character_hint", |ui| {
                card_title(ui, theme, &name);
                hint(ui, theme, &lives);
            }) {
                pick = Some(home.world.clone());
            }
        }
        if pick_card(ui, theme, ("who", "+new"), who.is_empty(), "characters.new_character_hint", |ui| {
            card_title(ui, theme, t("characters.new_character"));
        }) {
            new_character = true;
        }
    });
    widgets::hint::name_scroll_area(ui, list.id, t("characters.who"), "characters.characters_hint");

    ui.add_space(theme.spacing_sm);
    ui.separator();
    // Renaming is identity housekeeping, so it lives here; colours and
    // height are a session of their own, behind Edit look.
    ui.label(
        RichText::new(t("characters.name"))
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
    );
    ui.add(egui::TextEdit::singleline(&mut state.character_name).desired_width(f32::INFINITY))
        .hint("characters.name_hint");
    ui.add_space(theme.spacing_xs);
    if widgets::Button::secondary(t("characters.edit_look"))
        .hint("characters.edit_look_hint")
        .show(ui, theme)
    {
        edit_look = true;
    }

    if let Some(world) = pick {
        state.launcher_who = world.clone();
        // Pairing validity, first increment: a character stays bound to its own
        // home until the file split, so picking WHO carries the home half of
        // WHERE with it. A selected server is left alone (any character may
        // visit an Open Net server).
        state.launcher_selected_world = world.clone();
        // Preview this character on the 3D stage (lib.rs applies the save).
        state.launcher_pending_load = Some(world);
    }
    if new_character {
        state.launcher_who.clear();
        state.launcher_selected_world.clear();
        state.character_name = t("characters.default_name").to_string();
        // A new character starts from the default body, not the last one you
        // previewed, so the stage shows what you are actually about to be.
        state.appearance = Default::default();
        state.outfit = Default::default();
        state.appearance_dirty = true;
        state.outfit_dirty = true;
    }
    if edit_look {
        state.showroom_mode = 1;
        state.showroom_return_to_picker = true;
    }
}

/// WHERE column: the three trust models as colour cards (v0.784, operator's RGB scheme), each
/// using the column's width (2026-11-02): its rows are small cards side by side, and the selected
/// one's details are drawn under them, so every home and server stays in view above whatever the
/// selected one opens. Returns how tall its contents ran and the room it had.
fn draw_where_column(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> (f32, f32) {
    let homes = state.launcher_homes.clone();
    let who_is_new = state.launcher_who.is_empty();
    let kind = state.launcher_where_kind;
    let sel_world = state.launcher_selected_world.clone();
    let sel_server = state.launcher_selected_server.clone();
    let servers = server_rows(state);
    // Each server's address without its scheme ("public.guide", "127.0.0.1:3210").
    let addresses: Vec<String> = servers
        .iter()
        .map(|(id, _, _)| {
            let url = resolve_server(state, id).map(|s| s.1).unwrap_or_default();
            url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string()
        })
        .collect();

    let mut pick_world: Option<String> = None;
    let mut pick_server: Option<String> = None;

    let scroll = ScrollArea::vertical().id_salt("showroom_where").show(ui, |ui| {
        // ── Your Homes: RED (offline, fully self-owned) ──
        section_card(ui, theme, t("characters.your_homes"), theme.danger(), |ui| {
            hint(ui, theme, t("characters.your_homes_hint"));
            ui.add_space(theme.spacing_xs);
            // The fresh-home card is offered when you own none, and whenever the new-character
            // card is picked: a new character needs a new home until characters and worlds are
            // separate files.
            let offer_new = homes.is_empty() || who_is_new;
            let count = homes.len() + usize::from(offer_new);
            let new_sel = kind == LauncherWhere::Home && sel_world.is_empty();
            pick_cards(ui, theme, "where_homes", count, |ui, i| match homes.get(i) {
                Some(home) => {
                    let is_sel = kind == LauncherWhere::Home && sel_world == home.world;
                    if pick_card(ui, theme, ("home", &home.world), is_sel, "characters.home_hint", |ui| {
                        card_title(ui, theme, &home.world);
                        // The kind of home by its start's own name ("Homestead"), the save's id when
                        // the starts file no longer lists it.
                        let kind_name = wyl::starts().get(&home.design).map(|s| s.name.clone()).unwrap_or_else(|| home.design.clone());
                        detail_row(ui, theme, t("characters.design"), &kind_name);
                        detail_row(ui, theme, t("characters.last_played"), &last_played(home.timestamp));
                    }) {
                        pick_world = Some(home.world.clone());
                    }
                }
                None => {
                    if pick_card(ui, theme, ("home", "+new"), new_sel, "characters.new_home_hint", |ui| {
                        card_title(ui, theme, t("characters.new_home"));
                    }) {
                        pick_world = Some(String::new());
                    }
                }
            });
            // Selected: where the new home is (increment 6, 3.9). The choice is offered while
            // your own home has no save; Enter makes it (`remember_new_home`).
            if offer_new && new_sel {
                ui.add_space(theme.spacing_sm);
                match own_home() {
                    None => {
                        wyl::draw_choice(ui, theme, wyl::starts());
                    }
                    Some(_) => hint(ui, theme, t("characters.new_home_taken")),
                }
            }
        });

        // ── Open Net: GREEN (discovery; bring your own character) ──
        ui.add_space(theme.spacing_sm);
        section_card(ui, theme, t("characters.open_net"), theme.success(), |ui| {
            hint(ui, theme, t("characters.open_net_hint"));
            ui.add_space(theme.spacing_xs);
            if servers.is_empty() {
                hint(ui, theme, t("characters.no_servers"));
            } else {
                pick_cards(ui, theme, "where_servers", servers.len(), |ui, i| {
                    let (id, name, connected) = &servers[i];
                    let is_sel = kind == LauncherWhere::Server && sel_server.as_deref() == Some(id.as_str());
                    if pick_card(ui, theme, ("server", id), is_sel, "characters.server_hint", |ui| {
                        card_title(ui, theme, name);
                        // A second line on every card, so a row of them reads level: Connected for
                        // the server the game talks to, else where it is.
                        if *connected {
                            cards::status_badge(ui, theme, t("characters.connected"), theme.success());
                        } else {
                            hint(ui, theme, &addresses[i]);
                        }
                    }) {
                        pick_server = Some(id.clone());
                    }
                });
                // The selected server's own card, under every server's, so the others stay in view.
                if kind == LauncherWhere::Server {
                    if let Some(id) = sel_server.as_deref().filter(|id| servers.iter().any(|s| s.0 == *id)) {
                        ui.add_space(theme.spacing_sm);
                        draw_server_info(ui, theme, state, id);
                    }
                }
            }
        });

        // ── Closed Net: BLUE (the committed, server-held story world) ──
        ui.add_space(theme.spacing_sm);
        section_card(ui, theme, t("characters.closed_net"), theme.info(), |ui| {
            hint(ui, theme, t("characters.closed_net_hint"));
        });
    });

    if let Some(world) = pick_world {
        state.launcher_where_kind = LauncherWhere::Home;
        state.launcher_selected_world = world;
    }
    if let Some(id) = pick_server {
        state.launcher_where_kind = LauncherWhere::Server;
        state.launcher_selected_server = Some(id);
    }
    (scroll.content_size.y, scroll.inner_rect.height())
}

/// The Open Net rows: the LIVE connection you are on right now (virtual,
/// v0.775) first, then your explicitly-saved bookmarks. The virtual row is what
/// the operator was missing -- the app auto-connects to server_url for chat, but
/// only saved chat_servers were listed, so a connected user saw "No servers
/// yet". Deduped against saved servers by url so it never doubles.
/// Returns (row id, display name, connected).
fn server_rows(state: &GuiState) -> Vec<(String, String, bool)> {
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let primary_url = state.server_url.trim_end_matches('/').to_string();
    let already_saved = state
        .chat_servers
        .iter()
        .any(|s| s.url.trim_end_matches('/') == primary_url);
    let mut rows: Vec<(String, String, bool)> = Vec::new();
    if ws_connected && !primary_url.is_empty() && !already_saved {
        rows.push((
            CONNECTED_SERVER_ID.to_string(),
            crate::gui::pages::chat::server_display_name(&state.server_url),
            true,
        ));
    }
    rows.extend(state.chat_servers.iter().map(|s| {
        // A saved bookmark of the server we're LIVE-connected to counts as
        // connected (v0.779): ChatServer.connected is never maintained by the
        // connection code, so without this URL match, bookmarking your own
        // server made it show "Not connected" and permanently disabled Enter
        // (the working virtual row is deduped away above).
        let live = ws_connected && s.url.trim_end_matches('/') == primary_url;
        (s.id.clone(), s.name.clone(), s.connected || live)
    }));
    rows
}

/// Resolve a WHERE server row to (display name, url, connected). The virtual
/// CONNECTED_SERVER_ID row reads `server_url` directly: it is the LIVE
/// connection, not a saved bookmark. None = the row is gone from your list.
fn resolve_server(state: &GuiState, id: &str) -> Option<(String, String, bool)> {
    let live = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if id == CONNECTED_SERVER_ID {
        return Some((
            crate::gui::pages::chat::server_display_name(&state.server_url),
            state.server_url.clone(),
            live,
        ));
    }
    let s = state.chat_servers.iter().find(|s| s.id == id)?;
    let same = is_live_url(state, &s.url);
    Some((s.name.clone(), s.url.clone(), s.connected || (live && same)))
}

/// True when `url` is the connection this client is actually live on. Entering
/// a server world means joining the shared world over THAT socket; switching
/// the live connection to a different saved server from here is the
/// multiplayer-future step.
fn is_live_url(state: &GuiState, url: &str) -> bool {
    !state.server_url.is_empty()
        && url.trim_end_matches('/') == state.server_url.trim_end_matches('/')
}

/// Compose the current selection into the pairing the Enter bar states. The
/// blocked reasons are the first-increment pairing rules from
/// docs/design/play-characters.md, section 3.
fn pairing(state: &GuiState) -> Pairing {
    let who = display_name(&state.character_name);
    match state.launcher_where_kind {
        LauncherWhere::Home => {
            let world = if state.launcher_selected_world.is_empty() {
                new_home_name()
            } else {
                state.launcher_selected_world.clone()
            };
            // A character and its home are ONE file today, so the only home a
            // character can enter is its own.
            let blocked = if state.launcher_selected_world == state.launcher_who {
                None
            } else if state.launcher_who.is_empty() {
                Some(t("characters.blocked_new").to_string())
            } else {
                Some(t("characters.blocked_move").to_string())
            };
            Pairing { who, world, shared: false, blocked }
        }
        LauncherWhere::Server => {
            let Some(id) = state.launcher_selected_server.as_deref() else {
                return Pairing {
                    who,
                    world: t("characters.a_server").to_string(),
                    shared: true,
                    blocked: Some(t("characters.blocked_pick_server").to_string()),
                };
            };
            let Some((name, url, connected)) = resolve_server(state, id) else {
                return Pairing {
                    who,
                    world: t("characters.a_server").to_string(),
                    shared: true,
                    blocked: Some(t("characters.gone").to_string()),
                };
            };
            let blocked = if connected && is_live_url(state, &url) {
                None
            } else {
                Some(t("where_you_live.connect_first").to_string())
            };
            Pairing { who, world: name, shared: true, blocked }
        }
        LauncherWhere::ClosedNet => Pairing {
            who,
            world: t("characters.a_closed_server").to_string(),
            shared: true,
            blocked: Some(t("characters.blocked_closed").to_string()),
        },
    }
}

/// The selected server's card, under the Open Net cards: the few lines a person choosing needs
/// (its name and whether it is connected, its description, how many people), the rest behind
/// "Server details" (its address, version, channels, the Accord, and for the connected server the
/// note for its admins), then where you live on it. The details come from its /api/server-info,
/// fetched once (v0.478). No Enter button: the bottom bar is the single place a pairing is
/// committed.
fn draw_server_info(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, id: &str) {
    let Some((svr_name, svr_url, svr_connected)) = resolve_server(state, id) else {
        hint(ui, theme, t("characters.gone"));
        return;
    };

    // Kick off a one-time fetch of this server's info if we don't have it.
    // The VIRTUAL row's cache entry is keyed by its URL (v0.779): the sentinel
    // id maps to "whatever server_url is NOW", so a plain sentinel key served
    // the PREVIOUS server's cached info after switching connections.
    let cache_id = if id == CONNECTED_SERVER_ID {
        format!("{CONNECTED_SERVER_ID}:{svr_url}")
    } else {
        id.to_string()
    };
    if !state.server_info_cache.contains_key(&cache_id) {
        fetch_server_info(state, &cache_id, &svr_url);
    }
    let info = state.server_info_cache.get(&cache_id).cloned();
    let live = is_live_url(state, &svr_url);

    egui::Frame::NONE
        .fill(theme.bg_card())
        .stroke(egui::Stroke::new(1.0_f32, theme.accent()))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Name: the fetched name if we have it, else the locally-known one; and whether this
            // game is connected to it, in words and colour.
            let title = info.as_ref().map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| svr_name.clone());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(title).size(theme.font_size_body).strong().color(theme.text_primary()));
                if svr_connected {
                    cards::status_badge(ui, theme, t("characters.connected"), theme.success());
                } else {
                    cards::status_badge(ui, theme, t("characters.not_connected"), theme.warning());
                }
            });
            match &info {
                None => hint(ui, theme, t("characters.loading")),
                Some(i) => {
                    if !i.description.trim().is_empty() {
                        cards::readable(ui, theme, |ui| {
                            ui.label(RichText::new(&i.description).size(theme.font_size_small).color(theme.text_secondary()));
                        });
                    }
                    // In-world co-presence count (v0.776): avatars actually in the shared world
                    // right now, distinct from chat "Online now".
                    let members = tn("characters.members", i.member_count as i64, &[]);
                    let people = tf("characters.people", &[("members", &members), ("online", &i.users_online), ("in_world", &i.game_players)]);
                    hint(ui, theme, &people);
                }
            }
            if cards::fold(ui, theme, ("showroom_server_details", &cache_id), t("characters.server_details"), "characters.server_details_hint") {
                ui.indent("showroom_server_details_rows", |ui| {
                    detail_row(ui, theme, t("characters.address"), &svr_url);
                    if let Some(i) = &info {
                        if !i.version.is_empty() {
                            detail_row(ui, theme, t("characters.version"), &i.version);
                        }
                        detail_row(ui, theme, t("characters.channels"), &i.channels.len().to_string());
                        detail_row(ui, theme, t("characters.accord"), if i.accord_compliant { t("characters.accord_yes") } else { t("characters.accord_no") });
                    }
                    // The description is EDITED in Server Settings (the admin's home for their
                    // server), not here. For the server you are connected to, point the way.
                    if live {
                        hint(ui, theme, t("characters.admin_note"));
                    }
                });
            }

            // Where you live on it (increment 6, 3.9): only the server the game talks to can be asked.
            if svr_connected && live {
                ui.add_space(theme.spacing_xs);
                ui.separator();
                draw_home_here(ui, theme, state);
            }
        });
}

/// The connected server's card, "where you live here" (ship homes increment 6, 3.9): the server
/// is asked where you live on it (`where_you_live::ask_where_you_live`) once each time the picker opens,
/// and its `game_home` answer (`GuiState::home_answer`) is shown by `where_you_live::draw_server`:
/// "Where you live here: a Cabin, c2.", or "You have no home on this server yet." with the choice,
/// each start's free count and a Choose button. Choose asks the server for that home
/// (`where_you_live::choose_on_server`, with the household picked on the card, which it reads from
/// the same draft `draw_server` returns); its answer goes into that server's save
/// (engine/save_slot.rs `on_game_message`).
///
/// An answer kept from before the card asked (another server's, or an old one) is not shown: the
/// card asks afresh and says it is asking until the reply comes or `ANSWER_WAIT_SECS` pass. While a
/// Choose waits, the old result is cleared from the answer shown, so any reply differs from it.
fn draw_home_here(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if wyl::server_slot(state).is_none() {
        ui.label(RichText::new(t("where_you_live.needs_identity")).size(theme.font_size_small).color(theme.warning()));
        hint(ui, theme, t("where_you_live.server_note"));
        return;
    }
    let server = state.active_server_key();
    let now = Instant::now();
    let asking = HERE.with(|h| {
        let mut h = h.borrow_mut();
        if h.asked.as_deref() != Some(server.as_str()) && wyl::ask_where_you_live(state) {
            log::info!("Characters: asked this server where you live on it");
            state.home_answer = None;
            *h = HereCard { asked: Some(server.clone()), waiting_since: Some(now), before: None, note: None };
        }
        if let Some(since) = h.waiting_since {
            if state.home_answer != h.before || now.duration_since(since).as_secs_f32() > ANSWER_WAIT_SECS {
                h.waiting_since = None;
                h.before = None;
            }
        }
        // Not signed in to it yet: the request goes out once it is.
        h.waiting_since.is_some() || h.asked.as_deref() != Some(server.as_str())
    });
    let picked = wyl::draw_server(ui, theme, wyl::starts(), state.home_answer.as_ref(), asking);
    if let Some(choice) = picked {
        match wyl::choose_on_server(state, &choice.start) {
            Ok(()) => {
                if let Some(a) = state.home_answer.as_mut() {
                    a.chose = false;
                    a.refused = None;
                    a.message.clear();
                }
                let before = state.home_answer.clone();
                HERE.with(|h| {
                    let mut h = h.borrow_mut();
                    h.waiting_since = Some(now);
                    h.before = before;
                    h.note = None;
                });
            }
            Err(why) => HERE.with(|h| h.borrow_mut().note = Some(why)),
        }
    }
    if let Some(note) = HERE.with(|h| h.borrow().note.clone()) {
        ui.label(RichText::new(note).size(theme.font_size_small).color(theme.warning()));
    }
}

/// The new-home row's Enter (increment 6, 3.9): when the row is the new home and your own home
/// has no save yet, the start and household chosen on it are remembered for your own home's save
/// (`persistence::live_save::choose_new_home`), which `save_slot::follow` makes once lib.rs records the pairing.
/// Returns the choice remembered.
fn remember_new_home(state: &GuiState) -> Option<wyl::Choice> {
    let new_home = state.launcher_where_kind == LauncherWhere::Home && state.launcher_selected_world.is_empty();
    if !new_home || own_home().is_some() {
        return None;
    }
    let choice = wyl::draft_choice(wyl::starts());
    humanity_systems::persistence::live_save::choose_new_home(&choice.start, &choice.household);
    log::info!("Characters: a new home, a {} ({})", choice.start, choice.household);
    Some(choice)
}

/// The Enter bar's name for the new-home row: your own home's save when it has one (a new
/// character moves into it), else "a new Cabin", the start chosen on the row.
fn new_home_name() -> String {
    match own_home() {
        Some(name) if !name.trim().is_empty() => name,
        Some(_) => t("characters.your_own_home").to_string(),
        None => {
            let starts = wyl::starts();
            let choice = wyl::draft_choice(starts);
            let name = starts.get(&choice.start).map(|s| s.name.clone()).unwrap_or(choice.start);
            tf("characters.a_new", &[("name", &name)])
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Modes 1/2: the focused editors (wetroom mirror, bedroom wardrobe)
// ─────────────────────────────────────────────────────────────────────────

fn draw_editor(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    let wardrobe = state.showroom_mode == 2;
    let right_title = if wardrobe { t("characters.wardrobe") } else { t("characters.appearance") };
    // "Edit look" from the picker borrows this same editor, so Done has to go
    // back where it came from instead of dropping you into the world.
    let from_picker = state.showroom_return_to_picker;
    // The editor's columns: the picker's WHO width on both sides (the editor's right column holds
    // a few controls, not the WHERE column's cards), so the stage keeps its place when Edit look
    // opens from the picker.
    let (who_w, _) = column_widths(ctx.screen_rect().width());

    let who_rect = egui::SidePanel::left("showroom_select")
        .resizable(false)
        .exact_width(who_w)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(t("characters.character"))
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);
            let name = display_name(&state.character_name);
            card_title(ui, theme, &name);
            ui.add_space(theme.spacing_xs);
            hint(
                ui,
                theme,
                if from_picker { t("characters.editing_from_picker") } else { t("characters.editing_in_world") },
            );
        })
        .response
        .rect;

    let right = egui::SidePanel::right("showroom_details")
        .resizable(false)
        .exact_width(who_w.max(310.0))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(right_title)
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.text_primary()),
            );
            hint(ui, theme, t("characters.orbit_line"));
            ui.add_space(theme.spacing_sm);

            // Scrolls (2026-11-09): with the body shape sliders the editor is taller than a small
            // window, and Done must stay reachable.
            ScrollArea::vertical().id_salt("showroom_editor_scroll").auto_shrink([false, false]).show(ui, |ui| {
                if wardrobe {
                    draw_wardrobe(ui, theme, state);
                } else {
                    // The GAME character's name (separate from your chat profile).
                    // The picker has its own inline Name field; this one serves the
                    // in-world mirror, where there is no WHO column.
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("characters.name")).color(theme.text_secondary()));
                        ui.text_edit_singleline(&mut state.character_name).hint("characters.name_in_mirror_hint");
                    });
                    ui.add_space(theme.spacing_sm);
                    draw_appearance(ui, theme, state);
                }

                ui.add_space(theme.spacing_sm);
                // Backdrop lives here, not in the picker: it is stage dressing for
                // editing, not part of the launch flow.
                draw_backdrop(ui, theme, state);
                ui.add_space(theme.spacing_md);

                if widgets::Button::primary(t("characters.done")).hint("characters.done_hint").show(ui, theme) {
                    if from_picker {
                        state.showroom_return_to_picker = false;
                        state.showroom_mode = 0;
                    } else {
                        state.showroom_confirm = true;
                    }
                }
                ui.add_space(theme.spacing_md);
            });
        })
        .response
        .rect;
    DRAWN.with(|d| d.set(Drawn { screen: ctx.screen_rect(), who: who_rect, where_: right, where_content_h: 0.0, where_view_h: 0.0 }));
}

// ─────────────────────────────────────────────────────────────────────────
// Shared pieces
// ─────────────────────────────────────────────────────────────────────────

/// Spawn a background blocking fetch of GET {url}/api/server-info. Stores the
/// result channel in state; drain_server_info lands it into the cache. No-op if
/// already cached or a fetch for this id is already in flight.
fn fetch_server_info(state: &mut GuiState, server_id: &str, url: &str) {
    if state.server_info_cache.contains_key(server_id) {
        return;
    }
    if state.server_info_loader.as_ref().map_or(false, |(id, _)| id == server_id) {
        return;
    }
    let api = format!("{}/api/server-info", url.trim_end_matches('/'));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<crate::gui::ServerInfo, String> {
            let resp = ureq::get(&api).call().map_err(|e| e.to_string())?;
            let body = resp.into_string().map_err(|e| e.to_string())?;
            serde_json::from_str::<crate::gui::ServerInfo>(&body).map_err(|e| e.to_string())
        })();
        let _ = tx.send(result);
    });
    state.server_info_loader = Some((server_id.to_string(), rx));
}

/// Land a finished server-info fetch into the cache. Called once per frame.
fn drain_server_info(state: &mut GuiState) {
    let mut done: Option<(String, Result<crate::gui::ServerInfo, String>)> = None;
    if let Some((id, rx)) = &state.server_info_loader {
        match rx.try_recv() {
            Ok(res) => done = Some((id.clone(), res)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = Some((id.clone(), Err("fetch worker exited".to_string())));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    if let Some((id, res)) = done {
        state.server_info_loader = None;
        if let Ok(info) = res {
            state.server_info_cache.insert(id, info);
        }
        // On error we just leave it uncached; a later reselect retries.
    }
}

/// A COLOR-CODED picker section card (v0.784, operator's RGB scheme):
/// offline homes = RED, open-net = GREEN, closed-net = BLUE. A tinted frame +
/// colored title so the three trust models read at a glance; the color carries
/// meaning (self-custody vs discovery vs server-held), matching the same
/// red/green/blue language planned across surfaces.
fn section_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    color: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(color.linear_multiply(0.06))
        .stroke(egui::Stroke::new(1.0_f32, color.linear_multiply(0.55)))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(theme.font_size_body).strong().color(color));
            add_contents(ui);
        });
}

/// `count` small cards side by side in rows that fill the width (as many to a row as fit at
/// `PICK_CARD_MIN`), each drawn by `cell` (which draws its own `pick_card`). The homes and the
/// servers: twenty servers wrap into rows rather than one tower.
fn pick_cards(ui: &mut egui::Ui, theme: &Theme, id: &str, count: usize, cell: impl FnMut(&mut egui::Ui, usize)) {
    cards::Grid::new(("showroom_pick", id)).plain().min_width(PICK_CARD_MIN).gap(theme.spacing_sm).show(ui, theme, count, cell);
}

/// One card to pick (a character, a home, a server): the card's frame the width there is, edged in
/// the accent when `selected` (`widgets::shelf::tile_frame`), what `add` draws inside it, and the
/// whole card one click target with a stable id, so a click anywhere on it picks it. Returns
/// whether it was clicked.
fn pick_card(ui: &mut egui::Ui, theme: &Theme, id_salt: impl std::hash::Hash, selected: bool, hint_key: &'static str, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let width = ui.available_width();
    let rect = ui.scope(|ui| widgets::shelf::tile_frame(ui, theme, width, selected, |ui, _| add(ui))).response.rect;
    // Interacted AFTER the card's words are drawn, so the card is on top of them and takes the
    // click wherever it lands (the words are not selectable).
    let resp = ui.interact(rect, ui.id().with(("pick_card", id_salt)), egui::Sense::click()).hint(hint_key);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// A card's name, strong.
fn card_title(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_body).strong().color(theme.text_primary())).selectable(false));
}

/// A muted contextual hint (the operator loves in-page help).
fn hint(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_small).color(theme.text_muted())).selectable(false));
}

/// A "Label: value" detail line for the inline row summaries.
fn detail_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(RichText::new(format!("{label}:")).size(theme.font_size_small).color(theme.text_secondary())).selectable(false));
        ui.add(egui::Label::new(RichText::new(value).size(theme.font_size_small).color(theme.text_primary())).selectable(false));
    });
}

/// A character's display name, with the same fallback the confirm handler uses
/// so the Enter sentence never disagrees with who you end up as.
fn display_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        t("characters.default_name").to_string()
    } else {
        trimmed.to_string()
    }
}

/// Coarse "last played" for a save timestamp (unix seconds). Coarse on purpose:
/// the row is a reminder of which home this is, not a log.
fn last_played(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if timestamp == 0 || now <= timestamp {
        return t("characters.just_now").to_string();
    }
    let secs = now - timestamp;
    if secs < 3600 {
        tf("characters.min_ago", &[("n", &(secs / 60).max(1))])
    } else if secs < 86_400 {
        tf("characters.hours_ago", &[("n", &(secs / 3600))])
    } else {
        tn("characters.days_ago", (secs / 86_400) as i64, &[])
    }
}

fn draw_appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("characters.appearance")).strong().color(theme.text_primary()));
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("characters.skin")).color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.skin_tone).hint("characters.skin_hint").changed() {
            state.appearance_dirty = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("characters.hair")).color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.hair_color).hint("characters.hair_hint").changed() {
            state.appearance_dirty = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("characters.eyes")).color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.eye_color).hint("characters.eyes_hint").changed() {
            state.appearance_dirty = true;
        }
    });
    // The body (2026-11-09): one of the bodies data/characters/characters.ron lists (a female and a
    // male MakeHuman body), published by engine::animation; everyone you meet sees the one chosen.
    if !state.characters.body_choices.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("characters.body")).color(theme.text_secondary())).hint("characters.body_hint");
            for id in state.characters.body_choices.clone() {
                let selected = state.appearance.body_type == id;
                let label = humanity_core::i18n::t_dyn(&format!("characters.body_{id}"));
                if ui.selectable_label(selected, label).hint_dyn(&format!("characters.body_{id}_hint")).clicked() && !selected {
                    state.appearance.body_type = id;
                    state.appearance_dirty = true;
                }
            }
        });
    }
    // The height in metres (character animation rung 2a, 2026-11-09): the chosen body's own height
    // with the look's height, build and ancestry, measured from the body (engine::animation
    // publishes it), in place of the bare multiplier the slider carries underneath.
    let metres = state.characters.my_height_m;
    let mut height_changed = false;
    widgets::settings_row(ui, theme, t("characters.height"), |ui| {
        height_changed = widgets::with_hint(ui, "characters.height_hint", |ui| widgets::custom_slider_capped(ui, theme, &mut state.appearance.height_scale, 0.8..=1.2, 64.0));
        if let Some(m) = metres {
            let words = humanity_core::i18n::tf("characters.height_metres", &[("m", &format!("{m:.2}"))]);
            ui.label(RichText::new(words).color(theme.text_muted()).size(theme.font_size_small)).hint("characters.height_metres_hint");
        }
    });
    if height_changed {
        state.appearance_dirty = true;
    }
    draw_body_shape(ui, theme, state);
    // Reached from the picker there is no bedroom to walk to, so say where
    // outfits actually live.
    if state.showroom_return_to_picker {
        hint(ui, theme, t("characters.outfits_where"));
    }
}

/// THE BODY'S SHAPE (2026-11-09; docs/design/character-animation.md section 14, the sliders): the chosen body's
/// sliders, as its data file lists them (`shape.sliders`, published by engine::animation into
/// `body_sliders`), each run under its group's heading. A slider from one end to the other (build,
/// proportions) shows its two ends in words and the body as made in the middle; a blend's sliders
/// (ancestry) are shares of one whole, so moving one moves the others and each shows its share. The
/// values go into `appearance.shape` by slider id (and to everyone you meet in your look); Reset
/// empties it and puts the height back to the body's own.
fn draw_body_shape(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    use humanity_core::i18n::{t_dyn, t_marked};
    let blocks = match state.characters.body_sliders.get(&state.appearance.body_type).or_else(|| state.characters.body_sliders.values().next()) {
        Some(b) if !b.is_empty() => b.clone(),
        _ => return,
    };
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("characters.shape")).strong().color(theme.text_primary()));
    widgets::glossary_words(ui, theme, t_marked("characters.shape_note"), theme.font_size_small, theme.text_muted());
    let mut group: Option<&str> = None;
    for block in &blocks {
        if group != Some(block.group.as_str()) {
            group = Some(block.group.as_str());
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(t_dyn(&format!("characters.shape_group_{}", block.group))).color(theme.text_secondary()));
            let note = format!("characters.shape_group_{}_note", block.group);
            if humanity_core::i18n::has(&note) {
                hint(ui, theme, t_dyn(&note));
            }
        }
        if block.shares {
            draw_shape_shares(ui, theme, state, &block.sliders);
        } else {
            for id in &block.sliders {
                draw_shape_range(ui, theme, state, id);
            }
        }
    }
    ui.add_space(theme.spacing_xs);
    if ui.button(t("characters.shape_reset")).hint("characters.shape_reset_hint").clicked() {
        state.appearance.shape.clear();
        state.appearance.height_scale = 1.0;
        state.appearance_dirty = true;
    }
}

/// One slider from its low end to its high end, the body as made in the middle: its name, then the
/// two ends in words either side of the track.
fn draw_shape_range(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, id: &str) {
    use humanity_core::i18n::t_dyn;
    let key = format!("characters.shape_{id}");
    let mut v = state.appearance.shape.get(id).copied().unwrap_or(humanity_core::player_look::SHAPE_MIDDLE);
    let mut changed = false;
    widgets::settings_row(ui, theme, t_dyn(&key), |ui| {
        changed = shape_track(ui, theme, Some(t_dyn(&format!("{key}_low"))), &mut v, &format!("{key}_hint"));
        ui.add(egui::Label::new(RichText::new(t_dyn(&format!("{key}_high"))).size(theme.font_size_small).color(theme.text_muted())).truncate());
    });
    if changed {
        state.appearance.shape.insert(id.to_string(), v.clamp(0.0, 1.0));
        state.appearance_dirty = true;
    }
}

/// A shape slider's track: a slot for its low end's words (right-aligned against the track; empty for
/// a share), then the track, leaving a slot as wide for the high end's words (or the share), all
/// shares of the row's width, so every track in the section starts and ends at the same place
/// whatever its words, inside the column. Returns whether the value changed.
fn shape_track(ui: &mut egui::Ui, theme: &Theme, low: Option<&str>, v: &mut f32, hint_key: &str) -> bool {
    let gap = ui.spacing().item_spacing.x;
    let avail = ui.available_width();
    let end = (avail * 0.27).min(theme.settings_label_width * 0.5);
    let slot = egui::vec2(end, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(slot, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_width(slot.x);
        if let Some(words) = low {
            ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(theme.text_muted())).truncate());
        }
    });
    ui.spacing_mut().slider_width = (avail - 2.0 * end - 3.0 * gap).max(40.0);
    widgets::slider(ui, theme, egui::Slider::new(v, 0.0..=1.0).show_value(false)).hint_dyn(hint_key).changed()
}

/// A blend's sliders: each part's share of one whole (a look without them is an even mix). Moving
/// one moves the others in proportion, so they always add up to 100%, as MakeHuman's own do.
fn draw_shape_shares(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, parts: &[String]) {
    use humanity_core::i18n::t_dyn;
    let raw: Vec<Option<f32>> = parts.iter().map(|p| state.appearance.shape.get(p).copied()).collect();
    let mut shares = humanity_core::player_look::blend_shares(&raw);
    let mut moved: Option<(usize, f32)> = None;
    for (k, id) in parts.iter().enumerate() {
        let key = format!("characters.shape_{id}");
        let mut v = shares[k];
        widgets::settings_row(ui, theme, t_dyn(&key), |ui| {
            if shape_track(ui, theme, None, &mut v, &format!("{key}_hint")) {
                moved = Some((k, v.clamp(0.0, 1.0)));
            }
            let pct = (v * 100.0).round() as i64;
            ui.label(RichText::new(tf("characters.shape_share", &[("n", &pct)])).size(theme.font_size_small).color(theme.text_muted()));
        });
    }
    if let Some((k, v)) = moved {
        rebalance_shares(&mut shares, k, v);
        for (id, s) in parts.iter().zip(&shares) {
            state.appearance.shape.insert(id.clone(), *s);
        }
        state.appearance_dirty = true;
    }
}

/// Set share `k` to `v` and scale the others so all still add up to one (in proportion to what they
/// were; evenly when the others were all nothing).
pub(crate) fn rebalance_shares(shares: &mut [f32], k: usize, v: f32) {
    let n = shares.len();
    if k >= n {
        return;
    }
    let v = v.clamp(0.0, 1.0);
    let others: f32 = shares.iter().enumerate().filter(|(i, _)| *i != k).map(|(_, s)| *s).sum();
    for (i, s) in shares.iter_mut().enumerate() {
        if i == k {
            *s = v;
        } else if others > 1e-6 {
            *s *= (1.0 - v) / others;
        } else if n > 1 {
            *s = (1.0 - v) / (n - 1) as f32;
        }
    }
}

fn draw_wardrobe(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("characters.wardrobe")).strong().color(theme.text_primary()));
    let slot_ids: Vec<String> = state.equipment_slots.iter().map(|(id, _)| id.clone()).collect();
    for slot in slot_ids.iter().map(|s| s.as_str()) {
        let current = state.outfit.equipped.get(slot).cloned();
        // Cosmetics available for this slot (id, name).
        let items: Vec<(String, String)> = state
            .cosmetics_list
            .iter()
            .filter(|(_, _, s)| s == slot)
            .map(|(id, name, _)| (id.clone(), name.clone()))
            .collect();
        if items.is_empty() {
            continue;
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(cap(slot)).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(current.is_none(), t("characters.none")).hint("characters.none_hint").clicked() {
                state.outfit.equipped.remove(slot);
                state.outfit_dirty = true;
            }
            for (id, name) in &items {
                let selected = current.as_deref() == Some(id.as_str());
                if ui.selectable_label(selected, name).hint("characters.wear_hint").clicked() {
                    state.outfit.equipped.insert(slot.to_string(), id.clone());
                    state.outfit_dirty = true;
                }
            }
        });
    }
}

fn draw_backdrop(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("characters.backdrop")).strong().color(theme.text_primary()));
    let n = state.showroom_backdrop_names.len().max(1);
    ui.horizontal(|ui| {
        if ui.button(RichText::new("  <  ")).hint("characters.backdrop_prev_hint").clicked() {
            state.showroom_backdrop = (state.showroom_backdrop + n - 1) % n;
        }
        let name = state
            .showroom_backdrop_names
            .get(state.showroom_backdrop)
            .cloned()
            .unwrap_or_default();
        ui.label(RichText::new(name).color(theme.text_secondary()));
        if ui.button(RichText::new("  >  ")).hint("characters.backdrop_next_hint").clicked() {
            state.showroom_backdrop = (state.showroom_backdrop + 1) % n;
        }
    });
}

/// Capitalize a slot id for display ("chest" -> "Chest").
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "showroom_tests.rs"]
pub(crate) mod tests;
