# HumanityOS: Priorities

> **This is the TACTICAL backlog: what is next, right now.** The TOP item of
> TIER 0 is what gets worked on next. If you are picking up work without
> context, read this file first, then
> `data/coordination/orchestrator_state.json` for WHY we got here.
>
> Its strategic, themed, public-facing companion is
> **[ROADMAP.md](ROADMAP.md)** (the same to-do list grouped by theme with
> status badges, rendered on the website from `data/roadmap.json`). Use
> ROADMAP.md for "where are we going"; use this file for "what is the very
> next thing." Keep the two consistent, and regenerate the JSON
> (`node scripts/roadmap-to-json.js`) in the same commit as any ROADMAP edit.
>
> **Update rule:** every session that meaningfully changes scope updates this
> file before ending. Record WHAT COMES NEXT here and WHY in the journal. When
> an arc finishes, retire it: move the block to `docs/history/` with its
> reasoning intact rather than leaving it here looking pending. A shipped item
> still marked open is the most expensive defect this file can carry, because
> the next session rebuilds it.
>
> **Keep it short.** A backlog nobody finishes is a backlog that does not route
> work. Detail belongs in the design doc for that arc; this file carries the
> decision and the pointer.
>
> If a tool or a doc points you at "Active focus" (`just brief` still does), it
> means **TIER 0** below: that section was renamed on 2026-09-20 when the dated
> status blocks were retired.
>
> Current at **v0.1326.1, 2026-09-20**. Retired blocks live in
> `docs/history/priorities-archive-2026-08-to-09.md` and
> `docs/history/priorities-archive-2026-05-to-08.md`.

---

## TIER 0: the next thing to work on

Strict rank. Take the top item that is not marked CLAIMED. Everything below
TIER 0 is real work that has not been ranked against these four; do not promote
anything into this list without the operator.

### 1. FIXED in v0.1330.0. Weather is a place now, not a global (BUG-080)

Kept at the top for one more pass because the arc it opened is not finished.

The operator's "it morphs instead of remaining consistent in its positioning"
is closed. `wx_fade` is deleted, nothing in the cloud path reads camera altitude,
and the weather condition rides an environment region with a position and a
radius. Measured on `stormfade-*`: the descent column is flat within 1 percent
(200.6 / 201.9 / 202.5 / 202.5 / 202.7 / 201.0 at 200 / 120 / 90 / 60 / 30 /
10 km) against a 3.3x cliff before, and 12,000 km is unchanged at 22.1 so the
marble is not whitened. See BUG-080 and `docs/design/environment-fields.md`.

**What remains on this arc, in order:**

1. ~~**Aurora**~~ BUILT in v0.1331.0. Two ovals, one per pole, drawn as emission
   inside the air with the ray solved against the emitting layer so limb
   brightening falls out of the geometry. It was the second consumer of the
   region buffer and it needed no new channel, no new binding and no change to
   the record, which is the claim the mechanism was making. Fixtures
   `aurora-polar-1500` and `aurora-orbit-5000`. Follow-ups: an `override`
   switch so a pipeline that cannot draw it does not compile the branch at all,
   and the A/B cost measurement that switch makes possible.
2. **Disasters through the buffer.** `disasters.rs` already stores position,
   radius and intensity and nothing outside that file reads them, so a wildfire
   is invisible. Needs a consumer to be worth anything, which is why it now
   ranks below aurora rather than above it.
3. **The bake gap.** The sun-shadow cache and profile bakes in
   `45-cloud-temporal.wgsl` read the base coverage and do not apply the region
   floor, so a storm lights and self-shadows as though it were not there. A
   second-order error against a first-order fix; wants its own measurement.
4. **Layer 1**: temperature, pressure and wind as analytic fields with locked
   CPU and GPU twins. This is what makes the rest of the inventory in the design
   doc (humidity, fog, snow line, outbreaks, pollution, light pollution, ocean
   currents) reachable.

**Do not re-propose:** a weight that reads camera altitude, distance, or how
much of the planet is on screen. That is the defect class, not a tuning knob.
### 1c. The shore glow: RESOLVED 2026-09-24 as BUG-081 (v0.1331.20)

**Read BUGS.md BUG-081 first.** Everything below this line is the history of
measuring the wrong thing: every fixture here had a SUNLIT shore somewhere in
frame, and the defect only exists where the sun is fully down. The fixture
that reproduces it is `coast-night-2000` (local midnight over the Bahamas bank,
clouds off), with `coast-noon-2000` as its day-side control.

#### History: NOT reproducible at nadir, and v0.1331.8 did not regress it

Two negative results, both worth more than they look, because each closes off a
direction that would otherwise be re-walked.

**The sunrise ladder is clean.** `shore-dawn-*` steps one fixed coastline (the
Bahamas bank, the largest bright shallow shelf on the planet) through local
solar hours 5.0, 6.0, 6.2, 6.4, 6.6, 6.8, 7.0 and 9.0 at nadir from 400 km.
Measured on a tight central crop, which matters because a looser one counts the
chat overlay as cloud and reported a bright "cloud" on a frame of mean
luminance 1.4:

| local hour | frame | shore | cloud | deep ocean |
| --- | --- | --- | --- | --- |
| 6.2 | 4.7 | - | 44.1 | - |
| 6.4 | 14.4 | - | 47.3 | 11.3 |
| 6.6 | 33.1 | 19.4 | 56.3 | 11.2 |
| 6.8 | 57.1 | 21.6 | 92.2 | - |
| 7.0 | 80.5 | 26.8 | 130.3 | - |
| 9.0 | 128.9 | 48.6 | 206.7 | - |

The onset is SMOOTH across the terminator crossing (the fine rungs exist
precisely because the coarse ladder jumped 06:00 to 07:00 and hid it), the
shore is always brighter than deep ocean and always far darker than cloud. No
defect is visible at this geometry, so whatever the operator is seeing needs a
GRAZING view, where water Fresnel reflectance approaches unity and the sky
mirror dominates. Build the next fixture there, not at nadir.

**And the night-side rise was not a regression.** `orbit-terminator-3000km`
read 1.94/2.86/3.68 after the v0.1331.3 coast fix and 2.81/3.72/4.53 after
v0.1331.8, which looks like the coast glow coming back. It is not. The control
`orbit-terminator-3000km-noms`, identical but with `cloud_ms` forced off,
returns **1.96/2.87/3.70**, matching the post-fix baseline to two decimals.

So the entire rise is the multiple-scattering default lighting NIGHT-SIDE
CLOUD, which is physically correct: cloud at altitude stays sunlit after the
ground beneath it is dark, which is why sunset clouds glow. The coast fix is
intact.

That control is worth keeping for a general reason. v0.1331.8 changed cloud
brightness globally, so EVERY night-side or terminator metric taken before it
shifted, and any of them could be misread as a regression of something else.
When a baseline moves after a global lighting change, reproduce the old
configuration before believing the delta belongs to the thing you are looking
at.

Still unreproduced, and genuinely open: the shores glowing at a grazing view
when the sun is visible, and the clouds glistening at the dusk line.

### 1b. THE CLOUD DECK ERASES THE AURORA. Root cause, measured 2026-09-22

The operator asked three times why the aurora looked dark, and twice why it
differed over land and water. Both are the same defect and it is not in the
aurora at all.

The aurora emits between 99 and 190 km. The cloud deck sits at about 12 km. So
from above, the clouds are BEHIND the aurora and cannot occlude it. They do.

Measured at `aurora-over-land` and `aurora-over-water` (lat 67, nadir, 600 km,
local midnight, differing ONLY in longitude), as mean green-excess per pixel,
which is threshold free:

| arm | clouds ON | clouds OFF |
| --- | --- | --- |
| over Siberian land | 4.288 | **14.811** |
| over the Greenland Sea | 2.198 | **14.826** |

With the deck off the two are IDENTICAL to three digits, and about 3.4x
brighter than the land arm with it on. So:

- The land-versus-water difference is entirely the deck. Cloud cover is
  regional, so a cloud-cover difference wears a coastline and reads as a
  surface effect. The operator saw something real and named it by the nearest
  visible landmark, which is exactly what a good bug report does.
- The aurora is being dimmed 3.4x over land and 6.7x over water. That is the
  "kind of dark" as well, not only the land/water split.

Both confounds that could have faked this were removed before believing it.
The strand presence masks are functions of phi, so longitude changes how much
aurora the oval carries by design; the comparison was repeated with the ring
FLATTENED to uniform and the gap survived (4.288 against 2.198). And the first
version of the fixture aimed at the horizon, which filled the frame with lit
limb and let a green-dominance metric count vegetation; it now aims at nadir
and `scripts/aurora-comb.js` refuses any frame bright enough for that mistake.

**The mechanism.** `atmo_over` in `frame_shells.rs` is ALWAYS false, so the
fullscreen cloud composite always runs AFTER the transparent list that carries
the atmosphere shell. That was deliberate and is right for AIR: the cloud march
already applies this engine aerial perspective at the cloud first-hit distance,
so letting the dome blend over the deck applied the same air twice and the
second application was opaque ("the clouds just vanish", measured at 9,500 km:
1.2 percent of the disc written with the old order against 99.9 with this one).

It is wrong for EMISSION that lives above the deck. "Clouds last" is correct
for scattered air in front of them and incorrect for light sources behind the
camera-facing side of them, and the aurora is the first such source the engine
has had. Nothing was wrong when that ordering was chosen.

