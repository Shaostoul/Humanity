# Music HumanityOS can ship: sample libraries, AI generators and services, and the evidence on 432 Hz, "healing frequencies" and brown noise

**Research date: 6 November 2026** (Pacific time). Every source below was read that day. Each one
carries its own published or last-updated date beside it, which is often years older. If you are
reading this later, compare those dates with whatever has changed since.

**I am not a lawyer. This is a reading of public sources by an AI working on the project, not legal
advice, and not medical advice.** Nobody qualified has reviewed it. Licences and terms of service
are texts whose reading a court could disagree with, and the health studies below are summarised
from their own abstracts, not re-analysed.

The design built on this finding is [`docs/design/music.md`](../../design/music.md).

## The question

Which routes can legally give HumanityOS music it can ship inside its open-source (CC0) app and let
anyone redistribute: open sample libraries, open music-generation models, commercial AI music
services, or human contributors; and what does the research actually say about A = 432 Hz tuning,
"solfeggio" or "healing" frequencies, relaxing music and brown noise?

## Short answer

- **Open sample libraries: yes, several, and the best are public domain.** Under CC0 (ship the
  samples in the app, render anything from them, no credit needed): **VSCO 2 Community Edition**
  (string sections, harp, woodwinds, brass, organ, two upright pianos, percussion), the **Versilian
  Community Sample Library** (grand pianos, concert harp, vibraphone, glockenspiel, tubular bells,
  hand chimes, wine glasses, gongs, a TX81Z FM synth and more), **Karoryfer's free libraries**
  (among them a hollowbody electric guitar, basses, a cello and saxophones) and FreePats' **Upright
  Piano KW**. The Fluid GM soundfont is MIT (ship it with its notice); the Salamander Grand Piano is
  CC BY 3.0 (ship it with a credit). **No public-domain choir library was found**, so a choir has to
  be synthesised by our own engine or recorded by contributors.
- **Not shippable inside a CC0 app:** **Sonatina Symphonic Orchestra** (CC Sampling Plus 1.0, a
  licence Creative Commons retired in 2011: whole copies only for noncommercial purposes, though
  music made from it may be commercial), the **Philharmonia** samples (never "as is", "as samples
  or as a sampler instrument"), **Virtual Playing Orchestra** as a bundle (it mixes Sampling Plus,
  CC BY-SA and its author's "do not repackage and sell") and **No Budget Orchestra** (CC BY-SA 4.0,
  per Virtual Playing Orchestra's page; share-alike cannot enter a CC0 release). The University of
  Iowa samples say "may be downloaded and used for any projects, without restrictions" but name no
  licence: very likely fine, worth one email to confirm.
- **Open music-generation models: one is legally clean, none fits the app.** Meta's MusicGen
  weights are CC BY-NC 4.0 (no commercial use, so not in a CC0 app). Stable Audio Open's licence
  ends at USD $1,000,000 of annual revenue and travels with the weights, and its own card says it is
  "better at generating sound effects and field recordings than music". Google's **Magenta
  RealTime** is Apache 2.0 code with CC BY 4.0 weights and "Google claims no rights in outputs", so
  it could legally be used, but a large neural model generating audio live competes with the
  renderer for the GPU, against the project's compute budget. ACE-Step's code is Apache 2.0; its
  weights' licence was not read.
- **Commercial AI services: not a route for the project.** Suno's Free and Basic tiers allow only
  "lawful, personal and non-commercial purposes", which a CC0 release cannot honour. Its paid tiers
  assign Suno's rights to the subscriber, but commercial use stays limited to its conditions and
  approved downloads, Suno keeps a perpetual, sublicensable licence in all output, and it "makes no
  representation or warranty to you that any copyright will vest in any Output". Udio's terms could
  not be read (behind a login).
- **The US Copyright Office:** "copyright can protect only material that is the product of human
  creativity", and "When an AI technology determines the expressive elements of its output, the
  generated material is not the product of human authorship" (2023 policy statement); the 2025
  report adds "not the mere provision of prompts". For HumanityOS, which is CC0, this changes
  nothing about whether such music can ship (CC0 waives whatever rights exist). It matters if the
  project ever wants a CC BY or BY-SA licence on music: "CC licenses are operative only when applied
  to material in which a copyright exists", so on purely machine-made music they would bind nothing.
- **Human contributors: ask for CC0 1.0** (the repository's licence and the shipped sound effects'
  licence), with a list of every sample library, plugin and AI service used in the piece, because
  a contributor cannot dedicate what a sample licence does not let them.
- **A = 432 Hz: not shown to do anything 440 does not.** The trials comparing the two are few and
  small (12 to 162 people), mostly from one research group, with mixed results: music beats no
  music, and the 432 versus 440 differences are small, inconsistent and often not significant. No
  systematic review of 432 Hz was found in PubMed. **Tuning to 432 is fine as a matter of taste**;
  the app must not say it heals or relaxes better.
- **"Solfeggio" and "healing" frequencies (528 Hz and the rest): no good evidence.** No human trial
  found compares a solfeggio frequency against other music as an active control; the rest are animal
  studies (one at 100 dB) and biomarker studies. Nothing supports "healing".
- **What does help people relax, with real evidence:** music interventions reduce stress-related
  measures across 104 trials (d = .380 physiological, d = .545 psychological), though for healthy
  people recovering from a laboratory stressor the pooled effect was small and not significant.
  Tempo drives arousal (slow is calmer), a pause lowered heart rate and blood pressure below
  baseline, and listeners' own preference and familiarity matter as much as the music's properties.
- **Brown noise: no evidence for focus or sleep specifically.** A 2024 meta-analysis found **no
  studies of brown noise** for attention (white and pink noise helped people with ADHD a little and
  hurt people without it); a 2021 systematic review rated the evidence for continuous noise as a
  sleep aid "very low" and warned it may affect sleep and hearing; white, pink and brown noise did
  not differ in pupil-linked arousal. **The ship's hum is atmosphere, not a treatment.**

## How this was read

Licence and terms pages were read through an automated web reader that extracts the passages asked
for; where it reported a passage as verbatim it is quoted below, and where the page could not be
read that is said. GitHub repository facts (folder names, sizes, licence identifiers, dates) come
from the GitHub REST API's metadata listings; **no sample, model or audio file was downloaded**. The
Copyright Office's 2023 policy statement was read as full text (the PDF from copyright.gov,
extracted locally with `pdftotext`). Health studies were read as their full PubMed abstracts through
the National Library of Medicine's E-utilities interface (PubMed's own pages refuse automated
reading).

