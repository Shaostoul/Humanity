# Every species gets a baked card

Status: IMPLEMENTATION BRIEF (2026-07-31), written from a code read plus the
probe captures at `.probe-rig/sweeps/20260731-165006` and `20260731-165225`.
ONE increment, ONE agent, ONE commit. The fence below is the point of the
document: this is the rung that was already reserved in `docs/PRIORITIES.md`
lines 191-195 ("varied tree species ... needs matching far-field cards + atlas
entries or the card handoff pops"), and everything adjacent to it is listed at
the bottom as explicitly out of scope with the reason.

This is a companion to `docs/design/billboard-bake-generalization.md`, which
covers a DIFFERENT hole (giving `decorations.ron` glTF models a card rung).
That doc never mentions procedural species; grep it for "procedural" and you
get nothing. Do not merge the two.

## This is a shipped-build correctness fix, not dev-checkout polish

Read this section before deciding the priority. The release bundle does not
contain `assets/models/`: `.github/workflows/build-desktop.yml` lines 91-93 copy
`data/`, `assets/icons/` and `assets/shaders/` and nothing else. So on a build a
real person downloads:

- The two photoscan species (`fir`, `pine`) have no model to draw near AND no
  model to bake, so `parse_gltf_mesh_with_texture` fails for all six stems at
  `src/lib.rs:9469-9477`. Every `parts` vector comes back empty.
- `bake_tree_atlas` (`src/renderer/billboard_bake.rs:101`) calls
  `bake_billboard_texture` on the first empty list, which returns
  `Err("no parts to bake")` at line 147, and the `?` ABORTS the whole bake at
  the first stem. `tree_atlas_ready` stays false for the session.
- `src/lib.rs:8636` therefore never sets bit 2 of the type-12 `params.w`, so the
  sprite branch takes its else at `assets/shaders/pbr/90-fragment-main.wgsl:824`
  and paints `vec3(0.10, 0.16, 0.07)`: a flat dark green SQUARE, `h` by `h`,
  two crossed pairs of them per tree.
- The other six species are procedural, so they never reach the sprite path at
  all: `src/terrain/planet_chunks.rs:1925` gates it on
  `albedo.is_some() && !sp_proc`, and they fall to lines 1967-1970, which emit a
  0.5 m wide trunk rectangle plus a canopy rectangle 1.1h wide by 0.75h tall in
  one flat colour.

Net: **in a shipped build there is no correct vegetation card anywhere on the
planet.** Procedural species are the only vegetation a downloaded build has, and
their far field is coloured cardboard. The dev checkout is strictly better off
than the shipping product here, which is why this reads as a polish item and is
not one.

At Fuji the species weights (`data/vegetation/trees.ron`) put sakura at 3.0, oak
2.0, acacia 1.6, palm 1.5, birch 1.4 and momiji 1.2 against fir 1.0 and pine
1.0, so roughly 85% of the visible mid field is the coloured-rectangle path even
in the dev checkout. Photographed at v0.1078:
`.probe-rig/sweeps/20260731-165225/ground-storm-inslab.png`, crop x1900 y420
w620 h220 at 4x, a row of pale grey vertical slabs with hard straight tops and
hard vertical sides repeating across the frame.

## What PRIORITIES already decided, and why this does not contradict it

`docs/PRIORITIES.md:187-190` says the FAR-TREE CANOPY SHEET was pulled to
default OFF after the operator's "black squares in a grid" verdict, and "do not
iterate the card sheet further."

That verdict is about a different feature. `far_tree_sheet` (`src/config.rs:512`,
`src/terrain/far_trees.rs`, default false at `src/gui/mod.rs:6796`) is a single
coarse silhouette SHEET mesh rebuilt around the camera for the multi-kilometre
range. The per-tree cards this brief is about are baked into the patch meshes by
`emit_card` / `emit_sprite_card` and are the 70 m to 1500 m rung that is DEFAULT
ON and is what a player actually sees. Different code, different distance band,
different setting. `docs/PRIORITIES.md:191-195` is the line that governs this
work, and it schedules exactly this increment.

## PRECONDITION: land this first, in its own commit

Two items, one commit. Every acceptance number below is worthless until BOTH
land: the rig has to settle before it measures, and the counter it measures
with has to be able to represent the answer.

### P1. `settle_s` 25 -> 75 at `fuji-forest-ground`

`tests/visual/vantages.json`, `fuji-forest-ground`: `settle_s` 25 -> 75.
Measured 2026-07-31 on the operator's max-quality rig config (terrain_split_px
2, terrain_patch_budget 12288): at 25 s the sweep captures 43-48 ms while the
scene is still streaming, and the frame keeps getting heavier for about another
40 s before it settles. The sweep was reporting 21.3 and 18.8 fps on a scene
that had not finished building.

Note that `perf_floor_fps` never gates anything: `scripts/probe-sweep.js:224`
records it into the manifest and `rec.ok` is set true regardless. The floor is
advisory. A real steady-state gate would wait for `[PatchBatch]` to stop
changing in `run.log` instead of sleeping a fixed `settle_s`
(`scripts/probe-sweep.js:236` is a plain sleep), which is a rig change and is
NOT part of this increment.

