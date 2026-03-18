# HumanityOS Game Data

All game data lives here in human-readable flat files. No database, no binary blobs.
Edit any file with a text editor; the game hot-reloads on save (F5 to force).

## Directory Structure

```
data/
├── README.md              ← You are here
├── items.csv              ← All game items (weapons, tools, food, materials)
├── plants.csv             ← Crop/plant database with real agricultural data
├── recipes.csv            ← Crafting recipes (inputs → outputs)
├── game.csv               ← Runtime-tunable gameplay settings
├── config.toml            ← Master config (graphics, paths, accessibility)
├── input.toml             ← Keybindings (remappable)
├── calendar.toml          ← Calendar events (recurring + one-time)
├── player.toml            ← New player defaults (stats, spawn, starter gear)
├── quests/                ← Quest definitions (one RON file per quest chain)
│   ├── tutorial.ron       ← New-player tutorial chain (3 quests)
│   ├── construction.ron   ← Advanced building quests
│   ├── exploration.ron    ← Discovery and surveying quests
│   └── farming.ron        ← Agricultural progression quests
├── blueprints/            ← Construction system data
│   ├── materials.ron      ← Physical material properties (density, strength, cost)
│   ├── objects.ron        ← Buildable objects (furniture, lights, utilities, safety)
│   ├── construction.ron   ← Build mechanics (grid, snapping, room presets)
│   └── habitat.ron        ← Default Fibonacci-spiral habitat layout
├── ships/                 ← Ship definitions
│   ├── bridge.ron         ← Bridge room stations and equipment
│   ├── reactor.ron        ← Fusion reactor config and safety zones
│   └── layout_medium.ron  ← Medium player home ship layout
└── solar_system/          ← Celestial body data (real astrophysics)
    ├── sun.ron            ← Sol stellar properties
    ├── earth.ron          ← Earth planetary data
    └── mars.ron           ← Mars planetary data
```

## File Format Conventions

### CSV files (`.csv`)
- First line starting with `#` is a block comment explaining the file purpose
- Lines starting with `#` are comments (skipped by parser)
- First non-comment line is the header row with column names
- All values are comma-separated, no quoting unless value contains commas
- Colon-separated sub-values for compound fields (e.g., `stat:value:stat:value`)
- Pipe-separated for recipe ingredients (e.g., `iron_ore:2|coal:1`)

### TOML files (`.toml`)
- Used for configuration that maps cleanly to key-value pairs
- Standard TOML spec — any TOML parser works
- Comments with `#`

### RON files (`.ron`)
- Rusty Object Notation — used for structured game data with enums and nested types
- Comments with `//`
- Typed constructors (e.g., `Quest(...)`, `RoomDefinition(...)`)
- Parsed by the `ron` Rust crate

## Hot-Reload

The game watches the `data/` directory for file changes. When a file is modified:

1. File watcher detects the change event
2. Parser re-reads the file
3. Changed data is merged into the live game state
4. UI updates on next frame

Press **F5** to force a full reload of all data files.

Files that support hot-reload: all CSV files, all TOML files, quest RON files,
construction RON files. Solar system data and ship layouts require a game restart.

## How to Add New Content

### Adding a new item
1. Open `items.csv`
2. Add a row with a unique `id` (snake_case)
3. Fill in all columns — see the `#` comment block at the top for column definitions
4. Set `volume_liters` and `mass_kg` to physically realistic values
5. Save — item appears in-game after hot-reload

### Adding a new crop
1. Open `plants.csv`
2. Add a row with a unique `id`
3. Use real agricultural data for growth times, water needs, pH, temperature ranges
4. Define growth stages as colon-separated names
5. Add corresponding seed item to `items.csv` with type `seed`
6. Save both files

### Adding a new quest
1. Create or edit a RON file in `quests/`
2. Follow the `Quest(...)` struct format — see existing files for examples
3. Set `prerequisites` to chain quests together
4. Define `objectives` with `target_count` for each goal
5. `auto_start: true` means the quest activates automatically when prerequisites are met

### Adding a crafting recipe
1. Open `recipes.csv`
2. Add a row — ingredients use pipe-separated `item_id:quantity` format
3. Reference items from `items.csv` by their `id`
4. Set `workstation` to empty string for hand-craftable recipes

### Adding a new celestial body
1. Create a new RON file in `solar_system/`
2. Follow the `Planet(...)` or `Star(...)` struct format
3. Use real astronomical data from NASA/ESA sources
4. Game restart required for solar system changes

## Column Definitions for CSV Files

### items.csv
| Column | Type | Description |
|--------|------|-------------|
| id | string | Unique identifier |
| name | string | Display name |
| description | string | Tooltip text |
| type | enum | weapon, armor, tool, consumable, seed, food, ingredient, material, component |
| rarity | enum | common, uncommon, rare, epic, legendary |
| value | int | Base trade value in credits |
| mass_kg | float | Physical mass in kilograms |
| volume_liters | float | Physical volume for volumetric cargo |
| requirements | kv-pairs | Skill:level requirements |
| stats | kv-pairs | Stat modifiers |
| effects | kv-pairs | Applied effects |
| durability | int | Max durability (empty = indestructible) |
| stackable | bool | Whether item stacks |
| equip_slot | string | Equipment slot name |
| animation | string | Animation trigger |
| particles | string | Particle effect descriptor |
| sound | string | Sound effect filename |

### plants.csv
| Column | Type | Description |
|--------|------|-------------|
| id | string | Unique identifier |
| growth_days | int | Real-world days seed to harvest |
| water_liters_per_day | float | Water per plant per day |
| nutrient_n/p/k | float | Nitrogen/Phosphorus/Potassium needs |
| ph_min, ph_max | float | Preferred soil pH range |
| temp_min_c, temp_max_c | int | Growing temperature range |
| humidity_min, humidity_max | float | Preferred humidity range (0-1) |
| yield_min, yield_max | int | Harvest yield range per plant |
| growth_stages | string | Colon-separated stage names |
| seasons | string | Preferred growing seasons |
| companion_plants | string | Beneficial neighbors (IDs) |
| adverse_plants | string | Harmful neighbors (IDs) |
