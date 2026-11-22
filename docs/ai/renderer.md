# Renderer, shaders and planet-scale numbers

**Who reads this:** anyone changing the renderer, a shader, a pipeline or bind group, or math that
carries planet-scale positions. Before fencing a visual increment, also read CLAUDE.md's
"Realistic-first" rule; before booting anything, `docs/ai/game-boots.md`.

Moved from `CLAUDE.md` on 2026-10-23 with the wording unchanged; CLAUDE.md keeps a one-line
version of each rule.

**What the renderer lacks and the order to build it** (anti-aliasing and upscaling first, then
reflections, then skin and cloth): `docs/design/renderer-roadmap.md`. Its section 1 lists the limits
every new pass lives in: 14 of the 16 sampled textures a stage already bound in the megashader, 6 of
8 storage buffers, 32 bytes of colour targets a pass, no sampler LOD bias in wgpu, ray queries in
wgpu 24 on Vulkan only while Windows runs DirectX 12, and the scene pass clearing depth (so motion
vectors must come from every pass, not from one depth buffer).

## The PBR shader files

The PBR shader is a set of numbered files under `assets/shaders/pbr/`, not one megashader file;
the list that matters, and its order, is `src/renderer/shader_loader.rs` (`pbr_simple.wgsl` has
not existed for a long time). `90-fragment-main.wgsl` is the shared shader tail: more than two or
three agents editing the SAME shader branch create genuine 3-way-merge correctness hazards, not
just merge chores (`docs/ai/orchestrator.md`, section 4).

## The megashader's class entries

- **The PBR megashader is six class entries, one per PSO, and no `fs_main`: a heavyweight branch costs every pixel of every pipeline that compiles it, even untaken** (H1: the cloud march cost terrain 44.5 ms with clouds off; `docs/design/frame-cost-arc.md`, P1 to P3). A new heavyweight branch gets its own `override` switch, lives in ONE class entry (`pipeline.rs` `ShaderClass`, `PSO_REGISTRY`), is off in every pipeline that cannot draw it, and its switch is the FIRST operand of the guard (`if (HAS_X && type_test)`). Every draw loop picks its PSO by the object's class (`Pipeline::opaque_for` / `transparent_for` / `overlay_for`); a material on an entry without its branch draws the default panel grid, with no error in release. Group the celestial transparent list by planet LAYER BAND (`renderer/celestial_order.rs`), never by class (a class key sank the fallback dome under the sea). Keep `permutation_tests` (`pipeline.rs`) and the switch check in `shader_loader::validate_wgsl`: naga silently ignores an unknown override key, so a rename or a stale assets mirror re-enables the branch. Measure with the rig (same-boot A/B through the shader hot reload), not by reading.

## Verify by booting and entering the world

- **Verify every renderer, pipeline, bind-group or shader change by BOOTING the release exe AND ENTERING THE WORLD: `just verify-runtime`** (~3 min, needs the GPU, so never part of `just verify`; it boots the release binary in the portable rig, enters the world by autopilot and fails on any `PANIC`). Static checks miss both failures it exists for: wgpu device-limit rejections (v0.782 to v0.784 shipped unbootable, "a flicker but the game never comes up"; limits are now `wgpu::Limits::default()`) and world-entry panics (a missed lazy bind group, ten releases, "insta crashes when I press esc"). After changing a bind group layout, every `create_bind_group` site built from it must carry the same bindings: `tests/bind_group_entry_lint.rs` (in `just lints` and CI, since 2026-10-19) checks every site against its layout and fails closed on a site it cannot resolve. A quick manual boot, after `cargo build --features native --release`, is `just launch-bg` (freshness gate, no focus, no hand-off to a newer archive; a hand-typed `target/release/HumanityOS.exe` skips all three), then grep `%APPDATA%/HumanityOS/logs/run.log` for `PANIC` and `taskkill //PID <pid>`; world entry goes through the rig because the in-place exe refuses autopilot when a real identity exists. When the operator reports a boot failure, read `run.log`/`crash.log` FIRST: the panic hook always writes the cause there.

