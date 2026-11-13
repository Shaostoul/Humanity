# Temporal anti-aliasing and upscalers for the renderer: a scan

**Read on 12 November 2026, Pacific evening** (local clock: Thu Nov 12 22:30 PST 2026), by an AI agent
working for the project. **Not legal advice; the agent is not a lawyer.** The licence lines below are
quotations, so a reader can check them against the source; what they mean for a release is a
judgement, marked as one.

How it was read: github.com's robots.txt allows repository pages to Claude's readers (it disallows
`/*/raw/` and `/*/tree/`, not repository pages), so repository pages were read through a fetch tool that
identifies itself, and repository files (licence files, documentation pages) through the GitHub API
with the `gh` command line tool. learn.microsoft.com's robots.txt has no rule against its
documentation pages. The wgpu facts were read from the wgpu 24 source the project already builds
against (the cargo registry copy). **Nothing was downloaded**: no archive, binary, model or package.

Asked for by the renderer roadmap's brief (`docs/design/renderer-roadmap.md`, section 3) under the
standing rule: "This, research better alternatives, is something we want to consider every time we
learn of new tools." (operator, 2026-11-08).

## 1. What the job needs

- A temporal anti-aliasing resolve (and then temporal upscaling) for a forward renderer written in
  WGSL on wgpu 24, running on DirectX 12 on Windows (`src/renderer/mod.rs` picks `Backends::DX12`).
- It must run on the project's floor card, an RX 580 (Polaris), as well as the operator's RTX 4070.
- CLAUDE.md: "Open source only, and build what does not exist", and no proprietary licences for
  core technology. The project itself is CC0 (`LICENSE`); third-party notices live in
  `LICENSES.md` and `data/credits.ron`.

## 2. The candidates

| Candidate | What it is | Licence (quoted in section 5) | Hardware | Fit here |
|---|---|---|---|---|
| **Our own TAA, then TAAU** | Written in WGSL from the published methods (Karis 2014, Salvi 2016, Pedersen 2016, the Yang, Liu and Salvi 2020 survey) | ours (CC0) | any card wgpu runs on (compute shaders and float storage textures are core) | built for this engine's passes, depth spaces and dev switches |
| Bevy's TAA | Rust engine on wgpu; a TAA module (`crates/bevy_anti_alias/src/taa`) beside FXAA, SMAA, contrast-adaptive sharpening and a DLSS module | MIT or Apache-2.0, at the user's choice | any | the nearest working reference in our own shader family; its notes match this plan's (motion vectors for everything, trouble with alpha-blended meshes) |
| Playdead's TAA (INSIDE) | Unity HLSL plus the GDC 2016 talk | MIT | any | a clear, small reference implementation |
| AMD FSR 2.2 (standalone repository) | Temporal upscaler, HLSL and GLSL | MIT | any (its table runs down to an RX 590 and a Vega 56) | a portable reference with published costs; last pushed 2023-08-26 |
| AMD FSR 3.1 upscaler (in the AMD FSR SDK 2.4) | FSR 2's successor, HLSL; the SDK also carries FSR 4, frame generation, an ML denoiser | the SDK's main licence is binary-only; the FSR 3 upscaler's source files are listed in its MIT section (70 entries under `upscalers\fsr3`, counted) | any (compute) | portable in principle: a multi-pass compute port to WGSL |
| AMD FSR 4 (ML upscaler) | Machine-learned upscaler | signed binary only, under the binary-only licence | AMD only: its own table lists RX 6000, 7000 and 9000 cards and some Ryzen APUs, nothing older, nothing from NVIDIA or Intel | not open source; would need native DX12 interop under wgpu |
| NVIDIA DLSS | Machine-learned upscaler and frame generation | NVIDIA RTX SDKs licence | NVIDIA RTX only, by the licence's own words | not open source; the licence forbids putting the SDK under an open source licence |
| Intel XeSS | Machine-learned upscaler | Intel Simplified Software License: binary only, no modification | not checked in this scan (not needed while it is not adopted) | not open source |
| SMAA 1x | Spatial edge anti-aliasing (Jimenez et al. 2012), no history | MIT-style | any | a non-temporal mode for people who dislike temporal softness; does not fix shimmer, thin hair or sub-pixel leaves |

