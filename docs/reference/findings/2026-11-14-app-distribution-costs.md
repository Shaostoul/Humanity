# Running HumanityOS on every device: what each platform holder charges, requires and shows

**Researched 14 November 2026** (sources read between about 03:50 and 04:45 Pacific time). Every
quotation below was copied from the page as downloaded that day and checked by script against the
downloaded text. Fees, store rules and Android's developer verification are all moving: several of
the pages below were revised in the last month. If you are reading this later, check each source's
own date before relying on it.

**This is a reading of public sources by an AI working on the project. It is not legal or financial
advice, and nobody who wrote it is a lawyer.** Where it says what the project should do, that is the
project's own reading, marked as a judgement call. Prices are recorded as facts, not as
recommendations to buy anything.

**Scope note.** "Signing" here means each operating system's own trust label for installers
(Windows' Authenticode certificates and SmartScreen reputation, Apple's Developer ID and
notarization, Android's developer verification). It is a different thing from the project's own
release signature that the desktop updater checks
([`docs/design/signing-tiers.md`](../../design/signing-tiers.md)), which this finding does not cover.

The plan built on this finding is [`docs/design/platforms.md`](../../design/platforms.md).

## The question

What does each platform holder (Microsoft, Apple, Google, Flathub, and the browser makers for the web
app) charge and require before a free, open-source program can be installed on its devices, and what
does a person see when they install it without paying?

## Short answer

- **Windows.** An unsigned download shows "Windows protected your PC" and needs "Run anyway". On
  Windows 11, Smart App Control blocks unsigned apps it cannot vouch for, with "currently no way to
  bypass" it for one app. Microsoft now says EV certificates "no longer impact SmartScreen behavior"
  (fragment), and any signed file still warns until reputation builds. Routes:
  - **Microsoft Store: free.** Individual registration has "no registration fees" since the new
    onboarding (ID and selfie required), and an MSIX package gets "Free Microsoft code signing and
    CDN hosting". Store apps "are never subject to SmartScreen download warnings". An EXE or MSI in
    the Store must carry the developer's own certificate.
  - **SignPath Foundation: free for open source, but HumanityOS does not qualify today.** Its rule
    is "an OSI-approved Open Source license"; the project is CC0, and the OSI says CC0 was never
    approved. Plan B of the pending licence vote (AGPL-3.0 code) would meet that rule.
  - **Paid, for the record:** Microsoft's Artifact Signing from 9.99 USD a month (individuals only
    in the US or Canada, with name and city on the certificate); Certum's open-source certificate set
    at 69.00 EUR (a smart card and reader); SSL.com OV at 129.00 USD a year plus key storage. Since
    1 March 2026 no code signing certificate may last more than 460 days.
- **macOS.** Notarization needs the Apple Developer Program, 99 USD a year; free accounts cannot
  notarize. On macOS Sequoia and later a person can no longer Control-click past Gatekeeper; they
  allow an unnotarized app in System Settings > Privacy & Security. Apple's fee waiver is only for
  nonprofit, educational or government legal entities, "Not be an individual".
- **iPhone and iPad.** No route puts the app on other people's devices without the paid program.
  A free Apple Account can install your own build on up to 3 of your own devices through Xcode, and
  the install expires after 7 days. The EU's alternative marketplaces and Web Distribution need
  notarization through App Store Connect (paid), Web Distribution also needs one of seven
  eligibility criteria, and both are for EU users only. Wallets for virtual currency may only be
  offered by "developers enrolled as an organization", and push notifications are a paid-membership
  capability.
- **Do you need a Mac to build for iPhone?** Not to build and test: GitHub's standard macOS runners
  (Apple M1, an Xcode label) are free for public repositories, and the iOS Simulator runs there.
  Signing for a real device and distributing need Apple's certificates and provisioning profiles,
  which come from the paid program (or, free, from Xcode on a Mac signed in to your Apple Account,
  with the 7-day limit).
