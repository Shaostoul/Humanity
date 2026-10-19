// Child of widgets/file_views.rs (#[path]): the picker's List, Details and Pictures views, drawn
// headless over a real folder. Each test was seen failing once on purpose, recorded at the test.

use super::*;
use crate::gui::widgets::file_browser::{file_picker_modal, list_dir, FilePickerResult, FilePickerState};
use crate::gui::widgets::file_picker_memory as memory;

/// A folder holding a sub-folder, three pictures and a text file, written for the test.
fn folder(tag: &str) -> crate::test_temp::TempPath {
    let dir = crate::test_temp::dir(tag);
    std::fs::create_dir_all(dir.join("trips")).unwrap();
    for (i, name) in ["a-dawn.png", "b-noon.png", "c-dusk.png"].iter().enumerate() {
        image::RgbaImage::from_fn(60 + i as u32 * 20, 40, |x, _| image::Rgba([(x * 3) as u8, 80, 140, 255])).save(dir.join(name)).unwrap();
    }
    std::fs::write(dir.join("notes.txt"), b"a few words, and a few more").unwrap();
    dir
}

/// Every text one frame drew, with where.
fn texts(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::Shape, t: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(x) => t.push((x.galley.job.text.clone(), x.galley.rect.translate(x.pos.to_vec2()))),
            egui::Shape::Vec(v) => v.iter().for_each(|y| walk(y, t)),
            _ => {}
        }
    }
    let mut t = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut t));
    t
}

/// Draw the picker over `dir` a few frames (the previews are read on workers meanwhile).
fn picker_frames(ctx: &egui::Context, state: &mut FilePickerState, events: Vec<egui::Event>) -> (egui::FullOutput, FilePickerResult) {
    picker_frames_on(ctx, state, events, 900.0)
}

/// [`picker_frames`] on a screen `height` points tall (the list is as tall as the screen allows).
fn picker_frames_on(ctx: &egui::Context, state: &mut FilePickerState, events: Vec<egui::Event>, height: f32) -> (egui::FullOutput, FilePickerResult) {
    let theme = crate::gui::theme::load_theme();
    let input = |events: Vec<egui::Event>| egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, height))), events, ..Default::default() };
    let mut result = FilePickerResult::Open;
    let out = ctx.run(input(events), |ctx| result = file_picker_modal(ctx, &theme, state, "Choose"));
    (out, result)
}

/// DETAILS SORTS BY A HEADING, FOLDERS ALWAYS FIRST, AND SAYS WHEN A FILE CHANGED: by name A to Z,
/// by size largest first when reversed, the folder above the files whatever the sort; the date as
/// "YYYY-MM-DD HH:MM UTC". Seen red 2026-10-17 with the folders sorted among the files: the very
/// first list came back with "trips" after the pictures.
#[test]
fn details_sorts_by_a_heading_folders_first() {
    let dir = folder("views_sort");
    let entries = list_dir(&dir, &[]);
    let names = |v: Vec<FsEntry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(names(sorted(&entries, SortBy::Name, false)), vec!["trips", "a-dawn.png", "b-noon.png", "c-dusk.png", "notes.txt"]);
    let by_size = names(sorted(&entries, SortBy::Size, true));
    assert_eq!(by_size[0], "trips", "the folder stays first");
    let sizes: Vec<u64> = sorted(&entries, SortBy::Size, true).iter().skip(1).map(|e| e.size).collect();
    assert!(sizes.windows(2).all(|w| w[0] >= w[1]), "largest first, reversed: {sizes:?}");
    assert_eq!(names(sorted(&entries, SortBy::Kind, false))[1..4].to_vec(), vec!["a-dawn.png", "b-noon.png", "c-dusk.png"], "the pictures together, then the text file");
    assert_eq!(changed_words(Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_272_600))), "2026-10-17 21:30 UTC");
    assert_eq!(changed_words(None), "");
}

