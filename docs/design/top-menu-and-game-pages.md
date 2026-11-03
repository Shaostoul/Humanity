# The top menu, the game buttons, and the Humanity page as the overview

**Status (2026-11-02):** a plan with options. Sections 4a to 4d are taste decisions for the
operator: nothing in them is built or decided. Section 5, the Characters page relayout, is decided
(his concrete complaint) and built in the same piece of work.

Read against main at ee0d59ee1 (v0.1547). Sources: `src/gui/pages/escape_menu.rs` (the top menu),
`src/gui/pages/humanity.rs`, `src/gui/pages/showroom.rs` and `where_you_live.rs` (Characters),
`src/gui/pages/main_menu.rs` (the title screen), `web/shared/shell.js` (the website's menu),
[app-bottom-strip.md](app-bottom-strip.md) sections 5 and 7, [two-realities.md](two-realities.md),
[play-characters.md](play-characters.md), [humanity-page.md](humanity-page.md),
`docs/reference/ui-navigation-map.md`, `docs/PAGES.md`.

## 0. The operator's words (2026-11-02, answer 5 of app-bottom-strip.md section 7)

> I feel like the Humanity page was kinda meant to be the overview but we're kinda missing the
> mark. Realistically we need to think about the top menu buttons/pages. I also think we need to
> work on the "game" buttons. Like play and characters. The characters page definitely needs work
> because, it looks confusing right now. The left and right bar areas are small so a lot of text is
> crammed vertically. We need to even things out.

And later the same day, on the plan (relayed by the orchestrator):

> I almost feel like we should have a like... worlds/saves/homes page. I also feel like we should
> have a dedicated servers page. Like listing the ones you're in and possibly like a server
> browser. I'm not sure. Thoughts?

Section 4d answers it.

His screenshot of Characters at 2000 x 1125 showed a 230-point WHO column and a 310-point WHERE
column (both fixed in code), about 1,580 points of starfield between them, the connected server's
whole card stacked in the WHERE column (eleven lines of details, then the three kinds of home each
as a long wrapped paragraph), and the other two servers below the fold.

## 1. The top menu today

`escape_menu.rs` draws one row (it wraps on a narrow window), in this order. Colours are the
group's theme token; a dot separates groups. Snapshot of the whole row at the operator's width:
`tests/snapshots/nav_bar_full.png` (and wrapped at 720 points: `nav_bar_wrapped.png`).

| # | Button | Group colour | Opens | What it is for |
|---|---|---|---|---|
| 1 | Play | sim purple | the world (page None) | repeats the last pairing (a character in a world); none recorded: the picker |
| 2 | Characters | sim purple | the picker (page None) | choose who and where, then Enter |
| 3 | Apps | sim purple | Apps | games on this computer, HumanityOS's own tools, other programs, help and services |
| 4 | Humanity | reality red | Humanity | the mission landing, Governance, Laws, Directory, Donate (its own side list) |
| 5 | Chat | legacy red | Chat | messages, groups, servers |
| 6-8 | Studio, Watch, Files | legacy red | their pages | stream, watch streams, the files people share |
| 9 | Profile | legacy green | Profile (`GuiPage::Real`) | your body and measurements, identity, notes, network profile, skills, links; Wallet; Market, Trade, Guilds |
| 10 | Home | legacy green | Home | your homestead's design and loops, the household, the animals, where you live |
| 11 | Quests | sim purple | Quests | the game's quests |
| 12 | Tasks | sim purple | Tasks | real to-dos (kanban), with the learn-by-doing chains suggested |
| 13 | Inventory | sim purple | Inventory | the character's backpack and equipment |
| 14 | Crafting | sim purple | Crafting | recipes and the craft queue |
| 15 | Maps | sim purple | Maps | planet, ship, solar system, galaxy, night sky |
| 16 | Dev | tools blue | Dev | bugs, testing, performance, dev tools, planet tuner, Claude |
| 17-18 | Library, How-to | reality red | their pages | what you read; what you do |
| 19 | Settings | settings grey | Settings | everything configurable |
| 20 | Aa | none | none | icons and words, icons only, or words only |

Beside the row, anchored to the window: the notices bell and the help (?) button.

**Where the buttons overlap or mislead (each read in the code today):**

1. **Three doors into the world.** Play (the pairing), Characters (the picker), and the title
   screen's "Enter World" (`main_menu.rs`, which sets the page to None and takes neither the
   pairing nor the picker).
