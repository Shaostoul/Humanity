# Map data for the whole Earth: OpenStreetMap at planet scale, and the open sources for heights, seabed, buildings and land cover

**Researched 2026-11-08** (every source below was read on 8 November 2026, Pacific time, which was
already 9 November in UTC). Pages were fetched by an AI research agent that identified itself as one
in its user agent; two sites refused or failed (listed under "Sources I could not reach").

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** Where it says what the project should do, that is marked as a
judgement call. A lawyer could disagree with any of it, and should read it before v1.0.

## The question

For a free, open-source game whose own code is CC0, which open data can draw the whole Earth (roads,
buildings, water, heights, seabed, land cover, place names), what does each source's licence oblige,
and is the project's current answer to OpenStreetMap (credit it where drawn, offer the derived files
under ODbL) the right one at planet scale?

## Short answer

**What the project did about OpenStreetMap.** It complied, it did not route around the licence.
The two shipped regions (Silverdale and Seattle Center) are built from OSM; the Maps planet view and
the in-world view show `Map data (c) OpenStreetMap contributors (ODbL)` while OSM data is drawn (one
string, `credits::OSM_NOTICE`); `LICENSES.md` offers the region files under ODbL; the app never
contacts OSM's servers. That is what the licence asks: a notice on the rendered view (a Produced
Work) and share-alike on the files (a Derivative Database). Two gaps from the 2026-09-06 audit are
still open in the code read today: the ODbL offer (`LICENSES.md`) does not travel in the release
archive, and the in-world credit lives inside the HUD, which is switched off in the construction
editor and the showroom while OSM geometry can still be on screen (finding 27).

**Is compliance the right fix at planet scale? Yes. There is no better one to switch to.** No
permissive source replaces OSM for roads, coastlines, water and street names at street detail.
Overture's transportation, base and buildings themes are themselves ODbL (finding 10); Microsoft's
mined roads are ODbL (finding 24); the one road set that calls itself CC0 (GRIP4) says in the same
paragraph that it is built partly from OpenStreetMap, so it is no clean escape (finding 24). Fetching
packs on demand changes nothing in kind: sending a player a copy of a derived file is still
"Conveying" it (finding 1), so every pack must carry the ODbL notices itself, and the offer is met
by the packs being downloadable in a documented format, free.

**The split that IS worth making** is not to get out of ODbL but to keep it where it belongs. The
heights, the seabed and the land cover come from non-OSM sources (CC0, public domain or CC BY). If
the distributed files keep each of those as its own layer that is never altered with OSM data, a
pack is a Collective Database: the OSM layers are ODbL, the others keep their own licences, and the
blending (the shoreline, the road beds, the building pads) happens in the game at run time, which
makes the result a Produced Work that needs only the notice (findings 1, 4). That is exactly how
the existing water carve already works.

**One source per layer** (the operator asked for the best one, not a menu; reasons in "What it means"):

| Layer | Recommended | Its terms | Runner-up |
|---|---|---|---|
| Land heights (30 m) | **GEDTM30** v1.2, OpenGeoHub, 2026 (finding 16) | CC BY 4.0; credit Copernicus too, to be safe | Copernicus GLO-30 with our own forest and building removal |
| Seabed, and the global base heights | **ETOPO 2022**, NOAA (finding 14) | CC0 1.0 | GEBCO 2026 (public domain) |
| Coastlines and water | **OpenStreetMap** (finding 1) | ODbL 1.0 | Natural Earth 1:10m (public domain), far zooms only |
| Roads | **OpenStreetMap** | ODbL 1.0 | Overture transportation (ODbL) |
| Buildings | **Overture Maps buildings theme** (finding 10) | ODbL 1.0 for the theme | OpenStreetMap alone |
| Land cover | **ESA WorldCover 2021 v200** (finding 21) | CC BY 4.0, with its own credit line | OSM land-use polygons (ODbL, patchy) |
| Place names | **OpenStreetMap** | ODbL 1.0 | Wikidata (CC0) |

Ruled out, and why (negative findings): FABDEM, the best-measured bare-earth model, is
non-commercial (finding 15); the AWS Terrain Tiles the project uses today for the two regions'
elevation are a patchwork whose credits include two "All rights reserved" national sources, fine
for Puget Sound (US public domain) but not a clean base for the whole planet (finding 20); and
Copernicus GLO-30, though freely licensed, is a surface model that includes trees and roofs, the
wrong ground for a game that draws its own trees and buildings (finding 12).

## Findings

### 1. The ODbL itself: Produced Work, Derivative Database, Collective Database, and what "Convey" covers

- Source: Open Data Commons Open Database License (ODbL) v1.0,
  <https://opendatacommons.org/licenses/odbl/1-0/>. The page carries no date; its server reported
  last modification on 4 August 2026. Read 2026-11-08.
