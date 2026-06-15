# Room-purpose architecture

Status: keystone shipped v0.439.0. The rest is a phased roadmap (below). From a 4-lens
design pass (2026-06-13) grounded in the actual code.

## Philosophy

A room is not decoration; it is a verb. Every room in the homestead is a real place that
does a real job, and the game feature it hosts is the natural read of what that room is FOR
in actual life. The reality-fidelity rule ("the closer to reality, the less rework") becomes
a concrete engineering rule: each live, walkable room must KNOW its own function as data, and
that function drives one in-room action surface that opens the system you would really use
there. Twelve rooms become twelve data-declared functions in ONE registry, not twelve
hand-coded special cases. The two anchors (bathroom = who you are; bedroom = what you wear)
are the first two entries; they are the only ones that need a genuinely new system underneath
(a visible avatar). Build the avatar + the persistence + the room-function data model right
ONCE, and every other room lights up by surfacing a system that already exists.

## One source of truth (the anti-rework decision)

`data/rooms.ron` is THE single source of truth for room function (id -> name, purpose,
equipment, tags, and now `actions` + `access`). It already had all the rooms; it just was
not loaded by code. Do NOT add a `purpose` string to the live `RoomConfig` (that would be a
fourth drifting copy alongside the homestead_layout comments, fibonacci_homestead.ron, and
fibonacci_rooms.csv). Instead the live world JOINS each room's id to the registry at load
(the same data->flag pattern already used for spawn_room/hologram_room).

- `src/ship/room_types.rs` — `RoomTypeRegistry::load()` parses `data/rooms.ron` +
  `data/rooms/room_actions.ron`. New fields are `#[serde(default)]` so the 38 existing
  entries parse untouched.
- `gui::RoomBounds` carries the joined `display_name / purpose / actions / access`; the HUD
  reads it to show the room card and (later) drive the [E] action menu.
- In-room actions are DATA: a room's `actions: [..]` ids resolve to labels + a target `page`
  via `room_actions.ron`. Adding a feature to a room is a data edit, never code.

## The two hard dependencies (stated plainly so we do not corner-cut)

1. **Outfits need a body.** The player today is a first-person camera + physics capsule with
   NO visible mesh. "Customize your face" and "change outfits" both presuppose a visible,
   riggable avatar. Correct order: avatar (visible mesh) -> mirror render -> cosmetic layer
   -> THEN the bathroom/bedroom UIs. Building the wardrobe UI first is the rework trap.
2. **Persistence.** Appearance/identity fields live only in `GuiState` (RAM); `WorldSave`
   persists only inventory + skills + position/health. A customized character vanishes on
   restart. Appearance + chosen outfit must be added to `WorldSave` as `#[serde(default)]`
   fields (a home IS a WorldSave, v0.380, so "which character + how they look" belongs to the
   save). This is part of doing it right, not a follow-up.

## Spawn decision

Spawn in `respawner` (Respawn Chamber), not a new bathroom room. It is already where the game
says you "wake up" (a one-line data change), and a 2x2 room is too small for a character
SELECT stage. Split the operator's merged anchor: character SELECT = a full-screen menu shown
on world-entry at the respawner; appearance EDITING = the `wetroom` mirror (rooms.ron already
lists a mirror there); the `bedroom` is the wardrobe. A dedicated larger `bathroom` RoomConfig
can be added later (the engine already anticipates one: `fibonacci.rs::room_color` has a
'bathroom' key and rooms.ron has a full 'bathroom' entry).

## The 12-room registry

| Room | Purpose (game feature) | Hosts (system) | Access | Status |
|---|---|---|---|---|
| computer | In-game OS desktop / dashboard / tasks terminal | Mission Dashboard, Tasks, saves, settings, AI | private | exists |
| network | Comms + federation + market access | Chat, market, federation, identity | private | exists |
| respawner | Spawn + character SELECT + respawn + vitals | spawn flag, NEW select menu, Health/Vitals | private | partial |
| wetroom | Appearance customization at the mirror + hygiene + greywater | NEW appearance editor, vitals, greywater loop | private | partial |
| bedroom | Cosmetic OUTFIT wardrobe + sleep + personal storage | NEW outfit layer, avatar, time/sleep, inventory | private | new |
| kitchen | Cooking + nutrition | cooking, farming output, vitals/food, inventory | shared | exists |
| livingroom | Social + entertainment + community + visitor reception | multiplayer, guilds/governance, hologram | shared | exists |
| study | Fabrication + crafting + research + skills | crafting, PlayerSkills, blueprints, Library | shared | exists |
| garden | Farming / food growing | farming (live), towers, seed economy, greywater | shared | exists |
| garage | Power + vehicles + mining drone + turret + metalworking | electrical (live v0.437), vehicles, mining, combat, forge | shared | partial |
| depot | Bulk storage + logistics + trade staging | inventory (bulk), market staging | public | exists |
| hangar | Ships + spacecraft + heavy fabrication + launch | src/ship, heavy fab, future flight, terrain destinations | public | partial |

