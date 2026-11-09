# handTool_screwdriver_phillips_ph2_a01

Based on `handTool_screwdriver_phillips_ph2_a01` from NVIDIA SimReady Assets (https://huggingface.co/datasets/nvidia/simready-assets), package
`simready.hf.nvidia.simready-assets.handTool_screwdriver_phillips_ph2_a01.2026.10.09_052630270290`, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

Converted and modified by HumanityOS with `just simready` (scripts/simready/):

- converted from OpenUSD to binary glTF;
- every part joined into one mesh and simplified from 40266 to 2999 triangles, with a 749-triangle distance version;
- its materials baked into one texture set (base colour 512 px, normal and ORM 512 px) on a new UV layout;
- moved so its footprint is centred on the origin and its base sits on the floor;
- convex collider hulls computed from its collision parts, and its physics values carried into physics.ron;
- repainted: the name VORTEN and its ring mark, moulded into the handle on two sides, painted out of the bake with the handle's own orange (found 2026-11-09 in the baked base colour: the 2026-11-08 texture repaint of the two dark grip inlays had no visible effect, since the inlays and the letters are geometry).

The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see
LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,
assets/models/LICENSE-nvidia-simready.md and Settings > Credits.