- Definitions:

  > “Produced Work” – a work (such as an image, audiovisual material, text, or sounds) resulting from using the whole or a Substantial part of the Contents (via a search or other query) from this Database, a Derivative Database, or this Database as part of a Collective Database.

  > “Collective Database” – Means this Database in unmodified form as part of a collection of independent databases in themselves that together are assembled into a collective whole. A work that constitutes a Collective Database will not be considered a Derivative Database.

  > Conveying does not include interaction with a user through a computer network, or creating and Using a Produced Work, where no transfer of a copy of the Database or a Derivative Database occurs.

- The notice on a database (4.2 b), on a rendering (4.3), share-alike (4.4), its limits (4.5) and
  the offer (4.6):

  > Include a copy of this License (or, as applicable, a license permitted under Section 4.4) or its Uniform Resource Identifier (URI) with the Database or Derivative Database, including both in the Database or Derivative Database and in any relevant documentation

  > if you Publicly Use a Produced Work, You must include a notice associated with the Produced Work reasonably calculated to make any Person that uses, views, accesses, interacts with, or is otherwise exposed to the Produced Work aware that Content was obtained from the Database, Derivative Database, or the Database as part of a Collective Database, and that it is available under this License.

  > A Derivative Database is Publicly Used and so must comply with Section 4.4. if a Produced Work created from the Derivative Database is Publicly Used.

  > Using this Database, a Derivative Database, or this Database as part of a Collective Database to create a Produced Work does not create a Derivative Database for purposes of Section 4.4

  > If You Publicly Use a Derivative Database or a Produced Work from a Derivative Database, You must also offer to recipients of the Derivative Database or Produced Work a copy in a machine readable form of:

  followed by "The entire Derivative Database; or" a file of the alterations or "the method of
  making the alterations", available "free of charge if distributed over the internet".
- What it means: the game's picture is a Produced Work (notice only); our processed tiles are a
  Derivative Database (ODbL, with the licence or its URI inside each file and in the docs). Players
  fetching tiles on demand receive copies, so that is Conveying: each tile pack must carry the
  4.2 notices on its own, and the packs themselves, being downloadable for free in a documented
  format, are the 4.6 offer. A server that sent only rendered pixels would not be Conveying.

### 2. The OSMF's Produced Work guideline: a tile meant to be read back as data is a database

- Source: <https://osmfoundation.org/wiki/Licence/Community_Guidelines/Produced_Work_-_Guideline>,
  "Endorsed by the OSMF board 2014-06-06", page last edited 31 July 2016. Read 2026-11-08.

  > If the published result of your project is intended for the extraction of the original data, then it is a database and not a Produced Work.

  > However, if you publish a produced work, the underlying database has to be published as well (or alternations to the original database as is the case of derived databases), according to section 4.6 of ODbL.

- What it means: our vector tiles (coordinates a program reads back) are a database, never a
  Produced Work, whatever their format.

### 3. The OSMF Attribution Guideline: computer games, databases, and credit that follows the view

- Source: <https://osmfoundation.org/wiki/Licence/Attribution_Guidelines>, "Adopted by the
  OpenStreetMap Foundation board on 2021-06-25", page last edited 10 September 2026. Read 2026-11-08.

  > For video or computer games, attribution can be provided either by a splash screen on application startup, in the game view, during gameplay, on the credits page, in the menu, or in another suitable location. Detailed information must be provided in a suitable location if the initial attribution does not provide detailed information.

  > You must include attribution to OpenStreetMap and either the text of the ODbL or a link to it as part of the database, derivative database, or database as part of a collective database.

  > Optimally, attribution would dynamically change based on the location viewed, but this is not required.

  > The historical forms of attribution “© OpenStreetMap contributors” or “© OpenStreetMap” are acceptable.

- What it means: the current string is within the safe harbour; a credit line that names the
  sources of the tiles actually on screen is what the guideline calls optimal.

### 4. Mixing OSM with other data: the Horizontal Layers, Collective Database and Regional Cuts guidelines

- Sources (all on osmfoundation.org, all read 2026-11-08):
  <https://osmfoundation.org/wiki/Licence/Community_Guidelines/Horizontal_Map_Layers_-_Guideline>
  (endorsed 2014-06-06, last edited 31 July 2016);
  <https://osmfoundation.org/wiki/Licence/Community_Guidelines/Collective_Database_Guideline_Guideline>
  (endorsed 2016-06-17, last edited 31 July 2016);
  <https://osmfoundation.org/wiki/Licence/Community_Guidelines/Regional_Cuts_-_Guideline>
  (endorsed 2014-06-06, last edited 31 July 2016).

  > For all features of a given type ("Feature Types") (e.g. streets, restaurants, parks, cemeteries), if all data for that Feature Type is from non-OpenStreetMap sources, then the ODbl share-alike conditions do not apply to that Feature Type, even if OpenStreetMap data is used for other Feature Types.

  > An OSM dataset and a non-OSM dataset combined in a single database will be considered independent (and thus form a Collective Database rather than a Derivative Database) so long as the data used for a particular data type is either all OSM or all non-OSM within the same regional cut.

  The first of the listed cases is "the non-OSM and OSM datasets do not reference each other". The
  Regional Cuts guideline adds that a cut must be large: "The Regional Cut is at least a country."
