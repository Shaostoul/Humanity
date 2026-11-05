// Child of pages/escape_menu.rs (#[path]): the top menu's two rows, drawn headless into an egui
// context with no window, and driven the way a person does (docs/design/navigation.md, rung 2): a
// click on each primary button, a rest of the pointer on another (the preview), the pointer moving
// away, a click in the preview, the preview switched off, Play tapped and held and its page's
// button, and More at the operator's two window sizes and three text sizes. What a frame painted
// is read back from its shapes; where a click went, from the GuiState. Each test was seen red
// once; what it saw is at the test.

use super::*;
use crate::gui::theme::load_theme;

/// A menu drawn headless at a size, on egui's own clock (seconds, 60 frames a second).
struct Rows {
    ctx: egui::Context,
    theme: Theme,
    state: GuiState,
    t: f64,
    size: Vec2,
    /// The page under the menu is drawn too (Play's page and its button).
    with_page: bool,
    /// Hints on, and the help button drawn in the corner beside the bell as the app does (only the
    /// hints' own test: everywhere else a resting pointer would bring one up over the rows read).
    hints: bool,
}

impl Rows {
    fn new(size: Vec2) -> Self {
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let theme = load_theme();
        theme.apply_to_egui(&ctx);
        // No hover hints in these pictures: a resting pointer would bring one up over the rows
        // being read (the hints themselves are the a11y walk's, a11y_tests.rs).
        ctx.all_styles_mut(|s| {
            s.animation_time = 0.0;
            s.interaction.tooltip_delay = 1.0e6;
        });
        let mut state = GuiState::default();
        state.active_page = GuiPage::Chat;
        Rows { ctx, theme, state, t: 10.0, size, with_page: false, hints: false }
    }

    /// One frame, 1/60 s after the last, with `events`.
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)), time: Some(self.t), events, ..Default::default() };
        let (theme, state, with_page, hints) = (&mut self.theme, &mut self.state, self.with_page, self.hints);
        let mode = if hints { crate::gui::HintDisplay::Full } else { crate::gui::HintDisplay::Off };
        state.settings.hint_display = mode;
        self.ctx.run(input, |ctx| {
            crate::gui::widgets::hint::set_mode(ctx, mode);
            draw_nav_bar(ctx, theme, state);
            if hints {
                crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
            }
            if with_page && state.active_page != GuiPage::None {
                crate::gui::dispatch::draw_tool_page(ctx, state.active_page, theme, state);
            }
        })
    }

    /// Frames until `secs` have passed; the last frame's output.
    fn wait(&mut self, secs: f64) -> egui::FullOutput {
        let until = self.t + secs;
        let mut out = self.frame(Vec::new());
        while self.t < until {
            out = self.frame(Vec::new());
        }
        out
    }

    fn move_to(&mut self, at: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(at)])
    }

    fn press(&mut self, at: egui::Pos2, pressed: bool) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() }])
    }

    /// A click at `at`: the pointer there, pressed, let go.
    fn click(&mut self, at: egui::Pos2) {
        self.move_to(at);
        self.press(at, true);
        self.press(at, false);
    }

    fn key(&mut self, key: egui::Key) -> egui::FullOutput {
        let m = egui::Modifiers::NONE;
        self.frame(vec![
            egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m },
            egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m },
        ])
    }
}

/// Every text a frame painted, in paint order, with where (clipped to what shows).
fn texts(out: &egui::FullOutput) -> Vec<(String, Rect)> {
    fn visit(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() && !t.galley.text().trim().is_empty() {
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

/// The words drawn in a band of the window (y from `top` to `bottom`), left to right.
fn band(out: &egui::FullOutput, top: f32, bottom: f32) -> Vec<String> {
    let mut t: Vec<(String, Rect)> = texts(out).into_iter().filter(|(_, r)| r.center().y > top && r.center().y < bottom).collect();
    t.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    t.into_iter().map(|(s, _)| s).collect()
}

fn primary(out: &egui::FullOutput, theme: &Theme) -> Vec<String> {
    band(out, 0.0, theme.nav_primary_height)
}

fn secondary(out: &egui::FullOutput, theme: &Theme) -> Vec<String> {
    band(out, theme.nav_primary_height, theme.nav_primary_height + theme.nav_secondary_height)
}

/// Where `words` were drawn in the last frame (the centre of their text).
fn at(out: &egui::FullOutput, words: &str) -> egui::Pos2 {
    texts(out).into_iter().find(|(s, _)| s == words).unwrap_or_else(|| panic!("{words:?} is drawn: {:?}", texts(out).iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>())).1.center()
}

fn words_of(b: &NavButton) -> String {
    t_dyn(&b.label).to_string()
}

fn category<'a>(menu: &'a Menu, id: &str) -> &'a NavCategory {
    menu.desktop.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("a category `{id}`"))
}

