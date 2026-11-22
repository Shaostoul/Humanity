# Mirrors: a planar reflection

**Status:** built 2026-11-21 (rung 1: one mirror at a time). Code: `src/renderer/mirror.rs` (the
reflection camera, the textures, the levels), `src/engine/mirrors.rs` (which mirror reflects, the
glass quads, the pass), the player's whole body for the glass in `src/engine/animation.rs`, the
glass's shading in `assets/shaders/pbr/90-fragment-main.wgsl` (type 24 with `params2.y = 1`).
Settings: Settings > Graphics > Light and sky > Mirror reflections (`src/gui/pages/settings/mirrors.rs`,
the levels in `data/gui/mirror_levels.json`).

## Why

The operator, 2026-11-21: "I think we had a mirror in the bedroom at one point. If a player runs in
front of the mirror will it show the player's model?" It did not. The bedroom's standing mirror was
a wall screen whose source was `profile`: it showed the Profile page on its Skills section. The
orchestrator's call, under CLAUDE.md's realistic-first rule: make it a real mirror, built the way a
2030 release builds one, and keep the Profile page on a screen of its own.

## 1. The technique

A planar mirror is exact when the scene is drawn a second time from the eye reflected in the
mirror's plane. This engine does that with the **generalized perspective projection** (Robert
Kooima, 2008, "Generalized Perspective Projection", the off-axis frustum through a screen rectangle
used for CAVE walls, head-tracked displays and VR mirrors):

1. The eye `E` is reflected in the glass's plane: `E' = E - 2 d n`, `d` the eye's distance in
   front of the glass, `n` the glass's outward normal.
2. The reflection camera stands at `E'` and looks straight through the glass, along `n`, into the
   room, with the glass's up as its up (`Mat4::look_to_rh(E', n, -v)`).
3. Its frustum is the off-axis one whose edges are the glass's four edges and whose **near plane is
   the glass itself** (reverse-Z, the engine's convention).

What follows from those three, and why it is chosen over the textbook alternative (reflect the
camera matrix, project the whole screen, clip with an oblique near plane, Lengyel 2005, "Oblique
View Frustum Depth Projection and Clipping"):

- **Clipping at the glass.** Everything behind the glass (the wall the mirror stands against, the
  mirror's own body two millimetres behind its glass) is nearer to `E'` than the near plane and is
  clipped. That is what Lengyel's oblique near plane does; here the plane to clip by is
  perpendicular to the view axis, so the ordinary near plane is the oblique one, without the oblique
  matrix's loss of depth precision.
- **Every pixel drawn is a pixel the glass can show.** The glass's corners land on the texture's
  corners, so the texture holds the mirror's picture and nothing else, and the glass samples it by
  its own UV (no screen-space projection, no extra uniform, no binding: section 1 of
  `renderer-roadmap.md` counts how few the megashader has left).
- **No pipeline changes.** The view is a proper rotation (its 3 x 3 part has determinant +1, pinned
  by `the_reflection_view_keeps_triangle_winding`), so triangles keep their winding and back-face
  culling works unchanged. A reflection matrix (determinant -1) would turn every front face into a
  back face and need a second set of pipelines with the cull flipped.
- **The mirror image.** A camera behind the glass sees the room turned left for right relative to the
  glass's own u. The glass quad's u is written the other way (`engine::mirrors::glass_vertices`), so
  the texture's x is `1 - u`: the mirror image (`the_reflection_camera_puts_every_point_where_the_glass_shows_it`
  checks every point against the law of reflection worked out without the camera).

**The picture's size follows the glass on screen** (`renderer::mirror::picture_size`): the glass's
four corners are projected with the live camera, the longer of each pair of opposite edges is its
width and height in pixels, times the level's `scale`, the longer side held to the level's
`max_side`, rounded up to 32 pixels so walking about does not make a new texture every frame. A
mirror far away or small on screen costs little; one filling the view is sharp. A picture under 24
pixels is not drawn.

**One mirror at a time, the nearest in view** (`engine::mirrors::pick`): the eye in front of its
glass, within 15 m, the glass not wholly outside one side of the view's frustum, and at least one of
its centre and four corners not hidden behind the home's walls, shut doors and solid floors (the
same blockers the hand's reach uses, `built_uses::reach_blockers`). Every other mirror shows plain
silvered glass (a smooth metal in the glass's tint: it catches the lights as highlights and shows no
picture). Two mirrors facing each other therefore show each other as plain silver: no recursion.
**Nothing is drawn while no mirror is in view**, and the probe counts that (section 6).

## 2. A mirror is data

`humanity_ship::machines::MachineDef::mirror: Option<MirrorDef>`, the infinite-of-X rule: any piece
in any machine catalog may carry one (a standing mirror, a wardrobe door, a bathroom cabinet):

```ron
mirror: Some((
    face: "front",           // which face of the box is glass (as a screen's face)
    frame_m: 0.04,           // the glass's inset: the body's colour shows round it as the frame
    tint: (0.86, 0.88, 0.86) // the share of each colour the glass gives back
)),
```

