# Federation Activation — design

> **Status:** design (2026-05-20). Operator chose "plan activation" for TIER 1 #4. Federation code is fully implemented but **dormant** (zero peers). This doc is the vetting + abuse model + phased plan to take it live SAFELY. No code changes yet — design first, per the operator's call.

## Why federation exists

HumanityOS's mission is civilization-scale + no-single-point-of-control. Federation lets independent relays (run by different people/communities) interconnect so a user on one server can chat in shared channels + have their signed profile/objects replicate across servers. No home server; latest-timestamp-wins; any server caches. This is the multi-server future the architecture was built for — it just hasn't been switched on because one relay + zero peers needs no federation.

## Current state (what's already built)

**Storage** (`federated_servers` table): `server_id, name, url, public_key, trust_tier (default 0), accord_compliant, status, last_seen, added_at`. Managed via `add_federated_server` / `set_server_trust_tier` / `list_federated_servers`.

**Trust tiers** gate everything:
- `trust_tier 0` (default) — untrusted. Not connected to; inbound hellos rejected.
- `trust_tier < 2` — skipped by `start_federation_connections`.
- `trust_tier >= 2` — trusted: we make outbound connections to it AND accept its inbound `FederationHello`.

**Outbound** (`federation.rs::start_federation_connections` + `federation_connect_loop`): for each peer with trust_tier >= 2, open a persistent WS with exponential backoff. On connect, send a `FederationHello` signed with our server's Ed25519 key.

**Inbound** (`handle_federation_hello`): accept a hello ONLY from a server already in `federated_servers` at trust_tier >= 2, with a fresh timestamp (±5 min, replay-guarded) and a valid Ed25519 signature over that timestamp. Reply with `FederationWelcome` listing our federated channels.

**What flows once connected:**
- `FederatedChat` — chat in *federated channels only* (`is_channel_federated`), rate-limited 10 msg/s per peer.
- `ProfileGossip` — signed profiles; **Dilithium3-verified** since v0.276.0 (`verify_profile_signature`). Bad sig from a signed client = rejected.
- `SignedObjectGossip` — generic PQ-signed objects; `put_signed_object` verifies the Dilithium signature; multi-hop re-gossip to other peers (excluding source), rate-limited 50/s per peer; `INSERT OR IGNORE` breaks cycles.

**Key fact:** federation is **fail-closed today.** An unknown server cannot federate without the operator explicitly adding it AND raising its trust_tier to 2. So "dormant" is already safe — the risk is only realized when the operator deliberately trusts a peer. Good foundation.

## The trust model — how a peer earns trust_tier 2

This is the human process the code assumes but doesn't enforce. Proposed:

1. **Out-of-band introduction.** A prospective peer operator contacts you (not via the relay). You learn who they are + their server URL + their server public key.
2. **Accord agreement.** They affirm the Humanity Accord (esp. the altruistic-benevolence + no-prompt-injection articles). The `accord_compliant` flag records this. A peer that won't agree doesn't get trusted.
3. **Reciprocity.** Federation is bidirectional — both servers must add each other at trust_tier 2. Document the exact key exchange (each operator gives the other their server_id + public_key + url).
4. **Staged trust.** Add at trust_tier 1 first (present but not connected) to record the relationship, observe, then raise to 2 to activate. (Today only >=2 is meaningful; tier 1 is a documented "known but not yet activated" holding state.)
5. **Revocation is instant.** `set_server_trust_tier(server_id, 0)` + restart drops them (see INCIDENT-PLAYBOOK "A federated peer turns hostile"). This must stay a one-command kill switch.

## Abuse model — what a trusted peer can do, and our defenses

A peer is only as trustworthy as its operator; assume any peer *could* turn hostile or be compromised. Per-vector:

| Vector | What a hostile peer could do | Current defense | Gap |
|--------|------------------------------|-----------------|-----|
| Chat flood | Spam federated channels | 10 msg/s/peer rate limit | OK |
| Profile gossip flood | Spam profile updates → DB write storm | Dilithium sig verify | **No per-peer rate limit** (flagged in INCIDENT-PLAYBOOK). Phase 2 fix. |
| Signed-object flood | Spam objects + amplify via multi-hop | 50/s/peer + Dilithium verify + INSERT-OR-IGNORE dedup | OK-ish; the 50/s could still be a lot of DB churn — consider lowering for untested peers |
| Forged identity | Impersonate a user from their server | Profiles/objects are Dilithium-signed by the USER's key; a peer can't forge a signature it doesn't hold | OK (the user's PQ key is the root of trust, not the peer's) |
| Malicious content | Push illegal/abusive content into shared channels | Moderation tools (ban/mute/delete) work on federated messages? **UNVERIFIED** — needs testing | **Verify** moderation reaches federated content; if not, that's a Phase 3 blocker |
| Hello replay | Replay an old hello to spoof presence | ±5 min freshness window | OK |
| Metadata leak | A peer learns who's in shared channels, message timing | Inherent to federation; DMs are NOT federated (E2EE, server-mediated only) | Acceptable — document it |

**The biggest unknowns** (must resolve before trusting a real third-party peer): (a) does moderation (ban/mute/delete) propagate to or apply against federated content? (b) profile-gossip per-peer rate limit. Both are testable/fixable before Phase 4.

## Activation phases

### Phase 1 — federated-server admin UI + channel-federation toggle
Today, adding/trusting a peer is a raw storage call (no UI) and there's no surfaced way to mark a channel federated. Build:
- Server Settings → Federation panel: list peers (name, url, trust_tier, status, accord_compliant), add a peer, raise/lower trust_tier, **one-click defederate** (set tier 0 + drop connection without a full restart).
- A per-channel "federated" toggle (writes `is_channel_federated`).
- Native (`src/gui/pages/server_settings.rs`) + web parity.

### Phase 2 — close the profile-gossip flood gap
Add a per-peer profile-gossip rate limit (mirror the existing `federation_rate` map used for chat + objects). Cheap; removes the one clear DoS vector a trusted-but-compromised peer has.

### Phase 3 — first real peer (operator-controlled), end-to-end verify
Stand up a SECOND relay the operator controls (a cheap second VPS, or a local box). Federate the two at trust_tier 2. Verify:
- Chat in a federated channel appears on both.
- A profile set on A appears on B.
- **Moderation: a ban/mute/delete on A — does it affect the federated view on B?** (This is the load-bearing test.)
- Defederate via the Phase 1 UI; confirm clean disconnect.
- Kill switch: trust_tier 0 + confirm the peer can't reconnect.

### Phase 4 — open to vetted third-party peers
Only after Phases 1–3. Document the operator-facing "how to federate with HumanityOS" process (key exchange, accord agreement, reciprocity). Start with ONE trusted third party; expand slowly.

## Open questions for the operator

1. **Server-to-server hello: keep Ed25519 or move to PQ?** The federation HELLO handshake is signed with the server's Ed25519 key (the last Ed25519 path, deliberately untouched by the user-identity PQ cutover). Federated CONTENT (profiles, objects) is already Dilithium. Moving the hello to PQ is low-priority (it's a server key, not a user identity, and the freshness window limits replay) but worth a decision for full-PQ consistency.
2. **Channel-federation granularity:** federate specific channels only (e.g., a shared #network channel) vs all public channels? Per-channel is safer (limits blast radius); recommend starting there.
3. **Accord enforcement:** is `accord_compliant` a self-attestation (peer says yes) or is there a verification step? For Phase 4, probably self-attestation + revocation-on-violation.
4. **Default trust for the second-VPS test:** the operator's own second box can go straight to tier 2 (Phase 3); third parties start at tier 1 (observed) before tier 2.

## Relationship to current code

Nothing here requires undoing existing code — the dormant implementation is the correct foundation. Phase 1 adds management UI on top of the existing storage API; Phase 2 adds one rate-limit map; Phase 3 is testing; Phase 4 is process + docs. The fail-closed default (trust_tier 0) means we can build + test all of this without exposing the live relay to any untrusted peer until the operator deliberately flips a peer to tier 2.
