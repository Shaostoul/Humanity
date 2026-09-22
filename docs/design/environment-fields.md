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
   locked CPU and GPU twins.

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