DONE in this brief's commit, along with the acceptance text.

### P2. Unclamp the perf counters

Added v0.1081, and it invalidates every frame-time number in this document.

`src/lib.rs:2882` computes the frame delta as
`let dt = (now - state.last_frame).as_secs_f32().min(0.1);`. The clamp is
correct for SIMULATION: it stops a stall from teleporting the player. But that
same clamped `dt` is what stamps both perf counters,
`state.gui_state.fps = 1.0 / dt` (`src/lib.rs:11551`) and
`frame_times.push(dt * 1000.0)` (`src/lib.rs:11555`), which
`src/engine/ipc.rs:549-550` writes into `screenshot_done.json` and
`scripts/probe-sweep.js:269-270` records into the sweep manifest.

So fps saturates at exactly 10.0 and frame_ms at exactly 100.0. Nothing slower
is representable, anywhere in the rig, by anything.

Fix, about five lines, no image change:

- keep the clamp for the simulation `dt`, adding
  `let raw_dt = (now - state.last_frame).as_secs_f32();` before it;
- stamp `state.gui_state.fps` and the `frame_times` ring from `raw_dt`;
- report the MEDIAN of the 120-sample ring alongside the mean.
  `src/engine/ipc.rs:542-551` writes only `frame_ms_avg` today; add
  `frame_ms_med` there and record it beside `frame_ms` in
  `scripts/probe-sweep.js`. The median is not decoration: one 2 s streaming
  stall shifts a 120-frame mean by about 16 ms, which is a whole vsync step of
  the A/B this increment is judged on.

CONSEQUENCE, and this is why P2 is a precondition rather than a nicety. Every
vegetation frame time in this brief was read through the saturating counter and
is therefore a CENSORED sample, biased toward 100 ms:

- the "before" figure quoted below as 93.7 ms +/- 5.8 / 10.7 fps at
  `fuji-forest-ground` is a mean over a ring whose slow samples were clamped to
  100.0;
- the `ground-storm-inslab` A/B reference quoted as 50.7 ms +/- 1.8 is the same
  kind of number off the same counter, and that vantage returned a flat
  "10 fps / 100 ms" in two independent sweeps on 2026-07-31;
- timed OFF the log instead, between consecutive 60-frame `[ChunkDiag]` lines
  (`src/lib.rs:8909`), the forest runs about 131 ms, 7.6 fps, where the rig
  reported 99.1 ms;
- `fuji-forest-ground` carries `perf_floor_fps: 9` in
  `tests/visual/vantages.json`. A 9 fps floor CANNOT FAIL against a counter
  whose floor is 10 fps. That gate has never been able to fire.

Treat every frame-time number in this document as "at most this bad" until P2
lands. Do not A/B against them; re-measure first.

## Scope, in the order to do it

Files: `src/renderer/billboard_bake.rs`, `src/lib.rs` (bake site, around 9461),
`src/terrain/planet_chunks.rs` (emitters, 1668-1975), `data/vegetation/trees.ron`,
`src/renderer/mod.rs:938-939` (atlas texture size), and the tile decode at
`assets/shaders/pbr/90-fragment-main.wgsl:808-827`.

### 1. Registry-driven tile allocation

Today `ATLAS_COLS = 3`, `ATLAS_ROWS = 2`, `ATLAS_TILE_PX = 512`
(`billboard_bake.rs:77-79`) and `sprite_tile` is HAND-WRITTEN in
`data/vegetation/trees.ron` (0 for fir, 3 for pine, and a meaningless 0 on all
six procedural rows). Eight species times three variants is 24 tiles.

Make the tile index a PURE FUNCTION OF THE REGISTRY rather than something an
allocator hands out at bake time. This matters more than it sounds: the card
emitter runs on background patch-build threads that may already be in flight
when the bake happens, so any tile assignment produced BY the bake would be a
cross-thread ordering hazard. A pure function has no such problem, and it also
guarantees the CPU emitter and the GPU bake agree by construction.

```
base_tile(species_i) = sum of trees[0..species_i].variants.max(1)
tile(species_i, variant) = base_tile(species_i) + variant
```

Put it next to `registry()` in `tree_mesh.rs` as a `OnceLock<Vec<u32>>`, with a
unit test that the total fits the atlas.

DELETE the `sprite_tile` field from `TreeDef` (`tree_mesh.rs:40`) and from all
eight rows of `trees.ron`. Per the no-backwards-compatibility directive there is
no deprecation shim; a hand-written tile index that disagrees with the computed
one is a silent wrong-species card, so the field must not survive.

