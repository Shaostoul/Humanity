# Adding Game Data (Items, Recipes, Plants, Machines, Quests)

HumanityOS is data-driven, Space Engineers style: game content lives in flat
files under `data/` next to the exe, hot-reloadable, editable with a text
editor. Modding = editing these files. This doc is the contributor workflow;
the rule behind it is
[docs/design/infinite-of-x.md](../design/infinite-of-x.md): **anything that
can exist more than once is a data file, not code.** No `vec![Thing::a(),
Thing::b()]` literals, ever. Read that doc's pre-ship checklist before adding
any new content category.

## Formats and where things live

| Format | Used for | Examples |
|--------|----------|----------|
| CSV | Tabular databases | `data/items.csv`, `data/recipes.csv`, `data/plants.csv`, `data/creatures.csv`, `data/materials.csv`, `data/skills/`, `data/equipment.csv` |
| RON | Structured configs | `data/machines/home.ron`, `data/quests/*.ron`, `data/blueprints/*.ron`, `data/entities/*.ron`, `data/planets/*.ron`, `data/plants_visual.ron`, `data/vehicles/kits.ron` |
| TOML | Settings + catalogs | `data/config.toml`, `data/input.toml`, `data/calendar.toml`, `data/sounds.toml` |
| JSON | Web-facing + big generated data | `data/glossary.json`, `data/star_systems/sol.json`, `data/i18n/`, `data/tools/` |

`data/README.md` has the full directory map (76+ entries). Loading goes
through `AssetManager` (`src/assets/mod.rs`): `load_csv`, `load_ron`,
`load_toml`, each cached by path, plus `load_*_or_embedded` variants that fall
back to compile-time copies (`src/embedded_data.rs`) so a bare exe still runs
offline. CSV parsing skips `#` comment lines, which is why every CSV opens
with a documented column legend, keep that convention.

## Schemas and validation

- `schemas/*.toml` (23 files) document each data format: `schemas/item.toml`,
  `schemas/recipe.toml`, `schemas/creature.toml`, ... Read the relevant schema
  before adding rows; update it in the same commit if you extend a format.
- **`just validate-data`** is the fast gate (~0.2 s once built): it runs every
  data-loader + data-wiring test in the lib suite (registries parse, recipes
  reference real items, kits resolve, machines exist). Run it after ANY edit
  under `data/`. Details: `docs/contributor/validate_data.md`.
- Full check before pushing: `just verify`.

## Hot reload

The native app watches the data directory (`src/assets/watcher.rs`, notify
crate) and picks up saved changes within seconds, no restart. Some files have
dedicated live-regeneration hooks (editing `data/plants_visual.ron` while the
game runs regenerates every procedural plant, see `src/lib.rs`). If a file you
add needs live behavior beyond cache invalidation, wire its hook where the
watcher's change list is drained in `lib.rs`.

## ID naming conventions (observed in the shipped CSVs)

- snake_case everywhere: `vegetable_tomato_0`, `grain_wheat_0`, `t_shirt_0`.
- **The `_0` suffix = default style/variant** (documented in the
  `data/items.csv` header). Future visual variants of the same logical item
  count up: `_1`, `_2`. New items should carry `_0` from day one.
- Category prefixes on item ids (`vegetable_`, `grain_`, `fruit_`) keep the
  namespace readable; plant ids themselves are bare (`tomato`, `wheat`)
  because the CSV is already the plant namespace.
- Cross-references are by id string: `plants.csv`'s `harvest_item` column
  points at an `items.csv` id; `companion_plants` points at other plant ids,
  colon-separated. The validate-data tests catch dangling references for the
  wired registries.

## Worked example: adding a new plant end to end

Say we add quinoa.

1. **The agronomy row**: add a line to `data/plants.csv` following its header
   legend (26 columns: id, name, description, type, growth_days,
   water_liters_per_day, NPK, pH range, temperature range, humidity range,
   yield range, `growth_stages` colon-separated, seasons, seed/harvest value,
   skill_required, seed_source, companion/adverse plants, harvest_item). Use
   real agricultural data, the file's sources are USDA/FAO/university
   extension services and the game teaches real skills.

   ```
   quinoa,Quinoa,Andean pseudocereal with complete protein,grain,95,0.7,0.08,0.04,0.09,6.0,7.5,10,25,0.30,0.60,3,8,seed:sprout:vegetative:flower:grain_fill:ripe,spring:summer,4,14,2,store:trade,,,grain_quinoa_0
   ```

2. **The harvest item**: `harvest_item` references `grain_quinoa_0`, so add
   that row to `data/items.csv` (category `food` or `material` per its
   legend, with weight_kg, stack_size, volume_l, ...).

3. **The visual recipe**: add a `"quinoa"` entry to `data/plants_visual.ron`.
   This is the FULL 3D appearance, no model file needed:
   `src/renderer/plant_mesh.rs` turns the numbers into a procedural mesh at
   any growth stage (forms: `rosette | herb | vine | tree | bulb | bromeliad`,
   unknown forms fall back to rosette so a new species always renders). Copy
   the amaranth entry as a starting point (also a tall herb with a seed
   plume) and tune `height_m`, colors, `flower_at`/`fruit_at`. Edit live with
   the game running, plants regenerate on save.

4. **Optional recipes**: rows in `data/recipes.csv` to mill/cook it, again
   referencing the item ids.

5. **Optional photoscanned model**: only for hero visuals, see
   [adding-3d-models.md](adding-3d-models.md); the procedural path is the
   default and covers gameplay.

6. **Validate + verify**: `just validate-data`, then boot the game, plant one
   (garden tiles or a field machine), and watch it grow. If the id shows but
   the plant renders as a generic rosette, your `plants_visual.ron` key does
   not match the CSV id.

Note there is also `data/entities/plants/*.ron` (per-species growing-method
detail like NPK ppm per stage, e.g. `tomato.ron`), a deeper agronomic layer
used by the growing systems; add one only if your plant needs stage-level
nutrient behavior.

## Other content types, same shape

- **Machines**: a row in `data/machines/home.ron` (primitive `shape`/`size`
  plus optional `model:` GLB). Placeable via the construction catalog.
- **Quests**: RON chains in `data/quests/` (`tutorial.ron`,
  `farming.ron`, ...).
- **Creatures**: `data/creatures.csv` + `schemas/creature.toml`.
- **Blueprints/construction**: `data/blueprints/*.ron`.
- **Celestial bodies**: their own pipeline, see
  [adding-planets-celestial.md](adding-planets-celestial.md).
- **Sounds**: `data/sounds.toml`, see
  [adding-sounds-music.md](adding-sounds-music.md).

## The rules that gate a merge

1. Infinite-of-X checklist passes (no hardcoded content arrays in code).
2. Schema updated if the format changed; `just validate-data` green.
3. Ids follow the conventions above; cross-references resolve.
4. Real-world numbers sourced honestly (this project teaches real skills).
5. No compatibility shims for old formats pre-launch: when a format must
   change, change it outright (operator directive, 2026-06-30, see CLAUDE.md).
