# Realistic fire props, fire, smoke and gases (and, second, asteroid impacts)

> **Status:** design brief, 2026-09-24. Nothing here is built. Written from six read-only scout reports after the operator asked for 1:1 in-game flow-arts fire props (fire staff, poi, fire axe, darts) with believable fire, smoke and emitted gases, for teaching and for gameplay depth beyond a health bar. Arc ordering against the other fenced arcs in docs/PRIORITIES.md is the operator's call. The open questions in section 6 are his.

Date: 2026-09-24. This brief brings together six read-only scout reports. Every claim about code cites a file and line or a commit. Numbers the scouts marked as "derived estimate" are labelled that way here too, and they need measuring before they go into data.

---

## 1. What exists today

### Direct answer: was EmberGen-like functionality ever added?

**No.** No volumetric fire, smoke or explosion simulation has ever been built, started or removed. `git log --all -i --grep=embergen` returns nothing. EmberGen appears only as a named target in the realistic-first rule (CLAUDE.md:836, commit c71bc90b, 2026-08-01) and in two August journal entries (docs/history/journal-archive-2026-08.md:14, :98). The overnight arc on 2026-08-03 was told to "learn from SpeedTree + EmberGen", and it shipped tree work only.

The memory most likely blends four real things:

| What you may remember | What is actually there |
|---|---|
| JangaFX "forever ago" | JangaFX was listed as a partner on the old Project Universe site (docs/history/project-universe-site/pages/organizations.txt:14, archived 2023-07-10). No code. |
| "Fire, smoke, explosion" effects | v0.90.0 (b72a00c8, 2026-04-07) added emitter recipes for fire, smoke, sparks and explosion to data/particles.ron (:9, :26, :43, :181). They are flat glowing dots. **No code has ever spawned them.** The only emitters the world creates are leaf_drift, dive_bubbles, space_dust and rain/snow (src/lib.rs:3839, 3860, 3921, 4140). The "explosion" is 300 dots that last 0.3 to 1.0 s: firecracker scale. |
| EmberGen as a goal | It is named as a north star in the 2026-08-02 realistic-first norm. It was never built. |
| Something volumetric | The volumetric cloud ray-march (src/renderer/clouds.rs; assets/shaders/pbr/40-clouds.wgsl, 41-cloud-bodies.wgsl, 45-cloud-temporal.wgsl) is the one EmberGen-style technique that did ship. It renders clouds only. |

It also helps to know what EmberGen itself is. It is an **offline authoring tool**. Its GPU solver runs live for the artist, and the results are exported as flipbooks with motion vectors and six-way lighting maps, or as VDB volume sequences. It is not a runtime that ships inside a game. Games that "use EmberGen" play back what it baked.

### Rendering pieces that exist

- **Particle system.** CPU simulation with camera-facing sprites (src/renderer/particles.rs:1-649). Motion is constant gravity only (:250-258). Colour and size fade in a straight line (:359-375). Every particle spawned in a frame appears at the emitter's end-of-frame position (:330-335).
- **GPU particle pool.** One emitter per pool, alpha draw only, built for rain and snow (src/renderer/particles_gpu.rs:1-254; assets/shaders/particle_sim.wgsl). It went in at v0.1068 (146c3335) and became the default at v0.1085 (dabed6a2). Unlike the CPU path (particles.rs:476-487), it has **no floating-origin rebase**.
- **Particle shader.** A soft round disc that writes `rgb*(1+emissive)` straight into the frame (assets/shaders/particles.wgsl:76-95). No textures, lighting, soft edges against geometry or sorting.
- **The frame is 8-bit and bloom is off.** The scene target is 8-bit sRGB (src/renderer/surface.rs:171-194; mod.rs:1159-1165). Tone mapping happens inside each PBR material (pbr/80-fragment-shared.wgsl:436-442) and never for particles. Bloom is built but never called (src/renderer/bloom.rs:169; intensity 0 at mod.rs:2045). **Any flame brighter than white clips to a flat yellow-white disc.**
- **Local lights.** Point, spot and capsule line lights (src/renderer/light.rs:86-113), up to 2048 with tiled binning (light_tiles.rs). They cast no shadows and have no flicker type.
- **Lava material** (type 11, pbr/90-fragment-main.wgsl:1186-1193). A starting point for a glowing, charred wick.
- **Blackbody-ish star colour** (src/renderer/stars.rs:2285-2310). An approximation, not a proper Planck curve.

