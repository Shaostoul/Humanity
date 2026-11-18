# Listing HumanityOS on AlternativeTo: its rules, read 2026-11-17

The operator submitted HumanityOS to AlternativeTo and asked whether it will be accepted under the
site's rules (https://alternativeto.net/faq/#why-wasn-t-my-software-app-approved). Read in the
browser on 2026-11-17 (robots.txt allows /faq/). Paraphrased below; check the page again before
relying on it, since the site says it recently tightened its criteria.

## The rules that matter for HumanityOS

| Rule (paraphrased) | HumanityOS | Risk |
|---|---|---|
| English only | Yes | none |
| Nothing unreleased: no announced-only, closed beta, invite-only or early-access apps; open or public betas are probably fine | Public downloads for Windows, macOS and Linux, and a live web app, free to everyone | Low, unless the listing or the site reads as early access (the app's "work in progress" notice, "not fully playable" game) |
| Content, simple online tools, or a bundle of simple tools are declined | A full app (identity, sealed chat, servers, maps, game engine), but it also holds calculators and converters | Medium if the listing leads with the tools |
| Games are held to a higher bar | The game world is not fully playable yet | High if listed as a game: do not lead with it |
| An app is an alternative to another when its main task and focus are the same | Many halves, so the reviewer needs ONE main focus | Medium: a sprawling "does everything" listing is hard to place |
| No URLs, emails or phone numbers in the description | The website goes in the Official website field | none if followed |
| Not regional | Global | none |
| Indistinguishable, low-effort or over-saturated categories may be declined, at the admins' discretion | Chat alternatives are a busy category | some, and out of our hands |
| User profiles may not advertise a product (accounts doing so are blocked as spam) | The profile bio | keep the bio about the person, with no pitch and no link |

## Timing

The free queue holds several thousand apps and most are not reviewed for a very long time, with no
date given. A one-time fee moves an app to the front (usually 1 to 2 business days) but does not buy
approval; some power users' submissions are moved up free. Once approved, the developer can ask for
developer access so their edits go live at once. Per the operator's standing guidance, no purchase is
proposed as a fix.

## How to present it (if the submission can still be edited, or for any resubmission)

- One main focus: private, self-hostable community chat with no sign-up (sealed direct messages,
  your own server on your own network, federation), so it is placed as an alternative to the
  community-chat apps. The everyday tools and the game follow in the description as what else it
  does, never the lead.
- Present it as released software (it is: public downloads and a live web app), license
  open source, free, platforms Windows, macOS, Linux, Web, Self-Hosted.
- No links in the description; screenshots of the chat and the app, not the unfinished game.

## The listing as filled in (2026-11-17; reusable for other directories)

**Short description:** Free, open-source community chat with no sign-up and end-to-end encrypted
messages, plus everyday tools, offline guides and a game that teaches self-sufficiency.

**Full description:**

HumanityOS is a free, open-source app for private community chat and everyday planning, with no
sign-up. Your account is a key on your own device, backed up by a 24-word recovery phrase: no email,
no phone number, and no password stored on any server.

The chat works like a community server: channels, voice and video calls, screen sharing, files sent
straight between devices, and private messages sealed end to end with post-quantum encryption.
Anyone can run their own server, on a rented machine or on a home computer for the devices on its
own network, and servers connect to each other, so no single company holds your community.

Alongside the chat are everyday tools: a task board, maps that zoom from the galaxy down to your own
street, a marketplace for trading directly with others, a music generator, and a library of
practical guides on growing food, safe water, shelter and power that opens with no connection. The
same app holds a space game, in active development, that teaches those self-sufficiency skills
through simulation.

No ads, nothing sold about you, and no company owns it. Desktop downloads for Windows, macOS and
Linux, and a web version that runs in any browser.

**Other fields:** platforms Windows, Mac, Linux, Online, Self-Hosted (no Android: the build exists
only on GitHub's build machines); licence AGPL-3.0; release date 2026-01-16 (the repository's first
public commit; HumanityOS has carried that name since 1 January 2026); icon
`web/shared/icons/icon-512.png` (served at https://united-humanity.us/shared/icons/icon-512.png);
no package managers yet (a winget manifest, a pull request to microsoft/winget-pkgs, is a cheap
later add).
