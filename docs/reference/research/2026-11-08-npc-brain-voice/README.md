# Speaking NPCs and the in-app helper: open "brains" and "voices", and which carry US-government risk

**Research date: 8 November 2026** (Pacific time). A scan, not a legal finding. Every source below was
read that day by an AI research agent working for HumanityOS, identifying itself in every request as
"HumanityOS-research-agent/1.0 (automated AI research tool ...)". Hugging Face pages were read through
the public model API and raw file pages, GitHub through its API and raw.githubusercontent.com, US
regulations through the eCFR and Federal Register APIs. Nothing was downloaded, installed or run.

**I am not a lawyer, and this is not legal advice.**

Builds on `C:\Humanity\docs\reference\research\2026-11-08-ai-tools\README.md` and the findings
`docs/reference/findings/2026-11-08-paradee-tts.md` and `docs/reference/findings/2026-11-08-spark-x2-6-small-model.md`.

**Follow-up, same day:** the dated legal findings this scan asks for below are written:
[Ministral 3 3B](../../findings/2026-11-08-ministral-3-3b.md),
[Granite 4.0 1B](../../findings/2026-11-08-granite-4-0-1b.md),
[Gemma 4 E2B and E4B](../../findings/2026-11-08-gemma-4-e2b-e4b.md),
[Pocket TTS](../../findings/2026-11-08-pocket-tts.md) and
[audio.cpp](../../findings/2026-11-08-audio-cpp.md). The Pocket TTS finding found what this scan
could not: the data that trained the first released weights, in Kyutai's release post (its F3).
This scan's text below is left as written.

**Sources that refused or could not be read (not worked around):**
- The BIS/ITA **Consolidated Screening List API** (`data.trade.gov`) answered HTTP 401: "Access denied
  due to missing subscription key". The Entity List itself was read instead, from its primary text in
  the eCFR (Supplement No. 4 to Part 744), plus the Federal Register rules that change it.
- **`kyutai/pocket-tts`** (the voice-cloning repository) is gated; its card was read from the ungated
  twin `kyutai/pocket-tts-without-voice-cloning` and the GitHub README.
- **Kyutai's Pocket TTS blog post** returned no readable text to the reader (it gave no figures, no
  date); the GitHub README and model card were used instead.
- **Meta's Llama 3.2 model cards** and **Neuphonic's NeuTTS cards** are gated; Llama's licence was read
  from Meta's GitHub repository, NeuTTS was left at its metadata.
- GitHub's API rate limit was reached once; files were then read from raw.githubusercontent.com.

---

## Short answer

The operator asked: "Are there any alternatives that are potential risks with the US government?"
Both readings:

**Alternatives that DO carry a US-government tie** (checked 2026-11-08):

