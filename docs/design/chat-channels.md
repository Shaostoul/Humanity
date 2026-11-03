# The chat's channels: #global, #feedback, and #local as a channel row

**Status: built 2026-11-02** (sections 2 to 5), in three steps, each its own commit. The chat
access levels it builds on are `docs/design/chat-access.md`.

## 1. Why, in the operator's words (2026-11-02)

> "What do you think about renaming #general chat to #global since were federating that channel.
> Then we can make a #feedback channel that people can talk in directly and I can say check that
> channel to you. Also, we presently have the #local channels hidden in that we have to click the
> server name to go to the local channel. We need to just make local visible like other channels
> are. I feel like they're too hard to find and add a layer of complexity that we don't need."

The last part reverses his own decision in field test 4 (v0.1135,
`docs/history/journal-archive-2026-08.md`), which made #local the server's name rather than a row
of its own. That decision's other half stays: a bridged room still lists once, under COMMONS.

## 2. #general becomes #global

**The channel every server has is #global** (`humanity_protocol::chat_access::MAIN_CHANNEL`; the
website's twin is `HosChatAccess.MAIN_CHANNEL` in `web/shared/chat-access.js`, both held to
`scripts/tests/fixtures/chat-access-v1.json`). It cannot be deleted or renamed, a line or a request
that names no channel is filed in it, and an app lands in it when the room it was in is gone.

**The live servers.** Their #general holds the operator's real history (on united-humanity.us,
read 2026-11-02: 149 lines, 5 of them from a linked server, 12 reactions, 2 pins). The relay renames it
at startup (`src/relay/storage/mod.rs`, in the chat access upgrade's one transaction, first, so the
steps after it name #global): only where there is a #general and no #global, so a server whose admin
made a #global of their own keeps both and nothing moves into theirs. What moves
(`storage/channels.rs` `move_channel_on`, shared with `/channel-edit`'s rename):

- the channel's row, with its read and post levels, its `anyone_since` stamp, its Federated tick
  and its place in the list;
- its lines, threads and lines from linked servers, AND each line's stored copy (`raw_json`), which
  names the channel too and is what the history is served from. Left as it was, the apps would file
  the whole history under a channel that no longer exists and show none of it. `/channel-edit`'s
  rename had the same gap and now shares the fix;
- the reactions and the pins;
- the search index, through its triggers; the triggers themselves named the old channel as the
  fallback for a line with none, so they are dropped and made again naming #global.

What a live database still says: the `messages.channel_id` and `reactions.channel` column defaults
keep the old word (SQLite cannot change a default in place, and rebuilding the messages table is not
worth it). Nothing reaches them: every insert names its channel, `store_message` (server notices)
included, which until now left it to the default.

Held by `src/relay/chat_access_tests.rs` `the_v0_1456_database_renames_general_to_global` (the
live servers' whole v0.1456 database, the chat access steps in the same open) and
`a_database_with_the_levels_renames_general_to_global_once` (a server that ran the chat access
code first), each seen red first.

**A room that is gone.** A desktop app or a browser whose last room was #general (the website keeps
it as `humanity_channel`) lands in #global once the server's channel list arrives, else in the first
room listed, as it does for any room an admin deleted (`chat.rs` `land_if_room_gone`, `app.js`
`landIfRoomGone`). There is no mapping from the old name anywhere: no name is carried over.

**The second live server, public.guide, follows releases, not main** (`docs/admin/SELF-HOSTING.md`,
`scripts/relay-follow-releases.sh`). Until the next release reaches it, it keeps #general, so the two
servers' main rooms do not link for that while: a federated line is stored only in a channel of the
same name on the receiving server. Nothing is built for the gap; it closes with the release.

**What kept the old name, on purpose.** Records stay as written: the history, CHANGELOG, the
announcements archive, the journal, dated findings (the chat access table of the live servers on
2026-11-02, `docs/design/front-door.md` section 4.1), field-test notes and BUGS.md entries, and
people's own words quoted. The moderation known-answer vector
(`crates/humanity-protocol/src/moderation.rs`, `scripts/mod-object-kat.mjs`) keeps its example
reason, "repeated harassment in #general after a warning": it is frozen bytes with a signature file,
not a name of the channel, and changing it would mean regenerating every hash for nothing. The
upgrade itself names #general: it is what it looks for.

## 3. #feedback

A channel on every server where people say what works, what does not and what they would like,
so feedback from every linked server gathers in one place and the people building HumanityOS can
read it without a key. **Anyone reads it; people with a name post** (Anyone / Members,
`docs/design/chat-access.md`), federated (only a channel anyone may read ever travels), no voice.
Its description, by key (`chat.channel_desc_feedback`): "Tell the people building HumanityOS what
works, what does not, and what you would like next. Anyone can read it, on every linked server."

A live server makes it at its next start (section 4: once per server, so one an admin deleted stays
deleted). Its anyone-since stamp is the moment it is made, so a visitor reads all of it.

**"Check that channel":** `just feedback [count]` (`scripts/feedback.js`) prints the newest lines of
#feedback on united-humanity.us (20 by default, at most 200), each with its time on this computer,
the poster's name (and the linked server a line came from) and the words, read through the public
`GET /api/messages` exactly as a visitor reads them. A server without a #feedback anyone can read
says so in words ("https://united-humanity.us has no #feedback that anyone can read yet. ..."),
which is what it says until this code is deployed there. `docs/ai/orchestrator.md` lists it.

## 4. The default channels are data

`data/server/default_channels.json` holds the channels every server makes for itself: id, name,
description (a word key in `data/i18n/en.json`), read level, post level, position, local only,
federated, voice, and when it is made. `src/relay/default_channels.rs` is the one place that makes
them (`make_missing`, at every start, from relay/mod.rs `run_relay`); before, #general was made in
`Storage::ensure_default_channel`, #announcements in a first-boot seed in relay/mod.rs (and again in
`scripts/seed-announcements.js`), #local in the guaranteed-local-room block beside it.

| Channel | Reads | Posts | Place | Federated | Voice | Made |
|---|---|---|---:|---|---|---|
| #announcements | Anyone | Moderators and admins | 2 | yes | no | once |
| #local | Members | Members | 3 | never (local only) | yes | always, while the guaranteed local room is on |
| #global | Anyone | Members | 10 | yes | yes | always |
| #feedback | Anyone | Members | 11 | yes | no | once |

- **Always**: made at every start when missing. #global is the channel every server has; #local is
  made while Server Settings > Federation keeps the guaranteed local room on and no local-only
  channel is left (recreating it is the guarantee, v0.1132); a #local that became an ordinary
  channel while the guarantee was off is the local room again when it comes back on.
- **Once**: made the first time a server starts with it in the set, then recorded in `server_state`
  (`default_channel_made:<id>`), so a channel an admin deleted stays deleted (BUG-036). The old
  single `default_channels_seeded` record and the one-time "voice off on #announcements" correction
  went with the code that wrote them.
- A channel already there keeps whatever its admin set (name, words, levels, place, voice); only one
  made here gets the file's. #global's position is no longer forced back to 10 at every start.
- The description is stored as the server's words (English, from the en.json built into the
  program): a description is the server's own, edited in Server Settings, and the server build has
  no word lookup of its own. The rename (section 2) replaces the words #general was made with
  ("General discussion") with #global's, and leaves words an admin wrote.

