# Chat layout — web view rebuilt to mirror native (sync backbone)

> **Status:** active (2026-05-26). Operator directive: rebuild the web chat
> **view** from scratch to mirror the native app 1:1, **keep the proven JS
> engine**, and make ongoing sync *mechanical* (one web module per native
> draw-function, same names). Live chat is non-precious during the rebuild
> (no users) — we can break + swap in place. This doc is the single map both
> sides implement; it's also the build spec.

## Principle

**Native (egui, Rust) is canonical. The web view mirrors it.** They do NOT
share code — web is JS/DOM, native is Rust/egui. They stay consistent via:
the shared **relay + wire protocol**, the **crypto spec** (KAT-locked), the
shared **theme tokens** (`theme.ron` → `theme.css`), and *this map*.

Web stays **DOM-based** (never canvas): keeps screen-reader accessibility,
text selection, keyboard nav — and the rebuild is the moment to make those
*better* (semantic HTML + ARIA), serving the TIER 3 accessibility mandate.

## Engine ↔ View boundary (the clean line)

The web chat splits into two layers. Today they're tangled in `app.js`; the
rebuild draws the line cleanly.

- **Engine (KEEP — view-agnostic JS):** WebSocket client + protocol, state,
  Dilithium/Kyber crypto (`crypto.js`/`pq.js`), DM E2EE, **WebRTC** voice/
  video/streaming (`chat-voice*.js`), reactions/threads/search/pins data.
  The engine never touches the DOM directly after the rebuild.
- **View (REBUILD — `web/chat/view/*`):** pure rendering. Subscribes to
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
| `compute_pill_width` (4227) | (n/a — CSS auto-sizes) | — |
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
twemoji, inline video, link previews) — but must NOT diverge in **structure,
order, or behavior**. egui's constraints define the shape.

## Sync rules

1. **Same names** — a native `draw_x` has exactly one `view/x` (or `x()`).
   Native changes → open the same-named web file. No searching.
2. **Tokens only** — colors/spacing/radii/fonts from `theme.css` vars
   (generated from `theme.ron`). No literals in the rebuilt CSS.
3. **Shared layout constants** — `view/constants.js` holds the few structural
   numbers native hardcodes (avatar 32px, userbox gap 8px, pill radius 9px)
   so they can't drift. Mirror of `row.rs` consts.
4. **One CSS file for the rebuilt view** — `view/chat-view.css`, organized in
   the same section order as this table. Retire the patched
   `messages.css`/`sidebar.css`/`style.css` as sections migrate.

## Build order (rebuild in place; break freely)

1. **Scaffold** — clean semantic `index.html` (header / leftRail / center /
   rightRail / composer landmarks + ARIA) + `view/constants.js` +
   `view/chat-view.css` skeleton + the event-bus boundary.
2. **Engine extraction** — draw the line in `app.js`: keep logic, route all
   rendering through emitted events; delete inline DOM-render calls.
3. **centerPanel + messageRow + timestampPill** — the main surface.
4. **leftRail** (scratchpad/DMs/groups/servers).
5. **rightRail** (friends/members).
6. **composer + header**.
7. **modals** (pins/search/user/…).
8. **Sweep** — delete the old patched view files + dead CSS; verify against
   native screenshot.

Each step is screenshot-verified by the operator (UI is judged on a real
build). Accessibility (ARIA roles, labels, keyboard, focus) is built in per
component, not bolted on after.
