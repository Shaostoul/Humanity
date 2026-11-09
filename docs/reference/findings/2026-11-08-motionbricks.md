# NVIDIA MotionBricks: may HumanityOS use its code, its weights, its method, and which motion data could train our own

**Research date: 8 November 2026** (Pacific time). Every source below was read that day. Each one
carries its own published or last-updated date beside it, which is often older. If you are reading
this later, compare those dates with whatever has changed since: NVIDIA's model agreement, the
repository's licence files and the dataset terms were all published or last changed within the
year before this was written.

**I am not a lawyer. This is a reading of public sources by an AI working on the project, not legal
advice.** Nobody qualified has reviewed it. Licences, model agreements and dataset terms are texts a
court could read differently, and patents were only searched for, not analysed.

The design built on this finding is [`docs/design/character-animation.md`](../../design/character-animation.md).

## The question

May a free, open-source (CC0) project use NVIDIA's MotionBricks (its Apache-2.0 code, its pretrained
weights under the NVIDIA Open Model Agreement, or the published method trained on open motion data),
and on what conditions?

## Short answer

- **The code (Apache-2.0): yes.** Any part may be read, ported to Rust and shipped, on Apache-2.0's
  conditions: a copy of the licence, a notice in every changed file, the copyright and attribution
  notices kept. The repository has no NOTICE file at its root on the research date. One file,
  `motionbricks/animation/ik.py`, also credits Daniel Holden under MIT, whose notice travels with any
  port of it. Ported files stay Apache-2.0 inside the CC0 project (they cannot be relicensed as
  CC0), recorded in `LICENSES.md`.
- **The weights (SafeTensors on Hugging Face, ONNX in the repository): legally usable, but not open
  source by the OSI's measure.** The NVIDIA Open Model Agreement (released 2 April 2026, version of
  9 March 2026) grants a "perpetual, worldwide, non-exclusive, no-charge, royalty-free, irrevocable"
  licence to reproduce, modify, sublicense and distribute the weights and derivatives, says "Works
  are commercially usable", claims no ownership of outputs, and sets **no field-of-use, hardware or
  acceptable-use limits**. Its conditions: give recipients a copy of the agreement, keep the
  notices, and (where a NOTICE file exists) carry "Licensed by NVIDIA Corporation under the NVIDIA
  Open Model Agreement." Its one termination clause ends the licence for anyone who sues claiming
  the model **or its output** infringes a patent or copyright (otherwise the grant is
  "irrevocable", though use outside its conditions is use outside the licence). It grants **no
  patent licence**. It is **not on the OSI's list of approved
  licences**, and the release includes neither the training code nor the training data, which the
  OSI's Open Source AI Definition requires. **Whether that meets the project's "open source only"
  rule for core tech is the operator's decision**; on the OSI's own terms it does not.
- **Shipping them inside the download or fetching them on first run:** both allowed by the agreement
  as written. They are large: the SOMA model is 380 MB at mixed FP16 and 758 MB at FP32 (Git LFS
  pointers), about 200 million parameters.
- **The SOMA body model (SOMA-X):** Apache-2.0 code and Apache-2.0 assets on Hugging Face. Nothing
  read reaches a model's outputs or a game's characters. MotionBricks only needs SOMA's 30-joint
  skeleton layout, and its own copy of that rig is Apache-2.0. SOMA's optional SMPL backends are
  separately licensed and not redistributed.
- **The training data (Bones RigPlay 1):** licensed to NVIDIA "under a negotiated data license"
  that is not public. NVIDIA publishes the weights under its own agreement and says access to the
  dataset "is governed separately from the model weights". Nothing public says the dataset's terms
  reach the weights or what they generate, and nothing public says they do not: the agreement
  disclaims warranties of title and non-infringement, so that residual risk is ours.
- **The bundled sample motion clips (38 BVH, 37 CSV): not usable.** They are under NVIDIA's Sample
  Data Evaluation License: internal use, "solely to evaluate and test NVIDIA technologies", no
  distribution, revocable.
- **TensorRT is not required.** NVIDIA's own ONNX guide runs the model on ONNX Runtime's CPU
  provider and says the ONNX interface works with "another inference library". The agreement names
  no hardware, so DirectML, ONNX Runtime, Rust runtimes (`ort`, `tract`, `burn`, `candle`) or our
  own wgpu compute are all allowed by its terms. TensorRT itself is proprietary and NVIDIA-only, and
  its plan cache is a cache kept across runs (forbidden in development); we would not use it.