ATLAS SHAPE: `ATLAS_COLS = 6`, `ATLAS_ROWS = 8`, `ATLAS_TILE_PX = 256`, giving
1536 x 2048 (about 12.6 MB) and 48 slots for the 24 in use. Take the headroom,
do not size the grid to exactly 24: `renderer/mod.rs:938-939` derives the
texture from these constants and the shader decode hardcodes them, so a grid
sized to today's registry means the NEXT row somebody adds to `trees.ron`
silently overflows the atlas and corrupts rendering. A data file that can break
the renderer is the infinite-of-X rule inverted. Add a test that
`sum(variants) <= ATLAS_COLS * ATLAS_ROWS` and a `log::error!` if it ever trips
at runtime.

Start at 256 px. Only go to 512 if the silhouettes read thin at the rig; note
that 512 at 6x8 is 3072 x 4096 (about 50 MB), which is the point where the right
answer becomes a texture ARRAY rather than a bigger 2D grid.

SHADER DECODE, `90-fragment-main.wgsl:811-817`, three constants:
`clamp(u32(floor(a_enc)) - 1u, 0u, 5u)` -> `0u, 47u`; `% 3u` and `/ 3u` -> `% 6u`
and `/ 6u`; `/ 3.0` and `/ 2.0` -> `/ 6.0` and `/ 8.0`. Do NOT plumb these as a
uniform. Keep them compile-time in both places and add a lockstep test that
scans `shader_loader::assembled_pbr_source()` for the literals and fails when
they disagree with `billboard_bake::ATLAS_COLS/ROWS`. That idiom is already used
three times in this repo (`renderer/atmosphere.rs:586`, `renderer/clouds.rs:799`,
`renderer/water.rs:560`), it costs no uniform slot, and it cannot silently drift.

Also check the encoding headroom while you are here: the card carries
`|uv.x| = (1 + tile) + u01 * 0.5` (`planet_chunks.rs:1720`), so tile 47 gives 48.5.
Fine, and fine to several hundred, but the `+ u01 * 0.5` packing means the tile
index must stay integral, so keep the `floor`/`fract` pair exactly as it is.

### 2. Packed-colour decode in the baker

Without this, every procedural species bakes as the same olive blob. Procedural
meshes carry colour in the PACKED UV, not in a texture
(`plant_mesh.rs:202`, `pack_color_to_uv` at `terrain/planet_surface.rs:508`), so
`BAKE_WGSL` (`billboard_bake.rs:37`) samples the 1 x 1 grey-green fallback
`[90, 110, 70]` at line 299 and returns the same colour for every fragment of
every species.

Add a mode flag to `BakePart` and branch in `fs_main` on the same three lines
the megashader uses at `90-fragment-main.wgsl:1146-1151`:

```wgsl
let packed = u32(round(max(uv.x, 0.0)));
let c = vec3<f32>(
    f32((packed >> 8u) & 255u) / 255.0,
    f32(packed & 255u) / 255.0,
    clamp(uv.y, 0.0, 1.0),
);
```

Three details that will each cost you a bake if you miss them:

- **No alpha test in packed mode.** The textured path discards below `c.a < 0.3`.
  A packed part has no alpha channel and always covers, so skip the discard
  entirely rather than testing a synthesised alpha.
- **The flag is per PART, not per sprite.** A procedural tree is ONE part; a
  photoscan is a stem plus a `_bark` pair. Today all parts of one bake share the
  single `ubuf` created at `billboard_bake.rs:285`. Give each part its own small
  uniform (mvp plus a mode word) rather than trying to infer the mode from
  whether `p.texture` is `Some`, because "untextured" and "packed colour" are
  genuinely different states (a future untextured-but-not-packed part would bake
  wrong).
- **Do not gamma-correct.** The bake target is `self.config.format`, the
  swapchain format, which is sRGB; the packed decode yields LINEAR albedo; the
  card samples the atlas with `textureSampleLevel` which decodes sRGB back to
  linear. Output the decoded linear value directly and the round trip is exact.
  Applying any encode by hand washes every procedural card out, and it will look
  plausible enough in isolation that you will not catch it without an A/B against
  the near 3D mesh of the same tree.

### 3. Bake from the registry, and survive a missing model

Replace the hardcoded `for base in ["fir_sapling", "pine_sapling_small"]` at
`src/lib.rs:9465` with a walk of `tree_mesh::registry().trees`, and for each
variant `0..variants.max(1)`:

- **Procedural** (`model` empty): build the mesh with `PlantMeshBuilder` and
  `tree_mesh::build_tree(&mut b, t, t.height_m, v.wrapping_mul(2_654_435_761))`.
  That seed and that height are not a choice: they are copied verbatim from the
  near-model builder at `src/lib.rs:9354-9364`, and if they diverge the card is a
  DIFFERENT TREE from the model it hands off to, which is exactly the pop this
  rung exists to remove. Bake as one packed-colour `BakePart`.
- **Model-backed**: parse the glTF as today, two parts (`""` and `"_bark"`),
  textured mode.
