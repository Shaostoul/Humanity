# Next Session Continuation Prompt

Read these docs in order before doing anything:

1. `docs/ENGINE_REFERENCE.md` -- Complete engine source of truth (modules, LOC, status, gaps)
2. `docs/STATUS.md` -- Feature completion inventory
3. `docs/BUGS.md` -- Known issues
4. `docs/SOP.md` -- Version and deploy procedures
5. `docs/design/multi-agent-development.md` -- Multi-agent architecture

## Current State (v0.88.0)

The native Rust desktop client has:
- **Working renderer:** wgpu PBR pipeline with Cook-Torrance GGX shader, dynamic uniform buffers (256 slots), reverse-Z depth
- **Working hologram:** 36 celestial bodies from RON data, orbit rings (128-segment tube torus), pin markers
- **Working ship:** Fibonacci spiral homestead with 9 rooms, 3m walls, ceilings
- **Working GUI:** egui with 30 pages, custom widgets (slider, checkbox), infinite-scroll settings, chat bubbles
- **Working systems:** 15 registered ECS systems (time, weather, farming, AI, quests, ecology, etc.)

## Priority Gaps

1. **Shadow mapping** -- No shadows at all. Need sun shadow pass.
2. **Particle system** -- Completely missing. Needed for exhaust, dust, rain, fire, explosions.
3. **Solar system navigation** -- Clicking hologram planet pins should transition to orbit view.
4. **Room-specific materials** -- All rooms use same default material. Assign per-room material_type.
5. **Combat/Economy/Logistics** -- Stub systems, need real implementation.
6. **Frustum culling** -- All objects rendered every frame regardless of visibility.

## Build & Test

```bash
cargo build -p humanity-engine --features native --release
cp target/release/humanity-engine.exe C:\Humanity\HumanityOS.exe
# Or just: cargo run -p humanity-engine --features native
```

## Multi-Agent Workflow

Use the Agent tool to spin up parallel domain agents. Each agent gets a self-contained task scoped to specific files. See `docs/design/multi-agent-development.md` for domain ownership rules.
