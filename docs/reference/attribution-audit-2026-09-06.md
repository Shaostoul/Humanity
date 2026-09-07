# Attribution audit, 2026-09-06

An 88-agent read-only audit of every external source this repo uses, run
against CREDITS.md, data/credits.ron and LICENSES.md. Seven disjoint sweep
angles (fetch scripts, real-world data provenance, assets and fonts, shipped
documents, vendored web code, the dependency tree and external catalogs, and
verification of the already-declared surfaces) produced 148 source claims,
reconciled to 24 candidate gaps. Each gap then faced three adversarial
verifiers with distinct lenses (is it actually missing, is attribution
actually required, is it actually shipped). Sixteen survived; eight were
refuted and are recorded below so they are not re-raised.

Nothing here is legal advice. It is a developer-written record of what the
repo says versus what it ships, with file paths so each item can be checked.

## Surviving findings

### 1. egui / epaint bundled fonts, Ubuntu-Light, Hack-Regular, NotoEmoji-Regular, emoji-icon-font (via epaint_default_fonts 0.31.1)

**LEGAL_OBLIGATION** (3/3 confirmed)

All four typefaces are statically compiled into HumanityOS.exe and are the
entire native UI type system, yet no copyright notice or licence text for any
of them exists anywhere in the repo or in a release archive. The crate's own
licence expression is AND-joined, so OFL-1.1 and the Ubuntu Font Licence apply
ON TOP of the code licence, and both require their notice and text to travel
with the redistributed font software. This is the single largest attribution
obligation in the project and the only one with no surface at all.

**Where it goes:** A THIRD-PARTY-LICENSES file bundled into the release archive by
.github/workflows/build-desktop.yml (the four licence texts verbatim), plus a
CREDITS.md 'Technology' subsection naming the four faces. Not credits.ron, that file is scoped to data and its in-app page is headed 'Real-world data
HumanityOS is built on'.

**Evidence:** src/gui/fonts.rs:30 `let mut fonts = egui::FontDefinitions::default();`
(verified by me) is what pulls all four in. Registry:
epaint_default_fonts-0.31.1/Cargo.toml `license = "(MIT OR Apache-2.0) AND
OFL-1.1 AND Ubuntu-font-1.0"`; its fonts/ dir holds
Hack-Regular.ttf+Hack-Regular.txt, NotoEmoji-Regular.ttf+OFL.txt,
Ubuntu-Light.ttf+UFL.txt, emoji-icon-font.ttf+emoji-icon-font-mit-license.txt
(I listed the directory). I ran `grep -in
"font|ubuntu|hack|noto|emoji|twemoji|noble|symphonia|mpl" CREDITS.md
LICENSES.md data/credits.ron docs/reference/asset-and-map-sources.md`, the
only two hits are unrelated lines about a 'Simple Vegetation Pack'. Zero
coverage. (Separately, src/gui/fonts.rs:16-17 correctly notes the SYSTEM emoji
font is read from the user's machine and not redistributed, that one is
fine.)

### 2. The Rust dependency tree compiled into both shipped binaries (556 crates in the desktop graph): MIT, BSD-2/3-Clause, ISC, Zlib, Apache-2.0, Unicode-3.0, CDLA-Permissive-2.0

**LEGAL_OBLIGATION** (3/3 confirmed)

MIT, BSD-2/3-Clause, ISC, Zlib and Apache-2.0 section 4(a) all condition
redistribution, binary redistribution included, on reproducing the copyright
notice and licence text. The release archive contains no licence text of any
kind, not even the project's own root LICENSE. CREDITS.md's generic thanks to
'Rust, wgpu, egui, hecs, rapier3d, kira, axum, SQLite, and the whole
open-source ecosystem' does not reproduce a single notice. BSD-3-Clause clause
2 is the sharpest: redistributions in binary form must reproduce the notice
'in the documentation and/or other materials provided with the distribution', that covers ed25519-dalek and curve25519-dalek, i.e. the wallet identity path.

**Where it goes:** A generated THIRD-PARTY-LICENSES file (cargo-about or cargo-deny over the
exact target-filtered graph) copied into dist/HumanityOS/ alongside the
existing `cp -r data/` lines, plus LICENSE and LICENSES.md at the archive
root. One commit in .github/workflows/build-desktop.yml.

**Evidence:** I read .github/workflows/build-desktop.yml:69-107 in full: the bundle step
copies only the exe, the raw binary, dxcompiler.dll + dxil.dll, `cp -r data/`,
`cp -r assets/icons/`, `cp -r assets/shaders/`. There is no licence copy step
and no LICENSE at the archive root. The sweep's census (read from each crate's
own Cargo.toml in the local registry) found 270 'MIT OR Apache-2.0', 75 'MIT',
57 'Apache-2.0 OR MIT', 23 'MIT/Apache-2.0', 20 'Apache-2.0', 18
'Unicode-3.0', 11 'BSD-3-Clause', 8 'ISC', with 513 of 556 crates shipping a
LICENSE/COPYING file precisely because of this. Mitigating fact worth
recording: a NOTICE-file scan across all 556 crate dirs returned zero, so the
Apache-2.0 section 4(d) NOTICE-propagation obligation is NOT triggered, only section 4(a).

### 3. MPL-2.0 crates linked into the desktop binary: the symphonia family (9 crates) and triple_buffer, pulled in unconditionally by kira

**LEGAL_OBLIGATION** (3/3 confirmed)

MPL-2.0 section 3.2 is a different KIND of obligation from the permissive crates: a
distributor of the Executable Form must inform recipients how to obtain the
Source Code Form of the Covered Software on reasonable terms. Nothing in the
repo, the build, or the release archive does this. It is easy to miss
precisely because it does not look like a notice-reproduction problem, and
because kira drags symphonia in without it appearing in Cargo.toml.

**Where it goes:** The same bundled THIRD-PARTY-LICENSES file, with an explicit MPL-2.0
source-availability line naming the symphonia crates and where to get their
source (crates.io / the upstream repo).

**Evidence:** I confirmed both ends. Cargo.lock:3371-3372 lists "symphonia" and
"triple_buffer" as dependencies of kira, and Cargo.lock:6276-6289 enumerates
the symphonia bundle (bundle-flac, bundle-mp3, codec-pcm, codec-vorbis, core,
format-ogg, format-riff, metadata, utils-xiph). Registry:
symphonia-core-0.5.5/Cargo.toml `license = "MPL-2.0"`. Cargo.toml:42 declares
only `kira = { version = "0.9", optional = true }`, symphonia is invisible at
the declaration layer.

### 4. OpenStreetMap (ODbL 1.0), the Derivative Database half of the obligation, and the web/Library half of the Produced Work half

**LEGAL_OBLIGATION** (3/3 confirmed)

