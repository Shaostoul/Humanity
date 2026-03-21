# Rust Engine to WASM Architecture

Compile the existing Rust game engine (wgpu) to WebAssembly so the same engine runs in both the desktop app (native) and the browser (WASM + WebGPU). One codebase, one renderer, one set of shaders.

## Why Not Three.js / JavaScript

- **One codebase**: every feature, every shader, every system is written once in Rust and runs everywhere. No parallel JavaScript implementation to maintain.
- **No GC pauses**: WASM has deterministic memory management. No garbage collector stalling the render loop mid-frame.
- **Multithreaded**: Web Workers + SharedArrayBuffer enable parallel ECS systems, physics, and asset loading — same threading model as desktop.
- **Same performance profile**: desktop and browser builds share identical hot paths. Perf optimizations benefit both targets.
- **No throwaway code**: everything built contributes to the final product. A Three.js prototype would be discarded once the real engine ships.

## Architecture

```
engine/src/              Shared Rust code (renderer, ECS, physics, systems)
  |
  | cargo build --target wasm32-unknown-unknown
  v
engine-wasm/             WASM-specific entry point + JS glue
  src/lib.rs             wasm-bindgen entry, event loop, canvas binding
  pkg/                   Generated: engine_wasm_bg.wasm + engine_wasm.js
  index.html             Test harness

Desktop: engine runs natively via Tauri (winit window, native wgpu)
Browser: engine runs as WASM, renders to <canvas> via WebGPU
```

The shared `engine/src/` code has zero platform-specific imports. All platform differences are handled by a `Platform` trait with native and web implementations.

## Key Crates

| Crate | Role |
|-------|------|
| `wgpu` | Already used in the engine. Compiles to WebGPU on WASM, Vulkan/Metal/DX12 on desktop. |
| `wasm-bindgen` | Rust-to-JS interop. Exports functions callable from JS, imports browser APIs. |
| `web-sys` | Typed bindings to DOM APIs: canvas, keyboard/mouse events, requestAnimationFrame. |
| `winit` | Window and event abstraction. Supports web target (maps to canvas events). |
| `wasm-pack` | Build tool. Compiles Rust to WASM, generates JS bindings and TypeScript types. |
| `hecs` | ECS. Pure Rust, no platform dependencies. Compiles to WASM unchanged. |
| `rapier3d` | Physics. Has WASM support (needs testing for performance). |
| `kira` | Audio. Has web backend via WebAudio API. |

## What Compiles to WASM

| Component | Status | Notes |
|-----------|--------|-------|
| Renderer (wgpu) | Compiles | wgpu targets WebGPU in WASM automatically |
| ECS (hecs) | Compiles | Pure Rust, no platform deps |
| Game logic (farming, construction, crafting) | Compiles | Pure Rust game systems |
| Input handling (winit) | Compiles | winit maps browser events to its event types |
| Shaders (WGSL) | Compiles | WGSL is the native WebGPU shading language — no translation needed |
| Audio (kira) | Compiles | Web backend available; fallback to web-sys AudioContext |
| Physics (rapier3d) | Needs testing | WASM support exists but perf budget needs validation |
| File I/O | Does not compile | Replace with IndexedDB via web-sys |
| Networking (TCP/UDP) | Does not compile | Replace with web-sys fetch and WebSocket |

## Platform Abstraction Layer

```rust
// engine/src/platform.rs

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(not(target_arch = "wasm32"))]
mod native;

pub trait Platform {
    fn load_asset(path: &str) -> Vec<u8>;
    fn save_data(key: &str, data: &[u8]);
    fn load_data(key: &str) -> Option<Vec<u8>>;
    fn current_time() -> f64;
    fn open_url(url: &str);
}
```

**Native implementation** (`native.rs`): `std::fs` for file I/O, `std::time::Instant` for timing, `opener` crate for URLs.

**Web implementation** (`web.rs`): `fetch` for asset loading, IndexedDB for save/load, `performance.now()` for timing, `window.open()` for URLs.

Game code calls `Platform::load_asset()` and does not care which implementation runs.

## Browser Requirements

| Feature | Browser Support |
|---------|----------------|
| WebGPU | Chrome 113+, Edge 113+ (May 2023), Firefox Nightly, Safari Technology Preview |
| WebGL 2 (fallback) | All modern browsers. wgpu's GL backend provides this automatically. |
| WASM | Universal since 2017. Every modern browser. |
| SharedArrayBuffer | Chrome 91+, Firefox 79+, Safari 15.2+. Required for multithreading. |