2. **Four pages about "you".** Profile (body, identity, notes, wallet, market), Home (the
   homestead), Humanity's Directory (identity lookup) and, on the Sim side, Inventory. Profile's
   sections are mostly Real; Home is mostly the game's homestead; neither is labelled Real or Sim.
3. **The colours do not mean what the groups say.** Quests, Tasks, Inventory, Crafting and Maps
   are each a group of one in sim purple, though Tasks and Maps serve the Real side too; Library
   and How-to share Humanity's red; Chat, Studio, Watch and Files share a "legacy red".
4. **Game systems sit in the app's chrome.** two-realities.md (operator, 2026-07-16): "The
   character, inventory, and crafting live inside the world, reached from a backpack / in-game
   menu, not from the app chrome." Inventory and Crafting are still top-menu buttons.
5. **Nineteen buttons.** At 2000 points they fit one row; under about 1,500 they wrap to two.

The website's menu (`shell.js`) mirrors it with the native-only entries mapped: Play to the
Download page, no Characters, no Studio, Quests as Onboarding.

## 2. The Humanity page today, and what an overview should show

**Today** (`humanity.rs`, snapshot `tests/snapshots/humanity.png`): a side list (Mission Dashboard,
Governance, Laws, Directory, Donate) and, by default, the Mission Dashboard: the heading and the
mission line, "For you" and "For all of us", a Start here card (Get oriented, See your Laws, Fund
the work, Shape the rules), then long essays (Why this exists, Early days, Our mission, and three
folds: Why it's built this way, What it protects, Built for every situation), How we get there,
Where we stand (the server's live community numbers), Be part of it, and a closing line. It is the
first screen by default (`default_page`). It says what the project is for; it says nothing about
the person reading it, their day, their home or their character.

**An overview of the person's world** (two-realities.md: the app is your real life, the game is a
place you walk into; app-bottom-strip.md section 5: "a tightly packed overview of the
default/demo/active character's stats"):

| Band | Side | What it shows | From (all values the app already has, unless noted) |
|---|---|---|---|
| You | Real | today's tasks, the next calendar event, unread messages, the newest note, Real Me's last measures (Profile > Body), what your homestead needs | Tasks, Calendar, Chat, Notes, Profile, Home (`power_balance`, `water_days_autonomy`) |
| Your character | Sim | Food, Water, Energy, Air, temperature, health, wounds; where they live; the next quest step; crops ready | `GuiVitals` through `hud::vital_rows` (in the world), else the active character's save read once, "as of" its time; quests; `GuiCrop` |
| Us | both | the server's members and who is online, open proposals, the community numbers, the mission in one line | `/api/civilization` (already fetched for Where we stand), Governance |

Each tile is data (`data/overview/tiles.json`, bottom-strip plan 5.6), a word beside every
number, colour never alone, its hint by key, and a click opens the page it summarises. A person
with no character yet sees the demo character labelled Example (plan 5.3).

## 3. Play and Characters today

- **Play** repeats the last pairing straight into first person (`GuiState::apply_last_pairing`);
  with none it opens the picker. Its hint does not say which pairing it repeats.
- **Characters** always opens the picker (`showroom.rs` mode 0): WHO on the left (your characters,
  New character, Name, Edit look), WHERE on the right (Your homes in red, Open Net in green, Closed
  Net in blue), the character on a 3D stage between them, and one Enter bar along the bottom that
  states the pairing ("Enter as Shaostoul -> united-humanity.us (shared)").
- **The title screen's Enter World** enters the world with neither.

Should they stay two? They do two different jobs (go now; choose first), and play-characters.md's
open question 1 settled that Play repeats what you last did. What is missing is that Play never
says what it will do, and that a third door does neither. Options in 4c.

## 4. Options for the operator

Costs are what each takes to build, read from the code today; "an increment" is one agent's
piece of work with its tests and snapshots.

### 4a. The top menu's grouping

**Option A1: regroup the one row by the two realities (recommended).** Seven groups, each a colour
that means one thing, and the game's own systems moved into the game:

`[Humanity]  [Play | Characters | Worlds | Servers]  [Chat | Watch | Studio | Files]  [Profile | Home | Tasks | Maps]  [Library | How-to | Apps]  [Dev]  [Settings]`

Inventory, Crafting and Quests leave the top menu and are reached from inside the world (the
backpack and the in-game menu two-realities.md describes) and from the overview's "Your character"
band. Worlds and Servers join the game group if 4d's D1 is chosen (without them the group is
Play and Characters). 18 buttons with them, 16 without, instead of 19. Cost: one increment (`escape_menu.rs`, its web twin `shell.js`,
`ui-navigation-map.md`, the nav snapshots, a test pinning the order and groups), and a door inside
the world first: today only Inventory has one (the I key, `lib.rs`), so Crafting and Quests need a
key or an in-world menu before their buttons can leave. Risk: someone who used them from the menu
looks for them; the in-world door and the overview's links answer that.

**Option A2: a category row with hubs.** Six category buttons (Overview, Play, Talk, Life, Learn,
Make) each opening a hub page with its pages in a side list, the way Humanity and Dev do now. Fewer
buttons, one more click to most pages. Cost: two or three increments (the hub pages, the menu, the
web twin). Risk: the app had a two-tier menu and category landing pages, and removed both
(v0.196: "single-row nav is cleaner"; v0.699 deleted the unreachable landing pages): this goes back
the other way.

**Option A3: keep every button, fix the meaning.** Same nineteen, regrouped and recoloured so a
colour means one thing (Real, Sim, both, tools, settings), Humanity first. Cost: half an
increment. Leaves the overlaps of section 1 in place.

### 4b. The Humanity page as the overview

**Option B1: an Overview section first on the Humanity page (recommended).** The side list gains
Overview at its top and it is what the Humanity button opens: the three bands of section 2 (You,
Your character, Us) as tiles from `data/overview/tiles.json`, with the mission in one line and a
link to the Mission section. The Mission Dashboard becomes the "Mission" section, every word kept.
Cost: the bottom-strip plan's rungs 4 and 5 (two increments: the tiles and readers with the demo
character; the saved state "as of" and the Real tiles), placed on the Humanity page instead of a
new page. Nothing is lost; the first screen becomes about the person and links out to the mission.

