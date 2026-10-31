# humanity-renderer

## What it is for

The GPU side: everything that uploads and draws. It sets up the wgpu device and the pipelines
(`mod.rs`, `pipeline.rs`), assembles the PBR shader from its numbered parts (`shader_loader`), and
draws the sky, atmosphere, clouds, sun and stars, the water, the terrain's patches, trees and grass,
the ship's rooms with their lights and bounced light (`room_probes`, `light_tiles`), the shadows and
the passes after (bloom, SSAO, god rays); `assets` loads the models and textures it draws. It turns
what terrain, ship and the simulation describe into pixels, and it is the code whose failures a
static check cannot see.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - no gate (every build): `renderer/`, `assets/`
- **Size:** 65,877 lines in 67 files, 1,346 of them in 2 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `renderer/tree_cards.rs` 450 of 450, `renderer/celestial.rs` 1,949 of 1,950, `renderer/mod.rs` 2,794 of 2,800, `renderer/capture.rs` 299 of 300, `renderer/tree_species.rs` 743 of 750, `renderer/tree_allometry.rs` 537 of 560, `renderer/tree_mesh.rs` 5,945 of 6,400, `renderer/scene_draw.rs` 619 of 750, `renderer/materials.rs` 409 of 500, `renderer/overlay_draw.rs` 390 of 550, `renderer/material_bind_groups.rs` 232 of 350, `renderer/view_depth.rs` 135 of 250, `renderer/surface.rs` 237 of 500, `renderer/shadow_cutout.rs` 101 of 250.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 53, `wasm` 1.
- **May use** (the plan): core, terrain. **Uses today** (references in non-test code): core 61, terrain 33.
- **May be used by** (the plan): gui, engine. **Used today by:** engine 310, gui 28.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 1 in test code, to systems 1: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 499 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib renderer:: assets:: --skip relay::storage::assets::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `renderer::line::push_polyline` | 32 | engine 32 |
| `renderer::camera::CameraMode` | 30 | engine 30 |
| `renderer::mesh::Mesh` | 19 | engine 19 |
| `renderer::RenderObject` | 16 | engine 16 |
| `renderer::line::push_circle` | 13 | engine 13 |
| `renderer::line::LineVertex` | 12 | engine 12 |
| `renderer::light::RoomLight` | 10 | engine 10 |
| `renderer::Renderer` | 9 | engine 8, gui 1 |
| `renderer::light::light_type` | 9 | engine 6, gui 3 |
| `renderer::camera::Camera` | 8 | engine 7, gui 1 |
| `renderer::light::LightKind` | 7 | engine 5, gui 2 |
| `renderer::stars::StarCatalogTier` | 7 | gui 4, engine 3 |
| `renderer` | 6 | engine 6 |
| `renderer::billboard_bake::BakeCpuModel` | 6 | engine 6 |
| `assets::AssetManager` | 5 | engine 5 |

And 91 more items, 149 references (`node scripts/crate-cards.js --interface humanity-renderer` lists them all).
<!-- end generated -->

## Rules

- **Verify every renderer, pipeline, bind-group or shader change by booting the release exe and
  entering the world: `just verify-runtime`.** Static checks missed both an unbootable device-limit
  rejection (v0.782 to v0.784) and a world-entry panic (ten releases). When the operator reports a
  boot failure, read `%APPDATA%/HumanityOS/logs/run.log` and `crash.log` first. Source:
  [renderer.md](../ai/renderer.md), "Verify by booting and entering the world".
- **The PBR megashader is six class entries, one per pipeline, and no `fs_main`.** A heavyweight
  branch gets its own `override` switch, lives in one class entry, is off in every pipeline that
  cannot draw it, and its switch is the first operand of its guard. Source:
  [renderer.md](../ai/renderer.md), "The megashader's class entries".
- **The PBR shader is numbered files under `assets/shaders/pbr/`;** the list and its order are
  `shader_loader.rs`, and `90-fragment-main.wgsl` is the shared tail: more than two or three agents
  on the same shader branch is a correctness hazard, not a merge chore. Source:
  [renderer.md](../ai/renderer.md), "The PBR shader files".
- **f32 at planet scale is a defect class:** f64 on the CPU to the sampler, small magnitudes near
  the camera on the GPU. Source: [renderer.md](../ai/renderer.md), "f32 at planet scale".
