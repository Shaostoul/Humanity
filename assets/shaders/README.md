# Shader Library

All shaders use WGSL (WebGPU Shading Language) and follow consistent binding conventions.

## Binding Group Convention

| Group | Purpose | Typical Contents |
|-------|---------|-----------------|
| 0 | Camera/view | view_proj, camera position |
| 1 | Object/mesh | model matrix, mesh transform |
| 2 | Textures OR lights | texture/sampler pairs, or light arrays |
| 3 | Material + extras | material uniforms, additional lights |

## Standard Vertex Layout

All meshes use: position (vec3), normal (vec3), tangent (vec3), bitangent (vec3), uv (vec2).

---

## Core Shaders

| Shader | Purpose |
|--------|---------|
| `basic.wgsl` | Minimal passthrough for debug geometry and colored meshes |
| `pbr.wgsl` | Full PBR with albedo, metallic, roughness, normal map, AO, emission textures |
| `procedural_material.wgsl` | Zero-texture PBR foundation -- all properties generated from noise |
| `ghost_preview.wgsl` | Semi-transparent placement preview overlay |

## Space / Celestial

| Shader | Purpose |
|--------|---------|
| `star.wgsl` | Point-based stars with magnitude-driven brightness |
| `constellation_lines.wgsl` | Vertex-colored line overlays for constellations |
| `sun_surface.wgsl` | Animated solar surface: granulation, filaments, sunspots, corona |
| `sun_glow.wgsl` | Screen-space corona billboard with animated filament rays |

## Planet Shaders

| Shader | Purpose |
|--------|---------|
| `planet_surface.wgsl` | Generic configurable planet (rocky/ocean/desert/ice via uniforms) |
| `planet_clouds.wgsl` | Animated cloud layer overlay (semi-transparent sphere) |
| `mercury.wgsl` | Cratered, airless, temperature-variable surface |
| `venus.wgsl` | Thick sulfuric acid clouds, volcanic plains |
| `earth.wgsl` | Continents, oceans, biomes, atmospheric scattering |
| `moon.wgsl` | Craters and dark maria (basaltic plains) |
| `mars.wgsl` | Red surface, volcanoes, dust storms, polar ice caps |
| `jupiter.wgsl` | Gas giant bands, Great Red Spot, white ovals |
| `saturn.wgsl` | Pale yellow bands, hexagonal north pole storm |
| `uranus.wgsl` | Blue-green ice giant, subtle bands, polar regions |
| `neptune.wgsl` | Deep blue, Great Dark Spot, methane ice clouds |
| `pluto.wgsl` | Nitrogen ice plains (Tombaugh Regio), tholin highlands |

## Procedural Materials

All use PBR lighting (Cook-Torrance BRDF) with up to 16 point lights.

| Shader | Purpose |
|--------|---------|
| `steel.wgsl` | Industrial steel -- uniform metallic surface |
| `granite_tile.wgsl` | Speckled granite with grout lines |
| `hexagon_marble.wgsl` | White marble hexagonal tiles with veining |
| `stone_sapphire.wgsl` | White stone with sapphire blue veins |
| `polished_concrete.wgsl` | Polished concrete with aggregate and expansion joints |
| `plank_wood.wgsl` | Wood plank flooring with grain and per-plank variation |
| `carpet_tile.wgsl` | Office carpet with fiber texture and tile seams |
| `rubber_flooring.wgsl` | Lab/gym rubber with grip dots |
| `drywall.wgsl` | Painted drywall with paper texture and brush strokes |
| `astro_turf.wgsl` | Artificial grass with blade patterns and seams |
