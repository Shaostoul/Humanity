# HumanityOS UI System

Canonical design system spec. Read this before adding a widget, page, or visual element to either the native desktop app or the web frontend. The rules here protect dual-UI parity and prevent the "CSS magic that can't port" trap.

Last updated: v0.91.5 (2026-04-22).

## Principles

1. **Rust-first canonical.** The native egui client is the source of truth for visual language. Any new UI pattern must be egui-implementable first; the web mirrors it. The reverse direction leads to divergence because web has capabilities egui does not.

2. **One token source, two consumers.** All design tokens (colors, spacing, radius, fonts, widget sizes) live in `data/gui/theme.ron`. Native reads it directly. Web receives a generated `web/shared/theme.css` that is written from the same file by `scripts/gen-theme-css.js`. Editing `theme.ron` restyles both UIs.

3. **Every UI element maps to a token.** No literal colors, no magic numbers. If a new pattern needs a value, add a named token first, then use the token name.

4. **Component parity.** A widget that exists in native has a matching CSS class in web with the same visual signature. A widget that exists only in web is a bug unless explicitly marked web-only in this doc.

5. **Infinite-of-X.** Anything that can exist more than once is a data file, not code. See [`infinite-of-x.md`](./infinite-of-x.md).

6. **Settings-page theming.** Because every widget reads from theme tokens, the Settings page only needs to override `:root` CSS variables (web) or mutate `Theme` fields (native) to restyle the entire app. New widgets must follow this rule or they break the settings contract.

## Canonical color palette

The values below are in `data/gui/theme.ron`. When this doc disagrees with the RON file, the RON file wins and this doc needs updating.

| Token | Hex | Used for |
|-------|-----|----------|
| `bg_primary` | `#0a0a0c` | Deepest surface, app background |
| `bg_secondary` | `#141418` | Next-deepest (chat center panel, modal body) |
| `bg_tertiary` | `#252530` | Raised surfaces, hover states |
| `bg_card` | `#1a1a22` | Cards, content containers |
| `bg_modal` | `rgba(0,0,0,0.7)` | Modal backdrop scrim |
| `bg_panel` | `#141419` | Standard page content panels |
| `bg_sidebar` | `#16161c` | Sidebar column |
| `bg_sidebar_dark` | `#1e1e24` | Darker sidebar (chat left panel) |
| `accent` | `#ED8C24` | Primary orange (buttons, highlights, focus) |
| `accent_hover` | `#FFA63D` | Hovered accent |
| `accent_pressed` | `#CC7319` | Pressed accent |
| `text_primary` | `#e8e8ea` | Main text |
| `text_secondary` | `#888894` | Labels, secondary text |
| `text_muted` | `#6a6a75` | Hints, captions |
| `text_on_accent` | `#0d0d0d` | Text on orange buttons |
| `success` | `#33bf4d` | Positive state (green) |
| `warning` | `#f2bf1a` | Caution (amber) |
| `danger` | `#e6403d` | Errors, destructive |
| `info` | `#3380e6` | Informational blue |
| `border` | `#2a2a35` | Default borders |
| `border_focus` | `#ED8C24` | Focused border (accent) |

**Role badge colors** (used by native `role_badge` widget and web chat badges):

| Token | Hex | Role |
|-------|-----|------|
| `badge_admin` | `#e68021` | Admin |
| `badge_mod` | `#26ad61` | Moderator |
| `badge_verified` | `#3394d9` | Verified |
| `badge_donor` | `#9c59b5` | Donor |
| `badge_live` | `#e64d3d` | Live / streaming |

**Chat tint colors** (context-aware backgrounds):

| Token | Hex | Area |
|-------|-----|------|
| `dm_bg` / `dm_row_bg` / `dm_row_hover` | red tints | DM section |
| `group_bg` / `group_row_bg` / `group_row_hover` | green tints | Groups |
| `server_bg` / `server_row_bg` / `server_row_hover` | blue tints | Servers |

## Spacing and sizing

| Token | Value | Web equivalent |
|-------|-------|----------------|
| `spacing_xs` | 2 | `--space-xs` |
| `spacing_sm` | 4 | `--space-sm` |
| `spacing_md` | 8 | `--space-md` |
| `spacing_lg` | 12 | `--space-lg` |
| `spacing_xl` | 16 | `--space-xl` |
| `row_gap` | 2 | used inline |
| `section_gap` | 4 | used inline |
| `item_padding` | 4 | used inline |
| `panel_margin` | 6 | used inline |
| `card_padding` | 8 | used inline |

