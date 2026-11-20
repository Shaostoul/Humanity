# C. Threema, Apple and backup designs as references (read 2026-11-20)

Helper C of the forward-secrecy research set. Facts only, with one short exact quotation per entry
(two where essential). Everything outside quotation marks is my paraphrase. Read date for every
entry: 2026-11-20.

## robots.txt checks

| Site | Date checked | What it said about these agents |
|---|---|---|
| threema.ch | 2026-11-20 | 301 redirect to threema.com/robots.txt (below). |
| threema.com | 2026-11-20 | No ClaudeBot, Claude-User or anthropic-ai lines. `User-agent: *` disallows only `/docs/work/whitepapers/`. The whitepaper I read is under `/assets/documents/`, which is allowed. |
| threema-ch.github.io | 2026-11-20 | 404, no robots.txt (nothing disallowed). The guessed protocol-docs path was also 404. |
| github.com | 2026-11-20 | ClaudeBot and Claude-User share a group (`Crawl-delay: 1`) that disallows `/*/tree/`, `/*/raw/`, `/*/*/commits/` and more; `/blob/` and repo root pages are not disallowed. I read one repo root page only. |
| breakingthe3ma.app | 2026-11-20 | No User-agent or Disallow lines; only a content-signals comment block with no yes/no values set. Allowed. |
| www.usenix.org | 2026-11-20 | No Claude lines. `User-agent: *` disallows `/search`, `/publications/proceedings`, admin and user paths; `/conference/` allowed; `Crawl-delay: 10`. |
| security.apple.com | 2026-11-20 | 404, no robots.txt (nothing disallowed). |
| engineering.fb.com | 2026-11-20 | `User-agent: *` with an empty `Disallow:` (everything allowed). |
| www.whatsapp.com | 2026-11-20 | `User-agent: ClaudeBot` then `Disallow: /`. NOT read. |
| code.briarproject.org | 2026-11-20 | robots.txt answered HTTP 403. One honest attempt at the BTP spec page also got 403. NOT read. |
| briarproject.org | 2026-11-20 | 404, no robots.txt. One page read (C18). |

## Threema: forward secrecy (Ibex)

### C1. Threema Cryptography Whitepaper, section "Perfect Forward Secrecy (PFS)"
- URL: https://threema.com/assets/documents/threema-cryptography-whitepaper.pdf (44 pages; read as text)
- Source date: "Version: June 26, 2026". Its changelog: "2025-03-13 -- Move PFS to end-to-end encryption"; "2024-12-20 -- Document Multi-Device Protocol, add Desktop 2.0".
- Quotation: "between clients with the Multi-Device Protocol deactivated."
- Second (essential): "can decide whether it is safe to resend the message or not"
- Paraphrase:
  - Ibex gives PFS on the end-to-end layer, but only between clients that do not have the
    multi-device protocol switched on. The app tries to negotiate a PFS session as soon as a
    message is sent or received. Group messages also travel inside the pairwise PFS session.
  - No prekey server. The first sender starts a session by sending the public half of a fresh
    ephemeral Curve25519 key pair; the responder generates its own ephemeral pair and sends the
    public key back. Two modes: "Unilateral" (usable at once, before any reply; past messages
    survive compromise of the sender's long-term key but not the recipient's) and "Complete"
    (after the reply; past messages survive compromise of either long-term key).
  - Ratchet: ECDH over both ephemeral keys gives initial chain keys; each message derives a new
    chain key with BLAKE2b and erases the old one; per-direction chains via the Threema ID in the
    salt; the message key is a second BLAKE2b derivation from the chain key. Symmetric ratchet
    only (no new DH per message).
  - Counters: both sides keep a count of ratchet turns and put it in each message, so the
    receiver can detect lost messages, tell the user and step its ratchet forward.
  - Key storage: SQLite (iOS) / SQLCipher (Android), journaling off, `secure_delete` on.
  - Lost state: a party that cannot decrypt a PFS message because it lacks the key material
    signals this to the sender; the sender's user is told and decides whether to resend. Both
    users are told when a new session is negotiated, and when a non-PFS message arrives
    unexpectedly. (The whitepaper does not name the message types; see Open questions.)
  - Calls negotiate their own ephemeral keys, so calls have PFS independently of Ibex.
