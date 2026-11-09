# How the app runs AI models: one engine, downloaded separately, in its own process

**Research date: 9 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself in every request as
"HumanityOS-research-agent/1.0 (automated AI research tool; https://github.com/Shaostoul/Humanity)".
GitHub repositories were read through GitHub's command-line client (its API) and
raw.githubusercontent.com, crates.io and npm through their public APIs, US regulations through the
eCFR and Federal Register APIs, other sites as pages. Nothing was downloaded beyond the pages and
text files read, and nothing was built, installed or run. Each source carries its own date where it
shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences, project
documentation, security advisories and platform documentation, written so a later reader can check
every word against the same sources.

Builds on the 2026-11-08 NPC brain and voice scan,
[`../research/2026-11-08-npc-brain-voice/README.md`](../research/2026-11-08-npc-brain-voice/README.md),
and the findings it led to: [Ministral 3 3B](2026-11-08-ministral-3-3b.md),
[Granite 4.0 1B](2026-11-08-granite-4-0-1b.md), [Gemma 4 E2B and E4B](2026-11-08-gemma-4-e2b-e4b.md),
[Pocket TTS](2026-11-08-pocket-tts.md), [audio.cpp](2026-11-08-audio-cpp.md) and
[Paradee](2026-11-08-paradee-tts.md). The design that uses this finding is
[`docs/design/embeddable-ai.md`](../../design/embeddable-ai.md).

## The question

Given that the brains are GGUF files run by llama.cpp, the voice is a GGUF run by audio.cpp, the
label reader and read-aloud are ONNX models, a learned motion model may come later, and the runtime
itself must be a separate download the person chooses, which runtime (or runtimes) should the app
use, and in what form: compiled into the app, a library loaded into it, or a separate program?

## Short answer

- **The rule that shapes everything** is the operator's, 9 November 2026, now in CLAUDE.md (Dual
  modes): "AI needs to be optional, not required", and "The model runtime (llama.cpp, audio.cpp,
  ONNX Runtime or our own) is a separate download as well, so a person who wants no AI has none of
  it on their machine". So nothing below is compiled into `HumanityOS.exe`.
- **No single tensor engine runs all four model kinds from their makers' own files.** The approved
  brains are published by Mistral, IBM and Google as GGUF files (F1), which llama.cpp runs; the voice
  runs in audio.cpp, which carries its OWN modified copy of ggml, not llama.cpp's (F3); the label
  reader (PP-OCRv6) and Paradee are ONNX graphs, which neither ggml project loads (F1).
- **Every candidate's licence is permissive and OSI-approved**: llama.cpp and ggml MIT, audio.cpp
  Apache 2.0, ONNX Runtime MIT, the `ort` binding, candle, burn, tract and wgpu each "MIT OR
  Apache-2.0" (F2 to F8).
- **No publisher is on the Entity List or the 1260H list**: ggml (and Georgi Gerganov), Hugging
  Face, Microsoft, pyke, Sonos, Tracel AI, Dial AI, ShugoAI and gfx-rs all returned zero hits, with
  iFlytek and Zhipu as working controls (F9).
- **A model file is hostile input, by the runtimes' own record.** llama.cpp lists thirteen published
  security advisories, several of them in the code that reads a GGUF model file; its own policy says
  "Always execute untrusted models within a secure, isolated environment such as a sandbox"; tract
  lists four model-load advisories in 2026 (F2, F7, F10).
- **A library loaded into the app runs with everything the app holds**, including the person's
  unlocked keys, in Microsoft's words "gains the privileges of the user"; a separate process can be
  given less, and killed as a unit through a job object (F10).
- **Recommendation: one AI engine, built by the project, downloaded separately, running as its own
  program** (`humanity-ai.exe`, Rust), started only while an AI feature is in use and talking to the
  app over its own standard input and output (no network port). Inside it: llama.cpp for the brains,
  audio.cpp (built with only Pocket TTS) for the voice, and tract for the small ONNX models, each
  pinned to a release and loaded only from the engine's own folder by full path. ONNX Runtime is the
  named fallback for the ONNX models if tract fails a model. A learned motion model, if one ever
  comes, stays on the engine's own wgpu path, with its network's shaders and weights inside its
  optional pack (a question for the operator). Conditions are listed at the end.

## Findings

### F1. What has to run, and in which file format

- **The brains** are the publishers' own GGUF files, chosen for provenance in their findings:
  Mistral's `Ministral-3-3B-Instruct-2512-Q4_K_M.gguf`, 2,147,023,008 bytes
  ([finding](2026-11-08-ministral-3-3b.md), F4); IBM's `granite-4.0-1b-Q4_K_M.gguf`, 1,023,645,440
  bytes ([finding](2026-11-08-granite-4-0-1b.md), F5); Google's quantization-aware `gemma-4-E2B_q4_0-it.gguf`,
  3,349,516,256 bytes, and `gemma-4-E4B_q4_0-it.gguf`, 5,154,941,280 bytes
  ([finding](2026-11-08-gemma-4-e2b-e4b.md), F5). GGUF is llama.cpp's format.
