# Pocket TTS: may its weights and voices speak for NPCs and the helper, what trained it, and whose voices they are

**Research date: 8 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself in every request as
"HumanityOS-research-agent/1.0 (automated AI research tool ...)". Hugging Face pages were read
through the public model and dataset APIs and raw file pages, GitHub through its command-line client
and raw.githubusercontent.com, other sites as pages. Kyutai's blog renders its text in the browser,
so its posts were read from the site's own page scripts (the same text a browser shows). Two PDFs
that are themselves the sources cited (the voice donation terms and privacy policy) were read with
pdftotext. Nothing else was downloaded, nothing was installed or run. Each source carries its own
date where it shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences, model
cards, dataset terms and consent documents, written so a later reader can check every word against
the same sources.

Builds on the 2026-11-08 NPC brain and voice scan,
[`../research/2026-11-08-npc-brain-voice/README.md`](../research/2026-11-08-npc-brain-voice/README.md),
which recommended Pocket TTS as the one voice system for NPCs and the helper and asked for this
finding (Part 3 (a), item 3). The scan could not find what trained the released weights; this
finding did (F3).

## The question

May HumanityOS serve Kyutai's Pocket TTS weights and a chosen set of its voices from its own release
downloads and run them on the player's machine for NPC lines and the in-app helper, and on what
terms: the licences, the "Prohibited use" statement, the data that trained the weights, each voice's
consent record, the credit owed, and whether a "computer voice" disclosure is required anywhere?

## Short answer

- **ALLOWED: the weights under CC BY 4.0 and the code under MIT**, for any use, including shipping
  them, converting them to GGUF and running them for NPCs and the helper, with attribution (F1, F7).
  CC BY 4.0 says a format conversion "never produces Adapted Material".
- **The "Prohibited use" statement is not part of the licence.** It is the gate text of the gated
  repository and a paragraph on the cards; the ungated repository the project would use asks no one
  to accept anything. Creative Commons' own FAQ says such separate terms bind only the parties to
  them. The planned use (consented voices, labelled, no cloning) meets it anyway. The project must
  not pass it on to players as a licence condition, because CC BY 4.0 forbids adding terms (F2).
- **FOUND: what trained the first released weights.** Kyutai's release post of 13 January 2026: "The
  model is trained purely on publicly released data. Specifically, the dataset is composed of AMI,
  EARNINGS22, GIGASpeech, SPGISpeech, TED-LIUM, VoxPopuli, LibriHeavy, and Emilia. These datasets
  are all in English and add up to 88k hours of audio." (F3)
- **NOT clean: four of those eight carry non-commercial or research-only terms for their users**
  (GigaSpeech, SPGISpeech, TED-LIUM, and the original Emilia), and Kyutai licensed the weights CC BY
  4.0 regardless. Whether such terms reach a model trained on the data, or anyone who never accepted
  them, is an unsettled question no source read answers (F3).
- **UNDOCUMENTED: what trained the weights a player would actually get.** The default English model
  since October 2026 (`english_2026-10`, "re-aligned Dora data"), the five other languages and Dutch
  have no published data list. The small 18M/7M/2M models train on HiFi-TTS-2 (CC BY 4.0) while
  distilling a teacher whose data is not stated (F3).
- **Voices with a consent record** (quoted in F5): the volunteer donations (CC0, under signed-off
  terms that foresee synthetic speech and allow withdrawal), Kyutai's own recordings (CC0), the voice
  actor Alba MacKenna's RPG characters (CC BY 4.0, Kyutai's statement), Voice-Zero's LibriVox readers
  (CC0) and VCTK (CC BY 4.0, consent to synthesis not stated).
- **NOT usable: two of the 26 default voices are non-commercial** ("cosette" from Expresso, "jean"
  from EARS), two have no stated source ("juergen", "rafael"), two sit on a gated branch
  ("giovanni", "lola"); outside the defaults, one Alba MacKenna recording and two of Kyutai's
  website voices carry third-party questions (F4, F5).
- **A donated voice can be withdrawn.** Kyutai's privacy policy says dataset users are "obliged by
  our Acceptable Use Policy to regularly review any update"; that policy itself was not found
  published (F5).