A silvered glass mirror returns about 85 to 90% of the light, a little more of the green, which the
glass in front of the silver lets through best; the default tint says so.

**The bedroom (both homestead households, `data/machines/home.ron` and `home_solo.ron`):** the
standing mirror (`standing_mirror_1`, the east wall at z 32 to 33, facing into the room) is a mirror
with no screen, in a walnut frame colour. Pressing E at it still opens the look editor (the bedroom's
room type lists `customize_appearance`). **The Profile page moved to its own wall screen beside it**,
`profile_screen_1`: the east wall south of the mirror (z 30.4 to 31.6), its base 1.2 m over the
floor, facing into the room. The bedroom's several TVs for the game pages are another task; until
then this screen keeps the Profile page in the bedroom.
`the_bedroom_has_a_real_mirror_and_still_a_profile_screen` holds both homes to it.

## 3. What the reflection draws

The scene's opaque and see-through lists (the home, its furniture, its glass, its lights), every
person in them, and **the player's own body, whole, in first person too**:

- In first person the eye's copy of the body has its head and neck folded away (character animation
  rung 2b), and its whole twin already exists for the shadow maps. While a mirror reflects, the
  twin's palette rows are pushed whether or not the shadow maps want them, the twin is queued for
  the glass (`Renderer::skin_mirror_twin`, drawn by the vertex-skinning pipeline in the reflection's
  scene pass only), and the eye's folded copy is left out of the reflection
  (`MirrorView::eye_only`, `engine::mirrors::reflection_list`).
- With Settings' "Show my body in first person" off (or rung 2b off), first person draws no body;
  the player is posed for the glass alone (`Person::mirror_only`) and drawn the same way.
- In third person the body is in the scene lists already.
- Other players and the crew are in the lists (near bodies as their meshes, the middle tier skinned
  in the vertex stage), so they are reflected as they are drawn.

**Not drawn, for now:** the sky and the planets (a window seen in a mirror shows black, as a camera
screen's does: the celestial pass keeps one temporal history fitted to the player's view, the reason
`ViewPasses::SceneOnly` gives), the editor overlay and lines, and the hair's soft fringe (the
reflection draws the hair's hard-cut core; the main view's fringe comes from temporal anti-aliasing,
which an off-screen view does not run).

## 4. The frame

Everything is in the HOME frame (the frame the draw lists are in when the camera screens render;
lib.rs adds `station_off` to them later); the live camera is in the render frame, so its eye is moved
by `-station_off` (`engine::mirrors::eye_home`).