**The fix is an increment, not a patch, which is why it is fenced here rather
than attempted.** The aurora has to be applied AFTER the cloud composite. Two
ways, both needing the same new plumbing (the camera uniform and the
`env_regions` storage buffer reaching a pass that has neither today):

1. Add the emission inside `cloud_composite.wgsl`, which already runs last.
   Fewest passes, but that file is self-contained and would end up holding a
   SECOND copy of `aurora_emission`, which this repo has been bitten by before
   (a duplicated shader body drifts and no test compares them).
2. A dedicated fullscreen ADDITIVE pass after the composite. One more pass,
   but the aurora stays in one place and the pass is a natural home for any
   future above-deck emission (airglow, lightning, city light bloom).

Option 2 is preferred for exactly the reason option 1 is tempting. Whichever
is taken, gate it on the pair above: with clouds ON the two arms must come
within a few percent of the 14.8 that clouds-OFF already reaches.

**Do not flip `atmo_over`.** It would restore the erased-clouds regression that
the comment at `frame_shells.rs` documents, and that one cost a dozen
investigations because the cliff sat at the chunk-activation altitude.

### 2-0. Continent sheets and "grey closer" (operator, 2026-09-24). Sheets increments 1 and 2 SHIPPED (v0.1333.0, v0.1334.0)

The operator: clouds are "huge sheets that sometimes cover entire continents,
like Asia. Do we need to increase the base resolution of the base layer?" and
"when I'm far from the planet the clouds look more white but, as I get closer
they become noticeably gray." A fidelity review (read-only agent, 2026-09-24)
diagnosed both; every claim below that matters was then MEASURED, and several
of its magnitudes did not survive, so read the measured column, not the review.

**Base resolution is the wrong lever.** It was raised twice (3 to 5 octaves on
2026-07-17; the Medium base in v0.999) and the complaint returned both times.
Higher frequency makes more, smaller saturated blobs.

**Sheets, increment 1 (SHIPPED v0.1333.0).** The High path's placement now goes
through `cloud_weather_window` (CLOUD_WEATHER_EDGE 1.0) instead of the 0.30
window with dense-edge sharpening, for the procedural part only (live weather
keeps its calibrated window via `cloud_weather_alpha`). Measured on the sphere
with the shader's own noise: saturated share 40% to 12.5%, median saturated run
1,260 km to 834 km, longest ~6,000 km to 3,760 km, mean coverage 0.52 to 0.475,
and the coverage knob still linear. In the rig the same masses now break into
irregular clusters with gaps instead of smooth-edged solid blobs. Guarded by
`procedural_placement_is_a_fraction_not_a_sheet`, which also asserts the OLD
window fails it.

**Sheets, what is left, in order** (the review's plan; section (b) of its
report is summarised here because it is the build order):
1. **Synoptic organisation. SHIPPED v0.1334.0** as `cloud_synoptic_warp`: 5 storms
   per hemisphere at 35-65 degrees, twist 2-4 rad, radius 800-1,500 km, tapered to
   exactly zero at 3.5 R, plus a jet-shear bump at +-40 degrees; applied to the
   procedural placement and the type coordinate, mirrored in Rust, guarded by
   `synoptic_warp_is_a_measure_preserving_rotation` (unit length, sphere mean
   unchanged, and over a quarter of directions actually moved). Result: the
   isotropic blobs became curved frontal bands and arcs. NOT yet tight comma
   heads: the 1,300 km macro octave is the size of the storm radius, so it bends
   rather than winds. More twist and a smaller radius is a two-constant change;
   ask the operator before winding it tighter. Cost +0.3 to +0.55 ms of
   cloud_screen (6-10%). Original plan text follows.
   A `cloud_synoptic_warp(dir, t, seed)` at the top of
   `cloud_weather_adv` and `cloud_type_coord`: latitude-dependent jet shear plus
   4 to 6 cyclonic twists per hemisphere at 35 to 65 degrees (rotate `dir` about
   the storm centre by theta * exp(-(r/R)^2), R 800-1,500 km, theta 2-4 rad,
   counter-clockwise north, clockwise south, drifting east), 0 to 2 tropical
   cyclones, and a ridged term for frontal bands. A twist about an axis is
   area-preserving, so coverage calibration is untouched. This is what makes
   comma clouds and fronts instead of isotropic blobs, and it is the next
   visible step.
2. **Bake the weather field** into the existing `weather_map` texture at low
   cadence, so richer structure costs nothing at march time.
3. **Climatology and type mix.** The type coordinate clusters around 0.5, the
   cumulonimbus centre, so about 38% of the planet is drawn as cumulonimbus
   against about 1% deep convection on Earth; stratocumulus, the commonest real
   type, is about 0.3%. Equalise the coordinate and retune the centres, or
   better, derive type from the storm structure. The Rust `cloud_regime` mirror
   and its tests change in step.
4. **Cellular texture** (Worley 15-40 km cells for flat decks). Measure the deck's
   column optical depth first: texture only shows below roughly 30.

**The ball pit (operator, 2026-09-25). FIXED on High in v0.1335.0.** He runs
`cloud_quality` High, and nearly all the earlier anti-ball-pit work (built
bodies, SDF shaping, v0.1230) is Ultra-only: High draws the noise body. The
1.33 km Worley cell split (`CLOUD_CELL_SPLIT`) divided every mass into equal
balls; bisected at 55 km (fixtures `deck-55-*`), it alone was the ball pit.
Now 0.05 (from 0.15): masses stay merged and the cells survive as cauliflower
texture. Rejected on evidence, so do not retry: fully off (cotton blobs),
gating it to the upper band (visible tops sit low, so it vanished), moving
the cell texture into the water term alone (barely visible). Still open from
the same review, measure first: the powder term runs at full strength on High
at a noon down-look (removed for Ultra already); crevice darkening
`CLOUD_PUFF_AO`; the per-family `reg.tint` applied from above; built bodies
as the High default (environment-program increment 16, never done).

**Repeating shapes (operator, 2026-09-25). FIXED in v0.1337.0** with a fixed
rotation per tiled noise tap, mirrored and pinned in Rust. Confirmed first at
the planet-frame equator (fixture `deck-55-equator`): straight north-south
rows every 8 km (the cell tile), 23x the autocorrelation floor. After: no
straight rows; a 20 km oblique near-repeat remains (rotation lengthens the
repeat, it cannot remove periodicity). If a repeat is ever seen again, the
next rung is stochastic (hex) tiling on that tap.

**Grey from orbit (2026-09-25). Multiple-scattering gain 1.0 -> 1.8 in
v0.1337.0**, calibrated to physical reflectance against the Sahara in the same
frame (thick cloud p50 199 -> 223 against sand 181). The earlier unit gain was
matched to the Low tier, which is not a physical reference.

**Found in passing, not yet worked:** the ocean sun glint seen from 55 km is
crossed by parallel diagonal stripes about 2 km apart, a straight repeating
pattern in the WATER that may be part of what the operator reported. Cause
NOT yet checked; one candidate is that the ocean chop trains are axis-aligned
by design (CLAUDE.md, the water arc), another is sub-pixel wave detail
aliasing into a moire in the glint. Also rectangular blocks in the
open-ocean colour at the same range. Fixture `deck-55-nadir` shows both.

**Grey: what was measured.** Thick sunlit decks are NOT dark: forced overcast
from orbit reads 199 to 234, in range of the review's own 215 to 225 target.
Cloud median RISES as altitude falls on both ladders tried (broken coverage: 58,
147, 182, 189, 202 at 20,000 to 300 km). What reads grey is partial cloud over
dark ocean at mass edges plus the crumb texture inside the masses, filling the
screen close in. The review's three darkening causes, as measured:

| cause | review predicted | measured |
| --- | --- | --- |
| step-economy floor lights the first sample deep inside the top | 2 to 3.5x at 400 km | 1.02x at 400 km, 1.10x at 2,000 km |
| `reg.tint` darkens cumulonimbus tops | Cb 0.56x of Cu | 0.97x at 400 km, 0.95x at 2,000 km |
| powder term on the sunlit skin | 35% darker | not measured (no dev toggle) |

Queued, small: light each step at its median scattering point (the review's
fix for the economy, about 10% at 2,000 km); gate `reg.tint` off when multiple
scattering is on, mirroring the existing v0.909 switch. Measure the powder term
before raising it with the operator: the review's replica overstated the other
two by 3x or more.

Fixtures: `cloudgrey-*` (the broken-coverage ladder), `decklum-*-eco0/1` and
`decklum-*-cb`, each carrying its measured result in its own `desc`.

### 2. THE CLOUDS ARE TOO DARK. The static is its symptom, not the defect

**Rewritten 2026-09-22 after measuring the thing nobody had measured.** Seven
hypotheses had been refuted, all of them about NOISE. The framing was wrong.
Read this before spending anything on the grain.

At `approach-2000km-high` (lat 23, lon 13, NOON over the Sahara), measured off
the captures by mean luminance, low-saturation pixels against warm pixels:

| tier | cloud L | terrain L | cloud / terrain |
| --- | --- | --- | --- |
| **High, the DEFAULT tier** | 120.1 | 175.0 | **0.69** |
| Low | 189.2 | 175.9 | **1.08** |

Same camera, same sun, same clock. The terrain luminance is the control and it
is unchanged (175.0 against 175.9), so the tiers differ in the CLOUDS alone.