For browsers without WebGPU, wgpu's GL backend compiles WGSL shaders to GLSL and uses WebGL 2. Lower performance but wider compatibility. The engine detects the available backend at startup and selects automatically.

## Build Pipeline

```bash
# Desktop (existing workflow, unchanged)
cargo build --release -p humanity-engine

# WASM (new)
wasm-pack build engine-wasm --target web --out-dir pkg
# Produces:
#   engine_wasm_bg.wasm    (the compiled engine)
#   engine_wasm.js         (JS glue code with wasm-bindgen bindings)
#   engine_wasm.d.ts       (TypeScript type definitions)

# Serve locally for testing
npx serve engine-wasm/

# Production: copy pkg/ contents to ui/ static assets
# Served by nginx alongside the rest of HumanityOS
```

The WASM build is a separate `engine-wasm` crate that depends on the shared `engine` crate. It adds only the web-specific entry point and event loop.

## Integration with HumanityOS UI

The WASM engine renders to a `<canvas>` element. All HTML/CSS/JS UI (HUD, inventory, chat, menus) renders on top as standard DOM elements.

```
+------------------------------------------+
|  HTML/CSS UI layer (HUD, menus, chat)    |  <- z-index: above canvas
+------------------------------------------+
|  <canvas id="game-canvas">               |  <- WASM engine renders here
|    wgpu WebGPU context                   |
+------------------------------------------+
```

**JS to WASM** (calling engine from UI code):
```js
// wasm-bindgen exports these as JS functions
engine.get_player_position()   // returns {x, y, z}
engine.teleport_player(x, y, z)
engine.set_time_of_day(0.75)   // sunset
engine.get_inventory()         // returns item list
```

**WASM to JS** (engine calling UI code):
```js
// Engine calls these JS functions via wasm-bindgen imports
window.engineCallbacks = {
    show_dialog(text) { /* show in-game dialog */ },
    update_hud(health, stamina) { /* update HUD display */ },
    open_inventory() { /* show inventory panel */ },
    play_ui_sound(name) { /* trigger UI audio */ },
};
```

This boundary is deliberately thin. The engine owns all 3D rendering, physics, and game state. The JS layer owns all 2D UI. Communication is through exported/imported functions, not message passing.

## Performance Budget

| Metric | Target | Minimum |
|--------|--------|---------|
| WASM binary size | < 10 MB (< 3 MB gzipped) | < 20 MB |
| Initial load time | < 5 seconds (broadband) | < 10 seconds |
| Frame rate (1080p) | 60 FPS | 30 FPS |
| Memory usage | < 512 MB typical scene | < 1 GB |
| Input latency | < 16 ms (one frame) | < 33 ms |

Binary size is managed by:
- `wasm-opt -Oz` for size optimization
- `lto = true` in release profile
- Splitting large assets (textures, models) from the WASM binary — loaded on demand via fetch

## Implementation Phases

### Phase 1: Minimal WASM Shell
Prove the pipeline works. Canvas element, wgpu initialization, render loop, spinning colored cube. Validate that existing WGSL shaders load and run in WebGPU. No game logic.

### Phase 2: Camera System
All three camera modes (first-person, third-person, orbit) working in the browser. Keyboard and mouse input via winit web target. See `docs/design/camera-system.md`.

### Phase 3: Terrain Rendering
Load terrain tile data via fetch. Render ground, water, basic lighting. LOD system for multi-scale zoom (orbit mode). Day/night cycle with sky shader.

### Phase 4: Game Objects
Load object definitions from data files (CSV/TOML/JSON via fetch). Render trees, crops, buildings, items. Instanced rendering for performance.

### Phase 5: Player Controller
Character movement and collision. Interaction system (click to harvest, build, craft). Animation playback.

### Phase 6: Game Systems
Farming loop (plant, water, grow, harvest). Inventory management. Day/night cycle affecting gameplay. Crafting. These are pure Rust systems — they compile to WASM without changes.

### Phase 7: Networking
WebSocket connection to the relay server. Player position sync. Chat integration. Multiplayer state synchronization.

### Phase 8: Full Parity
Desktop and browser builds are feature-matched. Same game, same experience, same data. The only differences are platform-specific (file paths vs IndexedDB, native audio vs WebAudio).

## Related Files

- `engine/src/` — shared engine code (renderer, ECS, physics, systems)
- `engine/crates/` — 19 engine sub-crates
- `assets/shaders/` — 30 WGSL shaders (already WebGPU-compatible)
- `docs/design/engine-architecture.md` — master engine reference
- `docs/design/camera-system.md` — camera system (Phase 2)
- `docs/design/platform-architecture.md` — platform abstraction details
