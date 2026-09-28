# Environment fields: giving the world's effects a place and a size

**Status:** designed 2026-09-21 (operator request), building from increment 1.

The operator, after BUG-080: "Can we modularize/layer the params thing so we
don't max it out? What else should have latitude, longitude, and size? Not just
clouds, but other things too ... I really want to make sure our environmental
effects are consistent and realistic."

Both halves have the same answer. Stop hunting for a spare scalar per effect and
add ONE mechanism that every environmental effect reads.

## Why the current shape runs out

BUG-080 needed a single new per-frame number and there was nowhere to put it.
`material.params2` is full on the cloud material (`.x` and `.y` slab bounds,
`.z` planet radius, `.w` the placement pin) and every camera light pad is
already claimed, much of it through `.xyz` swizzles that a grep for `.x` does
not see.

That is not bad luck, it is what packing scalars into leftover vector lanes
always does. Each new effect costs a scavenger hunt, the lanes acquire
overloaded meanings (`params2.w` alone encodes a 0..1 blend, a dev pin at 1, a
type pin at 2+tc, and a temporal flag at +4), and the next person has to decode
them before they can add anything.

The repo has already solved this once. Scene lights used to live in capped
uniform pads; v0.782 moved them to an uncapped storage buffer of 64-byte
`GpuLight` structs bound at group 0 binding 1. Environment regions get the same
treatment at binding 4. One binding, uncapped, self-describing, and adding a new
kind of effect costs a row rather than a lane.

## The layering

Three layers, deliberately separate, because they have different costs and
different lifetimes.

**Layer 1: the analytic base field.** Continuous, defined everywhere, no storage
at all. Temperature from latitude, altitude and season; pressure from altitude;
prevailing wind from the circulation bands. A pure function of position and
time, implemented TWICE and kept in lockstep: once in Rust for the simulation
and the HUD, once in WGSL for the renderer. This repo already does exactly that
for ocean waves, where a CPU f64 twin matches the shader mod 1 (see the
CLAUDE.md gotcha on the water arc), so the pattern and its test discipline
already exist here.

**Layer 2: regions.** Discrete, positioned, sized, transient: a storm, a fog
bank, a fire, a flood, an outbreak, an aurora. These live in the storage buffer.
Each carries a planet-fixed direction, an angular radius, a kind, an intensity
and a soft edge, so it sits on the world rather than in front of the camera.

**Layer 3: interpretation.** Each consumer decides what a kind means to it. The
cloud deck reads storm regions as a coverage floor and a placement weight.
Farming reads them as reduced light and free irrigation. The HUD reads them as
the word it prints. Nobody re-implements the geometry.

The rule that falls out, and the one that fixes BUG-080: **the weight of an
environmental effect at a point is a function of that point, never of the
camera.** No effect may read camera altitude, camera distance, or how much of
the planet is on screen. `wx_fade` violates this and is deleted by increment 3.

## The region record

    struct EnvRegion {
        dir_radius:  vec4<f32>,  // xyz = unit direction from body centre to the
                                 //       ground point, PLANET-FIXED;
                                 //   w = angular radius in radians
        kind_shape:  vec4<f32>,  // x = kind, y = intensity 0..1,
                                 // z = edge softness, w = altitude band
        params:      vec4<f32>,  // per-kind payload
    };

48 bytes, mirroring the style of `GpuLight`. Kinds are DATA
(`data/environment/region_kinds.ron`), not a Rust enum with a hardcoded match,
because a hardcoded list of domain objects is what `docs/design/infinite-of-x.md`
forbids, and because the condition table in `frame_shells.rs` (Storm mapping
straight to the pair 0.95, 0.95) is exactly that mistake today.

Influence is a smooth falloff in great-circle angle, evaluated identically on
both sides: the angle between the region's direction and the sample's direction,
smoothstepped from the radius inward by the softness fraction.

## What should have a latitude, a longitude and a size

The audit, from `src/systems/`. The short answer to the operator's question is
"almost everything we call environmental, and today almost none of it does."

**Already positioned, keep as is:** terrain and its 16 biomes; the MODIS cloud
placement; water bodies in `hydrology.rs` (they carry elevation and a downstream
topology); room atmospheres in `atmosphere.rs`, which are correctly per-zone.

**Already positioned and going to waste:** `disasters.rs` already stores
`position`, `radius`, `intensity` and `duration` per active disaster. It is the
right shape and nothing outside that file reads it, so a wildfire or a flood is
invisible. Feeding these straight into the region buffer is close to free and is
the cheapest proof the mechanism works.

