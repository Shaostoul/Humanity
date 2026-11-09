# table_metal_stainless_a03

Based on `table_metal_stainless_a03` from NVIDIA SimReady Assets (https://huggingface.co/datasets/nvidia/simready-assets), package
`simready.hf.nvidia.simready-assets.table_metal_stainless_a03.2026.10.09_174629469614`, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

Converted and modified by HumanityOS with `just simready` (scripts/simready/):

- converted from OpenUSD to binary glTF;
- every part joined into one mesh and simplified from 13912 to 13912 triangles, with a 3478-triangle distance version;
- its materials baked into one texture set (base colour 1024 px, normal and ORM 1024 px) on a new UV layout;
- moved so its footprint is centred on the origin and its base sits on the floor;
- convex collider hulls computed from its collision parts, and its physics values carried into physics.ron.

The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see
LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,
assets/models/LICENSE-nvidia-simready.md and Settings > Credits.
