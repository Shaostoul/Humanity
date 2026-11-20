# B. MLS, OpenMLS, X-Wing and KEM-based ratchets (read 2026-11-20)

Helper B's notes for the forward-secrecy design: the MLS standards (RFC 9420, RFC 9750), the OpenMLS
library, the X-Wing hybrid KEM (specification, paper, Rust and JavaScript implementations), the
MLS post-quantum cipher-suite drafts, the modular Double Ratchet paper, the NIST sizes, and two
post-quantum X3DH replacements. Facts and short quotations only; no design. Every quotation was
copied from the page named in its entry (for the two FIPS PDFs, from the text extracted from the
PDF the page links to). Each "Paraphrase" line is my own wording of what the page says.

## robots.txt checks (each fetched on its own, before any other page of that site)

| Site | Checked | What it said about ClaudeBot, Claude-User, anthropic-ai, `*` |
|---|---|---|
| www.rfc-editor.org | 2026-11-20 | Only `User-agent: *`, disallowing `/rfc/authors/`, `/rfc/rerendered/`, `/rfc/v3test/`. No agent named; `/rfc/rfc9420.html` and `/rfc/rfc9750.html` allowed. |
| datatracker.ietf.org | 2026-11-20 | Only `User-agent: *` with `Disallow: /doc/pdf/`. No agent named; `/doc/<name>/` and `/doc/html/` allowed. |
| github.com | 2026-11-20 | ClaudeBot and Claude-User are in a named group with `Crawl-delay: 1` and the same rules as `*`; anthropic-ai not named. Disallows include `/*/tree/`, `/*/raw/`, `/*/*/commits/`, `/*/*/tags$` (and `tags?*`, `tags/*`). Repository roots and `/releases` are not disallowed (re-checked the robots file alone for `/releases`). |
| cryspen.com | 2026-11-20 | `User-agent: *` / `Allow: /`. No agent named. |
| eprint.iacr.org | 2026-11-20 | No active group names ClaudeBot, Claude-User or anthropic-ai (a commented-out block mentions ClaudeBot and Claude-User). `*` disallows the PDFs under the year paths and `/archive`, `/versions`, `/submit`, `/search`; HTML pages under the year paths are explicitly allowed. So only abstract pages were read, never a PDF. |
| csrc.nist.gov | 2026-11-20 | `ClaudeBot: Allow: /`. Claude-User and anthropic-ai fall under `*`, which disallows only image folders and search/keyword paths. `/pubs/fips/...` allowed. |
| nvlpubs.nist.gov | 2026-11-20 | HTTP 404: no robots.txt, so no path is disallowed. (The FIPS PDFs the csrc pages link to live here.) |
| docs.rs | 2026-11-20 | `User-agent: *` disallows `/releases` and paths containing `^`, `%5E` or `~`. No agent named; `/x-wing/latest/x_wing/` allowed. |
| crates.io | 2026-11-20 | `User-agent: *` with an empty `Disallow:`: everything allowed. No agent named. (Read through its `/api/v1/crates/<name>` JSON.) |
| book.openmls.tech | 2026-11-20 | HTTP 404: no robots.txt. |
| blog.openmls.tech | 2026-11-20 | HTTP 404: no robots.txt. |
| www.npmjs.com | 2026-11-20 | **HTTP 403 on robots.txt itself**: the site refuses automated reading. Not used; see "Sources I could not reach". |
| cdn.jsdelivr.net | 2026-11-20 | **HTTP 400 on robots.txt**: no usable answer. Not used (the package's README was read from the author's GitHub repository instead, which is the original, not a re-host). |

---

## B-Q1. RFC 9420, the MLS protocol

### B1. RFC 9420: status, date and the security claim
- URL: https://www.rfc-editor.org/rfc/rfc9420.html
- Source date: July 2023 (Standards Track)
- Read: 2026-11-20
- Quote: "efficient asynchronous group key establishment with forward secrecy (FS) and post-compromise security (PCS)" (Abstract)
- Paraphrase: an Internet Standards Track RFC. Section 3 sets the trust model: a trusted
  Authentication Service and a "largely untrusted" Delivery Service (DS), trusted mainly to deliver
  messages. In section 3.2 a member who sends a Commit updates its own state only after the DS
  returns that Commit to it.
