# HumanityOS on Android

The phone app, rung A1 of [docs/design/platforms.md](../docs/design/platforms.md): the app's pages
(Chat, Settings and the rest) on a phone. The 3D world is a later rung (A5) with its own lighter
settings; on a phone the Play page says so in place of its Enter button.

The operator, 2026-11-14: "It would be nice to have the native app on my one plus nine pro instead
of the PWA version." and, on cost: "Let's aim for free."

## Install it on your phone

You install the APK file yourself ("sideloading"). Google Play is not used (the reasons and the
costs: [the distribution finding](../docs/reference/findings/2026-11-14-app-distribution-costs.md)).

1. **Get the APK.** On your phone, open the newest release on GitHub
   (`https://github.com/Shaostoul/Humanity/releases`) and tap `HumanityOS-android-arm64.apk`. Until
   a release carries one, a build from the **Build Android App** workflow is a zip in that run's
   Artifacts section (`humanity-android-arm64`), which needs a GitHub sign-in to download; unzip it
   on the phone and use the `.apk` inside.
2. **Allow your browser (or Files app) to install apps.** The first time, Android says "For your
   security, your phone currently isn't allowed to install unknown apps from this source." Tap
   **Settings**, turn on **Allow from this source**, and go back. (On a OnePlus phone the same
   switch is under Settings > Apps > Special app access > Install unknown apps, then the browser
   or Files app you used.) You can turn it off again after installing.
3. **Install.** Tap **Install**. Google Play Protect may say it does not recognise the developer
   and offer to scan the app; scanning is fine, and **Install anyway** (under "More details" on
   some versions) finishes it. It warns because the app is not from Google Play, not because
   anything was found.
4. **Open HumanityOS** from the app drawer. The first start writes the app's files into its own
   private folder and takes a little longer than the ones after it.

**Updating.** Install the newer APK the same way; it replaces the old one and keeps your files, as
long as both were signed with the project's key. An APK whose name ends in `-test-signed.apk` was
built before that key existed: a later APK will not install over it, and uninstalling it deletes
the app's files, your identity included. Before uninstalling a test-signed copy, write down your
recovery phrase (Settings, Account: Reveal recovery phrase) so you can restore your identity afterwards.

**What a phone does not have yet**, each said on the screen where you meet it:

| What | Why | What works instead |
|---|---|---|
| The 3D world | Needs lighter graphics settings and a test on a phone's graphics chip first (rung A5) | The pages; the world in the desktop app |
| Remembering your key (Settings, Unlock on App Launch) | Needs the Android Keystore, not built in yet | Your passphrase each time |
| Launching programs (Apps page) | Android lets an app start only its own code (a platform holder's rule, Google's) | Open them on the phone yourself |
| Chat while the app is closed | Android stops background apps (rung A4) | Open the app to catch up |
| Pasting a picture from the clipboard | The clipboard library has no Android code | Attach the picture as a file |

## How the APK is built

Free, on GitHub's runners, by [.github/workflows/build-android.yml](../.github/workflows/build-android.yml),
which runs [build-apk.sh](build-apk.sh). Nothing is installed on anyone's PC.

1. **The library.** The app's library (the `humanity-engine` package, the same code as the desktop)
   is built for `aarch64-linux-android` as a shared object, `libhumanity_engine.so`, with
   `cargo ndk -t arm64-v8a -P 26 rustc --lib --release --crate-type cdylib`. cargo-ndk points cargo
   at the NDK's compilers and linker; `--crate-type cdylib` makes the shared object without making
   every desktop build link one too.
2. **The package.** Google's own SDK tools from the runner: `aapt2` makes the APK from
   [AndroidManifest.xml](AndroidManifest.xml) and the app's icon, `zip` adds the library,
   `zipalign` aligns it and `apksigner` signs it.
3. **NativeActivity.** The manifest names Android's own `NativeActivity`, which loads the library and
   calls its `android_main` ([src/android.rs](../src/android.rs)). So the APK holds no Java code and
   needs no Gradle. winit's `android-native-activity` feature supplies the glue.

Why these tools and not cargo-apk or xbuild, with their licences quoted:
[docs/reference/research/2026-11-14-android-build-tools/](../docs/reference/research/2026-11-14-android-build-tools/README.md).

**Signing.** The workflow signs with the project's key when the repository secrets
`ANDROID_SIGNING_KEY_B64` and `ANDROID_SIGNING_CERT_B64` are set; the operator makes them once with
`just android-signing-key` ([scripts/android-signing-key.sh](../scripts/android-signing-key.sh)),
which also keeps a passphrase-encrypted backup outside the repository. Without them the APK is
test-signed and named so, and a release does not publish it.

## What the code does differently on a phone

- **Entry and folders** ([src/android.rs](../src/android.rs)): HOME and TMPDIR point at the app's
  private folder before anything runs, so the storage root, the logs and the encrypted identity all
  live inside it; storage is always the per-user ("Installed") mode, and the first start writes the
  data files compiled into the app there. stdout and stderr go to logcat under the tag `HumanityOS`.
- **Leaving and coming back** ([src/engine/app_lifecycle.rs](../src/engine/app_lifecycle.rs)): the
  drawing surface is dropped when the app leaves the screen and made again when it returns; while
  away the app saves, draws nothing and reads its sockets a few times a second.
- **What the phone lacks** ([crates/humanity-core/src/platform/device.rs](../crates/humanity-core/src/platform/device.rs)):
  one list of capabilities and the words for each missing one (`device.*` in `data/i18n/en.json`).
  The tray icon, the clipboard picture library and the key store's crate are left out of the
  phone build in `Cargo.toml`; the chat socket uses rustls there instead of OpenSSL.
- **Touch**: on a phone every control is at least the theme's `touch_target` high (44 points by
  default, Settings > Widgets), and egui lays the pages out at the phone's own scale (the
  operator's phone is about 412 points wide).

**The phone preview on a desktop.** `HUMANITY_PHONE_PREVIEW=1` at start shows the pages as a phone
shows them (the notices, the touch sizes, no world) on a desktop, for checking a layout without a
phone. Make the window narrow to see the phone's width.

## What only a phone can prove

The workflow proves the library compiles and links for Android and that the APK is well formed and
signed. Whether it starts, draws, takes touch and the on-screen keyboard, survives leaving and
coming back, and how long the first start takes, only the phone shows. Reading its log needs a PC
with `adb` (`adb logcat -s HumanityOS`); without one, the app's own logs are inside its private
folder.
