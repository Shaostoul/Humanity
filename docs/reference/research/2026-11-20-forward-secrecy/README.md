# Forward secrecy for direct messages: research (2026-11-20)

**Research date: 2026-11-20** (UTC; the evening of 19 November in Pacific time). The evidence behind
[docs/design/dm-forward-secrecy.md](../../../design/dm-forward-secrecy.md), the plan for forward
secrecy and disappearing messages in HumanityOS direct messages. It answers the first research
question the contributor's post-quantum review set
([2026-11-16-pqc-review](../2026-11-16-pqc-review/README.md), recommendations 2 and 3): **which
forward-secrecy design fits a relay mailbox where the other person is often offline?**

Three research helpers read the sources on 2026-11-20. Each site's robots.txt was read first, in a
fetch of its own; a site that barred Claude or refused automated reading was not read and is listed
under "Sources we could not reach", and no re-hosted copy of a refused page was read. Every entry in
the three notes files carries its link, the source's own date (or "no date on page"), one or two exact
short quotations and the helper's paraphrase, marked as such. Nothing was downloaded except the pages
read (two Threema PDFs among them).

| File | What it covers |
|---|---|
| [A-signal.md](A-signal.md) | A1 to A4 PQXDH (revision 3); A5 to A8 the Double Ratchet (revision 4, with the Sparse Post-Quantum Ratchet and the Triple Ratchet) and the ML-KEM Braid; A9 Sesame (several devices); A10 the SPQR announcement; A11 to A14 the `spqr` code, licence and distribution; A15 Cryspen's verification of SPQR; A16 libsignal; A17 Signal Secure Backups; A18 history transfer to a linked device; A19 and A20 disappearing messages |
| [B-mls-xwing-and-kem-ratchets.md](B-mls-xwing-and-kem-ratchets.md) | B1 to B5 RFC 9420 (MLS); B6 to B9 RFC 9750 (MLS architecture); B10 to B12 OpenMLS and its audit; B13 Cryspen's post-quantum OpenMLS; B14 to B16 the MLS X-Wing drafts and their successor; B17 the X-Wing KEM draft; B18 to B20 the `x-wing` crate and `@noble/post-quantum`; B21 the X-Wing paper; B22 the modular Double Ratchet (Alwen, Coretti, Dodis); B23 and B24 FIPS 203 and 204 sizes; B25 and B26 post-quantum replacements for X3DH |
| [C-threema-apple-and-backups.md](C-threema-apple-and-backups.md) | C1 to C6 Threema's Ibex and multi-device protocol (whitepaper of June 2026, the 2023 formal analysis); C7 to C15 Threema's announcements, the ETH Zurich analysis and Threema's statement, Threema Safe and backups; C16 Apple's iMessage PQ3; C17 WhatsApp's encrypted backups; C18 Briar |

## The short answer

None of the four candidates fits as a drop-in, and the reasons are specific:

- **OpenMLS** (MIT, audited by SRLabs in 2026, B12) is a group protocol whose Delivery Service is
  "trusted to break ties" between Commits (RFC 9750, B6) and can learn group membership (B8). For
  HumanityOS that is a server-side social edge, which the house rules keep off the relay. Its
  X-Wing ciphersuite was added on an early draft and has no IANA code point (B13); the standards-track
  successor is still in IESG Last Call (B15). Right reference for groups later, wrong shape for DMs.
- **Signal's Triple Ratchet with SPQR** is the strongest reference, but `spqr` is not on crates.io
  (A14), libsignal says "Use outside of Signal is unsupported." (A16), and PQXDH's own text says
  "Authentication in PQXDH is not quantum-secure." (A4), because it authenticates through X25519
  identity keys that HumanityOS does not have (its identity is ML-DSA-65). SPQR's sparse, chunked
  design exists to save bandwidth on small messages (A10); a HumanityOS DM already carries a
  3,309-byte signature (B24), so that saving matters far less here.
- **Threema's Ibex** has exactly the right shape (no prekey server; a session is usable at once and
  becomes fully forward secret once the other side replies, C1, C5), but it is classical
  Diffie-Hellman with no post-compromise security inside a session (C5), and Threema's own
  multi-device protocol "comes without Perfect Forward Secrecy (PFS)." (C2): the one combination
  HumanityOS needs, forward secrecy across several devices of one person, is the one Threema has not
  shipped.
- **Rotating signed prekeys plus a hash chain** is a sound first rung but not a destination: without
  a ratchet that mixes in fresh key material when the other side replies, forward secrecy is bounded
  by the prekey's lifetime and there is no healing after a leak of session state.

So the design composes published, analysed parts: the Double Ratchet's structure (A5, A6) built on a
key-encapsulation ratchet (the modular construction of B22), started from signed per-device prekeys
(PQXDH's signed last-resort prekey role, A1 to A3), usable before the reply and upgraded by it (Ibex's
lesson, C1), one session per pair of devices (Sesame A9, PQ3 C16), the KEM a parameter (X-Wing, B17),
and history moved by transfer (Signal A18, Threema C2) or an encrypted backup that leaves disappearing
messages out (Signal A17). It is still a new protocol and needs the outside review the plan already
schedules.

## The candidates side by side