## 3. The choice: our own, with FSR 3.1 as the measured alternative

1. **Write our own TAA in WGSL from the papers**, with Bevy's and Playdead's implementations read
   for engineering lessons, not copied. Copying MIT or Apache code into a CC0 tree is allowed with its
   notice kept, but writing our own keeps the tree's licence simple and lets the resolve fit this
   engine's peculiarities (the scene pass clears depth, so velocity must be written by every pass;
   the sky reprojects by camera rotation alone; see the roadmap's section 2).
2. **Then temporal upscaling in the same resolve** (TAAU: a jittered lower-resolution input
   accumulated into a display-resolution history). It is the same pass with a different input size,
   so it costs little to build once the TAA exists.
3. **FSR 3.1's upscaler is the alternative to measure against our TAAU**, not a first step: its
   source files are MIT, it is the strongest open temporal upscaler, and it would cost a port of
   several HLSL compute passes to WGSL. Decide after TAAU is measured on the RTX 4070 and on an RX 580
   class card.
4. **Not adopted: FSR 4, DLSS and XeSS**, because each is distributed as a binary that may not be
   modified (and DLSS only for NVIDIA cards). That is this project's open-source rule, not a law, a
   patent or a platform holder's rule; whether to offer any of them as an optional, separately
   downloaded plugin is a question for the operator (the roadmap's section 13).
5. **SMAA 1x is the Simplified mode** of the anti-aliasing control (no history, so no ghosting and no
   softness), with the hair falling back to today's two passes.

## 4. What it means for this project

**Certain** (from the quotations): FSR 2.2 and the FSR 3.1 upscaler's listed source files may be
modified and redistributed under MIT terms with the notice kept; FSR 4 may be redistributed only as
the signed binary, unmodified; DLSS is licensed only for systems with NVIDIA GPUs and may not be made
subject to an open source licence; XeSS is binary only, without modification.

**Judgement calls**: that our own TAAU will come close enough to FSR 3.1 at the Quality ratio (1.5x)
to make the port unnecessary (to be measured); that writing our own beats copying Bevy's (simpler
licence record, better fit); that a vendor plugin is not worth its licence and maintenance cost before
v1.0.

## 5. Sources (read 12 November 2026), each with its date and the words that carry the meaning

1. **AMD FSR SDK licence**, `Kits/FidelityFX/docs/license.md` in
   https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK (file last changed 2026-10-28; the
   repository page names the release "AMD FSR™ SDK 2.4.0"). The main grant is "to install, reproduce,
   copy and distribute copies of the Software, in binary form only", and "No reverse engineering,
   decompilation, or disassembly of this Software is permitted." A list of files then "are subject to
   the following license:", an MIT grant ("to deal in the Software without restriction, including
   without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
   sell copies of the Software"). The FSR 3 upscaler's headers and internal sources are in that list
   (for example `Kits\FidelityFX\upscalers\fsr3\include\ffx_fsr3upscaler.h`). The repository's root
   holds no LICENSE file of its own (GitHub reports no licence for it).
2. **FSR 4 documentation**, `Kits/FidelityFX/docs/techniques/super-resolution-ml.md` (last changed
   2026-10-28): "AMD FSR™ Upscaling 4 requires integration using the AMD FSR™ API and use of the signed
   binary distribution." Its table (microseconds at peak clocks): RX 9070 XT at 1920x1080, 352; RX 7800
   XT at 2560x1440, 2050; RX 6600 at 1920x1080, 2183. It also asks for input "in LINEAR COLORSPACE".
3. **FSR 3 upscaler documentation**, `.../super-resolution-upscaler.md` (last changed 2026-10-28):
   "AMD FSR 3.1 performance is slightly lower than the AMD FSR 3.0 upscaler, in exchange for higher
   upscaling quality." Working memory at Quality (1.5x): 134MB at 2560x1440, 75MB at 1920x1080.
