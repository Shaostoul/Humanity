# California's Digital Age Assurance Act (AB 1043, as amended by AB 1856), and whether HumanityOS must ask the operating system for an age signal

**Researched 2026-10-10** (sources read between about 02:20 and 03:20 Pacific
time on 10 October 2026). Everything below was read on that day. The act takes
effect on 1 January 2027, it was amended one month before this was written,
and the operating systems are shipping their age interfaces now. If you are
reading this later, check each source's own date against anything that has
changed since, starting with the leginfo code page for Civil Code 1798.500,
which on 10 October 2026 did not yet show the 2026 amendments.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such, and a
lawyer could disagree with any of it.

**Supersedes in part:** the "California Digital Age Assurance Act (AB 1043)"
bullet in section 3 of
[`2026-10-10-childrens-online-safety-rules.md`](2026-10-10-childrens-online-safety-rules.md).
That bullet quoted the Civil Code as leginfo displays it, which is still the
2025 text. The law that takes effect on 1 January 2027 is the 2025 text as
amended by AB 1856 (2026), and two of the passages it quoted, the definition of
"developer" and the words "downloaded and launched", were changed. That file is
left as it was, per this folder's rules.

## The question

Does California's Digital Age Assurance Act require HumanityOS (a free,
open-source desktop app downloaded from GitHub Releases and the project's own
website, never from an app store, with no sign-up and no age asked) to ask the
operating system for an age signal, and if so what must it do with the answer,
from when, and on pain of what?

## Short answer

- **Amended before it starts:** AB 1856 (2026) rewrote much of AB 1043; both
  take effect **1 January 2027** (leginfo's code page still shows 2025 text).
- **Only a "developer" must request a signal**, now defined by reference to an
  application "in a covered application store"; anyone not required to ask
  **must not**.
- **Our website is not a covered application store; whether GitHub Releases is
  one, no source answers.** Project's reading: HumanityOS is probably outside
  today, so it should call no OS age interface; a Flathub, Snap, WinGet,
  Homebrew or F-Droid listing (planned) would very likely bring it inside.
- **If inside:** ask once per device at first launch and be "deemed to have
  actual knowledge" of the age range; "under 13" would very likely be COPPA
  actual knowledge too. **Attorney General only**, up to $2,500 or $7,500 per
  affected child. Web chat not covered; no AG guidance or lawsuit found.

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Long passages are trimmed to the words
that matter; nothing inside a quotation is reworded.

- **leginfo.legislature.ca.gov** refused direct requests (HTTP 403), so the
  chaptered texts of AB 1043 and AB 1856, AB 1856's status page, the code
  display of Title 1.81.9 and article IV of the California Constitution were
  read in a browser, and every statutory quotation below was checked by script
  against the rendered page text in that browser.
- **Committee analyses.** leginfo hands these out only as file downloads, so
  they were read from copies: the Assembly Privacy committee's April 2026
  analysis from the committee's own site, and the Senate committee (26 June
  2026), Senate floor (24 August 2026) and Assembly floor (27 August 2026)
  analyses from copies at billtexts.s3.amazonaws.com whose file names carry the
  same analysis numbers as leginfo's own list for AB 1856 (401404, 405003,
  405548). Their text was extracted and the quotations checked by script.
- **Operating-system vendors.** Microsoft Learn, the Windows blog, Google's
  Android developer site and Apple's developer Q&A were read in a browser.
  Apple's framework pages build themselves by script, so they were read through
  Apple's own JSON documentation data
  (`developer.apple.com/tutorials/data/documentation/...`). The systemd and
  xdg-desktop-portal records were read through GitHub's API.
- **Secondary sources** (EFF, GitHub's policy blog as a party's own position)
  are marked where used.

---

## 1. What the act says, section by section

### 1.1 Two statutes, one start date

- **AB 1043**, Chapter 675, Statutes of 2025,
  https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202520260AB1043
  (chaptered text, "Date Published: 10/14/2025"; approved by the Governor 13
  October 2025). It added Title 1.81.9 to the Civil Code, sections 1798.500 to
  1798.505, the "Digital Age Assurance Act". Section 1798.505:
  > This title shall become operative on January 1, 2027.
- **AB 1856**, Chapter 184, Statutes of 2026,
  https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202520260AB1856
  ("Date Published: 09/11/2026"; approved by the Governor 10 September 2026).
  Its title is "An act to amend Sections 1798.500, 1798.501, 1798.502,
  1798.503, and 1798.504 of the Civil Code". It does not touch 1798.505, so the
  operative date stands. The status page,
  https://leginfo.legislature.ca.gov/faces/billStatusClient.xhtml?bill_id=202520260AB1856 ,
  records on 09/10/26 "Chaptered by Secretary of State - Chapter 184, Statutes
  of 2026." and marks the bill "Non-Urgency".