### Simulation pieces that exist

- **FireSystem** (src/systems/fire.rs, v0.142.0, 5ffea893) is not registered (tests/engine_wiring_lint.rs:71), and no code attaches its components. If it did run, "intensity" would be a unitless 0 to 1 value, fuel would drop 1 unit per second, and damage would be a flat 1 HP/s within 1.5 m. It ignores oxygen, fuel type, temperature and smoke, and it never reads data/fire_system.ron (loaded as untyped `ron::Value`, fire.rs:31-36).
- **AtmosphereSystem** is live (lib.rs:1070, since 310714e4). It models one 14,000 m3 home air volume (engine/home_spawn.rs:36) with O2 and CO2 from occupants. Its explosive-limit and toxic-gas tables are hard-coded (atmosphere.rs:195-237) and duplicate data/chemistry/gases.csv. The damage path needs `InEnclosedSpace` and `IgnitionSource`, which nothing spawns, so only the O2 "breathable" flag reaches the player (lib.rs:5741-5758).
- **Vitals are richer than the HUD shows.** Satiation, hydration, energy, oxygen, body temperature and waste all exist and drain health (ecs/components.rs:79-110; food.rs:476-630). The HUD shows a health bar only (gui/pages/hud.rs:101-118). Everything else is on the Inventory page (inventory.rs:1431-1520).
- **Status effects are ready but unused.** Rows for burning, poisoned, toxic_atmosphere and overheated (status_effects.csv:46, 47, 80, 54) would tick through the live effect loop (food.rs:601-626) as soon as something applies them. Nothing does.
- **Burn degrees 1 to 3** are defined in data/medical.ron:105-139, but MedicalSystem is dormant.
- **DisasterSystem** has MeteorImpact and AsteroidImpact (src/systems/disasters.rs:36-37, 104-105, 124). It is not registered (engine_wiring_lint.rs:41), and nothing reads its `vfx_id` strings.

### Models and props

- **Model loading.** The glTF loader reads only the first primitive of the first mesh, ignores node transforms, and decodes only the base-colour texture (src/assets/mod.rs:261-432). The vertex type holds position, normal and UV, with no tangents or bone weights (src/renderer/mesh.rs:9-13).
- **What can carry a model.** Machines, furniture, crops and trees can. **Items cannot**: neither items.csv (header at line 16) nor `ItemDef` (inventory/mod.rs:277-295) has a model column.
- **No bodies or animation.** There are no hands, no held items and no local player body (camera.rs:1147-1180 orbits an empty point). There is no skeletal animation. Rapier joints exist but are never created (physics/mod.rs:19-20), so there is no rope or chain simulation.
- **No flow-arts props anywhere in data.** The nearest items are `staff_0` (an oak blunt weapon, items.csv:107), `torch_handheld_0`, `candle_0` and `lantern_oil_0`, and none of them gives off light.
- **The strongest real asset is the sourced Library guide** data/library/fire_staff_materials.md (a copy of docs/user/making/, commits 972582da, 113e4395, 1a6382f3, 2773ef75). It covers 1:1 dimensions (:446-460), fuels (:340-356), wicks, fumes and a safety kit that includes a natural-fibre clothing rule (:398-413).

### Stale docs found along the way (fix in passing)

- docs/FEATURES.md:1334-1338 and :2402-2404, and STATUS.md:333, claim emitters that do not exist and name the dead `particle.wgsl`.
- ENGINE_REFERENCE.md:409 and :75-85 say particles, bloom and shadows are missing.
- model-pipeline.md:38-40 says textures are ignored.
- The "hot reload" comment in particles.ron:2 is wrong: the file loads once at lib.rs:1699.
- The v0.1065 snowflake shape was written into the unused particle.wgsl (da795adc), so it never reached the screen. New particle shapes must go in `particles.wgsl`.

---

## 2. The target

A player picks up a fire staff, poi, fire axe, fire darts or meteor, fans or a hoop. Each is built to real dimensions from real materials, and lit, it behaves the way the real one does:

- **The flame is a trail of burning vapour left in the air**, not a sprite glued to the wick. A spinning staff tip moves at roughly 9 to 13 m/s (derived estimate). The flame lies back along the path as a tapering tongue about a quarter turn long, white-yellow at the wick, deepening through orange to a dull red tail, with a faint blue base. Standing still, it becomes an upright teardrop about 25 to 35 cm tall that puffs 5 to 7 times a second for a 5 cm wick (the Cetegen and Ahmed scaling).
- **The flame lights the world.** A warm pool of about 1900 K light, derived at roughly 250 to 400 lm per wick, sweeps across the ground, and the performer's shadow swings away from it. From 10 m or more, that pool is how people recognise a fire spinner.
- **The fuel sets the look.** Kerosene burns bright and sooty with black-brown smoke. Camp fuel burns cleaner and shorter. Alcohol burns nearly invisible pale blue. All of it comes from data rows.
- **The gases are real and mostly invisible.** Burning fuel makes CO2, water vapour, CO and soot. Only the soot (black) and condensed vapour (white, for example when dousing a wick) should be visible in the normal view. The invisible products still enter the room's air and still affect the player. An optional analysis view could make them visible for teaching (see Open questions).
- **Consequences matter beyond health.** Heat creeps up the shaft into the grip. Wicks wear, and you can see it. Clothing fibre decides whether a flare-up is a scare or a melt burn. Indoor spinning fills the room with smoke and CO. A spotter with a fire blanket actually matters.
- **It stays intuitive.** The world shows the consequence first (a smoking glove, a charred wick, a dimming sooty flame). A small chip explains it. The Library guide is one click away.

Secondary: an asteroid impact where the flash comes first, then a rising fireball and an inverted-cone ejecta curtain, then the air blast, dust skirt and sound after distance/343 m/s, a crater, and fires started by the heat pulse. Sizes come from the Earth Impact Effects Program equations.

**The architecture in one line.** The reference-technique scout recommends three tiers, and the 2026-08-26 smoke decision (commit 24e8e3ce, archived at docs/history/priorities-archive-2026-08-to-09.md:3347-3352) already rules out a second billboard system:

- **Tier A:** flame parcels plus one light per wick, for every prop at any distance.
- **Tier B:** a live sparse GPU combustion grid (NVIDIA Flow 2.2, BSD-3, is the open reference) for the 1 to 4 nearest flames, ray-marched with the cloud machinery.
- **Tier C:** baked volumes and flipbooks for one-off effects such as impacts. This is the part that is really "EmberGen".

---

## 3. One source of truth

### The principle

The Library guides and the simulation must read **the same data rows**. Today the best numbers live only as prose in two guides, and the CSVs disagree with them. Infinite-of-X applies: a fuel, alloy, wick fibre, clothing fabric or prop is a data row, never a code constant. The guides' tables should be **generated from** the data, the same way `scripts/build-library.js` already syncs the Accord. Then the guide, the game and the teaching cannot drift apart.

### Step zero: the research is in the repo

Done 2026-09-24. The research files (aluminium, titanium, steels-others, alloy-corrections, the corrected alloys-final table and its calculator, fuel-identities, fuel-use, heated-fumes, length-sizing, wall-thickness, the clothing dossier and the scout reports) are saved in docs/reference/research/2026-09-24-fire-performance/, with a README on how far to trust each file. The Library guides written from them cite registered sources in data/sources/registry.json.

### Proposed data shape

| File | Change |
|---|---|
| `data/chemistry/alloys.csv` | Split `melting_point_k` into `solidus_k` and `liquidus_k`. Add `temper`, `yield_mpa`, `modulus_gpa`, `elongation_pct`, `max_service_c` (strength-loss onset, which is **not** the melting point), `value_basis` (min / typical), and `source_ids`. Add the staff-relevant rows: 6063, 7075-T73, 7068, Grade 9 CWSR Ti, 17-4PH, 201, 410, 430. |
| `data/chemistry/fuels.csv` (new, or new columns on compounds.csv) | One row **per retail product class**, not per generic chemical: camp fuel / white gas, lamp oil (C12-15 alkanes), 1-K kerosene, denatured alcohol (5 to 60% methanol), isoparaffins. Columns: `flash_point_c`, `autoignition_c`, `heat_of_combustion_mj_kg`, `soot_yield`, `smoke_point_mm`, `h_c_ratio`, products per kg (CO2, H2O, CO, soot), `burn_time_min_range` with `basis=practitioner`, visible colour notes, `source_ids`. |
| `data/chemistry/gases.csv` | Add water vapour. `atmosphere.rs:195-237` should read this file instead of its hard-coded match arms. |
| `data/chemistry/toxins.csv` | Add metal-fume fever (ZnO), polymer-fume fever (PTFE), n-hexane neuropathy and hydrocarbon aspiration. |
| `data/materials.csv` | Add `decomposition_c`, `specific_heat`, `emissivity`, and `melts_and_drips` (the clothing rule). Add Nomex/aramid blends, Duvetyne, cotton, wool and polyester rows so clothing and fire blankets are materials, not special cases. |
| `data/props/flow_arts.ron` (new) | Each prop's parts, real dimensions (shaft OD and wall, length, wick width, number of wraps, chain length), materials by row id, and wick fibre blend. |
| `data/items.csv` | A `model` or `prop_def` column so an item can appear in the world. |