**Compiled in, with `include_str!`.** The relay is built on GitHub and only the program is swapped
in on the VPS; the VPS checkout does follow main, and a server that follows releases checks the repo
out at the release, but a server run from the program alone would have no `data/` beside it. The
set is part of what a release does, so it travels inside the program, as `data/admin/actions.json`
does. `scripts/seed-announcements.js` (the re-seed after a wipe, relay stopped) reads the same row
and words.

Held by `src/relay/default_channels_tests.rs`: the file is whole, a new server makes the set, a live
server gains #feedback and keeps what its admin set, each seen red first.

## 5. #local is a channel row; a server's name folds its rooms

**#local is an ordinary row under its server**, in the desktop app and on the website: its name,
its level badge (Members: the person), its unread mark, its right-click (desktop) or bell (website)
to mute it, like every other room (`src/gui/pages/chat/left_panel.rs`, both the current server's
rows and a saved server's; `web/chat/chat-ui.js` `renderServerList`, which already listed every
room). Until now the desktop filtered it out of every list and opened it from a click on the server's
name. COMMONS is unchanged: a bridged room still lists once, there, and not under each server.

**A click on a server's name folds or opens its rooms** (as the triangle beside it does), on every
server, current or not, and switches nothing; a room row is what opens a room, and a saved server's
room row switches to that server and lands in it. Why: the name had two jobs, a header that folds and
a hidden door to a room, and the door was the "layer of complexity that we don't need" the operator
named. Every other heading in the sidebar (Direct Messages, Groups, Servers, COMMONS) folds on a
click, so a server's name now does what a heading does, and a room is always opened the same way.
The alternative, "open the server's first channel", keeps the name a door (to a different room each
time an admin reorders the list) and would still need the triangle to fold. A switch from the server
strip or a server's cog lands in the room that server was left in (until now: #local).

