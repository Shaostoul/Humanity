// Child of pages/dev_tab.rs: the Dev tab and the top menu, after the Platform tab was retired
// (2026-10-20). Headless: the page and the menu are drawn into an egui context with no window and
// the text they painted is read back. Each test was seen failing once; what it saw red is at the
// test.

use crate::gui::theme::{load_theme, Theme};
use crate::gui::{GuiPage, GuiState};

fn ctx_and_theme() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    (ctx, theme)
}

/// Every text drawn, with where.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    out.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut found);
    }
    found
}

/// Three settled frames of `draw` in a window of `size`; the texts of the last.
fn drawn(size: egui::Vec2, mut draw: impl FnMut(&egui::Context, &mut Theme)) -> Vec<(String, egui::Rect)> {
    let (ctx, mut theme) = ctx_and_theme();
    let mut last = None;
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), ..Default::default() };
        last = Some(ctx.run(input, |ctx| draw(ctx, &mut theme)));
    }
    texts(&last.expect("three frames ran"))
}

/// THE DEV TAB LISTS THE DEVELOPER TOOLS, AND ONLY THEM: the Dev page, drawn through the app's
/// own page table, has a side list titled Dev holding Bugs, Testing, Performance, Dev, Planet
/// Tuner and Claude, in that order, and none of what the Platform tab also held (Recovery is in
/// Settings now; Calculator, Notes, Calendar and Browser are on the Apps page).
///
/// Seen red 2026-10-20 with `GuiPage::Dev` still drawing the old Platform list: "Platform is not
/// in the Dev tab's list: [("Platform", 13.0), ("Recovery", 65.0), ("Calculator", 99.0), ("Notes",
/// 133.0), ("Calendar", 167.0), ("Bugs", 201.0), ... ("Browser", 371.0), ("Title", 91.0), ...]".
/// The "Title" there was the Bugs page's form, which a looser column test let in; the column
/// below is the side panel's own (it opens 190 px wide, and the page beside it starts after it).
///
/// Claude joined the list on 2026-10-22 (dev_agents/).
/// Seen red 2026-10-23 with Claude taken out of SECTIONS: "Claude is in the Dev tab's list:
/// [(\"Dev\", 13.0), (\"Bugs\", 65.0), (\"Testing\", 99.0), (\"Performance\", 133.0), (\"Dev\",
/// 167.0), (\"Planet Tuner\", 201.0)]".
#[test]
fn the_dev_tab_lists_the_developer_tools() {
    let mut state = GuiState::default();
    let t = drawn(egui::vec2(1400.0, 1000.0), |ctx, theme| {
        crate::gui::dispatch::draw_tool_page(ctx, GuiPage::Dev, theme, &mut state);
    });
    // The side list is the left panel; the section drawn beside it is not part of the question.
    let side: Vec<(&str, f32)> =
        t.iter().filter(|(_, r)| r.left() < 150.0 && r.right() < 195.0).map(|(s, r)| (s.as_str(), r.top())).collect();
    for gone in ["Platform", "Recovery", "Calculator", "Notes", "Calendar", "Browser"] {
        assert!(!side.iter().any(|(s, _)| *s == gone), "{gone} is not in the Dev tab's list: {side:?}");
    }
    let tops: Vec<f32> = ["Bugs", "Testing", "Performance", "Planet Tuner", "Claude", "Journal"]
        .iter()
        .map(|name| side.iter().find(|(s, _)| s == name).unwrap_or_else(|| panic!("{name} is in the Dev tab's list: {side:?}")).1)
        .collect();
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "in the side list's order: {side:?}");
    // "Dev" is drawn twice: the list's title, and the Dev tools section between Performance and
    // Planet Tuner.
    let devs: Vec<f32> = side.iter().filter(|(s, _)| *s == "Dev").map(|(_, y)| *y).collect();
    assert_eq!(devs.len(), 2, "the title Dev and the section Dev: {side:?}");
    assert!(devs[0] < tops[0] && tops[2] < devs[1] && devs[1] < tops[3], "Dev's title on top, its section after Performance: {side:?}");
    assert_eq!(crate::gui::dispatch::page_id(GuiPage::Dev), "dev");
}

/// THE TOP MENU HAS DEV AND NO PLATFORM: the nav bar every page sits under names the developer
/// tools' tab Dev, and Platform is nowhere on it.
///
/// Seen red 2026-10-20 with the menu's entry still labelled Platform: "the top menu has Dev:
/// [..., "Crafting", "·", "Maps", "·", "Platform", "·", "Library", "·", "Settings", "Aa"]".
#[test]
fn the_top_menu_has_dev_and_no_platform() {
    let mut state = GuiState::default();
    let t = drawn(egui::vec2(2400.0, 200.0), |ctx, theme| {
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, &mut state);
    });
    let names: Vec<&str> = t.iter().map(|(s, _)| s.as_str()).collect();
    assert!(names.contains(&"Settings") && names.contains(&"Library"), "the menu was drawn: {names:?}");
    assert!(names.contains(&"Dev"), "the top menu has Dev: {names:?}");
    assert!(!names.contains(&"Platform"), "Platform is gone from the top menu: {names:?}");
}
