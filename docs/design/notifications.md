# Notifications

Written 2026-10-30, when the operator answered the open question of 2026-07-01 ("a desktop
notification system, which 'Mute server' needs") with: "let's get it made."

This is the whole plan, built in rungs (CLAUDE.md, "Plan the whole feature, build it in rungs").
Rung 1 is built; section 11 says what each later rung adds and what it needs. Read this before
adding a kind of notification, a control, or a platform.

## 1. What there was before (checked 2026-10-30)

| Piece | Where | State before rung 1 |
|---|---|---|
| Web Push (VAPID) | `web/shared/shell.js`, `web/shared/sw.js`, `src/relay/storage/push.rs`, `src/relay/relay.rs` `send_push_notification` | Works. Sent for a DM and an @mention to a person with no open socket. Ignored the person's preferences entirely. |
| Synced preferences | `notification_prefs` table (`src/relay/storage/notification_prefs.rs`): DM / mentions / tasks on or off, do-not-disturb start and end | Stored and returned. Read by nothing that decides anything. |
| Native ding | `src/engine/frame_ws_poll.rs`, `src/engine/mailbox.rs`, `src/engine/bg_connections.rs` | A sound for a DM (DM switch honoured) and for any line CONTAINING the person's name (no `@` needed), never quiet hours, never a toast. |
| Native Settings > Notifications | `src/gui/pages/settings.rs` | The three switches and two free-text time boxes ("22:00"), words written into the code. |
| Web browser notifications | `web/chat/chat-ui.js` `notifyNewMessage` | Fired for EVERY line in a background tab, with the first 100 characters of the message, a DM's included; ignored the synced switches, quiet hours and the Settings "Message Preview" switch. |
| Mute server | native server cog | Removed in v0.847 because there was nothing to mute (`chat/left_panel.rs`). The website never had one. |

Defects found on the way (BUG-218): the mention push carried the first 100 BYTES of the message
(`&content[..100]`), which panics the sender's socket task when byte 100 falls inside a letter, and
put public-room text into a notice by default; the web's quiet hours were stored as an hour ("22")
while the desktop wrote "22:00"; and four web Settings controls were read by nothing ("Message
Preview", "Notification Sound", the Do Not Disturb section's own quiet hours, and its two
exceptions).

## 2. What notifies: kinds are data

`data/notifications/notifications.json` lists every kind of notice (infinite-of-x: a new kind is a
row there plus the place that raises it). Each kind names:

- `id`; `built` (whether anything raises it yet, so Settings can say so honestly);
- `server_flag`: which synced switch turns it on or off (`dm`, `mentions`, `tasks`, or none);
- `scope`: `person` (a conversation with someone: never muted by a server or room mute),
  `room` (a server's room: server and room mutes apply), `game` (the simulation);
- its words, as `data/i18n/` keys: the label, the hint, the title with the name, the title and
  body used when a notice shows nothing about the sender.

| Kind | Flag | Scope | Rung |
|---|---|---|---|
| `dm`: a private message | dm | person | 1 (built) |
| `mention`: `@your name` in a room | mentions | room | 1 (built) |
| `reply`: a reply to your line | mentions | room | 2 |
| `contact_request`: someone asks to reach you | dm | person | 2 |
| `task`: a task given to you or changed | tasks | person | 3 |
| `call`: a call ringing | dm | person | 3 |
| `game_event`: something in your home worth knowing (a crop ready, air going bad) | none | game | 3 |

The same file lists the mute lengths (`mute_for`: 1 hour, 8 hours, until I unmute) and the sounds
a person can pick (`sounds`, each an id from `data/sounds.toml`). Adding a length or a sound is a
row.

## 3. Surfaces

1. **The operating system's notice** (the toast in the corner, kept in the notification centre).
   - Windows: the WinRT `ToastNotificationManager`, through the `windows` crate already in
     `Cargo.toml` (two features added, `UI_Notifications` and `Data_Xml_Dom`; no new crate). An
     unpackaged program must be known to Windows by an AppUserModelID, so the first notice writes
     `HKCU\Software\Classes\AppUserModelId\UnitedHumanity.HumanityOS` with `DisplayName` and
     `IconUri` (the per-user registration the Windows App SDK uses for unpackaged apps; nothing
     machine-wide, no shortcut). The notice is built as toast XML with every value escaped.
   - Linux: the freedesktop notification service, through `notify-send` (part of libnotify, on
     every mainstream desktop). The body is escaped because the service may read markup.
   - macOS: `osascript`'s `display notification`, with the words passed as arguments, never pasted
     into the script.
   - Why not `notify-rust` (MIT/Apache, maintained): it would bring a second copy of the `windows`
     crate at another version on Windows and `zbus` with its async stack on Linux, a large compile
     for what the crate already in the tree does. Why `notify-send` and not D-Bus directly: the
     `dbus` crate is in the lock file (through `keyring`), but no check compiles Linux in this
     repo before a release, and a call through `std::process` cannot break the Linux build. It is
     the same freedesktop service; rung 5 moves to D-Bus (and to the desktop portal, for Flatpak)
     once CI checks the native build on Linux.
   - Each notice runs on one worker thread fed by a bounded queue: the frame never waits for COM,
     D-Bus or a process, and a burst past the queue is dropped, not queued forever.
2. **The sound**, played by the app's own mixer (`pending_sfx`, the sound catalog), so the choice
   of sound, the volume and quiet hours are ours. The OS notice is silent (`<audio silent>`) so
   the two never double.
3. **The in-app list** (rung 2): a bell in the header with the count of unseen notices, opening a
   panel of the last 100 (kind, who, where, when; never the text unless the person's setting
   shows text), each opening its conversation; the same list on the website. Kept in memory for
   the run (rung 2), then in the encrypted local store beside the DMs (rung 4).
4. **In the world** (rung 2): a short banner at the top of the screen while playing full screen,
   because a toast may sit behind a full-screen game; and the taskbar flash
   (`winit::Window::request_user_attention`) as a cue that needs no sound.
5. **The website**: the browser's Notification API while a chat tab is open in the background, and
   Web Push from the server when no tab is open (`sw.js` shows it; Reply and Mark Read already
   exist there).

## 4. The rule: one decision, two twins

`src/notifications/decide.rs` (desktop) and `web/shared/notify-rules.js` (website) answer the same
question the same way, and both are held to the shared cases in
`scripts/tests/fixtures/notify-cases.json` (a Rust lib test and a Node test read the same file).
The checks, in order, each with the reason it reports:

1. `own_message`: the person's own line or DM (from this device or another of theirs). Never.
2. `unknown_kind`: a kind not in the data file.
3. `not_loaded`: the synced switches have not arrived from the server yet, so quiet hours are
   unknown. Nothing fires until they do (asked for at every sign-in on the desktop, and by the
   website's chat once its socket is accepted, so this lasts one round trip). The browser's own
   copy never counts as loaded: it may be old, or only the defaults (2026-10-30).
4. `kind_off`: the kind's synced switch is off.
5. `all_muted`: everything is muted (Settings > Notifications > Mute everything, and the tray's
   Mute from rung 2), DMs included. Then `server_muted`, `room_muted`: a room kind, from a server
   or a room muted until later (or until unmuted).
6. `quiet_hours`: now (the device's own clock) is inside the quiet hours. A window that ends before
   it starts runs past midnight; a window whose ends are equal is empty.
7. `on_screen`: the window has focus AND that conversation is the one showing (the Chat page or the
   in-world chat panel, on the same server; a shared room's Commons view counts).
8. `too_soon`: a notice for the same conversation went out under 5 seconds ago (a burst of lines
   is one notice, and a flood of mentions cannot turn the computer into a siren).
9. Otherwise it notifies: the OS notice when desktop notices are on AND the window is not focused
   (a focused window gets the sound, and the in-app list from rung 2), the sound when sound is on.
   Both off is `nothing_on`.

What is a mention: `@` and the person's name, any case, not followed by a letter, digit, `_` or
`-` (so `@Al` is not `@Alice`). The relay's push uses `@(\w+)`; the client's test is the stricter
of the two.

What reaches the decision at all: only LIVE messages (`chat`, `federated_chat` is rung 2, `dm_new`),
never history or a mailbox page, so opening the app never replays old notices. Blocked people
never reach it (they are screened before a message is stored), and a room the protected setup
hides never notifies.

## 5. The controls

| Control | Lives | Default | Desktop | Website | Rung |
|---|---|---|---|---|---|
| Direct messages / Mentions / Task updates | server (synced) | on | Settings > Notifications; the DM cog | Settings > Notifications; the DM cog | 1 |
| Quiet hours (from, to, by the hour) | server (synced) | off | Settings > Notifications | Settings > Notifications | 1 |
| Desktop notices on or off | this device | on | Settings | the browser's permission | 1 |
| What a notice shows: nothing / who it is from / who and the message | this device | who it is from | Settings | Settings | 1 |
| Sound on or off, and which sound | this device | on, "Message" | Settings, with a Play button | the chat's sound menu | 1 |
| Show a test notice | this device | | Settings | | 1 |
| Mute everything: 1 hour, 8 hours, until I unmute | this device | | Settings; the tray's right-click (rung 2) | | 1 desktop, 2 tray and web |
| Mute a server: 1 hour, 8 hours, until I unmute | this device | | its cog, a saved server's right-click | the server's bell | 1 |
| Mute a room | this device | | the room's right-click | | 1 desktop, 2 web |
| Muted list, with Unmute | this device | | Settings | Settings | 1 desktop, 2 web |
| Mute a conversation (a person) | this device | | the DM's right-click | the DM's menu | 2 |
| Per-room level: every line / mentions / nothing | this device | mentions | the room's right-click | same | 2 |
| Quiet-hours exceptions: friends' DMs, mentions | server (synced) | off | Settings | Settings (replacing the unread DnD section) | 2 |
| Per-kind sound | this device | | Settings | | 3 |
| Game events, per event | this device | per event in data | Settings | | 3 |
| Show HumanityOS in the notification area (the tray) | this device | on | Settings | n/a | 2 |
| Closing the window keeps HumanityOS running in the tray | this device | off (closing quits) | Settings | n/a | 2 |

### The tray (notification area) icon, rung 2

The operator, 2026-10-30: "Can we add the ability for HOS to run in the taskbar? Then we could add
to the right click menu a button for mute that opens to the available options."

- **The crate: `tray-icon`** (tauri-apps, MIT OR Apache-2.0; 0.26.1 checked 2026-10-30, needs
  Rust 1.90; menus through its sibling `muda`). On Windows it uses `windows-sys` 0.60/0.61, both
  already in `Cargo.lock`. On Linux it is built with its `ksni` feature (a StatusNotifierItem over
  D-Bus, pure Rust), NOT its default `libappindicator`, which needs GTK 3 and the ayatana
  libraries installed on the Linux release runner. On macOS it uses `objc2` and must be created on
  the main thread after winit's loop has started (`resumed`). Beside winit: the tray's own window
  procedure runs on the event-loop thread; its menu and click events arrive on `tray-icon`'s
  channels, read with one `try_recv` per frame, and its event handler forwards to winit's
  `EventLoopProxy` so a click wakes a loop that is waiting with the window hidden.
- **The menu**, every word by key: Open HumanityOS; Mute everything, opening to the same choices as
  Mute server (For 1 hour, For 8 hours, Until I unmute; and Unmute, with "Muted until 14:00" as a
  line that cannot be clicked, while muted); Quit HumanityOS.
- **What the tray's Mute mutes: everything** (the whole app's notices, DMs included), as distinct
  from one server's Mute. It is the same state as Settings > Notifications > Mute everything, which
  rung 1 builds, so the tray adds a second place to reach it and nothing else.
- **Closing the window: a choice in Settings, off by default, so closing still quits** unless the
  person turns it on. When on, closing hides the window instead: the renderer stops drawing (no GPU
  work while hidden), the sockets keep being read at a low rate so notices still arrive, and the
  first time a notice says HumanityOS is still running in the tray. Closing while in the world
  first leaves it the way the Escape menu's Leave does (saved), so a hidden window never simulates
  or renders a world. Quit from the tray runs the same save as closing does today
  (`engine/quit.rs`).
- **The icon shows state where the platform allows it:** on Windows and Linux a second icon for
  muted (a bell with a slash) and for unread (a dot), and the tooltip says "3 unread" or "Muted
  until 14:00"; on macOS the menu-bar item can carry a short title beside its icon (the count).
  Windows also allows an overlay badge on the taskbar button (`ITaskbarList3::SetOverlayIcon`).
- **Why rung 2 and not rung 1:** close-to-tray changes the window's close path in `lib.rs` (the
  merge funnel) and adds a hidden-window mode, and both can only be verified by booting the app,
  which rung 1 deliberately does not do; the crate is a new dependency tree on three platforms.
  The mute it drives is built in rung 1.

## 6. Privacy (safe by default; the operator, 2026-10-09: "Security is definitely something we need to get right. We must protect the users.")

- **A notice shows who it is from, never what was said, unless the person turns that on.** A notice
  is seen by anyone near the screen, lands in screenshots and screen shares, and is KEPT by the
  operating system (Windows keeps them in its notification database, `wpndatabase.db`). The
  setting's own words say so. "Nothing" (only that something arrived) is one click away.
- **A DM's text exists only on the device.** DMs are sealed-sender: the server never sees the
  sender or the text, the app decrypts, and the notice is built on the device after that. The
  Web Push for a DM stays the generic "New message" with no sender, because the push service
  (Google, Mozilla, Apple) handles it.
- **A file's key never reaches a notice:** a file marker (whose text holds the key that opens the
  file) is shown as "Photo" or the file's name (`chat/reply.rs preview_of`), as the DM list does.
- **The mention push no longer carries the text** (it carried the first 100 bytes until rung 1).
- **A notice cannot be made into something else:** every value is escaped for the toast XML (a
  name cannot add a button or a link) and for Linux's markup; control characters go; the title and
  body are cut to 80 and 240 characters. macOS gets the words as arguments.
- **The notification centre holds little:** a Windows notice expires on restart and replaces the
  previous one for the same conversation (its tag is a hash of the conversation, not the name).
- **What is written to the computer:** on Windows, the one per-user registration above, the first
  time a notice is shown, and a copy of the app icon beside the person's data folder. Nothing else.

## 7. Where each setting lives, and why

- **Synced (the server's `notification_prefs` row)**: the three switches and quiet hours. The
  server's Web Push must honour them (it now does for the switches), and a person expects the same
  quiet hours on every device. The desktop asks for them at every sign-in and sends a change to
  EVERY server it is signed in to, so each one's pushes agree.
- **A change names only what it changes (2026-10-30).** `update_notification_prefs` carries the
  switches that changed; every one it leaves out keeps its stored value, and quiet hours sent as
  null are cleared (storage/notification_prefs.rs `NotifPrefsChange`). The server answers every
  connection of that person with the whole stored set. No client writes before the server has
  answered it: the desktop's controls wait (`synced_frame` is `None` until then), the website's DM
  cog says it is still asking, and the website's Settings page, which has no socket, queues just
  the switch that changed (`hos_notify_pending`) for the chat to send. Before this, the server
  replaced all five values, a missing switch counting as on, so one click on the website's DM
  cog in a browser that never opened Settings wiped the person's quiet hours and turned mentions
  back on, on every device.
- **This device** (the desktop's `config.json` `notifications`; the browser's localStorage):
  desktop notices, what a notice shows, the sound, and mutes. What a notice may show depends on
  the screen (a shared family computer, a phone), and a sound is the device's. Mutes are kept
  locally in rung 1; rung 4 tells each server about its own mutes only, so its pushes stop too
  (a server learns that it was muted, nothing about other servers).

## 8. Accessibility

- Never sound alone: every notice is also a toast (read aloud by Narrator, Orca and VoiceOver) or,
  focused, the unread dot; rung 2 adds the in-app list and the taskbar flash for people who cannot
  hear it.
- Every control has its plain-language hint by key (`notifications.*_hint`) and is reachable by
  keyboard; the mute lengths are menu items, not a slider.
- No flashing: the web title alternates every 2 seconds, well under three flashes a second.
- The test notice lets a person check what they will get, without waiting for a message.

## 9. Budgets

- **Per frame, idle: nothing.** The decision runs only when a live message arrives, inside the
  socket pump that already handles it. The only per-frame work is two `Option::take` checks (a
  test notice asked for, a sound preview asked for). The worker thread sleeps on its queue.
- **Per notice:** one pure decision, one string build, one queue send. The toast itself costs the
  worker a COM call (Windows) or a process start (Linux, macOS), never the frame.
- **Network: no new polling, no new routes.** One `get_notification_prefs` per sign-in (before:
  one when a popup first opened), one `update_notification_prefs` per change to each connected
  server. The relay adds one primary-key read of `notification_prefs` per push it would send.

## 10. What the platforms decide (PLATFORM HOLDERS' RULES)

- A browser shows notifications only after the person allows them, and only while it runs; an
  iPhone delivers Web Push only to a site added to the home screen (Apple's rule). The website
  says so where notifications are turned on.
- macOS attributes an `osascript` notice to Script Editor. A notice under HumanityOS's own name
  needs a signed app bundle (Apple's rule for `UNUserNotificationCenter`), which arrives with the
  signed macOS build (rung 5).
- Windows' Focus Assist (Do not disturb) silences our notices like everyone's, which is right.

## 11. The rungs

**Rung 1 (2026-10-30, built):**
- The kinds, mute lengths and sounds as data (`data/notifications/notifications.json`).
- THE decision as a pure function on both apps, with the shared cases.
- The desktop: OS notices for DMs (the active server and every saved server signed in) and
  mentions (the same), on Windows, Linux and macOS; the sound through the same decision; the
  synced switches asked for at every sign-in and sent to every connected server.
- Mute server (1 hour, 8 hours, until I unmute) from the active server's cog and a saved
  server's right-click, a muted mark on the server row; Mute room from a room's right-click.
- Settings > Notifications, rebuilt from keys: this device (desktop notices, what a notice shows,
  sound and which, Play, Show a test notification), what notifies you (the synced switches,
  quiet hours by the hour), Mute everything, and the muted list with Unmute.
- The website: notices only for DMs and mentions, honouring the synced switches, quiet hours,
  mutes and what a notice shows; Mute server on the server row; "A notice shows" in Settings
  (the unread "Message Preview" removed); the DM cog flips the synced DM switch as the desktop's
  does, from what the server holds and sending only that switch; quiet hours written as "22:00".
- The relay: a push only when the person's switch for it is on; the mention push without the text.

**Rung 2:** the tray icon ("The tray", after section 5), with its Mute everything menu, the
close-to-tray choice and the icon's muted and unread states; the in-app list (bell, count, panel)
on both apps; clicking a notice opens its
conversation (Windows: a COM activator or `humanityos://` protocol activation; Linux: the
default action; web: `onclick` switches to it); the in-world banner and the taskbar flash; mute a
conversation; Mute room and the muted list on the website; the per-room level; the reply and
contact-request kinds; Commons (`federated_chat`) mentions; quiet-hours exceptions, replacing the
website's unread Do Not Disturb section and its "Notification Sound" switch.

**Rung 3:** task, call and game-event kinds (the relay sends task events; the tasks switch finally
does something; game events per event, from data, raised by the simulation's systems through the
existing `sfx_events`-style channel); per-kind sound.

**Rung 4:** sync: each server told its own mutes, so its Web Push honours them; quiet hours for
pushes (the device's UTC offset kept with the preferences, or the service worker checking its own
copy); what a push shows, synced, default nothing; the in-app list kept in the encrypted local
store; one set of preferences across all of a person's servers (written to each, newest wins).

**Rung 5:** Linux through D-Bus and the desktop portal; macOS through `UNUserNotificationCenter`
in the signed bundle; Reply and Mark read on desktop notices, as `sw.js` has on the web; Windows
notices grouped per conversation in the notification centre.

## 12. Gates

- `cargo test --lib notifications` (through `just test-lib notifications`): the shared cases, quiet
  hours, mutes, mention matching, toast XML and argument escaping, the data file's keys all in
  `en.json`, the settings round trip through `config.json`.
- `node --test scripts/tests/notify-rules-web.test.js`: the website's twin on the same cases.
- `src/relay/storage/notification_prefs.rs` tests: a push only when its switch is on.
- Snapshots: `just snapshot notification_settings`, `just snapshot chat_muted_server`.
- A real notice cannot be snapshotted: Settings > Notifications > Show a test notification is how
  a person (and the operator) checks one, and a failure is logged to `run.log` with Windows'
  error code.