**Global today, should be regional:**

1. **Weather condition and intensity.** The BUG-080 case. One scalar for a whole
   planet cannot be right at two altitudes at once.
2. **Wind.** Global speed and direction today. Real wind is a field: circulation
   bands by latitude, plus rotation around lows. It already drives cloud
   advection, and it should drive fire spread, sailing, flight, seed dispersal
   and turbine output. Layer 1 for the bands, layer 2 for the storms.
3. **Temperature.** Global plus a player-local lapse, which is the right
   instinct implemented in the wrong place. Belongs in layer 1: latitude,
   altitude, season, land-versus-sea contrast.
4. **Humidity and precipitation.** Global scalar. Should follow the storms and
   the coastlines, and it is what should decide whether falling water is rain or
   snow rather than a separate condition.
5. **Visibility and fog.** Global. Fog is the most obviously local weather there
   is: valleys, coastlines, mornings.
6. **Air pressure.** `atmosphere.rs` carries one `pressure_atm`. Altitude at
   minimum, and lows and highs if weather is going to mean anything.
7. **Snow and ice cover.** Should be a consequence of layers 1 and 2, not a
   toggle: a snow line that follows temperature, and sea ice that follows it.
8. **Disease outbreaks and species ranges** (`ecology.rs`). An outbreak has an
   origin and a spread front. A species has a range, which the biome map can
   supply.
9. **Pollution and contamination.** Water bodies carry a `contamination` scalar;
   air has nothing. Both want a source position and a plume.
10. **Aurora.** Genuinely latitude-shaped, and nearly free to draw once regions
    exist.
11. **Light pollution.** Needed the moment the night side is worth looking at,
    and it is a direct readout of where people live.
12. **Ocean currents and sea-surface temperature.** Hydrology has per-body
    temperature but no circulation.

**Deliberately not regional:** the season and the day/night terminator, which
are already global functions of time and orbit and are correct as they are.

## Build order

1. **The plumbing.** `EnvRegion`, the storage buffer, binding 4, the three
   `create_bind_group` sites on `pipeline.camera_bind_group_layout`, and the
   WGSL declaration. Its own commit with a world-entry probe boot, because this
   is the v0.1029 every-create-site class: miss one site and world entry panics
   while a menu-only boot stays green. `stars.rs` builds its own separate camera
   layout and is NOT affected; that is worth knowing before touching it.
2. **Disasters into the buffer.** The data already exists and is unused, so this
   exercises the path end to end without inventing anything.
3. **Weather systems.** Conditions gain a position and a radius; the cloud deck
   weights them by distance from the SAMPLE's ground point; `wx_fade` is
   deleted. The `stormfade-*` column going flat is the proof (BUG-080).
4. **Layer 1 proper.** Temperature, pressure and wind as analytic fields with
   locked CPU and GPU twins. **Built 2026-09-27**: see "Layer 1 as built" at the
   end of this document.

## Related

- `docs/design/weather-spatial-extent.md`: the BUG-080 diagnosis and the pad
  inventory that motivated this.
- `docs/design/infinite-of-x.md`: why the kind table is data.
- `docs/BUGS.md` BUG-080.

## A performance trap for whoever wires the first consumer

`env_influence_of_kind` loops over `arrayLength(&env_regions)`, which is the
buffer's CAPACITY (16 rows today), not the number of live regions. That is a
deliberate trade: it avoids carrying a count through the camera uniform, which is
the exact scarcity this mechanism exists to end, and an empty row costs one
compare.

But it means the call is not free inside a hot loop. **Evaluate the mask ONCE per
ray, at the ground point the ray is heading for, and carry the scalar into the
march.** Calling it per march sample multiplies sixteen iterations by the step
count, inside the shader this repo has already spent three releases making
cheaper (`docs/design/frame-cost-arc.md`). The cloud pass is the first consumer
and the one most able to make this mistake.

If the live region count ever genuinely grows past a handful, the answer is a
coarse spatial index or a per-frame cull to the visible cap, not a count uniform.

## A second trap, found by wiring the first consumer (2026-09-21)

Pick the ray's ground point as **where the ray meets the layer**, not as its
closest approach to the body centre. For a NADIR ray those are not the same
thing: the closest approach IS the centre, so `ro + rd * tca` is the zero vector
and normalizing it yields garbage. Every nadir fixture then reads exactly zero
influence.

