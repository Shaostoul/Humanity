# Organisations: a group that runs itself

**Status: planned 2026-11-14; rung 1 (members, roles, invitations, the directory, the audit log,
both apps) built the same day.** Code: `crates/humanity-protocol/src/org.rs` (the signed log, its
fold, the directory and the audit log: the rules every app and every server agree on),
`src/gui/pages/orgs.rs` and `src/gui/orgs_store.rs` (the desktop page and the logs kept on the
device), `web/shared/org-log.js` and `web/pages/orgs-app.js` (the same rules and the page on the
website), `data/orgs/roles.json` (the roles a new organisation starts with, and every permission).
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
(create, invite, members, roles) without that list. **Recommendation (question 2 below):** once
rung 2 delivers changes without copying, retire guilds into organisations: the Guilds page becomes
the Organisations page and the three guild tables are dropped (no migration: nobody uses them yet).

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

**`org_entry_v1`, one change.** `references = [org_id]`; payload `{act, ...}`:

| `act` | Fields | Who may sign it |
|---|---|---|
| `invite` | `who` (the invitee's public key), `role`, `until` (ms) | `members.invite`, and the role ranks below the signer's |
| `withdraw` | `invite` (the invite's entry id) | `members.invite`, or the inviter |
| `accept` | `invite` | Only the invitee named in it, before `until`, once, if not withdrawn |
| `remove` | `who` | `members.remove`, and the member ranks below the signer |
| `leave` | (none) | Any member except an owner |
| `assign` | `who`, `role` | `roles.assign`; both the member's current role and the new one rank below the signer's |
| `role_set` | `role` (a role map) | `roles.define`; a custom role only (not owner, admin or member), ranked below the signer, granting no permission the signer lacks |
| `role_drop` | `role` (an id) | `roles.define`; a custom role ranked below the signer; its holders become members |
| `listing` | `name` (up to 60), `level` | The member themself: how they appear in the directory (`members` or `organisers`) |
| `settings` | any of `name`, `about`, `directory` | `org.settings` |

### 2.2 The fold

Every app computes the current state the same way: drop entries whose signature does not verify or
that name another organisation; drop duplicates by `object_id`; sort by `(created_at, object_id)`;
apply each in turn, refusing any whose signer lacks the permission at that point. A refused entry
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
The owner cannot leave or be removed; handing the organisation on is rung 2 (`transfer`, signed by
the owner and accepted by the new owner).

### 2.5 Known limit of rung 1, and its fix

Ordering trusts each entry's own `created_at`. A removed admin could sign an entry dated before
their removal, and an app that had not yet seen that entry would apply it. Rung 2 adds
**checkpoints**: the owner's app signs, with each change it sends, the set of entry ids it holds; an
entry dated before a checkpoint that does not name it is refused by every app. Until then the audit
log shows every entry with its signer, so a backdated one is visible.

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
| **2** | Delivery as sealed DMs to each member and to your own devices; transfer of ownership; checkpoints; organisers-only names kept from members | unchanged | unchanged | One sealed DM per member per change (about 8 KB padded); a 50-member group's change is about 400 KB in total, once |
| **3** | A shared calendar with RSVP and reminders; announcements; opening a P2P group chat for members | under 1 ms (a month view) | unchanged | One sealed DM per member per event change |
| **4** | Sign-up forms, surveys and a booking page (for members, or a public page through a server) | under 1 ms | unchanged | Replies travel as sealed DMs to the organisers; a public booking page is the one place a server sees a form, by the organisation's choice |
| **5** | Shared folders (the Files page with an organisation place) and shared tables; organisation-scoped tasks | under 1 ms | unchanged | Files by the existing file passes and transfers; table edits as sealed DMs |
| **6** | Decisions: `proposal_v1` scoped to an organisation, the roster as electorate | unchanged | unchanged | One proposal and one vote object each, as today |
| **7** | Large organisations: an organisation key, one sealed blob per change in an opaque mailbox; public organisations listed by a server; guilds retired | unchanged | unchanged | One blob per change, about 8 KB, whatever the size |
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
2. **Retire guilds into organisations once rung 2 lands?** Recommendation: yes. Guilds keep a member
   list in the clear on one server; organisations do the same job without it. One page, one word.
3. **Should a member be able to see other members' public keys?** Rung 1 cannot hide them (every
   member holds the log). Recommendation: accept it for organisations up to rung 7, say so on the
   page (done), and give organisations that need members hidden from each other rung 7's
   organisation key, where organisers can hold entries only organisers read.
4. **Should the owner be able to hand the organisation on?** Recommendation: yes, in rung 2, signed
   by the owner and accepted by the new owner, the way an invitation is.
