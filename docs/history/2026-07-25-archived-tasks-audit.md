# Archived-tasks audit (2026-07-25, overnight backlog item 7)

Operator ask: "I think at one point you said you archived tasks. Let's evaluate
our archived tasks." A read-only sweep of 13 archived task lists and backlogs
(docs/history/ roadmaps and loop results, UI-AUDIT, SECURITY-CADENCE, the
PRIORITIES tiers below Active focus, orchestrator_state pending gaps, the
2026-04-26 agent-scope session audits, and three design-doc deferral lists),
each candidate cross-checked against the live tree, FEATURES.md, and STATUS.md
so nothing completed gets re-planned.

Repo was at v0.965.1 during the audit. Corrections applied the same night:
the two flat-wrong TIER 2 claims in PRIORITIES.md (native voice "No WebRTC
stack at all" - shipped v0.485-0.495; Studio "native transport stubs only" -
shipped v0.853-0.854) now carry STALE-CORRECTED notes.

## Top 10 still-relevant, not-done items (verified)

1. Release-signing backlog: v0.678 through current builds are unsigned, so
   v0.421+ desktop auto-update cannot see them. Operator-only action.
2. Native Identity + Recovery pages are dead stubs whose blocker no longer
   exists: the v2 endpoints shipped (src/relay/api_v2_did.rs, api_v2_trust.rs,
   api_v2_credentials.rs, api_v2_recovery.rs routed in relay/mod.rs:714-717)
   but identity.rs / recovery.rs only set *_pending flags nothing reads.
   Highest-leverage unblocked item.
3. Backup-restore drill has never been run once (SECURITY-CADENCE section 5
   log is empty; quarterly cadence; 3-2-1 chain shipped in May).
4. Monthly dependency audit overdue (last real run 2026-06-16) plus the
   accepted rsa/RUSTSEC-2023-0071 row owes a re-check.
5. Web governance has voting but no proposal-creation form
   (web/pages/governance.html:191 still points users at the native app).
6. No user-to-user block/report on native, no abuse-report pipeline anywhere
   (web has /block /unblock /blocklist; native and relay storage have none).
   Gates "invite strangers".
7. Native voice tail: per-peer volume/mute/squelch, web transmit-mode UI,
   two-str0m CI harness, graceful relay restart.
8. Distribution sovereignty steps 2/5/6/7: Codeberg, Internet Archive,
   Software Heritage, WinGet (steps 5+6 estimated ~40 min).
9. Wall-corner seam for mismatched-thickness walls
   (docs/design/wall-corners.md, "diagnosed, fix deferred"; needs a careful
   visual pass - a rushed fix regresses working equal-thickness corners).
10. Web universal widget layer never built: hosToast/hosConfirm missing while
    33 raw confirm() calls remain in web JS.

## Runners-up (verified not-done)

- Missing web/native pages from the 2026-04 roadmap: global search, events
  feed, notifications inbox, community/server directory (matters more since
  native Federation Phase 1 shipped), in-app help index.
- Sim-realism residue: durability/wear/repair absent; no Needs ECS component;
  PsychologySystem never registered; no point-light entity type; battery SoC
  hard-reset at spawn (home_spawn.rs:134).
- Concurrency-roadmap deferrals: C1 inline PBKDF2 in three draw_* paths,
  C7 blocking connect-time history GET (lib.rs:2653), C8 blocking clipboard
  image upload.
- Quest chains never authored: tasks, marketplace, wallet, contributor
  (data/quests/ has construction/exploration/farming/getting_started/tutorial).
- File-level delta sync in the auto-updater (forward-roadmap step 4.8).
- Frustum culling absent from the renderer (more relevant at 16k trees/km2).
- Operational observability + /api/v2/admin/* (low value, untracked).
- theme_token_lint allowlist rot (stale maps.rs entry; five live offenders).
- UI-AUDIT section 8: vault/relay sync for localStorage-only web pages
  (calendar, notes, bookmarks).

## Corrections applied to live docs

- PRIORITIES.md TIER 2 items 4 (native voice) and 2 (Studio transport):
  stale claims corrected in place, 2026-07-25.
- Known remaining wart: TIER 2 numbering runs 1-7 then restarts at 4-9, and
  STATUS.md cross-references those numbers. Renumbering deferred (touching it
  breaks the cross-references; do both together in a docs pass).

Everything else in the archives checked out as done or superseded; the full
verification detail (file-by-file) lives in the session journal for
2026-07-25. Never re-plan items from the "done" lists in those archives.
