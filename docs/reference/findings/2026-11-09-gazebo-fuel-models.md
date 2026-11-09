# Gazebo Fuel models: how each states its licence, what CC BY models owe, and what the Fuel service itself asks

**Research date: 9 November 2026** (Pacific time, early morning). Every source below was read that
day by an AI research agent working for the project, with the user agent
`HumanityOS-research/1.0 (AI research agent reading licence pages; https://united-humanity.us)`,
one request at a time with a pause between, after reading each host's `robots.txt` (gazebosim.org
and app.gazebosim.org allow everything; fuel.gazebosim.org has none, a 404). Only JSON metadata and
each model's small `model.config` text file were read: no mesh, texture or archive was downloaded.

**I am not a lawyer, and this is not legal advice.**

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(section 4.15), which found Open Robotics' oven, pump, shower, sink, electrical box and valves on
Fuel. Index of this day's asset findings:
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md). The CC BY 4.0
reading itself is the one in
[`2026-11-08-nvidia-simready-assets.md`](2026-11-08-nvidia-simready-assets.md), F2, re-checked
here against the same legal code.

## The question

May HumanityOS convert and ship Gazebo Fuel models (Open Robotics' first), where does each model
state its licence, what credit does a CC BY model need, and does the Fuel service add terms of its
own?

## Short answer

- **Each model's licence is a field in Fuel's database, not a file in the model.** The API returns
  `license_name` and `license_url` per model; the downloaded files (`model.config`, `model.sdf`,
  meshes, textures, thumbnails) carry no licence text (F1, F2).
- **Seven licences are possible on Fuel**, from CC0 to CC BY-NC-ND (F1). **ALLOWED: CC0 and CC BY
  4.0 models.** CC BY-SA is usable only if our changed version stays CC BY-SA; **NOT usable:** the
  NoDerivatives and NonCommercial ones (we change models, and the project cannot carry NC).
- **The model files carry the CREDIT, and sometimes a chain of it.** `model.config` names the
  author; Open Robotics' `Oven` names two people and states its geometry came from a Sketchfab model
  by Francesco Coldesina under CC BY 4.0. Both must be credited (F2, F4).
- **CC BY models need per-model credit**: creator(s) as named, licence with link, a link to the
  model, and a note of our changes (F4).
- **No terms of service for Fuel were found.** The service's published policies are a copyright and
  takedown page, a content deletion policy ("We place few restrictions when adding content") and a
  data and privacy policy; none restricts downloading or reuse (F3).
- **Watch-outs:** Google's 1,000-odd scanned objects on Fuel are mostly branded retail goods; two
  Open Robotics CC0 models (`Electrical Box`, `Valve`) were "Generated with mesh_to_sdf" with no
  author or source stated (F2).
- **Recommendation: yes, model by model**, CC0 and CC BY 4.0 only, recording each model's licence
  from the API and its credit from `model.config` at download.

## Findings

### F1. Fuel's licence list and a model's licence field

- https://fuel.gazebosim.org/1.0/licenses (read 2026-11-09) lists seven: ID 1 "Creative Commons Zero
  v1.0 Universal", 2 "Creative Commons Attribution 4.0 International", 3 "Creative Commons
  Attribution Share Alike 4.0 International", 4 "Creative Commons Attribution No Derivatives 4.0
  International", 5 "Creative Commons Attribution Non Commercial 4.0 International", 6 "Creative
  Commons Attribution Non Commercial Share Alike 4.0 International", 7 "Creative Commons Attribution
  Non Commercial No Derivatives 4.0 International".
- The model record, https://fuel.gazebosim.org/1.0/OpenRobotics/models/Oven (read 2026-11-09):
  > `"license_id":2,"license_name":"Creative Commons Attribution 4.0 International","license_url":"http://creativecommons.org/licenses/by/4.0/"`

  with `"upload_date":"2023-09-29T16:51:37Z"`, `"modify_date":"2023-09-29T16:51:37Z"`,
  `"version":1`, `"private":false`, categories "Furniture and Home" and "Appliance". The licence is
  one field per model; the model's files are versioned separately (`/tip/files/...`).
- The model's file list, https://fuel.gazebosim.org/1.0/OpenRobotics/models/Oven/tip/files:
  `materials/textures/` (Glass, Oven_Albedo, Oven_Metalness, Oven_Normal, Oven_Roughness PNGs),
  `meshes/oven.dae`, `model.config`, `model.sdf`, `thumbnails/Oven.png`. **No licence file.**

### F2. What each model's own files say, and each one's licence (read 2026-11-09)

For each model below, the licence is the API's `license_name`; author and description are from its
`model.config` (email addresses left out here). No `model.config` read had a licence element.

| Model (OpenRobotics/...) | Licence (API) | Uploaded | Author in `model.config` | Description in `model.config` |
|---|---|---|---|---|
| `Oven` | CC BY 4.0 | 2023-09-29 | "Cole Biesemeyer and Francesco Coldesina" | see below |
| `Pump` | CC BY 4.0 | 2024-08-28 | "Ian Chen" | "A pump" |
| `Shower` | CC BY 4.0 | 2020-06-25 (modified 2023-09-27) | "Wan Yi Seow" | "A generic shower" |
| `KitchenSink` | CC BY 4.0 | 2020-06-22 (modified 2023-09-27) | "Roselle Carmen" | "A countertop with a kitchen sink" |
| `Fridge` | CC BY 4.0 | 2020-06-09 (modified 2023-09-27) | "Roselle" | "A generic tall fridge with freezer" |
| `Kitchen and Dining` | CC0 1.0 | 2018-01-27 (modified 2023-09-27) | "Nate Koenig" | "A model of a kitchen and dining room." |
| `DRC Practice: Ball Valve` | CC0 1.0 | 2018-01-27 (modified 2023-09-27) | "Christina Gomez" | approximates "a ball valve for the DRC Trials Task 7" |
| `Electrical Box` | CC0 1.0 | 2019-01-29 (modified 2023-09-27) | none | "Generated with mesh_to_sdf" |
| `Valve` | CC0 1.0 | 2019-01-29 (modified 2023-09-27) | none | "Generated with mesh_to_sdf" |

The oven's `model.config` description, in full:

> Original geometry by "Francesco Coldesina" here "https://sketchfab.com/3d-models/gas-oven-8cc35c739c834b27b72d4761578dca3e" under the CC BY 4.0 license here "https://creativecommons.org/licenses/by/4.0/". Optimization, PBR textures, and SDF created by Cole Biesemeyer.

The upstream checks out: Sketchfab's API record for that model
(https://api.sketchfab.com/v3/models/8cc35c739c834b27b72d4761578dca3e, read 2026-11-09) shows
licence "CC Attribution" (`by`, linking CC BY 4.0), user "Francesco Coldesina" (`topfrank2013`),
published 2017-10-28, tags "gas oven kitchen".

Searching Open Robotics' models for the gap items (`?q=pump`, `shower`, `kitchen`, `electrical`,
`valve`, read 2026-11-09) also returned other owners' models in the same lists, among them Google
Research scans named after brands (`Reebok_PUMP_OMNI_LITE_HLS`, `Twinlab_Nitric_Fuel`,
`Calphalon_Kitchen_Essentials_12_Cast_Iron_Fry_Pan_Black`), all CC BY 4.0. A dozen valve models of
Open Robotics' DRC set are CC0; one (`drc_practice_wheel_valve_large`) is CC BY 4.0, so even one
owner's family mixes licences.

### F3. The Fuel service's own policies

- **About Fuel**, https://gazebosim.org/docs/all/fuel/ (Last-Modified 8 November 2026): Fuel is "a
  two-part web application for hosting and accessing simulation assets", the app.gazebosim.org site
  and "a backend server that manages the simulation assets". The footer: "Brought to you by Open
  Robotics." No terms are linked.
- **Content Deletion Policy**, https://gazebosim.org/docs/all/fuel_content_deletion_policy/
  (Last-Modified 8 November 2026):
  > We place few restrictions when adding content to this service, however deletion of content can cause downstream harm.

  Deleted content stays "in a soft-deleted state for one month", and the policy "provides time for
  users to copy content that is in the public domain, or make other arrangements before data is
  deleted."
- **Data and Privacy Policy**, https://gazebosim.org/docs/all/fuel_data_policy/: content "may be
  immediately removed" if it "violates copyright law".
- **Copyright**, https://gazebosim.org/docs/all/fuel_copyright/ (its source last changed 7 May 2026
  per the GitHub API): "This page was mostly copied from Google’s Copyright page." and "Open
  Robotics isn’t able to mediate rights ownership disputes."