- What it means: a height layer, a seabed layer and a land-cover layer that come wholly from non-OSM
  sources and are never edited using OSM stay outside share-alike, even in the same pack. Filling
  gaps in one feature type from two sources (OSM buildings, plus machine-learned buildings only
  where OSM has none) makes that feature type share-alike as a whole. A country-by-country swap of sources is
  allowed but buys us nothing, since the replacement sources are ODbL too.

### 5. "Substantial": no small-extract escape at our scale

- Source: <https://osmfoundation.org/wiki/Licence/Community_Guidelines/Substantial_-_Guideline>,
  endorsed 2014-06-06, last edited 27 February 2018. Read 2026-11-08.

  > We've set the boundary we are happy with at basically: village map OK, town map not OK.

- What it means: Silverdale alone is already past this line; the whole Earth plainly is.

### 6. The OSMF Licence and Legal FAQ on Produced Works

- Source: <https://osmfoundation.org/wiki/Licence/Licence_and_Legal_FAQ>, last edited 28 July 2023.
  Read 2026-11-08.

  > If you create a Produced Work, you can apply whatever terms you like to the Produced Work, but you must upon request offer recipients either a copy of your data and any Derivative Databases under the terms of the ODbL or the means of creating the Derivative Databases upon request.

- What it means: the game (a Produced Work) can stay CC0; what we must hand over on request is the
  data and our method, which our open format and open build script already are.

### 7. openstreetmap.org/copyright: two duties, and data form

- Source: <https://www.openstreetmap.org/copyright> (no date on the page). Read 2026-11-08.

  > Provide credit to OpenStreetMap by displaying our attribution notice.

  > Make clear that the data is available under the Open Database License.

  > If you are distributing OSM in data form, please name and link directly to the license(s).

  The page also lists national agencies whose data is inside OSM (Ordnance Survey, LINZ, Canada's
  GeoBase, and others); the copyright page link carries their credit.

### 8. The tile servers: never from OSM at run time

- Source: OSMF Tile Usage Policy, <https://operations.osmfoundation.org/policies/tiles/> (no date on
  the page). Read 2026-11-08.

  > Bulk download (“scrape”) tiles or offer prefetch features.

  (in its "You must not" list) and

  > Offline use is not permitted on tile.openstreetmap.org.

- What it means: the rule already in `LICENSES.md` stands. Region packs come from our own build of
  the planet file, hosted by us, never from OSM's tile or API servers.

### 9. Size of the planet, regional extracts, daily diffs, and how long a planet build takes

- Sources, all read 2026-11-08: <https://planet.openstreetmap.org/> and
  <https://planet.openstreetmap.org/pbf/> (listing dated 2026-11-05); Geofabrik,
  <https://download.geofabrik.de/> and <https://download.geofabrik.de/north-america/us.html>
  (served 2026-11-09 07:00 UTC); <https://wiki.openstreetmap.org/wiki/Planet.osm/diffs> (last
  modified 7 November 2026); Planetiler's README,
  <https://raw.githubusercontent.com/onthegomap/planetiler/main/README.md> (main branch, read
  2026-11-08).
- The planet: "Latest Weekly Planet PBF File (torrent) (RSS) 89 GB, created 3 days ago." The listing
  shows `planet-261102.osm.pbf 2026-11-05 17:40 89G`.
