//! F10 cloud dev panel (v0.1254.4, operator request).
//!
//! The cloud bisect toggles used to live only in debug/showcase_request.json
//! file drops - the operator: "When I tried to create the showcase_request
//! .json it vanished. Can you make sure you put these commands in a presently
//! nonexistent F10 menu? Since we want GUI buttons for everything." (The
//! vanishing was the game consuming the file - correct but invisible.) Per
//! the GUI-first norm, every dev knob an AI or operator can reach over IPC
//! must also be a button. This panel drives the SAME renderer flags the
//! showcase pins set, so the two surfaces can never disagree.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// One "please test this" request for the F10 panel, from
/// data/gui/dev_tests.json. `label` is a PREFIX of the row's label text;
/// `note` says what to do and what to expect.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct DevTest {
    pub label: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct DevTestsFile {
    #[serde(default)]
    tests: Vec<DevTest>,
}

/// The current test requests, re-read when the file changes (it is edited
/// by the AI during a session, so the panel must follow it live). A missing
/// or malformed file means "nothing to test", never a crash.
pub fn dev_tests() -> Vec<DevTest> {
    use std::sync::{Mutex, OnceLock};
    use std::time::SystemTime;
    static CACHE: OnceLock<Mutex<(Option<SystemTime>, Vec<DevTest>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new((None, Vec::new())));
    let path = "data/gui/dev_tests.json";
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Ok(mut c) = cache.lock() {
        if c.0 == mtime && mtime.is_some() {
            return c.1.clone();
        }
        let tests = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<DevTestsFile>(&t).ok())
            .map(|f| f.tests)
            .unwrap_or_default();
        *c = (mtime, tests.clone());
        return tests;
    }
    Vec::new()
}

/// Paint a red TEST tag before a row whose label matches a test request, so
/// the operator can find the exact switch the chat named.
pub fn test_mark(ui: &mut egui::Ui, theme: &Theme, label: &str, tests: &[DevTest]) {
    if let Some(t) = tests.iter().find(|t| !t.label.is_empty() && label.starts_with(t.label.as_str())) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("TEST").strong().color(theme.danger()));
            ui.label(RichText::new(t.note.as_str()).size(theme.font_size_small).color(theme.danger()));
        });
    }
}

/// Width of the collapse tab that lives on the panel's RIGHT edge (and is
/// all that remains of the panel when collapsed). Slim on purpose: the
/// operator asked for a strip they can click instead of pressing F10.
/// 22 rather than 18 (critic, 2026-09-05): the body panel's resize handle
/// senses drags 5 px either side of the panel edge, so the tab's leftmost
/// 5 px double as a resize grip; the extra width keeps a centred click
/// clear of it.
const TAB_W: f32 = 22.0;

/// What a click on the edge tab asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TabAction {
    /// Nothing clicked this frame.
    None,
    /// The strip itself: collapse an expanded sidebar, expand a collapsed one.
    Toggle,
    /// The small cross under the arrow: close the sidebar entirely (the
    /// mouse equivalent of F10 on an expanded sidebar). Added because the
    /// Window era had a title-bar close box and the first sidebar cut had
    /// no way to close it from the UI at all (critic, 2026-09-05).
    Close,
}

/// A slider row WITHOUT egui's click-to-type number box. egui's Slider
/// draws its value as a DragValue, and clicking that box takes keyboard
/// FOCUS; egui-winit then reports every key event as consumed while any
/// widget has focus, and lib.rs stops forwarding keys to the game on
/// consumed events. In a panel built to be clicked while flying, that meant
/// one stray click on a "1.000" box killed WASD AND swallowed the key
/// RELEASE, so the player drifted with nothing held (critic, 2026-09-05).
/// The value is shown as a plain label instead: same row shape (slider,
/// number, caption), nothing here can take focus. Returns true on change.
fn dev_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) -> bool {
    ui.horizontal(|ui| {
        let changed = ui
            .add(egui::Slider::new(value, range).show_value(false))
            .changed();
        // Three decimals, the same precision egui's own box showed.
        ui.label(RichText::new(format!("{value:.3}")).monospace());
        ui.label(label);
        changed
    })
    .inner
}
/// Default sidebar width. About 420 px shows the longest checkbox labels on
/// one or two lines; the edge is draggable if the operator wants more.
const DEFAULT_W: f32 = 420.0;

