# humanity-ai-engine

## What it is for

The AI engine, `humanity-ai.exe`: the one separate program that will run every AI model a person
chooses, never part of the app. Its design is docs/design/embeddable-ai.md, section 7, and the runtime
finding docs/reference/findings/2026-11-09-model-runtime.md: started by the app only while an AI
feature is in use, talking over its standard input and output (no socket, no listener, no network).
Rung 1 (2026-11-09) is its skeleton with no model and no runtime: `--fake` answers with scripted lines
and timing so every screen, fallback and notice is tested without a model, and without `--fake` it
refuses every model request in words. It has a workspace of its own, outside the app's, so
HumanityOS.exe can never depend on it and the C++ runtimes it will link (llama.cpp, audio.cpp) never
slow the app's build.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-ai-engine/`, a program with a workspace of its own (`scripts/crate-layers.json` gives its `dir` and `"workspace": "own"`: not a member of the app's workspace, built only by its own recipe, and nothing may depend on it); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-ai-engine/src/args.rs`, `crates/humanity-ai-engine/src/fake.rs`, `crates/humanity-ai-engine/src/folder.rs`, `crates/humanity-ai-engine/src/run.rs`
  - test builds only: `crates/humanity-ai-engine/src/tests.rs`
- **Size:** 767 lines in 7 files, 179 of them in 1 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): none.
- **May use** (the plan): ai-protocol. **Uses today** (references in non-test code): ai-protocol 19.
- **May be used by** (the plan): nothing; it is the top of the stack. **Used today by:** nothing.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 12 tests, and this runs exactly them, checked against every test's name:

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

## How to test it

- **Its own tests:** `just ai-engine test` (`src/tests.rs`, the line above): the fake's hello, load
  and word-by-word reply; the reuse switch's effect on the prefill; a cancel between words; every model
  request refused in words without `--fake`; lines it cannot read refused and the loop going on; the
  fake's memory; its speech; the end of input ending it; the strict command line; library paths. Not in
  `just verify` (it is not a member of the app's workspace): run it after any change here.
- **Driven by the app:** `src/ai/client_tests.rs` (in `just test-lib`, so in `just verify`) builds the
  debug engine into `target/ai-engine/` when it is missing or stale and drives the real program:
  started on demand, in its job object at below-normal priority, ended with the job, the memory cap, a
  crash and the two-failure rule, another protocol version refused, idle stop, the reuse switch. If the
  engine cannot be built, those tests fail and say so.
- **By hand:** Dev > AI in the app (Start, Send a test question, Stop as a crash, Try again).
- `just ai-engine` builds the release program into `target/ai-engine/release/humanity-ai.exe`.

## Open work

- **Rung 2** (the design, section 14): llama.cpp through its C library (pinned tag, CPU and Vulkan),
  Granite 4.0 1B as the first model, built by the project's CI from pinned sources and published as
  `humanity-ai-engine-v1-windows-x64.zip` under `assets-ai-1` (the top-level session uploads). Keep a
  pure-Rust `--fake` build that needs no CMake, so the app's client tests never need a C++ compiler.
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