- **Missing or unparseable**: log once, LEAVE THE TILE ZERO-FILLED, and
  CONTINUE. `bake_tree_atlas` must stop returning `Err` when a part list is
  empty; it should skip that slot and set `tree_atlas_ready = true` if ANY tile
  baked. This single change is what makes a downloaded build render correctly:
  fir and pine keep their empty (alpha 0, discarded) tiles, and the six
  procedural species that DO ship get real silhouettes.

`bake_tree_atlas`'s `.take((ATLAS_COLS * ATLAS_ROWS) as usize)` and its implicit
"index i is the tile" contract both go away: the caller now passes the tile index
alongside the parts.

BAKE COST, and the claim this paragraph used to make was wrong. "24 at 256 px is
well under a second" assumed the cost scales with tile RESOLUTION. It does not.
`run.log` records `[Bake] tree-card atlas ready (6 stems, 0.8s)` (the log line
is `src/lib.rs:9509`), which is about 133 ms PER TILE, and that time is
dominated by the per-tile render-pipeline compile, not by rasterising 512x512
pixels. `bake_billboard_texture` builds, FOR EVERY TILE: a shader module
(`src/renderer/billboard_bake.rs:179`), a bind-group layout (`:185`), a pipeline
layout (`:218`), a render pipeline (`:225`, the expensive one), a colour
texture, a depth texture, a uniform buffer and a sampler; then
`bake_tree_atlas` adds its own encoder plus `queue.submit` per tile
(`:104-132`). Dropping to 256 px shrinks none of it.

6 -> 24 tiles is therefore about 3 s of MAIN-THREAD FREEZE on world entry (the
bake runs inline in the frame loop at `src/lib.rs:9471`) - a quality regression
this increment would introduce by itself.

Item 2 is already rewriting this function to give each PART its own uniform, so
do the hoist in the same pass: build the shader module, bind-group layout,
pipeline layout, render pipeline and sampler ONCE outside the loop, and keep
per-tile only the MVP/mode uniform and the draw. One compile, not 24. Cheap and
optional while you are in there: render each tile straight into a viewport plus
scissor of the atlas texture instead of into a scratch texture followed by
`copy_texture_to_texture`, which also gives item 3's "leave a failed tile
zero-filled" behaviour for free out of a single `LoadOp::Clear(TRANSPARENT)`.

Measure it and put the number in the log line, as that line already does.
Target: `[Bake] tree-card atlas ready (24 stems, X s)` with X under 1.0, against
the roughly 3 s a naive 24-tile version of today's code would cost.

### 3b. The card FOOTPRINT, which is the piece that changes the image

Do not skip this one. Items 1-4 replace a rectangle with a silhouette; without
3b they ALSO introduce an artefact worse than the one they remove, in exactly
the place this rung exists to fix.

`bake_billboard_texture` frames a SQUARE sized on the larger of width and
height, `let half = 0.5 * w_m.max(h_m) * 1.05;`, centred on the joint AABB
(`src/renderer/billboard_bake.rs:167-171`), and it RETURNS that footprint.
`bake_tree_atlas` throws the return away as `_fp`
(`src/renderer/billboard_bake.rs:103`). `emit_sprite_card` then hardcodes
`let w = h; // square sprite frame` (`src/terrain/planet_chunks.rs:1717`) and
maps `v01` 0..1 onto world 0..h measured from the tree's base.

The two agree only when the tree is taller than it is wide. Fir at 22 m by about
6 m is height-dominant, so the error is just the 5% margin - which is precisely
why this has never been seen: the sprite path today only ever runs for fir and
pine.

It stops being invisible the moment the six procedural species get tiles.
`src/renderer/tree_mesh.rs:545` states the case in its own comment: acacia's
"crown is WIDER than the tree is tall". Work a 1.3:1 crown through the same
arithmetic - `half = 0.6825h`, AABB centre `0.5h`, so the frame spans `-0.1825h`
to `1.1825h`, a span of `1.365h` - and map that onto a card of side `h`:

- the drawn tree is `h / 1.365 = 0.733h`, 27% TOO SHORT;
- its trunk base sits 13.4% of the card height ABOVE the ground;
- the crown is squeezed to 74% of its true width.

Palm and sakura land between that and the conifer case.

The eye is unusually good at this one because it is a MOTION cue, not a static
one. Walking through the 120 m model handoff makes every wide-crowned tree jump
in height and hop off the ground at the same instant, which reads as the world
twitching rather than as a distant tree being slightly wrong. Shipping 1-4
without 3b trades one handoff artefact for a worse one.

FIX, and it is small:

- Widen `bake_billboard_texture`'s return from the square footprint to
  `(frame_m, h_nominal_m, base_offset)` - all three are values it ALREADY
  computes, it just throws two of them away: `frame_m = 2 * half`,
  `h_nominal_m = h_m` (its line 166), and
  `base_offset = (aabb_min_y - (cy - half)) / (2 * half)`, the dimensionless
  fraction of the frame between its bottom edge and the tree's base. Stop
  discarding it as `_fp` at `billboard_bake.rs:103` and store the triple per
  tile in the SAME `OnceLock` table item 1 already builds.