## Part A: open sample libraries

What matters for each library is two separate permissions: (1) **shipping the samples inside the
app**, which HumanityOS does with everything under `data/` and lets anyone redistribute for any
purpose, commercial included; and (2) **rendering music from them**, ahead of time or live.

### A1. VSCO 2 Community Edition (Versilian Studios): CC0

- Sources: https://github.com/sgossner/VSCO-2-CE (repository last pushed 2021-04-06, release 1.1.0
  of 2019-01-17, per the GitHub API; licence file `LICENSE`, identified as CC0-1.0, whose first line
  is "CC0 1.0 Universal") and https://versilian-studios.com/vsco-community/ (page footer "© 2026",
  heading "Chamber orchestra 2.1.1 Community Edition").
- Quotations (Versilian page): "The library is under a Creative Commons 0 (i.e. public domain)
  license." and, for the WAV version, "Licensed under CC0 (Creative Commons Zero) you can do
  whatever you want with these samples."
- Contents (repository folders): Strings: Violin Section, Viola Section, Cello Section, Solo Violin,
  Solo Contrabass, Harp; Brass: F Horn, Trumpet, Tenor Trombone, OldTrombone, Tuba; Woodwinds:
  Flute, Piccolo, Oboe, Clarinet, Bassoon; Keys: Organ, Upright Piano, Upright Nr1; Percussion
  (timpani, glockenspiel, marimba, xylophone, gongs, cymbals and more). 3,187 files, 3,088 MiB in
  all; the string sections and harp together are 442.5 MiB.
- No choir. Its older address, https://vis.versilstudios.com/vsco-community.html, failed with an
  expired certificate.
- **Ship in the app: yes. Render from it: yes. Credit: not required.**

### A2. Versilian Community Sample Library (VCSL): CC0

- Source: https://github.com/sgossner/VCSL (last pushed 2026-01-14; release v1.2.2-RC of
  2019-01-03; licence identified as CC0-1.0).
- Quotations (README): "This collection is under a Creative Commons 0 license." and "Essentially
  it's Public Domain- you can do whatever you want with these sounds"; on size, "each instrument
  weighs about 20-75 MB in raw .wav format".
- Contents that matter for calm sci-fi music (folder, size): Grand Piano, Kawai (438.6 MiB); Grand
  Piano, Steinway B (1,277.7 MiB); Upright Piano, Yamaha (236.6 MiB); Concert Harp (73.1 MiB);
  Vibraphone (94.9 MiB); Glockenspiel (16.4 MiB); Tubular Bells 1 (62.5 MiB); Hand Chimes
  (41.0 MiB); Wine Glasses (41.7 MiB); Mark Trees (18.8 MiB); Gong 1 (29.3 MiB); Pipe Organ
  (138.4 MiB); Yamaha TX81Z FM synthesiser presets; timpani. 4,282 files, 5,875 MiB in all.
- No choir.
- **Ship in the app: yes. Render from it: yes. Credit: not required.**

### A3. Karoryfer Samples, the free libraries: CC0

- Source: https://shop.karoryfer.com/pages/free-samples (page footer "© 2026"; an Osiris Piano entry
  is labelled a "2026 KVR Developer Challenge entry").