Values that rest on weak evidence carry that in the row (`basis=practitioner`, `basis=curve_read_by_eye`, `basis=from_memory_unverified`), so the game and the Library can both say "about" honestly.

### Conflicts to resolve before any of this becomes data

The data-consistency scout found **54 conflicts** (C1 to C54). None have been resolved. They are resolved by reading the primary sources, not by operator taste. The ones that change behaviour:

1. **Melting points mix two bases** (C1, C2). In alloys.csv, 7075 (908 K) and 2024 (911 K) are liquidus values, while 6061 (855 K) is a solidus. Fix: separate solidus and liquidus columns.
2. **"Starts to fail" uses melting points** (C25, C26; fire_staff_materials.md:67-78). The research puts permanent strength loss for 6061-T6 at about 150 to 260 C and the Ti-6Al-4V service ceiling at about 325 to 400 C. This is the single most important fix, because the game's heat-to-shaft model needs the service limit.
3. **Fuel flash points** (C35 to C39):
   - Camp fuel should not be grouped with VM&P naphtha.
   - Lamp oil is not kerosene: Firefly flashes at 83.5 C and Lamplight at 97 to 121 C.
   - Kerosene's minimum is 38 C under ASTM D3699, not 35 C.
   - Retail denatured alcohol flashes at 7 C, not 11 to 13 C.
4. **Burn time** (C38, C40). The sizing table's "7 to 9 min" contradicts the per-fuel figures of 3 to 4 min (white gas) and 5 to 7 min (kerosene), and it contradicts the guide itself. Sources also disagree on which fuel burns longest.
5. **Titanium** (C5, C30, C31). CP titanium conducts about 22 W/m-K, **more** than stainless at 14.6 to 16.3. Titanium staffs and Grade 9 tube are sold. The guide's "only material that solves grip heat" and "effectively absent from the market" are both wrong.
6. **Toxic flags** (C20 to C24). compounds.csv marks teflon, zinc oxide, titanium dioxide and kerosene non-toxic. The research documents fume or aspiration hazards for each, and the repo's own guides quote the ZnO hazard.
7. **Fuel versus wick and grip** (C41, C42). The guide says fuel does not attack the wick and that silicone sheds fuel. Makers and practitioners say otherwise. Record both sides with their basis.
8. **Stiffness claims** (C27, C29). Every aluminium alloy has about the same modulus, so 7075 buys yield strength, not stiffness. Roll-wrapped carbon fibre is roughly aluminium-stiff (a self-described rough estimate).
9. **Cost and "cheapest"** (C33, C45). Speedy Metals priced DOM carbon steel above 304 stainless for the same tube.
10. **Internal repo drift:**
    - metals_and_alloys.md still describes the corrosion vocabulary from before 6845aa5f.
    - phosphor_bronze and tin_bronze are nearly the same alloy with different numbers (alloys.csv:21, :75).
    - Methanol's lethal dose is 100 mg/kg in compounds.csv:48 but 810 mg/kg in toxins.csv:72.
    - The guide's 1670 C titanium melting point falls inside its own "every material except titanium" flame band.

Where the research files disagree with each other (6061 modulus 68.3 vs 68.9 GPa; lamp oil 83.5 vs 97 to 121 C; the carcinogen status of TiO2), record the range with both sources rather than picking one silently.

---

## 4. Rung ladder

Each rung is sized for one session unless flagged. Verification uses the tools the repo has:

- **Probe rig:** `node scripts/probe-sweep.js --only <vantage> --exe target/release/HumanityOS.exe`. It enters the world, which is the bar for renderer changes.
- **`debug/screenshot_request.json`**, for a live capture.
- **`just snapshots` / `just snapshot <page>`**, for egui pages.
- **`cargo test --features native --lib`**.
- **`cargo check --features relay --no-default-features`**, for any new module.

