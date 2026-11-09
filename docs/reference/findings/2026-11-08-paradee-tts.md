# Paradee 1.2: may its voice model ship in the app and on the website, and with what credit

**Research date: 8 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself as a research tool. GitHub repositories
were read through GitHub's API, Hugging Face repositories through their public API and raw file
pages. Nothing was downloaded, installed or run. Each source carries its own date where it shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences, model cards
and repositories, written so a later reader can check every word against the same sources.

Builds on the 2026-11-08 AI tools scan,
[`../research/2026-11-08-ai-tools/B-speech-text-vision-agents.md`](../research/2026-11-08-ai-tools/B-speech-text-vision-agents.md)
(row 7 and its notes), which named Paradee the strongest fit and asked for this finding.

## The question

May HumanityOS ship Paradee 1.2's weights and code inside the desktop app and on the website, and use
it for Library read-aloud and a ship-computer voice, without shipping the GPL-licensed eSpeak NG, and
how must it be credited?

## Short answer

- **ALLOWED: the weights (the ONNX files in the model repository) and the Paradee code**, under
  Apache 2.0, in the desktop app and on the website, for any use including a ship-computer voice. The
  conditions are Apache 2.0's own: hand every recipient a copy of the licence, and keep notices (F1,
  F2).
- **There is no NOTICE file** in either the GitHub repository or the Hugging Face model repository, so
  Apache section 4(d) asks nothing, and the licence files carry no filled-in copyright line to retain.
  Credit is still owed by custom and by section 4(a) and 4(c): name Paradee (Sahil Mahendrakar), the
  teacher Kokoro-82M (hexgrad) and StyleTTS 2 (MIT, Aaron (Yinghao) Li), and misaki (F2, F5).
- **eSpeak NG can be left out cleanly.** It is not inside the model. It enters only through Paradee's
  Python helper (which wires misaki's eSpeak fallback in and lists eSpeak packages as dependencies) and
  through the browser phonemizer kokoro-js uses (an Apache-labelled npm package that carries a compiled
  eSpeak NG inside it). A HumanityOS build that reads misaki's dictionaries in its own Rust and
  JavaScript, with no eSpeak fallback, ships no eSpeak NG code or data (F3, F4).
- **NOT without GPL-3.0's conditions: the browser path the Chickadee extension uses**, if copied
  whole. It runs eSpeak NG (GPL-3.0-or-later) in the browser, so the website would be distributing
  GPL code under that licence's terms, which the project's own web code does not carry. The website
  needs its own dictionary lookup instead, which the model accepts (F4).
- **Without eSpeak, a word missing from the dictionary is silently skipped** by misaki's released
  version 0.9.4 with no fallback, which for read-aloud means dropped words. HumanityOS must supply its
  own fallback (F3).
- **UNCLEAR: where the dictionaries and the voices came from.**
  - misaki's "silver" dictionaries were made by running eSpeak NG over a word list; GNU's own FAQ says
    a program's output is generally not covered by the program's licence (F5).
  - Kokoro says it trained on public-domain audio, permissively licensed audio and synthetic speech
    from closed TTS models of large providers (F6); it gives no per-voice source and no speaker
    consent record. Two of Paradee's four voice names (`am_puck`, `bm_fable`) match voice names Google and
    OpenAI sell (F6, F7).
  - StyleTTS 2's pre-trained models carry an ethics condition (tell listeners the speech is
    synthesized unless you have the speaker's permission), and Kokoro lists StyleTTS 2's LJSpeech
    model as its base; whether that condition follows into Kokoro and Paradee is not answered (F8).
- **Recommendation:** use it for Library read-aloud now, on both mirrors, with no eSpeak, the licence
  text beside the model file, the credits below, and a plain "this voice is computer-generated" label
  wherever it speaks. Hold the ship-computer voice, a voice that speaks *for the project*, until the
  operator decides whether an undocumented voice origin is acceptable (the question at the end).

## Findings

### F1. Paradee's own licence: Apache 2.0 for the code and for the weights

- **GitHub repository** https://github.com/sahilmahendrakar/paradee (created 2026-09-25, last push
  2026-11-03; GitHub's licence detection: Apache-2.0). README, "License" section:
  > Apache 2.0, the same as Kokoro. The sentences in `training/data/` come from WikiText-103 and are under CC BY-SA 3.0.

  `pyproject.toml` (package version 1.2.1): `license = "Apache-2.0"`.
- **Model repository** https://huggingface.co/sahilmahendrakar/Paradee-8M-v1.2 (created and last
  modified 2026-10-28; not gated). Card metadata `license: apache-2.0`, `base_model: hexgrad/Kokoro-82M`.
  Card, "License" section:
  > Apache 2.0, the same as Kokoro-82M.
