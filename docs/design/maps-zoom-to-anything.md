# Maps: zoom from the solar system to your bunk

Written 2026-10-20 from a read-only survey of the Maps page, the engine pieces it can
reuse, and how other products do a continuous zoom. Nothing here is built yet. The
increments in section 6 are the build order; PRIORITIES.md ranks when.

The operator, verbatim (2026-10-20, with a screenshot of the Solar System view):

> "Can we get the maps page to zoom in to where the ship the player is living on in
> orbit of Earth? If the ship moves we want the map to always zoom in to the player's
> position. Like it could provide a way for players to see the ship they're on as we
> build out the ship. Like the version we see of the mothership in the maps page is a
> high detail version of it so it looks as good as possible. Eventually we need to use
> the map page to navigate the multikilometer long spaceships AND planet surfaces
> including subterranean structures. I don't know exactly how we should improve the
> solar system appearance but, we need to figure something out. It just looks like a
> bunch of lines with a few colored dots for planets/moons/asteroids."

Companions: [maps-ladder.md](maps-ladder.md) (the view ladder and the OSM work, whose
rungs this document joins into one zoom), [maps-multi-scale.md](maps-multi-scale.md)
(an older web-only proposal; its paths `pages/maps.html` and `game/js/map.js` are stale,
its scale table is still useful), [cosmos-architecture.md](cosmos-architecture.md)
(the container model this map's camera follows), [ship-homes-increment-7-plan.md](ship-homes-increment-7-plan.md)
(storeys, being built now), [habitat-generation.md](habitat-generation.md) (spines and
drums: the multi-kilometre ships), [voxel-terrain.md](voxel-terrain.md) (caves).

## 0. In short

One map, one camera, one zoom: from the Sun's whole family down to the ship you live
on, through its hull into the deck you are standing on, and onto and under a planet's
surface. The camera is anchored to whatever you are looking at (the ship, a planet, a
spot on the ground) with an f64 offset from it, which is how planetarium software keeps
centimetre precision at a hundred AU. The picture has two layers: the WORLD layer is
the engine's own renderer looking from the map camera (so the ship on the map is the
real ship, every mesh and light, and Earth behind it is the real Earth with its clouds),
and the SYMBOLOGY layer on top is drawn by the page (orbit lines that fade and thin,
icons with a minimum size, labels that never overlap, the "you" marker, the level
picker). The first increment does exactly what the operator asked first: the page opens
on your ship in its orbit, keeps it centred as it moves, and draws it from its own
meshes.

## 1. What the Maps page is today

Facts, with file and line, as of 2026-10-20.

**Where it lives.** `src/gui/pages/cosmos.rs` (3,880 lines), drawn by
`src/gui/dispatch.rs:80` as `cosmos::draw(ctx, theme, state)`: the page receives
`GuiState` only, never `EngineState`, so it cannot see where the station or the player
is unless the engine publishes it into `GuiState`. Its state is twenty-odd `cosmos_*`
fields in `src/gui/mod.rs` (2398-2473). Four views by tab (`CosmosView`, cosmos.rs:412):
Planet (2D OpenStreetMap regions), Solar System (3D), Galaxy (2D, the 109,400-star HYG
file), Night Sky (2D RA/Dec).

**How the Solar System view draws** (`draw_system_view`, cosmos.rs:958-1821). Everything
is CPU-projected through `project_to_screen` (cosmos.rs:381) and painted with egui:
- Orbits: `sample_orbit_points` at 96 points per planet, 48 per moon, drawn as
  `line_segment`s in fixed dark colours (1094-1126). No fade, no trail, no thinning.
- Bodies: one `circle_filled` each in a colour from `body_color` (314-328), a
  hard-coded match on ten names; every moon and asteroid gets the same grey. The radius
  is the true angular size clamped to 2.5..40 px for planets (1357-1370), so a planet
  can never be drawn bigger than a 40 px dot however close you go.
- Labels: the "pill" widget (`widgets::body_pill`), priority-filtered (moons and
  asteroids only when hovered or selected).
- Overlays: Lagrange points and reference orbits (LEO, MEO, GEO...) as buttons; both
  lists are code literals (`LAGRANGE_PAIRS`, `reference_orbits_for` at 2645-2669, whose
  own comment says "Could move to data/ later").
- The camera (`Cosmos3DCamera`, 337) is in AU with its distance clamped to
  0.001..10,000 AU (1020-1021). **0.001 AU is 149,600 km: you cannot get closer to
  Earth than 23 Earth radii, so the ship can never be shown at all.**
- "You · Home" is a ring drawn at **Earth's centre** (1557-1566), and the mining
  asteroids are a fan at fixed pixel offsets around it (1567-1584), not at their data
  positions (`data/asteroids/belt.ron` gives each one a position in km).
- The body list's Stations row reads **"Home Station · 400 km LEO"** (cosmos.rs:614).
  That is stale: `data/stations/home.ron` has been a SYNCHRONOUS orbit since v0.1225,
  42,164 km from Earth's centre (35,786 km up), phased to hang over Silverdale, WA
  (mean anomaly -122.3). Its Track toggle only drives the in-world target marker.
- The time bar (`draw_time_controls`, 3585) runs its own clock, started from the wall
  clock and scrubbed or sped up independently of the game's clock.

**What the engine already does that the page does not use.**
- The game world IS the solar system at true scale: "the point of B is that the FPS sky
  IS the Maps page (real bodies, real positions, real scale)" (`src/engine/world_load.rs`
  ~667, map sync increment B). Every body is a mesh at its ephemeris position, every
  orbit a 1 px line with a fade trailing the body (`src/lib.rs` ~11130-11190), and the
  Sun is placed and lights things from its real direction (`src/lib.rs:11204`).
- The station is propagated every frame from the game clock (`src/lib.rs:3379-3394`,
  `station::orbit::propagate` with `over_its_longitude`), giving `station_world_pos`
  (Earth-centred metres) and `station_world_rot` (LVLH attitude).
- `engine::ipc::render_view_onto` (`src/engine/ipc.rs:2011`) renders the world from ANY
  camera into any display-format texture: the hi-res screenshot uses it with
  `ViewPasses::Everything`, the in-world camera screens with `ViewPasses::SceneOnly` at
  10 Hz (`src/engine/screens/camera.rs`).
- **While any page is open the live 3D frame is not drawn at all**
  (`src/lib.rs:14624` `page_active`, 14668: "UI-only frame: skip 3D render"). So a map
  that renders the world from its own camera spends the budget the live frame left
  unused; it does not add to it.
- The model viewer (`src/gui/model_view/gpu.rs`) is the house pattern for 3D inside a
  page: an egui paint callback that draws "only when something changed".
- `label_placer.rs` already declutters star and street names on the other views.

**Two engine facts the map must not contradict.**
- The game has no axial tilt (`src/renderer/sky_frame.rs:31`): Earth spins about world
  +Y by `planet_spin_from_time`. A map that drew Earth with its real 23.4 degree tilt
  would show the synchronous ship hanging over the wrong place. The map must use the
  engine's spin, and gets the tilt for free the day the engine has it.
- BUG-140 (FIXED 2026-10-21, v0.1534.0; see its entry in `docs/BUGS.md`): the orbit model
  mapped the ecliptic into the world with a reflection, so the planets went round the Sun the
  wrong way. It is a rotation now, held to JPL Horizons; the Moon's node and perigee turn at
  JPL's rates since v0.1536.0.

**The web mirror** (`web/pages/maps.html`) is ahead in one way and behind in others: it
already has a continuous zoom (galaxy, stellar, system, globe, map tiles, cross-faded by
zoom level), but its stars (30) and planets (9) are inline literals, its globe is a
canvas-drawn sphere with hand-drawn continents, and its street tiles are fetched from
CARTO's CDN at run time while the native Planet view reads installed region files.

## 2. What a better map reuses (do not rebuild)

| Need | Already exists |
|---|---|
| Real body positions and orbits | `src/cosmos.rs` (`sol_bodies`, `body_world_position_3d_au`, `sample_orbit_points`), `data/star_systems/sol.json` (70 bodies) |
| The ship's position and attitude | `station::orbit::{propagate, over_its_longitude, attitude}`, `data/stations/*.ron`, `state.station_world_pos/rot` |
| The real Earth, Moon, Mars, Pluto | albedo bins in `data/planets/`, the celestial pass with atmosphere and clouds (`src/renderer/celestial.rs`), vantage `blue-marble-12000km` (30 fps floor) |
| The real ship | `ShipStructure` zones, corridors and plots (`src/ship/ship_structure.rs`), the hull wrap (`src/ship/hull.rs`, `data/blueprints/hull_profile.ron`), rebuilt live on edits (`rebuild_hull`) |
| Rendering from another camera | `render_view_onto` + `ViewPasses`, the camera's `Projection::Orthographic` (`src/renderer/camera.rs:554`) for plan views |
| Storeys and rooms | increment 7's `FloorSlab` and `HomeStructure::storeys()`, `detect_rooms`, the `room_types` registry |
| Places and ways through the ship | `src/ship/door_points.rs`, `src/ship/transit.rs`, `src/ship/ship_space.rs` |
| Surface maps | `src/terrain/osm_region.rs`, `data/maps/regions/` (Seattle Center, Silverdale), `src/engine/region_meshes.rs` |
| Stars and the Milky Way | `src/renderer/stars.rs` (HYG catalog, galaxy glow), `data/stars-map.bin` |
| Labels that never overlap | `src/gui/widgets/label_placer.rs` (+ `web/shared/label-placer.js`) |
| A marker in the world | `GuiState::target_markers` (drawn by the HUD) |
| Rig and screenshots with the page open | IPC `{"page":"maps"}` (`src/engine/ipc.rs:792`); the swapchain capture runs after the egui pass (`src/lib.rs:15919`, 15965) |

