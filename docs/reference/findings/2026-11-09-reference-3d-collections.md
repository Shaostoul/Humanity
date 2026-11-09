# NASA 3D Resources, Khronos glTF Sample Assets and Smithsonian Open Access: may the project ship their models

**Research date: 9 November 2026** (Pacific time, early morning). Read that day by an AI research
agent working for the project, with the user agent `HumanityOS-research/1.0 (AI research agent
reading licence pages; https://united-humanity.us)`, one request at a time, after reading each
site's `robots.txt`. Repository files were read through GitHub's API and raw file pages. **No model
was downloaded.** These three are lower-value sources for the project's gaps (space-hardware
reference, test scenes and museum props), so this is one finding for the group, shorter than the
others of the day.

**I am not a lawyer, and this is not legal advice.**

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(sections 4.5, 4.6, 4.12 and 7.3). Index of this day's asset findings:
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md).

## The question

May HumanityOS convert and ship models from NASA's 3D Resources, Khronos's glTF Sample Assets and the
Smithsonian's Open Access collection, and on what conditions?

## Short answer

- **NASA 3D Resources: ALLOWED for NASA's own models, with conditions; the "NOSA 1.3" entry looks
  like a catalogue default, not the governing terms.** NASA's guidelines (updated 27 October 2026)
  say media "used in the rendition of 3-dimensional models, such as texture maps and polygon data in
  any format – generally are not subject to copyright in the United States", ask that "NASA should be
  acknowledged as the source", and reserve the insignia. The repository's README says "These assets
  are free and without copyright." Its `meta.json` says "NASA Open Source Agreement Version 1.3", a
  software agreement with blanks that the repository never fills in or includes. **UNCLEAR:** which
  models came from the non-NASA contributors the README lists; the folders carry no per-model credit
  (N1 to N4).
- **Khronos glTF Sample Assets: per model.** Of 150 models, 62 are CC0 and 45 CC BY 4.0 alone; most
  of the rest mix CC0 or CC BY with logo entries that grant no rights in the logos; **9 are NOT
  usable** (non-commercial, CRYENGINE, Poser, Adobe Stock, Stanford, SCEA and a testing-only licence)
  (K1 to K3).
- **Smithsonian Open Access: CC0 per record, terms page unread.** The Smithsonian's registry entry
  and GitHub repository say CC0; its website (FAQ and terms of use) answered with a robot check, so
  its terms were not read (S1).
- **Recommendation:** NASA's models for space-hardware reference and, where clearly NASA-made, for
  shipping with "NASA" credit and no insignia; Khronos's CC0 and CC BY models only, logos removed;
  Smithsonian later, after a person reads its terms of use.

## NASA 3D Resources

### N1. NASA's usage guidelines

https://www.nasa.gov/nasa-brand-center/images-and-media/ ("Page Last Updated: Oct 27, 2026";
`robots.txt` allows all):

> The NASA Insignia, Logotype, identifiers, and imagery are not in the public domain. The use of the Insignia, Logotype and NASA identifiers is protected by law, and imagery is made available for use consistent with Media Usage Guidelines.

> NASA content – images, audio, video, and media files used in the rendition of 3-dimensional models, such as texture maps and polygon data in any format – generally are not subject to copyright in the United States. You may use this material for educational or informational purposes, including photo collections, textbooks, public exhibits, computer graphical simulations and Internet Web pages.

> NASA should be acknowledged as the source of the material. NASA occasionally uses copyright-protected material of third parties with permission on its website. Those images will be marked identified as copyright protected with the name of the copyright holder. NASA’s use does not convey any rights to others to use the same material.

Also: material used commercially "must not explicitly or implicitly convey NASA’s endorsement of
commercial goods or services"; identifiable people may have rights of privacy or publicity; and a
new section headed "Super Intelligence (SI) Applications", citing "the Sept. 29, 2026, Executive
Order 14434", says "NASA does not permit our Insignia (i.e., NASA Meatball, Worm logotype, or Seal)
to appear with SI generated imagery or in the training of SI tools." The NASA 3D hub,
https://science.nasa.gov/3d-resources/ (where nasa3d.arc.nasa.gov now redirects): "All of these
assets are free to download and use. They are also mirrored on GitHub. Refer to the NASA Images and
Media Usage Guidelines."

### N2. The GitHub repository and its NOSA entry

https://github.com/nasa/NASA-3D-Resources (last push 2025-06-03; GitHub reports no licence). The
root holds only `.gitconfig`, `.gitignore`, `3D Models`, `3D Printing`, `Images and Textures`,
`README.md` and `meta.json`: **no licence file**. `README.md`:

> These assets are free and without copyright.

and points to the usage guidelines (N1). `meta.json`:

> `"license": "NASA Open Source Agreement Version 1.3",`