- The files in the model repository: `LICENSE`, `README.md`, `config.json`, `tokenizer.json`,
  `onnx/paradee.onnx`, `onnx/paradee_int8.onnx`, `pytorch/decoder.pt`, `pytorch/text_side.pt`, and 41
  sample files. The card describes the int8 file:
  > The whole model in one graph, weights in int8 (8.5 MB). Use this one.
- **Release date of 1.2.** The author's post https://www.sahilmahendrakar.com/writings/paradee-1-2
  is dated "October 28, 2026".

### F2. What Apache 2.0 asks of a redistributor, and the absence of a NOTICE file

- Both `LICENSE` files (GitHub and Hugging Face) are the unaltered Apache License 2.0 text, with the
  appendix's placeholder `Copyright [yyyy] [name of copyright owner]` left unfilled. Neither
  repository has a `NOTICE` file (GitHub tree read through the API: 33 files, no NOTICE; Hugging Face
  file list above).
- Section 4 (the licence text, "Version 2.0, January 2004", as in those files):
  > (a) You must give any other recipients of the Work or Derivative Works a copy of this License; and

  > (b) You must cause any modified files to carry prominent notices stating that You changed the files; and

  > (c) You must retain, in the Source form of any Derivative Works that You distribute, all copyright, patent, trademark, and attribution notices from the Source form of the Work, excluding those notices that do not pertain to any part of the Derivative Works; and

  Section 4(d) applies only "If the Work includes a "NOTICE" text file as part of its distribution",
  which neither repository does.
- Section 6, on names:
  > This License does not grant permission to use the trade names, trademarks, service marks, or product names of the Licensor, except as required for reasonable and customary use in describing the origin of the Work and reproducing the content of the NOTICE file.
