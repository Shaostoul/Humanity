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

## Star catalogues

- **ATHYG database** (astronexus) - the standard ~120k-star catalogue behind the
  night sky. <https://github.com/astronexus/ATHYG-Database>
- **ESA Gaia** - the optional extended catalogue (G<14, ~25M stars) and the
  integrated galaxy glow built from it. Gaia asks for this acknowledgement:

  > This work has made use of data from the European Space Agency (ESA) mission
  > Gaia (<https://www.cosmos.esa.int/gaia>), processed by the Gaia Data
  > Processing and Analysis Consortium (DPAC,
  > <https://www.cosmos.esa.int/web/gaia/dpac/consortium>).

## The project's own code

HumanityOS source is licensed as stated in the repository root. These
third-party entries cover DATA, which is licensed separately from the code that
reads it.