- Confidence: high.

### C2. Same whitepaper, section "Multi-Device Protocol"
- URL: as C1. Source date: Version June 26, 2026.
- Quotation: "Currently, the multi-device protocol comes without Perfect Forward Secrecy (PFS)."
- Second (essential): "Importantly, the mediator server has no knowledge about DGK."
- Paraphrase:
  - A mediator server sits between the chat server and the person's devices; it proxies to the
    chat server and delivers messages between devices of one Threema ID. Devices need not be
    online at the same time.
  - Linking (rendezvous): the existing device samples a 32-byte Authentication Key and shows it
    as a QR code the new device scans; handshake keys are BLAKE2b-derived from it; both devices
    exchange ephemeral X25519 keys in a three-way challenge-response; a path hash is shown as
    emoji for the user to compare (MITM defence); then ChaCha20-Poly1305 transport keys.
  - Device Group Key (DGK): 32 random bytes sampled by the first device (not derived from the
    permanent identity key), sent to the new device encrypted under the rendezvous transport key.
    From DGK: a path key pair used to authenticate to the mediator (challenge-response; the
    public key is the group's identifier), a device-info key, a reflect key, a shared-device-data
    key and a transaction-scope key. Device info (platform, version, label) is encrypted so the
    mediator cannot read it.
  - Outgoing messages: the sending device re-encrypts each sent message under the reflect key and
    sends it to the mediator, which forwards it or queues it for offline devices. Only one device
    at a time fetches from the chat server, and it reflects incoming messages the same way.
  - Contacts, groups, lists and settings sync under the shared-device-data key inside
    "transactions" to avoid conflicts.
  - The whitepaper section says nothing about transferring old message history.
  - Bringing PFS to multi-device is described as ongoing work. So Ibex is switched off while
    multi-device is active (C1 + C2 together).
- Confidence: high.

### C3. Same whitepaper, section "Threema Safe"
- URL: as C1. Source date: Version June 26, 2026.
- Quotation: "Messages and media data are not part of a Threema Safe backup."
- Paraphrase:
  - Contents: Threema ID, private key, profile (nickname, photo, linked email/phone), contacts
    (ID, public key, name, verification level), group definitions, distribution lists, settings.
    JSON, typically a few dozen KB.
  - Key: password of at least 8 characters, checked against a common-password list (warning).
    Master key = scrypt(password, salt = Threema ID, N = 65536, r = 8, p = 1, 64 bytes); first 32
    bytes are the backup ID (the file name, lowercase hex), last 32 bytes the encryption key;
    encryption is NaCl `crypto_secretbox` with a random 24-byte nonce.
  - Storage: Threema's servers by default, or the person's own WebDAV-compatible HTTPS server
    (URL may carry username:password). The backup server cannot tell which user a backup belongs
    to from the upload. Backup generated once a day while the app is in use; uploaded if changed
    or past half the server's retention time; DELETE when the user turns it off.
- Confidence: high.

### C4. Same whitepaper, section "Key Backup" (the ID export)
- URL: as C1. Source date: Version June 26, 2026.
- Quotation: "using PBKDF2 with HMAC-SHA256 and 100000 iterations"
- Paraphrase: a separate password-protected export of only the identity and private key (min. 8
  character password, random 64-bit salt, 2-byte truncated SHA-256 check value, XSalsa20 with an
  all-zero nonce), Base32 in 80 characters, also shown as a QR code. The app reminds users to make
  one because a lost private key cannot otherwise be recovered.
- Confidence: high.

### C5. "Security of Ibex" (formal analysis), sections 4, 5.2 and 6
- URL: https://threema.com/assets/6-resources/audits/security_analysis_ibex_2023.pdf
- Source date: July 30, 2023. Authors: Paul Gerhart, Paul Rösler, Dominique Schröder.
- Quotation: "forward security can only be achieved after one communication round."
- Paraphrase:
  - The key exchange is called 4DH and is modelled as a continuous key exchange plus a channel.
  - Per-peer local states: empty, 2DHS (sent first, no reply yet), 2DHR (received first), 4DH.
    2DH key = DH(static_A, static_B) combined with DH(ephemeral_A, static_B); messages under it are
    not forward secure for the responder. The responder's reply adds a fresh ephemeral key, and
    4DH = the 2DH key plus DH(ephemeral_B, static_A) and DH(ephemeral_B, ephemeral_A); both move
    to 4DH. KDF labels in the formal description: `ke-2dh-<ID>` and `ke-4dh-<ID>`.
  - The first half round-trip only gives forward security from initiator to responder, not the
    reverse ("cf. 2DH vs. 4DH").
  - Within an established session keys come from symmetric secrets, so stealing a party's state
    also reveals its future keys: no post-compromise security inside a session.
  - Result: no design flaws found; breaking Ibex reduces to Gap-Diffie-Hellman (random oracle
    model).
- Confidence: high.

### C6. "Security of Ibex", sections 9 and 10 (limitations and recommendations)
- URL and date: as C5.
- Quotation: "the current fallback option is to notify the user in the UI"
- Paraphrase:
  - Version negotiation as of 2023: when two users' versions do not allow Ibex, the app tells the
    user and falls back to the old non-forward-secure protocol. The authors warn that an attacker
    who can change the feature mask on the directory server could force that downgrade, and
    recommend binding the protocol version into associated data.
  - Not covered by the proof: the "double encryption" (Ibex ciphertext also encrypted under the
    static key pair, a circularity), unverified keys with a dishonest directory, and the desktop
    app linked to the phone. The implementation was not reviewed.
  - They recommend investing in post-quantum key exchange.
- Confidence: high.

### C7. Threema blog, "New Communication Protocol 'Ibex' and Extended Protocol Suite"
- URL: https://threema.com/en/blog/ibex
- Source date: December 4, 2022; an update note dated "8/2/2023" (format ambiguous; C8 suggests
  August 2023).
- Quotation: "The new Ibex protocol now also supports the exchange of ephemeral keys"
- Paraphrase: Ibex adds ECDH ephemeral keys and KDF ratcheting on the end-to-end layer; it is also
  said to let clients detect a server replaying, dropping or reordering messages. Threema 5.0 for
  Android was the first version with it, iOS to follow. The suite included a protocol for the then
  upcoming multi-device feature. No public Ibex wire specification is linked from the page; it
  links the whitepaper and the security-proof post.
- Confidence: high.

### C8. Threema blog, "Security Proof of Threema's Communication Protocol"
- URL: https://threema.com/en/blog/security-proof-ibex
- Source date: August 1, 2023.
- Quotation: "Perfect Forward Secrecy is now enabled by default in the latest mobile apps."
- Paraphrase: the proof is the Erlangen-Nuremberg analysis (C5); PFS on by default from Threema 5.1
  for Android and 5.4 for iOS. The post says nothing about multi-device or session handling.
- Confidence: high.

## Threema: the 2023 ETH Zurich analysis and the response

### C9. "Three Lessons from Threema" (project page)
- URL: https://breakingthe3ma.app/
- Source date: page footer 2023; public disclosure date given as 9 January 2023.
- Quotation: "Threema features neither forward security nor post-compromise security."
- Paraphrase: seven attacks in three threat models. Relevant here: Attack 4 (replay/reflection
  after the nonce database is lost on reinstall or device change), Attack 6 (cloning by ID export
  with a password chosen on the spot), Attack 7 (compress-then-encrypt Threema Safe backups leak
  the private key to an attacker who controls the nickname field; a few hours of attempts). The
  page says Ibex aims to add forward security and that the authors had not audited it. Timeline:
  disclosed 3 Oct 2022; Ibex released 29 Nov 2022; mitigations Q4 2022 and Q1 2023.
- Confidence: high (the forward-secrecy quote describes the pre-Ibex protocol).

### C10. The paper, "Three Lessons From Threema: Analysis of a Secure Messenger" (PDF)
- URL: https://breakingthe3ma.app/files/Threema-PST22.pdf (read as text)
- Source date: no date on the PDF; disclosure dates inside (whitepaper updated 15.12.2022 for PFS;
  public disclosure 09.01.2023).
- Quotation: "this cannot be guaranteed whenever the app is reinstalled"
- Second (essential): "We have not audited this new protocol."
- Paraphrase: Threema's pre-Ibex replay defence was a stored nonce list, which is lost on reinstall
  or device change (iOS Quick Start keeps it). A restore from Threema Safe is followed shortly by a
  C2S connection from the same IP, so the server can link the backup to the identity and know the
  nonce list was just wiped. With no forward secrecy, a compelled-access key theft exposed all past
  and future traffic.
- Confidence: high.

### C11. USENIX presentation page for the paper
- URL: https://www.usenix.org/conference/usenixsecurity23/presentation/paterson
- Source date: August 2023.
- Quotation: "32nd USENIX Security Symposium (USENIX Security 23)"
- Paraphrase: venue confirmed; authors Kenneth G. Paterson, Matteo Scarlata, Kien Tuong Truong
  (ETH Zurich); pages 1289-1306.
- Confidence: high.

### C12. Threema blog, "Statement on ETH Findings"
- URL: https://threema.com/en/blog/news-alleged-weaknesses-statement
- Source date: January 8, 2023.
- Quotation: "With PFS, reordered or missing messages are detected, and the user is warned."
- Second (essential): "Threema Safe compression is disabled in current app versions"
- Paraphrase: Threema called the findings without real-world impact and said Ibex was planned
  before the disclosure. Changes it lists: new chat-server and directory authentication and no
  more ephemeral-key caching (Android 5.0+, iOS 4.8.5+); Ibex now detects reordering/omission and
  drops replays even when the nonce database was lost; Threema Safe compression disabled in the
  same versions; ID export kept by design, with app lock advice, an MDM switch to disable export,
  and a new alert when another device connected since last use. Nothing on multi-device.
- Confidence: high.

## Threema: multi-device and backups (user pages)

### C13. Threema blog, "Threema Multi-Device: An Architectural Overview"
- URL: https://threema.com/en/blog/md-architectural-overview
- Source date: November 3, 2020.
- Quotation: "all key material is derived from the Threema ID’s private key"
- Paraphrase: the 2020 design derived the Device Group Key from the identity's private key; the
  mediator picks one "leader" device to take messages from the chat server and reflect them;
  reflected messages are encrypted with the Device Group Key; the mediator hides users' IP
  addresses from the chat server and can be self-hosted. NOTE: the 2026 whitepaper (C2) now says
  DGK is randomly sampled and does not rely on the permanent key, so the design changed.
- Confidence: high (for what the 2020 post said).

### C14. Threema FAQ, "What is the beta version of Threema 2.0 for desktop?"
- URL: https://threema.com/en/faq/md-overview
- Source date: no date on page (footer 2026; mentions Threema 6.0 Android / 6.4 iOS).
- Quotation: "Support for multiple linked devices, including tablets, is planned."
- Paraphrase: one phone plus up to two computers; link via Settings, "Add Device", on-screen steps.
  The page says nothing about old history or PFS.
- Confidence: high.

### C15. Threema FAQ, "Move Your Threema Data to a New Phone (Backup)"
- URL: https://threema.com/en/faq/data-backup
- Source date: no date on page (footer 2026).
- Quotation: "Moving data backups between devices is only possible within the same operating system."
- Paraphrase: a data backup (unlike Threema Safe) holds chats, groups, contacts with verification
  level and the ID; on Android, optionally large media; settings not included. Android: password
  set when creating it, file saved where the user picks on the device, then moved by hand. iOS:
  Quick Start or an encrypted iTunes/Finder backup plus a password-protected ID export. The page
  gives no password rules or KDF. Threema Safe is the advice for switching OS.
- Confidence: high (KDF unknown).

## Apple iMessage PQ3

### C16. Apple Security Research, "iMessage with PQ3"
- URL: https://security.apple.com/blog/imessage-pq3/
- Source date: February 21, 2024.
- Quotation: "the post-quantum ratchet is performed approximately every 50 messages"
- Second (essential): "rekeying is always guaranteed to occur at least once every 7 days"
- Paraphrase:
  - Why not every message: "the post-quantum KEM ratchet increases the message size by more than
    2 kilobytes" while a 32-byte ECDH ratchet is fine on every message; amortising avoids delays on
    poor connections; the schedule also weighs connectivity and server capacity and may tighten.
  - Keys: initial establishment uses a Kyber-1024 public key plus a P-256 ECDH key registered with
    Apple's Identity Directory Service and replaced regularly, so the first message to an offline
    device is already PQ-protected; the rekeying KEM is Kyber-768. Device keys are signed by a
    Secure Enclave P-256 ECDSA key with a timestamp-limited validity; each message is also signed
    with ECDSA P-256.
  - Several devices: every device has its own keys (private keys never leave it), and "The
    messages are exchanged through pair-wise sessions established between the sending device and
    each receiving device." The message is encrypted separately per device.
  - Analyses: Douglas Stebila (game-based proof) and Basin, Linker, Sasse (Tamarin, ETH Zurich).
- Confidence: high.

## WhatsApp end-to-end encrypted backups

### C17. Engineering at Meta, "How WhatsApp is enabling end-to-end encrypted backups"
- URL: https://engineering.fb.com/2021/09/10/security/whatsapp-e2ee-backups/
- Source date: September 10, 2021.
- Quotation: "WhatsApp will know only that a key exists in the HSM."
- Paraphrase: the backup is encrypted with a random key; the user either keeps a 64-digit key
  (entered by hand to restore) or picks a password, in which case the key is stored in an HSM-based
  Backup Key Vault that checks the password and makes the key permanently unreachable after a
  limited number of wrong attempts. The vault spans several data centres. The front-end service
  cannot read the client-vault traffic; the cloud provider (iCloud, Google Drive) and WhatsApp
  cannot read the backup. The page does not name the password-verification protocol.
- Confidence: high (protocol name not on this page).

## Briar

### C18. Briar, "How it works"
- URL: https://briarproject.org/how-it-works/
- Source date: no date on page.
- Quotation: "All communication between devices is encrypted end-to-end"
- Paraphrase: the page does not describe BTP tags or key rotation; it points to a wiki for
  technical detail. C-Q7 stays unanswered from a primary page (see below).
- Confidence: high (that the page lacks it).

## Sources I could not reach

- https://www.whatsapp.com/security/WhatsApp_Security_Encrypted_Backups_Whitepaper.pdf (and all of
  whatsapp.com): robots.txt has `User-agent: ClaudeBot` / `Disallow: /`. Hoped for the vault's
  guess limit, the OPAQUE-style password protocol and HSM details.
