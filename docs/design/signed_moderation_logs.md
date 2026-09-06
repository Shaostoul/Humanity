# Signed Moderation Logs And Space-Declared Authority

## Status

Accepted (2026-05). **Revised 2026-09-06** to answer the questions the original
left open, because they blocked implementation rather than merely deferring it.

The 2026-05 decision was right and is unchanged: moderation is space-scoped,
signed, append-only and client-verifiable. What it did not say was **who holds
the signing key at the moment an action happens**, and that single gap is why
nothing was built for four months. The relay executes moderation today (a
moderator types `/ban bob` over the chat socket), and the relay does not have
the moderator's secret key, so the relay cannot produce the signature the
design requires. Every attempt to implement the doc as written runs into that
wall on the first day.

The revision below resolves it, defines the action schema, and states what the
system deliberately does not promise.

## Context

A communication platform without verifiable governance collapses under spam,
harassment, coordinated abuse, impersonation, and moderator abuse without
accountability.

HumanityOS additionally requires longevity across time and forks, optional
decentralized replication, and offline-first clients that can enforce safety
policy locally.

If moderation is purely server-side and opaque:

- clients cannot reason about trust when operating offline
- decentralized replication becomes unsafe and inconsistent
- moderator abuse is harder to detect and resolve

What exists today, stated plainly so nobody re-derives it: a moderator's ban,
mute, verify or role change is a row in the operator's SQLite. Nothing is
signed, nothing is append-only, nothing is readable by the person it happened
to, and nothing survives a fork. That is precisely the opacity this project
exists to oppose, running on this project's own flagship server.

## Decision

Moderation and governance are space-scoped and verifiable.

- Each space declares an authority set (owner/admin/moderator keys) and rules.
- Moderation actions are recorded in an append-only log.
- Each moderation action is signed by an authorized moderation key.
- Clients apply moderation logs deterministically: bans and unbans, mutes and
  limits, hide or quarantine of specific content object hashes, and role or
  permission changes.

Server relay infrastructure may enforce moderation decisions at the edge, but
the source of truth for moderation decisions is the signed log.

### Who signs (the question the original left open)

**The moderator's client signs. The relay never signs a moderation action, and
never holds a key that could.**

A moderator already has a Dilithium3 identity: it is the same key that is their
chat identity, derived from their BIP39 seed. Moderating is therefore an
ordinary signed-object publication, exactly like casting a `vote_v1` or
publishing an `offering_v1`, and it rides the substrate that already exists
(canonical CBOR, `POST /api/v2/objects`, byte-locked across native, web and
relay by the KAT tests).

The flow is:

1. The moderator's client builds a `mod_action_v1` object and signs it.
2. It submits the object to the relay like any other signed object.
3. The relay verifies the signature, checks the signer against the space's
   authority set, and only then applies the effect (drops the socket, refuses
   the sender, hides the content).
4. The object is stored. It is now the record, and anyone can fetch and check
   it.

An action that arrives without a valid signed object is not applied. This is
the whole point: if the relay can act without a signature, the log is
decoration.

**Consequence for the existing slash commands.** `/ban`, `/mute`, `/verify` and
friends currently mutate the database directly with no signature anywhere. They
cannot be kept as they are and also be claimed as auditable. They become thin
wrappers that ask the client to build and submit the object, so the typed
command and the button produce the same record. A command path that cannot sign
(a bot, a console) may only perform actions the space policy marks as
`unsigned_allowed`, and those are recorded as unsigned with the actor named as
the relay operator, so the gap is visible rather than hidden.

### The action schema

`mod_action_v1`, canonical CBOR, keys in this order:

| Key | Type | Meaning |
|---|---|---|
| `action` | text | `ban`, `unban`, `mute`, `unmute`, `hide`, `unhide`, `grant_role`, `revoke_role`, `revoke` |
| `target` | text | Dilithium public key hex for a person, or an object id hex for content |
| `target_kind` | text | `identity` or `object` |
| `reason` | text | Plain language, shown to the target and in the public log. Required, may not be empty |
| `rule` | text | Identifier of the published rule being applied, or empty if none |
| `expires_at` | int | Milliseconds since epoch, or 0 for no expiry |
| `role` | text | For `grant_role` / `revoke_role` only, empty otherwise |

The object's own envelope carries the rest: `space_id` scopes the action,
`author_public_key` is the moderator, `created_at` is informational, and
`references` carries the superseded object id for `revoke`.

`reason` being mandatory is a design position, not an oversight. An unexplained
moderation action is the thing the Accord's appeals requirement exists to
prevent, and a schema that permits an empty reason will collect them.

### Authority: `space_policy_v1`

A space declares who may moderate it and under what rules, in its own signed
object, so the authority set is as verifiable as the actions taken under it.

| Key | Type | Meaning |
|---|---|---|
| `owner` | text | Public key hex of the space owner |
| `moderators` | array of text | Public key hex, each may sign `mod_action_v1` |
| `rules_url` | text | Where the published rules live |
| `rules_hash` | text | BLAKE3 of the rules text at publication time |
| `appeals` | text | How to appeal, in plain language |
| `unsigned_allowed` | array of text | Actions a keyless path may still perform |

