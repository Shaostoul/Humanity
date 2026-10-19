# The full text of the Known gotchas in CLAUDE.md (moved 2026-10-19)

CLAUDE.md is loaded by every session and every helper agent, so on 2026-10-19
each long entry in its "Known gotchas" section was cut down to its rule: what to
do or never do, a one-line why, and a pointer here. This file keeps each entry
EXACTLY as it stood in CLAUDE.md that day, incident narrative included (dates,
versions, what broke, who hit it), so nothing was lost in the cut.

The RULES live in CLAUDE.md, and where the two differ CLAUDE.md wins. Two entries
below were already out of date when moved, and are marked so.

When you cut another gotcha down, append its full text here the same way.

## The megashader and its per-class fragment entries

(CLAUDE.md line 829 on 2026-10-19; design of record: `docs/design/frame-cost-arc.md`, section 1, the P1 to P3 outcomes.)

- **Every fragment of every pipeline that uses `fs_main` pays the megashader's per-invocation private frame (H1, proven 2026-09-18, `docs/design/frame-cost-arc.md`).** The cloud march's 2.9 KB `var<private>` array was charged to every terrain and interior pixel with clouds OFF: 44.5 ms for six moon patches. Since P2 and P3 (same day) there is NO `fs_main`: six class entries (`fs_surface`, `fs_terrain`, `fs_vegetation`, `fs_water`, `fs_shell`, `fs_cloud` in `90-fragment-main.wgsl`) share `frag_prologue` + `frag_tail` (`80-fragment-shared.wgsl`), and the thirteen megashader PSOs each compile ONE entry plus a dead-switch set (`pipeline.rs` `ShaderClass`, `shader_class(material_type)` pinned to the WGSL bands by a test, `PSO_REGISTRY` rows, `MegashaderPsos`). Every draw loop picks its PSO by the object's class (`Pipeline::opaque_for` / `transparent_for` / `overlay_for`; `draw_opaque_objects` walks an opaque list once per class present); a material on an entry that lacks its branch renders as the default panel grid with no error in release. The celestial transparent list is grouped by the planet LAYER BAND (`renderer/celestial_order.rs`), never by class: a class key put the type-13 fallback dome under the sea (P2 review). Numbers: limb 25.0 to 0.37 ms and the console room 82.9 to 16.8 on P2, then 17.1 to 14.1 on P3 (register pressure). Rules: a new heavyweight branch gets its own `override` switch, lives in exactly one class entry, and is turned off in every pipeline that cannot draw it; the switch is the FIRST operand of the guard (`if (HAS_X && type_test)`), the convention the tests pin; naga IGNORES an unknown override key and returns a module with no overrides unchanged, so a rename on one side, or a stale assets mirror without the declarations, silently re-enables the branch (the `permutation_tests` in `pipeline.rs` and the switch check in `shader_loader::validate_wgsl` guard both; keep them). Measure with the rig, not by reading: the same-boot A/B through the shader hot reload is the tool.

## Bind group layouts and the world-entry verify

(CLAUDE.md line 830 on 2026-10-19; also the 2026-07-28 decision in `docs/history/journal-archive-2026-07.md`.)

- **Bind-group-layout changes must update EVERY create_bind_group site, and renderer verifies must ENTER THE WORLD (incident v0.1029-v0.1038, 2026-07-28).** Adding binding 15 to the texture BGL updated two of three creation sites; the missed one (the per-material Material Albedo Bind Group, built lazily when a TEXTURED material loads) made every WORLD ENTRY panic with a wgpu validation error - while menu-only boot-verifies stayed green for TEN releases (the operator hit it: "insta crashes when I press esc"). Rules: (1) after touching a bind group layout, grep that layout name for ALL create_bind_group sites and count entries at each; (2) the boot-verify bar for renderer changes now includes WORLD ENTRY - use the probe rig (node scripts/probe-sweep.js --only blue-marble-12000km --exe target/release/HumanityOS.exe): it runs a portable sandbox (the in-place exe REFUSES autopilot when a real identity exists), enters via autopilot, and reports panics=0.

