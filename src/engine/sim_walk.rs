//! `HumanityOS --sim walk --route FILE`: the homestead entered and walked with no window, no event
//! loop and no wgpu Instance (the headless sim runner, increment 4, 2026-11-15;
//! docs/design/headless-sim-runner.md). The engine is booted as the windowed app boots it,
//! `EngineState::boot(opts, None)` (engine/engine_boot.rs), the world entered through the same
//! `world_load::load_world` (its drawing skipped and named, `state.gpu_dirty`), and each step is
//! `frame_step::run`, the step the windowed frame takes, on a clock this driver advances 1/60 s at a
//! time. The route is the storeys rig's (scripts/sim-walk-route.js writes it from the plan
//! scripts/verify-storeys.js walks): each leg placed as the rig's `cam` + `walk` verbs place it,
//! turned as its `face` verb turns, keys held through the same `engine::input::apply_game_action` a
//! key press goes through, timed in simulation seconds. The report says, leg by leg, where the body
//! went, the floors it stood on, the rooms the HUD would name, how fast it moved, the holds a wall
//! cut short and whether it crossed the leg's wall line, read from the recorder's own block
//! (`storey_probe::frame_json`), as the rig's judge reads a windowed run.
//!
//! Safety: saves go to a scratch folder removed afterwards, no config is read or written, and none
//! of the app's outside services start (`BootOptions::services` false).
//!
//! `--without-wall X,Z` takes the wall segments within 0.6 m of home point (X, Z) out of the walk's
//! collision after entry: the red run for the walk's own check (a leg into that wall must then cross
//! its line).

use crate::engine::frame_prelude::*;
use humanity_core::input::bindings::GameAction;
use std::path::{Path, PathBuf};

/// The simulation step, seconds: the windowed game's at 60 frames a second.
const STEP_S: f32 = 1.0 / 60.0;
/// The rig's walking constants (scripts/verify-storeys.js): the speed it plans holds by, how close
/// is arrived, how many holds a point may take, how little movement is stuck, how long the walk
/// records after a hold lets go, and after a placement, and standing after a route.
const WALK_M_S: f32 = 5.0;
const ARRIVE_M: f32 = 0.25;
const MAX_PULSES: u32 = 10;
const STUCK_M: f32 = 0.03;
const PULSE_TAIL_S: f32 = 0.8;
const SETTLE_S: f32 = 1.5;
const AFTER_S: f32 = 1.5;

#[derive(serde::Deserialize)]
struct Route {
    name: String,
    eye_m: f32,
    legs: Vec<RouteLeg>,
}

#[derive(serde::Deserialize)]
struct RouteLeg {
    id: String,
    place: Option<[f32; 2]>,
    #[serde(default)]
    floor: f32,
    #[serde(default)]
    yaw: f32,
    steps: Vec<RouteStep>,
    wall: Option<Line>,
}

#[derive(serde::Deserialize, Clone, Copy)]
struct Line {
    a: [f32; 2],
    b: [f32; 2],
}

#[derive(serde::Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
enum RouteStep {
    Walk { to: Vec<[f32; 2]> },
    Hold { keys: String, s: f32, after: f32 },
    FaceMid { a: [f32; 2], b: [f32; 2] },
}

/// One leg walked: what the body did, in home metres.
#[derive(Default)]
struct LegReport {
    id: String,
    start: [f32; 3],
    end: [f32; 3],
    /// Every floor height stood on, in the order first met (metres, to the centimetre).
    floors: Vec<f32>,
    /// Every room the HUD would have named, in the order met.
    rooms: Vec<String>,
    /// The fastest the body crossed the floor in one step, m/s.
    max_speed: f32,
    /// The highest the eye rose, home metres (a jump's peak, a ride's top).
    peak_eye: f32,
    /// Holds a wall cut short: where the body stopped and how far of the free walk it got.
    walls_met: Vec<String>,
    /// For a leg with a wall line: whether the eye ever crossed it.
    crossed_wall: Option<bool>,
    error: Option<String>,
    steps: u64,
}

/// The engine booted with no GPU and the homestead entered: the walk's world.
struct Walk {
    state: EngineState,
    origin: Vec3,
    eye: f32,
    frame: FrameVars,
}

impl Walk {
    /// One step of the world: the clock on by `STEP_S`, then the windowed frame's step.
    fn step(&mut self) {
        self.state.clock.advance_by(STEP_S);
        crate::engine::frame_step::run(&mut self.state, &mut self.frame);
    }

    /// The eye in home metres.
    fn here(&self) -> [f32; 3] {
        let p = self.state.camera.position - self.origin;
        [p.x, p.y, p.z]
    }

