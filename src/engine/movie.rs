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
//! THE CAMERA. Three motions. Pan and descend combine with each other; a path
//! owns the whole camera, so it replaces both:
//!   - `path`: keyframes `[t, x, y, z, yaw, pitch]` in the home frame (the
//!     numbers scripts/home-vantages.json uses), joined by a Catmull-Rom curve:
//!     a dolly through the home. Re-applied every frame against the station's
//!     current offset, so the station moving in orbit cannot carry the camera
//!     off (the drift photograph-home.js documents). It sets the position AND
//!     the view every frame, so with a path, pan and descend are ignored, and
//!     the done file lists them under `"ignored"` (2026-10-02 review: these
//!     docs used to say all three combine, and a path silently dropped the
//!     other two). Descend cannot honestly ride along: a path is in the home
//!     frame and a descent is toward a planet's ground, and left running it
//!     sank the frame-lock anchor every frame under a camera the path kept
//!     putting back.
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
//! size, cropped to even sides, H.264 at CRF 14, BT.709); the clip maker cuts
//! the postable versions from it.
//!   - One recording at a time. A request that arrives while one runs is
//!     refused in `debug/record_rejected.json` and NEVER in the done file: that
//!     file belongs to the running recording, whose requester is still waiting
//!     on it (2026-10-02 review: the refusal used to overwrite it, so the first
//!     requester read the second one's answer as its own).
//!   - `debug/record_cancel.json` (any content) stops the running recording
//!     cleanly: ffmpeg is closed so the part already written is a valid file,
//!     the HUD and the descent are put back, and the done file says
//!     `{"ok":false,"error":"cancelled","path",...}`. It carries `"path"` only
//!     when at least one frame reached ffmpeg and ffmpeg closed cleanly: a
//!     cancel during the warmup has no file to name, so its done file is
//!     `{"ok":false,"error":"cancelled","frames":0}` (`finish`, `partial`).
//!     Closing ffmpeg blocks the frame until the encoder has flushed and the
//!     `+faststart` rewrite has moved the index to the front, seconds on a
//!     long master, so the done file can come well after the cancel is gone.
//!     The log says which happened: `[Movie] cancelled at frame N` when a
//!     recording was stopped, `nothing to stop` when none was running (the
//!     cancel is consumed either way). The clip maker sends it when it gives
//!     up on a shot, so the next shot does not start beside a recording still
//!     running, and reads those two lines to know how long to wait.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::engine::state::EngineState;

pub(crate) const REQUEST_PATH: &str = "debug/record_request.json";
pub(crate) const DONE_PATH: &str = "debug/record_done.json";
pub(crate) const CANCEL_PATH: &str = "debug/record_cancel.json";
pub(crate) const REJECTED_PATH: &str = "debug/record_rejected.json";

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

    /// Whether this recording drives the descent. Never with a path: the path
    /// owns the camera (see THE CAMERA above).
    pub(crate) fn descends(&self) -> bool {
        self.descend_half_life_s > 0.0 && self.path.is_empty()
    }

    /// The motions asked for that a path overrides, so the requester is told
    /// instead of getting a clip without them and no word why.
    pub(crate) fn ignored(&self) -> Vec<&'static str> {
        if self.path.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.pan_yaw_deg != 0.0 {
            out.push("pan_yaw_deg");
        }
        if self.pan_pitch_deg != 0.0 {
            out.push("pan_pitch_deg");
        }
        if self.descend_half_life_s > 0.0 {
            out.push("descend_half_life_s");
        }
        out
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