const TV: Vec2 = Vec2::new(2560.0, 1440.0);

/// THE ROWS ARE THE FILE'S (the parity pair's desktop half; the website's half is
/// scripts/tests/navigation-web.test.js): the primary row shows the eight categories' words in the
/// file's order between Back and Forward's arrows and the Aa switch, and the second row the words
/// of the docked category's items (Chat's, on the Chat page), read from the file here, not through
/// the module's resolved rows.
///
/// Seen red 2026-11-03 with the primary row drawing one category fewer than fit (`take(fit.min(7))`):
/// "the primary row is the file's categories; left: [\"Humanity\", ..., \"Dev\", \"aa\"], right:
/// [\"Humanity\", ..., \"Dev\", \"Settings\", \"aa\"]".
#[test]
fn the_two_rows_are_drawn_from_the_file() {
    let mut r = Rows::new(TV);
    r.state.nav_display_mode = NavDisplayMode::TextOnly;
    r.frame(Vec::new());
    let out = r.frame(Vec::new());
    let file = &crate::gui::navigation::menu().file;
    let mut want: Vec<String> = file.categories.iter().map(|c| t_dyn(&c.label).to_string()).collect();
    want.push(NavDisplayMode::TextOnly.glyph().to_string());
    assert_eq!(primary(&out, &r.theme), want, "the primary row is the file's categories");
    let chat = file.categories.iter().find(|c| c.id == "chat").unwrap();
    let want: Vec<String> = chat.items.iter().filter(|i| i.page.is_some()).map(|i| t_dyn(&i.label).to_string()).collect();
    assert_eq!(secondary(&out, &r.theme), want, "the second row is Chat's pages on the Chat page");
}

/// A CLICK ON EACH PRIMARY BUTTON OPENS ITS PAGE, and its row docks under it: Humanity the
/// Humanity page, Play Play's page (not the world: a tap), Dev the developer's tools, and so on
/// for all eight; the next frame's second row is that category's.
///
/// Seen red 2026-11-03 with `open` setting the Dev page's part only when the entry names one (the
/// Dev button after Claude left the page on Claude): "Dev opens the developer's tools; left:
/// \"claude\", right: \"dev\"".
#[test]
fn a_click_on_each_primary_button_opens_its_page() {
    let menu = crate::gui::navigation::menu();
    let mut r = Rows::new(TV);
    for cat in &menu.desktop {
        r.state.active_page = GuiPage::Settings;
        r.state.active_dev_section = "claude".to_string();
        let out = r.frame(Vec::new());
        let here = at(&out, &words_of(&cat.button));
        r.click(here);
        assert_eq!(r.state.active_page, cat.button.page, "{} opens its page", cat.id);
        if cat.id == "dev" {
            assert_eq!(r.state.active_dev_section, "dev", "Dev opens the developer's tools");
        }
        r.move_to(egui::pos2(1200.0, 900.0));
        let out = r.frame(Vec::new());
        let want: Vec<String> = cat.items.iter().map(words_of).collect();
        assert_eq!(secondary(&out, &r.theme), want, "{}'s row docks under it", cat.id);
    }
}

