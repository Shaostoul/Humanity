# Notes for the draft page "Who can read your private messages?" (never shipped)

**Status: a draft. Nothing has been posted anywhere, and the page is not in the Library yet.**
Drafted with AI (Claude) on 6 November 2026, at the operator's suggestion ("I feel like we could
make a post about HumanityOS' privacy vs all other platforms."). The page itself is
[`DRAFT-privacy-compared.md`](DRAFT-privacy-compared.md) beside this file, written exactly as it
will ship in the Library; these notes are for the operator and whoever lands it, and no Library
entry ever points at this file.

Every claim about another platform comes from the dated research note
[`docs/reference/findings/2026-11-06-social-platform-apis.md`](../../reference/findings/2026-11-06-social-platform-apis.md),
whose sources (each platform's own pages) were read on 6 November 2026; the finding numbers (MA-17,
BS-18 and so on) in the fact-check list at the end point into it. Every claim about HumanityOS was
checked against the code on 6 November 2026, not only against
[`docs/outreach/privacy_by_architecture.md`](../privacy_by_architecture.md) and
[`docs/reference/cryptography.md`](../../reference/cryptography.md); where those overclaim, the page
says the narrower thing (section B). Words: "recovery phrase", "no sign-up" (CLAUDE.md, account
words). No em dashes.

## The operator's answers (2026-11-06)

- **Where the long version lives:** "Library makes sense. Not sure which subcategory." The category
  is ours to recommend, not his decision: **"Your Privacy"**, in the Library's About HumanityOS
  section, the shelf made on 2026-11-06 for the badge-word test page
  ([`what_do_these_labels_mean.md`](../what_do_these_labels_mean.md)). Recommended because the page
  is about the reader's own privacy and sits naturally beside the test of the words the Inbox will
  use for it; "How It Helps" is the project's pitch to each audience, a different kind of page.
- **When it goes in:** "Yeah, we're waiting until after." That is, after the DM mailbox ordering fix
  (A.2 below) lands, so the page never ships with "We are fixing it" in it.
- **Posting at all:** "Let's get more of the security stuff finished before you start posting." The
  short version below is not posted by the project's accounts on any outside platform until the
  security work named in [`docs/design/outbox-and-inbox.md`](../../design/outbox-and-inbox.md)
  section 14 ("The gate before any posting") is finished. Adding the page to the Library is not
  posting and waits only for the mailbox fix.

## Landing the page, once the DM mailbox fix has landed (in this order)

1. **Bring the limits up to date.** In "What HumanityOS cannot protect you from", delete the bullet
   "A gap we found while checking this page", and add to "What is still coming" a line beside the
   other fix: "**Fixed on <date>:** the order in which the server stores sealed envelopes no longer
   pairs a message with the sender's own copy." Check those words against the fix as it was built.
   As built (BUG-259, 2026-11-06, A.2): the server keeps envelopes by recipient with only the day,
   and both apps send the sender's own copy a random 20 s to 2 min after the recipient's; "<date>"
   is the day the release carrying it went live on united-humanity.us, not the day it was written.
   If the group-metadata fix (C.2) has landed too, rewrite "Group chats show who is talking" to what
   is true then; if not, it stays.
2. **Check the dates.** Every comparison says "as of 6 November 2026" and stays true as of that day.
   If more than about three months have passed, re-read the finding's sources first and update both
   the finding and the tables.
