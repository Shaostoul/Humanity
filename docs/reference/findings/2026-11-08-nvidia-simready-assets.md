# NVIDIA SimReady Assets: may the project convert, change and ship them, and how must it credit them

**Research date: 8 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself as a research tool. The dataset was read
through Hugging Face's public API and raw file pages: its card, its file tree, and for every one of
its 2,677 assets the small licence metadata file and the presence of its licence file. No asset
(model, texture or physics file) was downloaded. Each source carries its own date where it shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences and metadata,
written so a later reader can check every word against the same sources.

Builds on the 2026-11-08 AI tools scan,
[`../research/2026-11-08-ai-tools/A-3d-worlds-game.md`](../research/2026-11-08-ai-tools/A-3d-worlds-game.md),
which listed the dataset as a candidate and asked this finding to settle each asset's own licence,
the MDL files' terms, and how attribution is shown.

## The question

May HumanityOS convert NVIDIA's SimReady Assets from USD to glTF, change them, and ship them in the
game's data, and how must it credit them?

## Short answer

- **ALLOWED: convert, change and ship them**, in the app and on the website, under CC BY 4.0. A format
  conversion alone is a "technical modification" that never makes an adaptation; real changes
  (decimating, re-texturing, splitting) make an adaptation, which the licence also allows (F2).
- **Every asset says CC BY 4.0 for itself.** All 2,677 asset folders were checked: each one's
  `com.nvidia.simready.packaging.json` reads `"license": "CC-BY-4.0"` and each carries
  `.metadata/LICENSE.txt` (F1, F3). No exception was found.
- **No NVIDIA terms beyond CC BY 4.0** were found: the dataset is not gated, has no root licence file
  or NOTICE, and the card says only "open-source" and `license: cc-by-4.0`. Hugging Face's own terms
  add one thing: never remove the reference to the licence (F1, F5).
- **Attribution is owed, in a reasonable manner for the medium**: name NVIDIA as creator, say it is
  CC BY 4.0 with a link, link the dataset, and say what was changed. NVIDIA supplied no copyright
  notice and requested no wording, so a Credits entry plus a licence file and a short source note in
  each converted asset's folder meets it (F2, F3).
