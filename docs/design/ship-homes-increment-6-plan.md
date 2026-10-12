# Increment 6 build plan: the starts as data, the Cabin, the mess hall

Date: 2026-10-12. Read against main at 7b2a09394 (v0.1487.1 plus the 5b Wave 1 merges; 5b lanes 2A, 2B and 3A are in flight in worktrees a21d7133, aa857953 and a349c3ae). Sources: docs/design/ship-homes-and-logistics.md (sections 1, 1.1, 1.2, 1.2b, 1.3, 1.4, 1.5, 2.8, 2.9, 3; increment 6 at :1076-1096; the increment 3 and fleet ledger "as built" notes at :875-925; question list at :1188-1219), docs/design/ship-homes-increment-5b-plan.md (shape; its 1.2 says fresh server homes are "increment 6"), docs/PRIORITIES.md (TIER 0 :121-182, :270-273, :298-301, :362-383), the journal (docs/history/journal-archive-2026-10.md:161 and :231, data/coordination/orchestrator_state.json), docs/design/engagement-modes.md, docs/design/characters-and-servers.md, CLAUDE.md, and the code below. Every line number was read today. Nothing was edited or booted.

## 0. In short

- **Much of the mess hall is already built** (section 1.1): the `mess_hall` zone type and the mess hall volume (increment 3), the relay's free, metered crew meal (`take_meal`, one a meal time, a "used" line in the fleet ledger), and the game eating it as a Basic Ration. What is missing is any way to take a meal in the world (today only Inventory > The fleet's button within 5 m), tables, a counter, water, and meals offline.
- **Not built at all:** starts as data, a cabin, a start in `game_join`, any home choice, and the operator's 2026-10-04 rule that server worlds start every player fresh (today one save, offline_home.json, is shared by your home and every server: showroom.rs:38-39 says so).
- **Starts are data.** NEW data/homes/starts.ron (Cabin, Apartment not offered yet, Homestead the default), read by the relay and the game through NEW src/ship/starts.rs. A row holds the design, plot kind, household sizes (the old Family/Solo choice, now per start), engagement defaults per domain, shared places, kit, storage seed, opening quest, whether the home has its own air, and which homestead world content it carries.
- **The Cabin.** NEW data/homes/shipped/cabin.ron (and the player's own copy data/homes/cabin.ron): one room 3.0 x 2.5 m = 7.5 m2 (the UK space standard's single bedroom), 2.6 m high, door in the west wall. NEW data/machines/cabin.ron: bed, nightstand, wardrobe, desk, chair, wall screen. Sixteen cabin plots c1..c16 stand east of First Street, each opening on a new block hall zone `cabins-1` (with the block's shared washroom), reached from First Street.
- **A block is a zone, not a parent plot** (resolved in 3.3): a walkable shared hall cannot sit inside any plot (validate refuses a zone overlapping a plot), and a parent plot's own door tube would overlap its cabins' tubes.
- **Zone types** `recreation`, `cabin_block`, `apartment_block`, `street` join `mess_hall` (built) in data/blueprints/zone_types.ron.
- **The relay** reads `start` in `game_join` and claims a plot of that start's kind, always keeping a plot already held. A guest's welcome says which kinds are free. NEW `game_home_request` / `game_choose_home` -> `game_home`: your own home on this server, free counts per start, and the starts' own words; choosing works only while you hold none (moving house is question 2).
- **One save per world.** Your home stays saves/offline_home.json; each server gets saves/servers/<server>-<identity>.json, made FRESH from the chosen start (its kit, its storage, its opening; only your name, look and outfit come along). `WorldSave` gains `start`, `household`, `server`, `ship_place` and `ship_meals` (replacing the unused `design`); `AppConfig` loses `home_variant` and `home_plots`. Which save is live follows the pairing (Characters' WHO/WHERE), never a moment's step-out.
- **The game's home** is the save's start on the save's plot. A welcome naming a plot of another kind builds that kind and never gives the plot up (today it does: home_plot.rs:364-374). A cabin spawns none of the homestead's world and breathes the ship's air.
- **The mess hall serves.** Tables, chairs and a serving counter (ship machine rows). E at the counter opens its card: Take a meal (in a server's world the relay's `take_meal`, offline the ship's stores on this computer, one a meal time, counted in the save) and Fill your bottles (drinking water). A second mess hall, the South Commons, at First Street's middle, meets the design's 5-minute rule for p5..p12 (the first thing cut).
- **Choose where you live** is in Characters (the new-home row, and each server's card), Settings > Gameplay (replacing Home Design, settings.rs:3722-3740, not :3585-3602 as the design cites) and the Home page. The website's chat gets a "Where you live" window (command palette) that shows and chooses, from the relay's answer.
- **The Cabin's own opening:** check your vitals, walk to the mess hall, eat, see the recreation room, go home.
- **Proof:** `just verify-starts` = `node scripts/verify-copresence.js --starts`, ONE game and ONE scripted walker on a throwaway relay: the game is marked offline, chooses a Cabin on the server, arrives fresh in its own cabin with nothing from home, walks to the mess hall, eats, goes back home and finds its things there; the walker joins as a Cabin, reaches the mess hall and eats.
- **Waves:** Wave 0 (one builder, the contract) can start now; Wave 1 (seven lanes) after 5b's 2B merges; Wave 2 (two lanes) after 5b's 2A and 3A merge; Wave 3 the rig. lib.rs, gui/mod.rs and msg_handlers.rs stay under budget (section 5).
- **Toward "properly playable":** after this, a new player on the operator's server can choose a Cabin, arrive in a room of their own, walk to the mess hall, eat and drink, and walk home inside ten minutes without starving and without bringing anything from their own home; a homestead player on a server starts fresh as the operator decided.

## 1. Today's code (file:line)

### 1.1 What the design's increment 6 text already has

| Design item | State | Where |
|---|---|---|
| Zone type `mess_hall` | DONE (increment 3) | data/blueprints/zone_types.ron:212-222; the volume `mess-hall` in the Commons body, data/blueprints/ship_structure.ron:970-978 |
| "The mess hall serves free, metered crew meals" | DONE on the relay; no in-world way to take one | src/relay/handlers/ship_stores.rs `take_meal` :437, `handle_take_meal` :427, `STORE_REACH_M` :204; routed at msg_handlers.rs:3864; the store at mess-hall spot (2, 2), data/food/ship_stores.ron:48-56; the fleet ledger's "meal" kind priced as `ration_basic_0`, data/ship/fleet_ledger.ron; the game eats it, src/engine/fleet.rs `on_meal` :141-153; the only button is Inventory > The fleet, src/gui/pages/fleet_ledger.rs:495-520, within `STORE_REACH_M` :36-38 |
| "The home is stored per save, not per install" | PART: the character's own home design and household machines are in the save | persistence.rs `WorldSave::home` :194-202, `SavedHome` :213-222; src/engine/own_home.rs; the plot's BOX is in the save (`home_plot_box` :173-180). Still per install: the Family/Solo machine file (config.rs `home_variant` :837-845, read by machines.rs `home_ron_path` :1118-1129) and the remembered plot (config.rs `RememberedPlot` :352-362, `home_plots` ~:501-511) |
| "WorldSave gains a plot id and a ship id" | NOT DONE | persistence.rs :15-208 |
| Plot kinds and child plots | DONE (1a) | ship_structure.rs `Plot` :256-273 (`parent`, `kind`), `validate_plots` :759-808, the "a cabin's corridor runs through its block" exemption :847-853 |
| starts.ron, cabin.ron | NOT DONE | data/homes/ has homestead.ron and shipped/homestead.ron only |
| Zone types recreation, cabin_block, apartment_block, street | NOT DONE | zone_types.ron |
| "game_join carries the requested start" | NOT DONE | game_state.rs `JoinHome` :443-447, `from_join` :491-498 read `ship_hash` and `home_spawn` only |
| "Choose where you live" replacing Home Design | NOT DONE | settings.rs:3722-3740 |
| Server worlds start fresh (operator, 2026-10-04) | NOT DONE | one save for all worlds: save_load.rs `active_home_path` :96-98; showroom.rs `WHO_HINT` :38-39 |
| Proof: "the walker joins as a Cabin and reaches the mess hall" | NOT DONE | |

### 1.2 Plots, kinds and designs
- **The ship file** (data/blueprints/ship_structure.ron): zones `commons` (65, 0, 20) 34 x 55 x 8 with the mess hall volume (Commons-local x 0..30, z 0..8) :68-983, `street-1` (65, 0, 85) 10 x 1100 x 4 with a glass roof :985-1006, one corridor Commons to street at lat x 70 :1008-1018; twelve plots p1..p12, all kind `homestead`, (0, 0, 99(N-1)) 55 x 3 x 89, doors on the west side only :1019-1188; `default_plot: Some("p1")` :1189; districts :1190-1282 (res-1 covers x 0..100, z 0..1195 and draws nothing; mall-1 x 80..120, z 205..245); `id: "mothership-1"` :1283. East of First Street, x 75..100, z 85..1185 is empty.
- **Designs.** `HomeDesign {kind, door, body}` ship_structure.rs:295-303; `load` disk first, else the built-in copy with `note_builtin_copy` :327-353; `built_in_ref` parses every row of `embedded_data::SHIPPED_HOMES` once :368-378. `SHIPPED_HOMES` holds only the homestead (embedded_data.rs:231; `HOME_HOMESTEAD_RON` :168; the player's own copy is `own_home_text` :249). build.rs FINGERPRINT_INPUTS stamps the whole `data/homes/shipped` folder, so a new shipped design is fingerprinted with no build.rs change.
- **Assembly.** `assemble` refuses a design of another kind :1610-1612, a design that does not fit :1613, a door that does not meet the plot's lat :1615-1621, and a corridor leaving through another wall :1627-1644. `assemble_from_own` :1766-1786; `plot_and_kind` :1789-1801; `design_fits_plot` :924-933. Corridors need a clear gap of `CORRIDOR_MIN_RUN` 0.25 m :82 (`tube_between` :957-1083). The twelve-homes test asserts every plot is a 55 x 3 x 89 homestead :3640-3642.
- **Neighbours** draw each other plot as its kind's shipped design; a kind with no built-in design is skipped (neighbours.rs `make_view` :143-162, the skip at :147).

### 1.3 The join, the claim and the welcome
- **Join.** msg_handlers.rs `handle_game_join` :3074: `JoinHome::from_join` :3127, `refused_join` :3128-3130 (home_plots.rs:42), `assign_home` :3137, the welcome built at :3281-3295 (`home_plot` from `HomeAssignment::home_plot_json`, game_state.rs:415-425).
- **Claim.** game_state.rs `assign_home` :1083-1110 passes every plot id to `Storage::claim_plot` (storage/plots.rs:50-87), which keeps a held plot that is still in the list and otherwise claims the first free one. A kind-filtered list would DROP a held plot of another kind (:60-69): the split in 3.5 exists to avoid that.
- **World.** `GameWorld::new` :509-537 loads the ship (`load_ship` :585-599); every ship zone and labelled volume becomes a relay room (ship_world.rs), so a new zone or volume joins the explore quest.
- **Routing.** relay.rs:3745-3748 (`game_join`), :3792-3796 (the 5b shared-build arm); `perception_rate_allows` msg_handlers.rs:3767; `/api/server-info` carries `ship {id, hash}` (api.rs:1450-1475).
- **Game side.** home_plot.rs: `add_join_fields` :488-495 (ship_hash, home_spawn); `plan_welcome` :333-376, where a plot of another kind ends in `refuse(does_not_fit_sentence, true)` at :364-374, giving the plot back; `apply_welcome_home` :769-863; `boot_plot` :694-697 and `assemble_for_boot` :703-719 read `GuiState::home_plots` (AppConfig); `remember_plot` :747-764; `bring_home_back` :658-673; `GUEST_ARRIVAL` :205 (the rig's judge checks its first words, copresence-judge.js:1184). The router: net_route.rs `route_game_message` :11-21 (fleet and shared_build hooks).
- **The walker.** scripts/second-player.js builds `game_join` at :629-639 (ship_hash, home_spawn).

### 1.4 Saves: one save for every world
- `active_home_path` = saves/offline_home.json (save_load.rs:96-98). Startup spawns a fresh player with the starting kit and `gs_first_steps` (lib.rs:1381-1405), loads that save (:1417), spawns three asteroids inline (:1418-1463), loads the machine layout through `home_ron_path` (:1528), applies the config (:1689; `copresence_solo` from `last_world`, config.rs:1979) and then the save (:1701-1743).
- `apply_save_to_world` save_load.rs:429-676 returns early for any `kind` but "offline" (:435-438). It is authoritative for the backpack, skills, vehicles, crops, builds and drone, and not for credits (-1 keeps), quests (None keeps), asteroids (None keeps) or machine levels (an empty list changes nothing).
- The picker's Play applies a save by NAME onto the running world (lib.rs:14562-14620; showroom.rs `load_homes` :172-210 scans saves/ top level only). Settings > Data's Restore uses the same path (save_load.rs:872-897).
- `save_active_home` / `save_home_at` :754-828 write the one path. `starting_kit` reads data/world/player.ron `starting_items` (save_load.rs:678-694; player.ron:21-46).
- The kind-of-home axis exists unused: `WorldSave::kind` "offline"/"server"/"real" (persistence.rs:45-50); `design` defaults to "fibonacci" and is read only by the picker's detail row (:51-54).

### 1.5 Settings' Home Design and the picker
- Settings > Gameplay "Home Design": Family or Solo, writing `settings.home_variant` (settings.rs:3722-3740; GuiState gui/mod.rs:4504-4506), which picks data/machines/home.ron or home_solo.ron per install (machines.rs:1124-1129, read at 8 call sites).
- The Play picker (showroom.rs): WHO is the offline saves; WHERE is Your Homes (with `NEW_HOMESTEAD` "My Homestead (new)" :24), Open Net servers (Enter only when connected, :539-561), Closed Net. Entering records the pairing (gui/mod.rs `record_pairing` :2944, `apply_last_pairing` :2963-2986; lib.rs:7313-7330 sets `copresence_solo`). Onboarding starts a new player at home alone (gui/first_steps.rs:42-48).
- The website has no game settings (web/pages/settings.html sections). The fleet ledger's web mirror is a chat window on the signed-in socket, reached from the command palette (web/chat/chat-fleet.js; data/commands.json `openFleetLedger`; chat-ui.js:2401; app.js:1559): the precedent for this increment's web half.

### 1.6 The mess hall and meals
- Machines do not collide (wall_collision.rs has no machine colliders), so furniture never blocks a walk; the crew walk straight lines between chore spots, and their meal seats are at mess-hall-local x 29, z 1.5..7.25 (data/npc/chores.ron:194-233), with work spots at (3, 4), (11, 4), (19, 4), (24, 3), (24, 6), (25.5, 4.5), (27, 3), (27, 6).
- Furniture machine types exist in home.ron's catalog with Kenney CC0 models: `dining_table` :1561, `chair` :1541, `couch` :1521, `bed` :1459 (a double-bed model), `nightstand` :1481, `wardrobe` :1501, `desk` :1621, `wall_screen` :2340 (no model, no power). Models on disk: assets/models/furniture/ (15 pieces; no single bed, no counter).
- data/machines/ship.ron (the Commons' rows) has an EMPTY catalog on purpose (:17-22) and three power rows from the family home's `battery_7` (:123-126). Only home.ron names it (`ship_machines`); `merge_ship_machines` machines.rs:1187-1203; `split_for_save` sends every connection touching a ship row back to ship.ron (machines.rs ~:1253-1275).
- The pinned machine card grows a Trade button for a trading post (lib.rs:14086-14100 sets `machine_card_vendor`; hud.rs:1813-1823 draws it).
- Water: a bottle is filled at a water tank's walk-up card (src/systems/fluids.rs:1-17). A home with no tank has no way to drink after its kit.
- The game's stores list comes only from the welcome's snapshot (fleet.rs `stores_in` :323-339).

### 1.7 The opening
- `gs_first_steps` (data/quests/getting_started.ron:46-107) is a homestead script (garden tower, workbench, smelter, chest, front door); it is accepted at spawn by lib.rs:1383. Travel places are data/entities/destinations.ron: fixed `pos` or `OwnFrontDoor` (quests/mod.rs `DestinationAnchor` :166-175, `publish_front_door` :284).

### 1.8 Homestead assumptions a Cabin meets
- world_load.rs spawns the herd (livestock.ron, anchored to machines, :459-500), decoration plants (:685), wild spawns at fixed homestead-frame points (:730), and the hologram, falling back to (-0.5, 1, 2.5) when the home has no hologram room (:196-201).
- Each home breathes its own air (ship_space.rs `home_air_volume_m3` :71; survival_env.rs `whereabouts` :324): a 19.5 m3 cabin with no air machines would be a sealed box.
- `ShipStructure::lighting_watts` charges every zone's lights to the player's own meter (:720-727), so the new hall gets a glass roof and no lights of its own, like First Street.

### 1.9 Constraints
- **Line budgets** (tests/file_size_ratchet.rs:376-417), measured today:

  | File | Lines | Budget |
  |---|---|---|
  | src/lib.rs | 16,476 | 16,700 |
  | src/gui/mod.rs | 4,683 | 4,800 |
  | src/relay/relay.rs | 6,322 | 6,400 |
  | src/relay/handlers/msg_handlers.rs | 4,653 | 4,660 |
  | src/engine/frame_ws_poll.rs | 1,698 | 1,800 |
  | src/gui/state_types.rs | 1,836 | 1,850 |

  Not budgeted: settings.rs, showroom.rs, home_plot.rs, ipc.rs, game_state.rs, net_route.rs.
- **In-flight 5b work owns:** engine/household.rs, net/protected.rs, engine/shared_build.rs, build_place.rs, home_plot.rs, ipc.rs, gui/connections.rs, net_route.rs (a test) (2A); gui/pages/homes.rs, household.rs, pages/mod.rs, ui_snapshots.rs (2B); scripts/verify-copresence.js, Justfile (3A).
- **BUG-133:** every new data file with a built-in copy is read through `embedded_data::load_data_or_embedded` (or names its built-in read after the disk read).
- **Lints:** theme tokens only; no em dashes; account words; `page_registry_lint` (no new GuiPage here).
- **The ship hash changes** (new zones, plots, volumes): every game built before is refused by a relay running this ("a different ship") until both update, as after increments 3 and 4. Installed games keep their old data/blueprints/ship_structure.ron (BUG-163 part 2, question 6).
- **Rigs:** one GPU, one game at a time (machine guard).

## 2. Stale words this increment corrects

| Where | Says | Now |
|---|---|---|
| ship-homes doc increment 6, :1083 | Home Design at settings.rs:3585-3602 | settings.rs:3722-3740, Settings > Gameplay |
| ship-homes doc increment 6 Files, :1089-1092 | app/web/pages/settings-app.js; msg_handlers.rs | web/chat/chat-home.js (app/web is regenerated); the relay's change is in game_state.rs, storage/plots.rs and home_plots.rs, with one line in msg_handlers.rs |
| ship-homes doc increment 6, :1081 | "a single-storey cabin block plot" | a single-storey cabin block ZONE with sixteen cabin plots (3.3) |
| data/food/ship_stores.ron:9-11, ship_stores.rs:19-21 | "the game's own hunger does not read it yet ... increment 6" | a taken meal is eaten in the game (since the fleet ledger) and taken at the counter |
| zone_types.ron:212-215 | "its tables and serving line are increment 6's" | built |
| data/machines/ship.ron:17-22 | "The catalog is empty on purpose" | it defines every type its rows use (3.3) |
| showroom.rs:38-39 WHO_HINT | "Gear and skills are one save ... every server you visit" | each server has its own save that starts fresh; only your name and look travel |
| home_plot.rs:205 GUEST_ARRIVAL | "your home ... is put away ... comes back when you leave" | per start, naming the kinds that are free (3.7); the first words stay |
| docs/FEATURES.md:2836-2847, docs/design/game-modes.md:110 | `AppConfig.home_variant`, `draw_data_content` | the household size is per save, in Where you live |
| 5b plan 1.2 | "That is not built (increment 6)" | built |
| CLAUDE.md file map | no starts, no server saves | starts.ron, save slots |

## 3. The design

### 3.1 data/homes/starts.ron and src/ship/starts.rs
Read by the relay (`GameWorld::new`) and the game, disk first, built-in copy (`embedded_data::STARTS_RON`) through `load_data_or_embedded`.

```rust
// src/ship/starts.rs (compiled into the relay too: no native-only crates)
pub const STARTS_FILE: &str = "homes/starts.ron";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engagement { Direct, Trade, Automation, Cooperative, Background } // engagement-modes.md
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementDefaults { pub food: Engagement, pub water: Engagement, pub energy: Engagement, pub fertilizer: Engagement }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Household { pub id: String, pub name: String, pub machines: String /* under data/machines/ */ }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeContent { Herd, WildSpawns, Decorations, ShowcaseGarden, Hologram }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Start {
    pub id: String, pub name: String, pub sentence: String,
    pub offered: bool,
    #[serde(default)] pub not_offered_why: String,
    pub design: String,        // data/homes/shipped/<design>.ron; its `kind` must equal plot_kind
    pub plot_kind: String,
    pub households: Vec<Household>, // the first is the default
    pub engagement: EngagementDefaults, // shown in words; nothing else reads it yet
    pub shared: Vec<String>,   // zone_types.ron ids the start relies on aboard
    pub kit: Vec<(String, u32)>,
    pub storage: String,       // the home-storage seed under data/
    pub opening: Option<String>, // quest id accepted for a new character
    pub own_air: bool,         // false: the home is on the ship's ventilation
    pub home_content: Vec<HomeContent>, // homestead world content this home carries
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Starts { pub default: String, pub starts: Vec<Start> }
impl Starts {
    pub fn load(data_dir: &Path) -> Starts;               // built-in copy on any failure, logged
    pub fn get(&self, id: &str) -> Option<&Start>;
    pub fn default_start(&self) -> &Start;
    pub fn resolve(&self, asked: Option<&str>) -> &Start;  // unknown or not offered: the default
    pub fn for_plot_kind(&self, kind: &str) -> Option<&Start>; // the first offered row of that kind
    pub fn household(&self, start: &str, household: &str) -> &Household; // unknown: the first
    pub fn offered(&self) -> impl Iterator<Item = &Start>;
    pub fn validate(&self) -> Result<(), String>; // ids unique; default offered; households non-empty
}
```

The rows (exact):

| | cabin | apartment | homestead |
|---|---|---|---|
| name | Cabin | Apartment | Homestead |
| sentence | "One private room of your own. You eat free crew meals in the mess hall, the washroom is down the hall, and nothing at home can break or die." | "A small home in a block that shares a common house: dinners together, a big kitchen, and a garden you look after as a group." | "The full acre: a house, a greenhouse, a barn, fields and a workshop. You grow your own food and make your own power, and the mess hall is there if a week goes wrong." |
| offered | true | false, "Apartments come with the first apartment block, in a later update." | true (the default) |
| design, plot_kind | cabin, cabin | apartment, apartment | homestead, homestead |
| households | (solo, "Just you", cabin.ron) | (solo, apartment.ron), (family, apartment.ron) | (family, "A family of three", home.ron), (solo, "Just you", home_solo.ron) |
| engagement | Background x4 | food Cooperative, water Background, energy Background, fertilizer Cooperative | Direct x4 |
| shared | mess_hall, room_bathroom, recreation | apartment_block, mess_hall, recreation | (none) |
| kit | water_purified_0 x2, water_bottle_empty_0 x2, ors_sachet_0 x2 | (none) | data/world/player.ron's `starting_items`, MOVED here verbatim (player.ron loses the list) |
| storage | places/cabin_seed.json | (none) | places/seed.json |
| opening | gs_first_steps_cabin | None | gs_first_steps |
| own_air | false | false | true |
| home_content | (none) | (none) | Herd, WildSpawns, Decorations, ShowcaseGarden, Hologram |

Validation of design, plots, machines, kit, storage, opening and shared places applies to OFFERED rows only. The other kinds of 1.2b (Townhouse, House, Smallholding, Farm) are added as rows when each has a design and plots; no code.

Wire shapes (also in starts.rs, shared by relay, game and the web test):
```rust
pub const HOME_REQUEST: &str = "game_home_request";   // {type}
pub const CHOOSE_HOME: &str = "game_choose_home";     // {type, start}
pub const HOME_ANSWER: &str = "game_home";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeldHome { pub id: String, pub kind: String, pub start: String }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartOffer { pub id: String, pub name: String, pub sentence: String, pub offered: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub why_not: String, pub free: u32 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeRefusal { HoldsOne, Full, NotOffered, NoShip, Again }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeAnswer {
    pub plot: Option<HeldHome>, pub starts: Vec<StartOffer>,
    #[serde(default, skip_serializing_if = "is_false")] pub chose: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub refused: Option<HomeRefusal>,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub message: String,
}
pub fn refusal_words(r: HomeRefusal, asked: &Start, held: Option<&HeldHome>, free_names: &[&str]) -> String;
```
`message` sentences: HoldsOne "You already have a home on this ship: a {name}, {plot}. Moving to another kind of home comes in a later update."; Full "No {name} is free on this ship right now." plus "{names} are free." when any; NotOffered = the row's `not_offered_why`; NoShip "This server's ship did not load, so there is no home to choose."; Again "The server is busy; try again in a moment."

### 3.2 The Cabin
- **data/homes/shipped/cabin.ron** (fingerprinted with its folder; `SHIPPED_HOMES` gains `("cabin", HOME_CABIN_RON)`), opening with the `// THE SHIPPED DEFAULT` paragraph; **data/homes/cabin.ron** is the same design under its own header (`own_home_text`). Shape:
  - `kind: "cabin"`, `door: (0.0, 1.25)` (the west wall's middle: every cabin's hall is west of it);
  - body `width 3.0` (x), `depth 2.5` (z), `height 2.6`; steel shell and roof (an inner cabin has no skylight); no interior walls;
  - one ceiling light (the light types the Commons uses) at the middle;
  - `spawn: Some((1.0, 1.25))`, a metre inside the door;
  - one labelled volume (`room_bedroom`, label "Cabin", `room_type: Some("bedroom")`).
  - Sources in its header: the Nationally Described Space Standard's single bedroom (at least 7.5 m2 and at least 2.15 m wide) and its 2.3 m ceiling floor; Navy Times 2014-10-13 for shared berthing washrooms (the design's own citations).
- **data/machines/cabin.ron**: a catalog of exactly the types it and ship.ron use (each byte-equal to home.ron's, 3.3); instances inside the cabin: a single bed along the north wall (x 1.0..3.0, z 0..0.9), a nightstand, a wardrobe (the locker), a desk and chair on the south wall, a wall screen; no power rows, no loops; `ship_machines: Some("ship.ron")`, so a cabin player sees the Commons, the mess hall furniture and the trading posts. The single bed is a new type `bed_single` (0.9 x 2.0 m, `provides: Some("rest")` like `bed`), modelled from a single bed exported through docs/dev/adding-3d-models.md (question 5), never the double-bed model squeezed.
- **data/places/cabin_seed.json**: the "you" backpack node and a "home" node labelled "Cabin" holding a "Locker" (spare clothes, towel, toiletry kit, as labels).

### 3.3 The ship
All in data/blueprints/ship_structure.ron (header comments describe them), validated by `validate()`:
- **The block hall** `cabins-1`, label "Cabin Block A", purpose "residence": origin (77, 0, 88), body 3.0 x 48.0 x 3.0 (x 77..80, z 88..136), steel shell, GLASS roof, no lights of its own (1.8). Its body's volumes: `cabin-block-1` (type `cabin_block`, (0,0,0) 3 x 3 x 40, label "Cabin Block A", room_type `hall`) and `washroom-1` (type `room_bathroom`, (0,0,40) 3 x 3 x 8, label "Washroom", room_type `bathroom`), an interior wall across z 40 with a door. The washroom's fixtures wait (3.8, last bullet).
- **Its corridor** from `street-1` to `cabins-1`: an X run across x 75..77 at lat z 92.0, width 3.0, door 2.0 x 2.2, glass top.
- **Sixteen cabin plots** `c1`..`c16`, kind `cabin`, origin (80.5, 0, 88 + 2.5(i-1)), size (3.0, 3.0, 2.5); door to `cabins-1` at lat 89.25 + 2.5(i-1), width 1.2, door 0.9 x 2.0, no glass top. Each tube runs x 80..80.5 (0.5 m, above the 0.25 m minimum); neighbouring tubes are 2.5 m apart and 1.2 m wide, so they never overlap; the cabins touch each other's faces, which is allowed.
- **No parent plot.** The design's "block plot" is resolved as this zone: a plot holding the hall would overlap a zone (refused, :815-867), and a parent plot's own required door tube overlaps its cabins' tubes (tube against tube is never exempt, :850-853). Block membership is still "where your home is": a cabin's block is the zone its door opens on (`plot.door.zone`). `Plot::parent` stays for apartments.
- **The Commons' recreation corner:** a volume `recreation` (type `recreation`, room_type `commons`) about 10 x 8 m, placed by the data lane at least 3 m from the guest spawn (Commons-local (22.5, 47.5)), every crew chore spot and crew walk, and every corridor mouth; furniture two couches, a rug and a wall screen (ship.ron rows).
- **The mess hall's furniture** (ship.ron rows, zone `commons`): a `serving_counter` along the north wall at mess-hall-local x 0.4..2.8, z 0.3..0.9, so the store's spot (2, 2) is 1.1 m in front of it; a long table at x 27.6..28.6 facing the crew's meal seats; four `dining_table`s with `chair`s across the hall, each at least 0.5 m from every crew work spot and walk (test).
- **ship.ron's catalog** gains every type its rows use (tent_humidifier, tent_co2_fan, aquaponic_tank, composter, mushroom_rack, aeroponic_tower_apothecary, trading_post, dining_table, chair, couch, wall_screen, rug, and the new `serving_counter`: 2.4 x 0.9 x 0.6 m, Furniture, the counter model of question 5), byte-equal to home.ron's where both define one (test), so any household file can merge it. Its three `battery_7` power rows move to home.ron (the family home still powers the Commons, as modelled), and `split_for_save` keeps a connection with the household when either end is a household row, so a Dev save never moves them back; a cabin then merges ship.ron with nothing dangling.
- **The South Commons** (FIRST TO CUT): zone `commons-2`, label "The South Commons", purpose "commons", AFTER `commons` in the list (so `guest_spawn` still finds the first), origin (77, 0, 770), 24 x 40 x 4, glass roof; its corridor from `street-1` across x 75..77 at lat z 790; volumes `mess-hall-2` (type mess_hall, 24 x 3 x 8 at its north end) and nothing else; a counter, tables and chairs (ship.ron, zone `commons-2`); a second store row `mess-hall-2-stores` in data/food/ship_stores.ron at `mess-hall-2` (2, 2). From p12's door (lat z 1129) it is about 350 m, 4.2 minutes at 1.4 m/s; from p5's (lat z 436) about 364 m. The design's rule 2.9 becomes a test (3.3 tests).
- data/ships/room_equipment.ron gains rows for room types `hall`, `bathroom` and `recreation`'s commons (AI perception only).
- The rigs' door-points fixture (scripts/tests/fixtures/door-points-p1.json) is regenerated by `write_the_rigs_fixture`.

### 3.4 Zone types
data/blueprints/zone_types.ron, after `mess_hall` (labels are what the picker shows):
- `recreation` "Recreation room", "Games, music and shared screens for watching together; where neighbours spend an evening.", default_size (10, 3, 8);
- `cabin_block` "Cabin block", "A row of private cabins off one hall, with a shared washroom; up to 40 cabins.", (3, 3, 40);
- `apartment_block` "Apartment block", "Small homes around a common house with a mess hall, a big kitchen and a garden.", (40, 6, 40);
- `street` "Street", "A walkway between places; people pass each other here.", (10, 4, 100).
None gets a zone_filler.ron entry (they hold real furniture or nothing).

### 3.5 The relay
- **`JoinHome`** (game_state.rs :443-447) gains `start: Option<String>`; `from_join` reads `"start"` when it is a string of at most 32 characters of [a-z0-9_-], else None.
- **`GameWorld::starts: Starts`**, loaded in `new` (:509-537) beside the ship; `load_ship` logs each offered start's plot count (a start whose kind has no plot is logged and never claimable).
- **`assign_home`** (:1083-1110): `let start = self.starts.resolve(join.start.as_deref())`; `all` = every plot id, `kind_ids` = the plots of `start.plot_kind` in file order; `claim_plot(world, owner, &all, &kind_ids)`. `HomeAssignment` gains `start: String` (the start of the plot held, from `for_plot_kind`) and `full: Option<String>` (the asked start when none of its kind was free and the player held none). `home_plot_json` adds `"start"`. A new `add_to_welcome(&self, welcome: &mut Value)` adds, for a guest only, `"home_full": <start id>` and `"home_free": {<start id>: <count>}` for every offered start. msg_handlers.rs gains exactly one line after the welcome is built (:3295): `home.add_to_welcome(&mut welcome);`.
- **`Storage::claim_plot(world, owner, all, free_order)`** (plots.rs:50): keep the held plot when it is in `all` (so asking for a cabin never drops a held homestead), else claim the first free of `free_order`. NEW `held_by(world, owner) -> Option<String>` and `free_of(world, ids) -> u32`. Callers updated: shared_build_tests.rs:34, account.rs tests :577-592.
- **`game_home_request` / `game_choose_home`** in home_plots.rs (`handle_home(state, key, kind, raw)`), routed by one relay.rs arm (+4 lines, beside :3745): `Some(kind @ ("game_home_request" | "game_choose_home")) => { home_plots::handle_home(...).await; continue; }`. Cheapest first:
  1. rate: `perception_rate_allows(state, key, "home")`, else `refused: again`;
  2. no ship loaded: `no_ship`;
  3. request: answer with the asker's own plot (`held_by`), and `starts` with `free` per offered row; never anyone else's;
  4. choose: unknown or not offered start: `not_offered`; holds one: `holds_one`; none of that kind free: `full`; else claim from that kind's ids and answer `chose: true` with the plot.
  - The answer goes to the asker alone (`send_game_private`). Log line, never a key: `Game: home chosen (cabin)` or `(refused/<why>)`. Nothing new is stored: the claim is the existing `game_plots` row.
  - A guest in the world who chooses gets `chose: true`; its game steps out and joins again so the welcome gives the plot (3.7).
- **The walker** (scripts/second-player.js): `--start <id>` puts `start` in the join; stdin `home` sends `game_home_request` and logs `home: <plot|none> <kind>; free <id> <n>, ...`; stdin `choose-home <start>`; stdin `take-meal` sends `game_interact` `take_meal` to the nearest `food_store` it knows within 5 m and logs `meal taken: <item>` or `meal refused: <error>`; it logs the welcome's `home_plot.kind` and `start`.

### 3.6 Saves: one save per world
- **`SaveSlot`** (persistence.rs): `Offline` | `Server { server: String /* normalized URL */, identity: String /* public key hex */ }`. `slot_path`: Offline is saves/offline_home.json; Server is saves/servers/<16 hex of BLAKE3(server)>-<first 16 hex of the identity>.json (no address in a file name). Picker scans stay top-level, so server saves never appear as WHO characters.
- **`WorldSave`** (persistence.rs :15-208) changes, all `#[serde(default)]` (the operator's own save from before loads as a homestead, family, no place):
  ```rust
  // replaces `design: String` (never read but by the picker's detail row)
  #[serde(default = "default_start")] pub start: String,          // "homestead"
  #[serde(default = "default_household")] pub household: String,  // "family"
  #[serde(default, skip_serializing_if = "String::is_empty")] pub server: String, // a server save's normalized URL
  #[serde(default, skip_serializing_if = "Option::is_none")] pub ship_place: Option<ShipPlace>,
  #[serde(default)] pub ship_meals: ShipMeals,
  pub struct ShipPlace { pub ship_id: String, pub ship_hash: String, pub plot: String }
  pub struct ShipMeals { pub taken: u32, pub next_at_game_s: f64, pub water_l: f64 } // offline counter meter
  ```
  `kind` is now "offline" or "server". An install whose Settings said Solo comes back as Family once (no shim; Where you live sets it again).
- **`save_load::ActiveHome`** (thread-local, the `RESTORED_SAVE_WAITING` pattern, save_load.rs:850-866): `{ slot, path, start, household, server, ship_place }`. `active_home_path()` returns its path; `save_home_at` writes its start, household, server, ship_place and the DataStore's `ShipMeals` into the save; `machines::home_ron_path` returns `data/machines/<starts.household(active.start, active.household).machines>` (all 8 call sites unchanged); `starting_kit(data_dir, start)` reads the start's kit.
- **Which slot is live** (`save_slot::wanted(gui)`, pure): Offline when `launcher_last_world` starts with "home:" or there is no identity; else `Server { active_server_key(gui), profile_public_key }`. It follows the pairing only: Dev travel, fly mode and the `solo` verb flip `copresence_solo` and never the slot. The boot rule is the same one `copresence_solo` already uses (config.rs:1979), so an empty pairing (a rig's fresh folder) is a server slot.
- **A fresh server save** (`save_slot::fresh_save(data_dir, starts, start, household, look)`, pure): built by spawning the fresh player (`save_load::spawn_fresh_player(world, data_dir, start)`: the start's kit and its opening accepted) and the fresh asteroids (`save_load::fresh_asteroids`, from NEW data/asteroids/belt.ron, the three rows moved verbatim from lib.rs:1418-1463) into a scratch world, extracting it, and setting every field explicitly: `kind "server"`, `credits` = `Wallet::default().credits`, `quests: Some(..)`, `asteroids: Some(..)`, `placed_items: Some(pool of the start's storage seed)`, `body: None` (a fresh body), no own home, no crops, vehicles, drone, packs or settled trades, empty `home_id`; then `character_name`, `appearance` and `outfit` from `look` (the live character). Applying one also resets every machine's stored level (`machine_levels::reset_all`), since an empty list keeps them.
- **Swapping** (`save_slot::follow(state)`, every frame after `home_plot::follow_server`): when `wanted != active` and the world is loaded, no restore is waiting and the build editor is shut:
  1. capture the live look; write the active slot (offline: `keep_progress = !fresh_world_each_launch`; a server slot always keeps progress);
  2. read the target slot's file, or write a `fresh_save` there (start = the picker's choice, else the default);
  3. apply it onto the running world through `save_load::apply_onto_running_world` (the lib.rs:14562-14620 sequence lifted into save_load.rs, which the picker and Restore use too): backpack, skills, credits, quests, body, vehicles, crops, builds, asteroids, drone, machine levels, `ShipSupplyLedger`, `ShipMeals`, `resume_home`, craft batches dropped, `fleet_recheck` set;
  4. set `ActiveHome` and the places/storage pool of the save's start; re-apply the captured look (the look crosses at every entry);
  5. build the home (3.7).
  - The join gate gains `slot_ready` (`JoinGate`, home_plot.rs:259-276): no join until the slot matches, so a server switch never joins with the previous server's start.
- **At boot** (lib.rs before :1417): read the config early (`AppConfig::load_if_exists`), compute the slot, load that file or write a fresh one, set `ActiveHome`, then spawn with the start's kit and opening, load the machines (:1528) from the start's household. `fresh_world_each_launch` applies to the offline slot only (Settings' hint says so).
- **An erased account** (gui/connections.rs :317, :334, which forget `home_plots` today): the server save is renamed aside (`<name>.erased-<yyyy-mm-dd>.json`, never deleted), so a new sign-up there starts fresh.
- **AppConfig** loses `home_variant` and `home_plots` (and `RememberedPlot`); GuiState loses `home_plots` and `settings.home_variant`; `remember_plot` becomes `save_slot::set_ship_place`.

### 3.7 The game's home
- **One function builds it:** `save_slot::home_for(ship_file, starts, start, household, ship_place, own, slot) -> Result<ShipStructure, String>` (pure): the plot is `ship_place.plot` when its `ship_hash` is this ship's and the plot exists with the start's kind, else `ShipStructure::default_plot_of_kind(kind)` (the default plot when it is of that kind, else the first plot of that kind; NEW in ship_structure.rs). The design is: the save's own home when its kind matches and the mode is not Dev (`own_home::in_effect`); else in Dev the data file (`HomeDesign::load`, Dev authors it); else for a SERVER slot the shipped design (`HomeDesign::built_in`), never data/homes/<kind>.ron, which an offline Dev edit can change; else (offline) `HomeDesign::load`. The world load (`assemble_for_boot` :703-719), the swap and a welcome of another kind all use it.
- **`add_join_fields`** (:488-495) adds `"start": active.start`.
- **`plan_welcome`** (:333-376): when the welcome's `home_plot.kind` differs from the home's kind, plan a `Move` built by `home_for` with that kind's start (`starts.for_plot_kind`) and the save's household reset to that start's first; `apply_welcome_home` then reloads `home_machines` for the new start before `move_home`, and records `start`/`household`/`ship_place` in `ActiveHome`. A kind mismatch NEVER gives the plot up; only a kind with no design at all does (today's `cannot_place_sentence`). Every applied Move/Stay records `ship_place`; a guest clears it; a refusal with `give_up_plot` clears it.
- **The guest sentence** (`guest_arrival(welcome, starts) -> String`) keeps `GUEST_ARRIVAL`'s first words "This server has no home plot free for you" and goes on: "no {asked} is free on its ship, so you are a guest for now: you cannot build, and the ship's shared places are open to you." plus, when `home_free` lists any, "{Names} are free: choose one on this server's card in Characters (Play)."
- **`game_home` in the game** (`save_slot::on_game_message`, hooked in net_route.rs `route_game_message` beside :15-21): stores the answer for the picker and Settings; on `chose: true` sets `ship_place` and, while joined as a guest, steps out and joins again (`respawn_through_relay`) so the welcome gives the plot.
- **What a cabin home spawns** (world_load.rs): the herd, wild spawns, decoration plants, showcase garden auto-seed and hologram each spawn only when the start's `home_content` lists them (the cabin lists none); the showroom stage stays.
- **Air:** a home whose start has `own_air: false` is no `HomeAir` space (ship_space.rs): it breathes the ship's air, as a berth on the ship's ventilation does.

### 3.8 The mess hall serves
- **The counter's card.** lib.rs beside :14086-14100 sets `GuiState::machine_card_meal` from `engine::mess_hall::card_offers_meal(machines, selected)` (true for a `serving_counter` row). hud.rs (beside :1813-1823) draws, under the card:
  - [Take a meal] with its line: in a server's world "Free crew meals, one a meal time; each is a line in your fleet ledger (Inventory > The fleet)."; elsewhere "Free crew meals from the ship's stores, one a meal time, counted on this computer."
  - [Fill your bottles] with "Drinking water from the ship, free." and the count it filled.
- **Take a meal** (`engine::mess_hall::take_meal(state) -> String`):
  - in the shared world: the nearest store within `STORE_REACH_M` gets `game_interact take_meal` through the fleet panel's own send, lifted into `fleet_ledger::take_meal_at_nearest` so the panel and the card share it; the answer is eaten by `fleet::on_meal` as today;
  - offline or out of the shared world: `systems::ship_meals::serve(meter, now_game_s, rules)`: when `now >= next_at_game_s`, one meal good (data/ship/fleet_ledger.ron's meal kind, `OfGood("ration_basic_0")`) goes into the backpack and is eaten through `fleet::feed`, `taken += 1`, `next_at = now + meal_interval_hours * 3600` (read from data/food/ship_stores.ron); else "Your next crew meal is ready in {h} h {m} min.".
- **Fill your bottles** fills every empty fillable vessel in the backpack (`fluids` `fillables`) from the ship's water, adds the litres to `ShipMeals.water_l`, and works the same online and offline. The fleet ledger gains no water line in this increment (logged follow-up).
- **Stores coming into view:** `fleet::on_game_message` adds a `food_store` from `game_in_view` (needed for the South Commons, which is out of most welcomes' view).
- **Inventory > The fleet offline** shows "On this computer the ship's stores have fed you {n} meals and {l} L of water."
- **Not in this increment:** the washroom's fixtures (toilet, shower and basin rows would count on the merging household's water budget: they wait for ship machines leaving household meters, the 1a follow-up); laundry and workshops; a meal timetable (question 4).

### 3.9 Choose where you live
One widget, NEW src/gui/pages/where_you_live.rs, theme tokens only (`text_primary`, `text_secondary`, `text_muted`, `accent`, `success`, `warning`), its draft in a `thread_local!`:
- **Title** "Choose where you live". **Hint** (muted): "Where you live sets how much space you hold and how much looking after it needs. It never changes how deep the world runs or how hard it is: that is your play mode."
- **One row per start** from starts.ron: the name (the default reads "Homestead (most people start here)"), its sentence, "Shared: {zone type labels}" (for example "Shared: the mess hall, a bathroom down the hall, the recreation room"), and one plain line from its engagement (food Background "Food: free crew meals in the mess hall."; Direct "Food: you grow it."; Cooperative "Food: shared dinners with your block."; water/energy Background "Water and power: the ship's, counted on your meter."; Direct "Water and power: you collect and make them."). A row not offered is greyed with its `not_offered_why`. A start with two households shows them as a sub-choice ("A family of three" / "Just you").
- **Never written:** tier, rank, level, berth.
- **In Characters (Play)** (showroom.rs):
  - the new-home row `NEW_HOMESTEAD` becomes "A new home"; selected, it shows the widget; Enter makes the offline save with that start and household; the Enter bar reads "Enter as {who} -> a new Cabin (solo)";
  - a server's card, when it is the connected server, shows "Where you live here: a Cabin, c2." from `game_home`, or "You have no home on this server yet." with the widget, each row's free count ("12 free", "none free") and [Choose] (sends `game_choose_home`, then writes the server save's start and `ship_place`); the note (muted): "On a server you start fresh: only your name and look come with you. Your home there is kept on this computer.";
  - WHO_HINT becomes "Your name and look travel with you. Each server you visit keeps its own home and things for you, starting fresh; your own home keeps yours."
- **Settings > Gameplay** (replacing settings.rs:3722-3740), "Where you live": "In this world you live in a {start} ({household})." with the household sub-choice for the live save (it changes the save's household; "Takes effect the next time you enter the world"), and "To live somewhere else, start a new home in Characters (Play), or choose on a server's card there. Moving to another kind of home on a server comes in a later update."
- **The Home page** (homes.rs `draw` :207, under 5b's Household card): the same read-only line, and the homestead loops section only when the home has loops.
- **The website** (NEW web/chat/chat-home.js, the chat-fleet.js pattern): command palette "Where you live" (data/commands.json `openWhereYouLive`, chat-ui.js action, index.html container); sends `game_home_request` on the signed-in socket; app.js hands `game_home` to `renderHome`. It shows the same facts and a [Choose] per offered start (sends `game_choose_home`). Every start name, sentence and refusal sentence it shows comes from the relay's answer (no copies). It shows nothing of anyone else.

### 3.10 The Cabin's opening
- data/quests/getting_started.ron gains `gs_first_steps_cabin`, "Your first minutes aboard: check on yourself, find the mess hall and eat, see where people meet, then head home." Steps (each at most 64 characters, the HUD's rule):
  1. "Check your vitals: press I, see Inventory > Status" (View vitals);
  2. "Walk to the mess hall, in the Commons" (Travel mess_hall);
  3. "Take a meal: E at the serving counter, then Take a meal" (Eat 1);
  4. "Visit the recreation room in the Commons" (Travel recreation);
  5. "Head home to your cabin door on {front_door_opens_on}" (Travel front_door).
  Reward: one `ration_basic_0`. No prerequisite, no follow-on quest.
- data/entities/destinations.ron gains `mess_hall` and `recreation` with `at: Some(NearestVolume(zone_type: "mess_hall"))` / `"recreation"`, radius 4.0; quests/mod.rs's `DestinationAnchor` gains `NearestVolume { zone_type: String }` (it stops being `Copy`), resolved to the centre of the nearest labelled volume of that type on the live ship (published like `FrontDoor`, `publish_places`).
- lib.rs:1383's hard-coded `gs_first_steps` becomes the boot start's `opening` (`spawn_fresh_player`).

### 3.11 Nobody is told who lives where
- `game_home` answers only about the asker's own plot; the free counts are counts. `/api/server-info` gains nothing. A guest welcome's `home_free` is the same counts.
- Server saves live on the device; the relay keeps nothing new.

## 4. Questions only the operator can answer (each with a recommendation)
1. **Which homes the first choice offers, and offline too.** Recommend: Cabin and Homestead now, offline and on servers, with the Homestead marked as where most people start; Apartment shown with "comes with the first apartment block"; the other kinds of 1.2b appear as starts.ron rows as each gets a design and plots. Onboarding stays as it is (a new player starts in the default Homestead, alone); the choice is in Characters.
2. **Moving house in this increment.** Recommend: not for players yet. A player chooses once per server (and per new home offline); an admin's release (Server Settings > ADMIN > Homes on the ship) frees a home for someone who wants another kind; Moving house, with its cooldown, arrives with shipments (increment 9), so a move carries your things instead of taking down what the server keeps on your plot.
3. **A server home lives on the computer that played it.** Two computers give one person two separate characters on one server until the server holds inventories (increment 8), and a modified app could still bring things in. Recommend: accept for this increment and say so on the server card (3.9); the server-held character (characters-and-servers.md, "closed") is the real fix.
4. **Meals any time, or at meal times?** Recommend: any time, one a meal time per person (8 game hours, as built), so a short session never goes hungry; hot meals at set times come with cooking and the block's rota (increment 10).
5. **A download for the cabin's bed and the counter.** The repo has a double bed and a kitchen cabinet only. Recommend: export a single bed and a bar counter from the CC0 Kenney Furniture Kit already credited in assets/models/LICENSE-cc0-model-packs.md; if assets/models/furniture.blend does not hold them, fetching the kit from kenney.nl is your call.
6. **Already asked 2026-10-05, now blocking players: BUG-163 part 2.** An installed game keeps its old ship file, so after this update every install extracted before it is refused by an updated server as "a different ship". Recommend: on update, refresh data files the player has not edited (the ship file, shipped designs, starts.ron, machine files), comparing with the copy the previous version shipped.

## 5. Waves

**Ownership.** Each file belongs to exactly one lane per wave; a file edited by two waves is edited in sequence:
- persistence.rs, starts.rs's types, embedded_data.rs (Wave 0, then 1C), save_load.rs (0, then 1C), machines.rs (0, then 1A), ship_structure.rs (0 `default_plot_of_kind`, then 1A tests), gui/mod.rs (0, then 2A), lib.rs (0 one line, 1D, then 2A), showroom.rs (0 one line, then 2B), save_slot.rs (0 stubs, 1C, 2A), ship_space.rs (1A tests, then 2A).
- Nobody: frame_ws_poll.rs, state_types.rs, msg_handlers.rs beyond 1B's one line, relay.rs beyond 1B's arm.
- **Start conditions:** Wave 0 now. Wave 1 after 5b 2B merges (pages/mod.rs, ui_snapshots.rs, homes.rs). Wave 2 after 5b 2A and 3A merge (home_plot.rs, ipc.rs, connections.rs, net_route.rs). Wave 3 after Wave 2.

**Budgets after all waves (estimates):**

| File | Lines | Budget |
|---|---|---|
| lib.rs | about 16,390 (+6 card, +7 boot and follow, -57 apply lifted, -44 asteroids to data) | 16,700 |
| gui/mod.rs | about 4,679 (+8 slot, answer, card; -12 home_plots and home_variant) | 4,800 |
| relay.rs | 6,326 (+4) | 6,400 |
| msg_handlers.rs | 4,654 (+1) | 4,660 |
| frame_ws_poll.rs | 1,698 (+0) | 1,800 |

**Rule for every test below:** it is red on the code before its lane with the named break, and its comment records the failure text.

### Wave 0: the contract (one builder, merges first). Player-visible: nothing yet; the game and the server agree on what a Cabin and a Homestead are.
Files:
- NEW src/ship/starts.rs and src/ship/mod.rs (3.1, wire shapes and words); NEW data/homes/starts.ron.
- NEW data/homes/shipped/cabin.ron, data/homes/cabin.ron, data/machines/cabin.ron, data/places/cabin_seed.json (3.2).
- src/embedded_data.rs: `STARTS_RON`, `HOME_CABIN_RON` and its `SHIPPED_HOMES` row, `MACHINES_CABIN_RON`, `PLACES_CABIN_SEED_JSON`, `get_embedded` arms (with `own_home_text` for homes/cabin.ron), the EMBEDDED list.
- src/persistence.rs: the fields of 3.6, `ShipPlace`, `ShipMeals`, `SaveSlot`, `slot_path`; `new_offline` defaults.
- src/save_load.rs: `ActiveHome` thread-local with `active_home()`/`set_active_home()`; `active_home_path` reads it; `starting_kit(data_dir, start)`; `SHIP_MEALS_KEY`.
- src/machines.rs: `home_ron_path` from `ActiveHome` and starts.ron.
- data/world/player.ron: `starting_items` removed (now the homestead row's kit); its readers (save_load test, tests/recipe_sources_lint.rs, quests' `every_shipped_home_has_what_the_starter_quests_need`) read the kit.
- src/ship/ship_structure.rs: `default_plot_of_kind`.
- src/gui/mod.rs: `save_slot: SaveSlot`, `home_answer: Option<HomeAnswer>`, `machine_card_meal: bool` and defaults.
- NEW src/engine/save_slot.rs with the signatures 1C and 2A fill (`wanted`, `fresh_save`, `follow`, `home_for`, `on_game_message`, `enter_pairing`, `choose_on_server`), as no-op stubs; src/engine/mod.rs.
- src/gui/pages/showroom.rs one line (`load_homes` reads `start` for the detail).

Tests:
- starts.rs `the_starts_are_data_and_none_is_called_a_tier`: three rows, ids unique, exactly the default offered, every offered row's design built in with `kind == plot_kind`, every household file loads, every kit item is in items.csv, every opening is a quest, no row's words hold "tier", "rank" or "level". Red with apartment flipped to offered: "the apartment start is offered but no apartment design is built in".
- starts.rs `the_built_in_starts_are_the_file`: disk and built-in parse equal. Red with one sentence changed in the disk copy.
- starts.rs `an_unknown_or_unoffered_start_is_the_default`. Red with `resolve` returning an unoffered row.
- starts.rs `the_wire_reads_as_written`: exact `game_home` JSON; `chose`/`refused`/`message` absent when empty. Red with `free` renamed.
- ship_structure.rs `a_cabin_is_seven_and_a_half_square_metres`: the shipped cabin's floor is 7.5 m2, at least 2.15 m wide, at least 2.3 m high, its door on the west wall's middle. Red with depth 2.0.
- persistence.rs `a_save_from_before_this_increment_loads_as_a_homestead_with_no_place`: a fixture written by today's code (NEW tests/fixtures/saves/offline_home_v1487.json, written by the current exe and not edited) loads with start homestead, household family, no place. Red with `start` lacking its default.
- persistence.rs `server_saves_are_one_file_per_server_and_identity`: two servers and two identities give four paths; no path holds the address. Red with the identity left out.
- machines.rs `the_household_file_comes_from_the_start`: homestead/family is home.ron, homestead/solo home_solo.ron, cabin/solo cabin.ron. Red with `home_ron_path` still reading AppConfig.
- `just validate-data` passes.

### Wave 1 (seven lanes in parallel)

**1A The ship and its furniture (data).** Player-visible: a row of sixteen cabins with its hall and washroom stands across First Street, the mess hall has tables and a serving counter, the Commons a corner with couches and a screen, and a second mess hall halfway down the street.
Files: data/blueprints/ship_structure.ron, zone_types.ron; data/machines/ship.ron, home.ron (the three power rows); data/food/ship_stores.ron (second store; stale comment); data/ships/room_equipment.ron; src/machines.rs (`split_for_save`, tests); scripts/tests/fixtures/door-points-p1.json; tests in src/ship/ship_structure.rs, neighbours.rs, door_points.rs, ship_space.rs, src/ship/starts.rs, src/relay/handlers/ship_world.rs, game_interest.rs; tests/visual/vantages.json (`ship-cabin-row`, `mess-hall-counter`).
Tests:
- ship_structure.rs `the_shipped_ship_has_a_cabin_block_beside_first_street`: c1..c16 each take the shipped cabin through their own door into `cabins-1`, the hall is reached from First Street, `validate` passes, the built-in copy is the same ship; the twelve-homes test filters to homesteads. Red with c7's door lat 1 m off: "plot 'c7' door lat ... does not meet the cabin design's door".
- ship_structure.rs `every_home_reaches_a_mess_hall_within_five_minutes` (design 2.9): along the corridor graph from each plot's door to the nearest `mess_hall` volume at 1.4 m/s. Red on the ship without the South Commons: "p8's door is 9.6 minutes from the nearest mess hall".
- starts.rs `each_offered_start_has_plots_and_its_places_aboard`. Red with the cabin plots' kind spelt "cabins".
- machines.rs `a_type_two_machine_files_define_is_defined_the_same` (home.ron, home_solo.ron, cabin.ron, ship.ron). Red with one chair's size changed in ship.ron.
- machines.rs `a_household_wire_to_a_ship_row_stays_with_the_household`: a Dev save of the family layout writes the `battery_7` rows to home.ron; a cabin layout has no connection naming a machine it lacks. Red with today's `split_for_save`: "battery_7 -> mushhum_c1 went into ship.ron".
- machines.rs `every_fleet_store_has_its_serving_counter`: each store in ship_stores.ron has a `serving_counter` row within 1.5 m. Red with the counter moved 3 m.
- ship_world.rs `every_crew_walk_is_clear_of_the_ships_furniture` (crew walks against every ship.ron Furniture box, 0.5 m). Red with a table on the walk to (11, 4).
- neighbours.rs `a_cabin_neighbour_is_drawn_with_its_door`: one shell baked for sixteen cabins. Red with the cabin left out of `SHIPPED_HOMES`.
- ship_space.rs `every_plot_is_aboard_and_breathes_its_own_air` assembles each plot's own kind. Red unchanged (homestead on c1 is refused).
- The existing neighbour, door, collision, view and room tests stay green with new counts, each change explained in its comment.
- Checks: `just validate-data`; the two vantages captured with probe-sweep and looked at against the NDSS figures and a real berthing photograph before the lane is called done.

**1B The relay and the walker.** Player-visible: the server gives each player the kind of home they ask for and can tell them where they live.
Files: src/relay/handlers/game_state.rs, src/relay/storage/plots.rs, src/relay/storage/account.rs (tests), src/relay/handlers/shared_build_tests.rs (call site), src/relay/handlers/home_plots.rs, src/relay/handlers/msg_handlers.rs (one line), src/relay/relay.rs (the arm), src/relay/features.rs, scripts/second-player.js, scripts/tests/second-player.test.js.
Tests:
- plots.rs `a_held_plot_is_kept_whatever_kind_is_asked`. Red with the kind ids passed as the only list: "Ann's homestead p2 was given up when she asked for a cabin".
- plots.rs `free_counts_count_only_plots_nobody_holds`. Red with a held plot counted.
- game_state.rs `a_join_asking_for_a_cabin_gets_a_cabin_and_an_unknown_start_the_default`. Red with `start` ignored.
- game_state.rs `a_full_kind_makes_a_guest_and_says_what_is_free` (`home_full`, `home_free` on a guest only). Red without `add_to_welcome`.
- home_plots.rs `choosing_a_home_claims_one_only_when_you_hold_none`: none -> claimed; holds one -> holds_one; full -> full; apartment -> not_offered; twice in 200 ms -> again; no ship -> no_ship. Red with the claim allowed while holding one.
- features.rs `a_cabin_and_a_homestead_on_one_ship` (real relay): Ann joins asking cabin and gets c1; Bo joins with no start and gets p1; Cy chooses a cabin before joining (c2) and her join gets c2; Dee's `game_home` names only her own (none) and the counts; no `relay.log` line added here carries a key. Red with `assign_home` ignoring `start`: "Ann was given p1, a homestead, asking for a cabin".
- features.rs `a_guest_chooses_a_free_cabin_and_the_next_welcome_gives_it` (twelve homesteads held). Red with choose refused for a player in the world.
- second-player.test.js: `--start`, `home`, `choose-home`, `take-meal` parse and the join carries `start`. Red before each exists.
- Checks: `just verify-relay`; `cargo check --features relay --no-default-features`; `just rig-tests`.

**1C Save slots (the core).** Player-visible: nothing new yet; the game can keep a separate save for each server, made fresh.
Files: src/engine/save_slot.rs (`wanted`, `fresh_save`, the swap's save half: `write_active`, `take_up`), src/save_load.rs (`apply_onto_running_world`, `spawn_fresh_player`, `fresh_asteroids`, the apply's kind gate admits "server", Restore reloads the active slot by path, `ShipMeals` round trip), NEW data/asteroids/belt.ron with its built-in copy in src/embedded_data.rs, src/systems/machine_levels.rs (`reset_all`).
Tests:
- `a_fresh_server_save_carries_only_the_look`: from an offline world with a full backpack, skills, 50,000 CR, an own home, quests done, crops, a vehicle, a drone and a charged battery, the fresh save holds the cabin kit only, no skills, the default credits, no own home, the cabin opening active, no crops, vehicles or drone, the fresh asteroids, a fresh body, and the same name, look and outfit. Red with the inventory copied: "the offline backpack's 7 gold ingots came into the server's save".
- `the_slot_follows_the_pairing_not_a_step_out`: home:, server:, empty, no identity; a server pairing with `copresence_solo` set by Dev travel stays the server slot. Red with `wanted` reading `copresence_solo`.
- `a_server_save_applies`. Red with the "offline only" early return kept.
- `a_fresh_save_resets_every_machines_level`. Red without `reset_all`: "the offline battery's 9.0 kWh came into the fresh home".
- `the_active_save_path_follows_the_slot` and `a_restore_reloads_the_active_slot`. Red with the fixed offline path.
- `the_asteroid_belt_is_data_and_the_same_as_before`: m12, s7, c3 with their ores. Red with one ore changed.
- `the_ship_meals_meter_round_trips`. Red with it not written.

**1D The mess hall serves (game).** Player-visible: pressing E at the mess hall's counter gives a free crew meal and fills your bottles, offline too.
Files: NEW src/engine/mess_hall.rs, NEW src/systems/ship_meals.rs (+ mod lines), src/gui/pages/hud.rs, src/gui/pages/fleet_ledger.rs, src/engine/fleet.rs, src/lib.rs (the card flag only).
Tests:
- ship_meals.rs `the_counter_serves_a_meal_offline_once_a_meal_time`: eaten, food up, taken 1; again: not yet, with minutes; after the interval: served. Red with no interval check.
- mess_hall.rs `in_the_shared_world_the_counter_asks_the_server`: one `game_interact take_meal` for the nearest store in reach; none in reach gives the walk-closer sentence; no local meal while joined. Red with the offline path taken while joined ("a meal not on the ledger").
- mess_hall.rs `filling_bottles_at_the_counter`. Red with nothing filled.
- mess_hall.rs `only_a_serving_counter_offers_a_meal`. Red with every Furniture card offering one.
- fleet.rs `a_store_coming_into_view_is_listed`. Red with `game_in_view` ignored.
- hud.rs `the_counter_card_in_words`. Red with the ledger line removed.

**1E The widget, Settings and the Home page.** Player-visible: Settings and the Home page say where you live instead of "Home Design".
Files: NEW src/gui/pages/where_you_live.rs and where_you_live_tests.rs, src/gui/pages/mod.rs, src/gui/pages/settings.rs (:3722-3740), src/gui/pages/homes.rs, src/gui/ui_snapshots.rs, tests/snapshots/where_you_live.png.
Tests:
- `where_you_live_in_words`: every row and line of 3.9 for an offline homestead, a server cabin and a guest; Apartment greyed with its words. Red with "Home Design" kept as the title.
- `the_household_choice_is_the_saves`: changing it sets `ActiveHome.household` and never AppConfig. Red with the AppConfig write kept.
- `no_tier_word_on_the_choice`. Red with "tier" added.
- The snapshot, rendered with `just snapshot where_you_live` and looked at.

**1F The website.** Player-visible: the website's chat shows where you live on its server and lets you choose a home there.
Files: NEW web/chat/chat-home.js, web/chat/index.html, web/chat/chat-ui.js, web/chat/app.js, data/commands.json, NEW scripts/tests/chat-home.test.js.
Tests: renders a held home, the choices with free counts, and each refusal from synthetic answers; [Choose] sends `game_choose_home` with the start id; every start name and sentence shown is the answer's (a test answer with renamed starts shows the new names). Red with a hard-coded "Cabin".

**1G The Cabin's opening.** Player-visible: a Cabin player's first minutes say: check yourself, find the mess hall and eat, see the recreation room, go home.
Files: data/quests/getting_started.ron, data/entities/destinations.ron, src/systems/quests/mod.rs, src/systems/quests/opening_tests.rs.
Tests:
- `the_cabin_opening_walks_on_this_data`: the five steps with the real systems on the shipped ship, standing in the mess hall, eating, the recreation room, the cabin door. Red with `NearestVolume` unresolved.
- `the_mess_hall_is_the_nearest_one`: from c1 the Commons' mess hall, from p11 the South Commons'. Red with the first volume taken.
- The existing `every_step_*` tests cover the new lines (64 characters, places named).

### Wave 2 (two lanes, after Wave 1 and 5b's 2A and 3A merge)

**2A The session and the home (glue).** Player-visible: on a server you start fresh in the home you chose, and your own home and things are waiting when you go back to it.
Files: src/lib.rs (boot slot before :1417; the fresh spawn and asteroids through save_load; the opening by start; `save_slot::follow` after `follow_server`; the picker's Enter makes no `launcher_pending_load` for a server pairing; :14562-14620 replaced by `apply_onto_running_world`), src/gui/mod.rs (drop `home_plots`, `settings.home_variant`), src/config.rs (drop `home_variant`, `home_plots`, `RememberedPlot`), src/gui/connections.rs (erase sets the server save aside), src/engine/save_slot.rs (`follow`, `home_for`, `on_game_message`, `enter_pairing`, `choose_on_server`), src/engine/net_route.rs (the hook), src/engine/home_plot.rs (`add_join_fields`, `plan_welcome`, `apply_welcome_home`, `boot_plot`, `assemble_for_boot`, `remember_plot` gone, `bring_home_back`, `JoinGate::slot_ready`, `guest_arrival`), src/engine/world_load.rs (home content by start), src/engine/own_home.rs (server slots), src/ship/ship_space.rs and src/engine/survival_env.rs (`own_air`), src/gui/pages/main_menu.rs tests.
Tests:
- home_plot.rs `a_welcome_of_another_kind_builds_that_kinds_home_and_keeps_the_plot`. Red on today's code: "gave up plot c2: plot 'c2' takes a cabin home, not a homestead".
- home_plot.rs `the_join_names_the_saves_start`. Red without the field.
- save_slot.rs `a_server_home_is_the_shipped_design_never_the_disk_file` (Normal mode, data/homes/homestead.ron with an extra wall on disk). Red with `HomeDesign::load`.
- save_slot.rs `a_cabin_save_builds_a_cabin_on_its_plot_and_a_homestead_save_a_homestead`. Red with the start ignored ("the homestead design does not fit plot c2").
- home_plot.rs `the_plot_is_remembered_in_the_save`: a Move sets `ship_place`; the next boot builds there; another ship's hash is not trusted. Red with `ship_place` ignored at boot.
- save_slot.rs `switching_servers_swaps_saves_before_the_join`. Red with `slot_ready` always true ("joined server B asking for server A's cabin").
- save_slot.rs `the_look_crosses_at_every_entry`. Red with the server save's old look kept.
- save_slot.rs `a_home_chosen_as_a_guest_joins_again_to_take_it`. Red with `chose` ignored.
- world_load.rs `a_cabin_home_spawns_none_of_the_homesteads_world`. Red with the herd spawned.
- ship_space.rs `a_cabin_breathes_the_ships_air`. Red with `own_air` ignored ("19.5 m3 of its own air").
- home_plot.rs `a_guest_is_told_which_homes_are_free` (first words unchanged). Red with today's sentence.
- connections.rs `an_erased_account_sets_its_server_save_aside`. Red with the save left in place.
- main_menu.rs: the relaunch tests stay green; a new `a_relaunch_on_a_server_pairing_loads_that_servers_save`. Red with the boot reading offline_home.json.
- Checks: `just verify`; then, serialized on the GPU, `just verify-copresence --plots` (all orders), `just verify-shared-build`, `just verify-household` and the default rig, because each now plays a fresh server save (judges that read the sandbox's offline home, such as its parked vehicle, are checked; `judgePlots` already accepts none).

**2B The picker and the probe.** Player-visible: Characters lets you choose where you live, for a new home and on each server.
Files: src/gui/pages/showroom.rs (3.9), src/engine/ipc.rs (probe and verbs), src/gui/ui_snapshots.rs (`snapshot_picker_where_you_live`; after 1E).
- Probe `home_session`: slot ("offline"/"server"), server, start, household, `ship_place`, home kind and box, backpack items, skills count, credits, active quests, own home present, `meals_offline`, the last counter result, the server's `game_home` answer.
- Verbs: `{"pairing":"home"}` and `{"pairing":"server"}` (the picker's Enter, `save_slot::enter_pairing`), `{"choose_start":"cabin"}` (the server card's Choose, `save_slot::choose_on_server`), `{"meal":"1"}` and `{"water":"1"}` (the counter card's buttons).
Tests:
- `the_new_home_row_offers_the_starts_from_data`. Red with Apartment selectable.
- `a_server_card_shows_where_you_live_or_the_choice`. Red with the choice shown while a home is held.
- `the_verbs_parse` and `the_probe_reports_the_home_session`. Red before each.

### Wave 3
**3A The rig.** Player-visible: nothing new; the rig proves a Cabin player starts fresh, eats at the mess hall and goes home.
Files: scripts/verify-copresence.js (`--starts`, on 5b 3A's walker pool), NEW scripts/lib/starts-judge.js and scripts/tests/starts-judge.test.js, Justfile (`verify-starts` beside `verify-shared-build`; `rig-tests` gains starts-judge.test.js and chat-home.test.js).
Test: a good synthetic run passes, and each doctored record fails exactly its own check.

**Docs (orchestrator, last):** ship-homes-and-logistics.md (increment 6 "as built", the stale lines of section 2, question 1 and 2 answers), docs/PRIORITIES.md (TIER 0), docs/STATUS.md, docs/FEATURES.md (:2836-2847 and the mess hall), docs/PAGES.md (Settings > Gameplay "Where you live", Characters), docs/design/characters-and-servers.md (per-server saves as the first slice), docs/design/game-modes.md:110, docs/reference/retention_and_deletion_semantics.md (server saves on the device; an erase sets one aside), docs/ai/onboarding.md and data/ai/onboarding.json (`start`, `game_home_request`), CLAUDE.md (file map, the starts row), the journal.

**Checks per merge:** `just verify`; `cargo check --features relay --no-default-features`; `just validate-data` after Wave 0 and 1A; `just verify-relay` after 1B; `just rig-tests` after 1B, 1F and 3A; snapshots after 1E and 2B; serialized on the one GPU: `--plots` (all orders) after 1A, every rig after 2A, `--starts` after 3A.

## 6. The in-game proof: `just verify-starts`
`node scripts/verify-copresence.js --starts [--dry-verdict <manifest>]`, evidence in .probe-rig/copresence/runs/<stamp>-starts/. ONE game and ONE scripted walker on a throwaway relay; never beside another rig or the operator's game (the machine guard refuses it).

1. **Setup.** The guard, `runFreshGate`, the Dev sandbox with saved servers cleared, as `--build`, except that the sandbox's config pairing is `home:` (the game boots in its own home, alone). The throwaway relay mirrors the tree's data/.
2. **Marked offline.** The probe shows slot "offline", start "homestead", not joined. `{"stock":"1"}`; the rig records the backpack as OFFLINE_MARK. Judged `offline_marked`.
3. **The walker** (TestBotCabin, its own forwarded address) joins with `--start cabin` and sends `home`: it logs `home_plot` c1 kind cabin, and `home: c1 cabin; free cabin 15, homestead 12`. Judged `walker_cabin`, `walker_home_answer`.
4. **The game chooses a Cabin on the server.** `{"choose_start":"cabin"}` then `{"pairing":"server"}`. Judged:
   - `fresh_on_server`: slot "server", start "cabin"; the backpack is exactly the cabin kit read from the tree's starts.ron, none of OFFLINE_MARK; skills 0; credits the fresh default; the active quest gs_first_steps_cabin;
   - `look_crossed`: character name and appearance as offline.
5. **Its home.** The join carries `start`; the welcome's `home_plot` is c2, kind cabin; `last_welcome` "stay" (the choice built the cabin on c2 before the join). Judged `cabin_plot`: home kind cabin, home box c2's 3.0 x 2.6 x 2.5 at (80.5, 0, 90.5), the camera inside c2; `relay.log` has `Game: home chosen (cabin)` and no key on it.
6. **To the mess hall.** The game walks (door route from c2 through the hall, First Street and the Commons corridor to 1.5 m in front of the counter) with `walk_to`; the walker walks its own route there. Judged `walks_to_mess_hall`: arrived, every step relayed, never corrected; the quest's step 2 done.
7. **Eats and drinks.** `{"meal":"1"}`. Judged `meal_eaten`: `take_meal` answered success, no ration left in the backpack, food up, the ledger (Inventory > The fleet's own request) shows one meal used. `{"meal":"1"}` again: refused "not yet" with its sentence (`one_meal_a_meal_time`). `{"water":"1"}`: two bottles full (`bottles_filled`). The walker's `take-meal` logs `meal taken: ration_basic_0` (`walker_ate`).
8. **The opening follows.** Steps 2 and 3 of gs_first_steps_cabin done (`opening_follows`).
9. **Home again.** `{"pairing":"home"}`. Judged `offline_untouched`: slot offline, start homestead, the home a homestead on p1, the backpack OFFLINE_MARK again; and `server_save_kept`: the server save file exists with the cabin's state after the meal.
10. **Back to the server.** `{"pairing":"server"}`: the cabin save comes back, the welcome Stays on c2 (`server_home_remembered`).
11. **Over the run:** no correction of the game or the walker, panics 0, `no_builtin_data`.

**Seen red once on a real build before it is trusted:**
- with `save_slot::follow` doing nothing: `fresh_on_server` fails (the backpack holds OFFLINE_MARK) and so does `offline_untouched`'s server-save half;
- with `assign_home` ignoring `start`: `walker_cabin` fails (p1, a homestead) and the game's `cabin_plot` fails;
- with the counter's offline path taken while joined: `meal_eaten` fails on the ledger.

Every check is also shown failing alone on a doctored copy of a green manifest (`--dry-verdict`).

## 7. If it must land in one day
**Keep:** Wave 0; 1A without the South Commons; 1B; 1C; 1D without offline meals; 2A; 2B; rig steps 1 to 7, 9 and 11.

**Cut, in this order:**
1. the South Commons (its zone, store, furniture, the 5-minute test, `game_in_view` stores);
2. the website's window (logged as a parity debt in docs/design/web-native-parity.md);
3. the recreation corner's furniture (the volume stays, so the opening's step still resolves);
4. the Cabin's opening (a cabin then gets NO opening, never the homestead's);
5. offline Cabin and offline meals (the new-home row greys Cabin: "A cabin's meals come from a server's mess hall for now");
6. Fill your bottles (the cabin kit grows to six bottles of water);
7. last, the Settings section (Characters still offers the choice).

**Never cut:** fresh server saves (the operator's decision), keeping a held plot whatever is asked, and never giving a plot up over a kind mismatch.

### Critical Files for Implementation
- C:\Humanity\src\engine\save_slot.rs (new: which save is live, fresh server saves, the swap, the home of a start)
- C:\Humanity\src\ship\starts.rs and C:\Humanity\data\homes\starts.ron (new: the starts as data and the game_home wire)
- C:\Humanity\src\engine\home_plot.rs (the join's start, a welcome of another kind, the plot kept in the save, the guest sentence)
- C:\Humanity\src\relay\handlers\game_state.rs, C:\Humanity\src\relay\storage\plots.rs and C:\Humanity\src\relay\handlers\home_plots.rs (kind-aware claims, game_home_request / game_choose_home)
- C:\Humanity\data\blueprints\ship_structure.ron with C:\Humanity\data\homes\shipped\cabin.ron and C:\Humanity\data\machines\ship.ron (the cabin block, the mess hall's counter and tables)
- C:\Humanity\src\save_load.rs and C:\Humanity\src\persistence.rs (ActiveHome, the new WorldSave fields, the apply lifted out of lib.rs)

## 8. Decisions by the orchestrator (2026-10-12, operator asleep; each can be overruled)

1. **Which homes the first choice offers:** as recommended. Cabin and Homestead, offline and on servers, the Homestead marked as where most people start; Apartment shown as coming with the first apartment block.
2. **Moving house:** as recommended, not in this increment; an admin's release frees a home.
3. **A server home lives on the computer that played it:** as recommended, accepted for this increment and said on the server card.
4. **Meals:** as recommended, any time, one a meal time per person.
5. **The CC0 furniture download:** LEFT FOR THE OPERATOR (a download needs his yes). Lanes use what the repo already holds (assets/models/furniture.blend first) and name any stand-in in their report; nothing is fetched from kenney.nl until he says so.
6. **BUG-163 part 2, refreshing unedited data files on update:** ADOPTED as recommended, and built as its own lane in Wave 1 (refresh a data file only when it equals the copy the previous version shipped, so a player's edit is never overwritten; an edited file is kept and the Settings > Data row says so). This update changes the ship file, so without it every install extracted before it would be refused by an updated server as "a different ship".
