# Hide and keep: a reported post out of view at once, a sealed copy for the hotline

**Written 20 November 2026. Status: designed, not built.** The operator asked for it the same day
("build the hide and keep moderator action") and set its limits: "Makes sense to build the system
in. We just want to make sure the system itself can't be abused." This document plans the whole
feature so each rung built is a rung of the real thing. The law it answers to is in
[`docs/reference/findings/2026-11-20-csam-reporting.md`](../reference/findings/2026-11-20-csam-reporting.md);
what people are promised is [`docs/legal/content-policy.md`](../legal/content-policy.md), section
"What a server's operator does about a report".

## Why

In the United States a provider that reports child sexual abuse material to NCMEC's CyberTipline
must keep what it reported for one year, securely, with access limited (18 U.S.C. 2258A(h)), longer
while a government preservation request runs (2703(f)), and destroy it if law enforcement asks
(2258B(c)(2)). Today the app cannot do that: "Delete the post" on a report removes the post's row and
leaves its upload at its public address, and the ordinary Delete removes the files only that post
linked. Neither keeps anything; an operator must do it by hand on the server.

## The guarantees (the operator's seven requirements, 2026-11-20)

1. **Hide.** The post and every picture or file it links leave every view at once: the web and
   desktop chat (the relay's `Delete` broadcast), every route that serves uploads (the files leave
   the served folders), and the copies on linked servers this server controls (the federated delete
   `after_delete` already sends for an ordinary delete). Reversible: the owner can restore the post
   and lift the ban if a report proves false.
2. **Keep sealed.** Each file, and the post's own words, are sealed on the relay to the server
   OWNER's key card (their Kyber768 / ML-KEM-768 key, signed by their Dilithium3 / ML-DSA-65
   identity, checked by the relay as for every card; `docs/design/key-cards.md`). No new algorithm:
   the construction of the DM seal (Kyber768 / ML-KEM-768, BLAKE3-KDF, AES-256-GCM; the "DM E2EE" row
   of `docs/reference/cryptography.md`) with a context label of its own, recorded in that table in
   the commit that adds it. The plaintext is deleted once the sealed copy is written; the relay, other
   admins and anyone who breaks into the server hold only ciphertext, and only the owner's device can
   open it. **If the relay has no checked key card for its owner, keeping is refused**, and the admin
   screen says why in words ("This server's owner has no key card here, so a copy could not be kept
   sealed. Nothing was kept."): never a readable copy instead.
3. **No viewing.** No button opens a held item, for anyone, the owner included. The only way out is
   **Export**: the owner's own device unseals the package into a file saved to disk, for upload to
   NCMEC's provider form or for the police, never shown on screen. Every export is logged.
4. **One year, then gone.** Each held item carries its delete time, 365 days after it was held; the
   expiry pass deletes the sealed files and the row. A **legal hold** (the owner, with its own end
   date) keeps it past that while a preservation request runs; a **destroy now** (the owner) deletes
   it early when law enforcement asks. Admins see how many items are held and when each expires,
   never what they contain.
5. **Accountable.** Hide and keep, restore, export, legal hold and destroy now are each a SIGNED
   moderation action through the signed moderation log (`src/relay/storage/moderation.rs`,
   `docs/design/signed_moderation_logs.md`): signed by the key that took it, visible to every admin.
   A count of items held and exported, with no details, is offered for a public transparency line.
6. **Scope.** It keeps only what the server could already see: public and members-only posts and
   the files uploaded with them. It never reaches a sealed direct message or group message, and adds
   no path that does (a report's evidence already refuses any message carrying a file).
7. **Telling the person** whose post was hidden: open, not decided here (below).

## Data

A new table, `held_items` (a plain `CREATE TABLE IF NOT EXISTS` batch of its own; BUG-046 concerns
ALTER-added columns only): `id`, `report_id`, `author_key`, `stamp`, `channel`, `file_count`,
`sealed_bytes`, `held_by`, `held_at`, `delete_after`, `legal_hold_until` (0 when none),
`exported_count`. Nothing readable about the content: the post's words go into the sealed package
with the files. The sealed packages live in `data/held/<report id>/` beside the uploads folder,
which nothing serves. The report that held them is kept as long as they are (storage/reports.rs's
90-day cull and `/reports-clear` skip a report with held items). The files' `user_uploads` rows go,
so no list shows them and no limit or sweep deletes them early.

## Rungs

| Rung | What | Proof (each seen red first) |
|---|---|---|
| 0 | The finding, the policy, this plan (2026-11-20) | the policy's links in both apps and on the site |
| 1 | Relay: the `hide_and_keep` decision for admins and moderators; the post's row, its files and their upload rows out of every view, the federated delete; refused, with the words above, when the owner has no checked key card | relay tests: the post gone, a `Delete` broadcast, the files no longer in any served folder nor answered by `/uploads/<name>`, a `fed_delete` queued, refusal without a card |
| 2 | Relay: the seal to the owner's card, the plaintext deleted, `held_items` written; cryptography.md row | a held file's bytes appear nowhere on disk in the clear; the package opens with the owner's test seed and with no other |
| 3 | Expiry and the legal hold: deletion at 365 days, a legal hold that defers it, destroy now; the report kept while its items are | the expiry pass at day 364 keeps, at 366 deletes; a hold past 366 keeps until its end |
| 4 | Signed actions for hide and keep, restore, export, legal hold and destroy now, in the signed moderation log; the transparency count | each action leaves a signed record naming its key; an unsigned request is refused |
| 5 | Both apps: "Hide and keep" on the Reports page (words and hint by key); a Held items list (count, each delete time and legal hold, no contents, no view button); owner-only Export, Legal hold, Destroy now and Restore, each done on the owner's device | a headless click on each control (desktop), the drawn panel and the frame sent (website) |
| 6 | Whatever the operator decides about telling the person | as decided |

When rung 5 ships, the policy's "What the app does not do for an operator yet" list loses its first
item, and `web/pages/rules.html` gains a row in its table of moderation powers.

## Open questions for the operator

- **Telling the person.** No official source read settles it (finding 12): nothing in 2258A forbids
  or requires it; a court can order silence about legal process (2705(b)); NCMEC's reporting API
  records whether the user was told. Recommendation: say nothing beyond what a ban already says, and
  never that a report went to the hotline; record in the CyberTipline report whether they were told;
  ask NCMEC (the question is in `docs/legal/ncmec-esp-question-draft.md`).
- **Plaintext on the owner's device.** Export and Restore need the owner's device to unseal the
  package, so the plaintext exists there for the moment it is written to a file (Export) or uploaded
  again (Restore), never on screen. Recommendation: accept; it is the only way NCMEC or the police can
  receive it.
- **No owner key card.** Recommendation: refuse keeping and offer only the existing "Delete the
  post", saying that nothing was kept, so a server without a card never holds a readable copy.

## Known limits, said plainly

- A file that sat in the public uploads folder was readable on the server's disk until it was held;
  deleting it does not scrub every trace from an SSD.
- Copies already fetched by other people's devices, and by linked servers this server does not
  control, are beyond reach.
- The existing "Delete the post" decision still does not federate its delete (it broadcasts to this
  server's clients only); rung 1 should fix it alongside.
