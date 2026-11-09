# The character body and clips: which files, under which licence, and where SOMA's joint names come from

**Research date: 2026-11-08.** Sources read that day; every quotation is from the page named beside
it. Written for character animation rung 1 (docs/design/character-animation.md), whose real assets
need the operator's yes before any file is downloaded. Builds on
[`2026-11-08-motionbricks.md`](2026-11-08-motionbricks.md), section 11 (MakeHuman, Quaternius), and
does not replace it.

## The question

Which exact files make up the MakeHuman CC0 body and the Quaternius CC0 clips the plan names, how big
are they, under what licence is each offered today, and from which source may SOMA's 30 joint names
be taken?

## Short answer

- **MakeHuman: CC0, and the smallest useful set is four things**: `base.obj` (1,749,303 bytes),
  `default.mhskel` (117,790 bytes), `default_weights.mhw` (897,521 bytes) and the macro targets for a
  young adult (96 files, 30,416,872 bytes; the full macro set is 348 files, 110,317,300 bytes). All on
  GitHub, no Git LFS, no release assets.
- **Quaternius: the licence is in conflict.** The pack page and the itch.io page say CC0. The site's
  own licence page now shows the "Quaternius Asset License (QAL) v1.0", "Last updated: 8/28/2026",
  which forbids redistributing the assets themselves and says the version in force when you obtained
  them governs. A game that ships its clips in an open, moddable `data/` folder needs CC0, not the
  QAL. The pack pages do not list clip names, so idle, walk, run and turning in place cannot be
  confirmed before downloading.
- **SOMA's 30 joint names and parents: take them from NVIDIA's Kimodo** (`definitions.py`,
  Apache-2.0), whose order matches MotionBricks' `soma_n1.json`. The `soma_n1.json` file itself sits
  in a folder whose licence notice covers ONNX models, and the file is not named either way.

## Findings

### 1. MakeHuman's licence

- https://github.com/makehumancommunity/makehuman/blob/master/LICENSE.md (last changed 2020-09-26),
  section C: "These assets have been released under CC0 1.0 Universal." The assets it defines are the
  base mesh and proxies, targets and modifiers, textures, clothes, and poses and expressions; the
  source code is AGPL (so MakeHuman's target-blending code is reimplemented, never copied). Its
  section D makes no claim over exports (the MotionBricks finding, section 11, quotes it).
- `makehuman/data/rigs/default.mhskel` (last commit 83e03219, 2020-10-31) carries its own field
  `"license": "CC0"`; 163 bones under `"root"`.

### 2. MakeHuman's files (sizes from the GitHub contents API, read 2026-11-08)

Repository https://github.com/makehumancommunity/makehuman, branch `master` (HEAD a8bc2d54,
2024-06-26). Raw URLs start `https://raw.githubusercontent.com/makehumancommunity/makehuman/master/`.

| File | Path | Bytes | Last commit |
|---|---|---:|---|
| Base mesh | `makehuman/data/3dobjs/base.obj` | 1,749,303 | 2020-09-26 |
| Default skeleton | `makehuman/data/rigs/default.mhskel` | 117,790 | 2020-10-31 |
| Its skin weights | `makehuman/data/rigs/default_weights.mhw` | 897,521 | 2021-03-29 |
| Macro targets | `makehuman/data/targets/macrodetails/` | see below | 2024-03-09 |
| Which targets do what | `makehuman/data/modifiers/modeling_modifiers.json` | 17,484 | |