- Extracts (Geofabrik's listing, its sizes as printed): Europe 33.0 GB, North America 18.2 GB,
  United States of America 11.4 GB, Washington 348 MB.
- Daily diffs: the day feed is "Launched at 00:05 UTC every day."
- Planetiler (Apache 2.0, the fastest open planet tiler) publishes timings for the OpenMapTiles
  schema. The row nearest the operator's PC (32 GB of memory):

  > planet-220530.osm.pbf (69GB) | 0.5.0 | c6gd.4xlarge (16cpu/32GB) | 2h38m

  and its memory rule: "Give the JVM about 0.5x the size of the input `.osm.pbf` file, leaving the
  rest of your RAM for memory-mapped files." Its whole-planet outputs run 69 to 81 GB. It also
  says: "Planetiler only does full imports from `.osm.pbf` snapshots, there is no way to
  incorporate real-time updates."
- What it means (arithmetic, not a measurement): the operator's i7-8700K has 6 cores, one of them
  his; against 16 cores, on a planet 1.3 times bigger, expect roughly 3 to 4 times the 2h38m, so a
  whole-planet vector build is an overnight job (estimate 8 to 12 hours). Incremental updates are
  ours to build.

### 10. Overture Maps: the licence per theme

- Source: <https://docs.overturemaps.org/attribution/>, "Last updated on May 15, 2026". Read
  2026-11-08.
- Base, Buildings, Divisions and Transportation each say "License for theme: ODbL". Buildings lists
  OSM, "Esri Community Maps contributors. Available under CC BY 4.0.", "Global ML Building
  Footprints. Licensed by Microsoft under the Open Database License.", "Google Open Buildings.
  Available under CC BY 4.0." and others; Transportation lists OSM and "Data from TomTom."; Places
  is a list of CDLA Permissive 2.0, Apache 2.0 and CC0 sources. Their credit for the OSM-based
  themes:

  > © OpenStreetMap contributors, Overture Maps Foundation.

- What it means: Overture is not a way out of ODbL for roads, water or buildings. Its value is
  that the buildings theme already merges OSM with the machine-learned footprint sets.
- **Sources disagree:** Overture lists Microsoft's footprints as ODbL; Microsoft's own repository
  has said CDLA Permissive 2.0 since 11 March 2026 (finding 22). Overture's page is dated 15 May
  2026, two months after Microsoft's change, and still says ODbL. For the Overture theme this does
  not matter, since the theme as a whole is ODbL.

### 11. Natural Earth: public domain

- Source: <https://www.naturalearthdata.com/about/terms-of-use/> (no date beside the terms; footer
  "© 2009 - 2026"). Read 2026-11-08.

  > All versions of Natural Earth raster + vector map data found on this website are in the public domain.

  > No permission is needed to use Natural Earth. Crediting the authors is unnecessary.

- The same page records that some inputs (from The Washington Post, the EU's JRC, XNR Productions,
  International Mapping Associates) were licensed to Natural Earth "for the sole purpose of creating
  a world base map". Natural Earth's own statement is what users rely on.

### 12. Copernicus DEM GLO-30 and GLO-90: free, worldwide, conditions that bind later users, and a surface model

- Sources: the licence, <https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Data/DEM/resources/license/License-COPDEM-30.pdf>
  ("Licence for Copernicus DEM instance COP-DEM-GLO-30-F Global 30m Full, Free & Open"; no date in
  the text, server date 3 November 2026); the Copernicus Data Space page,
  <https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM>;
  the AWS registry, <https://registry.opendata.aws/copernicus-dem/>. All read 2026-11-08.
- Rights (Article 4): reproduction, distribution, communication to the public, and "adaptation,
  modification and combination with other data and information". Article 3: "The rights granted
  under this Licence are worldwide and without limitation in time." Article 5: "The use rights
  granted under this licence are free of charge to the User."
- Conditions (Article 6): the notice, as the Data Space page prints it,

  > © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved

  ("produced using Copernicus WorldDEM-30 ..." for modified data), a liability sentence that must be
  added to any licence or notice covering redistribution,

  > The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30

  and a pass-through:

  > Where the user grants to any Subsequent User the rights to distribute or communicate to the General Public the Copernicus WorldDEM-30, modified or not, he shall ensure that its Subsequent Users are bound by the above obligations.

- What it is:

  > The Copernicus DEM is a Digital Surface Model (DSM) which represents the surface of the Earth including buildings, infrastructure and vegetation.

  Since the December 2023 release, "GLO-30: Free & Open release of the restricted countries:
  Armenia, Azerbaijan and Moldova" (Data Space table); the AWS mirror still describes "a small subset
  of tiles" as not released.
- What it means: shippable, with a notice and a liability sentence that every fork must keep, so it
  is attribution-style, not CC0. And because it is a surface model, ground under a Douglas-fir
  forest would sit at the top of the canopy, under which the game then plants its own trees.

### 13. NASA SRTM and NASADEM: CC0 by NASA policy, but only between 60° N and 56° S

- Sources: NASA Earthdata, "Data Use and Citation Guidance",
  <https://www.earthdata.nasa.gov/engage/open-data-services-software-policies/data-use-guidance>;
  NASADEM, <https://www.earthdata.nasa.gov/data/catalog/lpcloud-nasadem-hgt-001>; SRTMGL1 v3,
  <https://www.earthdata.nasa.gov/data/catalog/lpcloud-srtmgl1-003>. Read 2026-11-08 (pages undated;
  NASADEM's citation gives 2020).

  > Unless the content is marked with a use restriction or license, data provided from a NASA-led mission are licensed as Creative Commons Zero (CC0).

  > This dataset is openly shared, without restriction, in accordance with the EOSDIS Data Use and Citation Guidance.

  > NASADEM are distributed in 1 degree latitude by 1 degree longitude tiles and consist of all land between 60° N and 56° S latitude. This accounts for about 80% of Earth's total landmass.

