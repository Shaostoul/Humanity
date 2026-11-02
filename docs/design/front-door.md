# The website's front door: a title screen with tools in it

**Status (2026-11-02):** designed, with a prototype the operator can open on his monitor and his
phone (`web/prototypes/front-door/`, section 9). Nothing here replaces the live front page
(`web/pages/index.html`) and nothing is decided for the operator except where his own words say
so; his decisions are in section 8. This answers decision 6 of
[app-bottom-strip.md](app-bottom-strip.md) (section 7), the journal's open LANDING PAGE question.

## 0. The operator's words

2026-10-30, after neither a game-first nor a tools-first hero got people to care:

> I almost want to change the landing page to be something more utilitarian instead of just a
> landing page.

2026-11-02, on a "Start here" page built round one search box over the How-tos and the Library:

> I like this. We want to get people a bit interested but, provide tools/info that's useful
> immediately. A "if you're new, click here" might actually be the right call. How do you
> recommend making it better than the onboarding page?

2026-11-02, on what the front page should be:

> I'm still not sure. I feel like it needs to be informative but, also useful. Kinda like a title
> screen on a game with buttons to elsewhere about more indepth info about various aspects. Maybe
> we could do a like a live feed of the software updates. We #announcements, we could post little
> cards for each. Like a single row with like 1 to 9 card columns. I don't know how it'll look on
> my monitor or phone so I'm not sure how many is too many. We could have a read only bit of the
> #general chat so people can see activity beyond just updates. Kinda like the update cards in
> height, maybe only like 5 to 9 lines of text with standard page width aligned with other
> elements.

And the standing evidence (memory `feedback_homepage_comprehension_failure`, 2026-08-24): real
people handed his phone could not tell what the site is or "the reason why they're spending time
on this thing at all". The fix it names is placement, not deletion: say what it is and what you
would do first, keep the mission but stop leading with it, and test on cold readers rather than
re-judging taste.

## 1. What exists today (read and measured 2026-11-02)

- **The front page** (`web/pages/index.html`, "one breath per screen", 2026-07-16, the dawn clip
  added 2026-10-04): a hero ("Free tools for growing food, collecting water, and making power.",
  Download free, See what it looks like), the Silverdale dawn clip, the task-board picture, six
  feature cards and the game clip, "How the software fights back", "It costs nothing", the dream,
  a proof bar, "Come build it with us". Measured in the same browser as the prototype: **9,727 px
  tall at 2000 x 1125 (8.6 screens) and 9,528 px on a 390 x 844 phone (11.3 screens)**. Nothing on
  it is useful by itself: no search, no guide, no sign that anyone has done anything this week.
  Its words are not keyed (no `data-i18n`), and its styles carry colour literals.
- **The Onboarding page** (`web/pages/onboarding.html`): a hero ("Learn to provide for yourself"),
  four pillar paragraphs, eight tool tiles (Chat, Tasks, Maps, Market, Notes, Profile, Wallet,
  Settings), the quest chains from `data/onboarding/quests.json` with progress kept in the browser
  (`humanity_quest_progress`), and three buttons (Open the Chat, the Accord, back to the landing).
  Section 7.1 says where it loses a newcomer.
- **The tour** (`web/shared/onboarding-tour.js`): an opt-in card in the corner, the desktop's
  first-five-minutes steps (`data/onboarding/tour.json`): the recovery phrase, a first message, the
  world, where help lives. It orients a person in the app; it does not ask what they came for.
- **The chat's first-time wizard** (`web/chat/chat-onboarding.js`): five steps about the identity
  that was just made, before anything else. Its first step says "Nobody reads your messages,
  they're encrypted." (line 139), which is not true of channel posts (section 4.1).
