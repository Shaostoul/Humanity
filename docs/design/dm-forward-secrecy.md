# Forward secrecy and disappearing messages for direct messages

**Status: design (2026-11-20). Nothing is built.** The reset's item 5 (`docs/PRIORITIES.md`, "THE
RESET'S ORDER"). The research behind every choice here, with quotations and dates, is in
[`docs/reference/research/2026-11-20-forward-secrecy/`](../reference/research/2026-11-20-forward-secrecy/README.md);
entry ids like (A4) or (C2) point into its notes files.

**Decided by the operator (2026-11-16)**, after the contributor's post-quantum review
([`2026-11-16-pqc-review`](../reference/research/2026-11-16-pqc-review/README.md)): "Agree with your
recommendations. Best point to do this now is before anyone is using the app." So: forward secrecy,
yes, with an encrypted-backup path for history; the recovery phrase restores the identity, and old
direct messages come back by transfer from another device or from the encrypted backup.
**Disappearing messages** (his own design, [`outbox-and-inbox.md`](outbox-and-inbox.md) section
15.3) are designed in the same pass because both change the sealed message and the DM store: the
format changes once, and the backup must leave expired messages out.

This is the whole plan with all its parts (CLAUDE.md: plan the whole feature, build it in rungs), so
the data shapes and the costs are decided once and the first rung is a rung of the real thing.

## 0. In plain words

Today every private message to you is locked with one key that your recovery phrase can always make
again. That is what lets any device you restore open all your old messages, and it is also the weak
spot: anyone who ever copies the locked messages from a server, and later learns your recovery
phrase, can open every one of them.

With forward secrecy, each message is locked with keys that each device throws away once it has
opened the message, and the keys keep changing as you and the other person write back and forth. A copy of the
locked message taken from a server stays locked forever, even from someone who later gets your
recovery phrase or one of your devices.

The price, which the operator accepted: your recovery phrase still brings back **you** (your
identity, your name, your friends can still reach you), but not your **old messages**. Those come
back from another device of yours (a transfer you approve on that device) or from an encrypted
backup you chose to keep. Messages set to disappear never go into a backup.

Disappearing messages ride in the same change: a timer, set for a whole conversation or for one
message, is sealed inside the signed message, so the server never learns a timer exists, and each
device deletes its own copy when its time comes. Forward secrecy is what makes that meaningful: once
the keys are gone, even the server's leftover locked copy can never be opened.

## 1. What exists today, and what this changes

Read in the code on 2026-11-20:

- **One key per person.** `src/net/dm_pq.rs` seals each DM with ML-KEM-768 to the person's Kyber768
  key, which every device derives from the recovery phrase (BLAKE3 `derive_key("hum/kyber768/v1")`,
  `crates/humanity-protocol/src/pq_crypto.rs`), then BLAKE3 `derive_key("hum/dm-aes/v1")` and
  AES-256-GCM. The key card (`docs/design/key-cards.md`) publishes that key, signed by the person's
  ML-DSA-65 identity. The key cannot rotate (pqc-review, "Checked in the code", item 2).
- **The signed inner** is JSON `{v:2, from, to, ts, text, sig}` with `from` and `to` as whole
  ML-DSA-65 public keys in hex (3,904 characters each) and a 4,412-character base64 signature over
  `hum/dm/v2\nfrom\nto\nts\ntext`, padded to buckets `[256, 1024, 4096, 16384, 32768]`. An ordinary
  short message is about 12.3 KB inside and **about 23.4 KB on the wire** (section 13).
- **One DM is two deposits**: the recipient's copy in their mailbox, and a copy in the sender's own
  mailbox (sent a random 20 s to 2 min later, `src/net/self_mail.rs`) so the sender's other devices
  learn what was sent. Every note (follow, friendship pass, block, choice, task, organisation, file
  offer, household permit) rides the same envelope.
- **The relay's mailbox** (`src/relay/storage/dms.rs`) keeps `(id, to_key, content, received_day)`
  with no sender, up to 131,072 bytes an envelope, for `dm_mailbox_ttl_days` (30 by default).
- **History lives on the device**: `src/net/dm_store.rs` (one AES-256-GCM file per identity and
  server, under BLAKE3 `derive_key("hum/dm-store/v1", seed)`, rewritten whole on each save) and
  `web/chat/chat-dm-store.js` (IndexedDB, AES-GCM under a key from the same seed).