## 3. How others do it (researched 2026-10-20; methods only, no assets or code)

- **NASA's Eyes on the Solar System** ([science.nasa.gov/eyes](https://science.nasa.gov/eyes/),
  page updated 2026-10-05): a "simulated live view of the solar system", 3D spacecraft
  you can interact with, and time you "fast-forward or rewind" between 1950 and 2050.
  What we take: a live clock by default with scrubbing as a deliberate act, and the
  spacecraft itself as a first-class object you zoom to, drawn from its real model.
- **SpaceEngine's Map mode** ([developer blog, 2012-02-20](https://spaceengine.org/news/blog120220/)):
  a zoom slider labelled Planet, Moons, System, Galaxy, Universe; "the engine renders
  only those objects that are located inside the spherical region" around the selected
  object; lines and labels thin out at large scales ("The blue lines have been removed
  to prevent the map from being overloaded"). What we take: one camera centred on a
  selection, named scale stops as shortcuts rather than separate pages, and declutter
  rules tied to scale.
- **Kerbal Space Program's map view** ([KSP forum answer](https://forum.kerbalspaceprogram.com/topic/75700-questions-about-maneuvers-and-the-map),
  [KSP wiki, Map View](https://wiki.kerbalspaceprogram.com/wiki/Map_View); the wiki
  refused a scripted read, so this is from the search summary of it): the map opens on
  your vessel, Tab steps the focus through the bodies, Backspace "will return camera
  focus to your ship". What we take: follow-me as the default and a one-key "back to
  me".
- **Elite Dangerous's galaxy map and orrery** ([Frontier forum developer post](https://forums.frontier.co.uk/goto/post?id=8577080);
  the forum refused a scripted read, so this is from the search summary of it): the
  orrery is integrated into the galaxy map, zooming in on a selected system moves the
  view into its orrery and zooming out returns. What we take: no tab switch between
  scales, the zoom itself crosses them.