- **Android.** A direct APK download works today: the person allows "Install unknown apps" for the
  browser or file manager it came from. Google's developer verification began on 30 September 2026
  only for apps from "participating stores" in Brazil, Indonesia, Singapore and Thailand; sideloaded
  apps are not affected "yet", and Google says it expands "globally for all apps on certified
  Android devices in 2027". After that, an unregistered app installs only through a one-time
  "advanced flow" (with a 24-hour wait) or ADB. Registration is free for a Limited Distribution
  account (50 devices in one Google page, 20 in another) and 25 USD for full distribution. Google
  Play is 25 USD once, needs ID, and personal accounts must run a 14-day closed test with 12 testers
  first. F-Droid's own pages could not be read: its robots.txt bars Claude's crawlers.
- **Linux.** Flathub charges no fee that its pages mention, but builds everything from source with
  no network, wants "a meaningful history of development", and has a Generative AI policy: AI
  material must be disclosed, may be rejected "based on the extent or role of generated material",
  manifests "must not contain AI-generated or AI-assisted content", and AI agents must not open
  submission pull requests. HumanityOS is largely written with AI, so a Flathub listing is
  uncertain. A plain download of the Linux build needs nobody's permission.
- **The web app (PWA).** Free on every device with a browser. The Microsoft Store takes PWAs; Google
  Play takes them wrapped as a Trusted Web Activity (25 USD account); Apple's App Store rejects an
  app that is not "beyond a repackaged website".
- **Direct downloads are a thing on Windows, macOS, Linux and Android**, each with its own warning
  above. **They are not a thing on iPhone and iPad** outside the EU's paid, notarized routes.

## How each finding is recorded

Each finding gives the source's address, the source's own date (or the server's Last-Modified
header where the page shows none), and a quotation of the words that carry the meaning. "(fragment)"
marks a quotation cut from a longer sentence; nothing inside a quotation is reworded. Every page was
fetched with a plain script user agent after reading its site's robots.txt in a separate step.

## 1. Windows

### 1.1 How SmartScreen decides, and what an unsigned download shows

Source: "SmartScreen reputation for Windows app developers", Microsoft Learn,
https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation (ms.date
2026-05-04, updated 2026-08-17). An earlier finding read the same page
([`2026-10-19-tools-page-install-launch-and-icons.md`](2026-10-19-tools-page-install-launch-and-icons.md)).

- The two signals: "Publisher reputation — Is the file signed? Is the signing certificate from a
  known, trusted publisher?" and "File hash reputation — Has this specific file been downloaded by
  users without indications of malicious behavior?"
- Signed is not the same as trusted: "Even when signed, a newly created binary could still show a
  SmartScreen warning until its hash or publisher certificate accumulates sufficient evidence of
  positive reputation."
- Unsigned starts over every release: "When a file is not signed, SmartScreen reputation must build
  for each new version of your files, starting with zero reputation."
- The page's table, row by row: Microsoft Store, "✅ No warning — covered by Microsoft's
  certificate"; Valid Certificate (OV/EV), "⚠️ Warning — app flagged as unrecognized until
  reputation accumulates; verified publisher name is displayed"; No signature, "⚠️ Warning —
  "Windows protected your PC"; User must choose "Run anyway" before the app can run. Enterprise
  policy can prevent continuation entirely."; Self-signed Certificate, "⚠️ Warning — Same behavior
  as no signature".
- **Negative finding, EV:** "EV certificates no longer bypass SmartScreen." And: "Paying a premium
  for EV solely to avoid SmartScreen warnings is no longer justified."
- How long reputation takes: "There is no exact threshold, but it can take several weeks and
  hundreds of clean installs from a wide audience." And: "There is no need (or mechanism) to
  manually submit a file for SmartScreen reputation review for consumer endpoints."
- The Store: "Store-distributed apps are signed by a Microsoft certificate and are never subject to
  SmartScreen download warnings."

### 1.2 Smart App Control on Windows 11

Source: "Smart App Control Frequently Asked Questions", Microsoft Support,
https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003
(redirected to
https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions;
ms.date 08/17/2026). The SmartScreen page above adds: "Smart App Control will block execution of
unsigned files unless the file has a positive reputation."

- "If the app is unsigned, or the signature is invalid, Smart App Control will consider it untrusted
  and block it for your protection."
- **Negative finding:** "There is currently no way to bypass Smart App Control protection for
  individual apps. You can turn Smart App Control off, or (better yet), contact the developer of the
  app and encourage them to sign their app with a valid signature."
- To developers: "The simple answer is, sign your app with a valid certificate."

### 1.3 The industry rules every certificate follows