Three distinct holes in the project's best-documented obligation. (1) The ODbL
offer for data/maps/regions/*.bin lives ONLY in LICENSES.md, which is not in
the release archive, so anyone redistributing a release bundle receives the
Derivative Database with no ODbL offer travelling with it, which is exactly
the case LICENSES.md:31-35 says carries the obligation forward. (2) CREDITS.md, byte-identical to data/library/CREDITS.md, the only credits document a web
visitor or a Library reader can reach, never mentions OpenStreetMap at all.
(3) The in-world Produced Work notice is inside the HUD, and the HUD is gated
off in construction mode and the showroom while OSM geometry is still on
screen.

**Where it goes:** LICENSES.md copied into the release archive; an OpenStreetMap row added to the
'Data and imagery sources' section of CREDITS.md (which propagates to
data/library/CREDITS.md via scripts/build-library.js); and the in-world notice
moved out from behind the construction/showroom HUD gate in src/lib.rs.

**Evidence:** Release archive contents verified at
.github/workflows/build-desktop.yml:69-107, no LICENSES.md. I ran `grep -rn
-i "openstreetmap|odbl" CREDITS.md data/library/` and it returned ZERO hits.
The in-world notice is src/gui/pages/hud.rs:446-455 `if
!state.osm_regions_drawn.is_empty() { text_shadowed(...
crate::credits::OSM_NOTICE ...) }`, and I read the gate at
src/lib.rs:18381-18385: `if state.gui_state.active_page == GuiPage::None &&
state.gui_state.show_hud && !state.gui_state.showroom_active &&
!state.gui_state.construction_active { hud::draw(...) }`. What IS satisfied:
the native Maps planet-view footer (cosmos.rs:2089) and
web/pages/maps.html:1371, which prints '(c) OpenStreetMap contributors' with the
copyright link.

### 5. HYG star database (astronexus), the catalogue that actually ships, credited under the wrong name, with no licence pinned and a possible share-alike term unaddressed

**LEGAL_OBLIGATION** (3/3 confirmed)

Four compounding problems in one row. (a) data/stars.bin, the ~120k catalogue
every player sees by default, is HYG; credits.ron has no row for it and
instead attributes its work to ATHYG, a different (2.5M-row) astronexus
dataset that is gitignored and NOT shipped. (b) The row is marked
attribution_required: true but its licence field is the placeholder 'See the
ATHYG repository', the repo cannot say what the required attribution
requires. (c) CREDITS.md:67-69 gives a third, hedged answer ('AT-HYG (CC-BY-SA
/ public-domain components)') and docs/design/maps-multi-scale.md a fourth
('CC BY-SA 2.5'). (d) If CC BY-SA is correct, share-alike attaches to
data/galaxy_glow.png, a committed derived work that integrates the catalogue,
and nothing in the repo addresses that.

**Where it goes:** Pin the actual upstream licence, then split into two credits.ron rows (hyg for
the shipped standard tier, athyg for the extended download) with correct
used_for text; correct CREDITS.md:67-69 and LICENSES.md:55-56; and if
share-alike applies, state the galaxy_glow.png position in LICENSES.md.

**Evidence:** I read src/renderer/stars.rs:5-9 verbatim: 'STANDARD `data/stars.bin` (HYG,
~120k stars, ships with the app, generated by `scripts/build-stars-bin.js`),
EXTENDED `stars-athyg.bin` (ATHYG, ~2.5M, in-app download...)'. Against that,
data/credits.ron's athyg row says `used_for: "The standard star catalogue
(~120k stars)..."` and `licence: "See the ATHYG repository"`. The file's own
header comment claims 'Every entry here was checked against the code that
loads the data, not against memory', that check missed the one catalogue that
ships. stars-athyg.bin is gitignored (.gitignore:90).

### 6. @noble/post-quantum 0.6.1 and @noble/hashes 2.2.0, vendored as web/shared/vendor/noble-pq.bundle.js

**LEGAL_OBLIGATION** (3/3 confirmed)

The build deliberately strips upstream legal headers, and the shipped 61 KB
file, served same-origin from united-humanity.us and carrying the primary
chat identity crypto, contains no copyright line, no permission notice, and
no licence name. The build then deletes its temp npm workspace, so no upstream
LICENSE text survives anywhere in the checkout either. If the upstream is MIT
(as npm publishes these packages), MIT's notice-retention clause is not
satisfied by the distributed artifact. The licence is genuinely UNCLEAR from
the repo, and that is itself the gap: nothing here records it. What I
checked: the bundle, scripts/build-noble-bundle.mjs, package.json, CREDITS.md,
LICENSES.md, data/credits.ron.

**Where it goes:** Fixed at the generator: drop `--legal-comments=none` (or prepend the upstream
licence text to the written header) in scripts/build-noble-bundle.mjs so the
notice regenerates automatically. Also a 'Technology' line in CREDITS.md.

**Evidence:** I ran `grep -ciE "copyright|licen[cs]e|paulmillr"
web/shared/vendor/noble-pq.bundle.js` -> 0. I read the file's entire header
(lines 1-12): it records the source packages, versions, exports, a KAT and a
sha256, and no licence. The cause is scripts/build-noble-bundle.mjs:56: ``
`--target=es2022 --legal-comments=none --outfile=bundle.js` ``. The contrast
case in the same tree proves the house can do it right:
web/shared/qrcode.js:5-9 retains 'Copyright (c) 2009 Kazuhiko Arase ...
Licensed under the MIT license', and web/chat/twemoji.min.js:1 retains its MIT
banner.

### 7. dxcompiler.dll and dxil.dll (Microsoft DirectX Shader Compiler, copied from the runner's Windows SDK)

**LEGAL_OBLIGATION** (2/3 confirmed)

Two prebuilt Microsoft binaries are shipped INSIDE the Windows release archive
next to the exe. The entire licence record in the repo is one clause in a code
comment asserting they are MIT. No licence text, no source URL, no version is
recorded anywhere. If the assertion is right, MIT requires the notice to
accompany the copy and it does not. Whether the assertion is even correct
cannot be resolved from repo evidence, dxil.dll in particular is a
Microsoft-signed binary whose redistribution terms are not necessarily the
compiler source's terms. I checked the repo root, the workflow, LICENSES.md,
CREDITS.md and data/credits.ron.

**Where it goes:** A pinned licence record (text + upstream URL + version) in the bundled
THIRD-PARTY-LICENSES file, and the assertion in
.github/workflows/build-desktop.yml replaced by a reference to that record.

**Evidence:** I read the staging block at .github/workflows/build-desktop.yml:76-88. The
whole licence record is the comment at line 80-81: 'The hosted runner's
Windows SDK ships both; bundle the newest. MIT-licensed, redistributable.' The
copy is line 84: `cp "$SDKBIN/dxcompiler.dll" "$SDKBIN/dxil.dll"
dist/HumanityOS/`.

### 8. Twemoji emoji ARTWORK (jdecked/twemoji@17.0.3 assets, hotlinked from cdn.jsdelivr.net)

**LEGAL_OBLIGATION** (3/3 confirmed)

Every emoji rendered anywhere in the web chat is an <img> pointing at the
upstream asset set, plus one hardcoded bot-badge SVG. The vendored parser's
MIT banner covers the CODE only; the graphics are a separate work upstream and
their licence is recorded nowhere in this checkout. If they are CC-BY as
upstream publishes them, attribution would be required wherever the emoji are
displayed, and no such credit exists anywhere in web/, CREDITS.md, LICENSES.md
or data/credits.ron. Flagged as conditional rather than confirmed: the repo
does not state the artwork licence, which is the gap.

**Where it goes:** CREDITS.md (a 'Web' or 'Technology' subsection), and if CC-BY is confirmed, a
visible credit on the chat surface itself.

**Evidence:** web/chat/twemoji.min.js:1 '/*! Copyright Twitter Inc. and other contributors.
Licensed under MIT */', code only. Line 2 sets
`base:"https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.3/assets/"` and no
override of `twemoji.base` exists anywhere in web/, src/ or scripts/. A second
hardcoded asset URL at web/chat/app.js:1788. web/chat/index.html:38-40
acknowledges the runtime fetch. My grep of
CREDITS.md/LICENSES.md/data/credits.ron for 'twemoji' returned zero hits.

### 9. Solar System Scope planetary textures (CC-BY 4.0), Moon and Mars albedo bakes

**INCONSISTENCY** (3/3 confirmed)

The one CC-BY, attribution-genuinely-required source whose output SHIPS
(moon_albedo.bin and mars_albedo.bin, 25 MB each, both git-tracked, both
inside the wholesale data/ copy in every release). It has no row in
data/credits.ron, so the in-app Settings > Credits page never names it and the
enforcing unit test cannot see it, a missing row passes silently. The
obligation is currently discharged by exactly one thread: the CREDITS.md line,
mirrored into data/library/CREDITS.md and rendered by the Library. That is a
real user-facing surface, so this is not an unmet obligation today, but it is
a single unguarded string away from becoming one.

**Where it goes:** A solar_system_scope row in data/credits.ron with attribution_required: true
and shown_in naming the Library plus Credits, and a section in LICENSES.md
alongside the other obligation-bearing sources.

**Evidence:** I verified both bins are tracked: `git ls-files data/planets/` lists
mars_albedo.bin and moon_albedo.bin. Provenance is in-tree at
data/planets/moon.ron:13-14 '4096x2048 RGB grid baked from the Solar System
Scope 8k moon texture (CC-BY 4.0, based on NASA imagery) by
scripts/build-planet-albedo.js' (and mars.ron:24 for Mars). data/credits.ron
holds exactly five ids, openstreetmap, nasa_gibs, nasa_blue_marble, athyg,
esa_gaia, I read the whole file. The test that should catch it,
src/credits.rs:110 `every_required_attribution_names_a_surface`, iterates
`c.sources` only.

### 10. AWS Open Data 'Terrain Tiles' (Mapzen / Tilezen terrarium PNG tiles), the .dem.bin elevation companions

**INCONSISTENCY** (3/3 confirmed)