- **Every device is the same device** as far as keys go: "Link New Device" carries the recovery
  phrase itself (`docs/design/unpairable-dms.md`, "What exists today"). Per-device keys are accepted
  in principle (`docs/design/client_side_identity_keys.md`: "Each device has a device key ... that
  can be revoked") but not built.

What this design changes: each device gets its own keys; a DM is sealed through a **session**
between two devices whose keys move forward with every message and every reply; the phrase-derived
Kyber768 key stops being used for DMs; the signed inner gains the timer fields; history moves
between devices only by an approved transfer or an encrypted backup.

## 2. What it protects, and what it does not

**Protects:**

- Locked messages copied from a server (a copy of the database, its backups, a court order, someone
  recording the server's network) can never be opened once the keys for them are deleted, even by
  someone who later learns your recovery phrase or takes one of your devices.
- After a leak of a device's session state alone (not its recovery phrase), the conversation heals
  as soon as both sides have written once more and that device has replaced its prekey: the next keys
  come from new key material the thief never saw (post-compromise security). The prekey is replaced
  weekly, and at once with "Replace this device's keys now" (section 7.4).
- Both against today's computers and future quantum ones: every key exchange uses the hybrid KEM
  (X-Wing: secure "if SHA3 is secure, and either X25519 is secure, or ML-KEM-768 is secure", B17),
  and every handshake is authenticated by an ML-DSA-65 signature. (Signal's own handshake says
  "Authentication in PQXDH is not quantum-secure.", A4; ours does not have that limit.)
- A disappearing message is gone from the server's copy too, in effect: the copy may sit there until
  the mailbox window ends, but nothing that can open it exists once every device it went to has
  opened it.

**Does not protect, and the app says so (section 11):**

- **What a device keeps.** History you keep is readable on an unlocked device, as now. Forward
  secrecy is about keys in transit, not the archive you choose to keep. Timers, the app lock
  (outbox-and-inbox 15.4) and the disk check (15.5) are the tools for that.
- **Someone who has your recovery phrase now.** Every device of yours holds the identity, so whoever
  has the phrase can add a device of their own and receive your new messages. The new-device alert
  (section 7.3) is how you find out; only a new identity ends it. Healing works against a leak of
  session state, never against a leaked phrase.
- **The backup** opens with what locks it (section 8.3).
- **Messages sent before the other device answers** are protected by that device's weekly prekey,
  so they gain forward secrecy only when that prekey is deleted. This is the same limit Ibex's
  analysis names: "forward security can only be achieved after one communication round." (C5). A
  device answers a new session by itself as soon as it next runs (5.2), so for a device used daily
  this lasts hours; the outer bound, for a device used at least weekly with 30-day mailboxes, is
  about 39 days (4.5). One-time prekeys (rung 9) remove it.
- **A message is only as safe as the slowest device it went to.** One message is sealed for each of
  the recipient's devices; a device that has not run since keeps on its disk what opens it until it
  runs again and deletes it. Senders stop including a device 30 days after its card was last
  renewed (4.3), and Settings > Security > Devices shows devices not used for a week with
  **Revoke** beside them (7.4).
- Screenshots, photographs of a screen, and a changed app that ignores timers (the honest line,
  section 9.1).
- What a server sees at the moment you send (your connection and the mailbox you deposit into),
  as today (`docs/design/unpairable-dms.md` is the plan for that).

## 3. The choice, and the runner-up

### 3.1 The chosen design: device sessions with a key-encapsulation ratchet

Called **DM sessions v3** (the sealed message is version 3). It is composed of published parts, each
named with its source:

1. **Signed device cards with weekly-rotated prekeys**, one per device, signed by the person's own
   ML-DSA-65 identity (section 4): the role PQXDH's "signed last-resort pqkem prekey" plays (A1, A2),
   without a pool of one-time keys at first.
2. **A session starts on the first message**, with no reply needed: the sender encapsulates to the
   recipient device's prekey and can send at once, the start signed by the sender's identity; the
   recipient device answers by itself when it next runs, and the session is then fully forward
   secret. This is Ibex's "Unilateral" then "Complete" shape, in which "the responder generates its
   own ephemeral pair and sends the public key back" automatically (C1), done with a KEM instead of
   Diffie-Hellman.
3. **The Double Ratchet's structure** (A5, A6): a root key, a sending and a receiving chain per
   session, a message key per message deleted after use, skipped keys bounded and expiring. The
   asymmetric step is a **key-encapsulation ratchet**: on each change of turn the sender
   encapsulates to the other device's newest ratchet key and sends a fresh ratchet key of its own.
   Alwen, Coretti and Dodis show the Double Ratchet splits into continuous key agreement, forward-
   secure encryption and a PRF-PRNG, with the first built "generically" from public-key encryption
   (B22's abstract). The KEM ratchet here has that shape; the paper's own KEM instantiation could
   not be read (eprint's robots.txt bars its PDF, research README), so the outside reviewer is asked
   to confirm the match.
4. **One session per pair of devices**, as Signal's Sesame (A9) and Apple's PQ3 ("pair-wise sessions
   established between the sending device and each receiving device", C16) do.
5. **A sealed outer layer per recipient device** that hides who sent the message, from which device
   and at which point in the conversation, keeping today's sealed sender.
6. **The KEM is a parameter** named in each device card: X-Wing once the reset's item 3 has chosen
   one draft for both apps, ML-KEM-768 until then.

**Why this one fits the relay mailbox with an offline peer:**

- The relay needs nothing it does not already do in kind: it keeps signed public cards (as it keeps
  key cards) and mailbox envelopes. No ordering service, no group state, no social edges.
- A message can be sent and read with the other person offline for weeks, and loss, reordering and
  duplicates are handled per message (section 5.4).
- A conversation where the other person never replies still gets forward secrecy, bounded by their
  prekey's life, never none.
- It keeps the identity model: the ML-DSA-65 identity derived from the recovery phrase, key-card
  pins, signed messages that can be handed to a server's admins as evidence (`net/report.rs`).
- Bytes: a dense ratchet (fresh KEM material on every change of turn) costs about 2.3 KB a turn, the
  reason Apple amortises it (C16) and SPQR spreads it in chunks (A10). A HumanityOS DM already
  carries a 3,309-byte signature (B24), so the dense ratchet heals every round trip and a v3 message
  is still **smaller than today's** (section 13).

### 3.2 The runner-up: OpenMLS with an X-Wing ciphersuite

OpenMLS is MIT-licensed, implements a published standard (RFC 9420, B1), was audited by SRLabs in
2026 (B12) and builds for WebAssembly (B10). It lost on three counts:

- **The server shape.** MLS needs a Delivery Service "trusted to break ties when two members send a
  Commit message" (B6), which "may be able to determine group membership" (B8), and KeyPackages that
  are "intended to be used only once" (B4). For one-to-one messages that puts a group id, its epochs
  and its members on the relay: a server-side social edge, which the house rules keep off the relay
  (CLAUDE.md, "The removed tables stay removed").
- **The post-quantum parts are drafts.** Cryspen's X-Wing suite was built on "an early draft" and
  has no IANA code point (B13); the standards-track successor is in IESG Last Call (B15), and its
  suites with classical signatures "provide authenticity against classical attacks only." (B15).
- **Size of the change for a two-person conversation**: tree-based group key agreement, proposals
  and commits, for what is, per pair of devices, one ratchet.

It is the right reference for **groups**, whose epoch scheme has no forward secrecy inside an epoch
and whose relay keeps a readable roster today (BUG-261); a later design.

### 3.3 The others, as references

- **Signal's Triple Ratchet with SPQR**: the strongest reference, followed in structure. Not adopted
  as code: `spqr` is not on crates.io (A14), libsignal says "Use outside of Signal is unsupported."
  (A16), and PQXDH authenticates with X25519 identity keys we do not have (A4). Borrowed: keep at most
  two epochs of ratchet state (A7); downgrade allowed nowhere (A10 allows it only in a session's first
  exchange; we have one version); MAX_SKIP and deleting skipped keys after an interval (A5).
- **Threema's Ibex**: the closest in shape (no prekey server, usable before the reply, C1). Not
  adopted: classical Diffie-Hellman, and no post-compromise security inside a session (C5); and
  "Currently, the multi-device protocol comes without Perfect Forward Secrecy (PFS)." (C2), which is
  exactly the combination needed here. Borrowed: counters so loss is detected and told (C1); the
  sender's person decides whether to resend after a device lost its keys (C1); the 2023 analysis's
  advice to bind the protocol version into associated data (C6); QR-authorised device linking with a
  compared code (C2).
- **Rotating signed prekeys plus a BLAKE3 hash chain**: the first half of this design (sections 4 and
  5.2) alone. Without the ratchet it never heals and its forward secrecy is the prekey's life, so it
  is a rung, not the destination.

## 4. Keys and signed objects

### 4.1 The identity (unchanged)

The ML-DSA-65 identity, derived from the recovery phrase, shared by all of a person's devices. It
signs device cards, every message, and the key card. Key-card pinning (`key-cards.md`) stays exactly
as it is: an app trusts a device card only when it is signed by the identity it has pinned.

### 4.2 The device

Each device (each desktop install, each browser profile) creates on first unlock:

- a **device id**: 16 random bytes, never reused;
- a **device secret**: 32 random bytes that lock everything forward-secret on that device (sessions,
  prekey secrets) and the device's DM history store. **It is never derived from the recovery phrase,
  never synced, never in a backup or a transfer.** Desktop: kept in the OS keychain (`keyring`, as
  `crates/humanity-core/src/auto_unlock.rs` already does for its device key), unlocked with the
  identity; website: a non-extractable WebCrypto key in IndexedDB, opened by the same unlock as the
  identity. **The principle: nothing forward-secret is ever locked by a key the recovery phrase can
  make**, or a stolen copy of the files plus the phrase would undo forward secrecy.
- a **prekey**: a KEM key pair (random, not derived), replaced every 7 days while the device is used.

### 4.3 Device cards

A new signed object beside the key card, one per device:

```
{"v":1,"key":K,"dev":D,"kem":M,"spk":P,"spk_id":N,"at":T,"exp":E,"sig":S}
```

- `key`: the identity's ML-DSA-65 public key, lowercase hex (as in the key card).
- `dev`: the device id, base64url without padding (22 characters).
- `kem`: the KEM's name from `data/crypto/kems.json` (a data file so the KEM is a row, not code):
  `xwing` (the draft the reset's item 3 settles on, written into the row) or `mlkem768`.
- `spk`: the prekey's public key, canonical standard base64; `spk_id`: a number that only grows.
- `at`: signed time, Unix ms; `exp`: when it stops being valid, `at` + 30 days by default.
- `sig`: ML-DSA-65 by `key` over `hum/device-card/v1\n{key}\n{dev}\n{kem}\n{spk}\n{spk_id}\n{at}\n{exp}`.

One spelling only (canonical base64, lowercase hex), so the signed words cannot be written two ways.
A **revocation** is `{"v":1,"key":K,"dev":D,"revoked":T,"sig":S}` over
`hum/device-revoke/v1\n{key}\n{dev}\n{revoked}`, signable from any device of the identity.

**What a server keeps** (one row per identity and device, table `device_cards`; the key card's keep
rule, `key_card::keep_rule`, extended):

| A good card arrives | The server |
|---|---|
| nothing held for this device | keeps it |
| newer `at` for this device | replaces the one held |
| not newer | refuses it |
| more than 10 minutes ahead of the server's clock | refuses it (the key card allows a day; a device card is renewed weekly, so a far-future card could lock its device out of renewing) |
| `exp` more than 30 days after `at` | refuses it |
| for a device with a revocation held | refuses it |
| past its `exp` | refuses it; held cards past `exp` are deleted |

A person may have at most **8 listed devices** (a row in `data/crypto/dm_sessions.json`, the
protocol's numbers as data); the server refuses a ninth until one expires or is revoked. A
revocation is kept until the revoked card's `exp` has passed, then deleted.

**Serving and federation**, as key cards: `{"type":"device_cards_request","keys":[...]}` answered to
the asker alone with every current card of each key; `GET /api/device-cards/{key}`; the member lists
carry only `device_cards_at` (the newest `at`), so an app knows when to ask again; passed to linked
servers as `device_card_gossip` through the federation outbox (kind `device_card`), checked and kept
by the same rule on arrival. Asking and answering stay on whatever the owner switches off
(`WS_ALWAYS_ON`, as key cards): checking whom you seal to is a person's own protection.

**Publishing**: each device sends its current card with every sign-in (`identify.device_card`) and
whenever it replaces its prekey. **Each device also reads back the card the server holds for its own
id** at every sign-in and refresh: a card it did not sign itself (another prekey under its id) can
only come from someone holding the identity, and raises the alert of 7.3. **Account export** lists the person's device cards; **erase**
deletes them (and `erase_left_rows` checks the table).

**On the device**, other people's device cards are fetched at each start and refreshed (at most once
a minute, as key cards), never kept across runs: they are public data the server holds, so keeping
them would be a cache (CLAUDE.md's no-caches rule). With the Dev page's "turn off all reuse" switch
(`HUMANITY_NO_REUSE=1`) on, cards are asked for again before every seal.

### 4.4 What happens to the phrase-derived Kyber768 key

It stops being used for DMs and every note riding them. It stays for what must open from the
recovery phrase alone: recovery pieces sealed to trusted people (`settings/recovery_net.rs`), and a
group's epoch key until groups get their own design. The reset's item 4 (KEM keys by index, signed
into the key card) still applies to it: that makes the identity-level key rotatable, while device
prekeys carry the DMs.

### 4.5 When keys are made and deleted

| Key | Made | Deleted |
|---|---|---|
| Prekey secret | every 7 days while the device is used (a new `spk_id`, a new card) | when the longest mailbox window among the device's servers, plus 2 days, has passed since it was replaced (with 30-day mailboxes: 32 days after replacement, so about 39 days old for a device used at least weekly). A device that slept longer first fetches and opens its mail, then replaces its prekey and deletes every prekey secret past that age at once |
| Ratchet secret (one per turn) | each time this device starts a turn | as soon as the other device's next turn has been opened with it |
| Chain key | each message | the moment the next one is derived |
| Message key | each message | after the message opens; a skipped one when the longest mailbox window plus 1 day has passed (section 5.4) |
| Session | first message between two devices | when not the one in use and silent for the longest mailbox window plus 1 day (section 5.5) |
| Device secret | first unlock on the device | when the person removes the device ("Forget this device" in Settings > Security) |

The mailbox window is a server setting (1 to 365 days, `dm_mailbox_ttl_days`); a server with a long
window keeps prekey secrets longer on the devices that use it, and Settings > Security says so
("Your server keeps sealed messages for 90 days, so first messages to you are fully protected after
about 99 days.").

## 5. Sessions

### 5.1 One session per pair of devices

A session joins one device of mine to one device of theirs (or to another device of mine). Its state,
kept in the device's sessions file (section 10):

- `sid`: 16 random bytes chosen by the device that started it;
- the peer's identity and device id;
- `RK`, the root key (32 bytes); `CKs` and `CKr`, the sending and receiving chain keys;
- this device's current ratchet key pair and its turn number `epoch_s`, the peer's newest ratchet
  public key and its turn number `epoch_r`;
- `Ns`, `Nr`, `PN`: message numbers in the current sending chain, the current receiving chain, and
  the previous sending chain;
- skipped message keys, by `(epoch, n)`, each with its expiry;
- for this device's sent messages, `(epoch_s, n) -> message id` for the longest mailbox window plus 1
  day (for the "could not be opened" flow, section 5.6);
- while started (below), the start's `spk_id`, `ct0` and `rpk_A`, repeated in every message until the
  answer arrives.

### 5.2 Starting a session

Device A1 writes to person B for the first time, or to a device of B it has no session with. For each
of B's valid device cards (signed by B's pinned identity, not revoked, not expired), say B1:

1. `(ct0, ss0) = KEM.Encaps(spk_B1)`.
2. A1 makes its first ratchet key pair `(rpk_A, rsk_A)`, turn number 1, and a new `sid`.
3. `transcript = BLAKE3("hum/dm3/start/v1" ‖ A's identity fingerprint ‖ A1 ‖ B's identity fingerprint ‖ B1 ‖ spk_id ‖ ct0 ‖ rpk_A ‖ sid)`.
4. `RK ‖ CKs = BLAKE3 derive_key("hum/dm3/init/v1", ss0 ‖ transcript)`, 64 bytes of output.
5. **The start is signed**: the signed inner of every message sent while the session is started lists
   the transcript hash in its `starts` field (6.3). The sender makes all of an envelope's starts
   first, then signs the inner once, so one signature covers every device's start.
6. Messages go out on `CKs` (5.3). Every message of a started session carries the start (`spk_id`,
   `ct0`, `rpk_A`, `sid`) in its header (6.2), each wrap still with its own fresh outer
   encapsulation, so whichever message arrives first can open the session and none depends on an
   earlier one arriving.

B1 opens the outer layer (6.2), reads A's fingerprint, device, `sid` and the start, uses its prekey
secret for `spk_id`, looks up A's pinned key card (asking the server if needed), makes the same
transcript and keys, opens the message, and **verifies the ML-DSA-65 signature of the inner** (6.3):
`from` must be the identity named in the header, the message must be one this device accepts (next
paragraph), `dev` must be the A1 named in the header, and **the transcript hash must be in
`starts`**. Only a message that passes creates the session;
anything else is dropped. So nobody can make a session in A's name: not a stranger who encapsulates
to B1's prekey, and not someone holding an old message A signed (a server sent it as evidence in a
report, for example) and re-wrapping it as a new start, because that message's `starts` names other
transcripts.

**Whose messages a device accepts**: an inner whose `to` is my identity (a message to me), or whose
`from` is my identity (my own copy, from another device of mine), always with the `starts` check for a
start. A copy of my message to Ann cannot be replayed into a session with one of my devices by Ann,
because she cannot sign a `starts` naming it.

**The automatic answer.** When B1 opens a start from someone B already talks with (a conversation B
has written in, or a friend), it answers by itself after a random 20 seconds to 2 minutes, with an
envelope carrying only a turn of its own (an `[[hum:answer:v1]]` note, never shown). The session is
then **established** without waiting for B to write from B1. For a start from anyone else (a contact
request not yet accepted) there is no automatic answer, because it would tell the sender a device of
B has come online and read the request; the session is established when B replies.

**A started session lives for one prekey.** Until the answer arrives, the session's keys all descend
from `ss0`, so a leak of B1's prekey secret while it still exists would open those messages (2, "Does
not protect"). When A1 sees a newer card for B1 (a new `spk_id`) and its session with B1 is still
started, it starts a new session on the new prekey for its next message, so B1 never has to keep an
old prekey alive for a session that never got its answer.

### 5.3 The ratchet

All derivations are BLAKE3, in the `derive_key` and `keyed_hash` modes the project already uses and
tests on both apps:

- **A change of turn.** When a device sends after opening a message of the other device's newer turn
  (or B1's answer to a start, automatic or written): `(ct, ss) = KEM.Encaps(rpk_peer)`; `RK ‖ CKs = derive_key("hum/dm3/root/v1",
  RK ‖ ss)` (64 bytes); a new ratchet pair `(rpk_self, rsk_self)`, `epoch_s += 1`; `PN = Ns`;
  `Ns = 0`. Every message of that turn carries the same `ct`, `epoch_r` (whose key it was made to)
  and `rpk_self`, so whichever message of the turn arrives first lets the other side take the step.
- **Opening a newer turn.** `ss = KEM.Decaps(rsk for epoch_r, ct)`; the same `derive_key` gives `RK`
  and `CKr`; the peer's `rpk` is stored for this device's next turn; the ratchet secret just used is
  deleted. Messages of the previous receiving chain still due are kept as skipped keys first.
- **The chains.** For each message: `MK = keyed_hash(CK, 0x01)`, `CK = keyed_hash(CK, 0x02)`; the
  old `CK` is deleted at once.
- **The message key** gives the AES-256-GCM key and nonce: `derive_key("hum/dm3/msg/v1", MK)`, 44
  bytes (32 key, 12 nonce); it wraps the message's content key (6.1) with the header as associated
  data, and is deleted after use.

The protocol version (3), `kem`, `sid` and both device ids are in the associated data of every
seal, so nothing can be moved between versions or sessions (C6's recommendation).

### 5.4 Out of order, lost, duplicated

- A message ahead of the next expected number makes the keys in between **skipped keys**. Signal's
  specification bounds them per chain (`MAX_SKIP`) and suggests a cap per session, "e.g. 1000" (A5);
  ours are 1,000 per chain and 2,000 per session. Each expires when the longest mailbox window among
  the device's servers, plus 1 day, has passed since it was made (anything later can no longer
  arrive); a message that would exceed the limits is treated as unopenable (5.6). The numbers are
  rows in `data/crypto/dm_sessions.json`.
- A wrap whose `(sid, epoch, n)` has already been opened is a **duplicate** (the same message fetched
  from two servers, or replayed by a hostile one) and is dropped silently, never shown as an error.
- Today's dedupe by the signature's hash (`DmInner::dedupe_key`) stays as the message id.

### 5.5 Session states and races

| State | Meaning | What the person sees |
|---|---|---|
| none | no session with that device | nothing |
| started | I wrote first; that device has not answered yet (5.2) | the "not answered yet" sentence in the conversation's privacy panel (section 11) |
| established | that device has answered or written | the plain sentence about forward secrecy |
| broken | a message in it could not be opened (5.6) | the notice of 5.6, on both sides |

**Both start at once.** A device may hold up to 4 sessions with one peer device. It sends on the one
on which it most recently opened a message from that device; when two sessions have both had a
message opened, it sends on the one with the smaller `sid`. Both sides apply the same rule, so they
settle on the same session as soon as each has heard on both. Messages that arrive on any held
session still open. A session not in use and silent for the longest mailbox window plus 1 day is
deleted. (Because every start is signed, 5.2, nobody else can plant a session with a small `sid` to
win this rule.)

### 5.6 When a message cannot be opened

A device that loses its sessions (a reinstall that kept the device id, a restored disk image) or
gets a message past the skip limits cannot open it. When the outer layer still opens (the prekey
secret is there), the device knows who sent it and from which device and session, so:

- it says, in the conversation: "A message from Ann could not be opened on this device." (words in
  section 11), and
- it sends Ann's device a `[[hum:reopen:v1]]{"sid":..,"epoch":..,"n":..}` note in a new session, at
  most once an hour per device, so two broken devices cannot loop;
- Ann's device marks that session broken, starts a new one on the next send, and shows "Ann's
  phone could not open your message from 3:40 PM." with **Send again** (the person decides, as in
  Ibex, C1).

When even the outer layer cannot be opened, the message was for a device that no longer exists (its
prekey secrets are gone with it); there is nothing to tell, and the sender stops sealing to it once
its card expires or is revoked.

### 5.7 One writer per device, and saving before sending

A session's keys must never be used twice. Two browser tabs share one IndexedDB store, so the website
takes a Web Lock (`navigator.locks`) for the identity's sessions: one tab holds it and seals and opens
for every tab of that browser, the others ask it through a `BroadcastChannel`. On both apps the new
session state is saved before an envelope leaves the device, so a crash after sending cannot reuse a
message number or a ratchet key on the next start. (The website has neither today; both come in rung
3.)

## 6. The sealed message, version 3

### 6.1 The envelope

One envelope per mailbox deposit, as today, under the same `dm_put` with the same 131,072-byte cap:

```
{"v":3,"b":BULK,"w":[WRAP,...]}
```

- `b`: the **bulk**: the padded signed inner (6.3) sealed once with AES-256-GCM under a random 32-byte
  content key, associated data `hum/dm3/bulk/v1`: 12-byte nonce, ciphertext, tag; canonical base64.
- `w`: one **wrap** per recipient device, each giving that device the content key (6.2); canonical
  base64, in random order.

The **recipient's copy** has a wrap for each of their listed devices; the **sender's own copy** (to
their own mailbox, still 20 s to 2 min later) has a wrap for each of the sender's other devices, and is
not sent at all when there are none. The two copies are sealed separately (different content keys),
so no byte of one appears in the other: identical blobs in two mailboxes would pair sender and
recipient, which BUG-259's fix took apart.

### 6.2 The wrap

Every wrap is **the same length** for a given KEM, whatever it carries, so its size says nothing about
the conversation's state:

```
WRAP = ct_o ‖ AES-256-GCM(k_o, HEADER ‖ AES-256-GCM(MK, content key))
```

- `ct_o`: a **fresh** outer encapsulation for every wrap, to the recipient device's newest prekey:
  `(ct_o, ss_o) = KEM.Encaps(spk)`, and `k_o` and its nonce from `derive_key("hum/dm3/outer/v1", ss_o
  ‖ ct_o)`. Fresh every time, so no two wraps share a key, a nonce or a byte the relay could match.
  No prekey id travels in the clear: the recipient tries its kept prekey secrets, newest first (a
  device used weekly keeps about six).
- `HEADER`, fixed layout: kind (1 byte: start or message), the sender's identity fingerprint
  (32), sender device id (16), `sid` (16), `spk_id` (4: in a start, the prekey `ct0` was made to),
  `epoch_s` (4), `epoch_r` (4), the `ct` slot (the KEM's ciphertext size: `ct0` in a start, the turn's
  `ct` after the answer), the `rpk` slot (the KEM's public-key size: `rpk_A` in a start, the turn's
  `rpk` after), `n` (4), `PN` (4).
- The content key sealed under the message key, with the header as associated data (48 bytes).

The relay sees only random bytes of one fixed length per wrap. The outer layer's secret is the
prekey, so a leak of a prekey secret while it is kept would show the headers of messages sealed to it
(who sent what, when in the conversation), never their words, which need the session's keys.

### 6.3 The signed inner

Canonical CBOR (the encoding the project's signed objects already use, `cbor_canonical_v1` in
`web/shared/pq-object.js` and its Rust twin), a map:

| Field | What |
|---|---|
| `v` | 3 |
| `from`, `to` | the sender's and recipient's identity fingerprints (BLAKE3 of the public key, 32 bytes each), not whole keys: the recipient already holds the sender's pinned key card |
| `dev` | the sender's device id (must match the wrap's header) |
| `ts` | the sender's time, Unix ms |
| `text` | the message or note text, as today (markers `[[hum:...]]` unchanged) |
| `timer` | absent for a permanent message; else `{"secs": u32, "start": "read" or "sent"}` (section 9) |
| `keep` | absent when the message may be kept; `false` for "can't be kept" |
| `starts` | the transcript hashes (32 bytes each) of every still-started session this message is wrapped for, in the recipient's copy and in my own copy alike (5.2), so one signed inner serves both; absent when there are none. This is what signs a start. At most 15 (8 devices of theirs, 7 of mine): 480 bytes |
| `cert` | the optional friendship pass, as today (self-authenticating, outside the signature) |
| `sig` | ML-DSA-65 by the sender's identity |
| `pad` | zero bytes up to the bucket |

The signature covers `hum/dm/v3\n` followed by the canonical CBOR of the map **without** `sig`,
`cert` and `pad`, **with the whole public keys in place of the two fingerprints**, so a message handed
to a server's admins as evidence (`net/report.rs`) verifies against the keys exactly as today; the
report carries both keys beside it. Timers are inside the signature, so neither the server nor anyone
else can add, remove or shorten one (15.3's "a new field the signature covers").

### 6.4 Padding

Buckets `[4096, 8192, 16384, 32768]` bytes of plaintext (a row in `data/crypto/dm_sessions.json`).
The two smallest of today's buckets go: a signature alone is 3,309 bytes, so no signed message ever
fitted them. A short message is about 3.5 KB inside (about 3.9 KB with a full `starts` list) and
fills the 4,096 bucket (section 13); a household permit (about 20.8 KB today) the 32,768 one.

### 6.5 What the relay sees

| Seen | Not seen |
|---|---|
| the mailbox an envelope is deposited into, and the day | who sent it, from which device |
| the envelope's size: the bucket and the number of wraps (the recipient's device count, which their device cards already show) | the session, the turn, the message number |
| at the moment of sending, which connection deposited it (as today) | whether it carries a timer, a keep mark, or which kind of note it is |
| each person's device cards: how many devices, and roughly when each was last used (its weekly card) | the words |

### 6.6 The KEM as a parameter

Each device card names its KEM; a sender uses the one the recipient device names, for the outer layer
and for that session's turns. Sizes per wrap (section 13) follow from the row in
`data/crypto/kems.json` (public key, ciphertext, shared secret sizes, the draft or standard). Before
v1.0 the apps support exactly one KEM at a time (no negotiation, no fallback); moving from ML-KEM-768
to X-Wing is a release in which every device publishes a new card, and sessions of the old KEM end.

## 7. Several devices

### 7.1 Sending reaches every device

A message to Ann is sealed once and wrapped for each of Ann's listed devices (her copy) and for each
of my other devices (my copy). Each recipient device opens its own wrap through its own session. A
device the sender does not yet know about (Ann added a phone a minute ago) misses messages until the
sender's next card refresh, at most a minute later while the sender is online; filling that gap from
Ann's other devices is rung 8.

### 7.2 My own devices

My devices are ordinary recipients: each pair of my devices has a session, and every note only I read
(blocks, choices, tasks, organisations, my copy of a follow) reaches my other devices through them.
When `docs/design/unpairable-dms.md` route A moves my own copies off the mailbox into fixed-size
segments, a segment carries these same v3 envelopes; that design's "key only your devices hold"
becomes these sessions.

### 7.3 A new device of mine

When a device card for **my** identity appears, every device of mine says so at once, in Chat and on
the Devices page: "A new device signed in as you on 20 Nov at 3:40 PM, through united-humanity.us. If
that was you, there is nothing to do. If it was not, someone knows your recovery phrase." with **What
to do**. No device is ever exempt, not even one I just linked (8.2): a note saying "I added this
device" would be signed by the same identity a thief holds, so it could be forged; the notice simply
says when the device I am holding is the new one. The same alert fires when a device finds the server
holding a card under **its own** id that it did not sign (4.3), which is how a thief would try to
take over an existing device's place without adding one. Today a thief with the phrase is invisible;
device cards make the theft show.

Other people's new devices are not announced in the conversation (browsers that clear their storage
would make it noisy); the conversation's privacy panel lists where messages go ("Your messages to Ann
go to her 2 devices, added 3 Oct and 12 Nov"). (Open question 5.)

### 7.4 A device I lost

Settings > Security > Devices lists my devices (from my cards; labels travel in my own notes, never in
the card), each with **Revoke**. Revoking publishes a revocation, so my contacts' apps stop sealing to
it within a minute of their next refresh. The honest line beside it: "Revoking stops new messages
going to that device. If it is in someone else's hands and unlocked, they also have your recovery
phrase: only a new identity fully ends that." The list marks devices not used for a week ("Last used
12 days ago: it still holds keys for messages sent to it until it runs again"), and each device has
**Replace this device's keys now** (a new prekey at once, every session restarted, the old prekey
deleted a day later once mail already sealed to it has arrived), for when a device's files may have
been copied.

## 8. Moving history: the transfer and the encrypted backup

### 8.1 One archive format for both

An **archive** is the DM store's contents, written as sealed segments under a random 32-byte archive
key: the conversations, the person's lists and choices (follows, friendship passes, blocks, the
choice per friend, contact requests, the scratch pad, file-offer records), the key pins, my tasks and
organisations. Segments (as the Inbox store's, outbox-and-inbox 7.6: each one AES-256-GCM on its own)
let a big history move and verify in pieces.

**Never in an archive:** any message with a timer that has not been kept (section 9.6); a message
marked "can't be kept" that has a timer; sessions, ratchet keys, skipped keys, prekey secrets and the
device secret (restoring them would undo forward secrecy and fork live sessions; Signal backs up no
session state either). Files offered device to device stay where they are; the archive keeps their
records, and the new device can ask for a file again.

### 8.2 Adding a device when I have another one

The new device is linked from an existing one, as today ("Link New Device"), with one addition: the
link also carries a fresh 32-byte **link secret** shown only on the existing device's screen (in the
QR code, or as words to type). Then:

1. The new device gets the identity (as today), makes its device id, device secret and prekey, and
   publishes its card.
2. It sends the existing device a join request sealed under the link secret, carrying its card.
3. The existing device asks: "Send your messages to this new device? Only say yes if you are holding
   it right now." (My other devices still show the new-device notice of 7.3.)
4. On yes, it writes an archive, uploads it to a **transfer slot** on the server (deleted after the
   first download or after 24 hours; at most 256 MB, an owner's setting), and sends the archive key
   and slot to the new device sealed to its new prekey and under the link secret. When both devices
   can open a direct connection (the WebRTC channel file offers use), the archive goes that way and
   nothing is stored; desktop own-device connections are refused today (`src/net/webrtc.rs`,
   `on_offer`), which this rung changes.

Signal's linked devices move history the same way: an archive under a "strong, random, one-time-use
256-bit AES key" uploaded for a limited time, the key going to the new device through the linking
QR's channel (A18). The point of the link secret: **someone with only my recovery phrase can add a
device, but cannot pull my history**, which needs a device I am holding to show the secret and say
yes.

When the new device was restored from the phrase without a link (the old one is in a drawer), it can
still ask: it shows six words, which I type on the old device ("Type the words your new device
shows. Only do this if you are holding both."). The old device then sends the archive as above.

### 8.3 The encrypted backup

For the day there is no other device. An archive, refreshed once a day when anything changed and on
demand, kept on the server(s) I choose (Settings > Security > Message backup; the server I am on by
default) and, if I want, saved as a file (the Files page or a save dialog).

**The lock (open question 1).** Recommended: the recovery phrase **and** a backup password, both
needed. The archive key is sealed under `derive_key("hum/dm-backup/v1", phraseKey ‖
Argon2id(password, salt))`, where `phraseKey = derive_key("hum/dm-backup-phrase/v1", seed)` and the
salt and Argon2id's settings are in the archive's header (Argon2id is already the project's
server-side KDF, `crates/humanity-protocol/src/kdf.rs`; the website gains it in the rung). So:

- a copy of the server's database opens nothing (it would need the 256-bit phrase key and the
  password);
- someone with only my recovery phrase can fetch the backup but must still guess the password, and
  nothing limits their guesses: there is no guard like the hardware vault behind WhatsApp's password
  option, which locks the key after "a limited number of wrong attempts" (C17). So **the app suggests
  the password**: six random words from the recovery-phrase word list (about 66 bits), shown once to
  write down beside the recovery phrase. A person may type their own instead; the screen then shows
  how long it would take to guess and says plainly that a short one protects little against someone
  who has the recovery phrase;
- I need both to restore. A forgotten password loses the backup, never the identity. Changing the
  password (from any unlocked device of mine) makes a **new archive key** and a new archive, so an old
  password and an old copy never open a newer backup.

This is the shape of Threema Safe (a password, a slow KDF, kept on Threema's server or one's own,
C3) and of WhatsApp's password option (C17, minus its guess limit), with the phrase added so the
password alone is never enough. Signal instead uses a separate "64-character recovery key that is
generated on your device" (A17); the suggested six words are the same idea in a form people already
know how to keep, since they keep twenty-four of them.

**On the server**: one opaque blob per backup, in a table `dm_backups (tag, content, updated_day)`
where `tag` is a BLAKE3 keyed hash, under a key from the phrase, of that server's own `did:hum:`, so a
copy of the database does not say whose backup it is and two servers' tags do not match. Writing needs
a write key made the same way per server (the server keeps only its hash), so no server learns a key
that would let it overwrite or delete the copy on another; reading needs the tag. A server watching addresses can still link a restore to the sign-in that follows it
from the same address, which is how the 2023 analysis of Threema linked a Threema Safe restore to
its identity (C10); the restore screen offers to fetch the backup through another of the person's
servers to avoid that. Size: at most 64 MB by default (an owner's setting; about 60,000 messages at 1 KB
each). An owner may switch backups off; the app then says "This server's owner has switched message
backups off. You can keep your backup on another server or save it as a file."

**Restoring** ("Restore from recovery phrase", then "Bring back my messages"): the device computes the
tag, fetches the backup from the servers I name, asks for the backup password, opens it and loads it.
Messages that arrived after the backup was made and were sealed to my old devices cannot be opened;
the app says so.

### 8.4 Restoring from the recovery phrase alone

With no other device and no backup, the identity comes back and the history does not. The app says
it before and after (section 11). Lists and choices come back only from a transfer or the backup,
except the block list (open question 4: recommended, it is also kept in the existing settings sync
under a key from the phrase, because losing it would expose someone again to a person they blocked).

### 8.5 What this resolves elsewhere

- `docs/design/device-mesh.md` open question 1 (backup key: from the phrase or a separate
  passphrase) gets the same answer for its message part: both.
- `docs/design/client_side_identity_keys.md` follow-up tasks ("Define device enrollment and
  revocation flows", "Define encrypted backup format and recovery steps") are sections 4, 7 and 8.

## 9. Disappearing messages

The operator's design (outbox-and-inbox 15.3), made concrete. His words: "I feel like we should
default to permanent but, provide an always present option to enable disappearing messages of
various rates, senders. Like maybe we want only 1 message to be timed. Maybe we want certain timed
messages to remain permanent."

### 9.1 Permanent unless someone sets a timer

Every message is permanent unless a timer is set. The timer control is always visible: a clock in the
conversation's header (the conversation's setting) and a clock beside Send (the next message only).
The first time a person sets one, the honest line, word for word from 15.3: "A timer is a courtesy,
not a guarantee. Anyone can photograph their screen, and a changed app can ignore timers."

### 9.2 Whose messages: mine, theirs, or both

- **My messages**: my outgoing messages carry `timer` in their signed inner; honest apps delete them
  when the time comes, on every device that holds them, mine included.
- **Theirs**: my devices delete their messages after the time. Nothing in their messages changes; it
  is my choice about my copies.
- **Both**.

A change is sent as a signed note to the other person and to my devices,
`[[hum:timer:v1]]{"mine":S,"theirs":S,"start":"read"}` (S in seconds, 0 for permanent), so the
conversation shows everyone's choice ("Ana's messages disappear 1 day after you read them. Ana's
device deletes your messages 1 hour after she reads them."). The clock beside Send sets `timer` on one
message, overriding the conversation's setting either way (one timed message in a permanent
conversation, or one permanent message in a timed one).

### 9.3 When the clock starts

Each device starts the clock for its own copy: **when read** (shown on screen in the open
conversation while the app is in front) by default, or **when sent** (the sender's time `ts`) as a
choice per timer (outbox-and-inbox open question 13, recommendation unchanged). On the sender's own
devices a message counts as read when sent on the device that sent it, and when shown on the others.
Signal does the same for recipients: "the clock starts ticking for each recipient once they've read
their copy" (A19). A message never read keeps waiting; a message to someone away for a week does not
vanish unread.

### 9.4 Keeping a timed message, and "can't be kept"

- **Keep**: either person may keep a timed message permanently on their own devices. Keeping sends a
  sealed note `[[hum:kept:v1]]{"id":MESSAGE_ID}` to the sender and to my devices; the sender sees "Ana
  kept this message." (`MESSAGE_ID` is the signature hash, today's dedupe key.)
- **Can't be kept**: the sender may mark a message `keep: false`; honest apps then offer no Keep, and
  the message shows "Ana asked that this message not be kept."
- **Forget**: anyone can delete any message from their own devices at once, timed or not, as today.

### 9.5 Deleting

When a copy's time comes, the device removes the message from its DM store and saves (the desktop
store is rewritten whole, so the words leave the file; the website deletes the record). The session's
key for it was already deleted when it was opened, so once every device it went to has opened it, the
server's leftover locked copy can never be opened (2, "A message is only as safe as the slowest device
it went to"). Notices for timed messages show no words ("New message from Ana"), so the words do
not outlive the message in the system's notification list. What the app cannot promise, it says (the
honest line; and "Deleted data can sometimes be recovered from a disk; turn on disk encryption",
outbox-and-inbox 15.5).

### 9.6 Timers, transfers and backups

A message with a timer is never written into an archive, neither for a transfer nor for a backup,
unless it was kept. Signal's backups likewise leave out "messages scheduled to disappear within the
next 24 hours" (A17); ours leave out every timed message, because a backup outlives the timer by
design.

### 9.7 Rates as data, and groups later

`data/chat/message_timers.json`, rows of `{id, secs, start_default, label_key, hint_key}`: after
reading (0 seconds: gone when the conversation is closed after reading), 5 minutes, 1 hour, 1 day,
1 week; more are rows. The `timer` and `keep` fields are written so a group message (`group_msg_v1`)
can carry the same two fields later; groups' own forward secrecy is a separate design (3.2).

## 10. Where everything is kept

| What | Where | Locked by |
|---|---|---|
| Identity (ML-DSA-65), recovery phrase | the vault and auto-unlock, as today | as today |
| Device secret | desktop: OS keychain; website: non-extractable WebCrypto key in IndexedDB | the device's unlock; never synced |
| Prekey secrets, sessions, skipped keys | desktop: one file per identity, `dms/<hash>.sessions`; website: an IndexedDB store | AES-256-GCM under a key from the device secret |
| DM history and lists | desktop `dms/<hash>.dmstore` (per identity and server, as today); website the DM store | a key from the device secret (no longer from the phrase) |
| My device card, other people's cards | the server(s); other people's in memory only (4.3) | public, signed |
| Sealed envelopes | the mailbox, up to its window | the sessions (6) |
| Transfer archive | the server's transfer slot, 24 hours at most | a one-time archive key |
| Backup | the server(s) I choose, or a file | phrase key and backup password (8.3) |

Desktop and website are the same, field for field, held together by shared fixtures (section 15).

## 11. The words on screen

Proposed words; each goes in through `node scripts/i18n-add.js` under its key in the rung that builds
it, every control with its hint, and the jargon marked for its glossary card (`[[forward secrecy]]`).
The glossary's `forward_secrecy` entry, which today speaks only of groups, is rewritten for DMs in
rung 3.

| Key | Words |
|---|---|
| `dm_fs.panel.established` | "Each message is locked with keys that each device deletes once it has opened the message. Someone who copies the locked message from a server and later learns your recovery phrase still cannot open it." |
| `dm_fs.panel.started` | "Ann's device has not answered yet; it does so by itself the next time it runs. Until then, your messages are locked with a key her device replaces every week, and they are fully protected a few weeks after that. From its answer on, every message is." |
| `dm_fs.panel.devices` | "Your messages to Ann go to her {count} devices, added {dates}." |
| `dm_fs.cannot_open` | "A message from Ann could not be opened on this device. This happens when a device loses its keys, for example after a reinstall or cleared browser data. Ann has been asked to send it again." |
| `dm_fs.send_again` | "Ann's {device} could not open your message from {time}." with the button "Send again" |
| `dm_fs.restore.before` | "Your recovery phrase brings back you: your identity, your name, and the people who can reach you. It does not bring back your old messages. Those come from another device of yours or from your message backup." |
| `dm_fs.restore.after` | "Your identity is back. Your old messages are not here: each was locked with keys only your old devices held. Bring them from another device of yours, or from your backup." |
| `dm_fs.new_own_device` | "A new device signed in as you on {date} at {time}, through {server}. If that was you, there is nothing to do. If it was not, someone knows your recovery phrase." with "What to do" |
| `dm_fs.what_to_do` | "Make a new identity, tell the people you talk to, and revoke this device. Revoking stops your contacts' apps sending to it, but whoever has your recovery phrase can add another device, so only a new identity ends it." |
| `dm_fs.join.confirm` | "Send your messages to this new device? Only say yes if you are holding it right now." |
| `dm_fs.join.words` | "Type the words your new device shows. Only do this if you are holding both." |
| `dm_fs.backup.offer` | "Keep an encrypted backup of your messages? Without one, a device you lose takes your messages with it. The backup is locked with your recovery phrase and a backup password: the server holding it cannot open it, and someone who has only your recovery phrase would still have to guess the password. Write down the six words below beside your recovery phrase." |
| `dm_fs.backup.own_password` | "A password you make up protects the backup only as well as it is hard to guess. Someone who has your recovery phrase can try passwords as fast as their computer allows." |
| `dm_fs.idle_device` | "Last used {days} days ago: it still holds keys for messages sent to it until it runs again." |
| `dm_fs.backup.forgot` | "Without the backup password nobody can open the backup, you included. Your identity is not affected." |
| `dm_fs.backup.no_timed` | "Messages set to disappear are never put in a backup." |
| `dm_fs.backup.owner_off` | "This server's owner has switched message backups off. You can keep your backup on another server or save it as a file." |
| `dm_timer.honest` | "A timer is a courtesy, not a guarantee. Anyone can photograph their screen, and a changed app can ignore timers." (15.3, verbatim) |
| `dm_timer.kept` | "Ana kept this message." |
| `dm_timer.cant_keep` | "Ana asked that this message not be kept." |
| `dm_timer.notice` | "New message from Ana" |

Two modes, as every deep system has (CLAUDE.md): the conversation's privacy panel shows the plain
sentence; **Details** shows the protocol version, the KEM, each device's session state and the
fingerprint to compare in person (key-cards.md "Known limits": "not built yet").

## 12. Migration: none

Before v1.0 the format changes outright (CLAUDE.md). At the cutover release:

- v2 envelopes still in mailboxes are not opened by v3 apps and expire with the window;
- the DM store keeps the history already on each device (re-sealed once under the device-secret key,
  in the same release, since the old key came from the phrase); new fields default to permanent;
- `seal_v2`, `open_v2`, `build_signed_inner*`, the v2 buckets and `hum/dm/v2` are deleted, on both
  apps, in the same release; DMs and notes stop using the phrase-derived Kyber768 key.

## 13. The cost per message

Sizes computed from FIPS 203 and 204 and the X-Wing draft (B17, B23, B24) with the envelope layouts
above, for a short text (a 600-character one is the same: both fit the bucket):

| Envelope | Today (v2) | v3, ML-KEM-768 | v3, X-Wing |
|---|---|---|---|
| Inner plaintext | 12,285 B, padded to 16,384 | about 3,450 B (4,050 with 600 characters), padded to 4,096 | the same |
| One wrap | none | 3,509 B (fixed) | 3,605 B (fixed) |
| Recipient with 1 device | 23,385 B | about 10.2 KB | about 10.3 KB |
| Recipient with 2 devices | 23,385 B | about 14.9 KB | about 15.1 KB |
| Recipient with 3 devices | 23,385 B | about 19.6 KB | about 20.0 KB |
| My copy, no other device | 23,385 B | none | none |

So a message between two people with one device each costs **about 10 KB a copy instead of 23 KB**,
even though every message now carries ratchet material: the saving comes from fingerprints in place
of whole keys in the inner (7.7 KB) and binary fields in place of base64. The largest envelope (a
32,768 bucket to 8 devices) is about 82 KB, under the 131,072 cap. (Computed on 2026-11-20 by a small
script from the byte layouts of 6.1 to 6.3 and the sizes in B17, B23 and B24; rung 2 replaces the
estimate with a test that measures real envelopes.)

**Work per message**, in operations (the milliseconds are measured in rung 2 by a benchmark on both
apps, not estimated here):

| | Today | v3 |
|---|---|---|
| Sender | 1 signature; 2 encapsulations (two copies) | 1 signature; 1 encapsulation per wrap (outer layer); per change of turn, per session, 1 more encapsulation and 1 key-pair generation; per new session, 1 more encapsulation (the start) and, from the other device, one automatic answer envelope (about 10 KB) |
| Each receiving device | 1 decapsulation; 1 verification | 1 decapsulation (outer); per change of turn, 1 more; 1 verification |

**Kept on each device**: about 3 KB per session (keys, the peer's newest ratchet key, counters), up to
about 96 KB more for a session holding its full 2,000 skipped keys; a few dozen bytes per prekey
secret. **On the server**: about 10 KB per device card; mailbox use falls by about half per message.

## 14. The two apps: one implementation

The protocol is a state machine with exacting rules (section 5); two implementations of it would
double what the outside review must read and every place they could drift. Recommended (open
question 3): **one Rust implementation** (`crates/humanity-dm/`, a new crate in the workspace, with
its card in `docs/crates/`) used by the desktop app and compiled to WebAssembly for the website.
It also ends the X-Wing draft gap between the apps (RustCrypto `x-wing` follows draft 06, B18, noble
tests against draft 10, B19), because both apps would run the same crate. Costs: the website gains a
build step and a `.wasm` file served beside its scripts, and its Content-Security-Policy must allow
`'wasm-unsafe-eval'` in `script-src` (to be weighed with the reset's item 6b, the script policy).
The runner-up is a JavaScript twin held to the Rust one by shared fixtures, the house pattern for the
key card (`web/shared/key-card.js`).

## 15. Tests, each seen red first, and the outside review

- **The forward-secrecy test**, the one that matters: record a v3 envelope, let the recipient open
  it, then hand an attacker the recovery phrase, the recipient device's entire saved state after the
  read (sessions file, DM store, prekey secrets, the device secret) and the recorded envelope; the
  envelope must not open. **Red first** against today's v2, which opens with the phrase alone.
- **Phrase-only restore** cannot open envelopes recorded before it (red against v2).
- **No session in someone else's name**: a genuine signed message from A to B, re-wrapped by a third
  party as a new start to B's device (and, separately, a copy of my message to Ann re-wrapped by Ann
  to one of my devices), must not create a session. Red first against a build without the `starts`
  check.
- **No reused keys**: two website tabs sending at once, and a crash between sending and saving,
  never produce two wraps under the same `(sid, epoch, n)` (red first without the Web Lock and the
  save-before-send order).
- **A foreign card under my own id** and **a new device of mine** each raise the alert (7.3).
- **Vectors shared by both apps** (`scripts/tests/fixtures/dm-v3/`): a device card's bytes; a whole
  conversation under fixed randomness, three devices a side, with loss, reordering, duplicates and a
  start race, every intermediate key and every wire byte; one set per KEM. Both apps must produce and
  open them byte for byte, so the review of one is a review of both.
- **A simulator** (lib test): two people, up to three devices each, random offline stretches up to the
  mailbox window plus a day (30 and 90-day windows both), loss, reordering and duplicates; every
  message either opens or produces the notice of 5.6; none disappears silently.
- **Timers**: an expired message's words are absent from the saved store's decrypted bytes and from
  any archive (measured in what is produced, never in what the code was told).
- **The relay**: the device-card keep rule, the eight-device limit, the backup and transfer slots,
  erase deleting both.
- **Sizes**: the envelope sizes of section 13, measured on real envelopes.

**The outside review** (the reset's item 6): sections 4 to 6 are written to be reviewed as a
protocol specification. Send them, with the vectors, as soon as rung 2 has them, so findings land
before the cutover rather than after.

## 16. The rungs, in order

Before any of these: the reset's items 1 to 4 (the conformance tests, the wallet's own label, X-Wing
for DMs, KEM keys by index). Each rung lands on both apps, with its tests seen red first and its
words through `i18n-add.js`.

1. **Device cards.** Device id, device secret, weekly prekeys and their deletion schedule; the card,
   its keep rule, serving, gossip, export and erase; Settings > Security > Devices with Revoke, the
   idle-device marks and "Replace this device's keys now"; the new-device alert and the check of the
   card held under the device's own id (7.3). No change to messages yet. The cryptography table gains its row.
2. **The session core.** `crates/humanity-dm/` (or the twin, after question 3): sessions, the ratchet,
   the v3 envelope and inner (timer and keep fields included, so the format changes once), the
   vectors, the simulator, the benchmark. Not wired to anything; the protocol text and vectors go to
   the outside reviewer.
3. **The cutover.** DMs and every note riding them move to v3 on both apps in one release; v2 is
   deleted; the stores are re-sealed under the device secret; the automatic answer; one writer per
   browser (Web Locks) and saving before sending on both apps; the "could not be opened" flow; the
   privacy panel's words; the glossary and `docs/outreach/how_your_privacy_is_built.md` updated; the
   restore screens say what the recovery phrase brings back. **Forward secrecy is live from here.**
4. **Disappearing messages.** The timer controls, `data/chat/message_timers.json`, the timer note,
   reading and deleting, keep and can't-keep, notices without words.
5. **Adding a device with its history.** The link secret, the join request, the archive, the transfer
   slot, the direct connection for own devices on the desktop, the six-word path.
6. **The encrypted backup.** The lock, the server table, daily refresh, the file copy, the restore
   flow, the owner's switch.
7. **The block list kept with the phrase** (if question 4 says so).
8. **Filling gaps between my devices**, and route A of `unpairable-dms.md` carrying my own copies as
   segments.
9. **One-time prekeys**: a pool per device on the server, one handed out per first contact, so a
   first message is fully protected at once (PQXDH's one-time PQ prekeys, A1). The server then learns
   when someone starts a conversation with a person, as key-card requests already show.
10. **Backups by increment** (appended segments instead of a daily whole archive), and dummy wraps to
    a fixed count so the envelope stops showing the recipient's device count.

## 17. Documents this changes when built

`docs/reference/cryptography.md` (the DM E2EE, DM metadata and key-card rows; new rows for device
cards, sessions and the backup), `docs/design/key-cards.md` (what the Kyber768 key still seals),
`docs/design/unpairable-dms.md` (route A carries v3), `docs/design/device-mesh.md` (open question 1),
`docs/design/outbox-and-inbox.md` 15.3 (points here), `docs/outreach/how_your_privacy_is_built.md`,
the glossary's `forward_secrecy`, `docs/FEATURES.md` (a line per rung, when built).

## 18. Questions only the operator can answer

1. **What locks the message backup?** Recommendation: **the recovery phrase and a backup password,
   both needed, with the app suggesting six random words as the password** (8.3). The phrase alone
   would make the phrase bring back all history again, the very thing forward secrecy gave up, and it
   would put more history on the server than the 30-day mailbox holds today. Nothing can limit how
   fast someone with the phrase guesses the password (no hardware vault, C17), which is why the
   suggested six words, not a made-up password, are the default. A separate 64-character backup key
   (Signal's way, A17) is stronger still, but a second, unfamiliar thing to keep.
2. **Is the backup on unless a person says no?** Recommendation: **ask once, at the first unlock after
   the cutover and when a new identity is made, with "Keep a backup" preselected.** Without a backup,
   losing your only device loses your messages; with one, a password must be chosen. Asking, rather
   than switching it on silently, is what makes the password possible.
3. **The website: one Rust implementation compiled to WebAssembly, or a JavaScript twin?**
   Recommendation: **WebAssembly** (section 14): one protocol to review and no draft gap between the
   apps; the cost is a website build step and `'wasm-unsafe-eval'` in its script policy.
4. **Should the block list come back from the recovery phrase alone?** Recommendation: **yes**, kept in
   the existing settings sync under a key from the phrase (8.4). Losing it would let someone you
   blocked reach you again after a restore; the cost is that someone with your phrase could read whom
   you blocked (never your messages).
5. **Should your contacts' new devices be announced in the conversation?** Recommendation: **no**, only
   listed in the conversation's privacy panel (7.3); your own new devices are always announced at once,
   because that is how a stolen recovery phrase shows.

Still open from the operator's own design: **outbox-and-inbox question 13, when a timer starts**
(recommendation unchanged: when read, by default, with "when sent" as a choice per timer).
