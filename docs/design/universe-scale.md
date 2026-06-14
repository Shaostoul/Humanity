# Real-scale universe (vision)

Status: vision capture (operator, 2026-06-14). HumanityOS is "very particular about a
realistic scale to the universe." This is the north star for the renderer + world systems;
pieces ship incrementally.

## The goals

- **Real scale + real distances.** Earth's real radius, the Sun's real size + distance, real
  inter-star distances. The `src/cosmos` module already computes true Keplerian positions and
  real distances; the gap is RENDERING across that enormous dynamic range, not the data.
- **Planet generator for ANY size, without killing computers.** A planet generator (the
  icosphere planet in `src/terrain/planet.rs` already does distance LOD) that scales from a
  small moon to a gas giant, streaming detail by distance so the framerate holds. Real-world
  terrain fidelity is the target (e.g. the "mountain" showroom backdrop literally on Mount
  Rainier once heightmaps land, per the room-purpose doc).
- **Recreate the Milky Way in the skybox.** Beyond the 119k HYG point-stars (now rendering,
  v0.446), reproduce what makes the real Milky Way visible: the galactic-plane band, dust
  lanes, nebulae, and large-scale structure. Built from real catalogs/structure where
  possible (the same data-driven, real-position ethos as the star catalog).
- **Vision filters (naked-eye vs enhanced).** What a character can SEE depends on their gear:
  the naked human eye sees only the bright stars; a space helmet, cybernetic eyes, or sensors
  reveal more (faint stars, IR/UV, nebula detail, structure invisible to the eye). A "vision
  mode" gates what the skybox + scene render and how bright, tied to equipped cosmetics/gear.

## The enabling technical problem: depth range

The blocker for ALL of the above is the classic interior-scale + solar-scale + galactic-scale
depth-range problem. The gameplay camera far plane is ~500 m (for the room interior), which
clips everything astronomical: the star skybox was clipped until v0.446 gave the STAR PASS its
own 100,000-unit projection. The planet (Earth at GEO ~42,000 km), the solar bodies (millions
of km), and the orbit rings (AU-scale) are STILL clipped by the 500 m gameplay far.

The fix is a layered / dedicated CELESTIAL render pass with its own far plane (and likely
logarithmic or split depth), rendering distant bodies behind the interior scene, the same way
the star pass now renders behind everything. Order of bands, near to far:
1. Interior / local scene (~500 m far) -- the home, machines, avatar.
2. Celestial bodies pass -- planet + sun + solar bodies + orbit rings, huge far, drawn behind
   the interior (depth-composited or painter-ordered). Distant bodies keep the floored angular
   size already in the code so they read as discs.
3. Star skybox + (future) Milky Way structure -- rotation-only, effectively infinite.

This celestial pass is the next renderer increment; it also unblocks the showroom "stand on
Earth/Mars" backdrop (or, simpler first, a LOCAL placeholder sphere under the avatar). Vision
filters then layer on top as per-band brightness/visibility toggles driven by equipped gear.