**Servers other than the current one start folded.** Every server lists at least #local now, so at
dozens of servers (the operator is on 36 Discord servers) the list would grow by several rows a
server. A server the person never folded or opened is folded unless it is the one the app is on; what
they choose is kept per server (`GuiState::chat_server_folds`, saved as `server_folds` in the
config: renamed outright from `collapsed_servers`, whose saved folds are dropped). The website shows
one server's rooms (its own) and lists the others under Federation as one line each, with no rooms
under them, so it already follows the same rules; its server name folds the list
(`data-server-toggle`) and now carries the same hint by key as the desktop's
(`chat.server_row_hint`: "Shows or hides this server's channels.").

Held by `src/gui/pages/server_strip_tests.rs` `local_is_a_row_and_a_server_name_folds_its_rooms`
and `the_chat_rows_and_the_strip_switch_the_same_way` (desktop) and
`scripts/tests/chat-channels-web.test.js` (website), each seen red first; the picture is the
`chat_channels` snapshot (`src/gui/ui_snapshots.rs`, 2560 x 1440).

## 6. One room row for every server (BUG-242, 2026-11-03)

The operator, with two screenshots: "For some reason the formatting changes depending on the
active server. The active server everything is really tight. The inactive server channels are
loose." The desktop drew the active server's rooms in one block of `left_panel.rs` and every other
server's in another: the first as painted rows the theme's row height tall (15.9 points, shorter than
a line of the 16.4-point body text), the names 8 points from the panel's edge and pushed right by a
voice mark or an unread dot; the second as labels in a horizontal layout, indented 24 points, a line
of text (22 points) tall, with no ground of their own. Switching servers swapped which looked which.

**Now one drawer, `src/gui/pages/chat/channel_row.rs` `draw_room_row`, draws a room under any
server**, as `draw_server_row` has drawn every server's header since field tests 3 to 6. Its fixed
slots, left to right, every size a theme token:

- **the voice column**, `room_indent` wide (the small icon plus the row's padding and a small gap,
  25 points in the shipped theme): the mic, or the speaker while you are in the room's voice, for a
  room with voice; empty for one without. So every name starts at the same place, indented under its
  server's name, with or without voice. On the server the app is on the mic joins or leaves the
  voice; on another server it does what the rest of the row does, switches and lands in the room.
- **`# name`**, then its marks: the read level's badge, the eye (only moderators post), the
  node-graph (federated).
- **at fixed places from the right edge**: the unread dot, then the muted mark (Mute room, a bell
  with a slash, which rooms did not show before).

**Loose, not tight.** A row is a line of the body text tall, never less than the theme's row height:
what the other servers' rooms already were, the look the operator called loose, which reads better
and matches the tree under each server's name. The theme's row height alone is shorter than the text
it holds, which is what packed the active server's rooms together. The open room keeps its accent
wash and its bar (`row_accent_width`, a new token the DM and scratchpad rows use too).

The website lists only its own server's rooms (section 5), so it never had the two paths; its rooms
had the other half of the defect, a mic in front of a voice room's name only, and now keep the
voice column too (`web/chat/chat-ui.js` `renderServerList`, `.ch-voice-slot` in `sidebar.css`).

Held by `src/gui/pages/chat/channel_row_tests.rs`
`a_room_row_is_the_same_under_the_active_server_and_another` (the same left edge with and without
voice, height and steps, at 100% and 200% Font Size, whichever server is active) and the website's
`every room keeps the voice column, so a voice mark moves no name`
(`scripts/tests/chat-channels-web.test.js`), each seen red first; the pictures are the
`chat_rooms_home_active` and `chat_rooms_second_active` snapshots.

## 7. A link to a channel (2026-11-03)

The operator: "Can we make it so I can link people to specific channels like
https://united-humanity.us/chat#global or https://united-humanity.us/chat#fire-disciples", and
the same day: "I think we need to also fix the default channel to go to global".

**A channel's address is `https://<its server>/chat#<its id>`.** The rules are one file a side,
held to the same answers by `scripts/tests/fixtures/chat-links-v1.json`: `web/shared/chat-links.js`
(`HosChatLinks`) and `src/gui/pages/chat/channel_link.rs`. A link's word names the channel whose id
it is, else the one whose shown name it is, either in any case (an admin can rename what a channel
shows; public.guide shows id `general` as "global" until its next release). `#profile`, `#joined`
and anything holding `=` (`#dm=<key>`, a device link) are not channel words.

