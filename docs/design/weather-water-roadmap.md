# Weather + water: the hyper-realism roadmap

Status: DIRECTION, 2026-07-28, from the operator's questions after the
planetary-pillars wrap ("can we do water volumetric like the clouds? How
would we handle rivers? What about rain, snow, hail... the full breadth
of possible weather AND crazy weather, like fire storms, or meteor
storms, or tornadoes, hurricanes"). Companion to the three hyper-realism
arcs in PRIORITIES (water FFT, cloud froxels, plant impostors).

## 1. Water: hybrid volumetric, not fluid sim

Clouds are a participating medium (seen into everywhere); water is an
INTERFACE plus two special viewing regimes. True volumetric fluid (FLIP/
SPH) is film-budget, not planet-budget. The hybrid that reads hyper-real:

- SURFACE: FFT ocean spectrum (Tessendorf) - a whole statistical sea
  replacing the six hand trains; the displacement/normal/foam textures
  come from the same FFT so geometry, shading, and whitecaps agree by
  construction. Screen-space reflections + refraction with a depth
  buffer replace the alpha-blend approximation.
- UNDERWATER (volumetric regime 1): depth-graded extinction (murk),
  god-ray light shafts from the surface, projected caustics on the
  seafloor, suspended-particle motes. This is the Subnautica look the
  operator referenced; all camera-local, all cheap relative to the sea
  it sells.
- SPRAY/MIST (volumetric regime 2): particle volumetrics for breaking
  waves, waterfall bases, storm spume - the existing particle system
  grown a soft-particle material.

## 2. Rivers: derive from the real heightmap

Data first: FLOW ACCUMULATION over the shipped Earth elevation grid
computes the drainage network offline or at load (every cell's downhill
neighbor; accumulate; threshold = river). No new source data needed,
and dry worlds get correct arroyos for free.

- RENDER: ribbon meshes extruded along drainage polylines, flow-mapped
  shading (normals scroll along the local flow direction - the industry
  standard for bends), width/depth from accumulated flow, estuary blend
  into the ocean shell.
- The `hydrology` system module already exists as a skeleton; the
  drainage graph becomes its data spine. Gameplay inherits: irrigation
  intake placement, fishing spots, navigability, flood risk.

## 3. Precipitation: render what the sim already knows

The WeatherSystem state machine is LIVE (HUD: "Rain 13C 4m/s"); it
barely renders. The arc:

- Rain/snow/hail as camera-following particle volumes driven by the
  weather state (intensity, wind shear from the sim's wind vector).
- WET RESPONSE: ground albedo darkening + roughness drop + puddle
  accumulation in flats while raining; dries over minutes after.
- SNOW ACCUMULATION: temperature + slope driven white blend on the
  terrain shader (the sea-ice blend is the pattern), melts by the same
  rule backward.
- Hail = the rain volume with ballistic particles + real damage events
  (crops, glass roofs - ties to the greenhouse).
- Screen-space droplets/frost at the camera for storms.

## 4. Wind: clouds move at the weather's speed, not a constant

The operator is right that real cloud motion varies with the weather.
The froxel+advection cloud arc replaces the single global drift constant
with a WIND FIELD: the weather map advects through it, so trade winds,
jet streams, calm mornings, and storm fronts each move at their own
rate, varying by location (and eventually altitude per cloud family).

INTERIM SHIPPED (v0.1032.1): a global zonal advection angle integrates
the live weather-sim wind (2.5x surface, the gradient-wind rule) and
rotates every weather-map lookup - the sky's MODIS envelope, the storm
sea-state sample, and the CPU god-ray overhead dim mirror it exactly.
Storm wind = visible deck motion, calm = near-still. When a fresh MODIS
map lands, the accumulated angle transfers to a bucket that eases to
zero over ~45 s, so real geography re-wins without the deck snapping
(clouds::advance_cloud_advect, unit-tested). The full wind FIELD
(per-location direction + speed) remains the froxel-arc goal.

## 5. Extreme weather: infinite-of-x events

Weather EVENTS are data entries (data/weather/events.ron, future), each
declaring:

- visuals: cloud preset (e.g. hurricane spiral carved into the coverage
  field), sky tint, particle systems, volumetrics;
- physics: wind field (vortex, front, downburst), damage model, terrain
  effects (crater, burn, flood);
- spawn rules: where/when/how often, or scripted/manual for events.

Anchors that already exist: the `disasters` system module, the
impact-crater ladder in docs/design/voxel-terrain.md (meteor storms),
the ecology system (fire propagation for fire storms), the cloud
volumetrics (smoke reuses them), the wind-driven sea state (hurricanes
already darken + chop the ocean via the MODIS storm path).

Examples the operator named, sketched:
- HURRICANE: synoptic spiral coverage preset + rotating wind field +
  rain volume + storm sea state; visible from orbit as a real spiral.
- TORNADO: funnel volumetric (a small dedicated raymarch cone) + local
  vortex wind + debris particles + a damage track on the ground.
- METEOR STORM: disasters system spawns ballistic impactors; craters
  via the voxel overlay ladder; dust/smoke volumes after.
- FIRE STORM: ecology fire propagation + smoke columns through the
  cloud system + ember particles + burn-scar albedo.

## Sequencing

1. Water FFT + underwater volumetrics (the operator's most-visited view).
2. Cloud froxels + wind advection (fixes fly-through + varying rates).
3. Precipitation rendering (the sim is waiting).
4. Rivers (data spine + ribbons).
5. Event framework + the first two events (hurricane, meteor storm).
6. Plant impostors ride alongside (independent arc).
