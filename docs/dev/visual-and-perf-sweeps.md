# 3D visual-regression + perf sweeps

The egui page snapshots (`just snapshots`, `tests/snapshots/*.png`) cover the 2D
UI. This tooling covers the **running 3D scene** - atmosphere, ocean, terrain,
lighting, vegetation - by driving the real game through a fixed set of vantages
and capturing a screenshot plus the live fps at each.

## Pieces

- `tests/visual/vantages.json` - the **golden spec**. Each vantage is a camera +
  time/sea, an `expect` (what a correct frame looks like), `regressions` (the
  specific failures it exists to catch, each has bitten a real release), and an
  advisory `perf_floor_fps`. Data-driven: add a vantage, both sweeps pick it up.
- `scripts/probe-sweep.js` - the **capture driver**. Sets up a portable rig
  (`.probe-rig/`, junctions to the repo's `data/` + `assets/` so it always
  exercises the current tree), boots `target/release/HumanityOS.exe`, enters the
  world, does a warm-up teleport (so the v0.930 async sky-sphere + terrain
  builds finish before capture), drives the tour, writes `manifest.json` +
  one PNG per vantage, and kills the game by the pid it spawned.
- `scripts/perf-report.js` - reads a manifest and prints the fps/frame-time
  table, flagging anything below its floor. Exit 2 if any is below floor or a
  panic occurred, so it can gate.
- `.claude/workflows/visual-sweep.js` - the **visual-regression workflow**: it
  runs the capture, then one AI judge per screenshot rules pass/fail/warn
  against that vantage's `expect` + `regressions`, then synthesizes a report.

## Running

    just perf-sweep                 # capture + fps table (deterministic, no AI)
    just probe-sweep                # capture only -> .probe-rig/sweeps/<ts>/
    just probe-sweep --only moon-surface-200m,limb-400km   # a subset
    just perf-diff <old>/manifest.json     # fps delta vs a baseline sweep

    # visual regression (needs an AI runner):
    Workflow({ scriptPath: ".claude/workflows/visual-sweep.js" })

A sweep takes ~3-4 min (boot + warm-up + 8 vantages). Output lands in
`.probe-rig/sweeps/<timestamp>/` (gitignored); `.probe-rig/latest-sweep.txt`
points at the newest, which the perf report and workflow read by default.

## When to run

- After any renderer / shader / terrain / lighting change, before shipping - the
  visual sweep catches the black-shell / zebra-ocean / bare-forest class of
  regression the loop kept finding by hand.
- `just perf-sweep` after a perf change to confirm the fix and check nothing
  else regressed across the flight envelope (it would have caught the v0.930
  departure hang systematically).

## Adding a vantage

Add an entry to `tests/visual/vantages.json`. `showcase.time` is the game clock
(`12 + lon/15` is roughly local noon at that longitude - the sun is
Greenwich-referenced); `sea` is 0 glassy .. 1 storm. `camera` takes the same
fields `debug/camera_request.json` accepts (`body`, `lat`, `lon`,
`altitude_km`, `look_offset_deg`, or `aim: "sun"`). Write an honest `expect`
and list the `regressions` the vantage guards against.

## Gotcha

The capture drives one shared game instance, so the fan-out is "one driver
captures, N judges analyze" - not N game instances. If the exe copy fails with
EBUSY, a previous probe still holds the file; kill stray `HumanityOS` processes
and retry. The sweep only kills the pid it spawned.
