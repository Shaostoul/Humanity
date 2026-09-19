# Scheduled re-votes

**Status:** steps 1, 2 and 3 shipped (v0.1302.0, v0.1303.0, v0.1320.0). Steps 4
(UI on both clients) and 5 (early reopening by petition) are still open.

A decision can be marked for review. When the review date arrives, the system
puts the same question back in front of people as a new vote. Nothing that was
already decided is edited, and no cast vote is ever changed.

This doc records the design, the one question it needed the operator to answer,
and the safeguard that makes the answer safe.

## The problem, in the operator's words

> "I like the votes having the option to be final and being open to revision at
> a later time. Like maybe once a year we can revote on certain things. That way
> if something about our way of life changes we can actually vote to change
> things."

His examples: a rule written before AI agents existed that now needs to
accommodate them, or a technology that changes the ground under an old decision.

## Why not simply let people edit their vote

Because a vote is a signed object. The voter's key signs exact bytes, and the
relay stores the first vote per identity with `INSERT OR IGNORE`
(`src/relay/storage/governance.rs`). An editable signed object is a
contradiction, and an audit trail you can rewrite is not an audit trail.

## The design: never mutate a vote, supersede a decision

- A cast vote stays immutable and final.
- A proposal may carry a review date (`review_after`) or a cadence
  (`review_cadence_ms`, for example one year). A cadence with no explicit date
  means the first review falls one cadence after the vote closes.
- When the date arrives, a NEW proposal is opened on the same question, carrying
  `supersedes` pointing at the old one.
- **The standing answer is the most recent CLOSED, carried decision in the
  chain.** An open review changes nothing. A defeated review leaves the previous
  answer standing. Only a review that closes and carries takes over.
- The whole chain stays visible. Seeing that a rule was reaffirmed three times is
  itself information, which is why superseding is worth more than editing.

