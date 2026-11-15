# Organisations: a group that runs itself

**Status: planned 2026-11-14; rung 1 (members, roles, invitations, the directory, the audit log,
both apps) built the same day; rung 2 (delivery as sealed DMs, handing the organisation on, the
order and checkpoint rules, guilds retired) built 2026-11-15 (section 8b).** Code: `crates/humanity-protocol/src/org.rs` (the signed log, its
fold, the directory and the audit log: the rules every app and every server agree on),
`src/gui/pages/orgs.rs` and `src/gui/orgs_store.rs` (the desktop page and the logs kept on the
device), `web/shared/org-log.js` and `web/pages/orgs-app.js` (the same rules and the page on the
website), `data/orgs/roles.json` (the roles a new organisation starts with, and every permission). Rung 2's
delivery: `src/gui/org_mail.rs` (the rules) and `src/engine/org_mail.rs` (the sockets) on the
desktop, `web/shared/org-mail.js` and the Delivery section of `web/pages/orgs-app.js` on the website.
Algorithms: `docs/reference/cryptography.md` ("Organisations").

## Why, in the operator's words

On 2026-11-14 the gap analysis against Microsoft's services (`docs/design/everyday-suite.md`)
listed organisation tools: members, roles, directories and audit logs for the groups that run on
HumanityOS. The operator: "This definitely sounds high priority." The analysis puts them first in
its order (section 11 there), together with the shared calendar.

The mission's own unit is the group ("turn groups into cooperatives", ROADMAP). Most of the parts a
group needs exist already, scattered: chat servers with roles, guilds, P2P groups, the task board,
signed voting, the admin action registry, the calendar. What is missing is one place where a group
(a volunteer group, a club, a household, a cooperative, a school class, a small business) comes
together and runs itself, and that place must not hand a server the list of who belongs to what.

## 1. What an organisation is

An organisation is a group with:

| Part | What it means | Rung |
|---|---|---|
| Members | Who belongs. Joining is always the joiner's own signed act. | 1 |
| Roles and permissions | Who may do what. Owner, admin and member to start; custom roles as data. | 1 |
| Invitations | An organiser invites a person; only that person's acceptance makes them a member. | 1 |
| A directory | The member list, with a privacy level for the whole list and one for each member. | 1 |
| An audit log | Every membership and role change, each signed by the person who made it, readable by every member. | 1 |
| Delivery to members | Changes reach every member's devices without anyone copying text. | 2 |
| A shared calendar | Events with RSVP and reminders. | 3 |
| Announcements | A channel only organisers post in. | 3 |
| Sign-ups and bookings | Forms, surveys and a booking page, for members or the public. | 4 |
| Shared folders and shared tables | Documents; the Lists idea (a member list, an inventory, a delivery route). | 5 |
| Tasks | The task board, scoped to the organisation. | 5 |
| Decisions | Signed votes with the organisation's members as the electorate. | 6 |
| Printable material | Flyers and newsletters through Typst. | 7 |

### 1.1 Built on what exists, not beside it

- **Signed objects** (`docs/design/storage-architecture.md`): every change is a canonical-CBOR
  object signed by its author's Dilithium3 / ML-DSA-65 key, the same `ObjectBuilder` on the desktop
  and `buildSignedObject` (`web/shared/pq-object.js`) on the website. No new algorithm.
- **The P2P group's shape** (`docs/design/p2p-groups.md`): an object that founds the group, then an
  append-only log of signed entries whose deterministic fold is the current state, and "nobody is
  put in a group without asking to be". An organisation adds roles, permissions and a directory to
  that shape. A P2P group stays what it is (a sealed chat); an organisation will be able to open one
  for its members (rung 3).
- **The certificate pattern** (`docs/ai/storage.md`): membership and roles are proven by signed
  entries the members hold and any app checks for itself, never by a row a server keeps.
