//! MOVIE MODE: record the live view to a video file, frame-exact (2026-09-30,
//! the clip maker, `scripts/make-clips.js`).
//!
//! WHY THIS AND NOT A SCREEN RECORDER. A recorder watching the window gets
//! whatever the game managed to draw in real time: every hitch is a stutter
//! in the video, and the window has to be on screen, over the operator's
//! work. Movie mode is the technique game engines use for trailers (Source's
//! `host_framerate` with `startmovie`, Unreal's Movie Render Queue): every
//! frame advances the simulation by exactly 1/fps, the finished frame is read
//! back from the GPU and piped into ffmpeg, and the next frame is not drawn
//! until the last one is written. However long a frame takes to render, the
//! video plays smoothly at its nominal rate, and the game can stay behind
//! every other window while it records.
//!
//! THE CLOCK. The simulation's step is replaced (`frame_step`), and so is the
//! wall clock the renderer animates from: water waves, cloud drift, wind sway
//! and the sky's spin all read `state.start_time.elapsed()`. Each frame slides
//! `start_time` so that elapsed time advances by exactly one step too, which
//! keeps every one of them in time with the video without touching any of
//! them. `anim_speed` scales that clock alone, for a time-lapse of moving
//! clouds; the game clock's own speed (sun, day) is the showcase `time_scale`.
//!
//! THE CAMERA. Three motions, which combine:
//!   - `path`: keyframes `[t, x, y, z, yaw, pitch]` in the home frame (the
//!     numbers scripts/home-vantages.json uses), joined by a Catmull-Rom curve:
//!     a dolly through the home. Re-applied every frame against the station's
//!     current offset, so the station moving in orbit cannot carry the camera
//!     off (the drift photograph-home.js documents).
//!   - `pan_yaw_deg` / `pan_pitch_deg`: turn the view over the clip, eased at
//!     both ends, the way mouse-look turns it.
//!   - `descend_half_life_s`: sink toward the ground at a speed proportional to
//!     the height left (`descend_floor_m` above the drawn ground), so the altitude
//!     halves every half-life: an orbit-to-ground drop that slows as the ground
//!     comes up. It drives the rig's own descent (`probe_descend_mps`, the
//!     `probe_hold` in lib.rs), so it needs a planet park by `camera_request`
//!     first and does nothing without a planet frame lock.
//!   With none of them the camera holds still and the world moves.
//!
//! THE PROTOCOL. `debug/record_request.json` (every field optional):
//!   `{"fps":30, "seconds":8, "warmup_s":2, "out":"debug/clip.mp4",
//!     "hide_hud":true, "anim_speed":1, "pan_yaw_deg":0, "pan_pitch_deg":0,
//!     "path":[[0, x,y,z,yaw,pitch], ...], "descend_half_life_s":0,
//!     "descend_floor_m":50, "ffmpeg":""}`
//! Warmup frames run the clock and the camera but are not written, so streamed
//! terrain and the clouds' temporal history settle first. When the last frame
//! is in, `debug/record_done.json` says `{"ok":true,"path",...}` or
//! `{"ok":false,"error",...}`. The file is the full-size master (the window's
//! size, cropped to even sides, H.264 at CRF 14); the clip maker cuts the
//! postable versions from it.

use std::io::Write;
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::engine::state::EngineState;

pub(crate) const REQUEST_PATH: &str = "debug/record_request.json";
pub(crate) const DONE_PATH: &str = "debug/record_done.json";

/// One camera keyframe of a `path`: seconds into the clip, and the home-frame
/// pose `[x, y, z, yaw, pitch]` (y at eye height, yaw 0 north, radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Key {
    pub t: f32,
    pub pose: [f32; 5],
}

/// One recording, as requested. Every field has a default, so `{}` records
/// eight seconds of whatever the camera is looking at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecordSpec {
    pub fps: u32,
    pub seconds: f32,
    pub warmup_s: f32,
    pub out: String,
    pub hide_hud: bool,
    pub anim_speed: f32,
    pub pan_yaw_deg: f32,
    pub pan_pitch_deg: f32,
    pub path: Vec<Key>,
    pub descend_half_life_s: f32,
    pub descend_floor_m: f32,
    pub ffmpeg: String,
}

