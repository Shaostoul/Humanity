//! The in-game CAMERA screen (in-world screens, rung 3): a placed camera
//! post looks at the world, and a wall screen whose source is
//! `camera:<that post's instance id>` shows what it sees. The operator's
//! words: "live camera feeds of in-game events".
//!
//! How it works, end to end:
//!
//! * A machine def with a `camera: Some((yaw_deg, pitch_deg, fov_deg))`
//!   field is a camera post (`camera_post` in both home catalogs). Its
//!   placed instance's `rotation` adds to the yaw, so turning the post in
//!   the editor turns the camera. `PlacedMachine::camera_pose` (machines.rs)
//!   resolves the lens position and look angles; `engine::screens::
//!   camera_posts_from` collects every post's pose on each placement rebuild
//!   and move, so the provider re-resolves the pose from the CURRENT
//!   placements each time it renders.
//! * [`CameraProvider`] is a WORLD SCREEN (`ScreenProvider::world_render`):
//!   it does not draw its own content; the engine renders the world for it.
//!   `engine::screens::frame_surfaces` picks at most ONE due world screen
//!   per frame ([`pick_due`]: never-rendered first, then the oldest, each
//!   no more often than [`CAMERA_INTERVAL`]) and calls the provider's
//!   `render_world` with an [`EngineWorld`] handle. The provider builds a
//!   renderer `Camera` at the post's pose with the surface's aspect
//!   ([`camera_from_pose`]) and has the world rendered STRAIGHT INTO its
//!   surface texture, which was created in the scene's swapchain format for
//!   exactly this (`surface_format`), through `engine::ipc::render_view_onto`,
//!   the same function the hi-res screenshot uses.
//! * Ordering: the render happens inside `frame_surfaces`, which runs
//!   before the live frame's own scene pass, so the scene pass samples THIS
//!   frame's view; a skipped tick keeps the last image (the texture
//!   persists). The provider's `frame` only ever draws a notice ("no camera
//!   named ...") and only when the notice text changes; a rendered view is
//!   never painted over.
//!
//! Budget: one render per frame across all camera screens, each at 10 Hz
//! at most, at the surface's own pixel size (the def's `px`: a smaller
//! screen is a cheaper camera), with the sky and scene passes only (no
//! planet/cloud pass; see `ViewPasses::SceneOnly` for why).

use crate::engine::ipc::{render_view_onto, ViewPasses};
use crate::engine::screens::ScreenQuad;
use crate::engine::state::{EngineState, SceneDrawLists};
use crate::gui::screen_surface::{notice, ScreenProvider, ScreenSurface, WorldRender, WorldRenderState};
use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::machines::CameraPose;
use crate::renderer::camera::Camera;
use crate::renderer::RenderObject;
use std::time::{Duration, Instant};

/// The least time between two renders of one camera screen: 10 Hz. A
/// security feed at 10 frames a second reads as live; the scene pass it
/// costs stays a tenth of what a 60 Hz camera would.
pub const CAMERA_INTERVAL: Duration = Duration::from_millis(100);

/// Which of the DUE world screens renders this frame: the one that has
/// never rendered, else the one whose last render is oldest; ties go to
/// the first candidate (`frame_surfaces` lists them nearest first, so the
/// nearest of two equally starved cameras wins). `candidates` are (surface
/// index, last render) pairs already filtered to the due ones. Pure, so
/// the round-robin is pinned by a test without a GPU: two cameras alternate
/// frame by frame once both are due, three cycle, and a camera that just
/// rendered waits for every other due one before it renders again.
pub fn pick_due(candidates: &[(usize, Option<Instant>)]) -> Option<usize> {
    // `None` (never rendered) sorts before every `Some`, which is the
    // starvation order wanted; `min_by_key` keeps the FIRST minimum, so a
    // tie goes to the first candidate.
    candidates.iter().min_by_key(|(_, last)| *last).map(|(si, _)| *si)
}

