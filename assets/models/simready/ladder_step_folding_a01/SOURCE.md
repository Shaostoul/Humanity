# ladder_step_folding_a01

Based on `ladder_step_folding_a01` from NVIDIA SimReady Assets (https://huggingface.co/datasets/nvidia/simready-assets), package
`simready.hf.nvidia.simready-assets.ladder_step_folding_a01.2026.10.10_190040264289`, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

Converted and modified by HumanityOS with `just simready` (scripts/simready/):

- converted from OpenUSD to binary glTF;
- every part joined into one mesh and simplified from 129830 to 10000 triangles, with a 2499-triangle distance version;
- its materials baked into one texture set (base colour 1024 px, normal and ORM 512 px) on a new UV layout;
- moved so its footprint is centred on the origin and its base sits on the floor;
- convex collider hulls computed from its collision parts, and its physics values carried into physics.ron;
- repainted: the AVERKIN (R) maker oval on the decal sheet made transparent and a CE marking painted white (2026-11-08); the maker name ASSURED INDUSTRIES, moulded in orange rubber letters on both treads, painted out of the bake with each tread's own colour (2026-11-09: the letters are geometry, in no texture); the generic safety labels (CAUTION, DANGER, LADDER SAFETY) kept.

The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see
LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,
assets/models/LICENSE-nvidia-simready.md and Settings > Credits.
