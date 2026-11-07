# DRAFT, NOT POSTED: who can read your messages, HumanityOS and 13 other platforms (6 November 2026)

**Status: a draft for the operator to approve. Nothing has been posted anywhere.** Drafted with AI
(Claude) on 6 November 2026, at the operator's suggestion ("I feel like we could make a post about
HumanityOS' privacy vs all other platforms."). Rename the file to its posting date when it is
approved.

Every claim about another platform comes from the dated research note
[`docs/reference/findings/2026-11-06-social-platform-apis.md`](../../reference/findings/2026-11-06-social-platform-apis.md),
whose sources (each platform's own pages) were read on 6 November 2026; the finding numbers (MA-17,
BS-18 and so on) in the fact-check list at the end point into it. Every claim about HumanityOS was
checked against the code on 6 November 2026, not only against
[`docs/outreach/privacy_by_architecture.md`](../privacy_by_architecture.md) and
[`docs/reference/cryptography.md`](../../reference/cryptography.md); where those overclaim, the post
says the narrower thing (notes, section B). Words: "recovery phrase", "no sign-up" (CLAUDE.md,
account words). No em dashes.

**Before posting, three things the operator decides** (notes, section A): where the long version
lives (it needs a page with a link before the short version can point to it); whether the row-order
gap below is fixed first (then its line comes out) or posted as it stands; and whether to post at
all before the fact-check list's two open lines are settled.

---

## The post (the long version, for a page on united-humanity.us)

### Who can read your private messages? HumanityOS and 13 other platforms, compared

*As of 6 November 2026.*

When you send a private message, two questions matter more than any promise in a privacy policy.
Who, besides the person you wrote to, can read it? And when a court or the police ask for it, what
can the company actually hand over? A company can only hand over what it can read, and only keep
what it chose to collect.

We read each platform's own published pages on 6 November 2026 and put the answers side by side,
with HumanityOS's in the same table, its limits included. Platforms change these things (Instagram
ended its end-to-end encrypted messages on 8 May 2026), so every line says "as of" that date, and
our research notes, linked at the end, give each source with its own date.

#### Private messages: who can read them (as of 6 November 2026)

"End-to-end encrypted" means only the people in the conversation hold the key: the company in the
middle carries the message but cannot open it.

| Platform | End-to-end encrypted? | On by default, or switched on? | Who else can read the rest |
|---|---|---|---|
| HumanityOS | Yes: every direct message and every group chat | Always on: there is no unencrypted way to send one | Nobody else can read what they say. Channels on a server, even members-only ones, are not encrypted this way: whoever runs that server can read them |
| Mastodon | No | | The people who run your server and the other person's server |
| Bluesky | No (in 2024 it said encryption would come later) | | Bluesky, which can access them for trust and safety work |
| X | Only in X Chat | You choose X Chat; older direct messages are not encrypted | X, for older direct messages (X says it cannot read X Chat) |
| Facebook Messenger | Personal chats: yes | On by default, still being rolled out; not for chats with businesses or Pages | Meta, for chats with businesses and Pages. Group chats: Meta's own pages disagree |
| Instagram | No: it ended on 8 May 2026 | | Meta |
| Threads | Unclear: no Meta page we found says | | Unclear |
| YouTube | No | | YouTube, which may review messages against its rules |
| Reddit | It makes no claim of it | | Reddit, which collects private messages and chats |
| LinkedIn | It makes no claim of it | | LinkedIn, which scans messages for its own tools |
| Discord | Text: no. Voice and video calls: yes | Calls: always on, since 2 March 2026 | Discord, for text messages |
| Telegram | Only "Secret Chats" | You start one yourself: one to one, on one device | Ordinary chats are not end-to-end encrypted and are kept on Telegram's servers |
| Rumble | It has no private messages that we found | | |
| Patreon | No | | Patreon, which collects and moderates messages |

#### When the law asks (as of 6 November 2026)

Each company's own published rules for requests from the police and courts, as we read them on 6
November 2026. Most describe United States legal process, and the kinds of legal paper they name
differ in how hard they are to get.