- `emit_sprite_card` takes that triple and emits a square of side
  `side = frame_m * (h / h_nominal_m)`, with its bottom dropped to
  `base - up * (base_offset * side)`. That replaces both `let w = h;` and the
  `up * (h * v01)` term with `side`.
- For a procedural species the scale factor is exactly the `jitter` already
  computed at `src/terrain/planet_chunks.rs:1918` (`h = sp_h * jitter`, and the
  bake builds at `t.height_m`), but write it as `h / h_nominal_m` anyway: a
  model-backed species' baked AABB height is the glTF's, not the registry's, and
  only the ratio form is right for both.

Conifers keep their present look to within about 2%. Wide crowns land at the
right height, with their trunks on the ground.

No shader change, no extra vertices, three extra f32 per tile in a table that
already has to exist. One consequence worth knowing rather than acting on:
`v01` now spans the card FRAME rather than the tree, so item 6's crown-AO ramp
is measured on the frame. For the height-dominant case that is the same 2%
difference; for a wide crown it is what you want, because the frame is what the
baked pixels occupy.

### 4. Drop the procedural exclusion

`src/terrain/planet_chunks.rs:1925`: `if albedo.is_some() && !sp_proc` becomes
`if albedo.is_some()`. Delete the stale comment above it (lines 1919-1924, the
one that promises real impostors as a later increment; this IS that increment).

The `else` branch SURVIVES, unchanged, for noise planets (`albedo.is_none()`),
which have no imagery and no bake. Keep `emit_card` itself: grass tufts
(`planet_chunks.rs:1895-1896`) are its other caller and are not in scope here.

### 5. Index the quads while you are already inside those functions

Both emitters push a fresh vertex per corner per triangle:
`indices.push(vertices.len()); vertices.push(...)` at `planet_chunks.rs:1689-1697`
and `1735-1743`. A card is 4 triangles (two quads, the second pair with reversed
winding for two-sidedness) built from only 4 distinct corners, so each card
stores 12 vertices where 4 would do: 384 bytes of vertices plus 48 of indices,
against 128 plus 48 indexed.

This is not a separate perf project. It is a few hours inside a function you are
rewriting anyway, and it is what makes the acceptance gate (b) reachable.

Measured at `fuji-forest-ground` steady state: 470,935 of 39,321,600 vertex arena
elements free (98.8% used) but 25,073,096 of 71,303,168 index elements free.
That ratio, 1.19 indices per vertex against a bare patch's 2.394, resolves to
about 5.29M grid vertices and 33.56M CARD vertices, so cards are 86% of the 1.2
GB vertex arena, about 1.07 GB. Indexed, that is 11.19M and the arena drops to
about 42% used. The arena stops overflowing, and overflow is where the frame
time actually goes: every measured config with zero `[PatchBatch]`
classic-fallback ran 29-64 ms and every config churning against a full arena ran
93-98 ms. The control is `ground-storm-inslab`: 12,547 patches plus clouds plus
rain, 0 classic-fallback, 63.6 ms, against fuji's 93.7 ms with 2,343
classic-fallback.

Two correctness notes:

- The image must be BIT-IDENTICAL from this half of the change. Same triangles,
  same winding, same order, same per-corner attributes.
- Sharing corners across triangles is safe for `pack`, which is a FLAT varying
  taking the provoking vertex (`00-bindings-vertex.wgsl:242`), because all four
  corners of a coloured card carry the same packed colour. Sprite cards do not
  read `pack` at all: their tile encoding rides the SMOOTH `uv`
  (`00-bindings-vertex.wgsl:230`, decoded at `90-fragment-main.wgsl:801`), which
  is exactly why the sprite corners can each carry a different `u01`. Do not
  "fix" this by making them agree.

`ATLAS_COLS` also drops the far field's triangle count: a procedural species goes
from 4 coloured cards (48 vertices unindexed) to 2 sprite cards (24 unindexed, 8
indexed).

LIFESPAN, stated plainly so nobody re-opens the question later. Item 5 is the
one piece of this increment that the reserved instancing / impostor arc will
DELETE: once cards are instanced quads there are no per-card vertices in the
patch mesh to index. Do it now anyway, for two reasons. It is inside a function
you are rewriting regardless, so its marginal cost is near zero. And
`src/renderer/patch_arena.rs:194-201` sizes the arena as
`(1200 MB).min(device.limits().max_buffer_size)`: on any adapter reporting
wgpu's DEFAULT `max_buffer_size` of 256 MiB the vertex pool is 8.4M vertices
against the ~36M this scene demands, so every tree-bearing patch falls to the
classic per-draw path permanently. Indexed, the demand fits. On the cheap and
old hardware this project explicitly targets, item 5 is the difference between a
forest that batches and one that never does.

### 6. Card shading: blend the normal, ramp the crown

Applied to the current coloured rectangles this only produces gradient-shaded
rectangles, so it goes last, after 1-4 have made the cards real silhouettes.

