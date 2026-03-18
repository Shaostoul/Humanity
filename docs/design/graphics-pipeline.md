# HumanityOS Graphics Pipeline — Design Document

**Status:** Draft
**Target hardware:** RTX 4070 (12 GB VRAM, 3rd-gen RT cores)
**Stack:** Rust + wgpu + Tauri + OpenXR

---

## Table of Contents

1. [Overview](#overview)
2. [Technology choices](#technology-choices)
3. [Architecture: dual-surface approach](#architecture-dual-surface-approach)
4. [Rendering pipeline phases](#rendering-pipeline-phases)
5. [PBR material system](#pbr-material-system)
6. [Environment rendering](#environment-rendering)
7. [VR path](#vr-path)
8. [Build order](#build-order)

---

## Overview

HumanityOS needs a 3D rendering layer for its world/game experience while keeping the existing Tauri webview for UI. The renderer must support AAA-quality raytracing, large outdoor environments, and eventually VR — built incrementally by a solo developer.

```
 ┌─────────────────────────────────────────────┐
 │              Final Composited Frame          │
 │  ┌───────────────────────────────────────┐   │
 │  │   Tauri Webview (UI overlay, alpha)   │   │
 │  │   menus, HUD, chat, settings          │   │
 │  ├───────────────────────────────────────┤   │
 │  │   wgpu 3D Scene (native GPU surface)  │   │
 │  │   world, characters, terrain, sky     │   │
 │  └───────────────────────────────────────┘   │
 └─────────────────────────────────────────────┘
```

---

## Technology choices

### wgpu (Rust GPU abstraction)

- **What:** Cross-platform GPU API that maps to Vulkan (Linux/Windows), DX12 (Windows), Metal (macOS), and WebGPU (browser).
- **Why:** Pure Rust, no C++ bindings to maintain. Same codebase targets desktop and (eventually) web.
- **Shader language:** WGSL is the default. SPIR-V accepted via naga translator for cases where WGSL lacks expressiveness or when porting existing shaders.
- **Ray tracing:** `wgpu-hal` has a ray-tracing extension that maps to `VK_KHR_ray_tracing_pipeline` on Vulkan. The RTX 4070's Ada Lovelace RT cores are fully exposed through this path.

### Why not Bevy / rend3 / other engines?

Bevy is a full ECS game engine — useful but opinionated. Starting with raw wgpu gives full control over the hybrid Tauri integration and avoids fighting an engine's assumptions. If Bevy's renderer matures its RT support, migrating the render backend is possible since both use wgpu underneath.

---

## Architecture: dual-surface approach

Two approaches exist. Start with Option A (simpler); migrate to Option B if latency or compositing issues arise.

### Option A — Two OS windows, composited by the OS

```
┌──────────────────────────────┐
│  Tauri Window (transparent)  │  ← HTML/CSS/JS UI, transparent background
│  z-order: always on top      │
├──────────────────────────────┤
│  wgpu Window (borderless)    │  ← 3D rendering, positioned underneath
│  z-order: behind Tauri       │
└──────────────────────────────┘

Communication: Tauri IPC commands (invoke/listen)
- UI → 3D:  camera controls, spawn commands, menu actions
- 3D → UI:  player stats, world events, loading progress
```

**Pros:** Simple. Each surface owns its own rendering. Webview works normally.
**Cons:** Two windows to keep synchronized (position, resize, minimize). OS compositor overhead.

### Option B — Single window, webview-as-texture

```
┌───────────────────────────────────┐
│  Single wgpu Window               │
│  ┌─────────────────────────────┐  │
│  │  3D scene rendered first    │  │
│  │  ┌───────────────────────┐  │  │
│  │  │ Webview → offscreen   │  │  │
│  │  │ texture, alpha-blend  │  │  │
│  │  │ as final quad         │  │  │
│  │  └───────────────────────┘  │  │
│  └─────────────────────────────┘  │
└───────────────────────────────────┘
```

**Pros:** Single window, no sync issues. Required for VR (can't float two windows in a headset).
**Cons:** Offscreen webview rendering is tricky. Input routing needs manual hit-testing.

### IPC protocol (both options)

```rust
// Tauri command example
#[tauri::command]
fn set_camera_position(x: f32, y: f32, z: f32) -> Result<(), String> {
    RENDER_STATE.lock().camera.set_position(Vec3::new(x, y, z));
    Ok(())
}

// JS side
await invoke('set_camera_position', { x: 10.0, y: 5.0, z: -20.0 });
```

Shared state between the Tauri thread and the render thread via `Arc<Mutex<RenderState>>` or a lock-free command queue (`crossbeam::channel`).

---

## Rendering pipeline phases

```
Frame N
  │
  ▼
┌─────────────┐   ┌─────────────┐   ┌──────────────┐
│ 1. G-Buffer │──▶│ 2. Lighting │──▶│ 3. RT Pass   │
│    Pass     │   │    Pass     │   │  (hardware)  │
└─────────────┘   └─────────────┘   └──────────────┘
                                          │
                                          ▼
                                   ┌──────────────┐   ┌──────────────┐
                                   │ 4. Post-     │──▶│ 5. UI        │
                                   │  processing  │   │  Composite   │
                                   └──────────────┘   └──────────────┘
                                                            │
                                                            ▼
                                                       Swapchain
                                                       Present
```

### Phase 1 — G-Buffer pass (deferred rendering)

Render all opaque geometry into multiple render targets:

| G-Buffer target | Format         | Contents                        |
|-----------------|----------------|---------------------------------|
| RT0             | RGBA16Float    | World-space position + depth    |
| RT1             | RGBA8Unorm     | Normal (octahedral encoded)     |
| RT2             | RGBA8Unorm     | Albedo RGB + alpha              |
| RT3             | RGBA8Unorm     | Metallic, Roughness, AO, flags  |
| Depth           | Depth32Float   | Hardware depth buffer           |

**Why deferred?** Decouples geometry complexity from light count. A large outdoor scene with hundreds of lights only pays the lighting cost per pixel, not per triangle.

**Vertex shader (WGSL sketch):**
```wgsl
struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = (model.transform * vec4(input.position, 1.0)).xyz;
    out.clip_pos = camera.view_proj * vec4(out.world_pos, 1.0);
    out.normal = normalize((model.normal_matrix * vec4(input.normal, 0.0)).xyz);
    out.uv = input.uv;
    return out;
}
```

### Phase 2 — Lighting pass

Full-screen quad reads G-Buffer textures. Computes direct illumination:

- **Sun/directional light** with cascaded shadow maps (4 cascades for outdoor range)
- **Point/spot lights** via light volume culling (tile-based or clustered)
- **Shadow maps:** 2048x2048 per cascade, PCF or PCSS filtering

Output: HDR lit color (RGBA16Float).

### Phase 3 — Ray-traced pass (hardware RT)

Uses `VK_KHR_ray_tracing_pipeline` via wgpu-hal. The RTX 4070's RT cores handle:

| Effect              | Rays/pixel | Priority | Notes                              |
|---------------------|------------|----------|------------------------------------|
| Reflections         | 1          | High     | Roughness-based; fallback to SSR   |
| Ambient occlusion   | 1          | High     | RTAO replaces SSAO                 |
| Global illumination | 1-2        | Medium   | Diffuse bounce; start with 1 bounce|
| Soft shadows        | 1          | Low      | Shadow maps sufficient at first    |

**Performance budget on RTX 4070 at 1440p:**
- RT reflections + AO: ~3-4 ms
- RT GI (1 bounce, half-res): ~4-5 ms
- Total RT budget: ~8 ms (leaves room for 60fps with other passes)

**Fallback:** When RT hardware is unavailable (older GPUs, WebGPU), fall back to screen-space techniques (SSR, SSAO, SSGI). Feature detection at startup.

**Acceleration structure:**
```
TLAS (Top-Level Acceleration Structure)
  ├── BLAS: terrain chunks (rebuilt on LOD change)
  ├── BLAS: static props (built once)
  ├── BLAS: dynamic objects (rebuilt per frame)
  └── BLAS: vegetation (simplified proxy geometry)
```

### Phase 4 — Post-processing

Applied in order:

1. **TAA (Temporal Anti-Aliasing)** — jittered projection + history buffer. Essential for denoising RT output.
2. **Bloom** — threshold + downsample chain + upsample blend
3. **Tone mapping** — ACES filmic or AgX
4. **Motion blur** — per-object velocity buffer, half-res
5. **Color grading** — LUT-based, artist-controllable

### Phase 5 — UI composite

- Read webview surface as texture (Option B) or let OS composite (Option A)
- Alpha-blend UI on top of the post-processed 3D frame
- UI renders at native resolution regardless of 3D render scale

---

## PBR material system

### Metallic/roughness workflow

Industry standard (matches glTF 2.0, Blender, Substance).

```
Material
  ├── albedo_map:     RGBA8 (RGB = base color, A = opacity)
  ├── normal_map:     RG8 or RGB8 (tangent-space normals)
  ├── metallic_roughness_map: RG8 (R = metallic, G = roughness)
  │                   — packed in one texture per glTF convention
  ├── ao_map:         R8 (baked ambient occlusion)
  ├── emissive_map:   RGB8 (self-illumination)
  └── parameters:
        metallic_factor:  f32  (default 0.0)
        roughness_factor: f32  (default 0.5)
        emissive_factor:  vec3 (default 0,0,0)
        alpha_mode:       Opaque | Mask | Blend
        alpha_cutoff:     f32  (for Mask mode)
```

### Asset format

- **Primary:** glTF 2.0 (`.glb` binary preferred for single-file distribution)
- **Why:** Open standard, Blender exports natively, PBR maps embedded, animation support
- **Texture compression:** BC7 (desktop) via basis-universal or gpu-texture-tools at build time
- **LOD:** Meshoptimizer for automatic LOD generation; store LOD levels in glTF extras

### Material shader (WGSL, fragment)

```wgsl
// Cook-Torrance BRDF core
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    let g1 = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g2 = n_dot_l / (n_dot_l * (1.0 - k) + k);
    return g1 * g2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}
```

---

## Environment rendering

### Terrain

- **Clipmap-based LOD:** Concentric rings of increasing mesh density centered on camera. 5-6 rings covers view distances up to several km.
- **Heightmap:** 16-bit heightmap textures, streamed per-region. GPU tessellation optional (start without).
- **Texturing:** Triplanar mapping with material-ID splatmap. 4-8 terrain materials blended per tile.
- **Data budget:** 1 km^2 at 1m resolution = 1M vertices at highest LOD. Clipmap reduces active vertex count to ~200K.

### Vegetation

- **Grass:** GPU-instanced billboards or mesh cards. Indirect draw from compute shader that culls by frustum + distance.
- **Trees:** Impostor sprites at distance, LOD meshes up close. Wind animation via vertex shader (bend + rustle).
- **Draw budget:** Target 50K-100K vegetation instances visible. GPU instancing keeps draw calls under 50.

### Weather and sky

- **Sky:** Procedural atmosphere (Bruneton or Hillaire model). Sun + moon + stars.
- **Clouds:** Raymarched volumetric clouds (quarter-res, temporally reprojected). Start with a skybox fallback.
- **Rain/snow:** GPU particle system, 10K-50K particles. Screen-space wetness darkening for rain.
- **Day/night cycle:** Sun angle drives atmosphere, shadow direction, ambient color.

### Streaming

Large worlds don't fit in VRAM. Region-based streaming:

```
┌─────┬─────┬─────┐
│     │     │     │   Each cell: ~500m x 500m
│  7  │  8  │  9  │   Load ring: cells within 2 of camera
├─────┼─────┼─────┤   Unload ring: cells beyond 3
│  4  │ CAM │  6  │   Async loading on background thread
├─────┼─────┼─────┤   GPU upload via staging buffers
│  1  │  2  │  3  │
└─────┴─────┴─────┘
```

---

## VR path

### OpenXR integration

- **Crate:** `openxr` (Rust bindings to the OpenXR runtime)
- **Runtime:** SteamVR or Oculus runtime (both expose OpenXR)
- **Vulkan interop:** OpenXR provides Vulkan swapchain images directly; wgpu renders into them

### Stereo rendering

```
┌──────────────────────────────────────┐
│          OpenXR Frame Loop           │
│                                      │
│  1. xr_wait_frame()                  │
│  2. xr_begin_frame()                 │
│  3. Get eye poses + projection       │
│  4. For each eye:                    │
│     ├── Update view/proj matrix      │
│     ├── Run pipeline phases 1-4      │
│     └── Render to eye swapchain      │
│  5. Composite UI as head-locked quad │
│  6. xr_end_frame()                   │
└──────────────────────────────────────┘
```

**Single-pass instanced stereo:** Render both eyes in one draw call using `gl_ViewIndex` / instancing. Saves CPU draw-call overhead. Requires shader modification but halves CPU cost.

### Performance targets

| Metric                | Target      | Notes                          |
|-----------------------|-------------|--------------------------------|
| Frame rate            | 90 fps      | 11.1 ms per frame              |
| Resolution (per eye)  | ~1440x1600  | Typical for current headsets   |
| RT effects in VR      | Reduced     | Half-res RT, fewer rays        |
| UI in VR              | Head-locked  | Floating panel, ~2m distance   |

### Foveated rendering

- **Fixed foveated:** Render edges at half resolution. No eye tracker needed. 30-40% pixel savings.
- **Eye-tracked (future):** Variable rate shading (VRS) driven by gaze. RTX 4070 supports VRS natively.

---

## Build order

Prioritized for a solo developer. Each phase produces something visible and testable.

### Phase 1 — Triangle to screen (weeks 1-3)

- [ ] wgpu window with clear color
- [ ] Load a single glTF model (use a free asset — damaged helmet is the PBR test standard)
- [ ] Camera controls (orbit, WASD fly)
- [ ] Forward-rendered PBR shader (no deferred yet, just get lighting right)
- [ ] Tauri integration: wgpu window + webview window side by side

**Milestone:** Spinning PBR model with mouse orbit, Tauri UI next to it.

### Phase 2 — Deferred + multiple objects (weeks 4-6)

- [ ] G-Buffer pass (position, normal, albedo, metallic/roughness)
- [ ] Deferred lighting pass (directional + a few point lights)
- [ ] Shadow mapping (single cascade for now)
- [ ] Scene graph: load multiple glTF files, transform hierarchy
- [ ] Frustum culling

**Milestone:** Lit scene with shadows and multiple objects.

### Phase 3 — Raytracing (weeks 7-10)

- [ ] Build BLAS/TLAS acceleration structures
- [ ] RT reflections (1 ray/pixel, denoise with TAA)
- [ ] RT ambient occlusion
- [ ] Fallback path for non-RT GPUs (SSAO + SSR)
- [ ] Cascaded shadow maps (4 cascades)

**Milestone:** Reflective/metallic surfaces with RT reflections, AO on all surfaces.

### Phase 4 — Outdoor environment (weeks 11-15)

- [ ] Terrain system (clipmap, heightmap, splatmap texturing)
- [ ] Procedural sky + sun
- [ ] Basic vegetation (GPU-instanced grass)
- [ ] Region streaming (load/unload terrain chunks)
- [ ] Post-processing chain (TAA, bloom, tone mapping)

**Milestone:** Explorable outdoor terrain with sky, grass, and post-processing.

### Phase 5 — Polish + weather (weeks 16-20)

- [ ] Volumetric clouds (quarter-res raymarch)
- [ ] Rain/snow particles
- [ ] Day/night cycle
- [ ] Tree LOD + wind
- [ ] RT global illumination (1 bounce, half-res)
- [ ] Dual-window compositing (webview overlay on 3D)

**Milestone:** Full outdoor scene with weather, day/night, and UI overlay.

### Phase 6 — VR (weeks 21-26)

- [ ] OpenXR session + swapchain
- [ ] Stereo rendering (two-pass first, single-pass instanced later)
- [ ] Head-locked UI panel
- [ ] Fixed foveated rendering
- [ ] Performance profiling + optimization for 90fps

**Milestone:** Walk around the outdoor world in VR with UI panel visible.

---

## Crate dependencies (initial)

```toml
[dependencies]
wgpu = "24"                    # GPU abstraction
winit = "0.30"                 # Window + input
glam = "0.29"                  # Math (vec3, mat4, quat)
gltf = "1.4"                   # glTF 2.0 loader
image = "0.25"                 # Texture loading (PNG, JPG, HDR)
pollster = "0.4"               # Block on async (wgpu init)
bytemuck = { version = "1", features = ["derive"] }  # Safe transmute for GPU buffers
log = "0.4"
env_logger = "0.11"

# Phase 3+
# wgpu-hal for raw ray-tracing API access

# Phase 6
# openxr = "0.18"              # VR runtime
```

---

## Performance budget (1440p, RTX 4070, 60fps target)

| Pass                | Budget  | Notes                           |
|---------------------|---------|---------------------------------|
| G-Buffer            | 2 ms    | Geometry bound                  |
| Shadow maps         | 2 ms    | 4 cascades                      |
| Deferred lighting   | 1 ms    | Full-screen quad                |
| RT reflections      | 3 ms    | Half-res, 1 ray/px              |
| RT AO               | 2 ms    | Half-res                        |
| RT GI               | 4 ms    | Half-res, 1 bounce              |
| Post-processing     | 1.5 ms  | TAA + bloom + tonemap           |
| UI composite        | 0.5 ms  | Single textured quad            |
| **Total**           | **16 ms** | Headroom for 60fps (16.6 ms)  |

For VR (90fps, 11.1 ms budget): reduce RT to quarter-res, skip RT GI, use fixed foveated rendering.

---

## Open questions

1. **wgpu ray-tracing maturity:** The `wgpu-hal` RT extension is experimental. May need to drop to raw Vulkan (`ash` crate) for RT if wgpu's abstraction is too limiting. Monitor wgpu GitHub for stabilization.
2. **Webview-as-texture:** Chromium's offscreen rendering is possible via CEF or WebView2's `CreateCoreWebView2CompositionController`. Needs prototyping. If too complex, stick with dual-window.
3. **Asset pipeline:** Manual glTF export from Blender works initially. A proper asset pipeline (compression, LOD generation, texture packing) becomes necessary at Phase 4+.
4. **ECS adoption:** Raw Rust structs work for Phase 1-2. Once entity count grows (vegetation, particles), consider adopting `hecs` or `bevy_ecs` (without Bevy's renderer) for efficient iteration.