## Roadmap (after the keystone v0.439.0)

The keystone shipped: spawn in respawner + every live room declares its purpose + action list
from `data/rooms.ron`, surfaced in-world as a bottom-left HUD card. Next:

1. **Persistence unblock** (invisible but foundational): add `appearance: Appearance` +
   `outfit: Outfit` to `WorldSave` as `#[serde(default)]`; round-trip in save_load.rs like
   inventory+skills, with a test. Closes restart-amnesia BEFORE any avatar work.
2. **Route in-room actions to existing pages** via the live machine [E] surface: give machine
   catalog entries an `opens_page` field and route the existing `hud.rs` selected_machine [E]
   card to the room-action `page`. Lights up ~9 rooms with systems that already exist (reuse
   the machine system, NOT the orphan `Interactable` component; flag that for cleanup).
3. **Visible avatar (phases 0-1)**: add an `Appearance` ECS component + reuse
   `equipment_slots.json` as the data-driven slot model (never a code enum). Render a
   placeholder humanoid mesh at the player Transform via the existing instanced PBR path, so
   the third-person camera + a mirror have something to show.
4. **Bathroom/wetroom appearance editor (anchor 1)**: character-SELECT menu at the respawner
   + appearance editor at the wetroom mirror, binding the EXISTING profile.rs body fields to
   the avatar mesh.
5. **Cosmetic system + bedroom wardrobe (anchor 2)**: `data/cosmetics/cosmetics.csv` (slots
   reuse equipment_slots.json); cosmetics are craftable/tradeable ITEMS (flow through
   inventory/crafting/market for free, structurally no store code path); skin the avatar +
   render cosmetic gltfs on slot sockets; wardrobe = an inventory FILTER on slot-tagged items,
   equip -> `Outfit.equipped[slot]`, preview in the dressing mirror; sleep->save at the bed.
6. **Federation + multiplayer-readiness**: extend the signed-profile gossip with the PUBLIC
   half of Appearance (body_type/outfit/hair/eye) so visitors render each other; keep the
   PRIVATE real-life measurements local (the profile.rs PRIVATE/PUBLIC line IS the federation
   boundary). Honor per-room `access` for visit-view dispatch. Index homes by berth id even
   with one home, so the mothership-ring framing needs no repaint.
7. **Deep room arcs**: garage turret minigame, hangar ship-build UI, depot climate-zoned bulk
   logistics, livingroom entertainment. Each its own arc; the rooms already surface their core
   systems from step 2, so these are depth, not blockers.

## Character-select showroom (operator vision, 2026-06-13)

The character-select / customize experience the spawn flow builds toward:

- **Showroom view.** When selecting/creating a character, the room walls + ceiling vanish
  and the avatar stands on a podium while an orbit camera circles it. The player can swap
  the BACKDROP environment to preview the character in different places: spaceship interior,
  Earth, Mars, the Moon, floating in space, a forest, under the ocean, a mountaintop, a
  beach, etc. Each backdrop is a data-declared scene (skybox + ground + lighting preset) so
  the list is infinite-of-X, not a hardcoded set. Backdrops reuse the existing sky/stars
  renderer + terrain/biome assets where possible.
- **Selection -> emergence.** Once a character is confirmed, the camera transitions from the
  orbit showroom to first-person, and the player "emerges" into the world: the avatar is
  re-enclosed in the respawn pod, and the player steps out through a 2x2 m doorway/portal on
  the side of the respawner into the wetroom. The respawner and wetroom are adjacent; the
  portal is the shared wall. This makes the wake-up diegetic: reconstructed in the pod ->
  walk out the portal -> you are in your home.

**Build status (v0.443):** the showroom is live: orbit-locked camera (drag to spin, wheel to
zoom, no WASD/pan), hidden home, ground-disc backdrops, live appearance + wardrobe editing,
"Enter your home" emerge, all persisted. Remaining: per-backdrop SKYBOXES + the
portal-emergence transition.

