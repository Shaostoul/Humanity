#!/usr/bin/env bash
# HumanityOS media test fixtures (design: docs/design/media-player.md).
#
# Generates the small WebM files under tests/fixtures/media/ from SYNTHETIC
# lavfi sources with the machine's ffmpeg. Nothing is downloaded and nothing
# is recorded: every pixel is drawn and every sample is synthesised by the
# script, so the fixtures carry no third-party rights. Re-run this after
# changing a recipe and commit the result (the files are binary, pinned as
# such in .gitattributes).
#
# Needs an ffmpeg with libaom (AV1), libopus, libvpx and libvorbis. The
# gyan.dev "full" Windows build and most Linux distro builds have all four.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/tests/fixtures/media"
mkdir -p "$OUT"
command -v ffmpeg >/dev/null || { echo "ffmpeg not found on PATH" >&2; exit 1; }

# 1. The player fixture: 2 s, 320x180, 30 fps, AV1 (libaom) + stereo Opus.
#    A 40 px red bar slides right at 140 px/s over a dark grey field, so a test
#    can check the decoded PICTURE (bar left edge = 140 * pts pixels), not just
#    the frame count. The overlay filter re-evaluates x every frame; drawbox
#    does not, which is why the bar is a second source rather than a box.
#    Audio is a 440 Hz sine so the sample count and level can be checked.
#    Keyframe every 30 frames (two in the clip) keeps it small: about 20 KB.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=0x202020:s=320x180:r=30:d=2[bg];color=c=red:s=40x180:r=30:d=2[bar];[bg][bar]overlay=x='mod(t*140\,320)':y=0:shortest=1" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
  -ac 2 -c:v libaom-av1 -cpu-used 6 -crf 32 -g 30 -pix_fmt yuv420p \
  -c:a libopus -b:a 48k -t 2 \
  "$OUT/colour-bar-av1-opus.webm"

# 2. A refusal fixture: 0.2 s of VP8 + Vorbis, the codecs WebM carried before
#    AV1 and Opus. The player must refuse it and name the codec (V_VP8).
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=blue:s=64x36:r=10:d=0.2" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=0.2" \
  -c:v libvpx -b:v 50k -c:a libvorbis -t 0.2 \
  "$OUT/unsupported-vp8-vorbis.webm"

# 3. The transcode-on-ingest fixture (2026-09-18): 2 s, 320x180, 30 fps of
#    H.264 + AAC in an MP4, the format a phone or a screen recorder writes
#    and the player refuses (docs/design/media-player.md, the codec policy).
#    An orange 48 px square slides right and bobs on a sine so a frozen
#    frame is detectable (two snapshots half a second apart must differ),
#    over a 440 Hz tone so silence is detectable. About 23 KB. Needs libx264
#    and the built-in aac encoder (every ffmpeg build has both).
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=0x203040:s=320x180:r=30:d=2[bg];color=c=orange:s=48x48:r=30:d=2[box];[bg][box]overlay=x='mod(t*120\,272)':y='abs(66*sin(t*3))':shortest=1" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
  -ac 2 -c:v libx264 -preset veryfast -crf 26 -g 30 -pix_fmt yuv420p \
  -c:a aac -b:a 64k -movflags +faststart -t 2 \
  "$OUT/moving-box-h264-aac.mp4"

ls -l "$OUT"
