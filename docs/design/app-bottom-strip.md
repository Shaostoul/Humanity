# The bottom of the app screen: an unfinished-software warning, a chat footer, an Overview page

**Status (2026-10-30):** planned whole; **rung 1 built** (the warning, both apps, its Library
page). The chat footer and the Overview page wait for the work now changing Home, Quests, Tasks
and the chat's notifications to merge (section 6). Decisions for the operator are in section 7;
nothing here is decided for him except where his own words say so.

## 0. The operator's words (2026-10-30)

> What about an overview page? There could be a wide but short area anchored to the bottom of the
> current channel in chat. Maybe for the desktop app we can make the chat a footer that's anchored
> and collapsible/expandable to hide the chat on a give nonchat page. Then like a tightly packed
> overview of the default/demo/active character's stats, like nutrition, hydration, sleep, etc.
> Also, I feel we should slap an anchored warning somewhere on the menu pages not fps mode camera,
> that explains this software is wip and not to trust it with your life at the moment or whatever
> wording is appropriate. I want to make it obvious we can't be held liable for any harm prerelease
> software may cause. Until v1.0 I want to make it obvious it is still WIP. It could be anchored to
> the bottom of the screen. The footer chat would go above the WIP Warning visually. We could
> include a link on the wip warning to a page about more details about the warning and development.

It came up while deciding what united-humanity.us should open (the journal's open LANDING PAGE
question): the earlier proposal was a "Start here" page built round one search over the How-tos
and the Library; he is weighing an Overview page instead, or as well (section 5.5).

## 1. What exists today (read 2026-10-30)

- **The app frame.** `src/lib.rs` draws, each frame: the top menu (`pages/escape_menu.rs`) on
  every page but `None` (the first-person world) and `MainMenu` (the title screen); then the page,
  through `gui/dispatch.rs`'s one table, which also adds the server strip (`pages/server_strip.rs`)
  under the menu on Media, Watch, Chat and Server Settings. In-world wall screens draw pages through
  the same table into their own textures (`gui/screen_surface.rs`), without the top menu.
- **The website's frame.** `web/shared/shell.js` on every page (41 of 42; `mission.html` is a
  forwarder): a fixed top menu with a spacer under it, a collapsible footer fixed to the bottom
  (`.site-footer`, folded by default, its toggle tab peeking above it), and the help button.
- **What the game tracks about a body** (`src/systems/`, shown by the HUD, `pages/hud.rs`
  `vital_rows`): Food (`satiation`), Water (`hydration`), Energy (the sleep need: it drains over
  about sixteen waking hours and a night in a bed restores it, `systems/sleep.rs`), Air, Body
  temperature (`systems/body_heat.rs`), Shelter, Waste, Health (`player_health`), each wound and
  illness with what helps (`systems/wounds.rs`, `systems/illness.rs`), status effects; food also
  carries calories, protein, carbohydrate, fat, fibre and water per 100 g (`systems/food.rs`
  `NutritionProfile`). These reach the GUI as `GuiVitals` only while in the world: before it,
  `vital_rows` has nothing to say (`satiation_max` is 0).
- **The Real side** (`docs/design/two-realities.md`): Profile's Body & Measurements is the start of
  "Real Me"; the mandate is that every game system has a real mirror (vitals: real health metrics).
- **The first screen** is a setting (`page_to_config_str`: Humanity, Chat, Tasks, Maps, Notes,
  Calendar, Library; Humanity by default).
- **Nothing a person sees says the software comes as is.** The website's footer says "Public
  domain" and links CC0; the desktop app says nothing (finding 1 of the dated finding below).

## 2. The stack, bottom up

```
 +--------------------------------------------------------------+
 | top menu                                                     |
 | (server strip, on four pages)                                |
 |                                                              |
 |   the page                                                   |
 |                                                              |
 | (the page's own bottom panels: crafting queue, palette)      |
 +--------------------------------------------------------------+
 | chat footer: one line, or a short wide strip     (rung 2, 3) |
 +--------------------------------------------------------------+
 | unfinished-software warning: one line            (rung 1)    |
 +--------------------------------------------------------------+
   the operating system's taskbar
```

- **One entry point**, `gui::bottom_strip::draw`, called once a frame by `lib.rs` after the top
  menu and before the page. egui stacks bottom panels in the order they are added, the first
  outermost, so it adds the warning first and the chat footer after it; the page's own bottom panels
  come later still and sit above both, and its side panels end where the strip begins.
- **Where it shows:** every page a person reads, the title screen included; never the
  first-person world (`GuiPage::None`); never an in-world screen (the dispatch table does not call
  it, so a monitor in the world stays part of the world).