- **Implementing the paper ourselves: the method is free to use as far as copyright goes** (17
  U.S.C. 102(b); the paper is also CC BY 4.0 on arXiv). **No patent notice** appears on the project
  page, in the README or in the paper's text. But this is a patented field: Electronic Arts holds
  an active US patent on transformer inbetweening between key frames (US12582906B2, granted 24 March
  2026) and patents on motion matching variants (US11562523B1, US12403400B2). No freedom-to-operate
  search was done; a lawyer is the only way to settle patent risk.
- **Open motion data to train our own:** the usable ones are few. **CMU** (free, may be "copied,
  modified, or redistributed without permission" per its FAQ, though its home page also says not to
  "resell this data directly"), **ACCAD** (CC BY 3.0) and **100STYLE** (CC BY 4.0) fit a CC0
  project with credit. **Not usable** for a model we ship: LAFAN1 (CC BY-NC-ND 4.0), AMASS
  (non-commercial research; forbids training for commercial use), SFU (no commercial products),
  Bandai Namco (CC BY-NC 4.0), AI4Animation's dog data (CC BY-NC 4.0) and **BONES-SEED**, whose
  licence bars training "any generative model whose primary purpose or substantial use is to
  generate motion capture data" and admits only academic institutions and startups under USD
  $1,000,000 revenue. For bodies and starter clips: **MakeHuman's assets are CC0** (its output is
  "yours"), and Quaternius's **Universal Animation Library is CC0**.

## How this was read

GitHub files were read as text through the GitHub REST API (`gh api`), including the 134-byte Git
LFS pointer files that give the ONNX models' sizes; **no model, checkpoint, motion clip or dataset
was downloaded**. Hugging Face model and dataset facts come from its public JSON API and the raw
model card text. The NVIDIA agreement pages, the arXiv HTML paper, the project page, the Bones pages
and the licence pages of datasets were read through an automated web reader that returns requested
passages; where it gave a passage as verbatim, in consecutive segments, it is quoted below joined.
CMU's site failed the reader's certificate check, so its home page and FAQ were read over plain
HTTP (the address it publishes) with `curl` and a plain tool user agent; CMU asks people not to
crawl it, and only those two pages were read. SFU's page was read the same way over HTTPS.
MakeHuman's licence page was gone (404), so its repository's `LICENSE.md` was read instead. The US
Code was read from govinfo.gov.

## Findings

### 1. What MotionBricks is, and which licence covers which part

- Source: https://github.com/NVlabs/motionbricks (repository created 2026-04-21, last pushed
  2026-10-26; `README.md` and `LICENSE` last changed 2026-10-23; `CHANGELOG.md` entry "[0.2.0] -
  2026-10-23").
- README: "MotionBricks is a modular latent generative model that animates characters and robots in
  real time. It fills in the motion between four recent poses and up to four target poses."
- README, License section, quoted item by item (link markup left out):
  - "Code: Apache 2.0, including code examples and snippets in the documentation."
  - "Documentation and media: NVIDIA-authored documentation text and media (images, screenshots,
    and videos, including the files in `docs/media/`) use Creative Commons Attribution 4.0
    International (CC-BY-4.0)."
  - "Models: NVIDIA Open Model Agreement. This covers the SOMA and G1 checkpoints on Hugging Face
    and the ONNX models. It permits commercial use and redistribution of modified models, subject to
    the agreement's terms."
  - "Sample motions: NVIDIA Sample Data Evaluation License. The covered human BVH and retargeted G1
    CSV clips are for evaluating and testing NVIDIA technologies."
- `LICENSE`, first lines: "Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved."
  and "This code is dual-licensed with documentation/skills/assets under the CC-BY-4.0 AND source
  code under Apache-2.0 license terms." The same file says the rig assets
  `soma_base_skel_minimal_no_root.usd`, `g1_model_rig.npz` and `soma_model_rig.npz` are, as to
  NVIDIA's contributions, "licensed under the Apache License, Version 2.0", and that its
  sample-data notice "does not apply the sample license to model weights, ONNX models, robot assets,
  rig metadata, Smart Primitive documents, or scene files."
- `motionbricks/assets/onnx/LICENSE.md` (link markup left out): "The MotionBricks ONNX models in
  this folder are licensed under the NVIDIA Open Model Agreement." and "Licensed by NVIDIA
  Corporation under the NVIDIA Open Model Agreement."
- The repository's top level holds `LICENSE`, `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`,
  `SECURITY.md` and folders; **there is no `NOTICE` file** (GitHub contents listing, 2026-11-08).
- Sizes, from the Git LFS pointers: `mmm_v6_soma_fp16.onnx` "size 380327058",
  `mmm_v6_soma_fp32.onnx` "size 757864273". The manifest `soma_n1.json` gives 30 joints, 30 Hz,
  "context_frames": 4, "target_capacity": 4, ONNX opset 17.