    /// Step for `s` seconds, recording into `leg`.
    fn run_for(&mut self, s: f32, leg: &mut LegReport, wall: Option<Line>) {
        let n = (s / STEP_S).round().max(1.0) as u32;
        for _ in 0..n {
            let before = self.here();
            self.step();
            self.record(leg, before, wall);
        }
    }

    /// What the rig's recorder reads this frame (`storey_probe::frame_json`), into the leg.
    fn record(&mut self, leg: &mut LegReport, before: [f32; 3], wall: Option<Line>) {
        leg.steps += 1;
        let now = self.here();
        let block = crate::engine::storey_probe::frame_json(&self.state);
        if let Some(floor) = block.get("floor").and_then(|v| v.as_f64()) {
            let f = ((floor as f32 - self.origin.y) * 100.0).round() / 100.0;
            if !leg.floors.contains(&f) {
                leg.floors.push(f);
            }
        }
        if let Some(id) = block.get("room").and_then(|r| r.get("id")).and_then(|v| v.as_str()) {
            if leg.rooms.last().map(String::as_str) != Some(id) && !leg.rooms.iter().any(|r| r == id) {
                leg.rooms.push(id.to_string());
            }
        }
        let moved = ((now[0] - before[0]).powi(2) + (now[2] - before[2]).powi(2)).sqrt();
        leg.max_speed = leg.max_speed.max(moved / STEP_S);
        leg.peak_eye = leg.peak_eye.max(now[1]);
        if let (Some(w), Some(c)) = (wall, leg.crossed_wall.as_mut()) {
            if side_of(w, [now[0], now[2]]) != side_of(w, [leg.start[0], leg.start[2]]) {
                *c = true;
            }
        }
        leg.end = now;
    }

    /// The rig's `cam` + `walk` verbs: the eye at home point `p` over `floor`, facing `yaw`, on foot.
    fn place(&mut self, p: [f32; 2], floor: f32, yaw: f32) {
        let w = self.origin + Vec3::new(p[0], floor + self.eye, p[1]);
        for (_e, (t, _c)) in self.state.game_world.world.query_mut::<(&mut Transform, &Controllable)>() {
            t.position = w;
        }
        self.state.camera.position = w;
        self.state.camera.yaw = yaw;
        self.state.camera.pitch = 0.0;
        self.state.gui_state.dev_fly_mode = false;
        self.state.gui_state.dev_hover = false;
        self.state.controller.fly_mode = false;
    }

    /// Keys down (true) or up, through the path a key press takes.
    fn keys(&mut self, keys: &[GameAction], down: bool) {
        for k in keys {
            crate::engine::input::apply_game_action(&mut self.state.controller, &mut self.state.probe_hold, *k, down);
        }
    }

    /// The rig's walkRoute: to each point in turn, then standing.
    fn walk_route(&mut self, to: &[[f32; 2]], leg: &mut LegReport) -> Result<(), String> {
        for target in to {
            let mut pulses = 0;
            loop {
                let h = self.here();
                let d = ((target[0] - h[0]).powi(2) + (target[1] - h[2]).powi(2)).sqrt();
                if d <= ARRIVE_M {
                    break;
                }
                if pulses >= MAX_PULSES {
                    return Err(format!("still {d:.2} m from ({}, {}) after {MAX_PULSES} holds", target[0], target[1]));
                }
                pulses += 1;
                self.state.camera.yaw = yaw_to([h[0], h[2]], *target);
                let s = ((d - 0.05) / WALK_M_S).max(0.04);
                let from = self.here();
                self.hold(&[GameAction::MoveForward], s, PULSE_TAIL_S, leg, None);
                let to_now = self.here();
                let moved = ((to_now[0] - from[0]).powi(2) + (to_now[2] - from[2]).powi(2)).sqrt();
                if moved < STUCK_M {
                    return Err(format!(
                        "blocked at ({:.2}, {:.2}, {:.2}) walking toward ({}, {}): forward held {s:.2} s moved the body {moved:.3} m",
                        to_now[0], to_now[1], to_now[2], target[0], target[1]
                    ));
                }
            }
        }
        self.run_for(AFTER_S, leg, None);
        Ok(())
    }

