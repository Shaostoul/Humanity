// Child of net/held.rs (#[path]): the desktop app's hide and keep rules, with no socket and no
// GUI. Each test was seen red once on purpose, recorded at the test.

use super::*;
use humanity_protocol::held_seal::{part_context, seal_bytes, HeldFile, Manifest, MANIFEST_VERSION};
use humanity_protocol::pq_crypto::derive_kyber_seed;

pub(crate) const WORDS: &str = "HELD-WORDS-91c2 the post's words";
pub(crate) const PIC: &[u8] = b"HELD-PIC-5e0a picture bytes";
pub(crate) const PDF: &[u8] = b"HELD-PDF-77b1 members-only bytes";

pub(crate) fn owner_keys(seed: u8) -> KyberKeypair {
    KyberKeypair::from_seed(&derive_kyber_seed(&[seed; 32])).unwrap()
}

/// A package as the relay seals it: part 0 the manifest (with its restore key), part 1 a public
/// picture, part 2 a members-only file named with a folder escape in its original name.
pub(crate) fn package(kp: &KyberKeypair, pkg: &str) -> Vec<Vec<u8>> {
    let file = |part: usize, name: &str, folder: &str, original: &str, bytes: &[u8]| HeldFile {
        part,
        name: name.into(),
        folder: folder.into(),
        uploader: "aa".into(),
        shelf: 0,
        original_name: original.into(),
        bytes: bytes.len() as u64,
        hash: blake3::hash(bytes).to_hex().to_string(),
        poster_part: None,
        poster_bytes: 0,
        poster_hash: String::new(),
    };
    let m = Manifest {
        v: MANIFEST_VERSION,
        package: pkg.into(),
        report_id: 4,
        author: "aa".into(),
        stamp: 42,
        channel: "global".into(),
        words: WORDS.into(),
        files: vec![file(1, "1_x_pic.png", "public", "pic.png", PIC), file(2, "2_x_doc.pdf", "members", "../../evil/doc.pdf", PDF)],
        held_by: "bb".into(),
        held_at: 1,
        restore_key: "ab".repeat(32),
    };
    let json = serde_json::to_vec(&m).unwrap();
    [json.as_slice(), PIC, PDF].iter().enumerate().map(|(n, plain)| seal_bytes(&[&kp.public_key()], &part_context(pkg, n), plain).unwrap()).collect()
}

/// Read a stored .zip back: its entries' names and bytes, each CRC checked.
pub(crate) fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let end = (0..bytes.len() - 3).rev().find(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).expect("an end record");
    let count = u16_at(end + 10);
    let mut at = u32_at(end + 16) as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        assert_eq!(u32_at(at), 0x0201_4b50, "a central entry");
        let (crc, size, name_len, local) = (u32_at(at + 16), u32_at(at + 24) as usize, u16_at(at + 28), u32_at(at + 42) as usize);
        let name = String::from_utf8(bytes[at + 46..at + 46 + name_len].to_vec()).unwrap();
        assert_eq!(u32_at(local), 0x0403_4b50, "its local header");
        assert_eq!(u32_at(local + 14), crc, "the local header carries the CRC written back");
        let data_at = local + 30 + u16_at(local + 26) + u16_at(local + 28);
        let data = bytes[data_at..data_at + size].to_vec();
        assert_eq!(crc32_update(0, &data), crc, "{name}'s CRC-32");
        out.push((name, data));
        at += 46 + name_len + u16_at(at + 30) + u16_at(at + 32);
    }
    out
}

