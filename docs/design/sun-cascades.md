# Near sun cascades with the home as a caster

Written 2026-09-27 from a read-only planning pass over the code, after a
lighting review found that the player's home never casts into the sun shadow
map. It is the build order for the fix. Line numbers are as of v0.1386.0;
re-grep before trusting one.

## 1. How the sun shadow map works today, and why the home never casts

- **Build and fit** happen in `render_celestial_onto` (celestial.rs 538-646).
  It is one 4096 square Depth32Float ortho map (mod.rs 119-121, 1564-1587), a
  box of plus or minus 1,500 m centred on the eye, depth fitted to 2,400 to
  5,600 m, texel-snapped against the render origin (573-576). Texels are
  0.732 m, pinned as `SHADOW_TEXEL_M` (00-bindings-vertex.wgsl 273) with a
  lockstep test at sky_ambient.rs 101.
- **Storage** is group 3: binding 6 the map, 7 the comparison sampler, 8 the
  96 B `ShadowUniforms` (mod.rs 1711; layout at pipeline.rs 430-457). Binding 8
  has `min_binding_size: None` (pipeline.rs 454).
- **Sampling** is `sun_shadow_offset` (00-bindings 294-334): 3x3 PCF with an
  NDC bias of 0.0006 and 0.0025, which is 1.9 m and 8 m in world units
  (celestial.rs 562-563).
  - It is called from `frag_tail` (80-fragment-shared.wgsl 249-254), so every
    colour PSO whose entry reaches `frag_tail` samples it.
  - Direct calls also exist: fs_surface type 19 (90-fragment-main.wgsl 1000),
    fs_vegetation (1746, 2088, 2130) and the ocean (511, 779).
  - `sun_gate` (80 line 146) is only changed in fs_terrain (1257, 1315, 1621);
    fs_surface and fs_vegetation pass it through unchanged.
  - The shadow twin `fs_shadow` (90 line 2283) writes the map, using the
    dummy-depth group-3 twin (mod.rs 1817, `AlbedoBindGroup` at mod.rs 195).
- **Why the home is excluded.** It is a structural gap, not a floating-origin
  problem and not a documented cost cut. The caster loops walk only the
  celestial pass's own `objects` and `transparent` arguments and the patch
  arena (celestial.rs 745, 816, 891); `all_objects` goes only to
  `render_scene_onto` (lib.rs 15385). Both passes share the same render space
  and `state.camera`, and the home already RECEIVES the map. The v0.899 commit
  (c8c0c3bc) titled "home all cast" wired only receiving. Even if the home were
  drawn into this map, a 0.4 m shelf gap would vanish inside the 1.9 m bias.
- **A second defect.** The map is built along the world-frame `sun_dir_f`
  (lib.rs 15176, and again at ipc.rs 1311), but the deck is lit by the
  hull-frame sun (`to_hull`, lib.rs 11289). Aboard the station these differ by
  the station's attitude. The near cascades must use `cur_sun` (mod.rs 2637),
  which is what lights the home; on a planet the two are equal. The celestial
  pass's own direction is a separate fix.

Measured baseline (25b-mushroom-racks, noon): the lowest shelf top under three
shelves reads 130 sRGB against the open floor's 133.

## 2. Design: camera-centred clipmap cascades

The virtual-shadow-map clipmap idea without paging. Camera-centred rather than
frustum-fitted because at the default 90 degree vertical field of view a
bounding-sphere fit of width D reaches only about D/4.1 of view depth, while a
camera-centred box reaches D/2 and never refits when the player turns.

**Atlas:** Depth32Float, 8192 x 4096 (128 MiB, 64 MiB more than today). The
far map sits unchanged in tile (0,0), 4096 square, with the same fit, bias and
`SHADOW_TEXEL_M`. Four 2048 square near tiles:

| Cascade | Half-extent | Texel |
| --- | --- | --- |
| C0 | 4 m | 3.9 mm |
| C1 | 15 m | 1.46 cm |
| C2 | 60 m | 5.9 cm |
| C3 | 240 m | 23 cm |
| Far (existing) | 1,500 m | 0.73 m |

The review's suggested 30 m at 1.5 cm is C1 (16 MiB, cheap to fill), but on
its own it leaves a 50x texel jump at 15 m; the ratio-4 ladder keeps each
boundary within about 2x of the 1080p pixel footprint. In a 3 m room the whole
room is inside C1 from anywhere in it, and C0 covers arm's-length shelves. On
a planet surface nothing beyond 240 m changes.

**Fitting and bias.** Snap the eye to the texel grid in light space. Build the
light basis from `cur_sun`, quantised to 0.02 degree steps so the grid does not
crawl. Extend the depth range 4H (at least 200 m) toward the sun. Bias in world
units per cascade: one texel constant, plus one texel times sin(theta) as a
normal offset along the geometric normal. The shadow PSOs keep no hardware bias
(pipeline.rs 1420-1426).

**What casts.** Home opaque meshes (walls, floors, shelves, racks, machines,
fungus blocks, NPCs); plants (type 20 through the depth-only PSO, types 19 and
21 as cutouts through the existing `shadow_for` and shadow_cutout.rs 32);
celestial casters (patches, near trees, cards, wave crests). Glass and PVC
tents wait for increment 4.

**Culling.** Add a bounding sphere to `Mesh` (its only struct literal is
mesh.rs 163). Reject a caster per cascade when its light-space offset exceeds
H + R or it lies outside the depth slab. Terrain patches get a radius per arena
slot.

**Staging.** `stage_sun_casters(&all_objects, station_off, levels)`, one line
before lib.rs 15332. The capture path (ipc.rs 1317) reuses the staged set, so
rig screenshots show the same shadows as the live frame. Home caster uniforms
chain after the celestial slots in the existing upload (celestial.rs 698), with
a guard against `MAX_OBJECTS`.