- **When AB 1856 takes effect.** California Constitution, article IV, section
  8(c)(1),
  https://leginfo.legislature.ca.gov/faces/codes_displaySection.xhtml?lawCode=CONS&sectionNum=SEC.%208.&article=IV :
  > a statute enacted at a regular session shall go into effect on January 1 next following a 90-day period from the date of enactment of the statute

  Ninety days after 10 September 2026 is 9 December 2026, so AB 1856 takes
  effect on 1 January 2027, the same day the title becomes operative (the
  project's arithmetic).
- **The code page lags.** Title 1.81.9 as displayed on leginfo,
  https://leginfo.legislature.ca.gov/faces/codes_displayText.xhtml?lawCode=CIV&division=3.&title=1.81.9.&part=4.&chapter=&article= ,
  read 10 October 2026, still shows the 2025 wording under each section with
  the note "(Added by Stats. 2025, Ch. 675, Sec. 1. (AB 1043) Effective January
  1, 2026. Operative January 1, 2027, pursuant to Section 1798.505.)" and no
  mention of AB 1856.
- **What this means for us.** The text that will be in force on 1 January 2027
  is AB 1856's. Anyone quoting the code page today, including this project's
  children's finding of the same date, is quoting wording that will never be in
  force in that form. Everything below quotes AB 1856 unless it says otherwise.

### 1.2 The definitions, as amended (1798.500)

All from AB 1856, section 1.

- **"Account holder"**, 1798.500(a)(1):
  > “Account holder” means an individual who is at least 18 years of age or a parent or legal guardian of a user who is under 18 years of age in the state.

  Paragraph (a)(2) leaves out a parent of an emancipated minor, and a parent or
  guardian "who is not associated with a user’s device".
- **"Age bracket data"**, 1798.500(b): "nonpersonally identifiable data derived
  from a user’s birth date or age", showing at least whether the person is
  under 13, at least 13 and under 16, at least 16 and under 18, or at least 18.
- **"Application"**, 1798.500(c):
  > “Application” means a software application that may be run or directed by a user on a computer, a mobile device, or any other general-purpose computing device that can access a covered application store or download an application.

  and, new in 2026:
  > “Application” does not include software components that are not themselves offered to consumers as a stand-alone executable application through a covered application store.
- **"Child"**, 1798.500(d): a natural person under 18.
- **"Covered application store"**, 1798.500(e)(1):
  > “Covered application store” means a publicly available internet website, software application, online service, or platform that distributes and facilitates the download of applications from third-party developers

  (to users of a computer, a mobile device or other general-purpose device).
  Paragraph (e)(2) leaves out services that distribute "extensions, plug-ins,
  add-ons, or other software applications that run exclusively within a
  separate host application".
- **"Developer"**, 1798.500(f), rewritten in 2026:
  > “Developer” means a person that owns an application, or maintains or controls the hosting of the application, in a covered application store.

  The 2025 text was:
  > “Developer” means a person that owns, maintains, or controls an application.
- **"Operating system provider"**, 1798.500(g): a person or entity that
  "develops, licenses, or controls the operating system software" on a
  computer, phone or other general-purpose device, and, new in 2026:
  > “Operating system provider” does not mean a person or entity that distributes an operating system or application under license terms that permit a recipient to copy, redistribute, and modify the software.
- **"Signal"**, 1798.500(h):
  > “Signal” means age bracket data that pertains to the primary user of a device that is sent by a real-time secure application programming interface or operating system to an application.
- **"User" is gone.** AB 1043's 1798.500(i), "“User” means a child that is the
  primary user of the device.", has no counterpart in AB 1856; the duties now
  speak of "the primary user" of a device.

### 1.3 Operating system providers (1798.501(a) and (b), 1798.502(a))

- 1798.501(a), lead-in:
  > If an operating system operates on a device and has an account setup feature, the operating system provider shall do all of the following:

  (1) an accessible interface at account setup that "requires an account holder
  to indicate the birth date, age, or both, of the primary user of that device",
  for a signal to a covered application store and to a developer; (2) a signal
  to "a developer or covered application store who has requested" one, "via a
  reasonably consistent real-time application programming interface", naming
  at least the four brackets; (3) "Send only the minimum amount of information
  necessary to comply with this title." Subdivision (b): "An operating system
  provider shall not share the digital signal information with a third party
  for a purpose not required by this title."
- 1798.502(a): for devices set up before 1 January 2027, the operating system
  provider must offer the same interface "before July 1, 2027".

### 1.4 Covered application stores (1798.501(c), new in 2026)

> A covered application store shall do both of the following with respect to a user of the covered application store:

> (1) Request a signal from the user’s operating system provider.

> (2) Provide the signal received pursuant to paragraph (1) to a developer upon request.

### 1.5 Developers (1798.501(d))

Request a signal on download and launch; never prompt the person to change
their age; treat the signal as the person's age range and be deemed to know it;
use it only to comply with law. Quoted and explained in section 3.

### 1.6 Everyone: do not ask unless required (1798.501(e), new in 2026)

> A person shall not request a signal with respect to a particular user from an operating system provider or a covered application store if not required to do so by this title or any other applicable law.

The Assembly floor analysis of 27 August 2026 summarises this Senate amendment
as: "Prohibit a person from requesting an age signal if the person is not
required to do so under the Act."

### 1.7 Apps that are already installed (1798.502(b))

> If an application last updated with updates on or after January 1, 2026, was downloaded to a device before January 1, 2027,

and no signal has been requested for that device's user,
> the developer shall request a signal from a covered application store or an operating system provider with respect to that user before July 1, 2027.

