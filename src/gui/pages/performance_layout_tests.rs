// Child of pages/performance.rs: the pies as drawn (BUG-254). The operator's screenshot of Dev >
// Performance at his 2560x1440 (2026-11-05): "GPU frame time looks good but all the others look
// broke". Headless: the page is drawn into an egui context with no window, from a snapshot with a
// different number in every row, and what it painted is read back.
//
// WHAT WAS WRONG, measured on the old legend (2026-11-06, the real Dev > Performance path at
// 2560x1440): each row's share and amount were right-aligned against the whole column, so the GPU
// row "World surface (locked)" (x 32..157) had its "50%" and "6.90 ms" at x 1202..1275, a
// thousand points from its name and 22 from the CPU pie's "Render submission (locked)" at x 1297.
// Every left pie's numbers read as the right pie's. And the CPU pie's neighbouring rows "Maps:
// where you are", "Maps: symbols", "Maps: plan tiles" and "In-world screens" shared one colour,
// as did "Game systems" and "Data for the open pages", so their wedges ran together into one.

use std::f32::consts::{FRAC_PI_2, TAU};

use egui::{Color32, Pos2, Rect, Vec2};
use humanity_core::frame_costs::{Entry, GpuTiming, Snapshot};

use super::*;
use crate::gui::theme::{load_theme, Theme};
use crate::gui::widgets::pie::fractions;

/// A snapshot with a different, nonzero number in every row, so every row has a value of its own
/// and a wedge of its own. The registry's sources are leaked into `&'static str` ids (the store's
/// own type); a test process leaks a few dozen short strings.
fn busy_snapshot() -> Snapshot {
    let mut snap = Snapshot {
        gpu: Vec::new(),
        cpu: Vec::new(),
        vram: Vec::new(),
        ram: Vec::new(),
        frame_ms: 0.0,
        gpu_timing: GpuTiming::Timestamps,
        has_frames: true,
        gpu_untimed_passes: 0,
    };
    let (mut gpu_sum, mut cpu_sum, mut vram_sum) = (0.0, 0.0, 0.0);
    for (i, row) in systems().iter().enumerate() {
        let k = (i + 1) as f64;
        for source in &row.sources {
            let id: &'static str = Box::leak(source.clone().into_boxed_str());
            match row.category.as_str() {
                "gpu" => {
                    snap.gpu.push(Entry { id, value: 0.13 * k });
                    gpu_sum += 0.13 * k;
                }
                "cpu" => {
                    snap.cpu.push(Entry { id, value: 0.021 * k });
                    cpu_sum += 0.021 * k;
                }
                "vram" => {
                    snap.vram.push(Entry { id, value: 9.0e6 * k });
                    vram_sum += 9.0e6 * k;
                }
                _ => snap.ram.push(Entry { id, value: 4.0e7 * k }),
            }
        }
    }
    // The frame and the allocator's total a little over what the rows name, so the GPU and CPU
    // pies' "Elsewhere" and the VRAM pie's "Untracked / driver" are real wedges too.
    snap.frame_ms = (gpu_sum.max(cpu_sum) * 1.25) as f32;
    let reserved: &'static str = Box::leak(String::from("vram.driver_reserved").into_boxed_str());
    snap.vram.push(Entry { id: reserved, value: vram_sum * 1.3 });
    snap
}

/// What one frame painted: every text with where, and every filled polygon (a wedge is one).
struct Drawn {
    texts: Vec<(String, Rect)>,
    polygons: Vec<(Color32, Vec<Pos2>)>,
}

fn collect(out: &egui::FullOutput) -> Drawn {
    fn visit(shape: &egui::Shape, clip: Rect, d: &mut Drawn) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if rect.is_positive() {
                    d.texts.push((t.galley.text().to_string(), rect));
                }
            }
            egui::Shape::Path(p) if p.closed && p.fill != Color32::TRANSPARENT && p.points.len() >= 6 => {
                d.polygons.push((p.fill, p.points.clone()));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, d)),
            _ => {}
        }
    }
    let mut d = Drawn { texts: Vec::new(), polygons: Vec::new() };
    for cs in &out.shapes {
        visit(&cs.shape, cs.clip_rect, &mut d);
    }
    d
}

fn context(theme: &Theme) -> egui::Context {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    theme.apply_to_egui(&ctx);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    ctx
}