- Hugging Face: the model repository https://huggingface.co/nvidia/MotionBricks (created and last
  modified 2026-10-23, not gated) and its SOMA card,
  https://huggingface.co/nvidia/MotionBricks/blob/main/soma/README.md: "Number of Model Parameters:
  Approximately 200 million across the three components." and "This model is intended for commercial
  and non-commercial motion-generation applications."

### 2. The NVIDIA Open Model Agreement: what it allows

- Source: https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-agreement/
  (page header "Release Date: April 2, 2026"; version line at the end "(v. March 9, 2026)"; a PDF
  copy is linked as `nvidia-open-model-agreement-2026-04-02.pdf`, not opened). The two dates
  differ; the later release date is presumably publication of the March text.
- Introductory bullets: "Works are commercially usable." "You are free to create and distribute
  Derivative Works." "NVIDIA does not claim ownership to any outputs generated using the Works or
  Derivative Works."
- Section 1, "Work": "“Work” shall mean the work of authorship, including machine learning model,
  software, checkpoints, learnt weights, algorithms, parameters, configuration files and
  documentation, made available under the License, as indicated by a copyright notice that is
  included in or attached to the work". "Derivative Works" uses Apache-2.0's wording ("based on (or
  derived from) the Work") and does not mention fine-tuning, distillation or training on outputs.
- Section 2, the grant: "Subject to the terms and conditions of this License, NVIDIA hereby grants
  to You a perpetual, worldwide, non-exclusive, no-charge, royalty-free, irrevocable license to
  reproduce, prepare Derivative Works of, publicly display, publicly perform, sublicense, and
  distribute the Work and such Derivative Works in source or object form."
- Section 3, redistribution, its three conditions:
  - "You must give any other recipients of the Work a copy of this License; and"
  - "You must retain, in the source form of any Derivative Works that You distribute, all
    copyright, patent, trademark, and attribution notices from the source form of the Work,
    excluding those notices that do not pertain to any part of the Derivative Works; and"
  - "If the Work includes a “NOTICE” text file as part of its distribution, then any Derivative
    Works that You distribute must include a readable copy of the following attribution notice
    within a “Notice” text file with such copies and the following statement: “Licensed by NVIDIA
    Corporation under the NVIDIA Open Model Agreement.”"
  - and, after them: "You may add Your own copyright statement to Your modifications and may provide
    additional or different license terms and conditions for use, reproduction, or distribution of
    Your modifications, or for any such Derivative Works as a whole, provided Your use,
    reproduction, and distribution of the Work otherwise complies with the conditions stated in this
    License."
- Section 4: "This License does not grant permission to use the trade names, trademarks, service
  marks, or product names of NVIDIA, except as required for reasonable and customary use in
  describing the origin of the Work and reproducing the content of the NOTICE file."
- Section 5 disclaims "any warranties or conditions of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or
  FITNESS FOR A PARTICULAR PURPOSE".
- Section headings, in full: 1 Definitions, 2 Grant of License, 3 Redistribution, 4 Trademarks, 5
  Disclaimer of Warranty, 6 Limitation of Liability, 7 Accepting Warranty or Additional Liability, 8
  Feedback, 9 Governing Law (United States and Delaware; courts in Santa Clara County, California),
  10 Trade and Compliance.

### 3. The NVIDIA Open Model Agreement: what it forbids, and how it ends

- Same source. **No patent licence**: Section 2 grants copyright permissions only. The one patent
  sentence is a termination: "If You institute patent or copyright litigation against any entity
  (including a cross-claim or counterclaim in a lawsuit) alleging that the Work or an output from
  the Work constitutes direct or contributory patent or copyright infringement, then any licenses
  granted to You under this License for that Work shall terminate as of the date such litigation is
  filed." This is broader than Apache-2.0's (which ends only patent licences, on patent suits): it
  covers copyright suits and suits about outputs too.
