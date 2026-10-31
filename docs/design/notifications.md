# Notifications

Written 2026-10-30, when the operator answered the open question of 2026-07-01 ("a desktop
notification system, which 'Mute server' needs") with: "let's get it made."

This is the whole plan, built in rungs (CLAUDE.md, "Plan the whole feature, build it in rungs").
Rung 1 is built, and both parts of rung 2 (2026-10-31: the in-app list, the new mutes, levels,
kinds and exceptions; then the tray icon, closing into it, a click on a notice opening its
conversation, the in-world banner and the taskbar flash); section 11 says what each part and later
rung adds and what it needs. Read this before adding a kind of notification, a control, or a
platform.

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
   - **A click on a notice opens its conversation (rung 2, part 2, built 2026-10-31)**, through the
     in-app list's own path (`gui/notice_panel.rs` `open_entry`), and shows the window (from the
     tray too). Each notice carries a token that names nothing, this run's random id and a number
     (`notifications/clicks.rs`); the run keeps the last 64 numbers' conversations in memory. The
     operating system keeps a notice's payload (Windows in `wpndatabase.db`), so the token holds
     no key, address or name; a token from another run, or one past the kept 64, shows the window
     only. The test notice and the tray's own notice show the window.
     - Windows: **the notice activator**, a COM class the running app serves
       (`INotificationActivationCallback`, CLSID `{2A2508E1-B36C-451D-BC01-2114F1C9A751}`, registered
       with `CoRegisterClassObject` from the notice thread's multithreaded apartment), named by a
       `CustomActivator` value in the same per-user AppUserModelId key rung 1 writes: what the
       Windows App SDK writes for an unpackaged app. Windows calls it for a click in the corner
       AND in the notification centre (the in-process `Activated` event an unpackaged app can use
       instead fires only for the first). Why not `humanityos://` protocol activation: a protocol
       starts a SECOND copy of the exe for every click, which must find the running one and hand
       the click over, and its registration names an exe path that every worktree's and rig's
       build would overwrite. No `LocalServer32` is written beside it, so nothing names the exe and
       a click on a notice after the app has quit starts nothing (rung 5, with Reply on a notice).
     - Linux: `notify-send --action=default=Open --wait` when this `notify-send` has `--action`
       (libnotify 0.8, 2022, and later; read once from `--help`): it stays until the notice closes
       and prints `default` when clicked. At most three notices wait on a click at once (each holds
       one `notify-send` and one small thread); past that, and on an older libnotify, a notice
       shows without one. Rung 5's D-Bus moves this to the `ActionInvoked` signal.
     - macOS: `osascript` hands back no click. Rung 5's `UNUserNotificationCenter` brings one.
     - The website: the browser notice's `onclick` focuses the tab and opens the DM or the room
       (`chat-ui.js` `notifyNewMessage`, through `chat-notify.js` `openEntry`).
     - Only a person's launch answers clicks: an instance a script started (a rig) never serves
       the activator, so it never takes a click on the person's own notice (engine/tray.rs).
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
4. **In the world** (rung 2, part 2, built 2026-10-31): a short banner at the top of the screen
   while playing, because a toast may sit behind a full-screen game and a window in front gets no
   toast at all; and the taskbar flash (`winit::Window::request_user_attention`, Informational: the
   button lights until the window is in front) as a cue that needs no sound.
   - **Both go through THE rule:** only a notice it let through, and the cues follow from where
     the window is (`notifications/clicks.rs` `cues`, pure): the banner with the window in front
     and the HUD showing (the world, not a page, the showroom or the build editor); the flash
     with the window behind; neither while the window is hidden in the tray (no taskbar button,
     and the toast is the cue). The rule itself and its web twin are unchanged.
   - **The banner** (`widgets/notice_banner.rs`) is painted in the HUD's non-interactive layer
     under the compass, six seconds, fading over its last, in theme tokens. Its words are the
     notice's (`decide::words`): the title always, the message only when "What a notice shows"
     shows it (never its "Open HumanityOS to read it", which would be odd on HumanityOS's own
     screen). A newer notice replaces it.
   - **The flash** only for a person's launch: a rig's background window never lights the
     operator's taskbar.
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
| Show HumanityOS in the notification area (the tray) | this device | on | Settings | n/a | 2 part 2 (built) |
| Closing the window keeps HumanityOS running in the tray | this device | off (closing quits) | Settings | n/a | 2 part 2 (built) |

On the website the DM rows and room rows had no menu at all, so rung 2 gives each the server row's
bell (shown on hover and focus, and always while muted, keyboard-reachable as a button with its
hint), opening the same menu a right-click on the row opens. The desktop marks a muted conversation
with the server row's muted bell at the end of its row.

### The tray (notification area) icon, rung 2 (part 2)

The operator, 2026-10-30: "Can we add the ability for HOS to run in the taskbar? Then we could add
to the right click menu a button for mute that opens to the available options."

**Built 2026-10-31** (`src/engine/tray.rs`, `src/engine/tray_window.rs`, the pure look and menu in
`src/notifications/tray.rs`). What follows is what was built and why.

- **The crate: `tray-icon`** (tauri-apps, MIT OR Apache-2.0; 0.26.1 checked again 2026-10-31,
  `rust-version` 1.90, the tree builds with 1.99.1; menus through its sibling `muda` 0.21.2, same
  licence), default features off and `ksni` on (`Cargo.toml`). On Windows it uses `windows-sys`
  0.60/0.61, both already in `Cargo.lock`, so Windows builds only `tray-icon`, `muda` and
  `keyboard-types` new. On Linux the `ksni` feature (a StatusNotifierItem over D-Bus, pure Rust:
  `ksni` 0.3.7 and `zbus` 5) replaces its default `libappindicator`, which needs GTK 3 and the
  ayatana libraries installed on the Linux release runner. On macOS it uses `objc2`, already here
  through winit. `Cargo.lock` gained 37 packages (most of them `zbus` and its async stack, built on
  Linux only), every one MIT, Apache-2.0 or either, and `ksni` under the Unlicense (public
  domain). The icon is made in the first frame's socket pump, on the main thread after
  winit's loop has started (macOS's rule). Beside winit: on Windows the tray's own window
  procedure runs on the event-loop thread; its menu and click events go through handlers
  (`MenuEvent::set_event_handler`, `TrayIconEvent::set_event_handler`) that queue them and wake
  winit through its `EventLoopProxy` (`tray_window::wake`), so a click wakes a loop that is
  waiting with the window hidden. The frame takes them with one atomic load when nothing
  happened. The pointer passing over the icon is not queued. No `with_guid`: Windows binds an
  icon's GUID to the exe's path unless the exe is signed, and the person's exe moves with every
  version, which makes the icon fail to appear.