3. **Fill in the date** in the last line: "*Published [date].*"
4. **Give the page its permanent name**, which is also its Library address
   (united-humanity.us/library#who-can-read-your-messages):
   `git mv docs/outreach/posts/DRAFT-privacy-compared.md docs/outreach/who_can_read_your_messages.md`
5. **Add this entry to `data/library/catalog.json`** as the FIRST document of the "Your Privacy"
   category (before "What Do These Labels Mean to You?"), exactly:

   ```json
           {
             "title": "Who Can Read Your Messages",
             "src": "docs/outreach/who_can_read_your_messages.md",
             "tags": [
               "project",
               "real",
               "reference"
             ]
           },
   ```

6. **Build and check:** `node scripts/build-library.js`, then `node scripts/check-library-fresh.js`,
   `check-library-dashes.js`, `check-library-render.js`, `check-library-quotes.js` and
   `node scripts/check-doc-links.js` (all 0), and `node --test scripts/tests/library-order-web.test.js`.
   Also update the links to this draft in `docs/design/outbox-and-inbox.md` (section 14) to the new
   path.
7. **The short version's link** becomes https://united-humanity.us/library#who-can-read-your-messages
   (61 characters as written); recount fact-check line 47 with it.
8. **When the short version is posted** (after the posting gate), rename this file to
   `docs/outreach/posts/<posting date>-privacy-compared.md`, the record of what was posted where.

## Should `privacy_by_architecture.md` join "Your Privacy"?

**Recommendation: yes, but only after a correction pass, and not before the mailbox fix.** It is the
longer story of how the privacy work was built and belongs on the same shelf. It is not in the
Library today, and it should not go in as it stands, because the Library is a checked copy and this
page would then contradict it in public:

- six of its claims overclaim or are out of date (section B below: the conversation graph, groups
  "entirely" unreadable, every image stripped before it touches the disk, the onion door, passes
  that "cannot be revoked", logs kept "two days");
- its opening story (the court order to Discord and Microsoft about some 100,000 users) is not in a
  dated finding; the Library should carry it only with a source a reader can check (CLAUDE.md, legal
  and similar claims: a dated finding with its source);
- its "no conversation graph to reconstruct" becomes true again only once the mailbox fix lands.

So: correct it against section B, source the opening story (or cut it), then add it to "Your
Privacy" in the same change that lands this page, as its second document.

---

## The short version (for Mastodon, Bluesky and X; 222 characters before the link)

Who can read your private messages? We compared HumanityOS with 13 platforms, from each one's own
published rules as of 6 November 2026: encryption, and what each hands over when the law asks. Our
own limits are in it too. [link to the long version]

---

## A. Before posting

1. **Where the long version lives: decided** (above): a Library page in "Your Privacy", once the
   mailbox fix lands.
2. **The row-order gap** (the third limit). Found while checking the post: `src/relay/storage/dms.rs`
   numbers mailbox rows in arrival order (`id INTEGER PRIMARY KEY AUTOINCREMENT`), and both apps send
   a direct message's recipient copy and the sender's own copy one right after the other (the
   website, `web/chat/chat-ui.js`: `ws.send(built.recipientPut)` then `ws.send(built.selfPut)`; the
   desktop app, `src/gui/pages/chat.rs`: `client.send(&self_put)` then the recipient copy; control
   messages likewise, `src/engine/dm.rs`), both padded to the same size. So a copy of the database
   usually shows two neighbouring envelopes of the same size, one addressed to the recipient and one
   to its sender, which pairs them. The operator chose to wait for the fix before the page goes in
   (above), so the landing steps take the bullet out.
   **Update 2026-11-06: fixed in the code (BUG-259).** The relay keeps mailbox rows by recipient
   (WITHOUT ROWID, numbered per mailbox from the day: `src/relay/storage/dms.rs`), and both apps send
   the sender's own copy a random 20 s to 2 min after the recipient's (`src/net/self_mail.rs`,
   `web/shared/self-mail.js`). The page is left as written here until it lands: landing step 1 runs
   once the release carrying the fix is live. Rows stored before it keep their old numbers until they
   age out of the 30-day window. Still true, and in the page's other bullets or in
   `privacy_by_architecture.md`: the server sees sender and recipient at the moment of sending, and
   its write log and continuous backup keep write order, which on a very quiet server can still
   suggest a pair.
3. **Nothing here names a person or attacks anyone.** It credits the platforms that encrypt, and says
   plainly that keeping a readable copy is not a scandal. The Discord court-order story that opens
   `privacy_by_architecture.md` is left out on purpose: it is not in the dated finding, and this post
   cites only what the finding read.

## B. What the source documents overclaim (so the post says the narrower thing)

Checked against the code on 6 November 2026; each is for whoever next edits those documents.
**All corrected later on 6 November 2026** (the privacy-fix wave): `privacy_by_architecture.md`
says the narrower true thing for each, `SECURITY-CADENCE.md`'s threat model is current,
`cryptography.md` has a "P2P group messages" row, and the code changed where it was the code that
fell short (the mailbox order, BUG-259; a picture the server cannot clean, BUG-260). The group
metadata is logged, not fixed (BUG-261). Each item below says what changed.

- `docs/outreach/privacy_by_architecture.md`, "There is no conversation graph to reconstruct" (and
  the same in `docs/reference/cryptography.md`'s DM metadata row and `src/relay/storage/dms.rs`'s
  header): the row-order pairing in A.2. *Now: fixed in the code (BUG-259); all three places say
  exactly what is true, the residuals included.*
- Same document, "Groups run entirely on encrypted, signed data the server cannot read": what is said
  is sealed (`src/net/group_e2ee.rs`), but the relay's own tables hold each group's name in plain text
  (`p2p_groups.name`), its member list (`p2p_group_roster`), and each message's author and time
  (`p2p_group_messages.author_fp`, `created_at`), in `src/relay/storage/mod.rs`. *Now: the document
  says so; logged as BUG-261 with its fix direction (PRIORITIES TIER 1 item 5).*
- Same document, "every image uploaded to a HumanityOS server is stripped ... before the file ever
  touches our disk": the server's picture strip gives back the original bytes when it cannot read a
  picture's structure (`src/relay/upload_rules.rs` `finish_picture`), a person may choose to keep a
  file's details (`keep_metadata`), and a location written inside a video stream itself is not
  caught. The app cleans on the device first and refuses an unchecked file for a public place, so the
  post says "on your own device before they leave, unless you turn that off". *Now: the server
  refuses a picture it cannot clean, and keeps one as it is only on the person's own signed "Send
  anyway" (BUG-260); the document lists which formats are stripped, refused, or passed (GIF).*
- Same document, the Tor onion door: it reaches the server's interface and chat, not the website
  (`docs/reference/cryptography.md`, Transport IP privacy), and united-humanity.us has none: checked
  over SSH on 6 November 2026, no `tor` program installed and the service inactive. *Now: the
  document says both.*
- Same document, "Certificates do not yet expire ... cannot be revoked by the server": out of date the
  other way. Friendship passes can be withdrawn since v0.1466.0 (`cert_revoke`), and by design still
  have no end date. *Now: the document says so.*
- Same document, "we cut that to two days": the live rotation is daily with two old files kept
  (`/etc/logrotate.d/nginx`: `daily`, `rotate 2`, read on 6 November 2026), so up to three days. The
  post says "at most three days". *Now: the document says so too.*
- `docs/SECURITY-CADENCE.md`'s threat-model snapshot (2026-05-20) still says an operator can be made
  to hand over "who messaged whom, when"; that predates the sealed-sender mailbox of 2026-08-23.
  *Now: brought current, with what a server can be made to hand over.*
- `docs/reference/cryptography.md` has no row for group messages themselves (AES-256-GCM under a
  per-epoch group key sealed to each member with ML-KEM-768, `src/net/group_e2ee.rs`), and the
  relay's `src/relay/storage/groups_p2p.rs` header still names XChaCha20-Poly1305 for them. *Now:
  the row is in, and the header says AES-256-GCM.*

## C. "Not much else we can do to further protect users at this point?"

There is more, and some of it is worth doing. Already built or designed is left out: the sealed
mailbox, friendship passes, the onion door, photo cleaning, padding, export and erase, encrypted
backups, the scam warnings, the protected setup, and the Outbox design's section 15 (the link check,
the notification layering, disappearing messages, the app lock, the disk check). Checked against
`docs/PRIORITIES.md`, `docs/SECURITY-CADENCE.md` and `docs/BUS-FACTOR.md` on 6 November 2026.