- **No field-of-use or acceptable-use limits.** A word search of the page for "Trustworthy",
  "guardrail", "acceptable use", "ethic" and "prohibited" found none of them in the agreement (the
  two hits for "safety" were the site's navigation menu). The one restriction found is Section 10:
  "You agree to comply with all applicable export, import, trade and economic sanctions laws and
  regulations, as amended" and "These laws include restrictions on destinations, end-users and
  end-use."
- **Not to be confused with the older NVIDIA Open Model License** (a different document,
  https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/, "Last
  Modified" and "Version Release Date" October 24, 2025), which does have such limits: "If You
  bypass, disable, reduce the efficacy of, or circumvent any technical limitation, safety
  guardrail" ... "your rights under this Agreement will automatically terminate", and "Use of the
  Models under the Agreement must be consistent with NVIDIA's Trustworthy AI terms." MotionBricks
  names the 2026 Agreement, not this licence.
- The MotionBricks model cards add no use restrictions of their own: under "Use Case Restrictions"
  they point back to the agreement.

### 4. Is it open source?

- OSI list, https://opensource.org/licenses (no date on the page; "Open source licenses are licenses
  that comply with the Open Source Definition."): no licence with "NVIDIA" in its name is listed.
  Apache-2.0 and MIT are listed; CC0 is not.
- OSI's Open Source AI Definition, https://opensource.org/ai/open-source-ai-definition ("Version
  1.0"; no date on the page): an Open Source AI must be available under terms that grant the
  freedoms to "Use the system for any purpose and without having to ask for permission.", "Study how
  the system works and inspect its components.", "Modify the system for any purpose, including to
  change its output." and "Share the system for others to use with or without modifications, for any
  purpose." Its preferred form includes "The complete source code used to train and run the system."
  and "Parameters shall be made available under OSI-approved terms."
- What MotionBricks releases: the Hugging Face card says "this repository does not include a
  training pipeline or the full training data", and the SOMA card "A training pipeline is not
  included." and "This package does not redistribute the training corpus."
- So by the OSI's two published measures, the weights are not open source: the agreement is not an
  OSI-approved licence, and the training code and data are absent. The agreement's permissions
  read as close to Apache-2.0's, minus the patent grant; whether it would pass the Open Source
  Definition has not been tested by OSI review (judgement, not a finding).

### 5. The training data: Bones RigPlay 1, and BONES-SEED

- SOMA model card, https://huggingface.co/nvidia/MotionBricks/blob/main/soma/README.md (release
  "Hugging Face 10/23/2026"): "Training used RP1 skeletal motion collected by Bones and supplied to
  NVIDIA under a negotiated data license." and "Access to the dataset is governed separately from
  the model weights." Training subset: "313,526 human-motion clips"; source corpus "approximately
  350,000 clips and 700 hours of motion".
- Bones, https://bones.studio/datasets (no page date; footer "2016 - 2026"): RP1 is "700+ hours of
  labeled and annotated human 3D animations" by "170+ physically diverse performers"; "When you
  license BONES data, you get full IP clearance." No licence text or price is published; enquiries go
  to contact@bones.studio.
- The project page, https://nvlabs.github.io/motionbricks/ (no date), calls BONES-SEED "MotionBricks'
  training corpus — 350k production-grade mocap clips", while the paper calls BONES-SEED "an
  open-source subset of approximately 140k motion clips" and the model card names RP1. **The sources
  disagree on the name**; the model card, as the document about this exact checkpoint, is the more
  credible (judgement).
- BONES-SEED, https://huggingface.co/datasets/bones-studio/seed (created 2026-03-10, last modified
  2026-05-03, gated "auto"), licence "bones-seed-license" at https://bones.studio/info/seed-license
  (title "BONES-SEED Dataset License Agreement"; no version or date found):
  - Who may use it: an academic user, "A Licensee who is a non-profit academic institution (such as
    a university or research institute)", with use "strictly limited to non-commercial scientific
    research, evaluation, and educational purposes"; or a qualifying startup that "has an annual
    gross revenue of less than $1,000,000 USD." Otherwise: "If you do not qualify as (i) an Academic
    User or (ii) a Qualifying Startup, then you are NOT granted any rights under this Agreement".
  - Generative models: Section 6, "No Competing Datasets & Generative Models", bars using the data
    to train, fine-tune or condition "any generative model whose primary purpose or substantial use
    is to generate motion capture data". Its carve-out, "For the avoidance of doubt, this restriction
    does not prohibit training machine learning models", covers models whose outputs are not data,
    such as robot control policies and motion tracking (reader's paraphrase of the rest of that
    sentence).
  - Results: "a trained machine learning model is owned by the Licensee", "This permission is valid
    solely provided that the raw Dataset itself cannot be extracted", and "ownership of Results does
    not grant any right to redistribute the Dataset or its data."
- **What reaches the weights:** nothing public. NVIDIA is the licensor of the weights; the RP1 terms
  are a contract between Bones and NVIDIA that we cannot read.

### 6. The sample clips in the repository: evaluation only

- Source: `assets/motions/LICENSE.txt`, "NVIDIA SAMPLE DATA EVALUATION LICENSE" ("(v. Jan. 16,
  2026)"), and `assets/motions/README.md` ("The 38 BVH files and 37 CSV files listed below").
