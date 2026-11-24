# Chat layout: web view rebuilt to mirror native (sync backbone)

> **Status:** active (2026-05-26). Operator directive: rebuild the web chat
> **view** from scratch to mirror the native app 1:1, **keep the proven JS
> engine**, and make ongoing sync *mechanical* (one web module per native
> draw-function, same names). Live chat is non-precious during the rebuild
> (no users), we can break + swap in place. This doc is the single map both
> sides implement; it's also the build spec.

## Principle

**Native (egui, Rust) is canonical. The web view mirrors it.** They do NOT
share code, web is JS/DOM, native is Rust/egui. They stay consistent via:
the shared **relay + wire protocol**, the **crypto spec** (KAT-locked), the
shared **theme tokens** (`theme.ron` → `theme.css`), and *this map*.

Web stays **DOM-based** (never canvas): keeps screen-reader accessibility,
text selection, keyboard nav, and the rebuild is the moment to make those
*better* (semantic HTML + ARIA), serving the TIER 3 accessibility mandate.

## Engine ↔ View boundary (the clean line)

The web chat splits into two layers. Today they're tangled in `app.js`; the
rebuild draws the line cleanly.

- **Engine (KEEP, view-agnostic JS):** WebSocket client + protocol, state,
  Dilithium/Kyber crypto (`crypto.js`/`pq.js`), DM E2EE, **WebRTC** voice/
  video/streaming (`chat-voice*.js`), reactions/threads/search/pins data.
  The engine never touches the DOM directly after the rebuild.
- **View (REBUILD, `web/chat/view/*`):** pure rendering. Subscribes to
  engine events, calls engine actions. No protocol/crypto logic.
- **Boundary = the existing event bus** (`web/shared/events.js`,
  `hos.on/off/emit`). Engine emits (`chat:message`, `chat:peers`,
  `chat:channel`, `chat:reactions`…); view listens + renders. View calls
  engine actions (`engine.send`, `engine.react`, `engine.switchChannel`…).
  No WS handler ever calls a render function directly again.

## Native → web component map (1:1, same names)

Each native `draw_*` becomes a web `view/*` module of the matching name.
Native refs are `src/gui/pages/chat.rs` unless noted.

| Native fn | Web module | Renders |
|---|---|---|
| `draw_left_panel` (383) | `view/leftRail.js` | left rail container |
| `draw_scratchpad_row` (523) | `view/leftRail.js` → `scratchpadRow()` | `# scratchpad` top row |
| `draw_dm_section` (570) | `view/leftRail.js` → `dmSection()` | DMs (red) |
| `draw_groups_section` (764) | `view/leftRail.js` → `groupsSection()` | Groups (green) |
| `draw_servers_section` (1131) | `view/leftRail.js` → `serversSection()` | Servers (blue) → channels/voice |
| `draw_center_panel` (1730) | `view/centerPanel.js` | header + message list + composer |
| `message_row` (`widgets/row.rs`) | `view/messageRow.js` | one message row |
| `paint_avatar` (`widgets/row.rs`) | `view/messageRow.js` → `avatar()` | 32px gutter avatar |
| `paint_timestamp_pill` (4301) | `view/timestampPill.js` | `HH:MM Þ` + inline reactions |
| `compute_pill_width` (4227) | (n/a, CSS auto-sizes) |, |
| `draw_role_badges` (4120) | `view/roleBadges.js` | role/verified badges |
| `draw_right_panel` (1537) | `view/rightRail.js` | right rail container |
| `draw_friends_section` (1656) | `view/rightRail.js` → `friendsSection()` | Friends (collapsible) |
| `draw_members_section` (1688) | `view/rightRail.js` → `membersSection()` | Members (collapsible) |
| `draw_user_row` (1554) | `view/rightRail.js` → `userRow()` | one friend/member row |
| `draw_panel_lock_button` (3991) | `view/layout.js` → `panelLock()` | rail lock toggle |
| `draw_pins_modal` (217) | `view/modals/pins.js` | pinned messages |
| `draw_search_modal` (281) | `view/modals/search.js` | message search |
| `draw_user_modal` (3195) | `view/modals/user.js` | user profile/context |
| `draw_create_channel_modal` (3658) | `view/modals/createChannel.js` | |
| `draw_add_server_modal` (3714) | `view/modals/addServer.js` | |
| `draw_edit_channel_modal` (3795) | `view/modals/editChannel.js` | |
| `draw_create_group_modal` (3894) | `view/modals/createGroup.js` | |
| `draw_join_group_modal` (3940) | `view/modals/joinGroup.js` | |
| `draw_help_modal` (4680) | `view/modals/help.js` | |
| `draw_unencrypted_dm_modal` (4840) | `view/modals/unencryptedDm.js` | |

Web MAY render richer visuals where the DOM beats egui (real identicons,
emoji pictures, inline video, link previews), but must NOT diverge in **structure,
order, or behavior**. egui's constraints define the shape.

## Sync rules

1. **Same names**, a native `draw_x` has exactly one `view/x` (or `x()`).
   Native changes → open the same-named web file. No searching.
