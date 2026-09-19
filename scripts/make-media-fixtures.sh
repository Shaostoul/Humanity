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

# 4. The VIDEO_TS fixture (2026-09-18): a whole unencrypted video disc, the
#    kind a person burns themselves, so the disc path is tested with no disc
#    in the drive. Shape a real disc has:
#
#      VTS_01_0.VOB  the title set's MENU   (never played as the film)
#      VTS_01_1.VOB  the main title, part 1 |  one continuous program stream
#      VTS_01_2.VOB  the main title, part 2 |  split at a 2048 byte pack
#      VTS_02_1.VOB  a second, shorter title set (an extra, a trailer)
#
#    The main title is made as ONE MPEG-2 program stream and then SPLIT ON A
#    PACK BOUNDARY, which is exactly what a DVD author does at the 1 GB VOB
#    limit: the timestamps run on across the cut, so the two parts really do
#    convert as one film. (Two separately encoded files would each restart at
#    zero and ffmpeg would drop the second, which would make the test pass
#    for the wrong reason.) A cyan 48 px square slides and bobs so two
#    snapshots a second apart must differ; a 440 Hz tone rides along.
#    MPEG-2 video and AC-3 audio are what a video disc carries; both are
#    read by the machine's own ffmpeg and converted once, exactly like the
#    MP4 above. Whole tree: about 65 KB.
DVD="$OUT/VIDEO_TS"
mkdir -p "$DVD"
TMP_TITLE="$OUT/.disc-title.tmp"
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=0x102030:s=320x180:r=30:d=2[bg];color=c=0x00c0ff:s=48x48:r=30:d=2[box];[bg][box]overlay=x='mod(t*130\,272)':y='abs(60*sin(t*2.5))':shortest=1" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
  -ac 2 -c:v mpeg2video -b:v 300k -g 15 -pix_fmt yuv420p \
  -c:a ac3 -b:a 96k -t 2 -f vob "$TMP_TITLE"
node -e '
  const fs = require("fs");
  const [src, a, b] = process.argv.slice(1);
  const buf = fs.readFileSync(src);
  // Every pack of a VOB is 2048 bytes and starts with the pack start code
  // 00 00 01 BA. Refuse to split anywhere else: a cut inside a pack would
  // make a fixture no player could read, and the test would then be proving
  // nothing about our own code.
  for (let o = 0; o < buf.length; o += 2048) {
    if (buf.readUInt32BE(o) !== 0x1ba) throw new Error(`not a pack at byte ${o}`);
  }
  const half = Math.floor(buf.length / 2 / 2048) * 2048;
  if (half === 0 || half === buf.length) throw new Error("too short to split");
  fs.writeFileSync(a, buf.subarray(0, half));
  fs.writeFileSync(b, buf.subarray(half));
' "$TMP_TITLE" "$DVD/VTS_01_1.VOB" "$DVD/VTS_01_2.VOB"
rm -f "$TMP_TITLE"

# The title set's menu, which the main-title chooser must IGNORE: a still
# green field, deliberately different from the film so a snapshot that shows
# it is unmistakable.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=0x104020:s=320x180:r=30:d=0.4" \
  -f lavfi -i "sine=frequency=220:sample_rate=48000:duration=0.4" \
  -ac 2 -c:v mpeg2video -b:v 150k -g 15 -pix_fmt yuv420p \
  -c:a ac3 -b:a 96k -t 0.4 -f vob "$DVD/VTS_01_0.VOB"

# A second title set, shorter than the first, so "pick the biggest title
# set" has something to be right about.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=0x402010:s=320x180:r=30:d=0.4" \
  -f lavfi -i "sine=frequency=660:sample_rate=48000:duration=0.4" \
  -ac 2 -c:v mpeg2video -b:v 150k -g 15 -pix_fmt yuv420p \
  -c:a ac3 -b:a 96k -t 0.4 -f vob "$DVD/VTS_02_1.VOB"

ls -l "$OUT" "$DVD"