**Where the code goes.** A new `src/renderer/sun_cascades.rs` (fitting,
culling, the near pass with `gpu.shadow_near` and `cpu.shadow_near` timers,
uniform packing; timers added to budget_systems.ron 99-107) and a new
`assets/shaders/pbr/07-sun-cascades.wgsl`, inserted after 05 in
shader_loader.rs 54. celestial.rs grows about 8 lines, lib.rs 1 line;
renderer/mod.rs stays at or below its current count (fold
`shadow_uniform_buffer` and `light_camera_*` into one field, the four extra
light cameras as an array at mod.rs 1723).

**Bind group layouts: none change.** Binding 6 stays `texture_depth_2d` (only
the view grows). Binding 8 grows from 96 B to 496 B (four matrices, four tile
rects, four bias vectors and a control vector). All four group-3 bind groups
already bind the whole buffer (mod.rs 1785, 1855; material_bind_groups.rs 183,
built twice). Fix the stale comment at 00-bindings 269-272 that says this needs
a layout change.

**Megashader switch.** `override HAS_SUN_CASCADES_BRANCH: bool = true;`, guard
`if (HAS_SUN_CASCADES_BRANCH && shadow_u.near_ctl.x > 0.5)` right after
`frag_prologue` in fs_surface, fs_vegetation and fs_terrain, writing a new
FragSetup field `sun_vis` (default -1). `frag_tail` uses `sun_vis` when set and
the far map otherwise, reading no switch. The four in-entry `sun_shadow` calls
use `sun_vis` too, so leaf transmission is blocked by shelves (the BUG-060
rule). Tests: the switch in its own `LIGHTING_SWITCHES` list (the existing
`ALL_BRANCH_SWITCHES` feeds the band parser, pipeline.rs 2536, and the "all
three off" assertion, 2473-2480, which assume shell switches); live for
Surface, Terrain and Vegetation, dead for Water, Shell, Cloud and the four
shadow PSOs; extend the live-set test (2396); a test that the cascade function
is used only at the three guarded sites; extend the `validate_wgsl` switch
check.

## 3. Where this meets the room GI work (docs/design/room-gi.md)

- **Bind groups.** Room GI owns group 0 (`pipeline::camera_bind_group`,
  pipeline.rs 2997). This plan adds no group-0 entry and no new call site.
  Neither side touches the group-3 layout. Contact shadows would be the first
  thing that needs one, and they wait until GI has landed.
- **Frame order.** Cascades render in the shadow encoder (celestial.rs
  695-904); the probe update dispatches after that and before lib.rs 15385.
- **Sun visibility.** The probes' sun test covers only the room box and lid;
  probe hit points under racks need atlas visibility times lid transmittance.
  Write the cascade function with texture and sampler parameters so the probe
  update shader includes it rather than copying it, binding the atlas and
  `ShadowUniforms` in its own compute layout.
- **Glass.** When glass starts casting (increment 4), GI drops its lid factor
  for points the atlas covers, in the same commit; a flag at `near_ctl.w` tells
  both sides.
- **Shared shader file.** `sun_vis` goes right after `sun_gate` (80 line 74);
  GI's indirect term is a different hunk (80 lines 390-393).

## 4. Increments

Budget: the interior frame is about 14 ms and stays at or below 15.5 ms.

0. **Fixtures, no renderer change.** The station camera verb takes only
   `screen` (ipc.rs 2208-2275): add a `pose` option. photograph-home.js never
   pins the clock (238-251): add pinned vantages to tests/visual/vantages.json
   (`home-racks-noon` and `home-racks-dawn` at the 25b pose, noon being time
   20.15 as in home-clock-noon; `home-beds-noon` at vantage 24;
   `home-overview-noon` at vantage 00). Showcase keys `sun_shadows` and
   `near_levels` for same-boot A/B. A region-of-interest script beside
   home-clock-metrics.mjs (floor, shelf tops 1 to 4, rack post base, wall).
1. **Atlas, C1, home casters, PCF and the switch.** Pass: at noon the lowest
   shelf reads at most the night capture plus 10% (the sun term gone); the
   open floor unchanged within 2 codes; with `near_levels=0`, sahara-noon-ground,
   fuji-forest-ground, moon-surface-200m, ocean-grazing-calm and
   silverdale-osm-ground differ by at most 1 sRGB; `gpu.shadow_near` at most
   0.6 ms and `gpu.scene` up by at most 0.4 ms at 1600x900.
2. **The full ladder, culling, blend bands, a Settings slider.** Pass: the
   floor region's standard deviation at noon and dawn rises by at most 0.5
   (acne); a 5 mm camera step changes the shadow-edge region by at most 2
   (snapping); forest shadow total up by at most 2.0 ms.
3. **PCSS in C0 to C2** (a 16-tap blocker search with `textureLoad`, the sun
   at 0.53 degrees). Predicted penumbra 2.8 cm from a 3 m wall edge and 19 cm
   from a 20 m conifer, within 30%. A light gap of 3 px or more under rack feet
   at 1.5 m calls for contact shadows.
4. **Stochastic glass and PVC in fs_shadow, and the GI handshake.** Pass: the
   floor's lit-minus-night value comes out at 0.65 plus or minus 0.05 of
   increment 3's, with no visible dither pattern.

**What would prove it wrong:** a shelf-to-floor ratio above 0.9 after
increment 1 (casters not staged, or `station_off` or culling wrong); the open
floor getting darker (hull frame against world frame); a far-field difference
above 1 sRGB (the atlas remapping); flickering shadow edges (the texel snap);
cost over budget (drop C3 on planets).
