# Tasks: mine, shared with people I choose, and a server's board

**Status: planned 2026-11-02; rung 1 (Mine, both apps) built the same day.** Fixes BUG-217 (a task
made in the desktop app was never saved and vanished on reconnect, because the server's task list
replaced the local one). Code: `src/gui/mine_tasks.rs` (the records, the merge, the notes, the
encrypted file), `src/engine/mine_tasks.rs` (making, sending and taking notes on the desktop),
`src/gui/pages/tasks.rs` (the page); `web/shared/mine-tasks.js` (the same rules on the website)
and `web/pages/tasks-app.js` (the page). Algorithms: `docs/reference/cryptography.md` ("Tasks of
my own").

## Why, in the operator's words

Asked on 2026-11-02 whether members may add board tasks (BUG-221), the operator answered: "You
mean tasks, like in the task list page? Do we have two types of tasks, like private user tasks and
public tasks? Should we have more? People should be able to add tasks to their own account that
share between their devices but, they shouldn't have to share it with the server. I'm not really
sure how we should implement this.  Let's go with the safest option for users and the server."

The orchestrator proposed three kinds (below), and the operator answered "I like this. Let's do
it." The rule for every choice in this plan: **when in doubt, the server learns less.**

A person's own tasks are Real-side data (`docs/design/two-realities.md`): their real to-dos, kept
on their own devices, with the relay at most an end-to-end encrypted carrier and backup it cannot
read.

## 1. The three kinds at a glance

| | Mine | Shared with people I choose | A server's board |
|---|---|---|---|
| Who can read it | Only me, on my own devices | Me and the people I put on the list | Everyone who can open that server's Tasks page |
| Who can change it | Me | Everyone on the list | Moderators and admins (BUG-221); verified people, donors, moderators and admins comment |
| Where it is kept | Each of my devices, encrypted with a key from my recovery phrase | Each member's devices, encrypted the same way | The server's database (`project_tasks`, `task_comments`) |
| How it travels | Sealed notes to myself in my own mailbox (section 3) | Sealed notes to each member, through their "who can reach me" (section 5) | The server's task messages (`task_create`, `task_update`, ...) |
| What a server sees | Sealed mail in my own mailbox: how many, roughly how big (padded), which day. Not that it is a task, not a word of it | Sealed mail in each member's mailbox, and the depositing socket's key in memory for its rate limit. Not the list, its members or a word of it | All of it, by design: it is public to its members |
| Reminders | Only my own devices, worked out on the device | Only the members' own devices | The relay's `task_notice` and Web Push behind the Tasks switch (notifications rung 3) |
| Built | Rung 1 (this plan) | Rung 4 | Before this plan; its rules stay |

## 2. Mine: the record

One task, as both apps keep it and as a note carries it (JSON, field names exact, written in this
order with empty fields left out, so both apps write the same bytes;
`scripts/tests/fixtures/task-note-v1.json` holds them to it):

```
{"id":"<32 lowercase hex>","title":"...","description":"...","status":"todo|in_progress|done",
 "priority":"low|medium|high|critical","labels":["..."],"chain":"<chain id>"|null,
 "created_at":<ms>,"updated_at":<ms>}
```

A deleted task is a tombstone: `{"id":"...","updated_at":<ms>,"deleted":true}`, no words kept.

- `id`: 128 random bits from the operating system (desktop `getrandom`, website
  `crypto.getRandomValues`). Never a counter: two devices offline at once must never pick the
  same id.
- `title` up to 200 characters, `description` up to 3,000, at most 12 `labels` of up to 32
  characters each; control characters other than line breaks and tabs are taken out. A record
  that breaks these is clamped on the way in (from a note or a form), so a note always fits
  (section 3).
- `chain`: the learn-by-doing chain (`data/onboarding/quests.json`) Add to my tasks made it from,
  so its suggested card stays away and the task shows the chain's steps. The steps' ticks stay
  where they always were (desktop `config.json`, website `humanity_quest_progress`).
- `updated_at`: when it last changed, in milliseconds. A change on this device sets it to the
  larger of now and the previous value plus one, so an edit always wins over what it edited even
  on a device whose clock is behind.

**The merge (both apps, the same rule):** per id, the record with the larger `updated_at` wins.
At the same `updated_at` a deletion wins, then the later status (done over in progress over to
do), then the higher priority; otherwise what the device holds stays. The rule gives the same
answer whatever order notes arrive in, and applying a note twice changes nothing, so a relay that
delivers a note again, or my own note coming back to me, is harmless. Two devices editing the same
task in the same millisecond with different words is the one case it does not settle (each keeps
its own); it needs two devices and one millisecond, and is left at that.

Tombstones are kept (about 60 bytes each): a dropped one would let an old note still in a mailbox
bring the task back. Compacting them needs a horizon older than every server's mailbox lifetime,
which rung 2 adds.

## 3. Mine: how it travels between my devices, and why this way

**The mechanism already exists, and both apps use it:** a sealed note to myself. An ordinary
signed, sealed v2 DM whose inner payload is from me to me (Dilithium3 signature over the DM
preimage), sealed to my own Kyber768 key, padded to the DM size buckets, deposited with `dm_put`
in my own mailbox on every server the device is connected to. My other devices read it from the
mailbox (`dm_fetch`, or live as `dm_new`) and open it with the same key, which every device of
mine derives from the recovery phrase. The block list (`[[hum:block:v1]]`), the choice for each
friend (`[[hum:choice:v1]]`) and the always-allow list (`[[hum:quiet-allow:v1]]`) already travel
this way, so this plan adds one more marker and no new server code:

```
[[hum:task:v1]]{"tasks":[<record>, <record>, ...]}
```

A note carries one or more records, as many as fit in 19,000 bytes of text, so with the inner
payload's two keys and signature (about 12.3 KB) it stays inside the largest padding bucket
(32,768 bytes, `DM_PAD_BUCKETS`) and far below the relay's envelope cap (131,072). Self-copies
cost nothing against the DM send limiter (they are the sender's own mailbox; the relay's TTL
bounds them).

- **Only my own notes count.** A task note is applied only when its signature checks and it is
  from me AND to me. Anyone can seal mail to my public key, but only my recovery phrase signs as
  me. A task note from anyone else is dropped unread; either way a task note is never shown as a
  message, in either app's chat.
- **Kept before it is sent:** every change is saved on the device first with its id marked
  unsent, and the mark is cleared once the note went, so a crash or a closed page loses nothing.
  Made while no server is connected, it goes out on the next connection (desktop: on the member
  list, beside the block list's notes; website: when the Tasks page's socket signs in). Two
  website pages open at once each keep what they sent; one change made in each while offline may
  wait for its next change to go out (rung 2's book closes that too).
- **The website's Tasks page reads the mailbox itself.** It is a page of its own with its own
  signed-in socket (the board's), so it asks for my mailbox from where it last read (a read
  position per server, kept in its encrypted record) and takes only task notes; the chat page
  drops task notes it meets. Every device receives every mailbox page, so a page counts only when
  it carries this page's own fetch ref (the chat's rule, 10o O1).

**Why not the website's direct device-to-device data sync** (`web/chat/chat-p2p.js`, the calendar,
homes and notes): it exists on the website only, needs both devices open at once and a click, and
copies localStorage in the clear. The notes to myself work in both apps, with one device off, and
sealed.

