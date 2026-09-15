# HumanityOS UI System

Canonical design system spec. Read this before adding a widget, page, or visual element to either the native desktop app or the web frontend. The rules here protect dual-UI parity and prevent the "CSS magic that can't port" trap.

Last updated: v0.113.0 (2026-04-25).

## Principles

1. **Rust-first canonical.** The native egui client is the source of truth for visual language. Any new UI pattern must be egui-implementable first; the web mirrors it. The reverse direction leads to divergence because web has capabilities egui does not.

2. **One token source, two consumers.** All design tokens (colors, spacing, radius, fonts, widget sizes) live in `data/gui/theme.ron`. Native reads it directly. Web receives a generated `web/shared/theme.css` that is written from the same file by `scripts/gen-theme-css.js`. Editing `theme.ron` restyles both UIs.

3. **Every UI element maps to a token.** No literal colors, no magic numbers. If a new pattern needs a value, add a named token first, then use the token name.

4. **Component parity.** A widget that exists in native has a matching CSS class in web with the same visual signature. A widget that exists only in web is a bug unless explicitly marked web-only in this doc.

5. **Infinite-of-X.** Anything that can exist more than once is a data file, not code. See [`infinite-of-x.md`](./infinite-of-x.md).

6. **Settings-page theming.** Because every widget reads from theme tokens, the Settings page only needs to override `:root` CSS variables (web) or mutate `Theme` fields (native) to restyle the entire app. New widgets must follow this rule or they break the settings contract.

## Tokens: colour, spacing, typography, radii

**The tokens are not listed here, deliberately.** `data/gui/theme.ron` is the
canonical source, native reads it directly, and `scripts/gen-theme-css.js`
regenerates `web/shared/theme.css` from it. Earlier revisions of this document
transcribed every value into three tables and they drifted, which is the
predictable outcome: the RON stores colours as float tuples while the tables
showed hex, so the two could never be compared by eye, and the file is
live-edited from the Settings theme editor, so any snapshot is stale as soon as
somebody moves a slider.

To read the current values, open `data/gui/theme.ron`, or open Settings and
look at the theme editor, which renders every token by definition:
`cargo test --test theme_editor_coverage` fails if a token exists without an
editable row.

The rules that do belong here, because they are rules rather than values:

- Never hardcode a colour. `cargo test --test theme_token_lint` fails the build
  on a new `Color32::from_rgb(...)` under `src/gui/` or `src/renderer/`.
- Add a token to `theme.ron` AND an accessor in `src/gui/theme.rs`. Adding one
  without wiring it into the Settings editor breaks the coverage test.
- Do not hand-edit `web/shared/theme.css`. Edit the RON and run `just theme`.

## Component registry

Each widget below must exist in both native (`src/gui/widgets/`) and web (CSS class or shared JS component), with the same visual signature and the same theme tokens consumed.

### Native widgets in `src/gui/widgets/`

