# Renderer Architecture (Rust Engine, No Bevy)

## Goals
- Highest practical fidelity with minimal install size.
- Scalable quality tiers from ultra-low to ultra.
- Deterministic simulation decoupled from render fidelity.

## Proposed Pipeline
- API backend: Vulkan (primary), DX12/Metal abstraction later.
- Frame graph renderer with explicit passes.
- Deferred+forward hybrid:
  - Deferred for opaque world geometry.
  - Forward for transparents, particles, UI overlays.
- Temporal upscaling + TAA/FSR-class upscaler.
- Clustered/forward+ lights for dense ship interiors.

## Material Strategy
- Procedural-first materials (noise/masks/parameterized shaders).
- Small texture atlases for essentials only.
- Triplanar/signed-distance detail to reduce UV dependence.

## World Integration
- Sector streaming (ship districts, transit shafts, public hubs).
- LOD + HLOD with smooth transitions.
- Occlusion culling and visibility cells for interior-heavy scenes.

## Quality Tiers
- Ultra Low: unlit/flat color, no post, tiny shadow budget.
- Low: basic PBR, single directional shadow cascade.
- Medium: full PBR, selective SSAO/bloom.
- High: advanced reflections/GI approximations.
- Ultra: max draw distance/effects within budget.

## Needs Decision
- Primary graphics API target for v1 (Vulkan-only vs multi-backend).
- Preferred antialiasing path (TAA-first or SMAA-first fallback).