- **No eSpeak.** Pocket TTS reads text with a SentencePiece tokenizer (F6).
- **No licence requires a "computer voice" label**, but Kyutai's statement forbids passing output off
  as "genuine recordings of real people or events", and the EU AI Act's deep-fake rule, where it
  reaches, asks even a fictional work to disclose generated audio "in an appropriate manner" (F8).
- **Recommendation:** yes, under CC BY 4.0, with the ungated (no voice cloning) weights, only the
  documented voices, the training-data list disclosed in the credits, and a "computer voice" label
  everywhere. The question for the operator is whether the dataset terms behind the weights matter
  enough to train the project's own Pocket TTS on documented data, which Kyutai's MIT training code
  makes possible (F3, the end).

## Findings

### F1. The licences

- **Weights.** https://huggingface.co/kyutai/pocket-tts-without-voice-cloning (repository created
  2026-01-06, last modified 2026-10-21, **not gated**) and https://huggingface.co/kyutai/pocket-tts
  (created 2025-12-29, last modified 2026-10-21, **gated**, `"gated": "auto"`): both carry metadata
  `license: cc-by-4.0`. The card (read from the ungated repository; the gated one's raw card answered
  HTTP 401) lists `language: en, fr, de, pt, it, es`; the repository also holds a Dutch model
  (`languages/dutch/`). The ungated repository's name, the configs' separate
  `weights_path_without_voice_cloning` entries and its `remove_voice_cloning_and_push.py` script mark
  it as the variant with voice cloning removed; its voices come as precomputed embeddings
  (`embeddings_v3/*.safetensors`).
- **Code.** https://github.com/kyutai-labs/pocket-tts (created 2026-01-07, last push 2026-11-07;
  GitHub's licence detection: MIT). Its `LICENSE` is the MIT permission text **without a copyright
  line** (it starts "Permission is hereby granted, free of charge"). Kyutai's release post,
  https://kyutai.org/blog/2026-01-13-pocket-tts (dated 2026-01-13, "Kyutai Team"):
  > As usual with Kyutai releases, we open-source Pocket TTS under the MIT license.

  (As rendered; "Pocket TTS" is a link to the repository.)
