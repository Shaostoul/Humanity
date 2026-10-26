# HumanityOS's licence: public domain, or copyleft for code and the Library, or a middle way

**Researched 2026-10-25** (sources read on 25 October 2026, Pacific time). Every source below was
downloaded that day and its words copied from the download. Written for the community vote on the
project's licence that the operator asked for on 2026-10-25, so voters read the sources, not
anyone's opinion of them.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** What it says the project should do is marked as a judgement
call; the vote decides, and a lawyer could disagree with any of it.

## The question

Should HumanityOS stay public domain (CC0) for everything it makes, or put its code under the AGPL
and its Library and docs under CC BY-SA so it can use share-alike resources, or take a middle way,
and what does each choice actually change?

## Short answer

- **Public domain (CC0) is the most open choice, and it shuts out share-alike material.** Anything
  under CC BY-SA (Wikipedia, OpenStreetMap, Open Source Ecology, finding 6 of
  `2026-10-25-open-source-ecology.md`) can be linked or cited for facts, but never copied into a
  CC0 Library or release. CC0 also grants no patent licence, which is why the FSF and the OSI do
  not recommend it for software (findings 1, 4, 5).
- **AGPL for code makes anyone who runs a changed HumanityOS server for others share their
  changes** (finding 2), and **CC BY-SA for the Library and docs lets them take in share-alike
  material**, with GPLv3 code able to take in CC BY-SA 4.0 adaptations one way (finding 3).
- **Two limits apply to any change.** Every version already released as CC0 stays public domain for
  good (finding 1), so a change protects future work only. And much of HumanityOS was written with
  AI: the US Copyright Office says AI output is copyrightable where a human determined its
  expressive elements, and that prompts alone are unlikely to be enough (finding 6), so a copyleft
  licence binds only the parts that carry human authorship.

## Findings

### 1. CC0: no patent licence, and the waiver cannot be taken back

- Source: https://creativecommons.org/publicdomain/zero/1.0/legalcode.en (CC0 1.0 Universal,
  published 2009; read 2026-10-25).
- Section 4(a): "No trademark or patent rights held by Affirmer are waived, abandoned, surrendered,
  licensed or otherwise affected by this document."
- Section 2: the waiver is made "fully intending that such Waiver shall not be subject to
  revocation, rescission, cancellation, termination, or any other legal or equitable action to
  disrupt the quiet enjoyment of the Work by the public".

### 2. The AGPL's network clause

- Source: https://www.gnu.org/licenses/agpl-3.0.en.html ("Version 3, 19 November 2007").
- Section 13: "if you modify the Program, your modified version must prominently offer all users
  interacting with it remotely through a computer network (if your version supports such
  interaction) an opportunity to receive the Corresponding Source of your version".
- Section 11: "Each contributor grants you a non-exclusive, worldwide, royalty-free patent license
  under the contributor's essential patent claims, to make, use, sell, offer for sale, import and
  otherwise run, modify and propagate the contents of its contributor version."

### 3. CC BY-SA 4.0 and GPLv3: compatible one way

- Source: https://creativecommons.org/share-your-work/licensing-considerations/compatible-licenses/
  (read 2026-10-25).
- "The GNU General Public License version 3 was declared a "BY-SA–Compatible License" for version
  4.0 on 8 October 2015. Note that compatibility with the GPLv3 is one-way only, which means you may
  license your contributions to adaptations of BY-SA 4.0 materials under GPLv3, but you may not
  license your contributions to adaptations of GPLv3 projects under" BY-SA 4.0.

### 4. The FSF on CC0

- Source: https://www.gnu.org/licenses/license-list.html, section "CC0 1.0 Universal" (read
  2026-10-25).
- "Both public domain works and the lax license provided by CC0 are compatible with the GNU GPL. If
  you want to release your non-software work to the public domain, we recommend you use CC0. For
  works of software it is not recommended, as CC0 has a term expressly stating it does not grant
  you any patent licenses."

### 5. The OSI on CC0

- Source: https://opensource.org/faq, "What about the Creative Commons "CC0" ("CC Zero") public
  domain dedication? Is that Open Source?" (read 2026-10-25).
- "At this time, we do not recommend releasing software using the the CC0 public domain
  dedication." And: "CC0 was not explicitly rejected, but the License Review Committee was unable
  to reach consensus that it should be approved".

### 6. The US Copyright Office on AI-written work

- Source: Copyright and Artificial Intelligence, Part 2: Copyrightability (a report of the
  Register of Copyrights, January 2025),
  https://www.copyright.gov/ai/Copyright-and-Artificial-Intelligence-Part-2-Copyrightability-Report.pdf
  (downloaded 2026-10-25, text extracted locally).
- Conclusion: "in many circumstances these outputs will be copyrightable in whole or in part--where
  AI is used as a tool, and where a human has been able to determine the expressive elements they
  contain. Prompts alone, however, at this stage are unlikely to satisfy those requirements."

## The three plans on the ballot

| | A. Public domain (now) | B. Copyleft split | C. Library share-alike only |
|---|---|---|---|
| Code | CC0 | AGPL-3.0 | CC0 |
| Library and docs | CC0 | CC BY-SA 4.0 | CC BY-SA 4.0 |
| Plain data (game data, tables) | CC0 | CC0 | CC0 |
| Copy share-alike text into the Library (Wikipedia, OSM, OSE) | No | Yes | Yes |
| Bring GPL or AGPL code into the program | Only by making the whole program GPL (finding 4) | Yes | Only by making the whole program GPL |
| A changed server run for others must share its source | No | Yes (finding 2) | No |
| A patent grant from contributors | No (finding 1) | Yes (finding 2, section 11) | No |
| Anyone may use it with no conditions at all | Yes | No: share-alike and source-sharing conditions | Code yes, Library no |

Plan B also needs a short trademark policy for the name "HumanityOS", and so do A and C: no
licence above covers a name (finding 1), and a fake "HumanityOS" that steals recovery phrases is a
real risk to people.

## What is still unknown, and what would settle it

- **How much of HumanityOS carries human authorship** in the Copyright Office's sense (finding 6):
  the operator directs, reviews and decides, and the AI writes most of the text. Only a lawyer, or a
  court, can say where that line falls for this project; it decides how much a copyleft licence
  could actually enforce.
- **What a change means for contributions from other people**, if any were made under CC0: they
  stay CC0 (finding 1); whether they can be included in an AGPL or CC BY-SA whole is a question for
  each contribution. Settled by listing outside contributors from `git log` before any change.

## Sources not reached

- Fedora's 2022 decision to stop accepting CC0 for code: not fetched today; wanted as a second
  example of what finding 4 means for packaging.

## What it means for this project

**Certain (from the sources):**
- CC0 work can be linked to share-alike material and can cite its facts, but cannot contain its
  text; that is why `data/sources/registry.json` marks share-alike sources `use = "facts"`.
- Releases already made under CC0 stay public domain whatever is decided now.
- A copyleft licence has force only where there is copyright, which for AI-assisted work depends on
  human authorship.

**Judgement calls (the vote's to make):**
- Whether being usable by anyone with no conditions (A) is worth more than taking in the open
  commons (B or C) and keeping changed servers open (B).
- Whether the Library alone needs to change (C), which unlocks share-alike text for learning without
  putting conditions on the code.
