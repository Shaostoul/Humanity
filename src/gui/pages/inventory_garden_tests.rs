//! The Inventory page's Garden section costs what is on screen, not what exists (2026-10-31,
//! docs/design/sim-frame-cost.md "After the two fixes"; the technique: gui/widgets/offscreen.rs).
//!
//! Drawn for real, headless, the way the page draws it (a central panel, a vertical scroll area,
//! the section inside), at a normal window size, with a synthetic garden of thousands of crops in
//! many groups. The section counts the slot tiles it lays out (`note_tile`, test builds only).

use super::{draw_garden_section, GardenActions};
use crate::gui::theme::Theme;
use crate::gui::widgets::offscreen;
use crate::gui::{GuiCrop, GuiState};
use std::cell::Cell;

thread_local! {
    /// Slot tiles laid out since the last `take_tiles`.
    static TILES: Cell<usize> = const { Cell::new(0) };
    /// The last (group, slot) laid out, by order.
    static LAST: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

/// Called by the section for every slot tile it lays out (test builds only).
pub(super) fn note_tile(group: usize, slot: usize) {
    TILES.with(|c| c.set(c.get() + 1));
    LAST.with(|c| {
        if c.get().map_or(true, |l| (group, slot) > l) {
            c.set(Some((group, slot)));
        }
    });
}

/// The slot tiles laid out since the last call, and the last one laid out (also read by the
/// Garden snapshot in gui/ui_snapshots.rs, through `inventory::test_take_tiles_laid_out`).
pub(super) fn take_tiles() -> (usize, Option<(usize, usize)>) {
    (TILES.with(|c| c.replace(0)), LAST.with(|c| c.replace(None)))
}

/// A normal window: what the page shows on a 1280 x 800 screen.
const SCREEN: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// A garden of `crops` crops spread over `groups` machines ("ntower_0" ...), the way the showcase
/// tags each crop with the tower or bed it grows in. The tiles differ the ways that change their
/// height: a crop between picks reads two lines ("bearing / next 1.5d"), a crop that needs water
/// carries an outlook line under its bar, a ripe one reads "ready" with none.
fn garden(crops: usize, groups: usize) -> GuiState {
    let mut s = GuiState::default();
    s.trees_start_collapsed = false;
    let stages = ["seedling", "vegetative", "flowering", "fruiting", "ripe"];
    let names = ["Lettuce", "Basil", "Tomato", "Wheat", "Kale", "Pepper", "Bean"];
    for i in 0..crops {
        let bits = 1000 + i as u64;
        let stage = stages[i % stages.len()];
        let mature = i % 11 == 0;
        let bearing = (i % 13 == 3).then_some(1.5 + (i % 4) as f64);
        s.crops.push(GuiCrop {
            entity_bits: bits,
            name: names[i % names.len()].to_string(),
            stage: stage.to_string(),
            progress: (i % 5) as f32 / 4.0,
            water: 60.0,
            health: 90.0,
            mature,
            tower_id: Some(format!("ntower_{}", i % groups)),
            picking: crate::systems::farming::picking::PickRow {
                card: if bearing.is_some() { "picked 2 times".into() } else { String::new() },
                bearing_next_in: bearing,
            },
            ..Default::default()
        });
        let tile = match i % 3 {
            0 => "needs water",
            1 => "45 days",
            _ => "",
        };
        s.garden_pests.outlook.insert(
            bits,
            crate::systems::farming::outlook::CropOutlook { tile: tile.into(), ripens: "45 days".into(), ..Default::default() },
        );
    }
    s
}

/// One drawn frame: the section in a scroll area, as on the page.
struct Frame {
    /// Slot tiles laid out this frame.
    tiles: usize,
    /// The last (group, slot) laid out.
    last: Option<(usize, usize)>,
    /// The scroll area's content height and its visible height.
    content_h: f32,
    view_h: f32,
    /// The section's own draw, in ms (what `cpu.garden.page` times in the game).
    section_ms: f64,
    output: egui::FullOutput,
    actions: GardenActions,
}

fn ctx_for(theme: &Theme) -> egui::Context {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    theme.apply_to_egui(&ctx);
    ctx
}

fn draw(ctx: &egui::Context, theme: &Theme, state: &GuiState, scroll: f32, force: Option<bool>, events: Vec<egui::Event>) -> Frame {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let mut actions = GardenActions::default();
    let mut sizes = (0.0, 0.0);
    let mut section_ms = 0.0;
    take_tiles();
    let output = ctx.run(input, |ctx| {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
            .show(ctx, |ui| {
                let out = egui::ScrollArea::vertical().vertical_scroll_offset(scroll).show(ui, |ui| {
                    let t0 = std::time::Instant::now();
                    draw_garden_section(ui, theme, state, force, !state.trees_start_collapsed, &mut actions);
                    section_ms = t0.elapsed().as_secs_f64() * 1000.0;
                });
                sizes = (out.content_size.y, out.inner_rect.height());
            });
    });
    let (tiles, last) = take_tiles();
    Frame { tiles, last, content_h: sizes.0, view_h: sizes.1, section_ms, output, actions }
}

/// Warm a context up (the first sight of each group lays it out), then draw at `scroll`.
fn settled(ctx: &egui::Context, theme: &Theme, state: &GuiState, scroll: f32) -> Frame {
    for _ in 0..2 {
        draw(ctx, theme, state, scroll, None, Vec::new());
    }
    draw(ctx, theme, state, scroll, None, Vec::new())
}

/// The text shapes egui would put on screen, with their rects: what a person sees of the section.
fn visible_texts(output: &egui::FullOutput) -> Vec<(String, [i32; 4])> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, [i32; 4])>) {
        match shape {
            egui::Shape::Text(t) => {
                let r = t.galley.rect.translate(t.pos.to_vec2());
                if r.intersects(clip) {
                    let q = |v: f32| (v * 4.0).round() as i32;
                    out.push((t.galley.text().to_string(), [q(r.min.x), q(r.min.y), q(r.max.x), q(r.max.y)]));
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &output.shapes {
        walk(&cs.shape, cs.clip_rect, &mut out);
    }
    out
}

/// Where a drawn text is on screen (the dev IPC's own lookup, gui/screen_surface.rs).
fn find(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    crate::gui::screen_surface::find_text_in_shapes(&output.shapes, text).map(|f| f.rect)
}

fn click(at: egui::Pos2) -> [Vec<egui::Event>; 2] {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    [vec![egui::Event::PointerMoved(at), press(true)], vec![press(false)]]
}

/// THE BOUND. Scrolled to the middle of a big garden, the section lays out the tiles on screen and
/// a few rows either side, however many crops the garden holds: 2,000 crops in 90 groups (about the
/// showcase), five times the crops in five times the groups, and five times the crops in the same
/// 90 groups (bigger towers).
///
/// Seen red 2026-10-31 against the section as it was (every tile of every open group laid out every
/// frame): "2000 crops in 90 groups: 2000 slot tiles laid out in one frame scrolled to the middle;
/// a 1280 x 800 window shows about 150 of them, so at most 400 may be laid out".
#[test]
fn the_garden_lays_out_only_the_rows_on_screen() {
    offscreen::set_test_override(Some(true));
    let theme = crate::gui::theme::load_theme();
    for (crops, groups) in [(2000, 90), (10_000, 450), (10_000, 90)] {
        let state = garden(crops, groups);
        let ctx = ctx_for(&theme);
        let first = settled(&ctx, &theme, &state, 0.0);
        let mid = (first.content_h - first.view_h) / 2.0;
        let f = settled(&ctx, &theme, &state, mid);
        assert!(
            f.tiles <= 400,
            "{crops} crops in {groups} groups: {} slot tiles laid out in one frame scrolled to the middle; \
             a 1280 x 800 window shows about 150 of them, so at most 400 may be laid out",
            f.tiles
        );
        assert!(f.tiles >= 50, "{crops} crops in {groups} groups: only {} tiles laid out mid-garden; the screen is full of them", f.tiles);
    }
    offscreen::set_test_override(None);
}

/// Skipping changes nothing a person can see or scroll to: the content height (the scroll bar's
/// length) matches the page laid out whole, within a pixel; mid-garden every text is drawn in the
/// same place; scrolled to the bottom, the last group's last tile is laid out and its title shown.
#[test]
fn skipping_keeps_the_scroll_extent_and_the_picture() {
    let theme = crate::gui::theme::load_theme();
    let state = garden(2000, 90);

    offscreen::set_test_override(Some(false));
    let whole_ctx = ctx_for(&theme);
    let whole = settled(&whole_ctx, &theme, &state, 0.0);
    assert_eq!(whole.tiles, 2000, "with skipping off every tile is laid out, as before");
    let mid = ((whole.content_h - whole.view_h) / 2.0).round();
    let whole_mid = settled(&whole_ctx, &theme, &state, mid);

    offscreen::set_test_override(Some(true));
    let skip_ctx = ctx_for(&theme);
    let skip_top = settled(&skip_ctx, &theme, &state, 0.0);
    assert!(
        (skip_top.content_h - whole.content_h).abs() <= 1.0,
        "the content height with skipping ({}) differs from the whole page's ({})",
        skip_top.content_h,
        whole.content_h
    );
    let skip_mid = settled(&skip_ctx, &theme, &state, mid);
    assert!(skip_mid.tiles < whole_mid.tiles, "mid-garden some tiles were skipped ({} of {})", skip_mid.tiles, whole_mid.tiles);
    assert!((skip_mid.content_h - whole_mid.content_h).abs() <= 1.0, "mid-garden content height {} vs {}", skip_mid.content_h, whole_mid.content_h);
    let (a, b) = (visible_texts(&whole_mid.output), visible_texts(&skip_mid.output));
    assert!(!a.is_empty(), "the middle of the garden shows text");
    assert_eq!(a, b, "mid-garden the visible texts and their places differ with skipping");

    // Scrolled to the bottom: the last group (ntower_89, titled by its id) and its last tile.
    let bottom = skip_mid.content_h - skip_mid.view_h;
    let end = settled(&skip_ctx, &theme, &state, bottom);
    let last_group = 89;
    let last_slot = 2000 / 90 - 1; // 2000 = 90 x 22 + 20: groups 0..20 hold 23, the rest 22
    assert_eq!(end.last, Some((last_group, last_slot)), "scrolled to the bottom, the last group's last tile is laid out");
    assert!(find(&end.output, "ntower_89").is_some(), "the last group's title is on screen at the bottom");
    offscreen::set_test_override(None);
}

/// Collapse all reaches the groups off screen too (their open state is applied and kept while they
/// are skipped), and the page afterwards is the height of the same page laid out whole.
#[test]
fn collapse_all_reaches_groups_off_screen() {
    let theme = crate::gui::theme::load_theme();
    let state = garden(2000, 90);
    let mut heights = Vec::new();
    for skip in [false, true] {
        offscreen::set_test_override(Some(skip));
        let ctx = ctx_for(&theme);
        let first = settled(&ctx, &theme, &state, 0.0);
        let mid = ((first.content_h - first.view_h) / 2.0).round();
        settled(&ctx, &theme, &state, mid);
        // The click on "Collapse all" (it is up the page; the section is handed `force`), then
        // enough frames for the groups on screen to finish closing (egui animates it, 1/12 s).
        draw(&ctx, &theme, &state, mid, Some(false), Vec::new());
        for _ in 0..10 {
            draw(&ctx, &theme, &state, mid, None, Vec::new());
        }
        let after = settled(&ctx, &theme, &state, 0.0);
        assert_eq!(after.tiles, 0, "skip={skip}: every group is closed, so no tile is laid out");
        heights.push(after.content_h);
    }
    assert!((heights[0] - heights[1]).abs() <= 1.0, "collapsed heights differ: whole {} vs skipping {}", heights[0], heights[1]);
    offscreen::set_test_override(None);
}

/// The buttons still work mid-garden with rows skipped around them: a group's "Harvest N ready"
/// asks for exactly that group's ripe crops, and a slot tile clicked opens its card.
#[test]
fn buttons_work_mid_garden() {
    offscreen::set_test_override(Some(true));
    let theme = crate::gui::theme::load_theme();
    let state = garden(2000, 90);
    let ctx = ctx_for(&theme);
    let first = settled(&ctx, &theme, &state, 0.0);
    let mid = ((first.content_h - first.view_h) / 2.0).round();
    let f = settled(&ctx, &theme, &state, mid);

    // A "Harvest N ready" button on screen, and the group it belongs to.
    let harvest = crate::gui::screen_surface::find_text_in_shapes(&f.output.shapes, "Harvest").expect("a Harvest button on screen");
    let [down, up] = click(harvest.rect.center());
    draw(&ctx, &theme, &state, mid, None, down);
    let clicked = draw(&ctx, &theme, &state, mid, None, up);
    let bits = clicked.actions.harvest_many.expect("the Harvest click asked for a bulk harvest");
    let n: usize = harvest.text.trim_start_matches("Harvest ").trim_end_matches(" ready").parse().expect("Harvest N ready");
    assert_eq!(bits.len(), n, "the bulk harvest holds the group's ready crops");
    let groups: std::collections::HashSet<_> =
        bits.iter().map(|b| state.crops.iter().find(|c| c.entity_bits == *b).and_then(|c| c.tower_id.clone())).collect();
    assert_eq!(groups.len(), 1, "every crop in it is from one group: {groups:?}");

    // A tile on screen, clicked on its coloured square (a Wheat tile's square carries a "W"; the
    // words under it are selectable labels, which keep a press for themselves): its card opens
    // below its group's grid.
    let tile = find(&f.output, "W").expect("a Wheat tile on screen");
    let [down, up] = click(tile.center());
    draw(&ctx, &theme, &state, mid, None, down);
    draw(&ctx, &theme, &state, mid, None, up);
    let after = settled(&ctx, &theme, &state, mid);
    let shown = visible_texts(&after.output);
    assert!(shown.iter().any(|(t, _)| t == "Stage"), "the clicked tile's card is drawn (its Stage row)");
    offscreen::set_test_override(None);
}

/// The timings for docs/design/sim-frame-cost.md: the section's own draw (`cpu.garden.page`) for
/// 2,000 and 10,000 crops (in five times the groups, more towers, and in the same 90, bigger
/// towers), the median of 40 frames after warming up, at the top of the garden and in its middle,
/// and the first frame (nothing known yet). Run on its own:
///   just test-lib garden_section_timings --ignored --nocapture
/// A test build is unoptimised, so the absolute numbers are several times the release game's;
/// before against after, and 2,000 against 10,000, is what they are for.
#[test]
#[ignore]
fn garden_section_timings() {
    let theme = crate::gui::theme::load_theme();
    for skip in [false, true] {
        offscreen::set_test_override(Some(skip));
        for (crops, groups) in [(2000, 90), (10_000, 450), (10_000, 90)] {
            let state = garden(crops, groups);
            let ctx = ctx_for(&theme);
            let first = draw(&ctx, &theme, &state, 0.0, None, Vec::new());
            let mid = ((first.content_h - first.view_h) / 2.0).round();
            let mut line = format!("skip={skip:<5} {crops:>6} crops {groups:>3} groups: first frame {:7.2} ms", first.section_ms);
            for (label, at) in [("top", 0.0), ("middle", mid)] {
                for _ in 0..3 {
                    draw(&ctx, &theme, &state, at, None, Vec::new());
                }
                let mut ms: Vec<f64> = (0..40).map(|_| draw(&ctx, &theme, &state, at, None, Vec::new()).section_ms).collect();
                ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let tiles = draw(&ctx, &theme, &state, at, None, Vec::new()).tiles;
                line += &format!(", {label} {:7.2} ms ({tiles} tiles)", ms[ms.len() / 2]);
            }
            println!("{line}");
        }
    }
    offscreen::set_test_override(None);
}

/// The group titles' machine types, looked up without copying every machine in the home
/// (2026-10-31), agree with `MachineHome::instance_types` (every machine copied, arrays expanded)
/// for every placed machine in every shipped home, and name nothing that is not placed (a
/// zero-padded or out-of-range array cell, an unknown id).
#[test]
fn machine_types_match_every_placed_machine() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/machines");
    let mut homes = 0;
    for entry in std::fs::read_dir(&dir).expect("data/machines") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        // Through the simulation's loader, as everything above the ship must (crate split rung 0,
        // grow_machines::nothing_above_the_ship_loads_a_home_without_its_food).
        let Some(home) = crate::systems::grow_machines::load_home(&path) else { continue };
        homes += 1;
        let all = home.instance_types();
        let mut ids: Vec<&str> = all.keys().map(String::as_str).collect();
        let bogus = ["tower_01", "tower_99999", "not_a_machine", ""];
        ids.extend(bogus);
        let got = super::machine_types_of(&home, &ids);
        for (id, machine) in &all {
            assert_eq!(got.get(id), Some(machine), "{}: {id}", path.display());
        }
        for id in bogus {
            assert_eq!(got.get(id), all.get(id), "{}: {id} is not placed", path.display());
        }
        assert_eq!(got.len(), all.len(), "{}: nothing extra", path.display());
    }
    assert!(homes > 0, "the shipped homes load");
}

/// A slot's planned (plant, role, note), read by index (2026-10-31), is the one the flattened list
/// gave it before (each planting repeated `slots.max(1)` times, in order) for every shipped tower
/// design, and past the end there is none; a bed plans its crop in every unit.
#[test]
fn slot_specs_read_by_index_match_the_flattened_plantings() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let towers = crate::gui::load_tower_configs(&data);
    assert!(!towers.is_empty(), "the shipped tower designs load");
    for t in &towers {
        let flat: Vec<(String, String, String)> = t
            .plantings
            .iter()
            .flat_map(|p| std::iter::repeat((p.plant.clone(), p.role.clone(), p.note.clone())).take(p.slots.max(1) as usize))
            .collect();
        for s in 0..flat.len() + 3 {
            let want = flat.get(s).map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()));
            assert_eq!(super::garden_slot_spec(&None, Some(t), s), want, "{} slot {s}", t.id);
        }
    }
    let bed = Some(("grain_field".to_string(), "wheat".to_string(), 3u32));
    assert_eq!(super::garden_slot_spec(&bed, None, 2), Some(("wheat", "", "")));
    assert_eq!(super::garden_slot_spec(&bed, None, 3), None);
    assert_eq!(super::garden_slot_spec(&None, None, 0), None);
}
