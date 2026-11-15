#!/usr/bin/env bash
# Build the phone app's APK (Android rung A1, 2026-11-14; android/README.md, docs/design/platforms.md).
#
# Runs on GitHub's ubuntu runners, which carry the Android SDK and NDK (ANDROID_HOME and
# ANDROID_NDK_HOME are set there): .github/workflows/build-android.yml. Nothing here downloads an
# SDK; the workflow installs the Rust target and cargo-ndk before calling this.
#
# The route, compared in docs/reference/research/2026-11-14-android-build-tools/README.md:
#   1. cargo-ndk sets up the NDK's compilers and linker for the target and runs
#      `cargo rustc --lib --crate-type cdylib`, so the app's library is built as the shared object
#      NativeActivity loads (libhumanity_engine.so). The desktop's builds never make a cdylib.
#   2. Google's own SDK tools package it: aapt2 makes the APK from android/AndroidManifest.xml and
#      the icon, zip adds the library, zipalign aligns it, apksigner signs it.
#
# Signing: with ANDROID_SIGNING_KEY_B64 and ANDROID_SIGNING_CERT_B64 set (the project's key, kept
# as two repository secrets; `just android-signing-key` makes them), every build is signed with
# that key, so a new APK installs over the last one and keeps the app's files. Without them a key
# is made for this one build and the file is named ...-test-signed.apk: installing a later build
# over it is refused, and uninstalling it deletes the app's files, the identity included.
#
# Usage: android/build-apk.sh [out-dir]     (default: dist)
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="${1:-dist}"
: "${ANDROID_HOME:?ANDROID_HOME is not set (GitHub's ubuntu runners set it)}"
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME is not set (GitHub's ubuntu runners set it)}"

MIN_SDK=26          # Android 8.0: AAudio (the sound, through cpal) needs it
TARGET_SDK=35
ABI=arm64-v8a       # every phone of the last several years, the operator's OnePlus 9 Pro included
TRIPLE=aarch64-linux-android
NAME=HumanityOS-android-arm64

# The newest build tools and the newest stable platform the runner has (a beta platform such as
# android-37.2-beta3 has letters after its number and is passed over).
BT="$(ls -d "$ANDROID_HOME"/build-tools/*/ | sort -V | tail -1)"
BT="${BT%/}"
PLATFORM="$(ls -d "$ANDROID_HOME"/platforms/android-*/ | grep -E '/android-[0-9]+/$' | sort -V | tail -1)"
PLATFORM="${PLATFORM%/}"
echo "build tools: $BT"
echo "platform:    $PLATFORM"
echo "NDK:         $ANDROID_NDK_HOME"

# The version: the workspace's (Cargo.toml), and a version code that grows with it, which Android
# needs to install a new APK over an old one: 0.1561.2 is 1561002.
VERSION="$(tr -d '\r' < Cargo.toml | sed -n 's/^version = "\([0-9.]*\)"$/\1/p' | head -1)"
IFS=. read -r V_MAJOR V_MINOR V_PATCH <<<"$VERSION"
CODE=$(( V_MAJOR * 100000000 + V_MINOR * 1000 + V_PATCH ))
echo "version:     $VERSION (code $CODE)"

# 1. The library. 16 KB pages: Android 15 phones may use them, and a library whose segments are
#    aligned to 4 KB does not load there.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
cargo ndk -t "$ABI" -P "$MIN_SDK" rustc --lib --release --crate-type cdylib
LIB="target/$TRIPLE/release/libhumanity_engine.so"
test -f "$LIB" || { echo "no $LIB was built" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/lib/$ABI" "$WORK/res/mipmap-xxxhdpi" "$OUT"
"$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" --strip-debug -o "$WORK/lib/$ABI/libhumanity_engine.so" "$LIB"
ls -l "$LIB" "$WORK/lib/$ABI/libhumanity_engine.so"

# 2. The app's files for its pages (scripts/lib/release-files.js --phone-list: the release's files
#    less what only the 3D world reads), as APK assets under files/, with their list in files.txt.
#    The first start copies them into the app's own folder (src/android.rs, unpack_apk_files).
mkdir -p "$WORK/assets/files"
node scripts/lib/release-files.js --phone-list > "$WORK/assets/files.txt"
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  mkdir -p "$WORK/assets/files/$(dirname "$rel")"
  cp "$rel" "$WORK/assets/files/$rel"
done < "$WORK/assets/files.txt"
node scripts/lib/release-files.js --phone-summary

# 3. The APK: the manifest, the icon (the window's icon, assets/icon.png) and the files, then the
#    library.
cp assets/icon.png "$WORK/res/mipmap-xxxhdpi/ic_launcher.png"
"$BT/aapt2" compile --dir "$WORK/res" -o "$WORK/res.zip"
"$BT/aapt2" link -o "$WORK/unaligned.apk" \
  -I "$PLATFORM/android.jar" \
  --manifest android/AndroidManifest.xml \
  --min-sdk-version "$MIN_SDK" --target-sdk-version "$TARGET_SDK" \
  --version-code "$CODE" --version-name "$VERSION" \
  -A "$WORK/assets" \
  "$WORK/res.zip"
(cd "$WORK" && zip -q -r unaligned.apk "lib/$ABI/libhumanity_engine.so")
"$BT/zipalign" -p -f 4 "$WORK/unaligned.apk" "$WORK/aligned.apk"

# 4. Signed: the project's key when the secrets are there, else a key for this build only.
if [ -n "${ANDROID_SIGNING_KEY_B64:-}" ] && [ -n "${ANDROID_SIGNING_CERT_B64:-}" ]; then
  printf '%s' "$ANDROID_SIGNING_KEY_B64" | base64 -d > "$WORK/key.pk8"
  printf '%s' "$ANDROID_SIGNING_CERT_B64" | base64 -d > "$WORK/cert.pem"
  APK="$OUT/$NAME.apk"
  echo "signing with the project's key"
else
  openssl req -x509 -newkey rsa:4096 -nodes -days 3650 -subj "/CN=HumanityOS test build" \
    -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null
  openssl pkcs8 -topk8 -nocrypt -inform PEM -outform DER -in "$WORK/key.pem" -out "$WORK/key.pk8"
  APK="$OUT/$NAME-test-signed.apk"
  echo "::warning::No signing secrets: $APK is signed with a key made for this build only. A later build will not install over it, and uninstalling it deletes the app's files."
fi
"$BT/apksigner" sign --key "$WORK/key.pk8" --cert "$WORK/cert.pem" --out "$APK" "$WORK/aligned.apk"
"$BT/apksigner" verify --verbose --print-certs "$APK"
( cd "$OUT" && sha256sum "$(basename "$APK")" > "$(basename "$APK").sha256" )
ls -l "$OUT"
echo "APK=$APK" >> "${GITHUB_OUTPUT:-/dev/null}"
