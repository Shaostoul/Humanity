# Tiered signing: release signing and governance re-vote signing

Research record. The operator asked whether signing could be tiered and what
the pros and cons of each option are. Two signing questions were live at once,
release signing and who signs an automatically-opened re-vote, and both are the
same shape of question: who holds what authority, and what does each level buy.

A 14-agent read-only pass produced this: three grounding passes over what the
repo actually enforces, four on prior art (TUF, Sigstore, Android key rotation,
Authenticode, Debian, npm provenance, sunset clauses, ISO systematic review,
DAO governance), three independent option sets, an adversarial critique of each,
and one synthesis. Every load-bearing repo claim was re-verified against the
tree before this was written.

Two findings in the Question B section were live defects in v0.1303.0 and were
FIXED in v0.1304.0: the standing answer flipped to a successor before it closed
or passed, and the supersedes pointer was stored unverified so any keypair could
hijack a chain. The text below is preserved as written, including its
description of those defects, because the reasoning is what makes the remaining
options make sense.

---
# Can signing be tiered? Yes to both, but they need different tiers, and only one has a deadline.

I verified every load-bearing claim below against the tree myself. Where three research passes disagreed, I read the code and say who was right. Claims about outside systems (TUF, Sigstore, YubiKey firmware) I did not re-verify and mark unverified.

---

# QUESTION A: RELEASE SIGNING

## The situation, exactly

Verified live: v0.1294.1 is at position 13 of the newest 26 releases, and the 12 releases above it are all non-prerelease and unsigned. The client fetches one page of 20 (`src/updater.rs:388`), applies the prerelease filter **after** the fetch (`src/updater.rs:296-302`), and maps "nothing eligible" to `UpToDate` (`src/updater.rs:314-322`), which looks identical to "you are current."

So: **7 more publishes and v0.1294.1 is still inside the window. The 8th publish makes auto-update silently offer nothing.** Also worth saying plainly: even today, desktop users are being handed a build that is 12 releases old, and nothing tells them so.

## Five facts that decide which tiers are honest

These are not opinions. Each changes at least one option.

**A-F1. Your signature attests to bytes GitHub served, not bytes you built.** `just sign-release` runs `gh release download` into a temp dir (`Justfile:566-570`), then `scripts/make-release-manifest.js:35-45` hashes those downloaded files and you sign that. You never build the artifacts you sign. So the property the signature carries is "a human decided to bless this tag," not "these bytes came from my source." That is still real (an attacker owning GitHub cannot mint a signature), but `docs/admin/release-signing.md:15-18` and `src/release_update.rs:11-18` describe something stronger than what ships. Everything below inherits this.

**A-F2. The root key is already used unattended.** `scripts/archive-build.js:143-157` runs `--sign-file` on every `just build-game` whenever `HUMANITY_SIGNING_PASSPHRASE` is in the environment. So "the key only signs when a human types a passphrase" is already false, and any option whose stated cost is "the passphrase becomes warm" is pricing a cost you already pay.

**A-F3. The cause of 12-of-12 is a 40 minute gap, not release volume.** `just release` publishes immediately (`Justfile:729-745`). `just sign-release` refuses until CI has uploaded binaries and says so in its own error text, quoting 35 to 40 minutes (`Justfile:564`). Signing is structurally impossible at the moment of release. It requires a human to come back to a shell later. That is the whole disease. Also, correcting a claim that appeared in two of the three research passes: `just build-game` publishes nothing (`Justfile:483-486`), so all 12 unsigned releases are deliberate `gh release create` calls, not stray build stamps.

**A-F4. A format change to the key file fails OPEN, not closed.** `SigningPubkeys` derives `Default` and `embedded_pubkeys()` uses `unwrap_or_default()` (`src/release_update.rs:54, 72-75`). If `data/release/signing_pubkeys.json` becomes a list, an older client's parse fails, both fields go empty, `is_provisioned()` goes false, and the client takes the legacy branch at `src/updater.rs:622-631` and **installs without verifying anything.** Any tiering change must rename the file as well as change its shape, so old clients hard-fail instead of quietly disabling their own gate.

