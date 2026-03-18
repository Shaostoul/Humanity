# Game Engine Decision — HumanityOS

**Status:** Proposed
**Author:** Shaostoul + Claude
**Date:** 2026-03-17
**Affects:** Game client, desktop app (Tauri), rendering pipeline, multiplayer architecture

---

## Context

HumanityOS needs a 3D game layer that coexists with the existing Tauri/WebView2 desktop shell. The game world will render natively (GPU-accelerated) while UI overlays (chat, inventory, settings) remain in the HTML/JS layer already built. This hybrid architecture is the primary constraint — most off-the-shelf engines assume they own the entire window.

The relay server is Rust/axum/tokio. Staying in Rust for the game client means shared types, shared tooling, and one language across the stack.

## Requirements

| Requirement | Priority | Notes |
|---|---|---|
| Rust-native or Rust-compatible | Must | Same language as relay server |
| Cross-platform (Win/Mac/Linux) | Must | WASM nice-to-have for web demo |
| Coexist with Tauri WebView2 overlay | Must | HTML UI on top of 3D world |
| Multiplayer networking | Must | Already have WebSocket relay |
| Reasonable compile times for iteration | Should | Full rebuild under 10 min |
| Asset hot-reload during development | Should | Textures, models, shaders |
| MIT/Apache or similarly permissive license | Must | Open-source project |
| 3D renderer (PBR, shadows, particles) | Must | Not AAA, but good enough |
| Physics (rigid body, raycasting) | Should | Can add via Rapier crate |
| One-person maintainable | Must | Small team reality |

---

## Option A: Bevy (full engine)

### Pros

- **Rust-native.** Same language as the relay server. Shared types between client and server with no FFI boundary.
- **ECS architecture.** Data-oriented design is cache-friendly and scales well for large worlds with many entities. Systems compose cleanly.
- **Active community.** ~17k GitHub stars, regular release cadence, active Discord. Problems get answered quickly.
- **Plugin ecosystem.** bevy_rapier (physics), bevy_egui (debug UI), bevy_kira_audio, bevy_renet (networking) — most common game subsystems have community crates.
- **MIT/Apache dual-licensed.** No concerns for an open-source project.
- **Hot-reloading assets.** Built-in asset server watches for file changes during development.
- **Cross-platform.** Windows, macOS, Linux, and experimental WASM support.

### Cons

- **Pre-1.0 with breaking changes every release.** Currently at 0.15 (early 2026). Every minor version changes APIs. Migration guides exist but cost real time.
- **Opinionated ECS.** Everything is a system, component, or resource. If the problem doesn't fit ECS naturally, you fight the engine. UI, sequential game logic, and state machines feel awkward.
- **Heavy compile times.** Clean build: 5-8 minutes. Incremental: 15-40 seconds. Dynamic linking helps during dev but adds complexity.
- **Learning curve.** ECS thinking is genuinely different from OOP. Queries, system ordering, change detection, and the scheduler have non-obvious behavior.
- **Dependent on upstream release cycle.** If you need a renderer feature or a bug fix, you wait for the next release or vendor a fork.
- **Renderer is good, not great.** PBR, shadows, bloom, SSAO — solid. But no built-in ray tracing pipeline, and advanced effects require custom render graph nodes.

---

## Option B: Custom engine on wgpu

### Pros

- **Total control.** Build exactly the rendering pipeline the hybrid architecture needs. No fighting an engine that assumes it owns the window.
- **No upstream API churn.** wgpu itself is relatively stable (follows WebGPU spec). Your code breaks only when you change it.
- **Deep GPU knowledge.** You understand every draw call. Debugging is "read your own code" not "read engine internals."
- **Purpose-built for HumanityOS.** The Tauri overlay + native 3D world hybrid is unusual. A custom renderer can be designed around it from day one.
- **Lighter binary.** No unused subsystems. Ship only what you use.

### Cons

- **Massive time investment.** A usable engine (renderer + scene graph + asset pipeline + animation + audio + input + physics) is years of work for one person. Most solo engines never ship a game.
- **Every problem is your problem.** Frustum culling, skeletal animation, particle systems, LOD, texture streaming, spatial audio mixing — you build or integrate each one.
- **wgpu gives you GPU access, nothing above it.** No scene graph, no material system, no ECS, no asset loading. You start from raw triangles.
- **No community.** When you hit a rendering bug at 2 AM, there's no Discord to ask. Stack Overflow has sparse wgpu coverage.
- **Maintenance burden compounds.** Every subsystem you build is a subsystem you maintain forever.

