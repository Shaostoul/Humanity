# 2026-08-23: Sealed-sender DMs (the subpoena-proofing session)

## Why today

The operator saw the Take-Two/Discord news: a court order pulled registration
emails, phone numbers, IPs, device IDs, and linked accounts for ~100k Discord
users, most of them bystanders, while hunting the GTA 6 leaker. HumanityOS
already had no emails, no phones, no device IDs, and E2EE DM content, but an
honest audit against our own schema found the gap: the `direct_messages` table
stored `from_key, from_name, to_key, timestamp` in the clear, forever. Content
was sealed; the social graph was a CSV export away. A critic put it exactly
right: "if the server still has a readable inbox, a court order is just a CSV
export." For metadata, they were correct.

Nobody is using the platform yet (no compat debt), so the fix went in the same
day, whole, not as baby steps.

## What shipped

**Relay.** `direct_messages` is DROPPED by migration (secure_delete zeroes the
freed pages, WAL truncated). Its replacement `dm_mailbox` stores exactly
`(id, to_key, sealed_envelope, received_day)`: no sender column, day-granularity
arrival only. New protocol: `dm_put` / `dm_fetch` / `dm_purge` in,
`dm_new` / `dm_batch` / `dm_purged` out; the old
`dm/dm_open/dm_history/dm_list/dm_read` conversation machinery, server-side DM
search, and conversation-list endpoints are deleted because the relay can no
longer answer them, by construction. Mail expires after `dm_mailbox_ttl_days`
(server setting, default 30; boot sweep + 6h sweep) and any user can scrub
their own queue instantly. Push notifications went generic: no sender name
transits Google/Mozilla/Apple push infrastructure anymore.

**Envelope (v2).** The sender's identity now travels INSIDE the ciphertext:
inner payload `{v:2, from, to, ts, text, sig}` with a Dilithium3 signature over
`hum/dm/v2\nfrom\nto\nts\ntext`, sealed Kyber768 → BLAKE3-KDF → AES-256-GCM as
before. Clients verify the signature before trusting authorship, which upgrades
DM authenticity from relay-vouched to end-to-end: a spoofed sender fails
verification and never renders. One DM = two deposits (recipient copy + a self
copy into the sender's own mailbox so their other devices can fetch sent
history). Both clients dedupe by the inner-signature hash.

**Clients.** DM history now lives ONLY on the user's devices, encrypted under
seed-derived keys: native `src/net/dm_store.rs` (AES-GCM file, atomic writes,
per identity+server), web `chat-dm-store.js` (IndexedDB, every record body
AES-GCM sealed, which genuinely protects wrapped-key users). Sidebars, unread
dots, previews, and conversation views all render from the local store. The
native "send unencrypted anyway" modal is gone; there is no plaintext field
left in the protocol to downgrade to. New privacy control in both clients:
"Delete my server mailbox."

**Design call worth recording.** TTL-expiry was chosen over delete-on-ack
deliberately: ack-deletion breaks the same-identity-on-two-devices flow the
operator actually uses (whichever device fetches first would starve the other).
The TTL window preserves multi-device catch-up while still bounding what any
subpoena or breach can ever collect to N days of sender-less ciphertext.

## Verification

- v2 envelope tests: roundtrip both parties/any device, spoofed sender
  rejected, tampered fields rejected, v1 refused, dedupe-key stability.
- dm_store tests: encrypted persist/reload, wrong seed reads nothing, the
  plaintext never appears in the file bytes, unread tracks peer messages only.
- Mailbox storage tests incl. a schema guard that FAILS if anyone ever adds a
  sender-ish column, and a migration test that opens a legacy DB and proves
  the graph table is gone.
- 5 handler-level lifecycle tests through a real RelayState: sender-less
  storage + targeted delivery, non-envelope refusal, self-copy vs friend gate,
  unverified/bot rejection, fetch paging + purge scoping.
- Full lib suite green (1599 tests), native + relay feature checks clean,
  `node --check` on all six touched web files.

## Honest residuals (documented in retention_and_deletion_semantics.md)

Pre-cutover rotating backups still hold the old graph until they age out. The
relay necessarily sees which authenticated socket deposits mail in the moment
(abuse gates) and transport IPs; those are wiretap-class exposures a database
subpoena does not reach, and mitigating live traffic analysis is mixnet
territory, explicitly out of scope for now.
