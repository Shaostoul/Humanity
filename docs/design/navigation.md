# Navigation: the double top menu, the layers, the six questions, and Back and Forward

**Status (2026-11-03): being built, rung by rung (section 14).** Built so far: the menu's words,
by key (`nav.*`, `front_door.q_*` in `data/i18n/en.json`); **rung 1: the menu as data,
`data/gui/navigation.json`**, which both apps draw from; and **rung 2: the double menu on the
desktop** (the two rows, the preview, More, the sizes and colours as theme tokens, Back and
Forward drawn, Play's page with its tap and hold, the Humanity and Dev side lists become their
rows). The website keeps its single row until rung 5. The throwaway mock-up that drew this
design's pictures (section 15) was deleted with rung 2. The questions that are his are in section
16, each with a recommendation; his answers are 16.1.

It covers both apps, the desktop app first and the website mirroring it: the two menus and every
page placed in them (section 3), how they behave under a mouse, a keyboard and a thumb (4), the
three content layouts (5), one layer order for everything on screen (6), their sizes on his two
screens (7), the six question pages (8), the parity audit of the two apps (9), Back and Forward
(10), the 3D game (11), the keyboard and screen-reader paths (12), the menu as data (13), the rungs
to build it (14), and the mock-ups (15).

**Why a new document, not a rewrite of [top-menu-and-game-pages.md](top-menu-and-game-pages.md):**
that document's built part (section 5, the Characters relayout) and its findings (sections 1 to 3,
the inventory in 4d) stay true and are cited from `docs/PAGES.md` and `docs/FEATURES.md`; only its
section 4 options are overtaken, and its top now says so. This one is about the frame of the whole
app on both surfaces, and it is the design `docs/reference/ui-navigation-map.md` will follow once
built.

## 0. The operator's words (2026-11-02)

The double menu:

> The more we develop the more I think the going to a single top menu might've been a mistake.
> Previously we had a double top menu. Like the "primary top menu bar" is parent categories. Still
> pages in of themselves but, by going to that top page or hovering over that top menu button would
> popup the secondary top menu. I guess this means each page would have like 4 sections. Primary
> top menu. Secondary top menu. Page content. Collapsible footer featuring chat. Then page content
> would have like a few different modes. Primarily there'd be single column content mode,
> left/right side content mode, and left/middle/right side content mode. I don't know what the
> proper terminology is but, I hope that makes sense. The primary top menu would be on top of
> everything, we'll say layer 10. Then the second top menu is layer 20 underneath the primary. Then
> footer is 30. Then page content is 40+. That way top menus and footer are always sitting on top
> of page content and always visible. I don't know how things are now. The developer warning about
> the software being not released would probably be layer 21 since we don't want the footer
> covering it.

Full-screen popups, his answer to "one layer above the menus, for urgent things only (an incoming
call, an emergency alert); tooltips above everything":

> I guess when a full screen popup happens. The click to exit background dark shading for the page
> under the popup is like, layer 1, then the static popup window is layer 2, then any interactive
> bit are layer 3. This way it hides/shades the top menu buttons too.

The six questions:

> Do we have all of those pages? Who, what, when, where, why, how? We should have all of those.
> Then we could have those on the landing page. Each card would take someone to the page.

His answer to "the desktop app has no Roadmap or Devlog page":

> We should add the pages. We want to make sure the app provides the holistic HumanityOS
> experience. We can't leave anything out. Everything needs to be able to be done through the app.
> I guess you could say we're making a master control panel but, it's much  more than just a MCP.
> It's not just controls.

Back and forward:

> We need to make it so the back button on the mouse can take the user back to where they were like
> the internet browser.

The same day he asked for the chat channel #general to be renamed #global (it is the channel that
federates), for a #feedback channel, and for #local to show as an ordinary channel row instead of
behind a click on the server's name. Another agent builds that; the footer chat here is drawn with
those names.

His screens: a 65-inch TV used as his monitor at 2560 x 1440, 100% scaling, about 4 ft away at a
sit/stand desk; a OnePlus 9 Pro phone, 3216 x 1440 at 525 PPI, 20.1:9 (a CSS viewport of about
412 x 915).

## 1. What this decides, and what it replaces

- **The menu becomes two rows:** a primary row of eight categories, each a page of its own, and
  under it a secondary row with that category's other pages. Section 4 of
  top-menu-and-game-pages.md (A1 to A3, B1 to B3, C1 to C3, D1 to D3) is overtaken: this design
  takes A1's grouping as its categories, B1 (the Humanity page is the overview), C1 (Play goes in
  at once and says where) and D1 (a Worlds page and a Servers page, the picker their summary).
- **One layer order** for both apps, in his numbering (section 6), replacing today's two unrelated
  orders (the desktop's egui orders and the website's hundred-odd `z-index` literals).
- **A real history** on the desktop (section 10): Back and Forward, the mouse's side buttons,
  Alt+Left and Alt+Right, with Escape kept for leaving and closing; and the website's in-page views
  given the same steps.
- **Six question pages** (section 8): Who, What, When, Where, Why, How, each one page, carded on the
  website's front door and on the Humanity overview from one data file.

## 2. Today (read 2026-11-02, against main at 92c83658d)

### 2.1 The single row

`src/gui/pages/escape_menu.rs` draws one row of 19 buttons in 13 groups (Play, Characters, Apps;
Humanity; Chat; Studio, Watch, Files; Profile, Home; Quests; Tasks; Inventory; Crafting; Maps; Dev;
Library, How-to; Settings), then the Aa cycle, with the notices bell and help anchored at the
window's right. It wraps to a second row under about 1,500 points. The buttons are 28 points tall
with 11-point words, written as constants in the file (`BUTTON_H`, `FontId::proportional(11.0)`),
not theme tokens. The website's `web/shared/shell.js` writes the same row again by hand
(`navTab(...)` calls: 17 tabs after a front-door link, Onboarding where the desktop has Quests,
Download for Play) and a phone drawer listing 39 links in four groups. Two hand-written copies of one menu: they have
drifted before (ui-navigation-map.md, open item 9).

Many pages the operator uses are not in the row at all: Notes, Calendar, Calculator and Browser
are tiles on the Apps page; Governance, Laws, the Directory and Donate are the Humanity page's side
list; Wallet, Market, Trade and Guilds are the Profile page's side list; the Servers you run open
from the chat's sidebar; Bugs, Testing, Performance, Planet Tuner, Claude and Journal are the Dev
page's side list. The website has 42 pages and no Roadmap, Devlog, Rules or Civilization entry
outside the phone drawer.

### 2.2 The layers today

**Desktop (egui).** The top menu, the unfinished-software warning and every page's side and central
panels are egui panels: they never overlap, because each panel takes its room and the next one
gets what is left (lib.rs adds the menu, then `bottom_strip::draw`, then the page). Over the panels:
the themed dialogs (`widgets/dialog.rs`) put a full-screen dimming backdrop in `Order::Middle` and
their window just above it (`set_sublayer`), so **a dialog already shades the top menu** and its
window is kept above the warning (`bottom_strip::room_above`). The notices panel, the help panel,
the toasts, the chat's menus and the HUD are `Order::Foreground`, so the bell's panel would draw
**over** an open dialog's shade; tooltips are `Order::Tooltip`.

**Website.** `shell.js` puts the menu at `z-index: 5500`, the footer at 2100, the warning at 2150
(over the footer, which slides behind it when folded: his "layer 21" already holds there), the phone
menu at 7600 to 7700, tooltips at 9000, help at 9000 to 10050. The pages' own popups are scattered:
the Tasks, Trade, Wallet and Dev pages' modals are at 1000, **under** the menu, and stop at the
warning (`inset: 0 0 var(--hos-wip-h) 0`), so the menu stays bright and clickable over an open
popup; the chat's and Settings' modals are at 6000 to 10000, over it; the press-and-hold confirm and
the glossary card are at 100000. In all, 115 `z-index` literals in 45 distinct values, and no scale.
So the two apps disagree, and the website disagrees with itself, on whether a popup covers the menu.

### 2.3 History today

The desktop keeps `GuiState::nav_back_stack` (`src/gui/mod.rs`): `push_nav_to` pushes the current
page when a page opens another from inside it (25 call sites: the chat's cog to Server Settings, a
notice to its page, Open in the Library, ...), a top-menu click empties it (`clear_nav_back`), and
Escape pops it (`lib.rs`, the Escape handler) or, when it is empty, drops into the world. About 40
other places set `active_page` directly and bypass it. There is no Forward, nothing reads the
mouse's side buttons, and nothing smaller than a page is remembered: a Settings section, a chat
channel or a Files place is not a step. Two pages keep a history of their own: the Library's reader
(`library.rs`, a `back` list of documents) and the readable web view (`widgets/web_view.rs`, back
and forward).

