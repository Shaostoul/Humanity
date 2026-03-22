# Ecosystem Release Checklist

Use this checklist for every feature/fix release to prevent drift across GitHub, website, app, and docs.

## 0) Scope and intent
- [ ] Feature/fix is defined in 1-2 sentences.
- [ ] Impacted surfaces are listed:
  - [ ] Relay/backend (`server/`)
  - [ ] Web chat (`/var/www/humanity/chat` runtime mirror)
  - [ ] Desktop app (if affected)
  - [ ] Website/landing/docs

## 1) Local implementation (repo)
- [ ] Code changes completed.
- [ ] Local validation run (`cargo check -p humanity-relay` or relevant tests).
- [ ] Sensitive files check passed (no keys/tokens/secrets in tracked files).

## 2) Git hygiene
- [ ] Commit created with clear message.
- [ ] Pushed to `origin/main`.
- [ ] Commit SHA recorded in release notes.

## 3) Deploy (server)
- [ ] Pre-deploy DB backup created.
- [ ] Pull latest code on server.
- [ ] Build release artifacts.
- [ ] Restart services.
- [ ] Service health verified.

### Standard command
- `/usr/local/bin/humanity-deploy-relay`

## 4) Web runtime sync (anti-drift)
If web runtime serves static files outside repo path, sync them after deploy.

- [ ] Runtime file matches repo file for chat client JS:
  - Repo: `/opt/Humanity/web/chat/app.js`
  - Runtime: `/var/www/humanity/chat/app.js`
- [ ] Browser hard refresh tested.

## 5) Functional verification
- [ ] Feature works in web chat.
- [ ] Feature works in app/desktop (if applicable).
- [ ] Regression check for nearby features completed.

## 6) Docs + comms
- [ ] User-facing docs updated (commands/UX/limits).
- [ ] Internal notes/changelog updated.
- [ ] Quick test instructions posted.

## 7) Rollback readiness
- [ ] Latest backup exists in `/opt/Humanity/backups`.
- [ ] Restore command validated:
  - `/usr/local/bin/humanity-restore-latest-db`

## 8) Release done
- [ ] Final status shared: SHA, deploy status, test results, known caveats.
