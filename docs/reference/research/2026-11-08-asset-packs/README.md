# Openly licensed 3D asset sources for HumanityOS: a scan for the gaps SimReady leaves

**Read on 8 November 2026, Pacific evening** (local clock at the end: Sun Nov 8 23:40 PST 2026), by an
AI research agent working for the project. Pages were read through a fetch tool that identifies
itself; public metadata APIs were called with the user agent
`HumanityOS-research/1.0 (AI research agent reading metadata; https://united-humanity.us)`, one
request at a time with a pause between. **Nothing was downloaded**: no model, texture or archive,
only JSON metadata, licence files and page text.

**Status: SCAN ONLY. Nothing here is adopted.** Every source below is marked **needs a dated legal
finding before use** (the `docs/reference/findings/README.md` way, as the SimReady finding did). The
quotations below come from the pages as the fetch tool returned them; a finding must re-read each
primary page in full and quote it again with that day's date. I am not a lawyer and this is not legal
advice.

Asked for by the operator, 2026-11-08: "We'll want to do searches for other great asset packs. I'm
sure there are several we can pull from." Standing rule the same day: compare alternatives every
time a new tool or source comes up.

---

## 1. Short answer

- **No open library has a true life-support machine.** Nothing found in any source is a CO2
  scrubber, an electrolyser, a water recycler or a solar charge controller as built for a home or a
  ship. For those, the realistic route is **our own modelling** from real dimensions, with open
  parts as reference and kit (pumps, tanks, ducts, fans, breaker panels, cylinders), which do exist.
- **The stove and oven are solved at photoreal quality and CC0**: Poly Haven's `electric_stove`
  (a freestanding electric cooker with an oven, 13,914 triangles, measured 503 x 648 x 859 mm,
  glTF with textures from 1k to 8k).
- **Shower, washer and water heater exist only as CC BY 4.0 models from individual Sketchfab
  artists** (or as low-poly CC0 in Kenney's Furniture Kit, already the project's source, or as
  untextured CC BY 3.0 CAD in the FreeCAD parts library). Each needs per-item vetting: brands,
  fan art, AI-generated uploads and "NoAI" tags all turned up in the results.
- **Best general sources, in order**: Poly Haven (CC0 models, 869 textures, 997 HDRIs), ambientCG
  (CC0, 2,041 materials, 435 HDRIs), NVIDIA SimReady (already planned), Sketchfab's CC BY 4.0 pool
  (largest, needs vetting and credit), Gazebo Fuel's OpenRobotics models (CC BY 4.0 and CC0, PBR),
  Amazon Berkeley Objects (CC BY 4.0, glTF, 7,900+ real products), the FreeCAD parts library
  (CC BY 3.0 CAD at real dimensions), Infinigen (BSD-3 procedural kitchen and bathroom generators).
- **Plainly NOT open:** Quixel Megascans (Fab Standard License: no standalone redistribution,
  private repositories only), Blendkit's (formerly BlenderKit) Royalty Free assets (games only "if
  these can't be extracted by the users in an easy way", which an open-source repo cannot meet),
  Sketchfab's Standard, Free Standard and Editorial licences, ShareTextures ("Custom CC0 ... with
  specific restrictions"), HSSD (CC BY-NC 4.0).
- **Sites that refused automated reading** (HTTP 403; not worked around): si.edu and 3d.si.edu,
  thingiverse.com, opengameart.org, grabcad.com, and one news site (tutkit.com). The Smithsonian
  was read through its official open-data routes instead (GitHub, the AWS registry, its public API).
- **2026 changes worth knowing:** Sketchfab now belongs to KitBash (announced 10 August 2026) and
  its terms changed on 27 October 2026; BlenderKit renamed itself Blendkit; Godot's asset store left
  its beta address; NASA's media guidelines were updated 27 October 2026.

---

## 2. The gaps, as the repo states them

From `docs/design/simready-props.md` section 2 and `data/machines/*.ron`: no **stove, oven, shower,
bathtub, washing machine or water heater**, and nothing like the **life-support machines** (CO2
scrubber, air handler, heat pump, water purifier, battery bank, septic tank; the brief adds water
recyclers, CO2 removal, electrolysers, pumps, filters, batteries, inverters and solar charge
controllers). Catalogue rows that are boxes today and fall in these gaps: `stove_1`, `oven_1`,
`shower_1/2`, `washer_1`, `water_heater_1`, `air_recycler_1`, `air_handler_*`, `pump_1`,
`purifier_1`, `battery_1..8`, `gen_1`, `solar_1..10`, `wind_1`, `cistern_1`, `septic_tank_1`,
`exhaust_fan_1`, `mushroom_fan_1`, `freezer_1` (Kenney fridge today). The mothership plan
(`docs/design/mothership-master-plan.md` 3.8) adds a sector engineering segment with "the reactor
pair, pumps and air plant", farms' lamp power plant, a Medical Bay and mess halls.

The project's bar is the realistic-first rule (CLAUDE.md, 2026-08-02): pick the technique a 2030
release would use. So low-poly CC0 kits (Kenney, Quaternius, KayKit) count only as stand-ins.

---

## 3. Ranking

Ranked for HumanityOS: licence fit (CC0 best, then CC BY 4.0, then CC BY 3.0; anything stopping
redistribution in an open repo is out), realism (PBR, real scale), glTF, and how much of the gap
list it fills.