Source: CA/Browser Forum, "Baseline Requirements for the Issuance and Management of Publicly-Trusted
Code Signing Certificates", https://cabforum.org/working-groups/code-signing/requirements/ ("Version:
3.11.0 Date: June 16, 2026").

- Hardware keys since 2023: "Effective June 1, 2023, for Code Signing Certificates, CAs SHALL ensure
  that the Subscriber’s Private Key is generated, stored, and used in a suitable Hardware Crypto
  Module" (fragment). This is why every option below is a smart card, a USB token, a cloud HSM or a
  signing service, never a file on disk.
- Shorter certificates since March 2026: "For Code Signing Certificates issued on or after March
  1st, 2026, the validity period MUST NOT exceed 460 days." So any certificate is renewed (and paid
  for, where it is paid) about once a year.

### 1.4 Microsoft's own signing service (Artifact Signing, formerly Trusted Signing)

- The SmartScreen page (1.1): "Artifact Signing (formerly Trusted Signing) is Microsoft's recommended
  code signing service for non-Store distribution" and "Cost — Starts at $9.99/month." It also says
  "No hardware token required — integrates directly with CI/CD pipelines (GitHub Actions, Azure
  DevOps)".
- Price. Source: https://azure.microsoft.com/en-us/pricing/details/artifact-signing/ (no date shown;
  read 2026-11-14). The table's prices are filled in by the page's script; the page's own data holds
  9.99 (Basic) and 99.99 (Premium) a month, and 0.005 a signature over the quota. Visible text:
  "Quota (signatures/month)", "5,000" (Basic) and "100,000" (Premium).
- Who may use it. Source: "Quickstart: Set up Artifact Signing", Microsoft Learn,
  https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart (ms.date 2026-05-21, updated
  2026-10-08): "Individual developers must be located in the United States or Canada."
- What the certificate shows about an individual: "The city, state/province, and country/region from
  the address entered here's displayed on the certificate." Identity checks need a government ID,
  and "The legal name and sold-to address on the billing account must match the information on the
  government-issued ID used for identity validation."

### 1.5 Commercial certificates (two examples of the market)

- **Certum, Open Source Code Signing set.** Source: https://shop.certum.eu/open-source-code-signing.html
  (no date shown; read 2026-11-14). "The set includes a Code Signing Certificate, a card reader, and
  a Certum cryptographic card." Price shown: "€69.00". The name in it: "Data in the certificate:
  natural person data prefixed with “Open Source Developer” phrase". Certum's own claims include
  "Building a Microsoft SmartScreen Filter reputation." (Microsoft's page in 1.1 is the authority on
  what SmartScreen does; see "Where the sources pull in different directions".) The key lives on a
  physical card in a reader, so signing happens on the computer the card is plugged into.
- **SSL.com, OV Code Signing.** Source: https://www.ssl.com/certificates/code-signing/buy/
  (redirected to https://www.ssl.com/products/software-integrity/code-signing/ov/; no date shown).
  "1 Year" at "$129.00/yr", down to "$96.75/yr" for five years. Key storage is extra: "YubiKey",
  "Physical Token", "$379 each", or "eSigner Cloud Signing" as a "+ Subscription". The page's heading
  says "Put your organization's name on every binary you distribute."

### 1.6 SignPath Foundation: free code signing for open source

Source: https://signpath.org/ and https://signpath.org/terms (no dates shown; server Last-Modified
2026-09-09). signpath.org has no robots.txt (404 on the day).

- The offer: "Free Code Signing for Open Source software" and "SignPath Foundation provides you with
  a code signing certificate."
- Who signs: "The code signing certificate is issued to SignPath Foundation . This means that SignPath
  Foundation is the publisher of the OSS project." So the publisher name Windows shows would be
  SignPath Foundation, not the project or the operator.
- **The licence condition:** "OSS License: The project must use an OSI-approved Open Source license
  without commercial dual-licensing for all components."
- Other conditions, quoted: "No proprietary code: The project may not contain any proprietary, non
  open-source component" (fragment); "Released: The project must already be released in the form that
  should be signed."; "All team members must use multi-factor authentication for both SignPath and
  source code repository access (e.g. GitHub)."; "Reviewers : each change proposed by people who are
  not committers (e.g. pull requests) must be reviewed by a team member."; "Every release needs manual
  approval for signing."; "Binary artifacts must be built from source code in a verifiable way."
