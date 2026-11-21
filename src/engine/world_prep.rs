//! When the 3D world is prepared: the desktop starts PAGES-FIRST, and builds the world when Play is
//! pressed (the default) or beside the pages, gradually (2026-11-20; docs/design/platforms.md A1,
//! renderer/world_build.rs).
//!
//! WHY. The operator, 2026-11-20, verbatim: "Can we skip building the 3D world's graphics
//! immediately? Could we draw it out over several seconds to get people in chat faster?" Measured
//! that day: a desktop waited 18.6 s from the window to its first frame, a pages-only start under a
//! second. So the desktop now starts the way the phone does (only the device and the surface, the
//! pages drawn at once, chat first). The world's renderer, the star sky and the planet surfaces are
//! built on worker threads when Play is pressed: THE DEFAULT, the operator's the same day: "I
//! figure the all at start could be an option, but I'd like to default to having to press play from
//! now on. That way we can test non-game stuff faster." ([`Choice::OnPlay`]). Or, by choice, right
//! after the first page frame, below normal priority, while the pages draw ([`Choice::Background`]):
//! his standing rule (memory feedback_gradual_background_work) holds there, background preparation
//! runs gradually under a per-frame budget, never in one burst.
//!
//! THE FRAME'S SHARE ([`ON_FRAME_BUDGET`], 2 ms). All the building runs off the frame; what the frame
//! does is look whether it is done, and once, install the finished renderer (it takes the window's
//! surface over from the pages). [`frame`] times exactly that, every frame, and the time is logged
//! (`[WorldPrep] on-frame work ...`) so a rig judges it (scripts/lib/phone-start-judge.js).
//!
//! PLAY BEFORE IT IS READY ([`hold_entry`]), the default start's every first Play: the preparation
//! starts (or, beside the pages, its politeness ends) and runs flat out OFF the frame, the page stays
//! and shows a card with the step under way in words and how far along it is, and the world opens by
//! itself when everything is ready; the window never freezes on it. After it is done, entering costs
//! only what loading your home costs (world_load.rs). The rigs enter the same way (the dev
//! autopilot's entry answers once the world has loaded, engine/ipc.rs), so none of them sleeps.
//!
//! THE CHOICE is the person's: Settings > Graphics > "Prepare the 3D world" ([`Choice`]), read at
//! the start (it applies at the next one). Dev switches: `HUMANITY_WORLD_PREP=background|on_play|
//! at_start` overrides it; `HUMANITY_PAGES_FIRST=1` is "only when I press Play" (the rig switch of
//! 2026-11-20, kept) and `HUMANITY_PAGES_FIRST=0` "at start, before the pages" (the old order, for
//! comparison). A phone never builds the world (Play's Enter is a notice there, rung A1).

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::engine::state::EngineState;
use crate::gui::GuiPage;
use crate::renderer::world_build::Step;

/// The frame's share of the world's preparation: looking whether it is done and installing it.
pub(crate) const ON_FRAME_BUDGET: Duration = Duration::from_millis(2);

/// How often the page frames' timing is logged while the world is not entered.
const WINDOW: Duration = Duration::from_secs(5);
/// At most this many such lines a run.
const MAX_WINDOWS: u32 = 16;
/// A gap between two page frames this long is logged with what was happening.
const LONG_GAP: Duration = Duration::from_millis(100);
/// At most this many such lines a run.
const MAX_GAPS: u32 = 24;

// The choice and this start's times live with the pages (the gui layer may not reach the engine).
pub(crate) use crate::gui::world_prep_choice::{choice, init, Choice};

/// Page frames' spacing over a stretch: how many, the mean and the worst gap between two.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Frames {
    pub(crate) count: u32,
    pub(crate) sum: Duration,
    pub(crate) worst: Duration,
}

impl Frames {
    fn push(&mut self, d: Duration) {
        self.count += 1;
        self.sum += d;
        self.worst = self.worst.max(d);
    }
    fn mean_ms(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum.as_secs_f64() * 1000.0 / self.count as f64 }
    }
}