- **The admin action registry** (`data/admin/actions.json`, `docs/design/in-app-ops.md`): every
  organisation action is listed there, category `orgs`, so the GUI and an AI can enumerate them. They
  run `"device"`: the person's own app signs the entry; no server runs them.
- **Sealed notes** (`docs/design/tasks.md`, "Shared with people I choose"): rung 2 carries each
  change to each member as a sealed-sender v2 DM, the way shared task lists travel.
- **Governance** (`docs/design/governance-*`, `proposal_v1` / `vote_v1`): rung 6 scopes a proposal to
  an organisation and counts only its members' votes.
- **The calendar, the task board, Files** (FEATURES "Calendar Page", "Task Board", "Files"): rungs 3
  to 5 give each an organisation scope rather than building a second one.

### 1.2 Guilds

Guilds (`src/relay/storage/guilds.rs`, FEATURES "Guild System") are a server's rows: `guild_members`
lists who belongs to which guild in the clear, on one server. That is the shape the privacy
hardening removed everywhere else (`groups`, `follows`). Organisations do everything guilds do
(create, invite, members, roles) without that list. **Retired 2026-11-15 (decided 2026-11-14):** the
Guilds page, its routes (`/api/guilds`, the web page `/guilds`, which now sends a browser to
`/orgs`), its words and its three tables are gone; the relay drops the tables from a database that
still has them (`the_live_database_sheds_the_guild_tables`), with no migration, since nobody used
them. `tests/guilds_retired_lint.rs` keeps them gone.

## 2. How membership and roles are proven

### 2.1 The objects

**`org_v1`, the founding.** Signed by the founder, who is the first owner. `created_at` required,
no references. Payload, canonical CBOR map:

| Field | Type | Meaning |
|---|---|---|
| `name` | text, 1 to 80 characters | The organisation's name |
| `about` | text, up to 500 | One paragraph about it |
| `directory` | text | Who may read the member directory: `members` (default), `organisers` (those with `directory.see_all`), `public` (anyone the log is shared with) |
| `roles` | array of role maps | The roles it starts with, copied from `data/orgs/roles.json` when it is founded |

A role map is `{id, name, rank, perms}`: `id` lowercase letters, digits and `_` (1 to 32),
`name` up to 40 characters, `rank` 0 to 100, `perms` an array of permission ids. The organisation's
id is the founding object's `object_id` (BLAKE3 of its canonical bytes): self-certifying, minted by
no server.

**`org_entry_v1`, one change.** `references = [org_id, prev...]`, where `prev` (rung 2, at most 16,
sorted) names the entries its signer's log held as its heads when they signed it: the applied entries
no other applied entry follows (`OrgState::heads`). Payload `{act, ...}`:

| `act` | Fields | Who may sign it |
|---|---|---|
| `invite` | `who` (the invitee's public key), `role`, `until` (ms) | `members.invite`, and the role ranks below the signer's |
| `withdraw` | `invite` (the invite's entry id, or a handover's) | `members.invite`, or the inviter; a handover only by the owner who offered it |
| `accept` | `invite` | Only the invitee named in it, before `until`, once, if not withdrawn |
| `remove` | `who` | `members.remove`, and the member ranks below the signer |
| `leave` | (none) | Any member except an owner |
| `assign` | `who`, `role` | `roles.assign`; both the member's current role and the new one rank below the signer's |
| `role_set` | `role` (a role map) | `roles.define`; a custom role only (not owner, admin or member), ranked below the signer, granting no permission the signer lacks |
| `role_drop` | `role` (an id) | `roles.define`; a custom role ranked below the signer; its holders become members |
| `listing` | `name` (up to 60), `level` | The member themself: how they appear in the directory (`members` or `organisers`) |
| `settings` | any of `name`, `about`, `directory` | `org.settings` |
| `transfer` | `who`, `until` (ms) | The owner only, to another member: an offer, which changes nothing by itself; a newer one replaces an older one (rung 2) |
| `accept_transfer` | `transfer` (the handover's entry id) | Only the member it names, before `until`, while its giver is still the owner: they become the owner and the giver an admin (rung 2) |

### 2.2 The fold

Every app computes the current state the same way: drop entries whose signature does not verify or
that name another organisation; drop duplicates by `object_id`; HOLD any entry that follows (its
`prev`) an entry not here, or one itself held: it is counted, never applied and never refused, until
what it follows arrives (rung 2); order the rest so every entry comes after what it follows, the
earliest `(created_at, object_id)` first among those ready; apply each in turn, refusing any whose
signer lacks the permission at that point; then apply the checkpoint rule (section 2.5) and fold
again until it voids nothing new. Someone holding only an invitation folds it with
`fold_invitation`, which places an entry by what is there instead of holding it (an invitation
leaves out the entries the inviter's own entries follow) and voids nothing. A refused entry
stays in the audit log marked "not applied" with its reason, so nothing is hidden and nothing
changes. An entry dated more than five minutes ahead of the reader's clock waits until it is not.

### 2.3 Permissions

Enforced by the fold since rung 1: `members.invite`, `members.remove`, `roles.assign`,
`roles.define`, `org.settings`, `directory.see_all`. Carried and shown from rung 1, enforced when
their rung lands: `calendar.edit`, `announce.post`, `signups.manage`, `bookings.manage`,
`files.edit`, `tables.edit`, `tasks.edit`, `votes.open`. The catalogue, with each permission's words,
is `data/orgs/roles.json`. A role grants exactly the permissions it lists; the owner's role lists
all of them.

### 2.4 Rank

Every role has a rank. You can act only on people and roles ranked below your own, so an admin can
never remove or demote the owner or another admin, and nobody can hand out more than they hold.
The owner cannot leave or be removed; they hand the organisation on (rung 2): a `transfer` the owner
signs, then the named member's `accept_transfer`, both kept in the log; the owner role moves only when
both verify, and the giver becomes an admin.

### 2.5 Checkpoints (rung 2): a backdated change is voided

Rung 1's ordering trusted each entry's own `created_at`, so a removed admin could sign an entry
dated before their removal and an app that had not seen it would apply it. Rung 2 closes it with the
`prev` history every entry carries. A **checkpoint** is an applied entry that takes a role away: a
`remove`, an `assign` to a lower-ranked role, or an `accept_transfer` (for the giver). Every entry
by the person it takes the role from that would come before it in the order and is NOT in its
history (its signer had not seen it) is **voided**: refused, shown in the record of changes as "its
signer lost their role to a change made without seeing it". Removing someone therefore voids
whatever they did that the remover had not seen, which is also what a careful organiser would want.
The plan's earlier wording (the owner's app signing the set of ids it holds) became this: the
history is in every entry already, so nothing extra is signed.

## 3. What the relay stores and what stays on devices

| | Rung 1 | Rung 2 | Rung 7 (large organisations) |
|---|---|---|---|
| The log (who belongs, roles, names) | Each member's devices only | Each member's devices only | Each member's devices only |
| How a change travels | Signed text the person carries (copy, paste into any DM or message) | A sealed-sender v2 DM to each member (`[[hum:org:v1]]`), through their "who can reach me" | One sealed blob per change under an organisation key, in a mailbox named by an opaque id |
| What a server sees | Nothing | Sealed mail in each member's mailbox: how many, padded size, which day; no sender, no organisation, no word of it | Sealed blobs in one opaque mailbox: count, padded size, time |
| Server tables added | None. The relay refuses an `org_` object from a submission or from gossip (`put_signed_object`), so a log can never be stored in the clear by mistake | None | One mailbox table holding only ciphertext |

The relay holds no table the privacy hardening removed and no organisation table at all
(`organisations_leave_no_trace_on_the_relay`, `src/relay/storage/orgs_guard_tests.rs`).

**On the device:** the desktop keeps every organisation an identity belongs to in one encrypted
file, `<data folder>/orgs/<hash of the identity>.orgs` (AES-256-GCM under BLAKE3
`derive_key("hum/orgs/v1", seed)`, the pattern of Tasks of my own; a file that does not open is set
aside, never written over). Inside, each organisation's log is the same JSON the person carries
(section 3.1). The website keeps the same JSON in this browser's storage (`hos_orgs_v1:<did>`), encrypted
with AES-256-GCM under HKDF-SHA-256 of the seed (info `hum/orgs-web/v1`, `getPqOrgsKey`), said on
its page. A log never holds a private key.

### 3.1 The carried text

```
{"hum_org": 1, "org": "<org id, hex>", "objects": [<signed object>, ...]}
```

Each signed object is the JSON shape `POST /api/v2/objects` takes (`SignedObjectSubmission`: base64
key, payload and signature). Opening one merges its objects into the log by `object_id`, the
`applySyncBundle` pattern, so opening the same text twice changes nothing and texts can arrive in
any order.

**An invitation carries only what proves it:** the founding, the invite, and the entries that gave
the inviter their role (their own invite, acceptance and role changes, and so on back to the
founder). It never carries the member list of a private organisation to someone who has not
joined. After accepting, the new member receives the whole log from an organiser.

## 4. Privacy by default

- **The directory** is readable by members only (`members`, the default). An organisation can make
  it `organisers` (only those with `directory.see_all`) or `public`.
- **Each member chooses** how they appear: `members` (their name, to everyone who can read the
  directory) or `organisers` (named only to organisers; to other members they are counted, not
  named, in the directory and in the audit log).
- **A non-member** reads nothing of a private organisation: the apps refuse to draw its directory
  or log for them, and the invitation they hold names only the chain that proves it.
- **What this does not protect** (said on the page, in words): every member holds the log, so a
  member who reads the raw text can see every member's public key; the per-member level decides
  names on the screen, not secrecy from a determined member. Rung 2 keeps an organisers-only
  member's chosen name out of what is sent to other members.
- **Leaving** is one signed entry; nobody can stop it. Removing someone needs the permission and a
  higher rank, and is in the audit log with the remover's signature.

## 5. Federation

There is nothing for servers to federate in rungs 1 to 6: the log lives on members' devices, and
rung 2's sealed DMs already reach a member on any server their app uses. Members on different
servers are members of the same organisation, because membership is the signed log, not a server
row. Rung 7's mailbox lives on whichever server the owner picks, with a second server as a mirror;
losing both costs only delivery, never the organisation, because every member holds the log.

## 6. The real-life twin and the game twin

Every domain exists twice (`docs/design/two-realities.md`). **Real life:** a volunteer group, a
household, a club, a cooperative: the app's Organisations page, always real. **The game:** a ship's
crew and a cooperative aboard the fleet: rung 8 lets an organisation be a crew in the world, its
roles mapping to the ship ranks that already exist (`can_edit_ship`, household plot permits), with
the same signed log, so a crew's roster is as private as a club's. The game never needs a server
table for a crew either.

## 7. The rungs, each with its budget

Budgets: organisations are a page, not part of the 3D frame. Per-frame cost is the page's drawing
only, while it is open; the fold runs when the log changes, never per frame (in-memory, rebuilt on
every change of its source, and every frame with the reuse switch off).

| Rung | What it adds | Per frame (page open) | Fold, on a change | Network |
|---|---|---|---|---|
| **1** | The log, the fold, roles as data, invitations accepted by the invitee, the directory with privacy levels, the audit log, both apps, the registry's `orgs.*` actions | under 0.5 ms to draw 200 members | about 0.3 ms a signature checked: 200 entries in about 60 ms, on a change only | None: the person carries text (about 7 KB an entry) |
| **2** | Delivery as sealed DMs to each member and to your own devices; transfer of ownership; checkpoints; guilds retired (built 2026-11-15). Organisers-only names kept from members moves to rung 3 (section 8b) | unchanged | unchanged | One sealed DM per member per change (about 8 KB padded); a 50-member group's change is about 400 KB in total, once |
| **3** | A shared calendar with RSVP and reminders; announcements; opening a P2P group chat for members | under 1 ms (a month view) | unchanged | One sealed DM per member per event change |
| **4** | Sign-up forms, surveys and a booking page (for members, or a public page through a server) | under 1 ms | unchanged | Replies travel as sealed DMs to the organisers; a public booking page is the one place a server sees a form, by the organisation's choice |
| **5** | Shared folders (the Files page with an organisation place) and shared tables; organisation-scoped tasks | under 1 ms | unchanged | Files by the existing file passes and transfers; table edits as sealed DMs |
| **6** | Decisions: `proposal_v1` scoped to an organisation, the roster as electorate | unchanged | unchanged | One proposal and one vote object each, as today |
| **7** | Large organisations: an organisation key, one sealed blob per change in an opaque mailbox; public organisations listed by a server | unchanged | unchanged | One blob per change, about 8 KB, whatever the size |
| **8** | The game twin: an organisation as a ship's crew or cooperative, roles mapped to ship ranks | none in the 3D frame (ranks resolved on change) | unchanged | Only gameplay state, as today |
| **9** | Printable material through Typst | none | none | none |

Infinite-of-x: roles, permissions and every word are data (`data/orgs/roles.json`, `en.json`);
an organisation has as many custom roles and members as it likes (rung 1 caps a log at 10,000
entries to bound the fold; rung 7 lifts the cap with checkpoints).

## 8. Rung 1, as built (2026-11-14)

- **The rules** in `crates/humanity-protocol/src/org.rs`: building each object, the fold, the
  directory and the audit log views, the invitation's proof chain, the carried text. The website's
  `web/shared/org-log.js` folds the same frozen log to the same state
  (`scripts/tests/org-log-web.test.js` and `org::tests::the_frozen_log_folds_as_recorded`). The log,
  `scripts/tests/fixtures/org-log-v1.json`, was built and signed by the website's code
  (`scripts/gen-org-fixture.mjs`) with the expected state written by hand, so the desktop checks
  the website's signatures and encoding; it is also the saved shape's guard.
- **The desktop page** `src/gui/pages/orgs.rs` (Chat > Organisations): your organisations, start
  one, open a text (an invitation, an acceptance, an update), and per organisation the members
  (the directory), invite, roles, the record of changes (the audit log), settings and "me here",
  each control with its words by key and its hint. Logs kept by `src/gui/orgs_store.rs`.
- **The website** `web/pages/orgs.html` and `orgs-app.js`: the same page, signing with the
  person's key as the Governance page does.
- **The registry** (`data/admin/actions.json`, category `orgs`): `orgs.found`, `orgs.open`,
  `orgs.accept`, `orgs.invite`, `orgs.withdraw`, `orgs.remove`, `orgs.leave`, `orgs.assign`,
  `orgs.role_set`, `orgs.role_drop`, `orgs.listing`, `orgs.settings`, `orgs.directory`,
  `orgs.log`, `orgs.share`, `orgs.forget`, each `"run": "device"` with the `org_permission` it
  needs. The run endpoint refuses them; both apps' "Every admin action" lists say where they run
  (`every_organisation_change_is_a_registry_action_that_runs_on_the_device`).
- **The relay** refuses `org_` objects in `put_signed_object`, the one door for submissions and
  gossip (section 3), and keeps nothing about organisations.
- **Not wired yet:** an AI drives the page through the registry's list today, not yet through the
  dev IPC (the registry's own rung for device actions).

## 8b. Rung 2, as built (2026-11-15)

- **The rules** (`crates/humanity-protocol/src/org.rs`, twin `web/shared/org-log.js`): `prev`,
  holding, the causal order, checkpoints, `transfer` and `accept_transfer`, `fold_invitation`. The
  frozen log `scripts/tests/fixtures/org-log-v1.json` was rebuilt on purpose with `prev` (rung 1's
  entries had none) and now also holds a handover, a refused acceptance by the wrong member, a voided
  change by the old owner and a held entry; both apps fold it to the hand-written record.
- **Delivery** (`src/gui/org_mail.rs`, `src/engine/org_mail.rs`; website `web/shared/org-mail.js`
  and the Delivery section of `web/pages/orgs-app.js`): each signed change is queued at once, kept in
  the encrypted organisations file (desktop) or value (website) until sent, and sent as a sealed-sender
  v2 DM `[[hum:org:v1]]{carried text}` to every member of the organisation as it is after the change,
  to the person a removal names, and to my own mailbox (my other devices); an invitation also goes to
  the person invited as the invitation text. Sealing uses only the key on the member's own key card,
  checked and pinned (a member with no card yet is asked for one and waits). Sends go at the server's
  `dm_put` pace. The inviter's app, seeing an acceptance of its own invitation, forwards it to the
  other members and sends the new member the whole log.
- **Taking** a note: from my own key; from a member (or an open invitee or handover recipient) of an
  organisation I am in; or an invitation for me from its inviter. A note from someone this device
  does not yet know as a member waits in memory for the log that proves them; anything else is not
  merged, so a removed member's or a stranger's notes never fill the log. Whatever is taken, the fold
  checks.
- **Through "who can reach me"**: a member whose message setting keeps the sender out gets nothing
  (the relay refuses the note, reason `reach`); the page names them and offers Share the latest. The
  desktop's notes carry the friendship pass the member gave the sender, as every control message does;
  **the website's do not yet** (the passes live in the chat's own store), so from the website a member
  whose setting is "friends" is reached only through the fallback for now.
- **Proof**: `org::tests` (held then applied, a backdated change voided, a handover needs both
  signatures, the frozen log), `org_mail_tests` in `src/gui` (whole notes, a removed member stops
  receiving, who is taken, the inviter forwards, the website's notes of one delivery applied here),
  `engine/org_mail_tests.rs` (two desktop apps: invitation, acceptance, the whole log, a change
  applied in the other app, the removal reaching the removed member and nothing after it; a forged or
  stranger's change never applied), `scripts/tests/org-log-web.test.js` and `org-mail-web.test.js`
  (the website's twins, the same note bytes `scripts/tests/fixtures/org-note-v1.json`).
- **Not in rung 2** (moved to rung 3): keeping an organisers-only member's chosen name out of what is
  sent to other members. The name sits in a signed `listing` entry, and every member must hold every
  entry for `prev` to work, so this needs the name to travel apart from the entry (a commitment in
  the entry, the words sealed to organisers), a format change of its own.

## Decided (operator, 2026-11-14)

Asked which real group goes first (recommended: Sponsor-a-Can, so every rung is tested by people
who need it: cart days on the calendar, sign-ups, the delivery route as a shared table) and whether
guilds retire into organisations once rung 2 lands (recommended: yes, guilds keep a member list in
the clear on one server), the operator answered: "Agreed." Sponsor-a-Can is the first real group,
and guilds retire into organisations.

## 9. Questions for the operator (each with a recommendation)

1. **Which real group goes first?** Recommendation: Sponsor-a-Can, the volunteer group you already
   help run, so every rung is tested by people who need it (members, a shared calendar for cart
   days, sign-ups, the delivery route as a shared table).
2. **Retire guilds into organisations once rung 2 lands?** Decided 2026-11-14 ("Agreed."); done
   2026-11-15 (section 1.2).
3. **Should a member be able to see other members' public keys?** Rung 1 cannot hide them (every
   member holds the log). Recommendation: accept it for organisations up to rung 7, say so on the
   page (done), and give organisations that need members hidden from each other rung 7's
   organisation key, where organisers can hold entries only organisers read.
4. **Should the owner be able to hand the organisation on?** Built in rung 2 as recommended: the
   owner's `transfer`, the new owner's `accept_transfer`, both in the log.