| Widget | Native fn | Web class | Tokens consumed |
|--------|-----------|-----------|-----------------|
| **Universal button** (preferred) | `widgets::Button::new(...)` builder | `.btn.btn-{primary,secondary,danger,success,ghost}` | `accent`, `text_on_accent`, `text_primary`, `text_secondary`, `border`, `border_radius`, `button_height`, `font_size_*`, `danger`, `success` |
| Primary button (legacy fn) | `primary_button` → wraps `Button::primary` | `.btn.btn-primary` | tokens above |
| Secondary button (legacy fn) | `secondary_button` → wraps `Button::secondary` | `.btn.btn-secondary` | tokens above |
| Danger button (legacy fn) | `danger_button` → wraps `Button::danger` | `.btn.btn-danger` | tokens above |
| Card | `card` | `.card` (inline in most pages) | `bg_card`, `border`, `border_radius`, `card_padding` |
| Card with header | `card_with_header` | `.card.card-header` | as above + `heading_size` |
| Collapsible section | `collapsible_section` | `<details>` styled via `.details` | `text_primary`, `section_gap` |
| Settings row | `settings_row` | `.settings-row` | `settings_label_width`, `text_secondary` |
| Custom slider | `custom_slider` | `<input type="range">` styled | `slider_track`, `slider_track_height`, `slider_thumb_radius` |
| Labeled slider | `labeled_slider` | wraps slider + label | tokens above + `text_muted`, `font_size_small` |
| Custom checkbox | `custom_checkbox` | `<input type="checkbox">` styled | `checkbox_size`, `accent`, `text_on_accent`, `border` |
| Toggle row | `toggle` | `.toggle-row` | settings_row tokens + checkbox |
| Progress bar | `progress_bar` | `<progress>` styled | `accent` |
| Tab bar | `tab_bar` | `.tab-bar` | `accent`, `border_radius`, `font_size_body` |
| Role badge | `role_badge` | `.role-badge.r-admin\|r-mod\|r-verified\|r-donor` | `badge_admin|mod|verified|donor`, `badge_radius`, `small_size` |
| Badge | `badge` | `.badge` | passed color + `badge_padding`, `small_size` |
| Badge small | `badge_sm` | `.badge.badge-sm` | tighter padding |
| Detail row | `detail_row` | `.detail-row` | `text_secondary`, `text_primary`, `small_size` |
| Detail row bold | `detail_row_bold` | `.detail-row.bold` | above + `.strong` |
| Search bar | `search_bar` | `.search-bar` | `text_secondary`, `body_size` |
| Sidebar nav | `sidebar_nav` | `.sidebar-nav` + `.sidebar-nav-item.active` | `accent`, `text_secondary`, `body_size` |
| Category filter | `category_filter` | `.category-filter` | `accent`, `bg_card`, `text_on_accent`, `badge_radius`, `small_size` |
| Stat card | `stat_card` | `.stat-card` | card tokens + `success`, `danger`, `small_size`, `heading_size` |
| Page frame | `page_frame` | `main > .container` | `bg_panel`, `card_padding` |
| Sidebar frame | `sidebar_frame` | `.sidebar` | `bg_sidebar`, `panel_margin` |
| Section header | `section_header` | `h2.section-heading` | `heading_size`, `text_primary`, `section_gap`, `row_gap` |
| Themed separator | `themed_separator` | `<hr>` | `section_gap` |
| Modal | `widgets::dialog::dialog` / `dialog_anchored` | `.hos-help-backdrop` + `.hos-help-modal` | `bg_modal`, `bg_card`, `border`, `border_radius_lg`, `card_padding`, `modal_width` |
| Help button (`?`) | (new, see below) | `.hos-help-btn` | `border`, `text_muted`, `accent` |
| Help modal | (new, see below) | already done | same as modal |
| Onboarding quest chain | (new, see below) | `.quest-chain` / `.quest-step` | card tokens + `accent`, `border`, `small_size` |

### Widgets planned but not yet shared

- **Toast notification** (`hosToast` on web, `toast` widget in native), transient feedback.
- **Confirmation dialog** (replaces `window.confirm()` web and `modal_dialog` with Confirm/Cancel in native).
- **Context menu** (color-coded sections for role-based actions).
- **Inline tooltip** (hover-triggered, web has it partially in `shell.js`).

### Web-only (marked explicitly, for now)

- Compact mode toggle (web uses `[data-compact]` attribute to reduce spacing scale).
- Light theme (`[data-theme="light"]`).
- Accessibility overrides (high contrast, reduced motion, colorblind filters).
- Service worker integration.

These do not need native equivalents until there is a clear user benefit. When one is requested, it promotes to the shared list.

## The universal button (v0.113.0)

Every button across the app, chat back-arrows, marketplace Buy, settings Cancel, governance Vote, recovery Approve, the entire desktop nav, even the small inline `+ Create Channel` ghost links, should go through `widgets::Button`. **One source of truth.** Edit the builder once, every site updates.

```rust
use crate::gui::widgets::{self, Button, ButtonSize, ButtonVariant};

// Five variant shortcuts:
Button::primary("Save").show(ui, theme);
Button::secondary("Cancel").show(ui, theme);
Button::danger("Delete").show(ui, theme);
Button::success("Confirm").show(ui, theme);
Button::ghost("\u{2190} Back").show(ui, theme);  // transparent, used for nav

// Modifiers (any combination):
Button::primary("Sign in").icon("\u{1F511}").full_width().show(ui, theme);
Button::secondary("Submit").size(ButtonSize::Large).show(ui, theme);
Button::danger("Delete").icon("\u{1F5D1}").disabled(no_selection).show(ui, theme);
Button::icon_only("\u{2699}").tooltip("Settings").show(ui, theme);
Button::primary("Save").icon_trailing("\u{2192}").show(ui, theme);
```