LICENSES.md misattributes an entire upstream. It states that
data/maps/regions/*.bin 'plus their .dem.bin elevation companions' are built
from OpenStreetMap by scripts/fetch-osm-region.mjs. The .dem.bin files come
from a completely different source, the AWS elevation-tiles-prod S3 bucket,
via a different script, and this sweeps a source with unrecorded terms under
the ODbL heading. The tile set's own attribution guidance (tilezen/joerd) is
cited by the script but its content is nowhere in the repo, so what it obliges
is unclear from what is here. The underlying data for the shipped Puget Sound
regions is asserted to be US public domain, so no obligation is likely, but
the declaration as written is false.

**Where it goes:** Correct LICENSES.md:14-16 to scope the ODbL paragraph to the region .bin files
only, and add a separate short section (or a credits.ron row) for the terrain
tiles naming the actual upstream.

**Evidence:** LICENSES.md:14-16 (read in full): '`data/maps/regions/*.bin` (currently
`silverdale`, `seattle-center`, plus their `.dem.bin` elevation companions)
are built from OpenStreetMap by `scripts/fetch-osm-region.mjs`.' The real
producer, which I read: scripts/fetch-region-dem.mjs:148 `const TILE_URL = (z,
x, y) =>
\`https://s3.amazonaws.com/elevation-tiles-prod/terrarium/${z}/${x}/${y}.png\`;`
and its header at :18-30 'DATA SOURCE: the AWS Open Data "Terrain Tiles" ...
Underlying sources for the shipped Puget Sound regions are United States
public-domain datasets (USGS 3DEP/NED, NASA SRTM, NOAA bathymetry).'

### 11. LICENSES.md's claim about what the credits unit test enforces

**INCONSISTENCY** (2/3 confirmed)

LICENSES.md tells a reader that a unit test 'fails if a source marked
attribution_required does not name a surface where its notice is actually
shown'. The word 'actually' is not true: the test only checks that the
shown_in vector and the notice string are non-empty. shown_in is unvalidated
free text, `["Anywhere at all"]` passes. No test opens any renderer file, and
nothing checks that OSM_NOTICE is even referenced; deleting both paint sites
leaves all four tests green. A maintainer relying on this sentence will
believe the machinery is stronger than it is.

**Where it goes:** Correct the sentence in LICENSES.md:8-10 to say what the test does check, or
strengthen the test to grep the named surfaces.

**Evidence:** LICENSES.md:8-10 verbatim. Against it, I read src/credits.rs: the four tests
are `shipped_credits_file_parses` (:83), `osm_notice_matches_data` (:95, a
string-vs-string comparison), `every_required_attribution_names_a_surface`
(:110, two non-empty asserts at :113 and :118), and
`osm_names_both_drawing_surfaces` (:129, two substring checks on the joined
shown_in text at :133 and :138). None of them reads a renderer.

### 12. CARTO basemap tiles (basemaps.cartocdn.com)

**INCONSISTENCY** (2/3 confirmed)

A live third-party network dependency of the public website, every
street-level map tile on the web Maps page, that the attribution registry
does not know exists. No row in data/credits.ron, no section in LICENSES.md,
no line in CREDITS.md. The page's own author evidently believed a credit was
required (it prints one), but the basis is recorded nowhere, and the credit is
shown only when zoomed past the tile threshold on a planet with a map.

**Where it goes:** A carto row in data/credits.ron (or, since credits.ron is native-only, a
CREDITS.md 'Data and imagery sources' line) recording the actual basemap
terms.

**Evidence:** web/pages/maps.html:563 `` img.src =
`https://basemaps.cartocdn.com/dark_all/${z}/${x}/${y}.png`; ``. The
conditional credit is web/pages/maps.html:1371, which I read in my own grep:
`attribution.innerHTML = '&copy; <a
href="https://www.openstreetmap.org/copyright"
target="_blank">OpenStreetMap</a> contributors | &copy; <a
href="https://carto.com/" target="_blank">CARTO</a>';` My grep of
CREDITS.md/LICENSES.md/data/credits.ron for CARTO returned nothing.

### 13. data/external/catalog.json, licence claims published about 40 third-party software projects

**INCONSISTENCY** (3/3 confirmed)

The shipped Tools catalogue asserts a licence for each software entry and
shows it to users, with no source, no date and no verification note anywhere
in the file. Several assertions look checkably wrong: Paint.NET as 'MIT'
(closed-source freeware under its own EULA since v4), VS Code as 'MIT' (that
covers Code-OSS source, not the Microsoft-branded binary the linked download
provides), and Aseprite as 'GPL (source)' with 'Compile from source for free'
(Aseprite left the GPL in 2016). Not our own attribution obligation, but it is
the project publishing unverified licence claims about other people's work, in
a repo whose whole house rule is verify-before-you-state.

**Where it goes:** data/external/catalog.json, either add a verified-on date per entry, or drop
the license field where it has not been checked.

**Evidence:** data/external/catalog.json entries: {"name": "Paint.NET", "license": "MIT"},
{"name": "VS Code", "license": "MIT"}, {"name": "Aseprite", "license": "GPL
(source)", "description": "Professional pixel art and animation. Compile from
source for free."}. No sourcing metadata exists in the file. I did not verify
these against upstream (no network use), so they are flagged as unverified
claims, not as confirmed errors.

### 14. NOAA / NCEI ETOPO (ETOPO1 and ETOPO 2022)

**COURTESY** (2/3 confirmed)

Source of the single largest shipped data file (earth_heightmap.bin, 51.8 MB)
and the ocean mask derived from it, yet it has no row in data/credits.ron and
no section in LICENSES.md, so the in-app Credits page never names it. Public
domain, so nothing is breached; this is the project's own house rule going
unmet. Secondary drift worth fixing while there:
scripts/build-earth-heightmap.js (ETOPO1, 3600x1800) no longer produces the
shipped file, which is 7200x3600 from ETOPO 2022 via
examples/build_earth_grid.rs, and four Rust doc comments still name ETOPO1.

**Where it goes:** A noaa_etopo row in data/credits.ron (attribution_required: false, shown_in:
["Credits"]), matching the existing NASA rows.

**Evidence:** CREDITS.md:63-64 '**NOAA/NCEI ETOPO** global relief (terrain + bathymetry
tiles) - public domain.' I read data/credits.ron in full: five ids, none of
them NOAA. Public-domain basis: scripts/download-etopo2022.js:7-8 'Public
domain (US government work)' and scripts/build-earth-heightmap.js:7 'ETOPO1 is
a US government work: public domain, free to ship.'

### 15. The science tables with no provenance at all: data/chemistry/ (5 files, 488 rows), the 9 domain RON tables (geology, oceanography, medical, aging_fitness, electrical, hvac, genetics, psychology, astronomy_tools), the astronomy/geography JSONs (cities, coastlines, milky-way, stars-catalog, stars-nearby), and materials/items/creatures/recipes.csv

**COURTESY** (2/3 confirmed)

None of these names an upstream. No attribution obligation arises, numbers
are facts and facts are not copyrightable, but the project has made citation
a house rule and these are the tables where it is not met. Two are worth
singling out on safety rather than licensing grounds: toxins.csv carries 79
rows of lethal_dose_mg_kg, onset times and antidotes with no source, and
compounds.csv likewise carries lethal_dose values. If any of these were
transcribed from a copyrighted compilation (a CRC-style handbook) rather than
from primary NIST/IUPAC sources, that would be a different conversation, but
nothing in the repo says either way.

**Where it goes:** A '# Sources:' header line per file, following the pattern the repo already
uses well at data/plants.csv:31 and data/food/crop_nutrition.ron:14.

**Evidence:** All five chemistry files begin at the column header row with no comment block:
data/chemistry/elements.csv:1 is
'atomic_number,symbol,name,category,atomic_mass...'. The nine RON files each
have a header comment that documents units only, e.g. data/hvac.ron:5 '//
output_btu = thermal output in BTU/hr (1 BTU/hr ~ 0.293 watts thermal).' The
JSONs' only code reference is a bare embed, src/embedded_data.rs:41-46.
Contrast that proves it is a gap and not a house style: data/plants.csv:31 '#
Sources: USDA Plant Hardiness, FAO crop water requirements, Purdue/Cornell
extension data'.

### 16. The extension-service and federal citations across the ten Real Skills guides (USDA, CDC, EPA, FDA, DOE, FEMA/Ready.gov, WHO, NCHFP/UGA, and ~24 university Cooperative Extension services)

**COURTESY** (2/3 confirmed)

No gap in the obligation sense, and I am recording it explicitly so it is not
re-raised as one. The guides are original prose that CITES sources, not
datasets extracted from them: what is taken is facts (yields, blanching times,
bleach drops per gallon, processing minutes by altitude) and uncopyrightable
procedures, plus short attributed quotations, the longest about 35 words. US
federal works carry no attribution requirement. The one open question is WHO,
which is not a 17 USC 105 federal work and whose terms are nowhere recorded, but the use is a 24-word attributed quotation plus a pointer, which is
defensible under any of the terms WHO commonly applies.

**Where it goes:** Nowhere new. If anything, a one-line note in LICENSES.md stating that the
Library's prose cites sources for verifiability rather than under licence, so
a future audit does not mistake citation for extraction.

**Evidence:** The one genuine extraction in the Library is data/library/us-constitution.md,
and it is public domain: line 5 names the GPO/govinfo USLM source and
scripts/build-constitution.js:22-28 records 'Public domain. A work of the US
Government is not copyrightable under 17 U.S.C. 105(a).' The citation practice
is real and shipped: data/library/storing_water_safely.md:214-221 carries a
'## Sources' section listing cdc.gov, epa.gov, extension.usu.edu,
ohioline.osu.edu and ready.gov, and src/gui/mod.rs:6800 reads the full
markdown body so those sections render in-app.

## Refuted, do not re-raise without new evidence

### 1. data/credits.ron is never loaded until the user enters the 3D world, the
'Credits' surface is broken for all five rows

Claimed: `state.gui_state.credits` is populated in exactly one place, inside
load_world, which is called lazily on first Enter World. Until then the field
is Credits::default() (empty), so Settings > Credits hits its empty guard and
shows a warning instead of any attribution. For ESA Gaia and ATHYG, 'Credits'
is the ONLY surface named in credits.ron, so for a user who never enters the
3D world (the chat-first path the lazy load exists to serve) their required
attribution is shown nowhere in the app. The snapshot test cannot catch this
because ui_snapshots.rs loads credits itself before rendering.

Refuted because: Corrected statement: `state.gui_state.credits` is indeed
populated only inside load_world (src/engine/world_load.rs:901), so Settings >
Credits renders its empty-guard warning until the user first enters the 3D
world. That is a real but LOW-severity UX bug, and the warning text is
actively misleading because it blames a missing/broken data file when the file
ships fine and was simply never read. It is NOT an attribution gap and NOT a
legal obligation. Two independent reasons: (1) ESA Gaia and AT-HYG attribution
is displayed on the native Library page, which loads eagerly at startup
(src/lib.rs:1459, load_library at src/gui/mod.rs:6768) from
data/library/CREDITS.md, indexed under the first category "Credits" as
"Credits and Thanks", and the Library page defaults its selection to exactly
that first document (src/gui/pages/library.rs:80-92); Library is a top-level
tab and a valid boot page (src/gui/mod.rs:678). (2) The Gaia/ATHYG starfield
only renders when active_page == GuiPage::None (src/lib.rs:17763-17772), which
is the same condition that calls load_world 80 lines earlier in the same frame
(src/lib.rs:17683), so credits are always loaded before any Gaia-derived star
is drawn. Recommended action is therefore not "load credits at app init to
satisfy a legal obligation" but the much smaller: load credits in the eager
startup block next to load_library so the Settings > Credits page is never
falsely empty, and fix the warning wording. Optionally, update credits.ron's
`shown_in` for athyg/esa_gaia, which lists only ["Credits"] and so understates
where the notice actually appears (it omits the Library page). Two things I
could not verify and do not assert: whether CREDITS.md's licence
characterisations are accurate (Gaia as CC-BY-SA-IGO 3.0; AT-HYG as "CC-BY-SA
/ public-domain components"), and the ATHYG licence generally -
data/credits.ron itself says only "See the ATHYG repository", so from what is
in this repo the ATHYG licence is UNCLEAR. I checked data/credits.ron,
data/library/CREDITS.md and the credits render surfaces; I did not consult the
upstream repositories. || The load-site fact is correct but the finding's
severity and its central assertion are wrong. CORRECTED STATEMENT:
`state.gui_state.credits` is populated only at src/engine/world_load.rs:901
inside `load_world`, so Settings > Credits shows its empty-guard warning
(src/gui/pages/settings.rs:3859-3869) until the user first enters the 3D
world. This is a COSMETIC / UI defect, not a legal-obligation gap. It is not a
legal gap because: (a) ESA Gaia and AT-HYG attributions ARE shown to a
chat-first user, on a different surface loaded at app init. src/lib.rs:1459
loads the Library inside `fn resumed` (src/lib.rs:891); load_library
(src/gui/mod.rs:6768-6808) reads each doc body from disk there;
data/library/index.json's first category is "Credits" -> CREDITS.md; and
data/library/CREDITS.md:67-69 carries "ESA Gaia DR3 and the AT-HYG star
catalog compilation ... ESA/Gaia/DPAC (CC-BY-SA-IGO 3.0) and AT-HYG (CC-BY-SA
/ public-domain components)". Library is a nav page (src/gui/mod.rs:678)
dispatched with no world_loaded gate (src/lib.rs:18291) and renders the body
(src/gui/pages/library.rs:194-202). (b) Neither catalogue is even shipped to
that user: data/stars-athyg.bin and data/stars-gaia25m.bin are untracked in
git and are in-app downloads from a GitHub release asset
(scripts/build-athyg-bin.js:10-13; scripts/build-gaia-bin.js:8-12). (c) The
only ODbL-grade obligation (OpenStreetMap's rendered Produced Work) does not
read credits.ron at all -- it uses `const OSM_NOTICE` (src/credits.rs:72) at
src/gui/pages/cosmos.rs:2089 and src/gui/pages/hud.rs:451, by explicit design
(src/credits.rs:66-70). Two smaller REAL issues surfaced while refuting, worth
filing separately at low severity: 1. credits.ron's `shown_in: ["Credits"]`
for athyg and esa_gaia is UNDER-recorded: it omits the in-app Library, which
is where those notices actually reach a pre-world-entry user. The file's own
header says shown_in exists "so an audit can tell at a glance whether an
obligation is met", so the record should name that surface. 2. credits.ron's
ATHYG row credits the wrong catalogue for the data actually shipped. The
shipped ~120k catalogue is HYG (scripts/build-stars-bin.js:5 "data/stars.csv
(HYG star catalog, ~34 MB, ~120k rows)" -> data/stars.bin, tarred into the
release bundle), a different astronexus repo from ATHYG-Database; ATHYG is the
~2.5M optional download (scripts/build-athyg-bin.js:5-13). The shipped HYG
catalogue has no row of its own. Licence status per this lens: ESA Gaia's
attribution requirement is NOT overstated (data/library/CREDITS.md:68 states
CC-BY-SA-IGO 3.0). The ATHYG licence is UNCLEAR from what is in the repo --
data/credits.ron says only "See the ATHYG repository", LICENSES.md's
star-catalogue section states no licence at all, and
data/library/CREDITS.md:68-69 says "CC-BY-SA / public-domain components"; no
LICENSE file for the catalogue is vendored. I checked those four files plus
scripts/build-athyg-bin.js. Do not assert an ATHYG obligation without checking
the upstream repository. || The severity is wrong, and one premise is
factually false. Corrected statement: "`state.gui_state.credits` is populated
in exactly one place, src/engine/world_load.rs:901, inside load_world, so
Settings > Credits renders its empty-state warning ('Could not read
data/credits.ron. That is a bug worth reporting: attributions are not
optional.') until the user first enters the 3D world. Because an onboarded
user boots to a configured default_page (src/lib.rs:1649-1656;
BOOT_PAGE_OPTIONS at src/gui/mod.rs:668-679 offers
Humanity/Chat/Tasks/Maps/Notes/Calendar/Library, none of which is
GuiPage::None), a user may never trigger load_world at all, and the page can
stay broken for an entire session. It is the odd one out among ~a dozen data
loaders that already run at init in the same block (library, tower_configs,
garden_areas, grow_media, equipment_slots, bug_taxonomy...). Severity: normal
bug, NOT a legal obligation, the required attribution for both ESA Gaia and
AT-HYG is already displayed in-app pre-world via data/library/CREDITS.md lines
67-68, which is tracked, ships in the bundle, is the first category in
data/library/index.json ('Credits' -> 'Credits and Thanks'), and is loaded at
app init by src/lib.rs:1459 and rendered at src/lib.rs:18291. Fix: move the
Credits::load call to the init block near src/lib.rs:1459 and add a test
asserting state.credits is non-empty before world load. Separately,
credits.ron's shown_in fields are incomplete, they omit the Library surface
that is in fact carrying these attributions today."

### 2. ESA Gaia, used_for is factually wrong, and the required acknowledgement
wording never reaches a web reader

Claimed: Two separate defects on the one source whose acknowledgement wording
is explicitly specified. (a) credits.ron's used_for claims Gaia supplies 'the
integrated galaxy glow built from it'; the glow is baked from the ATHYG
catalogue, not Gaia. LICENSES.md:57-58 repeats the same error, and
CREDITS.md:67-68 makes the mirror-image error by crediting the 25M bake
jointly to Gaia and AT-HYG. (b) The verbatim acknowledgement renders only in
the native Settings > Credits page and in LICENSES.md (a repo file, not
shipped); no web surface contains the word 'Gaia' at all, and CREDITS.md
paraphrases rather than reproducing the wording Gaia asks for.

Refuted because: The claimed gap is wrong in both directions, and the fix it
proposes would introduce an error into a currently-correct file. Do NOT change
data/credits.ron's esa_gaia used_for or LICENSES.md:57-58: the glow genuinely
is the Gaia DR3 census bake. Three small residues survive, none of them the
claimed gap, all lower severity than INCONSISTENCY: 1. STALE COMMENTS (the
actual defect, and the thing that misled the claim). Two headers still
describe the superseded ATHYG-integration path as the glow's source:
scripts/build-galaxy-glow.js:5-7 ("integrates the light of every star in the
extended catalog (data/stars-athyg.bin, ~2.5M stars...)") and
src/renderer/stars.rs:18-21 ("generated by scripts/build-galaxy-glow.js from
the 2.5M-star extended catalog"). Both were true until
v0.804/v0.807.2/v0.810.2 moved the shipped bake to the Gaia census. Worth one
sentence each. This is a code-comment freshness issue, not an attribution one.
2. WORDING IMPRECISION, not misattribution. credits.ron's "the extended star
catalogue (Gaia G<14, ~25M stars) ... and the integrated galaxy glow built
from it" reads as though the glow comes from the 25M-star file; it actually
comes from a separate HEALPix density aggregate over all 1.81B Gaia sources.
Both are Gaia, so the credit is discharged either way. Optional tightening
only. 3. THE PARAPHRASE POINT IS PARTLY FAIR, THE REACH POINT IS NOT. It is
true that Gaia's exact requested sentence appears only in credits.ron
(rendered by native Settings > Credits via src/gui/pages/settings.rs:268 ->
draw_credits_content at :3842) and LICENSES.md:60-63, while CREDITS.md
paraphrases as "ESA/Gaia/DPAC (CC-BY-SA-IGO 3.0)". Reproducing the verbatim
sentence in CREDITS.md is a reasonable small improvement, since CREDITS.md is
the copy that reaches both the native Library and the live web Library. But it
should be filed as a polish item, not as "the acknowledgement never reaches a
web reader" - a Gaia credit demonstrably does. One thing I could not resolve
from the repo: credits.ron's licence field says "ESA Gaia data, freely
available with acknowledgement", CREDITS.md:68 says "CC-BY-SA-IGO 3.0", and
LICENSES.md:53-63 names no licence at all, only the acknowledgement. Which is
right is UNCLEAR from what is in the repo; I checked those three files and
docs/reference/asset-and-map-sources.md (no Gaia mention) and did not verify
the licence externally. || credits.ron's esa_gaia used_for and
LICENSES.md:57-58 are CORRECT, not wrong: the shipped 8192x4096
data/galaxy_glow.png is baked from the Gaia DR3 census via
build-galaxy-glow.js's --gaia path (bakeGaia, lines 449-486; 8192x4096 is
unreachable from the ATHYG path, which is fixed at 2048x1024), confirmed by
commits 7aeeb103 (v0.807.2 "THE REAL CENSUS SKY") and 919958f1 (v0.810.2
level-10 8192x4096) and by src/renderer/stars.rs:829. The Gaia acknowledgement
DOES reach a web reader: app/web/pages/library-app.js fetches
/data/library/CREDITS.md at runtime and data/library/CREDITS.md:67-68 names
ESA Gaia DR3 with its CC-BY-SA-IGO 3.0 licence; the claim's grep over static
web/ source could not see runtime-fetched markdown by construction.
Attribution is genuinely required and is correctly flagged, and the verbatim
wording is rendered where the data is actually used
(src/gui/pages/settings.rs:3894 prints src.notice unmodified;
LICENSES.md:60-63 block-quotes it); no web page renders Gaia-derived imagery,
so no web-surface obligation exists. The only real defect in this area is the
INVERSE of the one claimed and is out of scope for an attribution audit: the
doc comments at scripts/build-galaxy-glow.js:5-7 and
src/renderer/stars.rs:18-21 are stale, still describing the superseded ATHYG
star-integration bake as the source of the shipped glow. || The shipped galaxy
glow IS Gaia-derived, so data/credits.ron:69 and LICENSES.md:57-58 are
correct, not wrong: data/galaxy_glow.png is 8192x4096, a size only the Gaia
DR3 census path in scripts/build-galaxy-glow.js can produce (the ATHYG path is
fixed at 2048x1024, lines 125-131; the census ladder is at lines 474-487;
commit 919958f1 "LEVEL-10 GLOW"). Gaia also does reach a web reader:
data/library/CREDITS.md:67-68 names "ESA Gaia DR3 ... ESA/Gaia/DPAC",
scripts/sync-web-root.sh rsyncs data/library/ into the web root, and
web/pages/library-app.js:9-10 renders it at /library. The verbatim ESA
acknowledgement is shown where the data is actually used
(src/gui/pages/settings.rs:3894 renders each row's notice) and in
LICENSES.md:60-63. If anything is worth fixing here it is the opposite
direction of the claim: the stale docstrings at
scripts/build-galaxy-glow.js:5-7, src/renderer/stars.rs:18-21 and
scripts/gen-web-galaxy-bg.js:11 still call the glow an ATHYG/25M-catalog
integration, and credits.ron:69's "built from it" should say the glow comes
from the Gaia DR3 census aggregate rather than from the G<14 25M catalogue.
Both are cosmetic; no attribution obligation is unmet.

### 3. NASA New Horizons Pluto mosaic (NASA/JHUAPL/SwRI) and USGS Astrogeology
planetary mosaics

Claimed: Both appear in CREDITS.md but have no row in data/credits.ron, so
neither reaches the in-app Credits page. Both are US government works, so no
obligation. Worth noting a factual wrinkle for whoever fixes this: USGS is
credited for 'reference + future bakes' but is not the source of any shipped
file, the Moon and Mars albedos that ship were baked from Solar System Scope
instead, so the credit as written slightly overstates what USGS contributed.

Refuted because: Both sources ARE credited to players in-app, on a surface the
claim missed. CREDITS.md ships as a Library document:
data/library/CREDITS.md:73-76 carries both lines, data/library/index.json
lists "Credits" as its FIRST category ("Credits and Thanks" -> CREDITS.md),
src/gui/mod.rs:6789 loads it in manifest order, and src/gui/pages/library.rs
renders it as a top-level tab whose default selection is the first category's
first entry - so it is the document a player sees on opening Library.
web/pages/library.html:11 mirrors the same manifest. The correct statement of
the residual is therefore narrow and non-actionable: "New Horizons and USGS
are absent from data/credits.ron, so they do not appear on Settings > Credits,
but both are shown in-app on the default Library document; both are
public-domain US government works with no attribution obligation." The USGS
wrinkle should be dropped, not fixed. CREDITS.md:75-76 already reads
"(reference + future bakes)", which explicitly discloses that nothing ships
from it - it does not overstate. And the claim's phrase "USGS is not the
source of any shipped file" is wrong as written:
scripts/fetch-region-dem.mjs:23-24 names "USGS 3DEP/NED" among the underlying
sources of the shipped data/maps/regions/*.dem.bin files (a different USGS
program from Astrogeology). || Both sources are public domain and carry no
attribution obligation, and both already reach players in-app:
data/library/index.json:4-12 ships CREDITS.md as the Library's "Credits and
Thanks" doc, and data/library/CREDITS.md:73,:75 contain the New Horizons and
USGS lines. What is actually true is narrower: neither has a row in
data/credits.ron, so neither appears in the Settings > Credits panel
(src/gui/pages/settings.rs:3842, fed by src/credits.rs:44-57) -- the same
pre-existing, already-declared drift that also affects NOAA ETOPO, Solar
System Scope, Poly Haven and ambientCG. The claimed USGS wrinkle is wrong:
CREDITS.md:75-76 already reads "(reference + future bakes)", which exactly
matches the record (docs/reference/asset-and-map-sources.md:81-89 lists USGS
mosaics as candidates; scripts/build-planet-albedo.js:44-51 shows the shipped
Moon/Mars bakes came from Solar System Scope and the shipped Pluto bake from
assets.science.nasa.gov). No correction to CREDITS.md is needed. Adding a New
Horizons row to data/credits.ron (attribution_required: false) is optional
tidying only. || Both claimed sub-findings are wrong. First: the attribution
is NOT invisible to players. `data/library/CREDITS.md` is byte-identical to
root `CREDITS.md`, ships with the release
(`.github/workflows/build-desktop.yml:91`), and is the first document listed
in `data/library/index.json` under a "Credits" category, which
`src/gui/pages/library.rs:82-92` selects by default. Both the New Horizons and
USGS lines render in-app on the Library page. "Missing from data/credits.ron"
is true, but that file is the licence-OBLIGATION ledger (see its own header
comment and `src/gui/pages/settings.rs:3833-3841`); both sources are public
domain with no attribution obligation. Second: the USGS credit does not
overstate anything. `CREDITS.md:75-76` credits USGS Astrogeology for
"planetary mosaics (reference + future bakes)", it explicitly does not claim
a shipped file, and the reference role is real
(`docs/reference/asset-and-map-sources.md:82-89`). No correction to CREDITS.md
is warranted; the proposed edit would restate what the line already says. The
only accurate residue is an optional courtesy: a New Horizons row could be
added to `data/credits.ron` because `data/planets/pluto_albedo.bin` genuinely
derives from that mosaic (`scripts/build-planet-albedo.js:47-51`). That is a
nicety with no obligation behind it and no user-visible gap, since the credit
already ships and renders.

### 4. Quaternius (Nature Crops Pack), Kenney (Furniture Kit and four audio
packs), and OpenGameArt as the download mirror

Claimed: All CC0, so nothing is legally required, but CREDITS.md:83-85 makes
a promise this breaks: 'Additional CC0/CC-BY packs under evaluation are listed
in docs/reference/asset-and-map-sources.md; anything that ships gets credited
here.' Quaternius supplies ~100 crop models and Kenney supplies 15 furniture
models plus all 25 shipped sound effects, and neither is named in CREDITS.md
or data/credits.ron. Their only credit is a per-directory LICENSE file.
Mitigating: assets/models/, assets/audio/ and assets/textures/ are NOT in the
release archive, so this is a repo-level courtesy, not a distribution
question.

Refuted because: Quaternius (Nature Crops Pack) and Kenney (Furniture Kit +
four audio packs) are CC0 1.0, so no attribution is legally required, and
CREDITS.md:83-85 is NOT breached, because its trigger is "anything that ships"
and none of these assets ship. .github/workflows/build-desktop.yml:90-93
copies only data/, assets/icons/ and assets/shaders/; data/ holds no pack file
(only data/models/test_crate.glb, zero audio files, two galaxy-glow PNGs);
nothing is embedded via include_bytes!; and the engine states the consequence
outright in four places (src/engine/near_tree_models.rs:60,
src/renderer/tree_mesh.rs:9, src/renderer/billboard_bake.rs:515 and :4189), downloaded builds fall back to procedural geometry. The promise's other clause
is already met for the crops pack: docs/reference/asset-and-map-sources.md:8
names it and records that all 102 models "landed in assets/models/plants/ in
v0.991". OpenGameArt is a mirror, not a rightsholder
(LICENSE-cc0-model-packs.md: "mirror of the author's pack"), so it is owed
nothing. Both packs already carry authored credit with names, URLs and thanks
in assets/models/LICENSE-cc0-model-packs.md and assets/audio/LICENSE.md, and
Justfile:233 rsyncs those licence files into the public web root alongside the
assets. Adding Quaternius/Kenney rows to CREDITS.md would be a pleasant
courtesy, but it is not an attribution gap and no promise obliges it. Two
smaller true things worth separating out: the Kenney Furniture Kit and the
four audio packs are absent from docs/reference/asset-and-map-sources.md (a
doc-scope nit, that file covers plant models and planetary maps), and the
pre-existing credits.ron drift already declared in the brief, Poly Haven and
ambientCG appear in CREDITS.md with no credits.ron row, so the in-app Credits
page shows neither, is the only item here with any real user-visible
consequence. || Do not report this as an attribution gap. Corrected statement:
"Quaternius (Nature Crops Pack) and Kenney (Furniture Kit, four audio packs)
supply CC0 assets that live in the git repo at assets/models/ and
assets/audio/, but reach no user in any release artifact, build-desktop.yml:90-93 (desktop bundle) and :261-268 (data bundle) both copy
only data/, assets/icons/ and assets/shaders/, nothing is embedded in the exe
(src/embedded_data.rs is include_str! only), and no runtime downloader exists.
The engine's own comments document the consequence
(src/renderer/tree_mesh.rs:9, billboard_bake.rs:515,
near_tree_models.rs:59-60, data/vegetation/trees.ron:10), and the 25 Kenney
sounds silently fail to load in a release build because src/lib.rs:18817
resolves them under `assets/`. CC0 requires nothing, and the CREDITS.md:83-85
promise is conditional on shipping, so it is not breached. Its unconditional
half, that such packs be listed in docs/reference/asset-and-map-sources.md, is already satisfied (Quaternius at lines 8-9/15-19/25-27, Kenney at
21-24/44-45, OpenGameArt at 18/35-37), and further credit already exists in
assets/models/LICENSE-cc0-model-packs.md, assets/audio/LICENSE.md, each
converted glTF's `generator` field, and data/entities/decorations.ron:18." TWO
SMALL GROUNDED OBSERVATIONS, neither the claimed gap, offered only so they are
not lost: 1. FUTURE TRIGGER, not a present gap: the moment assets/models/ or
assets/audio/ is added to either archive step in build-desktop.yml,
CREDITS.md:83-85 fires and both packs would then need a line under "Asset
libraries." Worth a comment beside those cp lines rather than a credits edit
today. 2. Metadata inaccuracy: the Kenney furniture models carry the
Quaternius attribution string. assets/models/furniture/desk/desk.gltf opens
`"generator":"obj-to-plant-gltf.js (HumanityOS, Quaternius CC0 source)"`, the
converter hardcodes "Quaternius" regardless of pack, so the only per-file
credit on 15 Kenney models names the wrong author. Cosmetic and
CC0-irrelevant, but it is the one place an existing credit is factually wrong.
Not adjudicated here (out of scope, flagged because it surfaced as evidence):
a downloaded release appears to play none of the 25 sound effects, since
assets/audio/ is absent from the bundle and the failure is swallowed by a
warn-once. That is a possible gameplay bug for a separate look, not an
attribution matter.

### 5. data/constellations.geojson and data/constellations.json

Claimed: The one science/geometry data file whose origin is genuinely unknown
AND whose internal shape reads as an imported third-party dataset rather than
hand-authored data: a FeatureCollection named 'constellations.lines' with an
OGC CRS84 header and per-feature IAU abbreviation ids. constellations.json is
compiled into the binary and drives the constellation figures over the star
field. No origin or licence is recorded anywhere. This is distinguishable from
the rest of the unsourced tables below, which are numeric facts.

Refuted because: Corrected statement: data/constellations.json - the only
constellation file compiled into the binary (src/embedded_data.rs:43) and
rendered (src/renderer/stars.rs:2277, src/gui/pages/cosmos.rs:242) - is NOT an
unattributed third-party import. Its origin is recorded in this repo's git
history as in-project authorship: extracted from the project's own
game/index.html in 813e3fdc (2026-03-12) and re-authored to 594
machine-validated segments in 1f399b3a (v0.783.0, 2026-07-09, "complete
classical stick figures authored for all 86 constellations"), with per-entry
myth/season/keyStars prose no import would carry. No credits.ron row or
LICENSES.md entry is owed for it. The CRS84 FeatureCollection evidence belongs
to a different file, data/constellations.geojson, which is an ORPHAN: zero
references in any tracked file (src/, web/, scripts/, data/, docs/.github/ -
the only repo-wide hit, m.json, is untracked scratch), not embedded, and
structurally unrelated to the shipped file (RA/Dec coordinates, 89 features,
no star identities, vs 86 star-name entries). It does have no recorded origin
and it does ship, because .github/workflows/build-desktop.yml:91 and :262
bundle data/ wholesale. The appropriate action is to delete the unused file
(or add a one-line note saying it is unused and of unknown origin) - not to
add a data/credits.ron row for a dataset the product never reads. Its upstream
is unclear from what is in the repo; I checked git history, data/credits.ron,
CREDITS.md, LICENSES.md, docs/reference/asset-and-map-sources.md,
data/README.md, scripts/ and the Justfile, and the file itself carries no
metadata beyond its ogr-style name/crs header. || The finding as written is
wrong on its central factual premise and should not be reported. Corrected
statement: `data/constellations.json` (the file embedded at
src/embedded_data.rs:43 and parsed at src/renderer/stars.rs:2277) is NOT the
FeatureCollection quoted in the evidence. It contains zero coordinates and
zero GeoJSON structures; it is a hand-authored array of star-name pairs plus
myth/season/keyStars/objects/funFact prose, whose figures were authored
in-repo per commit 1f399b3a (v0.783.0): "complete classical stick figures
authored for all 86 constellations - 134 -> 594 segments. Every endpoint
machine-validated to resolve against the real star catalog". It predates the
geojson (created at d70b6cfb, v0.13.0, in the same shape). No attribution is
owed for it and it needs no credits.ron row. The FeatureCollection header
belongs to the separate `data/constellations.geojson`, added by d436315c
(v0.84.0) and referenced by NO code anywhere in the tree (the only tree-wide
grep hit is the untracked scratch file m.json at repo root). Its upstream is
unrecorded and its licence is UNCLEAR from repo contents; I checked
CREDITS.md, LICENSES.md, data/credits.ron,
docs/reference/asset-and-map-sources.md, the adding commit message, and
grepped the tree, and found no source named. Because it is dead weight that is
nonetheless shipped in every release archive
(.github/workflows/build-desktop.yml:91 and :266), the appropriate action is
to DELETE the unused file, not to add a credits row for it. Downgrade from
COURTESY finding to an optional housekeeping note. || The claimed gap does not
hold as written. Corrected statement: data/constellations.json, the file
embedded at src/embedded_data.rs:43 and parsed at src/renderer/stars.rs:2277
to draw the constellation figures, is NOT an imported third-party dataset and
has NO attribution gap. Its figures were authored in-house at commit 1f399b3a
(v0.783.0, "complete classical stick figures authored for all 86
constellations - 134 -> 594 segments"; the file has exactly 594 segments
today), building on content extracted from the operator's own index.html at
813e3fdc. Its schema (name/abbr/lines/myth/season/keyStars/objects/funFact,
star-name endpoints, prose) is authored educational content, not a GIS export.
No credits.ron row or LICENSES.md entry is warranted.
data/constellations.geojson is a separate file and a different question. It is
NOT embedded (grep -c geojson src/embedded_data.rs = 0) and has never been
referenced by any code in the repo's history (git log -S over src/ web/
scripts/ is empty; no consumer in src/, web/, scripts/, or data/). Its only
mention anywhere is the untracked, auto-generated m.json. It is nonetheless
copied into the release bundle wholesale by build-desktop.yml:91 and :266. Its
origin is unrecorded and its licence is UNCLEAR (nothing in CREDITS.md,
LICENSES.md, data/credits.ron, docs/reference/asset-and-map-sources.md, commit
d436315c, or the file's own header). The appropriate action is housekeeping, delete the orphan, or record its origin if kept, not an attribution row,
since it drives nothing and no shipped feature depends on it. The evidence
offered (FeatureCollection / CRS84 / IAU abbreviation ids) describes only the
geojson, and cannot be used to characterize the json.

### 6. BIP39 English wordlist (redistributed verbatim in two files)

Claimed: The canonical 2048-word list is re-emitted verbatim into
web/chat/bip39-english.js and src/net/bip39_wordlist.rs from the `bip39` Rust
crate's source. The files name their provenance but no terms for either the
crate or the list are recorded anywhere in the repo. A fixed standard artefact
rather than a creative dataset, so an obligation is unlikely, but this is a
third-party file redistributed verbatim in two shipped surfaces with
unrecorded terms. What I checked: scripts/gen-wordlist.js, both generated file
headers, LICENSES.md, CREDITS.md.

Refuted because: The BIP39 wordlist attribution is not a gap. The `bip39`
crate is licensed CC0-1.0 (bip39-2.2.2/Cargo.toml:33), pinned by
C:\Humanity\Cargo.toml:85 and C:\Humanity\Cargo.lock:589-592, and the CC0 1.0
Universal text is already in the repo at C:\Humanity\LICENSE -- byte-identical
to the crate's own LICENSE apart from one trailing blank line. CC0 requires no
attribution at all, which C:\Humanity\README.md:294 already states ("No
permission required, no attribution required"). Beyond that non-obligation,
the courtesy credit is already given twice: CREDITS.md "Technology" explicitly
covers "Every crate author in Cargo.toml", and both generated files
(web/chat/bip39-english.js:1-4, src/net/bip39_wordlist.rs:1-6) name the crate
AND the BIP39 / trezor python-mnemonic origin. The crate's own english.rs
carries no notice to preserve. No change is warranted; adding a
data/credits.ron row would misuse a file scoped to sources with real
obligations (src/credits.rs:106-124 enforces that attribution_required rows
name a live render surface). Unrelated and still true: the pre-existing drift
flagged in the prompt -- NOAA ETOPO, Solar System Scope, NASA New Horizons,
USGS, Poly Haven and ambientCG in CREDITS.md with no credits.ron row -- is
real and untouched by this finding. || The BIP39 English wordlist is NOT an
attribution gap of any severity. The redistribution path is the `bip39` Rust
crate v2.2.2, which is licensed CC0-1.0 (verified at the crate's
Cargo.toml:33, its full CC0 1.0 Universal LICENSE file, and the dedication
text at its src/lib.rs:4-7). CC0 is a public-domain dedication requiring no
attribution, no notice retention, and no licence text. The crate ships no
NOTICE/AUTHORS/COPYRIGHT file, and the specific file the generator reads
(src/language/english.rs) has no header at all, so there is no upstream notice
to carry forward. HumanityOS's own root LICENSE is also CC0 1.0 Universal, so
a CC0 file is being redistributed verbatim inside a CC0 project with zero
licence friction. The "Source: BIP39 / trezor python-mnemonic english.txt"
comment records ancestry only; scripts/gen-wordlist.js reads exclusively from
CARGO_HOME/registry/src/*/bip39-*/src/language/english.rs and never touches
bitcoin/bips or python-mnemonic. The absence from LICENSES.md and
data/credits.ron is correct scoping rather than drift, because LICENSES.md:1-6
and :65-68 explicitly limit themselves to third-party DATA "licensed
separately from the code that reads it", and this is a code dependency. The
courtesy dimension is already covered by CREDITS.md:87-91, which credits
"Every crate author in Cargo.toml". No action is required; at most, adding
"CC0-1.0" to the two generated header templates in scripts/gen-wordlist.js is
optional tidiness, and it should not be accompanied by a CREDITS.md Technology
line or a credits.ron row.

