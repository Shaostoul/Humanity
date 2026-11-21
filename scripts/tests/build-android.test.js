// THE PHONE APP'S BUILD (Android rung A1, 2026-11-14; android/README.md), checked where a phone or
// a CI run is not needed. Pure node, no game, the real repo: runs in `just rig-tests`.
//
// Run:  node --test scripts/tests/build-android.test.js
//
// The APK is built only on GitHub's runners (.github/workflows/build-android.yml runs
// android/build-apk.sh), so a mismatch between its parts shows up as a failed run half an hour
// later, or worse, as an APK that installs and then cannot start. These are the parts that must
// agree, each checked against the file that sets it. Each was seen to FAIL with its part broken:
// the break and the failure are written beside it.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const release = require("../lib/release-files.js");

const ROOT = path.resolve(__dirname, "..", "..");
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), "utf8").replace(/\r\n/g, "\n");

// NativeActivity loads lib<name>.so, <name> from the manifest; the build makes the library of the
// root package, whose lib name is its package name with dashes as underscores. Broken 2026-11-14 by
// setting the manifest's value to "humanityos": "the manifest's library ... humanityos".
test("the manifest names the library the build makes", () => {
  const manifest = read("android/AndroidManifest.xml");
  const lib = /android\.app\.lib_name"\s+android:value="([^"]+)"/.exec(manifest);
  assert.ok(lib, "no android.app.lib_name in the manifest");
  const cargo = read("Cargo.toml");
  const pkg = /\[package\]\s*\nname = "([^"]+)"/.exec(cargo);
  assert.ok(pkg, "no [package] name in Cargo.toml");
  const libName = pkg[1].replace(/-/g, "_");
  assert.strictEqual(lib[1], libName, `the manifest's library is ${lib[1]}, the package's ${libName}`);
  const script = read("android/build-apk.sh");
  assert.ok(script.includes(`lib${libName}.so`), `build-apk.sh does not copy lib${libName}.so`);
  assert.ok(/rustc --lib --release --crate-type cdylib/.test(script), "build-apk.sh does not build the lib as a cdylib");
  assert.ok(manifest.includes('android:name="android.app.NativeActivity"'), "the manifest's activity is not NativeActivity");
  assert.ok(manifest.includes('android:hasCode="false"'), "an APK with no Java code must say android:hasCode=\"false\"");
});

