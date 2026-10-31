//! The bottom of every app page: the unfinished-software warning, and later the chat footer above
//! it (docs/design/app-bottom-strip.md, 2026-10-30).
//!
//! The operator, 2026-10-30: "I feel we should slap an anchored warning somewhere on the menu
//! pages not fps mode camera, that explains this software is wip and not to trust it with your
//! life at the moment ... Until v1.0 I want to make it obvious it is still WIP. It could be
//! anchored to the bottom of the screen. The footer chat would go above the WIP Warning visually.
//! We could include a link on the wip warning to a page about more details about the warning and
//! development."
//!
//! THE STACK, bottom up: the warning, then (rung 2 of the plan) the chat footer, then the page.
//! egui stacks bottom panels in the order they are added, the first one outermost, so [`draw`]
//! adds the warning first and anything above it after. lib.rs calls [`draw`] once a frame, after
//! the top menu and BEFORE the page draws, so the page's own bottom panels (the crafting queue,
//! the construction palette) sit above the strip and its side panels end where the strip begins.
//!
//! WHERE IT SHOWS: on every page a person reads, the title screen included, and never in the
//! first-person world (`GuiPage::None`, [`shown_on`]). An in-world wall screen draws its page
//! through `dispatch::draw_tool_page`, which does not call this, so a monitor in the world never
//! carries the strip: it is part of the world, not of the app.
//!
//! UNTIL VERSION 1.0, decided by the version and nothing else ([`wip_shown`]): there is no setting
//! for it, so it cannot be left on or switched off by a forgotten flag. Every 0.x version shows it,
//! and so does a pre-release of 1.0 (`1.0.0-rc.1`); a version it cannot read shows it too.
//!
//! THE WORDS are the dated finding's (`docs/reference/findings/2026-10-30-wip-warning-and-
//! liability.md`): what the software is, what not to rely on it for, and that it comes as is with
//! no warranty, which is what the licence says. Its link opens the Library's page about it
//! (`docs/user/rights/unfinished_software.md`, [`DETAILS_SLUG`]). The website draws the same strip
//! from the same keys (`web/shared/shell.js`, `#hos-wip-strip`).
//!
//! COST: one panel holding three labels, laid out each frame the app already draws; it reads no
//! file, keeps nothing between frames, and never asks for a repaint, so an idle app stays idle.
//! Colours are theme tokens (`warning_bg`, `warning`, `text_primary`, `text_muted`; their contrast
//! is checked in scripts/tests/contrast.test.js).

use egui::{Align, Layout, RichText, Sense, Stroke};

use crate::gui::i18n::{t, t_marked, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiPage, GuiState, VERSION};

/// The Library document the warning's link opens (`docs/user/rights/unfinished_software.md`; the
/// slug is its file name, `scripts/build-library.js` `slugOf`). A test checks the shipped Library has it.
pub const DETAILS_SLUG: &str = "unfinished-software";

/// The warning's panel, by id. The chat footer (rung 2) gets its own, added after this one.
const WARNING_PANEL: &str = "hos_wip_strip";

/// Whether the bottom strip belongs on this page: every page except the first-person world.
pub fn shown_on(page: GuiPage) -> bool {
    page != GuiPage::None
}

/// Whether the unfinished-software warning shows for this version: before 1.0 it does.
///
/// Semantic versions: the major number below 1 shows it, and so does any pre-release (the part
/// after a `-`, as in `1.0.0-rc.1`), because a release candidate is not the finished thing.
/// Build metadata (after a `+`) is not a pre-release. A version it cannot read shows the warning:
/// failing toward the warning is the safe direction.
pub fn wip_shown(version: &str) -> bool {
    let v = version.trim().trim_start_matches('v');
    let v = v.split('+').next().unwrap_or("");
    let (core, pre_release) = match v.split_once('-') {
        Some((core, pre)) => (core, !pre.is_empty()),
        None => (v, false),
    };
    // Digits only: `u64::from_str` alone would also take "+1", which the website's rule does not.
    let major = core.split('.').next().unwrap_or("");
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    match major.parse::<u64>() {
        Ok(major) => major < 1 || pre_release,
        Err(_) => true,
    }
}

/// Draw the bottom strip for the page that is open. Called once a frame by lib.rs, before the page.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    draw_for_version(ctx, theme, state, VERSION);
}

/// [`draw`] with the version given, so a snapshot can pin it (the main menu's snapshot pins its
/// version the same way; unpinned, the picture would change with every release).
pub(crate) fn draw_for_version(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, version: &str) {
    if !shown_on(state.active_page) {
        return;
    }
    if wip_shown(version) {
        draw_warning(ctx, theme, state, version);
    }
}

/// The warning: one line on a dark amber ground across the whole width, its words on the left, What
/// this means and the version on the right. On a narrow window the words wrap onto a second line
/// rather than being cut off, since a warning cut short says less than it should.
fn draw_warning(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, version: &str) {
    let size = theme.font_size_small;
    egui::TopBottomPanel::bottom(WARNING_PANEL)
        .frame(egui::Frame::none().fill(theme.warning_bg()).inner_margin(egui::Margin::symmetric(8, 3)))
        // Its own edge in the warning colour, not the panel's grey separator.
        .show_separator_line(false)
        .resizable(false)
        .show(ctx, |ui| {
            let edge = ui.max_rect().expand2(egui::vec2(8.0, 3.0));
            ui.painter().hline(edge.x_range(), edge.top() + 0.5, Stroke::new(1.0, theme.warning()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(tf("wip.version", &[("version", &version)])).size(size).color(theme.text_muted()));
                ui.add_space(theme.spacing_md);
                let more = ui
                    .add(egui::Label::new(RichText::new(t("wip.more")).size(size).color(theme.warning()).underline()).sense(Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .hint("wip.more_hint");
                if more.clicked() {
                    open_details(state);
                }
                ui.add_space(theme.spacing_md);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // The warning sign is one of the glyphs the app's font draws (tests/icon_glyph_lint.rs).
                    ui.label(RichText::new("⚠").size(size).color(theme.warning()));
                    widgets::glossary_words(ui, theme, t_marked("wip.line"), size, theme.text_primary());
                });
            });
        });
}

