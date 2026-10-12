# The data folder after an update (BUG-163)

> **Status (2026-10-12):** both parts are built. Part 1 (2026-10-05): a data
> file this version cannot read in full is never used, the copy built into the
> game is used instead and the log says why in one line. Part 2 (2026-10-12):
> every start brings each data file nobody edited up to the running game's copy
> and keeps each edited one, and Settings > Data shows what it did. The decision
> below was taken on 2026-10-12 (A, and unedited homes and machine layouts
> follow the new default): see "Part 2, as built" at the end, which also lists
> where the build differs from the design text above it.

## The decision the operator is asked for

**When an update changes a data file that you, or a mod, edited, what should
happen to your edit?**

- **A. Keep your edit, as today.** Since part 1 it is used only while this
  version can read all of it; when it cannot, the game uses its own copy for
  that run and the log names the file and the line.
- **B. Use the new file, and keep your edit beside it** (for example
  `plants.csv.yours-0.1463.0`), so nothing is lost but nothing of yours is used
  until you merge it by hand.
- **C. Move your edit into a local mod** (`data/mods/local-edits/`, the layout
  `data/mods/README.md` already describes) and put the new file in its place, so
  the base file is always current and your edit applies over it. This needs the
  mod overlay wired first (below), so it is a later step.

The recommendation is **A now, C later**: A loses nothing and part 1 already
stops an unreadable edit from emptying the game; C is the clean long-term model.
Whichever is chosen, files nobody edited are refreshed (step 2 below), which is
the part that matters for every player who never opens the folder.

A second, smaller question is in step 5: whether a home and machine layout the
player never edited should follow a new default.

## What happens today