- **The How-to page's search** (`web/pages/how-to-app.js`, the rules in
  `web/shared/how-to-logic.js`) reads each how-to's title, summary, steps, its problems' signs and
  fixes ("people arrive with a symptom") and its tools. Six how-tos are written and 48 planned. It
  needs every word typed to appear, in the guides' own spelling, so plain words fail: **"my jar lid
  is bulging", "diarrhea", "throwing up", "power cut" and "my water is cloudy" each find nothing**
  (the ORS guide says "diarrhoea"; "my" and "is" are in no guide's words).
- **The Library** (`web/pages/library-app.js`): 268 documents in 25 shelves. Its full-text index
  `data/library/search-index.json` is now **9.5 MB** (its comment still says about 800 KB), fetched
  on the first key in its box. The front door must not fetch it.
- **#announcements:** one post per release since 2026-10-09,
  a rocket, the version in bold, a colon, the headline, a dot, and a "Release notes" link to the
  release on GitHub. The last 100 were read from the live server: **all 100 by the Deploy Bot,
  all 100 in that format, covering 2026-10-05 to 2026-10-25 (about five releases a day)**; the
  headline is 60 characters at the median, 82 at nine in ten, 103 at the longest.
- **The desktop app:** the Humanity page is to become the overview (answer 5 of
  [app-bottom-strip.md](app-bottom-strip.md), section 7), after a plan for the top menu, Play,
  Characters and the Characters page.

## 2. The layout

One page column for everything, so every block lines up with every other ("standard page width
aligned with other elements"): the site's usual 1,100 px (Humanity, Civilization, Governance,
Laws, Market, Trade, Wallet and Watch all use it), with a 16 px gutter.

```
 +------------------------- top menu (shell.js) ---------------------------+
 |                  END POVERTY. UNITE HUMANITY.     the mission, one line |
 |                          HumanityOS                                     |
 |  Free guides and tools for growing food, keeping water safe, making     |
 |  power and helping each other, and a space game that teaches the same   |
 |  skills.                                       what it IS, both halves  |
 |        No sign-up, no ads, nothing to buy. Open source, since 2019.     |
 |                                                                         |
 |   What do you need?                                                     |
 |   [ Try: cloudy water, a bulging jar lid, grow tomatoes          ]      |
 |                       [ New here? Start here ]                          |
 |                                                                         |
 |  [How-to guides] [The Library] [The game]      [Chat]                   |
 |  [Everyday tools][Why this exists][Decide together][What is being built]|
 |                                                                         |
 |  What changed lately                                     Every update > |
 |  [v0.1547.0 ...] [v0.1546.0 ...] [v0.1545.0 ...] [v0.1544.0 ...]        |
 |                                                                         |
 |  #general, as it happens  (Read only)            Join the conversation >|
 |  | Posts in this channel are shown on the website's front page.       | |
 |  | 12:39  Mara  Planted the first tomatoes on the windowsill ...      | |
 |  | ... 7 lines, as tall as the cards                                   | |
 |                                                                         |
 |  What brought you here?    (Start here: section 7)                      |
 +----------------- unfinished-software warning (shell.js) ----------------+
```

- **The title screen** is the top: the mission as a small line over the name (the same words as
  the Humanity page, key `humanity.mission`), then one sentence saying what HumanityOS IS, with
  both halves in it (the tools AND the game: the memory names that duality as the thing people
  fail to assemble), then the plain facts. Then the search, then "New here? Start here", then the
  buttons. Like a game's title screen, everything a person can do is on the first screen; unlike
  the current page, the useful thing (the search) comes before any persuasion.
- **The eight buttons**, all to pages that exist, each with its line and its hint (words
  `front_door.dest_*`), as data (`destinations.json`, a button is a row):

  | Button | Opens | The desktop's page |
  |---|---|---|
  | How-to guides | `/how-to` | HowTo |
  | The Library | `/library` | Library |
  | The game | `/download` | MainMenu (Play) |
  | Chat | `/?welcome=1&return=/chat` (the existing first-visit step) | Chat |
  | Everyday tools | `/apps` | Apps |
  | Why this exists | `/humanity` | Humanity |
  | Decide together | `/governance` | Governance |
  | What is being built | `/roadmap` | none (the roadmap is web only) |

  Four across on a desktop and a laptop, two on a tablet and a phone.
- **Under the title screen**, three sections in the same column: the update cards (section 3), the
  chat slice (section 4), Start here (section 7). Download is one click away (The game), not a
  second big button: the front door's one primary button is Start here, as the operator wanted.
- **What is gone** from the first screens: the clips, the picture, the six feature essays, the five
  adversaries, the proof bar. They belong on the pages the buttons open (the dawn clip on the game
  page or `/download`, "How the software fights back" in the Library beside the mission), which is
  the operator's call (decision 8.1).
- **Measured** (the prototype, screenshots in section 9): the whole page is **1,747 px tall at
  2000 x 1125 (1.6 screens) and 2,911 px on the phone (3.4 screens)**, against 8.6 and 11.3 for the
  current page. On the monitor the first screen holds the title, the search, Start here, all eight
  buttons and the update cards, with the chat slice's heading at the fold; on a 1366 x 768 laptop,
  the title, the search, Start here and the top of the buttons; on the phone, the title, the search
  and Start here, the buttons starting at the fold. (The monitor is 2000 x 1125; a browser's own
  bars and the taskbar take roughly 130 to 160 px of that, so in his browser the fold sits a little
  higher than the screenshots show.)

## 3. How many update cards: measured

**What a card holds:** the version, the date, the headline (up to five lines), and Release notes
(a link to the release on GitHub, only ever to `https://github.com/Shaostoul/Humanity/releases/tag/`:
the front door must not become a way to put any link before every visitor). About seven lines in
all, which is also the chat slice's height, as the operator asked.

**The narrowest readable card**, measured on the last 100 real headlines in the card's own font
(Noto Sans 0.98 rem, line height 1.45), lines each headline takes at each card width (`?measure=1`
on the prototype; the longest / nine in ten fit within / how many would be cut at four lines / at
five):

| Card width | Longest | 9 in 10 | Cut at 4 lines | Cut at 5 lines |
|---|---|---|---|---|
| 160 px | 7 | 6 | 42 | 19 |
| 180 px | 6 | 5 | 20 | 6 |
| 200 px | 6 | 5 | 10 | 2 |
| **220 px** | **5** | **4** | 1 | **0** |
| 240 px | 5 | 4 | 1 | 0 |
| 260 px | 4 | 3 | 0 | 0 |
| 300 px | 4 | 3 | 0 | 0 |
| 360 px | 3 | 2 | 0 | 0 |

So a card of **220 px with five headline lines** cuts none of the last 100; four lines would need
260 px and fewer cards.

**How many fit**, with that rule, at each window (the 1,100 px column, and for comparison the
widest page the site has, Apps at 1,560 px). Measured in the prototype, except the cells marked
"by the rule", which follow from the column's width by the same sum:

| Window | Column 1,100 | Column 1,560 |
|---|---|---|
| 2000 x 1125 (his monitor) | **4** cards, 258 px each | 6 cards, 245 px |
| 1920 x 1080 | 4 | 6 (by the rule) |
| 1366 x 768 (a common laptop) | **4**, 258 px | 5, 257 px |
| 1280 x 720 | 4 | 5 (by the rule) |
| 1024 x 768 | 4, 239 px | 4 (by the rule) |
| 768 x 1024 (a tablet upright) | 3, 237 px | 3 (by the rule) |
| 600 and 500 wide | 2 | 2 |
| 430, 390 and 360 wide (phones) | **a swipe row of 9**, 280 px (269 at 360), the next one peeking | the same |

Nine in a row needs at least 9 x 220 + 8 x 12 = 2,076 px of column: no screen in the table has
it, so nine side by side would mean cards of about 108 px in the standard column, unreadable.

**The rule recommended:** as many cards as fit at 220 px or more, at most nine, at least two side
by side; below two, a swipe row of the newest nine. It follows the screen without a list of
breakpoints, it never shows a half card, and it is one number in a data file
(`config.json`: `card_min_px`, `card_max`, `card_gap_px`, `swipe_card_px`). In the standard
column it shows 4 on every desktop and laptop, 3 on a tablet, a swipe row on a phone. If he wants
the row to grow with a big screen, the whole page takes the wide column (6 on his monitor, 5 on a
laptop) and everything stays aligned (decision 8.2). A fixed number was rejected: four is too many
for a phone and too few for nothing.

## 4. The #general slice is a privacy question

The 2026-10-09 rule wins here: "Security is definitely something we need to get right. We must
protect the users." Safe by default; people open up what they want.

### 4.1 Who can read #general today

- **Anyone on the internet, without signing in.** `GET /api/messages?channel=<any channel>`
  (`src/relay/api.rs` `get_messages`, routed in `src/relay/mod.rs` `build_router`) asks for no
  key and no signature; its only limit is the per-address Read allowance (240 at once, 1,200 a
  minute, `src/relay/ip_limit.rs`). For every post it returns the poster's **full public key**
  (`from`), their display name, the words, the time to the millisecond and the message id. Checked
  on the live server on 2026-11-02, reading counts only and not one person's words: #general
  answered **144 posts from 18 keys, 2026-05-21 to 2026-10-19, 22 of them in the last 30 days**.
- **The website shows it only to someone with a key.** `web/chat/index.html` sends a visitor with
  no key to `/?welcome=1`. But a key is free and made in a second, and the chat page itself loads
  each channel's history through the same open endpoint (`web/chat/app.js` `loadHistory`).
- **Every channel is the same.** No channel can be members only today: the same endpoint serves
  #local too (its promise is that it never leaves this server, a federation promise, not a privacy
  one). The Files page has a members-only switch (`media_visibility`); chat has none.