### 1.8 Penalties and defences (1798.503)

Quoted in section 4.

### 1.9 Limits (1798.504)

- (b): "This title does not require the collection of additional personal
  information from device owners or device users other than that which is
  necessary to comply with Section 1798.501."
- (d): the title's protections are "in addition to those provided by any other
  applicable law", naming the California Age-Appropriate Design Code Act.
- (f): the title does not apply to broadband internet access, to
  telecommunications services, or to "The delivery or use of a physical
  product."
- (g): "This title does not impose liability that arises from the use of a
  shared device or application by a person who is not the user to whom a
  signal pertains."
- (h), new: "This title does not require a developer that is in compliance with
  this title to request an age signal at every account log in after the initial
  creation of an account."
- (i), new: "This title does not prohibit a developer from periodically
  requesting an age signal to maintain compliance with this title after
  download and launch of an application."
- **No rulemaking.** Neither bill's enacted text contains the word "regulation"
  (searched in the rendered text of both). The title gives the Attorney General
  no power to make rules under it.

---

## 2. Is HumanityOS a "developer", and is GitHub a "covered application store"?

### 2.1 Two ways to read the new definition

The definition (section 1.2) reads: "a person that owns an application, or
maintains or controls the hosting of the application, in a covered application
store."

- **Reading A:** "in a covered application store" governs both halves; the
  commas set off "or maintains or controls the hosting of the application" as
  an aside. A developer is someone whose application is in a covered
  application store.
- **Reading B:** it governs only "hosting"; anyone who owns an application,
  wherever it is offered, is a developer.

What points to A:
- The statute's own commas.
- The Senate Privacy, Digital Technologies and Consumer Protection committee
  analysis (hearing 29 June 2026, analysis dated 26 June 2026, of the 18 May
  version), copy at
  https://billtexts.s3.amazonaws.com/ca/ca-analysishttps-leginfo-legislature-ca-gov-faces-billAnalysisClient-xhtml-bill-id-202520260AB1856-ca-analysis-401404.pdf ,
  describes the operating system's new duty as running to a covered
  application store and to "developers of applications in a covered application
  store".
- The new exclusion in 1798.500(c)(2) is keyed to software offered "through a
  covered application store", and 1798.502(a) still speaks of signals to
  "applications available in a covered application store".

What points to B:
- The Senate floor analysis (24 August 2026), copy at
  https://billtexts.s3.amazonaws.com/ca/ca-analysishttps-leginfo-legislature-ca-gov-faces-billAnalysisClient-xhtml-bill-id-202520260AB1856-ca-analysis-405003.pdf ,
  restates it without the commas:
  > "Developer" means a person who owns an application or maintains or controls the hosting of the application in a covered application store.

  which can be read either way.
- "Application" itself still covers any software that "can access a covered
  application store or download an application", which is almost all software.

**No source settles it.** The project reads it as A, with moderate confidence.
Under B, HumanityOS is a developer whatever happens with GitHub.

### 2.2 The project's own website and its release mirror

A covered application store distributes applications "from third-party
developers" (1798.500(e)(1)). The project's download page
(`web/pages/download.html`) and the release mirror that the build workflow
fills (`/var/www/humanity/releases/<tag>/` on the project's server, per
`.github/workflows/build-desktop.yml`) offer only HumanityOS, the project's
own software. **The project's reading: neither is a covered application
store.** (The download page's buttons in fact point at GitHub Releases, which
brings the question back to GitHub.)

### 2.3 GitHub Releases

- **The words could fit.** GitHub is a publicly available website and
  platform, and through its Releases pages it arguably "distributes and
  facilitates the download of applications from third-party developers": many
  unrelated developers publish programs there for anyone to download. The
  definition asks for no storefront, review, payment or ratings, and its only
  exclusion is for extension and add-on stores.
