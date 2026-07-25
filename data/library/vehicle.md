# Creating a Vehicle

This guide shows you how to add your own vehicle to HumanityOS. You do not need to know how to program. You will copy a few lines of text, change some words and numbers, and restart the game. That is all.

## What is a vehicle in HumanityOS?

A vehicle is something you can drive around the game world, like a truck or a rover. In HumanityOS, vehicles start as a "kit": a flat-packed box you carry in your backpack, then deploy (unpack) on the ground to assemble the real vehicle. Every vehicle in the game is described by plain text files that you can read and edit yourself, no programming required.

## What you need

1. **A text editor.** This is a program for editing plain text. Windows comes with one called Notepad. Anything similar works.
2. **The game folder.** This is the folder on your computer where HumanityOS is installed. It contains the game program (`HumanityOS.exe`) and a folder named `data`. All the files you will edit live inside that `data` folder.

A vehicle is defined across three files inside `data`:

- `data/vehicles/kits.ron` : one record describing the vehicle (its name, size, and speed). RON is just a text format, a list of labeled values inside parentheses. You will see it in a moment.
- `data/items.csv` : two rows, one for the kit and one for the assembled vehicle. CSV means "comma-separated values": each line is one item, and commas separate the columns, like a spreadsheet saved as plain text.
- `data/recipes.csv` (optional) : a crafting recipe so players can build your kit at a workbench, or have a vehicle assembler machine produce the finished vehicle directly.

## Step by step

The easiest way to make something new is to copy something that already works and change it one piece at a time.

1. Open the file `data/vehicles/kits.ron` in your text editor.
2. You will see a list of entries. Each entry starts with `(` and ends with `),`. Here is the real Rover entry from the game:

   ```
       (
           kit_item: "rover_kit_0",
           vehicle_item: "rover_0",
           display_name: "Rover",
           body_m: (3.2, 0.8, 1.9),
           cabin_m: (1.5, 0.65, 1.7),
           cabin_offset_x: 0.2,
           wheel_radius_m: 0.5,
           speed_mps: 6.0,
       ),
   ```

3. Select one whole entry (from its opening `(` down through its closing `),`), copy it, and paste the copy just below it. The pasted copy must stay inside the outer square brackets `[` and `]` that wrap the whole list.
4. In your copy, change one field at a time:
   - Change `kit_item` to a new id, for example `"buggy_kit_0"`. An id is a short unique name in lowercase with underscores instead of spaces. No two entries may share an id.
   - Change `vehicle_item` to a matching new id, for example `"buggy_0"`.
   - Change `display_name` to the name players will see, for example `"Dune Buggy"`.
   - Adjust the sizes and speed if you like (the Field reference at the bottom explains each one).
5. Save the file.
6. Open `data/items.csv` in your text editor. Every item in the game is one line in this file. Your vehicle needs two lines: one for the kit, one for the assembled vehicle. Here are the real lines these follow (a kit row, and an assembled vehicle row):

   ```
   -- matching data/items.csv rows:
   rover_kit_0,Rover Kit,vehicle,kit,aluminum,350.0,1,400,Flat-packed all-terrain rover. Deploy it to assemble the real thing on the spot,solid,518.5
   truck_pickup_0,Pickup Truck,vehicle,motorized,steel,2000.0,1,500,Open-bed utility truck,solid,1019.1
   ```

7. Copy those two lines, paste them at the end of the vehicle section of the file, and edit your copies:
   - The first column must exactly match the ids you wrote in `kits.ron` (`buggy_kit_0` on the kit line, `buggy_0` on the vehicle line).
   - The kit line keeps `vehicle` in the category column and `kit` in the subcategory column.
   - The vehicle line keeps `vehicle` in the category column (subcategory `motorized`).
   - Change the name, weight, and description to fit your vehicle. Do not add extra commas: a comma starts a new column.