Both emitters force the shading normal to the radial up: `let nrm = up;` at
`planet_chunks.rs:1686` and `let nrm = up.to_array();` at `:1727`. The comment at
`1682-1685` explains why, and you must read it before touching this: in v0.896 the
card plane normal was horizontal, an overhead sun gave `N.L` near 0, and every
tree rendered as a BLACK SLAB at noon. The fix removed the black slab and all
directional shading with it, which is why a distant forest currently reads
exactly as bright as the grass beside it and has no sunlit-crown / shaded-flank
split.

The standard billboard-foliage answer is to blend the shading normal partway
toward the card's own facing, roughly `normalize(mix(up, card_facing, 0.5))`,
which keeps `N . up` near 0.707 and so never reaches `N.L = 0`. Do it like this:

- `emit_sprite_card` emits `up.cross(side)`, the quad's plane normal, as the
  vertex normal instead of `up`. `emit_card` KEEPS `up` (grass tufts and
  noise-planet cards are not in scope).
- The sprite branch reconstructs the radial up from the planet centre, which it
  already has in `material.base_color.xyz` (used the same way at
  `90-fragment-main.wgsl:780`), flips the facing to the viewer side, and blends:

```wgsl
let up_r = normalize(in.world_position - material.base_color.xyz);
let f = normalize(in.world_normal);
let fs = f * sign(dot(f, normalize(camera.view_pos.xyz - in.world_position)));
normal = normalize(mix(up_r, fs, 0.5));
```

  The `sign` is load-bearing. Cards are two-sided (`cull_mode: None`,
  `renderer/pipeline.rs:705`), so without it half of every crossed pair lights
  from behind and you have reinvented the v0.896 black slab on one diagonal. Do
  NOT flip the whole blended normal: that would put `N . up = -0.707` and light
  the card from below.

- Crown AO from `v01`, which the branch already has at
  `90-fragment-main.wgsl:813`: one line, `albedo *= mix(0.55, 1.0,
  smoothstep(0.0, 0.75, v01))`, so the bottom of a card is darker than its top.

- **Dissolve the 1500 m cutoff, in the SAME insertion.** The sprite branch
  hard-discards on `card_dist > shadow_u.params2.x` at
  `assets/shaders/pbr/90-fragment-main.wgsl:805` (the coloured branch does the
  same at `:842`) with no fade at all, so the forest ends on a smooth arc drawn
  across the ground that follows the camera. Measured across one continuous
  ridge from an 880 m eye at v0.1081: inside the ring, mean luma 29.9 with local
  (8 px) SD 8.13; just outside it, 45.5 with local SD 1.18. A 34% luminance step
  and a 6.9x texture step, at a radius centred on the player. Nothing in nature
  draws a circle of forest around the observer.

  The engine already contains the exact idiom fifteen lines below: the v0.999
  grass 4x4 Bayer dissolve at `:853-866`, shipped for this same defect one LOD
  rung in (operator: "a line of light perpendicular to me like 10 meters away").
  Reuse it verbatim - `let fade = smoothstep(far - 250.0, far, card_dist);` with
  `far = shadow_u.params2.x`, then the same ordered-Bayer threshold built from
  `in.clip_position`, and `discard` when `fade >= thresh`. About 10 lines.

  It is nearly free, and it REDUCES fill, because a dissolved fragment discards
  before the BRDF. It needs no new geometry because the terrain imagery UNDER
  the cards is already the right colour: that measured 45.5 / rgb(40,49,31) just
  outside the ring IS NASA albedo of the same forest, so the cards only have to
  fade into something that already matches.

  This is NOT the `far_tree_sheet` the operator rejected
  (`src/terrain/far_trees.rs`, default off). It changes nothing beyond 1500 m
  and adds no mesh.

The shader half of item 6 is ONE anchored insertion into the sprite branch at
`assets/shaders/pbr/90-fragment-main.wgsl:801-827`, carrying all three pieces
(blended normal, crown AO, cutoff dissolve) together. Do it against a quoted
anchor, not as a free-hand pass over the file, and do NOT make two trips:
`90-fragment-main.wgsl` is the shared shader tail CLAUDE.md flags as a
three-way-merge hazard, and every extra visit is another chance to corrupt
another domain's concurrent edit.

## Acceptance, exactly these

(a) NO flat rectangular slabs anywhere in the tree field at `fuji-forest-ground`
and at `ground-storm-inslab`. Every card past the 3D-model ring shows a crown
outline with sky visible through its edge.

(b) `[PatchArena]` "vertex arena full" warnings GONE from `run.log`, and
`[PatchBatch]` classic-fallback at 0, at the operator's max-quality rig config
(terrain_split_px 2, terrain_patch_budget 12288).

(c) RE-MEASURE the before AND the after with honest counters (precondition P2),
at `ground-storm-inslab`, and record BOTH in the commit message: the mean and
the MEDIAN of the 120-frame ring, plus the capture width.

