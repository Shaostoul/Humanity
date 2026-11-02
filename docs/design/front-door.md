# The website's front door: a title screen with tools in it

**Status (2026-11-02):** designed, with a prototype the operator can open on his screen and his
phone (`web/prototypes/front-door/`, section 9). Revised the same day after his three answers:
**no chat on the front page** (section 4), **a signed release's card dressed its own way**
(section 3.1), and his real screen, **2560 x 1440 on a 65-inch TV** (section 3.2). Nothing here
replaces the live front page (`web/pages/index.html`), and nothing is decided for the operator
except where his own words say so; what is still his is in section 8. This answers decision 6 of
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

2026-11-02, his answers to the first version of this design:

> That's fair. We can just exclude the chat on front page.

> Can we also catch we I sign a version? Like normal updates are one color or something and then
> when I sign its widget dressing is slightly different from the normal update ones. Or something
> like that?

And his screen, relayed the same day: 2560 x 1440, a 65-inch TV used as his monitor (his
screenshots arrive downscaled to 2000 x 1125).

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
  a proof bar, "Come build it with us". Measured in the same browser as the prototype: **11,699 px
  tall at 2560 x 1440 (8.1 screens) and 9,528 px on a 390 x 844 phone (11.3 screens)**. Nothing on
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
  that was just made, before anything else. It said "Nobody reads your messages, they're
  encrypted.", untrue of channel posts; fixed the same day (BUG-223, section 4.1).
