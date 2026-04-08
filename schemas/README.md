# HumanityOS Data Schemas

Reference documentation for all game data formats. Anyone creating content
(modders, AI agents, community contributors) should read the relevant schema
before adding new items, creatures, vehicles, planets, etc.

## How game data works

All game data lives in `data/` next to the executable (like Space Engineers).
Files are hot-reloadable: edit a CSV or RON file, and the game picks up changes
within seconds without restarting.

**Formats used:**
- **CSV** for tabular data (items, materials, recipes, creatures, skills)
- **RON** for structured configs (quests, planets, ships, blueprints, construction)
- **TOML** for settings (input, calendar, player defaults)
- **JSON** for web-facing data (glossary, tools catalog, localization)

**Modding = editing data files.** No code changes needed. Drop new CSVs or RON
files into the data directory and the game loads them.

## Schema files

| Schema | Covers | Data format |
|--------|--------|-------------|
| `item.toml` | Clothing, tools, weapons, food, furniture, machines | CSV |
| `material.toml` | Metals, fabrics, wood, ceramics, polymers | CSV |
| `component.toml` | Gears, motors, circuits, pipes, wiring | CSV |
| `recipe.toml` | Smelting, crafting, cooking, construction, alchemy | CSV |
| `vehicle.toml` | Ships, cars, mechs, bikes, boats, aircraft | CSV/RON |
| `structure.toml` | Walls, floors, machines, furniture, decorations | CSV/RON |
| `creature.toml` | Humans, animals, aliens, monsters, NPCs | CSV |
| `npc.toml` | Merchants, quest givers, companions, guards | RON |
| `spell.toml` | Magic, abilities, tech powers, psionics | CSV |
| `biome.toml` | Terrain types, flora/fauna distribution | RON |
| `celestial_body.toml` | Stars, planets, moons, asteroids, stations | RON |
| `quest.toml` | Main quests, side quests, procedural quests | RON |
| `faction.toml` | Governments, guilds, corporations, alien civs | RON |
| `skill.toml` | Combat, crafting, survival, magic skills | CSV |
| `equipment_slot.toml` | Wearable slots, armor layers, accessories | CSV |
| `container.toml` | Chests, backpacks, tanks, fridges, silos | CSV |
| `status_effect.toml` | Buffs, debuffs, diseases, environmental effects | CSV |
| `weather.toml` | Rain, storms, fog, sandstorms, meteor showers | RON |
| `sound.toml` | Music, SFX, ambient, voice, spatial audio | TOML |

## Conventions

- **IDs** use `snake_case` (e.g. `iron_ingot_0`, `laser_rifle_mk2`)
- **The `_0` suffix** means "default style variant" (e.g. `t_shirt_0` is the plain version)
- **Pipe-separated lists** for multi-value fields (e.g. `steel:2|copper:1`)
- **Colon-separated pairs** for key:value data (e.g. `iron_ore:0.8` means iron ore at 80% abundance)
- **References** between schemas use IDs (e.g. a recipe's `station_required` references a structure's `id`)
- **Comments** in CSV files start with `#` and are ignored by the parser
- **Section headers** in CSV use `# === SECTION NAME ===` for organization

## Creating new content

1. Read the relevant schema file
2. Copy an existing entry as a template
3. Change the `id` (must be unique within that data type)
4. Fill in fields (required fields are marked in the schema)
5. Save the file (hot-reload picks it up automatically)
6. Test in-game

## Data relationships

```
Materials --> Components --> Items --> Recipes
    |              |           |
    v              v           v
Structures    Vehicles    Equipment
    |              |           |
    v              v           v
Biomes       Planets     Creatures --> Factions
    |              |           |           |
    v              v           v           v
Weather    Solar Systems   Quests     Skills
```