- Section 1: a "limited, non-exclusive, revocable, non-transferable, non-sublicensable license to
  internally (a) use and reproduce the Sample Dataset, and (b) modify and create derivative works of
  the Sample Dataset (“Derivative Works”), in both cases solely to evaluate and test NVIDIA
  technologies (“Purpose”)".
- Section 2(c): you may not "sell, rent, sublicense, transfer, distribute, offer or otherwise make
  the Sample Dataset and Derivative Works available to others".
- Section 6: "NVIDIA may terminate this Agreement at any time with prior written notice."
- The clips are walks, jogs, crouches, a chair sit, two jump heights and style walks. The Smart
  Primitive files carry the same categories' names, and the README says the editors "sample a
  handful of target poses from clips". **The Smart Primitive JSON files themselves are not covered by
  this licence** (the LICENSE says so), but poses we sampled from these clips into anything of ours
  would be "Derivative Works" under it (judgement), so we would author our own targets.

### 7. The SOMA body model

- Source: https://github.com/NVlabs/SOMA-X (created 2026-03-03, last pushed 2026-11-07; GitHub
  identifies the licence as Apache-2.0) and its README (link markup left out): "SOMA-X is licensed
  under Apache-2.0. Optional third-party models and dependencies retain their own license terms." Assets download from https://huggingface.co/nvidia/SOMA-X (licence field "apache-2.0",
  last modified 2026-11-06).