- Quotations: "All our free sample libraries are under a Creative Commons Zero license." "Older
  downloads may include a less permissive license, but we changed that and now they are all CC0."
  The exception: "Only Marie Ork has a different license".
- Free instruments listed include Black And Green Guitars (a hollowbody electric), Shinyguitar,
  several electric and upright basses, Karoryfer x bigcat cello, String Cyborgs, two saxophones, War
  Tuba, Cowsynth, Caveman Cosmonaut, Osiris Piano and several drum kits. No choir (the vocal
  libraries are a voice bank under another licence and female death-metal vocals).
- **Ship in the app: yes (re-check the licence file inside each download: the page warns older
  downloads may carry an older licence). Render from it: yes.** The electric guitar is the most
  useful find for the "soft metal" colour.

### A4. FreePats: per instrument; CC0 preferred

- Source: https://freepats.zenvoid.org/licenses.html (no date on the page; the GPL deprecation note
  is anchored "History-and-News#20220711").
- Quotations: CC0 is "Worldwide public domain dedication. Easy to use. It allows everything."
  "FreePats currently prefers Creative Commons CC0, please use it for your new contributions." CC BY
  "4.0 license is a free license which requires attribution"; the GPL with a sound exception "has
  been deprecated and it won't be accepted anymore".
- Source: https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html (read 2026-11-06).
  - **Upright Piano KW** (version 2022-02-21): "Published under the terms of the Creative Commons
    CC0 1.0 public domain dedication." SFZ+FLAC 32 MiB; the small SFZ+FLAC set 2.9 MiB.
  - **Salamander Grand Piano** (version V3+2020-06-02): "Creative Commons Attribution 3.0" license.
    SFZ+FLAC 707 MiB; 44.1 kHz 16-bit WAV 394 MiB.
  - **YDP Grand Piano** (2016-08-04): "Creative Commons Attribution 3.0" license.
- **Ship: yes for CC0 sets; yes with a credit for CC BY sets. Render: yes.** Check each
  instrument's own licence.

### A5. University of Iowa Musical Instrument Samples: "without restrictions", no named licence

- Source: https://theremin.music.uiowa.edu/MIS.html (dates on the page: freely available since
  1997; recording method changed from 2011; pre- and post-2012 catalogues).
- Quotation: the recordings are "freely available on this website" and "may be downloaded and used
  for any projects, without restrictions."
- Families: woodwinds, brass, strings, percussion, piano, guitar.
- **Ship: very likely, but the page names no licence and does not mention redistributing the
  samples themselves. Render: yes.** One message to the studios would settle it.

### A6. Sonatina Symphonic Orchestra: CC Sampling Plus 1.0 (not shippable in a CC0 app)

- Sources: https://sfzinstruments.github.io/orchestra/sso (page "© 2020-2025 SFZ Instruments
  Organization"; original by Mattias Westlund in 2011, maintained by Peter Eastman since 2017) and
  https://github.com/peastman/sso (README).
- Quotations: licence "CC Sampling Plus 1.0"; README: "SSO may be used and distributed under the
  terms of the Creative Commons Sampling Plus 1.0 license." It includes "other useful instruments
  like piano, harpsichord, and chorus" (a "Chorus" with variations "Mixed, Large"); size "1.39 GB".
- The licence's own deed, https://creativecommons.org/licenses/sampling+/1.0/: you may "sample,
  mash-up, or otherwise creatively transform this work for commercial or noncommercial purposes"
  and "perform, display, and distribute copies of this whole work for noncommercial purposes";
  "You may not use this work to advertise for or promote anything but the work you create from it";
  "You must give appropriate credit, provide a link to the license, and indicate if changes were
  made." And: "2011-09-12: Creative Commons has retired this legal tool and does not recommend that
  it be applied to works."
- The old project pages failed: http://ssol.sourceforge.net/ redirects to a SourceForge project page
  that returned 404, and http://sso.mattiaswestlund.net/ failed its certificate check.
- **Ship in the app: no** (anyone may redistribute HumanityOS commercially; whole copies of the
  samples may only go out noncommercially). **Render music from it: yes, commercially too, with the
  credit**, but then the music is not cleanly public domain. It is the only open library found with
  a sampled choir.

### A7. Philharmonia Orchestra sound samples: never "as is"

- Source: https://philharmonia.co.uk/resources/sound-samples/ (no date on the terms; footer
  "© 2026 Philharmonia").
- Quotations: "You are free to use these samples as you wish, including releasing them as part of a
  commercial work." "The only restriction is that they must not be sold or made available 'as is'"
  "(i.e. as samples or as a sampler instrument)."
- **Ship in the app: no** (a game's sampler voices are exactly "a sampler instrument"). **Render
  music from them: yes**, including commercially.

### A8. Virtual Playing Orchestra: a bundle of mixed licences

- Source: http://virtualplaying.com/virtual-playing-orchestra/ (version 3.3 scripts, 3.2 wave
  files; the newest comment is dated 21 October 2026).