| Platform | To get what your messages say | To get your account details and network addresses |
|---|---|---|
| HumanityOS (united-humanity.us) | No published rules yet. Direct messages and group chats: the server holds only sealed envelopes it cannot open, and none older than 30 days unless its operator chose a longer time. Channel messages: readable | Each member's public key, display name and joining date, and whatever they chose to publish (a profile, posts, channel messages). No email address, no phone number, no friends list. Network addresses: only in the web server's logs, kept at most three days |
| X | A search warrant, for what X can read (it says it cannot read X Chat) | A subpoena, a court order or other valid legal process |
| Meta (Facebook, Messenger, Instagram, Threads) | A search warrant. For an end-to-end encrypted chat it says it cannot give what was said, but still gives logs and addresses | A subpoena for basic records, including a recent login address; a court order for message records, such as who wrote to whom, and addresses |
| Google (YouTube) | A search warrant | A subpoena for subscriber details and some network addresses; a court order for other records |
| LinkedIn | A search warrant; it says a civil subpoena or order is not enough | A subpoena for basic details and login addresses; a court order for who messaged whom, and when |
| Bluesky | A court order, for direct messages | Subpoenas, warrants and court orders (no breakdown published) |
| Discord | The legal process United States law requires (no breakdown published) | The same |
| Telegram | It says it has disclosed no user messages to anyone, governments included | A network address and phone number, on a valid judicial order about a suspect; it keeps such records up to 12 months |
| Mastodon | Whatever the law where each server is run requires (mastodon.social: German law) | The same; mastodon.social keeps your latest login address up to 12 months |
| Reddit | Not read: its page for law enforcement would not let our research tools read it | Not read |
| Rumble | A subpoena, court order, government request or other legal process (no breakdown published) | The same |
| Patreon | Subpoenas, court orders and warrants (no breakdown published) | The same |

#### What the tables say, in plain words

Messages that only the people in them can read are still rare. Of these thirteen platforms, three
offer it for private messages: X in X Chat, Messenger in personal chats (by default, still rolling
out), and Telegram in Secret Chats, which you switch on yourself. Discord encrypts every voice and
video call that way, and that cannot be turned off. Each of those is real work worth credit.

Several platforms are also straightforward about their limits, which matters as much. Mastodon's own
documentation says plainly that it is not an encrypted messenger, and names apps that are.
LinkedIn says a civil subpoena is not enough to get what your messages say. Bluesky says it gives
direct messages to others only when a court orders it. Telegram says it has disclosed no user
messages to anyone, governments included. X, Meta and Discord say they tell you when someone asks
for your data, unless the law forbids it.

Everywhere else, the platform keeps a copy of your private messages that it can read, and the
platforms that publish rules for the police (X, Meta, Google, LinkedIn and Bluesky) say they hand
message content over on a search warrant or a court order. That is not a scandal; it is what
keeping a readable copy means. The only way to make sure a copy cannot be handed over is never to
hold one that can be read.

#### What HumanityOS does differently

HumanityOS is free, open-source software. There is no company in the middle: anyone can run a
server, and each server answers to the law where it runs. What a server can hand over is limited by
what the software lets it hold:

- **Every direct message and group chat is end-to-end encrypted, always.** The server refuses to
  store a direct message that is not in the sealed format, so an old or faulty app cannot quietly
  send one in the clear.
- **The server does not write down who sent a direct message.** It keeps each sealed envelope
  addressed only to its recipient, with the day it arrived and no sender, and deletes it after 30
  days by default (you can clear yours sooner). Your history is kept on your own devices, encrypted.
- **No friends list on the server.** A friendship is a signed pass that your own device keeps and
  shows when you send a message; the server checks it and keeps no list of who is friends with whom.
- **No sign-up.** No email address, no phone number, no password stored on any server. Your account
  is a key made on your own device, backed up by a 24-word recovery phrase that you keep.
- **Hidden until you choose.** New members are not shown as online and are not listed in the member
  directory until they choose to be.
- **Your photos keep your secrets.** Photos and videos lose their hidden location and camera details
  on your own device before they leave, unless you turn that off, and files sent in private chats are
  encrypted on your device first.
- **Your message lengths stay private.** Messages are padded to a few fixed sizes before they are
  sealed, so a one-word reply and a long paragraph look alike.
- **Your data, your buttons.** You can download everything a server holds about you, or erase your
  account yourself, and no server operator can switch either button off. Server backups are
  encrypted.

#### What HumanityOS cannot protect you from

A privacy post that hides its own limits is just marketing, so here are ours.

- **Your network address while you are connected.** Any server can see the address of a device
  connected to it; that is how the internet works. HumanityOS does not keep addresses in its
  database, and the web server's logs keep them at most three days. Server operators can add a Tor
  "onion" address, which hides yours from the server when you connect through Tor, but
  united-humanity.us does not have one yet (checked 6 November 2026), and that door does not yet
  serve the website.
- **The moment you send.** The server you are connected to knows, at that moment, that it is you
  sending and whom the sealed envelope is for. It does not write that down, but a server changed to
  record it could, from then on.
