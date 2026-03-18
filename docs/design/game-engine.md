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

## Recommendation: Hybrid approach — wgpu + cherry-picked Bevy crates

Don't buy into full Bevy. Don't build everything from scratch. Take the middle path:

### Phase 1: Foundation (Months 1-3)

Use **wgpu** directly for rendering. Pull in individual Bevy crates without the full engine:

- **bevy_ecs** — The ECS is Bevy's best piece. Use it standalone for game state, entity management, and system scheduling. It compiles independently.
- **bevy_asset** — Asset loading, hot-reload, handle-based references. No reason to rewrite this.
- **bevy_input** — Unified input handling across platforms.

Build your own:
- **Renderer** on wgpu, designed from the start for the Tauri webview overlay architecture. You control the swap chain, the render passes, and how the 3D viewport composites with the HTML layer.
- **Window management** via raw-window-handle, integrated with Tauri's window.

### Phase 2: Game systems (Months 3-6)

Add crates as needed:
- **Rapier** for physics (works standalone, doesn't need Bevy).
- **kira** for audio (ditto).
- **glam** for math (Bevy uses this internally anyway).
- **gltf** crate for model loading.

Evaluate at this point: Is the custom renderer worth maintaining, or has Bevy's renderer caught up enough to switch?

### Phase 3: Decision point (Month 6)

Two paths forward:

**Path A — Stay custom.** The hybrid renderer works well, you understand every line, and Bevy's renderer still doesn't support the overlay architecture cleanly. Keep building.

**Path B — Graduate to full Bevy.** The custom renderer is becoming a maintenance burden, Bevy's API has stabilized (0.16+?), and someone has solved the Tauri integration. Migrate to full Bevy, keeping the ECS code you've already written (since it's bevy_ecs anyway).

Either path is a valid outcome. The Phase 1-2 work is not wasted in either case because bevy_ecs code is bevy_ecs code whether you run it inside full Bevy or standalone.

### Why this works for HumanityOS specifically

1. **The hybrid UI is the hardest problem.** No engine solves it out of the box. A custom wgpu renderer lets you design the composition layer (3D world + HTML overlay) without fighting an engine's assumptions.
2. **bevy_ecs is the best Rust ECS.** Using it standalone gives you the architectural pattern without the renderer lock-in.
3. **Compile times stay manageable.** Cherry-picked crates compile faster than full Bevy. Incremental builds stay under 10 seconds.
4. **You keep optionality.** If Bevy hits 1.0 with a great Tauri story, migration is straightforward. If not, you haven't painted yourself into a corner.

### Risks of this approach

- **Integration tax.** Making standalone Bevy crates play together without the full engine requires some glue code. Not hard, but not zero.
- **Version drift.** If you pin bevy_ecs to 0.15 and Bevy ships 0.16 with breaking ECS changes, you have to decide when to upgrade.
- **Less community help.** "I'm using bevy_ecs standalone with wgpu" is a less common setup than "I'm using Bevy." Fewer blog posts, fewer examples.

These risks are manageable and preferable to the risks of full engine commitment (either Bevy's churn or custom engine's time sink).

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