- The app (https://app.gazebosim.org/, an Angular single-page app whose bundle is dated 8 August
  2023) has no terms page that I could find in its code, and a web search on 2026-11-09 found none.
  **So the only conditions on a downloaded model are its own licence.**

### F4. What a CC BY 4.0 model owes (the legal code, re-read)

https://creativecommons.org/licenses/by/4.0/legalcode.txt (Last-Modified 11 September 2023), section
3(a)(1): when sharing, "including in modified form", keep, "if it is supplied by the Licensor":

> i. identification of the creator(s) of the Licensed
>    Material and any others designated to receive
>    attribution, in any reasonable manner requested by
>    the Licensor (including by pseudonym if
>    designated);

plus "a copyright notice", "a notice that refers to this Public License", "a notice that refers to
the disclaimer of warranties" and "a URI or hyperlink to the Licensed Material to the extent
reasonably practicable"; and

> b. indicate if You modified the Licensed Material and
>    retain an indication of any previous modifications; and

Section 3(a)(2): "You may satisfy the conditions in Section 3(a)(1) in any reasonable manner based
on the medium, means, and context in which You Share the Licensed Material." Section 6(a): failing
the conditions means the rights "terminate automatically" (restored if cured within 30 days of
discovery, 6(b)). Section 2(b)(2): "Patent and trademark rights are not licensed under this Public
License."