- On data: "Software that collects user data and transfers it to systems not specified by the user
  must a) describe this behavior in a privacy policy, b) display this policy during installation and
  c) include installation options to disable these functions."
- A public page is required: "A code signing policy must be specified on the project’s home page."
- Its own status: "The code of conduct is still in a draft version."

**CC0 and the OSI.** Source: Open Source Initiative FAQ, https://opensource.org/faq (modified
2025-01-21). "At this time, we do not recommend releasing software using the the CC0 public domain
dedication." And: "CC0 was not explicitly rejected, but the License Review Committee was unable to
reach consensus that it should be approved, and Creative Commons eventually withdrew the
application." The project's `LICENSE` is CC0 1.0 today. The licence options and the vote are in
[`2026-10-25-licence-options.md`](2026-10-25-licence-options.md); plan B there is AGPL-3.0 for code.

### 1.7 The Microsoft Store

- Registration. Source: "Steps to open a Microsoft Store developer account in Partner Center",
  Microsoft Learn,
  https://learn.microsoft.com/en-us/windows/apps/publish/partner-center/open-a-developer-account
  (ms.date 2026-10-29): "With the new onboarding experience, there are no registration fees for
  either account type, so you can create your developer account and start publishing at no cost."
  The individual flow: "Begin identity verification with a government-issued ID and selfie." And:
  "Choose an Individual account if you want to publish apps under your own name". The free flow
  starts only at storedeveloper.microsoft.com: "This is the only supported entry point for the new
  flow."
- Packages. Source: "App package requirements for MSI/EXE app", Microsoft Learn,
  https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msi/app-package-requirements
  (ms.date 2022-10-30, updated 2026-08-24). For MSIX: "Free Microsoft code signing and CDN hosting."
  For an EXE or MSI: "The binary and all of its Portable Executable (PE) files must be digitally
  signed with a code signing certificate that chains up to a certificate issued by a Certificate
  Authority (CA) that is part of the Microsoft Trusted Root Program ." Also: "The installer is a
  standalone installer and is not a downloader stub/web installer that downloads bits when run."
- **Negative finding:** an EXE or MSI Store listing does not avoid buying a certificate; only the
  MSIX route does.

### 1.8 Web apps in the Microsoft Store

Source: "Publish a PWA to the Microsoft Store", Microsoft Learn,
https://learn.microsoft.com/en-us/microsoft-edge/progressive-web-apps/how-to/microsoft-store
(updated 2026-09-02): "Publishing your Progressive Web App (PWA) to the Microsoft Store brings the
following advantages:" (fragment).

## 2. macOS

### 2.1 The Apple Developer Program and its fee waiver

- Source: https://developer.apple.com/programs/whats-included/ (server Last-Modified 2026-09-17):
  "The Apple Developer Program is 99 USD per membership year, or in local currency where available."
- Source: "Apple Developer Program Fee Waiver", https://developer.apple.com/help/account/membership/fee-waivers/ (server
  Last-Modified 2026-11-10). To be eligible you must "Be a legal entity with a status as a nonprofit
  organization 1 , accredited educational institution 2 , or government entity;" and "Not be an
  individual, sole proprietor, or single-person business;" and "Not otherwise sell digital goods or
  services through any of your apps." The project has no legal entity yet
  ([`2026-10-12-project-entity-and-succession.md`](2026-10-12-project-entity-and-succession.md)).

### 2.2 Notarization, and who can do it

- Source: "Notarizing macOS software before distribution", Apple Developer Documentation,
  https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
  (read through the page's own data file at
  https://developer.apple.com/tutorials/data/documentation/security/notarizing-macos-software-before-distribution.json;
  no date shown): "Beginning in macOS 10.15, all software built after June 1, 2019, and distributed
  with Developer ID must be notarized."
- Source: "Signing Mac Software with Developer ID", https://developer.apple.com/developer-id/ (server
  Last-Modified 2026-01-30): "Please note that you must be the Account Holder of your development team
  in the Apple Developer Program in order to generate this cerificate." (Apple's spelling.)
