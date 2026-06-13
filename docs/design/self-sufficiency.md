# Homestead self-sufficiency: the variables, and a model for the score

> Status: **reference + proposal, 2026-06-07.** Written because the operator asked
> "what all variables are there to consider?" for homestead self-sufficiency, so we
> can design the closure / self-sufficiency *score* (improvement #4 in
> `homes-as-profiles.md`) on an honest model instead of a naive watts ratio. Nothing
> here is built yet; this is the thinking that the score data layer will encode.

## The one-sentence answer

Self-sufficiency is **not** a single number you read off a spec sheet. It is the
fraction of your needs you can meet **from inside the homestead, averaged over time,
through the worst stretch the climate throws at you**, across several **coupled
loops** (energy, water, food, waste, air, heat), each gated by **where you are** and
**how many mouths** you feed. You are only as self-sufficient as your **weakest
loop** (Liebig's law of the minimum: a plant grows only as tall as its scarcest
nutrient allows).

## The loops (each is a supply / demand / storage balance over time)

For every loop the same five things matter: **supply** (what you make/collect),
**demand** (what you consume), **storage** (buffer to ride the gaps), **losses**
(conversion/leak/spoilage), and the **time window** over which it has to balance.

### 1. Energy (power + heat are related but track them separately)
- **Supply:** solar (panel area x efficiency x irradiance x **sun-hours**, which swing
  with latitude + season + weather), wind (swept area x windspeed^3 x air density),
  micro-hydro (flow x head, the most constant if you have it), biomass/biogas,
  generator (fuel-dependent backup).
- **Storage:** batteries (usable kWh after depth-of-discharge + round-trip efficiency
  + degradation over years), thermal mass, pumped/flywheel. **Days of autonomy** =
  storage / daily use, the number that gets you through cloudy windless stretches.
- **Demand:** per-appliance/room load (W) x **duty cycle** -> Wh/day. Base load vs
  peaks; seasonal (lighting + heating in winter, cooling in summer).
- **Losses:** inverter + wiring + conversion + standby (often 10-20%).
- **The honest condition:** *average daily generation >= average daily consumption*
  **AND** *storage >= the longest expected lean stretch*. **Nameplate watts are
  misleading**, a 5 kW array and a 5 kW peak load are not "100%": the array only
  makes power ~5 sun-hours/day and the load never draws 5 kW continuously. Energy
  (Wh over a day/season), not power (W), is the unit.

### 2. Water
- **Supply:** rainwater (catchment area x rainfall x runoff coefficient, and rain is
  *seasonal*, so annual totals lie), well (aquifer yield + pump energy, which couples
  to the energy loop), spring/surface, atmospheric (dew/fog), and **recycling**
  (greywater -> irrigation; blackwater -> compost/biogas) which *reduces net demand*.
- **Storage:** cistern/tank volume = days of buffer through a dry spell.
- **Demand:** drinking + cooking + hygiene + laundry, but **irrigation + livestock
  usually dwarf household use**, the garden is the big draw, linking water to food.
- **Quality tiers:** potable vs greywater vs irrigation-grade; treatment (filter, UV,
  RO, and RO *wastes* a reject stream) costs energy + loses water.
- **Condition:** *collection + recycling >= demand averaged* **AND** *storage >= the
  dry season*. Distribution of rainfall matters more than the annual total.

### 3. Food / nutrition
- **Supply:** crops (growing area x yield/area x **growing-season length**;
  hydroponics/aeroponics multiply yield/area but cost energy + water; greenhouses
  extend the season), livestock (feed -> meat/eggs/dairy conversion ratios),
  aquaculture, mushrooms, foraging.
- **Inputs (couplings):** seeds, **soil fertility/nutrients** (ideally from the
  compost loop, else imported fertilizer), water (the water loop), light/heat (the
  energy loop, for grow-lights/greenhouses), and **labor**.
- **Storage/preservation:** root cellar, canning, drying, fermentation, freezing
  (energy), needed to carry a seasonal harvest across a year of eating; spoilage is
  the loss term.
- **Demand:** calories **and** completeness, protein, fats, vitamins, minerals, not
  just kcal (a potato monocrop is "100% calories, 40% nutrition"). Per person/day,
  times occupants, plus livestock feed.
- **Condition:** *annual production + preservation buffer >= annual nutritional need,
  complete, surviving the seasonal gap + a bad-harvest margin.*

### 4. Waste / nutrient cycling (the loop-closer)
- Organic waste + manure -> **compost** or **biogas** (energy + digestate fertilizer)
  -> soil -> food. Greywater -> irrigation. Humanure/blackwater -> compost/septic.
- This is what *lowers* the demand terms above: closing nutrient + water loops drives
  external fertilizer + water inputs toward zero. **Self-sufficiency is largely a
  measure of how closed these loops are.**

