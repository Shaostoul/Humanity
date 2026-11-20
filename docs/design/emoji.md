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

## The desktop app (rungs 1 and 3 built 2026-11-20; rung 2 awaits a download)

egui draws every font glyph in one colour, so today the native chat shows emoji from the system's
emoji font (`src/gui/fonts.rs`), as monochrome shapes, and a machine without one shows boxes. The
same pictures as the website need three things egui does not have: an SVG rasteriser, pictures inside
a line of text, and the matcher.

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
2. **Rasterising the SVGs (prepared, not built).** The standard route is an SVG renderer drawing each
   picture at the text's pixel size times the screen's scale into an egui texture. The comparison and
   licences: `docs/reference/findings/2026-11-20-svg-rasteriser.md` (resvg with usvg and tiny-skia
   recommended, without default features: 12 crates, about 909 KiB, to download once the operator
   says yes). Rung 2 is one implementation of the trait rung 3 already asks for,
   `widgets::emoji::EmojiPictures` ("the texture of this picture at this pixel size, if any"),
   handed to the app with `widgets::emoji::install` together with the pack's `EmojiIndex`.
   Rasterise on first sight of an emoji at a size; keep the textures in memory only, rebuilt when the
   pack's files change (the index too), and obey the Dev page's "turn off all reuse" switch
   (`HUMANITY_NO_REUSE=1`): CLAUDE.md's no-caches rule. No baked PNG atlas on disk.
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
   real emoji), each with a stand-in texture made in code. Still to follow: the ∞ panel's reaction
   buttons (`pill_expand`, egui buttons that lay out their own text), names and other surfaces.
4. **Ship it.** Add `assets/emoji` to `scripts/lib/release-files.js` `ROOTS` (about 24 MB; the phone
   app too, it has a chat), and keep the system-font path as the fallback when a picture fails.
   **The desktop picker shows the same credit** the moment the desktop draws these pictures (rung 2
   makes the message bodies and pills draw them as soon as it installs its pictures): the foot of its
   ∞ reaction panel (native `pill_expand`, the web panel's twin) carries the same line from the same
   keys (`emoji.credit` with its two links and hints), small and muted, and the native chat joins the
   `openmoji` row's `shown_in` in `data/credits.ron`. Until then the desktop draws system-font emoji,
   which are not OpenMoji's, so it owes no credit.

Until rung 4 lands, the release download does not carry `assets/emoji/`: nothing in the desktop app
reads it yet.