4. **FSR 2.2**, https://github.com/GPUOpen-Effects/FidelityFX-FSR2 (last pushed 2023-08-26).
   `LICENSE.txt`: "Copyright (c) 2022-2023 Advanced Micro Devices, Inc. All rights reserved.",
   followed by the MIT grant. README, "measured performance of FSR2 on a variety of hardware in DX12":
   at Quality (1.5x), RX 590 1.3ms at 1920x1080 and 2.3ms at 2560x1440; RX 6800 0.7ms at 2560x1440.
   Working set at 2560x1440 Quality: 207MB.
5. **NVIDIA DLSS**, `LICENSE.txt` in https://github.com/NVIDIA/DLSS (last changed 2026-09-08), titled
   "NVIDIA RTX SDKs LICENSE": "the DLSS SDK, NGX SDK and RTX Dynamic Vibrance SDK are licensed for you
   to develop applications only for their use in systems with NVIDIA GPUs." And: "You may not use the
   SDK in any manner that would cause it to become subject to an open source software license."
6. **Intel XeSS**, `LICENSE.txt` in https://github.com/intel/xess (last changed 2025-07-30): "Intel
   Simplified Software License (Version October 2022)"; "You may use and redistribute the software,
   which is provided in binary form only, (the "Software"), without modification".
7. **Playdead's TAA**, https://github.com/playdeadgames/temporal (last pushed 2021-04-03):
   `LICENSE.txt` "The MIT License (MIT)", "Copyright (c) [2015] [Playdead]"; the repository carries the
   GDC 2016 talk "Temporal Reprojection Anti-Aliasing in INSIDE" as a PDF.
8. **SMAA**, https://github.com/iryoku/smaa (last pushed 2024-07-10): `LICENSE.txt` "Copyright (C)
   2013 Jorge Jimenez" and his co-authors, with the MIT grant ("to deal in the Software without
   restriction").
9. **Bevy**, https://github.com/bevyengine/bevy (pushed 2026-11-13 UTC): README, "all code in this
   repository is dual-licensed under either:" MIT or Apache-2.0. The TAA module's notes
   (`crates/bevy_anti_alias/src/taa/mod.rs`): "TAA also does not work well with alpha-blended meshes,
   as it requires depth writing to determine motion." and "It is very important that correct motion
   vectors are written for everything on screen." Its resolve cites Karis 2014 and the Yang, Liu and
   Salvi survey.
10. **wgpu 24** (the version in `Cargo.toml`), read from the cargo registry copy:
    `EXPERIMENTAL_RAY_QUERY`'s "Supported platforms:" lists only Vulkan; the DX12 surface offers
    `Rgba16Float`; `SamplerDescriptor` has `lod_min_clamp`, `lod_max_clamp` and `anisotropy_clamp`
    and no LOD bias, so a temporal upscaler's texture bias must be applied in the shader.

## 6. What is still unknown, and what would settle it

- **How close our TAAU comes to FSR 3.1 at Quality.** Settled by building TAAU and capturing the hair,
  foliage and the crowd vantage against a 4x supersampled reference (the rig's hi-res capture). Cost:
  the TAAU rung itself, plus an afternoon of captures.
- **Whether the FSR 3.1 passes port to WGSL without loss.** They use wave operations and packed half
  precision; wgpu has `SUBGROUP` and `SHADER_F16` features, not guaranteed on every card. Settled by
  porting one pass (the accumulate) and comparing it with the HLSL on the same input.
- **Whether an RX 580 class card reaches the FSR 2 table's numbers under wgpu's DX12 path.** Settled by
  measuring on such a card; none is in the project's hands as of this date.

## 7. Sources not reached

- The AMD SDK's repository-root licence link from its page returned 404 (the file is under
  `Kits/FidelityFX/docs/` and `docs/`, read there).
- XeSS's hardware support and cost were not looked up; they are not needed while XeSS is not adopted.

**Not legal advice; the agent is not a lawyer.** Before shipping any third-party code or binary, read
its licence again on that day and add its notice to `LICENSES.md` and `data/credits.ron`.
