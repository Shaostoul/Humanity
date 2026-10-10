# Blocking, safety settings, reports and who can see your address

**Status: design, 2026-10-09. Nothing in sections 4 to 9 is built.** Section 3
and every passage marked **[Today]** describe current behaviour, checked by
reading the code on 2026-10-09 (file and function named each time). Every
passage marked **[Proposed]** is a plan. When a piece ships, move its text into
the "Today" column and update the docs listed in section 11.

The operator's request, 2026-10-09, verbatim:

> "We need to give users the tools to protect themselves. We should also
> probably add a kid safe mode that protects kids from online predators and
> scams. I guess kind of for everyone since no one wants to be
> scammed/harassed/abused."

And on voice, from the coordinator the same day: "What's a better way we can
do this? Do we have to go through Google?"

Related: `docs/PRIORITIES.md` TIER 1 items 2, 3 and 4;
`docs/accord/conformance_gaps.md` ("Contact consent cannot be withdrawn");
`docs/design/report-system.md` (an older, mostly unbuilt report design this
document supersedes for direct messages); `docs/design/p2p-groups.md`.

---

## 1. Words used here

- **Relay (the server).** The HumanityOS server program. It passes messages
  between people and stores what it must. Anyone can run one.
- **Identity key.** Your identity is a public key, a long string of hex
  digits, made from your recovery phrase on your own device. There is no
  sign-up: no email, phone number or password held by a company. Display
  names are labels on top of the key and can change; the key cannot.
- **Direct message (DM).** A private message to one person. Since 2026-08-23
  it is **sealed**: encrypted for the recipient with Kyber768 / ML-KEM-768,
  BLAKE3 and AES-256-GCM, so the relay cannot read it, and the sender's
  identity travels **inside** the sealed part, signed with Dilithium3 /
  ML-DSA-65. This is called **sealed sender**.
- **Mailbox.** Where the relay keeps sealed DMs until the recipient fetches
  them. It stores the recipient's key and the sealed blob, and no sender.
- **Following / friends.** You follow someone; when two people follow each
  other they are friends.
- **Friendship certificate ("cert").** A short note, signed by you, saying
  "this person may message me". Your client gives it to your friend; their
  client shows it to the relay with each message to you. The relay checks the
  signature and stores nothing. This "client holds it, relay checks it
  without remembering it" approach is the house pattern for anything that
  would otherwise need a list of who knows whom on the server.
- **Knock.** A DM from someone who holds no certificate from you. Allowed,
  but each sender gets only a few per day.
- **Preimage.** The exact text a signature is made over. Both clients and the
  relay must build it byte for byte the same.
- **KAT (known-answer test).** A test that pins exact inputs and outputs so
  the Rust and JavaScript sides cannot drift apart.
- **Revocation.** Withdrawing a certificate before it runs out.
- **WebRTC.** The browser standard for calls. It tries to connect two devices
  directly (peer to peer). A **data channel** is a WebRTC connection that
  carries data instead of sound.
- **IP address (network address).** The number that routes internet traffic
  to your connection. It reveals your internet provider and usually your
  rough location (city or region).
- **STUN.** A tiny service that tells a device what its public address looks
  like from outside, so WebRTC can try a direct connection.
- **TURN.** A server that forwards a call's packets when a direct connection
  is impossible, or when you do not want the other side to learn your
  address.
- **SFU (selective forwarding unit).** A server that receives each speaker's
  audio once and sends it on to the others in a room.

---

## 2. Summary

**Today's gaps, in one list** (each verified, details in section 3):

1. A friendship certificate can never be withdrawn; unfollowing leaves it
   working.
2. A stranger's knocks are capped at 20 a day per sender key, and a new key
   costs nothing, so the cap limits volume, not access.
3. Native chat has no block. Web's block list hides public posts locally, by
   display name, and does not touch DMs at all.
4. DMs from strangers arrive in the same list as friends' messages, with
   notifications. Nothing marks them as requests on the receiving side.
5. Anyone online can ring you (`voice_call`), and the relay tells the caller
   whether you are online even when you chose to hide that.
6. Anyone online can send you an in-game trade request carrying free text,
   outside every DM limit.
7. A group creator can add anyone to a group without their consent (the
   relay accepts it; shipped clients never do it).
8. The web client answers a data-sync request from **anyone** who opens a
   direct connection to it by sending your calendar, notes, inventory, home
   records and map pins, and merges whatever they send back.
9. Both clients ask Google's STUN servers for their address (native does it
   on every chat connection, voice or not); every voice call and voice room
   shows each participant's address to the others; native loads pictures
   from any website a message names, which shows your address to that site.
10. The report path stores a display name and a reason, with no evidence; the
    native Report button discards the message it was pressed on.

**Recommended approach, in a few lines:**

- **Block** is enforced in two places that keep the "no social graph on the
  server" promise: your own client discards everything from a blocked key on
  every path (DMs, posts, calls, trades, groups, the game), and the relay
  enforces a **certificate** rule: blocking withdraws their certificate, and
  a per-person **"who can reach me"** setting (anyone, or friends only) lets
  the relay refuse strangers outright. The relay never stores who blocked
  whom.
