# Adding a Room Structure to HumanityOS

This guide shows you how to add your own room structure to the game.
No programming knowledge is needed. If you can open a folder and edit a
text file, you can do this.

## What is a room-structure in HumanityOS?

A room structure is a building piece you can place in the game world:
things like walls, stairs, ramps, ladders, elevators, and floors. Each
one is described in a small text file that the game reads when it
starts. Adding a new structure means adding a few lines to that file,
no code changes at all.

## What you need

1. **A text editor.** This is a program for editing plain text files.
   Windows comes with Notepad, which works fine. (Notepad++ or VS Code
   are nicer, but optional.)
2. **The game folder.** This is the folder where HumanityOS lives on
   your computer. In this guide we call it `C:\Humanity`. If you
   installed it somewhere else, use that location instead wherever you
   see `C:\Humanity`.
3. That is all. No special tools, no internet connection.

### A quick word about the file format

The files you will edit end in `.ron`. RON stands for "Rusty Object
Notation". It is just a way of writing structured information as plain
text, with parentheses and `name: value` pairs. You do not need to
learn it in advance. You will copy an existing entry and change a few
values, and the pattern will be obvious.

### Where structure content lives (the three layers)

Room and structure content is split across three groups of files. All
of them are documented in `C:\Humanity\schemas\room.toml` and
`C:\Humanity\schemas\structure.toml` (those two files are reference
sheets, you read them, you do not edit them).

1. **Room types** (what a room IS, like "kitchen" or "bedroom"):
   `C:\Humanity\data\rooms.ron`. Each entry has a name, purpose,
   actions, access level, default color and material, equipment (item
   ids from `data/items.csv`), power use, life support flag, ambient
   sound, minimum size, and tags. The labels for in-room actions live
   in `C:\Humanity\data\rooms\room_actions.ron`.
2. **Room layouts** (WHERE rooms sit relative to each other): ship
   layouts such as `C:\Humanity\data\ships\layout_medium.ron` and the
   homestead layout `C:\Humanity\data\blueprints\homestead_layout.ron`.
   These refer to room type ids from layer 1.
3. **Structural building pieces** (walls, stairs, ramps, ladders,
   elevators, decks, roads): one entry each in
   `C:\Humanity\data\blueprints\structure_types.ron`. Wall surface
   materials are one-line entries in
   `C:\Humanity\data\blueprints\wall_materials.ron`, and door locks are
   in `C:\Humanity\data\blueprints\lock_types.ron`.

This guide walks through layer 3, because it is the simplest: one entry
per buildable piece.

## Step by step

1. **Make a safety copy.** Open the folder
   `C:\Humanity\data\blueprints` in your file browser. Copy the file
   `structure_types.ron` and paste the copy somewhere OUTSIDE the game
   folder, for example your Documents folder. If anything goes wrong,
   you can restore the original from this copy.
2. **Open the real file.** In your text editor, open
   `C:\Humanity\data\blueprints\structure_types.ron`.
3. **Look at an existing entry.** The whole file is one list, wrapped
   in square brackets `[` and `]`. Each building piece is one entry
   between the brackets. Here is the real "Stairs" entry, exactly as it
   appears in the file:

   ```
       (
           id: "stairs", label: "Stairs", category: "Structure",
           kind: Stairs, shape: Steps, size: (1.4, 3.0, 3.6), color: (0.55, 0.55, 0.58), steps: 14,
           note: "A staircase climbing one storey. Walk up it (the ground-height sampler lifts you step to step).",
       ),
   ```

4. **Copy that entry.** Select the whole block, from its opening `(` to
   the closing `),` and copy it. Paste the copy directly below the
   original, still inside the square brackets. The comma after the
   closing parenthesis matters: it separates entries.
5. **Change the `id` first.** Every entry needs a unique id. Change
   `"stairs"` in your copy to something new, all lowercase, with
   underscores instead of spaces. For example `"spiral_stairs"`.
6. **Change the `label`.** This is the name players see, so normal
   words are fine: `"Spiral Stairs"`.
7. **Change one field at a time.** Save the file after each change and
   check it in the game (next section). Small steps make mistakes easy
   to find. The "Field reference" at the bottom of this page explains
   what every field means.
8. **Keep the quotes and commas.** Text values sit inside double
   quotes. Numbers do not. Every `name: value` pair ends with a comma.
   If you match the shape of the original entry, you are safe.

## Seeing it in the game

There are two loader paths, and they reload differently:

1. **Room types and actions** (`data/rooms.ron` and
   `data/rooms/room_actions.ron`) are read from your disk when the game
   runs. The code that reads them is `RoomTypeRegistry::load` in
   `C:\Humanity\src\ship\room_types.rs`. If a file cannot be read (for
   example, a typo broke the format), the game falls back to an empty
   list instead of crashing. The reference sheet
   `schemas/room.toml` declares these files hot-reloadable: the game
   has a file watcher that notices when you save a change, so many
   edits show up without restarting.
2. **Ship layouts** also have compile-time copies baked into the game
   program itself (`C:\Humanity\src\embedded_data.rs`). These are an
   offline fallback: the asset loader tries the files in your `data`
   folder first, and only uses the baked-in copies if the files are
   missing.

The simple habit that always works: **save your edit, then restart the
game.** Then open the construction menu in build mode. Your new piece
appears in the Structure palette under the label you gave it.

## If something goes wrong

1. **Check your data files.** Open a command window in `C:\Humanity`
   and run `just validate-data`. It reads all the data files in about a
   second and tells you which file has a problem and roughly where.
2. **Restart the game.** Some changes only load at startup.
3. **The game will not crash from a broken file.** It skips files it
   cannot read and continues with the rest. So if your new piece simply
   does not appear, the file most likely has a typo: a missing comma,
   a missing quote, or an unmatched parenthesis.
4. **Worst case, restore your backup.** Copy your safety copy from
   step 1 back over `structure_types.ron` and you are back where you
   started.

## Field reference

Each entry in `structure_types.ron` has these fields:

- `id`: the unique internal name, lowercase with underscores, in quotes.
- `label`: the name players see in the build menu, in quotes.
- `category`: which build-menu group it appears in; use `"Structure"`.
- `kind`: what the piece DOES in the game (Wall, Stairs, Ladder,
  Elevator, Teleporter, Train, Road, or Deck), no quotes.
- `shape`: which placeholder 3D shape is drawn (Box, Steps, Ramp,
  Ladder, Frame, or Slab), no quotes.
- `size`: three numbers in parentheses: width, height, depth, in meters.
- `color`: three numbers in parentheses: red, green, blue, each from
  0.0 (none) to 1.0 (full).
- `steps`: how many steps or rungs it has; use 0 for pieces without any.
- `note`: a short sentence shown to players explaining the piece, in
  quotes.

Want to go deeper? The design rule behind all of this ("anything that
can exist more than once is a data file, not code") is explained in
`C:\Humanity\docs\design\infinite-of-x.md`.
