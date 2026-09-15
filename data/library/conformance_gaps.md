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

Last reviewed: 2026-09-14, from the library documentation audit recorded in
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

## Contact consent cannot be withdrawn

**The Accord says** (Communication and Association, Consent in contact): "A user
must be able to restrict who can contact them" and "A user must be able to close
contact pathways without escalating to moderators." Consent and Control adds that
consent "must be revocable: immediately, without justification, without loss of
essential function", and lists "irreversible lock-in" as a prohibited pattern.

**The software does** issue friendship certificates whose signed preimage is a
domain tag plus the two public keys, and nothing else:
`friend_cert_preimage` in `src/relay/core/pq_crypto.rs`. There is no serial, no
expiry and no nonce, and `verify_friend_cert` is stateless, so it consults no
revocation list. The DM path in `src/relay/handlers/msg_handlers.rs` gates on
mute state, certificate validity, a daily knock budget and the rate limiter;
there is no recipient-side deny list.

**So** a certificate, once issued, grants its holder unlimited direct-message
access permanently. Unfollowing does not close the pathway. The only way to
invalidate a certificate is to abandon the identity key, which is also the
account and the wallet address.

**To close it**, add a serial to the preimage plus a recipient-published
revocation object the relay checks at `dm_put`, or an expiry with renewal, and a
recipient-side block enforced in the DM handler.

---

## Related

- `rights_and_responsibilities.md` states the rights involved in the first two.
- `communication_and_association.md` and `consent_and_control.md` state the third.
- `transparency_guarantees.md` and `minimum_transparency_checklist.md` are why
  this document exists rather than the gaps sitting unrecorded.
- `docs/reference/voting_integrity_constraints.md` covers the coercion problem
  the second gap creates.