## Shadows: two maps, and what draws into each

- **The far sun map** (`celestial.rs`, 4096 square over 1.5 km, 0.73 m texels) is drawn from the
  CELESTIAL list only: terrain, near trees, wave crests, planet build pieces. **The shadow atlas**
  (`local_shadows.rs`, since 2026-11-10, plan: `docs/design/local-light-shadows.md`) is drawn from
  the SCENE lists (homes, furniture, every body) and the celestial build pieces, once a frame before
  the celestial pass: the budgeted room lights' cube faces and spot views, and the near sun view the
  shader multiplies into every sun term after the far map. A new caster list goes into
  `render_shadow_atlas`'s `lists` or it casts nothing; a material that emits light never casts.
- Group 0 bindings 8-10 are the atlas, its comparison sampler and its table. The atlas pass binds
  the 1x1 stand-in at 8 (it writes the atlas). A new skinned or otherwise special caster draws
  through a shadow PSO with the sun shadow PSOs' state (standard Z, `Less`, no culling) and a
  `PSO_REGISTRY` row, as the skinned shadow pair does.
- **Atlas casters cost CPU per draw, not per triangle** (about a microsecond each in wgpu 24's
  recording, measured 2026-11-11: 3,540 classic draws were 3.4 ms). Plain opaque surfaces therefore
  take the FAST PATH (`local_shadows.rs` `FastPath`, its own small shader
  `assets/shaders/shadow_atlas_instanced.wgsl`, no `PSO_REGISTRY` row since it is not the
  megashader): sorted by mesh, one instanced draw a run, the model matrix a per-instance attribute,
  no per-caster binding. Only what vs_main moves (wind classes 19 to 23, the water shell) or cuts out
  (`fs_shadow`) keeps the classic path (`takes_fast_path`). Showcase `shadow_fast` "0" is its red
  twin: the picture must not change. One draw per mesh across the whole atlas FOR EVERY CASTER (tile
  mapping in the vertex shader, a discard outside the tile) was tried and took `gpu.shadow_local`
  from 0.28 to 10.9 ms, since nothing then clips a face's triangles to the face. Rung 1b
  (2026-11-12) measured two more ways and took both out: drawing only the casters wholly inside a
  face across the atlas (each view's matrix remapped onto its tile) saved no draws, because most
  draws are unique meshes in one or two faces; replaying a view's unchanged draws from a render
  bundle saved 4 to 12%, because a replayed draw costs about what a recorded one does (wgpu replays
  bundles into the command list draw by draw, and the D3D12 calls per draw are the cost, not wgpu's
  validation). Only FEWER draws cut the CPU: the shadow-caster cull (`shadow_reaches_view`, a
  caster drawn only if its shadow can reach the view; showcase `shadow_cull` "0" is its red twin),
  and later rung 2's kept static depth. wgpu 24 has no clip distances (`enable clip_distances` is
  unimplemented in naga 24, no `CLIP_DISTANCES` feature), so "one draw per light with clip planes"
  waits for a wgpu upgrade.