- **Nothing tells a poster this, and one line says the opposite.** The chat's first-time wizard
  says "Nobody reads your messages, they're encrypted." (`web/chat/chat-onboarding.js` line 139).
  That is true of direct messages (sealed) and false for channel posts, which the server keeps as
  plain text and serves to anyone. **This is work to do now**, separate from the front door
  (decision 8.5).

### 4.2 Does a front-page slice widen who reads people's words?

In practice, yes. Today a post in #general is read by people who went into the chat, plus anyone
who knows the API. On the front door it is put before every visitor, every screenshot and link
preview, and every search engine that runs a page's script; and the 144 earlier posts were all
made by people who were told nothing of it. Posting in a room you walked into is not consent to a
billboard.

### 4.3 The safe design (recommended as the default)

1. **Off until an admin turns it on.** A channel setting, "Shown on the website's front page", in
   Server Settings on both apps (GUI first), off for every channel, at most one channel at a time.
2. **Only posts made after it was turned on.** The setting is a time, not a yes, so none of
   #general's earlier posts ever shows.
3. **Everyone posting there is told, before they post.** A line above the message box in that
   channel ("Posts in this channel are shown on the website's front page, to anyone.", key
   `front_door.chat_public_note`, already in the prototype), and the first time a person posts
   there, a one-time OK. The relay keeps that OK per key and per channel and refuses a post from a
   key without it, so an older app cannot post there unaware. Posting after being told is the
   person's own choice, one post at a time; deleting a post takes it back.
