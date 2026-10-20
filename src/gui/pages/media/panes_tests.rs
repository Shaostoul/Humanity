//! Headless tests of the Media page's three panes (2026-10-19, panes.rs, list.rs, viewer.rs, and
//! the edit form in place, edit.rs): the page drawn into an egui context with no window and no
//! network (requests go to a closed port, or to a small server on the loopback address), clicked
//! the way a person clicks it, and what it drew read back. Each was seen to fail once against the
//! break its comment names, with the words it failed with.

use super::api::{Item, ItemFile};
use super::item_view::item_tests::{answer_of, click_at, every_exact, frame, page_on, rich_item, serve_routes, until};
use super::item_view::{self, Discard, CHOOSE_LINE, COPY_LINK, EDIT, GO_BACK, KEEP_EDITING, DISCARD_THEM};
use super::tests::{ctx_and_theme, settled, shows, SERVER};
use super::*;

/// A picture of the uploader's own (so it shows without a press), with its texture in the image
/// cache, put there by "me".
fn own_picture() -> Item {
    Item { id: 31, title: "Garden this morning".into(), kind: "picture".into(), url: "/uploads/31_garden.png".into(), stored: "31_garden.png".into(), original_name: "garden.png".into(), size_bytes: 9000, uploader_key: "me".into(), uploader_name: "Me".into(), visibility: "public".into(), category_name: "General".into(), ..Default::default() }
}

/// A small made-up picture (a gradient), as PNG bytes.
fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(64, 40, |x, y| image::Rgba([(x * 4) as u8, (y * 6) as u8, 150, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// The page with the picture and the model in, the picture's texture ready.
fn two_items(ctx: &egui::Context) -> GuiState {
    let mut state = page_on(SERVER, answer_of(vec![own_picture(), rich_item(("k2", "Ann"))]), "tok");
    let key = tiles::cache_key(&state, &own_picture());
    state.image_cache.insert_for_test(ctx, &key, &png()).expect("the picture decodes");
    state
}

/// Draw the page once in a screen of `size`.
fn frame_at(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size)), events, ..Default::default() };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

/// The rect the text exactly `text` is drawn in, the last drawn.
fn at(out: &egui::FullOutput, text: &str) -> egui::Rect {
    *every_exact(out, text).last().unwrap_or_else(|| panic!("{text:?} is drawn"))
}

/// Click the row titled `title` (its title's place).
fn press_row(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, title: &str) {
    let out = settled(ctx, theme, state);
    let rect = every_exact(&out, title).into_iter().find(|r| r.left() < 1280.0 * 0.35).unwrap_or_else(|| panic!("the row {title:?} is in the list"));
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(rect.center())]);
    click_at(ctx, theme, state, rect.center());
}

/// Click the topmost drawn text that is exactly `text`.
fn press(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = settled(ctx, theme, state);
    let rect = at(&out, text);
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(rect.center())]);
    click_at(ctx, theme, state, rect.center());
}

/// Everywhere texture `tex` is drawn: a textured mesh (a row's small picture) or a textured rect
/// (an `egui::Image`).
fn meshes_of(out: &egui::FullOutput, tex: egui::TextureId) -> Vec<egui::Rect> {
    fn walk(s: &egui::Shape, tex: egui::TextureId, found: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Mesh(m) if m.texture_id == tex => found.push(m.calc_bounds()),
            egui::Shape::Rect(r) if r.brush.as_ref().is_some_and(|b| b.fill_texture_id == tex) => found.push(r.rect),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, tex, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, tex, &mut found));
    found
}