For the oven, "previous modifications" is concrete: Cole Biesemeyer's "Optimization, PBR textures,
and SDF" on Francesco Coldesina's geometry. Our credit keeps both names and both changes, then adds
ours.

## What is still unknown, and what would settle it

1. **Where the two `mesh_to_sdf` CC0 models' meshes came from.** Neither names an author or a
   source. Settled by asking Open Robotics (gazebo+support@openrobotics.org, named in the deletion
   policy; free, one message) or by choosing other models. Low stakes: an electrical box and a valve
   are simple shapes we could model ourselves.
2. **Whether a model's licence field could change after we take it.** CC licences are irrevocable
   for copies already received (CC BY 4.0 section 2(a)(1), "irrevocable license"), so record the
   licence, the model's `version` and the date at download.
3. **What any chosen texture shows.** No texture was opened. Settled by a look at each chosen
   model's albedo PNG before conversion, for printed brands.

## Sources I could not reach

None refused. Fuel has no published terms of service to read (F3).

## What it means for this project

### Certain

- A Fuel model's licence must be read from the Fuel API (or its web page), not from its files (F1).
- CC0 Fuel models may be converted and shipped like any CC0 asset; CC BY 4.0 models may be converted,
  changed and shipped with per-model credit and a note of changes (F4). Format conversion alone is a
  technical modification, not an adaptation (the SimReady finding, F2, section 2(a)(4)).
- NoDerivatives and NonCommercial models are out; ShareAlike models would make our changed version
  ShareAlike too.
- The oven's credit must name Cole Biesemeyer and Francesco Coldesina, link both the Fuel model and
  the Sketchfab original, and say what each changed (F2, F4).
- CC BY does not license trademarks (section 2(b)(2)): the Google Research scans of branded goods
  are not cleared by their licence.
- CC BY models are not CC0 and must be listed as CC BY 4.0 in `LICENSES.md`, as the SimReady finding
  set out.

### Judgement calls

- **Open Robotics' own models first**: they name generic objects, are built at real scale for
  robot simulation, and the newer ones carry PBR maps.
- **Skip Google Research scans** unless an item is plainly generic (a plain bowl), and even then look
  at its texture first.
- **Skip the two `mesh_to_sdf` models** until unknown 1 is answered; their shapes are easy to model.
- **Keep the credit with the asset**, as for SimReady: a licence file and a source note in each
  converted folder, plus the Credits page.

### How the project would credit it

- **In each converted folder** (for example `assets/models/fuel/oven/`): a `SOURCE.md` reading,
  for the oven: "Based on `Oven` from Gazebo Fuel
  (https://app.gazebosim.org/OpenRobotics/fuel/models/Oven), version 1, by Cole Biesemeyer and
  Francesco Coldesina: original geometry by Francesco Coldesina
  (https://sketchfab.com/3d-models/gas-oven-8cc35c739c834b27b72d4761578dca3e); optimization, PBR
  textures and SDF by Cole Biesemeyer (Open Robotics). Licensed under CC BY 4.0
  (https://creativecommons.org/licenses/by/4.0/). Converted to glTF by HumanityOS; [changes].
  Provided as is, without warranties." plus the CC BY 4.0 legal code as `LICENSE.txt`.
- **One file for the set**, `assets/models/LICENSE-gazebo-fuel.md`, listing each model, its
  licence as read from the API, its version and the download date.
- **`data/credits.ron`**: a row `gazebo_fuel` with `licence: "CC BY 4.0 (per model; some CC0)"`, a
  notice such as "3D models from Gazebo Fuel (Open Robotics and contributors), CC BY 4.0, modified;
  see the list of models and authors", `attribution_required: true`, `shown_in: ["Credits"]`.
- **`CREDITS.md`** and **`LICENSES.md`**: the same, as for SimReady.
- **Never**: Open Robotics' or Gazebo's logo, or wording implying they endorse HumanityOS.

### The question for the operator

**May Open Robotics' CC BY 4.0 and CC0 models from Gazebo Fuel (the oven, pump, shower, kitchen
sink, fridge, valves) fill gaps, given that the CC BY ones would join SimReady as models that are not
public domain?**

Recommendation: **yes**, model by model, CC0 and CC BY 4.0 only; licence and version recorded from
the API at download; credit taken from `model.config`, upstream sources included; a texture look per
model; Google Research scans and the two `mesh_to_sdf` models left out.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