- `docs/data_assets.md`: "The SMPL / SMPL-X identity models are **not redistributed** with SOMA."
  The default body backend, MHR (https://github.com/facebookresearch/MHR), is Apache-2.0 per GitHub.
- Apache-2.0 has no clause about outputs, and nothing read in SOMA-X claims rights in poses,
  animations or characters made with it. MotionBricks itself needs only the joint layout (30 joints,
  named in `soma_n1.json`) and ships its own Apache-2.0 copy of the rig (finding 1).

### 8. TensorRT, and other ways to run the ONNX files

- MotionBricks README: "The runtime uses TensorRT, which requires an NVIDIA GPU." and "The first
  launch of each model and precision builds a TensorRT plan. This takes a few minutes. Plans are
  cached in `.trt-cache/`, so later launches start quickly." It also says the TensorRT OSS notice
  in `licenses/` does not apply to the runtime packages, which "are governed by the NVIDIA TensorRT
  Software License Agreement distributed with the packages" (not read).
- `docs/onnx_inference.md` (same repository): "This page is for running the MotionBricks ONNX models
  without the bundled engines, such as in another inference library". Its example opens the model
  with `providers=["CPUExecutionProvider"]` in ONNX Runtime, and its timings note that "every timed
  plan matched ONNX Runtime on the same request."
- Its measured GPU time for the TensorRT plan alone, on an RTX 4090 under Windows with TensorRT
  10.8: SOMA FP32 "2.50 ms (400)" requests a second at N = 1, mixed FP16 "1.28 ms (778)"; at N = 32,
  FP32 "5.28 ms (6,063)" and FP16 "3.93 ms (8,138)". The SOMA ONNX card: "Timings are reported only
  for the GeForce RTX 4090 under Windows." and "A CPU-versus-GPU inference comparison is not
  provided."
- Each generation covers "6 to 16 tokens of four poses, or about 0.6 to 1.9 s of motion" at 30 Hz.
- The agreement (findings 2 and 3) names no hardware or runtime, so running the files on any vendor's
  GPU or CPU is within its terms.
- Runtimes, licences per GitHub's metadata on 2026-11-08: ONNX Runtime (microsoft/onnxruntime) MIT;
  `ort` (pykeio/ort, Rust bindings to ONNX Runtime) Apache-2.0; `burn` (tracel-ai/burn, has a wgpu
  backend) and `candle` (huggingface/candle) shown as Apache-2.0; `tract` (sonos/tract) not
  identified by GitHub; `wonnx` (a wgpu ONNX runtime) **archived**; DirectML (microsoft/DirectML) MIT,
  and its description begins "DirectML is in maintenance mode".

### 9. Porting the Apache-2.0 code

- Apache-2.0 (January 2004), as reproduced in the repository's `LICENSE`, Section 4: "(a) You must
  give any other recipients of the Work or Derivative Works a copy of this License; and (b) You must
  cause any modified files to carry prominent notices stating that You changed the files; and (c)
  You must retain, in the Source form of any Derivative Works that You distribute, all copyright,
  patent, trademark, and attribution notices from the Source form of the Work, excluding those
  notices that do not pertain to any part of the Derivative Works". Condition (d) applies only "If
  the Work includes a "NOTICE" text file", and there is none (finding 1).
- Section 3 grants a patent licence only for "those patent claims licensable by such Contributor
  that are necessarily infringed by their Contribution(s) alone or by combination of their
  Contribution(s) with the Work". It covers NVIDIA's code; whether it reaches a Rust port, and it
  certainly does not reach an independent implementation of the method, is a judgement a lawyer
  would have to confirm.
- `licenses/Daniel-Holden-MIT-LICENSE.txt` ("Copyright (c) 2021 Daniel Holden"), credited by
  `motionbricks/animation/ik.py` (GitHub code search for "Holden"): "The above copyright notice and
  this permission notice shall be included in all copies or substantial portions of the Software."
- What the code is (repository tree, 2026-11-08): a Python runtime (engines, an animation buffer,
  foot contacts, IK, skeleton), demos, a character state machine, target pose blenders, a Smart
  Primitive document format and two editors (`app/runtime/common/*`, `app/authoring/*`,
  `motionbricks/animation/*`).

### 10. Implementing the paper's method ourselves

- Copyright: 17 U.S.C. 102(b), read in the 2023 edition of the US Code at govinfo.gov
  (https://www.govinfo.gov/content/pkg/USCODE-2023-title17/html/USCODE-2023-title17-chap1-sec102.htm;
  enacted 1976, amended 1990): "(b) In no case does copyright protection for an original work of
  authorship extend to any idea, procedure, process, system, method of operation, concept,
  principle, or discovery, regardless of the form in which it is described, explained, illustrated,
  or embodied in such work."
- The paper: https://arxiv.org/abs/2604.24833 (v1, "Mon, 27 Apr 2026"; comment "ACM Transactions on
  Graphics; SIGGRAPH 2026"), posted under CC BY 4.0 per its arXiv page. Abstract: MotionBricks
  models "a dataset of over 350,000 motion clips with a single model" and reaches "a real-time
  throughput of 15,000 FPS with 2ms latency". The HTML text reports the 2 ms on an RTX 5090 and 5 ms
  on the robot's Jetson Orin, and component sizes of about 23.5M (tokenizer), 150M (pose module) and
  50M (root module) parameters (reader's figures from the paper body).
- Patent notices: the word "patent" does not appear in the paper's HTML text (both halves read), on
  the project page, or in the README. The project page's only licence line: "The website source is
  licensed under a Creative Commons Attribution-ShareAlike 4.0 International License."
- Patents found in the field (searched, not analysed; none is NVIDIA's):
  - **US12582906B2**, "System for generating animation within a virtual environment", Electronic
    Arts Inc., filed 2024-03-29, granted 2026-03-24, active (Google Patents estimates expiry
    2044-06-24), https://patents.google.com/patent/US12582906B2/en. Abstract: "A transformer-based
    model can be used to generate intermediate poses for an animation based on a set of key frames."
    Claim 1 opens "A computer-implemented method for generating animation of a virtual entity within
    a virtual environment, the method comprising:" and, in the reader's paraphrase, generates the
    poses between two frames iteratively, alternating between the pose after the first and the pose
    before the second.
  - **US11562523B1**, "Enhanced animation generation based on motion matching using local bone
    phases", Electronic Arts Inc., filed 2021-08-02, granted 2023-01-24, active (estimated expiry
    2041-08-02), https://patents.google.com/patent/US11562523B1/en.
  - **US12403400B2**, "Learning character motion alignment with periodic autoencoders", Electronic
    Arts (inventors Starke and Chaput), filed 2022-03-31, per the USPTO Official Gazette of
    2025-09-02 (https://patentsgazette.uspto.gov/week35/OG/html/1538-1/US12403400-20250902.html,
    via a search result; the gazette page itself was not opened).
  - No Ubisoft patent on motion matching or learned motion matching surfaced in two searches.

### 11. Motion data we could train on, or ship

Each line: the source, its date, and the words that carry the licence.

- **CMU Graphics Lab Motion Capture Database**, http://mocap.cs.cmu.edu/ (no visible date; an HTML
  comment reads "Last updated by Bonnie Jang - December 2002"). Home page: "This data is free for use
  in research projects. You may include this data in commercially-sold products, but you may not
  resell this data directly, even in converted form." and the requested credit, "The data used in
  this project was obtained from mocap.cs.cmu.edu." FAQ, http://mocap.cs.cmu.edu/faqs.php: "The
  motion capture data may be copied, modified, or redistributed without permission." **The two
  pages differ** on reselling; for a free project that sells nothing, both allow use (judgement).
  Capture: "a Vicon motion capture system consisting of 12 infrared MX-40 cameras", 120 Hz.
- **ACCAD (Ohio State) Open Motion Project**,
  https://accad.osu.edu/research/motion-lab/mocap-system-and-data (footer "© 2026"): "Open Motion
  Project by ACCAD/The Ohio State University is licensed under a Creative Commons Attribution 3.0
  Unported License." Walking, running, martial arts, dance; BVH, C3D, AMC.
- **100STYLE**, https://zenodo.org/records/8127870 (published May 4, 2022, v1): licence "Creative
  Commons Attribution 4.0 International"; "over four million frames of stylized motion capture data".
- **LAFAN1** (Ubisoft La Forge), https://github.com/ubisoft/ubisoft-laforge-animation-dataset
  (created 2020-04-14, last pushed 2022-09-10): "Creative Commons Attribution-NonCommercial-
  NoDerivatives 4.0 International Public License"; 5 subjects, 77 sequences, about 4.6 hours. Not
  usable for anything we ship.
- **AMASS**, https://amass.is.tue.mpg.de/license.html ("© 2020 Max-Planck-Gesellschaft"): "To use the
  Dataset for the sole purpose of performing non-commercial scientific research"; "Any other use, in
  particular any use for commercial purposes, is prohibited."; it "prohibits the use of the Dataset
  to train methods/algorithms/neural networks/etc. for commercial use". Not usable (and HumanML3D,
  built from AMASS, inherits that).