/// THE PANES GO ACROSS A WIDE WINDOW AND STACK ON A TALL ONE (operator, 2026-10-19: "Left side is
/// the list of all of them, middle is the fit to area picture ... then right side is data/details/
/// info. We could do that top, middle bottom too."): on the test's 1280 by 1000 screen the list,
/// the picture and its details are left, middle and right; on a 900 by 1300 one the same three are
/// top, middle and bottom; a 2560 by 1387 screen (the operator's monitor) is three across.
///
/// Seen red 2026-10-19 with the page's arrangement fixed at Across (`panes::arrange_of` returning
/// it whatever the shape): "a tall window stacks them: the list above the picture" (the list's row
/// and the picture were side by side at the same height).
#[test]
fn the_panes_go_across_a_wide_window_and_stack_on_a_tall_one() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = two_items(&ctx);
    item_view::open(&mut state, &own_picture());
    let tex = state.image_cache.get_texture(&tiles::cache_key(&state, &own_picture())).expect("the texture").id();
    for (size, across) in [(egui::vec2(1280.0, 1000.0), true), (egui::vec2(2560.0, 1387.0), true), (egui::vec2(900.0, 1300.0), false)] {
        let mut out = frame_at(&ctx, &theme, &mut state, size, Vec::new());
        for _ in 0..3 {
            out = frame_at(&ctx, &theme, &mut state, size, Vec::new());
        }
        let row = every_exact(&out, "Dusk").first().copied().expect("the model's row");
        // The big picture: the largest mesh of its texture (the row's small one is the smaller).
        let picture = meshes_of(&out, tex).into_iter().max_by(|a, b| a.area().total_cmp(&b.area())).expect("the picture is drawn");
        let heading = every_exact(&out, "Garden this morning").into_iter().max_by(|a, b| a.height().total_cmp(&b.height())).expect("the details' heading");
        if across {
            assert!(row.right() < picture.left() && picture.right() < heading.left(), "{size:?}: list, picture, details left to right: {row:?} {picture:?} {heading:?}");
            assert!((picture.center().y - row.center().y).abs() < size.y * 0.5, "{size:?}: side by side");
        } else {
            assert!(row.bottom() < picture.top() && picture.bottom() < heading.top(), "a tall window stacks them: the list above the picture above the details: {row:?} {picture:?} {heading:?}");
        }
    }
}

/// PRESSING A ROW SHOWS THAT ITEM IN THE MIDDLE AND ON THE RIGHT (2026-10-19): with nothing
/// chosen, the middle pane says to choose; a press on the picture's row shows the picture fitted to
/// the middle pane, its title, who put it there and its counts on the right, and the row
/// highlighted; a press on the model's row then shows the model's card in the middle (its kind and
/// View in 3D) and its details on the right, the picture gone.
///
/// Seen red 2026-10-19 with a row's press not reported (`list::row` returning false): "the press
/// shows the picture's item" (left: None, right: Some(31)).
#[test]
fn pressing_a_row_shows_that_item_in_the_middle_and_on_the_right() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = two_items(&ctx);
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, CHOOSE_LINE), "nothing chosen yet: the middle pane says to choose");
    press_row(&ctx, &theme, &mut state, "Garden this morning");
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(31), "the press shows the picture's item");
    let out = settled(&ctx, &theme, &mut state);
    let tex = state.image_cache.get_texture(&tiles::cache_key(&state, &own_picture())).expect("the texture").id();
    let picture = meshes_of(&out, tex).into_iter().max_by(|a, b| a.area().total_cmp(&b.area())).expect("the picture is drawn");
    assert!(picture.left() > 1280.0 * 0.25 && picture.right() < 1280.0 * 0.75 && picture.width() > 300.0, "fitted to the middle pane: {picture:?}");
    for text in ["by Me in General", "0 picks, 0 views, 0 downloads", COPY_LINK] {
        let r = every_exact(&out, text).last().copied().unwrap_or_else(|| panic!("the details show {text:?}"));
        assert!(r.left() > 1280.0 * 0.6, "{text:?} on the right: {r:?}");
    }
    assert!(highlighted_row(&out, &theme), "the chosen row is highlighted");

    press_row(&ctx, &theme, &mut state, "Dusk");
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(7), "the model's row shows the model");
    let out = settled(&ctx, &theme, &mut state);
    assert!(meshes_of(&out, tex).iter().all(|m| m.width() < 120.0), "the picture is gone from the middle (its row keeps its small one)");
    let view = every_exact(&out, crate::gui::model_view::VIEW_IN_3D).into_iter().find(|r| r.left() > 1280.0 * 0.25 && r.right() < 1280.0 * 0.75).unwrap_or(egui::Rect::NOTHING);
    assert!(view.left() > 1280.0 * 0.25 && view.right() < 1280.0 * 0.75, "the model's card in the middle, with View in 3D: {view:?}");
    assert!(every_exact(&out, "by Ann in Fire flow arts").iter().any(|r| r.left() > 1280.0 * 0.6), "and its details on the right");
}