| Model or family | The tie | Source |
|---|---|---|
| **Qwen** (Qwen3.5 0.8B/2B/4B, Qwen3-TTS) and everything **built on Qwen weights** (Microsoft's FrogNano-4B, OmniVoice, Maya2, VibeVoice-Realtime, the earlier scan's Groot-19B) | Publisher Alibaba. **Alibaba Group Holding Limited is on the Department of Defense's "Chinese military companies" list** (Section 1260H), notice of 10 June 2026. Not on the Entity List. | 91 FR 35189 |
| **Spark-X2.6-1.7B** | Publisher unconfirmed; third parties tie it to iFlytek. **iFlytek is on the Entity List** (since 2019, still listed in the version current through 2026-11-05). If SparkLLM is 50%+ owned by iFlytek, BIS's "affiliates rule" would reach it, but that rule is suspended through 10 January 2027. | eCFR Supp. 4 to Part 744; 90 FR 47201; 91 FR 70789 |
| Models from **Zhipu AI** (GLM, GLM-TTS), **BAAI**, **SenseTime** (the earlier scan's SenseNova items), **Tencent**, **Baidu** | Zhipu, BAAI and SenseTime are on the Entity List; SenseTime, Tencent and Baidu are on the 1260H list. None is in this scan's shortlist; listed so nobody picks one by accident. | as above |

**Alternatives with NO such tie found:** IBM Granite, AI2 OLMo, Hugging Face SmolLM, Microsoft Phi,
Mistral's Ministral, Google Gemma, Meta Llama, Liquid AI's LFM3 (brains); Kyutai Pocket TTS, Gradium
Phonon, Piper (Open Home Foundation), Kokoro/Paradee, Resemble AI Chatterbox, Supertone Supertonic,
KittenML, MyShell MeloTTS, StyleTTS 2, Coqui XTTS, NVIDIA Magpie, Edge0 Audio8 (voices). None of their
publishers appears on the Entity List (as of the version current through 2026-11-05) or on the 2026
1260H list. **"No US-government tie" is not "clean":** Llama and LFM3 fail the open-source rule on
their licences, and several voices fail on licence or on undocumented voice origin (below).

**The recommendations** (each still needs its own dated legal finding):

| Use | Brain | Voice |
|---|---|---|
| (a) NPC dialogue beside the running game, modest PC | **Ministral 3 3B Instruct** (Mistral AI, France; Apache 2.0; 2.15 GB at Q4_K_M). Low-memory fallback: **Granite 4.0 1B** (IBM; Apache 2.0; 1.02 GB). Rig-test **Gemma 4 E2B** against it (Apache 2.0; 3.11 GB; strongest multilingual of the small ones). | **Pocket TTS** (Kyutai, France; weights CC BY 4.0, code MIT; CPU on 2 cores, faster than real time; 6 languages; voices with documented releases, including CC0 donated voices and a voice actor's RPG characters). Run through audio.cpp (Apache 2.0; CPU, Vulkan, CUDA, HIP, Metal) or a Rust port, with no eSpeak. |
| (b) In-app helper, strong PC | **Gemma 4 E4B** (Google; Apache 2.0; 4.98 GB at Q4_K_M; text, image and audio input; 140+ pretraining languages). | **Pocket TTS** again (one voice system everywhere; its 24-layer language variants for quality). For languages beyond its 6: **Audio8 TTS Realistic Air** through audio.cpp, which now gives it a CPU and Vulkan route, pending its finding. |

---

## Part 0. The US-government check

### What was checked, and how

1. **The Entity List** (15 CFR Part 744, Supplement No. 4), searched by name through the eCFR search
   API, restricted to Part 744 and to the version in effect on **2026-11-05**. The eCFR's own currency
   record for Title 15 read: `"latest_amended_on":"2026-11-04","latest_issue_date":"2026-11-05","up_to_date_as_of":"2026-11-05"`.
   The Federal Register was then searched for BIS Entity List rules published after 1 July 2026 (two
   on 2026-08-24: an Arrow Electronics address revision and one removal under Turkey; neither touches
   any name here).
2. **The DoD Section 1260H list** of "Chinese military companies", read in full from the Federal
   Register (91 FR 35189, 10 June 2026). No later DoD removal notice was found in the Federal Register.
3. **BIS's "affiliates rule"** (the 50% rule) and its suspension, from the Federal Register.
4. **NIST's evaluations of PRC models** (CAISI), as context for dialogue content.

### What the eCFR returned (version in effect 2026-11-05)

**Found on the Entity List:**
- `IFLYTEK, National Intelligent Speech High-tech` (excerpt as indexed).
- Zhipu AI: "Beijing Zhipu Huazhang Technology Co., Ltd., a.k.a., the following two aliases: —Zhipu AI;
  and —Beijing Knowledge Atlas Technology Co., Ltd." (added 90 FR 4619, 1/16/25).
- `Beijing Academy of Artificial Intelligence` (90 FR 14048, 3/28/25), SenseTime, Megvii, Inspur,
  Huawei, Sophgo.

**Not found (zero hits in Supplement No. 4):** Alibaba, Alibaba Cloud, Qwen, Tongyi, DeepSeek, Moonshot,
MiniMax, Baidu, ByteDance, Tencent, Xiaomi, Ant Group, ModelBest, Mianbi, OpenBMB, Kuaishou, Stepfun,
Fish Audio, XHToken, SparkLLM, Edge0, Liquid AI, Hugging Face, Mistral, Allen Institute, International
Business Machines, Microsoft, Google, Meta Platforms, Open Home Foundation, Kyutai, Gradium, Supertone,
Resemble, MyShell, Neuphonic, Stellon, ShugoAI, MediaTek, Boson, Zhejiang Lab.

Limit of this method: a full-text search can miss an entry whose name is spelled differently or split
oddly. The Consolidated Screening List, which also covers other lists, could not be queried (above).

### The 1260H list (91 FR 35189, 10 June 2026)

The notice says the Deputy Secretary of Defense "has determined that the entities listed in the
SUPPLEMENTARY INFORMATION section of this notice satisfy the requirements to be designated as a
``Chinese military company.''". Its Alibaba entry, verbatim:

> Alibaba Group Holding Limited (Alibaba)
>
> <bullet> Alibaba is indirectly affiliated with SASAC (Section 1260H(g)(2)(B)(i)(I)). Alibaba is a military-civil fusion contributor to the Chinese defense industrial base because it is affiliated with MIIT (Section 1260H(g)(3)(B)(i)).

Baidu, Tencent, SenseTime, Huawei, Inspur and Unitree are also listed. **iFlytek, Ant Group, Xiaomi,
ByteDance, DeepSeek, Zhipu, Moonshot, MiniMax and ModelBest are not** on this list. The notice itself
states no restriction on anyone outside the Department of Defense; its procurement consequences were
not researched here.

### The affiliates rule, and its suspension

- 90 FR 47201 (30 September 2025), abstract: "Under this IFR, any entity that is at least 50 percent
  owned by one or more entities on the Entity List will itself automatically be subject to Entity List
  restrictions."
- 91 FR 70789 (Federal Register of 9 November 2026), abstract: "The suspension was set to end November
  9, 2026. This final rule extends the suspension through January 10, 2027."
- So if SparkLLM/XHToken were confirmed a wholly owned iFlytek subsidiary, it would fall under Entity
  List restrictions automatically once the suspension ends, unless BIS extends it again.

### What these lists do and do not do (unchanged from the Spark finding)

The Entity List controls exports **to** listed parties. Nothing read restricts receiving or running an
openly licensed model **from** one. The risk to HumanityOS is therefore about values, trust, and the
chance of future rules, not about the licence. A project that says it protects its users should not
build its NPCs' voice on a company the US government names for surveillance of Uighurs (iFlytek) or
calls a "Chinese military company" (Alibaba), when equally open alternatives exist.

### Why PRC-origin dialogue models also matter for content

NIST's CAISI, "CAISI Evaluation of DeepSeek AI Models Finds Shortcomings and Risks" (30 September
2025, updated 20 November 2025): "DeepSeek models echoed four times as many inaccurate and misleading
CCP narratives as U.S. reference models did." That evaluation is of DeepSeek, not Qwen; it is cited
only to show the class of problem an NPC brain could carry (an NPC asked about Taiwan). Any brain
chosen should be rig-tested on such prompts regardless of origin.

---

## Part 1. Brains: small open language models (up to about 4B) in llama.cpp on a CPU

**Notes on reading the table.**
- "OSI?" means the licence is on the Open Source Initiative's approved list. Apache 2.0 and MIT are.
  The Llama licence is not: OSI, "Meta's LLaMa license is still not Open Source" (Jordan Maris, 18
  February 2025): "Llama 3.x is still not Open Source by any stretch of the imagination."
- 4-bit size is the Q4_K_M GGUF file from the publisher's own GGUF repository where one exists,
  otherwise from a named community converter (unsloth, bartowski, ggml-org). RAM needed is that plus
  the context cache.
- Quality: **no publisher evaluates short in-character dialogue.** The nearest proxies they publish are
  instruction following (IFEval, IFBench), chat preference (Arena-Hard, AlpacaEval 2 LC, WildBench) and
  multilingual knowledge (MMMLU, Global MMLU). Numbers from different publishers use different
  harnesses and are not strictly comparable. The rig test (fixed NPC prompts, CPU time per reply beside
  a running game) is what settles it.
- llama.cpp support: every model below has a GGUF. LFM3's own card benchmarks Qwen3.5-4B, Qwen3.5-9B
  and Gemma 4 E4B "as Q4_0 GGUF files in llama.cpp ... release b11744", which confirms those
  architectures run there.

| Model | Publisher, country | Licence as stated (link) | OSI? | Entity List / 1260H | Params | Q4_K_M | Context | Languages | Quality proxies (publisher's own) |
|---|---|---|---|---|---|---|---|---|---|
| **Granite 4.2 3B** (released "August 25, 2026") | IBM, USA | Card: "**License** \| [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)"; "All Granite models are released under the Apache 2.0 license, enabling unrestricted commercial and academic use." [card](https://huggingface.co/ibm-granite/granite-4.2-3b) | Yes | None | 3.66B | 2.24 GB (official GGUF) | "Natively Supports 128K (Long-context extension to 512K)" | 12 tested: en, de, es, fr, ja, pt, ar, cs, it, ko, nl, zh | Arena-Hard-V2 34.96; IFBench 74.33; MMLU-Pro 67.84; MMLU-ProX lite 27.78 (weak multilingual on IBM's own test). Reasoning model with a non-thinking mode. |
| **Granite 4.0 1B** ("October 28, 2025") | IBM, USA | "**License:** [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)" [card](https://huggingface.co/ibm-granite/granite-4.0-1b) | Yes | None | 1.63B | 1.02 GB (official) | 131,072 (GGUF metadata) | same 12 | IFEval average 77.38; MMLU 59.39; MMLU-Pro 34.02 |
| **OLMo 2 1B Instruct** (April 2025) | AI2, Seattle, USA | "**License:** Apache 2.0" [card](https://huggingface.co/allenai/OLMo-2-0425-1B-Instruct). Fully open: "We are releasing all code, checkpoints, logs, and associated training details." | Yes | None | 1.48B | 0.94 GB (official) | 4,096 | en | IFEval 70.1; AlpacaEval 2 LC 9.1; MMLU 40.0. AI2 has no newer small OLMo: Olmo 3 starts at 7B (see Part 3b). |
| **SmolLM3 3B** (July 2025) | Hugging Face, "NYC + Paris" | "## License [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)" [card](https://huggingface.co/HuggingFaceTB/SmolLM3-3B). Pretraining datasets published. | Yes | None | 3.08B | 1.92 GB (ggml-org) | 64K trained, 128K with YaRN | "6 natively supported (English, French, Spanish, German, Italian, and Portuguese)"; less ar, zh, ru | No-think: IFEval 76.7; MixEval Hard 26.9; Global MMLU 53.5. Still HF's newest small chat model (SmolLM2-1.7B, 1.06 GB, English, is the smaller one). |
| **Phi-4-mini-instruct** (February 2025) | Microsoft, USA | "The model is licensed under the [MIT license](https://huggingface.co/microsoft/Phi-4-mini-instruct/blob/main/LICENSE)." LICENSE: "MIT License / Copyright (c) Microsoft Corporation." [card](https://huggingface.co/microsoft/Phi-4-mini-instruct) | Yes | None | 3.84B | 2.49 GB (unsloth, bartowski) | 128K | 23 listed (ar, zh, cs, da, nl, en, fi, fr, de, he, hu, it, ja, ko, no, pl, pt, ru, es, sv, th, tr, uk) | Arena Hard 32.8; Multilingual MMLU 49.3. Microsoft's newer 4B ("FrogNano-4B-2609") is a Qwen3.5-4B coding agent, not a dialogue model, and inherits the Qwen tie. |
| **Ministral 3 3B Instruct 2512** | Mistral AI, France | "This model is licensed under the [Apache 2.0 License](https://www.apache.org/licenses/LICENSE-2.0.txt)." [card](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512) | Yes | None | 3.4B LM + 0.4B vision | 2.15 GB (official GGUF; vision projector separate) | 256K | "dozens of languages, including English, French, Spanish, German, Italian, Portuguese, Dutch, Chinese, Japanese, Korean, Arabic" | Arena Hard 0.305; WildBench 56.8 (Gemma 3 4B: 0.318 / 49.1); base Multilingual MMLU 0.652. Non-reasoning instruct: short, fast replies. Mistral's newer small items are a safety classifier (Shieldstral 3B) and speech models, not chat. |
| **Gemma 4 E2B-it** (blog "Apr 02, 2026") | Google DeepMind, USA | Card: "**License**: [Apache 2.0](https://ai.google.dev/gemma/docs/gemma_4_license)"; the linked page is the plain Apache 2.0 text with no added terms. Blog: "That's why Gemma 4 is released under a commercially permissive Apache 2.0 license." [card](https://huggingface.co/google/gemma-4-E2B-it) | Yes (new: Gemma 3 was under Google's own terms) | None | "2.3B effective (5.1B with embeddings)" | 3.11 GB (unsloth); Google's QAT q4_0: 3.35 GB | 128K | "Out-of-the-box support for 35+ languages, pre-trained on 140+ languages" | MMLU Pro 60.0%; MMMLU 67.4%; Tau2 24.5%. Also takes image and audio input. |
| **Gemma 4 E4B-it** | Google DeepMind, USA | as E2B | Yes | None | "4.5B effective (8B with embeddings)" | 4.98 GB (unsloth); QAT q4_0 5.15 GB | 128K | as E2B | MMLU Pro 69.4%; MMMLU 76.6%; Tau2 42.2% |
| **Llama 3.2 1B / 3B Instruct** ("September 25, 2024") | Meta, USA | Llama 3.2 Community License: must "prominently display “Built with Llama”"; use must "adhere to the Acceptable Use Policy"; licensees above "700 million monthly active users" need Meta's permission. [LICENSE](https://github.com/meta-llama/llama-models/blob/main/models/llama3_2/LICENSE) | **No** (OSI, above) | None | 1.24B / 3.21B | 0.81 / 2.02 GB (bartowski) | 128K | en, de, fr, it, pt, hi, es, th | Card gated. OLMo's card measures "Llama 3.1 1B" (as labelled) at IFEval 54.0, AlpacaEval 2 LC 10.1. Meta's newer small models (MobileLLM-Pro, MobileMoE, 2025-2026) are under the "FAIR Noncommercial Research License". |
| **Qwen3.5 0.8B / 2B / 4B** (Feb 2026) | Alibaba (Qwen team), China | `license: apache-2.0`; LICENSE file: Apache 2.0, "Copyright 2026 Alibaba Cloud" [card](https://huggingface.co/Qwen/Qwen3.5-2B) | Yes | **1260H: Alibaba Group Holding Limited** (91 FR 35189) | 0.87B / 2.27B / 4.66B | 0.53 / 1.28 / 2.74 GB (unsloth) | 262,144 | "201 languages and dialects" | 2B non-thinking: IFEval 61.2, MMMLU 56.9. 4B thinking: IFEval 89.8, MMMLU 76.1. The strongest numbers at each size. |
| **LFM3-1.7B** (blog 2026-10-27) | Liquid AI, USA | "LFM Open License v1.0": "The rights granted under this License for Commercial Use are conditioned upon You or Your Legal Entity not exceeding the Threshold"; "Threshold" = "annual revenue of 10 million United States dollars ($10,000,000) or more". [LICENSE](https://huggingface.co/LiquidAI/LFM3-1.7B/blob/main/LICENSE) | **No** | None | 1.73B | 1.10 GB (official) | 256K | 16: en, ar, zh, fr, de, it, ja, ko, pt, es, vi, th, id, hi, ru, pl | IFEval 96.66; IFBench 73.03. Card: "not recommended for agentic coding or complex multi-step reasoning". Fastest on CPU per its card (figures in an image). |
| **Spark-X2.6-1.7B** (for comparison) | SparkLLM / XHToken, "Singapore" (GitHub); third parties say an iFlytek subsidiary | "The Spark-X2.6 model series is licensed under the [Apache 2.0 License]" (Spark finding) | Yes | **Publisher tie unconfirmed; iFlytek on the Entity List** | 1.98B (GGUF metadata) | 1.77 GB (official) | 1,048,576 (GGUF metadata) | en, zh | Benchmarks only in images on the card (not read). |

### Brain notes

- **Licence files.** Granite 4.2, Gemma 4, Ministral 3, SmolLM3 and OLMo 2 state their licence only in
  the card and its metadata; their Hugging Face repositories hold **no LICENSE file**. Apache 2.0's
  section 4(a) still requires handing recipients a copy, so the project ships the standard text beside
  the file. Phi-4-mini also has a `NOTICE.md` ("This software incorporates material from third
  parties"); MIT has no NOTICE clause, but shipping it beside the MIT text costs nothing.
- **Responsible-use text that is not a licence term.** OLMo 2: "OLMo 2 is intended for research and
  educational use. For more information, please see our [Responsible Use Guidelines]". Phi-4-mini:
  "Nothing contained in this Model Card should be interpreted as or deemed a restriction or
  modification to the license". The legal findings should record whether any such text binds.
- **Open training data.** Fully open (weights, code and data): OLMo (Dolma, Tülu), SmolLM3 (pretraining
  datasets published). Granite: "A full listing of training datasets is available in the Granite 4.2
  GitHub repository". The others do not publish their data.
- **Size versus the "2 GB beside the game" budget.** At Q4_K_M: OLMo 2 1B 0.94 GB, Granite 4.0 1B
  1.02 GB, LFM3 1.10 GB, Qwen3.5-2B 1.28 GB, SmolLM3 1.92 GB, Ministral 3 3B 2.15 GB, Granite 4.2 3B
  2.24 GB, Phi-4-mini 2.49 GB, Gemma 4 E2B 3.11 GB. Judgement: on the CPU (the plan for play), RAM is
  the limit, not VRAM; a 16 GB PC can hold any of these beside the game, an 8 GB PC only the ~1 GB ones.

---

## Part 2. Voices: open text-to-speech for a CPU or any GPU

**The runtime question first.** The "universal" part of the operator's ask (any GPU vendor, or the CPU)
is now answered by a runtime, not by one model:

- **audio.cpp** (https://github.com/0xShug0/audio.cpp; LICENSE: "Copyright 2026 ShugoAI LLC / Licensed
  under the Apache License, Version 2.0"; release v0.9.4 of 2026-11-04). README: "Runs on Windows,
  Linux, and macOS, with support for NVIDIA, AMD, Apple Silicon, and CPU-only machines", backends "CUDA,
  HIP/ROCm, Vulkan, Metal, WebGPU, SYCL, and CPU", and Windows x64 builds for "CPU, Vulkan, CUDA". It is
  to voice models what llama.cpp is to language models (both are built on ggml). It runs Pocket TTS,
  Supertonic 3, Kokoro, Phonon, Magpie, Chatterbox, OmniVoice, Piper and Audio8 as GGUF files.
- **eSpeak NG stays out by default.** Its CMakeLists: `option(AUDIOCPP_STATIC_ESPEAK "Build and
  statically link GPL-3.0-or-later eSpeak-ng; keep data separate" OFF)`. Its README notes that Kitten
  0.8 and Piper use "the shared eSpeak-ng frontend", so those two would pull GPL code back in.
- audio.cpp's own per-model licence table (`docs/model_licenses.md`) is a useful cross-check; it says
  "Model weights keep the license of their original release". Its owner (ShugoAI LLC) gives no country
  in what was read.

| Voice model | Publisher, country | Licence as stated | OSI? | Entity List / 1260H | Size | Languages | Voices: licence and consent | CPU speed, from the source |
|---|---|---|---|---|---|---|---|---|
| **Paradee 1.2** (baseline) | Sahil Mahendrakar (individual) | Apache 2.0, code and weights (Paradee finding) | Yes | None | 7.88M, 8.5 MB int8 ONNX | en (US, UK) | Kokoro's voices; **origin undocumented**, two share names with Google and OpenAI products (Paradee finding F6-F7) | "about 62 times faster than real time on one CPU thread" (Apple M4 Pro; earlier scan) |
| **Audio8 TTS Realistic Air 0.6B** (baseline) | Edge0, "United States of America" (GitHub) | "The code and model weights are released under the [Apache License 2.0](https://huggingface.co/Edge0/Audio8-TTS-Realistic-Air/blob/main/LICENSE)." [card](https://huggingface.co/Edge0/Audio8-TTS-Realistic-Air) | Yes | None | 601M + 1.35 GB codec | 23 (zh, en, ja, ko, de, fr, es, it, ru, ar, fi, hi, id, ms, nl, no, pl, pt, sv, th, tr, vi, yue) | Zero-shot cloning; no preset-voice provenance; training data not stated | Maker: "a CUDA-capable GPU are recommended". **New:** audio.cpp's GGUF port (Q8 1.43 GB) runs it with "`--backend vulkan` or `--backend cpu`"; no speed given |
| **Piper** | Open Home Foundation (engine); voices by many contributors | Engine: original `rhasspy/piper` MIT, **archived** ("Development has moved"); successor `OHF-Voice/piper1-gpl` is **GPL-3.0** and "embeds [espeak-ng] for phonemization". Voices repo: `license: mit` | Engine: GPL-3.0 is OSI, but copyleft; voices: per dataset (below) | None | ~20-60 MB per voice | 178 voices, 58 locales, 53 language families | Each voice's MODEL_CARD names its dataset licence; **consent is not stated per voice**. Many voices start from a non-commercial base (below) | Not stated as a number; "A fast and local neural text-to-speech engine" |
| **Kokoro-82M** | hexgrad (anonymous) | `license: apache-2.0`; "This is an Apache-licensed model" [card](https://huggingface.co/hexgrad/Kokoro-82M) | Yes | None | 82M | 9: American and British English, Japanese, Mandarin, Spanish, French, Hindi, Italian, Brazilian Portuguese (54 voices) | Trained on "permissive/non-copyrighted audio data" including synthetic audio from "closed TTS models from large providers"; no per-voice source (Paradee finding). Spanish, French, Hindi, Italian and Portuguese use "espeak-ng" for phonemes (VOICES.md), so multilingual Kokoro needs GPL code | Not stated on the card |
| **Coqui XTTS-v2** | Coqui (company closed) | "Coqui Public Model License 1.0.0 ... This license allows only non-commercial use of a machine learning model and its outputs." | **No** | None | ~1.9 GB | 17 | Cloning; speakers file of unstated origin | Not stated |
| **MeloTTS** | MyShell, "United States of America" (GitHub), with MIT researchers | "This library is under MIT License, which means it is free for both commercial and non-commercial use." | Yes | None | ~208 MB per language | en (US, UK, Indian, Australian), es, fr, zh (mixed en), ja, ko | Voice and training-data provenance **not stated** | "Fast enough for `CPU real-time inference`." |
| **StyleTTS 2** | Yinghao Aaron Li (academic) | "Code: MIT License"; "Pre-Trained Models: Before using these pre-trained models, you agree to inform the listeners that the speech samples are synthesized ..." | Code yes | None | ~100M+ | en | Pre-trained on LJSpeech (public domain) and LibriTTS (CC BY 4.0, LibriVox): documented. Needs "phonemizer and espeak" for its demo | Not stated |
| **Pocket TTS** (Jan 2026; training code Aug 2026) | **Kyutai**, "France" (GitHub); non-profit lab | Weights `license: cc-by-4.0` [card](https://huggingface.co/kyutai/pocket-tts-without-voice-cloning); code MIT (GitHub). Plus a "Prohibited use" statement, including "voice impersonation or cloning without explicit and lawful consent" | Code yes; **CC BY 4.0 is not an OSI licence** (permissive, attribution only) | None | 100M; "Small 18M/7M/2M" English variants (audio.cpp) | en, fr, de, pt, it, es | **Documented per voice** in [`kyutai/tts-voices`](https://huggingface.co/kyutai/tts-voices): volunteer donations "licensed as CC0" (228 verified voices, June 2025 to Feb 2026); "Characters voice-acted by Alba MacKenna" incl. "Merchant: A seller you'd typically encounter in RPG's etc.", "Released under the CC BY 4.0 licence"; VCTK (CC BY 4.0); LibriVox via Voice-Zero (CC0). Avoid Expresso and EARS voices: "**Non-commercial use only.**" Training data of the released weights: **not stated** in what was read | "~6x real-time on a CPU of MacBook Air M4", "Uses only 2 CPU cores"; on "a cloud x86 VM (4 vCPUs)", "RTF ~2.3-2.5x on CPU" (README, meaning 2.3-2.5 times faster than real time) |
| **Phonon-31m-en** (Oct 2026) | **Gradium**; builds on Pocket TTS; no country in what was read | "The weights, tokenizer and voices in this repository are released under [Apache 2.0] ... The runtime is a separate project, licensed MIT OR Apache-2.0." | Yes | None | 31M, 55-65 MB GGUF | en only (the 5-language Phonon models are "CC BY-NC 4.0") | ~100 catalogue voices, Apache 2.0, **origin not stated** | "small enough to run in a browser tab, on a phone, or on a laptop CPU"; no number. Its blind-test table puts it above Kokoro in English (Elo 2098 vs 2043) |
| **Supertonic 3** (May 2026) | Supertone (South Korea; repo now `supertone-oss-archive`) | "The accompanying model is released under the OpenRAIL-M License"; sample code MIT. OpenRAIL-M carries use restrictions that "will always have to include - at minimum - the same use-based restrictions" downstream | **No** | None | ~99M, ~400 MB ONNX | 31 (en, ko, ja, ar, bg, cs, da, de, el, es, et, fi, fr, hi, hr, hu, id, it, lt, lv, nl, pl, pt, ro, ru, sk, sl, sv, tr, uk, vi) | Fixed preset voices; **provenance and training data not stated** | "runs fast on CPU" (chart image); on an e-reader "an average RTF of 0.3×" |
| **Kitten TTS mini 0.8** (Feb 2026) / **Kitten TTS 2** (Sep 2026) | KittenML / Stellon Labs | 0.8: `license: apache-2.0`. 2: "Stellon Labs Community License", licences terminate above "USD $1,000,000 in annual revenue" or funding | 0.8 yes; 2 **no** | None | 0.8: 80M, 79 MB; 2: 1.7B | 0.8: en (8 voices); 2: 16 | Provenance **not stated**; 0.8's package depends on `phonemizer` and `espeakng_loader` (GPL path) | 0.8: "works without a GPU"; no number |
| **Chatterbox Multilingual V3** / **Chatterbox-Nano** (2026) | Resemble AI, "United States of America" (GitHub) | `license: mit` | Yes | None | 0.5B ("0.5B Llama backbone") / Nano 110M | V3: 23 languages; Nano: en | "Trained on 0.5M hours of cleaned data", **not itemized**; cloning. Every output carries the "Perth (Perceptual Threshold) Watermarker", which helps labelling | Nano: "3x faster than realtime on 8 cores"; V3: not stated |
| **NVIDIA Magpie TTS Multilingual 357M** | NVIDIA, USA | "NVIDIA Open Model License" | **No** | None | 357M | 12 | "4 proprietary voices and 1 public voice" (a LibriVox reader); training sets listed, including Emilia | Card: "designed and/or optimized to run on NVIDIA GPU"; audio.cpp runs it on other backends; no CPU number |
| **OmniVoice** (Mar 2026) | k2-fsa (Next-gen Kaldi) | Card: "Our code is released under the Apache 2.0 License. The pre-trained model is licensed under the CC-BY-NC due to constraints from its training data (e.g., Emilia)." | **No** (weights) | None for k2-fsa; **base model Qwen3-0.6B** (Alibaba, 1260H) | 0.6B | "646+ langs" (audio.cpp) | Cloning | Not stated |
| **Qwen3-TTS 0.6B/1.7B** (Jan 2026) | Alibaba, China | `license: apache-2.0` | Yes | **1260H (Alibaba)** | 0.9B / 1.7B | 10 | "Trained on over 5 million hours of speech data", not itemized | Not stated (GPU, flash-attn optional) |
| **VoxCPM2** (Apr 2026) | OpenBMB / ModelBest, China | "Released under the [Apache-2.0] license" | Yes | None found | 2.3B | 30 | "2M+ hours", not itemized | GPU: "RTF (RTX 4090) ~0.30"; "~8 GB" VRAM |
| Others seen, not shortlisted | | Mistral **Voxtral-4B-TTS**: CC BY-NC 4.0. **Gradium phonon-40m** (5 languages): CC BY-NC 4.0. Microsoft **VibeVoice-Realtime-0.5B**: MIT, but "limited to research purposes", English, Qwen2.5 base. **NeuTTS** (Neuphonic, UK): gated; Air is Apache 2.0 but lists the Emilia dataset; newer "2e" is `other`. **Ming-3.0-TTS** (inclusionAI / Ant Group, China; not on either list): MIT, 7 languages, 1.5B. **Fish Audio Drama-3**: 36B, research licence. | | | | | | |

### Piper's voices, one by one (178 MODEL_CARDs read)

How each card states its dataset licence (raw counts): CC0 51; "See URL" 24; CC BY 4.0 (various
spellings) 31; public domain 9; CC BY-NC-SA (various) 17; CC BY-NC 3; CC BY-SA (various) 11; Apache 2.0 or MIT 8;
"Unknown" 4; none stated 5; and singles including "AGPLv3", "GPL 2.0", "NewGenLTU Open RAIL-D License
1.0" and "Non-commercial use (see https://www.data-baker.com/data/index/TNtts/)".

**The starting checkpoint matters more than the dataset.** 75 voices are "Finetuned from U.S. English
lessac voice" and 21 more from the "ryan" voice, which is itself fine-tuned from lessac and whose own
dataset is "CC BY-NC-SA 4.0". The lessac voice is trained on the Blizzard 2013 Lessac data, whose
licence (read 2026-11-08) grants a "non-exclusive and non-transferable licence, without the right to
sub-licence" for "Research Purposes", which excludes "using the Materials for any commercial purpose".
It does not address models trained on the data. audio.cpp's own table marks Piper's lessac voice
commercial use "Unclear" for the same reason. Judgement: roughly half of Piper's catalogue sits on a
research-only base, which an open-source project whose forks may be commercial should not ship.

**The clean subset** (trained from scratch, or from the LibriTTS base, on CC0, public-domain, CC BY or
Apache data): 25 voices in 12 languages: `de_DE-mls`, `el_GR-rapunzelina`, `en_GB-cori` (2),
`en_US-kristin`, `en_US-libritts-high`, `en_US-ljspeech` (2), `en_US-norman`, `es_ES-carlfm`,
`et_EE-news`, `fr_FR-mls`, `it_IT-serena` (2; trained on a synthetic dataset), `kk_KZ` (3), `nl_BE-nathalie`,
`nl_BE-rdh` (2), `nl_NL-mls`, `no_NO-nvcc`, `sv_SE-nst`, `uk_UA-lada`, `uk_UA-ukrainian_tts`. Even these
need eSpeak NG phonemes at runtime: a Piper voice's config names `"phoneme_type": "espeak"` (checked on
`en_US-ljspeech-medium` and `zh_CN-huayan-x_low`); only a few voices, such as two Lithuanian ones, document a
different phonemizer.

### Voice notes

- **Documented consent is rare.** Of everything read, only Kyutai's voice repository says where each
  voice came from and under what terms, and only its donation set and its voice actor's recordings are
  people who knowingly gave their voice for synthesis. Datasets like VCTK, LibriVox and Common Voice
  record consent to the recording's release, not specifically to a synthetic voice. Piper documents
  datasets, not consent. Kokoro, Paradee, Phonon, Supertonic, MeloTTS, Kitten and Chatterbox document
  neither.
- **Pocket TTS's non-English default voices are not all in the documented repository.** "giovanni"
  (it) and "lola" (es) point to Common Voice clips (CC0); "juergen" (de) and "rafael" (pt, a file named
  like a YouTube video id) point to a branch of the gated repository with no stated source. The project
  would use documented voices or record its own.
- **Pocket TTS can be retrained on documented data.** Its training code is public (August 2026), and
  the training README's example uses NVIDIA's HiFiTTS-2: "Use 200 hours of audio for a model that
  speaks, 2000 for one that actually sounds good." Community models already exist, among them "92
  African languages and English in one model ... 2.4x faster than real time on 4 CPU cores" (Africa
  TTS, gated, licence not checked). This is the route to a voice with no provenance question at all.
- **Runs from Rust.** The Pocket TTS card lists community Rust ports ("Candle version (Rust) with
  WebAssembly", "A Rust port of Pocket TTS implemented with XN") and sherpa-onnx; Phonon's runtime is a
  Rust crate (`ptts`, "MIT OR Apache-2.0"). That fits a native Rust app with no Python.

---

## Part 3. Recommendations

Preference order used: OSI licence; no Entity List or 1260H tie; fits the hardware; documented
provenance; evidence of quality. "Best" here means the best fit for the rules, pending a rig test.

### (a) NPC dialogue beside the running game, on a modest PC

**Brain: Ministral 3 3B Instruct 2512.** Apache 2.0, Mistral AI (France), no list tie, official
Q4_K_M GGUF of 2.15 GB, 256K context, a non-reasoning instruct model (short replies, low latency),
eleven named languages "and dozens" more, and the best published chat-preference scores of the
OSI-licensed ~3B models (Arena Hard 0.305, WildBench 56.8; Gemma 3 4B 0.318 / 49.1).
- **Low-memory fallback (8 GB PCs): Granite 4.0 1B.** Apache 2.0, IBM, 1.02 GB, 12 languages, IFEval
  77.4.
- **Rig-test alongside: Gemma 4 E2B.** Apache 2.0 since April 2026, Google, newer, the strongest
  multilingual scores of the small non-Qwen models (MMMLU 67.4%), but 3.11 GB.
- **Why not the others:** Qwen3.5 has the best numbers but an Alibaba (1260H) tie; Spark-X2.6 has the
  iFlytek question and only two languages; LFM3 and Llama fail the OSI rule; OLMo 2 1B is English-only
  with 4K context; SmolLM3 has six languages; Granite 4.2 3B scores 27.78 on IBM's own multilingual test.

**Voice: Pocket TTS (100M, 6 languages), run through audio.cpp or a Rust port, without eSpeak.**
CPU-first ("Uses only 2 CPU cores", 2.3-6 times faster than real time on the maker's figures), so it
never touches the renderer's GPU; weights CC BY 4.0 and code MIT; publisher a French non-profit lab
with no list tie; and the only voice set read that records, voice by voice, people who gave their
voice for synthesis (CC0 volunteer donations, and a voice actor's "Merchant" and other RPG characters).
Piper records datasets per voice, but not consent. Use only the CC0 and CC BY voices, never Expresso or EARS.
- **English-only light fallback for the weakest PCs:** Paradee 1.2 (8.5 MB, 62 times real time on one
  thread; voice origin undocumented, so read-aloud rather than the project's own voice) or Phonon-31m-en
  (Apache 2.0, 55-65 MB; voice origin not stated).

**Dated legal findings still needed:**
1. `YYYY-MM-DD-ministral-3-3b.md`: Apache 2.0 from Mistral's own repository text (no LICENSE file on
   Hugging Face), the base model's terms, any use policy, the vision encoder's terms (the project would
   ship only the text model), and the GGUF's provenance.
2. `YYYY-MM-DD-granite-4-0-1b.md`: the same for the fallback, including IBM's listed training datasets.
3. `YYYY-MM-DD-pocket-tts.md`: whether CC BY 4.0 weights plus the "Prohibited use" statement meet the
   open-source rule and bind players or forks; how attribution must appear; what data trained the
   released weights (not stated anywhere read); each chosen voice's terms (CC0 donations, Alba MacKenna
   CC BY 4.0, VCTK CC BY 4.0); and the runtime chosen (audio.cpp Apache 2.0 built with
   `AUDIOCPP_STATIC_ESPEAK=OFF`, or a Rust port and its licence).

### (b) The in-app helper, on a strong PC

**Brain: Gemma 4 E4B.** Apache 2.0, Google, no list tie; 4.98 GB at Q4_K_M (Google's own QAT q4_0:
5.15 GB); 128K context; pretrained on 140+ languages; takes images and audio as well as text; MMLU Pro
69.4%, MMMLU 76.6%, Tau2 42.2% on Google's card. It fits the RTX 4070 when the game is not rendering,
or the CPU. The same family as the NPC test candidate, so one chat format serves both.
- **Fully open alternative (data included): Olmo 3 7B Instruct** (AI2, Apache 2.0; IFEval 85.6, MMLU
  69.1, AlpacaEval 2 LC 40.9 on its card), above the 4B scope and English-only. Its card adds "It is
  intended for research and educational use in accordance with Ai2's Responsible Use Guidelines".
- **Why not the earlier scan's Groot-19B-A3B:** its base is Qwen, so it carries the Alibaba (1260H) tie.
- **OSI-licensed smaller option:** Phi-4-mini (MIT, 3.8B, 2.49 GB, 23 languages).

**Voice: Pocket TTS again**, using its "biggers 24 layers variants that are higher quality but slower"
for non-English text, so players hear one consistent voice system everywhere.
- **For languages beyond Pocket TTS's six:** Audio8 TTS Realistic Air (Apache 2.0, 23 languages)
  through audio.cpp's GGUF port, which now runs on Vulkan or the CPU, so it is no longer NVIDIA-only.
  Open questions: the codec's origin; the training data; and audio.cpp's description of it as having
  "a Qwen backbone", which Edge0's own card does not say (its config names `"model_type": "arktts"`).
  If that backbone is Qwen weights, the Alibaba tie follows.
- **Not recommended despite 31 languages:** Supertonic 3 (OpenRAIL-M, not OSI, use restrictions that
  bind forks; voices of unstated origin).

**Dated legal findings still needed:**
1. `YYYY-MM-DD-gemma-4.md`: that the Apache 2.0 page is the whole of Gemma 4's terms (the card links
   to it and the blog says so; confirm no Gemma Terms of Use or Prohibited Use Policy still attaches),
   which components ship (text only, or the audio and vision encoders too), and attribution.
2. The Pocket TTS finding above covers the helper's voice.
3. `YYYY-MM-DD-audio8-tts.md` (only if the many-languages option is wanted): the codec's origin and
   licence, whether any Qwen or Fish Audio weights are inside, the training data, and audio.cpp's
   conversion of the codec.

### What a rig test should measure before the choice is final

For each candidate brain, at Q4_K_M in stock llama.cpp, beside the running game on the operator's PC
and on a floor-class PC: CPU time per 40-token NPC reply, RAM, persona consistency over ten turns,
refusal rate on ordinary game lines, behaviour on politically loaded prompts, and quality in Spanish
and Hindi. For Pocket TTS through audio.cpp: real-time factor on 2 cores beside the game, and the
frame-time cost.

---

## What is still unknown

1. **Who owns SparkLLM/XHToken** (unchanged from the Spark finding). It now matters twice: the
   iFlytek tie, and the affiliates rule returning after 10 January 2027.
2. **What trained Pocket TTS's released weights.** Only Kyutai can say; their training code and
   example data are public, so the project can train its own weights on documented data if the answer
   is unsatisfying.
3. **Whether Audio8 inherits Qwen weights.** Only a third party (audio.cpp) says "Qwen backbone".
4. **The Consolidated Screening List** was not queried (it needs a registered key); the Entity List and
   the 1260H list were read from their primary texts instead. Other lists (OFAC's, for example) were not
   checked.
5. **No benchmark measures in-character NPC dialogue** for these sizes; the rig test is the only answer.

## Sources (all read 2026-11-08 unless a date is given)

- Entity List: eCFR search API, 15 CFR Part 744 Supplement No. 4, version in effect 2026-11-05
  (https://www.ecfr.gov/api/search/v1/results; Title 15 "up_to_date_as_of":"2026-11-05").
- DoD 1260H list: 91 FR 35189, 10 June 2026,
  https://www.federalregister.gov/documents/2026/06/10/2026-11571/notice-of-availability-of-designation-of-chinese-military-companies
- Affiliates rule: 90 FR 47201 (30 September 2025), https://www.federalregister.gov/documents/2025/09/30/2025-19001;
  suspension 2025-19846 (12 November 2025); extension 91 FR 70789 (9 November 2026),
  https://www.federalregister.gov/documents/2026/11/09/2026-22793
- Entity List revisions of 2026-08-24: 91 FR 54658, 91 FR 54657.
- NIST CAISI, https://www.nist.gov/news-events/news/2025/09/caisi-evaluation-deepseek-ai-models-finds-shortcomings-and-risks
- OSI on Llama, https://opensource.org/blog/metas-llama-license-is-still-not-open-source (18 February 2025)
- Model cards and API metadata (Hugging Face): ibm-granite/granite-4.2-3b, -GGUF, granite-4.0-1b, -GGUF;
  allenai/OLMo-2-0425-1B-Instruct, -GGUF, Olmo-3-7B-Instruct; HuggingFaceTB/SmolLM3-3B,
  SmolLM2-1.7B-Instruct-GGUF; microsoft/Phi-4-mini-instruct, FrogNano-4B-2609;
  mistralai/Ministral-3-3B-Instruct-2512, -GGUF; google/gemma-4-E2B-it, gemma-4-E4B-it and their QAT
  GGUFs; meta-llama/Llama-3.2-1B-Instruct, 3B-Instruct (metadata; gated); Qwen/Qwen3.5-0.8B, 2B, 4B;
  LiquidAI/LFM3-1.7B, -GGUF; XHToken/Spark-X2.6-1.7B-GGUF; unsloth, bartowski and ggml-org GGUF repos
  for sizes. Gemma 4 licence page https://ai.google.dev/gemma/docs/gemma_4_license; launch blog
  https://blog.google/innovation-and-ai/technology/developers-tools/gemma-4/ ("Apr 02, 2026").
  Llama 3.2 licence https://github.com/meta-llama/llama-models/blob/main/models/llama3_2/LICENSE.
- Voices: rhasspy/piper-voices (voices.json and all 178 MODEL_CARDs); github.com/OHF-Voice/piper1-gpl;
  github.com/rhasspy/piper; Lessac licence https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html;
  hexgrad/Kokoro-82M (card, VOICES.md); coqui/XTTS-v2 (card, LICENSE.txt); github.com/myshell-ai/MeloTTS;
  github.com/yl4579/StyleTTS2; kyutai/pocket-tts-without-voice-cloning; kyutai/tts-voices;
  github.com/kyutai-labs/pocket-tts (README, training/README.md); gradium/phonon-31m-en, phonon-voices,
  phonon-31m-en-audiocpp; Supertone/supertonic-3 (card, LICENSE); github.com/supertone-oss-archive/supertonic;
  KittenML/kitten-tts-2 (card, LICENSE.md), kitten-tts-mini-0.8; github.com/KittenML/KittenTTS
  (dependencies); ResembleAI/chatterbox, chatterbox-nano; nvidia/magpie_tts_multilingual_357m;
  k2-fsa/OmniVoice; Qwen/Qwen3-TTS-12Hz-0.6B-Base; openbmb/VoxCPM2; microsoft/VibeVoice-Realtime-0.5B;
  mistralai/Voxtral-4B-TTS-2603; Edge0/Audio8-TTS-Realistic-Air; audio-cpp/Audio8-TTS-Realistic-0.6b-GGUF;
  github.com/0xShug0/audio.cpp (README, LICENSE, CMakeLists.txt, docs/model_licenses.md,
  docs/models/audio8_tts.md).
- Publisher locations: GitHub organization profiles (kyutai-labs "France", resemble-ai, Edge0-AI,
  myshell-ai and Liquid4All "United States of America", allenai "Seattle, WA", huggingface "NYC +
  Paris", QwenLM "China", XHToken "Singapore"). Others (IBM, Microsoft, Google, Meta: USA; Mistral:
  France; Supertone: South Korea; OpenBMB: China) are from general knowledge, not checked against a
  registry.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so a later reader can tell whether the sources changed or this reading was wrong.