/// A REST OF 200 MS ON ANOTHER CATEGORY PREVIEWS ITS ROW, AND MOVING AWAY BRINGS BACK THE PAGE'S
/// OWN: on the Chat page the pointer rests on Play; before 200 ms the row is still Chat's, after
/// it Play's (and nothing on the page moved: the row is the same height). The pointer leaves both
/// rows: 300 ms later Chat's row is back. Escape ends a preview at once (`end_preview`).
///
/// Seen red 2026-11-03 with `step`'s rest check asking a whole second more: "after a rest of 200 ms
/// the second row previews Play's pages; left: [\"Watch\", \"Studio\", \"Files\", \"Guilds\"], right:
/// [\"Characters\", \"Quests\", \"Inventory\", \"Crafting\"]".
#[test]
fn a_rest_on_another_category_previews_its_row_and_leaving_brings_back_the_pages_own() {
    let menu = crate::gui::navigation::menu();
    let chat: Vec<String> = category(&menu, "chat").items.iter().map(words_of).collect();
    let play: Vec<String> = category(&menu, "play").items.iter().map(words_of).collect();
    let mut r = Rows::new(TV);
    let out = r.frame(Vec::new());
    let play_at = at(&out, &t_dyn(&category(&menu, "play").button.label));
    r.move_to(play_at);
    let out = r.wait(0.1);
    assert_eq!(secondary(&out, &r.theme), chat, "before 200 ms the row is still the page's own");
    let out = r.wait(0.15);
    assert_eq!(secondary(&out, &r.theme), play, "after a rest of 200 ms the second row previews Play's pages");
    assert_eq!(r.state.active_page, GuiPage::Chat, "a preview opens nothing");
    // Away from both rows, onto the page.
    r.move_to(egui::pos2(1200.0, 900.0));
    let out = r.wait(0.2);
    assert_eq!(secondary(&out, &r.theme), play, "the preview stays a moment after the pointer leaves");
    let out = r.wait(0.2);
    assert_eq!(secondary(&out, &r.theme), chat, "300 ms after leaving both rows, the page's own row is back");
    // Escape, while a preview shows.
    r.move_to(play_at);
    let out = r.wait(0.3);
    assert_eq!(secondary(&out, &r.theme), play);
    assert!(end_preview(&r.ctx), "Escape ends the preview");
    let out = r.frame(Vec::new());
    assert_eq!(secondary(&out, &r.theme), chat, "the page's own row at once");
    assert!(!end_preview(&r.ctx), "with no preview, Escape is left for the rest of the app");
}

/// A CLICK IN THE PREVIEW OPENS THAT PAGE: the pointer rests on Play, travels down into the second
/// row (the preview stays while it is on either row) and clicks Quests.
///
/// Seen red 2026-11-03 with `in_rows` reading the primary row only: the preview ended while the
/// pointer was in the second row, so the click landed on Chat's own row: "the click in Play's
/// preview opens Quests; left: Studio, right: Quests".
#[test]
fn a_click_in_the_preview_opens_that_page() {
    let menu = crate::gui::navigation::menu();
    let mut r = Rows::new(TV);
    let out = r.frame(Vec::new());
    r.move_to(at(&out, &t_dyn(&category(&menu, "play").button.label)));
    let out = r.wait(0.25);
    let quests = at(&out, t("nav.quests"));
    // Down into the row, slowly enough to cross the leave time if the row did not hold it.
    r.move_to(egui::pos2(quests.x, r.theme.nav_primary_height + 2.0));
    r.wait(0.4);
    r.click(quests);
    assert_eq!(r.state.active_page, GuiPage::Quests, "the click in Play's preview opens Quests");
}

/// THE PREVIEW SWITCH OFF MEANS NO PREVIEW: with Settings > Appearance's "Show a category's pages
/// when the pointer rests on it" off, a rest of a whole second on Play leaves Chat's row, and only
/// a click changes it.
///
/// Seen red 2026-11-03 with `step` not reading the setting: "with the preview off, the row stays
/// Chat's; left: [\"Characters\", \"Quests\", \"Inventory\", \"Crafting\"], right: [\"Watch\",
/// \"Studio\", \"Files\", \"Guilds\"]".
#[test]
fn the_preview_switch_off_means_no_preview() {
    let menu = crate::gui::navigation::menu();
    let chat: Vec<String> = category(&menu, "chat").items.iter().map(words_of).collect();
    let mut r = Rows::new(TV);
    r.state.settings.nav_preview = false;
    let out = r.frame(Vec::new());
    r.move_to(at(&out, &t_dyn(&category(&menu, "play").button.label)));
    let out = r.wait(1.0);
    assert_eq!(secondary(&out, &r.theme), chat, "with the preview off, the row stays Chat's");
}

