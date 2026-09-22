# Weather needs spatial extent

**Status:** designed 2026-09-21 from the operator's descent report. Not yet built.

## The report

> "As I descend the clouds become 100% thick. From high orbit they look okay
> but, as I get closer to the cloud layer it fills in slowly. The weird dark gray
> needs to be fixed too. Let's get this cloud layer working right because it is
> very immersion breaking for it to morph instead of remaining consistent in its
> positioning."

Four screenshots at 2770 km, 88.7 km, 71.4 km and 47.9 km. The cloud cover grows
steadily across the last three, and the HUD reads `Clear 20C` in the first and
`Storm -8C` in the rest.

## The cause, read from source

`src/engine/frame_shells.rs` turns the weather condition into two numbers and
then fades both by CAMERA ALTITUDE:

```rust
let (wx_floor, wx_bypass) = match w.condition {
    WC::Storm  => (0.95, 0.95),
    WC::Rain   => (0.85, 0.85),
    ...
};
let wx_fade = (1.0 - (h_km - 30.0) / 90.0).clamp(0.0, 1.0);
let (wx_floor, wx_bypass) = (wx_floor * wx_fade, wx_bypass * wx_fade);
```

`wx_floor` raises effective coverage. `wx_bypass` is `params2.w`, which the
shader reads as the fraction of the live MODIS placement to BYPASS toward the
procedural field. So between 120 km and 30 km the deck does two things at once:

1. Its coverage climbs toward the storm floor.
2. Its PLACEMENT cross-fades from one spatial pattern (MODIS satellite
   geography) to a completely different one (the procedural field).

Point 2 is the operator's actual complaint. The clouds are not drifting or
being re-lit; they are being cross-faded between two unrelated layouts as a
function of how high the camera is. That is why they "morph instead of remaining
consistent in positioning", and it is why the effect is tied to descending
rather than to time.

The altitude fade is not a mistake. Its own comment records why it exists:

> "the sim's weather is a POINT sample at the player, so its floor fades out
> with altitude: from high orbit the operator watched a local Storm paint the
> ENTIRE planet 95% white (v0.1183 report)."

So the ramp was the fix for an earlier report by the same person. It traded a
planet-wide artifact for a descent artifact.

## The real root: the weather model has no space in it

`src/systems/weather.rs` carries ONE condition for the whole body:

```rust
pub struct Weather {
    pub condition: WeatherCondition,
    pub intensity: f32,
    ...
}
```

There is no latitude, no longitude, no extent. "Storm" is not a storm somewhere,
it is a global fact about the planet. Anything that applies a global fact to a
visible hemisphere paints the whole hemisphere, so the only lever left was to
turn the global fact down as the visible area grows, which is exactly what
`wx_fade` does.

Every artifact here follows from that missing dimension, and no amount of tuning
the ramp removes them, because the ramp is standing in for a spatial term that
does not exist:

- Apply the global condition at full strength everywhere and a local storm
  whitens the planet (v0.1183).
- Fade it by altitude and the sky changes as you climb or descend (this report).
- Fade it by anything else camera-derived and you get the same class of bug with
  a different trigger.

A separate, correct behaviour that is easy to mistake for a bug: the HUD
temperature fell from 20C to -8C during the descent. That is
`temperature_at_player`, which carries a real altitude lapse, and it is working
as designed. The condition changing from Clear to Storm is not altitude-driven
at all: the global condition simply rolled over during the minutes of flight,
which is its own argument for giving weather a place to be.

## The fix: give a weather system a position and a size

Roll a condition WITH an anchor, and let the deck weight the condition by
distance from that anchor instead of by camera altitude.

1. `Weather` gains `anchor_lat`, `anchor_lon` and `radius_km`, chosen when the
   condition rolls. Clear carries no anchor. The anchor is placed near the
   player so weather is something you actually experience, and then it STAYS
   PUT: it is a feature of the world, not of the viewer.
2. The condition table moves out of the `match` in `frame_shells.rs` and into a
   data file keyed by condition, carrying floor, bypass and a default radius.
   A hardcoded list of domain objects is exactly what `docs/design/infinite-of-x.md`
   forbids, and per-condition radius is content, not code.
3. `frame_shells.rs` passes the anchor and radius to the shader in place of
   `wx_fade`.