- **What it builds on.** README, "Acknowledgements":
  > Paradee is distilled from [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) by hexgrad, which builds on [StyleTTS 2](https://github.com/yl4579/StyleTTS2) and [iSTFTNet](https://arxiv.org/abs/2203.02395). Phonemes come from [misaki](https://github.com/hexgrad/misaki).

  And "How it works": "Paradee is mostly Kokoro's own code at smaller widths". Kokoro's code
  (https://github.com/hexgrad/kokoro, GitHub licence detection Apache-2.0, last push 2025-08-06) has
  no NOTICE file and an unfilled copyright line. StyleTTS 2's code
  (https://github.com/yl4579/StyleTTS2) is MIT, its `LICENSE` opening "MIT License" and
  "Copyright (c) 2023 Aaron (Yinghao) Li".

### F3. Where eSpeak NG enters the reference code, and what happens to unknown words without it

- **Paradee's Python helper wires eSpeak in.** `paradee/tts.py` (main branch, read 2026-11-08):
  > `self._g2p[british] = en.G2P(trf=False, british=british, fallback=espeak.EspeakFallback(british=british), unk="")`

  and `pyproject.toml` lists among its dependencies `"phonemizer-fork"` and `"espeakng-loader"`. On
  PyPI (read 2026-11-08), `phonemizer-fork` 3.3.2 is classified "GNU General Public License v3 or
  later (GPLv3+)"; `espeakng-loader` 0.2.4 states no licence and describes itself as "A Python
  package that provides shared library loader for eSpeak NG".
- **eSpeak NG's licence.** https://github.com/espeak-ng/espeak-ng (GitHub licence detection
  GPL-3.0; last push 2026-11-08), README "License Information":
  > eSpeak NG Text-to-Speech is released under the GPL version 3 or later license.

  (As rendered; in the README's source, "GPL version 3" links to the repository's `COPYING` file.)
- **The model itself has no eSpeak in it.** The model card, "Using the ONNX file directly" (GitHub
  README) and "Files" (card): the graph's input is phoneme ids from `config.json`, and
  > Phonemes must be written the way [misaki](https://github.com/hexgrad/misaki), Kokoro's own grapheme-to-phoneme library, writes them, with its American dictionary for `af_heart` and `am_puck` and its British dictionary for `bf_isabella` and `bm_fable`.

  So any code that produces misaki's phoneme spelling can drive it.
- **misaki is Apache 2.0 and documents running with no fallback.** https://github.com/hexgrad/misaki
  (created 2025-01-12, last push 2025-08-11; GitHub licence detection Apache-2.0; PyPI classifier
  "Apache Software License"). README, English usage:
  > `g2p = en.G2P(trf=False, british=False, fallback=None) # no transformer, American English`
- **What "no fallback" does, in the released version.** PyPI's latest misaki is 0.9.4, uploaded
  2025-04-05. Its successor code on the main branch changed this on 2025-04-14 (commit `232a5e32`,
  "Add a transformer-based default english fallback (#74)"), so the main branch now reads:
  > `self.fallback = fallback if fallback else FallbackNetwork(british)`

  and `FallbackNetwork` loads the BART model `PeterReid/graphemes_to_phonemes_en_us` (or `_gb`). That
  change is not in any PyPI release. In 0.9.4 and in the main branch alike, a word that neither the
  dictionaries nor a fallback can sound out gets the `unk` placeholder:
  > `result = ''.join((self.unk if tk.phonemes is None else tk.phonemes) + tk.whitespace for tk in tokens)`

  with `unk='❓'` by default. Paradee passes `unk=""`, and its `_run` keeps only characters found in
  the model's vocabulary, so **an unknown word is dropped from the audio without a sound**.
- **The BART fallback, as an option.** https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us
  (created 2025-04-12, modified 2025-04-17) and the `_gb` twin are tagged `license: apache-2.0`; the
  card is empty apart from that tag, so what it was trained on is not stated.

### F4. The browser path: kokoro-js carries eSpeak NG inside an Apache-labelled package

- Paradee's README, "Using the ONNX file directly":
  > Browser phonemizers such as [kokoro-js](https://github.com/hexgrad/kokoro/tree/main/kokoro.js) use eSpeak NG, which writes some sounds differently from misaki.

  and `web/misaki.js` opens:
  > `// kokoro-js phonemizes with eSpeak NG, but Paradee only ever saw phonemes from`

  `web/misaki.js` itself is a 2,185-byte conversion table, part of the Apache-2.0 repository.
- kokoro-js's `package.json` (https://github.com/hexgrad/kokoro, `kokoro.js/package.json`, version
  1.2.1) declares `"license": "Apache-2.0"` and depends on `"phonemizer": "^1.2.1"`.
- The npm package `phonemizer` (registry read 2026-11-08; version 1.2.1 of 2025-01-16; repository
  https://github.com/xenova/phonemizer.js, last push 2025-01-16) declares `Apache-2.0` and describes
  itself as:
  > Simple text to phones converter using eSpeak NG

  Its repository holds `src/espeakng.worker.js` and `data/espeakng.worker.data`; the worker file is
  a compressed blob in which the strings `espeak-ng-data` appear. A package's licence label does not
  relicense the GPL program inside it (F3), so a website that bundled kokoro-js's phonemizer would be
  distributing eSpeak NG under GPL-3.0-or-later.

### F5. The dictionaries: Apache 2.0 as published, provenance partly undisclosed

- misaki's English data are four files in `misaki/data/`: `us_gold.json` (3.0 MB), `us_silver.json`
  (3.1 MB), `gb_gold.json` (2.8 MB), `gb_silver.json` (3.7 MB), inside the Apache-2.0 repository. A
  backup is published at https://huggingface.co/datasets/hexgrad/misaki (card: `license: apache-2.0`,
  last modified 2025-04-13).
- **How they were made**, in the maintainer's own words, issue #51
  (https://github.com/hexgrad/misaki/issues/51, answered 2025-02-14 by `hexgrad`):
  > To obtain the `_gold.json` dictionaries, I seeded some local datasets of IPA labels, and then mapped and filtered them programmatically using some heuristics that I wrote as a native (American) English speaker.

  > To obtain the `_silver.json` dictionaries, I ran `espeak-ng==1.52` against https://github.com/dwyl/english-words/blob/master/words_alpha.txt and then transformed the espeak phonemes using a subset of the programmatic transforms.

  The word list's repository, https://github.com/dwyl/english-words, is detected by GitHub as the
  Unlicense.
- **The question is open upstream.** Issue #107 (https://github.com/hexgrad/misaki/issues/107,
  opened 2026-09-19, still open on 2026-11-08, no maintainer reply) asks exactly this:
  > Could you say what sources they were built from (for example CMUdict, Wiktionary, espeak-ng output, or your own work), and confirm that the files may be redistributed under Apache-2.0?

  A second project added the same request on 2026-09-23.
- **GNU on the output of a GPL program.** GNU's GPL FAQ, "In what cases is the output of a GPL program
  covered by the GPL too?" (https://www.gnu.org/licenses/gpl-faq.html#WhatCaseIsOutputGPL, page
  stamp "Updated: $Date: 2026/01/13 15:26:29 $"):
  > The output of a program is not, in general, covered by the copyright on the code of the program.

  The same answer makes an exception when the output carries "text and/or art that comes from the
  program".
- **A clean-provenance alternative for American English.** CMUdict
  (https://github.com/cmusphinx/cmudict, last push 2025-10-24) carries a BSD-style licence whose
  first lines read "Copyright (C) 1993-2015 Carnegie Mellon University. All rights reserved." and
  whose second condition is:
  > Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

  Its phoneme set is not misaki's; using it would need a project-written mapping.

### F6. The voices: Kokoro's, trained on undisclosed per-voice sources

- Paradee's README: it "speaks four of Kokoro's voices: `af_heart` and `am_puck` (American) and
  `bf_isabella` and `bm_fable` (British)", and its training corpus was the teacher's own speech:
  > Kokoro reads 12,000 WikiText-103 sentences (23.9 hours of audio), and every intermediate value is saved.

  The 1.2 post adds that 1.2 trained "on Kokoro reading 12,000 sentences in each of the four voices".
  So no human voice recording went into Paradee directly; every voice it has, it learned from Kokoro.
- **Kokoro-82M** (https://huggingface.co/hexgrad/Kokoro-82M; created 2024-12-26, last modified
  2025-04-10; `license: apache-2.0`; `base_model: yl4579/StyleTTS2-LJSpeech`). "Training Details":
  > **Data:** Kokoro was trained exclusively on **permissive/non-copyrighted audio data** and IPA phoneme labels.

  Its examples are "Public domain audio", "Audio licensed under Apache, MIT, etc" and
  > Synthetic audio<sup>[1]</sup> generated by closed<sup>[2]</sup> TTS models from large providers<br/>

  with footnote [2]: "No synthetic audio from open TTS models or "custom voice clones"". Its
  "Creative Commons Attribution" section names two CC BY sources (Koniwa `tnc`, CC BY 3.0, under 1 h;
  SIWIS, CC BY 4.0, under 11 h), a Japanese and a French corpus.
- **No per-voice source.** Kokoro's `VOICES.md` grades each voice's "Target Quality" and "Training
  Duration" but names no speaker and no dataset. `af_heart` has no grades at all (only "Overall Grade"
  A); `am_puck` is "B", "H hours"; `bf_isabella` and `bm_fable` are "B", "MM minutes" (10 to 100
  minutes of audio). Nothing read records any speaker's consent.

### F7. Two of the four voice names match commercial voice products (a coincidence of names, not proof)

- Google's Gemini speech generation guide (https://ai.google.dev/gemini-api/docs/speech-generation,
  read 2026-11-08, no page date shown) lists under "Prebuilt voices", among others, Puck ("Upbeat"),
  Kore ("Firm"), Fenrir ("Excitable") and Aoede ("Breezy"). Kokoro's American voices include
  `am_puck`, `af_kore`, `am_fenrir` and `af_aoede`.
- OpenAI's text-to-speech guide (https://platform.openai.com/docs/guides/text-to-speech, served with
  Last-Modified 9 November 2026 05:18 GMT, which is the evening of 8 November Pacific) lists its
  built-in voices as alloy, ash, ballad, coral, echo, fable, nova, onyx, sage, shimmer, verse, marin and
  cedar. Kokoro's voices include `af_alloy`, `am_echo`, `bm_fable`, `af_nova` and `am_onyx`.
