# Adding a 3D Model to HumanityOS

This guide walks you through adding your own 3D model to the game.
No experience needed. Every term is explained as we go.

## What is a 3D model in HumanityOS?

A 3D model is a file that describes the shape of an object, like a crate, a
machine, or a plant, so the game can draw it. In HumanityOS a model is a
GLB file (a single file that packs the whole shape together; GLTF, a close
cousin, also works). You tell the game about your model by adding a few
lines to a plain text file, and the game does the rest.

## What you need

1. **A text editor.** Any program that edits plain text, like Notepad on
   Windows. You do not need anything fancy.
2. **The game folder.** This is the folder where HumanityOS lives on your
   computer. Inside it you will find a folder named `data`. That `data`
   folder is where all the game's editable content lives, and it is where
   your changes go.
3. **A model file** ending in `.glb` (or `.gltf`). If you do not have one
   yet, the game already ships with a demo model at
   `data/models/test_crate.glb`, and this guide uses it.

A few rules about the model file itself, for when you make your own:
sizes are in meters (a 2 meter crate must be 2 meters in the file), "up"
is the Y direction, and everything should be joined into ONE mesh (one
connected shape) because the game reads only the first shape in the file.
Tools like Blender (a free 3D program) export this way by default. The
full authoring notes live in `docs/game/model-pipeline.md`.

## Step by step

We will add a new machine that uses a model. A "machine" here just means
a placeable object in your homestead.

1. Open the file `data/machines/home.ron` in your text editor. RON is a
   plain text format the game reads: names and values, grouped with
   parentheses. You can read it like a list of labeled facts.
2. Scroll through the file. Each machine is a block that starts with a
   name in quotes, like `"loom": (` and ends with `),`. Find one block and
   look at it for a moment so the shape feels familiar.
3. Copy a whole block, from its `"name": (` line down to its closing `),`
   line. Paste the copy right after the original, inside the same list.
4. Change the name in quotes on the first line. The name must be unique
   in the file. Here is a real, working block you can compare against
   (this exact entry ships with the game):

```ron
        "model_test": (
            shape: "box",
            size: (1.0, 1.2, 1.0),
            color: (0.75, 0.55, 0.25),
            label: "Model test crate",
            category: "Machines",
            stats: [
                (
                    kind: "progress",
                    value: "GLB pipeline demo",
                    status: "ok",
                ),
            ],
            power: None,
            ports: [],
            storage: [],
            rf_emission: 0.0,
            auto_recipe: None,
            container_type: None,
            model: Some("models/test_crate.glb"),
        ),
```

5. Change one field at a time, saving after each change. Start with
   `label` (the name players see), then `size` (width, height, depth in
   meters), then `color`.
6. The important line is the last one: `model: Some("models/your_file.glb")`.
   Put your model file in the `data/models` folder, then write its path
   here starting with `models/`. If you have no model yet, keep
   `models/test_crate.glb` to see the demo crate.
7. Save the file. Start the game (or restart it if it was running), open
   the construction editor, and place your new machine. It appears in the
   catalog under the `category` you chose.

## Seeing it in the game

When you place the machine, the game draws your GLB model instead of the
plain colored box. The box is still there behind the scenes: the `shape`
and `size` fields stay as the fallback (what gets drawn if the model file
is missing or broken) and as the pick volume (the invisible region you
click to select the object). So keep `size` roughly matching your model's
real dimensions.

For the curious, here is what happens inside the game code:

- The loader lives in `src/assets/mod.rs`. The function `parse_gltf_mesh`
  reads geometry only (the shape, no pictures) and is used for machine
  models. A second function, `parse_gltf_mesh_textured`, reads geometry
  plus the base color texture (the image painted on the surface, shrunk
  to at most 1024 pixels) and is used for decorations and trees.
- `resolve_model_path` looks for your file in the game DATA folder first
  (`data/models/...`, the tree you can edit and mod), then in that
  folder's parent (the repo root, so `assets/models/...` also works if
  you downloaded the source code).
- Machine models are wired through the `model` field on `MachineDef` in
  `src/machines.rs`. Each placed machine gets its own copy of the mesh.
- A second surface exists for scattered plants: `data/entities/decorations.ron`
  holds rows like `(model: "grass_medium_02_v1", near: "machine_id",
  count: 8, spread: 5.0)` that sprinkle plant models from
  `assets/models/plants/` near a machine. Those plant files are listed in
  `assets/models/plants/manifest.json` and prepared with
  `node scripts/repack-plant-gltf.js --split`. Textured plants render
  through a special sun-lit, see-through-edges material (search
  `src/engine/world_load.rs` for "Type 19: textured mesh").
- Not everything takes models yet: vehicle kits in
  `data/vehicles/kits.ron` do not have the `model` field so far, and the
  trees you see on planets come from a built-in list, not a data file.

## If something goes wrong

- The game is forgiving. If a model file is missing or broken, the game
  skips it and draws the fallback box instead of crashing. Check the log
  for a warning line naming your file.
- If your new machine does not appear at all, the text file itself
  probably has a typo (a missing comma or parenthesis). If you downloaded
  the source code, run `just validate-data` in a terminal: it checks every
  data file in a fraction of a second and points at the broken line.
  Otherwise, compare your block character by character against the
  example above.
- Still stuck? Undo your change (delete your new block), save, and
  restart. The game returns to normal, and you can try again.

## Field reference

One line per field, in plain words:

- `shape`: the fallback form, usually `"box"`.
- `size`: width, height, depth in meters, like `(1.0, 1.2, 1.0)`.
- `color`: red, green, blue amounts from 0.0 to 1.0 for the fallback box.
- `label`: the name shown to players in the game.
- `category`: which catalog group the machine appears under.
- `stats`: little status lines shown on the machine's info card; each has
  a `kind` (what it is), a `value` (the text), and a `status` (like `"ok"`).
- `power`: electricity use; `None` means it needs no power.
- `ports`: connection points for pipes or wires; `[]` means none.
- `storage`: built-in storage slots; `[]` means none.
- `rf_emission`: radio noise it gives off; `0.0` means silent.
- `auto_recipe`: a recipe it runs by itself; `None` means it does not.
- `container_type`: what kind of container it counts as; `None` for none.
- `model`: the 3D model file to draw; `Some("models/file.glb")` to use
  one, `None` to draw the plain box.