That failure is invisible from either side alone. The CPU instrument reported
`influence_under_camera=1.000` while the render was unchanged, and it was the
CONTRADICTION between the two that localised the bug to the boundary. That is
what the 1 Hz `[EnvRegions]` line in `frame_shells.rs` is for, and why it stays:
without it, "the region is not built" and "the region is built but the shader
computes the wrong point to sample it at" look identical in a capture.

In `cloud_march_core` the right value is `m0`, the slab entry distance, which is
computed a few lines after the coverage read, so the resolve belongs after the
slab interval rather than beside the material fetch.

## The aurora was drawn in the wrong half of the sky (2026-09-22)

The operator, on the first shipped version: "it still kinda looks like it is
laying on its side instead of vertical as seen by the orangey/red color being
against the surface, maybe we got a rotation axis wrong?"

No axis was wrong. The ALTITUDE was, and the two look identical from a
distance, which is why this is worth writing down.

Band 2 carries its emitting layer as a fraction of the atmosphere shell
thickness, and the aurora shipped with 0.08 to 0.78. Nobody converted that to
kilometres. Doing it:

- Earth ships `atmosphere_scale` 0.015, so `shell_packing` gives
  `rp = 1 / 1.03 = 0.9709` in shell units.
- One shell unit is therefore `6371 / 0.9709 = 6562 km`.
- The ENTIRE shell is `(1 - 0.9709) * 6562 = 191 km` thick.
- So 0.08 to 0.78 is **15 km to 149 km**.

A real auroral curtain runs from a sharp lower border near 100 km to 300 km and
beyond for the red. The shipped one started in the stratosphere, below most of
the aurora and below a low orbit, so from 155 km the operator was flying ABOVE
the whole layer and looking down on a sheet. A sheet seen from above is exactly
what "laying on its side" describes.

Now 0.52 to 0.995, which is 99 km to 190 km.

**The general lesson, which is not about auroras.** A payload expressed as a
fraction of something is unreadable on its own. Both numbers looked like
sensible fractions, the render looked like an aurora, and the error only
surfaced when somebody standing at 155 km noticed the sky was in the wrong
place. Any band-2 or band-1 kind that means a real altitude should carry its
conversion in the comment beside it, as the aurora row now does.

### The cap, and why it is not fixed here

**Superseded 2026-09-27, kept for the reasoning:** the aurora no longer draws
on the shell mesh (see "The aurora gets its own pass" below), so the mesh no
longer caps it. What follows describes the old constraint.

The top cannot go past 191 km today. The aurora integrates along the atmosphere
chord, and that chord ends at the shell; the shell mesh is drawn at
`1 + atmosphere_scale * 2` planet radii, so there is no fragment above it to
draw emission on. The real red cap at 300 to 400 km is therefore unreachable
without enlarging the shell.

Enlarging it is CHEAP IN PHYSICS and expensive in blast radius. `shell_packing`
derives both `rp` and `h_rel` from the same scale, so the scattering model is
invariant to shell size to about one part in a billion: at 191 km with an 8.5 km
scale height the density is already `exp(-22)`, so the added chord is vacuum.
But `atmosphere_scale` is read in several places, the sun-transmittance tests
pin 0.015 by hand, and the atmosphere is the most heavily tuned thing in the
renderer. That is its own increment with its own measurement, not a passenger
on an aurora change.

## Known gap, deliberate

The sun-shadow cache bake and the profile bake in `45-cloud-temporal.wgsl` read
`material.base_color.a` directly and do NOT yet apply the region floor. A storm
therefore lights and self-shadows as though it carried the base coverage. It is
a second-order error against a first-order fix, and wiring it wants its own
measurement rather than being folded in unmeasured.

## The second consumer, and what it proved (2026-09-21)

The aurora was built next specifically because it shares nothing with a storm
except having a place and a size. It needed no new uniform channel, no new
binding and no change to the record: two regions of band 2, one per pole,
carrying the oval geometry in the per-kind payload.

That is the claim the mechanism was making, now tested once.

Three things worth keeping from building it:

**Integrate along the layer, not along whatever chord you already have.** The
aurora is emission inside the air, so the atmosphere fragment already had a
chord. Sampling that whole chord put most samples outside a thin emitting layer,
and worst of all for GRAZING rays, which are exactly the ones that should be
brightest. Solving the ray against the layer first and marching only that makes
every sample count, and limb brightening then falls out of the geometry rather
than needing a fudge.