### 7. data/food/crop_nutrition.ron, a citation promise the file does not keep

Claimed: The file's header promises that for the nine crops with no USDA row,
'values come from the published composition literature named per line'. That
literature is not named per line. The most specific comment in the crops list
is 'finger_millet: Indian food-composition tables, no USDA row (FLAGGED)', no
table, edition or DOI. The file's own least-certain numbers are precisely the
ones whose promised citation is missing. No obligation (facts), but this is
the repo's best-practice exemplar failing its own stated standard.

Refuted because: Not an attribution gap. `data/food/crop_nutrition.ron` holds
only nutritional measurements (kcal and macros per 100 g), which are
uncopyrightable facts, and its dominant named source (USDA FoodData Central,
`:15`) is a US federal government work in the public domain that requires
nothing. No source in this file, USDA or otherwise, generates an attribution
obligation, so nothing here belongs in `data/credits.ron`, `CREDITS.md`, or
`LICENSES.md`. The only true residue is an internal wording inconsistency: the
header at `crop_nutrition.ron:47-50` says the nine non-USDA crops' values
"come from the published composition literature named per line", and no line
names a publication (`:103`, `:166`, `:190`, `:212` name none; `:185` names
only a category, "Indian food-composition tables"). The pointer at `:39` to
"the session report" also resolves to nothing in `docs/`, and the authoring
commit `f081e5e0` repeats the promise without naming a source either. That is
a data-file comment nit for whoever next edits the file (either name the
tables or soften the header to "unnamed published composition literature"),
not an audit finding and not an attribution or licence issue. || Not an
attribution gap; drop it from the attribution audit. The accurate residue, if
the operator ever wants it, is a one-line documentation nit and nothing more:
`data/food/crop_nutrition.ron:47-48` says the nine no-USDA-row values "come
from the published composition literature named per line," but no line names a
table, edition or DOI (`:103`, `:166`, `:190`, `:212` say only "no USDA row
(FLAGGED, see header)"; `:185` says "Indian food-composition tables" with no
edition). The honest fix is to SOFTEN the header, e.g. "no USDA row exists
for these; the values are estimates of published-literature magnitude and are
the least certain numbers in this file", not to invent per-line citations for
numbers that appear to be round estimates rather than transcribed table rows.
Two facts that keep it out of an attribution audit regardless of that wording:
(a) nutrition composition figures are uncopyrightable facts carrying no
attribution obligation (the claim concedes this), and (b) the table is dead
data at runtime, `CropNutrition::load`
(`src/systems/self_sufficiency.rs:189`) and both `food_supply_kcal_per_day`
call sites (`:266`, `:278`) sit inside the `#[cfg(test)]` module spanning
`:180-295`, with no production caller in `src/`.
`docs/design/gameplay-loop-map.md:95` states it directly: the scoring "is
built and tested but only tests consume it; the Home page still shows
hand-typed kcal strings."

