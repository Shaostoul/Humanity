# Telecommunications -- data media (copper / WiFi / fiber / ...) with real specs + consequences

> **Status:** Stage 1 SHIPPED (v0.619); Stages 2-4 are DESIGN. Stage 1 shipped a LEANER field set than
> the fuller proposal below -- `ConduitType` gained `bandwidth_mbps`, `range_m`, `latency_ms`, `wireless`,
> `rf_emission` (all `#[serde(default)]`) + `ConductorMaterial::{Glass, Radio}`, plus `check_data_link` /
> `cheapest_data_link_for` / `data_media` in `src/utilities.rs`, and three media in `conduits.ron`
> (`eth_cat6`, `fiber_om4`, `wifi_6`). The fuller proposal's `class: DataClass` + `interference` + `secure`
> fields and the 21-media catalog (coax, single-mode fibre, Bluetooth, Zigbee, LoRa, cellular, satellite,
> PLC, Li-Fi) land in later stages. Extends the power/water/air wiring system -- read
> `docs/design/utility-wiring.md` FIRST; this doc assumes it. Data is `Utility::Data`, already in the
> closed enum (`src/utilities.rs`), routed through the SAME `conduits.ron` registry + machine ports +
> buildability checks. Companions: `docs/design/conduits-node-graph.md` (the routing graph),
> `docs/design/sim-realism-roadmap.md` (the realism gaps this closes). The teaching goal: real
> telecom -- bandwidth, range, latency, RF emission, interference, security -- so a player learns to
> pick the right medium, and the tradeoffs BITE (WiFi RF harms a sensitive crop; emissions give you
> away to enemies + reveal them to you).

## The rule

Data does NOT magically transmit. It travels through real **media** -- copper ethernet, coax, fiber
optic, WiFi, Bluetooth, cellular, satellite, powerline, Li-Fi -- each with real limits: bandwidth
(Mbps/Gbps), range (m), latency, **RF emission**, interference susceptibility, and security. A medium
is a `ConduitType` row with `utility: Data` in `data/utilities/conduits.ron` (infinite-of-X: add a
row, no code), a machine declares Data IN/OUT `Port`s (a demand in Mbps), and a data run is validated
by `check_data_link` exactly like a power run is validated by `check_cable`. Wireless media (WiFi /
Bluetooth / cellular / satellite) carry NO physical cable but DO emit RF -- and that emission is a
real, consequential signal: it harms a nearby sensitive plant, and it is a **detection signature** an
enemy can sense (and a sensor you carry can sense theirs). Wired/optical media are quiet -- pick wired
to protect a grow or to stay dark.

## Data model (`src/utilities.rs`)

`Utility::Data` already exists (closed enum). The media are `ConduitType` rows; the new fields below
are ADDITIVE + `#[serde(default)]` so every existing electrical/water row in `conduits.ron` parses
unchanged (the non-negotiable migration rule). `ConductorMaterial` gains two variants:

- **`Glass`** -- optical fiber (light, not current; `ohm_per_m`/`ampacity_a` are 0/irrelevant).
- **`Radio`** -- a wireless "medium" with no physical conductor (WiFi/BT/cellular/satellite). It is a
  `ConduitType` so the picker + buildability share one code path, even though nothing is run as a cable.

(Copper stays for ethernet/coax/powerline; Superconductor is unchanged.)

### New `ConduitType` fields (data media)

```rust
// All #[serde(default)] -- omitted on electrical/water rows -> 0/false, parse unchanged.
pub class: DataClass,        // Wired | Wireless | Optical | Powerline | Other (default Wired)
pub bandwidth_mbps: f32,     // sustained throughput in Mbps (1 Gbps = 1000.0)
pub range_m: f32,            // max usable run (wired) or radius (wireless), metres; 0 = N/A
pub latency_ms: f32,        // one-way nominal latency at nominal length
pub wireless: bool,          // true => emits RF, no physical cable to route
pub rf_emission: f32,        // 0.0 (quiet: fiber/wired) .. 1.0 (loud: high-power WiFi/cell)
pub interference: f32,       // susceptibility 0.0 (immune: fiber) .. 1.0 (very: 2.4 GHz / PLC)
pub secure: bool,            // hard to passively tap (fiber/wired in-conduit) vs sniffable (radio)
```