**Option B2: the overview replaces the Mission Dashboard's top half.** The hero, mission line and
Start here stay; under them the three bands; the essays move into folds lower down. One page, one
scroll. Cost: the same two increments, plus a rework of `draw_mission_dashboard`. Risk: one page
doing two jobs (who I am today, and what the project is), which two-realities.md warns against.

**Option B3: split by who it is about.** Humanity stays the collective ("us": mission, community,
governance); Home becomes the person's real overview ("me", as two-realities.md's "Home = your real
life" describes); Characters shows the character's state beside its stage (the Sim side). Cost:
three increments in three places. Closest to the axiom, but it answers "the Humanity page was meant
to be the overview" with "no, three pages are".

### 4c. Play and Characters

**Option C1: keep two buttons, make Play say what it does (recommended).** Play's hint and, on a
wide row, its label name the pairing it repeats ("Play: Shaostoul on united-humanity.us"; with none,
"Play: choose who and where"). Characters is unchanged. The title screen's Enter World becomes
Play (the same pairing logic). Cost: a small increment (`escape_menu.rs`, `main_menu.rs`, words by
key, a test that the title screen and Play enter the same pairing).

**Option C2: one Play button with a "Change" segment.** A split button: the main part repeats the
pairing, an attached segment opens the picker. One fewer top-menu button. Cost: a new split-button
widget in `src/gui/widgets/` and its web class, plus C1's work.

