# Creating Furniture

A beginner's guide to adding your own furniture to HumanityOS. No experience needed.

## What is a furniture in HumanityOS?

Furniture is any object you can place and use inside a room: a bed, a shelf, a desk, a chair. In HumanityOS, each piece of furniture is one line of text in a data file that ships with the game. Add a line, and the game knows about your new furniture. No programming required.

## What you need

1. **A text editor.** A program for editing plain text files. Windows comes with one called Notepad. Anything similar works.
2. **The game folder.** The folder on your computer where HumanityOS is installed. Inside it is a folder named `data`. That is where all the game's content lives. If you are working from the project source, the folder is `C:\Humanity\data`.

That is all. The game reads its content from plain files, on purpose, so anyone can change them. (This is a core design rule of the project, described in `docs/design/infinite-of-x.md`: anything that can exist more than once lives in a data file, not in program code.)

## Step by step

1. **Open the items file.** In your file browser, go to the `data` folder and open the file `items.csv` with your text editor. A CSV file ("comma-separated values") is a plain text file where each line is one item, and each detail on the line is separated by a comma.
2. **Find the furniture section.** Scroll down until you see the line `# === FURNITURE ===`. Lines starting with `#` are comments: notes for humans that the game ignores.
3. **Look at two real examples.** These two lines already exist in the file:

   ```
   bunk_bed_0,Bunk Bed,furniture,sleeping,pine,35.0,1,300,Stacked two-tier bed frame,solid,571.9
   shelf_0,Shelf,furniture,storage,pine,3.0,1,150,Wall-mounted single shelf,solid,49.0
   ```

   Reading the first one, comma by comma: its id is `bunk_bed_0`, its display name is `Bunk Bed`, it is in the `furniture` category, subcategory `sleeping`, made of `pine`, weighs `35.0` kilograms, stacks `1` high (meaning it cannot be stacked), has `300` durability points, has a short description, holds `solid` contents, and takes up `571.9` liters of space.
4. **Copy a line.** Pick the existing furniture line closest to what you want. Select the whole line, copy it, and paste it on a new line right below, still inside the furniture section.
5. **Change the id first.** The id is the first field. It must be unique: no two lines in the file may share one. Use lowercase words joined by underscores, ending in `_0` (the `_0` means "default style"; other numbers are reserved for future style variants). For example: `reading_chair_0`.
6. **Change the rest, one field at a time.** Work left to right: name, then subcategory (use `seating`, `table`, `sleeping`, or `storage`), then material, weight, and so on. The "Field reference" section at the bottom of this page explains each one. Change one thing, save, and keep the commas exactly where they are: the game counts them to know which value is which.
7. **Save the file.** That is it. Your furniture now exists as an item.
8. **(Optional) Make it appear in starter-ship rooms.** Open `data/ships/room_equipment.ron`. A RON file is another kind of plain text data file that the game reads (RON stands for "Rusty Object Notation"). It holds a list of entries like this:

   ```
   // data/ships/room_equipment.ron
   (
       room_type: "quarters",
       items: ["bunk_bed", "locker", "desk_fold", "curtain_divider"],
   ),
   ```

   Each entry names a room type and the furniture that spawns in it. Add your item's id to the list for the room you want, in quotes, separated by commas. Notice the list says `"bunk_bed"`, not `"bunk_bed_0"`: this file uses the base id without the `_0` ending. (The file `data/rooms.ron` carries a parallel per-room equipment list; the same idea applies there.)
9. **(Optional) If your furniture stores things.** A cabinet, a crate, or any furniture that other items go inside is called a storage vessel. Its storage behavior is defined by one row in `data/containers/types.csv`, the same copy-a-line-and-edit approach as step 4. No code involved.

One path to leave alone for now: `data/entities/decorations.ron` exists, but it is a different system that scatters decorative models around anchor points in the world. It is currently empty and reserved for future ground structures. It is not how furniture works.

## Seeing it in the game

The game reads `items.csv` once, when it starts up. (For the curious: the loading code lives in `src/engine/registries.rs`, in a function called `load_data_registries`, which hands the file to the item registry in `src/systems/inventory`. It prefers the file on disk; a backup copy baked into the program via `src/embedded_data.rs` is only used if the disk file is missing.)

The game also watches the `data` folder while it runs. The file `schemas/item.toml` marks `items.csv` as hot-reloadable, which means the watcher (`src/assets/watcher.rs`) notices when you save the file and refreshes the game's copy without a restart.

So, in practice:

1. Save your change to `items.csv`.
2. If the game is running, your item usually appears within moments.
3. If it does not, close the game and start it again. A fresh start always picks up the file.

Changes to `data/ships/room_equipment.ron` affect what spawns when a room is set up, so to see furniture appear in rooms, restart and load into the ship.

## If something goes wrong

Do not worry: the game is built to skip a broken file or a broken line rather than crash. If your furniture does not show up:

1. **Check the commas.** Every line needs exactly 11 values, so exactly 10 commas. A missing or extra comma is the most common mistake.
2. **Check the id.** It must be unique, lowercase, with underscores and no spaces.
3. **Run the checker (if you have the developer tools).** From the project folder, the command `just validate-data` reads every data file and reports problems in about a second, pointing at the exact file and line.
4. **Restart the game.** This forces a clean re-read of everything.
5. **Undo if stuck.** Delete your added line, save, and the game is back to normal. You cannot permanently break anything by editing these files.

## Field reference

Each line in `items.csv` has these 11 fields, in this order. The full contract is documented in `schemas/item.toml`.

| # | Field | Plain meaning |
|---|-------|---------------|
| 1 | `id` | Unique internal name: lowercase, underscores, ends in `_0` for the default style. |
| 2 | `name` | The name players see, with normal capitalization and spaces. |
| 3 | `category` | What kind of item this is. For furniture, always the word `furniture`. |
| 4 | `subcategory` | The furniture's role: `seating`, `table`, `sleeping`, or `storage`. |
| 5 | `base_material` | The main material it is made from, such as `pine` or `steel`. |
| 6 | `weight_kg` | How heavy it is, in kilograms. Use a decimal point, like `35.0`. |
| 7 | `stack_size` | How many fit in one inventory slot. Furniture is normally `1` (not stackable). |
| 8 | `durability` | How much wear it can take before breaking. Bigger number, tougher item. |
| 9 | `description` | One short sentence about the item. Avoid commas here; they would split the line. |
| 10 | `content_class` | What kind of matter it counts as when stored. Furniture is normally `solid`. |
| 11 | `volume_l` | How much space it takes up, in liters. |

That is everything. Copy a line, change the fields, save, and your furniture is part of the game.