- Quotations: "I enforce no restrictions on using this library to make music, even for commercial
  purposes"; "I do not feel it is right to repackage and sell this library in part or in whole for
  profit." It lists its sources' licences: Sonatina "Creative Commons Sampling Plus 1.0 license.",
  Mattias Westlund's additional samples CC BY-SA 3.0, No Budget Orchestra CC BY-SA 4.0, VSCO 2 CE
  CC0, Iowa "freely available ... without restrictions", the Philharmonia samples.
- **Ship as a bundle: no. Render from it: yes, under each source's terms.** Take its CC0 parts from
  their originals instead.

### A9. No Budget Orchestra: CC BY-SA 4.0, per a secondary page

- Only source read: Virtual Playing Orchestra's licence list (A8). Its own repository's licence was
  not reached in this research. Share-alike material cannot go into a CC0 release (the 2026-10-25
  licence finding, finding 3 and the short answer).
- **Ship: no. Render: the result would be share-alike.**

### A10. The Fluid (R3) General MIDI soundfont: MIT

- Source: Debian's copyright file for the `fluid-soundfont` package,
  https://metadata.ftp-master.debian.org/changelogs//main/f/fluid-soundfont/fluid-soundfont_3.1-5.3_copyright.
- Quotations: "MIT license" ("described in COPYING"); "Permission is hereby granted, free of charge,
  to any person obtaining a copy of this software and associated documentation" (the opening of the
  MIT grant); "Copyright © 2000-2002, 2008 Frank Wen".
- A complete General MIDI set (strings, choir "aahs", guitars including overdriven ones), at the
  quality of a soundfont of that era.
- **Ship: yes, with the MIT notice. Render: yes.**

### A11. Not settled: GeneralUser GS and MuseScore General

GeneralUser GS's page (https://schristiancollins.com/generaluser.php) returned no licence text to
the reader, and the MuseScore General licence file
(https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General-License.md)
returned 404. Neither is needed while the CC0 libraries cover the same instruments.

### A12. A choir

No public-domain choir sample library was found (a search of the sfzinstruments catalogue, FreePats
and Karoryfer turned up none; Sonatina's chorus is Sampling Plus). Two routes remain: synthesise the
choir (formant synthesis, which the prototype does) or record contributors singing under CC0.

## Part B: open music-generation models

### B1. Meta MusicGen (AudioCraft): code MIT, weights CC BY-NC 4.0

- Source: https://github.com/facebookresearch/audiocraft (no release date shown). Quotations: "The
  code in this repository is released under the MIT license as found in the LICENSE file." "The
  models weights in this repository are released under the CC-BY-NC 4.0 license as found in the
  LICENSE_weights file."
- Source: https://huggingface.co/facebook/musicgen-large ("MusicGen was trained between April 2023
  and May 2023"; paper 8 June 2023). Quotations: "Code is released under MIT, model weights are
  released under CC-BY-NC 4.0." "The model was trained on licensed data using the following
  sources:" (the Meta Music Initiative Sound Collection, Shutterstock's and Pond5's music
  collections). "The model should not be used on downstream applications without further risk
  evaluation and mitigation."
- The NonCommercial term, https://creativecommons.org/licenses/by-nc/4.0/: "You may not use the
  material for commercial purposes." "A commercial use is one primarily intended for commercial
  advantage or monetary compensation."
- **Ship the model: no. Use its output in a CC0 release: not recommended**; whether a model's
  output counts as material adapted from NC-licensed weights is not settled by anything read.

### B2. Stable Audio Open 1.0: Stability AI Community License

- Source: https://huggingface.co/stabilityai/stable-audio-open-1.0 (paper "Published Jul 19, 2024").
  Quotations: "License: stable-audio-community"; training data "472618 are from Freesound and 13874
  are from the Free Music Archive (FMA)", "All audio files are licensed under CC0, CC BY, or CC
  Sampling+."; "The model is not able to generate realistic vocals."; it "is better at generating
  sound effects and field recordings than music."
- Source: https://stability.ai/community-license-agreement ("Last Updated: July 5, 2024").
  Quotations: section IV(c)(iii), "You own any outputs generated from the Models or Derivative Works
  to the extent permitted by applicable law."; section III sets the limit at those who "generate more
  than USD $1,000,000 in annual revenue", above which the licence ends and Stability's own licence
  must be requested; section IV(b) forbids using outputs "to create or improve
  any foundational generative AI model (excluding the Models or Derivative Works)"; section IV(a)
  requires giving the Agreement to anyone who receives the Materials.
- **Ship the model: no** (the revenue limit would travel to everyone who redistributes HumanityOS).
  **Use outputs: allowed by the licence**, but by its own card it is weakest at music.