---

## Option C: Fyrox

A more traditional Rust game engine with a scene editor (FyroxEd), node-based scene graph, and built-in renderer.

| Aspect | Assessment |
|---|---|
| Architecture | Traditional scene graph + nodes (like Godot). Familiar if you've used Unity/Godot. |
| Renderer | More mature than Bevy's. Deferred rendering, GI probes, multi-render-target. |
| Community | Much smaller (~7k stars). Primarily one developer (mrDIMAS). |
| Stability | More stable APIs than Bevy, but smaller team means slower bug fixes. |
| Editor | FyroxEd is functional — scene editing, material inspector, animation editor. |
| Risk | Bus factor of 1. If the maintainer steps away, the engine stalls. |

**Verdict:** Stronger renderer and editor than Bevy, but the tiny community is a real risk for a long-term project.

---

## Option D: godot-rust (Godot 4 + GDExtension)

Use Godot 4 as the game engine with Rust bindings via gdext.

| Aspect | Assessment |
|---|---|
| Editor | Godot's editor is excellent — scene tree, visual shader editor, animation tools. |
| Renderer | Vulkan-based Forward+ and Mobile renderers. Good quality, actively improving. |
| Rust integration | GDExtension bindings work but add friction. You write Rust that conforms to Godot's object model, not native Rust patterns. |
| Dependencies | Introduces GDScript and C++ into the stack. Build pipeline gets more complex. |
| Hybrid UI | Godot assumes it owns the window. Integrating with Tauri overlay is non-trivial — would likely need to embed Godot as a child window. |
| License | MIT. No concerns. |

**Verdict:** Best editor and most mature 3D of the Rust-compatible options. But the GDExtension boundary adds friction, and the Tauri hybrid architecture becomes harder, not easier.

---

## Option E: Ambient

Rust ECS engine designed for multiplayer from the ground up. Runs game logic as WASM modules on a shared server.

| Aspect | Assessment |
|---|---|
| Multiplayer | Built-in. Server-authoritative, client-predicted. Impressive architecture. |
| Maturity | Very early. API changes frequently. Documentation is sparse. |
| Community | Small. Backed by a startup that may or may not sustain funding. |
| WASM modules | Interesting for modding/UGC but adds complexity. |

**Verdict:** Architecturally interesting for an MMO-style game, but too immature and risky to bet on today.

---

## Option F: Unreal/Unity with Rust FFI

Use an established AAA engine and call Rust for game logic via C FFI.

| Aspect | Assessment |
|---|---|
| Renderer | Best-in-class (Nanite, Lumen, HDRP). Far beyond what any Rust engine offers. |
| Tooling | World-class editors, profilers, debuggers. |
| Rust integration | Painful. FFI boundary for every game logic call. Two build systems. Two languages. |
| License | Unreal: royalties above $1M revenue. Unity: per-install fees (controversial). Neither is truly open-source friendly. |
| Binary size | Unreal: 100MB+ minimum. Unity: 50MB+. Heavy for a cooperative platform. |

**Verdict:** Overkill and misaligned. Licensing concerns, massive binaries, and the FFI boundary defeats the purpose of using Rust.

---

## Decision Matrix

Scored 1-5 (5 = best). Weighted by priority.

| Criterion | Weight | Bevy | Custom (wgpu) | Fyrox | godot-rust | Ambient | Unreal/Unity |
|---|---|---|---|---|---|---|---|
| Rust-native | 5 | 5 | 5 | 5 | 3 | 5 | 1 |
| Hybrid UI compatibility | 5 | 3 | 5 | 3 | 2 | 2 | 2 |
| One-person maintainable | 5 | 4 | 2 | 3 | 4 | 2 | 4 |
| Community / ecosystem | 4 | 5 | 1 | 2 | 4 | 1 | 5 |
| API stability | 4 | 2 | 5 | 3 | 3 | 1 | 4 |
| Renderer quality | 3 | 3 | 2 | 4 | 4 | 2 | 5 |
| Compile times | 3 | 2 | 3 | 3 | 3 | 2 | 4 |
| Cross-platform | 3 | 5 | 4 | 4 | 5 | 3 | 4 |
| License | 3 | 5 | 5 | 5 | 5 | 4 | 2 |
| Learning curve | 2 | 3 | 2 | 4 | 4 | 2 | 3 |
| **Weighted total** | | **138** | **123** | **125** | **127** | **89** | **121** |