High renders a daylit cloud deck 31 percent DARKER than the desert under it.
That is wrong on physics and needs no reference render to say so: cloud albedo
is about 0.7 to 0.9, Sahara sand about 0.35, so cloud must come out BRIGHTER
than desert. It does on Low (1.08) and does not on the tier every user gets.

**This reframes the whole arc.** The operator described it in v0.1252 as
"awfully dark and white in spots to the point of looking like old TV static",
and the DARK half of his sentence was read as a description of noise rather
than as a second symptom. A direct-sun term that mostly fails and occasionally
succeeds produces exactly this: a dark deck with bright speckles. The bisect
that found channel 2 (direct-sun luminance) GRAINY was right about the channel
and probably wrong about the reading. Grainy is what a mostly-missing term
looks like.

It also explains what the noise work could not. Low is not merely cleaner, it
is BRIGHTER, and no amount of filtering turns a dark deck into a lit one.
Every upstream knob tried moved the grain by 9 to 14 percent because the grain
is a second-order symptom of the missing energy.

**The next question is therefore about brightness, not noise.** Why does the
High/Ultra screen march lose about 37 percent of the cloud radiance Low
produces at the same camera? Candidates, in order of how cheaply they can be
told apart:

1. The direct-sun term is being attenuated or dropped on the screen-march path
   in a way the direct shell path (Low) does not do. The two arms are shaded
   separately (`full` / arm A and the profile arm B in `cloud_march_core`), and
   their blend is the obvious place for energy to go missing.
2. `tau_sun` is systematically too LARGE, over-shadowing the deck. The cone
   spread control is suggestive: widening the cone to 2.50 brightened nothing,
   it DARKENED the frame further (mean L 128.8 to 119.9), so more occlusion
   sampling means more darkening, and the deck is already over-occluded.
3. Multiple scattering. A real deck is bright because photons bounce inside it
   many times; a single-scattering march with Beer-Lambert extinction and no
   multi-scatter compensation is DARK by construction. `g_ms_on` exists and is
   bit-gated; check whether it is on at this range.

Measure with the cloud-versus-terrain luminance ratio above, not with the
speckle census. The target is a ratio above 1.0 at noon.

### 2a. THE GRAIN LIVES AT THE TERMINATOR, and every measurement of it was taken at noon

Measured 2026-09-22 on `orbit-terminator-3000km`, in ten vertical bands across
the terminator, as mean absolute deviation from the four-neighbour mean over
the band mean:

| band | mean L | speckle |
| --- | --- | --- |
| 0, full daylight | 169.5 | **0.55%** |
| 1 | 167.8 | 4.04% |
| 2 | 146.2 | 11.79% |
| 3 | 86.0 | 15.54% |
| 4 | 47.5 | 27.45% |
| 5, the dusk line | 22.5 | **40.43%** |
| 6, night | 12.9 | 4.13% |
| 7, night | 18.1 | 4.66% |

The grain is **73x worse at the terminator than in full daylight**, and it
collapses again on the night side where nothing is directly lit. The operator
said it in one line without any instrument: the clouds are "glistening ... most
noticeable at the dusk line".

**This invalidates the vantage every earlier speckle number was taken at.**
Item 2b below, and all of the 2026-09-22 arms (temporal accumulation off, the
sun-cone azimuth, the sun ladder on the grid, the gate recalibration, the
forced spatial filter, and the multiple-scattering result), were measured at
`approach-2000km-high`, which is NOON over the Sahara: band 0, where the defect
is at its weakest. A baseline of 1.76 percent was being quoted for a defect
that reaches 40.

Nothing measured there is WRONG, and the multiple-scattering finding in
particular stands on its own (it was a brightness result with a terrain
control, not a grain result). But every RANKING of grain fixes done at noon is
a ranking in the easiest regime, and a fix that halves the noise at band 0 may
do nothing at band 5. Re-run the surviving candidates at the terminator before
trusting their order.

**Why the terminator, mechanically.** The channel bisect blamed the direct-sun
term, and the direct-sun term is `exp(-tau_sun)`. At a grazing sun the optical
path through the deck is at its longest and its most variable, so the
exponential is at its most nonlinear there: the same per-pixel sample
displacement that barely moves the result at noon swings it hard at dusk. The
defect is not uniform and never was; it is concentrated exactly where the
mathematics says the sensitivity peaks. That is a strong hint that the fix
belongs in how `tau_sun` is sampled at grazing angles rather than in any
downstream filter.

### 2a-ii. THE FULL CANDIDATE TABLE at the terminator, and a flaw in the metric

Every grain candidate, measured at `orbit-terminator-3000km` band 5 (the dusk
line, where the defect is 73x its noon value), with fizz from differencing two
settled captures in one boot:

| arm | grain at band 5 | fizz |
| --- | --- | --- |
| shipped baseline | 40.38% | 0.34% |
| sun ladder on the unjittered grid | 40.22% | - |
| spatial strength 0.35 to 0.85, gate untouched | 40.22% | - |
| spatial gate widened, strength untouched | 37.54% | 0.27% |
| **animated depth jitter** | **8.68%** | 2.09% |
| spatial filter forced to full (`cur_s = mu`) | 6.07% | 0.12% |

**READ THE LAST ROW WITH CARE, because the metric flatters it.** Grain here is
the mean absolute deviation from the four-neighbour mean, and forcing the
spatial filter to full replaces every pixel WITH the neighbourhood mean. That
is a 3x3 box blur, and a box blur minimises exactly this metric by
construction. Some of that 6.07 is the measurement rewarding blur rather than a
better picture, and how much is not known.

The animated-jitter row does not have that problem: temporal averaging removes
noise without blurring, so 8.68 is an honest number and is the best HONEST
result on the table.

**What the table proves regardless of that caveat.** Neither the gate nor the
strength alone gets anywhere: 37.54 and 40.22 against a 40.38 baseline. Only
the forced arm, which bypasses `noise_w` entirely AND uses strength 1.0, moves
the number, so both are binding together and neither is a one-constant fix.
This CORRECTS the v0.1331.14 note, which inferred from noon data that the
strength cap was the binding constraint; at the terminator it is not.

**Why the gate stays shut in the dark.** Its absolute arm is
`smoothstep(0.10, 0.30, sig)` in raw march-buffer units, and `sig` measured
0.0185 at NOON against that 0.25 threshold. The terminator is darker, so `sig`
is smaller still: the gate closes hardest exactly where the relative grain is
worst. Widening it to 0.02-0.10 was tried and bought only 40.38 to 37.54, so
the threshold units are part of the story but not all of it.

**Before the next attempt, fix the metric.** A grain number that a blur can
win is not a grain number. Judge spatial arms on a measure a blur cannot game,
for example the high-frequency energy retained in genuine cloud EDGES alongside
the noise reduction, or an explicit sharpness term, and keep the by-eye check
that the silhouettes and the terrain survive. Until that exists, prefer the
temporal arm, whose number is not gameable in this way.

### 2a-i. THE OPERATOR DECISION: frozen jitter costs 4.6x more grain at the dusk line

This is his call, not the AI’s, because he ran the original experiment himself
and chose the current setting. What is new is the price tag, which nobody had
measured when he chose.

Measured at `orbit-terminator-3000km`, band 5 (the dusk line), with the fizz
taken by differencing two settled captures of the same camera in one boot
(`orbit-terminator-3000km-b` exists for exactly that):

| jitter | grain at the dusk line | fizz between settled frames |
| --- | --- | --- |
| **frozen** (shipped, v0.1253.2) | **40.38%** | **0.34%** |
| animated per frame | **8.68%** | 2.09% |

Animating the depth jitter cuts the grain 4.6x and raises the fizz 6x. Neither
number existed in v0.1253.2: that decision was made on "temporal ON vs OFF
changed only soft static vs sharp static", which was measured at a close camera
and not at the terminator where the defect actually lives.

**Why it works, and why it could not have worked before.** The accumulator
averages history toward the current frame and is deep at rest (alpha 0.09 on
88.8 percent of speckled pixels, measured with a diagnostic build). Averaging
only removes noise that CHANGES between frames, and frozen jitters make a
parked frame pixel-identical to the last, so a correctly functioning filter was
averaging identical values and removing nothing. Animating the jitter gives it
something to do, and at the terminator, where per-sample variance is enormous,
that is worth 4.6x.

**And it inverts the ranking of every other candidate.** The same two arms
measured at noon and at the dusk line:

| candidate | at noon | at the dusk line |
| --- | --- | --- |
| sun ladder on the unjittered grid | 1.80 to 1.54 (14% better) | 40.38 to 40.22 (**0.4%**) |
| animated jitter | 1.80 to 1.64 (9% better) | 40.38 to **8.68** (78% better) |

The candidate that looked BETTER at noon does essentially nothing where the
defect lives, and the one that looked worse is the fix. Any grain candidate
ranked at noon is ranked in the wrong regime.

**The decision, stated so it can be answered in one line.** Is 2.09 percent
frame-to-frame fizz an acceptable price for 4.6x less grain at the dusk line?
If yes, the change is one line in `45-cloud-temporal.wgsl` (restore the fidx
advance on the depth jitter hash) and the fizz should be re-judged by eye while
parked, because 2.09 percent is small in absolute terms and film-grain crawl is
perceptually loud. If no, the grain has to come out of the sun-term variance at
grazing angles instead, and item 2b’s spatial filter becomes the lever (forcing
it to full strength gave 1.80 to 0.89 at noon and has NOT been re-measured at
the terminator).

Not flipped unilaterally. He chose frozen deliberately after his own on/off
experiment, and a look decision he made with his own eyes is not one to reverse
from a metric behind his back.