**A-F5. The watchdog samples a smaller window than the client.** `scripts/check-release-signing.js:24` defaults to 12 and `scripts/brief.js:73` calls it with 12, while the client sees 20. At limit 12 every row is unsigned, so it takes the `!offered` branch at `:66` and prints "NO SIGNED RELEASE EXISTS. Desktop auto-update offers nothing at all" plus exit 1, which is false: auto-update is live on v0.1294.1. Separately `unsignedCount` at `:39` counts the whole window rather than only rows newer than the offered release.

## The tiers that survive

### A0. Widen the window and fix the alarm (do today, not really a tier)

**How:** one number at `src/updater.rs:388` (`per_page=20` to `100`, which is GitHub's documented maximum). In `scripts/check-release-signing.js`: raise the default limit to at least the client's, scope `unsignedCount` to rows newer than `offered` (line 39), and add a middle rung that goes loud when the newest signed release is within a few positions of falling off the client's window.

**Setup cost:** one line plus about 20 lines of script. Under an hour.
**Per release:** zero.
**What it stops protecting against:** nothing, no gate changes. But be honest about what it buys: it converts "auto-update dies silently" into "users install a build that is weeks old and nobody is told." That is a support cost, not a fix.

**One catch nobody flagged:** `per_page` is compiled into the client, so raising it only helps installs that take a release containing the change, and that release has to be signed to be offered. So sign the current release by hand in the same sitting. That single command is the only thing on this whole page that reconnects users this week.

### A1. Make an unsigned release invisible by construction (draft-then-sign-then-publish)

**How:** add `--draft` to `gh release create` at `Justfile:743`, and append `gh release edit {{version}} --draft=false` to the end of `sign-release` (`Justfile:574`). Drafts are already excluded by the offer filter (`src/updater.rs:297`) and by GitHub's Latest pointer.

**Setup cost:** two lines. **Per release:** zero.
**What it stops protecting against:** nothing cryptographic. It does mean a release is not publicly downloadable until you sign it, which is a change in how the website behaves during those 40 minutes.

**Verify before building on it:** `.github/workflows/build-desktop.yml:119-121` uploads with `softprops/action-gh-release@v2` and `draft: false`. Whether that flips an existing draft to published is **unverified** and is the whole hinge. Test it on a throwaway tag first.

**This replaces the "mark routine tags prerelease" idea** that two of the three research passes recommended. That idea is dead, and it is worth knowing why: prereleases are fetched and then filtered (`src/updater.rs:296-302`), so they consume the client's 20 slots at exactly the same rate. It buys **zero** headroom. Worse, `check-release-signing.js:37` computes `latest` as the newest non-prerelease, so once routine tags are prereleases the watchdog would print "Latest is signed, auto-update is live and current" while the cliff kept approaching. It silences the only instrument you have.

### A2. Close the 40 minute gap with a local deferred signer

**How:** a small watcher on your machine that notices when CI has uploaded binaries for a tag and runs the existing signing flow with no human present. The passphrase comes from the OS keychain rather than an exported env var. `keyring` is already a dependency (`Cargo.toml:84`) and the store/load pattern already ships in `src/auto_unlock.rs:117-167`, though note that path stores exactly 32 bytes, so you would be storing a derived key or the vault blob, not literally reusing it.

**This is the option that actually fixes A-F3**, and it needs one guard the research passes got wrong.

The guard they proposed was "refuse any tag that is not in the local git repository." That does not work: a routine `git fetch` imports an attacker's pushed tag, and `build-desktop.yml:3-6` already fires on any `v*` tag. So a GitHub-write compromise pushes a tag, CI builds and publishes, your next fetch imports the tag, and your watcher signs it. That is the exact threat the scheme exists for.

**The guard that does work:** an append-only intent file written by `just release` before the push, recording the tag name and the exact commit SHA. The watcher signs only tags present in that file at that SHA. This also blocks a class you have already hit twice: CLAUDE.md records stray tags v0.415.0 and v0.421.1 that auto-published releases. Unattended signing without the intent file would have turned those accidents into shipped code.

**Setup cost:** roughly a day (watcher, keychain, scheduled task, intent file written from `Justfile:743`).
**Per release:** zero.
**What it stops protecting against:** the human timing check. A-F1 means your signature is currently the only thing standing between a poisoned CI build and your users, and this removes the human from that moment. It also means the key can be exercised by a process running as you, on a machine where CLAUDE.md documents two or three AI sessions with shell access working concurrently in this directory. That is a materially larger surface than "a human typed a passphrase for thirty seconds."

**The salvage that makes it much better, and it is cheap:** `just build-game` already compiles and archives a local Windows exe. Upload **that** as the Windows release asset instead of CI's, and have the watcher refuse to sign if the manifest's Windows hash does not match the local archive. Then a CI-forged Windows binary cannot be signed even by an automated signer. macOS and Linux stay CI-trusted because you cannot build them, and that limit should be written into `docs/admin/release-signing.md` rather than left implied.

### A3. Same watcher, but it asks before signing

**How:** the watcher stages the manifest and surfaces a card in the app with a Sign button. The click releases the passphrase.

**Setup cost:** half a day on top of A2. **Per release:** one click, at a time of your choosing, where you already are.
**What it stops protecting against:** less than A2 does, which is the point. But an in-app click is not proof a human is present on a machine that runs headless egui click tests as routine tooling. Gate it on an OS keychain prompt with user presence so the platform does the asking, not your code.

This also closes a real gap: grep of `src/gui` for `release_update` returns nothing, so release signing is CLI-only, and it is not listed as tracked CLI debt in `docs/design/in-app-ops.md`. That is an unlogged exception to your own GUI-first rule.

### A4. The actual tiering: a cold root that authorizes, a warm delegate that signs

**How:** the embedded pair becomes a **root** whose only job is signing a small delegation document (delegate's two public keys, a capability string, a `not_after`, a version counter). A second pair, the **delegate**, signs manifests. The client verifies root over the delegation, checks the window, checks it names the key that signed the manifest, then runs the existing both-signatures check.

Files: `src/release_update.rs:54-63` (key struct becomes a roster with ids), `:95-103` (signature gains a signer id), `:141-195` (verify becomes resolve-then-check-then-policy), `:44-46` (`HYBRID_ALG` bumps, which it is documented to do for exactly this), `data/release/signing_pubkeys.json` **renamed** per A-F4, `scripts/verify-release-signature.mjs:24,39,53` in the same commit or the independent third-party check silently stops matching. Roughly 150 to 250 lines plus tests.

**Setup cost:** a few days. **Per release:** unchanged.
**What it stops protecting against:** the trusted set stops being one key you hold. A stolen delegate is full remote code execution on every desktop until it expires, so the window length is the entire security parameter.

**Why it is still worth it:** there is no revocation anywhere today. Grep for `revoke|expir|not_after|threshold|keyid` across `src/release_update.rs`, `src/updater.rs` and the Node verifier returns nothing. The documented rotation procedure (`docs/admin/release-signing.md:89-91`) is "ship a build signed by the old key," which is useless when the old key is the stolen one. Expiry is the only revocation a solo operator reliably gets, because it needs no action. And it is what makes A2 defensible rather than merely convenient.

**Two honest costs the research passes underplayed.** First, this **creates new recurring toil** on a project whose disease is recurring toil, and when a renewal is missed the delegate stops verifying and auto-update dies again. Set the window long (a year, not 90 days) and treat expiry as damage-bounding, not as revocation. Second, expiry depends on the client's clock, which nothing validates.

**Do it before launch or not at all for a long time.** The trusted set is `include_str!`-compiled (`src/release_update.rs:52`), so after launch every trusted-set change must ship inside a release signed by the key clients already have.

### A5. Roster with threshold 1 now, threshold 2 when a second human exists

**How:** the same edit as A4, one more field. This is format-level multi-signature (count distinct valid signers), not threshold cryptography.

**Setup:** small increment on A4. **Per release:** unchanged at threshold 1.
**What it stops protecting against:** at threshold 1 the trust surface becomes the union of everyone on the roster. Any single listed key can ship code to every user. That is the correct trade for survival and the wrong trade for adversarial safety, and it should be a deliberate choice.

Do **not** set the threshold above 1 today. With one human it makes every release block on a person who does not exist.

Threshold cryptography (FROST and friends) is not on the table: there is no shippable threshold ML-DSA, and since your verifier requires both halves, covering only Ed25519 buys nothing (unverified as to the current state of the standards, but the conclusion holds either way because format-level multi-signature needs no threshold crypto).

### A6. Escrow the vault (do this first, it costs nothing)

`docs/BUS-FACTOR.md:122-128` hands a successor SSH, DNS, repo-owner access and a secrets vault, and never mentions the release signing key or its passphrase. So a successor with everything on that checklist **still cannot ship an update any installed client will accept.** Put `release-signing-key.enc` and its passphrase in the secrets store that checklist already tells you to designate, and add the line to the checklist.

**Setup:** an hour, no code. **Per release:** zero.
**What it stops protecting against:** sole custody. Whoever holds the escrow can sign as you. That is the entire point and should be stated in the document.

This is the real bus-factor fix. Shamir-splitting the root (which one research pass proposed, tying it to the unbuilt guardian-recovery feature) is a better version of the same idea later, but it depends on a Shamir implementation that does not exist in the tree and on a rehearsal drill that becomes another permanent recurring obligation. Escrow first.

### A7. Hardware token, root only, two tokens, later

Putting the Ed25519 half on a token makes theft of the software vault insufficient, because verification is an AND (`src/release_update.rs:147-192`). But it is anti-automation by design: a token requires a touch, so it cannot be combined with A2 on the same key, and under A4 it gates every delegation renewal. Lose the single token and once the current delegation expires nobody can ever ship again, which is worse than today. Two tokens plus an offline root backup, or skip it. (YubiKey Ed25519 firmware floor: unverified.)

### A8. Publish an append-only record of every manifest you ever signed

Commit each signed manifest and signature into the repo, which already mirrors to Forgejo over SSH. This is the one control that detects an A-F1 class forgery at all: it turns a stolen key from silent forgery into forgery that leaves a record on independent infrastructure. Cheap, additive, relaxes nothing.

## Dropped from Question A, and why

- **Mark routine tags prerelease.** Buys zero fetch headroom (prereleases consume the same slots) and silences the watchdog. Replaced by A1 (draft).
- **Sign in CI, keyed or Sigstore keyless.** Inverts the stated threat model outright (`docs/admin/release-signing.md:15-18`), drops the post-quantum half, and adds a third-party runtime dependency to a self-hosting project. The one salvageable idea is the transparency log, which is A8.
- **Threshold cryptography (FROST / threshold ML-DSA).** No shippable PQ half; the AND verifier makes a half-solution worthless.
- **Unattended signing without an intent file.** Fails to the exact attack the scheme exists for. See A2.
- **"Small headroom problem, just bump per_page and move on."** A0 alone leaves users on a stale build with no signal.

## Recommendation for A

**This week:** A0 plus sign the current release by hand. Then A6, which is an hour with no code and is the only thing that fixes survivability. Then A1, once you have tested how the workflow action treats an existing draft.

**Next:** A2 with the intent file and the sign-what-you-built salvage, shipped with A3's confirm as the default and unattended as an opt-in setting.

**Before launch:** A4 with the file renamed, the window set to a year, and A5's roster field defaulting to threshold 1. Then A8 when convenient.

**The strongest case against my own recommendation:** A4 is a real security regression dressed as an upgrade. It puts a second, warmer key in the trusted set, it depends on client clocks nobody validates, and it swaps "sign every release" for "renew every year" on a project whose demonstrated failure mode is exactly forgetting a recurring step, with the same silent consequence when it lapses. Given A-F1, the whole scheme's protection against a CI compromise is already thinner than advertised, so one could argue the honest move is A0 plus A1 plus A6 plus A8 and stop: fix the cliff, make unsigned releases invisible, make the key survivable, make forgery detectable, and leave the trust format alone until there is a second maintainer to justify a roster. I do not think that is right, mostly because there is no installed base to protect right now and there never will be a cheaper moment, but it is the argument I would want you to weigh.

---

# QUESTION B: GOVERNANCE RE-VOTE SIGNING

## The situation, and a correction that changes the answer

**B-F1. There is a bug in shipped step 2, and it invalidates the safety argument every research pass leaned on.**

`docs/PRIORITIES.md:3086` specifies "the standing answer is the most recent **closed** decision in the chain." What shipped is `standing_decision` (`src/relay/storage/governance.rs:172-203`), which walks `WHERE supersedes = ? ORDER BY created_at DESC LIMIT 1` and returns the newest row. No `closes_at` check, no `opens_at` check, no tally, no quorum. `GET /api/v2/proposals/{id}/chain` returns `chain.last()` as `"standing"` (`src/relay/api_v2_governance.rs:108`).

So **the standing answer flips to a successor the moment the successor is indexed**, before it opens, before anyone votes. The reassuring claim that "an unratified re-vote nobody votes in fails quorum, so the old decision keeps standing" is false against the code. It is false for every option equally, so it does not favor any of them, and it means every automatic-successor design is unsafe until this is fixed.

Also verified: `standing_decision` has no production caller other than `decision_chain`. The function the spec describes is dead code; the function that ships has no spec.

**B-F2. Anyone on the internet can move any standing decision, today.** `index_proposal` (`src/relay/storage/governance.rs:94-152`) performs no author check of any kind, and stores `supersedes` unverified with a comment saying so (`:126-129`). `POST /api/v2/objects` needs no account, only a valid self-signature and 30 objects per 60 seconds (`src/relay/api_v2_objects.rs:196-197`). Combined with B-F1, any free keypair can make `/chain` report its own proposal as the standing answer on any question. This is a live defect in v0.1303.0, not a prerequisite for a future feature.

**B-F3. Quorum counts raw rows against a shrinking electorate.** `index_vote` has no membership check at all, quorum uses `vote_count` (`src/relay/api_v2_governance.rs:180-186`), and the electorate is `get_member_count(None)`, which filters on directory-visible members (`src/relay/storage/members.rs:25-26`) while the default privacy tier sets `directory_unlisted: true` (`data/gui/privacy_tiers.json:12`). So N throwaway keys with zero trust weight force quorum while contributing nothing to yes/no weight, and a privacy setting is a governance lever: going private shrinks the denominator. Any petition or endorsement threshold built on this primitive is theatre until it is fixed.

**B-F4. Neither client verifies anything it fetches.** The only `verify_signature` calls in `src/gui/pages/governance.rs` are in tests. So a hostile relay can already fabricate proposals, fabricate votes and report any tally. This is context for how much a server signing key really adds, and it is also an argument for making relay forgery detectable rather than for adding a legitimate forgery path.

## The options that survive

### B0. Two rules, and they are prerequisites for everything else

**Rule 1: authorize the `supersedes` pointer.** A successor may only take over a chain if its author is entitled to (original proposer, a valid pre-signature, a petition set, or a constrained scheduler). Anything else is stored as an ordinary proposal and does not link.

**Rule 2: "standing" means the newest link that has closed and passed.** This is what `docs/PRIORITIES.md:3086` already says the design is. Change `standing_decision` (`src/relay/storage/governance.rs:172-203`) to consult `closes_at` and the tally, and make `decision_chain`'s last-element convention match, or have the API compute standing separately (`src/relay/api_v2_governance.rs:108`). Also make `opens_at` a real gate: it is currently honored at exactly one line, the list filter at `:445`.

**Setup:** medium, mostly in `governance.rs` plus tests. **Per release:** zero.
**What it stops protecting against:** nothing. It closes a hole.

Rule 2 is what makes every automatic-opening option below safe, because it means opening a successor no longer silently blanks the current answer.

### B1. The proposer signs the successor at proposal time

**How:** when someone sets a review cadence, their client builds and signs two objects: the proposal, and a post-dated successor carrying `supersedes` and `opens_at`. The relay stores it and starts serving it when the clock arrives. The scheduler holds no key at all.

**Setup:** the payload fields do not exist in either client yet (`src/gui/pages/governance.rs:246-258` and `web/shared/pq-object.js:348-369` both emit six fields), so it is two client builders plus a cross-language KAT plus the storage work. Medium, and larger than the research passes priced it, because of the next paragraph.
**Per release:** zero, forever.
**What it stops protecting against:** the ability to withdraw. A pre-signed successor cannot be recalled if the proposer changes their mind, rotates keys, or leaves. And the wording is frozen a year in advance, which is arguably the reason people want a re-vote in the first place.

**Two things that must be fixed first or it backfires.** Without B0 rule 2, the stored successor becomes the reported standing answer immediately, potentially a year early. And `proposals_due_for_review`'s `NOT EXISTS` clause (`:277-280`), which the research passes cite as a benefit, would see the successor and **permanently stop the parent from ever being due**, disabling the exact mechanism this implements.

**One real limit:** this covers **one** review hop. `review_after` is derived per proposal from its own cadence (`:120-124`), so a perpetual cadence needs an endless chain of pre-signatures. B1 automates the first review and then you are back where you started. That is fine if you say so out loud; it is not fine if it is sold as solving recurring reviews.

**Authorship story:** the best available and unconditionally true. The human signed that exact text, and the signature a verifier checks is theirs.

### B2. Fixed-form reaffirmation, as a constraint on whatever else you pick

**How:** a scheduled review does not open free text. It re-asks the same question with a fixed answer set, derived deterministically from the parent so any peer can recompute it.

**Setup:** small once B1 exists. Mostly schema and UI.
**Per release:** zero.
**What it stops protecting against:** the ability to amend at review time. If the world changed, you need a fresh proposal, which is the mechanism you already shipped.

**Why it matters more than it looks:** if there is no new text, there is no authorship to launder, which is what makes even a machine-opened successor defensible. This is how ISO systematic review works (a secretariat opens a confirm/revise/withdraw ballot and holds no vote in it) and how New York reopens its convention question every twenty years with no author at all, because the text is fixed in the constitution.

**One gap:** `data/governance/proposal_types.ron` has no reaffirmation type, and a reaffirmation inheriting `accord_amendment` rules (quorum 0.25, pass 0.75) will often fail quorum in a small community. Under today's code that failed reaffirmation still becomes the standing answer, which inverts the intent. Another reason B0 rule 2 comes first.

### B3. A due-list endpoint, unsigned, computed

**How:** `proposals_due_for_review` (`:271-286`) already computes exactly the right list and has no production caller. Expose it as a plain REST response plus a badge in the app. One click opens the successor with the viewing human's own key.

**Setup:** small server-side, plus real UI on two clients (neither client reads chains at all today, and `web` has zero `supersedes` handling, so this is more than a button).
**Per release:** one human confirmation per review.
**What it stops protecting against:** nothing, and that is its whole appeal. Zero new authority, zero new keys, zero new object types.

**This replaces the "server signs a review-due notice object" idea.** If the relay can sign a `review_notice_v1`, it can sign a `proposal_v1`; nothing in the object envelope expresses "only the server may sign this kind" or "this kind cannot be voted on," and `put_signed_object` validates kinds for exactly two types, neither of them governance (`src/relay/storage/signed_objects.rs:119-178`). So the notice object grants the same authority as B4 while promising not to use it. A computed list grants none and any peer can recompute it.

**The honest cost:** this is the same shape as the failure in Question A. A step that needs a specific human present is a step that stops happening. The IETF deleted its own two-year standards review in RFC 6410 after fifteen years of nobody performing it (their account, unverified by me). Twelve unsigned releases is the local version.

### B4. A dedicated scheduler key on the relay, hard-constrained

**How:** the relay already holds a Dilithium3 keypair derived exactly like a user identity (`src/relay/storage/misc.rs:511-529`), and signing a `proposal_v1` with it is about a dozen lines. If you do it, all of these are mandatory:

1. A **separate** scheduler key in its own `server_state` row, not the federation key, so a federation leak is not also governance forgery.
2. Fixed-form successors only (B2), derived deterministically, so the machine has no discretion over wording.
3. `origin: scheduled_review` and `on_behalf_of` stored as **columns** and rendered as labels. Today `index_proposal` records only `proposer_did` (`:96`) and clients render lists from that, so without this the record reads as "someone proposed this."
4. An explicit rule that this DID cannot vote, **with a test**. It cannot be a class check: `is_ai_agent` reads a self-asserted `subject_class` (`src/relay/storage/ai_status.rs:86-108, 175-179`) and `institution` is a valid class that is not excluded. It has to be a hardcoded DID comparison.

**Setup:** small for the code, medium once the four guards, the tests and a key-custody plan are counted.
**Per release:** zero.
**What it stops protecting against:** the seed sits as plaintext hex in a plain SQLite column (`src/relay/storage/misc.rs:514-523`) in a database snapshotted into backups every 30 minutes and 6 hours. A single database read becomes permanent agenda-setting power with no expiry and no rotation procedure. It also merges blast radius: today a leak means impersonating the relay in chat; this adds forging the schedule keeper.

**Authorship story:** honest only with guards 2 and 3. With them, the record reads "opened automatically under the review clause of proposal X, on behalf of its signers," and the machine holds no vote. Without them it is exactly the laundering you are worried about. Across legislation, standards bodies, constitutions and DAOs, none of the research found a precedent for an administrator signing substantive proposals as if it were a member; what precedent exists is for opening a fixed-form question and holding no vote in it.

### B5. Petition threshold, after B-F3 is fixed

**How:** N signed endorsements open the successor; the signature set is the authorship. Already step 5 of the backlog. Pairs naturally with B3, where the review date arms a lower threshold rather than just printing a reminder.

**Setup:** medium, and the denominator and eligibility work in B-F3 is most of it.
**Per release:** zero.
**What it stops protecting against:** nothing conceptually, but as specified today it is forgeable: keys are free and quorum counts rows, so "N signatures" is one person with a script. It needs either trust weighting (already used for vote weight at `:315-320`) or a real membership notion in the vote path, which does not exist anywhere.

### B6. Lapse by default, opt-in per proposal type

**How:** rather than automating who opens a review, automate the consequence of not holding one. A per-type flag in `data/governance/proposal_types.ron` makes an unreviewed decision stop standing at its review date.

**Setup:** the flag is small; the safety work is not. It needs a distinct **lapsed** state (otherwise "expired" and "never existed" look identical to a client, which is terrible for an audit trail), advance notice in the UI, and a decision about federated peers who never saw the reaffirmation.
**Per release:** zero.
**What it stops protecting against:** a lot. A decision can quietly stop standing when nobody is around, so operator absence changes policy instead of leaving it unchanged. And a 0.25-quorum failure repealing a 0.75-supermajority decision is a bad outcome. If you build it, make lapse revert to the **previous chain link**, never to nothing.

**Why keep it on the table anyway:** it is the only mechanism here that makes reviews actually happen without any automatic authorship, because inaction has a cost. Sunset regimes and W3C charter end dates work for exactly this reason.

## Dropped from Question B, and why

- **The relay signs as the original proposer.** Not possible (the relay holds no user key) and should not be built if it were. It attaches a human's signature to text they never saw. An audit trail whose whole value is authenticity cannot contain forged attributions.
- **Server-signed `review_notice_v1` object.** Grants the same object-signing authority as B4 while claiming to stay "on the calendar." Replaced by B3, a computed endpoint, which grants none.
- **A `hum/revote/v1` delegation certificate naming a delegate.** Good primitive, wrong time. Pre-launch the only available delegates are you and the server, so the field is ceremonial, and if the delegate is the server it is B4 with extra paperwork plus a laundering surface. It also imports the friendship-certificate pattern's known-deferred gap (no expiry or revocation) into a governance audit trail on day one. Revisit if a standing council ever exists.
- **"Quorum protects the standing decision from an unratified successor."** False against shipped code (B-F1). Do not let this sentence get quoted into a design doc.

## Recommendation for B

**First, and unconditionally: B0.** Both rules. Rule 2 is a bug fix against your own written spec, and rule 1 closes a hole any keypair on the internet can walk through today. Nothing else in this section is safe until both land.

**Then B3** (computed due list plus UI), which ships soonest and adds no authority at all. **Then B1 plus B2** as the default for scheduled reaffirmations, once `opens_at` is a real gate. **Then B5** once B-F3's eligibility and denominator problems are fixed. **B6 opt-in per type, with revert-to-previous semantics,** if you want reviews that actually bite. **B4 only if B1 plus B3 prove insufficient in practice, and only with all four guards.**

**The strongest case against my own recommendation:** B3 is the same design that produced twelve unsigned releases, and B1 only covers one review hop, so the combination I am recommending may automate the first year and then quietly stop, which is the failure mode you are explicitly trying to avoid. If your real goal is "once a year we revisit certain things, and it happens whether or not I am at a keyboard," then the constrained B4 is the only option in this list that delivers it indefinitely, and the case against B4 is mostly about a plaintext key in SQLite (fixable) and a laundering risk that B2's fixed-form constraint largely removes. A reasonable person could put B4 ahead of B1 and be right, and I would not argue hard against it once guards 2 and 4 are in place with tests.

---

# Things that are your call, not an engineering answer

1. **Does a release signature mean "I built this" or "GitHub served me this"?** Right now it means the second (A-F1). Deciding this determines whether the sign-what-you-built salvage in A2 is mandatory or optional, and it needs to be written into `docs/admin/release-signing.md` either way, because the current text claims more than the code does.
2. **How much of the trust surface are you willing to widen for continuity?** Escrow (A6), a roster (A5) and a successor delegation all trade "only I can ship" for "the project survives me." `docs/PRIORITIES.md` says the project is a means, not an end, which points one way, but it is your call.
3. **Stand by default or lapse by default (B6)?** This is a values question about what an unreviewed decision means. Nothing in the code decides it for you.
4. **Does the machine get to open questions at all (B4)?** Even fully constrained, this is a statement about what a relay is allowed to do in a system whose whole premise is that authority comes from consenting humans.

# Two loose ends worth fixing while you are in here

- `docs/ai/onboarding.md:383-384` tells AI participants they may vote if the server grants voting rights. There is no grant mechanism anywhere: `is_ai_agent` is unconditional and `index_vote` silently drops the row (`src/relay/storage/governance.rs:295-302`). The doc promises a capability that exists in no form, to an audience you call first-class citizens.
- The star catalogs are fetched from GitHub release assets (`src/renderer/stars.rs:1620-1629`) and validated only by an eight-byte magic header before being renamed into the data dir (`src/lib.rs:19161-19170`). That is a second, entirely unsigned install channel feeding a binary parser, and no release tier above touches it.