Bevy scores highest overall, but its weak spots (hybrid UI, API churn) align with the project's hardest requirements. Custom scores lower overall but highest on the two "must" criteria that matter most for HumanityOS specifically.

---

## Decision: Custom engine on wgpu — no Bevy dependency

**Status: DECIDED (2026-03-17)**

Build a fully custom game engine on wgpu. No Bevy, no half-measures.

### Rationale

1. **AI-accelerated development changes the calculus.** The "years of solo work" argument against custom engines assumes human typing speed and single-threaded thinking. With Claude spinning parallel agents across dozens of files simultaneously, the custom engine becomes viable in days/weeks, not years.
2. **Zero upstream dependency risk.** Bevy is pre-1.0 with breaking changes every release. A custom engine means we never get stuck behind someone else's API churn, deprecation cycle, or architectural decisions that don't fit our use case.
3. **No bloat.** We build exactly what we need — nothing more. No unused subsystems, no framework overhead, no fighting an engine's assumptions about window ownership.
4. **The hybrid UI is our unique constraint.** Tauri webview overlay + native GPU window is not a pattern any existing engine supports. We'd be fighting Bevy's renderer to make this work. Building our own means the composition layer is designed for this from day one.
5. **Full understanding.** Every line of engine code is code we wrote and understand. No black boxes, no "it works but I don't know why."

### Architecture

```
┌─────────────────────────────────────────────┐
│ HumanityOS Desktop App (Tauri)              │
├─────────────┬───────────────────────────────┤
│ HTML/JS UI  │ Native GPU Window             │
│ (WebView2)  │ (wgpu renderer)               │
│             │                               │
│ Chat, menus │ 3D world, terrain, entities   │
│ Settings    │ PBR materials, RT reflections  │
│ Inventory   │ Particles, weather, sky       │
│ HUD overlay │ Physics (Rapier)              │
├─────────────┴───────────────────────────────┤
│ Shared Layer (Tauri IPC)                    │
│ Game state, input routing, audio (kira)     │
├─────────────────────────────────────────────┤
│ Relay Server (Rust/axum) — multiplayer sync │
└─────────────────────────────────────────────┘
```

### Crate stack (standalone, no engine framework)

| Layer | Crate | Purpose |
|-------|-------|---------|
| GPU | **wgpu** | Cross-platform GPU (Vulkan/DX12/Metal), RT via extensions |
| Math | **glam** | Fast SIMD vec/mat/quat |
| Physics | **rapier3d** | Rigid body, collision, raycasting |
| Audio | **kira** | Lock-free mixing, 256+ voices, spatial |
| Spatial audio | **steam-audio** (FFI) | HRTF, occlusion, reverb |
| Models | **gltf** | glTF 2.0 loading |
| Images | **image** | Texture loading (PNG, JPEG, HDR) |
| Windowing | **winit** | Window creation, input events |
| VR | **openxr** | Headset rendering, tracking |
| ECS | **Custom** | Simple archetypal ECS tailored to our needs |
| Shaders | **WGSL** | Hand-written, no transpilation |

### Why custom ECS instead of bevy_ecs

- bevy_ecs pulls in 15+ transitive deps and has breaking changes each Bevy release
- Our ECS needs are straightforward: entities, components, system scheduling
- A minimal archetypal ECS is ~500 lines of Rust — Claude can write it in one pass
- No version drift, no integration glue, no "standalone bevy_ecs" gotchas

### Build phases

**Phase 1 — Window + Triangle (Week 1):**
Tauri app spawns a native wgpu window alongside the webview. Render a triangle with PBR. Prove the dual-window IPC works.

**Phase 2 — Scene graph + ECS (Week 2):**
Custom ECS, transform hierarchy, camera, mesh rendering, glTF loading. Render a textured model.

**Phase 3 — World systems (Weeks 3-4):**
Terrain, sky/atmosphere, day-night cycle, basic physics (Rapier), player controller. Audio via kira.

**Phase 4 — RT + polish (Weeks 5-6):**
Deferred rendering pipeline, hardware raytraced reflections/AO/GI, PBR materials, particle system, post-processing.

**Phase 5 — Multiplayer + VR (Weeks 7-8):**
Entity sync via existing WebSocket relay, OpenXR integration for VR headsets.

### Risks

- **Scope creep.** Engines are rabbit holes. Mitigate by building only what the game needs, not a general-purpose engine.
- **Shader complexity.** PBR + RT + deferred requires serious WGSL. Mitigate by starting with reference implementations and iterating.
- **Testing.** GPU code is hard to unit test. Mitigate with visual regression tests (screenshot comparison).

