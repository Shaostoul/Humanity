# Room GI: per-room irradiance probes

Written 2026-09-27 with rung 1. Line numbers drift; grep the names.

## 1. The problem

Ship interiors had no indirect light at all. The interior passes write the whole
camera uniform and zero the pad that carries the local up, so `frag_tail`
(`assets/shaders/pbr/80-fragment-shared.wgsl`) decides the fragment is not
under a sky, `sky_ambient` returns 0, and the indirect term is exactly
`AMBIENT_FLOOR` = 0.005: a silhouette floor, not light. Point lights cast no
shadows and have no indirect term either.

Measured at `25b-mushroom-racks`, 1600x900 (photograph-home rig): the floor box
(900,700)-(1300,850) reads sRGB 146 (about 0.20 before the tone map) and the
wall box (1000,350)-(1300,480) reads 4.0, which is the ambient floor. The wall
sits at 2.5% of the floor. A closed room at mean reflectance 0.5 returns about a
quarter of the floor's light to its walls by interreflection alone (the
integrating-sphere relation `E_ind = rho * Phi / (A * (1 - rho))`, about 0.24 to
0.31 of the floor's direct illuminance for a 10 x 10 x 3 m room).

## 2. The destination: DDGI, one grid per room

Dynamic Diffuse Global Illumination: a lattice of irradiance probes, each
updated every frame by tracing a rotated set of rays and blending the result
into an octahedral irradiance map and a depth-moment map, with the previous
frame's probes supplying every bounce after the first (Majercik, Guertin,
Nowrouzezahrai and McGuire, "Dynamic Diffuse Global Illumination with Ray-Traced
Irradiance Fields", JCGT 8(2), 2019). A shading point blends the eight probes
around it, weighted by a smooth backface term and a Chebyshev visibility test on
the depth moments. Relocation and classification (Majercik, Mara, Shirley and
McGuire, "Scaling Probe-Based Real-Time Dynamic Global Illumination for
Production", JCGT 10(2), 2021) move probes out of geometry and switch off the
ones that can see nothing.

The change this project makes: ONE GRID PER ROOM, and a fragment samples only
its own room's grid. DDGI's classic failure is light leaking through a thin wall
from a probe on the far side; the Chebyshev test reduces it but cannot remove it
where a wall is thinner than the probe spacing. Here the probe on the far side
belongs to a different grid, so the leak cannot happen by construction. The ship
is authored as rooms (`HomeStructure::detect_rooms`), so the partition comes for
free.

## 3. Rung 1, as built

Rung 1 is the whole architecture with the simplest scene representation: each
room's probes trace against the room's own box, analytically. The storage
format, the update, the blending and the sampling are DDGI as published; rung 2
replaces only the box intersection.

**Rooms.** `engine/room_gi.rs` turns the detected rooms (`RoomInfo`, flood-filled
AABBs) into `RoomBox`es on every home build or edit. Each vertical face snaps out
to the room-side surface of the wall it stands against (the flood fill stops up
to one 0.5 m cell short) and takes that wall's exposed material colour from
`data/blueprints/wall_materials.ron`, overlap-weighted where several walls share
a face and the zone shell's colour for any bare stretch. The floor takes the
shell material, the lid the roof material. A glass roof passes `1 - alpha` of
the light (that is what the renderer's alpha blend lets through a pane) and
reflects `alpha * colour`. Every reflectance is clamped to 0.95.

**Lattice.** 1 m target spacing, `ceil(size / 1 m) + 1` probes per axis (never
fewer than 2), probes at cell centres so none sits on a surface. The mushroom
room is 11 x 4 x 11. A room that alone would hold more than 8,192 probes
coarsens by itself (the 8 m commons hall, 17,640 at 1 m, runs at 1.46 m), so
one hall cannot coarsen every bedroom; the whole ship is then capped at 32,768
(the shipped ship holds 31,874: 34 rooms, the acre's all at 1 m).

**Storage.** One Rgba16Float atlas, 8100 texels wide: every probe's 8x8
octahedral irradiance tile (10x10 with the 1-texel border) in the top region,
its 16x16 depth-moment tile (18x18) below. The border holds the octahedral wrap
(edges mirrored, corners from the opposite corner) so a bilinear tap at a seam
reads the right neighbour. The irradiance texels' alpha carries the probe's
update count. The atlas grows with a quarter of headroom and is zeroed by a
compute clear whenever the rooms change.

**Update** (`assets/shaders/room_probes_update.wgsl`, one compute pass before the
scene pass). `cs_trace`: one workgroup of 64 threads per probe, one ray each,
from a spherical Fibonacci set turned by a fresh random rotation every update.
A ray hits the box face it exits through. The hit is shaded as a Lambert surface
of that face's reflectance lit by:
- the room's OWN lights, with exactly the fragment loop's attenuation (the line
  light closest point, the range window, `intensity / (1 + d^2)`, the spot cone
  and the 0.001 floor). A light belongs to the room its position (a line light's
  midpoint) lies in, so one room's lamp never reaches another's probes;
- the sun, only if the way from the hit to the sun leaves through a glass lid,
  times the lid's transmittance (the box is convex, so that is the whole test);
- the previous update's probes of the same room, sampled exactly as a fragment
  samples them, which is every bounce after the first.

A ray that meets a glass lid also brings back the sky through it from this
frame's sky-view table (zero when there is none, as aboard a station in orbit).
Each thread then folds the 64 rays into one irradiance texel (cosine-weighted
mean radiance, DDGI's normalisation) and four depth texels (mean distance and
mean squared distance, weighted `cos^50`), blends into the previous values and
writes the tile and its border into a scratch texture; `cs_resolve` copies the
finished tiles into the atlas (a probe's bounce reads its neighbours while they
are being rewritten, so the atlas is read-only in the trace). The blend is a
running mean over a probe's first updates (1, 1/2, 1/3, ...) settling at DDGI's
hysteresis of 0.97, so a fresh or cleared probe is right after one update
instead of creeping up at 3% a frame.

Budget: at most 4096 probes a frame, the camera's room first (all of it every
frame, or a moving window of three quarters of the budget through a room bigger
than that) and the rest of the ship round-robin, so the acre is refreshed about
every eight frames. Past 60 m from every room the update stops and the probes
keep what they have.

**Sampling** (`assets/shaders/pbr/85-room-gi.wgsl`, shared text between the
megashader and the update shader). The room is picked half a metre along the
normal and accepted within 0.25 m of a box: an inside wall face picks the room
it faces, and the outside of the hull picks nothing. The room table is written
in pick order every frame (the camera's room first, then the rest smallest
first), so the first box containing the point wins and most fragments stop at
the first step. Then DDGI's sample: the 8 probes of the cell around the point
offset 0.1 m along the normal, each weighted by the smooth backface term, the
Chebyshev test on its depth moments, the weight crush at 0.2 and its trilinear
weight, blended in square-root space. A probe with no trilinear weight is
skipped with its fetches, which is four of the eight on any floor, wall or
ceiling. In `frag_tail`:

    if (HAS_ROOM_GI && !under_sky && !screen_emitter) { ... indirect = max(room_light, AMBIENT_FLOOR) }

(a screen's colour replaces the lit result, so it never pays for the sample).

**The visibility test runs only where it can change the answer.** Inside a room
whose probes trace only its box, DDGI's Chebyshev test is an identity: the box is
convex, so a probe's ray toward any point inside it leaves the box no nearer than
that point, the stored mean depth is never short of the point's distance, and the
weight is 1. The twin checks this on every face and through the interior
(`visibility_is_an_identity_inside_a_box`: no sample moves by 0.05%). The test
and its depth fetch were also half the sampling cost, so a per-room flag
(`ROOM_FLAG_VISIBILITY`, WGSL `GI_ROOM_VISIBILITY`) decides whether a room runs
it: rung 1 leaves it off everywhere, rung 2 sets it for the rooms whose contents
it traces, and `showcase {"room_gi_vis":"1"}` forces it on for A/B. The depth
moments are written every update regardless, so rung 2 finds them converged. A
see-through surface (a tent's film, a pane) never runs it either; in the
mushroom room's tent overdraw it was the largest single cost.

`HAS_ROOM_GI` is a FEATURE switch, a second kind beside the three shell
switches: it guards a block in the shared tail and is live in more than one
class, the surface and vegetation PSOs (the classes that draw the inside of a
room), and compiled out of the terrain, water, shell and cloud PSOs.
`pipeline.rs` (`ShaderClass::live_features`, `ALL_SWITCHES`) and
`shader_loader::validate_wgsl` hold it to the same rules as the shell switches.
Outside every room the old floor stands, bit for bit.

**Bindings.** Group 0 bindings 5 to 7: the atlas, a bilinear clamp sampler, and
the room table (a 64-byte header, then 160 bytes per room). All four camera bind
group sites go through `pipeline::camera_bind_group`, and every grow path
(lights, environment regions, the room GI atlas and table) now calls one
`Renderer::rebuild_camera_bind_groups`, which rebuilds the sun-shadow pass's
camera bind group too (the lights grow used to leave that one stale).

**Units.** Nothing new. A probe stores the cosine-weighted mean radiance, E/pi,
the same quantity `sky_ambient` returns, so the tail's `albedo * indirect * ao`
reads it directly. `watts` is not read.

**Dev switch.** `showcase {"room_gi":"0"}` makes the room table say "off" (every
fragment keeps the old floor) and skips the dispatch, so a capture and its
room-GI-off twin in one boot differ by exactly the light the probes add, and the
cost is `gpu.room_probes` plus the difference in `gpu.scene` and
`gpu.transparent`. `{"room_gi_vis":"1"}` forces the visibility test on in every
room. The rigs: `node scripts/photograph-home.js --ab room_gi` photographs every
home vantage both ways (`--vantages FILE` tries poses without touching the
shipped list), the home vantage `25c-oyster-rack-close` is the close shelf-and-
block pose the acceptance reads, and the probe-sweep vantage
`console-face-6-room-gi-off` is `console-face-6`'s twin (probe-sweep resets the
switch between vantages with the other sticky pins). The Performance page counts
`gpu.room_probes` in the homestead row, `cpu.room_probes` in the uploads row, and
the atlas with the render targets.

**Files.** `src/renderer/room_probes.rs` (layout, math, CPU twin, tests),
`src/renderer/room_probes_gpu.rs` (atlas, table, update, bind group rebuild),
`src/engine/room_gi.rs` (rooms to boxes, the per-frame hook),
`assets/shaders/pbr/85-room-gi.wgsl`, `assets/shaders/room_probes_update.wgsl`,
and one line each in `lib.rs`, `world_load.rs`, `home_meshes.rs`.

**Tests** (the CPU twin is the WGSL transcribed name for name; each test was
seen failing first against a deliberate mutation, recorded in its comment):
- `closed_box_converges_to_the_integrating_sphere_value`: uniform direct light
  in a closed box converges to `rho * E0 / (pi * (1 - rho))`. Measured 0.7424
  against 0.7427 at rho 0.7.
- `closed_box_with_a_lamp_matches_the_mean_direct_light`: a real lamp in a 4 m
  cube, against the area-mean direct light. Measured +7.4% (the lamp's brightest
  patches are also nearest the probes).
- `with_the_lights_off_the_glow_decays_to_the_floor`: no self-sustaining energy.
- `a_probe_never_reads_the_neighbouring_rooms_light`: room B stays exactly 0 with
  a lamp in room A, and the shared wall's faces pick their own rooms.
- `visibility_is_an_identity_inside_a_box`: why rung 1 skips the Chebyshev test.
- `octahedral_encode_decode_round_trips`, `tile_border_copies_the_texel_across_the_seam`,
  `the_sun_comes_in_through_the_glass_lid_only`, `lattice_and_layout_are_what_the_design_says`
  (a hall coarsens alone, a bedroom keeps 1 m).
- `room_probes_gpu::tests`: the update shader validates with its three entries,
  naga's layout of `GiHeader`, `GiRoom`, `GiParams` and `GiLight` matches the
  Rust records byte for byte, and the shader's constant tiles-per-row match
  `AtlasLayout`.
- `pipeline.rs` `the_room_gi_sample_is_guarded_by_its_switch_in_the_tail` and the
  updated permutation tests; `shader_loader` refuses a megashader without
  `HAS_ROOM_GI`.
- `engine::room_gi::tests`: the shipped ship's rooms become sane boxes; a face
  takes the wall it stands against.

## 4. Measured

MEASURED_PLACEHOLDER

## 5. Known limits of rung 1

- **The room's contents are not traced.** A probe sees the six faces of its box
  and nothing in the room: racks, shelves, furniture and machines neither block
  nor bounce probe rays, and a probe inside a rack is as lit as one in the open.
  A shelf underside reads the floor below it as if nothing stood between. Rung 2.
- **The sun in the probes and the sun on the surfaces disagree.** The probe
  update lets the sun in through the glass lid only where the box's walls do not
  block it; the fragment's direct sun has no such test, because the home never
  casts into the sun shadow map. Until the near cascades land, a low sun lights
  the whole floor on screen while the probes see part of it in a wall's lee.
- **Glass walls are opaque to the probes.** A glass partition reflects its
  `alpha * colour` and passes nothing to the room behind it; light through
  glass walls and open doorways between rooms is a portal problem for a later
  rung.
- **Rooms are boxes.** An L-shaped room is its bounding box; where that box
  overlaps a neighbour, the smaller box wins the pick.
- **No fill light.** The fixed fill light was retired in v0.1104; the probes do
  not model it.
- **The CPU twin has no sky term.** The GPU adds the sky through a glass lid
  when a sky-view table exists; the twin does not model the table. Aboard the
  orbiting station the term is zero anyway.

## 6. Next

1. **Near sun cascades with the home as a caster** (`docs/design/sun-cascades.md`):
   makes the fragment's direct sun agree with what the probes see, and stops the
   sun lighting shelves through the shelves above them. It touches
   `80-fragment-shared.wgsl` and the frame order, so it follows this.
2. **Rung 2, the room's contents.** Voxelize each room's static contents (the
   machines, racks, furniture, the walls' openings) into a small per-room
   occupancy or distance volume, trace the probe rays through it instead of
   against the box, and turn on DDGI's relocation and classification (JCGT
   2021) so a probe that lands inside a rack moves out of it or switches off.
   The storage, the update and the sampling do not change.
3. **Portals.** Light through glass walls and open doorways into the next room:
   sample the neighbouring room's probes at the portal as a directional source.