- **The plain casters are GPU-DRIVEN since 2026-11-13** (`renderer/shadow_gpu.rs`,
  `assets/shaders/shadow_cull.wgsl`; `docs/design/local-light-shadows.md` section 10): their meshes
  live in ONE arena (copied on the GPU from each mesh's own buffers, which is why
  `Mesh::from_vertices` makes them `COPY_SRC`; keyed by the serial `add_mesh` and `replace_mesh` give
  each slot, `Renderer::mesh_serials`), a compute pass culls every caster against every drawn face by
  the processor's own rule and writes the indirect arguments, and each face is ONE
  `multi_draw_indexed_indirect_count`. **The cull's rule is written three times and kept equal**:
  `local_shadows::face_keeps` / `sun_keeps` (the per-draw loop, and which faces get a tile),
  `keeps` in the shader, and its Rust twin `shadow_gpu::gpu_keeps`; change one, change all three
  (`the_gpu_test_keeps_exactly_the_casters_the_processor_rule_keeps`). The instance matrix is a
  VERTEX ATTRIBUTE, never `@builtin(instance_index)`: DX12 hands an indirect draw's first instance to
  attribute fetch but not to the builtin on this adapter (`patch_arena`'s header). A mesh whose
  vertices change after it is made (the compute-skinned bodies, `prev_vertex_buffer`) or that is a
  range of shared buffers is never copied into the arena (it would hold last frame's copy). Since
  2026-11-14 the draws are DRAW SETS (`shadow_gpu::DrawSet`, one pipeline over one vertex source,
  one command a set a face): plain, card-hair cutouts per material (`shadow_atlas_cutout.wgsl`,
  `fs_shadow`'s type 26 test written again: change one, change both,
  `the_cutout_sets_test_is_fs_shadows`) and the see-through sheets (shadow_transmit's pipeline,
  whose 80-byte instance is the cull's), and a compute-skinned body's set reads its OWN buffers,
  the vertices the main view draws. Rung 4 (2026-11-14) added the SWAY sets (types 19 to 22,
  `shadow_atlas_sway.wgsl`: vs_main's `wind_displace` copied verbatim, so a change to the wind is
  made in both, `the_sway_sets_wind_is_vs_mains` compares the two; the wind and clock are the
  per-draw loop's `lit_uniform` values) and the SHADOW POOL (`renderer/shadow_pool.rs`, a child
  module of local_shadows): each middle-tier body that casts is compute-skinned once into it
  (`skinning.wgsl`'s `out_base`, `flags` bit 0 = no last-pose write) and its regions drawn as plain
  and card-hair sets. Still on the per-draw loop: the sprite tree cards and grass. Showcase
  `shadow_gpu` "0" is the per-draw loop's red twin, `shadow_gpu_more` "0" the 2026-11-13 state
  (plain arena casters only), `shadow_gpu_sway` "0" and `shadow_gpu_crowd` "0" rung 4's twins; a
  device without the three
  features (`shadow_gpu::required_features`) runs the per-draw loop and Settings names what it lacks.
  Measured (2026-11-13): `cpu.shadow_local` in the hall 2.52 to 1.54 ms, the plain casters' 933
  instances in 46 commands (the 2026-11-14 numbers: `docs/design/local-light-shadows.md` section 10).
- **Code in `frag_tail` costs even when its branch is never taken.** The room light's atlas lookup
  slows the scene pass by 0.4 to 1.65 ms with shadows OFF (2026-11-11, RTX 4070 at 2560 x 1387:
  the ground hall 13.16 ms against 11.51 with the lookup compiled out, the crowd 8.59 against 7.54),
  because a shader's register count is set by its fullest path. Lighting the shadowed lights in a
  loop of their own before the clustered loop was tried and was worse (off the same, on 1 ms more).
  Measure every new per-light or per-pixel term the same way, through a hot-reload A/B
  (`probe-hot-ab.js` patches): `gpu.scene` with the term compiled in but switched off, against
  compiled out. The fix for this one landed in rung 1b (2026-11-12): while Room light shadows is
  Off the surface, vegetation and terrain PSOs are TWINS with `HAS_SHADOW_ATLAS` false
  (`pipeline.rs` `ATLAS_TWINS`; a twin's label is its row's plus " (no shadow atlas)", and
  `pso_row` / `pso_constants` answer it), built on a background thread when the level crosses Off
  (`Pipeline::want_shadow_atlas`, called from `render_shadow_atlas`); a hot reload builds the variant
  the frame wants. A new feature switch that costs at Off gets the same treatment: a twin table, not
  a second registry. Showcase `atlas_code` "1" keeps the lookups in at Off (the red twin).

## Temporal anti-aliasing: the motion target and the jitter (rung 1a, 2026-11-13)

- **Every scene pass carries TWO colour attachments**: the scene colour and the MOTION target
  (`taa::MOTION_FORMAT`, Rgba16Float: xy = UV offset from last frame, z = the responsive mask, w = 1
  where a surface was drawn, 0 = the sky sentinel). The celestial opaque pass clears it
  (`Renderer::motion_attachment(MotionLoad::Clear)`), the scene opaque pass clears it only when no
  celestial pass ran (`ClearIfFirst`), every other pass loads it. **A PSO that draws in one of those
  passes takes its targets from `taa::scene_targets(format, blend, writes_depth)`**: a depth-writing
  PSO writes all of the motion, a blended one only the mask (Max). `pipeline::taa_motion_tests`
  (`every_scene_pso_writes_motion`) scans every builder in `src/renderer` that compiles a registry
  fragment entry and fails on one without it. An off-screen view (camera screens, the hi-res
  capture) swaps a view-sized motion target in with its depth (`begin_view_depth`).
- **Every class entry returns `FragOut`** (`frag_out(in, colour, responsive)`, 00-bindings-vertex.wgsl):
  a new return in a class entry wraps its colour the same way. `vs_main` and `vs_skinned` add the
  jitter in clip space (`taa_jitter`) AFTER `camera.view_proj`, which stays unjittered, and hand the
  fragment the unjittered clip position now and last frame (`motion_cur`, `motion_prev`).
- **Last frame's position** is `camera.prev_view_proj` (this frame's render space: the celestial pass
  folds in the locked body's placement, `taa::frame_delta`; the scene pass the home's offset, since the
  scene list is the home moved by `station_off`) times `vs_prev_model()` (the motion book's model for a
  mesh drawn once a frame, `ObjectUniforms::prev_model`) times the vertex last frame (vertex slot 2,
  `mesh::prev_position_layout`: a near body's last-frame skin, every other mesh's own buffer again; the
  wind at `camera.taa.w`, the previous clock; the water at the previous clock). Bodies take last frame's
  pose from the PREVIOUS PALETTE (`skin_upload_prev`, each person's rows found by the key the engine
  passes to `skin_push_palette`). A new vs_main draw site binds slot 2 next to slot 0 or fails wgpu's
  validation at its first draw. **Check a motion change on the Dev page's Image card** (showcase
  `taa_view` "1"): a parked camera's still surfaces must read black (a wrong origin delta lit a whole
  room, 2026-11-13).
- **Jitter and motion are stamped only into the main view's colour passes before the resolve**
  (`Renderer::colour_uniform`, the celestial pass's write, `taa_camera`): the shadow passes' camera
  buffers come from `Camera::uniforms()`, which fills `prev_view_proj = view_proj` and `taa = 0`. A new
  colour pass drawn before the resolve writes its camera through `colour_uniform`; one after it (or a
  shadow view) must not, or it jitters.
- **The resolve** (`taa_resolve`, after the transparent pass, before the overlay, lines and particles)
  is timed as `gpu.taa` (compute resolve plus the sharpen pass, summed). With
  `humanity_core::reuse::enabled()` false the history and the motion book are discarded every frame.
  A rig window capture waits `taa::SETTLE_FRAMES` resolved frames after a reset (`poll_screenshot_request`).
- **Hashed alpha (rung 1b, 2026-11-13, `renderer/hashed_alpha.rs`)**: `camera.taa_hash` (x the jitter
  phase, y = 1 while on) is stamped beside `taa` by `taa_camera`, so it is on only in the main view's
  colour passes under Temporal (off at Off, with the showcase `taa_hashed` "0", the red twin, and
  with the reuse switch off, since nothing would then average the pattern). A cutout that wants a
  soft edge compares its alpha with `taa_hash_threshold(in.clip_position.xy)` inside
  `if (taa_hashed())` and keeps its hard cut in the `else` branch: that branch is what `fs_shadow`
  mirrors and what `shadow_cutout_tests` reads, and the shadow passes keep the hard cut (a shadow
  texel is filtered by PCF, not by a history). Users today: the hair's core (which then draws the
  fringe, so `hashed_skips` leaves the fringe pass out of the main view, from the same frozen
  `FrameNow::hashed`, so the two never disagree), the far tree cards (type 12, with the stage's far
  end dissolving over its last tenth) and the level-of-detail crossfade in `frag_prologue`. The
  threshold's CPU twin is pinned by `the_shader_hash_is_the_twin`.

## Mirrors: a second view of the scene each frame one is in view (2026-11-21)

- **A mirror's reflection is an off-screen view with textures of its own** (`renderer/mirror.rs`,
  design `docs/design/mirrors.md`): `Renderer::mirror_begin` swaps the mirror's depth buffer and
  motion target in for the window's (as `begin_view_depth` does for the camera screens, but not the
  shared spare, which a 10 Hz camera and an every-frame mirror would reallocate in turn), clears its
  colour and sets the shadow uniform's light-tile size to 0 (the tiles are binned for the main
  view's screen; the reflection loops the whole light list); `mirror_end` puts all three back.
  `offscreen_view_active` and `film_blur_ready` count it as an off-screen view (no jitter, no motion
  anything reads, no film blur). Its passes publish under `SceneView::Mirror` (`gpu.mirror_*`).