`DataClass` is a small closed enum (like `Grade`): `Wired`, `Wireless`, `Optical`, `Powerline`,
`Other`. The existing electrical/water fields (`awg`, `ampacity_a`, `ohm_per_m`, `diameter_mm`,
`flow_max_lpm`) stay 0 on a data row; a data row stays 0 on the electrical/fluid fields. One struct,
one registry -- no parallel architecture.

## The media catalog (`data/utilities/conduits.ron`)

Real-ish numbers (approximate, teaching-grade -- not lab-exact). `rf` = `rf_emission` 0..1.
`intf` = `interference` susceptibility 0..1. Cost is per-metre for wired/optical, per-unit (the
radio/endpoint) for wireless.

| id | label | class | bandwidth | range (m) | latency | rf | intf | secure | cost | one-line pros/cons |
|----|-------|-------|-----------|-----------|---------|----|------|--------|------|--------------------|
| `cat5e` | Cat5e ethernet | Wired | 1000 Mbps | 100 | ~0.5 ms | 0.02 | 0.2 | yes | 0.4/m | Cheap, ubiquitous 1 Gbps to 100 m. Twisted-pair EMI pickup; quiet. |
| `cat6` | Cat6 ethernet | Wired | 1000 Mbps (10G to 55 m) | 100 | ~0.5 ms | 0.02 | 0.15 | yes | 0.7/m | 10 Gbps on short runs; better crosstalk than 5e. Stiffer. |
| `cat6a` | Cat6a ethernet | Wired | 10000 Mbps | 100 | ~0.5 ms | 0.02 | 0.1 | yes | 1.3/m | Full 10 Gbps to 100 m, shielded. Thick, pricier, harder to pull. |
| `cat7` | Cat7 ethernet (S/FTP) | Wired | 10000 Mbps | 100 | ~0.5 ms | 0.01 | 0.05 | yes | 2.0/m | Fully shielded 10 Gbps; great EMI rejection. Bulky, needs GG45. |
| `coax_rg6` | Coax RG6 | Wired | 1000 Mbps (DOCSIS) | 300 | ~1 ms | 0.03 | 0.15 | yes | 0.6/m | Long shielded runs, CATV/cable-modem. Shared-medium contention. |
| `fiber_om4` | Fiber multimode OM4 | Optical | 10000-100000 Mbps | 400 | ~0.005 ms/km | 0.0 | 0.0 | yes | 1.6/m | Huge bandwidth, EMI-immune, quiet. Short reach; needs transceivers. |
| `fiber_smf` | Fiber single-mode | Optical | 100000+ Mbps | 40000 | ~0.005 ms/km | 0.0 | 0.0 | yes | 1.2/m | Effectively unlimited reach + bandwidth, silent, tap-resistant. Costly optics/splices. |
| `wifi4` | WiFi 4 (802.11n 2.4 GHz) | Wireless | 150 Mbps | 35 indoor | ~10 ms | 0.5 | 0.8 | no | 25/unit | Cheap, wall-penetrating 2.4 GHz. Crowded band; sniffable; emits RF. |
| `wifi5` | WiFi 5 (802.11ac 5 GHz) | Wireless | 1000 Mbps | 25 indoor | ~5 ms | 0.45 | 0.4 | no | 40/unit | Fast 5 GHz, less congestion. Shorter range; walls hurt; RF. |
| `wifi6` | WiFi 6 (802.11ax) | Wireless | 1200 Mbps | 30 indoor | ~4 ms | 0.45 | 0.35 | no | 60/unit | High density + efficiency. Still RF-loud; sniffable. |
| `wifi6e` | WiFi 6E (6 GHz) | Wireless | 2000 Mbps | 20 indoor | ~3 ms | 0.4 | 0.2 | no | 80/unit | Clean 6 GHz spectrum. Very short range; walls kill it. |
| `wifi7` | WiFi 7 (802.11be) | Wireless | 5000 Mbps | 20 indoor | ~2 ms | 0.4 | 0.2 | no | 120/unit | Multi-link, very fast. Costly; short range; RF. |
| `bluetooth` | Bluetooth Classic | Wireless | 3 Mbps | 10 | ~30 ms | 0.15 | 0.5 | no | 8/unit | Cheap short-link peripherals. Tiny bandwidth; RF beacon. |
| `ble` | Bluetooth LE | Wireless | 2 Mbps | 30 | ~30 ms | 0.08 | 0.5 | no | 6/unit | Very low power, low RF. Telemetry only; weak signature but trackable. |
| `zigbee` | Zigbee (802.15.4) | Wireless | 0.25 Mbps | 70 mesh | ~30 ms | 0.1 | 0.6 | partial | 10/unit | Low-power mesh for sensors/controls. Tiny throughput; 2.4 GHz contention. |
| `lora` | LoRa | Wireless | 0.05 Mbps | 5000 | ~1000 ms | 0.2 | 0.1 | partial | 15/unit | Kilometres on milliwatts -- telemetry/IoT. Bytes-per-second; high latency; emits RF. |
| `cell_4g` | Cellular 4G LTE | Wireless | 100 Mbps | 5000 | ~40 ms | 0.6 | 0.3 | partial | 90/unit | Wide-area mobile data. Needs tower/uplink; loud RF; carrier-trusted. |
| `cell_5g` | Cellular 5G | Wireless | 1000 Mbps | 1000 (mmWave shorter) | ~15 ms | 0.6 | 0.3 | partial | 140/unit | Fast wide-area; low latency. mmWave is line-of-sight + short; loud RF. |
| `satellite` | LEO satellite | Wireless | 200 Mbps | 1000000 | ~30 ms (LEO) | 0.7 | 0.2 | partial | 300/unit | Anywhere-on-Earth uplink. Needs sky view; high power; very loud RF signature. |
| `plc` | Powerline (PLC/HomePlug) | Powerline | 200 Mbps | 100 | ~5 ms | 0.25 | 0.7 | no | 30/unit | Data over existing power wiring -- no new cable. Noisy, shared, leaks RF from mains. |
| `lifi` | Li-Fi (visible light) | Optical | 1000 Mbps | 10 | ~1 ms | 0.0 | 0.05 | yes | 70/unit | Light-based, ZERO RF, can't pass walls -> physically contained + quiet. Line-of-sight only; daylight swamps it. |

