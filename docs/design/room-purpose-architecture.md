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

**Build status (v0.440):** the avatar mesh exists (a blockman on the respawner podium). The
showroom camera state (orbit + hide walls + backdrop swap + confirm), the backdrop scene
registry, and the portal-emergence transition are the next increments, in that order. The
avatar is the shared dependency they all render.

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