/// Whether a row is drawn with the chosen row's accent border.
fn highlighted_row(out: &egui::FullOutput, theme: &Theme) -> bool {
    fn walk(s: &egui::Shape, accent: egui::Color32) -> bool {
        match s {
            egui::Shape::Rect(r) => r.stroke.color == accent && r.stroke.width > 1.0 && r.rect.left() < 1280.0 * 0.35,
            egui::Shape::Vec(v) => v.iter().any(|x| walk(x, accent)),
            _ => false,
        }
    }
    out.shapes.iter().any(|c| walk(&c.shape, theme.accent()))
}

/// EDIT IN PLACE: SAVE RETURNS TO THE DETAILS, CANCEL LETS CHANGES GO ONLY WHEN ASKED, AND CHANGES
/// ARE ASKED ABOUT BEFORE ANOTHER ITEM OR ANOTHER PAGE (operator, 2026-10-19: "Once done clicking
/// save undoes the GUI changes for edit mode."). The form is in the right pane with its three parts
/// and the pane's moving border; a changed title saved goes to the server and, once answered, the
/// pane is the details again with the new title; Cancel after a change asks (Keep editing keeps
/// the change, Discard the changes lets it go); a press on another row with a change asks and shows
/// nothing else until answered; and leaving the page with a change asks over the next page, Go back
/// to them returning to the Media page with the change still in the form.
///
/// Seen red 2026-10-19 three ways: with the form kept after the answer (`on_answer` not clearing
/// `edit`), "Save returns to the details" (the form was still open); with `EditForm::changed` always
/// false, "Cancel after a change asks" (the form closed, the change gone); and with the leave
/// question left out of `draw_item_window`, "leaving the page asks" (no "Unsaved changes").
#[test]
fn edit_in_place_saves_cancels_and_asks_before_changes_are_lost() {
    use crate::gui::widgets::tint_section::{painted, Tint};
    let saved = r#"{"ok":true,"item":{"id":7,"title":"Dusk, edited","url":"/uploads/7_dusk.glb","filename":"7_dusk.glb","kind":"model","visibility":"public","uploader_key":"me","uploader_name":"Me"}}"#;
    let (base, seen) = serve_routes(vec![("/api/media/edit", saved)]);
    let (ctx, theme) = ctx_and_theme();
    let mine = rich_item(("me", "Me"));
    let mut state = page_on(&base, answer_of(vec![mine.clone(), own_picture()]), "tok");
    item_view::open(&mut state, &mine);
    press(&ctx, &theme, &mut state, EDIT);
    let out = settled(&ctx, &theme, &mut state);
    assert!(painted(&out, &theme, Tint::Red) && painted(&out, &theme, Tint::Green), "the form's coloured parts");
    let editing = at(&out, &edit::editing_words("Dusk"));
    assert!(editing.left() > 1280.0 * 0.6, "in the right pane: {editing:?}");

    // Save: sent, and once answered the details again, with the new title.
    state.media.item.as_mut().and_then(|p| p.edit.as_mut()).unwrap().title = "Dusk, edited".into();
    press(&ctx, &theme, &mut state, edit::SAVE);
    until(&ctx, &theme, &mut state, "Save returns to the details", |s| s.media.item.as_ref().is_some_and(|p| p.edit.is_none()));
    assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("POST /api/media/edit ") && r.contains("Dusk, edited")), "the edit went");
    let out = settled(&ctx, &theme, &mut state);
    assert!(shows(&out, COPY_LINK) && shows(&out, item_view::SAVED), "the details again, saying Saved.");
    assert!(every_exact(&out, "Dusk, edited").iter().any(|r| r.left() > 1280.0 * 0.6), "with the new title");

    // Cancel after a change asks; Keep editing keeps it; Discard lets it go.
    press(&ctx, &theme, &mut state, EDIT);
    state.media.item.as_mut().and_then(|p| p.edit.as_mut()).unwrap().description = "Changed and not saved".into();
    press(&ctx, &theme, &mut state, "Cancel");
    assert_eq!(state.media.item.as_ref().and_then(|p| p.confirm_discard.clone()), Some(Discard::Cancel), "Cancel after a change asks");
    assert!(shows(&settled(&ctx, &theme, &mut state), item_view::DISCARD_TITLE), "in a small question");
    press(&ctx, &theme, &mut state, KEEP_EDITING);
    assert_eq!(state.media.item.as_ref().and_then(|p| p.edit.as_ref()).map(|f| f.description.as_str()), Some("Changed and not saved"), "Keep editing keeps the change");
    press(&ctx, &theme, &mut state, "Cancel");
    press(&ctx, &theme, &mut state, DISCARD_THEM);
    assert!(state.media.item.as_ref().is_some_and(|p| p.edit.is_none() && p.id == 7), "Discard the changes lets them go, the same item shown");

    // Another row while a change is unsaved: asked first, nothing else shown until answered.
    press(&ctx, &theme, &mut state, EDIT);
    state.media.item.as_mut().and_then(|p| p.edit.as_mut()).unwrap().title = "Half typed".into();
    press_row(&ctx, &theme, &mut state, "Garden this morning");
    assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(7), "another row with a change waits");
    assert!(matches!(state.media.item.as_ref().and_then(|p| p.confirm_discard.clone()), Some(Discard::Open(ref i)) if i.id == 31), "and asks");
    press(&ctx, &theme, &mut state, KEEP_EDITING);
    assert!(state.media.item.as_ref().is_some_and(|p| p.id == 7 && p.edit.as_ref().is_some_and(|f| f.title == "Half typed")), "Keep editing stays");

    // Leaving the page: asked over the next page; Go back returns with the change still there.
    state.active_page = crate::gui::GuiPage::Chat;
    let away = |state: &mut GuiState, events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), events, ..Default::default() };
        ctx.run(input, |ctx| draw_item_window(ctx, &theme, state))
    };
    let mut out = away(&mut state, Vec::new());
    for _ in 0..3 {
        out = away(&mut state, Vec::new());
    }
    assert!(shows(&out, item_view::DISCARD_TITLE) && shows(&out, &item_view::discard_words("Dusk, edited")), "leaving the page asks");
    let go = at(&out, GO_BACK).center();
    away(&mut state, vec![egui::Event::PointerMoved(go)]);
    away(&mut state, vec![egui::Event::PointerButton { pos: go, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
    away(&mut state, vec![egui::Event::PointerButton { pos: go, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
    assert_eq!(state.active_page, crate::gui::GuiPage::Media, "Go back to them returns to the Files page");
    assert!(state.media.item.as_ref().is_some_and(|p| p.edit.as_ref().is_some_and(|f| f.title == "Half typed")), "with the change still in the form");
}

/// THE SPLIT IS KEPT: a drag on the handle between the list and the middle pane widens the list,
/// the new split is in the settings, the release asks for them to be saved, and the config keeps it
/// through a save and a load (a split that is not usable loads as the default).
///
/// Seen red 2026-10-19 with the load leg left out of config.rs (`media_layout` not applied): "the
/// config keeps the split" (left: [0.28, 0.42, 0.3]).
#[test]
fn the_split_is_kept() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = two_items(&ctx);
    settled(&ctx, &theme, &mut state);
    let before = state.settings.media_layout.across.clone();
    // The handle: the gap after the list pane (its card's right edge, then the spacing).
    let total = 1280.0 - 2.0 * theme.panel_margin - 2.0 * theme.spacing_md;
    let x = theme.panel_margin + before[0] * total + theme.spacing_md / 2.0;
    let y = 600.0;
    let m = egui::Modifiers::default();
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(egui::pos2(x, y))]);
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: egui::pos2(x, y), button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    for step in 1..=10 {
        frame(&ctx, &theme, &mut state, vec![egui::Event::PointerMoved(egui::pos2(x + step as f32 * 10.0, y))]);
    }
    state.settings_dirty = false;
    frame(&ctx, &theme, &mut state, vec![egui::Event::PointerButton { pos: egui::pos2(x + 100.0, y), button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    let after = state.settings.media_layout.across.clone();
    assert!(after[0] > before[0] + 0.03, "the list is wider: {before:?} then {after:?}");
    assert!(state.settings_dirty, "the release asks for the settings to be saved");

    let config = crate::config::AppConfig::from_gui_state(&state);
    let mut loaded = GuiState::default();
    config.apply_to_gui_state(&mut loaded);
    assert_eq!(loaded.settings.media_layout.across, after, "the config keeps the split");
    let mut broken = config.clone();
    broken.media_layout.across = vec![f32::NAN, 1.0];
    broken.apply_to_gui_state(&mut loaded);
    assert_eq!(loaded.settings.media_layout.across, panes::ACROSS.to_vec(), "a split that is not usable loads as the default");
}

/// A SOUND IS SHOWN AS ONE, WITH PLAY HERE (2026-10-19): an item whose own file is a song shows
/// "Play here" in the middle pane and its name and size, and nothing is fetched before the press
/// (the waveform's own tests are widgets/sound_view.rs's).
///
/// Seen red 2026-10-19 with `viewer::is_sound` answering no to every file: "an .mp3 is a sound".
#[test]
fn a_sound_is_shown_as_one_with_play_here() {
    let (ctx, theme) = ctx_and_theme();
    let song = Item {
        id: 41,
        title: "Fire circle drums".into(),
        kind: "file".into(),
        url: "/uploads/41_drums.mp3".into(),
        stored: "41_drums.mp3".into(),
        original_name: "drums.mp3".into(),
        size_bytes: 4_200_000,
        uploader_key: "k2".into(),
        uploader_name: "Ann".into(),
        visibility: "public".into(),
        files: vec![ItemFile { file: 0, filename: "41_drums.mp3".into(), original_name: "drums.mp3".into(), kind: "file".into(), size_bytes: 4_200_000, url: "/uploads/41_drums.mp3".into() }],
        ..Default::default()
    };
    assert!(viewer::is_sound(&song), "an .mp3 is a sound");
    assert!(!viewer::is_sound(&rich_item(("k2", "Ann"))), "a .glb is not");
    let mut state = page_on(SERVER, answer_of(vec![song.clone()]), "tok");
    item_view::open(&mut state, &song);
    let out = settled(&ctx, &theme, &mut state);
    let play = at(&out, crate::gui::widgets::sound_view::PLAY_HERE);
    assert!(play.left() > 1280.0 * 0.25 && play.right() < 1280.0 * 0.75, "Play here for a sound, in the middle: {play:?}");
    assert!(shows(&out, "drums.mp3, 4.2 MB") || shows(&out, "drums.mp3, 4 MB"), "with its name and size");
    assert!(state.media.open.as_ref().is_some_and(|o| !o.sound.started()), "nothing fetched before the press");
}

/// AN ITEM FROM A CHAT CARD OPENS IN A WINDOW WITH THE SAME TWO PARTS (2026-10-19): the item to fit
/// on the left, its details on the right, in one window over the chat (the person stays in the
/// conversation), and closing it with nothing unsaved lets it go.
///
/// Seen red 2026-10-19 with the window's two panes drawn the other way round (details first): "the
/// picture left of its details".
#[test]
fn an_item_from_a_chat_card_opens_in_a_window_with_the_same_two_parts() {
    let (ctx, theme) = ctx_and_theme();
    let mut state = two_items(&ctx);
    state.active_page = crate::gui::GuiPage::Chat;
    item_view::open_in_window(&mut state, &own_picture());
    let tex = state.image_cache.get_texture(&tiles::cache_key(&state, &own_picture())).expect("the texture").id();
    let run = |state: &mut GuiState| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1000.0))), ..Default::default() };
        ctx.run(input, |ctx| draw_item_window(ctx, &theme, state))
    };
    let mut out = run(&mut state);
    for _ in 0..3 {
        out = run(&mut state);
    }
    let picture = meshes_of(&out, tex).into_iter().max_by(|a, b| a.area().total_cmp(&b.area())).expect("the picture in the window");
    let copy = at(&out, COPY_LINK);
    assert!(picture.right() < copy.left(), "the picture left of its details: {picture:?} {copy:?}");
    assert!(state.media.item.as_ref().is_some_and(|p| p.window), "in its window");
}