- Confidence: high

### B2. RFC 9420 section 16.6: how forward secrecy and PCS are obtained
- URL: https://www.rfc-editor.org/rfc/rfc9420.html (section 16.6)
- Source date: July 2023
- Read: 2026-11-20
- Quote: "provided by deleting private keys from past versions of the ratchet tree" (the sentence begins "Forward secrecy between epochs is")
- Second quote (essential): "There is thus a risk to forward secrecy as long as any member has not deleted these keys."
- Paraphrase: forward secrecy between epochs comes from deleting old ratchet-tree private keys;
  forward secrecy inside an epoch comes from deleting each message key after use.
  Post-compromise security comes from members regularly updating their leaf key, and takes
  effect when members process the Commit, not when it is created.
- Confidence: high

### B3. RFC 9420 section 14: ordering of Commits
- URL: https://www.rfc-editor.org/rfc/rfc9420.html (section 14, "Sequencing of State Changes")
- Source date: July 2023
- Read: 2026-11-20
- Quote: "MUST have an established way to resolve conflicting Commit messages for the same epoch" (subject: "Applications")
- Paraphrase: the protocol itself does not make the DS the orderer; it leaves conflict resolution to
  the application, which must also keep forked or old group states briefly and delete them
  promptly. Section 16.9 (per the reading) notes that if the DS resolves conflicts, it can
  influence which Commit is applied. Compare B6 (RFC 9750 puts tie-breaking on the DS).
- Confidence: high (section 14 text); medium for the 16.9 paraphrase (no quotation taken)

### B4. RFC 9420 sections 10 and 16.8: KeyPackages are one-time
- URL: https://www.rfc-editor.org/rfc/rfc9420.html (sections 10 and 16.8)
- Source date: July 2023
- Read: 2026-11-20
- Quote: "KeyPackages are intended to be used only once and SHOULD NOT be reused" (section 10; it continues "except in the case of a "last resort" KeyPackage")
- Second quote (essential): "An application MAY allow for reuse of a "last resort" KeyPackage" (section 16.8, "in order to prevent denial-of-service attacks")
- Paraphrase: a client publishes KeyPackages through the DS so others can add it to groups
  (section 3). Section 16.8 ties reuse to replay attacks, not to forward secrecy directly;
  section 16.6 separately warns that adding a member through an old KeyPackage whose private key
  was compromised reuses compromised key material.
- Confidence: high

### B5. RFC 9420: several devices of one person
- URL: https://www.rfc-editor.org/rfc/rfc9420.html (sections 2, 4.2, 5.3.3, 16.10)
- Source date: July 2023
- Read: 2026-11-20
- Quote: "if a user has multiple devices that are all present in an MLS group" (section 5.3.3)
- Paraphrase: a client is defined by the keys it holds (section 2), each member occupies its own
  leaf (section 4.2) and is identified internally by its leaf index (section 5.3.3); a user's
  several devices may present the same application-level identity, but each is its own client
  and leaf. Section 16.10 mentions schemes in which a user's clients vouch for each other. The RFC
  does not define a multi-device model beyond this; "each device is a separate leaf" is my
  inference from these sections.
- Confidence: medium (inferred from several sections, not stated in one sentence; B9 states it)

## B-Q2. RFC 9750, the MLS architecture

### B6. RFC 9750: what the Delivery Service does
- URL: https://www.rfc-editor.org/rfc/rfc9750.html
- Source date: April 2025 (Informational)
- Read: 2026-11-20
- Quote: "trusted to break ties when two members send a Commit message" (section 5.2.1; subject "The Delivery Service is")
- Paraphrase: the group must agree on one Commit per epoch, and Proposals should arrive before the
  Commit that references them (section 5.2). The DS is responsible for each KeyPackage being used
  to add its client to only a single group, except a designated "last resort" KeyPackage
  (section 5.1). Section 7 lists DS operations including uploading new KeyPackages for a user's
  own clients and delivering group and Welcome messages. A malicious DS cannot add itself to
  groups or recover the group key (section 5), but can block or reorder some messages, refuse to
  forward to specific clients, and hand out stale KeyPackages, which weakens PCS "to a limited
  extent" (section 8.4.2; the RFC suggests expiring initial keys).