## Renderer and device changes: boot the release exe

(CLAUDE.md line 831 on 2026-10-19; recorded nowhere else.)

- **Renderer/device changes MUST be verified by BOOTING the release exe (incident v0.782-784, 2026-07-10).** `cargo test` + naga shader validation do NOT catch wgpu DEVICE-LIMIT rejections: the v0.782 lights storage buffer passed every test, but the device was requested with `downlevel_webgl2_defaults` (fragment storage buffers = 0), so `create_bind_group_layout` failed validation at startup and the app died before the first frame — THREE releases (v0.782.0-v0.784.1) shipped unbootable before the operator hit it ("a flicker but the game never comes up"). Limits are now `wgpu::Limits::default()` (`renderer/mod.rs` `request_device`). Verify bar for ANY renderer/pipeline/bind-group/shader change: `cargo build --features native --release`, launch it with **`just launch-bg`** (it runs the freshness gate first, so the binary is proven to be this tree's build (BUG-133), and starts it with `HUMANITY_NO_FOCUS=1`, without which the boot-verify window steals foreground focus and kills the operator's raw-input mouse-look mid-game, the "mouse stopped aiming" incident, root-caused 2026-07-27, and `HUMANITY_NO_HANDOFF=1`, so it cannot hand itself off to a newer archive; a hand-typed `target/release/HumanityOS.exe` skips all of that), wait ~10 s, grep `%APPDATA%/HumanityOS/logs/run.log` for `PANIC` (expect 0), then `taskkill //PID <pid>`. When the operator reports a boot failure, read `run.log`/`crash.log` FIRST — the panic hook always writes the cause there.

## f32 at planet scale

(CLAUDE.md line 832 on 2026-10-19; the water arc detail is also in `docs/design/water-fft.md`.)

- **f32 at planet scale is a defect class: never pass planet positions OR unit directions through f32 on the way to a terrain/data sampler.** An f32 UNIT vector quantizes ground position at ~0.4-0.8 m; an f32 global-grid coordinate (lon-derived, ~72k cells) quantizes at ~3.6 m - both drew real artifacts (v0.1010 terrain ripples at the operator bm-7 bookmark; v0.1012 clamp micro-stepping). Keep dirs as DVec3 end-to-end and downcast ONLY at a coarse-data boundary (base heightmap cells are 460+ m, so .as_vec3() is fine THERE and nowhere earlier). GPU/WGSL is f32 by hardware - the discipline there is small magnitudes near the camera (floating origin, anchor-relative patch verts, drawn-shell units). Audit sweep 2026-07-27 (operator request) cleared: mesh vertex path, camera/floating origin, anchors/spin, ballistics, cloud shader units. **The water arc (v0.1017-v0.1018, same day) then proved the GPU edition of the class:** any shader phase/UV computed from a planet-radius-magnitude f32 dot (`dot(dir * r, d)`) carries ~0.5 m quantization that SHIFTS with camera motion - on short wavelengths that is camera-coupled vertex jitter. The established fix pattern: evaluate in the camera-anchored 64 m-modulus domain (`anchw + dvw`, the wave-texture anchor) with wavelengths that DIVIDE 64 and axis-aligned directions, so anchor re-snaps shift phase by exact integers and the CPU f64 twin matches mod 1 exactly (ocean chop trains W4/W5/W6 = the worked example; the lockstep test guards it). Also caught: a CALLER narrowing to f32 before an f64-upgraded function (the buoyancy float) - upgrading a function's internals is worthless if the call site already quantized; check the whole chain.

## SQLite schema changes and ALTER-added columns

(CLAUDE.md line 833 on 2026-10-19; also BUG-046 in `docs/BUGS.md`.)