Notes baked into the numbers: 2.4 GHz media (`wifi4`, `bluetooth`, `zigbee`) share a band so
`interference` is high; fiber + Li-Fi are `rf_emission` 0 (the quiet/stealth choices); satellite +
cellular are the loudest RF; LoRa trades bandwidth for kilometres of range; PLC reuses the power run
(no new cable) but is noisy and leaks RF off the mains.

## Link physics (`check_data_link`, mirrors `check_cable`)

```rust
pub enum DataVerdict { Pass, Warn, Fail }
pub struct DataLinkCheck { pub verdict: DataVerdict, pub utilization: f32, pub reason: String }

pub fn check_data_link(medium: &ConduitType, demand_mbps: f32, length_m: f32) -> DataLinkCheck
```

Same Pass/Warn/Fail shape + thresholds family as `check_cable`:

- **Utility-guard:** if `medium.utility != Utility::Data` -> `Fail` (mirrors the cable's
  not-electrical guard).
- **Bandwidth (the ampacity analogue):** `utilization = demand_mbps / bandwidth_mbps`.
  - `Pass` if `utilization <= 0.8` (the 80% headroom rule, same continuous-load derate as NEC copper).
  - `Warn` if `0.8 < utilization <= 1.0` (carries it but saturated -- a real install up-sizes).
  - `Fail` if `utilization > 1.0` (over capacity -- packets drop).