- What it means: the cleanest licence of any 30 m model, but no Alaska north of 60°, no northern
  Canada, Scandinavia or Siberia, and like Copernicus it is a radar surface model (finding 16 quotes
  a comparison ranking it below GLO-30).

### 14. ETOPO 2022: CC0, bare earth, land and seabed in one surface (and built partly from Copernicus and FABDEM)

- Sources: NCEI metadata,
  <https://www.ncei.noaa.gov/metadata/geoportal/rest/metadata/item/gov.noaa.ngdc.mgg.dem:etopo_2022/html>
  (publication 2022-10-01); the ETOPO 2022 User Guide,
  <https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/docs/1.2%20ETOPO%202022%20User%20Guide.pdf>
  (server date 17 October 2022). Read 2026-11-08.

  > These data were produced by NOAA and are not subject to copyright protection in the United States. NOAA waives any potential copyright and related rights in these data worldwide through the Creative Commons Zero 1.0 Universal Public Domain Dedication (CC0-1.0).

  > Since ETOPO 2022 is a bare-earth elevation dataset

  Its source table lists Copernicus DEM 30m and FABDEM (footnoted as "Forest and Buildings Removed
  Copernicus DEM") among the land layers, CUDEM (NOAA's 1/3 and 1/9 arc-second coastal models for
  the US), BlueTopo and GEBCO for the sea.
- What it means: the cleanest global land-and-seabed surface there is, already the source of
  `earth_heightmap.bin`. **An open point:** NOAA dedicates the result to CC0 while two of its inputs
  carry conditions (finding 12) or are non-commercial (finding 15). NOAA's statement is what we can
  rely on; see "What is still unknown".

### 15. FABDEM: the best-measured bare-earth model, and non-commercial (negative)

- Sources: University of Bristol data repository,
  <https://data.bris.ac.uk/data/dataset/s5hqmjcdj8yo2ibzi9b4ew3sn> (FABDEM V1-2, publication date
  18 Jan 2023); Fathom, <https://www.fathom.global/insight/fabdem-download/>. Read 2026-11-08.

  > The FABDEM dataset is licensed under a Creative Commons "CC BY-NC-SA 4.0" license. For commercial use queries, please contact fabdem@fathom.global

  Fathom: "This data is available under a commercial license or free for non-commercial use." The
  repository's own licence field says something else again, "Non-Commercial Government Licence for
  public sector information". **Sources disagree on which licence, and agree it is non-commercial.**
- What it means: a CC0 game anyone may sell cannot carry it. Not usable.

### 16. GEDTM30: a global 30 m bare-earth model under CC BY 4.0, labelled for testing

- Sources: Zenodo record v1.2.0, <https://zenodo.org/records/18887460> (read through its API,
  `https://zenodo.org/api/records/18887460`; publication date 2026-03-06; licence field
  `"id":"cc-by-4.0"`); the v1.1 record <https://zenodo.org/records/15689805> (2025-06-11, the same
  licence); the method paper, Ho, Hengl, Parente et al., PeerJ 2025, read on PubMed Central,
  <https://pmc.ncbi.nlm.nih.gov/articles/PMC12296579/>. Read 2026-11-08.

  > This is the first release of the Global Ensemble Digital Terrain Model (GEDTM30). Use for testing purposes only.

  > GEDTM30 is presented as a 1-arc-second (~30m) global Digital Terrain Model (DTM) generated using machine-learning-based data fusion.

  From the paper:

  > Copernicus DEM, ALOS World3D, and object height models were combined in a data fusion approach to generate a globally consistent digital terrain model (DTM).

  > GLO30 and AW3D30 are tied in quality but significantly more accurate than SRTM, NASADEM, and ASTER GDEM; FABDEM ranks best in terms of vertical accuracy compared to LiDAR DTMs.

  and against GNSS stations GEDTM30 gave "a standard deviation of 7.77 m, an RMSE of 10.69 m, and a
  mean error of 7.34 m". The v1.1 record also says the grid was resampled to 0.00025 degree,
  "closer to 0.9 arc sec".
- What it means: the only global bare-earth 30 m model found whose licence allows any use. Its
  inputs are Copernicus and JAXA's AW3D30, whose own conditions may or may not pass through an ML
  fusion (open point below). "Use for testing purposes only" is a disclaimer, not a licence term.

### 17. DeltaDTM: a bare-earth model for low coasts only

- Source: 4TU.ResearchData, <https://data.4tu.nl/datasets/1da2e70f-6c4d-4b03-86bd-b53e789cc629>,
  citation file <https://data.4tu.nl/export/cff/datasets/21997565> (v1.1, released 2024-12-12).
  Read 2026-11-08.

  > We present DeltaDTM, a global coastal Digital Terrain Model (DTM) available in the public domain, with a horizontal spatial resolution of 30 m and a vertical mean absolute error (MAE) of 0.43 m overall.

  The same file's licence line reads `license: CC BY 4.0`, and the model covers "Low-lying coastal
  areas (found below 10 m +Mean Sea Level (MSL))". **The two statements differ** (public domain in
  the abstract, CC BY 4.0 as the licence); the licence line is the one to follow. It corrects
  Copernicus DEM, so finding 12's question applies.