- Confidence: high

### B7. RFC 9750: what the Authentication Service does
- URL: https://www.rfc-editor.org/rfc/rfc9750.html (section 4)
- Source date: April 2025
- Read: 2026-11-20
- Quote: "Issue credentials to clients that attest to bindings between identities and signature key pairs." (section 4)
- Paraphrase: the AS provides three services: issuing such credentials, verifying a client's
  credential against a reference identifier, and checking whether two credentials represent the
  same client. A compromised AS could let an adversary impersonate group members.
- Confidence: high

### B8. RFC 9750: what metadata the Delivery Service sees
- URL: https://www.rfc-editor.org/rfc/rfc9750.html (sections 5, 8.1, 8.4.1.1, 8.4.2.1, 8.4.3.2)
- Source date: April 2025
- Read: 2026-11-20
- Quote: "the Delivery Service can learn that a given client is sending the same message to a set of other clients" (section 8.4.3.2, server-side fan-out)
- Paraphrase: message headers carry an opaque group_id, the epoch and the message type
  (section 8.1); the DS "may be able to determine group membership" depending on deployment
  (section 5). With client fan-out the server usually does not need recipient information but may
  learn it through traffic analysis. The RFC recommends encrypted group-operation messages
  (section 8.4.3.2), notes push providers learn which devices receive data and when
  (section 8.4.2.1, suggesting random delays or a mixnet), and that connections leak the IP
  address (section 8.4.1.1, suggesting MASQUE, Tor or a VPN). No sealed-sender style hiding of the
  sender from the DS is described.
- Confidence: high

### B9. RFC 9750: several devices per user
- URL: https://www.rfc-editor.org/rfc/rfc9750.html (sections 3.7, 6.7, 8.2.4)
- Source date: April 2025
- Read: 2026-11-20
- Quote: "a user may have multiple clients with the same identity and different keys." (section 3.7)
- Paraphrase: MLS does not require a user's clients to share signature keys. A new device joins as
  a new client and does not get the group's earlier history (section 6.7). Other members can
  tell which of a user's devices sent each message, a privacy concern; sharing state across
  clients is one way around it (section 8.2.4).
- Confidence: high

## B-Q3. OpenMLS

### B10. OpenMLS repository and README (with the book's front page)
- URL: https://github.com/openmls/openmls (also read https://book.openmls.tech/ , which repeats the same lists)
- Source date: no date on page (repository main branch as shown on 2026-11-20)
- Read: 2026-11-20
- Quote: "enable compilation to wasm" (the `js` feature)
- Paraphrase: MIT licence. Supported ciphersuites listed: MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
  (the mandatory one), MLS_128_DHKEMP256_AES128GCM_SHA256_P256 and
  MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519; no post-quantum or X-Wing suite listed. CI
  builds but does not test wasm32-unknown-unknown; an `openmls-wasm` folder exists. The top-level
  folders (openmls, traits, libcrux_crypto, openmls_rust_crypto, memory_storage, sqlite_storage,
  sqlx_storage, interop_client, book and others) include no delivery-service folder, and neither
  the README nor the book's front page mentions one: OpenMLS appears to ship no Delivery Service.
  No audit is mentioned on either page (see B12).
- Confidence: high for licence, ciphersuites and wasm; medium for "no Delivery Service provided"
  (absence of a folder and of any mention)

### B11. OpenMLS releases on crates.io (and the 0.9.0 release post)
- URL: https://crates.io/api/v1/crates/openmls ; https://blog.openmls.tech/posts/2026-08-25-0.9.0-release/
- Source date: 0.9.2 published 2026-10-22; release post 2026-08-25
- Read: 2026-11-20
- Quote: "A Rust implementation of the Messaging Layer Security (MLS) protocol" (crate description; it continues "as defined in RFC 9420.")
- Paraphrase: the highest stable version is 0.9.2 (2026-10-22T08:49Z); 0.8.3 was published 22 s
  later the same day (a maintenance line); before that 0.9.1 and 0.8.2 (2026-10-07) and 0.9.0
  (2026-08-25). All MIT. The 0.9.0 post (Franziskus Kiefer) covers storage-codec migration, an
  early "targeted messages" feature behind a flag, AppDataUpdate handling, several security bug
  fixes and MSRV Rust 1.91; it says nothing about post-quantum suites, wasm or a DS.
