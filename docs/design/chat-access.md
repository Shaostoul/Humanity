# Chat access levels: who can read and who can post in each channel

**Status: planned 2026-11-02, rung 1 built the same day, the operator's answers folded in the
same evening** (section 9 says what each rung adds and what is built). The background is BUG-223 in `docs/BUGS.md`; the rule that governs is CLAUDE.md's
2026-10-09 one: "We must protect the users", safe by default, people open up what they want.

## 1. Why, in the operator's words (2026-11-02)

> "Nonaccounts should be available to view. Unverified users with a chosen name should get to type
> reply and share 1 URL at a time. Verified get to share files/pictures/whatever. Is there anything
> I'm forgetting?"

> "Yeah, we should have the ability to set channels to private/public. I guess that's what
> unverified vs verified can read? Is there a nonaccount way to view the chat? I thought it
> redirects people to the pick a name page."

On the orchestrator's proposal (a read level and a post level per channel; new channels
members-only by default; admins open a channel to everyone): "Agreed".

### 1.1 His answers to the plan's open points (2026-11-02, relayed by the orchestrator)

- On the six additions (a written way someone becomes verified, an unverified person's links as
  plain text, a slow mode for new accounts, no @everyone below verified, a channel that is not
  open to everyone never federated, public channels not indexed): "All these make sense."
- On @everyone, narrower than proposed: "I feel like everyone should be reserved for mods and
  admins." So @everyone and @here are for moderators and admins only (section 5).
- On marking a channel's level: "we should come up with a little badge icon for this. Anyone,
  members, verified, mods/admins." (section 6.1).
- On a mark by people's names: "We should also come up with an icon that be next to people's names
  to indicate they have things posted for sale or download. Like how I have the 3D models. I guess
  there'd be no icon for someone that doesn't have an account." (rung 5, section 9).
- On becoming verified: "We need to figure out a way to do it... What are our options? It's
  technically a server for me and HumanityOS. I want to be able to backup our dev files and some of
  my personal files on there." (section 11).
- On search engines and AI crawlers: AI training crawlers are allowed ("Allow them."); the site's
  robots.txt is the orchestrator's to change. The chat's read-only view still asks search engines
  not to keep it (`noindex`), which is the approved "public channels not indexed".

## 2. What was true before rung 1 (checked in the code 2026-11-02)

- **Every channel was readable by anyone on the internet.** `GET /api/messages?channel=<name>`
  (`src/relay/api.rs` `get_messages`) asked for no key, and so did `GET /api/pins` and
  `GET /api/reactions`. Each answer carried every poster's full public key.
- **Every connected socket was sent every channel's lines**, whoever it belonged to, a socket
  that never chose a name included (the Tasks board signs in without one). The WebSocket search
  and a thread's replies were answered from any channel too.
- **No channel could be members only.** The one per-channel switch was `read_only` (only
  moderators and admins post), a posting rule, not a reading one.
- **The website sent a visitor with no key to the welcome step** (`web/chat/index.html`
  `preflightIdentity`, `/?welcome=1`): there was no way to look at the chat without making a key
  and choosing a name, which is what the operator's second question found.
- **Pictures and files were already verified-only to upload.** `src/relay/upload_rules.rs`
  `uploader_role` refuses a key with no role, and the Unverified role is seeded with
  `can_upload` off. But nothing stopped an unverified member posting the address of someone
  else's upload, or any number of links.