/// The collapse / expand tab: a slim vertical strip that fills `rect`,
/// painted with a left arrow when the panel is open (click = collapse) and a
/// right arrow when it is collapsed (click = expand). Arrow GLYPHS are banned
/// in UI text (they render as tofu, see icon_glyph_lint), so the arrow is
/// painted with the widgets::icons helpers. Below the arrow sits a small
/// painted cross that CLOSES the sidebar outright (its own hit rect,
/// registered after the strip so it wins the overlap). Returns what was
/// clicked.
fn collapse_tab(ui: &mut egui::Ui, theme: &Theme, rect: egui::Rect, expanded: bool) -> TabAction {
    let resp = ui.interact(rect, ui.id().with("cloud_dev_collapse_tab"), egui::Sense::click());
    // The close cross: a TAB_W square two icon-heights below the arrow.
    let close_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 24.0 + TAB_W * 2.0),
        egui::vec2(TAB_W, TAB_W),
    );
    let close = ui.interact(close_rect, ui.id().with("cloud_dev_close"), egui::Sense::click());
    // Tertiary normally so the strip stands out from the panel's dark ground
    // (a bg_secondary strip vanished into it in the first snapshot); accent
    // on hover so the click target is unmistakable.
    let fill = if resp.hovered() { theme.accent() } else { theme.bg_tertiary() };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);
    // Hairlines on BOTH sides so the strip reads as its own column even on
    // a theme whose background tokens are all near-black (the operator's
    // theme.ron has bg_secondary/bg_tertiary/bg_sidebar_dark within 2/255
    // of each other, so a fill alone was invisible in the snapshot). The
    // border token is the one guaranteed to contrast with every ground.
    let edge = egui::Stroke::new(1.0, theme.border());
    // Half a pixel inside each edge: a line ON the edge is split with the
    // neighbour and the panel's clip rect drops the outer half entirely.
    let (l, r) = (rect.left() + 0.5, rect.right() - 0.5);
    painter.line_segment([egui::pos2(l, rect.top()), egui::pos2(l, rect.bottom())], edge);
    painter.line_segment([egui::pos2(r, rect.top()), egui::pos2(r, rect.bottom())], edge);
    // The arrow sits near the top so it stays visible whatever the window
    // height; a small square keeps the icon helper's proportions.
    let icon = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 24.0),
        egui::vec2(TAB_W - 4.0, TAB_W - 4.0),
    );
    let color = if resp.hovered() { theme.text_primary() } else { theme.text_secondary() };
    if expanded {
        crate::gui::widgets::icons::paint_arrow_left(painter, icon, color);
    } else {
        crate::gui::widgets::icons::paint_arrow_right(painter, icon, color);
    }
    // The close cross: two diagonals inside a square inset from the strip
    // edges, danger-coloured on hover so it reads as "close", not "collapse".
    let x_color = if close.hovered() { theme.danger() } else { theme.text_secondary() };
    let x = close_rect.shrink(6.0);
    let x_stroke = egui::Stroke::new(1.5, x_color);
    painter.line_segment([x.left_top(), x.right_bottom()], x_stroke);
    painter.line_segment([x.right_top(), x.left_bottom()], x_stroke);
    // on_hover_text consumes the response, so read what we need first.
    let (close_clicked, close_hovered) = (close.clicked(), close.hovered());
    close.on_hover_text("Close the Cloud dev sidebar (F10 reopens it)");
    let resp = resp.on_hover_text(if expanded {
        "Collapse the Cloud dev sidebar to this tab (the cross below closes it, F10 closes it too)"
    } else {
        "Expand the Cloud dev sidebar (F10 also expands it)"
    });
    if close_clicked {
        TabAction::Close
    } else if resp.clicked() && !close_hovered {
        // The cross is registered after the strip so it wins the hit test;
        // the hovered() guard is belt and braces against the two firing
        // together on one press.
        TabAction::Toggle
    } else {
        TabAction::None
    }
}

