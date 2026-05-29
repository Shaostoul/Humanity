# P2P Groups — Design

> **Status:** design / not yet built. Decided 2026-05-27 (operator chose "true P2P
> groups" over relay-mediated or federated-fallback). This doc is the architecture
> + phased plan; **review the "Open decisions" section before Phase 2 code lands.**
>
> **Why:** Today groups are 100% relay-mediated — `handle_group_create/join/msg`
> (`src/relay/handlers/msg_handlers.rs` ~1757–1964) read/write the relay's SQLite
> (`groups` / `group_members` / `group_messages`, `src/relay/storage/mod.rs` ~1122;
> `src/relay/storage/social.rs`) and route every message through the relay. So if
> `united-humanity.us` is down, **create + join + messaging all break**, and the
> invite link 404s. That contradicts the mission's "no single point of failure."
> Goal: a group is **sovereign** — its membership and messages live in signed,
> replicated objects exchanged peer-to-peer; no single relay is required.

## North star

A group is not a row on a server. A group is:

1. a **signed group object** (`group_v1`) — identity = the creator's key, not a relay;
2. an **append-only signed membership log** (`group_member_v1` admit/remove entries, each signed by an admit-authorized key);
3. an **append-only signed message log** (`group_msg_v1` entries, each signed by its author and encrypted to the current group epoch key);

…all replicated between members over **WebRTC DataChannels** (peer-to-peer), with relays acting only as **optional accelerators** (signaling rendezvous, opportunistic cache, presence hints) — never as the source of truth. An invite is a **signed connection ticket**, not a URL.

This is the `signed_objects` + gossip + latest-wins model (`docs/design/storage-architecture.md`) and the append-only-signed-log governance shape (`docs/design/signed_moderation_logs.md`), applied to groups, carried over the P2P transport that already exists for DMs.

## What we reuse (already built — do NOT reinvent)

| Need | Existing primitive | Where |
|------|--------------------|-------|
| P2P transport | `RTCPeerConnection` + ordered DataChannel, queue/flush, fallback | `web/chat/chat-p2p.js` (`initDataChannel`, `bindDataChannel`) |
| N-peer mesh template | voice rooms already full-mesh N peers via the relay | `VoiceRoomSignal` (`src/relay/relay.rs` ~1113 / 5633) |
| Relay-free bootstrap | Dilithium-signed contact card (identity + Kyber pub) via QR/clipboard | `exportContactCard`/`importContactCard` (`chat-p2p.js`) |
| Sovereign data | `signed_objects` table + gossip + INSERT-OR-IGNORE + latest-timestamp-wins | `docs/design/storage-architecture.md`, `federation-activation.md` |
| Governance shape | append-only signed log, space-declared authority keys, deterministic enforcement | `docs/design/signed_moderation_logs.md` |
| Message E2EE | Kyber768 (ML-KEM-768) → BLAKE3-KDF → AES-256-GCM, **dual-seal** to N readers | `src/net/dm_pq.rs`, CLAUDE.md crypto table |
| Log reconciliation | DataChannel sync bundles, newest-wins / array-by-id merge | `applySyncBundle` (`chat-p2p.js` ~528) |
| Author identity / signing | Dilithium3 / ML-DSA-65 from the BIP39 seed | `src/net/identity.rs`, `web/shared/pq-identity.js` |

## What must be built (the real gaps)

1. **Relay-independent signaling / discovery — THE core gap.** Today WebRTC
   offer/answer/ICE ride one relay (`RelayMessage::WebrtcSignal`,
   `relay.rs` ~987/5507) and are unicast by the home relay only. If it is down,
   two not-yet-connected peers cannot negotiate. We need signaling that survives
   a single relay outage.
2. **Group as a signed object + signed membership log** (replacing mutable SQLite rows as the source of truth).
3. **Signed connection-ticket invites** (replacing the 6-hex relay code + the broken `…/chat/group/{id}` URL).
4. **N-member group encryption with key rotation** (today only O(N) per-message KEM exists; no sender-key, no forward secrecy across membership churn).
5. **Signed, E2EE group messages** (today `group_messages.content` is plaintext + unsigned).
6. **Peer store-and-forward for offline members** (today only relay-SQLite history-pull exists).
7. **NAT traversal of last resort** — no TURN server today, so symmetric-NAT peers can't connect even with signaling.

## Architecture

### The group object + logs
- `group_v1` — `{ group_id, name, creator_key, created_at, epoch }`, signed by creator. `group_id = base58(BLAKE3(creator_key ‖ created_at ‖ nonce))` so it's self-certifying and collision-free without a server.
- `group_member_v1` — append-only entries `{ group_id, action: admit|remove, subject_key, by_key, at, epoch }`, each signed by a key that the membership log already authorizes to admit/remove (creator bootstraps as admin). Current roster = deterministic fold of the log (same enforcement model as `signed_moderation_logs.md`).
- `group_msg_v1` — `{ group_id, author_key, ts, epoch, ciphertext }`, signed by author, `ciphertext` = AES-256-GCM under the epoch key. Message id = BLAKE3 of the signed bytes (idempotent dedupe on merge).

All three are `signed_objects` → they gossip + cache anywhere, latest-wins, no relay is authoritative.

### Invite = signed connection ticket
A ticket is a signed blob (QR / copyable string), **not** a relay URL:
```
{ group_id, group_name, admit_key (admin pubkey), epoch,
  bootstrap: [ {member_key, kyber_pub, relays:[...], last_addr_hint} ... ],
  invite_secret, expires_at, uses_remaining }
```
Opening it lets the joiner (a) verify the group object, (b) send a **signed join request** to a bootstrap member, who (if the ticket/secret checks) appends a signed `admit` entry and re-keys the epoch, (c) connect to ≥1 online member to sync. Hardened unlike today's no-expiry 6-hex code (mirror `friend_codes`' `expires_at` + `uses_remaining`, `mod.rs` ~1156).

