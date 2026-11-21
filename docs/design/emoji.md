# Emoji: one set of pictures for both apps

**Started 2026-11-20.** The operator: emoji are first an open set we host ourselves, chosen after the
house comparison ("of that list the OpenMoji sounds better"), then a HumanityOS set drawn by
volunteers, each design proposed and voted on, replacing the open set's emoji one by one
(`docs/PRIORITIES.md` 6b, `docs/outreach/volunteers-wanted.md`, "Emoji designers"). Licence and
credit: `docs/reference/findings/2026-11-20-openmoji.md`.

## The pack (built)

- `assets/emoji/<set>/<CODEPOINTS>.svg`, one picture per emoji, named by code points in upper-case
  hex joined by `-` (OpenMoji's naming). `assets/` is the shared media folder: the website gets it
  whole (`scripts/sync-web-root.sh` rsyncs `assets/` to the web root's `/assets/`, the way the icons
  reach it), and the desktop app reads `assets/` beside the exe. One copy serves both apps.
- `assets/emoji/sets.json`: the sets in look-up order; the first set holding an emoji draws it.
  Today only `openmoji` (OpenMoji 17.0.0, 4,495 pictures, CC BY-SA 4.0). The volunteers' set goes
  AHEAD of it as its own folder (CC0, drawn from scratch), so an adopted design replaces exactly one
  emoji, and each picture's licence follows its folder.
- `assets/emoji/index.json`, generated (`node scripts/gen-emoji-index.js`; `--check`): each set's
  picture names. `scripts/vendor-openmoji.js` copies a new OpenMoji release in, checking every picture
  (shapes only: `scripts/lib/emoji-pack.js` `checkSvg`).
- **Matching rules** (`web/shared/emoji.js`, the comment at its top): what counts as an emoji
  (Unicode's own properties: emoji-by-default characters, pictographs with U+FE0F, skin tones,
  joined sequences, flags, keycaps, tag flags; U+FE0E keeps text), the look-up key (code points with
  every U+FE0F taken out of both sides, because OpenMoji names a single emoji without it and keeps it
  in sequences), and the fallbacks when no set has the whole sequence. The answers both apps must
  give are `scripts/tests/fixtures/emoji-v1.json`.

## The website (built)

`web/shared/emoji.js` (`window.hosEmoji.parse(element)`) replaces Twemoji in the chat: every emoji
in a text node becomes `<img class="emoji">` of its picture on this server, lazily loaded, alt the
exact text. No request leaves the server for an emoji, so the CSP's `img-src` no longer needs
`https://cdn.jsdelivr.net`. Tests: `scripts/tests/emoji-web.test.js`.

**The credit where emoji are picked** (the operator said yes, 2026-11-20): the foot of the
reaction bar's ∞ panel (`web/chat/view/pillPopup.js`, styled `.pill-popup-credit` in
`web/chat/messages.css`) shows OpenMoji's requested attribution, verbatim, small and muted:
"All emojis designed by OpenMoji – the open-source emoji and icon project. License: CC BY-SA 4.0",
with "OpenMoji" linking https://openmoji.org/ and "CC BY-SA 4.0" linking the licence. The words are
`emoji.credit` in `data/i18n/en.json`, its `{{openmoji}}` and `{{licence}}` filled with the links
(`emoji.credit_openmoji`, `emoji.credit_licence`, each with its `_hint`). Held by
`scripts/tests/chat-rows-web.test.js`, which also holds the line to the Credits page's notice.

## The desktop app (rungs 1 to 4 built 2026-11-20)

egui draws every font glyph in one colour, so until rung 2 the native chat showed emoji from the
system's emoji font (`src/gui/fonts.rs`), as monochrome shapes, and a machine without one showed
boxes. The same pictures as the website needed three things egui does not have: an SVG rasteriser,
pictures inside a line of text, and the matcher. The system font is still the fallback for any emoji
the pack has no picture for, or when the pack is missing.