### 2b-0. What the still-frame crumb actually is (2026-09-23, RESOLVED)

Read this before touching item 2b. It changes what 2b is allowed to conclude.

The operator has reported the planet "twinkling with white specks" and clouds
"glistening ... almost like they are boiling". A capture at a grazing view over
the Bahamas bank at dawn shows the cloud masses as a fine granular crumb, which
is the thing being described. Four arms were captured at that one camera,
differing only in the knob named, and scored as mean absolute deviation from the
3x3 mean over bright desaturated pixels:

| arm | high-frequency energy |
| --- | --- |
| `cloudres-full` (march at full screen resolution) | **9.22%** |
| `cloudres-half` (divisor 2, the operator own setting) | 4.89% |
| `cloudres-nodither` (divisor 2, march dither OFF) | 5.17% |
| divisor 2, captured before convergence (one-off arm, not kept) | 4.85% |

Every half-res arm lands inside a 4.85 to 5.17 band. Three hypotheses die at
once:

- **Not the dither.** Turning the march jitter off RAISED the number. At a
  parked camera the temporal filter has already removed the jitter entirely,
  and what the dither-off arm adds back is its own mip-ring arcs.
- **Not convergence.** One second of settle scores the same as fourteen.
- **Not the upsample.** The composite already reconstructs with a 9-tap
  Catmull-Rom under a neighbourhood clamp, and full resolution is WORSE, not
  better, so the half-res buffer is not aliasing away detail it should keep.

What is left is that the crumb in a STILL frame is the cloud density field own
high-frequency structure. The march is resolving the field it was given. Full
resolution doubles the number because each pixel resolves more of that field,
which means **raising `cloud_res` to cure speckle makes it worse and charges GPU
time for the privilege.** That is worth knowing on its own, because it is the
first thing anyone would reach for.

Consequences for the work:

1. Item 2b is a denoising item and the still-frame crumb is not a denoising
   problem. Any further spatial-filter tuning against a still can only trade
   real field detail for smoothness, which is exactly the trap item 2a-ii
   already flagged when it found a box blur winning the grain metric.
2. The operator complaint is about something CHANGING, so the remaining live
   question is temporal and specifically under MOTION, where the temporal
   filter cannot converge. The v0.1331.18 fix (un-freezing the depth jitter so
   the filter has something to average) took terminator grain from 40.4% to
   8.17% and is the right shape of fix; parked fizz costs 2.09% against 0.34%
   frozen, and that is the line to revert if parked fizz reads worse to him
   than the boiling did.
3. If the still crumb is judged to LOOK wrong once it stops moving, that is a
   fidelity question about the field texture, for the fidelity-expert, not a
   filter question. Real cloud at this range is not a uniform crumb.

The four fixtures are kept in `tests/visual/vantages.json` and each one carries
the table in its own `desc`, so the conclusion cannot drift away from the
measurement.

### 2b. The grain itself, for when the brightness is fixed

Kept because the measurements are real and were expensive, but do NOT work on
this until item 2 is resolved, because the grain may substantially be its
symptom.

Two filters exist and both are inert on the pixels that are speckled, measured
with a diagnostic build that made the resolve return `vec4(noise_w, alpha,
sig * 4, 1)`:

| quantity | measured | what the shader assumes |
| --- | --- | --- |
| `alpha` (temporal blend) | 0.091, deep on 88.8% | 0.12 at rest, correct |
| `noise_w` (spatial filter) | 0.094, strongly engaged on 1.3% | near 1 on noise |
| `sig` | **0.0185** | the absolute gate wants 0.25 |

So the temporal filter is deep and correct, and averages frames that v0.1253.2
froze to be pixel-identical, which removes nothing. The spatial filter that
exists to cover that case is off: its absolute gate `smoothstep(0.10, 0.30,
sig)` returns exactly 0.000 at a measured sig of 0.0185, thirteen times below
threshold. Effective strength on a speckled pixel is `0.094 * 0.35 = 0.034`.

Two arms were then built and measured, and the second is the useful one:

| arm | speckle | mean L |
| --- | --- | --- |
| baseline | 1.80% | 128.8 |
| relative gate widened to 0.06-0.28 | 1.75% | 128.8 |
| **spatial filter FORCED to full (`cur_s = mu`)** | **0.89%** | 129.4 |

Widening the gate did almost nothing and that refuted a stated prediction of
~1.3%, so the gate is not the binding constraint. Forcing full strength halves
the grain with NO brightness change and, checked by eye on a zoom, with cloud
silhouettes and terrain detail intact. The binding constraint is therefore the
strength cap `mix(0.35, 0.75, shallow)`: at deep alpha, which is exactly where
the noise lives, the spatial filter is capped at 0.35 BECAUSE it defers to the
temporal filter, and the temporal filter cannot work while the jitter is
frozen. The two filters each stand down for the other.

So the eventual fix is a pair, not a single knob: either un-freeze the jitter
(restoring what the temporal filter needs, and re-running the operator own
v0.1253 fizz experiment) or raise the deep-alpha spatial strength. Full
strength is 0.89% against Low 0.63%, so it is most of the way. Do not ship a
noise tweak first: it would mask item 2.

### 2c. The original bisect, kept for its refuted list


Rewritten 2026-09-20 after measuring it. The previous entry called 2000 km
"much improved but not clean" and pointed at the far-rung sampling story. Both
halves were wrong for the symptom the operator actually reported, so read this
before spending anything on the old framing.

**What the captures show** (all at the operator's own graphics settings, run as
`node scripts/probe-sweep.js --only <id> --operator-config`):

- `approach-2000km-ultra` is not improved at all. The masses disintegrate into
  isolated grains.
- `approach-2000km-high` is the important one, because High is the DEFAULT tier
  and therefore what every user sees. Its cloud masses have CORRECT, coherent
  silhouettes in the right geographic places, and their interiors are filled
  with per-pixel black-and-white noise. That is precisely the operator's report:
  "awfully dark and white in spots to the point of looking like old TV static."

**The bisect, using the screen-path channel instrument in
`assets/shaders/pbr/45-cloud-temporal.wgsl`** (`map_diag` N; the channels render
one raw ingredient of the march as greyscale and, importantly, still go through
the same resolve and composite as content, so they converge like content):

| channel | vantage | result |
| --- | --- | --- |
| 1, coverage alpha | `orbit-2000-high-mask` | **clean**: coherent masses, crisp edges, no speckle |
| 3, ambient luminance | `orbit-2000-high-amb` | **clean**: flat, even grey interiors |
| 2, direct-sun luminance | `orbit-2000-high-sun` | **GRAINY**: the same masses, full of salt-and-pepper |

So the density field, its thresholding, and the accumulation that carries alpha
are all fine. The direct-sun term is the sole carrier.

**Four hypotheses refuted, each built and captured. Do not re-propose them:**

1. Coverage sampling (the far-rung story). Refuted by the clean channel-1 mask.
2. The sun-shadow CACHE. `orbit-2000-high-nolight` (`cloud_light 0`) is
   indistinguishable from the baseline.
3. The per-pixel march DITHER. `orbit-2000-high-nodither` (`cloud_dither 0`) is
   indistinguishable from the baseline.
4. Ambient shaping. Refuted by the clean channel-3 capture.

**Where to look next.** The channel-2 doc names what is inside it: sun taps,
powder, cavity-on-direct. The cache is already excluded, so the per-sample sun
taps and the powder/cavity shaping are what remain. The question worth asking
first is why alpha converges over the accumulation while the direct-sun term
does not, given both ride the same resolve: a low-sample stochastic estimate
that the variance clip refuses to accumulate would produce exactly this.
`orbit-2000-high-notemporal` (history snap, one un-accumulated frame) is built
and not yet captured; it answers that question directly.

**The far-rung sampling defect is REAL but is a different defect.** Ultra
rendering 0.9 percent coverage against High's 31 is the item below, and fixing
it would not have touched the default tier or the symptom reported here.

**Method that keeps working, reuse it:** change ONE thing per build, capture
between each, and check that the instrument could have failed before trusting a
clean result. Channel 1 would have proved nothing if it rendered an analytic
coverage instead of the marched one; it renders the marched one.

**2026-09-22, the operator reported it again in different words, and three
things were settled.** His report: "the entire planet seems to kind of be
twinkling with white specks. They are tiny and they do not seem to rotate with
the planet." That is this defect, not a new one. A zoom into
`approach-2000km-high` shows it plainly: cloud masses with correct coherent
silhouettes whose interiors are pure black-and-white salt and pepper.

*A fifth hypothesis refuted.* `orbit-2000-high-notemporal` was built in the
previous pass and never captured; the entry above named it as the experiment
that would answer whether the accumulator is refusing to converge the sun term.
Captured now, with `scripts/speckle-census.js` (new, written for this):

| arm | speckle | spikes |
| --- | --- | --- |
| `approach-2000km-high` (baseline) | 1.76% | 0.44% |
| `orbit-2000-high-notemporal` (accumulation off) | 1.88% | 0.45% |
| `orbit-2000-low-tier` (Low, unjittered) | **0.63%** | 0.43% |

Turning the accumulation off changes almost nothing, so the accumulator is not
the carrier. Low tier at the SAME camera is 2.8x cleaner, so the per-pixel
jitter is.