/// The BT.709 colour tags every clip carries (2026-10-02 review). Untagged
/// H.264 leaves each player to guess the colour space, and they guess
/// differently (BT.601 for small frames in some, BT.709 in others), so the same
/// file showed different greens and skin tones from one site to the next.
/// BT.709's primaries are sRGB's, which is what the swapchain holds. The same
/// list is in scripts/make-clips.js for the cuts.
pub(crate) const BT709_TAGS: [&str; 8] =
    ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"];

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
    // Then the RGB to YUV conversion, done HERE with the BT.709 matrix rather
    // than left to the converter ffmpeg inserts for `-pix_fmt`, which uses
    // BT.601: tags saying 709 over numbers made with 601 would shift every
    // colour (2026-10-02, checked by decoding a flat red frame). `setparams`
    // stamps the tags on the frames themselves, because ffmpeg 7 takes the
    // encoder's primaries and transfer from the frames and ignores the output
    // flags below (measured: with the flags alone, ffprobe read the primaries
    // and transfer as "unknown"). The flags stay for older ffmpeg.
    let vf = concat!(
        "crop=trunc(iw/2)*2:trunc(ih/2)*2,scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,",
        "setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv"
    );
    for s in ["-vf", vf, "-c:v", "libx264", "-preset", "veryfast", "-crf", "14"] {
        a.push(s.to_string());
    }
    for s in ["-pix_fmt", "yuv420p"].into_iter().chain(BT709_TAGS).chain(["-movflags", "+faststart"]) {
        a.push(s.to_string());
    }
    a.push(out.to_string());
    a
}

/// What ffmpeg said when it failed, so the report gives the cause ("Unknown
/// encoder 'libx264'", "No space left on device") and not only that a pipe
/// closed. The tail is kept when it is long: the cause comes last.
pub(crate) fn encoder_failure(code: Option<i32>, stderr: &[u8]) -> String {
    let said = String::from_utf8_lossy(stderr);
    let said = said.trim();
    const KEEP: usize = 1500;
    let tail = if said.len() > KEEP {
        let mut cut = said.len() - KEEP;
        while !said.is_char_boundary(cut) {
            cut += 1;
        }
        &said[cut..]
    } else {
        said
    };
    let how = code.map_or("ended by a signal".to_string(), |c| format!("exit code {c}"));
    if tail.is_empty() {
        format!("ffmpeg failed ({how}) and said nothing")
    } else {
        format!("ffmpeg failed ({how}): {tail}")
    }
}

/// The recording's result and the encoder's, as one report. When ffmpeg dies
/// the next frame cannot be written, so BOTH fail, and the encoder's words are
/// the cause. `result.and(encoded)` kept only the first and threw ffmpeg's
/// message away, so a dead encoder only ever reported "ffmpeg stopped taking
/// frames: the pipe is being closed" (2026-10-02 review).
pub(crate) fn outcome(recording: Result<(), String>, encoder: Result<(), String>) -> Result<(), String> {
    match (recording, encoder) {
        (Ok(()), enc) => enc,
        (Err(r), Ok(())) => Err(r),
        (Err(r), Err(e)) => Err(format!("{r}; {e}")),
    }
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

fn write_json(path: &Path, v: serde_json::Value) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, v.to_string());
}

fn write_done(v: serde_json::Value) {
    write_json(Path::new(DONE_PATH), v);
}

/// What this frame's request files ask the recorder to do.
#[derive(Debug, PartialEq)]
pub(crate) enum Incoming {
    Nothing,
    /// Stop the running recording (`record_cancel.json`).
    Cancel,
    Start(RecordSpec),
}

/// Read and consume this frame's request files under `root` (the working
/// directory in the game, a scratch folder in the tests), answering in the
/// files whatever can be answered without the engine. A cancel is read first,
/// so a cancel and a fresh request landing in the same frame stop the old
/// recording now and start the new one on the next frame.
pub(crate) fn take_requests(root: &Path, running: bool, world_loaded: bool) -> Incoming {
    let cancel = root.join(CANCEL_PATH);
    if cancel.exists() {
        let _ = std::fs::remove_file(&cancel);
        if running {
            return Incoming::Cancel;
        }
        log::info!("[Movie] cancel requested with no recording running: nothing to stop");
    }
    let request = root.join(REQUEST_PATH);
    if !request.exists() {
        return Incoming::Nothing;
    }
    let text = std::fs::read_to_string(&request).unwrap_or_default();
    let _ = std::fs::remove_file(&request);
    if running {
        // NOT the done file: the running recording writes that when it ends,
        // and its requester is waiting on it (see THE PROTOCOL above).
        let e = "a recording is already running";
        log::warn!("[Movie] record request refused: {e}");
        write_json(&root.join(REJECTED_PATH), serde_json::json!({"ok": false, "error": e}));
        return Incoming::Nothing;
    }
    let _ = std::fs::remove_file(root.join(DONE_PATH));
    let _ = std::fs::remove_file(root.join(REJECTED_PATH));
    let refuse = |e: String| {
        write_json(&root.join(DONE_PATH), serde_json::json!({"ok": false, "error": e}));
        Incoming::Nothing
    };
    if !world_loaded {
        return refuse("3D world not loaded -- enter the world first".to_string());
    }
    match RecordSpec::parse(&text) {
        Ok(s) => Incoming::Start(s),
        Err(e) => refuse(e),
    }
}

