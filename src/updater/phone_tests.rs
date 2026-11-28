//! The phone app's own updates (src/updater/phone.rs, 2026-11-27): which release it offers, the
//! checksum it holds the download to, what a start clears, and that the Java, the manifest and the
//! release workflow agree with the Rust about names.

use super::*;
use crate::updater::{parse_feed, UpdateState, UpdateTarget, Updater};
use std::io::Write;

/// GitHub's release feed as it answers, newest first, cut to the fields the updater reads: a
/// release whose APK is not attached yet (desktop binaries and the signed manifest only), an asset
/// release (instrument packs, with checksums of their own), a release with an APK but no checksum
/// beside it, then two complete phone releases.
fn feed() -> String {
    let asset = |tag: &str, name: &str, size: u64| {
        format!(r#"{{"name":"{name}","browser_download_url":"https://example.invalid/{tag}/{name}","size":{size}}}"#)
    };
    let release = |tag: &str, assets: &[String]| {
        format!(r#"{{"tag_name":"{tag}","name":null,"body":"notes for {tag}","published_at":"2026-11-27T00:00:00Z","prerelease":false,"draft":false,"assets":[{}]}}"#, assets.join(","))
    };
    let desktop = |tag: &str| vec![asset(tag, "HumanityOS-windows-x64.exe", 60_000_000), asset(tag, "HumanityOS-linux-x64", 60_000_000), asset(tag, "release-manifest.json", 900)];
    let phone = |tag: &str| vec![asset(tag, APK_NAME, 91_234_567), asset(tag, APK_SUM_NAME, 95)];
    let releases = [
        release("v0.1581.0", &desktop("v0.1581.0")),
        release("assets-instruments-1", &[asset("assets-instruments-1", "choir.tar", 40_000_000), asset("assets-instruments-1", "choir.tar.sha256", 75)]),
        release("v0.1580.0", &[desktop("v0.1580.0"), vec![asset("v0.1580.0", APK_NAME, 91_000_000)]].concat()),
        release("v0.1579.0", &[desktop("v0.1579.0"), phone("v0.1579.0")].concat()),
        release("v0.1578.0", &phone("v0.1578.0")),
    ];
    format!("[{}]", releases.join(","))
}

/// A phone updater at `current`, holding the feed above as a check would have left it.
fn phone_at(current: &str) -> Updater {
    let mut u = Updater::new(current);
    u.target = UpdateTarget::Phone;
    u.available_releases = parse_feed(&feed()).expect("the feed parses");
    u
}

/// THE FEED PARSER PICKS THE APK and passes over every release without one: the release whose APK
/// is not attached yet, the asset release, and the release whose APK has no checksum beside it.
/// The newest complete phone release is offered, by its APK's address and size.
///
/// Seen red 2026-11-27 with `apk_assets` returning the APK alone (its checksum not asked for):
/// "an APK without its checksum is not an update the phone can check".
#[test]
fn the_feed_parser_picks_the_apk_and_passes_over_releases_without_one() {
    let releases = parse_feed(&feed()).expect("the feed parses");
    assert!(apk_assets(&releases[0].assets).is_none(), "a release whose APK is not attached yet has no phone update");
    assert!(apk_assets(&releases[1].assets).is_none(), "an asset release has no phone update");
    assert!(apk_assets(&releases[2].assets).is_none(), "an APK without its checksum is not an update the phone can check");
    let mut u = phone_at("0.1577.0");
    u.evaluate_update();
    match &u.state {
        UpdateState::Available { version, download_url, asset_size, .. } => {
            assert_eq!(version, "v0.1579.0", "offered {version}, the release whose APK has no checksum: {:?}", u.state);
            assert_eq!(download_url, &format!("https://example.invalid/v0.1579.0/{APK_NAME}"), "the phone downloads the APK, nothing else");
            assert_eq!(*asset_size, 91_234_567, "the line shows the APK's own size");
        }
        other => panic!("the newest complete phone release was not offered: {other:?}"),
    }
}

/// A NEWER RELEASE IS OFFERED; AN EQUAL OR OLDER ONE IS NOT (Android would refuse to put an older
/// version over a newer one anyway, and the same version is no update). The two complete phone
/// releases only, so this asks about versions and nothing else.
///
/// Seen red 2026-11-27 with the version comparison skipped for the phone: "a phone at 0.1579.0 was
/// offered an update that is not newer: Available { version: \"v0.1579.0\", ...".
#[test]
fn a_newer_release_is_offered_and_an_equal_or_older_one_is_not() {
    let at = |current: &str| {
        let mut u = phone_at(current);
        u.available_releases.retain(|r| r.tag_name == "v0.1579.0" || r.tag_name == "v0.1578.0");
        u.evaluate_update();
        u
    };
    let u = at("0.1578.5");
    assert!(
        matches!(&u.state, UpdateState::Available { version, .. } if version == "v0.1579.0"),
        "a phone at 0.1578.5 was not offered v0.1579.0: {:?}",
        u.state
    );
    for current in ["0.1579.0", "0.1579.1", "0.1580.0", "0.1600.0"] {
        let u = at(current);
        assert_eq!(u.state, UpdateState::UpToDate, "a phone at {current} was offered an update that is not newer: {:?}", u.state);
    }
}

/// A one-file web server on 127.0.0.1 (no firewall prompt), answering each path with its bytes and
/// anything else with 404, until the test process ends. Returns its address.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let addr = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let n = std::io::Read::read(&mut s, &mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
            match files.iter().find(|(p, _)| *p == path) {
                Some((_, body)) => {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = s.write_all(body);
                }
                None => {
                    let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    format!("http://{addr}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// The files the download left in `dir`.
fn left_in(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).map(|it| it.flatten().filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default()
}

/// A CHECKSUM MISMATCH REFUSES, and leaves nothing behind for the installer: the APK is downloaded
/// (through the same streaming the desktop uses), hashed, found different from the published
/// checksum, and deleted. With the right checksum the same download is kept under its final name.
/// A checksum file that is not a checksum (a web page, say) stops it before the APK is fetched.
///
/// Seen red 2026-11-27 with the comparison in download_apk skipped: "a download that does not match
/// its checksum was kept: Ok(\"...HumanityOS-0.1580.0.apk\")".
#[test]
fn a_checksum_mismatch_refuses_and_leaves_nothing() {
    let apk: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let right = format!("{}  {APK_NAME}\n", sha256_hex(&apk));
    let wrong = format!("{}  {APK_NAME}\n", sha256_hex(b"some other file"));
    let base = serve(vec![
        ("/apk", apk.clone()),
        ("/right.sha256", right.into_bytes()),
        ("/wrong.sha256", wrong.into_bytes()),
        ("/page.sha256", b"<!doctype html><title>Not Found</title>".to_vec()),
    ]);
    let apk_url = format!("{base}/apk");

    let dir = humanity_core::test_temp::dir("phone_update_mismatch");
    let got = download_apk(&apk_url, &format!("{base}/wrong.sha256"), dir.path(), "v0.1580.0", &mut |_| {});
    assert!(matches!(got, Err(Refusal::Mismatch { .. })), "a download that does not match its checksum was kept: {got:?}");
    assert!(left_in(dir.path()).is_empty(), "a refused download left files for the installer: {:?}", left_in(dir.path()));

    let got = download_apk(&apk_url, &format!("{base}/page.sha256"), dir.path(), "v0.1580.0", &mut |_| {});
    assert!(matches!(got, Err(Refusal::NoChecksum(_))), "a checksum file that is not a checksum was taken: {got:?}");
    assert!(left_in(dir.path()).is_empty(), "the APK was fetched although its checksum was unreadable: {:?}", left_in(dir.path()));

    let mut last = 0.0f32;
    let got = download_apk(&apk_url, &format!("{base}/right.sha256"), dir.path(), "v0.1580.0", &mut |p| last = p);
    let path = got.expect("the download that matches its checksum is kept");
    assert_eq!(path, dir.path().join("HumanityOS-0.1580.0.apk"));
    assert_eq!(std::fs::read(&path).expect("read it back"), apk, "the kept file is the APK served");
    assert_eq!(left_in(dir.path()), vec!["HumanityOS-0.1580.0.apk".to_string()], "only the checked file stays (no .part)");
    assert!((last - 1.0).abs() < 1e-6, "the progress reached the end: {last}");
}

/// The checksum file reads the way `sha256sum` writes it (android/build-apk.sh), and nothing else
/// passes for it.
#[test]
fn the_checksum_file_reads_as_sha256sum_writes_it() {
    let hex = "ab".repeat(32);
    assert_eq!(parse_checksum(&format!("{hex}  {APK_NAME}\n")), Some(hex.clone()));
    assert_eq!(parse_checksum(&format!("{hex} *{APK_NAME}\r\n")), Some(hex.clone()), "binary mode's star");
    assert_eq!(parse_checksum(&hex.to_uppercase()), Some(hex.clone()), "the digest alone, in capitals");
    assert_eq!(parse_checksum(&format!("{hex}  HumanityOS-android-arm64-test-signed.apk")), None, "another file's checksum");
    // text-cut-ok: the digest is ASCII hex digits, one byte each
    assert_eq!(parse_checksum(&format!("{}  {APK_NAME}", &hex[..62])), None, "a short digest");
    // text-cut-ok: the digest is ASCII hex digits, one byte each
    assert_eq!(parse_checksum(&format!("{}zz  {APK_NAME}", &hex[..62])), None, "not hex");
    assert_eq!(parse_checksum(&format!("{hex}  {APK_NAME}\n{hex}  other.apk\n")), None, "more than the one line");
    assert_eq!(parse_checksum(""), None);
}

/// A start clears half-downloaded files and every update not newer than the version running (it
/// was installed, or it is older); a newer one waits for Install.
#[test]
fn a_start_clears_half_downloads_and_installed_updates() {
    let names: Vec<String> = ["HumanityOS-0.1579.0.apk", "HumanityOS-0.1580.0.apk", "HumanityOS-0.1581.0.apk", "HumanityOS-0.1582.0.apk.part", "notes.txt"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let gone = stale(&names, "0.1580.0");
    assert_eq!(gone, vec!["HumanityOS-0.1579.0.apk", "HumanityOS-0.1580.0.apk", "HumanityOS-0.1582.0.apk.part"]);
}

/// Every reason the update stops says a sentence of its own from data/i18n/en.json.
#[test]
fn every_refusal_says_a_sentence_of_its_own() {
    let all = [
        Refusal::NoChecksum(String::new()),
        Refusal::Download(String::new()),
        Refusal::Mismatch { expected: String::new(), got: String::new() },
        Refusal::NotAPhone,
        Refusal::Missing(String::new()),
        Refusal::Installer(String::new()),
    ];
    for r in all {
        assert!(humanity_core::i18n::has(r.words_key()), "{r:?} names {}, which data/i18n/en.json lacks", r.words_key());
    }
}

/// The Java serves the folder the Rust writes into, by the names the Rust gives its files; the
/// manifest declares the permission and provider (tests/android_manifest_lint.rs holds those).
#[test]
fn the_java_serves_the_folder_and_names_the_rust_writes() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let java = std::fs::read_to_string(root.join("android/java/us/united_humanity/humanityos/UpdateInstall.java")).expect("UpdateInstall.java");
    assert!(java.contains(&format!("FOLDER = \"{UPDATE_FOLDER}\";")), "UpdateInstall.java serves another folder than {UPDATE_FOLDER:?}");
    assert!(java.contains(r#"name.matches("HumanityOS-[0-9A-Za-z.\\-]+\\.apk")"#), "UpdateInstall.java's safeName changed: check file_name still makes names it serves");
    for version in ["v0.1580.0", "0.1580.0", "v1.0.0-rc.1"] {
        let name = file_name(version);
        let middle = name.strip_prefix("HumanityOS-").and_then(|m| m.strip_suffix(".apk")).expect("the name's shape");
        assert!(!middle.is_empty() && middle.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && !name.contains(".."), "{name} is not a name the Java serves");
    }
    assert_eq!(file_name("v0.1580.0/../../x"), "HumanityOS-0.1580.0....x.apk", "a slash never reaches the name");
}

/// The release workflow publishes the checksum the phone reads, beside the APK, and checks it
/// matches before publishing (.github/workflows/build-android.yml, android/build-apk.sh).
#[test]
fn the_release_workflow_publishes_the_checksum_the_phone_reads() {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let workflow = std::fs::read_to_string(root.join(".github/workflows/build-android.yml")).expect("build-android.yml");
    for name in [APK_NAME, APK_SUM_NAME] {
        assert!(workflow.contains(&format!("dist/{name}")), "the release workflow does not publish {name}");
    }
    assert!(workflow.contains("sha256sum -c"), "the release workflow must check the checksum against the APK before publishing it");
    let build = std::fs::read_to_string(root.join("android/build-apk.sh")).expect("build-apk.sh");
    assert!(build.contains("sha256sum") && build.contains(".sha256"), "android/build-apk.sh no longer writes the APK's checksum");
}
