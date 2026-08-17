# Play and Characters: the WHO/WHERE restructure

Status: PROPOSAL (2026-08-16, from operator direction: "characters could go between worlds on
different servers or saves... the characters page shows characters AND worlds... not quite
structured right"). Each section is separable; react per section. Pairs with
[characters-and-servers.md](characters-and-servers.md) (custody + open/closed policy),
[character-and-save-custody.md](character-and-save-custody.md) (the three-things split), and
[homes-as-profiles.md](homes-as-profiles.md) (a home is a save).

## 1. The model, stated plainly

- A **Character is WHO** you are: name, appearance, outfit. Self-custodial, always.
- A **World is WHERE** you play: a Home (a local world you fully own), an **Open Net** server
  world (you visit with your own character, the server trusts what you bring), or a **Closed
  Net** server world (you commit; the server holds the character).
- **Play = a pairing**: one WHO entering one WHERE. That pairing is the thing the screen
  should let you compose, and the thing the Play button should repeat.
- **Closed Net pins WHO** for anti-cheat: if the client held the character file, progress could
  be forged, so a closed server is the sole writer of progression and the client sends only
  intents. Your identity and look still travel (forging them grants no advantage); your gear
  and XP live on the server. This is the Diablo II open/closed Battle.net split already
  designed in characters-and-servers.md; this doc is only about the screen that expresses it.

Today a local save fuses both halves: `WorldSave` (`src/persistence.rs:15`) holds the world
(inventory, constructions, crops, credits, quests) AND the character (`character_name`,
`appearance`, `outfit`). The UI cannot be fully honest about WHO vs WHERE until that file
splits, but it can present the two axes now and grey the not-yet-possible pairings.

## 2. What exists today, and what is weird about it

Current surface (`src/gui/pages/showroom.rs`, mode 0): left SidePanel (230px) titled "Play"
with three color cards, Your Homes (red), Open Net (green), Closed Net (blue); right
SidePanel (310px) showing either the character editor (Name field, skin/hair/eyes/height,
Backdrop arrows, Enter World) or server details (info fetch, its own Enter World); the 3D
avatar orbits in the center gap. Nav: "Play" enters the world directly when a default
character is set, else opens this picker; "Characters" always opens it
(`src/gui/pages/escape_menu.rs:99-100`, click branch at 612-618; the per-frame open is
`src/lib.rs:16498`).

What is weird, precisely:

1. **One list answers two different questions.** "Your Homes" rows are fused character+world
   saves, "Open Net" rows are servers (pure WHERE), "Closed Net" has no rows at all. The left
   column is WHO and WHERE interleaved, so the operator's "characters AND worlds" reading is
   exactly right.
2. **WHO is an accident of the last click.** Picking a home swaps your whole character
   (`launcher_pending_load` loads that save's name/look); picking a server keeps whichever
   character happened to be loaded last. There is no way to say "as Astra, into X".
3. **The right pane flips identity.** Home selected: a grooming pane (colors, backdrop) that
   says nothing about the destination. Server selected: destination details, but WHO
   disappears entirely, you cannot see who you are about to be.
4. **Two Enter World buttons in two places** (character pane confirm at showroom.rs:105, server
   pane at showroom.rs:384), plus a third label "Connect" (showroom.rs:42) that never renders
   because `draw_server_details` returns before the confirm draws. Dead tuple entry.
5. **Launching walks through editing.** Every trip to the picker passes color pickers and a
   backdrop selector, even when you only want to go.
6. **Small mismatches:** the nav button says "Characters" but the pane title says "Play"; the
   default toggle is a small button buried under the selected home row; the file header still
   says mode 0 opens "on spawn" (stale since v0.476).

## 3. Proposed structure

Keep the two-side-panel + center-3D architecture. Reassign the panels and add a bottom bar:

```
+-----------+---------------------------+------------------+
| WHO       |                           | WHERE            |
| Astra     |       3D avatar           | Your Homes  RED  |
| Wanderer  |     (orbit preview)       | Open Net    GREEN|
| + New     |                           | Closed Net  BLUE |
| [Edit look]                           |                  |
+-----------+---------------------------+------------------+
|  Enter as Astra -> My Homestead (solo)       [ Enter ]   |
+----------------------------------------------------------+
```

**WHO column (left).** One flat list of characters: today, the `character_name` of each local
save (later, standalone `character_v1` files per characters-and-servers.md), plus "+ New
character". Below the list: the selected character's Name field (inline rename is cheap and
safe) and an "Edit look" button. No color pickers here.

**WHERE column (right).** The three color cards move here unchanged in spirit (section_card
red/green/blue, v0.784 language). Your Homes lists save worlds by save name. Open Net lists
servers exactly as now (live-connection virtual row, bookmarks, connected badge). Closed Net
lists server-held worlds when multiplayer lands; until then it keeps its one-paragraph
explainer. Selecting a row expands its details inline in the card: for a home, a small summary
(design, last played); for a server, the existing fetched info block
(`draw_server_details` content re-rooted here, minus its Enter button).

**Bottom bar (new).** The single place the pairing is stated and confirmed:
"Enter as {character} -> {world}", suffixed "(solo)" for a home or "(shared)" for a server,
with ONE Enter button. Disabled states are honest, never hidden: "Connect to this server in
Chat first", "Characters cannot move between homes yet" (until the file split),
"This server holds its own characters; create one here" (Closed Net).

**Closed Net pinning.** Selecting a Closed Net world pins the WHO column to that server's held
characters; local characters grey out with the reason (the greying rule already specified in
characters-and-servers.md). Primary action becomes "New character on this server" when you
hold none.

**Play button.** Play repeats the last successful pairing instantly (persisted on every Enter),
which preserves the behavior the operator likes: straight into the game. No last pairing (first
run, or cleared) opens the picker. "Characters" always opens the picker, unchanged, so a
pairing is never a dead end.

**First run.** No saves: WHO shows "New character" (Wanderer, editable), WHERE shows
"My Homestead (new)", both preselected; the bar reads "Enter as Wanderer -> My Homestead
(new)". One click in, same as today's implicit `NEW_HOMESTEAD` row but legible.

**Appearance editing.** "Edit look" opens the existing focused appearance editor (mode 1) over
the same 3D stage; Done returns to the picker instead of the world (a small return-to flag).
The Backdrop selector moves there: it is stage dressing, not launch flow. The bedroom
wardrobe (mode 2) is untouched.

**Pairing validity (first increment).** A local character stays bound to its own home until the
character/world file split ships, and picking a character still previews it via
`launcher_pending_load`. So initially: WHO row N + Your Homes row N is enterable, cross-home
pairings grey with the reason above, and WHO anyone + a connected Open Net server is enterable
(that cross-pairing already works today, it is just implicit). The two-column structure is
honest from day one; the split only removes grey.

## 4. Migration notes (showroom.rs keeps vs moves)

Keeps: the SidePanel/SidePanel/center-gap architecture; modes 1 and 2 as-is; `section_card`,
`hint`, `detail_row`; `fetch_server_info` / `drain_server_info` / the info cache and
`CONNECTED_SERVER_ID` virtual-row logic; the Back button.

Moves within showroom.rs: `draw_character_select` splits into a WHO column (left) and a WHERE
column (right, absorbing the three cards plus the server-details body); the Name field and
`draw_appearance` leave mode 0 (they live behind Edit look); `draw_backdrop` leaves mode 0;
both Enter buttons collapse into the new bottom bar; the dead "Connect" label dies.

Elsewhere: `GuiState` replaces the either/or `launcher_selected_kind` selection with a pair
(selected character, selected world); `AppConfig.default_character` (`src/config.rs:784`)
becomes last-pairing persistence (add a `last_world` field, serde-defaulted, so old configs
load); the escape_menu.rs Play branch (612-618) reads the pairing instead of
`default_character`; the lib.rs confirm handler (6732) derives `copresence_solo` from the
WHERE kind and records the pairing on success. The later `character_v1` extraction is already
specified in characters-and-servers.md and is not blocked or changed by this restructure.

## 5. Open questions (genuine forks only, with recommendations)

1. **Play memory: explicit default vs automatic last pairing.** Current code has a manual "Set
   as default" toggle; the proposal replaces it with "Play repeats what you last did".
   Recommendation: automatic last pairing. It deletes the buried toggle, matches the desired
   "Play goes straight in", and Characters remains the escape hatch. Add an explicit pin later
   only if switching pairings often proves annoying.
2. **Progression custody when a character moves between local worlds.** Character-bound (D2:
   your inventory/skills travel with you) vs world-bound (Minecraft: the home keeps its stuff,
   only identity travels). Recommendation: world-bound. It matches `WorldSave`, matches the
   custody invariant (identity travels, power is earned where it is used), and avoids
   ferrying inventory between your own saves becoming a dupe machine.
3. **Where renaming lives.** Inline Name field in the WHO column vs everything behind Edit
   look. Recommendation: inline name, everything else behind Edit look. Renaming is identity
   housekeeping; colors and height are a session of their own.
4. **Closed Net card before multiplayer.** Keep the empty explainer card visible vs hide it
   until it has rows. Recommendation: keep it visible. The red/green/blue trust language is
   worth teaching before it is load-bearing, and an empty blue card is a promise, not clutter.

## Implementation files

- src/gui/pages/showroom.rs
- src/gui/pages/escape_menu.rs
- src/lib.rs
- src/gui/mod.rs
- src/config.rs