- **A gap we found while checking this post.** The order in which the server stores envelopes can
  pair a message with the sender's own copy of it, stored right after it, so a copy of the database
  could show who wrote to whom on which day, for messages still on the server. We are fixing it.
- **Group chats show who is talking.** What is said in a group is sealed, but the server that carries
  a group can see its name, who is in it, and who posted when.
- **The other person.** Whoever you write to can screenshot, forward or report your messages, and
  because every message carries your signature, they can prove you wrote it.
- **Your own device.** Anyone holding your unlocked device, or with harmful software on it, can read
  what you can read. Protect your device and put a passphrase on your key.
- **The first time you meet someone.** The app trusts a person's key the first time it sees it and
  warns you if it ever changes, but a way to compare keys in person is not built yet.
- **Notifications in a web browser.** If you turn them on, your browser maker's notification service
  sees when a notice arrives and how big it is, never the words.
- **Public is public.** A post in a public channel is public, like anywhere else, and once it has been
  copied to other servers or devices, nobody can delete every copy.

#### What is still coming

- **A plain badge on every message** saying who else can read it (its words are being tested on
  people new to the app first), once the Inbox brings messages from other platforms beside your own,
  with your copies kept on your device and a one-tap choice to keep them, keep them for a while, or
  let them go.
- **Every link showing the site it really goes to**, with look-alike names flagged and a "Where does
  this go?" check that follows a shortened link without opening it.
- **An app lock**, a gentle check that your computer's disk is encrypted, and **disappearing
  messages** (permanent by default, a timer always one tap away).
- **Fixed on 6 November 2026:** the server could be made to send requests into its own network
  through a notification address someone gave it. Now every address someone else hands the server is
  checked at the moment it connects, and only the public internet is allowed.

#### Check it yourself

Everything above is open source, so none of it has to be taken on trust: the code, the database
layout and our research notes are public.

- The project: united-humanity.us
- The code: github.com/Shaostoul/Humanity
- Our research notes on the 13 platforms, with every source and its date:
  github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-11-06-social-platform-apis.md

*Published [date]. Every comparison is as of 6 November 2026.*

---

## The short version (for Mastodon, Bluesky and X; 222 characters before the link)

Who can read your private messages? We compared HumanityOS with 13 platforms, from each one's own
published rules as of 6 November 2026: encryption, and what each hands over when the law asks. Our
own limits are in it too. [link to the long version]

---

## Notes for the operator (not part of the post)

### A. Before posting

1. **Where the long version lives.** It needs its own page with a link before the short version can
   point to it (share-safe: a repost must still lead somewhere). Two options: a Library page (a row
   in `data/library/catalog.json`, then `node scripts/build-library.js`, giving a link like
   `united-humanity.us/library#...`), or a Devlog card. Your call.
2. **The row-order gap** (the third limit). Found while checking the post: `src/relay/storage/dms.rs`
   numbers mailbox rows in arrival order (`id INTEGER PRIMARY KEY AUTOINCREMENT`), and both apps send
   a direct message's recipient copy and the sender's own copy one right after the other (the
   website, `web/chat/chat-ui.js`: `ws.send(built.recipientPut)` then `ws.send(built.selfPut)`; the
   desktop app, `src/gui/pages/chat.rs`: `client.send(&self_put)` then the recipient copy; control
   messages likewise, `src/engine/dm.rs`), both padded to the same size. So a copy of the database
   usually shows two neighbouring envelopes of the same size, one addressed to the recipient and one
   to its sender, which pairs them. If the fix lands before posting, delete that bullet; if not,
   keep it, and change "We are fixing it" to what is true then: the post's promise is honesty about
   limits.
3. **Nothing here names a person or attacks anyone.** It credits the platforms that encrypt, and says
   plainly that keeping a readable copy is not a scandal. The Discord court-order story that opens
   `privacy_by_architecture.md` is left out on purpose: it is not in the dated finding, and this post
   cites only what the finding read.

### B. What the source documents overclaim (so the post says the narrower thing)

Checked against the code on 6 November 2026; each is for whoever next edits those documents.

- `docs/outreach/privacy_by_architecture.md`, "There is no conversation graph to reconstruct" (and
  the same in `docs/reference/cryptography.md`'s DM metadata row and `src/relay/storage/dms.rs`'s
  header): the row-order pairing in A.2.
- Same document, "Groups run entirely on encrypted, signed data the server cannot read": what is said
  is sealed (`src/net/group_e2ee.rs`), but the relay's own tables hold each group's name in plain text
  (`p2p_groups.name`), its member list (`p2p_group_roster`), and each message's author and time
  (`p2p_group_messages.author_fp`, `created_at`), in `src/relay/storage/mod.rs`.