| # | Protection | What it protects against | Rough size | Already planned? |
|---|---|---|---|---|
| 1 | **Fix the mailbox's row order** (A.2): rows kept in an order that does not follow arrival, for example keyed by recipient and a per-mailbox count, and the sender's own copy sent apart from the recipient's | A seized or leaked database pairing who wrote to whom | Small: the relay's mailbox table and its paging, plus a test that two puts in a row cannot be paired | Yes since 2026-11-06: the first item of the posting gate (`outbox-and-inbox.md` section 14) |
| 2 | **Hide a group's metadata from its server**: the group's name sealed like its messages, the author inside the sealed part, times coarsened like the mailbox's | The server learning a group's name, members and who posts when | Medium to large: the group object format on both apps and the relay | Yes since 2026-11-06: in the posting gate (section 14) |
| 3 | **Send without telling the server who you are** (the envelope put from a connection that has not signed in, with a delivery pass from the recipient, the way some messengers do it) | A changed server, or someone watching it live, seeing who sends to whom | Large: the reach and rate-limit checks now use the sender's identity and would need another basis | No ("mixnet-class traffic analysis" is listed as what remains, PRIORITIES TIER 1) |
| 4 | **Forward secrecy for direct messages**: keys that change as a conversation goes on, so a key taken later cannot open messages already delivered | Someone who later gets a person's recovery phrase or device opening envelopes they copied earlier (from the server's 30 days or its backups) | Large: a ratcheting protocol on the post-quantum keys, across a person's several devices | No (only a line in `docs/design/to_research.md`) |
| 5 | **Compare keys in person**: a short code from each person's key to read aloud or scan | Someone passing off a different identity as the person the first time two people talk, through a server or a look-alike name (a server cannot forge a person's key card, but today the first identity seen is trusted; later key changes are warned) | Small: a code from the key card on both apps | No (a known limit in `docs/design/key-cards.md`) |
| 6 | **Reproducible builds**: anyone can build the release from the source and get the identical program | A changed program shipped in place of the published code | Medium: fixed toolchain and paths in the build, a script and a check that two builds match | No |
| 7 | **An outside audit by people who did not build it** | Mistakes everyone close to the code misses | Large; a hired firm is budget-gated (`SECURITY-CADENCE.md` 4). The quarterly independent review (item 3 there) is the no-cost step: its log has nothing after the post-quantum cutover (about v0.266), though the 51-agent audit of 2026-06-12 (`docs/history/priorities-archive-2026-05-to-08.md`) was one in all but name and is not logged there. No outside human review or pentest is logged | Partly (the cadence exists; its log is behind) |
| 8 | **The onion door actually open on united-humanity.us**, and serving the website too; later, a "connect through Tor" switch in the desktop app | Servers and watchers learning people's network addresses | Small for the door (`docs/admin/tor-onion-service.md`, about ten minutes on the server, the operator's step) plus the website fix; large for Tor inside the app (a dated licence finding first for any Tor library) | The door exists; it is not running |
| 9 | **Default-on onion routing** in the app | The same, for everyone, without having to choose | Large, with costs: slower, and Tor is blocked in some countries, so a default needs a fallback | No |
| 10 | **A published page on how a server answers legal requests** (what it holds, what it can and cannot hand over, telling people unless barred), and perhaps a transparency report or warrant canary | People not knowing what a request to the server can reach | Small to write, but a LEGAL question: a dated finding first (CLAUDE.md), especially for a canary, whose legal standing must be read from primary sources, not assumed | No |
| 11 | **Looking someone up without the server learning who** | The server learning whom you are about to message when your app fetches their key card | Large research (private lookup schemes); low urgency, since there is no address-book upload or phone-number discovery to protect | No |