*Why it "does not rotate with the planet", which is the useful clue.* All three
jitters were FROZEN in v0.1253.2 (see the long note in
`45-cloud-temporal.wgsl`) after the operator own on/off experiment, because a
frame-advancing jitter made a parked frame fizz. Frozen means keyed on
`in.pos.xy`, which is the SCREEN pixel. So the grain is nailed to the screen
while the world slides underneath it. Parked, it is stable and looks like fine
texture; in motion, it reads exactly as he describes, specks that stay put while
the planet turns. The freeze did not reduce the noise, it changed which
reference frame the noise lives in.

*A stale comment that will mislead the next reader.* `cloud_layer_volumetric`
in `40-clouds.wgsl` still says of the temporal composite: "This is where the
boiling static dies: the map is an exponential average of many jittered
marches." That map was RETIRED in v0.1250. `frame_shells.rs` pins
`near_mix = 1.0` whenever temporal is armed and the octa pass never dispatches,
so nothing averages the march any more. Confirmed live: `[CloudRegime]` reports
`temporal=true mix=1.00` at every altitude captured, 0.8 km to 35,870 km.

**The next experiment, and why it is the right one.** The bisect says alpha is
clean and the direct-sun term is grainy, and both ride the same jittered sample
positions. That asymmetry is the whole answer: alpha is an integral of density,
which converges fast, while the sun term is `exp(-tau_sun)`, strongly
nonlinear, so the same sample displacement that barely moves alpha swings the
sun term hard. Nothing downstream can fix that, which is why four accumulator-
and filter-shaped hypotheses have now all failed.

So DECOUPLE the two. Keep the jittered eye-ray sampling for alpha, where it
earns its keep dissolving the step comb and the mip rings, and evaluate the SUN
optical depth on the unjittered step grid (or on a jitter of much smaller
amplitude). Untried, cheap to build, and it predicts a specific result: the
channel-2 capture goes smooth while channel 1 is unchanged and the ring cure
still works. Gate it red first the usual way, and keep the ring metric from
v0.1270 in the loop, because the last person to turn a dither off brought the
radial artifact back at full strength without noticing.

**A SIXTH hypothesis refuted the same day, and it narrows the target usefully.**
The first guess at "decouple" was the wrong one, and it is written down so
nobody spends a second afternoon on it.

`cloud_sun_tau` spirals its cone taps by a PER-PIXEL azimuth
(`ang = 2.3999632 * i + g_lod_jitter * 6.2831853`, 40-clouds.wgsl). That looks
exactly like the carrier: a per-pixel rotation of a small tap set, inside the
one term the bisect blames, and `g_lod_jitter` really is per-pixel on this path
(45-cloud-temporal.wgsl sets it from `pcg2d_hash(in.pos.xy)` whenever the ring
cure is on, which is the default). Dropping the jitter term entirely:

| arm | mean L | speckle |
| --- | --- | --- |
| baseline | 128.8 | 1.80% |
| per-pixel cone azimuth removed | 128.8 | 1.76% |
| POSITIVE CONTROL, cone spread 0.12 to 2.50 | 119.9 | **1.39%** |

The azimuth does nothing. The positive control is the important row: it is the
same constant block on the same code path, it moved the frame hard, and without
it the 1.80-to-1.76 null would have been indistinguishable from an edit that
never reached the GPU (the rig junctions `assets/`, so it did, but that is a
fact to demonstrate rather than assume).

**What the control actually tells us, and it points at the fix.** Widening the
sun cone LOWERED the speckle by a fifth. More lateral averaging of the sun
optical depth means less grain, so the carrier is UNDERSAMPLING of tau_sun, not
the pattern of the samples. Cone width itself is not the lever (2.50 is a
nonsense cone and it costs 7% of the scene brightness), but it localises the
defect to how tau_sun is sampled rather than to how the taps are arranged.

So the next experiment is more specific than the entry above says. The eye-ray
depth jitter offsets each sample along the view ray, and the sun ladder starts
FROM that offset sample, so a per-pixel depth offset becomes a per-pixel
tau_sun. Snap the sun ladder origin to the UNJITTERED step-grid position while
leaving the eye-ray sampling jittered, so alpha keeps the dither that dissolves
the step comb and the mip rings, and the nonlinear term stops inheriting it.
Predicted result: channel 2 goes smooth, channel 1 and the ring metric are
unchanged. Measure with `node scripts/speckle-census.js`, and positive-control
the arm before believing a null.

**THAT EXPERIMENT WAS THEN RUN, and it is a partial result with a side effect.**
Built and captured the same evening rather than left as a plan, because the
mechanism turned out to be one line: `var tm = t_cur + dt * jitter` places each
view sample INSIDE its own step by a per-pixel fraction, so every sample
position is displaced by up to a full step before the sun ladder starts from
it. Starting the ladder from the step CENTRE instead (`t_cur + dt * 0.5`),
leaving the eye ray jittered:

| arm | mean L | speckle |
| --- | --- | --- |
| baseline | 128.8 | 1.80% |
| sun ladder on the unjittered grid | **138.8** | **1.54%** |
| Low tier, the clean reference | 148.1 | 0.63% |

The direction is confirmed and the magnitude is disappointing: about a seventh
of the distance to Low, and it costs 8 percent of scene brightness. The
brightness is the tell. Moving the ladder origin half a step changes which
column the shadow ray traverses, and the grid position is systematically LESS
occluded than the jittered one, so clouds self-shadow less. That is a look
regression on the surface the operator looks at most, so it was reverted rather
than shipped. The shader is back at the committed state, verified clean.

**What that leaves, and it is now a fair fight between two readings.**

1. *Undersampling of the sun ladder itself.* The 1.80 to 1.54 move says the
   per-pixel origin contributes, and the cone-width control says lateral
   averaging helps, but neither gets close to Low. A refinement worth one
   build: `cloud_sun_tau` takes ONE origin today. Give it two, keep the true
   jittered sample for the near rungs (where the eye-visible rind must
   self-shadow exactly, which is where the 8 percent went) and the grid
   position for the far rungs (where the variance is). That predicts the
   variance reduction without the brightness loss.

2. *The resolve filter is not doing its job.* The v0.1252 note says the
   variance-adaptive spatial filter cannot remove STRUCTURE but that white
   noise "is exactly what a local mean annihilates". This noise IS white (PCG,
   per pixel). So why does it survive? Nobody has measured the filter in
   isolation. If it is gated off, weak, or its variance estimate is being
   fooled by the genuine cloud-edge contrast in the same neighbourhood, that
   would explain every failed hypothesis at once, because all of them assumed
   the noise had to be removed at the SOURCE.

Reading 2 is cheaper to test and explains more, so test it first.

Note that Low is not a like-for-like control: it takes the DIRECT shell path,
one smooth unjittered sample per screen pixel, which is a different algorithm
rather than the same march with the jitter off. It is the right target to
match and the wrong thing to call a bisect arm.
### 3. The night-side coast glow: FIXED as BUG-081 (v0.1331.20)

**The cause was on the surface path after all:** `underwater_apply` adds a
navy water-column in-scatter scaled by `camera.sun_direction.w`, a single
number for the whole frame that the celestial pass stamps at a hardcoded 2.5.
So the night half read as full daylight. It is now gated by the fragment's
own sun elevation. Night-side blue-dominant pixels 5.589% to 0.011%; noon
unchanged (0.014% of pixels moved). Full record in BUGS.md BUG-081.

**Why the analysis below concluded "nothing identifiable", and the lesson.**
It eliminated aerial perspective but never tested `underwater_apply`, which is
the NEXT line of the same shared tail. And it read the residual night-side
mean of 1.94 / 2.86 / 3.68 (R / G / B) as "the noise floor of a dark frame".
That residual is blue-biased, blue nearly twice red, and **noise has no hue.**
A coloured residual is a signal with a source. When a remainder is dismissed as
noise, check first whether its channels are equal.

The second reason it hid: a night-side MEAN over a frame with the terminator
in it dilutes a 5 percent-of-pixels glow into a fraction of a level. Scoring
the COUNT of pixels carrying the signature (blue-dominant and above black),
inside a disc fitted from the terrain itself, is what made it measurable.

#### History: one cause fixed, the whole surface path believed eliminated

Reported twice. Fixture `orbit-terminator-3000km`, measured with
`node scripts/night-side-mean.js <png>`. Baseline night-side mean was
2.69/3.43/3.95.

**FIXED (v0.1331.2): the ambient floor lit the night side.** The shared tail had
`ambient = albedo * max(sky_ambient(...), AMBIENT_FLOOR) * ao`. That floor is a
silhouette floor for interiors and deep space, and the `max()` applied it on a
planet at night too. Dark land survived it; bright turquoise shallow-water
albedo did not, and the floor is blue-biased, which is where the cyan came from.
It now applies only where there is no local up. Mean 2.69/3.43/3.95 to
1.94/2.86/3.68, night-side LAND now reads black, interiors verified unchanged.

**ELIMINATED, each with a reach test that moved a control value:**

| candidate | result |
| --- | --- |
| surface direct light (`lo`) | night unchanged; day moved 122.8 to 102.9, so the edit reached |
| aerial perspective | night AND day unchanged; same tail proven live by the arm above |
| procedural emissive | night unchanged; type 12 zeroes it by design |
| water shell CONTRIBUTION | forced black, night unchanged: it is already black at night |
| water sky-LUT mirror | gated on local daylight (v0.1331.3), night unchanged at this fixture |
| atmosphere shell | disabled: night went UP 1.94 to 2.00, so it DARKENS the night side. Limb moved 168 to 163, so the edit reached |