### B3. Google Magenta RealTime: code Apache 2.0, weights CC BY 4.0

- Source: https://huggingface.co/google/magenta-realtime (paper "Live Music Models",
  arXiv:2508.04651, "Published Aug 6, 2025"; "Copyright 2025 Google LLC").
- Quotations: "the codebase is licensed under Apache 2.0, and the model weights under Creative
  Commons Attribution 4.0 International." "Google claims no rights in outputs you generate using
  Magenta RealTime." "You and your users are solely responsible for outputs and their subsequent
  uses." Users are asked not to "generate content, including outputs, that infringe or violate the
  rights of others." "Magenta RealTime was trained on ~190k hours of stock music from multiple
  sources, mostly instrumental." It was trained on TPUs and the card names deployment "in
  environments with limited resources, including live performance settings or freely available
  Colab TPUs."
- The card gives no GPU requirement. Secondary write-ups (not confirmed on the card) describe it as
  about 800 million parameters and report a "Magenta RealTime 2" of June 2026, which the card does
  not mention.
- **Legally the cleanest model read: weights redistributable with a credit, no claim on outputs.**
  Practically, a neural model generating audio every second would share the GPU with the renderer;
  see the design's compute budget.

### B4. ACE-Step: code Apache 2.0, weights licence not read

- Source: https://github.com/ace-step/ACE-Step ("2025.05.06: Open source demo code and model";
  "2026.01.28: Released ACE-Step v1.5"). Quotations: "This project is licensed under Apache License
  2.0." "The authors are not responsible for any misuse of the model, including but not limited to
  copyright violations"; "we encourage users to verify the originality of generated works, clearly
  disclose AI involvement". The page states no separate weights licence and does not describe the
  training data. v1.5's licence was not read.

## Part C: commercial AI music services and the US Copyright Office

### C1. Suno

- Source: https://suno.com/terms ("Date of Last Revision: August 10, 2026"; "Effective Date:
  September 3, 2026").
- Quotations: on paid Pro and Premier plans, "Suno hereby assigns to you all of its right, title and
  interest in and to any Output owned by Suno." On Free and Basic, "you covenant and agree that you
  will only use such Outputs for your lawful, personal and non-commercial purposes." "You may
  commercially exploit Output solely to the extent it adheres to Suno's Conditions of Access and
  Use" and "You may not commercially exploit Output that has not been downloaded by you through an
  approved channel." Suno's licence in all Content, Output included, is "a worldwide, non-exclusive,
  fully paid-up, sublicensable (directly and indirectly through multiple tiers)" licence, described
  as perpetual and irrevocable, and Suno may "identify to the public (both on and off the Service)
  that Output (or any of it) was generated via the Service." And: "Suno makes no representation or
  warranty to you that any copyright will vest in any Output."
- **Free or Basic output: cannot go into HumanityOS** (a CC0 release lets anyone use it
  commercially). **Paid output: the subscriber could dedicate it**, subject to the conditions of
  use read here only in part, while Suno keeps its own licence.

### C2. Udio: not read

https://www.udio.com/terms-of-service returned only its title to the reader, and
https://www.udio.com/terms is a login page. Third-party summaries were found but are not used here.

### C3. The US Copyright Office

- The index, https://www.copyright.gov/ai/ (read 2026-11-06), lists Part 1 (Digital Replicas,
  31 July 2024), Part 2 (Copyrightability, 29 January 2025), Part 3 (Generative AI Training,
  pre-publication version 9 May 2025) and the registration guidance at 88 Fed. Reg. 16,190
  (16 March 2023).
- The 2023 policy statement, https://www.copyright.gov/ai/ai_policy_guidance.pdf ("DATED: MARCH 10,
  2023", "effective March 16, 2023"), read in full:
  - "In the Office's view, it is well-established that copyright can protect only material that is
    the product of human creativity."
  - "when an AI technology receives solely a prompt from a human and produces complex written,
    visual, or musical works in response, the "traditional elements of authorship" are determined
    and executed by the technology--not the human user." (footnote markers removed)
  - "When an AI technology determines the expressive elements of its output, the generated material
    is not the product of human authorship. As a result, that material is not protected by
    copyright and must be disclaimed in a registration application." (footnote markers removed)
  - "AI-generated content that is more than de minimis should be explicitly excluded from the
    application."
- The Part 2 announcement, https://www.copyright.gov/newsnet/2025/1060.html (Issue No. 1060,
  29 January 2025): outputs "can be protected by copyright only where a human author has determined
  sufficient expressive elements", which covers cases where "a human-authored work is perceptible in
  an AI output" or "a human makes creative arrangements or modifications of the output", "but not
  the mere provision of prompts." (The project's 2026-10-25 licence finding quotes the report's own
  conclusion from the full text.)