/// Start a recording when `debug/record_request.json` appears, or stop one when
/// `debug/record_cancel.json` does (both consumed either way). Called once a
/// frame beside the camera request.
pub(crate) fn poll_request(state: &mut EngineState) {
    let spec = match take_requests(Path::new("."), state.movie.is_some(), state.world_loaded) {
        Incoming::Nothing => return,
        Incoming::Cancel => {
            if let Some(rec) = state.movie.take() {
                log::info!("[Movie] cancelled at frame {}", rec.frame);
                finish(state, rec, Err("cancelled".to_string()));
            }
            return;
        }
        Incoming::Start(spec) => spec,
    };
    log::info!(
        "[Movie] recording {} s at {} fps (+{} s warmup) to {}",
        spec.seconds, spec.fps, spec.warmup_s, spec.out
    );
    let ignored = spec.ignored();
    if !ignored.is_empty() {
        log::warn!("[Movie] a path owns the camera, so these are ignored: {}", ignored.join(", "));
    }
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
    // Never with a path (`descends`): the path puts the camera back every
    // frame, and the hold's descent would sink the frame-lock anchor under it.
    let descend = rec.spec.descends().then_some((rec.spec.descend_half_life_s, rec.spec.descend_floor_m));
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
    if let Some((half_life_s, floor_m)) = descend {
        state.probe_descend_mps = height_above_ground(state).map_or(0.0, |a| descend_mps(a, floor_m, half_life_s));
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
    if rec.spec.descends() {
        state.probe_descend_mps = 0.0;
    }
    let frames = rec.frame.saturating_sub(rec.spec.warmup_frames());
    let mut size = [0u32, 0];
    let started_encoder = rec.encoder.is_some();
    let encoded = match rec.encoder.take() {
        None => Ok(()),
        Some(mut enc) => {
            size = [enc.size.0, enc.size.1];
            drop(enc.child.stdin.take());
            match enc.child.wait_with_output() {
                Ok(o) if o.status.success() => Ok(()),
                Ok(o) => Err(encoder_failure(o.status.code(), &o.stderr)),
                Err(e) => Err(format!("ffmpeg did not finish: {e}")),
            }
        }
    };
    // A recording stopped early (a cancel, a resize) still closed ffmpeg
    // cleanly, so the part written is a playable file: name it.
    let partial = started_encoder && encoded.is_ok();
    let wall_s = rec.started.elapsed().as_secs_f64();
    let ignored = rec.spec.ignored();
    let done = match outcome(result, encoded) {
        Ok(()) => {
            log::info!("[Movie] wrote {} ({frames} frames, {:.0} s to record)", rec.spec.out, wall_s);
            let mut v = serde_json::json!({"ok": true, "path": rec.spec.out, "frames": frames, "fps": rec.spec.fps,
                "size": size, "wall_s": wall_s});
            if !ignored.is_empty() {
                v["ignored"] = serde_json::json!(ignored);
            }
            v
        }
        Err(e) => {
            log::warn!("[Movie] recording failed: {e}");
            let mut v = serde_json::json!({"ok": false, "error": e, "frames": frames});
            if partial {
                v["path"] = serde_json::json!(rec.spec.out);
            }
            v
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

    /// THE MASTER IS BT.709 IN NUMBERS AND IN TAGS (2026-10-02 review). The
    /// tags alone are not enough: the RGB to YUV conversion must use the same
    /// matrix, inside the filter chain, before the format is fixed, or the
    /// converter ffmpeg inserts on its own does it with BT.601; and the frames
    /// must carry the tags, or ffmpeg 7 drops the primaries and transfer.
    /// Checked against ffmpeg 2025-01-22 by encoding a flat red frame with
    /// exactly these arguments: Y 63, Cb 102, Cr 240 (BT.709) where the old
    /// arguments gave Y 81, Cb 90 (BT.601), and ffprobe reading bt709 for the
    /// matrix, primaries and transfer and tv for the range. Seen red: with the
    /// old arguments restored this test fails on the missing tags.
    #[test]
    fn the_master_is_converted_and_tagged_bt709() {
        let a = ffmpeg_args(1920, 1080, 30, "debug/x.mp4");
        let joined = a.join(" ");
        assert!(
            joined.contains("-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv"),
            "untagged: {joined}"
        );
        let vf = &a[a.iter().position(|s| s == "-vf").expect("a filter chain") + 1];
        let matrix = vf.find("scale=out_color_matrix=bt709").expect("the conversion uses BT.709");
        let format = vf.find("format=yuv420p").expect("the format is fixed in the chain");
        let stamp = vf
            .find("setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv")
            .expect("the frames carry the tags");
        assert!(matrix < format && format < stamp, "convert, then fix the format, then stamp: {vf}");
        assert!(a.iter().position(|s| s == "-colorspace") < a.iter().position(|s| s == "debug/x.mp4"));
    }

    /// A scratch folder standing in for the game's working directory.
    fn scratch(name: &str) -> crate::test_temp::TempPath {
        let d = crate::test_temp::dir(&format!("movie_{name}"));
        std::fs::create_dir_all(d.join("debug")).unwrap();
        d
    }

    /// A SECOND REQUEST NEVER ANSWERS IN THE RUNNING RECORDING'S DONE FILE
    /// (2026-10-02 review). The first requester is still waiting on
    /// record_done.json; a refusal written there was read as ITS answer, and
    /// the clip maker moved on from a recording that was still running. Seen
    /// red: with the old refusal (write_done in the `running` branch) restored,
    /// the done file exists and this fails.
    #[test]
    fn a_second_request_is_refused_without_touching_the_done_file() {
        let root = scratch("second");
        std::fs::write(root.join(REQUEST_PATH), "{}").unwrap();
        assert_eq!(take_requests(&root, true, true), Incoming::Nothing);
        assert!(!root.join(REQUEST_PATH).exists(), "the request is consumed");
        assert!(!root.join(DONE_PATH).exists(), "the running recording owns the done file");
        let rejected = std::fs::read_to_string(root.join(REJECTED_PATH)).expect("the refusal is written somewhere");
        assert!(rejected.contains("already running"), "{rejected}");
        // With nothing running the same request starts, and clears a stale
        // refusal so a requester watching for one is not misled.
        std::fs::write(root.join(REQUEST_PATH), r#"{"seconds":3}"#).unwrap();
        match take_requests(&root, false, true) {
            Incoming::Start(s) => assert_eq!(s.seconds, 3.0),
            other => panic!("expected a start, got {other:?}"),
        }
        assert!(!root.join(REJECTED_PATH).exists());
        // Refusals that are the requester's own answer still go in done.
        std::fs::write(root.join(REQUEST_PATH), "{}").unwrap();
        assert_eq!(take_requests(&root, false, false), Incoming::Nothing);
        assert!(std::fs::read_to_string(root.join(DONE_PATH)).unwrap().contains("world not loaded"));
    }

    /// A CANCEL STOPS THE RUNNING RECORDING, AND ONLY THAT (2026-10-02 review:
    /// there was no way to stop one, so a clip maker that gave up left it
    /// running into the next shot). A cancel with nothing running is consumed
    /// without an answer, so it cannot linger and kill the NEXT recording; a
    /// request landing beside a cancel waits one frame and then starts. Seen
    /// red: with the cancel branch removed from take_requests, the first
    /// assertion gets Nothing instead of Cancel.
    #[test]
    fn a_cancel_stops_the_running_recording_only() {
        let root = scratch("cancel");
        std::fs::write(root.join(CANCEL_PATH), "{}").unwrap();
        assert_eq!(take_requests(&root, true, true), Incoming::Cancel);
        assert!(!root.join(CANCEL_PATH).exists(), "the cancel is consumed");

        std::fs::write(root.join(CANCEL_PATH), "{}").unwrap();
        assert_eq!(take_requests(&root, false, true), Incoming::Nothing);
        assert!(!root.join(CANCEL_PATH).exists(), "a cancel with nothing to stop is consumed too");
        assert!(!root.join(DONE_PATH).exists() && !root.join(REJECTED_PATH).exists());

        std::fs::write(root.join(CANCEL_PATH), "{}").unwrap();
        std::fs::write(root.join(REQUEST_PATH), "{}").unwrap();
        assert_eq!(take_requests(&root, true, true), Incoming::Cancel);
        assert!(root.join(REQUEST_PATH).exists(), "the new request waits for the next frame");
        assert!(matches!(take_requests(&root, false, true), Incoming::Start(_)));
    }

    /// FFMPEG'S OWN WORDS SURVIVE (2026-10-02 review). When ffmpeg dies, the
    /// frame write fails first and the encoder's exit second; the report kept
    /// only the first, "the pipe is being closed", and never said why. Seen
    /// red: with `outcome` replaced by the old `recording.and(encoder)`, the
    /// cause is missing and this fails.
    #[test]
    fn ffmpegs_error_is_in_the_report_when_it_dies() {
        let said = b"[libx264 @ 0000] frame size 0x0 invalid\nError while opening encoder\n";
        let e = outcome(
            Err("ffmpeg stopped taking frames: The pipe is being closed. (os error 232)".to_string()),
            Err(encoder_failure(Some(1), said)),
        )
        .unwrap_err();
        assert!(e.contains("pipe is being closed") && e.contains("Error while opening encoder"), "{e}");
        assert!(e.contains("exit code 1"), "{e}");
        assert_eq!(outcome(Ok(()), Ok(())), Ok(()));
        assert_eq!(outcome(Err("cancelled".into()), Ok(())), Err("cancelled".into()));
        assert!(encoder_failure(None, b"").contains("said nothing"));
        // A long log keeps its end, where the cause is, and never splits a
        // character.
        let long = format!("{}\u{e9}cause", "x".repeat(4000));
        let m = encoder_failure(Some(1), long.as_bytes());
        assert!(m.ends_with("\u{e9}cause") && m.len() < 1600, "{}", m.len());
    }

    /// A PATH OWNS THE CAMERA: pan and descend are not applied with one, and
    /// the requester is told which it asked for (2026-10-02 review: the docs
    /// said all three combine, and a path dropped the other two silently,
    /// while the descent kept sinking the frame-lock anchor underneath). Seen
    /// red: with `descends` back to `descend_half_life_s > 0` alone, the first
    /// assertion fails.
    #[test]
    fn a_path_overrides_pan_and_descend_and_says_so() {
        let with_path = RecordSpec::parse(
            r#"{"path":[[0,0,1.7,0,0,0],[2,1,1.7,0,0,0]],"pan_yaw_deg":20,"descend_half_life_s":1.3}"#,
        )
        .unwrap();
        assert!(!with_path.descends(), "a path and a descent are never driven together");
        assert_eq!(with_path.ignored(), vec!["pan_yaw_deg", "descend_half_life_s"]);
        let no_path = RecordSpec::parse(r#"{"pan_pitch_deg":40,"descend_half_life_s":1.3}"#).unwrap();
        assert!(no_path.descends(), "pan and descend combine");
        assert!(no_path.ignored().is_empty());
    }
}