Only one GPU rig may run at a time.

### Fire-prop ladder

**F1. Data from the reconciled research (data only, no renderer).**
- Technique: the Section 3 schema; Library tables generated from the rows.
- Files: new findings docs; data/chemistry/{alloys,compounds,gases,toxins}.csv; new data/chemistry/fuels.csv; data/materials.csv; data/sources/registry.json; data/props/flow_arts.ron; a build-library-style generator for the guide tables; docs/user/making/fire_staff_materials.md and metals_and_alloys.md corrected; atmosphere.rs reads gases.csv.
- Verify: `just validate-data`; a lib test that loads every fuel and alloy row and asserts solidus <= liquidus and max_service < solidus; a test that the generated guide tables match the CSVs; `node scripts/build-library.js` shows no diff after regeneration.

**F2. The prop exists in the world, plus the rig can show it.**
- Technique: procedural loft from real dimensions. A staff is a tube plus two wick coils, built from `Mesh::polytube`/`tube` (mesh.rs:593-685) and the parallel-transport ring lofting in plant_mesh.rs, following the procedural-tree precedent. This is the real architecture, not a stand-in, and hand-authored GLBs can replace any prop later.
- Items get a `prop_def`/`model` link. The prop spins kinematically around a fixed pivot (the door_anim rigid-animation precedent), because there is no body yet.
- Rig prerequisite: add a data-driven showcase entry `fire_prop {kind, fuel, spin_rps, radius_m, plane, lit_s}` and an N-frame capture at fixed dt.
- Files: data/items.csv, data/props/flow_arts.ron, a new prop mesh builder under src/renderer/, src/engine/ (placement), tests/visual/vantages.json plus the showcase handler.
- Verify: new vantage `firestaff-wick-closeup-0.5m` (unlit). The expected result names shaft diameter and length within 5% of the data row, and the wick reads as a coiled tape. World entry must report panics=0.

**F3. Tier A flame parcels (the top realism cue).**
- Technique: Lagrangian flame parcels.
  - Emission is spread along the wick's swept sub-frame arc, and each parcel is pre-aged by its sub-frame offset. This removes the 16.7 cm "string of beads".
  - Motion is drag toward the air velocity, buoyancy proportional to (T-Ta)/Ta, and curl-noise turbulence (Bridson 2007) from a small precomputed 3D noise.
  - Colour comes from a blackbody lookup table, Planck integrated against CIE 1931 (Nguyen/Fedkiw/Jensen 2002; Pegoraro and Parker 2006).
  - A premultiplied blend (emission plus destination times transmittance) lets soot absorb as well as emit. A thin CH*/C2* blue band sits at the wick.
- Engine changes: a GPU pool emitter table (replacing the single SimParams, particle_sim.wgsl:29-45) and the missing floating-origin rebase shift.
- Files: src/renderer/particles_gpu.rs, assets/shaders/particle_sim.wgsl, assets/shaders/particles.wgsl (not particle.wgsl), a new blackbody LUT builder, data/particles.ron flame entries tied to fuel rows.
- Verify: vantage `firestaff-night-4m` (2 rev/s, kerosene). Expect a continuous tapering tongue about a quarter turn long. Regressions to rule out: no beads spaced one frame apart, no full-circle rings, no flame standing upright at speed. Companion `firestaff-static-1m`: an upright teardrop 25 to 35 cm tall. A single PNG already shows the bead defect, so prove the gate red on today's code first.

**F4. Firelight.**
- Technique: one capsule line light per wick, from last frame's position to this frame's (light.rs:104-113). Intensity in lumens from the fuel row, colour from the same blackbody table. Flicker comes from the parcels' summed emission, not a separate random number.
- Shadows, first rung toward ReSTIR plus ray-query: screen-space contact shadows toward the 1 to 2 nearest flame lights (Bend Studio, Apache-2.0). A low-resolution cube shadow for the nearest performer can follow.
- Files: src/lib.rs light gather (~14631), data/lighting/light_types.ron (a flame type), src/renderer/light.rs.
- Verify: `firepoi-night-10m`. Expect a warm pool brighter on the side the flames are on. Measured gates: ground chromaticity 1700 to 2100 K CCT; the flicker series from the 64-frame capture peaks between 3.5 and 10 Hz for a 5 cm wick.

