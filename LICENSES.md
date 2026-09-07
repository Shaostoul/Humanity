# Third-party data and licences

HumanityOS ships real-world measurements, not invented ones. Some of the sources
carry obligations. This file records them for anyone redistributing the repo or a
release bundle; the in-app **Settings > Credits** page shows the same list to
players, generated from `data/credits.ron`.

If you add a data source, add a row to `data/credits.ron` in the same commit. A
unit test (`src/credits.rs`) fails if a source marked `attribution_required`
does not name a surface where its notice is actually shown.

## OpenStreetMap - the one with real obligations

`data/maps/regions/*.bin` (currently `silverdale`, `seattle-center`, plus their
`.dem.bin` elevation companions) are built from OpenStreetMap by
`scripts/fetch-osm-region.mjs`.

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
   player ever saw it.
2. **The region files are a Derivative Database.** They are an extracted,
   reprojected subset of OSM, so publicly distributing them means offering them
   under ODbL 1.0 as well. They are hereby offered under ODbL 1.0. If you
   redistribute a release bundle containing `data/maps/regions/`, you carry this
   obligation forward.

The application NEVER contacts OSM servers at runtime. Region files are fetched
once at development time and committed.

This is a plain-language summary written by developers, not lawyers. Where it and
the ODbL text disagree, the licence governs.

## NASA (public domain)

- **GIBS / Worldview** - live global cloud-cover imagery (MODIS cloud fraction).
  <https://worldview.earthdata.nasa.gov/>
- **Blue Marble** - Earth's surface colour grid.
  <https://visibleearth.nasa.gov/collection/1484/blue-marble>

NASA imagery is generally public domain and carries no attribution requirement.
It is credited anyway.

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

## The project's own code

HumanityOS source is licensed as stated in the repository root. These
third-party entries cover DATA, which is licensed separately from the code that
reads it.
