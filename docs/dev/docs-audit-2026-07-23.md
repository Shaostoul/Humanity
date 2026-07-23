# Documentation audit - 2026-07-23

Run by the `docs-audit` multi-agent workflow (5 audience auditors +
synthesis) against the code at v0.930.1. Full raw output + per-agent journal:
the workflow transcript under `subagents/workflows/wf_5d2f9746-f8e/`.

**Overall:** docs are in good-to-strong health. The link checker is green (0
broken across 344 files) and no stale `native/src`, `server/src`, or `crates/`
monorepo paths survive anywhere. Drift is concentrated in slow-moving
inventory/onboarding files, not structural rot. 20 real findings; the sharpest
are AI-facing (an AI following the onboarding docs literally cannot connect).

> Caveat: one of the five auditor agents returned placeholder data (area
> "test", doc "a"/detail "b") and was excluded - so one audience got thinner
> coverage this pass. Re-run that auditor if a fuller sweep is wanted. Verify
> each finding against code before acting (this file records claims, not
> confirmed fixes).

## High severity

1. **docs/ai/onboarding.md + data/ai/onboarding.json** - the connect flow is
   missing the mandatory two-phase `identify` challenge. After `identify` the
   client must handle `identify_challenge {nonce}`, sign
   `hum/identify/v1\n{nonce}\n{public_key}` with its Dilithium3 key, and reply
   `identify_response {sig_b64}` (bot_* + bot_secret fast-path skips it). Since
   the HIGH-2 fix (v0.274.0) a non-bot socket is never bound without it, so the
   docs' single-step `identify` leaves an AI silently unauthenticated.
2. **docs/ai/onboarding.md** - profile message type is wrong: `update_profile`
   should be `profile_update` (the serde-renamed variant), and the example
   fields should match the real `ProfileUpdate` variant (drop
   name/timestamp/signature, which belong to the separate `profile_gossip`
   federation object).
3. **docs/admin/SELF-HOSTING.md** (Quick Start ~L60-66) - overpromises: the bare
   relay exposes only `/ws` + `/api`; its `client/` static fallback is never
   populated by any build step, so "your own copy of the website at /" 404s.
   The website is served separately by nginx over `/var/www/humanity` (as the
   Production section already sets up).

## Medium severity

4. **data/ai/onboarding.json** (L109, L112) - `docs/ai-onboarding.md` ->
   `docs/ai/onboarding.md` (moved in the v0.422.3 refactor). The link checker
   only scans markdown, so this dead pointer in machine-readable JSON is
   invisible to CI.
5. **docs/STATUS.md** (KNOWN GAPS ~L16-19) - the "NetSyncSystem is never
   instantiated" claim is false (`NetSyncSystem::new()` at src/lib.rs). Restate
   the real gap: the two-human live-world proof (wiring is shipped).
6. **docs/STATUS.md (L471) + docs/FEATURES.md (L479-481)** - dead references to
   `web/pages/data.html` and `web/pages/projects.html`, both deleted in the
   2026-07-05 fluff trim. Keep the live Projects task-CRUD entry distinct from
   the removed timeline page.
7. **docs/ROADMAP.md** (Right now ~L35-61; world ~L208) - frozen at ~v0.637,
   omits ~250 releases of the planet-scale graphics arc. It is both the public
   roadmap and the build to-do list; refresh then regenerate data/roadmap.json
   via scripts/roadmap-to-json.js.
8. **docs/STATUS.md + docs/FEATURES.md** - drifted game-data counts: items ~756
   (docs say 404), plants 134 (161), creatures 92 (123), data files ~215 (108).
   Recipes (~366) and chemistry (396) are still accurate. Consider replacing
   hardcoded counts with a "generated" note.
9. **docs/PAGES.md** (native heading L24) - says "35 GuiPage variants"; real
   count is 37 (+None). Watch (v0.857) and RelayControl (v0.846) were added to
   the tables but not the count. Consider extending page_registry_lint.rs to
   assert the native count too (it only guards the web count today).

## Low severity

10. **docs/admin/SELF-HOSTING.md** (Admin Commands ~L322) - `/server-trust
    <name> <tier>` should be `/server-trust <server_id> <0-3>` per the handler's
    usage string (src/relay/relay.rs).
11. **docs/user/ONBOARDING.md** (L92) - "a short hex string" is a pre-quantum
    leftover; the `public_key` has been Dilithium3 hex (~3904 chars) since Inc3.
    Drop "short" or describe the `did:hum:...` DID (which is genuinely short).