/// The page in a window of `size`, settled over three frames, then (when `scroll` asks) scrolled
/// to its end with the mouse wheel (egui eases a wheel's scroll over a few frames) and the pointer
/// let go, so no wedge is lifted by a hover: what the last frame painted.
fn draw_page(size: Vec2, snap: &Snapshot, scroll: bool) -> Drawn {
    let theme = load_theme();
    let ctx = context(&theme);
    let screen = Rect::from_min_size(Pos2::ZERO, size);
    let run = |events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run(input, |ctx| page(ctx, &theme, |ui| draw_content_from(ui, &theme, snap)))
    };
    let mut last = run(Vec::new());
    for _ in 0..2 {
        last = run(Vec::new());
    }
    if scroll {
        run(vec![egui::Event::PointerMoved(screen.center())]);
        for _ in 0..40 {
            let wheel = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -600.0), modifiers: egui::Modifiers::default() };
            run(vec![wheel]);
        }
        run(vec![egui::Event::PointerGone]);
        last = run(Vec::new());
    }
    collect(&last)
}

/// Each pie's column, from where the titles were drawn: a pie's column runs from its title's left
/// edge to the next title's left edge on its row of the grid, or to the window's right edge, and
/// down to the next title below it. `(category, title, column)`, for the titles this frame painted.
fn columns(d: &Drawn, screen_w: f32) -> Vec<(&'static str, &'static str, Rect)> {
    let titles: Vec<(&'static str, &'static str, Rect)> = CATEGORIES
        .iter()
        .filter_map(|(cat, title)| d.texts.iter().find(|(s, _)| s == title).map(|(_, r)| (*cat, *title, *r)))
        .collect();
    titles
        .iter()
        .map(|(cat, title, r)| {
            let right = titles
                .iter()
                .filter(|(_, _, o)| (o.center().y - r.center().y).abs() < 4.0 && o.left() > r.left() + 1.0)
                .map(|(_, _, o)| o.left())
                .fold(screen_w, f32::min);
            let bottom = titles
                .iter()
                .filter(|(_, _, o)| (o.left() - r.left()).abs() < 4.0 && o.top() > r.bottom())
                .map(|(_, _, o)| o.top())
                .fold(f32::INFINITY, f32::min);
            (*cat, *title, Rect::from_min_max(egui::pos2(r.left(), r.top()), egui::pos2(right, bottom)))
        })
        .collect()
}

/// The room between two rectangles across a row (0 when they overlap).
fn gap_x(a: &Rect, b: &Rect) -> f32 {
    (a.left().max(b.left()) - a.right().min(b.right())).max(0.0)
}

/// The angle of `p` around `c`, measured the way the wedges are drawn (0 at 3 o'clock, growing
/// clockwise on screen, where y grows downward).
fn angle(c: Pos2, p: Pos2) -> f32 {
    (p.y - c.y).atan2(p.x - c.x)
}

/// Wrap an angle difference into (0, TAU].
fn sweep(a0: f32, a1: f32) -> f32 {
    let mut s = (a1 - a0) % TAU;
    if s <= 1e-4 {
        s += TAU;
    }
    s
}