- What it means: a strong later insert for low coasts (deltas, the Wadden Sea), not a planet source.

### 18. GEBCO 2026: public domain, ocean and land at 15 arc-seconds

- Source: <https://www.gebco.net/data-products/gridded-bathymetry-data> (the GEBCO_2026 Grid). Read
  2026-11-08.

  > The GEBCO Grid is placed in the public domain and may be used free of charge. Use of the GEBCO Grid indicates that the user accepts the conditions of use and disclaimer information.

  The grid is "a global terrain model for ocean and land, providing elevation data, in meters, on a
  15 arc-second interval grid." The separate "conditions of use" page was not read.

### 19. USGS 3DEP: public domain bare earth and lidar, United States only

- Sources: USGS "Copyrights and Credits",
  <https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits>; the 3D
  Elevation Program page, <https://www.usgs.gov/3d-elevation-program>. Read 2026-11-08.

  > USGS-authored or produced data and information are considered to be in the U.S. Public Domain.

  The 3DEP page describes "the first-ever national baseline of consistent high-resolution elevation
  data - both bare earth and 3D point clouds - including baseline and next generation coverage as of
  September 30, 2026."
- What it means: for the operator's own Kitsap County, a better source than any global one, as a
  later insert.

### 20. AWS Terrain Tiles (the source of today's `.dem.bin` files): a patchwork of credits (negative for the planet)

- Source: Tilezen/joerd attribution,
  <https://raw.githubusercontent.com/tilezen/joerd/master/docs/attribution.md> (the master branch;
  no date in the file). Read 2026-11-08.

  > Attribution is required for many terrain tile data providers.

  Its required list includes "New Zealand terrain data Copyright 2011 Crown copyright (c) Land
  Information New Zealand and the New Zealand Government (All rights reserved);" and "United Kingdom
  terrain data © Environment Agency copyright and/or database right 2015. All rights reserved;"
  beside Copernicus EU-DEM, Canada's Open Government Licence, INEGI (Mexico), Kartverket (Norway),
  Austria, Australia and the US public-domain sets.
- What it means: fine for the two Puget Sound regions (US 3DEP), as the 2026-09-06 audit said, but
  a whole-planet height layer built from these tiles would inherit a dozen credit lines and two "All
  rights reserved" sources. Use primary datasets instead.

### 21. ESA WorldCover: CC BY 4.0, 10 m, with its own credit line

- Source: <https://esa-worldcover.org/en/data-access> (no date on the page). Read 2026-11-08.

  > The ESA WorldCover product is provided free of charge, without restriction of use. For the full license information see the Creative Commons Attribution 4.0 International License.

  The credit asked for in a published map: "© ESA WorldCover project [year] / Contains modified
  Copernicus Sentinel data ([year]) processed by ESA WorldCover consortium". "The WorldCover 2021
  v200 reaches an overall accuracy of 76.7%."

### 22. Microsoft Global ML Building Footprints: relicensed from ODbL to CDLA Permissive 2.0 on 11 March 2026

- Sources: <https://raw.githubusercontent.com/microsoft/GlobalMLBuildingFootprints/main/LICENSE>
  and the README beside it; the file's history through GitHub's API,
  `https://api.github.com/repos/microsoft/GlobalMLBuildingFootprints/commits?path=LICENSE`; the
  earlier LICENSE at commit `bff313e9`; the CDLA Permissive 2.0 text, <https://cdla.dev/permissive-2-0/>.
  Read 2026-11-08.
