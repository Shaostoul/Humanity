# HumanityOS Features Directory

Complete inventory of every feature, where it lives, and what it does. Updated 2026-09-18 (v0.1319.x).

> The header said "v0.607.x" until 2026-09-20, which made this file look far more
> stale than it is - it does cover the in-world screens and `src/media`. Some
> individual PATHS in it are dead, though: `assets/shaders/pbr_simple.wgsl` (the
> shader is eleven numbered files under `assets/shaders/pbr/`), the Resources
> page (retired 2026-07-30 into `data/external/`), `web/activities/`, and a
> handful of deleted `src/gui/pages/` and `crates/humanity-systems/src/` files. Check a path
> exists before acting on it.

## How to Read This

Each feature lists:
- **What it does** (one line)
- **Web** (browser files, if any)
- **Native** (desktop Rust files, if any) -- all paths relative to repo root under `src/`
- **Server** (backend files, if any) -- relay code lives in `src/relay/`
- **Data** (config/data files, if any)

> **Architecture note (v0.90.0):** The `server/` and `native/` directories no longer exist.
> Everything is a single binary from `src/`. Server relay code is at `src/relay/`.
> Game/renderer/GUI code is at `src/renderer/`, `src/gui/`, `crates/humanity-systems/src/`, etc.
> Run `HumanityOS --headless` for server-only mode (VPS, Raspberry Pi).

---

## Civilization Trust Layer (v0.98.0 – v0.109.0)

### Post-Quantum Crypto Core
ML-DSA-65 (Dilithium3) + ML-KEM-768 (Kyber768) + Argon2id + BLAKE3.
- Native: `crates/humanity-protocol/src/pq_crypto.rs`, `crates/humanity-protocol/src/kdf.rs`, `crates/humanity-protocol/src/canonical.rs`, `crates/humanity-protocol/src/object.rs`
- Tests: 14 PQ + 7 KDF + 5 object roundtrip
- Cargo deps: `ml-dsa = 0.1.0-rc.8`, `ml-kem = 0.3.0-rc.2` (with `getrandom` feature), `argon2 = 0.6.0-rc.8`

### Signed-Object Substrate
Generic SQLite-backed table that every higher-level domain (VCs, governance, recovery, AI status, disputes) projects from. Auto-indexes derived domains on insert.
- Storage: `src/relay/storage/signed_objects.rs`
- API: `POST/GET/LIST/COUNT /api/v2/objects` in `src/relay/api_v2_objects.rs`
- Schema registry: `data/identity/schemas.ron`

### DID Resolver
`did:hum:<base58(BLAKE3(pubkey)[..16])>` format. Short enough for QR codes.
- Core: `crates/humanity-protocol/src/did.rs` (parse, format, fingerprint, hex conversion)
- Storage: `src/relay/storage/dids.rs` (resolve to current pubkey + activity metadata)
- API: `GET /api/v2/did/{did}` in `src/relay/api_v2_did.rs`

### Verifiable Credentials
W3C-style VCs over the signed-object substrate. 12 indexed schemas. Issuer-auth-checked revocation, subject-auth-checked withdrawal.
- Storage: `src/relay/storage/credentials.rs`
- API: `GET /api/v2/credentials`, `GET /api/v2/credentials/{vc_object_id}` in `src/relay/api_v2_credentials.rs`

### Multi-Layer Trust Score
0..1 normalized total + 6 transparent sub-scores. Sybil-farm-resistant via graph entropy.
- Storage: `src/relay/storage/trust_score.rs`
- API: `GET /api/v2/trust/{did}` in `src/relay/api_v2_trust.rs`
- Weights: `data/identity/trust_weights.ron`

### Governance
9 proposal types, 5 local-scope, 4 civilization-scope. Vote weight = trust score capped at 0.95.
- Storage: `src/relay/storage/governance.rs`
- API: `/api/v2/proposals`, `/api/v2/proposals/{id}`, `/api/v2/proposals/{id}/tally` in `src/relay/api_v2_governance.rs`
- Types: `data/governance/proposal_types.ron`

### Native Governance page: live feed + signed voting + proposal form (v0.660)
The native Governance page went from static instructional text to fully live: background
fetch joins GET /api/v2/proposals with each proposal object's CBOR payload (title/body) and
its weighted tally; proposals render as expandable rows (OPEN/CLOSED chip, scope, voting
window, tally bars for yes/no/abstain weights); voting and proposal creation build
`vote_v1`/`proposal_v1` signed objects with the SAME in-crate `ObjectBuilder` +
`DilithiumKeypair` the relay verifies with (zero cross-language canonicalization risk) and
POST them as `SignedObjectSubmission` JSON. Hardened by adversarial review pre-commit:
proposals from a previously-connected server are cleared before rendering (a vote must
never reference server A's proposal while POSTing to server B); the feed and vote buttons
gate on `server_connected`; an in-flight fetch aimed at an old server is replaced, not
awaited; a consecutive-failure circuit breaker bounds the per-proposal join against a
hung server; vote-success wording is honest about the server keeping the FIRST vote per
identity (INSERT OR IGNORE, votes are final); session vote-tracking resets on identity
switch. 7 regression tests, including a full round-trip through the real relay storage and
a wire-struct deserialization lock. Web voting became real on 2026-07-01 (d70cf340c:
Dilithium3-signed vote_v1 from the browser, locked byte for byte to the relay by `just vote-kat`),
and proposing from the web on 2026-09-06 (6966e3bf0).
- Native: `src/gui/pages/governance.rs` (fetch/build/post/draw + tests), `src/gui/mod.rs` (`governance_*` GuiState fields; `apply_pq_identity` clears per-identity vote tracking)
- Snapshot: `src/gui/ui_snapshots.rs::snapshot_governance` (state-injected feed render)

### Laws (location-aware rules and rights) (v0.496, chips v0.661)
Nested jurisdiction tree (Humanity -> Earth -> country -> state -> county -> locality); pick where you live and see the rules that apply, broadest first. Two kinds: HumanityOS base set (our framework, from the Humanity Accord) and real laws (plain-language summaries with a source, not legal advice). Condense, do not ingest. v0.661: the data file's own `categories` list (loaded since v0.496 but never surfaced) renders as clickable filter chips (Rights, Privacy, Work and money, ...), and the BASE/REAL kind badge is a real bordered chip instead of bare text.
- Native: `src/gui/pages/laws.rs` (`GuiPage::Laws`, reached from the Humanity hub "Laws" section; `kind_chip` + category chip row), loader `src/gui/laws.rs`
- Data: `data/laws/laws.json` (jurisdictions + rules + categories, hot-reloadable)
- Design: `docs/design/laws.md`. Web mirror is a follow-up.

### How-to (step-by-step procedures, with runs) (increment 1, 2026-10-23)
One procedure per data file: what it achieves and for whom, its hazard in words and an icon, what you need, the steps to tick off (timers that run while the app is closed, Yes/No checks that point at their problem or back at a step, values to write down with last time's beside them, choices that show and hide later steps, pages to open, commands to copy), how to tell it worked, what goes wrong and what to do, and its sources with the date each was read. Short and Full views (Short never drops a serious or lethal safety line: a checker gate and a second lock in the view rule); the Stop and call someone if box first; a person's dated review required before a lethal how-to can show as checked. Runs are the person's own, on their device, keyed by step id so an edited how-to keeps them. Six how-tos shipped (Change a furnace filter, Boil water to make it safe to drink, Test the seals on home-canned jars, Mix oral rehydration solution from a packet, Back up your recovery phrase, Restore a save snapshot), each sourced from primary sources and dated, and reviewed twice by AI on 2026-10-24 (the review bound to the words by its hash): the two HumanityOS ones verified, the four lethal ones sourced until a person has read them; 48 planned ones shown dimmed in the tree.
- Native: `src/gui/pages/how_to.rs` + `how_to/` (`GuiPage::HowTo`, page id `how_to`; nav button beside Library; Apps tile; rail, list and view in `widgets::panes`; words from `data/how_to/words.json` via `how_to/words.rs`)
- Model, checker, hash, loader, runs: `src/how_to/` (compiled into the relay too); runs in `%APPDATA%/HumanityOS/how_to/runs/`
- Checker twin and build: `web/shared/how-to-logic.js`, `scripts/build-how-to.js` (in `just lints` and `just validate-data`); every gate shown red by `tests/fixtures/how_to/broken.json` in both checkers
- Data: `data/how_to/categories.json` (the starting tree, 11 branches), `data/how_to/<top>/<id>.json`, `planned.json`, `words.json`, generated `index.json`; shape `schemas/how_to.toml`
- Web mirror: `web/pages/how-to.html` + `how-to-app.js` (IndexedDB runs, `/how-to#<id>/<step>` deep links); parity pairs in `tests/page_parity_lint.rs`
- Snapshot: `src/gui/ui_snapshots.rs::snapshot_how_to`. Design: `docs/design/how-to.md`

### AI-as-Citizen
Mandatory `subject_class_v1` declaration + `controlled_by_v1` operator binding. AI excluded from governance voting per Accord.
- Storage: `src/relay/storage/ai_status.rs`
- API: `GET /api/v2/ai-status/{did}` in `src/relay/api_v2_ai.rs`

### Social Key Recovery
Shamir-shared seed via guardians. Server stores opaque ciphertext only. HALF BUILT
(2026-10-20): the server has the lookups only; there is no screen to choose guardians or
split the phrase, and the people-facing words say so ("Recovery through trusted people",
"Not finished yet"). The two lookups work on both sides.
- Native: `src/gui/pages/settings/recovery.rs`, Settings > Account > "Recovery through
  trusted people", right after Security (2026-10-20; it was the Platform tab's Recovery page,
  whose two lookups set flags nothing read). An account is named by its `did:hum:` id
  (`account_id`, the server's own `did_for_pubkey`), your own when the field is empty.
- Web: `web/pages/recovery.html`, linked from the website's Settings > Security.
- Storage: `src/relay/storage/recovery.rs`
- Recovery through trusted people (built 2026-10-21, docs/design/recovery-through-trusted-people.md): choose 2 to 10 friends and how many must help; pieces of the recovery phrase sealed to each, posted as `recovery_share_v1` (the server never learns who they are); a new device asks with a one-time key and a 16-character code read aloud; trusted people check it is really them and send their piece sealed to that key; the account is restored from the rebuilt phrase. Desktop: Settings > Account (`src/gui/pages/settings/recovery.rs`, `recovery_net.rs`); website: /recovery (`web/pages/recovery-app.js`, `web/shared/recovery.js`). API in `src/relay/api_v2_recovery.rs`: `GET /api/v2/recovery/setup/{account}`, `POST .../withdraw`, `POST .../ask`, `GET .../ask/{code}`, `GET .../asks`, `POST .../answer`, `POST .../close`, `POST .../not-me`
- Key cards (built 2026-10-22, docs/design/key-cards.md): each person's Kyber768 key signed by their own Dilithium3 identity, sent with every sign-in (`identify.key_card`), kept one per person (`key_cards`), served with `key_card_request`/`key_cards` and `GET /api/key-card/{key}`, passed between linked servers as `key_card_gossip`. Every seal to a person on both apps uses only a key from a card the app checked and pinned; a newer card with another key is a change shown in the conversation with "Accept the new key", and nothing is sealed to them until it is accepted. Card: `crates/humanity-protocol/src/key_card.rs`, `web/shared/key-card.js`; relay: `src/relay/handlers/key_cards.rs`, `src/relay/storage/key_cards.rs`; desktop: `src/engine/key_cards.rs`, `src/net/key_pins.rs`, `src/gui/pages/chat/key_change.rs`; website: `web/chat/chat-key-cards.js` (pins in `chat-dm-store.js`)

### Federation v2
Generic signed-object gossip + per-issuer continuous trust + dispute objects + multi-hop with cycle-breaking via dedup.
- Handler: `src/relay/handlers/federation.rs` (`SignedObjectGossip`, `gossip_signed_object`)
- Storage: `src/relay/storage/issuer_trust.rs`
- Wire format: `RelayMessage::SignedObjectGossip` in `src/relay/relay.rs`

---

## Communication

### Chat (Text Messaging)
Real-time text chat with channels, threads, and message history.
- Web: `web/chat/app.js`, `web/chat/chat-messages.js`
- Server: `src/relay/relay.rs` (WebSocket routing), `src/relay/storage/messages.rs`
- **Who can read and who can post (2026-11-02, docs/design/chat-access.md):** each channel has a
  read level (Anyone, Members, Verified) and a post level (Members, Verified, Moderators and admins),
  enforced by the relay (`src/relay/chat_access.rs`, rules shared in
  `crates/humanity-protocol/src/chat_access.rs`, web twin `web/shared/chat-access.js`): history,
  pins and reactions of a channel not open to Anyone only through a member's signed POST, the
  socket's lines only to those who may read them, one link and no attachment below Verified,
  @everyone and @here for moderators and admins, only Anyone channels federated. A visitor reads an
  Anyone channel's posts since it opened, names only. Set in Server Settings > Channels (desktop)
  and the admin page's Channels panel (web, registry `channels.access`); the line above the box
  you type in and a badge beside each channel's name say who can read and post, in both apps; an
  unverified poster's links are drawn as plain words. The website's read-only view for a visitor
  without a name: `/chat/read` (`web/chat/read.html`).
- **The channels every server has (2026-11-02, docs/design/chat-channels.md):** #general renamed
  #global (`humanity_protocol::chat_access::MAIN_CHANNEL`; the live servers' history, reactions,
  pins, threads and search index moved by the relay's upgrade, `storage/channels.rs`
  `move_channel_on`), #feedback (anyone reads, people with a name post, federated), #announcements
  and #local, made from one data file (`data/server/default_channels.json`, compiled into the relay,
  `src/relay/default_channels.rs`). #local is an ordinary row under its server in both apps; a
  server's name folds its rooms; a room that is gone lands the app in #global. `just feedback`
  prints the newest #feedback lines from united-humanity.us (`scripts/feedback.js`).
- **One room row for every server (2026-11-03, BUG-242, docs/design/chat-channels.md section 6):**
  the desktop draws a room under the active server and under every other one with one drawer,
  `src/gui/pages/chat/channel_row.rs` `draw_room_row`: the voice column (mic or speaker, or empty),
  `# name` and its level badge, eye and node-graph, then the unread dot and the muted mark at fixed
  places on the right; a line of the body text tall. The website's rooms keep the same voice column
  (`web/chat/chat-ui.js` `.ch-voice-slot`).
- **Links to chat channels (2026-11-03, docs/design/chat-channels.md section 7):** `https://<server>/chat#<channel>`
  opens that channel (its id, else its shown name, any case; `web/shared/chat-links.js` and
  `src/gui/pages/chat/channel_link.rs`, one fixture). The website's address always shows the channel
  open (a DM, a group or the scratch pad never), an edited address switches channel, and a link in a
  message to this server's chat opens in place. A visitor's link keeps its channel to `/chat/read`,
  which opens it or says it is for members (or verified people) or not there (the relay's 401 or
  404). Copy link on every room's right-click and in the channel header, in both apps, copies the
  room's address on its own server. The desktop opens a link to its own or a saved server's
  channel in the app instead of the browser. With no link and nothing saved, every entry point
  lands on #global.
- **How much of a message the DM list shows (2026-11-03, desktop):** Names only, Name and one line
  (the default) or Name and two lines, in Settings > Privacy and the DMs header's cog; the levels are
  data (`data/gui/dm_preview.json`), the choice is `config.json` `dm_preview_lines`
  (`src/gui/pages/chat/dm_rows.rs`). The website's DM list shows names only.
- **Is my line posted? (2026-10-24):** my own room line reads "Sending...", then is posted, or
  "Not sent" with why (too long for my limit, too fast, a new account's wait) and Edit and send
  again; "Not confirmed by the server yet" after 20 s. The relay names each refusal to its sender
  (`chat_refused`, `src/relay/handlers/chat_gate.rs`, the checks a line meets). Web
  `web/chat/app.js` `markLineSending`; desktop `src/gui/pages/chat/line_limit.rs`.
- **Times in this computer's time or UTC (2026-10-24):** Settings > Timestamp format (five, from
  17:42 to milliseconds) and Time zone (this computer's time, the default, or UTC) in both apps,
  under the same config strings and words. The offset comes from the system for each time shown,
  daylight saving included (Windows `SystemTimeToTzSpecificLocalTime`, `localtime_r` elsewhere; no
  time library). In UTC a time says so; the expanded pill names its offset ("(UTC-07:00)"). Desktop
  `src/gui/pages/chat/timestamps.rs` (also the Files page's dates); web `web/chat/view/timestampPill.js`
  `formatStamp`, `web/pages/settings.html` Chat > Messages.
- **The profile bio's limit shows the same way (2026-10-24):** in both apps the bio is a soft limit:
  what is past the server's 280 characters shows in red on dark red with its count, and Save says to
  shorten it, where `char_limit` / `maxlength` (and the website's save, a second time) cut a
  longer paste without a word. The universal widget: `src/gui/widgets/over_limit.rs`, web
  `web/shared/over-limit.js` and `.css`.
- **Characters past the limit in red on dark red (2026-10-24):** both message boxes colour every
  character past my line limit (theme tokens `danger` on `danger_bg`), with a "312 / 280"
  counter near it, in rooms only (a DM, a group and the scratch pad are not measured). The limit
  is the relay's own word: `my_limits` on sign-in and whenever my role or its definition changes
  (`src/relay/handlers/broadcast.rs` `send_line_limit`), kept per server on the desktop. Web
  `web/chat/chat-ui.js` `showOverLimit`; desktop `line_limit.rs` `layouter`.

### Direct Messages (E2E Encrypted)
Private 1-on-1 conversations encrypted with pure Kyber768 / ML-KEM-768 →
BLAKE3-KDF → AES-256-GCM (dual-seal envelope; the relay stores opaque
ciphertext only). The old ECDH P-256 path was deleted (web v0.263.4, native
v0.264.0), see the canonical crypto table in `CLAUDE.md`.
- Web: `web/chat/chat-dms.js`, `web/chat/pq.js` (`pqDmSeal`/`pqDmOpen`)
- Native: `src/net/dm_pq.rs`
- Server: `src/relay/storage/dms.rs` (zero-knowledge, never decrypts)

### Voice Channels
Group voice chat rooms with join/leave sounds. Voice is per-channel: the voice
room IS the text channel, keyed by the channel's own string id (the relay
validates the channel's `voice_enabled` flag, not a separate voice_channels
table), so clicking a channel's mic joins THAT channel. Web and native join by
the same channel id and are interoperable.
- Web: `web/chat/chat-voice.js`, `web/chat/chat-voice-rooms.js`
- Server: `src/relay/handlers/msg_handlers.rs` (voice_room join/leave + roster), `src/relay/handlers/broadcast.rs` (voice_room_signal relay)

**P2P group voice** (native, v0.642.0): native's group-channel voice icon was
previously a `// TODO` no-op. Now wired to the same `voice_room` protocol,
using the group channel's synthetic `"group:<id>"` id as the room_id. The
relay's join handler special-cases this prefix and gates it on real GROUP
MEMBERSHIP (`Storage::is_group_member`) rather than the `channels` table's
`voice_enabled` flag (which has no row for a group room at all) -- verified
live against a local relay: a member joins silently, a non-member gets
"You are not a member of this group." **Known follow-up**: group voice rooms
don't yet appear in the `voice_channel_list` broadcast
(`build_voice_channel_list_msg`, `src/relay/handlers/broadcast.rs`, only
enumerates the `channels` table) -- join/leave and the underlying
`VoiceRoomSignal` audio signaling both work correctly (your own client's
`voice_joined` state and the existing-participant dial-in both work), but the
roster list won't show OTHER participants in a group voice room yet. Real
fix: extend that function to also report ad-hoc `state.voice_rooms` entries
with no backing `channels` row.

### Native Voice (Live Audio, Pure-Rust)
Full live voice on the desktop app: captures mic → DSP → Opus encode → sends to
each connected peer; receives peers' Opus → per-peer decode → mix → playback.
Native↔web is audible both ways. Pure-Rust stack (no C toolchain): cpal (WASAPI)
capture/playback, unsafe-libopus encode/decode, rtrb ring buffers, str0m WebRTC
media. Mic test loopback with a live level meter accepts any device sample
format (i16/u16/f32) and any rate (streaming linear resampler to/from 48 kHz).
Input stack: mic gain 0–200% (clip-protected); noise FILTER modes Off / Light
(85 Hz biquad high-pass + noise gate) / Noise suppression (RNNoise via the
pure-Rust nnnoiseless crate); TRANSMIT modes Open mic / Push-to-talk /
Voice-activated / Push-to-mute (bindable push key via raw winit input + VAD
threshold). Defaults: Noise suppression + Push-to-talk on CapsLock. WebRTC audio
is strictly opt-in (a connection only negotiates an audio m-line when asked), so
the P2P data mesh is unchanged. The voice-room JOIN registers with the relay
(`{type:voice_room, action:join, room_id}`) and signaling rides the web's
`voice_room_signal` protocol (newcomer-offers / incumbents-wait glare rule).
- Native: `src/net/voice.rs` (capture/DSP/encode/decode/mix/playback, `run_voice_session`), `src/net/webrtc.rs` (str0m bidirectional Opus media + voice signaling), `src/gui/pages/settings.rs` (`draw_audio_content` mic test + input controls), `src/lib.rs` (winit push-key input + signaling routing)
- Config: `src/config.rs` (`VoiceFilterMode` + `VoiceTransmitMode` enums, mic gain / push key / VAD threshold persisted to `AppConfig`)
- Server: `src/relay/handlers/msg_handlers.rs` (per-channel voice_room, `VoiceChannelData.id` is a String channel id), `src/relay/handlers/broadcast.rs` (voice_room_signal relay)

### Voice/Video Calls
1-on-1 WebRTC calls with camera support.
- Web: `web/chat/chat-voice-calls.js`, `web/chat/chat-voice-webrtc.js`
- **Checked against a real browser (2026-11-07):** `just verify-browser-webrtc` runs the website's own
  call and file scripts in a headless Chromium against the desktop app's own `WebrtcManager` and
  `FileLink`, through the relay's own forwarder on 127.0.0.1: a call each way (sound measured at both
  ends) and a file each way (the far end's bytes checked). Run before a release that touched WebRTC
  or anything under it. Test: `src/gui/crossing_tests/webrtc_browser.rs`; browser: `scripts/browser-webrtc.js`,
  `scripts/browser-webrtc/`; how-to: `docs/ai/builds.md`, "The check against a real browser".

### Screen Sharing / Streaming
Share your screen or stream to a channel. Server-wide master switch
(`video_streaming_enabled`, off by default) plus per-role `can_stream`
(mod/admin by default) both gate who can start one. Verified end-to-end
live against a local relay (2026-07-01 overnight-loop priority #2 sweep):
start/stop, viewer join/leave, and stream chat send + persistence all work
correctly. Found and fixed a real bug in the process (v0.645.0): the
persisted `viewer_peak` was fed the LIVE viewer count at leave/stop time,
which is only ever highest right at a join and decreases from there -- by
the time a stream actually ends (viewers have often already left), the
recorded peak was frequently 0 or far below the true maximum. `ActiveStream`
now tracks a `peak_viewers` high-water mark, updated on every join, used
instead of the live count when persisting. 4 regression tests
(`src/relay/handlers/msg_handlers.rs::stream_tests`), proven to actually
catch the bug via a revert-and-retest.
- Web: `web/chat/chat-voice-streaming.js`
- Server: `src/relay/storage/streams.rs`, `src/relay/handlers/msg_handlers.rs` (`handle_stream_start/stop/viewer_join/viewer_leave/chat`)

### Video Player Core (native, in-world screens rung 5, 2026-09-16)
A purpose-built player core, no VLC and no ffmpeg at runtime: opens a
WebM/Matroska file, or an MP4 holding AV1 (since 2026-11-03, BUG-228; any other format is
converted once by the machine's ffmpeg on ingest, 2026-09-18), decodes AV1 video to RGBA frames and Opus audio to PCM
on background threads, keeps a monotonic audio-led playback clock, and offers
play / pause / seek-to-start / poll-the-due-frame. Refuses any other codec by
name (H.264, H.265, AAC, VP8/VP9 and Vorbis are not decoded; other formats
will be transcoded on ingest). The display is the in-world screen surface
(next entry). Measured on the dev
machine in release mode, RGBA conversion included: 1080p AV1 decodes at
28 fps on one thread and 69 fps with rav1d's threads (the fixture, 320x180,
at 1154 and 1802 fps). Design, licence audit and numbers:
`docs/design/media-player.md`.
- Native: `src/media/mod.rs` (`VideoPlayer`, clock, bounded frame queue, decode thread, `attach_audio_with` + `AudioAttach` (looping, the initial mix), `set_audio_mix`, `audio_position_s`, `audio_state`), `src/media/video.rs` (`Av1Decoder` over rav1d, YUV to RGBA), `src/media/audio.rs` (`OpusTrack`, a kira streaming `Decoder`), `crates/humanity-core/src/audio/mod.rs` (`play_stream` at an absolute amplitude and pan, `master_volume`, `sfx_volume`)
- Data: `tests/fixtures/media/*.webm` and `sliding-bar-av1-*.mp4` (synthetic fixtures from `scripts/make-media-fixtures.sh`)
- Tests: `src/media/tests.rs` (demux, decode, pixels, samples, clock, seek, thread join, ignored fps benchmark; AV1 in MP4 with Opus and with AAC, plain words), `src/media/mp4_tests.rs` (the MP4 reader)

**AV1 in an MP4 plays through our own decoder (2026-11-03, BUG-228).** The form video sites hand
AV1 out in. `src/media/mp4.rs` reads the MP4 container through `mp4-atom` (MIT OR Apache-2.0),
every box decoded to a type, and builds each track's sample list itself, every count checked
against the file's length (a stranger's file cannot make it allocate without limit); fragmented
MP4 too. AV1 goes to rav1d, Opus to our decoder (its priming cut where the edit list says), AAC and
any other sound to the computer's own decoder (`system::probe_sound`, then `SystemAudio`), or the
picture plays silent and says so. A file nothing plays is named in plain words, its container and
codecs ("an MP4 file with H.265 video and AAC sound", `media::kind::FileKind`,
`MediaError::Unplayable`), never a reader's own message. The legal reading:
`docs/reference/findings/2026-11-03-mp4-container.md`.

### Video on in-world screens (native, in-world screens rung 5 integration, 2026-09-16)
"Movies on displays": a placed screen whose source is `video:<path>` plays a
WebM clip through the player above, on loop, with its sound placed in the
world at the screen, and a click on the screen toggles pause. The path is
resolved against the data dir then the repo root (the GLB model rule); the
player opens on the screen's first frame (a screen nobody looks at spawns no
decode thread); each framed tick takes the frame the player's own clock says
is due and writes it into the surface at the clip's native size, letterboxed
to the display's aspect over black bars, never stretched. The sound is the
clip's Opus track through kira as a looping stream, volume from distance
(the same 50 m linear law as one-shot spatial sounds) and stereo pan from
the screen's bearing, updated every frame for every video screen whether or
not it is on screen, under the master and sfx sliders. Paused shows a
one-line "Paused at X s of Y s" page and resumes on the kept last frame; a
missing file or an unsupported codec shows an error page naming the path
and the codec. Removing the screen stops and joins the decoder and stops the
sound. New seam on the provider trait: `ScreenWorld` + `world_update`, the
per-frame world context (screen centre, listener, audio manager); provider
`status()` fields now really do merge into `debug/screen_done.json`. Not in
this rung: synchronised playback between players (needs the relay clock), a
seek bar, subtitles, a file picker, true 3D spatial audio (no kira spatial
scene in the engine yet). Design: `docs/design/in-world-screens.md`, "Video
sources". Proven to fail (2026-09-16): two of the tests earned it on real
bugs during the build (the compose test caught the previous frame's pixels
showing through the bars when two layouts shared a canvas size, since the
bars were painted only on reallocation; the loop test caught the frame
taken just before a wrap being stamped with the next loop's number, since
the wrap check ran after the poll), and two on staged breaks: with
`on_button` toggling on release too the pause test failed at "a release
must not toggle back", and with the error page reduced to the bare word
"error" both error-page tests failed at "the page must name the path" and
"the page names the codec", each quoting the text read back from the drawn
shapes.
**A video disc in the drive (2026-09-18).** Press Open, pick the disc from the picker's quick row ("Disc (F:)", shown only while a disc is in), and press **Play disc**: the folder goes to `src/media/dvd.rs`, which finds `VIDEO_TS`, picks the main title (the biggest title set, `_0` menu excluded, parts in order) and converts the whole title as ONE film through ffmpeg's `concat:` protocol, cached like any other conversion. A COPY-PROTECTED commercial disc does not play and never will: HumanityOS does not break disc protection, so a disc whose packets are marked scrambled, or whose files cannot be read, gets one plain sentence saying so (and a disc conversion that fails later gets the same sentence), never a black screen or a bare decoder error. A `.iso` is refused with what to do instead. What plays: a disc you burned, a camcorder disc, an unprotected video disc, or the video files on a data disc.
- Native: `src/engine/screens/video.rs` (`VideoProvider`, `resolve_media_path`, `letterbox_layout`, `compose_letterbox`, `audio_placement`), `src/media/dvd.rs` (`find_video_ts`, `main_title`, `disc_read`, `begin_disc_ingest`, `disc_roots`), `src/media/transcode.rs` (`concat_input`, `cache_key_parts`), `src/gui/widgets/file_browser.rs` (`folder_offer`, disc drives in `quick_roots`), `src/engine/screens.rs` (`provider_for` arm, `world_update` loop in `frame_surfaces`), `src/gui/screen_surface.rs` (`ScreenWorld`, `ScreenProvider::world_update`, `ScreenSurface::world_update`), `src/engine/ipc.rs` (provider status merge)
- Web: none by design (the screens live inside the native 3D world).
- Data: `data/media/demo_colour_bar.webm` + `data/media/README.md` (provenance), `data/machines/home.ron` (`wall_screen_5` in the console room, `video:media/demo_colour_bar.webm`), `data/media/ingest.json` (the picker's extensions, now with the disc ones: vob, m2v, vro; never iso), `tests/fixtures/media/VIDEO_TS/` (a synthetic unencrypted disc, `scripts/make-media-fixtures.sh`)
- Tests: `src/engine/screens/video.rs` (scheme, path rule, pause toggle, error pages read back from the drawn text, frames + loop over the shipped clip, letterbox layout and compose, placement law; after the 2026-09-17 review: mix composition and epsilon gate, the sound link attaches once at the first mix, the stream starts at the placed mix not full volume, the attach waits for both the player and a device, a playing clip drops queued input, a clip paused on its last frame does not rewind, notices lay out at the def's px, the demo clip is byte-identical to the fixture; plus an ignored device test of the real attach), `src/gui/screen_surface.rs` (the event drop empties the queue and the next run sees none), `src/media/tests.rs` (the ignored looping-audio device test: the loop region wraps and the sound stays playing through two passes, with a one-shot control that stops)

### Reactions
Emoji reactions on messages.
- Web: `web/chat/chat-ui.js` (reaction picker)
- Server: `src/relay/storage/reactions.rs`

### Pins
Pin important messages to a channel.
- Server: `src/relay/storage/pins.rs`

### Message Search
Full-text search across channels.
- Server: `src/relay/api.rs` (`GET /api/search`)

### File Upload
Upload images and files to chat (10MB limit).
- Server: `src/relay/api.rs` (`POST /api/upload`), `src/relay/storage/uploads.rs`

### Shared-File Library (v0.675)
Public library of real, useful files (3D-printable parts, models, designs) shared by
people on the server. Uploads sent with `share=1` are publicly listed via
`GET /api/uploads` (search + limit params) and EXEMPT from the per-user media FIFO,
so a shared .blend never vanishes because its uploader later posted chat photos.
Chat auto-shares only 3D/model formats (`.blend .stl .obj .gltf .glb`) -- attaching
one signals intent to publish; photos and other chat media stay unlisted/private.
Original filename is preserved for display (stored name is timestamp-mangled).
Removal (v0.709): signed `POST /api/uploads/delete` — the uploader can remove their
own file; an admin can remove any (Dilithium-signed "delete_upload" request).
- Server: `src/relay/api.rs` (`GET /api/uploads`, `share` query param on upload, `POST /api/uploads/delete`), `src/relay/storage/uploads.rs` (`list_shared_uploads`, `delete_shared_upload`, FIFO exemption + tests), schema `user_uploads` (+`shared`, `original_name`, `size_bytes`)
- Web: `web/pages/shared-files.html` (browse/search/download), `web/chat/chat-messages.js` (auto-share on attach)
- Native (v0.710): the Files page's "Shared files" place since 2026-10-19 (the developer's Files page's top half until then): list (auto-load + Refresh), Upload via the in-app file browser (share=1), Remove (own files, or any as admin), Put on the Files page; a file chosen shows in the middle as the chat shows a public file. `src/gui/pages/files/shared_files.rs`. Web: the Shared files place, `web/pages/files-shared.js` (the Shared Files page until then). Since 2026-11-03 its green twin "Your shared files" is the same place with only the rows shared with your key (`shared_files.rs` `draw_list_mine`, `rows_of`; web `createSharedPlace({ mine })`, ids `sfm-*`).

### Files Page, formerly the Media Page (relay 2026-10-11, native page increment 2 and round 2 2026-10-11)
**One Files page (2026-10-19).** The operator: "What do you think about changing the name of media
to files", then "What's the real difference between these two? Why would we not want to merge
them? ... If we just start at the root HumanityOS folder for file browsing then we automatically
have 'data' files and we even have mods at the top too", and "I'd rather sync the names now while
no one is using HOS". So the Media page and the developer's Files page are one page, Files, with
PLACES at the top of its left pane (`src/gui/pages/files/places.rs`; web `files-places.js`). **In the
launcher's red, green and blue since 2026-11-03** (operator: "R for private, B for servers";
docs/design/files-showcase.md, last section): RED "Private, on this computer" (the HumanityOS folder,
Your folders; desktop only, the website says so in one line), GREEN "What you share" (Yours on
<server>: the list with the person filter set to you, `files.rs` `list_filters`, no shelf or
filters; Your shared files), BLUE "What everyone shares" (the rest, below), each group's heading and
picked row in the launcher's `danger`, `success`, `info` (`places.rs` `Group::color`); every place has
its hint by key (`files.places.*`); stacked panes draw them a line per group
(`widgets::section_nav_wrapped`); the page still opens on Shared on <server>. Shared
on <server> (the Media list, item and details below), Shared files (the server's public library),
Admin (admins), the HumanityOS folder (`files/app_folder.rs`: the app's own folder, the one holding
data/, assets/ and mods/, or for a copy installed to the person's folder its data folder, never their own files (`app_folder_for`, 2026-10-20), as a tree read folder by folder; a text file read and edited in the
middle; Save keeps `<name>.bak` beside it and Put back the previous version restores it; the web's
is The server's data folder instead, the server's data/ through `/api/data-files`, for the server's admins only, BUG-206, `files-folder.js`) and Your folders (desktop
only). The words say Files: the heading "Files on {host}", Add to Files, Put on the Files page,
Remove from Files, "On the Files page since", "File space (MB)". The names followed: `GuiPage::Files`
(page id `files`), `src/gui/pages/files.rs` + `files/`, `FilesState`, the relay's
`src/relay/files*.rs` and `storage/files*.rs` with its `/api/files/*` routes (an item's own files at
`/api/files/item-files/*`; the data-file routes moved to `/api/data-files`), `/files/<id>` item
pages, `web/pages/files.html` + `files-*.js`, `data/files/` (kinds, labels, licences), config.json
`files_layout`, `files_folders`, `files_unfinished_uploads`. Not renamed: the SQLite tables and
columns (`media_items` and the rest, kept on the live server) and the JSON fields both apps and the
relay exchange, and the playback engine `src/media/`. `tests/terminology_lint.rs` fails on the old
action words as quoted text. The entries below keep the names they were written with.

**Library guides open in the Library; How this page works (desktop, 2026-10-20).** In the HumanityOS
folder, a file in `data/library/` that the Library catalogs (`app_folder::guide_of`, the slug worked
out by the one `loaders::library_slug_of_file`) gets "Open in the Library": a button in the details,
a line with a link over its text in the middle, and a double-click on it in the tree (the tree
widget's leaves now report `double_clicked`); all three go through `app_folder::open_in_library`,
which opens the Library page with that guide in its reader (`library::show_doc`; the operator chose
on 2026-10-19 to keep the Library a separate reading room and connect the two). Its text can still be
edited there. And the page's explanations are folded behind one small "How this page works" toggle by
the heading (`files/how.rs`, `widgets/how_it_works.rs`, closed until opened, config.json
`files_how_open`): the page's own line and each place's (Shared files', the HumanityOS folder's, Your
folders'), word for word; outside it stay Shared files' line beside Upload a file (they are public)
and the HumanityOS folder's caution beside the file. Snapshots `files_how`, `files_app_folder_guide`.
The website has the same fold on its Apps and Files pages since 2026-10-20 (`web/shared/how-it-works.js`
+ `.css`; localStorage `hos_apps_how_open`, `hos_files_how_open`; the toggle's words read from the Rust
by `scripts/tests/apps-web.test.js` and `files-web-places.test.js`).

**Website mirror (v0.1522.0):** `web/pages/media-panes.js` and `media-sound.js` give the website
the same three panes (side by side or stacked by the same rule, the split in localStorage
`humanity_media_layout`), edit in place with the coloured parts and border and the same
leave-with-changes questions (plus the browser's own before-unload), a sound's waveform from
decodeAudioData with seek and an AnalyserNode spectrum (nothing of a sound fetched before Play
here), and the Gallery; chat's framed item window holds the item and its details. Tests:
`scripts/tests/media-web-panes.test.js` (in `just rig-tests`).

**Three panes, edit in place, sounds with a waveform (desktop, 2026-10-19; design
`docs/design/files-showcase.md` "The three panes"):** the Media tab is a list, the item to fit and its
details, side by side on a wide window and stacked top, middle, bottom on a tall or narrow one
(`src/gui/pages/media/panes.rs`, the widget `src/gui/widgets/panes.rs`: resizable, the split and the
view kept in config.json `media_layout`, a double-click on a handle puts two panes back). Left
(`media/list.rs`): the uploads, the featured shelf, the filters (they wrap in a narrow pane:
`filters.rs` `wrap_before`) and every item as a row, the one shown highlighted, a file's row drawn
with its type (a song's note). Middle (`media/viewer.rs`): a picture fitted (a press or View full size
opens the viewer), a video in the shared player, a model's cover with View in 3D (the viewer fills the
pane), a sound's waveform and spectrum, any other file a large card with Download. Right
(`media/item_view.rs` `draw_details`): everything about the item and its buttons, the day it was put
on the page. Edit no longer opens a window: the right pane becomes the form (`media/edit.rs`
`draw_in_place`) in its red, green and blue parts with the nav bar's moving border round the pane,
Save and Cancel always at its foot; Save returns to the details once answered; Cancel, another item,
closing the window or leaving the page with changes ask first (`item_view::Discard`,
`draw_leave_question`). The Gallery (the old full-width tiles with their buttons) is the other view;
a press on a tile shows that item in the panes. A card in chat opens the same two parts in one
window over the chat (`item_view::draw_window`). Sounds: `src/gui/widgets/sound_view.rs` (fetch,
play through the mixer as a stream, Play, Pause, Start again, Mute, the time) and
`src/gui/widgets/waveform.rs` (the peaks and a 24-band spectrum read once on a worker by symphonia,
the decoders kira carries, in a fixed amount of memory; a press on the waveform goes there; nothing
read or decoded while it draws). A sound is any item whose own file the file-type list calls one
(`data/safety/file_risks.json`). Tests: `media/panes_tests.rs`, the widgets' own, the updated
`item_tests.rs` (`edit_is_in_place_and_opens_every_time`), fixture
`tests/fixtures/media/two-tones-440-then-2000.wav`. Snapshots: `media_three_pane`,
`media_three_pane_tall`, `media_sound`, `media_edit_in_place`, `media_item_window` (`media` is the
Gallery). The website's Media page is unchanged (still one column with its item window).

**3D model viewer (v0.1508.0, docs/design/model-viewer.md):** View in 3D on an item's .glb or .stl
files, both apps. Desktop `src/gui/model_view/` (loader with bounds checks and limits, camera, an egui
paint callback drawing PBR into its own MSAA picture; pipelines registered in `lib.rs`), web
`web/shared/model-view.js` (WebGL 2). Not shown where the protected setup hides the person's pictures
(web `media-item.js` `hiddenBy`, which also made the website's Media page follow the setup at all).

**Round 5 (2026-10-12, v0.1491.0 to v0.1493.0; plan `docs/design/media-round-5-plan.md`):**
- Relay: an item edited in place, files and a cover added and removed (`src/relay/media_edit.rs`, signed, the edit over its JSON exactly as sent); members-only moves every file; removing an item removes them all; downloads and views counted once a person a UTC day (`src/relay/media_counts.rs`, X-Real-IP, a key for members-only); `/media/<id>` served by the relay with Open Graph tags (`src/relay/media_page.rs`, routed by nginx); a licence per item (`data/files/licences.json`, `src/relay/files_licences.rs`); a keep undone only by the same or a higher role.
- Native: "Media on {host}", Media and Admin tabs (the Media settings moved out of Server Settings, `media/admin.rs`), Sort, scroll paging, the shelf per category (`media/filters.rs`), the item's page (`media/item_view.rs`), Edit with the licence and its warnings, files and cover (`media/edit.rs`), counted downloads (`media/open.rs`), the hosting paragraph beside Add to Media; the server strip on the server pages (`src/gui/pages/server_strip.rs`, `GuiState::switch_to_server`).
- Web: "Media on {host}", the Admin tab, Sort, scroll paging, the shelf per category, `/media/<id>` addresses (`web/pages/media-app.js`); the server strip (`web/shared/server-strip.js`). The website's item page and editor (lane 2A) is next.
The pictures, videos, 3D models and files people on a server chose to show (design: `docs/design/files-showcase.md`). Nothing is on it unless its uploader put it there, nothing from a private conversation can be, and an item can be members only (only a file uploaded for Media; a file already public in a chat or Shared Files goes on as public only). Categories are the server's (admins set them, members suggest); content labels (Spoiler, Flashing lights, Graphic) hide an item until clicked; Picks (yours) and Featured (the admins'); its own storage budget, each person's share, and a meter with prune lists. Every write is Dilithium-signed with a purpose that names its target.
- Server: `src/relay/media.rs` (routes under `/api/media`, signed purposes, the members' file route `GET /api/media/file/{id}?token=`), `src/relay/storage/media.rs` (tables `media_items`, `media_categories`, `media_category_suggestions`, `media_picks`), `POST /api/upload?media=1`, server settings `media_visibility`, `media_budget_mb`, `media_per_person_mb`, `media_public_previews_only`
- Native: the Media page beside Watch, `src/gui/pages/media.rs` (state for one server and one identity, jobs off the frame, `poll_upload` every frame from src/lib.rs, the page, a 409 to an action on a tile reloads and says so), `media/api.rs` (requests, signing, answers; every action names the item's file, `filename`; the members' file fetched with the token in an `X-Upload-Token` header; `/api/media/mine` for the share; `featured=1`; picks by signed POST and `picked_by_me`; Keep on the server), `media/tiles.rs` (filters, tiles, what a tile shows: protected setup, label, click-to-load for non-friends, too large to show in place, the picture; Kept badge; Pick, Remove, Feature, Keep, Report with the `media` context), `media/add.rs` (Add to Media with the server's own share count and an over-share upload stopped before it starts, Put on Media, a waiting upload's publish, the category shown is the category sent, Suggest a category), `media/resumable.rs` (the upload in pieces: hash, start, pieces, Pause and Resume, resume after a dropped connection, the 429 allowance waited out, Retry after no room at the finish, `done` when the answer was lost; the unfinished upload kept in config.json `media_unfinished_upload`), `media/uploading.rs` (the page's side: progress bar, Cancel stops the upload and the publish, the person's waiting uploads with Put it on Media and Remove, Carry on after a restart), `media/open.rs` (public picture to the image viewer; card with Download for everything else, a picture inside; video in `crate::media::VideoPlayer` without sound for now; no whole-transfer timeout; Play here's temporary copy deleted on close, at exit and at the next start; Download named by the uploader's own file name), `media/kinds.rs`; Put on Media on your own Shared Files rows (`src/gui/pages/files.rs`, its list forgotten on a server switch); Server Settings > ADMIN > Media (`src/gui/pages/server_settings/media.rs`: unsaved category edits kept until Save or Discard, the changed-underneath note, the kept line on the meter, Keep in the prune lists); Server Settings > Roles' upload limits per role (`src/gui/pages/server_settings/role_limits.rs`: Media file, private file, private held, a day, an hour, sent with every role save); the Reports page shows a reported Media item with Open the item (`src/gui/pages/server_settings/reports.rs`); the chat's private-file limit from the server (`src/gui/pages/chat/attach_send.rs` `Limits`, `private_limit_mb`) and a private file's Save streamed and decrypted a megabyte at a time (`src/gui/pages/chat/attach_view.rs` `save_file`, `src/net/dm_file_stream.rs`); no upload token in any error words (`src/gui/widgets/image_cache.rs` `hide_token`)
- Data: `data/files/labels.json` (the content labels, read by the relay from data/files/ since 2026-10-20: until then it read data/media/ and served its built-in copy), `data/files/kinds.json` (the kinds' names and marks)
- Tests: `src/relay/media_tests.rs`, `src/relay/storage/media_tests.rs`, `src/gui/pages/media/api_tests.rs` (each signed purpose with its target and the item's file, accepted by the relay's own check and body types), `src/gui/pages/media/page_tests.rs` (the page's states), `src/gui/pages/media/resumable_tests.rs` (the upload in pieces against a simulated relay), `src/gui/pages/media/open_tests.rs` (the transfers on loopback), `src/gui/pages/server_settings/media_tests.rs` (the admin section), `src/gui/pages/server_settings/role_limits.rs`, `src/net/dm_file_stream.rs` (the streamed decrypt against the one-shot one)

### The Readable Web (native + web, 2026-09-16)
Open a real website INSIDE HumanityOS with no browser engine and no JavaScript: the
page is fetched over HTTPS on a background thread (10 s timeout, 4 MB cap on the page
itself, http(s) only, the scheme gate re-run on every redirect hop, no cookies; images
go through the chat image cache, 16 MB each, and are requested only as they scroll
into view), parsed with
html5ever into a readable document (headings, paragraphs, links, images, lists,
tables, code, quotes; scripts/styles/forms/nav/footer/aside dropped, main/article
kept), and drawn in egui with clickable links, back/forward, an editable URL row and
an "Open in browser" escape hatch. OFF by default (`AppConfig.readable_web`, Settings
> Privacy, independent of the privacy tier); off = Browser-page cards open the OS
browser as before. The websites database records per site whether its pages may
legally be shown in-app (`embed.status`, evidence fields enforced by the checker)
and any affiliate tag with its mandatory disclosure. Design: `docs/design/readable-web.md`.
- Native: `src/web_reader/` (mod.rs model + limits, dom.rs arena TreeSink, parse.rs readability + blocks, fetch.rs gate + capped fetch), `src/gui/widgets/web_view.rs` (the view), `src/gui/pages/browser.rs` (cards + view), `src/gui/pages/settings.rs` (Privacy toggle), `src/config.rs` (`readable_web`)
- Web: `web/pages/web.html` (mirror: same cards from the same file, opens in a new window)
- Data: `data/web/sites.json` (schema `schemas/web_sites.toml`), `data/web/readability.json` (schema `schemas/web_readability.toml`)
- Checks: `scripts/check-web-sites.js` (preflight), `tests/page_parity_lint.rs` (both clients read `web/sites.json`), fixtures `tests/fixtures/web/`
- Cargo deps: `html5ever = =0.40.1` (native-only), `url` (now native too)

### The Readable Web on a Wall + the screens rig (native, in-world screens rung 6 integration, 2026-09-16)
A `web:<url>` screen source puts the readable web view on an in-world display:
one `WebViewState` per screen (own history, own fetch, own status line),
navigated to its url ONCE on the first frame drawn while in-app web reading is
on, drawn every framed tick, links clickable by the look ray. While
`readable_web` is off the wall draws a notice naming the exact switch
(Settings > Privacy) and never touches the view, so nothing is fetched. The
dev IPC gained `find` (where is the drawn text "Home", answered from the
frame's shapes: exact, then prefix, then substring), `link` (click the Nth link
the provider drew, through the normal hover, press and release on three
frames), `wait_ready` (bounded by `WAIT_READY_LIMIT`), `uv` on every pointer
verb and the provider's status fields on every done file (a web screen's
`url`, `title`, `status` and the `links` it drew); the station camera request
parks facing a named screen (`{"station":"home","screen":"wall_screen_6"}`).
`just verify-screens` boots the release exe in its own rig, writes
`readable_web: true` before boot, enters the world, and PROVES with no human
that a click on the inventory wall toggles the Home header, a click on the web
wall's first same-host link navigates to a new page on our own site (no other
site is ever fetched), the tasks wall drew a page, and no panic occurred;
`--dry-verdict` and `--self-test` judge manifests without booting (the red
fixture is the verdict proven able to fail). Design:
`docs/design/in-world-screens.md` ("Web sources", "The screens rig") and
`docs/design/readable-web.md` ("On a wall").
- Native: `src/engine/screens/web.rs` (`WebProvider`), `src/engine/screens.rs` (`provider_for`, `link_uv`, `wait_ready_outcome`, `WAIT_READY_LIMIT`), `src/gui/screen_surface.rs` (`LoadState`, `FoundText`, `find_text_in_shapes`, `ScreenCore::find_text`), `src/engine/ipc.rs` (the verbs, `screen_done_base`, the screen camera pose), `src/gui/widgets/web_view.rs` (`show_sites_button`, `fetch_in_flight`, `history_len`)
- Web: none by design (the screens are surfaces inside the native 3D world).
- Data: `data/machines/home.ron` (`wall_screen_6`, `web:https://united-humanity.us`, console room east wall)
- Tooling: `scripts/verify-screens.js` (`just verify-screens`; `--dry-verdict`, `--self-test`, wired into `just preflight`), `scripts/lib/png.js` (dependency-free PNG decode/encode/diff/colour stats), `tests/fixtures/screens/{green,red}/`

### Apps Page (the Games Library and the Tools Page as one page, 2026-10-19)

A desktop page, "Apps", beside Play and Characters in the nav (`GuiPage::Apps`, page id "apps"; the Games
library of v0.1517.0 and the Tools page of v0.1063 made one, and `GuiPage::Games`/`GuiPage::Tools` are gone).
The operator, 2026-10-19: "What do you think about changing the name of ... games to apps. Then merge tools with
games/apps. Like an offline recommended list that also tracks installs, launches, and what not. Red section could
be games. Green could be humanityOS tools. Blue could be third party tools." and, the same day, "What do you think
extending the RGB style to CMY for beyond 3 options. In this case what if help is C and services is M and then we
still have Y." What may be done follows three dated findings, each opened from its Why in the Library ("Research
Findings"): `docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md`,
`2026-10-19-games-page-art-stats-and-links.md` and `2026-10-19-tools-page-install-launch-and-icons.md`.

- **Six parts, each under a band in its colour and its name** (never the colour alone, for colour-blind people):
  RED Games, GREEN HumanityOS tools, BLUE Other programs, CYAN Help, MAGENTA Services, YELLOW Added by you. Red,
  green and blue are the chat sidebar's section tints (`dm_bg`, `group_bg`, `server_bg`) with the nav's group
  colours for their edges; cyan, magenta and yellow are new theme tokens (`section_cyan_bg`/`section_cyan` and their
  magenta and yellow kin, editable in Settings > Appearance, mirrored in `web/shared/theme.css`;
  `widgets::tint_section::Tint` grew the three).
- **One toolbar** across the parts: search (every word, case aside), sort (Name; Recently played, Most played, Size
  for the games; Installed first for the programs; Category for the programs, Help and Services; only the sorts that
  order a part shown are offered), Grid or List, a chip per part with its count (and All), and "Only what is on this
  computer". Kept in config.json as `apps_view` (with each catalog part's category), `apps_added` and
  `apps_epic_covers`, guarded by `tests/settings_persistence_lint.rs`. Nothing migrated from the old fields.
- **Short tiles** (the operator: "the app tiles are too big. The titles are stated twice"): a name is drawn once, on
  the plain cover when there is no picture, else in one small line under the picture (tiles in a part with pictures
  keep that line's room so a row's buttons line up); one line of where it is from and one fact; a small main button
  with one more fact at its right (achievements, an installed version, a download size, a website). Tiles are 168
  px at the least, so more fit to a row (`widgets/shelf.rs`, `widgets/cover_tile.rs`).
- **RED, Games** (`apps/games.rs`; readers `src/games/`): Steam (registry `SteamPath`, `libraryfolders.vdf`, each
  `appmanifest_<appid>.acf`), the Epic Games Store (`%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item`; DLC
  and Unreal Engine left out) and GOG (registry `GOG.com\Games\<id>`, then `goggame-<id>.info`), read only (KEY_READ)
  on a worker when the page opens and on Refresh. Play, only on a click, through ONE launcher (`src/games/launch.rs`;
  a test build records instead): `steam://rungameid/<appid>`, Epic's documented `?action=launch` link, a GOG game's
  own primary program. "Open in Steam" and "Open in the Epic Games Store" instead of Install or Update (none for GOG).
  A pill per store found. Being installed, folder gone or program gone say so in Play's place. Pictures: Steam's own
  client's cached pictures read in place (never copied or sent), an Epic or GOG game's program icon, else a plain
  panel. **Epic covers, opt-in**: the launcher's own catalog (`Data\Catalog\catcache.bin`, base64 of JSON, read
  only, `src/games/epic_covers.rs`) gives the addresses of each installed Epic game's box art; only when the person
  turns on "Get covers for Epic games from Epic's servers (this tells Epic which of your games' pictures were looked
  up)" (off by default) are those pictures fetched, from `cdn1.epicgames.com` only, no redirect followed, on a
  worker, through the one recorded `fetch`, decoded into memory for the session and never written anywhere
  (`apps/covers.rs`). Details (`apps/game_detail.rs`): numbers (played, last played, size, last updated, version,
  folder), achievements (binary KeyValues, `binary_keyvalues.rs`), the person's screenshots (shown large), Workshop
  mods by number, links (Steam store page and community hub in Steam; on the web the Steam store, PCGamingWiki,
  SteamDB, only on `games::links::WEB_HOSTS`). ONE Steam account's records; GOG GALAXY's database never read. Said on
  the screen: the stores keep downloads and updates (a platform holder's rule), no store logos (a platform holder's
  rule), nothing leaves the computer except a web link opened or Epic's covers turned on, "HumanityOS is not
  affiliated with Valve, Epic Games or GOG."
- **GREEN, HumanityOS tools** (`apps/own_tools.rs`): the app's own pages a person uses (Maps, Tasks, Inventory,
  Crafting, Quests, Home, Library, Notes, Calendar, Calculator, Chat, Studio, Watch, Media, Market, Wallet,
  Browser), as DATA in `data/gui/humanityos_tools.json` (name, page id, one line; dev-only pages left out), loaded
  at startup; each tile shows its page's nav icon (`widgets::icons::nav_icon`) and Open goes to its page.
- **BLUE, Other programs** (`apps/programs.rs`; readers `src/software/`): the catalog's 40 free programs. Which are
  installed: Windows' installed-programs list (Uninstall keys, both hives, both views) and App Paths
  (`software/records.rs`), **the Start menu's shortcuts** for a program whose record names no program file
  (`software/shortcuts.rs`: the two Start-menu folders listed by name, a matching shortcut read with Windows' own
  IShellLinkW, loaded read-only, never saved or resolved; its program, arguments and folder; a shortcut naming no
  program is opened itself through `Launch::Shortcut`; DaVinci Resolve, `detect.shortcut` in the catalog), and
  Steam's records (Blender from Steam), and (2026-10-20) **this person's Microsoft Store apps** (`software/store.rs`:
  Windows' `PackageManager`, asked only to find the current user's packages of the families the catalog's
  `store_family` names, read only; windows crate features `ApplicationModel`, `ApplicationModel_Core`,
  `Management_Deployment`; a Store copy launches by its own app id, `shell:AppsFolder\<family>!<app>`, through
  `Launch::StoreApp`, says it came from the Microsoft Store, and has no Show in folder or Download page; 11 programs
  name their makers' own Store packages, researched 2026-10-20 from the Store's own manifest API). Matched by DATA
  (`detect.display_name`, `detect.exe`, `detect.shortcut`, `steam_appid`, `store_family`). Says "Installed" or "Not
  found on this computer", never "not installed". Launch through the one launcher; Show in folder; **Install on the
  tile** (2026-10-20: a program not found that winget has and that is not website only has Install as its tile's
  and its List row's main button, its Download and Website staying in its details; pressing it opens its details
  with the consent step showing, having only asked `winget show`; while another program's install is under way the
  tiles keep Download); Install through winget after a consent step in the details (the package, publisher,
  licence and agreements, Microsoft's two lines exactly, Windows' permission prompt, winget's usage data, what "I
  agree, install" means; `--accept-package-agreements` for that one package, `--interactive` for a program whose
  licence is not free software, never `--accept-source-agreements`; one at a time; return codes in plain words;
  then it looks again). Website only, saying whose rule (DaVinci Resolve, Aseprite, Ardour, Vectr, Boxy SVG). **A
  copy Steam installed** gets Open in Steam, its store page and community hub in Steam and its store page on the web
  (`games::links::steam_program_links`), beside its maker's website (no Download page: Steam keeps it up to date).
  Said on the screen: the safety line and not affiliated with the makers; where programs come from (with its Why)
  and how what is installed is found, no logos, are in "How this page works".
- **How this page works** (2026-10-20, `widgets/how_it_works.rs`, `apps/library.rs` `how_it_works`): one small toggle
  by the heading folds the parts' explanations, word for word with their Whys (the stores' terms, covers and logos,
  nothing done by itself, where the programs come from, how what is installed is found, Services' line); closed
  until opened, kept in config.json `apps_how_open`. Outside it stays only what must be seen before acting: the
  programs' safety line, Help's line (call the emergency number first), Epic's covers switch, and the two short
  not-affiliated lines.
- **CYAN, Help, and MAGENTA, Services**: the catalog's real-world help websites ("service"), split by each
  category's `section` in `data/external/catalog.json`: Help for urgent and protective needs (Emergency, Health,
  Mental Health, Legal), Services for everyday ones (Water, Energy, Food, Housing, Money and Benefits, Employment,
  Skills and Repair, Education, Technology). Each opens its own website on a click.
- **YELLOW, Added by you**: "Add a program" (the app's file picker, programs) adds any game or program by its own
  program, named by its file name; Launch starts it in its own folder; Rename; Remove takes it off the list only.
- Tests: `src/gui/pages/apps/tests.rs` (the six parts, a name once and short tiles, Epic covers only when turned on,
  a fetched cover kept small, HumanityOS tools opening their pages, How this page works folded), `games_tests.rs`,
  `game_detail_tests.rs`, `programs_tests.rs` (incl. Blender's Steam links, Resolve from its shortcut, Help and
  Services split by the data, Install on the tile running nothing but `winget show`, a Store copy launched by its
  app id), `src/software/store.rs` (only an app id handed to Explorer; the ignored `print_this_persons_store_apps`),
  `library.rs` (columns, sorts offered, every part its own colour), `own_tools.rs`, `src/games/epic_covers.rs`,
  `src/software/shortcuts.rs` and `matcher.rs` (real `.lnk` fixtures written with IShellLinkW). The ignored
  `games::this_computer` and `software::this_computer` print what this computer has (Resolve's launch path,
  Fortnite's cover addresses).
- Snapshots: `apps` (the Grid, every part, a game's details), `apps_list` (the List, each catalog part narrowed to
  one category, Krita's consent step), `apps_help` (the Help part, a service's details), `apps_how` (How this page
  works open).
- Files: `src/gui/pages/apps.rs` and `apps/` (`games.rs`, `own_tools.rs`, `programs.rs`, `library.rs` toolbar,
  parts, tiles and lists, `covers.rs`, `detail.rs`, `game_detail.rs`, `entry_detail.rs`); `src/games/` (`mod.rs`,
  `keyvalues.rs`, `binary_keyvalues.rs`, `steam.rs`, `steam_user.rs`, `achievements.rs`, `workshop.rs`, `art.rs`,
  `epic_covers.rs`, `links.rs`, `epic.rs`, `gog.rs`, `other.rs`, `launch.rs`, `registry.rs`, `fixtures.rs`);
  `src/software/` (`catalog.rs`, `records.rs`, `shortcuts.rs`, `store.rs`, `matcher.rs`, `install.rs`);
  `src/gui/widgets/shelf.rs`, `cover_tile.rs`, `tint_section.rs`, `how_it_works.rs`; nav item in
  `src/gui/pages/escape_menu.rs`, icon `widgets::icons::paint_apps`.
- Data: `data/external/catalog.json` (also read by the website's Apps page, which ignores the desktop-only fields),
  `data/gui/humanityos_tools.json` (also read by the website's Apps page); both pairs guarded by
  `tests/page_parity_lint.rs`.
- Web: `web/pages/apps.html` + `apps-app.js` (2026-10-20: the website's Tools page made its Apps page, every name
  with it: `/apps`, `hos_apps_view`, the nav's Apps beside Play with a four-squares icon in `web/shared/icons.js`).
  The same six parts in the same order under bands in their colours and names; RED Games and YELLOW Added by you are
  one line each saying the desktop app does them, with a link to /download; GREEN HumanityOS tools open the
  website's page for each (`TOOL_PAGES`; Quests and Studio say "In the desktop app"); BLUE, CYAN and MAGENTA show
  the catalog split by each category's `section`. One toolbar (search, Sort by Name or Category, Grid or List, a chip
  per part with its count), short tiles (the name once), the details panel. Tests:
  `scripts/tests/apps-web.test.js` (reads the app's parts, words, sorts and lines out of `apps.rs`, `apps/library.rs`,
  `apps/programs.rs` and `apps/games.rs`). Desktop only for the rest: a website cannot read or start what is
  installed on the computer it runs on.

### In-App File Browser (native, v0.708)
Universal file-picker widget (NOT the OS dialog — the all-in-one direction): quick
roots (Home/Downloads/Documents/Desktop/Game data/App folder), dirs-first ci-alpha
listing, extension filtering (incl. compound `.tar.gz`), max-size guard. Feeds chat
attach (v0.708) and the Files page shared-uploads manager (v0.710). Since v0.1503.0 every picker has
the person's own Favorites row (Add to favorites / Remove from favorites beside the path) and opens
in the folder the last file was picked from, both kept on this computer in `file_picker.json` beside
`config.json` (never synced).
Since v0.1504.0 a picker can take several files (`FilePickerState::multiple`, a tick per file,
Choose all here, Clear): Add to Media makes one item of them (the rest added once it is up,
`resumable::MoreFile`), Edit's Add a file adds each, chat attach and the Files page send them one
at a time through the one-file step (chat only to the conversation they were chosen in); the website
mirrors Add to Media, Edit and chat.
Since v0.1509.0 the picker opens over a dimmed screen (a click on it cancels, `BACKDROP_ALPHA`, its
window pinned above with `set_sublayer`) and its three parts are tinted like the chat sidebar, darker
(`SECTION_SHADE`): Places in the DMs' red, Favorites in the groups' green, This folder in the servers'
blue (`section`). Snapshot: `tests/snapshots/file_picker.png`.

### Files posted in chat, and what kind of file it is (v0.1509.0)
**Public files get a card (the "video attach" bug, BUG-203).** A file posted into a public channel is
sent as its address; both apps now show it as a card: its name (the server's time prefix taken off,
`%20` read as a space; the shared cases in `scripts/tests/fixtures/file-card-names.json`), its size
(a HEAD asked of the message's own server only, never another website before a click), Download, and
once saved, Show in folder. Sounds and videos keep their player on the website, and one the browser
cannot play says so instead of an empty player; on the desktop a video says it plays in the
computer's own player (the app's own plays WebM only: patents), and a .glb or .stl on the server gets
View in 3D in its own window. The kinds: every one a server takes that is not a picture drawn in
place (sounds, videos, documents, archives, HEIC/HEIF/AVIF, 3D models; `FILE_LINK_EXTS` and the
web's `FILE_LINK_EXTS`, kept in step). The relay now stores `<ms>_<words of the name>.<ext>`
(`upload_rules::safe_stem`), not the name with its dots and spaces squeezed out.
**What kind of file it is (from a suggestion in the project's Discord).** Not an antivirus: under a
file someone was sent, a line says when it is a program, a script, an archive, a document that can
carry macros, a shortcut, a web page or a Blender file, and to scan it or trust its sender first;
once saved, its first bytes are read too, and a file named like a picture, video, sound, PDF, text
file or 3D model whose bytes are a program, a script or an archive is called out as disguised (in
the danger colour). On public file cards, private files in DMs and P2P groups (the only check those
get: they are encrypted before upload), device-to-device files (by name at the Accept question,
then by bytes), and the Media item page's file rows. Cards offer Show in folder, never Open.
**Every file shows its type, always (v0.1510.0).** For people new to technology and people who read
little: a badge before every file's name, with a drawing, a word ("Video", "Archive", "Program") and
the file's ending, coloured by how much care it needs (green to look at, amber for care, red for
programs, scripts, shortcuts and anything disguised, grey when the type is not known). The drawings
are shapes in the list itself (`icons`), so both apps draw the same ones. On the chat's public,
private and device-to-device files, the Media item page and its Edit window, Add to Media, "Send a
file", the Files page and the file picker's list; from what is inside once a file is saved here.
Native `src/gui/widgets/file_badge.rs` (`show`, `paint_icon`, `level_color`) over
`file_risk::file_type` / `file_type_saved`; web `HosFileRisk.fileType` / `badgeHtml` / `fillBadges`.
**Drag and drop, and the upload window that posts in chat (v0.1511.0).** Files dragged from the
computer's file manager onto the window: while they hover, a dimmed overlay says what letting go
will do (with each file's badge); let go, they go where it said. By default the upload window opens:
Add to Media with every control. Over a channel of the chat it opens with "Also post in chat" ticked
for that channel (a tick on the left, then the channel, current first, then favourites, then the
rest, and Add to favorites, greyed out while unticked); when the item is published its title,
description and file are posted there through the chat's own send (a members-only item's page
instead of its file; not posted, and said so, when the chat is on another server or not
connected). Over a direct message, a group or the scratch pad: that conversation's private,
encrypted Attach, never a public upload. Over Shared Files: Shared Files. Over an item's Edit
window: that item. Over an open file picker: its choice. Folders are left out and named. The
Upload button wears the nav bar's animated border. Native `src/gui/drop_files.rs` (`Level`,
`register`, `refuse`, `take_for_picker`, `finish`), `media/add.rs` (`upload_dropped`,
`post_choices`, `draw_post_row`, `post_text`, `post_published`), `chat/public_files.rs`
(`post_in`), `widgets/tint_section.rs` (`rgb_border`); the upload record keeps `post_to`, so a
post survives a restart mid-upload. Web: `web/shared/drop-overlay.js`, the Media page, the item
editor, Shared Files (which gained its own Upload a file) and the chat.
**A Media item posted in chat is a card (v0.1512.0).** Add to Media's "Also post in chat" posts the
item's own address (`{server}/media/{id}`); both apps show a message holding one (of the server the
message came through) as a card, the address taken out of its words. It starts as one line (a
triangle to open it out, its type's badge, its title, "N files, SIZE", Open) and opens out to its
cover or picture (under the Media page's picture rules), who put it there, its description and every
file with its type and size, so people know what they are downloading. The title, the picture and
Open bring up the item's full window over the chat, with a Download per file. Native
`src/gui/pages/media/chat_card.rs` (`item_links`, `views`, `draw_all`, `apply`, `Job::Card`),
`media::draw_item_window`; web `web/chat/chat-media-card.js` (the Media page's item view framed over
the chat). Media sizes now read as the website's do: KB and bytes for small files, never "0 MB".
Snapshot: `tests/snapshots/chat_media_card.png`. With the protected setup on, a card's picture is
left out when the person who posted the link is not a friend, whoever put the item there. Only a
whole address is taken out of the words (a line without one keeps its spacing), and a stranger's DM
keeps its Media link held like any other link until the person presses Open.
**A file's name cannot hide what it is (v0.1512.0).** Both apps show and judge a file name with its
direction marks taken out (a right-to-left override made `photo<RLO>gnp.exe` read "photoexe.png",
and a mark after ".exe" hid the ending from the check), `src/net/file_risk.rs` `plain_name` and
`web/shared/file-risk.js` `plainName`, held to the shared `plain_names` in
`scripts/tests/fixtures/file-risk-cases.json`; the relay refuses a new name holding one. A saved
name past 80 letters is shortened before its ending, never through it, and Media shows each file's
real name (Download saves the safe one).
**MP4 plays in the desktop app, with sound, through Windows's own decoders (v0.1513.0).** An ordinary
MP4 (H.264 video, AAC sound; also whatever else Windows can decode, VP9 or MPEG-2 for example) plays
on the Media page, in an item's window over the chat and on in-world screens, with no ffmpeg and no
conversion. `src/media/system.rs` asks Windows Media Foundation (Source Reader, colour conversion to
RGB, float sound for kira); our own AV1/Opus path still plays WebM first. Media Foundation's five
functions are LOADED AT RUN TIME (`LoadLibraryW`), never linked, so the app still starts on a Windows
"N" edition, which then says "This copy of Windows has no video decoders. Microsoft's free Media
Feature Pack adds them." (H.265 without Microsoft's extension gets its own sentence; both are the
platform finding's words, and a screen still converts with the person's ffmpeg). Windows does not
apply an MP4's edit list as the file means it (the fixture's first picture comes stamped 0.133 s), so
each stream is counted from Windows's first time set to the start the MP4 declares for it (its edit
list's leading empty edits, `declared_starts`), with silence before a sound that starts later. The
decoder's real row layout is read after the first picture (an 854-wide H.264 picture drew skewed
before). A file whose sound Windows cannot decode (Opus in an MP4, DTS) is converted with sound on a
screen when ffmpeg is there, else plays silent, and the Media page says so (`sound_lost`). The Media
page's video now has sound (it played silent), with
Mute; the engine hands it the mixer each frame (`media::sync_sound`). A new player shows its first
picture at once, paused or not. Tests: `src/media/system_tests.rs` on
`tests/fixtures/media/moving-box-h264-aac.mp4`, two needing a sound device run by hand.
**Your folders (v0.1516.0).** A tab on the desktop Media page, beside Media (and Admin for admins):
"Add a folder" (the app's folder picker) points HumanityOS at folders on this computer, such as a
drive of pictures and videos; each is a row with its name, path and Remove (forgets it here, the
disk untouched), kept in config.json as `media_folders` and so across a restart; a folder not there
right now (a drive unplugged) says so and is not opened. The folder opened shows in the file
picker's own List, Details and Pictures views with the across slider, every entry with its type's
badge and pictures previewed; sub-folders open in place and Up never goes above the folder added.
A click ticks a file ("2 chosen", Clear; the second click of a double-click on a folder is ignored),
and Put on Media hands the ticked files to Add to Media exactly as a drop does (the first is the
item, the rest added to it) on the Media tab: nothing leaves the computer until Upload is pressed
there. A file this computer already put on a server's Media page is marked "On united-humanity.us
as Dusk" ("On Media" with no title), a link that opens the item, from a private record on this
computer only (`media_from_here.json` beside config.json, at most 5,000 rows, written when an
upload's publish or add finishes, from the file it was picked from: `Unfinished::origin`), and only
while the file's size and date are unchanged. Desktop only (operator, 2026-10-13: no website
version, so every browser behaves the same). Native `src/gui/pages/media/your_folders.rs` (the tab),
`src/gui/pages/media/from_here.rs` (the record), `src/gui/widgets/file_views.rs` (`Look::notes`,
`Act::Note`: links beside files in all three views); tests `src/gui/pages/media/your_folders_tests.rs`;
snapshot `media_your_folders`.
**Video previews in the file picker (v0.1515.0).** The Pictures view previews a video by one of its
frames (a tenth of the way in, at most 5 s, past a fade from black) where Windows decodes it
(`media::system::still_of`, `widgets/thumbnails.rs` `is_previewable`), on the same preview workers
as pictures.
**Play here for a video posted in chat (v0.1514.0).** A video card in a desktop chat room has Play
here (WebM always, an MP4 where Windows decodes it; never a file on another website before Download):
it plays in its own window with Pause, Start again, Mute, its sound and Close, its temporary copy
going with the window. The player is one widget for the Media page and the chat,
`src/gui/widgets/video_view.rs` (`VideoView`); the chat window is `chat/public_files.rs`
`draw_video_window`, its sound `sync_video_sound`. The "WebM only, because of patents" line now shows
only where the computer has no decoders to ask. The website plays a posted video in the page.
Since 2026-11-03 (BUG-228) Play here takes the in-world screens' road, `transcode::begin_ingest_as`
on the fetch's worker: played as it is, or converted once with the person's ffmpeg (the copy beside
the temporary one, deleted with it, "Converting it to play here: 40%"), or refused in plain words
naming the file by its posted name and what will play it. Offered for MP4 on every computer; the
patent line is gone.
**Videos play inside their chat message (2026-11-06).** The operator: "Can we make the videos show in
chat and playable in chat?" A video on the message's own server is a player in its card: the picture
(a black box with a drawn Play mark until first played, its last picture after), a seek bar, Play or
Pause, the time, Mute, the volume and Full window (the old window, kept as the bigger view of the
same player). Nothing is fetched or decoded until Play; one video plays at a time; a row off the
screen drops its player (decoder, sound, temporary copy; the video PLAYING goes into the dock
instead, rung 3, below) and keeps a small still and its place
(at most 24, none with reuse turned off); a file nothing here plays says so in the row in BUG-228's
words beside Download. `src/gui/pages/chat/inline_video.rs` (`ChatVideos`, `draw_in_card`,
`draw_video_window`, `end_frame`), drawn from `chat/public_files.rs` `draw_card`; the controls'
bar `widgets/video_view.rs` `track_bar`; a wall screen showing the chat keeps a set of its own
(`gui/screen_surface.rs` `ScreenTextures`, `ChatVideos::on_a_wall_screen`: no Full window there,
and its sound heard from the screen, `engine::screens::place_page_sound`, BUG-258); Full window's
body held to `widgets::dialog::body_room`, Close laid out first (BUG-258). Website: `<video controls playsinline preload="none">`
inline in the card, one playing at a time and a playing one scrolled out of view moved into the
dock (rung 3, below; `web/chat/app.js` `fileMediaOnePlaying`). Design: `docs/design/media-player.md`, "Videos inside
chat messages". Tests: `gui::pages::chat::inline_video::tests`, `private-files-web.test.js` ("plays
inline"). Snapshots: `tests/snapshots/chat_video_still.png`, `chat_video_playing.png`, `chat_video_window_short.png`.
**A posted video's poster picture (2026-11-06, rung 2).** The app that posts a video in a public room
makes one small JPEG of it (about a second in, at most 640 px on its longest side, aimed under 96 KB)
and sends it in the same upload as a `poster` part before the file: desktop
`src/gui/pages/chat/video_poster.rs` (`make_poster`, through the app's own player; called from
`attach_send.rs` `run`, sent by `upload_with_poster`, framed by `attach_body.rs`
`Multipart::with_poster`), website `web/shared/video-poster.js` (`make`, the browser's `<video>` and
a canvas; called from `web/chat/chat-messages.js` `uploadImage`). Never for a private conversation's
file. The relay (`src/relay/poster.rs` `check_poster`, `api.rs` `upload_file_in`) refuses a poster
that is not a JPEG, over 640 px a side or 128 KB, a second one, one with a non-video or an
`encrypted=1` file; cleans it with the picture strip; keeps it as `<stored name>.poster.jpg` beside
the video (`storage/files.rs` `poster_name`), its size in `user_uploads.poster_bytes` (counted in the
room, the day's allowance and the Media share); moves it with its file (`Folders::move_to`: a
members-only item's poster leaves the public address) and deletes it on every road the file leaves
by (`Folders::delete`, `files.rs` `delete_pruned`, Shared File removal) plus a deleted message's own
chat files (`handlers/federation_log.rs` `delete_own_message_in`, `storage/uploads.rs`
`forget_chat_files_of_deleted_message`); the expiry pass sweeps a lone one. Before Play the desktop row
fetches it once on a worker and draws it under the Play mark (`inline_video.rs`
`ChatVideos::want_poster`, at most 48 kept, only on-screen rows with reuse turned off); the website
card carries `data-file-poster` and sets `poster=` once the card is near the screen
(`app.js` `filePosterWatch`). A video from before keeps the black box. Design: media-player.md,
"Rung 2: the poster picture". Tests: `relay::poster::tests`, `gui::pages::chat::video_poster::tests`,
`inline_video::tests` ("poster"), `attach_send` tests, `video-poster-web.test.js`,
`private-files-web.test.js` ("poster:"); addresses shared in
`scripts/tests/fixtures/video-poster-addresses.json`. Snapshot: `tests/snapshots/chat_video_poster.png`.
**The chat videos' dock (2026-11-06, rung 3).** The operator: "When we scroll them off page can we add
like an anchored picture-in-picture mode that's stuck to the side of the chat it's at?" A PLAYING
video whose row leaves the view (a scroll, a channel or server switched, the chat page left) moves
into a small box, the same player, nothing fetched again: on the Chat page at the message list's top
right under the channel header (never over the header or the message box), on every other app page
at the right edge standing on the bottom strip, clear of the page's controls (egui's record of the
frame before) or as the slim tab; never in the first-person world (the video pauses, the dock
returns with the next page) and never on a wall screen. It says where the video came from ("From
#global on <server>"), with Minimize (a slim tab, "Video playing"; it plays on, its picture not made
an image), Close (paused, the row keeps its still), the picture, Play or Pause, the time, Mute and Go
to message (the Chat page at its channel, the row scrolled into view). Scrolling the row back returns
it inline; starting another video closes the dock; one chat video at a time across the app screen and
every wall screen (a thread-local claim, `ChatVideos::yield_if_elsewhere`, and
`engine::screens::chat_videos_one_at_a_time` every frame). Width 40% of the room, 240 to 420 points;
a picture under 72 points starts it as the tab. Desktop: `src/gui/pages/chat/video_dock.rs`
(`draw_video_dock`, `free_spot`, `page_controls`), `inline_video.rs` (`Dock`, `dock_moves`,
`close_dock`), `widgets/video_view.rs` `poll_with`; layer 30 (`gui/layers.rs`). Website:
`web/shared/video-dock.js` (the `<video>` element itself moved into the box, a line in its card;
Float it where the browser has its own picture-in-picture), wired in `web/chat/app.js`
(`fileDockTake`, `fileVideoDock`), styled in `web/chat/messages.css`. Design: media-player.md,
"Rung 3: the anchored dock". Tests: `gui::pages::chat::video_dock::tests`,
`video-dock-web.test.js`, `private-files-web.test.js` ("plays inline"). Snapshots:
`tests/snapshots/chat_video_dock.png`, `chat_video_dock_minimized.png`, `chat_video_dock_other_page.png`.
**Media card follow-ups (v0.1513.0).** The desktop card names a content label before the Media page has
loaded ("Labelled: Flashing lights."); the desktop takes only a link to a picture as a picture (a bare
"render.png" in a non-friend's message no longer withholds its card) and removes it word by word;
sizes round an exact half up on both apps, held to `scripts/tests/fixtures/size-words-cases.json`.
Website: the thread panel shows cards; an item opened from a card in its framed window signs through
the chat that opened it when the identity is passphrase-wrapped (`upload-window.js` `sign`, Media
purposes only); cards no longer on the page are let go.
**Rename a file, and choose a picture already on an item as its cover (v0.1512.0).** In an item's
Edit window every file has Rename (the item's own file too): a box holding the name without its
ending, the ending shown fixed after it, so a rename can never turn a model into something else.
Picture files on the item (png, jpg, gif, webp) have "Use as cover" for the uploader; the cover
there before becomes one of its files, nothing is deleted. Relay `POST /api/media/files/rename`
(purpose `media_file_rename:{id}:{file}:{BLAKE3 of the name}`; uploader or admin; the stored file's
ending is put back by `shown_name_for`) and `POST /api/media/files/cover` (`media_file_cover:{id}:{file}`,
uploader only) in `src/relay/media_edit.rs`, storage `rename_media_file` / `cover_from_file` in
`src/relay/storage/media_files.rs`; native `src/gui/pages/media/edit.rs` (`draw_files`), web
`web/pages/media-edit.js`.
**Remove any file of an item, its main file too (BUG-250, 2026-11-06).** In Edit the item's main
file ("name, main file, size") has Remove while the item has another file: the relay makes the next
file the item's own (its card, preview and page chosen from it), and a main file that came from a
chat or Shared files goes back there with its file kept; the only file has no Remove and says why
(remove the item instead, or add the new file first). Relay `POST /api/files/item-files/remove` with
file 0 (`src/relay/storage/files_items.rs` `detach_media_file`, `ONLY_FILE`); native
`src/gui/pages/files/edit.rs` `draw_files` and `item_view.rs` `remove_question`; web
`web/pages/files-edit.js`. Words `files.edit.*`.
**The file picker's views (v0.1511.0).** List (a badge per row, folders with a folder drawing),
Details (type, name, size, date changed; a click on a heading sorts, a second reverses; folders
first) and Pictures (1 to 8 across with a slider: a picture's own preview, read and shrunk on
worker threads, only what is on screen; any other file its type drawn large; one across shows one
at a time at full width). Remembered on this computer. `src/gui/widgets/file_views.rs`,
`widgets/thumbnails.rs`. Snapshot: `tests/snapshots/file_picker_pictures.png`.
The windows where people attach (the picker, Add to Media, Edit, "Send a file") are in tinted parts:
`src/gui/widgets/tint_section.rs` (`Tint::Red/Green/Blue`, the sidebar's `dm_bg`/`group_bg`/
`server_bg` at `SHADE`); web class `tint-section`.
- Data: `data/safety/file_risks.json` (kinds, endings, first bytes, the words; read from the data
  folder first), shared cases `scripts/tests/fixtures/file-risk-cases.json`.
- Native: `src/net/file_risk.rs` (`assess`, `assess_file`, `assess_saved`), `src/gui/widgets/file_caution.rs`
  (`show`, `show_in_folder`), `src/gui/pages/chat/public_files.rs` (`Card`, `draw_all`,
  `save_public`, `draw_model_window`), `attach_view.rs` (`caution`), `chat/transfer_rows.rs`,
  `media/item_view.rs` + `media/open.rs` (`saved_cautions`), `ImageCache::request_size`.
- Web: `web/shared/file-risk.js` (`HosFileRisk`), `web/chat/app.js` (`fileCardHtml`, `fillFileCards`,
  `fileCautionFill`, `fileMediaTrouble`), `chat-dms.js` (`privateFileCaution`), `chat-file-transfer.js`
  (`ftCaution`), `web/pages/media-item.js` (`riskOf`), `web/shared/protected.js` (a card hidden whole).
- Tests: `src/gui/pages/chat/public_files_tests.rs`, `src/net/file_risk.rs`, `upload_rules_tests.rs`
  (`the_stored_name_keeps_its_words_and_its_extension`), `scripts/tests/file-risk-web.test.js`,
  `private-files-web.test.js`, `file-transfer-chat-web.test.js`, `media-web-item.test.js`,
  `protected-web.test.js`. Snapshot: `tests/snapshots/chat_public_files.png`.
- Native: `src/gui/widgets/file_browser.rs` (`FilePickerState`, `file_picker_modal`), `src/gui/widgets/file_picker_memory.rs` (`PickerMemory`, `with`), chat attach wiring in `src/gui/pages/chat.rs` (`ATTACH_EXTS`) and `src/gui/pages/chat/attach_send.rs` (`upload_flagged`)

### Unread Indicators (native, v0.715–v0.719)
Sidebar-wide unread story, matching the web client's renderUnreadDots:
DM rows show a decrypted last-message preview line + unread dot (v0.715; own sends
show "You: …"; opening clears); group headers (v0.717) and channel rows (v0.718)
get the same dot, preserved across group_list/channel_list rebuilds; the Chat nav
tab paints a theme.danger() dot whenever ANY dm/group/channel is unread, visible
from every page (v0.719). P2P-group unread waits for native P2P push (closed P2P
groups only poll their list — no message event exists to flag).
NOTE: the WEB DM sidebar stays name-only visually (original call: operator
2026-05-27). Since the sealed-sender cutover (2026-08-23) BOTH clients build
their DM lists from the local decrypted history store, so the old
"web can't decrypt at list time" reason is gone; the web look is now just a
layout choice.
- Native: `src/lib.rs` (dm_new/dm_batch/chat WS handlers), `src/engine/dm.rs` (store glue), `src/gui/pages/chat.rs` (row painters), `src/gui/pages/escape_menu.rs` (nav dot)

### Privacy Tiers + Presence Privacy (both clients, 2026-08-23)
Every user chooses how visible to be, once, at first connect; the DEFAULT is
maximum privacy (Private, Balanced, Open; a fourth, "Spotlight", promised to show a
live stream to viewers and was taken out until that is built, BUG-238). Tiers are data (`data/gui/privacy_tiers.json`, their words keys in en.json), presets over two
real switches: server-enforced presence hiding (never online, no last_seen
WRITTEN, no join/leave/typing signals; new members join hidden until they
choose) and the public member-directory listing (profile privacy JSON). Both
individually overridable in Settings > Privacy in both apps (the website since 2026-11-03,
BUG-237: the level cards, both switches, and its own signed-in connection that tells this server
the presence choice at once; the chat tells each server it connects to). Settings > Privacy says (since
2026-11-03, BUG-235) what each switch hides and what none of them hides: the
name, key and role in a server's people list, the profile on that server, and
channel posts, in the same words in both apps (`privacy.*` keys). Web additionally nudges an
unwrapped seed into the passphrase-protection flow after the chooser. (Its old "relay my calls"
switch is gone: calls go through the server by default, chosen under Settings > Safety > Calls and
voice rooms, docs/user/privacy.md.)
- Native: `src/gui/pages/privacy.rs` (modal + Settings section), relay `privacy_update`
- Web: `web/shared/privacy-tiers.js` (the levels, what each sets, how a choice is kept), `web/chat/chat-privacy.js` (the first-connect chooser, the presence choice told on every connect), `web/pages/settings-privacy.js` (Settings > Privacy); tests `scripts/tests/privacy-web.test.js`
- Server: `src/relay/storage/members.rs` (hide_presence), roster masking in `handlers/broadcast.rs`
- Who sees a field kept for friends, in one key both apps show (BUG-239, 2026-11-03): `privacy.kept_for_friends` (the holder of a friendship pass from you, on that server: every mutual follow, and each person you send a contact request), in both apps' Settings > Privacy and under the website's Profile locks; `profile.network_seen_by` on both Network Profiles; glossary entry "friendship pass"

### Account Export + Erase (both clients, 2026-08-23)
Self-service data sovereignty: download everything the server stores about you
(JSON), or erase it all (typed-name confirmation; messages, uploads + files on
disk, profile, follows, mailbox, vault, listings, membership). Admins must hand
off the admin role first. Works regardless of server feature toggles.
- Native: Settings -> Account -> "Your data"; export lands in `%APPDATA%/HumanityOS/exports/`
- Web: identity block buttons (`web/chat/chat-privacy.js`)
- Server: `src/relay/storage/account.rs`
- After an erase (BUG-135, 2026-10-04): the server remembers for up to a set number of days
  that the account was erased (a one-way keyed fingerprint of the key, the day, and the
  window promised then; default 30 days, at most 100,000 entries, both in Server Settings,
  a longer window never stretching earlier entries), so the account's other devices are told
  and not signed up again by themselves; Connect (native) or Enter (web) under the erase
  note signs up again. One expiry pass culls it with the DM mailbox and message retention
  (start, every 6 h, after a settings change). `src/relay/storage/erased_accounts.rs`,
  `src/relay/storage/expiry.rs`, `src/relay/handlers/sign_ups.rs`

### Upload Metadata Stripping (server, 2026-08-23)
Every uploaded JPEG/PNG/WebP/GIF is stripped of EXIF/XMP/IPTC/text metadata before
the bytes touch disk, losslessly (whole-segment removal, no re-encode), so a
phone photo never publishes GPS coordinates or a camera serial.
- Server: `crates/humanity-protocol/src/strip_metadata.rs`, hooked in `api.rs::upload_file`
- **Could not be cleaned: refused, or sent on the person's signed choice (2026-11-06, BUG-260).**
  A picture whose structure does not walk is refused with 422, the upload routes' one "could not
  be cleaned" code (`upload_rules.rs` `CANNOT_CLEAN`, `PICTURE_UNREADABLE`; videos and HEIC/AVIF
  photos already were), never stored with what it carries (it used to be stored as it came). Both
  apps ask "Send it anyway, or don't send?", "Don't send" first (the chat's attach step for a file
  their own clean could not check, and a question on the server's 422; words `upload_keep.*`).
  `keep_metadata=1` is honoured only with the uploader's own Dilithium3 signature over the file's
  BLAKE3 (`KeepChoice`, the single request's `keep` part, the resumable start's `keep` field), so
  an upload token alone never keeps a file's details. Web: `web/shared/upload-clean.js`
  (`decide`, `keepChoice`, `keepAnswers`, `askSendAnyway`), `web/chat/chat-messages.js`
  (`uploadPrepared`); desktop: `src/gui/pages/chat/attach_step.rs` (`verdict`, `keep_answers`,
  `draw_server_ask`), `attach_send.rs` (`KeepSigner`).
- **GIF cleaned too (2026-11-07, BUG-263).** Comment blocks, every application block but looping
  (NETSCAPE2.0, ANIMEXTS1.0) and the colour profile (ICCRGBG1 012), extension blocks the standard
  does not define (XMP lives in an application block) and bytes after the trailer go; the frames,
  their timing and the plain-text blocks stay byte for byte. A GIF that does not walk is refused
  like any picture (the person may sign Send anyway). The rule every format follows is written at
  the top of `strip_metadata.rs`: what the picture is made of stays, words and data about it that
  no viewer is shown go. The same walk in the relay's strip (`walk_gif`), the desktop's own clean
  (`src/net/file_clean.rs`), the website's (`web/shared/strip-metadata.js` `walkGif`, held to the
  Rust byte for byte by `scripts/tests/strip-metadata.test.js`) and both describes (the attach
  step names a GIF's comment and the place in its XMP).
- **The Files page asks too (2026-11-07, BUG-264).** Add to Files, Shared Files and an item's Edit,
  in both apps, ask "Send it anyway, or don't send?" for a file the clean could not check, with the
  chat's own pieces: the words by key, "Don't send" first and holding the focus (desktop
  `widgets::Button::take_focus`), "Send anyway" sending it exactly as picked with the signed
  choice. Until then they refused it and pointed at the switch. Desktop: `files/add.rs`
  (`Form::anyway`), `files/shared_upload_step.rs`, `files/uploading.rs` (`draw_keep_ask`, after a
  resumable upload from an item's Edit stops with `Event::Unchecked`); web:
  `web/pages/files-add.js` (`startAttach` for an item's Edit), `web/pages/shared-files-upload.js`.
- **And when the server could not clean it (2026-11-07, BUG-275).** A file the app sent cleaned, or
  for the server to clean (the website's HEIC and AVIF photos), that the server still refuses at the
  finish with 422 (`CANNOT_CLEAN`; it stores nothing) gets the chat's question in its own window on
  Add to Files, an item's Edit and Shared Files, in both apps; "Send anyway" uploads the file exactly
  as picked (never the cleaned copy) with the signed choice, to the same place. Desktop:
  `files/resumable.rs` `Event::CannotClean`, `files/uploading.rs` (the question about the record's
  `origin`), `files/shared_files.rs` (`shared_sent`, `shared_ask`), and the one window every such
  question draws (`chat/attach_step.rs` `keep_ask_window`); web: `files-upload.js` (`cannotClean`),
  `files-add.js` (`askAfterServer`, `asPicked`), `shared-files-upload.js` (`send`'s `picked`). Under
  the protected setup the server's words stay, in the chat too since BUG-276.

### Encrypted Backups (server, 2026-08-23)
Both backup layers sealed at rest (in-process 6h AES-256-GCM `.db.enc`; VPS
30-min script openssl `.db.aes`); machine-local key `data/backup.key` lives
OUTSIDE the backups dir, so traveling backup copies are ciphertext. Crash
recovery decrypts transparently and scans both backup directories.
- Server: `src/relay/storage/backup_crypto.rs`, `scripts/humanity-backup-db.sh`, `scripts/decrypt-backup.sh`

### Server files kept current, and checked (servers, 2026-11-06, BUG-252)
Every file a server runs from outside its checkout (the backup script in /usr/local/bin, the
timers' units, the release follower) is listed with its role and mode, and installed from the
running release's own copy by rename, with a line naming each one replaced: by the deploy on every
push (united-humanity.us) and by the release follower after each update and every hour (any node
that follows releases). `just brief`'s SCRIPTS row compares each server's installed copies with
the repo's over a read-only ssh call. The deploy and the disk guard also clear the build leftovers
in target/, keeping the relay's program and the deploy's markers.
- Servers: `scripts/server-files.txt`, `scripts/install-server-files.sh`, `scripts/check-server-files.js` (`just server-files`), `scripts/relay-follow-releases.sh`, `scripts/clear-build-cache.sh`, `.github/workflows/deploy.yml`

### Saved Servers: add / switch / forget (native, v0.712–v0.714)
Add Server accepts a bare host (assumes https, v0.714); clicking one of a saved server's
rooms switches to it and lands there (reconnects with the same identity; a new connection
lands on #global; v0.712–713), and its name folds or opens its rooms (2026-11-02,
docs/design/chat-channels.md section 5: servers other than the current one start folded);
a small "x" forgets the bookmark. The active server shows "(current)".
- Native: `src/gui/pages/chat.rs` (saved-servers loop + `draw_add_server_modal`)

### System Health Panel (native, v0.720)
Server Settings → admin tab → "System health": read-only live snapshot of the
connected server via its public `/health` + `/api/stats` — status, deployed build
(git commit; a stale deploy is visible in-app), humanized uptime, messages stored,
connected peers. Worker-thread fetch + Refresh. In-app ops slice 1 native parity
(`docs/design/in-app-ops.md`).
- Native: `src/gui/pages/server_settings.rs` (`draw_system_health_admin`), `src/gui/mod.rs` (`SystemHealth`)

### Admin action registry, alert channels, federation panel, admin log (both, 2026-10-22)
Every action a server's admins and moderators can take, as data in
`data/admin/actions.json` (id, label, description, role, inputs, danger level, how
it runs), built into the relay and served at `GET /api/admin/actions`; one signed
route runs any action marked `"run": "here"`, `POST /api/admin/actions/run`
(Dilithium3 over `admin_action\n<id>\n<BLAKE3 of the inputs>`, fresh and used
once; role, confirm word for destructive ones, inputs checked against the entry).
24 run there: alert channels (list, add, change, on or off, remove, test send),
federation (list, add by address or key, trust, pause, resume, re-check key,
remove, connect, federate a channel), member moderation (kick, mute, unmute, ban,
unban, mod, unmod) and the admin log; every other admin chat command and route is
listed. Both apps draw Alert channels, Federation, "Every admin action" (from the
server's own list) and the Admin log from it. Pausing a peer keeps it and its
trust while federation stops. The admin log names the admin by a keyed fingerprint
and never holds a token, an address or the member an action was about. Coverage:
`tests/admin_action_coverage.rs` (every /api/admin route and chat command is
registered or a named member command) and the runner-table test. The design and
what is left: `docs/design/in-app-ops.md` ("The action registry").
- Server: `src/relay/admin/` (`mod.rs`, `alerts.rs`, `federation.rs`, `members.rs`, `tests.rs`), `src/relay/storage/admin_log.rs`, `src/relay/storage/federation_pause.rs`
- Native: `src/gui/pages/server_settings/` (`admin_api.rs`, `alerts.rs`, `federation.rs`, `all_actions.rs`), `src/gui/state_types/admin_console.rs`
- Web: `web/shared/admin-actions.js`, `web/pages/admin-console.js`, `web/pages/admin.html`; test `scripts/tests/admin-actions-web.test.js`

### Threads
Reply threads on messages.
- Server: `src/relay/storage/messages.rs` (thread_parent_id, reply_count)

---

## Identity and Security

### Dilithium3 / ML-DSA-65 Identity
Post-quantum cryptographic keypair (Dilithium3 / ML-DSA-65, FIPS 204) IS your identity, derived deterministically from the BIP39 24-word seed. No sign-up, no passwords stored on any server. (Ed25519 survives only as the seed scalar and Solana wallet address, see the canonical crypto table in `CLAUDE.md`.)
- Web: `web/chat/crypto.js` (key derivation), `web/shared/pq-identity.js` (Dilithium signing)
- Server: `src/relay/relay.rs` (signature verification), `crates/humanity-protocol/src/pq_crypto.rs`

### BIP39 Recovery Phrase
Since v0.1506.0 the 24 words, and the device-link code that carries the same key, start covered
wherever they appear (first run, Settings, the chat, the settings page, the device-link window) and
show only on a 3-second hold of "Hold to show the words" / "Hold to show the code" (v0.1507.0, the
app's hold-to-confirm button; a click does nothing), under a line about streaming and onlookers; they
cover themselves again on switching windows or tabs and each time their screen opens. Covered words
are never drawn. Native `src/gui/widgets/phrase_cover.rs`, web `web/shared/phrase-cover.js`.
24-word backup phrase for identity recovery.
- Web: `web/chat/crypto.js` (mnemonic generation/restoration)

### Key Rotation
Rotate keypair with dual-signed certificate (old + new keys).
- Web: `web/chat/crypto.js`
- Server: `src/relay/storage/key_rotation.rs`

### Signed Profiles
Profiles are cryptographically signed objects. Any server can cache and serve them.
- Server: `src/relay/storage/signed_profiles.rs`

### Vault Sync
Encrypted cloud backup of settings/keys (AES-256-GCM + PBKDF2-SHA-256 at 600,000 iterations, both web and native).
- Web: `web/chat/crypto.js` (encryption), `web/chat/chat-profile.js` (sync UI)
- Server: `src/relay/storage/vault_sync.rs`

### Rate Limiting
Fibonacci backoff per public key to prevent spam.
- Server: `src/relay/relay.rs`, `src/relay/handlers/chat_rate.rs`
- Per-address limits on every HTTP route (2026-10-22): one edge layer decides the client's address (headers believed only from a trusted proxy, `TRUSTED_PROXIES`, loopback by default) and takes each request from that address's allowance for the route's class (bot, costly, write, upload, read, socket, static); 429 with `Retry-After`, Fibonacci holds for an address that keeps asking; a bounded table. `src/relay/client_addr.rs`, `src/relay/ip_limit.rs`; summary in the admin stats (`ip_limits`)

---

## Push Notifications

### Push Subscribe/Unsubscribe
Web Push API with VAPID keys.
- Web: `web/shared/shell.js` (registration), `web/chat/chat-ui.js` (subscribes with the server's key, signed over the endpoint and keys; a refusal shown in plain words, `notifications.push_refused_*`), `web/pages/settings-app.js` (`unsubscribeFromPush`, signed), `web/shared/sw.js` (shows the notice)
- Server: `src/relay/storage/push.rs`, `src/relay/api.rs` (`/api/vapid-public-key`), `src/relay/push_routes.rs` (subscribe and unsubscribe, signed over the endpoint, unsubscribe by the owner only), `src/relay/outbound.rs` (where a push may go: https to a public name, checked at subscribe and at every send through the guarded client's own resolver, BUG-255; the same guard serves link previews), `src/relay/push_send.rs` (the send), `src/relay/web_push.rs` (our own RFC 8291 encryption and RFC 8292 VAPID token on p256, since 2026-11-06; the key is `data/vapid_private.key`)

### Notification Preferences
Per-user DM/mention/task switches, quiet hours and (since 2026-10-31) the two quiet-hours
exceptions, synced to the server (each server the person is signed in to), and honoured by the
server's Web Push since 2026-10-30 (`Storage::wants_push`).
- Web: `web/pages/settings-app.js` (the switches + quiet hours, written "22:00"); the chat's DM cog
  flips the synced DM switch (`web/chat/chat-ui.js` `openDmSettings`)
- Native: Settings > Notifications, `src/gui/pages/settings/notifications.rs` (the whole set, every
  word by key; changes sent to every connected server by `send_synced`); the chat's DM cog
- Server: `src/relay/storage/notification_prefs.rs`

### Desktop and browser notifications (2026-10-30, rung 1 of `docs/design/notifications.md`)
A notice from the operating system (Windows toast, Linux freedesktop, macOS) and a sound, for a
DM or an `@mention`, by ONE rule with a web twin held to shared cases: never for my own messages,
a switch turned off, a muted server or room, everything muted, quiet hours, or the conversation
already on screen; 5-second burst guard. A notice says who it is from, never what was said,
unless the person chooses "Who it is from and the message".
- The rule: `src/notifications/decide.rs` / `web/shared/notify-rules.js`; cases
  `scripts/tests/fixtures/notify-cases.json`; kinds, mute lengths, sounds as data
  `data/notifications/notifications.json`
- The OS notice: `src/notifications/toast.rs` (WinRT `ToastNotificationManager` through the
  `windows` crate, `notify-send`, `osascript`; one worker thread)
- Where messages reach it: `src/engine/notify.rs` (from `engine/mailbox.rs`,
  `engine/bg_connections.rs`, `engine/frame_ws_poll.rs`)
- This device's choices (`config.json` `notifications`): `src/notifications/settings.rs`
- Settings > Notifications: desktop notices, what a notice shows, sound + which + Play, Show a
  test notification, the synced switches, quiet hours, Mute everything, the muted list
- Web: `web/chat/chat-notify.js` (the chat's use of the rule); Settings > Notifications "A notice
  shows" (`notif-show`)

### Mute server, Mute room, Mute conversation, Mute everything (2026-10-30; rung 2 part 1 2026-10-31)
1 hour, 8 hours or until unmuted (lengths in `data/notifications/notifications.json`). Kept on the
device; a server's mute never silences a DM; a conversation's mute (`dm:<key>`, on every server)
stops that person's DMs and contact requests, never a mention of you in a room.
- Native: the active server's cog menu, a saved server's right-click, a room's right-click (a
  parked server's rooms too), a DM's right-click (`src/gui/pages/chat/mute_menu.rs`); the
  bell-with-a-slash mark on a muted server's row and a muted DM's row; Settings > Notifications >
  Muted (Mute everything, every server, room and conversation with Unmute, and the rooms set to
  every line or nothing with Back to mentions)
- Web: the bell on the server's row, and on each room's and DM's row (or a right-click on the row),
  in the chat sidebar (`web/chat/chat-notify.js`); Settings > Notifications > Muted with Mute
  everything and Unmute (`web/pages/settings-app.js` `renderMutedList`)
- Planned (rung 2 part 2): the tray icon's Mute everything menu

### Room levels: every line, mentions and replies, nothing (rung 2 part 1, 2026-10-31)
How much a room notifies, per room on this device, mentions and replies by default; "every line"
is the `room_line` kind (`data/notifications/notifications.json` `level`, `levels`).
- Native: a room's right-click (`mute_menu::level_items`); `config.json`
  `notifications.room_levels`
- Web: a room's bell or right-click; `localStorage` `hos_notify_levels`

### The in-app list: the bell and its panel (rung 2 part 1, 2026-10-31)
The last 100 notices the rule let through (also with the window in front, and with desktop notices
and sound off), the count of unseen ones on a bell, each opening its conversation; a burst folds
into one entry; seen when the panel opens; memory only, never written to disk; the message kept
only when "What a notice shows" shows it.
- Native: the bell beside the help button in every page's header (`src/gui/notice_panel.rs`,
  `src/gui/widgets/notice_bell.rs`, `icons::paint_bell`); the list `src/notifications/list.rs`
- Web: the bell in the chat page's header (`web/chat/chat-notify.js`; `.notice-bell`,
  `.notice-count`, `.notice-panel` in `web/chat/layout.css`)

### More kinds: replies, contact requests, Commons lines (rung 2 part 1, 2026-10-31)
A reply to a line of yours (`reply_to.from`), a contact request newly listed under Requests by a
live DM (never a mailbox page), and a shared room's line from a linked server (`federated_chat`,
live only, its author's `from_key` checked for blocks and for my own), through the same rule.
- Native: `src/engine/notify.rs` (`room_line`, `federated_line`, `contact_request`), raised from
  `engine/frame_ws_poll.rs`, `engine/bg_connections.rs`, `engine/mailbox.rs`
- Web: `web/chat/chat-ui.js` `notifyNewMessage` and its `handleMessage` wrapper;
  `web/chat/chat-privacy.js` (`live` through `ingestContactRequest` / `reachScreenDm`)

### Quiet-hours exceptions (rung 2 part 1, 2026-10-31)
A friend's DM (you follow each other) and mentions or replies may come through quiet hours, both
off by default, synced on the person's server (`notification_prefs.quiet_friends`,
`quiet_mentions`). The server can never apply the friends exception to a DM push (sealed sender):
`NotifPrefs::push_comes_through_quiet_hours`.
- Native: Settings > Notifications, under quiet hours (`src/gui/pages/settings/notifications.rs`)
- Web: Settings > Notifications, under quiet hours (replaced the unread Do Not Disturb section and
  the "Notification Sound" switch, both removed)
- Server: `src/relay/storage/notification_prefs.rs`, the guarded ALTERs in `storage/mod.rs`

### The tray icon and closing into it (rung 2 part 2, 2026-10-31)
The HumanityOS icon in the notification area (Settings > Notifications, "Show HumanityOS in the
notification area", on by default): a left click opens the window; its right-click menu has Open
HumanityOS, Mute everything (the data file's lengths, Settings' own Mute everything; while muted,
the "Muted until 14:00" line and Unmute) and Quit HumanityOS. Its picture gets a dot for notices
not opened yet and a bell with a slash while muted; its tooltip says both. "Closing the window keeps
HumanityOS running in the tray" (off by default) hides the window instead of quitting: the world
left and saved first, no frame drawn while hidden, the sockets read four times a second, a notice
the first time in a run. A close from another program (taskkill) always quits. No icon for an
instance a script started (`HUMANITY_TRAY=1` asks for one).
- Native: `src/engine/tray.rs` (the icon, its menu and clicks; `tray-icon` 0.26), `src/engine/tray_window.rs`
  (hide, show, the hidden tick, the outside-close hook), `src/notifications/tray.rs` (the look,
  tooltip, menu and picture, pure), `src/engine/frame_net.rs` (the network pump the hidden tick
  shares with the frame); Settings in `src/gui/pages/settings/notifications.rs`
- Dev: `debug/tray_request.json` / `tray_done.json` (close, a menu line, the icon, a notice's click,
  a made-up notice)

### A click on a notice opens its conversation; the in-world banner and the taskbar flash (rung 2 part 2, 2026-10-31)
A click on the operating system's notice opens its conversation (and the window) through the
in-app list's own path. Each notice carries a token naming nothing (this run's id and a number,
`src/notifications/clicks.rs`). In the world with the window in front, a short banner under the
compass shows the notice's own words; with the window behind, the taskbar button flashes.
- Native: Windows through the notice activator, a COM class the running app serves
  (`src/notifications/toast.rs` `win_activate`, `CustomActivator` in the per-user AppUserModelId
  key); Linux through `notify-send --action`; macOS not yet (rung 5). The banner
  `src/gui/widgets/notice_banner.rs` (drawn in `pages/hud.rs`); the cues `clicks::cues`; both from
  `src/engine/notify.rs` `act_on`
- Web: the browser notice's `onclick` focuses the tab and opens the DM or room
  (`web/chat/chat-ui.js`, `chat-notify.js` `openEntry`)

### Task, call and game-event notices; a sound per kind (rung 3, 2026-11-01)
A task given to you or one of yours changed (the relay's `task_notice` to the assignee's keys,
never the person who changed it; a Web Push behind the `tasks` switch, which until then did
nothing), a call ringing (its sound repeating while it rings, withdrawn when the ring ends however
it ends; no banner, the ringing screen is the call's own), and game events per event from data
(crops ripe, water low, short of air, hungry, thirsty), folded so a farm of 2,000 crops ripening
at once is one notice. Each kind's sound, or None, in Settings > Notifications > Sounds; each game
event on or off in "Your home".
- Relay: `src/relay/handlers/task_notice.rs` (`change`, `recipients`, `after_change`, from every
  task path: raw and enum handlers, the bot API), `RelayMessage::TaskNotice`, `PushKind::Task`
  (`storage/notification_prefs.rs`), the Web Push send in `src/relay/push_send.rs`
- Native: `src/engine/notify.rs` (`task`, `call`, `game`, `pump_ringing`, `pump_game_events`),
  `src/notifications/game.rs` (the fold), `src/notifications/toast.rs` (`withdraw`, Windows'
  `ToastNotificationHistory`; Linux a call's notice lasts its ring), `systems::push_game_event`
  (raised by `systems/farming/mod.rs`, `systems/plumbing.rs`, `systems/food.rs`), a click on a
  task notice opening it on the Tasks page (`GuiState::task_open_id`)
- Web: `web/chat/chat-notify.js` (`onTaskNotice`, `ring` / `ringEnded`, `playSound` from
  `/assets/audio/ui/`), `web/chat/chat-voice-calls.js` (the ring through the rule, its old
  service-worker notice removed), Settings > Notifications > Sounds (`web/pages/settings-app.js`
  `renderSoundRows`, `hos_notify_sounds`), `/tasks?task=<id>` (`web/pages/tasks-app.js`)

### The always-allow list for quiet hours (rung 3, 2026-11-01)
The operator, 2026-11-01: "I like the default being calls don't go through on quiet hours unless I
specifically add users to the always allow list." People added from a conversation's menu ("Always
allow during quiet hours"), nobody by default, whose calls and messages come through quiet hours;
said in the quiet-hours control itself; listed in Settings with Remove. Never on a server: it
travels between the person's devices as notes to oneself (`[[hum:quiet-allow:v1]]<key>/on|off`),
the newest note per person winning.
- Native: `src/notifications/allow.rs` (the list and the note, pure), `src/engine/quiet_allow.rs`
  (send, flush, screen), the DM right-click (`gui/pages/chat/mute_menu.rs`), Settings
  (`gui/pages/settings/notifications.rs` `draw_allow_list`)
- Web: `web/chat/chat-notify.js` (`allowed`, `setAllowed`, `screenAllowNote`, the DM bell's
  menu), `web/shared/notify-rules.js` (`allowNoteText`, `allowNoteParse`, `allowApply`), Settings
  (`web/pages/settings-app.js` `renderAllowList`)

### Per-person controls: "Your settings for <name>" on a profile card (rung 1, 2026-11-03)
The operator, 2026-11-03: "We need to add the various controls users can use on other users to the
profile page. Like we need to set file sharing limits, allowed to call during quiet hours, etc."
Design and the whole list (with its later rungs): docs/design/per-person-controls.md. On another
person's card, below their actions and above Report and Block, apart from Moderation and Admin:
Notifications from them (Everything / Only calls, mentions and replies / Nothing; THE rule's
`from_level`, both twins, the shared cases), Calls and messages come through quiet hours (the
always-allow list), Files from them (Use my setting / Always ask / Up to a size without asking /
Never; the file's plan and a big file's question), a friend's Message, Call and Trade ticks (the
"People I choose" choice), and their conversation's mute with Unmute. Each an override of a global
setting, safe by default, enforced on the person's own device, kept by identity on this device,
never told to a server or to the person. Also BUG-244: the card never shows an empty name.
- Native: `src/net/person_controls.rs` (the record, `file_verdict`, the lines), `AppConfig::person_controls`,
  `src/notifications/decide.rs` (`Candidate::from_level`, `Why::PersonLevel`; the data file's
  `person_level` and `person_levels`), `src/engine/notify.rs` (`person_level`), the file's plan
  (`src/gui/pages/chat/attach_view.rs` `Show::Refused`), the offer's question (`src/net/file_offer.rs`
  `question`, `Question::Refused`; `chat/transfer_rows.rs`), the section (`src/gui/pages/chat/person_controls.rs`),
  the card (`chat/modals.rs` `open_profile`, `card_name`, now a scrolling dialog)
- Web: `web/shared/person-controls.js` (`HosPersonControls`, localStorage `hos_person_controls`),
  `web/shared/notify-rules.js`, `web/chat/chat-notify.js` (`personLevel`), `web/chat/chat-dms.js`
  (`privateFilePersonVerdict`), `web/chat/chat-file-transfer.js` (`refusedPerson`), the card's section
  (`web/chat/chat-profile.js` `personControlsHtml`, `wirePersonControls`, `profileCardName`)
- Tests: `scripts/tests/fixtures/person-controls-cases.json` (both twins), `notify-cases.json` (15 new
  cases), `src/net/person_controls_tests.rs`, `src/gui/pages/chat/person_controls_tests.rs` (drawn
  and clicked headless), `src/engine/notify.rs` `every_candidate_from_a_person_carries_their_level_from_the_card`,
  `scripts/tests/person-controls-web.test.js`; snapshots `user_profile_modal`, `user_profile_modal_narrow`

### Notification Actions
Reply and Mark Read buttons on push notifications.
- Web: `web/shared/sw.js` (service worker)

---

## Task Board

The kinds of task (docs/design/tasks.md, 2026-11-02): **My tasks**, kept on the person's own
devices and synced between them sealed (below), and **this server's board**, the server's own
shared list (these entries). The Tasks page shows them side by side, one at a time, never merged.

### Tasks of my own (Mine), both apps (2026-11-02, fixes BUG-217)
A person's own tasks, kept on each of their devices encrypted with a key from the recovery phrase
(desktop: `<data>/tasks/<hash>.mine`, AES-256-GCM, written crash-safely; website: one record in
the chat's own IndexedDB, AES-256-GCM), and carried between their devices as sealed notes to
themselves in their own mailbox (`[[hum:task:v1]]`, the block list's own way): a server sees
padded sealed self-mail by day, never that it is a task. The merge is per task, the later change
winning in any order. New task, Add to my tasks (one click on a suggested card), move, priority
and Delete; the board's list never replaces them. A task note from anyone else is dropped, and
neither chat ever shows one as a message. Saved tasks that do not open are kept aside (the
desktop renames the file to `<name>.unreadable-<seconds>`, the website copies the record to its own
key), never written over, and the page says so. The website needs the identity Chat keeps in the
browser (a passphrase-protected one cannot be opened from the Tasks page yet). Each page's kind
switch says who can see what it shows.
- Native: `src/gui/mine_tasks.rs` (records, merge, notes, file), `src/engine/mine_tasks.rs`
  (add, change, delete, flush, the arrival rule), `src/gui/pages/tasks.rs` (the switch,
  `take_board_list`, `add_chain_task`)
- Web: `web/shared/mine-tasks.js`, `web/pages/tasks-app.js` (`openMine`, `mineAdd`,
  `renderMineBoard`, the page's own mailbox read), `web/shared/pq-relay-auth.js`
  (`getPqTasksKey`, `getPqDmKeys`)
- Tests: `src/engine/mine_tasks_tests.rs`, `src/gui/mine_tasks_tests.rs`,
  `src/gui/pages/tasks_click_tests.rs`, `scripts/tests/tasks-mine-web.test.js`, both held to
  `scripts/tests/fixtures/task-note-v1.json`

### Task CRUD
Create, read, update, delete tasks on the server's board with title, description, status,
priority, assignee (moderators and admins change it, verified people, donors, moderators and
admins comment: BUG-221). The desktop shows the board read-only for now (rung 5 of the plan).
- Web: `web/pages/tasks.html`, `web/pages/tasks-app.js`
- Server: `src/relay/storage/board.rs`

### Task Comments
Threaded comments on tasks.
- Server: `src/relay/storage/board.rs`

### Project Grouping
Tasks grouped by project with color/icon pickers.
- Web: `web/pages/tasks-app.js` (project modal)
- Server: `src/relay/storage/projects.rs`

---

## Marketplace

### Listings
Create and browse marketplace listings.
- Web: `web/pages/market.html`, `web/pages/market-app.js`
- Server: `src/relay/storage/marketplace.rs`

### Native Market Sync (v0.752)
The native Market page speaks the same WS listing protocol as web:
`listing_browse` on view, broadcast-synced list (`listing_new` / `listing_updated` /
`listing_deleted`), Publish via `listing_create` (client-minted id, the broadcast
round-trip is the confirm), Delete on own listings. `GuiListing` mirrors the relay's
`ListingData`; the wire contract is pinned by a frame round-trip test.
- Native: `src/gui/pages/market.rs`, `src/gui/mod.rs` (`GuiListing::from_relay_json`),
  lib.rs WS dispatch arms (`listing_list` / `listing_new` / `listing_updated` / `listing_deleted`)

### Listing Images
Image upload with drag-and-drop galleries (max 5 per listing).
- Server: `src/relay/storage/marketplace.rs`

### Full-Text Search (FTS5)
Search listings by keyword with SQLite FTS5.
- Server: `src/relay/storage/marketplace.rs`

### Reviews and Ratings
Star ratings and text reviews on listings.
- Server: `src/relay/storage/reviews.rs`

### Seller Profiles
Clickable seller names with aggregate ratings and listing count.
- Server: `src/relay/storage/members.rs`

### Buyer-Seller Messaging
"Message Seller" opens a sealed-sender end-to-end encrypted DM with the seller,
prefilled with the listing reference (2026-08-23; replaced the old server-stored
plaintext listing threads, which were also broadcast to every connected client;
the listing_messages table is dropped by migration). The relay stores no
marketplace correspondence.
- Native: `src/gui/pages/market.rs` (Contact card -> `chat::open_dm_conversation`)

### P2P Trading with Escrow
Direct player-to-player item exchange with dual confirmation.
- Web: `web/pages/trade.html`, `web/pages/trade-app.js`
- Server: `src/relay/storage/trading.rs`, `src/relay/relay.rs`

---

## Social

### Guild System
Create, join, search, and manage guilds with invite codes.
- Web: `web/pages/guilds.html`
- Server: `src/relay/storage/guilds.rs`

### Reputation System
Points, levels, and leaderboard for community standing.
- Server: `src/relay/storage/reputation.rs`

### Safety: who can reach me, Block, reports, warnings, protected setup (dual UI, v0.1465 to v0.1478)
Design: `docs/design/blocking-and-safe-mode.md` (steps A to G, sections 10b to 10h). Settings >
Safety on both clients. **Who can reach me** (v0.1467): one audience per kind (messages, calls,
trades: Nobody, People I choose, Friends, Friends and people in my groups, Anyone), safe by
default, enforced by the relay for everyone, admins included; strangers can send a contact
request that shows only their name. **People I choose** (v0.1472): a Message, Call and Trade tick
per friend, carried in the friendship pass (v2, never expires, withdrawn on Unfollow or Block).
**Block** (v0.1468): hides everything from that person, withdraws the pass, they are not told,
synced to your own devices. **Reports the admins can check** (v0.1469): signed reports with the
reported person's own signed DMs as evidence, never a file; Reports page for admins and
moderators; kept 90 days. **Help outside this server** (v0.1472): for a child or someone in
danger, the report dialog shows the emergency number and child-report body for a country the
person picks (`data/safety/outside_help.json`, from a dated finding). **Calls through the
server** (v0.1470): our own STUN and a room-scoped forwarder on UDP 3478, so nobody in a call sees
another's address. **How my calls connect** (10p, 2026-10-22): a per-person choice in Settings >
Privacy (desktop) and Settings > Safety (web), words from `data/net/call_connection.json`: Private
(the default, relay only), Direct (this device's own addresses too, found with the server's own
STUN), or Direct also asking a public service the person picks; each choice covers only that
person's address, so Private and Direct people still connect through the relay, and the protected
setup is always Private (native `src/net/call_connection.rs`, `src/net/webrtc.rs` `voice_plan`,
`src/gui/pages/settings/calls.rs`; web `web/shared/call-connection.js`,
`web/chat/chat-voice-rooms.js` `callRtcConfig`). **Warnings on messages and the recovery-phrase guard** (v0.1471): warnings
from `data/safety/warnings.json` checked on the device only; a send holding 4 or more of your
own recovery phrase's words in a row is stopped. **An admin erases another person's data** (v0.1474): admins and the owner only, with the person's name typed to confirm, the same steps as erasing your own account; the erased person is told an admin did it (native `src/net/admin_erase.rs`, `src/gui/pages/server_settings/admin_erase.rs`; web `web/shared/admin-erase.js`; relay `src/relay/handlers/account_erase.rs`). **A report about a group reaches its creator** (v0.1475): a sealed DM the creator's app checks against its own copy of the group, with Remove (which changes the group key first), Block and Dismiss (native `src/net/group_report.rs`, `src/engine/group_report.rs`, `src/net/group_remove.rs`, `src/gui/pages/safety_group_reports.rs`; web `web/shared/group-report.js`; relay the `group_report` exception in `src/relay/handlers/reach.rs`). **Files in private conversations are encrypted** (v0.1476): picked, pasted or dropped files in a DM or a P2P group, on both clients, shown inline under the click-to-load rule (native `src/gui/pages/chat/attach_send.rs`, `attach_view.rs`; web `web/chat/chat-messages.js` `sendAttachment`, `chat-dms.js` `privateFileHtml`); Pin, Edit and React are offered and accepted only in a server's own rooms (BUG-178). **A pass counts once the server took it** (v0.1478, spec 10l): every friendship pass put carries a `ref`, the relay answers `dm_put_ok` or `dm_put_refused` to the sender alone, and only `dm_put_ok` records the pass (relay `src/relay/handlers/dm_answer.rs`; native `src/net/put_answers.rs`, `src/engine/put_answer.rs`; web `web/chat/chat-social.js` `holdPassPut`, `settlePassPut`).
- Native: `src/gui/pages/safety.rs`, `src/net/reach.rs`, `src/engine/reach.rs`,
  `src/engine/block.rs`, `src/engine/report.rs`, `src/net/report.rs`,
  `src/gui/pages/chat/report_dialog.rs`, `src/net/outside_help.rs`, `src/net/warnings.rs`,
  `src/gui/pages/chat/warnings.rs`, `src/net/dm_store.rs` (passes, ticks, block list)
- Web: `web/shared/reach.js`, `web/shared/block.js`, `web/shared/report.js`,
  `web/shared/warnings.js`, `web/shared/friend-pass.js`, `web/chat/chat-privacy.js` (Safety),
  `web/chat/chat-social.js`, `web/chat/chat-reports.js`, `web/chat/chat-warnings.js`
- Server: `src/relay/handlers/reach.rs`, `src/relay/handlers/friend_passes.rs`,
  `src/relay/handlers/reports.rs`, `src/relay/call_forwarder.rs`, `src/relay/call_credentials.rs`
- Data: `data/safety/` (warnings, report reasons, outside help), `data/gui/safety_presets.json`
  (the protected setup, step G, v0.1473: a PIN lock on this device's safety settings, a review of who can already reach it, public rooms and non-friends' pictures hidden, never synced; native `src/net/protected.rs`, `src/engine/protected.rs`, `src/gui/pages/safety_protected.rs`; web `web/shared/protected.js`, `web/chat/chat-protected.js`)

---

## Wallet and Funding

### Solana Wallet
Ed25519 identity IS a Solana wallet address. Send, receive, balance queries.
- Web: `web/shared/wallet.js`, `web/pages/wallet.html`, `web/pages/wallet-app.js`

### Token Swaps (Jupiter)
Swap tokens via Jupiter aggregator API.
- Web: `web/shared/wallet.js`

### Staking
Stake SOL with validators.
- Web: `web/shared/wallet.js`

### NFT Support
Detect and display NFTs with Metaplex metadata.
- Web: `web/shared/wallet.js`

### Donation Page
Leads with the two ways to give (2026-10-04): the maintainer on Patreon (he
receives it, not tax-deductible), then the nonprofit Sponsor-a-Can
(tax-deductible, the money goes to Sponsor-a-Can), each card saying so in one
sentence with a badge. Then more direct links, the server's funding goal (no "raised so far": nothing
tracks it) and any addresses it lists, endorsed charities, and the FAQ. Entries with
no link or address are not shown. Native and web read the same four data files.
- Web: `web/pages/donate.html`, `web/pages/donate-app.js`
- Native: `src/gui/pages/donate.rs`
- Data: `data/donate/routes.json`, `methods.json`, `charities.json`, `faq.json`; `data/server-config.json` (funding goal + addresses array)
- Tests: `tests/page_parity_lint.rs` (both sides read all four files: it matches the read calls, the native loader, the startup line and the web fetch, not just a mention of the path), `src/gui/pages/donate.rs` tests (each route's sentence agrees with its tax flag, no route listed twice, never the viewer's own wallet, and, drawn headlessly, no card for a route with no link and no FAQ heading over an empty FAQ)

### Wallet Guide
Step-by-step beginner guide for all wallet operations (receive, send, buy, sell, swap, backup, glossary).
- Web: `web/pages/wallet-guide.html`, `web/pages/wallet-guide-app.js`
- Access: "?" help icon on wallet page tab bar

### Admin Donation Address Management
Admin settings UI for adding, editing, removing, and reordering donation addresses.
- Native: `src/gui/pages/settings.rs` (Donation Addresses section)

---

## Civilization Dashboard

### Live Community Stats
Aggregated population, infrastructure, economy, resources, social, activity metrics.
- Web: `web/pages/civilization.html`, `web/pages/civilization-app.js`
- Server: `src/relay/storage/civilization.rs`, `src/relay/api.rs` (`GET /api/civilization`)

---

## Web Tools and Utilities

### The front door (the website's front page, 2026-11-02)
A title screen with tools in it, adopted by the operator on 2026-11-02 ("Let's do it"), in place of the
seven-screen picture-book page: the mission in a line, what HumanityOS is (the tools and the game), one
search box that reads plain words, "New here? Start here", eight buttons to the in-depth areas, one row
of update cards (as many as fit at 220 px, at most nine; a swipe row on a phone), and Start here's one
question, "What brought you here?", with one first thing to open for each answer and its quest chain
sharing the Onboarding page's ticks. The standard 1,100 px column. No chat on the page (the operator:
"Let's exclude it from the front page"); the Chat button opens the chat. The welcome step for a
visitor the chat sent here (`?welcome=1`) is part of it, and follows only a `?return=` of this site
(BUG-227).
- A signed release's card is dressed its own way: the theme's verified colour on its band, a faint wash,
  and a "Signed" mark with its hint (the operator: "when I sign its widget dressing is slightly
  different"). The server decides "signed" by checking the release's signatures itself.
- Web: `web/pages/index.html`, `web/pages/index-app.js`; the rules and the card: `web/shared/front-door.js`,
  `web/shared/update-card.css`
- Data (rows, not code): `data/front_door/config.json` (the card rule's numbers), `destinations.json` (the
  buttons, each with the desktop page it opens), `goals.json` (Start here), `search_words.json` (words
  that mean the same: "diarrhea" finds the guide that says "diarrhoea")
- Server: `GET /api/front-door`, `GET /api/releases` (the Devlog), `POST /api/release-signature`
  (`src/relay/front_door.rs`, table `release_signatures` in `src/relay/storage/front_door.rs`); the check
  is `crates/humanity-protocol/src/release_signature.rs`, the desktop updater's own; `just sign-release`
  sends each signed pair, `just send-release-signatures` retries or backfills
- Tests: `scripts/tests/front-door-web.test.js`, `src/relay/front_door_tests.rs`,
  `crates/humanity-protocol/src/release_signature_tests.rs`
- Not yet: the desktop's Humanity overview sharing the buttons and the card (design rung 5)
- Design: `docs/design/front-door.md`

### Devlog (every release, one card each)
The update cards a page at a time from this server's `GET /api/releases`, signed releases marked as on the
front door. Since 2026-11-02 it never asks GitHub's API from the visitor's browser (which sent every
visitor's address to GitHub); each card's Release notes link opens that release's notes on GitHub.
- Web: `web/pages/devlog.html`, `web/pages/devlog-app.js`

### robots.txt and the sitemap (2026-11-02)
Search engines kept off the server's API, the uploads, the release binaries and a person's own pages,
and pointed at the public pages; every `/api/` answer, uploaded file and Files item page also carries
`X-Robots-Tag: noindex` (`src/relay/mod.rs` `keep_out_of_search`). AI training crawlers are treated like
any other until the operator decides (the comment in robots.txt says where the line goes).
- Web: `web/robots.txt`, `web/sitemap.xml` (written by `node scripts/gen-sitemap.js` from
  `data/site/public_pages.json`), placed at the web root by `scripts/sync-web-root.sh`
- Test: `scripts/tests/robots-web.test.js`

### Descriptions and share previews on every page (2026-11-07)
Every website page people can reach carries a one-sentence description and a share preview (og:
title, description, url, image, type; twitter:card), written into each page's `<head>` because
crawlers and social sites do not run scripts. One generator writes them from data; nothing is
hand-copied. Design: `docs/design/privacy-notice.md` section 6.
- Web: `scripts/gen-page-meta.js` from `data/site/page_meta.json` and `data/i18n/en.json`
  (`page_meta.*`, `page_titles.*`); the shared picture `web/shared/social/og-banner.png`
- Test: `scripts/tests/page-meta-web.test.js`

### The 404 page (2026-11-07)
Our own page for an address that is not here, in plain words with a way on, in place of nginx's
default page; nginx no longer names its version (BUG-277; the live server needs the operator's
commands in BUG-277). The desktop app has no addresses, so no 404.
- Web: `web/pages/404.html`; `scripts/nginx/humanity.conf` (`error_page 404`, `server_tokens off`)
- Test: `scripts/tests/not-found-web.test.js`

### The privacy policy (2026-11-07)
What the server keeps about a visitor or a member, why, for how long, who can see it and how to
erase it, true to an inventory read from the code, with the known gaps named (BUG-278 to BUG-284).
The law behind it: `docs/reference/findings/2026-11-07-privacy-notice.md`.
- Native: the Library's "Your Privacy" shelf ("Privacy: What This Server Keeps About You",
  `docs/outreach/privacy_policy.md` copied to `data/library/privacy_policy.md`)
- Web: `/privacy` (`web/pages/privacy.html`, `privacy-app.js`) drawing the same file; linked from
  every page's footer (`web/shared/shell.js`) and the front door; in the sitemap
- Evidence: `docs/design/privacy-notice.md`

### File Browser/Editor
Tree navigator for data/ directory. Built-in viewers for text, JSON, CSV, markdown, images, audio, video.
- Web: `web/pages/files.html`, `web/pages/files-app.js`
- Server: `src/relay/storage/files.rs`, `src/relay/api.rs`

### Calculator
Basic, scientific, unit converter modes with keyboard support and history.
- Web: `web/pages/calculator.html`, `web/pages/calculator-app.js`

### Calendar/Planner
Monthly/weekly view with event creation, color coding, localStorage persistence.
- Web: `web/pages/calendar.html`

### Notes/Journal
Local-first note editor with auto-save, search, markdown preview, export.
- Web: `web/pages/notes.html`, `web/pages/notes-app.js`

### External Catalog (the Apps page's programs and help)
Free open-source programs and real-world help services, with search and filters: the Apps page's Other programs,
Help and Services on both apps (see Apps Page above).
- Web: `web/pages/apps.html`, `web/pages/apps-app.js`
- Data: `data/external/catalog.json` (tools+services merged 2026-07-30)

### Resources Page
45 curated real-world resource links (education, health, housing, etc.) + in-game guides.
- Web: `web/pages/resources.html`, `web/pages/resources-app.js`

### Glossary / Dictionary System
150+ terms with definitions. Web: searchable tooltip overlay on all pages.
Native (v0.989): a Dictionary section in the Library (search + category
filters + term cards) plus a "Define words" toggle on every Library
document - click any word for its definition, dictionary hits underlined.
- Web: `web/shared/glossary.js`
- Native: `src/gui/glossary.rs`, Dictionary + define mode in `src/gui/pages/library.rs`
- Data: `data/glossary.json`
- Glossary marks in page words (2026-10-23): `[[term]]` in `data/i18n/en.json` draws
  underlined on both apps with the definition card on hover (desktop
  `src/gui/widgets/glossary_text.rs`; web `[data-term]` spans, click or Enter pins,
  Tab reaches it). Checked by `scripts/tests/glossary-terms.test.js`.

### Library Tags (v0.1305)
Cross-cutting filter chips on the Library, in both clients, grouped by four
axes: reality (real life / in the sim), domain (food, water, power, making,
materials, governance, mission, project), kind of document (guide, reference,
safety critical) and audience (start here, server operators, contributors,
technical, AI participants). A category is the ONE shelf a document sits on;
tags are every other way somebody might look for it, so a doc reachable under
"Making and Repair" is also reachable under "Safety critical". Picking a chip
narrows the rail without changing what is open; categories that end up empty
are hidden; the open document shows its own tags, and clicking one jumps to its
siblings.
- Taxonomy: `data/library/catalog.json` (categories + per-doc tags, the source
  of truth), `data/library/tags.json` (the vocabulary)
- Generated manifest: `data/library/index.json` via `scripts/build-library.js`,
  which FAILS the build on a tag the vocabulary does not define
- Native: `src/gui/pages/library.rs`, loader in `src/gui/mod.rs` (`load_library`)
- Web: `web/pages/library-app.js`, `web/pages/library.html`

### Library Order: A to Z or Suggested (2026-10-10)
The Library rail lists every shelf and every document A to Z by default
(operator: "it's hard to search through while not alphabetical"): by title,
ignoring case and a leading "The", "A" or "An" (titles are shown as written).
Sections keep their order. Two chips at the top of the tree switch to
Suggested, the catalog's own order, the learning path. The choice is kept
(native `AppConfig::library_suggested_order`, web localStorage
`hos_library_order`). Search results keep their relevance order, and the
reader's Next footer always follows the suggested order (the next rung).
- Native: `rail_order` / `title_sort_key` in `src/gui/pages/library.rs` (unit tests there)
- Web: `web/pages/library-order.js` (the same rule), used by `library-app.js`;
  `scripts/tests/library-order-web.test.js`

### Markdown Tables and Line Joining (v0.1305)
Both markdown renderers gained pipe tables, and both now JOIN wrapped source
lines the way markdown requires. Before this a paragraph rendered as one ragged
element per source line, a bullet ended at its first indented continuation line
(42 of the 81 Library documents wrap their bullets), inline markup spanning a
line break never matched (`humanity_accord.md` showed seven bold spans as
literal asterisks), and the 14 documents containing tables showed raw pipe text.
- Native: `src/gui/widgets/markdown.rs` (tables render as an `egui::Grid`)
- Web: `web/shared/markdown.js` (table CSS ships with the renderer, so every
  consumer gets it without a fourth copy of the `.md-viewer` rules)
- Snapshot: `tests/snapshots/markdown_features.png` via `just snapshot markdown_features`

### Admin Dashboard
Server analytics for admins. Users, messages, channels, federation, game state.
- Web: `web/pages/admin.html`, `web/pages/admin-app.js`
- Server: `src/relay/api.rs` (`GET /api/admin/stats`)

### Server → Services (feature + daemon toggles)
Operator one-click control of features backed by OS daemons (coturn
voice/video relay; future P2P distribution via transmission) from the
native Server Settings page, no SSH. Two layers: a soft
`server_settings` gate (relay stops offering instantly) + an
allowlisted privilege bridge that start/stops the daemon. Non-root
relay + tightly-scoped sudoers + compile-time allowlist (no shell, no
client strings as args); security-reviewed (no HIGH/MEDIUM). v0.262.16.
- Server: `src/relay/services.rs`, `src/relay/relay.rs`
  (`service_control`/`service_state`), `scripts/sudoers.d/humanity-relay-services`
- Native: `src/gui/pages/server_settings.rs` (Services panel)
- Design: `docs/design/services-toggles.md`

### Projects Page
Project Universe timeline (Dec 2017 ICU through Jan 2026 rename to HumanityOS).
- The standalone `web/pages/projects.html` timeline page was retired in the 2026-07-05 trim. (This is distinct from the live Projects task-CRUD system, documented separately.)

---

## Maps

### Multi-Scale Map
Galaxy to street level zoom on 2D canvas. Galaxy spiral, solar system, planet globe, OpenStreetMap tiles. Moon orbit fixed (v0.90.8).
- Web: `web/pages/maps.html`, `web/activities/map.js`, `web/activities/celestial.js`
- Data: `data/solar-system.json`, `data/stars-catalog.json`, `data/constellations.json`

---

## Navigation and UX

### Shell Navigation
Color-coded nav groups (red=identity, green=contextual, blue=system) with icon tooltips.
- Web: `web/shared/shell.js`

### Unfinished-software warning (both apps, 2026-10-30)
One amber line fixed to the bottom of every page until version 1.0 (rung 1 of
`docs/design/app-bottom-strip.md`): "Unfinished software: it can be wrong, break, or lose your work.
Do not rely on it for health, safety, food or water. It comes as is, with no warranty." (warranty a
glossary term), What this means opening the Library's `unfinished-software` page
(`docs/user/rights/unfinished_software.md`), and the version. Shown on every page and the title screen,
never in the first-person world or on in-world screens; decided by the version alone (every 0.x and
every pre-release of 1.0), one rule held to one list by both apps' tests
(`scripts/tests/fixtures/wip-versions.json`). Words from the dated finding
`docs/reference/findings/2026-10-30-wip-warning-and-liability.md`; colour the `warning_bg` token.
**Mute (2026-11-02):** for this session, one day or one week (`data/gui/wip_mute.json`, both apps),
from the bar's Mute or Settings > Notifications > Muted (until when, Show it again). Muted, the bar
takes no room; a mute ends at its time, at the next launch for a session's (the website: the tab),
and with a new version (one rule, `scripts/tests/fixtures/wip-mute-cases.json`); kept on the device
only (config.json `wip_mute`; the browser's localStorage/sessionStorage `hos_wip_mute`). The title
screen (website: the front page) and the warning's own Library page show it whatever the mute.
- Native: `src/gui/bottom_strip.rs` (called by `src/lib.rs` before the page; the chat footer is planned
  above it), `src/gui/bottom_strip/mute.rs` (the mute, its menu, the Settings row; tests in
  `mute_tests.rs`), snapshots `wip_strip`, `wip_strip_narrow`, `wip_strip_title`, `wip_details`,
  `wip_strip_muted`, `wip_mute_menu`, `wip_strip_title_muted`, `wip_mute_settings`
- Web: `web/shared/shell.js` (`wipStrip`, `.hos-wip-strip`, `hosWipMuted`, `window.hosWipMute`; the
  footer stands on it through `--hos-wip-h`, a spacer ends the page), `web/pages/settings-app.js`
  (`renderWipMuteRow`), `scripts/tests/wip-strip-web.test.js`, `wip-mute-web.test.js`,
  `wip-strip-room-web.test.js`

### Real/Sim Toggle
Global context switch between real-life tools and simulation mode. Stored in localStorage.
- Web: `web/shared/shell.js` (toggle UI), pages listen for `hos-context-change` event

### Dark/Light Theme
Theme toggle with CSS variables.
- Web: `web/shared/shell.js`, `web/shared/theme.css`

### Onboarding Tour (the first five minutes)
A short guided tour on both apps (2026-10-23): the recovery phrase written down, a first
message, the first steps in the world, and where help lives. A small card in the corner, never
over the page; skippable, resumable (Hide for now keeps the place), steps that tick themselves
when the app sees them done. Starts by itself after the desktop first-run setup; Settings >
Appearance > Guided tour on the desktop, Take Tour or the help button on the website.
- Steps: `data/onboarding/tour.json` (words are keys in `data/i18n/`), checked by
  `scripts/tests/first-minutes.test.js`
- Native: `src/gui/first_minutes.rs` (state in `config.json` `first_minutes`)
- Web: `web/shared/onboarding-tour.js` (state in localStorage `hos_first_minutes`)

### Settings Panel
Gear button with theme, notifications, wallet, and display settings.
- Web: `web/shared/settings.js`, `web/pages/settings.html`

### Localization (i18n)
5 languages (English, Spanish, French, Chinese, Japanese) with fallback.
As of 2026-10-22 every word of a converted page (both apps) comes from
`data/i18n/en.json` by key, a missing word falls back to English then to the
key, and ratchets keep the hardcoded words shrinking (`tests/i18n_lint.rs`,
`scripts/tests/i18n-web.test.js`). The language picker is in Settings.
- Native: `crates/humanity-core/src/i18n.rs` (`t`, `tf`, `tn`, `t_marked`)
- Web: `web/shared/i18n.js`, `web/shared/i18n-en.js` (generated by `scripts/gen-i18n.js`)
- Data: `data/i18n/*.json`
- Plain-language hints on every control, three hint modes (Full / Hover / Off):
  `src/gui/widgets/hint.rs`, `web/shared/shell.js`; ratchets `tests/hint_budget.txt`,
  `scripts/tests/hints-web.test.js`.

### Accessibility
High contrast, reduced motion, font scaling, colorblind mode filters.
- Web: `web/shared/accessibility.js`, `web/shared/theme.css`
- Native: Settings > Appearance > Font Size (2026-11-03, BUG-225): 100% to 200%, the zoom of the
  whole desktop app (every page, both menus, the bottom strip, the HUD), applied before each frame
  and saved; Ctrl+Plus / Ctrl+Minus step it by ten, Ctrl+0 puts it back to 100%. The in-world
  screens keep their own scale. `src/gui/font_size.rs`; snapshots `settings_appearance_100/150/200`
  and `top_row_150_at_720` / `top_row_200_at_720`.

---

## Server and Infrastructure

### WebSocket Relay
Message routing with authentication, rate limiting, federation.
- Server: `src/relay/relay.rs` (~5800 LOC)

### REST API
50+ endpoints for all platform features.
- Server: `src/relay/api.rs` (~2800 LOC), `src/main.rs` (routing)

### Federation
Server-to-server WebSocket connections, trust tiers, profile gossip.
- Server: `src/relay/handlers/federation.rs`
- Numbered log and catch-up (2026-10-22): everything a server sends its links (chat, edits, deletes, erases, bans, profile copies, signed objects) is numbered in its outbox; a link applies it once and in order and catches up after being down or paused. The origin's edits and deletes reach every copy; bans are honoured in shared rooms. `src/relay/handlers/federation_log.rs`, `src/relay/storage/federation_log.rs`
- Per-peer flow limits (2026-10-22): every kind of frame has an allowance; a peer over it is slowed, then paused, never dropped; shown per peer in the admin API (`link.flow`). `src/relay/handlers/federation_flow.rs`
- Phase 3 test, two relays on loopback: `src/relay/handlers/federation_phase3_tests.rs`; live check: `docs/admin/federation-live-check.md`
- The desktop's Commons merges a room only across LINKED servers (2026-10-24): each signed-in server's own /api/federation/servers is read, and a federated room of the same name on a server linked to none of the others (a private node on this PC) stays in that server's list and is never routed through. One rule for the sidebar, the merged view, the header, the composer and send routing: `src/gui/pages/chat/commons.rs` `commons_carriers`; design: `docs/design/federation-ux.md`, build order 6.

### Server Membership
Auto-join on connect, paginated member roster, role management.
- Server: `src/relay/storage/members.rs`

### Database Backups
Automated SQLite backup every 6 hours, keep last 5; "Back up now" in Server Settings (newest 10).
- Server: `src/relay/mod.rs` (background task), `src/relay/storage/backups.rs`

### Continuous Backup (2026-10-22)
WAL shipping built into the relay: every committed change reaches a destination within
seconds (recovery point about a minute), sealed (deflate + AES-256-GCM under a key derived
from `data/backup.key`, object names authenticated). Destinations: a folder, an
S3-compatible bucket (SigV4), or another HumanityOS server (signed by a Dilithium3 backup
sending key; the keeper allowlists senders with a size limit and stores ciphertext only).
Generations with periodic snapshots, retention window (default 72 h), point-in-time restore
(`HumanityOS --restore-backup <file> [--at <time>]`), restore drill (in-app, compares with
the live database; `--backup-drill` for the PC's monthly task, `scripts/backup-drill-pc.ps1`),
alerts when behind. Both apps: Server Settings > ADMIN > Backups and the web admin page's
Backups panel, one endpoint `POST /api/admin/backups`. Runbook: `docs/admin/continuous-backup.md`.
- Server: `src/relay/backup_stream/` (wal, capture, seal, names, dest, s3, peer, host, restore, api)
- Native: `src/gui/pages/server_settings/continuous_backup.rs`
- Web: `web/pages/backups-admin.js`, `web/pages/admin.html`

### Environment Validation
Fail-fast startup with clear error messages for missing config.
- Server: `src/main.rs`

### GitHub Webhook
Signature-verified webhook for CI/CD integration.
- Server: `src/relay/api.rs`

### Game State Authority
Server-side game world with entity management, position validation, player sync. Since ship homes increment 3 (2026-10-04) the world IS the mothership the game draws: its rooms are the ship file's shared zones (the Commons, First Street) and the labelled volumes in them (the Commons' mess hall), furnished from `data/ships/room_equipment.ron`; the Pioneer (`data/ships/starter_fleet.ron`) is retired from the relay, and a stored Pioneer world is upgraded once at startup. The explore quest visits every shared place, then "find your home" (the player's own plot). Spatial queries (room_for_position, smallest room first; entities_near, room_by_id) for AI perception.
- Server: `src/relay/handlers/game_state.rs`, `src/relay/handlers/ship_world.rs`

### Moving aboard: the speed check, transit links, delivery by view (ship homes increment 4, 2026-10-04)
The relay checks every position update against how far anyone can go in the time since (`data/ship/shared_world.ron`: 25 m/s on foot with a margin, 1.5 s banked) and answers a move past it with `game_position_correction`, which the game applies by standing the player back where the relay holds them: corrected, never frozen. The real fast moves are told apart: spawning, a reconnect (as far as the time away allows, at most 90 m), a teleporter by its link ids, shutting the build editor onto one's own plot, driving a vehicle at its own speed. Placed structures have stable ids and pairs name ids (the shipped homestead's west teleporter used to be paired with its ladder by list index). Game news reaches only the players who have the mover in view (250 m, out at 300), never a socket that is only chatting. "Aboard" is inside the ship's bounds, and each home breathes its own air, sized from its design. Nobody is told which plot another player lives on. Declared jumps come at most one a second; the teleporter pads re-arm only once you have stepped off them. The rig walks the game (showcase `walk_to`) the way a person walks with the mouse and W: it turns to face the way it goes, walks, and turns to the facing asked for at the end, at most 150 degrees a second, easing in and out (BUG-165, `crates/humanity-core/src/turning.rs`); it judges one deliberate oversized jump, and walks onto the home's own teleporter and shuts the build editor away from the build spot, each a jump only its declaration explains (the increment 4 review, 2026-10-04).
- Shared: `crates/humanity-ship/src/moves.rs`, `crates/humanity-ship/src/transit.rs`, `crates/humanity-ship/src/ship_space.rs`
- Server: `src/relay/handlers/move_check.rs`, `src/relay/handlers/game_interest.rs`
- Game: `src/engine/move_check.rs`

### Ship's food stores (ship homes increment 3, 2026-10-04)
One stock of meals at the mess hall (`data/food/ship_stores.ron`) that NPCs and players eat from alike: a crew member who eats walks to the mess hall when a meal is due and takes one; a player takes one with the `take_meal` interaction and eats it in their game (the reply names the good, a Basic Ration, since the fleet ledger). NPC homesteads mostly feed themselves (`npc_homestead_self_provided`) and give the fleet nothing for now (the operator, 2026-10-04: 0). Two modes, the server setting `fleet_supply_mode` (2026-10-04): **unlimited**, the default during early development (the stores never run out or refuse a meal), and **stocked**, the realistic mode where an empty store is a missed meal.

**The mess halls serve (ship homes increment 6, 2026-10-12).** Each store stands just in front of
a serving counter (1.1 m), and there are two: the Commons' mess hall and the South Commons' halfway down
First Street (`mess-hall-2-stores`), so every home is within five minutes' walk of one. E at a
counter opens its card: **Take a meal** (in a server's world the server's meal, asked of the
nearest store in reach and written in the fleet ledger; away from a server the ship's stores on
this computer, one a meal time on the game clock, counted in the save's `ship_meals`) and **Fill
your bottles** (every empty water vessel carried, online and offline, counted on the same meter).
Away from a server, Inventory > The fleet says how many meals and litres were counted here. The
mess halls have tables and chairs, and the Commons a recreation room with couches and a screen.
- Native: `src/engine/mess_hall.rs` (`card_offers_meal`, `take_meal`, `fill_bottles`), `crates/humanity-systems/src/ship_meals.rs` (`serve`), `src/gui/pages/hud.rs` (the counter card), `src/gui/pages/fleet_ledger.rs` (`take_meal_at_nearest`), `src/engine/fleet.rs` (`on_meal`, stores from `game_in_view`)
- Data: `data/food/ship_stores.ron`, `data/machines/ship.ron` (`serving_counter`, the tables, the recreation room), `data/ship/fleet_ledger.ron` (the meal good)
- Server: `src/relay/handlers/ship_stores.rs` (`take_meal`)
- Server: `src/relay/handlers/ship_stores.rs`

### Fleet ledger (2026-10-04)
The operator: "the fleet has unlimited of everything and just track what they player uses and contributes. That way they can be in the red or black." Each player's own ledger on the relay: meals taken and reactor power drawn (used), items given at a fleet store and surplus power returned (contributed), each line valued from data (`data/ship/fleet_ledger.ron`: a meal as a Basic Ration, power per kWh, an item at its `trade_goods.ron` base value), with a balance in the red or the black. A give's items leave the backpack when it is made and are held with it (saved with the backpack) until the server answers: a yes keeps them given, a refusal puts them back, so they are never in two places; items promised to a confirmed trade are not free to give. Each give carries its home's id, and after a save load the game settles again any of that home's recorded gives the save does not list (with a correction when fewer items were there). Gifts made in Creative mode are recorded but not counted. A player sees only their own ledger (Inventory > The fleet; on the web, read-only, the chat's command palette "Your fleet ledger"); an admin sees the fleet's totals once three other players have a ledger, and picks Unlimited or Stocked (Server Settings > ADMIN > Fleet supply, mirrored in the web chat's Game Admin window). Erasing an account deletes its ledger.
- Server: `src/relay/handlers/fleet_ledger.rs`, `src/relay/storage/fleet_ledger.rs` (table `fleet_ledger`), setting `fleet_supply_mode` (`src/relay/storage/server_settings.rs`)
- Native: `src/gui/pages/fleet_ledger.rs` (panel + admin), `src/engine/fleet.rs` (held gives, answers, settling again after a save load, meals eaten, power reports)
- Web: `web/chat/chat-fleet.js` (your ledger, read-only), `web/chat/chat-game-admin.js` (Fleet supply + totals)

### Crew Chore AI (v0.663; nameplates v0.667)
Relay-side crew NPCs work through a data-driven chore rotation instead of the old Brownian wander: walk (straight line, no pathfinding yet) to a chore's site (a spot in a room of the ship: the Commons or its mess hall since ship homes increment 3, never on a plot, every walk clear of walls), dwell there "working" for its duration, rotate to the next chore allowed for their role; a crew member who eats goes to the mess hall for its meals (the ship's food stores). Deterministic rotation staggered per crew member; chore state + the human-readable label live in the entity's components (`chore`, `activity`, `chores_done`) so world snapshots / AI perception carry them automatically. State transitions plus 2 Hz travel positions go out as `game_npc_update` to the players who have that crew member in view (since ship homes increment 4; every socket before). Native client spawns/interpolates `RemoteNpc` entities and renders amber humanoid markers for them; each figure turns as a person turns (at most 360 degrees a second, easing in and out) to face the way it is drawn walking, and keeps that facing when it stops, worked out on each screen and never sent (BUG-166). Nameplates SHIPPED (v0.667): the HUD floats each crew member's name over their head out to 40 m, plus the live chore line ("Taking reactor readings") within 15 m -- accent-colored while working at the site, muted while walking to it. Rebuilt every frame from the RemoteNpc components via `GuiState::crew_labels`, drawn through the machine-label world_to_screen path.
- Server: `src/relay/handlers/game_state.rs` (ChoreDef, tick, tick_chore_agent, next_chore_index, step_toward), `src/relay/mod.rs` (broadcast loop)
- Native: `src/net/protocol.rs` (NetMessage::NpcUpdate), `src/net/sync.rs` (RemoteNpc, and its facing), `crates/humanity-core/src/turning.rs` (how a figure turns), `src/lib.rs` (route_game_message + render pass + crew_labels refresh), `src/gui/pages/hud.rs` (crew_label_lines + nameplate draw)
- Data: `data/npc/chores.ron` (14 chores across 6 rooms), `schemas/chore.toml`

### AI Perception API (v0.131.0)
Headless gameplay protocol, AI agents perceive and act in the game world via structured JSON instead of rendered frames. Validates distance for interactions (5m), perception range (20m).
- WebSocket messages: `game_perceive` (room + nearby + environment), `game_interact` (action on entity), `game_query_inventory`, `game_query_entity`
- Server: `src/relay/handlers/msg_handlers.rs` (handle_game_perceive, handle_game_interact, etc.), `src/relay/relay.rs` (routing)
- Docs: `docs/ai/onboarding.md` (Playing the Game section), `docs/design/ai_interface.md` (Game Participation role)
- Test script: `scripts/test-perception-api.js`

### Unified Binary Deploy
VPS runs `HumanityOS --headless`. relay.db at `/opt/Humanity/data/`. systemd service updated (v0.90.0).
- Server: `src/main.rs`, `src/relay/`

---

## Native Desktop Client (egui)

### egui GUI System
Immediate-mode UI with theme.ron, 13 reusable widgets, 20+ pages.
- Native: `src/gui/` (theme.rs, widgets/, pages/)
- Data: `data/gui/theme.ron`

### Universal Widgets (v0.90.0)
13 widgets: badge, detail_row, search_bar, sidebar_nav, category_filter, stat_card, button, data_table, icons, item_list, modal, row, toolbar.
- Native: `src/gui/widgets/` (button.rs, data_table.rs, icons.rs, item_list.rs, modal.rs, row.rs, search_bar.rs, stat_display.rs, toolbar.rs, mod.rs)

### Theme System (v0.90.0)
6 new theme colors (bg_panel, bg_sidebar, bg_sidebar_dark, badge styling). Slider widget with blue-green-red gradient + animated RGB knob.
- Native: `src/gui/theme.rs`
- Data: `data/gui/theme.ron`

### Main Menu
Title screen with Play, Settings, Quit. Overlays on 3D scene.
- Native: `src/gui/pages/main_menu.rs`

### Top menu (the menu as data since 2026-11-03; two rows on the desktop since rung 2)
The menu at the top of every app page, on both apps, drawn from one file (docs/design/navigation.md;
rung 1, until then two hand-written copies that drifted). The file holds the design's eight
categories with every page placed in their rows, what is reachable outside the rows (each with why),
the desktop's timings, and `today_row.web`, the website's single row until rung 5. ON THE DESKTOP,
TWO ROWS (rung 2, 2026-11-03): the eight categories (Humanity, Play, Chat, Life, Apps, Library, Dev,
Settings; "Life" is a working name, the operator's to choose), Back and Forward (the app's history,
next entry), the Aa switch, the bell and help; under it the
category's pages, only pages that exist (Mission, People, Rules, Roadmap, Devlog, Worlds and Servers
wait for rung 7). Resting the pointer 200 ms on another category previews its row (Settings >
Appearance turns it off; Escape ends it); a row that does not fit puts the rest under More, never a
third row. Play: a press of any length opens Play's page, the game's hub (since 2026-11-11 nothing in the menu goes into the world by itself; the 800 ms press and hold was taken out). Sizes and
category colours are theme tokens (`nav_primary_height` and siblings, `nav_cat_<id>`), each in
Settings. The Humanity and Dev side lists became their rows. (The file name `escape_menu.rs` is
older than the menu: it is not an in-game pause overlay.)
- Data: `data/gui/navigation.json` (words and hints by key in `data/i18n/en.json`; the own-life category's id is `own_life`, its name one word, `nav.own_life`), `data/gui/theme.ron` (the sizes and colours)
- Native: `src/gui/navigation.rs` (read once and again on a save, refused whole with the entry named when malformed; the exe's own copy otherwise; `Menu::desktop`, `place_of`), `src/gui/pages/escape_menu.rs` (the two rows, the preview, More, Play's hold; `end_preview` for Escape), `src/gui/pages/play.rs` (Play's page and `enter_world`), `src/gui/widgets/icons.rs` (`named`: the desktop's picture for each icon name), `src/gui/widgets/mod.rs` (`hold_step`, every hold's clock)
- Web: `web/shared/shell.js` (draws `NAV_ROW`, written into it by `scripts/gen-navigation.js` from the file and en.json; `web_label` keeps the website's Home tab saying Home)
- Tests: `src/gui/navigation_tests.rs` (the file reads, every GuiPage once, the rows as drawn hold only pages that exist, each once, a malformed row refused, each page's place), `src/gui/pages/escape_menu_tests.rs` (the rows are the file's, a click on each category, the preview's rest and leave, a click in the preview, the switch off, Play's tap, hold and button, More at 100/150/200% on 2560x1440 and 1280x720), `a11y_tests.rs` (`a11y_the_top_menu_names_and_hints_every_stop`), `font_size.rs` (two lines at every size), `scripts/tests/navigation-web.test.js`, `page_registry_lint`, `page_parity_lint` ("The top menu")
- Snapshots: `tests/snapshots/nav_rows_tv_100.png`, `_tv_200`, `_720_100`, `_720_200` and each `_preview`, `play_page.png`, and `nav_bar_full.png`, `nav_bar_wrapped.png`, `nav_bar_icon_only.png`

### Back and Forward on the desktop (navigation rung 4, 2026-11-04)
The app's history, the way a web browser has one (operator, 2026-11-02: "We need to make it so the
back button on the mouse can take the user back to where they were like the internet browser.";
docs/design/navigation.md section 10). A STEP is a page, a Settings section chosen, a Dev or Profile
section, a chat channel or conversation (server and channel), a Files place or item, a Library
document, a how-to, a Maps view; at most 100, in memory for the session only. BACK AND FORWARD: the
mouse's back and forward buttons (egui `Extra1`/`Extra2`), Alt+Left and Alt+Right while no text box
has the keyboard (a text box keeps them as its word jumps), and the top menu's two arrows (dimmed
with a hint when there is nowhere to go). With a popup open, Back closes the popup first. ESCAPE:
a popup closes first; a page opened FROM inside another goes back to it (its inside steps
included); a page reached from the menu goes into the world; the world brings back the page left;
the title screen stays. Entering the world is never a step, and in the world the side buttons do
nothing (unbound, the operator's answer of 2026-11-03). The Library's own Back list joined the app's
(its reader's Back is the app's, shown while the step before is a Library document).
- Native: `src/gui/history.rs` (`GuiState::go`, `go_to`, `back`, `forward`, `back_out`, `escape`, `enter_world`, `leave_world`, `show_title`; `note_place`, once a frame, sees a place moved to inside a page; `frame`, the side buttons and Alt+arrows, from lib.rs; `close_popup`, which asks gui/layers.rs `close_nearest`), `src/gui/pages/escape_menu.rs` (the arrows), `src/gui/pages/library.rs` (`shown_place`, `show_place`), `data/keymaps.ron` (the Menu keys)
- Tests: `src/gui/history_tests.rs` (pages, a Settings section, a chat channel, a Files place, the cap, a popup closed first, Alt+Left in a text box, the side buttons on a page and in the world, Escape's rules, the world never a step), `tests/page_change_lint.rs` (`active_page` is assigned only in history.rs; it named 40 places when it went in)

### One layer order on both apps (navigation rung 3, 2026-11-04)
What is drawn over what, the operator's order (2026-11-02: "This way it hides/shades the top menu
buttons too"; docs/design/navigation.md section 6), nearest first: 1 tooltips, hints and toasts; 2
urgent (an incoming call and the call's own bar, at the top centre under the menus, over any popup,
never taking its typing); 3 to 5 a full-screen popup's controls, window and shade (the shade dims and
blocks both menus, the warning and the page); 9 the notices and help panels; 10 and 20 the menus; 21
the warning; 30 the footer; 40 the page's drop-downs and windows; 41+ the page. EVERY POPUP closes
with Back and Escape through one path (the image viewer, the help, passphrase-change, file picker,
document viewer, trading post and PIN popups included, which rung 4 left out); a popup asking for a
decision (the passphrase at start, the privacy choice, the death screen) keeps Back and Escape and
the page under it does not move. Escape closes a popup before the pinned help panel, on every screen.
On the website every z-index is on one scale and every modal covers the whole window, the menu and the
warning included (the Tasks, Trade, Wallet, Dev, Market, Guilds and vault modals had sat under the
menu, stopping at the warning); the incoming call is a band at the top centre that dims nothing.
- Native: `src/gui/layers.rs` (`Layer` in his numbers, `area`, `window`, `painter`, `shade`, `seat`, `Close`, `close_nearest`, `urgent_top`, `arrange` from lib.rs last in the frame), `widgets/dialog.rs` and every popup, panel, toast and tooltip on it
- Web: `data/gui/theme.ron` `z_*` tokens, written by `scripts/gen-theme-css.js` into `web/shared/theme.css` as `--z-tooltip` to `--z-page`; every page's and the shell's z-indexes on them
- Tests: `src/gui/layers_tests.rs` (the order with a popup, the notices panel, an incoming call and a toast, read from egui; the shade over both menus, the bell, the help button and the warning; a popup over the notices panel opened after a press on it; Back and Escape on seven popups; the raise plan), `tests/layer_lint.rs` (no egui Area, Window or Order outside layers.rs; it named 73 places in 28 files), `scripts/tests/z-index-scale-web.test.js` (the scale in his order, no z-index written as a number, every shade over the menu and the whole window; 123 numbers in 48 values and 17 shades when it went in), `wip-strip-room-web.test.js` and `wip-strip-web.test.js` (read through the scale)

### Settings Page
Graphics, audio, controls, game, account categories with sliders and toggles.
- Native: `src/gui/pages/settings.rs`

### Play: the game's hub (2026-11-11)
The operator: "Let's turn the play page into the MCP/overview of all the dedicated game related pages." Play in the top menu only ever opens this page (no press and hold). On it: the world you will enter (who and where you played last) with its Enter the world button, in a magenta tint section, then a card per page of Play's row in data/gui/navigation.json (Characters, Quests, Inventory, Crafting; Worlds and Servers join when built), each a tint section with a live summary (the last character and the saves the picker counted; the quest under way and its step; what you carry and your health; the recipes and how many you have the materials for) and an Open button that opens the page as a step from the hub (Escape comes back). Up to three cards across. The website's Play opens /download (no game runs in a browser), so it has no hub to mirror.
- Native: `src/gui/pages/play.rs` (`cards`, `summary`, `open_card`, `enter_world`), each page's `hub_summary` (`quests.rs`, `inventory.rs`, `crafting.rs`)
- Test: `escape_menu_tests.rs` `play_only_opens_its_hub_and_the_hubs_button_enters`; snapshot `play_page`

### Gameplay settings' levels, Off included (2026-11-11)
The operator: "I feel like we need to provide more than two options for our gameplay settings. Including just turning settings off completely." Settings > Gameplay draws each setting from data/gameplay/levels.json, its levels gentlest first, each with its name and a sentence saying what it changes (shown under the row and as each radio's hint). Hostile wildlife: Off (default) / Few (a third of each pack, at least one) / Normal. Vitals drain: Off / Slow / Real time (default) / Fast, plus its slider. Body heat: Off (the body at its neutral 36.8 C always, no shivering, sweat, condition or harm) / Forgiving (default) / Realistic. Illness and injury: Off (no food poisoning, no wound from a bite, burn or fall, no harm from burns or falls, any illness or wound heals at once) / Forgiving (default) / Realistic. Farm animals: Off (they take nothing and always make their full yield) / Forgiving (default) / Realistic. Walking pace: Realistic / Brisk / Simplified (default). Animation: Simplified / Realistic (default). Carrying weight: Off (no limit) / Forgiving (default) / Realistic. Death: Off (the player cannot die, health stops at 1) / Simplified (default) / Realistic. Mining drone: Off (always charged, never wears) / Simplified (default) / Realistic. Saved as `AppConfig::gameplay` (setting id to level id); an old config's true/false switches read as the defaults. The dev IPC's `gameplay` verb sets any level (`body_heat=off`).
- Native: `src/gui/gameplay_levels.rs` (the file, `level_of`, `understood`), `src/gui/pages/settings_gameplay.rs`, each system's `Mode::from_level` (`body_heat`, `illness` with `wounds` and `falls`, `animal_care`, `encumbrance`, `death_pack` with `combat` and `food`, `mining`), `src/engine/survival_env.rs`, `walk_pace.rs`, `world_load.rs` `spawn_wilds`
- Data: `data/gameplay/levels.json`, `data/game.csv` (`*_speed_brisk_mps`)
- Tests: one per system that Off means off (`body_heat_off_never_harms_or_chills`, `illness_and_injury_off_means_no_wound_no_illness_and_no_harm`, `farm_animals_off_need_nothing_and_always_lay`, `carrying_weight_off_has_no_limit`, `death_off_means_the_player_cannot_die`, `the_drone_off_flies_free`, `hostile_wildlife_off_few_and_normal`), the file's (`gameplay_levels::tests`), the page's and the save's

### The garden's and the ship's gameplay levels (2026-11-12)
The second pass of the levels: the five settings left as hand-drawn switches became rows of data/gameplay/levels.json's "garden" group, drawn under "Garden and ship" on Settings > Gameplay, each level saying what it changes. Garden pests, diseases and weeds: Off / Gentle (default, half the damage) / Realistic. Pollination: Off (every crop sets fully) / Gentle (new, default: the whole loop, but an unpollinated crop loses half of what it would, in its harvest, card and notice alike) / Realistic. Soil pH: Off (not modelled) / Gentle (new, default: the drift and the amendments, half the harvest lost outside the window) / Realistic. Picking: Off (new: every crop harvested once when ripe, its whole season at once; a part-picked plant gives only what it had left) / Forgiving (default) / Realistic. Ship life support: Station-supplied (default) / Realistic, with no Off because the air is the garden's physics at every level. Pipe markings and the HUD's survival bars stay switches: cosmetic. An old config's `pest_severity`, `soil_ph`, `pollination`, `picking_realistic` and `life_support_realistic` read as the defaults. Why each set of levels: docs/design/graphics-settings.md, "The garden's and the ship's levels".
- Native: `src/engine/garden_levels.rs` (`GardenLevels::of`, `publish`), `src/gui/gameplay_levels.rs` (`Setting::group`, `understood`), `src/gui/pages/settings_gameplay.rs` (`draw_group`); systems: `farming::pollination` (`severity`, `kept`, `CropPollinationDef::at`), `farming::soil_ph` (`severity`, `crop_ceiling`), `farming::picking` (`Mode`, `HarvestWindows::plan_at`), `life_support::Mode`
- Tests: `gentle_keeps_half_of_what_an_unpollinated_crop_loses`, `off_mode_sets_fully` (pollination), `every_soil_ph_level_takes_what_its_words_say`, `picking_off_harvests_every_crop_once`, `realistic_picking_loses_what_is_not_picked_in_time`, `every_garden_level_reaches_its_system`, `the_panels_requests_reach_their_systems`, the save's and the Garden page's key (`garden_mirror_tests`)

### Inventory Page
Every part in a tint section (2026-11-11, the operator: "Tighten it up. Bring in the RGB sections like we have elsewhere"): Status red, The fleet blue, Equipment yellow, You and your places green, Garden cyan, Mining magenta, Vehicles red, each still a fold. The status cards are compact (name and number on one line over a thin bar, up to four across: two short rows for eight vitals), the equipment slots are labelled cards with the worn thing's name or Empty (a click takes it off), and the item tiles are tighter (80 points, a 40-point picture). Words by key.
- Native: `src/gui/pages/inventory.rs`, `src/gui/widgets/tint_section.rs` (`frame`, the section without a title of its own)

### Chat Page (3-Panel, v0.89.0)
DMs (red), Groups (green cards), Servers (blue), message feed, input bar. DMs/Groups headers have settings cog menus (v0.90.0). Server header cog replaces X disconnect (v0.90.0).
- Native: `src/gui/pages/chat.rs`

### Compact Theme (v0.90.3)
All spacing/sizes halved for actual visual density. 35+ theme variables editable in Settings > Widgets.
- Native: `src/gui/theme.rs`
- Data: `data/gui/theme.ron`

### All Pages Refactored (v0.90.1)
All 27 pages refactored to use theme system and universal widgets consistently.
- Native: `src/gui/pages/*.rs`

### Post-Quantum Sealed-Sender DMs (both clients, 2026-08-23)
DMs use Kyber768 / ML-KEM-768 (FIPS 203) -> BLAKE3-KDF -> AES-256-GCM with the
SENDER SEALED INSIDE the ciphertext: the inner payload `{v:2,from,to,ts,text,sig}`
is Dilithium3-signed (preimage `hum/dm/v2\n...`) and clients verify authorship
end-to-end. The relay stores only `(to_key, envelope, arrival_day)` in `dm_mailbox`;
it never learns who a message is from, mail expires after `dm_mailbox_ttl_days`
(Server Settings, default 30), and `dm_purge` scrubs a user's own queue on demand.
Long-term history lives client-side under seed-derived encryption: native
`src/net/dm_store.rs`, web `web/chat/chat-dm-store.js`. Byte-shape identical
web<->native (`src/net/dm_pq.rs` <-> `web/chat/crypto.js`). The v1 dual-seal
`{v:1,r,s}` envelope, the plaintext-capable `dm` message, and the legacy
`direct_messages` graph table are all deleted. Canonical detail: the crypto
table in `CLAUDE.md` + `docs/reference/retention_and_deletion_semantics.md`.
- Native: `src/net/dm_pq.rs` (seal/open), `src/gui/pages/chat.rs` (send/receive UI)
- **Kept by mailbox, not by arrival (2026-11-06, BUG-259)**, and the rows numbered in arrival
  order dropped by the upgrade (2026-11-07, BUG-262: `dms::OLD_NUMBER_CEILING`, the migration in
  `src/relay/storage/mod.rs`). Ending the pairing for good (your own copies off the mailbox, cover
  traffic) is planned, not built: `docs/design/unpairable-dms.md`.

### HUD
Health bar, hotbar, crosshair, compass, day/night indicator, FPS counter. **Survival rows and the active quest
(2026-09-25):** food, water, energy, air, body temperature and waste under the health bar, then the first
active quest and its step. Settings > Gameplay > "Survival bars on the HUD": Always / When low (the default:
a need shows once below half; air and body temperature whenever out of range) / Off (health only).
`hud::vital_rows` decides, tested; snapshot `hud_vitals`.
- Native: `src/gui/pages/hud.rs`

### Maps Page
Multi-scale map with celestial navigation (nav label "Maps", `GuiPage::Maps`; the
duplicate `GuiPage::Cosmos` variant merged into it in v0.1145). Real Kepler orbital mechanics (`crates/humanity-core/src/cosmos.rs`,
one `SolBody` set + one propagator shared by the Maps page, FPS world spawn,
and the ECS position resolver -- see `crates/humanity-core/src/ecs/cosmos.rs`), a "Focus" button
(since Maps increment 1 a flight to the body that then stays with it; it was a
one-shot snap-to) and a "Track" button (continuous camera follow as the body
orbits, v0.647.0).
- Native: `src/gui/pages/maps/` (Maps increment 0, 2026-10-20: it was
  `src/gui/pages/cosmos.rs`, 3,880 lines, carved by view): `mod.rs` (the page,
  the view tabs, the shared pan and zoom canvas and footer), `system_view.rs`
  (the 3D Solar System view, its camera `MapCamera`, the reference-orbit and
  Lagrange overlays, a body's card with Focus and Track), `body_list.rs` (its two
  side panels), `planet_view.rs`, `galaxy_view.rs`, `night_sky.rs`,
  `sky_events.rs` (conjunctions, eclipses, the forward scan), `time_bar.rs` (the
  page's clock), `tests.rs`; state `GuiState::maps` (`src/gui/state_types/maps.rs`).
  The build order from here: `docs/design/maps-zoom-to-anything.md` section 6.
- **Opens on you, follows you, draws your ship (Maps increment 1, 2026-10-21).**
  Opening the page from the game frames your ship (about 1.5 ship lengths, Earth
  behind it) and follows it as it moves; "Stop following" leaves the camera where
  it is, a pan or a Focus on a body ends following too, and "Back to me" (or the
  Home key) flies back and follows again; Settings-style toggle "Open where I left
  it" on the page (`settings.maps_open_where_left`, persisted). The camera is an
  anchor plus an f64 offset, 10 m to 1e14 m, the ship's own up at ship scale and
  the fixed stars beyond Earth orbit, with an interruptible zoom-out, travel,
  zoom-in flight (`maps/camera.rs`, `maps/controls.rs`). At ship scale and aboard,
  the backdrop is the real world rendered from the map's camera (the ship from its
  own meshes, lit by the real Sun, the real Earth behind; half resolution while
  the camera moves), `src/engine/map_view.rs`; the page says why when it cannot
  draw it (not aboard, world not loaded, clock off Live). The ship's name and real
  height, its orbit line (from its own state vector), "You" aboard, and the mining
  asteroids at their data positions relative to the ship: `maps/you_layer.rs`. The
  Stations rows come from every `data/stations/*.ron` with a computed altitude
  (`maps/stations.rs`; the home is 35,870 km up, synchronous with the 24-hour
  solar day). Everything about the ship comes from the engine, never recomputed:
  `src/engine/map_feed.rs` (`MapSelf`). The clock opens on Live (the game's own);
  scrubbing detaches it and hides the world layer. Rig verbs `map`,
  `map_distance`, `map_ship`, `map_turn`, `map_report` (`debug/map_done.json`);
  vantages `maps-follow-home*` with the pixel-judged "ship centred" check
  (`scripts/lib/map-centre-check.js`) and its control; snapshot `maps_follow_home`.
- **A solar system that looks real (Maps increment 2, 2026-10-21).** Every body is
  a disc of its REAL colour (`map_color` in `data/star_systems/sol.json`, the mean
  of its albedo map where it has one, `scripts/build-body-colors.js`) lit on the
  side facing the Sun, so the phase shows (`maps/bodies.rs`), at true size with an
  icon floor, giving way to the real globe where the world layer draws one. The
  Sun is limb-darkened with an eye's point-spread glare round it. Orbit lines fade
  behind their bodies, the body is an exact vertex, sampling adapts to the view,
  and an orbit too small or far too big on screen fades out; asteroids' and
  comets' orbits show only near or selected (`maps/orbits.rs`). The belts are
  20,000 REAL minor planets from the Minor Planet Center's MPCORB
  (`data/maps/belts.json` + `data/maps/MPCORB-header.txt`, `scripts/build-belts.js`,
  terms in `docs/reference/findings/2026-10-21-mpc-orbit-data.md`): the main belt,
  the Hildas, Jupiter's two trojan swarms, the Kuiper belt (`maps/belts.rs`).
  Names go through the label placer, none overlapping (`maps/labels.rs`). The
  overlays are data: `data/maps/layers.ron` (the switches), `reference_orbits.ron`,
  `lagrange_pairs.ron` (`maps/overlays.rs`). The world layer draws the real star
  sky behind the solar system (`ViewPasses::SkyOnly`, `engine/map_view.rs`) and
  the whole world out to the edge of near space. A Diagram switch (log-spaced
  orbits, enlarged bodies, labelled not to scale, `maps/diagram.rs`). Theme tokens
  `map_*` (Settings > Appearance "Maps colors", Widgets "Maps: ..."). Cost stage
  `cpu.maps_symbology`. Rig verbs `map: overview`, `map_layer`, `map_diagram`
  (`engine/map_rig.rs`); vantages `maps-system-overview` and its control with the
  `map_overview` check (`scripts/lib/map-overview-check.js`); snapshots
  `maps_system_overview`, `maps_earth_moon`. Web: `web/pages/maps.html` places the
  planets at their real positions from the same sol.json, in their map colours,
  lit toward the Sun, with fading orbit trails, the real belts from the same
  belts.json and the names through `web/shared/label-placer.js`
  (`web/shared/sol-orbits.js`, the web twin of the orbit model).
- **Into the ship, deck by deck (Maps increment 3, 2026-10-22).** Close to your ship the
  LEVEL PICKER (`src/gui/widgets/level_picker.rs`) lists its levels top to bottom with yours
  marked; the levels come from the data (`crates/humanity-ship/src/levels.rs`: every distinct storey floor of
  every body aboard, neighbours' homes included; ordinals as IMDF's, named as the editor names
  storeys). Picking one shows it CUT OPEN: the home is drawn cut at that level by the build
  editor's own rule (`HomeStructure::cut_away`, which moved there from the editor;
  `src/engine/map_cut.rs`, `editor::wanted_cuts`, `HomeDraw::cut` now `Cuts`), roof and hull
  left out; its rooms are filled in their room types' colours (`default_color` in
  `data/rooms.ron`, `RoomBounds::fill`) and named as the HUD names them, with your dot and the
  way you face (the feed's `level` and `you_room`, by the game's own rules)
  (`src/gui/pages/maps/ship_view.rs`). "Plan" turns it straight down and draws it from baked
  PLAN TILES (`src/engine/map_tiles.rs`, their grid `src/gui/pages/maps/tile_grid.rs`: 512 px tiles keyed by level, zoom, x, y, one baked a
  frame through `ViewPasses::Plan`, 64 MB least recently used, and only the tiles an edit
  touched dropped, from a hash of every wall, slab, piece, light, room volume, corridor,
  neighbour and machine). Theme tokens `map_room_fill`, `map_room_edge`,
  `map_room_fill_alpha`. Cost stages `gpu.map_tile_*`, `cpu.map_tiles` (rows "Maps: plan
  tiles"). Rig verbs `map_level`, `map_plan` (`src/engine/map_rig_ship.rs`), the report's
  `ship` block; vantages `maps-ship-upstairs`, its control `maps-ship-upstairs-ground` and
  `maps-ship-plan` (with an edit through the `console` verb) judged by
  `scripts/lib/map-ship-check.js`; snapshots `maps_ship_upstairs`, `maps_ship_plan`. Web: a
  "Ship" button on `web/pages/maps.html` opens the ship's plan level by level
  (`web/shared/level-picker.js`, `web/shared/ship-plan.js`) from `data/maps/ship_plan.json`,
  which the app writes (`the_web_ship_plan_is_current`).

### Profile Page
User profile view/edit.
- Native: `src/gui/pages/profile.rs`

### Tasks Page
The Tasks page in native UI: My tasks and this server's board side by side (2026-11-02,
docs/design/tasks.md), each kind's line saying who can see it.
- Native: `src/gui/pages/tasks.rs`

### Suggested cards on the Tasks page (2026-10-30)
The learn-by-doing chains (`data/onboarding/quests.json`) as hollow "Suggested"
cards at the top of the Todo column (Backlog on the web), replacing the separate
Learn by doing side panel at the operator's request: title, one-line `purpose`,
"1 of 4", the steps with their ticks (the same `chain:step` progress keys as
before), a step's words with Go there and, where a step names a written how-to
(`how_to`), Open the how-to. Add to my tasks makes the chain one of My tasks,
linked to it (`GuiTask::chain`, the record's `chain`), in one click in both apps
(since 2026-11-02; the website's used to open the server's New Task form); the card
then goes away and the task shows its steps. The cards show on My tasks only. Hide
suggestions hides them. The shared hollow card is `widgets::card_suggested` /
`.card-suggested`.
- Native: `src/gui/pages/tasks.rs` (draw_suggestion), `src/gui/widgets/mod.rs` (card_suggested)
- Web: `web/pages/tasks-app.js` (renderSuggestions), `web/shared/theme.css` (.card-suggested)

### Wallet Page
Wallet management in native UI.
- Native: `src/gui/pages/wallet.rs`

### Market Page
Two tabs: the Directory (default) and free-form Classifieds.
- Native: `src/gui/pages/market.rs`

### Market Directory (v0.1141)
The read side of the signed catalog: background REST fetch of
`/api/v2/objects` (provider_v1 + offering_v1), client-side CBOR decode,
latest-revision resolution per (provider root, offering_key), TTL/expiry
filtering, Real/Sim split via the `reality` field. Views: offering browse
(need-shaped category sidebar + search), provider storefronts, full offering
detail with copyable provider key for DM contact. Categories are the
federation-shared vocabulary in `data/market/categories.json` ({id, label,
desc}), enforced at the relay's ingest chokepoint and consumed by native,
web (select population + badge palette), and the validator alike.
- Native: `src/gui/pages/market_directory.rs`, `src/gui/pages/market.rs` (tabs)
- Vocabulary: `data/market/categories.json`; relay: `market_payloads::shared_categories`
- Web: `web/pages/market-app.js` (`loadMarketCategories`)

### Market In-App Publishing (v0.1143)
"+ Publish" on the native Directory tab: create/update your shop
(provider_v1) and publish offerings (offering_v1) without a terminal.
Payloads are validated locally with the EXACT relay validators before
anything is sent, signed with the chat identity (`pq_object_keypair` +
`ObjectBuilder`), POSTed to `/api/v2/objects` off-thread; a success
refetches the directory. Enum dropdowns read the same
`market_payloads` pub consts the validator enforces. End-to-end test:
`market_publish::tests::end_to_end_against_local_relay` (ignored; run
against a throwaway `--headless` relay).
- Native: `src/gui/pages/market_publish.rs`; identity: `src/net/identity.rs::pq_object_keypair`

### Maps Galaxy View on the Real Catalog (v0.1145)
The Maps page's Galaxy view draws data/stars-map.bin: the full HYG catalog
(109,400 stars) as true galactic-cartesian positions in light-years,
brightest-first so the zoom-scaled magnitude cutoff is a plain break.
Spectral colors via the skybox's ci_to_rgb (one physics, two renderers),
1-2-5 distance rings to 10,000 ly, named-star labels + hover detail.
Generator scripts/build-stars-map-bin.js (just build-stars-map) is
self-verifying (31 checks) and the Rust parser locks it with
shipped_stars_map_bin_is_real (Sirius 8.6 ly, Vega 25 ly, sort order).
Ladder: docs/design/maps-ladder.md (OSM planet GPS next).
- Native: src/gui/pages/maps/galaxy_view.rs (map_stars/parse_stars_map/draw_galactic_view)

### Maps Planet View: OpenStreetMap Regions (v0.1146)
Since whole-Earth maps rung 2 (2026-11-09) the view draws named places cut from the map packs; the region files below are retired.
The Maps page's first view is Planet: real Earth regions as GPS-style 2D
maps. data/maps/regions/*.bin (HOSMREG1: roads by class + building
footprints with heights, meters east/north of a region origin) are
generated by scripts/fetch-osm-region.mjs from a one-off Overpass fetch
(the app NEVER calls OSM at runtime; ODbL attribution renders in the
view footer, a license obligation). Renderer: class-laddered road
styling, building outlines tinted by height, named-road labels + hover,
scale bar, viewport culling. Demo region: Seattle Center (8,966 roads,
1,457 buildings, byte-deterministic rebuilds). Rust lock:
shipped_seattle_region_is_real (Pike Street, Rainier Square Tower 259 m).
Ladder: docs/design/maps-ladder.md rung 3 (next: 3D extrusion in-world).
- Native: src/gui/pages/maps/planet_view.rs (map_regions/draw_planet_view; the parser, parse_region, is crates/humanity-terrain/src/osm_region.rs)
- Generator: scripts/fetch-osm-region.mjs

### Map Label Placement (2026-09-27)
Star and street names on the Maps views never print over each other: one greedy
collision pass per view, the standard interactive-map technique. Names are offered
in priority order (brightest star first; motorway before side street, then the
longer road), each with candidate spots in preference order (a star's name right
of its dot, then left, above, below; a street's name at five points along its
length, middle first), and a name with no clear spot is dropped. Labelled dots,
the footer and the scale bar are reserved; a street name is not repeated within
220 px and wears a thin ground-coloured halo. The footer (and the OpenStreetMap
attribution the licence requires) sits on a themed backing chip. The web Maps
page runs the same placer for its star labels and puts its attribution on the
same chip, from theme variables.
- Native: `src/gui/widgets/label_placer.rs` (`LabelPlacer`, `point_candidates`,
  `along_polyline`, `LABEL_GAP`, unit tests), `src/gui/pages/maps/galaxy_view.rs`
  (`label_stars`, `label_rings`) and `src/gui/pages/maps/mod.rs` (`MapFooter`)
- Web: `web/shared/label-placer.js`, `web/pages/maps.html` (`renderStellar`,
  `#attribution`)

### OSM Regions Extruded In-World (v0.1148)
Retired by whole-Earth maps rung 2 (2026-11-09): the mesher stays, fed by `map_meshes` districts from the packs.
The same HOSMREG1 regions stand up in the 3D world: real roads draped on
the terrain and real buildings extruded from their footprints, planet-
fixed (they ride Earth's spin like the trees). crates/humanity-terrain/src/osm_region.rs
is the shared parser + projection contract (region meters <-> lat/lon <->
unit dir, f64 end-to-end per the planet-scale discipline) + ear-clipper
triangulation + build_region_meshes, which takes an elevation closure so
mesh building stays testable without a renderer. src/engine/region_meshes.rs
wires it live: a progressive 512x512 elevation grid sampled on the main
thread (TerrainTiles cannot cross threads; 16,384 samples/frame), a
background mesh-build worker, five materials (masonry/concrete/glass by
height band, asphalt, footpath), roads lifted +0.18/+0.12 m over the
ground, building walls skirted 2.5 m below grade so slopes never show
gaps, sea-clamped bases. Regions auto-load from data/maps/regions/*.bin
within 40 km (drawn to 60 km) and rebuild once when terrain tiles sharpen.
Shipped regions: Seattle Center + Silverdale WA (the operator's real
town). Probe vantages silverdale-osm-ground + seattle-osm-5km +
silverdale-osm-2km lock it visually.
- Native: crates/humanity-terrain/src/osm_region.rs, src/engine/region_meshes.rs (lib.rs hook in the earth branch)

### Region Water: Real Inlets, Lakes, and the Terrain Carve (v0.1149)
Retired by whole-Earth maps rung 2 (2026-11-09): the shore band in `map_ground` replaced the carve.
Operator field report: "the water isn't right... Dyes Inlet is missing.
Lakes are missing too." Root cause: the sea only renders where the coarse
(460 m) heightmap dips below sea level, so narrow inlets vanish, and
lakes sit above sea level where the ocean shell cannot exist. Fix is the
flight-simulator technique: vector water masks override raster terrain.
HOSMREG2 adds water records (sea assembled from OSM coastline ways with
the land-left/water-right closure rule, inland water incl. multipolygon
relations, island inner rings). crates/humanity-terrain/src/water_carve.rs rasterizes
each region's water to a 1024x1024 mask and the terrain formula sites
(build_patch_mesh + drawn_elevation_normalized, i.e. patches, walk clamp,
grass, region grids) press ground under sea polygons 2 m below sea level,
so the EXISTING animated ocean fills the inlet; the water shell's
coverage + depth bake honor the mask too. Lakes get flat sheets at their
lowest-shore level (RegionMeshKind::WaterInland) with beds carved 1.5 m
under. The 2D Planet view draws the same polygons (ear-clipped fills,
island repaint, named-water labels). Rust locks: 7 water_carve tests +
shipped-file locks (Dyes Inlet sea area > 5 km2, Island Lake + its Clark
Island hole, Lake Union in Seattle Center).
- Native: crates/humanity-terrain/src/water_carve.rs, osm_region.rs (HOSMREG2 + lake sheets), planet_chunks.rs (carve + shell), src/engine/region_meshes.rs (mask publish), src/gui/pages/maps/planet_view.rs (2D)
- Generator: scripts/fetch-osm-region.mjs (HOSMREG2, coastline sea assembly, 34 self-checks)

### Whole-Earth Maps, Rung 1: the Coast Yardstick, the Tile Key, the Credit (2026-11-09)
The first rung of docs/design/whole-earth-maps.md (one grid for the map and the ground). THE GATE:
the showcase verb `{"shore_probe":"lat,lon;lat,lon","shore_step_m":"2.5"}` writes
`debug/shore_probe.json`, the ground along a great-circle transect every 2.5 m: the walk oracle the
player stands on, the patch drawn there, the raw land layer (the region's survey DEM, else the
uncarved ground) and the game's own coastline reading with its source (a region's OSM polygons,
about 9 m cells, or the 0.05-degree ocean mask). A vantage's `shore_probe` makes probe-sweep judge it
(scripts/lib/shore-probe-check.js): wet, dry, no_false_wall, kept_cliff, shallow, no_seam, with ends
checked against coastline data (a coast the game can only see on the coarse mask must be declared,
with its source, or the vantage is MISCONFIGURED). Four vantages measure today's coasts:
coast-silverdale-waterfront, coast-silverdale-old-edge, coast-dover-cliffs, coast-norderney-beach
(their red results are in each vantage's `_red`). Rig-only switches `HUMANITY_SHORE_TAPER=0` and
`HUMANITY_REGION_DEM=0` rebuild the region masks without the v0.1153 taper or the DEM overlay, for the
red runs. THE TILE KEY: crates/humanity-terrain/src/map_tiles.rs (relay-safe): level for a depth,
ancestor tile by mask, tile containing a point, the nested height lattice, the great-circle clipper
with clip-edge flags, the 10^7-degree codec. THE CREDIT: the OpenStreetMap notice over the 3D view has
its own layer (src/gui/map_credit.rs), so it shows in the construction editor, the showroom and with
the HUD off, and only while a region is within draw range (`region_meshes::drawn_names`; it used to
stay on anywhere on Earth once a region had been built); LICENSES.md no longer files the region `.dem.bin` files under OSM; data/credits.ron gains
NOAA ETOPO 2022 and the AWS Terrain Tiles.
- Native: src/engine/shore_probe.rs, src/engine/ipc.rs (verb), crates/humanity-terrain/src/map_tiles.rs, src/gui/map_credit.rs (rung 1's `water_carve.rs` and `region_meshes.rs` were retired by rung 2, below)
- Rig: scripts/lib/shore-probe-check.js, scripts/probe-sweep.js, tests/visual/vantages.json; tests: scripts/tests/shore-probe-check.test.js, map_tiles_tests.rs, credits.rs `shipped_heights_and_maps_name_their_sources`

### Whole-Earth Maps, Rung 2: Home Packs, the Shore Band, the Nested Lattice (2026-11-09)
The second rung of docs/design/whole-earth-maps.md. THE PACKS: `data/maps/packs/*.hmpack` (HOSMPAK1,
crates/humanity-terrain/src/map_pack.rs), one per depth-6 terrain patch (about 110 km), every layer
cut into the level-6, -9 and -12 tiles by the same PatchId key: GEDTM30 land heights and ETOPO seabed
as the tiles' 64-segment nested lattices, ESA WorldCover classes, and OpenStreetMap water rings (the
sea closed tile by tile, clip edges flagged), ways and buildings, each layer with its source,
snapshot and licence and the ODbL/CC BY/Copernicus notices in the pack's own manifest. The two home
packs (f00-320230 and f00-320212: Silverdale straddles them, Seattle Center is in the second) ship
with the app; data/maps/places.ron names the places and their gates, data/maps/levels.ron the levels
and the shore band's numbers. THE BUILDER: our own Rust tool (crates/humanity-terrain/src/map_build/,
`just map-packs`): its own OSM PBF reader, tiled GeoTIFF reader and GEDTM30 decode (the SCALE trap
pinned on Rainier and Adams), each pack built twice and compared byte for byte, every chunk decoded,
every place's gates passed. THE GROUND: crates/humanity-terrain/src/map_ground.rs, applied inside
`drawn_elevation_at_depth` (the one formula): land from the lattice (a patch vertex IS a sample),
OSM's coastline deciding land from water exactly, the distance to the TRUE shoreline shaping a shore
band (a straight slope through the waterline at least one vertex spacing wide past the wet distance,
so even a 54 m triangle draws water 12 m out; the equilibrium beach profile into ETOPO's seabed;
lakes flat at the median of their shore), faded into the base only at an edge whose neighbour pack is
missing; decoded tiles resident while the selection's patches need them, all of it obeying the Dev
page's reuse switch; the built-over mask for trees and grass per level-12 tile. THE MESHER: every
patch's 16-segment vertices are now four rounds of the tree's own midpoint subdivision (the nested
lattice), and DrawnPatchSurface finds the drawn triangle by the same descent. THE STREETS:
src/engine/map_meshes.rs builds OSM districts (level-9 tiles whose nearest ground is within 15 km
of the camera, drawn within 25 km) on a worker, every vertex on the finest drawn ground; the Maps
Planet view cuts its places from the packs. A drawn vertex on a tile border is judged 5 cm inside
its tile (the rings' corners are rounded to OSM's integers), so the open sea has no ridges along
tile borders. Retired:
data/maps/regions (HOSMREG2, HOSDEM1), scripts/fetch-osm-region.mjs, scripts/fetch-region-dem.mjs,
water_carve.rs, region_meshes.rs, the AWS Terrain Tiles credit. Red-run switch:
`HUMANITY_SHORE_BAND=0`. Road beds and building pads came in rung 2b (below).
- Native: crates/humanity-terrain/src/{map_pack.rs, map_levels.rs, map_ground.rs, map_build/}, planet_chunks.rs (nested lattice, the formula), drawn_surface.rs, osm_region.rs (`region_from_pack`), src/engine/map_meshes.rs, src/engine/shore_probe.rs, src/gui/pages/maps/planet_view.rs
- Data: data/maps/packs/, data/maps/levels.ron, data/maps/places.ron; tests: map_ground_tests.rs (the waterfront and the old box edge at depths 13 and 20, tile-border vertices in the open Sound), map_pack_tests.rs, map_build's own, map_tiles_tests.rs `lattice_is_the_meshers_vertices`, drawn_surface_pack_tests.rs

### Whole-Earth Maps, Rung 2b: Road Beds, Building Pads, Full Detail in a Pack (2026-11-09)
The rung between 2 and 3 of docs/design/whole-earth-maps.md (as built: section 6.4; outcome in
section 13). ROAD BEDS: inside the one ground formula (crates/humanity-terrain/src/map_beds.rs, a
child of map_ground, before the shore band), each road piece's centreline profile (the land layer
every 2 m, averaged per class, held to the class's grade, pinned to the natural ground at both ends)
with a flat carriageway plus shoulder and 1 in 2 cut or fill back to natural; bridges, tunnels and
`layer` above 0 leave the ground alone. BUILDING PADS: the floor is the median of the land layer over
the footprint, held over it and a 1.5 m apron, then 1 in 2 back. Pinning each piece at its ends makes
a profile identical in every tile that reads it (the clip's crossing is the same point on both sides
of a tile border), so the pack format is unchanged. The coast wins within the shore band. A modifier
changes a patch only where wider than one vertex spacing, and never above depth 15. THE MESHER draws
each ribbon edge on its own ground plus a 4 cm decal offset (2 cm for a footway), every 6 m (4 m on service roads and footways), and
stands a building's floor on its pad with a plinth down to the natural ground: the 0.18 m and 0.12 m
lifts and the 2.5 m buried skirt are gone (kept only behind `HUMANITY_GROUND_BEDS=0`, the red run);
tunnels are hidden, bridges keep the draped ribbon until their deck. FULL DETAIL: an installed pack's
tile refines to patch depth 20 within 100 m of the camera with or without the tile tier
(ChunkParams::deep, levels.ron's `pack_detail`; measured: 2 to 8% more GPU time near Silverdale, where
the whole view cost 16 to 65%; the dev showcase keys `pack_depth` and `pack_within_m` compare settings
in one boot). THE GATE: src/engine/bed_probe.rs
(the `bed_probe` verb), scripts/lib/bed-probe-check.js (road_on_bed, building_on_pad), the vantages
silverdale-road-slope and silverdale-building-slope. Bench: examples/bench_map_patches.rs.
- Native: crates/humanity-terrain/src/{map_beds.rs, map_beds_tests.rs, map_ground.rs, map_levels.rs, osm_region.rs (`road_edge_radius`, `building_levels`, `MeshGround`), planet_chunks.rs (`DeepTiles`)}, src/engine/{map_meshes.rs (`DistrictGround`, `deep_tiles`), bed_probe.rs, ipc.rs}
- Data: data/maps/levels.ron's `beds` block; tests: map_beds_tests.rs (sloped streets flat, sloped footprints on their floor, a road across a tile border, coarse patches untouched, grades, the coast owning the waterfront street, each with a beds-off negative control), osm_region's mesher tests, scripts/tests/bed-probe-check.test.js

### The Sharp Converged Sky (v0.1165, phase 8)
The temporal cloud map is 2048^2 (edge-sharpness ceiling halved to
~0.11 deg/texel = 1.7 screen px; the v0.1161 ground-occlusion cull paid
for the 4x texels - pass measured 6.4 ms), and the MEDIUM quality tier
now arms and composites the same map instead of marching per-pixel at
full resolution (the quality-ladder inversion: Medium measured SLOWER
than High; its celestial pass dropped 21.5 to 7.0 ms). Orbit and
unarmed cases keep their existing paths.
- Native: src/renderer/cloud_temporal.rs (size), src/lib.rs (Medium gate), assets/shaders/pbr/40-clouds.wgsl (dispatch + map texel angle), 45-cloud-temporal.wgsl (jitter stratum), src/renderer/clouds.rs (size-angle lock)

### Clouds Move Like Weather (v0.1163, phase 7 stage 1)
Per-family winds replace the single solid-body drift (which moved every
family at every altitude at 127 m/s equatorial, 10-40x too fast for a
low deck). CloudRegime carries wind_lo/wind_hi per family (cirrus rides
the jet at 28-60 m/s, stratus ambles at 3-7); the carve and erosion
domains drift at the sample's own band height so tower tops outrun
their bases (wind-shear skew), and placement follows the family's base
wind (coherence: silhouettes never slide through interiors). Realistic
rates also make the temporal map's deep EMA sub-texel for standing
viewers - the motion fix and the smear fix are the same change.
- Native: assets/shaders/pbr/40-clouds.wgsl (CloudRegime winds, cloud_wind_omega, cloud_weather_adv), src/renderer/clouds.rs (mirror + regime_winds_follow_the_real_ladder)

### The Luminous Deck (v0.1162, phase 6)
The cloud lighting energy rework (fidelity findings 3/4/5/11): the
multi-scatter is the true Wrenninge/Schneider octave ladder (extinction,
energy, and phase-g each halve per octave) plus a two-stream diffusion
floor - conservative-scattering transmittance decays ALGEBRAICALLY, so a
thick overcast is luminous grey from below, never exponentially black.
Forward HG g raised 0.55 to 0.80 (droplet optics): backlit rims blaze
into real silver linings. Ambient is two-tone - sky-hued (from the
aerial sky colour, so dusk ambient warms and night darkens for free)
plus a warm ground bounce - giving warm-lit/blue-shadow chromatic
separation. Aerial perspective applies at the cloud's FIRST-HIT surface
and its opacity raises the fragment alpha, so far masses recede into
haze colour while keeping their silhouette.
- Native: assets/shaders/pbr/40-clouds.wgsl (scatter ladder, two-tone ambient, first-hit aerial), 90-fragment-main.wgsl (aerial_transmittance), src/renderer/clouds.rs (mirror + re-banded locks)

### The Band-Limited Sky (v0.1161, phase 5)
From the 5-agent realism audit (12 pixel-measured findings): both cloud
noise volumes carry variance-renormalized box-filter mip chains and
every march sample picks the mip matching its physical footprint (LOD
at all five volume tap sites + per-tap light-march LOD - distant clouds
read pre-averaged noise instead of aliasing). The temporal map now
accumulates PREMULTIPLIED colour: straight-alpha EMA darkened every
cloud by its own hit fraction twice and bilinear-dragged edges toward
black (the "grey gauze"; measured cores at alpha 0.32). The celestial
pass sun colour is the transmittance-tinted cur_sun (was a hardcoded
white since v0.639 - sunset now reddens clouds, terrain, water). Limb
fade gates to outside cameras (the horizon deck thickens, as real).
Octa-map rays that strike the planet before the slab are culled (61%
marched the ANTIPODAL slab through the Earth; pass cost measured down
to 1.9 ms) and out-of-disc texels skip.
- Native: src/renderer/cloud_noise.rs (mip_chain + renormalize), mod.rs (per-mip upload, tinted sun), assets/shaders/pbr/40-clouds.wgsl (phase 5 LOD block), 45-cloud-temporal.wgsl (premultiply, disc skip)
- Locks: wgsl_lod_offsets_match_the_tile_ladder, mip_chain_sizes_match_wgpu_and_preserve_the_mean

### The Converged Sky (v0.1160)
The v0.1159 adaptive EMA chased noise instead of averaging it (in a
noisy region the new sample ALWAYS disagrees with history, so the blend
stayed high exactly where convergence mattered - the operator: tiny
dots became big dots). Now a deep near-flat EMA (0.04, ~25-march
average per direction) with a whisper of catch-up: grain converges into
soft cumulus bodies; real change still lands within ~1.5 s.

### Clouds Stop Being Static (v0.1159, phase 4)
Temporal accumulation, Horizon/Nubis-class: a 1024x1024 direction-
indexed cloud map (Lambert azimuthal on the local up - no reprojection
matrix, no fold seams) re-marched each frame with the animated
golden-ratio jitter and EMA-blended (0.10 at rest, 0.6 on hard change).
The type-15 composite samples the map through the albedo slot - zero
bind-group-layout changes - and is NET FASTER than the direct march.
Active under/near the deck at High quality; the direct path froze its
jitter (animation without accumulation was the boiling static).
- Native: assets/shaders/pbr/45-cloud-temporal.wgsl,
  src/renderer/cloud_temporal.rs, pipeline.rs (octa PSO in hot-reload)

### The Physical Cloud Medium (v0.1158, phase 3)
The fidelity-expert six-finding rewrite: silhouette/density separation
(interior = full density, Nubis-style), physical per-family extinction
(cumulus 45/km, cirrus 1.2/km), a hard condensation base (band knee 3%),
convection-keyed towering, the 8 km cell-split tap (discrete 1-2 km
cumuli near the camera), aerial perspective at the cloud own depth, the
moire jitter fix, and the in-game weather owning the local sky (a
Cloudy/Rain HUD label now guarantees matching clouds). From below: a
field of discrete dark-based cumulus with sky gaps and a horizon depth
ramp. Fenced next: temporal reprojection (the grain unlock).
- Native: assets/shaders/pbr/40-clouds.wgsl, src/renderer/clouds.rs,
  src/lib.rs (weather floor), src/engine/ipc.rs (cloud_type pin)

### Clouds Read From Below (v0.1157, phase 2)
The from-below presence fix: extinction moved toward physical (2.6/km
High, 1.75/km Medium - phase 1 look-preserved the old value, which left
the short vertical under-deck path at tau under 1, the probe-bisected
invisible-deck failure). The deck now renders as real cloud from under
it and orbital decks go solid white like actual Blue Marble imagery.
Plus the cloud_type dev pin (params2.w = 2+tc) so cloud vantages
measure a KNOWN family.
- Native: assets/shaders/pbr/40-clouds.wgsl, src/engine/ipc.rs

### Physical Cloud Slab (v0.1156, phase 1)
The cloud deck now lives at REAL altitudes: per-planet cloud_base_km /
cloud_top_km in the RON (Earth 0.4-12 km - the old hardcoded fractions
put it at 25.5-76.5 km, a leftover of the deleted 4x terrain
exaggeration). A second per-material data vector (params2) carries the
slab + planet radius to the shader; the whole noise/extinction ladder is
metric now (km constants converted per invocation), the seven family
bands are re-authored to real altitude bands, Medium marches the same
physical slab (with its first rig vantage as the BUG-049 lock), and the
rig gained cloud_quality / cloud_cover showcase pins. Orbit look
verified with live MODIS. Phase 2 (from-below presence calibration on
the thin slab) is fenced in docs/design/clouds-depth.md.
- Native: crates/humanity-terrain/src/planet.rs (cloud_slab_scales), renderer pipeline
  params2, assets/shaders/pbr/40-clouds.wgsl, data/planets/earth.ron
- Dev: scripts/measure-cloud-depth.mjs, probe-sweep junction self-heal

### Tiled Lights Stay In Their Lane (v0.1155)
The other half of the night-glow report: with tiled lighting on, the
whole terrain lit up at night. Mechanism: the tile lists are built
once per frame from the lit interior pass, but the celestial/terrain
pass writes its camera uniform without lit_uniform, so its declared
light_count is 0 - the classic loop correctly gives terrain no point
lights, while the tiled path ignored light_count and smuggled the
interior lights onto the night terrain through the tile lists. Fix:
the tiled loop skips any index >= the pass declared light_count. A/B
locked by the silverdale-osm-night vantage, which now pins
lights_tiled ON: dark terrain, dim rain, and 15 -> 22 fps at that
vantage since terrain stopped evaluating phantom lights.
- Native: assets/shaders/pbr/90-fragment-main.wgsl (one guard)

### Night-Honest Rain (v0.1154)
Operator field report: the rain shader glows at night. The particle
billboard shader rendered every authored tint at full brightness
regardless of scene light (correct only in daylight). The particle
frame uniform now carries a scene-illumination scalar in its free pad
(Renderer::scene_illum: sun + fill luminance, clamped to a small
moon/ambient floor), and non-emissive particles (rain, snow, leaves,
dust) scale by it while emissive ones (sparks, embers) stay self-lit.
Night rain now reads as faint grey streaks. The wider night-glow
report is bisected to the tiled-lights setting (A/B proven, mechanism
hunt journaled) and stays open.
- Native: assets/shaders/particles.wgsl, src/renderer/mod.rs (scene_illum)

### Real Coastlines and Hills: Region DEM + Shore Taper (v0.1153)
Retired by whole-Earth maps rung 2 (2026-11-09): GEDTM30 in the packs and the shore band replaced the DEM and the taper.
Operator field report: the carved water edge was a sheer cliff and the
heightmap had no coastal gradients. Two-part fix. (1) SHORE TAPER: the
region mask now builds a distance-field beach profile (two chamfer
feature transforms): water reaches full carve depth only 90 m from
shore (0.7 m at the waterline), and land within 60 m of water blends
down toward its adjacent waterline (sea or lake, propagated per cell),
never raised. (2) REAL ELEVATION: scripts/fetch-region-dem.mjs pulls
public AWS Terrain Tiles (terrarium decode, minimal zero-dependency
PNG reader, ~13 m grid) into HOSDEM1 files beside each region;
inside coverage the drawn ground IS the survey data (edge-blended over
400 m; bathymetry spikes floored at -2 m since the carve owns
underwater), applied at the shared carve seam. The patches and the region
grids agreed from the start; the trees, the grass and the player's feet did
not until BUG-156 (2026-10-05): they stood on `DrawnPatchSurface`, whose copy
of the elevation formula had no carve, so beside the Dyes Inlet waterfront
trees hung about 37 m in the air and the player stood up to 40 m above the
drawn ground. The formula now lives in one function,
`planet_chunks::drawn_elevation_at_depth`, which all of them call. Locks: north-up orientation test +
shipped-file geography gates (Dyes Inlet <= 2 m, the SW ridge > 150 m:
an orientation flip would land the ridge in the inlet). Probe rig
hardened the same day: tree-kill + rig-path process sweep + EBUSY
retry (three zombie instances in one session).
- Native: crates/humanity-terrain/src/osm_region.rs (parse_dem/RegionDem), water_carve.rs (taper + dem_overlay), src/engine/region_meshes.rs (DEM load + lake levels)
- Generator: scripts/fetch-region-dem.mjs (HOSDEM1, 17 self-checks + geography gates)

### Vegetation Keeps Off the Streets (v0.1152)
The region mask gained a BUILT channel: building footprints filled,
road ribbons stroked at their real class width plus a 1.5 m curb strip,
never painted over water cells (a bridge keeps the carve beneath it).
All four vegetation streams gate on it in lockstep: the card bake, the
near-model mirror, and the far sheet reject candidates at bilinear
weight > 0.35 (canopy clearance), grass at > 0.6 (hugs the curb). The
gates take lat/lon in DEGREES directly (the streams generate candidates
as lat/lon; recomputing them from the dir halved in-region fps before
the A/B caught it) and a per-harvest disc pre-reject makes them free
outside any region. Probe-proven: Silverdale's streets and homes read
through the forest instead of under it, 29.8 fps vs the 30 fps ungated
baseline.
- Native: crates/humanity-terrain/src/water_carve.rs (CELL_BUILT + built_weight_at_deg + any_mask_in_disc), planet_chunks.rs, near_trees.rs, far_trees.rs, grass.rs

### Chat Live Strip + Streaming Revive (v0.1150)
The v0.855-0.857 broadcast pipeline (app-window capture, MJPEG over a
binary WebSocket: /ws/live/pub in-band-signed, /ws/live/sub/<name>,
GET /api/live directory) re-verified live (34 tests incl. the real-frame
e2e) and surfaced where the operator wanted it: a full-width EXPANDABLE
LIVE STRIP at the top of the Chat page. Collapsed: one row with your
on-air state and the server's live count. Expanded: Go Live / End Stream
/ Open Studio plus the live-now directory with one-click Watch. Fixes
the dead Chat Go Live (it never set broadcast_request, so it silently
broadcast nothing) and replaces four stale "rehearsal only" strings with
honest publisher-mirrored status (ON AIR viewers/kbps/dropped,
Connecting, or the verbatim error). Web /watch accepts ?u=<name> as an
alias of ?s=. Design + remaining increment work: docs/design/studio-watch.md.
- Native: src/gui/pages/chat.rs (draw_live_strip), studio.rs, watch.rs (helpers shared)
- Web: web/pages/watch.html (?u= alias)

### Quests/Tasks Split (v0.1145)
Quests = gameplay (sim quests, auto-tracked, native-only). Tasks = real
life: the kanban plus the learn-by-doing tutorial chains, since 2026-10-30
as suggested cards at the top of the Todo column (see "Suggested cards on
the Tasks page"; until then a collapsible guide panel beside the board). Web
nav entry renamed Onboarding; /quests redirects to /onboarding.
- Native: src/gui/pages/quests.rs, tasks.rs (draw_suggestion). `onboarding.rs` (the chains' old
  drawing and loading) was removed 2026-10-30: the Tasks page reads the chains itself.

### Quests: the active card and the chain window (2026-10-30)
Operator: "when we click on that quest, I want it to like popup all the quests in the chain.
We'd want two modes." A quest under way is a card: "Quest 1 of the First Steps chain", its
name, "Step 2 of 8" over a meter with a segment per step, the step it is on as the card's focus
("Now", on a faint accent ground), and its rewards. Several under way sit side by side;
Available and Completed sit in a column beside them (stacked when narrow). A click on any quest
(under way, available or finished) opens its CHAIN: every quest joined to it by `prerequisite`,
top to bottom, quests needing the same one drawn as branches off it (finishing it starts them
all, so none locks another). Normal play shows only what the player has reached: the finished
steps and the current one, "6 more steps to discover", "More quests to discover"; the steps
ahead and the quests not unlocked are left out of the data itself (`Reveal::Learned`), not just
hidden. The Dev play mode with dev cheats on (`dev_cheats_active`) shows every quest and step,
the ones not reached dimmed. The chain is built only while its window is open on the Quests
page. Web: the /onboarding guides (the web's Quests target) became the same cards, each opening
its steps in a window (real-life guides: every step shows, the ones after the next dimmed).
- Native: src/gui/pages/quests.rs, src/gui/pages/quests/chain_window.rs, src/gui/quest_types.rs,
  crates/humanity-systems/src/quests/chain.rs (the view and its no-spoiler tests); lib.rs quest mirror
- Web: web/pages/onboarding.html
- Snapshots: quests, quests_branches, quests_chain_first_steps, quests_chain_branches, quests_chain_dev

### Market Directory (web mirror, v0.1143)
The web market page opens on a Directory section mirroring the native tab:
offerings browse (search + category filter), providers with storefront
filtering, detail modal with copyable provider key, same latest-revision +
TTL rules. CBOR decode via `decodeCanonicalCbor` (decoder extended with
floats/negatives/bools; encoder untouched, byte-lock proven by
`scripts/group-object-kat.mjs`).
- Web: `web/pages/market-app.js`, `web/pages/market.html`, `web/shared/canonical-cbor.js`

### Crafting Page
Recipe browsing and crafting UI. A hand craft takes its inputs from the
backpack first, then home storage, then the tanks for tap water, wherever the
player is aboard with their own home on the ship (BUG-147, 2026-10-04): the
Ingredients card shows each input's split ("3 in backpack + 40 in home
storage / needs 45") and says plainly what is short. What the backpack cannot
take of the result goes to home storage with its grade; the rover, truck and
Nova still roll out as vehicles. On a planet, in open space and as a guest on
a shared ship, only the backpack counts, and the page says why.
- Native: `src/gui/pages/crafting.rs`, `crates/humanity-systems/src/crafting/home_store.rs`
  (`HomeStore`, `Draw`), `src/engine/built_uses.rs` (`home_storage_here`)

### Guilds Page
Guild management in native UI.
- Native: `src/gui/pages/guilds.rs`

### Trade Page
P2P trading interface. Since v0.1433.0 a trade moves real items: an offer line
names an item you carry (by name or id), Confirm waits until you carry the whole
offer, and when the trade completes each player's game hands over its side, with
the item's wear and grade (`trade::settle_completed`). The web page's offers are
text and do not move.
- Native: `src/gui/pages/trade.rs`, types in `src/gui/trade_types.rs`

### Studio Page
Content creation / streaming rehearsal tools: scenes, sources, resolution/bitrate/FPS,
chat overlay. Real mic-level meter (v0.658, `crate::net::voice::mic_level()` -- reads
0 unless a mic test or live voice session is actually capturing, matching the page's
"rehearsal only, no real transport yet" honesty fix from v0.652.0). First-ever adoption
of the built-but-previously-unused help_modal system (4 topics: scenes/sources, stream
settings, chat overlay, program/preview) -- see the Help Modal entry below.
OBS-style Program/Preview split (v0.664): clicking a scene stages it into PREVIEW only;
a "Cut to Program" transition button deliberately pushes it live. Source editing always
targets the preview working set while PROGRAM renders a frozen snapshot from the last
cut, so a streamer can rearrange safely mid-broadcast. Center canvas is two panes side
by side (Program left / Preview right) when wide, one pane + toggle when narrow. Scene
list marks PGM/PRE. State + transitions live in `GuiState.studio` (`StudioState::
select_preview_scene` / `cut_to_program`, unit-tested; persists across page switches).
Headless snapshot: `just snapshot studio`.
- Native: `src/gui/pages/studio.rs`

### Clip Maker (movie mode)
Films the game for social media (v0.1435.0). `just clips` boots the release build in
the background rig, flies to each shot in `scripts/clips.json` (a planet vantage or a
place aboard the home, with weather, sea state and hour), and records it in the engine's
movie mode: every frame advances time by exactly 1/fps and goes straight into ffmpeg, so
the video is smooth however slowly frames render, and the animation clock the water,
clouds and wind read is slid to match. Camera motions: hold, an eased pan, a keyframed
dolly through the home, and an exponential drop from orbit toward the ground. Each shot
comes out as 1920x1080, 1080x1920 (a centre crop for now), a still and the full-size
master, with a clips.md of suggested first lines, in Videos\HumanityOS clips.
- Native: `src/engine/movie.rs` (`debug/record_request.json`), `src/renderer/capture.rs`
  (`capture_current_frame_rgba`)
- Script + shots: `scripts/make-clips.js`, `scripts/clips.json`

### Civilization Page
Community stats dashboard.
- Native: `src/gui/pages/civilization.rs`

### Calculator Page
Calculator in native UI. Two modes since 2026-11-13: Arithmetic (the keypad and history) and
Pulleys (below).
- Native: `src/gui/pages/calculator.rs`

### Pulleys: the block-and-tackle designer (native, rung 1, 2026-11-13)
The Calculator page's Pulleys mode (docs/design/pulleys.md). Give it the load, the lift height,
who pulls (one person, four people, a truck's winch, a ship's winch, the hangar hoist), the rope
(manila, 6x19 wire) and the sheave bearings (plain, bronze, roller); it searches about 2,800
plans of up to four tackles stacked block on block and picks the one with the fewest stages, then
the fewest sheaves, then the least rope pulled. It shows each tackle's parts, sheaves, rope and
sheave diameters and block length, the real and ideal advantage, the pull, the rope pulled (ideal
advantage x lift), the time, the rope and block masses, the trade in words, advice when a geared
hoist, more hands or a winch's first-layer rule matters, and a schematic drawn from the same
numbers. Realistic mode: friction compounding per sheave (Crosby's published ratios within 2
percent), the blocks' and rope's own weight, and a rope over its working load limit refused with
the numbers; Simplified mode: the ideal machine. Every block dimension (sheave, pin, side plates,
partitions, shackle, mass) is derived from the rope and the load, with a mesh (boxes, lathed
sheaves and pins, parallel-transported rope and shackle tubes) and collision boxes and capsules from
the same numbers, built once per configuration under the reuse switch. No web mirror yet (rung 2).
- Model: `crates/humanity-systems/src/pulley/` (`tackle.rs` the physics and `recommend`,
  `geometry.rs` the sizing, mesh, collision and `ShapeStore`, `data.rs` the rows, `tests.rs`)
- Data: `data/pulleys/pulleys.ron` (pullers, ropes, bearings, block rules, geared hoists, each with
  its source); finding `docs/reference/findings/2026-11-13-pulleys-and-rigging.md`
- Native: `src/gui/pages/calculator/pulleys.rs`; snapshot `just snapshot calculator_pulleys`
- Library: "Force, Levers and Mechanical Advantage" (`docs/user/skills/`) describes it

### Calendar Page
Calendar/planner in native UI.
- Native: `src/gui/pages/calendar.rs`

### Notes Page
Notes/journal in native UI.
- Native: `src/gui/pages/notes.rs`

### The data/ browser (the HumanityOS folder place of the Files page since 2026-10-19)
The developer's browser and editor for `data/` was its own Files page (Platform > Files) until
2026-10-19, when it became the Files page's "HumanityOS folder" place (see "Files Page, formerly the
Media Page"), starting one folder up so assets/ and mods/ show too. Dev tooling: kept, nothing lost.
A Library guide in `data/library/` opens in the Library from there (2026-10-20, "Open in the Library").
- Native: `src/gui/pages/files/app_folder.rs`
- Web: `web/pages/files-folder.js`, the place "The server's data folder" (the server's data/ through `/api/data-files`, signed POSTs for the server's admins only since BUG-206), a place of `/files` shown to admins alone

### Resources Page
Resources directory in native UI.
- Native: `src/gui/pages/resources.rs`

### Bugs Page
Bug reporting/tracking.
- Native: `src/gui/pages/bugs.rs`

### Donate Page
Donation page. As of v0.659 native fetches the CONNECTED server's real funding info
(GET /api/server-info `funding.addresses` + `goal_usd`/`goal_label`) on connect --
previously only the web client did this and native's list stayed empty unless a
self-hosting operator hand-typed addresses into Settings. Server-fetched funding
lives in `donate_addresses_server` (never persisted, so it can't clobber the
operator's local Settings list), is cleared/discarded whenever the connected server
changes (money-routing data: server A's addresses must never display as server B's
-- `GuiState::apply_server_funding` + tests), and the old hardcoded fake
"$350 / $1000" progress bar is replaced by a card showing the server's REAL goal
only when one exists. Preference order on the page: server list > local Settings
list > the two legacy single-address Settings fields. Since 2026-10-04 the page
leads with the giving routes (`data/donate/routes.json`, see "Donation Page"
above), and the old fallback that offered an address derived from the VIEWER'S
OWN key as the place to send money is gone.
- Native: `src/gui/pages/donate.rs` (`build_donation_sources`), `src/gui/mod.rs` (`ServerInfo.funding`, `DonateAddress::from_funding_json`, `GuiState::apply_server_funding`), `src/lib.rs` (connect-time fetch in the `peer_list` handler + per-frame drain)

---

## Game Engine

### Three-Mode Camera
First-person, third-person, orbit/free with smooth transitions.
- Native: `src/renderer/camera.rs`

### wgpu Renderer
PBR-lite rendering with depth buffer, materials, instanced rendering.
- Native: `src/renderer/mod.rs`, `src/renderer/pipeline.rs`

### Startup: the clouds compile in the background (v0.1517.0)
The two pipelines that bake the volumetric cloud march (the cloud shell's transparent PSO and the near-field screen march) were the two slowest compiles of the twenty, and the boot waited for them alone. They now compile on a background thread started beside the other eighteen, one step above normal priority, and are installed on the first frame after they finish (`Pipeline::poll_clouds`, called from the frame loop). Until then the frame draws no clouds: `transparent_for(Cloud)` and `cloud_screen_pipeline()` answer `None`, the draw loops skip the cloud shell, and the near-field cloud passes and the composite do not run. A shader hot reload still builds them inline (the A/B rig captures on its log line) and supersedes a background build still running (`renderer::deferred_build`, tested both ways round, seen red). No cache: nothing is kept between boots.
- Measured on `scripts/boot-timing.js`, 3 boots per arm on the same quiet machine the same day (2026-10-19, i7-8700K + RTX 4070, DX12/DXC): to playable median 13.3 s for the v0.1516.0 archive (12.9 to 13.4), 8.7 s with this (8.6 to 8.9); `pipeline_new` 8.3 s to 2.9 s. The clouds compile in 8.3 s and land a median 2.2 s after playable on an autopilot entry that skips the menu; the boot timer reports them as BACKGROUND spans (`cloud_psos_compile`, `clouds_ready_after_open`), never as part of the time to playable. `[Pipelines] clouds ready after N s, in the background` in run.log says when.
- `scripts/probe-sweep.js` waits for that line before its first capture.
- Every PSO compile runs on a thread with a 64 MiB stack (a reservation), and none on the main thread. An exe with no `dxcompiler.dll` + `dxil.dll` beside it compiles with FXC, and FXC overflows its thread stack on the Cloud Transparent PSO about 8.4 minutes in whatever the size (64 MiB and 1 GiB both, 2026-10-18), which aborts the process; so an FXC run builds everything else (the world in about 3 minutes) and leaves the clouds OFF, saying so once in run.log. `HUMANITY_FXC_CLOUDS=1` tries the cloud pair under FXC anyway (dev knob).
- Native: `src/renderer/pipeline.rs` (`CloudPsos`, `spawn_cloud_build`, `poll_clouds`), `src/renderer/deferred_build.rs`, `src/renderer/celestial.rs`, `src/renderer/scene_draw.rs`, `crates/humanity-core/src/boot_timing.rs` (background spans)

### Startup: the planet surfaces are prebuilt at boot (v0.1517.0)
Reading `data/planets/*.ron`, loading or synthesizing each elevation grid, loading each colour grid and baking each surface texture (about 1.2 s) used to run on world entry. It is CPU work on files, so a boot thread does it beside the star-sky prebuild (`net_route::bake_planet_defs`), and the first world entry only installs the result and uploads the textures (`install_planet_defs`, about 60 ms). A planet RON hot reload before world entry drops the prebuild so a stale one is never installed; later entries and hot reloads bake on the main thread as before (`reload_planet_defs`). Measured the same day as the cloud entry above, 3 boots: `planet_defs_bake` 1.22 s to 0.62 s on an autopilot entry that skips the menu (it waits out the rest of the 1.29 s prebuild, reported as the BACKGROUND span `planet_defs_prebuild`), and to playable 8.7 s to 8.4 s (8.3 to 8.6). With anyone on the menu for over a second, the wait is gone.
- Native: `src/engine/net_route.rs` (`bake_planet_defs`, `install_planet_defs`, `reload_planet_defs`), `src/engine/world_load.rs`, `src/engine/state.rs` (`planet_defs_preload_rx`), `src/lib.rs` (the boot thread)

### Room GI: per-room irradiance probes (rung 1, 2026-09-27)
Indirect light inside the ship. Each detected room gets its own lattice of DDGI irradiance probes (Majercik et al., JCGT 2019), about 1 m apart, updated on the GPU every frame: 64 rotated rays per probe traced against the room's own box, each hit lit by that room's lights with exactly the fragment loop's attenuation, by the sun where it comes in through a glass roof, and by the previous frame's probes for every further bounce. A fragment reads only its own room's probes (DDGI's 8-probe sample with backface weights), so one room's lamp never leaks through a wall into the next. Before this the whole indirect term indoors was a 0.005 floor: at `25b-mushroom-racks` a wall no lamp faced read sRGB 4 beside a floor of 146; it now reads 46 (wall/floor 0.17 before the tone map, where interreflection predicts 0.24 to 0.31 for a 10 x 10 x 3 m room), and the shelf undersides at `25c-oyster-rack-close` go from 2 to 76. Screens and see-through surfaces (panes, tent film) keep the old floor. Cost at `console-face-6` about 0.6 ms (update 0.21, sampling about 0.4). Storage is DDGI's format (8x8 octahedral irradiance plus 16x16 depth moments per probe in one atlas), so rung 2 (tracing each room's contents, with DDGI's visibility test switched on per room) changes only the intersection. Dev switches `showcase {"room_gi":"0"}` (A/B, the old floor) and `{"room_gi_vis":"1"}` (force the visibility test on); GPU timer `gpu.room_probes`.
- Native: `src/renderer/room_probes.rs` (layout, math, CPU twin + tests), `src/renderer/room_probes_gpu.rs` (atlas, room table, compute update), `src/engine/room_gi.rs` (rooms to probe boxes from `data/blueprints/wall_materials.ron`), `assets/shaders/pbr/85-room-gi.wgsl` (the sample, behind the `HAS_ROOM_GI` feature switch in `frag_tail`), `assets/shaders/room_probes_update.wgsl`
- **Bounced light detail (2026-10-20):** Settings > Graphics > Light and sky picks how finely the probes are laid out, live, no restart: Low, Medium, High (the default, the design's 1 m and 32,768 probes) and Ultra, each shown with its probe cap and its video memory (the cap x 3,392 bytes, about 27 to 212 MB). The frame's work is the same at every level. The page names the graphics card and its own memory (read from its driver's install record on Windows) and warns when a level would take more than a twentieth of it. The levels are data (`data/gui/bounced_light_detail.json`); the setting persists (`bounced_light_detail` in config.json); dev request `showcase {"bounced_light_detail":"low"}`. Native: `src/renderer/bounced_light_detail.rs` (the levels, their memory and words, the card's memory), `room_probes_gpu.rs` (`RoomGi::set_budget`, the atlas resize), `room_probes.rs` (`ProbeBudget`, `lattice_for`), `engine/room_gi.rs` (applies it each frame), `gui/pages/settings/graphics.rs` (`draw_bounced_light_detail`). Design: `docs/design/room-gi.md`.
- Rigs: `node scripts/photograph-home.js --ab room_gi`; vantages `25c-oyster-rack-close` (home) and `console-face-6-room-gi-off` (probe sweep)
- Design: `docs/design/room-gi.md` (destination, rung 1 as built, measured, limits, next)

### Shadows from room lights, people and furniture (rung 1, 2026-11-10)
Before this the room lights cast no shadows at all and the near world (the homes, furniture, every person's body) was never drawn into the sun's shadow map, which only the celestial list fills (measured from the rendered pixels: the design doc, section 1). One depth ATLAS a frame, drawn before the celestial pass: the room lights that matter most where you look (brightness times the light each puts at the eye and 2, 4 and 8 m ahead of it, up to a budget: 8 on Full, 4 on Simplified) each get six cube faces (a spot light one view), sized by rank and packed largest first in Morton order; a face with nothing in it, or that nothing in view can look up, gets no tile. Each lamp's shadow reaches only as far as its light stays above a twentieth of its value a metre away (5.2 m for the homestead's 15 m panels), fading out over the last fifth, so a lamp's shadow work stays in the room it lights. Plain opaque casters draw through a FAST PATH (2026-11-11): sorted by mesh, one instanced draw a run with each caster's matrix as an instance attribute, which (with one upload for every view's camera) cut the CPU cost of the ground hall's shadows from 3.4 ms to about 1.7 to 2.0 ms (1.0 in the mess hall); what sways or cuts out keeps the classic path. Measured costs, and the scene pass's share that is paid even at Off until rung 1b: the design doc's section 10. The NEAR SUN VIEW, an orthographic tile around you along the sun that lights the scene (the hull-frame sun aboard), shades the sunlight through a glass roof and outdoors on a planet, multiplied into every sun term after the far map, so the terrain under you and the floor under a chair both get it. Every opaque caster: structure, furniture and model packs, the compute-skinned near bodies (a bounding sphere per mesh, `Mesh::bounds`), card hair and cluster cards through `fs_shadow`, the middle tier and the first-person body's UNFOLDED twin through two skinned shadow PSOs (your shadow keeps the head the eye camera's copy folds away). A surface that gives off light (a lamp's bulb, a screen) never casts. 16 bilinear comparisons on a Vogel disk (Full) or 4 (Simplified), normal-offset bias in world units. Line lights (strips) are not shadowed yet (rung 3). Nothing is kept across frames except which lamps were shadowed (a 1.3x preference against flicker), which obeys the reuse switch.
- Native: `src/renderer/local_shadows.rs` (levels, light choice, views, packing, culling, table, the pass), group 0 bindings 8-10 (`pipeline::camera_bind_group`), `assets/shaders/pbr/00-bindings-vertex.wgsl` (`shadow_atlas_view`, `local_light_visibility`, `near_sun_visibility`) and the light loop in `80-fragment-shared.wgsl`, behind the feature switch `HAS_SHADOW_ATLAS` (surface, vegetation, terrain); `renderer/skinning.rs` (`skin_shadow_only`, `draw_skinned_shadow`); `engine/animation.rs` (the first-person twin).
- Settings > Graphics > Light and sky > **Room light shadows** (Off, Simplified, Full; levels in `data/gui/shadow_levels.json`): each level's lamps, face size, near-sun texel and video memory worked out beside it, the measured cost (`gpu.shadow_local`, `gpu.shadow_near`, `cpu.shadow_local`, `cpu.shadow_near`, `vram.shadow_atlas`), the card's memory against the level, and how many lamps cast shadows this frame. Sun shadows also switches the near sun view.
- Rung 1b (2026-11-12): Off is free (while the level is Off the surface, vegetation and terrain PSOs are twins with the atlas lookups compiled out, `pipeline::ATLAS_TWINS`, built in the background when the level crosses Off; the Settings line says "Off costs nothing in the scene pass", or that the shading is still rebuilding), and a caster is drawn into a lamp's face only if its shadow can reach the view (the shadow-caster cull, `local_shadows::shadow_reaches_view`). `scripts/home-shadow-metrics.mjs --gate` judges the shelves and the hut by the light they keep with and without the shadows.
- Showcase: `local_shadows` ("0", a level id, "auto"), `near_levels` ("0", "auto"), `fp_shadow_twin` ("0" drops the first-person twin), `shadow_fast` ("0" draws every caster the classic way), `shadow_cull` ("0" draws every caster in a face, no shadow-caster cull), `atlas_code` ("1" keeps the lookups compiled in at Off); vantages `shadow-racks-night` (the picture gate: one lamp over the racks), `shadow-hall-chair-night`, `shadow-racks-noon`, `shadow-planet-third`, `shadow-fp-racks-noon` and their `-off` red twins; scored by `node scripts/shadow-metrics.mjs`.
- Design: `docs/design/local-light-shadows.md` (what cast what before, the 2030 plan, budgets, rungs, rung 1 measured)

### Environment regions (plumbing, 2026-09-21)
Positioned, sized environmental effects that every consumer reads the same way, instead of each one scavenging a spare lane in a uniform. A region carries a planet-fixed direction, a great-circle angular radius, a kind, an intensity and a soft edge; influence at a point is a smooth falloff in angle from the region centre. Uncapped storage buffer of 48-byte records at group 0 binding 4, the same shape v0.782 gave scene lights at binding 1, so adding a new kind of effect costs a row in a data file rather than a hunt for a free float. Kinds are data (`data/environment/region_kinds.ron`: storm, rain, snow, cloudy, fog, wildfire, sandstorm, each with a default radius in km, an edge softness, an altitude band and its cloud floor/placement weights); radii are stored in km and converted to an angle per body, so the same table works on a moon. The CPU falloff and the WGSL one are a locked twin, pinned by a test that reads the shipped shader text and checks the struct field order and the binding number.
THE RULE it exists to enforce: an environmental effect's weight at a point is a function of THAT POINT, never of the camera. BUG-080 was a weather term faded by camera altitude.
- Native: `src/renderer/env_regions.rs`, `assets/shaders/pbr/00-bindings-vertex.wgsl`, layout in `src/renderer/pipeline.rs`, upload `Renderer::set_env_regions`
- Data: `data/environment/region_kinds.ron`
- Design: `docs/design/environment-fields.md` (includes the inventory of what else should be regional: wind, temperature, humidity, fog, pressure, snow cover, outbreaks, pollution, aurora, light pollution, ocean currents)
NOT YET CONSUMED: the buffer is built, bound and uploadable, and nothing reads it yet. Increment 2 feeds `disasters.rs` (which already stores position/radius/intensity and is read by nothing) through it; increment 3 moves the weather condition onto it and deletes the altitude ramp that causes BUG-080.

### Aurora (2026-09-21)
Auroral ovals around both poles, drawn as real emission inside the air rather than as a decal on a sphere. Two environment regions of band 2, one per pole, carrying the oval geometry in their payload: inner and outer ring angle (the real oval sits 20 to 25 degrees from the pole) and the emitting altitude band. The atmosphere fragment solves the view ray against that layer and integrates along the crossing, so limb brightening falls out for free: a bright arc where the view grazes the layer, a faint glow looking straight down, from one emissivity. Gated to ground in darkness with a soft terminator edge, coloured 557.7 nm green low down fading to 630 nm red at the top, with broad folds and finer vertical rays that lean with height. Intensity is the region's, so solar activity has somewhere to live.
It is the SECOND consumer of the environment region buffer and was the test of whether that mechanism generalises: an aurora shares nothing with a storm except having a place and a size, and it needed no new uniform channel and no new binding.
Since 2026-09-27 it is drawn by its own FULLSCREEN ADDITIVE EMISSION PASS rather than inside the atmosphere shell (PRIORITIES 1b): after the cloud composite for a camera above the emitting layer's floor, so the 12 km deck no longer paints over emission that sits 99 to 190 km above it, and before the transparent list for a camera below the floor, so an overcast sky covers it from the ground. Rays are analytic and clipped at the planet and the scene depth; the layer's top is data, no longer capped by the shell mesh; and the aurora is visible from the ground and from aircraft for the first time (it was invisible below 99 km). Dev switch `showcase {"aurora":"0"}` for A/B, GPU timer `gpu.aurora`.
- Native: `assets/shaders/pbr/30-atmosphere.wgsl` (`aurora_emission`), `assets/shaders/pbr/95-emission-pass.wgsl` (`fs_emission_pass`), `src/renderer/emission_pass.rs` (placement + marker), regions emitted in `src/engine/frame_shells.rs`
- Measure: `scripts/aurora-gate.js` (green-excess, and the linear light the aurora delivers against an aurora-OFF twin)
- Data: `data/environment/region_kinds.ron` kind `aurora` (ring angles, altitude band, size, softness)
- Fixtures: `aurora-polar-1500` (close over the night-side oval) and `aurora-orbit-5000` (the whole oval on the dark limb); the deck gate `aurora-over-land` / `aurora-over-water` with their clouds-off and `-dark` (aurora-off) twins; from below, `aurora-under-deck` / `aurora-under-clear` and `aurora-ground-horizon`
- Design: `docs/design/environment-fields.md`

### Environment Layer 1: each world's climate (2026-09-27)
The base field under all weather: the air temperature, air pressure and prevailing wind at any place and date on a world, a pure function of the direction from the body centre, the altitude, the share of land around the place and the time of year. Temperature is North, Cahalan and Coakley's energy-balance form (a P2 profile plus a seasonal term that flips between hemispheres) with separate land and sea seasons per hemisphere; pressure and the lapse are the 1976 US Standard Atmosphere's two lowest layers, started from the place's own sea-level temperature; the wind is the zonal-mean 10 m wind (trades, westerlies, Antarctic easterlies, and their seasonal shift) as a 16-term sine series. Earth's numbers are fitted to the NCEP/NCAR Reanalysis 1 1991-2020 means by a checked-in script; Mars carries its NASA fact sheet column; a world without a row keeps the generic body model. The weather is now a DEVIATION on top of it: the at-player temperature, pressure and wind the body heat model reads come from Layer 1 where the player stands, so the equator, a mountain and the pole feel different air. Implemented twice, f64 for the simulation and f32 WGSL for the renderer, and a test RUNS the shader functions on the CPU (a naga-IR interpreter) over about 2,000 places and dates against the Rust copy. No GPU caller yet: cloud advection is the first.
- Native: `crates/humanity-systems/src/env_layer1.rs` (model; tests in `env_layer1_tests.rs`), `assets/shaders/pbr/00-bindings-vertex.wgsl` (`EnvClimate`, `env_l1_*`), `crates/humanity-systems/src/weather.rs` (`air_at_player`, the at-player export), `src/engine/survival_env.rs` (feeds the body heat model), `src/engine/frame_lock.rs` (direction + land share), `src/renderer/shader_loader/wgsl_eval.rs` (the test-only WGSL interpreter, reusable for any pure shader twin)
- Data: `data/environment/climate.ron` (one row per world, sources beside every number); `node scripts/climate-fit.js` reproduces Earth's row
- Design: `docs/design/environment-fields.md`, "Layer 1 as built"
### In-world screens (rungs 1 and 2, 2026-09-16)
Any native egui tool page (inventory, tasks, chat, library, the MJPEG Watch page, ...) renders onto a flat display placed in the 3D world, and the player can look at it, click it, scroll it and type into it. It is the real page function drawn against the real `GuiState` (the wall inventory IS the player's inventory), under a second egui context per screen so scroll and open states are per screen. The scene samples the page's texture directly through a type-24 emitter material (no readback, no copy; sun and shadow do not darken a display, aerial haze still applies). Input: the camera look ray while the cursor is grabbed, the cursor ray while it is free, 3.5 m reach, click consumption (the game's own click skips a frame a screen took), keyboard focus on the last-clicked screen while its page wants text, Escape releases. Data: a `screen` block on a machine def (`page`, `px`, `face`, `bezel_m`, `brightness`) plus a per-instance `screen_page`; two `wall_screen` instances ship in the common room. Dev IPC: `debug/screen_request.json` (hover / click / scroll / text / snapshot through the same event API) and `debug/screen_done.json`. Every plain tool page now goes through ONE dispatch table shared by the main UI and the screens. Design doc: `docs/design/in-world-screens.md`.
- Native: `src/gui/dispatch.rs`, `src/gui/screen_surface.rs`, `src/engine/screens.rs`, `src/engine/ipc.rs` (`poll_screen_request`), `src/renderer/materials.rs` (`MATERIAL_TYPE_SCREEN`, `add_material_with_albedo_view`, `update_material_albedo_pixels`), `assets/shaders/pbr/90-fragment-main.wgsl` (type 24), `crates/humanity-ship/src/machines.rs` (`ScreenDef`)
- Web: none by design. The screens are surfaces inside the 3D world the native renderer draws; the website has no world to place them in.
- Data: `data/machines/home.ron`, `data/machines/home_solo.ron` ("Displays" category: `wall_screen`, `desk_monitor`)

### Live feeds and in-game cameras on screens (in-world screens rung 3, 2026-09-16)
Two more things a wall screen can show, both chosen from the data file alone. `watch:<stream id>` shows the relay's MJPEG live stream (the stream id is the publisher's registered name, lower case): each screen owns its OWN `LiveViewer`, started on its first frame against the configured server and dropped with the surface, so two walls on one stream get every frame instead of splitting them; frames are letterboxed into the surface's own size (never stretched), a status page covers "connecting" and "stream offline" with the relay's reason, and an ended stream is retried every 15 s. `camera:<instance id>` shows the game world from a placed CAMERA POST: a machine def with `camera: Some((yaw, pitch, fov))` (the instance's rotation adds to the yaw), resolved to a lens pose on every placement rebuild and move. The world is rendered straight into the screen's surface (created in the scene's format) through `render_view_onto`, the sky + scene passes refactored out of the hi-res screenshot so both share one function; at most one camera renders per frame and each at most 10 times a second (starvation-order round-robin), sky and scene passes only (no planet/cloud pass: its temporal history belongs to the player's camera). Ships as `camera_post` in both home catalogs, `camera_post_1` in the garden looking down the tower rows, and two console-room wall screens (`wall_screen_3` = the garden camera, `wall_screen_4` = `watch:shaostoul`). The dev IPC done file now carries the provider's status fields (`connected`/`frames`, `renders`/`live`/`error`). Design doc: `docs/design/in-world-screens.md`, "Live and camera sources".
- Native: `src/engine/screens/live.rs` (`LiveProvider`, `letterbox_rect`, `letterbox_into`), `src/engine/screens/camera.rs` (`CameraProvider`, `pick_due`, `camera_from_pose`, `EngineWorld`), `src/gui/screen_surface.rs` (`WorldRender`, `WorldRenderState`, `surface_format`, `new_with_format`), `src/engine/screens.rs` (`camera_posts_from`, the scheduler in `frame_surfaces`), `src/engine/ipc.rs` (`render_view_onto`, `ViewPasses`, the status merge), `crates/humanity-ship/src/machines.rs` (`MachineDef.camera`, `CameraPose`, `PlacedMachine::camera_pose`)
- Web: none by design (the screens live in the 3D world the native renderer draws).
- Data: `data/machines/home.ron`, `data/machines/home_solo.ron` (`camera_post`)

### A live screen names its own wall, and a test publisher to prove it (2026-09-18)
`live::live_status(server, stream, last_error)` is one pure decision returning both the words and whether to connect, so a `watch:` screen with no server set says "No server set" and where to set one and opens NO socket (it used to compose a host-less URL and retry "No host name in the URL" forever, which the operator photographed in the console room), a name nobody is publishing says "Stream offline / No one is streaming as ... Trying again shortly.", and a real socket failure keeps its reason; the case is chosen on `net::live_viewer::EndReason` rather than by matching English. Two defects the new rig caught and fixed: a stopped publisher left every viewer hanging on a dead broadcast channel (teardown now sends a zero-length sentinel frame and `viewer_loop` closes on it), and a memoized status page could not be searched by the dev IPC's `find` (it now runs when `core.find_pending()`). `scripts/live-publish.js` publishes a moving test pattern to a LOCAL relay (it refuses a non-loopback server without `--allow-remote`), authenticating exactly as the native publisher does and claiming its stream name through the ordinary identify challenge; `scripts/verify-live-screen.js` starts its own relay, publishes, and proves the wall live (99.4% of pixels changed in a second against a 0.00% static control), then proves both messages, with the no-socket claim read out of the log rather than taken on trust.
- Native: `src/engine/screens/live.rs` (`live_status`, `LiveStatus`, `LastError`, `server_has_host`), `src/net/live_viewer.rs` (`EndReason`), `src/relay/live.rs` (`END_OF_STREAM`)
- Scripts: `scripts/live-publish.js`, `scripts/verify-live-screen.js`
- Web: none by design (the screens live in the 3D world the native renderer draws).
- Docs: `docs/design/in-world-screens.md` ("What the wall says when it cannot show a stream", "The live-screen rig")

### Aim assist, screens of any size, build-mode handles and each screen's cog (in-world screens, 2026-11-06)
The operator's ask: a proximity click for in-game screen buttons with a line from the crosshair, screens of any width and height (a 100" 16:9 or a 200" 32:9 TV), edge handles and a move area in build mode, and a cog for each screen's settings. AIM ASSIST: while looking near a screen, the nearest clickable widget within a cone of the crosshair (3 degrees by default, Settings > Controls, on by default) is outlined in the accent colour, a thin line runs to it from the crosshair, and a click goes to it. The widgets are egui's own record of the screen's last pass (sensing a click and taking focus, so selectable text is not offered; enabled; not clipped away; on top at its centre), measured as ANGLES from the look ray so a small button on a far screen is as easy as a big one near by; a ray just off the picture still reaches a widget at its edge; hysteresis (keep while within 1.25 x the cone, switch only below 0.7 x the current angle) stops the choice flickering between neighbours; a held press moves the pointer only by the ray's movement so a click stays a click. A screen's reach grows with its diagonal (3.5 m floor, 1.75 m per metre so the shipped 79-inch wall screen keeps 3.5 m, 25 m cap). ANY SIZE: a placed screen keeps its picture size in metres (`screen_size_m`, beside `screen_source`, written only when set); the body is the picture plus the bezel, and the surface keeps the def's pixel height with the width from the aspect (32:9 is 2560 x 720) under a 1920 x 1080-pixel cap. BUILD MODE: on the screen under the cursor or selected, an edge drag resizes (the opposite edge stays), a corner keeps the aspect (Shift frees it), the middle slides it along its own wall, the grid snap steps 5 cm, and the cog near the top-right corner opens the screen's panel: size presets and width/height, and what it shows (always a HumanityOS page, or always a camera post). A page surface is resized in place and keeps its scroll state; a stream's or clip's surface is made anew at the final shape when the handle is let go. FULL VIEWS ON THE WALL: a page on a wall screen is laid out for that screen's own size and shape, and a full view it opens (a chat video's Full window, the picture viewer, the 3D view) opens on that screen, drawn by its own run and closed there; the picture viewer's and the 3D view's state is per screen (swapped like the textures), so nothing a wall opens lands on the app screen (the 3D view shows its plain "needs the app's graphics" words on a wall until the surfaces get the viewer's GPU pieces). A CONTROL ROOM (one fixed screen per page, all live) is designed as the next rung.
- Native: `src/engine/screens/aim.rs` (`targets`, `pick`, `plane_px`, `reach_m`, `Shown`, `PressHold`, `draw_overlay`), `src/engine/screens/edit.rs` (`handle_at`, `Frame`, `ScreenGrab`, `drag`, `write_drag`, `try_pick`, `apply_drag`, `draw_handles`), `src/gui/screen_tuning.rs`, `src/engine/screens.rs` (`Screens::aim_at`, `follow_placements`, the in-place resize in `sync_screens`), `src/gui/pages/screen_settings.rs` (the cog's panel), `src/gui/screen_surface.rs` (`draw_screen_full_views`, the per-screen `image_viewer_url` and `chat_model_view`), `src/gui/pages/chat/inline_video.rs` (Full window on a wall), `src/gui/pages/settings.rs` (Settings > Controls), `crates/humanity-ship/src/machines.rs` (`MachineInstance.screen_size_m`, `screen_px_for_shape`, `body_size_for_display`, `display_size_of`)
- Data: `data/gui/screens.json` (the aim assist's cone, hysteresis and reach; the size limits, grid, handle band and presets)
- Web: none by design (the screens live in the 3D world the native renderer draws).
- Docs: `docs/design/in-world-screens.md` ("Aim assist, any size, and each screen's own settings": the whole feature and its rungs; rungs 3 to 5, rules, live channels through the embedded-browser host and neighbour snapping, are designed, not built)

### Cloud dev panel (F10) with data-driven test markers (v0.1289)
Live cloud-renderer switches (every probe-rig pin: caches, step economy, bisect channels, resolution). `data/gui/dev_tests.json` names the rows the operator is being asked to test; the panel lists them at the top and paints a red TEST tag beside each named row, re-read live while the panel is open, so the chat and the panel name the same switch.
Since 2026-09-05 the panel is a LEFT SIDEBAR (resizable egui SidePanel, default 420 px) whose whole body scrolls with an always-visible bar, with a slim collapse tab on its right edge (painted arrow, `icons::paint_arrow_left/right`). F10 opens it expanded and FREES the cursor for as long as it is expanded (the held-Alt rule made sticky: `GuiState::cloud_dev_sidebar_expanded` feeds `reconcile_cursor` and the mouse-look gate in `lib.rs`), so no key needs holding to scroll or click; F10 again, or the tab, closes/collapses it and the cursor returns to whatever the game would have (grabbed in first person). Collapsed = only the tab remains at the screen edge, and only while the cursor is free (menu page, Alt held): with the cursor grabbed the tab is not drawn at all, because the hidden cursor drifts to the window edge on a long turn and a click target there would eat the next fire click; F10 expands a collapsed sidebar in that state. The tab also carries a painted close cross (closes the sidebar outright, same as F10 on an expanded one), so the panel can be closed from the UI as well as the key. Slider rows show their value as a plain label rather than egui's click-to-type box (a focused number box makes egui swallow every key event, which would freeze WASD mid-flight), so nothing on the panel can take keyboard focus. The collapsed flag and the cursor mirror (`GuiState::cursor_free`, written only by `reconcile_cursor`) are session-only, like every switch on the panel.
**Keys and the Escape ordering (2026-09-18).** F10 on a closed or collapsed sidebar opens it expanded; F10 on an expanded one closes it (`engine::input::toggle_cloud_dev_panel`). Escape closes an EXPANDED sidebar FIRST and does nothing else that press (`engine::input::escape_closes_cloud_dev`, called at the top of the Escape ordering in lib.rs, ahead of the in-world modal gate, the help-panel unpin and the menu cascade: the topmost thing on screen is what Escape means, and the sidebar never takes the keyboard so closing it interrupts no typing); with the sidebar closed or collapsed, Escape keeps its usual meaning (help panel, then the menu). The cursor re-grabs through `reconcile_cursor` either way. Both rules are pure `GuiState` functions shared by the key, the unit tests (`engine::input::tests`) and the dev IPC below.
**Clickable under the HUD (BUG-076, 2026-09-18).** The sidebar took no input in the world from the day it became a SidePanel: the HUD's full-screen Area allocated a `Sense::hover` rect in the Middle order, and egui's hit test stops at the first widget that covers the search area whatever its sense, so every widget in the Background-order panel under it was dropped. The HUD no longer allocates any rect (it only paints). Pinned headlessly by `cloud_dev::tests::a_sidebar_checkbox_click_flips_its_flag_under_the_hud` (the HUD and the sidebar drawn in one frame, a checkbox located by its text and clicked), proven red before the fix.
- Native: `src/gui/pages/cloud_dev.rs` (`dev_tests`, `test_mark`, `collapse_tab`, `draw_body`, the headless click tests), `src/engine/input.rs` (`toggle_cloud_dev_panel`, `escape_closes_cloud_dev`, `cursor_want_free`, `cloud_dev_flag`), `src/gui/pages/hud.rs` (no full-screen rect), `src/gui/ui_snapshots.rs` (`snapshot_cloud_dev`, `snapshot_cloud_dev_collapsed`), `src/lib.rs` (F10 + Escape call sites, `reconcile_cursor`)
- Web: none, by design. This is native-only renderer diagnostics (it drives wgpu cloud-march flags on the local GPU); the website has no renderer to diagnose, so there is nothing to mirror.
- Data: `data/gui/dev_tests.json`

### Main-UI dev IPC: `debug/ui_request.json` (2026-09-18)
Permanent dev tooling, the sibling of the in-world screens' `debug/screen_request.json` for the MAIN egui context: a rig can press the F10 sidebar's keys and click its widgets without a human at the keyboard, and read back what the cursor authority and egui think happened. Drop `{"action": "f10" | "escape" | "state" | "pointer_move" | "click" | "find", "pos": [x, y], "text": "...", "flag": "<name>"}` while the game runs. `find` answers where a drawn text is on the main frame (`found`, `text`, `matches`, `rect_px`, `pos_px` = the centre, ready to be a `click`'s `pos`) through the same `find_text_in_shapes` rule the screens use (exact, then prefix, then substring; clipped text skipped), scanned from that frame's shapes in lib.rs before tessellation, so a rig clicks a named toggle instead of guessing a pixel. `f10` and `escape` call the SAME functions the keys run (`engine::input`), so what they prove is the rule, not the winit layer between the key and the rule (that layer is covered by the headless harness and the operator's own key). `pointer_move` and `click` push `egui::Event::PointerMoved` / `PointerButton` into the main context's pending input, so egui's own hit-testing runs on the real panel; a click is move, press, release on three consecutive frames. `pos` is in window pixels (screenshot coordinates), converted with the live pixels-per-point. `flag` names any `GuiState` `cloud_dev_*` field (or `show_cloud_dev_panel` / `cloud_dev_collapsed`) and is reported before and after; an unknown name or a missing `pos` fails at once. Every request is consumed, and `debug/ui_request_done.json` answers after the frame the last event landed in, with: `ok`, `action`, `cursor_free` (winit side), `gui_cursor_free` (the GuiState mirror), `cursor_want_free` (what `reconcile_cursor` wants; the one to read in a background rig, where the window is never touched and `background_no_cursor` is true), `cloud_dev_sidebar_expanded`, `show_cloud_dev_panel`, `cloud_dev_collapsed`, `active_page`, `world_loaded`, egui's `wants_pointer_input` and `is_pointer_over_area`, `flag` / `flag_before` / `flag_after`, `pos_px` / `pos_points` / `pixels_per_point`, and `closed` for `escape`.
- Native: `src/engine/ipc.rs` (`UiIpc`, `poll_ui_request`, `ui_request_scan_shapes`, `complete_ui_request`), `src/engine/input.rs` (the rules and `cloud_dev_flag`), `src/lib.rs` (the update-phase poll beside `poll_screen_request`; the shape scan between `egui_ctx.run` and tessellation; the completion after the post-present `reconcile_cursor`)
- Web: none (a native dev rig).
- Docs: `docs/dev/performance-profiling.md` (the debug file protocol table), `docs/design/in-world-screens.md` (cross-reference from the screens IPC)

### Graphics presets: Ultra (reference) and Extreme (v0.1289)
Settings > Graphics > Presets applies a named preset from data. "Ultra (reference)" is what the probe rig measures and what every performance number in PRIORITIES means by Ultra (cloud march at half resolution, constructed bodies, every surface and lighting feature at its physical strength); "Extreme" is every slider at its limit (full-resolution cloud march, 4x the reference). Unknown keys are logged and skipped so the file can grow ahead of the code.
- Native: `src/gui/pages/settings.rs` (`graphics_presets`, `apply_graphics_preset`)
- Data: `data/gui/graphics_presets.json`

### Graphics settings say what they change and what they cost (2026-11-10)
Every control on Settings > Graphics is followed by its description (what you will see differently,
by key, glossary marks kept) and a cost line for this computer: the graphics card's time each frame
in the passes it drives, the processor's, the video memory or the frame time, MEASURED live from the
engine's own timers and memory counters (the Performance page's numbers); WORKED OUT from the choice
and shown with it; a DOWNLOAD whose size is on its button (the star catalogues' and the Ultra glow's
read from their release records); or nothing measurable, with why. It says when it cannot know:
before the world has drawn a frame, a pass the card cannot time, a pass not running now. One row per
control in `data/gui/graphics_settings.ron` (compiled in); the test
`every_graphics_control_states_its_cost` fails on a control without its description and cost, a row
without a cost, or a measured id the engine never records. The audit that made it, and the checklist
for a new setting: `docs/design/graphics-settings.md`.
- Native: `src/gui/pages/settings/graphics.rs` (`graphics_hint`, `cost_line`, `registry`, the page),
  `src/gui/pages/settings/graphics_tests.rs`, `src/renderer/stars.rs` (`StarCatalogTier::download_bytes`,
  `GALAXY_GLOW_ULTRA_BYTES`)
- Data: `data/gui/graphics_settings.ron`; words under `graphics.*` in `data/i18n/en.json`
- Web: none (the website draws no 3D world).
### PBR Shader with Emissive (v0.90.0)
PBR material pipeline supports emissive strength via params.w.
- Shaders: `assets/shaders/pbr_simple.wgsl`

### 12 Procedural Materials (v0.90.0)
Glass, ice, water, leather, crystal, rust, moss, lava + original brick, metal, wood, concrete.
- Native: `src/renderer/pipeline.rs`
- Shaders: `assets/shaders/procedural_material.wgsl`, `assets/shaders/procedural/*.wgsl`
- Data: `data/materials/procedural_materials.ron`

### Sky Renderer
Time-of-day colors (dawn/day/dusk/night) modified by weather.
- Native: `src/renderer/sky.rs`

### Stars Renderer
Star field rendering for space scenes.
- Native: `src/renderer/stars.rs`

### Hologram Renderer
Holographic display rendering for ship interfaces.
- Native: `src/renderer/hologram.rs`

### Multi-Scale Renderer
Floating-origin and multi-scale rendering for planetary to galactic distances.
- Native: `src/renderer/multi_scale.rs`, `src/renderer/floating_origin.rs`

### Particle System (v0.90.0; state checked against the code 2026-09-25)
Billboarded sprites, simulated on the CPU (`particles.rs`) and, for rain and snow when the GPU-particles setting is on (v0.1068), in a GPU compute pass (`particles_gpu.rs` + `particle_sim.wgsl`). `data/particles.ron` DEFINES 15 emitters (fire, smoke, sparks, rain, snow, dust, magic_sparkle, engine_exhaust, healing, explosion, bubbles, welding, dive_bubbles, leaf_drift, space_dust), but a definition only reaches the screen when code spawns it. The ones anything spawns today:
- `leaf_drift`, `dive_bubbles`, `space_dust`: ambient emitters placed by `src/lib.rs` (around lines 3885-3976).
- `rain`, `snow`: precipitation (`src/lib.rs` around 4099-4200), CPU or GPU path.
- Whatever `data/weather/events.ron` lists for an active extreme-weather event: currently `dust` (tornado) and `sparks` (meteor shower), plus rain and snow.

NOT spawned by anything: fire, smoke, explosion, welding, engine_exhaust, healing, magic_sparkle, bubbles. Fire, smoke and explosions are designed but not built (docs/design/fire-props-and-combustion.md).
- Native: `src/renderer/particles.rs`, `src/renderer/particles_gpu.rs`
- Shaders: `assets/shaders/particles.wgsl` (the live draw shader, PLURAL), `assets/shaders/particle_sim.wgsl` (GPU sim). `assets/shaders/particle.wgsl` (singular) is not loaded by anything.
- **New particle shapes go in `particles.wgsl`.** The procedural six-fold snowflake of v0.1065 (commit da795adc) was written into the unused `particle.wgsl`, so it never reached the screen: snow's `shape: 1.0` in `particles.ron` is copied into every particle's vertex data, but the live `particles.wgsl` reads only size and emissive from that attribute, and every flake draws as a round dot.
- Data: `data/particles.ron`, read once at startup (edits take effect on the next launch; it is not hot-reloaded)

### Bloom Post-Process (v0.90.0, built but not run)
Half-resolution bright-pixel extraction, Gaussian blur, composite. The pass is built at startup and resized with the window, but nothing calls `BloomPass::apply`: the call site was removed when `bloom_intensity` was set to 0.0 (`src/renderer/mod.rs`), so there is no bloom in the frame and no setting that turns it on. Switching it back on means restoring the call site; the `gpu.bloom` timing row is kept for that (`crates/humanity-core/src/frame_costs.rs`).
- Native: `src/renderer/bloom.rs`
- Shaders: `assets/shaders/bloom.wgsl`

### Sun Direction Uniform (v0.90.8)
Data-driven sun direction passed as shader uniform instead of hardcoded in WGSL.
- Native: `src/renderer/pipeline.rs`
- Shaders: `assets/shaders/pbr_simple.wgsl`

### Planet Registry (v0.90.8)
Unified celestial body management for renderer, terrain, and maps.
- Native: `crates/humanity-terrain/src/planet.rs`

### Construction Placement (v0.90.8, partial)
Scaffolded placement system for building in the game world. Needs full integration. **⚠️ `PlacementSystem` NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/construction/mod.rs`

### GLTF Model Loading
Load .glb/.gltf models with normal and UV fallbacks. Cached by path.
- Native: `src/assets/mod.rs`

### Instanced Rendering
GPU instancing through the per-instance vertex stream at slot 1 (`INSTANCE_STRIDE`, `GrassInstance`): one mesh, one material, one draw, N instances (the grass strands). The old `InstanceBatch` / `render_instanced` path, a per-instance uniform loop that drew straight to the swapchain and had no callers, was deleted 2026-09-27 with the HDR scene target.
- Native: `src/renderer/mesh.rs` (instance layout), `src/renderer/mod.rs` (grass instances)

### Scene Target and Present Pass
Every scene pass draws into a scene target; one full-screen present pass copies it to the display before egui (HDR scene target increments 1 and 2, 2026-09-27). Off-screen views (camera screens, the hi-res capture) draw into a view-sized scratch and are presented the same way. The scene target is `Rgba16Float` (increment 3), and the present pass clamps it and adds ONE triangular dither of about one display code before the 8-bit write (increment 4), which ends the banding on dark gradients; black and white stay exact. Showcase keys `scene_format` (display or hdr, a same-boot A/B that rebuilds every scene pipeline) and `present_dither` (0 or 1). The one tonemap (increment 5) is next.
- Native: `src/renderer/scene_target.rs`, `src/renderer/scene_format_ab.rs`, `assets/shaders/present.wgsl`; tests `src/renderer/scene_target_tests.rs`; plan `docs/design/hdr-scene-target.md`; lint `tests/scene_format_lint.rs`; banding metric `scripts/band-census.js`; A/B plans `scripts/hdr-ab/`

### Icosphere Planet Terrain
Recursive subdivision from icosahedron. LOD from billboard to walkable surface.
- Native: `crates/humanity-terrain/src/icosphere.rs`, `crates/humanity-terrain/src/planet.rs`
- Data: `data/planets/*.ron`

### Procedural Sky-Planet Surfaces (v0.763)
Sky planets render as fractal-surfaced icospheres instead of flat-color
spheres: seeded FBM elevation displaces land (oceans stay smooth at the
sphere radius), per-face colors classify from elevation + latitude (ocean,
shore, lowland, highland, mountain, polar cap; dark basins on dry worlds),
plus a translucent fresnel atmosphere shell for bodies with air. Subdivision
level is screen-size-driven (one more level per doubling of projected pixel
size; threshold + max level + master toggle live in Settings, Graphics,
Planets, persisted in AppConfig). Meshes cache per (body, level). Colors ride
packed in the UV channel (PBR shader material types 12/13), so no new vertex
layout or pipeline. Planet defs HOT-RELOAD (v0.764): saving a
`data/planets/<id>.ron` mid-game re-reads the defs + drops cached meshes, so
palette/noise/sea-level tuning shows in the sky within a frame.
- Native: `crates/humanity-terrain/src/planet_surface.rs`, `crates/humanity-terrain/src/planet.rs`
  (`lod_level_for_pixels`), `src/renderer/mesh.rs` (`from_planet_surface`),
  `assets/shaders/pbr_simple.wgsl` (types 12/13), sky loop in `src/lib.rs`
- Data: `data/planets/earth.ron`, `mars.ron`, `moon.ron` (palette + noise
  params per body; a new planet look = a new RON file)

### Chunked Planetary LOD (2026-07-11)
When a heightmap-bearing planet's disc overflows the screen (the LOD
ladder's level-8 rung), the whole-sphere path hands off to a quadtree of
surface patches whose detail follows the camera: 20 icosahedron roots, each
patch a 16x16-tessellated spherical triangle (256 grid + 96 skirt
triangles), refined by screen-space error down to depth 13 (~54 m triangle
edges near the surface; the ~1 m goal is a documented follow-up at depth
~19 + micro-detail synthesis). Per-patch f64 anchors keep sub-cm precision
(vertices are small f32 offsets; translations compose in f64 and narrow
last). Culling: horizon cone test (the far side costs zero geometry) +
frustum planes, both during tree descent; built patches carry measured
radial bounds for tight culls. Depth-scaled skirts seal cross-depth seams.
Streaming: budgeted builds (6/frame, worst-error first), restricted descent
(no holes ever), 256 MB LRU cache (64 MB warm floor after departure, roots
pinned, renderer slots recycled). Below the heightmap's 0.1 deg resolution,
3 octaves of seeded land-masked detail noise (~30 m) keep close terrain
from going flat. Toggle in Settings > Graphics > Planets ("Chunked surface
detail"), persisted in AppConfig.
- Native: `crates/humanity-terrain/src/planet_chunks.rs` (pure math + 17 headless tests),
  sky loop in `src/lib.rs` (selection/build/draw), `src/renderer/mesh.rs`
  (`placeholder` slot recycling)
- Data: any `data/planets/<id>.ron` with a `heightmap` (Earth today)

### Heightmap Terrain Generation
Procedural terrain from heightmaps with 16 biome types.
- Native: `crates/humanity-terrain/src/heightmap.rs`

### Voxel Asteroids
Sparse octree storage, greedy meshing, ore veins by classification, mining.
- Native: `crates/humanity-terrain/src/asteroid.rs`
- Data: `data/asteroids/types.csv`

### Ship Interiors
Ship layouts from RON, room mesh generation, BFS pathfinding between rooms.
- Native: `crates/humanity-ship/src/layout.rs`, `crates/humanity-ship/src/rooms.rs`
- Data: `data/ships/starter_fleet.ron`

### Physics (rapier3d)
Rigid bodies, colliders, raycasting, simulation stepping.
- Native: `crates/humanity-core/src/physics/mod.rs`

### Audio (kira)
Sound effects, music, spatial audio with distance falloff, volume controls.
- Native: `crates/humanity-core/src/audio/mod.rs`, `crates/humanity-core/src/audio/sounds.rs`

### Loudness: "Even out loudness" and the video start volume (2026-11-06)
Every sound played at the loudness of its kind (speech, video, music, sound effects, ambience,
interface, notifications; targets in LUFS as data), an ITU-R BS.1770 meter written here (tested on
EBU Tech 3341's signals), the shipped sounds measured into data by `just loudness`; Settings >
Audio "Even out loudness" (on by default). And the volume a new video starts at, one choice for chat
videos, the Media page and in-world screens ("My last volume", kept across restarts, or "A set
volume"), in both apps, with a volume bar on the Media page's player and a screen's strip and a
"Volume settings" cog beside a chat video's bar. Rungs 2 to 5 (videos and received files levelled as
they play, voices levelled per person, a slider per kind, an opt-in mic level) are planned in
`docs/design/loudness.md`.
- Native: `crates/humanity-core/src/audio/loudness.rs` (meter), `audio/levels.rs` (kinds, gains),
  `audio/start_volume.rs` (the video choice), `audio/mod.rs` (`AudioManager::one_shot`, the reuse
  switch), `src/gui/pages/settings.rs` (`video_start_choices`), `src/gui/widgets/video_view.rs`,
  `src/gui/pages/chat/inline_video.rs`, `src/engine/screens/video.rs` (`compose_mix` with the clip's
  own volume)
- Data: `data/audio/loudness.toml`, `data/audio/measured_loudness.toml` (generated), `category` in
  `data/sounds.toml`
- Web: `web/shared/video-volume.js`, Settings > Audio & Video > Videos (`web/pages/settings.html`)

### Live music and the ship's hum (2026-11-06)
The music on the pages, from the app's start (2026-11-11, the operator: "we should have the music start playing when HumanityOS starts", then "Play on the app pages as well."): ONE switch, Settings > Music > "Music on the app's pages" (on by default), plays the score on the title screen and the app's pages from the moment HumanityOS opens and all session, a trip into the world and back included; its hint says it plays alongside your own music player, each at its own volume in Windows. (A second switch, "Play the music when HumanityOS opens", which stopped at the first world entry, was folded into it the same day.) It follows Music on and the Master and Music volumes; an instance a script started in the background has no sound device until a person clicks into it, so rigs stay silent (`engine/music.rs` `gains`; test `the_music_starts_with_the_app_and_a_script_launch_stays_silent`).
The game composes its own music live at A = 432 Hz (440 one setting away): a composer walks each
mood's chord graph with voice leading, varies motifs and never repeats a phrase within ten minutes;
synthesised voices (pads, a formant choir, celesta bells, the STEEL PAN, membrane DEEP DRUMS and
taiko, FM electric piano, plucked bass and harp, noise drums, vinyl crackle) with a reverb and a
limiter, levelled to the music category's loudness. Six moods (mothership calm, menu, garden
morning, night watch, tension, action), places and the ship's hour choosing among them; GENRES as
data over the moods, chosen in Settings > Music's genre selector (mothership calm, lo-fi, and since 2026-11-12
ELECTRO SWING at 128 bpm: swung eighths over a house kick, a walking upright bass, gypsy-jazz comping,
brass stabs, clarinet calls answered by trombone, its own minor-key jazz progressions, played by the
core set's recorded trumpet, trombone, clarinet, pizzicato bass and guitar; a blend slider; places that may pick a livelier genre;
community genres are files); DANGER AND ACTION driven by an intensity the game sets (stinger, riser,
bar-line transitions, taiko ostinatos, brass stabs; the Dev page's knob and the `music_intensity` /
`music_breach` showcase verbs); the ship's hum (brown noise with resonances on the key at 432, louder
and darker toward the drive at the stern, softer in small rooms, silent off the ship, a stressed
breach alarm with a klaxon tuned to the key); the optional tones (off by default, honest words);
Settings > Music; `just music-render` renders offline. Since 2026-11-07 (BUG-266 to 270): the build
(the data read, the recordings decoded) runs on its own thread and a new score takes over at a
phrase (after an instrument install, a change of genre, and every phrase with "turn off all reuse"
off); the music sits 12 dB under a playing chat video's sound and 6 dB under on a page opened from
the world; the moods' pads, bells and drones and lo-fi's keys, lead and melody play recorded
instruments where installed. Rungs 5 to 10 (the self-meter, voice-call ducking, the website...) are
planned in `docs/design/music.md`.
- Native: `crates/humanity-music/` (the crate: `composer.rs`, `voice.rs`, `score.rs`, `bed.rs`,
  `intensity.rs`, `kira_sound.rs`, card `docs/crates/humanity-music.md`), `src/engine/music.rs` (the
  frame wiring), `src/gui/pages/settings_music.rs`, `src/gui/pages/dev_music.rs`,
  `crates/humanity-core/src/audio/mod.rs` (`AudioManager::play_data`, the ambience volume)
- Data: `data/music/instruments/*.ron`, `data/music/moods/*.ron`, `data/music/genres/*.ron`,
  `data/music/places.ron`, `data/music/intensity.ron`, `data/music/tones.ron`, `data/audio/hum_sources.ron`
- Tools: `just music-render <mood> <genre> <seconds>`, the example `crates/humanity-music/examples/music_render.rs`

### Sound levels: each shipped sound's own level (2026-11-06)
Settings > Audio > "Sound levels": every sound the game ships (each `data/sounds.toml` entry whose
file exists), grouped by kind, each with its name by key (`sound_names.<id>`), a level slider from
-24 to +12 dB (0 = as the app plays it, on top of "Even out loudness"), Play (heard at that level
through the game's own one-shot path) and Reset; a search box and Reset all. The levels are the
person's, kept in `config.json` (`sound_levels`, by sound id); `data/sounds.toml` stays the shipped
default. The website plays only notification sounds, so its mirror lists just those (kept in the
browser, `hos_sound_levels`; a level past the `<audio>` element's volume goes through Web Audio).
- Native: `crates/humanity-core/src/audio/sound_levels.rs` (the held levels), `audio/mod.rs`
  (`one_shot` and `preview` apply them; `AudioManager::without_device` for tests on kira's mock
  backend), `src/gui/pages/settings/sound_levels.rs` (the section and `play_preview`), `src/config.rs`
- Web: `web/shared/sound-levels.js`, Settings > Audio & Video > Sound levels (`web/pages/settings.html`),
  `web/chat/chat-notify.js` (`playSound` plays at the level)
- Buses (2026-11-07, BUG-265): the game plays every catalogued sound on its catalogue bus, as Play
  does (`src/engine/sfx.rs`); a NOTIFICATIONS volume (Settings > Audio > Volumes, `AppConfig::
  notifications_volume`) carries every notice's chime, so turning the clicks down never silences
  one; the "ambient" bus follows the ambience volume.

### Free instrument library: the core set, downloadable packs, Settings > Audio > Instruments (2026-11-06)
Real recorded instruments for the live score (`docs/design/music.md` rung 6, section 23), all public
domain (CC0): FreePats' Upright Piano KW, VCSL, VSCO 2 Community Edition and Karoryfer's Black And
Green Guitars, and since 2026-11-07 Jeff Learman's steel pan, FreePats' Hang and Karoryfer's Big
Rusty Drums, downloaded once to the dev machine (`docs/reference/instrument-sources.md`, licence
texts in `docs/reference/instrument-licences/`). `just instrument-packs` keeps the samples each
instrument needs, MEASURES each one's pitch (so the library's real tuning is recorded; nothing is
retuned on disk, the engine plays each at the rate for A = 432 or 440) and encodes Ogg Vorbis: a
small core set ships in the app (piano, harp, vibraphone, glockenspiel, tubular bells, wine glasses,
violins, cellos, a steel pan, a bass drum), and five packs (keys, strings, mallets and bells with the
steel pan, guitars, percussion and deep drums with the big mallet toms and the Hang, which plays only
its own nine notes) are one checked archive each, GitHub release assets the operator uploads. Settings >
Audio > Instruments lists them like the star catalog's tiers: what each holds, its size, its licence
in plain words, installed or not, Download / Remove with progress, and what the music plays for each
part now. Each pack is checked against its SHA-256 before it is unpacked; a damaged one is refused.
For each part of the score the best instrument installed plays, and its synthesised voice when none
is (`data/music/parts.ron`), so the music always plays. Decoded samples obey the "turn off all
reuse" switch. The score plays them through its sampler voice (`src/instrument_packs/score.rs` attaches them by part).
- Native: `src/instrument_packs/` (catalog, definition, install, library, samples, tar, ui),
  `src/gui/pages/settings/instruments.rs`, `instrument_packs::ui::tick` in `src/lib.rs`
- Data: `data/music/packs.ron` (generated), `data/music/parts.ron`, `data/music/instruments/*.ron`
  and `data/music/samples/<id>/` (generated core set, each with `LICENSE.txt` and `source.ron`)
- Tools: `scripts/instrument-packs.js`, `scripts/instrument-recipes.json`, `scripts/lib/sevenzip.js`
  and `lzma.js` (FreePats' .7z), `scripts/serve-instrument-packs.js` (try packs on 127.0.0.1);
  upload recipe in `docs/ai/release.md`, "Instrument packs"
- Web: none yet (the website plays no music until music.md rung 9)

### NVIDIA SimReady props: real-scale furniture, fixtures, tools and safety gear (2026-11-09)
The project's first 3D models that are not CC0 (`docs/design/simready-props.md`; licence research
`docs/reference/findings/2026-11-08-nvidia-simready-assets.md`): 40 of NVIDIA's CC BY 4.0 SimReady
Assets ship in the app (`assets/models/simready/`), and the 6 Medical Bay ones are the first
downloadable model pack. `just simready` converts each OpenUSD package with the installed Blender's
own `pxr` (extract), simplifies it and bakes its many materials onto one texture set (base colour,
normal, ORM; Cycles, fixed seed), then packs ONE GLB per model in node (mesh 0 the baked model, mesh
1 a quarter-triangle distance version, then its collider hulls), with NVIDIA's `LICENSE.txt` byte for
byte, a `SOURCE.md` of every change and a `physics.ron` of its UsdPhysics values; `check` rebuilds
and compares every byte. Every model passed a brand and emblem check first (eight repainted: maker
names, logos, two certification marks, and a red-and-white cross turned to the ISO 7010 green, plus
three names moulded into the geometry painted out of the bake itself; one dropped). Catalogue rows: the toilet, bathroom sinks, both workshop benches, the tool storage, the
mushroom humidifier, pantry and workshop shelving and the Commons serving counter, plus 35 new rows
(kitchen and bathroom props, hand tools, a bench vise, lamp and multimeter, a step ladder, a
workshop cabinet, a fire extinguisher and its stand, a first aid kit, storage, and the Medical Bay).
The walk-up card of each carries a one-line credit (a prop with nothing else to report shows its
name until looked at). Settings > Graphics > Furniture detail (Auto, Full, Simple, Boxes) shows each
level's measured video memory (textures and meshes) and warns when the card is short. Every machine
model now stands centred on its placement (`GltfCpuMesh::centre_footprint`: the Kenney furniture was
drawn from its corner), and a model read decodes only the picture it draws.
- Native: `src/assets/model_physics.rs`, `src/model_packs/` (catalog, ui), `src/gui/furniture_detail.rs` (+ the engine's rebuild, `src/engine/furniture_detail.rs`),
  `src/engine/home_meshes.rs` (`build_machine_bodies`), `src/gui/pages/settings/furniture.rs`
- Data: `data/machines/home.ron`, `ship.ron` (rows), `data/models/packs.ron` (generated),
  `data/gui/furniture_detail.json`, `data/credits.ron` (`nvidia_simready`)
- Assets: `assets/models/simready/<model>/` (GLB, LICENSE.txt, SOURCE.md, physics.ron), `manifest.json`,
  `assets/models/LICENSE-nvidia-simready.md`
- Tools: `scripts/simready/` (`fetch.js`, `build.js`, `extract_usd.py`, `bake_lods.py`, `bake-run.js`,
  `pack.js`, `repaint.js`, `review-sheet.js`, `find-blender.js`, `starter-set.json`); `just simready`
- Tests: `tests/simready_lint.rs`, `scripts/tests/simready-pack.test.js`, `model_physics` and
  `model_packs` lib tests, `config::every_offered_furniture_detail_survives_a_restart`,
  `gui::furniture_detail` tests, `hud::walk_up_card_tests`,
  `crossing_tests::assets::every_modelled_machine_in_the_home_is_drawn_centred_on_its_placement`
- Web: none (the website draws no 3D home); the Files page's 3D viewer opens the GLBs

### Embeddable AI, rung 1: the engine skeleton and the door, with no model (2026-11-09)
AI is optional, never required (the operator, 2026-11-09; `docs/design/embeddable-ai.md`, section 14
item 1). The app holds NO model and NO runtime: what will run a model is the AI engine,
`humanity-ai.exe`, a separate program in `crates/humanity-ai-engine/` with a workspace of its own
(never a member of the app's, never a dependency; `just ai-engine` builds it). Rung 1's engine has no
runtime: `--fake` answers with scripted lines and timing (prompt reuse, cancel, memory, speech and a
label photo all stood in), and without `--fake` it refuses every model request in words. The two talk
over the engine's standard input and output, one JSON message a line, every message carrying the
protocol version (`crates/humanity-ai-protocol/`); a mismatched engine is refused with words. The
app's client starts the engine on demand by its full path, with pipes (no socket, no listener), no
console window, at below-normal priority, inside a Windows job object that caps its memory, ends it
with the app (even a crashed one) and keeps a crash quiet; it stops it after five idle minutes; a crash
falls back at once, the next request restarts it, and two failures in a row stop automatic restarts
until Try again; the Dev page's reuse switch reaches it. Settings > AI says "No AI is installed.
Everything in HumanityOS works without it.", what that means, this computer's memory, cores, AVX2,
graphics card and free space in plain words, and one card per later AI choice (talking characters Rich
and Quick, the helper on strong and mid computers, the computer voice and its small model) with what
it needs, whether this computer has it, and what works instead. Dev > AI drives the fake engine
(start, stop, stop as a crash, Try again, a test question streamed word by word, status, the job and
priority and memory against the cap, counters, the reuse switch's effect) and pretends less free
memory so every notice can be seen. No downloads yet (rung 2).
- Native: `src/ai/` (`client.rs`, `job.rs`, `hardware.rs`, `choices.rs`, `paths.rs`, `shared.rs`),
  `src/gui/pages/settings/ai.rs`, `src/gui/pages/dev_ai.rs`
- Crates: `crates/humanity-ai-protocol/` (serde types, in the app under `native` only),
  `crates/humanity-ai-engine/` (the engine program; its own `Cargo.lock`)
- Data: `data/ai/choices.ron` (the choices and their needs), `data/i18n/en.json` (`ai.*`, `dev.ai.*`),
  `data/glossary.json` (AI, AI engine, AI model, computer voice, AVX2), `data/gui/navigation.json` (Dev > AI)
- Tests: `tests/no_ai_in_app_lint.rs` (in `just lints`: no model runtime in the app's manifests or
  Cargo.lock, no model file in data/ or assets/), `src/ai/client_tests.rs` (the real fake engine over
  pipes: on demand, in its job at below-normal priority, ended with the job, the memory cap, crash and
  the two-failure rule, another protocol refused, idle stop, the reuse switch), `src/ai/hardware_tests.rs`,
  the protocol's lib tests, `just ai-engine test`, `scripts/tests/check-crate-layers.test.js` (a crate
  with a workspace of its own)
- Snapshots: `just snapshot settings_ai`, `settings_ai_low_memory`, `dev_ai`
- Web: Settings > AI in `web/pages/settings.html` says AI is optional and desktop-only, and why

### Embeddable AI, rung 2: the catalog, downloads, checks and removal; llama.cpp and Granite 4.0 1B (2026-11-09)
AI a person can actually choose, download, use and remove (`docs/design/embeddable-ai.md`, section 14
item 2). The engine gains llama.cpp behind its `llama` cargo feature (CPU only; one
`ggml-cpu-<level>.dll` per processor generation, the best chosen at start; built from the pinned
llama.cpp source of the crate llama-cpp-sys-2 0.1.161 through the engine's own C door, `shim.c`, with
the CMake inside Visual Studio's Build Tools): load, a reply streamed in whole characters with cancel,
max_tokens and an optional GBNF grammar, unload, a status with its real committed memory, prompt reuse
that obeys the reuse switch. `just ai-engine package` makes the stored, licensed zip
(`humanity-ai-engine-v1-windows-x64.zip`). The app compiles in `data/ai/catalog.ron` (written by
`just ai-catalog` from its recipe: every file's size and SHA-256, Granite's equal to IBM's own; files of
2 GiB or more in parts, over 2,000,000,000 bytes since 2026-11-11), and Settings > AI shows, before anything downloads, each downloadable model's
size (the engine's added the first time: it comes with the first model and goes with the last), source
and licence (its full text opens on the card); Get downloads in the background with progress, Stop
keeps what arrived and Get carries on with an HTTP Range request, every file is checked (a mismatch is
deleted and said in plain words), parts are joined and checked whole, the engine is unpacked from its
checked zip; Remove deletes a model (the engine with the last), Remove all AI lists exactly what goes
and that it cannot be undone, then deletes the `ai` folder. Dev > AI's "Send a test question" goes to
the installed engine and Granite (checked byte for byte before its load, the job object's cap raised to
its need plus 512 MB, 4 threads on a 6-core PC) and streams Granite's answer.
- Native: `src/ai/` (`catalog.rs`, `install.rs`, `models.rs`, `zip.rs`; `client.rs` gains
  `set_memory_cap`, `shutdown_now`, a real `load`), `src/gui/pages/settings/ai.rs`, `src/gui/pages/dev_ai.rs`
- Engine: `crates/humanity-ai-engine/` (`build.rs`, `src/llama/` with `shim.c`, `src/real.rs`,
  `src/text.rs`, `src/memory.rs`; `llama-src/` pins the llama.cpp source), `scripts/ai-engine.js`
  (`just ai-engine build|test|check|package|measure`)
- Data: `data/ai/catalog.ron` (generated), `data/ai/licences/` (apache-2.0.txt, llama.cpp-mit.txt),
  `data/credits.ron` (llama_cpp, llama_cpp_rs, granite_4_0_1b), `data/i18n/en.json` (`ai.card.*`,
  `ai.download.*`, `ai.remove_all.*`, `ai.licence.*`, `dev.ai.brain*`, `dev.ai.loaded*`)
- Scripts: `scripts/ai-catalog.js` + `scripts/ai-catalog-recipe.json` (`just ai-catalog [--stage DIR]`),
  `scripts/check-ai-catalog.js` (in `just preflight`), `scripts/serve-ai-files.js` (a loopback server
  with Range, for trying downloads before an upload; with `HUMANITY_AI_CATALOG` and
  `just ai-catalog --print --base-url http://127.0.0.1:<port>/`)
- Tests: `src/ai/install_tests.rs` (the built-in catalog; its rules; install, resume with Range,
  refusal and deletion of a mismatch, parts joined and checked whole, Remove and Remove all leaving
  nothing, the zip read strictly, brain threads; and the ignored `granite_through_the_app_end_to_end`),
  the engine's llama tests (`just ai-engine test`), `scripts/check-ai-catalog.js`
- Snapshots: `just snapshot settings_ai` (Granite not installed, its licence open),
  `settings_ai_installed` (installed, Remove all AI's confirmation)
- Release files: the `assets-ai-1` release (the top-level session uploads; `just ai-catalog --stage`
  prepares exactly its files and the `gh release create ... --latest=false` command)

### Embeddable AI: Ministral 3 3B, the Rich brain for talking characters (2026-11-11)
The operator, 2026-11-11: "setup ministral". Mistral AI's own text-only GGUF
(`Ministral-3-3B-Instruct-2512-Q4_K_M.gguf`, 2,147,023,008 bytes, SHA-256 Mistral's own) is a downloadable
model beside Granite: the Rich choice of Talking characters (`data/ai/choices.ron` `characters-rich`), the
stronger brain beside Granite's Quick one. The pinned llama.cpp runs it unchanged (architecture
`mistral3`); measured on the operator's PC at 4 threads and below-normal priority: 2.2 s load, about
2.1 GB committed, 54 to 57 tokens a second reading a persona and 12 to 14 writing (design 9.3a). Its
catalog row comes from the recipe with the Apache 2.0 text and the finding's credit README beside the
file; it goes up to `assets-ai-1` in two parts, because any file over 2,000,000,000 bytes is now split
(7% under GitHub's 2 GiB; the recipe's `part_bytes` and the app's `PART_LIMIT`, held equal by
`scripts/check-ai-catalog.js`). Settings > AI's Rich card shows its size, maker (Mistral AI) and
licence before anything downloads. The engine's prompts never use the GGUF's own default system message
(the finding's condition), and the talk card itself is rung 3.
- Data: `scripts/ai-catalog-recipe.json` (the row), `data/ai/catalog.ron` (regenerated with
  `just ai-catalog --keep-engine`, which keeps the published engine zip's row), `data/credits.ron`
  (`ministral_3_3b`), `CREDITS.md`, `LICENSES.md`
- Scripts: `scripts/ai-catalog.js` (`--keep-engine`; `--stage DIR --add ID` stages only the files to add
  to the published release and writes the `gh release upload` command), `scripts/check-ai-catalog.js`
- Tests: `ministral_loads_and_answers` (the engine, `just ai-engine test`; SKIPPED in words without the
  file), `ministral_installs_in_its_parts_from_a_loopback_server` (`src/ai/install_tests.rs`)
- Snapshot: `just snapshot settings_ai` (both models' cards, nothing downloaded)

### Embeddable AI, rung 3: the talk card, written for everyone and AI for those who chose it (2026-11-12)
The card the operator approved on 2026-11-11 (`docs/design/embeddable-ai.md`, section 14 rung 3 as
built). Walk up to a crew member, look, press E: their name, job and what they are doing; their line;
and WRITTEN choices from data per role and person: Talk about (the ship, their job, the cause,
directions), Work (the quests they offer, with their state, and Take it on), Trade (only someone who
trades), Show me the way (the HUD's tracked-marker ring on the place, with its name and distance,
until you arrive), Goodbye. Works whole with no AI installed. With a brain installed (Granite or
Ministral) and "Written lines and Say anything" chosen in Settings > AI, a "Say anything" box: the
character answers in its persona from the project's own prompt, in the fixed shape (the line and at
most one action the card grants, offered as a button the player clicks), through the filter on every
streamed piece, with the AI tag on every AI line, and falls back to its written lines on any failure,
refusal or timeout, in words. The model is checked, loaded and handed the persona when the card opens
(warm-up). Local only. The AI engine 1.1.0 formats Ministral in its own `mistral-v7-tekken` shape;
Settings > AI offers the engine update to anyone whose models came with 1.0.0; and with nothing
installed the card offers AI once.
- Native: `src/gui/talk/` (`mod.rs` the card's state and choices, `book.rs` talk.ron and crew.ron,
  `chat.rs` the box and the engine, `card.rs` drawing), `src/ai/talk.rs` (prompt, grammar, shape check,
  grants, filter, counts), `src/engine/talk.rs` (the way on the HUD, the talk IPC), `src/ai/models.rs`
  (`pick_brain`), `src/ai/install.rs` (`update_engine`), Settings > AI (`src/gui/pages/settings/ai.rs`:
  How characters talk, Which AI model, the engine update), Dev > AI's talk counts,
  `crates/humanity-systems/src/quests/mod.rs` (`DestinationDef::target`)
- Engine: `crates/humanity-ai-engine/src/llama/shim.c` (`hai_template_for`), the fake's talk shape
- Data: `data/npc/talk.ron`, `data/npc/crew.ron` (`persona`), `data/ai/prompts/character.txt`,
  `data/ai/filter.ron`, `data/ai/catalog.ron` (engine 1.1.0); words under `talk.*` and `ai.talk.*`
- Settings: `ai_talk_mode`, `ai_brain`, `ai_talk_offer_dismissed` (config.json)
- Rig: `just ai-talk` (`scripts/ai-talk-rig.js`, timing), `debug/talk_request.json`
- Tests: `src/ai/talk_tests.rs`, `src/gui/talk/tests.rs`, `an_engine_update_keeps_the_models_and_drops_the_old_engine`,
  `the_fake_answers_the_talk_cards_shape` and the Tekken check in `ministral_loads_and_answers` (engine)
- Snapshots: `just snapshot talk_card`, `just snapshot talk_card_ai`

### ECS (hecs)
System trait, SystemRunner, 20+ components, per-frame tick.
- Native: `crates/humanity-core/src/ecs/systems.rs`, `crates/humanity-core/src/ecs/components.rs`

### Hot-Reload
File watcher (notify) invalidates asset cache per frame.
- Native: `crates/humanity-core/src/hot_reload/`, `src/assets/mod.rs`

### Homes on the ship: plots (increment 1a of docs/design/ship-homes-and-logistics.md, v0.1442.0)
The ship and the home are separate files: `data/blueprints/ship_structure.ron`
holds the Commons, street-1, the corridors, 13 districts and a `plots` list
(twelve homesteads since 2026-10-04: p1 on the Commons and p2 to p12 down the
west side of First Street, which runs 1.1 km; `default_plot`; since increment 6, 2026-10-12,
also sixteen cabin plots, c1 to c16, on Cabin Block A east of the street's north end, and the
South Commons halfway down it); `data/homes/homestead.ron` holds the home design (23 room
zones, door, spawn); `data/machines/ship.ron` the Commons machines (zone-local).
Loading assembles the ship with "my home" at my plot's origin
(`ship_structure::load_and_assemble`); moving a plot moves the whole home, its
machines and its spawn. Plots and zones may not overlap; the E key builds only
inside your own plot (Dev exempt); outside Dev the ship's machines are
read-only. Dev mode has a Plots and Districts panel in Construction
(`src/gui/pages/construction/ship_tools.rs`). The relay hands out plots since 1b
(`src/relay/handlers/game_state.rs` `assign_home`, `src/relay/storage/plots.rs`), and since
increment 6 a plot of the kind the player's start asks for (next entry).

### Where you live: starts, the Cabin, one save per world (ship homes increment 6, 2026-10-12)
The kinds of home a player can start in are data, read by the relay and the game alike: Cabin
and Homestead offered (the Homestead the default), Apartment shown greyed until there is an
apartment block. Each row names its design, plot kind, households (Family/Solo now lives here,
per start), kit, storage, opening quest, whether the home has its own air, and which of the
homestead's world content it carries. The Cabin is one room of 3.15 x 2.5 m (7.54 m2 inside
its walls) with a single bed, nightstand, wardrobe, desk, chair and wall screen; sixteen of them
line Cabin Block A's hall, which has one shared washroom. The relay gives a joining player a
plot of the kind their `start` asks for and always keeps one already held; `game_home_request`
and `game_choose_home` say where you live there and let you choose while you hold none (answers
name only your own plot and the free counts). Your own home is `saves/offline_home.json` and
each server keeps its own save for you on this computer (`saves/servers/`), made fresh from the
start you choose there: only your name, look and outfit cross. A server's home is the shipped
design on your plot; a cabin breathes the ship's air and has none of the homestead's herd, wilds
or garden; an erased account's server save is set aside, never deleted. Choose it in Settings >
Gameplay "Where you live" (the household, for the live save), on the Home page (read only), and
in Characters (Play): "A new home" while your own home has no save yet, and the connected
server's card. The website's chat has a "Where you live" window (command palette).
**Characters relaid out (2026-11-02, docs/design/top-menu-and-game-pages.md section 5):** the
WHO and WHERE columns are shares of the window, the character stands in the middle of the stage
between them (a camera lens shift), the characters, homes and servers are small cards side by
side, the selected server shows its name, Connected, description and how many people with the
rest behind Server details, and the kinds of home are cards side by side, each with its first
sentence, the household choice where it belongs, Choose, and a "What it is like" fold; every word
by key. Tests: `showroom_tests.rs` (`the_picker_reads_evenly_at_the_operators_size`,
`the_stage_keeps_its_room_at_every_size`, `every_control_on_the_picker_answers_a_click`,
`the_character_stands_in_the_middle_of_the_stage`).
- Data: `data/homes/starts.ron`, `data/homes/shipped/cabin.ron` (the player's copy `data/homes/cabin.ron`), `data/machines/cabin.ron`, `data/places/cabin_seed.json`, `data/asteroids/belt.ron`, `data/blueprints/ship_structure.ron` (`cabins-1`, c1 to c16, `commons-2`), `data/blueprints/zone_types.ron`
- Native: `crates/humanity-ship/src/starts.rs` (`Starts`, `HomeAnswer`, `refusal_words`), `src/engine/save_slot.rs` (`wanted`, `fresh_save`, `follow`, `home_for`, `on_game_message`), `crates/humanity-systems/src/persistence/live_save.rs` (`ActiveHome`, the choices waiting for a fresh save, `set_aside_erased`), `src/save_load.rs` (`apply_onto_running_world`, `starting_kit`), `crates/humanity-systems/src/persistence.rs` (`SaveSlot`, `slot_path`, `ShipPlace`, `ShipMeals`), `src/engine/home_plot.rs` (`add_join_fields`, `guest_arrival`), `src/engine/world_load.rs` (`refresh_home_content`), `crates/humanity-ship/src/ship_space.rs` (`air_spaces_for`), `src/gui/pages/where_you_live.rs`, `src/gui/pages/showroom.rs`, `src/gui/pages/settings.rs`, `src/gui/pages/homes.rs`, `src/engine/ipc.rs` (`choose_start`, `pairing`, `meal`, `water`, `home_session`)
- Server: `src/relay/handlers/game_state.rs` (`JoinHome`, `assign_home`, `HomeAssignment::add_to_welcome`), `src/relay/handlers/home_plots.rs` (`handle_home`, `answer_home`), `src/relay/storage/plots.rs` (`claim_plot`, `held_by`, `free_of`)
- Web: `web/chat/chat-home.js` (palette item `openWhereYouLive` in `data/commands.json`)
- Tests: `crates/humanity-ship/src/starts.rs`, `src/engine/save_slot_tests.rs`, `src/engine/save_slot_home_tests.rs`, `src/save_load_slot_tests.rs`, `src/gui/pages/where_you_live_tests.rs`, `src/gui/pages/showroom_tests.rs`, `src/relay/features.rs` (`a_cabin_and_a_homestead_on_one_ship`), `scripts/tests/chat-home.test.js`. Design: `docs/design/ship-homes-and-logistics.md` (increment 6 as built), `docs/design/ship-homes-increment-6-plan.md`

### Multiplayer Networking
WebSocket client (tungstenite), message protocol, ECS sync, position interpolation.
Other players are drawn by snapshot interpolation (v0.1440.0, BUG-125): a short buffer
per player, placed on the sender's own clock (`timestamp`, seconds, real time),
drawn `INTERP_DELAY_S` behind at constant speed, brief extrapolation on the
real velocity the sender now sends, teleports snapped (`SnapshotBuffer`,
`PositionSender` in `src/net/sync.rs`). A scripted second player
(`scripts/second-player.js`) signs in as a real identity and walks a path, for
testing with one person (`just verify-second-player`).
- Native: `src/net/protocol.rs`, `src/net/client.rs`, `src/net/sync.rs`

### Mod Support
Mod manifest format, directory scanning, load order, path override resolution.
Nothing calls `ModLoader` yet: the working way to mod today is editing the files
in the data folder. Every registry reads the folder's copy of a file only when
this version can read ALL of it (a single CSV row it cannot read refuses the
file); otherwise it uses the copy built into the exe and writes one
`[built-in data copy]` log line naming the file, the line and why (BUG-163,
2026-10-05). Since 2026-10-12 every start brings each data file nobody edited
up to the running game's copy and keeps each edited one (a stamp,
`data/.shipped.json`, records what the game wrote), never in a source tree or a
linked folder; Settings > Data > "Data folder" lists what the last update did,
the files it kept with "Use the game's copy", and the files this run did not
use (BUG-163 part 2, `docs/design/data-folder-updates.md`).
- Native: `crates/humanity-core/src/mods/mod.rs`, `crates/humanity-core/src/embedded_data.rs` (`load_data_or_embedded`),
  `crates/humanity-core/src/data_parse.rs` (`refusing_rows`), `crates/humanity-core/src/storage/refresh.rs` (the
  refresh), `src/gui/pages/settings/data_folder.rs` (the Settings block)
- Data: `data/mods/README.md`, `data/mods/example-mod/mod.json`

### World Persistence
Save and load game world state (entities, terrain, player progress). Every save
is written to a temp file and renamed over the real one, and the previous
version is kept as a snapshot under `backups/saves/<slot>/` (the newest 10, at
most one per 15 minutes, never a duplicate); Settings > Data lists them with
Restore and "Snapshot now" (v0.1442.0, BUG-129).
- Native: `crates/humanity-systems/src/persistence.rs`, `src/save_load.rs` (the offline home: inventory,
  skills, wallet, quests, vehicles, crops, builds and scaffolds, craft batches in
  flight, the world clock; since 2026-09-27 also the asteroids as mined down,
  the drone in flight with its cargo, the "Keep mining" standing order and the
  herd's yield timers)
- **What the home's machines hold (2026-09-27):** each battery bank's charge,
  each water tank's litres and each machine vessel's contents (the genset and
  refinery fuel drums, the grain silo, the pantry, the freezer, the furniture
  drawers, with their residue and toxic history) are saved by machine instance
  id and carried across world entry, which respawns every machine. Before, each
  restart and each world entry reset banks and tanks to half and emptied every
  vessel, destroying what was stored in it. None of them moves on by the time
  away. `crates/humanity-systems/src/machine_levels.rs`, `WorldSave.machine_levels`. The home's
  air (oxygen, carbon dioxide, humidity) and each grow room's air were already
  saved in `SoilMemory` (`home_air`, `rooms`).
- **Offline progression (2026-09-25):** crops, scaffolds under construction and
  craft batches catch up by the real time away when the game loads, with a
  notice. Settings >
  Gameplay > "Keep growing while away". `save_load::catch_up_world`;
  `docs/design/offline-progression.md`.
- **The world held until it is entered (2026-11-06, BUG-253):** the world loaded at startup
  neither runs nor speaks until the person enters it, nor while the character picker is open:
  its systems tick with no time passing (a page's request still applies), its notices wait and
  show on entry (a load while held drops the old world's), and a save written while held is
  stamped with the moment the world is current to, so the next catch-up counts the held time.
  A system's reply to a page's request (a craft refused at an unpowered station, a Plant press
  with no seed, a drone's service short of parts) is not the world speaking and shows at once
  (BUG-258): the system writes it on the replies channel, `humanity_systems::notices::reply`,
  the world's own lines on `notices::world`.
  `src/gui/world_entry.rs`, `src/engine/world_hold.rs` (`world_hold_tests.rs`),
  `save_load::WorldHold`, `crates/humanity-systems/src/notices.rs`.
- **Offline progression, the rest of single player (2026-09-27):** the
  automated machines run the time away through moment by moment on the inputs
  really on hand and the power the home could spare, resting at their keep
  targets and feeding each other (`crates/humanity-systems/src/crafting/away.rs`); the drone
  finishes its trip and keeps flying a standing order out of the asteroid's
  real ore (`mining::advance_away`); each animal's egg, milk or wool timer
  moves on to one yield waiting (`livestock::timers_after_away`), and since
  2026-10-12 they eat the time away's feed out of home storage
  (`systems::animal_care`, the character feeding them). Nothing else is
  used up, and nothing dies, without the player's say. A character select now
  restores the Barn with the save (`save_load::after_resume`). The power the
  time away charges each machine is what it really drew over its last game
  day (2026-10-22, the day record, `systems::day_energy`, saved as
  `WorldSave.machine_day_energy`), its data's estimate until it has run one:
  a space heater about 182 W in a fruiting tent, 1,500 W where it never
  reaches 24 C. (From 2026-10-05 only a heater had its own figure, its draw
  at the moment of the save, `heater_draw_w`, which the record replaced.)

### Data-Driven Tools (v0.90.7)
The tool catalog loads from external JSON instead of hardcoded data (the Apps page's Other programs,
Help and Services since 2026-10-19).
- Native: `src/gui/loaders.rs` (`load_external_catalog`), `src/software/catalog.rs`, `src/gui/pages/apps/programs.rs`
- Data: `data/external/catalog.json` (tools+services merged 2026-07-30)

### Data-Driven Sounds (v0.90.7)
sounds.rs loads sound configuration from TOML instead of hardcoded data.
- Native: `crates/humanity-core/src/audio/sounds.rs`
- Data: `data/sounds.toml`

### Chat Tint Colors in Theme (v0.90.7)
Chat channel tint colors moved from hardcoded values to theme.ron for customization.
- Data: `data/gui/theme.ron`

### Server Config Externalized (v0.90.7)
Server constants moved from hardcoded Rust to external JSON configuration.
- Data: `data/server-config.json`

### 8 Game System Modules (v0.90.7)
Scaffolded system modules for expanded gameplay.
- Native: `crates/humanity-systems/src/`

### Shader Library
WGSL shaders: planet surfaces (earth, mars, venus, mercury, jupiter, saturn, uranus, neptune, moon, pluto), PBR, procedural materials (brick, metal, wood, concrete, fabric, aperiodic), stars, constellations, orbit rings, ghost preview, particles, bloom, the Sun's visibility taps (`sun_vis.wgsl`).

### The Sun as a body (2026-10-21)
The Sun at its true angular size (0.533 degrees across at 1 AU, from its real radius and distance; a far Sun is held at 0.7 px and dimmed so its light is kept), a limb-darkened photosphere (Hestroffer & Magnan 1998 per colour channel) in its own colour (Kurucz flux atlas chromaticity), at its real radiance (the lighting's irradiance over its solid angle, about 46,000 times a sunlit wall), so the tone curve decides how it looks: white by day, yellow then orange near the horizon. Through air the scattering shell draws it with the ray's own per-channel transmittance (a Sun setting into Earth's limb from orbit reddens row by row). Baumbach's mean corona round it (faint at the fixed daylight exposure, seen round a covered disc). The glare round it is the diffraction of a camera aperture (30 mm by default; Settings > Graphics > Light and sky, "Sun glare: camera opening (mm)", since 2026-10-22: smaller spreads it wider, as a phone camera's) convolved with the disc, added over the finished frame in the present pass and scaled by how much of the disc each view can see (64 depth taps over the celestial and the scene depth). God rays now stop above the air, on a body with no air, and aboard the home (they lay on the planet below the Sun from orbit, and spread a soft glow over open space). A see-through pane (the home's glass roof, a window) takes its light from the side it is seen from, so the roof seen from below by day is clear glass with space behind it, not a sunlit grey with a dark hole round the Sun. Dev keys: `sun_glare`, `sun_aperture_mm` (a pin over the setting), `godrays`. Code: `src/renderer/sun_disc.rs`, `sun_glare.rs`, `assets/shaders/sun_vis.wgsl`, type 17 in `90-fragment-main.wgsl`, `sun_disc()` in `80-fragment-shared.wgsl`, the shell term in `30-atmosphere.wgsl`, the glare in `present.wgsl`.
- Shaders: `assets/shaders/`, `assets/shaders/procedural/`

---

## Game Systems

> **Registration status (recounted 2026-07-08 after the combat arc; older per-system notes below may be stale):** **24** systems are actually registered + tick in the runtime (`src/lib.rs`, grep `system_runner.register`): Time/Day-Night, Weather, Solar, Electrical, Plumbing, Atmosphere, Player Controller, Interaction, Farming, Inventory, ContainerCompatibility, Crafting, Construction, Manufacturing, Food, Drone, Vehicles, Livestock (v0.751), Ability (v0.753), Combat (v0.760), AI (v0.761), Economy (v0.747), Skills, Quests. Systems still **implemented but NOT registered** never tick (Ecology, Hydrology, Disaster, Psychology, and the scaffold-tier systems). `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` is the authoritative live list (the build fails if a system is neither registered nor deferred-with-reason) -- always cross-check THAT before trusting any per-system note below, since these notes silently go stale as systems get registered over time. `docs/STATUS.md` has the per-system status.

### Combat (v0.760 - v0.765)
The X1 combat arc live end to end at the wolf tier. CombatSystem (registered
v0.760): hits arrive via the `damage_events` channel, armor mitigates per
damage type, death inserts Dead, kills roll the creature's authored loot table
(chance-gated, min..=max counts) into the killer's pack when the PLAYER landed
the blow, creature corpses decay after 20s. AISystem (registered v0.761):
predators hunt prey INCLUDING the player, close distance under their own
locomotion, and bite (~15% of beast health per 1.5s, kinetic). Combat deaths
publish "killed by a Wolf" to the death screen. Armor-on-equip: worn
equipment.csv armor columns rebuild the player Armor component every frame,
per-type sums capped 0.85. Livestock flee living predators within 12m at
double speed (v0.762). Kills train melee (kinetic) / ranged (else) via
xp_grants. Melee (v0.765): F swings the worn hands-slot weapon (tool/blade
equipment rows: axe 14, spear 15 at 3m, pickaxe 12...) or bare hands (3 dmg);
offensive abilities cast at the faced creature (v0.760). Wild spawns are data.
- Native: `crates/humanity-systems/src/combat/` (system, `player_swing`, damage types),
  `crates/humanity-systems/src/ai/mod.rs` (behavior machine + bites), lib.rs bridges (swing,
  loot settle, armor rebuild, cast targeting)
- Data: `data/entities/wild_spawns.ron`, `data/creatures.csv` loot tables,
  `data/equipment.csv` damage/armor columns, 128 generated loot items in
  `data/items.csv` (`scripts/gen-loot-items.js`)

### Characters: skinned, animated bodies (character animation rungs 0 and 1, 2026-11-08; real bodies 2026-11-09)
Every humanoid is one skinned, animated body (`docs/design/character-animation.md`): the player in
third person (third person now WALKS the body, with the walls, floors, ladders and lifts of first
person, the camera behind it), the other players (speed and turning read from their interpolated
positions; their chosen body rides in their look), the crew, the showroom figure (tinted live by the
look and the outfit) and the dev crowd. **The bodies are MakeHuman's (CC0), a female and a male**
(1.59 and 1.73 m), made by our own converter `scripts/gen-makehuman-body.js` from MakeHuman's data
(56 of its 163 bones, the rest merged; long-sleeved shirt, trousers, shoes, hands, hair with a thin
shell, brows and eyes as tinted regions), each with 15 shape targets as glTF morph targets: the look's
height drives the height targets on the GPU (a tall person is long in the leg, not scaled up head and
all) and the skeleton follows the shaped mesh (`Skeleton::reshaped`). The character page chooses the
body and, since 2026-11-09, shapes it: **Body shape** sliders for build (muscle and weight, MakeHuman's
grid), proportions (shorter to longer legs) and ancestry (African, Asian and European as shares of
one whole), each a row of the body file (`shape.sliders`), saved in the look (`Appearance::shape`),
carried to other players clamped (`PlayerLook::shape`), the height holding still while they move and
the skeleton following, with Reset body shape; crew are women and men of varied build, ancestry and
height (a stable hash per person in `characters.ron`'s ranges). **The clips are Quaternius's Universal Animation Library (CC0)**:
idle, walk, jog and sprint play now, crouch, jump and sit are retargeted for rung 2; turning on the
spot stays our procedural clip. A locomotion state machine with blend spaces plays idle, turning on
the spot, and walk, jog and sprint stride-matched to the real speed (each clip's speed measured from
its feet, scaled to the body's legs), backwards when walking backwards. **The walking pace** is a
Settings > Gameplay level (three since 2026-11-11: Realistic, Brisk at 2 and 6 m/s, Simplified; data/gameplay/levels.json, the speeds game.csv's `*_speed_<level>_mps`): Realistic walks at 1.4 m/s and runs at 4, Simplified keeps 5 and 9.5
(data/game.csv). The nearest bodies (up to 16 within 15 m) are skinned once a frame by a compute pass
into ordinary meshes that every pass draws unchanged (and, since 2026-11-10, the shadow atlas: until then the scene list they stand in never reached the sun's map, so no body cast a shadow); the rest are skinned in
the vertex shader. The body is DATA: the humanoid standard uses VRM 1.0's bone names as roles; any
rig maps onto it by a role map (MakeHuman's default skeleton, Quaternius's rig, the test rigs, SOMA's
30 joints), with the MakeHuman-to-SOMA joint map written out for a learned model later; clips are
retargeted by role at load (rest poses aligned, a finger riding its hand's alignment, root travel
scaled by leg length), in memory only. The procedural test figure stays as the test asset and the
fallback for a body that does not load; the old box figures stand in if the data cannot be read, and
for a same-boot A/B (showcase `figures`).
- Crate: `crates/humanity-animation/` (card `docs/crates/humanity-animation.md`): skeleton, roles,
  role and joint maps, poses and the pose palette, clips and retargeting, the graph, the motion
  tracker, the tiers
- Native: `src/assets/skin.rs` (glTF skins: every primitive, JOINTS_0/WEIGHTS_0, the node tree,
  inverse binds, animations), `src/renderer/skinning.rs` (compute and vertex-shader skinning, the
  palette), `assets/shaders/skinning.wgsl`, `assets/shaders/skinning_vs.wgsl`,
  `src/engine/animation.rs` (every person each frame), `crates/humanity-core/src/skin_data.rs`,
  `SkinnedVertex` in `crates/humanity-core/src/vertex.rs`, material type 25
  (`renderer::materials::MATERIAL_TYPE_CHARACTER`)
- Data: `data/characters/` (`characters.ron`, `skeletons/humanoid.ron`, `bodies/`, `role_maps/`,
  `joint_maps/`, `clips.ron`, `locomotion.ron`); the bodies `assets/models/characters/makehuman/`
  (made by `scripts/gen-makehuman-body.js`), the clips `assets/models/characters/quaternius_ual/`
  (trimmed by `scripts/gen-quaternius-clips.js`, Quaternius's `License.txt` beside them), the test
  figure `assets/models/characters/hos_test_figure/` and clip library
  `assets/models/characters/hos_test_clips/` (made by `scripts/gen-test-figure.js`); the walking
  pace's speeds in `data/game.csv`
- Settings: Gameplay > Walking pace (`settings.walk_realistic`, `src/engine/walk_pace.rs`); the
  character page's body chooser (`Appearance::body_type`, carried to other players in the look) and Body shape
  sliders (`Appearance::shape`, `PlayerLook::shape`; `draw_body_shape` in `src/gui/pages/showroom.rs`)
- Dev: showcase verbs `crowd`, `figure_path`, `figures`, `camera_mode`, `body`, `look_height`,
  `walk_pace`, `shape`, `lineup`, `skeletons`, `varied_shapes` (engine/ipc.rs); the Dev page's Show skeletons
  (every joint and bone drawn on top); vantages `anim-*` (`anim-shape-*` the sliders at their ends) (rung 0's box baselines `*-boxes` beside them;
  `anim-tp-{female,male}-{idle,walk,run}` for the bodies in third person); costs `cpu.animation`,
  `cpu.skinning`, `gpu.skinning`

**Feet on the world, crouch and jump (character animation rung 2a, 2026-11-09;
`docs/design/character-animation.md` section 15).** Every near body's feet stand on the ground under
them: the floor, a stair tread, a ramp, a deck or a built piece aboard (the footing, asked per sole
point), the drawn terrain on a planet (map packs' road beds and building pads included; positions
and directions in f64 to the sampler). Contacts are labelled on every clip at load; two-bone analytic
leg IK on each person's reshaped skeleton puts a planted foot's sole on the ground (no more sinking
in the walk cycle) with the pelvis lowered by the larger drop, the foot turned to a slope about its
heel or ball, foot locking so a planted foot never slides, and footsteps planned on stairs onto each
tread's middle (from the stair piece's riser and tread). Middle bodies, near bodies beyond the feet's
measured per-frame budget (`full_budget_ms`), and everyone in Settings > Gameplay > Animation:
Simplified, get the pelvis offset alone. **Crouch** is a key (C, Settings >
Controls): the walk slows to the crouch pace (data/game.csv), the first-person view lowers, the body
plays the crouch idle and crouch walk, and other players see it; **jump** (Space) plays the
take-off, air loop and landing. The movement message carries the stance (`humanity_core::stance`,
one number, left out when standing so a standing player's message is unchanged, clamped by the relay
and the app alike). The character page shows the height in metres ("1.62 m"), measured from the
body with the look's build and ancestry.
- Crate: `crates/humanity-animation/src/feet.rs` (the `Ground` trait, `FootRig`, `label_contacts`,
  `FootPlacer`, `two_bone`); the graph's `Crouch`, `Airborne`, `VerticalSpeed` and `StateTime`
  inputs (`graph.rs`); `crates/humanity-core/src/stance.rs`
- Native: `src/engine/feet_ground.rs` (the ground aboard and on a planet), `src/engine/foot_probe.rs`
  (the drawn feet measured against the ground), `src/engine/animation.rs` (the feet per person and
  tier), `Footing::tread_at` (`src/engine/footing.rs`), the controller's crouch (`src/renderer/camera.rs`,
  `Camera::view_drop`), `GameAction::Crouch` (`crates/humanity-core/src/input/bindings.rs`), the
  relay passing the stance on (`src/relay/handlers/msg_handlers.rs`)
- Data: `characters.ron`'s `feet`, `locomotion.ron`'s crouch and jump states, `data/game.csv`'s
  `crouch_speed_*_mps`
- Settings: Gameplay > Animation (`settings.anim_simplified`, Realistic by default); Controls > Crouch;
  the character page's height in metres (`GuiState::my_height_m`)
- Dev: the Dev page's Feet on the world (Show skeletons also marks where each foot found the
  ground); showcase verbs `feet`, `animation_mode`, `crowd_crouch`, `foot_probe`, `hold` with
  `crouch`; vantages `anim-stairs`, `anim-ramp`, `anim-crouch`, `anim-slope-map` with the
  `foot_probe` check (scripts/lib/foot-probe-check.js) and their `-feet-off` red twins,
  `anim-crowd-{near,middle}-feet-off` for the cost

**The look: faces and bodies feature by feature, real clothes, shoes, hats and hair (character
animation, 2026-11-09; `docs/design/character-animation.md` section 16).** Each near person (and the
player, and the showroom's figure) is drawn with their OWN mesh, built once when their look changes:
their build, proportions, ancestry and 248 face and body detail sliders (MakeHuman's detail targets:
head, forehead, brows, eyes, nose, cheeks, mouth, chin and jaw, ears, neck, torso, chest, belly, hips,
arms, hands, legs, feet, tailor's measurements, whole-body shapes, one side against the other) applied
to every base-mesh vertex; MakeHuman's clothes (14 outfits), shoes (6), fedoras (2), hair (10 styles),
brows (12) and lashes (4) placed on that shaped mesh by their own bindings, so every shape fits them,
and skinned with the body; the body under a garment hidden where MakeHuman's own fitting file says
(cosmetics.csv's `covers`), so skin never pokes through; the skeleton and the feet's soles measured
from the person's own mesh with their shoes on (a shoe's sole lifts them). Cloth draws with its
texture, ambient occlusion and normal map; hair, brows and lashes as alpha cards in two passes (a
solid core and a blended fringe) with Karis's hair shading, coloured by the look's hair colour. The
character page gains the detail sliders (folded by group, a reset per group and for all) and choosers
for hair style, brows and lashes; the wardrobe lists the MakeHuman garments. The look reaches other
players when Done is pressed, at most once every 10 seconds, read through one clamping reader at
the relay and in the app (unknown sliders, garments and cosmetics dropped, values clamped, counts
capped), and passed only to players who can see you. Middle bodies keep the shared body with its shape
on the GPU. Settings > Controls > Crouch: Hold or Toggle. **Garment textures (2026-11-10)**: each garment's
texture is packed and BC7-encoded at conversion time by our own encoder (a DDS beside its source
PNGs, a quarter of the uncompressed memory), and lives in a slot of a GARMENT TEXTURE POOL (one array
texture per slot shape, allocated once, slots handed from garment to garment as people come and go,
sized from the near tier's cap; a full pool makes a newcomer wait in its mean colour, never an
allocation on the frame). Settings > Graphics > Garment textures (levels as data since 2026-11-10,
`data/gui/garment_textures.json`): 512, 1K and 2K, a garment never drawn larger than its own colour
map (`art_px`, measured by the converter: 24 garments at 2048, 6 shoes at 1024, 16 brows and lashes
at 512; no colour map reaches 4096, so 4K is not offered), each level with what it shows, its pools'
video memory on this card and its download; 512 and 1K ship, 2K is an optional pack (24 garments,
256 MB (268,460,032 bytes), a release asset under `assets-garments-1`, its size and SHA-256 compiled in from
`data/characters/garment_packs.ron`) downloaded when 2K is picked, resumable, checked, removable,
and drawn once installed (1K meanwhile); a level whose pools pass an eighth of the card's own memory
says so in words; a GPU without block compression draws them uncompressed, decoded from the same
files, and the page says so in words.
**The own-mesh pool (2026-11-10)**: each near person's own mesh lives in a range of three GPU buffers
(source vertices, their skinned copy, indices) allocated once on entering the world and sized for
the near tier all in the largest dressed person (the terrain patch arena's first-fit range
allocator), so a crowd arriving is two queue writes a person and never an allocation on the frame; a
newcomer takes the room of a person out of the near tier (one whose room alone fits it, else the
longest out), and when the free room is in
pieces the people near are packed down one a frame; a mesh past the plan is drawn with the shared
body and logged. Its memory shows under the garment textures in Settings > Graphics.
- Crate: `crates/humanity-animation/src/look.rs` (`LookLibrary`, `build_person`, `look_key`,
  `rebuild_due`), `library.rs` (`dress_for`, wearers' `dress`), `feet.rs` (`FootRig::with_soles`)
- Core: `crates/humanity-core/src/look_data.rs` (the binary readers, `GarmentData::place`),
  `player_look.rs` (`detail`, `wear`, `eyes`, `LookIds`, `clean_detail`, `clean_wear`, `look_due`),
  `cosmetics.rs` (`model`, `covers`), `Appearance::{detail, brows, lashes}`
- Native: `src/engine/animation.rs` (own meshes built on `look_threads` worker threads, garment
  materials, their slots in the garment texture pools and their release after 30 s unworn, the fringe pass), `src/engine/look_textures.rs` (a garment's DDS read off the frame, its levels for the setting, decoded where there is no block compression), `src/renderer/texture_pool.rs` (`SlotPool`, `create_texture_pool`, `texture_pool_write`, `set_material_pool_slot`), `src/renderer/bc7.rs` (the DDS reader and the BC7 decoder), `src/gui/garment_textures.rs` (the levels, `plan_for`, `pool_shapes`, `effective`, `source`, the notices, the section's state and `tick`), `src/gui/garment_textures/packs.rs` (the compiled-in pack catalog, `packs_dir`), `src/gui/pages/settings/garments.rs` (the levels with their memory and downloads, Remove, the notices, the own-mesh pool's memory), `src/instrument_packs/install.rs` (the resumable download), `scripts/gen-makehuman-look.js --pack` and `--check-pack` (the 2K pack), `scripts/lib/ustar.js`, `src/renderer/mesh_pool.rs` (`MeshPool`: the own-mesh pool's books, `next_step`, `pack_step`), `src/renderer/skinning.rs`
  (`skin_create_own_pool`, `skin_own_next_step`, `skin_own_place`, `skin_own_pack`, `skin_release_body`), `src/renderer/patch_arena.rs` (`RangeAlloc`, shared with the pool), `src/renderer/materials.rs` (`add_character_material`,
  `add_material_like`, types 25 textured and 26 hair), `assets/shaders/pbr/90-fragment-main.wgsl`
  (the pack, the cotangent frame, the hair's two passes and lobes), `src/engine/net_route.rs` (the
  look's send and `game_player_look`), `src/gui/pages/showroom.rs` (details, hair, brows, lashes),
  the relay's `take_look` (`src/relay/handlers/msg_handlers.rs`)
- Data: `data/characters/details.ron`, `data/characters/garments.ron` (generated by
  `scripts/gen-makehuman-look.js`, which packs and encodes the textures through `scripts/lib/garment-texture.js` and `scripts/lib/bc7.js`), `data/gui/garment_textures.json` (the setting's levels), `characters.ron`'s `look`, `look_threads` and
  each wearer's `dress` (players, crew, others), cosmetics.csv's MakeHuman rows,
  `assets/models/characters/makehuman/` (base files, details, garments with their source textures and
  their BC7 `<id>.dds`, `LOOK-SOURCE.md` with each texture's quality), the encoder scan
  `docs/reference/research/2026-11-10-bc7-encoders/`
- Dev: showcase verbs `own_mesh`, `covers`, `soles`, `garment_textures`, `look_detail`, `look_wear`,
  the `lineup` verb's `d:`, `hair=`, `wear:`; vantages `anim-look-face-{female,male}`,
  `anim-look-covers-off`, `anim-shoes` and `anim-shoes-soles-off` (the shod foot probe and its red
  twin), `anim-stairs-soles-off`, `anim-crowd-near-look-off`; costs
  `cpu.look_build`, `vram.characters`, `vram.textures` (the pools); `HUMANITY_NO_BC=1` starts the renderer
  without block compression (the fallback's arm)

**Interactions anchored to things, hands, look-at and true first person (character animation rung
2b, 2026-11-10; `docs/design/character-animation.md` section 17).** Sitting, lying, climbing a ladder,
working at a bench and a hand on a door are ANCHORS, rows of `data/characters/interactions.ron`
(where on the object, the approach, the contact targets, each phase's clip or target pose, root
driven or not, time limits) that objects name: a machine definition's `anchors` (the chair, the
couch's three seats, the bed's mattress and edge, the workbenches), a structure type's (the ladder),
an opening style's (a swing door's handle). A new chair is a row. **Motion warping** moves the hips
from where the sit clip puts them to where THIS seat puts them (the seat's height from the drawn
model, the flesh under the hips measured on each person's drawn mesh), eased in to the enter clip's
contact frame and out from the exit clip's lift-off; the root's travel and turn are eased over the
approach (a walk-in first when the player stood further off). E at a seat sits the player (the
capsule held there, a movement key or E gets up); E at the bed sleeps and lies them down; the
ladder's own climb shows hands and feet on its rungs (from the piece's rung spacing, a diagonal
gait; out at the hatch the hands let go); an open workbench card puts both hands on its top; a hand
opens a hinged door (the operator, 2026-11-10): walking up to a shut swing door the hand reaches for
its handle, the door starts to swing at 0.40 m from it (or a body at its face), away from the
person, pushed ahead of them as they walk through with the palm on it; sprints, animals, sliding and
corridor doors still open at 2.6 m (`opening_styles.ron`'s `hand`). On a seat high for the body
the legs hang from it (only the lower legs swing, so nothing clips down through the seat or
stretches). Where the free clip library has no clip (lying, climbing, a bench, a door) the
pose is a target pose with two-bone IK, said so in the design. **Hands**: two-bone arm IK with the
hand turned as asked and a grip library (`data/characters/grips.ron`: relaxed, power, rung, pinch,
staff, two-handed, flat) blended per finger; the equipment's single hands slot split into two grip
sockets for held things (gloves stay worn on the hands). **Look-at**: the head, then the eyes,
toward the camera's aim (the player), another player's yaw and pitch, or the player for a crew
member near and facing them, at a person's turn rate within the head's limits. **True first person**:
the same body drawn for the eye, the head and neck folded to the base of the neck, the camera riding
the drawn head with a low-pass (Settings > Gameplay: Show my body in first person, Reduce motion).
The movement message carries the look's pitch and the act (anchor id and phase), each through one
clamping reader at the relay and the app, left out when idle.
- Crate: `crates/humanity-animation/src/interact.rs` (anchors, grips, the hand rig, `apply_limbs`,
  `look_at`, `collapse_subtree`, phases, weights and warping, the lying pose, `climb_limbs`);
  `FootFrame::pelvis_fixed` (`feet.rs`); `crates/humanity-core/src/stance.rs` (`Doing`, `Act`,
  `AnchorIds`, the pitch)
- Native: `src/engine/interact.rs` (anchors placed in the world, the grants, the capsule held,
  `local_doing`), `src/engine/anchor_probe.rs` (the drawn body against what it touches),
  `src/engine/animation.rs` (each person's act, track, warp, limbs, look-at, first person),
  `CameraController::held` and `Camera::view_offset` (`src/renderer/camera.rs`), the relay passing
  the pitch and act on (`src/relay/handlers/msg_handlers.rs`), Settings' two switches
  (`src/gui/pages/settings.rs`), `MachineDef::anchors`, `StructureType::anchors`,
  `door_anim::opening_styles`
- Data: `data/characters/interactions.ron`, `data/characters/grips.ron`, the furniture's `anchors`
  in `data/machines/*.ron`, the ladder's in `data/blueprints/structure_types.ron`,
  `data/blueprints/opening_styles.ron` (rows with anchors), `data/inventory/equipment_slots.json`
  (two grip sockets), `data/equipment.csv`'s `grip` column
- Dev: showcase verbs `crowd_act`, `act`, `warp`, `hand_ik`, `fp_head`, `fp_body`,
  `reduce_motion`, `rung2b`, `anchor_probe`; vantages `anim-sit-chair`, `anim-sit-couch`,
  `anim-lie-bed`, `anim-ladder-climb`, `anim-workbench`, `anim-door-hand`, `anim-fp-legs`, their
  red twins, `anim-fp-legs-no-body`, `anim-crowd-{near,middle}-2b-off`; the rig's judge
  `scripts/lib/anchor-probe-check.js`

### Crowd navigation: people who walk somewhere (rung 1, 2026-11-13)
People walk to destinations through doors, round furniture and round each other
(`docs/design/crowd-navigation.md`, every rung planned there). A NAVIGATION MESH of one storey of a
region is built from what the walk collides with (the walls and doors as `wall_collision`
segments by the walk's own height rule, every placed machine and piece of furniture as a turned
footprint, the floors of the ship's zones, corridors and slabs from its layout): 0.1 m cells whose
centres keep a body's 0.25 m radius clear, merged into convex rectangles joined by portals, rebuilt
only when the fingerprint of what it was built from changes (a table moved), and every frame with
the Dev page's "Turn off all reuse" switch off. Paths are A* over the rectangles, string-pulled by
the funnel algorithm; people avoid each other by ORCA (optimal reciprocal collision avoidance) and
keep off the walk's edge; a body's centre never enters furniture or a wall. The dev crowd's walkers
(the `crowd` verb, aboard) use it and open automatic doors as they come; the relay's crew adopt it
next (design section 9). No doorway queueing yet (rung 2): a walker stuck 4 s gives its destination
up.
- Native: `crates/humanity-ship/src/crowd_nav/` (`mesh.rs`, `path.rs`, `avoid.rs`, `crowd.rs`,
  `floor.rs`; in the relay's build too), `src/engine/crowd_walk.rs` (the dev crowd on it; its cost
  is the stage `cpu.crowd_nav` inside `cpu.animation`), the crowd as door actors in
  `src/engine/home_meshes.rs`
- Data: `data/crowd/walking.ron` (time horizon, neighbours, personal space, dwell, give-up; each
  number's source in the design)
- Dev: the `crowd` verb's sixth field (`nav`, `nav:<roam_m>`, or `circle` for the old circles);
  vantages `crowd-nav-mess-hall` and `crowd-nav-doors`; tests `crowd_nav::tests::*` (a path through
  the door, no body in furniture over a minute, two walkers head-on pass, the mesh rebuilt only when
  furniture moves, the crowd's region holding while it walks, the shipped ship's Commons to the
  home)

### Livestock (v0.751)
CreatureRegistry parses all 92 `data/creatures.csv` species (new `renewable_product`
column: `item:amount:regrow_seconds`). Starter herd (chickens/goats/sheep) placed by
`data/entities/livestock.ron` near the outdoor field plots, wandering on per-animal
lissajous graze paths. Walk-up `[E]` collects the regrown product (egg/milk/wool)
volume-gated into the pack, grants farming XP + a `harvest_<creature>` quest event.
Placeholder block bodies sized from real species mass.
- Native: `crates/humanity-systems/src/livestock.rs` (registry, spawn list, collect, LivestockSystem),
  `crates/humanity-core/src/ecs/components.rs` (`Creature`, `Harvestable`), lib.rs walk-up + collect bridges + render pass
- Data: `data/creatures.csv`, `data/entities/livestock.ron`, `wool_0` in `data/items.csv`
- **Animal care (2026-10-12, first-hour audit S6):** each kept animal eats real dry matter a
  day from home storage (the machines' `home_stock` channel; which items are feed is
  `data/livestock/care.ron`) and drinks from the home's water tanks, then stored tap water.
  Short of either it makes less, then nothing; Settings > Gameplay > Farm animals: Realistic
  (it then sickens and dies, and stays dead across restarts: `WorldSave::herd_lost`) or
  Forgiving (production only). Yields are per real day at 1x (a hen every 26 h, a goat 3 L a
  day, a sheep's fleece a year), timed in f64. The walk-up prompt says how the animal is and
  what it needs; Home > Animals lists the herd. Feed is drawn while the player is away, and
  nothing sickens then. Native: `crates/humanity-systems/src/animal_care.rs` (+ `_tests.rs`),
  `src/gui/pages/herd.rs`; data: `data/livestock/care.ron`, `hay_0` (items.csv, trade_goods.ron,
  30 bales in the seeded Barn).

### Abilities (v0.753)
AbilityRegistry parses all 110 `data/abilities.csv` rows (renamed from spells.csv;
new `flavor` column: real | tech | fantasy). AbilitySystem drains `ability_request`
casts: skill gate (level-1 gates baseline-open), energy cost (mana + stamina columns
both pay from the energy vital), live cooldowns. v1 effects are self-scoped healing
(first_aid, cauterize, repair, heal...); offensive rows load but honestly wait for
the combat arc. Profile > Skills gains the Abilities panel with Cast buttons.
**Building abilities (BUG-153, 2026-10-05):** a row whose `builds` column names a
blueprint builds that piece where a piece in hand would go (the engine's spot,
`abilities::BUILD_SPOT_SLOT`), through `construction::begin_build`, spending its
energy only when the build starts. The Campfire ability builds a campfire
(`construction::fires`: lit with its own logs, burns down, E adds a log, radiates
heat the body heat model feels; outdoors only).
- Native: `crates/humanity-systems/src/abilities.rs`, `src/gui/pages/profile.rs` (panel),
  lib.rs bridge (`pending_cast`, `ability_status`, `ability_cooldowns`),
  `src/engine/build_place.rs` (`publish_cast_spot`), `crates/humanity-systems/src/construction/fires.rs`
- Data: `data/abilities.csv`, `data/blueprints/basic.ron` (`campfire`)

### Player Controller
WASD movement, gravity, jump, ground detection via raycast.
- Native: `crates/humanity-systems/src/player.rs`

### Interaction System
Raycast from camera, find nearest interactable entity.
- Native: `crates/humanity-systems/src/interaction.rs`

### Day/Night Cycle
One game clock (2026-09-27, decision-briefs.md Brief 6): an hour is 3,600 game seconds, a day is the Settings "Hours in a day" (default 24, 12 to 48), a year the "Days in a year" (default 365), and the "Time speed" (default 1 = real time, presets Realistic 1x, a day an hour 24x, Simplified 72x, Garden testing 720x, up to 1,000x) is the only speed-up: crops, tanks, batteries, the body's daily needs, the weather, the sun, the planet's spin and the seasons all follow it. The sun is up the middle half of any day (`GameTime::solar_hour`). Holds over the speed: the F11 freeze, sleep (7,200x, a night in 4 real seconds) and the probe rig. Offline time counts at the time speed.
- Native: `crates/humanity-systems/src/time.rs`, `src/gui/pages/settings_time.rs` (Settings > Gameplay > Time)

### Weather System
7 conditions (clear, cloudy, rain, storm, snow, fog, sandstorm). Seasonal transitions. **Registered, ticks live** (`WeatherSystem` is NOT in `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` -- this "NOT registered" note was stale, corrected 2026-07-01 during the overnight loop's registration-status sweep).
- **Rain or snow decided by the air (2026-09-27).** The condition says whether water falls and how hard; the air where it falls says rain or snow, by Jennings et al. 2018's rain-snow model (air temperature, humidity and pressure, a smooth band about 3 C wide centred near 1 C in wet air): a Rain roll at the winter pole snows, a Snow roll at the equator rains, a mountain climb crosses the snow line, and nothing falls where there is no air. The body heat input, clothing wetness, the HUD, the weather fog, the rain and snow particles, rain watering fields and the water bodies all read the one answer.
- **The HUD weather line reads the air the player stands in (2026-09-27):** the condition as it falls there ("Rain and snow" inside the band), the temperature at the player, and the wind at the player (environment Layer 1's prevailing wind plus the weather's own) with the compass point it blows from. The F11 panel repeats it as "At the player". Sealed indoors (the home aboard) the line
is the home's air instead, "Indoors 20C, still air" (`hud_weather_text`, 2026-09-28): it used to show the planet's weather
in a room at 20 C.
- Native: `crates/humanity-systems/src/weather.rs`, `crates/humanity-systems/src/precipitation.rs` (phase), `src/gui/pages/hud.rs` (`weather_line`)
- Design: `docs/design/environment-fields.md`, "Rain or snow, decided by the air"

### Hydrological System
Rain cycle, rivers, aquifers, contamination tracking, water table simulation. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/hydrology.rs`

### Atmospheric System
Gas tracking, explosions, suffocation, pressure simulation. **Registered (v0.617), ticks the home's sealed EnclosedSpace + publishes live AirStatus** (`AtmosphereSystem` is NOT in `DEFERRED_SYSTEMS` -- the lint's own comment there notes it was removed at v0.617; this "NOT registered" note was stale, corrected 2026-07-01).
- Native: `crates/humanity-systems/src/atmosphere.rs`

### Disaster System
21 disaster types with chain reactions, severity scaling, black holes. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/disasters.rs`

### Farming
6 growth stages, water/health simulation, seasonal effects. Yield follows the
crop's SEASON health, the time-average of its health while growing
(`farming::season_health`, FAO 33 linear response; v0.1350.0), shown on the
crop card; stage models map to species through `stage_models` in
`data/plants_visual.ron`. Light (v0.1352.0): green crops grow only while lit
(the sun, through the skylight indoors; after dark, the powered `lights_crops` grow
lights near them, each covering about 0.58 m2 of canopy per 100 W, nearest machines
first: `farming/lighting.rs`, `data/garden/lighting.ron`, v0.1364.0) and pause in the
dark; plants.csv `needs_light` is false for fungi. Each crop's own light need
(plants.csv `dli_target` for 27 crops, v0.1368.0) sets its share of a grow light, the
lights run on an 18 h timer (off at midnight, Runkle), and the crop card shows the need.
Picking windows (v0.1369.0): tomatoes, peppers, cucumbers, zucchini, strawberries, kale
and cut herbs are picked over a cited window at a cited interval, their season split
into picks that sum to the cited season yield (`farming/picking.rs`,
`data/garden/harvest_windows.ron`); seed return follows what was harvested; Settings:
Picking Forgiving/Realistic. Grow machine cards show kcal a day computed from their
crops (`systems/grow_machines.rs`), the same figure the Home page's food loop sums.
Each bed, tray or field plot is drawn with the plants it holds at the crop's spacing, up
to 128 per plot and then as wider clumps, within a per-plot vertex budget
(`engine/plant_layout.rs`, `data/plants_visual.ron`; only changed machines rebuild).
Mushroom crops are drawn as the substrate they grow from (2026-09-27): oyster and shiitake
as 5 lb fruiting blocks of the supplier's size, two to a rack shelf, colonising white in
their filter-patch bags (shiitake browning and fruiting bare), button as a cased compost
bed across the shelf; the fruit follows the stage, pins then full flushes (oyster clusters
from the bag's X cut, single shiitake caps, white buttons through the casing), drawn rigid
so nothing sways in the tent (`engine/fungus_mesh.rs`, `data/plants_visual.ron` `fungi`).
Heaters (2026-10-05, BUG-155; the heat pump and the wood stove 2026-10-20): three
placeable machines warm the air they stand in (a grow room's or a tent's, else the
home's own) on a 24 C thermostat (`farming/heat.rs`, THE HEAT in
`data/garden/humidity.ron`). The space heater (`heater`, 1,500 W, its heat its draw);
the heat pump (`heat_pump`, a Mitsubishi Electric MSZ-FS09NA: 620 W for 2,815 W of
heat, its COP read off its spec sheet's table at the station air's temperature,
`heat_cop` / `HeatPump`, charged only its draw by the meter and the time away); and
the wood stove (`wood_stove`, an EPA-certified Drolet Escape 1800: 13,040 W, no power,
4.07 kg of Wood Logs an hour from its own 68 L `stove_firebox`, which takes only Wood
Logs, `VesselTakes::Firewood`; burned exactly in f64 at any frame rate; its fire burns
1.094 kg of oxygen per kilogram, its exhaust goes up the flue; a half-burned log is saved,
`MachineLevels::stove_burned_kg`; `FuelStove`, `burns_fuel`). Where a stove's fire gets
its air is data (2026-10-20, `burns_fuel.combustion_air`, `CombustionAir`): the shipped
stove has its maker's outside air kit (`Outside`, per 24 CFR 3280.709(g) and Drolet's
manual, docs/reference/findings/2026-10-20-stove-combustion-air.md), so it takes none of
the home's oxygen and the ship's supply ledger meters it on an "oxygen" line in kg
(`ship_power::OXYGEN`, both Ship life support modes); a stove with `Room` takes it from
the home's air as before. Each is
built at a workbench (`build_heater`, `build_heat_pump`, `build_wood_stove`) and placed
from the Construction page's Life support tab; neither home places one. The Garden panel
names each kind ("heat pump flat out, 2,815 W of heat on 620 W", "stove off (no wood)"),
says where a burning stove's air comes from ("and its air from outside through its kit" /
"from this room"), keeps a room-air stove's oxygen apart from breathing, and gives an
outside-air stove's day from the ship's air apart from the home's balance. Tests:
`farming::heat_tests` and
`engine::home_spawn::tests::the_heat_pump_and_wood_stove_are_built_and_placed_and_the_stoves_box_takes_only_wood`.
Greenhouse air (v0.1370.0): each grow room gains the water its crops breathe out and
loses it to leakage and an exhaust fan (`exhaust_fan_1`, humidity-triggered, cube-law
power), relative humidity from the FAO-56 formula; gray mold, powdery mildew and downy
mildew are pests.ron rows favoured by humid air, with ventilate, remove leaves, sulfur and
potassium bicarbonate controls; a crop outside its plants.csv humidity range is capped
(`farming/humidity.rs`, `data/garden/humidity.ron`). A plant picked over a season keeps
drinking, breathing and taking stress through its window. Beds, trays, fields and racks are
divided into plots (grow_media.ron `plots`), one crop per plot, each crop tagged with
the machine it stands in. A plot holds as many plants as fit at the crop's cited
spacing (plants.csv `area_per_plant_m2`; a tower cup is one plant) and yields a
cited harvest per plant, with water and nutrient need per plant (`farming/units.rs`,
`data/garden/yields.ron`, v0.1367.0). Pollination (v0.1367.0): an indoor fruiting
crop sets its cited share without help; Hand-pollinate and a bumblebee hive
(`bumblebee_hive` in the machine catalog) restore full set; fields set fully;
Settings: Pollination Off/On (`farming/pollination.rs`, `data/garden/pollination.ron`).
Soil pH (v0.1366.0): each soil plot has a pH remembered
between crops (beds start at 6.5; tower solutions are held at 6.0); nitrogen from urine
and fertilizer acidifies it, garden lime raises it and garden sulfur lowers it after
their delays, buffered over the plot's real floor area; a crop outside its plants.csv
`ph_min`..`ph_max` has its health capped; Lime and Sulfur buttons per grow area;
Settings: Soil pH Off/On (`farming/soil_ph.rs`, `data/garden/soil_ph.ron`).
Nutrients (v0.1355.0):
each unit holds grams of N, P2O5 and K2O (`CropSoil`), crops draw their
season need, the scarcest nutrient caps health, compost adds its cited
first-season nutrients, the "nutrient" slider runs a feeder from home
storage, and the soil is saved (`farming/soil.rs`, `data/garden/nutrients.ron`).
Nitrogen sources (v0.1359.0): compost banks the organic N it does not release
in year one and releases it over the following years; stored urine (collected
on the Compost action) is an all-available N fertilizer withheld a month
before harvest; legumes draw only their unfixed share and leave a credit
(plants.csv `n_fixed_pct`). A crop's season need is its expected harvest
times its cited removal per kg (plants.csv `removal_n_g_per_kg`,
`removal_p2o5_g_per_kg`, `removal_k2o_g_per_kg`, 64 crops, v0.1363.0, sources
in `data/garden/nutrients.ron`), else its index read against the tomato
anchor; the crop card shows what each kg takes. Pests (v0.1361.0): aphids, spider mites, cabbage
caterpillars, slugs and Colorado potato beetles build up per grow area on their
host crops under their favouring conditions and cap crop health; the Garden
panel lists each with its controls in IPM order (hose off, hand-pick, Bt,
predatory mites, insecticidal soap made from the player's soap, iron phosphate
bait); rotation lets pressure decay; Settings: Off/Gentle/Realistic
(`farming/pests.rs`, `data/garden/pests.ron`). Weeds (2026-09-26): every soil grow
area holds a weed cover and a seed bank (saved), weeds come up from the bank and cap
crop health hardest in the crop's cited critical period at WSSA's season-long losses;
Hoe (wears the backpack hoe) and Mulch with sawdust or bark (spends them, stops most new
weeds for a cited time, ties up nitrogen as it rots) per soil area; the pest severity
setting covers weeds (`farming/weeds.rs`, `data/garden/weeds.ron`). Plantable grow
areas beyond towers (v0.738 grain loop): beds/trays/fields render as Garden
groups with a Plant button (crop = edit-modal Crop field or the medium's
`default_crop` in grow_media.ron; survival consumes one seed per unit,
replant is idempotent); bed crops carry the grow-area MACHINE id in
CropInstance.tower_id + tower_slot = unit index, so grouping/irrigation/
harvest reuse the tower paths. 8 `grain_<plant>_0` harvest items (dry_goods,
silo-compatible) + fruit_sunflower_0. Garden slots render as a compact TILE
GRID with one detail card (v0.739) and each group has a "Harvest N ready"
bulk button (`harvest_many_request` channel). Harvest overflow routes to a
compatible home vessel (v0.729 silo routing). A seed's Plant button in the
backpack (2026-10-12, first-hour audit F11) sows it into the nearest free
tower cup, bed, tray or field plot within 5 m of where the player stands (a
mushroom's spawn only onto a rack, nothing into a fish tank: grow_media.ron
`takes`), so it is drawn and irrigated like any machine crop; with nothing in
reach it plants nothing and names the nearest machine with room
(`farming/sowing.rs`). The notice and each crop's card say when it ripens in
real time at the clock's speed and what its water needs (`farming/outlook.rs`;
lib.rs fills `GardenPests::outlook`). The Garden section lays out only the
groups and rows of tiles on screen (2026-10-31): one wholly off screen takes
its remembered size, kept under a key of everything that decides it and off
with the Dev page's "Turn off all reuse" (`gui/widgets/offscreen.rs`, the
reusable helper), so a bigger farm costs no more to look at. The farm runs on
its own clock (2026-10-31, `farming/clock.rs`, docs/design/farm-clock.md rung 2):
each grow area advances once per interval by exactly the time it owes, on its
own turn, the air on its own turn at least every game second; Settings >
Gameplay > Garden updates (every frame, every second by default, every minute,
every hour). Whatever reads a crop (a command, the Garden page, the plant
meshes of the grow areas in the camera's view) brings its area up to now
first, inside the frame's home-storage window so its fertilizer draws leave
the containers (`engine/stock_piles.rs`); areas in view take 1 s turns. The
save writes the garden as it stands with what each area still owes
(`WorldSave::farm_owed`), and the load owes it again. The Garden page keeps
each grow area's rows and tile keys until that area's change number
(`farming/versions.rs`), a data file it reads or the clock's words move
(`engine/garden_mirror.rs`); a wall screen builds at most an eighth of the
crops a build. "Turn off all reuse" runs every area every frame and builds
every row every build.
- Native: `crates/humanity-systems/src/farming/mod.rs`, `src/gui/pages/inventory.rs` (Garden), `src/gui/widgets/offscreen.rs`
- Data: `data/plants.csv`, `data/garden/grow_media.ron`, `data/items.csv`

### Inventory
ItemStack slots, add/remove/transfer, max stack from data. Volume tracking
(v0.726, material-storage Stage A slice 1): every item carries `volume_l`
(items.csv column generated by `scripts/gen-item-volumes.js` from mass /
material density × per-category packing fraction; `RECOMPUTE_MATS=` env to
refresh a material's rows); `Inventory.volume_current_l` recalculated every
tick beside weight, `volume_capacity_l` default 65 L; the Inventory page shows
a Volume tile + per-item Volume detail row. Slice 2 ENFORCEMENT (v0.727):
`add_item_volume_gated` caps by remaining litres on transfers/crafting/
harvest/compost; `outputs_fit` volume headroom pauses auto-machines; a
volume-legal add GROWS the slot grid (v0.735 — volume is the real pack limit,
raw `add_item` stays slot-bound for bandolier-likes). Transfer UX (v0.736):
right-click an item tile for a Stash/Move/Take context menu at the cursor,
or DRAG a tile onto any container header (accent highlight + floating label).
Auto machines draw recipe inputs from HOME STORAGE backpack-first (v0.737),
the build menu too, and hand crafts since BUG-147: the main loop mirrors the
organize-layer placed items into a `home_stock` map pre-tick (leaving out
chests built on a planet, whose contents are not in the home); the systems
count + consume; the diff drains back post-tick.
- Native: `crates/humanity-systems/src/inventory/mod.rs`, `src/gui/pages/inventory.rs`,
  `crates/humanity-systems/src/crafting/mod.rs` (home_stock), `src/engine/stock_piles.rs`
  (`publish_home_stock`, `take_consumed_home_stock`),
  `crates/humanity-systems/src/inventory/placed.rs` (`stock_counts`, `take_consumed`)
- Data: `data/items.csv` (volume_l), `data/materials.csv` (densities)

### Typed Containers (volume-capped vessels)
Volume-capped, content-class-typed vessels (barrels, tanks, jars:
capacity_liters, no-mixing, wrong-class damage/breakage) with registry +
tests. WIRED (v0.728+): `MachineDef.container_type` in home.ron spawns a
`Container` component on the machine entity (grain silo bin, steel fuel
drum); the walk-up machine card shows "Holds: Nx item" + Take + per-item
Store buttons; harvest overflow fills them; the backstop
genset burns only the fuels its data names (`fuels` on its `Generator` power
role, spawned as `BurnsFuels`; BUG-154), and its drum takes and offers only
those (`vessel_takes_item`, `store_offers` in `containers.rs`;
`crates/humanity-systems/src/electrical.rs`). The homes' genset names Gasoline, which the fuel
refinery distils from crude oil along with Diesel at EIA's split (`refine_crude`:
10 crude oil of 1.25 L into 6 L of gasoline and 4 L of diesel; both are litre items at their
GREET densities, cited in `data/items.csv`; 2026-10-20, replacing "Refined Fuel").
The refinery's drum is its feed (2026-10-20): it draws its recipe's crude oil from
the drum first, then home storage, in a session and through the time away, and its
products go to home storage; so the drum takes and offers only the feedstock its
recipe names (`VesselTakes::Feed`, `vessel_takes`, `feed_items`), where it took any
liquid, and the card's note says why the rest have no Store button (`refused_note`;
a generator's drum says the same of its fuel).
The diesel burns (2026-10-20): both homes' catalogs carry `generator_diesel`,
"Generator (diesel)", a Kubota GL-7000 class set (6.5 kW prime output on 2.6 L of
diesel an hour at full load, 0.40 L a kWh against the gasoline set's 0.63, 235 kg;
its spec page quoted in `data/machines/home.ron`). Not placed by default: built at a
workbench (`build_generator_diesel` in `data/recipes.csv`) as `generator_diesel_0`
and placed from the Construction page's Power tab like any machine; a backstop whose
drum takes, offers and burns only Diesel (`fuels: ["diesel_0"]`, `vessel_takes`).
Test: `engine::home_spawn::tests::the_diesel_generator_burns_diesel_and_nothing_else`.
Always pre-check `registry.check().is_accepted()` before `try_store` — a
wrong-class store DAMAGES the vessel by design.
Content memory and materials (v0.1345.0): a vessel remembers its last
content and any toxic history, needs a Clean (water from the tanks) before
switching contents, never holds food again after fuel or solvents, and its
material (`data/containers/materials.csv`) refuses what it reacts with.
- Native: `crates/humanity-systems/src/inventory/containers.rs`, `crates/humanity-ship/src/machines.rs`
- Data: `data/containers/types.csv`, `materials.csv`, `content_traits.ron`, `data/machines/home.ron`
- Design: `docs/design/containers.md`

### Quality Grades for Crafted Goods
Hand-made durable goods are graded (defective to masterwork, from
data/manufacturing.ron) by the crafter's skill level; the grade scales a
tool's uses and its sale price, never merges across stacks, and survives
storage. Machines make standard goods (v0.1353.0). Above standard the
trading post pays the grade's multiple only up to a ceiling set by what
the good's parts cost there, so no grade loops money (BUG-146, v0.1463.0).
- Native: `crates/humanity-systems/src/crafting/quality.rs`, `crates/humanity-systems/src/inventory/mod.rs` (`ItemStack::quality`, `add_item_q`, `TransferOp`), `crates/humanity-systems/src/economy/mod.rs` (`vendor_sell`), `crates/humanity-systems/src/economy/parts.rs` (`graded_pay`, `parts_prices`)
- Data: `data/manufacturing.ron` (`quality_levels`)

### Byproducts and Machine Output to Storage
Recipes leave real byproducts at sourced ratios (slag, sawdust, bran, press
cake, olive and apple pomace, whey; ratios and sources as # comments in
data/recipes.csv), each consumed by another recipe (slag concrete, compost,
whey bread); `tests/byproduct_use_lint.rs` keeps them used and stops
byproduct recipes creating mass or multiplying an input. Automated machines
deliver into home storage (the Barn) and rest while `auto_keep` of their
product is on hand (v0.1351.0).
- Native: `crates/humanity-systems/src/crafting/mod.rs` (`home_stock_outputs`, keep gate), `src/engine/stock_piles.rs` (`receive_machine_outputs`)
- Data: `data/recipes.csv`, `data/items.csv`, `data/machines/home.ron` (`auto_keep`)

### Station Power (idle and working draw)
A work station's power role can carry `idle_watts` (data/machines/*.ron): it
draws that until a craft runs at it and its full `watts` while one does. A
manual craft at an electric station with no powered machine of its type is
refused (notice, nothing spent) and the Crafting page says the station has no
power (v0.1349.0).
- Native: `crates/humanity-systems/src/crafting/mod.rs` (`station_unpowered`, the draw pass at the end of the tick), `src/engine/home_spawn.rs` (`MachineType`, `StationLoad`), `crates/humanity-ship/src/machines.rs`
- Data: `data/machines/home.ron` (stove, oven, electronics bench, sewing machine)

### Hand Tools and Tool Wear
A manual craft needs its hand tools in the backpack (data/crafting/tools.ron:
rules by station and category, plus per-recipe lists); tools are not consumed
but each craft wears one use off them, and they break at their items.csv
durability. The inventory card shows "Uses left". Automated machines need no
tools. The web Crafting page lists the same tools (v0.1348.0).
- Native: `crates/humanity-systems/src/crafting/tools.rs`, `crates/humanity-systems/src/crafting/mod.rs`, `crates/humanity-systems/src/inventory/mod.rs` (`wear_item`), `src/gui/pages/crafting.rs`
- Data: `data/crafting/tools.ron`, `data/items.csv` (durability)
- Web: `scripts/gen-recipes-json.js` -> `data/recipes.json`, `web/pages/crafting.html`

### Tap Water and Water Vessels (fluids as litres)
A recipe that needs a measure of tap water draws it from the home tanks when
the backpack has none (the Crafting page counts the tanks too); a water
tank's walk-up card fills the empty bottles and jerrycans you carry and
pours full ones back; drinking a bottle hands the empty back (v0.1347.0).
- Native: `crates/humanity-systems/src/fluids.rs`, `crates/humanity-systems/src/crafting/mod.rs`, `crates/humanity-systems/src/food.rs`
- Data: `data/containers/fluids.ron`, `data/items.csv` (water_bottle_empty_0, water_jerrycan_empty_0, water_jerrycan_0)

### Visible Storage (Barn racks and tank gauges)
The Barn's storage zone builds open pallet racks (`ZoneFiller` mesh_kind
"rack"), and whatever is filed in the Barn (the organize layer's places tree)
shows as 96 L crates on their decks, bottom deck first, updating as the stock
changes. Every water tank carries a level gauge (v0.1346.0).
- Native: `src/engine/stock_piles.rs`, `crates/humanity-ship/src/structure.rs` (`rack_deck_levels`, `ZoneFiller::cells`)
- Data: `data/places/seed.json` (the seeded Barn store), `data/blueprints/zone_filler.ron`

Every other district kind has its own shape too (v0.1434.0,
`structure::filler_parts`): market stalls (counter, back shelf, posts, roof)
in the mall, medical and arena zones; parking spots for craft (floor rails with chocks; called cradles until 2026-11-07) in
hangars and mech bays; machine arrays (plinth, machine, riser pipe, ceiling duct)
in industrial, power and agriculture zones. A new kind is one match arm.

### Machine Walk-Up Cards (live)
Distance-LOD machine labels (dot → name → stat card) with LIVE values
(v0.724): a per-frame pass patches each label's stats from its entity via
`MachineInstanceId` — cistern litres from the plumbing sim, battery kWh from
the electrical sim, "low" status under 15%. The pinned (E-opened) card grows
an "Auto-build:" recipe dropdown (v0.725): same-station recipes.csv rows,
selection rewrites the entity's AutoRefine (home.ron auto_recipe = default
only). The dropdown is its own interactable Area — the HUD layer is
deliberately paint-only (v0.461 click-eating lesson).
- Native: `src/gui/pages/hud.rs` (cards + `draw_machine_recipe_selector`), `src/lib.rs` (live-stat + selector bridges), `crates/humanity-core/src/ecs/components.rs` (`MachineInstanceId`)

### Crafting
Recipe matching from CSV, input validation, timed crafting.
- Native: `crates/humanity-systems/src/crafting/mod.rs`
- Data: `data/recipes.csv`

### Construction
Blueprint placement, snap-to-grid, timed building, material consumption. **Registered, ticks live** (`src/lib.rs`
`system_runner.register(...ConstructionSystem::new())`; the old "NOT registered" note here was stale, corrected
2026-09-25). Materials are real item ids (BUG-082, 2026-09-25: the catalog had named items that did not exist,
so nothing could be built). Finished structures and scaffolds persist in the save. **A built structure is a
workstation** when its blueprint lists `stations` (a Furnace works as a smelter and a kiln, a Crafting Table as
a workbench), feeding the same recipe station gate the home's machines do; the Crafting page says "or build a
Furnace" when a station is missing.

**A built bed and chest do something (2026-09-27).** Looking at a finished structure within 5 m shows an
[E] prompt when its blueprint's `provides` names a use (`systems/construction/uses.rs`; any blueprint can opt
in by data, no ids in code; the first structure the look ray meets wins, so a wall hides what is behind it):
- `provides: "rest"` (the Bed): E lies down and sleeps 8 game hours. The clock runs 120 times faster through
  the night (the time scrubber's channel, so crops, crafts and the sun live the night by their own rules;
  never a clock jump), then the player wakes with full energy, no `fatigued`, the `rested` buff, the clock back
  at its old speed, and a notice saying when. Dying asleep restores nothing. `crates/humanity-systems/src/sleep.rs`, run by
  FoodSystem. The Inventory page's button is a **Short rest** since 2026-09-27: a ten-minute nap (Brooks and
  Lack 2006, Sleep 29(6)) that repays ten minutes of sleep (1.6 energy points) and lifts the `fatigued` slowdown
  for 155 minutes (the `refreshed` buff) for a tired person, not an exhausted one (energy below 10); a second
  nap inside the window adds nothing. Only the bed restores energy (`sleep::short_rest`).
- `provides: "storage"` (the Storage Chest): every finished one is a container in the Inventory page's "You &
  your places" tree (a keyed node, `gui/organize.rs` `sync_built_stores`), so drag onto its header, right-click
  Stash to / Take to backpack / Move to all work on it, and its contents count as home storage for crafting and
  building (a chest built in the home; one built on a planet holds what is on that planet). Its items are placed items filed under `built:<uid>`; `Structure.uid` is saved with the structure,
  so the chest comes back at the same address and its contents with it. E on the chest opens the Inventory
  page. No volume limit yet: no places container has one.
- `provides: "shelter"` (walls, roof) **shelters (2026-09-27).** Standing outside under a finished shelter
  piece overhead (the roof: whichever shelter piece is straight above the head, not an id or category) with
  finished shelter pieces on at least THREE of the four sides within half a metre of the edge of the COVERED
  AREA (the roof overhead and every roof touching it, so a hall under several roof tiles counts:
  `uses::covered_run`) (`uses::shelter_at`, level rays at chest height), with the open side away from the wind,
  sets `EnvironmentContext::sheltered`,
  on a planet's ground as well as aboard (the test runs in the player's build site), so the body heat
  model gets still air and nothing falling at the outside temperature. The fourth side is left open as the way
  in, like a lean-to (or, since 2026-09-28, a room closes with a Wood Wall with Door). On a wet, windy 5 C day the everyday outfit holds
  36.66 C after 6 h under it, against 36.28 C soaked in the open. A roof with walls missing keeps the rain and
  snow off, and since 2026-09-28 the WIND depends on which way it blows (`ShelterCheck::wind_share`): a wall on
  the windward side stops it, and each open side facing into it lets in the cosine of the wind's angle to it, so
  three walls keep the wind off only with the open side turned away from it ("back to the wind"). The HUD's
  survival rows say "Sheltered", "Out of the rain and the wind", or "Out of the rain; 70% of the wind gets in",
  and the Inventory page's readout says Sealed / Sheltered / Outside.

**Walls on uneven ground meet at the top (2026-09-28).** A piece standing on the ground that touches a finished piece
of the same kind and built height whose top is within 0.3 m of its own is made that much taller or shorter, its
bottom on its own ground, so the tops meet and a roof rests flush (`placement::level_top`). Before this the roof, which
rests on the tallest wall, left a sliver of sky over a lower one on uneven sand.

**Doors in walls (2026-09-28).** A blueprint can carry a `doorway` (width and height): the Wood Wall with Door
(`data/blueprints/basic.ron`, 9 planks) is one piece to place, save and take down, drawn and walked into as its parts
(`systems::construction::doorway::parts`: the wall either side of the gap, the lintel, and the door leaf, drawn in a
darker wood so a shut door reads against its wall; vantage `planet-built-door`). Looking at it
the crosshair says "[E] open or shut the door"; E toggles the `DoorOpen` marker (with the home's own door sounds), and an open door swings a
quarter turn out of the gap and stops blocking. An open door stays open across a save (`ConstructionSave.open`).

**Windows in walls (2026-09-28).** A blueprint can carry a `window` (width, height, sill): the Wood Wall with Window
(7 planks and a glass pane) has a 1 x 1 m pane 0.9 m up (`doorway::window_parts`: the wall around the opening and the
pane). The pane is solid to walk into and is drawn in the transparent pass with the home's own window glass: aboard in the
scene's transparent list, at a planet build site in the celestial transparent list AFTER everything else, water included
(`celestial_order::list_key`), so the sky and the ground seen through it are already there when it blends. Vantage
`planet-built-window`.

**Built pieces are solid (2026-09-28).** Every finished piece in the home frame whose box reaches from
above the knee to the eye (a wall, a bed, a chest, a machine) joins the home's own walls in the first-person
wall resolver (`engine::build_place::built_piece_segments` into `ship::wall_collision::resolve`), so you no longer
walk through what you built; roofs overhead, floors underfoot and scaffolds still going up do not block. On a planet's
ground the player moves by the frame lock's anchor, so the walk's step is resolved in the build site's own frame
against that site's pieces (`engine::planet_build::collide_on_site`); dev fly mode stays noclip. On a planet the walk
also STANDS on them (2026-10-20): its floor is the higher of the terrain and the top of a finished piece under the body
within a 0.6 m step up of its feet (`engine::planet_build::built_floor_radius`, the aboard walk's `footing::STEP_UP_M`,
taken into `surface_move::walk_rest`), in the build site's frame and in f64 to the answer, so a roof, a wall's top or a
foundation holds a player up, a body climbed over a roof lands on it, and walking off one's edge is a fall like any other,
its landing hurting the way data/medical/falls.ron says (proven in the running game by `just verify-falls`' planet deck
leg). Before, the planet walk's floor was the terrain alone and a player went straight through a roof. So nobody can be walled
in, a piece can be TAKEN DOWN: with a piece in hand, the Swing tool key (F) takes down the finished piece in view
within 8 m and puts all its materials back in the pack (what does not fit goes to storage, through the "Take to
backpack" channel; on a planet that storage is the home's, in orbit); a chest that still holds anything is refused until it
is empty (`build_place::take_down_plan`). The look ray (E and F alike, `uses::first_in_view`) meets a door or window wall as
its parts, so it goes through an open doorway, and a scaffold in front hides what is behind it (review of 2026-09-28), and
aboard it stops at the home's own walls (`uses::ray_crosses_walls` against `wall_colliders`), so a chest in the next room
cannot be opened or taken down through one.

**Placing a built piece (2026-09-27).** Build on the Crafting page's Structures now puts the piece IN HAND
(`engine/build_place.rs`): the page closes and a half-dithered scaffold ghost follows the crosshair, on the floor
point the look ray meets (1.5 to 8 m ahead, 4 m when looking level). The Toggle roof key (R) turns it a quarter,
Interact (E) builds it where the ghost stands and keeps it in hand for the next one, Esc puts it down; the hint
under the crosshair says so with the player's own keys. Where it lands is one function,
`construction::placement::placement_pose`, used by the ghost and the ConstructionSystem alike: x and z on the
metre grid, the turn saved in the Structure's rotation, and y on the floor or, for a blueprint with
`mount: OnTop`, on top of the tallest finished `snap_to` piece its footprint covers (a roof on the walls at 3 m,
walls on a foundation at 0.2 m), resting only on pieces standing on the player's own storey and never on a
few centimetres of overlap. The ghost's pose is kept every frame and E builds exactly that pose; a second
build of a box that already stands (or is still going up) is refused before anything is spent, and the hint
says "already built here". While a ghost stands, the machine, door, talk, vehicle and livestock "[E]" prompts
hide, since E builds. Before this every piece went 4 m ahead at world y 0, unturned (and mirrored
left-right off the look direction by a sign slip in the old bridge).

**Building on a planet (2026-09-27, BUG-102 fixed).** On a planet's ground a piece goes into a BUILD SITE
(`construction/site.rs`): the body and an origin in its unrotated frame in f64, with a flat Y-up tangent
frame; a new piece joins the site of the nearest piece within 1 km of it (and its grid), and the player is
at the site of the nearest piece within 1 km. The ghost meets the drawn ground under the
crosshair (`placement::aim_point_on_ground`, the surface the player's feet stand on), the shelter test and
the bed and chest look ray run in the site, and the save carries the site. Site pieces draw with the
terrain's own `render_off` and `rot_d` in the celestial pass only, at every distance (hidden by hills,
casting the sun's shadow, and behind whatever stands in front of them); the celestial pass's near plane is
5 cm (`terrain::celestial_projection::CELESTIAL_NEAR_M`, one shared constant), so a wall beside the eye is whole
(`engine/planet_build.rs`). The piece in hand on a planet is drawn in the scene pass, on top and without a
shadow, as a preview. On a planet a piece is built from what the player CARRIES (the home's storage is in
orbit; the hint under the crosshair says what is missing), a station built there is on no power grid until
its own site makes power (the home's grid is in orbit; the Crafting page says so), and the stations that
count for crafting are the ones where the player is: aboard, the home's machines and pieces built in the
home; at a planet site, that site's built stations; elsewhere, none (`engine/built_uses.rs`
`publish_stations`, `CraftingSystem::station_unpowered_at`). Building refuses, with a plain hint, in open
space, in a vehicle, while flying and on water. Dev (no dev gate, like every showcase verb): showcase
`{"build":"id@dx,dz,turns;...","build_at":"lat,lon"}` stands pieces up finished (the rig's
`planet-built-shelter` vantage uses it through `post_showcase`), and `{"stand":"dx,h,dz,heading,pitch",
"stand_at":"lat,lon"}` puts the eye at a point of that site (the `planet-built-inside` vantage stands inside
an open hut with a chest and a furnace in front of its north wall, through probe-sweep's new
`final_showcase`, sent after the re-park). `{"walk":"1"}` (v0.1420.0) then turns fly mode off, so the
survival rules and the HUD's outdoor lines run as for a player on foot; every park leaves fly mode on, so
before it no capture could show them (`planet-open-noon-walk`). `{"hold":"forward","hold_s":"100"}`
(BUG-156, 2026-10-05) presses movement keys for that long through the controller's own action path, so a
capture can ARRIVE on foot the way a player does instead of by teleport (`engine/rig_walk.rs`), and
`{"tree_ground":"1"}` writes `debug/tree_ground.json`: every near tree's base and the eye against the
ground drawn under them, the finest ground and the surface the harvest sampled (`engine/tree_ground.rs`).
A vantage with `ground_probe` has probe-sweep read it after the capture and fail the vantage when an
on-screen tree floats or the eye is not standing on the drawn ground (`scripts/lib/tree-ground-check.js`).
Still missing: a door or window set INTO a wall (they sit on the floor); a second storey (on a planet a player
stands on a roof since 2026-10-20, but pieces are placed only from the ground, and aboard nothing stands on a built
roof or foundation yet: the aboard walk's floor is the rooms and the ship's own structures, `engine::footing`); a
built roof that stops a climb from below (on a planet Space climbs up through one, and once the feet are within a
step of its top the body is stood on it); the one canonical layout schema (built
pieces, the home editor's `InteriorWall`s and the ship structure pieces are three different shapes); pieces
on a planet stand at the drawn ground's height where they were built, so far off, where the terrain draws at
a coarser level, they can sit a little high or low; a planet piece's face turned from the sun reads near
black; a generator that can be built at a site (nothing on a planet can make power yet, so a planet stove
or oven stays unpowered).
- Native: `crates/humanity-systems/src/construction/mod.rs`, `crates/humanity-systems/src/construction/placement.rs`,
  `crates/humanity-systems/src/construction/site.rs` (build sites), `crates/humanity-systems/src/construction/uses.rs`,
  `crates/humanity-systems/src/sleep.rs`, `src/engine/built_uses.rs` (prompt, E press, chest sync, the stations where the player is),
  `src/engine/build_place.rs` (the piece in hand), `src/engine/planet_build.rs` (the frame, the ground, the
  draw, the dev build verb), `src/engine/survival_env.rs` (shelter to the body), `src/gui/organize.rs`
  (`place_path`, `sync_built_stores`), `src/gui/pages/hud.rs` (the prompts and the Shelter row)
- Data: `data/blueprints/basic.ron` (`mount`, `provides`, `snap_to`)

### Body heat (2026-09-27)
The core temperature is a heat balance, not a drift toward the air. Before this, the core moved toward the air
temperature at 0.5 C a second, so stepping outside on a 15 C day was hypothermia in 4.5 s and death about 47 s
later. Now it is the **Gagge two-node model** (Gagge, Stolwijk and Nishi 1971; the 1986 form ASHRAE 55 and
ASHRAE Fundamentals ch. 9 use, coefficients from pythermalcomfort's `two_nodes_gagge`): metabolic heat from
what the player is doing (ASHRAE met: standing 1.2, walking 2.0, sprinting 3.8, driving 1.5, asleep 0.7),
clothing insulation in clo (the everyday outfit is 0.61; worn gear adds its `clo` from `data/equipment.csv`),
convection with the weather's wind (two thirds of the 10 m wind reaches a person), radiation, sweat
evaporation, breathing, and the body's answers: skin blood flow, sweating, and shivering (capped, fading at 30
to 32 C, tiring after hours: Eyolfson et al. 2001, Tikuisis et al. 2002). Rain soaks clothing, which loses
about 30 percent of its insulation and cools the body as it dries (Zhao et al. 2025; Havenith et al. 2013).
Inside the sealed home it is the home's own air, still. In everyday clothes 15 to 25 C is comfortable
indefinitely; standing in a 5 m/s wind at 0 C reaches 35 C after about 10 h; walking holds it off; heavy work
at 35 C reaches heatstroke in about 2 h. Calibrated against the Cold Exposure Survival Model (Tikuisis 1995)
and measured human trials (Helland et al. 2025; Thompson and Hayward 1996).
- Conditions: `hypothermia` below 35 C, `heat_exhaustion` above 39, `heat_stroke` above 40, `shivering` while
  the body shivers. Lasting harm only below 32 C (moderate hypothermia) and above 40 C (heatstroke).
- **Two modes** (Settings > Gameplay > Body heat): **Forgiving** (the default: the same physics, the core swings
  half as far from normal and harm comes at half the rate, so it can never kill sooner) and **Realistic**.
- Inputs: `EnvironmentContext` (air, humidity, wind, precipitation, pressure, activity, and `sheltered`, set
  since 2026-09-27 when a built roof on three walls is over the player aboard: Construction, above), published
  by `engine::survival_env`. Outside, the air temperature, pressure and wind are the weather's at-player values:
  environment Layer 1 where the player stands plus the weather's deviation (2026-09-27), and what falls is rain
  or snow as the air there decides (`systems::precipitation`; snow wets clothing at a third of rain, a game choice).
- **The open sky and the sun** (v0.1416.0, sun v0.1417.0): in the open the body radiates to a sky well below the
  air (Swinbank's clear-sky temperature, clouds bringing it back to the air's; about 0.5 C on a clear 10 C night),
  and by day the sun adds its warmth (ASHRAE 55 SolarCal: about 47 C mean radiant with a clear sun overhead in 20 C
  air). A roof overhead keeps both off: warmer to stand under on a clear night, shade on a hot day
  (`body_heat::open_sky_radiant_c`, `sun_mrt_rise_c`, `Weather::cloud_share`). The HUD's weather line ends with
  what the air feels like when that is 2 C or more from its temperature ("feels 35C" in the noon sun in 20 C air,
  v0.1419.0): the model's operative temperature, `body_heat::operative_c`.
- **A campfire's warmth** (BUG-153, 2026-10-05): a burning built fire radiates (16 kW for the campfire) and a
  person near it feels warmer surroundings, falling as the inverse square: about 16 C instead of -11 C at 1.5 m
  on a clear 0 C night, nothing at 20 m, nothing from an out fire (`construction::fires::warmth_at`,
  `body_heat::radiant_with_source_c`, applied in `engine::survival_env`).
- **Sweat costs water** (v0.1422.0): the sweat the model evaporates comes out of hydration, about 50 mL a
  point (an hour walking in dry 35 C air, about 0.2 L; hard work in heat several times that), scaled by the
  Vitals drain slider (`BodyHeat::sweat_litres`, `HeatOutcome::sweat_l`, `food::HYDRATION_PER_LITRE`).
- **A drink puts back the water it carries** (2026-10-20): its litres (`drinks` in
  `data/food/item_profiles.ron`) times the share of it that is water (its profile's `water_g`, USDA's figure) at
  20 points a litre, so a 500 mL bottle of water puts back 10 points, a litre jug 20, a litre of milk about 18
  and a cup of coffee under 5 (`FoodSystem::drink_points`). Every drink gave 30 points before, whatever it was.
- **Food feeds and waters by what it weighs** (2026-10-20): every food says how many grams of it are eaten
  (`portions` in `data/food/item_profiles.ron`: its items.csv weight less the peel, core, shell, rind or bone
  USDA's SR28 counts as refuse, cited per row), and those grams put back their food energy at a day's 2,000 kcal
  (21 CFR 101.9(d)(9)) for a day's hunger (`SATIATION_PER_KCAL`) and their water at 20 points a litre (each
  profile's `water_g` in `data/food_system.ron`, USDA's figure). A 300 g loaf of bread 5.7 Satiation and 2.2
  Hydration, a cucumber 0.5 and 5.4 (`FoodSystem::portion_g`, `satiation_points`, `water_points`). It was 0.15
  points per kcal in 100 g whatever the item weighed (a loaf 40, nearly three days) and 10 Hydration for produce,
  3 for the rest. Coverage tests: `every_food_says_how_much_of_it_is_eaten`,
  `every_profile_says_how_much_water_it_carries`.
- Native: `crates/humanity-systems/src/body_heat.rs` (model, tests in `body_heat_tests.rs`), `crates/humanity-systems/src/food.rs` (the vitals
  pass), `src/engine/survival_env.rs` (inputs), `src/gui/pages/settings.rs` (mode)
- Data: `data/equipment.csv` (`clo`), `data/status_effects.csv` (the four conditions)
- Design: `docs/design/body-heat.md`

### Carrying weight (2026-10-04, BUG-136)
What the player carries has a limit and, in the Realistic mode, consequences. The limit is the inventory's own 50 kg
plus what worn gear adds (`carry_capacity` in `data/equipment.csv`: a large backpack adds 25 kg), comfortable at 1 g
(9.81 m/s^2, the homestead's gravity and Earth's), and it FOLLOWS GRAVITY: the legs hold up weight, mass times local
gravity, so the same legs carry 9.81 / g times the mass (about 2.6 times on Mars, 6 times on the Moon, no limit when
weightless). The gravity is the one the walk applies: the planet's at the player's altitude while a surface is engaged,
otherwise the homestead's `gravity_m_s2`. Operator decision 2026-10-04: "slower walking in realistic mode and no
jumping in nonzero G based on weight/mass. We can obviously carry heavier in low-g to zero-g but, mass still applies."
- **Two modes** (Settings > Gameplay > Carrying weight): **Forgiving** (the default: the same limit, a warning only,
  movement unchanged) and **Realistic**: over the limit, walking slows by how far over (10% over walks at 90%, half
  again over at half speed, never below a 15% crawl) and there is no jumping; and any load is extra mass to launch,
  so a jump leaves the ground at sqrt(70 / (70 + load)) of its speed (30 kg: 84%; a 150 kg load on the Moon, well
  under the limit there, still 56%). The slowdown is WALKING only, kept in its own controller field
  (`carry_speed_factor`) apart from the status-effect and gear multiplier: dev flight, the planet's flight band
  (10-100 km), swimming and ladders move at their usual speed whatever is carried. On a planet Space is a held climb
  at walking speed rather than a one-shot jump, so the jump scale gates climbing in mid-air too.
- Shown: the Inventory page's Weight tile (the real limit where you stand, and a line saying where it comes from and
  why you are slow or cannot jump) and a HUD line under the survival bars while overloaded.
- Not modelled, because the movement model has no momentum there: starting and stopping are instant in both walks
  (no horizontal inertia), and there is no zero-g pushing (the homestead always has its gravity; dev flight is a
  noclip camera). The jump is the one place carried mass acts. Ladders climb at the same rate whatever is carried.
- The mode is each player's own setting, also in the shared world: a server cannot yet require Realistic carrying
  (tracked in `docs/design/in-app-ops.md`).
- Native: `crates/humanity-systems/src/encumbrance.rs` (the rules, standalone tests), `src/engine/carry_load.rs` (applies them each
  frame), `src/renderer/camera.rs` (`carry_speed_factor` and `jump_scale`, the homestead walk and jump),
  `crates/humanity-systems/src/surface_move.rs` (`carry_walk_factor` and `carry_gated_radial`, the planet walk and jump), `crates/humanity-systems/src/inventory/mod.rs` (`carry_bonus_kg`, the gravity-aware `encumbered` flag),
  `src/gui/pages/inventory.rs` (tile), `src/gui/pages/hud.rs` (HUD line), `src/gui/pages/settings.rs` (mode).
  Web: none; movement and the in-game Status tiles exist only in the desktop app.

### Death and your pack (2026-10-04)
What dying costs, in two modes (the operator's decision on the first-hour audit: "Today the death screen says
'Nothing was lost.' I'd keep that as the Simplified mode. In Realistic mode, your carried items would stay where you
fell for a while, to go back for."). Either way you wake in the respawner with full health.
- **Two modes** (Settings > Gameplay > Death): **Simplified** (the default: nothing is lost, the death screen's old
  words) and **Realistic**: everything in the backpack stays behind as one pack where you fell; what you wear and what
  you have equipped stay on you, and so do credits, skills, quests and home storage. Aboard it lies on the floor under
  you; on a planet on the ground under you; from open space (outside every room) on the nearest room's floor; from deep
  water on the nearest dry ground (a ring search, then back to the shore); from the air on the ground below; on a body
  with no ground to walk to near you, on the ship's nearest floor. The death screen says what stayed, where, and why
  somewhere else when it is.
- A tracked HUD marker ("Your pack · 120 m", BUG-148's direction-placed marker, numbered when there are several) and
  "[E] Take back your pack (14 items)" in reach: E takes back as much as the backpack's volume holds, each stack with
  its wear, grade and food age, and leaves the rest in the pack (last in the E chain).
- It stays for 60 minutes of PLAY, counted only in the world and alive, never while the game is closed, with a notice
  5 minutes before and when it is gone. The save keeps every pack (`WorldSave::left_packs`), a pack in the home moves
  with the home to another plot, and food in a pack keeps aging.
- On a shared server the pack is yours alone (it lives in your own game and save): a lootable pack is an open
  decision, and the design note lists what it would need.
- Native: `crates/humanity-systems/src/death_pack.rs` (rules, leave, take back, clock, placement; relay-safe), `src/engine/death_pack.rs`
  (the frame: death surfaced, where you fell, prompt, E, draw, marker; tests in `death_pack_tests.rs`),
  `src/gui/pages/hud.rs` (`death_screen_lines`, the prompt), `src/gui/pages/settings.rs` (`death_hint`, the mode),
  `src/config.rs` (`death_realistic`), `src/save_load.rs` + `crates/humanity-systems/src/persistence.rs` (the save).
  Web: none; the 3D world exists only in the desktop app.
- Data: `data/world/death.ron` (the time, the warning, reach, facing cone, air height, wall clearance, room slack,
  open-space distance, shore search, the pack's size and colours)
- Design: `docs/design/death-and-your-pack.md`

### Skills/Progression
20 skills across 5 categories, XP curves, level-up notifications. **Registered, ticks live** (`SkillSystem` is NOT in `DEFERRED_SYSTEMS` -- this "NOT registered" note was stale, corrected 2026-07-01). Note: `crates/humanity-systems/src/skills/learning.rs`'s `Skill`/`add_practice` is a SEPARATE, unused struct with its own unresolved TODO (learning-curve level thresholds) -- it has zero callers anywhere in the tree and is not what the live, registered `SkillSystem` actually uses; treat it as dead/superseded code, not a gap in the live skill system.
- Native: `crates/humanity-systems/src/skills/mod.rs`
- Data: `data/skills/skills.csv`

### AI Behaviors
5 behavior types (passive, aggressive, herd, predator, guard) with state machines. **⚠️ Native `AISystem` NOT registered, never ticks (the relay drives ambient NPCs separately, server-side). See the lint.**
- Native: `crates/humanity-systems/src/ai/mod.rs`

### Vehicles/Mechs
Enter/exit vehicles, torso twist, jump jets, heat management. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/vehicles/mod.rs`

### Ecology/Disease
Disease spread by proximity, seasonal effects, population tracking. **⚠️ NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/ecology.rs`

### Quests
Data-driven quest progression from RON files. 10 objective kinds: Gather, Craft, Make, Harvest, Build, Travel, Talk, and since 2026-10-04 Eat, Plant and View. **Registered, ticks live in native single-player too** (`QuestSystem` is NOT in `DEFERRED_SYSTEMS`; this "NOT registered, native single-player doesn't work" note was stale, corrected 2026-07-01. The relay separately runs the authoritative quest chain for multiplayer -- that part remains accurate.) Each step done posts a notice naming the next step, and a finished quest one naming what it gave and the quest that starts (`step_notice`, `completion_notice`, `next_quest_notice`).
- Native: `crates/humanity-systems/src/quests/mod.rs`, `crates/humanity-systems/src/quests/objectives.rs`
- Data: `data/quests/*.ron`, `data/entities/destinations.ron`

**The opening: the first ten minutes (2026-10-04).** First Steps (`gs_first_steps`, accepted at spawn) is a new player's first ten minutes, in the order the operator accepted: check your vitals (press I, Inventory > Status), eat (a Basic Ration from the starting kit), plant (Inventory > Garden > the Variety Greens and Beans tower > Plant this tower), craft a tool (Esc > Crafting > Carve Fishing Rod, a plank and a wire from the Barn at the workbench), get 3 iron ore (Inventory > Mining, or the trading post), smelt iron (Crafting > Smelt Iron, coal from the trading post, or graphite from asteroid C-3), build a Storage Chest (Crafting > Structures > Build, then E), and step out of your own front door. Then Toolsmith and Build First Habitat follow as before. Each step's line is the HUD's quest line (64 characters with its counter) and names the key, page and section as the game shows them. A player who finished the old two-step First Steps keeps it finished (same id).
- **Eat** counts what the food system really eats from the backpack (`eat_<item>`, `FoodSystem` on the consume channel; a drink is not eating). **Plant** counts every crop the player sows, from a backpack seed, a tower design or a bed (`plant_<crop>`, `FarmingSystem`; the showcase garden is not the player). Both count any item when the step names none. **View** is a view coming on screen: the Inventory page's Status section marks "vitals" (`GuiState::on_screen`) and `src/engine/quest_hooks.rs` reports it once per opening. A Plant button that plants nothing now says why (no seed in the backpack for what it grows, or every cup or plot already taken).
- **Travel** is done while the player stands in the place AND it is the current step (`destinations_here`): walking past a place earlier does not count. A destination can be `at: Some(OwnFrontDoor(out_m: 1.5))` instead of a fixed `pos`: the doorstep of the player's OWN front door, on whichever plot they were given (the Commons for a family plot, First Street for a street plot), published every frame from the assembled ship (`front_door_of`, `publish_front_door`, `FRONT_DOOR_KEY`). A step line can say what the door opens on with `{front_door_opens_on}` (`step_text`). A guest (no plot, the home put away) has no front door aboard, and cannot build or craft at the home's stations either, so a guest's opening waits until they hold a plot or play solo.
- Tests: `src/engine/crossing_tests/quests_opening.rs` (the engine's since the crate split moved tests that reach up) walks the whole opening on the shipped data with the real systems, plus one test per new kind, the HUD's 64 characters for every step of every shipped quest on every plot, the names each step quotes, and a returning player; `every_shipped_home_has_what_the_starter_quests_need` (`crates/humanity-ship/src/machines.rs`) checks both shipped homes have the food, the seeds, the tower, the stations and every recipe input the chain uses.

**Each start has its own opening (ship homes increment 6, 2026-10-12).** A new character is given its start's `opening` (data/homes/starts.ron): the Homestead's First Steps above, or the Cabin's **Settling In** (`gs_first_steps_cabin`): check your vitals, walk to the mess hall, take a meal at the serving counter, visit the recreation room, head home to your cabin door. The mess hall and the recreation room are destinations of a new kind, `at: Some(NearestVolume(zone_type: ...))`: the ship's labelled volume of that zone type nearest the player's own front door (nearest where they stand, for a guest), the place being its whole floor plus the destination's radius (the ship's volumes are published every frame beside the front door, `publish_front_door`, `SHIP_PLACES_KEY`). The Quests page never offers another start's opening (`ship::starts::opening_quests`).
- Data: `data/quests/getting_started.ron` (`gs_first_steps_cabin`), `data/entities/destinations.ron` (`mess_hall`, `recreation`), `data/homes/starts.ron` (`opening`)
- Tests: `src/engine/crossing_tests/quests_opening.rs` (the engine's since the crate split moved tests that reach up) (`the_cabin_opening_walks_on_this_data`, `the_mess_hall_is_the_nearest_one`), `crates/humanity-systems/src/quests/mod.rs` (`a_shared_place_aboard_is_its_whole_floor`)

### Combat
Damage calculation, status effects. **⚠️ `CombatSystem` NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/combat/`

### Economy
Fleet resource management. **⚠️ `EconomySystem` NOT registered, never ticks (see the lint).**
- Native: `crates/humanity-systems/src/economy/`

### Economy Automation Phase 1: the living production chain (v0.663)
The operator's living-ecosystem loop, running with ZERO player interaction after one
click: commission a mining drone (with "Keep mining" checked) and raw asteroid ore
becomes finished tools on its own. Three pieces:
- **AutoRefine machines** (`data/machines/home.ron` `auto_recipe` -> `AutoRefine` ECS
  component): a machine continuously runs one recipe against the HOME inventory
  whenever the inputs are in stock -- the smelter auto-runs `smelt_iron` (drone ore +
  coal -> ingots), the NEW workbench machine auto-runs `craft_hammer` (ingot + plank
  -> hammer). Inputs consumed from home stock, timed batch per machine (machines run
  concurrently), outputs land back in home stock. Deliberately no skill gate: owning
  the machine is the unlock. Data-driven -- any machine gains auto production by
  adding `auto_recipe` in the RON, no code.
  **Since BUG-150 (2026-10-04) "home stock" is home storage only:** a machine never
  takes from the player's backpack (the sawmill used to saw the logs the player
  carried), its status line says "waiting for X in home storage", and the drone
  unloads its haul into home storage, so the chain still runs untouched. A quest's
  Gather step counts what the home holds as well as what is carried.
- **Drone standing orders**: the Mining panel's "Keep mining" checkbox turns a
  commission into a standing order (`auto_mine_order`); the drone re-launches the
  same trip after every delivery until the asteroid depletes (which removes the
  target and ends the loop naturally) or the box is unchecked.
- **Game-time economy clocks** (`systems::time::scaled_dt`): craft timers, drone
  phase timers, and manufacturing progress all scale with `time_scale`, so
  "accelerated for testing" speeds the whole economy, not just the wall clock
  (previously they all ran on raw frame dt and ignored the time scale).
Locked by 5 new tests, incl. `full_chain_drone_ore_becomes_a_hammer_untouched`: one
drone commission -> ore -> auto-smelted ingot -> auto-crafted hammer, untouched.
- Native: `crates/humanity-systems/src/crafting/mod.rs` (AutoRefine arm + completion redirect), `crates/humanity-systems/src/mining.rs` (standing order), `crates/humanity-systems/src/time.rs` (`scaled_dt`), `crates/humanity-systems/src/manufacturing.rs`, `crates/humanity-core/src/ecs/components.rs` (`AutoRefine`), `crates/humanity-ship/src/machines.rs` (`MachineDef::auto_recipe`), `src/lib.rs` (spawn + bridge), `src/gui/pages/inventory.rs` ("Keep mining")
- Data: `data/machines/home.ron` (smelter `auto_recipe` + the new workbench machine)

### Economy Automation Phase 2, Stage 1: Vehicle Kits (v0.677)
The operator's staged vehicle pipeline (decision 2026-07-02: BOTH item and world-spawn
models, staged). Stage 1 makes big end-products CRAFTABLE as an oversized flat-pack
"kit" ITEM that reuses the whole existing crafting/storage chain, then DEPLOYS into a
real, persistent Vehicle entity standing in the world:
- **Kit items + recipes are pure data**: `truck_pickup_kit_0` / `rover_kit_0` rows in
  `data/items.csv` (category `vehicle`, subcategory `kit`), workbench recipes in
  `data/recipes.csv` (steel + iron + rubber), and the kit->vehicle mapping with body
  proportions in `data/vehicles/kits.ron` (`VehicleKitRegistry`). Adding a deployable
  vehicle is data rows, no code (infinite-of-X).
- **Deploy action**: the inventory item card shows Deploy for vehicle kits ->
  `pending_deploy_kit` -> `deploy_kit_request` channel -> `VehicleSystem::handle_deploy`
  (first registration of VehicleSystem; its enter/exit/mech arms stay dormant until
  Stage 3). Registry lookup happens BEFORE consume so an unknown kit never costs the
  item; survival consumes the kit, creative deploys free (same semantics as crafting).
- **The vehicle is real world content**: spawned 6 m in front of the camera at floor
  level facing the player's look direction, rendered as body/cabin/4-wheel primitives
  scaled from the registry (build-once meshes, drone-dock pattern), and persisted in
  `WorldSave.deployed_vehicles` so a parked truck survives an app restart.
Locked by 8 tests incl. one-kit-cannot-become-two-vehicles and the save round-trip.
Next: Stage 3 (physical transport the player can follow or take over).

### Economy Automation Phase 2, Stage 2: Factory World-Spawn (v0.679)
Factories now END the production chain in a vehicle standing on the lot, not an item
on a shelf. The new `vehicle_assembler` machine (data/machines/home.ron, placeable
from the build palette) auto-runs `assemble_rover`: steel/iron ingots + rubber in the
home stock become a REAL rover that rolls out onto the pad 3 m in front of the
machine -- so drone -> smelter -> assembler is now mine-ore-to-vehicle, untouched.
- **Vehicle-class outputs**: any recipe output the kit registry resolves as an
  assembled vehicle world-spawns at the factory pad instead of the inventory
  (`CraftingSystem::deliver_outputs`, shared by timed + instant completion). A full
  backpack cannot stall the line (`outputs_fit` skips vehicle outputs); a machine
  despawned mid-batch still delivers at the pad captured at start; a MANUAL
  assemble craft rolls the vehicle out in front of the player.
- **Machines have a world pose now**: every home-machine ECS entity carries a
  Transform (resolved position from load_world; raw offset in menu mode) -- the
  factory pad, and the anchor for future per-machine spatial behavior.
- Factory-spawned vehicles are the same persistent Vehicle entities Stage 1 deploys
  (rendered by the same pass, saved in `WorldSave.deployed_vehicles`).
Locked by 5 tests incl. the full-backpack line, mid-batch despawn, and a data lint.
- Native: `crates/humanity-systems/src/crafting/mod.rs` (`deliver_outputs`, `ActiveCraft::pad`), `src/lib.rs` (machine Transform)
- Data: `data/machines/home.ron` (`vehicle_assembler`), `data/recipes.csv` (`assemble_rover`/`assemble_truck`)

### Economy Automation Phase 2, Stage 3 slice 1: Vehicles Move (v0.680-v0.682)
The first moving vehicles. The Inventory page's **Vehicles section** lists every
vehicle standing in the world (name, live distance, status); **Summon** a distant
one and it drives itself to you -- straight-line travel on game time, yawing to
face its motion, parking within 4 m ("En route to you..." while moving). Stage 2's
pad lanes automatically reuse the slot a summoned vehicle vacates.
- `VehicleRoute { dest, speed_mps, arrive_radius }` ticked by VehicleSystem;
  removed on arrival. Transit is deliberately NOT persisted (a mid-transit save
  restores the vehicle parked in place, consistent with drone flights).
- Per-vehicle speeds in `data/vehicles/kits.ron` (truck 8 m/s, rover 6 m/s).
- **Live production status (v0.681)**: the same section shows one honest line per
  auto machine each tick -- "Assemble Rover — 42%", "waiting for Steel Ingot x6",
  "pad full, line paused" -- covering assembler, smelter, and workbench (from the
  operator's field test: the assembler sat idle with no rubber and nothing said so).
- **Docking sequence (v0.682)**: the hangar drone lifts off (~2 s to +4 m) before
  vanishing and settles back down on return, instead of popping.
- Web parity: none by design -- these are views into the live 3D world, which the
  web client does not render.
Remaining Stage 3: follow-cam, take-over driving (VehicleSystem's dormant
enter/exit arms), the buy-side order flow (gated on the wallet/currency decision).
- Native: `crates/humanity-systems/src/vehicles/mod.rs` (summon + `tick_routes`), `crates/humanity-systems/src/crafting/mod.rs` (`auto_craft_status`), `src/gui/pages/inventory.rs` (Vehicles section), `src/net/sync.rs` (crew grounding, v0.681), `src/lib.rs` (sync + bridges + `drone_dock_anim`)
- Native: `crates/humanity-systems/src/vehicles/mod.rs` (registry + deploy arm), `crates/humanity-core/src/ecs/components.rs` (`Vehicle`), `src/lib.rs` (registry load + channel + bridge + render pass), `src/gui/pages/inventory.rs` (Deploy button), `crates/humanity-systems/src/persistence.rs` + `src/save_load.rs` (`deployed_vehicles`)
- Data: `data/vehicles/kits.ron`, `data/items.csv` (kit rows), `data/recipes.csv` (kit recipes)

> The old "Navigation" and "Logistics" support-module entries were removed 2026-07-02:
> `crates/humanity-systems/src/navigation/` and `crates/humanity-systems/src/logistics/` were deleted in the v0.674.0
> dead-code sweep (zero callers). `crates/humanity-systems/src/transportation.rs` (CargoVehicle routes)
> still exists unregistered and is the Stage 3 raw material.

---

## Construction and Build Editor (v0.455 - v0.606)

The in-app homestead builder. An overlay editor (gated by the `construction_active` flag in
`src/gui/mod.rs`, NOT a `GuiPage` variant) over the 3D viewport: a left object browser + 3D astral
camera + a right details pane + a bottom placement palette. The panel UI lives in `construction.rs`;
the input/gizmo/grab/duplicate/snapshot logic lives in `src/lib.rs`.

### Build Editor Shell
Three-zone editor: resizable left `SidePanel` (search box at top + collapsible sections), center orbit
viewport, right detail pane that routes to the selected object's editor, and a bottom placement palette.
Save/Close pinned to the bottom of the left panel so "Save home" is never off-screen.
- Native: `src/gui/pages/construction.rs` (`draw`, `draw_wall_editor`)
- Flag: `src/gui/mod.rs` (`GuiState.construction_active`)

### Unified Object Browser (v0.596 - v0.598)
One single-line row per object across every type (walls, structures, machines, lights, roads, conduit
nodes), grouped into collapsible per-type sub-headers with counts. A filter box and per-type collapse.
Double-click a row to fly the camera to that object.
- Native: `src/gui/pages/construction.rs` (`draw_object_browser`); filter via `construction_object_filter`; focus via `construction_focus_request` (consumed in `src/lib.rs`)

### Move / Select / Duplicate Gizmos (v0.549 - v0.600)
Tap-to-select vs hold-to-move on every object; drag corner-nodes, machines, openings, lights,
road/conduit nodes, and the player-spawn avatar. Double-click-to-focus. Duplicate the selected object
with Ctrl+D. Grid-snap toggle (0.25 m). Constant-width "line circle" gizmo bounds visible through walls;
the active grabbed gizmo RGB-cycles.
- Native: `src/lib.rs` (`construction_duplicate`, the `construction_*_grab` states), `src/gui/pages/construction.rs` (browser hints, grid-snap toggle)

### Lock Per Object Type (v0.614)
Each object-browser type-group has a "Lock type" toggle: a locked type (walls, structures, machines,
lights, road/pipe nodes) can't be selected or grabbed in the viewport, and shows no `[x]` in the browser
-- so on a busy build you lock your walls while arranging machines and never fat-finger them. The group
title shows `[locked]` in the warning colour. (Viewport HIDE-per-type is a deferred follow-up.)
- Native: `src/gui/mod.rs` (`construction_locked_types`), `src/gui/pages/construction.rs` (the per-group toggle), `src/lib.rs` (the pick dispatch gates each `try_pick_*` on the type not being locked)

### Alignment Snap Guides (v0.613)
While dragging any object, its X and/or Z snaps to the nearest other object within 0.3 m (independent per
axis, applied after grid-snap), and a faint amber guide line spans the box along the snapped axis so you
see what you are lining up with. Walls contribute both corners; the dragged object is excluded.
- Native: `src/lib.rs` (`snap_to_alignment` pure helper + `gather_other_positions`, wired into `apply_object_drag`; the guide line drawn into the construction overlay's `ring_lines`)

### Pipe materials and marker bands (2026-10-04)
Pipes, hoses and cables draw their real MATERIAL (copper tube, rubber hose, a power cord's or a data
cable's jacket, from `data/piping/pipe_materials.ron`), and coloured MARKER BANDS say what flows inside,
from the ship's marking scheme, ISO 14726 (`data/piping/marking_schemes.ron`, every row citing
`docs/reference/findings/2026-10-04-pipe-marking-standards.md`). Markers sit beside each machine, just past
each bend and never more than 6.1 m apart (GSFC's 20 ft) along a run, generated from what the line
carries, never typed (honest by construction): its connection kind, made specific only by the machine it
leaves (a machine type's `outlet_media`, `MachineHome::line_content`), so water leaving the purifier is
marked drinking water, an air handler's line condensate, the toilet's line waste, and the cistern's untreated
water fresh water's group colour alone. Grain conveyed to the silo is deliberately unmarked (ISO 14726 does not
cover cargo). Two modes, Settings > Gameplay > Pipe markings: Simplified (one band of the main colour) and Full
(the scheme's whole marker: potable water blue-green-blue, condensate blue-yellow-blue, electrical lines
MIL-STD-1247D's brown and orange stripes). Copper is drawn with measured copper's reflectance. The registry also carries ISO 20560-1, ASME A13.1, BS 1710, DIN
2403 and MIL-STD-1247D for the Real side. All bands of one colour are one merged mesh (one draw per colour).
The build editor's utility legend reads the same scheme. Dev pin: showcase `{"pipe_marking":"full"}`.
- Native: `crates/humanity-ship/src/pipe_marking.rs` (registry, placement, bands), `crates/humanity-ship/src/pipe_materials.rs`,
  `src/engine/pipe_markers.rs` (merged band meshes, mode switch), `src/engine/home_meshes.rs`
  (`rebuild_connection_objects`), `crates/humanity-ship/src/machines.rs` (`connection_color` = the scheme's main colour, `line_content`,
  `gizmo_colours`: the 3D port gizmos in linear light, dark band colours outlined), `src/gui/pages/construction.rs`
  (`legend_look`: the editor legend is an outlined swatch of the pipes' marker beside theme text)

### Run catalogs: rules and materials as data (spline routing rung 0, 2026-11-02)
What pipes, hoses, tubes and cables ("runs") are made of, and the "to code" rules they are laid by, as
data rows with a source each: the foundation the spline routing rungs build on
(`docs/design/spline-routing.md`). ONE material catalog: `data/piping/pipe_materials.ron` grew from the
pipe looks into every material's behaviour class (Rigid, Bendable, Flexible), form, what it may carry,
tags, and its SIZES (outside and inside diameter, DN, minimum bend radius by method, stick length,
mass per metre, roughness, temperature rating, and for cable the AWG, ampacity, voltage rating, ohm per
metre, bandwidth, range, RF emission, cost per metre). The old cable registry
(`data/utilities/conduits.ron`) merged in as size rows of their materials, every id kept, so a
connection's pinned `spec` resolves to look, behaviour and specs; WiFi is a radio link row (a data
medium with no run). Copper Type L (drawn and annealed, CDA Table 4.1's bend radii), PEX, CPVC,
coiled CPVC, PVC, steel, CSST, hydraulic and rubber hose, portable cord, NM-B and THHN (Southwire's
radii), Cat6, OM4 fibre, EMT, and the fictional superconductor. `data/piping/run_rules.ron`: 101 rules,
every kind of the design's 7.1 (bend radius, fittings only, support spacing and nearness, separation,
keep-out, vertical order, no shared holes, supports or bundles, slope, drain turns, penetrations, pull
limits, fill, allowed materials) grouped into three profiles, `dwelling_us` (24 CFR 3280, UFGS, makers),
`ship_us` (46 CFR, IEEE 45.8) and `spacecraft` (NASA-STD-8739.4A, FAA AC 43.13-1B), each rule with its
standing there (Law, Standard, Specification, Guidance, Maker, Ours) so the editor calls only law
"code". `fittings.ron` (elbows, R38's drain bends, tees, couplings, glands, transits, plates, PEX bend
supports), `anchors.ron` (straps, cushioned clamps, rod hangers by pipe size, J-hooks, trays, ties,
protector ramps), `benders.ron` (Klein's EMT benders, copper benders at the CDA's radii). Every row
names a finding of the 2026-11-01 findings (R1..R44, D1..D19) or `Ours` with a reason; the tests read
the findings documents' numbered paragraphs, so a row with no source, or naming a finding that does
not exist, fails `just validate-data`. All of it loads disk-first and relay-safe; nothing evaluates the
rules yet (rung 8), and the game routes, draws and simulates as before. The drinking-water copper-only
rule is the ship profile's `Ours` row (the operator's v0.535 rule; an open question in the design's
section 19).
- Core: `crates/humanity-core/src/run_materials.rs` (the catalog's types and loader, here because
  `utilities` reads its cable specs), `crates/humanity-core/src/row_source.rs` (`Source`, the findings
  check), `crates/humanity-core/src/utilities.rs` (`conduit_types()` is a flat view of the catalog's
  checkable rows)
- Ship: `crates/humanity-ship/src/run_rules.rs` (`RunRules`, `RuleKind`, `Applies`, `Standing`), `crates/humanity-ship/src/run_fittings.rs`,
  `crates/humanity-ship/src/run_anchors.rs`, `crates/humanity-ship/src/run_benders.rs`, `crates/humanity-ship/src/pipe_materials.rs` (today's four drawn
  kinds over the catalog)
- Data: `data/piping/pipe_materials.ron`, `run_rules.ron`, `fittings.ron`, `anchors.ron`, `benders.ron`
- Tests: `shipped_run_rules_find_the_nm_cable_spacing_in_the_dwelling_profile` (24 CFR 3280.808's 1.37 m),
  `shipped_catalog_carries_the_cda_radius_for_half_inch_annealed_copper` (57 mm), the `*_cite_real_findings`
  and `shipped_*_registry_parses_and_is_sound` tests; `crates/humanity-ship/src/machines.rs`
  `shipped_connection_specs_resolve_through_the_material_catalog` (every pinned spec resolves)
- Check: `scripts/check-run-sources.js` (the same source rules in `just lints` and CI, which runs no lib
  tests; its test `scripts/tests/run-sources.test.js`, in `just rig-tests`)
- Design: `docs/design/spline-routing.md` (rung 0); findings `docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md`

### Conduit Flow Visualization (v0.622, refined v0.623)
Makes connections legible in a dark room. Every pipe was drawn as a STATIC line in its utility colour
(the `connection_color` legend), faintly emissive; since 2026-10-04 a pipe draws its material and its
marker bands instead (see "Pipe materials and marker bands" above), and the legend is the ship's marking
scheme. The SELECTED machine's
connections additionally get animated rainbow marker spheres travelling along their routed path in the
flow direction, so it's obvious which runs go to/from the thing you're inspecting (v0.623: selected-only,
where v0.622 animated every pipe -- the change both declutters the view and keeps the render loop cheap
no matter how many conduits the home has). Build-mode only, gated on the "Helper gizmos" toggle; the
markers are small (0.10 m) beads with moderate emissive so they read as spheres, not flat discs.
- Native: `src/lib.rs` (`connection_flow_paths` carries `(path, from_id, to_id)` from `rebuild_connection_objects`; the render loop animates only `from_id`/`to_id == construction_machine_selected` via the `flow_rgb_mats` rainbow), `crates/humanity-ship/src/machines.rs` (`connection_color` legend, read from the ship's marking scheme since 2026-10-04)

### Rail cars -- the rail line comes alive (M2b, v0.637)
A small car now animates along each rail edge in build mode, so the rail graph (v0.635) reads as a living
transit line rather than static topology. Render-only (no state, no new field): mirrors the conduit flow-
marker pattern -- a box `RenderObject` slides from rail-node A to B by a `start_time`-driven phase, looping,
oriented along the track, one per edge (staggered). Confirms the rail graph at a glance; full passenger/
cargo routing + stops is later M2b.
- Native: `src/lib.rs` (`rail_car_mesh`/`rail_car_mat` + the per-edge animated car in the render loop)

### Viewport hide-per-type (declutter) (v0.636)
A per-type HIDE toggle in the object browser, beside the existing Lock toggle, for each type group
(Walls / Structures / Machines / Lights / Road nodes / Pipe nodes). Hiding a type skips its meshes +
gizmos in the 3D view and makes it un-pickable -- so a busy build can be decluttered to focus on one
system at a time. Machines hides the machine bodies + bounds cubes + port nodes; Pipe hides the routed
pipes + conduit-node rings; Road hides the road-graph gizmo; all six disable picking (like Lock). The
group header shows a `[hidden]` badge. Not serialized (a pure view toggle), mirroring lock-per-type.
- Native: `src/gui/mod.rs` (`construction_hidden_types`), `src/gui/pages/construction.rs` (Hide/Show toggle + `Act::ToggleHidden`), `src/lib.rs` (render + pick gating on the hidden set)

### Mothership rail graph (M2 transit) (v0.635)
The first transit network of superstructure M2 (`docs/design/mothership-superstructure.md`): a RAIL line as
a NODE + EDGE graph, generalising the v0.592 paired-train-platform link into a multi-stop line. Mirrors the
road graph exactly. `HomeStructure` gained `rail_nodes` + `rail_edges` (serde-default; `RailNode` = id +
(x,z) like `RoadNode`, `RailEdge` = from/to) with `add_rail_node` / `remove_rail_node` (prunes touching
edges) / `add_rail_edge` (refuses self-loop, unknown endpoint, or either-direction duplicate). A "Rail"
editor panel drops stops + wires edges (from/to pickers); a gizmo renders each stop as a pale-gold ring
and each edge as a straight track line. Cars + multi-stop routing are M2b.
- Native: `crates/humanity-ship/src/home_structure.rs` (`RailNode`/`RailEdge` + the rail-graph methods + tests), `src/gui/pages/construction.rs` (`draw_rail_editor`), `src/lib.rs` (rail-graph gizmo render), `src/gui/mod.rs` (`rail_edge_from`/`rail_edge_to`)

### Zone interactivity -- click / drag / duplicate (v0.634)
Zones (the macro districts from M1) became first-class gizmo objects, like machines/nodes. Click a zone
box in the 3D view to SELECT it (ray-vs-AABB pick, runs last so it never steals a click from anything in
front); the selected box highlights bright white and its detail shows on the right -- type + purpose,
editable origin/size, and Duplicate / Remove / Deselect. Drag a selected zone on the floor (its centre
follows the cursor). `duplicate_zone` clones a zone with a fresh id nudged +2 m. No new persisted field
(`construction_zone_selected` is a pure selection); zones render live so edits show immediately.
- Native: `src/lib.rs` (`try_pick_zone` + `ObjectGrab::Zone` drag + selected-zone highlight), `crates/humanity-ship/src/home_structure.rs` (`duplicate_zone`), `src/gui/pages/construction.rs` (`draw_zone_detail` + dispatch), `src/gui/mod.rs` (`construction_zone_selected`)

### Machine rotation (yaw) (v0.633)
A placed machine can now be ROTATED about its vertical axis -- so a box-shaped machine (a teleporter, a
server, a battery bank) can face a chosen direction instead of always being axis-aligned. `MachineInstance`
gained a `rotation` yaw field (degrees, serde-default 0, so every existing home + array cell is unchanged);
it flows through `PlacedMachine` into the renderer (`Quat::from_rotation_y`). The machine detail panel has
a Rotation control (a degree DragValue + a "+90" button); direct instances only (an array member has no
individual pose). Persists with Save.
- Native: `crates/humanity-ship/src/machines.rs` (`MachineInstance.rotation` + `PlacedMachine.rotation` + placements carry it), `src/lib.rs` (yaw threaded through `machine_objects` + applied in the machine RenderObject), `src/gui/pages/construction.rs` (Rotation control in `draw_machine_detail`)

### Conduit node TIERS + service-entrance grid-tie (v0.632)
The conduit pipe-graph gained its trunk-hierarchy controls (`conduits-node-graph.md` Stage 2 foundation +
`grid-hierarchy.md`). A selected conduit node's detail panel now sets its TIER -- Main (0) / Sub (1) /
Subsub (2) -- and the node renders sized by tier (a main line is a big ring, subs smaller) so the trunk-
and-branch reads at a glance. And a node can be flagged a GRID TIE / service entrance (where the home or
zone meets the external mothership/fleet grid), rendered as a distinct amber double ring. `ConduitNode`
gained a `grid_tie` bool (serde-default); both tier + grid_tie persist with the home.
- Native: `crates/humanity-ship/src/machines.rs` (`ConduitNode.grid_tie`), `src/gui/pages/construction.rs` (tier selector + grid-tie checkbox in `draw_conduit_node_detail`), `src/lib.rs` (tier-sized + grid-tie node rendering)

### Mothership ZONES, M1 -- the macro layout primitive (v0.631)
The first piece of the mothership superstructure (`docs/design/mothership-superstructure.md`): a ZONE is a
labelled, bounded VOLUME -- the macro analogue of a room. A `zone_types.ron` registry (infinite-of-X)
defines the districts the operator named -- residential, civic mall / meeting zone, industrial / factory,
hangar bay, mech bay, cargo, storage, agriculture, power, medical, transit hub -- each with a colour,
purpose, and default size. The Construction editor's "Zones" panel adds them (type picker -> drops one
centred in the footprint), lists them, and edits their origin/size; each renders as a colour-coded
wireframe box in build mode so the layout reads at a glance. Zones live on `HomeStructure` (serde-default,
so every existing home parses unchanged) and persist with Save. Data model only this stage -- transit
graphs (M2), the civic mall (M3), industrial+cargo (M4), hangar/mech (M5) build on it.
- Native: `crates/humanity-ship/src/structure.rs` (`ZoneType` + `zone_types()`), `crates/humanity-ship/src/home_structure.rs` (`Zone` + `zones` + `add_zone`/`remove_zone`), `data/blueprints/zone_types.ron`, `src/gui/pages/construction.rs` (`draw_zones_editor`), `src/lib.rs` (zone wireframe render)

### Room identity + the Console room (zone-named rooms, homestead increment)
Rooms detected from the walls (`HomeStructure::detect_rooms`) stop being anonymous: a room-grade
zone whose footprint contains a detected room's centre lends it its id, label and, through the new
`Zone::room_type` (a `data/rooms.ron` key), its FUNCTION: purpose, walk-up actions
(`data/rooms/room_actions.ron`) and access class, which the HUD's "you are in ..." surface reads
through `RoomTypeRegistry::function_for`. Rooms no zone covers keep `room_N`. Smallest containing
zone wins; a zone names at most one room; an unknown `room_type` warns and yields no actions. The
construction editor's zone detail panel has a room-type picker and shows what the pick resolves to
(GUI-first). The CONSOLE ROOM (colloquially the battlestation; the mothership bridge is the "command
deck") is the household's fixed workstation: zone type `console_room`, rooms.ron entry `console_room`,
and a walled 3.5 x 3 m annex north of the common room (x 47.5..51.0, z 44.0..47.0) in
`ship_structure.ron`, zone `console-room`. It is the mounting point for in-world screens
(`wall_screen`, `desk_monitor`, built since: see "In-world screens" and "Video on in-world
screens"; `wall_screen_5` on its north wall plays the demo clip). Also added
room-grade zone types `room_garden` and `room_living`, and `room_type` on every house zone with a
rooms.ron entry.
- Native: `crates/humanity-ship/src/home_structure.rs` (`Zone::room_type`, `name_rooms_from_zones`, `room_actions_for`), `crates/humanity-ship/src/fibonacci.rs` (`RoomInfo::room_type`/`label`/`type_key`), `crates/humanity-ship/src/room_types.rs` (`lookup_type`, `function_for`, `RoomFunction`), `src/engine/home_meshes.rs` + `src/engine/world_load.rs` (RoomBounds join), `src/gui/pages/construction.rs` (`draw_zone_detail` room-type picker), `src/gui/mod.rs` (`room_type_registry`)
- Data: `data/blueprints/zone_types.ron`, `data/rooms.ron` (`console_room`), `data/blueprints/ship_structure.ron` (zone `console-room` + annex walls, door, light)
- Docs: `docs/design/homestead.md` (Room identity, Console room), `docs/game/ship_zoning_transit.md`

### Grow-light power meter (v0.664, homestead gap #5)
The honest teaching artifact docs/design/self-sufficiency.md called for ("turns red the instant any LED
is added past the free pump headroom"). A `grow_light` machine (100 W LED panel, matching
`data/electrical.ron`) is placeable from the construction palette in both home designs; the moment one
is placed, the Buildability panel grows a dedicated "Grow-light power meter" row: GREEN while the lights'
draw (watts x a documented 14 h/day crop photoperiod, `GROW_LIGHT_DUTY_HOURS`) fits inside the home's
free headroom (generation minus all non-grow-light demand), AMBER when the home starts eating its battery
reserves every day, RED when the lights ALONE outdraw everything the home generates -- with the plain-
language note "this is why the garden uses the sun." Pure + world-free (`MachineHome::grow_light_report`),
so an AI can read it before committing a design. Neither seed design places a light (the reference gardens
are sun-lit; discovering the meter is the lesson). Same release also fixed `utility_meters` counting each
battery bank's bus terminal as 48 kWh/day of phantom demand.
- Native: `crates/humanity-ship/src/machines.rs` (`GrowLightReport`, `GrowLightVerdict`, `grow_light_report`, `GROW_LIGHT_DUTY_HOURS`), `src/gui/pages/construction.rs` (`draw_buildability` grow-light section), `data/machines/home.ron` + `home_solo.ron` (`grow_light` catalog entry)

### Usage meters + home self-sufficiency (grid S2, v0.630)
The Buildability panel now shows per-utility USAGE METERS: for power (kWh/day), water (L/day), and data
(Mbps) it reports the home's daily GENERATION vs DEMAND and a self-sufficiency fraction. Framed to TEACH,
non-punitively (grid-hierarchy.md): "makes 4.5, uses 2.4 kWh/day -- fully self-sufficient (+2.1 to share
with the community)" or "... 60% self-sufficient (X imported from the grid)". Pure + world-free
(`MachineHome::utility_meters`, computed from the placed machines' catalog roles), so it runs in the
editor and an AI can read it; no penalty for consuming, just understanding what you use + make.
- Native: `crates/humanity-ship/src/machines.rs` (`UtilityMeter` + `utility_meters` + `data_supply_mbps`), `src/gui/pages/construction.rs` (`draw_buildability` usage section)

**The day record (2026-10-22, the operator's "Do rooms not automatically calculate their volumes on
the fly?").** The meters read the simulation: the machines' power tick (`ElectricalSystem`) records
what every placed machine drew, an hour at a time on the game clock, as a rolling record of its last
24 game hours (`crates/humanity-systems/src/day_energy.rs`, `DayEnergy`), its request kept apart for the hours its
island had shed it. Once a machine has run a whole game day the meters charge that day (what it drew
plus what it asked for while shed: what the home needs); until then, and for a work station's idle
draw or a battery, the data's figure (`MachineDef::average_load_watts`, the estimate). So a fan in a
room made bigger or smaller, or a player's own room, is charged what its controller really did there.
Every daily figure reads the same record (`MachineHome::day_draw`): the Usage meter and the
Buildability energy balance, the grow-light meter, the Construction page's machine card ("Daily
energy"), the Home page's Energy, Water and Air loops (a live "running" line, `HomeLoop::meter`) and
the time away (`crafting::away::day_power_balance`). The pages say which figures are measured and which
are estimated. Saved with the home (`WorldSave::machine_day_energy`); the time away adds nothing to it.
- Native: `crates/humanity-systems/src/day_energy.rs`, `crates/humanity-systems/src/electrical.rs` (step 3b), `crates/humanity-ship/src/machines.rs` (`DayDraw`, `DaySource`, `MeterSource`, `day_draw`, `grid_day_draw`, `loop_meter_line`), `src/gui/pages/construction.rs`, `src/gui/pages/homes.rs`; tests `crates/humanity-systems/src/day_energy_tests.rs`, `farming::humidity_tests::a_room_resized_changes_its_fans_day_on_the_meter`, `farming::heat_tests::the_time_away_charges_each_machine_the_day_it_drew`

### Viewport conduit-node placement + drag-a-port-to-a-node (build Phase 2, v0.629)
The pipe GRAPH is now built in the view, not via panel dropdowns. The Conduit-nodes panel has a "Place in
view" toggle: while active, clicking the floor drops a junction node there (a "main line" point); right-
click cancels. And a dragged machine PORT can now land on a conduit NODE (not just another machine) -- it
branches the machine onto the main line as a graph edge (`add_conduit_edge`, machine -> node), routed as a
real pipe. The drag preview snaps to a hovered node and rings it. Together this is the operator's "drag the
node to the main power line." Nodes already had drag + remove gizmos (v0.599); this adds in-view creation +
port-to-node wiring.
- Native: `src/lib.rs` (`construction_place_conduit_node` mode in the press chain; `port_drop_node_target`; the release-handler node branch; the preview snap-to-node), `src/gui/mod.rs` (the place flag), `src/gui/pages/construction.rs` ("Place in view" toggle)

### Pipes terminate at port nodes (grid S1, v0.628)
A wire/pipe now ROUTES TO the matching-utility port NODE (the v0.627 sphere+arrow gizmo above the machine)
instead of a generic floor anchor -- a water pipe plugs into the water node, the power wire into the power
node, so the two also leave the machine at different points (less overlap). Resolved from `port_pick` by
matching `port.utility.id()` to the connection's kind, falling back to the floor anchor if the machine
declares no port of that utility. First concrete step of the grid-hierarchy staging (`docs/design/grid-
hierarchy.md` S1: "cables/pipes go to the input/output nodes").
- Native: `src/lib.rs` (`rebuild_connection_objects` route building resolves each machine endpoint to its port-node position)

### Port NODE gizmo: central sphere + in/out arrows (v0.627)
The v0.625/626 in/out rings weren't legible at a glance, so a machine port is now drawn as a real NODE: a
solid ~10cm sphere (coloured by utility) with 4 CARDINAL ARROWS radiating from it. Arrows pointing IN
(heads near the sphere) mark an INPUT/draw port; pointing OUT mark an OUTPUT/supply port; both heads =
bidirectional (a battery terminal). Far clearer than the rings, and the sphere is a real object (toward
the operator's "eventually plug into a 3D model with real ports"). Shown for the selected machine; still
the drag-to-connect grab target. Captured alongside it: the GRID HIERARCHY vision (`docs/design/grid-
hierarchy.md`) -- home breakers -> residential substations -> generators -> the mothership/fleet grid,
with non-punitive consumption METERING to teach supply/demand + community self-sufficiency (the
civilization-on-a-mothership goal).
- Native: `src/lib.rs` (`port_node_mesh` central sphere in the all-objects pass; the 4-arrow in/out gizmo in the line overlay)

### Clickable pipes/wires + bracket dedup + bolder port handles (v0.626)
Pipes/wires became first-class clickable objects (they had no viewport interaction before). Click a
routed connection in the 3D view and it's selected (`try_pick_connection` ray-samples each route's
polyline): the pipe traces bright white and the right panel shows "Wire / pipe" with its utility,
endpoints, and a **Remove** button (`remove_connection_between`, either direction). Also: the conduit
support FITTINGS (ceiling hangers + wall gaskets) are now DEDUPED by position across all connections --
many pipes share a service-height run, so their brackets used to stack invisibly at the same spot,
wasting polygons; one bracket now serves every pipe through that point. And the v0.625 port handles got
a bolder double/triple ring so the drag-to-connect targets are obvious.
- Native: `src/lib.rs` (`try_pick_connection` + press-chain wiring + the selected-pipe highlight + the fitting dedup `HashMap` in `rebuild_connection_objects` + bigger port rings), `crates/humanity-ship/src/machines.rs` (`remove_connection_between`), `src/gui/mod.rs` (`construction_connection_selected`), `src/gui/pages/construction.rs` (`draw_connection_detail` + dispatch + `clear_sel`)

### Viewport drag-to-connect (machine ports) + array-member move (v0.625)
The panel dropdowns ("pick from, pick to, Connect") were a confusing way to wire machines, so wiring is
now a VIEWPORT gesture. Select a machine and its declared ports (`derive_ports()`) show as coloured
handles floating above it (amber power, blue water, violet data, ... -- the pipe legend; an OUT port
gets an outer "target" ring). Click-drag a handle and a rubber-band line follows the cursor: it turns
the utility colour over a machine that has a compatible (same-utility) port and RED over one that
doesn't. Release to create the connection (`add_connection`, oriented supply->demand). The dropdown
panel stays as a fallback. Also fixed: dragging a machine that is part of an ARRAY (e.g. a grain tray)
now WORKS -- the first drag explodes that array into individual instances (`detach_array_member`, ids +
positions preserved so nothing jumps), since array cells had no editable offset before.
- Native: `src/lib.rs` (`port_pick` built in `rebuild_machine_objects`; `port_gizmo_pos`; `try_pick_port` + `port_drop_target` + `machine_has_port`; the release-handler wiring; port-gizmo + rubber-band render; `ObjectGrab::Machine` detach-on-drag), `crates/humanity-ship/src/machines.rs` (`detach_array_member`), `src/gui/pages/construction.rs` (the "drag a port handle" tip)

### Build-editor click fix + lock/list footgun guards (v0.623 + v0.624)
**v0.624 (the real root cause):** entering build mode never rebuilt the machine PICK VOLUMES
(`machine_pick`), so machines were not clickable in the viewport until some *other* edit (e.g. nudging a
light -> `construction_structure_dirty` -> `rebuild_homestead` -> `rebuild_machine_objects`) repopulated
them -- the operator's "I have to drag a light first, then it updates and machines are clickable" repro.
Fix: build-mode entry now sets `construction_structure_dirty` so the pick volumes rebuild immediately, in
the editor's coordinate space. **v0.623 (partial mitigations, still useful):** a locked object type
silently blocks viewport picking, and a >12-row type group (e.g. 24 grow towers) collapses and egui
remembers it collapsed -- so the browser shows a loud "LOCKED (can't click in 3D): ..." banner with a
one-click "Unlock all", and the group holding the current selection is forced open.
- Native: `src/lib.rs` (force `construction_structure_dirty` on build-mode entry), `src/gui/pages/construction.rs` (locked-types banner + `Act::UnlockAll`; `CollapsingHeader::open(Some(true))` for the selected group)

### Capped-cylinder mesh fix (v0.624)
`Mesh::cylinder_capped` wound BOTH end caps inward (their front faces pointed into the cylinder), so under
the renderer's CCW-front + back-cull convention the caps were culled and tanks/cisterns rendered open-
topped. Flipped both caps to face outward (verified against the side-wall winding, which is the reference).
Fixes the missing tops on the water cistern + every other cylinder-shaped machine (and the podium).
- Native: `src/renderer/mesh.rs` (`cylinder_capped` cap winding)

### Multi-Select + Group Delete / Nudge (v0.612)
Ctrl+click rows in the object browser to build a multi-select set (across every type -- walls, machines,
lights, structures, road/pipe nodes); selected rows show a `*` and the accent colour. A group-action bar
gives Delete (removes them all, index-keyed types in descending order, id-keyed types via the pruning
helpers so connections stay consistent), Clear, and Nudge (+/-X, +/-Z by 0.5 m, keeping each object's
height). A plain click resets the set to single-selection.
- Native: `src/gui/pages/construction.rs` (`group_delete`, `group_nudge`, the browser's Ctrl+click + group bar), `src/gui/mod.rs` (`construction_multi`)

### Construction Console (AI / dev act surface) (v0.578 - v0.580)
A text-command console, the discoverable act surface for both a human and an AI. Verbs: `help`, `list`,
`add_wall`, `rm_wall`, `set_material`, `add_door`, `add_window`, `set_style`, `add_lock`, `add_light`,
`rm_light`, `add_structure`, `rm_structure`, `add_layer`, `rm_layer`, `add_road_node`, `rm_road_node`,
`add_road`, `rm_road`, and the storey verbs `storey`, `add_slab`, `rm_slab`, `slab_glass`.
- Native: `src/gui/pages/construction.rs` (`exec_construction_command`, `CONSOLE_VERBS`)
- Dev IPC (2026-10-21): the showcase verb `{"console":"<line>"}` runs one line on the live home for
  that session only, never arming the autosave that writes the data files in the Dev mode
  (`src/engine/ipc.rs` `console_this_session`); its answer is the run.log's `Showcase: console` line.
  `just verify-falls` places and removes its deck with it.

### Live Home JSON Introspection (AI read surface) (v0.576)
Every rebuild writes a machine-readable snapshot of the live home so an AI can READ what the operator is
building, to `debug/home_snapshot.json`.
- Native: `crates/humanity-ship/src/home_structure.rs` (`HomeStructure::to_introspection_json`), written by `src/lib.rs` (`rebuild_homestead`)

### CAD Dimension Overlay and Wall Wireframe (v0.545, v0.594)
A live measurement overlay: wall lengths, corner angles, and the angle where a custom wall meets the box
hull; per-wall length labels; a wall-wireframe (layout outline) debug toggle. Master "Helper gizmos" +
dimension-overlay toggles in the "Options / Dev" section.
- Native: `src/gui/pages/construction.rs` ("Options / Dev" header, `construction_dimension_overlay`, `construction_show_helpers`); overlay lines drawn engine-side in `src/lib.rs`

### Footer Placement Palette and Building Info (v0.527, v0.602, v0.605)
Bottom palette with a "Structure" tab plus per-category machine tabs, a 10-column grid, held-item
highlight, expand/collapse. Holding a building shows its info card: category, size, power role, stat
readouts, and its connection points (ports) with direction arrows and per-utility colors.
- Native: `src/gui/pages/construction.rs` (`draw_palette`, `draw_building_info`, `draw_held_structure_info`, `port_line`/`port_color`)

---

## Home Structure (fixed box + interior walls) (v0.532 - v0.591)

The home-construction data model (replaced the old rooms-as-sliding-AABBs approach): a FIXED outer box
(the mothership allotment, default 55x89x3 m steel, glass roof) plus freely-placed INTERIOR WALLS; rooms
EMERGE from the walls via grid flood-fill rather than being placed as boxes.

### HomeStructure Model + the Ship Superstructure (v0.766 - v0.769)
The serialized home body: box dims + shell/roof material, interior walls, placed lights, placed
structures, a road graph (nodes + edges), and the player spawn point. Meshes regenerate on edit; rooms
detected by flood-fill. The ship-superstructure arc (docs/design/ship-superstructure.md, absorbs
decision-briefs Brief 1) generalized it into a whole vessel:
- **Zones (v0.766)**: the home body is ONE ZONE of `ShipStructure { zones }` - each zone carries
  id/label/purpose (residence | commons | bay | agriculture | corridor) + a world origin + the full
  home body unchanged. Editor Ship zone selector (Add zone, label/purpose/origin, confirmed delete);
  machines carry a `zone` id (default "home") and clamp into their zone's footprint; a lone legacy
  home_structure.ron adopts once as zone "home".
- **Corridors (v0.767)**: one data row (from_zone/door -> to_zone/door, width, glass_top) GENERATES the
  walled tube (floor, sides, glass-or-steel lid) and cuts walkable door apertures through both zones'
  perimeter shells in mesh AND collision. Straight/axis-aligned v1 with specific validation rejections;
  editor Corridors section; save prunes broken rows.
- **The Commons (v0.768, pure data)**: the seed ship's communal hall - a 34x55x8 glass-roofed commons
  zone with a 3x3 aeroponic tower grove (tank-watered, composter-fed like the home garden), apothecary
  towers + mushroom rack, three open-fronted Trading Post stalls, and a 10 m glass gallery from the
  home's east door. Zero engine code - the architecture's proof.
- **The Hull Wrap (v0.769)**: lofted plating swept through data-driven silhouette stations scaled to
  the cluster's real bounds (long-axis auto-detect), taper clamps that never slice a pressurized box,
  top cutouts over every glass roof + corridor lid, double-sided plating (look up through glass and see
  hull), greebles (engines/radiators/masts) as data rows. Regrows on any structure edit; H key /
  Settings "Show hull" toggle; purely visual (no exterior collision - bay doors/EVA are follow-ups).
  Dev pin: showcase `{"hull":"0"}` hides it as H does (released with "1" before every other vantage;
  the ship-first-street vantage uses it to see the homes along First Street, 2026-10-05).
- Native: `crates/humanity-ship/src/ship_structure.rs` (zones + corridors, load/save/adopt, merged meshes),
  `crates/humanity-ship/src/home_structure.rs` (the per-zone body incl. shell cuts), `crates/humanity-ship/src/hull.rs` (profile +
  loft + greebles), `crates/humanity-ship/src/wall_collision.rs` (`ship_wall_segments` + shell-cut gaps),
  `crates/humanity-ship/src/machines.rs` (per-zone clamping)
- Data: `data/blueprints/ship_structure.ron` (the seed ship: home + Commons + gallery),
  `data/blueprints/hull_profile.ron` (silhouette stations, margin, greebles - screenshot-tunable)

### Interior Walls + Wall Materials (v0.552, v0.585)
Walls are corner-node segment chains with per-wall material, per-wall thickness (down to a 1 mm screen),
and stackable surface LAYERS. The wall material picker shows real engineering values (density, tensile
strength, cost/kg, renewable) while you build.
- Native: `crates/humanity-ship/src/home_structure.rs` (`InteriorWall`, `SurfaceLayer`, `WallMaterial`, `wall_materials`)
- Data: `data/blueprints/wall_materials.ron` (8 materials: Steel, Concrete, Oak, Tempered glass, Aluminum, Pine, Granite, HDPE)

### Mitred Corners and Wall Joins (v0.549, v0.558, v0.566, v0.574)
Clean mitred corners where walls meet; round corner columns at 3+-wall joins; mid-span T-junction
clipping so a thick wall doesn't spear through another; corner-node snapping to a shared 5 cm grid.
- Native: `crates/humanity-ship/src/home_structure.rs` (`wall_end_miter`, `clip_end_to_walls`, `corner_column`, `quantize_corner`, `CORNER_GRID`)

### Doors and Windows (openings) (v0.533 - v0.578)
Doors and windows are openings placed on still-solid walls, each with a position/width/sill/height,
draggable opening gizmos + edge resize handles, and a data-driven animation STYLE: swing, slide, iris,
rotate, fold, energy, nanowall, fixed. Doors carry auto-open vs manual states + an interaction distance,
and an optional control panel.
- Native: `crates/humanity-ship/src/home_structure.rs` (`Opening`, `OpeningKind`; `style` is a data-driven String), `crates/humanity-systems/src/door_anim.rs` (style to `PanelMotion`), `crates/humanity-ship/src/door_panels.rs` (`panel_placements`, `PanelPlacement`)
- Editor: `src/gui/pages/construction.rs` (`OPENING_STYLES` const)

### Door Control Panels (v0.567)
Walk up to a manual door and press E at its control panel to open it; the panel mounts beside the door at
hand height.
- Native: `crates/humanity-ship/src/door_panels.rs` (`control_panel_pos`), `crates/humanity-systems/src/interaction.rs`

### Door Locks (v0.570)
Data-driven locks on a door; a door is passable only when every lock is Unlocked or Broken. Lock
interactions: KeyItem, Code (keypad), Knob, Crank (emergency no-power override), Biometric, Panel. Defeat
methods: Lockpick, HackPanel, ShootOut, BlowOpen, CutPower. Power-dependent flag per lock.
- Native: `crates/humanity-ship/src/lock_types.rs` (`LockType`, `LockInteraction`, `DefeatMethod`, `LockState`), `crates/humanity-ship/src/home_structure.rs` (`LockInstance`)
- Data: `data/blueprints/lock_types.ron` (metal_key, keypad, knob, crank, biometric)

### Per-Home Lights (v0.571 - v0.576)
Data-driven placeable lights; the renderer evaluates up to 8 point lights. Add lights from a picker,
click a light to edit it, drag light gizmos (RGB range sphere + diamond). Energy doors emit light
(emissive-as-light). Sun/global-illumination off toggle.
- Native: `crates/humanity-ship/src/home_structure.rs` (`PlacedLight`), `src/renderer/light.rs` (loads the registry), editor in `src/gui/pages/construction.rs` (`draw_lights_editor`, `draw_light_detail`)
- Data: `data/lighting/light_types.ron` (ceiling_panel, warm_lamp, cool_panel, spotlight, strip; kinds Point/Spot/Bar)

### Wall and Door Collision (v0.556)
Geometric first-person collision against walls (substepped so a sprinter can't tunnel a thin wall); door
apertures are walk-through gaps, window spans stay solid (glass blocks).
- Native: `crates/humanity-ship/src/wall_collision.rs` (`WallSegment`, `wall_segments`, `resolve`)

---

## Structural Pieces (v0.583 - v0.592)

A data-driven registry of buildable structural pieces, rendered by the construction "Structure" palette.
Add a buildable by adding one `.ron` line; no code.

### Structure Registry
Each piece has an id/label/category, a `kind` (drives behaviour) and a `shape` (placeholder geometry),
size, color, and step count. Kinds: Wall, Stairs (also Ramp via shape), Ladder, Elevator, Teleporter,
Train, Road, Deck. Shapes: Box, Steps, Ramp, Ladder, Frame, Slab.
- Native: `crates/humanity-ship/src/structure.rs` (`StructureType`, `StructureKind`, `MeshShape`, `structure_types`, `structure_mesh`, `walk_surface`)
- Data: `data/blueprints/structure_types.ron`

### Walkable Stairs / Ramps / Decks (v0.584, v0.588 - v0.589)
Walk UP stairs and ramps (a ground-height sampler lifts you step to step); a floor/deck piece for
multi-level builds; "place at height" so a deck sits at the top of a staircase.
- Native: `crates/humanity-ship/src/structure.rs` (`walk_surface`, `in_footprint`), placement in `src/lib.rs`

### Ladder Climb (v0.589)
Stand at a ladder and hold Space to climb (Shift to descend), step off onto a deck.
- Native: `src/lib.rs` (ladder-climb state), `crates/humanity-ship/src/structure.rs` (`StructureKind::Ladder`)

### Elevator Ride (v0.590)
A moving car that carries the player between levels; wait in the shaft to recall it.
- Native: `src/lib.rs` (elevator-car state), `crates/humanity-ship/src/structure.rs` (`StructureKind::Elevator`)

### Teleporters (v0.584)
Step through a teleport arch to jump to its paired pad (pair set in the detail panel).
- Native: `crates/humanity-ship/src/structure.rs` (`StructureKind::Teleporter`), pairing via `PlacedStructure.pair`

### Train / Rail Line (v0.592)
Pair two train platforms and a rail track connects them.
- Native: `crates/humanity-ship/src/structure.rs` (`StructureKind::Train`)

### Roads as a Node Graph (v0.585 - v0.591)
Roads are a node graph (nodes + edges); each edge is a road-class ribbon with a fixed top-to-bottom
material STACK (wearing course down to subgrade). Edge centerlines curve through the graph via
Catmull-Rom splines. Draggable road-node gizmos + per-node detail panels.
- Native: `crates/humanity-ship/src/home_structure.rs` (`RoadNode`, `RoadEdge`, `road_edge_centerline`), `crates/humanity-ship/src/structure.rs` (`RoadType`, `road_types`)
- Editor: `src/gui/pages/construction.rs` (`draw_roads_editor`, `draw_road_node_detail`)
- Data: `data/blueprints/road_types.ron` (footpath, residential, highway, runway)

---

## Home Power and Electrical Sim (v0.437 - v0.606)

The live electrical simulation for the home, plus the data-driven machine layout it runs on. Both
`ElectricalSystem` and `SolarSystem` ARE registered and tick the live home power sim (`src/lib.rs`).

### Live Electrical System
Each tick: sum active generators, sum enabled consumers, shed load by priority on a deficit, and
integrate the surplus/deficit into battery banks (charge/discharge with the day/night solar swing). As of
v0.607 the flow is PER ISLAND (a generator only feeds loads on its own wired circuit). Publishes a live
`PowerStatus` (generation, consumption, balance, battery Wh, autonomy hours) to the DataStore for the GUI.
Solar panels follow the sun where they stand: the home's longitude aboard (BUG-090), a planet build site's own
on the ground (v0.1413.0), and on the ground under the weather's clouds (v0.1418.0: Kasten and Czeplak's
`1 - 0.75 (N/8)^3.4`, a quarter under a full overcast, `solar::cloud_light_share`). Panels aboard are above the
weather.
- Native: `crates/humanity-systems/src/electrical.rs` (`ElectricalSystem`, `integrate_battery`, `PowerStatus`), `crates/humanity-systems/src/solar.rs` (`SolarSystem`)
- Data: `data/electrical.ron`
- ECS: `PowerGenerator`, `PowerConsumer`, `Battery`, `PowerCircuit` (island) components

### The ship's reactor, metered per home (2026-09-27)
The operator's energy decision (PRIORITIES Blocked #3b): in the default Ship life support mode
(Station-supplied) every home power island carries a feed tap to the ship's bus; after the island's own
generation and batteries the reactor (one KLT-40S-class plant, 30 MWe net per the IAEA/OKBM status report; 35 MWe gross) supplies the
rest, so a home never sheds, and every watt-hour drawn and returned is metered per home and utility in
f64, saved with the home, and shown on the Home card's Live power, the HUD power line and the Usage
meter. Realistic mode runs on the home's own generation and batteries. A buildable Solar Panel
blueprint (400 W, `generates`) offsets the draw one for one aboard, and powers a planet build site on
its own island, which the reactor never reaches. In a server's shared world the power a home
draws and returns is reported to that server's fleet ledger (built 2026-10-04, see "Fleet
ledger"; designed in docs/design/ship-life-support.md section 8).
- Native: `crates/humanity-systems/src/ship_power.rs` (`ShipFeed`, `ShipSupplyLedger`, `feed_watts`, `site_island`), `crates/humanity-systems/src/electrical.rs` (`ShipFeedReading`), `crates/humanity-systems/src/construction/mod.rs` (`wire_built_generators`), `src/engine/home_spawn.rs` (`spawn_home_feed_taps`), `crates/humanity-systems/src/crafting/away.rs` (`meter_away_reactor`)
- Data: `data/ship_power.ron`, `data/blueprints/basic.ron` (`solar_panel`)
- Tests: `crates/humanity-systems/src/ship_power_tests.rs`

### Home Machine Layout
The data-driven machine layout for the 3D home: a catalog of machine types, placed instances + arrays
(row x col grids), connections, conduit nodes/edges, and self-sufficiency loops. Machines carry a power
role (Solar / Generator / Consumer / Battery) and stat readouts; positioned by absolute box-home
coordinates. Editable live in the construction editor (place / move / wire / inspect).
- Native: `crates/humanity-ship/src/machines.rs` (`MachineHome`, `MachineDef`, `MachineInstance`, `MachineArray`, `MachineConnection`, `MachinePower`, `HomeLoop`, `placements`)
- Data: `data/machines/home.ron`
- Editor: `src/gui/pages/construction.rs` (`draw_machine_detail`)

### Household size: Family vs Solo home design (v0.656)
A second, fully self-sufficient home layout for ONE person alongside the existing 3-person
`home.ron`: `data/machines/home_solo.ron` (see `docs/design/homestead-solo-design.md` for the
full sizing derivation -- 4 solar / 2 battery / 1 wind / 1 generator, 1 cistern / pump /
purifier / household tap, 1 air recycler, 2 composters, 9 nutrition towers + 1 apothecary + 8
potato beds + 3 oilseed beds + 2 grain trays + 2 mushroom racks + 1 aquaponic tank + 1 grain
field + 1 legume field + 1 silo + 1 irrigation -- the same 34x34 m garden room that covers
only ~half the calories for 3 people closes ~94% for 1). Since ship homes increment 6
(2026-10-12) the household belongs to the SAVE, not the install: each start in
`data/homes/starts.ron` lists its households (the Homestead "A family of three" = `home.ron` and
"Just you" = `home_solo.ron`; the Cabin "Just you" = `cabin.ron`), the save records which
(`WorldSave::household`; a save from before is a family), and `machines::home_ron_path()` reads
the live save's start and household (`persistence::live_save::ActiveHome`) at every real `MachineHome::load`
call site. Chosen in Settings > Gameplay > "Where you live", which replaced "Home Design"
(Family / Solo, one setting per install in `AppConfig.home_variant`, now gone). Takes effect the
next time you enter the world, not live mid-session.
- Native: `crates/humanity-ship/src/machines.rs` (`home_ron_path`), `src/save_load.rs` (`ActiveHome`), `crates/humanity-ship/src/starts.rs` (`Starts::household`), `src/gui/pages/where_you_live.rs` (`draw_settings_section`), `src/gui/pages/settings.rs` (`draw_gameplay_content`)
- Data: `data/machines/home_solo.ron`, `data/machines/cabin.ron`, `data/homes/starts.ron`

### The Home page as cards that use the width (2026-10-30)
The operator, of the Home page: "everything is left aligned. There's a ton of unused space ... We
should join the two areas and make everything more visually friendly. Like the Closed-loop
self-sufficiency areas are brutal walls of text." The page is ONE scroll (the Household card and the
animals stood still above a scroll of their own, and the Household card had a capped third). Its
parts are cards in rows that fill the width, one column on a narrow window and as many as fit on a
wide one: the Household card beside Where you live; the Animals as a card each (the kind's tile, a
pill for doing well / needs care / dead, what it makes, its feed and water a day) or a table, one row
each, switched by a Table / Cards toggle; At a glance and the live power, water and air readouts side
by side, with gauges (make against use, the battery, the cistern); the home's loops as cards (a pill
for closed or short and "weakest loop", a gauge where the numbers are known: the food the placed
crops grow, and the power meter's make against use; the need and supply cut to two lines, whole on
hover; the rest behind "How this is worked out", which opens in a panel the width of the grid under
the card's row, so the long words read at a comfortable width and the cards beside it keep their
size); the ideal loops the same, or a table, each loop's
arithmetic (`balance`) and assumptions now shown in its fold (both were in the data and invisible);
the traded loops edged in the warning colour, why behind a fold; the bill of materials in columns;
each room a card. The web Home page mirrors the ideal loops (cards or a table).
- Native: `src/gui/pages/homes.rs` (`draw`, `draw_page`, `draw_design`, the readout cards, `room_card`), `src/gui/pages/home_loops.rs` (`draw_home_loops`, `gauge_for`, `draw_outline`, `draw_cannot_close`), `src/gui/pages/herd.rs` (`draw_card`, the cards and the table), `crates/humanity-systems/src/animal_care.rs` (`HerdRow`'s columns, `needs_parts`), `src/gui/widgets/cards.rs` (`Grid`, `status_badge`, `plain_badge`, `gauge`, `fold`, `clamped`), `src/gui/widgets/view_toggle.rs`, `src/gui/widgets/data_table.rs`
- Web: `web/pages/home.html`, `web/shared/view-toggle.js`, `web/shared/list-views.css`
- Tests: `gui::widgets::cards::tests`, `gui::widgets::view_toggle::tests::a_click_on_table_shows_the_table`, `gui::widgets::data_table::tests`, `gui::pages::home_loops::tests`, `systems::animal_care::tests::the_walk_up_lines_and_the_animals_card_say_it_plainly`, `scripts/tests/view-toggle-web.test.js`; snapshots `homes`, `homes_wide`, `homes_narrow_tables`

### "What one home cannot close" panel (Home page)
The pedagogical payoff of the homestead design (operator: "people need to see the bare minimum
for 100% self-sufficiency so they understand the importance of all supporting civilizational
infrastructure"). Directly below the closed-loop summary, a visually distinct outlined panel
(warning stroke, muted treatment -- deliberately NOT the green closed-loop styling) lists the
five loops no single homestead can close: electronics/semiconductors, metal from raw ore,
medicine synthesis, equipment replacement, and raw chemistry inputs. Each is a card (since
2026-10-30, edged in the warning colour): its title, a "traded" pill and the "provided by" trade
line; "Why it stays open" opens a plain-language body naming the game recipe that abstracts the
gap away (manufacture_cpu, smelt_steel, ...). Medicine names the opposite: since 2026-10-05 no recipe makes antibiotics (the
old craft_antibiotics cultured them from water, flour and sugar), and the trading post sells
them. Intro + footer carry the non-defeatist framing: these gaps ARE why
civilization exists. Data-driven (infinite-of-X): categories live in the RON, not code.
Since ship homes increment 6 (2026-10-12) the Home page shows "Where you live" (the live save's
start and household, read only) under the Household card, and draws the homestead's
section (its design, its loops and this panel) only when the loaded household file has loops,
so a Cabin's Home page has none.
- Native: `src/gui/pages/homes.rs` (`CannotCloseEntry`, `CannotCloseData`, `load_cannot_close`, the panel in `draw_design`, `home_has_loops`), `src/gui/pages/where_you_live.rs` (`draw_home_line`)
- Data: `data/self_sufficiency/cannot_close.ron` (distilled from `docs/design/homestead-solo-design.md` section 8)
- Tests: `gui::pages::homes::tests::{cannot_close_data_parses_and_is_complete, cannot_close_missing_file_is_empty}`

### Buildability Report (v0.524, v0.605 - v0.606)
A design-time validator surfaced in the editor with check marks. Checks: Power source (a consumer needs a
generator/solar), Energy balance (kWh/day generated vs consumed + overnight battery sizing), Wiring (no
connection dangles to a missing machine), Conduits (per power run, validate the pinned cable or auto-pick
the cheapest copper against the load + run length: ampacity + voltage drop), and Power circuit (union-find
over the power graph: every electrical LOAD must share a wired component with real generation; a battery
is storage, not a source).
- Native: `crates/humanity-ship/src/machines.rs` (`buildability_report`, `power_circuit_check`, `electrical_islands`, `BuildabilityReport`, `CheckStatus`), `src/gui/pages/construction.rs` (`draw_buildability`)

---

## Utility Wiring (v0.604 - v0.607)

Power, water, air, and data do NOT magically transmit through the air; they travel through cables and
plumbing with real limits (volts, watts, amps, AWG gauge, ampacity, shielded vs unshielded). A machine
declares physical IN/OUT ports by utility. Stages 1-3 shipped; the wire-A-to-B gizmo + the superconductor
upgrade mission are the next stages.

### Data Routing + Medium Picker (v0.621, telecom Stage 2)
The telecom utility's primary function: wire devices to the internet via a chosen medium, validated like
a power run. `Port` gained an `mbps` field; machines declare Data IN/OUT ports (a `home_server` demands
100 Mbps, a `network_uplink` supplies it). A "data" connection carries a medium `spec`; the editor's
utility-lines panel shows a per-data-run medium picker (auto / Cat6 / fibre / WiFi). A new "Data links"
buildability check sizes each data run (bandwidth + range via `check_data_link`); a WiFi link passes
exactly like a wired one wherever its bandwidth and range suffice. (Until 2026-09-27 it also warned that
a wireless link's RF can harm a grow; that warning went with the crop harm, see below.) The seed home
wires its uplink to its server over Cat6.
- Native: `crates/humanity-core/src/utilities.rs` (`Port.mbps`, `Port::data_in`/`data_out`), `crates/humanity-ship/src/machines.rs` (`data_demand_mbps`, the "Data links" check), `src/gui/pages/construction.rs` (the "data" kind + the data-medium picker)
- Data: `data/machines/home.ron` (`network_uplink`, `home_server`, a `data` connection on `eth_cat6`)

### Telecom RF -> Plant Harm (v0.620) -- REMOVED 2026-09-27
A powered `wifi_router` used to drain the health of every crop in the home. The operator decided "We'll
assume no wi-fi crop harm at this time" on the evidence in
`docs/reference/findings/2026-09-27-wifi-and-plants.md` (no source shows a household router harming a
garden at the distances plants sit from one), and the harm was removed that day: the FarmingSystem's
home-RF drain, the `RfEmitter` component and its spawn, `MachineDef.rf_emission`, and the Data-links RF
warning. The `wifi_router` stays as a powered network device (8 W, a wireless data port).
- Test: `powered_wifi_router_leaves_crop_health_unchanged` (`crates/humanity-systems/src/farming/mod.rs`)
- Data: `data/machines/home.ron` (`wifi_router`, placed as `router_1` in the study)

### Data / Telecom Media, Stage 1 (v0.619)
The internet/telecom utility: teach real telecommunications. Data is `Utility::Data` in the same
cable registry (the material catalog since 2026-11-02), with media that have real tradeoffs -- bandwidth, range, latency, cost, and RF
emission. Stage 1 ships the data model + link physics + 3 core media; emissions becoming detection
signatures + the full media catalog are later stages. (RF harming plants was built in v0.620 and removed
2026-09-27; the medium's `rf_emission` stays as the real property a detection layer would read.)
- Native: `crates/humanity-core/src/utilities.rs` (`ConduitType` data fields `bandwidth_mbps`/`range_m`/`latency_ms`/`wireless`/`rf_emission`, `ConductorMaterial::{Glass,Radio}`, `check_data_link`/`cheapest_data_link_for`/`data_media`)
- Data: `data/piping/pipe_materials.ron` (`eth_cat6` quiet wired workhorse and `fiber_om4` high-bandwidth no-RF, size rows; `wifi_6` convenient but RF-loud, a radio link row)
- Design: `docs/design/telecom.md` (the 21-media catalog + the emissions-as-signature design + the staged plan)

### Conduit / Cable Data Model + Physics
A closed `Utility` enum (Electricity, Water, HotWater, Air, Data, Fuel, Nutrient, Waste);
`Port { utility, dir: In/Out/Bidirectional, label, watts, flow_lpm, anchor }`; a cable registry with real
NEC-ish copper specs (AWG, ampacity, voltage rating, ohm/m, cost/m, grade). `check_cable` computes amps +
round-trip voltage drop into Pass/Warn/Fail; `cheapest_cable_for` is the auto-picker; `awg_to_mm2` for
display.
- Native: `crates/humanity-core/src/utilities.rs` (`Utility`, `Port`, `ConduitType`, `ConductorMaterial`, `Grade`, `check_cable`, `cheapest_cable_for`, `conduit_types`)
- Data: `data/piping/pipe_materials.ron`, the one material catalog (copper 14/12/10 AWG home and 6 AWG industrial shielded as sizes of the power cord, the `sc_room_temp` superconductor upgrade target, two water pipes; `data/utilities/conduits.ron` until 2026-11-02)
- Design: `docs/design/utility-wiring.md`

### Superconductor Bulk-Upgrade (v0.616)
The late-game wiring payoff: an "Upgrade all power runs to superconductor" button in the utility-lines
editor sets every power connection's `spec` to the room-temperature superconductor (near-zero loss, huge
ampacity, so the Conduits check goes all-green); "Reset to auto" reverts to cheapest-copper auto-sizing.
The action ships now; a future quest gates earning it.
- Native: `src/gui/pages/construction.rs` (the bulk-spec buttons in `draw_machines_and_connections`), `data/piping/pipe_materials.ron` (`sc_room_temp`)

### Per-Connection Cable Picker (v0.615)
The utility-lines editor gives every POWER run a cable dropdown: "auto (cheapest copper)" or a pinned
type from the registry (copper 14/12/10/6 AWG ... the room-temp superconductor). Picking sets the
connection's `spec`; the Conduits buildability check then validates it (over-ampacity or >5% voltage drop
-> warn/fail), so the whole cable-physics system is finally interactive in the editor.
- Native: `src/gui/pages/construction.rs` (`draw_machines_and_connections`, the cable ComboBox writing `MachineConnection.spec`), choices from `crates/humanity-core/src/utilities.rs::conduit_types`

### Machine Ports + Conduit Checks (v0.605 - v0.606)
`MachineDef` gained `ports: Vec<Port>` + a `derive_ports()` fallback (electrical ports inferred from the
power role; fluid ports declared); `MachineConnection` gained `spec: Option<String>` (a pinned cable id,
else auto-pick). The Conduits + Power circuit buildability checks consume these. The seed `home.ron` is a
physically connected network (PV + wind + generator to battery bus to loads).
- Native: `crates/humanity-ship/src/machines.rs` (`MachineDef::derive_ports`, `MachineConnection.spec`), `crates/humanity-core/src/utilities.rs`

### Runtime Power-Flow Gating (v0.607)
Each spawned power entity carries a `PowerCircuit { island }` from `MachineHome::electrical_islands`, so
`ElectricalSystem` balances + sheds PER ISLAND instead of summing the whole world. A load on an
unconnected circuit is shed (no magic transmission). Entities without the component fall into one shared
bucket (the old global behaviour, for tests/legacy).
- Native: `crates/humanity-core/src/ecs/components.rs` (`PowerCircuit`), `crates/humanity-ship/src/machines.rs` (`electrical_islands`, `power_component_roots`), `crates/humanity-systems/src/electrical.rs`

### Live Water / Plumbing Sim + Power Coupling (v0.608)
The water mirror of the electrical sim, and the first POWER -> WATER consequence chain. A machine's
water producers/consumers derive from its PORTS (`flow_lpm`); a cistern's capacity from
`MachineDef.storage`; `water_islands` groups them per pipe network. `PlumbingSystem` fills/drains the
cistern per island and publishes a live `WaterStatus` (production, demand, stored, days autonomy). A
producer/consumer flagged `needs_power` only flows while the SAME entity is powered -- cut the power and
the pump stops, the cistern drains. Shown on the Home page next to Live power.
- Native: `crates/humanity-systems/src/plumbing.rs` (`PlumbingSystem`, `WaterStatus`), `crates/humanity-core/src/ecs/components.rs` (`WaterTank`, `WaterProducer`, `WaterConsumer`, `PlumbingCircuit`), `crates/humanity-ship/src/machines.rs` (`water_islands`, `MachineStorage`, `water_production_lpm`/`water_demand_lpm`/`water_capacity_l`), `src/gui/pages/homes.rs` (Live water card)
- Data: `data/machines/home.ron` (cistern storage + rain inflow, pump water-out, tower/irrigation water-in)

### Live Air / Atmosphere Sim + power -> air -> Vitals (v0.617 - v0.618)
The 3rd life-support utility, with its consequence chain. The AtmosphereSystem (now REGISTERED) ticks the
home's sealed air space (a HomeAir + HomeMachine tagged `EnclosedSpace`), publishing a live `AirStatus`
(O2/CO2/pressure/temp/breathable) to a "Live air" Home-page card beside power + water. **Stage 2 (v0.618):**
occupancy (a ~3-person household) steadily drains O2 + raises CO2; a powered `air_recycler` (an `AirScrubber`
derived from an Air-OUT port, gated on its PowerConsumer) offsets it. Cut the grid -> the recycler sheds ->
O2 falls -> unbreathable -> the inside-homestead `EnvironmentContext.oxygenated` flips off -> the FoodSystem
drains the player's `Vitals.oxygen` -> hypoxia. So **power -> air -> Vitals** runs end to end.
- Native: `crates/humanity-systems/src/atmosphere.rs` (`AtmosphereSystem`, `AirStatus`, `HomeAir`, `AirScrubber`, the occupancy/scrubber dynamics), `src/lib.rs` (`spawn_home_air_space`, the AirScrubber spawn from an Air-OUT port, `EnvironmentContext.oxygenated = breathable`), `src/gui/pages/homes.rs` (Live air card)
- Data: `data/machines/home.ron` (`air_recycler`: a critical-priority power consumer + an Air-OUT port, wired to the battery bus)

### Water to Food Coupling (v0.611)
The downstream end of the power to water to food consequence chain. The `FarmingSystem` reads the live
`WaterStatus`: if the home has a real cistern and it has run DRY, automated irrigation can no longer top
crops up, so they dehydrate and lose health (existing crop water-stress logic). Cut the power, the well
pump sheds, the cistern drains over days, then the garden starts to wilt. Absent water system / no
cistern = water available (un-plumbed homes + tests unchanged).
- Native: `crates/humanity-systems/src/farming/mod.rs` (the `water_available` gate on the per-area irrigation top-up)

### Node-Based Conduits (v0.535, v0.581)
Conduit junction nodes + auto-routed edges in the editor (draggable node gizmos), plus the
Manhattan/service-height auto-router that runs pipes up to the ceiling and down to the fixture
(auto-placing brackets, elbows, and wall-passthrough gaskets).
- Native: `crates/humanity-ship/src/machines.rs` (`ConduitNode`, `ConduitEdge`, `ConduitEnd`), `crates/humanity-ship/src/conduits.rs` (`ConduitKind`, `ConduitRoute`, `route_conduit`)
- Editor: `src/gui/pages/construction.rs` (`draw_conduit_node_detail`)

---

## Game Data

### Chemistry Database
118 elements, 59 alloys, 132 compounds, 35 gases, 52 toxins across 5 CSV datasets.
- Data: `data/chemistry/elements.csv`, `data/chemistry/alloys.csv`, `data/chemistry/compounds.csv`, `data/chemistry/gases.csv`, `data/chemistry/toxins.csv`

### Solar System Database
70+ celestial bodies with orbital parameters, physical properties, and RON planet definitions.
- Data: `data/solar_system/bodies.json`, `data/solar_system/earth.ron`, `data/solar_system/mars.ron`, `data/solar_system/sun.ron`

### Materials Database
92 materials with properties and categories.
- Data: `data/materials.csv`

### Components Database
102 components for crafting and construction.
- Data: `data/components.csv`

### Items Database
~781 items for crafting, construction, and gameplay. (Counts here are
approximate and drift as data grows; run `just data-counts` for live figures.)
- Data: `data/items.csv`

### Recipes Database
~444 recipes for crafting and construction.
- Data: `data/recipes.csv`

### Plants Database
164 plants with growth stages, climate requirements, and harvest data.
- Data: `data/plants.csv`

### Creatures Database
99 creatures with behaviors, stats, habitats, and loot tables.
- Data: `data/creatures.csv`

### Spells Database (v0.90.0)
149 spells across multiple schools of magic with mana costs, cooldowns, and effects.
- Data: `data/spells.csv`

### Structures Database (v0.90.0)
163 structures for construction with material costs and placement rules.
- Data: `data/structures.csv`

### Status Effects Database (v0.90.0)
80 status effects (buffs, debuffs, conditions) with duration and stacking rules.
- Data: `data/status_effects.csv` (every column read since BUG-162; stacking is
  read but not acted on yet)

### Illness and medical items (BUG-162, 2026-10-05)
Food poisoning takes water over game days and passes on its own (Realistic two
days, Forgiving one, Settings > Gameplay > Illness and injury); drinking puts the water back,
oral rehydration solution most (sachets in the starting kit, mixed by the
`mix_ors` recipe); harm only through dehydration. The Inventory's Use button works
for the medical items, each from data: health restored, effects ended by tag
(antibiotics: `bacterial` only), a plain reason when it cannot help. Spoiled or
raw food eaten while still ill does not start it over (`again_adds_h`, 0 for food
poisoning) and the player is told the time it still has to run. The body's needs,
its health and the illness's countdown are exact at any frame rate (BUG-164,
`change_exactly` and f64 effect timers).
- Code: `crates/humanity-systems/src/illness.rs`, `crates/humanity-systems/src/treatment.rs`, `crates/humanity-systems/src/food.rs`
- Data: `data/medical/illnesses.ron`, `data/medical/treatments.ron`

### Wounds and healing (first-hour audit S8, 2026-10-12)
A creature's bite takes health and leaves Bleeding (stops on its own in about ten
minutes, or at once with a Bandage or Medkit) and a Bite Wound (heals over days); a
fire leaves a burn by how long you stood in it: red skin, blistered or deep (no fire
burns anyone in play yet: FireSystem is not registered). A wound that can go bad
may turn into an Infected Wound a day or two in, far less often once a Bandage, a
Medkit or Antiseptic has cleaned and covered it (`Wounds Cared For`; a new wound is
not covered); Antibiotics end it. A body heals by itself when nothing is harming it:
Realistic about three weeks from near death, Forgiving about three days, 1.8 times as
fast asleep, half as fast Hungry or Thirsty, never while bleeding, infected or ill.
Well Fed no longer heals. Both modes are Settings > Gameplay > Illness and injury.
The HUD shows a row per wound and illness with what helps; the Inventory's Status
section says how each passes. Wounds are status effects, so the save keeps them.
MedicalSystem, its `MedicalConditions` component and the never-read
`data/medical.ron` were deleted.
- Code: `crates/humanity-systems/src/wounds.rs` (data and rules), applied in `crates/humanity-systems/src/food.rs`'s
  body pass; injuries from `crates/humanity-systems/src/combat/mod.rs` and `crates/humanity-systems/src/fire.rs`;
  tests `crates/humanity-systems/src/wounds_tests.rs`
- Data: `data/medical/wounds.ron`, the `wound` rows of `data/status_effects.csv`,
  `cares` in `data/medical/treatments.ron`

### Falls (2026-10-20)
A landing hurts by how fast the floor stops you, so the same rule holds under the
ship's spin gravity, on the Moon and on Earth (a 4 m drop on the Moon lands at 3.6
m/s and does nothing; it takes about 24 m there to land as hard as 4 m on Earth).
In Realistic, on Earth: about 2 m leaves a Sprained Ankle (two weeks, limping),
about 3 m a Broken Ankle (six weeks), about 6 m a Broken Leg (twelve weeks), each
slowing walking while it heals, and from full health about 15 m kills ("a fall" on
the death screen). Every speed is a cited fall height through v = sqrt(2 g h): the
field triage guideline's 10 ft and 20 ft, Warner and Demling's two and three
storeys, Risser's five storeys, the NHS's healing times. Forgiving, the default
(Settings > Gameplay > Illness and injury), reads a fall at 70% of its speed, so
it needs about twice the height and hurts less. Landing in water, a fall that began
in dev flight (fly mode, F9's hover, the flight band), and the landing after the
body was placed (a teleport, driving, the rig's camera hold) hurt nobody; a step
and the game's own jump are well under the first harm. The HUD row reads "Broken
Ankle: from a fall; you hobble". Letting go of a ladder's keys part way up now
holds on there (it used to drop you); stepping away from it is a fall.
- Code: `crates/humanity-systems/src/falls.rs` (data, `FallWatch`, `player_landed`); the landing
  aboard in `CameraController::update_first_person` (`src/renderer/camera.rs`), on
  a planet in `surface_move::radial_step` (`RadialStep::impact_m_s`) handed to
  `CameraController::surface_step`; lib.rs takes it once a frame
  (`take_landing`); the injury in `Wounds::take_injuries` (`crates/humanity-systems/src/wounds.rs`);
  tests in `crates/humanity-systems/src/wounds_tests.rs`
- Data: `data/medical/falls.ron` (speeds, harm, sources), the `sprained_ankle`,
  `broken_ankle` and `broken_leg` rows of `data/medical/wounds.ron` and
  `data/status_effects.csv` (which replaced the never-applied `broken_bone`)
- Checked in the running game: `just verify-falls` (`scripts/verify-falls.js`, judged by
  `scripts/lib/falls-judge.js`, 2026-10-20; brought up to the homestead's upper floor
  2026-10-21) places a 3 m deck in the greenhouse with the build editor's console (the
  showcase `console` verb, for that session only) and walks off its edge by holding
  forward, in Forgiving (a landing reaches the body and does nothing) and Realistic (a
  Sprained Ankle, the health taken on the landing frame); walks down the 16-riser stairs
  (no landing); climbs the ladder and steps off its top through the hatch's gate onto the
  upper floor (no fall, no wound); drops the player from a placement (no landing reaches
  the body); and checks world entry hands the body no landing. The dev IPC's body probe (`src/engine/body_probe.rs`): health and
  wounds in the recorder's home session vitals, health on every recorded frame, a `falls`
  block (landings handed to the body, the last speed, the mode), the showcase `illness`
  verb, and a `[Falls]` run.log line per landing

### Enchantments Database (v0.90.0)
133 enchantments for equipment with tier scaling and compatibility rules.
- Data: `data/enchantments.csv`

### Trade Goods (v0.90.0)
185 trade goods with balanced pricing, weight, categories, and regional availability.
- Data: `data/trade_goods.ron`

### Factions (v0.90.0)
Faction definitions with relations, territories, and reputation thresholds.
- Data: `data/factions.ron`

### Biomes (v0.90.0)
Biome definitions with flora, fauna, climate parameters, and resource distribution.
- Data: `data/biomes.ron`

### Tech Tree (v0.90.0)
Technology progression tree with prerequisites, costs, and unlock rewards.
- Data: `data/tech_tree.ron`

### NPCs (v0.90.0)
NPC definitions with dialogue triggers, schedules, and trade inventories.
- Data: `data/npcs.ron`

### Dialogues (v0.90.0)
Dialogue trees with branching choices, conditions, and consequences.
- Data: `data/dialogues.ron`

### Particle Emitters (v0.90.0)
15 particle emitter DEFINITIONS (fire, smoke, sparks, rain, snow, dust, magic_sparkle, engine_exhaust, healing, explosion, bubbles, welding, dive_bubbles, leaf_drift, space_dust). Only leaf_drift, dive_bubbles, space_dust, rain, snow and the weather-event emitters (dust, sparks) are ever spawned; fire, smoke, explosion and the rest are data with nothing that uses them yet. See "Particle System" under rendering for the detail.
- Data: `data/particles.ron`

### Sound Configuration (v0.90.0)
Sound effect and music configuration with volume, spatial, and category settings.
- Data: `data/sounds.toml`

### Offline Behaviors (v0.90.0)
Autonomous agent presets for off-screen NPC simulation (patrol, trade, farm, build, explore).
- Data: `data/offline_behaviors.ron`

### Simulation Systems (v0.90.0)
Data-driven simulation modules for engineering and infrastructure. **⚠️ Most consuming systems are still
unregistered scaffolds (see `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS`); their data files exist but
nothing consumes them at runtime. EXCEPTIONS now LIVE: `ElectricalSystem` + `SolarSystem` (home power
sim) and `PlumbingSystem` (home water sim, v0.608) -- see "Home Power and Electrical Sim" + "Utility
Wiring" above. The old `plumbing.ron`/`WaterFixture` scaffold was removed when PlumbingSystem went live.**
- Data: `data/electrical.ron`, `data/hvac.ron`, `data/transportation.ron`, `data/fire_system.ron`, `data/docking.ron`

### Real-World Systems (v0.90.0)
Data definitions for social and biological simulation. **⚠️ The consuming systems are scaffolds, NOT registered, they never tick (see the lint); data exists but isn't consumed at runtime.**
- Data: `data/governance.ron`, `data/psychology.ron`, `data/food_system.ron`, `data/economy.ron`, `data/creative_arts.ron`, `data/aging_fitness.ron` (`data/medical.ron` was deleted 2026-10-12: see "Wounds and healing")

### Science Systems (v0.90.0)
Data definitions for natural science simulation. **⚠️ The consuming systems are scaffolds, NOT registered, they never tick (see the lint); data exists but isn't consumed at runtime.**
- Data: `data/geology.ron`, `data/oceanography.ron`, `data/astronomy_tools.ron`, `data/genetics.ron`, `data/manufacturing.ron`, `data/waste_management.ron`

### Data Schemas (v0.90.0)
24 TOML schema files documenting all data formats for modding and validation. (Count corrected 2026-07-01: the old "22" omitted room; chore added with the crew chore AI.)
- Data: `schemas/*.toml` (item, material, component, creature, spell, structure, status_effect, enchantment, recipe, quest, biome, celestial_body, faction, npc, chore, room, skill, sound, vehicle, weather, economy, offline_agent, equipment_slot, container)

### Platform Brand SVGs
Platform detection icons (Steam, Epic, GOG, PlayStation, Xbox) as inline SVGs.
- Assets: `assets/icons/platforms/`

### Total: ~215 data files, thousands of entries (`just data-counts` for live figures)

---

## UI Foundation (v0.92.0)

### Design System Spec
Canonical reference for tokens, components, and dual-UI parity rules. Must be read before any widget or page work.
- Docs: `docs/design/ui-system.md`

### Infinite-of-X Principle
Enforced rule: anything that can exist more than once is a data file, not code. Includes a pre-ship checklist and current audit of hardcoded instances.
- Docs: `docs/design/infinite-of-x.md`

### Theme Token Pipeline
Single source of truth for colors, spacing, radii, fonts. Native reads `data/gui/theme.ron` directly. Web's `theme.css` is regenerated from the same file by a Node script. Editing the RON updates both UIs after running the generator.
- Data: `data/gui/theme.ron`
- Native: `src/gui/theme.rs`
- Web: `web/shared/theme.css` (auto-generated section marked with comments)
- Script: `scripts/gen-theme-css.js`

### Universal Help Modal
`data/help/topics.json` is shared between web and native. Both UIs load it on startup and show the same help content. Help buttons (`?`) anywhere in the UI open a themed modal with the topic body. The native `help_button`/`draw` plumbing was fully wired at the app level (registry loads at startup, modal draws in the render loop) since it shipped, but had ZERO real call sites in any native page until v0.658 wired 3 into the Studio page (Scenes, Resolution/Bitrate/FPS, Chat Overlay) -- the first native page to actually use it.
- Data: `data/help/topics.json`
- Native: `src/gui/widgets/help_modal.rs` (help_button + draw fn + HelpRegistry loader), `src/gui/pages/studio.rs` (first real adoption)
- Web: `window.hosHelp.register/show` in `web/shared/shell.js`, plus `[data-help-id]` attribute on any button

### Anchored Help Button (dual UI, v0.1212)
One `?` button pinned to the top right of every screen, which becomes an `X` while the
panel is open so the way in and the way out are the same pixel. It is the single
tutorial entry point on both clients, and it replaced the onboarding tour's auto-popup:
the tour used to open itself 2s after load behind a full-viewport click catcher, which
also swallowed the first tap anywhere on the page. The tour still exists, opt-in, from
inside the panel.
The panel is per-page and data-driven: prose from the shared `data/help/topics.json`
(its `pages` map pairs a route with topic ids; `native_pages` aliases the screens whose
name is not the route), keys from `data/keymaps.ron` on native and
`data/keybindings/web.json` on web. Only bindings that actually resolve are listed.
- Absent in two places on native: the first-person world view (the screen belongs to the
  game there) and MainMenu (neither draws a nav bar to reserve the corner). F1 still
  works everywhere, including both.
- **F1 is unchanged**: hold to glance, non-interactable, centred, exactly as since
  v0.465. The button pins the SAME panel, which additionally scrolls and shows prose.
  Prose is pinned-only ON PURPOSE: the F1 overlay does not scroll, so a page with topics
  (`/chat` is 3 topics, 14 paragraphs) overflowed a 900px window and clipped key rows
  out of reach. Guarded by `snapshot_help_f1_prose_page`.
- Data: `data/help/topics.json` (`pages`, `native_pages`), `data/keymaps.ron`, `data/keybindings/web.json`
- Native: `src/gui/pages/keymap.rs` (`draw_help_toggle` + `draw`), `src/gui/widgets/help_modal.rs` (`topics_for_page`), `src/gui/pages/escape_menu.rs` (reserves the nav corner), `src/lib.rs` (per-frame draw + Esc precedence)
- Web: `initHelpPanel` in `web/shared/shell.js` (`#hos-help-toggle` / `#hos-help-panel`)
- Snapshots: `tests/snapshots/help_button_closed.png`, `help_panel_open.png`, `help_f1_world.png`, `help_f1_prose_page.png`

### Real/Sim Help Icon
Built-in help topic `real-sim` explains the context toggle. Rendered as a `?` next to the Real/Sim pill in both the native nav bar and the web hub nav.
- Native: `src/gui/pages/escape_menu.rs`
- Web: `web/shared/shell.js` (buildContextToggle)

### Onboarding Page (dual UI)
First-run orientation plus permanent reference. Four core concepts, core-pages overview, data-driven quest chains. Progress tracked locally per step.
- Data: `data/onboarding/quests.json` (version 5: six chains, 26 steps; each chain has a one-line `purpose`, a step may name its written `how_to`)
- Native: no page. `GuiPage::Onboarding` and its nav tab went in v0.415.0, and `onboarding.rs` was removed on 2026-10-30; the chains are the Tasks page's suggested cards (`src/gui/pages/tasks.rs`)
- Web: `web/pages/onboarding.html`, the chains as quest-style cards with their purpose and a chain window per chain (Go there, and Open the how-to where a step has one); the website's Tasks page has them as suggested cards too
- Route: `/onboarding` (web only)

### Universal Spreadsheet / Nested-Row Widgets (v0.400 - v0.517)
The one-panel inventory redesign's reusable primitives: a nested expandable row, a fixed-width row cell,
a collapsible section disclosure, item swatch tiles, and the recursive nested-container renderer (person
to shirt to pocket to wallet spatial inventory) with cross-container item transfer that persists across
restart.
- Native: `src/gui/widgets/mod.rs` (`expandable_row`, `row_cell`, `section_disclosure`), `src/gui/pages/inventory.rs` (`draw_container`, `item_tile`), `src/gui/mod.rs` (`Place`), `crates/humanity-systems/src/inventory/placed.rs` (`PlacedItem`)

---

## Developer Tooling

### The play journal and F8, Mark this moment (2026-10-24, rung 1 of docs/design/play-journal.md)
While the game runs it keeps what happened, by kind, in small capped files in a `journal` folder
beside its logs (the operator's install: %APPDATA%/HumanityOS/journal): `log` (every warning and
error line), `panic`, and `mark`. F8 on any page (or the dev IPC's `{"action": "mark"}`) writes
a numbered mark with that frame's picture (`marks/<n>.png`, the last 30 kept). A Claude session
reads it through the dev MCP, even after the game closed: `dev_play_marks` lists the marks,
`dev_play_journal {since_mark: 12}` gives everything after one. Never in it: message words, keys,
tokens, recovery phrases. `crates/humanity-core/src/play_journal.rs` (its frame half `src/engine/play_journal_frame.rs`), `data/dev/journal_kinds.json`,
`scripts/mcp-server.js`. In the app: Dev > Journal (`src/gui/pages/dev_journal.rs`, rung 2): each
kind's count and newest event, the marks with Open picture, Mark this moment, Open the folder and
Clear the journal. Rung 3 (the same day): `page` (each page change), `frame` (hitches: 100 ms and
three times the recent average), `world` (entered, aboard, the room, death, falls, saves) and
`net` (signed in, connection lost, a refused line and why, a refusal by someone's choices, never
who), from one cheap call a frame plus calls where those events happen. Rung 4: every rig vantage
marks itself (labelled, no picture), and a failed vantage's manifest entry and log carry what the
game recorded since its mark (`scripts/probe-sweep.js`, `scripts/lib/play-journal.js`).

### Code map for AI sessions (2026-10-18)
One generated line per source file (every `.rs` under `src/`, every `.js` under `web/`): what its
header comment says it is, and its main public items, with a one-line-per-folder index first. A
session or helper agent reads the Folders section (about 5 KB) and greps the rest instead of
opening hundreds of files to find where something lives. Never edited by hand; `just brief`
refreshes it every session, and `--check` says whether it is stale. CLAUDE.md's START HERE points
at it; the same day CLAUDE.md lost its 34 KB cryptography table to
`docs/reference/cryptography.md` (122 KB to 89 KB, loaded by every session and every helper).
- Generator: `scripts/code-map.js`; output `docs/ai/code-map.md`; recipe `just code-map`

### Rules that check themselves (2026-10-19)
Four of CLAUDE.md's "Known gotchas", each the cause of a past incident, turned into checks that
fail on their own, each proven red against the original mistake:
- `tests/schema_alter_order_lint.rs`: nothing in `Storage::open` names a column before the ALTER
  that adds it (BUG-046 took the live relay down; the v0.262.2 crash loop was the same with an
  INSERT). In `just lints` and CI.
- `tests/bind_group_entry_lint.rs`: every `create_bind_group` site carries the same bindings as
  the layout it is built from, followed through fields, functions and callers; a site it cannot
  resolve fails (the v0.1029 to v0.1038 world-entry panic). In `just lints` and CI.
- `scripts/check-encoding.js`: refuses a file that gains a byte-order mark or UTF-16, and
  mojibake in added lines, the marks of a PowerShell `Get-Content`/`Set-Content` round trip;
  `--fix <file>` repairs one. Pre-commit, `just lints`, CI, `just check-encoding`. Checked with a
  real PowerShell round trip.
- `scripts/tag-release.js` (`just tag`, `--release <notes>`, `--dry-run`): tags HEAD with the
  version read from the committed Cargo.toml and refuses an empty version, an existing tag, an
  uncommitted bump, or a HEAD not on origin/main (the stray `v` and `v0.1212.2` tags). `just
  release` stages only the version stamps and tags through it.
- Tests: `scripts/tests/check-encoding.test.js`, `scripts/tests/tag-release.test.js` (temporary
  repos; push and gh are recorded, never run).

### Low impact: rigs that keep running while the operator plays (2026-10-22)
The operator: "I'd rather development testing continue if possible ... minimum graphics vs max."
A GeForce cannot be split, so what a rig does while one of his GPU-heavy programs runs
(`scripts/lib/operator-apps.json`, and his own HumanityOS: any copy outside a rig folder, a
worktree, a target or a temp folder, `machine-guard.js isOperatorGame`) depends on what its verdict is made of, one table for every rig
(`scripts/lib/rig-needs.js`): a rig whose checks are about behaviour (verify-runtime's
boot-and-enter check, verify-storeys, verify-falls, verify-copresence and its --build, --starts and
--household modes, verify-screens, verify-live-screen) does not wait for it, it boots its game in
LOW IMPACT beside it; a rig that judges the picture or times frames (probe-sweep, photograph-home,
make-clips, boot-timing, perf-report) waits for it to close and refuses low impact. `--low-impact`
and `--full-graphics` override per run. Low impact is one environment variable the rig sets for
its game only (`HUMANITY_LOW_IMPACT`, read once at boot, never shown to players): the rig-only
"minimum" graphics preset laid over the sandbox config and never saved (clouds off, bounced light
at Ambient, sun shadows, god rays, ambient occlusion and grass off, few 3D tree models), VSync off
and a 15 fps cap, a 960 x 540 window not maximized, the god-ray, aurora and Sun-glare passes off,
and the showcase pins that would put any of it back held. Nothing the game does changes. The game
confirms with `debug/low_impact.json`; a game that never does is stopped. Every manifest records
`low_impact`; perf-report refuses a low-impact sweep; a low-impact verify-runtime PASS says which
passes it did not cover.
- Engine: `src/engine/low_impact.rs`; hooks in `src/config.rs` (load / save), `src/lib.rs` (the
  window, the renderer switches), `src/engine/ipc.rs` (held pins), `src/engine/launch_focus.rs`
- Rigs: `scripts/lib/rig-needs.js`, `scripts/lib/game-launch.js` (point 7),
  `scripts/lib/machine-guard.js` (`own.lowImpact`, `operatorApps`)
- Data: `data/gui/graphics_presets.json` ("minimum", `rig_only`)
- Tests: `scripts/tests/rig-needs.test.js`, `machine-guard.test.js`, `game-launch.test.js`,
  `perf-report.test.js` (`just rig-tests`); `src/engine/low_impact.rs` tests (`just test-lib low_impact`)

### Headless UI Snapshots
Renders native egui pages to PNGs via an offscreen egui-wgpu + wgpu pipeline (no extra dependency; egui_kittest was rejected over an accesskit / egui-winit 0.31.1 incompatibility), so the native UI can be reviewed without launching the app. Output lands in `tests/snapshots/`.
- Native: `src/gui/ui_snapshots.rs`
- Output: `tests/snapshots/` (PNG)
- Recipe: `just snapshots`

### Build / Verify Recipes
Convenience recipes for the pre-push gate. `just verify` runs both feature builds (native + relay) plus lib tests and lints; `just lints` runs the four `src/gui` file-scanner lints via standalone rustc (Windows-PDB-safe, dodges the LNK1318 limit); `just snapshots` renders the UI PNGs; `just preflight` checks untracked source + doc links then runs verify.
- Recipes: `Justfile` (`verify`, `lints`, `snapshots`, `preflight`)
- **Co-presence rig (v0.1441.0):** `just verify-copresence` boots one background game
  against a throwaway relay (`scripts/lib/throwaway-relay.js`, shared with
  `just verify-second-player`), has `scripts/second-player.js` cross in front of the
  camera, and judges what the GAME drew, frame by frame, with `scripts/lib/copresence-judge.js`
  (unit tests in `just rig-tests`). The recorder is a dev request,
  `debug/remote_players_request.json` (`src/engine/ipc.rs`): each frame's drawn remote
  players, the frame clock, the computer's clock and the camera. `--dry-verdict
  <manifest.json>` re-judges a saved run.

### Crash-Safe Logging (v0.601)
A file logger that tees every log line to disk (flushed per line) plus a panic hook, so a windowed exe
that crashes leaves the cause on disk even with no console. Truncated `run.log` per launch + an appended
persistent `crash.log`, under `%APPDATA%/HumanityOS/logs` (Windows) / `~/.local/share/HumanityOS/logs`
(Linux).
- Native: `src/lib.rs` (`init_logging`, the `std::panic::set_hook` panic hook, `log_dir`)

---

## Unified Chat + Co-presence + Dev Tools (v0.771 - v0.779)

### In-World Chat (feed + interactive panel)
The SAME relay chat visible in the 3D world: a paint-only bottom-left feed of the
active PUBLIC channel (DM/group channels fall back to #global so private text never
paints on the world overlay / a stream), and an interactive panel opened with Enter
(frees the cursor, disables look/move, swallows gameplay hotkeys while typing; Esc or
click-away closes, typed text survives a dismissed panel and an aborted send). Channel
switcher mirrors the Chat page semantics (clear + refetch + unread clear).
The feed shows each message as ONE styled line read the way the Chat page reads it
(2026-09-27): bold, italic, code, strike, links and quotes are styled by the Chat
page's own span painter (`widgets::row::append_formatted`) and their markers never
show; a fenced code block becomes inline code where it sat, list items keep their
bullet, and the line elides at the box edge with an ellipsis.
- Native: `src/gui/pages/hud.rs` (feed), `src/gui/pages/chat.rs` (`draw_ingame_chat`,
  `channel_display_label`, `one_line_formatted`), `src/lib.rs` (modal input guards
  keyed on `GuiState::in_world_modal_open`, Enter handler)

### Shared-World Co-presence (visible)
Auto-joins the relay's shared game world whenever in-world + connected (no launcher
step); HUD top-left shows "Shared world - <host>" + a live roster of other players
(entity-count, duplicate names NOT collapsed). The session survives menu round-trips
(net_sync keeps applying updates; the relay treats a duplicate game_join as a RESYNC
and re-sends the welcome + snapshot). Relay reaps ghost player entities on restart
(persisting their progress) so counts stay honest and rejoins work. Each other player's name floats over
their teal figure, the way the crew's do (v0.1421.0, `engine::net_route::nameplate_labels`: out to 40 m,
hidden behind walls like the crew's; the relay always sent the name, nothing drew it). In the shared world the host's
clock wins (v0.1424.0, operator decision): the relay's `game_time_sync` sets the game clock, its speed and its calendar
(24-hour days, v0.1427.0) while the
player is joined, crops keep their age across the jump (`time::REBASE_SLOT`), and the bed says the night can't be
slept away there. The shared world's clock runs in real time (1x) by default, like a new solo game (operator,
2026-10-04 evening: "For normal mode, especially for my MMO server, let's have everything be real time, not the 72x.
That way anyone joining isn't dealing with accelerated death."; it had shipped at 72x earlier that day with the admin
control he asked for: "let's do 72x but, make sure there's admin tools for me to adjust it from inside the app"):
the server setting `world_time_scale` (`server_settings`, 1 to 1,000, `relay::storage::default_world_time_scale`),
changed in Server Settings > ADMIN > Shared world clock (native
`src/gui/pages/world_clock_admin.rs`, web: the chat's Game Admin window), which says in words what the picked speed
means from the data (at 1x a day takes a real day and a lettuce its real 45 days; at 72x a day passes in 20 minutes
and a lettuce grows in about 15 hours). The relay applies it to the running world at
once and sends a `game_time_sync` carrying `time_scale` to every game (`handlers/server_settings_update.rs`
`set_world_clock`); a game runs at the host's speed between words (`time::HostClock`). Only the clock is scaled: the
crew walk and do their chores in real seconds. Each other player looks like themselves (v0.1430.0 and v0.1431.0, `crates/humanity-core/src/player_look.rs`): their
skin tone, hair colour and height travel in `game_join` (clamped by the relay and the client), and their figure's head
and a cap of hair wear them, sized by their height (`net_route::remote_figure_parts`); the body stays teal.
- Native: `src/lib.rs` (multiplayer block, roster mirror), `src/gui/pages/hud.rs`
- Server: `src/relay/handlers/msg_handlers.rs` (`handle_game_join` resync + 48-char
  name clamp + name stamp), `src/relay/handlers/game_state.rs` (ghost reap in
  `restore_from_db`)

### Server Join Surface (launcher + public counts)
The character-select launcher lists the server you are connected to (virtual row,
deduped against bookmarks; a bookmark of the live connection counts as connected) with
live `/api/server-info` details and a working "Enter World". `/api/server-info`
exposes `game_players` (avatars in-world, distinct from chat `users_online`) - the
public mirror of the in-world roster, shown as "In world" in the launcher pane. Since ship
homes increment 6 (2026-10-12) the connected server's card also asks the server where you live
on its ship (`game_home_request`) and shows your home there, or each kind of home with how many
are free and a Choose button (`game_choose_home`), with the note that a server starts you fresh
and keeps your home there on this computer; Enter on a server pairing plays that server's own
save. The WHERE column's new-home row is "A new home", offering "Choose where you live" while
your own home has no save yet.
- Native: `src/gui/pages/showroom.rs`, `src/gui/pages/where_you_live.rs` (`draw_server`, `draw_choice`), `src/engine/save_slot.rs` (`ask_where_you_live`, `choose_on_server_gui`, `enter_pairing_gui`)
- Server: `src/relay/api.rs` (`get_server_info`), `src/relay/handlers/home_plots.rs` (`handle_home`)

### Dev Tab (the developer tools, 2026-10-20)
One top-menu category, Dev, holding the developer tools: Bugs, Testing, Performance, Dev,
Planet Tuner, (2026-10-22) Claude and (2026-10-24) Journal, the play journal (all permanent: perpetual development). Since
navigation rung 2 (2026-11-03) they are the Dev row of the top menu instead of a side list: the
Dev button opens the developer's tools, Bugs and Testing are pages of their own, the rest the Dev
page's parts; Report a problem (the Bugs page) is in the pinned help panel too, for everyone.
It replaced the Platform tab, which
the operator called redundant on 2026-10-20: Platform's Calculator, Notes, Calendar and Browser
are HumanityOS tools on the Apps page, its Recovery is in Settings > Account.
- Native: `src/gui/pages/dev_tab.rs` (`SECTIONS`, `GuiPage::Dev`, page id "dev"); tests in
  `dev_tab_tests.rs` (the Dev row lists the six and nothing else, no side list; the top menu says Dev, never Platform)
- Web: `web/pages/dev.html` (the Dev Workbench) carries the same list; the top menu's
  Dev tab opens it; `web/pages/platform.html` retired (removed from the live site by
  `scripts/sync-web-root.sh`)

### Dev > Claude: what Claude is doing (2026-10-22)
The Dev tab's Claude section, read-only: from this computer's own files, refreshed on background
threads only while it is on screen (files every 3 s, git every 10 s, the exe checks every minute,
GitHub every ten minutes), it shows every git worktree (the main checkout first, then the agents'
folders under `.claude/worktrees/`, newest commit first) with its branch, last commit subject and
time, uncommitted files and whether the agent holding it still runs; the agents' handoff notes
(`.claude/handoff/*.md` in the checkout and each worktree); the newest three journal decisions; the TIER 0 headings of PRIORITIES.md; Cargo.toml's version against the newest GitHub
release; whether the taskbar exe holds this tree's code (check-delivery's verdict, ported); the
share guard, the disk guard, the rigs' boot lock and the build slot (who compiles our crate now and
who waits for memory). A missing guard file reads as "not running",
never as an error. git runs with optional locks off, so the panel never holds an agent's index.
- Native: `src/gui/pages/dev_agents/` (`mod.rs` the panel and its jobs, `read.rs` the readers,
  `delivery.rs` the port of `scripts/check-delivery.js`, `tests.rs`); words in
  `data/i18n/en.json` under `dev_claude`; snapshot `just snapshot dev_claude`
- Web: the Claude tab of `web/pages/dev.html` (`web/pages/dev-claude.js`, tested by
  `scripts/tests/dev-claude-web.test.js`): GitHub's newest release and commit on main, TIER 0 from
  GitHub's copy, the journal the site serves; it names what only the desktop app can show

### Dev > Performance: each pie's legend a table beside its donut (2026-11-06, BUG-254)
Each pie's legend is a table measured first (the widest share, amount and name), each row reading
swatch, share, amount, name, so every number stands next to its own name whatever the names'
lengths; it sits beside its donut when the column has room, under it when not, and the page uses
two columns only when every pie fits beside its legend. At 2560x1440 the old legend right-aligned
the numbers against the whole column, where they read as the next pie's. Neighbouring wedges are
cut apart by a thin line, a legend row under the pointer lifts its wedge, and neighbouring rows of
a pie look different over the page (measured as the eye sees them: several theme tokens are near
twins, and orbit_line and constellation_line are too faint for a wedge).
- Native: `src/gui/widgets/pie.rs` (`Legend`, `donut`, the cuts in `paint_pie`),
  `data/performance/budget_systems.ron`; tests `src/gui/pages/performance_layout_tests.rs` (2560x1440
  and 1280x720) and `performance::tests::neighbouring_rows_differ_in_colour`; snapshots
  `performance`, `performance_wide`, `performance_narrow`

### Dev > Performance: the slow-frame record and one-off part timers (2026-10-31)
Beside each stage's average and its worst frame per 5 s window, the frame costs keep the last 32
slow frames: any frame whose work (`cpu.frame_total`) ran more than 4 ms over its running average,
with THAT frame's eight largest stages and spans (spans by their own CPU time where the platform
counts it). The worst-frame windows name each id's largest frame, which may be two different
frames for two ids; a spike is one frame, so its parts are what made it long. One-off jobs are
timed in parts so a spike names its cause: a home rebuild (`cpu.home_rebuild.*`, also a log line
"Homestead rebuilt: N rooms in X ms (...)"), a page screen's turns up to its first picture
(`cpu.screen_first.*`, and a `[Screens]` log line), and the save (`cpu.save.read`, `.extract`,
`.serialize`, `.disk`, `.write`). These are parts of stages already counted, never rows of their own.
- Native: `crates/humanity-core/src/frame_costs.rs` (`note_spike`, `spikes()`, `SPIKE_OVER_MS`, `SPIKES_KEPT`;
  `spikes` in `snapshot_json`, read by the dev MCP's `game_frame_costs`), the timers in
  `src/engine/home_meshes.rs` (`RebuildParts`), `src/gui/screen_surface.rs`
  (`note_first_turn`), `crates/humanity-systems/src/persistence.rs` and `src/save_load.rs`; test
  `a_slow_frame_is_kept_with_what_made_it_slow`; the survey that used them:
  `docs/design/sim-frame-cost.md`, "Survey 2"

### Dev Page: Spawn Any Creature + Walk-Up Editor
The Dev tab's Dev section (cheats-gated, like every dev affordance): searchable list of all 92
creatures.csv species, Spawn drops one ~2 m ahead (passive species keep the anchored
livestock graze - no AIBehavior; hunt species spawn as predators), Despawn-all + live
count. Walk-up editor: look at any creature, press G - rename, health (0 = kill via
the real Dead marker), Hostile toggle (maps to "predator", the behavior that actually
hunts the player), size, tint, despawn; the AI is only touched if the toggle is
actually flipped (opening the editor never rewrites predator/guard behaviors).
- Native: `src/gui/pages/dev.rs`, `src/lib.rs` (spawn/editor consumers),
  `crates/humanity-systems/src/livestock.rs` (`spawn_creature_at`)
