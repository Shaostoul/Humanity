# HumanityOS Mod System

Mods extend or override game data without touching base files.

## Structure

Each mod is a directory inside `data/mods/`:

```
data/mods/
  example-mod/
    mod.json          # Required manifest
    items.csv         # Overrides base data/items.csv
    planets/
      custom.ron      # Adds new planet definition
```

## Manifest (mod.json)

Every mod directory must contain a `mod.json`:

```json
{
  "name": "My Mod",
  "id": "my-mod",
  "version": "1.0.0",
  "author": "Your Name",
  "description": "What this mod does.",
  "load_order": 100,
  "dependencies": []
}
```

| Field          | Type       | Description                                       |
|----------------|------------|---------------------------------------------------|
| `name`         | string     | Human-readable mod name                           |
| `id`           | string     | Unique identifier (matches directory name)        |
| `version`      | string     | Semver version                                    |
| `author`       | string     | Mod author                                        |
| `description`  | string     | Short description                                 |
| `load_order`   | integer    | Lower numbers load first (default: 100)           |
| `dependencies` | string[]   | List of mod IDs this mod requires                 |

## How Overrides Work

Mods can override any file in `data/` by mirroring the path structure.
For example, to override `data/items.csv`, place your version at
`data/mods/my-mod/items.csv`.

### Load order

1. Base files from `data/` are loaded first.
2. Mods are loaded in ascending `load_order` (lower first).
3. Mods with the same `load_order` are sorted alphabetically by `id`.
4. Later mods override earlier mods for the same file path.

### Conflict resolution

If two mods modify the same file, the mod with the higher `load_order`
wins. To control precedence, adjust `load_order` values. There is no
automatic merge -- the entire file is replaced.

## Creating a Mod

1. Create a directory: `data/mods/your-mod-id/`
2. Add a `mod.json` manifest.
3. Add or override data files mirroring the `data/` structure.
4. Launch the game. The mod loader scans `data/mods/` on startup.

See `data/mods/example-mod/` for a working example.
