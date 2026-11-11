# humanity-ai-engine

## What it is for

The AI engine, `humanity-ai.exe`: the one separate program that will run every AI model a person
chooses, never part of the app. Its design is docs/design/embeddable-ai.md, section 7, and the runtime
finding docs/reference/findings/2026-11-09-model-runtime.md: started by the app only while an AI
feature is in use, talking over its standard input and output (no socket, no listener, no network).
Rung 1 (2026-11-09) built its skeleton: `--fake` answers with scripted lines and timing so every
screen, fallback and notice is tested without a model. Rung 2 (2026-11-09) adds llama.cpp behind the
`llama` cargo feature: CPU only, the portable kernels (one `ggml-cpu-<level>.dll` per processor
generation, the best chosen at start), Granite 4.0 1B as the first model (Ministral 3 3B runs on the same
engine unchanged, 2026-11-11); built without the feature
(the plain `cargo build` the app's client tests make) it still refuses every model request in words. It
has a workspace of its own, outside the app's, so HumanityOS.exe can never depend on it and the C++
runtimes it links (llama.cpp now, audio.cpp later) never slow the app's build.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-ai-engine/`, a program with a workspace of its own (`scripts/crate-layers.json` gives its `dir` and `"workspace": "own"`: not a member of the app's workspace, built only by its own recipe, and nothing may depend on it); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-ai-engine/src/args.rs`, `crates/humanity-ai-engine/src/fake.rs`, `crates/humanity-ai-engine/src/folder.rs`, `crates/humanity-ai-engine/src/memory.rs`, `crates/humanity-ai-engine/src/run.rs`, `crates/humanity-ai-engine/src/text.rs`
  - `feature = "llama"`: `crates/humanity-ai-engine/src/llama/`, `crates/humanity-ai-engine/src/real.rs`
  - test builds only: `crates/humanity-ai-engine/src/tests.rs`
- **Size:** 1,742 lines in 12 files, 421 of them in 1 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `llama` 19.
- **May use** (the plan): ai-protocol. **Uses today** (references in non-test code): ai-protocol 25.
- **May be used by** (the plan): nothing; it is the top of the stack. **Used today by:** nothing.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 18 tests, and this runs exactly them, checked against every test's name:

```
just ai-engine test
```

**Interface:** no other crate references anything in it.
<!-- end generated -->

## Rules

- **Never a dependency of HumanityOS.exe, never a member of its workspace.** Its own `[workspace]` and
  `Cargo.lock`, listed in the root Cargo.toml's `exclude`; `node scripts/check-crate-layers.js`
  refuses it as a member, refuses any plan entry that may use it, and `tests/no_ai_in_app_lint.rs`
  refuses a model runtime in the app's dependency tree. Source: the operator, 2026-11-09 ("AI needs to
  be optional, not required"; the runtime a separate download too, "Agreed."); the design, 7.1.
- **Standard input and output only.** No socket, no listener, no network access; the app downloads and
  checks every file and hands the engine paths. Only messages go to standard output; notes for a person
  go to standard error, which the app writes into its log. Source: the design, 7.2; the runtime
  finding's condition 3.
- **Libraries only from its own folder, by full path** (`folder::library_path`), and Windows' library
  search narrowed to its folder and System32 at start (`folder::restrict_library_search`). Source: the
  runtime finding, F10 and condition 4 (Microsoft's "specify a fully qualified path").
- **No work before the first message.** The app places it in its job object right after it starts and
  before sending anything, so nothing runs outside the job's memory cap and priority. Source: the
  design, 7.2.
- **Nothing kept on disk; reuse obeys the switch.** `reuse {enabled: false}` turns every in-memory
  reuse off (rung 1's fake: the persona's prefill). Never a saved key-value cache, a pipeline cache or
  a kernel cache. Source: the design, 9.6; CLAUDE.md, the no-caches rule.
- **Built by the project from pinned sources, CPU and Vulkan only.** No CUDA, ROCm or SYCL packages.
  Source: the runtime finding's conditions 1 and 6; the operator's answer to question 2.
- **llama.cpp only through `src/llama/shim.c`, with plain values** (integers, floats, byte buffers,
  opaque handles): no llama.h struct is copied into Rust, so the C compiler decides every layout and
  no bindgen (libclang) is needed. The source is the `llama.cpp/` folder of the crate llama-cpp-sys-2
  pinned exactly in `llama-src/Cargo.toml` (its checksum in `llama-src/Cargo.lock`); bump it on
  purpose, with an engine version. Source: rung 2 (this PC has no LLVM; the commit's message says why
  the `llama-cpp-2` binding itself could not be built).
- **The `llama` feature is never a default.** The app's client tests build this crate with plain
  `cargo build` and must never need CMake or a C++ compiler. Source: rung 1's lesson, the brief of
  rung 2.
- **llama.cpp's libraries use the shared Visual C++ runtime (`/MD`)**, the one HumanityOS.exe already
  needs; linking it into each library (`/MT`) aborted the first model load (0xc0000409), the hazard of
  several libraries each with its own C++ runtime. And the optimiser is passed explicitly (`/O2 /Ob2
  /DNDEBUG`): the cmake crate writes a Release flag set without it under Visual Studio's generator
  (rust-lang/cmake-rs issue 240), which left llama.cpp four times slower. Both in `build.rs`.
- **Keep the build's folder short.** CMake under Visual Studio's generator fails past Windows' 260-
  character path limit ("MSB3191 ... The specified path, file name, or both are too long"): a
  `--target-dir` deep in a temp folder did (2026-11-09). `target/ai-engine/` under a checkout is fine.

## How to test it

- **Its own tests:** `just ai-engine test` (`src/tests.rs`, the line above) runs two halves. The pure
  Rust half (debug, no feature): the fake's hello, load and word-by-word reply; the reuse switch's
  effect on the prefill; a cancel between words; every model request refused in words without
  `--fake`; lines it cannot read refused and the loop going on; the fake's memory; its speech; the end
  of input ending it; the strict command line; library paths; a model's pieces joined into whole
  characters. The llama half (release, `--features llama`, `--nocapture`): the real engine starts with
  the CPU backend and refuses a non-GGUF in words (always runs); a real model answers piece by piece, a
  grammar fixes the reply's shape, a cancel stops a real reply between pieces (these need a model file:
  HUMANITY_AI_TEST_GGUF, else the operator's Granite on E:, and say SKIPPED when there is none); and
  Ministral 3 3B loads and answers with Mistral's template around the project's persona, never the
  GGUF's default system message (HUMANITY_AI_TEST_MINISTRAL, else the operator's copy on E:,
  SKIPPED likewise; 2026-11-11). Not in
  `just verify` (it is not a member of the app's workspace): run it after any change here.
- **`just ai-engine check`**: both builds type-checked, warnings budget 0 (seen red on a scratch
  unused function). **`just ai-engine measure`**: the design's 9.1 to 9.3 on this computer (an
  ignored test, below-normal priority, 4 threads by default).
- **`just ai-engine package`**: the zip `assets-ai-1` carries (stored, sorted, dated 1980-01-01; with
  `licenses/` and a README), its SHA-256 and size in `target/ai-engine/package/engine.json`, which
  `just ai-catalog` reads. Two builds in two folders differ only in a two-byte link stamp of the nine
  `ggml-cpu-*.dll` (derived from the build folder's path); the program, llama.dll, ggml.dll and
  ggml-base.dll are byte-identical (2026-11-09).
- **Driven by the app:** `src/ai/client_tests.rs` (in `just test-lib`, so in `just verify`) builds the
  debug engine into `target/ai-engine/` when it is missing or stale and drives the real program:
  started on demand, in its job object at below-normal priority, ended with the job, the memory cap, a
  crash and the two-failure rule, another protocol version refused, idle stop, the reuse switch. If the
  engine cannot be built, those tests fail and say so.
- **By hand:** Dev > AI in the app (Start, Send a test question, Stop as a crash, Try again).
- `just ai-engine` builds the release program (the real engine, with llama.cpp) into
  `target/ai-engine/release/humanity-ai.exe` with its libraries beside it. `--llama-log` passes every
  line llama.cpp logs to standard error (the app's log), for a developer chasing a failure.

## Open work

- **Vulkan** (the operator's answer to question 2: the helper on the graphics card outside the 3D
  world): needs the Vulkan SDK's shader compiler to build `GGML_VULKAN`, which this PC does not have
  (rung 7, the helper, is where it is needed).
- **A CI job for the engine** (7.1, "built by the project's CI from pinned sources"): deferred. The
  Windows runner would build and test it, but `actions/upload-artifact` or a release step must never
  publish from it by accident, and the zip would differ from the operator's by the build-folder stamp
  above; the operator's `just ai-engine package` is the published one for now.
- **Rung 5:** audio.cpp's C library, Pocket TTS only, loaded through `folder::library_path`. **Rung 8:**
  tract for the label reader. **Rung 9:** AppContainer once Vulkan is tested inside one.
- **Other systems:** on Linux and macOS the app starts it without a job object (no memory cap, priority
  or kill-on-close; it still ends when the app closes its input). Rung 9's hardening.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
