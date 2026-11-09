# Embeddable AI: talking characters, a helper and a computer voice, on your own computer, never required

**Status: plan (rung 0), written 2026-11-09.** Nothing here is built. The app contains no AI model
and no model runtime today (checked 2026-11-09: no llama.cpp, ONNX, GGUF, tract or similar in
`src/`, `crates/` or `web/`). This document is the whole plan, with all its bells and whistles, so
the first slice is a rung of the real thing. The runtime decision it rests on is the dated finding
[`docs/reference/findings/2026-11-09-model-runtime.md`](../reference/findings/2026-11-09-model-runtime.md);
the model research is the 2026-11-08 scan
[`docs/reference/research/2026-11-08-npc-brain-voice/README.md`](../reference/research/2026-11-08-npc-brain-voice/README.md)
and its findings (linked in section 4).

## 0. In plain words

Some things in HumanityOS can be better with AI: characters in the game who answer whatever you ask
them, a helper that answers questions about the app, and a voice that reads text aloud. All of it is
optional. The app comes with none of it. If you want it, you choose it in Settings > AI, the app
downloads it from the project's own release page, checks it, and runs it on your own computer.
Nothing you say to it leaves your computer. If you never choose it, there is no AI on your machine at
all, and everything still works: characters speak their written lines, the label checker takes typed
words or a barcode, and read-aloud uses your computer's own voice or stays off. Wherever AI wrote
something or speaks, the screen says so.

## 1. The operator's decisions (2026-11-09)

Quoted, not added to:

- **"AI needs to be optional, not required."** Now a CLAUDE.md rule (Dual modes): no AI model ships
  inside the app; each is a download the person chooses; every feature that uses one also works
  without it; and the model runtime is a separate download too (the operator: "Agreed."), so a person
  who wants no AI has none of it on their machine.
- **The starting set, approved with "Yes":** Ministral 3 3B Instruct as the NPC brain; Granite 4.0 1B
  for low-memory PCs; Gemma 4 (E2B and E4B) as the in-app helper, and E2B as the head-to-head test
  against Ministral; Pocket TTS as the voice, with its conditions (documented voices only, its
  training data listed in the credits, a "computer voice" label everywhere, a check before each
  release that no volunteer has withdrawn their voice); audio.cpp as the voice runtime, built with
  only Pocket TTS inside.
- **On this document:** "let's create a dedicated doc for it like embeddable_AI.md or something like
  that. In the doc we can include alternatives but, let's just stick with this set to start." (The
  file is `embeddable-ai.md` after the repository's kebab-case convention.)
- **Standing rules that apply** (CLAUDE.md): say plainly on screen when a person's hardware cannot
  run something (what it needs, what they have, what works instead); compare open alternatives for
  every new tool; open source only; compute is a hard budget beside the running game; every deep
  system has a full and a simplified mode; plan the whole feature and build it in rungs.

## 2. What exists today (read from the repository, 2026-11-09)

- **Characters speak written lines.** `data/npc/crew.ron` gives each crew member a name, a post, a
  role, a description, rotating `dialog` lines and `greetings`. The relay sends them in the welcome
  snapshot; the walk-up talk card (`draw_npc_talk_card` in `src/gui/pages/hud.rs`, v0.797) shows one
  line at a time, and "the client displays, it never authors dialogue". `data/dialogues.ron` holds 15
  branching trees with choices that can start quests or set flags; nothing in `src/` loads it yet.
- **The label checker** is designed in [`ingredients-and-labels.md`](ingredients-and-labels.md): rung
  1 takes typed or pasted words, a barcode needs no model, and the photo reader (PP-OCRv6) is an
  optional download (its section 5.2, "AI is optional, never required").
- **Downloads from release assets already work** for instrument packs (`src/instrument_packs/`,
  `data/music/packs.ron`, the `assets-instruments-1` release) and star catalogues (`assets-stars-1`):
  a background thread, progress drawn by a Settings section, a SHA-256 check that refuses a mismatch.
  [`docs/ai/release.md`](../ai/release.md), "Instrument packs", describes the build and the upload.
- **Settings** has 17 sections (`SettingsCategory` in `src/gui/mod.rs`); there is no AI section.
- **No read-aloud** exists on either mirror (no `speechSynthesis` call in `web/`, no speech API use in
  `src/`).
- **Character animation** plans a learned motion model as its last, optional rung, "in our own wgpu
  compute pipelines" ([`character-animation.md`](character-animation.md), sections 8.7 and 10).

## 3. The rules this design follows

| Rule | What it means here |
|---|---|
| AI optional (2026-11-09) | No model, no runtime in `HumanityOS.exe` or `data/`; a lint proves it (section 12) |
| Hardware notice (2026-11-08) | Every model card in Settings checks memory, processor, disk and graphics and says what it needs, what this PC has, what works instead (4.7) |
| Compare alternatives (2026-11-08) | Section 16, from the scan |
| Open source only | Every model and runtime is Apache 2.0, MIT or CC BY 4.0 (weights), each with its dated finding |
| Compute is a hard budget | The brain runs on the processor during play, at reduced priority, inside a memory cap; the game's frame cost is measured (section 9) |
| Two modes | Written and Conversational characters; Quick and Rich brains; voice off, the computer's own, or the computer voice (section 10) |
| Words by key, plain hints, glossary | Every word under `ai.*` in `data/i18n/en.json`; `[[AI]]`, `[[AI engine]]`, `[[computer voice]]` glossary cards (section 11) |
| No caches across runs; reuse obeys the switch | Prompt reuse and voice-state reuse are in memory only and turn off with the Dev page's switch (9.6) |
| GUI-first | Install, remove, test and inspect from Settings and the Dev page; no command line needed |
| Web mirrors native | The website mirrors what a browser can do (5.5) and says plainly what is desktop-only |
| Two realities | The helper lives on the app's pages (your real life); characters live in the game |
| Protect users | Nothing leaves the device; the engine has no network; keys stay out of its reach (sections 7 and 8) |
| Say what we will not build, and why | Section 13 |

## 4. The starting set: what each does, for whom, on what hardware

### 4.1 At a glance