/// Draw the panel. Returns true when the operator changed something, so the
/// caller (lib.rs) re-publishes the flags to the renderer.
///
/// Layout (2026-09-05, operator request: "the F10 menu is far too long for
/// me to see the full list. We should add it as a left sidebar menu that we
/// can scroll ... add a button to the right side of the sidebar menu that
/// collapses it"): TWO left SidePanels. egui stacks successive left panels
/// left to right, so the first (the body: resizable, one vertical
/// ScrollArea holding the whole switch list) sits at the screen edge and
/// the second (the slim collapse tab) lands flush against the body's right
/// edge. When collapsed the body panel is simply not drawn, so the tab
/// alone remains at the screen edge; clicking it expands the body again.
/// The tab is its own panel rather than a strip carved out of the body
/// because a vertical ScrollArea GROWS sideways to fit rows that cannot
/// wrap (slider captions), which pushed the scroll bar over a carved-out
/// strip in the first snapshot.
/// The cursor is freed while the body is expanded (see
/// GuiState::cloud_dev_sidebar_expanded and lib.rs reconcile_cursor), so
/// the operator no longer has to hold Alt to click anything here.
///
/// The tab is only drawn while the cursor is FREE (expanded, or collapsed
/// on a menu page / with Alt held): while the cursor is grabbed the hidden
/// pointer drifts to the window edge on a long turn, and a click-sensing
/// strip there would eat the operator's next fire click and pop the
/// sidebar open (critic finding, 2026-09-05). F10 expands a collapsed
/// sidebar for exactly that reason, so nothing becomes unreachable.
///
/// Intentional, do not "fix": this is the LAST panel drawn in the frame
/// (after the page CentralPanel in lib.rs), and egui's CentralPanel does
/// not shrink the frame's available rect, so with a GuiPage open the
/// sidebar OVERLAYS that page's left edge instead of pushing it right.
/// Moving this call earlier in the frame would shift every page's layout
/// whenever the dev sidebar is open; overlaying is the right trade for a
/// dev tool the operator uses in the world, not on the menu pages.
pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) -> bool {
    if !state.show_cloud_dev_panel {
        return false;
    }
    let frame = egui::Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0);
    let mut changed = false;
    let expanded = !state.cloud_dev_collapsed;
    if expanded {
        egui::SidePanel::left("cloud_dev_panel")
            .frame(frame)
            .default_width(DEFAULT_W)
            .width_range(260.0..=1200.0)
            .resizable(true)
            .show_separator_line(false)
            .show(ctx, |ui| {
                // The scroll HANDLE is painted with widgets.inactive.bg_fill,
                // which the theme maps to bg_card; on the operator's
                // near-black theme that is invisible against the panel (the
                // first snapshot showed no bar at all). Paint the handle in
                // the border token instead, scoped to this ui: the bar is
                // drawn by THIS ui, and the content ui below restores the
                // original so checkboxes and sliders keep their look.
                let inactive_fill = ui.style().visuals.widgets.inactive.bg_fill;
                ui.style_mut().visuals.widgets.inactive.bg_fill = theme.border();
                // egui's default "floating" bar fades its handle to ZERO
                // opacity when idle, even with AlwaysVisible (that setting
                // only stops the bar being dropped, not faded). The solid
                // style paints the handle at full opacity, so the bar truly
                // stands: the operator can see at a glance how much of the
                // list is below the fold.
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                // The whole list scrolls; auto_shrink(false) keeps the
                // scroll area at the panel's full size so the bar sits at
                // the panel edge rather than hugging the content.
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.style_mut().visuals.widgets.inactive.bg_fill = inactive_fill;
                        egui::Frame::NONE
                            .inner_margin(theme.spacing_md)
                            .show(ui, |ui| {
                                ui.heading("Cloud dev (F10)");
                                ui.add_space(theme.spacing_sm);
                                changed |= draw_body(ui, theme, state);
                            });
                    });
            });
    }
    // The tab: its own panel id, so the body's remembered width is never
    // overwritten by the tab's, and it exists in both states (flush right
    // of the body when expanded, alone at the screen edge when collapsed),
    // EXCEPT while the cursor is grabbed: then no click target may exist at
    // the screen edge (see the doc comment above; `state.cursor_free` is
    // the mirror lib.rs reconcile_cursor keeps for exactly this question).
    // Expanded implies free, the `||` just makes that explicit.
    if expanded || state.cursor_free {
        egui::SidePanel::left("cloud_dev_tab")
            .frame(frame)
            .exact_width(TAB_W)
            .resizable(false)
            .show_separator_line(false)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                match collapse_tab(ui, theme, rect, expanded) {
                    TabAction::Toggle => state.cloud_dev_collapsed = expanded,
                    // Same as F10 on an expanded sidebar: the cursor re-grabs
                    // through reconcile_cursor once the expanded term drops.
                    TabAction::Close => state.show_cloud_dev_panel = false,
                    TabAction::None => {}
                }
            });
    }
    changed
}