- **Each `create_bind_group` gives its layout exactly the layout's bindings, and every scene
  pipeline is built for the scene format** (the HDR target), never the display's. Source:
  [bind_group_entry_lint.rs](../../tests/bind_group_entry_lint.rs) and
  [scene_format_lint.rs](../../tests/scene_format_lint.rs), in `just lints`.
- **No hardcoded colours:** `theme_token_lint` covers `src/renderer/` too; a new colour is a token in
  `data/gui/theme.ron` with an accessor in `theme.rs`, never an entry in `LEGACY_OFFENDERS`. Source:
  [CLAUDE.md](../../CLAUDE.md), "One theme source".
- **Build the 2030 technique, and state its budget.** Each effect tier states its per-frame
  milliseconds and its network footprint before it is built; cosmetic effects run locally and are
  never networked. Source: [CLAUDE.md](../../CLAUDE.md), "Realistic-first" and "Dual modes, open
  source, and a compute budget".
- **No shader, pipeline or baked-asset caches across runs:** make the cold path fast (the v0.865 DXC
  swap, the v0.866 background prebuild). Source: [CLAUDE.md](../../CLAUDE.md), "No caches kept
  across runs during development".
- **One booted game machine-wide, in the background;** a rig that judges the picture or times
  frames waits for the operator's GPU-heavy programs. Source: [CLAUDE.md](../../CLAUDE.md), "The
  operator's machine", and [game-boots.md](../ai/game-boots.md).

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance (`scene_target_tests.rs` and the trees'
  card gates in `tree_mesh.rs` among them).
- **Boot:** `just verify-runtime` (about 3 minutes; `--operator-config` at the operator's live
  graphics settings, `--full-graphics` before a release).
- **See and time it:** `just probe-sweep` (every canonical vantage, a screenshot and its frame
  rate), `just perf-sweep` then `just perf-diff <old manifest>`, `just terminator-grain` (the cloud
  grain at the dusk line), the `visual-sweep` skill (captures judged against golden specs), and
  `just verify-screens` (the wall screens drawn and clickable).
- **`just lints`:** the bind-group, scene-format and theme-token lints.
- **Node tests in `just rig-tests`:** [terminator-grain.test.js](../../scripts/tests/terminator-grain.test.js),
  [perf-report.test.js](../../scripts/tests/perf-report.test.js) (a contaminated capture is never
  graded), [dxc-dlls.test.js](../../scripts/tests/dxc-dlls.test.js) (the shader compiler's dlls
  beside the exe, BUG-204).

## Open work

- **BUG-079, reopened:** clouds from orbit drawn as black-and-white static; the speckle near the limb
  and on some coasts is still open. Source: [BUGS.md](../BUGS.md).
- **BUG-141, open:** the drawn sky stays black through nautical twilight. Source: [BUGS.md](../BUGS.md).
- **BUG-153's last part:** the campfire gives no light, because point lights are not evaluated in
  the celestial pass, where a planet's ground and what is built on it are drawn (its light count is 0
  by design since v0.1155, `80-fragment-shared.wgsl`). Source: [BUGS.md](../BUGS.md).
- **BUG-050's open gap:** no mechanical check that a buffer struct shared by Rust and WGSL has the
  same layout on both sides. Source: [BUGS.md](../BUGS.md).
- **BUG-131, mitigated:** a lib-test run that hung for hours; the cause is unproven, and the one
  test past 60 s was `renderer::billboard_bake::tests::cluster_sprites_carry_real_per_leaf_colour_variation`.
  Source: [BUGS.md](../BUGS.md).
- **TIER 0's visual arcs:** environment regions (disasters through the region buffer, the cloud
  bake gap), the clouds, the dusk-line grain (the operator's call), the rooms' bounced light (next:
  probe streaming) and the far-rung cloud gates G0(d) and G1 to G7. Source:
  [PRIORITIES.md](../PRIORITIES.md), TIER 0 sections 1, 2, 2a, 3c and 4, and
  [cloud-far-rung.md](../design/cloud-far-rung.md).
- **Upstairs, slab occlusion costs +0.53 ms at 2560x1387.** Source: [PRIORITIES.md](../PRIORITIES.md),
  "Increment 7 follow-ups", and [room-gi.md](../design/room-gi.md).
- **Rung 3 cuts the renderer** with relay and net. Source: [crate-split.md](../design/crate-split.md).

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
