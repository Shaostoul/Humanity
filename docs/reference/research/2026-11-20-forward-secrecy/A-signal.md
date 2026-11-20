# A. Signal's protocols as references (read 2026-11-20)

Helper A's notes for the forward-secrecy design: Signal's published specifications, blog posts and
code pages, read as references for forward secrecy in an offline-mailbox messenger. Facts and
short quotations only; no design. Every quotation was copied from the page named in its entry.
Each "Paraphrase" line is my own wording of what the page says.

## robots.txt checks (each fetched on its own, before any other page of that site)

| Site | Checked | What it said about ClaudeBot, Claude-User, anthropic-ai, `*` |
|---|---|---|
| signal.org | 2026-11-20 | HTTP 404: no robots.txt, so no path is disallowed for any agent. |
| github.com | 2026-11-20 | ClaudeBot and Claude-User are in a named group with `Crawl-delay: 1` and the same rules as `*`; anthropic-ai is not named. `*` disallows `/*/tree/`, `/*/raw/`, `/*/*/commits/`, `/gist/`, `/account-login` among others. Repository roots and `/blob/` file pages are not disallowed. |
| crates.io | 2026-11-20 | `User-agent: *` with an empty `Disallow:`: everything allowed. No agent named. |
| docs.rs | 2026-11-20 | `User-agent: *` disallows `/releases` and paths containing `^` or `~`; `/crate/` allowed. No agent named. (No docs.rs page was needed in the end, see A14.) |
| cryspen.com | 2026-11-20 | `User-agent: *` / `Allow: /`. No agent named. |
| support.signal.org | 2026-11-20 | No agent named; `*` disallows `/hc/activity`, sign-in, requests and search paths, not `/hc/*/articles/`. **But the first article fetched returned HTTP 403**, so the site refuses automated reading in practice (see "Sources I could not reach"). |

---

## A-Q1. PQXDH

### A1. PQXDH: revision and what the server holds per device
- URL: https://signal.org/docs/specifications/pqxdh/
- Source date: Revision 3, 2023-05-24, last updated 2024-01-23 (authors Ehren Kret, Rolfe Schmidt)
- Read: 2026-11-20
- Quote: "Bob's signed last-resort pqkem prekey and its identifier" (section 3.2; "pqkem" is italic on the page)
- Paraphrase: Bob publishes to the server his curve identity key IKB, a signed curve prekey with its
  identifier, a signed last-resort PQ KEM prekey with its identifier, a set of one-time curve
  prekeys, and a set of signed one-time PQ KEM prekeys, plus the signatures over the signed and
  one-time PQ prekeys. The KEM is not mandated: the parameter table gives "e.g. Crystals-Kyber-1024"
  and cites a draft of FIPS 203; the curve is "one of curve25519 or curve448".
- Confidence: high

### A2. PQXDH: when one-time prekeys run out
- URL: https://signal.org/docs/specifications/pqxdh/
- Source date: Revision 3, last updated 2024-01-23
- Read: 2026-11-20
- Quote: "the bundle will instead contain Bob's pqkem last-resort signed prekey."
- Paraphrase: with no one-time curve prekey left, the bundle simply lacks one and Alice computes
  SK = KDF(DH1 || DH2 || DH3 || SS), dropping DH4. With no one-time PQ prekey left, the server hands
  out the signed last-resort PQ prekey, which "is only used when one-time pqkem prekeys are not
  available". The spec does not say whether the last-resort key is deleted after use.
- Confidence: high

### A3. PQXDH: deletion, rotation and the asynchronous (offline) setting
- URL: https://signal.org/docs/specifications/pqxdh/
- Source date: Revision 3, last updated 2024-01-23
- Read: 2026-11-20
- Quote: "Eventually, Bob should delete this private key for forward secrecy."
- Paraphrase: the server gives out one one-time prekey "and then delete it". After receiving
  Alice's first message, Bob "deletes the ciphertext and any one-time prekey private key that was
  used". Bob uploads new signed prekeys "at some interval (e.g. once a week or once a month)" and may
  keep the previous private key for a while to handle delayed messages before deleting it. The
  protocol targets the case where Bob "is offline but has published some information to a server",
  and the server can store Alice's messages for Bob to retrieve later.
- Confidence: high