**Why not the vault backup** (`/api/vault/sync`, `vault_blobs`): it is one blob per key for the
Settings vault, replaced whole; using it for tasks would need a second slot on the server. The
two-realities design's personal backup slot (encrypted, quota-bounded) is the right home for a
long-term copy, rung 6.

**What rung 1 does not cover yet (rung 2):** a mailbox keeps mail for its server's TTL (30 days
by default). A device that was away longer, or a new device, sees only the notes of the last 30
days, and a server my device first meets after a change never got that change's note. Rung 2's
book closes both.

## 4. Mine: where it is kept on each device

- **Desktop:** one file per identity, `%APPDATA%/HumanityOS/tasks/<24 hex of BLAKE3("tasks-mine\n"
  + identity)>.mine` (the name a hash, so the folder names nobody), its body AES-256-GCM under
  BLAKE3 `derive_key("hum/tasks-mine/v1", seed)`, written crash-safely
  (`humanity_core::atomic_write::write_atomic`: a crash leaves the whole old file or the whole new
  one). Read the first time it is needed after the identity is unlocked. With no identity
  unlocked, the Mine tab says the tasks open with the identity; the board still shows.
- **Website:** the chat's own encrypted store, IndexedDB `humanity_dms`, its `meta` object store,
  one record whose scope is SHA-256 of `<identity>\ntasks-mine` (so neither the identity nor the
  word "tasks" is in the clear), the body AES-256-GCM under HKDF-SHA-256 of the seed with info
  `hum/tasks-mine-web/v1` (the DM store's own scheme, its own key). The chat page never opens it.
  A write reads the record again and merges first, so two open Tasks pages lose nothing.
  **Limit:** the page needs the identity Chat saved in this browser (`humanity_key_backup`); a
  passphrase-protected identity cannot be opened from a page other than Chat yet (the board's
  writes and Settings' vault sync have the same limit). The page says so.

**Saved tasks that do not open are never written over** (both apps, 2026-11-02, from the review of
rung 1, which found that the first build started empty and overwrote them). Only a file or record
that is not there is an ordinary empty start. One that is there but does not open (damaged, or not
this seed's) is kept aside before anything is written: the desktop renames it to
`<name>.unreadable-<unix seconds>` beside it (never a delete), the website copies it as it is to
its own key in the same store (`<scope>:unreadable-<unix seconds>`); the list starts empty from
there. A read the system refused (a lock, a permission), or a keep-aside it refused, keeps that
session from saving over the file at all. Changes still go to my other devices as notes either
way, and the Tasks page says plainly what happened and that tasks from my other devices come back
as their notes arrive (`tasks.mine_kept_aside`, `tasks.mine_unreadable` and their `_web` twins,
each with a hint). Nothing yet opens a kept-aside copy for the person; a repair step is rung 3.

No migration: nothing was ever saved before (BUG-217), and CLAUDE.md's rule holds until 1.0.

## 5. Shared with people I choose (rung 4, designed here, built later)

A list ("Home", "Garden club") with a random 128-bit id, its members held only by the members'
own apps. Each change is a signed record (the Mine record plus the list id and the author's key),
sealed to each member's Kyber768 key **taken from their key card** (`docs/design/key-cards.md`;
never from a server's list) and deposited in each member's mailbox, through their "who can reach
me" with the friendship pass they gave me (so only people who let me message them can be put on
a list). The member list itself travels the same way, signed by whoever changed it; the merge is
Mine's, per record. Removing someone stops sending to them; what they already received stays
with them (said plainly in the UI, as for a file shared in a DM).

Why not the P2P groups' signed objects (`docs/design/p2p-groups.md`): their roster objects are
kept on the relay, so the server would learn who is on the list. The sealed fan-out shows it only
mail deposits, which it sees for DMs anyway. The rung's first step is to measure the fan-out's
cost for the largest list we want (say 20 members) and confirm the choice.

## 6. A server's board (built; its rules stay)

The relay's `project_tasks` and `task_comments`, public to the server's members, unchanged:
moderators and admins create, change and assign (`may_edit_tasks`); verified people, donors,
moderators and admins comment (`may_comment_on_tasks`, BUG-221); a task given to someone, or
changed while it is theirs, sends them a `task_notice` (notifications rung 3).

**Shown beside Mine, never merged into it.** The server's list (`task_list`,
`task_list_response`) fills the board's own list and nothing else. On the desktop the board is
read-only in rung 1: the desktop never sent a board change (its edits stayed local and were
replaced on the next reconnect, BUG-217), and board editing for moderators and admins on the
desktop is rung 5.

## 7. The Tasks page, in both apps

- **A switch at the top: "My tasks" and "This server's board"** (rung 4 adds "Shared"), each with
  one line under it saying who can see what is shown:
  - My tasks: "Only you can see these, on your own devices. They travel between your devices
    sealed, and no server can read them."
  - Board: "Everyone on this server can see these. Moderators and admins add and change them."
- My tasks is shown first. The website remembers the last choice in this browser.
- **New task** makes a task of the kind on screen: on My tasks it is kept at once, on the device,
  and sent sealed to my other devices; on the website's board the New Task form goes to the
  server as before (the relay checks the role).
- **The suggested cards** (the learn-by-doing chains) show on My tasks only. **Add to my tasks**
  makes a Mine task in one click in both apps (the website's used to open the server's New Task
  form, because the board was the server's). Nothing about it reaches a server in readable form.
- On My tasks the website shows three columns (To do, In progress, Done); the board keeps its
  four (with Testing, whose test votes belong to the board). The scope tabs, projects, comments,
  assignee and test votes are the board's and do not show on My tasks.
- A Mine task can be moved between columns, given a priority and deleted (rung 1); editing its
  words is rung 3.

## 8. Reminders and notifications

- **Mine:** only my own devices remind me, from what they hold (due dates are rung 3). Never a Web
  Push: a push goes through Google, Mozilla or Apple and would need the server to know when, and
  so roughly what, I planned. The desktop reminds from the tray; the website while a page of it is
  open.
- **Shared:** each member's own devices, the same way. A change by someone else may ring the
  members' own devices when its note arrives (rung 4), by the notification rules
  (`docs/design/notifications.md`), never through the server.
- **Board:** unchanged (the relay's `task_notice`, the `tasks` switch, quiet hours).

## 9. Threat model

| Who | What they can learn or do to Mine |
|---|---|
| A server (or whoever copies its database) | That my mailbox holds N sealed notes, each padded to a bucket, by arrival day. Not that they are tasks. It can drop or delay notes (my other device catches up later, rung 2), or deliver one again (the merge makes that harmless). It cannot forge one: only my recovery phrase signs as me. |
| Another person | Nothing. A note they seal to my key is dropped unread (it is not signed by me). |
| Someone with my device's files, not my seed | Nothing: the file (desktop) or the record (website) is AES-256-GCM under a key derived from my seed, and its name is a hash. |
| Someone with my recovery phrase | Everything, as with every other private thing of mine. |
| A network watcher | TLS, then the same as the server. |

Metadata left: a burst of self-notes when I make many changes at once, on every server my device is
connected to. The same as the block list's notes today.

## 10. Scale (infinite-of-x) and budgets

- **Thousands of tasks:** kept in memory as a list (the board's shape), saved whole on each change
  (1,000 tasks is about 300 KB of JSON, one encrypt and one write, a few milliseconds, never per
  frame). The page draws the visible columns only. Above about 10,000 (rung 3) the Done column
  folds and the file splits by month.
- **Many devices:** each reads the same mailbox; notes merge in any order; nothing per device is
  kept anywhere.
- **Many servers:** a note goes to every server the device is connected to; rung 2's book gives a
  server met later what it lacks.
- **Per frame:** no work unless a note arrives or the person changes something. A note: one open
  and one signature check (a few milliseconds), then a merge by id.
- **Network:** one sealed note per change (16 or 32 KB with padding, each on every connected
  server); a bulk change goes in as few notes as fit. Rung 2's book refresh: the whole set at most
  once per refresh interval per server.
- **Website, first visit:** the page opens each envelope in the mailbox once (up to the server's
  TTL of mail), then reads only what is past its read position.

## 11. The rungs, each with its gate

0. **This plan.** Gate: written before the code (2026-11-02).
1. **Mine in both apps (built 2026-11-02).** Kept on the device, encrypted, crash-safe; notes to
   myself per change; the board beside it, never merged; Add to my tasks makes a Mine task; New
   task on My tasks makes a Mine task; move, prioritise, delete. Gates (each seen red first):
   a Mine task survives the server's list arriving and a restart (desktop
   `a_mine_task_survives_the_servers_list_and_a_restart`, website
   `scripts/tests/tasks-mine-web.test.js`); its words never reach a server (both: the frames the
   app sends hold no word of it, only a sealed `dm_put` to my own key); a task made on one device
   arrives on my other device and never on another person's (both apps, and across them: the
   desktop opens the website's note and the website opens the desktop's); the board's BUG-221
   tests still pass; the controls are clickable; the page's snapshots.
2. **The book.** Each device remembers, per server, which versions it has deposited there, and
   on connecting deposits what that server lacks; and it refreshes the whole set on a server when
   no note of mine is younger there than half the server's mailbox lifetime (the server tells it
   the TTL). Tombstones older than every server's lifetime are dropped. Gate: a device signed in
   for the first time after 40 days sees every task; a server met for the first time gets every
   task.
3. **Mine, fuller.** Edit the words, due dates with reminders on my own devices only, labels as
   filters, undo of a delete, the Done column folded. Gate: a reminder fires from the tray and no
   frame to any server carries its time.
4. **Shared with people I choose** (section 5). Gate: a list member's device shows the change;
   a non-member's never does; the relay's stored rows hold no list id, member or word.
5. **The board on the desktop for moderators and admins** (`task_update`, comments), the relay's
   role check unchanged. Gate: BUG-221's tests, and a member's desktop shows no edit controls.
6. **The personal backup slot** (two-realities: an encrypted, quota-bounded slot per permitted
   person) holds the long-term copy and replaces the book's refresh. Gate: a restore on a new
   device from the slot alone.
7. **The website for a passphrase-protected identity**: the Tasks page asks for the passphrase
   (or a same-origin worker holds the unlocked seed). Gate: such a person keeps and syncs Mine
   tasks on the website.
