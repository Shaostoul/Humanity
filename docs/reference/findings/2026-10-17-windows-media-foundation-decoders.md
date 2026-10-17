# Can the desktop app play an ordinary MP4 (H.264 video, AAC sound) by asking Windows?

**Research date: 17 October 2026.** Both sources below were read on that date. Each carries its
own last-updated date, given beside it. If you are reading this later, compare those dates with
whatever has changed since.

**I am not a lawyer. This is a reading of public sources, not legal advice.** It adds one
technical fact to [2026-09-19-video-codec-licensing.md](2026-09-19-video-codec-licensing.md),
which holds the legal reading, and changes none of its conclusions.

## The question

The operator asked, after v0.1509.0 said a chat video "plays in your computer's own player": "How
do we give users the ability to play MP4s in our app/website?" The September finding judged the
route to be the operating system's own decoders, and found one gap: the browser engine it measured
had no path to the platform's AAC decoder, so that route gave a picture with no sound. Is that gap
real for our own player, which would call Windows directly rather than through a browser engine?

## The short answer

**No gap on Windows.** Windows 7 and later carry, as part of Media Foundation, both an H.264 video
decoder and an AAC audio decoder that any desktop application may load. The September finding's
missing sound was a limit of the browser engine, not of Windows. A player that calls Media
Foundation itself gets the picture and the sound, and the H.264 decoder falls back to software when
the graphics hardware cannot help, so old or blocklisted graphics cards are not left out.

The legal reading is unchanged and still the September one: we would ship no decoder and call the
one the person's own copy of Windows already has, the posture `docs/reference/media-stance.md`
already takes toward the machine's own ffmpeg. Microsoft has still not said in plain words that
developers may do this; the September finding names that as the open question.

## Finding 1: Windows has an H.264 decoder for applications

Source: Microsoft Learn, "H.264 Video Decoder" (Media Foundation),
<https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder>, page updated
2024-06-20.

- It decodes, in its own words, "Baseline, Main, and High profiles, up to level 5.1". That covers
  the H.264 a phone, a camera or a video website produces.
- Minimum supported client: Windows 7, desktop applications.
- Above 1920 by 1088 the page says decoding uses the graphics hardware where it can and software
  otherwise; at and below it, hardware acceleration is optional (an attribute turns it on or off).
  So a machine without suitable graphics hardware still decodes, in software.
- Created by `CLSID_CMSH264DecoderMFT`, or reached through Media Foundation's Source Reader, which
  also opens the MP4 file itself (the "MPEG-4 File Source").

## Finding 2: Windows has an AAC decoder for applications

Source: Microsoft Learn, "AAC Decoder" (Media Foundation),
<https://learn.microsoft.com/en-us/windows/win32/medfound/aac-decoder>, page updated 2023-08-18.

- It decodes AAC-LC (up to six channels) and HE-AAC v1 and v2, the sound in an ordinary MP4.
- Minimum supported client: Windows 7, desktop applications.
- Created by `CLSID_CMSAACDecMFT`; the MPEG-4 File Source hands it the sound track.
- Not supported, per the page: AAC Main, SRS and LTP profiles, sample rates above 48 kHz. Rare in
  everyday files.

## What is still unknown

- **Windows "N" editions** (sold in Europe and Korea without media features) need Microsoft's free
  Media Feature Pack before these decoders exist. The player must say so plainly when they are
  missing, not fail silently. Not checked against a Microsoft page on this date.
- **H.265 (HEVC)**, common from newer phones, is not in Windows by default; Microsoft sells it as
  an extension in its Store. Where it is installed, Media Foundation offers it the same way. Where
  it is not, the honest sentence names the patent reason and the extension.
- **macOS and Linux.** macOS has AVFoundation and VideoToolbox (the September finding covers the
  video half there); Linux has no single system decoder, and the existing route (the person's own
  ffmpeg, converted once) stays.
- **The legal question itself** is the September finding's, unchanged.

## What this means for HumanityOS

- **The website already plays MP4.** A browser carries its own licensed decoders; the chat's file
  card (v0.1509.0) keeps the player and says when a browser cannot play a file.
- **The desktop app can play MP4 with sound on Windows by calling Media Foundation.** That is the
  build to do next: a Media Foundation path in the media player (open with the Source Reader, take
  decoded frames and decoded sound into the existing player), used for any file the app's own AV1
  and Opus path cannot play, on Windows. Nothing is shipped or downloaded; the same file works in
  the chat's card, the Media page and the in-world screens.
- **Until then** the existing route stays: a file is converted once with the ffmpeg the person
  already installed, where they have it.