/// What this means: the Library, open on the page about the warning.
fn open_details(state: &mut GuiState) {
    if !crate::gui::pages::library::show_doc(state, DETAILS_SLUG) {
        // Only when data/library/ is missing the page; the test below keeps the slug shipped.
        log::warn!("bottom strip: the Library has no {DETAILS_SLUG:?} document; opening the Library as it is");
    }
    state.push_nav_to(GuiPage::Library);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The versions that show the warning, from the list the website's test reads too
    /// (scripts/tests/fixtures/wip-versions.json, scripts/tests/wip-strip-web.test.js), so the two
    /// apps cannot disagree about which builds are unfinished.
    #[test]
    fn the_warning_shows_before_one_point_oh_and_not_after() {
        let text = std::fs::read_to_string("scripts/tests/fixtures/wip-versions.json").expect("read the shared version list");
        let list: serde_json::Value = serde_json::from_str(&text).expect("the version list is JSON");
        let of = |k: &str| -> Vec<String> {
            list[k].as_array().unwrap_or_else(|| panic!("the version list has no {k:?}")).iter().map(|v| v.as_str().unwrap().to_string()).collect()
        };
        let (shown, hidden) = (of("shown"), of("hidden"));
        assert!(shown.len() >= 5 && hidden.len() >= 3, "the shared version list is nearly empty");
        for v in &shown {
            assert!(wip_shown(v), "{v:?} is unfinished, so the warning must show");
        }
        for v in &hidden {
            assert!(!wip_shown(v), "{v:?} is a finished release, so the warning must not show");
        }
    }

    #[test]
    fn the_running_version_shows_the_warning_until_one_point_oh() {
        // Pins today's state so a reader sees which way the build is: 0.x shows it. The day the
        // version reaches 1.0 this test fails on purpose, to make that a decision and not a surprise:
        // delete it then, with the warning, in the same piece of work.
        assert!(wip_shown(VERSION), "version {VERSION} hides the warning: is this 1.0?");
    }

    #[test]
    fn the_strip_is_on_every_app_page_and_never_in_the_world() {
        assert!(!shown_on(GuiPage::None), "the first-person world must not carry the strip");
        for &page in crate::gui::dispatch::ALL_PAGES {
            if page != GuiPage::None {
                assert!(shown_on(page), "{page:?} is a page a person reads, so the strip belongs on it");
            }
        }
    }

    /// Drawn for real under a headless context: on a page the strip takes room at the bottom of the
    /// window (what is left for the page ends above it); in the world, or on a 1.0 build, it takes none.
    #[test]
    fn the_strip_takes_room_on_a_page_and_none_in_the_world_or_after_one_point_oh() {
        let theme = crate::gui::theme::load_theme();
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let left_for_page = |page: GuiPage, version: &str| -> f32 {
            let ctx = egui::Context::default();
            crate::gui::install_fonts(&ctx);
            theme.apply_to_egui(&ctx);
            let mut state = GuiState::default();
            state.active_page = page;
            let mut bottom = 0.0;
            for _ in 0..2 {
                let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
                let _ = ctx.run(input, |ctx| {
                    draw_for_version(ctx, &theme, &mut state, version);
                    bottom = ctx.available_rect().bottom();
                });
            }
            bottom
        };
        let on_page = left_for_page(GuiPage::Humanity, "0.1547.0");
        assert!(on_page < screen.bottom() - 10.0, "the warning took no room on a page: the page ends at {on_page}");
        assert!(on_page > screen.bottom() - 80.0, "the warning is taller than a line or two: the page ends at {on_page}");
        assert_eq!(left_for_page(GuiPage::None, "0.1547.0"), screen.bottom(), "the warning drew in the first-person world");
        assert_eq!(left_for_page(GuiPage::Humanity, "1.0.0"), screen.bottom(), "the warning drew on a 1.0 build");
    }

    /// The link must lead somewhere: the shipped Library has the page it opens.
    #[test]
    fn what_this_means_opens_a_document_the_library_ships() {
        let lib = crate::gui::load_library(std::path::Path::new("data"));
        let found = lib
            .sections
            .iter()
            .flat_map(|s| s.categories.iter())
            .flat_map(|c| c.entries.iter())
            .any(|e| e.slug == DETAILS_SLUG);
        assert!(found, "data/library/ has no {DETAILS_SLUG:?}: run node scripts/build-library.js, or fix the slug");
        let mut state = GuiState::default();
        state.library = lib.sections;
        state.active_page = GuiPage::Humanity;
        open_details(&mut state);
        assert_eq!(state.active_page, GuiPage::Library);
        assert_eq!(crate::gui::pages::library::shown_doc(&state).as_deref(), Some(DETAILS_SLUG));
    }
}