- **SQLite schema changes: never reference an ALTER-added column from the main schema batch (BUG-046, took the live relay down ~25 min, v0.675.0).** The main `execute_batch` in `src/relay/storage/mod.rs` runs BEFORE the ALTER-TABLE migration block. On a fresh DB, `CREATE TABLE IF NOT EXISTS` includes new columns so everything passes (all unit tests + local smoke tests are fresh-DB); on the LIVE DB the table already exists without them, so an index/trigger/view over the new column aborts the whole batch and the relay dies at startup (exit 3). Rule: indexes over ALTER-added columns go AFTER the ALTER block, and any change to an existing table's shape needs a pre-migration-shape `Storage::open` test (pattern: `opens_a_pre_v0675_database_and_migrates_it` in `src/relay/storage/uploads.rs`).

## Worktree and main build caches filling C:

(CLAUDE.md line 834 on 2026-10-19; also `docs/history/2026-09-27.md` and `docs/history/2026-09-29.md`.)

- **Every worktree agent builds its own full `target/` (about 8 to 10 GB), and they are never cleaned up (disk-full incident, 2026-09-27).** Overnight, parallel worktree builds filled C: completely: builds died with "not enough space on the disk" (os error 112) and the linker's LNK1318. At the time, 56 old worktrees held 451 GB of build caches. Rules: when a worktree agent's branch is merged, delete that worktree's `target/` (build output only: the worktree and its branch stay); give scratch builds a target inside the scratchpad and delete it after; check `df -h /c` before launching a wave of worktree agents. Deleting `target/` never loses work, so it is not the operator-only `clean-worktrees` action, which removes worktrees and force-deletes branches. **The MAIN `target/` grows without limit too (2026-09-29): 1.13 TB, with C: at 91%.** Cargo never prunes: `debug/incremental` was 688 GB, `debug/deps` held 397 old test-binary debug files (245 GB) and 361 old test executables (44 GB), `release/deps` 120 GB. `cargo clean` when C: runs short (no other session building, no game running); the next release build and test build cost about 10 minutes together. A landed worktree (`git cherry main <branch>` prints no `+` line) can go with `git worktree remove --force` while its branch stays; the only local change in each was `.claude/settings.local.json`. **Since 2026-09-29 this is automatic:** the hourly, silent "HumanityOS Dev Disk Guard" task (`scripts/pc-disk-guard.js`, installed by `just install-disk-guard`) deletes the `target/` of any worktree idle 48 h, the main target's `debug/incremental` at 75% full and the whole main `target/` at 85%, and never while cargo, rustc or HumanityOS runs; `just brief`'s DISK row shows when it last ran. Build output only: never source, saves, screenshots or backups.

## Checking the relay build before a push

(CLAUDE.md line 835 on 2026-10-19; recorded nowhere else.)

- **Check the RELAY build before pushing any Rust change: `cargo check --features relay --no-default-features`.** The native check alone is NOT enough — CI deploys with the relay feature set, and a native-only module left ungated breaks it invisibly. An ungated `save_load` (it imports the native-gated `persistence`) kept the Deploy-to-VPS workflow red for 25 consecutive releases (v0.381→v0.414) with nobody noticing, so the live relay binary never rebuilt; fixed v0.416.0 with a `#[cfg(feature = "native")]` gate. Rule: any new `src/` module touching GUI/renderer/persistence/save state gets the native gate. The repo's `just install-hooks` pre-commit runs exactly this check (operator: install it once). **Also: after pushing a Rust release, glance at `just ci` — a red deploy means the VPS is silently running stale code.**

## PowerShell Get-Content and Set-Content

(CLAUDE.md line 836 on 2026-10-19; recorded nowhere else.)

- **NEVER round-trip a repo source file through PowerShell `Get-Content`/`Set-Content`.** PS 5.1's `Get-Content` decodes BOM-less UTF-8 as the ANSI codepage, silently corrupting EVERY non-ASCII character (emoji, arrows, box-drawing in comments), and `Set-Content -Encoding utf8` adds a BOM + rewrites line endings — a "1-line append" becomes a 570-line diff of mojibake. It bit twice within one hour on 2026-07-06 (both times on chat.rs; recovered via `git checkout --` + redoing edits with the Edit tool). Use the Edit tool (with `replace_all` for repeated lines) or a `node` script (byte-exact) for any scripted source edit.

