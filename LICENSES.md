# Third-party data and licences

HumanityOS ships real-world measurements, not invented ones. Some of the sources
carry obligations. This file records them for anyone redistributing the repo or a
release bundle; the in-app **Settings > Credits** page shows the same list to
players, generated from `data/credits.ron`.

If you add a data source, add a row to `data/credits.ron` in the same commit. A
unit test (`crates/humanity-core/src/credits.rs`) fails if a source marked `attribution_required`
does not name a surface where its notice is actually shown.

## OpenStreetMap - the one with real obligations

The whole-Earth map packs, `data/maps/packs/*.hmpack` (HOSMPAK1, built by our own
builder, `just map-packs`, from Geofabrik's OpenStreetMap extracts), carry
OpenStreetMap's coastlines, water, roads and building footprints in their
`osm_water`, `osm_ways` and `osm_buildings` layers. The app ships the packs around
home (Silverdale and Seattle Center). The packs' other layers are NOT OpenStreetMap
data and are never edited with it in the file (below under "Heights and land cover").

**Map data (c) OpenStreetMap contributors, ODbL 1.0**
<https://www.openstreetmap.org/copyright>

ODbL creates **two separate obligations**, and it is easy to satisfy one and
believe you are done:

1. **The rendered view is a Produced Work.** Anything that DRAWS this data must
   show the credit where the drawing is shown. In this repo that means the Maps
   planet view footer and the in-world credit line, both of which take their text
   from `credits::OSM_NOTICE`. A credit that lives only in this file does NOT
   discharge this - which was exactly the state of the in-world view before
   v0.1226, where the notice went to a log line and the debug console and no
   player ever saw it. Since 2026-11-09 the in-world line is drawn by its own
   layer (`src/gui/map_credit.rs`), not inside the HUD, so it also shows in the
   construction editor, in the showroom and with the HUD turned off.
2. **The packs' OpenStreetMap layers are a Derivative Database.** They are an
   extracted, re-cut subset of OSM, so publicly distributing them means offering
   them under ODbL 1.0 as well. They are hereby offered under ODbL 1.0, and every
   pack says so itself: its manifest names each layer's source, snapshot date and
   licence, carries the ODbL notice and offer, and names where its format is
   specified (`crates/humanity-terrain/src/map_pack.rs`), so a pack passed on alone
   still carries the offer. If you redistribute a release bundle containing
   `data/maps/packs/`, you carry this obligation forward.

The application NEVER contacts OSM servers at runtime. Packs are built at
development time from downloaded extracts and committed (the home packs) or
published as release files.

This is a plain-language summary written by developers, not lawyers. Where it and
the ODbL text disagree, the licence governs.

## Heights and land cover (GEDTM30, NOAA ETOPO 2022, ESA WorldCover)

- **GEDTM30 v1.2** (OpenGeoHub) - the land heights (`land_h`) inside the map
  packs, 30 m bare earth. CC BY 4.0
  (<https://creativecommons.org/licenses/by/4.0/>), record
  <https://zenodo.org/records/18887460>. Credit: Land heights: GEDTM30 v1.2,
  Global Ensemble Digital Terrain Model (Ho, Hengl, Parente et al., OpenGeoHub,
  2026), CC BY 4.0. GEDTM30 is built partly from the Copernicus DEM, so until the
  finding's open question 1 is answered its notice travels too: © DLR e.V.
  2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under
  COPERNICUS by the European Union and ESA; all rights reserved. The organisations
  in charge of the Copernicus programme by law or by delegation do not incur any
  liability for any use of the Copernicus WorldDEM-30.
- **NOAA ETOPO 2022** - Earth's shipped heights on land and under the sea
  (`data/planets/earth_heightmap.bin`, built by `examples/build_earth_grid.rs`),
  the connected-ocean mask derived from them (`earth_ocean_mask.bin`), the
  optional 15 arc-second tile tier, and the seabed layer (`sea_h`) of the map
  packs. NOAA's metadata: "NOAA waives any potential copyright and related rights
  in these data worldwide through the Creative Commons Zero 1.0 Universal Public
  Domain Dedication (CC0-1.0)." Nothing is owed; it is credited anyway, in the
  citation NOAA asks for: NOAA National Centers for Environmental Information.
  2022: ETOPO 2022 15 Arc-Second Global Relief Model. DOI: 10.25921/fd45-gt74.
- **ESA WorldCover 2021 v200** - the land cover layer (`cover`) of the map packs.
  CC BY 4.0, with the project's own credit line: © ESA WorldCover project 2021 /
  Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover
  consortium.

The region surveys (`data/maps/regions/*.dem.bin`, AWS Open Data Terrain Tiles)
that stood under Silverdale and Seattle Center until whole-Earth maps rung 2
(2026-11-09) are retired with the region files. All four sources are rows in
`data/credits.ron` (`gedtm30`, `copernicus_dem`, `noaa_etopo`, `esa_worldcover`);
the research behind them, quoted and dated, is
`docs/reference/findings/2026-11-08-whole-earth-map-data.md` (findings 12, 14, 16
and 21).

## NASA (public domain)

- **GIBS / Worldview** - live global cloud-cover imagery (MODIS cloud fraction).
  <https://worldview.earthdata.nasa.gov/>
- **Blue Marble** - Earth's surface colour grid.
  <https://visibleearth.nasa.gov/collection/1484/blue-marble>

NASA imagery is generally public domain and carries no attribution requirement.
It is credited anyway.

## Minor Planet Center orbits (the Maps page's asteroid belts)

`data/maps/belts.json` holds the orbits of 20,000 real minor planets (the main
belt, the Hildas, Jupiter's trojans, the Kuiper belt and beyond), derived by
`scripts/build-belts.js` from the Minor Planet Center's `MPCORB.DAT`
(downloaded 2026-10-21). It is **not** under the project's own licence: the MPC
asserts copyright in its data files, and the header of `MPCORB.DAT` lets
software include the data on three conditions, all met here:

- **the header ships with the data**: `data/maps/MPCORB-header.txt`, byte for
  byte as the MPC wrote it;
- **attribution to the Minor Planet Center**: on the Maps page wherever the
  belts are drawn (app and website), in `data/credits.ron` and here;
- **how to get updated copies**: <https://www.minorplanetcenter.org/iau/MPCORB/MPCORB.DAT>,
  updated daily; rerun `node scripts/build-belts.js` to refresh the file.

Every row keeps its `ref` and `computer` columns, which is how the header says
the people who computed each orbit are credited. The research behind this,
with the MPC's own words quoted and dated, is in
`docs/reference/findings/2026-10-21-mpc-orbit-data.md` (a reading of public
sources, not legal advice; it names the one message to the MPC that would
settle its two open points).

## Star catalogues - the second share-alike source

The app ships THREE star catalogues and lets the player choose between them in
Settings > Sky, so people on lesser hardware can pick a smaller one. Two of the
three come from astronexus and **both are CC BY-SA 4.0**, verified by fetching
the LICENSE file from each repository on 2026-09-07. Before v0.1300 this file,
`CREDITS.md`, `data/credits.ron` and `docs/design/maps-multi-scale.md` each gave
a different answer, and `credits.ron` recorded the licence as the placeholder
"See the ATHYG repository".

- **HYG database** (astronexus), **CC BY-SA 4.0** - the STANDARD ~120k-star
  catalogue that ships with the app and is what every player sees by default.
  `data/stars.csv` is this catalogue, and `data/stars.bin` is built from it by
  `scripts/build-stars-bin.js`. <https://github.com/astronexus/HYG-Database>
- **ATHYG database** (astronexus), **CC BY-SA 4.0** - the EXTENDED ~2.5M-star
  catalogue, an optional in-app download. HYG combined with Tycho-2. The GitHub
  repository is archived; the project moved to
  <https://codeberg.org/astronexus/athyg>

**CC BY-SA is share-alike, not merely attribution, and that has two
consequences this project has to state rather than assume.**

1. `data/stars.csv` and `data/stars.bin` are adapted material. They are tracked
   in git and copied into every release archive by the wholesale `cp -r data/`
   in `.github/workflows/build-desktop.yml`. Distributing them means offering
   them under CC BY-SA 4.0, which is hereby done. If you redistribute a release
   bundle, you carry that obligation forward, exactly as with the OpenStreetMap
   region files above.
2. `stars-athyg.bin` is published by this project as a GitHub release asset
   (`assets-stars-1`) and fetched by the in-app downloader at
   `src/renderer/stars.rs`. That is redistribution of adapted material from our
   own server, and no licence text currently travels with it.

This is the same shape as the OpenStreetMap problem and it means the root
`LICENSE`, a bare CC0 dedication, does not describe the whole tree. The project
dedicates to the public domain what its contributors wrote. It also
redistributes third-party works, some share-alike, that it cannot and does not
dedicate.

- **ESA Gaia** - the optional ULTRA catalogue (G<14, ~25M stars) and the
  integrated galaxy glow baked from it. Gaia asks for this acknowledgement:

  > This work has made use of data from the European Space Agency (ESA) mission
  > Gaia (<https://www.cosmos.esa.int/gaia>), processed by the Gaia Data
  > Processing and Analysis Consortium (DPAC,
  > <https://www.cosmos.esa.int/web/gaia/dpac/consortium>).

## NVIDIA SimReady Assets - the first 3D models that are not CC0

`assets/models/simready/` (the home's furniture, fixtures, tools and safety gear,
shipped in the app) and the Medical Bay model pack (downloaded from Settings >
Graphics > Furniture models into `<data>/models/packs/simready-medical/`) are based
on **NVIDIA SimReady Assets** (<https://huggingface.co/datasets/nvidia/simready-assets>),
**by NVIDIA**, licensed under **CC BY 4.0**
(<https://creativecommons.org/licenses/by/4.0/>). They stay CC BY 4.0 inside this
project: they are NOT public domain, unlike the CC0 model packs credited in
`assets/models/LICENSE-cc0-model-packs.md` and unlike what the project's
contributors wrote.

HumanityOS converted each one from OpenUSD to glTF and modified it (joined and
simplified its mesh, baked its materials into one texture set, moved its origin,
computed collider hulls, carried its physics values into `physics.ron`, and in seven
cases painted out a maker's name, logo or a protected emblem). Each folder holds
the asset's own `LICENSE.txt`, unchanged, and a `SOURCE.md` listing its changes;
`assets/models/LICENSE-nvidia-simready.md` lists every folder and package. The
licence grants no trademark rights and no endorsement: NVIDIA's name appears only to
credit the source. NVIDIA's MDL shader files carry different terms and were never
downloaded or shipped. The research, quoted and dated:
`docs/reference/findings/2026-11-08-nvidia-simready-assets.md`.

Redistributing these models (in a fork, a release archive or a website) means
keeping that credit, the licence link and the note that they were modified: under
CC BY 4.0 a failure to credit ends the licence until it is put right (section 6).

## The project's own code

HumanityOS source is licensed as stated in the repository root. These
third-party entries cover DATA, which is licensed separately from the code that
reads it.