**Five variants:**
- `Primary`, filled accent, primary CTA
- `Secondary`, outlined, secondary action
- `Danger`, red filled, destructive
- `Success`, green filled, confirm
- `Ghost`, transparent, inline links / nav back / icon-only

**Three sizes:** `Small`, `Medium` (default), `Large`, drives both font size and min-height from theme tokens.

**No literal colors, no magic numbers.** All styling derives from `Theme`. Want different button corners app-wide? Edit `border_radius` in `data/gui/theme.ron`. Want all primaries to be tighter? Adjust `button_height`. Settings-page theming works for free.

**Icon support** is unicode glyphs in the `label` text run (e.g., `\u{2190}` for `←`, `\u{2699}` for `⚙`). For painted icons, compose your own horizontal layout using `widgets::icons::paint_*`.

**Backward compatibility:** the older free functions `primary_button(ui, theme, "Save")`, `secondary_button(...)`, `danger_button(...)`, `btn_primary(...)`, etc. are preserved as thin wrappers and will keep working forever. New code should prefer `Button::primary("Save").show(ui, theme)` for the more flexible builder API. The duplication that previously existed between `widgets/mod.rs` and `widgets/button.rs` is gone, there's one definition now.

## How to add a new widget

1. **Start in native.** Write the widget in `src/gui/widgets/` using only `Theme` fields. No literal colors, no magic numbers.
2. **Add any new tokens** to `data/gui/theme.ron` and `src/gui/theme.rs` (struct field + default function).
3. **Regenerate `theme.css`** by running `node scripts/gen-theme-css.js`.
4. **Implement the web version** in CSS using the generated variables. Match the visual signature from native.
5. **Document it** by adding a row to the component registry table above.
6. **Use it** on at least one page in each UI before considering it shipped.

## How to add a new help topic

Help topics live in `data/help/topics.json`. Both UIs read from the same file.

```json
{
  "version": 1,
  "topics": {
    "my-topic-id": {
      "title": "What this is",
      "body": [
        "First paragraph.",
        "Second paragraph. Use <strong>tags</strong> for emphasis on web.",
        "Third paragraph."
      ]
    }
  }
}
```

**Web** consumes this via `window.hosHelp.show('my-topic-id')` and the `[data-help-id="my-topic-id"]` attribute on any button.

**Native** consumes via `src/gui/widgets/help_modal.rs`, wired into the render loop and reading `gui_state.active_help_topic` (shipped; this note was stale, corrected 2026-07-01 -- it was never revisited after the widget landed). As of this correction only `chat.rs` actually calls it ("What's a group vs a server?"); most pages with controls that could use a help topic don't yet -- see `data/help/topics.json` (9 topics today) to add more.

To add a help button next to any UI element:

- **Web:** `<button class="hos-help-btn" data-help-id="my-topic-id" aria-label="Help">?</button>`
- **Native:** call `widgets::help_modal::help_button(ui, theme, "my-topic-id", &mut gui_state.active_help_topic)`.

## How to add a new page

1. **Check existing pages first.** If there is an overlap, enhance the existing page instead.
2. **Check `docs/FEATURES.md`.** If the feature is listed, do not rebuild it.
3. **Create native page first:** add a file under `src/gui/pages/`, add the variant to `GuiPage` enum in `src/gui/mod.rs`, register it in the page dispatch.
4. **Register state in `GuiState`** for anything the page needs to remember across frames.
5. **Create web page:** `web/pages/<name>.html` using the generated `theme.css`, sharing copy and structure with the native page.
6. **Link it** in `web/shared/shell.js` nav (if top-level) and in any onboarding or help content.
7. **Add to the component registry** if it introduces any new shared widget.

## Migration status (live)

