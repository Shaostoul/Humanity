//! Installing an instrument pack from a server on this computer (127.0.0.1, never the internet),
//! the way Settings > Audio > Instruments does it: downloaded, checked against the catalog's
//! SHA-256, unpacked beside the others, removed cleanly; a damaged download refused with nothing
//! installed and nothing left behind.
//!
//! The pack is `tests/fixtures/instrument_packs/`, built by the pipeline itself from its fixture
//! library (scripts/tests/instrument-packs.test.js with HUMANITY_WRITE_INSTRUMENT_FIXTURE=1).
//! What each test saw red first is at the test.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use super::catalog::{packs_dir, PackCatalog, PackEntry};
use super::install::{self, Failure, Stage};

pub(super) fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("tests").join("fixtures").join("instrument_packs")
}

pub(super) fn fixture_tar() -> Vec<u8> {
    std::fs::read(fixture_dir().join("humanity-instruments-fixture-v1.tar")).expect("the fixture pack")
}

pub(super) fn fixture_entry() -> PackEntry {
    let catalog = PackCatalog::load(&fixture_dir().join("data")).expect("the fixture catalog");
    catalog.pack("fixture").expect("the fixture pack is listed").clone()
}

/// A server on 127.0.0.1 that answers one GET with `body`: the address to fetch.
pub(super) fn serve_once(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buf[..n]),
                }
            }
            let reply = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/x-tar\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(reply.as_bytes());
            let _ = s.write_all(&body);
            let _ = s.flush();
        }
    });
    format!("http://{addr}/")
}

/// Every name in a folder, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// A PACK INSTALLS FROM A LOOPBACK SERVER AND REMOVES CLEANLY: downloaded from 127.0.0.1, checked,
/// unpacked as `<data>/music/packs/fixture/` with its manifest, definitions and samples, listed as
/// installed at its version, nothing else left in the folder; removed, the folder is empty again.
///
/// Seen red 2026-11-06 with the swap into place left out of `install_from_reader` (the unpacked
/// folder deleted with the leftovers): "the pack is installed" (left: None, right: Some(1)).
#[test]
fn a_pack_installs_from_a_loopback_server_and_removes_cleanly() {
    let data = humanity_core::test_temp::dir("instr_install");
    let packs = packs_dir(&data);
    let entry = fixture_entry();
    let url = entry.url(&serve_once(fixture_tar()));
    assert!(url.starts_with("http://127.0.0.1:"), "served from this computer: {url}");
    let progress = install::new_progress();
    install::download_and_install(&url, &entry, &packs, &progress).expect("the pack installs");
    let p = progress.lock().unwrap().clone();
    assert_eq!(p.stage, Stage::Done);
    assert_eq!(p.done, entry.bytes, "every byte arrived");
    let installed = install::installed_packs(&packs);
    assert_eq!(installed.get("fixture").map(|i| i.version), Some(1), "the pack is installed");
    assert_eq!(names(&packs), vec!["fixture".to_string()], "nothing but the pack is left in the folder");
    let pack = packs.join("fixture");
    for f in ["pack.ron", "instruments/fixture_grand.ron", "instruments/fixture_drum.ron", "samples/fixture_grand/LICENSE.txt", "samples/fixture_grand/source.ron"] {
        assert!(pack.join(f).is_file(), "{f} is installed");
    }
    install::remove_pack(&packs, "fixture").expect("removes");
    assert!(names(&packs).is_empty(), "removed cleanly: {:?}", names(&packs));
    assert!(install::installed_packs(&packs).is_empty());
}