- **GitHub's own position is that such definitions should not reach it**
  (a party's own statement, not law). GitHub blog, *Why age assurance laws
  matter for developers*, 8 May 2026, updated 12 May 2026,
  https://github.blog/news-insights/policy-news-and-insights/why-age-assurance-laws-matter-for-developers/ :
  it warns that
  > some definitions of “application store” are broad enough to capture developer infrastructure

  and argues
  > Making software available for download is not the same as operating the kind of centralized, consumer-facing marketplace that most people would understand to be an app store.

  GitHub blog, *Developer policy update: Transparency, state policy, and what's
  ahead*, 29 September 2026,
  https://github.blog/news-insights/policy-news-and-insights/developer-policy-update-transparency-state-policy-and-whats-ahead/ :
  > In California, our engagement on the Digital Age Assurance Act (AB 1043) has focused on keeping age assurance requirements from sweeping in open source operating systems, developer tools, and other services that aren’t consumer-facing.

  It does not say that effort succeeded for code hosting, and the enacted text
  has no exclusion for it.
- **A store now has duties of its own.** Since AB 1856 a covered application
  store must request the signal and pass it to developers on request (section
  1.4). No GitHub announcement of any such interface was found (GitHub's blog
  searched 10 October 2026). If GitHub were a covered application store it
  would be out of step with the act on 1 January 2027; that is a hint about how
  GitHub reads the law, not proof of how a court would.
- **What the Legislature described.** The Governor's 2025 signing message, as
  reproduced in the Senate committee analysis above:
  > Parents who allow their children to be the main user of a device will be able to configure the device to inform application developers of the child's age.

  No analysis read mentions code hosting, GitHub or software downloaded outside
  app stores.
- **What this means for us.** Nobody official has said whether GitHub Releases
  is a covered application store. If it is, HumanityOS is "in" one and the
  operator, who owns the application, is a developer under either reading. If
  it is not, then under reading A HumanityOS is not a developer while it is
  offered only through GitHub and its own site.

### 2.4 Being open source does not take an application out

- The open-source exclusion (section 1.2) sits inside the definition of
  "operating system provider" only. HumanityOS is released under CC0 1.0
  (`LICENSE`), which permits copying, redistributing and modifying, but
  HumanityOS is not an operating system provider in the first place, and the
  definitions of "developer" and "covered application store" carry no
  open-source exclusion.
- EFF (secondary), *One Step Forward, Two Steps Back: CA's AB 1856 Exempts
  Open Source But Expands Age-Gating*, published 29 May 2026,
  https://www.eff.org/deeplinks/2026/05/one-step-forward-two-steps-back-cas-ab-1856-exempts-open-source-expands-age-gating ,
  noticed the same gap:
  > given the structure of where the exemption is placed under the “operating system provider” definition, lawmakers could stand to clarify that the exemption applies to open-source operating systems and applications.

  The enacted text was not changed on this point after that.

### 2.5 The listings the project has planned

`docs/admin/distribution-mirrors.md` recommends F-Droid, Flathub, the Snap
Store, WinGet and Homebrew. Flathub and the Snap Store are app stores in every
ordinary sense (F-Droid too, for Android, where HumanityOS has no build). WinGet
and Homebrew are package managers, the kind GitHub's blog says broad
definitions could capture. **The project's reading: a copy of HumanityOS
listed in any of them very likely makes the operator a developer "in a
covered application store" for that copy.** One wrinkle: on Linux, an
open-licensed distribution is no longer an operating system provider, so a
Flathub or Snap copy would have no operating system to ask, and the store's own
duty is to ask "the user’s operating system provider" too.

---

## 3. What a developer must do with a signal, and how that meets COPPA

### 3.1 When to ask, and from whom

- 1798.501(d)(1)(A):
  > A developer shall request a signal with respect to a particular user from an operating system provider or a covered application store when the application is downloaded onto, and launched from, a particular device.

  AB 1043 said "when the application is downloaded and launched"; the 2026
  wording ties the request to a device. 1798.504(h) and (i) (section 1.9): not
  at every log-in, but re-asking from time to time is allowed.
- Apps already installed: before 1 July 2027 (section 1.7).
- 1798.501(d)(1)(B):
  > An entity subject to this title shall not prompt the user to change the user’s age information.

### 3.2 What receiving a signal does

- 1798.501(d)(2)(A): a developer that receives a signal
  > shall be deemed to have actual knowledge of the age range of the user to whom that signal pertains

  "even if the developer willfully disregards the signal", across both of the
  following:
  > (i) Any platform of an application, including an internet website owned, maintained, or controlled by a developer, through which the user may create an account for the platform accessed by that same application.

  and (ii) any "point of access of the application, including an internet
  website" used "to create an account or log into an account from the device"
  that received the signal.
- 1798.501(d)(3): the signal is "the primary indicator of a user’s age range",
  unless the developer has "internal clear and convincing information" that
  the age is different; then the developer
  > shall use that information as the primary indicator of the user’s age and shall be deemed to have actual knowledge of the age range of the user to whom that information pertains.

  "Clear and convincing information" includes age information an account
  holder gives the developer about a user of a sub-account (1798.501(d)(3)(B)(ii)).
  1798.501(d)(2)(B): a developer "shall not willfully disregard" such
  information.
- 1798.501(d)(4): the developer
  > shall use that signal to comply with applicable law but shall not do any of the following:

  "(A) Request more information from an operating system provider or covered
  application store than the minimum amount of information necessary to comply
  with this title." and "(B) Share the signal with a third party for a purpose
  not required by this title."

### 3.3 What the act does not require (negative findings)

- **No treatment of children of its own.** The act says to use the signal "to
  comply with applicable law". It contains no duty to block, filter, change
  features or get a parent's consent; those come, if at all, from other laws
  (1798.504(d)).
- **No age checks.** Nothing in the title mentions identity documents or
  verification; the signal comes from what the account holder entered at
  account setup. 1798.504(b): no collection of "additional personal
  information".
- **No asking at every log-in** (1798.504(h)), and no liability for a shared
  device used by someone else (1798.504(g)).
- **No duty to send the signal anywhere.** The only rule about passing it on is
  the ban on sharing it with a third party for a purpose the title does not
  require.

### 3.4 How the deemed knowledge meets COPPA

- **COPPA's own test.** The children's finding (US-2) explains that a
  general-audience service is covered by COPPA only on "actual knowledge" that
  it collects personal information from a child under 13, and that whether
  COPPA reaches HumanityOS at all ("operated for commercial purposes") is
  unresolved.
- **The FTC on age mechanisms.** *Enforcement Policy Statement Promoting the
  Adoption of Age-Verification Technology*, dated February 25, 2026 on its
  page,
  https://www.ftc.gov/legal-library/browse/enforcement-policy-statement-promoting-adoption-age-verification-technology
  (PDF fetched by a tool, text extracted), footnote 9:
  > In the course of using age-verification mechanisms, general audience sites and services may obtain actual knowledge about the age of a user.

  > if a general audience operator uses an age-verification mechanism that identified a user as 11 years old, that would provide the operator with actual knowledge that the user is a child.

  > Note that general audience sites and services can also obtain actual knowledge through means other than utilizing an age-verification mechanism.

  The statement does not mention operating-system or app-store signals by name;
  its definition of age tools (footnote 7) includes "age inference tools that
  infer a user's likely age or age range based on various signals".
- **The limit on state law.** 15 U.S.C. 6502(d),
  https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap91-sec6502.htm
  (2024 Edition):
  > No State or local government may impose any liability for commercial activities or actions by operators in interstate or foreign commerce

  in connection with COPPA-covered activities
  > that is inconsistent with the treatment of those activities or actions under this section.
- **What this means for us.** The California act creates no COPPA duties; it
  deems knowledge and says to use the signal to comply with applicable law.
  Whether its "deemed" knowledge, including "even if the developer willfully
  disregards the signal", would be carried into COPPA, or found inconsistent
  with it, has not been decided anywhere found. In practice a signal saying
  "under 13" is very likely actual knowledge in fact, on the FTC's own example.
  For an app that today never learns anyone's age, **requesting a signal
  creates knowledge it did not have.** If COPPA applies, that knowledge brings
  the duty to give notice and get verifiable parental consent, or to delete the
  child's information (children's finding, US-2); the no-sign-up design has no
  way to get verifiable parental consent.