- **Who gets an icon:** a person's launch (a double-click, `just play`). An instance a script
  started (`launch_focus::launch_in_background`: a rig, a boot check) gets none, so agents' boots
  never pile icons into the operator's tray (a force-closed process leaves its icon behind until
  the pointer passes over it), and it never answers a click on the person's own notices. A rig
  that tests the tray sets `HUMANITY_TRAY=1` and closes gracefully, so the icon goes with it.
- **A left click on the icon** opens the window (a double-click too); the right click is the menu.
- **The menu**, every word by key: Open HumanityOS; Mute everything, opening to the same choices as
  Mute server (For 1 hour, For 8 hours, Until I unmute; and Unmute, with "Muted until 14:00" as a
  line that cannot be clicked, while muted); Quit HumanityOS.
- **What the tray's Mute mutes: everything** (the whole app's notices, DMs included), as distinct
  from one server's Mute. It is the same state as Settings > Notifications > Mute everything, which
  rung 1 builds, so the tray adds a second place to reach it and nothing else.
- **Closing the window: a choice in Settings, off by default, so closing still quits** unless the
  person turns it on, and it does nothing without the icon showing (there would be no way back to
  the window): `tray_window::keeps_running`. When on, closing hides the window instead:
  - the world is left first, the way quitting leaves it: saved (`quit::save_game`, the game's
    half of `save_before_exit`; the run's temporary files stay, it is not an exit), out of a
    server's shared world (`game_leave`, so nobody sees a frozen figure; entering the world again
    joins afresh), and the main menu in front. The game has no "Leave" of its own to reuse (the
    Escape key toggles between the world and the last page), so this is it;
  - no frame is drawn: `about_to_wait` stops asking for redraws and the RedrawRequested arm returns
    at once, so the GPU does nothing and no world simulates;
  - four times a second a tick runs the frame's own network pump (`engine/frame_net.rs`, moved out
    of the frame loop for this: dialling, reading every message, reconnecting, WebRTC, chat
    history, every saved server's link), takes the tray's and the notices' clicks, and plays a
    notice's sound, so notices still arrive;
  - the first time in each run, an operating-system notice says HumanityOS is still running beside
    the clock (with desktop notices on): once per run, so a person who chose this months ago is
    told where it went, and never at every close.
  Quit from the tray runs the same save as closing does (`engine/quit.rs` `save_before_exit`, in
  `about_to_wait` while hidden, and through the frame's quit request while shown).
- **Closed by another program, it quits whatever the setting** (Windows): a WM_CLOSE POSTED to the
  app (taskkill without `/F`, an installer) is told apart by winit's message hook
  (`tray_window::note_message`), because a person's close button and Alt+F4 SEND theirs straight
  to the window procedure. Agents close the operator's game that way so it saves (CLAUDE.md, "The
  operator's machine"); hiding would have kept it running until it was forced, unsaved. While the
  window is hidden, `taskkill` finds only winit's own message window to post to (it skips hidden
  windows), so a close posted to ANY of the app's windows counts, and one posted to a window
  other than the app's own is kept from its window procedure, which would destroy it: the second
  rig run of 2026-10-31 found the hidden game outliving `taskkill` until it was forced 15 s later.