- **The voice** is Pocket TTS converted to GGUF and run by audio.cpp
  ([finding](2026-11-08-audio-cpp.md)). audio.cpp's own converted packages, listed through the
  Hugging Face API for `audio-cpp/audio.cpp-gguf` on the research date:
  `PocketTTS-GGUF/english/pocket-tts-english-q8_0.gguf` 127,867,968 bytes; the small English models
  `english-18m-refl-f32.gguf` 111,809,536 and `english-2m-refl-f32.gguf` 46,792,352.
- **The label reader** is PaddleOCR PP-OCRv6 "through ONNX Runtime" in the ingredients design
  ([`ingredients-and-labels.md`](../../design/ingredients-and-labels.md), section 5.2), and
  **read-aloud** is Paradee 1.2, whose model card says of its int8 ONNX file "The whole model in one
  graph, weights in int8 (8.5 MB). Use this one." ([finding](2026-11-08-paradee-tts.md), F1).
- **A learned motion model**, if one is ever adopted, is planned on the engine's own GPU path:
  [`character-animation.md`](../../design/character-animation.md), section 8.7: "Any learned model
  would run in our own wgpu compute pipelines, built at startup like every other pipeline, with no
  pipeline cache on disk". The same document records today's state: "**No machine-learning
  runtime** is in the dependency tree."

### F2. llama.cpp and ggml

- **Licence.** https://github.com/ggml-org/llama.cpp (created 2023-03-10, last push 2026-11-09;
  GitHub's licence detection: MIT). `LICENSE`, first lines:
  > MIT License
  >
  > Copyright (c) 2023-2026 The ggml authors

  The ggml library's own repository, https://github.com/ggml-org/ggml (created 2022-09-18, last push
  2026-11-08), has the same first lines.
- **Publisher.** The GitHub organization `ggml-org` lists no location; its profile links to
  https://huggingface.co/ggml-org. Hugging Face's post "GGML and llama.cpp join HF to ensure the
  long-term progress of Local AI" (its source file in https://github.com/huggingface/blog,
  `ggml-joins-hf.md`, dated "Feb 20, 2026" in the blog's index `_blog.yml`):
  > We are super happy to announce that GGML, creators of Llama.cpp, are joining HF in order to keep future AI open.

  and, on what changes:
  > Not much – Georgi and team still dedicate 100% of their time maintaining llama.cpp and have full autonomy and leadership on the technical directions and the community.
- **Hardware.** README (master, research date):
  > Plain C/C++ implementation without any dependencies

  > AVX, AVX2, AVX512 and AMX support for x86 architectures

  and its "Supported backends" table, as rendered (backend, then "Target devices"): "CUDA", "Nvidia
  GPU"; "HIP", "AMD GPU"; "SYCL", "Intel GPU"; "Vulkan", "GPU". So the CPU path runs on any x86-64
  processor and the Vulkan path on any vendor's GPU with a Vulkan driver.
- **Backends as separate libraries.** `docs/build.md`:
  > Backends can be built as dynamic libraries that can be loaded dynamically at runtime. This allows you to use the same llama.cpp binary on different machines with different GPUs. To enable this feature, use the `GGML_BACKEND_DL` option when building.
- **Size on disk.** The newest build on the research date, `b12212` (published 2026-11-09 08:49
  UTC), Windows x64 archives through the API: `llama-b12212-bin-win-cpu-x64.zip` 24,886,394 bytes;
  `-vulkan-x64.zip` 37,845,777; `-cuda-12.4-x64.zip` 288,918,507 plus `cudart-llama-bin-win-cuda-12.4-x64.zip`
  391,443,627; `-rocm-10.0-x64.zip` 267,490,363; `-sycl-x64.zip` 156,207,516. Each archive carries
  every tool (command line, server and others); a library-only build is smaller (not measured).
- **How it can be hosted.** As a C library (`include/llama.h`, for example
  `LLAMA_API struct llama_model * llama_model_load_from_file(`), with a Rust binding on crates.io,
  `llama-cpp-2` 0.1.161 (2026-11-03, "MIT OR Apache-2.0", repository `utilityai/llama-cpp-rs`, owner
  Dial AI, "Canada"), whose features include `vulkan` and `dynamic-backends`. Or as its own HTTP
  server, `llama-server`, whose README gives `--host` "(default: 127.0.0.1)", `--port` "(default:
  9931)" and `--api-key` "(default: none)".
- **Release cadence.** Eight numbered builds were published between 2026-11-08 08:11 UTC (`b12202`)
  and 2026-11-09 08:49 UTC (`b12212`); versioned releases also exist (`v0.9.0`, 2026-11-04).
