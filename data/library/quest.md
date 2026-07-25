# Creating a Quest

This guide shows you how to add your own quest to HumanityOS. No programming
knowledge is needed. If you can open a folder and edit a text file, you can do this.

## What is a quest in HumanityOS?

A quest is a small goal the game gives you, like "craft a compass" or "gather
5 ore samples." When you finish all the steps, the game gives you a reward.
Every quest in the game is written in a plain text file, so you can read them,
change them, or add your own.

## What you need

1. **A text editor.** This is a program that edits plain text files. Windows
   comes with one called Notepad. Any text editor works.
2. **The game folder.** This is the folder on your computer where HumanityOS
   is installed. Inside it there is a folder called `data`, and inside that a
   folder called `quests`. That is where all the quest files live.

Inside `data/quests/` you will find five files:

- `getting_started.ron`
- `tutorial.ron`
- `exploration.ron`
- `farming.ron`
- `construction.ron`

The `.ron` ending means the file is written in RON. RON (short for "Rusty
Object Notation") is just a way of writing structured information in plain
text, using parentheses, quotes, and commas. You do not need to learn it in
advance. You will copy an example and change the words.

## Step by step

You can either add a quest to one of the five existing files, or create a
brand-new file in the same folder. The game reads every file in `data/quests/`
that ends in `.ron`, so both ways work. A new file is safest for your first
try, because you cannot break the existing quests.

1. Open the `data/quests/` folder in your file browser.
2. Make a copy of `exploration.ron`. (Right-click the file, choose Copy, then
   Paste. You will get a file like `exploration - Copy.ron`.)
3. Rename the copy to something simple, like `my_quests.ron`. Keep the `.ron`
   ending.
4. Open your new file in your text editor.
5. Delete everything inside it, then paste in this skeleton. The outer square
   brackets `[` and `]` mean "this is a list of quests." Right now the list
   holds one quest:

```ron
[
(
    id: "exploration_first_survey",
    name: "Initial Survey",
    description: "Explore your immediate surroundings and gather information about the local environment.",
    steps: [
        (
            description: "Craft a compass to guide your survey",
            objective: Craft(recipe_id: "craft_compass", quantity: 1),
        ),
        (
            description: "Gather ore samples from the local terrain (5)",
            objective: Gather(item_id: "ore_sample_0", quantity: 5),
        ),
    ],
    rewards: [
        ("flashlight_0", 1),
    ],
    prerequisite: Some("tutorial_first_habitat"),
)
]
```

6. Now change one field at a time. A field is one named piece of information,
   like `id:` or `name:`.
   - Change `id` to something unique, like `"my_first_quest"`. The id is the
     quest's internal name. No two quests may share an id. Use lowercase
     letters and underscores, no spaces.
   - Change `name` to what players should see, like `"My First Quest"`.
   - Change `description` to a sentence explaining the quest.
7. Change the steps. Each step has a `description` (the text the player sees)
   and an `objective` (what the game actually checks). An objective is one of
   these kinds: `Gather` (pick up items), `Craft` (make something from a
   recipe), `Harvest` (harvest a grown plant), or `Build` (place a structure).
   Two kinds exist but do not work yet: `Travel` and `Talk`. Avoid `Travel`
   for now, it never completes (there is a note about this at the top of
   `exploration.ron`).
8. Change the reward. `("flashlight_0", 1)` means "give 1 flashlight." The
   first part is an item id, the second is how many. Item ids must match real
   items listed in `data/items.csv`, and recipe ids (for `Craft` steps) must
   match `data/recipes.csv`. Open those files to find valid ids.
9. Change `prerequisite`. This says which quest must be finished first.
   `Some("tutorial_first_habitat")` means "the player must finish the quest
   whose id is tutorial_first_habitat before this one appears." If your quest
   should be available right away, write `None` instead (no quotes).
10. Save the file. That is it. Your quest is written.

Watch the punctuation: every field ends with a comma, text goes inside double
quotes, and every opening `(` or `[` needs a matching closing `)` or `]`.
Missing commas and quotes are the most common mistakes.

## Seeing it in the game

Close the game if it is running, then start it again. The game reads every
quest file when it starts up, and again every time you load into a world. So a
restart, or leaving and re-entering your world, is all it takes for your quest
to appear.

If a file has a mistake in it, the game does not crash. It writes a note in
its log, skips that file, and keeps loading everything else.

For the curious: the code that does this is `QuestRegistry::from_ron_dir` in
`src/systems/quests/mod.rs`. It reads every `*.ron` file in `data/quests/`,
parses each one as a list of quest definitions, and merges them all into one
registry keyed by quest id. It runs at app startup and again on every world
load.

## If something goes wrong

- **Your quest does not show up.** Run the command `just validate-data` from
  the game folder (or simply restart the game and check that other quests
  still work). This checks every data file and prints exactly which file and
  line has a problem. Most problems are a missing comma, a missing quote, or
  an unmatched parenthesis.
- **The rest of the game is fine but your quest is missing.** The game skipped
  your file because of a formatting mistake. It never crashes over a broken
  quest file, it just ignores it. Fix the punctuation and restart.
- **The quest appears but a step never completes.** Check that the item id or
  recipe id in the objective really exists in `data/items.csv` or
  `data/recipes.csv`, and that you did not use a `Travel` objective.

## Field reference

One line per field, in plain words. The full technical schema with more
examples lives in `schemas/quest.toml`.

| Field | What it means |
|---|---|
| `id` | The quest's unique internal name. Lowercase, underscores, no spaces. |
| `name` | The title players see. |
| `description` | A sentence or two shown to the player explaining the quest. |
| `steps` | The list of things to do, in order. Each step has a `description` and an `objective`. |
| `objective` | What the game checks: `Gather(item_id, quantity)`, `Craft(recipe_id, quantity)`, `Harvest(...)`, or `Build(...)`. (`Travel` and `Talk` exist but do not work yet.) |
| `rewards` | Items given when the quest is done, as `("item_id", how_many)` pairs. |
| `xp_rewards` | Optional. Skill experience given when done, as `("skill_id", amount)` pairs. |
| `prerequisite` | `Some("other_quest_id")` to require another quest first, or `None` for available immediately. |

Why is it built this way? HumanityOS follows a rule called "infinite of X":
anything that can exist more than once (quests, items, recipes) lives in data
files, not in the program itself. That is what makes it possible for you to
add content with nothing but a text editor. You can read more about that idea
in `docs/design/infinite-of-x.md`.