- Confidence: high

### B12. OpenMLS security audit
- URL: https://blog.openmls.tech/posts/2026-05-27-independent-audit/
- Source date: 2026-05-27 (author Raphael Robert)
- Read: 2026-11-20
- Quote: "The audit was conducted by SRLabs."
- Paraphrase: sponsored by the Sovereign Tech Agency and completed earlier in 2026. Eight issues,
  one rated High (improper authentication of MACs); fixes shipped in releases the post
  calls 8.1 and 7.3 (presumably crate versions 0.8.1 and 0.7.3), with one Low issue still being
  fixed at posting time. The post
  does not state the audited commit or the scope (whether crypto backends were included). The
  report is linked as a PDF on the same site; I did not open it.
- Confidence: high for auditor, sponsor, counts; scope unknown

## B-Q4. Cryspen's post-quantum OpenMLS post

### B13. "Post-Quantum OpenMLS" (Cryspen)
- URL: https://cryspen.com/post/pq-openmls/
- Source date: 2024-04-11 (author Franziskus Kiefer)
- Read: 2026-11-20
- Quote: "There is no IANA code-point for this ciphersuite yet" (it continues "such that interoperability may not be guaranteed")
- Paraphrase: OpenMLS gained MLS_256_XWING_CHACHA20POLY1305_SHA256_Ed25519 with the private code
  point 0x004D, using libcrux's formally verified ML-KEM and X25519. It follows
  draft-connolly-cfrg-xwing-kem-02, which the post calls "an early draft". The post does not use the
  word "experimental"; the caveats are the missing code point and the early draft. Signatures stay
  Ed25519. Sizes, X-Wing versus X25519: KeyPackage 2669 vs 299 bytes, Welcome 5457 vs 716, ratchet
  tree 4007 vs 408, self-update 3954 vs 495. Note: this suite is not in the current README's
  supported list (B10), and draft -02 predates the final X-Wing drafts.
- Confidence: high

## B-Q5. draft-mahy-mls-xwing, and what replaced it

### B14. draft-mahy-mls-xwing (Datatracker page and text)
- URL: https://datatracker.ietf.org/doc/draft-mahy-mls-xwing/ ; https://datatracker.ietf.org/doc/html/draft-mahy-mls-xwing-00
- Source date: revision 00 of 2024-03-04 (the only revision); page last updated 2024-09-05
- Read: 2026-11-20
- Quote: "MLS_128_XWING_AES128GCM_SHA256_Ed25519"
- Paraphrase: an individual submission (author Rohan Mahy), expired and archived, "Replaced by
  draft-mahy-mls-pq". The header says Informational; the IANA value is "new assignment" (no
  number). It cites draft-connolly-cfrg-xwing-kem-01 normatively.
- Confidence: high

### B15. draft-mahy-mls-pq, and its successor draft-ietf-mls-pq-ciphersuites
- URL: https://datatracker.ietf.org/doc/draft-mahy-mls-pq/ ; https://datatracker.ietf.org/doc/draft-ietf-mls-pq-ciphersuites/ ; https://datatracker.ietf.org/doc/html/draft-ietf-mls-pq-ciphersuites-08
- Source date: draft-mahy-mls-pq-01 of 2025-07-07 (replaced); draft-ietf-mls-pq-ciphersuites-08 of 2026-11-18
- Read: 2026-11-20
- Quote: "provide authenticity against classical attacks only." (of the first eight suites, those with traditional signatures)
- Paraphrase: draft-mahy-mls-pq was adopted by the MLS working group and replaced by
  draft-ietf-mls-pq-ciphersuites, which is in IESG Last Call (ends 2026-12-03, telechat
  2026-12-17), intended Proposed Standard, not yet an RFC. It registers 11 suites, 0x0010 to 0x0022,
  including MLS_128_MLKEM768X25519_AES128GCM_SHA256_Ed25519 (0x0010) and
  MLS_192_MLKEM768_AES256GCM_SHA384_MLDSA65 (0x0021). The word "X-Wing" does not appear; the
  ML-KEM-768 + X25519 suites use HPKE KEM 0x647a from draft-ietf-hpke-pq and the
  concrete-hybrid-kems draft, which says that KEM is X-Wing (B16).
