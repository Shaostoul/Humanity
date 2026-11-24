# Navigation redesign: the Menu button, the breadcrumb, the bell and the Notifications page

**Status (2026-11-23): PLAN ONLY, nothing built.** Written from the operator's words on his phone
(v0.1572.0, a OnePlus 9 Pro, 412 x 915 points). It extends, and in places replaces, the top menu of
`docs/design/navigation.md` (the double menu, the layers, Back and Forward) and builds on the bell
and list of `docs/design/notifications.md` (rung 1 and rung 2 part 1). The phone's Chat layout is
`docs/design/chat-layout.md` ("The phone layout"); the widgets and words rules are
`docs/design/ui-system.md`. Native egui first, the website mirrors it.

## The operator's words (2026-11-23, verbatim)

"Let's turn the main menu into a hamburger that's a table of contents like scrollable noncollapsible list. Like it takes up the full screen since mobile phone screen space is so limited and people with bad eyesight will be using it. We'll have like the hamburger icon followed by the text "menu" as a RGB channeling button then we can do the breadcrumb after with the H icon, parent page, sub page. Let's do a like right arrow, right bullet, or middle dot text character. between the H icon, parent, child. Then top right we'll have a notification bell icon that can pull up a full page notifications list. We'll have to build something like that for desktop too. We'll want a dedicated page for notifications and a simple popup of the most recent. Then clicking the notification should take the user to the relevant page. Or if it's a chat it loads the channel and sets the view to the reply so the user can read the full thing and previous/future conversation. We can also move that button that switches buttons from icon+text, icon only, text only to inside the hamburger as an extra chapter thing at the bottom with the 3 toggles listed with text. I guess the right term would be like radial buttons. That way the breadcrumbs can change to user preference. Also, the mobile shows the ? button at the top right that displays desktop shortcuts like F1. I can't do F1 on my phone. We can keep the forward/back page buttons at the top left most side. We can make their icon be the animated RGB like we want the menu button border and the dots between breadcrumbs. Also, when I log in on my phone and go to chat have it load the last channel or default to global. Right now it loads the server list which is kind of confusing since when I click on the global channel it doesn't load the channel text chat area. Right now I have to click to go to another channel, then click "Rooms", the click back to global for it to show. I have no way of closing the rooms list without switching channels."

The last three sentences (open the last channel, tap the selected room, close the rooms list) and
the "?" on a phone were fixed the same day (Part A, section 9). Everything else is this plan.

## 1. What there is today (checked 2026-11-23)

| Piece | Where | Note |
|---|---|---|
| The double top menu: the primary row (categories, More, the button-display switch, the bell, "?") and the secondary row | `src/gui/pages/escape_menu.rs`, data in `data/gui/navigation.json` | 48 and 40 points; on a phone the two rows cost 88 points of 843 |
| Back and Forward | `docs/design/navigation.md` rung 2, `src/gui/history.rs` | already drawn at the left |
| The button-display switch ("Aa", Both, IconOnly, TextOnly) | `state.nav_display_mode`, `NavDisplayMode::next` (cycles), `nav.display_mode` words | one button that cycles; hard to know what the next press gives |
| The notices bell, its count and a small panel | `src/gui/notice_panel.rs`, `widgets/notice_bell.rs`; web twin in `web/chat/chat-notify.js` and the shell | notifications.md section 3, item 3; entries open their conversations |
| RGB cycle tokens | `theme.ron`: `animations_enabled`, `nav_separator_animation`, `nav_active_border_animation`, the `anim` codes (OFF, SOLID, RGB_CYCLE, PULSE) | the active tab's border and the row separator already cycle |
| The "?" help toggle | `src/gui/pages/keymap.rs` `draw_help_toggle` | its panel lists the desktop's keys; hidden on a phone since 2026-11-23 |

## 2. The top bar, left to right

`[Back][Forward]  [ ≡ Menu ]  (H) · Parent · Child ........................ [Bell]`

1. **Back and Forward** at the far left, as now. Their glyphs take the shared RGB treatment (3.3).
2. **The Menu button**: the hamburger icon followed by the word "Menu" (key `nav.menu`), a finger
   tall (the theme's `touch_target`), with the animated RGB border. It opens the Menu page (section 4).
   On the website it is the button that replaces the single row's wrapped links under 640 px.
3. **The breadcrumb**: the H (home) icon, the parent page, the child page, as buttons: tapping the
   H goes Home, the parent goes to the parent page's landing, the child is the current page (not a
   button, marked as current for screen readers). A page with no parent shows H and itself only;
   Home shows H alone. Where a page's parent is comes from `navigation.json` (the category the page
   sits under), never a second table.
