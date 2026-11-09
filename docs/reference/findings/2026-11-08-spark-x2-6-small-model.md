# Spark-X2.6-1.7B: may the project bundle or fetch it for local NPC dialogue and an on-device helper

**Research date: 8 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself as a research tool. Hugging Face pages
were read through the public model API and raw file pages, GitHub through its API. Nothing was
downloaded, installed or run. Each source carries its own date where it shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences and model
cards, written so a later reader can check every word against the same sources.

Builds on the 2026-11-08 AI tools scan,
[`../research/2026-11-08-ai-tools/C-language-models-news.md`](../research/2026-11-08-ai-tools/C-language-models-news.md),
which named Spark-X2.6-1.7B "the only model here that fits every player's PC" and asked for this
finding.

## The question

May HumanityOS bundle Spark-X2.6-1.7B with the desktop app, or fetch it on first use, to run local
NPC dialogue and an on-device helper, and does anything in its terms add to Apache 2.0 or bind
players?

## Short answer

- **ALLOWED: bundling it, or fetching it on first use, and using it for NPC dialogue and a helper.**
  The licence is Apache 2.0, stated on every card and present as a `LICENSE` file in the post-trained,
  base and GGUF repositories (identical files, "Copyright 2026 XHToken") and in the GitHub repository
  (F1, F2).
- **Nothing adds terms.** The base model is the publisher's own (`XHToken/Spark-X2.6-1.7B-Base`, also
  Apache 2.0, with no upstream base model listed); no dataset is named; no acceptable-use policy,
  gating, or "responsible use" clause appears on any card, in the GitHub repository, or on the
  publisher's Ollama page; there is no NOTICE file (F2, F3).
- **Nothing binds players.** Apache 2.0 says nothing about generated text, and the cards claim nothing
  over outputs (F4). The project's own duties as the one shipping an AI that talks with people (the EU
  AI Act's Article 50, read 2026-11-06) are the project's, not the players' (F6).
- **UNCLEAR: who the publisher is, and what it trained on.**
  - The Hugging Face organization "XHToken" calls itself "SparkLLM", is not verified, gives Singapore
    on GitHub, and names no parent company; its own website could not be read (a TLS certificate
    error). Third-party pages call it a wholly owned subsidiary of iFlytek; no primary source read
    confirms or denies that (F5).
  - iFlytek has been on the US Entity List since 9 October 2019, one of 28 entities the listing rule
    says "have been implicated in human rights violations and abuses" in the campaign against Uighurs
    and other Muslim minorities, and it is still on the current list. The Entity List governs exports *to* listed parties; nothing read restricts
    receiving or using an openly licensed model from one. The question it raises is the project's
    values, not its licence (F5).
  - No card says what data trained the model (F3).