- On its first run an installed game writes every data file built into it into
  its data folder (`%APPDATA%\HumanityOS\data`, or `data\` beside the exe in
  portable mode): `storage::extract_data_if_needed` and `extract_embedded_to`
  (`src/storage.rs`). That runs only while the folder does not exist, so it
  happens once.
- An update replaces only the exe (`src/updater.rs`). Nothing ever rewrites the
  data folder again.
- Every loader reads the data folder first, so the files there can be edited
  ("Mods = editing files in the data directory", CLAUDE.md). Until part 1 the
  copy built into the exe was used only when a file was missing
  (`embedded_data::read_data_or_embedded`); since part 1, also when this version
  cannot read all of the file (`embedded_data::load_data_or_embedded`).

So after an update, the game reads the data files of the version that first ran
on that machine. Two things can go wrong, and part 1 catches only the first.

1. **The file no longer fits the code.** A column was removed (BUG-162 removed
   `dispel_type` from `status_effects.csv`, and the status effect rows now
   refuse a column they do not declare), a field became required, a format
   changed. Before part 1 the CSV loader skipped each refused row and went on,
   and the registry loaded EMPTY: on the operator's machine, all status effects
   gone, with one warning per row and nothing else.
2. **The file still fits, but it is old.** It loads without a word and the game
   plays with old content. The serde defaults that let "a mod's items.csv
   without the column still parse" (`ItemRow::volume_l`) are exactly what lets
   a stale file load quietly with zeros in its new columns.

## Measured on the one installed copy (the operator's, 2026-10-05)

The folder was written on 2026-07-11 at 02:19:55: all 90 files carry that same
time, so nothing in it was ever edited. It is what an installed copy run from
outside the checkout reads; `just launch` from the checkout reads the checkout's
own `data/` (`find_data_dir` prefers a source-tree data folder). Loaded, from a
copy, through today's loaders:

| File | July | Today | What the game does with the July file |
|---|---|---|---|
| `status_effects.csv` | 67 rows, `dispel_type` column | 70 rows | refused (part 1: the built-in copy is used) |
| `items.csv` | 754 rows | 987 rows | loads: 233 items do not exist in his game |
| `recipes.csv` | 362 rows | 384 rows | loads: 22 recipes missing |
| `plants.csv` | 132 rows, 26 columns | 189 rows, 35 columns | loads: 57 crops missing, and every crop is read without its light, nitrogen-fixing, nutrient-removal, area and light-sum values (each falls back to its default) |
| `creatures.csv` | 92 rows | 101 rows | loads: 9 species missing |
| `equipment.csv` | no `clo` column | `clo` | loads: nothing worn keeps the body warm |
| `abilities.csv` | no `builds` column | `builds` | loads: no ability builds anything (BUG-153's Campfire) |
| `containers/types.csv` | 11 rows, no `direct_contact`, `keeps_zone` | 21 rows | loads: 10 container types missing, and none keeps its contents at a temperature of its own |
| `world/player.ron` | no `starting_items` | a starting kit | loads: a new character starts with an EMPTY kit |
| `food_system.ron` | 36 nutrition profiles fewer | | loads, while today's item list (built in, as his folder has none) names those profiles: 90 foods and drinks are not edible, the water bottle, purified water, milk and oral rehydration solution among them |

Files added since July (illness and treatment data, the climate table, the
container contact rules, `crafting/tools.ron`, ...) are absent from his folder,
so the game uses its own copies of those, which is how a July file and a
current one end up side by side (the food table above). Two files in it are no
longer shipped at all (`gui/navigation.json`, `tools/catalog.json`) and are
simply never read.

## What the folder holds

Three kinds of file live side by side, and a refresh must tell them apart:

1. **Shipped files nobody edited.** Almost all of them, on almost every
   machine. These should simply follow the game.
2. **Shipped files someone edited on purpose:** a modder's `plants.csv`
   (`docs/user/creating/plant.md` tells players to do exactly this).
3. **The player's own files, which the game itself writes:** the in-game editor
   saves the home design (`homes/<kind>.ron`), the ship file and the machine
   layouts (`machines/home.ron`, `machines/ship.ron`) into the data folder
   (BUG-151). These are the player's state, not shipped content, even though a
   first run wrote their starting versions.

## The design

### 1. A stamp of what was written

Whenever the game writes a shipped file into the data folder (the first run, or
a refresh), it records it in `data/.shipped.json`: the game version that wrote
the folder, and per file the hash of exactly the bytes it wrote (BLAKE3,
already a dependency). A file whose hash still matches was not edited. A file
whose hash differs was.

### 2. The refresh, on start

When the exe's version differs from the stamp's, for every file built into the
exe:

- **unedited** (its hash matches the stamp): replace it with the built-in copy
  (written to a `.tmp` beside it and renamed over it, so a crash never leaves
  half a file), and update the stamp;
- **edited**: the operator's decision above (A: keep it, record it as kept);
- **absent**: write it, so a modder finds every shipped file in the folder;
- **no longer shipped**: leave it; nothing reads it.

Then write the stamp with the new version. Logged as one line ("data folder
refreshed from 0.1440.0 to 0.1463.0: 61 files updated, 2 kept because they were
edited"), and shown in Settings > Data (step 6).

### 3. Folders written before the stamp existed

Every install made before step 1 ships has no stamp: the operator's is one.
There is no record of what was written, so "unedited" has to be judged another
way. Options, in order of preference:

- **The extraction time.** A first run writes every file within a second or two,
  so a file whose modified time is within a few seconds of the folder's oldest
  file was not touched since. The operator's folder (90 files, one second)
  passes cleanly. A file edited later has a later time and is kept.
- **Known shipped versions.** A table of the hash of every version of every data
  file the game ever shipped (from git history, generated by a script and
  committed beside the data) would recognise any old shipped file exactly. It is
  precise but needs the history, which a shallow CI checkout does not have.
- **Set the whole folder aside.** Rename it to `data-backup-<date>` (never
  delete) and write a fresh one: nothing is lost, but any edit has to be copied
  back by hand.

The extraction time is the proposal; the folder-aside fallback covers a folder
whose times cannot be read.

### 4. Never in a source tree

The refresh only ever touches `storage::writable_data_dir()` in an installed or
portable install, and never a folder that is the source tree: a data folder
whose parent holds `Cargo.toml` (the same test `find_data_dir` uses), or one
reached through a junction or symlink. Every rig runs the game against the
checkout's own `data/` through a junction (BUG-133), and an autosave that wrote
there once rewrote four tracked files (BUG-151). A test must prove the refresh
leaves such a folder alone, and that it never writes the stamp there either.

### 5. The player's own files

`homes/`, the ship file and `machines/*.ron` are never refreshed until the
operator answers the second question below, and are kept as they are. One the
player edited is theirs whatever is decided. One nobody edited could follow a
new default like any other file, but a new default can move rooms and machines
under things the player placed, which live in the save rather than in these
files, so that is a decision about the player's home, not about data.

**The second question for the operator:** should a home and machine layout the
player never edited follow the game's new default after an update? (The
recommendation: not yet. Keep them until homes have a step that carries what
the player placed over to a new layout.)

### 6. In the app

The GUI-first rule (CLAUDE.md, `docs/design/in-app-ops.md`): Settings > Data
gains a "Data folder" block: written by which version, what the last update
refreshed, which files were kept because they were edited, and for each kept
file "Use the game's copy" (moves yours aside as `<name>.yours-<version>`,
never deletes) and "Show the file". The same block lists any file this run did
not use because this version cannot read it (part 1's log line), so a player
never needs the log to find out why their edit did nothing.

### 7. Tests

Each seen failing first:

- a stamped folder with an unedited, older file gets the new file; an edited one
  is kept (per the decision) and named in the result;
- an unstamped folder with all files at one time is refreshed whole; one file
  with a later time is kept;
- a folder whose parent holds `Cargo.toml`, and a junctioned folder, are left
  byte-for-byte alone and get no stamp;
- a refresh that dies part-way (a write refused) leaves every file whole and the
  old stamp in place, so the next start finishes it.

## Part 1, as built (2026-10-05)

`embedded_data::load_data_or_embedded` (and `load_text_or_embedded` for a parser
that takes text) is the one rule every registry loads through: the data folder's
file when this version can read ALL of it (a CSV is parsed under
`assets::loader::refusing_rows`, so a single row it cannot read refuses the
file), else the copy built into the game, with one `[built-in data copy]` line
naming the file, the line and why. A file with no built-in copy keeps the old
behaviour: the rows this version can read are used (`environment/region_kinds.ron`
and `manufacturing.ron` gained a built-in copy with it, so every registry the
game loads at startup has one). What it covers, and what it does not, is listed
in BUG-163 in `docs/BUGS.md`.

## Part 2, as built (2026-10-12)

**The decision.** Taken on 2026-10-12 by the orchestrator while the operator
was away (`docs/design/ship-homes-increment-6-plan.md` section 8, item 6,
adopted as recommended; he can overrule it): option A. A data file is refreshed
only when it still equals the copy the game last wrote there, and an edited
file is kept. The same item answers the second question (section 5) for now:
the ship file, the home designs (`homes/<kind>.ron`), `homes/starts.ron` and the
machine layouts follow the rule like every other file, so one nobody edited
follows the new default and one the player saved from the editor is kept.
Increment 6 changes the ship file, and an install that kept its old one would be
refused by an updated server as "a different ship". The code has one rule for
every file; carving the homes out again would be a filter in `refresh`.

Where it lives:
- `src/storage/refresh.rs`: the stamp (`Stamp`, `data/.shipped.json`),
  `refresh`, `write_fresh` (the first run), `why_left_alone`,
  `first_run_files`, `use_games_copy_in`, and for the running game
  `refresh_on_start`, `view` and `use_the_games_copy`.
- `src/storage.rs` `extract_data_if_needed`, already called at start (the
  `extract_data` boot phase in lib.rs, which needed no change): it writes a
  folder that does not exist (`extract_embedded_to`, now through `write_fresh`,
  which stamps it) or refreshes one that does (`refresh_on_start`).
- `src/gui/pages/settings/data_folder.rs`: the Settings > Data block (one call
  in settings.rs).
- `src/embedded_data.rs` `note_builtin_copy` remembers each file a run served
  from the built-in copy (`builtin_copies_this_run`), which the block lists.
- `scripts/lib/compiled-in.js`: `refresh_on_start` and `use_the_games_copy`
  are reviewed `FALLBACK_SITES` (they write built-in copies out, and note it).

**The stamp** holds the version, a fingerprint of everything that version
ships (`shipped`: each path and the hash of its bytes), per file the BLAKE3
hash of the bytes the game last wrote there, and the last refresh's report
(`last_update`: from, to, when, updated, added, kept, failed).

**When it runs:** every start of an installed or portable game, before
anything reads the folder. A folder whose stamp names this version AND this
build's fingerprint is current, and nothing but the stamp is read. Otherwise
each file the exe ships is judged: it holds this exe's copy (recorded, nothing
written); it holds what the stamp says the game last wrote (replaced: written
beside it as `.<name>.<pid>.tmp`, then renamed over it); it is absent
(written); anything else was edited (kept, and named). Logged as one line,
"data folder <dir> refreshed from 0.1440.0 to 0.1463.0: 61 files updated, 2
added, 2 kept because they were edited", plus one `[built-in data copy]
data/*` note when anything was written (the rigs' marker: a rig's folder is a
link, never refreshed, so a rig run carrying it went wrong).

**Where the build differs from the design text above**, each a choice made
while building it:
1. **The trigger is the data, not only the version number.** Two builds of one
   version that ship different data (an agent's build of an unbumped tree) are
   told apart by the fingerprint, so the folder follows the exe it runs with,
   an older one included.
2. **A pass that could not write every file** (section 7's "dies part-way")
   keeps the stamp's OLD version, so the next start runs it again, but it does
   record the files it wrote. A stamp left exactly as it was would make those
   files look edited to any later version (the test was seen red against that).
   A crash between two files leaves the old stamp; the files already written
   equal the new copy, which always counts as current.
3. **The first run's moment (section 3)** is not "within a few seconds of the
   folder's oldest file": that takes a single file copied in from a backup with
   an older time for the first run, and overwrites it (seen red). It is the
   earliest 30-second stretch holding at least half of the shipped files present
   and at least 10. A file written outside it is kept, and a folder with no such
   stretch keeps every file that differs from the exe's copy.
4. **The folder-aside fallback is not built.** A file whose time cannot be read
   is kept rather than the whole folder being set aside, which would have taken
   the player's own home out of the game's sight. A stamp that cannot be read
   counts as no stamp (the time rule), with a warning in the log.
5. **Left alone (section 4)** also covers a real data folder with a link on the
   way to any shipped file (probe-sweep's `--shipped-assets` rig links data/'s
   entries one by one) and a folder whose real location is a source tree's
   data/. The first run and "Use the game's copy" refuse such a folder too.
6. **The Settings > Data block (section 6)** shows the version the folder holds
   (or that it was written before stamps, or that the last update could not
   finish), the last update (from, to, when, how many brought up to date and
   added), each file not written and why, each kept file with "Use the game's
   copy" and "Show the file", and each file this run did not use with why. "Use
   the game's copy" renames the player's file to `<name>.yours-<version>` (then
   `-2`, `-3`), writes this version's copy and updates the stamp; it never starts
   a stamp in a folder that has none.

Known limits:
- An editor save rewrites a home, the ship or a machine layout in its own
  format, so a file saved with no real change counts as edited and is kept.
  Before v0.1459.0 (BUG-151) merely opening and shutting the editor did that, so
  an install from then keeps those files until "Use the game's copy".
- Things a player placed live in the save, not in these files, so a home or
  machine layout that follows a new default can move rooms under them (section
  5's concern). That is why the operator may still want the homes carved out.
- The sentence a server running another ship gets (`engine/home_plot.rs`
  `other_ship_sentence`) still says updating the app does not replace the ship
  file. It is now reached only when the ship file was kept (edited) or the
  folder is left alone; pointing it at Settings > Data is in-app-ops debt.

Tests, each seen red against a deliberate break of the code it guards (the break
and the failure text are in each doc comment): in `src/storage/refresh_tests.rs`,
`a_stamped_folder_gets_the_new_copy_of_an_unedited_file_and_keeps_an_edited_one`,
`a_build_of_the_same_version_with_different_data_refreshes_the_folder`,
`an_unstamped_folder_written_at_one_time_is_refreshed_whole_and_a_later_edit_kept`,
`an_unstamped_folder_with_no_first_run_moment_keeps_every_file`,
`a_source_tree_and_a_linked_folder_are_left_alone_byte_for_byte` (real junctions
made with `mklink /J`),
`a_refresh_that_dies_part_way_leaves_every_file_whole_and_the_next_start_finishes_it`,
`the_first_run_stamps_every_file_it_wrote` and
`use_the_games_copy_moves_the_players_file_aside_and_never_deletes_it`; the
block's
`gui::pages::settings::data_folder::tests::the_block_names_each_kept_file_with_its_two_buttons_and_each_file_not_used`;
and `embedded_data::tests::a_file_served_from_the_built_in_copy_is_listed_once_for_settings`.

## The mod overlay (for option C)

`src/mods/mod.rs` already has `ModLoader`: mods in `data/mods/<id>/` mirror the
data folder's paths, later mods override earlier ones (`data/mods/README.md`).
Nothing calls it. Wiring it means every loader resolves a file through
`ModLoader::resolve_path` before `load_data_or_embedded` reads it. With that in
place the base files never need editing, a refresh can always replace them, and
an edited base file can be moved into `data/mods/local-edits/` once, keeping the
player's change applied over every later version.
