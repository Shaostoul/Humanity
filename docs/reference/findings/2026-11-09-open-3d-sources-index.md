# Open 3D model, material and HDRI sources: the verdicts of 9 November 2026, in one table

**Research date: 9 November 2026.** An index to the day's findings on the sources ranked in the
2026-11-08 asset scan, [`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md).
Every verdict below comes from the linked finding, which carries the sources, their dates and the
quotations; read the finding before acting on a row.

**I am not a lawyer, and this is not legal advice.**

## The question

Which of the open sources the scan ranked may HumanityOS use for models, materials and sky photos
shipped in a free, CC0, open and moddable game and app (converted by its own tools, often run by AI
agents), and on what conditions?

## Short answer

| Source | Licence of what we would ship | Verdict | Credit | Key conditions | Finding |
|---|---|---|---|---|---|
| **Poly Haven** (models, textures, HDRIs) | CC0 | **Yes** | Not required; courtesy line per asset | Fetch through the API with a user agent naming our software (the website bans scraping); an in-app live-API browser would have to name Poly Haven; never ship their descriptions, renders or logo; brand, sign and person glance per asset | [poly-haven-and-ambientcg](2026-11-09-poly-haven-and-ambientcg.md) |
| **ambientCG** (materials, HDRIs) | CC0 (previews too) | **Yes** | Not required; its suggested line | No terms of service or API terms exist; one person runs it, so fetch gently | [poly-haven-and-ambientcg](2026-11-09-poly-haven-and-ambientcg.md) |
| **Gazebo Fuel** (Open Robotics models) | per model: CC0 or CC BY 4.0 usable; SA, ND, NC not | **Yes, model by model** | Per CC BY model: author(s) from `model.config`, upstream sources, links, changes | Licence lives in the Fuel API, not the files; the oven credits a Sketchfab original too; skip branded Google scans and the two authorless `mesh_to_sdf` models; no service terms found | [gazebo-fuel-models](2026-11-09-gazebo-fuel-models.md) |
| **Amazon Berkeley Objects** | CC BY 4.0 (since 2023; NC before) | **Yes, with conditions** | "(c) Amazon.com", CC BY 4.0, modified; builders in `CREDITS.md` | Take files only from Amazon's bucket; generic names, never product, brand or ASIN; texture check; brands and others' design rights are not licensed; listings metadata (83 MB) to check brands needs the operator's yes | [amazon-berkeley-objects](2026-11-09-amazon-berkeley-objects.md) |
| **Kenney** | CC0 | **Yes (stand-ins)** | Not required; "Kenney" if given | Logo reserved; in-zip licence file and terms unread; kenney.nl asks AI agents not to read it, so a person checks | [kenney-assets](2026-11-09-kenney-assets.md) |
| **Sketchfab** (CC0 and CC BY filters) | per model: CC0 or CC BY 4.0 usable; Standard, Editorial, NC, ND not | **Yes, last resort, hand-picked** | Per model: title, author, links, licence, changes | Operator downloads with his account; avoid NoAI (tag or statement) and CreatedWithAI; no brands or fan art; texture check; never bulk-harvest | [sketchfab-cc-models](2026-11-09-sketchfab-cc-models.md) |
| **NASA 3D Resources** | NASA guidelines (US public domain, generally); `meta.json` says NOSA 1.3 | **Reference yes; ship NASA-made ones with care** | "NASA" | No insignia or logotype; no endorsement; per-model authorship unrecorded (non-NASA contributors exist); one question to NASA would settle NOSA | [reference-3d-collections](2026-11-09-reference-3d-collections.md) |
| **Khronos glTF Sample Assets** | per model: 62 CC0, 45 CC BY 4.0, 34 mixed (30 with logo entries, 4 with two licences), 9 not usable | **Yes, per model** | Per model, from its `metadata.json` | Remove logos; never the 9 (NC, CRYENGINE, Poser, Adobe Stock, Stanford, SCEA, testing-only) | [reference-3d-collections](2026-11-09-reference-3d-collections.md) |
| **Smithsonian Open Access** | CC0 per record | **Later** | Per record | Terms of use unread (robot check); no gap items | [reference-3d-collections](2026-11-09-reference-3d-collections.md) |
| **NVIDIA SimReady** | CC BY 4.0 | Yes (earlier finding) | NVIDIA, CC BY 4.0, modified | Leave out MDL files and the two branded assets | [2026-11-08-nvidia-simready-assets](2026-11-08-nvidia-simready-assets.md) |
| **Quaternius** (animation library) | CC0 in the download; site page says QAL | Yes for the file obtained (earlier finding) | Not required | Keep `License.txt` beside the clips | [2026-11-08-character-assets](2026-11-08-character-assets.md) |

Not researched today (lower value for the named gaps, or covered by the scan's plain "no"): the
FreeCAD parts library (CC BY 3.0 CAD), Infinigen (BSD-3 code; whether generated meshes carry any
condition), KayKit, Blendkit's CC0 subset, Objaverse, Printables and Thingiverse, OpenGameArt, and a
dated negative finding for Quixel Megascans on Fab. The scan's section 7 still lists what each needs.

## Lessons that apply to every source

1. **Read `robots.txt` first, before any page.** kenney.nl disallows named AI agents (Claude's
   included) and, for every automated reader, its `/assets/` pages; the 2026-11-08 scan and this
   finding both read pages there before seeing it (the Kenney finding records it).
2. **The licence often lives outside the files.** Poly Haven's and Fuel's downloads carry no licence
   text; Fuel's sits in its database, Poly Haven's on its website. Record the licence, date and
   version at download, and put a licence note beside every converted folder.
3. **Credits chain.** Fuel's oven and Khronos's refrigerator are both built on Sketchfab CC BY
   originals and credit them; our credit keeps the whole chain and adds our changes.
4. **Licences change, and old papers do not.** Amazon Berkeley Objects was CC BY-NC in its 2022
   paper and CC BY since 2023; take files from the source as it stands today.
5. **CC licences cover copyright only.** Brands, logos, signs, identifiable people and other
   companies' product designs are never cleared by CC0 or CC BY; every chosen asset gets a texture
   look.
6. **AI agents in the pipeline matter only where an artist said no.** Poly Haven explicitly welcomes
   AI use; Sketchfab's NoAI content is the one place an AI-run conversion is a real question, and the
   answer is to avoid it.

## What it means for this project

**Certain:** the CC0 sources (Poly Haven, ambientCG, Kenney, CC0 items elsewhere) may ship freely;
the CC BY sources (Fuel, ABO, Sketchfab, Khronos, SimReady) may ship with per-asset credit, a note of
changes and a `LICENSES.md` entry, and must not be called public domain.

**Judgement:** take sources in the order of the table (Poly Haven and ambientCG first, Sketchfab
last), model our own where nothing open exists (CO2 scrubber, electrolyser, water recycler, battery
bank, charge controller, tank water heater), and keep every asset's provenance in a source note.

**The questions for the operator** are at the end of each finding; the two that need his hands are
Sketchfab downloads (his account) and the ABO listings metadata download (his yes).

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026.
