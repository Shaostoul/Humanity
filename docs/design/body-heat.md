# Body heat

Written 2026-09-27. Code: `src/systems/body_heat.rs` (the model and its
tests in `body_heat_tests.rs`), `src/systems/food.rs` (the vitals pass that
runs it), `src/engine/survival_env.rs` (the inputs).

## Why

The core temperature used to move straight toward the air temperature at
0.5 C a second. There was no body heat in it, no clothes, no wind, no wet, no
sun. A player who stepped outside on a mild 15 C day crossed 35 C (hypothermia)
in 4.5 seconds and, on the old damage constants, died about 47 seconds later.
A real person in ordinary clothes is comfortable at 15 C all day. A coat did
not keep anyone warm either: it only scaled the freezing damage down.

## The model

The Gagge two-node model: Gagge, Stolwijk and Nishi 1971, "An effective
temperature scale based on a simple model of human physiological regulatory
response", ASHRAE Transactions 77(1), in the form of Gagge, Fobelets and
Berglund 1986, "A standard predictive index of human response to the thermal
environment", ASHRAE Transactions 92(2B). It is the model ASHRAE Standard 55
and ASHRAE Handbook Fundamentals chapter 9 use. The coefficients are the ones
the Center for the Built Environment's open-source `pythermalcomfort` codes as
`two_nodes_gagge` (read 2026-09-27), and the code names them.

Two layers, a core and a skin. Per square metre of skin (a 70 kg person with
1.8258 m2 of skin):

- Metabolic heat `M = met x 58.2 + shivering` (W/m2).
- Heat the blood carries from core to skin: `(T_core - T_skin)(5.28 + 1.163 x blood_flow)`.
- Dry heat through the clothes to the surroundings:
  `(T_skin - T_op) / (R_cl + R_air)`, where `R_cl = 0.155 x clo`,
  `R_air = 1 / (f_cl (h_c + h_r))`, `f_cl = 1 + 0.15 x clo`,
  `T_op = (h_r T_rad + h_c T_air) / (h_c + h_r)`.
  Convection `h_c` is the largest of natural (`3.0 p^0.53`), wind-forced
  (`8.6 (v p)^0.53`) and the body's own movement (`5.66 (met - 0.85)^0.39`,
  ASHRAE 55). Radiation `h_r = 4 x 0.95 x sigma x ((T_cl + T_rad)/2 + 273.15)^3 x 0.73`,
  iterated with the clothing surface temperature `T_cl` to 0.01 C.
- Breathing: `0.0023 M (44 - p_air)` latent plus `0.0014 M (34 - T_air)` dry.
- Sweat evaporation, limited by what the air can take:
  `E_max = (p_sat(T_skin) - p_air) / (R_e,air + R_e,cl)` with the Lewis relation
  (2.2 K/mmHg at sea level) and the clothing permeation efficiency 0.45.
- The two temperatures move with the layer's heat storage and heat capacity
  (0.97 W h/kg/K, the skin layer's share of the mass growing as the vessels
  narrow).
- The responses: skin blood flow `(6.3 + 120 warm_core) / (1 + 0.5 cold_skin)`,
  0.5 to 90 L/m2/h; sweat `170 warm_body e^(warm_skin/10.7)`, at most 500
  g/m2/h; shivering `19.4 cold_skin cold_core` W/m2.

