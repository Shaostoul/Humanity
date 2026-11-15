#!/usr/bin/env bash
# Make the project's APK signing key, ONCE, and hand it to GitHub as two repository secrets
# (Android rung A2, 2026-11-14; android/README.md). The operator's step: `just android-signing-key`.
#
# WHY A KEY OF OUR OWN. Android installs a new APK over an old one only when both are signed with
# the same key, and uninstalling to get past that deletes the app's files (the identity included).
# So every APK the project publishes is signed with one key that never changes: this one. Lose it
# and no later APK can update the installed app; keep the backup this makes somewhere safe.
#
# WHAT IT DOES (openssl from Git for Windows, gh already signed in):
#   1. Refuses if a backup already exists: a second key would orphan every installed copy.
#   2. Makes an RSA 4096 key and a self-signed certificate valid for 30 years (Android checks only
#      that the key matches, not who signed the certificate).
#   3. Sets ANDROID_SIGNING_KEY_B64 and ANDROID_SIGNING_CERT_B64 on the repository through
#      `gh secret set`, reading each from standard input so the key never appears on a command line.
#   4. Writes a PASSPHRASE-ENCRYPTED backup of the key (openssl asks for the passphrase twice) and
#      the certificate to the backup folder, then deletes the plain key. GitHub holds the only plain
#      copy, encrypted at rest on its side, readable only by the workflow.
#
# The backup folder: $HUMANITY_ANDROID_KEY_DIR, else %APPDATA%\HumanityOS-release-keys\android
# (outside the repository, never committed). Copy it to the same safe place as the release
# signing vault.
set -euo pipefail

REPO="${HUMANITY_REPO:-Shaostoul/Humanity}"
DIR="${HUMANITY_ANDROID_KEY_DIR:-${APPDATA:-$HOME}/HumanityOS-release-keys/android}"

command -v openssl >/dev/null || { echo "openssl was not found (it comes with Git for Windows)"; exit 1; }
command -v gh >/dev/null || { echo "gh was not found (GitHub CLI)"; exit 1; }
if [ -e "$DIR/key-backup.p8" ]; then
  echo "A key already exists: $DIR/key-backup.p8"
  echo "Making a second one would stop every installed copy from updating. Nothing was changed."
  exit 1
fi

mkdir -p "$DIR"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

openssl req -x509 -newkey rsa:4096 -nodes -days 10950 -subj "/CN=HumanityOS/O=HumanityOS" \
  -keyout "$WORK/key.pem" -out "$DIR/cert.pem"
openssl pkcs8 -topk8 -nocrypt -inform PEM -outform DER -in "$WORK/key.pem" -out "$WORK/key.pk8"

base64 -w0 "$WORK/key.pk8" | gh secret set ANDROID_SIGNING_KEY_B64 --repo "$REPO"
base64 -w0 "$DIR/cert.pem" | gh secret set ANDROID_SIGNING_CERT_B64 --repo "$REPO"

echo "Now a passphrase for the backup of the key (asked twice):"
openssl pkcs8 -topk8 -v2 aes-256-cbc -v2prf hmacWithSHA256 -iter 600000 \
  -inform PEM -outform PEM -in "$WORK/key.pem" -out "$DIR/key-backup.p8"

echo
echo "Done. The APK signing key is set on $REPO (ANDROID_SIGNING_KEY_B64, ANDROID_SIGNING_CERT_B64)."
echo "Backup (encrypted key + certificate): $DIR"
echo "Copy that folder to a safe place: without it no later APK can update an installed copy."
echo "The certificate's fingerprint (an APK signed with this key shows the same):"
openssl x509 -in "$DIR/cert.pem" -noout -fingerprint -sha256