/// The world's preparation as the engine keeps it between frames (the module docs).
#[derive(Default)]
pub(crate) struct WorldPrep {
    /// When the preparation began (the background start, or Play).
    started: Option<Instant>,
    /// Everything is ready: the renderer installed, the star sky and the planet surfaces baked.
    ready: bool,
    /// Play was pressed before it was ready: the world opens by itself when it is.
    pub(crate) pending_enter: bool,
    /// The page to show while it gets ready.
    back_page: Option<GuiPage>,
    /// When the person asked for the world, for the log.
    asked_at: Option<Instant>,
    /// The star sky's and the planets' prebuilds have reported (their results wait in their
    /// channels, re-wrapped, for world_load.rs).
    star_done: bool,
    planets_done: bool,
    /// The frame's own share, worst and count, while the preparation ran (the module docs).
    on_frame_worst: Duration,
    on_frame_frames: u32,
    /// The page frames' spacing, this window and the whole build.
    last_frame: Option<Instant>,
    window: Frames,
    window_start: Option<Instant>,
    windows_logged: u32,
    gaps_logged: u32,
    /// The last frame's whole length and its present's, and when it ended ([`note_frame_end`]).
    last_handler: Duration,
    last_present: Duration,
    last_end: Option<Instant>,
    during: Frames,
    /// The dev autopilot's answer to an entry, written once the world has loaded (engine/ipc.rs).
    pub(crate) autopilot_done: Option<(serde_json::Value, Instant)>,
    /// When a background preparation begins ([`BACKGROUND_DELAY`] after the first page frame).
    background_due: Option<Instant>,
}

/// After the first page frame (engine/redraw.rs): a background start begins preparing the world.
pub(crate) fn after_first_frame(state: &mut EngineState) {
    crate::gui::world_prep_choice::note_pages(state.boot_timer.boot_start.elapsed().as_millis() as u64);
    if choice() == Choice::Background && state.gpu.is_some() {
        state.world_prep.background_due = Some(Instant::now() + BACKGROUND_DELAY);
    }
}

/// How long after the first page frame a background preparation begins. Measured 2026-11-20: begun
/// on the first frame itself, the rest of that frame took 970 ms (a 1,084 ms gap to the next), the
/// first frame being the one that makes egui's textures and buffers; begun a second later the worst
/// gap in the build's first steps was 489 ms, on a PC busier with other compiles. What in those
/// first steps holds a page frame up is NOT settled: the long-gap lines (`[WorldPrep] a page frame
/// ...`) split each gap into the frame's own time, its present and the time between frames, for
/// whoever takes it further.
const BACKGROUND_DELAY: Duration = Duration::from_secs(1);

/// Whether the frame's world stages run (frame_sky.rs, frame_lists_world.rs and the rest): the
/// world's renderer built AND the world entered. A page covers the world until then, so a renderer
/// prepared beside the pages costs the page frames nothing before Play. Measured 2026-11-20: the
/// first page frame after the background install spent 189 ms in those stages' first run.
pub(crate) fn drawn(state: &EngineState) -> bool {
    state.world_loaded && state.gpu.as_ref().is_some_and(|g| g.renderer.world_built())
}

/// At the start, a run that builds before the pages: the world's renderer is built already
/// (renderer_slot.rs); the star sky and the planet surfaces start beside the rest of the boot.
pub(crate) fn at_boot(state: &mut EngineState) {
    if choice() == Choice::AtStart && state.gpu.is_some() {
        start_prebuilds(state);
        state.world_prep.started = Some(state.boot_timer.boot_start);
    }
}

/// Begin preparing the world (no-op when begun): the renderer on a worker, the star sky and the
/// planet surfaces on their threads. `polite`: beside the pages, below normal priority.
fn start(state: &mut EngineState, polite: bool, reason: &str) {
    if state.world_prep.started.is_some() {
        if !polite {
            crate::renderer::world_build::set_polite(false);
        }
        return;
    }
    state.world_prep.started = Some(Instant::now());
    log::info!(
        "[WorldPrep] preparing the 3D world for {reason}, {} ({} ms after the window opened)",
        if polite { "beside the pages, paced, below normal priority" } else { "flat out" },
        state.boot_timer.boot_start.elapsed().as_millis()
    );
    if let Some(gpu) = state.gpu.as_ref() {
        gpu.renderer.start_background(polite, reason);
    }
    start_prebuilds(state);
}

/// Whether the world may be loaded this frame (frame_load_screens.rs, at the lazy load): yes when
/// everything is ready or there is no view; otherwise Play waits for it ([`hold_entry`]) and no.
pub(crate) fn ready_to_enter(state: &mut EngineState, page_before: GuiPage) -> bool {
    if state.gpu.is_none() || state.world_prep.ready {
        state.world_prep.pending_enter = false;
        return true;
    }
    hold_entry(state, page_before);
    false
}

