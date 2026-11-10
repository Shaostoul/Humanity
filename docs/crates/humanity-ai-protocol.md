# humanity-ai-protocol

## What it is for

The messages HumanityOS and its AI engine speak to each other. Per docs/design/embeddable-ai.md, section
7.3 (rung 1, 2026-11-09), that is one JSON object per line over the engine's standard input and output, every
message carrying the protocol version as `"v"`, so a mismatched engine is refused with words instead
of guessed at. Serde types only: no model, no runtime, no file or network access. It is a crate of its
own because the app and the engine (a separate program, `humanity-ai-engine`) must compile the very
same types, and because it is all of AI a person who never chooses AI has in their app.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-ai-protocol/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - test builds only: `crates/humanity-ai-protocol/src/tests.rs`
- **Size:** 453 lines in 2 files, 124 of them in 1 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): none.
- **May use** (the plan): nothing; it is the bottom of the stack. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): ai-engine, gui, engine. **Used today by:** ai-engine 25, gui 14.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 4 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-ai-protocol
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_ai_protocol::`):

| Item | References | From |
|---|---:|---|
| `EngineMessage` | 5 | ai-engine 3, gui 2 |
| `ErrorCode` | 4 | ai-engine 3, gui 1 |
| `ModelKind` | 4 | ai-engine 2, gui 2 |
| `LoadedModel` | 3 | ai-engine 3 |
| `PROTOCOL` | 3 | ai-engine 3 |
| `Turn` | 3 | ai-engine 2, gui 1 |
| `AppMessage` | 2 | ai-engine 1, gui 1 |
| `Backend` | 2 | ai-engine 1, gui 1 |
| `Device` | 2 | ai-engine 1, gui 1 |
| `MAX_LINE_BYTES` | 2 | ai-engine 1, gui 1 |
| `ProtocolError` | 2 | ai-engine 1, gui 1 |
| `Role` | 2 | ai-engine 1, gui 1 |
| `Counters` | 1 | ai-engine 1 |
| `decode_app` | 1 | ai-engine 1 |
| `decode_engine` | 1 | gui 1 |

And 2 more items, 2 references (`node scripts/crate-cards.js --interface humanity-ai-protocol` lists them all).
<!-- end generated -->

## Rules

- **Serde types and their reading only.** No model, no inference library, no file access, no network:
  the app carries this crate for everyone, so nothing here may run a model. Source: the operator,
  2026-11-09, "AI needs to be optional, not required" (CLAUDE.md, Dual modes); the design, 5.1 ("the
  door, not the room"); `tests/no_ai_in_app_lint.rs` holds the app's dependency tree to it.
- **Every message carries `v`, read first.** `decode` refuses another version before it reads the
  rest of the line (`ProtocolError::Version`), so the words name the real reason. Source: the design,
  7.3 ("a mismatched engine is refused with words, never guessed at").
- **Raise `PROTOCOL` on any change of meaning or shape; no compatibility code.** The app and the
  engine come from the same release, pinned by the catalog (7.4). Source: CLAUDE.md, "No
  backwards-compatibility debt before launch".
- **Words are keys.** A refusal names its words key (`ErrorCode::words_key`, `ProtocolError::words_key`);
  the English lives in `data/i18n/en.json` under `ai.error` and `ai.protocol`. Source: CLAUDE.md, One
  word source.
- **Desktop only.** The root package names it under `native`; the relay build never compiles it,
  because the server never talks to an AI engine and never learns that a person has one. Source: the
  design, 8.1.

## How to test it

- **Its lib tests:** the `just test-lib --crate humanity-ai-protocol` line above (`src/tests.rs`):
  every message of 7.3's table round-trips through its line with `v` first; another version is
  refused before the rest is read; lines that are not messages say why; every words key the crate names
  has its English.
- **Both sides together:** the engine's own tests (`just ai-engine test`) and the app's client tests
  (`src/ai/client_tests.rs`, in `just test-lib`), which drive the real engine program over pipes.

## Open work

- **Rung 2** (built 2026-11-09) needed no change here: the app raises the job object's memory cap
  (`AiClient::set_memory_cap`) to the model's need plus a margin before it sends `load`; `load`'s
  `threads` and `context` now carry real values (9.2's brain threads, the catalog's context); `status`'s
  `memory_mb` is the engine's committed memory read from the system; `generate`'s `grammar` is a GBNF
  grammar llama.cpp applies. The catalog (`data/ai/catalog.ron`, compiled in) names the model ids.
- **Rungs 3, 5 and 8** fill `generate` (the fixed output shape in `grammar`), `speak` and
  `read_image` with real runtimes; their shapes are here from rung 1 and may change with `PROTOCOL`.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