/// The switch list itself, unchanged from the Window era: every row is the
/// same renderer flag the probe rig pins, with its TEST tag where
/// data/gui/dev_tests.json names it. Returns true when a value changed.
fn draw_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> bool {
    let mut changed = false;
    ui.label(
        RichText::new(
            "Live cloud-renderer diagnostics. These are the same \
             switches the probe rig uses; flipping them takes effect \
             the same frame and nothing here is saved.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_sm);
    // NEEDS TESTING (data/gui/dev_tests.json): the operator asked for a
    // marker on exactly the switches the chat names, so the list is
    // data the AI edits during a session and the panel follows live.
    let tests = dev_tests();
    if !tests.is_empty() {
        ui.label(RichText::new("NEEDS TESTING").strong().color(theme.danger()));
        for t in &tests {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t.label.as_str()).strong().color(theme.danger()));
                ui.label(RichText::new(t.note.as_str()).size(theme.font_size_small).color(theme.text_secondary()));
            });
        }
        ui.add_space(theme.spacing_sm);
    }

    ui.label(RichText::new("Look").strong().color(theme.accent()));
    // Dither: the static-vs-agate trade, the operator's live choice
    // until the mip-response calibration retires both (v0.1254.3).
    let mut dither_on = !state.cloud_dev_dither_off;
    test_mark(ui, theme, "Depth dither (off = smoother clouds, agate arcs on overcast)", &tests);
    if ui
        .checkbox(
            &mut dither_on,
            "Depth dither (off = smoother clouds, agate arcs on overcast)",
        )
        .changed()
    {
        state.cloud_dev_dither_off = !dither_on;
        changed = true;
    }
    let mut temporal_on = !state.cloud_dev_temporal_off;
    test_mark(ui, theme, "Temporal accumulation (off = raw march, sharp per-frame noise)", &tests);
    if ui
        .checkbox(
            &mut temporal_on,
            "Temporal accumulation (off = raw march, sharp per-frame noise)",
        )
        .changed()
    {
        state.cloud_dev_temporal_off = !temporal_on;
        changed = true;
    }

    let mut shape_on = !state.cloud_dev_shape_off;
    test_mark(ui, theme, "Cloud shape frame (off = old round balls, for comparison)", &tests);
    if ui
        .checkbox(
            &mut shape_on,
            "Cloud shape frame (off = old round balls, for comparison)",
        )
        .changed()
    {
        state.cloud_dev_shape_off = !shape_on;
        changed = true;
    }

    // Clock pin (v0.1268). Not a look control - a MEASUREMENT one.
    // The cloud clock is app-start-relative, so two rig runs of one
    // build rendered different cloud fields (20% of pixels differing
    // by >40 levels). Freezing it makes a capture a function of the
    // build alone, which is what an A/B needs to mean anything.
    let mut frozen = state.cloud_dev_clock_pin >= 0.0;
    test_mark(ui, theme, "Freeze cloud drift (for before/after comparisons)", &tests);
    if ui
        .checkbox(&mut frozen, "Freeze cloud drift (for before/after comparisons)")
        .changed()
    {
        state.cloud_dev_clock_pin = if frozen { 120.0 } else { -1.0 };
        changed = true;
    }

    // The v0.1268 rosette fix, as a live A/B (operator directive:
    // "we have the F10 dev menu to toggle things on/off"). ON here
    // restores the OLD behaviour, so the rosette should come BACK -
    // that is how you tell the fix is doing something.
    // The v0.1269 test. A cloud must not change SHAPE because you
    // flew closer to it, and lines of equal distance-to-camera are
    // circles centred on the nadir - which is exactly where the
    // artifact sits.
    // The mip ring cure (v0.1270). Split out of the depth-dither box,
    // which used to switch both. Measured at 9.2 km, clock pinned:
    // direct-sun radial energy 23.97 with the cure on, 28.11 with it
    // off, against a 0.7 noise floor.
    let mut cure = !state.cloud_dev_ring_cure_off;
    test_mark(ui, theme, "Per-pixel mip dither (default OFF: measured never better, worse inside the deck)", &tests);
    if ui
        .checkbox(
            &mut cure,
            "Per-pixel mip dither (default OFF: measured never better, worse inside the deck)",
        )
        .changed()
    {
        state.cloud_dev_ring_cure_off = !cure;
        changed = true;
    }

    // The v0.1271 estimator experiments. Thesis: the density edge is a
    // cliff a few metres wide sampled with steps of tens to hundreds of
    // metres, so every edge sample is a coin flip (the glitter) and the
    // converged mean depends on step spacing, which is a function of
    // the angle to the local vertical (the rosette). Two levers.
    // v0.1280: increment A, the in-cloud light. Inside a cloud at noon
    // the eye should see bright fog-white: the interior was ~10x too
    // dark because every ambient term was a transmittance and the sun
    // ladder resolved the shadows of the lobes around the eye.
    let mut msb = state.cloud_dev_ms;
    test_mark(ui, theme, "In-cloud light (Eddington source: changes the LIGHT only, coverage is identical; fog-white interiors, no lobe shadows through the eye)", &tests);
    if ui
        .checkbox(&mut msb, "In-cloud light (Eddington source: changes the LIGHT only, coverage is identical; fog-white interiors, no lobe shadows through the eye)")
        .changed()
    {
        state.cloud_dev_ms = msb;
        changed = true;
    }
    if state.cloud_dev_ms {
        let mut g = if state.cloud_dev_ms_gain > 0.0 { state.cloud_dev_ms_gain } else { 1.0 };
        test_mark(ui, theme, "in-scatter gain", &tests);
        if dev_slider(ui, &mut g, 0.2..=3.0, "in-scatter gain") {
            state.cloud_dev_ms_gain = g;
            changed = true;
        }
    }
    // Increment C (v0.1282): interior density saturation of the
    // constructed bodies. 0 = the shipped profile (crown fade to the
    // base fraction over the top third, +-42% turbulence); 1 = LWC
    // peaking at the top, opaque within tens of metres, and the
    // in-cloud light fed the body's own column.
    let mut sat = state.cloud_dev_int_sat;
    test_mark(ui, theme, "interior saturation (built bodies)", &tests);
    if dev_slider(ui, &mut sat, 0.0..=1.0, "interior saturation (built bodies)") {
        state.cloud_dev_int_sat = sat;
        changed = true;
    }
    // Perf increment 3 (v0.1287): keep each cell's built lobe cluster
    // for the whole ray instead of rebuilding it at every sample.
    // Perf increment 2 (v0.1288): far rays stop taking 22 m steps and
    // opaque rays relax their step; 0 = off, 1 = full.
    let mut eco = state.cloud_dev_step_eco;
    test_mark(ui, theme, "step economy (footprint floors + deep relaxation)", &tests);
    if dev_slider(ui, &mut eco, 0.0..=1.0, "step economy (footprint floors + deep relaxation)") {
        state.cloud_dev_step_eco = eco;
        changed = true;
    }
    let mut bc = state.cloud_dev_body_cache;
    test_mark(ui, theme, "Body cluster cache (per ray; off = rebuild the lobes at every sample, for A/B)", &tests);
    if ui
        .checkbox(&mut bc, "Body cluster cache (per ray; off = rebuild the lobes at every sample, for A/B)")
        .changed()
    {
        state.cloud_dev_body_cache = bc;
        changed = true;
    }
    let mut fld = state.cloud_dev_field;
    test_mark(ui, theme, "Field walls (three-octave warp: sinuous walls, turbulent 100-500 m band)", &tests);
    if ui
        .checkbox(&mut fld, "Field walls (three-octave warp: sinuous walls, turbulent 100-500 m band)")
        .changed()
    {
        state.cloud_dev_field = fld;
        changed = true;
    }
    // Performance plan increment 1 (v0.1286): the sun optical depth
    // becomes a planet-fixed cached quantity (two nested windows
    // around the camera, baked by the same rung ladder, read with
    // one tap per sample). Off is the exact old path, so this box
    // IS the A/B: flip it and the picture must not change while
    // the frame time does. The "Sun source" bisect channel below
    // paints which window each pixel read from.
    let mut lcb = state.cloud_dev_light;
    test_mark(ui, theme, "Sun shadow cache (off = 12-rung ladder per pixel, for A/B)", &tests);
    if ui
        .checkbox(&mut lcb, "Sun shadow cache (off = 12-rung ladder per pixel, for A/B)")
        .changed()
    {
        state.cloud_dev_light = lcb;
        changed = true;
    }
    // v0.1272: the two fixes the estimator assessment designed.
    let mut estb = state.cloud_dev_est;
    test_mark(ui, theme, "Sample-anchored march (default ON since v0.1272; off = the OLD under-counting march for A/B only: see-through, misses whole bodies, glittery)", &tests);
    if ui
        .checkbox(&mut estb, "Sample-anchored march (default ON since v0.1272; off = the OLD under-counting march for A/B only: see-through, misses whole bodies, glittery)")
        .changed()
    {
        state.cloud_dev_est = estb;
        changed = true;
    }
    let mut wbl = state.cloud_dev_warp_bl;
    test_mark(ui, theme, "Warp band-limited to its own tile (default ON since v0.1272; off = 3-20 m silhouette hash)", &tests);
    if ui
        .checkbox(&mut wbl, "Warp band-limited to its own tile (default ON since v0.1272; off = 3-20 m silhouette hash)")
        .changed()
    {
        state.cloud_dev_warp_bl = wbl;
        changed = true;
    }
    // The lean (v0.1275): if the fan is the prism walls, its
    // convergence point moves off straight-down as this rises.
    let mut sh = state.cloud_dev_shear;
    test_mark(ui, theme, "cloud lean (m per m of height; 0 = off)", &tests);
    if dev_slider(ui, &mut sh, 0.0..=1.0, "cloud lean (m per m of height; 0 = off)") {
        state.cloud_dev_shear = sh;
        changed = true;
    }
    // Component bisect (v0.1279): one density term off at a time.
    ui.label(RichText::new("Density component bisect (each turns ONE term off)").size(theme.font_size_small).color(theme.text_secondary()));
    for (label, flag) in [
        ("detail erosion off", &mut state.cloud_dev_no_detail),
        ("puff erosion off", &mut state.cloud_dev_no_puff),
        ("cell split off", &mut state.cloud_dev_no_cell),
        ("fray off", &mut state.cloud_dev_no_fray),
        ("base drop off", &mut state.cloud_dev_no_bdrop),
        ("sharp base (0.5% of band, not 3%)", &mut state.cloud_dev_sharp_base),
        ("interior relief fade (no lobe shading deep inside cloud)", &mut state.cloud_dev_relief_fade),
        ("coarse sun ladder deep inside (skip rungs under 200 m)", &mut state.cloud_dev_deep_rung),
        ("SYNTHETIC CHECKER density (projection test: must render as a grid)", &mut state.cloud_dev_checker),
    ] {
        if ui.checkbox(flag, label).changed() {
            changed = true;
        }
    }
    let mut hvw = state.cloud_dev_hv_warp;
    test_mark(ui, theme, "Height-varying cloud walls (the prism-wall fix: walls wander with altitude)", &tests);
    if ui
        .checkbox(&mut hvw, "Height-varying cloud walls (the prism-wall fix: walls wander with altitude)")
        .changed()
    {
        state.cloud_dev_hv_warp = hvw;
        changed = true;
    }
    if state.cloud_dev_hv_warp {
        let mut hk = if state.cloud_dev_hv_km > 0.0 { state.cloud_dev_hv_km } else { 0.5 };
        test_mark(ui, theme, "wall wander km per 1.3 km of height", &tests);
        if dev_slider(ui, &mut hk, 0.1..=5.0, "wall wander km per 1.3 km of height") {
            state.cloud_dev_hv_km = hk;
            changed = true;
        }
    }
    let mut sm = state.cloud_dev_sigma_mul;
    test_mark(ui, theme, "extinction x (0 = off; the transparency test)", &tests);
    if dev_slider(ui, &mut sm, 0.0..=10.0, "extinction x (0 = off; the transparency test)") {
        state.cloud_dev_sigma_mul = sm;
        changed = true;
    }
    let mut thin = state.cloud_dev_thin_deck;
    test_mark(ui, theme, "Thin deck (band height x0.3: the prism-wall test for the rosette)", &tests);
    if ui
        .checkbox(&mut thin, "Thin deck (band height x0.3: the prism-wall test for the rosette)")
        .changed()
    {
        state.cloud_dev_thin_deck = thin;
        changed = true;
    }
    let mut iso = state.cloud_dev_iso_step;
    test_mark(ui, theme, "Isotropic near step (no verticality or chord term inside 27 km)", &tests);
    if ui
        .checkbox(&mut iso, "Isotropic near step (no verticality or chord term inside 27 km)")
        .changed()
    {
        state.cloud_dev_iso_step = iso;
        changed = true;
    }
    let mut nfl = state.cloud_dev_norm_floor;
    test_mark(ui, theme, "Carve normaliser floor (low-coverage stencil becomes a 450 m ramp)", &tests);
    if ui
        .checkbox(&mut nfl, "Carve normaliser floor (low-coverage stencil becomes a 450 m ramp)")
        .changed()
    {
        state.cloud_dev_norm_floor = nfl;
        changed = true;
    }
    let mut ustep = state.cloud_dev_uniform_step;
    test_mark(ui, theme, "Uniform march step (distance-only; no verticality term)", &tests);
    if ui
        .checkbox(&mut ustep, "Uniform march step (distance-only; no verticality term)")
        .changed()
    {
        state.cloud_dev_uniform_step = ustep;
        changed = true;
    }
    if state.cloud_dev_uniform_step {
        let mut sm = state.cloud_dev_step_m;
        test_mark(ui, theme, "fixed step m (0 = off)", &tests);
        if dev_slider(ui, &mut sm, 0.0..=600.0, "fixed step m (0 = off)") {
            state.cloud_dev_step_m = sm;
            changed = true;
        }
    }
    let mut wedge = state.cloud_dev_wide_edge;
    test_mark(ui, theme, "Wide cloud edge (~300 m ramp, radiative smoothing scale)", &tests);
    if ui
        .checkbox(&mut wedge, "Wide cloud edge (~300 m ramp, radiative smoothing scale)")
        .changed()
    {
        state.cloud_dev_wide_edge = wedge;
        changed = true;
    }

    if state.cloud_dev_wide_edge {
        let mut m = if state.cloud_dev_edge_mul > 0.0 { state.cloud_dev_edge_mul } else { 20.0 };
        test_mark(ui, theme, "edge width x (hinge)", &tests);
        if dev_slider(ui, &mut m, 1.0..=200.0, "edge width x (hinge)") {
            state.cloud_dev_edge_mul = m;
            changed = true;
        }
        let mut r = if state.cloud_dev_rind_wide_m > 0.0 { state.cloud_dev_rind_wide_m } else { 300.0 };
        test_mark(ui, theme, "body rind m", &tests);
        if dev_slider(ui, &mut r, 90.0..=1500.0, "body rind m") {
            state.cloud_dev_rind_wide_m = r;
            changed = true;
        }
    }

    let mut wsl = state.cloud_dev_world_shape_lod;
    test_mark(ui, theme, "World-anchored cloud shape (shape detail stops following camera distance)", &tests);
    if ui
        .checkbox(
            &mut wsl,
            "World-anchored cloud shape (shape detail stops following camera distance)",
        )
        .changed()
    {
        state.cloud_dev_world_shape_lod = wsl;
        changed = true;
    }

    let mut chord = state.cloud_dev_chord_foot;
    test_mark(ui, theme, "Old chord detail scale (on = pre-v0.1268, sun rosette returns)", &tests);
    if ui
        .checkbox(
            &mut chord,
            "Old chord detail scale (on = pre-v0.1268, sun rosette returns)",
        )
        .changed()
    {
        state.cloud_dev_chord_foot = chord;
        changed = true;
    }

    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new("Cloud resolution")
            .strong()
            .color(theme.accent()),
    );
    // The operator, v0.1254.4: "It seems like the cloud layer is
    // lower resolution than the surface layer, would upping the
    // resolution of the cloud layer help at all?" It IS lower -
    // the march has always run at a QUARTER of screen resolution
    // (one cloud sample per 4x4 screen pixels), which is exactly
    // the "solid pixels" look and the binary opaque-or-clear
    // edges. Terrain renders full-res beside it. Raising this
    // costs frames roughly with the pixel count, so it is a
    // slider for their machine to answer, not a guess for mine.
    ui.label(
        RichText::new(
            "The cloud march runs BELOW screen resolution - this is \
             why cloud edges read as blocky pixels next to full-res \
             terrain. Higher costs frames roughly with pixel count.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    let res_opts: [(u32, &str); 3] =
        [(4, "Quarter"), (2, "Half"), (1, "Full")];
    let mut res = state.cloud_dev_res_div.clamp(1, 4);
    ui.horizontal(|ui| {
        for (div, name) in res_opts {
            if ui.selectable_label(res == div, name).clicked() {
                res = div;
            }
        }
    });
    if res != state.cloud_dev_res_div {
        state.cloud_dev_res_div = res;
        // Persisted since v0.1285 (config cloud_res_div): the choice
        // survives a restart, so play and the rig read the same cost.
        state.settings_dirty = true;
        changed = true;
    }

    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new("Bisect channel (grayscale forensics)")
            .strong()
            .color(theme.accent()),
    );
    ui.label(
        RichText::new(
            "Renders one raw ingredient of every cloud pixel instead \
             of the finished image - the instrument that cracked the \
             rosette. Off = normal rendering.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    // The discard-reason bisect (v0.1262). This checkbox was
    // MISSED when the feature shipped - the plumbing landed but the
    // edit script aborted before the UI insert, so the operator had
    // the flag with no way to reach it. GUI-first means the button
    // ships in the same commit as the flag, every time.
    let mut dsc = state.cloud_dev_discard_diag;
    test_mark(ui, theme, "Discard reasons (why a pixel's cloud was killed)", &tests);
    if ui
        .checkbox(&mut dsc, "Discard reasons (why a pixel's cloud was killed)")
        .changed()
    {
        state.cloud_dev_discard_diag = dsc;
        changed = true;
    }
    ui.label(
        RichText::new(
            "grey = the march found no cloud, GREEN = terrain in                      front, RED = planet horizon cull, blue = ray missed                      the shell, purple = behind camera, orange = empty slab.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_sm);
    // "March steps" / "Step comb" (v0.1268): the iteration-count
    // instrument, rebuilt. A ring at a fixed screen radius in a
    // channel that has NOTHING to do with lighting or density can
    // only come from the march schedule itself.
    // (value, label): the VALUE is what the shader's map_diag
    // ladder in 45-cloud-temporal.wgsl reads. 7 has no channel
    // there (burial is `diag >= 7.5`, i.e. 8), so the old "Burial"
    // button at index 7 was showing entry depth; the values are
    // explicit now so a gap cannot mislabel a channel again.
    // 9 = "Sun source" (v0.1286): which sun-shadow source each
    // cloud pixel read - fine window white, coarse grey, analytic
    // column dark - so the window EDGES are visible where a ring
    // in the diff would sit.
    let names: [(i32, &str); 9] = [
        (0, "Off"),
        (1, "Coverage alpha"),
        (2, "Direct sun"),
        (3, "Ambient"),
        (4, "March steps"),
        (5, "Step comb"),
        (6, "Entry depth"),
        (8, "Burial"),
        (9, "Sun source"),
    ];
    let mut pick = state.cloud_dev_map_diag.clamp(0, 9);
    ui.horizontal_wrapped(|ui| {
        for (value, name) in names.iter() {
            if ui.selectable_label(pick == *value, *name).clicked() {
                pick = *value;
            }
        }
    });
    if pick != state.cloud_dev_map_diag {
        state.cloud_dev_map_diag = pick;
        changed = true;
    }
    if pick >= 4 && pick <= 5 {
        ui.label(
            RichText::new(
                "Black = few steps, white = this ray spent the whole                          224-step budget and integrated its remaining tail in                          ONE giant sample. Step comb bands every 8 steps.",
            )
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
        );
    }
    if pick == 9 {
        ui.label(
            RichText::new(
                "White = the fine cache window, grey = the coarse                          window, dark = the analytic column beyond both.                          With the cache off every pixel is dark.",
            )
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
        );
    }
    changed
}