- Today's LICENSE, typo included: "Data in this repository has been licensed by Microsoft under CLDA
  Permissive 2.0." The commit of 2026-03-11: "Updating Readme and License file to relfect the change
  in license to CDLA Permissive 2.0 (#146)". The version before it (2022):

  > Data in this repository has been licensed by Microsoft under the Open Data Commons Open Database License (ODbL).

  The README counts "999M building footprint polygon geometries". CDLA Permissive 2.0:

  > A Data Recipient may share Data, with or without modifications, so long as the Data Recipient makes available the text of this agreement with the shared Data.

- What it means: since March 2026 these footprints need only the agreement text beside them. Copies
  taken before then came under ODbL.

### 23. Google Open Buildings: CC BY 4.0 or ODbL, the user's choice

- Source: <https://sites.research.google/gr/open-buildings/> (v3; no page date). Read 2026-11-08.

  > The data is shared under the Creative Commons Attribution (CC BY-4.0) license and the Open Data Commons Open Database License (ODbL) v1.0 license. As the user, you can pick which of the two licenses you prefer and use the data under the terms of that license.

  "The dataset contains 1.8 billion building detections, across an inference area of 58M km2
  covering Africa, South Asia, South-East Asia, Latin America and the Caribbean." No Europe, North
  America or northern Asia.

### 24. Roads without OSM: none that is both permissive and clean (negative)

- Sources: Microsoft Road Detections,
  <https://raw.githubusercontent.com/microsoft/RoadDetections/main/LICENSE>; GLOBIO's GRIP page,
  <https://www.globio.info/download-grip-dataset>. Read 2026-11-08.
- Microsoft: "Data in this repository has been licensed by Microsoft under the Open Data Commons Open
  Database License (ODbL)." GRIP4, in one paragraph:

  > GRIP4 is based on many different sources (including OpenStreetMap) and to the best of our ability we have verified their public availability, as a criteria in our research.

  > GRIP4 is provided under a Creative Commons License (CC-0) and is free to use.

- What it means: a CC0 label on a set built partly from OSM is not something a project can lean
  on (judgement); and GRIP is a coarse, environmental-modelling set anyway, "not suitable for
  navigation" in its own words.

### 25. Place names outside OSM: Wikidata (CC0) and GeoNames (CC BY 4.0)

- Sources: <https://www.wikidata.org/wiki/Wikidata:Licensing> (last modified 27 October 2026);
  <https://www.geonames.org/about.html> (no date). Read 2026-11-08.

  > All structured data (i.e. the main, Property, Lexeme, and EntitySchema namespaces) is released into the public domain under Creative Commons Zero.

  GeoNames: "available for download free of charge under a creative commons attribution license",
  and lower on the page "This work is licensed under a Creative Commons Attribution 4.0 License."

### 26. Hosting the packs: GitHub release limits

- Source: GitHub Docs, "About releases",
  <https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases>. Read
  2026-11-08.

  > Up to 1000 release assets may be associated with a single release. Each file included in a release must be under 2 GiB. There is no limit on the total size of a release, nor bandwidth usage.

### 27. What the project does today (read from the repository on 2026-11-08, commit `20a66044a`)

- `data/credits.ron` has the OSM row with `shown_in: ["Maps planet view footer", "In-world region
  credit", "Credits"]`; `crates/humanity-core/src/credits.rs:72` is the notice string.
- `LICENSES.md` offers the region files under ODbL and says the app never contacts OSM servers. It
  still files the `.dem.bin` elevation files under the OpenStreetMap heading, though they come from
  AWS Terrain Tiles by a different script (audit 2026-09-06, item 10, still true).
- `.github/workflows/build-desktop.yml:111` copies `data/` into the release archive; `LICENSES.md`
  is not in `data/` (only `data/library/CREDITS.md`, which now names OSM). So the ODbL offer for
  the region files in `data/maps/regions/` does not travel with the bundle (audit item 4, still
  open).
- `src/gui/pages/hud.rs:493` draws the OSM credit when `osm_regions_drawn` is not empty, inside
  `hud::draw`, which `src/lib.rs:15128-15131` skips while `showroom_active` or `construction_active`
  (audit item 4, still open).

## What is still unknown, and what would settle it

1. **Do Copernicus's conditions pass through a fused or resampled product** (GEDTM30, DeltaDTM,
   ETOPO 2022) whose publisher states a different licence (CC BY 4.0, CC0)? Article 6(e) binds
   "Subsequent Users" of the WorldDEM-30 "modified or not". Settles it: a written answer from the
   Copernicus DEM licensor (through the Copernicus Data Space help desk) or a lawyer; cost, one
   message. Until then, show the Copernicus notice and liability sentence wherever GEDTM30 heights
   are credited (one line, no harm).
2. **How ETOPO 2022's CC0 dedication squares with FABDEM (non-commercial) as an input.** Settles
   it: one email to NOAA's DEM team (`dem.info@noaa.gov`, named on the ETOPO page) asking which
   terms govern the FABDEM-derived cells; cost, one message.
3. **Whether a height field edited with OSM coastlines at build time becomes ODbL.** The Collective
   Database guideline points to yes if the layers reference each other (its word). The plan avoids the
   question by keeping the edit at run time; if a later rung wants build-time fusion, a question to
   the OSMF Licensing Working Group settles it.
4. **GEBCO's "conditions of use and disclaimer information"** were not read. Settles it: reading
   that page (free).
5. **The real size and time of a planet build on the operator's PC.** The estimates here are
   arithmetic from Planetiler's published table and Geofabrik's sizes. Settles it: rung 3 of the
   plan measures it.
6. **Overture's buildings theme size, schema and release cadence** were not read (only its licence
   page). Settles it: Overture's release notes, before rung 3.
7. **Which tide line OSM's `natural=coastline` follows.** Needed to set the shoreline height in the
   plan; to be read from the OSM wiki in rung 2.