**A ring is not a cap.** An auroral oval sits 20 to 25 degrees from the pole, so
it needs two angles. The disc falloff that `EnvRegion::influence` provides is the
wrong shape for it, which is why the aurora reads its ring angles out of the
payload and does its own test. That is the layer-3 idea working as intended: the
geometry primitive is shared, the interpretation is not.

**The polar case hides a frame bug.** The spin axis is the ONE direction that
reads the same in the body frame and in world space, because bodies spin about
+Y. The aurora therefore compares a body-frame region direction against
world-space sample directions and gets away with it. Any NON-polar region
consumed from the atmosphere shader would have to undo the spin first, and would
fail silently and subtly if it did not.

Cost note, and a trap in the tooling. The loop runs per atmosphere fragment
whether or not anything is emitting, because the band test is inside it. It is
pure ALU with no texture fetches and the ray-versus-layer test rejects early,
but its cost is genuinely UNMEASURED.

An A/B was attempted and the numbers were not real. **The `*-costs.json` a
plain `probe-sweep` run writes is not a reading.** Two runs of the same vantage,
one with the aurora emitting and one with its layer collapsed to zero
thickness, produced byte-identical numbers to full float precision, and so did
two COMPLETELY DIFFERENT vantages: a 30 km storm view and a 1500 km polar view
both reported `gpu.celestial_t` 25.276 and `frame_ms` 35.88. Those files carry
defaults or a stale snapshot unless the perf capture is armed
(`HUMANITY_FRAME_COSTS=1`, see the memory note on the cloud cost table).

Reading them naively yields "the aurora costs 0.00 ms, it is free", which is
a fabrication, and it is the same failure class as a gate whose evidence is
its own setup. Any future perf claim from a sweep has to come from an armed
capture, and is worth sanity-checking by confirming two different vantages
disagree before trusting either.

The honest follow-ups: an armed measurement, and an `override` switch so a
pipeline that cannot draw an aurora does not compile the branch at all, which
is what CLAUDE.md asks of any new heavyweight shader branch and which would
also give the A/B a clean off arm.

**Both follow-ups are closed by the pass below (2026-09-27), without a switch.**
No class entry calls `aurora_emission` any more, so no class pipeline compiles
it at all, and the pass has its own GPU timer (`gpu.aurora`) and its own off arm
(`showcase {"aurora":"0"}`).

## The aurora gets its own pass (2026-09-27, PRIORITIES 1b)

**The defect.** The aurora emits between 99 and 190 km and the cloud deck sits
near 12 km, so from above the deck is BEHIND the aurora and cannot occlude it.
It did. The fullscreen cloud composite runs after the celestial transparent
list (`atmo_over` in `engine/frame_shells.rs` is always false, deliberately and
correctly for scattered AIR), and the aurora was drawn inside that list, so the
deck painted over it. Cloud cover is regional, so the dimming wore a coastline:
the operator's "the aurora is darker over land masses" was the deck, measured
2026-09-22 as green-excess 4.288 over land and 2.198 over water with clouds on,
against 14.811 and 14.826 with them off.

**The first fix and what it left.** v0.1331.18 drew the atmosphere shell a
second time after the composite, carrying only the emission. That removed the
land/water split, but:

1. It blended OVER (the transparent pipeline's alpha blend), delivering
   `emission + dst * (1 - alpha)`, so a bright curtain erased the cloud or
   ground behind it. Light adds.
2. It drew the aurora last for every camera. From BELOW the deck the clouds are
   in front of the aurora, and "last" paints it through an overcast sky. (It
   never did in practice, only because of the separate bug below: nothing drew
   from below at all.)
3. It rasterised the shell mesh, so the layer could never pass 191 km.
4. Found while measuring, and the reason the clouds-OFF numbers moved: the OVER
   blend DROPPED every pixel fainter than half an 8-bit step. The emission rode
   in the alpha (colour / alpha, alpha = the brightest channel) and the blend
   unit rounds the source alpha to the 8-bit target, so a glow under 0.5/255 in
   linear light delivered nothing. Proven on one aligned pair (same heading,
   clouds off, `aurora-water-noclouds`): where the old draw wrote black, the new
   pass writes sRGB green codes 0 to 6 in 99.7 percent of 1.85 M pixels; where
   the old draw wrote anything, 7 and up (half a step, 0.00196 linear, falls
   between code 6 at 0.00182 and code 7 at 0.00213). A positive control ruled
   out coverage: with the old draw painting magenta wherever it ran and found
   no aurora, the band it normally left black was NOT magenta, so it ran there
   and computed an aurora above its own 0.0005 discard, and the blend then
   rounded that to nothing. That cut is the blotchy, speckle-edged diffuse glow
   in every capture of the old draw.

**The pass.** `fs_emission_pass` (`assets/shaders/pbr/95-emission-pass.wgsl`,
`src/renderer/emission_pass.rs`) is a fullscreen triangle with ANALYTIC rays,
colour blend One/One, alpha untouched. It reads the camera uniform and this
buffer through the SHARED camera bind group (group 0), so neither is copied,
and the scene depth plus a 96-byte uniform through its own group 1 (a new
layout, not a new binding in a shared one, so no existing bind group changed
shape). It calls the ONE `aurora_emission` in `30-atmosphere.wgsl`; a test pins
one definition and one call site. Each ray is clipped at the planet sphere
(analytic) and at the scene depth (terrain above the sphere, a moon, anything
opaque), with the reverse-Z linearisation the cloud composite uses, now shared
through `Camera::celestial_projection`.

**Where in the frame, and why the split is exact.** Altitude along a straight
ray has a single minimum. So a camera ABOVE the emitting layer's floor meets the
layer (its near crossing, which is what `aurora_emission` integrates for such a
camera) before any cloud: the pass runs LAST, after the composite. A camera
BELOW the floor reaches the layer only after crossing every cloud it will ever
meet: the pass runs BEFORE the celestial transparent list, so the dome
attenuates it by the air column in front of it (the dome's alpha is that
column's gray transmittance) and the deck covers it wherever the deck is in
front, whichever renderer draws the deck. The floor comes from the same region
rows the GPU reads (`emission_pass::emitting_layer_fractions`).

**Which body.** The region buffer holds one body's ovals per frame (the
cloud-bearing body in view). That body announces itself with an emission MARKER
in the celestial transparent list: the air shell's placement on a material whose
emissive lane is set. The transparent loop skips it; the pass reads its centre
and radius. The list is rebuilt every frame and handed to both callers of
`render_celestial_onto` (the live frame and the hi-res capture), so the marker
cannot go stale and a capture always matches its frame. The old twin was pushed
for EVERY body with an atmosphere, which would have drawn Earth's ovals around
Mars.

**Two defects found on the way, both fixed in the same increment.**

- The aurora was INVISIBLE from anywhere below 99 km. For a ray that dips inside
  the layer's floor, `aurora_emission` took only the inbound crossing, which for
  a camera below the floor lies entirely behind the eye. It now takes the
  outbound crossing when, and only when, the inbound one is wholly behind the
  camera, so every ray that drew before draws exactly as before.
- With the deck switched off the region buffer was never uploaded (the upload
  lived inside the deck branch), so a boot with clouds off had no aurora at all
  and one switched off mid-session kept a stale storm. The ovals are uploaded on
  their own when the deck is off.

**The gate, as numbers.** `scripts/aurora-gate.js` reads the committed
definition of green-excess (the 2026-09-22 figures were never committed as code;
this one reproduces them to 0.7 percent) and, given an aurora-OFF twin, the
light the aurora DELIVERS in linear light. Measured 2026-09-27 at operator
settings on the final build: delivered green 5.568 and 5.584 x1e-3 with the
deck on (land, water) against 5.558 and 5.564 with it off, equal to 0.4 percent.
Green-excess itself reads 1.7 and 3.9 percent lower with the deck on, and that
is the metric, not attenuation: the sRGB encode is concave, so the same light
laid over night cloud that is not quite black lifts fewer codes than over black
ground. The full table, the no-regression checks and the history are PRIORITIES
item 1b.

**Measuring it, and a trap.** The rig's heading changes between parks, even
within one boot, so two captures of one vantage are NOT pixel-aligned; a
cross-build pixel diff is meaningless. `showcase {"aurora":"0"}` flipped in
place, with nothing re-parked, gives aligned on/off pairs, and a third capture
with it back on is the noise floor. The same holds for a shader edit: swap the
part file under a running rig and the hot reload rebuilds every PSO, the
emission pass included, which is how the segment rejection below was proven
pixel-identical. And a rig boots its shaders FROM DISK
(`HUMANITY_SHADERS_FROM_DISK=1`): an old exe measured beside a working tree
renders the working tree's shaders, not its own. The first baseline of this
increment did exactly that and read "no aurora at all" for HEAD; the fix is a
rig whose `assets/shaders` is the old tree.

**Cost.** `gpu.aurora` at 2560x1387: 1.06 to 1.50 ms with the oval filling the
screen at nadir, 1.7 to 1.9 ms from the ground under it, 0.19 to 0.59 ms
oblique, 0.34 to 0.38 ms over a daylit planet, 0.06 ms at 12,000 km. The old
shell draw was untimed, so there is no before. Two things keep it down: a
cheap rejection against the top of the emitting layer, and a WHOLE-SEGMENT
rejection in `aurora_emission` that bounds a chord's angle from the pole and
from the sun by the triangle inequality and skips segments no sample could
survive (exact; cut the daylit cost about 23 percent). With the oval in view
the cost is the curtain shading itself; a half-resolution pass is the next
lever if it matters.

## Layer 1 as built (2026-09-27, PRIORITIES item 1 step 3)

Code: `src/systems/env_layer1.rs` (CPU, f64), `EnvClimate` and the `env_l1_*`
functions in `assets/shaders/pbr/00-bindings-vertex.wgsl` (GPU, f32), one row
per world in `data/environment/climate.ron`, and Earth's numbers reproducible
with `node scripts/climate-fit.js`.

**What it is.** The air at any place and date on a world with a row, as a pure
function of the unit direction from the body centre (body frame), the altitude,
the share of land around the place, and the fraction of the game year
(`GameTime::year_fraction`, the 120-day year). Nothing stored, nothing about
the camera.

- **Temperature at sea level**: North, Cahalan and Coakley's form (1981, Rev.
  Geophys. Space Phys. 19:91-121, equation 51 and Table 1): a P2 Legendre
  profile in sin(latitude) plus a seasonal term proportional to sin(latitude),
  which is zero at the equator and flips sign between the hemispheres, so the
  seasons flip. Earth's coefficients are fitted to the NCEP/NCAR Reanalysis 1
  (Kalnay et al. 1996) long-term monthly means for 1991-2020: the annual-mean
  profile from SEA cells (the ocean surface is sea level; land carries terrain
  height), the seasonal pair separately per hemisphere and per surface. North
  et al.'s published fit (14.9, -28.0, and -13.2 / -8.1 seasonally) agrees to
  within 1.6 C and sits between the fitted northern land and sea pairs, as it
  should for a hemisphere that is about 40 percent land.