- Confidence: high

### B16. draft-irtf-cfrg-concrete-hybrid-kems-04: MLKEM768-X25519 is X-Wing
- URL: https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-concrete-hybrid-kems
- Source date: revision 04, 2026-07-06
- Read: 2026-11-20
- Quote: "It is identical to the X-Wing construction from [XWING-SPEC]." (section 4.2, MLKEM768-X25519)
- Paraphrase: a CFRG (research group) draft; its MLKEM768-X25519 has a 1216-byte encapsulation key,
  1120-byte ciphertext and 32-byte shared secret, matching X-Wing (B17). Only the first 100,000 of
  about 287,000 characters were read.
- Confidence: high

## B-Q6. draft-connolly-cfrg-xwing-kem

### B17. X-Wing KEM draft, revision 11
- URL: https://datatracker.ietf.org/doc/draft-connolly-cfrg-xwing-kem/ ; https://datatracker.ietf.org/doc/html/draft-connolly-cfrg-xwing-kem-11
- Source date: revision 11, published 2026-09-23 (expires 2027-03-27)
- Read: 2026-11-20
- Quote: "Informally, X-Wing is secure if SHA3 is secure, and either X25519 is secure, or ML-KEM-768 is secure." (section 6)
- Paraphrase: authors Deirdre Connolly, Peter Schwabe, Bas Westerbaan. The draft header says
  Workgroup "Crypto Forum", Intended Status Informational, but Datatracker lists it as an
  individual draft in the Independent Submission stream, not a CFRG-adopted document, with no
  intended RFC status. Sizes (section 5.1): decapsulation key 32 bytes (a seed), encapsulation key
  1216 bytes, ciphertext 1120 bytes, shared secret 32 bytes. Appendix C's test vectors are marked
  TODO (to be replaced with vectors reusing ML-KEM and X25519 values).
- Confidence: high (sizes and claim); the stream mismatch is as each page states it

## B-Q7. X-Wing implementations

### B18. Rust crate `x-wing` (RustCrypto)
- URL: https://crates.io/api/v1/crates/x-wing ; https://docs.rs/x-wing/latest/x_wing/
- Source date: 0.1.1 published 2026-10-01 (0.1.0 on 2026-07-09)
- Read: 2026-11-20
- Quote: "The implementation contained in this crate has never been independently audited!"
- Second quote (essential): "Current implementation matches the draft RFC version 06."
- Paraphrase: licence Apache-2.0 OR MIT; repository RustCrypto/KEMs; crate description says
  "(draft 06)". So it implements draft -06, not the current -11 (B17); whether -06 and -11 differ
  on the wire was not checked here. Docs list ENCAPSULATION_KEY_SIZE, CIPHERTEXT_SIZE and
  DECAPSULATION_KEY_SIZE without showing values.
- Confidence: high

### B19. `@noble/post-quantum` README (author's repository)
- URL: https://github.com/paulmillr/noble-post-quantum
- Source date: no date on page (README as shown 2026-11-20)
- Read: 2026-11-20
- Quote: "The library has not been independently audited yet."
- Paraphrase: MIT licence. A self-audit at version 0.6.1 in April 2026, scope "everything". The
  ML-KEM presets are "tested against" connolly-cfrg-xwing-kem-10, irtf-cfrg-concrete-hybrid-kems-03
  and irtf-cfrg-hybrid-kems-12; X-Wing is exported as `ml_kem768_x25519`. So its X-Wing matches
  draft -10 by test, not by an explicit conformance claim.
- Confidence: high for licence and audit wording; medium for "matches -10" (the README says
  "tested against")