4. **A hold:** a post shows only after 30 minutes (`hold_minutes`), so a moderator can remove
   abuse first. A post deleted, reported or hidden in that time never shows; an edit starts the
   hold again; a post deleted or hidden later leaves the front page on the next load.
5. **A new account waits:** no post from a key that joined less than 7 days ago
   (`registered_names.registered_at` holds the time). The front door is the worst place for spam.
6. **What shows of a poster:** the display name and nothing else (no key, no picture, no link to a
   profile); the words as plain text, one line (two on a phone), cut at 200 characters, links not
   clickable, no files, pictures or mentions; the time to the minute.
7. **Moderators get "Take off the front page"** beside delete, for a post that may stay in the
   channel but not on the door.
8. **The server enforces it, not the page:** the endpoint (section 5) serves only what these rules
   allow, so no page can be made to show more.

Recommended with it: a **new channel made for the front page** rather than #general, so #general
keeps the expectation its 144 posts were made under. Marking #general from now on also works
under rule 2.

**#announcements does not need the consent machinery**: the Deploy Bot posts it through the bot
secret, one line per release, no person's words. It still needs the filter: the feed takes only
the bot's and the server's own posts in that channel (a fresh server seeds #announcements read
only, `src/relay/mod.rs`, but admins and moderators can post in a read-only channel, and whether
the live channel is read only could not be checked), parses each into version, headline and link,
keeps only a link to the project's own release page, and drops anything else.