- Google's Gemini API Additional Terms (https://ai.google.dev/gemini-api/terms, "Effective March 23,
  2026"):
  > You may not use the Services to develop models that compete with the Services (e.g., Gemini API or Google AI Studio).

  Those terms bind whoever used the API; they are the 2026 wording, not necessarily what applied when
  Kokoro was trained (late 2024 to early 2025). OpenAI's terms of use could not be read (below).
- **Judgement, labelled as such:** matching names suggest, and Kokoro's own "Training Details"
  allows, that some of these voices were learned from Google and OpenAI voice products. Nothing read
  proves it for any one voice, and nothing read shows those companies or their voice actors consented
  to the voice being reused.

### F8. StyleTTS 2's ethics condition on its pre-trained models

- StyleTTS 2 README (https://github.com/yl4579/StyleTTS2, README last changed 2024-03-07), "License":
  > Code: MIT License

  > Pre-Trained Models: Before using these pre-trained models, you agree to inform the listeners that the speech samples are synthesized by the pre-trained models, unless you have the permission to use the voice you synthesize.

- The author, issue #37 (https://github.com/yl4579/StyleTTS2/issues/37, comment of 2023-11-19):
  > The codebase itself is MIT, and any model you trained yourself does not have to follow these rules.

  and in the same comment: "pre-trained model on LJSpeech cannot be used for deception". A user asked
  on 2024-01-22 whether a fine-tuned model counts as a pre-trained one; no answer is in the thread.
- Kokoro's card metadata names `yl4579/StyleTTS2-LJSpeech` as its base model; that Hugging Face
  repository (created 2023-11-21) has no model card ("Entry not found" for its README).

### F9. Two things that are not licences but still apply

- **Hugging Face's terms** (https://huggingface.co/terms-of-service, "Effective Date: September 15,
  2022"):
  > When Content contains notice of a reasonable and customary license, (such as an open source license) such Content is intended to remain under the terms of such license when further accessed, distributed, or used. Neither party is permitted to remove reference to any such license.

  This adds nothing beyond keeping the licence reference, which Apache 2.0 already asks for.
- **The EU AI Act's Article 50** applies to AI that talks with people and to synthetic audio. It was
  read and quoted from EUR-Lex on 2026-11-06 in
  [`2026-11-06-social-platform-apis.md`](2026-11-06-social-platform-apis.md) (items EU-1, EU-2, EU-6,
  EU-7), not re-read today. Article 50(2) asks providers of systems "generating synthetic audio" to
  mark outputs "in a machine-readable format"; whether the project is a "provider" was left open
  there. A read-aloud voice reading the Library is close to the "assistive function for standard
  editing" exception in spirit but not obviously inside its words.

## What is still unknown, and what would settle it

1. **Where misaki's gold dictionaries came from, and whether the silver dictionaries owe anything to
   eSpeak NG's data.** Settled by the maintainer answering issue #107 (free; a comment adding this
   project's request costs nothing, but posting is the operator's call). Alternative that needs no
   answer: build the American dictionary from CMUdict (BSD-style, F5) plus a project-written mapping,
   a few days of work, and accept that British English then has no equally clean source (not
   researched here).
