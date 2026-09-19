# Can HumanityOS lawfully play H.264 video and AAC audio?

**Research date: 19 September 2026.** Every source below was read on that date.
Every quotation carries its own publication or last-updated date, which is often
much older. If you are reading this later, compare those dates against whatever
changed, and re-check the three things flagged as moving targets at the bottom.

**I am not a lawyer. This is a reading of public sources, not legal advice.**
Nothing here has been reviewed by anyone qualified to give an opinion. Where the
sources are ambiguous this document says so instead of picking a side, and where
the sources disagree it shows both. The one place a real opinion would change the
answer is named explicitly in "What is still unknown".

## The question

Twitch sends H.264 video with AAC audio, our player and our measured browser
engine carry neither decoder, and the operator wants Twitch to work: is there a
lawful route to H.264 and AAC playback for a free, open-source, donation-funded
project that publishes binaries from a public repository under one named person?

## The short answer

**Yes, probably, and the cheapest route is the one that ships no decoder at all:
ask the operating system, which already has both codecs and is already licensed
for them.** The maintainer of the browser engine we measured has himself written
down that this route exists and why he believes it is licence-free, and has an
open ticket to build it. It gets us H.264 video today on Windows and macOS. It
does **not** get us AAC audio, because the browser engine has no path to the
platform audio decoder, so today that route yields a picture with no sound.

The other routes each fail for a specific, nameable reason:

| Route | Verdict |
|---|---|
| Cisco's prebuilt OpenH264, the way Firefox uses it | **Lawful and genuinely free, but useless here.** It decodes Constrained Baseline Profile only, and Twitch sends Main or High. It has no AAC at all. |
| Wait for the patents to lapse | **Not yet in the United States.** Europe, China and Japan cleared in 2025. The last US H.264 patent on the tracked list expires 2030-11-26. AAC-LC may already be clear in the US, and sources disagree. |
| Call the operating system's own decoders | **The most promising.** Video works, audio does not, and the gap is engineering rather than law. |
| Buy a pool licence | **Affordable for video, not for audio, and unshippable either way.** H.264 is free under 100,000 units a year. AAC has no free tier. Neither can be passed to a fork or a mirror. |
| Buy a commercial codec pack | **Exists** (Fluendo), price unpublished, and has the same non-transferability problem. |

The decisive constraint is not money. It is that **a patent licence covers the
licensee, and this project's whole distribution model is other people
redistributing it.** That was already the reasoning in
[`media-player.md`](../../design/media-player.md), and nothing found here
overturns it. What the research does overturn is the implied conclusion that
therefore H.264 is unreachable. It is reachable, by not shipping a decoder.

---

## Finding 1: The Cisco route

### What Cisco actually grants

