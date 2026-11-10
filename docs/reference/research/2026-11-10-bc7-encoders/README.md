# BC7 texture encoders for the look's garment textures: a scan

**Read on 10 November 2026, Pacific early morning** (local clock: Tue Nov 10 04:06 PST 2026), by an AI
agent working for the project. Pages were read through a fetch tool that identifies itself, after
each site's robots.txt (github.com allows repository pages, not raw files or archives, to Claude's
readers; learn.microsoft.com was read as a documentation page). **Nothing was downloaded**: no
source archive, binary or package, only repository pages, licence files shown as pages, and one
documentation page. Asked for by the brief that built the garment texture pool (character
animation's "the look", `docs/design/character-animation.md` section 16), under the standing rule:
"This, research better alternatives, is something we want to consider every time we learn of new
tools." (operator, 2026-11-08).

**Status: chosen, our own encoder** (`scripts/lib/bc7.js`). Nothing here is a legal finding: the
encoder chosen is the project's own code, so no third-party licence applies to it. Should one of the
others be adopted later, it needs its dated finding first (`docs/reference/findings/README.md`).

## 1. What the job needs

- Encode about 46 garment textures (colour beside normal map, 2048 by 1024 at level 0, every level
  down to 4 texels a half) to BC7 **at conversion time**, inside `scripts/gen-makehuman-look.js`
  (node), so the files ship as asset data. CLAUDE.md's no-caches rule forbids encoding on load and
  keeping it.
- **Deterministic**: the converter's `--check` re-runs it and compares bytes with the committed files.
- **Quick enough to run in `just verify`**: the converter's test runs `--check` with the sources on the
  operator's machine.
- Quality where the eye sees it: cloth at arm's length on a 1440p screen, hair and lash cards whose
  alpha is cut at 0.8 and blended below it.
- Open source (CLAUDE.md: "Open source only, and build what does not exist"), and nothing to download
  without the operator's go-ahead (this wave's rule: "Never download a file other than a page you
  read").

## 2. The candidates

| Encoder | Publisher | Licence (as the page says) | Language, build | BC7 | Fit here |
|---|---|---|---|---|---|
| bc7enc_rdo | Rich Geldreich (richgel999) | Most files "MIT License (Alternative A)" or the Unlicense (Alternative B, "This is free and unencumbered software released into the public domain."); `bc7e.ispc` is Apache 2.0, "Copyright (C) 2018-2021 Binomial LLC" | C++ with CMake; bc7e (all modes) needs Intel's ISPC compiler | all modes (bc7e), or a 4-mode encoder (bc7enc.cpp) | The best quality and speed of the CPU encoders; a download and a C++ build (and ISPC for bc7e) in the conversion path |
| ISPC Texture Compressor | Intel (GameTechDev) | "MIT license" | ISPC compiler 1.9.2 binaries fetched separately, Visual Studio 2017 | yes, with BC6H, ASTC, ETC1 | **Archived on 23 September 2024**, read-only; Intel no longer maintains it |
| intel_tex_2 (Rust crate) | Traverse Research BV, forked from Graham Wihlidal's intel-tex-rs | "Apache-2.0, MIT" | Rust bindings over the ISPC compressor, the ISPC compiler only to regenerate kernels | yes | A crate download; wraps the archived ISPC code; in Rust, while the converter is node |
| Compressonator | AMD (GPUOpen) | the page did not name the main licence (its `license` folder lists the bundled parts' licences) | C++, CMake, a script that fetches external libraries | yes (CPU shown) | A large tool with fetched dependencies; licence not confirmed from the page |
| DirectXTex (texconv) | Microsoft | "subject to the terms of the MIT License" | C++, Visual Studio 2022 or later, Windows 11 SDK; mostly Windows | the page does not say which BC7 paths; a DirectCompute header is in the tree | A Windows tool; a download and a build |
| basis_universal | Binomial LLC | "The encoder library is Apache 2.0" | C++17, CMake | BC7 by transcoding (UASTC is "a common subset of both BC7 and ASTC"; its bc7f encoder supports "all BC7 modes/features") | Built for supercompressed delivery (KTX2); more than this job needs |
| etcpak | Bartosz Taudul (wolfpld) | three-clause BSD ("Redistribution and use in source and binary forms, with or without modification, are permitted") | C++, CMake | yes (it carries bc7enc's source): 12.1 Mpx/s single-threaded, 225 multithreaded on a Ryzen 9 7950X, as its README gives | Fast; a download and a build |
| texpresso (Rust crate) | jansol | MIT (docs.rs) | pure Rust, a libsquish port | listed on its roadmap with unchecked boxes: not confirmed | BC1 to BC5 for certain; BC7 not shown as done |
| JavaScript packages | various | not read | WebAssembly builds (bcn-encdec-wasm and others), WebGPU encoders (gputex, spark.js) | some | No pure-JavaScript BC7 encoder found; the WebAssembly ones are other projects' C++ compiled, a download each |
| **Ours** (`scripts/lib/bc7.js`) | HumanityOS | the project's own | plain JavaScript in the converter, no dependency | modes 1, 3, 5, 6 and 7 (not 0, 2, 4) | Runs where the converter runs, deterministic, nothing to download |

The format itself is public: Microsoft's "BC7 Format" page (learn.microsoft.com, updated 2025-03-11)
gives the modes, the endpoint and p-bit rules, the weight tables and the decoding steps, and says
"BC7 decompression hardware must be bit accurate". The two-subset partition and fix-up tables are the
format's own; our decoder is checked against the GPU's decoder in a test
(`renderer::bc7::the_gpu_decodes_bc7_as_we_do`), so a table error cannot hide.

## 3. The choice: our own, and why

1. **Nothing to download or build.** Every other candidate is a download (a source archive, a crate
   or a package) and most a C++ build, in a conversion step that today needs only node; this wave may
   not download without the operator's go-ahead. Ours is about 730 lines of JavaScript beside the
   converter.
2. **Deterministic by construction**: plain arithmetic on the block's own values, the garments split
   across worker threads but assembled in order, so `--check` compares bytes.
3. **Quick enough**: the whole converter runs in about 30 s on four threads (the textures take about
   20 s of it), so `just verify` keeps its converter check.
4. **Good enough, measured**: level 0 against the packed source, peak signal-to-noise ratio (higher is
   closer; about 40 dB and over is hard to tell apart), as `LOOK-SOURCE.md` records for every garment:
   cloth colour 41 to 53 dB (most 44 to 50), normal maps 48 to 62 dB, hair and lash colour where it
   shows 41 to 53 dB, the brows' dark colour 38 to 42 dB (their alpha 45 to 58), card alpha 41 to 58
   dB. The others were not run here (a download each), so there is no like-for-like comparison of
   quality. Weighing a card's colour more (by twice its alpha, or fully where it shows) was tried on
   five card garments: about 1 dB of colour for about 1 dB of alpha, so the weighting stays.
5. **Ours to grow**: the encoder is small and tested (the error it reports equals the error of its
   own decoded blocks, in every mode), so more modes (4, and the three-subset 0 and 2), more
   partitions tried, or a perceptual weighting are an edit, not a dependency.

**What we give up**: some quality (bc7e uses every mode and searches harder) and speed (etcpak's README
gives 12.1 million texels a second on one thread of a Ryzen 9 7950X; ours packs and encodes a 2048 by
1024 garment with its levels, about 2.8 million texels, in about 3.5 s on one thread of the operator's
i7-8700K, under a million a second). Neither matters at 46 textures encoded once.

**Recommendation to the operator**: keep ours. If a garment ever shows visible blocks up close,
the next step is to let the encoder try more partitions on that garment (an edit); fetching
bc7enc_rdo (MIT or public domain) to compare against would need your go-ahead.

## 4. Normal maps: BC5 considered, BC7 kept

BC5 stores two channels at the same byte a texel and is the usual choice for normal maps (its two
channels each get their own endpoints). It does not fit here without a binding-layout change: a
material binds ONE albedo texture at group 3 (sixteen bindings, every one used), so a garment's colour
and normal map share one texture side by side, and one texture has one format. A separate BC5 texture
would need a seventeenth binding, past the default limit of 16 sampled textures a shader stage that
the device is created with, and every creation site of the layout changed (the v0.1029 incident
class). The normal half as BC7 measures 48 to 62 dB against its source, which is close enough that the
layout change is not worth it now; if normal maps ever need it, BC5 in a texture of their own is the
lever.

## 5. Sources (read 10 November 2026)

- https://github.com/richgel999/bc7enc_rdo and its LICENSE page
- https://github.com/GameTechDev/ISPCTextureCompressor
- https://github.com/Traverse-Research/intel-tex-rs-2
- https://github.com/GPUOpen-Tools/compressonator and its `license` folder
- https://github.com/microsoft/DirectXTex
- https://github.com/BinomialLLC/basis_universal
- https://github.com/wolfpld/etcpak and its LICENSE.txt page
- https://docs.rs/texpresso/latest/texpresso/ and https://github.com/jansol/texpresso
- https://learn.microsoft.com/en-us/windows/win32/direct3d11/bc7-format (updated 2025-03-11)
- A web search for JavaScript and WebAssembly BC7 encoders (no pure-JavaScript encoder found)
