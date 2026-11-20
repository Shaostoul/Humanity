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

## The desktop app (planned, in rungs)

egui draws every font glyph in one colour, so today the native chat shows emoji from the system's
emoji font (`src/gui/fonts.rs`), as monochrome shapes, and a machine without one shows boxes. The
same pictures as the website need three things egui does not have: an SVG rasteriser, pictures inside
a line of text, and the matcher.

1. **The matcher in Rust**, beside the chat (or in `humanity-core` if a second surface needs it):
   the same rules as `emoji.js`, held to `scripts/tests/fixtures/emoji-v1.json` by a Rust test (the
   `channel_link.rs` / `chat-links-v1.json` pattern). Unicode's emoji properties come from a crate
   or from a table generated from Unicode's own data files: compare the open options first (the
   house rule for every new tool) and record the choice.
2. **Rasterising the SVGs.** The standard route is an SVG renderer drawing each picture at the text's
   pixel size times the screen's scale into an egui texture (`egui_extras` does this with `resvg`).
   Before adopting any renderer, scan the alternatives and write a dated finding on its licence
   (never from memory). Rasterise on first sight of an emoji at a size; keep the textures in memory
   only, rebuilt when the pack's files change, and obey the Dev page's "turn off all reuse" switch
   (`HUMANITY_NO_REUSE=1`): CLAUDE.md's no-caches rule. No baked PNG atlas on disk.
3. **Pictures in a line of text.** egui's text layout has no inline images. The technique that keeps
   wrapping, selection and copy working: lay the text out with a transparent placeholder of the
   picture's width in each emoji's place, then paint each picture over its placeholder's glyph
   rectangle from the laid-out galley. Copying the text gives the real emoji, because the source
   text is unchanged. Start with chat message bodies and reaction pills; names and other surfaces
   follow.
4. **Ship it.** Add `assets/emoji` to `scripts/lib/release-files.js` `ROOTS` (about 24 MB; the phone
   app too, it has a chat), add the native chat to the `openmoji` row's `shown_in` in
   `data/credits.ron`, and keep the system-font path as the fallback when a picture fails.

Until rung 4 lands, the release download does not carry `assets/emoji/`: nothing in the desktop app
reads it yet.