These risks are significantly lower than Bevy's risks (API churn, renderer lock-in, bloat) given the AI-accelerated development model.

---

## Procedural-First Asset Philosophy

The engine prioritizes procedural generation over pre-baked assets to minimize install size and maximize scalability.

### Materials and textures

- **Procedurally generated via WGSL shaders.** Base materials (metal, wood, stone, soil, fabric, etc.) are created at runtime using noise functions (Perlin, Simplex, Worley, FBM) combined with PBR parameters (albedo, roughness, metallic, normal, AO).
- **Tiny install footprint.** No large DDS/KTX2 texture packs are needed for base materials. A few kilobytes of shader code replaces hundreds of megabytes of pre-baked textures.
- **Hand-painted assets only where procedural falls short.** UI art, concept art, photographs, and specific artistic elements that require a human touch are stored as traditional image files. Everything else is generated.

### Audio

| Format | Use case | Notes |
|--------|----------|-------|
| **OGG Vorbis** | SFX, dialog | Lossy, small files, good quality for short clips |
| **OGG / FLAC** | Music, ambient | OGG for streaming, FLAC for lossless archival |
| **WAV** | Development only | Never shipped in release builds — converted to OGG before packaging |

### Models

- **GLB (binary glTF)** is the standard model format. Single-file, includes mesh, materials, animations, and metadata.
- glTF JSON (`.gltf` + separate `.bin`) is acceptable during development but should be packed to GLB for shipping.

### Scalable quality tiers

The procedural approach naturally supports hardware scaling:

| Tier | Target hardware | Adjustments |
|------|----------------|-------------|
| **High** | Modern GPU (RTX 3060+) | Full PBR, raytraced reflections/AO/GI, high-res procedural textures (2048+), full particle budget |
| **Medium** | Mid-range GPU (GTX 1060 / RX 580) | PBR without raytracing, screen-space reflections, medium-res procedural textures (1024), reduced particles |
| **Low** | Integrated / older GPU | Simplified shaders, low-res procedural textures (512), minimal particles, baked lighting fallbacks |

- **Older hardware skips raytracing** and uses simpler shader variants with fewer noise octaves and lower-resolution outputs.
- **Procedural texture resolution scales** with a single quality parameter — the same shader produces 512x512 on low-end and 2048x2048 on high-end hardware.

### Asset categories for selective install

Asset categories can be toggled or excluded entirely to reduce install size on storage-constrained or older hardware:

| Category | Contents | Excludable? |
|----------|----------|-------------|
| **Core** | Engine shaders, base procedural materials, essential UI | No — required |
| **Audio SFX** | Sound effects, UI sounds | Partial — can use reduced set |
| **Music** | Soundtrack, ambient tracks | Yes — gameplay unaffected |
| **Voice** | Dialog, narration | Yes — subtitles as fallback |
| **High-res models** | Detailed GLB models for close-up viewing | Yes — use LOD0 only |
| **Cinematic assets** | Intro sequences, cutscene-specific assets | Yes — skip or use lower quality |

This modular approach means the base install can be kept small (sub-100 MB for core + procedural shaders), with optional packs downloaded as needed.

---

## Appendix: Crate versions (as of 2026-03-17)

| Crate | Version | Purpose |
|---|---|---|
| wgpu | 24.x | GPU abstraction (WebGPU/Vulkan/DX12/Metal) |
| bevy_ecs | 0.15.x | Entity-Component-System |
| bevy_asset | 0.15.x | Asset loading + hot-reload |
| bevy_input | 0.15.x | Keyboard/mouse/gamepad input |
| rapier3d | 0.22.x | Physics engine |
| kira | 0.9.x | Audio engine |
| glam | 0.29.x | Linear algebra (vec3, mat4, quat) |
| gltf | 1.4.x | glTF model loading |
| raw-window-handle | 0.6.x | Cross-platform window handle |

---

## References

- [Bevy Engine](https://bevyengine.org/) — Main site, examples, migration guides
- [wgpu](https://wgpu.rs/) — GPU abstraction layer docs
- [Fyrox](https://fyrox.rs/) — Alternative Rust engine
- [gdext (godot-rust)](https://github.com/godot-rust/gdext) — Godot 4 Rust bindings
- [Rapier physics](https://rapier.rs/) — Standalone Rust physics
- [Learn wgpu](https://sotrh.github.io/learn-wgpu/) — Tutorial for building a renderer on wgpu