/// Every pie whose title is on screen. Returns how many pies were checked whole.
fn check(d: &Drawn, screen_w: f32, snap: &Snapshot, at: &str) -> usize {
    let theme = load_theme();
    let cols = columns(d, screen_w);
    let specs: Vec<PieSpec> = CATEGORIES.iter().map(|(cat, title)| build_spec(&theme, snap, cat, title)).collect();
    let row_name = |s: &PieSlice| if s.locked { format!("{} (locked)", s.label) } else { s.label.clone() };
    // Every legend name of every pie, wherever it was drawn: the names a number could be read with.
    let all_names: Vec<String> = specs.iter().flat_map(|s| s.slices.iter().map(row_name)).collect();
    let mut checked = 0;
    for (cat, title, col) in &cols {
        let spec = &specs[CATEGORIES.iter().position(|(c, _)| c == cat).expect("a category")];
        let fracs = fractions(&spec.slices);
        assert_eq!(fracs.len(), spec.slices.len(), "{at}: the {title} pie has numbers in every row");
        let inside = |r: &Rect| r.left() >= col.left() - 0.5 && r.right() <= col.right() + 0.5;
        let in_pie = |r: &Rect| inside(r) && r.top() > col.top() && r.bottom() < col.bottom();

        // THE LEGEND. Each row's name in this pie's column; on that row its own share and amount,
        // (1) inside the column, (2) nearer its own name than any other name on the row (a number
        // is read with the name beside it), and (3) in a table as wide as the legend's words, not
        // stretched across the column.
        let mut rows: Vec<(Rect, Rect, Rect)> = Vec::new();
        for (i, s) in spec.slices.iter().enumerate() {
            let name = row_name(s);
            let Some(label) = d.texts.iter().filter(|(t, r)| *t == name && in_pie(r)).map(|(_, r)| *r).next() else {
                // Below the window's foot (the 720 point window before scrolling): not drawn.
                continue;
            };
            let on_row: Vec<&(String, Rect)> = d.texts.iter().filter(|(_, r)| (r.center().y - label.center().y).abs() < 4.0).collect();
            let row_words = || on_row.iter().map(|(t, r)| (t.as_str(), r.left().round(), r.right().round())).collect::<Vec<_>>();
            let share = format!("{:.0}%", fracs[i] * 100.0);
            let mut found = Vec::new();
            for want in [&share, &s.display] {
                let r = on_row
                    .iter()
                    .filter(|(t, _)| t == want)
                    .map(|(_, r)| *r)
                    .min_by(|a, b| gap_x(a, &label).total_cmp(&gap_x(b, &label)))
                    .unwrap_or_else(|| panic!("{at}: the {title} row {:?} draws {want:?} on its row: {:?}", s.label, row_words()));
                assert!(
                    inside(&r),
                    "{at}: the {title} row {:?} has its {want:?} inside its own column {:?}; it is at {:?}",
                    s.label,
                    (col.left(), col.right()),
                    (r.left(), r.right())
                );
                let (nearest, nearest_gap) = on_row
                    .iter()
                    .filter(|(t, _)| all_names.contains(t))
                    .map(|(t, n)| (t.as_str(), gap_x(&r, n)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .expect("the row's own name is on its row");
                assert_eq!(
                    nearest,
                    name,
                    "{at}: the {title} row {:?} has its {want:?} at x {:.0}..{:.0}, {:.0} points from {nearest:?} and {:.0} from \
                     its own name: it reads as {nearest:?}'s (the row: {:?})",
                    s.label,
                    r.left(),
                    r.right(),
                    nearest_gap,
                    gap_x(&r, &label),
                    row_words()
                );
                found.push(r);
            }
            rows.push((label, found[0], found[1]));
        }
        if !rows.is_empty() {
            let widest = |f: fn(&(Rect, Rect, Rect)) -> f32| rows.iter().map(f).fold(0.0_f32, f32::max);
            let table = widest(|r| r.0.width()) + widest(|r| r.1.width()) + widest(|r| r.2.width());
            for (label, share, value) in &rows {
                let (left, right) = (label.left().min(share.left()).min(value.left()), label.right().max(share.right()).max(value.right()));
                assert!(
                    right - left <= table + 80.0,
                    "{at}: the {title} legend is a table as wide as its words ({table:.0} points of words): the row of \
                     {:?} runs from x {left:.0} to {right:.0}",
                    label
                );
            }
        }
        if rows.len() < spec.slices.len() {
            // A pie cut by the window's foot is checked whole from the scrolled frame instead.
            continue;
        }

        // THE WEDGES: the pie's centre is under its "systems" caption (pie.rs paints the count 7
        // points above the centre and the caption 9 below); every filled polygon within its radius
        // is one of its wedges, in paint order, which is the legend's order: each spans its row's
        // share, starts where the one before ended, is its row's colour, and differs in colour
        // from the wedges beside it.
        let caption = d
            .texts
            .iter()
            .filter(|(t, r)| t == "systems" && in_pie(r))
            .map(|(_, r)| *r)
            .next()
            .unwrap_or_else(|| panic!("{at}: the {title} pie's centre caption is drawn"));
        let c = caption.center() - Vec2::new(0.0, 9.0);
        let wedges: Vec<&(Color32, Vec<Pos2>)> = d.polygons.iter().filter(|(_, pts)| pts.iter().all(|p| (*p - c).length() < 70.0)).collect();
        let drawn_rows: Vec<usize> = (0..spec.slices.len()).filter(|i| fracs[*i] > 0.0).collect();
        assert_eq!(
            wedges.len(),
            drawn_rows.len(),
            "{at}: the {title} pie draws one wedge per row with a share ({} rows, {} wedges)",
            drawn_rows.len(),
            wedges.len()
        );
        let mut start = -FRAC_PI_2;
        for (k, (w, i)) in wedges.iter().zip(&drawn_rows).enumerate() {
            let (fill, pts) = (w.0, &w.1);
            let n = pts.len() / 2;
            let a0 = angle(c, pts[0]);
            let got = sweep(a0, angle(c, pts[n - 1]));
            let want = fracs[*i] * TAU;
            let s = &spec.slices[*i];
            assert!(
                (got - want).abs() < 0.02,
                "{at}: the {title} wedge of {:?} spans {:.1} degrees; its row's share is {:.1} degrees ({:.1}%)",
                s.label,
                got.to_degrees(),
                want.to_degrees(),
                fracs[*i] * 100.0
            );
            let off = sweep(start, a0) % TAU;
            assert!(
                off < 0.02 || TAU - off < 0.02,
                "{at}: the {title} wedge of {:?} starts where the row before it ends ({:.1} vs {:.1} degrees)",
                s.label,
                a0.to_degrees(),
                start.to_degrees()
            );
            assert_eq!(fill, s.color, "{at}: the {title} wedge of {:?} is its row's colour", s.label);
            let next = wedges[(k + 1) % wedges.len()].0;
            let next_row = &spec.slices[drawn_rows[(k + 1) % drawn_rows.len()]];
            assert!(
                wedges.len() < 2 || fill != next,
                "{at}: the {title} wedges of {:?} and {:?} stand side by side in one colour, so they read as one wedge",
                s.label,
                next_row.label
            );
            start += want;
        }
        checked += 1;
    }
    checked
}

/// EVERY NUMBER BESIDE ITS OWN NAME, EVERY WEDGE ITS ROW'S SHARE (BUG-254): at the operator's
/// 2560x1440 the page is two columns of pies; each pie's legend numbers stand inside its own
/// column, next to their own names and nearer them than to any other pie's names, in a table as
/// wide as its words; each wedge is its row's share, in the legend's order and colours, and no two
/// wedges side by side share a colour. At 1280x720 the same, the second row of pies read after
/// scrolling to the page's end.
///
/// Seen red 2026-11-06 on the old legend (pie.rs right-aligning the numbers against the column):
/// "2560x1440: the GPU frame time row \"World surface\" has its \"1%\" at x 1202..1218, 79 points
/// from \"Render submission (locked)\" and 1045 from its own name: it reads as \"Render submission
/// (locked)\"'s"; then, with the new legend and the registry's colours as they were, "2560x1440:
/// the CPU frame time wedges of \"Maps: where you are\" and \"Maps: symbols\" stand side by side
/// in one colour, so they read as one wedge"; and with the legend's names first (each number in a
/// column after the widest name), "1280x720: the GPU frame time row \"Clouds\" has its \"1.56 ms\"
/// at x 532..576, 229 points from \"Maps: where you are\" and 316 from its own name", which is why
/// a row reads swatch, share, amount, name.
#[test]
fn every_number_beside_its_own_name_and_every_wedge_its_rows_share() {
    let snap = busy_snapshot();
    for size in [egui::vec2(2560.0, 1440.0), egui::vec2(1280.0, 720.0)] {
        let at = format!("{}x{}", size.x, size.y);
        let top = draw_page(size, &snap, false);
        let mut checked = check(&top, size.x, &snap, &at);
        if size.y < 1000.0 {
            let end = draw_page(size, &snap, true);
            checked += check(&end, size.x, &snap, &format!("{at}, scrolled to the end"));
        }
        assert!(checked >= CATEGORIES.len(), "{at}: all four pies were checked whole ({checked})");
        // Two columns: the GPU and CPU pies side by side.
        let cols = columns(&top, size.x);
        let gpu = cols.iter().find(|c| c.0 == "gpu").expect("the GPU pie is drawn").2;
        let cpu = cols.iter().find(|c| c.0 == "cpu").expect("the CPU pie is drawn").2;
        assert!((gpu.top() - cpu.top()).abs() < 4.0, "{at}: the GPU and CPU pies share a row: {gpu:?} {cpu:?}");
        assert!(cpu.left() > gpu.left() + size.x * 0.3, "{at}: the CPU pie stands in the right column: {gpu:?} {cpu:?}");
    }
}