Integrated with steps of at most ten seconds, kept in f64 (a degree an hour is
below an f32's resolution at 37 C at 60 frames a second).

### What is added to Gagge

Gagge built the model for rooms. A game needs a person to be caught out in the
cold, so four things are added, each with its source:

| Addition | Value | Source |
|---|---|---|
| Shivering ceiling | peak 4.9 x resting metabolism | Eyolfson et al. 2001, Eur J Appl Physiol 84:100-106 |
| Shivering tires | the drive falls 17 percent an hour past its endurance | Tikuisis et al. 2002, Eur J Appl Physiol 87:50-58 |
| Shivering stops | fades out as the core passes 32 to 30 C | StatPearls, "Hypothermia" (as cited in the Library's cold_and_hypothermia guide) |
| Wet clothes insulate less | soaked clothing loses about 30 percent | Zhao et al. 2025, Building and Environment 267:112299 ("almost 30%", from its abstract as indexed; the paper itself not read) |
| Wet clothes cool as they dry | half the latent heat is drawn from the body | Havenith et al. 2013, J Appl Physiol 114:778-785: 28 percent lost for a wet base layer, over 62 percent for a wet outer layer |
| Wind at the body | two thirds of the 10 m weather wind | the 2001 wind chill chart's reduction (Osczevski and Bluestein 2005, BAMS 86:1453) |

### What is calibrated, not published

Three numbers: the share of the peak a person SUSTAINS (0.4), the shivering
endurance (0.1 hours at the full peak), and the intensity exponent (lighter
shivering lasts as the cube of how much lighter it is). They were chosen
together so the model reproduces:

| Case | Literature | Model |
|---|---|---|
| Bare, still, calm air, 0 C, to a 28 C core | CESM 9.0 h (Tikuisis 1995, Int J Biometeorol 39:94-102; survival ends at 28 C) | 9.6 h |
| Same at 10 C | CESM more than 24 h | 24.7 h |
| Wet light clothes, 5 C, wind, still, 3 h | core fell 0.25 C (Helland et al. 2025, Wilderness Environ Med, doi 10.1177/10806032251378099) | fell 0.24 C |
| Walking in 5 C rain without rainwear | most held their core 4 h, rapid decline after as shivering tires (Thompson and Hayward 1996, J Appl Physiol 81:1128-1137) | 36.8 C after 5 h |
| Bare, calm, -10 C / -20 C | CESM 4.1 h / 2.5 h | 5.5 h / 4.0 h |

The last row is the model's known limit: one skin layer and no limbs, so in
severe cold it cools slower than the multi-layer CESM. It is inside the
literature where the game spends its time (above about -10 C in clothes).

### What it gives

In the everyday outfit (trousers and a long-sleeve shirt, 0.61 clo):

| Situation | 35 C (hypothermia) | 28 C (severe) |
|---|---|---|
| 15 C still air, walking or standing | never (36.95 / 36.78 C after 8 h) | never |
| 0 C, 5 m/s wind, standing | 9.8 h | 14.3 h |
| 0 C, 5 m/s wind, walking | never | never |
| 5 C, 3.3 m/s wind, soaked by rain, standing | 7.3 h | 11.1 h |
| 5 C, same wind, dry | never | never |
| -20 C, 5 m/s wind, standing | 3.0 h | 4.8 h |
| -20 C, same, plus the winter coat (0.70 clo) | 6.7 h | never in 8 h |
| 35 C, 50 percent humidity, 0.5 clo, heavy work (4 met) | core 39.0 C at 1 h, 40.3 C (heatstroke) at 2 h | |

`cargo test --features native --lib reference_figures -- --ignored --nocapture`
prints this table from the code.

## Thresholds and harm

The cold bands are the ones the Library's cold_and_hypothermia guide cites:
hypothermia below 35 C (CDC), mild 32 to 35, moderate 28 to 32, severe below
28 (StatPearls). The heat bands are the Merck Manual Professional's: heat
exhaustion usually below 40 C with no change in mental state, heatstroke above
40 C with altered mental status. Hard exercise alone puts a healthy core at 38
to 39, so the game's heat exhaustion onset is 39.

| Core | Condition | Harm |
|---|---|---|
| below 35 C | `hypothermia` (movement halved) | none |
| below 32 C | | 100 health points over 4 h per degree below 32 (1 h at 28, 30 min at 24): a game choice on the bands |
| above 39 C | `heat_exhaustion` (movement 0.8) | none |
| above 40 C | `heat_stroke` (movement halved) | 100 points in 1 h per degree above 40: a game choice |
| shivering hard | `shivering` (no penalty; a warning) | none |

## Two modes

The house rule for deep systems (CLAUDE.md, "Dual modes"), in
Settings > Gameplay > Body heat:

- **Forgiving**, the default. The same physics, so clothes, shelter, wind and
  wet count exactly as much, but the core shown (and acted on) swings half as
  far from normal, and harm comes at half the rate. The garden's Gentle mode is
  the same "half" rule. Because the Forgiving core is always closer to normal
  and its harm is halved, it cannot kill sooner than Realistic; a test holds
  that on a grid of weather, clothes and work anyway.
- **Realistic**. The model's core as it is.

Only the core temperature is saved (in `Vitals`); the rest of the body state
restarts from it on a load, and a respawn (which sets the core back to 37 C)
starts the body over.

## Inputs

`EnvironmentContext`, published each frame by `engine::survival_env`:

- Inside the sealed home: the home's own air (temperature, humidity, pressure
  from its enclosed space), still, nothing falling.
- Outside: the weather at the player's position (`temperature_at_player`,
  humidity, the 10 m wind at the player, rain at the weather's intensity, snow
  at a third of it) and the air pressure there, or vacuum where there is no air.
  Since 2026-09-27 whether it is rain or snow is decided by the air at the
  player, not the condition's name (`systems::precipitation`, Jennings et al.
  2018's rain-snow model), and nothing falls where there is no air. The third
  is a game choice, unsourced (`SNOW_WETTING_SHARE`).
  Since 2026-09-27 the temperature, pressure and wind are environment Layer 1
  where the player stands (latitude, altitude, land or sea, time of year, the
  prevailing wind) with the weather's condition as the deviation on top
  (`docs/design/environment-fields.md`, "Layer 1 as built").
