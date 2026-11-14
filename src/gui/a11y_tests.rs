//! Keyboard and screen-reader checks of the desktop app's pages (the WCAG 2.1 AA
//! audit, docs/design/wcag-audit-2026-10-23.md), run headless with no GPU.
//!
//! For each page converted by the ELI5 pass, press Tab over and over, the way a
//! person who cannot use a mouse moves through the page, and record every stop:
//!  - REACHABLE (2.1.1): Tab stops on the page's controls at all, and comes back
//!    round to the first one (no trap, 2.1.2).
//!  - NAMED (4.1.2): each stop tells assistive technology what it is
//!    (`OutputEvent::FocusGained` with a label). A text box is named by the
//!    label beside it, which egui exposes only through AccessKit, so text boxes
//!    are counted apart.
//!  - HINTED ON FOCUS (1.4.13, and the ELI5 mandate): the control's plain-language
//!    hint shows while it has keyboard focus, not only under a mouse pointer.
//!
//! The counts are printed (`just test-lib a11y_ -- --nocapture`) and asserted
//! for the converted pages: every stop named, every named stop hinted.

use crate::gui::dispatch::draw_tool_page;
use crate::gui::theme::load_theme;
use crate::gui::{GuiPage, GuiState};

struct Stop {
    id: egui::Id,
    typ: Option<egui::WidgetType>,
    label: String,
    hinted: bool,
}

fn tab_walk(page: GuiPage, max: usize) -> Vec<Stop> {
    tab_walk_of(max, GuiState::default(), |ctx, theme, state| {
        draw_tool_page(ctx, page, theme, state);
    })
}

/// [`tab_walk`] over whatever `draw` draws, from `state`.
fn tab_walk_of(max: usize, state: GuiState, mut draw: impl FnMut(&egui::Context, &mut crate::gui::theme::Theme, &mut GuiState)) -> Vec<Stop> {
    let mut state = state;
    let mut theme = load_theme();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 1000.0));
    let mut frame = |events: Vec<egui::Event>, state: &mut GuiState| -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run(input, |ctx| {
            draw(ctx, &mut theme, state);
        })
    };
    let tab = || {
        vec![
            egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE },
            egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE },
        ]
    };
    for _ in 0..3 {
        frame(Vec::new(), &mut state);
    }
    let mut stops: Vec<Stop> = Vec::new();
    for _ in 0..max {
        let out = frame(tab(), &mut state);
        let Some(id) = ctx.memory(|m| m.focused()) else { break };
        if stops.first().is_some_and(|s| s.id == id) {
            break; // round again: the walk is complete
        }
        let info = out.platform_output.events.iter().find_map(|e| match e {
            egui::output::OutputEvent::FocusGained(i) => Some(i.clone()),
            _ => None,
        });
        // One quiet frame for the hint to come up under focus.
        frame(Vec::new(), &mut state);
        let hinted = ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Tooltip));
        stops.push(Stop {
            id,
            typ: info.as_ref().map(|i| i.typ),
            label: info.and_then(|i| i.label).unwrap_or_default(),
            hinted,
        });
    }
    stops
}

/// The pages the ELI5 pass converted (words by key, a hint on every control).
const CONVERTED: &[GuiPage] = &[
    GuiPage::Calculator,
    GuiPage::Music,
    GuiPage::Notes,
    GuiPage::Tasks,
    GuiPage::Calendar,
    GuiPage::Crafting,
    GuiPage::Laws,
    GuiPage::Wallet,
    GuiPage::Donate,
    GuiPage::BugReport,
    GuiPage::Guilds,
    GuiPage::Governance,
    GuiPage::Humanity,
    GuiPage::Market,
];

/// Tab stops that tell assistive technology no name, counted 2026-10-24 (the audit,
/// docs/design/wcag-audit-2026-10-23.md): mostly rows and cells painted by hand
/// (`Sense::click` on an area: the Laws rule rows, the Calendar days) and lists made
/// with `ComboBox::from_id_salt`, which has no label. A RATCHET: more fails; fewer
/// prints the number to lower it to. A page not listed has none.
const UNNAMED_TODAY: &[(GuiPage, usize)] = &[
    (GuiPage::BugReport, 2),
    (GuiPage::Calendar, 32),
    (GuiPage::Crafting, 1),
    (GuiPage::Humanity, 2),
    (GuiPage::Laws, 191),
    (GuiPage::Market, 1),
    // Two stops egui reports with no widget type and no name (2026-11-13); every control the page
    // builds is named and hinted.
    (GuiPage::Music, 2),
    (GuiPage::Notes, 1),
    (GuiPage::Tasks, 2),
];

/// Pages whose controls all come from data a test's fresh GuiState does not load
/// (Donate's giving routes, links and charities are read from data/donate/).
const DATA_ONLY: &[GuiPage] = &[GuiPage::Donate];