| Who follows the link | What happens |
|---|---|
| A person signed in on the website | The chat waits for the server's channel list, then opens the channel. The address keeps showing the channel open, and every switch replaces it (`history.replaceState`, `syncChatAddress` in `web/chat/app.js`, the one place navigation rung 5 turns into steps, `docs/design/navigation.md` 10.3). An edited address switches channel; a link in a message to this server's own chat switches in place. |
| A visitor with no key | `/chat` sends them to `/chat/read` with the hash kept. A channel anyone may read opens its tab. For any other, the read view asks `GET /api/messages`: 401 and its level says "#x is for people with a name on this server" (or verified people), with Choose a name to join in, which carries the channel through the welcome step to the chat (`sessionStorage` `hos_chat_link`, read once); 404 says there is no such channel. No tab opens and nothing is drawn in either case. |
| The desktop app, a link clicked anywhere in its window | A link to the server the app is on, or to a saved server, opens the channel on the Chat page (a saved server is switched to first, as its room row does; one being dialled keeps the link until its channel list comes, `GuiState::chat_link_pending`). A link to a server not saved here opens the browser, as before: the app does not dial a server because a message pointed at it. In the world the channel changes and the app does not take the player out of it. |

**A link naming no channel there says so** ("There is no channel #x on this server.") and opens no
other channel in its place: the website's chat and the desktop app stay in the channel they were
in, the read view opens no tab.

**A direct message is never in the address** (navigation.md 10.1: the address is seen, shared in
screenshots and synced by browsers): a conversation, a group and the scratch pad leave the address
at `/chat`. `#dm=<key>` stays the explicit conversation link it was (the Market's Message Seller);
it opens the conversation and the key leaves the address at once. The desktop app opens it too.

**What a visitor learns of a members' channel.** `GET /api/channels` lists only the channels anyone
may read, and `GET /api/server-info`, which listed every channel id to anyone, now lists the same
ones. A visitor's read of a channel that is not open to everyone answers 401 with its level and
nothing else of it (not its id, not its words); one that does not exist answers 404 (before,
both answered 401 "for members", the missing-channel default). So a visitor learns whether the
name in the link they already hold is a channel here, and its level, and no other channel's
name. Every signed-in socket is sent the whole channel list regardless of rank
(`build_channel_list`), so this tells a visitor nothing a fresh key would not; whether a members'
channel's name should be kept from keys without a name is an open question (section 8).

**Copy link** is the first item of a room's right-click (every server's rooms, each with its own
server's address: the desktop's old "Copy Channel Link" always built
`https://united-humanity.us/chat/<name>`, which opens nothing) and sits at the end of the channel's
header, in both apps. It copies the address and says what was copied; on a server with no public
address (localhost, 127.x, a home network: `is_public_host`) it copies the same address and says
the link opens only on this computer or its home network.

**#global by default.** With no link and nothing saved, every entry point lands on #global
(`MAIN_CHANNEL`), else the first room listed: the website chat's first open, the read view (which
opened `channels[0]`, #announcements, until this change), the desktop's first open, a switch to a
server being dialled, and a room that is gone. A switch on the website (the server strip) is a
link to the other server's `/chat` with no channel, so it lands there the same way.

Held by `scripts/tests/chat-links-web.test.js` (seven tests), `src/relay/chat_access_tests.rs`
`a_visitor_holding_a_link_is_told_whether_the_channel_is_for_members_or_not_there` and
`src/gui/pages/chat/channel_link_tests.rs`, each seen red first.

## 8. Open, for the operator

- **#feedback's levels.** Anyone reads it and people with a name post. Opening posting to people
  without a name would gather more but invites spam; keeping reading open is what lets
  `just feedback` read it without a key.
- **The order of the default rooms.** #announcements, #local, #global, #feedback: the live servers'
  order, with #feedback after #global. An admin reorders them in Server Settings.
- **public.guide's #general.** It becomes #global when the next release reaches it; until then the
  two servers' main rooms do not link (section 2).
- **Members' channel names for a key with no name (section 7).** Every signed-in socket is sent the
  whole channel list, names and levels, whatever its rank. The recommendation is to leave it: a
  name costs nothing to choose, so hiding the list from a nameless key keeps nothing from anyone
  who wants it, and a name in the sidebar with its Members badge is how a newcomer learns the room
  is there to join. A channel whose very name is private wants a level of its own (a list only its
  readers see), not this.
- **A channel link from an unverified member (section 7).** Below Verified a link is drawn as plain
  words (`docs/design/chat-access.md` section 5), so a member's link to one of this server's own
  channels is not clickable. The recommendation: draw a link to this server's own chat as a link
  from anyone, since it opens nothing outside the app and is how people point each other at a room.

## 9. Not done here

- The server cog's "Copy Invite Link" (chat/left_panel.rs) and Server Settings' invite link
  (server_settings.rs) still build `https://united-humanity.us/chat`, whichever server they are
  for: the same defect Copy link had, left for its own change.