| Model | Job | Who it is for | File (maker's own) | Memory while running (estimate, 4K context) | Where it runs | Speed | Finding |
|---|---|---|---|---|---|---|---|
| **Ministral 3 3B Instruct 2512** | Talking characters ("Rich") | PCs with about 16 GB of memory | `Ministral-3-3B-Instruct-2512-Q4_K_M.gguf`, 2,147,023,008 bytes, text model only | about 2.9 GB | Processor during play | Ceiling about 20 tokens (word pieces) a second on dual-channel DDR4-2666; expected 10 to 13 (estimate, 4.2) | [2026-11-08-ministral-3-3b](../reference/findings/2026-11-08-ministral-3-3b.md) |
| **Granite 4.0 1B** | Talking characters ("Quick") | PCs with 8 GB of memory | `granite-4.0-1b-Q4_K_M.gguf`, 1,023,645,440 bytes | about 1.6 GB | Processor | Ceiling about 42 tokens a second on the same memory; expected 20 to 28 (estimate) | [2026-11-08-granite-4-0-1b](../reference/findings/2026-11-08-granite-4-0-1b.md) |
| **Gemma 4 E2B-it** | The helper on mid PCs; the head-to-head test against Ministral | 16 GB PCs | `gemma-4-E2B_q4_0-it.gguf` (Google's quantization-aware file), 3,349,516,256 bytes, text only | about 3.8 GB | Processor; graphics card outside the 3D world | Measured in rung 4 | [2026-11-08-gemma-4-e2b-e4b](../reference/findings/2026-11-08-gemma-4-e2b-e4b.md) |
| **Gemma 4 E4B-it** | The helper on strong PCs | 16 to 32 GB PCs | `gemma-4-E4B_q4_0-it.gguf`, 5,154,941,280 bytes, text only | about 5.7 GB | Graphics card (Vulkan, any maker) outside the 3D world; processor otherwise | Measured in rung 7 | as above |
| **Pocket TTS** (100M) | The computer voice for characters, the helper and read-aloud | Any PC with 4 or more cores | `pocket-tts-english-q8_0.gguf`, 127,867,968 bytes (the project converts its own, 6.3) | a few hundred MB (measured in rung 5) | 2 processor cores | "~6x real-time on a CPU of MacBook Air M4", "Uses only 2 CPU cores"; "RTF ~2.3-2.5x on CPU" on "a cloud x86 VM (4 vCPUs)" (maker's README, via the scan) | [2026-11-08-pocket-tts](../reference/findings/2026-11-08-pocket-tts.md) |
| **Pocket TTS small** (18M, 7M or 2M) | The voice on the weakest PCs (English) | 2 to 4 cores | `english-18m-refl-f32.gguf` 111,809,536 bytes; `english-2m-refl-f32.gguf` 46,792,352 | less than the full model | Processor | Measured in rung 5 | as above |
| **audio.cpp** (runtime, not a model) | Runs Pocket TTS inside the AI engine | Everyone who chooses the voice | built by the project, Pocket TTS only | small | inside the engine | | [2026-11-08-audio-cpp](../reference/findings/2026-11-08-audio-cpp.md) |

Sizes are the publishers' files on 2026-11-08 (Pocket TTS: audio.cpp's converted packages, listed
2026-11-09). The vision and audio encoders (`mmproj` files) of Ministral and Gemma are never shipped:
characters and the helper need text only.

### 4.2 Ministral 3 3B: the character brain

- **What it does.** Answers a player who types (later, speaks) to a crew member, in that crew
  member's voice and knowledge, in one to three sentences. Mistral calls it a non-reasoning instruct
  model, so replies are short and start quickly (the scan, Part 3).
- **Memory, worked out.** Its configuration (Hugging Face `config.json`, read 2026-11-09) has 26
  layers, 8 key-value heads and a head size of 128. Each token of conversation kept in its working
  memory (the key-value cache) costs 2 x 26 x 8 x 128 x 2 bytes = 106,496 bytes at 16-bit precision,
  so a 4,096-token conversation costs about 0.44 GB, beside the 2.15 GB file and a few hundred MB of
  working buffers (estimate). About 2.9 GB in all. An 8-bit cache halves the 0.44 GB.
- **Speed, worked out (an estimate, not a measurement).** A model of this kind reads all its weights
  once per token, so memory speed sets a ceiling: dual-channel DDR4-2666 moves at most about 42.7
  GB/s (2,666 million transfers x 8 bytes x 2 channels), and 42.7 / 2.15 is about 20 tokens a second.
  Processors reach perhaps half to two thirds of that ceiling, and the running game shares the same
  memory, so 10 to 13 tokens a second is the planning figure. A 40-token reply then takes 3 to 4
  seconds, shown word by word as it comes. Rung 4 measures the truth on the operator's PC and a
  floor-class PC.
- **The persona prompt is reused.** A character's description and rules (several hundred tokens) are
  read once when the conversation opens; each new turn only adds the player's words. This reuse is in
  memory, obeys the Dev page's "turn off all reuse" switch, and is never written to disk (9.6).
- **Its own system prompt never ships.** The repository's `SYSTEM_PROMPT.txt` begins "You are
  Ministral-3-3B-Instruct-2512, a Large Language Model (LLM) created by Mistral AI" and "You power an
  AI assistant called Le Chat." (the Ministral finding, F4). A character running that would claim to
  be Mistral's product. The project writes its own (8.2).

### 4.3 Granite 4.0 1B: the brain for low-memory PCs

- **What it does.** The same job as Ministral, more plainly, on 8 GB PCs. IBM publishes the datasets
  that trained it (the Granite finding, F3).
- **Memory.** Its configuration has 40 attention layers, 4 key-value heads and a head size of 128
  (2,048 hidden / 16 heads): 81,920 bytes a token, about 0.34 GB for 4,096 tokens, about 1.6 GB in all.
- **Speed.** The same arithmetic gives a ceiling of about 42 tokens a second; 20 to 28 is the planning
  figure.

### 4.4 Gemma 4 E2B and E4B: the helper, and the head-to-head test

- **The helper** answers questions about HumanityOS itself ("how do I back up my recovery phrase?",
  "where are the instrument packs?"), from the app's own words: the help topics
  (`data/help/topics.json`), the Library (`data/library/`), the glossary (`data/glossary.json`), the
  page registry and the app's action registry (the "data-driven registry the GUI renders AND an AI can
  enumerate" of CLAUDE.md's GUI-first rule). It finds the passages by plain word search (no extra
  model), answers from them, links each source, and offers "Open this page" as a link the person
  clicks. It never changes a setting itself (8.5).
- **E4B on strong PCs, E2B on mid PCs.** Google's quantization-aware files are 5.15 GB and 3.35 GB.
  The helper lives on the app's pages, not in the 3D world, so it may use the graphics card through
  Vulkan (any maker) when the world is not being drawn, and the processor otherwise.
- **Memory.** In E2B's `config.json`, 28 of its 35 layers use a 512-token sliding window and 20 reuse
  another layer's key-value cache (`num_kv_shared_layers`), so its cache is smaller than its size
  suggests. Measured in rung 4.
- **The head-to-head test** (rung 4): E2B against Ministral as the character brain, on the same
  battery (4.2's measures plus persona, refusals, loaded prompts and other languages).

### 4.5 Pocket TTS through audio.cpp: the computer voice

- **What it does.** Speaks a character's line, the helper's answer, or a Library page, in one of a
  set of documented voices, in English, French, German, Portuguese, Italian or Spanish (the card's
  languages, the Pocket TTS finding F1). It reads text with SentencePiece, so no eSpeak (GPL) is
  needed (F6).
- **The conditions, as approved:**
  1. **Documented voices only.** The set in the finding's judgement, kept in `data/ai/voices.ron`
     with each voice's source file, set, licence and credit line: Kyutai's own recordings (CC0,
     except the three exceptions the voice repository names), the volunteer donations (CC0),
     Alba MacKenna's "Casual", "Merchant" and "Announcer" (CC BY 4.0; not "A Moment By"), the
     Voice-Zero LibriVox readers (CC0) and VCTK voices (CC BY 4.0). Never Expresso, EARS,
     `degaulle-2` or the gated-branch voices ("cosette", "jean", "giovanni", "lola", "juergen",
     "rafael").
  2. **The training data listed in the credits**: Kyutai's January 2026 list (AMI, EARNINGS22,
     GigaSpeech, SPGISpeech, TED-LIUM, VoxPopuli, LibriHeavy, Emilia) and the plain note that later
     versions' data is not published.
  3. **A "computer voice" label everywhere it speaks** (5.4).
  4. **A check before each release** that no shipped donated voice has been withdrawn (12.4).
- **The ungated weights** (`kyutai/pocket-tts-without-voice-cloning`) are used, so nobody can make a
  character speak in a neighbour's voice.
- **Speed.** The maker's figures above put it 2.3 to 6 times faster than real time on 2 cores; the
  engine streams sentence by sentence, so speech starts while the rest is made. The target beside the
  game is in 9.4.
- **audio.cpp** is built by the project with only `pocket_tts` and `pocket_tts_small`, eSpeak off,
  its model manager off, and its C library on (the runtime finding's condition 1).

### 4.6 Other models that will use the same engine (not part of the starting set)

- **The label reader's photo model** (PP-OCRv6, ingredients 5.2 and its rung 2), through tract inside
  the engine, with ONNX Runtime as the fallback (the runtime finding).
- **A learned motion model** (character animation rung 7), if one ever earns its place: on the
  engine's own wgpu path inside the frame, with its shaders and weights in its optional pack
  (question 4).

### 4.7 Saying plainly when a PC cannot run something

The app reads: total and free physical memory, processor cores and instruction sets (AVX2 and
higher), free disk space in the data folder, and the graphics adapter's maker and memory (wgpu
already reports the adapter). Each model card in Settings > AI shows three lines: what it needs, what
this PC has, what works instead. Words by key; numbers filled in.

| Situation | What the screen says | What works instead |
|---|---|---|
| Too little free memory for Ministral | "Talking characters (Rich) needs about 3 GB of free memory while you play. This PC has {free} GB free of {total} GB." | "Talking characters (Quick) needs about 1.6 GB." Or written lines |
| Too little for Granite too | "Talking characters needs about 1.6 GB of free memory while you play. This PC has {free} GB free." | "Characters will speak their written lines." |
| A processor without AVX2 | "This processor is missing a feature (AVX2) that makes AI fast. It will work, but slowly: a reply may take {n} seconds." | Quick, or written lines |
| Fewer than 4 cores | "The computer voice needs 2 of this PC's {cores} cores, which would slow the game." | The small voice, the computer's own voice, or subtitles only |
| Not enough disk | "This needs {size} of free space in {folder}. There is {free}." | Free space, or choose a smaller model |
| The helper's graphics card route | "The helper can use your graphics card when you are not in the 3D world. Yours ({adapter}, {vram} GB) {can, cannot} hold it." | The processor (slower), or E2B |
| A reply too slow in practice (measured live, 9.3) | "Replies are taking about {n} seconds on this PC." | "Try Quick", or written lines |

The checks run before the download is offered and again before each load; a model that no longer
fits (another program took the memory) is not loaded, and the screen says why.

## 5. AI optional, concretely

### 5.1 With nothing installed

What the app contains with no AI: the Settings > AI section, the downloader, the client that would
start the engine, the hardware checks, the labels and words, and every feature's non-AI path. No
model, no inference library, no network call to any AI service. The door, not the room.

| Feature | Without AI (always there) | With AI (chosen) |
|---|---|---|
| Talking to a crew member | The talk card's written lines (`data/npc/crew.ron`); later the branching trees in `data/dialogues.ron` with their choices | A text box beside the written choices: ask anything; the character answers in its own voice and knowledge |
| A character's voice | Subtitles only | The computer voice, one documented voice per character |
| Help with the app | Help topics, the Library, the glossary, search | The helper, answering from those same pages and linking them |
| Reading a page aloud | Off, or the computer's own speech (Windows' built-in voices; the browser's local voices on the website, 5.5) | The computer voice |
| Checking a food label | Typed or pasted words, or a barcode (plain arithmetic) | A photo, read on this device (ingredients 5.2) |

### 5.2 Installing, from Settings

1. **Settings > AI** (a new `SettingsCategory::Ai`). The top line, when nothing is installed: "No AI
   is installed. Everything in HumanityOS works without it." Below it, "Your computer" (memory,
   processor, disk, graphics) and one card per feature: Talking characters, Helper, Computer voice,
   Photo reader (when that rung lands).