- **Range (the voltage-drop analogue):** let `r = length_m / range_m`.
  - `Fail` if `r > 1.0` (beyond the medium's reach -- no link).
  - `Warn` if `r > 0.8` (marginal: a long copper run or a far wireless edge -- signal degrades).
  - else range contributes `Pass`.
- The verdict is the WORST of the bandwidth + range verdicts (Fail beats Warn beats Pass), exactly
  like `check_cable` takes the worst of ampacity + drop.
- `reason` is a human string, e.g. `"850 of 1000 Mbps (85%) over 90 m of Cat6 -- saturated"`.

```rust
pub fn cheapest_medium_for(demand_mbps: f32, length_m: f32, allow_wireless: bool)
    -> Option<&'static ConduitType>
```

The auto-picker (mirrors `cheapest_cable_for`): cheapest `Utility::Data` row that `Pass`es the demand
over the length. `allow_wireless` lets a caller force a WIRED/optical pick (the "protect my grow"
button: exclude `wireless` media so nothing emits RF near the crops).

## Consequence chains (the point)

### (a) RF emission -> plant harm

The downstream consequence, hooked into `FarmingSystem` the SAME way the v0.611 water->food coupling
is (`src/systems/farming/mod.rs`, the `water_available` gate). A wireless emitter (a WiFi router,
cellular modem, satellite dish) near a sensitive crop slows its growth + drains its health.

- **Emitter source:** any spawned machine whose chosen Data medium is `wireless` carries an
  `RfEmitter { level, radius_m }` component (new, in `src/ecs/components.rs`), where `level =
  medium.rf_emission` and `radius_m` derives from `medium.range_m` (RF falls off with distance, so
  harm scales with `1 - dist/radius`). Wired/optical media spawn NO emitter -> zero harm (the
  teaching payoff: route a cable and your grow is safe).
- **"Near" detection:** a crop is affected if it sits within an emitter's `radius_m`. Crops in towers
  have a world position via the home placement (`MachineHome::placements`); the FarmingSystem already
  reads per-tower context by `tower_id`. Simplest first cut (Stage 3): publish a per-tower RF dose
  (a `HashMap<String, f32>` keyed by `tower_id`, like `garden_irrigation`/`garden_nutrient`) computed
  from emitters near each tower's position; the FarmingSystem reads `rf_dose.get(tower_id)`. A later
  cut can do true per-crop world-distance once seed-planted (non-tower) crops carry a position.
- **Harm model:** mirror the water-stress path. With dose `d` (0..1, the emitter `level` scaled by
  proximity, summed + clamped over nearby emitters):
  - health drains at `RF_HARM_RATE * d * dt` (a new const, tuned so a loud router at point-blank
    visibly wilts a sensitive crop over a couple of in-game days -- never instant, like the air
    drain), AND
  - growth is scaled by an `rf_factor = (1 - RF_GROWTH_PENALTY * d).max(floor)` multiplier folded in
    next to the existing `health_factor` / `nutrient_factor` in the growth-progress calc.
  - Tolerance is per-species (see Open questions): a `rf_tolerance` column on `plants.csv`
    (`#[serde(default)] = 1.0`, fully tolerant) scales `d` down, so "sensitive" crops (e.g. a
    delicate medicinal) suffer while a hardy crop shrugs it off. Default keeps every existing plant
    row + test unchanged.
- **The lesson:** the player who puts a WiFi router in the grow room watches yields fall, moves to
  Cat6 or Li-Fi (rf 0), and recovers -- learning a real (if dramatized) EMF-vs-sensitive-systems
  tradeoff.

### (b) Emissions-as-signature -> detection (the awareness layer)

Generalize "RF is a real signal" into a stealth/awareness layer. This is the BRIDGE into the
otherwise-deferred combat work -- framed as **awareness, not weapons**: who can sense whom, not who
shoots whom.

- **`Signature { kind, strength, radius_m }`** (new component): a thing an entity EMITS. `kind` is a
  small closed enum `SignatureKind { Rf, Bluetooth, Pheromone, Thermal, Acoustic }` (extensible the
  way `Utility` is -- distinct physics each, so it is code not data; the per-emitter *instances* are
  data). RF + Bluetooth signatures come straight from the chosen wireless medium's `rf_emission`
  (Bluetooth/BLE are their own low-strength kind). **Pheromones** are a future `kind` an organism (a
  creature, a flowering crop, the player after certain actions) emits -- "other such things" plug in
  here as new `SignatureKind`s with no new system.
- **`Sensor { kind, sensitivity, range_m }`** (new component): a thing an entity DETECTS with. A
  player's scanner detects `Rf` (find the enemy's router/radio); a predator's nose detects
  `Pheromone`; a bug detects `Acoustic`. An entity can carry several sensors.
- **Detection rule (a new `SignatureSystem`, sibling of `AtmosphereSystem`):** entity B with a
  `Sensor{kind,sensitivity,range_m}` detects entity A's `Signature{kind,strength,radius_m}` of the
  same `kind` when `dist(A,B) <= min(range_m, radius_m)` AND `strength * (1 - dist/radius) *
  sensitivity >= DETECT_THRESHOLD`. Symmetric: a player running loud WiFi is easy to find; a player
  on fiber/Li-Fi (rf 0) is invisible to an RF sensor. The system publishes a per-observer "contacts"
  list (the same neutral DataStore-channel pattern the other systems use -- no GUI type in the sim)
  for an awareness HUD / minimap blips.
- **Why this is the combat bridge, safely:** it ships the *sensing* half (detection signatures,
  stealth choices, "go dark by going wired") with zero weapon/damage code. Combat, when it comes,
  consumes this layer (you can only engage what you can detect) instead of inventing its own.