8. Save the file.
9. Optional: open `data/recipes.csv` and add a recipe that produces your kit at a workbench, or a `vehicle_assembler` recipe that produces the assembled vehicle directly. Copy an existing vehicle recipe line and change the ids, the same way as above. Without a recipe, your vehicle still exists, it just cannot be crafted by players yet.

## Seeing it in the game

Close HumanityOS if it is running, then start it again. Your vehicle list is read once, when the game starts up, so a restart is required.

For the curious, here is exactly what happens: at engine startup, `src/engine/registries.rs` calls `embedded_data::read_data_or_embedded(data_dir, "vehicles/kits.ron")`. The copy of the file on disk, next to the exe, wins; if it is missing, the game falls back to a compile-time embedded copy (an `include_str!` in `src/embedded_data.rs`, line 131). The file is parsed by `VehicleKitRegistry::from_ron` in `src/systems/vehicles/mod.rs` (it is a plain RON list of `VehicleKitDef` records) and stored in the game's DataStore under the key `vehicle_kit_registry`. You do not need to touch any of those code files. The point is: edit the text file, restart, done.

Once in game, get your kit item into your inventory (craft it, or spawn it if you have developer tools enabled), then deploy it on open ground. Your vehicle appears, built from simple box shapes. That is expected: body proportions are drawn as basic shapes until real 3D models are added to the game.

## If something goes wrong

- **The vehicle does not appear.** The game skips files it cannot read rather than crashing, so a typo usually means your entry is silently ignored. Check for a missing comma, a missing quote mark, or an unmatched parenthesis in `kits.ron`.
- **Check your files quickly.** If you have the developer tools installed, open a terminal in the game folder and run `just validate-data`. It reads every data file and reports problems in about a fifth of a second. If you do not have that, simply restart the game and watch whether your vehicle shows up.
- **Ids must match.** The most common mistake: the id in `kits.ron` does not exactly match the id in `items.csv`. They must be identical, letter for letter.
- **Still stuck?** Compare your entry against the untouched Rover entry, one line at a time.

## Field reference

Fields in a `kits.ron` entry:

- `kit_item` : the id of the flat-packed kit item, must match a row in `items.csv`.
- `vehicle_item` : the id of the assembled vehicle item, must match a row in `items.csv`.
- `display_name` : the name shown to players in the game.
- `body_m` : the main body box size in meters, written as (length, height, width).
- `cabin_m` : the cabin (where the driver sits) box size in meters, same order.
- `cabin_offset_x` : how far the cabin sits forward of the body center, in meters. Use a negative number to shift it backward.
- `wheel_radius_m` : the radius of each wheel in meters. Four wheels are placed at the body corners.
- `speed_mps` : driving speed in meters per second. 6.0 is a gentle pace, 12.0 is quick.
- `starter: true` (optional) : marks the vehicle as pre-built in a brand-new home, so new players can drive on day one. Leave this line out for normal vehicles.

Columns in an `items.csv` row, left to right:

- `id` : the unique item id (lowercase, underscores, ends in `_0` for the default style).
- `name` : the display name of the item.
- `category` : always `vehicle` for both rows here.
- `subcategory` : `kit` for the flat-pack, `motorized` for the assembled vehicle.
- `base_material` : the main material it is made of, for example `steel` or `aluminum`.
- `weight_kg` : how much it weighs, in kilograms.
- `stack_size` : how many fit in one inventory slot. Vehicles use `1`.
- `durability` : how much damage it can take before breaking.
- `description` : a short sentence about the item. Avoid commas here; a comma starts a new column.
- `content_class` : what kind of matter it is. Vehicles use `solid`.
- `volume_l` : how much space it takes up, in liters.

One last note: you may find a file called `schemas/vehicle.toml` in the game folder. It describes a richer, planned vehicle format (fuel, hull points, thrusters, and more) for a `data/vehicles.csv` file that does not exist yet. Ignore it for now. The three files in this guide (`kits.ron`, `items.csv`, `recipes.csv`) are the real, working way to add a vehicle today.
