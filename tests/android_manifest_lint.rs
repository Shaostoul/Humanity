//! The phone app's manifest carries what its Java needs on current Android (Android rungs A1 and
//! A4, 2026-11-20; android/README.md "The Java in the APK", docs/design/platforms.md).
//!
//! Nothing on this PC can build or install the APK (no Android SDK here; the build runs on GitHub,
//! .github/workflows/build-android.yml), and a manifest mistake shows only on a phone: Android 14
//! and later throw when a foreground service starts without a declared type and its permission,
//! Android 13 and later show no notices without POST_NOTIFICATIONS, and a service the manifest
//! does not name never starts. So this reads android/AndroidManifest.xml (comments removed) and
//! the Java beside it, and fails on any of those:
//!
//! - the permissions: INTERNET, FOREGROUND_SERVICE, FOREGROUND_SERVICE_SPECIAL_USE (the type's
//!   own, required since Android 14) and POST_NOTIFICATIONS (Android 13 and later);
//! - the stay-connected service (`ChatService`): named with its package, not exported, type
//!   `specialUse` with the `PROPERTY_SPECIAL_USE_FGS_SUBTYPE` property saying why in words,
//!   `stopWithTask="true"` (swiping the app away ends it, as android/README.md says);
//! - the Java agrees with it: `ChatService.java` starts in the same type
//!   (`FOREGROUND_SERVICE_TYPE_SPECIAL_USE`), every class the manifest names has its source under
//!   android/java/, and the application no longer says `hasCode="false"`;
//! - android/build-apk.sh compiles that Java (javac and d8) into the APK.
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/android_manifest_lint.rs
//!
//! Seen red 2026-11-20 against the manifest before the service was added: "the manifest lacks the
//! permission android.permission.FOREGROUND_SERVICE" and the rest of the list, then "no <service>
//! named us.united_humanity.humanityos.ChatService".

use std::fs;
use std::path::{Path, PathBuf};

const PACKAGE: &str = "us.united_humanity.humanityos";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The manifest's text with its `<!-- -->` comments taken out (they mention attributes in words).
fn manifest() -> String {
    let raw = fs::read_to_string(repo().join("android/AndroidManifest.xml")).expect("android/AndroidManifest.xml");
    let mut out = String::new();
    let mut rest = raw.as_str();
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Every element `<tag ...>` (or `<tag .../>`) in `xml`, as its text between the angle brackets.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        if after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            let end = after.find('>').unwrap_or(after.len());
            out.push(&after[..end]);
        }
        rest = &rest[i + open.len()..];
    }
    out
}

/// The value of `name="..."` in an element's text.
fn attr<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let i = element.find(&key)? + key.len();
    let end = element[i..].find('"')?;
    Some(&element[i..i + end])
}

