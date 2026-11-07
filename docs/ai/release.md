# Releasing, delivering and deploying

**Who reads this:** anyone about to bump a version, tag, cut a release, deliver the exe to the
operator, or touch the deploy. The step-by-step procedure is the `release` skill
(`.claude/skills/release/SKILL.md`); this page holds the rules it follows and why. `docs/SOP.md`
holds the wider development SOP and the component-update policy.

These rules lived in `CLAUDE.md` until 2026-10-23, when it was split by audience; their wording is
unchanged. One standing rule from CLAUDE.md governs everything here: **never tell the operator a
release needs signing.**

## When to release

**Release in batches, push often (operator, 2026-10-22).** Merge and push verified work as it
lands (cheap, and the push is the backup); between releases a merge needs only the checks it
touches (the relevant tests, `just lints`, both `cargo check`s). Cut a RELEASE (bump, release
build, `verify-runtime` and the rigs, tag, `archive-build`) once or twice a day on the batch, not
per feature: each release costs 30 to 45 minutes of the one machine, which ten releases in a day
took from the agents' builds. The steps below are the release, not every push.

## Component updates

**Keep components current, deliberately (operator, 2026-10-23).** `rust-toolchain.toml` pins one Rust for the PC, GitHub and both servers; `just brief`'s UPDATES row says when Rust, rustup or crates have updates and never applies them. Judge the release notes, update in a quiet window, servers first: docs/SOP.md "Keeping components up to date".

## Version SOP (the release, not every push)

*Note (2026-10-23):* this section was headed "MANDATORY before every push" until the 2026-10-22
batching rule above; these are the steps of a RELEASE.

**Semver rules:**
- `0.X.0` → Rust code changed (requires recompile on VPS)
- `0.X.Y` → Non-Rust changes only (HTML/JS/CSS/docs/config)
- `1.0.0` → Reserved for fully functional product

**Session start: Version sync check**
1. Read local version: `node -p "require('fs').readFileSync('Cargo.toml','utf8').match(/^version\\s*=\\s*\"(.+?)\"/m)[1]"`
2. Read GitHub version: `gh release list --repo Shaostoul/Humanity --limit 1`
3. If local > GitHub: push + tag + release immediately (GitHub must match local)
4. If local < GitHub: investigate (local should never be behind)
5. If equal: proceed normally

**Before tagging a release, ALWAYS:**
1. Bump version: `node scripts/bump-version.js [patch|minor]`
   - This updates all 7 locations: `Cargo.toml`, `Cargo.lock` (its own `humanity-engine` entry, since 2026-10-12: it had lagged a version behind), `web/shared/sw.js`, `web/pages/settings-app.js`, `web/pages/ops.html`, `web/shared/shell.js`, `web/pages/download.html`. Stage all seven BY NAME in the release commit (the v0.1316.0 commit missed one because the list here was short). `web/pages/index.html` left the list on 2026-11-02: the front door that replaced the page has no proof tile to stamp.
2. Commit the version bump IN the same commit (not separate)
3. Push to main
4. Tag and release: `git tag vX.Y.Z && git push origin vX.Y.Z && gh release create vX.Y.Z --title "vX.Y.Z" --notes "..."`
5. **SIGN THE RELEASE (release signing is ACTIVE as of v0.421.0).** After the tag's Build-Desktop workflow uploads the platform binaries, the operator runs `export HUMANITY_SIGNING_PASSPHRASE=... && just sign-release vX.Y.Z`. The desktop updater + the local launcher only trust SIGNED releases: an unsigned release is invisible to auto-update (not an error, just not offered). So every release from v0.421.0 onward MUST be signed or v0.421+ desktop users won't see it. Signing is operator-only (needs the passphrase + `release-signing-key.enc`); an AI/CI cannot sign. Full procedure: `docs/admin/release-signing.md`. **Do NOT tell the operator a release needs signing** - see the standing rule in CLAUDE.md; this step exists so the AI understands why an unsigned release is invisible to auto-update, not so it can be relayed.

