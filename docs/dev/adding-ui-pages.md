# Adding UI Pages and Widgets

Native egui first, web mirrors it. The canonical design rules live in
[docs/design/ui-system.md](../design/ui-system.md), read it before touching
any widget, page, or visual code; this doc is the mechanical how-to for
adding a page without breaking the parity and theming contracts.

## The order of operations

1. **Check [docs/PAGES.md](../PAGES.md) and
   [docs/FEATURES.md](../FEATURES.md) first.** If the page or feature exists,
   enhance it. Several past "new pages" were deleted as duplicates.
2. Design the pattern so it is implementable in egui. Web-only capabilities
   (CSS magic egui cannot express) are the divergence trap.
3. Build native, then mirror to web (or document in the design doc why no
   web mirror is needed).
4. Register everywhere the lints check (below), update PAGES.md in the SAME
   commit.

## Native: adding an egui page

The registration is a hand-wired chain, all in predictable places:

1. **Module**: create `src/gui/pages/your_page.rs` with a
   `pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState)`
   (that exact signature is what every page uses). Declare it in
   `src/gui/pages/mod.rs`.
2. **Enum variant**: add it to `pub enum GuiPage` in `src/gui/mod.rs` (with a
   doc comment saying what it is; the file's history comments show the
   convention). If it should be selectable as the boot page, also
   `BOOT_PAGE_OPTIONS` + `page_to_config_str` / `config_str_to_page`.
3. **Dispatch**: add the match arm in `src/lib.rs`'s page dispatch
   (`GuiPage::YourPage => your_page::draw(ctx, &state.theme, &mut
   state.gui_state)`, grep `GuiPage::Cosmos =>` to find the block).
4. **Navigation**: the single-row nav bar lives in
   `src/gui/pages/escape_menu.rs` (`draw_nav_bar_one_tier`, `NavItem`
   arrays grouped into Play / Humanity / Chat / Real / Sim / Platform
   groups). Most new surfaces should NOT be a new top-level tab: the v0.358+
   direction folds related sections into the existing hub pages (Real,
   Platform, Humanity) via their internal `section_nav`. Adding a top-level
   tab is an operator taste decision.
5. **Page state**: fields go on `GuiState` (`src/gui/mod.rs`), not statics.

## Widgets and theme tokens

- Reusable widgets go in `src/gui/widgets/` AND get a matching CSS
  class/shared JS component on the web side. Both consume THEME TOKENS, never
  literals.
- Tokens live in `data/gui/theme.ron`; native accessors in
  `src/gui/theme.rs`; web's `web/shared/theme.css` is GENERATED from the RON
  by `node scripts/gen-theme-css.js` (never hand-edit color values in the
  CSS).
- Adding a token means: RON value + accessor in `theme.rs` + an editable row
  in Settings (`src/gui/pages/settings.rs`, `draw_appearance_content` for
  colors, `draw_widgets_content` for numerics). The coverage lint fails if
  you skip the Settings row.

## The lints that will catch you (run `just lints`)

All std-only file scanners, compiled standalone so they dodge the Windows
LNK1318 PDB limit (see CLAUDE.md gotchas):

| Lint | What it enforces |
|------|------------------|
| `tests/theme_token_lint.rs` | No new hardcoded `Color32::from_rgb(...)` in `src/gui/` + `src/renderer/`. Fix by adding a token, NOT by adding to `LEGACY_OFFENDERS`. Genuinely computed colors escape via an inline `// theme-exempt: <reason>`. |
| `tests/theme_editor_coverage.rs` | Every theme token is editable in Settings (or listed as `intentionally_omitted` with a reason). |
| `tests/icon_glyph_lint.rs` | No known-tofu Unicode in UI strings. The egui font has spotty coverage: Math Operators and Dingbats blocks are mostly broken, `U+FE0F` always renders a trailing tofu, and even `←`/`↔` are broken (use `widgets::icons::paint_arrow_*` instead). Safe families: Latin, General Punctuation, `↩ →`, and the confirmed set `❤ ⭐ ∞ ✓ ⚠ ·`. Escape: `// glyph-exempt: <reason>` with screenshot evidence. |
| `tests/emdash_lint.rs` | No em dashes (U+2014) in any `src/gui/` string literal. Applies to docs prose by operator rule too, write with hyphens and commas. |
| `tests/page_registry_lint.rs` | Every `GuiPage` variant and every `web/pages/*.html` file is mentioned in docs/PAGES.md, and the web-page count in its heading matches reality. This is why PAGES.md must be updated in the same commit. |

(`just lints` runs a sixth scanner too, `tests/engine_wiring_lint.rs`, which
guards game-system wiring rather than UI.)

## Web mirror

- Standalone pages live in `web/pages/<name>.html` (+ `<name>-app.js` when
  they have logic). Every page loads `web/shared/shell.js` first (nav
  injection) and consumes `web/shared/theme.css` variables.
- Dual-UI parity rule: when a web feature adds a UI pattern, ask whether
  native needs it; if yes, port BEFORE shipping; if no, document why in
  ui-system.md. Web mirrors native, never the reverse (the app is the
  product, the site is its reflection).

## Verifying without booting: headless snapshots

- `just snapshots` renders the snapshot-covered native pages to
  `tests/snapshots/*.png` headlessly; `just snapshot <name>` renders one
  (names = the `snapshot_` test suffixes in `src/gui/ui_snapshots.rs`, e.g.
  `just snapshot construction`).
- Give a new page a snapshot test in `ui_snapshots.rs` (copy an existing
  `snapshot_*` fn) so reviewers and AI agents can SEE it without a GUI
  session.
- Snapshots prove rendering, not interactivity. Panels can render but be
  un-clickable (a real past bug class): interactive flows still need a boot
  + click check, or a headless click test
  (`docs/contributor/development_loop.md`).

## Checklist

1. PAGES.md row added (native table, and web table if mirrored) - same
   commit.
2. `just lints` green (all six).
3. Snapshot test added; `just snapshot <name>` output looks right.
4. Theme tokens only; new tokens wired into Settings.
5. Web mirror shipped or its absence documented.
6. `cargo check --features relay --no-default-features` still green (GUI
   modules are native-gated; an ungated import once broke deploys for 25
   releases).