/// PLAY: A TAP OPENS PLAY'S PAGE, A HOLD GOES INTO THE WORLD, AND THE PAGE'S BUTTON GOES IN (the
/// operator's answer of 2026-11-03). A press let go at once opens Play's page and its row; a press
/// held for `play_hold_ms` (800 ms) goes into the world without a release (the picker, with no
/// game played yet), and its release opens nothing; on Play's page, Enter the world goes in on a
/// click, and on Enter while the keyboard is on it.
///
/// Seen red 2026-11-03, four ways: with a click on Play going into the world (rung 1's): "a tap on
/// Play opens Play's page; left: None, right: Play"; with the hold never counting: "a hold of
/// 800 ms goes into the world; left: Chat, right: None"; with the latch taken out (and egui letting
/// a long press count as a click, as below): "letting go after the hold opens nothing more; left:
/// Play, right: None"; and with Enter the world doing nothing: "Enter the world goes in (a click);
/// left: Play, right: None".
#[test]
fn play_tap_opens_its_page_a_hold_enters_and_its_button_enters() {
    let menu = crate::gui::navigation::menu();
    let play_words = t_dyn(&category(&menu, "play").button.label).to_string();
    // A tap.
    let mut r = Rows::new(TV);
    let out = r.frame(Vec::new());
    let play = at(&out, &play_words);
    r.click(play);
    assert_eq!(r.state.active_page, GuiPage::Play, "a tap on Play opens Play's page");
    assert!(!r.state.launcher_open_select, "a tap does not go in");
    // A hold. egui counts a press up to 0.8 s long as a click, exactly the default hold; let it
    // count presses of any length here, so the release after the hold WOULD be a click (a hold
    // set shorter in the file is one), and only the latch keeps it from opening Play's page.
    let mut r = Rows::new(TV);
    r.ctx.options_mut(|o| o.input_options.max_click_duration = 60.0);
    let out = r.frame(Vec::new());
    let play = at(&out, &play_words);
    r.move_to(play);
    r.press(play, true);
    r.wait(0.5);
    assert_eq!(r.state.active_page, GuiPage::Chat, "half a hold goes nowhere yet");
    r.wait(0.4);
    assert_eq!(r.state.active_page, GuiPage::None, "a hold of 800 ms goes into the world");
    assert!(r.state.launcher_open_select, "with no game played yet, through the character picker");
    assert_eq!(r.state.history.current().map(|p| p.page), Some(GuiPage::Chat), "Escape in the world returns to the page left");
    r.press(play, false);
    r.frame(Vec::new());
    assert_eq!(r.state.active_page, GuiPage::None, "letting go after the hold opens nothing more");
    // Play's page and its button, a click and Enter.
    for keyboard in [false, true] {
        let mut r = Rows::new(TV);
        r.with_page = true;
        r.state.active_page = GuiPage::Play;
        r.frame(Vec::new());
        let out = r.frame(Vec::new());
        let enter = at(&out, t("play.enter"));
        if keyboard {
            // Tab until the keyboard is on Enter the world, then Enter.
            let mut on_it = false;
            for _ in 0..40 {
                let out = r.key(egui::Key::Tab);
                let focused = out.platform_output.events.iter().any(|e| matches!(e, egui::output::OutputEvent::FocusGained(i) if i.label.as_deref() == Some(t("play.enter"))));
                if focused {
                    on_it = true;
                    break;
                }
            }
            assert!(on_it, "Tab reaches Enter the world");
            r.key(egui::Key::Enter);
        } else {
            r.click(enter);
        }
        assert_eq!(r.state.active_page, GuiPage::None, "Enter the world goes in ({})", if keyboard { "Enter" } else { "a click" });
    }
}