- **The How-to page's search** (`web/pages/how-to-app.js`, the rules in
  `web/shared/how-to-logic.js`) reads each how-to's title, summary, steps, its problems' signs and
  fixes ("people arrive with a symptom") and its tools. Six how-tos are written and 48 planned. It
  needs every word typed to appear, in the guides' own spelling, so plain words fail: **"my jar lid
  is bulging", "diarrhea", "throwing up", "power cut" and "my water is cloudy" each find nothing**
  (the ORS guide says "diarrhoea"; "my" and "is" are in no guide's words).
- **The Library** (`web/pages/library-app.js`): 268 documents in 25 shelves. Its full-text index
  `data/library/search-index.json` is now **9.5 MB** (its comment still says about 800 KB), fetched
  on the first key in its box. The front door must not fetch it.
- **#announcements:** one post per release since 2026-10-09: a rocket, the version in bold, a
  colon, the headline, a dot, and a "Release notes" link to the release on GitHub. The last 100
  were read from the live server: **all 100 by the Deploy Bot, all 100 in that format, covering
  2026-10-05 to 2026-10-25 (about five releases a day)**; the headline is 60 characters at the
  median, 82 at nine in ten, 103 at the longest. The post is made when the release is built, so it
  never says whether the release was later signed.
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
 |  [v0.1547.0 Signed] [v0.1546.0 ...] [v0.1545.0 ...] [v0.1544.0 ...]     |
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

  Four across on a desktop and a laptop, two on a tablet and a phone. The Chat button stays: it
  opens the chat page; what the operator excluded is a slice of the chat shown on this page.
- **Under the title screen**, two sections in the same column: the update cards (section 3) and
  Start here (section 7). Download is one click away (The game), not a second big button: the
  front door's one primary button is Start here, as the operator wanted.
- **What is gone** from the first screens: the clips, the picture, the six feature essays, the five
  adversaries, the proof bar. They belong on the pages the buttons open (the dawn clip on the game
  page or `/download`, "How the software fights back" in the Library beside the mission), which is
  the operator's call (decision 8.1).
- **Measured** (the prototype, screenshots in section 9): the whole page, Start here included, is
  **1,440 px tall at 2560 x 1440 (exactly one screen), 1,402 px on a 1366 x 768 laptop (1.8
  screens) and 2,246 px on the phone (2.7 screens)**, against 8.1 and 11.3 for the current page.
  On his screen at 100% scaling everything shows at once; on the laptop, the title, the search,
  Start here and the top of the buttons; on the phone, the title, the search and Start here, the
  buttons starting at the fold.

## 3. The update cards

**What a card holds:** the version, the date, the headline (up to five lines), and Release notes
(a link to the release on GitHub, only ever to `https://github.com/Shaostoul/Humanity/releases/tag/`:
the front door must not become a way to put any link before every visitor). About seven lines in
all.

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
260 px and fewer cards. (Widths in CSS pixels: with display scaling, a CSS pixel is drawn on more
than one screen pixel, section 3.2.)

**How many fit**, with that rule, at each window (the 1,100 px column, and for comparison the
widest page the site has, Apps at 1,560 px), in CSS pixels. Measured in the prototype, except the
cells marked "by the rule", which follow from the column's width by the same sum:

| Window (CSS px) | Column 1,100 | Column 1,560 |
|---|---|---|
| **2560 x 1440 (his screen at 100%)** | **4** cards, 258 px each | **6**, 245 px |
| 2000 x 1125 | 4 | 6 |
| 1920 x 1080 | 4 | 6 (by the rule) |
| **1707 x 960 (his screen at 150%)** | 4 | 6 |
| 1366 x 768 (a common laptop) | **4**, 258 px | 5, 257 px |
| **1280 x 720 (his screen at 200%)** | 4 | 5, 240 px |
| 1024 x 768 | 4, 239 px | 4 (by the rule) |
| 768 x 1024 (a tablet upright) | 3, 237 px | 3 (by the rule) |
| 600 and 500 wide | 2 | 2 |
| 430, 390 and 360 wide (phones) | **a swipe row of 9**, 280 px (269 at 360), the next one peeking | the same |

Nine in a row needs at least 9 x 220 + 8 x 12 = 2,076 px of column. Neither column has it; only a
column the full width of a screen 2,108 px wide or more would (his screen at 100%: 9 cards of
270 px, by the rule), and that column would no longer line up with the rest of the site.

**The rule recommended:** as many cards as fit at 220 px or more, at most nine, at least two side
by side; below two, a swipe row of the newest nine. It follows the screen without a list of
breakpoints, it never shows a half card, and it is one number in a data file
(`config.json`: `card_min_px`, `card_max`, `card_gap_px`, `swipe_card_px`). In the standard
column it shows 4 on every desktop and laptop, his screen included at any scaling, 3 on a tablet,
a swipe row on a phone. If he wants the row to grow with a big screen, the whole page takes the
wide column (6 on his screen at 100% or 150%, 5 at 200% and on a laptop) and everything stays
aligned (decision 8.2). A fixed number was rejected: four is too many for a phone and too few for
nothing.

### 3.1 A signed release looks different

The operator asked for it (section 0). What "signed" means today, read from the code, not
guessed:

- **The mark of a signed release** is two files on its GitHub release, `release-manifest.json`
  (the version and every platform binary's SHA-256) and `release-manifest.json.sig.json` (an
  Ed25519 and a Dilithium3 signature over the manifest's exact bytes). `just sign-release vX.Y.Z`
  (the Justfile) builds the manifest with `scripts/make-release-manifest.js`, signs it with the
  passphrase-locked key, and uploads both (`docs/admin/release-signing.md`).
- **What the desktop app does with them** (`src/updater.rs` `evaluate_update`): it offers as an
  update only a release that is not a pre-release or a draft and carries `release-manifest.json`,
  and before installing it checks both signatures against the public keys compiled into it
  (`data/release/signing_pubkeys.json`, `src/release_update.rs` `verify_manifest_bytes`) and the
  download's SHA-256 against the manifest. So for a person, signed means: this version is one the
  desktop app will offer, and it checked that the project's keys made it.
- **Where the front door could learn it:**
  - The Deploy Bot's post: no. It is posted when the release is built, before anyone signs it.
  - GitHub's releases API from the visitor's browser: no. Every visitor's address would go to
    GitHub (the reason the fonts are self-hosted), and GitHub allows a browser 60 such requests an
    hour.
  - **The relay, checking the signatures itself (recommended).** `just sign-release` adds one
    step: it sends the same two files to the server (`POST /api/release-signature`). The relay
    keeps them only if both signatures verify against the same compiled-in public keys, records
    the version and the time, and `/api/front-door` gives that update `signed: true` (section 5).
    The route needs no secret, because only the project's keys can make a pair that passes, and a
    forged "signed" mark is impossible rather than merely unlikely. A one-time backfill sends the
    pairs already on GitHub. **It needs the signature check in the server build:**
    `release_update` is native-only today and placed in the GUI crate by the crate split ("native
    only, never called by the server", `docs/design/crate-split.md`), so its verify half (the
    public keys and `verify_manifest_bytes`) would move to a crate the relay links, a change to
    that plan (decision 8.4).
  - A weaker fallback: `just sign-release` tells the relay with the bot secret, and the relay
    trusts it. A stolen secret could then put the mark on a fake, though the desktop app would
    still refuse to install it.
- **The dressing** (the prototype, from theme tokens only): a signed card's top band takes the
  theme's verified colour (`--badge-verified`, the colour the chat gives a verified person)
  instead of the accent, the card gets a faint wash of the same colour, and a small "Signed" mark
  with a shield sits beside Release notes. The mark carries the word as well as the colour, the
  keyboard reaches it, and its hint (`front_door.signed_hint`) says what signed means for a
  person: "The project signed this version with its release keys. The desktop app offers only
  signed versions as updates, and checks the signature before it installs one." A card that is not
  signed stays exactly as it was. The mark's colour against its card, computed from the tokens:
  5.90 to 1 in the dark theme and 4.84 to 1 in the light (WCAG AA asks 4.5); the real build adds
  the pair to `scripts/tests/contrast.test.js`.
- **In the prototype**, the dressing shows on the real signed releases among the cards:
  **v0.1547.0 (signed 2026-10-28) and v0.1543.0 (signed 2026-10-23)**, read from the releases'
  files on GitHub on 2026-11-02 into `signed-releases.json`, which stands in for the endpoint's
  `signed` field. In the standard column v0.1547.0 is the first card; the wide column shows both.

### 3.2 On his screen: 2560 x 1440 on a 65-inch TV

- **The size of a pixel:** a 65-inch 16:9 panel is 1,439 x 809 mm, so at 2560 x 1440 one screen
  pixel is 0.56 mm.
- **The type, in CSS pixels** (measured in the prototype): the name 70.4, the "what it is" line
  20.5, the search box 17.3, a button's name 16, a card's headline 15.7, a button's line 13.8, the
  Release notes link 13.6, the Signed mark 12. At 100% scaling a card headline is 15.7 screen
  pixels, 8.8 mm on his screen.
- **The reference** (Microsoft, "Designing for Xbox and TV", archived UWP guidance, `ms.date`
  2020-09-24, read 2026-11-02,
  <https://learn.microsoft.com/en-us/previous-versions/windows/uwp/xbox-apps/designing-for-tv>): the couch is
  "approximately 10 feet away from the screen"; a TV app's interface is scaled "at a default of
  **200%** for XAML apps and **150%** for HTML apps" on a 1920 x 1080 picture; and the "rules of
  thumb" are "Main text and reading content: 15 epx minimum" and "Non-critical text and
  supplemental content: 12 epx minimum". For an HTML page that is main text at least 22.5 of 1,080
  rows (2.1% of the picture's height, 16.9 mm on his TV) and other text at least 18 rows (1.7%,
  13.5 mm).
- **So at 100% scaling, no:** a card headline is 1.1% of the picture's height, about half the
  10-foot minimum, and the button lines and the Signed mark are smaller still. At a desk distance
  it reads; from a sofa it does not.
- **At 200% scaling, yes:** Windows' display scale at 200% (or the browser's zoom at 200%, or the
  site's own font size in Settings, which sets the root size every `rem` on the page follows) makes
  the page a 1280 x 720 layout drawn at two screen pixels each: a card headline becomes 31.4 screen
  pixels (2.2%, 17.6 mm), above the HTML minimum, and the Signed mark 24 (1.7%), exactly at the
  minimum for other text. The page reflows to the laptop layout: 4 cards (5 in the wide column),
  the update row just under the first screen. At 150% (1707 x 960) the headline is 1.6%, still
  under it.
- **Recommendation:** do not make the page guess at a TV (a 2560-wide window is as likely a
  27-inch monitor at a desk); the scaling is the person's, and the 200% shots show the page holding
  up under it. For his own sofa use, Windows' 200% scale (Settings > System > Display) is the
  setting that meets the 10-foot guidance on every page, not only this one.

## 4. No chat on the front page (decided 2026-11-02)

The operator, after reading section 4.1: "That's fair. We can just exclude the chat on front page."
So the front door shows no channel's posts, and the prototype's read-only slice of #general is
gone (its 21 words are out of `en.json`). The Chat **button** stays, opening the chat page. The
findings below are kept because they still matter: they are the evidence behind BUG-223's open
question of whether channel reading becomes members-only.

### 4.1 Who can read #general today (found 2026-11-02)

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
- **The wizard said the opposite**: "Nobody reads your messages, they're encrypted." True of direct
  messages (sealed), false for channel posts. Fixed the same day (BUG-223, 06ce44527): the line now
  says direct messages are sealed and channel posts are public.

### 4.2 Why a front-page slice would have widened who reads people's words

Today a post in #general is read by people who went into the chat, plus anyone who knows the API.
On the front door it would be put before every visitor, every screenshot and link preview, and
every search engine that runs a page's script; and the 144 earlier posts were all made by people
who were told nothing of it. Posting in a room you walked into is not consent to a billboard.

### 4.3 The safe design, kept for the record

If a chat slice is ever reopened, this was the safe default the operator weighed: off until an
admin marks a channel (at most one, set in Server Settings on both apps); only posts made after the
mark; everyone posting there told before they post, with a one-time OK the relay enforces; a
30-minute hold so a moderator can remove abuse first, an edit restarting it; no post from a key
joined less than 7 days ago (`registered_names.registered_at`); the display name only, plain text,
links not clickable, nothing else of the poster; "Take off the front page" for moderators; every
rule applied in the server. Better still, a channel made for the purpose rather than #general.

### 4.4 #announcements still needs a filter

The update cards are #announcements, posted by the Deploy Bot through the bot secret, one line per
release, no person's words: no consent machinery. The feed still takes only the bot's and the
server's own posts in that channel (a fresh server seeds #announcements read only,
`src/relay/mod.rs`, but admins and moderators can post in a read-only channel, and whether the live
channel is read only could not be checked), parses each into version, headline and link, keeps
only a link to the project's own release page, and drops anything else.

### 4.5 Still open, under BUG-223

Whether channel reading becomes members-only by default, an admin able to mark a channel public (a
chat setting like `media_visibility`, with history over signed requests on a members-only server),
and a `robots.txt` keeping search engines off `/api/` (the live site has none). The front door no
longer depends on either.

## 5. Where the data comes from

- **There today:** `GET /api/messages?channel=announcements&limit=9`, open to anyone, so the
  update cards can be built now; the prototype reads it.
- **Missing: one route, `GET /api/front-door`**, answering
  `{ updates: [{ version, title, url, at, signed }] }`: at most nine, the filter of section 4.4
  applied in the server, `signed` from the signatures the relay has checked (section 3.1). Feature
  `chat` in `src/relay/features.rs`'s route table (it reads the chat's #announcements); the
  existing Read allowance per address (no new limit needed: the page asks once per visit, and
  again only when a tab comes back after ten minutes, never by a timer). One indexed read per
  request (`idx_messages_channel` on `messages(channel_id, id)` exists) and one small lookup of
  signed versions, so **no cache**: CLAUDE.md's no-caches rule forbids one in our code before
  launch, and nginx's cache in front would be one we configure, so none there either.
- **And `POST /api/release-signature`** (section 3.1): a manifest and its signature file in,
  checked against the compiled-in public keys, kept only when both signatures verify. Storage: a
  small table of signed versions (version, the manifest's hash, when it was checked), created in
  the main schema batch as a new table (no ALTER, so BUG-046's order rule is not in play);
  `migration-guard` reviews it.
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
  on theme tokens, the signed dressing included (the verified band and the Signed mark with its
  hint by key); the same answer from `/api/front-door` of the server the app is connected to; the
  same count rule on the panel's width (`card_min_px`, `card_max`). The desktop already checks
  signatures itself in its updater, so its card can say Signed from its own check.
- **No chat slice on either** (section 4). Talking stays in the chat, and on the desktop in the
  chat footer (desktop only, answer 4).
- **The search:** the same plain-words file and the same matching, in the desktop's How-to page
  (`src/how_to/view.rs`) and the website's, held to one fixture of queries and expected guides by
  both apps' tests, the way the how-to checkers share `tests/fixtures/how_to/broken.json`.
- **Start here:** the goals file and its words (`front_door.goal_*`); on the desktop, "What brought
  you here?" can be the first card of the first minutes (`src/gui/first_minutes.rs`), the quest
  chains already being the Tasks page's suggested cards.
- **Not on the front door:** the demo character's Overview (app-bottom-strip 5.5). The front door
  shows real activity instead (the updates); a pretend character there would need its own
  "Example" label on every tile and answers decision 7 of that design, which stays his.

## 7. Start here, made better than the onboarding page

### 7.1 What the onboarding page does today, and where it loses a newcomer

- **It never asks what the person came for.** Everything is offered at equal weight: four
  pillars, eight tools, six chains.
- **It explains before it helps.** The first useful thing, the quest chains, starts **1,995 px
  down on a 2000 x 1125 screen (1.8 screens) and 3,251 px down on the phone (3.9 screens)**, under
  two sections of paragraphs.
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

**Better how:** the first useful thing (the search) is at 384 px on a 2000 x 1125 screen and
434 px on the phone, instead of 1,995 and 3,251; one question replaces four sections; nothing
needs a key.

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

**Answered 2026-11-02:** no chat on the front page (section 4); a signed release's card dressed
its own way (section 3.1).

Still his:

1. **Adopt this as the front door**, in place of the seven-screen page? Then where the clips and
   "How the software fights back" go (proposed: the game's page or `/download`, and the Library).
2. **The column:** the standard 1,100 px (4 cards on his screen at any scaling and on a laptop), or
   the wide 1,560 px (6 on his screen at 100% or 150%, 5 at 200% and on a laptop). The card rule
   is the same either way.
3. **The signed dressing as shown** (the verified band, the wash, the Signed mark), or another
   look; and whether the Devlog page's list carries the same mark.
4. **Where the server learns a release is signed:** the relay checking the signatures itself
   (recommended), which moves the verify half of `release_update` into a crate the relay links,
   a change to the crate split's placement; or the bot-secret fallback.
5. **Start here's seven answers** and the first thing each opens: his eye and a reviewer's on each
   answer before it goes live (teaching data needs a refutation pass, memory
   `project_teaching_data_needs_refutation`).
6. **The cold-reader test:** hand three people the phone with the prototype's first screen and ask
   what it is and what they would do with it; their wrong guesses locate what is still unclear.
   Only a person can run it.

## 9. The prototype

`web/prototypes/front-door/`: `index.html` and `app.js`, and its data: `config.json` (the
numbers), `destinations.json`, `goals.json`, `search-words.json`, `signed-releases.json` (which
releases are signed, a 2026-11-02 snapshot standing in for the endpoint), `sample-updates.json`
(nine real announcements, shown only when the feed cannot be reached, labelled as samples). Every
word is a key (`front_door.*` in `data/i18n/en.json`, 97 keys), every control has a hint, theme
tokens only; the i18n, hints, accessibility and glossary web tests pass with it.

**Kept off the live site:** `web/prototypes/` is outside what the deploy copies
(`scripts/sync-web-root.sh` copies `web/pages/` and `web/shared/`), it is not a page of the
registry, and it carries `noindex`. It posts nothing.

**To open it:** `node scripts/preview-server.js 8099`, then
`http://127.0.0.1:8099/prototypes/front-door/`. Without a relay the update cards are the samples,
labelled so. The screenshots used a scratch server answering only the announcements history, from
the 100 posts read from the live server on 2026-11-02. Address options: `?theme=light` (the light
theme for this visit only, without changing the person's setting), `?col=wide`, `?measure=1` (the
readout and the headline measurement), `?min=<px>`, `?q=<words>` (the search answering),
`?goal=<id>` (Start here with a pick).

**The screenshots** (in the session's scratchpad, prefixed `frontdoor-`):
- his screen: `2560x1440-dark`, `2560x1440-light`, `2560x1440-wide-column-dark`,
  `2560x1440-wide-column-light`, the update row up close (`2560x1440-cards-closeup-dark` and
  `-light`, the wide column, both signed cards), the whole page (`2560x1440-whole-page-dark`), and
  the same screen at 200% scaling (`2560x1440-at-200pct-dark` and `-light`);
- a laptop: `1366x768-dark`, `1366x768-light`, `1366-whole-page-light`;
- a phone: `390x844-dark`, `390x844-light`, `390-whole-page-dark`;
- the search: `1366x768-search-dark` ("my jar lid is bulging"), `390x844-search-light`
  ("diarrhea").

Taken by headless Edge through its DevTools protocol, each at an exact viewport. The earlier
2000 x 1125 set, which still showed the chat slice, was removed.

**Not to be merged as is.** The real build differs: section 10.

## 10. What the real build takes, in rungs

1. **`GET /api/front-door`, updates:** the filter and parse of section 4.4, its route in
   `features.rs`, relay tests (`just verify-relay`), the relay check.
2. **The page:** `web/pages/index.html` becomes the front door (title screen, search, Start here,
   the update cards); its data in `data/front_door/` (`config.json`, `destinations.json`,
   `goals.json`, `search_words.json`); the words from this prototype; web tests for the card rule
   (widths in, counts out), the search (the fixture of 7.4) and Start here (the progress shared
   with the Onboarding page); `docs/PAGES.md`. The current page's clip and essays move (decision
   8.1).
3. **The signed mark:** after decision 8.4, the verify half of `release_update` where the relay
   can link it, `POST /api/release-signature` and its table, one step added to `just
   sign-release`, a one-time backfill of the pairs already on GitHub, `signed` in the endpoint,
   the dressing on the page, its contrast pair in `contrast.test.js`.
4. **The Library reads `?q=`.**
5. **The desktop's overview:** the update card widget (the signed dressing included) and the
   destinations, native first, with snapshots; the shared search fixture.

## 11. What could not be found out

- Whether the live #announcements channel is read only (no public route lists channel flags).
- How people read the prototype: only the cold-reader test (8.6) can say.
- Which display scaling his PC uses on the TV, so whether he sees the 100% or the 200% shots.
- Whether search engines have indexed `/api/messages`: the live site has no `robots.txt` (404),
  which neither invites nor forbids it.