### Encryption — epoch group key (recommended)
Per-group **epoch key** (random AES-256 key). On any membership change the epoch bumps and the new key is sealed to each current member's Kyber pub (generalize the DM dual-seal in `dm_pq.rs` to N recipients). Messages within an epoch are O(1) to encrypt (one AES key); re-keying is O(N) but only on churn. Gives forward secrecy across membership changes (removed members can't read new epochs). Alternative considered — per-message N-way KEM — is simpler but O(N) per message and no rotation FS; rejected for scale.

### Transport + signaling
- **Live messaging:** full-mesh DataChannels among online members (reuse `chat-p2p.js` + the voice-room mesh pattern).
- **Signaling that survives a relay outage** (the core gap), layered:
  1. **Multi-relay failover** — `webrtc_signal` becomes routable through *any* relay a target advertises (members publish their reachable relays in the bootstrap list / presence), not just the home relay. One relay down ≠ group down.
  2. **Peer-assisted signaling** — once any two members share a DataChannel, that channel relays signaling for *other* members. The group bootstraps connectivity from a single live link; no relay needed once one peer is reachable.
  3. **LAN discovery (later)** — mDNS for same-network peers, fully serverless.
- **NAT last resort:** a TURN fallback (operator-run, or peer-as-TURN for a well-connected member). Without it, symmetric-NAT home users fail to connect P2P — see Open decisions.
- **Relays remain optional accelerators:** opportunistic message-log cache + presence + signaling rendezvous. Always replaceable by peers.

### Offline members
Group message logs reconcile peer-to-peer on connect (DataChannel sync/merge, newest-wins by message id — the `applySyncBundle` pattern). Any member holding history serves a catching-up member. A relay, when up, may also cache+serve as a convenience. No offline member depends on one server being up.

## Phased plan (each phase shippable + testable)

- **Phase 1 — Sovereign data + working invite.** Model the group + membership as signed objects/logs; relay stores them as a *cache of signed objects* (not the truth). Ship the **signed connection-ticket invite** + the **web join flow** (fixes the 404). Transport/signaling still via the relay this phase, so groups *work* again immediately — but the data is now P2P-ready. *Exit:* a group survives being re-hydrated purely from its signed objects.
- **Phase 2 — Signed + E2EE messages.** `group_msg_v1` signed by author + epoch-key AES-GCM. Epoch re-key on membership change (seal to each Kyber pub). *Exit:* relay never sees group plaintext; removed members can't read new epochs. **DONE.**
  - **Multi-epoch history — FIXED v0.311.0.** A message is encrypted under the epoch key current WHEN SENT, so after a re-key the log spans epochs. Clients had fetched only the *latest* key (`/epoch`) and decrypted the whole log with it → pre-re-key messages silently vanished for existing members. Now: relay serves ALL epoch objects (`GET /api/v2/groups/{id}/epochs`, oldest→newest — the relay already retained them in `p2p_group_epochs`); both clients build an `epoch→key` map and decrypt each message under its own epoch's key. An existing member is sealed into every epoch (full history); a later joiner is sealed only from their join epoch on (forward secrecy — secure default; re-sealing prior epochs to new members is a deliberate future opt-in). Locked by `group_e2ee::multi_epoch_history_decrypts_per_epoch` + `groups_p2p::all_epoch_objects_oldest_to_newest`.