- **Where the knowledge would sit.** The signal arrives inside the app on the
  person's own device. Whether "a developer that receives a signal" covers an
  app that reads it locally and sends nothing to the developer is not
  answered. The deeming reaches "an internet website owned, maintained, or
  controlled by a developer" through which the person may create an account,
  which may take in the project's own server and web chat even though they are
  never told. Servers run by other people are third parties, and the signal may
  not be shared with them for a purpose the title does not require.

---

## 4. Penalties, who enforces, and the defences

- 1798.503(a): a violator "shall be subject to an injunction and liable for a
  civil penalty of"
  > not more than two thousand five hundred dollars ($2,500) per affected child for each negligent violation or not more than seven thousand five hundred dollars ($7,500) per affected child for each intentional violation

  > which shall be assessed and recovered only in a civil action brought in the name of the people of the State of California by the Attorney General.
- **No private lawsuits.** "only in a civil action brought ... by the Attorney
  General" leaves no right for an individual to sue under this title.
- **A good-faith defence for developers, new in 2026.** 1798.503(c):
  > A developer that makes a good faith effort to comply with this title, taking into consideration available technology and any reasonable technical limitations or outages, shall not be liable for an erroneous signal indicating a user’s age range

  "or any conduct by an operating system that sent a signal indicating a user’s
  age range." The project's reading: this covers a wrong or missing answer from
  the operating system, not a failure to ask. 1798.503(b) gives operating
  systems and stores the matching defence.
- 1798.504(g): no liability for use of a shared device by someone the signal is
  not about.
- **What this means for us.** The exposure is a suit by the California
  Attorney General, nothing else. What counts as an "affected child" for a
  developer that fails to ask, or for a person who asks without being required
  to (1798.501(e)), is not explained anywhere read.

---

## 5. What the operating systems and the Attorney General have published

### 5.1 Microsoft (Windows)

- *Age signals overview*, Microsoft Learn, "Last updated on 09/24/2026",
  https://learn.microsoft.com/en-us/windows/apps/develop/security/age-signals/ .
  `GetUserAgeRangeAsync` returns one of under 10, 10 to 12, 13 to 15, 16 to 17,
  18 and over (these fit inside California's four brackets), or null:
  > When the result is null, the age range is unknown or unavailable. Apps must use their fallback experience and must not interpret null as a specific age group.

  > The app package must declare the userAccountInformation capability.

  > Identity-provider age signals are currently retrieved only for Microsoft accounts.
- *User.GetUserAgeRangeAsync Method*,
  https://learn.microsoft.com/en-us/uwp/api/windows.system.user.getuseragerangeasync
  (no date shown), device family: "Windows 11, version 24H2 (introduced in
  10.0.26100.0)".
- Windows Experience Blog, 8 September 2026,
  https://blogs.windows.com/windowsexperience/2026/09/08/helping-families-and-educators-support-safer-experiences-and-healthier-habits-on-windows/ :
  > The Windows Age APIs are broadly available to Windows Insiders now and will be available to all Windows users soon.
- Neither page names California or the act.
- **What this means for us.** On Windows 10, on Windows 11 before 24H2, and on
  local (non-Microsoft) accounts, there is no answer to get. HumanityOS ships a
  plain `.exe`, not an app package; whether an unpackaged program can call this
  interface at all was not established ("The app package must declare" suggests
  it needs package identity).

### 5.2 Apple (macOS)