- **Star Citizen 3.23** ([Star Citizen wiki, release 2024-05-10](https://starcitizen.tools/Update:Star_Citizen_Alpha_3.23.0)):
  a reworked starmap, "a minimap for the HUD" and "interior maps for FPS gameplay".
  What we take: the same map surface navigates space and the inside of places.
- **OpenSpace / the Dynamic Scene Graph** (Axelsson, Costa, Silva, Emmart, Bock,
  Ynnerman, Computer Graphics Forum 2017,
  [Eurographics library](https://diglib.eg.org/handle/10.1111/cgf13202); abstract read
  through search, the library refused a scripted read): a reference frame assigned
  dynamically keeps numerical precision for the objects of interest, letting a viewer
  go from Martian surface features to the Milky Way in one render.
  [OpenSpace's scene graph node docs](https://docs.openspaceproject.com/latest/reference/asset-components/Other/SceneGraphNode.html)
  give each node an InteractionSphere, "The minimum radius that the camera is allowed
  to get close to this scene graph node", with approach and reach distances. What we
  take: the map camera's anchor plus f64 offset, and a per-object closest approach.
- **Google Earth** ([help: explore the Earth](https://support.google.com/earth/answer/148186?hl=en)):
  one continuous zoom from space to street, and for the sea floor the water surface is
  a toggle ("View, then Water Surface") so you see from above or below it. What we
  take: the surface itself is a layer you can switch off to see what is under it.
- **CesiumJS** (open source; [GlobeTranslucency](https://cesium.com/learn/cesiumjs/ref-doc/GlobeTranslucency.html),
  [ClippingPlaneCollection](https://cesium.com/learn/cesiumjs/ref-doc/ClippingPlaneCollection.html)):
  the globe can turn translucent by camera distance and only inside "a Rectangle used to
  limit translucency to a cartographic area"; "When the camera is underground back
  faces and front faces are swapped"; clipping planes "selectively disable rendering in
  a region". What we take: underground views by making the ground translucent around
  the focus, and cutaway views of a storey by clipping above it.
- **Indoor maps.** Google Maps' `IndoorBuilding`
  ([Android reference](https://developers.google.com/android/reference/com/google/android/gms/maps/model/IndoorBuilding)):
  levels "are in 'display order' from top to bottom", an active level and a default
  level, and `isUnderground()`. Apple's Indoor Mapping Data Format, an OGC Community
  Standard ([IMDF 1.0.0, approved 2021-02-02](https://docs.ogc.org/cs/20-094/index.html),
  [Level](https://docs.ogc.org/cs/20-094/Level/index.html)): a Level is "a floor area"
  with an `ordinal`, "The Level feature's 'stacking' position, relative to others", and
  a `short_name`. What we take: the level picker appears when a multi-level place is in
  focus, lists levels top to bottom with an ordinal, defaults to yours, and the same
  picker serves decks, floors, basements and caves.
- **Fly-to animation.** Mapbox's flyTo "implements an 'optimal path' animation, as
  detailed in: Van Wijk, Jarke J.; Nuij, Wim A. A."
  ([Mapbox docs](https://docs.mapbox.com/android/maps/api/10.0.0/mapbox-maps-android/com.mapbox.maps.plugin.animation/-camera-animations-plugin-impl/fly-to.html));
  Reach and North (2018, [arXiv 1801.09358](https://arxiv.org/abs/1801.09358)) make it
  interruptible: "smooth and efficient zooming and panning based on dynamical systems in
  hyperbolic space". What we take: jumps across twelve orders of magnitude that zoom out,
  travel and zoom in, and can be grabbed mid-flight.
- **Asteroid belts from real orbits.** The Minor Planet Center's MPCORB "contains
  orbital elements of minor planets that have been published in the Minor Planet
  Circulars" ([MPCORB](https://minorplanetcenter.net/iau/MPCORB.html)). What we take: a
  belt drawn from the real distribution of orbits, not a random ring. Its terms of use
  need a dated findings document before any derived file ships (CLAUDE.md, "research a
  legal question, then write it down").

## 4. The destination

One map. You scroll and it zooms; you click a thing and the camera flies to it; you
press Home (or "Back to me") and it flies back to you and follows. The scale stops are
shortcuts on a breadcrumb, not pages:

| Band (camera distance to its anchor) | What you see | World layer | Symbology layer |
|---|---|---|---|
| **Inside** (a storey picked, under ~1.5 ship lengths) | the deck or floor you chose, cut open from above, rooms named, you and the people who share their presence with you, doors, lifts, routes | the real meshes with everything above the storey cut away; far out, baked plan tiles | room names, the level picker, your dot and facing, a route line |
| **Ship** (10 m to ~60 km) | the whole ship from outside, lit by the real Sun, Earth behind it | the real hull, zones, lights (`render_view_onto`) | the ship's name, its plot labels at close range, docking lanes later |
| **Near space** (~60 km to ~2 million km) | Earth and the Moon as real globes, the ship as an icon on its orbit line, other stations, your mining asteroids where they really are | the real Earth (atmosphere, clouds, night side) | the ship's orbit ring, reference orbits, Lagrange points, labels |
| **Solar system** (~1e9 m to ~1e14 m) | the Sun's glare, the star sky and Milky Way behind, every body as a phase-lit icon of its real colour, orbit trails fading behind each body, the belts as dust | the star sky | icons with a minimum size, orbits, labels, belts |
| **Galaxy** (> ~1e15 m) | the existing Galaxy view's catalog stars, Sol highlighted | the star sky | the Galaxy view, folded into the same camera |
| **Surface and under it** (on a planet, ~10 m to ~1,000 km) | terrain, OSM roads and buildings, building floors, and with the ground made translucent around the focus, basements, tunnels and later caves | the chunked-LOD planet and region meshes | street names, the level picker for buildings and underground |

The Night Sky stays a separate mode: it is a view outward from a point, not a place.

Every anchor is data (infinite-of-x): bodies from `sol.json`, stations from
`data/stations/*.ron`, ships from the ship file and `data/fleet.json`, storeys derived
from floor slabs, rooms from `detect_rooms`, regions from `data/maps/regions/`.

## 5. Techniques and what each costs

### 5.1 The camera: an anchor and an f64 offset

`MapCamera { anchor, offset_m: DVec3, log_distance, yaw, pitch, frame, follow }`.
`anchor` is a container in the sense of cosmos-architecture.md: `Ship(id)`,
`Body(id)`, `Surface(body, lat, lon, alt)`, `Station(id)`. Every position is computed in
f64 relative to the anchor, and only the camera-relative vector is narrowed for drawing
(the dynamic reference frame of Axelsson et al.). Zoom is multiplicative and
cursor-anchored, as today, over 10 m to 1e14 m (the clamp at 149,600 km goes).

- **Follow.** With follow on, the anchor is wherever you are: your ship while aboard,
  the surface point while frame-locked to a planet, free space after dev travel. The
  ship moving (any orbit, a manoeuvre, later an interplanetary transfer) moves the
  anchor; the camera keeps its offset from it. Dragging to rotate keeps follow; panning
  or clicking another body ends it; Home or "Back to me" flies back and resumes it.
- **Frame.** At ship scale the default frame is the ship's LVLH frame, so Earth stays
  below the ship the way residents see it from the deck; beyond near space it is
  inertial (the stars hold still). Switchable.
- **Fly-to.** Interruptible zoom-out, travel, zoom-in path in (log distance, position)
  (van Wijk and Nuij, made interruptible by Reach and North).
- **The ship's place comes from the engine, never a copy of the maths.** The engine
  publishes what it computed this frame (section 6, increment 1); the map does not call
  `propagate` with its own inputs, because two callers drift (the v0.881 and BUG-090
  history in `src/lib.rs` around 3370-3450 is the warning).

Cost: CPU under 0.05 ms per frame. GPU none. Network none.

### 5.2 Two layers

- **World layer.** `src/engine/map_view.rs` (new) renders the world from the map camera
  with `render_view_onto` into a texture registered with the main egui renderer, and
  the page draws that texture as its canvas background. Re-rendered only when something
  changed: the camera moved, the clock advanced past a threshold, the ship was edited.
  While the camera is moving it renders at half resolution (a quarter of the pixels)
  and refines to full 250 ms after input stops (Google Earth's progressive refinement).
  Used when the world is loaded and the clock is Live; otherwise the page shows the
  symbology layer alone over a star backdrop and says why ("Enter your home to see your
  ship up close").
- **Symbology layer.** The page's own egui painting, projected with the same f64 camera.
  This is where the "lines and dots" problem is fixed (5.3 to 5.5).

Cost, world layer: at rest 0 ms. Moving: at most the live frame's cost at the same
place, and the live frame is not drawn while the page is open (`page_active`), so the
map spends what the game would have spent, not more. Half resolution while moving
roughly quarters the pixel-bound passes (celestial, clouds, scene). To be measured with
`HUMANITY_FRAME_COSTS=1` at the new vantages before and after.

### 5.3 Bodies: true scale, an icon floor, a real globe when it is big enough

- Always true scale and true distance (the realistic mode). A body whose disc would be
  under ~6 px is drawn as an ICON of a minimum size: a disc of its real colour lit on
  the side facing the Sun (a crescent, quarter or gibbous shape from the real
  Sun-body-camera angle), so even a 4 px Mars reads as a lit world, not a dot. Over
  ~6 px the icon fades out and the world layer's real globe takes over.
- Colours are data: a `map_color` per body in `sol.json`, computed for bodies with an
  albedo map as the mean of that map, so the hard-coded `body_color` match goes.
- The simplified mode (the dual-mode rule): a **Diagram** toggle with log-spaced orbits
  and enlarged bodies, the textbook orrery, labelled as not to scale.

Cost: 70 bodies, under 0.05 ms CPU.

### 5.4 Orbit lines that fade and thin

- **Trail.** Bright at the body, fading to nothing half an orbit behind it, the way the
  engine's own in-world orbit lines already do (`src/lib.rs` ~11130-11190); the body's
  exact position is a vertex so the body never sits off its line.
- **Size fade.** A whole orbit fades out when it is smaller than ~8 px on screen (moon
  orbits around a far planet stop cluttering) and thins when it is much larger than the
  screen.
- **Adaptive sampling.** More samples near the camera, so zoomed in on Earth its orbit
  is not a visible chord 10 million km long (today's 96 fixed samples).
- **The ship's own orbit**, sampled from the engine's propagation over one period, in
  the accent colour.
- Reference orbits and Lagrange points become data (`data/maps/reference_orbits.ron`,
  `data/maps/lagrange_pairs.ron`), and all overlays become a layer list in data
  (`data/maps/layers.ron`: id, label, the bands it shows in, default on or off), so the
  toggle buttons are drawn from the file.

Cost: at most 70 orbits at 96 to 384 points, under 1 ms CPU (egui tessellation), under
0.1 ms GPU. Measured as its own stage (`cpu.maps_symbology`).

### 5.5 Labels that declutter

`label_placer.rs` (already used by the Galaxy and Planet views) for every body, station
and ship name, offered in priority order: you, the anchor, the Sun, planets by apparent
size, dwarf planets, moons only when their parent's disc or orbit is large on screen.
A name with no clear spot is dropped. Cost under 0.1 ms.

### 5.6 The ship from its real meshes

At ship scale the world layer IS the high-detail ship: the hull wrap, zone shells,
glass roofs with the gardens under them, lights, the neighbours' homes, and the player's
own build, all from the same draw lists the game uses, lit by the real Sun with Earth
behind. Every edit to the ship regrows it on the map next frame (the hull is rebuilt by
`rebuild_hull` already). When the ship passes through Earth's shadow it goes dark, as it
really would (the game has no axial tilt, so a synchronous ship in the ecliptic plane
crosses the shadow every day); a **Map light** toggle adds a fill light for the map
render only (section 7, decision 4). Beyond ~60 km the ship is an icon on its orbit line
with its name.

Cost: as the world layer (5.2). At synchronous distance Earth hardly moves in the frame
(the ship hangs over one spot), so a following map at rest re-renders only for the clock
and clouds, about once a second.

### 5.7 Decks and rooms: the level picker, the cutaway, plan tiles

- **Level picker** (`src/gui/widgets/level_picker.rs` and a matching web component):
  appears when a multi-level place is in focus, lists its levels top to bottom with your
  level marked, defaults to yours (Google's `IndoorBuilding`, IMDF's `ordinal`). The
  same widget serves the ship's decks, a home's floors, a building's floors on Earth,
  basements and caves.
- **Cutaway.** Picking a level cuts everything above it away: hull, roof, ceilings,
  upper storeys. Prefer object-level culling if increment 7's meshes are split per
  storey (lane G draws "Walls, sills, headers ... per band"); if a zone's walls share one
  mesh, a shader cut height behind its own `override` switch (the megashader rule:
  `HAS_CUT`, off in every pipeline that cannot need it, the switch as the first operand).
- **Rooms and people** from `detect_rooms` and the `room_types` registry: names, fills
  by room type, your dot and facing, and other people only if they share their presence
  with you (section 7, decision 7).
- **Plan tiles for multi-kilometre ships.** Far out, a deck is shown as tiles rendered
  once from above with the orthographic camera and the cut, cached by (ship hash, level,
  zoom, x, y) like a slippy map, and re-baked only where an edit touched. A 2 km drum
  deck at 0.5 m per pixel is about 64 tiles of 512 px.

Cost: cutaway, object culling 0 ms, a shader cut about one compare per fragment and 0 ms
on views with the switch off (bit-identical). Plan tiles: baking at most one tile per
frame, at most 2 ms GPU while baking; showing them under 0.1 ms; 1 MB per tile, a 64 MB
cap, least recently used first.

### 5.8 Onto and under a surface

- Zooming down onto Earth below the ship lands in the world layer's chunked terrain and
  OSM region meshes (the ship hangs over Silverdale, and Silverdale is a shipped region).
  The 2D Planet view becomes the straight-down orthographic mode of the same camera.
- **Under the surface:** the ground turns translucent inside a rectangle around the
  focus (CesiumJS's method), revealing what is below on the level picker: basements and
  tunnels from OSM tags (`level`, `layer`, `tunnel`, `building:levels`, a new fetcher
  field), and caves and mines when voxel terrain exists (voxel-terrain.md is a direction
  document; nothing of it is built).

Cost: translucency limited to a rectangle and behind its own switch, measured in the
celestial pass at the surface vantages; 0 ms when off.

### 5.9 Time: one clock

The page opens on **Live**: the game's own clock, so the map shows the ship and the
planets exactly where the game has them. Scrubbing or speeding time detaches from Live
and shows "Back to live". Until increment 6 the world layer cannot follow a scrubbed
date, because the engine reads the wall clock in eleven places
(`946_728_000` in `src/engine/frame_lock.rs` x3, `src/engine/ipc.rs` x2,
`src/engine/world_load.rs`, `src/lib.rs` x5), so a scrubbed map shows the symbology
layer alone.

### 5.10 The sky behind

In the solar-system band the world layer draws the real star sky and Milky Way from the
map camera's direction (the star renderer, about 0.9 to 1.2 ms GPU per render per
`docs/design/frame-cost-arc.md`), re-rendered only when the view direction changes. The
Sun gets its real glare from the same render. The belts are point clouds sampled from
the real orbit distribution, propagated only when the clock moves a visible amount.

### 5.11 Cost summary

| Technique | CPU per frame | GPU per frame | Network |
|---|---|---|---|
| Engine publishes "where you are" | under 0.01 ms (values already computed) | 0 | 0 |
| Camera, projection, follow | under 0.05 ms | 0 | 0 |
| Symbology (orbits, icons, labels) | under 1 ms | under 0.1 ms | 0 |
| World layer at rest | 0 | 0 | 0 |
| World layer while moving | as the live frame at that place (not drawn while the page is open) | same, at a quarter of the pixels while moving | 0 |
| Star sky behind the solar system | 0 | 0.9 to 1.2 ms per re-render, only on direction change | 0 |
| Belts (up to 20,000 points) | under 0.3 ms, propagated only on clock change | under 0.1 ms | 0 |
| Storey cutaway | 0 | 0 (object cull) or one compare per fragment (switch on) | 0 |
| Plan tiles | 0 | at most 2 ms while baking one tile; under 0.1 ms showing | 0 |
| Route through the ship | under 1 ms once per request | 0 | 0 |
| Other people on the map | 0 extra | 0 | 0 new: positions already reach peers in the same container |

Every number above is a budget to be proved by measurement, never a reading: each
increment's proof captures `HUMANITY_FRAME_COSTS=1` at its vantages.

## 6. Increments, in build order

Each increment ships on its own, says which files it touches, and names its proof. Every
new test is seen red first with the red output in its comment; every rig check is made to
fail on purpose once before it is trusted.

### Increment 0: rename and carve (mechanical, no behaviour change)

**DONE 2026-10-20.** The files below exist as named; the three Maps snapshots
(`maps_system`, `maps_galaxy`, `maps_planet`) rendered byte-identical before and
after. Section 1's file and line references describe the page as it was before
this increment.

The page is called Maps; its code is called cosmos. The standing rule (CLAUDE.md, names,
2026-10-19) is to rename now. `crate::cosmos` (`src/cosmos.rs`, the Sol model) keeps its
name: it means the model, not the page.

- `src/gui/pages/cosmos.rs` becomes `src/gui/pages/maps/` (`mod.rs`, `system_view.rs`,
  `galaxy_view.rs`, `planet_view.rs`, `night_sky.rs`, `sky_events.rs`, `time_bar.rs`,
  `body_list.rs`, `tests.rs`); `CosmosView` and `Cosmos3DCamera` take map names.
- The `cosmos_*` fields leave `src/gui/mod.rs` (4,736 of its 4,800-line budget) for one
  `maps: MapsState` in a new `src/gui/state_types/maps.rs`.
- Touches `src/gui/pages/mod.rs`, `src/gui/dispatch.rs`, `src/gui/mod.rs` (the
  "cosmos" page alias at 443 goes), `src/gui/ui_snapshots.rs` (`snapshot_cosmos` becomes
  `snapshot_maps_system`), `src/gui/pages/inventory.rs`, `src/gui/pages/apps/tests.rs`,
  `tests/file_size_ratchet.rs` (the new files join the list), `docs/PAGES.md`,
  `docs/FEATURES.md`, `docs/design/maps-ladder.md` (its naming section).
- Proof: `just verify`; every Maps snapshot renders byte-identical under its new name.

### Increment 1: the map opens on you, follows you, and draws your ship

**BUILT 2026-10-21** (status, what differs from the plan below, and the
measurements: "Increment 1 as built" at the end of this section).

The operator's first ask.

What changes:
- **The engine says where you are.** New `src/engine/map_feed.rs` builds a `MapSelf`
  each frame from what the frame already computed: anchor (ship id and name, its parent
  body), the ship's Earth-centred position and velocity and its attitude, Earth's spin
  angle, the Sun's position, aboard or not, your position in the ship frame, the game
  clock, and (later) your level. One call line in `src/lib.rs` after the station block
  (~3533); `src/lib.rs` is at 16,468 of its 16,700-line budget, so only call lines go
  there.
- **The camera** (new `src/gui/pages/maps/camera.rs`): anchor plus f64 offset, 10 m to
  1e14 m, follow, LVLH frame at ship scale, Home and "Back to me", fly-to.
- **Opens on you.** Opening the page from the game sets follow on and the anchor to
  your ship at a distance that frames the whole hull with Earth behind it (about 1.5
  ship lengths).
- **The ship drawn there, from its meshes.** New `src/engine/map_view.rs`: owns the
  texture, registers it with the main egui renderer, converts the map camera into the
  render frame (the hull frame while riding: `station::to_hull`), and renders with
  `render_view_onto(.., ViewPasses::Everything)` when dirty, half resolution while
  moving. One call line in `src/lib.rs` inside the `page_active` branch before the
  egui pass. The page draws the texture as its backdrop.
- **Symbology for you**: the ship's name and real altitude from data (the "400 km LEO"
  row becomes every station in `data/stations/*.ron` with its computed altitude), its
  orbit ring, "You" on the ship instead of on Earth's centre, the icon when the ship is
  under ~6 px. The mining asteroids move to their data positions relative to the ship.
- **Live clock** by default; scrubbing detaches and hides the world layer.

Files: new `src/engine/map_feed.rs`, `src/engine/map_view.rs`,
`src/gui/pages/maps/camera.rs`; `src/engine/mod.rs`, `src/lib.rs` (two call lines),
`src/gui/state_types/maps.rs`, `src/gui/pages/maps/system_view.rs`,
`src/gui/pages/maps/body_list.rs`, `src/gui/pages/maps/mod.rs`, `src/engine/ipc.rs`
(a showcase verb for the rig: follow on or off, distance), `tests/visual/vantages.json`,
`src/gui/ui_snapshots.rs`, `docs/FEATURES.md`, `docs/PAGES.md`.

Proof:
- Unit tests: the feed's ship pose equals the station pose the frame computed, exactly
  (one source, not two); with follow on, the ship projects to the canvas centre within
  0.5 px across a day of clock steps at 1 AU plus 10 m (f64 stability); the zoom reaches
  10 m and 1e14 m; the station row reads 35,786 km for the shipped `home.ron`; a pan
  ends follow and "Back to me" resumes it.
- Snapshot `maps_follow_home` (no world, pinned clock and a fixed `MapSelf`): the ship
  icon at the centre on its synchronous ring, labelled, no "400 km LEO" anywhere.
- Rig vantages (one GPU, background boot, `just verify-runtime` after it as this is a
  render path): `maps-follow-home` (enter the world, `{"page":"maps"}`: the hull fills the
  centre with Earth behind it); `maps-follow-home-later` (the clock moved two game hours:
  the ship is still centred, the lit side of Earth has moved); the control,
  `maps-follow-home-unfollowed` (follow off, same clock jump: the ship leaves the frame),
  which must fail the "ship centred" check.
- Costs: `HUMANITY_FRAME_COSTS=1` with the page open at rest and while dragging, against
  the game at the same place.

#### Increment 1 as built (2026-10-21)

Built as planned above, with these differences, each for a stated reason:

- **The altitude is 35,870 km, not 35,786 km.** The station block (lib.rs) makes the
  home synchronous with the 24-hour SOLAR day on purpose ("synchronous with the
  day/night cycle the player actually sees"), so `orbit::semi_major_axis_m` gives
  42,241 km from Earth's centre, 35,870 km over the game's 6,371 km Earth. Real
  geostationary satellites (35,786 km over the equator, 42,164 km from the centre) keep
  pace with the 23 h 56 min sidereal day. The Stations row, the ship's label and the F11
  card all show the game's number; the row's hover says why it differs from real GEO.
  Section 1's "42,164 km ... (35,786 km up)" was the sidereal figure.
- **The feed's call line sits at the end of the station block, not after it**, because
  the velocity it copies (`vel`) is local to that block. The map view's call line sits
  just before the page/world branch rather than inside it, so it also runs on the frame
  the page closes and gives back its textures and view buffers.
- **The world layer is drawn aboard only, within 60 km of the ship**
  (`camera::WORLD_LAYER_MAX_M`, the "Ship" band). Aboard, the draw lists in the page
  branch are already in the render frame (`station_off` is zero while riding); off the
  ship they are not, and the cloud history is fitted to the player's camera, which a map
  camera far away would smear (risk 1). Beyond the band the ship is an icon on its orbit
  line, as section 4 has it. Near space with the real Earth is increment 2's.
- **Rendered with the screenshot's passes, keeping its view buffers**: a new
  `ipc::render_view_onto_kept` (the old `render_view_onto` keeps its behaviour), and
  `Renderer::release_view_buffers` when the page closes.
- **Costs are measurable on a page frame**: `Renderer::mark_view_frame`
  (`renderer/frame_costs.rs`) makes a UI-only frame that rendered the world layer publish
  its GPU passes and CPU stages, which until now were thrown away with every menu frame.
  The world layer's passes keep the live frame's ids (`gpu.scene`, `gpu.celestial`, ...)
  so they compare directly with the game at the same place; `cpu.map_view` is the whole
  render (a listed remainder, since its parts are rows) and `cpu.maps_feed` the copy.
  Known limit: `cpu.frame_total` and `cpu.present_wait` are recorded only by the live
  branch, so on a map frame they decay; read the pass ids, not the frame total.
- **Not built in this increment**: the Diagram toggle (decision 5) is increment 2's, with
  the solar-system work it simplifies; the Map light toggle (decision 4) needs a fill
  light set for one view only, which belongs to the renderer's lighting, a lane being
  changed today, so it waits; the ship goes dark in Earth's shadow as decided.
- **Renamed** (decision 8, the names-sync rule): `data/stations/home.ron` and its built-in
  fallback are "UHS Mothership" (was "Homestead"); the in-world target marker takes the
  station's own name (was the literal "Home Station"); the F11 card's heading reads the
  ship's name (was "Home station"); the HUD tests follow. "Homestead" stays the name of a
  start (`data/homes/starts.ron`), which it means. Code names (`station_def`,
  `track_station`, `station_world_pos`) mean the orbiting object and kept their names.
- **Two dev verbs beyond the plan**: `map_turn` (the camera turns round the ship as a drag
  does, so the rig can measure the world layer while the camera moves) and `map_report`
  (`debug/map_done.json`: the canvas on the window, where the ship landed, the world
  layer's state).
- **Two controls, not one**: `maps-follow-home-unfollowed` is the plan's (2-hour jump);
  there the camera is 22,000 km from the ship and the world layer is off, so it fails the
  check on a map of symbols only. `maps-follow-home-unfollowed-near` jumps 10 seconds:
  the ship goes about 30 km, out of the view but within the world layer's band, so the
  check fails on rendered pixels with Earth drawn behind. Both must fail.
- **What the first rig run found, and fixed:** BUG-210 (aboard, the celestial pass lit
  Earth and the planets with a world-axes Sun against hull-frame bodies, so at local noon
  Earth showed its terminator through the middle; one function, `ipc::celestial_sun_dir`,
  for the live frame and every view); a label's pill cap painted over the real globe
  (now a plain label); the Maps page staying open from one vantage into the next with its
  dev turn still running (`home` resets the dev switches, the sweep closes what it
  opened); the centred judge counting the star field's twinkle (solid 3x3 patches only).
  `maps-closed-after` came back identical to `home-clock-noon`.
- **The rig, second run (merged with main, 2026-10-21, sweep `20261021-145646`), 7 of 7
  captured, no panics:** `maps-follow-home` and `-later` pass "ship centred" (the hull's
  pixels 1.5 to 1.6% of the canvas, their centre 38 to 39 px, 3.1% of its height, from
  the canvas centre); both controls fail it as they must (0.00% changed); Earth below the
  ship fully lit at local noon after BUG-210. Costs, read from `HUMANITY_FRAME_COSTS=1`
  but marked CONTAMINATED by other sessions' builds and a second rig on the machine, so
  indicative only: the game at the same place (`home-clock-noon`) about 4.7 ms GPU a
  frame; the map at rest about 2.4 ms GPU and 5.7 ms CPU (`cpu.map_view`) per render, one
  render a second; the map while the camera turns about 0.85 ms GPU and 5.2 ms CPU a
  frame at half resolution; `cpu.maps_feed` 0.002 ms. The CPU cost is the scene's draw
  submission (`cpu.scene` about 3 ms, the same as the game's).
- **Seen in the captures, not fixed here (FIXED 2026-10-21 as BUG-211):** from the home
  (an equatorial synchronous orbit) the rendered Earth shows its NORTH POLE under the
  ship, where the equator at 122 degrees west should be. Present before BUG-140 and
  BUG-210 alike. The cause: aboard, every draw of a body was turned by the spin alone
  while its placement went through the hull frame; `planet_build::body_model_rot` now
  narrows the one f64 rotation. `maps-follow-home` shows the Pacific under the ship.
- **BUG-140's follow-ups in this view** (found by its fixer): the Lagrange points now
  take the orbit normal from the child's own motion (L4 leads, L5 trails, on every date);
  the overview looking down from ecliptic north with the planets going round
  counter-clockwise, and the reference rings in the equatorial plane, are held by tests.

### Increment 2: a solar system that looks real

**BUILT 2026-10-21** (status, what differs from the plan below, and the
measurements: "Increment 2 as built" at the end of this section).

- **Fix BUG-140 first** (DONE 2026-10-21, v0.1534.0), as its own lane: map the ecliptic with a proper rotation and
  follow every reader the bug names (the spin's sun azimuth, the station's phase and
  `over_its_longitude`, the travel tool, the hologram, the page). Proved against JPL
  Horizons positions for three dates, compared as numbers only.
- Bodies: the icon floor with phase-lit discs, the hand-off to the real globe over
  ~6 px, colours from `map_color` in `sol.json` (a build script computes it from each
  albedo map; `body_color` goes).
- Orbits: trail fade, size fade, adaptive sampling, the body as an exact vertex.
- Labels through `label_placer.rs`. Belts as point clouds from the real orbit
  distribution (MPCORB-derived, after a dated findings document on its terms).
- The star sky and Sun glare behind (world layer, re-rendered on direction change).
- Overlays as data: `data/maps/layers.ron`, `data/maps/reference_orbits.ron`,
  `data/maps/lagrange_pairs.ron`.
- The Diagram toggle (simplified mode).
- New theme tokens for map lines and labels in `data/gui/theme.ron` with their
  `src/gui/theme.rs` accessors and Settings rows (the theme lint and the editor
  coverage test both fail otherwise).

Files: `src/cosmos.rs` and BUG-140's followers (`src/engine/frame_lock.rs`,
`src/station/orbit.rs`, `src/renderer/hologram.rs`, `src/dev_travel.rs`), new
`src/gui/pages/maps/bodies.rs`, `maps/orbits.rs`, `maps/belts.rs`,
`src/gui/pages/maps/system_view.rs`, `data/star_systems/sol.json`, new
`data/maps/layers.ron`, `reference_orbits.ron`, `lagrange_pairs.ron`,
`scripts/build-body-colors.js`, `scripts/build-belts.js`, `data/gui/theme.ron`,
`src/gui/theme.rs`, `src/gui/pages/settings.rs`, `docs/reference/findings/2026-..-mpc-orbit-data.md`,
`docs/BUGS.md`.

Proof: snapshots `maps_system_overview` and `maps_earth_moon`; the rig vantage
`maps-system-overview` with the world loaded; the BUG-140 test (a planet's heliocentric
longitude against Horizons within 0.5 degrees on three dates, and the orbit direction
counter-clockwise seen from ecliptic north); `cpu.maps_symbology` under 1 ms at the
overview.

#### Increment 2 as built (2026-10-21)

Built as planned above, file by file, with these differences, each for a stated reason.

- **Bodies** (`src/gui/pages/maps/bodies.rs`). Each body is a small mesh of rings
  whose vertices are shaded by Lambert's law against the Sun's direction in the
  camera's frame, so the phase (crescent, quarter, gibbous) comes from the real
  Sun-body-camera geometry at any size; the night side keeps a tenth of the
  day side's light so the disc still reads against black (a map convention,
  said in the module doc). The icon floor is a minimum RADIUS by kind (Sun 5 px,
  planets 3.5, dwarf planets 3, moons 2.4, the rest 2); the hand-off to the real
  globe fades the icon out between 5 and 8 px across, and only where the world
  layer draws globes. The colour is `SolBody::map_color` through an exposure
  curve (`icon_lit_linear`: the brightest channel `m` becomes `m^0.4`, hue kept),
  because true reflectances run from 4% to 99% and a linear picture drew most of
  the family as near-black dots. The Sun is limb-darkened (`1 - 0.6 (1 - mu)`)
  with a point-spread glare round it (the two far terms of Spencer, Shirley,
  Zimmerman and Greenberg's 1995 glare function, at an exposure of 0.1), drawn
  in the symbol layer because the solar-system band's world layer is the star
  sky alone (below), and left out where the world layer draws the real Sun.
- **Colours** (`scripts/build-body-colors.js`, `data/star_systems/sol.json`,
  catalog version 6). Every body has a `map_color` and a `map_color_source`.
  Earth, the Moon, Mars and Pluto are the area-weighted (cos latitude),
  linear-light mean of their albedo maps; the Sun is its G2V colour; Mercury,
  Venus and the giants keep the page's old hand palette, SAID SO in their source
  field, until increment 5 brings their maps; every other body is a neutral grey
  at its own albedo. `body_color` is gone. Earth's mean of the cloud-free Blue
  Marble is a dark grey-blue (`#46474e`); through the exposure curve it reads as
  the pale blue-grey of a far Earth, but the true disc colour of Earth is
  whiter (clouds), which the cloud-free map cannot give.
- **Orbits** (`maps/orbits.rs`). Trail fade bright at the body to a floor of 22%
  half an orbit behind, measured in mean anomaly (time). The body's own eccentric
  anomaly is the first and last sample. 64 even steps, then a piece is split
  while its middle strays a third of a pixel from its chord, only on or near the
  canvas (at most 2,048 points). Size fade: gone under about 8 px across,
  thinning from 4 screens wide and gone at 200, measured at the nearer of the
  orbit's centre and its body (the first near-space snapshot drew Earth's orbit
  as a bright line straight through Earth because it was measured at the Sun).
  Drawn as one anti-aliased mesh with a colour per vertex, each faded in GAMMA
  space (`bodies::faded`): egui's own `from_rgba_unmultiplied` premultiplies in
  linear light, so a point at 8% opacity still added a quarter of its colour,
  the trails barely faded and the belts piled up into a solid blob. **Not in
  the plan, found by the snapshots:** the orbits of the asteroids and comets in
  sol.json (Vesta, Bennu, Halley and the rest) crossed the inner system as a
  dozen eccentric ellipses, the "bunch of lines" itself; they, and those bodies'
  icons, now show only when the body is hovered, selected or looked at, or the
  camera is in the solar-system band within 1.5 AU (the belts' dust stands for
  them otherwise); a moon's icon is left out while it sits on its own planet.
  And a SCALE fade beside the size fade: an orbit 300 to 3,000 times wider than
  the camera's distance fades out, so 1.5 km from the ship the Moon's and the
  planets' orbits are not lines across the view. The ship's own orbit stays as
  increment 1 drew it (`you_layer.rs`), through `MapScene::ship_place` for the
  Diagram.
- **Names** (`maps/labels.rs`). Through `label_placer.rs` in the design's order:
  you (the ship's name, never dropped), the anchor and anything hovered or
  selected, the Sun, planets bigger on screen first, dwarf planets, moons once
  they stand 22 px clear of their planet, asteroids and comets within 1.5 AU.
  Each name has a one-pixel dark edge (`map_label_halo`). The pills went; the one
  body whose card is open keeps its pill, since the card hangs from it, and a
  name is clickable to open it. **Not in the plan, found by the first overview
  snapshot:** the overlays' own names (L1 to L5, the ring names) are placed AFTER
  the bodies' (the Sun-Earth L1 and L2 labels had taken Earth's and Venus's
  spots), and an L point within 8 px of either of its bodies is left out.
- **Belts** (`maps/belts.rs`, `data/maps/belts.json`, `data/maps/MPCORB-header.txt`,
  `scripts/build-belts.js`). The findings document came first:
  `docs/reference/findings/2026-10-21-mpc-orbit-data.md` (the MPCORB header lets
  software include the data with its header, attribution and the update address;
  the MPC asserts copyright, so the file carries its terms, not the project's
  licence: `LICENSES.md`, `data/credits.ron`). The file is the brightest 13,000 of
  the main belt, 1,500 Hildas, 2,500 Jupiter trojans and 3,000 distant objects of
  the 1,575,913 orbits in MPCORB on 2026-10-21, by the MPC's own orbit-type flag,
  each row keeping its reference and orbit computer; 2.1 MB of JSON (JSON, not
  CSV, because the website's deploy copies only `data/**/*.json`). Moved to the
  page's date when the clock has moved six hours or the Diagram switched; drawn
  as one small triangle per point in one mesh, each class thinned by the square
  of how small it looks (full strength from 150 px of median orbit radius on
  screen), so the page's 60 AU overview shows the inner belts as a haze, not a
  blob. The credit line shows on the map whenever the dust is drawn.
- **The world layer behind the solar system** (`engine/map_view.rs`,
  `engine/ipc.rs` `ViewPasses::SkyOnly`, `maps/world_layer.rs`). Wherever the whole
  world cannot be drawn but is loaded (the solar-system band, the Diagram, away
  from the ship, off Live) the world layer renders the star pass alone, at night
  whatever the player's own sky, from the map camera's direction (the star pass
  depends on direction only, so it is exact at any distance), re-rendered when
  the view turns and once a second. **The Sun's glare is not from that render**,
  unlike the plan: the celestial pass cannot be run from a camera an AU from the
  ship (the render frame is the ship's, and the celestial files are another
  lane's), so the glare is the symbol layer's point-spread function above. The
  WHOLE world (`Draw`) now reaches out to the edge of near space (2 million km)
  instead of 60 km, as increment 1 left for this one; the ship's own draw lists
  go in only within 60 km, where it is more than a pixel.
- **Overlays as data** (`maps/overlays.rs`, `data/maps/layers.ron`,
  `reference_orbits.ron`, `lagrange_pairs.ron`). Five layers (orbits, names,
  belts, reference orbits, Lagrange points), each with the bands it shows in; the
  bar of switches is drawn from the file and offers only what the band can show.
  A reference ring now shows when the RING stands clear of the body's disc on
  screen (GEO round a 4 px Earth), not only once the body is 12 px across.
- **Diagram** (`maps/diagram.rs`, `MapScene` in the new `maps/scene.rs`). Distance
  from the Sun `r' = K ln(1 + r / 0.25 AU)`, K so that 1 AU stays 1 AU (Neptune at
  2.98 AU); bodies `0.012 AU x (radius / Earth's)^0.45`; anything round a planet
  spaced logarithmically from its enlarged surface. Everything goes through
  `MapScene`, so the camera, the orbits, the bodies, the belts and the ship move
  together; the world layer shows only the star sky; a banner says "Diagram:
  distances and sizes are not to scale". Switching it on flies to its overview.
- **Theme tokens** `map_space`, `map_orbit_planet`, `map_orbit_moon`,
  `map_orbit_minor`, `map_label`, `map_label_minor`, `map_label_halo`, `map_belt`,
  `map_belt_outer`, `map_reference`, `map_orbit_width`, `map_label_size`
  (`data/gui/theme.ron`, `src/gui/theme.rs`, Settings > Appearance "Maps colors"
  and Widgets "Maps: ..."); the web's `--map-*` variables from
  `scripts/gen-theme-css.js` (dark in both themes: the map is space).
- **Cost** `cpu.maps_symbology` (`data/performance/budget_systems.ron` row
  "Maps: symbols"), the whole build of the view's symbols; the report
  (`debug/map_done.json`) carries the page's own measure and what was drawn.
- **Code moved to keep the line budgets**: `MapScene` to `maps/scene.rs`, the
  world-layer want to `maps/world_layer.rs`, the rig's verbs, report and
  `apply_verb` to `engine/map_rig.rs`; new rig verbs `map: overview`,
  `map_layer`, `map_diagram`.
- **The web** (`web/pages/maps.html`, `web/shared/sol-orbits.js`, the web twin of
  the orbit model). The planets now stand at their REAL places on today's date
  (they were at a golden-angle spread), in their map colours, half lit toward the
  Sun, with fading orbit trails from their real ellipses, the real belts from
  the same belts.json, the Sun's glare from the same function, and names through
  `web/shared/label-placer.js`. Its distances stay compressed (it always used a
  log scale), so it says "distances not to scale". The belts and trails are
  drawn into an offscreen layer and reused while the view holds (0.6 ms a frame
  at rest, 12 ms on a frame the view moves, measured in the browser pane).
  Not mirrored, and why: the layer switches, reference orbits and Lagrange
  points (their files are RON and the deploy copies only JSON; the web page has
  none of those overlays yet); the real star sky behind (the web has no star
  renderer; its galaxy backdrop image stays). Test:
  `scripts/tests/sol-orbits-web.test.js`.


### Increment 3: into the ship, deck by deck

Needs increment 7 merged (`FloorSlab`, `storeys()`).

- The level picker; the cutaway; room names and fills; your dot and facing.
- Plan tiles for the straight-down view, baked and invalidated as in 5.7.
- Files: new `src/gui/widgets/level_picker.rs` (and its web component), new
  `src/gui/pages/maps/ship_view.rs`, new `src/engine/map_tiles.rs`,
  `src/engine/map_view.rs` (cut, orthographic), and only if object culling cannot do it,
  the shader cut (`src/renderer/pipeline.rs` `PSO_REGISTRY`,
  `assets/shaders/pbr/05-overrides.wgsl`, `assets/shaders/pbr/90-fragment-main.wgsl`,
  one builder, as the hot shader).
- Proof: vantages `maps-ship-upstairs` (the upstairs bedroom named, the ground floor's
  rooms not drawn over it) and `maps-ship-plan`; an edit through the editor's IPC
  changes exactly the tiles it touched; `permutation_tests` if the shader switch exists;
  `just verify-runtime`.

**BUILT 2026-10-22** (what differs from the plan above, and the measurements: "Increment 3
as built" below).

#### Increment 3 as built (2026-10-22)

Built as planned above, with these differences, each for a stated reason.

- **The levels are data** (`src/ship/levels.rs`, new). A ship's levels are the distinct
  storey floors of every body standing in it (`standing_bodies`: each zone, and each
  neighbour's home as the game draws it, its kind's shipped design on its plot), in ship
  metres, each ending at the next floor up or the ship's highest top. The shipped ship has
  two: "Storey 1 (ground)" (0 to 3 m) and "Storey 2" (3 m to the Commons' 8 m roof). The
  ordinal is IMDF's (0 the level the deck datum, y = 0, is on; negative is a basement,
  "Basement 1", "B1"), the names are the build editor's storey names, which now come from
  the same function (`level_name`; `editor_storeys::storey_name` calls it).
- **The cutaway is the editor's own, not a shader cut** (`src/engine/map_cut.rs`, new). The
  plan's preference, object-level culling, was not possible as it stood (a home's walls are
  one mesh per wall material across the whole ship), but the build editor already draws a
  home cut at a storey by rebuilding its shell from the home cut away, and leaves out what
  stands over the cut through `HomeDraw::cut`. The map does exactly that: its rule moved from
  `editor_storeys::cut_away_body` to `HomeStructure::cut_away` (the editor calls it), a level
  cuts each body at the storey that level is on (`levels::level_cuts`, `level_meshes`;
  neighbours through `ShipStructure::generate_meshes_at` and `neighbours::neighbour_view_at`,
  which keeps the whole view and the level view both, so moving between them bakes neither),
  and `HomeDraw::cut` became `Cuts` (one `CutAway` per body cut) so the machines, screens,
  doors, lift cars, plants, tents and pipes follow every cut the way they followed the
  editor's. `editor::wanted_cuts` says what the home should be drawn as (the editor while it
  is open, else the map's level, else whole) and the editor's per-frame follower
  (`follow_storey_view`) redraws it when that changes. The roof (both ceiling meshes) and the
  hull are whole meshes, left out of the map's view by object (`map_cut::inside_lists`). No
  shader, pipeline or permutation changed, so `permutation_tests` has nothing new to hold.
  The live frame is not drawn while the page is open, so the game never shows the cut.
- **When a level shows** (`gui/pages/maps/ship_view.rs`, new). The picker shows within three
  ship lengths of the ship's centre, aboard, on the System view, not on the Diagram, and only
  for a place of two levels or more. The page opens on the whole ship, as before: picking a
  level shows it, from far off flying to YOU (`MapAnchor::You`, new: where you stand,
  followed as you walk, turning with the ship as `Me` does) 90 m out at 60 degrees down;
  "Whole" goes back to the ship. "Back to me" and Home go to you on the level shown. A level
  is not shown from farther than the picker shows (the whole ship is the view again).
- **Rooms, names, you.** The rooms on a level are the HUD's own (`GuiState::room_bounds`,
  whose floor is on the level by `levels::level_at`), filled in their room type's colour
  (`default_color` in `data/rooms.ron`, now parsed and joined onto `RoomBounds::fill`;
  `map_room_fill` for a room with no type) at `map_room_fill_alpha`, outlined in
  `map_room_edge`, named by `RoomBounds::display_name` inside the room or not at all (your
  room first, then the bigger rooms, through the label placer). Your dot and a wedge the way
  you face are drawn on the level you are on: the feed gives your level (the level your FEET
  are on, the rule the editor seeds its avatar's storey by) and the room the HUD's room card
  names (`hud::room_card_at`), so the map and the game cannot disagree. With no world drawn
  under the level (a snapshot, before the world loads) its walls are drawn from the data as
  lines (`wall_in_storey`, the body's outline). The ship's orbit ring and the increment 1
  "You" marker are left out while a level shows.
- **Plan tiles** (`src/engine/map_tiles.rs`, new), as section 5.7 planned, with these
  specifics: a tile is a square of a level's floor in ship metres keyed by (level ordinal,
  zoom, x, y), zoom 0 is 512 m at 1 m a pixel and each zoom halves it to zoom 6 (8 m at
  1.6 cm a pixel); the page asks for the tiles its view covers at the zoom that keeps a
  tile's pixel no bigger than a screen pixel (at most 48, coarser if more; only over the
  ship; nearest the centre first) and draws a coarser tile's part where a finer one is not
  baked yet. The bake is not orthographic: the renderer's orthographic projection is not
  reverse-Z (the pipelines' depth test is), so a tile is rendered through a 2 degree field
  of view from 28.6 tile widths up (a 3 m wall's top sits 1.3% off at the finest zoom, 0.02%
  at the coarsest), with a new `ViewPasses::Plan` (the scene objects on black, no sky) timed
  as its own passes (`SceneView::MapTile`, `gpu.map_tile_*`, `cpu.map_tiles`, budget rows
  "Maps: plan tiles"). A tile draws the shell without roof or hull, the machines and their
  screens, and the plants; no people, doors or pipes. It is lit as the game is, by the Sun
  and the rooms' lights, so a tile baked at night is lamp-lit. Invalidation: every wall,
  slab, piece, light and room volume of every zone as the level cuts it, each zone's shell,
  each corridor, each neighbour's home and each machine is an item with a footprint and a
  hash; when the home's shell or machines are rebuilt (`HomeDraw::edits`, new) a level's
  items are made again and the footprints of the items that went or came drop exactly the
  tiles of that level under them. Plants growing are not edits.
- **The plan as the camera's mode**: "Plan" on the picker flies straight down over you (89
  degrees, the camera's limit; tiles and fills are placed on the floor plane by their
  corners, so they are exact at any tilt), the ship's bow up the screen; a drag pans, as on
  any flat map. The world layer is not rendered then (`WorldLayerWant::Plan`, new) and the
  sky's bodies are not painted over the floor.
- **Rig** (`src/engine/map_rig_ship.rs`, new): `{"map_level":"<ordinal>|you|whole"}`,
  `{"map_plan":"0|1"}`; the report's `ship` block (levels, the level shown, the rooms named
  and filled, your dot, the tiles wanted, kept, baked, the last bake's time, the tiles an
  edit dropped and the footprints that dropped them). Vantages `maps-ship-upstairs`, its
  control `maps-ship-upstairs-ground` (the ground level shown, which must fail the upstairs
  check) and `maps-ship-plan` (with a strip of road surface added upstairs through the
  `console` verb), judged
  by `scripts/lib/map-ship-check.js`; snapshots `maps_ship_upstairs`, `maps_ship_plan`.
- **The web** (`web/shared/level-picker.js`, `web/shared/ship-plan.js`, `web/pages/maps.html`):
  a "Ship" button opens the ship's plan level by level, with the same picker rule, the rooms
  filled and named, the walls of the level and your home outlined, from
  `data/maps/ship_plan.json`. The web cannot read the RON the ship is made of (the deploy
  copies only JSON), so the APP writes that file by its own rules
  (`the_web_ship_plan_is_current`, which fails when it is stale). Not mirrored, and why: the
  cutaway in 3D and the plan tiles (the web has no renderer of the ship; increment 8's `.glb`
  export is the route), and your dot (the web is not the game and has no position).
- **Not built here**: other people on the map (decision 7) belong with increment 4's search
  for people, and need the presence-sharing setting read on this page.

- **What the first rig run found, and fixed:** room names that were ids (`corridor_0`,
  `commons:room_1`, the HUD's fallback for a room the data does not name) over the Commons and
  the corridors: such a room is filled and not named now (`ship_view::room_is_named`, and the
  web's `roomIsNamed`); the plan read faintly, its walls being thin tops from straight above:
  the level's walls are drawn as lines over the tiles; the edit vantage's deck did not show from
  above (a deck is the floor's own grey), so the vantage adds a strip of road surface; and a
  placed piece's footprint for the tiles is now its type's size (it was a fixed 3 m, under an
  8 m train platform). And from the snapshots: your dot sat on your room's name ("Bed(dot)oom");
  the dot is placed first now and your room's name goes beside it.
- **The web plan, looked at in a browser** (2026-10-23, the preview server on this tree): the
  "Ship" button opens the plan on your home; "2" shows the Bedroom, Bathroom, Study and Hall
  named, the Dressing Room and Wet Room filled but too narrow to name; "1" the Dining Room,
  Second Bedroom, Entry, Hall, Storage Room, Kitchen and Living Room. One fix from it: the
  dialog was named by `aria-label`, which the shell's hover hints take, so the whole plan
  showed a generic "Tap or click to use this control" hint under the pointer; it is named by a
  hidden label now.

**Proof (2026-10-22 and 23):** every new test seen red first (its red output in its comment) and
green with the whole lib suite (4,968 passed); `just lints` and `just rig-tests` green;
`cargo check` for both feature sets clean; snapshots `maps_ship_upstairs`, `maps_ship_plan`
rendered and looked at. Rig, first run (release build of the merged tree, sweep
`20261024-011034`, 4 of 4 captured, no panics): `maps-ship-upstairs` passes (Storey 2, the
Bedroom, Study and Halls named, 8 rooms filled, your dot drawn; the capture shows the upper
floor with its furniture, roof and hull gone); its control `maps-ship-upstairs-ground` fails the
check as it must (the ground floor cut open at 3 m: Dining Room, Kitchen, Living Room, Workshop
named, the upper floor gone, the neighbour on p2 cut the same way, the Commons whole);
`maps-ship-plan` passes (25 tiles baked and kept, 25 MB, the last bake 4.4 ms of CPU) and its
edit through the `console` verb dropped exactly one tile of 25, the one under it;
`maps-follow-home` still passes (ship centred, 3.2% of the canvas height off). The symbols took
0.40 to 0.49 ms by the page's own clock on three vantages and 1.8 ms on one (a machine running
fourteen other agents' builds: the captures were marked contaminated).
Rig, second run, after the fixes above (release build of the tree merged with main on
2026-10-24, sweep `20261024-080924`, frame costs armed, 4 of 4 captured, no panics):
`maps-ship-upstairs` passes with no id named (Bedroom, Study, Halls; 8 filled; your dot);
the control fails as it must (17 rooms named, none by id); `maps-ship-plan` passes (27 tiles
baked and kept, the last bake 3.6 ms) and the strip of road dropped 2 tiles of 27, both under
it (its 4 x 6 m footprint straddles a tile edge), and the capture after the edit differs from
the one before in one 45 x 69 pixel patch, the strip; `maps-follow-home` still passes. Costs:
`cpu.maps_symbology` 0.27 to 0.44 ms on the three ship vantages (budget 1 ms); a tile's bake
0.47 ms of GPU (`gpu.map_tile_*`, the scene 0.42), inside its 2 ms; on the CPU, `cpu.map_tiles`
3.9 ms in a frame that bakes a tile (the scene's submission 2.75 ms), over the 2 ms the design
gave the bake, paid only in the frames that bake (27 here, then none). Left to look at:
`maps-follow-home`, where no level is shown, averaged 13.7 ms of `cpu.maps_symbology` over its
window while the page's own clock read 0.47 ms at its capture; it is the first vantage after the
boot, so its window holds the page's first opening. A first try of this run lost the game a
second after its first park with nothing in its log (sweep `20261024-080239`); the rerun was
clean. `just verify-runtime` on the same build: 3 of 3 vantages entered the world and
captured, no panics.

### Increment 4: finding your way aboard

- Search (rooms, places, services such as a mess hall or clinic, people who share their
  presence), a route along doors, corridors, stairs, lifts and rail, the time on foot at
  1.4 m/s and by rail, and "Guide me", which puts the destination in
  `target_markers` so the HUD points the way in the world.
- Files: new `src/ship/route.rs` (over `door_points`, `transit`, storeys), new
  `src/gui/pages/maps/search.rs`, `src/gui/pages/hud.rs`.
- Proof: route lengths on the shipped ship (front door to the Commons, p2 to street-1);
  a rig walk that follows the marker from the front door arrives at the Commons.

### Increment 5: every body a real globe

Albedo and colour maps for the bodies that have none (Mercury, Venus, the giants and
Saturn's rings, the large moons, Ceres, Vesta), fetched by a script from public
mission mosaics after a dated findings document on each source's terms. The engine's
celestial pass uses them, so the map gets them without map code. Loaded only within a
few radii of the camera (25 MB per full map today), mip-mapped.

Files: `scripts/fetch-body-maps.js`, `data/planets/*.ron` and `*_albedo.bin`,
`src/renderer/celestial.rs` (lazy loading), `docs/reference/findings/`.
Proof: a close vantage per body; memory with every body loaded at once under a stated cap.

### Increment 6: one sky clock

The eleven wall-clock sites become one `sky_seconds(state)` in `src/systems/time.rs`,
which the map can override while it is open, so a scrubbed date moves the real globes
and the ship in the world layer too. A lint (the `rig_pin_lint` pattern) fails on a new
`946_728_000` outside the clock module. Proof: the map at six months ahead shows Earth on
the far side of the Sun with the world layer on.

### Increment 7: onto and under the surface

- Follow on a planet (the frame-locked surface point becomes the anchor); the hand-off
  from the Earth band into the world layer's terrain and region meshes; the Planet view
  as the orthographic mode of the same camera; the level picker for OSM buildings and
  below-ground levels; ground translucency inside a rectangle.
- Files: `scripts/fetch-osm-region.mjs` (level, layer, tunnel tags; a new region format
  version), `src/terrain/osm_region.rs`, `src/engine/region_meshes.rs`,
  `src/gui/pages/maps/planet_view.rs`, the planet chunk shader behind its own switch.
- Proof: vantages `maps-silverdale-from-ship` (zoom from the ship down to Silverdale in
  one camera) and `maps-under-seattle` (a tunnel shown under translucent ground).

### Increment 8: the bridge screen and other ships

The map on an in-world screen (a world-screen provider at the screens' 10 Hz, the "Ship
Bridge viewscreen" of cosmos-architecture.md section 5), and other stations and ships
from data (`data/fleet.json`, every `data/stations/*.ron`).

### The web, every increment

Native first, the web mirrors (CLAUDE.md, dual-UI parity). `web/pages/maps.html` takes
the same camera model and data files each increment: body colours, layers and reference
orbits from the same files, the ship placed from `data/stations/home.ron` and the
server's shared world clock, the level picker and room plan from the ship's data, and the
ship itself as a `.glb` the app exports for the web model viewer (`web/shared/model-view.js`).
The web cannot run the engine, so its "world layer" is the model viewer's render of that
`.glb`. Where a step cannot be mirrored, the increment says so in this document.

## 7. Decisions for the operator

**All eight recommendations ACCEPTED by the operator, 2026-10-20 ("I like your
recommendations, let's keep going").** Build to them as written below.

1. **One continuous map, or the four tabs?** Recommend one map, with Ship, Earth orbit,
   Solar system and Galaxy as shortcuts on a breadcrumb; Night Sky stays its own mode.
   The web page already zooms continuously; native would catch up and lead.
2. **Where the map opens.** Recommend: on you, following, every time it is opened from
   the game, with Home or "Back to me" after you look elsewhere, and a setting for
   "open where I left it".
3. **Which way is up when following the ship.** Recommend the ship's own frame (Earth
   stays below the ship, as the residents see it) at ship scale, and the fixed stars
   beyond Earth orbit.
4. **The ship in Earth's shadow.** It goes dark for a while every day. Recommend
   realistic by default, with a Map light toggle that lights the map's view only (the
   dual-mode rule), never the game.
5. **True scale or a diagram.** Recommend true scale with an icon floor by default, and
   a Diagram toggle (log distances, enlarged bodies, labelled "not to scale") as the
   simplified mode.
6. **BUG-140 before the solar-system work.** The planets currently go round the Sun the
   wrong way. Recommend fixing it as the first lane of increment 2; it moves the
   station's phase and several readers, so it gets its own proof.
7. **Other people on the map.** Recommend showing only people who share their presence
   with you, through the privacy settings that already exist, and nobody else by default.
8. **The ship's name.** `data/stations/home.ron` calls the orbiting object "Homestead"
   and `data/fleet.json` calls the ship "UHS Mothership". Recommend the map label the
   ship "UHS Mothership" with "your home" on your plot, and the station file take the
   ship's name, so one thing has one name.

## 8. Risks

- **Cloud history on entering and leaving the map.** The cloud passes keep temporal
  history fitted to the player's camera (`ViewPasses` docs, `src/engine/ipc.rs:1870`).
  With the page open the live frame does not run, so the map camera is the only one,
  but entering and leaving are camera cuts. The increment 1 rig checks both directions
  for smearing; if they smear, the fallback is a history reset at the cut (as a
  teleport) or `SceneOnly` plus the real Earth drawn separately.
- **Frames.** The draw lists are in the home frame until `station_off` is added further
  down the frame, and the render frame is the hull frame while riding. The camera
  screens already handle the first (`src/engine/screens/camera.rs`); the map view must
  handle both, with a test that a point on the hull lands where the symbology says.
- **Two truths.** The map must take the ship's pose, Earth's spin and the Sun from the
  engine, never recompute them; otherwise the ship drifts off the place it hangs over.
- **Budgets.** `src/lib.rs` and `src/gui/mod.rs` are near their line budgets: increment 0
  frees `gui/mod.rs`, and `lib.rs` gets call lines only.
- **One GPU.** Every vantage runs one rig at a time, in the background, per CLAUDE.md.