NOSA 1.3 (https://opensource.org/license/NASA-1.3, read 2026-11-09) "DEFINES THE RIGHTS OF USE,
REPRODUCTION, DISTRIBUTION, MODIFICATION AND REDISTRIBUTION OF CERTAIN COMPUTER SOFTWARE ORIGINALLY
RELEASED BY THE UNITED STATES GOVERNMENT"; it has blanks for the agency, the software's designation
and title, and requires that "a copy of this Agreement must be included with each copy of the
Subject Software". The repository includes no filled-in agreement. **Where the sources disagree:**
the README and guidelines (written for these assets) against a catalogue field naming a software
agreement that is never instantiated. I find the README and guidelines more credible for the models
(a judgement; unknown N-a would settle it).

### N3. Who made each model

The README thanks contributors including NASA centres and several who are not NASA: "Ball
Aerospace", "DigitalSpace Corporation" and individuals with no organisation listed (for example
"Conti, Carlo"). A model folder holds only the model and a preview: `3D Models/Orbital Replacement
Unit/` has `Orbital Replacement Unit.glb` and `.png`; the ISS interior folder has eight `.7z` parts
and a `.png`. **No per-model author or licence is recorded**, so which models came from non-NASA
contributors cannot be told from the repository.

### N4. What is still unknown for NASA

- **N-a. Whether NASA means NOSA 1.3 to apply to the models.** Settled by asking the repository's
  listed contact, `arc-special-proj@lists.nasa.gov` (named in `meta.json` and the README); free, one
  message, the operator's call.
- **N-b. Who made each model we would pick.** Settled by the same contact, or by choosing models
  whose NASA origin is documented elsewhere (a NASA mission page crediting a NASA centre).
- **N-c. Insignia in textures.** Station and spacecraft models may carry the NASA "meatball" or flags
  on their textures; settled by looking at each chosen texture and painting marks out.
- **N-d. Status outside the United States.** The guidelines speak of copyright "in the United
  States"; not researched further.

## Khronos glTF Sample Assets

### K1. The repository and its per-model legal metadata

https://github.com/KhronosGroup/glTF-Sample-Assets (last push 2026-11-05). `README.md`: "A summary
of the model license is shown in each display, but see the `README.md` in each model's directory for
detailed license information." Its footer: "&copy; 2026, The Khronos Group. Licensed as CC-BY-4.0
International". It also records that "Assets without licenses or licenses that were not suitable for
use in this collection were removed".

Each model has a `metadata.json` with a `legal` list. Two examples (read 2026-11-09):

- `Models/WaterBottle/metadata.json`: `"license": "CC0-1.0"`, `"artist": "Microsoft"`, `"year":
  "2017"`, `"owner": "Public"`, `"what": "Everything"`.
- `Models/CommercialRefrigerator/metadata.json`: two entries, both `"license": "CC-BY-4.0"`: one
  `"artist": "Eric Chadwick"`, `"owner": "Darmstadt Graphics Group GmbH"`, `"what": "Model and
  textures and all images in the README.md"`; the other `"artist": "Sean Thomas"`, with:
  > `"what": "This work is based on 'Commercial Fridge' (https://sketchfab.com/3d-models/commercial-fridge-2174e1e4f1f24f1a95aa110ee060f473) by Sean Thomas (https://sketchfab.com/foon.) licensed under CC-BY-4.0 (http://creativecommons.org/licenses/by/4.0/)",`

  Its README describes "champagne bottles" and says "The printed label in the top right interior was
  re-added on top of the lightmap, and the upper backlit signage was also composited on top": labels
  and signage to check for brands before use.

### K2. Every model's licence, from REUSE.toml

https://github.com/KhronosGroup/glTF-Sample-Assets/blob/main/REUSE.toml (read 2026-11-09) gives an
SPDX licence per path; a default `path = "**"` is "CC-BY-4.0" with "2017-2026 Khronos Group". Tallied
by model folder (150 models):

| Licence expression | Models |
|---|---:|
| CC0-1.0 | 62 |
| CC-BY-4.0 | 45 |
| CC0-1.0 with a Khronos, UX3D or similar logo entry | 18 |
| CC-BY-4.0 with a Khronos, Cesium or DGG logo entry | 12 |
| CC-BY-4.0 and CC0-1.0 (parts) | 4 |
| Not usable (below) | 9 |

**Not usable:** `BrainStem` (LicenseRef-Poser-EULA), `DamagedHelmet` (CC-BY-4.0 AND CC-BY-NC-4.0),
`DragonAttenuation` and `DragonDispersion` (LicenseRef-Stanford-Graphics), `Duck` (SCEA),
`EnvironmentTest` (LicenseRef-Adobe-Stock), `PlaysetLightTest` (CC-BY-NC-SA-4.0), `Sponza`
(LicenseRef-CRYENGINE-Agreement), `VirtualCity` (LicenseRef-3DRT-Testing). The `LICENSES/` folder
holds the text of each.