/// The rows at a Font Size in a window of `px` pixels: the words drawn in each of the two rows,
/// the window's width in points, and the right edge of the rightmost word.
fn rows_at(px: Vec2, percent: f32, page: GuiPage, dev_section: &str) -> (Vec<String>, Vec<String>, f32, f32, Vec<f32>) {
    let theme = load_theme();
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);
    let mut state = GuiState::default();
    state.settings.font_size = percent;
    state.active_page = page;
    state.active_dev_section = dev_section.to_string();
    let mut out = None;
    for i in 0..3 {
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, px / ctx.pixels_per_point())), time: Some(i as f64 / 60.0), ..Default::default() };
        crate::gui::font_size::before_frame(&ctx, &mut input, &mut state.settings.font_size);
        out = Some(ctx.run(input, |ctx| draw_nav_bar(ctx, &theme, &mut state)));
    }
    let out = out.expect("ran");
    let all = texts(&out);
    let below = theme.nav_primary_height + theme.nav_secondary_height;
    let mut tops: Vec<f32> = Vec::new();
    for (_, r) in all.iter().filter(|(_, r)| r.top() < below) {
        if tops.iter().all(|t| (t - r.top()).abs() > 12.0) {
            tops.push(r.top());
        }
    }
    let right = all.iter().map(|(_, r)| r.right()).fold(0.0, f32::max);
    (primary(&out, &theme), secondary(&out, &theme), ctx.screen_rect().width(), right, tops)
}

/// MORE TAKES WHAT DOES NOT FIT, NEVER A THIRD ROW (section 4.1; BUG-225's sizes): on the
/// operator's 2560x1440 screen and in the app's restored 1280x720 window, at 100%, 150% and 200%,
/// the two rows' words sit on exactly two lines, nothing is drawn past the window's right edge,
/// and every category and every item of the longest rows (Dev's six, the own-life category's six)
/// is either in its row or under its More, in order. The counts are printed for the record
/// (`-- --nocapture`).
///
/// Seen red 2026-11-03 with `fitting` taking every button whatever the room (no More, the row
/// running on past the window, where its words are cut off): at 150% in 1280x720 "More is there
/// exactly when a category does not fit; left: true, right: false"; the same breakage turned
/// font_size.rs's `font_size_top_row_rows_at_each_size` red ("a word of the top row ends at 866.25,
/// past the window's 853.34375 points").
#[test]
fn more_takes_what_does_not_fit_never_a_third_row() {
    let menu = crate::gui::navigation::menu();
    let more = t("nav.more").to_string();
    for (px, percent) in [(TV, 100.0), (TV, 150.0), (TV, 200.0), (Vec2::new(1280.0, 720.0), 100.0), (Vec2::new(1280.0, 720.0), 150.0), (Vec2::new(1280.0, 720.0), 200.0)] {
        for (page, section, cat_id) in [(GuiPage::Dev, "dev", "dev"), (GuiPage::Homes, "dev", "own_life")] {
            let (top, second, width, right, tops) = rows_at(px, percent, page, section);
            let cat = category(&menu, cat_id);
            let cats_in_row = top.iter().filter(|w| menu.desktop.iter().any(|c| words_of(&c.button) == **w)).count();
            let items_in_row = second.iter().filter(|w| cat.items.iter().any(|b| words_of(b) == **w)).count();
            println!(
                "{}x{} px at {percent}% ({width:.0} points across), {cat_id}'s row: primary {cats_in_row} of {} categories{}, secondary {items_in_row} of {} pages{}, words on {} line(s)",
                px.x, px.y, menu.desktop.len(), if top.contains(&more) { " + More" } else { "" }, cat.items.len(), if second.contains(&more) { " + More" } else { "" }, tops.len()
            );
            assert_eq!(tops.len(), 2, "at {percent}% in {}x{} px the rows' words sit on two lines, never a third: {top:?} {second:?}", px.x, px.y);
            assert!(right <= width + 0.5, "at {percent}% in {}x{} px a word ends at {right:.0} points, past the window's {width:.0}", px.x, px.y);
            // In order, the first ones drawn; the rest under More.
            let want_top: Vec<String> = menu.desktop.iter().take(cats_in_row).map(|c| words_of(&c.button)).collect();
            // (Leaving out More, the Aa switch, and Humanity's icon, which is the letter H.)
            let words: Vec<String> = top.iter().filter(|w| **w != more && **w != NavDisplayMode::Both.glyph() && *w != "H").cloned().collect();
            assert_eq!(words, want_top, "the categories that fit, in order");
            assert_eq!(cats_in_row < menu.desktop.len(), top.contains(&more), "More is there exactly when a category does not fit");
            assert_eq!(items_in_row < cat.items.len(), second.contains(&more), "the second row's More is there exactly when a page does not fit");
        }
    }
}