**Backdrops as REAL in-game locations (operator vision, the renderer showcase).** The
backdrop list should eventually not be flat color discs but ACTUAL in-game places, so the
character preview doubles as a tour of the renderer: the spaceship interior, the surface of
a generated Earth, Mars, the Moon, the ocean floor, a forest, a mountaintop. The north star:
once planet generation + real-terrain heightmaps land, "mountain" should literally place the
avatar on Mount Rainier (the closer the in-game Earth terrain matches the real-world terrain,
the better). So `data/showroom/backdrops.ron` is designed as a registry that will grow a
`location` field (a world/biome/coordinate reference) the showroom camera teleports a preview
render to, replacing the placeholder ground tint. Keep backdrops data-driven so adding a real
location is a data edit once the terrain it points at exists.

## Floor plan (operator spec, 2026-06-14) -- to implement

The rooms currently auto-place in a Fibonacci spiral with a doorway cut on the center of
every wall (overlap-detected). The operator specified a PRECISE floor plan instead, which
needs (a) explicit room positions for the adjacency and (b) a per-room, per-WALL config
(solid / doorway / window / open / mirror), replacing the auto-doorway-on-every-wall.

Per-wall config needed per room (N/S/E/W each one of: solid, door, window, open, mirror):

- **computer (1x1)** and **network (1x1)**: NO doors on ANY side (all solid). They are
  ABSTRACT stations, not walked into -- computer = local files/folders/games, network =
  online services (chat + the rest). You interact, you do not enter.