- Source: "Developer account overview",
  https://developer.apple.com/help/account/basics/about-your-developer-account (reached from
  /support/compare-memberships/; server Last-Modified 2026-11-10). In its table, the "Registered for
  free" column has no mark for "Mac software notarization", "Certificates, Identifiers & Profiles",
  "App Store Connect" or "TestFlight"; the "Apple Developer Program member" column has all four.

### 2.3 What a person sees, and how they allow it

Source: "Updates to runtime protection in macOS Sequoia", Apple Developer News,
https://developer.apple.com/news/?id=saqachfa ("August 6, 2024"): "In macOS Sequoia, users will no
longer be able to Control-click to override Gatekeeper when opening software that isn’t signed
correctly or notarized. They’ll need to visit System Settings > Privacy & Security to review security
information for software before allowing it to run."

Apple's user-facing page on opening apps could not be read (section "Sources that could not be
reached"), so the exact wording of the dialog a person sees is not quoted here.

## 3. iPhone and iPad

### 3.1 Free: your own devices only, for 7 days

Source: "Developer account overview" (2.2): "To install and test your apps on a personal device,
you’ll need to sign in to your Apple Account in Xcode." Then:

- "You can register up to 10 App IDs, which expire after 7 days."
- "You can register up to 3 devices, which expire after 7 days."
- "You can install up to 3 apps per device. Provisioning profiles that enable apps to be installed on
  a device will expire 7 days from issuance. You’ll need to rebuild and reinstall your app to your
  device after expiration."

### 3.2 The EU's other routes (Digital Markets Act)

Source: "Changes for apps in the European Union", https://developer.apple.com/support/dma-and-apps-in-the-eu/
(redirected to https://developer.apple.com/support/apps-in-the-eu; server Last-Modified 2026-10-23).

- "In addition to the App Store, users in the EU can install notarized iOS and iPadOS apps from
  alternative app marketplaces or directly from an authorized developer’s website."
- Notarization goes through Apple's own console: "To get your app notarized, select the alternative
  distribution option in App Store Connect when you submit it for review." App Store Connect is a
  paid-membership feature (2.2).
- Web Distribution needs Apple's authorization and at least one of: "Meet a moderate
  financial-stability bar as scored by Dun & Bradstreet;" "Are publicly traded, or owned by a
  publicly traded company;" "Have received venture funding from an established investment firm;"
  "Have completed a financial audit by a licensed accountant;" "Are a government entity, educational
  institution, or nonprofit approved for a fee waiver;" "Provide a stand-by letter of credit of USD
  1,000,000 (or equivalent in local currency); or" "Have one million first annual installs
  worldwide."
- Apple's commission applies to sales: the routes are "subject to a commission on the sales of paid
  apps and digital goods or services" (fragment). A free app with no sales sells nothing to
  commission.
- **Negative finding:** every route on this page is for users in the EU. None reaches a person in
  the United States.

### 3.3 App Review Guidelines that touch HumanityOS

Source: https://developer.apple.com/app-store/review/guidelines/ ("Last Updated: October 16, 2026").

- 2.5.2: apps "may not read or write data outside the designated container area, nor may they
  download, install, or execute code which introduces or changes features or functionality of the
  app, including other apps" (fragment).
- 3.1.5: "(i) Wallets: Apps may facilitate virtual currency storage, provided they are offered by
  developers enrolled as an organization."
- 4.2: "Your app should include features, content, and UI that elevate it beyond a repackaged
  website."

### 3.4 Push notifications

Source: "Supported capabilities (iOS)", https://developer.apple.com/help/account/reference/supported-capabilities-ios/
(server Last-Modified 2026-11-10). The table marks "Push notifications" for "ADP" and "ADEP" but not
for "Apple Developer", which the legend defines as: "Apple Account holders who have agreed to the
Apple Developer Agreement to access certain resources on the Apple Developer website. No cost is
associated with this agreement and developers can’t distribute apps."

### 3.5 Building for iPhone without owning a Mac

- Source: "GitHub Actions billing", https://docs.github.com/en/billing/concepts/product-billing/github-actions
  (no date shown): "The use of standard GitHub-hosted runners is free:" then "In public repositories".
- Source: "GitHub-hosted runners reference", https://docs.github.com/en/actions/reference/runners/github-hosted-runners
  (no date shown). The table "Standard GitHub-hosted runners for public repositories" lists a macOS
  runner with "3 (M1)" processors, "7 GB" of memory, "arm64", and the labels "macos-latest ,",
  "macos-15 ,", "macos-26 ," and "xcode-27". Also: "During periods of high demand, jobs that run on
  macOS runners may take longer to start." For Android: "GitHub-hosted Linux runners support hardware
  acceleration for Android SDK tools" (fragment).
- Source: "Installing an Apple certificate on macOS runners for Xcode development", https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications
  (no date shown): "The signing process involves storing certificates and provisioning profiles,
  transferring them to the runner, importing them to the runner's keychain, and using them in your
  build."
- The project already builds macOS binaries on these runners
  (`.github/workflows/build-desktop.yml`, `macos-latest`, `aarch64-apple-darwin` and
  `x86_64-apple-darwin`; recent runs succeeded on 2026-11-11 to 2026-11-14).

## 4. Android

### 4.1 A plain download from a website

Source: "Alternative distribution options", Android Developers,
https://developer.android.com/distribute/marketing-tools/alternative-distribution ("Last updated
2026-02-26 UTC"): "To install an app distributed in this way, users must opt-in for installing
unknown apps ." And: "On devices running Android 8.0 (API level 26) and higher, users must navigate
to the Install unknown apps system settings screen to enable app installations from a particular
location".

### 4.2 Android developer verification

Sources: https://developer.android.com/developer-verification (server Last-Modified 2026-10-23), its
FAQ https://developer.android.com/developer-verification/guides/faq ("Last updated 2026-09-30 UTC",
each answer dated), the limited distribution guide
https://developer.android.com/developer-verification/guides/limited-distribution ("Last updated
2026-10-28 UTC") and the open-source guide
https://developer.android.com/developer-verification/guides/open-source-app-registration ("Last
updated 2026-10-23 UTC").