/// A renderer camera at `pose` with the projection aspect `aspect` (the
/// surface's width over height, so the picture is never stretched). The
/// yaw sign is the one subtlety: a machine's front at rotation `r` is
/// `Quat::from_rotation_y(r) * -Z`, while `Camera::forward()` for yaw `y` is
/// `(sin y, 0, -cos y)`; those agree when `y = -r`, which the test
/// `camera_forward_matches_the_machine_front` proves numerically rather
/// than by reading the formulas.
pub fn camera_from_pose(pose: &CameraPose, aspect: f32) -> Camera {
    let mut cam = Camera::new();
    cam.position = glam::Vec3::new(pose.position.0, pose.position.1, pose.position.2);
    cam.yaw = -pose.yaw_deg.to_radians();
    cam.pitch = pose.pitch_deg.to_radians();
    cam.fov_degrees = pose.fov_deg;
    cam.aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 16.0 / 9.0 };
    cam
}

/// The engine's side of `WorldRender`: the current camera posts and one
/// scene render from any pose, over the engine state and this frame's
/// draw lists. Built by `frame_surfaces` for the one world screen it serves;
/// `surface` is that screen's index into `Screens::surfaces`.
pub(crate) struct EngineWorld<'a, 'b> {
    pub state: &'a mut EngineState,
    pub lists: &'a SceneDrawLists<'b>,
    pub surface: usize,
}

/// The opaque draw list WITHOUT the display quads of screen `surface`.
///
/// A camera screen's surface texture is the render TARGET of its view, and
/// every screen quad's material samples its surface texture as the albedo.
/// wgpu refuses a texture that is both a render attachment and a sampled
/// texture within one pass (a usage conflict, a validation error that
/// would take the frame down), and the draw list is not culled per camera,
/// so the target's own quad must be left out of what the camera renders.
/// The consequence is the natural one: a camera never sees its own
/// screen; a camera pointed at its own monitor shows that monitor's body
/// with no picture on it. Pure, so a test can pin that exactly the target's
/// quads go and every other object, including OTHER screens' quads, stays.
pub fn without_own_quads(opaque: &[RenderObject], quads: &[ScreenQuad], surface: usize) -> Vec<RenderObject> {
    let own: Vec<usize> = quads.iter().filter(|q| q.surface == surface).map(|q| q.material).collect();
    opaque.iter().filter(|o| !own.contains(&o.material)).cloned().collect()
}

impl WorldRender for EngineWorld<'_, '_> {
    fn camera_pose(&self, instance_id: &str) -> Option<CameraPose> {
        self.state.screens.camera_posts.iter().find(|(id, _)| id == instance_id).map(|(_, pose)| *pose)
    }

    fn render_view(&mut self, camera: &Camera, target: &wgpu::TextureView, size: (u32, u32)) -> bool {
        // No world, no picture: the target is left untouched and the
        // caller is told so (a skipped render must not count as one, or
        // `renders > 0` in the done file would promise a picture that was
        // never drawn).
        if !self.state.world_loaded {
            return false;
        }
        let opaque = without_own_quads(self.lists.opaque, &self.state.screens.quads, self.surface);
        let lists = SceneDrawLists {
            celestial: self.lists.celestial,
            celestial_transparent: self.lists.celestial_transparent,
            orbit_lines: self.lists.orbit_lines,
            opaque: &opaque,
            transparent: self.lists.transparent,
            overlay: self.lists.overlay,
            ring_lines: self.lists.ring_lines,
        };
        render_view_onto(self.state, camera, target, size, &lists, ViewPasses::SceneOnly);
        true
    }
}

/// The provider for `camera:<instance id>` (see the module doc).
pub struct CameraProvider {
    /// The camera post's placed instance id.
    instance: String,
    /// When the world was last rendered for this screen.
    last_render: Option<Instant>,
    /// How many views have been rendered into the surface.
    renders: u64,
    /// The last attempt's outcome: `None` before the first, `Ok` when the
    /// surface holds a rendered view, `Err(text)` when the post could not be
    /// found (the text is the notice to show).
    outcome: Option<Result<(), String>>,
    /// The notice text currently drawn on the surface, so `frame` redraws
    /// only when it changes (and never over a rendered view).
    notice_shown: Option<String>,
}

impl CameraProvider {
    pub fn new(instance: &str) -> Self {
        Self {
            instance: instance.to_string(),
            last_render: None,
            renders: 0,
            outcome: None,
            notice_shown: None,
        }
    }

