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
- The test program runs with `-q` (one character per test); `--verbose` gives the old output.
- One to five failures are each run again, alone, by exact name. One that passes alone is
  reported FLAKY (a banner), written to `data/coordination/flaky-tests.jsonl` with its first
  failure's message, and does not fail the run. One that fails again is a REAL FAILURE. More
  than five failures are not timing, and none is rerun.
- `--run-timeout-min N` limits the test RUN, not the build before it or a wait in the slot.

## Lints

`just lints` (scripts/run-lints.js) compiles each lint once per content (its source, `rustc -V`,
the checkout's path) into `target\lint-cache\`, runs four at a time, and runs EVERY lint: a red
one no longer hides the ones behind it. 32.7 s before, 7.4 s cold and 3.9 s warm after
(2026-10-23). The list in the Justfile is still the authority.

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

## Still to measure (scripted in scripts/build-ab/)

On 2026-10-23 the machine never had room to measure: at 12:45 fourteen compiles of our crate
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
