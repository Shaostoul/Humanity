# Adding a Plant to HumanityOS

This guide shows you how to add your own plant to the game. You do not need
any programming experience. If you can open a folder and edit a text file,
you can do this.

## What is a plant in HumanityOS?

A plant is something you can grow in the game: a tomato, a carrot, wheat,
and so on. Every plant is described by one line of text in a data file that
ships with the game. Adding a new plant means adding one new line to that
file, no programming required.

## What you need

1. **A text editor.** This is a program that edits plain text files.
   Windows comes with one called Notepad. Any text editor works.
2. **The game's data folder.** The game keeps its plant list in a folder
   named `data`, which sits next to the game program (`HumanityOS.exe`).
   If you have a copy of the project source code, the folder is
   `C:\Humanity\data`.
3. The file you will edit is called `plants.csv` inside that folder
   (for example `C:\Humanity\data\plants.csv`).

**What is a CSV?** CSV stands for "comma separated values". It is a plain
text file where each line describes one thing, and the details on that line
are separated by commas. Think of it as a spreadsheet saved as simple text:
each line is a row, each comma starts a new column.

## Step by step

1. **Open the file.** In your file browser, go to the `data` folder and
   open `plants.csv` with your text editor (right-click the file, choose
   "Open with", pick Notepad or your editor).
2. **Look at the top of the file.** The first lines all start with a `#`
   symbol. Lines starting with `#` are comments: notes for humans that the
   game ignores. These comments explain what every column means. They are
   the official documentation for this file.
3. **Find an existing plant to copy.** Each plant is one line. Here is the
   real tomato line from the game (it is one single long line in the file,
   even if it wraps on your screen here):

   ```
   tomato,Tomato,Fresh red tomatoes - heavy feeder requiring consistent moisture,fruit,70,1.5,0.15,0.05,0.20,6.0,6.8,18,30,0.50,0.80,2,8,seed:sprout:vegetative:flower:fruit:ripe,spring:summer,5,15,1,store:trade,basil:carrot:parsley,fennel:brassica,vegetable_tomato_0
   ```

4. **Copy that line and paste it at the very bottom of the file**, on a new
   line of its own.
5. **Change the first value.** The first value is the plant's id: a short,
   unique, lowercase name with underscores instead of spaces (this style is
   called snake_case). Change `tomato` to something new, like `blueberry`.
   No two plants may share an id.
6. **Change the rest of the values one at a time.** Work left to right.
   Change the display name, then the description, and so on. The "Field
   reference" section at the bottom of this guide explains each one in
   plain words. Two rules to keep in mind:
   - Do not add or remove commas. The number of values must stay the same.
   - Some values are lists. Lists use colons between entries, like
     `spring:summer`. Leave a list empty (nothing between the commas) if it
     does not apply.
7. **Check the last value carefully.** The final column, `harvest_item`, is
   the id of the item you receive when you harvest. It must match a real
   item id from `data/items.csv`. If it does not, harvesting will not give
   you anything sensible.
8. **Save the file.** In Notepad that is File, then Save.

### Optional: give your plant a custom 3D look

The game can build a 3D model of your plant automatically. If you skip this
step, your plant still works and gets a sensible default look.

To customize it, open `data/plants_visual.ron` in your text editor. RON is
another human-readable text format, a bit like CSV but with named values in
parentheses. Each entry in this file is the full appearance recipe for one
plant: its overall form (rosette, herb, vine, tree, bulb, or bromeliad),
height in meters, leaf, flower, and fruit colors, and at what point in its
growth flowers and fruit appear (`flower_at` and `fruit_at`, as fractions
from 0 to 1). Copy an existing entry, change the key in quotes to match
your new plant's id exactly, and adjust the numbers. The comments at the
top of the file explain each value. You can even edit this file while the
game is running and watch the plants change live.

## Seeing it in the game

1. Save your edits.
2. Start the game (or close it and start it again if it was running).
3. Plant your new seed in the farming area and watch it grow through the
   stages you listed.