**Option C3: Play always opens the picker,** with the last pairing preselected and Enter focused, so
going in is Play then Enter (or Play then the Enter key). Characters goes away. Cost: small. Costs
the one-click "straight into the game" the operator asked for on 2026-06-16
(play-characters.md 3, "preserves the behavior the operator likes").

### 4d. A Worlds page and a Servers page (the operator's question of 2026-11-02)

**Why they fit.** The picker answers three questions on one screen: WHO you are, WHERE you live,
and WHICH SERVER that is on. Today the WHERE column carries the second and third in full (every
home, every server, a server's whole card, every kind of home), which is why it towers. Pages of
their own for worlds and servers let the picker keep only what a person needs to choose, and give
the rest somewhere to live where it can grow (infinite of X: many saves, many servers). Play then
means "go, with the last character and world", and Characters means "pick".

**What exists to build on (read 2026-11-02):**

- *Saves.* `persistence::SaveSlot`: your own home is `saves/offline_home.json` (`Offline`), and each
  server keeps a save per identity on this computer, `saves/servers/<server hash>-<identity>.json`
  (`Server { server, identity }`, ship homes increment 6); other top-level saves (`auto_save.json`)
  are what the picker's `load_homes` scans today. A `WorldSave` carries its name, the character's
  name and look, its start (`Cabin`, `Homestead`), the time it was saved, the inventory, skills,
  constructions, crafts and game time: everything a Worlds card would show. The picker today lists
  only the top-level saves; the server saves are not listed anywhere a person can see.
- *Where you live.* `where_you_live.rs`: the live save's start and household (Settings > Gameplay,
  the Home page's line), a server's answer (`game_home`: your plot there, or the kinds of home free).
- *Servers you are in.* `GuiState::chat_servers` (the saved list, kept in config.json since
  2026-08-14) and `connections` (one live socket per saved server, `engine/bg_connections.rs`); the
  chat sidebar's Servers section; the picker's Open Net card (the live row plus the saved ones);
  each server's `/api/server-info` (name, description, members, online, in the world, channels,
  the Accord), cached per server (`server_info_cache`).
- *Servers you run.* The Servers page that exists (`GuiPage::RelayControl`, "MY SERVERS", opened
  from the chat sidebar's Servers header): health, actions, the VPS console, a node on this PC.
- *The federation.* Each server's public `GET /api/federation/servers` (its linked servers: name,
  address, trust tier, the Accord, online or not, last seen), already fetched per connected server
  for the Commons (`pages/chat/commons.rs`). federation-ux.md planned the **Federation Directory**
  on exactly this ("Discovery, not auto-add": you discover, you choose to join; trust is never
  chained for you) and never built it.

**Option D1: a Worlds page and a Servers page, the picker their summary (recommended).**

- *Worlds:* one card per place you have a home or a save: your own home on this computer, each
  server's home (from `saves/servers/`, named by its server), any other offline world. Each says
  what it is (a Cabin, a Homestead; the household), who lives there (the character's name and
  look), when you last played, and what is in it (the things you carry, what you have built and
  grown, the game time), with Play here (opens the picker with this world chosen) and, for a
  place with no home yet, Choose where you live. Nothing is ever deleted from it: an erased or old
  save is set aside, as the save code does now.