/// Play before the world is ready: finish it flat out, keep the page with the progress card, and
/// open the world when it is ready (the module docs).
fn hold_entry(state: &mut EngineState, page_before: GuiPage) {
    start(state, false, "entering the world");
    crate::renderer::world_build::set_polite(false);
    let wp = &mut state.world_prep;
    if !wp.pending_enter {
        wp.pending_enter = true;
        wp.asked_at = Some(Instant::now());
        // The page Play was pressed on: the frame before's, or the step the history is on (Play
        // sets the page to the world in the frame it is pressed, before this one began).
        let history = state.gui_state.history.current().map(|p| p.page).filter(|p| *p != GuiPage::None);
        wp.back_page = Some(if page_before == GuiPage::None { history.unwrap_or(GuiPage::Play) } else { page_before });
        log::info!("[WorldPrep] Play pressed before the world was ready: finishing it now, the page shows how far along it is");
    }
    // Back on that page through the history (Replace: the step it is already on), so Back and
    // Escape still know where they are (docs/design/navigation.md 10.2).
    let back = wp.back_page.unwrap_or(GuiPage::Play);
    state.gui_state.go(back, crate::gui::How::Replace);
}

/// Every frame, before the lazy load (frame_load_screens.rs): install the finished renderer, note
/// the prebuilds, time the frame's share and the page frames' spacing, and open the world when Play
/// was waiting for it.
pub(crate) fn frame(state: &mut EngineState, frame_t0: Instant) {
    if state.gpu.is_none() || state.world_loaded {
        return;
    }
    let now = Instant::now();
    note_spacing(state, now, frame_t0);
    if state.world_prep.ready {
        return;
    }
    if state.world_prep.background_due.is_some_and(|due| now >= due) {
        state.world_prep.background_due = None;
        start(state, true, "the background, after the first page frames");
    }
    let Some(started) = state.world_prep.started else {
        return;
    };
    // ── The frame's share: everything from here to `on_frame` (the module docs) ──
    let t0 = Instant::now();
    let installed = state.gpu.as_mut().is_some_and(|g| g.renderer.poll_world());
    let built = state.gpu.as_ref().is_some_and(|g| g.renderer.world_built());
    if !state.world_prep.star_done {
        state.world_prep.star_done = peek(&mut state.star_preload_rx);
    }
    if !state.world_prep.planets_done {
        state.world_prep.planets_done = peek(&mut state.planet_defs_preload_rx);
    }
    let on_frame = t0.elapsed();
    let wp = &mut state.world_prep;
    wp.on_frame_worst = wp.on_frame_worst.max(on_frame);
    wp.on_frame_frames += 1;
    if installed {
        log::info!("[WorldPrep] the world's renderer installed on the frame in {:.2} ms", on_frame.as_secs_f64() * 1000.0);
    }
    if built && wp.star_done && wp.planets_done {
        wp.ready = true;
        let at = state.boot_timer.boot_start.elapsed();
        crate::gui::world_prep_choice::note_ready(at.as_millis() as u64);
        log::info!(
            "[WorldPrep] the 3D world is ready: {} ms after the window opened, {} ms after its preparation began",
            at.as_millis(),
            started.elapsed().as_millis()
        );
        log::info!(
            "[WorldPrep] on-frame work while it was prepared: worst {:.2} ms over {} frames (budget {} ms)",
            wp.on_frame_worst.as_secs_f64() * 1000.0,
            wp.on_frame_frames,
            ON_FRAME_BUDGET.as_millis()
        );
        log::info!(
            "[WorldPrep] page frames while it was prepared: {}, {:.1} ms apart on average, worst {:.1} ms",
            wp.during.count,
            wp.during.mean_ms(),
            wp.during.worst.as_secs_f64() * 1000.0
        );
        state.boot_timer.record_background("world_prepared_after_open", at);
        state.boot_timer.record_background("world_prepared_in", started.elapsed());
        if wp.pending_enter {
            wp.pending_enter = false;
            if let Some(asked) = wp.asked_at {
                log::info!("[WorldPrep] opening the world {} ms after Play was pressed", asked.elapsed().as_millis());
            }
            // The page Play left: the world now (the lazy load below runs this frame).
            state.gui_state.enter_world();
        }
    }
}