- Creative Commons' FAQ, https://creativecommons.org/faq/ (page dated 2025-05-13): "CC licenses are
  operative only when applied to material in which a copyright exists"; "CC licenses should not be
  applied to works in the worldwide public domain."; CC0 "is not a license; it is a public domain
  dedication." The part of the FAQ read had no entry on AI output itself (about 79,000 characters of
  the page were not read).
- The D.C. Circuit's decision in *Thaler v. Perlmutter* (No. 23-5233, 18 March 2025) was not read:
  Justia returned 403. It is not relied on here.

## Part D: human contributions

- The repository is CC0 1.0 (`LICENSE`), and the shipped sounds are too: `assets/audio/LICENSE.md`
  says they are "licensed CC0 1.0 Universal (public domain)".
- [`2026-10-12-media-item-licences.md`](2026-10-12-media-item-licences.md) found that NonCommercial
  or all-rights-reserved items "cannot go into the CC0 game as they stand", and that a CC licence or
  CC0 "cannot be taken back for copies already taken".
- [`2026-10-25-licence-options.md`](2026-10-25-licence-options.md) sets out the licence vote: every
  plan on the ballot keeps "Plain data (game data, tables)" as CC0. Music is not named on the ballot.
- `docs/dev/adding-sounds-music.md` already says "CC0 strongly preferred" and that CC BY needs a
  credit in `CREDITS.md`.

## Part E: the evidence on 432 Hz, solfeggio frequencies, relaxing music and brown noise

All abstracts were read in full from PubMed. "Significant" below is the authors' word.

### E1. A = 432 Hz against A = 440 Hz (or 443 Hz)

PubMed lists 24 records matching "432 Hz" in title or abstract (searched 2026-11-06); these are the
human trials among them. The difference between the two tunings is 31.77 cents, under a third of a
semitone (1200 x log2(432/440)).

- Calamassi D, Pomponi GP. *Explore (NY)* 2019;15(4):283-290, PMID 31031095 (corrigendum PMID
  32113613). Cross-over pilot, 33 volunteers, film music at both tunings, 20 minutes each. "432 Hz
  tuned music was associated with a slight decrease of mean (systolic and diastolic) blood pressure
  values (although not significant), a marked decrease in the mean of heart rate (-4.79 bpm, p =
  0.05)". Conclusion: "The study results suggest repeating the experiment with a larger sample pool
  and introducing randomized controlled trials covering more clinical parameters." Its background
  says "there are no scientific studies that support this hypothesis."
- Calamassi D, et al. *Acta Biomed* 2020;91(12-S):e2020008, PMID 33263352. Double-blind cross-over
  pilot, 12 patients with spinal cord injuries. "After listening to music at 432 Hz there was a
  significant improvement in sleep scores (+3.6, p=0.02), while there was no improvement in sleep
  scores listening to music at 440 Hz (-1.50, p=0.34)." "Listening times and wash-out periods were
  variable."
- Calamassi D, et al. *Acta Biomed* 2022;93(S2):e2022149, PMID 35545982. 54 emergency nurses, three
  arms (440 Hz, 432 Hz, no music). "The median values of STAI X1 decreased in all the 3 groups from
  T0 to T1"; in the 432 group "a reduction of mean values of respiratory rate and systolic blood
  pressure was recorded at T1".
- Aravena PC, et al. *J Appl Oral Sci* 2020;28:e20190601, PMID 32401941. 42 patients before tooth
  extraction. "Significantly lower anxiety level values were observed at 432 Hz (8.7±2.67) and 440
  Hz (8.4±2.84) compared to the control group (17.2±4.60; p<0.05)." Salivary cortisol was lower at
  432 Hz than at 440 Hz.
