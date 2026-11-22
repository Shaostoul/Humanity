# Can HumanityOS get permission to play Netflix, Disney+ and Prime Video (Widevine)?

**Research date: 21 November 2026.** Every source below was read on that date. Each quotation carries the source's
own date where the page gave one; many pages give none, and that is said. If you are reading this later, compare those
dates with what has changed, and re-check the items flagged at the end.

**I am not a lawyer. This is a reading of public sources, not legal advice.** Nothing here has been reviewed by anyone
qualified to give an opinion. Where sources are silent or conflict this document says so. The one place a real
opinion would change the answer is named in "What is still unknown".

## The question

How does a third-party app or browser obtain the Widevine content decryption module and the signature the streaming
services check, what does it involve, can a free, open-source project distributed under AGPL-3.0 realistically obtain
it, and what do Netflix, Disney+ and Prime Video themselves require of a browser?

## The short answer

**No route was found that a project like ours can simply take.** What exists:

- Widevine is licensed by agreement. Google says "A license agreement is required for the use of Widevine products or
  services" and "Widevine does not assess any fee for use of its products and services." No application form for a
  desktop browser or app was found on any page I could read.
- Every open-source-based browser I checked that has Widevine (Firefox, Brave, Vivaldi) fetches Google's module from
  Google at run time; none of their pages I could read describes the agreement behind it. Two could not be read.
- A **free signing service** exists for one engine family: castLabs EVS, which signs "production Widevine/VMP" for
  macOS and Windows packages of Electron for Content Security. Whether it signs a CEF app is **not stated** on its page.
  CEF's own tracker says the signature "require[s] a signing certificate from Google".
- Whether the module plays anything without that signature, and at what quality, is **not stated** by any page I could
  read. Netflix's help page lists only named browsers, and a custom CEF or Servo app is not on that list.
- Servo's tracker has no Widevine or EME entry (0 results). Ladybird has begun the EME interface in code (ClearKey
  only, merged 2026-11-20), with Widevine not mentioned.
- Whether a signature that only our own build carries sits comfortably with AGPL-3.0 is an open question for a
  lawyer (below).

**Therefore the project should not plan any feature around paid streaming services.** This is a platform holder's
rule (a company decides which apps may play its content), not a law and not a patent.

---

## Finding 1: Widevine is licensed by agreement, free of charge