4. **The separator**: the three characters he named, offered as a setting (Settings, Appearance,
   "Breadcrumb separator", three radio buttons with words), default **the middle dot**.
   Recommendation and why: the middle dot is the narrowest of the three and is read by screen
   readers as a pause, not as a word ("greater than", which the right arrow is read as on some
   readers); on 412 points the breadcrumb of three levels with "Home · Farming · Beds" must not
   wrap. The arrow reads best for people with low vision who want direction; the setting costs one
   key per choice and a string (data, not code: `data/gui/breadcrumb_separators.json`, so a fourth
   can be added without code, the infinite-of-X rule).
5. **The bell** at the top right (section 6), with its unseen count. The "?" is not in this row:
   help moves into the Menu (a chapter, "Help", holding the guidance for the current page, and on a
   desktop the key list).
6. **When the breadcrumb is too long** for the width: the middle levels shorten to an ellipsis
   button, in this order: parent first, then the child's text is cut with an ellipsis; the H is
   never dropped. The whole path is always in the Menu page's header and in the child's accessible name.

### 2.1 The one animated RGB treatment (3.3 of this plan, "the Pulse")

Back, Forward, the Menu border and the separators share ONE phase, so they breathe together
instead of beating against one another. Tokens (all in `theme.ron`, editable in Settings by
`theme_editor_coverage`): `nav_rgb_style` (the `anim` codes), `nav_rgb_speed`, `nav_rgb_saturation`,
`nav_rgb_static_color` (the fallback), so no literal colour lives in code.

**Fallbacks, in order of strength.** The system or the app's reduce-motion setting (`reduce_motion`,
and the OS's, where the platform tells us) or `animations_enabled = false` turn the cycle OFF: the
border is the static colour token, the separators are the muted text colour. No information is
carried by the colour alone (the Menu button has its icon and word; the current breadcrumb level is
the one that is not a button), so the static form loses nothing but decoration.

### 2.2 The per-frame budget of the animation

The cycle must not keep a phone redrawing at 60 or 120 frames a second. The approach:

- **Quantise the phase.** The colour is a function of `floor(time * steps_per_second)`, 8 steps a
  second at the default (a hue step of 1/48 of the wheel at 6 s per cycle is invisible as banding on
  a 2-point border). The frame asks egui for a repaint **after the next step** with
  `ctx.request_repaint_after(1/8 s)`, never `request_repaint()`. The idle phone then draws 8 frames
  a second while the bar is visible and nothing else changes, instead of 60.
- **Draw only what changes.** The animated pieces are a handful of strokes and glyph tints (about 12
  shapes); a frame that only advances the phase builds the same galleys (egui caches text layout), so
  its cost is the tessellation of those shapes, well under 0.1 ms. The budget stated in the design:
  at most 8 repaints per second from the bar, at most 0.2 ms each on the operator's phone; a test
  counts the repaint requests of an idle bar over a simulated 10 seconds (at most 80 plus the
  frames input caused) and fails over that.
- **Stops entirely** when the window is hidden, the screen is off, a full-screen page that covers the
  bar is open (the Menu page, the game's own view), on reduce-motion, and in the phone's battery-saver
  state where the platform reports it (Android `PowerManager.isPowerSaveMode`, read through the
  existing JNI bridge used by the visible-area code).
- **The website** does the same with a CSS custom-property hue animated by `@keyframes` on the
  compositor (a `filter` free, `background-position` free gradient on the border using a conic
  gradient clipped by `mask`), stepped with `animation-timing-function: steps(48)`, and
  `@media (prefers-reduced-motion: reduce)` plus the `html[data-motion="off"]` setting give the
  static colour. No JavaScript timer runs.

## 3. Sizes, accessibility, words

- **Sizes.** At 412 x 915 the bar is one row of the theme's `touch_target` (48 points), the Menu
  button at least 96 points wide with its word; the secondary row of `navigation.md` is removed on a
  phone (its pages are in the Menu), which returns 40 points of height to the page. On the 65-inch
  TV at 2560 x 1440 four feet away the same bar scales with the app's text-size setting (the
  `Aa`/zoom setting, `docs/design/ui-system.md`, Tokens): the Menu and breadcrumb text use the body
  size, which the TV profile raises; the Menu page's text is the title size (about 1.4 x body),
  never below 20 points at the TV profile.
- **Screen-reader names.** The Menu button: "Menu, button, opens the list of every page". Each
  breadcrumb level: its page name, "Home" for the H, "current page" on the last. The bell: "Notifications, N unread".
  The separator is decorative and hidden from assistive technology (the breadcrumb is a list in
  order, the way a `nav aria-label="Breadcrumb"` with an `ol` is on the web; on native the same
  `WidgetInfo` labels).