- Hohneck A, et al. *BMC Complement Med Ther* 2025;25:18, PMID 39844155. Randomised cross-over, 43
  cancer patients, a body monochord at 432 or 443 Hz ("the concert pitch in German professional
  orchestras"). "Both interventions led to a significant reduction in heart rate"; several measures
  changed only at 432 Hz, "Nevertheless, these effects were not significantly different compared to
  443 Hz", except pulse wave velocity.
- Trials comparing 432 Hz music with no music or other sounds, with no 440 Hz arm, show that music
  helped but cannot isolate the tuning: Di Nasso L, et al. *J Endod* 2016, PMID 27430941 (100
  patients, music against none); Menziletoglu D, et al. *Med Oral Patol Oral Cir Bucal* 2021, PMID
  33247575 (binaural beats, 432 Hz music, no treatment); Zaki H, et al. *J Pak Med Assoc* 2025,
  PMID 40269359 (432 Hz music, binaural beats, white noise: "No significant difference was found
  between the interventional groups"); Dubey P, et al. *J Family Med Prim Care* 2019, PMID 31879635
  (15 men, a daytime nap: "some decrease in the mean sleep latency (P > 0.05)").
- Context worth recording: the composer co-author of the three Calamassi trials is listed as
  "Independent film score composer and sound engineer. gp.pomponi@gp432.com." The trials declare no
  conflict of interest.
- A 2026 narrative review in *Cureus* (PMID 41909396) states that healing frequencies "including 432
  Hz and 528 Hz, are reported to reduce inflammation, promote cellular repair, and improve
  neuroplasticity", and also names "methodological variability and limited large-scale studies". It
  is not a systematic review; this finding does not treat it as evidence.

### E2. "Solfeggio" frequencies and 528 Hz

PubMed lists 10 records for "528 Hz" and 12 for "solfeggio" (most of the latter are about teaching
sight-singing). The relevant ones:

- Bhoot A, et al. *Ann Neurosci* 2025, PMID 41395351. 60 patients with stage 1 hypertension; OM
  chanting "composed at 528Hz" daily for a month against "standard care". Without an active control
  (other chanting, other music) it cannot separate the frequency from listening and meditating.
- Bozok ÜG, et al. *Brain Behav* 2026;16(5):e71452, PMID 42043979. 162 students, no music against
  528 Hz and 432 Hz music. Salivary BDNF and CREB rose with 528 Hz and GRP78 with 432 Hz; "No
  significant differences were observed between groups in Stroop test performance." No 440 Hz arm.
- Animal studies: zebrafish (PMID 37119977, "solfeggio-frequency music exposure"; PMID 42710333,
  where 528 Hz "induced a modest increase in GST activity" in embryos) and rats exposed to "528 Hz
  frequency in 100 dB intensity" (PMID 30414050).
- No human trial was found that compares a solfeggio frequency against ordinary music as an active
  control. The historical claims made for these numbers were not researched here.

### E3. What research supports about relaxing music

- de Witte M, et al. *Health Psychol Rev* 2020;14(2):294-324, PMID 31167611. Two meta-analyses of
  104 RCTs, 9,617 participants: "music interventions had an overall significant effect on stress
  reduction in both physiological (d = .380) and psychological (d = .545) outcomes"; "Larger effects
  were found on heart rate (d = .456), compared to blood pressure (d = .343) and hormone levels (d =
  .349)."
- Adiasto K, et al. *PLoS One* 2022;17(6):e0270031, PMID 35714120. 14 experiments, 706 healthy
  people after a laboratory stressor: "a non-significant cumulative effect of music listening on
  stress recovery g = 0.15"; effects "seemed to vary depending on musical genre, who selects the
  music, musical tempo, and type of stress recovery outcome."
- Bernardi L, et al. *Heart* 2006;92(4):445-52, PMID 16199412. 24 people, six styles: "Music
  induces an arousal effect, predominantly related to the tempo. Slow or meditative music can induce
  a relaxing effect; relaxation is particularly evident during a pause." "The pause reduced heart
  rate, blood pressure, and minute ventilation, even below baseline."
- Tan X, et al. *J Music Ther* 2012;49(2):150-79, PMID 26753216. The properties listeners found
  relaxing "included tempo, mode, harmonic, rhythmic, instrumental, and melodic complexities,
  timbre, vocalization/lyrics, pitch range, dynamic variations, and contour"; "music preference was
  highly correlated with listeners' perception of relaxation in music". It also notes "the dire lack
  of evidence to support such claims" made for commercial relaxation music.

### E4. Brown noise, and coloured noise generally

- Nigg JT, et al. *J Am Acad Child Adolesc Psychiatry* 2024;63(8):778-788, PMID 38428577.
  Meta-analysis: white and pink noise gave "a small but statistically significant benefit" on tasks
  for young people with ADHD symptoms (g = 0.249); "No studies of brown noise were identified."; for
  people without ADHD, "white and pink noise had a negative effect (g = -0.212".
- Riedy SM, et al. *Sleep Med Rev* 2021;55:101385, PMID 33007706. Systematic review of continuous
  noise and sleep: "the quality of evidence for continuous noise improving sleep was very low, which
  contradicts its widespread use", and more research is needed "especially since it may also
  negatively affect sleep and hearing."
- Ding Y, et al. *Sleep Med* 2025;136:106869, PMID 41151421. Meta-analysis of 12 RCTs of white
  noise: improvements in sleep-quality scores, with the study "limited by the heterogeneity of the
  included research and methodological quality". White noise, not brown.
- Erfanian M, et al. *Int J Psychophysiol* 2025;218:113271, PMID 41110471. 31 listeners: "no
  significant modulation of pupil size across noise conditions"; coloured noises "do not
  differentially affect sustained pupil-linked arousal in naïve listeners."
- A 2025 study rating brown noise "more sublime" than white or pink (Færøvik UH, et al. *Front
  Neurosci*, PMID 40352908) was **retracted** on 13 October 2025 and is not evidence.

## What is still unknown, and what would settle it

1. **Iowa's samples: may the files themselves be redistributed inside an app?** One email to the
   University of Iowa Electronic Music Studios asking that exact question. Cost: one message.
2. **No Budget Orchestra's own licence.** Read the licence file in its repository. Not needed while
   CC0 libraries cover the same instruments.
3. **Whether output of NC-licensed model weights is itself NC.** A lawyer, or Meta's own answer.
   Not needed if the project does not use MusicGen.
4. **Udio's terms.** Read with a signed-in browser by a person. Not needed if the project does not
   use Udio.
5. **Suno's Conditions of Access and Use**, which limit commercial use of paid output; only the
   Terms were read. Read them if a paid Suno route is ever proposed.
6. **Magenta RealTime's real cost on a desktop GPU** (memory, milliseconds per second of audio) and
   whether a smaller or distilled version runs on a CPU. A measurement on the operator's RTX 4070,
   which needs the model downloaded (his permission per file).
7. **Whether an ACE-Step weights licence exists on its Hugging Face page.** Read that page.
8. **Brown noise specifically:** no study was found. Settled only by new research; the app needs
   none, because it claims nothing.

## Sources not reached

- https://vis.versilstudios.com/vsco-community.html: certificate expired (wanted: the VSCO licence
  wording; got it from versilian-studios.com instead).
- http://ssol.sourceforge.net/ (to a SourceForge page that returned 404) and
  http://sso.mattiaswestlund.net/ (certificate failure): wanted the original Sonatina licence page.
- https://schristiancollins.com/generaluser.php: the reader received no licence text (wanted:
  GeneralUser GS's terms).
- https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General-License.md:
  404 (wanted: MuseScore General's licence).
- https://www.udio.com/terms-of-service and https://www.udio.com/terms: title only, then a login
  page (wanted: output ownership).
- https://law.justia.com/cases/federal/appellate-courts/cadc/23-5233/23-5233-2025-03-18.html: 403
  (wanted: the *Thaler v. Perlmutter* holding in the court's words).
- https://www.federalregister.gov/documents/2023/03/16/2023-05321/...: redirected to a bot check;
  the same policy statement was read from copyright.gov instead.
- https://pubmed.ncbi.nlm.nih.gov/31031095/: refuses automated reading (cookies); every abstract was
  read through E-utilities instead.
- https://archive.fosdem.org/2026/events/attachments/RPQVKY-foss-orchestra/slides/267608/fosdem202_01wjonr.pdf:
  a FOSDEM 2026 talk on free orchestral libraries, found but not read; it may list libraries this
  finding missed.
- ISO 16 (the A = 440 Hz standard) was not read.

## What it means for this project

### Certain

- HumanityOS may ship, inside the app and for anyone to redistribute, samples from VSCO 2 CE, VCSL,
  Karoryfer's free libraries and FreePats' CC0 instruments, with no credit required, and the Fluid
  soundfont with its MIT notice and the Salamander piano with a CC BY 3.0 credit.
- It may not ship Sonatina, the Philharmonia samples, Virtual Playing Orchestra as a bundle, or
  MusicGen's weights inside a CC0 release.
- Suno Free or Basic output cannot enter the project.
- Material whose expressive elements were determined by an AI is not protected by US copyright, per
  the Copyright Office. CC0 still works on it (there is simply nothing to waive); a CC BY or BY-SA
  licence would bind nothing on those parts.
- No study read supports a health claim for A = 432 Hz over 440 Hz, for solfeggio frequencies or for
  brown noise. The app must not make one.
- Slow tempo, few sudden changes, pauses, and music people like and recognise are the properties
  with research behind them.

### Judgement calls (an AI's reading; reject freely)

- **Real instruments: CC0 samples played by our own engine, not AI.** VCSL and VSCO 2 CE cover
  piano, harp, bells, vibraphone, glass, strings, woodwinds and brass; Karoryfer covers electric
  guitar and bass. The engine plays them live, so the music never repeats and every note obeys the
  tuning setting. A curated subset (a few velocity layers, every few semitones) keeps the shipped
  size small.
- **The choir is synthesised** (formant synthesis) until contributors record one under CC0. It
  will sound like a choir pad, not like a real choir singing words; that is the honest limit.
- **AI generators are not needed and add risk.** The legally cleanest (Magenta RealTime) breaks the
  compute budget if run live, and anything generated offline is fixed audio that cannot follow the
  tuning, the place or the time of day the way a live score can. If ever used, use it offline only,
  as raw material a human then arranges, and record the model and its licence beside the file.
- **Ask contributors for CC0 1.0** and a list of every sample library, plugin and AI service in
  their piece; accept CC BY 4.0 only with a credit line, never NC or share-alike.
- **Tuning to A = 432 Hz by default is a taste the operator chose**, and the Settings words should
  say so: a matter of taste, no health effect claimed. The optional "tones" layer and the ship hum
  carry the same plain words.

**Again: I am not a lawyer, and this is not legal or medical advice.** It is a dated reading of
public sources, written so a later reader can check the same words and see what changed.