/// The page frames' spacing: this frame's gap from the last, into the window and, while the world is
/// prepared, into the build's total; a line every [`WINDOW`].
fn note_spacing(state: &mut EngineState, now: Instant, frame_t0: Instant) {
    let step = state.gpu.as_ref().and_then(|g| g.renderer.build_step());
    let wp = &mut state.world_prep;
    let building = wp.started.is_some() && !wp.ready;
    if let Some(last) = wp.last_frame {
        let gap = now - last;
        wp.window.push(gap);
        if building {
            wp.during.push(gap);
        }
        // A long gap, named with what the preparation was doing, so a hitch can be put down to its
        // cause (the first frames' own work, a build step, the install) rather than guessed at.
        if gap >= LONG_GAP && wp.gaps_logged < MAX_GAPS {
            wp.gaps_logged += 1;
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            log::info!(
                "[WorldPrep] a page frame {:.0} ms after the last ({} ms after the window opened; the frame before took {:.0} ms, {:.0} of them presenting, then {:.0} ms between frames, and this one {:.0} ms to here); the world: {}",
                ms(gap),
                state.boot_timer.boot_start.elapsed().as_millis(),
                ms(wp.last_handler),
                ms(wp.last_present),
                wp.last_end.map_or(0.0, |e| ms(frame_t0.saturating_duration_since(e))),
                ms(now - frame_t0),
                match (wp.ready, wp.started.is_some(), step) {
                    (true, _, _) => "ready".to_string(),
                    (false, true, Some(s)) => format!("being prepared, step {}", s.key()),
                    (false, true, None) => "being prepared, the renderer built".to_string(),
                    (false, false, _) => "not started".to_string(),
                }
            );
        }
    }
    wp.last_frame = Some(now);
    let start = *wp.window_start.get_or_insert(now);
    if now - start >= WINDOW && wp.windows_logged < MAX_WINDOWS {
        let w = std::mem::take(&mut wp.window);
        log::info!(
            "[WorldPrep] page frames in the last {:.1} s: {}, {:.1} ms apart on average, worst {:.1} ms; the world: {}",
            (now - start).as_secs_f32(),
            w.count,
            w.mean_ms(),
            w.worst.as_secs_f64() * 1000.0,
            if wp.ready { "ready" } else if wp.started.is_some() { "being prepared" } else { "not started" }
        );
        wp.window_start = Some(now);
        wp.windows_logged += 1;
    }
}

/// The frame ended (frame_tail.rs): its whole length from the handler's start, and its present's.
pub(crate) fn note_frame_end(state: &mut EngineState, frame_t0: Instant, present: Duration) {
    let wp = &mut state.world_prep;
    wp.last_handler = frame_t0.elapsed();
    wp.last_present = present;
    wp.last_end = Some(Instant::now());
}

/// Whether a prebuild's channel has its result (or will never have one), never waiting: a result is
/// put back in a fresh channel for world_load.rs to receive as before.
fn peek<T>(rx: &mut Option<Receiver<T>>) -> bool {
    let Some(r) = rx.as_ref() else {
        return true;
    };
    match r.try_recv() {
        Ok(v) => {
            let (tx, fresh) = std::sync::mpsc::channel();
            let _ = tx.send(v);
            *rx = Some(fresh);
            true
        }
        Err(TryRecvError::Empty) => false,
        Err(TryRecvError::Disconnected) => true,
    }
}

/// The star sky and the planet surfaces, prebuilt on their own threads (v0.865 and 2026-10-18, moved
/// here from engine_boot.rs): the first world entry receives them (world_load.rs). Polite like the
/// renderer's build while the preparation runs beside the pages.
fn start_prebuilds(state: &mut EngineState) {
    let Some(gpu) = state.gpu.as_ref() else {
        return;
    };
    // ── The star sky (v0.865 boot speed): catalog read, vertex packing, the glow's PNG decode and
    // the GPU upload, 4.1 s measured on the world-entry path, none of it needing the main thread.
    if state.star_preload_rx.is_none() && state.star_renderer.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        let device = gpu.renderer.device.clone();
        let queue = gpu.renderer.queue.clone();
        // The scene's format from the surface's, without reaching for the world's renderer.
        let scene_format = crate::renderer::scene_target::scene_format_for(gpu.renderer.surface_format()); // display-format: the scene target's format is derived from it, as the world renderer derives it
        let sky_dir = state.data_dir.clone();
        let tier_setting = state.gui_state.settings.star_catalog_tier.clone();
        let ultra_glow = state.gui_state.settings.sky_glow_tier == "ultra";
        let spawned = std::thread::Builder::new().name("star-sky".into()).spawn(move || {
            crate::renderer::world_build::polite_thread();
            let t = Instant::now();
            let cap = crate::renderer::stars::StarCatalogTier::resolve_cap(&tier_setting);
            let built = crate::renderer::stars::StarCatalog::load(&sky_dir, cap).as_ref().and_then(|catalog| {
                crate::renderer::stars::StarRenderer::new(&device, &queue, scene_format, catalog, &sky_dir, ultra_glow && cap >= 2)
            });
            log::info!(
                "[Boot] star sky prebuilt in background: {} ms ({})",
                t.elapsed().as_millis(),
                if built.is_some() { "ok" } else { "unavailable" }
            );
            let _ = tx.send(built);
        });
        if spawned.is_ok() {
            state.star_preload_rx = Some(rx);
        }
    }
    // ── The planet surfaces (2026-10-18 boot speed): every data/planets/<id>.ron, its elevation and
    // colour grids and its baked surface texture, CPU work on files (net_route::bake_planet_defs). A
    // planet RON hot reload before world entry drops the receiver (reload_planet_defs), so a stale
    // prebuild is never installed.
    if state.planet_defs_preload_rx.is_none() && !state.world_loaded {
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = state.data_dir.clone();
        let spawned = std::thread::Builder::new().name("planet-defs".into()).spawn(move || {
            crate::renderer::world_build::polite_thread();
            let t = Instant::now();
            let bakes = crate::engine::net_route::bake_planet_defs(&dir);
            let took = t.elapsed();
            log::info!("[Boot] planet surfaces prebuilt in background: {} ms ({} bodies)", took.as_millis(), bakes.len());
            let _ = tx.send((bakes, took));
        });
        if spawned.is_ok() {
            state.planet_defs_preload_rx = Some(rx);
        }
    }
}