Sizes (component heights, widget sizing):

| Token | Value |
|-------|-------|
| `button_height` | 24 |
| `button_padding_h` | 10 |
| `input_height` | 24 |
| `sidebar_width` | 240 |
| `modal_width` | 440 |
| `icon_size` | 14 |
| `icon_small` | 12 |
| `row_height` | 18 |
| `header_height` | 24 |
| `status_dot_size` | 6 |
| `checkbox_size` | 14 |
| `settings_label_width` | 160 |

## Typography

| Token | Value | Notes |
|-------|-------|-------|
| `font_size_small` | 11 | Captions, badges |
| `font_size_body` | 13 | Body copy |
| `font_size_heading` | 16 | Section heading |
| `font_size_title` | 22 | Page title |
| `name_size` | 13 | User names in chat |
| `body_size` | 13 | Widget body |
| `small_size` | 11 | Widget captions |
| `heading_size` | 15 | Widget section |
| `title_size` | 20 | Widget title |

## Radii

| Token | Value | Used for |
|-------|-------|----------|
| `border_radius` | 4 | Standard buttons, cards |
| `border_radius_lg` | 8 | Modals |
| `border_radius_widget` | 3 | Widget containers |
| `badge_radius` | 3 | Role badges, chips |

## Component registry

Each widget below must exist in both native (`src/gui/widgets/`) and web (CSS class or shared JS component), with the same visual signature and the same theme tokens consumed.

### Native widgets in `src/gui/widgets/`

| Widget | Native fn | Web class | Tokens consumed |
|--------|-----------|-----------|-----------------|
| Primary button | `primary_button` | `.btn.btn-primary` | `accent`, `text_on_accent`, `border_radius`, `button_height`, `font_size_body` |
| Secondary button | `secondary_button` | `.btn.btn-secondary` | `border`, `text_primary`, `border_radius`, `button_height` |
| Danger button | `danger_button` | `.btn.btn-danger` (web TBD) | `danger`, `border_radius`, `button_height` |
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
| Modal | `modal::modal_dialog` | `.hos-help-backdrop` + `.hos-help-modal` | `bg_modal`, `bg_card`, `border`, `border_radius_lg`, `card_padding`, `modal_width` |
| Help button (`?`) | (new, see below) | `.hos-help-btn` | `border`, `text_muted`, `accent` |
| Help modal | (new, see below) | already done | same as modal |
| Onboarding quest chain | (new, see below) | `.quest-chain` / `.quest-step` | card tokens + `accent`, `border`, `small_size` |

### Widgets planned but not yet shared

- **Toast notification** (`hosToast` on web, `toast` widget in native) — transient feedback.
- **Confirmation dialog** (replaces `window.confirm()` web and `modal_dialog` with Confirm/Cancel in native).
- **Context menu** (color-coded sections for role-based actions).
- **Inline tooltip** (hover-triggered, web has it partially in `shell.js`).

### Web-only (marked explicitly, for now)

- Compact mode toggle (web uses `[data-compact]` attribute to reduce spacing scale).
- Light theme (`[data-theme="light"]`).
- Accessibility overrides (high contrast, reduced motion, colorblind filters).
- Service worker integration.

These do not need native equivalents until there is a clear user benefit. When one is requested, it promotes to the shared list.

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

**Native** consumes via `gui_state.help_registry.show("my-topic-id")` (pending the help-modal widget implementation).

To add a help button next to any UI element:

- **Web:** `<button class="hos-help-btn" data-help-id="my-topic-id" aria-label="Help">?</button>`
- **Native:** call `help_button(ui, theme, "my-topic-id", &mut help_state)` (pending implementation).

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
- [ ] `scripts/gen-theme-css.js` generates `web/shared/theme.css` from `theme.ron`. **In progress v0.91.5.**
- [ ] Web color palette aligned with native. **Migration required; web currently uses `#FF8811` but native uses `#ED8C24`.**
- [x] Universal help modal (web).
- [ ] Universal help modal (native). **In progress v0.91.5.**
- [x] Onboarding page (web, `/onboarding`).
- [ ] Onboarding page (native, `GuiPage::Onboarding`). **In progress v0.91.5.**
- [ ] Toast notifications (both).
- [ ] Confirmation dialog (both).
- [ ] Context menu with role-colored sections (both).

When a checkbox flips to `[x]`, bump its status and the version in the "Last updated" line above.