#[test]
fn the_manifest_asks_for_what_the_java_needs() {
    let xml = manifest();
    let permissions: Vec<&str> = elements(&xml, "uses-permission").iter().filter_map(|e| attr(e, "android:name")).collect();
    let mut missing = Vec::new();
    for p in [
        "android.permission.INTERNET",
        "android.permission.FOREGROUND_SERVICE",
        "android.permission.FOREGROUND_SERVICE_SPECIAL_USE",
        "android.permission.POST_NOTIFICATIONS",
    ] {
        if !permissions.contains(&p) {
            missing.push(format!("the manifest lacks the permission {p}"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn the_stay_connected_service_is_declared_for_current_android() {
    let xml = manifest();
    let name = format!("{PACKAGE}.ChatService");
    let service = elements(&xml, "service")
        .into_iter()
        .find(|e| attr(e, "android:name") == Some(name.as_str()))
        .unwrap_or_else(|| panic!("no <service> named {name}"));
    assert_eq!(attr(service, "android:exported"), Some("false"), "the service is the app's own: not exported");
    assert_eq!(attr(service, "android:foregroundServiceType"), Some("specialUse"), "Android 14 needs the type declared");
    assert_eq!(attr(service, "android:stopWithTask"), Some("true"), "swiping the app away ends it (android/README.md)");
    // The property sits inside the service element.
    let body_start = xml.find(service).expect("the service's text") + service.len();
    let body = &xml[body_start..xml[body_start..].find("</service>").map(|e| body_start + e).expect("</service>")];
    let property = elements(body, "property")
        .into_iter()
        .find(|e| attr(e, "android:name") == Some("android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"))
        .expect("specialUse needs PROPERTY_SPECIAL_USE_FGS_SUBTYPE inside the service");
    assert!(attr(property, "android:value").is_some_and(|v| v.split_whitespace().count() >= 5), "the property says why in words");

    let java = fs::read_to_string(repo().join("android/java").join(PACKAGE.replace('.', "/")).join("ChatService.java")).expect("ChatService.java");
    assert!(java.contains("FOREGROUND_SERVICE_TYPE_SPECIAL_USE"), "ChatService must start in the type the manifest declares");
    assert!(java.contains("START_NOT_STICKY"), "the service never restarts without the app");
}

/// Nothing of the app's folder leaves the phone by backup OR by device-to-device transfer: the
/// remembered keys (`key-store/`, sealed by keys that never leave this phone's key store) and the
/// config with the passphrase-sealed identity and the Quick PIN's blob live there. At targetSdk 31
/// and later `allowBackup="false"` stops the cloud backup but, on some makers' phones, not a
/// device-to-device transfer (developer.android.com, "Behavior changes: Apps targeting Android 12",
/// Backup and restore, read 2026-11-20), so the manifest names data extraction rules that exclude
/// everything from both (the review's finding 3). Seen red 2026-11-20 against the manifest with
/// only allowBackup="false": "the <application> names no android:dataExtractionRules".
#[test]
fn nothing_leaves_the_phone_by_backup_or_device_transfer() {
    let xml = manifest();
    let application = elements(&xml, "application").into_iter().next().expect("<application>");
    assert_eq!(attr(application, "android:allowBackup"), Some("false"), "no cloud backup (and none at all before Android 12)");
    let rules = attr(application, "android:dataExtractionRules").expect("the <application> names no android:dataExtractionRules");
    let name = rules.strip_prefix("@xml/").expect("the rules are an XML resource: @xml/<name>");
    let path = repo().join("android/res/xml").join(format!("{name}.xml"));
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // Comments out, as for the manifest.
    let mut text = String::new();
    let mut rest = raw.as_str();
    while let Some(start) = rest.find("<!--") {
        text.push_str(&rest[..start]);
        rest = rest[start..].find("-->").map_or("", |end| &rest[start + end + 3..]);
    }
    text.push_str(rest);
    assert!(text.contains("<data-extraction-rules>"), "{} is not a data-extraction-rules file", path.display());
    assert!(elements(&text, "include").is_empty(), "the rules include nothing: an <include> would start a list of what leaves the phone");
    for section in ["cloud-backup", "device-transfer"] {
        let open = format!("<{section}>");
        let close = format!("</{section}>");
        let start = text.find(&open).unwrap_or_else(|| panic!("the rules have no <{section}> section"));
        let body = &text[start..text[start..].find(&close).map(|e| start + e).unwrap_or_else(|| panic!("no {close}"))];
        let excluded: Vec<(Option<&str>, Option<&str>)> = elements(body, "exclude").iter().map(|e| (attr(e, "domain"), attr(e, "path"))).collect();
        for domain in ["root", "device_root", "external"] {
            assert!(
                excluded.contains(&(Some(domain), Some("."))),
                "<{section}> must exclude everything in the {domain} domain (domain=\"{domain}\" path=\".\"): {excluded:?}"
            );
        }
    }
    let build = fs::read_to_string(repo().join("android/build-apk.sh")).expect("android/build-apk.sh");
    assert!(build.contains("android/res/xml"), "android/build-apk.sh must put android/res/xml into the APK's resources");
}

#[test]
fn every_class_the_manifest_names_is_built_into_the_apk() {
    let xml = manifest();
    let application = elements(&xml, "application").into_iter().next().expect("<application>");
    assert_ne!(attr(application, "android:hasCode"), Some("false"), "the APK carries Java now");
    for tag in ["service", "receiver", "activity", "provider"] {
        for e in elements(&xml, tag) {
            let Some(name) = attr(e, "android:name") else { continue };
            if !name.starts_with(PACKAGE) {
                continue; // Android's own (NativeActivity)
            }
            let src = repo().join("android/java").join(format!("{}.java", name.replace('.', "/")));
            assert!(src.is_file(), "the manifest names {name}, but {} does not exist", src.display());
        }
    }
    let build = fs::read_to_string(repo().join("android/build-apk.sh")).expect("android/build-apk.sh");
    for needed in ["android/java", "javac", "/d8\"", "classes.dex"] {
        assert!(build.contains(needed), "android/build-apk.sh must compile the Java into the APK (no {needed:?})");
    }
    assert!(Path::new(&repo().join("android/java")).is_dir());
}
