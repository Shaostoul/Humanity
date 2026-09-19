# Our stance on playing media you own

Status: adopted 2026-09-18 by the project operator. This page is the public
record of what HumanityOS does with video, why, and what happens if anyone
objects. It is written for three readers at once: a person who just wants to
watch something, a contributor deciding what to build, and a lawyer deciding
whether to send a letter.

## What we believe

You should be able to watch a film you own, on a screen you own, in software
you own, without asking anyone. That is not a fringe position. It is what
almost everyone assumes is already true, and for a disc sitting in your own
drive it very nearly is.

The operator's words, which started this: "It seems silly to ship a game with
all the video viewing options and then someone like me with thousands of DVDs
can't watch my own movies on my own free open source software."

## What the app does

- **Plays video files.** Any file on your machine, chosen from inside the app,
  shown on any screen in the world. A format our player cannot decode is
  converted once with the ffmpeg already installed on your machine, cached, and
  played. Your original file is never modified.
- **Plays discs it can read.** A disc you burned, a camcorder disc, a data disc,
  an unprotected video disc: the app finds the main title and plays it.
- **Uses what your machine already has.** If your computer already carries a
  library that can read a protected disc, the app will use it, in the same way
  it uses the ffmpeg you already installed. This is the same posture HandBrake
  has held for years.
- **Says no plainly.** A disc the app cannot read produces a sentence telling
  you so, not a black rectangle and not a cryptic error.

## What the app does not do

- **We ship no disc decryption.** Not in the download, not in the repository,
  not fetched at runtime, not built by our scripts.
- **We do not tell you where to get any.** We do not link to it, name a
  supplier, or write an installation guide. That restraint is deliberate and it
  is the line between a player that uses what is on your machine and a
  distribution channel for something else. Publishing a procurement guide is the
  single thing most likely to bring a complaint down on the repository that
  hosts every release of this project, and it would help almost nobody, because
  anyone whose machine can already do this got that capability with an ordinary
  media player years ago.
- **We do not strip advertising.** Where the app shows a video platform, it
  shows that platform's own player with its own advertising intact. That is the
  condition under which embedding is permitted, and we keep it.
- **We do not pull streams out of platforms.** No extraction, no ripping of
  anything served to us by somebody else's service, whatever their terms of
  business with us.

## What each platform can actually do

Say this plainly so nobody meets a wall we did not warn them about. Most of
this is not about law at all. It is about what the machine physically has and
what its owner permits an app to touch.

| Where you are | Video files | A disc in the drive | A protected disc |
|---|---|---|---|
| Windows, macOS, Linux (the downloaded app) | Yes, any file, converted once if needed | Yes, when the disc can be read as files | Only if your own machine already carries that capability |
| iPhone, iPad, Android (the installed web app) | See below | No | No |
| Game consoles | No build exists | No | No |

**The downloaded app** is where the 3D world and its in-world screens live.
Everything on this page describes that app.

**Phones and tablets** run HumanityOS as an installed web app: the pages, not
the world. There is no 3D room to hang a screen in, so the question of playing
a film on an in-world display does not arise there yet. What a browser can
play, it plays. A browser cannot reach an optical drive, cannot run ffmpeg and
cannot load a system library, so the conversion and disc features are desktop
features and will stay that way until there is a native mobile build. When
there is one, a phone still has no disc drive.

**Consoles.** We do not build for Xbox or PlayStation, and we do not promise
to. If we ever did, a disc in the tray would still be unavailable to us: those
machines play their own discs through their own certified player and expose no
optical drive to a third-party app. That is the platform holder's rule, not
ours and not a law.

**Nothing here is withheld from you as a punishment or an upsell.** Where a
capability is missing it is missing because the hardware lacks it, the
platform forbids it, or we have said above that we will not build it. The app
says which one on the screen, in a sentence, at the moment you hit it.

## Why the line sits there

Copyright law in most places says you may watch what you bought. A separate
body of law, written in the late 1990s, says that undoing a technical
protection is an offence in itself, with no exception for owning the thing.
Those two rules disagree, and the second one is the one with the enforcement
history.

We are not positioned to test it. This project distributes its releases from a
public repository under a named person, and takes donations. The realistic
risk is not a lawsuit, it is a notice to the host that removes every release we
have ever published. So the code we distribute contains nothing whose purpose
is to break a protection, and what your own computer can do is your own
business, exactly as it is with every other program you run.

## If someone objects

This feature is small and it is not load bearing. If a rights holder, a host,
or a lawyer tells us this crosses a line, we will turn it off, and say publicly
that we did and why. Nothing else in HumanityOS depends on it. We would rather
be told and correct course than argue.

Contact: the operator, through the project's public channels.

## Where this lives in the code

- `src/media/` is the player and the conversion (`transcode.rs`).
- `src/engine/screens/video.rs` is the in-world screen and its controls.
- `data/media/ingest.json` is the data file naming what the picker lists and
  where ffmpeg is looked for.
- The design behind them: [media-player.md](../design/media-player.md) and
  [in-world-screens.md](../design/in-world-screens.md).
- The per-site record for web embedding is `data/web/sites.json`, whose
  `embed.status` decides what an in-world screen may load.