### B20. `@noble/post-quantum` releases
- URL: https://github.com/paulmillr/noble-post-quantum/releases
- Source date: latest 0.7.1 dated "27 Aug" (the page shows no year; the README dates 0.6.1 to April 2026, so 0.7.1 is 2026-08-27 by inference)
- Read: 2026-11-20
- Quote: "`XWing`/`MLKEM768X25519` → `ml_kem768_x25519`" (0.7.0 notes)
- Paraphrase: 0.7.1 (27 Aug), 0.7.0 (09 Aug: hybrid legacy aliases removed, X-Wing renamed, faster
  hybrid), 0.6.1 (12 Apr). The 0.7.1 notes thank an auditor from "QuantaKrypto audits" without
  describing an audit. npm itself was not read (B-unreachable).
- Confidence: medium for the year; high for names and order

## B-Q8. The X-Wing paper

### B21. "X-Wing: The Hybrid KEM You've Been Looking For"
- URL: https://eprint.iacr.org/2024/039
- Source date: received 2024-01-09, last revised 2025-03-19; published in IACR Communications in Cryptology
- Read: 2026-11-20
- Quote: "X-Wing is a post-quantum IND-CCA secure KEM if ML-KEM-768 is itself an IND-CCA secure KEM."
- Paraphrase (abstract only; the PDF is disallowed by robots.txt): authors Barbosa, Connolly,
  Duarte, Kaiser, Schwabe, Varner, Westerbaan. Classically IND-CCA secure if the strong
  Diffie-Hellman assumption holds (random oracle model); post-quantum IND-CCA secure if ML-KEM-768 is
  IND-CCA secure and SHA3-256 is a secure PRF (standard model). The abstract warns the result relies
  on the concrete choices and may not apply in general.
- Confidence: high (abstract)

## B-Q9. The modular Double Ratchet

### B22. Alwen, Coretti, Dodis: "The Double Ratchet: Security Notions, Proofs, and Modularization for the Signal Protocol"
- URL: https://eprint.iacr.org/2018/1037
- Source date: received 2018-10-30, last revised 2020-02-21 (major revision of the EUROCRYPT 2019 paper)
- Read: 2026-11-20
- Quote: "can be easily and generically built from public-key encryption" (of continuous key agreement, CKA)
- Paraphrase (abstract only): the Double Ratchet is built from three parts: CKA, forward-secure AEAD
  (the symmetric ratchets), and a two-input hash that is a PRF-PRNG. The result gives forward
  security, post-compromise security and "immediate (no-delay) decryption", so parties recover
  seamlessly if a message is permanently lost. The framework lets one "easily get post-quantum
  security" without random oracles. **The abstract says public-key encryption, not KEM**; the
  KEM-based CKA construction is in the paper body (PDF), which robots.txt disallows, so the KEM
  wording is not confirmed from a page I read.
- Confidence: high for what the abstract says; the KEM claim is unconfirmed

## B-Q10. FIPS 203 and FIPS 204

### B23. FIPS 203, ML-KEM
- URL: https://csrc.nist.gov/pubs/fips/203/final ; PDF https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf (Table 3, section 8)
- Source date: published 2024-08-13
- Read: 2026-11-20
- Quote: "Table 3. Sizes (in bytes) of keys and ciphertexts of ML-KEM"
- Paraphrase: ML-KEM-768: encapsulation key 1184 bytes, decapsulation key 2400, ciphertext 1088,
  shared secret 32. (ML-KEM-512: 800 / 1632 / 768 / 32; ML-KEM-1024: 1568 / 3168 / 1568 / 32.)
- Confidence: high

### B24. FIPS 204, ML-DSA
- URL: https://csrc.nist.gov/pubs/fips/204/final ; PDF https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf (Table 2)
- Source date: published 2024-08-13
- Read: 2026-11-20
- Quote: "Table 2. Sizes (in bytes) of keys and signatures of ML-DSA"
- Paraphrase: ML-DSA-65: private key 4032 bytes, public key 1952, signature 3309. (ML-DSA-44:
  2560 / 1312 / 2420; ML-DSA-87: 4896 / 2592 / 4627.)
- Confidence: high