### 4.4 What the operator decides

Whether the front door has a chat slice at all; which channel (a new one, recommended, or #general
from now on); the hold (30 minutes proposed); the name shown (recommended) or "A member"; the
new-account wait (7 days proposed); and whether the wider gap, every channel readable by anyone
without signing in, becomes its own work (recommended: a chat setting like `media_visibility`,
public or members, with history over signed requests on a members-only server). Decision 8.3.

## 5. Where the data comes from

- **There today:** `GET /api/messages?channel=announcements&limit=9`, open to anyone, so the
  update cards can be built now; the prototype reads it. Nothing serves a chat channel under the
  rules of section 4.3.
- **Missing: one route, `GET /api/front-door`**, answering
  `{ updates: [{ version, title, url, at }], chat: { channel, lines: [{ name, text, at }] } | null }`:
  at most nine of each, every rule of 4.3 applied in the server. Feature `chat` in
  `src/relay/features.rs`'s route table; the existing Read allowance per address (no new limit
  needed: the page asks once per visit, and again only when a tab comes back after ten minutes,
  never by a timer). Two indexed reads per request (`idx_messages_channel` on
  `messages(channel_id, id)` exists), so **no cache**: CLAUDE.md's no-caches rule forbids one in our
  code before launch, and nginx's cache in front would be one we configure, so none there either.
- **Storage it adds:** the channel's front-page time (a column added by ALTER, never referenced
  from the main schema batch: BUG-046, `tests/schema_alter_order_lint.rs`), and the one-time OKs
  (a small table: channel, key, time). `migration-guard` reviews both.
- **The search** reads, on the first key and never on load: the How-to index (12.9 KB), the
  planned list (10.1 KB), the Library's manifest (66 KB, titles and tags) and the plain-words file
  (`search-words.json`, section 7.4), about 90 KB; never the Library's 9.5 MB full-text index.
  "Search the whole Library" hands the words to `/library?q=`, which the Library page has to learn
  to read (it opens on `#slug` only today).