| Rank | Source | Licence | Realism | Formats | Fills the gaps? | Verdict |
|---|---|---|---|---|---|---|
| 1 | **Poly Haven** | CC0 | photoreal, scanned or hand-built, real scale (mm in the API) | glTF, blend, FBX, USD; 1k to 8k | stove + oven; generator, compressor, ducts, breaker box, condenser, tanks, cylinders, ceiling fan | **best general source**; needs a dated legal finding before use |
| 2 | **NVIDIA SimReady** | CC BY 4.0 | CAD-grade, real scale, physics | USD | already planned; none of the named gaps | finding exists (2026-11-08) |
| 3 | **Sketchfab, CC BY 4.0 / CC0 filters** | per model | varies, some excellent | glb/glTF/USDZ/source per model | the only pool with a shower, washer, water heater, pumps, inverters, purifier stand-ins | **largest gap filler, heavy vetting**; needs a dated legal finding before use |
| 4 | **Gazebo Fuel (OpenRobotics)** | per model, CC BY 4.0 or CC0 | PBR on newer models, robot-sim scale | SDF + DAE/OBJ + PNG | oven, pump, shower, kitchen sink, electrical box, valves | **strong, small**; needs a dated legal finding before use |
| 5 | **ambientCG** | CC0 | photoreal materials, HDRIs | PNG/JPG/EXR sets | no models for the gaps | **best for materials**; needs a dated legal finding before use |
| 6 | **Amazon Berkeley Objects** | CC BY 4.0, credit "Amazon.com" | real products, 4K PBR | glTF 2.0 | furniture and home goods; appliances unconfirmed | **promising, brands to check**; needs a dated legal finding before use |
| 7 | **FreeCAD parts library** | CC BY 3.0 | exact CAD shapes, no textures or UVs | FCStd, STEP, STL, BREP | stove top, built-in oven, washer, dishwasher, hood, fridge, shower pads, pumps, ducts | **geometry base for our own modelling**; needs a dated legal finding before use |
| 8 | **Infinigen** (Princeton) | BSD-3-Clause code | procedural, photoreal-aimed, real scale | Blender-generated meshes | oven, dishwasher, microwave, fridge, toaster, bathtub, toilet, sink generators | **2026 standout for endless variants**; needs a dated legal finding before use |
| 9 | **Kenney** | CC0 | low-poly, stylised | glTF, OBJ, FBX, DAE, STL | stove, electric stove, shower, washer, dryer, bathtub (stylised) | **stand-in only**; needs a dated legal finding before use |
| 10 | **NASA 3D Resources** | NASA guidelines; repo metadata says NOSA 1.3 (conflict) | mixed, some excellent | mostly glb; some 7z archives | ISS interior, habitat unit, ORU box, EMU suit, tools | **space hardware reference**; needs a dated legal finding before use |
| 11 | **Smithsonian Open Access** | CC0 per record | museum photogrammetry | Voyager 3D packages | none | heritage props only; needs a dated legal finding before use |
| 12 | **Khronos glTF samples** | per model, CC0 or CC BY 4.0 | reference-quality PBR | glTF | a commercial refrigerator, lantern, bottle | test and reference; needs a dated legal finding before use |
| 13 | **Quaternius** | CC0 | low-poly stylised | FBX, OBJ, glTF (newer), blend | interior kit, sci-fi kits; no appliances confirmed | stand-in only; needs a dated legal finding before use |
| 14 | **KayKit** | CC0 | low-poly stylised | OBJ, FBX, glTF | furniture | stand-in only; needs a dated legal finding before use |
| 15 | **Blendkit (formerly BlenderKit)** | Royalty Free (unusable here) or CC0 | high | .blend, some glTF | CC0 subset is mostly Poly Haven mirrors | only its CC0 items; needs a dated legal finding before use |
| 16 | **Objaverse / Objaverse-XL** | ODC-By for the set; per object | Sketchfab-sourced | glb | same items as Sketchfab | use the Sketchfab originals instead; needs a dated legal finding before use |
| 17 | **Printables / Thingiverse** | per item | printable STL, untextured | STL/3MF/STEP | enclosures, parts; few appliances | reference only; needs a dated legal finding before use |
| 18 | **OpenGameArt** | per item | mostly low-poly | mixed | not checked (refused) | needs a dated legal finding before use |
| 19 | **Godot Asset Library / Store** | per asset (SPDX) | n/a | mostly code | negligible | not a model source |
| - | **Not open**: Megascans/Fab, Blendkit RF, Sketchfab Standard/Free Standard/Editorial, ShareTextures, HSSD, GrabCAD (unread) | | | | | do not use |

---

## 4. Source by source

Every source below: **needs a dated legal finding before use.**

### 4.1 Poly Haven (models, textures, HDRIs)

- **Licence**, https://polyhaven.com/license (read 2026-11-08; no date on the page):
  > Our assets are all licensed as CC0

  > You can use our assets for any purpose

  > You can redistribute them

  > You do not need to give credit

  Its Terms of Service 4.1 keep site content (logos, renders) reserved, "excluding the CC0 assets
  themselves". Terms 3.2 bars "Web scraping or data mining without express permission", a site-use
  rule; the scan used the public API Poly Haven offers for this instead.
- **API terms**, https://polyhaven.com/our-api (dated on the page 18 July 2026): "There's no premium
  tier, no revenue share, and no login or API key required for the default endpoints." "All requests
  to the API require a unique User-Agent header". Attribution for apps that use the live API "is a
  condition of using the live API service, not of the assets themselves."
- **Redistribution in a game download:** yes; no attribution needed (crediting anyway matches
  `assets/models/LICENSE-cc0-model-packs.md`).
- **Realism:** photoreal, PBR (diffuse, normal GL/DX, roughness, metal, AO, ARM packed), real scale
  (the API gives dimensions in millimetres). Many models are dense: 8k to 80k triangles.
- **Formats:** glTF (1k, 2k, 4k, 8k texture sets), .blend, FBX, USD, per model
  (`api.polyhaven.com/files/electric_stove` lists `gltf`, `blend`, `fbx`, `usd`).
- **Counts (API, 2026-11-08):** 521 models, 869 textures, 997 HDRIs.
- **Relevant to the gaps:** `electric_stove` (stove AND oven: tags "stovetop, oven, cooktop"),
  plus life-support kit: `portable_generator` (26,419 tris, 2026-07-03), `old_military_compressor`
  (79,042 tris), `exterior_aircon_unit` (a condenser, the outdoor half of a heat pump),
  `modular_airduct_rectangular_01` and `_circular_01`, `modular_pipes`, `modular_pipes_plastic_01`,
  `modular_industrial_pipes_01`, `modular_electric_cables`, `power_box_01` (a breaker board, rigged
  door), `utility_box_01/02`, `propane_tank`, `small_lpg_tank`, `Barrel_02` (plastic water barrel),
  `ceiling_fan`, `scandinavian_masonry_heater`, `vintage_microwave`, `korean_fire_extinguisher_01`.
  About 20 items useful, 1 (the stove) filling a named gap outright.

### 4.2 ambientCG (materials, HDRIs, a few models)

- **Licence**, https://docs.ambientcg.com/license/ (read 2026-11-08; no date shown):
  > All ambientCG assets are provided under the Creative Commons CC0 1.0 Universal License.

  > You can include the raw files in your project, for example a video game.

  > You don't need to give credit but I would of course appreciate it, if you did it anyways.
- **Redistribution:** yes; no attribution needed.
- **Realism:** photoreal PBR material sets, HDRIs; scanned and procedural.
- **Formats:** PNG/JPG texture sets at several sizes, EXR/HDR for HDRIs, Substance files.
- **Counts (API v2, 2026-11-08):** 2,041 materials, 435 HDRIs, 127 decals, 61 atlases, 211 Substance
  files. The 3D model type returned no results through the API; the site's filter shows only food and
  tree-stump model categories. **No models for the gaps**; the material library is what matters.

### 4.3 Kenney