/// EXPORT WRITES ONE .ZIP, ONLY WHERE THE OWNER CHOSE, AND KEEPS THE RESTORE KEY OUT OF IT: the
/// list (who, when, which files), the words and every file byte for byte under a name with no
/// folder escape; nothing else is written into the folder; an existing file is never written over
/// (the next name is offered); and a part that does not open leaves no file behind.
///
/// Seen red 2026-11-20 with `ZipOut::add` not writing the CRC back into the local header: "the
/// local header carries the CRC written back" (left 0).
#[test]
fn export_writes_one_zip_only_where_chosen_without_the_restore_key() {
    let kp = owner_keys(9);
    let parts = package(&kp, "pkg");
    let dir = humanity_core::test_temp::dir("held_export");
    let path = export_path(dir.path(), 7);
    assert_eq!(path.file_name().unwrap(), "held-item-7.zip");
    let fetch = |p: usize| -> Result<Box<dyn Read>, String> { Ok(Box::new(std::io::Cursor::new(parts[p].clone()))) };
    let done = export_with(&kp, &chosen("pkg", 4), &path, 99, fetch).unwrap();
    assert_eq!(done, Exported { path: path.clone(), files: 2 });
    let in_folder: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(in_folder, vec![std::ffi::OsString::from("held-item-7.zip")], "only the chosen file is written");
    let bytes = std::fs::read(&path).unwrap();
    let zip = unzip(&bytes);
    let names: Vec<&str> = zip.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["held-item.json", "post.txt", "files/1-pic.png", "files/2-doc.pdf"]);
    assert_eq!(zip[1].1, WORDS.as_bytes());
    assert_eq!((zip[2].1.as_slice(), zip[3].1.as_slice()), (PIC, PDF));
    let about: serde_json::Value = serde_json::from_slice(&zip[0].1).unwrap();
    assert_eq!((about["report_id"].as_i64(), about["exported_at"].as_i64()), (Some(4), Some(99)));
    assert!(about.get("restore_key").is_none() && about.get("words").is_none());
    assert!(!bytes.windows(64).any(|w| w == "ab".repeat(32).as_bytes()), "the restore key is nowhere in the file");

    assert_eq!(export_path(dir.path(), 7).file_name().unwrap(), "held-item-7-2.zip", "an existing file is never written over");
    let second = export_path(dir.path(), 7);
    let mut broken = parts.clone();
    let last = broken[2].len() - 1;
    broken[2][last] ^= 1;
    let fetch = |p: usize| -> Result<Box<dyn Read>, String> { Ok(Box::new(std::io::Cursor::new(broken[p].clone()))) };
    assert!(export_with(&kp, &chosen("pkg", 4), &second, 99, fetch).is_err(), "a part that does not open fails the export");
    assert!(!second.exists(), "and leaves no file behind");
    let other = owner_keys(8);
    let fetch = |p: usize| -> Result<Box<dyn Read>, String> { Ok(Box::new(std::io::Cursor::new(parts[p].clone()))) };
    assert!(export_with(&other, &chosen("pkg", 4), &second, 99, fetch).is_err() && !second.exists(), "another key opens nothing and writes nothing");
    // W7 (the third review): the copy that opened must be the item chosen (report #4 here): another
    // report is refused with words and no file is written. Seen red 2026-11-21 before export_with
    // compared the manifest: the export of "report #5" wrote report #4's file.
    let fetch = |p: usize| -> Result<Box<dyn Read>, String> { Ok(Box::new(std::io::Cursor::new(parts[p].clone()))) };
    let wrong = export_with(&kp, &chosen("pkg", 5), &second, 99, fetch);
    assert!(wrong.as_ref().is_err_and(|w| w.contains("not the item you chose") && w.contains("report #4")), "{wrong:?}");
    assert!(!second.exists(), "and no file is written");
    // The fourth review's finding 8: a SIBLING post of the same report (same package and report as
    // the relay listed them, another post time or author) is refused too. Seen red 2026-11-21
    // (scratchpad ownfix7-batch1.log, mutant P-2 of ownfix6-rust-mutants.json) with manifest_is
    // comparing the package and report alone (the round-4 rule): "a sibling post is refused" failed,
    // the sibling's export written (Ok(Exported { .., files: 2 })).
    for sibling in [held_seal::Chosen { stamp: 43, ..chosen("pkg", 4) }, held_seal::Chosen { author: "bb", ..chosen("pkg", 4) }] {
        let fetch = |p: usize| -> Result<Box<dyn Read>, String> { Ok(Box::new(std::io::Cursor::new(parts[p].clone()))) };
        let wrong = export_with(&kp, &sibling, &second, 99, fetch);
        assert!(wrong.as_ref().is_err_and(|w| w.contains("another post of that report")), "a sibling post is refused: {wrong:?}");
        assert!(!second.exists(), "and no file is written");
    }
}

/// The item the owner chose, as the test package's list would show it (author "aa", post time 42).
fn chosen(package: &str, report_id: i64) -> held_seal::Chosen<'_> {
    held_seal::Chosen { package, report_id, author: "aa", stamp: 42 }
}