- *Declared Age Range* framework, Apple documentation data,
  https://developer.apple.com/documentation/declaredagerange (no date shown):
  available on iOS, iPadOS, Mac Catalyst and macOS, each from version 26.0.
  Its summary:
  > Create age-appropriate experiences in your app by asking people to share their age range.

  It needs the `com.apple.developer.declared-age-range` entitlement, described
  as "A Boolean value indicating whether your app may request a person’s age
  range."
- *Age assurance frameworks Q&A*, https://developer.apple.com/support/age-assurance/
  (no date shown):
  > In certain regions, where legally required, Apple will share age categories as defined by law.

  The Q&A's version notes speak only of iOS and iPadOS (26, 26.2, 26.4); it does
  not mention macOS or California.
- **What this means for us.** An interface exists on macOS 26 and later. The
  HumanityOS macOS builds are plain binaries with no Apple signing step in
  `.github/workflows/build-desktop.yml`; whether such a build can hold that
  entitlement was not checked.

### 5.3 Google (Android)

- *Play Age Signals overview*, "Last updated 2026-07-20 UTC",
  https://developer.android.com/google/play/age-signals/overview :
  > Use of the Play Age Signals API is limited to apps that are updated by Google Play

  and, in the page's banner, "Ongoing updates will be provided in advance of age
  verification bills in other US states." HumanityOS has no Android build, so
  this does not touch it today. ChromeOS was not looked at.

### 5.4 Linux

- Since AB 1856, an open-licensed distribution is not an "operating system
  provider" (section 1.2), so most Linux distributions have no duty under the
  act.
- Canonical, *Ubuntu's response to California's Digital Age Assurance Act (AB
  1043)*, 4 March 2026 (before that exclusion was added),
  https://discourse.ubuntu.com/t/ubuntus-response-to-californias-digital-age-assurance-act-ab-1043/77948 :
  > Canonical is aware of the legislation and is reviewing it internally with legal counsel, but there are currently no concrete plans on how, or even whether, Ubuntu will change in response.
- systemd pull request 40954, *userdb: add birthDate field to JSON user
  records*, opened 5 March 2026, merged 18 March 2026,
  https://github.com/systemd/systemd/pull/40954 :
  > Stores the user's birth date for age verification, as required by recent laws in California (AB-1043)

  systemd's `docs/USER_RECORD.md` on 10 October 2026 still lists `birthDate` as
  optional.
- xdg-desktop-portal pull request 1922, *Draft: Add parental controls to the
  Accounts portal*, opened 2 March 2026,
  https://github.com/flatpak/xdg-desktop-portal/pull/1922 : closed without
  being merged (last updated 13 April 2026). No other age-related pull request
  was found in that repository on 10 October 2026.
- **What this means for us.** There is no standard way for a Linux program to
  ask for an age bracket, and under the amended act there is usually no
  operating system provider to ask.

### 5.5 The California Attorney General

- **Negative finding.** The site search on oag.ca.gov, run 10 October 2026,
  returned nothing for "Digital Age Assurance" or "AB 1043 age signal"; for
  "age assurance" it returned only pages about the Protecting Our Kids from
  Social Media Addiction Act (SB 976) and its regulations, a different law.
  **No Attorney General regulations or guidance on this act were found**, and
  the act gives no rulemaking power (section 1.9).

---

## 6. Court challenges, amendments, delays

- **Amended: yes, by AB 1856** (section 1.1). The changes that matter here:
  "developer" tied to a covered application store; component software left out
  of "application"; open-licensed operating systems left out of "operating
  system provider"; duties keyed to the device's primary user; operating
  systems bound only if they have account setup; stores must request and relay
  the signal; the request tied to download onto, and launch from, a device; no
  prompting to change age; deemed knowledge across platforms and points of
  access spelled out; clear and convincing information carrying its own deemed
  knowledge; the ban on requesting when not required; the developer good-faith
  defence; and 1798.504(h) and (i).
- **Browser and website duties were added and then removed.** The Assembly
  floor analysis of 27 August 2026, copy at
  https://billtexts.s3.amazonaws.com/ca/ca-analysishttps-leginfo-legislature-ca-gov-faces-billAnalysisClient-xhtml-bill-id-202520260AB1856-ca-analysis-405548.pdf ,
  lists among the Senate amendments:
  > Delete the Act's application to browser providers and website operators.
- **Delayed: no.** 1798.505 was not amended; 1 January 2027 stands, with 1 July
  2027 for devices set up and apps installed before then.
- **Challenged: none found.** A search of the CourtListener federal docket
  archive (RECAP), run 10 October 2026, for "Digital Age Assurance Act" in
  filings after 1 October 2025 returned one docket: *Computer & Communications
  Industry Association v. Paxton*, W.D. Tex. No. 1:25-cv-01660, which concerns
  the Texas app store law (the document mentioning the California act was not
  read). "1798.501" returned nothing. State courts were not searched. EFF
  (secondary), *California Steps Back From Dangerous Expansion of its
  Age-Gating Law*, published 15 July 2026,
  https://www.eff.org/deeplinks/2026/07/california-steps-back-dangerous-expansion-its-age-gating-law :
  > EFF still believes the underlying law that A.B. 1856 amends, A.B. 1043, is unconstitutional.

  It mentions no lawsuit.