Source: [`https://www.openh264.org/BINARY_LICENSE.txt`](https://www.openh264.org/BINARY_LICENSE.txt),
the file itself marked `v1.0` and carrying a 2014 Cisco copyright, retrieved
2026-09-19.

> Cisco provides this program under the terms of the BSD license.
>
> Additionally, this binary is licensed under Cisco's AVC/H.264 Patent Portfolio License from MPEG LA, at no cost to you, provided that the requirements and conditions shown below in the AVC/H.264 Patent Portfolio sections are met.

The grant covers **both directions**, encode and decode, but only for
unremunerated use:

> THIS PRODUCT IS LICENSED UNDER THE AVC PATENT PORTFOLIO LICENSE FOR THE PERSONAL USE OF A CONSUMER OR OTHER USES IN WHICH IT DOES NOT RECEIVE REMUNERATION TO (i) ENCODE VIDEO IN COMPLIANCE WITH THE AVC STANDARD ("AVC VIDEO") AND/OR (ii) DECODE AVC VIDEO THAT WAS ENCODED BY A CONSUMER ENGAGED IN A PERSONAL ACTIVITY AND/OR WAS OBTAINED FROM A VIDEO PROVIDER LICENSED TO PROVIDE AVC VIDEO.  NO LICENSE IS GRANTED OR SHALL BE IMPLIED FOR ANY OTHER USE.

### Who must distribute the binary, which is the detail that matters

The four conditions, quoted in full because the first one is the whole answer:

> In addition, the Cisco-provided binary of this Software is licensed under Cisco's license from MPEG LA only if the following conditions are met:
>
> 1. The Cisco-provided binary is separately downloaded to an end user's device, and not integrated into or combined with third party software prior to being downloaded to the end user's device;
>
> 2. The end user must have the ability to control (e.g., to enable, disable, or re-enable) the use of the Cisco-provided binary;
>
> 3. Third party software, in the location where end users can control the use of the Cisco-provided binary, must display the following text:
>
>        "OpenH264 Video Codec provided by Cisco Systems, Inc."
>
> 4.  Any third-party software that makes use of the Cisco-provided binary must reproduce all of the above text, as well as this last condition, in the EULA and/or in another location where licensing information is to be presented to the end user.

So the coverage **does** survive a third party's program downloading the binary
at runtime. That is precisely what the conditions describe, and it is what
Firefox does. What breaks the coverage is bundling: put the binary inside your
installer and condition 1 fails.

Note what the text does and does not say. It says "separately downloaded to an
end user's device". It does **not** say, in so many words, that the bytes must
come from a Cisco server. Fedora's practice reads it as though it does. The
Fedora wiki ([`fedoraproject.org/wiki/OpenH264`](https://fedoraproject.org/wiki/OpenH264),
a wiki page with no displayed date, retrieved 2026-09-19) describes a repository
that contains

> OpenH264 binary built inside the Fedora infrastructure, but distributed by Cisco, so that the all licensing fees are still covered by them.

Read that carefully: Fedora **built the binary itself, inside its own
infrastructure, and then handed the hosting to Cisco**. That is a genuinely
awkward arrangement to set up, and a distribution does not do it for fun. The
plain inference is that Fedora believed the bytes had to come from Cisco for the
grant to hold. I could not find an official Fedora or Cisco statement saying so
in those words, and a forum thread widely quoted as saying it did not contain
the sentence when I fetched it (see "Sources I could not reach").

**So the two readings disagree and I cannot close the gap from public sources.**
Under the licence text alone, we could mirror the binary ourselves and let our
app fetch it from us. Under the behaviour Fedora actually adopted, the fetch
must hit Cisco. Fedora has lawyers and made the more expensive choice, so treat
Cisco-hosted as the requirement until someone asks Cisco directly.

### A practical risk, if this route is ever revisited

Depending on a third party's CDN at runtime is a dependency, not just a licence
term, and it has already failed in the field. Cisco's binary host geo-blocks
some countries, returning HTTP 403, which broke package updates for Fedora users
in those regions
([cisco/openh264 issue #3886](https://github.com/cisco/openh264/issues/3886),
and a long [Fedora Discussion thread](https://discussion.fedoraproject.org/t/ciscobinary-openh264-org-is-unreachable-in-some-countries-ru-ua-ir/161434)
naming Russia, Ukraine and Iran). Fedora 44 changed its openh264 packaging in
response, and a Fedora Discussion post of 12 August 2026 gives the reason: "the
change was made since there are regions that could not access the
fedora-cisco-openh264 repo". A project whose whole point is reaching everyone
should note that this route has a geography attached to it.

### Who else uses it this way

Mozilla, since Firefox 33 in October 2014. Fedora, in the
`fedora-cisco-openh264` repository, shipped since Fedora 24 and enabled by
default since Fedora 33. GStreamer, through `gstreamer1-plugin-openh264`.
FFmpeg can use OpenH264 as both encoder and decoder. So the answer to "has
anyone other than Mozilla done this" is yes, and one of them is an entire Linux
distribution that restructured its package hosting to satisfy condition 1.

### Why it does not help us

Cisco's own README states the limit for **both halves** of the codec:

> Constrained Baseline Profile up to Level 5.2 (Max frame size is 36864 macro-blocks)

Source: [`github.com/cisco/openh264` README](https://github.com/cisco/openh264),
retrieved 2026-09-19. The gap has been known since the beginning. Issue
[#1407](https://github.com/cisco/openh264/issues/1407), opened 9 October 2014
and since closed without the feature:

> With just Constrained Baseline Profile decoding support, OpenH264 is basically limited to WebRTC and WebRTC only. The vast, vast majority of H.264 video (especially HD video) on the web uses High Profile.

Mozilla says the same thing about its own use of it. From
[`wiki.mozilla.org/Media/openh264`](https://wiki.mozilla.org/Media/openh264),
last edited 13 August 2020:

> OpenH264 is for WebRTC **only** for the time being. Streaming video decode typically requires H.264 profiles not included yet in OpenH264 and also audio codecs that are not freely available.

and

> it doesn't enable playing back MP4 videos at this time

Our own measurement in
[`embedded-browser.md`](../../design/embedded-browser.md) records that Twitch's
"baseline is H.264 with AAC (HEVC for 1440p, AV1 still in beta)". I could not
fetch Twitch's own encoding guidelines to pin the exact profile (see "Sources I
could not reach"); secondary guides consistently say Main or High with AAC-LC,
and Mozilla's statement above says the general case directly: streaming video
"typically requires H.264 profiles not included yet in OpenH264". Constrained
Baseline is a narrow subset that essentially only WebRTC emits, so the
conclusion holds whichever of Main or High Twitch actually sends. And OpenH264
has no audio decoder at all, so even a lucky profile match would leave the
stream silent.

**Verdict: lawful, free, well-trodden, and the wrong shape for this problem.**
It would be a fine answer if we wanted WebRTC video calls with H.264-only
hardware. It is not an answer for Twitch.

---

## Finding 2: Do the patents still bite?

### H.264

Tracking page:
[Have the patents for H.264 MPEG-4 AVC expired yet?](https://meta.wikimedia.org/wiki/Have_the_patents_for_H.264_MPEG-4_AVC_expired_yet%3F),
Wikimedia Meta-Wiki, **last edited 16 September 2026** (three days before this
research, so it is current). Its own scope note and disclaimer:

> This article only considers H.264 Version 3 / High Profiles, which are the most common used profiles.

> This page is not suitable for official legal advice. If you require official legal advice, please call your lawyer.

Its headline answer:

> NO! (at least in Brazil, Malaysia, and the United States) YES! (elsewhere)

Patents it lists as not yet expired, with the page's own dates:

| Patent | Holder | Expires |
|---|---|---|
| US 8175148 | Nokia | 2026-12-03 |
| US 7702013 | Fraunhofer-Gesellschaft | 2027-01-16 |
| US 7630435 | Panasonic Holdings Corporation | 2027-02-11 |
| US 8050321 | Nokia | 2027-05-19 |
| US 7609767 | Microsoft Corporation | 2027-08-09 |
| US 7684489 | Cisco Technology, Inc. | 2027-09-09 |
| US 8204134 | Nokia | 2028-01-21 |
| BR PI0304568-4 | not stated in the rows I retrieved | 2030-11-10 |
| US 9356620 | Siemens AG | 2030-11-26 |

Two cautions on that table. The page's headline also names **Malaysia** as
unexpired, and no Malaysian row came back in what I retrieved, so the table above
is not necessarily the complete set. And the page marks its own attribution of
the two later Nokia entries to Scalable Video Coding with question marks, so
whether those cover ordinary playback is uncertain on the page's own account.

Already gone, per the same page and a Fedora developer post: the last European
patents EP 1709801 and EP 2384002 expired 2025-01-26; the last Japanese patents
JP 4628216 and JP 4892628 on 2025-08-09; the last Chinese patent CN 1922888 on
2025-08-16. Those three dates were flagged in advance on the Fedora devel list
by Mateus Rodrigues Costa on 1 August 2025
([archive](https://www.mail-archive.com/devel@lists.fedoraproject.org/msg208369.html)),
asking whether Fedora could improve its default multimedia support once they
lapsed. The same ground was covered in the LWN comments on 2 and 3 June 2025
([LWN](https://lwn.net/Articles/1023539/)).

**So "mostly expired but not entirely" is exactly right, and the "not entirely"
is the United States, where we publish.** Taking the table at face value, the
last US entry falls on **2030-11-26**. If the Siemens patent turns out not to
read on ordinary decoding, the next date back is 2028-01-21, and if the Nokia
entries really are Scalable Video Coding as the page half-suggests, 2027-09-09.
All three are years away, and none of them is a plan for a feature wanted now.
Note also that the page's scope is *High Profile*, which is what a streaming
platform sends, so this is the relevant reading rather than a conservative one.

### AAC

Much less clear, and the sources genuinely conflict.

**The permissive reading** is Fedora's, and it is nine years old and still
standing. Phoronix dated the change to 12 October 2017
([article](https://www.phoronix.com/news/Fedora-FDK-AAC), Michael Larabel):
"Fedora is now able to distribute a third-party modified version of the
Fraunhofer FDK AAC codec for Android." Jacob Adams, writing on 25 February 2024
in [AAC and Debian](https://tookmund.com/2024/02/aac-and-debian), states what
that package is scoped to and why:

> This version of the library includes only the AAC LC profile, which is believed to be entirely patent-free.

Fedora has not backed away from it even when the licence classification moved
against it. From [Fedora's Licensing/FDK-AAC wiki page](https://fedoraproject.org/wiki/Licensing/FDK-AAC),
retrieved 2026-09-19:

> As of some date in August 2022, the FDK-AAC license is expected to be reclassified as not-allowed (essentially, non-free) because of its 'no patent licenses' feature. This is not expected to have an impact on the continued packaging of fdk-aac-free in Fedora.

A Red Hat legal review is widely reported to sit behind the 2017 decision. I
could not reach a primary Fedora or Red Hat statement saying so, so treat that
as reported rather than verified; what is verifiable is the shipping record.

**The restrictive reading** is in the same Debian article, and the author flags
his own doubt about it:

> the "base" patents expire in 2031, with the extensions expiring in 2038

He attributes that to Wikipedia and then undercuts it, noting the underlying
source "is some guy's spreadsheet in a forum", which is why he declined to do a
patent deep-dive himself.

Also worth knowing: the FDK AAC licence explicitly grants nothing on patents.

> NO EXPRESS OR IMPLIED LICENSES TO ANY PATENT CLAIMS, including without limitation the patents of Fraunhofer, ARE GRANTED BY THIS SOFTWARE LICENSE.

And Via LA's pool is still selling AAC licences, still taking new licensees,
and still lists AAC-LC among the profiles it covers (see Finding 4). A pool
continuing to license a technology is not proof the technology is still
patented, but it is not nothing either.

**Verdict: AAC-LC in the US is probably clear, and the evidence on the two sides
is of very unequal quality.** On one side, a major distribution shipping the
thing for nine years without being sued. On the other, a figure its own relayer
traces to an unsourced forum spreadsheet. I lean strongly to the first and would
still want a lawyer before acting on it, because "nobody sued Fedora" is an
argument about risk rather than about entitlement. Twitch uses AAC-LC
specifically, which is the variant on the permissive side of the argument.

---

## Finding 3: The operating system's own decoders

This is the most useful thing the research turned up, and it comes from the
maintainer of the exact browser engine we measured.

Source: [chromiumembedded/cef issue #3559, "Enable proprietary codecs with OS
decoding support only"](https://github.com/chromiumembedded/cef/issues/3559),
opened by `magreenblatt` (the CEF maintainer) on 24 August 2023, **still open**
as of 2026-09-19. Quoted from the issue body:

> By default, CEF/Chromium builds have proprietary codecs (H.264/AAC) disabled [1]. This is due to patent portfolio licensing requirements when using the FFmpeg library for software decoding of proprietary codecs (via the `ffmpeg_branding=Chrome` part). If you enable and distribute (via FFmpeg configuration) a software implementation of the patent protected technology as part of your application then you will likely be subject to said licensing requirements and associated costs.

And then the part that matters:

> On Windows and MacOS, a (preferred) hardware video decoding path using OS platform APIs already exists in Chromium, and this implementation does not depend on configuring or distributing FFmpeg with software decoding of proprietary codecs enabled (leave out the `ffmpeg_branding=Chrome` part [2]). The OS manufacturers (Microsoft and Apple respectively) allow applications to use these OS platform APIs with no added licensing requirements or costs. We can consequently enable hardware-only support for proprietary video codecs (via OS platform APIs) with only minor changes to the existing CEF/Chromium build configuration.

The cost, in his words:

> The problem with this approach is that devices lacking hardware decoding support will not be able to play proprietary codecs (due to the lack of software decoding support via FFmpeg). This is most likely to impact Windows users with old or blocklisted GPU hardware.

And the measured state, dated in the issue itself:

> 8/28/23 SUMMARY OF TEST RESULTS (M117)
>
> - Chromium supports hardware h264 video decoding via OS APIs with minimal changes on Win/Mac.
> - Chromium does not (directly) support AAC audio decoding.
>   - FFmpeg on Mac should theoretically support AAC decoding via AudioToolbox, but we were unable to get it working.
>   - FFmpeg on Windows does not support decoding via Media Foundation (only encoding).
> - Chromium with `proprietary_codecs=true` includes additional (non-FFmpeg) proprietary code in //media that may potentially need to be replaced by OS APIs.

**That is the whole shape of the answer: video yes, audio no, and the audio gap
is engineering work nobody has finished.** Three years after that summary the
issue is still open, which is itself informative about how much appetite there
is for the work.

### Is the legal theory sound?

The claim "the OS manufacturers allow applications to use these OS platform APIs
with no added licensing requirements or costs" is a CEF maintainer's assertion,
not a statement from Microsoft or Apple. I could not find either vendor saying
it plainly, and a developer who asked Microsoft directly got no answer.
[Microsoft Q&A, asked by Nicholas V on 24 October 2022](https://learn.microsoft.com/en-us/answers/questions/1060279/using-windows-media-foundation-to-encode-h264-(lic):

> Since it seems that the h264 encoder (Mfh264enc.dll) is preinstalled with all new windows installations as of today can I use the encoder (via windows media foundation) commercially in my product?

The only answer, from Lex Li the same day, ends:

> Here you need a lawyer, not some random guy over the internet.

What both vendors do carry, according to secondary sources I could not verify at
the vendors themselves, is the pool's standard consumer notice, which restricts
the **end user** rather than the developer: the AVC functionality is licensed
for the personal and non-commercial use of a consumer, to decode video obtained
from a video provider licensed to provide AVC video. Twitch is such a provider,
so an ordinary viewer is inside that notice. The full wording of that notice is
quoted verbatim in Cisco's licence file in Finding 1, which I did read directly;
it is the same boilerplate that appears on cameras, phones and operating systems
everywhere. It says nothing either way about a developer calling the decoder.

The underlying argument, which I find persuasive but cannot verify, is that a
patent covers making, using and selling an implementation, and an application
that ships no implementation and calls one the user already owns is not making
or selling anything. That is the same posture the project already takes in
[`media-stance.md`](../media-stance.md) toward the machine's own ffmpeg and the
machine's own disc libraries. It is not a new position; it is the position we
already hold, applied to one more thing.

### Others are trying the same route

[ungoogled-chromium issue #2752, "Enable platform decoder for aac and hevc"](https://github.com/ungoogled-software/ungoogled-chromium/issues/2752),
opened 11 March 2024, still open. The stated motive: "Remove the licensing
problem caused by the possible inclusion of proprietary aac/hevc codec in ffmpeg
decoders." The submitter offered a patch already running in the Cromite fork.
So at least one shipping Chromium fork has platform-decoder AAC working on some
platforms, which is direct evidence the engineering is possible.

**Verdict: the best route, and incomplete.** H.264 video is reachable now on
Windows and macOS. AAC is the blocker, Linux is unaddressed, and machines with
old or blocklisted GPUs get nothing.

---

## Finding 4: What a licence would cost

### H.264, from Via LA

Source: [Via LA, AVC/H.264 licensing programme](https://www.via-la.com/licensing-programs/avc-h-264/),
read 2026-09-19; the page's patent attachment is marked "Updated August 1, 2026".

Codec manufacture and sale, products sold to end users and OEMs for PCs:

| Volume, per unit, annual reset | Fee |
|---|---|
| 1 to 100,000 units | $0.00 (available to one legal entity in an affiliated group) |
| 100,001 to 5,000,000 | $0.20 each |
| 5,000,001 and more | $0.10 each |

Enterprise annual maximum: $3.5M (2005 to 2006), $4.25M (2007 to 2008), $5M
(2009 to 2010), $6.5M (2011 to 2015), $8.125M (2016), and **$9.75M per year in
2017 and after**.

**HumanityOS would pay nothing.** We are nowhere near 100,000 units a year, and
the free tier is not a discount, it is $0.00. This is a genuinely important
figure and it is easy to miss.

### AAC, from Via LA

Source: [Via LA, AAC rate structures](https://www.via-la.com/aac-license-fees-structures/)
and [AAC programme page](https://www.via-la.com/licensing-programs/aac/), both
read 2026-09-19, neither carrying a visible last-updated date.

Standard worldwide rates, per unit with annual reset: $0.98 for the first
500,000 units, then $0.78, $0.68, $0.45, $0.42, $0.22, $0.20, $0.15 and $0.10 at
the higher tiers. An alternative region-based structure exists where "R2"
territories pay $0.64 at the first tier. Products with more than two channels
count as 1.5 units.

**There is no free tier and no cap.** At $0.98 a unit from the first unit, ten
thousand downloads of a free product is a $9,800 bill. The programme covers
AAC-LC, HE-AAC, HE-AAC v2, xHE-AAC, AAC-LD, AAC-ELD and MPEG-D DRC, from fifteen
licensors including AT&T, Dolby, Fraunhofer, Google, Microsoft, NEC, NTT,
Orange, Panasonic, Philips, Samsung and Sony. The term is "five years, and can be
renewed for additional five year terms". Distributing AAC bitstreams is free:
there are "no patent license fees due for the distribution of bit-streams
encoded in AAC".

**So the audio is the expensive half, not the video.** That inverts the usual
assumption and it is the second most useful fact in this document.

### A 2026 change worth knowing about

Via LA restructured its AVC **streaming** licence during 2025 without a public
announcement. Reported by Luke James, Tom's Hardware, 3 April 2026
([via Yahoo Finance](https://finance.yahoo.com/sectors/technology/articles/firm-quietly-boosts-h-264-165304375.html)):
a flat "$100,000 annual cap" was replaced by tiers reaching "$4,500,000 per year
for the largest platforms", applying to "unlicensed implementers seeking a new
license in 2026 or later", while "all companies that held an active AVC license
as of the end of 2025" keep their old terms.

This does not touch decoding in a client. It matters to us only if HumanityOS
ever becomes a service that **sends** H.264 to other people, which
[`streaming.md`](../../design/streaming.md) already rules out on the encode
side. Recorded here because it is exactly the kind of quiet change that makes
dating a findings document worthwhile: the terms moved, nobody announced it, and
a project relying on a remembered "$100,000 cap" would have been a year out of
date.

### Commercial alternative

Fluendo sells licensed codec integrations, including an "FFmpeg Enabler"
specifically for enabling "H.264 and AAC playback within their Chromium
distribution", described as a "seamless, legal, and compatible solution"
([fluendo.com](https://fluendo.com/technologies/browsers/), no date on page,
read 2026-09-19). **No price is published.** It is a "talk to an expert" page.
It also cannot solve our real problem, which is transferability, since a
licence bought by us does not travel to a fork.

---

## Finding 5: Who has done this before, and how

| Project | Route | Note |
|---|---|---|
| **Mozilla Firefox** | Both. Cisco's OpenH264 downloaded at runtime for WebRTC, and the platform decoder (Media Foundation, VideoToolbox, system ffmpeg) for HTML5 video. | The Mozilla wiki's own words: OpenH264 "is for WebRTC **only** for the time being" and "doesn't enable playing back MP4 videos at this time". Firefox's actual H.264 video playback is the operating system's, not Cisco's. |
| **Fedora** | Cisco's binary, with the hosting handed back to Cisco to satisfy condition 1. Separately, `fdk-aac-free` for AAC-LC only, since 2017. | The most careful implementation of the Cisco route in existence, and it still only gets Constrained Baseline. |
| **Debian** | `fdk-aac` sits in non-free; a patent-free `fdk-aac-free` has been proposed for main and stuck in the queue since at least 2022. PipeWire ships with AAC deliberately disabled. | A distribution with strong legal instincts choosing to do without rather than guess. |
| **VideoLAN / VLC** | Jurisdiction. From [videolan.org/legal.html](https://www.videolan.org/legal.html): "Neither French law nor European conventions recognize software as patentable" and therefore "software patents licenses do not apply on VideoLAN software". | Not available to us. This project publishes from a US-hosted repository under a named US person. Their answer is their address, and we cannot borrow it. |
| **Cromite / ungoogled-chromium** | Platform decoders for AAC and HEVC in a Chromium fork, explicitly to "Remove the licensing problem caused by the possible inclusion of proprietary aac/hevc codec in ffmpeg decoders". | Direct evidence Finding 3's missing half is buildable. |
| **Fluendo** | Buy a licence, sell it on as a product. | The commercial answer. Price unpublished. |

The pattern across all of them: **nobody free ships a software H.264 or AAC
decoder from a US address.** They either borrow Cisco's grant, lean on the
user's operating system, move to France, or do without.

---

## What is still unknown, and what would settle it

1. **Whether calling the OS decoder needs no licence.** The CEF maintainer
   asserts it, Microsoft declined to answer a direct question, and Apple's
   notice speaks to consumers rather than developers. *Settled by:* an opinion
   from a patent lawyer, or a written statement from Microsoft or Apple. This is
   the one question worth actually paying for an answer to, and it is worth it
   because a yes unlocks the whole feature for free.
2. **Whether AAC-LC is patent-free in the United States.** Fedora has shipped
   `fdk-aac-free` on that basis since 2017 without incident. A Red Hat legal
   review is reported to sit behind that decision, but I could not find Red Hat
   or Fedora saying so themselves, so the reasoning behind the best evidence we
   have is not actually public. Against it, a figure of 2031 whose only traced
   source is a forum spreadsheet. *Settled by:* the same lawyer as item 1, or by
   finding Fedora's own written reasoning, which would convert a shipping record
   into an argument.
3. **Whether CEF with `proprietary_codecs=true` and without
   `ffmpeg_branding=Chrome` ships anything patented.** The maintainer's own note
   says that configuration "includes additional (non-FFmpeg) proprietary code in
   //media". If that code is only a container parser and bitstream splitter, it
   decodes nothing and is probably outside the pool's definition of a product.
   If it contains decode paths, the route is narrower than it looks.
   *Settled by:* reading `media/formats/mp4/` in the Chromium tree ourselves,
   which is free and which we can do.
4. **Whether Twitch would serve something else to a browser that lacks H.264.**
   [`embedded-browser.md`](../../design/embedded-browser.md) already flags this
   as unobserved. Twitch's Enhanced Broadcasting has carried HEVC and AV1 on the
   ingest side since 2024, and Twitch has said it wants to "wait to enable AV1
   codec support until there is a significant enough portion of devices that have
   robust and efficient AV1 decode support". *Settled by:* pointing the probe at
   a live channel from a build with no H.264 and reading what it offers. Cheap,
   and nobody has done it. The same run would settle a smaller open item: the
   exact H.264 profile Twitch delivers, which I could not get from Twitch itself.
5. **Whether the Cisco grant requires Cisco to host the bytes.** The licence text
   says "separately downloaded to an end user's device"; Fedora behaves as though
   it must come from Cisco. *Settled by:* asking Cisco, though the question is
   moot for us while the profile limit stands.

## Sources I could not reach

- `https://raw.githubusercontent.com/cisco/openh264/master/BINARY_LICENSE.txt`, 404. The identical file was read successfully at `openh264.org` and is quoted above from there.
- `https://help.twitch.tv/s/article/broadcasting-guidelines` and `https://link.twitch.tv/BroadcastingGuidelines`, both returned a portal error rather than content. Twitch's own statement of the H.264 profile and audio codec it expects is therefore **not** quoted in this document. Getting it would firm up Finding 1's closing argument, though it would not change the conclusion.
- `https://discussion.fedoraproject.org/t/confusion-about-ciscos-openh264/199050/7`, reachable but did not contain the sentence about downloading RPMs from Cisco directly that search engines attribute to it. That claim is therefore **not** quoted here, and the Cisco-must-host reading rests on Fedora's observable behaviour instead.
- `https://connect.mozilla.org/t5/ideas/native-aac-lc-decoding-patents-expired/idi-p/33929`, HTTP 403. A Mozilla community request for native AAC-LC decoding on patent-expiry grounds; would have been useful for Mozilla's current position on AAC.
- `https://hydrogenaudio.org/index.php/topic,118084.0.html` ("AAC-LC patent expiry?"), HTTP 403. This is the forum thread the Debian article both relies on and doubts, so reading it directly would materially sharpen Finding 2's AAC half.
- `https://src.fedoraproject.org/rpms/fdk-aac-free/raw/rawhide/f/fdk-aac-free.spec`, blocked by anti-scraping. Fedora's own words on what `fdk-aac-free` strips and why are quoted here only at second hand, through the Debian article.
- `https://www.tomshardware.com/...h264-streaming-license-fees...`, paywalled. Read via the Yahoo Finance syndication of the same article.
- `https://via-la.com/wp-content/uploads/2025/09/avcweb.pdf` ("AVC Patent Portfolio License Briefing"), downloaded but not readable. Inflating its streams yields 735 KB of content from which only punctuation, digits and a handful of loose words recover: the text is held in subset-font hex strings, so it needs a PDF tool that resolves font CMaps, not string-matching. The same figures were taken from Via LA's own HTML pages, so no figure in Finding 4 is missing, but **the authoritative document behind them has not been read directly** and this is the single largest hole in Finding 4.
- `https://meta.wikimedia.org/wiki/Have_the_patents_for_AAC_expired_yet%3F`, 404. No such companion page exists for AAC, which is part of why AAC's status is so much murkier than H.264's.

---

## What this means for HumanityOS

### Certain

- **The existing policy in [`media-player.md`](../../design/media-player.md) is
  correct as written, and the reason it gives is the right reason.** Shipping an
  H.264 or AAC decoder inside our binary would need a licence we cannot pass to
  mirrors and forks. Nothing found here contradicts that.
- **The cost of the H.264 licence is not the obstacle.** Under 100,000 units a
  year it is $0.00. If anyone ever argues this is a money problem, it is not.
  AAC is a money problem, at $0.98 per unit with no free tier.
- **The Cisco route cannot play Twitch.** Constrained Baseline only, no audio.
  This is a technical fact from Cisco's own README, not a legal judgement, and
  it closes that option regardless of what any lawyer says.
- **H.264 is not patent-free in the United States and will not be before
  2028-01-21 at the earliest, more likely 2030-11-26.** Waiting is not a plan
  for a feature the operator wants now.
- **Europe, Japan and China cleared in 2025.** If the project ever needs a
  jurisdictional argument, it has a much better one than it had two years ago,
  though not one that helps a US-hosted repository.
- **Twitch video would remain out of reach even with H.264, without AAC.** Any
  plan that solves only the video half produces a silent picture.

### Judgement calls, mine, and arguable

- **The OS-decoder route is the one to pursue.** It fits the posture the project
  already adopted for ffmpeg and for disc libraries in
  [`media-stance.md`](../media-stance.md): use what the machine already has,
  ship nothing. It costs no money, creates no obligation a fork inherits, and
  the engine's own maintainer has written down both the method and his reading
  of why it is clear. I would build it.
- **I would adopt Fedora's position on AAC-LC** rather than the 2031 spreadsheet
  claim: nine years of a company with lawyers shipping it unchallenged is real
  evidence. That said, the difference matters only if we ever decode AAC
  ourselves, which the OS route avoids entirely, so we may never need to decide.
- **I would not buy a pool licence,** even the free-tier H.264 one. A licence
  covers the licensee. The moment someone forks or mirrors us, which is the
  entire point of the project, it stops covering them, and we would have
  published a binary whose lawfulness depends on a paper only we hold. The
  $0.00 price tag makes this tempting and it should be resisted for that reason.
- **The realistic near-term outcome for Twitch is still no.** Video is
  solvable, audio is not solvable today without finishing work that two
  Chromium-adjacent projects have left open for years. Per the standing rule in
  `CLAUDE.md` about saying what we will not build and why, the honest sentence
  on the screen names a PATENT limit for the audio and points here.
- **The wording on screen should change.** The current design note says Twitch
  video "cannot decode in an official CEF build" and calls it "a PATENT
  licensing limit". That is true but incomplete: the video half is solvable
  without any licence, and it is the **audio** that is stuck. A person reading
  the current wording would conclude the whole thing is legally impossible,
  which is not what the sources say.

---

*Written 19 September 2026. Not legal advice. If this document is more than a
year old when you read it, re-check the three moving targets: the US patent
expiry dates in Finding 2, the state of CEF issue #3559 in Finding 3, and Via
LA's published rates in Finding 4.*
