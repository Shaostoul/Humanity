# The renderer roadmap: what is missing, the 2030 technique for each, and the order

Written 2026-11-12 for the operator's question of 2026-11-10: "Is there anything else renderer related
that we're missing?" This is the plan of record for the renderer gaps that no other plan owns.
Section 1 is what the renderer has and lacks, read from the code; section 2 the shared foundation
most of the gaps stand on (motion vectors, jitter, a thin G-buffer); sections 3 to 10 each gap as a
2030 release would build it, with how it fits this engine, its cost on the RTX 4070 and on an RX 580
class card, its Settings control, its simplified mode and its hardware notice; section 11 the
budgets together; section 12 the build order with rungs; section 13 the questions only the operator
can answer.

Plans this one leans on and does not repeat: [local-light-shadows.md](local-light-shadows.md)
(every near shadow; its shape is the model for this file), [sun-cascades.md](sun-cascades.md),
[room-gi.md](room-gi.md) (bounced light in rooms), [hdr-scene-target.md](hdr-scene-target.md) (the
float scene target and the one tonemap), [character-animation.md](character-animation.md) (bodies,
hair, the look), [graphics-settings.md](graphics-settings.md) (every control's words and cost line).
The upscaler scan with its licences quoted: [2026-11-12 upscalers and anti-aliasing](../reference/research/2026-11-12-upscalers-and-anti-aliasing/README.md).
Engine rules for anyone building a rung: [docs/ai/renderer.md](../ai/renderer.md).

**Every cost below is an estimate unless it says measured.** The operator's card is an RTX 4070
(504 GB/s, about 29 TFLOPS) driving 2560 x 1440 at 120 frames a second, so 8.3 ms a frame; the
project's floor is an RX 580 (256 GB/s, about 6 TFLOPS, 4 or 8 GB) at 1920 x 1080. A full-screen pass
limited by memory traffic costs about the same on the RX 580 at 1080p as on the 4070 at 1440p; one
limited by arithmetic costs about 2.5 times as long. Measured today, for scale: the crowd vantage
(`anim-crowd-near`, fifty people indoors) spends 13.46 ms of graphics card time a frame, 7.57 of it in
the scene pass (graphics-settings.md); the homestead's ground hall's scene pass is 11.5 to 13.2 ms
(renderer.md). The game does not reach 120 frames a second there today, which is why upscaling
(section 3) is part of the first rung, not an afterthought.

## 1. What the renderer has, and what it lacks (read from the code, 2026-11-12)

The coordinator's audit of 2026-11-10 (the journal) was checked against the code. It holds, with
four corrections (marked).

| Area | State | Evidence |
|---|---|---|
| Anti-aliasing | **none** (temporal anti-aliasing built 2026-11-13, rung 1a: section 12): no MSAA in the 3D world, no TAA, no FXAA or SMAA, no motion vectors, no render scale | every world pipeline `sample_count: 1`; `billboard_bake.rs`: "this engine has no MSAA anywhere"; only the Settings model preview (`gui/model_view/gpu.rs`) multisamples |
| Temporal history | clouds only, and indexed by direction, not screen | `45-cloud-temporal.wgsl`: "No previous view-proj plumbing, no disocclusion vectors" |
| Indirect specular (reflections) | **none anywhere except the water** (correction: not only indoors) | `80-fragment-shared.wgsl`: `ambient = albedo * indirect * ao`, diffuse only; outdoors a metal or a wet surface reflects no sky; room probes carry irradiance only (`room_probes.rs`); the water samples the sky table itself |
| Material lobes | one GGX lobe plus foliage transmission and the hair's Marschner lobes; no subsurface, sheen, clearcoat or anisotropy | `50-brdf.wgsl`; grep finds none of them |
| Material textures | the ground (colour and normal layers), garments and hair (colour and normal packed side by side); most surfaces are procedural types with one albedo slot | `00-bindings-vertex.wgsl`, `materials.rs` |
| Decals | none | grep |
| Light units | intensity is a multiplier; `light_types.ron` carries electrical `watts` but light is `intensity: 9.0` beside them; colour is RGB, not a temperature | `light.rs` ("Brightness multiplier"), `data/lighting/light_types.ron` |
| Exposure | **fixed constants per shader** (correction: there is no exposure control or eye adaptation) | `30-atmosphere.wgsl` `ATMO_EXPOSURE: f32 = 4.0`, `20-surface-detail.wgsl` `WATER_SKY_LUT_EXPOSURE: f32 = 15.0` |
| Tonemap | inline in each shader; the float scene target holds display-referred values; the one tonemap is increment 5 of hdr-scene-target.md, not built | `present.wgsl`: "5 (later): the one tonemap" |
| Bloom | **built but dormant** (correction: the audit listed it as present) | `performance.rs`: "DORMANT: nothing calls `apply` today" |
| HDR display output | none: the swapchain is the first sRGB format offered | `renderer/mod.rs` `surface_caps ... find(\|f\| f.is_srgb())` |
| Parallax, displacement | none; the ground textures derive a height for blending only | `ground_textures.rs` `derive_height` |
| Depth of field, motion blur | none | grep |
| Eyes, teeth, tongue | the generated eyes stay; MakeHuman's eye meshes, teeth and tongue are "converted nowhere yet" | character-animation.md, known limits |
| Ambient occlusion | **covers the celestial layer only** (correction): SSAO multiplies the colour target before the scene pass, so the homes, furniture and people drawn after it get none; materials carry an albedo-based occlusion | `ssao.rs` header; frame order in `lib.rs` (atlas, celestial, god rays, SSAO, scene, transparent, overlay, lines, particles, present) |
| Present | sun shadows with cascades, room light shadows (rung 1), light shafts, room GI rung 1, atmosphere, clouds, ocean, particles, the float scene target with one dither | the plans above |

### The constraints every rung lives in

- **A forward renderer** with six class entries, one per pipeline, and the H1 rule: a heavyweight
  branch costs every pixel of every pipeline that compiles it, so each new lobe gets its own
  `override` switch and lives in one class entry (renderer.md). Measure each new term with the rig's
  hot-reload A/B, compiled in and switched off against compiled out.
- **Sampled textures: 14 of the 16 a shader stage may use** (counted from the WGSL declarations
  today: group 0 has the room light atlas and the shadow atlas, group 1 the emission depth, group 3
  eleven: albedo, two cloud volumes, the weather map, the sun shadow map, the ground array, the two
  atmosphere tables, the sky view, the tree atlas, the ocean spectrum). The limit is
  `wgpu::Limits::default()`, kept deliberately since v0.782 to v0.784 shipped unbootable on a limit.
  **Two bindings are left for every future feature that samples inside the megashader**, so this plan
  puts most new work in passes of its own and spends the two on decals and (later) one more.
- **Storage buffers: 6 of 8 a stage** (lights, tile counts, tile indices, regions, room GI, the shadow
  table). **Colour attachments: 32 bytes a pixel** across all targets of a pass (the default).
- **No sampler LOD bias in wgpu** (`SamplerDescriptor` has clamps and anisotropy only), so a temporal
  upscaler's sharper texture reads are `textureSampleBias` in the shader.
- **Two depth spaces a frame**: `render_scene_onto` clears depth (reverse Z, `Depth32Float`) and draws
  the homes, furniture and bodies in their own, after the celestial pass drew the planet in its own.
  One end-of-frame depth buffer therefore cannot reproject the whole picture: each pass must write
  its own motion (section 2).
- **Ray queries in wgpu 24 are Vulkan only**, and this engine runs DirectX 12 on Windows. Every
  ray-traced rung needs a newer wgpu with DirectX 12 ray queries or the Vulkan backend on Windows.
- **The tile light lists** are a 16 x 9 screen grid built on the processor, 64 lights a tile
  (`light_tiles.rs`); decals bin into the same grid (section 7).
- **No caches across runs; reuse only versioned and behind the switch.** A temporal history is
  data kept across frames. Every history in this plan (the anti-aliasing history, the reflection
  denoiser's, the room reflection captures) is thrown away every frame while the Dev page's "Turn off
  all reuse" is on (`HUMANITY_NO_REUSE=1`), so a smear that smells like stale data is ruled in or out
  with one click. Found on the way: the cloud octahedral history (`cloud_temporal.rs`) and the room
  probes (`room_probes*.rs`) do not consult the switch today (grep); whether each is a reuse the rule
  covers is for their owners to settle.

## 2. The shared foundation: motion vectors, jitter and a thin G-buffer

Anti-aliasing, upscaling, the reflection denoiser, motion blur, the hair's single pass and later the
ray-traced shadows and volumetric fog all need the same three things. They are built once, in the
first rung (section 12).

1. **Motion vectors** (screen-space velocity, `Rg16Float`, 4 bytes a pixel), written as a second
   colour target by **every opaque pass**, celestial and scene, each with its own camera's previous
   matrices, because the scene pass clears depth. What moves on its own writes its own motion:
   - near bodies (compute-skinned): last frame's skinned positions kept beside this frame's (a second
     vertex buffer per near body, ping-ponged; the skinning pass already writes the first);
   - middle-tier bodies (vertex-shader skinning): last frame's bone palette kept beside this frame's
     (a second palette buffer; about 4 KB a body);
   - the far tier's baked animation: the previous frame index;
   - wind-moved foliage (classes 19 to 23): the wind evaluated at the previous time as well, in
     `vs_main`;
   - the ocean: its displacement at the previous time (the spectrum pass keeps the previous frame's
     displacement, in the water's own binding);
   - the floating origin: on the frame the origin moves (`floating_origin.rs`) the previous matrices
     are re-expressed in the new origin, or every pixel streaks for one frame.
   The velocity target is cleared to a sentinel meaning "sky": those pixels reproject by the camera's
   rotation alone, which is exact for the stars, the sky and the clouds (whose own history is
   already by direction).
2. **Jitter**: a sub-pixel offset added to the projection each frame (a Halton 2,3 sequence of 8 or
   16), removed again when the velocity is computed. The shadow passes, the cloud octahedral pass and
   the off-screen camera screens are not jittered.
3. **A thin G-buffer** for the passes that run after the opaque scene: the surface normal and
   roughness (`Rgb10a2Unorm`, octahedral normal) and the specular colour with an occlusion term
   (`Rgba8Unorm`), 8 bytes a pixel. With the colour (8) and the velocity (4) that is 20 of the 32 bytes
   a pass may write. It is what lets reflections be added after the forward pass in a pass of their
   own (section 4), instead of costing megashader bindings. Doom (2016) is the shipped reference for a
   clustered forward renderer that writes such a buffer beside its colour for screen-space work
   (Sousa and Geffroy, "The Devil is in the Details: idTech 666", SIGGRAPH 2016 Advances).

The local shadow plan's rung 1b proposes a depth pre-pass for its shadow mask. If it lands first,
the velocity can be written there instead of in the colour pass, and ground-truth ambient occlusion
(section 10) reads the same depth before the scene is shaded.

**Cost** (estimates): the extra targets are 15 MB (velocity) and 30 MB (thin G-buffer) at
2560 x 1440, 8 and 17 MB at 1080p. Writing them is about 0.1 ms on the 4070 with typical overdraw;
the doubled animation work (previous wind, previous palettes) adds up to 0.3 ms in a forest or a
crowd. Velocity adds two interpolants to every pipeline, which can raise the register count of the
fullest path (the H1 lesson): measured through the A/B before it is trusted.

## 3. Anti-aliasing and upscaling (first)

### 3.1 The 2030 technique

**Temporal anti-aliasing with temporal upscaling**, the standard of every current engine: each frame
renders at a jittered sub-pixel position, and a resolve blends it into a history reprojected through
the motion vectors, rejecting history that no longer matches. The published methods: Karis 2014
("High Quality Temporal Supersampling", SIGGRAPH Advances: the closest-depth velocity, the YCoCg
neighbourhood clamp, the reversible tonemap weighting); Salvi 2016 ("An Excursion in Temporal
Supersampling", GDC: variance clipping); Pedersen 2016 ("Temporal Reprojection Anti-Aliasing in
INSIDE", GDC); and the survey that collects them, Yang, Liu and Salvi 2020 ("A Survey of Temporal
Antialiasing Techniques", Computer Graphics Forum 39(2)). Upscaling is the same resolve fed a smaller
jittered image and accumulating into a display-sized history (Unreal's TAAU; AMD FSR 2 and 3).
Machine-learned upscalers (DLSS, XeSS, FSR 4) are the frontier, and none is open source (3.8).
Alongside it, **geometric specular anti-aliasing** (Kaplanyan et al. 2016, "Filtering Distributions of
Normals for Shading Antialiasing", HPG; Tokuyoshi and Kaplanyan 2019, I3D) widens the roughness where
the normal changes fast inside a pixel, so highlights on normal-mapped cloth and ground stop
sparkling; it costs a few instructions and helps with or without TAA.

### 3.2 How it fits this engine

- **The resolve** is one compute pass after the transparent list and before the overlay (labels,
  holograms), the line passes and the particles, which draw after it, unjittered. Particles draw after
  the resolve because a blended sprite writes no depth or motion and would trail (Bevy's TAA notes say
  the same). The present pass then reads the resolved image.
- **The input today is display-referred** (each shader tonemaps inline). The resolve works on it
  directly; when hdr-scene-target's increment 5 makes the target linear, the resolve switches on
  Karis's luminance weighting (weight by `1 / (1 + luma)`, undo after) so fireflies do not ghost. One
  flag; no rewrite.
- **History rejection**: Catmull-Rom history sampling, variance clipping in YCoCg, the closest of the
  3 x 3 depths' motion, and a reset on a camera cut (teleport, vantage jump, world entry, the rigs'
  parks).
- **Blended surfaces that stay before the resolve** (glass, the water's surface, the hair fringe until
  3.3, the cloud composite's edge) write their alpha into a **responsive mask** that lowers the
  history's weight where they are (FSR 2's reactive mask idea).
- **Texture sharpness**: under TAA the textures are read with a small negative bias, and under
  upscaling with the bias `log2(render width / display width)`, through `textureSampleBias` (wgpu has
  no sampler bias). A contrast-adaptive sharpen after the resolve (written by us after AMD's published
  CAS idea) restores what the blend softens, with a strength slider.
- **The camera screens and the hi-res screenshot** do not use it: the screens render at 10 Hz into
  their own targets; the hi-res capture is already supersampled.

### 3.3 What it changes for the card hair and the alpha-tested foliage

- **Card hair** uses Scheuermann's two passes today (an opaque core at alpha 0.8 and up, a blended
  fringe in the transparent list) precisely because the engine has no MSAA and no TAA
  (character-animation.md). With TAA the fringe becomes **hashed alpha testing** (Wyman and McGuire
  2017, "Hashed Alpha Testing", I3D) in the same opaque pass: below the core each card fragment is
  kept or discarded by a stable hash against its alpha, the hash rotating with the jitter, and the
  resolve integrates the coverage into a soft edge (the common practice for hair cards in engines with
  TAA; Unreal's dithered opacity mask is the same idea). It gains: the fringe writes depth and motion (no ghost), casts into the
  shadow atlas with its true coverage, needs no sorting, and **the player's own fringe appears on a
  planet** (today their body stands in render space, which has no transparent list, so only the core
  is drawn: a known limit in character-animation.md). The two passes stay as the path when
  anti-aliasing is Off or "Edges only".
- **Alpha-tested foliage** (leaves, grass, the far cards) shimmers wherever a leaf is thinner than a
  pixel; TAA is what makes it stable, and with the wind's previous position (section 2) a swaying leaf
  does not ghost. Far foliage can then switch from a hard cut to hashed alpha, and level-of-detail
  crossfades can dither, both of which TAA turns into smooth blends.
- **The near-sun and room-light shadow edges** (PCF) and the cloud march's own jitter become calmer
  too; the shadow passes themselves are not jittered.

### 3.4 Budgets (estimates)

| What | RTX 4070, 2560 x 1440 | RX 580, 1920 x 1080 |
|---|---|---|
| Motion vectors and jitter (section 2) | 0.1 to 0.4 ms; 15 MB | 0.2 to 0.6 ms; 8 MB |
| TAA resolve with sharpen | 0.3 to 0.5 ms; history 59 MB (two `Rgba16Float` frames) | 0.5 to 0.8 ms; 33 MB |
| "Edges only" (SMAA 1x) instead | 0.2 to 0.4 ms; 15 MB | 0.3 to 0.5 ms; 17 MB |
| TAAU at Quality (renders 1707 x 960) | resolve 0.4 to 0.6 ms; history at display size (59 MB); **saves the pixel-bound share of the frame: up to 3 to 5 ms at the crowd vantage** | renders 1280 x 720; similar share saved |
| FSR 3.1 upscaler, if ported (3.8) | about 0.6 to 0.8 ms (AMD's published FSR 2 figure for an RX 6800, a card of the 4070's class, is 0.7 ms at 2560 x 1440 Quality); 134 MB working set (AMD's FSR 3 table) | 1.3 ms (AMD's published FSR 2 figure for an RX 590 at 1080p Quality); 75 MB |

The resolve's cost is mostly memory traffic (about 50 bytes a pixel), so the RX 580 pays about the
same at 1080p as the 4070 at 1440p. Plain TAA is lighter than FSR 2, whose published numbers (above)
bound it from above.

### 3.5 Modes

- **Temporal** (Full, the default): TAA with motion vectors and sharpening; hair in one pass with
  hashed alpha; the render scale offered.
- **Edges only** (Simplified): SMAA 1x (Jimenez et al. 2012, "SMAA: Enhanced Subpixel Morphological
  Antialiasing", Eurographics), no history, so no ghosting and no softness; edges smooth, but thin
  hair, grass and highlights still shimmer, and the hair keeps its two passes.
- **Off**: as today. Low impact (`HUMANITY_LOW_IMPACT`) keeps Temporal: it is cheap and the rigs need
  their pictures to match the player's.

### 3.6 Settings > Graphics > Image (a new section)

The cost rows below follow `data/gui/graphics_settings.ron`'s kinds. An id the engine does not record
yet (`gpu.taa`, `gpu.smaa`, and in later sections `gpu.ssr`, `gpu.skin_scatter`, `gpu.present` and the
rest) gets its pass timer in the rung that adds the pass, before its row (graphics-settings.md's
checklist, step 2; the ratchet fails on an id nothing records).

- **Anti-aliasing** (Off, Edges only, Temporal). Hint, what you will see: "Smooths the stair-steps on
  edges. Temporal also steadies thin things that shimmer (hair, grass, leaves, far railings) by
  blending each frame with the last few; Edges only smooths edges without blending, so nothing trails
  but thin things still shimmer." Cost line: `Gpu(["gpu.taa"])` (or `gpu.smaa`), `Shares(["gpu.scene",
  "gpu.celestial"])` for the motion vectors, `Memory(["taa.history"])`.
- **Sharpening** (a slider, shown under Temporal): "Restores the crispness the blending softens; too
  much draws bright edges." `Gpu(["gpu.taa"])` (it runs inside the resolve).
- **Render scale**, levels as data (`data/gui/render_scale_levels.json`: Native, Quality 67%,
  Balanced 59%, Performance 50%): each level's line names the pixels it renders and the size it is
  shown at ("renders 1707 x 960, shown at 2560 x 1440"), computed from the window; the measured line
  shows the scene and surface passes at the current level, so picking each and reading the line
  compares them. A level that would render under 720 lines says it is blurry rather than hiding it.
  Offered only under Temporal (it needs the history).
- The page states plainly why DLSS, XeSS and FSR 4 are not offered: each is a binary that may not be
  modified (DLSS only on NVIDIA cards), and HumanityOS uses only open source for its core; that is
  this project's own rule, not a law, a patent or a platform holder's rule.

### 3.7 The hardware notice

Everything here is compute shaders and float storage textures, core on every card wgpu runs, so no
card is refused and the hint says so. The memory line names the card's own memory and the history's
share, warning past an eighth (as the shadow atlas does).

### 3.8 The open-source options (scan: [2026-11-12](../reference/research/2026-11-12-upscalers-and-anti-aliasing/README.md))

Our own TAA and TAAU in WGSL from the papers, with Bevy's TAA (Rust on wgpu, MIT or Apache-2.0) and
Playdead's (MIT) read as references, not copied. AMD's FSR 3.1 upscaler, whose source files are listed
in the MIT section of the AMD FSR SDK's otherwise binary-only licence, is the alternative to measure
against our TAAU before porting. FSR 4 (signed binary only), DLSS ("licensed for you to develop
applications only for their use in systems with NVIDIA GPUs") and XeSS ("provided in binary form only
... without modification") are not adopted. SMAA (MIT) is the Edges only mode. Quotations, dates and
what is unknown are in the scan.

### 3.9 Dev tooling and the reuse switch

- Showcase keys `taa` (off, smaa, temporal) and `render_scale`, the red twins: with `taa` off the
  picture must equal today's.
- **A deterministic capture mode**: the rigs' captures (and `visual-sweep`'s goldens) park the camera,
  reset the history and accumulate a fixed number of frames before capturing, so a capture does not
  depend on the frames before it (the present dither is spatial-only for the same reason).
- A Dev page view of the velocity target and the responsive mask (forever-dev rule: inspection tools
  are permanent).
- With "Turn off all reuse" on, the history is discarded every frame (the picture aliases, as Off),
  which is the A/B the switch exists for.

## 4. Reflections (second)

### 4.1 The 2030 technique

The industry ladder, every rung a fallback of the next: **ray-traced reflections** where the
hardware has ray queries (Unreal's Lumen: Wright, Narkowicz and Kelly 2022, "Lumen: Real-time Global
Illumination in Unreal Engine 5", SIGGRAPH Advances); **stochastic screen-space reflections** with
hierarchical depth tracing and a temporal denoiser (Stachowiak and Uludag 2015, "Stochastic
Screen-Space Reflections", SIGGRAPH Advances; Uludag 2014, "Hi-Z Screen-Space Cone-Traced Reflections",
GPU Pro 5); **parallax-corrected reflection captures** for what the screen does not show (Lagarde and
Zanuttini 2012, "Local Image-based Lighting With Parallax-corrected Cubemaps", SIGGRAPH), prefiltered
by roughness with the split-sum approximation (Karis 2013, "Real Shading in Unreal Engine 4", SIGGRAPH
course); and the **sky** for everything outdoors. Around them: specular occlusion (Lagarde and de
Rousiers 2014, "Moving Frostbite to Physically Based Rendering 3.0"; Jimenez et al. 2016, "Practical
Realtime Strategies for Accurate Indirect Occlusion") and multiple-scattering energy compensation so
rough metals keep their brightness (Kulla and Conty 2017, "Revisiting Physically Based Shading at
Imageworks"; Fdez-Aguera 2019, JCGT 8(1)).

### 4.2 How it fits this engine

- **A rooms-are-boxes advantage.** Lagarde's parallax correction projects a capture onto a box; this
  engine's rooms already are boxes to the probes (`room_probes.rs` traces each room's own box). So a
  room's reflection capture, projected onto its own box, is exact for its walls, floor and ceiling,
  and a fragment uses only its own room's capture, the same rule that keeps the room GI from leaking
  through walls.
- **The specular composite**, a full-screen pass after the opaque scene and before the transparent
  list, adds the indirect specular to the colour: for each pixel it reads the thin G-buffer, takes the
  screen-space reflection where one was found, else its room's capture, else the sky (where the pixel
  is under the sky), and multiplies by the split-sum's analytic environment term (Karis 2014, "Physically
  Based Shading on Mobile", no table), the specular occlusion (the room box, the ambient occlusion)
  and the haze transmittance written by the forward pass. Indirect specular only adds, so it can be
  added late; the megashader gains no binding, which section 1's budget needs.
- **The sky source** is a small prefiltered cube (6 faces of 64 x 64 with five roughness mips) rebuilt
  each frame from the sky-view table the engine already computes (Hillaire 2020): about 0.05 ms. It
  gives every outdoor metal, painted panel, wet rock and window its sky.
- **Room captures**: six faces of 256 (Full) or 128 (Simplified) rendered from the room's centre with
  the scene's static contents and its lamps, people left out, prefiltered into roughness mips. Kept
  across frames, so versioned: a capture is redrawn when its room's version changes (the home's edit
  counter, a lamp switched, a door opened) and on a schedule for the sun through windows (one face a
  frame), and every frame while the reuse switch is off. Resident only for the rooms near and in view,
  by a budget; the rest fall back to the room's ambient colour.
- **Screen-space reflections** (Full only): a depth pyramid (also useful for occlusion culling later),
  tracing at half resolution for rough surfaces and full for smooth ones, reusing the previous
  resolved frame through the motion vectors, denoised temporally with TAA's history and spatially.
  They catch the people and moving things the captures leave out. They see only the scene pass's
  depth space; a celestial-layer reflection (a planet seen through a window, reflected in a floor)
  falls back to the capture or the sky.
- **Glass and water**, which are blended and so miss the composite, read their reflections in their
  own class entries: the water keeps its sky (and gains its room's capture aboard), glass takes the
  room capture. That costs one binding in those entries, the second of the two spare, or shares the
  sky cube's when the group 3 layout is split by class (a refactor worth its own rung when the
  bindings run out).
- **Ray-traced reflections** (rung 4) replace the screen-space trace where ray queries exist; they need
  a wgpu with DirectX 12 ray queries (section 1) and a scene representation for the rays (the room
  GI's rung 2 voxelizes each room's contents; triangles for the near scene).

### 4.3 Budgets (estimates)

| What | RTX 4070, Full | RX 580, Simplified |
|---|---|---|
| Thin G-buffer (section 2) | 30 MB; about 0.1 ms to write | 17 MB |
| Sky cube | 0.05 ms; 0.2 MB | the same |
| Room captures | 24 rooms resident at 256: about 100 MB; drawing a face 0.1 to 0.3 ms GPU and up to 0.3 ms CPU only while a room refreshes (nothing when nothing changed) | 12 rooms at 128: about 13 MB |
| Screen-space reflections | 1 to 2 ms (pyramid, trace, denoise); about 50 MB | off |
| The composite | 0.2 ms | 0.25 to 0.4 ms |
| **In all** | **about 1.3 to 2.5 ms; about 180 MB** | **about 0.3 to 0.5 ms; about 30 MB** |

### 4.4 Modes, Settings, hardware

- **Full**: sky, room captures at 256, screen-space reflections. **Simplified**: sky and room captures
  at 128, no screen-space trace (people are not reflected). **Off**: as today, the water alone
  reflects.
- **Settings > Graphics > Light and sky > Reflections** (Off, Simplified, Full): "Shiny things reflect
  what is around them: metal, polished floors, wet stone, windows. Full also reflects people and
  things that move, from what is on screen." Each level's line: the capture size, how many rooms are
  kept, its video memory (computed); measured `Gpu(["gpu.reflect_capture", "gpu.ssr",
  "gpu.reflect_composite"])`, `Memory(["reflect.captures"])`, and what it did this frame ("Now: 9
  rooms reflect, 2 waiting").
- **Hardware**: none past core for Off to Full; the memory warning past an eighth. The ray-traced level,
  when it exists, is offered only where the card has ray queries, and says so where it is greyed:
  what it needs (ray tracing hardware: an RTX 20 series, an RX 6000 series or newer), what this card
  has, and that Full works instead.

## 5. Skin, cloth, clearcoat, eyes and teeth (third)

### 5.1 The 2030 technique

- **Skin**: screen-space subsurface scattering with a measured diffusion profile, separable (Jimenez
  et al. 2015, "Separable Subsurface Scattering", Computer Graphics Forum 34(6)) using Burley's
  normalized diffusion (Christensen and Burley 2015, "Approximate Reflectance Profiles for Efficient
  Subsurface Scattering", Pixar), as Unreal does; light through thin parts (ears, nostrils, fingers)
  from the shadow map's depth (Jimenez et al. 2010, "Real-Time Realistic Skin Translucency", IEEE CG&A);
  a two-lobe specular for skin's oily layer (d'Eon and Luebke 2007, GPU Gems 3 chapter 14). The
  forward-friendly simplified form is **pre-integrated skin** (Penner and Borshukov 2011, GPU Pro 2):
  the scattering folded into a function of the light angle and the surface's curvature.
- **Cloth**: the Charlie sheen lobe (Estevez and Kulla 2017, "Production Friendly Microfacet Sheen
  BRDF", Sony Imageworks) with Neubelt and Pettineo's visibility (2013, "Crafting a Next-Gen Material
  Pipeline for The Order: 1886"), the model glTF's `KHR_materials_sheen` and Google's Filament use;
  anisotropic highlights for satin and brushed metal.
- **Clearcoat**: a second, fixed-index GGX lobe over the base (Filament's model, glTF's
  `KHR_materials_clearcoat`) for lacquered wood, painted metal, car paint, varnish.
- **Eyes**: refraction through the cornea onto an iris set back in depth, a wet sclera with
  scattering, a specular highlight on the tear film, and a soft occlusion shell where the lids meet
  the eye (Jimenez, von der Pahlen et al., "Next-Generation Character Rendering", GDC 2013; shipped
  digital humans carry such a shell over each eye). **Teeth and tongue**: scattering, plus the mouth cavity's
  occlusion so the back teeth fall into shade.

### 5.2 How it fits this engine

- **Sheen and clearcoat** are lobes inside the light loop, each behind its own `override` switch, in
  the class entries that draw such materials (cloth is type 25; clearcoat goes where lacquered and
  painted materials draw), with their parameters (sheen colour and roughness, clearcoat amount and
  roughness) in the material's uniform as data, never per-item code. The H1 A/B measures each.
- **Pre-integrated skin** is a branch of the skin class: curvature from the screen derivatives of the
  normal (as the hair and cloth already take their frames), the falloff as an analytic fit, so no
  table and no binding.
- **Screen-space skin scattering** (Full): skin pixels write their diffuse light into a target of their
  own (8 bytes a pixel; with section 2's targets 28 of the 32 bytes) and the specular into the colour;
  two separable blur passes run over the skin pixels only (tile-classified), and the blurred diffuse is
  added back. The cost follows the skin on screen, not the number of people.
- **Thin-part transmission** reads the room light shadow atlas (local-light-shadows.md) and the near
  sun tile for the thickness between the light and the surface: a light behind an ear reddens it.
- **Eyes, teeth and tongue**: MakeHuman's eye meshes, teeth and tongue converted the way the bodies
  were (character-animation.md), an eye class branch for the cornea and iris, and the occlusion shell
  as a small blended mesh. Mostly content work; the shading costs little because eyes are small on
  screen.

### 5.3 Budgets (estimates)

| What | RTX 4070, Full | RX 580, Simplified |
|---|---|---|
| Sheen, clearcoat | under 0.1 ms each in a crowd (a few instructions a light, in their classes only) | the same share |
| Skin | screen-space scattering: 0.1 to 0.3 ms typical, 0.5 to 1 ms with a face filling the screen; 30 MB | pre-integrated: under 0.05 ms; nothing extra |
| Transmission | under 0.05 ms (atlas lookups for skin pixels) | the same |
| Eyes, teeth | under 0.05 ms; a few MB of meshes | the same |

### 5.4 Modes, Settings, hardware

- **Full**: screen-space scattering, transmission, sheen, clearcoat. **Simplified**: pre-integrated
  skin, transmission, sheen and clearcoat (they are cheap). There is no Off: sheen and skin are what
  the materials are; Simplified is the cheapest correct form.
- **Settings > Graphics > Light and sky > Skin and cloth** (Simplified, Full): "Full lets light soak
  into skin and spread under it, the softness real faces have; Simplified fakes the spread from how
  curved the skin is, which looks close from a distance." Measured `Gpu(["gpu.skin_scatter"])` and
  `Shares(["gpu.scene"])`, `Memory(["skin.diffuse"])`.
- **Hardware**: none past core.

## 6. Light in real units, exposure, bloom and HDR display

These four belong together and come after the one tonemap (hdr-scene-target.md increments 5 and 6),
because each needs the scene target to hold linear light.

### 6.1 Photometric units and a physical camera

- **The 2030 technique** (Lagarde and de Rousiers 2014, "Moving Frostbite to Physically Based
  Rendering 3.0"): lamps in **lumens**, spot and profiled lamps in **candela** with measured photometric
  profiles (the IES LM-63 file format), the sun in **lux** (about 100,000 at noon in a clear sky), the
  sky and emissive surfaces (screens, lava) in **nits**, light colour as a **temperature in kelvin**;
  a **physical camera** (aperture, shutter, ISO folded into an exposure value) with **eye adaptation**
  from a luminance histogram, and pre-exposure (the exposure multiplied in before the float write) so
  half floats never overflow.
- **How it fits**: `light_types.ron` already carries each lamp's electrical watts; a per-type luminous
  efficacy (an LED panel about 100 to 150 lumens a watt, an incandescent about 15) gives its lumens,
  so the lamp's brightness and its power draw come from the same real numbers, which is the
  educational point. Every fixed exposure constant in the shaders (`ATMO_EXPOSURE`, the water's 15) is
  retired into the one camera. The histogram is a compute pass at quarter size.
- **Cost**: the histogram and adaptation about 0.05 ms (4070), 0.1 ms (RX 580); under 1 MB. The real
  cost is a re-tune of every lamp and the sky, once (question 6).
- **Settings > Image > Exposure** (Automatic, with an exposure compensation slider; Manual for photo
  mode): "Automatic adjusts to the light the way eyes do: stepping out of a dim room the day is too
  bright for a moment." `Free` (a number the shading uses) plus `Gpu(["gpu.exposure"])` for the
  histogram.

### 6.2 Bloom, brought back

- **The technique**: an energy-conserving bloom over a mip chain of the linear image, a 13-tap
  downsample and a tent upsample (Jimenez 2014, "Next Generation Post Processing in Call of Duty:
  Advanced Warfare", SIGGRAPH Advances), weak by default: it is the glow a real lens and eye give
  bright things, not a filter.
- **How it fits**: `bloom.rs` exists, wired for timing (`gpu.bloom`), dormant since its threshold-and-
  Gaussian form was switched off. It is rebuilt in this form after the one tonemap and runs before the
  present.
- **Cost**: 0.2 to 0.4 ms (4070), 0.3 to 0.6 ms (RX 580); about 10 and 6 MB.
- **Settings > Image > Glow around bright lights** (Off, a strength slider): measured `Gpu(["gpu.bloom"])`.

### 6.3 HDR display output

- **The technique**: a 16-bit float swapchain in scRGB on Windows (linear, sRGB primaries, 1.0 = 80
  nits), the scene tonemapped toward the display's own peak brightness (ITU-R BT.2390's EETF or a
  hue-preserving curve) instead of toward 1.0, and the interface drawn at the person's SDR reference
  white (Fry 2017, "High Dynamic Range Color Grading and Display in Frostbite", GDC).
- **How it fits**: wgpu 24's DirectX 12 surface offers `Rgba16Float`, and Microsoft's documentation
  (updated 2025-09-19) says a float swap chain is "treated as if it uses the
  DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709 color space", that is scRGB, so no colour-space call is
  needed. The display's peak and its HDR state come from Windows (`IDXGIOutput6::GetDesc1`), the SDR
  white from `QueryDisplayConfig` ("SDR reference white levels are typically set to around 200 nits",
  the same page). The present pass does the HDR tonemap; egui draws into its own 8-bit target that the
  present composites at the SDR white, because egui draws after the present today and would otherwise
  be written as 80-nit scRGB.
- **Cost**: the present about 0.05 ms more; the float swapchain about 30 to 45 MB more at 2560 x 1440.
- **Settings > Image > HDR display** (Off, Automatic; with peak brightness and interface brightness
  sliders read from Windows): "Uses your display's full brightness range: the sun and lamps can be
  truly bright while dark corners keep their detail. Needs an HDR display with HDR turned on in
  Windows." `Memory(["swapchain"])`, `Gpu(["gpu.present"])`.
- **Hardware notice**: an HDR10 display with Windows HDR on, and (Microsoft's list) "AMD Radeon RX 400
  series (Polaris), or newer" or an NVIDIA GeForce 10 series or newer, so the RX 580 qualifies. Where
  Windows reports SDR, the control is greyed with the reason: "Your display, or Windows, is in
  standard range now. Turn on HDR in Windows' display settings if your display supports it." Never a
  silent failure.

## 7. Decals

- **The 2030 technique**: **clustered decals** evaluated in the forward pass before lighting, binned in
  the same grid as the lights, from one atlas (Persson 2013, "Practical Clustered Shading",
  SIGGRAPH course; Sousa and Geffroy 2016, Doom's decals), so a decal changes the colour, normal and
  roughness the lights then see (a wet patch is darker and shinier; a scuff breaks the highlight).
  Deferred screen-space decals (Persson 2011, "Volume Decals", GPU Pro 2) cannot change what the forward
  lighting already used, so they are not the fit here.
- **How it fits**: decal boxes join the light tile lists (`light_tiles.rs`), indexed in the same tile
  buffer (no new storage binding; a decal buffer can take the seventh); the atlas is one BC7 array
  texture (colour, and normal with roughness), the **first of the two spare bindings**. Decal kinds are
  data (`data/decals/*.ron`: spills, mud, scorch marks from the fire props, footprints, wear on
  thresholds, posters and signs people place), never code. Infinite-of-x: a tile keeps its nearest
  decals up to a cap, the rest are skipped and counted on the Settings line.
- **Cost**: 0.1 to 0.4 ms (4070) with a few decals a pixel; the atlas 4096 square with mips, colour
  and normal layers, about 42 MB (Full), 2048 and about 11 MB (Simplified, near decals only); the
  binning shares the light tiles' processor time (tens of microseconds for hundreds).
- **Settings > Light and sky > Surface marks** (Off, Simplified, Full): "Stains, wear, footprints and
  signs drawn onto surfaces." Computed atlas memory; `Shares(["gpu.scene"])`, `Cpu(["cpu.light_tiles"])`.
- **Hardware**: none past core.

## 8. Surface relief: parallax and displacement

- **The 2030 technique**: virtualized geometry with real displacement for relief you can see the
  silhouette of (Karis, Stubbe and Wihlidal 2021, "Nanite: A Deep Dive", SIGGRAPH Advances; Unreal 5.3's
  displaced Nanite), and parallax occlusion mapping with self-shadowing for the sub-centimetre relief
  every engine still shades rather than models (Tatarchuk 2006, "Dynamic Parallax Occlusion Mapping
  with Approximate Soft Shadows", I3D).
- **How it fits**: WebGPU, and so wgpu, has no tessellation or geometry shaders, so displacement is
  done when meshes are built (a compute pass subdividing and displacing near ground and large relief,
  the way the terrain is already a displaced mesh) and parallax mapping in the fragment for the fine
  scale. The height data is the shared rung: the ground textures already derive one
  (`derive_height`); textured home materials carry one when they get textures. Parallax runs only
  within a distance (about 15 m) and only on materials with a height.
- **Cost**: parallax 0.2 to 0.8 ms (4070) depending on how much ground fills the view, 8 to 32 steps;
  Simplified 4 to 8 steps, 0.1 to 0.3 ms (RX 580); no extra memory beyond the heights.
- **Settings > Light and sky > Surface relief** (Off, Simplified, Full): "Gravel, cobbles, bark and
  brick look raised and cast tiny shadows instead of looking painted on." `Shares(["gpu.celestial",
  "gpu.scene"])` (the world surface pass and the scene pass).
- **Hardware**: none past core.

## 9. Depth of field and motion blur

- **The technique**: both from Jimenez 2014 (the Call of Duty post-processing talk): a scatter-as-gather
  bokeh depth of field at half resolution, and per-object motion blur reconstructed from the motion
  vectors (McGuire et al. 2012, "A Reconstruction Filter for Plausible Motion Blur", I3D; Guertin,
  McGuire and Nowrouzezahrai 2014, "A Fast and Stable Feature-Aware Motion Blur Filter", HPG).
- **Realism note**: the eye refocuses wherever you look and does not smear a turning view the way a
  camera's shutter does, so in play both are a camera's look, not the eye's. Depth of field belongs to
  photo mode, cinematic views and in-world cameras; motion blur to fast objects (fan blades,
  propellers, wheels, a thrown thing) more than to the camera turning (question 4).
- **How it fits**: motion blur needs nothing past section 2's motion vectors; depth of field needs
  the linear image (after the one tonemap) and a focus distance (the thing under the crosshair, or
  photo mode's).
- **Cost**: depth of field 0.5 to 1 ms (4070), 1 to 2 ms (RX 580), about 15 MB; motion blur 0.3 to
  0.6 ms and 0.5 to 1 ms, about 5 MB.
- **Settings > Image > Depth of field** (Off, Photo mode and cutscenes only, Always) and **Motion blur**
  (Off, Moving things only, Everything, with a strength slider): measured `Gpu(["gpu.dof"])`,
  `Gpu(["gpu.motion_blur"])`. Motion blur's hint names that it can cause motion sickness and is off by
  default.
- **Hardware**: none past core.

## 10. Smaller gaps found on the way

- **Ambient occlusion for the scene layer.** Ground-truth ambient occlusion (Jimenez et al. 2016, the
  same course as 4.1) computed from the depth before the scene is shaded (the local shadow plan's
  rung 1b pre-pass), applied to the indirect light only, with specular occlusion from the same
  horizons; it replaces today's SSAO, which shades only the celestial layer and darkens direct light
  too. It can ride a channel of rung 1b's shadow mask, so it needs no binding.
- **Volumetric fog for room lights and smoke**: froxel-based scattering (Wronski 2014, "Volumetric Fog",
  SIGGRAPH Advances; Hillaire 2015, "Physically Based and Unified Volumetric Rendering in Frostbite"),
  so lamps in dusty air and the fire props' smoke catch light; it needs temporal history, so after
  section 3. The fire props design (fire-props-and-combustion.md) sets the smoke's own budget.
- **Order-independent transparency** for glass behind glass, particles and the hair fringe in the
  Off mode: weighted blended OIT (McGuire and Bavoil 2013, JCGT 2(2)).
- **Colour grading** as a look-up table in the one tonemap (hdr-scene-target.md increment 5), so a
  grade is data and the HDR output grades the same way.

## 11. Budgets together (estimates)

| Gap | RTX 4070 at 2560 x 1440, Full | RX 580 at 1920 x 1080, Simplified | Video memory (Full / Simplified) |
|---|---|---|---|
| Motion vectors, jitter | 0.1 to 0.4 ms | 0.2 to 0.6 ms | 15 / 8 MB |
| Anti-aliasing | TAA 0.3 to 0.5 ms | SMAA 0.3 to 0.5 ms | 59 / 17 MB |
| Upscaling (Quality) | saves up to 3 to 5 ms where pixel-bound | saves a similar share | history at display size (counted above) |
| Reflections | 1.3 to 2.5 ms | 0.3 to 0.5 ms | 180 / 30 MB |
| Skin and cloth | 0.2 to 0.5 ms | under 0.1 ms | 30 / 0 MB |
| Exposure | 0.05 ms | 0.1 ms | under 1 MB |
| Bloom | 0.2 to 0.4 ms | 0.3 to 0.6 ms | 10 / 6 MB |
| HDR display | 0.05 ms | 0.05 ms | 30 to 45 / about 20 MB (only on an HDR display) |
| Decals | 0.1 to 0.4 ms | under 0.2 ms | 42 / 11 MB |
| Surface relief | 0.2 to 0.8 ms | 0.1 to 0.3 ms | none extra |
| Depth of field, motion blur (when on) | 0.8 to 1.6 ms | 1.5 to 3 ms | 20 MB |
| **Everything on, without depth of field and motion blur** | **about 2.5 to 6 ms** | **about 1.5 to 3 ms** | **about 400 / 100 MB** |

Read against today's frame: everything Full costs about what upscaling at Quality gives back where the
scene pass is pixel-bound, so the order (upscaling with the first rung) keeps the operator's 120
frames a second in reach while the picture gains. The video memory is about 3% of the 4070's 12 GB;
Simplified's 100 MB fits a 4 GB RX 580.

## 12. The build order, in rungs

Each rung boots and enters the world (`just verify-runtime`), measures its passes through the rig's
same-boot A/B, adds its Settings row with its cost line, and ships its red-twin showcase key.

1. **Temporal anti-aliasing** (sections 2 and 3). First, because upscaling, the hair's single pass,
   stable foliage, the reflection denoiser, volumetric fog and the ray-traced shadow plan's denoiser
   all depend on it.
   1a. Motion vectors from every opaque pass (bodies of all three tiers, wind, ocean, the floating
       origin), jitter, the resolve with sharpening, the responsive mask, the sky sentinel, particles
       and overlays after the resolve, geometric specular anti-aliasing, the deterministic capture
       mode, the reuse switch, the Dev page view; the Anti-aliasing and Sharpening controls.
   1b. Card hair to hashed alpha under Temporal (fixes the player's fringe on a planet); far foliage
       to hashed alpha and dithered crossfades.
   1c. Edges only (SMAA 1x) as the Simplified mode.

### Rung 1a as built (2026-11-13)

`src/renderer/taa.rs`, `assets/shaders/taa.wgsl`; engine rules in [docs/ai/renderer.md](../ai/renderer.md)
(Temporal anti-aliasing). What it contains, against section 2 and 3's plan:

- **Motion from every opaque pass.** Every scene pass carries the motion target beside its colour,
  and every megashader PSO takes its two targets from `taa::scene_targets` (a lint over the pipeline
  builders, `pipeline::taa_motion_tests`, fails on one without). The camera writes it through each
  pass's own previous view-projection; the FLOATING ORIGIN is folded into that matrix: the celestial
  pass's from the locked body's placement (`taa::frame_delta`; on a planet render space is centred on the
  eye and turned with the planet, so without it every pixel streaked), the scene pass's from the home's
  offset (the scene list is the home moved by the station offset; taking the planet's delta for it gave
  a parked room 4 pixels of false motion, caught in the Dev page's motion view); wind-moved vegetation evaluates the wind at the
  previous clock as well (`wind_displace`, moved out of `vs_main`); the ocean its displacement at the
  previous clock (the depth-writing water PSO); an object that moved its previous model matrix
  (`MotionBook`: kept by mesh for meshes drawn once a frame, which includes every near body's region
  meshes, so a person's walk is in it). BODIES, all drawn tiers: a PREVIOUS PALETTE (`skin_upload_prev`)
  holds each person's rows from the last frame, found by their key, their placement carried into this
  frame's render space; the near tier's compute pass skins every vertex by it as well into a last-frame
  copy that its region meshes bind at vertex slot 2 (`mesh::prev_position_layout`; every other mesh
  binds its own buffer there), and the middle tier's `vs_skinned` skins by it directly. The FAR tier
  (baked animation, the plan's "previous frame index") does not exist in the engine yet; it writes
  motion through the same palette when it lands.
- **Deviation: the motion target is Rgba16Float, not Rg16Float.** z carries the responsive mask and w
  the sky sentinel, so neither needs an attachment of its own (8 bytes a pixel, 30 MB at 2560 x 1440).
- **Jitter**: Halton 2,3, eight phases, added in clip space after an unjittered `view_proj`, so every
  shader that reads `view_proj` for anything else is untouched. Not jittered: the shadow passes, the
  cloud octahedral pass, the off-screen views (camera screens, the hi-res capture), and everything
  drawn after the resolve.
- **The resolve and the sharpen** (section 3.2): a compute resolve after the transparent pass
  (closest-depth motion over the 3 x 3, a five-tap Catmull-Rom history fetch, variance clipping in
  YCoCg at gamma 1.25, a blend floor of 1/16 that rises with motion and under the responsive mask) and
  a contrast-adaptive sharpen as a fragment pass back into the scene target, so the target needs no
  storage usage. The overlay, the lines and the particles draw after it, unjittered. Karis's luminance
  weighting is in the shader behind one flag, off while the target holds display values (increment 5
  of hdr-scene-target.md turns it on).
- **The sky sentinel and the responsive mask** (section 2, 3.2): cleared motion (w = 0) reprojects
  through the celestial camera's turn alone; blended surfaces (glass, the water's surface, the
  atmosphere, the cloud shell) write their coverage into z with a Max blend.
- **The reuse switch**: with it off the history and the motion book are discarded every frame and the
  picture aliases as Off does. **The deterministic capture**: a rig window capture resets the history
  and waits `SETTLE_FRAMES` (16) resolved frames.
- **Geometric specular anti-aliasing** (Kaplanyan et al. 2016, Tokuyoshi and Kaplanyan 2019): where the surface normal turns fast inside a pixel the GGX lobe is widened by the normal's screen-space change (alpha squared plus min(2 x 0.25 x |dn|^2, 0.18)), taken in `frag_prologue` before any non-uniform branch and used by `evaluate_light`. It rides Temporal in 1a (`camera.taa.z`), so Off draws exactly the pre-1a picture; it uses the geometric normal, so normal-map sparkle is not yet filtered (a later refinement filters the mapped normal).
- **Dev page > Image**: the motion target, the history's weight and the responsive mask in place of the
  picture (showcase `taa_view`).
- **Settings > Graphics > Image**: Anti-aliasing (Off, Temporal; Temporal by default) and Sharpening
  (0.35 by default), with the cost rows of section 3.6 (`gpu.taa`, `vram.taa_history`, the motion's
  share of `gpu.scene` and `gpu.celestial`), the history's memory at the window's size against the
  card's own, and the vendor-upscaler sentence of section 3.6. No card is refused (section 3.7).

**Measured** (same boot, 2026-11-13, through `probe-hot-ab.js` with showcase-only arms alternating Off and Temporal three times each at one park per vantage (medians of three), the RTX 4070 at 2560 x 1387 (the rig window), GPU timestamps; CPU stages were wall-clock under other agents' compiles and are not quoted):

| Vantage | `gpu.taa` (Temporal) | `gpu.scene` Off / Temporal | `gpu.celestial` Off / Temporal | GPU passes summed, Off / Temporal |
|---|---|---|---|---|
| shadow-hall-chair-night (the ground hall) | 0.275 ms (0.274, 0.275, 0.276) | 13.47 / 13.46 | 0.09 / 0.09 | 19.53 / 19.79 |
| anim-crowd-near (fifty people) | 0.279 ms (0.278, 0.279, 0.282) | 8.74 / 8.79 | 0.06 / 0.06 | 15.35 / 15.66 |
| horizon-line-3km (a planet horizon) | 0.248 ms (0.249, 0.248, 0.246) | 0.46 / 0.46 | 8.85 / 8.97 | 29.51 / 29.88 |
| anim-look-face-female (hair close up) | 0.275 ms (0.282, 0.275, 0.275) | 9.41 / 9.40 | 0.06 / 0.06 | 15.52 / 15.77 |

The resolve and the sharpen together cost 0.25 to 0.28 ms at 2560 x 1387, under section 3.4's estimate of 0.3 to 0.5 ms, and the history holds 54.2 MiB (two Rgba16Float frames). The scene and world surface passes are the same in both arms within the run-to-run spread, as they must be: the motion target is written in both (the PSOs carry it whatever the mode), so this A/B prices the resolve, not the motion; the world surface pass's 0.12 ms at the horizon is the geometric specular anti-aliasing's derivatives (on only under Temporal) or noise. What the motion target costs against a build without it was NOT measured in one boot (two builds, two boots); section 2's estimate (about 0.1 ms, 30 MB here) stands until a rung measures it. Captures: Off against Temporal at all four vantages, the motion view and the history weight view (the session's scratchpad, named in the rung's report).

**What 1a leaves for 1b and 1c.** 1b: the card hair stays two passes (the opaque core and the blended
fringe, which writes only the responsive mask, so the fringe is held less in the history but still
trails a little and still cannot appear on the player's body on a planet); far foliage keeps its hard
alpha cut and the level-of-detail crossfade its Bayer dither. Both become hashed alpha under Temporal
in 1b. 1c: "Edges only" (SMAA 1x) is not in the modes list yet (`taa::MODES` is Off and Temporal), so
there is no Simplified mode for anti-aliasing until 1c. Also left: the motion target is written at Off
too (the PSOs have it whatever the mode; its cost at Off is the row above), the far tier's motion (no
far tier yet), and the camera screens stay without anti-aliasing by design (section 3.2).

### Rung 1b as built (2026-11-13)

`src/renderer/hashed_alpha.rs` (the CPU twin, its tests, the fringe skip); in the shaders
`00-bindings-vertex.wgsl` (`taa_hashed`, `taa_hash_threshold`), `80-fragment-shared.wgsl` (the
crossfade) and `90-fragment-main.wgsl` (the hair, the far cards); engine rules in
[docs/ai/renderer.md](../ai/renderer.md) (Hashed alpha). Against section 3.3:

- **The threshold.** Wyman and McGuire's hashed alpha test, with a screen-space hash in place of
  their object-space one: Jimenez's interleaved gradient noise (2014), spread evenly over every 3 x 3
  neighbourhood (the resolve's clip box, so a half-covered patch keeps its history), moved by
  (phase x 5 mod 8) / 8 with the jitter phase, so over the eight phases a still pixel takes one value
  in each eighth of [0, 1) and repeats every eight frames. `camera.taa_hash` (x the phase, y on) is a
  new row at the camera uniform's tail, stamped beside `taa`.
- **Card hair in one pass.** At or over the cut (0.8) a card fragment is solid as before; under it,
  kept where its alpha beats the threshold (never under 0.03, the fringe pass's own floor), so the
  expected coverage is exactly what the two passes blended. The fringe pass is left out of the main
  view (`Renderer::hashed_skips`, read from the same frozen flag the camera stamps, so the two never
  disagree). The fringe now writes depth and motion, needs no sorting, and **the player's own fringe
  shows on a planet** (vantage `taa-hair-planet-third` and its red twin `-two-pass`, scored by
  `scripts/hair-fringe-metrics.mjs`).
- **The far tree cards** (type 12): the sprite's alpha times the far band's coverage is tested
  against the threshold instead of a hard cut at 0.5, the band being the stage's last tenth before
  "Tree silhouette distance" (`CARD_FAR_BAND`), so the stage dissolves out instead of ending at a ring.
- **The level-of-detail crossfade** (terrain patches, the near trees fading in): the threshold in
  place of the 4 x 4 Bayer table; the rising and the falling level share it, so they cover each pixel
  exactly once and the resolve turns the dissolve into a blend.
- **Where it stays off**: anti-aliasing Off; the showcase `taa_hashed` "0" (the red twin: Temporal
  with rung 1a's picture); the reuse switch off (no history to average the pattern); the off-screen
  views (the camera screens and the hi-res capture draw the fringe pass as before); and the shadow
  passes: the Bayer table and the hard cuts stay in the `else` branches, which `fs_shadow` mirrors and
  `shadow_cutout_tests` reads.

**Measured** (same boot, 2026-11-13, `probe-hot-ab.js` with showcase-only arms alternating Temporal
hashed, Temporal two-pass (`taa_hashed` 0) and Off three times at one park per vantage, medians of
three, the RTX 4070 at 2560 x 1387, GPU timestamps; other agents compiled throughout, so CPU stages
are not quoted):

| Vantage | `gpu.scene` hashed / two-pass | `gpu.transparent` hashed / two-pass | `gpu.celestial` hashed / two-pass |
|---|---|---|---|
| anim-look-face-female (hair close up) | 9.409 / 9.399 | 0.670 / 0.751 | 0.063 / 0.063 |
| anim-crowd-near (fifty people) | 8.801 / 8.845 | 0.701 / 0.751 | 0.063 / 0.063 |
| taa-hair-planet-third (the player's hair on a planet) | 0.637 / 0.635 | 0.009 / 0.009 | 5.283 / 5.268 |
| fuji-forest-ground | 0.469 / 0.469 | (none) | 12.536 / 12.588 |
| fuji-forest-hillside | 0.467 / 0.467 | (none) | 7.497 / 7.513 |
| horizon-line-3km | 0.471 / 0.471 | (none) | 9.219 / 9.270 |

In the ship's rooms the hair now costs less: the fringe pass's 0.05 to 0.08 ms leaves the
transparent pass and the opaque pass absorbs the fringe within its spread. On a planet the player's
fringe, drawn now, adds about 0.015 ms to the world surface pass at a 35-degree close-up of the head.
The terrain and the forest read the same within the spread (the crossfade's threshold costs no more
than the Bayer table). One earlier run at a 20-degree lens over bare sand read about 0.1 ms more
in the world surface pass under hashed in all three repeats (5.13 against 5.01 ms) with no hair or
tree in the frame; the 35-degree run of the same park did not repeat it, so it is recorded here as
unexplained, not as a cost.

**Judged.** Stills: at the face close-up the hashed hair reads the same density and softness as the
two passes, and the lashes a little stronger (the blended pass drew them faintly). On the planet the
bob gains its soft outline: the head's covered area against the two-pass twin was +208, +362 and
+211 px of about 13,400 in the three repeats, where the control (Off against two-pass, the same hair
passes) read -216, +585 and -234. Motion: consecutive captures of the idling faces differed by mean
5.2 and 2.9 codes under hashed, 6.1 and 3.0 under two-pass and 22.7 and 5.7 at Off, so hashed alpha
adds no shimmer of its own; the crowd's walkers show no stipple on their hair.

**What 1b leaves.** The far tree cards could not be judged on screen: no sprite card drew at the
forest vantages in any arm, Off included, at the rig's settings (the near models stood to about
93 m and the slope beyond was bare), so that branch is proven by the twin and the shader validation
only. (Judged 2026-11-14 once BUG-301 put the stage back: no release had ever drawn it, because the
cards need depth-15 patches and only the tile tier reached them. At `fuji-forest-far` the cards thin
out over the last tenth under Temporal and run to a hard edge at Off, and the dissolve is still:
consecutive captures there differ by a mean of 0.07 codes under Temporal against 0.08 at Off. The
stage costs 7 to 10 ms of `gpu.celestial` at the forest vantages, so the forest rows of the table
above are frames without any card.) The hair's sun and room-light shadows still cast the core alone (the shadow passes have no
history; section 3.3's "casts with its true coverage" needs a pattern that PCF, not a history,
averages). Stacked hair cards share one threshold per pixel, so two overlapping half-transparent
layers cover like one; Wyman and McGuire's object-space hash would decorrelate them, at the price of
white rather than evenly spread noise. The near trees' leaves (types 19 and 21) keep their hard cuts,
and the card-to-model hand-off at the hide radius stays a hard per-pixel line (a true crossfade there
needs the models to fade out over the same distance band). And 1c, "Edges only".
2. **Upscaling**: render scale as data, TAAU in the same resolve, the texture bias; then port one
   FSR 3.1 pass and compare it with our TAAU on the hair, foliage and crowd vantages against a 4x
   supersampled capture, and port the rest only if it wins.
3. **Reflections** (section 4).
   3a. The thin G-buffer, the sky cube and the specular composite, with energy compensation: every
       outdoor metal and wet surface gains its sky.
   3b. Room captures, box-projected, versioned, budgeted; glass and water read them.
   3c. Screen-space reflections with the depth pyramid and the denoiser (Full).
   3d. Ray-traced reflections where ray queries exist, after wgpu gains DirectX 12 ray queries.
4. **Skin and cloth** (section 5).
   4a. Sheen and clearcoat lobes as material data (and anisotropy for satin and brushed metal).
   4b. Pre-integrated skin and thin-part transmission through the shadow atlas.
   4c. Screen-space skin scattering (Full).
   4d. Eyes, teeth and tongue converted, with the eye shading and the occlusion shell.
5. **Light and image** (section 6), after hdr-scene-target.md's increments 5 and 6.
   5a. Photometric units and the physical camera with eye adaptation; the one re-tune.
   5b. Bloom rebuilt.
   5c. HDR display output.
6. **Decals** (section 7): clustered decals and their data; then the systems that place them.
7. **Surface relief** (section 8): parallax on the near ground, then on textured home materials, then
   displacement at mesh build.
8. **Depth of field and motion blur** (section 9), with photo mode.
9. **The smaller gaps** (section 10), the ambient occlusion first, alongside the local shadow plan's
   rung 1b.

## 13. Questions only the operator can answer

1. **Vendor upscalers.** DLSS, XeSS and FSR 4 are binaries that may not be modified (DLSS only on
   NVIDIA cards), so the open-source rule keeps them out of the core. Would you accept any of them as
   an optional, separately downloaded plugin for people who own that hardware? Recommendation: no
   before v1.0: our own temporal upscaling (and FSR 3.1's open source, if it measures better) works on
   every card, a plugin is a second path to maintain, and the Settings page says plainly why they are
   not offered.
2. **Temporal anti-aliasing as the default.** It steadies hair, grass and leaves but softens the picture
   slightly, and some players dislike that. Recommendation: Temporal on by default with a moderate
   sharpening, Edges only and Off one click away, each saying what it changes.
3. **The render scale default on your card.** Native keeps the most detail; Quality would likely bring
   the busy rooms back toward 120 frames a second. Recommendation: Native by default everywhere (the
   max-graphics default the other controls use), with the line showing the frame time so picking
   Quality and reading the line shows what it saves.
4. **Depth of field and motion blur in play.** Neither is how eyes see. Recommendation: both off in
   play by default; depth of field on in photo mode, cutscenes and in-world cameras; motion blur
   offered for moving things only, opt-in.
5. **HDR on your television.** Do you run Windows with HDR on for the TV you use as a monitor? It
   decides how soon HDR output is worth building and how it gets tested. Recommendation: build it as
   "Automatic" (on when Windows reports HDR on, the peak and the interface brightness read from Windows
   and adjustable), after the one tonemap and the photometric units.
6. **The photometric re-tune.** Real units change every lamp's number and, once, how every room and the
   sky look (brighter or dimmer where today's multipliers were guessed), and the lamps' brightness then
   follows their real watts. Recommendation: accept the one-time change, in one rung right after the
   one tonemap, with before-and-after captures of the canonical vantages for you to judge.

## His answers (operator, 2026-11-12)

- **Scope:** "Let's make sure we properly fully develop everything and include everything." Every gap in this plan is to be built, in the order above.
- **DLSS, XeSS or FSR 4 as an optional download:** "I agree with you." (No before v1.0.)
- **Temporal anti-aliasing on by default:** "Yes."
- **Render scale:** "I run my TV at 2560x1440 because it gets 120fps. 4k gets 60fps. I prefer the higher framerate. I prefer native resolution with no upscaling." So the default is native resolution with no upscaling; upscaling is built as a choice for cards that need it, its cost line showing what it saves.
- **Depth of field and motion blur:** "Agreed. Both off by default."
- **HDR display output:** "I don't but, I could. My brother's OLED on his laptop does HDR." Build it as Automatic (on when Windows reports HDR), testable on an HDR laptop; it stays in the plan, after the single tonemap.
- **The one-time re-tune when lights move to real units:** "Yes." With before-and-after captures of the standard viewpoints.
