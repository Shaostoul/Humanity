# Verified free asset + planetary map sources (2026-07-19)

Research pass for two needs: real plant models for the gardens, and real
planetary imagery for Moon/Mars/Pluto. Every license below was verified
against the live page on 2026-07-19. NOTHING here has been downloaded -
links only, per the operator's instruction; the operator reviews and
downloads personally.

## Plant / crop 3D asset packs (commercial use OK)

### Tier 1 - CC0 (public domain, no attribution, no strings)

1. **Quaternius - Ultimate Crops Pack** - THE crop pack.
   https://quaternius.com/packs/ultimatecrops.html
   (glTF via https://poly.pizza/m/Ro6K0Yg7mx , mirror
   https://opengameart.org/content/lowpoly-crops-pack )
   100+ crop models in 5 GROWTH STAGES (lettuce, pumpkin, apple, more).
   The growth stages map directly onto the farming system's stage ticks.
2. **Kenney - Nature Kit** - https://kenney.nl/assets/nature-kit
   (329-model GLB repack:
   https://eclair-assets.itch.io/nature-kit-glb-pack-329-free-cc0-3d-models )
   330 trees/foliage/rocks/terrain props, cohesive low-poly style.
3. **Quaternius - Ultimate Nature Pack**
   https://quaternius.com/packs/ultimatenature.html - 150 nature models,
   style-matches the crops pack (same author).
4. **KayKit - Forest Nature Pack** - https://kaylousberg.itch.io/kaykit-forest
   "Free for personal and commercial use, no attribution required (CC0)".
   Free base tier only; the "Extra" tier is paid.
5. **Poly Haven models** - https://polyhaven.com/models/plants +
   https://polyhaven.com/models/grass - photoscanned, CC0
   ( https://polyhaven.com/license ). HEAVY (up to 2M tris) - decimate
   before shipping; best as hero pieces or texture reference.
6. **OpenGameArt CC0 3D plants collection**
   https://opengameart.org/content/cc0-3d-plants - grab-bag; check
   format per item.
7. **ambientCG** - https://ambientcg.com/list?type=3DModel - CC0
   ( https://docs.ambientcg.com/license/ ). Produce props (apple, pear,
   lemon) + leaf/grass/flower TEXTURE atlases (useful for crop
   billboards and ground texturing).
8. **Takyin - Simple Vegetation Pack**
   https://takyin.itch.io/simple-vegetation-pack - small CC0 filler.
9. **Kenney - Survival Kit** - https://kenney.nl/assets/survival-kit -
   CC0 campsite/homestead props.

### Tier 2 - CC-BY 4.0 (free + commercial, ATTRIBUTION REQUIRED - a
credits-page line satisfies it)

10. **LOLIPOP - "Farm plants models" (Sketchfab)** - the exact vegetable
    roster: POTATO, carrot, tomato, onion, corn, wheat, peas, pumpkin +
    a planting-bed mesh, game-ready poly counts.
    https://sketchfab.com/3d-models/farm-plants-models-mobile-game-ready-lowpoly-9b59e7cf3bcd4feb80ff0cf5fb780055
11. **Bumroker - "Crops (low poly)" (Sketchfab)** - tomato/potato/
    carrot/wheat each in 3 GROWTH STATES; one big scene file, needs a
    one-time Blender split.
    https://sketchfab.com/3d-models/crops-low-poly-637198c4b8be41e6aeaca8cea8fb81f4
12. **Poly Pizza (aggregator)** - https://poly.pizza/ - glTF downloads,
    license shown per model (Quaternius = CC0; archived Google Poly =
    CC-BY 3.0).

### Checked and REJECTED (do not use)

- Luceed Studio "Farm Crops 01" (Sketchfab): no license shown, routes to
  the paid FAB marketplace.
- "Grow a Garden Crops" (Sketchfab): fan recreation of Roblox game
  assets - the uploader likely cannot grant the license. IP risk.
- Artisau Gardening Pack (itch.io): good but $2.99 paid (operator may
  still want it).

**Best three for the vegetable gardens:** Quaternius Ultimate Crops
(CC0), LOLIPOP Farm plants (CC-BY, exact roster + bed), Bumroker Crops
(CC-BY, growth states).

## Planetary imagery (public domain, for the albedo bake pipeline)

US government works - public domain, free for commercial use. The bake
path is `scripts/build-earth-albedo.js` -> `HOSALB1` `.bin` -> the
`PlanetDef.albedo` field; the loader is body-agnostic (integration
details + gotchas G1-G3 in the 2026-07-19 journal entry).

- **Moon** - LRO/LROC WAC global morphology mosaic 100 m/px:
  https://astrogeology.usgs.gov/search/map/moon_lro_lroc_wac_global_morphology_mosaic_100m
  Color-shade variant:
  https://astrogeology.usgs.gov/search/map/moon_lroc_wac_gld100_colorshade_79s79n_118m
- **Mars** - Viking MDIM 2.1 colorized global mosaic 232 m/px:
  https://astrogeology.usgs.gov/search/map/mars_viking_colorized_global_mosaic_232m
- **Pluto** - New Horizons global mosaic 300 m/px:
  https://astrogeology.usgs.gov/search/map/Pluto/NewHorizons/Pluto_NewHorizons_Global_Mosaic_300m_Jul2017
  NASA color map page: https://science.nasa.gov/resource/pluto-global-color-map/
- **Convenience (CC-BY 4.0, NOT public domain)** - Solar System Scope
  2K/4K/8K equirect JPEGs for all bodies incl. gas giants:
  https://www.solarsystemscope.com/textures/

Full-resolution USGS mosaics are multi-GB GeoTIFFs; reduced-resolution
versions exist on each page. The bake script only decodes 8-bit
non-interlaced PNG today - convert first or generalize the decoder.

Gas giants need no imagery: the planned type-18 procedural band shader
(latitude-ramp palette + noise warp, per-giant colors, also fixes the
uranus/neptune ochre bug) is designed in the 2026-07-19 journal entry.