- **The website** builds the same layers in `shell.js`: the warning fixed at `bottom: 0`, the
  footer standing on it (`bottom: var(--hos-wip-h)`), and a spacer as the page's last element so
  nothing is hidden under it (the chat page's message box included). A future chat footer would
  stand on the warning the same way (section 4.6). **Every other element fixed or sticky at the
  bottom of the window stands on it too** (2026-10-30): `bottom: var(--hos-wip-h, 0px)`, or
  `calc(<its offset> + var(--hos-wip-h, 0px))`, Settings' Save bar, the chat's phone drawers, the
  Tasks drawer, the dialogs' backdrops and the toasts among them; the one other answer is a layer
  over the whole window above the strip (a full-screen viewer, the phone menu), which hides the
  warning while it is open. `scripts/tests/wip-strip-room-web.test.js` finds every such rule in
  web/ and fails on one that does neither. **A page box as tall as the window** (`height:
  calc(100vh - <the menu>)`: Settings, Accord, Calendar, Inventory, Maps, Notes, Profile, the Files
  panes) subtracts `var(--hos-wip-h, 0px)` too (2026-10-31), or the spacer after it makes the whole
  window scroll by the strip's height over the page's own scrollbar; the same test fails on one
  that does not. **The desktop's twin:** egui anchors a window to the
  whole screen, not to the room the bottom panels leave, so a window anchored to the bottom adds
  `bottom_strip::height(ctx)` to its offset (the first-minutes tour card, the toasts) and the
  themed dialogs are kept inside `bottom_strip::room_above(ctx)`; in the world, where the strip
  does not draw, both are the whole screen.
- **Budget, all layers:** nothing per frame when idle. Each layer lays out a few labels on frames
  the app already draws, reads no file, keeps no cache, and never asks for a repaint. The website
  builds static DOM once; its only listener is a resize watch on the strip.

## 3. The unfinished-software warning (rung 1, BUILT)

### 3.1 The words

From the dated finding, `docs/reference/findings/2026-10-30-wip-warning-and-liability.md`, which
read the licence, the UK, EU and Washington law from primary sources. Its short answer, which the
words follow: the licence (CC0 1.0) already disclaims warranties but has no liability limit; a
warning makes the risk obvious and warnings count in every law read; no warning makes anyone
immune, and several rules forbid excluding liability for death or injury from negligence.

- The line (`wip.line`): "Unfinished software: it can be wrong, break, or lose your work. Do not
  rely on it for health, safety, food or water. It comes as is, with no [[warranty]]." The word
  warranty is a glossary term (`data/glossary.json` `warranty`).
- The link (`wip.more`): "What this means", with its hint (`wip.more_hint`). It opens the Library
  on the details page, except during the desktop's first-run setup (the title screen before
  `onboarding_complete`), where the same page opens in a window over the setup, with Close
  (`wip.more_setup_hint`, `wip.details_close_hint`), from the copy of the Library already loaded:
  the Library page carries the top menu, and the link must not be a way into the app before the
  person has an identity (2026-10-30, the seams review). A browser was the other choice and was
  not taken: the setup may be offline, and a person new to computers should not be sent out of it.
- The version (`wip.version`): "Version {{version}}", the running version.

**Why not "we can't be held liable for any harm":** the finding shows that is untrue in places
(UCTA 1977 s.2(1), the Consumer Rights Act 2015 s.65, the EU Product Liability Directive Article 15,
the Consumer Protection Act 1987 s.7), and an untrue disclaimer can read as misleading. The
details page carries the limit that can honestly be claimed: "to the extent the law allows".

### 3.2 The details page

`docs/user/rights/unfinished_software.md`, in the Library's "Your Rights and Consent" shelf
(slug `unfinished-software`; `node scripts/build-library.js`), so the desktop Library and the
website's `/library#unfinished-software` show one document. It says what unfinished means here
(BUG-216 and the 48-of-60 species review as real examples; saves may not carry over before 1.0),
what not to rely on it for, what "as is, with no warranty" means with CC0's own words, the
"to the extent the law allows" sentence and its honest limit, when the warning goes (1.0), how the
project is built, how to report a problem, and the emergency line.

### 3.3 Look and behaviour