## B-Q11. Post-quantum replacements for X3DH

### B25. Hashimoto, Katsumata, Kwiatkowski, Prest: post-quantum X3DH from KEMs and signatures
- URL: https://eprint.iacr.org/2021/616
- Source date: received 2021-05-17, last revised 2022-05-10; Journal of Cryptology 2022 (full version of a PKC 2021 paper)
- Read: 2026-11-20
- Quote: "the first post-quantum secure replacement of the X3DH protocol on well-established assumptions"
- Paraphrase (abstract): built from key encapsulation mechanisms and signature schemes; the key
  stays secure under every non-trivial combination of compromised long-term and session secrets;
  it has "a weak flavor of deniability", which ring signatures or NIZK proofs can strengthen.
- Confidence: high

### B26. K-Waay: deniable post-quantum X3DH without ring signatures
- URL: https://eprint.iacr.org/2024/120
- Source date: received 2024-01-27, last revised 2024-01-29; USENIX Security 2024 (major revision)
- Read: 2026-11-20
- Quote: "At its core, K-Waay uses a split-KEM"
- Paraphrase (abstract): authors Collins, Huguenin-Dumittan, Nguyen, Rolin, Vaudenay. The split-KEM
  (from Brendel et al.) is instantiated from Frodo key exchange on plain LWE; it avoids ring and
  designated-verifier signatures, claims deniability and is reported much faster than earlier
  proposals. No bandwidth figures in the abstract.
- Confidence: high

---

## Sources I could not reach

| URL | Why | What I hoped to get |
|---|---|---|
| https://www.npmjs.com/package/@noble/post-quantum | robots.txt itself returned HTTP 403: the site refuses automated reading, so no npm page was read (and no registry or re-host was used to get around it). | The published npm version and its date. GitHub releases (B20) gave version names but no year. |
| https://cdn.jsdelivr.net/npm/@noble/post-quantum/README.md | robots.txt returned HTTP 400, no usable answer; the same README was read from the author's repository (B19). | Nothing beyond B19. |
| https://eprint.iacr.org/2018/1037.pdf | eprint robots.txt disallows PDFs for every reader. | The paper's KEM-based CKA construction and its exact statement (B22). |
| https://eprint.iacr.org/2024/039.pdf | eprint robots.txt disallows PDFs. | Details beyond the abstract (B21); not needed for the questions. |
| https://blog.openmls.tech/SRL-OpenMLS_security_assurance_assessment.pdf | Not opened: the URL was inferred from a relative link, and the questions only needed who and when. | The audit's scope and the audited commit (B12). |

## Open questions

- **Does OpenMLS today ship any X-Wing or ML-KEM suite?** The README lists only classical suites
  (B10), while Cryspen's 2024 post added 0x004D on X-Wing draft -02 (B13). Settle by reading the
  current `openmls/src/ciphersuite` sources or the libcrux provider's supported list on GitHub
  (blob pages are allowed), or the CHANGELOG.
- **Which X-Wing draft versions are wire-compatible with -11?** The Rust crate follows -06 (B18),
  noble tests against -10 (B19), OpenMLS's 2024 suite used -02 (B13). Settle by comparing the
  combiner label and key-derivation sections of -02, -06, -10 and -11 on Datatracker, or running
  each library against -11's test vectors (which are still marked TODO, B17).
- **Why does Datatracker list the X-Wing draft in the Independent Submission stream while its header
  says Crypto Forum?** (B17) The draft's history tab on Datatracker would show when the stream
  changed.
- **The KEM-based CKA in Alwen, Coretti, Dodis** (B22): the abstract says public-key encryption; the
  KEM construction needs the paper body, which robots.txt bars. The published EUROCRYPT 2019
  proceedings version on a publisher page whose robots.txt allows reading would settle it.
- **Scope of the SRLabs OpenMLS audit** (B12): the linked report would say whether the crypto
  backends (libcrux, RustCrypto) and the wasm build were covered.
- **RFC 9420 section 16.9's exact wording on a DS that resolves Commit conflicts** (B3) was
  paraphrased from the reading, not quoted; a second targeted read of section 16.9 would give it.
