# Shadows from room lights, people and furniture

Written 2026-11-10 after the operator's report: "I've also noticed player characters and furniture
don't see to have shadows." This is the plan of record for every shadow the near world casts: from
the room lights, from the sun near you, and later from contacts and ray queries. Section 1 is what
cast what before it (measured from the rendered pixels), section 2 the whole feature as a 2030
release would build it, sections 3 to 8 its budgets and rules, section 9 the rungs, section 10 what
rung 1 built and measured. Code: `src/renderer/local_shadows.rs`; levels:
`data/gui/shadow_levels.json`; shaders: `assets/shaders/pbr/00-bindings-vertex.wgsl` (the lookups)
and `80-fragment-shared.wgsl` (the light loop). Related plans: [sun-cascades.md](sun-cascades.md)
(the near sun ladder; its increment 1 is this file's near sun view),
[character-animation.md](character-animation.md) sections 8 and 17 (the bodies), and
[graphics-settings.md](graphics-settings.md) (the control's words and cost line).

## 1. What cast what before rung 1 (measured 2026-11-10)

Read from the code first, then settled on the drawn picture, because FEATURES.md and the skinning
code disagreed:

- **The room lights cast no shadows at all.** `light.rs` and the clustered light loop in
  `frag_tail` add every lamp's light with only a range window, a spot cone and the floors between
  storeys (`slab_blocks_light`); nothing tested whether anything stood between a lamp and a surface.
  Aboard the ship and in the homestead almost all light is room light, so nothing there (bodies,
  furniture, walls, machines) shaded anything: a lamp behind a wall even lit the next room.
- **The near world never reached the sun's shadow map.** The sun map is built in the celestial pass
  from the celestial list only (terrain, near trees, wave crests, the planet build pieces); the scene
  list, which holds the homes, the furniture and every person's body, goes only to the colour pass
  (`render_scene_onto`). sun-cascades.md found this for the home in September; it holds for bodies
  too. FEATURES.md's "near bodies are skinned into ordinary meshes so the sun shadow and every pass
  draw them" was wrong: the skinned meshes were in the wrong list for the sun map. The player's body
  on a planet stands in the scene list, so it cast no sun shadow on the sand either.
- **Card hair** had a cutout band in `fs_shadow` (type 26) that only the sun map could have used, and
  the sun map never drew a body.

The red captures, from the release build of main (3d52baad9) in the rig, one boot, the camera parked
identically for each pair (`scripts/probe-hot-ab.js`, same park, a figure added and removed by
showcase between captures):

| Vantage | What it shows | Before (main) |
|---|---|---|
| shadow-hall-chair-night | the homestead's ground hall at night, a warm lamp over a chair, a woman standing 1.7 m from the lamp | no shadow from the chair, the woman, the shelves or the table: section 10's table has the numbers |
| shadow-racks-noon | the mushroom room at noon, sun through the glass roof (the window case aboard), a woman among the racks | no sun shadow from the woman or the racks |
| shadow-planet-third | on the Sahara at 16:30, the player's own body in third person | no shadow on the sand |
| shadow-fp-racks-noon | first person, looking down in the racks room at noon | no shadow of your own body |

## 2. The whole feature (the 2030 version)

What a game released near 2030 does for shadows in rooms, in public literature and shipped engines
(Unreal Engine 5's virtual shadow maps and MegaLights, Unity HDRP's shadow atlas and cached
shadows, id Tech and Frostbite's cached atlases, NVIDIA's RTXDI from Bitterli et al. 2020):

1. **Shadow maps for local lights in one shared atlas.** Every shadowed light gets tiles in a single
   depth texture: a point light six cube faces (a tetrahedral four is the alternative; the cube keeps
   the face pick a major-axis test and wastes no corners), a spot light one perspective view. Each
   light's face size follows how much of the screen its reach covers and how important it is.
2. **A per-frame budget of shadowed lights**, the rest unshadowed and said so. A crowded deck with
   hundreds of lamps shadows the few that matter where you look; a lamp across the hall still lights
   its floor, without a shadow.
3. **The near sun in the same atlas** for the homes, furniture and bodies, laddered to the far map
   (sun-cascades.md's camera-centred clipmap).
4. **Soft edges by the light's real size**: percentage-closer filtering first, then contact-hardening
   soft shadows (PCSS, Fernando 2005) with each lamp's physical size from `light_types.ron`, so a
   chair leg's shadow is sharp where it meets the floor and soft a metre away.
5. **Reuse of static casters' depth, versioned**: the walls and furniture do not move, so each face
   keeps their depth until something in its view changes (a version the home bumps on every edit),
   and only the moving casters (people) are drawn each frame on top. Obeys the one reuse switch.
6. **Screen-space contact shadows** for what no map resolves: a short march in the depth buffer
   toward the sun and the strongest lamp, for feet on a floor and a cup on a table.
7. **Ray-traced shadows where the hardware has ray queries** (RTX 20 series and later, RX 6000 and
   later), first for the sun and the shadowed lamps with the maps as the fallback, then stochastic
   direct lighting (ReSTIR DI, MegaLights) so every lamp shadows with a fixed cost, which needs a
   temporal denoiser and so the temporal history the engine does not have yet.
8. **Virtual shadow maps** (16k virtual texels a face, pages resident only where the depth buffer
   needs them, cached by invalidation), replacing the fixed atlas and the sun cascades on hardware
   without ray queries.
9. **Every caster**: static structure, furniture and the model packs, every tier of person (near
   compute-skinned meshes, middle vertex-shader bodies, the far tier's baked animation), card hair
   and foliage through their alpha cut, the first-person body with its head (the eye camera's copy
   folds it away, the shadow's must not), and glass and tent film as coloured, partial casters.

Chosen for rung 1: (1), (2), (3) at one tile, (4) as PCF, (9) except glass and the far tier. The
rest are rungs (section 9). Every rung is a step of this one architecture: the atlas is what the
cached rung caches, what the PCSS rung filters, what the ray-traced rung falls back to.

## 3. Budgets

The plan's budgets (estimates; rung 1's measurements on the RTX 4070 are in section 10, and the RX 580
column stays an estimate until someone measures one). The operator's card is an RTX 4070 (504 GB/s);
the project's floor is an RX 580 (256 GB/s, about a third of the 4070's raster rate).

| What | RTX 4070, Full | RX 580, Simplified |
|---|---|---|
| Shadowed lights | 8 | 4 |
| Face size | 512 for the top two, 256 for the next two, 128 for the rest | 256, 128 |
| Near sun tile | 2048 over 32 m (1.6 cm texels) | 1024 over 24 m (2.3 cm texels) |
| Atlas | 4096 square, 64 MB | 2048 square, 16 MB |
| GPU: the faces | under 1 ms at the homestead (depth only, a few hundred draws) | about 1 ms |
| GPU: the near sun tile | under 0.3 ms | under 0.5 ms |
| GPU: sampling in the scene pass | 16 comparisons a shadowed light a pixel: under 0.6 ms at 2560 x 1440 | 4 comparisons: about 0.5 ms at 1080p |
| CPU: choosing lights, culling, drawing | under 1 ms | the same (it scales with the processor) |

The CPU side is the one to watch: each face is a separate draw list in rung 1, so the casters in a
light's reach are drawn up to six times. Measured (section 10), rung 1 is over the CPU line on Full
in the busiest rooms (about 1.5 to 1.8 ms) even after the fast path halved it, and the scene pass
pays for the lookup's code even at Off. Rung 1b (section 9) is the fix for both; the cached rung
(rung 2) is what takes the walls and furniture out of the per-frame draws.

## 4. Infinite-of-x: a crowded deck with hundreds of lamps

- **Lamps**: choosing is linear in the lamps (a brightness and a reach test each, about 10 ns a lamp:
  2,048 lamps, the light buffer's ceiling, cost about 20 microseconds). What the GPU pays is
  proportional to the budget, never to the lamp count. The lamps past the budget are listed on the
  Settings page as lighting without shadows.
- **Casters**: first only those inside a shadowed lamp's reach or the near sun view are considered,
  then culled per face by its frustum, all by bounding sphere (`Mesh::bounds`, computed when a mesh
  is made; a skinned body a 2 m sphere about its middle), and a caster smaller than a texel at its
  distance is skipped. A face whose pyramid lies outside the camera's view is skipped whole (no
  visible surface can look it up). Only the casters a drawn view holds get an object uniform. A deck
  of 500 people: only those within a shadowed lamp's reach are drawn, and the middle tier's bodies
  through the vertex-shader skinning, as the colour pass draws them.
- **What the table costs**: one row a view and a quarter row a lamp, rewritten every frame.

## 5. Two modes, and Off

- **Full** (the default): eight lamps, soft 16-comparison edges, the near sun at 1.6 cm.
- **Simplified**: four lamps, a quarter of the memory, 4-comparison edges that read harder.
- **Off**: no atlas at all; the lamps light without shadows and the near world casts no sun shadow,
  as before 2026-11-10. Low impact (`HUMANITY_LOW_IMPACT`, the rigs beside the operator's game) holds
  it off.
- The levels are data (`data/gui/shadow_levels.json`); a level is offered only if its numbers are
  real (no upscaled level labelled as more).

## 6. Settings > Graphics > Light and sky

- **Room light shadows** (Off, Simplified, Full): each level's line says how many lamps, the largest
  face, the near sun's texel size, the comparisons and its video memory, all from the level's own
  numbers. Under it, the measured cost (the two atlas passes' GPU time, the CPU stages, the atlas
  memory, `graphics_settings.ron`), the card's memory against the level, and what it did this frame
  ("Now: 6 of 23 room lights cast shadows (4 more in view without them)").
- **Sun shadows** (existing) now also switches the near sun view, and its cost line counts it.

## 7. The hardware notice

The atlas needs depth textures and comparison sampling, which every graphics card wgpu runs on has
(core WebGPU), so no card is refused; the hint says so. The memory is the notice that applies: the
line under the control names the card's own memory and the level's share, and warns past an eighth
(64 MB on a 512 MB card). The ray-traced rung will need ray queries and will say so where it is
offered: what it needs, what the card has, and that the maps work instead.

## 8. No caches across runs; reuse only versioned and behind the switch

Rung 1 keeps nothing across frames as worked-out data: every face is drawn again every frame. The
one thing read from the frame before is which lamps were shadowed (a 1.3x preference so a lamp at
the edge of the budget does not flicker), and it obeys `humanity_core::reuse::enabled()`
(`HUMANITY_NO_REUSE=1`, the Dev page's "Turn off all reuse"): off, the ranking starts fresh every
frame. Rung 2's static depth reuse is versioned by the home's edit counter and the caster set, and
with the switch off draws every caster every frame.

## 9. The rungs

1. **The atlas** (built 2026-11-10, section 10): the budgeted lamps' cube faces and spot views, the
   near sun tile, every opaque caster, PCF, the Settings control.
1b. **The scene pass's share** (found 2026-11-11, section 10): (a) at Off, rebuild the surface,
   vegetation and terrain PSOs with `HAS_SHADOW_ATLAS` false (the pipelines can be rebuilt at run
   time, as `scene_format_ab.rs` does), so Off costs nothing, with a short hitch when the setting
   crosses Off; (b) on, move the lookups out of `frag_tail` into a SHADOW MASK: a depth pre-pass,
   then one compute pass writing each pixel's visibility for the shadowed lights (8 lights in two
   RGBA8 targets) that the light loop reads with one texture fetch, the deferred shadow mask most
   forward renderers use. (b) also makes the mask the place where contact shadows (rung 4) land.
   And (c), on the CPU: one draw per mesh per LIGHT instead of per face, with the face's four clip
   planes as clip distances (wgpu's `CLIP_DISTANCES`, where the card has it; per face elsewhere),
   since one draw for the whole atlas without them cost 10.9 ms of GPU.
2. **Cached static depth and a larger budget**: per face, the static casters' depth kept and copied in
   each frame, people drawn on top; versioned, behind the switch. Lets Full shadow 16 to 32 lamps and
   use 1024 faces. Then PCSS with each lamp type's size (`light_types.ron`, a new `size_m`).
3. **Line lights (strips) and area lights**: shadowed from sample points along the strip, then the
   PCSS penumbra of the strip's length. Until then strips light without shadows, as the Settings
   words say.
4. **Screen-space contact shadows** for the sun and the strongest lamp.
5. **The near sun ladder** (sun-cascades.md increment 2): more tiles out to 240 m, culling, blend
   bands; then glass and tent film as partial casters (its increment 4).
6. **Ray-traced shadows** where ray queries exist (wgpu's ray query feature), with the hardware
   notice; then stochastic many-light direct lighting once the engine has temporal history.
7. **Virtual shadow maps** on hardware without ray queries.

## 10. Built: rung 1 (2026-11-10, finished 2026-11-11)

- **`src/renderer/local_shadows.rs`**: the levels, the light choice (`select_lights`: brightness
  times the shader's own falloff at the eye and at three points ahead of it, 2, 4 and 8 m along the
  view, so the lamp over what you look at outranks a brighter one in the next room; lamps whose
  reach the view cannot see left out), each light's SHADOW REACH (`shadow_reach`: where its falloff
  is a twentieth of its value a metre away, 5.2 m for the homestead's 15 m panels, 3.1 m for a 4 m
  lamp; casters past it are not drawn and the shader fades the shadow out over its last fifth, since
  the light there is a twentieth of what it is close up), the views (`cube_face_view`, `spot_view`, `near_sun_view`, each face a little over 90 degrees
  so its filter never leaves it), the Morton packing (`pack_tiles`), the per-view culling, the table
  (`pack_table`) and the draw (`render_shadow_atlas`), called from lib.rs before the celestial pass
  with the scene lists and the celestial build pieces.
- **Group 0 bindings 8-10**: the atlas, a comparison sampler and the table, in `camera_bind_group`
  (the one constructor). The atlas pass binds a 1x1 stand-in there (the atlas is its depth target).
- **The shaders**: `shadow_atlas_view` (normal offset and a pull toward the light in world units,
  16 bilinear comparisons on a Vogel disk or 4 on a square, clamped inside the tile),
  `local_light_visibility` (the cube face by major axis), `near_sun_visibility` (with Settings'
  shadow strength, as the far map). Behind a new feature switch, `HAS_SHADOW_ATLAS`, live in the
  surface, vegetation and terrain PSOs, off in water, shell and cloud.
- **The casters**: every opaque scene object (structure, furniture, model packs, compute-skinned near
  bodies), depth only or through `fs_shadow` for the cutouts (card hair, cluster cards); a surface
  that gives off light (a lamp's bulb, a screen) never casts, or every lamp would sit in its own
  fixture's shadow. The skinned bodies the scene list does not hold go through two new skinned
  shadow PSOs (`vs_skinned` into the atlas, depth only and with `fs_shadow`): the middle tier, and
  the first-person body's unfolded twin (`skin_shadow_only`, pushed by engine/animation.rs before the
  head is folded), so your shadow keeps your head.
- **The fast caster path** (2026-11-11): plain opaque surfaces (`takes_fast_path`: the surface
  class, no cutout, not the swaying type 19) draw through their own small shader
  (`assets/shaders/shadow_atlas_instanced.wgsl`), sorted by mesh within each view, one instanced
  draw per run, each instance its caster's matrix with the view's index in the spare w, the views'
  matrices in one storage array: no per-caster binding at all. Everything else keeps the classic
  path. The classic views' cameras share ONE buffer (a slot each, one upload a frame), and a view
  with no classic caster and no skinned body binds nothing classic. Showcase `shadow_fast` "0" is
  the red twin; the picture is the same (below).
- **Costs**: `gpu.shadow_near`, `gpu.shadow_local`, `cpu.shadow_near`, `cpu.shadow_local`,
  `vram.shadow_atlas`, on the Performance page's Shadows row and under the Settings control. The
  log line (`[Shadows] cpu this frame`) splits `cpu.shadow_local` into culling, uploads, the near
  bodies' skinning (which moved into this stage: the atlas needs this frame's vertices, so it is not
  new work), recording and submitting.
- **Showcase pins**: `local_shadows` ("0", a level id, "auto"), `near_levels` ("0", "auto"),
  `fp_shadow_twin` ("0") and `shadow_fast` ("0"), the red twins' switches; released between
  vantages by the rigs.

### Measured after (2026-11-11, RTX 4070, release build, the rig at 2560 x 1387)

Shadow presence, from the rendered pixels (`scripts/shadow-metrics.mjs`: the region the atlas makes
darker than the same frame with it off, same park, same boot; "before" is main at 3d52baad9 measured
inside that region; the floor, two off captures of one park, is 0.000% of the frame in every pair).
Captures: `sh2-F-pics` and `sh2-before` in the session scratchpad.

| Vantage | Shadow region | Darker by | Light kept there | Before (main) |
|---|---|---|---|---|
| shadow-racks-night (one lamp over the racks: the picture gate) | 2.32% | 32.5 codes | 0.115 | 1.000 |
| shadow-racks-noon (sun through the glass roof) | 3.65% | 68.2 codes | 0.473 | 1.000 |
| shadow-fp-racks-noon (your own shadow, head and all) | 4.11% | 62.0 codes | 0.515 | 1.000 |
| shadow-planet-third (the Sahara at 16:30) | 1.75% | 21.2 codes | 0.479 | 0.998 |
| shadow-hall-chair-night (about 40 lamps overlap: faint by physics) | 0.92% | 28.9 codes | 0.709 | 1.000 |

The racks gate of sun-cascades.md increment 1 (`scripts/home-shadow-metrics.mjs --gate`, atlas off
against on): the lowest shelf top at noon fell from 159.2 to 54.9 codes (linear 0.376 to 0.047)
while the open floor moved 0.02 codes. Its strict cap (the night capture plus 10%) still fails: with
the room's bounced light switched off (showcase `room_gi` "0") the shelf reads 0.0039 against the
night's 0.0009, so what remains is almost all the room GI, which no shadow map occludes (room GI's
own visibility test, rung 1 of that system, is skipped). Its planet hut check is stale: the vantage
now looks at the hut's inside wall, with no sand in frame; the floor under the roof itself went from
0.60 to 0.075 linear with the atlas on.

The fast path does not change the picture: against the classic path, the share of pixels more than 4
codes apart is the same as two captures of the fast path (the hall 1.18% against 1.17%, the racks at
night 0.41% against 0.42%, the crowd 2.90% against 2.93%: what moves in the scene).

CPU, `cpu.shadow_local` (ms; `cpu.shadow_near` adds 0.10 to 0.15):

| Vantage | WIP (classic, per-view camera writes) | Rung 1 Full | Rung 1 Simplified |
|---|---|---|---|
| shadow-hall-chair-night | 3.38 | 1.7 to 2.0 | 1.25 |
| shadow-racks-night | (not measured) | 1.6 to 2.2 | 1.2 to 2.3 |
| anim-crowd-near | 2.83 | 1.4 to 2.2 | 1.2 to 1.7 |
| home-overview-noon | 2.60 | 1.5 | 1.0 |
| mess-hall-counter | 1.80 | 1.0 | 0.7 |

GPU (ms): the atlas passes and the scene pass against main, and against this build with the
lookups compiled out (a hot-reload A/B: what the code costs when it never runs).

| Vantage | main: scene | lookups compiled out: scene | Off: scene | Full: atlas passes (near + rooms) | Full: scene | Simplified: scene | Full over main, all told |
|---|---|---|---|---|---|---|---|
| shadow-hall-chair-night | 11.54 | 11.52 | 12.51 | 0.10 + 0.28 | 13.36 | 13.01 | +2.2 |
| shadow-racks-night | 8.43 | (not run) | 9.10 | 0.19 + 0.21 | 9.74 | 9.59 | +1.7 |
| anim-crowd-near | 7.52 | 7.52 | 8.09 | 0.36 + 0.20 | 8.26 | 8.25 | +1.3 |
| home-overview-noon | 5.10 | 5.20 | 5.44 | 0.07 + 0.57 | 5.63 | 5.57 | +1.2 |
| mess-hall-counter | 8.15 | 8.16 | 8.82 | 0.05 + 0.12 | 9.14 | 9.05 | +1.2 |

Memory: 65.1 MB on Full (the 64 MB atlas, the fast path's 1 MB of instances, the table and the views), 17.1 MB on Simplified, 1.1 MB at Off (the fast path's buffers and the table, made at start so nothing is allocated mid-game). Main is the release build of 3d52baad9 (the rung 2b door and seat work since does not touch the renderer).

The CPU figures vary by about half a millisecond between runs of the same build (two runs of two captures each are given as a range). Where the time goes in the hall on Full, from the log line: culling the faces 0.13 to 0.18 ms, the uploads 0.42 to 0.47, the near bodies' skinning 0.13 (moved here, not new), recording the draws 0.76 to 1.09 (about a microsecond a draw: 1,040 draws for 4,660 caster instances), submitting 0.13. The ground hall's 147 lamps and dense structure are the worst case measured; the mess hall is at 1.0 ms.

**What the GPU numbers say.** The passes themselves are cheap. The scene pass is not: compiling the
room light's lookup into `frag_tail` costs the scene pass 0.2 to 1.0 ms (the hall 12.51 against 11.52 with the lookups compiled out, the crowd 8.09 against 7.52) even at Off, because a
shader's register count is set by its fullest path, taken or not; switched on, the sampling adds
0.2 to 0.85 ms more. Lighting the shadowed lights in a loop of their own was tried (2026-11-11) and
cost the same at Off and 1 ms more on; so was reading the view matrix column by column (kept: about
0.15 ms). The fix is a rung of its own (section 9, rung 1b).

## 11. Questions only the operator can answer

1. **How many lamps should Full shadow?** Rung 1 shadows the 8 that matter most where you look
   (Simplified 4). The homestead's halls are lit by dozens of overlapping panels (40 lamps reach the
   ground hall's view), so a person under them casts many faint shadows rather than one dark one,
   which is what a real room lit that way looks like. Recommendation: keep 8 until rung 2's cached
   walls and furniture make 16 to 32 affordable, then raise Full and keep Simplified at 4.
2. **Strips (line lights) light without shadows until rung 3.** A shadow from a strip's midpoint
   would be wrong for a long strip and thrown away later. Recommendation: wait for rung 3's sampled
   strips rather than ship a midpoint stopgap.
3. **The default.** Full on every machine (the "max-graphics default" the other lighting controls
   use), Off in low impact. Recommendation: keep Full as the default; the Settings line warns where
   the 64 MB passes an eighth of a card's own memory.
4. **Off is not free yet, and the Settings cost line does not show it** (measured 2026-11-11,
   section 10): the scene pass pays for the compiled-in lookup at Off too, and the line counts only
   the atlas passes. Recommendation: build rung 1b (a), rebuilding the scene PSOs without the atlas
   when the setting crosses Off (a hitch of the PSO compile at that moment), next, so Off is truly
   free and the line becomes true; then (b), the shadow mask, which takes the cost out when on.