- **The MDL shader files carry different terms** (NVIDIA's BSD-style licence, "Copyright 2022 NVIDIA
  Corporation. All rights reserved."). The engine does not need them; leave them out and the question
  disappears (F4).
- **NOT licensed: anyone's name or marks.** CC BY grants no trademark rights and no endorsement; use
  "NVIDIA" only to credit the source (F2). Two assets are named after other companies' trademarked
  products, `consumable_lubricant_silicone_wd40_a01` (WD-40) and `toys_rubiksCube_a01` (Rubik's
  Cube): NVIDIA could license its model, not those marks. Leave them out, or ship them only under a
  generic name with any printed mark painted out (F3).
- **UNCLEAR: what the textures show.** No picture was looked at. Packaged food, signs and medicine
  boxes could carry printed labels; each chosen asset's colour texture needs a look before it ships
  (F3).
- **Not CC0.** These assets stay CC BY 4.0 inside the CC0 project and must be listed as such, the way
  the Solar System Scope textures and the star catalogues already are (F6).
- **Recommendation:** yes, use them, chosen asset by asset, with the conditions above.

## Findings

### F1. The dataset card and its metadata

- https://huggingface.co/datasets/nvidia/simready-assets (created 2026-06-29, last modified
  2026-11-04; API: `"gated": false`, `"private": false`; tags `license:cc-by-4.0`, `simready`).
  The card's metadata is `license: cc-by-4.0` and `viewer: false`; its text, in full apart from its
  heading:
  > Welcome to NVIDIA's open-source SimReady Assets dataset!

  > [SimReady Central](https://huggingface.co/spaces/nvidia/simready-central) brings together OEM-curated datasets like this one, helping you discover simulation-ready assets for your projects.

  and an invitation to contribute through `nvidia/simready-central-library`.
- **The root holds only `.gitattributes` and `README.md`**: no `LICENSE`, no `NOTICE`, no terms file
  (tree API, 2,679 entries: 2,677 asset folders and those two files).
- **Other SimReady Central datasets are not this one.** `nvidia/simready-central-library` (created
  2026-09-29) is gated ("manual") and tagged `license: apache-2.0`. This finding covers
  `nvidia/simready-assets` only.
- **Not the NVIDIA Open Model Agreement.** NVIDIA's MotionBricks weights, researched the same day in
  [`2026-11-08-motionbricks.md`](2026-11-08-motionbricks.md), carry that agreement; nothing in this
  dataset that was read (the card, the packaging files, one asset's metadata in full) refers to it.

### F2. What CC BY 4.0 allows and asks

The licence text read is the one each asset carries (F3), byte-identical to Creative Commons' own
plain-text legal code (https://creativecommons.org/licenses/by/4.0/legalcode.txt, served with
Last-Modified 11 September 2023; both SHA-256 `9ba9550a...9411`).

- **The grant**, section 2(a)(1): to "reproduce and Share the Licensed Material, in whole or in part"
  and to
  > produce, reproduce, and Share Adapted Material.
- **Converting the format is not adapting**, section 2(a)(4):
  > The Licensor authorizes You to exercise the Licensed Rights in all media and formats whether now known or hereafter created, and to make technical modifications necessary to do so.

  > For purposes of this Public License, simply making modifications authorized by this Section 2(a) (4) never produces Adapted Material.

  (In the plain-text file "2(a)" and "(4)" fall on two lines.)
- **Changing it is adapting**, section 1(a): Adapted Material is material "in which the Licensed
  Material is translated, altered, arranged, transformed, or otherwise modified in a manner requiring
  permission".
- **Attribution**, section 3(a)(1), when sharing, "including in modified form":
  > a. retain the following if it is supplied by the Licensor with the Licensed Material:

  namely "identification of the creator(s) of the Licensed Material and any others designated to
  receive attribution, in any reasonable manner requested by the Licensor", "a copyright notice", "a
  notice that refers to this Public License", "a notice that refers to the disclaimer of warranties"
  and "a URI or hyperlink to the Licensed Material to the extent reasonably practicable"; and
  > b. indicate if You modified the Licensed Material and retain an indication of any previous modifications; and

  > c. indicate the Licensed Material is licensed under this Public License, and include the text of, or the URI or hyperlink to, this Public License.
- **How**, section 3(a)(2):
  > You may satisfy the conditions in Section 3(a)(1) in any reasonable manner based on the medium, means, and context in which You Share the Licensed Material.
- **Adaptations**, section 3(a)(4):
  > If You Share Adapted Material You produce, the Adapter's License You apply must not prevent recipients of the Adapted Material from complying with this Public License.
- **No added restrictions**, section 2(a)(5)(B): no "additional or different terms or conditions" and
  no "Effective Technological Measures" that restrict recipients.
- **Not licensed**, section 2(b)(2):
  > Patent and trademark rights are not licensed under this Public License.

  and section 2(a)(6), no endorsement: nothing is permission "to assert or imply that You are, or that
  Your use of the Licensed Material is, connected with, or sponsored, endorsed, or granted official
  status by, the Licensor".
- **Losing the credit ends the licence**, section 6(a):
  > However, if You fail to comply with this Public License, then Your rights under this Public License terminate automatically.

  Section 6(b) reinstates it "automatically as of the date the violation is cured, provided it is
  cured within 30 days of Your discovery of the violation". Section 6(c): the licensor may stop
  distributing "at any time; however, doing so will not terminate this Public License."
- **No warranty**, section 5(a): the licensor offers the material "AS-IS AND AS-AVAILABLE".

### F3. Each asset's own licence

- **One asset read in full**, `appliance_toaster_a01`. Its folder holds `.metadata/LICENSE.txt`
  (18,657 bytes, the CC BY 4.0 legal code above, with no NVIDIA notice added at the top or bottom),
  four other `.metadata` JSON files (two conformance reports, a bill of materials with each file's
  SHA-256, a root-USD pointer), `com.nvidia.simready.packaging.json`, and `simready_usd/` with the
  USD files, `Textures/` (albedo, normal and ORM PNGs), `payloads/` (PhysX, Newton, MuJoCo and a
  generic physics layer), `.thumbs/` and `source_assets/` (the MDL files and copies of the PNGs).
  Its packaging file:
  > `"package_id": "simready.hf.nvidia.simready-assets.appliance_toaster_a01.2026.10.13_180535304265",`

  > `"license": "CC-BY-4.0",`

  It names no author and no copyright holder; `payloads/base.usda` holds no licence, copyright or
  author field either.
- **Every asset checked.** For all 2,677 asset folders, the `license` field of each
  `com.nvidia.simready.packaging.json` was read, and the presence and size of each
  `.metadata/LICENSE.txt` checked (one request at a time, 2026-11-08, Pacific evening):

  | What was checked | Result |
  |---|---|
  | `license` in `com.nvidia.simready.packaging.json` | `CC-BY-4.0` in 2,677 of 2,677 (every file read, HTTP 200) |
  | `.metadata/LICENSE.txt` present | 2,677 of 2,677 (HTTP 200) |
  | its size | 18,657 bytes in all 2,677, the size of the CC BY 4.0 legal code |

  Nine licence files were also hashed (`appliance_toaster_a01`, `food_produce_apple_grannySmith_a01`,
  `sign_hospital_board_a01`, `rack_h3m_s01`, `IV_pole_a01`, `medicine_a03`,
  `food_packaged_mix_cake_a01`, `computer_printer_office_a01`, `kitchenware_knife_m01`): all nine are
  byte-identical to Creative Commons' CC BY 4.0 legal code.
- **Two assets are named after trademarked products.** The 2,677 folder names were searched for
  company and product names and for "logo", "brand", "label", "sticker" and "decal". Two name a
  branded product: `consumable_lubricant_silicone_wd40_a01` (WD-40) and `toys_rubiksCube_a01`
  (Rubik's Cube), whose colour texture is `RubikCube_Color.png`. `dishware_thermos_a01` (a vacuum
  flask, a word that is also a brand; its textures are generic chrome, plastic and rubber tiles),
  `tool_tape_duct_a02` and apple varieties such as `food_produce_apple_grannySmith_a01` also matched;
  nothing else did. A first, narrower search missed the two; this is a word search, not a guarantee. The largest groups are
  food (538), kitchenware (179), boxes (154), hand tools (97), consumables (77), paint (68) and body
  parts (67).
- **Texture names are generic; their pictures were not seen.** Texture file names checked for four
  more assets (packaged cake mix, a hospital sign board, an office printer, a medicine box) are
  material or object names (`t_food_packaged_mix_cake_a01_alb.png`, `t_metal_steel_a01_tile_alb.png`,
  `T_Plastic_Black_B_Albedo.png`, `t_medicine_a01_alb.png` in `medicine_a03`). A box's own colour
  texture such as `t_medicine_a01_alb.png` is exactly where printed packaging would be. **What the
  textures depict was not checked**: that needs each image opened.

### F4. The MDL material files: NVIDIA's BSD-style licence

- `appliance_toaster_a01/simready_usd/source_assets/OmniPBR.mdl` opens:
  > Copyright 2022 NVIDIA Corporation. All rights reserved.

  and grants redistribution on BSD-3-Clause terms, including:
  > Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

  > Neither the name of NVIDIA CORPORATION nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

  (Each line of the header begins with ` * `, dropped here.) The same folder has
  `OmniPBRBase.mdl` and `OmniPBR_ClearCoat.mdl`, not opened.
- These are shader programs for NVIDIA's renderers. HumanityOS's engine draws with its own
  metallic-roughness materials from the PNG maps; the MDL files need not ship.

### F5. Hugging Face's terms

https://huggingface.co/terms-of-service ("Effective Date: September 15, 2022"):

> When Content contains notice of a reasonable and customary license, (such as an open source license) such Content is intended to remain under the terms of such license when further accessed, distributed, or used. Neither party is permitted to remove reference to any such license.

and:

> Any Content you download, access or use from us or another User, is at your own risk and subject to these Terms and/or the terms accompanying such Content.

Nothing else on the page restricts using a public dataset.

### F6. How the project already handles CC BY material (its own records)

- `LICENSES.md` records two share-alike star catalogues and the MPC orbits, and says of the root
  licence: "The project dedicates to the public domain what its contributors wrote. It also
  redistributes third-party works, some share-alike, that it cannot and does not dedicate."
- `CREDITS.md`, "Data and imagery sources", already credits a CC BY 4.0 source, the Solar System
  Scope textures, by name, licence and link.
- The 2026-09-06 attribution audit ([`../attribution-audit-2026-09-06.md`](../attribution-audit-2026-09-06.md),
  finding 9) found that the one CC BY source whose output ships had no row in `data/credits.ron`, so
  the in-app Credits page and its unit test could not see it; it asked for a row with
  `attribution_required: true`. The same applies here.
- `data/credits.ron` explains its own rule: "an attribution-REQUIRED source that renders content
  must also name the surface that renders it", and `crates/humanity-core/src/credits.rs` fails when
  a required source names no surface.
- Release archives copy `data/` and `assets/models/` wholesale (`.github/workflows/build-desktop.yml`),
  and `assets/models/LICENSE-cc0-model-packs.md` already credits the CC0 packs there, so a licence
  file placed beside converted models travels with every release.
- [`2026-10-12-media-item-licences.md`](2026-10-12-media-item-licences.md), section 1.2, read the
  same licence on 2026-10-12 and reached the same reading: "Anyone, including the project, may use a
  CC BY item in a game or a promotional video, for any purpose, with credit given in a reasonable
  manner for the medium."

## What is still unknown, and what would settle it

1. **Whether any chosen asset's texture shows a real brand, logo, label or a person's likeness.**
   Settled by opening each chosen asset's albedo PNG (and thumbnail) before it is converted: minutes
   per asset, done as part of choosing it. CC BY licenses NVIDIA's copyright only; a third party's
   mark printed on a box would not be NVIDIA's to license (F2, 2(b)(2)). The two assets already named
   after trademarked products (`consumable_lubricant_silicone_wd40_a01`, `toys_rubiksCube_a01`) need
   that look first, or are simply left out.
2. **Whether NVIDIA wants a particular credit line.** It requested none (F3). Settled by NVIDIA
   adding one to the card, or by asking on the dataset's Community tab (free; posting is the
   operator's call). Not needed to proceed.
3. **Whether the dataset's terms change later.** CC BY 4.0 is irrevocable for copies already taken
   (section 2(a)(1): "irrevocable license"; section 6(c), F2), so recording each converted asset's
   `package_id` (which carries its version date) fixes what was taken and when.

## Sources I could not reach

None refused. Not attempted: NVIDIA's own SimReady pages on nvidia.com and the USPTO trademark search
for "SimReady" (a script-driven search page); neither changes the dataset's licence, and the
trademark question is answered for practical purposes by using NVIDIA's name only in credits.

## What it means for this project

### Certain

- The project may convert any of these assets to glTF, change them, and ship them in `assets/models/`
  or `data/`, in releases and on the website, under CC BY 4.0 (F1 to F3).
- Converting alone needs no "modified" note (section 2(a)(4)); anything more (decimation, merged
  meshes, re-baked or resized textures, new colliders) needs one (section 3(a)(1)(B)).
- Attribution with a licence link is a condition, not a courtesy: failing it ends the licence for the
  project until cured (CC BY 4.0, section 6(a) and (b), F2). It must survive in the app, in the
  release archive and on the website.
- The converted assets are not CC0 and must not be described as public domain: list them as CC BY 4.0
  in `LICENSES.md` and keep the CC0 scope sentence true (F6).
- No DRM or extra terms may be put on them (section 2(a)(5)(B)); the project uses none.
- The licence covers NVIDIA's copyright, not other companies' trademarks (section 2(b)(2)), so the
  WD-40 and Rubik's Cube assets, and any texture showing a real brand, are not cleared by it (F3).
- Leaving out the MDL files avoids the BSD notice entirely (F4). If one ever ships, its NVIDIA
  copyright notice and licence must travel with it.

### Judgement calls

- **Credit both ways: one line on the Credits page, and a note in each asset's folder.** For a game,
  a Credits entry naming NVIDIA, the dataset link and the licence link, plus a list of the assets
  used, is the "reasonable manner" section 3(a)(2) describes. Showing the credit in the item viewer
  when one of these objects is inspected is a kind touch, not a requirement.
- **Keep the physics data with its asset.** Masses and collider shapes taken from the payloads are
  part of the licensed material as published; credit them with the asset rather than arguing that a
  number is a fact.
- **Check fit before volume.** The library leans to hospitals, warehouses and car parts; the scan's
  room-by-room fit check (kitchen, life support, a medical area) is the right filter, and each pick
  gets the texture look in unknown 1.

### How the project would credit it

- **In each converted asset's folder** (under `assets/models/`, for example
  `assets/models/simready/appliance_toaster_a01/`): the asset's own `LICENSE.txt`, copied unchanged,
  and a `SOURCE.md` with: "Based on `appliance_toaster_a01` from NVIDIA SimReady Assets
  (https://huggingface.co/datasets/nvidia/simready-assets), package
  `simready.hf.nvidia.simready-assets.appliance_toaster_a01.2026.10.13_180535304265`, by NVIDIA,
  licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/). Converted from USD to glTF
  by HumanityOS; [list of changes, or 'no other changes']. Provided as is, without warranties (see
  LICENSE.txt, section 5)."
- **One file for the whole set**, `assets/models/LICENSE-nvidia-simready.md`, beside the existing
  `LICENSE-cc0-model-packs.md`: the same credit in general form and the list of folders taken.
- **`data/credits.ron`**: a row `nvidia_simready` with `licence: "CC BY 4.0"`, the notice "3D models
  based on NVIDIA SimReady Assets, by NVIDIA, CC BY 4.0, modified", `attribution_required: true` and
  `shown_in` naming "Credits" (and the item viewer if it shows the line). The unit test then guards it.
- **`CREDITS.md`** under "Data and imagery sources" (or a new "3D models" heading) and a section in
  **`LICENSES.md`** beside the other obligation-bearing sources, saying these files stay CC BY 4.0.
- **Never**: NVIDIA's logo, "SimReady" branding on screen, or wording that suggests NVIDIA endorses
  HumanityOS.

### The question for the operator

The operator said go on the SimReady assets on the evening of 2026-11-08, to be integrated once this
finding landed (recorded in `data/coordination/orchestrator_state.json`, `recent_decisions`, with
his words "Stuff we don't have to make saves us time"). What remains is the conditions:

**Do these conditions suit you, given that these would be the project's first 3D models that are not
public domain (every model shipped today is CC0, per `assets/models/LICENSE-cc0-model-packs.md`)?**

Recommendation: **yes**, asset by asset, chosen for fit; with no MDL files, each asset's licence file
and a source note in its folder, a `credits.ron` row shown on the Credits page, the `LICENSES.md`
entry, a look at each chosen asset's textures for brands or labels before it ships, and the WD-40
and Rubik's Cube assets left out (or renamed and their marks painted out).

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