- Timeline: "September 30, 2026", "Protections begin for all users who install apps from
  participating stores in Brazil, Indonesia, Singapore, and Thailand on certified devices running
  Android 7+." Then "2027 and beyond", "Verification capability will be expanded globally for all
  apps on certified Android devices in 2027."
- Sideloading today (FAQ, updated July 15, 2026): "If you distribute your app through other stores,
  or if users sideload your app directly, these new verification requirements won't apply to your
  app yet."
- After the rollout (FAQ, March 23, 2026): "Android is introducing an advanced flow for power users
  who want to take educated risks to install apps from unverified developers. It's a one-time setup
  for users to install apps from unverified developers after acknowledging the risks." The wait (May
  11, 2026): "This 24-hour pause breaks that sense of urgency and gives users time to verify the
  request independently." ADB (March 23, 2026): "No, there are no changes to how ADB works. You will
  be able to install applications using ADB as usual. The waiting period does not apply to ADB
  installs."
- Fees (FAQ, March 25, 2026): "The $25 fee for the Full Distribution account in the ADC helps cover
  administrative costs and investment in protecting the ecosystem, similar to Play's $25 registration
  fee." And: "We are waiving the fee for developers who qualify for a Limited Distribution account."
- Limited distribution: "Share apps with up to 50 devices that end-users have explicitly authorized."
  "This account is free." "There are no additional verification requirements for limited
  distribution accounts. For example, you don't need to provide a government ID ."
- Open-source repositories: "While you don't have to register your app in order to continue
  distributing them on open-source repositories, doing so will let your users continue with the
  same" (fragment, the sentence continues past the line break on the page).

### 4.3 Google Play

- Fee. Source: https://support.google.com/googleplay/android-developer/answer/6112435 (no date shown):
  "There is a US$25 one-time registration fee." And: "To process your request for a Play Developer
  account you may be asked for a valid government ID and a credit card, both under your legal name."
- Testing first. Source: https://support.google.com/googleplay/android-developer/answer/14151465 (no
  date shown): "Developers with personal accounts created after November 13, 2023, must run a closed
  test for their app with a minimum of 12 testers who have been opted in continuously for at least 14
  days."
- No self-updating, no downloaded native code. Source: "Device and Network Abuse",
  https://support.google.com/googleplay/android-developer/answer/9888379 (no date shown): "An app
  distributed via Google Play may not modify, replace, or update itself using any method other than
  Google Play's update mechanism. Likewise, an app may not download executable code (such as dex,
  JAR, .so files) from a source other than Google Play."