The macro targets (height, weight and muscle, proportions, the three ancestries): for a young adult
(MakeHuman's default age, 25), 96 files and 30,416,872 bytes: 6 ancestry files, 18
`universal-{female,male}-young-{min,average,max}muscle-{min,average,max}weight.target`, 36
`height/...-{min,max}height.target` and 36 `proportions/...-{ideal,uncommon}proportions.target`.
Every age: 348 files, 110,317,300 bytes. MPFB2 (https://github.com/makehumancommunity/mpfb2, CC0
assets per its LICENSE.md section C) carries the same base mesh (same blob, 1,749,303 bytes) and a
53-bone `rig.game_engine.json` (40,936 bytes) with `weights.game_engine.json` (3,026,778 bytes): a
smaller rig in Unreal's naming, if the 163 bones prove too many.

### 3. Quaternius's Universal Animation Library

- Pack page https://quaternius.com/packs/universalanimationlibrary.html (dated "March 2025"): its
  licence links "CC0" to the Creative Commons CC0 1.0 deed, and its features say "Free to use in
  personal, educational and commercial projects." It claims "120+ Animations", formats FBX, GLB and
  Blend, and names no clip; its categories are "Locomotion in 8 directions, jog, sprint, push,
  crawling, swimming, sitting, death animations and more" (the itch.io page's wording).
- itch.io https://quaternius.itch.io/universal-animation-library (devlog entries dated Mar 21 2025,
  Jan 23 2026 and Jun 16 2026; "v3.0(16/6/2026)"): "Asset license" field "Creative Commons Zero v1.0
  Universal". Files: "Universal Animation Library[Standard].zip" 15 MB (no price shown: the free
  tier), "[Pro].zip" 41 MB "if you pay $9.99 USD or more", "[Source].zip"
  46 MB "if you pay $14.99 USD or more". itch.io gives per-session download links, so no fixed file
  URL exists.
- Licence page https://quaternius.com/license.html: "Quaternius Asset License (QAL) v1.0", "Last
  updated: 8/28/2026". Section 3a forbids: "Resell or redistribute the Assets themselves." Section 7:
  "the version in effect at the time you obtained the Assets governs your use of them." The page
  does not mention CC0.
- The rig the library animates: Universal Base Characters,
  https://quaternius.com/packs/universalbasecharacters.html ("August 2025", labelled CC0; its free
  Standard zip 122 MB on itch.io). Read in a research pass the same day, not quoted here: not needed
  while MakeHuman is the body.

### 4. SOMA's 30-joint layout

- https://github.com/nv-tlabs/kimodo/blob/main/kimodo/skeleton/definitions.py (last commit 4f6494c8,
  2026-04-10; SPDX header Apache-2.0, NVIDIA 2026): class `SOMASkeleton30` gives the 30 joints and
  their parents (written into `data/characters/role_maps/soma_30.ron`, every joint given a role).
- https://github.com/NVlabs/motionbricks/blob/main/motionbricks/assets/onnx/soma/soma_n1.json (commit
  54fc563a, 2026-10-23): the same 30 names in the same order, no parents. Its folder's
  `LICENSE.md` puts the ONNX models under the NVIDIA Open Model Agreement and does not name the JSON;
  the repository's root licence gives code Apache-2.0 and documentation and assets CC-BY-4.0.

## What is still unknown, and what would settle it

1. **Which licence governs a Universal Animation Library download made today.** The itch.io page
   (where the files are) says CC0; the licence page says QAL v1.0, whose section 7 ties the terms to
   the time of obtaining. Settled by reading the licence file inside the zip at download (cost: the
   download, which needs the operator's yes), and by asking Quaternius which applies to the itch.io
   files (cost: one message). Until then, treat clips obtained after 2026-08-28 as possibly QAL.
2. **The clip names** (is there an idle, a walk, a run, turns on the spot?). Settled by listing the
   Standard zip after download, or by opening https://quaternius.com/animviewer.html in a browser.
3. **Whether the 30 names of `soma_n1.json` are under any licence at all.** Joint names are facts,
   and Kimodo's Apache-2.0 copy is a clean source; settled for certain only by NVIDIA (cost: one
   question on the repository).

## Sources I could not reach

- The animation viewer https://quaternius.com/animviewer.html: a compiled viewer whose clip list
  cannot be read as text. Wanted: the clip names.
- itch.io's per-session download URLs: they exist only after the purchase page. Wanted: fixed file
  URLs for the download list.

## What it means for this project

**Certain** (from the quotations):

- MakeHuman's base mesh, targets, default skeleton and weights are CC0 and may ship in `data/` and
  be modified; its code may not be copied into ours (AGPL).
- The Quaternius pack and itch.io pages say CC0; the Quaternius licence page says QAL v1.0, which
  forbids redistributing the assets themselves.
- Kimodo's `definitions.py` is Apache-2.0 and names SOMA's 30 joints and their parents.

**Judgement calls:**