- **Recommendation:** licence-wise it is clear to use. Before it becomes the project's small-tier
  brain, the operator should decide whether an unconfirmed tie to an Entity-Listed company matters;
  meanwhile a rig test can measure whether 1.7B is good enough at all. Conditions: run it through the
  GGUF file in stock llama.cpp (never the Transformers path, which runs the repository's own Python),
  ship the licence beside the model, credit it, and serve it from the project's own release assets
  rather than from Hugging Face so a first-use download tells no third party who is playing.

## Findings

### F1. The licence as stated, and the file itself

- **Model card** https://huggingface.co/XHToken/Spark-X2.6-1.7B (repository created 2026-10-19, last
  modified 2026-11-06, not gated). Metadata `license: apache-2.0`, `language: en, zh`,
  `base_model: XHToken/Spark-X2.6-1.7B-Base`. "License" section:
  > The Spark-X2.6 model series is licensed under the [Apache 2.0 License](https://huggingface.co/XHToken/Spark-X2.6-1.7B/blob/main/LICENSE).
- **The file is there.** The repository's file list includes `LICENSE`. Read in full: the unaltered
  Apache License 2.0 ("Version 2.0, January 2004"), its appendix filled in as:
  > Copyright 2026 XHToken

  The `LICENSE` files of the base repository (https://huggingface.co/XHToken/Spark-X2.6-1.7B-Base,
  created 2026-10-19, last modified 2026-11-01) and the GGUF repository
  (https://huggingface.co/XHToken/Spark-X2.6-1.7B-GGUF, created 2026-10-19, last modified
  2026-11-06) are byte-identical to it (SHA-256 `525c465a...84c36` for all three).
- **The GitHub repository** https://github.com/XHToken/Spark-X2.6 (created 2026-08-24, last push
  2026-11-04; GitHub licence detection Apache-2.0) has a `LICENSE` whose appendix reads "Copyright
  2026 XHToken Authors", and its README ends:
  > The Spark-X2.6 model series is licensed under the Apache 2.0 License.

  (As rendered; in the source, "Apache 2.0 License" links to the repository's `LICENSE` file.)
- **The GGUF file the project would run**: `Spark-X2.6-1.7B-Q4_K_M.gguf`, 1,768,720,512 bytes, in the
  GGUF repository, whose card carries the same licence line and the same `LICENSE` file.
- **No NOTICE file** in any of the three Hugging Face repositories (file lists read through the API)
  or in the GitHub repository (tree: `.gitignore`, `CODE_OF_CONDUCT.md`, `FAQ.md`, `LICENSE`,
  `README.md`, `SECURITY.md`, six images). Apache 2.0's section 4(d) therefore asks nothing; its
  section 4(a) still does:
  > (a) You must give any other recipients of the Work or Derivative Works a copy of this License; and

### F2. The base model and the rest of the family: all the publisher's own, all Apache 2.0

- The post-trained model's only listed base is `XHToken/Spark-X2.6-1.7B-Base`, whose metadata lists
  `license: apache-2.0` and **no** `base_model` of its own. Its card's "License" section points to its
  own Apache 2.0 file. (Its card also says it holds "model weights and configuration files for the
  post-trained model", word for word as the post-trained card does; the earlier
  `XHToken/Spark-X2.5-1.7B-Base` card says "pre-trained model" in the same place. A copy-paste slip
  on the publisher's side, recorded here so nobody reads it as a second licence.)
- Every one of the organization's 24 models, listed through the API on 2026-11-08 (Spark-X2.5 and
  X2.6, base, post-trained, GGUF, FP8, INT8, INT4 and the DSpark draft models), is tagged
  `license:apache-2.0`, and every `base_model` tag among them names another XHToken model. No Qwen,
  Llama or other outside model appears as a base.
- The architecture is its own: `config.json` names `"Spark2_5ForCausalLM"` and `"model_type":
  "spark2_5"`; `configuration_spark.py` opens
  > # Copyright 2026 The XHToken team and the HuggingFace Inc. team. All rights reserved.

  followed by the Apache 2.0 header. The card states llama.cpp support:
  > Native `spark2_5` support requires [llama.cpp](https://github.com/ggml-org/llama.cpp/) `b10828` or later.

  llama.cpp (https://github.com/ggml-org/llama.cpp) is MIT by GitHub's licence detection.

### F3. Training data: not disclosed

- No card in the family names a training dataset, and none carries a `datasets:` tag. The
  organization's only dataset is `XHToken/gaokao_math_2026` (Apache 2.0), a set of Chinese
  university-entrance mathematics questions tagged "llm-as-a-judge", an evaluation set.
- The Spark-X2.5 base card (the architecture X2.6 keeps) says only:
  > **Advanced Training Algorithms**: The models were trained on Huawei Ascend clusters.

  and calls the series one with "support for more than 200 languages". The X2.6 cards are tagged
  English and Chinese only.

### F4. Acceptable use and generated content: nothing in the terms

- Searched for on 2026-11-08 and **not found**: an acceptable-use or usage policy, a "responsible AI"
  or "prohibited uses" section, a gate with terms to accept, or any claim over outputs, in the
  post-trained, base and GGUF cards, the GitHub `README.md`, `FAQ.md` and `LICENSE`, and the
  organization's card.
- The publisher's Ollama page (https://ollama.com/SparkLLM/Spark-X2.6-1.7B, "Updated 1 week ago")
  names the Apache 2.0 licence and shows no model-specific terms.
- The repository's `CODE_OF_CONDUCT.md` and `SECURITY.md` point to https://github.com/XHToken/community.
  Its "XHToken Community Code of Conduct v1.0" governs behaviour in the community (comments, commits,
  issues, events), not use of the models; its README asks contributors to sign a contributor licence
  agreement, which matters only if the project ever sends changes back.
- Apache 2.0 governs the Work and Derivative Works; it does not mention a model's output. Its warranty
  clause (section 7) puts the risk of use on the user:
  > You are solely responsible for determining the appropriateness of using or redistributing the Work and assume any risks associated with Your exercise of permissions under this License.

- The GitHub FAQ (https://github.com/XHToken/Spark-X2.6/blob/X2.6/FAQ.md) gives practical cautions,
  not terms, for example:
  > Choose and test the model against the actual task rather than treating the parameter count as an accuracy guarantee.

### F5. Who publishes it

- Hugging Face organization overview (API, read 2026-11-08): name `XHToken`, full name "SparkLLM",
  `"isVerified": false`, 26 members, and:
  > SparkLLM focuses on developing leading general-purpose foundation models and professional AI agent products. The company offers its proprietary Spark model family, together with two companion harness systems, AStudio and Loomy.

  So the open Spark-X models sit beside a proprietary "Spark model family"; nothing read ties the open
  models' licence to the proprietary line's terms.
- GitHub organization (API): name "SparkLLM", location "Singapore", blog `https://xhtoken.ai/`, email
  `xhtoken@xhtoken.com`, created 2026-08-09, not verified.
- **Not established from a primary source:** a parent company. A web search on 2026-11-08 returned
  third-party pages (llmreference.com's family page titled "Spark X2.6 Models by iFLYTEK Developer",
  and Chinese-language pages) describing XHToken as a wholly owned subsidiary of iFlytek. The
  publisher's own pages read here do not say so, and its website could not be read (below).
- **iFlytek and the Entity List.** The Bureau of Industry and Security's final rule of 9 October 2019
  (84 FR 54002, https://www.govinfo.gov/content/pkg/FR-2019-10-09/html/2019-22210.htm) added
  "IFLYTEK" among "eight additional entities" it said "are enabling activities contrary to the foreign
  policy interests of the United States", beside Xinjiang's public security bureaus, and said of all
  of them:
  > Specifically, these entities have been implicated in human rights violations and abuses in the implementation of China's campaign of repression, mass arbitrary detention, and high-technology surveillance against Uighurs, Kazakhs, and other members of Muslim minority groups in the XUAR.

  On what the listing does, the same rule:
  > the ERC has determined that the conduct of these twenty-eight entities raises sufficient concern that prior review of exports, reexports or transfers (in-country) of all items subject to the EAR involving these entities, and the possible imposition of license conditions

  (the sentence continues). The eCFR's Supplement No. 4 to Part 744 (the Entity List), in its version
  in effect from 2026-08-24 with no end date, still lists "IFLYTEK, National Intelligent Speech
  High-tech" (eCFR search API, read 2026-11-08).

### F6. The project's own duties when an AI talks with players (not the model's terms)

- The EU AI Act's Article 50(1) asks providers to make sure people "are informed that they are
  interacting with an AI system, unless this is obvious", and 50(2) asks providers of systems
  generating synthetic text to mark outputs "in a machine-readable format". Both were read and quoted
  from EUR-Lex on 2026-11-06 in
  [`2026-11-06-social-platform-apis.md`](2026-11-06-social-platform-apis.md) (items EU-1 to EU-7),
  not re-read today; whether the project is a "provider" was left open there.
- These duties fall on whoever provides or deploys the system, which is the project, never on a
  player reading an NPC's line.

## What is still unknown, and what would settle it

1. **Who owns SparkLLM / XHToken.** Settled by a primary source: the company's own site (unreadable
   today), a corporate registry in Singapore (the location its GitHub profile gives) or China (not
   tried here; costs not checked), or an announcement by iFlytek itself. About an hour of research,
   or one message to `xhtoken@xhtoken.com`.
2. **What the model was trained on.** Only the publisher can say. It changes no licence term, but a
   model that learned from copyrighted text carries the industry-wide, unsettled question of whether
   its output can echo that text; for short NPC lines the practical risk is low.
3. **Whether 1.7B is good enough for NPC dialogue at all.** Not legal; the scan's rig test (fixed NPC
   prompts, CPU time per reply measured beside a running game) settles it.
4. **Whether Article 50 reaches the project**: as in the 2026-11-06 finding, a lawyer's question.

## Sources I could not reach

- **https://xhtoken.ai/**, the publisher's website: the TLS certificate could not be verified
  ("unable to verify the first certificate") by either reader. Not worked around (no certificate
  checks were switched off). Hoped for: the company's legal name and parent, and any terms of use.
- **The benchmark numbers**: they are only in SVG images on the cards, not read (not legal).

## What it means for this project

### Certain

- The project may ship the GGUF file inside a release, publish it as a release asset, or have the app
  fetch it on first use, and run it for NPC dialogue and a helper, provided every copy travels with the
  Apache 2.0 text (F1). No NOTICE file means nothing more is owed by section 4(d).
- No term in any source read limits what the model may be used for or claims anything over what it
  writes (F4). Players are bound by nothing from the model's side.
- The model's family has no outside base model and no other licence anywhere in it (F2).

### Judgement calls

- **Run it only through the GGUF file in llama.cpp.** The Transformers path needs
  `trust_remote_code=True`, which runs `modeling_spark.py` from the repository. The GGUF path runs no
  code from the publisher. This is protection, not licence.
- **Serve it from the project's own release assets, not from Hugging Face.** Both are allowed. A
  first-use download straight from Hugging Face tells a third party the player's address and that they
  play HumanityOS; the project's GitHub release asset (as the optional star catalogue already is, per
  `LICENSES.md`) keeps that inside the project's existing exposure. The `LICENSE` file is published
  beside it either way.
- **The publisher question is the operator's to weigh.** If SparkLLM is iFlytek's, the project would
  be building its NPCs' voice on a model from a company the US government named for its part in
  surveillance of Uighurs. Nothing read makes that unlawful. The project's mission ("unite humanity",
  "protect the users") makes it a fair question to ask before it becomes a default, and Apache 2.0
  makes it cheap to swap later: the project depends on llama.cpp and a GGUF file, not on this
  publisher.
- **Label it as AI wherever it speaks**, the helper especially (F6). An NPC in a game is plainly not a
  person, which is close to Article 50(1)'s "obvious" exception, but a small marker costs nothing.
- **What NPCs may say is the project's rule to write**, since the licence sets none: a children's
  setting and the project's safety rules apply to generated lines as to any other words on screen.
- **Say what it needs.** CLAUDE.md's rule of 2026-11-08 ("Say plainly when a person's hardware cannot
  run something") applies: the 1.77 GB file needs that much free disk and at least that much free
  memory beside the game (an estimate, not the maker's figure; the FAQ warns that file size "does not
  include runtime and context memory"), so the helper's settings should check and say so, and name the
  scripted NPC lines as the fallback. (A product rule, not a licence term.)

### How the project would credit it

- **Beside the model file** (for example `data/ai/models/spark-x2.6-1.7b/` or the release asset's
  folder): `LICENSE` (the Apache 2.0 text with "Copyright 2026 XHToken", copied from the repository)
  and a short `README.md`: "Spark-X2.6-1.7B (Q4_K_M GGUF) by SparkLLM (Hugging Face organization
  XHToken), https://huggingface.co/XHToken/Spark-X2.6-1.7B, Apache License 2.0. Unmodified."
- **llama.cpp**, if bundled: its MIT licence text beside the runtime, and a line in the third-party
  licences file the 2026-09-06 attribution audit asked for
  ([`../attribution-audit-2026-09-06.md`](../attribution-audit-2026-09-06.md), finding 2).
- **`data/credits.ron`**: a row for the model with `attribution_required: true` and `shown_in`
  naming "Credits" and the helper's own settings or about panel; the same lines in `CREDITS.md` and
  `LICENSES.md`.
- **If the app fetches it**: the download screen names the model, its size (1.77 GB), its maker and
  its licence before the person agrees, which also fits the 2026-10-19 finding on showing each
  licence before an install.

### The question for the operator

**Does an unconfirmed tie between Spark-X2.6's publisher and iFlytek (on the US Entity List since 2019
for its part in surveillance of Uighurs) matter to you, given that the licence itself is clean?**

Recommendation: **licence-wise, yes, it may be bundled or fetched.** Before it becomes the default
small-tier brain, settle who the publisher is (an hour, or one email). Run the NPC rig test meanwhile.
If the tie is confirmed and it troubles you, keep the same llama.cpp design and pick another small
Apache-2.0 model; the swap costs a file, not a rewrite. Whatever model is chosen: GGUF through
llama.cpp only, the licence beside the file, served from the project's own release assets, and an
"AI" label wherever it speaks.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