/// What the progress card shows: the step under way, how many of how many, and whether Play waits.
pub(crate) struct View {
    pub(crate) step_key: String,
    pub(crate) number: usize,
    pub(crate) total: usize,
}

/// The card's contents this frame, when Play is waiting for the world (else None).
pub(crate) fn view(state: &EngineState) -> Option<View> {
    let wp = &state.world_prep;
    if !wp.pending_enter || wp.ready {
        return None;
    }
    let step = state.gpu.as_ref().and_then(|g| g.renderer.build_step()).unwrap_or(if state.gpu.as_ref().is_some_and(|g| g.renderer.world_built()) { Step::Done } else { Step::Waiting });
    // After the renderer: the star sky and the planet surfaces, which run beside it.
    let total = Step::WORK + 2;
    let (key, number) = match step {
        Step::Done if !wp.star_done => ("stars", Step::WORK + 1),
        Step::Done => ("planets", Step::WORK + 2),
        s => (s.key(), s.number().max(1)),
    };
    Some(View { step_key: format!("world_prep.step.{key}"), number, total })
}

/// The progress card while Play waits for the world (the module docs): the step in words, a bar,
/// and a button that stays on the pages (the preparation goes on behind it).
pub(crate) fn draw(ctx: &egui::Context, theme: &crate::gui::theme::Theme, view: &View, state: &mut WorldPrep) {
    use crate::gui::widgets::hint::Hint;
    use humanity_core::i18n::{t, t_dyn, tf};
    let area = crate::gui::layers::area(ctx, egui::Id::new("world_prep_card"), crate::gui::layers::Layer::PageFloat)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0));
    area.show(ctx, |ui| {
        crate::gui::widgets::card(ui, theme, |ui| {
            ui.set_max_width(theme.spacing_md * 30.0);
            ui.label(egui::RichText::new(t("world_prep.title")).size(theme.font_size_heading).color(theme.text_primary()).strong());
            let step = t_dyn(&view.step_key);
            ui.label(egui::RichText::new(tf("world_prep.progress", &[("n", &view.number), ("total", &view.total), ("step", &step)])).color(theme.text_secondary()));
            ui.add(egui::ProgressBar::new(view.number.saturating_sub(1) as f32 / view.total.max(1) as f32).animate(true));
            ui.label(egui::RichText::new(t("world_prep.note")).size(theme.font_size_small).color(theme.text_muted()));
            if ui.button(t("world_prep.cancel")).hint("world_prep.cancel_hint").clicked() {
                state.pending_enter = false;
                log::info!("[WorldPrep] the person stayed on the pages; the world keeps getting ready behind them");
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prebuild_is_peeked_without_losing_its_result() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let mut slot = Some(rx);
        assert!(!peek(&mut slot), "nothing sent yet");
        tx.send(7).unwrap();
        assert!(peek(&mut slot), "sent");
        assert_eq!(slot.as_ref().unwrap().recv().unwrap(), 7, "the result waits for world_load.rs");
        let mut none: Option<Receiver<u32>> = None;
        assert!(peek(&mut none), "no prebuild: nothing to wait for");
        let (tx2, rx2) = std::sync::mpsc::channel::<u32>();
        drop(tx2);
        let mut dead = Some(rx2);
        assert!(peek(&mut dead), "a dead prebuild: world_load.rs bakes itself");
    }
}