- **The icon shows state:** a dot in the unread colour (`danger`, the bell's count) at its
  top-right for notices not opened yet (the in-app list's unseen count), and the picture dimmed
  and grey with a bell and a slash at its bottom-right while everything is muted, both drawn by
  `notifications::tray::picture` (pure, tested) in theme tokens over the app icon at 32 pixels,
  only when the look changes. The tooltip says the same in words: "HumanityOS", "Muted until
  14:00", "Notices: 3 not opened yet" (the bell's own words), cut to Windows' 127 units. The same
  picture goes to Linux's tray. Not built, recorded for later: on macOS a short title beside the
  menu-bar icon (the count); and Windows' overlay badge on the taskbar button
  (`ITaskbarList3::SetOverlayIcon`), which needs `HICON` from the windows crate's
  `Win32_UI_WindowsAndMessaging`, its largest module and one the tree does not build yet, for what
  the tray icon already shows.
- **Linux desktops with no tray** (GNOME without its AppIndicator extension) show no icon, which is
  the desktop's choice; Settings says so on Linux, and to keep closing-into-the-tray off there.
  The StatusNotifierItem cannot tell whether anything shows it.
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
  previous one for the same conversation (its tag is a hash of the server and the conversation,
  not the name), and its click token names nothing (section 3, item 1).
- **What is written to the computer:** on Windows, the one per-user registration above, the first
  time a notice is shown (`HKCU\Software\Classes\AppUserModelId\UnitedHumanity.HumanityOS`:
  `DisplayName`, `IconUri`, and since rung 2 part 2, for a person's launch, `CustomActivator`, the
  notice activator's class id), and a copy of the app icon beside the person's data folder.
  Nothing else: no protocol registration, no `LocalServer32`, no shortcut. The tray icon itself
  writes nothing (Windows keeps its own record of icons it has seen, under its notification-area
  settings, as it does for every program with one).

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
  keyboard; the mute lengths are menu items, not a slider. The tray's menu is the operating
  system's own (keyboard and screen readers as every tray menu), its tooltip says its state in
  words, and the taskbar flash (built) is the cue for a person who cannot hear the sound.
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
- **The tray (rung 2, part 2), per frame:** two atomic loads (its events, the notices' clicks), one
  look compare (the everything mute and the unseen count, a few integers), and the dev request
  check every IPC handler makes. The picture, tooltip and menu are rebuilt only when the look or
  the language changes (a 32-pixel picture, under a millisecond). The app icon is read once, when
  the icon is made. In the world the banner adds one card's layout while it shows.
- **Hidden in the tray:** no frame, no GPU work, no world; four ticks a second of the network pump
  the frame already runs, and nothing else.
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

*Part 2 (2026-10-31, built):*
- The tray icon ("The tray", after section 5), with its Mute everything menu (Settings' own mute),
  Open and Quit, the icon's muted and unread looks and its tooltip; none for a script's instance.
- The close-to-tray choice (off by default): the world left and saved, no frame while hidden, the
  sockets read four times a second through the frame's own network pump (moved out of `lib.rs`
  into `engine/frame_net.rs` for it), the once-a-run notice, and a close from another program
  always quitting.
- A click on an operating-system notice opens its conversation: Windows through the notice
  activator (a COM class, section 3 item 1, and why not `humanityos://`), Linux through
  `notify-send`'s default action, the website through the browser notice's `onclick`.
- The in-world banner and the taskbar flash, through the rule's answer and `clicks::cues`.
- Two leftovers of part 1: the bell's panel left open on entering the world was still open on
  return (the page check ran only while the header was drawn; `notice_panel::keep_to_header` runs
  every frame now), and the burst guard's time was keyed by the conversation without its server
  (two servers' "general" rooms shared one five seconds; `decide::burst_key`, which the Windows
  notice's tag now hashes too).
- For a rig, the dev IPC's `debug/tray_request.json` does what each click does (close, a menu
  line, the icon, a click on a notice by its token, a made-up notice) and reports the tray and
  the window in `debug/tray_done.json` (engine/tray.rs).

*Still later, recorded:* Windows' taskbar overlay badge (section 5, "The tray"), macOS's menu-bar
count, a click on a notice after the app has quit starting it (rung 5, with Reply), and Linux's
click through D-Bus (rung 5).

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
  `just snapshot notification_panel` (its panel open); since part 2 `just snapshot notice_banner`
  (the in-world banner under the compass), and `notification_settings` shows the two tray switches.
- A real notice cannot be snapshotted: Settings > Notifications > Show a test notification is how
  a person (and the operator) checks one, and a failure is logged to `run.log` with Windows'
  error code.
- Rung 2, part 2: the tray's menu, tooltip and picture, a notice's click token and what it opens,
  the cues, the launch attribute's escaping and Linux's action arguments
  (`notifications/tests.rs`); `tray_window::keeps_running`; the burst guard across servers
  (`engine/notify.rs`); the panel shut off the header (`gui/notice_panel.rs`); every way out of the
  frame loop saving, the tray's Quit included (`engine/quit.rs`); the website's notice click
  (`notify-list-web.test.js`, test 8). A real tray, a real click and a real flash cannot be
  driven by a test: a rig boots the game with `HUMANITY_TRAY=1` and drives
  `debug/tray_request.json`, reading `run.log`'s `tray:`, `window:` and `notifications:` lines;
  a person checks them by hand (Settings > Notifications, then close the window, right-click
  the icon beside the clock, click a notice).
- What the rigs showed on 2026-10-31 (four boots of the release exe with `HUMANITY_TRAY=1`, run.log
  and `tray_done.json` read): the icon made and the activator served; a made-up mention through
  THE rule stopped at `not_loaded` (no server in the sandbox), and past it with `force` it made a
  real toast, the dot and "Notices: 1 not opened yet"; the per-user registration gained
  `CustomActivator`; another process calling the activator by its CLSID with that notice's token
  (as Windows does on a click, with no `LocalServer32` registered) opened Chat on the
  conversation; closing from the world saved it, put the main menu in front, hid the window (no
  frame for 15 s, no CPU used in 5 s, the reconnect countdown still running), and showed the
  "still running" notice once; Mute for 1 hour from the menu wrote `everything` in `config.json`
  and turned the menu into "Muted until 11:09" and Unmute; the icon brought the window back
  (frames again); Quit while hidden saved and shut down cleanly; `taskkill` without `/F`
  (`game_close`) closed it gracefully both hidden and shown with closing-to-tray on. The two
  findings (a background launch's window stayed on screen; `taskkill` did not reach a hidden
  window) were fixed and re-run. A script's launch (the runtime gate) logs "tray: none for this
  instance". The banner is `just snapshot notice_banner`.
