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

Every acceptance number below is worthless until the rig measures this vantage
at steady state.

`tests/visual/vantages.json`, `fuji-forest-ground`: `settle_s` 25 -> 75.
Measured 2026-07-31 on the operator's max-quality rig config (terrain_split_px
2, terrain_patch_budget 12288): at 25 s the sweep captures 43-48 ms while the
scene is still streaming, and the frame keeps getting heavier for about another
40 s before settling at 93.7 ms +/- 5.8, which is 10.7 fps. The sweep was
reporting 21.3 and 18.8 fps and passing an 18 fps floor at 55% of the true
number.

Note that `perf_floor_fps` never gates anything: `scripts/probe-sweep.js:224`
records it into the manifest and `rec.ok` is set true regardless. The floor is
advisory. A real steady-state gate would wait for `[PatchBatch]` to stop
changing in `run.log` instead of sleeping a fixed `settle_s`
(`scripts/probe-sweep.js:236` is a plain sleep), which is a rig change and is
NOT part of this increment.

DONE in this brief's commit, along with the acceptance text.

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

Bake cost: 24 single ortho passes. The existing six take 1-2 s total per the
`[Bake]` log at `src/lib.rs:9498`, and that is at 512 px, so 24 at 256 px is
well under a second of the world-load path. Measure it and put the number in the
log line.

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

The shader half of item 6 is a localized edit to the sprite branch at
`90-fragment-main.wgsl:808-827`. It is in the shared shader tail that CLAUDE.md
flags as a merge hazard, so it goes in as a single anchored insertion, not as a
free-hand pass over the file.

## Acceptance, exactly these

(a) NO flat rectangular slabs anywhere in the tree field at `fuji-forest-ground`
and at `ground-storm-inslab`. Every card past the 3D-model ring shows a crown
outline with sky visible through its edge.

(b) `[PatchArena]` "vertex arena full" warnings GONE from `run.log`, and
`[PatchBatch]` classic-fallback at 0, at the operator's max-quality rig config
(terrain_split_px 2, terrain_patch_budget 12288).

(c) Record the steady-state frame time before and after, both at 75 s settle, in
the commit message. The before number is 93.7 ms +/- 5.8; the expected after is
roughly the measured zero-classic state, 60-65 ms, but it is a measurement, not
a promise.

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
- **The black-canopy ambient fix.** `90-fragment-main.wgsl:1457` is
  `albedo * vec3(0.005, 0.005, 0.006)`, a 0.5% grey, and the type-20 leaf
  transmission is gated off past 12 m by the `smoothstep(2.5, 12.0, plant_dist)`
  at `:1163`, so a leaf edge-on to both the sun and the single fill light goes
  to pure black at local noon. This is the next-highest QUALITY item after this
  increment. It still gets its own wave, because a two-colour hemispheric
  ambient plus a sky-view LUT tap is a change to the shared BRDF region of the
  shader tail, which is the merge hazard CLAUDE.md calls out, and because it must
  be judged on its own before/after rather than tangled with a silhouette change.
- **Wind.** Separate file, separate task, no conflict. See
  `docs/design/foliage-wind-from-weather.md`.
