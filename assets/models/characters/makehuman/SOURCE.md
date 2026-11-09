# MakeHuman bodies: where they come from

Made by `scripts/gen-makehuman-body.js` (HumanityOS, character animation rung 1, 2026-11-09) from
MakeHuman's data files. Do not edit these files by hand: change the converter and run it again.

## Licence

The MakeHuman assets these are made from are dedicated to the public domain under **CC0 1.0
Universal** (https://creativecommons.org/publicdomain/zero/1.0/). MakeHuman's `LICENSE.md`,
section C: "These assets have been released under CC0 1.0 Universal."; `default.mhskel` carries
`"license": "CC0"`. MakeHuman's source code is AGPL and none of it is used here: the converter
reads the data files' formats itself. Finding:
`docs/reference/findings/2026-11-08-character-assets.md` (with its 2026-11-09 addendum).

## Source

- Repository: https://github.com/makehumancommunity/makehuman
- Commit: `a8bc2d54ff0ac92e78ff71431b1023eda42bf482` (2024-06-26)
- Fetched: 2026-11-09, approved by the operator, 2026-11-09 ("Yes. We need at least a male and a female.")
- Files read (under `makehuman/data/`), each checked against its git blob SHA-1 before reading:

  - `3dobjs/base.obj` `d26635e9326e3cca30778fd7b9c00062b03cce09`
  - `rigs/default.mhskel` `b02cbecae00143856410d7561adf006d83bf9b3e`
  - `rigs/default_weights.mhw` `66185806afacf87749be8e5cf6dbf66496b2a6b0`
  - `targets/macrodetails/african-female-young.target` `a7d2b130667cc81ecd1ca0598da9be27bec1864a`
  - `targets/macrodetails/african-male-young.target` `dd5743e48700267d76596f575bf17b4b5cc3b3e0`
  - `targets/macrodetails/asian-female-young.target` `f862c96add11c31ab1ca75c69e1c67ae5e0e3d5f`
  - `targets/macrodetails/asian-male-young.target` `8cd3ef3e9ddb06ee2d166f75f9ac10871251f938`
  - `targets/macrodetails/caucasian-female-young.target` `9d1f0cbeedc9a6a51abe33f1ebb5fa7c5a7edbf1`
  - `targets/macrodetails/caucasian-male-young.target` `c3b82f92c5ced85599199cd184b0faf3b3fc6881`
  - `targets/macrodetails/universal-female-young-averagemuscle-maxweight.target` `e0b442f75e36ad0980cd7ccda41b6b8d67b9f200`
  - `targets/macrodetails/universal-female-young-averagemuscle-minweight.target` `7b4dfc290b1228f48f00e7b121ffe8932a7bbb75`
  - `targets/macrodetails/universal-female-young-maxmuscle-averageweight.target` `91ca14d335389e4e4a964132fc6580ada688cb64`
  - `targets/macrodetails/universal-female-young-maxmuscle-maxweight.target` `c3f7a80bfa534dc8723deff98d045c9f99679ccd`
  - `targets/macrodetails/universal-female-young-maxmuscle-minweight.target` `7eb470715d485b9f21733dd7142f0cd0369638ac`
  - `targets/macrodetails/universal-female-young-minmuscle-averageweight.target` `8d3c861b41dda1167fac41eae3dd6458cc28e8c8`
  - `targets/macrodetails/universal-female-young-minmuscle-maxweight.target` `a4317e0dd09303df9034d1fe1906d49d98cdcfa8`
  - `targets/macrodetails/universal-female-young-minmuscle-minweight.target` `da95e85f2ab24a32c2d2ae037254325b2f10fec3`
  - `targets/macrodetails/universal-male-young-averagemuscle-maxweight.target` `15d495149eec197fed9ab8dda745cdfd4878ab2b`
  - `targets/macrodetails/universal-male-young-averagemuscle-minweight.target` `0a53c15a2d6cf23120c786d95b36d7a618c61acf`
  - `targets/macrodetails/universal-male-young-maxmuscle-averageweight.target` `b7c039c7f463fe1f1721b9ab269c7248d9c4f2f7`
  - `targets/macrodetails/universal-male-young-maxmuscle-maxweight.target` `a0f12e4b0c0f84f2a6b90badfaf252a125e9e3d1`
  - `targets/macrodetails/universal-male-young-maxmuscle-minweight.target` `6b21e86b3800e7bd736b8d9aa98cd98040acd334`
  - `targets/macrodetails/universal-male-young-minmuscle-averageweight.target` `2b585bb27c88a0ef63972f0b2d480d61850c5889`
  - `targets/macrodetails/universal-male-young-minmuscle-maxweight.target` `42d9f8ca69fe7bdc40429efc7c515e08e90b7a20`
  - `targets/macrodetails/universal-male-young-minmuscle-minweight.target` `e63021e74918cff0f60b65e2d134ce6f256a3cb2`
  - `targets/macrodetails/height/female-young-averagemuscle-averageweight-maxheight.target` `1db1440ae3d7ad4259d64991047093f450fb38b0`
  - `targets/macrodetails/height/female-young-averagemuscle-averageweight-minheight.target` `864d9eea661dab7e37177957f452d622f20a5c0a`
  - `targets/macrodetails/height/male-young-averagemuscle-averageweight-maxheight.target` `1db1440ae3d7ad4259d64991047093f450fb38b0`
  - `targets/macrodetails/height/male-young-averagemuscle-averageweight-minheight.target` `7208f5dcf183c0cdba3eaa1e2da8cf21e7726b84`
  - `targets/macrodetails/proportions/female-young-averagemuscle-averageweight-idealproportions.target` `145e69247246510dd0a0e5a81a63bed2f6e5090a`
  - `targets/macrodetails/proportions/female-young-averagemuscle-averageweight-uncommonproportions.target` `28d0cd73e8927c828844bafcb6b5c570c27e8c90`
  - `targets/macrodetails/proportions/male-young-averagemuscle-averageweight-idealproportions.target` `c0a95b13bd0d2a78b025da7502f7b5653f917904`
  - `targets/macrodetails/proportions/male-young-averagemuscle-averageweight-uncommonproportions.target` `4adadda4ad9c78086f309a47e2ece30670097a8c`

## What was made

- `makehuman_female`: 15479 vertices, 27860 triangles, 56 joints, 15 shape targets, 1.590 m tall
- `makehuman_male`: 15479 vertices, 27860 triangles, 56 joints, 15 shape targets, 1.730 m tall

## Output hashes (SHA-256)

`node scripts/gen-makehuman-body.js --check` compares the files with these when the sources are
not on the machine.

- `makehuman_female.gltf` `7f53277c8c12e4c7a7b20f80e9d329ebe4dde4efd109c801e5a6b0be3cdc2e3c`
- `makehuman_female.bin` `5e63a1883a96b120bb5e291ddc7bc673a3fc17990fac3835b8f73c0cf9520ff0`
- `makehuman_male.gltf` `489277eb1e179ce6806b9765a3c7c83d3d2cb7cf7d3cd8c6bbca83a557a93857`
- `makehuman_male.bin` `995063548f5f244a0bb5cffbe14bf55dd80dced7f5dd998452bf0712672f7fcb`