/// WHAT MORE HOLDS: in a window too narrow for the primary row, More opens a list of the categories
/// that did not fit, and a click on one opens its page.
///
/// Seen red 2026-11-03 with the list drawn from the first categories instead of the rest: "More
/// lists the categories that do not fit; left: [\"Humanity\", \"Play\", \"Chat\", \"Life\",
/// \"Apps\"], right: [\"Life\", \"Apps\", \"Library\", \"Dev\", \"Settings\"]".
#[test]
fn more_lists_what_does_not_fit_and_opens_it() {
    let menu = crate::gui::navigation::menu();
    let mut r = Rows::new(Vec2::new(640.0, 400.0));
    let out = r.frame(Vec::new());
    let top = primary(&out, &r.theme);
    let shown = top.iter().filter(|w| menu.desktop.iter().any(|c| words_of(&c.button) == **w)).count();
    assert!(shown < menu.desktop.len(), "640 points is too narrow for all eight: {top:?}");
    r.click(at(&out, t("nav.more")));
    let out = r.frame(Vec::new());
    let listed: Vec<String> = texts(&out).into_iter().filter(|(_, rect)| rect.top() > r.theme.nav_primary_height).map(|(s, _)| s).filter(|s| menu.desktop.iter().any(|c| words_of(&c.button) == *s)).collect();
    let want: Vec<String> = menu.desktop.iter().skip(shown).map(|c| words_of(&c.button)).collect();
    assert_eq!(listed, want, "More lists the categories that do not fit");
    let last = menu.desktop.last().unwrap();
    let place = texts(&out).into_iter().filter(|(s, rect)| *s == words_of(&last.button) && rect.top() > r.theme.nav_primary_height).map(|(_, rect)| rect.center()).next().expect("the last category is listed");
    r.click(place);
    assert_eq!(r.state.active_page, last.button.page, "a click in More's list opens {}", last.id);
}

/// Where the hints showing in the last frame were drawn: each visible tooltip layer's rect, as egui
/// laid it out (its frame included).
fn hint_rects(ctx: &egui::Context) -> Vec<Rect> {
    ctx.memory(|m| m.areas().visible_layer_ids().into_iter().filter(|l| l.order == crate::gui::layers::TOOLTIPS).filter_map(|l| m.area_rect(l.id)).collect())
}