- **The reflection camera is off-axis** (`Camera::off_axis`: when set, `view_matrix`,
  `projection_matrix` and `effective_position` are its matrices and eye). Anything that builds a
  projection from `fov_degrees` and `aspect` itself (the star pass, god rays, SSAO) is wrong for it,
  which is one reason the reflection draws the scene and see-through passes only. A new pass drawn
  into a reflection reads the camera's matrices, never its angles.
- **The player's twin for the glass**: `Skinning::mirror_twins` (`skin_mirror_twin`) is drawn by
  `draw_skinned_middle` only while a reflection draws; `MirrorView::eye_only` names the meshes the
  reflection leaves out (the eye's folded first-person body). A new body path that draws the player
  for the eye must say what the mirror sees instead.
- **The glass is type 24 with `params2.y = 1`** (`MIRROR_PARAMS2`): its texture holds tone-mapped
  scene colour, turned back into light by `aces_inverse` (80-fragment-shared.wgsl) before the tail's
  curve maps it again. `aces_inverse`, the tail's ACES curve and `present.wgsl`'s `aces_inv` are in
  LOCKSTEP: change one, change all three.

## The tree card stage needs depth-15 ground (BUG-301, 2026-11-14)

- **The far tree cards (type 12's sprite branch in `fs_terrain`) are baked into terrain patches at
  depth >= `TREE_MIN_DEPTH` (15) and nowhere else**, and the planet's own cap without the 460 m tile
  tier is 13. `humanity_terrain::tree_card_stage::CardFloor` (built by
  `engine/tree_card_stage.rs` `floor_for`, passed as `ChunkParams::card_floor`) raises the cap to 15
  within the "Tree silhouette distance" on a breathable world with trees on; without it no release
  and no rig drew a single card, and every forest capture looked like a bare slope behind the
  near models. Anything that changes the patch depth caps, `TREE_MIN_DEPTH` or the card distance
  must keep the floor's test (`a_game_without_the_tile_tier_still_has_a_card_stage`) green.
- **Count the cards before judging a picture of them**: the `[TreeCards]` log line (1 Hz) and the
  `tree_cards` object in every screenshot's done file give the drawn patches, the card patches, the
  cards, the cards inside the kept window (hide radius to far cutoff), the deepest drawn patch and
  whether the sprite atlas is baked. A vantage with `min_tree_cards` fails probe-sweep without them
  (`scripts/lib/tree-cards-check.js`; verify-runtime's `fuji-forest-ground` carries it), and the
  showcase `card_floor` "0" is the red twin (`fuji-forest-ground-no-floor`, a control that passes
  only by failing).
- **The cards are culled on the GPU before they are drawn (2026-11-14, `renderer/tree_card_cull.rs`,
  `assets/shaders/tree_card_cull.wgsl`).** Each patch mesh ends in its cards (the patch builder
  emits the skirt BEFORE the trees: keep it so, or `card_tail` finds no tail and the patch
  draws whole, unculled; `vegetation_bakes_into_deep_land_patches_deterministically` holds it). The
  arena keeps one cull record a card face (`CardRecords`); one compute dispatch a frame keeps a face
  for the main view when its bounding sphere meets the kept window and the camera's frustum, and for
  the sun view when it meets the shadow map's box, and writes compact index buffers and one indirect
  draw a patch. **The main view's kept indices go into a range reserved INSIDE the patch arena's
  index buffer** (`CardCull::region`, grown at upload), so one multi-draw carries each patch's
  ground and then its kept cards in the frame loop's front-to-back order, the order the whole-patch
  draw had. Do not split it into "all ground, then all cards": the first cut did, and at
  `fuji-forest-ground` it cost 2 ms MORE than no cull (23.0 against 21.1 ms `gpu.celestial`), since
  near cards drawn early had been hiding the ground behind them from the depth test. The sun pass
  draws its ground per patch (`Renderer::patch_icount`) and then its kept cards from a buffer of its
  own, ONE winding a face (it draws without face culling). The cull reads the SAME
  uniform buffers the draws read (the celestial camera, the light camera, the shadow uniform's hide
  and far, the batch rotation), so it must be dispatched after they are written and before the sun
  pass (`card_cull_dispatch` sits right after the patch instance upload). The shader's rule has a
  Rust twin (`cpu_keeps`, `the_gpu_cull_keeps_exactly_the_faces_the_rust_twin_keeps`). Showcase
  `card_cull` "0" is the red twin; a device without multi-draw indirect draws every patch whole.
  **Measured 2026-11-14** (same boot, medians of four interleaved arms, RTX 4070 at 2560 x 1387,
  default 1,500 m; other agents were compiling, so only the `gpu.*` columns are readings): the
  world surface pass 21.3 to 20.3 ms at `fuji-forest-ground`, 17.4 to 15.8 at `fuji-forest-hillside`,
  13.7 to 12.2 at `fuji-forest-far`, 15.1 to 14.0 at `black-forest-far`; the sun pass 1.9 to 1.6,
  1.9 to 1.5, 1.5 to 1.2 and 2.3 to 1.7; the cull itself 0.02 ms. The profile arms (hot-reload
  patches in the same boot: the card vertices moved off screen, or every card fragment discarded at
  the top of `fs_terrain` and `fs_shadow`) put the cards' whole cost at 6.3 to 8.0 ms of the world
  surface pass before the cull and 5.0 to 7.0 after it: what is LEFT is shading the card pixels
  that show (3.0 to 5.7 ms) and launching and discarding the transparent part of the kept cards
  (1.5 to 2.5 ms, the sprite fills a fifth to a quarter of its square). The next rungs: a hull per
  atlas tile instead of the square (trimmed cards, the transparent area never rasterized), a
  depth prepass for card overdraw, and a lower-detail card set for the sun pass (cards still cast
  1.0 to 1.5 ms there). Agent scratch: `cp-ab2/` (pictures) and `cp-analyze.js`.
- **Each card face is its tile's OUTLINE, not the square (rung 2, 2026-11-14,
  `humanity_terrain::card_hull`).** The bake reads every atlas tile back from the GPU and fits a
  convex polygon of at most six vertices around the texels a bilinear sample can reach (1.25 texels
  past each covered centre), cut down from the hull one edge at a time by least added area and never
  off the tile (Persson's particle trimming; Unreal's particle cutouts). It always CONTAINS the hull,
  so no visible texel is lost and the hashed edge and the far dissolve are decided per pixel as
  before. The patch builder draws each face as that polygon fanned twice (`card_hull::face_indices`:
  `6 (n - 2)` indices, the square's are exactly the old quad's); before the bake, or for a tile
  that failed, the face is the square padded to the patch's size. The cull reads a patch's face size
  off its tail (`card_tail`: 3 to 8 vertices, one size a patch, else the patch draws whole), so keep
  every face of one patch the same size: the builder takes one snapshot of the table a build. The
  `[Bake] card outlines` line gives every tile's covered and outline share of its square (covered 7
  to 17%, the six-vertex outline 30 to 38%). Showcase `card_hull` "0" is the full square (the red
  twin, the rung 1 geometry), "4", "6", "8" the fitted counts; a change rebuilds the card patches in
  place a few a frame (the done file's `tree_cards.hull_pending` counts what is left, `hull` and
  `hull_area` say what is drawn). Why six: over the shipped atlas six vertices cover 0.340 of the
  square, eight 0.337 and four 0.604, and the six-vertex arm was the cheapest at three of the four
  vantages (fewer, fatter triangles).
- **A card WHOLLY inside the hide radius casts no sun shadow (rung 2, an orchestrator decision).**
  Every tree inside that radius has its 3D model, which casts its own shadow; the card's was a second,
  board-shaped one on the ground under the near trees. The cull's sun view drops a face whose sphere
  lies inside the radius (`keeps_sun`, its twin `cpu_keeps`), a face the ring crosses still casts,
  and fs_shadow's per-pixel rule is unchanged (its comment says why). Showcase `card_sun_hide` "0" is
  the red twin. The crop (agent scratch `c2-r2-crop-near-shadow.png`) shows the extra dark blots
  beside the model shadows going; nothing else in the frame moves beyond the capture noise.
- **Measured 2026-11-14 for rung 2** (same boot, medians of four interleaved arms against rung 1's
  square path, RTX 4070 at 2560 x 1387, default 1,500 m; another agent compiling, so only `gpu.*`
  are readings; the eight-vertex outline was the measured arm, six is cheaper still): the world
  surface pass 20.36 to 19.57 ms at `fuji-forest-ground`, 15.98 to 15.61 at `fuji-forest-hillside`,
  12.55 to 11.94 at `fuji-forest-far`, 13.91 to 13.51 at `black-forest-far`; the sun pass 1.31 to
  1.09, 1.22 to 1.10, 0.95 to 0.85, 1.07 to 0.92; the cull 0.03 ms more (0.13 to 0.21);
  `vram.tree_cards` 183 to 192 MB at the ground vantage. Pictures: square against outline differs
  no more than two captures of the same arm. The profile arms (hot-reload patches, same boot) put
  the cards' sun pass share at 0.45 to 0.81 ms, so no lighter sun-pass set was built, and the cards'
  world surface cost at 4.2 to 5.7 ms: 2.2 to 2.5 ms rasterizing and launching the outlines' pixels
  (the visible ones included), 1.8 to 3.2 ms shading the ones that show, 1.0 to 2.4 ms of that the
  lighting tail (`frag_tail`; the sun shadow lookup under 0.9 ms of it, single noisy arms). No picture-neutral cut was
  found for the shading: every cheaper lighting tail changes the far forest's light, and
  `frag_prologue` is a few derivatives, so moving the card's alpha test above it buys nothing. The
  next rungs: a card depth prepass (each card pixel shaded once; a pipeline change, since the
  ground and the cards share one draw), or an agreed cheaper far-card lighting. Agent scratch:
  `c2-r2-ab1/` (pictures, costs), `c2-r2-plan.json`, `c2-r2-analyze.js`.

## f32 at planet scale

- **f32 at planet scale is a defect class.** On the CPU, keep planet positions AND unit directions in f64 (`DVec3`) all the way to a terrain or data sampler, and downcast only at a coarse-data boundary (base heightmap cells are 460+ m, so `.as_vec3()` is fine THERE and nowhere earlier): an f32 unit vector quantizes ground position at ~0.4-0.8 m, an f32 global-grid coordinate at ~3.6 m. Check the whole chain: a CALLER that narrows to f32 undoes an f64-upgraded function (the buoyancy float). On the GPU (f32 by hardware), keep magnitudes small near the camera (floating origin, anchor-relative patch verts, drawn-shell units); a shader phase or UV from a planet-radius f32 dot (`dot(dir * r, d)`) carries ~0.5 m quantization that moves with the camera, so evaluate it in the camera-anchored 64 m-modulus domain (`anchw + dvw`) with wavelengths that DIVIDE 64 and axis-aligned directions, so anchor re-snaps shift phase by whole numbers and the CPU f64 twin matches mod 1 (ocean chop W4/W5/W6 and its lockstep test are the worked example).
