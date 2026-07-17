# Terrain detail, cliffs, caves, Agartha (design notes, 2026-07-17)

> Operator: "If we can find some way to replicate Earth without somehow using
> more than 10GB that'd be amazing. Maybe something to do with plate tectonic
> barriers as to drive terrain detail generation. I don't know how we'll get a
> sheer cliff like the White Cliffs of Dover. Most games seem to do fine with
> rolling hills but, struggle with cliffs. Especially with tunnels/caves. It
> would be cool if we could recreate Agartha in-game. Maybe not on Earth but,
> definitely the main story arc artificial planet."

## Earth under 10 GB (the budget works)

- ETOPO 2022 @ 15 arc-sec, whole globe, int16 metres: 86400x43200x2 = 7.5 GB
  raw. Delta-encoded + compressed region tiles: **~2-3 GB on disk**.
- Ocean floor is most of the planet and is smooth at 15s: store abyssal tiles
  at a coarser level of the same pyramid (quadtree tiles already imply this) -
  realistic total closer to **1.5-2 GB**.
- Below 460 m the last mile is PROCEDURAL (seeded, deterministic) - costs
  zero storage. Verdict: full-fidelity-feeling Earth in well under 10 GB.

## Tectonics as the procedural detail driver (operator idea - keep)

The sub-data procedural layer should not be uniform noise. Real terrain
character follows plate boundaries: young convergent margins are sharp
(Andes, Himalaya), old shields are smooth (Canadian, Baltic), transforms
shear, rifts drop. Plan: a tiny static dataset of plate boundaries (public
domain, a few hundred KB as polylines) -> precomputed distance-to-boundary +
boundary-type field at low resolution -> modulates the procedural layer's
amplitude/sharpness/ridge-ness. Young margin: ridged, high-amplitude
wrinkles; shield: gentle rolling; rift: stepped normal faults. This is how a
125 m procedural layer stays PLAUSIBLE instead of generic.

## Cliffs (Dover) - why games fail and what we do

A heightmap stores one elevation per (lat,lon): overhangs are impossible and
smooth interpolation (incl. our bicubic) rounds sharp edges into ramps.
Strategy, cheap to expensive:
1. **Data first**: at 15s (460 m) Dover's ~100 m wall is still sub-cell; at a
   hero-tile 30 m level it IS several cells - most famous cliffs come back
   with data alone.
2. **Edge-preserving sampling**: detect high-gradient cells (a cliff mask,
   same pipeline as the ocean mask) and switch interpolation from smooth
   bicubic to edge-respecting (no smoothing ACROSS the scarp line) so the
   wall stays a wall instead of a 2-km ramp.
3. **Scarp meshes**: where the cliff mask fires, the patch mesher inserts a
   near-vertical skirt face along the scarp polyline (still not an overhang,
   but visually a true cliff). Chalk-white material comes from the same mask.
4. **True overhangs**: heightmaps never do these - that is voxel/SDF
   territory (below).

## Caves, tunnels, Agartha

The engine ALREADY has the right second representation: the voxel asteroids
(sparse octree, `src/terrain/`). Caves and tunnels are not a heightmap
feature and should not pretend to be:
- **Hybrid carving**: local voxel/SDF volumes embedded in the heightmap
  world where digging/caves happen (mines, lava tubes, bunkers). The
  heightmap surface hands off to the voxel volume at a portal boundary -
  exactly how the mining loop's asteroids already work, applied planetside.
- **Agartha**: reserved for the MAIN STORY ARC ARTIFICIAL PLANET (operator:
  "maybe not on Earth"). An artificial world is the perfect fit: its "geology"
  is authored, so the interior (a hollow shell world with its own inner
  surface, structural pillars, sky-core light source) can be a designed
  RON-driven megastructure - the ship/room pipeline at planetary scale plus
  voxel volumes for the wild parts. No real-Earth data constraints apply.
  Story hook: the artificial planet IS the fleet's destination mystery.

## Order of operations

1. (running) ETOPO 2022 15s tiles downloaded to ext_data/.
2. Rebuild the shipped base grid at 0.05 deg from the new data (+ restore
   true peak window; Everest stops being clamped).
3. Region tile pyramid + streaming through the existing patch quadtree
   (Fuji becomes a cone; Dover comes back at hero-tile levels).
4. Cliff mask + edge-preserving sampling (2 above).
5. Tectonic-modulated procedural layer.
6. Voxel-hybrid carving; Agartha designs move to the artificial-planet doc
   when the story arc starts.