- **The codec inside.** Pocket TTS decodes audio with Kyutai's Mimi (the config's `mimi:` section).
  https://huggingface.co/kyutai/mimi (created 2024-09-10, last modified 2025-07-02): `license:
  cc-by-4.0`.
- **CC BY 4.0 on format conversion**, from the legal code, https://creativecommons.org/licenses/by/4.0/legalcode.txt
  (served with Last-Modified 11 September 2023), section 2(a)(4):
  > The Licensor authorizes You to exercise the Licensed Rights in all media and formats whether now known or hereafter created, and to make technical modifications necessary to do so.

  > For purposes of this Public License, simply making modifications authorized by this Section 2(a)(4) never produces Adapted Material.

  (Quoted with the file's line breaks joined.)

### F2. The "Prohibited use" statement, and what it binds

- **The text**, the same on both cards, in the GitHub README, and as the gate prompt
  (`extra_gated_prompt`) of both repositories:
  > Use of our model must comply with all applicable laws and regulations and must not result in, involve, or facilitate any illegal, harmful, deceptive, fraudulent, or unauthorized activity. Prohibited uses include, without limitation, voice impersonation or cloning without explicit and lawful consent; misinformation, disinformation, or deception (including fake news, fraudulent calls, or presenting generated content as genuine recordings of real people or events); and the generation of unlawful, harmful, libelous, abusive, harassing, discriminatory, hateful, or privacy-invasive content. We disclaim all liability for any non-compliant use.

  On the gated repository a person must accept it (with "Company or university" and a use of "Work",
  "Studies" or "Fun") before downloading. The ungated twin shows the same prompt in its metadata but
  is `"gated": false`, so no one is asked to accept it there.
- **What CC BY 4.0 says about other terms.** Section 7(b):
  > Any arrangements, understandings, or agreements regarding the Licensed Material not stated herein are separate from and independent of the terms and conditions of this Public License.

  And section 2(a)(5)(b), which binds every licensee, the project included:
  > No downstream restrictions. You may not offer or impose any additional or different terms or conditions on, or apply any Effective Technological Measures to, the Licensed Material if doing so restricts exercise of the Licensed Rights by any recipient of the Licensed Material.
- **Creative Commons' FAQ**, https://creativecommons.org/faq/ (served with Last-Modified 12 June
  2026), "What if I have received CC-licensed material with additional restrictions?":
  > These additional terms do not form part of the license for the work and may not be applicable to distributions of the work outside of the platform.

  and "Can I enter into separate or supplemental agreements with users of my work?":
  > It should be noted that any agreements you make with other parties only have an effect on the other parties to that agreement, and do not apply to anyone else receiving the licensed material.

  (Both quoted with the page's line breaks joined.)

### F3. The data that trained the weights

- **The first release, in Kyutai's words**, https://kyutai.org/blog/2026-01-13-pocket-tts, section
  "Data":
  > The model is trained purely on publicly released data. Specifically, the dataset is composed of AMI, EARNINGS22, GIGASpeech, SPGISpeech, TED-LIUM, VoxPopuli, LibriHeavy, and Emilia. These datasets are all in English and add up to 88k hours of audio.

  (As rendered; each name is a link.) Earlier in the post:
  > To maximize reproducibility, we trained the model on public English datasets only.
- **Each dataset's own terms**, read the same day:

  | Dataset | Terms, from its publisher | Source |
  |---|---|---|
  | AMI | "The AMI corpus and its annotations are released under the Creative Commons Attribution 4.0 license agreement (also called CC BY 4.0)." | https://groups.inf.ed.ac.uk/ami/corpus/license.shtml |
  | EARNINGS22 | README badge "License: CC BY-SA 4.0" | https://github.com/revdotcom/speech-datasets/tree/main/earnings22 |
  | GigaSpeech | Gate prompt: "SpeechColab does not own the copyright of the audio files." and "1. Researcher shall use the Database only for non-commercial research and educational purposes." (metadata tag `apache-2.0`) | https://huggingface.co/datasets/speechcolab/gigaspeech (gated; terms from its public metadata, last modified 2026-02-07) |
  | SPGISpeech | Gate prompt: "1.1 The Content is provided for academic research purposes and internal use only and must not be used to: assemble or create a database; construct or facilitate the construction of products which compete with the Content; identify or attempt to identify or contact any individual; or link to another dataset." | https://huggingface.co/datasets/kensho/spgispeech (gated; public metadata, last modified 2026-01-20) |
  | TED-LIUM | Primary page unreadable (below). TensorFlow Datasets' catalogue, served in Indonesian, gives each release as Creative Commons "BY-NC-ND 3.0" and adds in English: "All talks and text are property of TED Conferences LLC." | https://www.tensorflow.org/datasets/catalog/tedlium (secondary) |
  | VoxPopuli | "VoxPopuli Data", "CC0 (see also European Parliament's legal notice for the raw data)" (a table row, as rendered; "CC0" and "legal notice" are links) | https://github.com/facebookresearch/voxpopuli (README, "License") |
  | LibriHeavy | Built from LibriLight's LibriVox recordings (its README's example paths); no licence statement in the README | https://github.com/k2-fsa/libriheavy |
  | Emilia | Gate prompt: "1. The researcher shall use the Emilia dataset under the CC-BY-NC license and the Emilia-YODAS dataset under the CC-BY license." (metadata tag `cc-by-4.0`). Kyutai's post links the original Emilia paper and does not say which part it used. | https://huggingface.co/datasets/amphion/Emilia-Dataset (gated; public metadata, last modified 2025-02-28) |

  (Gate prompts quoted with their line breaks joined. The gated cards themselves answered HTTP 401;
  the terms above are the text any visitor sees before accepting.)
- **The weights a player would get today are later ones, with no data list.** The default English
  config, https://github.com/kyutai-labs/pocket-tts/blob/main/pocket_tts/config/english.yaml:
  > # Default English model: identical weights to english_2026-10.yaml.

  and `english_2026-10.yaml`:
  > # english_2026-09_2 recipe (teacher and student on re-aligned Dora data, EMA weights) retrained with the training-loader fix

  and `english_2026-09.yaml`:
  > # english_2026-04 re-distilled from a robustness-fine-tuned 24-layer teacher

  No page read says what the "Dora data" is. The multilingual post of 2026-05-04 ("Pocket TTS now
  supports six languages") states no training data for French, German, Spanish, Portuguese or
  Italian; nothing read covers Dutch.
- **The small models** (post of 2026-10-20, "The tiny TTS was actually 10x too big", Manu Orsini):
  > We train on HiFi-TTS-2, 11.3M utterances of public audiobook speech, with training stages of 100k steps of 64 utterances, enough for every stage to converge.

  The same post distils them from a teacher model ("Here the target is the teacher’s head").
  HiFi-TTS-2, https://huggingface.co/datasets/nvidia/hifitts-2: `license: cc-by-4.0`.
- **Mimi's data**: its card says "The training data is detailled in the paper." (Kyutai's Moshi
  report, not read here.)
- **The way to a voice with no data question**: Kyutai released its training code in August 2026
  (MIT, `training/` in the repository). The post of 2026-08-25, "Pocket TTS training code released":
  > A TTS training run will cost you less than $200 if you rent your hardware, and an order of magnitude less if you just pay for power.

  The training README's example uses HiFi-TTS-2: "Use 200 hours of audio for a model that speaks,
  2000 for one that actually sounds good." A retrain would still use Kyutai's Mimi codec.

### F4. The default voices and where each comes from

The card's voice list, each linked to its source file (ungated card, research date):

| Voice | Source file | Set | Terms of that set (F5) |
|---|---|---|---|
| alba | `alba-mackenna/casual.wav` | Alba MacKenna | CC BY 4.0 |
| anna, azelma, charles, eponine, eve, fantine, george, jane, mary, michael, paul, vera | `vctk/p228..p361_023_enhanced.wav` | VCTK | CC BY 4.0 |
| bill_boerst, caro_davy, peter_yearsley, stuart_bell | `voice-zero/*.wav` | Voice-Zero (LibriVox) | CC0 |
| javert, marius | `voice-donations/Butter.wav`, `Selfie.wav` | Volunteer donations | CC0 |
| estelle (fr) | `unmute-prod-website/developpeuse-3.wav` | Kyutai's own recordings | CC0 |
| **cosette** | `expresso/ex04-ex02_confused_001_channel1_499s.wav` | Expresso | **Non-commercial** |
| **jean** | `ears/p010/freeform_speech_01_enhanced.wav` | EARS | **Non-commercial** |
| **giovanni** (it), **lola** (es) | Common Voice clips on the gated repository's `add_lang_not_documented` branch | not read | not established |
| **juergen** (de), **rafael** (pt) | `de-DE-juergen.mp3`, `g-Vi8PgmSY0-enhanced-v2.wav` on the same gated branch | none stated | **undocumented** |

The card itself points to the voice repository for terms:
> You can take a look at [this page](https://huggingface.co/kyutai/tts-voices) which details the licenses

### F5. The consent record behind each voice set

**The voice repository**, https://huggingface.co/kyutai/tts-voices (created 2025-07-01, last modified
2026-03-09, not gated). Its card, set by set:

- **Volunteer donations** (`voice-donations/`):
  > Voices of volunteers submitted through our [Unmute Voice Donation Project](https://unmute.sh/voice-donation), licensed as CC0.

  > The Voice Donation project ran from June 2025 to February 2026 and collected 228 voices that passed verification, out of a total 374 submissions.

  **What each donor agreed to.** "Kyutai Unmute Voice Donation Project Terms of Use", "Version
  Effective Date: 26 June 2025" (https://kyutai.org/legal/Terms%20of%20Use%20-%20Unmute%20Voice%20Donation%20Project%20v1.pdf,
  linked from https://unmute.sh/voice-donation/terms-of-use). Section 5:
  > Publish your Voice Contribution as part of a public dataset under an open-source license (e.g. Creative Commons CC0 license). This means anyone, individuals or organizations, may download, copy, modify, transform, and use your Voice Contribution for any purpose (commercial or non-commercial) without restriction, subject to our Acceptable Use Policy.

  Section 6, the donor's warranties, include:
  > The voice in the recording is your own, and you have full legal rights and authority to contribute to it.

  Section 7:
  > Please note that we impose on any user of the open-source voice dataset the obligation to process your Voice Contribution in accordance with all applicable laws, in particular any applicable data protection and privacy laws.

  > Kyutai cannot control all potential future uses of your Voice Contribution. Specifically, in the context of TTS models, including Kyutai's models, it is possible that your voice could be used to generate speech or content that you did not personally create or authorize. By contributing, you acknowledge and accept this possibility.

  (Quoted from the PDF's text with its line breaks joined.) **The donation page as donors saw it**
  (archived 10 February 2026, https://web.archive.org/web/20260210073052/https://unmute.sh/voice-donation;
  the page's text is in its script file) required three boxes, among them:
  > I authorize Kyutai to collect, process and publish worldwide my voice recordings and embeddings as part of public datasets under a CC0 license or similar open-source license, in accordance with Kyutai’s

  > I authorize Kyutai to use my voice recording and embedding worldwide to develop and train Kyutai’s AI models and make them available to the public, in accordance with Kyutai’s

  (each ends with a link reading "Privacy Policy"), and told them:
  > Your voice may be made available for use with Kyutai Text-To-Speech. As a result, third parties could generate synthetic speech that closely resembles your voice. After public release, each third-party user will be directly responsible for its own use of your voice recording.

  **Withdrawal.** The donation project's privacy policy (https://kyutai.org/legal/Privacy%20Policy%20-%20Unmute%20Voice%20Donation%20Project%20v1.pdf,
  no date on its face, "v1" in its address), "Consent withdrawal":
  > We will do our best efforts to have your personal data deleted from the existing available open-source voice dataset, for example by deleting your personal data from said dataset, but we cannot guarantee that users of said open-source datasets, who may have accessed your personal data before their deletion, will stop using your personal data, even though they are obliged by our Acceptable Use Policy to regularly review any update to the open-source voice dataset so to identify any consent withdrawal requests.

  and earlier:
  > While we impose on any person accessing the open-source voice dataset an Acceptable Use Policy prohibiting them to use your voice in any unlawful manner, we will have very limited means to enforce this policy due to the open-source nature of the voice dataset.

  The "Acceptable Use Policy" these documents name was **not found**: neither the donation page,
  its archived copy, the terms, the privacy policy nor the voice repository links to it. The only
  use rules Kyutai publishes beside the voices are Pocket TTS's "Prohibited use" (F2).
- **Alba MacKenna** (`alba-mackenna/`):
  > Characters voice-acted by Alba MacKenna:
  > - Casual: Very casual flavour dialogue.
  > - Merchant: A seller you'd typically encounter in RPG's etc.
  > - Announcer: An announcer you'd hear typically in competitive games.
  > - A Moment By: Private recordings requested by Kinder World for their 'Moment' series.
  >
  > Released under the CC BY 4.0 licence.

  This is Kyutai's statement; no release signed by the actor is published, and nothing read says
  what Kinder World agreed to for the recordings it requested.
- **Kyutai's own recordings** (`unmute-prod-website/`):
  > The others are our own recordings and you may use them as CC0.

  The three exceptions it lists: `degaulle-2.wav` (Kyutai: "I don't understand how the license here
  works exactly, but I think it's safe to assume this recording is in the public domain since it's
  from 1940."), `ex04_narration_longform_00001.wav` ("comes from the Expresso dataset, so CC-NC") and
  `p329_022.wav` ("comes from VCTK, so CC BY 4.0").
- **Voice-Zero** (`voice-zero/`):
  > A couple of voices from [Voice-Zero](https://github.com/OwenTyme/voice-zero), a curated selection of voices from LibriVox.

  > Released under the CC0 licence.

  Voice-Zero's own README, https://github.com/OwenTyme/voice-zero:
  > Unless otherwise noted, all files in this repository are under the [CC0 license](https://creativecommons.org/public-domain/cc0/).
- **VCTK** (`vctk/`):
  > From the [Voice Cloning Toolkit](https://datashare.ed.ac.uk/handle/10283/3443) dataset, licensed under the Creative Commons License: Attribution 4.0 International.

  (Line break joined.) The University of Edinburgh's record, "CSTR VCTK Corpus: English Multi-speaker
  Corpus for CSTR Voice Cloning Toolkit (version 0.92)" (available 2019-11-13), carries
  `license_text.txt`, which opens "Creative Commons License: Attribution 4.0 International". No
  statement about the speakers' consent to synthetic voices was found there.
- **Expresso and EARS**: both
  > licensed under the Creative Commons License: Attribution-NonCommercial 4.0 International.

  each followed by "**Non-commercial use only.**"
- **CC BY 4.0 does not license a speaker's personality rights**, section 2(b)(1):
  > Moral rights, such as the right of integrity, are not licensed under this Public License, nor are publicity, privacy, and/or other similar personality rights; however, to the extent possible, the Licensor waives and/or agrees not to assert any such rights held by the Licensor to the limited extent necessary to allow You to exercise the Licensed Rights, but not otherwise.

  (Line breaks joined.) So a CC BY or CC0 label on a recording covers copyright; whether the person
  speaking agreed to become a synthetic voice is a separate record, which only the donation set has
  in writing.

### F6. No eSpeak in Pocket TTS

- `pyproject.toml` (package version 4.0.1) lists `sentencepiece`, `tokenizers`, `torch` and others;
  no phonemizer and no eSpeak. The configs point to a `tokenizer.model` per language. audio.cpp's
  port reads text the same way (its `text_conditioner.cpp` calls `tokenize_sentencepiece`); see
  [`2026-11-08-audio-cpp.md`](2026-11-08-audio-cpp.md).

### F7. What attribution CC BY 4.0 asks

- Section 3(a)(1): if You Share the Licensed Material, You must
  > a. retain the following if it is supplied by the Licensor with the Licensed Material:

  namely "identification of the creator(s) of the Licensed Material and any others designated to
  receive attribution, in any reasonable manner requested by the Licensor", "a copyright notice", "a
  notice that refers to this Public License", "a notice that refers to the disclaimer of warranties",
  "a URI or hyperlink to the Licensed Material to the extent reasonably practicable"; and
  > b. indicate if You modified the Licensed Material and retain an indication of any previous modifications; and

  > c. indicate the Licensed Material is licensed under this Public License, and include the text of, or the URI or hyperlink to, this Public License.

  Section 3(a)(2): these may be met "in any reasonable manner based on the medium, means, and context
  in which You Share the Licensed Material", for example by "providing a URI or hyperlink to a
  resource that includes the required information".
- **What Kyutai supplies**: no copyright notice (neither the cards nor the MIT `LICENSE` has one); an
  "Authors" section on the card:
  > Manu Orsini*, Simon Rouard*, Gabriel De Marmiesse*, Václav Volhejn, Neil Zeghidour, Alexandre Défossez

  and, in the multilingual post, a citation request:
  > Pocket TTS is based on the method introduced in the CALM paper. If you use Pocket TTS, please cite that paper:

  (the CALM paper, arXiv:2509.06926, and the release post).
- **For voices**: Alba MacKenna and VCTK are CC BY 4.0, so each needs the same kind of credit
  (creator, licence, link); CC0 voices need none, though naming the donation project is a courtesy
  the voice repository asks of academic work.

### F8. Whether a "computer voice" disclosure is required anywhere

- **CC BY 4.0, MIT and CC0**: no labelling term.
- **Kyutai's "Prohibited use"** (F2): forbids "presenting generated content as genuine recordings of
  real people or events". A label is the plain way to never do that.
- **The donation terms** (F5) put responsibility for each use on "each third-party user"; they
  require lawful processing but no label.
- **The EU AI Act**, read through the Commission's AI Act Service Desk, which reproduces the
  "EUR-Lex consolidated version of the AI Act as at 27 July 2026" (pages served with Last-Modified
  6 November 2026; EUR-Lex itself refused the reader, below):
  - Article 3(60), https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-3:
    > ‘deep fake’ means AI-generated or manipulated image, audio or video content that resembles existing persons, objects, places, entities or events and would falsely appear to a person to be authentic or truthful;
  - Article 50(4), https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-50:
    > Deployers of an AI system that generates or manipulates image, audio or video content constituting a deep fake, shall disclose that the content has been artificially generated or manipulated.

    > Where the content forms part of an evidently artistic, creative, satirical, fictional or analogous work or programme, the transparency obligations set out in this paragraph are limited to disclosure of the existence of such generated or manipulated content in an appropriate manner that does not hamper the display or enjoyment of the work.
  - Article 50(2) (providers marking synthetic audio "in a machine-readable format"), Article 50(5)
    (timing), the open-source rule and the date of application were read on 2026-11-06 and are
    quoted in [`2026-11-06-social-platform-apis.md`](2026-11-06-social-platform-apis.md), items EU-2,
    EU-4, EU-6 and EU-7; whether the project is a "provider" or "deployer" was left open there.

## What is still unknown, and what would settle it

1. **Whether the non-commercial and research-only terms of GigaSpeech, SPGISpeech, TED-LIUM and
   Emilia reach the Pocket TTS weights, or anyone downstream.** The terms are agreements between the
   dataset publishers and the researchers who accepted them (Kyutai); the project accepted none.
   Whether a model trained on such data carries them is unsettled law. Settled only by a court, or
   practically by Kyutai explaining its basis for CC BY 4.0 (one message to Kyutai; free) or by a
   lawyer (cost unknown). Removing the question costs a retrain on documented data: Kyutai's own
   estimate is under USD 200 of rented hardware (F3).
2. **What trained `english_2026-10`, the other five languages and Dutch.** Only Kyutai can say: a
   GitHub issue on `kyutai-labs/pocket-tts` (free; posting is the operator's call).
3. **Where Kyutai's "Acceptable Use Policy" for the donated voices is published**, and how a user is
   told of a withdrawal. Settled by asking `unmute@kyutai.org` (the contact the terms give; free).
4. **Whether Kyutai holds a written release from Alba MacKenna, and Kinder World's position on "A
   Moment By".** Only Kyutai can say (same channels).
5. **Whether the GDPR reaches the project shipping donated voices** (a voice recording is personal
   data; the donation terms pass on "data protection and privacy laws"). The project's own
   2026-11-07 finding, [`2026-11-07-privacy-notice.md`](2026-11-07-privacy-notice.md), found the GDPR
   "probably not, not certain" for the website; shipping a donor's voice inside an app is a
   different act. A lawyer's question.
6. **Whether a game NPC's voice can be a "deep fake"** under Article 3(60) ("would falsely appear to
   a person to be authentic or truthful"), and the "provider" question for Article 50(2). A lawyer's
   question; the label below satisfies the fictional-work rule either way.

## Sources I could not reach

- **`kyutai/pocket-tts`'s card** (https://huggingface.co/kyutai/pocket-tts/raw/main/README.md): HTTP
  401, gated. Read from the ungated twin and the GitHub README instead. Not worked around. The same
  holds for the files on its `add_lang_not_documented` branch (the giovanni, lola, juergen and rafael
  samples).
- **The gated dataset cards** of Emilia, SPGISpeech and GigaSpeech (raw README: HTTP 401). Their
  terms were read from the gate prompts in the public metadata, which every visitor is shown.
- **TED-LIUM's primary pages**: https://openslr.org/51/ answered "Resource not found: 51" (to both
  readers), https://lium.univ-lemans.fr/en/ted-lium3/ and `?p=22550` answered 404, and the Hugging Face
  dataset `LIUM/tedlium` answered 401. The licence is therefore from a secondary source (F3). Hoped
  for: the licence in the publisher's words.
- **EUR-Lex** (https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:02024R1689-20260727):
  HTTP 202 with an empty body, to both readers. Not worked around; the Commission's Service Desk was
  used (F8).
- **Not opened**: the Moshi report (for Mimi's training data) and the CALM paper (arXiv:2509.06926);
  the release post's own "Data" section was the primary statement for the weights.

## What it means for this project

### Certain

- The project may ship Pocket TTS weights (converted to GGUF or not) in its release downloads and
  run them for NPC lines and the helper, under CC BY 4.0, with attribution (F1, F7). The code may be
  used under MIT.
- The "Prohibited use" statement is not a licence term (F2). The project may not add it, or any
  other term, to the weights as a condition on players (section 2(a)(5)(b)).
- "cosette" (Expresso) and "jean" (EARS) are non-commercial and cannot go into a CC0 project whose
  forks may be commercial; "juergen" and "rafael" have no stated source (F4, F5).
- Only the donation set carries a written consent to synthetic use from the person speaking (F5).
- A donated voice can be withdrawn, and Kyutai says dataset users are expected to check for
  withdrawals (F5).

### Judgement calls

- **Use the ungated weights** (`kyutai/pocket-tts-without-voice-cloning`). They need no acceptance,
  and a build that cannot clone arbitrary audio matches the project's rule to protect users: nobody
  can make an NPC speak in a neighbour's voice.
- **The training-data question is real but small for this project, and it must be said plainly.**
  Kyutai published its data list and chose CC BY 4.0; the dataset terms bind those who accepted them;
  HumanityOS is free and non-commercial. But forks of a CC0 project may be commercial, and "we checked
  where it came from" is a promise the project makes. Disclose the list in the voice's credits, and
  treat a project-trained Pocket TTS on documented data (HiFi-TTS-2, CC BY 4.0; LibriVox; the CC0
  donations) as the long-term default. Mimi's own data would still need its own look.
- **The voice set to ship**, in order of how well documented it is: Kyutai's own CC0 recordings
  (except the three exceptions) and the CC0 donations; Alba MacKenna's "Casual", "Merchant" and
  "Announcer" (CC BY 4.0, credit her; leave out "A Moment By"); the Voice-Zero LibriVox voices (CC0);
  VCTK voices (CC BY 4.0, credit the corpus). Never Expresso, EARS, `degaulle-2`, or the gated-branch
  voices.
- **Donated voices need a step before each release**: compare the shipped donation voices against
  the current `voice-donations/` folder and drop any that left. A tracked list (`data/voices/`) makes
  that a check a script can run. Prefer non-donated voices for the default NPC voices, so a
  withdrawal never removes a character's voice mid-game.
- **Label it "computer voice" wherever it speaks**, with a line in the voice settings. It meets
  Kyutai's statement, the fictional-work disclosure in Article 50(4) if it applies, and the project's
  honesty rule. Machine-readable marking (Article 50(2)) stays open with the "provider" question.

### How the project would credit it

- **Beside the model files**: the CC BY 4.0 notice, and a `README.md`: "Pocket TTS by Kyutai (Manu
  Orsini, Simon Rouard, Gabriel De Marmiesse, Václav Volhejn, Neil Zeghidour, Alexandre Défossez),
  https://huggingface.co/kyutai/pocket-tts-without-voice-cloning, licensed CC BY 4.0
  (https://creativecommons.org/licenses/by/4.0/), provided as-is without warranties. Converted to
  GGUF; otherwise unmodified. Includes Kyutai's Mimi codec (CC BY 4.0). Trained, per Kyutai's
  January 2026 post, on AMI, EARNINGS22, GigaSpeech, SPGISpeech, TED-LIUM, VoxPopuli, LibriHeavy and
  Emilia; later versions' data not published." The format conversion needs no modification notice
  under 2(a)(4), but saying so costs nothing (as the SimReady finding chose). If any code is ported,
  the MIT text goes with it.
- **Beside each voice**: its set, licence and credit line ("Merchant voice by Alba MacKenna, CC BY
  4.0"; "VCTK Corpus, CSTR, University of Edinburgh, CC BY 4.0"; "Voice donated to the Unmute Voice
  Donation Project, CC0").
- **`data/credits.ron`**, `CREDITS.md`, `LICENSES.md`: rows for Pocket TTS, Mimi, Alba MacKenna and
  VCTK with `attribution_required: true`; the CALM paper and release post cited on the Credits page,
  as Kyutai asks.

### The question for the operator

**May the NPC and helper voice be Kyutai's Pocket TTS, under CC BY 4.0, knowing its first weights
were trained partly on datasets whose own terms are non-commercial or research-only, and its current
weights on data Kyutai has not listed?**

Recommendation: **yes for now, and plan a project-trained voice.** Ship the ungated weights with only
the documented voices above, the data list and credits beside them, a "computer voice" label
everywhere, and a withdrawal check for donated voices before each release. Then train the project's
own Pocket TTS with Kyutai's MIT training code on documented data (HiFi-TTS-2, LibriVox, the CC0
donations), which by Kyutai's estimate costs under USD 200 of rented hardware, so the voice that
speaks for HumanityOS carries no question about what it learned from.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
8 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