2. **Which human voices, if any, stand behind `af_heart`, `am_puck`, `bf_isabella` and `bm_fable`.**
   Settled only by Kokoro's trainer (`@rzvzn`, reachable through the Kokoro Discord linked from the
   card). Nothing public answers it. If the answer matters, the way to remove the question entirely
   is a voice with documented consent: a volunteer recorded for the project under a written release,
   or a public-domain corpus such as LJSpeech, distilled the same way (not researched here; a
   separate finding).
3. **Whether StyleTTS 2's pre-trained-model condition reaches Kokoro and Paradee.** Settled by the
   StyleTTS 2 author or Kokoro's trainer saying whether Kokoro started from the LJSpeech weights.
   Complying costs one sentence on screen, so the answer does not change the recommendation.
4. **Whether Article 50(2)'s machine-readable marking applies to a read-aloud voice the project
   ships.** Settled by a lawyer, or by the Commission's code of practice on marking once final. Not
   urgent for a free project outside the EU, but the "provider" question from the 2026-11-06 finding
   decides it.

## Sources I could not reach

- **OpenAI's terms of use** (https://openai.com/policies/row-terms-of-use/): HTTP 403 to an automated
  reader, as in the 2026-11-08 scan. Hoped for: the clause on using output to develop competing
  models. Not worked around.
- **Paradee's paper PDF** (`paper/Paradee-Mahendrakar-2026.pdf`, 120 KB): not opened, because its
  arXiv abstract (https://arxiv.org/abs/2610.06817, "Submitted on 5 Oct 2026") and the README cover
  the training corpus, and the README says "The paper describes version 1.0".
- **The `yl4579/StyleTTS2-LJSpeech` model card**: the repository has none.

## What it means for this project

### Certain

- The ONNX model, `config.json` and Paradee's code may be shipped in the desktop app's `data/` and
  served from the website, and used for read-aloud and an in-game voice, under Apache 2.0 (F1).
- Each copy must travel with the Apache 2.0 text (F2, section 4(a)). The project's precedent fits:
  the instrument pipeline puts each library's licence text "in every sample folder it makes, so it
  travels with the samples into the app and into every pack"
  ([`../instrument-sources.md`](../instrument-sources.md)), and `assets/models/LICENSE-cc0-model-packs.md`
  sits beside the models it covers. The 2026-09-06 attribution audit's finding 2 (no third-party
  licence text in the release archive) is still open; a licence file inside the model's own folder
  under `data/` reaches the archive through the workflow's wholesale `cp -r data/`.
