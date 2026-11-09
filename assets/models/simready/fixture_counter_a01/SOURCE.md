# fixture_counter_a01

Based on `fixture_counter_a01` from NVIDIA SimReady Assets (https://huggingface.co/datasets/nvidia/simready-assets), package
`simready.hf.nvidia.simready-assets.fixture_counter_a01.2026.10.10_185802539552`, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

Converted and modified by HumanityOS with `just simready` (scripts/simready/):

- converted from OpenUSD to binary glTF;
- every part joined into one mesh and simplified from 659420 to 25000 triangles, with a 6250-triangle distance version;
- its materials baked into one texture set (base colour 1024 px, normal and ORM 1024 px) on a new UV layout;
- moved so its footprint is centred on the origin and its base sits on the floor, and turned 90 degrees about the vertical;
- convex collider hulls computed from its collision parts, and its physics values carried into physics.ron.

The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see
LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,
assets/models/LICENSE-nvidia-simready.md and Settings > Credits.