/// THE CLIENT'S RULES: the list read from the relay's frame (the package id only when sent), the
/// signed action naming its server, target and reason and verifying, the hold's end, the addresses,
/// and which private lines are about held items.
///
/// Seen red 2026-11-20 with `hold_until` taking 367 days: "no hold past 366 days" (left Some).
#[test]
fn the_list_the_signed_actions_and_the_hold_follow_the_protocol() {
    let frame = serde_json::json!({
        "type": "held_items", "can_keep": false, "cannot_keep_why": "This server has no owner set.", "i_am_owner": true,
        "items": [{ "id": 3, "report_id": 9, "file_count": 2, "parts": 3, "sealed_bytes": 2048, "held_at": 1, "held_by_name": "Ada",
                    "delete_after": 5, "legal_hold_until": 0, "deleted_at": 5, "exported_count": 1, "sealed_to_owner": true, "package": "pkg" }],
        "actions": [{ "action": "hide_and_keep", "target": 9, "signer_name": "Ada", "reason": "the policy", "until": 0, "signed_at": 1 }],
        "counts": { "held": 1, "exported": 1, "legal_holds": 0, "destroyed": 0, "restored": 0 },
    });
    let l = parse_list(&frame).unwrap();
    assert!(!l.can_keep && l.i_am_owner && l.cannot_keep_why.contains("no owner"));
    assert_eq!((l.items[0].id, l.items[0].package.as_str(), l.items[0].sealed_to_owner), (3, "pkg", true));
    assert_eq!((l.actions[0].action.as_str(), l.counts.exported), ("hide_and_keep", 1));
    assert!(parse_list(&serde_json::json!({ "type": "reports" })).is_none());

    let seed = [3u8; 32];
    let s = sign(&seed, "did:hum:test", "held_legal_hold", 3, "  a preservation request  ", 1_000, 5).unwrap();
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let o = submission_object(&s.submission);
    assert!(o.verify_signature().is_ok(), "the signed action verifies");
    assert_eq!(o.object_id().unwrap().to_hex(), s.object_id, "its id is the grant");
    assert_eq!(o.space_id.as_deref(), Some("did:hum:test"));
    let payload: ciborium::Value = ciborium::from_reader(&o.payload[..]).unwrap();
    let text = format!("{payload:?}");
    assert!(text.contains("held_legal_hold") && text.contains("\"3\"") && text.contains("a preservation request") && text.contains("held"), "{text}");
    assert_eq!(B64.decode(s.submission["author_public_key_b64"].as_str().unwrap()).unwrap(), o.author_public_key);
    assert!(sign(&seed, "did:hum:test", "ban", 3, "r", 0, 5).is_err(), "only hide and keep's actions");
    assert!(sign(&seed, "", "held_destroy", 3, "r", 0, 5).is_err(), "never without the server's identity");
    assert!(sign(&seed, "did:hum:test", "held_destroy", 3, "   ", 0, 5).is_err(), "never without a reason");

    assert_eq!(hold_until(0, 10), Some(0));
    assert_eq!(hold_until(1, 10), Some((10 + DAY_MS) as u64));
    assert_eq!(hold_until(366, 0), Some((366 * DAY_MS) as u64));
    assert_eq!(hold_until(367, 0), None, "no hold past 366 days");
    assert_eq!(hold_until(-1, 0), None);
    assert_eq!(part_url("https://h/", 3, 1, "g"), "https://h/api/held/3/1?grant=g");
    assert_eq!(restore_url("https://h", 3, "g"), "https://h/api/held/3/restore?grant=g");
    assert!(is_held_words("Held item #3 is under a legal hold.") && is_held_words("Nothing was done: only this server's owner can do that."));
    assert!(!is_held_words("Report #4 decided: dismiss."));
    assert_eq!(safe_entry_name("../../evil/doc.pdf"), "doc.pdf");
    assert_eq!(safe_entry_name("..hidden"), "_hidden");
    assert_eq!(safe_entry_name("a:b?.txt"), "a_b_.txt");
    assert_eq!(safe_entry_name(""), "file");
}

/// The object a submission JSON carries (the protocol's reading of the shape
/// `POST /api/v2/objects` takes; the client may not reach the relay's own type).
pub(crate) fn submission_object(sub: &serde_json::Value) -> humanity_protocol::object::Object {
    humanity_protocol::org::object_from_json(sub).unwrap()
}