In egui 0.31 the mouse's side buttons are `PointerButton::Extra1` and `Extra2`; egui-winit maps
winit's `MouseButton::Back` to `Extra1` and `Forward` to `Extra2`, and winit on Windows maps
`WM_XBUTTONDOWN` with `XBUTTON1` to `Back` and `XBUTTON2` to `Forward` (read in the crates'
sources, egui-winit 0.31.1 `lib.rs` and winit 0.30.13 `platform_impl/windows/event_loop.rs`). The
game's bindings (`data/input.toml`) use only `Mouse0` and `Mouse1`; nothing in the code reads
`Extra1`, `Extra2` or winit's `Back` and `Forward`. One trap: egui's text box moves the cursor a
word left or right on **Alt+Left and Alt+Right** as well as Ctrl (`text_selection/cursor_range.rs`:
"alt on mac, ctrl on windows"), so those keys can only mean Back and Forward while no text box has
the keyboard.

The website: each page is its own address, so the browser's Back works between pages. Inside a page
it mostly does not: the chat's channels, conversations and groups push nothing (`web/chat/app.js`
only cleans its `#profile` and `#joined` marks with `replaceState`); the Files page's places use
`replaceState` (an opened item does push `/files/<id>`); the Settings sections, the Maps views, the
Profile sections and the Market's tabs have no address at all; the How-to page uses `replaceState`
for `#<id>`. The Library's documents already push (`library-app.js`), and the Accord and the Dev
page's Claude tab push through their `#` addresses.

### 2.4 The double menu we had, and why it went

**What it was.** v0.164.0 added it as an opt-in preview, v0.174.0 made it the default, v0.196.0
(2026-05-08) removed it. The top row was four or five wide category tabs, **Reality** (red), **Sim**
(purple), **Tools** (blue), **Settings** (grey) and, during development, **Dev** (amber), each 120 x
36 points, with a brand "H" before them; the second row, always shown, listed the active
category's pages (at v0.164.0 Reality held fourteen, Sim five, Tools seven and Settings four; by
v0.195.0 Reality eleven, from Profile, Chat and Wallet to Civilization, Governance and Identity,
Sim five, Tools six, Settings fourteen and Dev five), with an RGB line between the rows and under
them. It replaced the Real/Sim switch: the
point was that real money and game money could not be confused. v0.181.0 then made each top tab
open an **overview landing page**, a grid of cards repeating its second row with a line each, five
such pages (`OverviewReality` to `OverviewDev`), and the twelve Settings sections became pages of
their own so the Settings landing could show them as cards.

**Why it went.** The operator, on a screenshot (v0.196.0's commit): "I think we should remove the
two row main menu... The single row main menu is cleaner." And, recorded in `escape_menu.rs`: "I
really want to not need so many pages..." The release note says the operator decided "we should
reduce total page count rather than add nav layers". On 2026-07-04 (v0.699.0) the five landing pages
and the twelve Settings variants were deleted as dead code, unreachable since v0.196. (Earlier
still, in 2026-03, a "Private | H | Public" header with drop-downs was proposed and never built:
`docs/history/nav-proposal-*.md`.)

**The reasons in that evidence, and this design's answer to each:**

1. *The categories were kinds, not things.* Reality, Sim and Tools say what sort of page is behind
   them, not what is there; finding Tasks meant reading the second row. **Here every category is
   named for a page people know** (Humanity, Play, Chat, Home, Apps, Library, Dev, Settings), and
   its button opens that page.
2. *The menu made pages of its own.* The five landing pages were card grids repeating the second
   row, and twelve Settings pages existed only to be their cards: seventeen pages for the menu's
   sake, the "so many pages" he did not want. **Here the menu adds no page:** a category's page is
   one of the app's real pages (the Humanity page is the overview he asked it to be, Chat is the
   chat, Play is the world), and nothing exists only to list other things.
3. *The rows were uneven.* Reality's fourteen pages beside Sim's five made one crowded row and one
   bare one. **Here a row holds one to eight pages**, grouped by what a person does there.
4. *One row fit then.* In May about fifteen buttons fit one row on his screen. **Today it is 19
   buttons that wrap under 1,500 points**, and the operator now wants more pages in the app, not
   fewer ("We can't leave anything out. Everything needs to be able to be done through the app."):
   Roadmap, Devlog, People, Rules, Worlds and Servers, and Notes, Calendar, Governance, Wallet and
   the rest brought out from the side lists they hide in. One row cannot carry 44 destinations; two
   rows can, with a category's pages one hover away (section 4), so for a mouse the second level
   costs no extra click.

## 3. The menus: eight categories, every page placed

Each category is a page of its own, its button labelled with that page's name (the naming rule,
ui-navigation-map.md: "The nav button label is the page's name"). The secondary row lists the
category's **other** pages; the category's own page is its primary button, so no page is listed
twice and no label stands for two pages. The colours are one per category (new theme tokens
`nav_cat_<id>`, section 13), never alone: every button carries its words.

| # | Category: its page | Colour (default) | Secondary row (the category's other pages) |
|---|---|---|---|
| 1 | **Humanity**: the overview (the bands You, Your character and Us; the six question cards; the latest updates) | red (`nav_reality`) | Mission · People *(new)* · Governance · Laws · Rules *(new on the desktop)* · Donate · Roadmap *(new on the desktop)* · Devlog *(new on the desktop)* |
| 2 | **Play**: the world, first person, with the character and world played last (its hint names them, C1) | purple (`nav_sim`) | Characters · Worlds *(new)* · Servers *(new)* · Quests · Inventory · Crafting |
| 3 | **Chat**: the chat | cyan (`section_cyan`) | Watch · Studio · Files · Guilds |
| 4 | **Home**: your home and your real life (two-realities.md: "Home = your real life") | green (`nav_legacy_green`) | Profile · Tasks · Maps · Wallet · Market · Trade |
| 5 | **Apps**: every program in one place | blue (`nav_tools`) | Notes · Calendar · Calculator · Browser |
| 6 | **Library**: everything to read | magenta (`section_magenta`) | How-to |
| 7 | **Dev**: the developer's tools (the creature spawner and friends, today's Dev section) | amber (`nav_dev`) | Bugs · Testing · Performance · Planet Tuner · Claude · Journal |
| 8 | **Settings**: this app's settings, with their side list of sections | grey (`nav_settings`) | Server settings |

**Every page today, and where it goes (none lost, none twice):**

- *Top-menu buttons today:* Play, Characters, Apps, Humanity, Chat, Studio, Watch, Files, Profile,
  Home, Quests, Tasks, Inventory, Crafting, Maps, Dev, Library, How-to, Settings: all placed above.
  Inventory, Crafting and Quests leave the primary row (two-realities.md: the game's own systems
  are reached from inside the world) but stay one hover away under Play, beside the I key and the
  overview's "Your character" band; nothing a person reaches today is lost.
- *Hidden in side lists or tiles today:* the Humanity page's Mission Dashboard (now **Mission**),
  Governance, Laws, Directory (now part of **People**, "Look someone up") and Donate; the Profile
  page's Wallet, Market, Trade and Guilds (out of Profile, into the Home and Chat rows; Profile
  keeps its own sections: Body and Measurements, Identity, Private Notes, Network Profile,
  Interests, Skills, Social Links, Streaming); the Apps page's Notes, Calendar, Calculator and
  Browser tiles (still tiles too: the Apps page lists every tool); the Dev page's sections (each a
  page in the Dev row); Server Settings (from the chat's cog today; a Settings item now, the cog
  still opens it); the Servers you run (`RelayControl`, from the chat's sidebar today: the Servers
  page's "Yours" part, as 4d's D1 planned).