- **Land versus sea**: the seasonal pair is blended by the share of land
  around the player, nine samples of the Earth ocean mask on a one-degree ring
  (`land_fraction_around`), because the reanalysis cells the pairs were fitted
  on are 1.9 degrees across. Northern land swings about 20 C either side of its
  mean at the pole-ward end, northern sea about 9; the Southern Ocean barely
  has seasons, which one symmetric fit cannot say.
- **Pressure and the lapse**: the 1976 US Standard Atmosphere's two lowest
  layers (6.5 K per geopotential km to 11 km, then isothermal), generalised so
  another world is a row, starting from the PLACE's sea-level temperature, so a
  cold polar column thins faster with height than a tropical one. With a 15 C
  sea level it is the standard atmosphere exactly (tested at 0, 1,500 and
  5,000 m against the tabulated values).
- **Prevailing wind**: the zonal-mean 10 m wind from the same reanalysis, annual
  mean plus its first annual harmonic, as a 16-term sine series in colatitude
  (area-weighted rms error 0.35 m/s east-west, 0.18 north-south). It holds the
  trades from the east-north-east and east-south-east meeting near 5 N, the
  westerlies at 47 N (weak, land drag) and 52 S (the strongest, the Roaring
  Forties), and the easterlies off Antarctica. The data's Arctic zonal mean is
  nearly calm at 10 m: the northern "polar easterlies" of the textbook picture
  are regional, not zonal, and the model says what the data says.