- Activity from the movement keys: standing 1.2 met, walking 2.0, sprinting
  3.8 (ASHRAE Table 4; the game's walking pace is faster than a real walk, so
  the gait is billed, not the speed); driving 1.5; asleep in a bed 0.7.
- Clothing: the everyday outfit (0.61 clo, the ASHRAE ensemble "trousers,
  long-sleeve shirt") plus each worn item's `clo` (data/equipment.csv, sources
  in its header). Garment values are summed.
- **`sheltered`**: under a roof and out of the wind, still air and nothing
  falling, at the outside air temperature (a shelter with no fire is as cold
  as the weather; it stops the wind and the wet). SET SINCE 2026-09-27 by the
  built pieces: outside the home, `construction::uses::shelter_at` finds a
  finished `shelter` piece straight overhead (the roof) and finished `shelter`
  pieces on at least three of the four sides within half a metre of the edge
  of the covered area (the roof overhead and every roof touching it, so a
  hall under several roof tiles counts), and
  `engine::survival_env::outside_context` passes that on. It works on a
  planet's ground as well as aboard: the test runs in the build site the
  player stands in (`construction::site`, `engine::planet_build`), which is
  where the weather is.
  `Exposure::from_context` then zeroes the wind and the precipitation, so the
  air at the body is still and the clothes stay dry. A roof with fewer walls
  is not `sheltered`, but `outside_context` still zeroes the precipitation
  under it: rain off, wind in. Three sides, not four, is
  a game choice: the open side is the way in (doors cannot be set into walls
  yet), as in a lean-to. On the wet, windy 5 C case above, standing in the
  everyday outfit, the core is 36.66 C after 6 h under a roof on three walls
  and 36.28 C soaked in the open
  (`a_built_shelter_keeps_a_wet_windy_5c_day_off_the_body`). What it does not
  know yet: which way the wind blows (the weather has a direction, so an open
  side facing into the wind could count against it, and a wall on the lee
  side matters less than one on the windward side), and rain driven sideways
  under the eaves. (The wind's side is in since 2026-09-28,
  `ShelterCheck::wind_share`, and so is a roof's radiant warmth, below.)
- `radiant_temp_c`: the surroundings' radiant temperature. None means the
  air's. Since 2026-09-28 (v0.1416.0), in open air that can be
  breathed and with no roof overhead, it is the clear night sky's
  (`body_heat::open_sky_radiant_c`): half the view is ground at the air's
  temperature and half is sky, at Swinbank's clear-sky temperature
  (`0.0552 * T_air^1.5` K, Swinbank 1963) where it is clear and at the air's
  where cloud covers it (`Weather::cloud_share`), mixed as fourth powers.
  On a clear 10 C night that is about 0.5 C. (v0.1416.0 let this in only
  at night, as a stand-in until the sun was modelled; v0.1417.0 replaced
  that with the sun itself.) By day the sun's rise is added on top
  (`sun_mrt_rise_c`, ASHRAE 55-2020 Appendix C "SolarCal", with the
  constants pythermalcomfort's `solar_gain` uses: h_r 6 W/m2 K, f_eff 0.725
  standing, short-wave absorptivity 0.7, long-wave 0.95, diffuse light 0.2
  of the beam): the diffuse sky light, the direct beam on the body's
  projected area (`projected_area_factor`, Fanger's
  `0.308 cos(b (0.998 - b^2/50000))`, within 0.003 of pythermalcomfort's
  standing table averaged over the azimuths) and the light the ground
  reflects (albedo 0.2 outdoors; SolarCal's 0.6 is an indoor floor). The
  beam is Meinel's clear-sky model, `1353 * 0.7^(AM^0.678)` W/m2 with Kasten
  and Young's air mass (about 950 W/m2 overhead, 430 at 10 degrees), cloud
  taking it away and the diffuse light kept. The sun's height is the
  gameplay sun's (`solar::sun_factor` of the local solar hour, the arc the
  crops and panels use). In 20 C air a clear sun overhead makes about 47 C
  (the sky's 12.3 C plus a 34.7 C rise) and an overcast noon about 34 C
  (`the_open_sky_by_night_and_by_day`). Under a roof, walls or not, the
  surroundings are the air's: the roof radiates at about the air's
  temperature, hides the zenith, the sky's coldest part, and shades the
  body. By day that is shade: at a clear, dry 30 C noon in a light breeze,
  after two hours standing the core is about a quarter of a degree warmer in
  the sun than under a roof and the skin about 1.5 C warmer
  (`a_roof_is_shade_at_noon`). By night it is a shelter's radiant warmth; on
  the calm, clear 10 C night in the everyday outfit, after six hours the
  core is nearly the same either way (36.70 C under a roof, 36.67 C in the
  open), but the skin is about 1.9 C warmer under the roof (25.5 C against
  23.6 C) and the body shivers about 40 percent less
  (`a_roof_keeps_the_night_sky_off_the_body`).

## Not modelled yet

- A fire's radiant heat (the `radiant_temp_c` input is there for it; the
  `campfire_warmth` effect is the obvious first user).
- For the sky and the sun: humidity's effect on the night sky (Brunt's and
  Idso's formulas use the vapour pressure; Swinbank's does not); the sun
  coming in under a roof's eaves through the open sides when it is low, and
  the sky light that comes in the same way (a roof is taken as full shade);
  the ground warmer than the air in the sun; and the sun's real height,
  which follows the gameplay sun (up at 6, down at 18, overhead at noon at
  every latitude) until the latitude question is settled.
- Sweat soaking clothes (only rain wets them), and sweat costing water: a
  litre an hour of hard work in heat should come out of hydration.
- Shivering and activity costing food energy.
- Frostbite: in severe cold the model's mean skin goes below 0 C with no
  penalty; local cold injury is its own system.
- Surface pressures of worlds with air but no climate row (they take Earth's
  standard column; the air supply rules them first). Mars has its own since
  2026-09-27.
- The bed's bedding insulation while asleep.