`rules_hash` is what makes "the rules were published before you participated"
checkable rather than merely asserted: a client can prove which text was in
force when an action was taken.

The current owner may publish a new policy; the newest valid policy signed by
the current owner wins. Changing the owner key is how a space forks, which the
Accord treats as legitimate, so the mechanism is deliberately available rather
than prevented.

### Ordering, conflicts and supersession

Clients replay the log deterministically:

1. Sort by `created_at`, then by object id hex as the tie-break. Object ids are
   content hashes, so the tie-break is stable across every client without
   coordination.
2. Apply each action whose signer was in the authority set **at the time the
   action is applied during replay**, not at the time of reading. A moderator
   removed later does not retroactively void what they did while authorized,
   and a moderator added later cannot backdate.
3. A `revoke` referencing an earlier object id cancels it. Revoking something
   already revoked is a no-op, not an error.
4. `expires_at` in the past means the action is inert. Clients do not need to
   see an explicit unban for a timed mute to lapse.

`created_at` is attacker-controlled and the object format says so. It is used
for ordering because a moderator ordering their own actions wrongly only
affects their own space, and the authority check is what actually bounds harm.
Nothing security-relevant is decided by the timestamp alone.

### The audit surface

- A public, unauthenticated read of a space's moderation log, so a person can
  see what was done to them without asking permission from the person who did
  it.
- The target of an action can always read the action, including its reason.
- The published rules and the appeals path are linked from the same place.

## Consequences

### Positive

- Moderation actions are attributable and auditable.
- Offline-first clients can enforce safety rules without constant server access.
- Decentralized replication can coexist with governance.
- Spaces can fork by changing authority keys and rules.

### Negative

- Adds implementation complexity.
- Requires careful key management for moderators: a moderator who loses their
  seed loses the ability to moderate, and one whose seed leaks can be
  impersonated until the owner publishes a policy without them.
- Once replicated, content cannot be guaranteed deletable; moderation becomes
  non-display and non-relay.
- Moderation now requires an unlocked identity. A moderator cannot act from a
  device where they have not signed in, which is a real usability cost accepted
  deliberately.

### What this does not promise

Stated so the transparency claim stays honest:

- It does not make content unrecoverable. A hide is a hide.
- It does not prevent a relay operator from dropping traffic at the edge
  without publishing anything. It makes such an action *unaccounted for* rather
  than impossible, which is the achievable goal.
- It does not stop a majority of a space's authority set from acting badly in
  concert. It records that they did.

### Non-negotiable requirements created by this decision

- Spaces must publish authority and policy metadata.
- Moderation actions must be signed and append-only.
- Clients must implement deterministic enforcement.
- Every action carries a reason, and the target can read it.

## Rejected alternatives

### Opaque server-only moderation

Rejected due to offline-first requirements and lack of verifiable governance.
This is also what the code does today, which is why it is named here.

### Fully democratic moderation without declared authority

Rejected due to high abuse risk and unclear accountability.

### Global moderation only (platform-wide authority) for everything

Rejected because spaces require sovereignty and diverse rulesets.

### The relay holding a moderation key and signing on a moderator's behalf

Rejected 2026-09-06. It would let the design ship quickly and be implemented
entirely server-side, and it is exactly the shape the original doc's silence
invited. It is worthless: a log signed by the party whose behaviour it is meant
to constrain proves only that the relay agrees with itself. If the relay can
sign, moderator abuse and relay abuse are indistinguishable in the record.

### Recording unsigned audit rows now and adding signatures later

Rejected as a migration path, though tempting. An unsigned audit table is cheap
and looks like progress, but it establishes an interface (and a page showing
it) that asserts accountability the data cannot support, and every later
signature added on top has to reconcile with rows that were never signed. Build
the signed path first even though it is slower.

## Implementation order

Each rung is useful alone and none of them lie about what the one below it
provides.

1. `space_policy_v1` and `mod_action_v1` schemas, plus builders in the relay,
   native and web, cross-checked by a KAT the way `vote_v1` is by
   `just vote-kat`. Without byte-identical builders the object id differs per
   client and nothing verifies.
2. Relay-side ingest: verify signature, check the signer against the current
   policy, apply the effect, store the object. This is the rung where the
   promise becomes real.
3. Convert the existing slash commands and mod buttons to build and submit
   objects, so no unsigned mutation path remains except the ones the policy
   explicitly allows.
4. The public log read and the audit page, linked from the rules page.
5. Client-side deterministic replay, which is what makes offline enforcement
   and forking work.

## Current state (2026-09-06)

None of the above is implemented. There is no `mod_action_v1`, no
`space_policy_v1`, and no audit row of any kind: a ban is an unsigned,
unexplained, unreadable row in one SQLite file. The rules and appeals page
published on 2026-09-06 describes what moderators may do and how to appeal, and
says plainly that the audit trail does not exist yet, because promising it
before it is built is the failure mode this document is trying to avoid.