The old form of this gate - "the before number is 93.7 ms +/- 5.8; expect
roughly 60-65 ms after" - is WITHDRAWN. Both halves were read off the
saturating counter described in P2, so neither is a measurement. The same
applies to the `ground-storm-inslab` reference of 50.7 ms +/- 1.8 quoted below:
that vantage came back as a flat "10 fps / 100 ms" in two independent sweeps on
2026-07-31, and off-log timing between consecutive 60-frame `[ChunkDiag]` lines
puts the forest at about 131 ms / 7.6 fps where the rig reported 99.1. Take the
before number again, on a quiet machine, after P2. Then state it.

MEASURE FRAME TIME AT `ground-storm-inslab`, NOT at `fuji-forest-ground`
(added v0.1081). Four runs of a byte-identical `fuji-forest-ground` config
returned 39.7 and 85.3 ms and failed to capture in 2 of 4, because the walking
player settled on different ground each time (alt 1240-1424 m); it now carries
`hold_altitude`, but its reproducible-enough twin is `ground-storm-inslab`,
recorded at 50.7 ms +/- 1.8 (n=3) through the censored counter, and that is the
vantage an A/B is judged at once the number itself is honest. Also
note every rig frame time is quantized to the refresh interval - the config
runs `vsync: true`, and setting it false currently panics at boot in
`Surface::configure` - so read a change of less than one 16.7 ms step as noise.

Plus the new `fuji-forest-hillside` vantage (earth, lat 35.3, lon 138.8,
altitude_km 0.15, look_offset_deg 72), added in this brief's commit. There were
25 vantages, exactly one forest one, and both forest cameras sat at 30 m INSIDE
the stand, so the card rung had no clean rig coverage from above the canopy,
which is precisely the view the operator's "black squares in a grid" verdict came
from.

## Verification bar

`cargo check --features native` AND `cargo check --features relay
--no-default-features`. Then, because this is renderer plus bind-group-adjacent
plus shader work, the rig, which enters the world (a menu boot is not the bar,
see the v0.1029-v0.1038 incident in CLAUDE.md):

```
cargo build --features native --release
node scripts/probe-sweep.js --only fuji-forest-ground --exe target/release/HumanityOS.exe
node scripts/probe-sweep.js --only fuji-forest-hillside --exe target/release/HumanityOS.exe
node scripts/probe-sweep.js --only ground-storm-inslab --exe target/release/HumanityOS.exe
```

Expect panics 0 on each. `.probe-rig/HumanityOS.exe` is a HARD LINK to
`target/release/HumanityOS.exe`, so a running rig locks the source and the next
sweep dies with EBUSY. Check `ExecutablePath` before killing anything named
HumanityOS.exe: the operator's own game, another session's rig and yours all
look identical in `tasklist`.

The bake itself is provable without the rig through the existing PNG dump
(`bake_billboard_to_png` plus the showcase `{"bake":"trees"}` IPC). Use it to
eyeball all 24 tiles before spending a sweep.

## Explicitly NOT in this increment

Each of these is real work that someone will be tempted to fold in. Do not.

