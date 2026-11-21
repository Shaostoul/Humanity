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
| Launching programs (Apps page) | Android lets an app start only its own code (a platform holder's rule, Google's) | Open them on the phone yourself |
| Opening a conversation from a notice | A tap opens the app, not yet the conversation | Open the conversation yourself |
| Pasting a picture from the clipboard | The clipboard library has no Android code | Attach the picture as a file |

## Remembering your key

Settings, Account, **Unlock on App Launch** works on a phone (2026-11-20). **Remember on this
device** unlocks by itself when the app starts; **Quick PIN** asks for a short PIN instead of the
passphrase. Either way your passphrase still unlocks it. Choosing one way forgets what the other
remembered, and **Always ask for passphrase** forgets both. Should forgetting fail, Settings says
so, with **Try again**, and the app tries again each time it starts. Remembering again keeps the
old secret working until the new one is safely kept, so a remember that fails changes nothing.

How: Android's key store makes a key that cannot be copied off the phone and works only for
HumanityOS, and the secret the desktop's key store would keep (your key, or the PIN's device key) is
sealed with it into the app's private folder. On Android 9 and later, on a phone with a screen lock
(a PIN, pattern or password), Android also refuses to use that key while the phone is locked;
Settings says whether this phone's key carries that rule, since Android 8 has none and some phones
refuse it. The Quick PIN itself is checked by HumanityOS, not by Android
(each guess is slowed down on purpose). Settings says where this phone keeps that key: in a
separate security chip (StrongBox), in its secure hardware, or, on a phone without either, in
software, where Settings says your passphrase is the safer choice. Uninstalling the app forgets it,
and nothing of the app's folder goes into a backup or moves to a new phone: set a new phone up
from your recovery phrase. The detail: [docs/reference/cryptography.md](../docs/reference/cryptography.md),
"Remembering the key on a phone".

## Staying connected while closed

Android stops an app that is not on the screen, so by default a phone gets no messages or notices
while HumanityOS is in the background. Settings, Notifications, **Stay connected while HumanityOS
is closed** (off unless you turn it on) keeps it running: Android then shows a notification in the
bar for as long as it runs, and it uses some battery. Turning it on asks Android to allow notices
(Android 13 and later); if they are blocked, Settings says how to allow them.

- What a notice says is your choice in the same Settings page (**A notice shows**): who it is from
  by default, the message itself only if you choose **Who it is from and the message**.
- On a phone with a screen lock (a PIN, pattern or password), the locked screen shows nothing of a
  notice, unless you turn on **Show a line on the locked screen** (off unless you turn it on). A
  phone locked only by a swipe is not locked as far as Android is concerned, so its locked screen
  shows every notice whole whatever you choose. With the line on, a locked phone shows only "Something new
  arrived" if your phone's own setting hides sensitive content on the lock screen ("Hide sensitive
  content", or on newer Android "Show sensitive content when locked" turned off); if your phone
  shows sensitive content there, which is Android's default, the locked screen shows the whole
  notice. Android decides that, not HumanityOS.
- Swiping HumanityOS away in the recent-apps list closes it, notification and all.
- If the phone still closes it after a while (some phones, OnePlus among them, are strict), set
  HumanityOS's battery use to **Unrestricted** (or "Don't optimise") in Android's Settings, Apps,
  HumanityOS, Battery.

Why a notification in the bar and not a push service: [docs/design/platforms.md](../docs/design/platforms.md),
rung A4.

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
   calls its `android_main` ([src/android.rs](../src/android.rs)), so the app itself is Rust and
   needs no Gradle. winit's `android-native-activity` feature supplies the glue.

### The Java in the APK

Three small classes under [java/](java/us/united_humanity/humanityos/) (2026-11-20), compiled by
`build-apk.sh` with the runner's `javac` against the platform's `android.jar` and turned into
`classes.dex` by the SDK's `d8`; no Gradle, no libraries:

- `KeyStoreBridge`: Android's key store, for remembering your key.
- `ChatService`: the foreground service that keeps the app connected while closed.
- `Notices`: the phone's notices, and the request to allow them.

Why Java and not a chain of JNI calls from Rust: `javac` checks every Android class, method and
argument when the APK is built, where a JNI call names its method in a string that fails only on a
phone. The other direction, that each call Rust makes matches what the Java declares, is checked
on every PC by a test (`crates/humanity-core/src/platform/android_java.rs`), and the manifest's
permissions and service by `tests/android_manifest_lint.rs`.

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
  phone build in `Cargo.toml` (the key is remembered through Android's own key store instead,
  above); the chat socket uses rustls there instead of OpenSSL.
- **Touch**: on a phone every control is at least the theme's `touch_target` high (44 points by
  default, Settings > Widgets), and egui lays the pages out at the phone's own scale (the
  operator's phone is about 412 points wide).
- **A pages-only start** ([src/engine/renderer_slot.rs](../src/engine/renderer_slot.rs)): the phone
  makes only the GPU device and the window's surface; the 3D world's renderer (its pipelines, the
  cloud-noise bake, the ground textures, the star sky) is never built, since nothing on a phone
  enters the world.
- **Drawing only on change** ([src/engine/redraw.rs](../src/engine/redraw.rs)): a frame for a touch, a
  key, an animation or a page's timer, or a network message the pages show; otherwise the app sleeps.
  The run log says every ten seconds how many frames it drew (`[Redraw] reactive: ...`).

**The phone preview on a desktop.** `HUMANITY_PHONE_PREVIEW=1` at start shows the pages as a phone
shows them (the notices, the touch sizes, no world) on a desktop, for checking a layout without a
phone, and starts the way a phone starts: pages-only, drawing only on change. Make the window narrow
to see the phone's width. `HUMANITY_REDRAW_ALWAYS=1` beside it draws every frame again, for a
comparison; `HUMANITY_PAGES_FIRST=1` alone starts a desktop pages-only with the world still
enterable (built at the first Enter). `just verify-phone-start` boots all of these and judges them.

## What only a phone can prove

The workflow proves the library compiles and links for Android, that the Java compiles against
Android's own classes, and that the APK is well formed and signed. Whether it starts, draws, takes
touch and the on-screen keyboard, survives leaving and coming back, how long the first start takes,
whether remembering the key and staying connected work, only the phone shows. Reading its log needs a PC
with `adb` (`adb logcat -s HumanityOS`); without one, the app's own logs are inside its private
folder.
