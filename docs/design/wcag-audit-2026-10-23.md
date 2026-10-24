# WCAG 2.1 AA audit of HumanityOS (desktop app and website)

**Date of the audit: 2026-10-23.** Re-check against this date: a later change to a page, a theme
token or a shared widget may have moved any line below.

Part of the ELI5 mandate (`docs/PRIORITIES.md` TIER 3): what was checked, how, what passed, what
was fixed, and what remains, for the desktop app (Rust, egui) and the website (HTML and script),
against the Web Content Accessibility Guidelines 2.1 at level AA (https://www.w3.org/TR/WCAG21/).
This is an engineering audit by the project, not a certification.

## How

- **Automated, in the build.** Each check was made to fail on purpose (or run against the code
  before its fix) before it was trusted:
  - `scripts/tests/contrast.test.js` (`just rig-tests`): every text and control colour pair the
    UI paints, from the theme tokens, dark theme (both apps) and light theme (website).
  - `scripts/tests/a11y-web.test.js` with `scripts/a11y-web.js` (`just rig-tests`): every web
    control has an accessible name, a clickable element that is not a control is reachable by
    keyboard, no tabindex above 0, every page has a language and a title, no page stops zoom,
    every picture has an alt. A ratchet holds every page to its count; the converted pages are
    held at zero.
  - `src/gui/a11y_tests.rs` (`just test-lib a11y_`): the desktop pages walked with Tab, headless:
    every stop named and showing its hint on focus.
  - `src/gui/widgets/hint.rs` (`a_hint_shows_on_keyboard_focus_unless_hints_are_off`) and the
    hint ratchets (`tests/i18n_lint.rs`, `scripts/tests/hints-web.test.js`).
  - `scripts/tests/glossary-terms.test.js`: every glossary mark names a definition that exists.
- **By hand, in a real browser** (Chromium, the Claude desktop app's browser pane, pages served from
  the working tree) at desktop width and at 375 px phone width: Tab order, focus rings, Enter and
  Space on cards, text at 200 %, the size of every control.
- **By reading the code** where neither could reach (egui's screen-reader wiring, motion settings).

The pages examined most closely are the ones the ELI5 pass converted: Calculator, Notes, Tasks and
Quests, Calendar, Crafting, Laws, Wallet, Donate, Bugs, Guilds, Governance, the Mission Dashboard
(Humanity), Civilization, Market, and Settings > Appearance. The automated checks run over every page.

## Results by criterion

| WCAG 2.1 | What it asks | Desktop app | Website |
|---|---|---|---|
| 1.1.1 Non-text content | a text alternative for pictures | n/a (no informative images on the converted pages) | **Pass** on the converted pages; 2 pictures without alt elsewhere (chat, `shared/qrcode.js`), in the ratchet |
| 1.3.1 Info and relationships | a label tied to its control | **Fixed:** a Settings row now names its control (`settings_row` passes its words to a box or slider that paints itself) | **Fixed:** 14 `<label>`s gained `for=` (Bugs, Calendar, Tasks, Guilds), 15 controls with no label gained an `aria-label` by key (Calculator, Bugs, Wallet, Notes), and Settings > Hints and Language are tied to their words (`aria-labelledby`) |
| 1.4.3 Contrast (text) 4.5:1 | words readable on their background | **Fixed:** white words on the dark theme's success green were 2.41:1, on its danger red 4.08:1, and on the admin, moderator, verified and live badges 2.81 to 3.82:1; filled buttons and badges now pick white or black per fill (`Theme::ink_on`): 5.15:1 at worst | **Pass** (light theme 4.53:1 at worst, the accent as link text); the website's danger buttons are outlined, so their words are danger on the page, 5.02:1 |
| 1.4.4 Resize text | text at 200 % without loss | Settings > Appearance > Font Size, 10 to 24 pt, every page reflows | **Pass** at 200 % on the converted pages; Maps stops pinch zoom (`user-scalable=no`), in the ratchet, left for the Maps owner |
| 1.4.10 Reflow | no sideways scroll at 320 to 375 px | n/a (a desktop window) | **Pass** at 375 px on Calculator, Notes, Crafting, Tasks, Market |
| 1.4.11 Non-text contrast 3:1 | a control's outline, a slider's rail, a focus ring | **Fixed:** the slider rail (`slider_track`) was 1.65:1 on a card; raised to 3.37:1 in `theme.ron`. Checkbox outline and focus ring pass (4.20:1 at worst) | **Pass** |
| 1.4.13 Content on hover or focus | a hint that can be reached, kept and dismissed | **Fixed:** a hint shows on keyboard focus as well as on hover (not while typing in a text box); hints can be turned off (Settings > Appearance > Hints) | the shell's hint shows on hover and focus; a glossary card stays while the pointer is on it, pins with a click or Enter, closes with Escape |
| 2.1.1 Keyboard | everything works from a keyboard | **Fixed:** the custom slider moves with the arrow keys once Tab reaches it (it ignored the keyboard). Buttons and boxes answer Space and Enter | **Fixed:** 16 kinds of clickable card and row (notes and log entries, quests and their tick box, task cards, quest categories, validators, collectibles, calculator history, colour and icon pickers) gained `role="button" tabindex="0"`, and the shell answers Enter and Space for any such card; glossary words are reachable with Tab |
| 2.1.2 No keyboard trap | Tab leaves every part | **Pass** (the Tab walk comes back round on every converted page) | **Pass** |
| 2.4.1 Bypass blocks | skip the repeated menu | the page is the only panel with focusable controls | **Fixed:** a "Skip to the page" link is the first Tab stop on every page |
| 2.4.3 Focus order | a meaningful order | **Pass** (egui walks in drawing order) | **Fixed:** Tab walked into the CLOSED "More pages" drawer on every page (about 40 hidden links before the page); the drawer is `inert` while closed. No tabindex above 0 anywhere |
| 2.4.7 Focus visible | where the keyboard is, visibly | **Fixed:** the project's buttons, checkboxes and sliders paint their own colours and hid egui's focus look; a 2 px `border_focus` ring now shows on keyboard focus | **Fixed:** many pages set `outline: none`; `theme.css` draws a `:focus-visible` ring on every control (keyboard only, so the mouse look is unchanged) |
| 2.3.3 / motion | turn motion off | Settings > Animations is a master switch; it does not follow the system's reduce-motion setting yet | follows `prefers-reduced-motion` and has its own switch (`shared/accessibility.js`) |
| 3.1.1 Language of page | the page says its language | n/a | **Pass** on every page |
| 3.3.2 Labels or instructions | what to type | every control on the converted pages has a plain-language hint (phase 2) | the same, `data-tip-desc` by key |
| 4.1.2 Name, role, value | assistive technology knows what a control is | the Tab walk names every button, box and list label; 233 stops on the converted pages have no name yet (Laws 191 and Calendar 32, mostly rule rows and day cells painted by hand, plus a few lists made without a label; 10 more across Bugs, Tasks, Humanity, Crafting, Market and Notes), held by a ratchet in `a11y_tests.rs`; **no screen reader can read any of it yet**, see below | **Pass** on the converted pages; 77 unnamed controls remain elsewhere (Settings 35, Chat 15, Profile 7, Inventory 6, Dev 4, Trade 3, Files 2, Admin 1, Maps 1), in the ratchet |
| Target size (WCAG 2.2, 2.5.8, 24 px) | a finger can hit it | n/a | **Fixed** at phone width: the footer links and toggle, the calculator's Copy and history Clear, the Notes sort list and the Crafting chips were 12 to 23 px tall; now 24 px. Links inside a sentence are exempt |

## The biggest gap: no screen reader can read the desktop app

egui builds an accessibility tree (AccessKit) when a platform adapter asks for one, but this app
does not create the adapter: `egui-winit`'s `accesskit` feature is off and the event loop has no
user event type. The adapter (`accesskit_winit`) must be made before the window is first shown,
and the window is created already visible on purpose (`src/engine/launch_focus.rs`, the
focus-stealing fix of v0.1069), so the two need reconciling before it can be turned on. Until then
the names the desktop app sets (checked by the Tab walk) reach no screen reader. This is the next
accessibility increment for the desktop app.

## What remains (all in the ratchets, so none can grow)

- **Website:** 147 static problems on pages this pass did not convert: Settings (59, mostly rows
  whose words are not tied to their control, and the section list), Inventory (30), Chat (34),
  Profile, Dev, Trade, Files, Admin, Roadmap, Maps (its viewport stops zoom), the in-page browser's
  tab close button in `shell.js`, and the QR picture. `node scripts/a11y-web.js` lists them.
- **Desktop app:** the screen-reader adapter (above); names for the 233 unnamed Tab stops (above); following the system's reduce-motion
  setting; the slider thumb's colour cycle runs whatever Settings > Animations says.
- **Pages left to their owners** during this pass (other work was open in them): Maps, the admin
  console (Server Settings, Relay Control, Game Admin, Host Node; `admin.html`, `ops.html`),
  Identity and Profile, Chat (voice and direct messages), federation, Settings > Data (backups),
  the HUD's power meters, Files, and Homes.

## Visible changes a person will notice

- Danger and success buttons on the dark theme now have **black** words on their fill (the white
  was too faint on both), and badges pick black or white per colour.
- Slider rails are lighter.
- A keyboard user sees an orange ring around the control they are on, on both apps, and a "Skip to
  the page" link on the website's first Tab.