impl RecordSpec {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("malformed JSON: {e}"))?;
        let num = |k: &str, d: f64| v.get(k).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
        let mut path = match v.get("path") {
            None => Vec::new(),
            Some(p) => p
                .as_array()
                .ok_or("path must be a list of [t, x, y, z, yaw, pitch]")?
                .iter()
                .map(|k| {
                    let n: Vec<f32> =
                        k.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
                    if n.len() == 6 {
                        Ok(Key { t: n[0], pose: [n[1], n[2], n[3], n[4], n[5]] })
                    } else {
                        Err(format!("a path key needs six numbers, [t, x, y, z, yaw, pitch]: {k}"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        path.sort_by(|a, b| a.t.total_cmp(&b.t));
        let text_of = |k: &str, d: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or(d).to_string();
        Ok(Self {
            fps: (num("fps", 30.0).round() as u32).clamp(1, 120),
            seconds: num("seconds", 8.0).clamp(0.1, 600.0),
            warmup_s: num("warmup_s", 2.0).clamp(0.0, 60.0),
            out: text_of("out", "debug/clip.mp4"),
            hide_hud: v.get("hide_hud").and_then(|x| x.as_bool()).unwrap_or(true),
            anim_speed: num("anim_speed", 1.0).clamp(0.0, 1000.0),
            pan_yaw_deg: num("pan_yaw_deg", 0.0),
            pan_pitch_deg: num("pan_pitch_deg", 0.0),
            path,
            descend_half_life_s: num("descend_half_life_s", 0.0).max(0.0),
            descend_floor_m: num("descend_floor_m", 50.0).max(0.0),
            ffmpeg: text_of("ffmpeg", ""),
        })
    }

    pub(crate) fn warmup_frames(&self) -> u32 {
        (self.warmup_s * self.fps as f32).round() as u32
    }

    pub(crate) fn clip_frames(&self) -> u32 {
        ((self.seconds * self.fps as f32).round() as u32).max(1)
    }
}

/// An angle brought into (-PI, PI].
fn wrap_pi(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

/// Smoothstep: a move that starts and stops gently instead of jerking.
pub(crate) fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The pose at `t` seconds along `keys` (sorted by time): a Catmull-Rom curve
/// through every key, so a dolly passes each key exactly and bends smoothly
/// between them; yaw takes the short way round. Before the first key and
/// after the last, the end pose holds. None for an empty path.
pub(crate) fn pose_at(keys: &[Key], t: f32) -> Option<[f32; 5]> {
    let n = keys.len();
    if n == 0 {
        return None;
    }
    if n == 1 || t <= keys[0].t {
        return Some(keys[0].pose);
    }
    if t >= keys[n - 1].t {
        return Some(keys[n - 1].pose);
    }
    let i = keys.windows(2).position(|w| t >= w[0].t && t < w[1].t).unwrap_or(n - 2);
    let (k1, k2) = (keys[i], keys[i + 1]);
    let k0 = if i > 0 { keys[i - 1] } else { k1 };
    let k3 = if i + 2 < n { keys[i + 2] } else { k2 };
    let u = (t - k1.t) / (k2.t - k1.t).max(1e-6);
    let cr = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b + (c - a) * u + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u + (3.0 * b - a - 3.0 * c + d) * u * u * u)
    };
    let mut out = [0.0; 5];
    for j in [0usize, 1, 2, 4] {
        out[j] = cr(k0.pose[j], k1.pose[j], k2.pose[j], k3.pose[j]);
    }
    // Yaw: each neighbour unwrapped onto k1's branch first, so a turn through
    // north does not spin the long way round.
    let near = |x: f32| k1.pose[3] + wrap_pi(x - k1.pose[3]);
    out[3] = cr(near(k0.pose[3]), k1.pose[3], near(k2.pose[3]), near(k3.pose[3]));
    Some(out)
}

/// The descent speed, m/s, that halves the height above `floor_m` every
/// `half_life_s`: fast from orbit, slowing as the ground comes up.
pub(crate) fn descend_mps(alt_m: f64, floor_m: f32, half_life_s: f32) -> f32 {
    if half_life_s <= 0.0 || alt_m <= floor_m as f64 {
        return 0.0;
    }
    ((alt_m - floor_m as f64) * std::f64::consts::LN_2 / half_life_s as f64) as f32
}

/// ffmpeg's arguments for the master: raw RGBA frames on stdin, H.264 out.
pub(crate) fn ffmpeg_args(w: u32, h: u32, fps: u32, out: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.push(format!("{w}x{h}"));
    a.extend(["-r".to_string(), fps.to_string(), "-i".to_string(), "-".to_string()]);
    // yuv420p needs even sides, and a window filling the work area is often
    // odd (2560x1387 on the operator's screen): crop the stray row or column.
    for s in ["-vf", "crop=trunc(iw/2)*2:trunc(ih/2)*2", "-c:v", "libx264", "-preset", "veryfast", "-crf", "14"] {
        a.push(s.to_string());
    }
    for s in ["-pix_fmt", "yuv420p", "-movflags", "+faststart"] {
        a.push(s.to_string());
    }
    a.push(out.to_string());
    a
}

struct Encoder {
    child: std::process::Child,
    size: (u32, u32),
}

/// A recording in progress (`EngineState::movie`).
pub(crate) struct Recorder {
    spec: RecordSpec,
    /// Frames drawn since the recording began, warmup included.
    frame: u32,
    /// The virtual seconds `start_time.elapsed()` reads this frame.
    virtual_s: f64,
    /// The view the pan turns from, taken on the first frame.
    pan_base: Option<(f32, f32)>,
    encoder: Option<Encoder>,
    started: Instant,
    hud_was: bool,
}

impl Recorder {
    /// Seconds into the clip for the frame being drawn (0 through warmup).
    fn clip_time(&self) -> f32 {
        self.frame.saturating_sub(self.spec.warmup_frames()) as f32 / self.spec.fps as f32
    }
}

fn write_done(v: serde_json::Value) {
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE_PATH, v.to_string());
}

/// Start a recording when `debug/record_request.json` appears (consumed either
/// way). Called once a frame beside the camera request.
pub(crate) fn poll_request(state: &mut EngineState) {
    if !std::path::Path::new(REQUEST_PATH).exists() {
        return;
    }
    let text = std::fs::read_to_string(REQUEST_PATH).unwrap_or_default();
    let _ = std::fs::remove_file(REQUEST_PATH);
    let _ = std::fs::remove_file(DONE_PATH);
    if state.movie.is_some() {
        write_done(serde_json::json!({"ok": false, "error": "a recording is already running"}));
        return;
    }
    if !state.world_loaded {
        write_done(serde_json::json!({"ok": false, "error": "3D world not loaded -- enter the world first"}));
        return;
    }
    let spec = match RecordSpec::parse(&text) {
        Ok(s) => s,
        Err(e) => {
            write_done(serde_json::json!({"ok": false, "error": e}));
            return;
        }
    };
    log::info!(
        "[Movie] recording {} s at {} fps (+{} s warmup) to {}",
        spec.seconds, spec.fps, spec.warmup_s, spec.out
    );
    let hud_was = state.gui_state.show_hud;
    if spec.hide_hud {
        state.gui_state.show_hud = false;
    }
    state.movie = Some(Recorder {
        virtual_s: state.start_time.elapsed().as_secs_f64(),
        spec,
        frame: 0,
        pan_base: None,
        encoder: None,
        started: Instant::now(),
        hud_was,
    });
}

/// The frame's time step while recording: exactly 1/fps, and the animation
/// clock slid to match (see THE CLOCK above). None when not recording.
pub(crate) fn frame_step(state: &mut EngineState, now: Instant) -> Option<f32> {
    let rec = state.movie.as_mut()?;
    let step = 1.0 / rec.spec.fps as f64;
    rec.virtual_s += step * rec.spec.anim_speed as f64;
    if let Some(origin) = now.checked_sub(Duration::from_secs_f64(rec.virtual_s)) {
        state.start_time = origin;
    }
    Some(step as f32)
}

/// Move the camera for the frame being drawn (see THE CAMERA above).
pub(crate) fn steer(state: &mut EngineState) {
    let Some(rec) = state.movie.as_ref() else { return };
    let t = rec.clip_time();
    let u = t / rec.spec.seconds;
    let pose = pose_at(&rec.spec.path, t);
    let pan = (rec.spec.pan_yaw_deg.to_radians(), rec.spec.pan_pitch_deg.to_radians());
    let descend = (rec.spec.descend_half_life_s, rec.spec.descend_floor_m);
    if let Some(p) = pose {
        let at = Vec3::new(p[0], p[1], p[2]) + state.station_off;
        for (_e, (tr, _c)) in state
            .game_world
            .world
            .query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
        {
            tr.position = at;
        }
        state.camera.position = at;
        state.camera.yaw = p[3];
        state.camera.pitch = p[4];
        // A rig park's hold would pull the camera back to where it parked.
        if let Some((held, _)) = state.probe_hold.as_mut() {
            *held = at;
        }
    } else if pan != (0.0, 0.0) {
        let (yaw, pitch) = (state.camera.yaw, state.camera.pitch);
        let Some(rec) = state.movie.as_mut() else { return };
        let (y0, p0) = *rec.pan_base.get_or_insert((yaw, pitch));
        let e = ease(u);
        state.camera.yaw = y0 + pan.0 * e;
        state.camera.pitch = (p0 + pan.1 * e).clamp(-1.5, 1.5);
    }
    if descend.0 > 0.0 {
        state.probe_descend_mps = height_above_ground(state).map_or(0.0, |a| descend_mps(a, descend.1, descend.0));
    }
}

/// The camera's height over the DRAWN ground below it, metres: the frame-lock
/// anchor (body-fixed, so the same direction the heightmap is indexed by)
/// against the ground radius the camera park samples. Not over the bare
/// sphere: the first drop over Fuji (2026-09-30) stopped 900 m above sea
/// level, which is underground there, and its last second was the inside of
/// the terrain. None without a planet frame lock.
fn height_above_ground(state: &EngineState) -> Option<f64> {
    let body = state.frame_lock_body.as_deref()?;
    let def = state.planet_defs.get(body)?;
    let a = state.frame_lock_anchor;
    if a.length_squared() < 0.5 {
        return None;
    }
    let tiles = (body == "earth" && state.terrain_tiles.tier_installed()).then_some(&state.terrain_tiles);
    let hm = state.planet_heightmaps.get(body).map(|h| h.as_ref());
    let ground = crate::engine::frame_lock::ground_radius_m(Some(def), hm, None, tiles, a.normalize());
    Some(a.length() - ground)
}

/// Write the frame just drawn (called before present, beside the screenshot
/// command), and finish once the last one is in.
pub(crate) fn capture(state: &mut EngineState, texture: &wgpu::Texture) {
    let Some(mut rec) = state.movie.take() else { return };
    if rec.frame >= rec.spec.warmup_frames() {
        let (px, w, h) = match state.renderer.capture_current_frame_rgba(texture) {
            Ok(f) => f,
            Err(e) => return finish(state, rec, Err(e)),
        };
        if rec.encoder.is_none() {
            match spawn_encoder(&rec.spec, w, h, &state.gui_state.settings.ffmpeg_path) {
                Ok(enc) => rec.encoder = Some(enc),
                Err(e) => return finish(state, rec, Err(e)),
            }
        }
        let Some(enc) = rec.encoder.as_mut() else { return };
        if enc.size != (w, h) {
            let e = format!("the window changed size mid-recording ({}x{} to {w}x{h})", enc.size.0, enc.size.1);
            return finish(state, rec, Err(e));
        }
        let wrote = match enc.child.stdin.as_mut() {
            Some(stdin) => stdin.write_all(&px).map_err(|e| e.to_string()),
            None => Err("ffmpeg has no input".to_string()),
        };
        if let Err(e) = wrote {
            return finish(state, rec, Err(format!("ffmpeg stopped taking frames: {e}")));
        }
    }
    rec.frame += 1;
    if rec.frame >= rec.spec.warmup_frames() + rec.spec.clip_frames() {
        return finish(state, rec, Ok(()));
    }
    state.movie = Some(rec);
}

fn spawn_encoder(spec: &RecordSpec, w: u32, h: u32, configured: &str) -> Result<Encoder, String> {
    let wanted = if spec.ffmpeg.is_empty() { configured } else { spec.ffmpeg.as_str() };
    let ffmpeg = crate::media::transcode::find_ffmpeg(wanted)
        .ok_or("ffmpeg not found: set it in Settings > Media, or put it on PATH")?;
    if let Some(dir) = std::path::Path::new(&spec.out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args(ffmpeg_args(w, h, spec.fps, &spec.out))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashes up
    }
    let child = cmd.spawn().map_err(|e| format!("could not start ffmpeg: {e}"))?;
    Ok(Encoder { child, size: (w, h) })
}

/// Close the encoder (ffmpeg writes the file's index on end of input), put
/// the HUD and the descent back, and report.
fn finish(state: &mut EngineState, mut rec: Recorder, result: Result<(), String>) {
    state.gui_state.show_hud = rec.hud_was;
    if rec.spec.descend_half_life_s > 0.0 {
        state.probe_descend_mps = 0.0;
    }
    let frames = rec.frame.saturating_sub(rec.spec.warmup_frames());
    let mut size = [0u32, 0];
    let encoded = match rec.encoder.take() {
        None => Ok(()),
        Some(mut enc) => {
            size = [enc.size.0, enc.size.1];
            drop(enc.child.stdin.take());
            match enc.child.wait_with_output() {
                Ok(o) if o.status.success() => Ok(()),
                Ok(o) => Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&o.stderr).trim())),
                Err(e) => Err(format!("ffmpeg did not finish: {e}")),
            }
        }
    };
    let wall_s = rec.started.elapsed().as_secs_f64();
    let done = match result.and(encoded) {
        Ok(()) => {
            log::info!("[Movie] wrote {} ({frames} frames, {:.0} s to record)", rec.spec.out, wall_s);
            serde_json::json!({"ok": true, "path": rec.spec.out, "frames": frames, "fps": rec.spec.fps,
                "size": size, "wall_s": wall_s})
        }
        Err(e) => {
            log::warn!("[Movie] recording failed: {e}");
            serde_json::json!({"ok": false, "error": e, "frames": frames})
        }
    };
    write_done(done);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request fills its gaps with defaults, clamps what it cannot honour,
    /// sorts the path by time and refuses a malformed key.
    #[test]
    fn a_record_request_parses_with_defaults_and_clamps() {
        let s = RecordSpec::parse("{}").unwrap();
        assert_eq!((s.fps, s.seconds, s.warmup_s, s.hide_hud), (30, 8.0, 2.0, true));
        assert_eq!((s.warmup_frames(), s.clip_frames()), (60, 240));
        let s = RecordSpec::parse(r#"{"fps":500,"seconds":0,"path":[[4,1,2,3,0,0],[0,0,0,0,0,0]]}"#).unwrap();
        assert_eq!((s.fps, s.seconds), (120, 0.1));
        assert_eq!(s.path[0].t, 0.0, "keys sorted by time");
        assert!(RecordSpec::parse(r#"{"path":[[1,2,3]]}"#).is_err());
        assert!(RecordSpec::parse("not json").is_err());
    }

    /// THE DOLLY PASSES EVERY KEY AND DOES NOT JUMP. At each key time the pose
    /// is the key's; between keys it moves in small steps; yaw crosses north
    /// the short way. Red check, run: without the yaw unwrap the turn from
    /// 3.0 to -3.0 spins the long way round and fails.
    #[test]
    fn a_path_passes_its_keys_smoothly() {
        let k = |t: f32, x: f32, yaw: f32| Key { t, pose: [x, 1.7, 0.0, yaw, 0.0] };
        let keys = [k(0.0, 0.0, 3.0), k(2.0, 4.0, -3.0), k(5.0, 4.0, -2.5)];
        for key in &keys {
            let p = pose_at(&keys, key.t).unwrap();
            assert!((p[0] - key.pose[0]).abs() < 1e-4 && (wrap_pi(p[3] - key.pose[3])).abs() < 1e-4, "misses key at {}: {p:?}", key.t);
        }
        let mut last = pose_at(&keys, 0.0).unwrap();
        for i in 1..=500 {
            let p = pose_at(&keys, i as f32 * 0.01).unwrap();
            assert!((p[0] - last[0]).abs() < 0.05, "a jump at {}", i as f32 * 0.01);
            // 3.0 to -3.0 is 0.28 rad the short way; the long way would be 6 rad.
            assert!(wrap_pi(p[3] - last[3]).abs() < 0.02, "yaw spun the long way at {}", i as f32 * 0.01);
            last = p;
        }
        assert_eq!(pose_at(&keys, 9.0).unwrap(), keys[2].pose, "the end pose holds");
        assert!(pose_at(&[], 1.0).is_none());
    }

    /// The descent halves the height left every half-life and stops at the floor.
    #[test]
    fn the_descent_halves_the_height_every_half_life() {
        let (mut alt, dt) = (12_000_000.0_f64, 1.0 / 30.0);
        for _ in 0..(2.0 / dt) as u32 {
            alt -= descend_mps(alt, 0.0, 2.0) as f64 * dt;
        }
        assert!((alt / 6_000_000.0 - 1.0).abs() < 0.02, "one half-life should halve it: {alt}");
        assert_eq!(descend_mps(40.0, 50.0, 2.0), 0.0);
        assert_eq!(descend_mps(1000.0, 50.0, 0.0), 0.0);
        assert_eq!((ease(0.0), ease(1.0), ease(0.5)), (0.0, 1.0, 0.5));
    }

    /// The master is raw RGBA in, H.264 out, and never chokes on an odd side.
    #[test]
    fn the_encoder_takes_raw_rgba_and_crops_to_even_sides() {
        let a = ffmpeg_args(2560, 1387, 30, "debug/x.mp4").join(" ");
        assert!(a.contains("-f rawvideo -pix_fmt rgba -s 2560x1387 -r 30 -i -"), "{a}");
        assert!(a.contains("crop=trunc(iw/2)*2:trunc(ih/2)*2") && a.contains("yuv420p") && a.ends_with("debug/x.mp4"), "{a}");
    }
}