**F5. Linear HDR frame (gate for "flame reads as light").** Flagged: may exceed one session.
- Technique: an Rgba16Float scene target, one tone map after every layer, and multi-mip energy-conserving bloom (Jimenez 2014), wiring in the dormant bloom.rs.
- This is W5 of docs/design/environment-program.md:807, which says the layers cannot be converted piecemeal. Coordinate with that lane rather than doing a fire-only version.
- Files: src/renderer/surface.rs, mod.rs, bloom.rs, pbr/80-fragment-shared.wgsl, particles.wgsl.
- Verify: `just verify-runtime` across its three vantages plus the F3/F4 vantages. Expect near-white cores with a soft halo, and overlapping flames brighter where they cross. Regressions to rule out: no clipped flat discs, and no halo on non-emissive surfaces.

**F6. Smoke from the fuel, with soft edges.**
- Technique: smoke parcels continue from flame parcels and inherit soot mass as they cool. Extinction is soot mass times 8.7 m2/g, with low albedo for fresh soot (Mulholland and Croarkin 2000; Bond and Bergstrom 2006).
- Soft particles fade by scene depth (Lorach 2007); the depth texture is already sampleable (surface.rs:211-212).
- Lighting starts with sun direction plus the flame light. Froxel lighting (Hillaire 2015) comes later.
- Files: particles.wgsl, particle_sim.wgsl, fuels.csv soot_yield.
- Verify: `firestaff-static-dusk-1m` A/B. Kerosene: thin black-brown wisps gone within about 1 m. Methanol: a nearly invisible blue flame, no smoke, a dimmer pool. No hard cut lines where sprites meet the ground or body.

**F7. The wick as a simulated object: fuel, burn-out, gases, heat, injuries.**
- Technique: a new registered WickSystem. Do not revive fire.rs as it stands.
  - Soak mass, spin-off, and burn rate from the fuel row.
  - Burning dry chars the wick.
  - Products per kg go into the AtmosphereSystem composition, and O2 is consumed, so a wick cannot burn in vacuum.
  - Heat conducts wick to shaft to grip in 1D, using the alloy's conductivity, specific heat and max_service_c.
  - Radiant and contact exposure map to medical.ron burn degrees.
  - A CO/smoke field on EnvironmentContext lets the existing status rows (toxic_atmosphere, poisoned, burning, overheated) apply through the live effect tick. Each needs only an `effects.apply()` call.
- Also fix the status-ID mismatch: fire_system.ron uses `debuff_*` IDs that are not in status_effects.csv.
- Files: new src/systems/wick.rs, src/ecs/components.rs, src/systems/atmosphere.rs, src/systems/food.rs, data/status_effects.csv, registration in src/lib.rs, tests/engine_wiring_lint.rs.
- Verify: lib tests. For example: a 25 mL kerosene soak burns out inside the fuel row's range; the grip of a 6061 shaft rises more slowly than the grip of a CP-Ti shaft only if the data says so; a closed-room spin raises CO and triggers toxic_atmosphere. Then `just snapshot` of the Inventory page and the Home air card.

**F8. Close-up truth: wick material, char and motion.**
- Technique: an anisotropic woven-fibre normal and BRDF for aramid tape. The char parameter is driven by F7's accumulated burn time: fresh yellow-tan, charred outer wraps, and white fibreglass showing at worn edges, the guide's replacement signal (fire_staff_materials.md:232-235). This needs normal maps, which means adding tangents to the vertex type (mesh.rs:9-13).
- Spinning-body motion blur uses K sub-frame instances at 1/K weight, which is exact for rigid rotation. A velocity buffer plus reconstruction filter (McGuire 2012) is the later rung.
- The poi tether is a small constrained-particle (Verlet) chain.
- Verify: `firestaff-wick-closeup-0.5m` after a long burn, and `firestaff-night-4m` showing the shaft as a translucent blurred disc with a solid grip and no crisp copies.

**F9. Embers, droplets and heat haze.**
- Embers: per-particle temperature and mass, quadratic drag into the shared air field, and emission from the blackbody table scaled by T^4.
- An overloaded wick flings burning droplets on a tangent (the reason spinners do a fuel dump).
- Heat haze: copy scene colour before the transparent draw, and let the hot parcels offset UVs by curl noise scaled by temperature (Sousa, GPU Gems 2 ch. 19).
- Verify: `firestaff-static-dusk-1m` with a fence behind. Edges wobble by about a pixel, with no shimmer against open sky and no whole-screen wobble. An over-soaked spin shows tangent droplets.