Source: [Widevine overview, Google for Developers](https://developers.google.com/widevine/drm/overview). No date on the page.

> A license agreement is required for the use of Widevine products or services.

> Widevine does not assess any fee for use of its products and services.

> If you are interested in integrating Widevine into a device, please contact us.

Source: [Widevine access, Google for Developers](https://developers.google.com/widevine/access). No date on the page.
It concerns access to Widevine's source repositories, not to the browser module:

> Partner access to the repository is contingent upon signing of the Widevine Master License Agreement.

It also says a Google account with a "valid corporate individual email address" is required and personal accounts
such as gmail.com are not accepted. "Access to each repository is evaluated and approved separately."

**What this does not say:** how an app that is only a *user* of the browser module (not a device maker) applies, whether
a non-company can sign, or what the Master License Agreement contains. I found no page for browsers.

## Finding 2: Third-party labs exist, but their pages describe devices

Source: [Widevine Third Party Labs (3PL)](https://developer.widevine.com/solutions/widevine-3pl), footer "(c) 2025 Google",
no other date.

> allows for licensed and authorized third party partners to assist with the implementation and approval

It lists three services (test execution, product development support, upgrade and maintenance support) and the labs
Altimedia, castLabs, Irdeto, SEI Robotics, Skyworth and Smartlabs. It does **not** mention VMP certification or browser
apps, and gives no prices ("Contact the labs directly" is my reading of the silence).

## Finding 3: castLabs EVS, the free signing service

Source: [castlabs/electron-releases wiki, "EVS"](https://github.com/castlabs/electron-releases/wiki/EVS), last edited
14 May 2026.

> provides production Widevine/VMP signing of `macOS`/`Windows` application packages

> It is a free service, but requires signup to use.

It signs for "streaming" (streaming-only) or "persistent" (offline playback) and "does not provide signing of `Linux`
binaries". It names castLabs as "a partner in the Widevine 3PL program". It does not mention Netflix, Disney+, security
levels, or any agreement with Google.

Source: [castlabs/electron-releases wiki home](https://github.com/castlabs/electron-releases/wiki), last edited
7 November 2026, on what can go wrong:

> In May 2022, EVS was temporarily disabled for signing ECS "at the request of Widevine" while they investigated possible misuse.
> (page wording as read; full functionality restored 2022-05-24)

and that Widevine's concerns "may prompt future changes to the terms of the service". It also says Google's current policy
is that a supported module is available through the component updater for about a year of Chromium releases, and an
older release's ability "can be revoked at any time" (the page's own words, as read).

**What this does not say:** that EVS signs anything other than Electron for Content Security builds. Everything on the
page is framed around ECS. For a CEF or Servo app **the page gives no answer**, so this is an open item, not a route.

## Finding 4: CEF's own notes on the signature

Source: [CEF issue 3404, "widevine: Enable support for persistent licenses (VMP)"](https://github.com/chromiumembedded/cef/issues/3404),
opened 4 October 2022 by the CEF maintainer, closed 5 October 2022.

> which require a signing certificate from Google

Sig files are required beside the binaries, "'.exe.sig' and 'libcef.dll.sig' files must exist in the same directory as
the named binary" (comment, 4 October 2022), and Chromium "will selectively enable CDM host verification at runtime if
valid sig files exist". "CDM host verification is only available on Windows and MacOS." On Linux (comment 26 October 2022):
"Persistent licenses are not supported, only streaming licenses", and the service must allow clients with
"PLATFORM_UNVERIFIED status".

Source: [CEF issue 3149, Widevine CDM as a component](https://github.com/chromiumembedded/cef/issues/3149), opened
17 June 2021, closed 10 August 2021: "Widevine CDM binaries will be downloaded on supported platforms shortly after
application startup." The page "contains no mention of VMP, code signing, or licensing terms".

Source: [CEF issue 3952, "win: Support Widevine VMP with bootstrap"](https://github.com/chromiumembedded/cef/issues/3952),
23 June 2025: "VMP support requires that all binaries be listed in AddCdmHostFilePaths." (A technical fix: it shows
the signature covers every binary of the app, so the signed set must match what ships.)

## Finding 5: Who in the open-source browser world has Widevine

| Browser | What their own page says | Source date |
|---|---|---|
| **Vivaldi** | "Widevine is a 'Content Decryption Module' (CDM) that provides Vivaldi with support for" EME; users update it at `vivaldi://components` | [help page](https://help.vivaldi.com/desktop/media/widevinecdm-eme-drm-netflix-amazon-spotify/), no date |
| **Firefox** | **Could not read.** support.mozilla.org returned a robot check. A search summary says Firefox fetches Google's CDM on demand, runs it in a sandbox, and lets people turn it off. | not quoted |
| **Brave** | **Could not read.** support.brave.app returned 403. A search summary says it is "disabled in Brave by default" and the person agrees to Google's terms to install it. | not quoted |
| **Zen** | A community post (not a maintainer) on 2026-07-09 says it "waits for license approval from Google"; a reply on 2026-11-19 is only a link to a post I could not read | [discussion 14536](https://github.com/zen-browser/desktop/discussions/14536) |

A small independent browser that has been waiting months for Google's approval (Zen, if the post is right) is a signal of
how the application is going for projects of our size. **A community post is weak evidence, shown as such.**

## Finding 6: Do Servo or Ladybird plan DRM?

Servo: searching its issues for "EME DRM" and for "widevine" returned **0 results** each (2026-11-21), and no page of
servo.org or its book mentions DRM. **Negative finding: no plan on record.**

Ladybird: issue 11390, "Spotify cannot play songs due to missing EME implementation", opened 27 August 2026, open, with a
contributor's comment "Spotify is using a DRM system. So the core problem is likely missing support for Encrypted Media
Extensions." Pull request 15167, "LibWeb: Implement Encrypted Media Extensions in Rust", merged 20 November 2026; per the
page's automated walkthrough, only ClearKey is mentioned and the change "does not establish full EME playback support".
Ladybird's FAQ does not mention DRM or Widevine. Neither project has said it would seek Widevine in words I found.
([Servo issues search](https://github.com/servo/servo/issues?q=widevine), [Ladybird issue](https://github.com/LadybirdBrowser/ladybird/issues/11390),
[PR](https://github.com/LadybirdBrowser/ladybird/pull/15167).)

## Finding 7: What Widevine's levels mean

Source: [Chrome 62 media updates](https://developer.chrome.com/blog/media-updates-in-chrome-62/), no date on the page
(it covers Chrome 62, 2017):

> all Android devices are required to support Widevine Security Level 3 (Widevine L3)

> all content processing, cryptography, and control is performed within the Trusted Execution Environment (TEE)

(the second is Level 1). I did not find a Google page defining L3 as "software" in so many words. The common
statement that browsers on a PC are typically L3 is therefore **reported, not verified here**. Netflix's page below
is the evidence of what services then do with a browser.

## Finding 8: What the services say about browsers

**Netflix**, [supported browsers and system requirements](https://help.netflix.com/en/node/30081), no date on the page.
Selected rows as read:

> Windows: Edge 118 or later Up to Ultra HD (2160p)*; Chrome 117 or later Up to Ultra HD (2160p)*;
> Firefox 129 or later Up to Full HD (1080p); Opera 92 or later Up to Full HD (1080p)

> Linux: Chrome 117 or later Up to Full HD (1080p); Firefox 129 or later Up to Full HD (1080p) ...

and "Due to the many configurations of Linux, we can't guarantee Netflix will work on devices with a Linux OS". On the
other help page the same table says "Ultra HD (2160p) is only available on Windows computers that meet these Ultra HD
requirements". The page does not mention Widevine or DRM at all.

**Disney+:** the browser-requirement help page loaded empty (script-rendered), so it could not be read. Third-party
reports say a 720p cap and a Level 1 requirement; I do not rely on them.

**Prime Video:** primevideo.com and amazon.com robots.txt bar Claude-User, so no page was read.

**Meaning:** Netflix lists named, shipping browsers. An app made from Chromium or CEF is not a named browser. Whether
a service refuses it, serves it at low quality, or accepts it is **not stated anywhere I could read**.

## Finding 9: What WebView2 shows about the market

Not Widevine, but relevant: Microsoft's WebView2 Fixed Runtime failed PlayReady while Evergreen worked
([issue 4632](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4632), June 2024), and a request for Widevine
in WebView2 ([issue 4828](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4828), opened 21 September 2024)
got one reply, on 22 November 2024: "share the workaround you are currently using?". Microsoft, an operating-system
holder, has not delivered it either.

---

## The open question for a lawyer: the signature and AGPL-3.0

Our code is licensed AGPL-3.0-or-later. Section 6 of the AGPL (read from the repository's `LICENSE`) says:

> "Installation Information" for a User Product means any methods, procedures, authorization keys, or other information
> required to install and execute modified versions of a covered work in that User Product from a modified version of
> its Corresponding Source. The information must suffice to ensure that the continued functioning of the modified object
> code is in no case prevented or interfered with solely because modification has been made.

and that this applies when the work is conveyed "in, or with, or specifically for use in, a User Product", where a User
Product is "(1) a 'consumer product', which means any tangible personal property which is normally used for personal,
family, or household purposes".

The facts that make the question: (a) a Widevine VMP signature, per CEF issue 3404, is made with a certificate from
Google or a lab and covers named binaries; (b) a modified build would carry no valid signature, so protected content would
not play in it. **The question, stated without a conclusion:** if the project distributes a signed build to people on a
personal computer, does the "Installation Information" clause require it to give recipients whatever lets *their modified
build* carry the same signature (and is a personal computer a "User Product" at all, as "doubtful cases shall be resolved
in favor of coverage")? The clause also says the requirement "does not apply if neither you nor any third party retains the
ability to install modified object code" and does not require "support service, warranty, or updates", which may or may
not help. I did not find any source that applies this clause to a VMP signature.

## What is still unknown, and what would settle it

| Unknown | What would settle it | Cost |
|---|---|---|
| Does EVS or any Widevine route accept a CEF or Servo app (not Electron)? | Ask castLabs (their sign-up and contact), or ask Google's Widevine contact for the browser agreement's text | an email; the answer may be a contract |
| The terms of the Widevine Master License Agreement for a browser | the agreement itself, which is not published | needs a legal entity and a request |
| Whether a service accepts an unlisted app and at what quality | trying it on an account, or the service's partner terms | a subscription and a signed build |
| Disney+ and Prime Video browser rules | a person reading their help pages by hand | minutes |
| The AGPL question | a lawyer who reads Section 6 and a signature agreement | professional fees |
| Whether Zen's wait for approval is real and why | its maintainers' own statement | a question to them |

## Sources I could not reach

- `support.mozilla.org/en-US/kb/enable-drm`: robot check (wanted: Firefox's own statement on Widevine).
- `support.brave.app/hc/en-us/articles/360023851591`: 403 (wanted: Brave's agreement wording).
- `help.disneyplus.com` browser requirement article: empty to a text reader.
- `help.twitch.tv/s/article/stream-quality`: empty to a text reader.
- Prime Video help pages on primevideo.com and amazon.com: robots.txt bars Claude.
- Any Google page for a browser Widevine agreement: not found.
- The Widevine Master License Agreement and the castLabs paid certification terms: not public.

## What this means for this project

**Certain:**

- Widevine is by agreement with Google; "no fee" for the service, but a licence agreement is required.
- castLabs EVS is free with sign-up, for macOS and Windows, framed for Electron for Content Security.
- CEF's own tracker says the signature needs a certificate from Google, covers every shipped binary, and works only
  on Windows and macOS, with streaming-only licences on Linux.
- Netflix lists named browsers by version; a custom app is not on its list.
- Servo has no Widevine or EME plan on its tracker; Ladybird has started the EME interface with ClearKey only.

**Judgement calls (mine, arguable):**

- Do not plan features that depend on a paid streaming service, and say so on screen where a person meets it, naming it
  a platform holder's rule (the line already in `docs/design/embedded-browser.md`).
- If the operator wants to try, the cheapest honest probe is an email to castLabs asking whether EVS signs a CEF app, with
  the AGPL question put to a lawyer before any signed build is distributed.
- Software-level (L3) Widevine without the signature is not worth building toward: no source says a service will accept it.

**Not a lawyer:** this finding is a reading of public pages. Do not rely on it for any agreement or distribution decision
without a qualified opinion.