- **Single-sided cards plus a cull-none colour pipeline for the card range.**
  Both emitters duplicate every triangle with reversed winding so the card
  survives back-face culling. Dropping the duplicate is another ~2x on card
  geometry on top of item 5 (a tree would go from 24 vertices + 24 indices to
  4 + 12), and it also stops every card being rasterised TWICE into the shadow
  map for zero effect, since both depth-only pipelines are already
  `cull_mode: None` (`src/renderer/pipeline.rs:560`, "Patch Batch Shadow
  Pipeline", and `:705`, "Sun Shadow Pipeline"). It is left out because it is
  not a two-closure edit like item 5: it needs a new colour-pipeline variant, a
  new field on `PatchSlot`, and a mesh-block reorder to [grid+skirt | cards] so
  the card index range is contiguous. RE-MEASURE AFTER ITEM 5 FIRST - item 5
  alone may already put the arena far enough under the ceiling that this buys
  nothing visible.
- **MSAA.** The residual black speckle in the near canopy is geometric aliasing
  of sub-pixel leaf blades, not shading: there is no MSAA anywhere (every
  `MultisampleState` in `src/renderer/pipeline.rs` is `count: 1`, e.g. `:540`
  and `:642`, default elsewhere) and no temporal accumulation. MSAA 4x at
  2560x1440 is roughly 20-30% of frame time and interacts with the bloom/SSAO
  chain, so it is an operator decision and its own wave. What this increment
  DOES owe it is one sentence of provenance on the gate, which is why
  `fuji-forest-ground`'s black-canopy regression now records its capture width:
  the same v0.1081 build measures 0.448 / 22.8% at 2560x1387 and 0.211 / 35.3%
  at 1280x720, so the dark-fraction half of that gate passes at one width and
  fails at the other.
- **Near-tree frustum culling.** The 256 near 3D models are selected
  view-independently on purpose (the card-hide radius `covered_r2` has to stay
  stable), and frustum-testing only the colour-pass push would be
  image-identical. Left out because it was MEASURED to cost nothing today:
  disabling the near models ENTIRELY moved 130.9 ms to 132.8 ms, inside the
  noise. Correct work with no current payoff; revisit when the near models are
  the limit, not before.
- **Per-patch card index-range culling.** About 30.8M of the ~32M card vertices
  drawn per frame are discarded on distance by
  `90-fragment-main.wgsl:801-806`, and skipping their index range per patch is
  cheap because the batched path already rebuilds `IndirectArgs` per patch per
  frame (`renderer/mod.rs:2949-2961`). But it needs the mesh block order changed
  to [grid+skirt][vegetation], and after item 5 lands the arena is no longer the
  constraint. RE-MEASURE FIRST. It may simply be unnecessary.
- **Shadow-pass caster culling.** The shadow pass costs a measured 20.8 ms, 22%
  of the frame, and culls casters at 6 km against a +/-1500 m ortho box. Its own
  wave, and it must be a bounding-sphere-vs-light-frustum test, never a distance
  test: a low sun makes a distant tall caster's shadow legitimately reach into
  the box. Related, and the reason it is a fidelity question too: both shadow
  pipelines are `fragment: None` (`renderer/pipeline.rs:558` and `:703`), so the
  alpha cutout never runs in the shadow pass and every card writes depth as a
  SOLID RECTANGLE.
- **Atlas mips.** The atlas is `mip_level_count: 1` (`renderer/mod.rs:942`)
  sampled with `textureSampleLevel(..., 0.0)` behind a hard 0.5 alpha test, which
  is 10-25x minification with no filtering and is a real source of distant
  sparkle. The naive fix is a trap: box-filtered mips of an alpha cutout lose
  coverage and the canopy visibly thins as it recedes. It needs alpha-coverage
  preservation (Castano's per-mip binary search for a scale that holds the
  above-cutoff texel fraction constant), which is its own increment with its own
  acceptance.
- **Procedural tree mesh LODs.** `tree_mesh.rs` `limb()` already takes
  `max_depth`, so LOD1 = depth 2 and LOD2 = depth 1 with `leaf_size` scaled up is
  cheap to build, and at up to 2.2M mostly sub-pixel leaf quads it is the largest
  frame-time lever at this vantage. It changes the image at every swap distance,
  so it needs its own before/after.
- **Stand clustering, reverse-J height distribution, understory, more than 3
  variants.** The stand reads as an orchard: uniform-random placement in 220 m
  cells (`planet_chunks.rs:2205`), one canopy height, three meshes per species
  (`:2263`), bare ground beneath. Correctly ranked after this one, because
  shrubs and extra variants each need matching atlas entries or they pop at the
  handoff, which is the same trap `PRIORITIES.md:191-195` already recorded.
- **Instanced grass.** Near-ground luminance standard deviation measures
  2.17-2.68 out of 255 (under 1.1%) against 48 in the canopy in the same frames,
  because grass is 0.147 untextured tufts per square metre. It is the reserved
  instancing increment 1 and needs no atlas bake, which is exactly why it is
  independent of this work.
- **The black canopy. FIXED v0.1081, and the cause was NOT ambient.** This
  bullet used to name `90-fragment-main.wgsl`'s `albedo * vec3(0.005, 0.005,
  0.006)` ambient plus the 12 m detail gate as the reason a backlit crown went
  black, and defer a hemispheric-ambient rewrite of the shared BRDF. That
  diagnosis was wrong at the root, so do not re-defer that rewrite on its
  strength. The real mechanism was an ORGAN TAG that was never set:
  `tree_mesh::blade()` emitted foliage through `PlantMeshBuilder::tri2`, and
  `tri2` bakes whatever `self.organ` currently is - always `Organ::Stem`,
  because only `plant_mesh::leaf`/`petal` ever assigned `Organ::Leaf` and
  `blade()` called neither. Bit 19 was therefore clear on every foliage face of
  sakura, momiji, oak, birch and acacia (76% of the stand at Fuji, effectively
  100% in a shipped build with no `assets/models/`), `is_leaf` evaluated false,
  and the canopy was shaded by the BARK branch - stretched voronoi fissures,
  0.42x crevice darkening, roughness 0.78-0.96, and no transmission term at all.
  Palm was the only species that ever looked right, because its fronds go
  through `b.leaf()`. Measured before: canopy median luma 8.4 against sky 133.8,
  a ratio of 0.063, with 53.9% of canopy pixels under luma 16. The fix is
  `PlantMeshBuilder::set_organ` plus three lines in `blade()`, and hoisting the
  two transmission terms out of the `detail > 0.001` gate so transmittance
  (a material property) runs to the 120 m model cutoff while venation, mottle,
  pucker, wax and `micro` stay distance-faded. Gated by the new "NO black
  backlit canopy" regression on `fuji-forest-ground`. A hemispheric ambient is
  still a legitimate future want, but it is now an ordinary quality item, not
  the lever for this defect.
- **Wind.** Separate file, separate task, no conflict. See
  `docs/design/foliage-wind-from-weather.md`.
