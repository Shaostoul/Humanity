# Unpairable direct messages: your own copies off the server, and cover traffic

**Status: planned, nothing built (2026-11-07).** The operator, 2026-11-07, asked whether ending the
pairing for good should keep your own copies off the server entirely or hide real messages among
dummy ones: "I imagine we want both." This document plans the whole feature for both routes
(CLAUDE.md: "Plan the whole feature, build it in rungs, keep performance in mind throughout"), with
what each costs and what each protects against. Rung 1 was weighed for building in the same piece
of work and was not built; why is at the end.

Where it starts: `docs/BUGS.md` BUG-259 (fixed 2026-11-06, live 2026-11-07). The relay's DM mailbox
now keeps rows by mailbox, not by arrival, and both apps send your own copy of a message a random
20 seconds to 2 minutes after the recipient's (`src/net/self_mail.rs`, `web/shared/self-mail.js`).
What that left, said in `docs/outreach/how_your_privacy_is_built.md`:

1. At the moment you send, the server knows it is you and whom the envelope is for: the recipient's
   copy arrives on your signed-in connection. It writes nothing of that down, but a server changed
   to record it, or ordered to, could.
2. Your own copy still goes to the same server soon after. On a server so quiet that nothing else is
   written in those minutes, its write-ahead log and continuous backup could still suggest a pair.
3. Someone watching the server's network (or yours) sees when you send, and how much.

Route A ends 2 outright and changes nothing about 1 and 3. Route B is what reaches 1 and 3.

## What exists today (read 2026-11-07)

- **The mailbox** (`src/relay/storage/dms.rs`; `handle_dm_put`, `handle_dm_fetch`, `handle_dm_purge`
  in `src/relay/handlers/msg_handlers.rs`). A row is `(id, to_key, content, received_day)`, with no
  sender. A put is pushed at once to every connected device of its addressee (`DmNew`); a device
  pages its mailbox 200 rows at a time after the last number it saw (`src/net/mailbox_fetch.rs`).
  Envelopes are capped at 131,072 bytes and expire after `dm_mailbox_ttl_days` (30 by default,
  1 to 365). The rate limit (`src/relay/handlers/dm_rate.rs`) is a token bucket per sender (8 at
  once, one a second) and runs only on recipient copies; your own copies are not limited.
- **The envelope** (`src/net/dm_pq.rs`). Inside: `{v:2, from, to, ts, text, sig}`, signed with
  Dilithium3, where `from` and `to` are whole public keys in hex (3,904 characters each) and the
  signature is 4,412 characters of base64. So an ordinary short text is about 12.3 KB inside, padded
  to the 16,384 bucket (`DM_PAD_BUCKETS = [256, 1024, 4096, 16384, 32768]`), and about 23 KB on the
  wire once sealed (ML-KEM-768, then AES-256-GCM) and base64ed into JSON.
- **Your own copy.** Every `dm_put` addressed to your own key waits 20 s to 2 min: your copy of a DM,
  of a follow or unfollow, a friendship pass, a household permit, a file offer and its answers, a
  choice note. Notes only you read (blocks, task notes, the allow list) go at once. It exists so your
  other devices learn what you did; nothing else copies history between your devices.
- **Your devices.** Every device of yours derives the same keys from the same recovery phrase ("Link
  New Device" carries the phrase itself: `openLinkDeviceModal` in `web/chat/crypto.js`,
  `device_link_payload_json` in `src/net/identity.rs`). There are no keys of a single device, so one
  mailbox serves all of them. Per-device keys are designed, not built
  (`docs/design/client_side_identity_keys.md`); so is the device mesh with backups through the
  server and sync on a home network (`docs/design/device-mesh.md`).
- **Device to device.** Big files go over a WebRTC data channel, signalled through the relay
  (`handle_webrtc_signal`, forwarded only while the other side is connected, never queued) and by
  default carried by the relay's own forwarder (`src/relay/call_forwarder.rs`). Both devices must be
  online; nothing is stored. The website's own-device "Sync" button (`web/chat/chat-p2p.js`,
  `buildSyncBundle`) carries settings-like data, not DMs, needs both pages open, and no production
  code opens its channel. The desktop refuses own-device offers (`src/net/webrtc.rs`, `on_offer`).
  Own-device links get only the server's STUN entry from `/api/turn-credentials`, no relay
  allocation, so two devices behind strict routers may not connect at all.