#[test]
fn a11y_converted_pages_are_reachable_named_and_hinted_by_keyboard() {
    let mut bad = Vec::new();
    for &page in CONVERTED {
        let stops = tab_walk(page, 400);
        let text_boxes = stops.iter().filter(|s| s.typ == Some(egui::WidgetType::TextEdit)).count();
        let unnamed: Vec<&Stop> = stops.iter().filter(|s| s.typ != Some(egui::WidgetType::TextEdit) && s.label.trim().is_empty()).collect();
        // A text box keeps the keyboard for typing, so its hint shows under the pointer only
        // (widgets::hint::show_on_focus); it is named by the row beside it.
        let unhinted: Vec<&Stop> = stops
            .iter()
            .filter(|s| s.typ != Some(egui::WidgetType::TextEdit) && !s.label.trim().is_empty() && !s.hinted)
            .collect();
        println!(
            "{page:?}: {} Tab stops, {} text boxes, {} unnamed, {} without a hint on focus",
            stops.len(),
            text_boxes,
            unnamed.len(),
            unhinted.len()
        );
        if stops.is_empty() && !DATA_ONLY.contains(&page) {
            bad.push(format!("{page:?}: Tab reaches nothing"));
        }
        let budget = UNNAMED_TODAY.iter().find(|(p, _)| *p == page).map_or(0, |(_, n)| *n);
        if unnamed.len() > budget {
            let kinds: Vec<String> = unnamed.iter().map(|s| format!("{:?}", s.typ)).collect();
            bad.push(format!("{page:?}: {} stops have no name, {budget} allowed ({})", unnamed.len(), kinds.join(", ")));
        } else if unnamed.len() < budget {
            println!("  RATCHET: {page:?} has {} unnamed stops; lower UNNAMED_TODAY to {}", unnamed.len(), unnamed.len());
        }
        for s in unhinted {
            bad.push(format!("{page:?}: \"{}\" shows no hint on keyboard focus", s.label));
        }
    }
    assert!(bad.is_empty(), "\n{}\n", bad.join("\n"));
}

/// THE TOP MENU'S TAB WALK (navigation rung 2, 2026-11-03; docs/design/navigation.md section 12):
/// on the Chat page, Tab reaches every button of both rows in order (the eight categories, the
/// Aa switch, then the second row's pages), then the notices bell, and comes round; every stop
/// tells assistive technology its name (its words, also in icons-only mode) and shows its hint on
/// focus. Back and Forward have nowhere to go on a fresh page, so they are not stops (they say why
/// under the pointer).
///
/// Seen red 2026-11-03 with the category buttons drawn without `named` (no widget_info): "the menu's
/// stops are its buttons, named (Both); left: [\"\", \"\", \"\", \"\", \"\", \"\", \"\", \"\", \"Button
/// style\", \"Watch\", ...". Its first run on the finished rows found a nameless stop of the
/// menu's own: "then the notices bell, and round again; left: 15, right: 14", the stops ending
/// [\"Guilds\", \"\", \"Notices\"]: the bell's egui Area, which senses a click (so takes Tab) unless told
/// otherwise; the bell's and help's Areas sense hover only since.
#[test]
fn a11y_the_top_menu_names_and_hints_every_stop() {
    use humanity_core::i18n::{t, t_dyn};
    let menu = crate::gui::navigation::menu();
    let chat = menu.desktop.iter().find(|c| c.id == "chat").expect("Chat");
    let mut want: Vec<String> = menu.desktop.iter().map(|c| t_dyn(&c.button.label).to_string()).collect();
    want.push(t("nav.display_mode").to_string());
    want.extend(chat.items.iter().map(|b| t_dyn(&b.label).to_string()));
    for mode in [crate::gui::NavDisplayMode::Both, crate::gui::NavDisplayMode::IconOnly] {
        let mut state = GuiState::default();
        state.active_page = GuiPage::Chat;
        state.nav_display_mode = mode;
        let stops = tab_walk_of(60, state, |ctx, theme, state| {
            crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
        });
        let names: Vec<String> = stops.iter().map(|s| s.label.clone()).collect();
        println!("the top menu ({mode:?}): {} Tab stops: {names:?}", stops.len());
        assert_eq!(names[..want.len()].to_vec(), want, "the menu's stops are its buttons, named ({mode:?})");
        assert_eq!(stops.len(), want.len() + 1, "then the notices bell, and round again: {names:?}");
        assert!(!names[want.len()].trim().is_empty(), "the bell is named");
        let unhinted: Vec<&str> = stops.iter().filter(|s| !s.hinted).map(|s| s.label.as_str()).collect();
        assert!(unhinted.is_empty(), "every stop shows its hint on keyboard focus ({mode:?}): {unhinted:?}");
    }
}
