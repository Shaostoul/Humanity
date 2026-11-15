# Building the Android APK: cargo-ndk, cargo-apk, xbuild (read 2026-11-14)

**Why this exists.** The phone app's rung A1 ([`../../../design/platforms.md`](../../../design/platforms.md),
Android) builds an APK on GitHub's free runners, and CLAUDE.md's rule (operator, 2026-11-08: "This,
research better alternatives, is something we want to consider every time we learn of new tools.")
asks for the open alternatives before one is adopted. The brief named three. Read the same day from
each project's own repository page on GitHub (robots.txt read first: repository main pages are not
disallowed for ClaudeBot or Claude-User) and the crates.io API (its robots.txt allows everything);
the licence lines below are quoted from those pages and from `cargo info`.

## The three

| | cargo-ndk | cargo-apk | xbuild |
|---|---|---|---|
| Publisher | bbqsrc (github.com/bbqsrc/cargo-ndk) | the rust-mobile organization | the rust-mobile organization |
| Licence (quoted) | "Apache-2.0 OR MIT" (crates.io); the repository: "Apache-2.0, MIT licenses found" | "MIT OR Apache-2.0" (crates.io); the repository: "Apache-2.0, MIT licenses found" | "Apache-2.0 OR MIT" |
| Latest release (crates.io) | 4.1.2, 2025-08-09 | 0.10.0, 2023-11-30 | 0.2.0, 2022-12-21 |
| Downloads, last 90 days (crates.io) | 1,059,470 | 31,895 | 564 |
| Maintenance | active; MSRV 1.86 | no release in two years; no deprecation notice on its page | its own description: "Cross compile rust to any platform (unmaintained)." |
| What it does | sets up the NDK's clang, ar and linker for a target and runs any cargo command with them; can lay out a `jniLibs` folder | builds a complete APK from a `cdylib` crate with `[package.metadata.android]`; "Ideal for apps that provide a NativeActivity" | builds and packages for many platforms and stores; its sample output fetches precompiled artifacts, and its `x doctor` checks for Gradle |
| Makes the APK | no (packaging is a separate step) | yes | yes |
| Needs | the NDK (`ANDROID_NDK_HOME`), the Rust target | the NDK and SDK | its own fetched pieces, plus tools it checks for |

## The choice: cargo-ndk for the library, Google's SDK tools for the package

- **cargo-ndk** is the maintained one by a wide margin, does one job (the cross-compiling
  environment), and passes any cargo command through: its argument parser hands every argument
  that is not its own (`-t`, `-P`, `-o`, `--manifest-path`, `--link-builtins`,
  `--link-libcxx-shared`) to cargo verbatim (read in its `src/cli/mod.rs`, `parse_mixed_args`). So
  `cargo ndk -t arm64-v8a -P 26 rustc --lib --release --crate-type cdylib` builds the app's library
  as the shared object Android loads WITHOUT changing the crate's own `crate-type`, which would
  make every desktop build link a second, huge library (and meet the Windows PDB limit, BUG-174).
- **The package** is four commands from Google's own build tools, already on the runner image
  (actions/runner-images, Ubuntu 24.04, image 20261111: build-tools 34 to 37, platforms up to
  android-37, NDK 27.3 as `ANDROID_NDK_HOME`, Java 17): `aapt2 compile` and `aapt2 link` (manifest
  and icon), `zip` (the library), `zipalign`, `apksigner`. With NativeActivity there is no Java
  code to compile, so nothing needs Gradle. [android/build-apk.sh](../../../../android/build-apk.sh)
  is the whole route, about a hundred lines anyone can read.
- **cargo-apk** would do both steps in one tool and fits NativeActivity, but it has had no release
  since 2023-11, and it reads its configuration from the crate's manifest metadata, which our crate
  (a desktop app first) would carry for one platform. A fallback if the hand-written packaging ever
  proves fragile.
- **xbuild** calls itself unmaintained. Not considered further.

Not compared here: Gradle with the Android Gradle Plugin (needed later if the app moves to
GameActivity for a better on-screen keyboard, which brings Java code), and Google's `bundletool`
(app bundles are a Google Play format; Play is not on the path, per
[`../../findings/2026-11-14-app-distribution-costs.md`](../../findings/2026-11-14-app-distribution-costs.md)).

## Sources

- github.com/bbqsrc/cargo-ndk (repository page, and `src/cli/mod.rs` through the GitHub API), read 2026-11-14
- github.com/rust-mobile/cargo-apk (repository page), read 2026-11-14
- github.com/rust-mobile/xbuild (repository page), read 2026-11-14
- crates.io API, `/api/v1/crates/<name>` for each (versions, dates, downloads), read 2026-11-14
- github.com/actions/runner-images, `images/ubuntu/Ubuntu2404-Readme.md` (image 20261111.370.1), read 2026-11-14
- robots.txt read first: github.com (main repository pages allowed), crates.io (allows all)