    /// The notice the screen should show right now, or `None` when it holds
    /// a rendered view. Pure, so the wording is pinned by a test.
    pub fn notice_text(&self) -> Option<String> {
        match &self.outcome {
            Some(Ok(())) => None,
            Some(Err(text)) => Some(text.clone()),
            None => Some(format!("Camera {}: waiting for its first picture", self.instance)),
        }
    }

    /// Book one render attempt at a found post: `rendered` is what
    /// `WorldRender::render_view` returned. A real render is counted and
    /// becomes the outcome (the view replaced whatever notice was on the
    /// texture); a skipped one (the world was not loaded) changes nothing,
    /// so the count and the notice keep telling the truth. Pure, so the
    /// rule is pinned by a test without a GPU.
    fn record_render(&mut self, rendered: bool) {
        if !rendered {
            return;
        }
        self.renders += 1;
        self.outcome = Some(Ok(()));
        self.notice_shown = None;
    }

    /// The last outcome as the rig sees it: (renders so far, live, error).
    fn report(&self) -> (u64, bool, Option<String>) {
        match &self.outcome {
            Some(Ok(())) => (self.renders, true, None),
            Some(Err(e)) => (self.renders, false, Some(e.clone())),
            None => (self.renders, false, None),
        }
    }
}

impl ScreenProvider for CameraProvider {
    fn frame(
        &mut self,
        surface: &mut ScreenSurface,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
    ) {
        // A rendered view IS the frame: never paint over it. A notice is
        // drawn once per distinct text.
        let Some(text) = self.notice_text() else { return };
        if self.notice_shown.as_deref() == Some(text.as_str()) {
            return;
        }
        surface.run_and_render(device, queue, theme, gui_state, |core, _theme, state| {
            core.run_with(state, |ctx, _| notice(ctx, &text))
        });
        self.notice_shown = Some(text);
    }