## git commit -F, never -m

(CLAUDE.md line 837 on 2026-10-19; recorded nowhere else.)

- **ALWAYS commit multi-line messages with `git commit -F <file>` — NEVER `-m`, not even a `@'...'@` here-string.** Windows PS 5.1 native-arg re-quoting splits the message and git treats the tail as pathspecs (commit silently FAILS while the subsequent `git tag`/`gh release create` still run → a tag/release on the WRONG commit + uncommitted work). This has now bitten TWICE: the stray `v0.415.0` tag (2026-06-11) AND `v0.421.1` (2026-06-12, a `@'...'@` here-string mangled despite the same here-string working in earlier commits that session — it is content-dependent and unreliable). Use `git commit -F <tmpfile>` + `gh release create --notes-file <tmpfile>` for ANYTHING beyond a single trivial line. After any tag push, `gh release list` to check for a stolen Latest / mislabeled release. The stray `v0.415.0` tag points at the v0.414.0 commit; the tag-triggered Build Desktop App workflow then auto-published a release for it (briefly stealing "Latest" from v0.416.1 — caught 2026-06-11). It is now marked PRE-RELEASE with a warning body. Per the never-delete/re-tag SOP: leave the tag AND the prerelease exactly as they are; we incremented past it. Corollary: any pushed tag gets an auto-release from the workflow, so a mistagged push must be followed by checking `gh release list` for a stolen Latest pointer.

## cargo fmt

(CLAUDE.md line 842 on 2026-10-19; recorded nowhere else.)

- **NEVER run `cargo fmt` (or `cargo fmt -p humanity-engine`) in this repo.** The codebase is NOT maintained rustfmt-clean — a whole-crate fmt reformats ~240 files (huge whitespace churn) and, worse, **moves trailing `// theme-exempt:` / inline comments onto their own line** when the source line exceeds rustfmt's 100-col max_width, which silently BREAKS `theme_token_lint` (the exempt marker must be INLINE on the same line as the `Color32` literal). Match surrounding style by hand. If you ever do run it by reflex, `git diff --stat` before committing — a 200+ file diff means fmt ran; revert the fmt-only files to HEAD and re-apply your real edits manually. (Incident v0.390, 2026-06-08.)

## LNK1318 on Windows test builds

(CLAUDE.md line 843 on 2026-10-19; the lib-test part is BUG-174 in `docs/BUGS.md`.)