### 4.4 F-Droid

Not read. f-droid.org's robots.txt (read 2026-11-14) has groups for `anthropic-ai`, `claude-web` and
`ClaudeBot`, each with `Disallow: /`. Under this project's research rules its Inclusion Policy and its
statements on developer verification were not read, and no copy hosted elsewhere was read either.
Nothing in this finding says what F-Droid requires or costs.

### 4.5 Web apps in Google Play

Source: the Trusted Web Activity overview, Chrome for Developers,
https://developer.chrome.com/docs/android/trusted-web-activity/ ("Last updated 2020-02-04 UTC"):
"Trusted Web Activity is a new way to open your web-app content such as your Progressive Web App
(PWA) from your Android app using a protocol based on Custom Tabs."

## 5. Linux

### 5.1 Flathub

Source: "Requirements", Flathub documentation, https://docs.flathub.org/docs/for-app-authors/requirements
(server Last-Modified 2026-11-11). The submission page,
https://docs.flathub.org/docs/for-app-authors/submission, was also read; neither page mentions a fee.

- "There is no network access during the build process."
- "All source available submissions must be built entirely from source code."
- "Submissions must demonstrate a meaningful history of development or existence, evidence of
  real-world use and a clear commitment to ongoing maintenance."
- **The Generative AI policy:** "Submitters must disclose any AI-generated code, documentation,
  packaging, or other material they know or reasonably believe is included in the application or its
  Flathub packaging." "Flathub manifests must not contain AI-generated or AI-assisted content."
  "Reviewers may reject a submission, including without further review, based on the extent or role
  of generated material or concerns about its review, quality, or maintainability." "AI tools or
  agents must not open or automate Flathub submission pull requests, or generate their commit
  messages, descriptions, review comments, or replies."
- Flathub on licences in its own packaging repository: "restrictive licenses must not be used and the
  general recommendation is to use permissive licenses such as MIT, 0BSD, or the Unlicense"
  (fragment). That is about the packaging files, not the app.

### 5.2 Distributions' own packaging

Not researched today (see "What is still unknown").

## 6. The operator's phone

