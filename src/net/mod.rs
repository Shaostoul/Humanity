//! Multiplayer networking: WebSocket client and ECS state synchronization.
//!
//! Native-only (requires tungstenite). The server relay handles message
//! routing; this module is the client side.

#[cfg(feature = "native")]
pub mod protocol;
#[cfg(feature = "native")]
pub mod client;
#[cfg(feature = "native")]
pub mod sync;
#[cfg(feature = "native")]
pub mod ws_client;
#[cfg(feature = "native")]
pub mod identity;
#[cfg(feature = "native")]
pub mod bip39_wordlist;
/// Post-quantum DM envelope (pure ML-KEM-768 → BLAKE3-KDF →
/// AES-256-GCM). Full-PQ replacement for the deleted ECDH `dm_crypto`;
/// the recipient keypair is deterministic from the BIP39 seed so web
/// and native derive the same key (kills the cross-client DM bug).
#[cfg(feature = "native")]
pub mod dm_pq;

/// Local encrypted DM history (sealed-sender cutover, 2026-08-23): the
/// relay's mailbox is a delivery window that expires, so long-term DM
/// history lives on-device, AES-GCM-encrypted under a seed-derived key.
#[cfg(feature = "native")]
pub mod dm_store;

/// "Who can reach me" (step B of docs/design/blocking-and-safe-mode.md, 2026-10-09): the
/// audiences per kind of contact, the relay's frames, the "show as a request" rule and the
/// contact request itself, with no socket and no GUI so each rule is unit tested.
#[cfg(feature = "native")]
pub mod reach;

/// Block (step C of docs/design/blocking-and-safe-mode.md, 2026-10-09): the identity's block
/// list, encrypted on this device, and the notes to oneself that sync it to its other devices.
#[cfg(feature = "native")]
pub mod block_list;

/// The choice for each friend as its own note to oneself (10n of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): `[[hum:choice:v1]]<friend key>/<may>`, so a person's devices agree on it.
#[cfg(feature = "native")]
pub mod choice;

/// Reports the admins can check (step D of docs/design/blocking-and-safe-mode.md, 2026-10-09):
/// the signed report and its evidence, the evidence picker's choice, and the admins' list.
#[cfg(feature = "native")]
pub mod report;

/// A report about a group reaches the group's creator (10j of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): the marker and its limits, the dialog's choices, and the creator's checks.
#[cfg(feature = "native")]
pub mod group_report;

/// Removing someone from a group I created (10j): a new group key for everyone else first, then
/// the signed remove, so nobody is removed yet still able to read.
#[cfg(feature = "native")]
pub mod group_remove;

/// Help outside this server, in the report dialog (step D follow-up, blocking-and-safe-mode.md
/// 10e-ii, 2026-10-10): the per-country emergency numbers and child-report lines, and which
/// country the block starts on.
#[cfg(feature = "native")]
pub mod outside_help;

/// Calls through the server (step E of docs/design/blocking-and-safe-mode.md, 2026-10-09): the
/// `call_credentials` request and reply, and the state the call UI shows while it is asked.
#[cfg(feature = "native")]
pub mod call_relay;

/// Warnings, and the recovery-phrase guard (step F of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): the warnings file, the matching rule both clients share, and the guard's test.
#[cfg(feature = "native")]
pub mod warnings;

/// The protected setup (step G of docs/design/blocking-and-safe-mode.md, 2026-10-10): the preset
/// (data/gui/safety_presets.json), the PIN's verifier and its wait, which actions need the PIN,
/// the review step's rows and the "Forgot the PIN?" phrase check, with no socket and no GUI.
#[cfg(feature = "native")]
pub mod protected;

/// Where the protected setup is kept: its own file beside config.json, read field by field and
/// written whole or not at all, so damage to config.json can never turn it off (2026-10-10).
#[cfg(feature = "native")]
pub mod protected_store;

/// The pace of background `dm_put`s (the friendship-pass sweep): a burst of six, then one a
/// second, under the server's limit so nothing sent in the background is refused (2026-10-10).
#[cfg(feature = "native")]
pub mod put_pacer;

/// Passes on their way: a `dm_put` that changes friendship state carries a `ref` and counts only
/// once the server answers `dm_put_ok` for it (section 10l, 2026-10-10).
#[cfg(feature = "native")]
pub mod put_answers;

/// One connection's own mailbox fetch (section 10o O1, 2026-10-10): a fresh ref on every
/// `dm_fetch`, and only pages carrying it count as this connection's.
#[cfg(feature = "native")]
pub mod mailbox_fetch;

/// An admin erases another person's data (section 10i of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): who is offered it, the typed-name check, the `admin_erase` frame and the receipt.
#[cfg(feature = "native")]
pub mod admin_erase;

/// Native client → relay v2 signed-object submission + invite ticket helpers
/// (P2P groups). HTTP via the same blocking-ureq pattern as image upload.
#[cfg(feature = "native")]
pub mod api_v2;
// Live Earth weather fetcher (NASA GIBS): needs ureq + the renderer, both
// native-only. Ungated it would break the relay CI build (see CLAUDE.md
// gotcha: the v0.381-v0.414 red-deploy incident).
#[cfg(feature = "native")]
pub mod live_weather;

/// P2P group end-to-end encrypted messaging (Phase 2). Epoch-key sealing via
/// the same ML-KEM-768 → BLAKE3-KDF → AES-256-GCM scheme as `dm_pq`, and
/// AES-256-GCM message ciphertext under that epoch key. Byte-compatible with
/// the web client's `pq-object.js` Phase-2 helpers.
#[cfg(feature = "native")]
pub mod group_e2ee;

/// Native WebRTC DataChannel P2P transport (increment 1). Sans-IO via the
/// `str0m` crate, driven from a blocking-UDP `std::thread` — the same
/// thread+mpsc model as `ws_client`, no async runtime. Opens an ordered data
/// channel to a peer via the relay's `webrtc_signal` and round-trips frames
/// off-server. Group mesh = inc-2, STUN/TURN = inc-3.
#[cfg(feature = "native")]
pub mod webrtc;

/// Native voice audio (v0.485): mic capture, Opus codec, speaker playback, and
/// (later phases) WebRTC audio media + the voice mesh. See voice.rs.
#[cfg(feature = "native")]
pub mod voice;

/// Live video publishing (v0.853): GPU frame -> downscale -> JPEG -> binary
/// WebSocket -> the relay's /live/pub fanout. This is what makes Studio's
/// "Go Live" actually leave the machine. See `docs/design/streaming.md`.
#[cfg(feature = "native")]
pub mod live;

/// Live video viewing (v0.857): the receive side. Connects to /ws/live/sub,
/// decodes the MJPEG frames, and hands the newest to the Watch page so streams
/// play inside the native app, not only on the web.
#[cfg(feature = "native")]
pub mod live_viewer;