- **Other 2026 bills.** A web search found no 2026 bill other than AB 1856 that
  amends Title 1.81.9; this was not checked exhaustively.

---

## 7. What it would mean, concretely, for HumanityOS

### 7.1 As distributed today (GitHub Releases and the project's own site)

If reading A holds and GitHub Releases is not a covered application store,
HumanityOS is not a developer and **the act asks nothing of it, and forbids it
to request a signal** (1798.501(e)). Calling the Windows or Apple age interface
"to be safe" would then itself be the violation. Today the app calls neither
(no use of `GetUserAgeRangeAsync`, Declared Age Range or a `birthDate` field
anywhere in `src/` or `web/`, checked 10 October 2026).

### 7.2 If HumanityOS is a developer (GitHub counts, reading B holds, or a store listing)

| Platform | What the app would call | When there is no answer |
|---|---|---|
| Windows 11, 24H2 and later | `Windows.System.User.GetUserAgeRangeAsync` for the user running the app; needs the `userAccountInformation` capability, and possibly an app package | null on local accounts, older Windows or when switched off; use the ordinary experience |
| Windows 10 | nothing exists | the ordinary experience |
| macOS 26 and later | Apple's Declared Age Range, with its entitlement | the ordinary experience |
| Linux | no standard interface, and usually no "operating system provider" | the ordinary experience, or ask the store it came from if that store offers a signal (none known) |

- **When:** on first launch after install on each device, from 1 January 2027;
  for installs downloaded before then and updated since 1 January 2026, before
  1 July 2027 (in practice, on the first launch of an update shipped in 2027).
  Not at every start; asking again now and then is allowed.
- **With the answer:** treat it as the person's age range; never ask them to
  change it; ask for nothing more than the bracket; do not send it to servers
  run by other people (third parties), and, as the project's own choice, do
  not send it to the project's server either; use it only to comply with
  applicable law. The act itself does not say what to do for a child; COPPA, if
  it applies, does (section 3.4).
- **What it need not do:** ask anyone's birth date, check identity documents,
  collect anything more, ask at every log-in, or change anything on a server.

### 7.3 The web chat

- The act as passed has no duty for websites or browsers (section 6), and a
  web page is not "offered to consumers as a stand-alone executable application
  through a covered application store". The Assembly Privacy committee's
  analysis of AB 1856 (hearing 21 April 2026),
  https://apcp.assembly.ca.gov/system/files/2026-04/ab-1856-wicks-apcp-analysis.pdf ,
  on AB 1043:
  > Later in the process, due to technical and policy complications, the author narrowed the bill to apply only to app developers with the intent to address the issue of websites later.

  and the Senate committee analysis of 26 June 2026 says AB 1043 "did not apply
  to the internet writ large or websites." **The web chat need not request
  anything.**
- One indirect link: if the desktop app ever receives a signal, the deemed
  knowledge reaches the developer's websites through which the person may
  create an account (section 3.2), which would include the web chat.

---

## Where the sources pull in different directions

**The definition of "developer".** The enacted text's commas favour reading A
(an application in a covered application store); the Senate floor analysis
drops them. The Senate committee analysis speaks of "developers of applications
in a covered application store", which supports A. The project weighs the
statute's own punctuation and that committee description above a summary.

**GitHub.** The literal definition of "covered application store" could
include it; GitHub's stated position, the act's described purpose (devices and
app stores parents set up) and the new duties on stores (which GitHub has not
announced it will meet) point the other way. No official source decides it.

**Asking and not asking.** A developer must request a signal
(1798.501(d)(1)(A)); a person not required to must not (1798.501(e)). The cost
of guessing wrong runs in both directions, which is why the scope question
above matters more than it would under the 2025 text.

## What is still unknown, and what would settle it

- **Whether GitHub Releases is a "covered application store".** Settles it: a
  lawyer's opinion; or a short written question to the office of the bill's
  author, Assemblymember Buffy Wicks, or to the Attorney General's office (the
  cost of a letter, and staff views are not binding); or asking GitHub's policy
  team how GitHub reads it and whether it will relay signals (one email).
- **Reading A or reading B of "developer".** Same routes: a lawyer, or the
  author's office.
- **Whether an app that reads a signal on the person's device, and sends it
  nowhere, means the developer "receives" it**, and so whether the deemed
  knowledge reaches the project's server and web chat. A lawyer.
- **Whether the duty is limited to people in California.** "Account holder" is
  defined by reference to "the state", but nothing tells a developer how to
  know where its users are. A lawyer.
- **What "per affected child" means** for a failure to ask, or for asking when
  not required. A lawyer.
- **Whether an unpackaged Windows program can call `GetUserAgeRangeAsync`.**
  Settles it: a one-hour test on a Windows 11 24H2 machine with a Microsoft
  account, or Microsoft's packaging documentation for programs with package
  identity.
- **Whether a macOS build not signed through Apple's developer programme can
  hold the Declared Age Range entitlement, and whether Apple will send
  California age categories on macOS.** Settles it: Apple developer support or
  a test build (needs a paid Apple developer membership).
- **Whether COPPA treats a received signal as actual knowledge, and whether
  California's deeming is consistent with COPPA's preemption clause.** No case
  found. The FTC's COPPA hotline (see the children's finding) or a lawyer.