- **The relay fetches no link previews** (the chat arm ends at "Link previews placeholder
  (future feature)"; `handlers/utils.rs` `fetch_link_preview` has no caller). The website turns a
  picture address into a "click to load" box and any other address into a link; the desktop app
  draws pictures from this server inline and other addresses as links.
- **What "verified" means today.** A role, `verified`, that an admin gives by hand: the Verify
  button in a person's card on the desktop app (`gui/pages/chat/modals.rs`), Verify in the
  website's right-click menu (`/verify <name>`), or the role picker. Only admins can do it
  (`handlers/msg_handlers.rs` `mod_action_refusal`). There is no form to ask for it and no
  automatic path: it means "an admin of this server knows who this is". Verified members (and
  Donors, who share the Verified role's powers) get longer lines, uploads, voice and a faster send
  limit (`handlers/chat_rate.rs`).

## 3. The model

Every channel has two levels.

| Read level | Who can read |
|---|---|
| **Anyone** | everyone, no sign-up needed: a visitor to the website, any app, a search engine |
| **Members** | anyone with a chosen name on this server |
| **Verified** | verified members, moderators and admins of this server |

| Post level | Who can post |
|---|---|
| **Members** | anyone with a chosen name on this server |
| **Verified** | verified members, moderators and admins |
| **Moderators and admins** | only them (what `read_only` meant) |

Nobody can post where they cannot read: a channel that reads at Verified posts at Verified or
higher, whatever was picked (`chat_access::normalized_post`).

**Who counts as what** (`crates/humanity-protocol/src/chat_access.rs` `Rank`, worked out on the
server by `src/relay/chat_access.rs` `rank_of`):

- **Visitor**: no key, a key with no chosen name on this server, or a banned key.
- **Member**: a chosen name registered here (`registered_names`), role empty, `member`,
  `unverified`, or a role whose trust is below Verified's.
- **Verified**: role `verified` or `donor`, or a role the admins made whose trust level is 1 or more.
- **Moderator**: role `mod` (or the older spelling `moderator`).
- **Admin**: role `admin`, or the server's `owner`.
- Bots (`bot_*` keys) post through the bot secret, the operator's own route, and stay trusted as
  before.

## 4. Each channel's starting levels

- **A new channel reads and posts at Members** (safe by default). An admin opens it to Anyone.
- **#global (the old #general, renamed the same day, `docs/design/chat-channels.md`) reads at Anyone, posts at Members** ("Nonaccounts should be available to view").
- **#announcements reads at Anyone, posts at Moderators and admins.**
- **#local reads at Members, posts at Members** (it never leaves this server anyway).
- **Any other channel already on a server when the upgrade runs reads at Members**: it was
  readable by anyone, and nobody posting there was told so. Its post level carries over:
  Moderators and admins where it was read only, Members otherwise.

**The live servers on 2026-11-02** (a read-only query of each database's channels):

| Server | Channel | Before | Posts | After the upgrade |
|---|---|---|---:|---|
| united-humanity.us | #announcements | read only, federated | 2,449 | Anyone / Moderators and admins |
| united-humanity.us | #local | local only | 2 | Members / Members |
| united-humanity.us | #general | federated | 149 | Anyone / Members |
| public.guide | #announcements | read only, federated | 3 | Anyone / Moderators and admins |
| public.guide | #local | local only | 0 | Members / Members |
| public.guide | #general | federated | 19 | Anyone / Members |

The operator can change any of them in Server Settings > Channels (desktop) or the admin page's
Channels panel (website).

### 4.1 What a visitor sees of a channel opened to Anyone: only what was posted after it opened

Everyone who posts is now told who can read the channel, just above the box they type in (section 6).
Nobody who posted before was told: #general's 149 posts were made while the wizard said "Nobody
reads your messages, they're encrypted" (BUG-223), and a channel an admin opens later holds posts
made under "Only people on this server can read this". Opening a channel to Anyone must not turn
those into a billboard (the front door design's finding, `docs/design/front-door.md` section
4.2: "Posting in a room you walked into is not consent to a billboard").

So each channel keeps `anyone_since`: the time its read level last became Anyone. A request
without a member's signature is answered only with posts made after it, and with no poster's key
(a name is enough to follow a conversation; a key ties a person across servers). No signature either:
a signature is checked against a key, so anyone holding a list of keys could try each and find the
poster (found in the merge review, 2026-11-02, and held by the visitor test). Members, who
read through a signed request, see the whole history as before. The upgrade sets `anyone_since`
to the moment it runs for #global (then #general), so the website's read-only view of it starts empty and
fills as people post with the line in front of them. #announcements keeps its whole history
public (`anyone_since` 0; decided in the merge review, 2026-11-02): every one of its 2,449 lines on
united-humanity.us is the Deploy Bot's, so nobody's words are exposed, and the front page shows
those posts already. The upgrade runs as one transaction (the migration review): all of it or
none, so a crash halfway cannot leave #general and #announcements at Members.

**For the operator to overrule:** if he wants visitors to see the earlier posts too, that is a
one-line change of the upgrade's `anyone_since` (or, later, a Server Settings button "Show
earlier posts to visitors", rung 3). The recommendation is to leave it.

## 5. Each role's rights

| | Reads | Posts | Links | Pictures and files |
|---|---|---|---|---|
| **Visitor** | Anyone channels, posts since each opened (4.1), names only | nothing: read only | shown as plain text | not shown |
| **Member** (a chosen name, unverified) | Anyone and Members channels, all history | where the post level is Members; replies | **one link a message**, shown to everyone as plain text, no preview fetched, no picture pulled in | none: an upload is refused, and so is a line holding an upload's address or a file marker |
| **Verified** | all three levels | Members and Verified channels | as many as the line allows, shown as links | yes, through the upload checks and metadata stripping already built, within the role's allowances (section 11.3); link previews once the relay fetches them (rung 3) |
| **Moderators and admins** | everything | everywhere, as today | as Verified | as Verified |

**@everyone and @here are for moderators and admins only** (the operator: "I feel like everyone
should be reserved for mods and admins."). A line holding either from anyone below Moderator is
refused, `mass_mention`, like a line with two links from an unverified member; the relay never
turns either word into a notice for a person who happens to be named "everyone" or "here". There
is no notice to everyone on this server yet: a moderator's @everyone posts as words, and the day
such a notice is built it is for moderators and admins only.

Where the rules live: the relay refuses (the line never posts, and the sender's own copy is marked
"Not sent" through `chat_refused`, as for a line over the length limit); the apps draw an
unverified poster's links and picture addresses as plain words, so a line posted before rung 1
looks the same as one posted after.

## 6. What the screens say

- **The line just above the box you type in**, in both apps, says who can read the channel:
  "Anyone can read this, even without signing up." / "Only people on this server can read this." /
  "Only verified people on this server can read this." When you cannot post it says why ("Only
  moderators and admins can post here." / "Only verified people can post here."), and an
  unverified member is told "One link a message. Pictures and files need a verified account: an
  admin of this server verifies people.", its word verified opening the glossary card for
  Verified (`data/glossary.json`, `verified_member`), which says what it means and how a person
  becomes verified (section 11).
- **The wizard** (`web/chat/chat-onboarding.js`, the BUG-223 line) follows the model: direct
  messages are sealed; each channel says above the box you type in who can read it, and some
  channels can be read by anyone, even without signing up.
- **Server Settings > Channels** (desktop) replaces the Read-only tick with two pickers, Who can
  read and Who can post; the channel's own editor has the same two. The website's admin page has a
  Channels panel with the same pickers, on the registry action `channels.access` (so an AI can list
  and run it too).

### 6.1 The level badges (the operator: "Anyone, members, verified, mods/admins")

Each channel carries a small badge for its read level beside its name, in the channel list and on
the line above the box, in both apps: a globe for **Anyone**, a person for **Members**, a tick for
**Verified**; and the eye the sidebar already had marks a channel where only **moderators and
admins** post. Painted shapes on the desktop (`widgets::icons`, so no glyph the egui font draws as
a box), the matching icons on the website (`/shared/icons.js`), all in the theme's muted text
token. Each badge's hint, by key, says who can read and who can post: "Who can read: Anyone. Who
can post: Members."

## 7. The website's read-only view, for a visitor without a name

The answer to "Is there a nonaccount way to view the chat?": there was not, and now there is. A
visitor to `/chat` with no key on this browser goes to `/chat/read` instead of the welcome step:
the channels that read at Anyone (`GET /api/channels`), each channel's posts since it opened
(section 4.1) as plain text, names only, no composer, and one button, "Choose a name to join in",
which opens the welcome step and comes back to the chat. The page asks search engines not to keep
it (`noindex`): visitors may read, a search index does not get people's words. The desktop app
always has a key, so it has no such view: it signs in, and what it may read follows its member's
rank.

## 8. Enforcement (all of it on the server)

- **Storage.** `channels.read_level`, `channels.post_level` and `channels.anyone_since`, added by
  guarded ALTERs and named nowhere in the main schema batch (`tests/schema_alter_order_lint.rs`).
  The upgrade moves `read_only` into `post_level` and drops the `read_only` column (no
  compatibility shims before v1.0).
- **REST.** `GET /api/messages`, `/api/pins` and `/api/reactions` answer for an Anyone channel
  only (since `anyone_since`, no keys); any other channel is refused with 401 and the level it
  needs. A member reads with `POST` on the same routes, signed with their Dilithium3 key over
  `chat_read\n<what>\n<channel>` and the time (fresh for five minutes, used once), the house
  pattern for a signed request; a verified channel refuses an unverified member with 403.
  `GET /api/channels` lists the Anyone channels for the read-only view.
- **The socket.** Each line (chat, a federated line, an edit, a reaction, a pin, link previews)
  goes only to sockets whose rank may read its channel; the search and a thread's replies are
  filtered the same way; the pins and reactions sent at sign-in are #global's only when the
  socket may read it.
- **Posting.** After the slash commands (a command is not a post), the post level, then @everyone
  and @here below Moderator, then for a member below Verified at most one link and no upload
  address or file marker (`chat_access::post_refusal`).
- **Federation.** A channel whose read level is not Anyone does not travel to another server,
  whatever its Federated tick says: a peer would show its posts to the peer's own visitors.
- **Admin.** `channel_update` carries `read_level` and `post_level` (admins only, as before);
  the registry action `channels.access` sets both and `channels.list` lists them;
  `channels.readonly` (`/channel-readonly`) stays as a shortcut toggling the post level between
  Members and Moderators and admins.
- **Cost.** Each line sent to each socket asks the database for its channel's levels (one
  primary-key read on the read pool), and for a channel that is not Anyone the socket's role and
  name (two more). At 1,000 sockets and 10 lines a second that is 10,000 to 30,000 reads a second
  of a few microseconds each; rung 4 replaces them with an in-memory table rebuilt on every level
  change and obeying the Dev page's "turn off all reuse" switch (CLAUDE.md's no-caches rule).

## 9. The rungs

Each rung's gate is seen failing first.

### Rung 1 (built 2026-11-02)

The levels stored and enforced on the relay (REST reads, the socket, the search and threads, the
post level, @everyone and @here for moderators and admins, one link and no attachments below
Verified, the federation guard), the defaults of section 4 with `anyone_since`, the admin control
in both apps, the composer's line in both apps with the level badges of 6.1 (in the channel list
too), the wizard line, the plain-text drawing of an unverified poster's links in both apps, the
website's read-only view (not indexed), and the glossary card for Verified, which says how a person
becomes verified today.

- **A visitor** sees the Anyone channels' posts made since each was opened, names only.
- **A member** sees Anyone and Members channels whole; an unverified member posts one link a
  line and no files; a verified member sees Verified channels too.
- **Another federated server** receives only Anyone channels' new lines.

### Rung 2: the edges

- The channel list sent to a socket names only the channels it may read (today every socket is
  told every channel's name and description, never its posts).
- The Federation panel (both apps) locks the Federated tick of a channel that is not Anyone, with
  the reason, instead of the relay quietly not sending it.
- The REST thread and search routes, if one is added, follow the same signed read.
- The website weighs a role the admins made by its trust level, as the desktop app does (it
  reads the role list the relay sends: today the website counts any such role as a member when it
  draws links).

After it: a visitor and a member are told no name of a channel they cannot read.

### Rung 3: becoming verified, and the newcomer's first day (approved: "All these make sense.")

- **"Ask to be verified"** (section 11.2): the request from the line above the box, the queue in
  Server Settings, who verified whom kept for accountability, and moderators able to verify.
- **The longer slow mode for new accounts.** A slow mode exists today: an account that is not
  verified and is under 10 minutes old waits 5 seconds between lines (`NEW_ACCOUNT_WINDOW_SECS`,
  `NEW_ACCOUNT_DELAY_SECS` in relay.rs, `handlers/chat_rate.rs`), after a first 60-second wait
  (`handlers/chat_gate.rs`). The approved change (a day, one line a minute in channels) is here
  rather than in rung 1 because those two numbers also time direct messages (`handlers/dm_rate.rs`),
  so channels need numbers of their own, and the numbers decide how a newcomer's first day feels:
  they belong in Server Settings (GUI-first), not in a constant.
- **Link previews**, fetched by the relay for verified posters only (`fetch_link_preview` exists
  and is called by nothing).
- **"Show earlier posts to visitors"** for a channel an admin opened (section 4.1).

### Rung 4: the cost at scale

The in-memory level table of section 8, versioned by every change and behind the reuse switch,
measured before and after with a fixed number of sockets.

### Rung 5: a mark by the names of people with things for sale or to download

The operator: "We should also come up with an icon that be next to people's names to indicate they
have things posted for sale or download. Like how I have the 3D models. I guess there'd be no icon
for someone that doesn't have an account." A small mark beside a member's name, in the member list
and on their lines, when they have a live Market listing or a published Media or Files item on this
server; none for a visitor (who has no name to mark). It needs the relay to say, with the member
list, whether each key has either (a count from the listings and the published items, by key), and
a hint saying what the mark means and where to find their things. Not built in rung 1: it is not
trivial (a new field on the member list from two other features' tables, and a mark in both apps'
rows), and nothing in the access levels depends on it.

## 10. Not decided here

- Whether a Verified channel should also hide who posts in it from the member list (it does not:
  the member list is the server's, not a channel's).
- Groups and direct messages are not channels: they are sealed end to end and untouched by any of
  this.

## 11. Becoming verified

### 11.1 Today

Verified is a role an admin of this server gives by hand: the Verify button in a person's card on
the desktop app (`gui/pages/chat/modals.rs`), Verify in the website's right-click menu, or the role
picker. Only admins can do it today (`handlers/msg_handlers.rs` `mod_action_refusal`); it is kept
per server, and nothing records who gave it. The operator's way, in practice: he verifies people he
knows in person, or who message him on another social platform, after posting "join the humanityos
general chat, let me know the name you chose, and i'll verify ya!" The glossary card for Verified
says this in the app, reached from the line above the box.

### 11.2 The options, and the recommendation

1. **An admin or moderator vouches (today's way, the core).** Someone on the server who knows the
   person presses Verify. Cheap, human, and it means what it says: "someone here knows who this
   is". Its weakness is that it does not scale past the people the admins know, which is fine for
   a server that is, in the operator's words, "technically a server for me and HumanityOS".
2. **"Ask to be verified" (recommended, rung 3).** A request from the line above the box: the
   person says how the admin knows them ("we met at the Tuesday food drive", "I'm @someone on X").
   It lands in a queue in Server Settings (both apps), beside the reports queue, where an admin or
   moderator accepts or declines it. Nothing is automatic.
3. **A proof from another account, as evidence (recommended, inside option 2).** The request can
   carry a short code the app makes (`hum-verify-` and six letters) that the person puts in a
   profile or a post on another platform; the admin opens that page and checks the code is there.
   It tells the admin the request comes from whoever runs that account. It never verifies by
   itself: the admin still decides.
4. **Who verified whom, kept (recommended, rung 3).** Each verification records the verifier's key,
   the time, and the request it answered, readable by the server's admins (the admin log keeps no
   person's name today, so this is a record of its own, kept with the person's role and erased
   with their account).
5. **Moderators may verify too (recommended, rung 3).** The operator's "an admin or moderator
   vouches"; today only admins can. With the record of option 4, a moderator's verification is
   accountable.
6. **Not recommended:** automatic verification by account age, by activity, by payment, or by a
   third-party identity service. Each either says nothing about who someone is, or hands people's
   identities to a company, against the project's no-sign-up promise.

### 11.3 Verification grants sharing, not unlimited storage

The operator wants the server to keep room for his own backups ("I want to be able to backup our
dev files and some of my personal files on there"). Being verified lets a person share pictures and
files; it does not lift the allowances. What exists today, all set by an admin in Server Settings:

- **Per role** (Server Settings > Roles, `storage/roles.rs`): the largest single upload
  (`max_upload_mb`; Verified 25 MB by default), how many chat files a person keeps before the
  oldest goes (`max_uploads_kept`; Verified 20), how much a person may upload in a day
  (`max_upload_mb_per_day`; Verified 1,024 MB) and how many uploads an hour
  (`max_uploads_per_hour`; Verified 60), the largest Media file (`max_media_file_mb`, 0 = up to
  the person's Media share), the largest private file and how much of them a person may hold
  (`max_private_file_mb`, `max_private_held_mb`), and the call forwarder's day
  (`max_forwarded_mb_per_day`).
- **Per server** (Server Settings, `storage/server_settings.rs`): the disk cap for chat files
  (`max_total_upload_mb`), the Media page's budget (`media_budget_mb`, 2,048 MB by default) and
  each person's share of it (`media_per_person_mb`, 512 MB by default).

So a verified member's uploads already count against a per-person allowance the admin sets, and
the Media budget is apart from the disk the operator's own backups use (the backups are the
continuous-backup feature's, `backup_stream/`, not uploads). Nothing new is needed for this in
rung 1; rung 3's request queue shows the admin the person's current use beside the request.