    fn kind(&self) -> &'static str {
        "camera"
    }

    fn status(&self) -> serde_json::Value {
        let (renders, live, error) = self.report();
        // The error field is named for THIS provider (`camera_error`), not
        // `error`: the dev IPC's own failure schema is
        // `{"ok": false, "error": "..."}`, and a successful request that
        // merged a provider's `"error": null` into the done file would
        // collide with it (the IPC merge also skips null fields, so the
        // key is absent, not null, when there is no error; both rules
        // guard the same collision).
        serde_json::json!({
            "camera": self.instance,
            "renders": renders,
            "live": live,
            "camera_error": error,
        })
    }

    /// The world is rendered straight into this surface, so the surface
    /// must be in the format the scene pipelines draw in.
    fn surface_format(&self, scene_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
        scene_format
    }

    fn world_render(&self) -> Option<WorldRenderState> {
        Some(WorldRenderState { last: self.last_render, interval: CAMERA_INTERVAL })
    }

    fn render_world(&mut self, surface: &mut ScreenSurface, world: &mut dyn WorldRender, now: Instant) {
        // Stamped on every attempt, found or not, so a missing post is
        // re-resolved at the camera cadence rather than every frame.
        self.last_render = Some(now);
        match world.camera_pose(&self.instance) {
            None => {
                self.outcome = Some(Err(format!("No camera post named {} is placed", self.instance)));
            }
            Some(pose) => {
                let (w, h) = surface.size();
                let camera = camera_from_pose(&pose, w as f32 / h.max(1) as f32);
                // Counted only if the engine really rendered (it skips
                // when the world is not loaded and leaves the texture as
                // it was).
                let rendered = world.render_view(&camera, surface.view(), (w, h));
                self.record_render(rendered);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::screens::{camera_posts_from, provider_for};
    use crate::gui::screen_surface::ScreenSource;
    use crate::machines::{MachineHome, PlacedMachine, ZoneRect, CAMERA_FOV_RANGE_DEG, CAMERA_PITCH_LIMIT_DEG};
    use glam::{Quat, Vec3};

    /// A placed camera post, the way `MachineHome::placements` builds one.
    fn post(pos: (f32, f32, f32), rotation: f32, camera: Option<(f32, f32, f32)>) -> PlacedMachine {
        let size = (0.14, 1.7, 0.14);
        PlacedMachine {
            id: "camera_post_1".into(),
            room: "garden".into(),
            pos,
            top_y: pos.1 + size.1,
            floor_y: pos.1,
            ceiling_y: pos.1 + 3.0,
            shape: "box".into(),
            size,
            color: (0.2, 0.2, 0.22),
            label: "Camera post".into(),
            stats: Vec::new(),
            rotation,
            model: None,
            screen: None,
            camera,
        }
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1.0e-4
    }

    /// The lens sits just below the top of the post, ON ITS FRONT FACE, and
    /// the yaw is the def's yaw plus the instance's rotation. A post at
    /// rotation 90 has its front facing -X (the machine convention), so the
    /// lens is half a depth toward -X of the body centre.
    #[test]
    fn pose_puts_the_lens_on_the_front_face_and_adds_the_rotation_to_the_yaw() {
        let p = post((10.0, 0.5, 5.0), 90.0, Some((0.0, -10.0, 70.0)));
        let pose = p.camera_pose().expect("a def with a camera resolves a pose");
        assert!((pose.position.0 - (10.0 - 0.07)).abs() < 1.0e-4, "x {}", pose.position.0);
        assert!((pose.position.2 - 5.0).abs() < 1.0e-4, "z {}", pose.position.2);
        assert!((pose.position.1 - (0.5 + 1.7 - 0.05)).abs() < 1.0e-4, "y {}", pose.position.1);
        assert_eq!(pose.yaw_deg, 90.0);
        assert_eq!(pose.pitch_deg, -10.0);
        assert_eq!(pose.fov_deg, 70.0);

        // The def's own yaw adds to the instance rotation.
        let p = post((0.0, 0.0, 0.0), 90.0, Some((30.0, 0.0, 70.0)));
        assert_eq!(p.camera_pose().unwrap().yaw_deg, 120.0);
        // No camera in the def: no pose, whatever the rotation.
        assert!(post((0.0, 0.0, 0.0), 45.0, None).camera_pose().is_none());
    }

    /// Pitch and field of view are clamped to values a camera can have: a
    /// data typo of pitch -120 becomes -89, fov 400 becomes 150, fov 1
    /// becomes 10. The lens never drops below the post's base either.
    #[test]
    fn pose_clamps_pitch_and_fov() {
        let p = post((0.0, 0.0, 0.0), 0.0, Some((0.0, -120.0, 400.0)));
        let pose = p.camera_pose().unwrap();
        assert_eq!(pose.pitch_deg, -CAMERA_PITCH_LIMIT_DEG);
        assert_eq!(pose.fov_deg, CAMERA_FOV_RANGE_DEG.1);
        let p = post((0.0, 0.0, 0.0), 0.0, Some((0.0, 95.0, 1.0)));
        let pose = p.camera_pose().unwrap();
        assert_eq!(pose.pitch_deg, CAMERA_PITCH_LIMIT_DEG);
        assert_eq!(pose.fov_deg, CAMERA_FOV_RANGE_DEG.0);
        // A post shorter than the lens margin keeps the lens at its base.
        let mut stub = post((0.0, 2.0, 0.0), 0.0, Some((0.0, 0.0, 70.0)));
        stub.top_y = 2.01;
        assert!((stub.camera_pose().unwrap().position.1 - 2.0).abs() < 1.0e-6);
    }

    /// THE SIGN PROOF. The camera built from a pose looks exactly where the
    /// machine's front face points: for rotations 0, 90, 180, 37 and a def
    /// yaw on top, `Camera::forward()` (pitch 0) equals
    /// `Quat::from_rotation_y(rotation) * -Z`, the renderer's own body
    /// rotation. With the yaw sign flipped in `camera_from_pose` the 90 and
    /// 37 cases fail (their forward mirrors across Z).
    #[test]
    fn camera_forward_matches_the_machine_front() {
        for (def_yaw, rotation) in [(0.0_f32, 0.0_f32), (0.0, 90.0), (0.0, 180.0), (0.0, 37.0), (30.0, 90.0)] {
            let p = post((3.0, 0.0, -2.0), rotation, Some((def_yaw, 0.0, 70.0)));
            let pose = p.camera_pose().unwrap();
            let cam = camera_from_pose(&pose, 16.0 / 9.0);
            let want = Quat::from_rotation_y((def_yaw + rotation).to_radians()) * -Vec3::Z;
            assert!(
                close(cam.forward(), want),
                "def yaw {def_yaw} + rotation {rotation}: forward {:?}, front {:?}",
                cam.forward(),
                want
            );
            // The lens is on that front face.
            let lens = Vec3::new(pose.position.0, pose.position.1, pose.position.2);
            let body = Vec3::new(3.0, pose.position.1, -2.0);
            assert!(close(lens - body, want * 0.07), "lens offset {:?}", lens - body);
        }
        // Pitch tilts the same forward: -10 looks down, +10 up, the xz
        // heading unchanged.
        let p = post((0.0, 0.0, 0.0), 90.0, Some((0.0, -10.0, 70.0)));
        let cam = camera_from_pose(&p.camera_pose().unwrap(), 1.0);
        let f = cam.forward();
        assert!(f.y < 0.0, "looks down: {f:?}");
        assert!(close(Vec3::new(f.x, 0.0, f.z).normalize(), -Vec3::X), "heading -X: {f:?}");
        // The aspect is the surface's; a degenerate one falls back to 16:9.
        assert_eq!(camera_from_pose(&p.camera_pose().unwrap(), 1280.0 / 720.0).aspect, 1280.0 / 720.0);
        assert_eq!(camera_from_pose(&p.camera_pose().unwrap(), 0.0).aspect, 16.0 / 9.0);
        assert_eq!(cam.fov_degrees, 70.0);
    }

    /// THE 10 HZ GATE: a screen that rendered 99 ms ago is not due, one
    /// that rendered 100 ms ago is, one that never rendered always is.
    #[test]
    fn a_camera_screen_is_due_at_most_ten_times_a_second() {
        let now = Instant::now();
        let never = WorldRenderState { last: None, interval: CAMERA_INTERVAL };
        assert!(never.due(now));
        let fresh = WorldRenderState { last: Some(now - Duration::from_millis(99)), interval: CAMERA_INTERVAL };
        assert!(!fresh.due(now), "99 ms after a render is too soon");
        let ripe = WorldRenderState { last: Some(now - Duration::from_millis(100)), interval: CAMERA_INTERVAL };
        assert!(ripe.due(now), "100 ms after a render is due");
        // A provider fresh from `provider_for` has never rendered.
        let p = CameraProvider::new("camera_post_1");
        assert!(p.world_render().expect("a camera provider is a world screen").due(now));
    }

    /// THE ROUND-ROBIN: with several due cameras only ONE is picked per
    /// frame, the never-rendered ones first, then the oldest render, ties to
    /// the first candidate; a camera that just rendered goes to the back of
    /// the line. Simulates three cameras over a run of frames with the
    /// production `pick_due` and `WorldRenderState::due`.
    #[test]
    fn due_cameras_render_one_per_frame_in_starvation_order() {
        let t0 = Instant::now();
        let mut last: [Option<Instant>; 3] = [None; 3];
        let mut picks = Vec::new();
        // Frames every 40 ms for 400 ms: 10 frames.
        for frame in 0..10u64 {
            let now = t0 + Duration::from_millis(40 * frame);
            let due: Vec<(usize, Option<Instant>)> = (0..3)
                .filter(|&i| WorldRenderState { last: last[i], interval: CAMERA_INTERVAL }.due(now))
                .map(|i| (i, last[i]))
                .collect();
            let pick = pick_due(&due);
            if let Some(i) = pick {
                last[i] = Some(now);
            }
            picks.push(pick);
        }
        // Frames 0, 1, 2: each never-rendered camera in index order.
        assert_eq!(&picks[..3], &[Some(0), Some(1), Some(2)]);
        // Frame 3 (120 ms): camera 0 rendered at 0 ms, due again (120 >= 100);
        // 1 and 2 are not. Then 1 at 160 ms, 2 at 200 ms, and so on: the
        // order cycles 0, 1, 2 and no camera ever renders twice in a row.
        assert_eq!(&picks[3..6], &[Some(0), Some(1), Some(2)]);
        for w in picks.windows(2) {
            if let (Some(a), Some(b)) = (w[0], w[1]) {
                assert_ne!(a, b, "a camera rendered on two consecutive frames: {picks:?}");
            }
        }
        // Every frame picked exactly one (all ten had a due camera).
        assert!(picks.iter().all(|p| p.is_some()), "{picks:?}");
        // Nothing due: nothing picked.
        assert_eq!(pick_due(&[]), None);
        // Ties go to the lowest index.
        assert_eq!(pick_due(&[(4, None), (2, None)]), Some(4), "first minimum wins, in candidate order");
        assert_eq!(pick_due(&[(2, Some(t0)), (4, None)]), Some(4), "never-rendered beats rendered");
    }

    /// A camera's draw list leaves out exactly ITS OWN screen's quads (the
    /// texture it renders into must not also be sampled in the pass) and
    /// keeps everything else, other screens' quads included. Two screens
    /// share a material only through their own quads here, so material
    /// identity is the test.
    #[test]
    fn a_camera_never_draws_its_own_screen_quad() {
        use crate::engine::screens::{QuadGeom, ScreenQuad};
        let geom = QuadGeom { origin: Vec3::ZERO, u_axis: Vec3::X, v_axis: -Vec3::Y, normal: -Vec3::Z };
        let quad = |id: &str, surface: usize, material: usize| ScreenQuad {
            id: id.into(),
            geom,
            surface,
            mesh: 0,
            material,
            position: Vec3::ZERO,
            yaw_deg: 0.0,
        };
        let quads = vec![quad("cam_screen", 0, 7), quad("tasks_screen", 1, 9), quad("cam_screen_twin", 0, 8)];
        let obj = |material: usize| RenderObject {
            fade: 0.0,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh: 0,
            material,
        };
        let opaque = vec![obj(3), obj(7), obj(9), obj(8), obj(3)];
        let kept: Vec<usize> = without_own_quads(&opaque, &quads, 0).iter().map(|o| o.material).collect();
        assert_eq!(kept, vec![3, 9, 3], "surface 0's two quads (materials 7 and 8) go; the rest stay");
        let kept: Vec<usize> = without_own_quads(&opaque, &quads, 1).iter().map(|o| o.material).collect();
        assert_eq!(kept, vec![3, 7, 8, 3], "surface 1 drops only material 9");
        let kept = without_own_quads(&opaque, &quads, 5);
        assert_eq!(kept.len(), opaque.len(), "a surface with no quad drops nothing");
    }

    /// The registry resolves both rung-3 schemes to their providers, and
    /// the camera provider asks for the scene's format while the live one
    /// keeps the default.
    #[test]
    fn provider_for_resolves_the_watch_and_camera_schemes() {
        let live = provider_for(&ScreenSource::parse("watch:shaostoul")).expect("watch: has a provider");
        assert_eq!(live.kind(), "watch");
        assert!(live.world_render().is_none(), "a live stream is not a world screen");
        let scene = wgpu::TextureFormat::Bgra8UnormSrgb;
        assert_eq!(live.surface_format(scene), crate::gui::screen_surface::SURFACE_FORMAT);
        let cam = provider_for(&ScreenSource::parse("camera:camera_post_1")).expect("camera: has a provider");
        assert_eq!(cam.kind(), "camera");
        assert_eq!(cam.surface_format(scene), scene, "the world is rendered straight into a camera surface");
        assert!(provider_for(&ScreenSource::parse("inventory")).is_none());
        // Every other wired kind resolves too; the registry is one match.
        assert_eq!(provider_for(&ScreenSource::parse("video:x.webm")).expect("video: has a provider").kind(), "video");
        assert_eq!(provider_for(&ScreenSource::parse("web:https://united-humanity.us")).expect("web: has a provider").kind(), "web");
    }

    /// Before its first render a camera screen shows a waiting notice; a
    /// missing post names itself; a rendered view shows no notice at all.
    /// The rig's status fields follow the same state.
    #[test]
    fn notice_and_status_follow_the_render_outcome() {
        let mut p = CameraProvider::new("camera_post_9");
        assert_eq!(p.notice_text().as_deref(), Some("Camera camera_post_9: waiting for its first picture"));
        assert_eq!(p.status()["live"], false);
        assert_eq!(p.status()["renders"], 0);
        p.outcome = Some(Err("No camera post named camera_post_9 is placed".into()));
        assert!(p.notice_text().unwrap().contains("camera_post_9"));
        assert_eq!(p.status()["camera_error"], "No camera post named camera_post_9 is placed");
        assert!(p.status().get("error").is_none(), "the provider never emits a bare `error` key (the IPC's own failure field)");
        p.outcome = Some(Ok(()));
        p.renders = 3;
        assert!(p.notice_text().is_none(), "a rendered view is never painted over");
        assert_eq!(p.status()["live"], true);
        assert_eq!(p.status()["renders"], 3);
        assert_eq!(p.status()["camera"], "camera_post_9");
        assert!(p.status()["camera_error"].is_null(), "no error on success");
    }

    /// `renders` counts REAL renders only. The engine's `render_view`
    /// returns false when it skipped (the world not loaded), and a skipped
    /// attempt leaves the count, the outcome and the notice exactly as they
    /// were; a real one counts, becomes the outcome, and clears the notice.
    /// So `renders > 0` in the done file means a picture was drawn.
    #[test]
    fn a_skipped_render_is_not_counted() {
        let mut p = CameraProvider::new("camera_post_1");
        p.notice_shown = Some("Camera camera_post_1: waiting for its first picture".into());
        p.record_render(false);
        assert_eq!(p.renders, 0, "a skipped render does not count");
        assert!(p.outcome.is_none(), "and does not claim a picture");
        assert!(p.notice_shown.is_some(), "and the notice on the texture is still there");
        assert_eq!(p.status()["renders"], 0);
        assert_eq!(p.status()["live"], false);
        p.record_render(true);
        assert_eq!(p.renders, 1, "a real render counts");
        assert_eq!(p.outcome, Some(Ok(())));
        assert!(p.notice_shown.is_none(), "the view replaced the notice");
        assert_eq!(p.status()["live"], true);
        p.record_render(false);
        assert_eq!(p.renders, 1, "a later skip keeps the last picture's count and outcome");
        assert_eq!(p.outcome, Some(Ok(())));
    }

    /// DATA WIRING. Both shipped catalogs offer a `camera_post` with a
    /// camera; home.ron places one in the garden, and every `camera:` screen
    /// in home.ron names a post that is actually placed (a typo would show
    /// a notice on the wall forever). Resolved through the real placement
    /// path with the home zone's footprint.
    #[test]
    fn shipped_homes_place_a_camera_post_and_the_console_screens_name_it() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        for file in ["home.ron", "home_solo.ron"] {
            let home = MachineHome::load(&data.join(file)).unwrap_or_else(|| panic!("{file} parses"));
            let def = home.catalog.get("camera_post").unwrap_or_else(|| panic!("{file} catalogs a camera_post"));
            assert!(def.camera.is_some(), "{file}: camera_post has a camera");
            assert_eq!(def.category, "Displays", "{file}: the post sits with the displays in the palette");
        }
        let home = MachineHome::load(&data.join("home.ron")).expect("home.ron parses");
        let zones = [ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (55.0, 89.0, 3.0) }];
        let placements = home.placements(&Default::default(), Some(&zones));
        let posts = camera_posts_from(&placements);
        assert!(!posts.is_empty(), "home.ron places at least one camera post");
        let mut camera_screens = 0;
        for p in &placements {
            let Some(screen) = p.screen.as_ref() else { continue };
            if let ScreenSource::Camera(id) = ScreenSource::parse(&screen.source) {
                camera_screens += 1;
                assert!(posts.iter().any(|(pid, _)| *pid == id), "screen {} names camera post {id:?}, which is not placed", p.id);
            }
        }
        assert!(camera_screens >= 1, "the console room has a camera screen");
        // The garden post looks along +X at the tower rows (rotation 270
        // turns the -Z front to +X) and a little down.
        let (_, pose) = posts.iter().find(|(id, _)| id == "camera_post_1").expect("camera_post_1 is placed");
        let cam = camera_from_pose(pose, 16.0 / 9.0);
        let f = cam.forward();
        assert!(f.x > 0.9, "the garden camera looks along +X: {f:?}");
        assert!(f.y < 0.0, "and a little down: {f:?}");
    }
}