/// A DAMAGED PACK IS REFUSED: one byte changed in the middle, or the archive cut short, and the
/// install fails as damaged, installs nothing and leaves nothing behind; an installed pack stays
/// as it was when its update arrives damaged.
///
/// Seen red 2026-11-06 with the SHA-256 comparison taken out of `install_from_reader`: "a changed
/// byte is refused as damaged: Ok(())".
#[test]
fn a_damaged_pack_is_refused_and_nothing_is_installed() {
    let data = humanity_core::test_temp::dir("instr_damaged");
    let packs = packs_dir(&data);
    let entry = fixture_entry();
    let mut bad = fixture_tar();
    let mid = bad.len() / 2;
    bad[mid] ^= 0x5a;
    let progress = install::new_progress();
    let got = install::download_and_install(&entry.url(&serve_once(bad)), &entry, &packs, &progress);
    assert!(matches!(got, Err(Failure::Damaged(_))), "a changed byte is refused as damaged: {got:?}");
    assert!(matches!(progress.lock().unwrap().stage, Stage::Failed(Failure::Damaged(_))));
    assert!(names(&packs).is_empty(), "nothing installed, nothing left: {:?}", names(&packs));

    let mut short = fixture_tar();
    short.truncate(short.len() - 1000);
    let got = install::download_and_install(&entry.url(&serve_once(short)), &entry, &packs, &install::new_progress());
    assert!(matches!(got, Err(Failure::Damaged(_))), "a short download is refused: {got:?}");
    assert!(names(&packs).is_empty());

    // An installed pack survives a damaged update.
    install::install_from_reader(&mut fixture_tar().as_slice(), &entry, &packs, &install::new_progress()).expect("installs");
    let mut bad = fixture_tar();
    bad[100] ^= 1;
    let got = install::install_from_reader(&mut bad.as_slice(), &entry, &packs, &install::new_progress());
    assert!(matches!(got, Err(Failure::Damaged(_))));
    assert_eq!(names(&packs), vec!["fixture".to_string()], "the installed pack is still there, and only it");
    assert!(packs.join("fixture").join("instruments").join("fixture_grand.ron").is_file());
}

/// A pack listed but not built yet (no checksum) is never downloaded: there is nothing to check it
/// against.
///
/// Seen red 2026-11-06 with the check taken out of `download_and_install`: it went to the network
/// (left: Err(Network("http://127.0.0.1:9/never: Connection Failed: ...")), right:
/// Err(NotPublished)).
#[test]
fn an_unbuilt_pack_is_not_downloaded() {
    let data = humanity_core::test_temp::dir("instr_unbuilt");
    let mut entry = fixture_entry();
    entry.sha256.clear();
    let got = install::download_and_install("http://127.0.0.1:9/never", &entry, &packs_dir(&data), &install::new_progress());
    assert_eq!(got, Err(Failure::NotPublished));
}

/// One ustar entry, for archives no pipeline would write.
fn tar_entry(name: &str, data: &[u8], kind: u8) -> Vec<u8> {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
    h[136..148].copy_from_slice(b"00000000000\0");
    h[148..156].copy_from_slice(b"        ");
    h[156] = kind;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|&b| b as u32).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut out = h.to_vec();
    out.extend_from_slice(data);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

/// THE ARCHIVE READER WRITES NOTHING OUTSIDE ITS FOLDER: a path climbing out with `..`, an absolute
/// path, a drive letter, a link, each stops the unpacking with nothing written outside.
///
/// Seen red 2026-11-06 with `safe_path` accepting every name: "a path climbing out is refused:
/// ../outside.txt".
#[test]
fn the_archive_reader_writes_nothing_outside_its_folder() {
    let root = humanity_core::test_temp::dir("instr_tar");
    let dest = root.join("inside");
    std::fs::create_dir_all(&dest).unwrap();
    for (name, kind, why) in [
        ("../outside.txt", b'0', "a path climbing out is refused"),
        ("/abs.txt", b'0', "an absolute path is refused"),
        ("C:/drive.txt", b'0', "a drive letter is refused"),
        ("ok/../../outside.txt", b'0', "a climb hidden in the middle is refused"),
        ("link", b'2', "a symbolic link is refused"),
        ("hard", b'1', "a hard link is refused"),
    ] {
        let mut tar = tar_entry(name, b"x", kind);
        tar.extend_from_slice(&[0u8; 1024]);
        let got = super::tar::extract(&mut tar.as_slice(), &dest);
        assert!(got.is_err(), "{why}: {name}");
        assert_eq!(names(&root), vec!["inside".to_string()], "{why}: nothing beside the folder");
    }
    // And a good one unpacks.
    let mut tar = tar_entry("a/b.txt", b"bee", b'0');
    tar.extend_from_slice(&[0u8; 1024]);
    let got = super::tar::extract(&mut tar.as_slice(), &dest).expect("a plain file unpacks");
    assert_eq!(got, vec![PathBuf::from("a").join("b.txt")]);
    assert_eq!(std::fs::read(dest.join("a").join("b.txt")).unwrap(), b"bee");
}
