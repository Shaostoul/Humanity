# Adding a Recipe to HumanityOS

This guide walks you through adding your own crafting recipe to the game. No experience needed. If you can open a folder and edit a text file, you can do this.

## What is a recipe in HumanityOS?

A recipe tells the game how to turn some items into other items. For example: two pieces of iron ore plus one piece of coal become one iron ingot. Every recipe in the game lives in one plain text file, and you can add your own by adding one line to that file.

## What you need

1. **A text editor.** This is a program that edits plain text files. Windows comes with one called Notepad. To open it, press the Windows key, type "notepad", and press Enter. (Do not use Word or another word processor; those add hidden formatting that breaks the file.)
2. **The game folder.** This is the folder on your computer where HumanityOS is installed. Inside it there is a folder called `data`. The recipe file is `data\recipes.csv`. If you got the game as a source checkout, the full path looks like `C:\Humanity\data\recipes.csv`.

A quick word about the file type: CSV stands for "comma separated values". It is just a text file where each line is one entry, and the parts of the entry are separated by commas. That is all. You can open it like any other text file.

## Step by step

We will add a recipe by copying a line that already works and changing it one piece at a time.

1. **Open the file.** In your file browser, go to the game folder, then into `data`. Right-click `recipes.csv`, choose "Open with", and pick Notepad (or your text editor).
2. **Look at the top of the file.** Lines that start with `#` are comments. Comments are notes for humans; the game ignores them. Below the comments is the header line, which names each column:
   ```
   id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description
   ```
3. **Find a real recipe line.** Here is one that ships with the game:
   ```
   smelt_iron,Smelt Iron,smelting,iron_ore_0:2|coal_0:1,iron_ingot_0:1,10,smelter_0,metalworking,1,Reduce iron ore into a pure iron ingot
   ```
   Read it against the header. Each comma moves you to the next column: the id is `smelt_iron`, the display name is `Smelt Iron`, the category is `smelting`, and so on.
4. **Copy that line.** Select the whole line, copy it (Ctrl+C), click at the very end of the file, press Enter to start a new line, and paste (Ctrl+V).
5. **Change the id.** The id is the recipe's internal name. It must be unique (no two recipes can share one). Use lowercase letters and underscores, no spaces. For example, change `smelt_iron` to `smelt_copper`.
6. **Change the display name.** This is what players see. Spaces are fine here. For example, `Smelt Copper`.
7. **Change the inputs.** Inputs are written as `item_id:quantity` pairs. The `|` character (called a pipe, usually typed with Shift+Backslash) separates multiple pairs. So `iron_ore_0:2|coal_0:1` means "2 iron ore and 1 coal". Every item id you use must already exist in `data\items.csv` or `data\components.csv`. Open those files to find real ids to use.
8. **Change the outputs.** Same format as inputs. `iron_ingot_0:1` means "produces 1 iron ingot".
9. **Set the craft time.** A plain number of seconds. `10` means the recipe takes ten seconds.
10. **Set the station, if any.** Some recipes need a machine, called a station, such as `smelter_0` or `kiln_0`. The value here is that station's item id. Leave this column empty (nothing between the two commas) if the recipe can be made by hand.
11. **Set the skill, if any.** A skill name like `metalworking`, and after the next comma, the minimum level as a number. Use an empty skill and `0` if no skill is needed.
12. **Write a short description.** One sentence about what the recipe makes.
13. **Save the file.** Ctrl+S in Notepad. That is it: your recipe is now part of the game's data.

## Seeing it in the game

You do not need to do anything special to "install" the recipe. The game reads `recipes.csv` on its own:

- At startup, the game loads all data files, including recipes, into its internal catalog (in the code, `RecipeRegistry::from_csv` in `src/systems/crafting/mod.rs` reads the file, and `load_data_registries` in `src/engine/registries.rs` stores it under the name `recipe_registry`).
- The crafting system and the Crafting page's recipe browser both read from that catalog, so your recipe shows up in the recipe list automatically.
- The data is also re-loaded every time you enter the 3D world. So if the game is already running, you can edit `recipes.csv`, save, leave the world, and re-enter it, and your changes are picked up. No restart required (though restarting also works).

If you also use the website's crafting page: the website reads a separate file, `data/recipes.json`, which is generated from the CSV. After editing the CSV, run this command in a terminal from the game folder to regenerate it:

```
node scripts/gen-recipes-json.js
```

The CSV is always the source of truth. The desktop game never reads the JSON, so you can skip this step if you only play the desktop app.

## If something goes wrong

The game is built to be forgiving: if a data file has a broken line, the game skips it rather than crashing. So the worst case is that your recipe simply does not appear.

1. **Check your line against the header.** Count the commas. Every recipe line needs exactly the ten columns from the header, in order. A missing or extra comma shifts everything after it.
2. **Check your item ids.** Every id in inputs, outputs, and station_required must exist in `data\items.csv` or `data\components.csv`. A typo in an id is the most common mistake.
3. **Run the data checker.** If you have the developer tools set up, open a terminal in the game folder and run:
   ```
   just validate-data
   ```
   It checks all data files in a fraction of a second and tells you exactly what is wrong and where.
4. **Restart the game.** If in doubt, close the game fully and start it again so everything loads fresh.
5. **Undo is always possible.** If things get messy, delete the line you added, save, and you are back where you started.

## Field reference

One line per column, in order:

| Field | What it means |
|---|---|
| `id` | The recipe's unique internal name. Lowercase, underscores, no spaces. |
| `name` | The name players see in menus. Spaces allowed. |
| `category` | What kind of recipe it is: smelting, refining, crafting, cooking, construction, electronics, assembly, textile, or chemistry. |
| `inputs` | What the recipe consumes: `item_id:quantity` pairs separated by `|`. |
| `outputs` | What the recipe produces: same `item_id:quantity` format. |
| `craft_time_sec` | How many seconds the recipe takes. |
| `station_required` | The item id of the machine needed (like `smelter_0`), or empty for hand-crafting. |
| `skill_required` | The skill needed (like `metalworking`), or empty for none. |
| `skill_level` | The minimum level of that skill, as a number. Use `0` for none. |
| `description` | A short sentence describing what the recipe makes. |

One more file worth knowing about: `schemas/recipe.toml` is the formal written specification of this format. It describes some extra fields (like byproducts and failure chance) and a `data/recipes/` folder of per-recipe files. Those are planned but not yet in the game: today, the game only reads the ten CSV columns listed above. When the schema and this guide disagree, trust this guide and the CSV header.

Happy crafting!