- **respawner (2x2, fib #3)**: NO walls at all (fully open). Its EAST side is the shared
  boundary with the wetroom and IS the 3 m x 3 m mirror/portal (see wetroom).
- **wetroom (3x3, fib #4)**: only a NORTH doorway (into the bedroom). Its WEST wall is a
  3 m wide x 3 m tall MIRROR / portal (the shared respawner-east boundary) -- this is the
  character-customize mirror + the emergence portal. Other walls solid.
- **bedroom (5x5, fib #5)**: SOUTH door (into the wetroom), WEST door (into the kitchen).
  NORTH and EAST walls are BAY WINDOWS looking into the study and the garden respectively.
- **kitchen (8x8, fib #6)**: connects EAST to the bedroom (the bedroom's west door). Other
  connections per the spiral / later spec.
- Larger rooms (livingroom, study, garden, garage, depot, hangar): adjacency + windows TBD
  with the operator; the bedroom's north/east windows imply study is north of the bedroom
  and garden is east.

Implementation notes: `RoomConfig` already supports `position: Option<[f32;3]>`, so place
the small core rooms explicitly to realize this adjacency. Add a per-wall enum to the room
data (data-driven, infinite-of-X) and have the homestead mesh generator honor it instead of
cutting a doorway on every overlapping wall. New mesh kinds: a WINDOW (transparent/framed
panel) and a MIRROR (a flat reflective-tinted panel; later a real planar reflection). The
respawner-east / wetroom-west mirror is also the showroom emergence portal.

## Construction mode + modular building pieces (operator direction, 2026-06-14)

The operator does NOT want the floor plan (doors/walls/windows/trim) hardcoded into the
homestead mesh generator. Instead, build CONSTRUCTION MODE with modular, reusable building
PIECES that plug into a wall grid, so any layout is assembled from parts (and later edited
in-app). This supersedes the "hardcode explicit positions + per-wall enum" approach in the
floor-plan section above; that floor plan becomes the DEFAULT arrangement of pieces, not
baked geometry.

Pieces to design (each a data-defined part with a consistent size + a mesh):
- **Door** -- one canonical door that plugs into any wall (today doorways vary in size; they
  should be a single standard size). A door is a piece placed in a wall slot, not a hole cut
  per-overlap.
- **Door frame** -- a proper frame around the doorway (the operator: trim going around the
  door creates the frame).
- **Floor trim + ceiling trim** -- baseboard + crown molding around the room edges; the same
  trim wraps a doorway to form its frame.
- **Window / bay window** -- a framed transparent panel (the bedroom's north/east walls).
- **Mirror** -- the wetroom panel (also the showroom emergence portal).
- **Wall / floor / ceiling** segments -- the base pieces; the ceiling/roof piece should be
  toggleable or transparent (atmospheric tests: air transfer between rooms + venting), which
  is why the baked ceiling was removed (v0.445).

This is the `src/systems/construction` system finally made real (it has a Blueprint registry
+ the AutoRouter already): a piece catalog (data), a placement/snap system, and a build-mode
UI. It is a large arc; the homestead would then be GENERATED by placing the default pieces
rather than the current bespoke wall/ceiling mesh code.

**Build status (v0.453) -- the data-driven per-wall layer is SHIPPED.** The first and most
important slice landed: walls are now built from a per-wall `WallKind` instead of "cut a
doorway on every overlapping wall." Each `RoomConfig` in `data/blueprints/homestead_layout.ron`
carries `walls: (north/south/west/east: Kind)` where Kind is one of
`Auto | Solid | Door | Window | Open | Mirror`:
- `Auto` (the default for any omitted wall) reproduces the OLD behaviour -- a STANDARD-size
  door where a passable neighbour faces the wall, else solid -- so unconfigured rooms are
  unchanged (zero regression). It also fixes the "doors vary per room" bug: all openings use
  the single `door_width`/`door_height` from the layout's standard-dimensions block.
- The operator's specific rules are now data, not code: respawner = all `Open` (doorless,
  wall-less alcove), computer + network = all `Solid` (sealed closets), wetroom east =
  `Mirror` (the 3x3 portal), bedroom north + east = `Window` (bay windows).
- New mesh families generated alongside walls: **trim** (baseboards + crown + door/window
  frames, wood), **windows** (tinted-glass panes), **mirrors** (emissive portal panel), and
  **ceilings** (built always, drawn only when the roof is toggled on).
- **Toggleable roof**: `gui_state.show_roof` (default OFF so the sky shows through the open
  top). Toggle with the **R** key in-world or the **Settings -> Graphics -> Show roof**
  checkbox. Reuses the showroom hide path: the showroom still hides everything regardless.
- Verified by `cargo test --features native --lib ship::fibonacci` (RON parses with the new
  walls; the special rooms carry the right kinds; generation emits every mesh family).

**Still deferred (NOT done):**
- A real alpha-blend pass. Windows + the portal are currently OPAQUE-tinted / emissive (no
  transparency), because the main PBR pipeline is `BlendState::REPLACE`. True see-through
  glass + a translucent roof need an `opacity` material field + a blend pipeline variant.
  Tracked as renderer debt; the per-wall layer does not depend on it.
- ~~The build-mode editor~~ **SHIPPED (v0.455).** `src/gui/pages/construction.rs`: press **B**
  in-world to open a panel listing every room with a per-wall dropdown (N/S/W/E -> WallKind) +
  a ceiling-height slider. Changing any wall rebuilds the home LIVE (the engine holds the
  layout in `EngineState.homestead_layout`, watches `construction_dirty`, and re-runs
  `generate_from_layout` -> `apply_homestead_meshes`). "Save layout" writes the layout back to
  the RON via `fibonacci::save_layout` (data-only; comments not preserved). The camera freezes
  + the cursor frees while editing. So per-wall config is now a GUI action, not a RON edit --
  closing the one GUI-first gap. (Remaining build-mode polish: a full snap/placement grid for
  arbitrary pieces, per-room height + the "go down" deeper floors, and add/remove rooms.)
- Explicit floor-plan POSITIONS. Rooms still auto-place in the Fibonacci spiral, so which
  physical wall a `Window`/`Mirror` lands on follows the spiral; if a special wall faces the
  wrong way in-app, move the kind to the correct side in the RON (no recompile). Pinning
  exact positions to match the operator's adjacency sketch is a follow-up.

## Showroom backdrops = real in-game bodies (operator direction, 2026-06-14)

Refinement of the backdrop vision: the showroom should render ONLY the player + pedestal +
star skybox (nothing else, so nothing can occlude). Selecting a backdrop then TELEPORTS the
avatar onto the actual in-game body: pick Earth and the avatar stands on the green Earth
sphere in the live solar system, Mars on the red sphere, etc. (placeholder colored spheres
until real terrain/textures exist). "Space" = just the avatar against the stars. So a
backdrop is a (body id + stand position) the showroom camera + avatar teleport to, reusing
the real solar-system bodies the renderer already places, not a flat ground tint.

## Appearance / outfit data shapes (locked now, built later)

```rust
// src/systems/appearance.rs (future)
struct Appearance { body_type: String, height_scale: f32, skin_tone: [f32;3],
                    hair_style: String, hair_color: [f32;3], eye_color: [f32;3] }
struct Outfit { equipped: HashMap<String,String> } // slot_id -> cosmetic_item_id
// WorldSave gains: #[serde(default)] appearance: Appearance, #[serde(default)] outfit: Outfit
// data/cosmetics/cosmetics.csv: id,name,slot,model,tint_mask,unlock,description
//   slot is one of equipment_slots.json (head/chest/legs/feet/hands/back) - reuse, never fork
```