- **State-court challenges** (not searched), and **any 2026 bill other than AB
  1856** touching the title (not searched exhaustively). Settles it: a search
  of California court records and of 2026 chaptered bills for "1798.50", before
  the end of 2026.
- **Whether a CC0 project has an "owner"** for the definition of developer.
  Probably not worth relying on either way; noted only because the definition
  turns on ownership.

## Sources that could not be reached

- leginfo.legislature.ca.gov returned 403 to direct requests; its pages were
  read in a browser.
- leginfo's bill analyses (for example
  https://leginfo.legislature.ca.gov/faces/billAnalysisClient.xhtml?bill_id=202520260AB1856 )
  are served only as downloads through a form; opening one in the browser
  aborted. They were read from the copies named above. Wanted from the
  originals: confirmation that the copies are unaltered.
- The Governor's AB 1043 signing message on gov.ca.gov was not looked up; it is
  quoted as the Senate committee analysis reproduces it.
- Apple's documentation pages render by script; they were read through Apple's
  JSON documentation data instead.
- The FTC's policy statement PDF was fetched by a tool and its text extracted,
  so its quotations were checked by script.
- No Microsoft page was found saying whether unpackaged desktop programs can
  use the age interfaces.
- State court records were not searched.

## What it means for HumanityOS

### Certain (stated by the sources above)

1. The act takes effect on 1 January 2027 in its AB 1856 form; existing devices
   and installs have until 1 July 2027.
2. Only a "developer", as now defined, must request a signal, and only when the
   application is downloaded onto and launched from a device; anyone not
   required to request a signal must not request one.
3. The act requires no age checks, no birth dates from the app, no extra
   personal information, no requests at every log-in and no particular
   treatment of children; it points to "applicable law" for that.
4. A developer that receives a signal is deemed to know the person's age range,
   on its websites and other points of access too, and must not share the
   signal with third parties for purposes the title does not require.
5. Enforcement is by the Attorney General only: an injunction and up to $2,500
   (negligent) or $7,500 (intentional) per affected child, with a good-faith
   defence for developers against erroneous signals.
6. The enacted act has no duties for browsers or websites.
7. Windows (11, 24H2 and later) and macOS (26 and later) have age interfaces;
   Linux has none, and open-licensed distributions are not operating system
   providers. No Attorney General guidance and no court challenge were found.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **As distributed today, HumanityOS is probably not a "developer", so do not
   add any operating-system age call.** Reading A, plus GitHub Releases not
   being an app store in the sense the Legislature described. This is the
   main uncertainty, and asking without being required is itself forbidden, so
   "ask just in case" is not the safe default it looks like.
2. **Keep the protected setup free of operating-system age calls too.** Using
   the Windows or Apple age interface to switch the setup on automatically
   would be a request for a signal; while the project is not required to make
   one, 1798.501(e) forbids it. The setup stays parent-applied with a PIN, as
   designed (`docs/design/blocking-and-safe-mode.md` section 6.4 and 10h).
3. **Decide before listing on any store.** Before HumanityOS goes into Flathub,
   the Snap Store, WinGet, Homebrew or F-Droid (`docs/admin/distribution-mirrors.md`),
   the operator should decide whether to build the signal path first (section
   7.2) or hold the listing. This is the operator's decision; the listing,
   not the code, is what most likely changes the answer.
4. **If HumanityOS ever must ask, keep the answer on the device.** Never send it
   to any server, never prompt the person to change it, and decide in advance,
   with the operator, what the app does for an "under 13" answer. COPPA's
   answer, if it applies, is consent or deletion, and the no-sign-up design
   cannot do consent; the realistic choices are offline-only use or not serving
   that person. That is a direction decision for the operator, not one to make
   in code.
5. **Settle the GitHub question cheaply before 1 January 2027.** One email to
   GitHub's policy team and one letter to the author's office would turn the
   biggest unknown here into a stated position, at little cost.
6. **Re-check this finding** before any store listing, before 1 January 2027,
   and after the 2027 legislative session, since the act was amended once
   before it even started.

### Observed in the code on 10 October 2026 (relevant to the judgement calls)

Read, not changed, while writing this; a reviewer should confirm.

- `web/pages/download.html` links its downloads to
  `https://github.com/Shaostoul/Humanity/releases`, and `src/updater.rs`
  checks `https://api.github.com/repos/Shaostoul/Humanity/releases`.
- `.github/workflows/build-desktop.yml` builds a Windows `.exe`, a Linux binary
  and two macOS binaries, publishes them to the GitHub release, and copies them
  to `/var/www/humanity/releases/<tag>/` on the project's server. No code
  signing, notarisation or app packaging step was found.
- `LICENSE` is CC0 1.0 Universal.
- No operating-system age interface is called anywhere in `src/` or `web/`.
- `web/chat/index.html` (line 116) still says "By entering, you confirm you are
  18 years or older"; the children's finding asks the operator to settle the
  age line before the protected setup ships.

---

This document is a reading of public sources on 10 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. The act was amended a month
before this was written and takes effect on 1 January 2027; check the dates
above, and ask a lawyer before relying on any of it for a decision with legal
consequences.