2. **A card** names the model, its maker, its job, its size, its licence (a link that opens the full
   text), what it needs and what this PC has (4.7), and one button: Get. Each model's finding asks
   for exactly this ("The download screen names the model, its size (2.15 GB), its maker and its
   licence before the person agrees", the Ministral finding).
3. **Get** downloads, in the background with progress, the model and, the first time, the AI engine
   (its size added to the total shown before the click). Both are checked against the catalog (6.2);
   a mismatch is refused and said in words ("The download did not match what we published, so it was
   deleted. Try again later.").
4. **Where the person meets a feature**, a small link offers it once: the talk card says "Want
   {name} to answer anything you ask? Talking characters (2.2 GB) in Settings > AI." Dismissed, it
   stays dismissed. No pop-ups during play.

### 5.3 Removing

- **Each model**: Remove on its card deletes its folder; a feature that used it returns to its non-AI
  path at once.
- **Remove all AI**: one button that lists exactly what will be deleted (each model, the engine, their
  sizes) and the plain words "You can download them again later." It deletes `ai/` in the data folder
  and clears the AI settings. Afterwards the section shows the "No AI is installed" line again, and a
  test checks the folder is gone.
- **The engine** goes when the last model goes, or with Remove all AI.

### 5.4 The labels, and where they appear

| Label (by key) | Where |
|---|---|
| **AI** (a small tag with a hint: "Written by AI on this computer, not by a person") | On every character line the brain wrote, on the talk card and in subtitles; on every helper answer; on the Settings cards |
| **computer voice** (a tag with a hint: "This voice is made by the computer from a recording someone allowed us to use") | In subtitles whenever the computer voice speaks; beside the read-aloud control; in each voice's line in Settings, with its credit |
| Written lines | No tag: they were written by people |

A character asked sincerely whether it is a person says it is a character run by AI on the player's
computer (8.3). The labels also meet the "appropriate manner" disclosure the EU AI Act asks of
generated audio in a fictional work, where it applies (the Pocket TTS finding, F8).

### 5.5 The website

- **No brain or voice on the website** at first: 1 to 5 GB downloads into a browser tab are not a
  fair default, and the website loads no WebAssembly today. The website's Settings shows the same AI
  section saying which features are desktop-only and why (the Dual-UI parity rule).
- **Read-aloud on the website** uses the browser's own speech, but only voices that run on the
  device. The Web Speech API (Draft Community Group Report, 27 October 2026,
  https://webaudio.github.io/web-speech-api/) defines `localService` as "true for voices supplied by a
  local speech synthesizer, and is false for voices supplied by a remote speech synthesizer service."
  A remote voice would send the text away, so the website lists only `localService` voices and says
  so.
- **The label reader's photo model** on the website follows ingredients rung 3, with the same tract
  code compiled to WebAssembly if the spike chooses tract (question 5).

## 6. Where downloads come from

### 6.1 The project's own release assets

Everything AI is served from GitHub release assets on the project's repository, under a non-`v*` tag
(`assets-ai-1`), the instrument packs' and star catalogues' way: no workflow runs on upload, files
come from GitHub's CDN, and a first use never tells a model publisher who is playing (the Ministral
finding's reason). **Producing the files is an agent's job; uploading them is the top-level
session's** (CLAUDE.md standing permissions, 2026-11-07), always with `--latest=false` so
`releases/latest` stays the newest game release ([`docs/ai/release.md`](../ai/release.md)). A
dispatched agent never uploads.

GitHub caps each release file: "Each file included in a release must be under 2 GiB" (the runtime
finding, F10). Ministral fits by 460,640 bytes; Granite, the engine and Pocket TTS fit easily; Gemma 4
E2B goes up in two parts and E4B in three.

### 6.2 The catalog

`data/ai/catalog.ron`, written by a script from a recipe (as `scripts/instrument-packs.js` writes
`packs.ron`), never by hand, and **compiled into the app at build time**, so the signed release
vouches for every hash in it. A mod cannot point the app at a different model. (A dev aid,
`HUMANITY_AI_CATALOG`, reads a local file instead, the way `HUMANITY_INSTRUMENT_PACKS_URL` serves
local packs.) The shape, by example:

```ron
AiCatalog(
    release: "assets-ai-1",
    base_url: "https://github.com/Shaostoul/Humanity/releases/download/assets-ai-1/",
    engine: (
        version: "1",
        file: "humanity-ai-engine-v1-windows-x64.zip",
        bytes: 0,            // filled by the build
        sha256: "",          // filled by the build
        sources: ["llama.cpp <tag>", "audio.cpp v0.9.4", "tract 0.23.x"],
        licences: "ai.licence.engine",
    ),
    models: [
        (
            id: "ministral-3-3b",
            job: "characters",
            tier: "rich",
            words: "ai.model.ministral",
            maker: "Mistral AI",
            licence: "ai.licence.apache2",
            finding: "docs/reference/findings/2026-11-08-ministral-3-3b.md",
            upstream: "https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-GGUF",
            upstream_sha256: "9ed150d4367e68df0ac8e1540f6ddc65b42d0ee26378329d1ecbca60f93fc5f8",
            file: "Ministral-3-3B-Instruct-2512-Q4_K_M.gguf",
            bytes: 2147023008,
            parts: [],       // one part when under 2 GiB
            needs: (memory_mb: 2900, cores: 4, disk_mb: 2200),
        ),
        // granite-4-0-1b, gemma-4-e2b (two parts), gemma-4-e4b (three parts), pocket-tts-en, ...
    ],
    voices: "data/ai/voices.ron",
)
```

- **Hashes.** Each file's SHA-256 and size, the same check as the instrument packs. For a file taken
  unchanged from its publisher, the catalog also records the publisher's own hash (the Hugging Face
  storage id is a SHA-256), so anyone can compare the project's copy with the maker's: Ministral
  `9ed150d4...fc5f8` and Granite `22ec0f9c...31d7f1` are in their findings.
- **Parts.** A file over 2 GiB is listed as parts, each with its own hash; after joining, the whole
  file is checked against the whole-file hash (which, for Gemma, is Google's).
- **Each model's folder** carries its licence text and a short credit README, as its finding wrote
  them (Apache 2.0 for the brains; CC BY 4.0 for Pocket TTS and Mimi with the training-data line).

### 6.3 Who makes the files

- **Brains**: copied unchanged from the publishers' own repositories at pinned revisions; the
  project's copy must hash to the publisher's.
- **Pocket TTS**: converted by the project with audio.cpp's converter from Kyutai's pinned revision of
  the ungated weights, so the credits name exactly what was converted (the audio.cpp finding's
  judgement); the result is checked against audio.cpp's own package of the same revision where one
  exists.
- **The engine**: built by the project's CI from pinned source tags (7.1), never taken from upstream
  release archives.

## 7. The engine: one program, its protocol and its limits

The runtime finding's recommendation, as the design takes it (the operator's question 1).

### 7.1 What it is

`humanity-ai.exe`, a small Rust program the project builds, in `%APPDATA%\HumanityOS\ai\engine\<version>\`
with its libraries and licence files beside it. Inside: llama.cpp (through its C library) for the
brains; audio.cpp (its C library, Pocket TTS only) for the voice; tract, compiled in, for the small
ONNX models. Portable processor kernels and a Vulkan backend; no CUDA, ROCm or SYCL packages (the
vendor packages are larger than the models and serve one maker each). Its source lives in the
workspace as its own binary crate (`crates/humanity-ai-engine/`), with the message types in a tiny
shared crate (`crates/humanity-ai-protocol/`, serde types only); `HumanityOS.exe` depends on the
protocol crate and never on the engine crate (a lint holds this, 12.3). Each gets its crate card in
`docs/crates/`. The engine needs CMake and a C++ compiler for llama.cpp and audio.cpp, so it builds
only through its own recipe and CI job, never in `just verify`'s ordinary build.

### 7.2 How the app runs it

- **Started only when needed**: when a character conversation opens, the helper is asked, or a voice
  is needed; stopped after a few minutes idle, and always when the app closes.
- **Pipes, not ports.** The app starts it with its standard input and output connected; messages are
  one JSON object per line. No socket, no listener, so no firewall question and no other program on
  the PC can talk to it. The engine needs no network at all: the app downloads and checks every file
  and hands the engine paths.
- **Contained.** Started inside a Windows job object that kills it with the app and caps its memory;
  at below-normal priority, so the game's threads win; libraries loaded from its own folder by full
  path (Microsoft's guidance, the runtime finding F10). Running it inside an AppContainer (no network,
  no files beyond its folder) waits on a test that Vulkan still works there (rung 9).
- **Crash and out-of-memory.** If the engine dies, the app notes it, falls back at once to written
  lines (or subtitles, or no read-aloud), and offers "Try again". Two failures in a row in one session
  stop automatic restarts and say so. The game never stops because of the engine.

### 7.3 The protocol (version 1)

| From app | Meaning | Engine answers |
|---|---|---|
| `hello {protocol, app_version}` | Start | `hello {engine_version, backends: [cpu, vulkan], cpu_features}` |
| `load {id, path, sha256, kind, context, threads, device}` | Load a model already checked by the app | `loaded {id, memory_mb}` or `error {code, words_key}` |
| `generate {id, request, persona, turns, grammar, max_tokens}` | A character line or helper answer | `token {request, text}` many times, then `done {request, tokens, ms_prefill, ms_generate}` |
| `speak {voice, text, request}` | Speech for a sentence | `pcm {request, samples}` chunks (24 kHz mono 16-bit is 48 KB a second), then `done` |
| `read_image {id, request, png}` | The label reader | `text {request, lines, confidence}` |
| `cancel {request}` | The player walked away | `cancelled` |
| `reuse {enabled}` | The Dev page's switch | `ok` |
| `status` | The Dev page | memory, threads, loaded models, counters |
| `unload {id}`, `shutdown` | | `ok` |

Every message carries the protocol version; a mismatched engine is refused with words, never guessed
at.

### 7.4 Updates

The engine and each model are pinned in the catalog of an app release. A new engine version arrives
only with an app release that names it, never automatically from upstream; Settings shows "An update
to the AI engine is ready: {what changed}, {size}". A security fix in llama.cpp, audio.cpp or tract
is a reason for that release (llama.cpp now takes security fixes only as public patches, so a flaw
is public as soon as its fix is proposed; the runtime finding F2).

## 8. Privacy and safety

### 8.1 Nothing leaves the device

- The engine has no network. Prompts, replies, voices and photos stay on the PC.
- No telemetry about AI use, ever. The relay never learns that a person has AI installed.
- **Character conversations** are cosmetic and local: never sent to the relay or to other players
  (the "cosmetic simulation is never networked" rule). Whether a character remembers past
  conversations is question 6.
- **Helper conversations** live in memory and are gone when the app closes, unless the person keeps
  one (question 7).

### 8.2 The project's own prompts

- Prompts live in `data/ai/prompts/` as reviewed text files (one per job: character, helper), each with
  a version and a test battery (12.2). A character's persona is built from its row in
  `data/npc/crew.ron` (name, role, description, post, its written lines as examples of its voice) and
  live facts the client already has (what it is doing now, where it is).
- Every prompt says plainly what the model is: a character in HumanityOS run by AI on the player's
  computer. None names a model maker as the speaker; the maker's name appears only in the credits,
  "describing the origin of the Work" (Apache 2.0, section 6).

### 8.3 What characters may and may not say

**May:** talk in character about the ship, their work, their post, the people and places they know,
what they are doing, and the player's questions about the game world; decline gently; say "I don't
know".

**May not, enforced by the prompt, the output shape and a filter:**
- Change anything in the game. A brain's line is words only. Quests, items, flags and trades come
  only from the written choices (`dialogues.ron`) and the game's rules; the free-text reply is
  generated in a fixed shape (llama.cpp's "Schema-constrained JSON response format"): `{line, mood,
  gesture}` from fixed lists, nothing else, and anything outside the shape is discarded.
- Claim to be a person when sincerely asked, or claim to be a company's product.
- Give medical, legal, financial or safety advice, or real-world facts about food, plants, medicine or
  survival as fact. The project's own record: 48 of 60 researched species records were refuted on
  review. A character that is asked such a thing points to the Library page on it ("There's a guide on
  that in the Library"), which carries sources. **The brain may phrase; it never sources.**
- Sexual content, harassment, slurs, threats, encouragement of self-harm, persuasion about real-world
  politics, or role-playing real people.
- Break the account words (it never says "seed phrase" or "no account"; the terminology rules apply to
  what a person reads).

A deterministic filter after generation (a short list of patterns and a length cap) replaces any line
that breaks a rule with one of the character's written lines; the Dev page counts how often. The
written choices stay visible under the text box, so the conversation always has a way forward.

### 8.4 Real-world facts in the helper

The helper answers only from the app's own pages it retrieved, links each one, and says "I could not
find that in HumanityOS's help" when nothing matches. It does not answer from the model's general
knowledge about the outside world.

### 8.5 Prompt injection

Where text from someone else could reach a model, and what stops harm:

| Path | Exposure | Guard |
|---|---|---|
| The player types to a character | Their own words, to their own copy | Nothing to protect from: the output is words on their own screen, in a fixed shape |
| Another player's words reaching your character (if characters ever repeat what others said) | Another person steering your character | Not built. If it ever is, others' words go in as quoted data, never instructions, and only into cosmetic lines |
| The helper reading chat, mail or web pages for you ("summarise this") | Instructions hidden in someone else's message | Not in the starting set. If built: only on the person's command, the content marked as quoted data, and the helper still cannot act |
| The helper reading the app's own pages | Our own text | Reviewed in the repository like code |

The deeper guard is that **no model can act**: the engine has no network, no files beyond its folder,
no tools, and the app never executes model output. The helper's "Open this page" is a link the person
clicks; it never toggles a setting, sends a message or spends anything. AI suggests; the person
clicks.

### 8.6 The voice

- No cloning: the ungated weights, and no reference-recording feature in the app.
- Documented voices only (4.5), each credited in Settings and on the Credits page.
- The release check for withdrawn donated voices (12.4). Characters' default voices come from
  non-donated sets, so a withdrawal never silences a character mid-game (question 11).

### 8.7 Laws not settled here

Whether the EU AI Act's Article 50 reaches the project, and whether a game character's voice can be a
"deep fake" under its Article 3(60), are lawyer's questions left open in the Pocket TTS and
2026-11-06 findings. The labels in 5.4 satisfy the fictional-work disclosure either way;
machine-readable marking (Article 50(2)) stays open with the "provider" question.

## 9. Budgets beside the running game

Targets the design is held to; each rung measures, never assumes.

### 9.1 Memory

| On a PC with | Allowed for AI during play | Brain | Voice |
|---|---|---|---|
| 32 GB (the operator's) | up to 4 GB | Ministral (2.9 GB) | Pocket TTS |
| 16 GB | up to 3.5 GB, and only if 2 GB stays free beside it | Ministral, or Quick if free memory is short | Pocket TTS |
| 8 GB | up to 2 GB | Granite (1.6 GB) | Pocket TTS small |

The job object's memory cap enforces the number; the cap is the model's need plus a margin from
the catalog.

### 9.2 Processor

- During play the brain uses at most (physical cores minus 2), and never more than 4, threads; the
  voice 2; all at below-normal priority. On the operator's 6-core i7-8700K that is 4 and 2, leaving the
  game its own (and the operator's development share rule is separate, for development tools).
- **No graphics card during play.** The renderer keeps the GPU; the helper may use it through Vulkan
  only when the 3D world is not drawn.

### 9.3 Time

- **Character reply**: the first words within 1 second of the player pressing Enter (the persona
  already read, 4.2), the whole 40-token reply within 4 seconds, on the operator's PC; on a floor PC
  the measured times are shown in Settings, and replies slower than 6 seconds trigger the plain notice
  in 4.7.
- **Helper**: the first words within 2 seconds on a strong PC.
- **Live measurement**: the engine reports prefill and generation times with every reply (7.3); the
  app keeps a running average for the notice, in memory only.

### 9.4 The voice's real-time factor

At least 1.5 times faster than real time on 2 threads beside the game on the floor PC (the maker's
figures suggest 2.3 to 6), with speech starting within 300 ms of the first sentence being ready. Below
that, the small model is offered by name.

### 9.5 The game's frame

The AI may cost the game at most 5% of its frame time (CLAUDE.md: a waste of 5% or more is worth a
fix), measured by a rig that compares the frame time with the engine idle and with it generating and
speaking (12.1). If it costs more, threads drop first, then the brain tier.

### 9.6 Reuse, and the no-caches rule

- **In memory, obeying the switch**: the persona prompt's key-value cache (llama.cpp's prompt reuse,
  on by default in its server, "Re-use KV cache from a previous request if possible") and audio.cpp's
  prepared voice states (`pocket_tts.voice_state_cache_slots`, default 4, "zero disables reuse"). The
  app sends `reuse {enabled: false}` when the Dev page's "turn off all reuse" switch is on
  (`HUMANITY_NO_REUSE=1` for rigs), and the engine then sets prompt reuse off and voice-state slots to
  0. Correct, just slower.
- **Never on disk**: no saved key-value caches (llama.cpp's `--slot-save-path` stays unset), no
  pipeline caches, no compiled-kernel caches. Model files are downloaded content, not caches.

### 9.7 Disk and network

- Disk: the engine (tens of MB; measured when first built) plus each model chosen (table 4.1).
- Network: downloads only, when the person clicks Get. Nothing at run time.

## 10. Two modes

- **Characters: Written and Conversational.** Written is the authored lines and choices, always there,
  the only mode with nothing installed, and a mode a person can pick even with a brain installed.
  Conversational adds the text box. Gameplay is identical in both.
- **The brain: Quick and Rich.** Quick is Granite (short, plain, light); Rich is Ministral (or E2B if
  the head-to-head test chooses it). Settings picks by hardware and the person can override.
- **Voice: Off, the computer's own, the computer voice.** Off shows subtitles; the computer's own is
  Windows' or the browser's local voices (no AI download); the computer voice is Pocket TTS.
- **Helper: Search and Ask.** Search is the existing help and Library search; Ask adds the helper.

## 11. Words, hints and the glossary

- Every word under `ai.*` in `data/i18n/en.json`, added with `node scripts/i18n-add.js` in the rung
  that draws it; every control with a plain hint by key.
- Glossary cards (`data/glossary.json`): `[[AI]]` ("A computer program that writes or speaks by
  predicting what comes next, learned from a great deal of text or speech. In HumanityOS it runs only
  on your computer, and only if you choose it."), `[[AI engine]]`, `[[computer voice]]`, `[[model]]`.
- The account words rule holds in prompts and filtered output (8.3).

## 12. Dev tooling (permanent)

Never trimmed as cleanup (CLAUDE.md, "Forever development"):

### 12.1 On the Dev page and in rigs

- **An AI section on the Dev page**: the engine's state (running, process id, memory against its cap,
  threads, priority, version), loaded models with their hashes, live counters (prefill and generation
  tokens a second, voice real-time factor, filter replacements, fallbacks), the last few prompts and
  outputs of this session (memory only, cleared on close), buttons to stop the engine (to test crash
  recovery), "pretend this PC has {n} GB free" (to see every hardware notice), and the reuse switch's
  effect shown live.
- **A fake engine**: `humanity-ai.exe --fake` answers with scripted lines and timing, so every screen,
  fallback and notice is tested in CI and rigs without any model.
- **A rig, `ai-talk`**: boots the game (one instance machine-wide, CLAUDE.md), opens a talk card,
  sends a fixed question, and records frame time with the engine idle and generating; the 5% budget
  (9.5) fails it.
- **A dev MCP tool** (`game_ai_status` beside the other `game_*` tools, `docs/ai/mcp.md`) reading the
  same status as the Dev page.

### 12.2 The battery

`just ai-battery` runs a fixed set of prompts against the engine directly (no game): persona
consistency over ten turns, refusal rate on ordinary game lines, politically loaded and
"are you a person" questions, a real-world safety question that must be deflected to the Library,
Spanish and Hindi quality, time per 40-token reply, memory. Each prompt file change and each model
change reruns it; results go in `data/ai/battery/` with the date, for the operator to read side by
side.

### 12.3 Lints and checks

- **`tests/no_ai_in_app_lint.rs`** (in `just lints`): fails if `HumanityOS.exe`'s own dependency tree
  (the root package's `Cargo.toml`, followed through workspace members) names llama.cpp, audio.cpp,
  tract, ONNX Runtime, candle or burn, or if `data/` or `assets/` holds a `.gguf`, `.onnx` or
  `.safetensors` file. Seen red first by adding a dependency in a scratch branch.
- **`scripts/check-ai-catalog.js`** (in `just preflight`): every catalog entry has a hash, a size, a
  licence key, a credit row in `data/credits.ron`, a finding that exists, and a part list summing to
  the whole; every voice in `data/ai/voices.ron` has a source, a set and a licence, and none comes from
  a forbidden set.
- **A labels test**: every place that shows brain or voice output draws the `ai.tag` or
  `ai.computer_voice` key (golden snapshots of the talk card and the helper).

### 12.4 The voice-consent check before each release

`scripts/check-voice-donations.js` lists `voice-donations/` in `kyutai/tts-voices` through the Hugging
Face API and compares it with the donated voices in `data/ai/voices.ron`; a shipped voice that is gone
upstream fails the check, and the release skill runs it. The voice is then dropped from the catalog
and from the next voice package.

## 13. What we will not build, and why

Said plainly where a person would look for it, naming the kind of reason (CLAUDE.md, 2026-09-18):

- **AI that runs on someone else's computer** (a cloud service). A choice: it would send what people
  say to a company, which the privacy promise rules out.
- **AI included with the app.** A choice, the operator's: "AI needs to be optional, not required."
- **Cloning a voice from a recording.** A choice, backed by Kyutai's own "Prohibited use" ("voice
  impersonation or cloning without explicit and lawful consent"): nobody should be able to make a
  character speak in a real person's voice.
- **AI that acts for you** (changing settings, sending messages, buying). A choice: AI suggests, the
  person clicks.
- **Meta's Llama and Liquid AI's LFM models.** A licence reason: their licences are not open source
  (Llama's carries a use policy and a 700-million-user clause; LFM's ends commercial rights at USD 10
  million revenue), per the scan.
- **Models from Alibaba (Qwen and models built on it), iFlytek, Zhipu, SenseTime and others on US
  government lists.** A choice, not a law: nothing read forbids running them, but the project will not
  build its characters on companies the US government names, when equally open models exist (the
  scan, Part 0).
- **NVIDIA-only or AMD-only engine packages.** A choice: Vulkan reaches every maker's card, and the
  vendor packages are larger than the models.

## 14. The rungs, in build order

Each rung is a slice of the real thing.

0. **This document and the runtime finding.** (Done, 2026-11-09.)
1. **The engine skeleton and the door, with no model.** The engine crate with its protocol and the
   fake mode; the app's client (start, pipes, job object, priority, restart and fallback); Settings >
   AI with "No AI is installed", "Your computer" and the hardware checks; the no-AI-in-app lint; the
   Dev page section; words by key. Proves the plumbing and the no-AI path before any download exists.
2. **Catalog, downloads, checks and removal.** `data/ai/catalog.ron` compiled in, the `assets-ai-1`
   release (the top-level session uploads), parts and joining, the licence on the card, Remove and
   Remove all AI; the first real content: the engine with llama.cpp and Granite 4.0 1B, the smallest.
3. **Talking characters.** The talk card's text box beside the written lines; personas from
   `crew.ron`; the project's prompt; the fixed output shape; the filter; the AI tag; fallbacks;
   Ministral added; local only.
4. **The head-to-head test.** The battery and the `ai-talk` rig on Ministral, Granite and Gemma 4 E2B,
   on the operator's PC and a floor-class PC; the operator reads the results side by side; defaults
   per hardware tier chosen from them.
5. **The computer voice.** audio.cpp (Pocket TTS only) in the engine; `data/ai/voices.ron` with every
   voice's provenance; the project's own conversion; the consent check in the release skill; the
   "computer voice" label; a voice per character; streaming per sentence; the small model for weak PCs.
6. **Read-aloud.** Library pages and the label checker's result read by the computer voice; the
   computer's own voice as the no-AI option (Windows' speech; the website's local voices).
7. **The helper.** Gemma 4 E4B (E2B on mid PCs), word search over the help, Library, glossary, pages
   and action registry, sources linked, "Open this page"; the graphics card route outside the 3D world.
8. **The photo reader on the engine** (ingredients rung 2): the tract spike against ONNX Runtime on
   PP-OCRv6 decides which runs it.
9. **Hardening.** AppContainer with Vulkan tested; memory caps tuned from measurements; per-load hash
   timing on the floor PC; crash and out-of-memory drills from the Dev page.
10. **More languages.** Pocket TTS's six languages with documented voices per language; prompts and
    personas in the app's other languages.
11. **A project-trained voice** (question 10): Kyutai's MIT training code on documented data, so the
    voice that speaks for HumanityOS carries no question about what it learned from.
12. **The learned motion model's pack** (character animation rung 7), on the wgpu path (question 4).

## 15. Questions for the operator, each with a recommendation

1. **May the AI run in one separate program the project builds** (one download, its own folder,
   started only while AI is in use, talking through pipes with no network), containing llama.cpp,
   audio.cpp with only Pocket TTS, and tract, with ONNX Runtime as the fallback? *Recommendation: yes
   (the runtime finding): it keeps a model file away from the person's keys, keeps the game running if
   a model crashes, and makes "remove AI" one folder.*
2. **The graphics card**: processor only during play, Vulkan for the helper outside the 3D world, and
   no NVIDIA-only or AMD-only packages? *Recommendation: yes; the renderer keeps the GPU while you
   play, and Vulkan reaches every maker.*
3. **Hosting**: the project's GitHub release assets (`assets-ai-1`), with Gemma's files split into
   parts under 2 GiB? *Recommendation: yes; the same channel as the instrument packs, and no model
   publisher learns who is playing.*
4. **The learned motion model**: if one is ever adopted, its network runs on the engine's own GPU
   path, with its shaders and weights inside its optional pack, so a person without the pack has no AI
   on their machine. Does that meet the rule? *Recommendation: yes; the engine keeps only the generic
   GPU plumbing it already uses for skinning.*
5. **The website's photo reader**: the same tract code compiled to WebAssembly, instead of
   onnxruntime-web, if the spike chooses tract (a change to ingredients 5.2); brains and voice
   desktop-only for now? *Recommendation: yes; one code path gives the same answer on both mirrors.*
6. **Character memory**: may a character remember past conversations with you, kept in your save on
   your computer, visible and erasable on its card, never synced? *Recommendation: yes, off until the
   battery shows it stays in character.*
7. **Helper conversations**: kept only until the app closes, unless you press Keep? *Recommendation:
   yes.*
8. **Conversational on by default** for every crew member once a brain is installed, with the written
   choices always shown? *Recommendation: yes; a person who installed it wants it, and Written stays
   one click away.*
9. **Your own model** ("bring your own GGUF"): *Recommendation: not now ("let's just stick with this
   set to start"); revisit after v1.0, and if ever, behind a plain "not checked by HumanityOS" label.*
10. **A project-trained voice**, the Pocket TTS finding's question: train our own Pocket TTS on
    documented data (Kyutai estimates under USD 200 of rented hardware, "an order of magnitude less if
    you just pay for power")? *Recommendation: yes, as rung 11, after the voice works.*
11. **Characters' default voices from non-donated sets** (Alba MacKenna's characters, Kyutai's own,
    VCTK, Voice-Zero), donated voices for variety only, so a withdrawal never silences a character?
    *Recommendation: yes.*

## 16. Alternatives (from the scan; alternatives, not plans)

The starting set stands ("let's just stick with this set to start"). These are recorded so the next
question starts from what was found.

### 16.1 Brains

| Model | Why it is not in the starting set |
|---|---|
| Granite 4.2 3B (IBM, Apache 2.0) | Weak on IBM's own multilingual test (MMLU-ProX lite 27.78); a reasoning model |
| OLMo 2 1B (AI2, Apache 2.0, fully open data) | English only, 4K context |
| Olmo 3 7B (AI2, Apache 2.0) | Above the size budget; English only |
| SmolLM3 3B (Hugging Face, Apache 2.0) | Six languages |
| Phi-4-mini (Microsoft, MIT, 23 languages) | Larger than Ministral (2.49 GB); the scan's smaller open option for the helper, the next candidate if Gemma disappoints |
| Qwen3.5 (Alibaba) | The best numbers, but a 1260H tie (section 13) |
| Llama 3.2 (Meta), LFM3 (Liquid AI) | Licences not open source (section 13) |
| Spark-X2.6 (SparkLLM) | Unconfirmed iFlytek tie; two languages |

### 16.2 Voices

| Voice | Why not |
|---|---|
| Paradee 1.2 (Apache 2.0, 8.5 MB) | Voices of undocumented origin, two sharing names with Google and OpenAI products; English only. A light read-aloud option if wanted, under its own finding's conditions |
| Phonon-31m-en (Gradium, Apache 2.0) | Voice origin not stated; English only |
| Piper (25 clean voices) | Needs eSpeak NG (GPL) at runtime; half the catalogue sits on a research-only base |
| Kokoro-82M | eSpeak for most languages; voice origin undocumented |
| Audio8 TTS (Edge0, Apache 2.0, 23 languages) | Codec origin unknown; a possible Qwen backbone; the candidate for more languages, after its own finding |
| Supertonic 3 | OpenRAIL-M, not open source; voices of unstated origin |
| Chatterbox (Resemble AI, MIT) | Training data not itemized; cloning |
| MeloTTS, StyleTTS 2, Kitten 0.8 | Provenance not stated, or eSpeak, or English only |
| XTTS-v2, OmniVoice, Voxtral TTS | Non-commercial licences |
| NVIDIA Magpie | NVIDIA Open Model License, not open source |
| Qwen3-TTS | 1260H tie |
| VoxCPM2 | A GPU with about 8 GB |

### 16.3 Runtimes (detail in the runtime finding)

| Runtime | Why it is not the choice |
|---|---|
| ONNX Runtime with the `ort` binding (MIT) | The fallback for ONNX models: complete and fast, but a second C++ library, a Windows GPU route (DirectML) in sustained engineering, and a binding still a release candidate |
| candle (Hugging Face) | No Vulkan, so no GPU for AMD or Intel; each model re-written in Rust |
| burn (Tracel AI) | The widest Rust GPU reach, but limited ONNX coverage, no GGUF, pre-1.0; the one to revisit if the project writes its own model code |
| Our own wgpu kernels for the brains | Months of kernel work per architecture, and they would compete with the renderer for the GPU; right only for the motion model, which lives in the frame |
| `llama-server` and `audiocpp_server` as the engine | TCP listeners with no key by default, web interfaces, features not needed; llama.cpp's policy says not to expose its server in untrusted settings |
| A library loaded into the app | A model parser bug would run beside the person's unlocked keys; a crash would take the game down |
| sherpa-onnx (named on the Pocket TTS card) | Not researched here |

## 17. Sources

- The runtime decision: [`2026-11-09-model-runtime.md`](../reference/findings/2026-11-09-model-runtime.md).
- The models: the scan [`2026-11-08-npc-brain-voice`](../reference/research/2026-11-08-npc-brain-voice/README.md);
  the findings [Ministral 3 3B](../reference/findings/2026-11-08-ministral-3-3b.md),
  [Granite 4.0 1B](../reference/findings/2026-11-08-granite-4-0-1b.md),
  [Gemma 4](../reference/findings/2026-11-08-gemma-4-e2b-e4b.md),
  [Pocket TTS](../reference/findings/2026-11-08-pocket-tts.md),
  [audio.cpp](../reference/findings/2026-11-08-audio-cpp.md),
  [Paradee](../reference/findings/2026-11-08-paradee-tts.md),
  [MotionBricks](../reference/findings/2026-11-08-motionbricks.md); the AI tools scan
  [`2026-11-08-ai-tools`](../reference/research/2026-11-08-ai-tools/README.md).
- Model configurations, read 2026-11-09 from Hugging Face: `mistralai/Ministral-3-3B-Instruct-2512`,
  `ibm-granite/granite-4.0-1b`, `google/gemma-4-E2B-it` and `google/gemma-4-E4B-it` (`config.json`);
  audio.cpp's Pocket TTS packages, `audio-cpp/audio.cpp-gguf` (API tree).
- The Web Speech API, https://webaudio.github.io/web-speech-api/ (Draft Community Group Report, 27
  October 2026).
- Related designs: [`ingredients-and-labels.md`](ingredients-and-labels.md) (5.2),
  [`character-animation.md`](character-animation.md) (8.7, 10),
  [`embedded-browser.md`](embedded-browser.md) (the opt-in separate host pattern),
  [`two-realities.md`](two-realities.md), [`resource-budgets.md`](resource-budgets.md).
