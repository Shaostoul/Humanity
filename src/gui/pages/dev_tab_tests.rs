// Child of pages/dev_tab.rs: the Dev page and the top menu, after the Platform tab was retired
// (2026-10-20) and the Dev page's side list became the Dev row of the top menu (navigation rung 2,
// 2026-11-03). Headless: the page and the menu are drawn into an egui context with no window and
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

/// THE DEV ROW LISTS THE DEVELOPER TOOLS, AND ONLY THEM: on the Dev page, the top menu's second
/// row holds Bugs, Testing, Performance, Planet Tuner, Claude and Journal, in that order, and none
/// of what the Platform tab also held (Recovery is in Settings now; Calculator, Notes, Calendar and
/// Browser are on the Apps page); and the Dev page itself has no side list any more, opening on the
/// developer's tools.
///
/// Seen red 2026-10-20 with `GuiPage::Dev` still drawing the old Platform list: "Platform is not
/// in the Dev tab's list: [("Platform", 13.0), ("Recovery", 65.0), ...]". Claude joined the list on
/// 2026-10-22 (dev_agents/); seen red 2026-10-23 with Claude taken out of SECTIONS. Rung 2: seen
/// red 2026-11-03 with a side list drawn back beside the page (Bugs and Performance in it): "the
/// Dev page has no side list any more: [(\"Performance\", 114.0), (\"Dev\", 105.0)]".
#[test]
fn the_dev_row_lists_the_developer_tools() {
    let mut state = GuiState::default();
    state.active_page = GuiPage::Dev;
    let size = egui::vec2(1800.0, 1000.0);
    let t = drawn(size, |ctx, theme| {
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, &mut state);
        crate::gui::dispatch::draw_tool_page(ctx, GuiPage::Dev, theme, &mut state);
    });
    let theme = load_theme();
    let (top, bottom) = (theme.nav_primary_height, theme.nav_primary_height + theme.nav_secondary_height);
    let mut row: Vec<(&str, f32)> = t.iter().filter(|(_, r)| r.center().y > top && r.center().y < bottom).map(|(s, r)| (s.as_str(), r.left())).collect();
    row.sort_by(|a, b| a.1.total_cmp(&b.1));
    let names: Vec<&str> = row.iter().map(|(s, _)| *s).collect();
    assert_eq!(names, ["Bugs", "Testing", "Performance", "Planet Tuner", "Claude", "Journal"], "the Dev row");
    for gone in ["Platform", "Recovery", "Calculator", "Notes", "Calendar", "Browser"] {
        assert!(!names.contains(&gone), "{gone} is not in the Dev row: {names:?}");
    }
    // No side list: nothing is drawn in a narrow column at the page's left under the rows.
    let side: Vec<(&str, f32)> = t.iter().filter(|(_, r)| r.top() > bottom + 4.0 && r.right() < 195.0).map(|(s, r)| (s.as_str(), r.top())).collect();
    assert!(!side.iter().any(|(s, _)| *s == "Bugs" || *s == "Performance"), "the Dev page has no side list any more: {side:?}");
    assert_eq!(state.active_dev_section, crate::gui::pages::dev_tab::FIRST_SECTION, "the Dev page opens on the developer's tools");
    assert_eq!(crate::gui::dispatch::page_id(GuiPage::Dev), "dev");
}

/// THE TOP MENU HAS DEV AND NO PLATFORM: the menu every page sits under names the developer
/// tools' category Dev, and Platform is nowhere on it.
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