### K3. The logo entries

`LICENSES/LicenseRef-LegalMark-Khronos.txt`:

> Use in this model do not change the license of the model. The model
> license does not confer any usage rights on the Mark beyond that
> specified in the Guidelines. In particular, there is no right to
> use the Mark in any other context based on the usage here.

(line breaks as in the file, which also ends each of these lines with a space; "do not" sic.) So a
model with a logo entry is usable under its model licence with the logo removed.

## Smithsonian Open Access

### S1. What could be read

- https://www.si.edu/termsofuse answered "Smithsonian request verification" and "Enable JavaScript
  and cookies to continue" (HTTP 403) on 2026-11-09; https://www.si.edu/openaccess/faq refused the
  scan the same way on 2026-11-08 and was not retried. Not worked around.
- The Smithsonian's entry in the AWS Open Data Registry,
  https://github.com/awslabs/open-data-registry/blob/main/datasets/smithsonian-open-access.yaml (last
  changed 2020-02-28; `ManagedBy` "SI"): "On February 25th, 2020, the Smithsonian released over 2.8
  million CC0 interdisciplinary 2-D and 3-D images" and `License: CC0`.
- https://github.com/Smithsonian/OpenAccess (GitHub licence CC0-1.0; last push 2021-12-21): "We have
  transitioned away from GitHub for our compressed archive" and points to the AWS bucket.
- The 2026-11-08 scan found, through the public API, that each record's media carries an access
  value (`"access":"CC0"`) and that 1,937 records carry 3D models; not re-read today.
- **Gaps filled: none** (the scan found heritage objects, not appliances). Use, if ever: CC0 per
  record, checked record by record, with no Smithsonian marks; a person reads the terms of use first.

## Sources I could not reach

- https://www.si.edu/openaccess/faq, https://www.si.edu/termsofuse: robot check (S1). Wanted: the
  Smithsonian's own wording on CC0 and any conditions on its marks.
- The text of 17 U.S.C. 105 (works of the US government) at uscode.house.gov: two query forms
  returned 404; NASA's guideline wording was used instead.

## What it means for this project

### Certain

- NASA's guidelines describe NASA's 3D model data as generally free of US copyright, ask for NASA to
  be acknowledged, and reserve the insignia, logotype and identifiers (N1).
- The NASA repository names NOSA 1.3 in a metadata field but includes no NOSA agreement, and records
  no per-model authorship (N2, N3).
- Khronos's sample models carry per-model licences; 9 of 150 are not usable; logo entries grant no
  rights in the logos (K2, K3).
- The Smithsonian's own registry entry and repository say CC0; its terms page was not read (S1).

### Judgement calls

- **NASA: reference first, ship with care.** The ISS interior is the best open reference for real
  life-support racks; study it to model our own. Ship a NASA model only where its NASA origin is
  clear (or after unknown N-b is answered), credit "NASA", keep every insignia and logotype out of
  textures and screens, and never word anything as NASA endorsement. Asking NASA about NOSA (N-a) is
  cheap and would close the question.
- **Khronos: fine for what fits, which is little.** The commercial refrigerator suits a food hall,
  after its label and signage are checked; the rest are test scenes, useful to test the renderer.
  Copy each model's `legal` entries into its credit unchanged.
- **Smithsonian: later.** No gap item there; heritage props only.

### How the project would credit them

- **NASA**: "Model: NASA, from NASA 3D Resources (https://science.nasa.gov/3d-resources/), modified
  by HumanityOS" in the model's folder and `CREDITS.md`; a `credits.ron` row `nasa_3d`, `licence:
  "Public domain in the US (NASA media guidelines)"`, `attribution_required: false` (the guidelines
  ask, they do not condition), `shown_in: ["Credits"]`. The existing `nasa_gibs` row is the
  pattern.
- **Khronos**: per model, from its `metadata.json` `legal` list (for the refrigerator, both Eric
  Chadwick / Darmstadt Graphics Group GmbH and Sean Thomas, CC BY 4.0, with the Sketchfab link), plus
  a "modified" note; a `credits.ron` row only if a CC BY model ships.
- **Smithsonian**: per record, if ever: object name, unit, record link, "CC0".

### The question for the operator

**Should an agent use NASA's ISS interior and station models as reference for modelling our own
life-support racks, and may the project ship NASA-made models credited to NASA with insignia
removed; and would you like a short question sent to NASA's 3D Resources contact about the NOSA 1.3
entry?**

Recommendation: **yes to reference use now; yes to shipping clearly NASA-made models on those
conditions; and yes, send the question** (one message settles N-a and N-b together). Khronos models
as needed, CC0 and CC BY only; Smithsonian deferred.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
