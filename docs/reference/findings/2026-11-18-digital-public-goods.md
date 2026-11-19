# Digital public good recognition: the DPG Standard against HumanityOS (2026-11-18)

**Why this was looked at.** The operator wants cities and nations to be able to trust HumanityOS as
their software infrastructure. Recognition as a digital public good (DPG), listed in the Digital
Public Goods Alliance's registry, is the formal signal that audience looks for. The operator, on
2026-11-18: "Let's pursue this."

**Last looked:** 2026-11-18.

## Sources

| Source | Its own date | Read |
|---|---|---|
| The standard's page, https://www.digitalpublicgoods.net/standard (lists the nine indicators; the requirement text sits in collapsed panels) | not dated on the page | 2026-11-18, robots.txt allows it |
| The standard's text, https://github.com/DPGAlliance/DPG-Standard/blob/main/standard.md | its header says version 1.1.4, 2021-01-04, but the changelog's latest entry is 1.1.7, 2025-06-26, and the repository was last pushed 2026-10-28: the header was not kept up | 2026-11-18 |
| The changelog, same repository, CHANGELOG.md | 1.1.7, 2025-06-26 | 2026-11-18 |

The standard is licensed CC BY-SA 4.0 (the standard's page: "The DPG Standard is an open standard
licensed under CC-BY-SA 4.0"), so the requirement text below is quoted with that credit.

## What the standard requires, and where HumanityOS stands

| Indicator (quoted from standard.md) | HumanityOS today | Verdict |
|---|---|---|
| 1. "Digital public goods must demonstrate relevance to advancing the Sustainable Development Goals" | Goal 1 (no poverty) is the mission; also 2 (food), 4 (education), 6 (water), 7 (energy), 16 (accountable institutions: signed voting) | meets |
| 2. "For open-source software, only OSI approved licenses are accepted." | Code AGPL-3.0-or-later (OSI approved); Library and docs CC BY-SA 4.0; data CC0; shipped outside data ODbL 1.0, CC BY-SA IGO 3.0, CC BY 4.0 (LICENSES.md). 1.1.7 added "requiring only fully open-licensed content collections and data sets": the optional instrument packs are all open too (CC0, the Unlicense, CC BY 4.0 and 3.0; checked 2026-11-18) | meets |
| 3. "Ownership of assets that the digital public good produces must be clearly defined and documented." | LICENSING.md names the licences and the trademark policy, but no file states who holds the copyright (it discusses AI-assisted authorship instead) | **gap: a plain copyright line** ("Copyright 2026 Shaostoul and contributors", or as the operator chooses) |
| 4. Mandatory dependencies more restrictive than the licence need open alternatives | wgpu, the RustCrypto and noble libraries, DXC (open source) on Windows, Metal/Vulkan elsewhere; optional AI models are separate downloads | meets |
| 5. "technical documentation that would allow a technical person unfamiliar with the project to launch and run the software" | README, docs/contributor/, CLAUDE.md; the macOS contributor's failure was fixed 2026-11-16 (docs/design/cross-platform-dev.md) | meets; a "your first hour" guide would make it plain |
| 6. Non-PII data and content extractable "in a non-proprietary format" | data/ is CSV, TOML, RON, JSON; the Library is Markdown | meets |
| 7. "designed and developed to comply with privacy and other applicable laws" | docs/outreach/privacy_policy.md, docs/user/rights/laws_that_limit_this_software.md, account erasure (data/erased-accounts.key) | meets, evidence to gather |
| 8. "align with relevant standards, best practices, and/or principles" | FIPS 203 and 204, BIP39, WebRTC, OpenStreetMap; ACVP conformance tests planned (the reset's first item) | meets, stronger after the reset |
| 9a. Data that is personally identifiable: show "how they ensure the privacy, security and integrity of this data" | Keys on the device, sealed DMs, the 2026-06 security audit, the 2026-11-16 PQC review; no outside review since the post-quantum switch | meets, with that said honestly |
| 9b. "policies identifying inappropriate and illegal content such as child sexual abuse materials in addition to processes for detecting, moderating, reporting and removing" | Block, reports, warnings and moderation tools exist (FEATURES, "Safety", v0.1465 to v0.1478); **no written content policy** that names illegal content and says how it is reported to the authorities and removed | **gap: a content policy and its process** |
| 9c. "a process for users and contributors to protect themselves against grief, abuse, and harassment" and "system(s) to address the safety and security of underage users" | Block, who-can-reach-me, reports; the protected setup for children (docs/design/blocking-and-safe-mode.md, section 6) | meets; describe it in the application |

The standard's note on 7 to 9: "Evidence for requirements 7-9 can only be given by someone
authorized to speak on behalf of the project. We collect title, name and contact information to
confirm this authority." So the application is the operator's to submit; the AI prepares it.

## Conclusion

HumanityOS meets most of the standard as it is. Two gaps stand between it and an application, and
both are worth closing for their own sake:

1. **A plain ownership statement** (indicator 3): who holds the copyright, beside the licences.
2. **A content policy** (indicator 9b): what is not allowed (child sexual abuse material first, and
   the Accord's absolute prohibitions), how a person reports it, how server operators remove it and
   report it to the authorities, and what end-to-end encryption means for it (sealed DMs cannot be
   scanned; reports from a participant can be acted on). Written plainly, linked from the app's
   safety settings and the website. This is protection work (CLAUDE.md: "We must protect the
   users"), not paperwork.

Then: run the DPGA's eligibility tool, gather the evidence for each indicator in one document, and
the operator submits.

## Not checked yet

- The review policy and the submission form's questions (the page links them).
- (Answered 2026-11-18: the instrument packs are all open. The core set and most packs are CC0 or the Unlicense; the two credited packs, Choir and Electric pianos, are CC BY 4.0, with CC BY 3.0 on one SFZ mapping: LICENSES.md, "The instrument packs".)

## Progress

- 2026-11-18: the copyright line is in (LICENSING.md, README). The instrument-pack licences are checked (all open).
- Next, at the weekly reset: the dated legal finding on what US law asks of a server operator about child sexual abuse material, then the content policy and its report-and-remove process (both apps, linked from Settings > Safety and the website), then the evidence document for the nine indicators, then the eligibility tool. The operator submits.