- **Security.** `SECURITY.md` (master, research date), "Untrusted models":
  > *Always execute untrusted models within a secure, isolated environment such as a sandbox* (e.g., containers, virtual machines). This helps protect your system from potentially malicious code.

  "Untrusted environments or networks":
  > Confirm the hash of any downloaded artifact (e.g. pre-trained model weights) matches a known-good value.

  "Multi-Tenant environments":
  > Since there are no reliable methods to detect malicious models, sandboxing the model execution is the recommended approach to mitigate the risk.

  And at its top:
  > The private security disclosure program is disabled until further notice. Please submit patches with fixes directly to the repo as public PRs. Emails will be ignored.

  The repository's published advisories (API, research date) number thirteen, from 2024-04-26 to
  2026-03-26. Among them, by title: "Heap Buffer Overflow via Integer Overflow in GGUF Tensor
  Parsing" (GHSA-96jg-mvhq-q7q7, CVE-2026-33298, high, 2026-03-18); "Integer Overflow in GGUF Parser
  can lead to Heap Out-of-Bounds Read/Write in gguf" (CVE-2025-53630, high, 2025-07-10); "Buffer
  Overflow in llama.cpp via Malicious GGUF Model – Exploitable via Vocabulary Loading
  (`llama_vocab::impl::token_to_piece`)" (CVE-2025-49847, high, 2025-06-14); and "Unauthenticated
  RCE via GRAPH_COMPUTE buffer=0 bypass in llama.cpp RPC backend" (CVE-2026-34159, critical,
  2026-03-26).

### F3. audio.cpp

- **Licence and dependencies**: Apache 2.0 with the notice "Copyright 2026 ShugoAI LLC", and only
  permissive vendored dependencies; eSpeak NG out of a Pocket-TTS-only build. All of this is in the
  [audio.cpp finding](2026-11-08-audio-cpp.md) of the day before and was not re-derived. The
  repository's last push on the research date was 2026-11-09 05:26 UTC.
- **Hardware.** README (main, research date):
  > `audio.cpp` is a high-performance C++ audio inference framework built on top of `ggml`, designed to make modern local audio models practical, portable, and fast.

  > A native `ggml` runtime supporting CUDA, HIP/ROCm, Vulkan, Metal, WebGPU, SYCL, and CPU through common CLI and server interfaces.

  Its prebuilt table: `| Windows x64 | CPU, Vulkan, CUDA (12.4 / 13.3), HIP/ROCm (test) |`.
- **Size on disk.** Release `v0.9.4` (published 2026-11-05), through the API:
  `audio-v0.9.4-lib-windows-x64-cpu.zip` 13,542,870 bytes; `-lib-windows-x64-vulkan.zip` 30,751,615;
  `-bin-windows-x64-cpu.zip` 32,323,805; `-lib-windows-x64-cuda12.4.zip` 456,455,616 plus
  `audio-v0.9.4-cudart-windows-x64-cuda12.4.zip` 607,273,675. These carry every model family; a
  Pocket-TTS-only build is smaller (not measured).
- **How it can be hosted.** `CMakeLists.txt`:
  > option(AUDIOCPP_BUILD_C_API "Build the C ABI (include/audiocpp.h) as a shared library" OFF)

  and its comment over that section (comment lines joined):
  > A thin facade over engine::runtime for embedding audio.cpp in-process from languages that can call C. Purely additive: nothing above this point changes when AUDIOCPP_BUILD_C_API is OFF, which is the default.

  It also ships a server, `audiocpp_server`; the README: "Open `http://127.0.0.1:8080`."
