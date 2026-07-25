# Creating a Spaceship

A beginner's guide to adding a spaceship to HumanityOS. No experience needed. If you can open a folder and a text editor, you can do this.

## What is a spaceship in HumanityOS?

A spaceship is the place your character lives and works in the game. It has decks (floors), rooms, and doors, just like a building. The game reads the whole ship from a small text file, so you can change the ship by editing that file.

## What you need

1. **A text editor.** This is a program for writing plain text. Windows comes with one called Notepad. Free editors like Notepad++ or VS Code also work. Do not use Microsoft Word (it adds hidden formatting that breaks the file).
2. **The game folder.** This is the folder where HumanityOS is installed. Inside it is a folder called `data`, and inside that is a folder called `ships`. Ship files live in `data/ships/`.
3. **Nothing else.** No programming knowledge, no special tools.

A quick word about the file format: ship files end in `.ron`. RON stands for "Rusty Object Notation." It is just organized text: names, numbers, and lists, grouped with parentheses. You will see exactly what it looks like below.

## Step by step

Right now the game loads one ship file: `data/ships/starter_fleet.ron`. That file describes the Pioneer, the ship every player starts in. The easiest way to make your own ship is to edit a copy of it, then put your version in its place.

1. **Open the ships folder.** In your file browser, go to the game folder, then open `data`, then `ships`.
2. **Make a backup copy.** Right-click `starter_fleet.ron`, choose Copy, then Paste. Rename the copy to something like `starter_fleet_original.ron`. Now you can always get the original back.
3. **Open `starter_fleet.ron` in your text editor.** You will see something like this (this is the real beginning of the file):

   ```ron
   (
       name: "Pioneer",
       class: frigate,
       length: 40.0,
       width: 16.0,
       height: 8.0,
       decks: [
           (
               deck_index: 1,
               name: "Upper Deck",
               rooms: [
                   (
                       id: "bridge",
                       name: "Bridge",
                       room_type: bridge,
                       position: (0.0, 4.0, 0.0),
                       size: (6.0, 3.5, 5.0),
                       doors: [
                           (connects_to: "quarters", direction: south),
                       ],
                   ),
   ```

4. **Change one thing at a time.** Start small. Change `name: "Pioneer"` to `name: "My First Ship"` and save the file. Small steps make it easy to find mistakes.
5. **Try changing a room.** Find a room's `name` (like `"Bridge"`) and give it a new one. The words in quotes are yours to change. The words without quotes (like `bridge` after `room_type:`) must come from a fixed list, shown in the Field reference below.
6. **Try changing a size.** Numbers like `40.0` are meters. Making `length: 40.0` into `length: 60.0` makes the hull 20 meters longer. Keep the `.0` on whole numbers.
7. **Add a room (optional, once you feel comfortable).** Copy an entire room block, from its opening `(` down to its closing `),`, and paste it after another room. Give the copy a new `id` (no two rooms may share one), a new `name`, a new `position`, and add a door in each of the two rooms you want connected, pointing at each other.
8. **Save the file.** That is it. The next section explains when the game notices your changes.

**Want a completely separate ship file instead?** You can create one (for example `data/ships/my_ship.ron`) with the same shape as above, and there are other examples to study in the same folder: `layout_medium.ron`, `bridge.ron`, and `reactor.ron`. But be aware: today the game only loads `starter_fleet.ron`. A new file will not appear in the game until a small code change tells the game to load it. If that is your goal, ask a programmer (or an AI assistant) to add a load site like `GameWorld::load_starter_ship` in `src/relay/handlers/game_state.rs`, and to register the file in `src/embedded_data.rs` so a copy ships inside the game program as a fallback.

**One thing that is NOT this file:** `data/blueprints/ship_structure.ron` also describes a ship, but it is the ship you build inside the game with the structure editor (handled by `src/ship/ship_structure.rs`). The game writes that file for you. Do not edit it by hand as a way to make a new ship.

## Seeing it in the game

Ship files are read **when a world starts up**, not while the game is running. Changing the file while playing does nothing until a restart.

Here is what happens under the hood, in plain words:

- When the world boots, the game runs a step called `load_starter_ship` (in `src/relay/handlers/game_state.rs`). It reads the text of `data/ships/starter_fleet.ron` and converts it into the ship the game uses.
- If the file is missing or has a typo the game cannot understand, the game does **not** crash. It writes a warning to its log and starts the world with no ship layout.
- The ship's rooms and name are deliberately **never saved** into the world's save data. Every time the world boots, the ship is rebuilt fresh from the file. That means your edits always take effect on the next start, and you can never "corrupt a save" by editing the ship file.

So the routine is simple: save your edit, close the game, start it again, and look around.

## If something goes wrong

- **Check your file before starting the game.** If you have the developer tools set up, open a terminal in the game folder and run `just validate-data`. It reads all the data files in about a fifth of a second and tells you exactly which file and line has a problem.
- **The game will not crash from a broken ship file.** It skips the broken file and keeps going. If your world starts with no ship, that is the sign your edit did not parse.
- **Common typos:** a missing comma at the end of a line, a missing closing `)` or `]`, quotes around a word that should not have them (like `class: "frigate"`, which is wrong; it should be `class: frigate`), or a `connects_to` that names a room `id` that does not exist.
- **Worst case:** delete your edited file and rename your backup copy back to `starter_fleet.ron`. You are back where you started.

## Field reference

Every field in a ship file, in plain words. Words in `quotes` are free text you choose. Words without quotes must be picked from the fixed list given.

**Top level (the ship itself)**

- `name`: the ship's display name, in quotes.
- `class`: the ship's size category. One of: `frigate`, `cruiser`, `carrier`, `station`.
- `length`: hull length in meters (front to back).
- `width`: hull width in meters (side to side).
- `height`: hull height in meters (bottom to top).
- `decks`: the list of decks, from bottom to top.

**Each deck (one floor of the ship)**

- `deck_index`: the deck's number. 0 is the lowest deck, counting upward.
- `name`: the deck's display name, in quotes.
- `rooms`: the list of rooms on this deck.

**Each room**

- `id`: a short unique label for this room, in quotes, no spaces (like `"bridge"` or `"cargo_1"`). Doors use it to name their destination.
- `name`: the room's display name, in quotes.
- `room_type`: what the room is for. One of: `bridge`, `quarters`, `cargo`, `engineering`, `medbay`, `hydroponics`, `armory`, `hangar`. (What furniture and machines appear inside comes from a separate file, `data/ships/room_equipment.ron`.)
- `position`: three numbers in parentheses, the room center's location in meters, measured from the deck's origin point.
- `size`: three numbers in parentheses, the room's width, height, and depth in meters.
- `doors`: the list of this room's doors (can be empty: `[]`).

**Each door**

- `connects_to`: the `id` of the room this door leads to, in quotes.
- `direction`: which wall the door sits on. One of: `north`, `south`, `east`, `west`, `up`, `down`. (`up` and `down` connect decks, like a ladder or lift.)

For the exact technical definition of every field, the source of truth is the `ShipDef` structure in `src/ship/layout.rs`. For the project's rule that content like ships must live in data files rather than code, see `docs/design/infinite-of-x.md`.