Source: OnePlus 9 Pro specifications, https://www.oneplus.com/us/9-pro/specs (server Last-Modified
2025-01-17): "Operating System: OxygenOS based on Android™ 11", "CPU: Qualcomm® Snapdragon™ 888",
"GPU: Adreno 660", "RAM: 12GB LPDDR5". (Android 11 is what it shipped with; the version it runs
now is on the phone's About page.)

## Where the sources pull in different directions

- **Android's device limit for a free Limited Distribution account.** The overview page says "share
  apps with up to 20 devices" (fragment, "Learn how students, teachers, and hobbyists can share apps
  with up to 20 devices without a government-issued ID or registration fee."); the limited
  distribution guide, updated 2026-10-28, says "up to 50 devices". Judgement: the guide is newer and
  more specific, so 50 is more likely current; either is enough for the operator's own phone.
- **What a certificate does for SmartScreen.** Certum's shop lists "Elimination of the "Unknown
  publisher" message" and "Building a Microsoft SmartScreen Filter reputation."; Microsoft says a
  signed file still shows a warning "until reputation accumulates" (fragment). These do not contradict
  each other (signing names the publisher and lets reputation carry from one release to the next), but
  a certificate alone does not make the warning go away on day one. Microsoft is the authority on its
  own filter.

## What is still unknown, and what would settle it

- **F-Droid's inclusion rules and costs.** Settled by the operator (or a person) reading
  https://f-droid.org/docs/Inclusion_Policy/ and F-Droid's statements on developer verification. Cost:
  half an hour of reading.
- **The exact Gatekeeper dialog wording on current macOS.** Settled by reading Apple's support page
  (https://support.apple.com/en-us/102445) in a browser. Free.
- **Whether a Limited Distribution account (free) covers installing on the operator's own phone after
  2027.** The guide says it shares apps with explicitly authorized devices by QR code or link; trying
  it once the console opens to everyone would settle it. Free.
- **Whether HumanityOS passes SignPath's "No proprietary code" rule.** The Windows build loads
  `dxcompiler.dll` and `dxil.dll` beside the exe when present (`src/renderer/mod.rs`); whether those
  count as "System Libraries" or proprietary components, and whether any bundled data or asset counts,
  is SignPath's call. Settled by asking SignPath Foundation after the licence vote. Free.
- **The Microsoft Store's limits for a package of about 1 GB of data, and Store policies on the
  Solana wallet and the relay.** Not read today. Settled by reading the Microsoft Store Policies and
  the MSIX package limits. Free.
- **Google Play's policies on cryptocurrency wallets, and its app size limits.** Not read today.
- **Distributions' packaging (Debian, Fedora, Arch), Snap, AppImage, ChromeOS, Meta Quest and game
  consoles.** Not researched in this finding. Each needs its own dated read before any plan relies on
  it; console makers' terms sit behind developer agreements and may not be public at all.
- **Whether a free Apple Account's 7-day profile can be made on a GitHub runner instead of a Mac.**
  Apple's page ties it to "sign in to your Apple Account in Xcode", an interactive step; no source
  read today says it works unattended. Settled only by trying it.

## Sources that could not be reached

- https://f-droid.org/ (every page): robots.txt disallows `ClaudeBot`, `anthropic-ai` and
  `claude-web`. Hoped for: the Inclusion Policy, the Anti-Features list, and F-Droid's response to
  Android developer verification.
- https://support.apple.com/en-us/102445 and
  https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac: "429
  Too Many Requests" on both tries, so treated as a refusal and not retried further. Hoped for: the
  exact steps and dialog text for opening an unnotarized app on current macOS.
- https://www.qualcomm.com/products/mobile/snapdragon/smartphones/snapdragon-8-series-mobile-platforms/snapdragon-888-5g-mobile-platform:
  the page's text is drawn by script and came back empty. Hoped for: the Adreno 660's Vulkan version.

## What it means for HumanityOS

### Certain (stated by the sources above)

- On Windows, the only route with **no warning at all** is the Microsoft Store, and its MSIX route
  costs nothing: free registration with an ID check, and Microsoft signs the package.
- Any signed download outside the Store **still warns** until reputation builds; EV certificates no
  longer skip that.
- Unsigned downloads are **blocked outright** where Windows 11's Smart App Control is on, with no
  per-app exception.
- SignPath Foundation's free certificate requires an OSI-approved licence; CC0 is not one.
- Notarizing for macOS, and any distribution to other people's iPhones and iPads, requires the
  Apple Developer Program at 99 USD a year. An individual cannot get Apple's fee waiver.
- An individual's iOS build may not include a virtual currency wallet (guideline 3.1.5(i)).
- GitHub's standard macOS runners are free for public repositories and carry Xcode.
- Android sideloading needs no registration today; from 2027 an unregistered app needs the advanced
  flow (one-time, a 24-hour wait) or ADB, and a Limited Distribution registration is free.
- Google Play forbids self-updating and downloading native code (".so files"), so a Play build cannot
  keep the desktop updater or download a separate AI runtime.
- Flathub requires disclosure of AI-generated material and lets reviewers reject on it.

### Judgement calls (the project's own reading; a lawyer could disagree)

- **The free route for Windows today is the Microsoft Store (MSIX).** It is the only free route that
  removes the warning, but it publishes under the account holder's own name ("Choose an Individual account if you want to publish apps under your own name"), and Store installs update through the Store, not through our updater.
- **SignPath Foundation becomes the free route for direct downloads if the licence vote picks plan B
  (AGPL-3.0 code).** Under CC0 it is closed as written.
- **Flathub is uncertain, not closed.** Its policy evaluates disclosed AI material "at reviewer
  discretion"; a submission would have to disclose honestly, and the operator would have to write the
  manifest and open the pull request himself.
- **On iPhone there is no free route for anyone but the operator's own devices**, and he does not
  have an iPhone today. iOS waits for money or a nonprofit entity.
- **California AB 1043 (as amended by AB 1856) starts 1 January 2027** and, by the project's earlier
  reading, a listing in an application store would very likely bring HumanityOS inside it
  ([`2026-10-10-california-ab-1043-age-signals.md`](2026-10-10-california-ab-1043-age-signals.md)).
  Any store listing (Microsoft Store, Google Play, Flathub, F-Droid, the App Store) should wait for
  that decision.

**Again: this is a reading of public sources by an AI, not legal or financial advice, and nobody who
wrote it is a lawyer.**