### A4. PQXDH: authentication and the limits the spec names
- URL: https://signal.org/docs/specifications/pqxdh/
- Source date: Revision 3, last updated 2024-01-23
- Read: 2026-11-20
- Quote: "Authentication in PQXDH is not quantum-secure."
- Second quote: "PQXDH is not designed to provide protection against active quantum attackers."
- Paraphrase: Alice verifies Bob's prekey signatures and aborts on failure; identities are bound
  by the DH terms DH1 = DH(IKA, SPKB) and DH2 = DH(EKA, IKB) and by AD = EncodeEC(IKA) ||
  EncodeEC(IKB); without an out-of-band identity-key comparison (for example a QR code) there is no
  cryptographic assurance of who the peer is. The introduction says it still relies on discrete log
  for mutual authentication. Section 4 headings: 4.1 Authentication; 4.2 Protocol replay (a first
  message with no one-time prekey "may be replayed to Bob"); 4.3 Replay and key reuse (later
  protocols "MUST take measures to prevent catastrophic key reuse"); 4.4 Deniability; 4.5
  Signatures (without prekey signatures a malicious server can forge prekeys, a "weak forward
  secrecy" attack); 4.6 Key compromise (identity-key compromise allows impersonation); 4.7 Passive
  quantum adversaries (harvest now, decrypt later is addressed); 4.8 Active quantum adversaries;
  4.9 Server trust (a malicious server can make communication fail); 4.10 Identity binding
  (identity misbinding / unknown key share); 4.11 Risks of weak randomness sources; 4.12 Preventing
  KEM Re-encapsulation Attacks (putting the PQ prekey in AD mitigates it for generic KEMs); 4.13 Key
  Identifiers.
- Confidence: high

## A-Q2. Double Ratchet (revision 4, with SPQR and the Triple Ratchet)

### A5. Double Ratchet: revision, skipped message keys and MAX_SKIP
- URL: https://signal.org/docs/specifications/doubleratchet/
- Source date: Revision 4, 2025-11-04 (Trevor Perrin (editor), Moxie Marlinspike, Rolfe Schmidt (revision 3+))
- Read: 2026-11-20
- Quote: "should delete skipped message keys after an appropriate interval."
- Paraphrase: MKSKIPPED stores skipped message keys indexed by ratchet public key and message
  number. MAX_SKIP is the maximum number of message keys that can be skipped in a single chain, set
  "high enough to tolerate routine lost or delayed messages". Parties should also cap stored
  skipped keys per session ("e.g. 1000"), and deletion can be triggered by a timer or by counting
  events (section 8.4).
- Confidence: high

### A6. Double Ratchet: key deletion, break-in recovery, header encryption
- URL: https://signal.org/docs/specifications/doubleratchet/
- Source date: Revision 4, 2025-11-04
- Read: 2026-11-20
- Quote: "the message key and old chain key can be deleted"
- Paraphrase: forward security is defined as past output keys looking random to someone who learns
  the KDF key; break-in recovery as future output keys looking random after such a compromise, as
  the compromised DH private key "will eventually be replaced with an uncompromised one". Recovery is
  against a passive eavesdropper (section 8.2). Secure deletion itself is "outside the scope of this
  document" (8.1). Section 4 (header encryption) hides which messages belong to which session and
  their order, since plain headers carry the ratchet public key and the (PN, N) counters.
- Confidence: high

### A7. Double Ratchet rev 4: the Sparse Post-Quantum Ratchet and the Triple Ratchet
- URL: https://signal.org/docs/specifications/doubleratchet/
- Source date: Revision 4, 2025-11-04
- Read: 2026-11-20
- Quote: "use a KDF to combine these two message keys into an encryption key."
- Paraphrase: yes, the current revision describes the PQ variant: section 5 "The Sparse
  Post-Quantum Ratchet" (built on a Sparse Continuous Key Agreement, SCKA) and section 6 "The Triple
  Ratchet: combining Secure Messaging protocols for hybrid security". The two ratchets run in
  parallel and each message key is `KDF_HYBRID(ec_mk, pq_mk)` (salt = SCKA key, IKM = EC key,
  section 7.2). Section 7.2 recommends the ML-KEM Braid as the SCKA; the page itself names no ML-KEM
  parameter set. Section 5.7: the state keeps KDF chains for only two epochs, and clearing older
  epochs also clears their skipped keys. Section 8.9: dropped messages can slow the PQ ratchet and
  degrade post-compromise security, which the classical ratchet does not suffer. Section 8.11:
  classical healing gives no protection against harvest-now-decrypt-later. The page does not discuss
  multiple devices.
- Confidence: high

### A8. ML-KEM Braid (the SCKA the Triple Ratchet recommends)
- URL: https://signal.org/docs/specifications/mlkembraid/
- Source date: Revision 1, first published 2025-02-21, last updated 2025-09-26 (Rolfe Schmidt; design by Graeme Connell and Rolfe Schmidt)
- Read: 2026-11-20
- Quote: "as long as fresh messages are delivered"
- Paraphrase: works with ML-KEM-512, -768 or -1024 (examples use 768). A header (seed plus a SHA3-256
  hash of the full encapsulation key) is sent first in erasure-coded chunks; the peer then samples
  the first ciphertext part, and the key vector and that part travel in parallel; the second
  ciphertext part, with a MAC, follows. Recommended code: Reed-Solomon over GF(2^16); with 32-byte
  chunks and ML-KEM-768 that is about 3 + 30 + 36 + 5 messages. Any sufficient set of chunks decodes,
  so loss and reordering are tolerated; messages carry an epoch number. Progress needs both sides to
  keep sending: if Bob never replies, Alice's session never heals. Each epoch is one completed KEM
  exchange; no fixed rate. A MAC failure means abandoning the session. 64-bit epoch counters advised.
- Confidence: high

## A-Q3. Sesame

### A9. Sesame: per-device sessions, device lists, stale devices
- URL: https://signal.org/docs/specifications/sesame/
- Source date: Revision 2, 2017-04-14 (Moxie Marlinspike, Trevor Perrin (editor))
- Read: 2026-11-20
- Quote: "For each old DeviceID, the sending device marks the relevant DeviceRecord as stale"
- Second quote: "With per-user identity keys, all devices under a user share the same key pair"
- Paraphrase: each device keeps an active session (and an ordered list of inactive ones) for each
  other device. Devices ask the server about users and devices; when the server rejects a send
  because the device list changed, the sender marks removed devices stale. A stale record is kept for
  delayed messages and may be deleted once older than MAXLATENCY (the interval after which an
  undelivered message counts as lost) and mailboxes are processed. A message arriving on an inactive
  session reactivates it, except past MAXSEND. The server mailbox holds messages until fetched.
  Retries: a device that cannot decrypt may send an unencrypted retry request. Sesame supports both
  per-user identity keys (one key pair on every device) and per-device keys; it does not say how a
  per-user private key reaches a new device. Section 6 headings: Authentication; Device compromise;
  Protecting server communications; Deleting old data (time thresholds decide when old sessions are
  deleted); Bounded loops and bounded storage; Error handling.
- Confidence: high

## A-Q4. SPQR blog post

### A10. "Signal Protocol and Post-Quantum Ratchets"
- URL: https://signal.org/blog/spqr/
- Source date: 02 Oct 2025 (Graeme Connell, Rolfe Schmidt)
- Read: 2026-11-20
- Quote: "any N chunks can be used to recreate the original message."
- Paraphrase: motivation is bandwidth ("bandwidth is expensive, especially on consumer devices"):
  ML-KEM 768 keys (1184 bytes) and ciphertexts (1088 bytes) dwarf 32-byte ECDH keys, so they are
  erasure-coded into small uniform chunks riding on ordinary messages (also avoiding a size
  signature); about 37 chunks for the key and 34 for the ciphertext. The ML-KEM Braid sends the
  64-byte seed-and-hash first so the peer can start encapsulating early. Both ratchets' keys go into
  a KDF, so an attacker must break both elliptic curve and ML-KEM. Loss and reordering were
  simulated; fixed-schedule key sending is criticised for breaking with long-offline peers; faster
  rotation was found to expose more messages to a single breach. Rollout began with the post
  ("slowly and carefully now"); downgrade is possible only in a session's first exchange and the
  SPQR data is MAC'd; Signal plans to enforce SPQR for all sessions later and archive unprotected
  ones. Verification with ProVerif and hax/F* with Cryspen; the ML-KEM library is libcrux-ml-kem.
- Confidence: high

## A-Q5. The SPQR code

### A11. SparsePostQuantumRatchet README
- URL: https://github.com/signalapp/SparsePostQuantumRatchet
- Source date: no date on page (41 commits; no releases)
- Read: 2026-11-20
- Quote: "This project is licensed under the AGPLv3."
- Paraphrase: "SPQR is a Rust implementation of Signal's post-quantum secure ratchet protocol."
  External contributions accepted with a CLA, including ones Signal's apps will not use directly.
  The crate is machine-verified with hax and F* to be panic free; handwritten ProVerif models of the
  ML-KEM Braid. No statement about use outside Signal, API stability, wasm or no_std.
- Confidence: high

### A12. SPQR Cargo.toml
- URL: https://github.com/signalapp/SparsePostQuantumRatchet/blob/main/Cargo.toml
- Source date: no date on page (main branch as of the read)
- Read: 2026-11-20
- Quote: `license = "AGPL-3.0-only"`
- Paraphrase: package `spqr`, version 1.7.0, edition 2021, rust-version 1.88.0, no `publish = false`.
  Depends on libcrux-ml-kem 0.0.10 with default features off and features `incremental` and
  `mlkem768`, plus hax-lib, hkdf, libcrux-hmac, prost, rand 0.9, sha2 and others.
- Confidence: high

### A13. SPQR src/lib.rs (header and API shape)
- URL: https://github.com/signalapp/SparsePostQuantumRatchet/blob/main/src/lib.rs
- Source date: header says "Copyright 2025 Signal Messenger, LLC"
- Read: 2026-11-20
- Quote: "// SPDX-License-Identifier: AGPL-3.0-only"
- Paraphrase: no `#![no_std]` and no wasm or alloc gating in the visible part (the page was
  truncated at about 1,556 lines, so not fully ruled out). API works on opaque serialized state:
  `initial_state(Params)`, `send(&SerializedState, rng)`, `recv(&SerializedState, &SerializedMessage)`,
  returning epoch secrets / message keys.
- Confidence: medium (truncated view)

### A14. crates.io: is SPQR published?
- URLs: https://crates.io/api/v1/crates/spqr (HTTP 404); https://crates.io/api/v1/crates/libcrux-ml-kem (control, resolved); https://crates.io/api/v1/crates?q=sparse%20post-quantum%20ratchet
- Source date: API responses as of the read
- Read: 2026-11-20
- Quote: "Libcrux ML-KEM & Kyber implementations" (libcrux-ml-kem's description)
- Paraphrase: no crate named `spqr` exists on crates.io (404, while the control crate resolves), and
  the search lists none from github.com/signalapp; the only related hit is a third-party vendored
  copy, `spqr-syft` (repository OpenMined/libsignal-protocol-syft, 0.85.3-beta.5, updated
  2025-12-02), which I did not read further. So docs.rs has no page for SPQR (docs.rs builds only
  published crates; not fetched). Control: libcrux-ml-kem newest 0.0.12, created 2026-11-19,
  Apache-2.0, repository github.com/cryspen/libcrux.
- Confidence: high for "not published as spqr"; medium for docs.rs (inferred)

## A-Q6. Cryspen's verification post

### A15. Cryspen: verifying Signal's SPQR
- URL: https://cryspen.com/post/signal-spqr-verification/
- Source date: October 2, 2025 (Karthikeyan Bhargavan)
- Read: 2026-11-20
- Quote: "These proofs only cover the core of the new SPQR code."
- Paraphrase: Cryspen helped verify the new code "for panic-freedom and some correctness goals"
  using hax with its F* backend, and helped build ProVerif models of the design; both the ProVerif
  models and the hax proofs run in CI. libcrux ML-KEM itself is described as verified for
  panic-freedom, functional correctness and secret independence. Cryspen implemented the new
  incremental ML-KEM API in libcrux at Signal's request. Full verification of Signal's codebase is
  "still some distance away".
- Confidence: high

## A-Q7. libsignal

### A16. libsignal README
- URL: https://github.com/signalapp/libsignal
- Source date: copyright line "Copyright 2020-2026 Signal Messenger, LLC"
- Read: 2026-11-20
- Quote: "Use outside of Signal is unsupported."
- Paraphrase: licensed under the GNU AGPLv3 (the README does not say "only" or "or later"). "All
  APIs and implementations are subject to change without notice"; incompatible changes show in the
  version number on a best-effort basis. Crates listed: libsignal-protocol, signal-crypto,
  device-transfer, attest, zkgroup, zkcredential, poksho, account-keys, usernames, media. The README
  does not say whether they are on crates.io. CLA required for contributions.
- Confidence: high

## A-Q8. Secure backups

### A17. "Introducing Signal Secure Backups"
- URL: https://signal.org/blog/introducing-secure-backups/
- Source date: 08 Sep 2025 (jimio)
- Read: 2026-11-20
- Quote: "excluding view-once messages and messages scheduled to disappear within the next 24 hours"
- Second quote: "64-character recovery key that is generated on your device"
- Paraphrase: text messages are backed up (free tier 100 MiB) with the last 45 days of media; a paid
  plan (US$1.99 a month) extends media up to 100 GB. View-once messages and messages due to
  disappear within 24 hours are left out. The recovery key is never shared with Signal's servers,
  is separate from the PIN, can be regenerated, and if lost "Signal cannot help you recover it".
  Backups are end-to-end encrypted ("Only you can decrypt your backup archive"); media get a second
  per-backup key and padding; archives are stored without a direct link to a payment, using the
  zero-knowledge technology of Signal groups (no mechanism named). The device makes a new archive
  daily, replacing the previous one. The post does not say where archives are physically stored or
  whether settings are included.
- Confidence: high

## A-Q9. History transfer to a newly linked device

### A18. "A Synchronized Start for Linked Devices"
- URL: https://signal.org/blog/a-synchronized-start-for-linked-devices/
- Source date: 27 Jan 2025 (ravi-signal)
- Read: 2026-11-20
- Quote: "strong, random, one-time-use 256-bit AES key"
- Paraphrase: the primary phone compresses its message history into one archive, encrypts it with
  a one-time AES key, and uploads it through Signal's servers as an attachment available only for a
  limited time. The key reaches the new device inside the provisioning message, which is encrypted to
  the Curve25519 public key carried (with a provisioning address) in the QR code the phone scans;
  after decrypting, the new device can "throw away the key forever". Included: messages, stickers,
  call history, group updates, quotes, reactions and receipts; media go as attachment pointers.
  Excluded: "we cannot synchronize media older than 45 days" (attachments are deleted 45 days after
  upload). Signal says it cannot read the archive. Linked devices: Desktop and iPad. The post says
  nothing about disappearing or view-once messages in the transfer.
- Confidence: high

## A-Q10. Disappearing messages

### A19. "Disappearing messages for Signal"
- URL: https://signal.org/blog/disappearing-messages/
- Source date: 11 Oct 2016 (moxie0)
- Read: 2026-11-20
- Quote: "the clock starts ticking for each recipient once they've read their copy"
- Paraphrase: the setting is per conversation and applies to all parties of it; launch range five
  seconds to one week. Not meant for "situations where your contact is your adversary" (a recipient
  can photograph the screen). Says nothing about when the sender's own timer starts.
- Confidence: high

### A20. "Embrace ephemerality with default disappearing messages"
- URL: https://signal.org/blog/disappearing-by-default/
- Source date: 10 August 2021 (jimio)
- Read: 2026-11-20
- Quote: "preconfigure all conversations you initiate with a default timer"
- Paraphrase: the per-account default timer applies to chats you start, not to every conversation;
  custom durations are supported (examples range from 60 seconds to 4 weeks). It does not state when
  timers start, and does not mention backups or linked devices.
- Confidence: high

---

## Sources I could not reach

| URL | Why | What I hoped to get |
|---|---|---|
| https://support.signal.org/hc/en-us/articles/9708267671322-Signal-Secure-Backup | HTTP 403 Forbidden on the first article fetch, though robots.txt allows article paths. Per the rules I did not retry it or other articles on that site, and did not use the site's JSON API or re-hosted copies. | Backup contents (settings, call history), where archives are stored, retention if not refreshed, platforms. |
| https://support.signal.org/hc/en-us/articles/360007320771-Set-and-manage-disappearing-messages | Not attempted: same site refused automated reading (403 above). | When the SENDER's timer starts, current timer range, per-chat default timer details. |
| Signal support articles on device transfer and linked devices (support.signal.org) | Not attempted, same reason. | Whether disappearing messages are excluded from the linked-device history transfer. |

## Open questions

- **Sender-side timer start** for disappearing messages: signal.org's blog posts state only the
  recipient's (after reading). Settled by a person reading the support article above in a browser.
- **Disappearing and view-once messages in the linked-device transfer** (A18 is silent). Settled by
  the support article on linked devices, or by libsignal's message-backup code.
- **Last-resort PQ prekey after use**: PQXDH (A2) does not say whether it is deleted or replaced
  sooner than the signed-prekey interval of "once a week or once a month" (A3). Settled by
  libsignal's prekey handling code or a later PQXDH revision.
- **SPQR on wasm / no_std**: not stated anywhere read (A11, A13); lib.rs was truncated. Settled by
  building it for `wasm32-unknown-unknown` or reading the full source (not done: no code downloads).
- **SPQR distribution**: not on crates.io (A14), so use would mean a git dependency; licence is
  AGPL-3.0-only (A12, A13). Licence compatibility with the project's own licence is for the designer
  to check, not settled here.
- **ML-KEM parameter set in the Triple Ratchet spec**: the Double Ratchet page (A7) names none; the
  Braid allows 512/768/1024 (A8); the blog and the crate use 768 (A10, A12).
- **libsignal licence variant**: the README says "GNU AGPLv3" without "only" or "or later" (A16); a
  Cargo.toml or LICENSE file in that repo would settle it.
- **Where Secure Backups are physically stored** and whether settings are included (A17 silent).