- **Phase 3 — P2P transport.** Full-mesh DataChannels for live group messaging; relay drops to signaling-only. *Exit:* messages flow with the relay carrying zero message bytes.
  - **Increment 1 — SHIPPED v0.309.2 (web).** Live group messages broadcast over WebRTC DataChannels to connected roster members, ADDITIVE to the relay POST (relay = cache + offline backfill, not removed). Reuses `chat-p2p.js` transport + `webrtc_signal` (no new relay handler; relay signaling-only). New `pq-object.js verifyObjectSubmission` lets a peer verify a pushed `group_msg_v1` locally (same ML-DSA check the relay runs) + dedup by `object_id` against the poll. Glare-free mesh (larger-pubkey offers). `scripts/object-verify-kat.mjs` locks the verify primitive. **Needs operator two-identity browser test before increment 2.**
  - **Increment 2 (next, after browser-verify):** epoch-key + history sync over the DataChannel (`applySyncBundle` pattern) so a catching-up peer syncs from a connected member, not just the relay; then stop the relay poll while P2P-connected (toward the zero-relay-bytes exit). Native group P2P transport waits on the native-WebRTC arc.
- **Phase 4 — Relay-independence (the payoff).** Multi-relay signaling failover + peer-assisted signaling + TURN/peer-relay fallback + peer store-and-forward. *Exit:* kill the home relay; a group with ≥1 other reachable member still creates/joins/messages. **This is where "united-humanity.us down → groups still work" is true.**
- **Phase 5 — Serverless discovery (later).** mDNS (LAN) + optional DHT for fully relay-free bootstrap.

## Decisions (settled 2026-05-27, operator)

1. **TURN fallback — DECIDED: (a)+(b).** Operator-run TURN server when available, **plus** peer-as-TURN (elect a well-connected member to relay) as fallback. Rationale: the mission is *everyone*, including users behind symmetric/carrier-grade NAT who can't hole-punch; skipping TURN silently locks them out. Not a single point of failure — TURN down only costs the hardest-NAT pairs their fallback, never the whole group. Lands in Phase 4.
2. **Encryption scheme — DECIDED: per-epoch group key.** One shared group key; messages encrypted once (O(1), scales to large groups); re-key on every join/remove (new epoch) so removed members are cut off from future messages (forward secrecy). Rejected per-message N-KEM (simpler but O(N) per message — won't scale to "billions"-sized groups). Lands in Phase 2.
3. **Relay role — DECIDED: optional accelerator.** Relays, when up, cache message history (fast offline catch-up), assist signaling, and show presence; when down, everything still works peer-to-peer. Rejected strictly-peer-only (purest but bad cold-start/catch-up UX across timezones). This is what makes "relay down ≠ group dead" true while keeping good everyday UX. Shapes Phases 3–4.

**Still open (lower stakes, decide during build):**
- **Migration of existing relay-mediated groups** into signed objects (one-time, on Phase 1 deploy) — straightforward; the live DB has the rows to mint `group_v1` + a membership log from. Decide whether to migrate the (currently 1) live group or start fresh post-cutover.

## Security notes
- Admission must verify the ticket secret AND that the admitting key is authorized by the membership log fold — never trust a self-claimed admit.
- Epoch re-key on *remove* is what gives forward secrecy; a removed member retains old-epoch plaintext they already had (unavoidable) but is cut off from new epochs.
- A hostile member can leak group plaintext (inherent to any group E2EE — trust is per-member). Document, don't pretend otherwise.
- Signed messages give non-repudiable authorship; consider whether that's desired vs. deniability (DMs today are not deniable either).

## Anchors (files to touch / reference)
`web/chat/chat-p2p.js` (transport, contact card, sync) · `src/relay/relay.rs` ~987/5507 (signaling, to be made multi-relay) · `src/net/dm_pq.rs` (PQ envelope to generalize) · `src/relay/storage/social.rs` + `mod.rs` ~1122 (current model → signed-object cache) · `src/relay/handlers/msg_handlers.rs` ~1757–1964 (current handlers) · `docs/design/{storage-architecture,signed_moderation_logs,two_timeline_offline_model}.md` (replication / governance / offline frame).