2. **Tokens only**, colors/spacing/radii/fonts from `theme.css` vars
   (generated from `theme.ron`). No literals in the rebuilt CSS.
3. **Shared layout constants**, `view/constants.js` holds the few structural
   numbers native hardcodes (avatar 32px, userbox gap 8px, pill radius 9px)
   so they can't drift. Mirror of `row.rs` consts.
4. **One CSS file for the rebuilt view**, `view/chat-view.css`, organized in
   the same section order as this table. Retire the patched
   `messages.css`/`sidebar.css`/`style.css` as sections migrate.

## Build order (rebuild in place; break freely)

1. **Scaffold**, clean semantic `index.html` (header / leftRail / center /
   rightRail / composer landmarks + ARIA) + `view/constants.js` +
   `view/chat-view.css` skeleton + the event-bus boundary.
2. **Engine extraction**, draw the line in `app.js`: keep logic, route all
   rendering through emitted events; delete inline DOM-render calls.
3. **centerPanel + messageRow + timestampPill**, the main surface.
4. **leftRail** (scratchpad/DMs/groups/servers).
5. **rightRail** (friends/members).
6. **composer + header**.
7. **modals** (pins/search/user/…).
8. **Sweep**, delete the old patched view files + dead CSS; verify against
   native screenshot.

Each step is screenshot-verified by the operator (UI is judged on a real
build). Accessibility (ARIA roles, labels, keyboard, focus) is built in per
component, not bolted on after.

## The phone layout (planned 2026-11-23; not built yet)

The operator, 2026-11-23, on his OnePlus 9 Pro (412 x 915 points) with v0.1571.0: "the chat page
UI needs work ... the go live and open studio buttons could be hidden in the studio page instead
of also being in the chat page." The first cleanups shipped the same day (Go Live and Open Studio
left the page, the room's description stops before Copy link, every window fits the phone:
`chat/phone_tests.rs`, `widgets/window_fit.rs`). This section is the whole of the phone pass, so
the rungs below are pieces of one layout, not separate patches. Native egui first
(`chat/phone.rs` owns the phone's Chat), the website mirrors it under its phone width.

### What a phone shows today, top to bottom

| Row | Height (points) | What |
|---|---|---|
| Status bar | about 24 | Android's (the visible area starts under it) |
| Primary menu row | 48 | the categories, More, the Aa switch, the bell, help |
| Secondary menu row | 40 | Chat's tabs and a second More |
| Server strip | about 36 | the server's name, Connected, Switch |
| Rooms back row | about 52 | the Rooms button (a 44-point touch target and its margin) |
| Room header | about 44 | `# room`, its description, Copy link |
| Messages | what is left | |
| Message box row | about 52 | the box, search, pins, help, Attach, Send |
| Warning strip | about 28 | the unfinished-software warning |
| Navigation bar | about 48 | Android's |

Above the messages that is about 220 points of the 843 the bars leave, a quarter, with the menu's
More twice (once in each row) and the message box sharing its row with five buttons.

### The target

1. **One compact header row** on a phone: the Rooms arrow, then the server and room as one line
   ("server / # room", cut short with an ellipsis), the room's description folded under it in the
   small style (one line, cut short, the whole of it in the room's information on a tap), and one
   menu button at the right holding Copy link, Switch server, the room's information and its
   notification setting. The server strip and the Rooms back row fold into it on a phone; a desktop
   keeps them as they are.
2. **No duplicate More.** On a phone the secondary row's More and the primary row's More are one:
   the primary row keeps its More, the secondary row shows only the tabs that fit and scrolls
   sideways for the rest (the egui ScrollArea the page's tab rows already use), or folds into the
   primary row's More when a page has one tab.
3. **The message box on its own full-width row.** The box takes the whole width, growing upward as
   today (`chat/composer.rs`); under it one row of Attach, one menu button (search, pins, the
   slash-command help) and Send. The menu's items keep their hints by key.
4. **At most a fifth of the screen above the messages.** On the operator's phone that is 168 of
   843 points: the two menu rows (88) and the compact header (about 64 with its description line)
   come to about 152. A test lays the page out at 412 x 915 with the bars and holds the rows above
   the message list to a fifth of the visible height, and the message box row under the list to a
   tenth.
5. **The keyboard.** With the keyboard up (about 343 points on that phone) the menu rows stay, the
   header shrinks to its one line, and the list keeps at least four lines of messages above the
   box (`gui/visible_area.rs` already gives the page only the area above the keyboard).

### Rungs, each built, tested at 412 x 915 and seen in a snapshot

1. The compact header (server strip and Rooms row folded in on a phone), its menu.
2. The message box row and its menu.
3. One More.
4. The fifth-of-the-screen test over the whole page, with and without the keyboard.
5. The website: the same rows under its phone width (`web/chat/`, the chat's CSS media query at
   600 px), held by a web test of the same fraction.

What it costs: nothing per frame beyond today's (the rows are the same widgets, fewer of them); no
new data shapes (the menus list existing actions; a room's information is what the room already
carries); no network change.
