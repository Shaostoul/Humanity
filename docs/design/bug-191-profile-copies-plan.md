# BUG-191 build plan: people's apps sign the copy of their profile that other servers keep

Date: 2026-10-12. Read against main at cd6cc01d3. Sources: docs/BUGS.md BUG-191, BUG-194 to BUG-197 and BUG-200, docs/network/server_federation.md, docs/design/interplanetary.md (rule 3), docs/design/storage-architecture.md, docs/reference/retention_and_deletion_semantics.md, CLAUDE.md (Cryptography table), and the code cited below. I read every line number today. I edited nothing and booted nothing.

## 0. In short

- **What is broken.** A save is stored in `signed_profiles` with an empty signature and gossiped with an empty signature (src/relay/handlers/msg_handlers.rs:556-571). Every peer refuses it (src/relay/handlers/federation.rs:110-112), so nothing replicates. The copy's time is the origin server's clock (msg_handlers.rs:550).
- **What leaves a server: the public copy.** The copy is the profile as anyone may see it:
  - name, bio, avatar, banner and links always;
  - pronouns, location and website blank when the person keeps them for friends;
  - every field blank (a "withdrawal") when the person is unlisted.
  - Friends-only fields never leave the server, signed or not.
- **Who signs, and when.** The app signs after the save, not inside it.
  - The relay already sends the person's own complete copy to every device signed in under their name (msg_handlers.rs:505-527).
  - That own copy gains `signed_at` (the time of the copy this server holds for that device's key) and `sign_now` (the server asks for a signature).
  - Each device that is asked signs the public copy of exactly what the server stored, with its own key, and sends a new `profile_sign {at, signature}`.
  - The relay recomputes the public copy from what it stored, checks the signature, keeps it, and passes it on.
  - Why not sign inside `profile_update`: the app would have to repeat the relay's clean-up of the bio and the trims byte for byte. Also, a device linked by a link code has its own key, so it has to re-sign when someone makes a field friends-only on another device, which needs the relay to ask anyway. One path does both.
- **The time on a copy:** `max(now, signed_at + 1, last signed here + 1)`, the friendship choice notes' rule (src/net/dm_store.rs:726-734, web/chat/chat-dm-store.js:585-593).
- **Agreeing byte for byte.** There is one function per side:
  - Rust: `relay::core::profile_copy`, used by the relay and the desktop;
  - web: web/shared/profile-copy.js;
  - both are pinned by a new scripts/tests/fixtures/profile-copy-v1.json that the Rust builder makes (ML-DSA signing in Rust is deterministic, as for file-offer-v1.json).
  - The signed words stay `profile_v1\n...` (federation.rs:30-53). The one new rule: no line break in any field but the bio, so the words split only one way.
- **A copy that does not match** is not kept or passed on. The relay keeps the copy it holds and logs a line (key cut to 16 characters). The person is told only when they can fix it (their clock is more than a day ahead).
- **Making a field friends-only** asks every device of the person to sign again. The new copy blanks the field, and every linked server that holds the old copy replaces the whole row, so the old value is gone there. A server that was down gets it when its link comes back up (new catch-up).
- **Going unlisted** signs a withdrawal, which replaces every copy already passed on. Today the relay only deletes its own row (msg_handlers.rs:551-555) and peers keep theirs forever. A server that never held a copy keeps no trace of a withdrawal.
- **Pictures.** `avatar_url` / `banner_url` are replicated as they are: public https links that show while the server holding the file is up (section 3.12).
- **Found while reading, and it matters for scope:** no app reads a replicated copy yet. Only `GET /api/profile/{key}` serves `signed_profiles` (api.rs:2369-2430). Chat on public.guide shows only public.guide's own profiles (msg_handlers.rs:643 `get_public_profile`). This fix makes the copy correct and private. Showing it is question 3.
- **Room in the files.** msg_handlers.rs is at 4,653 of 4,660 lines, so Wave 0 moves the two profile handlers (374 lines) into a new src/relay/handlers/profile.rs.
- **Waves:**
  - one builder for Wave 0;
  - four lanes after it (relay sender, relay receiver, desktop, website);
  - one proof lane on two real relays;
  - docs last.

## 1. Today's code (file:line)

### 1.1 The save (src/relay/handlers/msg_handlers.rs)
- `handle_profile_update` is at :315-589.
- Steps in order:
  - 30 s limit per socket (:332-343);
  - the bio is cleaned of "<...>" and every ">" (:345-358) but not trimmed;
  - limit of 280 characters (:360-369);
  - links: JSON, at most 1,024 bytes, https only (:371-435);
  - avatar, banner, website, pronouns and location are trimmed, and URLs must start with https:// (:437-454);
  - the privacy map must be any JSON (:456-463), not necessarily an object of words;
  - `accepts_files` (:465-481);
  - save (:483-486).
- **Public copy to everyone** (:490-503): bio, links, avatar and banner, never pronouns, location or website.
- **Own copy to every key under the name** (:505-527): privacy map, `stored: true`, and `name: name.clone()`, which is the SAVER's display name even for a linked device's key.
- **Unlisted check (the existing gossip gate)** (:529-542): it reads `directory == "unlisted"` from the raw `privacy`.
- **Public fields** (:544-549): `public_field` for pronouns, location and website.
- **Time** (:550): `ts = now_millis()`, the origin server's clock.
- **Unlisted** (:551-555): `delete_signed_profile(my_key)` on this server only. Peers keep their copy.
- **Listed** (:556-571): `store_signed_profile(..., ts, "")` and `gossip_profile(..., ts, "")`. Its comment names BUG-191.
- **`handle_profile_request`** (:591-682):
  - the own branch (:610-641) answers with the name ASKED for (:627), not the name the socket signed in as;
  - the public answer comes from `get_public_profile` (:643-660).
- `BIO_MAX_CHARS` is at :313 and is used by src/gui/pages/profile.rs:219.
- The tests are a `#[path]` child at :4552-4555 (msg_handlers_profile_tests.rs, 381 lines).

### 1.2 The wire and the connection (src/relay/relay.rs)
- Message shapes:
  - `ProfileUpdate` :1199-1227;
  - `ProfileData` :1229-1269 (`privacy`, `stored` and `target` are own-copy-only options);
  - `ProfileRequest` :1271-1281;
  - `ProfileGossip` :2183-2198.
- `RelayState` :157-316 (`federation_connections` :237, initialised :500).
- The own copy on connect is at :3217-3239 (`name` = `final_name`, the socket's display name; sent straight on `ws_tx`).
- `last_profile_update` per socket is at :3639. Dispatch is at :5839-5846.
- A display name can differ from the registered name's case, and a link code replaces it (:2990-3017, :3054-3112).

### 1.3 The copy for other servers (storage)
- src/relay/storage/signed_profiles.rs:
  - `store_signed_profile` :43-107: refuses more than 24 h ahead (:57-71); older or equal loses (:73-86); the whole row is replaced (:88-104); an empty signature is NOT refused at this level;
  - `delete_signed_profile` :26-41 (its doc says peers "age it out on their own retention", which does not exist);
  - `get_signed_profile` :109-135;
  - `get_profiles_since` :137-163 and `count_signed_profiles` :165-171 have no caller.
- The table is in src/relay/storage/mod.rs:2169-2188.
- The export reads the row at src/relay/storage/account.rs:78; the erase deletes it at :283.

### 1.4 Gossip (src/relay/handlers/federation.rs)
- **Signing and checking:**
  - `canonical_profile_message` :30-53: `profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{timestamp}`;
  - `verify_profile_signature` :55-82 (hex, Dilithium3);
  - `should_accept_profile_gossip` :84-117 (its doc :87-91 says clients have signed since the PQ cutover, which is false).
- **Receiving:**
  - `handle_peer_message`'s ProfileGossip arm :731-767: a verify in `spawn_blocking`, then store;
  - no field limits at all;
  - no passing on, unlike signed objects, which are forwarded to other peers (:851-863) and rate limited per source (:808-842).
- **Link up:**
  - outbound registration :295-306;
  - inbound registration :620-635;
  - FederationWelcome arm :727-730;
  - nothing is sent on link up.
- **Sending:** `gossip_profile` :971-1016 sends to every connected peer. Its doc :972-977 still describes empty-signature gossip.
- **Tests:** :1018-1158.
- **Two-relay test:** tests/federation_two_relays.rs:141-186 hand-makes one signed gossip (with `socials: ""`) and one unsigned one.

### 1.5 The privacy rules on the relay
- src/relay/storage/profile.rs:15-23 `public_field`: parses the map as `HashMap<String, String>`, so a map holding any non-string value makes every field public.
- `get_public_profile` :205-248: bio, links, avatar and banner are always shown; pronouns, location and website follow the map; it has its own copy of the parse.
- src/relay/api.rs:2369-2430 `get_signed_profile`:
  - an unsigned copy of someone with no account here blanks the three fields (:2396-2404);
  - a local account's copy has the map as it is now applied (:2405-2416, BUG-197).
- The map's words today:
  - the website writes `"private"` for pronouns, location and website, and `"directory": "unlisted"` (web/pages/profile.html:1150, :1227-1230);
  - the desktop writes only the directory (src/gui/pages/own_profile.rs:159-171).

### 1.6 The desktop
- src/gui/pages/own_profile.rs:
  - `OwnProfile` :49-71 (in `GuiState`, reset whole on a new identity at src/gui/mod.rs:3021);
  - `names_socket` :80-83;
  - `own_copy` :88-109 (links "" read as "{}");
  - `keep_copy` :112-121;
  - `whole_update` :176-192;
  - `want_save` :204-206;
  - `pump` :210-263.
- `ServerProfile` is at src/gui/pages/files_settings.rs:42-61.
- Own copies arrive on the active socket at src/engine/frame_ws_poll.rs:724-744 and on background sockets at src/engine/bg_connections.rs:775-784. A new sign-in drops the copy at :566-570.
- Signing material:
  - `GuiState.private_key_bytes` (the 32-byte seed) and `profile_public_key` (Dilithium3 hex), as the tests use them (src/engine/block.rs:442-453);
  - `net::identity::pq_object_keypair` :109-112 and `pq_sign_raw` :118-122.
- The Profile page's directory checkbox is at src/gui/pages/profile.rs:227-234.

### 1.7 The website
- web/chat/chat-profile.js:
  - `buildProfileUpdate` :100-120;
  - `adoptOwnProfile` :176-201;
  - `ownProfileArrived` :336-349 (own complete copies only: `typeof msg.privacy === 'string'`);
  - `pushProfileToRelay` :378-406.
- web/chat/app.js:1443-1475 routes `profile_data`, and the own copy at :1468-1473.
- Signing: `myDilithiumSecret` comes from `attachPqIdentity` (web/chat/crypto.js:802-838); `window.pqSignMessage` produces hex as in `pqSignChatMessage` (:847-860); `myKey` is the Dilithium hex (app.js:602).
- web/shared/profile-changes.js `tidyBio` :73-86 mirrors the relay's bio clean-up.
- Scripts are loaded at web/chat/index.html:725-727 and :756 (chat-profile.js?v=10).
- The directory words are at web/pages/profile.html:738-745; the privacy note is at :735.

### 1.8 Precedents to follow
- **Shared fixture:**
  - scripts/tests/fixtures/file-offer-v1.json;
  - held to the builder by `file_pass_fixture_matches_the_builder` (src/relay/core/pq_crypto.rs:1569-1593);
  - read by both apps' tests.
- **Pinned preimage read by the web test:** scripts/tests/friend-pass.test.js (reads `assert_eq!(friend_cert_preimage(...), "...")` and the consts out of pq_crypto.rs).
- **Recovery-phrase guard list:** scripts/tests/warnings-web.test.js:870-897. A `profile_sign` frame carries no text a person typed, so it is not added.
- **Gate:** src/relay/features.rs:371. `profile_` maps to Chat, so `profile_sign` is classified with no edit.

### 1.9 Constraints

| File | Lines | Budget (tests/file_size_ratchet.rs:376-417) |
|---|---|---|
| src/relay/handlers/msg_handlers.rs | 4,653 | 4,660 |
| src/relay/relay.rs | 6,322 | 6,400 |
| src/relay/api.rs | 3,754 | 3,930 |
| src/engine/frame_ws_poll.rs | 1,698 | 1,800 |
| src/gui/mod.rs | 4,683 | 4,800 |
| src/lib.rs | 16,476 | 16,700 |

- **Lints:** emdash_lint, terminology_lint (never "seed phrase" in what people read), theme_token_lint.
- **app/web/** is regenerated by scripts/bundle-web.js. Never edit it.

## 2. Stale words this fix corrects

| Where | Says | Now |
|---|---|---|
| msg_handlers.rs:557-562 | the signature is empty because no app signs | gone; the device signs after the echo |
| federation.rs:87-91 | "clients have signed their profiles since the full-PQ cutover" | they sign from this release |
| federation.rs:972-977 | pass an empty string; peers accept under trust-by-source | an empty signature is never kept or sent |
| signed_profiles.rs:1-3, :26-33 | the doc of `store` sits on `delete`; peers "age it out on their own retention" | no retention exists; a withdrawal replaces it |
| docs/network/server_federation.md:135-177, :211 | "On connect: Client sends ProfileUpdate with signed profile data"; a message format showing a signed ProfileUpdate; "Tier 2+", "100 per minute", "< 10KB" | the app signs after the echo (`profile_sign`); copies go to every linked peer; there is no gossip rate limit; a signed copy is about 13 KB |
| docs/design/storage-architecture.md:138 | "It pulls your signed profile via federation gossip" | nothing pulls: copies are pushed on save and on link up |
| docs/design/interplanetary.md:51-55, :90-91 | profiles are dated by the server; the apps do not sign | dated by the device after the newest seen |
| docs/reference/retention_and_deletion_semantics.md:350-356 | going unlisted "retracts the local replicated copy" | it withdraws the copy on every linked server that had it |
| CLAUDE.md:534 | "NOT working end to end (BUG-191)" | the row as built |
| CLAUDE.md:828 | `signed_profiles (public_key, name, bio, avatar_url, socials, timestamp, signature)` | plus banner_url, pronouns, location, website |
| docs/ROADMAP.md:238-241 | `[building]`, every gossiped profile refused | done |
| web/shared/profile-changes.js:73-75 | "src/relay/handlers/msg_handlers.rs, the profile save's clean-up" | src/relay/handlers/profile.rs |

## 3. The design

### 3.1 What leaves the server: the public copy (THE privacy rule)
`relay::core::profile_copy` (new, compiled in the relay and the desktop, since `native` includes `relay`):

```rust
pub const FIELDS_FOR_FRIENDS: [&str; 3] = ["pronouns", "location", "website"];
pub struct PublicCopy { pub name, bio, avatar_url, banner_url, socials, pronouns, location, website: String }
```

**Reading the privacy map (one rule, both sides).**
- Parse the text as JSON. If it is not an object, nothing is kept for friends and the person is listed.
- `unlisted(map)`: `map["directory"]` is the string `"unlisted"`.
- `kept_for_friends(map, f)`: `map[f]` is the string `"private"`.

**`public_copy(name, bio, avatar_url, banner_url, socials, pronouns, location, website, privacy)`:**
- **Unlisted:** the withdrawal. Every field is `""`, the name included, and links are `"{}"`.
- **Otherwise:**
  - the exact stored text of name, bio, avatar and banner;
  - links are the exact stored text, with blank read as `"{}"`;
  - each of `FIELDS_FOR_FRIENDS` is `""` when kept for friends.
- No trimming and no reformatting: the input is what the server stored, which the own copy carries.
- `is_withdrawal(c)` is `c.name.is_empty()`. A real name is never empty (relay.rs:3022).

**The relay uses the same rule everywhere.** `storage::profile::public_field` and `get_public_profile`'s `is_private` call `kept_for_friends`. The save refuses a privacy map that is not an object whose values are all strings ("Invalid privacy map."), so both apps only ever meet well-formed maps.

**`accepts_files` is never in the copy.** It is this server's setting, as api.rs:2393-2395 already says.

### 3.2 The words signed
- **`copy_text(public_key, c, at)`** is today's `canonical_profile_message`, moved from federation.rs:30-53 into `profile_copy`:
  - `profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{at}`;
  - UTF-8 bytes; `at` in decimal milliseconds;
  - signed with Dilithium3; the signature is 6,618 lowercase hex characters.
- **Not ambiguous.** No field but the bio may hold a line break, so the words split one way only: the last 8 lines are fixed, the first two after the tag are fixed, and the bio is the rest.
- **`within_limits(public_key, c)`**, checked by both the sender and the receiver. It holds the save's own limits:
  - the key is 3,904 hex characters;
  - the name is empty or 1 to 24 of [A-Za-z0-9_-];
  - a withdrawal has every other field empty and links `"{}"`;
  - the bio is at most 280 characters and holds no `<` or `>`;
  - links are a JSON object of at most 1,024 bytes;
  - avatar and banner are at most 512 bytes; website at most 256; each is empty or starts with `https://`;
  - pronouns are at most 64 characters; location at most 128;
  - no `\n` in any field but the bio.
  - `BIO_MAX_CHARS` moves here from msg_handlers.rs:313.
- **The save also refuses a line break** in pronouns, location, avatar, banner, website or the links: "Pronouns, location and links cannot hold a line break." No app's inputs can type one.
- **The version tag stays `profile_v1`.** Nothing was ever signed under it, so there is nothing to shim.

### 3.3 Asking, and who signs
- **`ProfileData` gains two fields for own copies only** (`None` elsewhere, `skip_serializing_if`):
  ```rust
  /// The time of the copy for other servers this server holds for the receiving key (0: none).
  signed_at: Option<u64>,
  /// This server asks the receiving device to sign its public copy now.
  sign_now: Option<bool>,
  ```
- **The own copy's `name` is always the receiving socket's display name.** This fixes three sites:
  - the echo loop (:513) uses each target key's `peers[key].display_name`, falling back to the saver's name;
  - the own request (:627) uses `peers[my_key].display_name`;
  - connect is already right.
  - The app signs with exactly this name.
- **`sign_wanted(row, now, just_saved, privacy)`**, in core. It is false when `!within_limits(now)`. Otherwise:
  - no copy held here: `just_saved && !is_withdrawal(now)`;
  - `now` is a withdrawal: the held copy is not one;
  - just saved: held copy `!=` now;
  - otherwise: `exposes(held, privacy)`, which means the held copy is not a withdrawal AND (the person is unlisted here OR a field kept for friends here is non-empty in it).
- **Where it applies:**
  - the save echo, for EVERY key under the name: `just_saved = true`, so every online device re-signs and the person's keys agree;
  - the own copy on connect (relay.rs:3217-3239) and the own request: `just_saved = false`, so on connect a device signs only to narrow what is out there, never to push an older profile over a newer save made elsewhere.
- **`profile::ask_for_copy(state, key, &now, privacy, just_saved) -> CopyAsk { signed_at, sign_now }`:**
  - reads `get_signed_profile(key)`;
  - records an ask in a new `RelayState.profile_sign_asks: std::sync::Mutex<HashMap<String, Instant>>` when `sign_now`;
  - never asks for a key that is not 3,904 hex characters (bots).

### 3.4 The time on a copy
- **`next_copy_time(now_ms, signed_at, last_here) = max(now_ms, signed_at + 1, last_here + 1)`** (saturating).
  - `signed_at` comes from the own copy. It is this server's row for the key, which includes a newer copy that arrived from a linked server.
  - `last_here` is the app's own last signed time for that server in this run. It is in memory: `OwnProfile.signed_here: HashMap<String, u64>` on the desktop, and `profileCopyLastAt` on the website.
- **A device with a fast clock no longer wins every conflict.** The next edit from any device is dated after it, and it is still accepted while within the 24 h that receivers allow.

### 3.5 The relay: checking, keeping and passing on (`handle_profile_sign`, new in handlers/profile.rs)
Wire: `{"type":"profile_sign","at":1760000000001,"signature":"<6618 hex>"}` (`RelayMessage::ProfileSign { at: u64, signature: String }`). Steps, cheapest first:

1. **An ask for this key younger than 60 s**, else drop. An ask is used up only by a copy that is kept or that is found not newer than the held one. A mismatch leaves the ask, because a save from another device may have changed the profile in between, and that device's copy is coming.
2. **Shape:** exactly 6,618 hex characters, else drop.
3. **Name:** the socket's display name, else drop.
4. **Profile:** `get_profile_extended(name)`. None: drop. An error: log, drop.
5. **The copy:** `now = public_copy(...)`. `!within_limits`: drop.
6. **Clock:** `at > now + 24 h`: Private notice "Your profile is saved on this server, but it was not passed on to the servers this one is linked with: your device's clock is more than a day ahead. Set your clock right, then save your profile again."
7. **Newer:** the held copy has `timestamp >= at`: drop and use up the ask.
8. **Check:** `verify_copy` in `spawn_blocking`; a panic counts as no. No: drop, no notice. The app signs on its own, so the only causes are a race with another device's save (followed by its own signing) or a broken app (which the shared fixture catches). A notice they cannot act on is noise.
9. **Keep:** `store_signed_profile`. Kept: `gossip_profile(.., None)`.

- Log lines name the key by 16 characters (`key_in_log`), for example: "Profile: kept and passed on the copy for other servers of {key16} (withdrawn)".
- **The save itself no longer writes `signed_profiles`.** msg_handlers.rs:529-571 is replaced by the asks in the echo.

**Storage rules.** `store_signed_profile(key, &PublicCopy, ts, sig)` refuses:
- an empty signature;
- a copy more than 24 h ahead;
- a copy not newer than the held one;
- a withdrawal when nothing is held.

A withdrawal over a held copy replaces the whole row. At open, `DELETE FROM signed_profiles WHERE signature = ''`. Those rows were never valid copies, and the ones from before 2026-10-11 hold fields kept for friends.

### 3.6 The receiving server
- **The ProfileGossip arm** builds a `PublicCopy`. `should_accept_profile_gossip(key, &copy, ts, sig)` requires:
  - a signature;
  - `within_limits`;
  - `verify_copy`.
- **Then `store_signed_profile`.** A withdrawal for a key this server never held is kept nowhere: nothing is stored and nothing is learned.
- **Passing on.** A newly kept copy whose key has an account here (`registered_names`) goes to this server's OTHER peers (`gossip_profile(.., Some(source))`), as signed objects already do (federation.rs:851-863).
  - It does not loop: an equal or older copy is not kept, so it is not passed on.
  - With two servers it never fires. With a third it carries a withdrawal or a field made friends-only to every server that has the copy.

### 3.7 Catch-up when a link comes up
- **`send_own_copies(state, peer_id)`** queues on that peer's `tx` this server's copies for keys with an account here, withdrawals included, newest first, at most 1,000 (`copies_of_accounts_here`, which replaces the unused `get_profiles_since`).
- **Called from two places:**
  - in `run_inbound_peer` after registration (after :634);
  - in the FederationWelcome arm (:727-730) for the outbound side.
- **So a server that was down when a person made a field friends-only or went unlisted learns it when the link returns.** This is the public.guide case.

### 3.8 What happens, server by server

| Person does | Their own server | Linked server holding an older copy | Linked server holding none |
|---|---|---|---|
| Saves, listed | asks every online device; keeps the newest valid copy | replaces the whole row | keeps it |
| Makes location friends-only | hides it at once in chat and in both lookups (BUG-197); asks every online device | the new copy has location `""`; the old value is gone | keeps the copy without it |
| Goes unlisted | asks every device whose key has a copy; keeps a withdrawal; `/api/profile/{key}` answers 404 at once | replaces the row with the withdrawal; 404 | keeps nothing |
| A device that was offline connects | asks it only if its key's copy shows more than allowed here | as above, once it signs | as above |
| The link was down | (nothing) | catch-up on link up | catch-up on link up |
| Erases their account | the row goes (account.rs:283) | **keeps the copy** (question 2) | n/a |

**One copy per key, network-wide.** The person's latest signed copy wins on every server. Two servers with different profiles for the same person converge on the last one saved, and a narrowing wins at the next connect (question 1).

### 3.9 This server's lookup (`GET /api/profile/{key}`, api.rs:2369-2430)
- Not found when:
  - the held copy is a withdrawal;
  - the key has an account here and that account is unlisted here (core `unlisted` on its map).
- The unsigned-copy branch (:2396-2404) is removed; no unsigned copy exists any more.
- Kept: a local account's copy is answered with the map as it is now (:2405-2416). That covers the seconds between a save and its signature, and a signature that failed.

### 3.10 The desktop
- **`own_profile::take_own_copy(state, server, val, socket_name) -> Option<String>`** replaces the `own_copy` + `keep_copy` pair at both arrival sites (frame_ws_poll.rs:738-741, bg_connections.rs:780-783). Each site sends the returned frame on its own socket.
  - It keeps the copy as today.
  - If `val["sign_now"] == true` and the identity is unlocked (`private_key_bytes` is 32 bytes and `profile_public_key` is set), it builds `public_copy(val["name"] exactly, the copy's fields, its privacy)`.
  - It sets `at` from `next_copy_time(now, val["signed_at"], signed_here[server])`, signs with `profile_copy::sign_copy`, records `signed_here`, and returns `{"type":"profile_sign","at","signature"}`.
- **A pure inner function** `sign_frame(seed, key, name, &ServerProfile, signed_at, last_here, now_ms)` is the one the fixture test calls.
- **Locked:** one log line, nothing sent.
- **Profile page words:** under the directory checkbox (profile.rs:227-234), in muted text: "Off: you are left out of this server's member list, and your profile is not passed on to the servers this one is linked with."

### 3.11 The website
- **web/shared/profile-copy.js** (new; a classic script and a CommonJS module, like profile-changes.js) exports `FIELDS_FOR_FRIENDS`, `unlisted`, `keptForFriends`, `publicCopy(name, p, privacyText)`, `withdrawal`, `copyText(key, c, at)` (`String(at)`) and `nextCopyTime`.
- **chat-profile.js `signProfileCopy(msg)`:**
  - guarded on `window.HosProfileCopy`, `myDilithiumSecret`, `window.pqSignMessage` and an open `ws`;
  - signs `copyText(myKey, publicCopy(msg.name, msg, msg.privacy), at)` (`msg.socials` is used as the string it is, never re-stringified);
  - sends `profile_sign`;
  - is called from `ownProfileArrived` right after its own-copy check, when `msg.sign_now === true`.
- **index.html** loads `/shared/profile-copy.js?v=1` after profile-changes.js and bumps chat-profile.js to `?v=11`.
- **profile.html words:**
  - directory hint (:744): "...without appearing in the member list at all. It also keeps your profile off the servers this one is linked with.";
  - privacy note (:735): "Public = visible to everyone, here and on the servers this one is linked with."

### 3.12 Pictures (`avatar_url`, `banner_url`)
- **Replicated as they are. This is fine:**
  - They are public by design (get_public_profile :233-234; profile.html:759).
  - The save already requires a full https link (msg_handlers.rs:444-454; an upload answers with a relative `/uploads/..`, upload_rules.rs:326, which a person turns into a full link).
  - An uploaded picture's link names the server that holds the file, so it shows while that server is up and stops when the file is removed or the account erased.
- **Copying the files to other servers is out of scope.**
- **When an app first shows a replicated copy**, it should know that loading the picture tells the holding server the viewer's address, as any linked picture does today.

### 3.13 After release on the two live servers
- **Old unsigned rows** are dropped at open (3.5).
- **Profiles go out the next time each person saves or changes a privacy choice.** Connecting alone never signs a listed profile that has no copy yet (3.3), so a stale server can never overwrite a newer save.
- **public.guide follows releases hourly** (BUG-192), so both run the same version within the hour. An older receiver still verifies the same words.

## 4. Questions only the operator can answer (each with a recommendation)

1. **One copy per person across all linked servers, latest wins.**
   - The copy is per key, not per server, as the design always said ("no home server").
   - If you keep different profiles on united-humanity.us and public.guide, linked servers carry whichever you saved last.
   - A field you keep for friends on either server is blanked at your next connect there.
   - Each server's own chat profile is unchanged.
   - **Recommend: accept.** The alternative is one copy per (key, server). That needs the server's did:hum in the signed words and a new table key, and is only worth it if different public profiles per server are to be shown side by side.
2. **Erasing your account leaves your copy on linked servers.** Only your key can sign a withdrawal, and the erase does not sign one.
   - **Recommend: include it as Wave 2B (about half a day):**
     - `account_delete` carries an optional `withdrawal {at, signature}` over the withdrawal copy;
     - both apps sign one before erasing (web/chat/chat-privacy.js:253, src/gui/pages/settings.rs:547);
     - the relay checks and passes it on before it erases (no ask needed: the erase is the ask);
     - the docs say that an admin's erase cannot withdraw it, because only the person's key can.
   - If not, the docs say an erase leaves the copy on linked servers.
3. **Nothing shows a replicated copy yet.** Only `GET /api/profile/{key}` serves it, and public.guide's chat shows only profiles saved on public.guide.
   - **Recommend: ship this fix as scoped.** It makes the copy correct, signed and private.
   - Decide separately whether a server's chat should show a linked server's copy for someone with no profile there. If so, label it with the server it came from, because names are per server.

## 5. Waves

**Ownership.** Edited by two waves in sequence, never in parallel:
- handlers/profile.rs and profile_tests.rs: 0, then 1A;
- federation.rs: 0, then 1B;
- tests/federation_two_relays.rs: 0, then 2A;
- src/gui/pages/profile.rs: 0, then 1C.

Wave 0 only: relay.rs, msg_handlers.rs, storage/signed_profiles.rs, storage/mod.rs, handlers/mod.rs, core/*, the fixture, tests/file_size_ratchet.rs.

No wave touches: lib.rs, gui/mod.rs, features.rs, app/web.

**Budgets after all waves:**

| File | Lines |
|---|---|
| msg_handlers.rs | about 4,279 (-374) |
| relay.rs | about 6,350 (+28) |
| api.rs | about 3,752 |
| frame_ws_poll.rs | about 1,700 |
| gui/mod.rs | 4,683 (unchanged) |
| lib.rs | 16,476 (unchanged) |

Add `src/relay/handlers/profile.rs` to the ratchet at 650 (Wave 0).

**Rule for every test:** it is red on the code before its wave with the named break, and its comment records the failure text.

### Wave 0: the contract and the room (one builder, merges first)
**Files:**
- NEW src/relay/core/profile_copy.rs and core/mod.rs: everything in 3.1 to 3.4 (`PublicCopy`, the map rule, `public_copy`, `withdrawal`, `copy_text`, `sign_copy`, `verify_copy`, `within_limits`, `sign_wanted`, `exposes`, `next_copy_time`, `BIO_MAX_CHARS`, `MAX_AHEAD_MS`).
- NEW scripts/tests/fixtures/profile-copy-v1.json, written by an `#[ignore]` test from signer byte 7. Four cases:
  1. listed, location kept for friends, pronouns marked "public", a Greek bio with a line break, links `{"github":"ann","discord":"ann#1"}` in that order, `signed_at` ahead of `now`;
  2. unlisted;
  3. links `""` and map `"{}"`;
  4. map `"[]"`.
  - Each case holds the own-copy frame, `now_ms`, the expected copy, `at`, the text and the signature. There is also a table of `next_copy_time` rows.
- src/relay/relay.rs:
  - `ProfileSign`;
  - `ProfileData.signed_at` / `sign_now`;
  - `RelayState.profile_sign_asks` and its init;
  - the dispatch arm;
  - the connect copy calls `profile::ask_for_copy` (stub).
- NEW src/relay/handlers/profile.rs: msg_handlers.rs:309-682 moved verbatim, with:
  - stubs for `handle_profile_sign` (log and drop) and `ask_for_copy` (`{0, false}`);
  - `None` added to every `ProfileData`;
  - the unsigned store and gossip (:556-571) removed (the unlisted delete stays until 1A).
- handlers/mod.rs: `pub mod profile;` and re-export of the three handlers.
- `git mv` msg_handlers_profile_tests.rs to profile_tests.rs (child of profile.rs). Tests that relied on the save writing a copy store a signed row themselves. `a_copy_for_someone_no_longer_here_leaves_out_the_private_fields` is deleted with a note, because its case can no longer arise.
- storage/signed_profiles.rs: the new `store_signed_profile` rules; `copies_of_accounts_here`; `SignedProfileRecord::copy()`; doc fixes. storage/mod.rs: the open-time DELETE.
- federation.rs:
  - the canonical text and the verify move to core;
  - `should_accept_profile_gossip(key, &PublicCopy, ts, sig)`;
  - `gossip_profile(state, key, &PublicCopy, ts, sig, exclude: Option<&str>)` (final signature, so 1A and 1B never collide);
  - the arm builds the copy;
  - its tests are adapted.
- tests/federation_two_relays.rs: built with core; links `"{}"`.
- src/gui/pages/profile.rs:219: the `BIO_MAX_CHARS` path.
- account.rs test :769: the call shape.

**Tests:**
- `the_copy_for_other_servers_holds_only_what_anyone_may_see`: all four map cases. Red with the `"private"` check removed: "a location kept for friends went into the copy for other servers".
- `the_copy_text_is_pinned`: an `assert_eq!(copy_text(..), "profile_v1\n...")` literal. Red with location and website swapped.
- `each_copy_is_dated_after_the_newest_seen`. Red with `now_ms` returned alone.
- `a_copy_over_the_limits_or_with_a_line_break_outside_the_bio_is_refused`. Red with the line-break rule removed: "a location holding a line break passed".
- `the_relay_asks_for_a_copy_only_when_one_is_due`: a truth table over `sign_wanted`. Red with `exposes` ignoring the directory: "an unlisted person's listed copy was left out there".
- `the_shared_fixture_is_what_the_builder_makes`: copy, text and signature byte for byte, and one changed character fails `verify_copy`. Red with the fixture's bio edited.
- signed_profiles.rs:
  - `a_copy_without_a_signature_is_never_kept`;
  - `a_withdrawal_replaces_a_held_copy_and_leaves_no_trace_otherwise` (red with the "nothing held" rule removed);
  - `a_copy_more_than_a_day_ahead_or_not_newer_is_not_kept`;
  - `the_catch_up_holds_only_this_servers_accounts_newest_first`;
  - `unsigned_copies_from_before_are_dropped_at_open` (BUG-046 shape: a raw old database with an unsigned row, then open).
- The moved tests and federation.rs's tests pass. `cargo test --features relay --no-default-features --test federation_two_relays` passes.

### Wave 1 (four lanes in parallel)

**1A The relay sends: asks, checks, keeps, passes on.**
Files: handlers/profile.rs, profile_tests.rs, storage/profile.rs, api.rs.
- The save: the privacy map must be an object of words; no line break in a one-line field; the echo asks every key (3.3) with each target's display name; msg_handlers' unlisted, delete, store and gossip block is replaced.
- `handle_profile_request` and `ask_for_copy` as in 3.3. `handle_profile_sign` as in 3.5.
- `public_field` and `get_public_profile` use the core rule. api.rs as in 3.9.

Tests (profile_tests.rs):
- `a_save_asks_each_of_the_persons_devices_to_sign`: Ann and her linked phone both get `sign_now` and `signed_at: 0`; the public copy carries neither. Red with the echo's `sign_now` left `None`.
- `a_signed_copy_is_kept_and_passed_on_without_what_friends_see`: a fake peer `tx` gets one `profile_gossip` with location `""`. Red with the relay's copy built without the map: "Lisbon reached the other server while Ann kept it for friends".
- `a_copy_that_does_not_match_is_not_kept_or_passed_on`: no row, no gossip, no notice. Red with the verify result ignored.
- `a_copy_nobody_asked_for_is_dropped`. Red with the ask check removed.
- `a_copy_for_an_older_save_does_not_use_up_the_ask`. Red with the ask taken before the check.
- `a_copy_dated_more_than_a_day_ahead_is_refused_with_words`: the notice names the clock. Red with the clock check left to the store (no notice).
- `going_unlisted_signs_a_withdrawal_that_replaces_the_copy`: the gossip carries an empty name; the API answers 404. Red with the old delete-only path.
- `a_connect_asks_only_when_the_copy_out_there_shows_more_than_allowed`: own request, then the map is narrowed in the database, then asked again. Red with `held != now` on connect: "a benign difference asked to sign on connect".
- `the_own_copy_names_the_socket`: a request for "ann" is answered "Ann". Red with the asked name used.
- `a_privacy_map_must_be_words` and `a_line_break_in_a_one_line_field_is_refused`.
- storage/profile.rs `the_relay_reads_a_privacy_map_one_way`. Red with the HashMap parse kept.

**1B The relay receives: limits, the passing-on rule, catch-up.**
Files: federation.rs.

Tests:
- `gossip_a_save_would_refuse_is_not_kept`: a 281-character bio, a location with a line break, an http avatar. Red with `within_limits` skipped: "a copy whose location holds a line break was kept".
- `a_withdrawal_from_a_peer_replaces_the_copy_held_and_nothing_else`.
- `a_new_link_is_sent_this_servers_own_copies`: a fake `FederatedConnection` with an mpsc channel; the key with an account goes, the gossiped one does not. Red with the account filter removed.
- `a_copy_kept_for_one_of_ours_goes_on_to_the_other_links_and_not_back`: two fake peers; a repeat forwards nothing. Red with the exclusion removed: "the copy went back to the server it came from".

**1C The desktop.**
Files:
- src/gui/pages/own_profile.rs and own_profile_tests.rs;
- src/engine/frame_ws_poll.rs:738-741 (net +2 lines);
- src/engine/bg_connections.rs:780-783;
- src/gui/pages/profile.rs (the words, and the :229 comment).

Tests:
- `a_copy_that_asks_is_signed_once_for_its_own_server`: `WsClient::recording_as`; `verify_copy` passes over the frame's public copy; no `sign_now` means `None`. Red with `sign_now` ignored.
- `the_desktop_signs_the_fixtures_copies_byte_for_byte`. Red with the name taken from the socket instead of the copy.
- `each_edit_is_dated_after_the_newest_seen_on_that_server`. Red with `signed_here` not recorded.
- `a_locked_identity_signs_nothing`.

**1D The website.**
Files:
- NEW web/shared/profile-copy.js;
- web/chat/chat-profile.js;
- web/chat/index.html;
- web/pages/profile.html;
- NEW scripts/tests/profile-copy-web.test.js;
- web/shared/profile-changes.js:74 (the comment's path);
- Justfile `rig-tests` (:628).

Tests:
- The fixture's copies and texts come out of `publicCopy` and `copyText`, and the fixture's Rust signatures verify over the web's text (noble `ml_dsa65.verify`, comparing nothing byte for byte, since noble's signing is hedged). Red with `keptForFriends` reading `"friends"`.
- The pinned literal and `FIELDS_FOR_FRIENDS` are read out of profile_copy.rs (the friend-pass.test.js way).
- `nextCopyTime` rows.
- In a small VM (profile-changes.js, profile-copy.js, chat-profile.js; stub `ws`, `document`, `localStorage`; `pqSignMessage` = noble with a fixed seed):
  - an own copy with `sign_now: true` and `signed_at` a minute ahead sends exactly one `profile_sign`, with `at = signed_at + 1`, whose signature verifies;
  - without `sign_now`, or on a public copy, nothing is sent.
  - Red with the `sign_now === true` check removed: "signed a copy nobody asked for".

### Wave 2
**2A The proof on two real relays** (tests/federation_two_relays.rs, new test `a_saved_profile_reaches_the_other_server_with_only_what_anyone_may_see`):
- Ann (seed 7) signs in on A through a real `/ws` socket (the identify handshake, as src/relay/features.rs:756-797 does).
- She saves with location friends-only, reads her own copy (`sign_now`), signs with `profile_copy`, and sends `profile_sign`. B keeps her bio with location `""`, and the signature checks on B.
- On a new socket each time (the 30 s limit is per socket):
  - location public: B shows "Lisbon";
  - friends-only again: B's location is `""`;
  - unlisted: B holds a withdrawal and `api::get_signed_profile` on B and on A answers 404.
- A third relay C, linked only after all that, gets Ann's current copy by catch-up.
- Red:
  - with `gossip_profile` taken out of `handle_profile_sign`: "B never had Ann's profile";
  - with `send_own_copies` not called: "C never heard Ann's copy".

**2B (only if question 2 is yes)** The erase withdrawal:
- relay.rs:2014-2020 and :6080 (`AccountDelete.withdrawal`);
- msg_handlers.rs `handle_account_delete` (:1943);
- web/chat/chat-privacy.js:253;
- src/gui/pages/settings.rs:547.
- Test: an erase on A leaves B with a withdrawal. Red with the withdrawal not passed on before the erase.

**Docs (orchestrator, last):**
- every row of section 2;
- docs/BUGS.md BUG-191 as FIXED, with the design;
- CLAUDE.md's file map (handlers/profile.rs, core/profile_copy.rs, web/shared/profile-copy.js);
- docs/STATUS.md;
- the journal.

**Checks per merge:**
- `just verify`;
- `cargo check --features relay --no-default-features`;
- the two-relay test after Wave 0, 1B and 2A;
- `just rig-tests` after 1D;
- `just verify-relay` after 1A.

## 6. After release: the live check (read-only; the operator saves)
1. On united-humanity.us, save your profile on the website with location set to Friends.
2. `curl -s https://public.guide/api/profile/<your key>` shows your bio, an empty location and a signature. united-humanity.us answers the same.
3. Untick the directory and save. Within seconds both answer 404.
4. Tick it and save. Both show it again.

## 7. If it must land in one day
**Keep:** Wave 0, 1A, 1B's limits, 1C, 1D, and 2A's first three legs.

**Cut, in this order:**
1. The words under the directory choice in both apps (the docs say it).
2. Wave 2B, the erase withdrawal (the docs say an erase leaves the copy on linked servers).
3. Passing a member's copy on to other links (it matters only with three or more servers; today there are two).
4. Asking on connect. A device that was offline during a narrowing save leaves its key's old copy out there until it saves.
5. Last, the catch-up on link up. Without it, a change made while public.guide is down reaches it only at the person's next save, which is the very case it is there for.

**Never cut:**
- the public copy rule;
- the withdrawal;
- the receiver's limits and line-break rule;
- the shared fixture;
- dating after the newest seen.

## 8. Adjacent, noted, not in scope
- Profile gossip has no per-peer rate limit (docs/design/federation-activation.md:46, :63). If one is added, it must pace or exempt the catch-up (up to 1,000 copies).
- A copy carries a per-server name. Whoever first shows replicated copies must label them with their server.
- `count_signed_profiles` (signed_profiles.rs:165-171) has no caller.
- The 30 s limit notice starts with an emoji (msg_handlers.rs:338).
- The website's "Portfolio" link (`socials.website`) is always public, while the separate Website field can be kept for friends (profile.html:1219 vs :1150). Someone who puts the same link in both may think it is hidden.
- Catch-up sends at most the 1,000 most recently signed copies.

### Critical Files for Implementation
- C:\Humanity\src\relay\core\profile_copy.rs (new: the public copy, the words, limits, asks, dating)
- C:\Humanity\src\relay\handlers\profile.rs (new, moved from msg_handlers.rs:309-682: the save's asks, `handle_profile_sign`)
- C:\Humanity\src\relay\handlers\federation.rs (receive with limits, pass on, catch-up)
- C:\Humanity\src\relay\storage\signed_profiles.rs (keep rules, withdrawal, catch-up query)
- C:\Humanity\src\gui\pages\own_profile.rs and C:\Humanity\web\chat\chat-profile.js with new C:\Humanity\web\shared\profile-copy.js (each app signs when asked)

## 9. Decisions by the orchestrator (2026-10-12), each the plan's own recommendation unless said

1. **One copy per person across all linked servers, latest wins** (question 1): accepted as recommended.
2. **Erasing an account withdraws its copy on linked servers** (question 2): INCLUDED as Wave 2B. An erase that leaves the person's public profile on every linked server is the opposite of what they asked for, and the protect-users rule (memory `feedback_protect_users_safe_defaults`) says to close that now. The docs say plainly that an admin's erase cannot withdraw it, because only the person's own key can sign the withdrawal.
3. **Nothing shows a replicated copy yet** (question 3): shipped as scoped. Whether a server's chat should show a linked server's copy for someone with no profile there is left for the operator, with the plan's note that such a copy must be labelled with the server it came from.