**Why a sine series and not a lookup table.** The GPU twin must not index an
array at run time: a function argument's array read with a variable index is
copied into per-invocation private memory, and that frame is charged to every
fragment of every pipeline that can reach the function (H1,
`docs/design/frame-cost-arc.md`). The series is read with constant indices only
(four vec4 per coefficient set), its basis comes from the Chebyshev recurrence
with no trigonometry (cos of the colatitude IS the direction's y), and every
term vanishes at the poles, where a wind has no single east component.

**The twin discipline.** The ocean wave twin (`terrain::ocean_waves`) compares
the shader's constants with the CPU's. This one compares their ANSWERS:
`env_layer1::tests::wgsl_twin_matches_the_cpu_model` parses the shipped
megashader with naga and runs `env_l1_air`, `env_l1_wind_en` and
`env_l1_wind_body` through a small IR interpreter
(`renderer::shader_loader::wgsl_eval`, test-only) over about 2,000 combinations
of latitude, longitude, altitude, land share and date, for Earth and Mars, and
builds the shader's `EnvClimate` from `ClimateRow::pack_gpu` using the shader's
OWN member offsets, so a lane swapped in the pack fails there too. Seen red
three ways (a swapped WGSL term, a swapped pack lane, a CPU-only change). It
proves the formula, not a particular GPU's rounding of `pow` or `sin`; the
tolerances absorb that.

**The weather on top.** `WeatherSystem` now ramps a temperature DEVIATION (the
condition's offset and its random spread) rather than an absolute. The global
`temperature` that farming and hydrology read is the body's reference climate
(Earth's calibrated seasonal table, unchanged) plus that deviation, and it now
follows the season the moment it turns over instead of at the next condition
change. The at-player values are Layer 1 plus the same deviation:

- `temperature_at_player`: Layer 1 at the player plus the deviation plus the
  body-wide day/night swing (zero on Earth, whose table stood in for it).
- `pressure_kpa_at_player`: Layer 1's column; Earth's standard column on a world
  with air but no row; 0 in space or on an airless body. Replaces the fixed
  barometric constant `survival_env` used to carry.
- `wind_east_at_player`, `wind_north_at_player`: Layer 1's prevailing wind plus
  the weather's own (whose rolled direction is read as east and north until it
  has a geographic frame). While the F11 weather panel drives, only the panel's
  wind: "calm" has to mean calm in the trades too.

`survival_env` feeds all of them to the body heat model, so a player at the
equator, on a mountain and at the pole feel different air; a test runs the
whole chain from the weather export to an hour of body heat and finds the
summit colder than the shore. The home station (no world under it) reads
exactly what it did before.

**Worlds.** Earth has a full row. Mars has its column (NASA Mars Fact Sheet,
updated 2025-05-19: 6.36 mb at mean radius, 214 K average, molecular weight
43.49, gravity 3.73; lapse rates from NASA Glenn's Mars atmosphere model), and
the two agree with each other: R T / (g M) at 214 K is the fact sheet's 11.0 km
scale height. Its latitude and seasonal terms and its winds are ZERO, not
invented: no zonal climatology was in hand. The Moon has no row on purpose; its
surface temperature is sunlight on regolith as a function of local solar time,
which needs per-longitude sun geometry. Worlds without a row keep the generic
body model in `body_environment.rs`.

**What is not modelled yet**, and where it goes:

- Local geography below the reanalysis scale: a rain shadow, a sea breeze, a
  valley's cold pool. Layer 1 is zonal plus land/sea; regional detail is layer
  2's job (or a finer climatology row).
- The weather's deviation is still body-wide. It should be weighted by its own
  region's influence at the player (`EnvRegion::influence`), so walking out of
  a storm takes its chill with it. The anchor sits under the player when the
  condition appears, so the two agree until the player travels hundreds of km.
- Mars by latitude and season, and the Moon (above).

**Next consumers**, in order: cloud advection (the first GPU caller:
`env_l1_wind_body` at the ray's ground point, once per ray, with `EnvClimate` as
one uniform the consumer adds: the v0.1029 every-create-site rule applies to
that binding); the sea state reading the wind at the player (it reads the
weather's own wind today); field crops and water bodies sampling Layer 1 at
THEIR positions instead of the global reference (farming and hydrology read the
global on purpose today, so a player's climb cannot chill a field; the right fix
is the field's own position, not the player's); fire spread, seed dispersal and
turbines reading the wind. Rain versus snow and the HUD readout were built
next: see the section below.

## Rain or snow, decided by the air (2026-09-27)

Code: `src/systems/precipitation.rs`. The first two CPU consumers of Layer 1
after the body heat model.

**What changed.** The condition still names the weather SYSTEM (its clouds, its
temperature deviation, whether it brings water and how hard: Rain, Storm and
Snow at their intensity). It no longer decides the PHASE. The air where the
water falls does: a Rain roll at 70 N in winter falls as snow, a Snow roll over
the equator falls as rain, and walking up a mountain carries the player from
rain through a mixed band into snow. Where there is no air (a station in orbit,
an airless world) nothing falls on the player, whatever the world below is
doing; before this, rain landed on a body standing in vacuum outside the hull.

**The source and the threshold.** Jennings, Winchell, Livneh and Molotch (2018),
"Spatial variation of the rain-snow temperature threshold across the Northern
Hemisphere", Nature Communications 9:1148 (doi:10.1038/s41467-018-03629-7, open
access, read 2026-09-27): 17.8 million observations at 11,924 stations,
1978-2007. Rain and snow fall with equal frequency at an air temperature
"averaging 1.0 °C and ranging from –0.4 to 2.4 °C for 95% of the stations", and
humidity moves it: from "0.7 °C in the 90–100% RH bin to 4.5 °C in the 40–50%
RH bin", because dry air cools a falling flake by evaporation (the wet-bulb
effect). We use their trivariate logistic model, p(snow) = 1 / (1 + exp(alpha +
beta T + gamma RH + lambda P)), with T in C, RH in percent and P in kPa, and the
coefficients from their Supplementary Table 2: alpha -12.80, beta 1.41, gamma
0.09, lambda 0.03. At 90 percent humidity and sea level the 50 percent point is
1.18 C, and the share runs from nine tenths snow to nine tenths rain over about
3 C: a smooth band, not a switch.

Two readings of ours, stated in the code: the paper fits how OFTEN snow falls
and we read it as the SHARE of what falls that is frozen (so the band is mixed
rain and snow rather than a coin toss); and the fit is Northern Hemisphere land
stations from 60 to 105 kPa, which we clamp to and apply over the sea and in the
south too.

**Every consumer, and what it now gets.**

| Consumer | Reads | Gets |
|---|---|---|
| Body heat input (`engine::survival_env`, `ExposedAir::from_weather`) | `Weather::falling_at_player` | rain at full weight, snow at a third, a mix by its parts; nothing where there is no air |
| Clothing wetness (`body_heat`, the wetting step) | the input above | the same number: it has no other source |
| HUD weather line (`gui::pages::hud::weather_line`) | the bridged condition and `Falling` | "Snow", "Rain", "Rain and snow" in the band, "Storm, snow", an event plus its phase |
| Weather fog (lib.rs, the aerial-haze floor and tint) | the bridged condition, `Weather::condition_at_player` | the snow floor (400 m) when snow falls here, the rain floor when rain does |
| Rain and snow particles (lib.rs precipitation block) | `Falling::emitters` | the DOMINANT phase: one GPU pool, so a mixed band draws the larger share |
| F11 panel readback | `weather_line` | an "At the player" line under the panel's own values |
| Outdoor fields (farming) | `Weather::falling_global().rain` | only the liquid share waters a field |
| Water bodies (hydrology) | `Weather::falling_global()` | snow adds its water to a lake or glacier; catchment runoff and aquifer seepage carry only the rain |
| Cloud region kind (`frame_shells.rs`) | the condition | unchanged: the rain and snow kinds carry identical params today, and the region is the weather system's, anchored where it appeared |

Farming and hydrology read the phase in the body-global reference air
(`temperature`, sea-level pressure), the same air the rest of their climate
reads, so a player's climb cannot freeze a field. Wherever fields exist today
(the home frame) that is the same air as the player's.

**Snow wets less than rain, and that is a game choice.** `body_heat` wets
clothing with snow at a third of rain at the same rate
(`SNOW_WETTING_SHARE`), unsourced: dry snow mostly sheds and melts in slowly
while it stays frozen. It does not follow wet snow near 0 C soaking harder; the
phase share moves part of the way, because inside the band part of what falls
is rain. A sourced wet-snow model is open.

**The HUD line.** From the at-player values the weather exports: the condition
as felt here, the temperature at the player, and the wind at the player (Layer
1's prevailing wind plus the weather's own) with the eight-point compass point
it blows FROM, or "calm" below 0.5 m/s. It used to print the weather's own wind,
which in the trades was not the wind the player stood in. No Settings choice
governs the line: `HudVitals` chooses the survival rows, and the weather line
was always shown.

**Not done, and where it goes.**

- Hydrology is not registered in the runner yet (FEATURES.md), and until this
  change it read a DataStore slot the weather never writes, so it always saw
  the default weather. It now reads the real slot.
- No snowpack: snow on a field or a catchment is neither banked nor melted
  later. That water is not counted anywhere.
- Mixed particles: drawing rain and snow together at their shares needs
  per-emitter rates in lib.rs's precipitation block and a second GPU pool.
- Snow on the ground: nothing accumulates on terrain or roofs.