Recommended order: 1 now (small, and it narrows a public claim), 5 and the door in 8 next (small, real
protection), then 2, 4 and 6 as designed rungs, with 10's finding written whenever the operator wants
the page. Nothing here needs a purchase except the hired audit in 7.

## D. Fact-check list

Each checkable claim in the post, its source, and whether it was checked on 6 November 2026. Finding
numbers point into `docs/reference/findings/2026-11-06-social-platform-apis.md` (its sources read 6
November 2026, each with its own date there).

| # | Claim in the post | Source | Checked |
|---|---|---|---|
| 1 | Instagram ended end-to-end encryption on 8 May 2026 | IG-8 | yes |
| 2 | Mastodon DMs are not end-to-end encrypted; both servers' operators can read them | MA-17 | yes |
| 3 | Bluesky DMs are not end-to-end encrypted, can be accessed for trust and safety; in 2024 it promised encryption later | BS-17 | yes |
| 4 | X Chat is end-to-end encrypted; older X DMs are not; X says it cannot read X Chat | X-22 | yes |
| 5 | Messenger personal chats end-to-end encrypted by default, still rolling out; not business or Page chats; group chats unclear | FB-10 | yes |
| 6 | Threads DMs: no Meta page says whether they are encrypted | TH-10 | yes |
| 7 | YouTube messages not end-to-end encrypted; may be reviewed against its rules | YT-11 | yes |
| 8 | Reddit makes no claim of end-to-end encryption; collects private messages and chats | RD-6 | yes |
| 9 | LinkedIn makes no claim of it; scans messages for its own tools | LI-9 | yes |
| 10 | Discord text not encrypted; voice and video calls always end-to-end encrypted since 2 March 2026 | DC-9 | yes |
| 11 | Telegram: only Secret Chats, one to one, tied to one device; ordinary chats on its servers | TG-8 | yes |
| 12 | Rumble: no private-message feature found | RU-6 | yes |
| 13 | Patreon collects and moderates messages; not end-to-end encrypted | PA-6 | yes |
| 14 | X: content on a search warrant; records on a subpoena, court order or other process; tells users unless prohibited | X-23 | yes |
| 15 | Meta: subpoena for basic records with a recent login address; court order for headers and addresses; warrant for content; cannot give encrypted content, still gives logs and addresses; notice unless barred | FB-11 | yes |
| 16 | Google: subpoena for subscriber details and some addresses; court order for other records; warrant for content | YT-12 | yes |
| 17 | LinkedIn: subpoena for basic details and login addresses; court order for from, to, date and time; warrant for messages; civil process not enough | LI-10 | yes (the finding transcribed a PDF from page images; re-read before posting if you want certainty) |
| 18 | Bluesky: DMs to third parties only on a court order; no breakdown | BS-18 | yes |
| 19 | Discord: the legal process US law requires, not broken down; tells users unless prohibited | DC-10 | yes |
| 20 | Telegram: says 0 bytes of user messages disclosed, governments included; IP and phone on a judicial order about a suspect; metadata up to 12 months | TG-9 | yes |
| 21 | Mastodon: each operator under its own law; mastodon.social under German law, keeps the latest login address up to 12 months | MA-18, the finding's "Findings: Mastodon" introduction | yes |
| 22 | Reddit's law-enforcement page could not be read by the research tools | RD-7 | yes |
| 23 | Rumble and Patreon: no breakdown of which process unlocks what | RU-7, PA-7 | yes |
| 24 | Mastodon's documentation says it is not an encrypted messenger and names others that are | MA-17 (it names Signal and Matrix) | yes |
| 25 | HumanityOS: the server refuses to store an unsealed DM | `src/relay/handlers/msg_handlers.rs` `deposit_dm`, `is_v2_envelope` | yes |
| 26 | HumanityOS: a DM envelope is stored with recipient and day, no sender column; deleted after 30 days by default; you can clear yours sooner | `src/relay/storage/mod.rs` (`dm_mailbox`; `dm_mailbox_ttl_days ... DEFAULT 30`), `src/relay/storage/dms.rs` (`mailbox_purge`), `storage/expiry.rs` | yes |
| 27 | HumanityOS: DM history kept on your devices, encrypted | `src/net/dm_store.rs` (AES-256-GCM under a seed-derived key), `web/chat/chat-dm-store.js` | yes |
| 28 | HumanityOS: group chats end-to-end encrypted | `src/net/group_e2ee.rs` | yes |
| 29 | HumanityOS: group name, members, author and time readable by the server | `src/relay/storage/mod.rs` (`p2p_groups`, `p2p_group_roster`, `p2p_group_messages`) | yes |
| 30 | HumanityOS: no friends list on the server; passes checked and not stored | `docs/reference/cryptography.md` (Friendship passes v2), `src/relay/handlers/friend_passes.rs` | yes (the table's description; the relay's storage has no friends table) |
| 31 | HumanityOS: no email, no phone number stored | `src/relay/storage/mod.rs` (no such column) | yes |
| 32 | HumanityOS: new members hidden online and unlisted until they choose | `src/relay/storage/members.rs` (`join_server_on`: `hide_presence` 1, `last_seen` NULL; directory only for `listed`) | yes |
| 33 | HumanityOS: photos and videos cleaned on the device by default; files in private chats encrypted on the device | `src/net/file_clean.rs`, `web/shared/upload-clean.js`; `docs/reference/cryptography.md` (Encrypted attachments) | yes |
| 34 | HumanityOS: messages padded to fixed sizes | `docs/reference/cryptography.md` (DM size padding; `src/net/dm_pq.rs` `DM_PAD_BUCKETS`) | yes |
| 35 | HumanityOS: export and erase cannot be switched off by a server operator | `src/relay/features.rs` (never-gated list: `account_delete`; the export route) | yes |
| 36 | HumanityOS: server backups encrypted | `docs/reference/cryptography.md` (Backups at rest), `src/relay/storage/backup_crypto.rs` | yes (from the table and the file's existence) |
| 37 | HumanityOS: addresses not in the database; web server logs at most three days | `docs/admin/SELF-HOSTING.md` (Privacy); `/etc/logrotate.d/nginx` on united-humanity.us (`daily`, `rotate 2`, read 6 November 2026) | yes |
| 38 | united-humanity.us has no Tor onion address | SSH to the server, 6 November 2026: no `tor` program, service inactive | yes |
| 39 | At the moment of sending, the server knows the sender and the recipient key | `deposit_dm(state, my_key, to, ...)`: the put arrives on the sender's signed-in connection | yes |
| 40 | The row-order gap, and that it is fixed in the code | Before: `src/relay/storage/dms.rs` (AUTOINCREMENT id), `web/chat/chat-ui.js` and `src/gui/pages/chat.rs` (the two puts back to back). After (BUG-259, 2026-11-06): dms.rs WITHOUT ROWID, per-mailbox numbers from the day; `src/net/self_mail.rs`, `web/shared/self-mail.js` | yes (fixed in the code 6 November 2026); the bullet comes out when the release carrying it is live (landing step 1), so the page never ships "We are fixing it" |
| 41 | Every message carries your signature, so the recipient can prove you wrote it; reports hand over a readable copy by choice | `docs/reference/cryptography.md` (Reports row) | yes |
| 42 | First key trusted, changes warned; comparing in person not built | `docs/design/key-cards.md` (Known limits) | yes |
| 43 | A browser's push service sees when and how big, never the words | `docs/reference/cryptography.md` (Web Push) | yes |
| 44 | Fixed 6 November 2026: addresses others hand the server are checked as it connects, public internet only | BUG-255 in `docs/BUGS.md`; `src/relay/outbound.rs`; deployed: the "Deploy to VPS" run for its commit succeeded and the live relay restarted about 19:07 Pacific | yes |
| 45 | The coming items (badge, links, app lock, disk check, disappearing messages) | `docs/design/outbox-and-inbox.md` (7.3, 7.6, 15.1, 15.3 to 15.5); `docs/PRIORITIES.md` arc I | yes (planned, not built: the post says "coming") |
| 46 | "No published rules yet" for HumanityOS's legal requests | a search of the docs for law-enforcement pages, 6 November 2026 | yes |
| 47 | The short version fits Bluesky (300), X (280) and Mastodon (500) with its link | 222 characters and graphemes before the link (counted with Node's `Intl.Segmenter`); X counts any link as 23 (X-21); Bluesky's 300 graphemes take the link as written (BS-16): the planned link, https://united-humanity.us/library#who-can-read-your-messages, is 61 characters, so 222 + 1 + 61 = 284, which fits | **recount once the link is final** |