Note the water shell DRAWS on the night side (forcing it magenta moved the mean
to 15.64/2.86/16.51 with green untouched) but contributes no light. Those are
different claims and conflating them cost an arm.

**What is left: nothing identifiable, and that is the finding.** Every draw in
the frame has now been eliminated with a reach test, including the atmosphere,
which turned out to DARKEN the night side rather than light it. The residual
night-side mean is 1.94/2.86/3.68, about 1.5 percent of full scale, which is the
noise floor of a dark frame rather than a glow with a source.

So the honest position is that the AMBIENT_FLOOR fix removed the dominant cause
and what remains may be nothing at all. Whether coastlines still READ as glowing
is now a judgement for the operator in the running game, not something another
still-frame arm can settle. If he says it still glows, the untested surfaces left
are the cloud deck over the night side and the post passes (bloom, godrays),
neither of which traces geography, so start by asking WHICH pixels he means.

**METHOD, the expensive lesson.** Roughly nine hypotheses have died here and at
least three died because the edit never reached the pixels: an early `return` is
rejected by the megashader validator; forcing albedo right after its binding is
overwritten by the textured path further down; and an arm that produces no
change is indistinguishable from an arm that never ran. Tint rather than
early-return, tint at the LAST write, and make every arm carry a value that MUST
move if it executed. See `feedback_prove_the_edit_reaches_pixels` in memory.
### 3b. 8-bit banding everywhere: the scene has no HDR target and no dither

Found 2026-09-24 while fixing the aurora. The scene renders straight into the
8-bit sRGB surface format (`renderer/mod.rs`, `surface_format` picked by
`is_srgb()`, and `create_scene_texture` uses it too), each pass tonemaps in
its own shader, and **nothing dithers before the 8-bit write.** A slow dark
gradient therefore quantises into flat rings one display level apart, and the
eye reads each ring as a hard edge. The aurora's faint diffuse glow spanned
three or four levels and drew as nested ellipses with crisp outlines; rendered
alone at full strength the rings multiplied into a contour map.

v0.1331.21 dithers the AURORA's own output (`srgb_dither` in
`30-atmosphere.wgsl`, triangular noise of one 8-bit step converted to linear
at the pixel's value). Every other dark gradient is still exposed: the night
sky, the atmosphere's limb and twilight falloff, dusk terrain, fog. Expect
more "harsh edges between shades" reports from those until this lands.

**The real fix** is the one every modern renderer uses: render the scene into
an `Rgba16Float` target, keep radiance linear and unclamped through every
pass, and do ONE tonemap and ONE dither in a final pass to the surface. It
touches every pass that currently writes `surface_format` (the scene texture,
bloom, godrays, SSAO, the cloud composite, the celestial passes) and every
shader that tonemaps inline, so it is an arc, not an increment. Until then,
`srgb_dither` is the stopgap to reach for on any surface that gets reported.

### 4. The far-rung gates, G0(d) and G1 to G7

Unchanged, and still the plan for the deeper cloud work. The increment is merged
behind knob 0; the gates are what turn it on. Design of record:
`docs/design/cloud-far-rung.md` (v2, after the v1 contract failed its adversarial
critique on eight real blockers). The measured target it exists to fix: at 873 km
Ultra renders about 0.9 percent coverage against High's 31, because one sample
per ray misses a 300 m layer vertically.

### 5. Fifty-five stale page snapshots

All 55 checked-in PNGs under `tests/snapshots/` are stale relative to main,
including pages the GUI extraction never touched. Proven by control, not
assumed: the base `src/gui` was restored into a worktree, five pages re-rendered,
and they came out byte-identical to what the extracted code produces while both
differ from what is committed.

So the snapshot check cannot catch a real change today, and a snapshot diff is
not evidence of anything until this is done. Regenerating blind would bake in
whatever drifted: the honest fix is `just snapshots`, then LOOK at the results
page by page before committing new baselines. Nobody owns it yet.

---

## Blocked on the operator

Not AI work. Listed so a session knows to route around them rather than pick
them up.

1. **Release signing happens on the operator's own schedule.** Recorded here
   only so a session understands why the desktop updater may be offering
   nothing: it trusts signed releases only, and an ineligible one is invisible
   rather than an error. **Do not raise this with the operator** - standing
   rule, CLAUDE.md "Release signing is the operator's to raise, never yours".
   He signs when he decides to; `docs/admin/release-signing.md` is there if he
   asks.
2. **Clear the old agent worktrees** under `.claude/worktrees/`. Audited
   2026-08-04: none could be cheaply proven redundant, and
   `just clean-worktrees` force-deletes branches and has destroyed
   review-approved work before. Operator-only by standing rule.
3. **Two gameplay questions** from
   `docs/design/playable-assessment-2026-09-19.md` section 7. **Crop growth speed
   is ANSWERED (2026-09-20)**: a growth multiplier separate from the world clock,
   1x / 10x / 100x plus a custom value, shipping at 10x, implemented and tested;
   Tier A item 3 is unblocked. Offline progression was answered on 2026-09-21 as well, and is broader than crops (a toggle on all three modes, applied to crafting too); its first rung (crops, builds and craft batches, single player) was BUILT on 2026-09-25, see `docs/design/offline-progression.md`. Still
   open: what the first ten minutes are, and whether a pipe reads as its real
   material or its utility colour. A third, lower: does multiplayer enforce
   anything, or is it co-operative trust until launch. NOTE that the report's
   question 2 ("do the 3D models ship with the release") is ANSWERED: they do,
   since v0.1322.0.
4. **The two demoted lethal Library guides**
   (`/library#making-water-safe-to-drink`, `/library#keeping-what-you-grew`)
   stay at `sourced` until a human has read them, per the rule the operator
   chose. `curriculum-status.js` enforces it.
   **Added 2026-09-24:** the fire-performance guides on the lethal topic
   `materials_fire_staff` (`/library#fire-staff-materials`, updated, plus the
   new `/library#staff-tubes`, `/library#fire-performance-fuels` and
   `/library#fire-performance-clothing`) are at `sourced` for the same reason.
5. **GitHub branch and tag protection on `main`.** Deploy auto-pushes to the
   live relay with no approval gate. GitHub settings, not code.
6. **Donations copy** needs the exact earmarked Sponsor-A-Can URL for HumanityOS
   and confirmation of whether those donations are tax-deductible and earmarked,
   before the CTA and FAQ wording can be finalized.
7. **Landing screen 2 hero shot:** click Play, frame something pretty, and tell
   the session to capture (`debug/screenshot_request.json`); it swaps the cosmos
   stand-in for the real 3D shot.

---

## Fenced arcs

**Arc A (in-world screens) is the one the operator picked, 2026-09-20**, when
asked which should come up next after the cloud work. Take arc A work ahead of
the rest of this section. The others remain unranked against each other and
against TIER 0: that ordering is the operator's call, and asking for it is
cheaper than guessing.

### A. In-world screens, the remaining rungs. PICKED (operator, 2026-09-20)

Six rungs merged (v0.1313 to v0.1314): the ScreenSurface, the screen as machine
data, the live feed and in-game camera, the console room, the video player, and
the readable web. Design in `docs/design/in-world-screens.md`,
`docs/design/readable-web.md`, `docs/design/media-player.md`.

Remaining, in the order they were fenced:

- ~~A placement gate on `embed.status`~~ BUILT 2026-09-25: a forbidden site
  is refused before it is fetched on the Browser page and every wall, an
  unreviewed one carries a note (`docs/design/readable-web.md`). **All 34
  records DECIDED 2026-09-25 by the operator** from
  `docs/reference/findings/2026-09-25-site-embed-terms.md`: 21 allowed, each
  with the credit line its licence asks for (`embed.attribution`, drawn on
  every page; the checker requires it for an allowed third-party site), seven
  forbidden (Project Gutenberg, Khan Academy, Instructables, Coursera,
  Discord, GOG, Examine), four unknown (no terms address it), OpenFarm and
  the dead ISS tracker link removed. Asking GOG and others for permission is
  the operator's to send; the contact routes are being gathered.
- ~~The screens' share of the interior frame (the console room at 9 fps)~~
  STALE, corrected 2026-09-25: that figure predates the P2/P3 megashader split.
  Measured after P3 (`docs/design/frame-cost-arc.md`, the phase-B table):
  `console-face-6` 29.8 fps at the 30 fps vsync cap, `gpu.scene` 14.16 ms,
  `gpu.transparent` 2.45; `console-face-3` (camera wall in view) 11.81 / 2.64.
  A web screen costs 0.064 ms GPU. What remains is the room's own scene cost,
  which belongs to arc B, not to the screens.
- **Sync** (redesigned by the operator 2026-09-25): a one-shot "jump to where
  they are". The receiver's own app loads the same URL and seeks to the same
  moment; playback, pauses and ads stay each viewer's own. Screens are drawn
  per viewer and never streamed; synced displays share a SOURCE and only our
  own domains. `docs/design/in-world-screens.md`, "Screens are per viewer".
  Then subtitles. Seek shipped in v0.1325.0 and is off this list.
- ~~Open defect: a `watch:` screen with no server retries a hostless URL~~
  FIXED 2026-09-18 in a494c7fc (`live_status` names where to set a server
  and does not connect at all); this line was stale until 2026-09-25.
- Deferred on purpose: VR controller rays; per-context `thread_local` page state
  (a screen and the main UI showing the same page share it); the `rooms.ron`
  entries for entry, pantry, hall and utility (named, not yet functional).

