# Hide and keep: a reported post out of view at once, a sealed copy for the hotline

**Written 20 November 2026. Status: rungs 1 to 5 built 20 November 2026 (see "Built" below): the
relay, and the screens in both apps with the owner's Export and Restore. Rung 6 waits for the
operator, and so does how the owner role is set.** The operator asked for it the same day
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

With rung 5 (2026-11-20) the policy's "What the app does not do for an operator yet" list lost its
"no button" item (it now says that setting the owner is not in the app yet), and
`web/pages/rules.html` gained a Hide and keep row in its table of moderation powers.

## Open questions for the operator

- **How the owner role is set.** Today only by hand in the database. Recommendation: a one-time
  "Claim ownership" code the relay prints to its log when no key holds the role (the first-admin
  `/claim` pattern), typed in the app; never an admin button, since the owner receives every held
  copy.

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

## Built (20 November 2026)

- **Who the owner is**: the one key holding the role `owner` (`Storage::owner_card`, src/relay/storage/held.rs). No path in the app grants it (a signed `grant_role` refuses it); with none, with two or more, or with no key card that checks out, keeping is refused with words and nothing is kept. Setting it is CLI debt (docs/design/in-app-ops.md) until the operator picks the mechanism (below).
- **Rung 1**: `hide_and_keep` in a report's decisions (src/relay/handlers/held.rs, reports.rs); the post, its pinned copy, its files from every served folder and their upload rows go; the Delete broadcast; the federated delete and the outbox scrubbed. "Delete the post" now federates its delete too. BUG-330 (the owner role was not an admin in `mod_action_refusal`) fixed with it.
- **Rung 2**: the package (`data/held/<random id>/`, part 0 the words and the file list, a part per file and poster picture), sealed by `crates/humanity-protocol/src/held_seal.rs` (ML-KEM-768, BLAKE3 `derive_key("hum/held/v1")`, AES-256-GCM in 1 MiB pieces; the "Held copies" row of cryptography.md) from the original a piece at a time into a staging folder, synced, put in place, then the rows in one transaction, then the originals deleted. A failed seal changes nothing. `held_items` keeps no readable content; the expiry pass also takes out a held file a crash left in a served folder (keyed name hashes). The folder name is the package id, not the report id.
- **Rung 3**: expiry a year after holding; a legal hold (after today, at most 366 days ahead, renewable, 0 lifts) defers it; destroy now; the report kept while its items are.
- **Rung 4**: each action a `mod_action_v1` (`hide_and_keep` on a `report`, `held_legal_hold`, `held_export`, `held_destroy`, `held_restore` on a `held` item; `expires_at` the hold's end) built and signed by the actor's app (core `moderation::HELD_ACTIONS`, its web twin in `web/shared/pq-object.js`), checked by src/relay/storage/moderation_held.rs (signature, signed by the signed-in key, this server's DID, five minutes fresh, never used before, admins and moderators for the hide, the owner alone for the rest) and recorded in `held_actions` with the signed object once carried out. Kept out of the public signed-object store (refused there, never gossiped): admins see the log through `held_list`; anyone sees counts at `GET /api/held-count`. An export grants the owner's device ten minutes to fetch the sealed parts (`GET /api/held/{id}/{part}?grant=<the export's object id>`), ciphertext only; `held_restore` is refused until rung 5 builds Restore.
- **Rung 5, the screens** (both apps; native `src/gui/pages/server_settings/reports.rs` and `held.rs` with `src/net/held.rs` and `src/engine/held.rs`, the website `web/chat/chat-held.js` with `web/shared/held-seal.js`, built as DOM with no innerHTML): on an open report about a post, **Hide and keep** beside the other decisions, sending the decider's own signed action; offered only when the Reports list says the server can seal a copy (`can_keep`), otherwise the relay's own sentence, then what works instead ("Delete the post") and what to do today while setting an owner is CLI debt. **Held copies** (Server Settings, Moderator, for admins; `/held` or the Reports view's button on the website): how many are held, each one's report, who held it and when, when it is deleted, any legal hold, its size and exports, the signed log and the counts; never contents, never the package id (sent only to the owner whose key it is sealed to, never drawn), no view button for anyone. For the **owner** alone, each with a reason that is signed into the log: Legal hold (days from today, at most 366, 0 lifts), Destroy now (asks first, says it cannot be undone), Export (a chooser that says what the file is for; the owner's device fetches the sealed parts under the export's ten-minute grant and opens them straight into one stored .zip, `held-item.json` without the restore key, `post.txt`, `files/<part>-<name>`, never drawn), Restore (asks first).
- **Restore, both halves**: the relay records a signed `held_restore` as a ten-minute grant; the owner's device opens each part and streams it back in pieces under nginx's 1 MB `/api/` limit (`PUT /api/held/{id}/{part}?grant=&offset=`, `x-held-restore-key`), then `POST /api/held/{id}/restore`. The relay never trusts the device for anything but bytes: at holding it sealed a restore plan (the manifest with each file's BLAKE3 hash) under a random key kept only inside the owner's part 0 (cryptography.md, "Held copies"), so the key opens the plan, every file must match it, and the words, names and folders come from the plan. Then the files go back to their own folders, their upload rows and the post's row in one transaction, every client is sent the post again (the broadcast filters by room), the held copy is deleted. A ban is lifted by the ordinary Unban.
- **The owner counts as an admin** everywhere the relay checks for one (msg_handlers.rs `is_admin_role`, 2026-11-20): marketplace listing edit and delete, review delete, project edit and delete, the board and the knock budget. BUG-330 had fixed `mod_action_refusal` alone.
- **Not yet**: rung 6 (telling the person) and setting the owner role in the app both wait for the operator; the screens say what to do today (docs/design/in-app-ops.md, "Setting the server's owner").

## Known limits, said plainly

- A file that sat in the public uploads folder was readable on the server's disk until it was held;
  deleting it does not scrub every trace from an SSD.
- Copies already fetched by other people's devices, and by linked servers this server does not
  control, are beyond reach.
- A held post's words stay in its report's evidence (links taken out), as before hide and keep: the
  report is what the admins decided on.
- A Media item whose file a held post linked keeps its row (its file is gone); the Media page's own
  report path handles items.
- Restoring a post restores it on this server only; linked servers that dropped their copy do not
  get it back.