- **Windows `cargo test --test <X>` can fail with `LNK1318: Unexpected PDB error; LIMIT (12)`.** Integration tests make cargo LINK the full native `HumanityOS` bin (so it's available via `CARGO_BIN_EXE`), and that bin's debug PDB has grown large enough to trip MSVC's type-server limit. This is an ENVIRONMENT limit, NOT a code error, and it does NOT affect `cargo build`/release (release links fine) or Linux CI (no PDB). Workarounds: (1) **`just test-lib [filter] [--nocapture]`** (`scripts/lib-tests.js`) runs the LIB unit tests (the bulk). **Since 2026-10-09 the LIB test program itself hits LNK1318 too** (BUG-174), so a bare `cargo test --features native --lib` FAILS on Windows; the script builds it with `cargo rustc ... -- -C link-arg=/DEBUG:NONE` (no PDB) and runs it, and `just verify`, `validate-data` and the snapshot recipes all go through it. Measured that day and NOT the cause: the PDB's size (line-tables-only shrank it to 487 MB, still refused), the object count (~2,300), and `strip` (ignored by rustc's MSVC link). (2) The four `src/gui` **file-scanner lints** (`emdash_lint`, `theme_token_lint`, `theme_editor_coverage`, `icon_glyph_lint`) are std-only and import nothing from the crate, so compile + run each standalone, dodging the lib+bin entirely:  `CARGO_MANIFEST_DIR='C:\Humanity' rustc --test --edition 2021 -A warnings tests/emdash_lint.rs -o t.exe && ./t.exe` (set `CARGO_MANIFEST_DIR` because the tests read it via `env!` at COMPILE time). If you genuinely must run a bin-dependent integration test on Windows and hit this, `taskkill //F //IM mspdbsrv.exe` then retry; if it persists it's the size limit, not a stale server. (v0.411, 2026-06-08.)

## The unified binary

(CLAUDE.md line 844 on 2026-10-19; now in the Architecture section of CLAUDE.md.)

- **Unified binary (v0.90.0):** `server/` merged into `src/relay/`, `native/` merged into `src/`, `crates/` eliminated. Single `Cargo.toml` at repo root with feature flags (`native`, `relay`, `wasm`). No workspace.

## VPS and desktop feature sets

(CLAUDE.md line 845 on 2026-10-19; now in the Architecture section of CLAUDE.md.)

- VPS builds use `--features relay --no-default-features` (no GPU deps). Desktop uses `--features native` (default).

## Context rot prevention (superseded advice)

(CLAUDE.md line 846 on 2026-10-19; superseded 2026-10-19, see the note below.)

- **Context rot prevention (CRITICAL for AI agents):** Stale git worktrees cause AI agents to write edits to cached dead file paths. If you see an agent reporting edits to `native/src/` or `server/src/` (paths that no longer exist on main), the agent found a stale worktree. **Fix: run `just clean-worktrees` regularly** to remove all worktrees except main + current. Never trust agent reports blindly. After major restructures, immediately sync your current worktree to main with `git fetch origin main && git reset --hard origin/main` to ensure file layout matches.

**Superseded 2026-10-19.** Two parts of this entry no longer stand. "Run `just
clean-worktrees` regularly" lost to START HERE step 0: that command is for the
operator or an orchestrator only, never a dispatched subagent. And `git reset
--hard origin/main` is safe only inside a throwaway worktree of your own: in the
shared main checkout it destroys other sessions' uncommitted work (the "Sharing
this checkout" section of CLAUDE.md).

## clean-worktrees before it was hardened (superseded)

(CLAUDE.md line 847 on 2026-10-19; superseded 2026-07-01 by the hardened script, see the note below.)

- **`just clean-worktrees` is destructive, not just tidying (incident 2026-07-01).** `scripts/clean-worktrees.sh` runs `git worktree remove --force` AND `git branch -D` (force-delete) on every worktree/branch except main and your current one, unconditionally, no check for uncommitted OR unmerged-but-committed work. Two dispatched subagents' completed, review-approved-minus-one-bugfix work (a spot-light rendering feature and a whole web Accord page) were destroyed this way mid-session, unrecoverable via stash/reflog/any branch, because clean-worktrees ran while their worktrees still held unmerged changes. **Rule: before running `clean-worktrees`, confirm every dispatched agent this session is either merged into main or has been explicitly abandoned.** Committing inside a worktree does NOT protect it, `git branch -D` deletes the branch (and eventually its commits) right along with the worktree.

**Superseded the same day.** `scripts/clean-worktrees.sh` was hardened on
2026-07-01: it now SKIPS a worktree with uncommitted changes or unmerged commits,
and only `--force-unmerged` destroys one (see the script's header). The
"unconditionally, no check" description above is how it behaved before that.

## The forge mirror

(CLAUDE.md line 848 on 2026-10-19; also `docs/admin/forgejo-setup.md`.)

- **The forge mirror PULLS; nothing pushes to it (since 2026-09-29).** git.united-humanity.us is a Forgejo pull mirror of GitHub that fetches every 8 hours by itself, so `just ship` pushes only to `origin`. The local `forge` remote is `https://git.united-humanity.us/shaostoul/Humanity.git`, fetch-only and credential-free (the old SSH push setup, and its GCM warnings, no longer apply: a pull mirror refuses pushes). Health: `just brief`'s MIRROR row reads the mirror's last sync from its API. It was once gone for eight weeks unseen while pushes to it failed with errors ignored; see `docs/admin/forgejo-setup.md`.