- **Licence**, https://kenney.nl/support (read 2026-11-08):
  > all game assets on the asset pages are public domain licensed (CC0)

  > Attribution is not required, but if you choose to give credit you can do so by mentioning "Kenney".

  > Do not use our logo, as it is reserved for official projects by our studio.

  Furniture Kit page, https://kenney.nl/assets/furniture-kit: licence "Creative Commons CC0",
  "Released in 2018", 140 files.
- **Redistribution:** yes (CC0); the project already ships 14 of its models.
- **Realism:** low-poly, flat-coloured, stylised. A stand-in, not the 2030 look.
- **Formats:** glTF, OBJ, FBX, DAE, STL.
- **Relevant items** (model names read from a third-party GitHub mirror's file listing,
  `xiaojilele-glitch/WhyBuddy`, not from Kenney; confirm against the official zip):
  `kitchenStove`, `kitchenStoveElectric`, `hoodLarge`, `hoodModern`, `shower`, `showerRound`,
  `bathtub`, `washer`, `dryer`, `washerDryerStacked`, `kitchenMicrowave`, `ceilingFan`,
  `kitchenFridgeLarge`, `kitchenFridgeBuiltIn`. That is six of the named gaps at stylised quality,
  through the converter the repo already has (`scripts/obj-to-plant-gltf.js`).

### 4.4 Quaternius

- **Licence**, https://quaternius.com/faq.html (read 2026-11-08): "All models are under the CC0
  License." "attribution is not necessary. However, credit is always appreciated."