- **SFU Motion Capture Database**, https://mocap.cs.sfu.ca/ (no date): "This dataset of motions is
  free for research purposes. The data cannot be used for commercial products or resale,
  unfortunately." Not usable.
- **Bandai Namco Research Motion dataset**,
  https://github.com/BandaiNamcoResearchInc/Bandai-Namco-Research-Motiondataset (created 2022-04-27,
  last pushed 2023-07-04): both datasets "CC BY-NC 4.0". Not usable.
- **AI4Animation** (including the quadruped dog capture), https://github.com/sebastianstarke/AI4Animation
  (last pushed 2026-04-17): "This project is only for research or education purposes, and not freely
  available for commercial use or redistribution. The motion capture data is available only under
  the terms of the Attribution-NonCommercial 4.0 International (CC BY-NC 4.0) license." (link
  markup left out). Not usable.
- **BONES-SEED**: finding 5. Not usable for a generative motion model.
- **Quaternius Universal Animation Library**,
  https://quaternius.com/packs/universalanimationlibrary.html ("March 2025"): CC0, "Free to use in
  personal, educational and commercial projects."; "120+ Animations" on "a universal humanoid rig",
  FBX, glTF and Blend. Whether the clips are captured or keyed is not stated.
- **MakeHuman** (bodies, not motion), https://github.com/makehumancommunity/makehuman, `LICENSE.md`
  (last changed 2020-09-26): the source code is AGPL, while "These assets have been released under
  CC0 1.0 Universal" (the base mesh, targets and modifiers, textures, clothes, poses), and on
  output: "the MakeHuman project makes no claim whatsoever over output such as: * Exports to files
  (FBX, OBJ, DAE, MHX2...)". Its `default.mhskel` reads "license": "CC0", 163 bones.
- Reference code: Daniel Holden's Motion Matching and Learned Motion Matching examples,
  https://github.com/orangeduck/Motion-Matching (MIT, created 2021-10-19), whose README warns that
  its data comes from LAFAN1, "licensed under Creative Commons Attribution-NonCommercial-NoDerivatives
  4.0 International Public License (unlike the code, which is licensed under MIT)".

### 12. The public methods the alternatives rest on

- Motion matching: Simon Clavet (Ubisoft Montreal), "Motion Matching and The Road to Next-Gen
  Animation", GDC 2016, https://www.gdcvault.com/play/1023280/Motion-Matching-and-The-Road: "At
  runtime, they continuously find the frame in the mocap database that simultaneously matches the
  current pose and the desired future plan, and transition with a small blend time to this other
  place in the data."
- Learned motion matching: Ubisoft La Forge, 8 July 2020,
  https://www.ubisoft.com/en-us/studio/laforge/news/3NWqwcMU9lMfumgfPAEfka/learned-motion-matching:
  "Our final model has no need to store animation data or additional matching meta-data in memory".
  No memory or timing figures are on that page or on Holden's article page.
- glTF skinning: Khronos glTF 2.0 specification ("version 2.0.1, 2021-10-11"), source
  `specification/2.0/Specification.adoc` on GitHub: "Each skin contains a **REQUIRED** `joints`
  property that lists the indices of nodes used as joints and an **OPTIONAL** `inverseBindMatrices`
  property"; "The `JOINTS_n` attributes provide indices of the joints that influence each vertex and
  `WEIGHTS_n` attributes provide weights of those joints".