**F10. Tier B live combustion grid for the nearest 1 to 4 flames.** Flagged: likely two sessions.
- Technique: a world-anchored sparse GPU grid around the performer. Port NVIDIA Flow 2.2's approach (BSD-3) to WGSL. Foundations: Stam 1999, vorticity confinement from Fedkiw/Stam/Jensen 2001, and GPU Gems 3 ch. 30.
- Its air velocity pushes the Tier A parcels. Temperature and soot are ray-marched with the cloud machinery.
- Derived cost estimate: about 1 ms per 64^3 domain on the RTX 4070. Measure it.
- Any branch that lands in the megashader must follow the PSO-class and override-switch rules. A separate pipeline avoids them.
- Verify: a hero close-up where the flame tears into curling tongues that pinch off several times a second, with no voxel stair-steps or domain box edge. Record a cost A/B via `HUMANITY_FRAME_COSTS=1`.

Outside this ladder but blocking the full vision: **a character body with skeletal animation and two hand attachment points** (today there is one "hands" slot, data/inventory/equipment_slots.json). F2 to F10 deliberately work on a kinematic pivot so they do not wait on it.

### Asteroid-impact ladder (secondary)

**A1. Impact physics as data.** Earth Impact Effects Program equations (Collins, Melosh and Marcus 2005) produce fireball radius, crater size, air-blast arrival and ejecta range from diameter, density, velocity, angle, target and distance. Register DisasterSystem through the environment region buffer (docs/PRIORITIES.md:63-66, item 2), and let a data table read `vfx_id`. Verify with lib tests against the live calculator at impact.ese.ic.ac.uk, within 20%.

**A2. Flash, then boom.** A flash light plus sky brightening at impact. Sound delayed by distance/343 m/s in the audio lane. The explosion.ogg asset does not exist yet (data/sounds.toml:118-126). Verify with vantage `impact-300m-20km`: flash first, boom about 58 s later.

**A3. Ejecta curtain.** GPU particles launched as an inverted cone at about 45 degrees. Material from farther out launches later and slower. Blanket thickness falls as distance^-3 (McGetchin 1973). Oblique impacts are lopsided. Verify: a cone, not a sphere.

**A4. Fireball and plume as Tier C baked volumes.** An offline solver run, stored as a VDB or NanoVDB sequence or a six-way flipbook, then ray-marched with the cloud machinery. Verify: no loop seam or pop, and size matches A1.

**A5. The crater.** voxel-terrain.md rung 6 (heightmap depression plus rim voxels, :82-84). This depends on that plan's rungs 1 to 5, which are unbuilt.

**A6. Aftermath.** A dust column fed into the cloud system (weather-water-roadmap.md:80-104), fires ignited through F7's system, and the existing tsunami height field (src/terrain/ocean_events.rs) wired to the disaster instead of a dev command.

---

## 5. Beyond the health bar

What the ladder makes possible, using hooks that mostly already exist:

- **Fuel choice has consequences.**
  - Camp fuel is bright and clean but short-burning, and its low flash point makes refuelling near a lit prop dangerous.
  - Kerosene burns longer but sootier, leaves more smell and residue, and gives more radiant heat on the face.
  - Lamp oil is safer to store.
  - Alcohol is nearly invisible, which is its own hazard.
  - Re-dipping a hot head can self-ignite it: lamp oil autoignites at about 200 to 216 C (fuel-use research F21).
  - All of this falls out of fuels.csv. None of it is scripted.