- **No third party:** the front door asks nothing of GitHub or any other site, so no visitor's
  address leaves our servers (the self-hosted fonts' reasoning, `web/shared/theme.css`). The
  Devlog page does ask GitHub's API from each visitor's browser (`web/pages/devlog-app.js`), worth
  knowing when the cards' "Every update" link points there.

## 6. What the desktop's Humanity page shares with it

The Humanity page is becoming the overview (answer 5); native first, so each pattern below is
built in egui and mirrored, not the other way round.

- **The destinations:** one file (`data/front_door/destinations.json`) whose rows carry both the
  website's address and the desktop's page (`native`), so the overview's buttons and the front
  door's are one list.
- **The update cards:** one widget, `src/gui/widgets/update_card.rs` and a matching CSS class, both
  on theme tokens; the same answer from `/api/front-door` of the server the app is connected to;
  the same count rule on the panel's width (`card_min_px`, `card_max`).
- **The chat slice:** the same endpoint and rules, read only; it shows what the world sees. Talking
  stays in the chat footer (desktop only, answer 4).
- **The search:** the same plain-words file and the same matching, in the desktop's How-to page
  (`src/how_to/view.rs`) and the website's, held to one fixture of queries and expected guides by
  both apps' tests, the way the how-to checkers share `tests/fixtures/how_to/broken.json`.
- **Start here:** the goals file and its words (`front_door.goal_*`); on the desktop, "What brought
  you here?" can be the first card of the first minutes (`src/gui/first_minutes.rs`), the quest
  chains already being the Tasks page's suggested cards.
- **Not on the front door:** the demo character's Overview (app-bottom-strip 5.5). The front door
  shows real activity instead (updates, people talking); a pretend character there would need its
  own "Example" label on every tile and answers decision 7 of that design, which stays his.

## 7. Start here, made better than the onboarding page

### 7.1 What the onboarding page does today, and where it loses a newcomer

- **It never asks what the person came for.** Everything is offered at equal weight: four
  pillars, eight tools, six chains.
- **It explains before it helps.** The first useful thing, the quest chains, starts **1,995 px
  down on the monitor (1.8 screens) and 3,251 px down on the phone (3.9 screens)**, under two
  sections of paragraphs.
- **Its tools lead with the app, not the need:** Chat, Profile, Wallet and Settings are half the
  tiles; the chat needs a key, and its first screen is the identity wizard.
- **It has no search,** so a person with a problem ("the jar lid is bulging") has no way in.
- **It does not remember the person,** beyond ticked steps.
- Its words are written into the page rather than keyed.

### 7.2 What was weighed

- **A. Ask what they came for** and answer within one click.
- **B. Search first:** one box that reads a symptom or a goal in plain words.
- **C. Reorder the onboarding page:** chains first. Cheaper, still no question and no search.
- **D. A tour:** exists (the corner card) and orients a person in the app, not in their need.

### 7.3 Recommended: A, with B beside it

"What brought you here?" with seven answers, each a row of data (`goals.json`): Grow food, Safe
water, Power at home, Someone is sick or hurt, Play the game, Meet people, Just looking. A pick
shows at once, on the same page:

- one sentence of answer, and **one** first thing to open (a How-to, a Library guide or a page),
  with no sign-up for any of them;
- **then, step by step:** the matching quest chain from `data/onboarding/quests.json` with "Step 1
  of 4", its meter and its next step, sharing the Onboarding page's progress
  (`humanity_quest_progress`), so a tick in either place is a tick in both;
- on "Someone is sick or hurt", before anything else: "If someone is in danger right now, call your
  local emergency number first.";
- remembered on this device (`hos_front_door_goal`): a return visit opens on "Welcome back. You
  picked Safe water." with the step reached, and "Pick something else".

The search sits above it on the same page (B). The Onboarding page then becomes the view of every
step (the cards' "See the whole chain"); its pillar paragraphs move to the Library or go.

**Better how:** the first useful thing is at 384 px on the monitor and 434 px on the phone (the
search) instead of 1,995 and 3,251; one question replaces four sections; nothing needs a key.

### 7.4 The search reads plain words

`search-words.json` holds rows of words that mean the same ("diarrhea", "diarrhoea", "loose
stool"; "throw up", "vomit"; "power cut", "blackout") and a short list of small words to drop
("my", "is", "the"). A query becomes groups, each matched by any word in it; every group must
match. The How-to page's own search against the prototype's, on the same data:

| Typed | How-to page today | Front door |
|---|---|---|
| my jar lid is bulging | nothing | Test the seals on home-canned jars |
| diarrhea | nothing | Mix oral rehydration solution from a packet |
| throwing up | nothing | Mix oral rehydration solution from a packet |
| power cut | nothing | The first hour of a power cut (planned, said so) |
| my water is cloudy | nothing | Boil water to make it safe to drink |
| grow tomatoes | (no how-to) | Your First Tomato (Library) |

A row must hold only words that truly mean the same: "runs" sat in the diarrhoea row for one
screenshot and sent "diarrhea" to the furnace filter first ("so it runs as it should"). The list
is English; a translation brings its own.

## 8. Decisions for the operator

1. **Adopt this as the front door**, in place of the seven-screen page? Then where the clips and
   "How the software fights back" go (proposed: the game's page or `/download`, and the Library).
2. **The column:** the standard 1,100 px (4 cards on his monitor and on a laptop), or the wide
   1,560 px (6 and 5). The card rule is the same either way.
3. **The chat slice** (section 4.4): at all; which channel; the hold; names; the new-account wait;
   and members-only chat as its own work.
4. **Start here's seven answers** and the first thing each opens: his eye and a reviewer's on each
   answer before it goes live (teaching data needs a refutation pass, memory
   `project_teaching_data_needs_refutation`).
5. **The wizard's "Nobody reads your messages, they're encrypted."**: proposed to fix now, by
   saying which messages are sealed (direct messages) and which are public (channels).
6. **The cold-reader test:** hand three people the phone with the prototype's first screen and ask
   what it is and what they would do with it; their wrong guesses locate what is still unclear.
   Only a person can run it.

## 9. The prototype

`web/prototypes/front-door/`: `index.html` and `app.js`, and its data: `config.json` (the
numbers), `destinations.json`, `goals.json`, `search-words.json`, `sample-updates.json` (nine real
announcements, shown only when the feed cannot be reached, labelled as samples). Every word is a
key (`front_door.*` in `data/i18n/en.json`, 116 keys), every control has a hint, theme tokens only;
the i18n, hints, accessibility and glossary web tests pass with it.

**Kept off the live site:** `web/prototypes/` is outside what the deploy copies
(`scripts/sync-web-root.sh` copies `web/pages/` and `web/shared/`), it is not a page of the
registry, and it carries `noindex`. It posts nothing and shows **example** chat lines only, never a
real person's words.

**To open it:** `node scripts/preview-server.js 8099`, then
`http://127.0.0.1:8099/prototypes/front-door/`. Without a relay the update cards are the samples,
labelled so. The screenshots used a scratch server answering only the announcements history, from
the 100 posts read from the live server on 2026-11-02. Address options: `?theme=light` (the light
theme for this visit only, without changing the person's setting), `?col=wide`, `?measure=1` (the
readout and the headline measurement), `?min=<px>`, `?q=<words>` (the search answering),
`?goal=<id>` (Start here with a pick).

**The screenshots** (in the session's scratchpad, prefixed `frontdoor-`): `2000x1125-dark`,
`2000x1125-light`, `1366x768-dark`, `1366x768-light`, `390x844-dark`, `390x844-light`,
`2000x1125-wide-column-dark`, the whole page at `2000`, `1366` (light) and `390`, and the search at
`1366x768` ("my jar lid is bulging") and `390x844` ("diarrhea"). Taken by headless Edge through its
DevTools protocol, each at an exact viewport.

**Not to be merged as is.** The real build differs: section 10.

## 10. What the real build takes, in rungs

1. **The wizard's false line** (web, one string; the desktop's twin checked for the same claim).
2. **`GET /api/front-door`, updates only:** the filter and parse of 4.3's last paragraph, its route
   in `features.rs`, relay tests (`just verify-relay`), the relay check.
3. **The page:** `web/pages/index.html` becomes the front door (title screen, search, Start here,
   the update cards); its data in `data/front_door/` (`config.json`, `destinations.json`,
   `goals.json`, `search_words.json`); the words from this prototype; web tests for the card rule
   (widths in, counts out), the search (the fixture of 7.4) and Start here (the progress shared
   with the Onboarding page); `docs/PAGES.md`. The current page's clip and essays move (decision
   8.1).
4. **The Library reads `?q=`.**
5. **The desktop's overview:** the update card widget and the destinations, native first, with
   snapshots; the shared search fixture.
6. **The chat slice,** only after decision 8.3: the channel's front-page time (ALTER, the
   migration guard), its control in Server Settings on both apps, the notice and the one-time OK
   (both apps), the OK table, the hold, the new-account wait, "Take off the front page" for
   moderators, the endpoint's chat part, tests on a fresh and an upgraded database.
7. **Members-only chat,** if he wants it (4.4).

## 11. What could not be found out

- Whether the live #announcements channel is read only (no public route lists channel flags).
- How people read the prototype: only the cold-reader test (8.6) can say.
- The exact fold in his browser on his monitor (its bars and the taskbar are not in the
  screenshots).
- Whether search engines have indexed `/api/messages`: the live site has no `robots.txt` (404),
  which neither invites nor forbids it.