/// PICTURES SHOWS THEM ACROSS AS ASKED, EACH WITH ITS TYPE, AND REMEMBERS IT (operator, 2026-10-17:
/// "1 image at a time ... 2 could split it evenly so there's like 2 columns. Then 3 for 3 columns"):
/// two across puts the first two side by side and the third under them, at half the width; one across
/// gives each the whole width; every cell shows its type's badge; the choice is kept for the next
/// picker. Seen red 2026-10-17 with the slider's number ignored (always four across): "two across:
/// the third starts a new row".
#[test]
fn pictures_shows_them_across_as_asked_each_with_its_type() {
    let dir = folder("views_grid");
    memory::with(|m| {
        m.view = memory::PickerView::Pictures;
        m.across = Some(2);
    });
    let ctx = egui::Context::default();
    let mut state = FilePickerState::new(&[], 0).starting_in(Some(dir.to_path_buf()));
    let mut out = None;
    for _ in 0..4 {
        out = Some(picker_frames(&ctx, &mut state, Vec::new()).0);
    }
    let t = texts(out.as_ref().unwrap());
    let at = |name: &str| t.iter().find(|(s, _)| s == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("{name} is drawn: {:?}", t.iter().map(|x| &x.0).collect::<Vec<_>>()));
    // Folders first: trips, a-dawn on the first row; b-noon, c-dusk on the second; notes.txt third.
    let (trips, dawn, noon) = (at("trips"), at("a-dawn.png"), at("b-noon.png"));
    assert!((trips.top() - dawn.top()).abs() < 2.0 && dawn.left() > trips.left(), "two across: the first two side by side");
    assert!(noon.top() > dawn.top() + 40.0, "two across: the third starts a new row");
    assert!(t.iter().filter(|(s, _)| s == "Picture").count() >= 3 && t.iter().any(|(s, _)| s == "Folder"), "each cell on screen shows its type");
    assert!(!t.iter().any(|(s, _)| s == "notes.txt"), "a row below the visible part is not drawn (nor its preview read)");
    let half = dawn.left() - trips.left();

    memory::with(|m| m.across = Some(1));
    // A tall screen, so the list shows more than the one picture a 900-point screen has room for.
    for _ in 0..3 {
        out = Some(picker_frames_on(&ctx, &mut state, Vec::new(), 1500.0).0);
    }
    let t = texts(out.as_ref().unwrap());
    let one = |name: &str| t.iter().find(|(s, _)| s == name).map(|(_, r)| *r);
    // One across, each cell is the whole width: the second cell (a picture) starts under the first's
    // name, its badge at the same left edge. (Its own name is below the visible part of the list.)
    let trips = one("trips").expect("the folder is drawn");
    let picture = t.iter().find(|(s, _)| s == "Picture").map(|(_, r)| *r).expect("the next cell, a picture, is drawn");
    let folder_badge = t.iter().find(|(s, _)| s == "Folder").map(|(_, r)| *r).expect("the folder's badge");
    assert!(picture.top() > trips.top() && (picture.left() - folder_badge.left()).abs() < 2.0 && half > 100.0, "one across: one under the other, each the whole width");
    assert!(!t.iter().any(|(s, _)| s == "b-noon.png"), "one across: the third is further down, not beside");
    assert_eq!(memory::with(|m| (m.view, m.across)), (memory::PickerView::Pictures, Some(1)), "remembered for the next picker");
}

/// IN PICTURES, A CLICK CHOOSES, A DOUBLE-CLICK TAKES, A FOLDER OPENS: in a picker for one, a click
/// on a picture makes it the one chosen and a double-click picks it at once; a click on the folder
/// opens it; in a picker for several a click ticks. Seen red 2026-10-17 with a cell's click ignored:
/// "a click on a picture chooses it".
#[test]
fn in_pictures_a_click_chooses_and_a_double_click_takes() {
    let dir = folder("views_click");
    memory::with(|m| {
        m.view = memory::PickerView::Pictures;
        m.across = Some(3);
    });
    let ctx = egui::Context::default();
    let mut state = FilePickerState::new(&[], 0).starting_in(Some(dir.to_path_buf()));
    let mut out = None;
    for _ in 0..4 {
        out = Some(picker_frames(&ctx, &mut state, Vec::new()).0);
    }
    let t = texts(out.as_ref().unwrap());
    let dawn = t.iter().find(|(s, _)| s == "a-dawn.png").map(|(_, r)| *r).expect("drawn");
    // The cell's preview sits above its name.
    let target = egui::pos2(dawn.center().x, dawn.top() - 20.0);
    let m = egui::Modifiers::default();
    let press = |pressed| egui::Event::PointerButton { pos: target, button: egui::PointerButton::Primary, pressed, modifiers: m };
    picker_frames(&ctx, &mut state, vec![egui::Event::PointerMoved(target)]);
    picker_frames(&ctx, &mut state, vec![egui::Event::PointerMoved(target)]);
    picker_frames(&ctx, &mut state, vec![press(true)]);
    picker_frames(&ctx, &mut state, vec![press(false)]);
    assert_eq!(state.selected.as_ref().map(|e| e.name.as_str()), Some("a-dawn.png"), "a click on a picture chooses it");
    picker_frames(&ctx, &mut state, vec![press(true)]);
    let (_, result) = picker_frames(&ctx, &mut state, vec![press(false)]);
    match result {
        FilePickerResult::Picked(paths) => assert!(paths.len() == 1 && paths[0].ends_with("a-dawn.png"), "{paths:?}"),
        other => panic!("a double-click picks it at once: {other:?}"),
    }

    // The same rules, said by the views to the picker.
    let entries = list_dir(&dir, &[]);
    let (trips, noon) = (&entries[0], &entries[2]);
    let look = Look { selected: None, chosen: &[], multiple: true, dir_mode: false, max_size: 0, notes: None };
    assert_eq!(look.click(trips), Some(Act::Open(trips.path.clone())), "a folder opens");
    assert_eq!(look.click(noon), Some(Act::Toggle(noon.clone())), "in a picker for several, a click ticks");
    let small = Look { selected: None, chosen: &[], multiple: false, dir_mode: false, max_size: 10, notes: None };
    assert_eq!(small.click(noon), None, "a file over the size limit cannot be chosen");
}