**Gate for any screen change:** both cargo checks, the screens / surface /
dispatch / machines lib tests, the standalone lints, AND a boot that enters the
world and drives the wall through the dev IPC. `just verify-screens` is the
named gate; static verification cannot see a dark or mirrored screen.

### B. The frame cost arc, remaining rungs

**Open, measured 2026-09-24: `gpu.celestial_t` is 25.6 ms looking straight
down from 600 km over the auroral oval** (`aurora-over-water`, about 20 fps on
the rig), against 11.3 ms at an oblique 300 km view. Two suspects are already
ruled OUT by measurement, so start elsewhere: the aurora itself (the thin-sheet
rework left the pass at 25.80 -> 25.64 ms, cost-neutral), and the emission
twin computing and discarding the atmosphere integral (moving its return ahead
of the integral changed 25.64 -> 25.63 ms). What differs straight down is that
the shell fills the whole screen, so look at what else in the celestial
transparent pass scales with shell coverage.

Design of record `docs/design/frame-cost-arc.md`. P1, P2 and P3 shipped: there is
no `fs_main`, six class entries share `frag_prologue` and `frag_tail`, thirteen
PSOs each compile one entry, and the split was itself a perf win (console
`gpu.scene` 17.1 to 14.1 ms).

- A wgpu pipeline cache. P3 cost 6 seconds of boot from three more PSO bakes,
  and this is the named next step. Note that wgpu 24 advertises PIPELINE_CACHE
  only on Vulkan; the DX12 path returns a unit struct that stores nothing, so
  requesting it naively fails device creation (the v0.782 class of bug).
- Split bodies, patches and grass inside `gpu.celestial` (three passes over the
  same attachments), which is still one unattributed number.
- A near-tree LOD ladder and impostor handoff. V1 proved the whole near-tree
  cost is ON-SCREEN fragment work, about 0.47 ms per visible model, so culling
  has no more to give and the ladder is the rung with reach.
- Clustered lights, then interior culling.
- W1 re-scoped: the water shell is 1.4 ms after P2, so the depth prepass plus
  backface cull is a fidelity and ordering call (a deterministic nearest fragment
  instead of heap-order blend), not a perf item.
- Rejected on evidence, do not re-propose: a masked-discard variant, an interior
  depth prepass, a G-buffer.

### C. The playable game

`docs/design/playable-assessment-2026-09-19.md` is the honest read: eleven loops
close end to end through the UI with no console and 24 of 42 systems tick, so
"framework built but not wired" is half wrong. What is missing is the layer
between the simulation and the person. Its tier ladder is the build order.

- **Tier A (make the existing game legible and durable).** A0 (ship the art) is
  DONE in v0.1322.0. **Builds persisted and offline progression rung 1 DONE
  2026-09-25**: `Structure` and `Construction` round-trip through `WorldSave`,
  the world clock is saved (every restart used to rewind the garden), and crops
  plus scaffolds plus craft batches (now saved too; a restart used to destroy
  whatever was mid-smelt) catch up by the time away behind a Settings toggle.
  Offline remaining: drone and manufacturing timers, livestock, then the server
  clock for multiplayer. **Simulation on the HUD DONE 2026-09-25:** food,
  water, energy, air, body temperature and waste under the health bar, and the
  active quest objective, with a Settings choice of Always / When low (default)
  / Off (health only). **Crop pacing DONE** (the growth-speed setting of
  2026-09-20 plus offline progression). Remaining: a scripted first-run
  sequence in the world. **Made a built thing do something
  (2026-09-25):** a built Furnace is a smelter and kiln, a Crafting Table a
  workbench, through `Blueprint::stations` into the station gate; and the build
  menu works at all now (BUG-082: every blueprint named items that did not
  exist). Still unconsumed: `rest` (bed), `storage` (chest), `shelter`.
- **Tier B (make the construction tool good enough to build a city).** Pick one
  canonical layout schema of the three that exist; the four multi-storey
  blockers in order, starting with a base Y on `InteriorWall`; collision for
  generated geometry; then author the acre with the tool. The report argues the
  tool over hand-authoring on evidence: the four code blockers stopping a second
  storey are the same four stopping the editor.
- **Tier C (make the world look right).** Un-gate hero plant models for towers;
  the conduit render pass; models for the machines a player stands in front of
  daily; read `mesh_kind` in `zone_filler.ron`.
- **Tier D (multiplayer, in the only order that works).** Move `PlacedItem` out
  of `src/gui` so `persistence` compiles into the relay; bridge
  `game_time_sync`; nameplates and appearance sync; make a trade move items; a
  `SystemRunner` host in the relay.
- **Tier E: NPCs**, which is arc D below.

### D. Populate the ship, and seat a dozen

Fenced 2026-09-15, NOT STARTED. The operator's two calls: the LLM-driven AI
player is backburnered in favour of simple AI humans (500 in one mothership
sector, billions eventually), and co-op must seat at least a dozen players.
Architecture in `docs/design/crowd-simulation.md`, modes in
`docs/design/game-modes.md`, measurement in `docs/design/npc-crowd-stress.md`.

The architecture in one line: do not simulate 500 agents, because that is a dead
end at 501. Three tiers instead (an aggregate population per zone, a roster whose
members are DERIVED, an embodied set promoted only for what is visible), so
per-frame cost scales with what you can see rather than with who lives there.

Rungs, each separately measurable: 0) reconcile the relay and client worlds (the
relay simulates a multi-deck ship while the client renders the flat homestead,
and remote Y is clamped to a constant to stop crew floating in the sky);
0.5) attribute the 17.7 ms of the 18.84 ms interior deck that has nothing to do
with NPCs; 1) the `npcs:N` knob plus `[npc-diag]` counters; 2) instanced crowd
rendering; 3) the timetable refactor (chores stop being countdown timers, which
also kills the network bill); 4) promotion and demotion with interest
management; 5) navigation; 6) animation.

Measured, not assumed, and it changes the budget: NOTHING in this repo can path
an agent around a wall (`behavior.rs` and `flow_field.rs` are 26-line dead stubs
with no call sites), and there is NO skeletal pipeline and no GLTF animation
import, so NPCs and remote players draw as a two-primitive marker. A crowd on
today's renderer is sliding markers.

### E. The Library curriculum

97 of 143 topics still have no document. The cheapest 16 already have both data
and sources, so one document completes all four layers; `just curriculum` lists
them and gives the live count. Budget a refutation pass PER GUIDE (48 of 60
researched species records were refuted on adversarial review).

Data gaps the curriculum already promises and cannot support:

- `data/laws/laws.json` is the declared data layer for both `greywater` and
  `forage_ethics` and has nothing for either.
- `data/chemistry/alloys.csv` records one of the four strength properties its
  topic promises and has no temper column, so 6061-O and 6061-T6 are one row.
- `plants.csv` and `creatures.csv` have no pest or disease field, so the pests
  guide's central lesson is unmodellable.
- `items.csv` cannot express edge state, tool condition, a workholding
  requirement, or a mallet.
- `data/constellations.json` is missing Cepheus and Hydrus.

For the remaining lethal-adjacent topics (`forage_toxic`, `forage_edible`,
`health_poisoning`, `chem_toxins`, `hunting`, `butchery`), write the prompt the
way the finished guide would describe itself to a reader. See the CLAUDE.md
note: the safety content is unchanged, it is the framing that trips the
classifier.

### F. Watching things on the in-world monitors

Operator: watching YouTube, Twitch and Rumble on the in-world monitors "is what
we want". The legal position is written down in
`docs/reference/media-stance.md` and the Library page
`docs/user/rights/laws_that_limit_this_software.md`.

- **Route A, our own player on open streams:** an HLS and DASH client plus the
  operating system's licensed decoders (Media Foundation, VideoToolbox), which
  also settles H.264 without shipping a patented decoder, and an Owncast
  instance on the VPS so anyone can stream to united-humanity.us from OBS and be
  watched on the monitors with no third party involved. Mission-aligned and
  unblocked. Re-read `docs/reference/findings/` before quoting the codec
  position: the video half needs no licence, the AUDIO half is the stuck one.