- *Servers:* the servers you are in (saved and connected), each with its status, how many people,
  its description, the Accord, and your home there ("Where you live here: a Cabin, c2"); the
  servers you run (today's RelayControl page becomes this page's "Yours" part, one servers concept
  in one place, as federation-ux.md wanted); and **Find more**: the Federation Directory, every
  server your servers are linked to, each labelled with whose list it came from ("linked from
  united-humanity.us"), with Add to my list. Before a person adds a stranger's server, the page
  says plainly what that server will and will not have (its operator sees what you send there and
  your public name; your own home and your other servers' saves stay on this computer; a server
  starts you fresh and keeps your home there on this computer; its rules and whether it declares
  the Accord), each fact checked against the code before it is written (the crypto words only
  from `docs/reference/cryptography.md`).
- *The picker* keeps WHO, a WHERE column that is the summary of the two pages (a card per world
  and per server, the selected one's few lines, and "where you live here" with Choose, because a
  first-time player must be able to choose a home and enter from one screen), and the Enter bar.
  The details the picker folds away today move to those pages and the folds become links to them.
- Cost: Servers, two increments (the page with Yours and the servers you are in; then Find more
  with its safety words and a test that it never adds a server by itself); Worlds, one or two
  (listing the server saves needs a reader of `saves/servers/` that names each by its server).
  Each needs its web twin decided: the website can list servers (and already reads
  `/api/federation/servers`) but has no saves.

**Option D2: one Worlds page with two parts,** Worlds and Servers, under one menu button. The same
content as D1 behind one button. Cheaper in menu space; mixes "where my things are" with "who I
can visit".

**Option D3: no new pages; deepen the picker.** The WHERE column's folds open into a third panel
over the stage. The least code; the picker stays three pages in one, the thing this plan is about.

**The orchestrator's view, weighed:** "splitting WHO (Characters), WHERE (Worlds) and WHICH
SERVER (Servers) is what would let the picker stop being three pages crammed into one, and Play
then becomes 'go, with the last character and world, or pick'." Agreed, with one condition the
recommendation keeps: the picker stays the ONE place a pairing is made and stays complete for a
first run (who, a world or a server, a kind of home, Enter), so nobody has to visit three pages to
start playing. The new pages deepen it; they never become steps.

## 5. The Characters page relayout (decided; built 2026-11-02)

**What it fixes:** the operator's "The left and right bar areas are small so a lot of text is
crammed vertically." Snapshots before and after, at the app's own restored window, his size, and
4K: `tests/snapshots/picker_small.png` (1280 x 720), `picker_operator.png` (2000 x 1125),
`picker_4k.png` (3840 x 2160); the before pictures are in the commit that added those tests.

**The layout, and why.** A person new to tech choosing who they are and where they go needs to see
all their choices at once and read each in sentences, not columns of three words.

**It is rung 1 of the recommended plan (4d, D1), not something D1 throws away.** The WHERE column
is laid out as the summary the Worlds and Servers pages would deepen: one card per home or world
and one per server (the cards those pages show in full), the selected server's few lines, and
"where you live here" with Choose (the part a first run needs on this screen). What is folded away
today is exactly what moves to those pages when they exist: "Server details" becomes a link to that
server on the Servers page, and a home's card gains a link to its world on the Worlds page. The
kinds of home keep their "What it is like" fold here, because it is part of choosing.

- **The columns are shares of the window**, not fixed widths: WHO a fifth (never under 260 points,
  never over 520), WHERE 0.44 (never under 400, never over 1,280), the stage the rest, and the two
  columns never more than three quarters of the window. At 2000 points that is 400, 880 and 720;
  at his window's real size, 2560 x 1387 (his screenshot was scaled to 2000), 512, 1,126 and 922.
  The character stands in the middle of the stage, not of the window (the camera's lens is
  shifted by the stage's offset, `Camera::lens_shift_x`; seen in the running game, the character
  at the stage's middle about 340 points left of the window's).
- **WHO**: the hint reads in two lines; the characters are a list that scrolls on its own (many
  characters), each with the home it lives in under its name; New character under them; Name and
  Edit look stay at the column's foot.
- **WHERE keeps its three coloured parts** (Your homes red, Open Net green, Closed Net blue: the
  trust language of play-characters.md) but uses the width:
  - the homes and the servers are rows of small cards side by side (a grid, so twenty servers
    wrap into rows rather than one long tower), the selected one edged in the accent;
  - the selected server's card shows the few lines a person choosing needs: its name with a
    Connected pill, its description, and how many people (members, online now, in the world);
    the address, version, channels, the Accord line and the admins' note fold behind
    "Server details";
  - the kinds of home are cards side by side, three across at the operator's size (two on a
    small window): each with its name, how many are free, its first sentence, the household
    choice on the card it belongs to (the Homestead's), Choose, and its own "What it is like" fold
    whose words (the whole sentence, the places shared aboard, food, water and power) open in a
    panel under the row at a readable width; the Apartment greyed with why on its card;
  - the note that a server starts you fresh closes the server's card.
- **Every word by key** (`data/i18n/en.json`, `characters.*` and `where_you_live.*`), every
  control with its hint, theme tokens only.

Nothing that was on the page is lost: every fact moved into a card, a pill or a fold.

**His screen, and the text size on it (2026-11-02).** His display is 2560 x 1440, a 65-inch TV
used as his monitor (the orchestrator, relaying him; the screenshot arrived scaled to 2000 x 1125);
the game's window on it is 2560 x 1387 maximized. The layout test treats 2560 x 1440 as his size
and keeps 2000 x 1125, the app's restored 1280 x 720 and 4K. The text is the other half of
"crammed": a 65-inch screen 2560 pixels wide has pixels about 0.56 mm wide (1,439 mm across),
against 0.23 mm on a 27-inch desk monitor of the same resolution, so the picker's small text (the
theme's `font_size_small`, 11.9 px: an em of 6.7 mm on his screen) seen from 2.5 m looks about 0.58
times the size it does on that desk monitor from 0.6 m (0.72 times from 2 m, 0.48 from 3 m).
Everything on the page is drawn from the theme's sizes, so a scale for the whole app would fix it,
but **the Settings > Appearance "Font Size" slider does nothing today**: `settings.font_size` is
saved and loaded (config.rs) and read nowhere else (2026-11-02). The fix is app-wide (egui's zoom
factor, or the theme's sizes from that setting) and is noted for the operator, not built here. The
picker is ready for it: the layout test also runs at 1707 x 960 points, his screen with everything
half again as large, and the WHERE column still fits without a scroll.

**The website:** it has no Characters page (no game runs in a browser; its Play goes to the
Download page). Its one mirror of a part of this page, the chat's "Where you live" window
(`web/chat/chat-home.js`), already shows the kinds of home as a short list in a centred window, so
it has no narrow-column problem to fix and keeps its shape; noted here so the difference is
written down (dual-UI parity).

## 6. Questions for the operator

1. **The top menu (4a):** A1, A2 or A3? Recommended: A1.
2. **The overview (4b):** B1, B2 or B3? Recommended: B1, built as the bottom-strip plan's rungs 4
   and 5 on the Humanity page.
3. **Play and Characters (4c):** C1, C2 or C3? Recommended: C1.
4. **A Worlds page and a Servers page (4d):** D1, D2 or D3? Recommended: D1, Servers first (it
   gathers three things that exist into one place: the servers you are in, the servers you run,
   and the planned Federation Directory), then Worlds.
5. **The Characters relayout (5)**, built as rung 1 of D1: the shares (a fifth and two fifths) and
   the folds are the parts most open to taste; say if a fact folded away should be on the card.
6. **Text size on the TV (5):** the Font Size setting does nothing yet. Recommended: make it scale
   the whole app (egui's zoom), with a starting value that suits a 65-inch screen from the sofa,
   its own small increment; the picker's layout already holds at half again as large.

**Taken together, the recommendation** is one shape: Humanity is the overview (B1); the game group
is Play (goes, and says where), Characters (the pairing screen, its WHERE column a summary), Worlds
and Servers (the depth) (C1 and D1); and the menu's colours mean one thing each, with the game's
own systems inside the game (A1). Built in rungs: the Characters relayout (done), Servers, Worlds,
Play's words, the menu, the overview.