/// A HINT NEVER COVERS THE SECONDARY ROW (the operator, 2026-11-04, from a screenshot: "The tooltip
/// popup blocks the secondary top menu buttons."). With hints on, the pointer rests long enough for
/// a hint on every button of the secondary row (Chat's, on the Chat page) and then on every button
/// of the primary row: Back and Forward (with nowhere to go, so their hints are the disabled
/// kind), each category the window shows (its words, and Humanity's icon), More when there is one,
/// the Aa switch, the notices bell and help. Each time a hint comes up (read from egui's visible
/// tooltip layers, so the test cannot pass with no hint drawn), and its rect does not overlap the
/// secondary row's (the rect the row drew in). At the operator's 2560x1440 and the restored
/// 1280x720.
///
/// Seen red 2026-11-04 with egui's own placement (a hint just under its own button), on the
/// operator's case, Humanity's button (its icon, the letter H): "at 2560x1440 the hint on \"H\"
/// covers the secondary row: hint [[96.0 46.0] - [585.3 77.0]], row [[0.0 48.0] - [2560.0 88.0]]";
/// on a secondary button ("the hint on \"Watch\" ... hint [[96.0 87.0] - [343.3 118.0]]", a point
/// over the row's foot) and on Back ("hint [[8.0 46.0] - [177.3 77.0]]") the same way.
#[test]
fn a_hint_never_covers_the_secondary_row() {
    for size in [TV, Vec2::new(1280.0, 720.0)] {
        let mut r = Rows::new(size);
        r.hints = true;
        r.ctx.all_styles_mut(|s| {
            s.interaction.tooltip_delay = 0.3;
            s.interaction.show_tooltips_only_when_still = false;
        });
        let away = egui::pos2(size.x * 0.5, size.y - 40.0);
        r.move_to(away);
        let out = r.wait(0.5);
        let th = &r.theme;
        let (ph, sh) = (th.nav_primary_height, th.nav_secondary_height);
        // The secondary row's buttons first, while no preview has changed it.
        let mut spots: Vec<(String, egui::Pos2)> = texts(&out).into_iter().filter(|(_, rect)| rect.center().y > ph && rect.center().y < ph + sh).map(|(s, rect)| (s, rect.center())).collect();
        assert!(spots.len() >= 2, "the secondary row is drawn: {spots:?}");
        // Back and Forward (arrows painted, no words), every word of the primary row, and the
        // bell (an icon).
        let bh = th.nav_primary_button_height.min(ph);
        spots.push(("Back".into(), egui::pos2(8.0 + bh / 2.0, ph / 2.0)));
        spots.push(("Forward".into(), egui::pos2(8.0 + bh + 3.0 + bh / 2.0, ph / 2.0)));
        spots.extend(texts(&out).into_iter().filter(|(_, rect)| rect.center().y < ph).map(|(s, rect)| (s, rect.center())));
        let bell = crate::gui::notice_panel::BELL_SIZE;
        let bell_x = size.x - 8.0 - crate::gui::pages::keymap::HELP_TOGGLE_SIZE - crate::gui::notice_panel::BELL_GAP - bell / 2.0;
        spots.push(("the bell".into(), egui::pos2(bell_x, corner_y(th, bell) + bell / 2.0)));
        for (name, spot) in spots {
            r.move_to(away);
            r.wait(0.5);
            r.move_to(spot);
            r.wait(0.8);
            let tips = hint_rects(&r.ctx);
            assert!(!tips.is_empty(), "at {}x{} a rest on {name:?} brings up its hint", size.x, size.y);
            let row: Rect = r.ctx.data(|d| d.get_temp(Id::new(SECONDARY_RECT_ID))).expect("the secondary row's rect");
            for tip in tips {
                assert!(!tip.intersect(row).is_positive(), "at {}x{} the hint on {name:?} covers the secondary row: hint {tip:?}, row {row:?}", size.x, size.y);
            }
        }
    }
}

/// The preview's timing on its own (`step`), at the edges the rows' tests cross in between: a rest
/// shorter than `rest` previews nothing, resting on the docked category ends a preview, the
/// pointer back in the rows cancels the leave, and off is off.
///
/// Seen red 2026-11-03 with the rest asking a second more ("left: None, right: Some(1)") and with
/// the setting ignored ("off is off; left: Hover { resting: Some((1, 2.0)), preview: Some(1),
/// left_at: None }").
#[test]
fn the_preview_step_keeps_its_times() {
    let (rest, leave) = (0.2, 0.3);
    let h = step(Hover::default(), 0.0, Some(1), true, Some(2), true, rest, leave);
    assert_eq!(h.preview, None);
    let h = step(h, 0.19, Some(1), true, Some(2), true, rest, leave);
    assert_eq!(h.preview, None, "199 ms is not a rest");
    let h = step(h, 0.21, Some(1), true, Some(2), true, rest, leave);
    assert_eq!(h.preview, Some(1));
    let h = step(h, 0.3, None, false, Some(2), true, rest, leave);
    let h = step(h, 0.5, None, true, Some(2), true, rest, leave);
    let h = step(h, 0.9, None, true, Some(2), true, rest, leave);
    assert_eq!(h.preview, Some(1), "back in the rows before the leave time: the preview stays");
    let h = step(h, 1.0, Some(2), true, Some(2), true, rest, leave);
    let h = step(h, 1.25, Some(2), true, Some(2), true, rest, leave);
    assert_eq!(h.preview, None, "resting on the page's own category brings its row back");
    assert_eq!(step(Hover { preview: Some(1), ..Hover::default() }, 2.0, Some(1), true, Some(2), false, rest, leave), Hover::default(), "off is off");
}