- Same document, "every image uploaded to a HumanityOS server is stripped ... before the file ever
  touches our disk": the server's picture strip gives back the original bytes when it cannot read a
  picture's structure (`src/relay/upload_rules.rs` `finish_picture`), a person may choose to keep a
  file's details (`keep_metadata`), and a location written inside a video stream itself is not
  caught. The app cleans on the device first and refuses an unchecked file for a public place, so the
  post says "on your own device before they leave, unless you turn that off".
- Same document, the Tor onion door: it reaches the server's interface and chat, not the website
  (`docs/reference/cryptography.md`, Transport IP privacy), and united-humanity.us has none: checked
  over SSH on 6 November 2026, no `tor` program installed and the service inactive.
- Same document, "Certificates do not yet expire ... cannot be revoked by the server": out of date the
  other way. Friendship passes can be withdrawn since v0.1466.0 (`cert_revoke`), and by design still
  have no end date.
- Same document, "we cut that to two days": the live rotation is daily with two old files kept
  (`/etc/logrotate.d/nginx`: `daily`, `rotate 2`, read on 6 November 2026), so up to three days. The
  post says "at most three days".
- `docs/SECURITY-CADENCE.md`'s threat-model snapshot (2026-05-20) still says an operator can be made
  to hand over "who messaged whom, when"; that predates the sealed-sender mailbox of 2026-08-23.
- `docs/reference/cryptography.md` has no row for group messages themselves (AES-256-GCM under a
  per-epoch group key sealed to each member with ML-KEM-768, `src/net/group_e2ee.rs`), and the
  relay's `src/relay/storage/groups_p2p.rs` header still names XChaCha20-Poly1305 for them.

### C. "Not much else we can do to further protect users at this point?"

There is more, and some of it is worth doing. Already built or designed is left out: the sealed
mailbox, friendship passes, the onion door, photo cleaning, padding, export and erase, encrypted
backups, the scam warnings, the protected setup, and the Outbox design's section 15 (the link check,
the notification layering, disappearing messages, the app lock, the disk check). Checked against
`docs/PRIORITIES.md`, `docs/SECURITY-CADENCE.md` and `docs/BUS-FACTOR.md` on 6 November 2026.

| # | Protection | What it protects against | Rough size | Already planned? |
|---|---|---|---|---|
| 1 | **Fix the mailbox's row order** (A.2): rows kept in an order that does not follow arrival, for example keyed by recipient and a per-mailbox count, and the sender's own copy sent apart from the recipient's | A seized or leaked database pairing who wrote to whom | Small: the relay's mailbox table and its paging, plus a test that two puts in a row cannot be paired | No (new, found today) |
| 2 | **Hide a group's metadata from its server**: the group's name sealed like its messages, the author inside the sealed part, times coarsened like the mailbox's | The server learning a group's name, members and who posts when | Medium to large: the group object format on both apps and the relay | No |
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

### D. Fact-check list

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
| 40 | The row-order gap | `src/relay/storage/dms.rs` (AUTOINCREMENT id), `web/chat/chat-ui.js` and `src/gui/pages/chat.rs` (the two puts back to back) | yes; **"We are fixing it" is a promise: true only once the fix is scheduled** |
| 41 | Every message carries your signature, so the recipient can prove you wrote it; reports hand over a readable copy by choice | `docs/reference/cryptography.md` (Reports row) | yes |
| 42 | First key trusted, changes warned; comparing in person not built | `docs/design/key-cards.md` (Known limits) | yes |
| 43 | A browser's push service sees when and how big, never the words | `docs/reference/cryptography.md` (Web Push) | yes |
| 44 | Fixed 6 November 2026: addresses others hand the server are checked as it connects, public internet only | BUG-255 in `docs/BUGS.md`; `src/relay/outbound.rs`; deployed: the "Deploy to VPS" run for its commit succeeded and the live relay restarted about 19:07 Pacific | yes |
| 45 | The coming items (badge, links, app lock, disk check, disappearing messages) | `docs/design/outbox-and-inbox.md` (7.3, 7.6, 15.1, 15.3 to 15.5); `docs/PRIORITIES.md` arc I | yes (planned, not built: the post says "coming") |
| 46 | "No published rules yet" for HumanityOS's legal requests | a search of the docs for law-enforcement pages, 6 November 2026 | yes |
| 47 | The short version fits Bluesky (300), X (280) and Mastodon (500) with its link | 222 characters and graphemes before the link (counted with Node's `Intl.Segmenter`); X counts any link as 23 (X-21); Bluesky's 300 graphemes take the link as written (BS-16), so a link of up to about 75 characters fits there | **no: recount once the link is known** |