1. `mirrors::choose` (frame_lists_home.rs, before the home's lists are filled and the bodies posed):
   the reflecting mirror and its picture's size, or none.
2. The bodies (frame_people.rs, `animation::draw_people`): the player's whole body queued for the
   glass while one reflects.
3. `mirrors::render` (frame_load_screens.rs, right after the camera screens, before the live scene
   pass): the lists culled to the reflection's frustum, whose far plane is 80 m from the reflected
   eye (`REFLECTION_FAR_M`: longer than any room's sight line, shorter than the homestead), by each
   mesh's bounding sphere
   (`Renderer::cull_to_view`, `cpu.mirror_cull`: the glass is a narrow window onto the room, and
   every draw costs the processor about a microsecond whatever it covers), then
   `Renderer::mirror_begin` (the mirror's own colour, depth and motion textures swapped in,
   the colour cleared, the light tiles off), the scene and see-through passes under
   `SceneView::Mirror`, `Renderer::mirror_end` (the window's depth and motion back, the tiles back).
   The glass's material (type 24, `MIRROR_PARAMS2`) samples the texture in the live scene pass.

**The tone curve.** Every scene fragment ends with the ACES curve (80-fragment-shared.wgsl), so the
reflection's texture holds tone-mapped colour. The glass turns it back into light (`aces_inverse`, in
lockstep with the curve and with `present.wgsl`'s `aces_inv`), multiplies it by its tint, and the
shared tail maps it once more: the reflection is the room's light times the glass's, through one
curve, not two.

**The lights.** The tiled light lists are binned for the main view's screen (`light_tiles.rs`), which
the reflection's pixels do not share, so while it draws the shadow uniform's tile size is 0 and every
pixel loops the whole list (the classic loop). The lights themselves are last frame's (they are set
later in the frame), as the camera screens' are.

**Reuse.** Nothing is kept across frames as data: the picture is drawn anew on every frame a mirror
is in view. Only the textures' memory is kept while the picture's size holds (as a camera screen
keeps its depth buffer), and given back when the level goes Off or no mirror is placed. So there is
nothing for the Dev page's "Turn off all reuse" switch to turn off.

## 5. Settings > Graphics > Light and sky > Mirror reflections

The levels are data (`data/gui/mirror_levels.json`), each saying what it changes and costs:

| Level | Draws the glass at | Longest side | Most video memory (a full-height mirror) |
|---|---|---|---|
| Off | nothing: plain silvered glass | | none |
| Low | half the screen's sharpness each way (a quarter of the pixels) | 1024 px | about 10 MB |
| High (default) | the screen's sharpness | 2048 px | about 40 MB |

Memory is 20 bytes a pixel: the colour (four 16-bit floats), the depth (32-bit) and the motion target
every scene pass writes beside its colour. The control's cost line is measured live
(`gpu.mirror_scene`, `gpu.mirror_transparent`, `cpu.mirror_scene`, `cpu.mirror_cull`, `vram.mirror`),
and a line under it says what the mirrors were doing in the world ("In the world: a mirror reflects
at W x H pixels" or "no mirror is in view"; Settings is a page over the world, which draws nothing
while it is open, so the line keeps the last world frame's answer). The Performance page shows the pass as its own row (Mirrors, in the GPU and CPU pies; its
memory in Render targets). Works on every graphics card the game runs on: it needs nothing past what
the scene pass already uses.

## 6. Budget, measured

The target: well under 2 ms at 2560 x 1440 on the operator's RTX 4070 with a mirror in view, nothing
without. Measured 2026-11-21 with the rig (`home-bedroom-mirror` and its twins, the RTX 4070 at
2560 x 1387, `HUMANITY_FRAME_COSTS=1`), the standing mirror 2.25 m away filling about 290 x 560
pixels of the window:

| | High (picture 320 x 576) | Low (160 x 288) | No mirror in view, or Off |
|---|---|---|---|
| `gpu.mirror_scene` + `gpu.mirror_transparent` | 1.31 + 0.005 ms | 0.50 + 0.005 ms | 0 |
| `cpu.mirror_cull` + `cpu.mirror_scene` + `cpu.mirror_transparent` | 0.18 + 2.17 + 0.44 ms | 0.33 + 2.08 + 0.53 ms | 0 |
| draws sent (opaque, see-through) | 744 of 3,180, 10 of 43 | the same | none |
| `vram.mirror` | 3.7 MB | 0.9 MB | 0 (given back at Off) |

**Every capture was contaminated**: other agents' builds shared the machine throughout (the probe
marks it), and the main scene pass measured in the same captures, 18.5 ms on the graphics card and
10.6 ms of submission, is about twice what it is on a quiet machine, so these figures are high. The
graphics card's share is in proportion to the pixels (the reflection is 5% of the window's pixels and
7% of the scene pass's time) and inside the 2 ms target; the processor's share is the submission of
744 draws, a quarter of the scene's, about a fifth of the scene pass's processor time. That is the
part to make cheaper next (the GPU-driven draws the shadow atlas uses, or culling by the home's
rooms rather than by the frustum alone), and the first thing to measure again on a quiet machine.

## 7. Tests

- `renderer::mirror::tests`: the reflection camera puts every point where the law of reflection says
  the glass shows it (the flip included); what lies behind the glass is clipped; an eye behind the
  glass sees nothing; the view keeps triangle winding; the picture follows the glass on screen; the
  shipped levels are Off, Low, High.
- `engine::mirrors::tests`: a mirror in view is picked and one out of view (turned away, behind the
  glass, too far, hidden) is not; the nearest of two reflects; the glass samples left for right; the
  reflection leaves out its own glass and the eye's folded body and nothing else; the bedroom has a
  real mirror and still a Profile screen, in both homes.
- `scripts/tests/mirror-check.test.js`: the rig's judge.
- The rig (`tests/visual/vantages.json`): `home-bedroom-mirror` (a pass on every frame, counted by the
  game's mirror probe from the passes the renderer drew; and the player's body in the glass, by the
  shown/hidden picture method, showcase `mirror_me` 0), `home-bedroom-mirror-nobody` (the figure
  check's red twin on real pixels: the player left out of both pictures, the check must fail),
  `home-bedroom-mirror-away` (turned round: not one pass), `home-bedroom-mirror-off` (Off: not one pass), `home-bedroom-mirror-low` (Low: a smaller
  picture).

Showcase keys: `{"mirror":"0"}` (no reflection, the red twin), a level id pins it, `"auto"` hands it
back to Settings; `{"mirror_me":"0"}` leaves the player out of the reflection; `{"mirror_probe":"1.0"}`
writes `debug/mirror_probe.json`.

## 8. Later rungs

- **Several mirrors at once**, each its own texture by a budget (the nearest N), and a mirror seen in
  a mirror (one bounce: the second mirror's last-frame picture, which is a reuse and so obeys the
  switch).
- **The sky and the planets in the reflection**: a celestial pass with a temporal history of its own
  per view, the same rung camera screens wait for.
- **The hair's fringe** in the reflection (the transparent fringe pass for the twin).
- **Rough, fogged or tinted glass**: a blurred reflection (the roughness pyramid of
  `renderer-roadmap.md` section 4), a bathroom mirror after a shower.
- **Wall mirrors from the old layouts** (`WallKind::Mirror`, the Fibonacci homestead's flush panel)
  as mirror data.
- **Mirrors on a planet**: the player's body stands in render space there, outside these lists.
- **Windows and polished floors** are not mirrors: they are the reflections of
  `renderer-roadmap.md` section 4 (screen-space reflections and room captures), which this pass does
  not replace.