- One line, the full width, on `warning_bg` (a new theme token: dark amber, pale amber on the
  website's light theme) under a 1 px `warning` edge; the words in `text_primary`, the link in
  `warning`, the version in `text_muted`, the glossary word in `accent`. Every pair is held to
  WCAG AA in both themes by `scripts/tests/contrast.test.js` (seen red with a failing ground).
  Readable, not shouting: small text, no animation, no flashing.
- On a narrow window the words wrap; the warning is never cut short.
- **Until 1.0, decided by the version**, `bottom_strip::wip_shown` (desktop) and `hosWipShown`
  (website), one rule held to one list of versions by both apps' tests
  (`scripts/tests/fixtures/wip-versions.json`): every 0.x shows it, every pre-release of 1.0 shows
  it (`1.0.0-rc.1`), a version neither can read shows it, a release (`1.0.0`, `1.0.0+build`) hides
  it. No setting can leave it on or switch it off. A test fails on the day the version reaches 1.0
  (`the_running_version_shows_the_warning_until_one_point_oh`), so removing the warning is a decision.
- **Not dismissible in rung 1** (decision 7.1 proposes keeping it so).
- Accessibility: the website's strip is a `role="note"` region with a label (`wip.label`); its
  link is reachable by keyboard and carries its hint; the desktop link has its hint by key.

### 3.4 Built, and where

| Part | Desktop | Website |
|---|---|---|
| The strip | `src/gui/bottom_strip.rs`, called by `src/lib.rs` | `web/shared/shell.js` (`wipStrip`, CSS `.hos-wip-strip`) |
| Words | `data/i18n/en.json` `wip.*` | the same keys (English in the markup, `i18n.js` for others) |
| Colour | `data/gui/theme.ron` `warning_bg`, `theme.rs`, Settings editor row | `theme.css` `--warning-bg` (generated; light value in `gen-theme-css.js`) |
| Details | the Library, `unfinished-software` | `/library#unfinished-software` |
| Tests | `bottom_strip.rs` tests (version list, on every page and never in the world, the panel takes room, the link's page ships) | `scripts/tests/wip-strip-web.test.js` (7 tests, each seen red on a broken copy of web/), `contrast.test.js` (4 new pairs) |
| Pictures | `tests/snapshots/wip_strip.png`, `wip_strip_narrow.png`, `wip_strip_title.png`, `wip_details.png` | |

## 4. The chat footer (rungs 2 and 3, NOT BUILT)

### 4.1 What it is

The desktop app's chat, as a footer above the warning on every page except Chat itself: so a
person on the Inventory, the Library or the Maps page still sees the conversation and can answer
it, without leaving the page. In the operator's words, "anchored and collapsible/expandable to hide
the chat on a given nonchat page".

### 4.2 Three states

- **Off:** no footer. On the Chat page (the page is the chat), in the world, on in-world screens,
  when not signed in to any server, and when the person turns it off.
- **Folded** (the default when on, rung 2): one line, the warning's height. The current channel's
  name, its latest message as "name: words" cut to the line, the unread count when there is one, an
  unfold arrow, and Open in Chat.
- **Unfolded** (rung 3): a short, wide strip of the current channel, about 180 px tall by default,
  resizable from its top edge between about 120 px and 40% of the window: the last messages, newest
  at the bottom as on the Chat page, and the composer under them.

### 4.3 How it relates to the Chat page

One conversation, two views of it. The footer shows `chat_active_channel`, the same channel the
Chat page has open; picking another channel in the footer changes it for both. The draft is the
same (`chat_input`), so a sentence started in the footer is there on the Chat page. Sending is the
Chat page's own send path, so line limits, the send limiter, replies and the protected setup apply
unchanged. Unread counts and mentions come from the chat's notification work (being changed now)
and are read, never recomputed. Opening the Chat page hides the footer; leaving it brings the
footer back in the state it was in.

### 4.4 Code shape

`src/gui/bottom_strip/chat_footer.rs`, drawn by `bottom_strip::draw` after the warning. The message
rows and the composer must be the Chat page's own, not a second copy: rung 2 extracts the row
renderer and the composer from `pages/chat.rs` (which is at its file-size budget) into
`pages/chat/rows.rs` and `pages/chat/composer.rs` that both call. That extraction is why it waits
for the notifications work to merge.

### 4.5 Settings, words, budget

- A Settings row, Chat footer: Off, Folded, Unfolded (`config.json`), and the unfolded height
  remembered; a keymap entry (data, not code) folds and unfolds it.
- Words by key (`chat_footer.*`), a hint on every control, theme tokens only.
- Folded: one line built from the newest message each frame (microseconds), no cache. Unfolded:
  only the visible rows are laid out, as the Chat page does.

### 4.6 The website

The website's chat is its own page, and a footer on every other page would open a relay
connection on every page. Native first (the operator said "for the desktop app"); the website only
if he wants it (decision 7.4), standing on the warning through `--hos-wip-h`.

## 5. The Overview page (rungs 4 to 6, NOT BUILT)

### 5.1 What it is

One tightly packed screen of the active character's state, "like nutrition, hydration, sleep,
etc.": everything the game tracks about a body and a home, at a glance, each number with a plain
word beside it ("Water 48%: drink soon") and its hint, coloured by the HUD's own severity (success,
warning, danger), never colour alone.

### 5.2 What it shows (the Sim side)

Tiles, each one domain, all from values the app already has:

| Tile | From |
|---|---|
| Body: Food, Water, Energy (sleep), Air, temperature, Waste | `GuiVitals` through `hud::vital_rows` in Always mode |
| Health, wounds and illnesses, what helps | `player_health`, `GuiVitals::care` |
| Where you are: shelter, the air and what it feels like | `GuiVitals` (`shelter_note`, `air_c`, `feels_c`) |
| Home: power, water, air | the Home page's live values (`power_balance`, `water_days_autonomy`, `air_breathable`) |
| Today: the next quest step, tasks due, crops ready | quests, tasks, `GuiCrop` |
| What you ate today: calories and the macronutrients | `systems/food.rs` (needs a day's intake log: the first new data the page asks for) |

### 5.3 Whose character

- **Active:** the character of the last pairing played (Play repeats it,
  `docs/design/play-characters.md`), live in the world (the Esc menu over the world).
- **On the menu, before the world:** the vitals are not synced yet, so the page reads the active
  character's last saved state from its save once when opened, labelled "as of" its save time.
- **None yet:** a demo character from data (`data/overview/demo_character.json`), labelled
  Example on every tile, never mistaken for the person's own.

### 5.4 The Real side

With the Real/Sim toggle on Real, the Overview is the person's own real tracking (Real Me: what
Profile's Body & Measurements holds, and later diet, sleep and fitness logs). It shows only what
the person entered; an empty tile says how to start ("Add your weight"). Real and Sim numbers are
never mixed on one screen.

### 5.5 The app's first screen, and the website's front door

- **App:** Overview added to the first-screen choices; whether it becomes the default is decision
  7.5.
- **Website:** it has no character, so the front door would show the demo character's Overview
  (static data, labelled Example) as "what HumanityOS keeps track of", with the tiles linking to
  the tools. The "Start here" idea fits on the same page: a one-line mission, then one search over
  the How-tos and the Library ("what can it do for me now"), then the Overview ("what it looks
  like"). Which leads is decision 7.6.

### 5.6 Data, not code (infinite of X)

The tiles are data, `data/overview/tiles.json` (id, side real or sim, a reader name, the words and
hint keys, the order), not a list in code. A reader name picks one of a small registry of
functions that read a value the app has, so a tile for an existing value is a row; a new kind of
value is a new reader.

### 5.7 Budget

It reads values already in `GuiState`; the saved state is read once when the page opens or the
active character changes; no file is read per frame; no cache is kept.

## 6. The rungs, in order

1. **The warning** (BUILT 2026-10-30): both apps, its words, its token, its Library page, its
   tests, its snapshots. Checks: `just lints`, `just test-lib`, the relay check,
   `node --test scripts/tests/`, `node scripts/check-doc-links.js`, the snapshots read by eye.
2. **The chat footer, folded:** after the chat notifications work merges. Extract the row
   renderer and composer from `chat.rs`; the folded line; Open in Chat; the Settings row. Checks:
   a headless test that sending from the footer goes through the Chat page's send path, a snapshot
   folded over the Inventory page.
3. **The chat footer, unfolded:** the strip, its resize, the composer, the keymap entry. Checks:
   the same draft on both views; a snapshot unfolded.
4. **The Overview, Sim side, live:** the tiles from `tiles.json`, the readers, the demo
   character; reachable from the menu. Checks: snapshots with the demo state, a test that every
   tile's reader exists.
5. **The Overview before the world, and the Real side:** the saved state read, "as of"; the Real
   tiles from Profile; the first-screen choice.
6. **The website's front door:** the demo Overview and the Start-here search on one page (after
   decision 7.6), mirrored from the desktop page.

Files rung 1 deliberately did not touch, because other work is changing them now: `homes.rs`,
`herd.rs`, `household.rs`, `quests.rs`, `onboarding.rs`, `tasks.rs`, `chat.rs`.

## 7. Decisions for the operator

1. **Can the warning be dismissed?** Proposed: no. It is one thin line, and its point is that it is
   always there until 1.0. The alternative, if it gets in the way: fold it to a small "Unfinished"
   tab for the session, still a link, back at every launch, with no setting.
2. **The title screen carries it too** (built so; it is a menu page). Say if it should not.
3. **The wording:** the finding's; a lawyer should read it before 1.0 (the finding says what a
   lawyer would settle). Change any word and the tests hold both apps to the new one.
4. **The chat footer on the website** too, or the desktop app only (proposed: desktop only).
5. **The Overview as the app's default first screen**, in place of Humanity.
6. **The website's front door:** the Overview, the Start-here search, or both on one page, and
   which leads. Proposed: both, the search first.
7. **The demo character:** its name and look, and whether the website shows it at all.
