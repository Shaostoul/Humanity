# fireExtinguisher_abc_a03

Based on `fireExtinguisher_abc_a03` from NVIDIA SimReady Assets (https://huggingface.co/datasets/nvidia/simready-assets), package
`simready.hf.nvidia.simready-assets.fireExtinguisher_abc_a03.2026.10.09_031516888509`, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

Converted and modified by HumanityOS with `just simready` (scripts/simready/):

- converted from OpenUSD to binary glTF;
- every part joined into one mesh and simplified from 108942 to 10000 triangles, with a 2499-triangle distance version;
- its materials baked into one texture set (base colour 1024 px, normal and ORM 512 px) on a new UV layout;
- moved so its footprint is centred on the origin and its base sits on the floor;
- convex collider hulls computed from its collision parts, and its physics values carried into physics.ron;
- repainted: on the label sheet: the BENITEZ logo and both BENITEZ CORP. address blocks painted out, and two certification marks on the gauge face (a UL-style cRUus mark and a T-in-triangle mark); the generic instructions, ABC class letters and pictograms kept.

The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see
LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,
assets/models/LICENSE-nvidia-simready.md and Settings > Credits.
