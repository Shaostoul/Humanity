# World-space line overlays (the "orbit path" primitive)

> Reusable since v0.568. Lives in `src/renderer/line.rs`. The operator asked us to keep this idea
> handy because it's "consistently useful" -- this doc is the pointer so we reach for it instead of
> reinventing a polygon strip every time.

## What it is

A **constant-width world-space line** renderer. You build a `Vec<LineVertex>` (world position +
RGBA per endpoint) and the renderer draws it as GPU `LineList` primitives via
`Renderer::draw_lines_onto(&camera, &lines, &view)`. It reuses the MAIN camera (full view-proj +
floating origin, so lines sit exactly where world geometry is) and the SAME reverse-Z depth buffer
the scene wrote -- but **does not write depth**. So a line is occluded by nearer solid geometry
(the "fade behind the planet/wall" cue) while overlapping lines never fight each other.

Two append helpers (both take `out: &mut Vec<LineVertex>`):

- `push_circle(out, center, radius, color, segments)` -- a horizontal circle in the XZ plane.
- `push_polyline(out, points, color)` -- an open connected path through arbitrary 3D points.

## Why a line, not a polygon ring/strip

A polygon ring (a `flat_ring` mesh scaled to a radius) has a **band thickness that scales with its
radius** -- the operator saw a door's auto-open ring get visibly thicker as the radius grew. A GPU
line is **~1px regardless of distance or size**, and it's a fraction of the vertex count (2 verts
per segment, no tube). When you want a boundary/path that reads as a thin clean stroke at any scale,
use this. When you want a filled/extruded surface, use a mesh.

## Where it's used today

- Solar-system **orbit paths** on the Maps/skybox (the original use, v0.262.20).
- **Constellation lines**.
- A door's **auto-open radius ring** (`render_door_panels`, v0.565).
- **Corner angle-circles** + the **selected-machine highlight ring** in the construction editor
  (v0.568) -- these replaced their `flat_ring` polygon meshes.

## Reuse candidates (operator-flagged + obvious)

Reach for `push_circle` / `push_polyline` (not a new mesh) for any of these when we build them:

- **Thrown-object trajectory preview** -- the parabola a grenade/rock will arc along before you
  release it (`push_polyline` over the sampled ballistic path; recolor red near impact).
- **Gun laser-pointer / aim beam** -- a `push_polyline` from the muzzle along the aim ray to the hit
  point (or a fixed length).
- **Area-of-effect / range markers** -- blast radius, sensor range, turret reach, build-snap radius
  (`push_circle`).
- **Planned travel routes / waypoint paths**, patrol loops, conduit centerline previews.
- Any **gizmo bound** that today is (or would be) a scaled ring mesh -- convert it.

## How to add a new overlay

1. Get (or make) a `&mut Vec<LineVertex>` that's drawn this frame -- in the home/construction render
   path the shared buffer is `ring_lines` (drawn once via `draw_lines_onto` after the overlay pass).
2. `push_circle` / `push_polyline` your geometry into it with a color (alpha blends).
3. That's it -- no mesh, no material, no pipeline. Depth-occluded by solid geometry, not by other
   lines. If you need it to draw THROUGH walls (like the corner orbs do), that's a separate overlay
   pass concern -- lines are depth-tested against the scene by design.