4. The shader weights floor and bypass by a smooth falloff in great-circle
   distance from the anchor to the SAMPLE's ground point, not the camera's.
5. Consumers that ask "what is the weather" get the condition when the player is
   inside the anchor and Clear otherwise, so the HUD and the sky agree by
   construction rather than by a floor that was bolted on to make them agree.
6. `wx_fade` is deleted. Nothing in the deck should read camera altitude.

What this buys:

- From orbit a storm is a storm-sized patch where the storm is. The v0.1183
  artifact cannot recur, without an altitude ramp.
- Descending changes nothing about placement or coverage. The morph is gone,
  because altitude is no longer an input.
- Standing under it, coverage is full overhead and the HUD agrees.

Real mid-latitude systems run 500 to 1500 km across, so a few hundred km radius
with a smooth edge is the physically honest default, and being data-driven it is
tunable without a rebuild.

## Not covered here

The "weird dark gray" is a SEPARATE defect and already bisected: see
`docs/PRIORITIES.md` TIER 0 item 1 and BUG-079. Cloud coverage alpha and ambient
luminance both render clean; the direct-sun luminance carries per-pixel grain,
which is what makes a cloud read as dirty grey rather than white. Fixing the
spatial model will not fix that, and fixing that will not fix this.

## Related

- `docs/design/cloud-far-rung.md`: the coverage-vs-distance work, a third and
  also separate defect (Ultra renders 0.9 percent coverage where High renders 31).
- Fixtures: `covladder-*` pins coverage and type so only the RENDERER varies, and
  `stormfade-*` (added 2026-09-21) pins a storm and leaves coverage unpinned so
  the altitude ramp applies. Together they separate the two.

## The implementation blocker, found 2026-09-21

The shader needs ONE new per-frame scalar: the global condition strength, which
it then multiplies by a planet-fixed mask. There is no free channel to carry it.

- `material.params2` is full on the cloud material: `.x`/`.y` are the slab bounds
  as planet-radius multiples, `.z` is the planet radius in km, `.w` is the pin.
- The light pads look free at a glance and are not. Grepping for
  `camera.lightN_color.x` misses `.xyz` SWIZZLES: `light3_color.xyz` is the sun
  cache anchor (`40-clouds.wgsl`) and `.w` its cell height, and
  `45-cloud-temporal.wgsl` states that `light4.xyz` and `light5/6/7.xyz` are
  owned by the octa pass's reprojection. Any inventory of free pads has to
  search swizzles or it will report slots that are in use.

So carrying the scalar means extending the camera uniform struct, which is
declared in every shader that binds it (`pbr/00-bindings-vertex.wgsl`,
`galaxy_glow.wgsl`, `stars.wgsl`, `star_halo.wgsl`, plus the Rust mirrors in
`renderer/line.rs` and `renderer/stars.rs`). That is the change class the
v0.1029 incident rule in CLAUDE.md is about: update every site or world entry
panics, and static verification will not catch it.

It is perfectly doable, it is just not a change to make in a hurry at the end of
a session. Whoever picks it up: do the uniform extension as its own commit with
every declaration site updated and a world-entry probe boot, THEN the mask, THEN
delete `wx_fade`. The fixtures below hold the defect red the whole way.

## How to prove it fixed

`stormfade-*` measured on 2026-09-21, same column, storm pinned, only altitude
varying (`node scripts/cloud-fraction.js <png>`, mean luminance in the centre
crop):

| altitude | mean luminance | predicted `wx_fade` |
| --- | --- | --- |
| 200 km | 72.3 | 0.00 |
| 120 km | 63.3 | 0.00 |
| 90 km | 62.3 | 0.33 |
| 60 km | 61.5 | 0.67 |
| 30 km | 203.2 | 1.00 |
| 10 km | 200.8 | 1.00 |

At 60 km the HUD reads Storm over an empty ocean. At 30 km the frame is an
edge-to-edge cloud carpet, matching the operator's own 47.9 km screenshot. A fix
is correct when that column is FLAT and the placement does not move between
rungs. The `covladder-*` fixtures pin coverage and type, which bypasses the
weather path entirely, and they are already flat: that is what proves the
renderer is not the thing at fault here.
