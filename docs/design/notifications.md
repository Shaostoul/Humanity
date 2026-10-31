# Notifications

Written 2026-10-30, when the operator answered the open question of 2026-07-01 ("a desktop
notification system, which 'Mute server' needs") with: "let's get it made."

This is the whole plan, built in rungs (CLAUDE.md, "Plan the whole feature, build it in rungs").
Rung 1 is built, and rung 2's first part (2026-10-31: the in-app list, the new mutes, levels,
kinds and exceptions); section 11 says what each part and later rung adds and what it needs.
Read this before adding a kind of notification, a control, or a platform.

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
- `scope`: `person` (a conversation with someone: never muted by a server or room mute; muting
  that conversation stops it), `room` (a server's room: server and room mutes apply, and the
  room's level), `game` (the simulation);
- `level` (a room kind, rung 2): the least a room must be set to for it to notify (nothing <
  mentions < all; a room is at mentions unless the person sets it otherwise). Every room kind
  needs at least mentions, so a room set to nothing never notifies (a lib test and the web test
  hold the file to it);
- `quiet_exception` (rung 2): the synced quiet-hours exception that lets the kind through quiet
  hours, `friends` (from a friend) or `mentions`; none when absent;
- its words, as `data/i18n/` keys: the label, the hint, the title with the name, the title and
  body used when a notice shows nothing about the sender.

| Kind | Flag | Scope | Level | Quiet exception | Rung |
|---|---|---|---|---|---|
| `dm`: a private message | dm | person | | friends | 1 (built) |
| `mention`: `@your name` in a room | mentions | room | mentions | mentions | 1 (built) |
| `reply`: a reply to a line of yours | mentions | room | mentions | mentions | 2 (built) |
| `contact_request`: someone asks to reach you | dm | person | | | 2 (built) |
| `room_line`: any line in a room you set to every line | none | room | all | | 2 (built) |
| `task`: a task given to you or changed | tasks | person | | | 3 |
| `call`: a call ringing | dm | person | | (rung 3 decides) | 3 |
| `game_event`: something in your home worth knowing (a crop ready, air going bad) | none | game | | | 3 |

`room_line` is its own kind because the data file's shape calls for one: "every line" is an
ordinary line, which has no synced switch (the mentions switch is about you being named, not
about a room you chose to follow), needs the room at `all`, and no quiet-hours exception lets it
through. A line that is both a mention and a reply is a mention; one that is neither is a
`room_line`.

The same file lists the room levels a room's right-click offers (`levels`: every line, mentions
and replies, nothing), the mute lengths (`mute_for`: 1 hour, 8 hours, until I unmute) and the
sounds a person can pick (`sounds`, each an id from `data/sounds.toml`). Adding a length or a
sound is a row.

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
3. **The in-app list** (rung 2, part 1, built 2026-10-31): a bell in the header with the count of
   unseen notices, opening a panel of the last 100 (kind, who, where, when; never the text unless
   the person's setting shows text), each opening its conversation; the same list on the website.
   - **What goes in it:** whatever THE rule lets through. The rule answers a third thing beside
     the notice and the sound, `list`, true exactly when it answers `notify`: also with the
     window in front (the focused-window case of 4.9), and also with desktop notices and the
     sound both off, because the list is the one surface that never interrupts. (Rung 1's
     `nothing_on` reason went with this: there is always the list.)
   - **What a check stops is not listed, `on_screen` and `too_soon` included,** reasoned from the
     person's view: a line you are looking at needs no entry, and a burst of lines is one entry
     (the burst guard's five seconds holds the second line back, and the list goes further:
     a later notice for a conversation whose entry is not seen yet folds into that entry, its
     sender and time the latest, so an hour of chatting with the panel shut is one entry, not one
     every five seconds). A muted or quiet-hours notice is not listed either: a mute and quiet
     hours stop notices, and the list holds notices; the unread dots still show.
   - **Seen means the panel was opened;** while it stays open a new entry is seen as it arrives.
     The bell's number is the count of unseen entries, kept as entries come and go, so the bell
     reads it in constant time each frame.
   - **Where the bell sits:** on the desktop, in the header of every page (not the first-person
     world, whose banner is part 2, nor the title screen), anchored just left of the help button
     the way that button is, so a wrapping nav row never moves it (`gui/notice_panel.rs`,
     `widgets/notice_bell.rs`). On the website, in the chat page's own header, left of the
     sound menu (`web/chat/chat-notify.js`; `.notice-bell`, `.notice-count`, `.notice-panel` in
     `web/chat/layout.css`), not in the shared header of every page: the website's notices come
     only from an open chat tab, and the list is that tab's memory, so another page would show a
     bell that could never fill. The sound menu's button beside it showed a bell too; it is a
     speaker now, so the header has one bell and it means the notices.
   - **Kept in memory for the run (the tab, on the website), never written to disk.** What was
     said is kept only when the person's "What a notice shows" setting shows the message when it
     arrives; otherwise who, where and when. Rung 4 moves it to the encrypted local store beside
     the DMs.
   - **Opening an entry** opens its conversation: on the desktop it switches to the entry's
     server when that is another one this device keeps, then the DM, the room or the shared
     room's Commons view, on the Chat page; on the website, the DM or the room (a tab is one
     server).