Both chains reuse existing patterns -- no parallel architecture: (a) is the farming gate pattern, (b)
is the atmosphere-system pattern (a `System` that reads components + publishes a status to the
DataStore).

## Staged build plan

- **Stage 1 -- the foundation (smallest valuable slice, ship FIRST; mirrors wiring v0.604).** Add the
  data fields to `ConduitType` + the `DataClass` enum + the `Glass`/`Radio` `ConductorMaterial`
  variants; add 3-5 media rows to `conduits.ron`; implement `check_data_link` + `cheapest_medium_for`;
  full unit tests (registry parses, every existing row still parses unchanged, Pass/Warn/Fail on
  bandwidth + range, the picker scales with demand + respects `allow_wireless`). PURE data + physics,
  NOT wired into machines -- zero risk to existing data/behaviour, exactly like wiring Stage 1. **This
  is the slice the orchestrator builds next.**
- **Stage 2 -- machines + picker + buildability (mirrors wiring v0.605).** `MachineDef` gains nothing
  new (it already has `ports: Vec<Port>`); machines that move data declare a Data `Port` with a
  bandwidth demand carried in a new `Port.bandwidth_mbps` field (`#[serde(default)]`). Add a Data
  buildability check to `buildability_report()` next to "Conduits"/"Power circuit": every Data run
  (a `connection`/`conduit_edge` of `kind == "data"`) is validated/auto-sized by `check_data_link`
  over the run length, and (the union-find already in `utility_component_roots`, generic over `kind`)
  every Data consumer must reach a Data source (a router/uplink). A "data connection" medium picker in
  the editor mirrors the cable picker.
- **Stage 3 -- RF -> plant harm.** The `RfEmitter` component + the per-tower RF-dose channel + the
  `rf_tolerance` plants.csv column + the FarmingSystem hook (health drain + growth penalty), with a
  test mirroring `dry_cistern_stops_irrigation_and_wilts_crops`: a loud emitter beside a sensitive
  tower wilts it; a wired run (no emitter) does not.
- **Stage 4 -- emissions-as-signature detection + pheromones.** `Signature` + `Sensor` components, the
  `SignatureKind` enum (Rf/Bluetooth/Pheromone/Thermal/Acoustic), the `SignatureSystem`, the contacts
  channel + awareness HUD. Pheromones (and other future kinds) are new enum variants on the shipped
  system. This is the combat-awareness bridge.

## Open design questions (for the operator)

1. **RF tolerance: per-species or global?** Proposed: per-species `rf_tolerance` on plants.csv
   (default 1.0 = fully tolerant) so only flagged "sensitive" crops suffer. Alternative: a single
   global sensitivity. Per-species is more realistic + more interesting, at the cost of authoring a
   column. (Leaning per-species.)
2. **Fiber transceivers as machines at each end?** Real fiber needs a media converter / SFP at both
   ends. Model that as a required endpoint machine (more realistic, more build cost, a real teaching
   point) or fold it into the fiber medium's per-metre cost (simpler)? (Leaning fold-in for Stage 1,
   transceiver machines as a later realism pass.)
3. **Wireless range vs walls.** Should a wall attenuate a wireless link's effective `range_m` (and an
   RF signature's `radius_m`)? The home has wall geometry (HomeStructure). Realistic but needs a
   ray/occlusion test. Stage 1-3 can use plain radius; Stage 4 detection is where occlusion matters
   most (a wall should hide you). How much do walls block -- per-band attenuation, or a flat factor?
4. **Does PLC ride the existing power run, or need its own data edge?** Realistically PLC = data over
   the power wiring (no new cable). Model it as: a `data` link is satisfied for free where a `power`
   connection already exists between the two machines? That is a neat teaching reward (reuse your
   power wiring) but couples two graphs.
5. **Latency: cosmetic or consequential?** Bandwidth + range gate buildability. Should latency ever
   FAIL a link (e.g. a real-time control machine that LoRa's ~1 s latency can't serve), or stay an
   informational stat? (Leaning: a `Port` could carry a `max_latency_ms`; only some machines care.)
6. **Signature detection: does the player emit by default?** Carrying a phone/radio = an RF
   signature. Should the player auto-emit from equipped gear (so "go dark" means stripping radios), or
   only from placed machines? (Leaning: equipped gear emits -- it makes the stealth layer personal.)