- [x] `data/gui/theme.ron` is the canonical token source.
- [x] `src/gui/theme.rs` loads theme.ron natively.
- [x] `scripts/gen-theme-css.js` generates `web/shared/theme.css` from `theme.ron`.
- [x] Web colour palette aligned with native (one source, regenerated by `just theme`).
- [x] Universal help modal (web).
- [x] Universal help modal (native), `src/gui/widgets/help_modal.rs`.
- [x] Onboarding (web, `/onboarding`).
- [x] Onboarding (native). The standalone page was folded into Tasks/Quests in
      v0.415.0; `onboarding::draw_quests` is called from `src/gui/pages/tasks.rs`.
- [x] Toast notifications (native, `widgets::draw_toasts`).
- [ ] Confirmation dialog (both).
- [ ] Context menu with role-coloured sections (both).

## Every page must earn its existence (operator principle, 2026-06-30)

HumanityOS has spent a long time getting the GUI right, especially the main menu /
top-level nav that ties every page together (see the "Merged super-tabs" and
"Category-landing pages" sections of `docs/PAGES.md` for the several rounds of nav
consolidation this has already been through). The standing rule going forward: a page
is only justified if it is irreplaceable and unique enough that merging it into
another page would lose something real. This cuts both ways, don't fold a genuinely
distinct page into another just to shrink the nav count, and don't add a new page for
something that's really a section of an existing one. When proposing a new page or a
merge, name specifically what would be lost by the other choice.

## Page parity (web ↔ native)

**The inventory is not duplicated here any more.** This document used to carry a
hand-maintained list snapshotted at v0.124.0, and by 2026-09 it named three pages
that exist in neither client and two that are no longer native pages at all.

The enforced sources, which cannot drift without failing a build:

- `docs/PAGES.md` is the canonical page registry, and its heading carries the
  live native and web page counts.
- `cargo test --test page_registry_lint` checks every page is registered.
- `cargo test --test page_parity_lint` checks the web and native sets against
  each other and requires a written reason for each deliberate difference.

The RULE still belongs here: when a web feature adds a UI pattern, ask whether
native needs it. If yes, port it before shipping. If no, record why in the
parity lint rather than letting the two silently diverge.

## Verifying the native UI (snapshots + headless interaction)

The native egui UI has two automated verification layers. Both run on the dev host;
the link-free parts also run in CI (`.github/workflows/verify.yml`).

### Snapshots (does it RENDER)

`just snapshots` renders pages to `tests/snapshots/*.png` via an offscreen wgpu
device (`src/gui/ui_snapshots.rs::render_page_png`). Read the PNGs to review layout.
These need a GPU, so they are `#[ignore]`d and run single-threaded.

### Interaction tests (does it WORK)

Rendering is not interactivity: an egui panel can paint yet be un-clickable (the
"shows != works" trap). `ui_snapshots.rs` drives SYNTHETIC pointer input through the
app's own egui with NO GPU, so a click can be asserted in the normal
`cargo test --features native --lib` pass (and in CI):

- `headless_run(screen, frames, build)` runs `build` once per frame, feeding that
  frame's `egui::Event`s, and returns the `Context` to read post-run state.
- A click is the canonical 3-frame sequence: `PointerMoved(pos)`, then
  `PointerButton{pressed:true}`, then `{pressed:false}` in SEPARATE frames (same-frame
  press+release works for a plain `Button` but not for a re-`interact()`ed row).
- Locate a widget by recording its `rect` at layout time behind `#[cfg(test)]` (egui
  has no query-by-content API). See `RECORDED_HEADER_RECTS` + `test_recorded_header_rect`
  in `inventory.rs`, and the worked example `inventory_container_header_click_toggles_open`.

Two rules this discipline surfaced, worth following for any new clickable widget:

1. **Give a clickable region a STABLE `Id`** (`ui.interact(rect, Id::new(("thing", key)), Sense::click())`),
   not an auto-generated one. A stable Id makes the cross-frame press/release reliably
   attribute to the widget; an auto Id is sequence-dependent and the synthetic click is
   silently dropped. (This is also why the inventory container header got a stable Id.)
2. **A click target that claims `available_width()` can run wider than the screen** in a
   scroll context, so its `rect.center()` may be off-screen. Click a known on-screen
   point on the widget, not blindly its center.

When you add a new interactive widget, add a click-assert here in the same increment,
the same way a new color token must appear in the Settings editor.