| | Rotating prekeys + hash chain | OpenMLS + X-Wing suite | Signal PQXDH + Triple Ratchet (SPQR) | Threema Ibex |
|---|---|---|---|---|
| Fits a mailbox with an offline peer | Yes | Yes for messages; Commits need tie-breaking by the server (B3, B6) | Yes, with a prekey server (A1, A2) | Yes, no prekey server (C1) |
| What the server must hold | Signed prekeys | KeyPackages, one-time (B4); group ids, epochs; may see membership (B8) | Identity key, signed prekeys, one-time curve and PQ prekeys per device (A1) | Nothing new |
| Forward secrecy | Bounded by prekey lifetime | Yes, between epochs (B2) | Yes, per message (A6) | Yes after one round trip (C5) |
| Healing after a leak of session state | No | Yes, by updates (B1) | Yes (A6, A7) | No, inside a session (C5) |
| Post-quantum | With a PQ KEM | X-Wing suite on draft -02 (B13); signatures classical (B15) | KEM ratchet PQ; authentication classical (A4) | No (classical DH; the analysis recommends PQ work, C6) |
| Several devices | Must be designed | Each device a client (B5, B9) | Per-device sessions (A9) | Disabled with multi-device (C1, C2) |
| Licence | Ours | MIT (B10) | `spqr` AGPL-3.0-only (A12), libsignal AGPLv3 (A16) | Not checked (no public protocol specification found) |
| Maturity, audit | Homemade | RFC 9420 (B1); SRLabs audit 2026 (B12) | Signal production since October 2025 (A10); core of SPQR verified (A15) | Proof of 2023 (C5); implementation not reviewed (C6) |
| Website (WebAssembly) and desktop | Ours either way | Builds for wasm through the `js` feature, built in CI but not tested (B10) | Not stated (A11, A13) | Not applicable |

**Licences, against ours** (AGPL-3.0-or-later, `Cargo.toml`): MIT (OpenMLS), Apache-2.0 OR MIT
(`x-wing`, B18) and MIT (`@noble/post-quantum`, B19) are permissive licences. `spqr` is
AGPL-3.0-only (A12), which the review of 2026-11-16 recorded as compatible with ours. Nothing in the
chosen design depends on `spqr` (it stays a reference), so no licence finding was written; if it is
ever adopted, a dated finding under `docs/reference/findings/` comes first, quoting both licences.

## What is still unknown, and what would settle it

- **Which X-Wing drafts interoperate.** `x-wing` follows draft 06 (B18), noble tests against draft
  10 (B19), OpenMLS's suite used draft 02 (B13), the draft is at 11 with its test vectors still
  marked TODO (B17). Settled by the reset's item 3 (X-Wing for DMs): one draft chosen for both apps,
  proven by the same vectors on both. The forward-secrecy design takes the KEM as a parameter so it
  does not wait on this.
- **The KEM-based continuous key agreement in Alwen, Coretti, Dodis** (B22): the abstract says it is
  built "from public-key encryption"; the construction is in the paper body, whose PDF eprint's
  robots.txt bars. Settled by the EUROCRYPT 2019 proceedings version on a publisher page that allows
  reading, or by the outside reviewer.
- **When the sender's own disappearing-message timer starts in Signal** (A19 covers the recipient
  only). Settled by a person reading Signal's support article in a browser (the site refused us).
- **Whether Signal's linked-device transfer leaves disappearing messages out** (A18 is silent).
  Settled the same way, or by libsignal's message-backup code.
- **Ibex's message names and its rule when both sides start a session at once** (C2). Threema
  publishes no protocol specification we could find; settled by its Android or iOS source files
  (single files are readable under GitHub's robots.txt when the path is known).
- **The scope of the SRLabs OpenMLS audit** (B12): the report PDF, not opened.
- **Whether `spqr` builds for WebAssembly**: not stated (A11, A13); settled only by building it,
  which needs a code download and is not needed for the chosen design.

## Sources we could not reach

Each helper's file lists its own with what it hoped to get: support.signal.org (403 on the first
article, so no other article there was tried; A); npmjs.com (robots.txt answered 403) and
cdn.jsdelivr.net (robots.txt answered 400), so noble was read from its author's repository (B); the
PDF bodies of eprint 2018/1037 and 2024/039 (robots.txt disallows PDFs; abstracts read; B);
whatsapp.com (robots.txt bars ClaudeBot; C17 came from Meta's engineering blog instead); and
code.briarproject.org (403; C18).

## What it means for this project

**Certain** (each from a quoted source):

- Signal's post-quantum handshake authenticates classically (A4); an ML-DSA-65 signature over the
  handshake, which HumanityOS already makes on every DM, does not have that limit.
- An MLS Delivery Service breaks ties between Commits and may learn membership (B6, B8).
- Threema runs forward secrecy only with multi-device switched off (C1, C2).
- Apple re-keys with a post-quantum KEM about every 50 messages and at least every 7 days because
  the KEM adds "more than 2 kilobytes" a message (C16); each device has its own pair-wise sessions.
- Signal's backup leaves out messages due to disappear within 24 hours and uses a 64-character
  recovery key made on the device (A17); its linked-device transfer sends an archive under a
  one-time 256-bit AES key (A18).
- X-Wing is secure "if SHA3 is secure, and either X25519 is secure, or ML-KEM-768 is secure" (B17);
  its public key is 1,216 bytes and its ciphertext 1,120 (B17).

**Judgement calls** (the design's reasoning, open to rejection):

- That a dense key-encapsulation ratchet (fresh KEM material on every change of turn) suits
  HumanityOS better than a sparse one, because the 3,309-byte signature already in every message
  makes 2.3 KB more per turn affordable.
- That per-device sessions with signed device cards, rather than a shared device-group key, are the
  way to keep forward secrecy across one person's several devices (Threema's device-group key is the
  part it could not combine with Ibex).
- That composing published parts and paying for an outside review is safer than adopting a library
  whose server model or identity model does not match.