/// THE SNAPSHOTS' PINNED PREVIEW IS A PREVIEW (ui_snapshots.rs `nav_rows_*_preview`): a picture has
/// no pointer to rest, so those pictures pin the preview with `preview_for_snapshot`; this holds
/// that the pinned row is the previewed category's, and the page's own category stays marked, so a
/// preview picture can never be the docked row under another name.
///
/// Seen red 2026-11-03 with the pin written to an id the rows do not read: "the pinned preview
/// shows Play's pages; left: [\"Watch\", \"Studio\", \"Files\", \"Guilds\"]".
#[test]
fn the_snapshots_pinned_preview_shows_the_previewed_row() {
    let menu = crate::gui::navigation::menu();
    let play: Vec<String> = category(&menu, "play").items.iter().map(words_of).collect();
    let mut r = Rows::new(TV);
    r.frame(Vec::new());
    preview_for_snapshot(&r.ctx, "play");
    let out = r.frame(Vec::new());
    assert_eq!(secondary(&out, &r.theme), play, "the pinned preview shows Play's pages");
    assert_eq!(r.state.active_page, GuiPage::Chat, "on the Chat page still");
}

/// THE TWO ROWS COST A SMALL PART OF A FRAME (docs/design/navigation.md section 13's budget: "anything
/// near 0.1 ms a frame is a finding"). The frame costs never see the menu (it draws only on frames
/// a page covers, which they discard), so it is timed here: 300 frames of the rows alone on the
/// operator's screen, the pointer resting on a category (the preview showing), the median and the
/// worst printed for the record (`-- --nocapture`). This is a test build (our code unoptimised,
/// egui optimised, Cargo.toml's dev profile), so the release exe is cheaper still. The bound is on
/// the FASTEST of the 300 frames, which other work on the machine (the other tests running beside
/// it, a build) can only slow, never speed: a guard against a blow-up, not the budget. Measured
/// 2026-11-03: fastest 0.31 ms, median 0.32 to 0.34, and with the menu file read and parsed every
/// frame (`navigation::load` in place of `menu()`) median 1.04 ms, which this bound catches.
///
/// Seen red 2026-11-03 with the file read every frame: "the rows took 1.024 ms on their
/// fastest frame of 300 (median 1.034): a blow-up".
#[test]
fn the_two_rows_cost_a_small_part_of_a_frame() {
    let menu = crate::gui::navigation::menu();
    let mut r = Rows::new(TV);
    let out = r.frame(Vec::new());
    r.move_to(at(&out, &t_dyn(&category(&menu, "play").button.label)));
    let mut ms: Vec<f64> = Vec::new();
    for _ in 0..300 {
        r.t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, r.size)), time: Some(r.t), ..Default::default() };
        let (theme, state) = (&mut r.theme, &mut r.state);
        let mut took = 0.0;
        let _ = r.ctx.run(input, |ctx| {
            let t0 = std::time::Instant::now();
            draw_nav_bar(ctx, theme, state);
            took = t0.elapsed().as_secs_f64() * 1000.0;
        });
        ms.push(took);
    }
    ms.sort_by(f64::total_cmp);
    let (best, median, worst) = (ms[0], ms[ms.len() / 2], ms[ms.len() - 1]);
    println!("the two rows, 2560x1440, previewing: fastest {best:.3} ms a frame, median {median:.3}, worst {worst:.3} (test build)");
    assert!(best < 0.6, "the rows took {best:.3} ms on their fastest frame of 300 (median {median:.3}): a blow-up");
}