- **Build choices have consequences.** Shaft alloy and wall decide weight, flex and how fast the grip heats. Wick fibre decides lifespan. Aramid chars; polyester melts. Galvanised or cadmium-plated hardware near the wick gives off fumes.
- **Burns are graded injuries, not HP.** Contact and radiant exposure map to burn degrees 1 to 3 with treatment and recovery (medical.ron). The Library's treating_burns.md is the matching real-world skill.
- **Air is a resource.** Spin indoors and the room's CO and soot rise, visibility drops, and toxic_atmosphere applies. Ventilation and scrubbers matter. Smoke inhalation is a condition that can be treated.
- **Clothing matters.** The Forest Service rule the guide already cites: synthetics melt onto skin, while cotton, wool, silk and aramid do not. A flare-up in a polyester shirt is a different injury from one in cotton.
- **Safety is gameplay, and co-op.** A spotter role with a Duvetyne fire blanket, a fuel depot at the Portland FIR 3.07 distances, and a spin-off can. In multiplayer the spotter is a real job, not a menu.
- **Maintenance is visible.** A wick shows its wear. "White showing through yellow" tells you to replace it.
- **Skills that exist in data but do nothing today.** fire_making (skills.csv:4) currently awards nothing. A flow-arts / performance skill could grow into tricks and fewer mishaps.
- **Keep it intuitive.**
  - Consequence in the world first: a smoking glove, a guttering sooty flame, a coughing animation.
  - Then one short chip on the HUD. The HUD needs in-world vitals and condition warnings anyway (gameplay-loop-map.md:71).
  - Then a "why" link to the exact Library paragraph built from the same data row.
  - No meters to study before playing.

---

## 6. Decisions (operator, 2026-09-24)

The seven open questions were answered the same day. Quotations are the operator's words.

1. **Invisible gases get every method, gear or toggle.** "We generally want to support all methods as they could be gear dependent but, also could just be a toggle so we don't need to worry about gear while developing." So: a gas meter and a thermal camera as owned gear that reveal CO, CO2 and heat, AND a view setting that shows the same overlays without gear. Both draw from the same parcel and room-air data. This follows the project-wide dual-mode rule below.
2. **Fire trails are drawn "as the eye sees it".** "Longer as the eye sees it. Would likely be visually appealing." The physical vapour trail plus motion blur is the base, and a retinal-persistence extension lengthens it to match what a person sees at a real fire jam. Keep the extension a tunable data value, so it can be shortened if it reads wrong.
3. **Open source only, and we build what does not exist.** "We want to keep everything on open-source. If we have to build the system ourselves, especially if it doesn't exist in rust or whatever, then we should." No EmberGen licence. Tier C baked effects (impact fireballs, large explosions) come from our own offline solver, written in Rust/WGSL, drawing on published methods and permissively licensed references such as NVIDIA Flow (BSD-3), never on proprietary code or assets.
4. **The character body comes first, long term.** "The character body makes the most sense long term." The ladder is re-ordered: F1 (data) does not depend on a body and can go now. Then the character body arc (skinned mesh, skeletal animation, two hand attachment points instead of the single hands slot) is the prerequisite for props in hands. Pivot-mounted props are still acceptable as test fixtures for the probe rig, but not as the shipped path.
5. **Realistic available, softened by default, and compute is a hard budget.** "We want to support realistic but, for general gameplay we want to soften for gameplay. We have to be careful about introducing too much physics data. Even graphically simple games like Starmade can use tons of compute/bandwidth to the point of making the game unplayable despite having a powerful PC." So:
   - Burn and smoke severity: real mechanisms, with a realism setting whose default softens severity and recovery.
   - **Performance rules for every rung in this doc:** cosmetic effects (flame parcels, smoke, embers, haze, light flicker) are simulated locally on each client and are NEVER sent over the network. Only gameplay state is synchronised, and at a low rate: lit or unlit, fuel remaining, wick wear, burn injuries, room air values. Each effect tier has a per-frame millisecond budget measured with HUMANITY_FRAME_COSTS=1, and scales down by distance and count (Tier A for every prop, Tier B for the nearest 1 to 4 flames only, Tier C baked). Physics bodies are created only where they change gameplay.
6. **Coloured fire: yes, both ways.** Realistic mode takes colour from chemistry: each colourant is a data row (the element or salt, its emission lines and strength, its brightness penalty, and the fume hazard of the colourant and its carrier fuel), verified from primary sources before it becomes data. A creative toggle allows any colour, for people who just want purple fire.
7. **Calibration footage: later.** The operator will find someone in the Fire Disciples group with a 240 fps camera. Until then the derived estimates (about 2.5 kW per wick, 250 to 400 lm, trail length, flicker rate) stay labelled as estimates in the data.

**The dual-mode rule behind answers 1 and 5.** Every deep system ships with a full-realism mode and a simplified mode, the way the simple health bar was built first and the full vitals depth sits behind it. Gear-gated features also get a toggle so development and accessibility never depend on owning the gear.