`standing_decision` walks the chain forward and answers "what is the rule right
now". `decision_chain` returns the ordered history. `GET
/api/v2/proposals/{id}/chain` serves both, plus a `question_digest` per link.

Both reads are cycle-safe, because `supersedes` arrives inside a signed object
from an author nobody has to trust.

## Who signs an automatically-opened re-vote

The honest question step 3 had to settle: a re-vote must be a properly signed
`proposal_v1`, and the relay holds no user's key. So who signs it?

**The server, with its own identity.** The operator's call, and his words: "Makes
sense for the server to sign. We want to avoid forged signatures."

The reasoning, stated plainly because a signature is easy to misread as
agreement:

- A signature here means AUTHORSHIP and INTEGRITY. It says who produced these
  exact bytes and proves nobody altered them afterwards. It does not mean the
  signer endorses the proposal.
- Signing as the original proposer would forge their authorship. That person may
  have changed their mind since, may have left the community, may have died.
  Their original vote and their original proposal stay immutable and stay theirs.
- The server already has its own Dilithium3 identity for exactly this class of
  machine-authored record. It signs federation hellos and server announcements
  with the same key (`get_or_create_server_keypair` in
  `src/relay/storage/misc.rs`).
- The record says so in plain words. The successor carries an
  `auto_opened_note` field reading, in part, "This re-vote was opened
  automatically by the server because the review date set on the previous
  decision arrived."

The alternative, "nobody signs until a human confirms", was not chosen because it
turns a scheduled review into a task somebody has to remember, which is the
problem the feature exists to solve.

## The safeguard: the server may not write the question

A server that can open a vote could, if buggy or taken over, quietly reword what
people are voting on. So it is not allowed to write the question at all.

The successor carries the question of the proposal it supersedes, copied
VERBATIM, byte for byte. Everything the server authors lives in separate,
clearly-named fields.

### What is copied, and what the server writes

| Field | Who wrote it |
|---|---|
| `proposal_type` | copied from the predecessor |
| `scope` | copied from the predecessor |
| `title` | copied from the predecessor |
| `body` | copied from the predecessor |
| any other payload field | copied from the predecessor |
| `opens_at` | the server: now |
| `closes_at` | the server: now plus the predecessor's window, clamped to 7 to 90 days |
| `review_cadence_ms` | carried forward from the predecessor, so a yearly review stays yearly |
| `supersedes` | the server: the predecessor's object id |
| `auto_opened_by` | the server: its own `did:hum:` identifier |
| `auto_opened_at` | the server: when it opened this |
| `auto_opened_reason` | the server: the token `scheduled_review` |
| `auto_opened_note` | the server: one plain-English sentence saying what happened |

`proposal_type` and `scope` count as part of the question, not as metadata,
because the type decides the quorum and pass thresholds and the scope decides who
votes. A server quietly moving a proposal to a type with a lower bar would be
changing the vote as surely as rewording the title.

`closes_at` gets a seven-day floor because nobody chooses the moment a scheduled
review opens, so the community needs time to notice it even if the original vote
ran for an hour.

### How the rule is enforced

Twice, in two different places, so a bug in one does not silently disable it:

1. `build_scheduled_revote` (`src/relay/governance_revotes.rs`) copies the
   question fields out of the predecessor's stored payload. It never composes
   text.
2. `index_proposal` (`src/relay/storage/governance.rs`) REFUSES to link a
   server-signed successor whose question bytes differ from its predecessor's.
   The refused proposal is still stored, because it is a validly signed statement
   somebody made, but it gets no `supersedes` pointer and never joins the chain.

Check 2 also requires that a review was genuinely scheduled: the predecessor's
own author set a `review_after`, that date has arrived, and the original vote has
closed. The server cannot decide on its own that a settled question should be
reopened.

This is the narrowest rule that does the job. It does not extend to a peer
server's scheduler: a federated auto-review signed by some other server's key is
stored unlinked, because this server cannot judge whether that peer was entitled
to move this chain. Cross-server review is a separate design.

### What counts as "the question"

Everything in the payload except a short, fixed list of scheduling and machine
fields (`SCHEDULE_AND_MACHINE_FIELDS` in `src/relay/governance_revotes.rs`):
`opens_at`, `closes_at`, `review_after`, `review_cadence_ms`, `supersedes`, and
the four `auto_opened_*` fields.

The direction matters. An unknown field counts as part of the question, so a
server that invents a new one is caught by the byte comparison rather than
slipping past an allowlist somebody forgot to update. Options, choices or a
budget figure added to `proposal_v1` later are protected automatically, with no
change here.

### How a client verifies the copy for itself

Without taking this server's word for anything:

1. `GET /api/v2/proposals/{id}/chain` gives the ordered chain of object ids.
2. For each link, `GET /api/v2/objects/{object_id}` gives the signed object.
3. Verify each object's Dilithium3 signature against its own
   `author_public_key`. This is the step that makes the rest meaningful: it
   proves the bytes are the ones their author signed.
4. Decode the payload as canonical CBOR, drop the fields listed above, re-encode
   the remainder canonically, and compare the bytes across links.

The chain endpoint publishes `question_digest` (BLAKE3 of those bytes) per link
and a `same_question` flag as a convenience for showing "the same question,
asked three times". They are a convenience, never the authority. A client that
cares recomputes them.

## The scheduler

`open_due_revotes` is the whole scheduler; the timer only calls it.
`scheduled_revote_loop` runs hourly (30 seconds after boot, then every hour),
spawned in `run_relay` alongside the relay's other periodic work. A review date
is a date, not a deadline to the second, so hourly is plenty and costs one
indexed query per hour. On a server whose proposals never named a review date it
does nothing at all.

**Idempotence.** `proposals_due_for_review` carries a `NOT EXISTS` clause, so once
a successor exists its predecessor stops being returned. A scheduler that runs
twice, or a relay that restarts, opens one re-vote, not two. The test
`running_the_scheduler_twice_opens_exactly_one_revote` runs the sweep twice and
asserts exactly one successor.

**Failure does not become a flood.** An unlinked successor would leave the
predecessor due forever, so a failed attempt is recorded with a 24-hour cooldown
in `server_state` rather than retried every hour. Without that, one bug would
fill the database with near-duplicate proposals, one per hour, indefinitely.

At most 25 re-votes open per sweep, so a relay that was offline for a year does
not do all of it in one tick; the rest come due again next hour.

## Early reopening by petition (step 5, not built)

The design has always included a petition threshold that reopens a question
before its scheduled date, for the case where the world changes faster than the
cadence. It is not built, and this increment deliberately did not build it.

The two paths meet at the same place: both end in a successor proposal carrying
`supersedes`, and both must pass `index_proposal`'s authorization. Today that
function accepts exactly two authors, and each has its own justification:

- the predecessor's own proposer, who is revising their own decision, and
- this server, under the three conditions above.

A petition is a third entitlement and needs a third path: petition signatures
would have to be verifiable objects, counted against a threshold, and checked at
the same chokepoint. It must NOT arrive by loosening either existing rule. The
copy-the-question safeguard applies to it just as strongly, because a petition
that reopens a question with the question reworded is the same attack wearing a
different hat.

## Still open

- **Step 4, UI on both clients**: show the standing answer, the history, and the
  next review date. Native egui first per the Rust-first rule, then web mirrors.
  The chain endpoint already returns everything the UI needs, including whether a
  link was machine-opened and the note explaining it.
- **Step 5**, the petition path above.
- **Per-type thresholds in `standing_decision`.** It currently applies the
  universal minimum, a simple majority of decisive weight with at least one vote.
  The richer quorum and pass rules in `data/governance/proposal_types.ron` are
  read in the API layer, so a successor that clears a simple majority but not its
  type's supermajority will currently take over. Consulting the registry inside
  `standing_decision` is its own increment.
