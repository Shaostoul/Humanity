# Building and testing on the shared PC

**Who reads this:** every agent that compiles the crate, runs its tests or runs `just lints`
and `just verify` on the operator's PC. It says what the build tooling does on its own, so you
do not write a waiter of your own, rerun a suite for nothing, or fight another session for
memory. Written 2026-10-23 from the build-speed review (operator: "Less waste = more
productivity").

## The machine

One PC: 32 GB of physical memory, 6 cores, a 100 GB page file since 2026-10-23 (commit limit
about 132 GB; it was 8 GB before, and out-of-memory kills were seen 13, 11 and 21 times on
2026-10-20, 21 and 22). Several sessions build on it at once, each in its own worktree. One
compile of our crate (`humanity_engine`, about 600k lines in one crate) peaks near 10 GB of
private memory. Memory, not CPU, is what runs out: with the bigger page file a crowd of compiles
no longer dies, it pages, and then every one of them crawls and so does the operator's PC. On
2026-10-23 at 11:58 there were 80 cargo and rustc processes and 690 MB of free physical memory.

## The build slot: compiles of our crate take turns for memory

`just build-slot` (in the main checkout, once per machine, and again after its source changes)
builds `scripts/build-slot/hos-build-slot.rs` into `C:\Humanity\.cargo\hos-build-slot.exe` and
writes the machine-local `C:\Humanity\.cargo\config.toml` that makes cargo run every compile of
the workspace's own units through it (`[build] rustc-workspace-wrapper`). Cargo finds that file
from the main checkout and from every worktree under it. Both files are gitignored on purpose: a
checked-in relative path would point each worktree at a wrapper of its own, and Linux CI and the
servers have none.

For each compile of our crate (the 700 dependency crates are not gated) it:

- **waits** until free physical memory (and, second, free commit memory) has room for the
  compile's expected peak plus 2 GB, counting what compiles already running may still grow by
  and what compiles waiting longer need. First come, first served; a small compile passes a big
  one only when both fit. It says so on the build's output, with the numbers and what is
  running, and gives up waiting after 60 minutes (compiling anyway, loudly).
- **retries** a compile that died of memory (exit 0xC0000409, "memory allocation of N bytes
  failed", "LLVM ERROR: out of memory", Win32 error 1455, E0786), up to three attempts, asking
  for more room each time. The dead attempt's error lines are not passed to cargo, so a compile
  that succeeded on its second try shows no errors. Every retry is a line in
  `%LOCALAPPDATA%\HumanityOS\build-slot\oom-retries.txt`.
- **learns** each kind of compile's peak (the whole process tree, the linker included, from a
  Windows job object) and its CPU seconds, in `peaks.txt` beside it.

What you do with it:

```bash
just build-slot-status            # who is compiling, who waits, the memory, the learned peaks
HUMANITY_BUILD_SLOT=off cargo ... # skip the wait for one command (you then compete for memory)
just build-slot-check             # exit 1, naming the fix, if the wrapper is missing or stale
```

Never delete the config to get around it: cargo hashes the wrapper into our crate's artifacts,
so every checkout would rebuild the crate whole. If cargo ever says it "could not execute
process ...hos-build-slot.exe", the wrapper is missing: `just build-slot` in `C:\Humanity`.
`scripts/lib-tests.js` and `just verify` rebuild a missing or stale wrapper on their own.

## Tests: never rerun a pass, and a flake costs one test, not the suite

`just test-lib` (scripts/lib-tests.js) and `just verify` apply the policy CLAUDE.md asks for:

- A FULL run that passes is recorded against the identity of the whole checkout (the commit's
  tree, every changed or untracked file's content, `rustc -V`, the features). A full run asked
  for on the same identity is refused, exit 0, naming when it passed. `--force` runs it anyway.
  A run with a filter is never refused.
- One character per test on the screen (the script turns the test program's line per test into
  its mark, so it knows which tests finished); `--verbose` gives every line.
- One to five failures are each run again, alone, by exact name. One that passes alone is
  reported FLAKY (a banner), written to `data/coordination/flaky-tests.jsonl` with its first
  failure's message, and does not fail the run. One that fails again is a REAL FAILURE. More
  than five failures are not timing, and none is rerun.
- A FLAKY line is a lead, not a verdict: "passes alone" is also how a bug that shows only under
  load looks. Of the four tests that failed under load on 2026-10-21, three waited on the clock
  and one was BUG-201, the file link losing datagrams on a loaded machine. A test that comes
  back to the ledger gets its cause found (run it many times beside a busy machine, before and
  after); the fix is a test that waits for the event it means, never a longer timeout.
- `--run-timeout-min N` limits the test RUN, not the build before it or a wait in the slot.
- **A run that prints nothing for 15 minutes is stopped and fails** (`--stall-min N`, 0 = never),
  naming the tests that were in flight. On 2026-11-01 the full run froze at 3,852 of 5,584 for two
  hours, burning CPU, without one "has been running for over 60 seconds" line. The silence is
  libtest's own: its main loop joins each finished test's thread BEFORE printing that test's
  result, and its 60-second warnings come from the same loop, so a thread that reports and then
  cannot exit stops every line after it (scripts/tests/fixtures/lib-tests/tiny_suite.rs makes
  that hang on purpose; plain libtest stays silent past 60 s). libtest starts tests in name order,
  so the suspects are the first unfinished ones, at most one per test thread.
- **Beside the operator's GPU game, the tests that open a GPU device stand down.** That freeze
  was beside helldivers2.exe, at the tests that open a real device (renderer::scene_target's
  round trips run in every full run), and the same run passed in 232 s once the game closed.
  Before a run, scripts/lib-tests.js asks scripts/lib/machine-guard.js which of his GPU-heavy
  programs run (scripts/lib/operator-apps.json, or his own HumanityOS); when one does, every test
  that would open a device asks `humanity_core::test_gpu::may_use()` (crates/humanity-core/src/test_gpu.rs) first and returns
  early, and the script names them in a GPU TESTS NOT RUN banner (libtest counts them as passed).
  The pass is recorded without them: the next full run on the same tree, once the game has
  closed, runs just those and then records the full pass. `--gpu-anyway` runs them beside it
  (they may hang the run). `tests/gpu_test_gate_lint.rs` (in `just lints` and CI) fails on a
  `wgpu::Instance` made with no `test_gpu::may_use()` above it in the same function. A renderer
  change verified beside his game is not verified on the GPU until that follow-up run: say so.

## Lints

`just lints` (scripts/run-lints.js) compiles each lint once per content (its source, `rustc -V`,
the checkout's path) into `target\lint-cache\`, runs four at a time, and runs EVERY lint: a red
one no longer hides the ones behind it. 32.7 s before, 7.4 s cold and 3.9 s warm after
(2026-10-23). The list in the Justfile is still the authority.

## The warnings ratchet (2026-11-01)

Our crates have **zero** rustc warnings, natively and in the relay build, and a change that adds one
fails `just verify` and CI. `scripts/warnings-ratchet.js native` (the first line of `just verify`)
and `scripts/warnings-ratchet.js relay` (`just verify-relay-compiles`, and CI's relay step) run
`cargo check --workspace --lib --bins --tests` for that feature set with `--message-format=json` added (not
part of cargo's fingerprint, so nothing rebuilds), count the warnings in OUR crates only, every workspace member's (each one
once: the library and its test build repeat them; vendor/ and other path packages are printed but
not counted), and compare with `scripts/warnings-budget.json` (0 and 0). Over the budget fails and
lists the kinds; under it asks for the budget to come down. A warning is fixed, never budgeted.

**It refuses to count through hidden warnings.** Before cargo runs it reads every place cargo takes
rustc flags from (RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, CARGO_BUILD_RUSTFLAGS, any
CARGO_TARGET_<triple>_RUSTFLAGS, `rustflags` in every `.cargo/config(.toml)` from the checkout up and
in CARGO_HOME) and the crate roots and Cargo.toml for a crate-wide allow; a lint allow (`-A...`,
`--allow`) or `--cap-lints` in any of them fails the run, saying the count cannot be read and how to
fix it. Why: until 2026-11-01 this PC's Windows user environment set `RUSTFLAGS=-Awarnings` (User
scope; nobody recorded who set it or why), so every cargo command printed no warnings, every
"0 warnings" report proved nothing, and 1,071 native and 258 relay warnings piled up (counted
2026-11-01 on 817b0396d). The operator removed it on 2026-11-01. If it, or anything like it, comes
back, `just verify` says so instead of passing. For one command: `RUSTFLAGS= just verify` in bash.

Its own checks: `scripts/tests/warnings-ratchet.test.js` (in `just rig-tests` and in CI). Seen red on
purpose when it landed: an unused import in a relay file failed both `just verify`'s native count
and CI's relay command, run locally.

What it costs: `just verify`'s first line used to check the library and the binary only; it now
checks the test targets too, which is where most of the thrown-away results were. After a one-file
touch, measured 2026-11-01 on a busy machine (the operator's game and another agent's builds
running): 55 s for the old `cargo check --features native`, 85 s for the new line. The release
build is not counted (it is not part of `just verify`); it printed one warning, in the vendored
rav1d, fixed the same day with upstream's own line.

## The workspace (2026-11-01, the crate split's rung 1)

The repo is a Cargo workspace since rung 1 of the crate split (`docs/design/crate-split.md`): the
root package `humanity-engine` (`src/`: the app, the server mode and the binary) and the crates cut
out of it so far, `crates/humanity-core/` and `crates/humanity-protocol/` (the root `Cargo.toml`'s
`[workspace] members` is the list). What that changed for builds and tests:

- **One version, one dependency list.** `[workspace.package]` holds the version every member
  carries (the app's: the core's own code reports it), as the file's first `version = ` line, which
  is where `just bump` and the release scripts read and write it. `[workspace.dependencies]` holds
  the version and features of every dependency more than one member uses, so no member can turn on
  a feature the root left off (the image crate's AVIF encoder, say).
- **Every member's lib tests.** Each member has a test program of its own (`humanity_core-<hash>`,
  `humanity_protocol-<hash>`, `humanity_engine-<hash>`). `scripts/lib-tests.js` builds them all in
  ONE `cargo test --no-run` (one feature unification: the core is compiled once per edit, and the
  programs compile side by side), runs each with the same arguments, adds up the counts, reruns a
  failure in the program it came from, keeps the pass ledger, the stall stop and the GPU stand-down
  per program, and fails when a member's program was not built (a script that took "the" program
  would run a third of the suite and call it green). `--crate <name>` builds and runs one member's.
  A test's name is its path in its own crate: the protocol's lost their `relay::core::` prefix.
  lib-tests runs every program from the repo root, as before; plain `cargo test` (`just
  verify-relay`) runs a member's in that member's folder, so a test that writes a path relative
  to the working directory lands in `crates/<name>/` (the core's boot-timing test made an
  untracked `crates/humanity-core/debug/` until its timer stopped writing under test). Tests
  name files from `env!("HUMANITY_REPO_ROOT")` or a `test_temp` folder, never from the cwd.
- **BUG-174 since then:** the engine's test program links WITH its debug file again (459 MB, it
  linked on 2026-11-01: line-tables-only debug info took the limit out of reach on 2026-10-24).
  Building the engine's program separately with `cargo rustc ... /DEBUG:NONE` meant a second cargo
  build for the other members with a feature unification of its own, which compiled about 160
  dependencies and the core a second time. That `cargo rustc` form is now the fallback, taken (and
  said) only when the link reports LNK1318.
- **The test helpers cross the boundary through `test-support`.** A crate's `#[cfg(test)]` items
  are never compiled into another crate's tests, so the core's `test_temp`, `test_clock`,
  `test_log`, `test_gpu`, the frame-cost store's reset and lock, and the `fixtures` modules (one
  beside each test module other crates' tests share: `storage::fixtures`, `dev_travel::fixtures`,
  `pq_crypto::fixtures`, `profile_copy::fixtures`, and the protocol's `strip_heif_metadata::build`)
  sit behind each crate's `test-support` feature, which a dependent turns on in its
  `[dev-dependencies]` only. Resolver 2 never unifies those into a product build.
- **The repo root has one name in the source: `env!("HUMANITY_REPO_ROOT")`,** set by each member's
  build script (the root's `build.rs`; `crates/repo_root.rs`, which every crate under `crates/`
  shares). In a workspace `CARGO_MANIFEST_DIR` is each member's own folder, and a `../` include
  path is relative to its file; `tests/repo_root_lint.rs` (in `just lints` and CI) fails on either.
  Not an `[env]` entry in `.cargo/config.toml`: that file is machine-local and lives in the main
  checkout only, so every worktree would have read the MAIN checkout's `data/` (silently, and
  green), and CI has none. tests/ and examples/ keep `CARGO_MANIFEST_DIR` (the root package's own
  targets; the std-only lints are compiled by rustc alone).
- **The checks cover every member.** `scripts/warnings-ratchet.js` checks `--workspace` and counts
  the warnings of every member's manifest (vendor/ is still printed, not counted); `just
  verify-relay` is `cargo test --workspace --features relay --no-default-features --lib`, so the
  battery runs the core's and the protocol's tests too; `just clippy` lints the workspace. A command
  that names no package (`cargo check --features native`, the release builds) builds the root
  package, which compiles the two crates as dependencies but not their tests.
- **The build slot wraps every member** (cargo's `rustc-workspace-wrapper` applies to each one; the
  slot's log shows `humanity_core` and `humanity_protocol` compiles admitted and timed since the
  first build). Since rung 1 its source names a member's compiles by the crate (`core-check-native`,
  `protocol-test-native`) so the ledger learns each one's peak apart from the engine's (`lib-...`)
  once `just build-slot` installs it from the main checkout; until then they are "other-...", whose
  learned peak (about 1.5 GB) is far above a core compile's (0.6 GB check).
- **The source stamp includes `crates/`** (build.rs `FINGERPRINT_INPUTS`), so the freshness gate
  every rig runs sees an edit to the core or the protocol; `scripts/lib/compiled-in.js` reads both
  crates' includes.

## Rigs wait for memory, not for "no builds"

A rig used to wait for every cargo, rustc and link on the machine to leave, which in a shared
checkout meant most of an hour. Since 2026-10-23 a rig waits for other games and rigs, the
operator's programs, the boot lock and free physical memory (6 GB, `HUMANITY_RIG_MIN_FREE_MB`),
not for builds; builds are held at below-normal priority by the share guard and, during a
capture, by `holdBuilds`. A run that measures timings waits for builds too: `probe-sweep
--wait-builds` (what `just perf-sweep` runs) and `boot-timing`. Every capture beside a build is
still marked contaminated, so perf-report never grades one.

## Disk

The hourly disk guard (`just disk-guard`) now deletes old builds of our crate in every target
folder first, even while builds run: cargo puts the version into every artifact's hash, so each
release left a full set behind (182 GB of the main checkout's target on 2026-10-23). Builds of
our crate untouched for 3 days go when six newer ones exist; dependencies are never touched.

## Cargo's parallelism

The slot's config also sets `jobs = 4`: four compiles at a time per cargo instead of one per
logical processor (twelve here). The dependency crates are not gated by the slot, and on
2026-10-23 eighty cargo and rustc processes left 690 MB of 32 GB free. `CARGO_BUILD_JOBS`
overrides it for one command.

## Measured 2026-10-24 (quiet machine, Rust 1.99.1, about 23 GB of RAM free throughout)

- **Debug info (adopted, 41e1e8b3f):** line tables only for our crate, none for the dependencies.
  The lib test build after a one-line edit: 81 s, 13.3 GB peak, 136 CPU s before; 56 s, 10.1 GB,
  68 CPU s after. Cold: 522 s and 14.7 GB before, 431 s and 11.5 GB after. Deps on disk: 3.8 to
  2.4 GB. The image crate lost its unused AVIF encoder in the same commit.
- **rust-lld (rejected):** the link step is 2.97 s against link.exe's 4.91 s (4.54 against 5.80
  with a debug file), about 2% of a build: under the operator's 5% bar for a change that would
  also mean trusting another linker for the release exe.
- **BUG-174 after the debug-info change:** link.exe linked the test program WITH its debug file
  (a 446 MB PDB) without LNK1318. The /DEBUG:NONE workaround in scripts/lib-tests.js stays (it is
  harmless and faster), but the limit is no longer reached.
- **The relay gate (adopted, relay-gate.js):** one `cargo check --features relay --no-default-features
  --lib --bins --tests` catches both failures the old pair caught (a relay test module reaching for
  native-only code, E0433; a relay-only error in the binary, E0308) and costs 21 s against the pair's
  50 s after a touch, 82 s against 372 s warm. `just verify` runs it once as
  `just verify-relay-compiles` in place of the pair.
- **Where one cold compile of our crate goes (time-passes, 241 s):** type checking 46 s and borrow
  checking 50 s (single-threaded inside one crate), code generation 105 s (37 s to LLVM IR, 54 s
  of LLVM passes, already spread across the CPUs), linking 5 s. So about 40% of every compile is
  analysis that one crate cannot run in parallel, and a one-line edit still re-checks the whole
  crate (56 s). Splitting the crate is what would cut that; it reverses the 2026-06-30 one-crate
  decision, so it is the operator's call (PRIORITIES).

## Measured 2026-11-01: the crate split's rung 1 (quiet machine, about 23 GB of RAM free)

`node scripts/build-ab/split-measure.js <label> <core file> <engine file>` times `cargo check`
and the lib test build, cold and after a one-line edit in the core and in the engine, from the
build slot's own lines; run it on main and on the branch, the same evening. Rung 1 changed
nothing worth the name: a one-line edit is 18 s of `cargo check` before and after, the test build
53 s before and 55 s after (the engine's test program is now linked with its debug file, 2 s),
cold 237 s and 242 s. The table and why: [crate-split.md](../design/crate-split.md), rung 1.

## Still to measure (scripted in scripts/build-ab/)

`relay-gate.js` and `seed-ab.js` below are still unmeasured. On 2026-10-23 the machine never had room to measure: at 12:45 fourteen compiles of our crate
from twelve worktrees held about 102 GB of commit (each 3 to 12 GB of private memory, each with
under 1 GB of it in physical memory), free physical memory was 0.6 GB and free commit 10.5 GB.
A timing then measures the paging. Run these once the slot is installed and the machine is quiet,
and report the load beside every number (the scripts log it):

```bash
node scripts/build-ab/ab-debug.js a     # debug info as Cargo.toml has it: cold + incremental
node scripts/build-ab/ab-debug.js b     # line-tables-only for our crate, none for dependencies
node scripts/build-ab/linker-ab.js b    # rust-lld against link.exe; BUG-174's PDB question
node scripts/build-ab/time-passes.js b  # analysis against code generation, one cold compile
node scripts/build-ab/relay-gate.js     # can one `cargo check --tests` replace verify's relay pair
node scripts/build-ab/seed-ab.js        # a fresh checkout seeded from a warm target, first build
```

What each decides: the dev-profile debug info (Cargo.toml, with the image crate's unused `avif`
feature, in one commit, so every target folder rebuilds once); the linker (adopted only if faster
and the release exe passes `just verify-runtime`); whether splitting the crate is worth its cost;
the relay gate of `just verify`; and whether `just seed-target` (scripts/seed-target.js, tested,
not yet a recipe) earns a recipe.

## sccache

sccache was 0.17.0 and wrapped nothing (no `RUSTC_WRAPPER` anywhere; its server had answered 0
requests, its cache held 47 MB from one try on 2026-10-22). On 2026-10-23 it was updated to
0.18.0 (`cargo +1.99.1 install sccache --locked`: 0.18 needs Rust 1.91 or newer, and the PC's
default `stable` toolchain outside the repo is still 1.88.0) and its cache raised to 30 GiB in its
own config file, `%APPDATA%\Mozilla\sccache\config\config` (`[cache.disk] size = 32212254720`;
`sccache --show-stats` reads back "Max cache size 30 GiB").

Measured on a two-crate test the same day: a dependency compiled in one checkout is a cache HIT
in a copy of that checkout at another path (a fresh worktree's case: 2 of 2 hits), but a MISS
when only `CARGO_TARGET_DIR` differs, because sccache hashes every `CARGO_*` variable. So wiring
it would turn a fresh worktree's 700 dependency compiles into cache reads, for every feature set,
without copying anything (scripts/seed-target.js's job, done better). It is NOT wired yet: that
is one line in the slot's config (`rustc-wrapper = "sccache"` under `[build]`, written by
scripts/build-slot.js), and it should first be measured on the real crate set with the machine
quiet (cold first build of a fresh worktree with and without it), and checked with the slot
nested inside it (cargo then runs `sccache hos-build-slot.exe rustc ...` for our crate, which
sccache passes through as incremental).
