# Where the software does not yet meet the Accord

## Purpose

The Accord states what HumanityOS must do. This document states where it does
not do it yet.

Both halves are necessary. Rights and Responsibilities section 3 binds this
project to "accurate claims about system capabilities" and says plainly that
"Claiming security or privacy that does not exist constitutes harm." A charter
that asserts a protection the code does not provide, with nothing recording the
difference, is exactly that harm. So the principles stay as written, and every
known gap is listed here with the file and line that proves it.

Nothing here is a plan to weaken the Accord. Each gap is a defect to close in
the code. Until it closes, a reader is entitled to know.

**Do not read a silence here as conformance.** This list contains what an audit
has found. It is not proof that nothing else is missing.

Last reviewed: 2026-10-09 (the contact-consent gap closed); before that 2026-09-14, from the library documentation audit recorded in
`docs/history/2026-09-14-library-audit.md`.

---

## Governance votes are weighted, not one person one voice

**The Accord says** (Rights and Responsibilities, section 5, Equal Standing):
"one person, one voice", "no amplification, suppression, or substitution", "no
privilege based on wealth, power, or access".

**The software does** weight every vote by the voter's trust score at the moment
they cast it. `src/relay/storage/governance.rs` snapshots
`get_trust_score(...).total`, clamps it to `MAX_VOTE_WEIGHT = 0.95`, stores it as
`weight_at_vote`, and the tally sums that column rather than counting rows.

The trust score is itself a weighted blend, defined in
`src/relay/storage/trust_score.rs`: verifiable credentials 0.30, vouches from
other members 0.25, recent activity 0.15, account age 0.10, stake 0.10,
reputation 0.10. Its sigmoid returns 0.0 for an input of 0.

**So** a member who joined today, holds no credentials and has nobody vouching
for them carries a small fraction of the weight of a long-tenured, well-vouched
member. Influence is proportional to standing in the community, which is a
defensible design and is not what section 5 promises.

**To close it**, either count rows instead of summing `weight_at_vote` and keep
the trust score for eligibility only, or amend section 5 to describe weighted
voting and name the inputs. That is a product decision, not a bug fix.

---

## Governance votes are public, not confidential

**The Accord says** (Rights and Responsibilities, section 2, Privacy and
Confidentiality): "confidentiality of beliefs, votes, and associations".

**The software does** store each vote as a signed object carrying `voter_did`,
and serves them from an endpoint with no authentication. `list_objects` in
`src/relay/api_v2_objects.rs` takes only `State` and `Query`, and the route is
registered in `src/relay/mod.rs` with a plain `get(...)`. A request filtered by
`object_type=vote_v1` and an author fingerprint returns that identity's votes,
with the choice recoverable from the payload.

**So** anyone can determine how a given identity voted. Because each vote is
signed, a voter can also prove their choice to someone else, which is the
mechanism vote-buying and coercion need.

**To close it**, separate eligibility from ballot content before claiming ballot
confidentiality, or remove votes from the section 2 list and state that
governance votes are public signed objects by design, naming the trade-off.

---

## Contact consent cannot be withdrawn (CLOSED 2026-10-09, with the limits below)

**The Accord says** (Communication and Association, Consent in contact): "A user
must be able to restrict who can contact them" and "A user must be able to close
contact pathways without escalating to moderators." Consent and Control adds that
consent "must be revocable: immediately, without justification, without loss of
essential function", and lists "irreversible lock-in" as a prohibited pattern.

**Until 2026-10-09 the software did not:** a friendship certificate, once given,
granted its holder unlimited direct messages forever, unfollowing did not close
the pathway, and nobody could block anyone.

**The software now does** (`docs/design/blocking-and-safe-mode.md`):
- **Withdrawable friendship passes** (v0.1466.0): each pass carries a serial and
  what the friend may do; Unfollow and Block withdraw it at once (`cert_revoke`,
  checked by `friend_pass` in `src/relay/handlers/friend_passes.rs`).
- **Who can reach me** (v0.1467.0): for messages, calls and trades, each person
  chooses Nobody, People I choose, Friends, Friends and people in my groups, or
  Anyone, and the relay enforces it for everyone, admins included
  (`src/relay/handlers/reach.rs`). By default only friends can message or trade
  and only chosen people can call.
- **Block** (v0.1468.0), on both clients, without asking a moderator: it withdraws
  the passes you gave, hides everything from that person, and tells them nothing.

**What it still cannot do**, so nobody is told more than is true:
- Block is carried out by your own app and by withdrawing passes. If you set
  Messages to "Anyone", a blocked person's messages still reach your mailbox (up
  to the 20 a day any stranger may send) and your app discards them unread; only
  a narrower setting makes the server refuse them.
- A blocked person can still read your public posts, and can be in a voice room
  you do not run. In the game, blocking does not yet hide their figure or name.
- These settings and passes are per server for now; sharing them across
  federated servers comes with the federation work.

---

## Related

- `rights_and_responsibilities.md` states the rights involved in the first two.
- `communication_and_association.md` and `consent_and_control.md` state the third.
- `transparency_guarantees.md` and `minimum_transparency_checklist.md` are why
  this document exists rather than the gaps sitting unrecorded.
- `docs/reference/voting_integrity_constraints.md` covers the coercion problem
  the second gap creates.