1. **The matcher in Rust (built).** `src/gui/widgets/emoji/mod.rs`: `find` (each emoji's byte range),
   `pieces` (text and pictures, each picture with its byte range, set folder and file name, the same
   fallbacks as `emoji.js`), `key_of`, and `EmojiIndex` (reads `assets/emoji/index.json`; the first
   set holding a key wins). It follows `emoji.js` rule for rule, the bare red heart exception
   included, and is held to `scripts/tests/fixtures/emoji-v1.json` by
   `the_desktop_matcher_gives_every_answer_of_the_shared_fixture` (`widgets/emoji/tests.rs`).
   **Unicode's properties: a generated table, compared and chosen 2026-11-20.** The open options:
   the regex crate's tables (regex-syntax 0.8.11, already in `Cargo.lock`), which carry Unicode 16.0
   and have no look-ahead, so the rules would be hand-written anyway; a Unicode-properties crate
   (none is in `Cargo.lock`, so a download); or a table generated from the Unicode data the website's
   engine has. The pack is OpenMoji 17.0.0, which draws Unicode 17.0's new emoji (U+1F6D8 landslide,
   U+1FAEA distorted face), and with the regex crate's Unicode 16 tables the desktop showed them as
   text (`unicode_17_emoji_in_the_pack_are_pictures`, seen red that way). So `props.rs` is written by
   `scripts/gen-emoji-props.js` from Node's own `\p{...}` (Unicode 17.0, the same data `emoji.js`
   asks), and `scripts/tests/emoji-props.test.js` holds it there: exact with a Node of the same
   Unicode version, a superset with an older one (CI's Node 20), and a failure saying to regenerate
   with a newer one. No crate added. The matcher is pure Rust; it lives under `src/gui/` (native
   only) because only the GUI draws emoji, and moves to `humanity-core` if a second surface needs it.
2. **Rasterising the SVGs (built).** `src/gui/widgets/emoji/pack.rs`: resvg 0.50 (with usvg and
   tiny-skia 0.12, no default features; chosen, and its licences quoted, in
   `docs/reference/findings/2026-11-20-svg-rasteriser.md`; the operator approved the download)
   draws each picture at the text's pixel size times the screen's scale (`rasterise`: scaled to fit
   the square and centred, as a browser draws the `<img>`) into a texture of the egui context that
   asks. `PackPictures` is rung 3's `EmojiPictures`; `install_pack` hands it, with the pack's
   `EmojiIndex`, to every egui context through `gui::install_fonts` (the main window and every wall
   screen, so a screen never draws emoji the main window does not). Without `assets/emoji/` beside
   the exe it installs nothing (said once in the log) and emoji stay the system font's glyphs.
   **Reuse, per CLAUDE.md's no-caches rule:** a drawn picture is kept in memory only, keyed by set,
   file and pixel size (the size already carries the theme's font scale and the screen's scale) and
   tagged with the pack's version, which a watcher of `assets/emoji/` bumps on any change there; a
   changed pack drops every kept picture and reads the list again (`inline::source` asks
   `pack::due`). With the Dev page's "Turn off all reuse" (`HUMANITY_NO_REUSE=1`) every ask reads
   and draws its file again and the list is read again every `LIST_REUSE_OFF_S` (1 s). Nothing is
   written to disk. A picture that cannot be drawn is logged, kept as "none" (not read again every
   frame) and stays the font's glyph. Over 2,048 kept pictures, those unused for 30 s are let go.
   **Cost** (measured 2026-11-20 in a debug test build with resvg, usvg and tiny-skia optimised, as
   the dev profile builds them; `pack_tests.rs` prints both): drawing a picture at 24 px, reading its
   file included, took 0.853 ms on average over all 4,495 pictures, and 3.7 ms for a process's very
   first picture (cold file and code). A reused picture took 0.006 ms, a hash look-up. So a frame pays
   only for emoji it has never drawn at that size: opening the ∞ panel the first time draws up to 42
   pictures (up to about 36 ms, once: a visible hitch on that one frame), and every later frame reuses
   them. That hitch is gone since the panel's pictures are drawn ahead, gradually, by a worker after
   start-up (below, "Drawn ahead, gradually"); there is still no cache on disk.
3. **Pictures in a line of text (built).** `src/gui/widgets/emoji/inline.rs`. egui's text layout has
   no inline images, so `with_pictures` gives each emoji that has a picture its own section of the
   layout job: the same characters, drawn transparent, at the font size that makes their run the
   picture's room wide (1.35 em: the website's 1.2 em picture with its 0.1 em and 0.05 em margins,
   `web/chat/base.css`), with the line height of the text around it so no row grows. After layout,
   `picture_rects` reads each placeholder's run back from the galley's glyphs (in whichever row
   wrapping put it) and `paint` draws the picture there, centred on the row. The galley's text is
   unchanged, so wrapping, click hit-testing (mentions, links) and copying all see the real emoji.
   `layout` / `layout_singleline` are the drop-in for `painter.layout_job` / `layout_no_wrap`: with
   nothing installed, or no picture for an emoji, the job is laid out exactly as before (the system
   font's glyph). Wired into **chat message bodies** (`widgets::row::message_row`, so the Settings
   page's message-row preview too) and **reaction pills** (`pages::chat` `compute_pill_width` and
   `paint_timestamp_pill`, measured and painted the same way). Tests: `widgets/emoji/tests.rs` (a
   picture on its placeholder, wrapping, nothing changes without a picture) and
   `pages/chat/emoji_tests.rs` (the pill on the real chat page; the drawn text and Copy text keep the
   real emoji), each with a stand-in texture made in code. **The reaction buttons** (the quick row
   beside the Þ and the ∞ panel, `pages::chat::rows`) were egui buttons, which lay out their own
   text; since 2026-11-20 each is `widgets::emoji::button::button`, framed and coloured as egui's
   button is, its emoji laid out through `layout`. Still to follow: names, channel names, the DM
   list and the other surfaces the website's `hosEmoji.parse` reaches.