## Sources I could not reach

- <https://osmdata.openstreetmap.de/data/land-polygons.html> (the OSM-derived land and water
  polygons used by most planet renderers): connection refused twice, by both a script and the
  fetch tool. Hoped for: its licence line (expected ODbL) and file sizes.
- <https://peerj.com/articles/19673/> (the GEDTM30 paper on the publisher's site): HTTP 403 to
  automated reading; not worked around. The same paper was read on PubMed Central instead.
- <https://www.eorc.jaxa.jp/ALOS/en/aw3d30/index.htm> (JAXA AW3D30 terms, one of GEDTM30's inputs):
  the page has moved; not followed further, since AW3D30 is not recommended directly.
- <https://www.ncei.noaa.gov/products/etopo-user-guide>: 404; the User Guide PDF was read instead.
- Not attempted: the Overpass API usage policy (the dev-time fetcher uses Overpass; a planet
  pipeline would not), CARTO's basemap terms (the website's Maps page fetches CARTO tiles at run
  time, audit item 12), EMODnet bathymetry, Esri's 10 m land cover, Dynamic World, MERIT DEM,
  FathomDEM (named in the GEDTM30 paper as a newer bare-earth model; its licence was not read).

## What it means for this project

### Certain (from the sources and the code above)

- Rendering OSM in the game needs a notice where it is shown, and for a game the credits page or
  the menu is enough if the short notice in view points to detail (findings 1, 3).
- Every file of processed OSM we hand out, bundled or fetched, must be ODbL, carry the licence or
  its URI inside it, and be offered free; on-demand fetching is Conveying (findings 1, 2).
- Overture, Microsoft's roads and GRIP do not take roads, water or buildings out of ODbL (findings
  10, 24). Natural Earth, NASA, NOAA, GEBCO and USGS data carry no conditions (findings 11, 13, 14,
  18, 19). WorldCover, GEDTM30, Google's buildings and GeoNames need attribution (CC BY 4.0).
  Copernicus needs a notice, a liability sentence, and passes both on (finding 12). FABDEM cannot be
  used (finding 15). Microsoft's footprints need only the CDLA text since 11 March 2026 (finding 22).
- Two compliance gaps are open today (finding 27): the ODbL offer does not ship in the release
  archive, and the in-world credit disappears in the construction editor and the showroom.
- The licence vote's plan B ("CC0 data") cannot cover these files whichever plan wins: map packs
  stay ODbL and CC BY because their sources say so.

### Judgement calls (reasons given; a reader may weigh them differently)

- **Compliance is the right fix; keep it, finish it.** Ship the ODbL text and the offer inside the
  release archive and inside every pack; draw the map credit outside the HUD's on/off gate; name
  the sources of what is on screen.
- **Keep the non-OSM layers independent** (heights, seabed, land cover as their own layers in each
  pack, never edited with OSM data in the files) and do the joining at run time. This keeps
  share-alike off the layers that are not OSM's and avoids open question 3.
- **Land heights: GEDTM30.** Bare earth matters more to this game than a few metres of absolute
  accuracy: a surface model puts the ground on top of the forest and the roofs, and the game then
  draws trees and buildings on top of that. GEDTM30 is the only bare-earth global 30 m set found that any
  use may carry. Its 7 m mean bias does not move coastlines, because the coastline polygon decides
  where land ends (see the plan). Runner-up: Copernicus GLO-30 with a FABDEM-style removal of
  forest and buildings that we build ourselves from WorldCover's tree class and the building
  footprints, if GEDTM30's next release disappoints.
- **Seabed and global base: ETOPO 2022** over GEBCO 2026, because it is one surface for land and sea,
  it carries an explicit CC0 dedication, it already includes NOAA's fine coastal models for US
  waters (Puget Sound among them), and it is already our base. GEBCO 2026 is newer offshore.
- **Coastlines, water, roads, place names: OpenStreetMap, at every zoom.** One coastline from
  continents down to the doorstep is worth more than a public-domain base layer whose coastline
  would jump at the hand-over; we comply with ODbL anyway. Wikidata (CC0) is the runner-up for
  names, Natural Earth for far-zoom coastlines and borders.
- **Buildings: Overture's buildings theme**, because it already merges OSM with Microsoft's, Google's
  and Esri's footprints into one theme (how it resolves overlaps was not read; open question 6), the hardest part to redo ourselves, under the
  licence we already meet. Runner-up: OSM alone, with Microsoft's footprints merged by our own build
  where OSM has none (the merged layer is then ODbL, which costs nothing extra).
- **Land cover: ESA WorldCover 2021**, one consistent global year at 10 m; OSM's land-use polygons
  stay as separate features the game may prefer locally.

**Again: this is not legal advice and its author is not a lawyer.** It records what the sources
said on 8 November 2026, so a later reader can tell whether the world changed or we misread it.
