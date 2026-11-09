# AI tools from one news video: what could serve HumanityOS (read 2026-11-08)

**Why this exists.** On 2026-11-08 the operator asked to "review all the new AI tools showcased on
this video": "AI finally sleeps, Gemini 4 Carbon, ETH cracked, energy-based models, Anthropic IPO:
AI NEWS" by AI Search, 2026-10-31, https://www.youtube.com/watch?v=IGICYFyP_EI. Its description
lists about 30 tools and news items with links. Three research agents read each one's PRIMARY source
(project page, repository, model card, official post) on 2026-11-08, not the video and not news
write-ups. Each file below quotes every licence as its source states it, with the link.

**What this is not.** It is a scan, not a decision. A "candidate" still needs its own dated legal
finding in [`../../findings/`](../../findings/README.md) before anything is downloaded, bundled or
built on (CLAUDE.md: research a legal question, write it down and date it; open source only for core
tech). MotionBricks, also in the video, has its own finding and plan, written separately.

| File | Group |
|---|---|
| [A-3d-worlds-game.md](A-3d-worlds-game.md) | 3D, animation, worlds and game: IDOL, SceneBloom, UniAction, NVIDIA SimReady Assets, MUNDO, Genie 4, PhysAg3D, OverSeer, StreamVibe, MatAnyone-E, Open-H3-Regenerate-2K |
| [B-speech-text-vision-agents.md](B-speech-text-vision-agents.md) | Speech, text, vision, agents and frontier models: Unlimited OCR 2, Hy-SimulMT, Nemotron 3.5 Transcribe, Audio8 TTS, SenseNova Code2Video, AutoSwarm, Paradee 1.2, SLEEP, the energy-based model segment, GPT-6.1 Terra, Gemini 4 Carbon |
| [C-language-models-news.md](C-language-models-news.md) | Open-weight language models and news: Nanbeige4.3-33B, LFM3, Apex 35B, Groot, Clover 35B, Spark-X2.6-1.7B, SenseNova-Loop 9B, the OpenAI science post, Anthropic's S-1 |

## The verdicts in one place

**Candidates** (open licence and plainly useful; each still needs a dated legal finding):

| Tool | What it could do here | Licence as stated | The catch |
|---|---|---|---|
| Paradee 1.2 | Read the Library aloud; a ship-computer voice; reads in a browser too, and reports when each word is spoken | Apache-2.0, code and weights | English only. Its Python code falls back to eSpeak NG (GPL-3.0) for words it does not know: the finding must keep that out |
| Unlimited OCR 2 (Baidu) | Turn scanned pages and documents into text locally, for the Library and the Files page | MIT, code and weights | About 3B parameters; the maker documents NVIDIA GPUs only |
| Audio8 TTS Realistic Air 0.6B | Voices for NPCs and read-aloud in 23 languages | Apache-2.0 | The bundled audio codec's origin and licence are not stated; NVIDIA only. Its 0.1B sibling has a revenue-capped licence, which is not open source |
| NVIDIA SimReady Assets | 2,677 real-scale props with physics data for homestead kitchens, life support and a medical bay | CC BY 4.0 | USD files, converted to glTF offline; attribution must be shown |
| Spark-X2.6-1.7B | NPC dialogue or the in-app helper on the CPU, beside the game, on modest PCs | Apache 2.0 | 1.77 GB at 4 bits; tagged English and Chinese only |
| Groot-19B-A3B | A private local drafter on the operator's PC (Outbox drafts, Library questions; reads images) | Apache 2.0 | 11.7 GB at 4 bits; anonymous publisher; the terms of its base model (Qwen) must be checked; its 65B sibling has a different licence |
| IDOL (conditional) | Turn a photo into a 3D object (Gaussian splat) | Apache-2.0 for its own code and weights | Needs two non-open parts (DINOv3, the FLUX.2-dev VAE); NVIDIA only; the engine has no splat renderer yet |

**Watch** (useful, but not open, not released, or not ready): LFM3 (its licence cuts off at $10M
revenue, so it is not open source), PhysAg3D (needs a ChatGPT/Codex sign-in and a 28 GiB GPU),
StreamVibe (nothing released), Hy-SimulMT (Chinese and English only, 19.7 GB), Nemotron 3.5
Transcribe (34B, offline only), SenseNova Code2Video (27B, no licence in its repository), OpenEBFM
(the likely subject of the energy-based segment; not confirmed).

**Skip**:
- Genie 4, GPT-6.1 Terra and Gemini 4 Carbon: closed models, reached only through paid APIs.
- MUNDO and Open-H3-Regenerate-2K: the MiniMax H3 licence lists the United States among its excluded territories.
- OverSeer, UniAction and AutoSwarm: not relevant here.
- SLEEP: a paper only, with no code.
- Apex 35B, Clover 35B, Nanbeige4.3-33B and SenseNova-Loop 9B: open, but too big for 12 GB.
- The two news items: they do not touch the project.

**Sites that refused automated reading** (not worked around; another primary source was used or the
item was marked unchecked): openai.com (HTTP 403), sec.gov (asks for contact details in the request,
which the research did not give). GitHub's web pages timed out (HTTP 504), so repositories were read
through GitHub's API.
