---
name: game-checks
description: Choose and run the right HumanityOS runtime checks (boot and world entry, screens, storeys, falls, multiplayer flows, vantage screenshots, page snapshots, frame costs) without disturbing the operator's PC.
---

# Run the game checks

Static checks (`cargo check`, lints, lib tests) cannot see a boot failure, a world-entry panic or
a wrong picture. These can, but each one boots the real game, so they share one GPU with the
operator.

## Before any boot

- **One instance machine-wide.** `tasklist | grep -i humanityos` (the archive is named
  `v<version>_HumanityOS.exe`, so an exact-name filter misses it). The operator allows closing
  his game while developing: `taskkill //PID <pid>` without `//F`, force only after ~15 s.
- **His heavy games hold the rigs.** The machine guard waits while one listed in
  `scripts/lib/operator-apps.json` runs; do not work around it.
- **Fresh exe.** The rigs check the build is newer than the source and skip when it is not; build
  first: `CARGO_BUILD_JOBS=4 cargo build --features native --release`.
- **Never set `HUMANITY_TAKE_FOCUS`.** Script-launched boots open behind his windows and silent.
  Any relay a check starts binds 127.0.0.1.
- **Never run several rig-booting agents at once**: stagger them.

## Which check for what

| Changed | Run |
|---|---|
| Renderer, shaders, pipelines, bind groups, device, startup | `just verify-runtime` (boot, enter the world, 3 vantages, fails on PANIC) |
| In-world screens | `just verify-screens`, `just verify-live-screen` |
| Floors, stairs, ladders, elevators | `just verify-storeys` |
| Falling, landing, footing | `just verify-falls` |
| Home starts and choosing where you live | `just verify-starts` |
| Households, shared building, two players | `just verify-household`, `just verify-shared-build`, `just verify-copresence`, `just verify-second-player` |
| How a place looks | `just probe-sweep --only <vantage>` (vantages: `tests/visual/vantages.json`), then the `visual-sweep` workflow to judge |
| An egui page | `just snapshot <page>` or `just snapshots` (needs a GPU: a skipped page prints a note and the run still exits 0) |
| Frame time | `just perf-sweep`, then `just perf-diff <baseline>` |
| The rig scripts themselves | `just rig-tests` (pure node, no boot) |

## Reading the results

- A boot problem: `%APPDATA%/HumanityOS/logs/run.log` and `crash.log` first; the panic hook
  always writes the cause there.
- Screenshots are PNGs: open them with the Read tool and look. A check whose only evidence is its
  own setup code proves nothing; make a new gate fail on purpose once before trusting it.
- Verify on the state a player actually reaches (flying there, not teleporting, when that is how
  players arrive).
- Probe-sweep cost files are stale defaults unless performance capture was armed: two vantages
  reporting the same number is the tell.