- **Route B2, the embedded browser:** Chromium rendered OFFSCREEN onto a screen
  surface through a SEPARATE opt-in host process (the default build never needs
  the Chromium SDK, the runtime is an opt-in hash-verified download, our own
  watch page hosts each platform's OFFICIAL embed with their ads untouched).
  Measured cost is not the obstacle: 0.064 ms GPU and under 0.63 ms CPU per 720p
  screen. The spike died in its reading stage with nothing written, so it starts
  fresh. `docs/design/embedded-browser.md`.
- **Refused as policy:** stream extraction the yt-dlp way. It breaks the
  platforms' terms, strips the ads that make embedding permitted, and invites a
  takedown against the repository that hosts our releases. B1 (a docked WebView2
  panel) is dropped: it cannot project onto a 3D surface.

### G. Realistic fire props, fire, smoke and gases (designed 2026-09-24)

Operator: 1:1 in-game flow-arts fire props (fire staff, poi, fire axe, darts,
fans, hoops) with believable fire, smoke and emitted gases, for teaching and for
depth beyond a health bar, while staying intuitive. Design, scout findings and
his seven answers: `docs/design/fire-props-and-combustion.md` (section 6 is
decisions). Key facts: EmberGen-class volumetrics were NEVER built (the fire,
smoke and explosion rows in data/particles.ron are never spawned); items cannot
carry a model; there is no body, hand or rope physics; the frame is 8-bit with
bloom off.

- **Next rung, unblocked: F1, one source of truth.** Turn the verified research
  in `docs/reference/research/2026-09-24-fire-performance/` into data rows both
  the Library and the sim read (alloys.csv solidus, liquidus, temper and
  service-limit columns; a fuels table; materials rows for fabrics). First
  resolve the 54 conflicts the data-consistency scout listed between
  data/chemistry/ and that research (fire-props-scout-reports.json).
- **Then the character body arc** (skinned mesh, skeleton, two hand attachment
  points instead of the single hands slot): the operator chose body first for
  props in hands. Pivot-mounted props are test fixtures only.
- **Rules he set for every rung:** full-realism and simplified modes; gear AND a
  toggle for gas visibility; open source only, our own solver (no EmberGen
  licence); cosmetic simulation client-local and never networked, each tier with
  a measured frame budget; fire trails drawn as the eye sees them; coloured fire
  from chemistry data plus a creative any-colour toggle.
- Ordering against arcs A to F is the operator's call.

---

## TIER 1: hardening before invites scale beyond a known group

**Effectively closed.** Everything code-actionable shipped (fail2ban, watchdog
plus multi-channel alerting, SQLite corruption recovery, crash-loop detection),
and the two decision-gated items were decided by the operator in 2026-05.
Retired detail is in `docs/history/priorities-archive-2026-05-to-08.md`.

Two residuals from the 2026-06-12 security audit, both MEDIUM, neither a launch
blocker:

1. **`/api/send` per-IP rate limit.** Needs X-Real-IP plumbing. Low value while
   the bot path is the trusted API_SECRET path.
2. **`/api/members` directory opt-out.** Design settled (reuse the existing
   `profiles.privacy` JSON with a `directory: "unlisted"` key, honored in
   `get_members`, `get_member_count` and `get_member_by_key`), deliberately
   deferred: a backend flag is useless without the user-facing toggle, so build
   both in the same privacy-UI increment. Verify json1 is compiled in first.

---

## TIER 2: big-feature gaps

Real features the system promises but does not deliver on every platform. Weeks
of work each.

> **Cross-cutting mandate (CLAUDE.md non-negotiable rule): GUI-first
> configurability.** Every ops and config capability must be reachable in-app,
> not CLI-only. The shipped ops work (alerts, backups, fail2ban, watchdog,
> secrets) is still CLI/SSH, and that is tracked debt. See
> `docs/design/in-app-ops.md`. New features with an ops dimension build their
> in-app control in the same increment.

1. **Web-mirrors-native parity (Track W).** Divergence map and migration order
   in `docs/design/web-native-parity.md`. Native chat is the parent. Steps 1 and
   2 done; NEXT is step 3 (message rows, timestamp pill, inline reactions), then
   header and composer, top-nav alignment, and a spacing sweep with dead-CSS
   removal.
2. **Studio and streaming (Track S).** `docs/design/studio-streaming.md`. Native
   capture, encode and stream shipped (v0.853 to v0.854), so build the widget on
   web first and mirror once native transport exists. Order: S0 persistent
   session, S1 web studio widget and modal, S2 viewer widgets and modal, S3
   privacy guard (independent, can land early), S4 native mirror.
3. **In-app ops console.** `docs/design/in-app-ops.md`. Slice 1 (System/Health)
   shipped on both clients. Remaining: the alert-channels editor (the first
   write panel), a backups panel, a federation panel, then fail2ban /
   relay-control / secrets (these need a sudo-gated relay-to-system bridge),
   then factoring out the action registry with AI-facing list and run endpoints
   plus a coverage test.
4. **Federation activation.** `docs/design/federation-activation.md`. Native
   Phase 1 admin UI shipped (v0.722.0); the web mirror is what remains of Phase
   1. Phase 2 per-peer profile-gossip rate limit, Phase 3 a second
   operator-controlled relay federated end to end (the load-bearing test is
   whether moderation propagates), Phase 4 vetted third-party peers. The
   fail-closed default means dormant is safe.
5. **P2P groups, phases 3 to 5.** `docs/design/p2p-groups.md`. P1 and P2 are
   done on both clients (signed objects, offline-joinable invites, E2EE
   messages, group-as-channel, leave and disband). P3 P2P transport (the relay
   becomes signaling-only), P4 relay-independence (multi-relay signaling plus
   peer-assisted plus TURN: the actual payoff, a group survives a dead home
   relay), P5 serverless discovery (mDNS/DHT).
6. **Privacy follow-ups** from the 2026-08 arc: full native inline image decrypt
   and render; group-chat attachments (the same pattern as DM attachments, not
   yet done); friendship certificate expiry and revocation; a native TURN
   toggle. Beyond these the honest remaining set is mixnet-class traffic
   analysis and fundamental limits.
7. **Native voice tail.** The str0m arc shipped voice itself. Remaining:
   per-peer volume / mute / squelch UI, web transmit-mode UI, a two-str0m CI
   harness, graceful relay restart.
8. **Native trade UI completion.** The Trade page exists in `src/gui/pages/` but
   trade events (`trade_response`, `trade_confirm`) are not dispatched. Wire them
   or remove the page until it is ready.
9. **Library, the federated file and media catalog.** `docs/design/library.md`.
   The Files engine first (trust-tiered LRU cache, bounded disk by construction,
   identity by content hash), then the Files UI, pin and torrent, perceptual
   dedup, federation aggregation, and folding Tools / Browser / Resources in.
10. **Device mesh.** `docs/design/device-mesh.md`. Your devices back up each
    other and the relay; review every device's system info from any one device.
    Phase A system-info reporting and a My Devices dashboard, B backup
    designation and pull (subsumes the shipped PowerShell stopgap), C restore
    flow, D LAN direct-sync plus mobile members and remote wipe.
11. **Real-life-first boot and the real/fake multi-save model** (revised
    2026-06-30; the operator rejected the "game/simulator toggle" framing as too
    confusing). Multiple saves, each house or character flagged real or fake.
12. **Litestream or equivalent continuous backup.** Documented-optional today
    and verified NOT deployed. SQLite WAL to blob storage, RPO about a minute.
13. **Mobile clients.** Android needs a JNI bridge for the keyring plus an
    AndroidKeyStore backend; iOS mostly needs a build target.

---

## TIER 3: UX accessibility (the ELI5 mandate)

The mission requires this layer. Not optional, just sequenced after the
load-bearing work.

1. **A tooltip on every interactive element**, in plain language. Audit pages one
   at a time.
2. **The first five minutes.** A guided tour: identity, seed backup, first
   channel, first message, status, done. The Onboarding page exists but the flow
   needs polish.
3. **Localization expansion.** Five languages today (en, es, fr, ja, zh). Add at
   least ar, hi, pt, ru, de, sw. `data/i18n/` supports it; the work is
   translation, not code.
4. **A full accessibility audit** against WCAG 2.1 AA. The modes exist in
   `src/gui/theme.rs`; the audit does not.
5. **Glossary on every page.** 442 terms in `data/glossary.json`; web has the
   overlay, native has no widget yet.

---

## TIER 4: long horizon

Do not touch these until TIERs 0 to 3 are mostly done. Listed so they are not
forgotten.

1. **LoRa mesh hardware integration.** Needs actual radio hardware on hand.
2. **STARK selective disclosure.** The scaffold exists; circuit design deferred.
3. **AI agent governance.** First-class AI participation is documented in
   `docs/ai/onboarding.md`; as more AI participants connect, Article 14 needs to
   become enforced rules with appeals rather than documented intent.
4. **Distribution beyond GitHub.** The Forgejo mirror exists, BitTorrent and IPFS
   are scaffolded; Codeberg, Software Heritage and a WinGet manifest are pending
   per `docs/admin/distribution-mirrors.md`.
5. **Real-hardware control layer.** Bind a home to real monitoring and automation
   hardware, so the game becomes the control panel for an actual homestead. The
   north star.

---

## Tier criteria: how to decide where something goes

- **TIER 0**: the next thing to work on, strict-ranked, at most a handful of
  items. If TIER 0 has grown a second "current focus" block, that is the signal
  to resolve it back into one order.
- **TIER 1**: "we can invite known people but not unknown people until this is
  done."
- **TIER 2**: "the feature is promised but does not fully work." Multi-week.
- **TIER 3**: "real users can use the app but they need help understanding it."
- **TIER 4**: "nice eventually; do not let it crowd out the load-bearing work."

When adding an item, pick the LOWEST tier it could justifiably go in. Tier-up is
rare; tier-down is normal as things turn out less critical than they felt.

---

## Where the shipped work went

This file lists only what is NOT done. For what shipped, the live sources are
`git log`, the GitHub release titles (unusually descriptive in this repo),
`data/coordination/orchestrator_state.json` `recent_decisions` (the WHY),
`docs/FEATURES.md`, `docs/STATUS.md`, and `docs/history/<date>.md`.

Retired backlog blocks, verbatim and with their reasoning intact:

- `docs/history/priorities-archive-2026-08-to-09.md` (the cloud, rosette, perf,
  screens, Library and content arcs, 2026-08-21 to 2026-09-19).
- `docs/history/priorities-archive-2026-05-to-08.md` (the old Active focus stack
  back to the web chat rebuild, plus the settled TIER 0 and TIER 1 entries).

Do not reintroduce a hand-maintained "recently shipped" list here. One rotted to
v0.283.0 while the project shipped past v0.515, and a third competing "what is
done" list is worse than none.