- *Not in a menu, by design:* the title screen (at launch), the in-world screens, the vendor and
  construction overlays (the world's), and Settings' sixteen sections (its side list: one page, one
  search, one save).
- *New:* People, Worlds, Servers, and on the desktop Roadmap, Devlog and Rules (section 9 says
  why each).

**Why these eight.** They are A1's seven groups with Apps on its own: the collective (Humanity), the
game (Play), talking and sharing (Chat), your own life (Home), the programs (Apps), reading
(Library), the developer (Dev) and the settings. Real and Sim stay apart by navigation, as
two-realities.md requires: the game's pages are all under Play, your real life under Home.

**Two names are his to settle (section 16):** Home or Profile for category 4 (recommended: Home,
two-realities.md's word; the website's front-door link stops being a house so there are not two),
and whether Rules joins Laws later as its innermost "this server" level.

**On the website** the rows are the same file (section 13). A desktop-only page (Characters,
Worlds, Studio, Quests, Testing, Performance, Planet Tuner, Journal) shows in the website's row
dimmed with "In the desktop app", the way the website's Apps page already marks Quests and Studio,
and opens `/download`: the difference is visible, never silent. The other way round, the
website's Dev row also holds its two pages of its own, Ops and the Widget Playground, and its
Settings row's Server settings opens `/admin`. The website's Play opens
`/download`, as today. The website keeps its front-door link at the row's left end, drawn as the
site's name instead of a house.

## 4. Hover, click and touch

### 4.1 The desktop app, and the website under a mouse

- **The secondary row is always there**, under the primary row, showing the pages of the category
  of the page on screen; its item for the page on screen is marked (a bar in the category's colour
  and its words in `text_strong`), and the primary button of its category is marked the same way,
  its colour running down into the row.
- **Click a primary button:** its page opens and its row docks. **Click a secondary item:** that
  page opens. (Play's click goes into the world, as today, saying where in its hint.)
- **Rest the pointer on another primary button for 200 ms:** the secondary row shows that
  category's pages instead, a preview, tinted in its colour, in the same row and at the same height,
  so nothing on the page moves or is covered (the row is layer 20 whatever it shows). The pointer
  can travel down into the row and click one. The preview stays while the pointer is on either row,
  and the docked row comes back 300 ms after the pointer leaves both, or at once on Escape. The
  200 ms is what stops a pointer crossing the row on its way down from flicking through every
  category (the "diagonal" problem of hover menus).
- **The timings are data** (`data/gui/navigation.json`, section 13), and **Settings > Appearance
  gets "Show a category's pages when the pointer rests on it"** (on by default): a person who finds
  hover menus jumpy turns it off and the row changes only on a click.
- **Too narrow for a row** (a small window, or a large text size, section 7): the row shows what
  fits in order and puts the rest under **More** at its end (the "priority plus" pattern), never a
  third wrapped row.

### 4.2 Touch (the phone, and a touch screen on the desktop)

There is no hover before a tap, so there is no preview:

- **Tap a primary button:** its page opens with its row. A category's other pages are then one tap
  away; reaching a page in another category is two taps, the same as two clicks.
- **On the phone** (section 7) the primary row is icons over short words, 64 points each; eight do
  not fit 412 points, so the row scrolls sideways under a thumb with the page's category kept in
  view and a fade at the edge saying it goes on. The secondary row is a sideways-scrolling row of
  chips. **All pages** (today's phone menu, the hamburger) stays pinned at the primary row's right
  end: a full-screen list of every category with its pages, from the same file, for finding
  anything by reading.

## 5. The three content layouts

His words were "single column content mode, left/right side content mode, and left/middle/right
side content mode". Plainly, **one column**, **two columns** and **three columns**:

| Layout | What it is | Pages |
|---|---|---|
| **One column** | One readable column, centred, at most a reading width (the website's standard 1,100 px column; a token `layout_reading_max` on the desktop), cards or a grid inside it as wide as the column | Humanity (the overview), Mission, People, Governance, Laws, Rules, Donate, Roadmap, Devlog, Worlds, Home, Tasks (its board), Quests, Calendar, Calculator, Wallet, Trade, Watch, Browser, Bugs, Testing, Performance, Planet Tuner, Claude, Journal, Dev |
| **Two columns** | A side list (or a filter rail) beside the content | Settings (its sections), Profile (its sections), Server settings (USER, MOD, ADMIN), Library (its rail and the reader), Notes (the list and the note), Market (its filters and the grid), Inventory (the grid and the equipment), Guilds (the list and the guild), Servers (the list and the server), Apps (the parts and the details) |
| **Three columns** | A list, the thing chosen, and its details or members | Chat (the conversations, the messages, the members), Files (the list, the item, its details), How-to (the rail, the list, the how-to), Characters (who, the stage, where), Maps (the bodies, the map, the details), Crafting (the recipes, the recipe, the queue), Studio (the scenes, the preview, the properties) |

- **What changes:** the Humanity and Dev pages go from two columns to one, because their side lists
  become secondary rows (their items are pages: the website already has `/governance`, `/laws`,
  `/donate`, `/bugs` as pages). Profile and Settings keep theirs: those are one page's parts.
- **One widget each** (`widgets::layout::{one_column, two_columns, three_columns}`, with matching
  CSS classes `.layout-1`, `.layout-2`, `.layout-3`), the columns as shares of the width with a
  narrowest and widest each (theme tokens), the way the Characters relayout and
  `widgets::panes` already do it, so every page of a kind behaves the same.
- **When the window is too narrow** for its columns (the phone; a small window; a large text
  size): three columns show the list alone, and choosing an item shows the item with its details
  under it, a step Back undoes (section 10); two columns show the side list as its own screen
  first. So on the phone every page is one column at a time, and the phone's own back gesture walks
  back up.

## 6. One layer order

His two answers count the same way at the top level: lower is nearer the viewer (the menus at 10
and 20 sit on the page at 40 and up). His popup numbers 1, 2 and 3 all come before 10, which is
what makes a popup cover the menus ("This way it hides/shades the top menu buttons too"). Inside
the popup, though, he numbered in the order its parts are made (first the shade, then the window,
then its controls), which read with "lower is nearer" would put the shade on top of the window and
swallow its clicks. What he described is the window on the shade and the controls on the window,
so the one order below keeps his numbers' meaning everywhere and renumbers the popup's three parts
so that there too, lower is nearer.

**The order, nearest the viewer first:**

| Layer | What | Notes |
|---|---|---|
| **1** | Tooltips, hints and toasts | They never take a click. A hint on a popup's button shows over the popup. |
| **2** | Urgent: an incoming call, an emergency alert | Over an open popup (recommended, section 16), in its own band at the top centre: it never takes the popup's typing, never dims anything, and is answered by its own buttons. |
| **3** | A full-screen popup's controls | Its buttons, boxes and the drop-down lists inside it. |
| **4** | The popup's window | |
| **5** | The popup's shade | Dims and blocks everything farther away: the menus, the warning, the footer and the page. A click on it closes the popup when the popup may be closed that way. The window itself still stands clear of the warning's line, as the desktop's dialogs already do. |
| **9** | The primary row's own drop-downs | The notices panel, the help panel, More. |
| **10** | **The primary menu** | Always visible on every page. |
| **19** | The secondary row's More | |
| **20** | **The secondary menu** | Its hover preview never leaves its row. |
| **21** | **The unfinished-software warning** | At the bottom; nearer than the footer, so the footer never covers it (it slides behind it when folded, as the website's already does). |
| **30** | **The footer chat** | Folded or unfolded, above the warning on screen. |
| **40** | The page's drop-downs, context menus and its windows that are not full-screen popups | They stay inside the page's area: the menus and the footer are nearer, so a drop-down near the bottom opens upward instead of under the footer. |
| **41+** | **The page** | Its columns and content. |

In the first-person world there are no menus, no footer and no warning (section 11); the order
there is the same from the top: 1 (tooltips and toasts), 2 (urgent), 3 to 5 (a popup over the
world, such as the vendor or the death screen), then the HUD and the in-world chat panel, then the
world.

**How each app holds it:**

- **Desktop.** 10, 20, 21, 30 and 41+ are egui panels, which never overlap: each takes its room, so
  "always visible" holds by construction (lib.rs adds the primary row, the secondary row, the
  warning, the footer, then the page's panels). The rest map onto egui's orders: 1 is
  `Order::Tooltip` (the toasts move there from `Foreground`; they are not interactive), 2 is
  `Order::Foreground`, raised to its top each frame, and 3 to 5 and 9, 19 and 40 are
  `Order::Middle`, ordered each frame by one function in a new `src/gui/layers.rs`: the page's
  windows lowest, then the menus' drop-downs, then any popup's shade with its window above it
  (`set_sublayer`, as `dialog.rs` does now). The notices and help panels move from `Foreground` to
  `Middle`, below a popup's shade, where his order puts them. A headless test opens a dialog, the
  notices panel and an urgent card together and reads egui's layer order.
- **Website.** One scale of custom properties in `theme.css`, generated like the colours, from the
  same table (`--z-page-float: 100` for 40, `--z-footer: 200`, `--z-warning: 210`, `--z-menu-2:
  300`, `--z-menu-2-drop: 310`, `--z-menu-1: 400`, `--z-menu-1-drop: 410`, `--z-shade: 1000`,
  `--z-popup: 1010`, `--z-urgent: 2000`, `--z-tooltip: 3000`), and a web test that fails on a
  `z-index` written as a number anywhere else (a ratchet: today's 115 literals each move onto
  the scale as their page is touched). The modals at 1000 that leave the menu bright, and stop at
  the warning, change to cover the whole window: the room test (`wip-strip-room-web.test.js`)
  already allows "a layer over the whole window above the strip", which hides the warning while it
  is open.
- **On the phone**, a full-screen popup also pushes a history step without changing the address,
  so the phone's back gesture closes the popup instead of leaving the page (section 10).

## 7. Sizes on his two screens

**His TV at 4 ft.** A 65-inch 16:9 panel is 1,439 mm wide, so at 2560 x 1440 a pixel is 0.562 mm.
At 1,219 mm (4 ft) one pixel spans 1.28 times the angle a pixel of a 27-inch 2560 x 1440 desk
monitor (0.233 mm) spans at 65 cm: **everything on his screen looks about 1.3 times as large as it
would on that monitor at a desk.** (The earlier designs measured from a sofa at 2.5 m; at a desk
at 4 ft the picture is kinder.) So:

| Part | Today | Proposed (token) | On his screen | Looks like, on a 27-inch desk monitor |
|---|---|---|---|---|
| Primary row | 28-point buttons, 11-point words | 48-point row, 36-point buttons, words at `font_size_body` (16.5), icons 18 (`nav_primary_height`, `nav_primary_button_height`, `nav_icon_size`) | words with a 6.5 mm capital, 18 minutes of arc | 21-pixel words |
| Secondary row | none | 40-point row, 30-point buttons, 14.5-point words (`nav_secondary_height`, `nav_secondary_button_height`, `nav_secondary_font_size`) | a 5.7 mm capital, 16 minutes of arc | 19-pixel words |
| Both rows | 37 points | 88 points | 6.1% of 1,440 | |

Today's 11-point menu words look like 14-pixel words on that monitor: small, which is part of why
the row is hard to read across the room. At the app's restored window (1280 x 720) the two rows are
12% of the height. Measured in that picture, Back, Forward and the eight primary buttons with their
words take about 870 points and the right end's three buttons 110 more, and the longest secondary
row (Humanity's eight) about 780, so both rows fit on one line each from about 1,000 points up,
and More takes over below that (pictures: section 15).

**His phone.** One CSS pixel is 3.5 device pixels at 525 PPI, 0.169 mm. Held at about 30 cm:

| Part | Proposed | Measured |
|---|---|---|
| Primary row | 60 points tall; buttons 64 x 52, an icon of 22 over an 11.5-point word | a button 10.8 x 8.8 mm, above Android's 48 dp touch minimum (about 7.6 mm) and WCAG 2.5.8's 24-point minimum |
| Secondary row | 48 points tall; chips 40 tall, 14.5-point words | a chip 6.8 mm tall with its row's spacing, words with a 1.7 mm capital (20 minutes of arc) |
| Both rows | 108 points | 11.8% of 915 |

The website's warning wraps to about four lines on the phone (about 75 points), so the frame takes
about a fifth of the screen.
His rule is that the menus are always visible; whether the two rows may slide away while a page
scrolls down on the phone, and come back at the first scroll up, is a question for him (section
16; recommended: keep his rule, it is one switch to change).

**What the design needs from BUG-225** (the Font Size setting does nothing; it is fixed separately,
after this):

1. **One scale for the whole app** (egui's zoom factor, or the theme's sizes from the setting), set
   before the menus measure themselves, so the menus grow with everything else. Their sizes are
   tokens in points (section 13), never pixels, so the scale reaches them.
2. **A row that no longer fits goes to More** (section 4.1), never to a third row. At 200% on his
   screen the app is 1,280 points wide: both rows still fit with their words.
3. **The website's menu must follow its text size too.** `shell.js` fixes it today ("font-size:
   15px !important; /* fixed so global font-size slider doesn't break nav */"); with More in place
   the menu no longer breaks, so the fixing goes (WCAG 1.4.4, resize text).
4. **A range of at least 100% to 200%**, and a test that renders both rows at 150% and 200%
   without a third row.

## 8. The six question pages

Each question is one page, on both apps. Three already exist (What, How, and on the desktop Why);
three are new (Who, Where, and on the desktop When).

| Question | The page | Today | In the menus | Its card (`front_door.q_*`) | The card's second link |
|---|---|---|---|---|---|
| **Who** | **People** *(new, both)* | No page lists the people on a server or who builds HumanityOS | Humanity > People | "The people here, and the people who build HumanityOS" | Chat |
| **What** | **Apps** | Both (`apps.rs`, `/apps`) | Apps | "Every tool in HumanityOS, and the game" | The game |
| **When** | **Roadmap** | Website only (`/roadmap`); desktop **new** | Humanity > Roadmap | "What changed lately, and what is being built next" | Devlog |
| **Where** | **Servers** *(new, both)* | The chat's server list; RelayControl; no public list | Play > Servers | "The servers people meet on, and how to find more" | Maps |
| **Why** | **Mission** | Desktop: the Humanity page's Mission Dashboard; website: `/mission` forwards to a Library essay | Humanity > Mission | "Why HumanityOS exists: to end poverty and unite humanity" | Decide together (Governance) |
| **How** | **How-to** | Both (`how_to.rs`, `/how-to`) | Library > How-to | "Step-by-step how-tos for real life and for this app" | The Library |

**What each new page holds:**

- **People (Who).** The people on this server: the counts for everyone (members, online now, in
  the world, from `/api/server-info`), and to a signed-in member the member list the chat already
  shows that member, from the same source, each with their role and with AI participants listed and
  labelled as AI (the mission counts every human and every AI). Who looks after this server: its
  admins and moderators and what each can do, in the words the Rules page publishes (one source).
  Look someone up: the Directory (today's Identity page: a key, credentials, the trust score).
  People you follow (follow and unfollow, never "friend"). Who builds HumanityOS: `CREDITS.md`,
  moved here from Settings > Credits, whose own rule holds ("Everyone named here asked to be
  here"). **Safe by default:** the page adds no list a reader could not already see; a visitor to
  the website who is not signed in sees the counts, the moderators and the credits, never a member
  list.
- **Servers (Where).** 4d's D1 page: the servers you are in (status, people, description, the
  Accord, your home there), the servers you run (today's RelayControl, desktop only: SSH and a node
  on this PC), and Find more, the Federation Directory planned in federation-ux.md, each server
  labelled with whose list it came from, added only by your own click, with the plain words about
  what a stranger's server will and will not have. The website shows this server, the servers it
  links to (`/api/federation/servers`, which its chat already reads) and how to join one.
- **Roadmap (When), on the desktop.** `data/roadmap.json`, the file the website's page renders
  (held together by a `page_parity_lint` pair), its tiers drawn the same way; the newest three
  update cards at its top with "Every update", which opens the Devlog.
- **Devlog, on the desktop.** The update cards (the front door's rung 5 widget,
  `widgets/update_card.rs`, the signed dressing included) from the connected server's
  `/api/releases`, a page at a time; once the release notes move onto our own site, a card opens
  its notes in the app instead of on GitHub.
- **Mission (Why), on the website.** `/mission` becomes the desktop Mission page's mirror, every
  word by key, the Library's mission essay linked from the same manifest, and the live "Where we
  stand" numbers, which absorbs `/civilization` (the desktop merged it in v0.1147). This reverses
  the forwarder's "do not regrow" note, whose reason was a second copy of the essay; this page
  holds no copy (question 7).

**Could the six questions be the primary menu's categories? No.**

1. A menu is used every day to do things: open my tasks, go to the chat. Its words must name those
   things. The questions name what a newcomer wants to know once.
2. Every page would need a question, and most fit several or none: Tasks is what, when and how;
   Chat is who and where; Settings and Dev are none. The placement would be arbitrary, which is
   exactly what sank the abstract Reality, Sim and Tools categories (section 2.4).
3. A button labelled "Who" opening a page called People gives one thing two names, which the
   naming rule forbids.

So the questions are **the overview's table of contents, not the menu**: three of their pages live
in the Humanity row (Mission, People, Roadmap), and the overview carries all six as cards.

**The cards.** `data/front_door/destinations.json` gains `question` and `also` on its rows, and rows
for People, Servers, Devlog and Maps. On the **website's front door** the six question cards
replace the eight buttons: each card opens its page and carries one second link, so every old
destination stays one click away (How-to guides and the Library under How; the game under What;
Chat under Who; Everyday tools as What; Why this exists as Why; Decide together under Why; What is
being built as When). Start here stays the one primary button. On the **Humanity overview** the
same six cards sit under the You, Your character and Us bands (the person first; a newcomer's
questions after). Three across on his screen, two across on the phone. Pictures: section 15.

## 9. The parity audit (c2)

Everything a person can do must be doable in the app; where a page is on one side only, the table
says where it goes and why (CLAUDE.md, "Dual-UI parity": a gap is documented, never silent). The
42 website pages and the desktop's 35 pages and their sections were read against
`docs/PAGES.md`, `src/gui/mod.rs` (`GuiPage`, 36 variants with `None`) and `web/pages/`.

**Website pages with no desktop page:**

| Website page | What it is | In the new menus | Verdict |
|---|---|---|---|
| `/` (index) | The front door: a title screen with tools in it | The site's name at the primary row's left | One-sided by shape (a website's front page). Its parts are shared: the six question cards and the update cards are on the desktop's Humanity overview, from one file. |
| `/roadmap` | The public roadmap | Humanity > Roadmap | **Build on the desktop** (his answer, c2). |
| `/devlog` | Every release as a card | Humanity > Devlog | **Build on the desktop** (c2), carrying the full release notes once they move onto our site. |
| `/rules` | This server's rules, who moderates, how to report and appeal | Humanity > Rules | **Build on the desktop**: the Accord requires a space to publish its rules before anyone takes part, and the desktop's people meet the same server. |
| `/mission` | A forwarder to the Library's mission essay | Humanity > Mission | **Build on the website**: the desktop's Mission page's mirror (section 8). |
| `/civilization` | The community dashboard | none | **Merge on the website** into Mission's "Where we stand" and the overview's Us band, as the desktop did in v0.1147. |
| `/wallet-guide` | The Wallet's "?" help page | none | **Move into the help system** on both apps (a help topic and its Library page), then retire. |
| `/onboarding` | The learn-by-doing chains as quest cards | none: Start here on the front door opens it | One-sided until the chains become how-tos (how-to.md, increment 6); the desktop shows them as the Tasks page's suggested cards. |
| `/ops` | Operations and monitoring | Dev > Ops (the website's row) | One-sided for now; its desktop twin is the Servers page's Yours part (health, actions, the console). |
| `/download` | Download the desktop app | Play (the website's Play opens it) | One-sided by nature: the desktop app is what it downloads; its twin is Settings > Updates. |
| `/hi` | The printed card's QR address | none | One-sided by nature: a card's address, never in a menu. |

**Website pages whose desktop twin has another name or shape** (both exist; listed so nothing
reads as a gap): `/admin` is Server Settings > ADMIN (one name for both: question 11); `/web` is the
Browser page (one name for both: question 11); `/accord` is the Library's Accord shelf (the
permalink stays the website's, an address to share); `/identity` is the Humanity page's Directory
(both become People's Look someone up); `/recovery` is Settings > Account > Recovery through
trusted people; `/widgets` is Settings > Widgets' test bench (the website lists it under Dev);
`/humanity` is the Humanity page (the overview on both); `/chat/read` (built 2026-11-02,
chat-access.md) is the chat for a visitor with no key, which the desktop never is after its setup:
the website's Chat opens it for such a visitor, and its channels are steps like the chat's.

**Desktop pages with no website page:**

| Desktop page | In the new menus | Verdict |
|---|---|---|
| The title screen (`MainMenu`) | none (at launch) | One-sided by nature; the website's twin is its front door. |
| The world (Play) | Play | No game runs in a browser: the website's Play opens `/download`. |
| Characters (the picker) | Play > Characters | Desktop only by nature. |
| Worlds *(new)* | Play > Worlds | Desktop only by nature: a browser holds no saves. |
| Quests | Play > Quests | No game in a browser (the website's `/quests` goes to `/onboarding`). |
| Studio | Chat > Studio | One-sided (operator, 2026-08-17): broadcasting is the desktop's; the website watches (Watch). |
| Dev > Testing, Performance, Dev, Planet Tuner, Journal | the Dev row | One-sided by nature (they work on the running game); the website's Dev page lists them as the desktop's. |
| Server Settings > USER and MOD | Settings > Server settings | **A gap:** the website's `/admin` holds the ADMIN part only. Its USER and MOD parts belong on the same website page, for a member and a moderator. |
| Settings > Credits | Humanity > People | **A gap:** moves to People, which both apps have. |
| Parts of pages: Apps' Games and Added by you; Files' Your folders; the Servers page's Yours | their pages | One-sided parts: a website cannot see what is installed or run anything on the computer (operator, 2026-10-13, for Your folders). |

**In all: 11 website pages have no desktop page.** Six get a twin or a merge (Roadmap, Devlog and
Rules on the desktop; Mission on the website; Civilization merged; the Wallet guide into help) and
five stay one-sided (the front door, Download and `/hi` by their nature, Onboarding until its
chains become how-tos, Ops for now). **11 desktop pages have no website page** (the title screen,
the world, Characters, Worlds, Quests, Studio, and the Dev row's Testing, Performance, Dev, Planet
Tuner and Journal), all by nature: no game, no saves and no broadcast in a browser. **Two parts
are gaps** on the website: Server Settings' USER and MOD parts, and the Credits.

**To build:** on the desktop, **Roadmap, Devlog and Rules**; on the website, **Mission** (and the
Civilization merge, the Wallet guide into help, Server Settings' USER and MOD on `/admin`); on both,
**People** (with Credits and the Directory in it) and **Servers**; and Worlds on the desktop. The
parity lint (`tests/page_parity_lint.rs`) gains a pair for each page both read: Roadmap
(`roadmap.json`), the menu itself (`gui/navigation.json`), the destinations, People's credits.

## 10. History: Back and Forward

### 10.1 What counts as a step

**A step is anything that has, or gets, its own address on the website.** One rule, testable, the
same on both apps, because the website's addresses already name these places:

| Step | Desktop place | Website address |
|---|---|---|
| A page | `GuiPage` | `/page` |
| A Settings section | Settings, `audio` | `/settings#audio` |
| A Profile section | Profile, `body` | `/profile#body` |
| A Dev page | Dev, `claude` | `/dev#claude` (exists) |
| A chat channel | Chat, `global` | `/chat#global` |
| A conversation or a group | Chat, the conversation (in memory only) | `/chat`, the conversation kept in `history.state`: **the address never names who you talk to** (the address bar is seen, shared in screenshots, and synced by browsers) |
| A Files place, a Files item | Files, `shared-files`; Files, item 412 | `/files#shared-files`; `/files/412` (exists) |
| A Library document | Library, its slug (the reader's own `back` list joins the app's) | `/library#<slug>` (exists) |
| A how-to | How-to, its id | `/how-to#<id>` |
| A Maps view | Maps, `galaxy` | `/maps#galaxy` |
| The Market's faces | Market, `classifieds` | `/market#classifieds` |
| A page in the readable web view | Browser, its address | (the browser's own history) |

**Not a step:** scrolling, typing, opening a fold, a sort or a filter, a search being typed (opening
a result is a step), choosing an item in a list that stays on screen (the Apps page's details), a
selection in a form (the picker's character and world), the secondary row's preview, a popup (Back
and Escape close it), the footer chat's own channel choice (it does not change the page you are
on), and **entering the world** (section 11). Choosing the place you are already on adds nothing. A
place the app opens by itself (the first screen, a redirect) replaces the current step instead of
adding one. A notice clicked is a person's choice, so it is a step.

### 10.2 The rules

- **Browser semantics:** a list of steps and a position. Going somewhere drops the steps ahead of
  the position and adds the new one; Back moves the position back one, Forward forward one. At most
  100 steps (the oldest goes); kept for the session only, in memory, never written to disk (the
  no-caches rule, and nothing about where a person went is stored).
- **Each step remembers how it was made:** *beside* (a menu click, a link to another page) or
  *from* (a page opened from inside another: the chat's cog to Server settings, Open in the Library
  from Files, a notice to its page: today's `push_nav_to`). Back walks every step; Escape walks
  only the "from" steps (10.4).
- **Back and Forward:** the mouse's back and forward buttons (`Extra1`, `Extra2`) on any page; **Alt+Left
  and Alt+Right** whenever no text box has the keyboard (egui uses them for word jumps inside one);
  and on the desktop two buttons at the primary row's left end, an arrow each, dimmed with a hint
  saying why when there is nowhere to go (the website has the browser's own). **With a popup open,
  Back closes the popup first**, the way Android's back closes a dialog; the next Back goes back.
- **Nothing typed is lost:** a page's state lives in `GuiState`, so going back and forward on the
  desktop keeps a half-written message or form (the chat's draft is already shared with the
  footer).
- **One function moves between places** (`GuiState::go(place, how)`), and the forty direct writes to
  `active_page` move onto it; a lint then allows `active_page` to be written only there (and in the
  world's own entry and exit), so a new page change cannot skip history.
- **In-world screens** keep their own pages and history (their own Back on the screen); the app's
  history is the app's.

### 10.3 The website

The browser's Back already walks pages. Each in-page view in the table above moves onto
`history.pushState` and answers `popstate`, through one small shared helper
(`web/shared/history.js`: `hosHistory.go(place, { push, state })` and `hosHistory.on(fn)`) so every
page does it the same way: the chat (channels by `#name`, conversations and groups by state only),
the Files places (push instead of replace), Settings (`#section`), How-to (push instead of
replace), Maps, Profile, the Market. A full-screen popup on the phone pushes a state without an
address change, so the phone's back gesture closes it. The phone's back then walks exactly the
steps the desktop does, which is what makes the narrow three-column pages work (section 5).

**Already built for the chat (2026-11-03, `docs/design/chat-channels.md` section 7):** the address
shows the channel open (`/chat#<id>`) and a conversation, a group or the scratch pad leaves it at
`/chat`; every switch REPLACES the address through one function, `syncChatAddress` in
`web/chat/app.js` (the read view's twin is `showInAddress` in `web/chat/read.js`), and a
`hashchange` opens its channel. This rung turns those two into `hosHistory.go` with `push`, and
the `hashchange` listener into `hosHistory.on`.

### 10.4 Escape from then on

One press does the first of these that applies:

1. **Close the nearest thing open:** the help panel, a drop-down, the secondary row's preview, a
   popup that may be closed (a popup asking for a decision, like a press-and-hold confirm, keeps its
   own rule).
2. **Back out of a "from" step:** the page was opened from inside another, so Escape returns to it,
   the operator's rule of 2026-05-08 ("if I'm in nested pages like that esc needs to reliably take
   people back to the previous menu/page").
3. **On a page reached from the menu: go into the world**, as today (Escape is the game's pause
   key: it leaves the menus for the world and brings them back).
4. **In the world: open the page you left**, as today.
5. **The title screen:** nothing. **The picker** keeps today's Escape (back to the page before it),
   because Escape on a chooser means "not now", never "go in".

### 10.5 As built on the desktop (rung 4, 2026-11-04)

- **One file:** `src/gui/history.rs`. `GuiState::go(page, how)` opens a page and `go_to(place, how)`
  a place inside one; `How` is `Beside`, `From` or `Replace` (a place the app opens by itself: the
  first screen, the Profile alias). `back`, `forward`, `back_out` (Escape's rule 2), `escape` (rules
  2 to 5), and the world's `enter_world` and `leave_world` and the title screen's `show_title`,
  which are never steps. `GuiState::history` holds the steps (at most 100, in memory only).
- **The writes moved:** the 40 direct assignments to `active_page` in 18 files went onto those
  functions, and the 32 `push_nav_to` calls became `go(page, How::From)`; `nav_back_stack`,
  `push_nav_to`, `pop_nav_back`, `clear_nav_back` and `last_page` are gone. Each direct write that
  was a link to another page became `Beside`, so Escape on it still goes into the world as before.
  `tests/page_change_lint.rs` (in `just lints`) refuses an assignment to `active_page` outside
  history.rs, test code aside; it named the 40 when it went in.
- **A place inside a page is seen, not told:** once a frame on a page (`history::frame`, called by
  lib.rs before the menu) `note_place` compares what the page shows with the step the history is on
  and adds a step when they differ: the chat's server and channel (the Chat page writes its channel
  in 86 places), the Files place and item, the Library document, the how-to, the Maps view, the Dev
  and Profile sections. The frame after a step is made or returned to, the place the page settles on
  becomes that step's instead of a new one. Settings is told: its open section follows the scroll,
  so a section is a step only when chosen (its sidebar, a link to it, through `go_to`).
- **A chat step on another server** switches to that server when it is still a saved one (the
  server strip's own switch), then opens the channel the way its row does; a conversation opens
  through `open_dm_conversation`. The step holds the conversation's channel id in memory only.
- **The Library** lost its own `back` list: every document opened is a step of the app's history,
  and the reader's Back (shown while the step before is a Library document, "Back (2)" for two) is
  the app's Back.
- **Popups:** every dialog drawn through `widgets::dialog` says it is open (`popup_drawn`) and
  closes when asked; `close_popup` closes an open drop-down, else the dialog drawn last. Back from
  every source and Escape (lib.rs, after the help panel and the menu's preview) ask it first.
  Popups drawn some other way (the image viewer, a raw `egui::Window`) are not closed by it yet.
- **In the world** `history::frame` returns at once: the side buttons do nothing (unbound) and
  Escape keeps its world meaning. In-world screens draw pages with their own context and no
  history of their own yet.

## 11. The 3D game

- **In first person (Play): no menus, no footer, no warning**, as today: the world is the page, and
  the HUD is its own frame. Escape brings back the page you left, with its menus. The urgent layer
  and tooltips still show over the world.
- **The mouse's back and forward buttons in the world are game buttons, unbound by default**,
  bindable in Settings > Controls (`data/input.toml` gains `Mouse3` and `Mouse4` as names; the
  game's own buttons are `Mouse0` and `Mouse1` today). They never leave the world: a gaming mouse's
  side buttons are pressed by a gripping thumb, and being thrown out to a menu mid-fight is worse
  than pressing Escape. (Question 4: the other way is "back leaves the world like Escape".)
- **Entering the world is not a history step:** Play leaves the history where it is, and leaving
  the world returns to the step you were on, so Back after leaving goes to the page before it.
- **The picker (Characters) becomes a page with the menus** over its 3D stage: the primary row, the
  Play row, then WHO, the stage and WHERE, with the warning and the footer under it. It loses 88
  points of height (its layout already works in shares of the window,
  top-menu-and-game-pages.md section 5).
- **In-world screens, construction and the vendor never carry the menus:** they are the world's.

## 12. Keyboard and screen readers

- **Tab order:** Skip to the page (the website has it; the desktop gets it), the primary row, the
  secondary row, the page, the footer, the warning's links.
- **Inside a row:** Left and Right move between its buttons, Home and End to its ends; **Down** on a
  primary button moves into its secondary row, previewing it (the keyboard's version of resting the
  pointer); **Up** returns; Enter or Space opens; **Escape** ends a preview and returns to the page.
- **F6 and Shift+F6** jump between the regions (the primary row, the secondary row, the page, the
  footer), as Windows programs and browsers do.
- **The new keys are data:** Back, Forward, F6 and the mouse's side buttons go into the keymap files
  the help panel lists (`data/keymaps.ron` on the desktop, `data/keybindings/web.json` on the
  website).
- **Names and states:** every button carries its words (also in Icons only mode, as its accessible
  name) and its hint by key; the page's button says it is the current one (the website's
  `aria-current="page"`, its category's button `aria-current="true"`); the two rows are two `<nav>`
  regions labelled "Main" and "<category> pages". The preview is pointer and keyboard only: for a
  screen reader the secondary row always lists the current category's pages, and another
  category's pages are reached through its page, as on touch.
- **The desktop has no screen reader yet:** egui-winit's AccessKit adapter is off
  (docs/design/wcag-audit-2026-10-23.md, "The biggest gap"). The menus name every control so that
  the adapter's rung reads them the day it lands; the `a11y_tests.rs` Tab walk covers both rows.

## 13. The menu as data, and its tests

**One file, both apps: `data/gui/navigation.json`** (infinite of X: a category or an item is a row,
never code). Per category: its id, its words and hint keys, its icon name, its colour token, its
desktop page and its website address; per item the same, plus `desktop_only`, `web_only` and `new`. The
preview timings and the phone sizes are in it too. The desktop's `escape_menu.rs` and the website's
`shell.js` both draw from it, the phone's All pages list included, so the two menus cannot drift
again (a `page_parity_lint` pair holds both readers to it).

**As built in rung 1 (2026-11-03).** The file's own `_comment` says what each field means. Beside
the categories it has `outside` (what a person can reach that the rows leave out: the title screen,
the Profile alias, the website's front door, Onboarding, the Accord, Civilization, the Wallet guide,
Recovery and `/hi`, each with `why`) and `today_row` (each app's single row as it is, its groups'
colour tokens or CSS classes, and the older keys the website lights tabs by), which rung 2 replaces
with the two rows on the desktop and rung 5 on the website. A new page with a stand-in today opens
the stand-in (People the Directory, Servers the Relay Control page), its `_note` saying so. The
own-life category's id is `own_life` and its name the key `nav.own_life` while its name is open
(16.1). The desktop reads the file in `src/gui/navigation.rs`, once and again when it is saved,
refusing a malformed file whole (the exe's own copy is used at start, the menu kept on a save).
The website does not fetch it: `scripts/gen-navigation.js` writes the website's part, with its
English words, into `shell.js` (a page builds its menu before a fetch could answer, and the
desktop's offline copy of the website carries `web/` only), and a test fails while that copy is
out of date. Left for rung 2, which uses them: the preview timings, the phone sizes, and drawing
the icons by the file's names on the desktop (it draws each button's page's icon meanwhile).

**As built in rung 2 (2026-11-03).** The file gained `timings` (`preview_rest_ms` 200,
`preview_leave_ms` 300, `play_hold_ms` 800) and lost `today_row.desktop` (the desktop draws the
categories as its two rows; `today_row.web` stays the website's until rung 5, with `web_label`
keeping its own-life tab on Home). The phone sizes wait for rung 5, which draws the phone. Its
rules grew with the rows: a category always opens a whole desktop page and has a desktop picture
for its icon (`widgets::icons::named`: the file's names are the website's, mapped to the desktop's
own painted pictures, each the one its page wore in the single row); a part (`section`) must be
one its page has (the world's `picker`, the Dev page's parts); and a page not built yet (`new`)
never opens a desktop page, so **a row shows only pages that exist and never one under another
page's name**. That is why the rows as drawn differ from section 3's table: the Humanity row is
Directory (the Identity page, under the side list's word, standing where People will be),
Governance, Laws, Donate; Mission waits because until the overview is built the Humanity page IS
the Mission Dashboard (a Mission button would open the page Humanity opens); People, Rules,
Roadmap, Devlog, Worlds and Servers wait for rung 7 (the Relay Control page, Servers' future Yours
part, opens from the chat's sidebar as before and is in `outside`). The own-life category's word is
"Life", a working name the operator may change (16.1). Play is a page of its own
(`GuiPage::Play`, `src/gui/pages/play.rs`): who and where was played last and Enter the world; a
tap on Play opens it, a press and hold of 800 ms goes in (the button fills as it is held; the
hold's clock is `widgets::hold_step`, shared with the delete confirms, and 800 ms is where egui
stops counting a press as a click, so no press is both). The sizes are the section 7 tokens and
the colours one `nav_cat_<id>` token per category, each in Settings (Appearance's colours, Widgets'
Nav card), and the preview's switch is Settings > Appearance, "Show a category's pages when the
pointer rests on it". Back walks the steps Escape walks (the old back stack) until rung 4; Forward
is drawn dimmed, saying it is not built yet. The help panel, pinned, has Report a problem (16.1,
question 9). The cost is not a frame-cost row: the menu draws only on frames a page covers, which
the frame costs discard (no world was drawn), so it is timed headless instead
(`the_two_rows_cost_a_small_part_of_a_frame`): 0.31 ms on the fastest frame and 0.32 to 0.34 ms
median in a test build (our code unoptimised), on his screen with the preview showing. The release
build was not timed; if it is near the 0.1 ms this section calls a finding, that is the next look.

**New theme tokens** (`theme.ron`, each with its Settings editor row and its accessor): the sizes in
section 7 (`nav_primary_height`, `nav_primary_button_height`, `nav_icon_size`,
`nav_secondary_height`, `nav_secondary_button_height`, `nav_secondary_font_size`), one colour per
category (`nav_cat_humanity` and its seven siblings, defaulting to the colours of section 3), the
layout shares (`layout_reading_max`, `layout_side_min`, `layout_side_max`, `layout_details_min`,
`layout_details_max`), and the layer scale for the website (section 6).

**Tests, each seen red first:**

- every `GuiPage` a person can reach is in the file exactly once, and every website address in it
  exists (`page_registry_lint` extended: "none lost, none twice");
- the two rows' order and words on both apps come from the file (the parity pair);
- a headless click on each primary button opens its page, a rest of 200 ms on another previews its
  row, a click in the preview opens that page (`ui_snapshots.rs`'s synthetic input);
- the layer order with a dialog, the notices panel and an urgent card open (section 6);
- Back, Forward, Alt+Left in a text box (the cursor moves, the page does not), Escape's five rules,
  the world never a step (section 10);
- the website: a `z-index` literal outside `theme.css` fails; a conversation never puts a key or a
  name in the address; `popstate` restores each in-page view;
- snapshots of both rows at 2560 x 1440, 1280 x 720, at 150% and 200%, and the phone's at 412 x 915.

**Budget:** two rows of at most nine buttons each, laid out on frames the app already draws (the
desktop draws every frame while shown, `engine/tray_window.rs`), no file read per frame (the menu
file is read once at start and on hot reload), no cache. The preview's timer asks for one repaint
when it ends. A `cpu.span.nav` span measures the rows in rung 2; anything near 0.1 ms a frame is a
finding.

## 14. The rungs, each with its gate

1. **The menu as data, no visible change.** `data/gui/navigation.json`; `escape_menu.rs` and
   `shell.js` draw today's single row from it; the parity pair and the "none lost, none twice"
   test. *Gate:* `just lints`, `just test-lib`, `node --test scripts/tests/`, `nav_bar_full.png`
   unchanged. **Built 2026-11-03** (section 13, "As built"): `nav_bar_full.png` re-renders byte
   for byte the same, and the website's menu markup is byte for byte the same on 25 pages. Its
   tests: `src/gui/navigation_tests.rs` (the file reads, every `GuiPage` once, a malformed row
   refused naming the entry, the desktop's row drawn and clicked is the file's),
   `scripts/tests/navigation-web.test.js` (the website's half), `page_registry_lint` (every web
   page once, every address a page) and `page_parity_lint`'s "The top menu".
2. **The double menu on the desktop.** The two rows, the preview, More, the sizes as tokens, Back
   and Forward buttons (drawn, wired in rung 4); the Humanity and Dev side lists become their rows;
   Inventory, Crafting and Quests move under Play. Then **delete `src/gui/nav_mockup_tests.rs`**.
   *Gate:* `just verify`, the click and preview tests, the a11y Tab walk, the snapshots of section
   13 read by eye. **Built 2026-11-03** (section 13, "As built in rung 2"; with the operator's
   answers of 16.1: Play's tap and hold, the preview's switch, the Dev row, Report a problem in
   the help panel). Its tests: `src/gui/pages/escape_menu_tests.rs` (the rows are the file's, a
   click on each category opens its page, a rest of 200 ms previews and leaving brings back the
   page's own row, a click in the preview, the switch off, Play's tap, hold and button, More at
   100%, 150% and 200% on 2560 x 1440 and 1280 x 720 with never a third row, what More lists),
   `navigation_tests.rs` (the rows hold only pages that exist, each once), `a11y_tests.rs` (the
   menu's Tab walk names and hints every stop, in both icon modes), `font_size.rs` (two lines at
   every size, where BUG-225's test had pinned the single row's two and three), the help panel's
   `the_pinned_help_panel_reports_a_problem`. Its pictures: `tests/snapshots/nav_rows_*.png` and
   `play_page.png`. The mock-up and its five pictures are deleted (a picture no test writes is
   refused, `tests/snapshot_recipe_lint.rs`).
3. **The layer order.** `src/gui/layers.rs`; the notices and help panels under a popup's shade; the
   toasts to the tooltip order; the website's `--z-*` scale and the literal ratchet; the website's
   modals covering the whole window. *Gate:* the layer-order test, `wip-strip-room-web.test.js`,
   the web tests.
4. **History on the desktop.** `go`, the places, the forty writes moved, the side buttons,
   Alt+arrows, Escape's rules, the Library's own `back` joined. *Gate:* the history tests, `just
   verify-runtime` (in the world the side buttons do nothing). **Built 2026-11-04** (section 10.5).
   Its tests: `src/gui/history_tests.rs` (Back and Forward through pages, a Settings section, a
   chat channel and a Files place; the cap of 100; a popup closed first; Alt+Left in a text box
   moves its cursor and not the page; the mouse's `Extra1` and `Extra2` on a page and in the world;
   Escape's rules; the world never a step), each seen red against a broken history (the file's
   head says how), and `tests/page_change_lint.rs`.
5. **The website's menus and history.** `shell.js` from the file (hover, touch, the phone's
   rows and All pages), `web/shared/history.js` and each in-page view on it. *Gate:* the web tests,
   screenshots at 2560 x 1440 and 412 x 915 in both themes.
6. **The three layouts.** `widgets::layout` and `.layout-1/2/3`, then the pages onto them, a few
   at a time (or as each page is next touched). *Gate:* each page's snapshots at three widths.
7. **The pages the audit needs.** Desktop Roadmap, Devlog (with the front door's rung 5 update
   card) and Rules; People on both (Credits and the Directory moved in); Servers on both
   (RelayControl its Yours part; Find more); Worlds on the desktop; the website's Mission and the
   Civilization merge; the Wallet guide into help; the six question cards on the front door and on
   the Humanity overview (with app-bottom-strip.md's rungs 4 and 5, the overview's bands). One
   increment each. *Gate:* each page's own tests and snapshots, `docs/PAGES.md`, the parity pairs.
8. **The footer chat** (app-bottom-strip.md rungs 2 and 3, the chat's renamed channels) takes layer
   30, the slot this design keeps for it.

Rungs 1 to 3 change shared files (`escape_menu.rs`, `shell.js`, `lib.rs`'s frame, `theme.ron`), so
they go one at a time; rung 7's pages can go in parallel once rung 2 has landed.

## 15. The mock-ups

**Deleted with rung 2 (2026-11-03), code and pictures:** the design's mock-up was throwaway by
plan, and `tests/snapshot_recipe_lint.rs` refuses a picture no test writes. The five pictures
below are in git at the commit before rung 2 (`git show 57fde1c95:tests/snapshots/nav_double_tv.png`
and so on); the two they draw that are not built yet, the popup's layers (rung 3) and the phone
(rung 5), are drawn again by those rungs' own snapshots. Rung 2's real rows are
`tests/snapshots/nav_rows_{tv,720}_{100,200}.png` (the Dev page's row, docked) and the same with
`_preview` (the Chat page, the pointer resting on Play), and `play_page.png`.

They were rendered by `src/gui/nav_mockup_tests.rs` with the real theme, the real fonts, the real
warning bar and the menu's words by key, each band tagged with its layer number in his numbering.

| Picture (deleted) | What it showed |
|---|---|
| `tests/snapshots/nav_double_tv.png` | His screen, 2560 x 1440: the primary row (Humanity open; Back and Forward at its left), Humanity's row with the new pages marked, the overview in one column (the three bands, the six question cards, the latest updates), the footer chat folded on # global, the warning |
| `tests/snapshots/nav_double_tv_hover.png` | The Files page (Chat's category) in three columns, the pointer resting on Play so the secondary row previews Play's pages, and the footer chat unfolded with # global, # local, # feedback and # announcements as rows |
| `tests/snapshots/nav_double_tv_popup.png` | A full-screen popup: its shade over everything, the menus and the warning included (5), the window (4), its controls (3), an incoming call over it (2) and a tooltip (1) |
| `tests/snapshots/nav_double_laptop.png` | The app's restored window, 1280 x 720: both rows still fit on one line each |
| `tests/snapshots/nav_double_phone.png` | His phone, 412 x 915: the primary row of icons over words scrolling sideways with All pages pinned, the chips of the secondary row, the overview in one column with the question cards two across, the warning. (The warning there is the desktop's bar drawn at phone width, its words squeezed beside its links; the website's own bar runs its words the full width with the links under them, about four lines.) |

## 16. Questions for the operator

1. **The eight categories** (section 3): Humanity, Play, Chat, Home, Apps, Library, Dev, Settings,
   with their rows. In particular **Home or Profile** as the name of your-own-life category.
   *Recommended:* as listed, Home (two-realities.md's own word), Profile its first page.
2. **Play's click:** go into the world at once, its hint naming the character and world (today's
   behaviour, which you asked for on 2026-06-16), with Characters and the rest in its row.
   *Recommended:* yes.
3. **The preview on hover** (section 4.1): a category's pages show in the second row after the
   pointer rests 200 ms on its button, with a Settings switch to turn it off. *Recommended:* yes, as
   your words describe.
4. **The mouse's side buttons in the world:** unbound game buttons, or "back leaves the world like
   Escape". *Recommended:* unbound, so a gripping thumb never throws you out mid-fight.
5. **An urgent alert over an open popup** (section 6): an incoming call or an emergency alert shows
   over an open popup, in its own band, never taking the popup's typing. *Recommended:* yes.
6. **The six question pages** (section 8): Who is People, What is Apps, When is Roadmap, Where is
   Servers (or Maps), Why is Mission, How is How-to; and the six cards replace the front door's
   eight buttons, each with one second link. *Recommended:* as listed, Where as Servers.
7. **The website's `/mission`** becomes a real page again (the desktop's Mission page's mirror, no
   copy of the essay). *Recommended:* yes.
8. **Rules** as a desktop page beside Laws now, and later perhaps the innermost "this server" level
   of Laws. *Recommended:* beside Laws now.
9. **The Dev page's sections as pages in the Dev row**, with Bugs also in the help panel ("Report a
   problem") for everyone, since Dev hides at 1.0. *Recommended:* yes.
10. **On the phone**, the two rows always visible (your rule), or sliding away while a page scrolls
    down and back at the first scroll up. *Recommended:* always visible.
11. **One name each:** "Server settings" (desktop) and "Admin" (website) for one page; "Browser"
    (desktop), "Web" and "Bookmarks" (website) for another. *Recommended:* "Server settings" and
    "Websites".
12. **People:** the name (People, Members, or Community), and AI participants listed among them,
    labelled AI. *Recommended:* People, with AI listed and labelled.

### 16.1 His answers (2026-11-03)

Quoted exactly. Where a question is not here, it is still open, and its recommendation stands
until he answers.

- **Question 1, the name of the own-life category.** "I'm worried about there being a conflict
  between the home page and the page with the player homes. What other options do you think makes
  sense? "Mine"?" Open. The orchestrator's answer: "Life", which pairs with Play (your real life and
  the game, `two-realities.md`) and names nothing else in the app; "Mine" collides with the Tasks
  page's own "Mine" view (tasks rung 1, BUG-217) and does not fit Market or Trade; "Me" and
  "Personal" are the others. Since rung 2 the desktop shows "Life" as a working name (one word,
  `nav.own_life`); it still opens the Home page, whose own heading is "Your home". The website's tab
  says Home until rung 5 (`today_row.web_label`).
- **Question 2, Play's click.** "Maybe click and hold after booting the app. Then just tapping Play
  brings up the subpages that are relevant to the game. Like characters, homes, servers, etc.?"
  So: a tap on Play opens Play's page and row (Characters, homes, Worlds, Servers, ...); a press
  and hold goes into the world, filling as it is held (the reusable `widgets::hold_to_confirm`,
  with a hold far shorter than its 3 s for deleting). Holding is hard for some hands, so Play's
  page also has its own "Enter the world" button, and a keyboard's Enter on that button goes in at
  once. Section 3 and rung 2 follow this.
- **Question 3, the preview on hover.** "For what exactly? Yes with option for off." The preview is
  this: resting the pointer on a category's button for a moment shows that category's pages in the
  second row, without clicking, so a person can see what is inside before going there; moving
  away brings back the row of the page they are on. Yes, with a Settings switch to turn it off.
- **Question 4, the mouse's side buttons in the world.** "I'm not sure what to make them do yet."
  They stay unbound in the world until he decides; on every page they are Back and Forward.
- **Question 5, an urgent alert over an open popup.** "Yes."
- **Question 8, Rules beside Laws now.** "Yes"
- **Question 9, the Dev page's sections as row pages, Bugs also in the help panel.** "Yes."
- **Question 10, the phone's rows always visible.** "Yes."

On the mock-up of the Files page (`nav_double_tv_hover.png`): "I feel like that first image should
be ordered as your private local files files, your publicly shared files, then files by
friends/servers? Maintains the RGB styling. R for private, B for servers." The RGB language is
the launcher's (`characters-and-servers.md`: offline RED, open GREEN, closed BLUE), so the Files
page's places go red (this computer: the HumanityOS folder, Your folders), green (what you share
publicly), blue (what friends and servers share). That is a change to the Files page itself,
queued as its own increment; the menus here are unchanged by it. **Built 2026-11-03, both apps**
(docs/design/files-showcase.md, "The places in the launcher's red, green and blue"): red "Private,
on this computer" (the HumanityOS folder, Your folders; desktop only), green "What you share"
(Yours on <server>, Your shared files), blue "What everyone shares" (Shared on <server>, Shared
files, Admin), in the launcher's own tokens; the page still opens on Shared on <server>. The
mock-up picture itself (`nav_double_tv_hover.png`) was deleted with the mock-up in rung 2, still
drawing the old order (it is in git at 57fde1c95); the Files page's own snapshots draw the new.