- **Hints by key**, every control (the word-source rule): `nav.menu_hint`, `nav.breadcrumb_home_hint`,
  `nav.breadcrumb_parent_hint`, `nav.bell_hint`, and the preference radios'. Words by key with
  `scripts/i18n-add.js`.
- **Focus.** Tab order: Back, Forward, Menu, the breadcrumb levels, the bell. Opening the Menu moves
  focus to its first chapter heading; closing it returns focus to the Menu button (a11y walk test).

## 4. The Menu page: a table of contents

A full-screen page (a page of the app, `GuiPage::Menu`, with its own Back like any page; on the
website `/menu`), never a drawer or a popup: on a phone it takes the whole screen, as the operator
asked, for people with bad eyesight.

- **Content**: every page of `navigation.json`, grouped as **chapters** (the categories), each
  chapter a heading and its pages as large rows (a finger tall, the title size, the page's icon and
  name, one plain-language line under it from the page's hint key). **Scrollable, and nothing
  collapses**: no accordions, no hidden More (the operator: "noncollapsible"); the whole list is
  one scroll. A letter index is NOT added (a long list is short enough to scroll; revisit if the
  chapter count passes about 12).
- **Chapter order**: the order of the current primary row (so what people learned still holds),
  then **Settings**, **Help**, and last the chapter **How buttons show** (the display preference).
- **Help** holds the current page's guidance (what "?" opened) and, on a device with a keyboard,
  the key list; this is where the "?" went.
- **The last chapter, "How buttons show"** (the operator: "an extra chapter thing at the bottom with
  the 3 toggles listed with text ... radial buttons"): three radio buttons with words: "Icon and
  text", "Icon only", "Text only" (keys `nav.display_both`, `nav.display_icon`, `nav.display_text`),
  each with a one-line hint of what it changes ("Smaller buttons; every icon keeps its name for
  screen readers"). It sets `state.nav_display_mode` (the existing `NavDisplayMode`), which the
  breadcrumb, the Menu button and the pages' buttons follow: with Icon only the breadcrumb is the H
  and the pages' own icons (the page names stay in their accessible names), with Text only the words.
  The Menu button itself keeps its icon AND word in "Icon and text" and "Text only" (text), and
  shows the icon alone in "Icon only", with its accessible name unchanged.
- **Where the old header goes**: the current Aa button that cycles the mode leaves the primary row
  (the cycle is replaced by the three explicit choices); the secondary row's pages become the
  Menu's rows; the primary row's More disappears (nothing is hidden any more). On a desktop the
  two-row bar of `navigation.md` stays available as a setting ("Top bar: Menu and breadcrumb /
  the two rows"), default the Menu and breadcrumb on a phone and the two rows on a desktop until
  the operator says otherwise (question 2).

## 5. Rungs, in order

1. **The data and the words**: `navigation.json` gains each page's parent and icon (it has the
   category), `data/gui/breadcrumb_separators.json`, the tokens in `theme.ron`, the keys. A pure
   function `breadcrumb(page) -> Vec<Crumb>` with its tests (every page has a parent chain ending at
   Home; no page's chain loops).
2. **The Menu page** (native, then web): the chapters, big rows, the radio chapter, Help chapter,
   the display preference read by the existing bar.
3. **The top bar**: Back, Forward, Menu, breadcrumb, the bell moved to the right edge; the old
   two rows kept behind the setting; the phone shows the new bar.
4. **The RGB treatment** with the quantised phase, the repaint-count test, and the static fallbacks.
5. **The Notifications page and the popup** (section 6), native then web.
6. **Deep links into chat** (section 7).
7. **The sweep**: remove the Aa cycle button and the phone's secondary row; update `PAGES.md`,
   `FEATURES.md`; snapshots at 412 x 915 and 2560 x 1440.

Each rung is releasable on its own; rungs 2 and 3 are the phone's biggest win.

## 6. Notifications: a page and a popup

Builds on notifications.md rung 1 (the one decision, the list) and rung 2 part 1 (the in-app list,
the bell, entries that open their conversations). What changes: the list stops being only a panel
under the bell.

- **The popup**: tapping the bell opens a small popup (on a phone a sheet under the bar, never
  larger than about 60 percent of the height) with the most recent five entries (their kind icon,
  who and where, the age) and one row at the bottom, "See all notifications". Tapping an entry
  marks it seen and goes to its page (below).
- **The page**: a dedicated page, "Notifications" (`GuiPage::Notifications`, `/notifications`),
  both apps: every kept entry, newest first, grouped by day, filter chips (All, Messages,
  Mentions and replies, Calls, Everything else: the kinds as data, from
  `data/notifications/notifications.json`), "Mark all as seen", "Clear", and a link to Settings >
  Notifications. The Menu has it as a row under the first chapter.
- **Where an entry goes**: each kind names its destination as data (kind -> page and place):
  a DM opens that conversation; a room line, mention or reply opens the room at that message
  (section 7); a call goes to the call; a task to the task; a contact request to that person's card.
  The list entry keeps the destination the notification candidate carried (server, conversation,
  and, new, the message's address).
- **Privacy** (notifications.md section 6) is unchanged: a sealed DM's entry shows what the person's
  "what a notice shows" setting allows, and the list is kept on this device only.
- **Budget**: the page draws only the rows in view (a virtual list); the popup is five rows. The
  list is bounded (the kept count in notifications.md section 9).

## 7. Deep links into a chat message

The operator: "if it's a chat it loads the channel and sets the view to the reply so the user can
read the full thing and previous/future conversation."

- **How a message is addressed**: by the pair the app already uses to target a reaction, the
  sender's key and the message's timestamp in milliseconds, within a conversation. A link is the
  place `Chat { server, channel, message: Option<(sender_key, timestamp_ms)> }`. The existing
  `At::Chat` of `src/gui/history.rs` (Back and Forward) gains the optional message; the website's
  hash `#/chat/<server>/<channel>/<sender8>-<timestamp_ms>`. (The relay assigns no message id today,
  and adding one is a protocol change for later; the pair is unique enough per conversation because
  a sender cannot send two lines in the same millisecond.)
- **Opening**: switch to the server and the channel (the existing `open_place`), then scroll so the
  message is centred, and flash its row once with the accent (no motion if reduce-motion). The
  message list keeps its ordinary scroll afterwards, so reading before and after it is the
  ordinary conversation, not a cut-out.
- **Not in the loaded history** (the page holds a window of recent lines): the page asks the server
  for the history window around the timestamp (the relay's history request takes a `before` bound
  today; a `around` request is one small addition to the history route, rung 6), shows "Loading
  earlier messages" in place, then scrolls. If the server no longer holds it (aged out, deleted) or
  the person is not allowed to read that room any more, the channel opens at its newest line and a
  plain line says "That message is no longer there" (never a silent miss). A DM's message is found
  in the local encrypted store by its timestamp, no server involved.
- **On the phone** this is the conversation column of `chat/phone.rs`, so the rooms list is closed
  and the room shown, the way a tap on a room does since 2026-11-23.

## 8. The gates for each rung

`just verify`, `cargo check --features relay --no-default-features`, the web tests of the touched
files, snapshots at 412 x 915 and 2560 x 1440 (looked at), the a11y Tab walk test with the new bar,
the repaint-count test for the RGB, `theme_token_lint` and `theme_editor_coverage` for the new
tokens, the i18n and hint ratchets, `node scripts/check-doc-links.js` at 0.

## 9. Part A, built 2026-11-23 (the chat page on a phone)

Opening Chat shows a conversation (the last room this person had open on this server, else
`#global`); tapping a room, the one already selected included, opens it; the rooms list has a Close
button; the top bar's "?" is not drawn on a phone (the bell takes its place at the right edge) and
the redesign above moves help into the Menu. Why hidden rather than touch help: the panel it opens
is the desktop's key list, which has nothing a phone can use, and a second help surface would
duplicate the Help chapter planned here. The phone's own Back button closing the list is not wired
(Android delivers it to winit as a "go back" key; `src/engine/frame_keys.rs` is where it would be
handled; it needs a device to verify), so it is a question below.

## 10. Questions only the operator can answer

1. **The separator**: middle dot, right arrow or right bullet as the default? Recommendation: the
   middle dot (narrowest, read as a pause), with the other two as a setting.
2. **The desktop**: should the desktop get the Menu button and breadcrumb too (one bar everywhere),
   or keep the two rows? Recommendation: keep the two rows on the desktop at first, ship the
   Menu and breadcrumb on the phone, and ask again after you have used it for a week; "native
   first, mirror" says one design, but the desktop's width is the reason the two rows work there.
3. **Is the Menu a page you go Back from, or does it cover the page you were on?** Recommendation:
   a page (Back returns to where you were, and it appears in history), since the operator wants it
   to take the whole screen.
4. **Should Android's own Back button close the rooms list and the Menu?** Recommendation: yes,
   when the build can be verified on the phone; it is the way phone people expect to leave a list.
5. **Message ids**: add a server-assigned message id to the protocol (a bigger change, stable
   links) or use the sender key plus millisecond time for now? Recommendation: the pair now;
   a real id when the protocol next changes (after v1.0, with its compatibility cost).
6. **The "last channel" across app restarts**: kept for this run today (and per server while the
   app is open); should it be saved in the settings so a restart returns to it? Recommendation:
   yes, a small per-server saved field (one string), as it is what he described ("load the last
   channel").