4. **In the world** (rung 2, part 2): a short banner at the top of the screen while playing full
   screen, because a toast may sit behind a full-screen game; and the taskbar flash
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
   copy never counts as loaded: it may be old, or only the defaults (2026-10-30). So the server
   must always answer: the switches belong to its Chat feature, not to Push (2026-10-31,
   `relay/features.rs`). Under Push, a server whose owner turned Push off never answered, and
   neither app ever notified there (the seams pass found it). Push still decides whether that
   server SENDS a Web Push.
4. `kind_off`: the kind's synced switch is off.
5. `all_muted`: everything is muted (Settings > Notifications > Mute everything, and the tray's
   Mute from rung 2's part 2), DMs included. Then `person_muted` (rung 2): a person kind (a DM, a
   contact request) in a conversation muted until later or until unmuted; or `server_muted`,
   `room_muted`: a room kind, from a server or a room muted so.
   - A conversation's mute is keyed by its Chat id, `dm:<their key>`, on every server: it is the
     same person whichever server carried their message. It never mutes a mention of you in a
     room (the room is the conversation there), and a server's mute never mutes a person.
6. `room_level` (rung 2): a room kind in a room set below the kind's `level`: an ordinary line in
   a room at mentions (every room's level until the person changes it), or anything in a room set
   to nothing. A room's level is the room's mute key, `<server>|<room>`, on this device.
7. `quiet_hours`: now (the device's own clock) is inside the quiet hours, unless the kind's
   quiet-hours exception is on (rung 2): `friends` lets a DM from a friend through, `mentions` a
   mention or a reply. A window that ends before it starts runs past midnight; a window whose ends
   are equal is empty. An exception lifts quiet hours only: a mute still holds (mutes are checked
   first), and so do the checks after it.
8. `on_screen`: the window has focus AND that conversation is the one showing (the Chat page or the
   in-world chat panel, on the same server; a shared room's Commons view counts).
9. `too_soon`: a notice for the same conversation went out under 5 seconds ago (a burst of lines
   is one notice, and a flood of mentions cannot turn the computer into a siren).
10. Otherwise it notifies: the OS notice when desktop notices are on AND the window is not focused
    (a focused window gets the sound and the in-app list), the sound when sound is on, and the
    in-app list always (rung 2, section 3: `list`).

**A friend** (the friends exception) is someone you follow who follows you: the client-held store's
`DmStore::is_friend` (the website's `hosDmStore.isFriendPeer`), whose follows travel as sealed
control DMs with the friendship passes (the house pattern: client-held, statelessly verified). The
device decides it when the DM arrives, on the store of the server it came through (the desktop
keeps the active server's follows on GuiState and each parked server's on its link); the server
never knows it (section 7).

What is a mention: `@` and the person's name, any case, not followed by a letter, digit, `_` or
`-` (so `@Al` is not `@Alice`). The relay's push uses `@(\w+)`; the client's test is the stricter
of the two.

What reaches the decision at all: only LIVE messages (`chat`, `federated_chat` since rung 2,
`dm_new`), never history or a mailbox page, so opening the app never replays old notices. Blocked
people never reach it (they are screened before a message is stored), and a room the protected
setup hides never notifies. Where each kind is raised (rung 2):

- **A reply:** a live `chat` line whose `reply_to.from` (the replied-to line's author, which the
  sender's client sets and the relay passes on) is the person's own key, from someone else. The
  desktop's background links did not read `reply_to` at all before; they do now
  (`engine/bg_connections.rs`). That `from` is the sender's claim, so a sender can make any line
  "a reply to you", which is no more than they could do by writing your name.
- **A contact request:** a live `dm_new` (or, on the website, a direct device-to-device DM) that
  newly lists its sender under Requests: a contact request whose pass checks out, or a DM the
  person's own "who can reach me" setting refused (both are listed there the same way). The
  desktop compares the Requests list before and after the DM is taken in (`engine/mailbox.rs`
  `on_new`, `engine/bg_connections.rs` `bg_dm_new`); the website passes `live` through
  `ingestContactRequest` and `reachScreenDm` (`web/chat/chat-privacy.js`). One found in a mailbox
  page (`dm_batch`) is listed, never notified; the website notified those until rung 2.
- **A shared room's line (`federated_chat`):** a mention, or every line in a room set to it,
  read as a local line (`engine/notify.rs` `federated_line`, `web/chat/chat-ui.js`). The frame
  carries the author's key (`from_key`, since 2026-10-22), so a line of the person's own said on
  the other server is theirs and a blocked author's is dropped. A frame older than five minutes is
  not offered: a link that catches up re-sends its numbered lines live
  (`relay/handlers/federation_log.rs`), and those are history in all but the frame they ride.

## 5. The controls

| Control | Lives | Default | Desktop | Website | Rung |
|---|---|---|---|---|---|
| Direct messages / Mentions / Task updates | server (synced) | on | Settings > Notifications; the DM cog | Settings > Notifications; the DM cog | 1 |
| Quiet hours (from, to, by the hour) | server (synced) | off | Settings > Notifications | Settings > Notifications | 1 |
| Desktop notices on or off | this device | on | Settings | the browser's permission | 1 |
| What a notice shows: nothing / who it is from / who and the message | this device | who it is from | Settings | Settings | 1 |
| Sound on or off, and which sound | this device | on, "Message" | Settings, with a Play button | the chat's sound menu | 1 |
| Show a test notice | this device | | Settings | | 1 |
| Mute everything: 1 hour, 8 hours, until I unmute | this device | | Settings; the tray's right-click (rung 2, part 2) | Settings > Notifications > Muted | 1 desktop, 2 part 1 web (built), 2 part 2 tray |
| Mute a server: 1 hour, 8 hours, until I unmute | this device | | its cog, a saved server's right-click | the server's bell | 1 |
| Mute a room | this device | | the room's right-click (a parked server's rooms too, since rung 2) | the room's bell, or its right-click | 1 desktop, 2 part 1 web (built) |
| Muted list, with Unmute | this device | | Settings | Settings | 1 desktop, 2 part 1 web (built) |
| Mute a conversation (a person) | this device | | the DM's right-click | the DM's bell, or its right-click | 2 part 1 (built) |
| Per-room level: every line / mentions / nothing | this device | mentions | the room's right-click; the rooms set otherwise listed in Settings > Muted, with Back to mentions | same | 2 part 1 (built) |
| Quiet-hours exceptions: friends' DMs, mentions | server (synced) | off | Settings, under quiet hours | Settings, under quiet hours (the unread DnD section and the "Notification Sound" switch removed) | 2 part 1 (built) |
| The in-app list: the bell, its count and the panel | (memory only) | | every page's header | the chat page's header | 2 part 1 (built) |
| Per-kind sound | this device | | Settings | | 3 |
| Game events, per event | this device | per event in data | Settings | | 3 |
| Show HumanityOS in the notification area (the tray) | this device | on | Settings | n/a | 2 part 2 |
| Closing the window keeps HumanityOS running in the tray | this device | off (closing quits) | Settings | n/a | 2 part 2 |

On the website the DM rows and room rows had no menu at all, so rung 2 gives each the server row's
bell (shown on hover and focus, and always while muted, keyboard-reachable as a button with its
hint), opening the same menu a right-click on the row opens. The desktop marks a muted conversation
with the server row's muted bell at the end of its row.

### The tray (notification area) icon, rung 2 (part 2)

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
- **The in-app list keeps what was said only by the person's choice** (rung 2): an entry holds
  the message only when "What a notice shows" shows it at the time it arrives, and it is drawn
  with the setting as it is now, so choosing "Only that something arrived" hides who at once.
  It is never written to disk; closing the app (or the tab) empties it.
- **A contact request's notice says who, never what:** its words are a claimed name and a pass,
  and the name shown is the member list's for the signed key, never the one it claims.
- **A notice cannot be made into something else:** every value is escaped for the toast XML (a
  name cannot add a button or a link) and for Linux's markup; control characters go; the title and
  body are cut to 80 and 240 characters. macOS gets the words as arguments.
- **The notification centre holds little:** a Windows notice expires on restart and replaces the
  previous one for the same conversation (its tag is a hash of the conversation, not the name).
- **What is written to the computer:** on Windows, the one per-user registration above, the first
  time a notice is shown, and a copy of the app icon beside the person's data folder. Nothing else.

## 7. Where each setting lives, and why

- **Synced (the server's `notification_prefs` row)**: the three switches, quiet hours, and (rung 2)
  the two quiet-hours exceptions. The server's Web Push must honour them (it now does for the
  switches), and a person expects the same quiet hours on every device. The desktop asks for them
  at every sign-in and sends a change to EVERY server it is signed in to, so each one's pushes
  agree.
- **The exceptions' columns (rung 2, 2026-10-31):** `quiet_friends INTEGER NOT NULL DEFAULT 0` and
  `quiet_mentions INTEGER NOT NULL DEFAULT 0`, in the CREATE TABLE for a fresh database and added
  to a live one by guarded ALTERs right after it (`storage/mod.rs`; nothing in the schema batch
  names them, BUG-046; a pre-migration-shape test, `opens_a_pre_rung_2_database_and_migrates_it`).
  On the wire `update_notification_prefs` carries either as an optional boolean (absent: kept),
  and `notification_prefs_data` always carries both; a client reads a missing or malformed one as
  OFF, because an exception lets more through quiet hours.
- **What the server can and cannot know, for the exceptions.** The devices apply them (the
  desktop, and the website's open chat tab), through THE rule. For its own Web Push the server
  knows when a push is a mention, so the mentions exception is one it can apply. It can NEVER apply
  the friends exception to a DM: DMs are sealed-sender, so the server does not know who sent one,
  and the design keeps it that way rather than tell it (that would hand every server the
  person's social graph, which the 2026-08-24 follows removal took back). So the server's answer
  is written down in code, `NotifPrefs::push_comes_through_quiet_hours`: a mention with the
  mentions exception on, yes; a DM, never, whatever the friends exception says. Today the server
  applies no quiet hours to a push at all, because it does not know the person's clock (rung 4
  gives it that and reads this answer), so a push the switches allow comes at any hour; the
  website's Settings says so under quiet hours.
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
  desktop notices, what a notice shows, the sound, mutes, and (rung 2) each room's level. What a
  notice may show depends on the screen (a shared family computer, a phone), and a sound is the
  device's. Mutes are kept locally in rung 1; rung 4 tells each server about its own mutes only,
  so its pushes stop too (a server learns that it was muted, nothing about other servers).
  - The desktop: `notifications.mutes.people` (`dm:<key>` -> until) and
    `notifications.room_levels` (`<server>|<room>` -> `all` | `nothing`; mentions is no entry),
    both `#[serde(default)]`, so a `config.json` from before them loads with none.
  - The website: `hos_notify_mutes` gains `people`; `hos_notify_levels` holds the levels; and
    `hos_notify_names` keeps a muted conversation's name beside its mute, for the Settings page's
    Muted list (that page has no DM list of its own), removed with the mute.

## 8. Accessibility

- Never sound alone: every notice is also a toast (read aloud by Narrator, Orca and VoiceOver) or,
  focused, the unread dot; rung 2 adds the in-app list (built: the bell says its count to a screen
  reader, "Notices: 3 not opened yet") and the taskbar flash (part 2) for people who cannot hear
  it.
- Every control has its plain-language hint by key (`notifications.*_hint`) and is reachable by
  keyboard; the mute lengths are menu items, not a slider.
- No flashing: the web title alternates every 2 seconds, well under three flashes a second.
- The test notice lets a person check what they will get, without waiting for a message.

## 9. Budgets

- **Per frame, idle: nothing new but constant-time reads.** The decision runs only when a live
  message arrives, inside the socket pump that already handles it. The per-frame work is two
  `Option::take` checks (a test notice asked for, a sound preview asked for) and, since rung 2, the
  header's bell: it reads the list's unseen count (a counter kept as entries come and go, never a
  walk of the list) and paints a few shapes and at most three characters. The panel, while open,
  lays out its entries (at most 100, in a scroll area). The worker thread sleeps on its queue.
- **Per live room line (rung 2):** every live line of a listed room now becomes a candidate (a
  mention, a reply, or an ordinary line), because "every line" is the person's choice per room and
  the rule is the only place that knows it. A line in a room at the default level stops at
  `room_level`, the sixth check, after a few comparisons and one map lookup; the candidate's
  build is a handful of short strings. No new work per frame, and none per DM.
- **Per notice:** one pure decision, one string build, one queue send, and one entry in the list
  (a fold is a scan of at most 100 entries). The toast itself costs the worker a COM call
  (Windows) or a process start (Linux, macOS), never the frame.
- **Network: no new polling, no new routes.** One `get_notification_prefs` per sign-in (before:
  one when a popup first opened), one `update_notification_prefs` per change to each connected
  server. The relay adds one primary-key read of `notification_prefs` per push it would send.
  Rung 2 adds two booleans to the two existing messages and nothing else.

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

**Rung 2** is built in two parts. Part 2 needs the game booted to verify each piece, so it is a
separate increment.

*Part 1 (2026-10-31, built):*
- The in-app list (section 3, item 3): the bell, its count and the panel on both apps; the rule's
  `list` answer; entries opening their conversations.
- Mute a conversation (a DM's right-click on the desktop; a DM's bell or right-click on the
  website), with the new reason `person_muted` in both twins and in the shared cases.
- The website's Mute room (a room's bell or right-click), Mute everything and the Muted list with
  Unmute in Settings.
- The per-room level (every line, mentions and replies, nothing; mentions by default) from a
  room's right-click on both apps, a parked server's rooms included on the desktop, with the new
  kind `room_line` and the new reason `room_level`; the rooms set otherwise are listed in the
  Muted card with Back to mentions.
- The reply and contact-request kinds, and Commons (`federated_chat`) lines, raised from live
  messages only (section 4, last part).
- The quiet-hours exceptions (synced; section 7 for the columns and what the server can and
  cannot know), in Settings on both apps. On the website they replace the unread Do Not Disturb
  section (presence, status, its own quiet hours and exceptions, none of which anything read) and
  the "Notification Sound" switch, removed outright.
- On the way: the desktop's parked-server mention candidates now carry the server's normalised
  address, as the mutes name it (they carried the saved address as typed, so a server saved with
  a trailing slash never matched its own Mute server).

*Part 2 (still to come; each needs the game booted):* the tray icon ("The tray", after section 5),
with its Mute everything menu, the close-to-tray choice and the icon's muted and unread states;
clicking an operating-system notice opens its conversation (Windows: a COM activator or
`humanityos://` protocol activation; Linux: the default action; web: the browser notice's
`onclick` switches to it); the in-world banner and the taskbar flash.

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
  `en.json`, the settings round trip through `config.json`; since rung 2 the list (folds, unseen,
  the cap, text only by choice), conversation mutes and room levels, the bell's count, the room-
  line, federated, DM-friend and contact-request candidates (`engine/notify.rs`), and the
  exceptions' parse (`engine/ipc_parse.rs`, off unless the server says on).
- `node --test scripts/tests/notify-rules-web.test.js`: the website's twin on the same cases.
- `node --test scripts/tests/notify-list-web.test.js` (rung 2): the chat's glue to the rule (the
  list, the exceptions, the mutes and levels, the kinds, live-only contact requests and Commons
  lines) and Settings' exceptions and Muted list.
- `src/relay/storage/notification_prefs.rs` tests: a push only when its switch is on; the
  exceptions kept by a change that leaves them out, and never a DM through quiet hours on the
  server's side; a pre-rung-2 database migrating. `relay/handlers/msg_handlers_notif_prefs_tests.rs`:
  the exceptions on the wire.
- Snapshots: `just snapshot notification_settings` (now with the exceptions and a muted
  conversation and room level), `just snapshot chat_muted_server`, and since rung 2
  `just snapshot notification_bell` (the header's bell with a count) and
  `just snapshot notification_panel` (its panel open).
- A real notice cannot be snapshotted: Settings > Notifications > Show a test notification is how
  a person (and the operator) checks one, and a failure is logged to `run.log` with Windows'
  error code.
