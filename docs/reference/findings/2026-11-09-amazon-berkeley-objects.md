# Amazon Berkeley Objects: the licence, the "Amazon.com" credit, and what the licence does not grant for real branded products

**Research date: 9 November 2026** (Pacific time, early morning). Every source below was read that
day by an AI research agent working for the project, with the user agent
`HumanityOS-research/1.0 (AI research agent reading licence pages; https://united-humanity.us)`,
one request at a time. The dataset's S3 bucket was read through its public listing; its index page,
READMEs and licence file were read; the dataset's CVPR 2022 paper (a PDF, 20.5 MB) was read as a
cited source. **No model, image or archive was downloaded.**

**I am not a lawyer, and this is not legal advice.**

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(sections 4.15 and 7.6), which asked this finding to settle the "Amazon.com" credit wording, the
brand names on products, and which categories exist. Index of this day's asset findings:
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md). The CC BY 4.0 reading
is the one in [`2026-11-08-nvidia-simready-assets.md`](2026-11-08-nvidia-simready-assets.md), F2,
and the branding approach follows its F3.

## The question

May HumanityOS convert and ship 3D models from Amazon Berkeley Objects (ABO), how must it credit
them, and what does the licence not grant for models of real products that carry brands?

## Short answer

- **ALLOWED: CC BY 4.0.** The licence file in the bucket (dated 26 June 2023) is the CC BY 4.0 legal
  code word for word; the README and index page say the same (F1, F2).
- **The licence CHANGED.** The dataset's own CVPR 2022 paper says "The dataset is released under CC
  BY-NC 4.0 license"; Amazon's entry in the AWS Open Data Registry explains that "The license for all
  assets was updated from CC BY-NC to the more permissive CC BY in 2023", a note added on
  16 September 2026. The files in the bucket today carry CC BY 4.0. **Older copies elsewhere may
  still say non-commercial: take files only from the bucket, today** (F3).
- **Required credit:** "Credit for the data, including all images and 3D models, must be given to:"
  "Amazon.com"; the README's title line is a copyright notice ("(c) by Amazon.com") to keep; and the
  dataset builders are to be credited "for building the dataset, archives and benchmark sets" (the
  README names six people, the index page ten) (F2).
- **NOT licensed: the brands, and possibly the product designs.** These are artist-made models of
  real products sold on Amazon, each listing with a `brand` field (the dataset's own example is the
  Amazon brand "Stone & Beam"). CC BY 4.0 licenses Amazon's copyright in the model; "Patent and
  trademark rights are not licensed", and Creative Commons warns that use "may still be restricted
  for other reasons, including because others have copyright or other rights in the material" (F4).
- **UNCLEAR: which products are which brand, and whether appliances are among the 7,953 models.**
  Settled only by reading the listings metadata (an 83 MB archive), which needs the operator's yes
  (unknown 1).
- **Recommendation: yes, with conditions:** pick plain, generic-looking products; ship them under
  generic names, never product names, brand names or ASINs; look at every texture for logos or
  labels; take files from the bucket only.

## Findings

### F1. The licence file in the bucket

The bucket listing, https://amazon-berkeley-objects.s3.amazonaws.com/?list-type=2&delimiter=/ (read
2026-11-09), shows at its root `LICENSE-CC-BY-4.0.txt` (LastModified 2023-06-26, 14,527 bytes),
`README.md` (2023-07-18) and `index.html` (2023-07-18), with folders `3dmodels/`, `archives/`,
`benchmarks/`, `images/`, `listings/`, `spins/` and `static_html/`.

`LICENSE-CC-BY-4.0.txt` opens:

> Creative Commons Attribution 4.0 International Public License

and was compared word by word with Creative Commons' plain-text legal code
(https://creativecommons.org/licenses/by/4.0/legalcode.txt, Last-Modified 11 September 2023): from
"By exercising the Licensed Rights" to "processes of any jurisdiction or authority." the two are
identical (12,806 characters with whitespace normalised). The bucket's copy leaves out only Creative
Commons' own preamble ("Using Creative Commons Public Licenses") and closing notice, neither of
which is part of the licence. No Amazon notice is added inside it.

### F2. What the README and index page say

https://amazon-berkeley-objects.s3.amazonaws.com/README.md (LastModified 2023-07-18), title line:

> # Amazon Berkeley Objects (c) by Amazon.com

Under "License":

> This work is licensed under the Creative Commons Attribution 4.0 International
> Public License.

Under "Attribution":

> Credit for the data, including all images and 3d models, must be given to:

> > Amazon.com

> Credit for building the dataset, archives and benchmark sets must be given to:

followed by six names: Matthieu Guillaumin, Thomas Dideriksen, Kenan Deng, Himanshu Arora (all
Amazon.com), Jasmine Collins and Jitendra Malik (UC Berkeley). It also says of the licence file:
"You must read, agree and comply to the License before using the Amazon Berkeley Objects data."
The `3dmodels/README.md` (2023-07-17) repeats the licence and credit, and describes the models as
glTF 2.0 "(GLB/binary representation)", "Positive `Y` direction is up", with standing products
translated "such that they *stand* on the `Y=0` plane".

The index page, https://amazon-berkeley-objects.s3.amazonaws.com/index.html (LastModified
2023-07-18): "A CC BY 4.0-licensed dataset of Amazon products with metadata, catalog images, and 3D
models." Its "Attribution" section has the same "Amazon.com" line but names **ten** builders: the
six above plus Arnab Dhua, Xi (Brian) Zhang, Tomas Yago-Vicente (Amazon.com) and Shubham Goel (UC
Berkeley). Its footer: "© Amazon.com 2021-2023". It counts "more than 7'900 products" with 3D models
"in the standard glTF 2.0 format", with "4K texture maps for physically based rendering"; the README
gives 7,953.