- https://code.briarproject.org/briar/briar-spec/ (BTP.md): robots.txt and the spec page both
  returned HTTP 403. Hoped for the BTP tag design. A web search snippet (NOT a page I read, so
  unverified) claimed each stream starts with a pseudo-random 16-byte tag the recipient computes in
  advance, and that outgoing keys for period P are deleted at the end of P and incoming keys at
  the end of P + 1. Copies of the spec on other sites were not read (rule 2).
- Threema protocol specifications (Ibex wire format, session races, multi-device "device join"
  and history transfer): no public specification site found. threema-ch.github.io/threema-protocols/
  is 404; the threema-desktop README says a `threema-protocols` repository must be cloned but gives
  no public URL; the GitHub tree views are disallowed by robots.txt.

## Open questions

- Ibex session lifecycle details (the names of the init/accept/reject/terminate messages, what
  happens when both sides start a session at the same moment, how a version is negotiated today
  after the 2023 downgrade recommendation): would be settled by Threema's protocol specification
  or the Ibex source in the threema-android / threema-ios repositories (individual `/blob/` files
  are readable under GitHub's robots.txt if a direct path is known).
- History transfer to a newly linked Threema device: neither the whitepaper nor the FAQ I read
  says. A search snippet claimed a threema.com FAQ says messages from before linking are not shown
  in the beta; I did not read that page.
- Whether Threema ever added the recommended protocol-version binding (C6): later whitepaper text
  does not mention it.
- The KDF and parameters for Threema Android data backups (C15): not stated on the FAQ; the ETH
  paper mentions Zip4j zip files, which suggests zip encryption, unconfirmed.
- WhatsApp's guess limit and password protocol: only in the whatsapp.com whitepaper, which
  robots.txt bars.
