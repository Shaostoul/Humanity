# Ecosystem Release Checklist

A literal tick-list for every feature/fix release, to prevent drift across GitHub, VPS,
desktop app, and docs. This is the checkbox companion to the procedures already documented
in `docs/SOP.md` (version sync, deploy pipeline) and `docs/INCIDENT-PLAYBOOK.md` (failure
recipes), read those for the "how", use this list to confirm the "did I do every step".

## 0) Scope and intent
- [ ] Feature/fix is defined in 1-2 sentences.
- [ ] Impacted surfaces are listed:
  - [ ] Relay/backend (`src/relay/`)
  - [ ] Web chat (`web/chat/`, mirrored at `/var/www/humanity/chat/` on the VPS)
  - [ ] Desktop app (if affected)
  - [ ] Website/landing/docs

## 1) Local implementation (repo)
- [ ] Code changes completed.
- [ ] Local validation run: `cargo check --features relay --no-default-features` (server build) AND `cargo check --features native` (desktop build), see the "Known gotchas" entry in `CLAUDE.md` about why the relay check specifically cannot be skipped.
- [ ] Sensitive files check passed (no keys/tokens/secrets in tracked files).

## 2) Version + git hygiene (see `docs/SOP.md` "Version Sync Protocol" for the full procedure)
- [ ] Version bumped with `node scripts/bump-version.js [patch|minor]`.
- [ ] Version bump committed in the SAME commit as the change (not separate).
- [ ] Commit created with a clear message; multi-line messages use `git commit -F <file>`, never `-m` (PowerShell mangles them, see `CLAUDE.md` gotchas).
- [ ] Pushed to `origin/main`.
- [ ] Commit SHA recorded in release notes.

## 3) Deploy (automatic via CI, `just sync` if CI fails)
- [ ] Push to `main` triggers `.github/workflows/deploy.yml` (SSH to VPS, `cargo build --release --features relay --no-default-features`, rsync web assets, `systemctl restart humanity-relay`).
- [ ] If CI is red: `just sync` (force-fetch, reset, rebuild, rsync, restart).
- [ ] Service health verified: `just status` or `curl https://united-humanity.us/health`.

## 4) Web runtime sync (anti-drift)
- [ ] Runtime file matches repo file for chat client JS:
  - Repo: `/opt/Humanity/web/chat/app.js`
  - Runtime: `/var/www/humanity/chat/app.js`
- [ ] Browser hard refresh tested.

## 5) Functional verification
- [ ] Feature works in web chat.
- [ ] Feature works in native desktop binary (if applicable), run `just build-game` (CI does not build Windows).
- [ ] Regression check for nearby features completed.

## 6) Docs + comms
- [ ] `docs/STATUS.md` and `docs/FEATURES.md` updated if scope changed.
- [ ] `docs/BUGS.md` updated if a bug was fixed.
- [ ] User-facing docs updated (commands/UX/limits).

## 7) Release + signing (only for tagged releases, see `docs/SOP.md` "GitHub Release Tags")
- [ ] `git tag vX.Y.Z && git push origin vX.Y.Z && gh release create vX.Y.Z ...`
- [ ] Operator signs the release: `just sign-release vX.Y.Z` (operator-only, needs the passphrase, see `docs/admin/release-signing.md`). Unsigned releases are invisible to the desktop auto-updater.
- [ ] `gh release list` checked afterward to confirm the correct release holds "Latest" (a mistagged push can steal it).

## 8) Rollback readiness (see `docs/INCIDENT-PLAYBOOK.md` for the full recipe)
- [ ] A recent DB backup exists (`data/backups/`, auto-rotated, keep last 5).
- [ ] Restore command known: `cp data/backups/relay_LATEST.db data/relay.db && systemctl restart humanity-relay`.

## 9) Release done
- [ ] Final status recorded: SHA, deploy status, test results, known caveats, in `data/coordination/orchestrator_state.json` per the session-end convention in `CLAUDE.md`.