- Download the Quaternius Standard zip only after the operator has seen the licence conflict, keep
  its licence file beside the clips, and record the download date; if it carries the QAL, do not
  ship its clips in `data/` (CMU, ACCAD and 100STYLE, approved for rung 3, are the fallback, and the
  procedural test clips keep the game animated meanwhile).
- MakeHuman's young-adult macro set (30 MB of text targets) is enough for rung 1's height and build;
  the other ages can follow when the look gains an age.

## Addendum, 2026-11-09: the downloads, and what the Quaternius file itself says

Both downloads were approved by the operator on 2026-11-09 ("Yes. We need at least a male and a
female." for MakeHuman; "Yes, let's see what it has." for Quaternius) and made that day. The text
above is left as written on 2026-11-08.

**MakeHuman.** 100 files, 33,198,970 bytes, fetched from commit
`a8bc2d54ff0ac92e78ff71431b1023eda42bf482` (2024-06-26), each checked against the git blob hash
GitHub reports, into `E:\HumanityOS\makehuman-src\a8bc2d54\` with a `MANIFEST.json` and the fetch
script beside it: `base.obj`, `default.mhskel`, `default_weights.mhw`, `modeling_modifiers.json`
and the 96 young-adult macro targets (female and male, three ancestries, muscle, weight, height,
proportions). MakeHuman makes a female or male body by blending these targets on its one base mesh.

**Quaternius, Universal Animation Library [Standard].** Downloaded through the built-in browser
from https://quaternius.itch.io/universal-animation-library (the free tier, "No thanks, just take
me to the downloads"), 15,904,933 bytes, SHA-256
`cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724`, kept at
`E:\HumanityOS\quaternius-src\ual-standard-2026-11-09\`. The zip's own `License.txt` reads, in
full apart from the author's support links:

> License:
> CC0 1.0 Universal (CC0 1.0)
> Public Domain Dedication
> https://creativecommons.org/publicdomain/zero/1.0/

Its `README.txt` carries the same licence block. So the file obtained today is dedicated CC0 by
the licence text inside it, as the itch.io licence field and the pack page say. The conflict with
https://quaternius.com/license.html (QAL v1.0, "Last updated: 8/28/2026", section 7: "the version
in effect at the time you obtained the Assets governs your use of them") is not removed by this,
but the instrument that travels with these exact bytes is CC0, and a CC0 dedication cannot be
withdrawn from a copy already given under it.

What is inside: `UAL1_Standard.glb` and `UAL1_Standard_RM.glb` (the second with root motion baked
in), each one skinned mesh on a 65-joint rig in Unreal's mannequin names (`root`, `pelvis`,
`spine_01` to `spine_03`, `neck_01`, `Head`, clavicles, arms, five fingers of four joints, `thigh`,
`calf`, `foot`, `ball`), plus FBX copies for Unity. 43 clips: A_TPose, Crouch_Fwd_Loop,
Crouch_Idle_Loop, Dance_Loop, Death01, Driving_Loop, Fixing_Kneeling, Hit_Chest, Hit_Head,
Idle_Loop, Idle_Talking_Loop, Idle_Torch_Loop, Interact, Jog_Fwd_Loop, Jump_Land, Jump_Loop,
Jump_Start, PickUp_Table, Pistol_Aim_Down, Pistol_Aim_Neutral, Pistol_Aim_Up, Pistol_Idle_Loop,
Pistol_Reload, Pistol_Shoot, Punch_Cross, Punch_Jab, Push_Loop, Roll, Sitting_Enter, Sitting_Exit,
Sitting_Idle_Loop, Sitting_Talking_Loop, Spell_Simple_Enter, Spell_Simple_Exit,
Spell_Simple_Idle_Loop, Spell_Simple_Shoot, Sprint_Loop, Swim_Fwd_Loop, Swim_Idle_Loop,
Sword_Attack, Sword_Idle, Walk_Formal_Loop, Walk_Loop. No turn-in-place and no strafing clips in
the free tier (the "120+" counts the paid tiers).

**What it means now:** the unknowns 1 and 2 above are answered for this download. Recommendation:
use these clips as CC0, ship them with `License.txt` beside them and this addendum's date and hash
in the credits, keep the procedural turn-in-place until a CC0 turn clip exists (CMU or 100STYLE at
rung 3), and send Quaternius one polite question about the QAL page so the record is complete
(the operator's message to send, if he wants it).