- **Its ggml is its own copy, modified in place.** `external/ggml` is a vendored tree (it holds
  ggml's own `LICENSE`, `CMakeLists.txt`, `src` and so on), and audio.cpp commits change it directly.
  The newest commits touching `external/ggml` (API, research date) include "ggml-sycl: kernels and
  graph replay for audio models (#1242)" (2026-11-09) and "Add WebGPU support for 3D im2col (#1203)"
  (2026-11-05). So audio.cpp's ggml and llama.cpp's ggml are different code, and the two cannot be
  linked into one program as one ggml.
- **Its own warning about sharing a process.** `CMakeLists.txt`, on the C API library (comment lines
  joined):
  > The static libraries linked in (ggml, cJSON, sentencepiece, ...) are built without -fvisibility=hidden, so without an explicit export list they are re-exported from libaudiocpp -- measured at 2767 dynamic symbols where the header declares 69 -- and can collide with whatever the host application has already loaded.

  It restricts the exported names on macOS and Linux; for Windows it says "Exclusivity there comes
  from making sure no dependency annotates its symbols".
- **In-memory reuse it keeps** (relevant to the project's reuse switch), `docs/tts.md`:
  `| pocket_tts.voice_state_cache_slots | integer >= 0 | 4 | Cached prepared voice states; zero disables reuse. |`
  (backticks dropped).
- **Security.** No advisories are published for the repository (API, research date: an empty list).
  That is an absence of reports, not evidence of safety: the project is four and a half months old.

### F4. ONNX Runtime, and the `ort` binding

- **Licence.** https://github.com/microsoft/onnxruntime (created 2018-11-10, last push 2026-11-09;
  GitHub's licence detection: MIT). `LICENSE`:
  > MIT License
  >
  > Copyright (c) Microsoft Corporation
- **Hardware, from its documentation** (https://onnxruntime.ai/docs/execution-providers/, undated
  pages). DirectML, the route to any DirectX 12 GPU on Windows:
  > Note: DirectML is in sustained engineering. DirectML continues to be supported, but new feature development has moved to WinML for Windows-based ONNX Runtime deployments.

  > The DirectML execution provider requires a DirectX 12 capable device.

  WebGPU, the newer cross-vendor route:
  > On native platforms it uses Dawn (Google’s WebGPU implementation), which in turn dispatches to D3D12, Vulkan, or Metal, depending on the platform.

  > The native WebGPU EP is distributed as a plugin EP: a shared library that is registered at runtime alongside a compatible core ONNX Runtime install.

  The CPU provider is the default and runs everywhere.
- **Size on disk.** Release `v1.30.1` (published 2026-10-29): `onnxruntime-win-x64-1.30.1.zip`
  82,695,782 bytes; `onnxruntime-win-x64-gpu_cuda12-1.30.1.zip` 380,122,657. The archives hold more
  than the library itself (the library's own size was not measured).
- **The website's twin.** npm `onnxruntime-web` 1.31.1, `"license": "MIT"`, repository
  `github.com/Microsoft/onnxruntime` (registry, research date).
- **The Rust binding.** https://github.com/pykeio/ort (created 2022-11-26, last push 2026-11-01).
  `Cargo.toml`: `version = "2.0.0-rc.15"` and `license = "MIT OR Apache-2.0"`; crates.io has no
  stable 2.x release (`max_stable_version` null; 2.0.0-rc.15 of 2026-10-19). `LICENSE-MIT`:
  > Copyright (c) 2023-2026 pyke.io
  > Copyright (c) 2020 Nicolas Bigaouette

  Owner `pykeio`, "pyke", "United States of America". Its linking guide,
  https://ort.pyke.io/setup/linking (undated), on the `load-dynamic` feature:
  > This means that the path to the ONNX Runtime library can be configured at runtime, and the executable will not just completely fail to start if the binary couldn’t be found.

  Its features page, https://ort.pyke.io/setup/cargo-features (undated), on the default
  `download-binaries`:
  > Downloads prebuilt binaries from pyke’s CDN service for supported platforms. Disabling this feature means you’ll need to compile ONNX Runtime from source and link ort to it.

  README: "`ort` is primarily a wrapper for Microsoft's [ONNX Runtime](https://onnxruntime.ai/)
  library, but offers support for [other pure-Rust runtimes](https://ort.pyke.io/backends)"; its
  guide lists `ort-candle`, `ort-tract` and `ort-web` as alternative backends.
- **Security.** `SECURITY.md` is Microsoft's standard text ("Please do not report security
  vulnerabilities through public GitHub issues", reports to the Microsoft Security Response Center).
  The repository lists no GitHub advisories (API: zero); Microsoft publishes through its own centre,
  which was not searched, so no comparison of advisory counts with llama.cpp is possible.

### F5. candle

- **Licence.** https://github.com/huggingface/candle (created 2023-06-19, last push 2026-11-02).
  crates.io `candle-core` 0.11.1 (2026-10-31): "MIT OR Apache-2.0". Its `LICENSE-MIT` is the MIT
  permission text with no copyright line (it starts "Permission is hereby granted, free of charge,
  to any"); `LICENSE-APACHE` keeps the appendix placeholder `Copyright [yyyy] [name of copyright owner]`.
- **Publisher**: Hugging Face ("NYC + Paris" on its GitHub profile), the same company that ggml
  joined (F2).
- **Hardware.** README, "Features":
  > Optimized CPU backend with optional MKL support for x86 and Accelerate for macs.

  > CUDA backend for efficiently running on GPUs, multiple GPU distribution via NCCL.

  > WASM support, run your models in a browser.

  The crate's features on crates.io are `accelerate cuda cudnn cutile default metal
  metal-debug-labels mkl nccl`: no Vulkan, so no GPU route for AMD or Intel on Windows.
- **Formats.** "File formats: load models from safetensors, npz, ggml, or PyTorch files." and
  "Quantization support using the llama.cpp quantized types." Its included models are written one by
  one in Rust; the README's list names none of Ministral 3, Granite 4.0, Gemma 4 or Pocket TTS (the
  Pocket TTS card lists a community "Candle version (Rust) with WebAssembly", per the scan).
- **How it can be hosted**: a Rust library, compiled into whatever program uses it. Its goal, in its
  own words: "Candle's core goal is to *make serverless inference possible*."

### F6. burn

- **Licence.** https://github.com/tracel-ai/burn (created 2022-07-18, last push 2026-11-08).
  crates.io `burn` 0.22.1 (stable, 2026-10-20; 0.23.0-pre.1 on 2026-10-29): "MIT OR Apache-2.0".
  `LICENSE-MIT`: "Copyright (c) 2022 Nathaniel Simard & Burn Framework Contributors".
- **Publisher**: Tracel AI, "Canada" (GitHub profile).
- **Hardware.** README, "Supported Backends", GPU table rows:

  | | CUDA | ROCm | Metal | Vulkan | WebGPU |
  |---|---|---|---|---|---|
  | Nvidia | ☑️ | - | - | ☑️ | ☑️ |
  | AMD | - | ☑️ | - | ☑️ | ☑️ |
  | Intel | - | - | - | ☑️ | ☑️ |

  (three of the table's six rows, as published), and two CPU backends, one "a pure-Rust eager
  backend with no native dependencies that also runs on Wasm and `no_std`". Also: "LibTorch was
  deprecated in `0.22.0` and has been removed from `main`."
- **ONNX.** Through `burn-onnx`, which turns a model into Rust source; the README's note:
  > **Note**: This crate is in active development and currently supports a

  (continuing, as rendered, "limited set of ONNX operators", a link to its list of supported
  operators). No GGUF loading is described.
- **How it can be hosted**: a Rust library compiled into a program. Pre-1.0: version 0.22 to 0.23
  in ten days.

### F7. tract

- **Licence.** https://github.com/sonos/tract (created 2017-08-07, last push 2026-11-09). crates.io
  `tract-onnx` 0.23.10 (2026-11-05): "MIT OR Apache-2.0". README, "License":
  > All original work is licensed under either of

  (continuing with Apache 2.0 and MIT, "at your option"), and before it:
  > Files in `tensorflow/protos` are copied from the [TensorFlow](https://github.com/tensorflow/tensorflow) project and files in `onnx/protos` from the [ONNX](https://github.com/onnx/onnx) project; neither is covered by the licence statement below.

  (Line breaks joined.)
- **Publisher**: "Sonos, Inc.", "USA" (GitHub profile). README:
  > Sonos' neural-network inference engine.

  > It is used in production at Sonos for wake-word and streaming speech-recognition workloads

  (the second quotation is the middle of a sentence; line breaks joined).
- **Hardware.** README, its runtime table: `| CPU (x86, ARMv6/7/8, ARM SVE) | "default" | tract-linalg | Default. Hand-rolled SIMD micro-kernels. |`,
  `| Apple Metal | "metal" | tract-metal | Apple GPUs. |`, `| NVIDIA CUDA | "cuda" | tract-cuda | NVIDIA GPUs. |`,
  `| WebAssembly | "default" | _via standard wasm32 targets_ | Browser / WASI deployment. |`
  (backticks dropped). So: any CPU, and the same Rust code in a browser; no GPU route for AMD or
  Intel.
- **Formats**: ONNX and NNEF; it can convert an ONNX model to NNEF once at build time and ship
  "only `tract-core` + `tract-nnef`" at runtime (README).
- **Security.** Four advisories published in 2026 (API, research date), all on loading a model:
  "Arbitrary file read via unsanitized ONNX external_data `location` (path traversal) on model load
  in tract-onnx" (CVE-2026-55832, 2026-06-18); "Incomplete CVE-2026-55832 remediation: ONNX
  external_data follows symlinks outside the model directory" (2026-10-16); "tract-nnef: integer
  overflow in NNEF `.dat` tensor parser yields an out-of-bounds read on model load" (CVE-2026-55093,
  2026-06-17); and "Unbounded allocation in tract-nnef NNEF tensor parser allows denial of service on
  untrusted model load (CWE-770)" (2026-10-09). All medium.

### F8. wgpu: the engine's own GPU path

- **Licence.** https://github.com/gfx-rs/wgpu (created 2018-09-13, last push 2026-11-09).
  crates.io `wgpu` 30.0.1 (2026-08-22): "MIT OR Apache-2.0". `LICENSE.MIT`: "Copyright (c) 2026 The
  gfx-rs developers".
- **Hardware.** README:
  > `wgpu` is a cross-platform, safe, pure-Rust graphics API. It runs natively on Vulkan, Metal, D3D12, and OpenGL; and on top of WebGL2 and WebGPU on wasm.
- **In this repository**: the engine depends on `wgpu = "24"` (root `Cargo.toml`), six major versions
  behind the newest. It has compute pipelines (skinning, character-animation rung 1) but no neural
  network kernels: a model on this path means writing and maintaining our own matrix, attention and
  quantization kernels per model architecture.
- **Publisher**: the gfx-rs organization ("Rust Graphics Mages", location "Earth"), a community
  project; the README says it "serves as the core of the WebGPU integration in Firefox, Servo, and
  Deno".

### F9. The US-government check

The same method as the 2026-11-08 scan (its Part 0):

- **The Entity List** (15 CFR Part 744, Supplement No. 4), searched by exact phrase through the eCFR
  search API, restricted to Title 15, Part 744, at the version in effect on 2026-11-05. The eCFR's
  currency record for Title 15 read, on the research date:
  `{"latest_amended_on":"2026-11-04","latest_issue_date":"2026-11-05","up_to_date_as_of":"2026-11-05"}`.
  **Zero hits** for each of: "ggml", "Gerganov", "Hugging Face", "Microsoft", "pyke", "Sonos",
  "Tracel", "Dial AI", "ShugoAI", "gfx-rs", "Mozilla", "Khronos". **Controls**: "iFlytek" and
  "Zhipu", both on the list per the scan, returned 6 hits each, so the search works.
- **The DoD 1260H list**, 91 FR 35189, "Notice of Availability of Designation of Chinese Military
  Companies" (10 June 2026), read in full through the Federal Register API
  (`https://www.federalregister.gov/documents/full_text/text/2026/06/10/2026-11571.txt`): none of
  the names above appears; the controls "Alibaba" and "Baidu" each appear. The one later Federal
  Register document mentioning 1260H (2026-10-26, document 2026-21786, "Outbound Investment Security
  Program Regulations") is Treasury's investment rule, not a change to the list.
- Limit, as in the scan: a full-text search can miss a differently spelled entry, and the
  Consolidated Screening List was not queried (it needs a registered key).

### F10. A library inside the app, or a separate program: what the platform documents say

- **What the app holds in memory.** [`docs/ai/storage.md`](../../ai/storage.md): the desktop's
  `config.json` holds the encrypted identity vault, and "the Dilithium3 chat identity and Kyber keys
  are re-derived from it, never stored". Once the person unlocks it, those private keys live in the
  app's memory, and any code running in the app's process can read them.
- **Loading a library.** Microsoft, "Dynamic-Link Library Security",
  https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-security (page metadata:
  updated 2025-04-15):
  > If an attacker gains control of one of the directories on the DLL search path, it can place a malicious copy of the DLL in that directory.

  > The malicious copy of the DLL then runs within the application and gains the privileges of the user.

  > Wherever possible, specify a fully qualified path when using the LoadLibrary, LoadLibraryEx, CreateProcess, or ShellExecute functions.

  The same holds for a genuine library with a bug: a parser overflow in a loaded library (F2, F7)
  runs inside the app.
- **A separate process as a unit.** Microsoft, "Job Objects",
  https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects (updated 2025-07-16):
  > Operations performed on a job object affect all processes associated with the job object. Examples include enforcing limits such as working set size and process priority or terminating all processes associated with a job.
- **A separate process with less.** Microsoft, "AppContainer isolation",
  https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation (updated
  2025-07-10):
  > Even if vulnerability in an app is exploited, the app cannot access resources beyond what is granted to the AppContainer.

  > Granular access can be granted for Internet access, Intranet access, and acting as a server.

  Whether a GPU driver (Vulkan) loads inside an AppContainer was not tested here (below).
- **Hosting the files.** GitHub, "About releases",
  https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases (undated page):
  > Each file included in a release must be under 2 GiB. There is no limit on the total size of a release, nor bandwidth usage.

  2 GiB is 2,147,483,648 bytes. Ministral's file (2,147,023,008) fits by 460,640 bytes; Granite and
  Pocket TTS fit easily; Gemma 4 E2B needs two parts and E4B three.
- **Signing.** [`docs/reference/cryptography.md`](../cryptography.md), "Release signing": the
  updater installs only a release whose `release-manifest.json` (`{version, artifacts: [{name,
  sha256, size}]}`) carries both signatures, and the public keys are "compiled into every build". A
  hash list compiled into the signed app therefore vouches for anything the app later downloads, the
  way `data/music/packs.ron` already pins each instrument pack by SHA-256
  ([`docs/ai/release.md`](../../ai/release.md), "Instrument packs").

## Side by side

| | Licence | Publisher, list check | Hardware | Size (Windows x64) | Separate download? | Where it would run |
|---|---|---|---|---|---|---|
| **llama.cpp / ggml** | MIT | ggml-org (Hugging Face since 2026-02-20); none | Any x86-64 CPU (AVX to AMX); Vulkan on any vendor; CUDA, HIP, SYCL | CPU 24.9 MB, Vulkan 37.8 MB (all tools); CUDA 289 + 391 MB | Yes: a C library, backends as runtime-loaded libraries, or its own server | The brains |
| **audio.cpp** | Apache 2.0 | ShugoAI LLC; none | CPU, Vulkan, CUDA, HIP (test) on Windows | lib CPU 13.5 MB, lib Vulkan 30.8 MB (all models) | Yes: C API as a shared library (off by default), or its own server | The voice; its own ggml copy |
| **ONNX Runtime + `ort`** | MIT; binding MIT OR Apache-2.0 | Microsoft, pyke; none | CPU everywhere; DirectML (sustained engineering) or WebGPU plugin on any DX12/Vulkan GPU; CUDA | 82.7 MB archive (library unmeasured); CUDA 380 MB | Yes: `load-dynamic` loads it from a path at runtime | ONNX models; the website twin `onnxruntime-web` |
| **candle** | MIT OR Apache-2.0 | Hugging Face; none | CPU (MKL optional), CUDA, Metal; no Vulkan | Compiled in (unmeasured) | Only as part of a program we build | Per-model Rust code; GGUF types |
| **burn** | MIT OR Apache-2.0 | Tracel AI; none | CPU (two), CUDA, ROCm, Metal, Vulkan, WebGPU | Compiled in (unmeasured) | Only as part of a program we build | ONNX to Rust (limited operators); pre-1.0 |
| **tract** | MIT OR Apache-2.0 | Sonos; none | Any CPU, CUDA, Metal; WebAssembly | Compiled in (unmeasured) | Only as part of a program we build | Small ONNX models, desktop and browser alike |
| **wgpu (our own)** | MIT OR Apache-2.0 | gfx-rs; none | Vulkan, DX12, Metal, GL; any vendor | Already in the app (no new library) | The kernels and weights could ship in a pack | A learned motion model, in the frame |

## What is still unknown, and what would settle it

1. **Whether tract runs PP-OCRv6 and Paradee correctly and fast enough.** Settled by a spike in the
   label reader's rung 2: both models through tract and through ONNX Runtime on the same golden
   inputs, outputs compared within a tolerance, time and binary size measured (a day of work).
2. **The size of the engine as the project would build it** (llama.cpp library plus Vulkan and CPU
   backends, audio.cpp with only Pocket TTS, tract). Settled by building it once (an hour); the
   archive sizes above are upper bounds that include every tool and model family.
3. **Whether the host can run inside an AppContainer and still reach the GPU through Vulkan**, and
   whether a CPU-only host gains anything from one. Settled by a test program on the operator's PC and
   one AMD or Intel PC (a day); until then the host runs as a normal child process at reduced
   priority inside a job object.
4. **Whether the per-load hash check of a 2 to 5 GB model is cheap enough on a floor-class CPU.**
   Settled by timing SHA-256 over the Ministral file on the floor PC in rung 1 (minutes).
5. **Who owns copyright in llama.cpp after the Hugging Face move.** The post does not say; it
   changes nothing in the MIT grant, which every past contributor already gave. Settled, if it ever
   matters, by asking in the repository (free).
6. **ONNX Runtime's security record**, published through Microsoft's own response centre, was not
   searched. Settled by searching Microsoft's update guide for "ONNX Runtime" (an hour) if ONNX
   Runtime becomes the fallback in use.

## Sources I could not reach

- **https://ort.pyke.io/prebuilt**: answered "404 This page could not be found." Hoped for: how pyke
  builds the binaries its default feature downloads. The features page (F4) was used instead.
- **https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setprocessmitigationpolicy**:
  404 at that address (the address was a guess). Hoped for: the process mitigation policies a host
  could set on itself. Not needed for this finding's conclusion; the job object and AppContainer
  pages were read instead.
- **The Hugging Face post's rendered page and GitHub discussion #19759 on llama.cpp** were not opened;
  the post was read from its source file in Hugging Face's blog repository (F2), which is the same
  text.

## What it means for this project

### Certain

- Every runtime compared may be used and redistributed under its licence, with its notice shipped
  (MIT's "in all copies or substantial portions", Apache 2.0's section 4), and none of their
  publishers is on either US list as of the dates above (F2 to F9).
- No one tensor engine runs the approved brains, the approved voice and the ONNX models from their
  makers' own files; audio.cpp's ggml and llama.cpp's ggml are different code (F1, F3).
- Model loaders in these runtimes have had memory-safety bugs reachable from a crafted model file,
  and llama.cpp's own policy asks for isolation and hash checks (F2, F7).
- GitHub release assets cap each file under 2 GiB, so the larger Gemma files must be split (F10).
- A library loaded into the app would run with the person's unlocked keys in reach (F10).

### Judgement calls

- **A separate program beats a library in the app**, for five reasons: the keys stay out of reach of
  a model parser; a crash or an out-of-memory in a 2 GB model does not take the game down; the
  operating system can cap its memory and priority as a unit and kill it with the app (job object);
  it needs no network at all, so no firewall question ever arises (stdin and stdout, no listener);
  and "remove AI" is deleting one folder. The cost is a message boundary, negligible next to the
  work: a token is a few bytes every 50 to 100 ms, speech is 48 KB a second at 24 kHz mono 16-bit,
  a label photo a few MB once.
- **Our own program, not `llama-server` and `audiocpp_server`.** Their servers listen on a TCP port
  with no key by default (F2, F3), carry web interfaces and features the project does not need, and
  llama.cpp's policy says not to expose its server in untrusted settings. A small Rust host that
  links the two C libraries and speaks the app's own protocol over pipes has none of that surface.
- **Two ggml copies in one host are acceptable**, each inside its own library (llama.cpp's static in
  the host, audio.cpp's inside its C-API library with its exports limited to `audiocpp_*`), because
  Windows keeps each library's names to itself and audio.cpp already engineers for exactly this
  (F3). If that proves fragile, the fallback is two host processes with the same protocol.
- **tract for the small ONNX models, ONNX Runtime as the fallback.** tract is pure Rust, compiles
  into the host with no second C++ library, and the very same code compiles for the website, so the
  label reader gives the same answer on both mirrors (the web-mirrors-native rule). ONNX Runtime is
  more complete and faster on large models, but its Windows GPU route (DirectML) is in sustained
  engineering, its Rust binding is still a release candidate that downloads binaries from pyke's
  CDN by default, and these models are small CPU models. The spike in unknown 1 decides it.
- **CPU and Vulkan only.** The vendor packages (CUDA 289 to 456 MB plus 391 to 607 MB of runtime,
  ROCm, SYCL) are bigger than the models and serve one vendor each; Vulkan reaches every vendor. During
  play the brain runs on the CPU so the renderer keeps the GPU (the scan's plan).
- **candle and burn are not needed now.** candle has no GPU route for AMD or Intel and would mean
  re-implementing each model in Rust; burn has the widest Rust GPU reach but limited ONNX coverage,
  no GGUF and a fast-moving pre-1.0 API. burn is the one to revisit if the project ever writes its
  own model code.
- **The learned motion model stays in the engine's wgpu path** (character-animation's plan): it runs
  inside the frame on the renderer's GPU, so a separate process would add a cross-process GPU hand-off
  for nothing. To keep "no AI on the machine" true when its pack is absent, the network's shaders and
  weights ship inside that optional pack, and the engine carries only its generic compute plumbing,
  which it already uses for skinning.

### The recommendation, and its conditions

**One AI engine: a program the project builds, downloaded separately, in its own folder, started
only while an AI feature is in use.**

1. **Built by the project from pinned sources**: a llama.cpp tag, audio.cpp `v0.9.4` or a later tag
   built with `AUDIOCPP_MODEL_SET=custom`, `AUDIOCPP_MODELS=pocket_tts` (plus `pocket_tts_small`),
   `AUDIOCPP_STATIC_ESPEAK=OFF` and `AUDIOCPP_BUILD_C_API=ON` (the audio.cpp finding's conditions plus
   the C API), and tract `0.23.x`; portable CPU kernels and a Vulkan backend; no CUDA, ROCm or SYCL.
2. **Never inside `HumanityOS.exe`**: a lint fails the build if the app's own dependency tree names
   llama.cpp, audio.cpp, tract, ONNX Runtime, candle or burn, or if `data/` holds a model file.
3. **Talks over standard input and output only**: no network socket, no listener, no network access
   needed; the app downloads and verifies every file and hands the engine paths.
4. **Contained**: started inside a job object that kills it with the app and caps its memory, at below
   normal priority; libraries loaded only from its own folder by full path (Microsoft's guidance);
   AppContainer once unknown 3 is settled.
5. **Pinned and verified**: the engine and every model listed with SHA-256 and size in a catalog
   compiled into the signed app (the instrument packs' way, with the release signature over it);
   checked after download and before each load; files over 2 GiB split into parts, each with its own
   hash, the whole checked again after joining.
6. **Updated on purpose**: a new engine version arrives only with an app release that names it,
   never automatically from upstream; a llama.cpp security fix is a reason for that release.
7. **Credited**: the engine's folder carries the MIT, Apache 2.0 and every vendored notice (llama.cpp,
   ggml, audio.cpp with "Copyright 2026 ShugoAI LLC" and its dependencies per the audio.cpp finding,
   tract, and the Rust crates the host links); rows in `data/credits.ron`, `CREDITS.md` and
   `LICENSES.md`.
8. **ONNX Runtime only as the fallback**, if tract fails the spike: Microsoft's own release (not the
   `ort` default download), loaded by the host from its folder with `load-dynamic`, and
   `onnxruntime-web` on the website.

### The question for the operator

**May the app's AI run in one separate program the project builds (one download, its own folder,
started only while AI is in use, talking to the app through pipes with no network), containing
llama.cpp for the brains, audio.cpp with only Pocket TTS for the voice, and tract for the label
reader and read-aloud, with ONNX Runtime as the fallback?**

Recommendation: **yes**, on the conditions above. It keeps every AI part off a machine whose owner
never asks for it, keeps the person's keys out of reach of a model file, keeps the game running if a
model crashes, and uses each runtime where it is strongest.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