- **Packs:** Ultimate House Interior Pack (https://quaternius.com/packs/ultimatehomeinterior.html):
  "120+ models" covering "doors and windows to kitchen and bathroom furniture", FBX/OBJ/Blend, CC0,
  free; appliances not itemised. Modular Sci-Fi Megakit
  (https://quaternius.com/packs/modularscifimegakit.html, September 2024): 277 models, FBX/OBJ/glTF,
  "Free to use in personal, educational and commercial projects"; the free Standard tier is about
  60 to 70% of it, Pro and Source tiers cost money (licence still CC0).
- **Realism:** low-poly stylised. **Gaps filled:** none confirmed.

### 4.5 Smithsonian Open Access 3D

- **The website refused automated reading**: https://www.si.edu/openaccess/faq and https://3d.si.edu/
  both returned HTTP 403 on 2026-11-08. Not worked around.
- **Read instead, the official open-data routes:** the Registry of Open Data on AWS
  (https://registry.opendata.aws/smithsonian-open-access/): licence "CC0"; the 25 February 2020
  release had "over 2.8 million CC0 interdisciplinary 2-D and 3-D images". The GitHub repository
  https://github.com/Smithsonian/OpenAccess is licensed CC0-1.0. The public API (api.si.edu, with
  the documented `DEMO_KEY`) shows each record's media with `"access":"CC0"`; **1,937 records carry
  3D models**. The demo key hit its rate limit after nine queries; stopped there.
- **Redistribution:** CC0 per record (check each record's `usage.access`).
- **Realism:** museum photogrammetry (Apollo 11 command module, its interior, the Wright Flyer).
- **Formats:** the API lists each model as a `3d_voyager` package (Smithsonian's Voyager viewer;
  these usually hold glb files, but the scan did not open one).
- **Gaps filled:** none. Searches for stove, oven, heater and pump found no appliance (one fire
  hat). Heritage and space-history props only.

### 4.6 NASA 3D Resources

- **Guidelines**, https://www.nasa.gov/nasa-brand-center/images-and-media/ ("Page Last Updated: Oct 27,
  2026"): media "generally are not subject to copyright in the United States"; "NASA should be
  acknowledged as the source of the material."; "The NASA Insignia, Logotype, identifiers, and
  imagery are not in the public domain."; commercial use "must not explicitly or implicitly convey
  NASA's endorsement of commercial goods or services."; and "Those wishing to use copyright protected
  material of third parties must contact the copyright holder directly." (The page also mentions
  "the Sept. 29, 2026, Executive Order 14434" and that NASA "does not permit our Insignia" with
  "SI-generated imagery"; worth reading in full for the finding.)
- **The repository**, https://github.com/nasa/NASA-3D-Resources (last push 2025-06-03): README says
  "These assets are free and without copyright." **But its `meta.json` says `"license": "NASA Open
  Source Agreement Version 1.3"`**, a licence with conditions. And the README's contributor list
  includes non-NASA parties (Ball Aerospace, DigitalSpace Corporation, individuals), whose work may
  carry copyright. The finding must settle which governs, per model.
- science.nasa.gov/3d-resources: "All of these assets are free to download and use."
- **Redistribution:** probably yes for NASA-made models, with credit and no insignia; unsettled.
- **Formats:** 227 model folders, mostly `.glb` with a preview PNG; some large `.7z` archives
  (format inside unknown).
- **Relevant:** `International Space Station (ISS) (E) (Internal)` (a 7z split in 8 parts, about
  304 MB), `Habitat Demonstration Unit` (two glb parts), `Orbital Replacement Unit` (a generic
  station hardware box, 0.6 MB glb, a fair housing for a life-support module), `Gateway Core` (66 MB
  glb), `Extravehicular Mobility Unit`, `Crew Lock Bag`, hand tools (Hammer, Wrench, Ratchet, Grease
  Gun, Pistol Grip Tool), `JSC Mission Control Room`. The ISS interior is the best open reference
  for what real station life-support racks look like.

### 4.7 Sketchfab (CC0 and CC BY filters) and its terms

- **Who runs it now:** KitBash announced it was acquiring ArtStation and Sketchfab from Epic Games
  on 10 August 2026 (https://www.epicgames.com/site/news/kitbash-acquires-artstation-and-sketchfab);
  the terms name "KitBash SF Operations LLC." **Fab is not part of that deal.**
- **Terms**, https://sketchfab.com/terms ("Effective October 27, 2026"):
  - §4.2.2: "Users may make User Content available to other users for download for free via either
    the License Agreement" or a Creative Commons licence; the downloader follows the licence in
    force at download.
  - §4.1: downloaded content "will always be subject to the most-current version of the License
    Agreement" ... "or Creative Commons license, as applicable." **For the finding:** CC BY 4.0
    says its grant is irrevocable, so how this clause applies to CC material needs a reading.
  - §4.2.4: "Users accessing and downloading User Content through the download API operated by
    Sketchfab shall be authenticated." Downloads need a logged-in account: an operator step.
  - §5: no use of the Services to "collect, aggregate, mine, scrape, or otherwise use any
    Sketchfab Content or User Content" except where the agreement allows.
  - §15(a): "You shall not collect, aggregate, mine, scrape, or otherwise use NoAI Content" for AI
    datasets, development or inputs. **Several candidates carry a NoAI tag** (below). Since this
    project's conversions are run by AI agents, NoAI items are best avoided until the finding reads
    §15 in full.
- **Licence labels** (public API https://api.sketchfab.com/v3/licenses, 2026-11-08):
  `by` "Author must be credited. Commercial use is allowed."; `cc0` "Credit is not mandatory.
  Commercial use is allowed."; `free-st` (Free Standard) and `st` (Standard) "Under basic
  restrictions, use worldwide, on all types of media, commercially or not, and in all types of
  derivative works"; `ed` "Use only in connection with events that are newsworthy or of public
  interest". Every CC BY model checked links `http://creativecommons.org/licenses/by/4.0/`.
  **Standard, Free Standard and Editorial are not open licences**; the licence page
  (https://sketchfab.com/licenses) says Editorial assets "cannot be used for any commercial or
  promotional use".
- **Redistribution:** CC0 yes; CC BY 4.0 yes, with credit per model (creator, title, link, licence,
  changes), the same pattern as the SimReady finding but per artist.
- **Realism:** from excellent (4K PBR, real scale) to junk; the face count, texture count and size
  come back in the search API.
- **Formats:** each downloadable model offers glb, glTF, USDZ and the uploader's source.
- **Counts:** CC0 is rare for these items (no relevant CC0 result for oven, washing machine, water
  heater, pumps, inverters or batteries; the few CC0 hits were museum scans); CC BY gives 24+
  results for most terms. About 40 candidates were
  worth recording (section 5).
- **Hazards seen in the results**, which the finding's checklist should cover:
  - **Brand replicas**: Daikin, Levoit, Panasonic, Electrolux, Samsung, IKEA, Thor Kitchen,
    Whirlpool, Midea, SMA Sunny Boy, SolarEdge, Fujitsu, Philips; and manufacturer uploads
    (polar_air, Sofosolerltd, a Philips oxygen concentrator from a Japanese distributor).
  - **Fan art** the uploader cannot license: a Mega Man Legends boiler, Pokemon, Kill la Kill,
    FNAF, Star Wars.
  - **AI-generated uploads**: "White dehumidifier unit" says "This model is generated by the
    state-of-the-art 3D GenAI tool, Rodin Gen-1"; copyright in such output is uncertain.
  - **NoAI tags**: glowbox3d's water heater and wind turbine, cyberreno's heat pump,
    epilogueronin's diesel generator.

### 4.8 Blendkit (formerly BlenderKit)

- **Renamed:** https://www.blenderkit.com/docs/licenses/ redirects (301) to
  https://www.blendkit.com/docs/licenses/, whose footer reads "Blendkit (formerly BlenderKit)",
  "© 2026".
- **Licences** (read 2026-11-08): Royalty Free "doesn't allow for re-sale of the asset in the same
  form"; CC0 "so that others may freely build upon, enhance and reuse the works for any purposes
  without restriction."
- **Licensing FAQ**, https://www.blendkit.com/docs/licenses/licensing-faq/:
  > Can I sell games made with Blendkit models?

  > Yes, if these can't be extracted by the users in an easy way.

  and "Yes, you can, but the assets shouldn't be directly extractable in the distribution."
  **HumanityOS ships loose glTF files in a public repository, so Royalty Free assets are out.**
- **Free tier:** the free plan's items are mostly Royalty Free; some CC0 items need the paid plan
  to download (CC0 then lets them be shared).
- **Counts (public search API):** 4,070 CC0 models, 2,491 CC0 materials, 6,886 CC0 HDRs (many are
  Poly Haven mirrors). Gap items found were all Royalty Free (washing machines named Midea and
  Samsung, a "Shower Water Heater", a "Solar inverter"). CC0 hits were Poly Haven mirrors plus a few
  "Free poly" items ("Old rusty stove", "Water tank", "Generator", "Air conditioner, heater").
- **Formats:** .blend first; newer items also have glTF and Godot glTF uploads.

### 4.9 Polyhaven-like scan libraries (and Megascans, which is not open)

- **Poly Haven, ambientCG**: above; CC0, the two to use.
- **Quixel Megascans is NOT open.** It is sold and given away through Fab under the **Fab Standard
  License**. Fab's EULA page served this agent a Chinese summary (https://www.fab.com/eula, read
  2026-11-08; no date shown); it forbids "单独转售或免费重新分发资源" (reselling or freely
  redistributing the assets on their own) and allows sharing with collaborators "直接共享、通过私有
  存储库或在项目中共享" (directly, via private repositories, or within the project). A public
  open-source repository holding Megascans files would break that. A news report says Quixel changed
  its terms on 5 November 2026 and made the library free; that page refused automated reading and
  was not verified. Free is not open: still no.
- **ShareTextures is not plain CC0**: https://www.sharetextures.com/p/license says it is "licensed
  under the Custom CC0 (Creative Commons Zero) license with specific restrictions", including "No
  asset redistribution on other websites, in plugins, or as part of collections without our written
  permission." and "No automated downloads, hotlinking, or embedding direct downloads in third-party
  apps (like Blender addons)." Shipping in an open repo may count as redistribution; avoid unless a
  finding says otherwise.
- **cgbookcase**: the home page says "100% free, no restrictions" but names no licence and shows no
  licence page; a finding would need the author's written terms.
- **Physically Based** (https://physicallybased.info/): a database of measured material values
  (about 92 materials with colour, IOR, complex IOR, F0, density) whose data repository,
  AntonPalmqvist/physically-based-api, is CC0-1.0. Not a model source, but the right reference for
  shader constants.

### 4.10 Objaverse (licence per object)

- https://huggingface.co/datasets/allenai/objaverse (last modified 2026-10-08, not gated), card:
  > The use of the dataset as a whole is licensed under the ODC-By v1.0 license. Individual objects in Objaverse are all licensed as creative commons distributable objects, and may be under the following licenses:

  CC-BY 4.0 721K objects, CC-BY-NC 4.0 25K, CC-BY-NC-SA 4.0 52K, CC-BY-SA 4.0 16K, CC0 1.0 3.5K;
  "The metadata will provide the license for each object."
- Objaverse-XL (https://huggingface.co/datasets/allenai/objaverse-xl): "Individual objects in
  Objaverse-XL are licensed under different licenses." (Polycam data is non-commercial, on request.)
- **Verdict:** Objaverse 1.0 objects are Sketchfab uploads; take each from its Sketchfab page, where
  the licence and the artist's name are primary. It adds no items the Sketchfab search lacks, and
  its scale and quality vary widely. Not a separate source for us.

### 4.11 Thingiverse and Printables (CAD of real appliances)

- **Thingiverse refused automated reading** (https://www.thingiverse.com/legal/ returned 403).
  Secondary sources say uploaders choose CC, GPL/LGPL, BSD, public domain or CERN-OHL per design,
  and a 2014 study found most content under CC, much of it non-commercial. Not verified here.
- **Printables terms**, https://www.prusa3d.com/page/terms-of-service-of-printablescom_231249/ (valid
  "from October 15, 2026"): §2.3 lists "Creative Commons licences, GNU licences, BSD License, Apache
  License 2.0, CERN Open Hardware Licence," the Open Community License and variants, the Standard
  Digital File License and Commercial Use licences. The Standard Digital File License's terms were
  not on that page.
- **Realism:** printable parts, untextured STL; real dimensions. Useful for enclosures, mounts,
  scale models; the appliance CAD that exists is better covered by the FreeCAD library.
- **GrabCAD** (the usual home of manufacturer CAD of pumps and inverters) **refused automated
  reading** (grabcad.com/terms, 403). Manufacturer CAD is usually free to download, not open; treat
  as reference only.

### 4.12 KhronosGroup glTF Sample Assets

- https://github.com/KhronosGroup/glTF-Sample-Assets (last push 2026-11-05): 150 models, each with a
  `metadata.json` giving its licences, for example `CommercialRefrigerator`:
  `"license":"CC-BY-4.0" ... "artist":"Eric Chadwick","year":"2025","owner":"Darmstadt Graphics
  Group GmbH"`; `WaterBottle`: `"license":"CC0-1.0" ... "artist":"Microsoft"`. Some carry a
  `LicenseRef-LegalMark-Khronos` logo entry (leave the logo out).
- **Realism:** reference-grade PBR, but mostly test scenes. **Gaps filled:** none; the commercial
  refrigerator suits the Concourse food hall.

### 4.13 Godot Asset Library and Store

- The library API (https://godotengine.org/asset-library/api) gives each asset an SPDX licence
  ("MIT", "CC0"); its categories are tools, shaders, materials, scripts, templates, demos. Model
  packs are rare (`KayKit Furniture Bits` CC0, `Tiny Treats - Charming Kitchen set` CC0, Quaternius
  mirrors). The store moved from store-beta to https://store.godotengine.org/, "currently in beta",
  showing tools and templates. **Not a model source.**

### 4.14 OpenGameArt

- **Refused automated reading** (https://opengameart.org/content/faq and its search returned 403).
  The repo already used it as a mirror of Quaternius's crops. Licences are per item; a person must
  read each page. Not scored.

### 4.15 Strong sources found beyond the list

**Gazebo Fuel, OpenRobotics models** (https://app.gazebosim.org/, API https://fuel.gazebosim.org/1.0):
3,433 models, each with its own licence in the API. Relevant OpenRobotics items: `Oven` ("Creative
Commons Attribution 4.0 International", 2023-09-29, DAE + albedo/metalness/normal/roughness PNGs),
`Pump` (CC BY 4.0, 2024-08-28, PBR), `Shower` ("A generic shower", CC BY 4.0, OBJ + diffuse),
`KitchenSink` (CC BY 4.0), `Kitchen and Dining` ("Creative Commons Zero v1.0 Universal"),
`Electrical Box` (CC0), a family of CC0 wheel and ball valves, and a "Moon base" set (dome, fan blade,
water; CC BY 4.0). Watch the names: `Washer` is a bolt washer and `Scrubber` a floor polisher.
Robot-sim assets are built at real scale.

**Amazon Berkeley Objects** (https://amazon-berkeley-objects.s3.amazonaws.com/index.html):
> This work is licensed under the Creative Commons Attribution 4.0 International Public License (CC BY 4.0).

"Credit for the data, including all images and 3D models, must be given to:" "Amazon.com." 3D models
for "more than 7'900 products", "provided in the standard glTF 2.0 format", 4K PBR maps; "© Amazon.com
2021-2023". The page names furniture; whether it holds appliances was not checked (that needs its
metadata listing). Real products carry brand names (its example is the Amazon brand "Stone & Beam"),
so each pick needs the brand check.

**Google Scanned Objects** (Gazebo Fuel owner `GoogleResearch`): 1,033 scanned household objects,
each "Creative Commons Attribution 4.0 International". **Mostly branded retail goods** (VTech toys,
Reebok shoes, Nestle cans): largely unusable for that reason, except plain items.

**FreeCAD parts library** (https://github.com/FreeCAD/FreeCAD-library, last push 2026-10-07).
`LICENSE-Assets`:
> All of it is licensed under the Creative Commons Attribution 3.0 Unported license (SPDX identifier: CC-BY-3.0), reproduced in full below.

> Each part is copyrighted by its own author, not by the FreeCAD project.

with authorship recorded in the git history. `Industrial Design/Appliances`: `ElectricStoveTop_900x600mm`,
`Oven_builtIn`, `LaundryMachine_600mm`, `Dishwasher_600mm`, `Fridge_600x600x2000mm`,
`KitchenHood_900mm`, `Microwave_builtIn`, and parametric `Stove`, `Washing_machine`,
`Refrigerator` (author F_Rosa, 2024). `Architectural Parts/Hydro equipment`: shower pads (1 x 1 m,
2 x 1.2 m), an 80 x 80 cm shower box, floor drain, wall-hung toilet, washbasin. `Electric
equipment`: an air conditioner. `HVAC/Ducts` (round, oval, rectangular, flex), `Hydraulics/Pump`
(vertical pump), `Architectural Parts/Solar` (solar tubes). STEP/STL/BREP/FCStd: no materials, no
UVs, so they are geometry to build on (unwrap, bake, texture), the rung of "our own modelling"
done with real dimensions. Avoid brand-named parts (`Schneider_EZ9F16110_MCB`, `yuasa_np7_12`).
CC BY 3.0 differs from 4.0 in its attribution and notice wording; the finding should read it.

**Infinigen** (https://github.com/princeton-vl/infinigen, last push 2026-11-05): "BSD 3-Clause
License", "Copyright (c) 2023, Princeton University". Its README: "It now features some procedural
code obtained from the internet under CC-0 licenses, which are marked with code comments where
applicable". Generators under `src/infinigen/assets/objects/`: `appliances/` (`beverage_fridge`,
`dishwasher`, `microwave`, `oven`, `toaster`, `tv`), `bathroom/` (`bathroom_sink`, `bathtub`,
`toilet`, `hardware`), plus shelves, tables, lamps, tableware, and procedural materials (metal,
ceramic, plastic, wear and tear). Runs in Blender (already on this PC). It gives endless real-scale
variants, which fits infinite-of-x, but its materials are procedural shader graphs that need baking,
and **whether generated meshes carry any licence condition** is a question for the finding (BSD
covers the code).

**KayKit** (https://kaylousberg.itch.io/furniture-bits): "Free for personal and commercial use, no
attribution required. (CC0 Licensed)", with a request: "please don't resell unmodified copies or
claim them as your own." Stylised low-poly.

**Poly Pizza** (https://poly.pizza/): "10,700+ free models", low-poly; licence per model
(Quaternius CC0, archived Google Poly CC BY 3.0, per `docs/reference/asset-and-map-sources.md`).

**Rejected:** HSSD, `hssd/hssd-models` on Hugging Face, tagged `license:cc-by-nc-4.0`. A
non-commercial licence would bind everyone who forks or redistributes the open-source project, which
the project's open-licence posture cannot carry; treat NC as out.

---

## 5. The gaps, item by item

Every row: **needs a dated legal finding before use.** "f" is the face count the source reports;
sizes are the glb download size. CC BY items need a credit line naming the artist, title, link and
licence, and a note of changes.

### Stove and oven

| Item | Source | Licence | Notes |
|---|---|---|---|
| **Electric Stove** (stove + oven), https://polyhaven.com/a/electric_stove | Poly Haven, Kuutti Siitonen, 2023-12-11 | CC0 | **First choice.** 13,914 tris, 503 x 648 x 859 mm, glTF 1k-8k, rigged |
| Kitchen Appliances, https://sketchfab.com/3d-models/kitchen-appliances-0b01a398fe1649aaacd6ebdf85bf5d6c | Sketchfab, Nicolai Kilstrup, 2023 | CC BY 4.0 | stove, oven, fridge, boiler, coffee machine; f13,407, 20.8 MB, 12 textures at 2048; mirrored UVs plus a second UV set |
| Cooking Stove/Range, https://sketchfab.com/3d-models/cooking-stoverange-030d8363b5c24904ad02bf9ba607faa2 | Sketchfab, neverfollow81, 2023 | CC BY 4.0 | range with oven; f2,794 |
| Gas Stove, https://sketchfab.com/3d-models/gas-stove-056ccf898f9e49acb6e3e370deabf184 | Sketchfab, Lyskilde (longtail), 2020 | CC BY 4.0 | old Soviet gas stove, rusted; f2,530, 17.7 MB |
| Oven, https://app.gazebosim.org/OpenRobotics/fuel/models/Oven | Gazebo Fuel, OpenRobotics, 2023 | CC BY 4.0 | DAE + PBR maps |
| `Oven_builtIn`, `ElectricStoveTop_900x600mm`, parametric `Stove` | FreeCAD library (Industrial Design/Appliances) | CC BY 3.0 | exact CAD, untextured |
| `oven.py` generator | Infinigen | BSD-3 code | procedural variants |
| `kitchenStove`, `kitchenStoveElectric` | Kenney Furniture Kit | CC0 | stylised stand-in |

### Shower (and bathtub)

| Item | Source | Licence | Notes |
|---|---|---|---|
| Shower Room - Assets, https://sketchfab.com/3d-models/shower-room-assets-8e13a866ee184bed95bb5b59ddee62cd | Sketchfab, Evan Petrov, 2022 | CC BY 4.0 | worn public shower room kit ("a bathroom for a remote research facility"); f23,154, 41 MB, 12 textures at 4096. **Best fit for ship and homestead realism** |
| Shower Cabin, https://sketchfab.com/3d-models/shower-cabin-e2c6a8dd490e4e4398378e1f6c9121a8 | Sketchfab, Heliona, 2020 | CC BY 4.0 | "Separated objects"; f30,434, 15.6 MB, 2048 |
| Bathroom Furniture Set - Game Ready, https://sketchfab.com/3d-models/bathroom-furniture-set-game-ready-0aabe78df5be407ea48e7ed70afd16a0 | Sketchfab, Melon Polygons, 2026-07-29 | CC BY 4.0 | 10,050 tris, 4 textures at 2048; tags include shower, washing machine, ventilation |
| Shower, https://app.gazebosim.org/OpenRobotics/fuel/models/Shower | Gazebo Fuel, OpenRobotics, 2020 | CC BY 4.0 | "A generic shower"; OBJ + diffuse only |
| shower pads, 80 x 80 cm shower box, floor drain | FreeCAD library (Hydro equipment) | CC BY 3.0 | CAD |
| `bathtub.py` | Infinigen | BSD-3 code | procedural |
| `shower`, `showerRound`, `bathtub` | Kenney | CC0 | stylised |

### Washing machine

| Item | Source | Licence | Notes |
|---|---|---|---|
| Washing Machine, https://sketchfab.com/3d-models/washing-machine-a27aa928a6664473a0ebfbe2c7f72b23 | Sketchfab, Zlat (goldie), 2019 | CC BY 4.0 | "A bit old and worn"; f5,168, 4K MetalRough PBR. **Good game weight** |
| Game-Ready: Washing Machine, https://sketchfab.com/3d-models/game-ready-washing-machine-6043c99592764c54b42f0af4a5d89f0c | Sketchfab, Melon Polygons, 2024 | CC BY 4.0 | 1.4k tris, 4096 PBR; sized against the UE5 mannequin, so check real dimensions |
| Washing Machine, https://sketchfab.com/3d-models/washing-machine-d81d81853a9f478ca9fac019eabcf453 | Sketchfab, re1monsen, 2021 | CC BY 4.0 | f21,005, 57 MB, 4K; tagged sci-fi |
| `LaundryMachine_600mm`, parametric `Washing_machine` | FreeCAD library | CC BY 3.0 | CAD |
| `washer`, `dryer`, `washerDryerStacked` | Kenney | CC0 | stylised |

### Water heater

| Item | Source | Licence | Notes |
|---|---|---|---|
| Water Heater (used), https://sketchfab.com/3d-models/water-heater-used-f1a7274c1a4c4d849cb2e67859763532 | Sketchfab, Glowbox 3D, 2020 | CC BY 4.0 **+ NoAI statement** | tank heater, f3,830, 11.2 MB, 2048. The only tank-type found; the NoAI line ("may not be used in datasets for, in the development of, or as inputs to generative AI programs") needs the finding's reading before an AI-run pipeline touches it |
| Soviet gas water heater KGI-56, https://sketchfab.com/3d-models/soviet-gas-water-heater-kgi-56-63221a93315c4e4e9b3ed8234ff026e7 | Sketchfab, GoldGryphon, 2025 | CC BY 4.0 | wall-hung gas geyser, f5,868, 4096; a replica of a real Soviet model |
| 3D Solar Thermal Water Heating System, https://sketchfab.com/3d-models/3d-solar-thermal-water-heating-system-6e1e65a8a27246baa3659edf4b771e1c | Sketchfab, Fevzi_Beydili, 2026-07-06 | CC BY 4.0 | f33,426, 4096; fits the homestead's solar loop |
| (none) | Poly Haven, ambientCG, Fuel, FreeCAD, Kenney | | **no open tank water heater at photoreal CC0 quality; model our own** (a cylinder with fittings, a simple shape) |

### Air: scrubbers, CO2 removal, air handlers, heat pumps, fans, filters

| Item | Source | Licence | Notes |
|---|---|---|---|
| **CO2 scrubber / CO2 removal** | none found anywhere | | model our own (a zeolite-bed rack like ISS's CDRA), using the NASA ISS interior as reference |
| COVID-19 Air-S Industrial Purifier Prototype, https://sketchfab.com/3d-models/covid-19-air-s-industrial-purifier-prototype-d605550137dc47a18cc726ff4f2af2db | Sketchfab, re1monsen, 2021 | CC BY 4.0 | the artist's own industrial design; f263,202, 134 MB, 4K (heavy); a plausible "air recycler" stand-in |
| COVID-19 Air-e Purifier Prototype, https://sketchfab.com/3d-models/covid-19-air-e-purifier-prototype-0c8d63784ce249ecb550ce41ceefe5e2 | Sketchfab, re1monsen, 2021 | CC BY 4.0 | room purifier, f22,094, 4K |
| Wet Scrubber, https://sketchfab.com/3d-models/wet-scrubber-9c2c80c8e40948a8b0eb4541cea6b170 | Sketchfab, RealShmell, 2025 | CC BY 4.0 | industrial air-cleaning tower, low-poly label but f321,423 |
| `modular_airduct_rectangular_01`, `modular_airduct_circular_01` | Poly Haven | CC0 | duct kit for air handlers |
| `exterior_aircon_unit` | Poly Haven, Monsta3D, 2023 | CC0 | condenser = heat pump outdoor unit; 18,986 tris; check textures for a maker mark |
| Industrial Air Conditioning Unit, https://sketchfab.com/3d-models/industrial-air-conditioning-unit-2048px2-b287991d98b84af1999a0d5ce1f9c5b2 | Sketchfab, Mark Peters, 2026-02-28 | CC BY 4.0 | worn industrial unit, f30,240, 2048; an air-handler stand-in |
| `ceiling_fan` | Poly Haven | CC0 | 16,356 tris |
| HVAC ducts, "Air conditionning" | FreeCAD library | CC BY 3.0 | CAD |
| HEPA Filter, https://sketchfab.com/3d-models/hepa-filter-12058b6fa3ab4bc58a3303e15d80cb39 | Sketchfab, tortol, 2022 | CC BY 4.0 | untextured |
| avoid: Daikin, Levoit, Fujitsu (NoAI), polar_air AHUs, the Rodin-generated dehumidifier | Sketchfab | | brands, NoAI or AI-generated |

### Oxygen: electrolysers and gas storage

| Item | Source | Licence | Notes |
|---|---|---|---|
| **Electrolyser / oxygen generator** | none found anywhere | | model our own (stack + separators + cylinders); real units are simple cabinets |
| `propane_tank`, `small_lpg_tank`, `portable_welding_cart` (oxygen and acetylene cylinders) | Poly Haven | CC0 | gas cylinders for O2/H2 storage |
| Hydrogen tank, https://sketchfab.com/3d-models/hydrogen-tank-c9b1c89380e947f58aa06eb56c93c6d8 | Sketchfab, ChesterGames, 2020 | CC BY 4.0 | small sci-fi tank, f1,214 |

### Water: recyclers, purifiers, filters, pumps, tanks, septic

| Item | Source | Licence | Notes |
|---|---|---|---|
| **Water recycler (distiller, WRS-style)** | none found | | model our own |
| Reverse Osmosis Unit, https://sketchfab.com/3d-models/reverse-osmosis-unit-0ece0c450ea54b87ad7ff6f025814bcf | Sketchfab, giacar25, 2019 | CC BY 4.0 | f292,518, untextured: geometry only |
| Berkefeld water filter, https://sketchfab.com/3d-models/berkefeld-water-filter-ff0bf0e32e024d729db9cd9fed594cce | Sketchfab, Virtual Museums of Małopolska (WirtualneMuzeaMalopolski), 2020 | **CC0** | museum scan of a historic (1900s) ceramic gravity filter; 8K texture; Berkefeld is a maker's name, so check the texture for a printed mark |
| Water Filter-purifier, https://sketchfab.com/3d-models/water-filter-purifier-with-water-bottle-19c845cf44c94057af0632c7bbc6c8a7 | Sketchfab, Nihindu Vinthaka, 2021 | CC BY 4.0 | countertop filter, low-poly |
| Pump, https://app.gazebosim.org/OpenRobotics/fuel/models/Pump | Gazebo Fuel, OpenRobotics, 2024 | CC BY 4.0 | PBR (albedo, metallic, normal, roughness) |
| Centrifugal Water Pump, https://sketchfab.com/3d-models/centrifugal-water-pump-c2a1d16920db45a7a6d74c53f841af77 | Sketchfab, Poly-Arms, 2026-04-05 | CC BY 4.0 | 3,658 tris, 4K; the description's "no copyright issue" is not a licence: the CC BY label governs |
| Old Water Pressure Booster Pump, https://sketchfab.com/3d-models/old-water-pressure-booster-pump-41c1a132baf544fa9ffd668bb056f52e | Sketchfab, Dasamura, 2024 | CC BY 4.0 | pump with pressure tank, f39,782, 4K; fits `pump_1` + the household water system |
| Water Pump, https://sketchfab.com/3d-models/water-pump-76ca12fd7aec4748b96445892b1a5d95 | Sketchfab, OOtis_Pro (Mohit.Lodhi), 2024 | CC BY 4.0 | electric pump, f150,430 |
| CENTRIFUGAL PUMP - HORIZONTAL END SUCTION, https://sketchfab.com/3d-models/centrifugal-pump-horizontal-end-suction-dee0b9325925453eb20acfcbeb1add91 | Sketchfab, ENGIPROS, 2024 | CC BY 4.0 | from engineering CAD; textures 512 |
| vertical pump | FreeCAD library (Hydraulics) | CC BY 3.0 | CAD |
| `Barrel_02`, `modular_pipes`, `modular_pipes_plastic_01`, `garden_hose_wall_mounted_01` | Poly Haven | CC0 | storage barrel and plumbing |
| Big plastic water tank, https://sketchfab.com/3d-models/big-plastic-water-tank-e9ca331f6e394d498ccdf295c561f2ef | Sketchfab, Mouad Hsaine, 2022 | CC BY 4.0 | f6,420, PBR; for `cistern_1` (SimReady's IBC tote already planned) |
| STM039-Septic Tank, https://sketchfab.com/3d-models/stm039-septic-tank-4b817c03c69c4ef0a58b2a6d82d145ae | Sketchfab, TribbleTrouble, 2020 | CC BY 4.0 | photogrammetry of a real septic tank, 8K; needs heavy decimation |
| CC0 wheel and ball valves | Gazebo Fuel, OpenRobotics | CC0 | plumbing detail |

### Power: batteries, inverters, charge controllers, generators, panels

| Item | Source | Licence | Notes |
|---|---|---|---|
| **Battery bank (LiFePO4 rack)** | none found | | model our own (simple boxes, terminals, busbars); FreeCAD's parametric LiPo and AA/AAA cells are CC BY 3.0 detail |
| PPB - Pro Portable Battery, https://sketchfab.com/3d-models/ppb-pro-portable-battery-32a260ed44e84d6bb3933570d4d9a9a9 | Sketchfab, re1monsen, 2021 | CC BY 4.0 | a portable power station design, f254,990, 171 MB (heavy) |
| Solar Power Inverter, https://sketchfab.com/3d-models/solar-power-inverter-d883410a5c384ef6b8e12a2f696961b8 | Sketchfab, vernygora2019, 2025-10-31 | CC BY 4.0 | f7,171, 4K. **Best inverter pick** (no brand named) |
| Solar Inverter Charger, https://sketchfab.com/3d-models/solar-inverter-charger-c59ad7d3919c482a92e07f5e27b2d1b4 | Sketchfab, MohJeha, 2022 | CC BY 4.0 | "34cm x 26cm x 9cm", 824 faces, 4K |
| Inverter, https://sketchfab.com/3d-models/inverter-9c3623c5eda049efad0e005b86bfd163 | Sketchfab, One80 Solar (an installer), 2019 | CC BY 4.0 | no description; likely a real product, check for a maker mark |
| **Solar charge controller** | none found anywhere | | model our own (a finned box with a display) |
| `portable_generator` | Poly Haven, 2026-07-03 | CC0 | 26,419 tris, fits `gen_1` |
| Diesel generator, https://sketchfab.com/3d-models/diesel-generator-7a0b72c7d01944a8aafab59d44eb120e | Sketchfab, epilogueronin, 2021 | CC BY 4.0 + NoAI tag | avoid until the finding |
| Solar Panel, https://sketchfab.com/3d-models/solar-panel-83483a66f8974d7e8f2e7bbde518d606 | Sketchfab, rivetech, 2021 | CC BY 4.0 | panel on a stand, f1,854 |
| `power_box_01`, `utility_box_01/02`, `modular_electric_cables` | Poly Haven | CC0 | breaker board and distribution |
| Electrical Breaker Panel Box - LP model, https://sketchfab.com/3d-models/electrical-breaker-panel-box-lp-model-4b12b4e00fc14f95beb55d7016eaf28d | Sketchfab, Nikoleta.Zhecheva, 2025 | CC BY 4.0 | open panel with wiring, f4,698, 4K |
| Electrical Box | Gazebo Fuel, OpenRobotics | CC0 | |
| `old_military_compressor` | Poly Haven | CC0 | compressor for air or gas |

### Space-habitat reference

| Item | Source | Licence | Notes |
|---|---|---|---|
| International Space Station (ISS) (E) (Internal) | NASA 3D Resources | NASA guidelines / NOSA 1.3 (unsettled) | ~304 MB 7z; the open reference for station racks |
| Orbital Replacement Unit, Habitat Demonstration Unit, Crew Lock Bag | NASA 3D Resources | same | glb |
| Root Module - Biomass Production System, https://sketchfab.com/3d-models/root-module-biomass-production-system-0bd845b747f44183a217be6cf7cfcd7f | Sketchfab, Kami Rapacz, 2023 | CC BY 4.0 | plant-growth rack "loosely based on experiments conducted aboard the International Space Station"; f92,805, 154 MB, "not very well optimised" |
| Space Colony Modular Kit Bash, https://sketchfab.com/3d-models/space-colony-modular-kit-bash-1ac694729ad04977ba93dafa66914aff | Sketchfab, R-LAB, 2022 | CC BY 4.0 | sci-fi interior kit, 4K |
| Apollo 11 Command Module Interior | Smithsonian (also on Sketchfab as CC0) | CC0 | heritage reference |

---

## 6. Best general sources for future props, textures and HDRIs

- **Props:** Poly Haven first (CC0, photoreal, real scale, glTF); then NVIDIA SimReady (CC BY 4.0,
  the finding exists); then Gazebo Fuel's OpenRobotics models and Amazon Berkeley Objects (CC BY 4.0,
  glTF, PBR); then Sketchfab CC BY 4.0 per item, vetted. For machines nobody has modelled, the FreeCAD
  library's real-dimension CAD and Infinigen's generators are the start of our own models.
- **Textures and materials:** ambientCG (2,041 CC0 materials) and Poly Haven (869 CC0 textures);
  Physically Based (CC0) for the measured values behind them. Not ShareTextures (restricted) or
  Megascans (not open); cgbookcase only after its author's terms are confirmed.
- **HDRIs:** Poly Haven (997, CC0) and ambientCG (435, CC0). Blendkit's 6,886 CC0 HDRs are worth a
  later look but overlap Poly Haven.

---

## 7. What the dated legal finding(s) should settle

1. **Poly Haven and ambientCG**: re-quote both licences in full with the date; confirm no API-terms
   obligation reaches the shipped files (the Poly Haven API page says it does not).
2. **Sketchfab**: §4.1 ("most-current version") against CC BY 4.0's irrevocable grant; §15 NoAI and
   whether an AI-run conversion pipeline is "inputs to generative AI programs"; what a credit line
   must hold; the KitBash ownership change. Per-item: licence label at download, brand check, fan-art
   check, AI-generated check, NoAI check. Downloads need the operator's logged-in account.
3. **NASA**: README "without copyright" versus `meta.json` NOSA 1.3; which contributors are not
   NASA; insignia rules.
4. **FreeCAD library**: CC BY 3.0's attribution terms; authors per part from the git history.
5. **Infinigen**: whether generated meshes carry any condition (BSD covers code), and the CC0 code
   pieces it mentions.
6. **Amazon Berkeley Objects**: "Amazon.com" credit wording; brand names on products; which
   categories exist (its metadata listing).
7. **Kenney's Furniture Kit item names**: confirm against the official zip (read here from a mirror).
8. **Megascans/Fab**: record the negative finding properly, from the English EULA text.

## 8. Sources that refused automated reading (not worked around)

si.edu/openaccess/faq and 3d.si.edu (403), thingiverse.com/legal (403), opengameart.org (403),
grabcad.com/terms (403), tutkit.com news (403). The Smithsonian's public API rate-limited the
documented demo key after nine queries; the scan stopped there.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so a later reader can tell whether they changed or this reading was wrong. Nothing
here is adopted until a dated finding clears it.