## What is still unknown, and what would settle it

1. **Whether the Bones RP1 licence to NVIDIA limits what downstream users may do with the weights or
   their output.** Settled by a written statement from NVIDIA (the repository's issues, or the
   authors) or from Bones (contact@bones.studio). Cost: one email each.
2. **Whether the NVIDIA Open Model Agreement would pass the OSI's Open Source Definition.** Settled
   only by OSI licence review, which NVIDIA would have to request. For the project's own rule, the
   operator's decision settles it.
3. **Whether the Apache-2.0 patent grant (Section 3) covers a Rust port of NVIDIA's code, and whether
   any NVIDIA patent covers the method.** Settled by asking NVIDIA whether patents cover
   MotionBricks, and by a patent search of NVIDIA's filings since 2025 (applications publish 18
   months after filing, so a filing made in 2025 might not be visible yet). Cost: a lawyer.
4. **Whether an independent implementation (ours) would read on EA's US12582906B2 or its motion
   matching patents.** Settled only by a freedom-to-operate opinion from a patent lawyer. A design
   that does not generate inbetween poses by alternating from both ends is at least a different
   process from claim 1 as paraphrased; that is not a legal conclusion.
5. **The TensorRT runtime licence.** Not read, and moot if TensorRT is never used.
6. **AIST++ dance data** (Google): its licence was not found on the pages reached. Settled by reading
   its download page.

## Sources I could not reach

- `nvidia-open-model-agreement-2026-04-02.pdf` (the agreement's PDF copy): not opened; the HTML page
  was read instead. Would confirm the text matches.
- https://www.theorangeduck.com/media/uploads/other_stuff/Learned_Motion_Matching.pdf: larger than the
  reader's 10 MB limit. Wanted: the paper's memory and timing figures.
- https://google.github.io/aistplusplus_dataset/factsheet.html: 404. Wanted: AIST++'s licence.
- http://www.makehumancommunity.org/content/license_explanation.html: 404 over HTTP and a certificate
  mismatch over HTTPS; the repository's `LICENSE.md` was read instead.
- https://patentsgazette.uspto.gov/week35/OG/html/1538-1/US12403400-20250902.html: seen only as a
  search result.
- The NVIDIA TensorRT Software License Agreement: not looked up (not needed for the conclusion).

## What it means for this project

**Certain** (from the quotations above):

- MotionBricks' code can be ported to Rust under Apache-2.0's notice conditions; the ported files stay
  Apache-2.0 and go in `LICENSES.md`.
- The weights may be shipped, fetched, modified, fine-tuned, quantized or converted, and run on any
  hardware, with the agreement's copy and notices passed on. They are not under an OSI-approved
  licence, carry no patent licence, and come with no training code or data.
- The bundled sample clips cannot be used.
- MotionBricks' TensorRT runtime would break two house rules at once (proprietary, NVIDIA-only; and
  a plan cache kept across runs), and nothing obliges us to use it.
- The method may be reimplemented as far as copyright is concerned; patents in the field exist and
  were not analysed.
- Only CMU, ACCAD and 100STYLE among the motion datasets read are usable for a model or a database
  we ship, with credit for ACCAD and 100STYLE; MakeHuman (bodies) and Quaternius (clips) are CC0.

**Judgement calls** (accept the facts above and reject these if you disagree):

- The agreement reads as permissive in practice, close to Apache-2.0 without the patent grant, and
  its broad termination (a copyright or patent suit about the model or its output ends the licence)
  is unlikely to matter to a project that does not sue anyone.
- The operator's rule is "We want to keep everything on open-source"
  (`docs/design/fire-props-and-combustion.md`, section 6, decision 3). The weights do not meet the
  OSI's definition, so this finding would not put them in the core of the game; at most an optional
  pack the person chooses, with its licence shown. **That is the operator's decision.**
- The open motion data available is a few to perhaps tens of hours, against MotionBricks' 700. A
  model of MotionBricks' class trained only on it would not match NVIDIA's breadth, which is why the
  design leads with motion matching (which works with less data) and keeps a learned rung for later.
- The most valuable thing MotionBricks gives a project like ours is its **interface**: the
  character's recent poses, plus target poses placed relative to objects, plus root constraints,
  handed to whatever solver is behind it. An interface is an idea (17 U.S.C. 102(b)) and can be
  adopted without the weights.

**I am not a lawyer, and this is not legal advice.** A lawyer should read this before any NVIDIA
weights ship with HumanityOS, and before any learned inbetweening model of our own does.