- **Friendship certificates v2** carry a server, a random serial and an end
  date (the household permits' pattern). Withdrawing one puts its random
  serial on a short list the relay checks; the list names nobody.
- **Safety settings** for everyone, plus a **protected setup** for children
  that a parent locks with a PIN. It promises only what software can keep:
  limits and defaults, never age checks.
- **Reports** carry evidence the admins can check: the DMs you choose to
  attach still carry the sender's own signature.
- **Voice** goes through a forwarder built into the relay, so no player sees
  another's address and Google is not asked at all. One UDP port, the
  operator's firewall call.

---

## 3. How things work today (verified in code)

### 3.1 How a direct message reaches someone

**[Today]**

1. The sender's client builds the inner payload `{v:2, from, to, ts, text,
   sig}`, where `sig` is Dilithium3 over
   `hum/dm/v2\n{from}\n{to}\n{ts}\n{text}` (`sig_preimage` and
   `build_signed_inner_ext` in `src/net/dm_pq.rs`; web `pqBuildDmPuts` in
   `web/chat/crypto.js`). It pads it to a size bucket (`DM_PAD_BUCKETS`) and
   seals it twice: once for the recipient, once for the sender's own other
   devices (`seal_v2`).
2. The client sends two `dm_put` frames over its signed-in WebSocket. The
   recipient's copy carries the recipient's friendship certificate when the
   sender holds one (`build_dm_puts` in `src/gui/pages/chat.rs`; web
   `pqBuildDmPuts`).
3. The relay's `handle_dm_put` (`src/relay/handlers/msg_handlers.rs`) refuses
   anything that is not a sealed v2 envelope (`is_v2_envelope`), refuses bots
   and muted members (`is_muted`), then, for mail addressed to someone else
   and from anyone who is not an admin or mod, checks the certificate with
   `verify_friend_cert(&to, my_key, cert)`. Without a valid one the message
   is a knock: it spends one of the sender's `DM_KNOCKS_PER_DAY = 20` for the
   day. Then the Fibonacci rate limiter and the new-account delay
   (`NEW_ACCOUNT_DELAY_SECS = 5` for the first `NEW_ACCOUNT_WINDOW_SECS =
   600`) apply, and `mailbox_put(to, content)` stores the blob
   (`src/relay/storage/dms.rs`). A recipient who is online gets it at once
   (`DmNew`); one who is offline gets a generic push: "You have a new
   encrypted message."
4. The recipient's client opens it, verifies the inner signature
   (`open_verify_dm` in `src/engine/dm.rs`, `parse_verify_inner` in
   `dm_pq.rs`; web `pqOpenDmEnvelope`), and stores it in its local encrypted
   store (`src/net/dm_store.rs`; web `chat-dm-store.js`).

**Does the relay know who is sending?** Yes, at the moment of `dm_put`. The
frame arrives on a socket that proved its key through the identify
challenge, so `my_key` in `handle_dm_put` is the sender. The relay uses it for
the gates and keeps it only in memory: `RelayState.dm_knocks` maps sender key
to (day, count) (`src/relay/relay.rs`, reset on restart), and `rate_limits`
maps sender key to its limiter state. Neither is written to the mailbox or the
database. The code comment above `handle_dm_put` says the same. So the relay
**could** be changed to log sender and recipient pairs; it is written not to.
Every design below is judged against what the running relay must know at
`dm_put` and what it must **store**.

**How knocks are counted.** Per sender, across all recipients, per UTC day,
in memory. Deliberately never per pair: the comment on `DM_KNOCKS_PER_DAY`
says a per-pair counter "would be a social graph again".

**What the recipient sees of a knock.** Nothing different. Neither client
marks a stranger's DM as a request on arrival. Web tells the **sender** once
per partner that their message is "an introduction request" (`knockNoticeShown`
in `web/chat/chat-ui.js`); native says nothing.

### 3.2 Following, friendship and certificates

**[Today]**

- Follow and unfollow are sealed control DMs, `[[hum:follow]]` and
  `[[hum:unfollow]]` (`CTL_FOLLOW`, `CTL_UNFOLLOW` in `src/net/dm_pq.rs`).
  When a follow becomes mutual, each client mints its certificate and sends
  it as `[[hum:friend-cert]]` (`set_follow`, `send_friend_cert`,
  `ingest_control` in `src/engine/dm.rs`; `setFollowLocal`,
  `sendFriendCertTo`, `ingestDmControl` in `web/chat/chat-social.js`).
- The certificate is a base64 Dilithium3 signature over
  `hum/friend/v1\n{issuer}\n{grantee}` (`friend_cert_preimage`,
  `build_friend_cert`, `verify_friend_cert` in `src/relay/core/pq_crypto.rs`;
  web `pqBuildFriendCert`, `pqVerifyFriendCert` in `crypto.js`). No serial,
  no end date, no server.
- The relay checks it at `dm_put` (above) and at `handle_profile_request`
  (friends-only profile fields).
- **Nothing can be withdrawn.** The code says so itself (pq_crypto.rs, "Known
  v1 limitation"): certificates "do not expire and cannot be server-side
  revoked". Unfollowing clears local state and tells the other side, but
  their client keeps your certificate (`certs_from` in `DmStore`) and the
  relay keeps accepting it. The only escape is abandoning the identity key,
  which is also the account and the wallet address.
- The certificate store is **per server**: `DmStore::load(seed, identity,
  server)` keys the whole store, including follows and certificates, by the
  server address.
- Cross-client pinning: the Rust preimage is pinned by
  `friend_cert_roundtrip_and_pinned_preimage` in pq_crypto.rs. **The web
  preimage is not machine-checked**: `scripts/pq-kat.mjs` covers only the
  Dilithium and Kyber key derivation, and crypto.js builds the string inline.

### 3.3 Every way one person can reach another

| Path | Today (verified) | Where | Can a block cover it? |
|---|---|---|---|
| DM | Sealed, delivered to anyone; strangers capped at 20 a day per sender key | `handle_dm_put` | Yes: client discard; relay refusal through certificates and a "friends only" setting (section 4) |
| Follow notice, friend-cert delivery | Sealed control DMs, same gates as DMs | `ingest_control`, `ingestDmControl` | Yes, as DMs |
| Friend code | Owner makes a one-use code valid 24 h; redeemer gets the owner's key, owner is told who redeemed | `handle_friend_code_request`, `handle_friend_code_redeem` (msg_handlers.rs) | Client ignores a blocked redeemer's hello |
| Public channel posts | Broadcast to every member of the channel | `src/relay/relay.rs` chat path | Client hides by key. Cannot stop the post existing |
| Mentions | `@name` in a post; the relay pushes "X mentioned you" to an **offline** mentioned person | `relay.rs` around the `mention_re` push | Client hides; the offline push needs the relay (section 4.5) |
| Replies, reactions, typing | Broadcast with the author's key | `relay.rs` `Reaction`, `Typing` | Client hides by key |
| 1:1 call ring | Forwarded to the target if online, **no gate at all**; caller is told "User is not online." otherwise | `handle_voice_call` | Client ignores; relay refusal with the reach setting |
| Direct-connection offer (`webrtc_signal` `dc_offer`) | Forwarded to anyone online, no gate; **both clients answer anyone** | `handle_webrtc_signal`; web `handleDCOffer` (chat-p2p.js); native `on_offer` (src/net/webrtc.rs) | Client must not answer non-friends (section 3.7) |
| Voice rooms | Anyone may join a channel with voice on; every participant connects directly to every other | `handle_voice_room`; web `chat-voice-rooms.js` | Mute locally; leave. Cannot remove them |
| Group chats (P2P signed-object groups) | Join by pasting an invite ticket; creator may admit or remove anyone; messages are signed by their author | `src/relay/storage/groups_p2p.rs`; `web/chat/chat-groups-p2p.js` | Client hides their messages; leave the group. A creator can remove them |
| Group invite | A ticket string passed by hand or in a DM; no push to the invitee | `createP2pInvite`, `joinP2pGroupByTicket` | Covered by DM blocking |
| Profile view | Anyone can request your public fields by name; friends-only fields need your certificate | `handle_profile_request`, `get_public_profile` (`src/relay/storage/profile.rs`) | Withdrawing the certificate hides friends-only fields. Public fields stay public |
| Member directory `/api/members` | Lists members unless profile privacy says `directory:"unlisted"` | `src/relay/storage/members.rs` | Not per person; use "unlisted" |
| In-chat member list | **Every** registered name with its identity key and DM key goes to every connected client, unlisted or not; presence is masked for hidden members | `broadcast_full_user_list` (`src/relay/handlers/broadcast.rs`), `list_all_users_with_keys` (`storage/pins.rs`) | Not per person |
| Marketplace contact | Rides sealed DMs | (DM path) | As DMs |
| In-game trade request | Sent to anyone online with a **free-text message**, not counted against DM limits; max 10 active trades per sender; "Trade target is not online." otherwise | `handle_trade_request` (msg_handlers.rs) | Client auto-declines; relay refusal with the reach setting |
| Game co-presence | Players within view distance see each other's figure and name; there is no player-to-player chat in the shared world | `src/relay/handlers/game_interest.rs`; `nameplate_labels` (`src/engine/net_route.rs`) | Client can hide the figure and name; it cannot stop them standing there |
| Building near you | Building happens only on your own plot or with your household permit | `permit_lets_in` (`src/relay/handlers/shared_build.rs`) | Already consent-gated. Permits cannot be withdrawn early either (section 5.6) |
| Link and image in a message | Native fetches any `https` image a message names, automatically; web shows "Image (click to load)"; web loads link-preview pictures from the original site | `extract_image_urls`, `resolve_url` (`src/gui/widgets/image_cache.rs`); web `app.js` image placeholder and `link_previews` | Address exposure, not contact (section 7) |

### 3.4 Block, report and moderation tools that exist

**[Today]**

- **Web block list** (`getBlockList`, `blockUser`, `isBlocked` in
  `web/chat/chat-profile.js`): display names in `localStorage`
  (`humanity_blocks`). Used by `addChatMessage` (app.js), the typing
  indicator (chat-messages.js), the user-list context menu (chat-ui.js) and
  the voice participant list (chat-voice-modal.js). Not used on the DM path
  (`dm_new` and `dm_batch` in app.js never call it). `/block`, `/unblock`
  and `/blocklist` exist. A name is the wrong key: names change, and the same
  name on another server is another person.
- **Native block:** none. The 2026-07-25 archived-tasks audit already noted
  it (`docs/history/2026-07-25-archived-tasks-audit.md`, item 6).
- **Report:** the slash command `/report <name> [reason]` (`relay.rs`),
  stored in `reports (id, reporter_key, reported_name, reason, created_at)`
  (`src/relay/storage/mod.rs`, `add_report` in `storage/misc.rs`). Three an
  hour, five for verified or donor members. Online admins and mods get a
  private notice. `/reports` lists the latest 20; `/reports-clear` empties it.
  Web's context menu prompts for a reason (`reportUser` in chat-ui.js);
  native's message menu sends `/report <sender_name>` and drops the message
  content it was pressed on (`pending_reports` in `src/gui/pages/chat.rs`).
  No evidence, no target key, no decision record.
- **Admin and moderator tools** (help text in `relay.rs`): `/kick`, `/mute`
  (also stops their DMs, through `is_muted` in `handle_dm_put`), `/ban`,
  `/unban`, `/lockdown` with `/invite` codes (invite-only registration),
  `/wipe`, `/wipe-all`, read-only channels, `/name-release`, deleting any
  message (v0.281.0), `game_ban`, and taking down builds. A ban is on a key;
  a new key is free, which is why `/lockdown` is the server-wide answer to
  throwaway identities.

### 3.5 Privacy settings that exist

**[Today]**

- **Privacy tiers** (`data/gui/privacy_tiers.json`; native
  `src/gui/pages/privacy.rs`; web `web/chat/chat-privacy.js`): Private
  (default), Balanced, Open, Spotlight, each a preset over two switches:
  `hide_presence` (relay `privacy_update`; never shown online, no last-seen
  stored) and the directory listing.
- **Directory opt-out is shipped, not deferred.** `members.rs`
  (`MEMBER_DIR_VISIBLE`) honours `directory:"unlisted"` in `get_members`,
  `get_member`, `get_member_count`, with tests; it shipped in v0.425.0
  (`git log -S MEMBER_DIR_VISIBLE`), and both clients have the toggle.
  **PRIORITIES TIER 1 item 2 is stale** and should be closed, with one
  residual: the in-chat member list ignores it (table above).
- **Per-field profile privacy** (`"location":"private"` and so on in the
  same JSON, `get_public_profile`). Native's Profile page, Privacy page and
  tier switch send a privacy JSON holding only the directory key, which
  wipes any per-field privacy set on web (the comment in
  `src/gui/pages/profile.rs` says so). Any new privacy key must be merged,
  not overwritten.
- **"Relay my calls"** (web only, `setRelayCallsOnly` in chat-privacy.js):
  forces calls through TURN and fails closed. The server has no TURN today
  (section 7), so on web this switch currently means "calls fail". Native
  has no such switch (PRIORITIES TIER 2 item 6).

### 3.6 Earlier designs and attempts

- `docs/design/report-system.md` (v0.188 era): a full three-context report
  design. Only its smallest part exists (the slash command). Its DM section
  asks the reporter to "copy-paste" messages and signs reports with Ed25519;
  both are superseded by sealed sender and Dilithium3 identity. Section 8
  here replaces its DM section; its anti-abuse list (rate limits, same-target
  cooldown, no self-reports, reviewer may not judge a report against
  themselves) still stands.
- `docs/user/skills/handling_conflict.md` (Library, "Handling Conflict")
  tells readers today that none of this exists, and points at
  `conformance_gaps.md`. Both change when this ships (section 11).
- `git log --oneline -i --grep=block` finds no block feature work (the hits
  are building blocks and blockers).
- PRIORITIES TIER 1 item 3 asked for "a recipient-side deny list the relay
  enforces at dm_put (kept as the recipient's own sealed data where
  possible)". Those two halves cannot both hold: whatever the relay enforces,
  it must be able to read at the moment of `dm_put`. Section 4.3 lays out the
  real choices.

### 3.7 Defects found while checking (fix regardless of the rest)

1. **Web data sync answers strangers.** `handleDCOffer` (chat-p2p.js)
   answers a direct-connection offer from any key; the relay forwards
   `dc_offer` from anyone online. Once the channel is open, a `sync_offer`
   frame makes `handleSyncFrame` reply with `buildSyncBundle()`: the
   `SYNC_STORES` keys `hos_calendar_v1`, `hos_homes_v2`, `hos_home_todos`,
   `hos_home_notes`, `hos_inventory_v1`, `hos_notes_v1`, `hos_skills_v1`,
   `hos_quests_v1`, `hos_equipment_v1`, `hos_logbook_v1`, `map_pins_v1`,
   `map_polygons_v1`, plus your display name. A `sync_data` frame is merged
   into your storage unasked (`applySyncBundle`). Map pins and home records
   can hold a real home. Verified by reading, not by running. Fix: sync only
   with your own devices (the peer key equals your key) and ask before
   merging; never answer a direct-connection offer from someone who is not a
   friend or a member of a group or voice room you are in.
2. **Native answers anyone's direct-connection offer** (`cmd_signal` to
   `on_offer` in src/net/webrtc.rs; `frame_ws_poll.rs` passes every `dc_*`
   signal through). It carries no data on its own, but it hands over the
   device's public address (section 7).
3. **Hidden presence leaks through three handlers.** `handle_voice_call`,
   `handle_webrtc_signal` and `handle_trade_request` answer "not online" when
   the target is offline and stay silent when they are online, for every
   target, including people who chose to hide their presence. Fix: answer a
   hidden person's callers the same way whether they are online or not (and
   the reach setting in section 4 refuses before anything is revealed).
4. **Group admit without consent.** The roster projection accepts a
   creator-signed `group_member_v1` admit for any subject
   (`groups_p2p.rs`, the authorization block above `is_self_leave`), and
   `/api/v2/groups?pubkey=` then lists the group for that person
   (`p2p_groups_for_member`). Shipped clients only ever join through a
   ticket, so this needs a modified client, but the protocol allows it. Fix:
   a creator's admit counts only alongside the subject's own signed join.
5. **Trade requests are an unlimited side channel**: free text to anyone
   online, outside the DM limits (`handle_trade_request`).
6. **Native loads any website's picture a message names**, in public posts
   and DMs, which tells that website your address (section 7).

---

## 4. Blocking a person

### 4.1 What a block must stop

**[Proposed]** When you block someone:

- Their DMs, knocks, follow notices and friend-code hellos never show and
  never notify.
- Their public posts, replies, reactions, mentions and typing never show
  (optionally a collapsed "hidden: someone you blocked" row; off by default).
- Their calls never ring; their direct-connection offers are never answered.
- Their trade requests are declined without showing.
- Their group messages are hidden in groups you share.
- In the game, their figure and name are not drawn for you.
- Their friendship certificate from you stops working, so they lose the
  friend lane to you and any friends-only profile fields.
- Your safety settings decide whether they can still deliver anything at all
  (section 4.4).

What a block cannot do, said plainly on screen (section 6.5): it cannot stop
them seeing your public posts or public profile, cannot stop someone making a
new key, cannot remove them from a public channel, voice room or group you do
not run, and cannot stop them standing near you in a shared world.

### 4.2 The hard part

The relay deliberately keeps no list of who knows whom (CLAUDE.md, the
"Removed (privacy hardening)" note: the `follows` table, the DM graph and
plaintext groups were all deleted). A block is a **negative** edge, "A does
not want to hear from B", and it is at least as sensitive as a friendship: a
copy of the database, a court order or a careless admin would reveal that a
person blocked their abuser, and an admin could tell the abuser.

### 4.3 The options

| Option | How it works | Protects against | Leaks |
|---|---|---|---|
| **A. Client only** | Your client drops anything whose verified sender is on your block list. The list lives on your device, synced to your other devices by a sealed note to yourself | Seeing or being notified of anything they send, on every path | Nothing new. But their knocks still land in your mailbox (up to 20 a day per key), an offline push "You have a new encrypted message" still fires, and new keys get 20 more each |
| **B. Deny list at the relay** | You tell the relay "refuse B"; `handle_dm_put` checks it | All of A, plus their mail is never stored and never pushed | The relay stores who blocked whom. Readable by the operator, in backups, under a court order |
| **C. Certificates plus a reach setting** | Withdraw their certificate (section 5), and choose "friends only" so the relay refuses every DM, ring and trade without a valid, unwithdrawn certificate from you | Everything A covers, plus delivery itself from anyone who is not a current friend, new keys included | The relay stores one setting about you ("accepts friends only") and a list of withdrawn certificates identified by random serials, which name nobody. It still sees, in memory, which socket presents which certificate at each `dm_put`, as it does today |
| **D. B with hashed entries** | Store each deny entry as BLAKE3 keyed with a secret file on the server (the `erased_accounts` pattern) | A database copy taken without the secret file reads as noise | The running relay can still test any pair, so the operator still learns it. It also reveals how many people each person blocked |

### 4.4 Recommendation

**A plus C, never B or D.** A covers every path the instant you press
Block. C gives the server-enforced closure the Accord asks for ("close
contact pathways without escalating to moderators") without a negative edge
on the server: if you want strangers stopped at the door rather than
filtered by your client, choose "friends only". Because new keys are free, a
per-key server deny list (B) would only stop the lazy anyway; "friends only"
stops everyone who is not a current friend.

So a Block button does three things: adds the key to your block list (A),
withdraws your certificate to them if you gave one (C), and shows one line
offering "Only friends can message me" if you are not already set that way.

### 4.5 Where each part is enforced

| Path | Client (A) | Relay (C) |
|---|---|---|
| DM, knock, follow, friend-cert | Drop after verifying `from` (`ingest_dm` before `store.insert`; web `dm_new`/`dm_batch` before `hosDmStore.insert`) | `handle_dm_put`: certificate v2 check plus withdrawn-serial check; "friends only" refuses certless mail |
| Offline push for a DM | n/a | Sent only for mail that passed "friends only" when that is set. A new optional switch "push only for friends' messages" uses what the relay already knows at put time (was a valid certificate presented), so it needs no edge |
| Mention push (offline) | n/a | Cannot be filtered per person without an edge. Offer "mention notifications: off / from anyone" (the existing `notification_prefs.mentions_enabled`) |
| Posts, replies, reactions, typing | Hide by key | None (public by nature) |
| Call ring | Ignore silently (no "reject" reply, which would reveal you are online) | `handle_voice_call`: same certificate rule under the call setting (section 6.2) |
| Direct-connection offer | Never answer a non-friend who is not in the same group or voice room | `handle_webrtc_signal`: refuse `dc_offer` to a "friends only" person without a certificate |
| Trade request | Decline silently | `handle_trade_request`: same rule |
| Group messages | Hide by author key | None (a creator can remove them) |
| Groups they made that added you | Hide; fix defect 3.7.4 so it cannot happen | `groups_p2p.rs` consent fix |
| Profile, friends-only fields | n/a | Withdrawn certificate fails `verify_friend_cert` v2 |
| Game figure and name | Skip in `nameplate_labels` and the figure pass; needs the player's key on `RemotePlayer` (`src/net/sync.rs` keeps only `player_id` and `name`; the relay snapshot has an `owner` field) | None |

The block list is a set of identity keys, never names. On native it lives
with the identity's settings; on web in the encrypted local store. Devices of
the same identity sync it with two new sealed control notes addressed to
yourself only, `[[hum:block:v1]]` and `[[hum:unblock:v1]]` carrying the key
(constants beside `CTL_FOLLOW` in `src/net/dm_pq.rs` and crypto.js, which
must match). The blocked person is never sent anything. Web's old name list
is replaced outright (no compatibility code before launch, CLAUDE.md).

### 4.6 The interface (native first, web mirrors)

- **Block** beside Report in the message context menu (`src/gui/pages/chat.rs`,
  where `Report` is today), in the DM conversation header, in the member
  list menu, and on a player's name in the game. Web: the same three places
  plus `/block`.
- **Settings > Safety > Blocked people**: the list, each with Unblock and
  the date blocked.
- A block asks no confirmation (it is undoable) and shows one line: "Blocked.
  You will not see anything from them. They are not told."

### 4.7 What the blocked person sees

**Recommended: nothing that singles out a block.** In "anyone" mode their
messages appear sent and are silently discarded on your side. In "friends
only" mode they get the same sentence every non-friend gets: "This person
only accepts messages from friends." Calls ring out with no answer. (Operator
question 3.)

---

## 5. Withdrawing friendship: certificate v2

### 5.1 Today

See 3.2: `hum/friend/v1\n{issuer}\n{grantee}`, no way back.

### 5.2 Proposed format

**[Proposed]**, modelled on the household permits that shipped on 2026-10-05
(`plot_permit_preimage`, `build_plot_permit`, `verify_plot_permit` in the same
file, which already carry a server, an end date and "mint refuses what the
check refuses"):

```text
preimage = "hum/friend/v2\n{server}\n{issuer}\n{grantee}\n{serial}\n{expiry}"
server   = the relay's own did:hum: (Storage::server_did, /api/server-info)
serial   = 16 random bytes, hex, chosen by the issuer's client
expiry   = Unix seconds, at most FRIEND_CERT_MAX_DAYS ahead
cert     = {"v":2,"serial":"...","exp":N,"sig":"<base64 Dilithium3>"}
```

- **Server-bound**, like permits. The follow state and certificates are
  already stored per server (`DmStore`), friendship is formed over one
  server's mailbox, and binding means a withdrawal on that server is exact
  and a certificate copied elsewhere is useless.
- **An end date** so nothing is endless (the operator's rule for permits,
  2026-10-05). The issuer's client renews each current friend's certificate
  automatically when it has less than a third of its life left, on any
  connection. If the issuer is away longer than the lifetime, friends fall
  back to knocks (or are refused in "friends only") until the issuer returns
  and the renewals go out. Operator question 4 sets the length.
- **A serial** so one certificate can be withdrawn without touching the
  others.

**What the relay checks at `dm_put`** (and the same for profile requests,
rings, direct-connection offers and trades): it rebuilds the preimage from
**its own facts** (its server DID, `to` as issuer, the socket's key as
grantee) plus the serial and expiry the certificate carries; checks the end
date (not passed, not more than the ceiling plus one day of clock slack, as
`PLOT_PERMIT_CLOCK_SLACK_SECS` does); checks the signature; then checks that
`(to, serial)` is not withdrawn.

**Withdrawing.** The issuer's client sends `cert_revoke {serial, exp}` on its
own signed-in socket; the relay stores `(issuer_key, serial, exp)` in a new
table `friend_cert_revocations` and the row is dropped once `exp` passes
(culled by `src/relay/storage/expiry.rs`, as `erased_accounts` is). The issuer
is the socket key, so no extra signature is needed. A new table is a plain
`CREATE TABLE IF NOT EXISTS`, which is safe on the live database (BUG-046
concerns ALTER-added columns only).

**When it is withdrawn.** On Block, on Unfollow (an unfollow means "I no
longer consent to the friend lane"), and from a "Remove friend" action. The
unfollow notice the other side receives today stays; a block sends nothing.

### 5.3 What the relay learns

**[Built 2026-10-09, step A.]** The revocation table,
`friend_cert_revocations (issuer_fingerprint, serial, revoked_day)`, holds
random serials under a keyed one-way fingerprint of the key that withdrew
them, never the key: BLAKE3 keyed with the relay's machine-local secret (the
erased-accounts one, `data/erased-accounts.key`, kept beside the live database
and never in the backups) over `hum/withdrawn-pass/v1\n` plus the lower-cased
key. Public keys are public, so a plain key or a plain hash would let whoever
holds a copy of the database (a backup, a breach, a court order) see who took
back friendships and when; with the secret outside the database, a copy names
nobody. The different first line means the same key gives unrelated values
here and in `erased_accounts`, so the two tables cannot be matched, and one
secret means one file to carry when a server moves.

The rows are **kept when an account is erased**: they name nobody, and
deleting them would let someone who erased and signed up again with the same
recovery phrase revive every pass they had withdrawn. The person's own export
lists them by computing their fingerprint.

If the secret file is damaged (the relay then runs on a secret of its own and
says so, `erase_memory: this_run_only`), the relay fails safe: it records and
confirms no withdrawal (the issuer's client keeps resending) and counts every
pass as withdrawn, so friends fall to the stranger's lane until it is fixed. A
lost file is replaced with a new secret, as for erased accounts, and then the
earlier withdrawals stop matching; that is why the file travels with the
database.

A relay modified to log every pass presented at `dm_put` could match a serial
to the person who presented it; that same modified relay could log sender and
recipient pairs of every DM today, so this adds no new class of exposure. The
Cryptography table in CLAUDE.md says so.

### 5.4 Files and tests that change in the same commit

- `src/relay/core/pq_crypto.rs`: `FRIEND_CERT_DOMAIN` becomes
  `hum/friend/v2`; `friend_cert_preimage`, `build_friend_cert` (now takes
  server, serial, expiry, now and returns a Result), `verify_friend_cert`
  (takes the relay's facts and `now`); the pinned test
  `friend_cert_roundtrip_and_pinned_preimage` re-pinned; new tests: expired,
  too long, wrong server, wrong grantee, mint refuses what verify refuses.
- `src/relay/handlers/msg_handlers.rs`: `handle_dm_put`,
  `handle_profile_request`; the DM tests that mint certificates
  (`knock_budget_and_forged_cert` and its neighbours) updated; a new test
  that a withdrawn serial falls back to the knock lane and is refused under
  "friends only".
- `src/relay/storage/`: the revocation table, its expiry cull, a test.
- `src/engine/dm.rs` (`send_friend_cert`, `ingest_control`, renewal),
  `src/net/dm_store.rs` (`certs_from` keeps the parsed certificate; a
  `certs_sent` entry records serial and expiry so renewal and withdrawal know
  what to act on).
- Web: `web/chat/crypto.js` (`pqBuildFriendCert`, `pqVerifyFriendCert`),
  `web/chat/chat-social.js` (`sendFriendCertTo`, `ingestDmControl`),
  `web/chat/chat-dm-store.js`. `app/web/` is generated by
  `scripts/bundle-web.js` and is never edited by hand.
- **Cross-client pinning, new:** a Node test that reads the Rust pin out of
  `pq_crypto.rs` and compares it with the web builder, the way
  `scripts/tests/second-player.test.js` already does for
  `plot_permit_preimage`; and a vector in `scripts/pq-kat.mjs`: a
  certificate minted by Rust from the KAT seed, verified with the vendored
  bundle, so the signature path is checked too, not only the string. To make
  the web builder reachable from Node, move it into a small module under
  `web/shared/` that crypto.js also uses.
- CLAUDE.md, the "Friendship certificates" row of the Cryptography table.
- Old v1 certificates simply stop working; each client re-mints for its
  current mutual follows on first run (clear `certs_sent` when the format
  changes). No compatibility branch.

### 5.5 What withdrawing does not do

It does not erase messages they already have, and it does not stop knocks in
"anyone" mode. The client-side block does the second.

### 5.6 The same gap in household permits

A household permit cannot be withdrawn before its end date either (the
pq_crypto.rs comment: "the relay keeps no list of permits, so none can be
withdrawn before it runs out"). The same serial-and-withdrawal list can cover
it: add a serial to `plot_permit_preimage` in the same increment or the next.
Someone you let build on your plot who then harasses you should lose that the
moment you say so, not up to 90 days later.

---

## 6. Safety settings for everyone, and a protected setup for children

### 6.1 What software can and cannot promise

There is no sign-up and no age check, and the project will not collect
identity documents to add one. **Nothing in HumanityOS can prove who is a
child, or who is an adult pretending to be one.** So the protection is
defaults and limits that hold whoever is on the other end, and a setup a
parent applies on a child's device and locks.

Messages are end-to-end encrypted. Nobody on the server, including admins,
can read them, so every warning in this section runs **on your own device**
and nothing it looks at leaves it. That is a strength (no one scans your
messages) and a limit (the server cannot spot a predator for you).

One rule for the protected setup: **no "this is a child" flag ever leaves the
device.** Every switch it sets is one any adult can set too, so the server
cannot single out children's accounts.

### 6.2 The switches

Settings > Safety (native first; web mirrors). Presets are data, like the
privacy tiers: `data/gui/safety_presets.json`, read by both clients.

| Switch | Everyone, by default | Protected setup | Enforced by |
|---|---|---|---|
| Block a person | Always available | Always available | Client plus relay (section 4) |
| Who can message me | Anyone, strangers go to **Requests** | Friends only | Relay (`dm_put`), new per-person setting |
| Message requests: no sound, links not clickable, pictures not loaded until you accept | On | n/a (no requests) | Client |
| Who can call me | Friends only | Friends only | Relay (`voice_call`) plus client |
| Hide my network address in calls | On (once section 7 ships) | On, cannot be turned off | Client (relay-only connections) plus the relay's forwarder |
| Who can send me trade requests | People in the shared world | Friends only | Relay (`trade_request`) |
| Warnings on messages (section 6.3) | On for non-friends | On for everyone, friends too | Client |
| Pictures and files from non-friends | Click to load | Never shown | Client |
| Listed in the member directory | Off (the Private tier, today's default) | Off | Relay (exists) |
| Shown as online | Off (Private tier) | Off | Relay (exists) |
| Making a friend (follow back, friend code, accepting a request) | Free | Needs the parent's PIN | Client |
| Joining a group by ticket | Free | Needs the PIN | Client |
| Joining a voice room | Free | Needs the PIN | Client |
| Never send my recovery phrase | Always on, no switch | Always on | Client |
| Changing any of the above | Free | Needs the PIN | Client |

The per-person reach settings are stored by the relay as columns on
`server_members` keyed by identity (`contact_policy`, `call_policy`), set
through the existing `privacy_update` message the way `hide_presence` is, and
re-sent on every connection the way the privacy tier is re-asserted
(`reassertPrivacyTier` in chat-privacy.js). They are ALTER-added columns, so
they follow the BUG-046 rule and get a pre-migration `Storage::open` test.

**Admins and mods under "friends only".** Today admins and mods skip both the
certificate check and the knock limit (`handle_dm_put`). Recommended: "friends
only" means friends only, admins included. A server admin is a volunteer, not
a vetted person, and admins already have system notices (`Private` messages)
for moderation. (Operator question 5.)

**"Hide from the in-chat member list"** is not offered yet: the list is also
how DM keys reach senders (`broadcast_full_user_list` carries
`kyber_public`), so hiding from it needs friends to receive your key another
way first. Listed as later work, and said on screen.

### 6.3 Warnings

**[Proposed]** A data file, `data/safety/warnings.json` (one entry per
pattern, per the infinite-of-x rule), read by both clients. Each entry: the
words or patterns it matches, who it applies to (non-friends, friends,
anything you are about to send), a title, a plain explanation and what to do.
First entries:

- **Asking for money**: gift cards, wire transfers, "send crypto", an
  investment that cannot lose.
- **Asking for your recovery phrase, wallet words or keys**, or claiming to
  be HumanityOS staff or a server admin. "Nobody from HumanityOS or any server
  will ever ask for your recovery phrase." DMs show the sender's real server
  role next to their name, so "I am an admin" can be checked at a glance.
- **Moving the conversation elsewhere**: another app, a phone number, a
  private video call.
- **Urgency and secrecy**: "right now", "your account will be closed", "do
  not tell anyone", "this is our secret". In the protected setup also:
  questions about age, school, home address or being alone, and requests for
  photos.
- **Links and files from non-friends**: a line under the message saying who
  sent it and that they are not a friend, before anything opens.

**Outbound recovery-phrase guard.** Your client can derive your own recovery
phrase. If anything you are about to send (DM, post, group message) contains
it, or a long run of its words, the send is stopped with: "This is your
recovery phrase. Anyone who has it owns your identity and everything in it.
Nobody legitimate will ever ask for it." There is no "send anyway"; edit the
text to send the rest.

Warnings never block a friend's message and never report anything anywhere.
A friend's account can be taken over, which is why the money and phrase
warnings apply to friends too.

### 6.4 The protected setup and its PIN

- A parent opens Settings > Safety > "Set up for a child" on the child's
  device, reads what it does and does not do (6.5), chooses a PIN, and
  applies the preset.
- The PIN is stored as a verifier with PBKDF2-SHA-256 at 600,000 iterations,
  the same as the vaults, never in plain text. Forgetting it: re-entering the
  identity's recovery phrase resets it, so the parent should keep the
  recovery phrase and the child should not.
- The strongest arrangement the software offers is a family server: the
  in-app Host page runs one, and `/lockdown` with `/invite` makes it
  invite-only. Say so in the setup screen.
- Friend requests in the protected setup appear as "Waiting for a parent":
  the child sees who asked; accepting needs the PIN.

### 6.5 What it says on screen

Per CLAUDE.md "Saying what we will not build, and why", these sentences
appear in the setup screen and in Settings > Safety, not only in docs. Each
says why: by design, or not possible in software.

- "HumanityOS cannot check anyone's age. There is no sign-up, and we do not
  collect ID. These settings work by limiting who can reach this device."
  (By design.)
- "Messages are end-to-end encrypted. Nobody, including server admins, can
  read them to look for danger. Warnings are checked on this device only."
  (By design.)
- "Anyone can make a new identity for free. Blocking stops that identity;
  'Only friends can message me' stops everyone who is not a friend." (Not
  possible to prevent in software without collecting identity.)
- "This lock protects the settings in this app on this device. It does not
  stop another app, another browser or another device. Your computer's or
  phone's own family controls can." (Not possible in software we control.)
- "Public rooms are public. Anyone on the server can post there. Choose a
  server you trust, or run a family server." (By design.)
- "Not yet: hiding from the in-chat member list." (Later work.)

### 6.6 Legal questions to research before shipping

Per the project rule (CLAUDE.md, "RESEARCH A LEGAL QUESTION, THEN WRITE IT
DOWN AND DATE IT"), none of these is answered here, and the feature must not
be called "kid safe", "child safe" or "compliant with" anything until a dated
findings document under `docs/reference/findings/` says what the primary
sources say:

- Whether children's-privacy law (in the US, COPPA; in the UK, the Age
  Appropriate Design Code; in the EU, the Digital Services Act's provisions on
  minors) applies to a no-sign-up app that collects no personal data on
  purpose, and what it would require if it does.
- What duties a server operator has on learning of child sexual abuse
  material (in the US, start from 18 U.S.C. 2258A), and what the report flow
  in section 8 must then do.

Operator question 6 covers the name.

---

## 7. Who can see your network address

### 7.1 Today (verified)

**[Today]**

1. **Google sees the address of every desktop app connected to a server,
   voice or not.** The native WebRTC manager starts on every chat connection
   with an identity (`WebrtcManager::start` in `src/lib.rs`, "Lazy start")
   and its loop sends STUN requests to `stun.l.google.com:19302` and
   `stun1.l.google.com:19302` until it learns its public address
   (`STUN_SERVERS`, `maybe_send_stun` in `src/net/webrtc.rs`). The web client
   asks the same servers whenever it opens a peer connection: the starting
   `rtcConfig` lists them (`web/chat/chat-voice-rooms.js`), and the list it
   then fetches from the relay's `/api/turn-credentials` puts Google first
   (`turn_credentials` in `src/relay/turn.rs`).
2. **That list also offers `stun:<host>:3478`, where nothing listens.**
   coturn was removed after the 2026-08-07 incident
   (`docs/INCIDENT-PLAYBOOK.md`: a leaked static credential let attackers use
   it as a reflector; "coturn is BANNED from the box"), and with no
   `TURN_STATIC_SECRET` set no TURN entries are issued.
3. **Everyone in a call or voice room sees everyone else's address.** Voice
   rooms are a full mesh of direct connections (`window._roomPeerConnections`
   in chat-voice-rooms.js); screen and camera streams ride the same
   connections (chat-voice-streaming.js). An address gives a rough location
   and the internet provider. Anyone can join a channel with voice on.
4. **Anyone online can learn your address without a call**, by sending a
   direct-connection offer: both clients answer anyone (3.7.1 and 3.7.2).
5. **Pictures from other websites.** Native fetches any `https` image a
   message names, in posts and DMs (`extract_image_urls`, `resolve_url` in
   `image_cache.rs`), which shows your address to that website; someone can
   post a link to a server they control to collect viewers' addresses. Web
   shows such images only when clicked, but loads link-preview pictures
   straight from the original site (`link_previews` in app.js).

### 7.2 What uses direct connections today

| Feature | Web | Native | Has a path through the server already? |
|---|---|---|---|
| 1:1 voice call | chat-voice-calls.js | webrtc.rs voice (`offer_to_voice`) | No |
| Voice rooms, plus screen and camera streams | chat-voice-rooms.js, chat-voice-streaming.js | webrtc.rs voice | No |
| P2P group mesh (pushes new group messages to online members) | `ensureGroupMesh` (chat-groups-p2p.js) | `ensure_group_mesh` (`src/gui/pages/chat/p2p_groups.rs`) | Yes: every group message is also posted to the relay and polled every 4 s |
| Contact-card DMs (`p2p_dm`) | chat-p2p.js | (none) | Yes: falls back to the mailbox |
| Data sync between devices (`sync_*`) | chat-p2p.js | (none) | No (and see defect 3.7.1) |
| Dev "P2P test" | (none) | right_panel.rs Dev tools | n/a |

### 7.3 The options

| Option | What it does | Address seen by | Cost and risk |
|---|---|---|---|
| **Keep Google** | Today | Google (every native app, every web call); every other participant | Nothing to build |
| **Another company's STUN** | Swap the host name | That company; every participant | Moves the third party, solves nothing |
| **Our own STUN responder in the relay** | The relay answers STUN requests on one UDP port. A STUN answer is about the size of the question and forwards nobody's traffic, unlike TURN | Our server (which already sees it through the WebSocket); every participant | One UDP port opened on the VPS, the operator's firewall call (asked 2026-10-05). Small reflection risk: rate-limit per source address |
| **Voice over the WebSocket the app already holds** | Each client encodes Opus itself and sends frames to the relay, which passes them to the room | Our server only | No new port. TCP can stutter on lossy Wi-Fi (one lost packet holds up the ones behind it). Web must encode and decode Opus itself: `AudioEncoder` is in Chrome 94+, Edge, Firefox 130+ on desktop (not Firefox for Android) and Safari 26+ (MDN browser-compat-data, read 2026-10-09), and whether it accepts Opus must be checked per browser with `AudioEncoder.isConfigSupported`; a WebAssembly build of the Opus reference library works everywhere. Echo cancellation, jitter buffering and loss concealment must be built and tested. The server could hear the audio unless frames are sealed end to end (a per-room key sent in sealed DMs) |
| **Room-scoped forwarder in the relay (our own TURN, closed)** | Clients use WebRTC exactly as now but connect only through the relay ("relay only"). Credentials are issued over the signed-in socket, bound to the identity and to one voice room or accepted call, and the forwarder only ever passes packets between its own allocations for that same room. It never sends to an outside address | Our server only | One UDP port (the same one the STUN responder would use). Packets stay encrypted end to end by WebRTC; the forwarder cannot hear them. Server bandwidth: a mesh room of N people forwards N times N minus 1 streams (Opus voice is tens of kilobits a second per stream, so a 6-person room is on the order of 1 Mbit/s each way; measure before trusting). A small added delay for people far from the server. Web needs no change beyond `iceTransportPolicy: 'relay'` (already behind "Relay my calls"); native already has a TURN client (inc-3b in webrtc.rs) that must become able to run relay only |
| **SFU in the relay** | The server receives each speaker once and sends to the others | Our server only | Best for big rooms (each client uploads one stream). The server ends WebRTC's encryption and **could hear the audio** unless an extra layer of frame encryption is added (the WebRTC Encoded Transform API). A larger build (str0m, already a dependency, is designed for SFUs) |

**Why the forwarder is not the 2026-08-07 incident again.** The harm then was
a public, static credential that let anyone relay traffic to any address on
the internet. The forwarder here has no public credential (each one is tied
to a signed-in identity and one room), and no outside destination at all: it
only connects two of its own allocations in the same room. There is nothing
to reflect towards a victim. It also must live in `scripts/provision-vps.sh`
with an assertion that the port is open and that only the relay listens on
it, per the incident's lesson that a fix is not shipped until something
checks it ran.

### 7.4 Recommendation

1. **Now, no port needed:** stop native starting STUN on connect (gather only
   when a call or room starts); never answer a direct-connection offer from a
   non-friend outside a shared group or voice room; native loads pictures
   only from its own server automatically and asks before loading any other
   site's (the web already does); web shows link-preview pictures only when
   the relay serves them itself. Drop the dead `stun:<host>:3478` entry until
   something listens there.
2. **Then the room-scoped forwarder plus our own STUN responder on one UDP
   port** in the relay binary, and remove Google from every list. Calls and
   rooms go through the forwarder by default for everyone ("Hide my address
   in calls": on). A direct connection is an opt-in that applies only when
   both people chose it, and it uses our own STUN, never Google.
3. **In hide-address mode, skip the group mesh and contact-card channels**:
   both already have a path through the server (7.2), so only a few seconds
   of delay is lost. Data sync becomes own-devices only (3.7.1).
4. **Later rungs:** voice over the WebSocket as the fallback for networks
   that block UDP (common at schools and workplaces); an SFU with frame
   encryption if rooms grow large; signing the call setup with the identity
   key, since today the encryption between two callers rests on the relay
   passing their call setup unchanged.

In the protected setup, "Hide my address" is on and cannot be turned off;
until the forwarder ships, the protected setup allows no calls or voice rooms
at all, and says so: "Calls are off in this setup until HumanityOS can keep
your address private in calls."

---

## 8. Reporting someone

### 8.1 Today

See 3.4: a name and a reason, no evidence.

### 8.2 Proposed: reports the admins can check

**[Proposed]**

- **By key, not name.** A report names the target's identity key.
- **Evidence you choose.** From a DM conversation you tick the messages to
  include. Each is the verified inner payload your client already holds:
  `{from, to, ts, text, sig}`. The admin's client (or the relay) checks the
  Dilithium3 signature over `hum/dm/v2\n{from}\n{to}\n{ts}\n{text}` against
  the target's key. That proves the target wrote exactly that text and sent
  it to the reporter. A reporter cannot forge it or pin it on someone else.
  The relay never decrypts anything; the reporter hands over the readable
  copy by choice.
- What it does not prove, said to the admin on screen: the time is the
  sender's own clock (`ts` is sender-claimed), and the reporter chose which
  messages to include, so context may be missing.
- The other side of this, said to everyone in the privacy explanation: any
  DM you send carries your signature, so its recipient can prove to others
  that you wrote it. Reporting relies on this.
- **Public posts:** the report references the message (author key and
  timestamp); the server already has it, signed.
- **Group messages:** the signature covers the encrypted group object
  (`buildGroupMsgV1` in `web/shared/pq-object.js`), so proof needs that
  epoch's group key, which would expose the whole epoch to the admin. Send
  group reports to the group's creator by default (who can already read the
  group); a server report from a group carries the text without proof, and
  says so.
- **Reasons** from a data file (`data/safety/report_reasons.json`), including
  "a child may be in danger" and "scam".
- **The report itself** is signed by the reporter's key over a fixed preimage
  (`hum/report/v1\n...` with a BLAKE3 hash of the evidence), sent over the
  signed-in socket, rate-limited as today, plus `report-system.md`'s
  same-target cooldown and no self-reports.
- **Stored** in a reworked `reports` table (target key, context, reason,
  note, evidence, state, reviewer, decision). New columns follow BUG-046.
  Kept for a set time after a decision (operator question 7), because
  evidence is readable DM text given by choice.
- **Reporting offers blocking** in the same dialog (on by default for DM
  reports), as `report-system.md` proposed.
- **Admins and mods review in-app**, native first: a Reports page with each
  report, a "signature checked: sent by X to the reporter" badge per piece of
  evidence, and Dismiss, Warn, Mute, Kick, Ban, Delete. Decisions are logged
  (`docs/design/signed_moderation_logs.md`). The target is never told who
  reported them. This replaces `/reports` as the main tool (GUI-first rule);
  the slash commands stay.

### 8.3 When it is more than a server matter

The report dialog's "a child may be in danger" and "someone is in danger"
reasons show, before sending, where to go outside the app: emergency
services, and the national reporting line for child exploitation for the
person's country. Server admins are volunteers, not police. The exact list is
data (`data/safety/outside_help.json`, the Tools page's external catalogue is
the model) and is fact-checked before it ships, as Library guides are.

---

## 9. Increment plan

Smallest useful first. Each is one release with its proof. Increments 0 to 2
need no protocol change and can ship in parallel on disjoint files.

| # | What changes | How it is proven |
|---|---|---|
| 0 | **Fix the defects in 3.7.** Web sync only with your own devices, and asks before merging; neither client answers a non-friend's direct-connection offer outside a shared group or room; hidden-presence people answer callers the same online or not; trade requests from non-friends lose their free text; group admit needs the subject's own join; native asks before loading another website's picture | Web: a two-browser rig on a loopback relay where a stranger's `sync_offer` gets nothing. Relay tests: a creator's admit without a join does not list the group; `voice_call` to a hidden, online person gets the offline answer. Native: unit test on `extract_image_urls` routing other hosts to click-to-load |
| 1 | **Block, client side, both clients**: block list by key, sealed self-sync notes, every path in 4.5's client column, Settings > Safety > Blocked people. Web's name list replaced | `src/engine/dm.rs` unit tests (a blocked `from` is not stored or notified); `just snapshot settings` and a headless click test that the Block button is reachable; a rig where a blocked key's DM, post, ring and trade all fail to show |
| 2 | **Message requests**: non-friends' DMs in a Requests list, quiet, links inert, pictures not loaded; Accept (follows back) or Block | Snapshot of the Requests list; unit test of the friend/non-friend split |
| 3 | **Certificate v2 and withdrawal** (section 5), plus withdrawal on Block, Unfollow and Remove friend, and automatic renewal | The test and KAT list in 5.4; `just pq-kat`; relay battery (`just verify-relay`) |
| 4 | **Who can reach me**: `contact_policy` and `call_policy` on `server_members`, enforced at `dm_put`, `voice_call`, `dc_offer` and `trade_request`; Settings > Safety switches; Requests notice "only friends can message this person" for senders | Relay tests for each handler under both settings; pre-migration `Storage::open` test (BUG-046); `cargo check --features relay --no-default-features` |
| 5 | **Reports with evidence**, submit side: dialog on both clients, signed report, evidence check, new table | Relay test: a forged or re-addressed evidence item is rejected; a genuine one is accepted and marked checked |
| 6 | **Reports page for admins and mods** (native, then web) | Headless snapshot; click test through Dismiss and Mute |
| 7 | **Warnings**: `data/safety/warnings.json`, both clients, plus the outbound recovery-phrase guard | `just validate-data`; unit tests per pattern; a test that the guard stops a message holding the phrase and lets one without it through |
| 8 | **Address, no port**: 7.4 step 1 (native STUN only when a call starts, dead 3478 entry dropped, link-preview pictures served by the relay) | Native log shows no STUN before a call; a packet capture on the dev machine during a chat-only session shows no traffic to Google |
| 9 | **Address, the forwarder**: 7.4 steps 2 and 3, Google removed, `provision-vps.sh` opens and asserts the port | Two clients on different networks in a call where each side's connection list shows only the server's address; a probe that the forwarder refuses an allocation without a room seat and never sends to an outside address |
| 10 | **Protected setup**: `data/gui/safety_presets.json`, the PIN lock, friend and group joins waiting for a parent, the on-screen sentences in 6.5 | Snapshot of the setup screen; tests that each locked switch refuses a change without the PIN; a rig where a non-friend's DM, ring and trade are all refused by the relay |
| 11 | **Household permits get serials and withdrawal** (5.6) | Relay test: a withdrawn permit stops working at once |

Increment 10 waits for the legal findings document in 6.6 before anything is
called "for children" in public copy; the code itself does not.

---

## 10. Questions only the operator can answer

1. **Default for "who can message me" for everyone.** Recommendation:
   anyone, with strangers in a quiet Requests list. Friends only as the
   default would undo the 2026-09-06 "no gatekeeper" decision that let new
   members write to anyone; Requests keep that while removing the noise.
2. **Default for "who can call me".** Recommendation: friends only for
   everyone, now. A call is the one contact that also reveals your address
   today.
3. **Should a blocked person be told?** Recommendation: no. Only the neutral
   "only accepts messages from friends" sentence, which every non-friend gets
   too.
4. **How long a friendship certificate lasts before automatic renewal.**
   Recommendation: 90 days, matching household permits, renewed by the
   issuer's client once a third of its life is left.
5. **Does "friends only" apply to server admins and mods?** Recommendation:
   yes. They keep system notices for moderation.
6. **The name.** Recommendation: "Safety settings" for everyone and
   "Protected setup" for children, never "kid safe" or "child safe", and no
   public claim about children until the dated legal findings exist.
7. **How long reports and their evidence are kept.** Recommendation: 90 days
   after a decision, readable by the server's admins and mods only, then
   deleted.
8. **Open one UDP port (3478) on the VPS for the relay's own forwarder and
   STUN responder, and route all calls through the server by default.**
   Recommendation: yes, after increment 9 is built and its refusal probes
   pass, and with the port in `provision-vps.sh` so it is checked. Until
   then, calls stay friends only and the protected setup has no calls.

---

## 10a. The operator's answers (2026-10-09)

These change sections 4 to 7 where they disagree; those sections are revised as each
increment is built.

**Safe by default** (replaces the recommendation in question 1). Verbatim: "I feel like we
should default users to safe mode. Then people can enable messages/calls from strangers. We
should also add a way to select individuals or groups of users or something of users that can
message/call. Like I don't want most people calling me by default. I prefer texts from
strangers or maybe voice messages. I like being public but, calls are very disruptive."

So the model is a **"Who can reach me" table**: one row per kind of contact, each with its
own audience, chosen from a short ladder.

- Kinds: text messages, voice messages, calls (voice and video), invitations to groups and
  rooms, trade requests.
- Audiences, narrowest first: nobody; people I choose (named lists the person makes, such as
  Family or Close friends, and single people); friends; people in groups I am in; anyone.
- Safe defaults for everyone: messages and voice messages from friends; calls from people I
  choose (an empty list until the person adds someone, with a one-tap "let my friends call
  me"); invitations from friends; trade requests from friends. A stranger can still send a
  **contact request** that carries only their name (no text, no links, no pictures), rate
  limited, which the person accepts or ignores; accepting makes them a friend. That keeps
  people findable without giving strangers a channel for content.
- Someone like the operator opens it up: messages and voice messages from anyone (strangers'
  ones in a quiet Requests list), calls from Family only.
- The server enforces it without storing who is whose friend: the pass you give a person
  (the friendship certificate) lists what they may do (message, voice message, call), so a
  named list is simply the people whose pass includes calls, and the relay checks the pass as
  it does today. Only the per-row audience setting is stored with the person, as a signed
  setting that federated servers copy like a profile, so every server enforces the same rules.

**Friendships do not expire** (answers question 4; replaces the end date in section 5.2).
Verbatim: "What happens when a friendship expires? What if we want to remain friends forever?
Like I never want to stop being friends with my parents and brothers." With an end date, a
person away longer than its lifetime would find their family's messages refused under safe
defaults until their app came back online and renewed. So a friendship pass has no end date;
it ends only when one of the two ends it (unfriend, block), and that withdrawal works at once
through the serial (section 5.2). The withdrawn-serial list then keeps its rows (random serials,
a few dozen bytes each) until the issuer erases their account (revised in review the same day:
kept for good, erase included, under a keyed fingerprint of the issuer's key; see 5.3). Household permits keep their
end dates: lending your plot is a temporary thing.

**Our own address lookup (STUN) in the relay, and the port: yes.** "Let's do this." Combined
with routing calls through the server (question 8): one UDP port carries both our STUN
responder and the room-scoped forwarder (section 7.3), Google is removed from every list,
calls and voice rooms go through the forwarder by default, a direct connection is an opt-in
that both people must choose (and uses our STUN, never Google), and voice over the existing
WebSocket is the fallback for networks that block UDP. Public voice channels in the MMO are
server-routed by nature: only the server knows who is near whom, so it can forward each
person only the voices within earshot, which keeps bandwidth proportional to neighbours rather
than to the crowd. The port is opened by the operator once the responder and forwarder are
built and their refusal checks pass, and `scripts/provision-vps.sh` asserts it.

**Federation.** Every server runs its own STUN responder and forwarder (they are in the same
program), so a person's address is seen only by the server they chose to use. A call between
people on two federated servers goes from each person to their own server, and server to
server between them, rather than either person learning the other's address.

**Questions 2, 3, 5, 6, 7:** the recommendations stand ("I'm liking where you're going"):
calls are narrower than friends by default (above), a blocked person is not told, friends-only
binds admins and mods too, the children's setup is called the "Protected setup" until the dated
legal findings exist, and reports are kept 90 days after a decision.

**Raised for later designs (not in this document):** hardware security keys, signing in on a
new device by scanning a code from one already signed in, NFC and RFID. See the journal entry of
2026-10-09 for the first answer; they belong in an identity-on-every-device design.

## 10b. Build order after the operator's answers (2026-10-09)

Increment 0 shipped in v0.1465.0 (BUG-170 to BUG-173). The rest, in order, each its own
release with its proof:

**A. Friendship passes v2: no end date, a serial, and what the friend may do.** Replaces
section 5.2's format:

```text
preimage = "hum/friend/v2\n{server}\n{issuer}\n{grantee}\n{serial}\n{may}"
server   = the relay's own did:hum: (Storage::server_did, /api/server-info)
serial   = 16 random bytes, lowercase hex, chosen by the issuer's client
may      = the sorted, comma-joined subset of: call, invite, message, trade, voice_message
cert     = {"v":2,"serial":"...","may":"...","sig":"<base64 Dilithium3 over the preimage>"}
```

- **No end date** (10a). Withdrawal is by serial: the issuer sends `cert_revoke {serial}` on its
  own signed-in socket; the relay keeps `friend_cert_revocations (issuer_fingerprint, serial,
  revoked_day)`, under a keyed fingerprint of the issuer's key, for good (revised in review
  the same day: not the key, and kept past an account erase; see 5.3). A new table, so a
  plain `CREATE TABLE IF NOT EXISTS` (BUG-046 concerns ALTER-added columns only).
- **What the friend may do lives in the pass.** The relay rebuilds the preimage from its own
  facts (its server DID, the recipient as issuer, the socket's key as grantee) plus the serial
  and `may` the pass carries, checks the signature and that the serial is not withdrawn, and
  learns which kinds of contact this sender may use. Changing what a friend may do is a new
  pass (new serial) and a withdrawal of the old one.
- **Defaults when two people become friends** (mutual follow): message, voice_message, invite,
  trade. Not call: calls come only from people the person chooses (10a), so `call` is added
  only for people they put on a "may call me" list.
- **Every contact path carries the pass**: `dm_put` (as today), `trade_request` (the optional
  `friend_cert` added in v0.1465.0), `voice_call`, and `dc_offer`. Until B ships the relay only
  reads it; nothing is refused for lacking a capability except as today's knock budget does.
- **Withdrawn on**: Unfollow, Block (increment C), Remove friend.
- **v1 passes stop working outright**; each client re-mints for its current mutual follows on
  first run (no compatibility branch, CLAUDE.md).
- **Proof**: `pq_crypto` tests (pinned preimage, wrong server, wrong grantee, withdrawn serial,
  a `may` that was not signed), the cross-client pin (a Node test reading the Rust pin and
  comparing the web builder, as `scripts/tests/second-player.test.js` does for permits),
  `scripts/pq-kat.mjs` (a pass minted by Rust from the KAT seed verified by the vendored web
  bundle), relay tests at each handler, and the relay battery. CLAUDE.md's Cryptography row in
  the same commit.

**B. "Who can reach me".** The per-kind audience table (10a), stored as one signed setting per
person and enforced by the relay at `dm_put`, `voice_call`, `dc_offer`, `trade_request` and
group invitations, using the pass's `may` for the friend and chosen-list cases. A stranger who is
refused gets the same sentence every non-friend gets, and may send a **contact request**
(name only, rate limited). Settings > Safety on both clients (native first). Safe defaults for
new and existing people alike (nobody uses the platform yet, CLAUDE.md). Proof: relay tests per
handler under each audience; a pre-migration `Storage::open` test if a column is added; a
headless snapshot of the Safety page.

**C. Block, on both clients**, and the **Requests** list for strangers' messages where a person
allows them (section 4, increment 1 and 2 of section 9).

After C: reports (section 8), warnings, our own STUN and the forwarder (7.4), the protected
setup.

## 10c. Step B specification: "Who can reach me" (2026-10-09)

Step A (passes v2) shipped in v0.1466.0. This is step B, written so the relay and both clients
can be built in parallel against one protocol.

**Kinds the relay enforces now:** `message` (`dm_put`), `call` (`voice_call` rings and the call's
`webrtc_signal`s), `trade` (`trade_request`). Not yet: `invite` (group invitations travel as
tickets people paste into messages, so the message rule covers them) and `voice_message` (no
voice messages exist yet); both stay valid words in a pass for later.

**Audiences**, narrowest first, one per kind:

| Audience | Who gets through |
|---|---|
| `nobody` | no one |
| `chosen` | holders of a valid pass from you whose `may` includes this kind |
| `friends` | holders of any valid pass from you |
| `groups` | friends, plus people who share a P2P group with you (`p2p_groups_for_member` on both keys) |
| `anyone` | everyone (strangers still spend the daily knock budget) |

**Safe defaults** (a person with no saved settings): message `friends`, call `chosen`, trade
`friends`. New friends' passes carry `invite,message,trade,voice_message` (step A), so by default
no one can call until the person adds `call` to someone's pass. Admins and mods are bound too
(10a question 5): no exemption at these gates.

**Storage:** a new table `reach_settings (public_key TEXT, kind TEXT, audience TEXT,
PRIMARY KEY(public_key, kind)) WITHOUT ROWID`, created with plain `CREATE TABLE IF NOT EXISTS`
(no ALTER, so BUG-046 does not apply); a missing row means the default. Included in the account
export, deleted by the account erase. Per server for now; sharing it across federated servers
as a signed setting (10a) comes with federation work.

**Protocol (exact names; all three parts build against these):**
- client to relay: `{"type":"reach_set","settings":{"message":"friends","call":"chosen","trade":"friends"}}`
  from the signed-in socket; any subset of kinds; unknown kinds or audiences refuse the whole
  set with a Private notice and change nothing.
- relay to client: `{"type":"reach_settings","settings":{"message":"...","call":"...","trade":"..."}}`
  with all three kinds filled in (defaults included), sent after a successful identify and after
  every `reach_set`.
- a sender refused for `message` or `trade` gets `{"type":"reach_refused","kind":"message","to":"<target key>"}`
  (the same for everyone refused, so it does not single anyone out, 4.7); nothing is stored or
  delivered. A refused `call` gets nothing back at all (it rings out, and stays consistent with
  hidden presence, BUG-172).
- `dc_offer`: the relay also refuses (silently) an offer unless the sender holds a valid pass
  from the target, shares a P2P group with them, or both are in the same voice room; this backs
  up the clients' own gate (BUG-171, BUG-173).
- **Contact requests (AMENDED in review, 2026-10-09).** The first version (a name-only sealed
  payload of at most 256 bytes) had two holes the web build found: under the safe defaults the
  accepter's reply was refused at the requester's end (the accepter held no pass from the
  requester), so no friendship could ever form; and 256 bytes cannot hold a signature, so anyone
  could send a request under someone else's name. So a contact request is an ordinary sealed v2
  DM (its inner payload is signed by the sender, as every DM's is), sent with
  `"contact_request": true` on the `dm_put`, whose inner text is the control marker
  `[[hum:contact-request:v1]]` followed by the JSON `{"name":"<sender's registered name>",
  "pass":"<the sender's v2 pass for the recipient, as its JSON string>"}`. The pass is minted by
  the requester for the recipient with the default `may` (`invite,message,trade,voice_message`):
  asking someone to connect is consenting to hear back from them.
  - **Relay:** lets a `contact_request` `dm_put` through whatever the recipient's `message`
    audience, except `nobody`, when the sender has a contact request left today (5 a day per
    sender, separate from the 20 knocks), with the ordinary DM size limits. It stores nothing
    about who asked whom: the reply gets through because it carries the requester's pass.
  - **Recipient's client:** verifies the inner signature as for any DM, parses the marker,
    verifies the pass (issuer = the signed sender, grantee = itself, server = its server's
    did:hum), and shows only the sender's registered name as the member list knows it for that
    key (never the claimed `name` alone), with Accept and Ignore; any other text is never shown.
    Accept follows back and sends its own pass, attaching the requester's pass as `friend_cert`
    on that `dm_put` so the requester's relay gate admits it. Ignore does nothing and tells no
    one. A request whose pass does not verify is dropped.
  - Clients still treat any DM from someone their own settings would refuse as a request
    (name only, text dropped), so a modified client gains nothing by skipping the flag.

**Clients (native first, web mirrors):**
- **Settings > Safety**: "Who can reach me", one row per kind (Messages, Calls, Trades) with the
  five audiences in plain words ("Nobody", "People I choose", "Friends", "Friends and people in
  my groups", "Anyone"), and a "People who may call me" list: choosing a friend re-issues their
  pass with `call` added (withdraw the old serial, mint a new one), removing them re-issues
  without it. Show a short line under each row saying what it means.
- On `reach_refused` for a message: "This person only accepts messages from people they know. You
  can send a contact request: they will see only your name." with a Send request button.
- Contact requests appear in a Requests list with Accept and Ignore.
- `reach_settings` from the relay is the source of truth for what the Safety page shows.

**Proof:** relay tests for each kind under each audience (including the defaults with no row,
`groups` with a shared P2P group, admins bound, a contact request let through under `friends` and
refused under `nobody`, the 5-a-day budget, the accepter's reply admitted because it carries the
requester's pass, a refused call getting no reply, a refused `dc_offer`); a
storage test for the table, export and erase; client unit tests for the settings model and the
"show as request" rule; a headless snapshot of Settings > Safety (`just snapshot`), only when no
other HumanityOS instance runs; `just verify`, `just verify-relay`.

### 10c-ii. Step B follow-up: a tick per friend for Messages, Calls and Trades (2026-10-10)

Step B shipped "People I choose" for all three kinds, but only Calls had ticks ("People who may
call me"), so "People I choose" for Messages or Trades let every friend through: every pass
carries `message` and `trade` by default. The operator asked to choose individuals who may
message or call. The relay needs no change: under `chosen` it already admits only a pass whose
`may` holds the kind (`allowed` in `src/relay/handlers/reach.rs`, tested in `reach_tests.rs`).
This is client work on both apps, native first, web mirrors.

**The model.** For each friend (someone I have given a pass), my choice of three ticks:
Message, Call, Trade. A friend with no saved choice has the defaults: Message and Trade ticked,
Call not (the same as a new pass, step A). The pass I give them carries exactly:

- `message`, `invite` and `voice_message` when Message is ticked (an invitation and a voice
  message are forms of messaging, and travel together; the relay enforces only `message` today),
- `trade` when Trade is ticked,
- `call` when Call is ticked.

A friend with all three unticked keeps a pass whose `may` is `invite` alone: the format refuses
an empty `may` (`FriendMay::from_words` in `src/relay/core/pq_crypto.rs`), and `invite` gives
nothing the relay enforces today. Say so in a comment where it is built. Unticking does not end the friendship: it is still a mutual follow, and
under "Friends" every friend still gets through.

**Changing a tick** re-issues that friend's pass the way the call tick does today (mint the new
pass with the new `may` first, then withdraw the old serial), so the relay honours it at once.
Unfollow and Block clear the friend's choices (Block already unticks the call tick, 10d).

**Settings > Safety.** "People who may call me" becomes **"People I choose"**: each friend once,
with three ticks labelled Message, Call and Trade. Above the list, one line: "These ticks count
for a row set to People I choose." Under it, which rows use them now, in plain words built from
the person's settings, for example "In use now: Calls. Messages and Trades are set to Friends,
so every friend gets through for those." or "Not in use now: no row is set to People I choose."
The per-row help sentences that name the old list (`web/shared/reach.js` says 'Only the friends
on your "People who may call me" list can call you.') name the new one, and gain matching
sentences for Messages and Trades under "People I choose".

**Storage.** Native: replace `may_call: HashSet<String>` in `src/net/dm_store.rs` with one map
from friend to their ticks (absent means the defaults); `intended_may` in `src/net/reach.rs`
takes the ticks instead of a bool. No migration of the old field (no installed base, CLAUDE.md):
a stored `may_call` is simply not read. Web: today `friendMayCall` reads the kinds from the pass
already given (`store.passMayTo`); keep that approach, so the pass itself is the record, and
generalise `setFriendMayCall` to a per-kind setter.

**Proof:** unit tests on both clients: a friend with no choice gets the default `may`; unticking
Message drops `message`, `invite` and `voice_message` and nothing else; ticking Call adds only
`call`; all three unticked still gives a valid pass; Block and Unfollow clear the choice; the
"In use now" sentence for each mix of row settings. Each test seen failing once. Native: the
snapshot fixture in `src/gui/ui_snapshots.rs` shows the new list (render only when no other
HumanityOS instance runs). `just verify`.

## 10d. Step C specification: Block, on both clients (2026-10-09)

Steps A and B shipped (v0.1466.0, v0.1467.0). Block is client side (section 4.4's option A),
with the relay already doing its part: blocking someone withdraws the pass you gave them, so
under the safe defaults (10c) the relay refuses their messages, calls and trades from then on.

**What Block does, at once and without a confirmation dialog** (it is undoable):
1. Adds their identity key (never a name) to your block list.
2. Withdraws every pass you gave them (`cert_revoke` for each serial, step A) and unfollows.
3. Hides everything from them on every path in section 4.5's client column: DMs and knocks
   (dropped before they are stored, and no notification), contact requests (dropped, never
   listed), posts, replies, reactions and typing in channels (hidden by key), call rings
   (ignored silently, no reject sent), trade requests (declined silently), direct-connection
   offers (not answered), group messages (hidden by author key), and their figure and name in
   the game where the client knows the key.
4. Shows one line: "Blocked. You will not see anything from them. They are not told." Nothing is
   ever sent to the blocked person (4.7).

**Unblock** removes them from the list. It does not re-follow or re-issue a pass: becoming
friends again is a fresh follow or contact request.

**Your other devices learn it** through two sealed control notes addressed to yourself only (the
same self-copy path follows use), so the list is the same everywhere without the server knowing
it: `[[hum:block:v1]]<key>` and `[[hum:unblock:v1]]<key>`, constants beside `CTL_FOLLOW` in
`src/net/dm_pq.rs` and the web equivalent in `web/chat/crypto.js` or `web/shared/`, which must
match exactly. A note addressed to anyone but yourself is ignored. Web's old name-based block
list is replaced outright (no compatibility code before launch).

**Where Block appears** (native first, web mirrors): beside Report in a message's menu, in the
DM conversation header, in the member list's menu for a person, on a contact request (Block
instead of Ignore), and on a player's name in the game where that menu exists; web also takes
`/block <name>` and `/unblock <name>`. **Settings > Safety > Blocked people**: the list by
member-list name (or short key), each with Unblock and the date blocked.

**Proof:** unit tests on both clients that a blocked key's DM is not stored or notified, a
blocked key's contact request is dropped, a blocked key's channel post is hidden, a ring from a
blocked key sends nothing back, Block withdraws the passes and unfollows, the self-sync notes
round-trip between the two clients' builders (a Node test reading the Rust constants, as the pass
test does), and a note addressed to someone else is ignored. A headless snapshot of Blocked
people when no other HumanityOS instance runs. `just verify` and `just rig-tests`.

## 10e. Step D specification: reports the admins can check (2026-10-09)

Section 8 is the design; this fixes the protocol so the relay and both clients can be built in
parallel. Report retention is 90 days after a decision (10a).

**Reasons** live in `data/safety/report_reasons.json`: an ordered list of `{ "id", "label",
"help" }` with ids `spam`, `scam`, `harassment`, `threats`, `hate`, `unwanted_sexual`,
`impersonation`, `child_danger`, `someone_in_danger`, `other`. For `child_danger` and
`someone_in_danger` the `help` text says, before sending: "If anyone is in danger right now,
contact your local emergency number. This server's admins are volunteers, not police." The
per-country list of outside reporting lines (section 8.3) is NOT in this step: it needs a dated
findings document from primary sources first (CLAUDE.md, "Research a legal question, then write
it down and date it").

**Evidence items**, chosen by the reporter:
- `{"kind":"dm","from","to","ts","text","sig"}`: a DM's verified inner payload, as the
  reporter's client already holds it after opening the seal. The relay checks the Dilithium3
  signature over the DM v2 inner preimage (find the exact words in `src/net/dm_pq.rs`
  `build_signed_inner` and web `crypto.js`; the relay must rebuild them byte for byte) against
  the TARGET's key, with `from` = target and `to` = reporter. Proven items are marked
  `checked: true`; anything else `checked: false` and kept.
- `{"kind":"post","from","timestamp"}`: a public post; the relay looks it up in `messages` by
  author key and timestamp and stores the text it has.
- `{"kind":"group_text","from","ts","text"}`: from a P2P group; stored unproven and labelled so.
- At most 20 items, and at most 64 KB of evidence in all.

**Protocol:**
- client to relay: `{"type":"report_v2","target","context","reason","note","evidence":[...],
  "ts","sig"}`, `context` one of `dm`, `post`, `group`, `profile`; `note` at most 500
  characters; `sig` the reporter's Dilithium3 signature over
  `"hum/report/v1\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}"`, where
  `evidence_hash` is the lowercase hex BLAKE3 of the `evidence` array serialised exactly as sent
  (the relay hashes the JSON text of the `evidence` value as it received it; clients send compact
  JSON) and `reporter` is the signed-in socket's key. Refused (with a Private notice): a bad
  signature, an unknown reason, a self-report, more than 3 reports an hour from one reporter, or
  a second report of the same target by the same reporter within 24 hours.
- relay to reporter on success: `{"type":"report_received","id"}`.
- admins and mods: `{"type":"reports_list","state":"open"|"decided"}` returns
  `{"type":"reports","items":[...]}` with each report's id, target key and name, context,
  reason, note, evidence (with `checked`), created time, state, and decision; the reporter's
  identity is included only for admins (mods see "a member"), and the target is never told who
  reported them.
- `{"type":"report_decide","id","decision","note"}`, `decision` one of `dismiss`, `warn`, `mute`,
  `kick`, `ban`, `delete_post` (the reported post), carried out through the existing moderation
  path (`handle_mod_action` and its storage) so its rules (a mod cannot act on an admin, and so
  on) still apply; the report records reviewer, decision and time. Admins and mods only.
- Storage: a new table `reports_v2` (plain `CREATE TABLE IF NOT EXISTS`; any index over its own
  columns may sit in the same new-table batch), culled 90 days after `decided_at` by
  `src/relay/storage/expiry.rs`. Open reports are not culled. In the reporter's account export;
  on the reporter's account erase the reporter key is blanked, the report stays for the admins.
  The old name-and-reason report path is replaced (the `/reports` command reads the new table).

**Clients (native first, web mirrors):**
- **Report dialog** from a message's menu, a DM conversation's header, and a person's entry in
  the member list: the reasons from the data file (with their help text shown when chosen), an
  optional note, and for a DM report a list of that person's messages in the conversation to
  tick as evidence (the most recent one ticked by default); for a post, the post itself.
  "Also block them" is ticked by default for DM reports (it runs step C's Block).
- **Reports page for admins and mods**, native in the admin or moderation area that exists
  (find it), web in the admin page's equivalent: open and decided lists, each report's evidence
  with a "Signature checked: sent by <name> to the reporter" badge on proven items and
  "Not proven" on the rest, and the decision buttons. The page says what a checked signature
  does not prove: the time is the sender's own clock, and the reporter chose which messages to
  include.
- The privacy explanation gains one sentence: any direct message you send carries your
  signature, so the person you sent it to can prove to others that you wrote it.

**Proof:** relay tests (a genuine DM item checked, a forged one and one pinned on someone else
not checked, a post looked up, the evidence hash and the reporter signature, each refusal case,
the cooldowns, admin and mod listing differences, a decision carried out through the moderation
path with its rules, the 90-day cull, export and erase); a Node test that the web report builder
produces the relay's preimage (reading a pinned string out of the Rust tests); client unit tests;
headless snapshots of the dialog and the Reports page; `just verify`, `just verify-relay`.

### 10e-ii. Step D follow-up: help outside this server, in the report dialog (2026-10-10)

The dated finding `docs/reference/findings/2026-10-09-outside-help-lines.md` now exists, so the
per-country lines (section 8.3) can be shown. The data is `data/safety/outside_help.json`:
`researched` (the date), `default` (for any country not listed: `emergency_text` instead of a
number, and the INHOPE list for child reports), and `countries`, each `{code, name, emergency,
also?, child_report (or null), note?, source}`. Both clients read it (native from
`src/embedded_data.rs`, beside the other safety files; web from `/data/safety/outside_help.json`).

**When it shows.** In the report dialog, when the chosen reason is `child_danger` or
`someone_in_danger`, under that reason's help text: a block titled "Help outside this server".
Never for other reasons.

**What it shows.** A country picker ("Country: <name>", the listed countries by name, then
"Another country"), and for the chosen entry:

- the emergency number, large, with "Emergency:" before it (for "Another country", the
  `emergency_text` sentence instead),
- each `also` entry (its number and what it is for),
- for `child_danger`, the `child_report` body's name as a link that opens in the browser
  (for a country whose `child_report` is null, the default's INHOPE link with "Find the hotline
  for your country"); for `someone_in_danger`, the child line is shown too, smaller, after the
  emergency number,
- the entry's `note` when it has one,
- one small line: "Numbers checked on <researched>. If one is wrong, tell us." (the date from the
  file, never typed into the code).

**Which country first.** The last one this person picked on this device, kept in local settings
(native config; web localStorage, wrapped in try/catch). With none saved: the region of the
device's language setting when it names a listed country (web `navigator.language`, for example
`en-GB` gives GB; native the same from the OS locale if the app already reads it, otherwise
skip this step), else "Another country". **Never look up the person's location** (no IP lookup,
no location service): the choice stays on the device and is never sent with the report or
anywhere else.

**Proof:** unit tests on both clients: the block shows for exactly the two reasons; a listed
country shows its number, `also` lines and child body; a country with a null `child_report`
falls back to the INHOPE link; "Another country" shows the sentence; the first-country rule
(saved choice, then language region, then "Another country", and an unlisted region falls to
"Another country"); the date line reads the file's date. Each seen failing once. Native: a
snapshot fixture of the dialog with the block open (render only when no other HumanityOS
instance runs). `just verify`.

## 10f. Step E specification: our own STUN and the room-scoped call forwarder (2026-10-09)

The operator approved this (10a): our own STUN responder and a call forwarder on one UDP port,
Google removed from every list, calls and voice rooms through the server by default. This fixes
the protocol so the relay and both clients can be built in parallel. The port is opened on the
VPS firewall by the operator once this is built and its refusal checks pass; until then nothing
listens publicly and calls fall back as today.

**One UDP port**, `TURN_PORT` (default 3478), bound to `TURN_BIND` (default `0.0.0.0` on a
server; every dev rig and test binds `127.0.0.1`, CLAUDE.md "No Windows Firewall prompts"). The
address the relay tells clients is `TURN_PUBLIC_HOST` (default the host name it already uses for
`/api/turn-credentials`). It serves two things:
- **STUN Binding** (RFC 5389): a Binding request gets a Binding success with
  XOR-MAPPED-ADDRESS, nothing else. Rate limited per source address (say 20 a second with a
  small burst, refusing silently past it) so it cannot be used to flood anyone; a reply is never
  larger than the request plus a few dozen bytes.
- **A closed TURN forwarder** (the RFC 5766 subset WebRTC uses: Allocate over UDP, Refresh,
  CreatePermission, ChannelBind, Send and Data indications, ChannelData), long-term credentials
  with realm `humanityos`. **Relayed addresses are virtual**: each allocation gets the public
  host's address with a unique virtual port, never bound to a socket. A permission, a channel or
  a Send is accepted ONLY for a peer address that is another live allocation of the SAME room;
  anything else is refused (403) and nothing is ever sent to an address outside the forwarder.
  Data from allocation A to allocation B is delivered inside the process to B's client as a Data
  indication or ChannelData from A's relayed address. So the forwarder cannot reach the outside
  internet at all, which is what made the 2026-08-07 reflector possible.

**Credentials, over the signed-in socket only:**
- client to relay: `{"type":"call_credentials","room":"<voice room id>"}` or
  `{"type":"call_credentials","call":"<the other person's key>"}`.
- The relay answers only if the asker is in that voice room (its live roster) or in an open call
  with that person (`reach.rs`'s open calls): `{"type":"call_credentials","room"|"call",
  "urls":["turn:<host>:<port>?transport=udp","stun:<host>:<port>"],"username","credential",
  "ttl":3600}`. `username` is `"{expiry}:{room_tag}"` where `room_tag` is a hash of the room id
  (or of the two call keys, sorted) plus a per-request nonce; `credential` is the base64 HMAC-SHA1
  of `username` under a secret the relay makes at start and never stores (so credentials die with
  the process). An allocation made with them belongs to that room.
- Anything else gets no credentials (and no reply that says why beyond a Private notice).
- `/api/turn-credentials` (HTTP, unauthenticated) now returns only our own STUN entry; Google is
  gone from it, and it never returns TURN.

**Clients (native first, web mirrors):**
- Every Google STUN entry is removed (`STUN_SERVERS` in `src/net/webrtc.rs`, `rtcConfig` in
  `web/chat/chat-voice-rooms.js`, anywhere else).
- Joining a voice room or an accepted call asks `call_credentials` first and connects
  **relay only** (web `iceTransportPolicy: 'relay'`; native: str0m with only relay candidates
  through our TURN, building on the TURN client work already in `webrtc.rs`, inc-3b). So the
  other people in the call see only the server's address.
- Until the operator opens the port, a relay-only connection cannot form; the clients say so
  plainly ("Calls go through the server to keep your address private; this server is not set up
  for that yet.") rather than falling back to a direct connection.
- The P2P group mesh and contact-card channels are not opened in this mode (they already fall
  back to the server: group messages through the relay and its 4-second poll; design 7.2 and 7.4
  step 3). Own-device sync is unchanged.
- A direct, lower-delay connection both people opt into is a later step; not in this one.

**Provisioning:** `scripts/provision-vps.sh` sets `TURN_PORT` and asserts after start that only
the relay listens on it; the firewall rule itself is the operator's (the exact command goes in
`docs/admin/` with this step).

**Proof:** relay tests on loopback: a STUN Binding answered with the right XOR-MAPPED-ADDRESS;
the rate limit; Allocate refused without valid credentials and with credentials for another room;
two allocations of one room exchange data both ways through Send, ChannelBind and ChannelData; a
permission or Send toward any address that is not an allocation of the same room is refused and
NO packet leaves for it (assert on a listening socket at that address); credentials refused to
someone not in the room or call; credentials stop working after the process restarts. Client
tests: no Google entry anywhere (a lint-style test that greps both clients), the credential
request and relay-only configuration. A loopback rig with two clients in one room, if it can be
built without booting the game; otherwise say so.

## 10g. Step F specification: warnings, and the recovery-phrase guard (2026-10-10)

Section 6.3 is the design; this fixes what both clients must do identically. The patterns live
in `data/safety/warnings.json` (already written; reviewed by the coordinator; read by both
clients, built into the desktop app like the report reasons).

**Matching, identical on both clients.** Normalise a text by lower-casing it and turning every
run of characters that are not letters or digits into one space, then trimming; normalise each
phrase the same way. A message matches an entry when `" " + text + " "` contains
`" " + phrase + " "` for any phrase in `any`. So "I'm an admin!!" matches the phrase
"i'm an admin" (both become "i m an admin"), and "admin" never matches inside "administer".
Letters and digits are Unicode-aware and identical on both clients: Rust's
`char::is_alphanumeric` (the Alphabetic property or a numeric category) and the JavaScript class
`/[^\p{Alphabetic}\p{N}]+/gu` (CORRECTED in review, 2026-10-10: the first version said `\p{L}`,
which treats combining marks such as Devanagari vowel signs as separators where Rust keeps them as
part of the word; the shared cases include one that tells the two apart). Both clients lower-case
with their standard Unicode lower-casing.

**Where warnings show:** under a received direct message, and under a message in a P2P group,
when its sender is someone the entry `applies_to`. `friends` means a mutual follow (you follow
them and they follow you), the same test both clients already use when they give passes;
everyone else is `strangers`. Not on public channel posts in this step (too noisy for words
like "urgent"), and never on your own messages. Each matching entry shows once under the
message: its `title`, `explain` and `advice`, and a "Got it" that hides it for that message.
Several matching entries show in the file's order.

**Links from strangers:** a direct message from a stranger that contains a link shows one line
under it, "<name> is not your friend. Links open only when you choose.", and the link opens only
after a click on that line's Open button (the existing click-to-load picture rule already covers
pictures).

**The switch:** Settings > Safety, "Warnings on messages", On by default; Off hides them all.
Persisted with the other safety settings on each client.

**The recovery-phrase guard (always on, no switch, no "send anyway").** Before anything you
write is sent (a direct message, a channel post, a reply, a group message, a contact request's
name, a profile field), your own client checks it against your own recovery phrase: normalise
both the same way as above; if the text contains a run of 4 or more of your phrase's words
consecutively and in the phrase's order (words made only of digits are left out of the text
first, so a phrase pasted as a numbered list, "1. word 2. word ...", the way backup screens show
it, is still caught; added 2026-10-10), the send is stopped and nothing leaves, with: "This is
your recovery phrase. Anyone who has it owns your identity and everything in it. Nobody
legitimate will ever ask for it. Remove it to send the rest." The phrase is derived on the
device from what the client already holds (never stored anywhere new, never sent anywhere).
When the identity is locked and the phrase cannot be derived, the guard cannot run; say nothing
and let the send go as today.

**Proof:** on both clients, unit tests of the matcher on the examples above (including Unicode
text, punctuation inside a phrase, and a near miss), the friends and strangers rule, the link
line, the switch, and the guard (a run of 4 words stops the send, 3 words or the words out of
order do not, the full phrase stops it, the phrase as a numbered list stops it, a locked
identity sends). A cross-client test: a Node
test that runs the web matcher over a shared list of cases in `scripts/tests/fixtures/` and a
Rust test that runs the native matcher over the same file, so the two cannot drift.

## 11. Docs to update as each piece ships

- `docs/accord/conformance_gaps.md` ("Contact consent cannot be withdrawn")
  and the note in `docs/accord/communication_and_association.md`; then
  `node scripts/build-library.js` to resync `data/library/`.
- `docs/user/skills/handling_conflict.md`, the passage that says no block
  exists, and its file list at the end.
- CLAUDE.md Cryptography table (friendship certificate row; a report row; a
  forwarder note under transport privacy) and the storage schema list
  (`friend_cert_revocations`, the `server_members` columns, the new `reports`
  shape).
- `docs/PRIORITIES.md`: close TIER 1 item 2 now (the directory opt-out
  shipped in v0.425.0; the in-chat list residual moves here), and point TIER
  1 items 3 and 4 and TIER 2 item 6's certificate and native TURN entries at
  this document.
- `docs/design/report-system.md`: a note at the top that section 8 here
  replaces its DM section.
- `docs/FEATURES.md` and `docs/PAGES.md` when Settings > Safety and the
  Reports page exist.
- The in-app privacy explanation (the privacy tier screen) for the address
  facts in 7.1 until increment 9 ships, as PRIORITIES TIER 1 item 4 already
  asks.