For the curious, here is what happens under the hood. When the game starts,
it reads `plants.csv` right away: the startup code in
`src/engine/registries.rs` (a function called `load_data_registries`) hands
the file to `PlantRegistry::from_csv` in `src/systems/farming/mod.rs`, and
the result is stored in the game's data store under the name
`plant_registry`. The file is read by a shared, forgiving parser
(`parse_csv` in `src/assets/loader.rs`) that skips the `#` comment lines,
matches values to columns by the header line, and simply drops any broken
row instead of crashing. If the `plants.csv` file is missing from disk
entirely, the game falls back to a copy that was baked into the program
when it was built (`src/embedded_data.rs`), so the game always has plants.

## If something goes wrong

The game is built to be forgiving. A broken line in `plants.csv` is
skipped, not fatal: the game starts normally and simply acts as if that one
plant does not exist. So if your plant does not show up:

1. Open `plants.csv` again and re-read your line slowly. The most common
   mistakes are a missing comma, an extra comma, or a `harvest_item` that
   does not match a real item id in `data/items.csv`.
2. Compare your line against the tomato line, value by value.
3. Fix it, save, and restart the game.

If you have the project source code and a terminal (a window where you type
commands), you can also run the command `just validate-data` from the
project folder. It checks every data file in about a fifth of a second and
prints exactly what is wrong, which is faster than restarting the game.

One more note: unlike items or creatures, plants have no separate schema
file (there is no `schemas/plant.toml`). The `#` comment block at the top
of `plants.csv` itself is the official field documentation.

## Field reference

The values on each line, in order, in plain words:

| # | Field | Meaning |
|---|-------|---------|
| 1 | `id` | Unique short name in snake_case, like `blueberry`. Never shown to players. |
| 2 | `name` | The display name players see, like `Blueberry`. |
| 3 | `description` | One short sentence about the plant. |
| 4 | `type` | Category: `fruit`, `vegetable`, `grain`, `legume`, `herb`, or `fiber`. |
| 5 | `growth_days` | Days from planting to harvest, based on the real plant. |
| 6 | `water_liters_per_day` | Liters of water one plant needs per day. |
| 7 | `nutrient_n` | How much nitrogen it needs (higher means hungrier). |
| 8 | `nutrient_p` | How much phosphorus it needs. |
| 9 | `nutrient_k` | How much potassium it needs. |
| 10 | `ph_min` | Lowest soil pH it likes (pH is a 0 to 14 acidity scale; 7 is neutral). |
| 11 | `ph_max` | Highest soil pH it likes. |
| 12 | `temp_min_c` | Coldest comfortable temperature, in Celsius. |
| 13 | `temp_max_c` | Warmest comfortable temperature, in Celsius. |
| 14 | `humidity_min` | Lowest comfortable air moisture, from 0.0 (bone dry) to 1.0 (saturated). |
| 15 | `humidity_max` | Highest comfortable air moisture, same 0.0 to 1.0 scale. |
| 16 | `yield_min` | Smallest harvest one plant can give. Fractions are allowed, like 0.3. |
| 17 | `yield_max` | Largest harvest one plant can give. |
| 18 | `growth_stages` | The stages it grows through, separated by colons, like `seed:sprout:vegetative:flower:fruit:ripe`. Every plant can have its own list. If you leave it empty, a built-in default list is used. |
| 19 | `seasons` | Seasons it grows in, separated by colons, like `spring:summer`. |
| 20 | `seed_value` | Trade value of its seeds, in credits (the game's money). |
| 21 | `harvest_value` | Trade value of the harvested crop, in credits. |
| 22 | `skill_required` | Minimum farming skill level needed to grow it (1 is beginner). |
| 23 | `seed_source` | Where seeds come from, separated by colons, like `store:trade`. |
| 24 | `companion_plants` | Ids of plants that grow well next to it, separated by colons. May be empty. |
| 25 | `adverse_plants` | Ids of plants that should not grow next to it, separated by colons. May be empty. |
| 26 | `harvest_item` | The id of the item harvesting gives you. Must be a real id from `data/items.csv`. |

That is everything. One new line in one text file, and you have added a
plant to HumanityOS. For the design philosophy behind this (everything that
can exist more than once lives in a data file, not in code), see
`docs/design/infinite-of-x.md`.