*Superseded in part (2026-10-23 reading):* tag with `just tag --release <notes-file>` rather than
step 4's hand-typed chain. It reads the version from the committed `Cargo.toml` (the next
section) and takes the notes from a file (inline `--notes` is what the `-F`, never `-m` rule
below forbids). The `release` skill does exactly that.

**Session end:** never leave a session with local ahead of GitHub (`docs/ai/orchestrator.md`,
section 2).

**Never delete/re-tag**: always increment to next version number.

**`just build-game` auto-bumps the PATCH version at its START (it runs `just bump` before `cargo build`).** So the moment you launch build-game, the working tree already has all 8 locations `bump-version.js` stamps (Cargo.toml, Cargo.lock and the six web files) bumped to the next patch. The version-stamp commit after it must therefore just `git add` the already-bumped files and commit/tag; do NOT run `bump-version` again (that double-bumps). And do NOT inject a separate patch-bumped change (another `just release`/`bump-version`) while a build-game bump is still uncommitted in the tree: commit the build-game stamp FIRST, then do the next change. Getting this wrong strands local ahead of GitHub with a mislabeled tag (happened v0.705.1: a dead-code patch ran while build-game's 0.705.1 bump was pending, landing Cargo.toml at 0.705.2 under a v0.705.1 tag; reconciled by tagging HEAD v0.705.2 forward per the never-retag rule).

## Tag numbers come from the file, never from memory

- **NEVER type a tag number from memory. Read it back from `Cargo.toml` after bumping.**
  In a shared checkout another session ships releases while you work, so the version you
  saw at the start of your session is stale by the time you tag. This produced a
  mislabeled tag on 2026-08-25: a session that began at 0.1211.0 ran `bump-version.js
  patch`, which printed `0.1214.1 -> 0.1214.2` because a concurrent session had shipped
  three releases in between, and then tagged the commit `v0.1212.2` from its own stale
  arithmetic. Per the never-delete/never-re-tag rule the stray tag was kept, the correct
  `v0.1214.2` was tagged forward at the same commit, and the stray release was marked
  pre-release so it could not take the Latest pointer. The safe pattern:
  ```bash
  node scripts/bump-version.js patch          # or minor
  V=$(node -p "require('fs').readFileSync('Cargo.toml','utf8').match(/^version\s*=\s*\"(.+?)\"/m)[1]")
  git commit -F msg.txt && git push origin main
  [ -n "$V" ] || exit 1                       # an empty V tags a release named just "v"
  git tag "v$V" && git push origin "v$V"      # the number comes from the file, not from you
  # `just tag` does the read-back, the guard and the tag push
  ```
  That guard line exists because it happened (2026-09-29): a `grep -c` with no
  matches exits 1, which stopped an `&&` chain before the bump and the `V=`
  line, while the tag and release steps after a newline still ran. The result
  was a tag named just `v` and a release marked Latest (now a pre-release with
  a warning; the build it triggered was cancelled). Never put a command that
  can legitimately "fail" (grep, diff) in the middle of a release chain.

## Commit messages and tags: `-F`, never `-m`

- **Commit anything beyond one trivial line with `git commit -F <file>` (release notes: `gh release create --notes-file <file>`); NEVER `-m`, not even a `@'...'@` here-string.** PS 5.1 re-quoting splits the message: the commit silently FAILS while the `git tag` / `gh release create` after it still run, on the WRONG commit (`v0.415.0`, `v0.421.1`; content-dependent, so "it worked earlier" proves nothing). Every pushed tag auto-publishes a release, so after any tag push check `gh release list` for a stolen Latest or a mislabeled release. Never delete or re-tag a stray: mark its release PRE-RELEASE with a warning and increment past it (`v0.415.0` stays exactly as it is). Tag with `just tag` (`--release <notes-file>` creates the release): it reads the version from the committed Cargo.toml and refuses an empty version, an existing tag, an uncommitted bump, or a HEAD not yet on origin/main.

## Deliver the exe to the operator

- **Before pushing a release**: `git status --short` and stage any untracked .rs/.ron/.csv. Local builds pass with untracked files; CI fails on fresh checkout.
- **After pushing a Rust release**: run `just deliver` (build + archive + refresh the taskbar exe), or the operator cannot see the work. `just brief` now prints a DELIVERY row that answers this directly, and `just check-delivery` answers it on demand; the VERSION row above it compares the repo to GitHub, which is NOT the same question and read "in sync" through five undelivered releases. Detail follows. CI does not build Windows, and **`just launch` boots the newest `v*_HumanityOS.exe` ARCHIVE in the repo root**, which only `just build-game` or `node scripts/archive-build.js` creates. A plain `cargo build --features native --release` updates `target/release` and NOTHING else, so an agent can ship five tagged releases while the operator keeps launching the build from before them (this happened 2026-09-21: he was on v0.1326 at v0.1331). Use `node scripts/archive-build.js` on its own when the version is already bumped and tagged, since `just build-game` bumps again and would strand local ahead of GitHub.

## Deploy pipeline

Push to `main` → GitHub Actions (`.github/workflows/deploy.yml`, since 2026-10-30): **plan** reads from the VPS the commit the running relay was built from (`target/release/HumanityOS.source`) and the commit at its last restart (`HumanityOS.started`); **build** (only when `src/`, `Cargo.*`, `build.rs`, `rust-toolchain.toml` or `.cargo` changed) compiles `--features relay --no-default-features` on GitHub's Ubuntu 22.04 and checks it needs no newer glibc than the VPS's Debian 12 (2.36); **deploy** updates the checkout, timers and web root, swaps the new program in with one rename, and restarts the relay only when its program or `data/` changed, then waits for `/health` to answer ok. A docs or web-page push no longer restarts the relay. The VPS no longer compiles at all (it took 10 to 11 minutes on its two cores, beside the live relay).

Two more things every deploy does (2026-11-06, BUG-252): it installs the push's copy of every file the server runs from OUTSIDE the checkout (`scripts/server-files.txt`: the backup script in `/usr/local/bin`, the timers' units, the release manifest script) through `scripts/install-server-files.sh`, by rename, with a line for each one it replaced; and it clears the build leftovers in `target/` with `scripts/clear-build-cache.sh`, which keeps `target/release/HumanityOS` and the `HumanityOS.source`, `.started` and `.incoming` files beside it (29 GB of leftovers from the VPS's own builds were still there on 2026-11-06). public.guide gets the same files from `relay-follow-releases.sh`, which runs the release's installer after each update and every hour it finds the node current; both servers' disk guards clear the leftovers every 20 minutes. `just brief`'s SCRIPTS row (and `just server-files`) compares each server's installed copies with the repo's over a read-only ssh call and names any that differ.

The VPS runs the same unified binary in headless mode: `HumanityOS --headless`

When the GitHub deploy cannot run (the server has local changes, the workflow is broken, GitHub is down), the fallback builds on the VPS itself, slowly, and always restarts; it writes the same two markers the deploy reads. `just ship` no longer runs it (since 2026-10-30: the push deploys):
```bash
just sync    # fetches, git reset --hard, builds ON the VPS, lays out the web root, restarts
```

**VPS paths**:
- Repo: `/opt/Humanity/`
- Web root: `/var/www/humanity/`
- Relay binary: `/opt/Humanity/target/release/HumanityOS` (runs with `--headless`)
- SQLite DB: `/opt/Humanity/data/relay.db`
- Uploads: `/opt/Humanity/data/uploads/`

- After pushing a Rust release, glance at `just ci`: a red deploy means the VPS is silently running stale code.
- **The DM mailbox upgrade is forward-only (BUG-259, 2026-11-06).** Once a release carrying it has
  started successfully on a server, the release before it refuses every `dm_put` (it inserts mail
  with no `id`). From then on roll forward, never back: on united-humanity.us no "revert and push"
  and no `just sync` from an older commit; on public.guide no reinstalling the previous release.
  The incident playbook says the same (`docs/INCIDENT-PLAYBOOK.md`, "The relay will not start
  after an update").
- Deploy `git pull` fails if server has local changes -> `just sync` fixes it
- **The forge mirror PULLS from GitHub every 8 hours; nothing pushes to it (since 2026-09-29).** `just ship` pushes only to `origin`; the local `forge` remote is `https://git.united-humanity.us/shaostoul/Humanity.git`, fetch-only and credential-free (the old SSH push setup no longer applies: a pull mirror refuses pushes). Health: `just brief`'s MIRROR row. It once vanished for eight weeks unseen: `docs/admin/forgejo-setup.md`.

## Instrument packs (release assets; uploading is the operator's step)

Settings > Audio > Instruments downloads the instrument packs (docs/design/music.md rung 6) from
GitHub release assets under the non-`v*` tag `assets-instruments-1`, the star catalog's way
(`assets-stars-1`): uploading there triggers no workflow and serves from GitHub's CDN. The app checks
every download against the SHA-256 in `data/music/packs.ron` and refuses a mismatch, so **the catalog
committed to main and the assets uploaded must come from the same run**. Producing the packs is an
agent's job; **uploading them is publishing, so it is the operator's step**. An agent never uploads.

1. Build (needs the raw libraries on `E:\HumanityOS\instruments-src`, recorded in
   `docs/reference/instrument-sources.md`, and ffmpeg with libvorbis; about half an hour):

   ```bash
   just instrument-packs
   ```

   It rewrites the core set in `data/music/` and `data/music/packs.ron`, and writes each pack as
   `E:\HumanityOS\instrument-packs\humanity-instruments-<pack>-v<version>.tar` with a `.sha256`
   beside it. The encoding is bitexact, so an unchanged recipe on the same ffmpeg rebuilds
   byte-identical packs (`git diff data/music/packs.ron` shows no change). Each pack carries each
   library's licence text in every sample folder (a release asset has no licence file of its own).

2. Upload (the operator; the first time creates the release, later times replace the assets):

   ```bash
   gh release create assets-instruments-1 --repo Shaostoul/Humanity --latest=false --title "Instrument packs" --notes "CC0 instrument packs for Settings > Audio > Instruments. Built by just instrument-packs; sources in docs/reference/instrument-sources.md." E:/HumanityOS/instrument-packs/*.tar E:/HumanityOS/instrument-packs/*.sha256
   # later rebuilds:
   gh release upload assets-instruments-1 --repo Shaostoul/Humanity --clobber E:/HumanityOS/instrument-packs/*.tar E:/HumanityOS/instrument-packs/*.sha256
   ```

   **Never let an asset release take GitHub's "Latest" label** (`--latest=false` above). The
   download page, `web/shared/shell.js`'s version check and the relay's `releases/latest/download/`
   links all read `releases/latest`, which must always be the newest GAME release. On 2026-11-07
   the first upload was made without the flag and became Latest, so the download page found no
   app; `gh release edit v<newest game version> --repo Shaostoul/Humanity --latest` put it back.

3. Commit `data/music/` (the core set and the catalog) from the SAME run, then ship. A pack whose
   contents change gets its `version` raised in `scripts/instrument-recipes.json` first, so its file
   name changes and an installed older pack is offered as an update.

To try packs before uploading, serve the folder on this computer and point the app at it:
`node scripts/serve-instrument-packs.js` (127.0.0.1:8000, the packs folder) and run the app with
`HUMANITY_INSTRUMENT_PACKS_URL=http://127.0.0.1:8000/` (a dev aid, `instrument_packs::ui`).

## Recipes

```
just sync             # fallback deploy, builds on the VPS ← only when the GitHub deploy cannot run
just sync-web         # assets only, no rebuild (fast)   ← front-end changes
just build-game       # bump version, compile, archive versioned exe
just play             # build-game + launch
just launch           # launch latest build (no compile)
just build-relay      # headless server build (no GPU)
just status           # git + CI + live API health
just logs             # tail server logs
just deliver          # build + archive + refresh the taskbar exe (after a Rust release)
just check-delivery   # is the operator's exe this release?
just tag              # tag from the committed Cargo.toml (`--release <notes-file>` creates the release)
```

`just play` and `just launch` set `HUMANITY_TAKE_FOCUS`: they are the operator's recipes, never an
agent's (CLAUDE.md, "The operator's machine").
