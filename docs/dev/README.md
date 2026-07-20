# docs/dev/ - Content Pipelines and Dev Instrumentation

How to ADD things to HumanityOS (sounds, models, shaders, data, planets, UI)
and how to measure the running game. Every guide is grounded in the actual
code with real file paths; when a guide and the code disagree, the code wins
and the guide needs a fix. House rules: plain language for three audiences
(operator, contributors, AI agents), no em dashes, cross-link instead of
duplicating.

For orientation-level docs (architecture, module map, the dev loop) see
[docs/contributor/](../contributor/); for design rules see
[docs/design/](../design/).

## The guides

| Guide | One line |
|-------|----------|
| [adding-sounds-music.md](adding-sounds-music.md) | Audio: the honest not-yet-wired state of AudioManager/SoundCatalog, asset + licensing conventions, and the checklist for the first real game sound. |
| [adding-3d-models.md](adding-3d-models.md) | glTF pipeline: the single-mesh single-primitive loader, the repack/split script, type-19 textured rendering, decorations.ron scatter, triangle budgets. |
| [adding-shaders.md](adding-shaders.md) | The pbr_simple.wgsl megashader: material types 0-19, group-3 bindings, adding a type, naga/FXC gotchas, the boot-the-exe verify bar. |
| [adding-game-data.md](adding-game-data.md) | The data-driven content system: data/ + schemas/, validate-data, hot reload, id conventions, and a worked add-a-plant example. |
| [adding-planets-celestial.md](adding-planets-celestial.md) | Celestial bodies: sol.json catalog, PlanetDef surfaces, the HOSALB1 albedo + heightmap pipelines, gas-giant shader params. |
| [adding-ui-pages.md](adding-ui-pages.md) | Native egui pages + web mirrors: registration chain, theme tokens, the six lints, PAGES.md, headless snapshots. |
| [performance-profiling.md](performance-profiling.md) | Overlays (F2/F3/F4), the debug/ file protocol, the portable perf probe, ChunkDiag, where the frame budget goes. |
| [updater-testing.md](updater-testing.md) | Pre-release checklist for the auto-updater flow. |

Dated findings from specific profiling/debugging sessions also land in this
folder as `performance-findings-<date>.md` notes.