### 8. data/glossary.json, 460 definitions, 323 carrying a Wikipedia link

Claimed: An unresolved unknown rather than a confirmed gap, recorded so it is
not forgotten. If any definition were copied or closely adapted from a
Wikipedia article rather than written fresh, CC BY-SA 4.0 attribution AND
share-alike would attach, and 323 bare 'Learn more' links do not discharge
that. All repo evidence points the other way, but the file carries no
provenance note and non-derivation cannot be proved by reading the repo alone, settling it means diffing definitions against article leads, which needs
network access.

Refuted because: The glossary carries no attribution obligation and no
unresolved provenance question that reading the repo cannot answer. Corrected
statement: data/glossary.json holds 460 house-written definitions, 323 of
which carry an optional outbound Wikipedia link rendered as further reading
("Learn more" at web/shared/glossary.js:192; "More: {url}" at
src/gui/widgets/mod.rs:553; not rendered at all in the Library Dictionary
view, which never reads .link). The links are a further-reading field
documented as such in the commits that introduced them (2052e863 "optional
learn-more link"; 6da1f8b9 "term + category + definition + Wikipedia link"),
and hyperlinks create no licensing obligation. The definitions are
house-written: link coverage is inverse to what copying predicts (platform
12/59 and crypto 22/54 linked versus space 63/63 and materials 40/40, with 137
entries unlinked entirely), many definitions state HumanityOS-specific
behavior found in no encyclopedia article, the generic entries follow one
uniform authoring template.claude/agents/lexicographer.md requires
definitions be verified against the codebase rather than sourced, and no
scraping script or Wikipedia instruction exists anywhere in scripts/ or
.claude/. No provenance note is needed. If the operator still wants one for
the record, it belongs as a line in CREDITS.md, not as a "header line" in
glossary.json, which is strict JSON with only "categories" and "terms" at the
top level and no comment syntax. The genuinely actionable item in this area is
the already-noted drift: NOAA ETOPO, Solar System Scope (CC-BY 4.0), NASA New
Horizons, USGS, Poly Haven and ambientCG appear in CREDITS.md with no row in
data/credits.ron and no LICENSES.md entry, contrary to the rule LICENSES.md
itself states. || No attribution obligation attaches to data/glossary.json,
and no action is needed. The 460 definitions are house-written: none of the
460 restates the term as its subject the way every Wikipedia lead does; 76 of
the 323 links point to an article about a different subject than the term
(Amperage -> Electric current, Tick -> Game loop), with two article URLs each
serving two distinct terms, which is impossible if the article were the
source; linked and unlinked entries are stylistically indistinguishable (29.6
vs 33.2 words, 2.2 vs 2.1 sentences); and 94 definitions, many of them linked
ones, describe HumanityOS behaviour that exists nowhere but this repo. The
repo's own native loader names the field "Optional external reference"
(src/gui/glossary.rs:26), matching the outbound "Learn more" in
web/shared/glossary.js:184-193. Separately, even if a definition had been
drafted from an article, CC BY-SA 4.0 protects expression, not facts, so
fact-level overlap in a two-sentence definition of a technical term creates no
attribution or share-alike duty. The claim's remaining true residue is
trivial: glossary.json carries no provenance note. That is not a gap, since
nothing obliges one. If the operator wants the question closed on the record
anyway, it is cosmetic; note that JSON takes no comments, so it would mean a
new top-level key, which the Rust loader tolerates (no deny_unknown_fields on
GlossaryFile, src/gui/glossary.rs:41-48). The genuinely actionable item in
this whole area remains the one already flagged in the prompt: NOAA ETOPO,
Solar System Scope (CC-BY 4.0, which does carry a real attribution duty), NASA
New Horizons, USGS, Poly Haven and ambientCG are in CREDITS.md but absent from
data/credits.ron, so the in-app Credits page does not show them. || No gap.
data/glossary.json ships 460 house-written definitions plus 323 outbound
reference URLs; it ships no Wikipedia text, image, or database content, so no
CC BY-SA obligation is triggered and there is nothing for credits.ron or
LICENSES.md to carry. The derivation question is not genuinely open either: 0
of 460 definitions use Wikipedia's lead-sentence form, 159 of 323 links point
at a differently-titled article, 11 articles are cited by multiple terms with
distinct definitions (Entity_component_system by four), 137 terms carry no
link in the same house voice, and no code anywhere in scripts/src/web fetches
Wikipedia. The proposed provenance header line in data/glossary.json is
optional tidiness at most, not an attribution remedy - and note the repo ships
CC0 1.0 with "no attribution required" (README.md:38, :294), so a provenance
note there would be documenting authorship, not discharging a licence. The one
real (unrelated) defect surfaced: .claude/agents/lexicographer.md:61 documents
the entry shape as {term, category, definition}, omitting the "link" field
that 323 shipped entries actually carry.

## Completeness critic

## 1. Places third-party content can hide that no angle searched

**`web/shared/qrcode.js` (a whole vendored library nobody listed).** The "vendored web code" angle produced `noble-pq.bundle.js` and `twemoji.min.js` and stopped. It missed the third vendored file. `web/shared/qrcode.js:1-16` reads:

```
// QR Code Generator for JavaScript
// Copyright (c) 2009 Kazuhiko Arase
// Licensed under the MIT license:
// The word 'QR Code' is registered trademark of DENSO WAVE INCORPORATED
```

It appears in no credits surface at all: not `CREDITS.md`, not `LICENSES.md`, not `data/credits.ron`. Worse, the repo makes a false statement about it. `web/chat/index.html:713` says `<!-- qrcode-generator (public domain, ~18 KB) -->` immediately above the `<script src="/shared/qrcode.js?v=4">` tag. That is the project publishing an incorrect licence claim about someone else's MIT-licensed work, in its own tree, which is the exact defect class the audit flagged against `data/external/catalog.json` (gap 13) without noticing the same thing one directory over. The in-file header does survive verbatim, so MIT's notice-retention is arguably met for the served file itself; the defect is the false characterisation plus the registry blind spot.

**`assets/icons/platforms/` (five verbatim third-party brand SVGs, shipped in every release).** `assets/icons/platforms/{steam,xbox,playstation,epic,gog}.svg` all carry the Simple Icons markup signature: `<svg fill="#ffffff" role="img" viewBox="0 0 24 24" xmlns="..."><title>Steam</title><path d="...">`. Compare the hand-drawn in-house icons in the parent directory, for instance `assets/icons/steam.svg`, which opens `viewBox="0 0 48 48" fill="none" stroke="currentColor"` with a `<!-- Steam piston/joystick logo -->` comment, or `assets/icons/dev.svg` and `assets/icons/penguin.svg`, which are polyline sketches. The five in `platforms/` are a different provenance from everything around them and nothing in the repo records where they came from. They are displayed publicly at `web/pages/download.html:291, 304, 317, 330, 343`, and `assets/icons/` is copied wholesale into both release artifacts (`.github/workflows/build-desktop.yml:92` `cp -r assets/icons/ dist/HumanityOS/assets/icons/`, and `:266` in the data bundle). `assets/icons/discord.png` is the same problem in raster form. The whole `assets/icons/` tree of 170 files entered in one restructure commit (`git log --diff-filter=A` gives `d70b6cfb`, v0.13.0) with no provenance note anywhere, and `docs/reference/asset-and-map-sources.md` contains no occurrence of the word "icon".

**`package-lock.json`, a tracked root file, and the entire npm side.** The dependency angle swept Cargo and `data/external/catalog.json` and never touched npm. `package-lock.json` is committed and records licences: 10 packages are `LGPL-3.0-or-later` (`@img/sharp-libvips-*`), 3 more are `Apache-2.0 AND LGPL-3.0-or-later` (`@img/sharp-win32-*`), pulled in by the single `sharp` devDependency in `package.json`. This is the only copyleft in the entire project. It is dev-time only, `node_modules/` is not redistributed, and using an LGPL image library to bake a PNG does not make the PNG LGPL, so I do not think there is an obligation here. But "no angle looked at npm at all" is the finding: nobody can say that from the audit as it stands, and the one place copyleft actually appears in this repo went unexamined.

**The GitHub release assets the app downloads at runtime.** `src/renderer/stars.rs:1623, 1626, 839` point the in-app downloader at `https://github.com/Shaostoul/Humanity/releases/download/assets-stars-1/stars-athyg.bin`, `.../stars-gaia25m.bin`, and `.../assets-glow-1/galaxy_glow_ultra.png`. Every refutation in the adversarial pass leaned on "neither catalogue is even shipped" to dismiss the ATHYG and Gaia questions. That reasoning is backwards. These files are not fetched from astronexus or ESA; they are built by `scripts/build-athyg-bin.js` and `scripts/build-gaia-bin.js` and **published by this project, from this project's own release page**, which is redistribution of a derived database in exactly the sense `LICENSES.md:31-35` describes for the OSM regions. A GitHub release asset carries no accompanying licence file, so whatever ATHYG and Gaia oblige, nothing travels with those three files. No angle audited the release-asset channel as a distribution surface; it is treated throughout as if it were someone else's server.

**`assets/shaders/`, which ships and contains unattributed published techniques.** `assets/shaders/` is in both bundles (`build-desktop.yml:93`, `:267`). `assets/shaders/pbr/30-atmosphere.wgsl:401-408` and `assets/shaders/pbr/40-clouds.wgsl:1551-1558` and `:1689-1696` hardcode `aces_a = 2.51; aces_b = 0.03; aces_c = 2.43; aces_d = 0.59; aces_e = 0.14`. Those five constants are a specific published curve fit, not a formula the project derived, and the repo names no source for them. `assets/shaders/cloud_resolve.wgsl:80` says "variance-clip bounding box (Karis-style)", naming a person in a comment and nowhere else. I am not claiming a copyright obligation on a five-constant rational fit; I am saying the shader tree is a place where third-party technique arrives with no record, and the "assets and fonts" angle evidently scanned it for binaries rather than for provenance.

**`CONTRIBUTING.md`, the inbound side.** `CONTRIBUTING.md` is 244 lines, tells people to fork and submit a PR (`:226-231`), and contains zero occurrences of "licen", "copyright", "CC0", "public domain", "waive", or "assign". There is no DCO, no CLA, no inbound CC0 grant. `CREDITS.md` nevertheless has a "Contributors" section describing "People who have done work on HumanityOS", some of whom "asked not to be named at all. Their work is in here anyway." The whole audit examined outbound obligations and never asked whether the project actually holds the rights it purports to dedicate. That is the other half of the CC0 question and it is completely unexamined.

**Two smaller ones, recorded so they are not re-hunted:** `data/library/us-constitution.md` is a shipped third-party document (public domain, no obligation, transcription source unrecorded), and `wallpapers/` plus `_pu-archive/` are untracked, so they do not ship.

## 2. Obligation classes nobody checked

**Patents.** This is the sharpest omission after the CC0 question, and it is not hypothetical here. `Cargo.toml:18` puts `dep:unsafe-libopus` and `dep:nnnoiseless` in the `native` feature. The licence text that `unsafe-libopus-0.2.0/LICENSE` requires be reproduced does not end at the BSD disclaimer; it continues:

> Opus is subject to the royalty-free patent licenses which are specified at: Xiph.Org Foundation ... Microsoft Corporation ... Broadcom Corporation ...

with three IETF IPR declaration URLs. That paragraph is part of the notice, and shipping the codec with no licence text means a downstream recipient is not told that a patent grant exists or where to find it. `nnnoiseless-0.5.2/COPYING` similarly carries five copyright lines (Joe Neeman, Mozilla, Jean-Marc Valin, Xiph.Org, Mark Borgerding) that the audit's generic "MIT, BSD, ISC, Zlib, Apache" sweep never named. Separately, the patent question runs the other way too: the ~380 Apache-2.0 or dual-Apache crates in the graph carry an express patent grant under Apache-2.0 section 3, and CC0 section 4(a) states that no patent or trademark rights are waived by the dedication. So a fork acting on `README.md:294` ("No permission required") inherits the project's copyright waiver and no patent peace whatsoever, in a codebase that ships ML-KEM, ML-DSA and Opus.

**Trademark.** Nobody checked it and the repo is full of it: the DENSO WAVE "QR Code" mark called out in `web/shared/qrcode.js:11-14`, the five platform marks in `assets/icons/platforms/`, `assets/icons/discord.png`, and the ~40 software vendors named in `data/external/catalog.json`. Nominative use of a brand name to say "this runs on Steam" is ordinary and almost certainly fine. The problem is not the use; it is that `LICENSE` is a bare CC0 dedication covering the whole tree and `README.md:294` tells recipients "No permission required" over content that includes other companies' marks, which CC0 section 4(a) explicitly does not and cannot waive.

**NOTICE files (Apache-2.0 section 4(d)).** I checked this and the answer is a useful negative: across the resolvable dependency source cache there is exactly one, `cfg_aliases-0.2.1/NOTICES.md`. Apache section 4(d) is therefore a near-empty obligation here. Record it so the next auditor does not spend a pass on it.

**Copyleft compatibility, the check that should have run first.** `Cargo.lock` holds 816 `[[package]]` entries; 609 resolve in the local registry cache. Their declared licences are: 297 `MIT OR Apache-2.0`, 83 `MIT`, 59 `Apache-2.0 OR MIT`, 33 `MIT/Apache-2.0`, 23 `Apache-2.0`, 18 `Unicode-3.0`, 11 `BSD-3-Clause`, 11 `MPL-2.0`, 9 `ISC`, 5 `BSD-2-Clause`, 5 `CC0-1.0`, 2 `BSL-1.0`, 2 `CDLA-Permissive-2.0`, and assorted Zlib/Unlicense/0BSD/BlueOak disjunctions. **There is no GPL, LGPL, AGPL, CDDL, EPL, SSPL or BUSL anywhere in the Rust graph.** The strongest term is MPL-2.0 (the symphonia family plus `triple_buffer`, gap 3), which is file-level copyleft and does not reach into the project's own sources. That negative result is the actual answer to "is this project's licensing coherent at the code layer", and no angle produced it. Note also that `epaint_default_fonts 0.31.1` declares `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`, which is the machine-readable confirmation of gap 1, and that `(MIT OR Apache-2.0) AND Unicode-3.0` on `unicode-ident` plus the 18-crate ICU4X family makes Unicode-3.0 the second-largest AND-joined obligation after the fonts.

**Share-alike propagation, which was raised for the wrong file.** See section 3.

## 3. Claims in the surviving list that are stated too strongly

**Gap 5(d) points the share-alike consequence at the wrong upstream, and in doing so buries the finding that matters.** It says: "If CC BY-SA is correct, share-alike attaches to `data/galaxy_glow.png`, a committed derived work that integrates the catalogue" (the catalogue being HYG/ATHYG). That is wrong on the evidence three separate refuters already established, and I verified the deciding fact independently: `data/galaxy_glow.png` is 8192x4096 (PNG IHDR), a size only the Gaia DR3 census path in `scripts/build-galaxy-glow.js` produces, the ATHYG path being fixed at 2048x1024. So the glow is a **Gaia** derivative, and `CREDITS.md:68` characterises Gaia as CC-BY-SA-IGO 3.0. The share-alike question is real and live, it just belongs to Gaia, and it extends to a second file nobody mentioned: `galaxy_glow_ultra.png`, a 16384x8192 bake (`src/renderer/stars.rs:837-843`) that this project publishes as a release asset. Aiming that clause at HYG let the Gaia share-alike question die with the refuted Gaia finding.

**Gap 2 overstates the reach of "all condition redistribution".** The sentence "MIT, BSD-2/3-Clause, ISC, Zlib and Apache-2.0 section 4(a) all condition redistribution, binary redistribution included, on reproducing the copyright notice and licence text" is wrong at two edges. The Zlib licence's notice clause is scoped to source distributions and its altered-version clause to source; it does not impose a binary-notice duty the way MIT and BSD do. And `BSL-1.0` (`clipboard-win 5.4.1`, `error-code 3.3.2`, and the alternate arm of `ryu`) expressly exempts "machine-executable object code generated by a source language processor" from its notice requirement. The MIT/BSD/ISC/Apache core of the finding is sound and I would not soften it; the universal quantifier is not. The "556 crates in the desktop graph" figure is also unsourced against a lock file with 816 packages.

**Gap 8 asserts more than a base-URL constant can prove.** "Every emoji rendered anywhere in the web chat is an `<img>` pointing at the upstream asset set" rests on `twemoji.min.js`'s `base:"https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.3/assets/"` plus the one hardcoded badge at `web/chat/app.js:1788`. That establishes the default source for parsed emoji, not that every emoji on every surface is parsed. The durable, checkable version of the finding is narrower and stronger: the site fetches emoji artwork from a third-party CDN at runtime and records no licence for it. Which also means gap 12 is under-scoped, since it presents CARTO as the one live third-party network dependency the registry does not know about, when `cdn.jsdelivr.net` is a second.

**Gap 4(3) is understated rather than overstated, and I verified it.** The HUD guard at `src/lib.rs:18381-18384` is `active_page == GuiPage::None && show_hud && !showroom_active && !construction_active`. So the OSM notice at `src/gui/pages/hud.rs:451` is suppressed in the showroom and the construction editor as claimed, and additionally is conditioned on `gui_state.show_hud` (`src/gui/mod.rs:2587`, defaulted true at `:4993`). That flag currently has no toggle anywhere in `src/`, so it is not a live hole today, but the obligation is one boolean away from being user-dismissible.

**Gap 11 is accurate as written.** `src/credits.rs:106-140` confirms it: `every_required_attribution_names_a_surface` asserts only `!s.shown_in.is_empty()` and `!s.notice.trim().is_empty()`, and `osm_names_both_drawing_surfaces` only checks that the joined free text contains "maps" and "in-world". Nothing opens a renderer file.

## 4. Is the project's CC0 claim compatible with every incoming source

No. It is incompatible in at least three distinct ways, and the incompatibility is not confined to a repo file that a lawyer might read charitably. It is stated to users, in the app, and in the bundle they download.

**Where the unqualified claim lives.** The root `LICENSE` is the bare CC0 1.0 Universal text with no scope statement. `README.md:38` is careful and correct ("The code, the docs and the commits we wrote are in the public domain"), but nothing else is: `README.md:5` and `:294` ("No permission required, no attribution required. This belongs to everyone"), `web/shared/shell.js:1397` and `:1973` (a site-wide footer, "HumanityOS, Public domain, CC0 1.0", injected on every page including `web/pages/maps.html`, which renders ODbL data), `web/pages/download.html:485` and `:494` ("Everything is public domain (CC0)" and "No permission needed"), `src/gui/pages/humanity.rs:311, 334` in the native app ("Public domain, forever", "The whole system is public domain"), and, most consequentially, two documents that ship inside the bundle: `data/library/ONBOARDING.md:24` ("Public domain (CC0), no permission required to use, fork, or deploy") and `data/library/SELF-HOSTING.md:615` ("Public domain. No permission needed. Run your own server and join the federation.").

**Collision one, ODbL, and it is worse than gap 4 states.** ODbL is share-alike over the Derivative Database, and CC0 purports to waive sui generis database rights the project does not hold in the OSM extraction. `LICENSES.md:31-35` says plainly that a redistributor of a bundle containing `data/maps/regions/` carries the ODbL obligation forward. I confirmed the bundle's contents from `build-desktop.yml:88-93`: exe, the two DXC DLLs, `data/`, `assets/icons/`, `assets/shaders/`. `git ls-files data | grep -i licen` returns nothing, and `data/library/` contains `CREDITS.md` but no `LICENSES.md`. So the release archive contains the four ODbL region files, contains no ODbL offer, and contains `SELF-HOSTING.md` telling the reader that no permission is needed to redistribute it. That is not merely an obligation that failed to travel; it is a contrary instruction that did travel, to the exact person the obligation lands on.

**Collision two, Gaia's CC-BY-SA-IGO 3.0.** Per `CREDITS.md:68`, Gaia is share-alike. `data/galaxy_glow.png` (8192x4096, Gaia census bake) is git-tracked and ships inside `data/`. `galaxy_glow_ultra.png` (16384x8192) is published by this project as a release asset (`src/renderer/stars.rs:837-839`). Both are distributed under the blanket CC0 claim. If `CREDITS.md:68` is right about the licence, the CC0 claim over those two files is void and share-alike attaches. If `data/credits.ron`'s different answer is right ("ESA Gaia data, freely available with acknowledgement", no licence named), the tension may dissolve. The project currently gives three different answers about Gaia across `data/credits.ron`, `CREDITS.md:68` and `LICENSES.md:53-63` (which names no licence at all), so from repo evidence alone the Gaia licence is **unclear**, and it is unclear on precisely the axis that decides whether the CC0 claim holds. I checked those three files plus `docs/reference/asset-and-map-sources.md`; I did not consult ESA.

**Collision three, the incoming permissive stack.** CC0 at the root claims the whole tree, and the tree contains `web/shared/qrcode.js` (MIT, Arase, notice must be retained), `web/chat/twemoji.min.js` (MIT), `web/shared/vendor/noble-pq.bundle.js` (upstream notice deliberately stripped by `--legal-comments=none` in `scripts/build-noble-bundle.mjs`), five third-party brand SVGs, and `dxcompiler.dll` / `dxil.dll` sitting beside the exe in the archive. A fork that acts on "No permission required, no attribution required" and strips those headers is doing exactly what the project told it to do and violating four upstream licences in the process.

**The honest formulation** is the one `README.md:38` already reaches for and every other surface abandons: the project dedicates to the public domain only what its contributors wrote, and it redistributes third-party works, some share-alike, that it cannot and does not dedicate. The concrete moves that follow are a scope paragraph at the top of `LICENSE` itself, the same qualifier on the four unqualified user-facing surfaces (`shell.js` footer, `download.html`, `humanity.rs`, and the two Library docs that ship), and `LICENSES.md` copied into `data/library/` with an `index.json` entry so the offer travels in the archive that carries the obligation. And, unresolved from the other direction, `CONTRIBUTING.md` needs an inbound grant before the CC0 claim is backed for any code the operator did not personally write.