// The library's entry point is the function NativeActivity's glue calls, and winit's backend for
// it is on in the phone build. Broken by renaming android_main: "src/android.rs has no android_main".
test("the entry point and winit's NativeActivity backend are there", () => {
  const entry = read("src/android.rs");
  assert.ok(/#\[no_mangle\]\s*\nfn android_main\(app: AndroidApp\)/.test(entry), "src/android.rs has no android_main");
  const cargo = read("Cargo.toml");
  const androidTable = cargo.split("[target.'cfg(target_os = \"android\")'.dependencies]")[1] || "";
  assert.ok(/winit = \{[^}]*"android-native-activity"/.test(androidTable.split("\n[")[0]), "winit's android-native-activity feature is not on for Android");
  const lib = read("src/lib.rs");
  assert.ok(/#\[cfg\(all\(feature = "native", target_os = "android"\)\)\]\s*\npub mod android;/.test(lib), "src/lib.rs does not declare the android module for the phone build");
});

// The crates with no Android code are declared for every target but Android. Broken by moving
// tray-icon back to [dependencies]: "tray-icon is declared outside the not-Android table".
test("the desktop-only crates are left out of the phone build", () => {
  const cargo = read("Cargo.toml");
  const notAndroid = (cargo.split("[target.'cfg(not(target_os = \"android\"))'.dependencies]")[1] || "").split("\n[")[0];
  for (const name of ["tray-icon", "arboard", "keyring"]) {
    assert.ok(new RegExp(`^${name} = `, "m").test(notAndroid), `${name} is declared outside the not-Android table`);
    const deps = (cargo.split("\n[dependencies]\n")[1] || "").split("\n[")[0];
    assert.ok(deps.length > 0, "no [dependencies] table in Cargo.toml");
    assert.ok(!new RegExp(`^${name} = `, "m").test(deps), `${name} is still in [dependencies], for every target`);
  }
  assert.ok(/^tungstenite = \{[^}]*"native-tls"/m.test(notAndroid), "the desktop's chat socket lost native-tls");
  const android = (cargo.split("[target.'cfg(target_os = \"android\")'.dependencies]")[1] || "").split("\n[")[0];
  assert.ok(/^tungstenite = \{[^}]*"rustls-tls-webpki-roots"/m.test(android), "the phone's chat socket is not on rustls");
  const core = read("crates/humanity-core/Cargo.toml");
  const coreNotAndroid = (core.split("[target.'cfg(not(target_os = \"android\"))'.dependencies]")[1] || "").split("\n[")[0];
  assert.ok(/^keyring = /m.test(coreNotAndroid), "humanity-core declares keyring for Android, whose mock store forgets the key");
});

// The mirror on the VPS takes a release's platform files by name, and a platform must have all three
// of its files before the release counts as ready: an APK taken as a platform would hold the release
// back. It takes the APK as an OPTIONAL file instead (PHONE_RE, 2026-11-20), so its manifest lists the
// phone app and the download page shows the install steps (web/pages/download-app.js), and never its
// .sha256. Broken by naming the APK HumanityOS-android-arm64.zip: "the mirror would take ... as a
// platform"; by dropping PHONE_RE's branch: "the mirror would not list".
test("the VPS mirror takes the APK as an optional file, never as a platform", () => {
  const mirror = read("scripts/vps/humanity-mirror-release");
  const m = /PLATFORM_RE = re\.compile\(r"(.*)"\)/.exec(mirror);
  assert.ok(m, "no PLATFORM_RE in the mirror");
  const platform = new RegExp(m[1]);
  const p = /PHONE_RE = re\.compile\(r"(.*)"\)/.exec(mirror);
  assert.ok(p && /elif PHONE_RE\.match\(name\):/.test(mirror), "the mirror would not list the phone app (no PHONE_RE branch)");
  const phone = new RegExp(p[1]);
  const workflow = read(".github/workflows/build-android.yml");
  const names = [...workflow.matchAll(/dist\/(HumanityOS-android[^\s']*)/g)].map((x) => x[1]);
  assert.ok(names.length >= 2, "the workflow publishes no APK");
  for (const n of names) {
    assert.ok(!platform.test(n), `the mirror would take ${n} as a platform`);
    assert.strictEqual(phone.test(n), n.endsWith(".apk"), `PHONE_RE on ${n}`);
  }
});

// A test-signed APK is never a release asset: the release step publishes only the name the
// project's key gives. Broken by publishing dist/*.apk: "the release step publishes ... -test-signed".
test("only an APK signed with the project's key goes on a release", () => {
  const workflow = read(".github/workflows/build-android.yml");
  const publish = workflow.split("- name: Publish to Release")[1].split("- name:")[0];
  assert.ok(publish.includes("hashFiles('dist/HumanityOS-android-arm64.apk') != ''"), "the release step does not wait for the signed APK");
  assert.ok(!/\*\.apk/.test(publish), "the release step publishes every .apk, -test-signed ones included");
  const script = read("android/build-apk.sh");
  assert.ok(script.includes('APK="$OUT/$NAME-test-signed.apk"'), "build-apk.sh does not name a test-signed APK so");
  assert.ok(/NAME=HumanityOS-android-arm64\n/.test(script), "build-apk.sh's name differs from the workflow's");
});

// The phone's files: the release's, less what only the 3D world reads, and never the app's own
// words or theme left out. Broken by adding data/i18n/ to PHONE_LEFT_OUT: "data/i18n/en.json is not in".
test("the phone app carries the pages' files and not the 3D world's", () => {
  const files = release.phoneFiles(ROOT);
  const all = release.releaseFiles(ROOT);
  assert.ok(files.length > 0 && files.length < all.length, "the phone list is empty or the whole release");
  for (const rel of ["data/i18n/en.json", "data/gui/theme.ron", "data/gui/navigation.json"]) {
    if (all.includes(rel)) assert.ok(files.includes(rel), `${rel} is not in the phone's files`);
  }
  for (const rel of files) {
    assert.ok(!rel.startsWith("assets/models/") && !rel.startsWith("assets/textures/") && !rel.startsWith("data/planets/"), `${rel} is the 3D world's`);
    assert.ok(!rel.split("/").includes(".."), `${rel} leaves the folder`);
  }
  assert.ok(read("android/build-apk.sh").includes("release-files.js --phone-list"), "build-apk.sh does not package the phone list");
});