4. **Ship it (built 2026-11-20).** `assets/emoji` is in `scripts/lib/release-files.js` `ROOTS` (about
   24 MB; the phone app's files follow the release's, so its chat has them too), and the system-font
   path stays the fallback when a picture fails. **The desktop picker shows the same credit**: the
   foot of its ∞ reaction panel (`pages::chat::rows`, the web panel's twin) carries the same line from
   the same keys (`emoji.credit` with its two links and hints, `widgets::emoji::button::credit`),
   small and muted, while the pack's pictures are installed (`emoji::pack_installed`); system-font
   emoji are not OpenMoji's and owe no credit. The native chat is in the `openmoji` row's `shown_in`
   in `data/credits.ron`, and resvg, usvg and tiny-skia's notices are in `THIRD-PARTY-NOTICES.txt`.

## The outline on a dark theme (built 2026-11-20)

OpenMoji draws every picture with black lines, which vanish on a dark ground: in the GPU snapshot
`chat_emoji` the faces lost their edge and the seedling showed as two green dashes. The operator
approved (2026-11-20) "a faint light halo behind each emoji on dark themes, as a theme setting so it
can be adjusted or turned off".

- **One source, two tokens.** `data/gui/theme.ron` `emoji_outline` (its colour, with its opacity:
  how faint it is) and `emoji_outline_width` (in points; 0 is off). `Theme::emoji_outline` and
  `emoji_outline_tokens` read them; Settings > Appearance > Theme colors > "Emoji outline" edits
  both, words by key with their hints (`settings.emoji_outline_*`); `tests/theme_editor_coverage.rs`
  holds them there. `node scripts/gen-theme-css.js` copies them into the website's `theme.css` as
  `--emoji-outline` and `--emoji-outline-width`.
- **Per theme.** The desktop has one palette, dark: on. The website's dark theme is that palette:
  on. The website's light theme sets the width to 0 (`LIGHT_OTHER` in the generator): on a light
  ground OpenMoji's own black lines read, and a light edge would only blur them.
- **Desktop: drawn into the picture.** `src/gui/widgets/emoji/outline.rs` `apply`, called by
  `pack::rasterise`: the picture's alpha silhouette grown by the width (an exact disc: each
  neighbour is covered by the share of it lying within the width of a drawn pixel, measured once per
  outline on an 8 by 8 grid, so edges stay soft and fractional widths work), filled with the colour,
  laid beneath the picture (premultiplied "over", in sRGB as egui blends). Since each picture is
  rasterised once and kept, the outline costs nothing per frame. The outline is part of a kept
  picture's key (with set, file, size and the pack's version), so a theme change draws the pictures
  again. The width is in points times the screen's scale, the same on screen at every emoji size,
  as the website's CSS pixels are. `Theme::apply_to_egui` hands each egui context the tokens
  (`outline::set_tokens`), at start-up, after an edit in Settings, and every frame for a wall screen;
  `inline::layout` reads them before it lays a line out.