### F3. The licence was non-commercial until 2023

- The dataset paper, https://amazon-berkeley-objects.s3.amazonaws.com/static_html/ABO_CVPR2022.pdf
  (LastModified 2022-04-27; arXiv 2110.06199, last revised 24 June 2022), introduction:
  > The dataset is released under CC BY-NC 4.0 license

  continuing "and can be downloaded at" the dataset's address.
- Amazon's entry in the AWS Open Data Registry,
  https://github.com/awslabs/open-data-registry/blob/main/datasets/amazon-berkeley-objects.yaml
  (read from the raw file 2026-11-09), `License: Creative Commons Attribution 4.0 (CC BY 4.0)`, and
  in its description:
  > (NB - The license for all assets was updated from CC BY-NC to the more permissive CC BY in 2023, older announcements, artefacts or publications might not reflect this change)

  The file's history (GitHub API) dates that note: commit of 16 September 2026, "Fix license
  information ABO", with the message "The license was changed from CC BY-NC to CC BY in 2023,
  updating the registry information and adding clarification in description." The registry names
  Amazon as manager and `amazon-berkeley-objects@amazon.com` as contact.
- **Where the sources disagree:** the 2022 paper says NC; the 2023 licence file, README, index page
  and the 2026 registry note say CC BY. The licence file travels with the data and is the later
  instrument; I take CC BY 4.0 as governing files taken from the bucket today (a judgement, but one
  Amazon's own registry note states plainly).

### F4. Brands and real products: what the licence does and does not grant

- **These are real products.** The paper: the dataset "is derived from Amazon.com product listings,
  and as a result, contains imagery and 3D models that correspond to modern, real-world, household
  items", with "artist-designed 3D meshes". The index page's sample listing is the "Stone & Beam
  Westport Modern Nailhead Upholstered Sofa", whose metadata includes `"brand"` "Stone & Beam",
  `"model_number"` and item names reading "Amazon brand" in several languages; the paper's figure
  shows two "Rivet" products. Listings metadata fields include `brand`, `model_name`,
  `model_number` and `product_type` (listings `README.md` and the Athena schema on the index page).
- **CC BY 4.0 licenses copyright, not marks**, section 2(b)(2) of the legal code:
  > 2. Patent and trademark rights are not licensed under this
  >    Public License.

  and section 2(a)(6) grants no permission to imply endorsement by the licensor.
- **Others' rights may remain.** Creative Commons' preamble to the same legal code ("Considerations
  for the public", not part of the licence):
  > Our licenses grant only permissions under copyright and certain
  > other rights that a licensor has authority to grant. Use of
  > the licensed material may still be restricted for other
  > reasons, including because others have copyright or other
  > rights in the material.

  (quoted from `legalcode.txt`, where those words fall mid-line.) Amazon can license its own models
  and photographs. A brand name, a logo on a texture, or a protected product design belonging to the
  maker of a third-party product is not Amazon's to license, and the licence does not try to.
- **Not settled here:** how many of the 7,953 models are Amazon's own brands and how many are other
  makers', and whether any appliances are among them. The 3D metadata file
  (`3dmodels/metadata/3dmodels.csv.gz`, 340,528 bytes) carries only counts and sizes; brands and
  product types are in the listings (`archives/abo-listings.tar`, "83 Mb").

## What is still unknown, and what would settle it

1. **The brand and product type of every model with a 3D file.** Settled by downloading the listings
   metadata (83 MB, Amazon's bucket) and joining it to `3dmodels.csv.gz` on `3dmodel_id`: a script
   of minutes. It needs the operator's yes, since it is a download. It would also answer whether any
   stove, oven or washer is in the set.
2. **Whether a particular product's design is protected by someone other than Amazon** (a design
   patent, a registered design, a famous designer piece). Not something a licence page can answer;
   settled per item only by a lawyer. Avoiding recognisable designer pieces and choosing plain
   products keeps the question small.
3. **Whether Amazon wants a particular credit format.** It asks for "Amazon.com" and the builders;
   nothing more specific. Settled by the dataset contact if ever needed.

## Sources I could not reach

None refused. Not read: the listings metadata itself (a download, unknown 1) and the 3D model
files.

## What it means for this project

### Certain

- ABO's 3D models, taken from the bucket today, may be converted, changed and shipped under CC BY 4.0,
  with credit to "Amazon.com", its copyright notice kept, a licence link and a note of changes (F1,
  F2; the SimReady finding's F2 for the conditions).
- Copies obtained from anywhere else (mirrors, older archives, other datasets that bundled ABO before
  2023) may carry the old non-commercial terms and should not be used (F3).
- The licence grants no rights in brand names or logos and does not clear third parties' rights in
  the products depicted (F4). It is not CC0, and the models must be listed as CC BY 4.0 in
  `LICENSES.md`.

### Judgement calls

- **Generic names only.** Ship a model as "fabric sofa" or "bookshelf", never as its listing title,
  brand, model number or ASIN, in the game, its data or the repository's file names. The 3D model ID
  can stay in a source note as provenance.
- **Prefer plain products, and Amazon's own brands among them.** Amazon can license the copyright in
  its own products' models; for other makers' products, pick only plain, non-distinctive designs.
- **Look at every texture** for a logo, label or printed name before conversion, as for SimReady.
- **Credit the builders too.** The README makes builder credit a condition for "building the dataset,
  archives and benchmark sets", which we would not redistribute; crediting them in `CREDITS.md`
  costs one line and is kind. Use the index page's ten names (it is the later, larger list).

### How the project would credit it

- **In each converted folder**: a `SOURCE.md`: "Based on 3D model `<3dmodel_id>` from Amazon
  Berkeley Objects (https://amazon-berkeley-objects.s3.amazonaws.com/index.html), (c) Amazon.com,
  licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/). Converted to glTF by
  HumanityOS; [changes]. Provided as is, without warranties." and the bucket's
  `LICENSE-CC-BY-4.0.txt` copied unchanged.
- **`data/credits.ron`**: a row `amazon_berkeley_objects`, `licence: "CC BY 4.0"`, notice "3D models
  from Amazon Berkeley Objects, (c) Amazon.com, CC BY 4.0, modified", `attribution_required: true`,
  `shown_in: ["Credits"]`.
- **`CREDITS.md`**: the same line, then "Dataset built by" and the ten names; **`LICENSES.md`**: an
  entry beside SimReady's.
- **Never**: Amazon's logo, any product or brand name on screen, or wording suggesting Amazon endorses
  HumanityOS.

### The question for the operator

**May the project use Amazon Berkeley Objects models for furniture and home goods, under generic
names, and may an agent download the 83 MB listings metadata first, so each pick's brand and product
type can be checked (and whether any appliances exist)?**

Recommendation: **yes to both**: the metadata download first (it settles which models are generic
and whether the set fills any gap), then models chosen one by one from the bucket, under generic
names, textures checked, credited as above.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.

## Addendum, 2026-11-09 (later): the listings read

Approved by the operator the same morning ("These all sound reasonable. LEt's proceed."), the listings
archive was fetched from Amazon's bucket:
https://amazon-berkeley-objects.s3.amazonaws.com/archives/abo-listings.tar, 87,480,320 bytes
(server Last-Modified 2023-07-17), SHA-256
`b7f7ceacb328fa5ab6e143b88e1f948443a877cfc95b67ff09c8ebabd50644e3`, kept at
`E:\HumanityOS\abo-src\` with `LICENSE-CC-BY-4.0.txt` inside it and a scan's output
(`abo-3d-scan.json`). The archive's own README repeats the credit lines quoted above ("Amazon.com"
for the data, the six builders for the dataset).

What it settles (the unknown above): of the listings, **7,960 items have a 3D model**, across 98
product types, overwhelmingly furniture and decor: CHAIR 1,184, SOFA 859, RUG 817, WALL_ART 599,
TABLE 584, HOME_FURNITURE_AND_DECOR 550, LAMP 345, STOOL_SEATING 340, OTTOMAN 288, PILLOW 275,
HEADBOARD 206, LIGHT_FIXTURE 197, PLANTER 182, DESK 145, BED 140, CABINET 137, HOME_MIRROR 109,
SHELF 106. **None of the homes' gap appliances is there:** a search of every 3D item's name finds no
oven, washer, dryer, refrigerator or freezer, shower, water heater, dishwasher, microwave, sink,
toilet or bathtub (a "stove" pattern matched only words like "Orange"). The nearest are
ELECTRIC_FAN 29 (ceiling fans), AIR_CONDITIONER 13 and DEHUMIDIFIER 3. Most items carry Amazon's own
brand names (Rivet, Stone & Beam), so the generic-names condition above applies to all of them.

**What it means now:** ABO is a furniture and decor source (chairs, sofas, tables, beds, lamps, rugs,
shelves, cabinets), not an appliance source; the stove, oven, shower, washer and water heater come
from Poly Haven, Gazebo Fuel or a hand-picked Sketchfab model, as the index says. Choosing ABO
furniture models means fetching each from the 3D model archive (each model a separate download,
listed by name and size first).