    /// Hold `keys` for `s` seconds, recording `s + after`; a forward hold the walls cut short is noted.
    fn hold(&mut self, keys: &[GameAction], s: f32, after: f32, leg: &mut LegReport, wall: Option<Line>) {
        let from = self.here();
        self.keys(keys, true);
        self.run_for(s, leg, wall);
        self.keys(keys, false);
        self.run_for(after, leg, wall);
        if keys == [GameAction::MoveForward] {
            let to = self.here();
            let moved = ((to[0] - from[0]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
            let free = self.state.controller.speed * s;
            if moved < 0.5 * free {
                leg.walls_met.push(format!("stopped at ({:.2}, {:.2}) after {moved:.2} m of a {free:.2} m free walk", to[0], to[2]));
            }
        }
    }

    fn walk_leg(&mut self, rl: &RouteLeg) -> LegReport {
        if let Some(p) = rl.place {
            self.place(p, rl.floor, rl.yaw);
            let mut settle = LegReport::default();
            self.run_for(SETTLE_S, &mut settle, None);
        }
        let h = self.here();
        let mut leg = LegReport { id: rl.id.clone(), start: h, end: h, peak_eye: h[1], crossed_wall: rl.wall.map(|_| false), ..Default::default() };
        for st in &rl.steps {
            let done = match st {
                RouteStep::Walk { to } => self.walk_route(to, &mut leg),
                RouteStep::Hold { keys, s, after } => match crate::engine::rig_walk::parse_keys(keys) {
                    Some(k) => {
                        self.hold(&k, *s, *after, &mut leg, rl.wall);
                        Ok(())
                    }
                    None => Err(format!("hold names keys the walk does not know: {keys:?}")),
                },
                RouteStep::FaceMid { a, b } => {
                    let h = self.here();
                    self.state.camera.yaw = yaw_to([h[0], h[2]], [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
                    Ok(())
                }
            };
            if let Err(e) = done {
                leg.error = Some(e);
                break;
            }
        }
        leg
    }
}

/// The rig's yaw toward a point: 0 north (-Z), +PI/2 east (storeys-judge.js `yawTo`).
fn yaw_to(p: [f32; 2], q: [f32; 2]) -> f32 {
    (q[0] - p[0]).atan2(-(q[1] - p[1]))
}

/// Which side of the line a point is on (the sign of the cross product).
fn side_of(l: Line, p: [f32; 2]) -> bool {
    (l.b[0] - l.a[0]) * (p[1] - l.a[1]) - (l.b[1] - l.a[1]) * (p[0] - l.a[0]) >= 0.0
}

/// The walk's report.
pub(crate) struct WalkReport {
    route: String,
    legs: Vec<LegReport>,
    not_drawn: Vec<&'static str>,
    entry_s: f64,
    walk_s: f64,
    removed_walls: usize,
}

/// Boot with no GPU, enter the homestead and walk `route`.
pub(crate) fn walk(data_dir: &Path, route_path: &Path, without_wall: Option<[f32; 2]>) -> Result<WalkReport, String> {
    let route: Route = serde_json::from_str(&std::fs::read_to_string(route_path).map_err(|e| format!("{}: {e}", route_path.display()))?)
        .map_err(|e| format!("{}: {e}", route_path.display()))?;
    let saves = std::env::temp_dir().join(format!("humanityos-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&saves);
    std::fs::create_dir_all(&saves).map_err(|e| format!("{}: {e}", saves.display()))?;
    let t0 = std::time::Instant::now();
    let mut state = EngineState::boot(
        crate::engine::engine_boot::BootOptions {
            data_dir: data_dir.to_path_buf(),
            saves_dir: saves.clone(),
            config: None,
            services: false,
            boot_timer: humanity_core::boot_timing::BootTimer::new(t0),
            egui_ctx: egui::Context::default(),
        },
        None,
    );
    crate::engine::world_load::load_world(&mut state);
    // The person steps into the world (the systems run from the first step, BUG-253), as
    // sim_home.rs's homestead does.
    state.gui_state.enter_world_if(true);
    let ship = state.gui_state.ship_structure.as_ref().ok_or("the world entered with no ship structure: no homestead aboard")?;
    let (origin, _) = crate::engine::home_plot::home_box(ship).ok_or("the homestead has no home box")?;
    let mut removed_walls = 0;
    if let Some(p) = without_wall {
        let at = (origin.x + p[0], origin.z + p[1]);
        let before = state.wall_colliders.len();
        state.wall_colliders.retain(|w| segment_distance(w.a, w.b, at) > 0.6);
        removed_walls = before - state.wall_colliders.len();
    }
    let mut walk = Walk { state, origin, eye: route.eye_m, frame: FrameVars::new(std::time::Instant::now()) };
    // The home is entered a few frames before anything is judged (the rig waits 6 s after entry).
    let mut entry = LegReport::default();
    walk.run_for(1.0, &mut entry, None);
    let entry_s = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();
    let legs = route.legs.iter().map(|rl| walk.walk_leg(rl)).collect();
    let walk_s = t1.elapsed().as_secs_f64();
    let not_drawn = walk.state.gpu_dirty.clone();
    drop(walk);
    let _ = std::fs::remove_dir_all(&saves);
    Ok(WalkReport { route: route.name, legs, not_drawn, entry_s, walk_s, removed_walls })
}

/// The distance from point `p` to the segment a..b, metres.
fn segment_distance(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cz) = (a.0 + t * dx, a.1 + t * dz);
    ((p.0 - cx).powi(2) + (p.1 - cz).powi(2)).sqrt()
}

impl WalkReport {
    /// The legs that failed the walk's own checks: a wall line crossed, or a leg that could not go on.
    fn failed(&self) -> Vec<&str> {
        self.legs.iter().filter(|l| l.crossed_wall == Some(true) || l.error.is_some()).map(|l| l.id.as_str()).collect()
    }

    pub(crate) fn text(&self) -> String {
        let steps: u64 = self.legs.iter().map(|l| l.steps).sum();
        let mut s = format!(
            "Headless walk of the {} route: {} legs, {steps} steps of 1/60 s ({:.1} s of the world) in {:.2} s of wall time ({:.0}x real time); the engine booted and the homestead entered in {:.2} s, with no window and no GPU.\n",
            self.route,
            self.legs.len(),
            steps as f64 / 60.0,
            self.walk_s,
            steps as f64 / 60.0 / self.walk_s.max(1e-9),
            self.entry_s
        );
        if self.removed_walls > 0 {
            s += &format!("RED RUN: {} wall segment(s) taken out of the walk's collision.\n", self.removed_walls);
        }
        for l in &self.legs {
            s += &format!(
                "  {:<20} end ({:.2}, {:.2}, {:.2})  peak eye {:.2}  floors {}  rooms {}  max {:.2} m/s{}{}{}\n",
                l.id,
                l.end[0],
                l.end[1],
                l.end[2],
                l.peak_eye,
                l.floors.iter().map(|f| format!("{f:.2}")).collect::<Vec<_>>().join("|"),
                if l.rooms.is_empty() { "none".to_string() } else { l.rooms.join(">") },
                l.max_speed,
                l.crossed_wall.map_or(String::new(), |c| if c { "  CROSSED its wall line".into() } else { "  stayed on its side of the wall line".into() }),
                if l.walls_met.is_empty() { String::new() } else { format!("  wall: {}", l.walls_met.join("; ")) },
                l.error.as_ref().map_or(String::new(), |e| format!("  STOPPED: {e}")),
            );
        }
        let failed = self.failed();
        s += &if failed.is_empty() { "Every leg stayed on its side of its wall line and went on to its end.\n".to_string() } else { format!("FAILED: {}.\n", failed.join(", ")) };
        s += &format!("Not drawn (no GPU): {}.\n", if self.not_drawn.is_empty() { "nothing".to_string() } else { self.not_drawn.join("; ") });
        s
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "route": self.route,
            "entry_s": self.entry_s,
            "walk_s": self.walk_s,
            "removed_walls": self.removed_walls,
            "not_drawn": self.not_drawn,
            "failed": !self.failed().is_empty(),
            "legs": self.legs.iter().map(|l| serde_json::json!({
                "id": l.id, "start": l.start, "end": l.end, "floors": l.floors, "rooms": l.rooms,
                "max_speed": l.max_speed, "peak_eye": l.peak_eye, "walls_met": l.walls_met, "crossed_wall": l.crossed_wall,
                "error": l.error, "steps": l.steps,
            })).collect::<Vec<_>>(),
        })
    }
}

/// `--sim walk` from the command line: (the report's text, its JSON).
pub(crate) fn cli(args: &[String], data_dir: &Path) -> Result<(String, serde_json::Value), String> {
    let route: PathBuf = args
        .iter()
        .position(|a| a == "--route")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .ok_or("--sim walk needs --route FILE (node scripts/sim-walk-route.js --out FILE writes the storeys rig's)")?;
    let without_wall = match args.iter().position(|a| a == "--without-wall") {
        None => None,
        Some(i) => {
            let v: Vec<f32> = args.get(i + 1).map(|s| s.split(',').filter_map(|p| p.trim().parse().ok()).collect()).unwrap_or_default();
            if v.len() != 2 {
                return Err("--without-wall needs X,Z (home metres)".into());
            }
            Some([v[0], v[1]])
        }
    };
    let report = walk(data_dir, &route, without_wall)?;
    Ok((report.text(), report.json()))
}