- **Website: the same three steps as an SVG filter.** `web/shared/emoji.js` `applyOutline` reads
  the two variables from `<html>`'s computed style and builds one hidden filter, `#hos-emoji-outline`:
  `feMorphology` dilating `SourceAlpha` by the width, `feFlood` in the colour and opacity with
  `feComposite in` (the grown silhouette coloured), `feMerge` with the picture over it,
  `color-interpolation-filters="sRGB"`. While it is on, `<html data-emoji-outline>` and
  `web/chat/base.css` give every `img.emoji` `filter: url(#hos-emoji-outline)`. A
  `MutationObserver` on `<html>`'s `data-theme`, `class` and `style` reads the tokens again, so
  switching to the light theme turns it off. CSS `drop-shadow` was weighed and refused: chained
  offset shadows with a see-through colour compound into an uneven, lopsided edge, and a blurred
  shadow gives thin lines (the seedling's stems) almost no edge at all, which is the case this exists
  for.
- **How the two were compared.** The browser's `feMorphology` grows by a square of whole pixels; the
  desktop grows by a disc with fractional widths, which stays round at large sizes. resvg follows the
  SVG filter specification as the browsers do, so `outline_tests.rs`
  `the_desktop_outline_matches_the_websites_filter` draws the website's exact filter round the same
  pictures with resvg and compares the outline each lays round the picture (the summed opacity of
  the ring outside the silhouette, and its colour) at 24 px with a 1 px width and 48 px with 2 px.
  Measured 2026-11-20 on the seedling, the grinning face and the red heart: both reach the same
  ring pixels (75 and 75, 300 and 300, 272 and 272: never more than one apart), in the same colour
  (within 3 of 255 on each channel), and the desktop's ring holds 0.79 to 0.86 of the website's
  opacity, because the browser's square fills each diagonal neighbour whole where the disc fills
  its share (pi/4 at a 1 px width). The test holds the reach within 10% and the opacity within 0.75
  to 1.25. The comparison pictures (plain, desktop, website, scaled up 8 times on the theme's black)
  come from `write_the_outline_comparison_pictures` (`HUMANITY_EMOJI_OUTLINE_PNGS=<folder>`); by
  eye the desktop's and the website's are the same picture, the seedling's stem and leaves now
  drawn where they had been two green dashes. A real browser was not driven for this comparison;
  resvg's filter code, written to the same specification, stands in for it (worth one look in a
  browser at the live chat after the next deploy).
- **The shipped values**: `emoji_outline: (0.92, 0.92, 0.95, 0.55)` (near-white with a little blue,
  at 55% opacity: a faint grey ring on black, #808083 where it is whole) and `emoji_outline_width:
  1.0` point, chosen by looking at the comparison pictures and the GPU snapshot `chat_emoji`
  (`tests/snapshots/chat_emoji.png`): the faces have their edge again and the seedling reads as a
  seedling, while the ring stays fainter than the emoji's own colours. Settings offers 0 to 3 points.

## Drawn ahead, gradually (built 2026-11-20)

Each picture is drawn the first time it is asked for, so the first opening of the ∞ reaction panel
drew up to 42 pictures in one frame (the hitch the operator asked about). He then set the rule for
the fix (2026-11-20): "Just remember we don't want to precache all at once. We want to do it
gradually so the app doesn't hang."

- **The worker.** `src/gui/widgets/emoji/warm.rs`, called once a frame from the app's main frame
  (`engine::frame_ui_run`). From `warm::START_AFTER_S` (2 s) after the pictures were installed, it
  hands a worker thread the reaction panel's pictures: the palette's reactions
  (`data/reactions.json`) at the quick row's size (the body text) and the ∞ panel's (egui's button
  text), with the theme's outline, through the same size rule the chat asks with
  (`inline::picture_px`). The worker draws ONE picture, rests `warm::GAP_MS` (15 ms), draws the next:
  never a burst, never holding a core, and it holds a lock only to look and to hand a picture over.
  A new list (a font size, a screen scale, a theme or a pack change) replaces the old one and stops
  its worker.
- **The frame's share.** Each frame turns at most `warm::UPLOADS_PER_FRAME` (4) finished pictures into
  textures, and stops early past `warm::FRAME_BUDGET_MS`: **the budget is 2 ms of emoji work per
  frame**. Nothing waits for the worker: a picture asked for before it is ready is drawn then, as
  before, and a finished one still waiting is used instead of being drawn twice.
- **Reuse.** The drawn-ahead pictures are kept pictures like the rest (memory only, tagged with the
  pack's version, keyed by size and outline). With "Turn off all reuse" nothing is drawn ahead
  (`PackPictures::stop_ahead`), since every ask draws again anyway.
- **Measured** (2026-11-20, `pages/chat/emoji_tests.rs`
  `the_panels_first_opening_draws_no_picture_on_the_frame`, a debug test build with resvg, usvg and
  tiny-skia optimised, the chat page at 1280x800, the shipped pack and theme, frames about 8 ms
  apart):
  - **While the worker draws ahead** (two runs): 52 pictures (the quick row's 10 and the panel's
    42, each at its own size) over 79 to 83 frames and 828 to 859 ms; the worst frame 1.8 to 5.4 ms,
    the drawing-ahead's share of any frame at most 0.27 to 0.38 ms (the budget is 2 ms), at most 1
    upload in a frame, nothing drawn on a frame.
  - **The panel's first opening:** before, the frame that opened ∞ took 51.0 ms (42 pictures drawn
    in it) and the hover that showed the quick row 10.5 ms (10 pictures); after, 7.4 to 11.5 ms and
    3.0 to 5.3 ms, with no picture drawn on any frame of the opening. What is left is egui laying
    the panel out for the first time (its 42 buttons, and the placeholder font sizes it meets for
    the first time), in a debug build.

**Tests** (each seen red, the break recorded at the test): `widgets/emoji/pack_tests.rs` (a
rasterised emoji's drawn pixels cover its share of the square in the file's own colours; every one of
the pack's 4,495 pictures draws; the reuse switch: with it off a second ask draws again; a missing
picture stays text and is tried once; when the list is read again; the credit's two links; a reaction
button paints its picture), `widgets/emoji/outline_tests.rs` (a dark-outlined picture gets a ring in
the outline's colour on the dark theme and is exactly the plain picture with the token off; the
desktop's outline against the website's filter drawn by resvg; a theme change draws the pictures
again), `widgets/emoji/warm_tests.rs` (drawing ahead is gradual and so are the uploads; a waiting
picture is used, not drawn twice; the reuse switch and the start delay), `pages/chat/emoji_tests.rs`
(the ∞ panel draws every reaction as a picture and carries the credit only for the pack's pictures;
its first opening draws no picture on the frame), `scripts/tests/emoji-web.test.js` test 5 (the
website's outline from the tokens, off in the light theme), and the GPU snapshot
`just snapshot chat_emoji` (it refuses a frame that painted no emoji picture).