### 5. Air / atmosphere (dominant in sealed/space builds, secondary on open Earth)
- O2 (plants/algae make, people/combustion consume), CO2 (the inverse), humidity,
  filtration, ventilation. In a **sealed** homestead this is a hard closed loop
  (bioregenerative life support: plant/algae area per person). On open Earth it is
  mostly free (ventilation) but indoor air quality + fresh-air heat loss still count.

### 6. Thermal / building envelope (the biggest *demand-side* lever)
- Heating + cooling demand is set by **climate (degree-days)** and the **envelope**:
  insulation (R-value), thermal mass, orientation/passive-solar gain, glazing,
  air-sealing. A superinsulated passive house can need an order of magnitude less
  energy than a leaky one. **The cheapest kilowatt is the one you never need**, so
  envelope quality is a self-sufficiency variable, not just an architecture choice.

### 7. Materials, maintenance, repair (the long-tail that makes it *permanent*)
- True self-sufficiency includes **keeping the systems running**: spare parts, the
  ability to **fabricate/repair** (the blueprint's 3D printer, forge, sawmill,
  workshop), raw-material sourcing (recycle/mine/grow), and the **skills/knowledge**
  to do it. A system that fails on an unobtainable part is not self-sufficient. This
  is where the parts-list / BOM and the real-world bridge close.

## Cross-cutting variables (they gate every loop)

- **Location + climate (the single biggest driver):** latitude + sun-hours, rainfall
  amount *and seasonal distribution*, wind, temperature range, growing-season length,
  soil quality, water table. The *same* design is self-sufficient in one place and
  fails in another, which is exactly why homes carry a lat/long (the `Place` model).
- **Scale / occupancy:** people (+ livestock) multiply every demand. This is the
  Fibonacci Solo/Family/Community/Colony axis already in the blueprint.
- **Autonomy / resilience margin:** how long can you run with zero outside input?
  Days (battery/cistern), a season (food preserves), a bad year (soil + seed stock).
  This is the difference between "balances on a good day" and "actually survives."
- **Time horizon:** daily (energy/water), seasonal (food/heat), annual (soil, seed),
  multi-year (equipment lifespan, soil depletion).
- **Loop coupling (the crucial one):** the loops are a **network**, not a list.
  Energy pumps water, lights the garden, preserves food; water grows food; food waste
  + manure feed soil + biogas; biogas feeds energy. A real score models the graph, so
  improving one loop can relieve another (more solar -> run more hydroponics -> more
  food per m2 -> less land/water).
- **Labor + skills + time:** a homestead needs work; "self-sufficient" includes
  whether the workload is sustainable for the occupants (or automated, the eventual
  Real-home control layer).
- **Redundancy:** a backup source per critical loop (generator, second water source)
  so one failure does not break survival.

## Proposed model for the *score* (so we can build it)

Keep it honest and legible:

1. **Per loop**, compute a ratio over the loop's time window:
   `self_sufficiency(loop) = min(1, internal_supply_over_time / demand_over_time)`
   - energy: Wh/day made (with sun-hours/wind) vs Wh/day used.
   - water: L/day collected + recycled vs L/day used.
   - food: kcal/year (and a separate nutrient-completeness flag) vs kcal/year needed.
2. **Per loop also report an autonomy number:** `storage / daily_demand` (days you
   last with zero input). A loop at 100% supply but 0 storage still fails on the first
   bad day.
3. **Overall self-sufficiency = the minimum across loops** (Liebig's limiting factor, 
   honest about "what breaks first"), with each loop shown so you see the bottleneck.
   (A weighted blend is friendlier but hides the weak link; show both maybe.)
4. **Gate everything by location + scale** (data inputs): a design is scored *for a
   place and a household size*, never in the abstract.
5. **Same metric in sim and real:** in sim the supply/demand come from the design +
   component data; in a Real home the *supply* numbers come from live sensors and the
   score becomes a live readout + prediction.

### What data we'd add to compute it
- The blueprint already gives **demand**: each room's `power_watts` and
  `water_liters_per_day` (today these read as rated/peak, we'd want average draw +
  duty cycle for energy-over-time).
- We'd add a small, **editable** component-output table (infinite-of-X data): for each
  generation/collection/recycling component an output figure + assumptions, e.g.
  `solar_panel: 400 W peak, ~5 sun-h/day`, `rain_tank: 1000 L`, `composter:
  X kg fertilizer/week`. Clearly marked as editable estimates the operator tunes.
- A **location** record (lat/long -> sun-hours, rainfall, degree-days; the `Place`
  coordinate is the hook) and a **household size** (the scale selector).

### Deliberately deferred
Full loop-coupling/network simulation, livestock + soil dynamics, and energy-over-time
weather modeling are later. The first buildable cut is **per-loop supply/demand + an
autonomy number + the limiting-factor overall**, with editable component data, enough
to show an *honest* "this design is energy-limited at 60% for a family in Silverdale,"
which is already far more useful than a nameplate ratio.
