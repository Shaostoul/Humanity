# Mods

Drop mod folders here. Each mod mirrors the root folder structure.

## Structure

```
mods/
  my-mod/
    mod.json          <- mod manifest (name, version, author, load order)
    data/             <- overrides files in root data/
      items.csv       <- replaces or extends root data/items.csv
      recipes.csv
      planets/
        custom.ron    <- adds a new planet
    assets/
      shaders/
        procedural/
          brick.wgsl  <- replaces root assets/shaders/procedural/brick.wgsl
      models/
        custom.glb    <- adds a new model
    web/              <- overrides web pages (for server-side mods)
```

## How It Works

1. Engine loads base files from root `data/` and `assets/`
2. Engine scans `mods/` for mod folders with `mod.json`
3. Mods are loaded in order specified by `mod.json` priority
4. Mod files OVERRIDE base files with the same relative path
5. New files (paths that don't exist in base) are ADDED

## Rules

- Mods never modify the base files (they override by path)
- Updates to HumanityOS never touch the `mods/` folder
- CSV overrides: mod CSV replaces the entire base CSV (merge not automatic)
- RON/JSON overrides: mod file replaces the base file entirely
- Shader overrides: mod shader replaces the base shader

## mod.json Format

```json
{
  "name": "My Cool Mod",
  "version": "1.0.0",
  "author": "YourName",
  "description": "Adds cool stuff",
  "priority": 100,
  "enabled": true
}
```

Priority: lower numbers load first. Default mods use 100. Use 50 for core overrides, 200+ for cosmetic mods that should load last.