- None of eSpeak NG, `phonemizer-fork`, `espeakng-loader`, kokoro-js's `phonemizer` or misaki's
  `EspeakFallback` may be part of what ships if the project is to stay clear of GPL-3.0 (F3, F4).
  Leaving them out loses nothing the model needs.
- `training/data/` (WikiText-103 sentences, CC BY-SA 3.0) must not be copied into the project: CC
  BY-SA material cannot go into the CC0 tree (see
  [`2026-10-25-licence-options.md`](2026-10-25-licence-options.md)). Nothing at runtime needs it.
- Without a fallback, unknown words vanish from the audio (F3). A read-aloud feature that drops words
  silently fails the people it is for; the project must write its own fallback.

### Judgement calls

- **The dictionaries are fit to ship under Apache 2.0.** Their licensor published them under it, the
  word list is Unlicensed, and GNU's own FAQ says program output is generally outside the program's
  licence. The residual doubt (the gold sources, eSpeak's exception lists echoed in the silver
  entries) is small; recorded, not ignored.
- **The fallback for unknown words** should be project-written and free of eSpeak: spell the word out
  letter by letter from the dictionary's own letter entries, and log the word so the dictionary can
  grow. The BART fallback is a later option once its training data is known.
- **Read-aloud is fine to build now. A voice that speaks for the project is a different matter.**
  Copyright and licence allow both. What differs is trust: the project tells people it is honest about
  where things come from, and two of these four voices may be learned from voices another company
  sells, with no consent on record (F6, F7). For a Library "read this to me" button, where the voice is
  a tool and labelled as computer-generated, that risk is small. For the ship computer, the voice that
  becomes the project's own, a voice with documented consent is the better foundation.
- **Always say the voice is synthesized**, wherever it speaks (a small label beside the play control;
  a line in the voice's settings). It meets StyleTTS 2's condition whatever its reach (F8), goes
  toward Article 50 (F9), and is plainly honest.

### How the project would credit it

- **Beside the model file** (for example `data/voices/paradee/`): `LICENSE` (the Apache 2.0 text,
  copied from the model repository), and a short `README.md` saying: "Paradee-8M v1.2 by Sahil
  Mahendrakar, https://huggingface.co/sahilmahendrakar/Paradee-8M-v1.2, Apache License 2.0.
  Distilled from Kokoro-82M by hexgrad (Apache 2.0), which builds on StyleTTS 2 (MIT, Copyright (c)
  2023 Aaron (Yinghao) Li). Phonemes follow misaki by hexgrad (Apache 2.0)." Add the StyleTTS 2 MIT
  text too, since the graph is Kokoro's code at smaller widths and Kokoro builds on StyleTTS 2.
- **Beside the dictionaries**: misaki's Apache 2.0 text and a line naming misaki and its repository.
  If CMUdict is used, its full BSD-style notice (its condition 2 requires it in binary
  distributions).
- **`data/credits.ron`**: one row for Paradee and one for misaki's dictionaries, with
  `attribution_required: true` and `shown_in` naming "Credits" and the voice settings; the unit test
  in `crates/humanity-core/src/credits.rs` then guards them. Add the same lines to `CREDITS.md` and a
  section in `LICENSES.md`.
- **On the website**: the same licence files served next to the model and dictionary files, and the
  same credit on the web Credits page.
- **Courtesy, not obligation**: Kokoro's card credits Koniwa (CC BY 3.0) and SIWIS (CC BY 4.0) as part
  of its training audio. They are Japanese and French corpora, unlikely to be in these English voices,
  but a line in the README costs nothing.

### The question for the operator

**May the project use Paradee's voices, which are Kokoro's voices of undocumented origin (two of them
sharing names with Google and OpenAI voice products), for Library read-aloud now, and should the
ship-computer voice wait for a voice with documented consent?**

Recommendation: **yes to read-aloud now** on both the app and the website, with no eSpeak NG, a
project-written fallback for unknown words, the licence files and credits above, and a "computer
voice" label wherever it speaks. **Hold the ship-computer voice** for a voice whose speaker agreed in
writing (a volunteer, or a public-domain corpus), distilled the same way, so the voice that represents
HumanityOS carries no question about whose voice it is.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