- **Encrypted storage on the server already exists**: `sync_save`/`sync_load` (settings, notes,
  garden, AES-GCM under a key from your recovery phrase) and `/api/vault/sync`
  (`src/relay/storage/vault_sync.rs`, one opaque blob per key).
- **Waking a sleeping device**: none. Web Push carries a fixed "New message" notice, sent only when
  no device of yours is connected anywhere, and the service worker (`web/shared/sw.js`) only shows
  it. The desktop has no background service.
- **Federation**: one identity works on many servers at once and has no home server, but DMs are not
  federated: an envelope sits on the server the sender was connected to
  (`docs/design/federation-activation.md`).

## Route A: your own copies kept off the mailbox

### The idea

Your other devices need to learn what you sent, but they do not need to learn it message by message,
and the server does not need a copy of each one sitting beside the recipient's. So the self copy
stops being an envelope in the mailbox. Instead each device keeps a **sent log**, and on a fixed
schedule, whether or not you sent anything, it uploads one **segment** of that log: a fixed-size
block, encrypted under a key only your devices hold, holding what you sent since its last segment
(a DM's recipient, time and words; a follow; a pass given; a file offered; a choice made). Your other
devices fetch segments when they are awake and merge them into their stores (`src/net/dm_store.rs`,
`web/chat/chat-dm-store.js`), exactly as they merge self copies today.

A segment carries nothing that ties it to any one message: same size every time (one bucket, 4 KB),
sent on the clock rather than on a send, encrypted so the server sees only noise. A burst bigger than
one segment spills into the next ones instead of making a bigger segment, so even volume does not
show. The server keeps segments for the same window as the mailbox and drops them after.

This works while a device is asleep without any wake mechanism: segments wait on the server like
mail does. A device asleep longer than the window misses what it did not fetch, as it does today.

### The rungs of route A

- **A1, the sent log in segments** (the first rung of the whole feature). A new relay store
  `sent_segments (owner_tag, device_tag, period, content)`, where `owner_tag` is a BLAKE3 tag of your
  key under a key from your recovery phrase (so a copy of the database does not even list whose log
  it is), `device_tag` a random tag per device, `period` the hour, and `content` the 4 KB block.
  Handlers to put one (signed in, one per device per period, refused if not exactly 4 KB) and to
  fetch everything for an `owner_tag` after a period. Both apps: a scheduler (one segment per hour
  of being connected, plus one on the way out if the last one is more than ten minutes old), the
  merge, and every `dm_put` to your own key replaced by an entry in the log. `is_self_put` and the
  20 s to 2 min wait (`src/net/self_mail.rs`, `web/shared/self-mail.js`) go, with their tests. No
  compatibility code: self copies already in a mailbox expire with it.
- **A2, the log on a server you choose.** Segments go to one server you pick (a Settings choice, the
  server you are on by default), not to whichever server each conversation is on. One server's
  database then holds your recipient copies and another holds your activity, and neither holds a
  pair.
- **A3, store and forward through a device of your own.** An always-on device of yours (the desktop
  app left running, or a home server, `docs/design/device-mesh.md`) keeps the log itself, and your
  other devices sync with it over a data channel carried by the relay's forwarder. Nothing is stored
  on any server. Needs: own-device WebRTC on the desktop (refused today), a relay allocation for
  own-device links, and the device being on whenever another wants to sync; when it is not, A1's
  segments are the fallback. Kept for later because it only removes something A1 already made
  harmless (opaque fixed-size blocks on a schedule).

### What route A protects against, and what it does not

| Who | Before | After A1 |
|---|---|---|
| A copy of the database (a court order, a breach, a backup with its key) | No pairing in rows since BUG-259, but recipient copy and self copy on the same server, minutes apart | No self copy at all. Rows addressed to you are mail you received; your segments are fixed-size noise on the hour, under a tag that does not name you |
| The write-ahead log and the continuous backup | On a very quiet server, page order can suggest a pair | Nothing to pair: the recipient copy is the only envelope a send writes |
| The server itself, at the moment you send | Sees the recipient copy arrive on your signed-in connection | Unchanged (route B, rung B3) |
| Someone watching the network | Sees when you send (a 23 KB frame) and your copy minutes later | Sees the recipient copy only; still sees when you send (route B, rung B2) |
| A court order to intercept live | Who wrote to whom, as it happens | Unchanged (route B) |

### Costs of route A

Assumed for every table in this document: each person has two devices (a phone and a computer), each
connected eight hours a day, and sends 50 DMs a day; mail is kept 30 days. A sent-log entry is about
300 bytes (recipient fingerprint, time, words), so a 4 KB segment holds about 13 entries.

Per device per day: 8 segments up (33 KB), the other device's 8 down (33 KB). Today the self copies
cost each person 50 envelopes up (1.17 MB) and each other device 1.17 MB down.

| People | Server: new storage per day, A1 | Server: kept over 30 days, A1 | The self copies A1 removes, per day |
|---|---|---|---|
| 10 | 0.66 MB | 20 MB | 11.7 MB |
| 10,000 | 660 MB | 20 GB | 11.7 GB |
| 1,000,000 | 66 GB | 2 TB | 1.17 TB |

A1 cuts what the mailbox stores by almost half (the recipient copies stay) and costs each device
about 66 KB a day. Its price is in time, not bytes: your other devices learn what you sent up to an
hour later (plus however long they slept), where today it is two minutes. That is the operator's
call (questions, below).

## Route B: cover traffic

### The idea, and the designs it draws on

Make the stream of envelopes leaving each device look the same whether you are sending or not, so
that neither someone watching the network nor (with the later rungs) the server can tell when you
wrote, how much, or to whom.

- **Loopix** (Piotrowska, Hayes, Elahi, Meiser, Danezis, "The Loopix Anonymity System", USENIX
  Security 2017; arXiv 1703.00536): each client sends at the times of a Poisson process, and a real
  message waiting to go takes the next slot, otherwise a dummy goes. Some dummies are dropped where
  they land, some loop back to the sender to detect a network that delays or drops traffic. Service
  providers hold mail for clients that are offline. The Poisson schedule is what makes the stream
  memoryless: an observer learns nothing from the gap since the last envelope.
- **Vuvuzela** (van den Hooff, Lazar, Zaharia, Zeldovich, "Vuvuzela: Scalable Private Messaging
  Resistant to Traffic Analysis", SOSP 2015) and **Karaoke** (Lazar, Gilad, Zeldovich, OSDI 2018):
  every client sends one fixed-size request every round, real or not, to "dead drops" whose
  addresses come from a secret the two people share, and the servers add dummy requests drawn from a
  distribution chosen for differential privacy, so what an observer of the dead drops sees is
  provably close to what it would see without you.
- **Groove** (Barman, Kol, Lazar, Gilad, Zeldovich, "Groove: Flexible Metadata-Private Messaging",
  OSDI 2022): the closest to our problem. It drops the assumption of one always-online device: a
  person can have several devices and message people who are offline, by "oblivious delegation" to
  an untrusted service provider that takes part in the rounds for them. Its prototype reports about
  32 seconds of latency for a million users with 50 contacts each, about 100 MB a month on a Pixel 4,
  and about 50 mW more battery than an idle phone (read 2026-11-07 from the OSDI page).
- **Privacy Pass** (Davidson, Goldberg, Sullivan, Tankersley, Valsorda, "Privacy Pass: Bypassing
  Internet Challenges Anonymously", PETS 2018; standardized as RFC 9576 to RFC 9578, 2024): tokens a
  server signs blind while it knows who you are, which you later spend on a connection that does not
  say who you are. The server can count and limit without learning who spent what.

None of these is copied in code; they are the published methods each rung below follows.

### The rungs of route B

- **B1, a smaller envelope** (not cover traffic, but what makes it affordable). Inside the envelope
  `from` and `to` become key fingerprints (the recipient already holds the sender's checked key card,
  `docs/design/key-cards.md`) and the signature binary; a short text then fits the 4,096 bucket, about
  5.3 KB on the wire as a binary frame instead of 23 KB. It also cuts what the mailbox stores today
  by three quarters. A format change on both apps and the relay at once.
- **B2, a steady stream while connected (Loopix's send schedule).** Each connected device sends one
  fixed-size slot at Poisson times, one every 20 seconds on average. A real recipient copy waiting
  takes the next slot; a longer message takes several; otherwise a dummy goes, sealed and padded
  like a real one, addressed to a cover address the server drops on arrival. Against someone watching
  the network this hides when, how often and how much you send, inside the hours you are connected.
  The server can still tell a dummy from real mail (it drops them), so B2 does nothing against the
  server; it costs bandwidth and no storage.
- **B3, sending without saying who you are.** The recipient copy goes on a separate connection that
  is not signed in (through the server's onion address where the app can reach it, so the network
  address does not join the two connections), paying with a one-time Privacy Pass token instead of
  your identity. Tokens are issued blind on your signed-in connection, a day's worth at a time, which
  keeps the rate limit. The server then stores mail it cannot attribute. Two consequences to accept
  first: who may write to you (strangers' daily knocks, `src/relay/handlers/reach.rs`) can no longer
  be checked by the server against the sender, so it moves into the envelope and your app, as the
  friendship pass already works; and the token schemes in use today (VOPRF, blind RSA) are not
  post-quantum: a future quantum computer could forge tokens (spam) but could not unmask anyone,
  since the blinding hides who spent a token without any computational assumption.
- **B4, dead drops instead of mailboxes, and a fetch stream.** Mail is addressed not to your key but
  to a drop address derived from a secret you share with each correspondent and the day (the
  Vuvuzela and Pung pattern), and B2's dummies go to random drop addresses that look the same, kept
  for the same window. A copy of the database then shows neither recipients nor which rows are real.
  Fetching your drops would reveal which are yours, so each device also fetches on a schedule, a fixed
  number of drop addresses each time, real and random. A stranger's first message still needs one
  public drop per person (a knock box), as today's mailbox is. Only here does cover traffic cost
  storage, which is why it is last.
- **B5, loops.** A share of each device's dummies come back to its own fetch, so a server that delays
  or drops traffic to watch what changes is noticed (Loopix's loop cover traffic).

### What route B protects against, and what it does not

- **Someone watching the network** (an internet provider, a hostile Wi-Fi, a tap at the server's
  host): from B2, a steady stream of identical frames while you are connected; when you sent, to how
  many people and how much cannot be read from it. They still see when you are connected and that you
  use the server (unless through the onion address).
- **The server, or anyone ordered to intercept it live**: from B3 it receives mail it cannot tie to a
  sender; from B4 it cannot tell real mail from dummies or whom a row is for.
- **A copy of the database**: route A already removes the pairing; B4 also removes the recipients.
- **Not protected, at any rung:** a compromised device; someone who can watch both you and your
  correspondent for weeks and match the hours you are connected (cover traffic while connected does
  not hide when you are connected; long-term intersection attacks are the open problem these papers
  bound but do not end); and a small server. On a server of ten people, the people who could have sent
  a message are few whatever the cover. The protection grows with the number of people sharing it,
  which is what federation and B4's shared drop space are for.

### Costs of route B

B2 at one slot every 20 seconds while connected: 180 slots an hour, 1,440 a day per device. Per
device per day: 33.7 MB with today's 23 KB envelope, 7.6 MB after B1 (5.3 KB), about 230 MB a month,
in the same range as Groove's 100 MB. A single message waits 20 seconds on average for its slot; five
sent at once take about 100 seconds to all leave. At one slot a minute the bandwidth is a third and
the waiting three times longer.

| People | Server: received per day, B2 after B1 | Same, without B1 | Kept, B2 | Kept over 2 days, B4's dummies | Over 30 days, B4's dummies |
|---|---|---|---|---|---|
| 10 | 153 MB | 674 MB | nothing (dropped) | 306 MB | 4.6 GB |
| 10,000 | 153 GB (4.6 TB a month) | 674 GB | nothing | 306 GB | 4.6 TB |
| 1,000,000 | 15.3 TB (about 1.4 Gbit/s around the clock) | 67 TB | nothing | 30.6 TB | 460 TB |

What this says, plainly. B2 is affordable only after B1, and even then a server of 10,000 people
receives several terabytes a month of cover, which is more than many virtual servers include, so its
rate is a setting the server's admin chooses (Server settings), not a constant. B4's dummies cannot be
kept for the full window at a million people on one server: B4 needs either a short window for drop
rows (two days, with longer-term delivery to a person's own devices by route A's segments), a much
lower dummy rate with Karaoke-style server-added noise sized for a privacy bound rather than a fixed
rate, or the load spread across federated servers. Cosmetic it is not, but like a cosmetic effect
(CLAUDE.md) its network footprint is stated before it is built, and it gets a budget test (below).
Phones: B2 runs only while the app is open and connected; a page in the background sends nothing,
which shows when you are connected but not when you write.

## The order to build, and the test that proves each rung

Each test fails on purpose against the code before its rung, and its red is recorded at the test.

1. **A1, the sent log.** Relay: a DM between Alice and Bob through the handlers leaves exactly one
   mailbox row, Bob's, and none addressed to Alice (red today: Alice's copy is there); a segment that
   is not exactly 4 KB is refused; a second segment in one period from one device is refused. Apps:
   a DM sent on device one is in device two's store after device two merges device one's next segment
   (red: no merge); over a simulated day the segments go on the hour and are the same size with 0 and
   with 20 messages sent (red: a segment sent at the moment of a send); a burst of 40 entries spills
   into the next segments rather than a bigger one. Both apps hold to one fixture of a segment's
   bytes.
2. **B1, the smaller envelope.** A short text's sealed envelope falls in the 4,096 bucket in both
   apps, read through one shared fixture (red today: 16,384); the relay's padding check refuses any
   other size for a DM.
3. **B2, the steady stream.** On a simulated clock, an hour's send times of a device with no real
   messages and with 30 are the same in count (within the Poisson spread) and their gaps pass the same
   exponential fit; every frame is the same size; the server drops every dummy before storage (red: a
   real message sent at once). A budget test computes the stream's bytes an hour from the scheduler's
   own constants and fails above the stated budget (1 MB an hour per device after B1), so nobody raises
   the rate without changing the budget in this document.
4. **B3, sending without signing in.** Relay: an envelope put on a connection that is not signed in,
   with a valid token, is stored, and the put's log line and stored row carry no identity; a token
   spent twice is refused; issuing, the server's records hold nothing that matches a token at
   spending (red: the put goes on the signed-in socket). The rule of who may write to whom, moved
   into the envelope, is held by the same tests that hold the friendship pass today.
5. **A2 and A3**, the log on a chosen server and through your own device, each with the test that the
   segments reach the chosen place and nowhere else.
6. **B4 and B5**, dead drops, the fetch stream, loops: a copy of the database after a simulated day
   of real and dummy traffic, given to a test that tries to tell real rows from dummies and to name
   recipients, must do no better than chance (red: today's mailbox, where both are plain).

Rungs 1 and 2 are each a piece of work of their own; 3 and 4 change what people feel (latency, who
may write to them) and come back to the operator first; 6 is a research-sized piece.

## Why nothing was built in this task

Rung 1 (A1) was considered. It is not small: a new relay store and two handlers, a scheduler and a
merge in both apps, five kinds of self-addressed notes moved into the log, and the self-mail queue
and its tests removed. And it changes something people feel, how soon your other devices show what
you sent, from two minutes to up to an hour, which is the operator's to decide. So it is planned
here and not built.

## Questions for the operator

1. **How soon should your other devices show what you sent?** Route A sends your copies on a schedule
   instead of with each message. Recommended: once an hour while a device is connected, and once
   more as the app closes. Every 15 minutes is possible and costs four times the (small) bandwidth;
   more often starts to look like the sends themselves.
2. **Cover traffic's cost.** After the smaller envelope, about 7.6 MB a day per device while the app
   is open, and a server of 10,000 people receiving several terabytes of cover a month. Recommended:
   on by default in both apps with a Settings > Privacy switch that says what it costs; the rate a
   Server settings choice for each server's admin, starting at one slot every 20 seconds.
3. **Sending without signing in (B3)** moves "who may write to you" from the server into your app,
   and uses blind tokens that are not post-quantum (forgeable by a future quantum computer, which
   would mean spam, never unmasking). Recommended: accept both, with a post-quantum token scheme as a
   later rung.
