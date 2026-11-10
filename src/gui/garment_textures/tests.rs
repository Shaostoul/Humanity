//! Settings > Graphics > Garment textures' levels and plan (2026-11-10): what each level draws, what
//! it takes, which it offers. What each test saw red first is at the test.

use super::packs::{GarmentPack, GarmentPackCatalog, PackGarment};
use super::*;

fn art(id: &str, kind: &str, shipped_px: u32, art_px: u32) -> GarmentArt {
    GarmentArt { id: id.into(), kind: kind.into(), shipped_px, art_px }
}

/// The shipped library's sizes in small: an outfit and a hair style made at 2048, shoes at 1024, a
/// brow at 512 (each kind as MakeHuman's are).
fn garments() -> Vec<GarmentArt> {
    vec![art("suit", "Clothes", 1024, 2048), art("hair", "Hair", 1024, 2048), art("shoes", "Shoes", 1024, 1024), art("brow", "Brows", 512, 512)]
}

fn catalog_2k() -> GarmentPackCatalog {
    GarmentPackCatalog {
        release: "assets-garments-1".into(),
        base_url: "http://127.0.0.1:9/".into(),
        packs: vec![GarmentPack {
            id: "garments-2k".into(),
            version: 1,
            px: 2048,
            file: "humanity-garments-2k-v1.tar".into(),
            bytes: 1000,
            sha256: "0".repeat(64),
            garments: ["suit", "hair"].iter().map(|id| PackGarment { id: (*id).into(), sha256: "0".repeat(64) }).collect(),
        }],
    }
}

/// THE LEVELS MEAN WHAT THEY SAY: the file offers 512, 1K, 2K and 4K, 1K by default, an unknown id
/// (the old "full") falls back to 1K, and a broken file offers 1K alone.
#[test]
fn the_levels_mean_what_they_say() {
    for id in ["512", "1k", "2k", "4k"] {
        assert!(is_level(id), "the file lists {id}");
    }
    assert_eq!((max_px("512"), max_px("1k"), max_px("2k"), max_px("4k")), (512, 1024, 2048, 4096));
    assert_eq!(level("full").id, DEFAULT_LEVEL, "an id the file no longer lists loads as the default");
    assert_eq!(parse(Some("{ not json")).levels.len(), 1);
    assert_eq!(parse(None).levels[0].max_px, 1024);
    assert!((warn_share() - 0.125).abs() < 1e-9, "the file's warn share");
}

/// A LEVEL ABOVE EVERY GARMENT'S ART IS NOT OFFERED, AND A GARMENT IS NEVER DRAWN ABOVE ITS OWN ART:
/// with art at 2048, 1024 and 512, the plan offers 512, 1K and 2K and leaves 4K out (no garment's
/// picture was made that large); at 2K the shoes are drawn at their 1024 and the brow at its 512, and
/// the plan says so (two garments at their own largest); every pool shape is one a garment is drawn
/// at, never larger than its art; 2K needs the 2K pack, the shipped sizes need none.
///
/// Seen red 2026-11-10 with the `reaching == 0` skip taken out of `plan_for` (every level in the
/// file offered, as a fixed list would): "4K is offered though no garment's art reaches it: an
/// upscale labelled as more".
#[test]
fn a_level_above_every_garments_art_is_not_offered() {
    let g = garments();
    let plan = plan_for(levels(), &g, 16, true, &catalog_2k());
    let ids: Vec<&str> = plan.levels.iter().map(|l| l.id.as_str()).collect();
    assert!(!ids.contains(&"4k"), "4K is offered though no garment's art reaches it: an upscale labelled as more ({ids:?})");
    assert_eq!(ids, vec!["512", "1k", "2k"]);
    let two = plan.level("2k").unwrap();
    assert_eq!(two.reaching, 2);
    assert_eq!(two.below, vec![(1024, 1), (512, 1)], "the shoes at their 1024, the brow at its 512");
    assert_eq!(two.packs, vec!["garments-2k".to_string()]);
    assert_eq!(two.download, 1000);
    assert!(plan.level("1k").unwrap().packs.is_empty() && plan.level("512").unwrap().packs.is_empty(), "the shipped sizes need no download");
    for l in &plan.levels {
        for x in &g {
            assert!(drawn_px(x, l.px) <= x.art_px, "{} drawn at {} over its art {} at {}", x.id, drawn_px(x, l.px), x.art_px, l.id);
        }
    }
    let shapes = pool_shapes(&g, 16, 2048, true);
    assert_eq!(shapes.iter().map(|(c, _)| c.height).collect::<Vec<_>>(), vec![2048, 1024, 512], "one shape a drawn size");
    // A garment whose art stops below the level reads its own largest file: the shoes' shipped file
    // at 1024, not a 2K one that does not exist.
    assert_eq!(source(&g[2], 2048, &catalog_2k()), Source::Shipped { max_px: 1024 });
    assert_eq!(source(&g[0], 2048, &catalog_2k()), Source::Pack { pack: "garments-2k".into(), px: 2048 });
    assert_eq!(source(&g[0], 512, &catalog_2k()), Source::Shipped { max_px: 512 });
}

/// THE LEVEL DRAWN WAITS FOR ITS FILES: 2K chosen and its pack not installed draws 1K (the largest
/// whose files are here); installed, 2K; an unpublished 2K (no pack in the catalog) is shown as not
/// published and never drawn; a saved 4K (not offered) draws the largest that is.
#[test]
fn the_level_drawn_waits_for_its_files() {
    let g = garments();
    let plan = plan_for(levels(), &g, 16, true, &catalog_2k());
    assert_eq!(effective(&plan, "2k", |_| false).unwrap().id, "1k");
    assert_eq!(effective(&plan, "2k", |id| id == "garments-2k").unwrap().id, "2k");
    assert_eq!(effective(&plan, "512", |_| true).unwrap().id, "512");
    assert_eq!(effective(&plan, "4k", |_| true).unwrap().id, "2k", "a saved level no garment reaches draws the largest offered");
    let bare = plan_for(levels(), &g, 16, true, &GarmentPackCatalog::default());
    let two = bare.level("2k").expect("offered: the art reaches it");
    assert!(!two.published, "no pack holds it");
    assert_eq!(effective(&bare, "2k", |_| true).unwrap().id, "1k", "an unpublished level is never drawn");
}

/// THE MEMORY IS THE POOLS': each level's figure is its pool shapes at their capacities (a 2K slot
/// of BC7 is 4096 by 2048 with ten levels), four times as much uncompressed; a near tier smaller than
/// the garments caps a shape at the near tier's worth of each kind.
#[test]
fn the_memory_is_the_pools() {
    let g = garments();
    let bc = plan_for(levels(), &g, 16, true, &catalog_2k());
    let raw = plan_for(levels(), &g, 16, false, &catalog_2k());
    let slot = |half: u32| SlotClass::for_half(half, true).slot_bytes();
    assert_eq!(bc.level("2k").unwrap().bytes, 2 * slot(2048) + slot(1024) + slot(512));
    assert_eq!(bc.level("1k").unwrap().bytes, 3 * slot(1024) + slot(512));
    assert_eq!(raw.level("2k").unwrap().bytes, 4 * bc.level("2k").unwrap().bytes);
    let many: Vec<GarmentArt> = (0..10).map(|k| art(&format!("s{k}"), "Clothes", 1024, 2048)).collect();
    assert_eq!(pool_shapes(&many, 2, 2048, true), vec![(SlotClass::for_half(2048, true), 3)], "a near tier of 2 and the player: 3 slots of the one kind");
}

/// THE CARD IS TOLD WHEN A LEVEL WILL NOT FIT, IN WORDS: a level whose pools take more than the
/// file's share of the card's own memory names what it needs, the card and what it has, and the
/// level that fits; a card with room, an integrated card or one that reports no memory gets none.
/// Seen red 2026-11-10 with `memory_notice` answering None always (as the page had no such notice
/// before the levels went past 1K): "a 2K level over an eighth of a 1 GB card is told so".
#[test]
fn a_level_too_big_for_the_card_is_told_so() {
    let g: Vec<GarmentArt> = (0..24).map(|k| art(&format!("s{k}"), if k % 2 == 0 { "Clothes" } else { "Hair" }, 1024, 2048)).collect();
    let plan = plan_for(levels(), &g, 16, true, &catalog_2k());
    let small = AdapterMemory { name: "Small GPU".into(), integrated: false, dedicated_bytes: Some(1 << 30) };
    let words = memory_notice(&plan, "2k", Some(&small)).expect("a 2K level over an eighth of a 1 GB card is told so");
    assert!(words.contains("Small GPU") && words.contains("2K"), "names the card and the level: {words}");
    assert!(words.contains("1K"), "says what works instead: {words}");
    let big = AdapterMemory { name: "Big GPU".into(), integrated: false, dedicated_bytes: Some(12 << 30) };
    assert!(memory_notice(&plan, "2k", Some(&big)).is_none());
    let shared = AdapterMemory { name: "Laptop GPU".into(), integrated: true, dedicated_bytes: None };
    assert!(memory_notice(&plan, "2k", Some(&shared)).is_none());
    assert!(memory_notice(&plan, "2k", None).is_none());
}

/// A GPU WITHOUT BLOCK COMPRESSION IS TOLD SO, IN WORDS: the notice names the card, what the
/// textures need and what works instead; a GPU with block compression, or a renderer not yet
/// started, shows none. Seen red 2026-11-10 with the notice never given once the GPU's answer is
/// known (as main gave none): "a GPU without block compression is told so".
#[test]
fn a_gpu_without_block_compression_is_told_so() {
    let card = AdapterMemory { name: "Test GPU".into(), integrated: true, dedicated_bytes: None };
    let words = hardware_notice(Some(false), Some(&card)).expect("a GPU without block compression is told so");
    assert!(words.contains("Test GPU"), "the notice names the card: {words}");
    assert!(words.contains("BC7"), "the notice says what the textures need: {words}");
    assert!(words.contains("512"), "the notice says what works instead: {words}");
    assert!(hardware_notice(Some(false), None).is_some(), "told so before the card has reported its name");
    assert!(hardware_notice(Some(true), Some(&card)).is_none(), "a GPU with block compression shows no notice");
    assert!(hardware_notice(None, Some(&card)).is_none(), "nothing to say before the renderer starts");
}

// ── The download, against a server on this computer (127.0.0.1, never the internet) ──

/// One ustar entry (the converter's layout, scripts/lib/ustar.js).
fn tar_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
    h[136..148].copy_from_slice(b"00000000000\0");
    h[148..156].copy_from_slice(b"        ");
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|&b| b as u32).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut out = h.to_vec();
    out.extend_from_slice(data);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

/// A small garment pack as the converter lays one out: its pack.ron and one garment's file (64 KB of
/// stand-in bytes, enough that a connection can drop in the middle).
fn fixture_pack() -> (Vec<u8>, GarmentPack) {
    let dds: Vec<u8> = (0..65536u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    let mut tar = tar_entry("pack.ron", b"(id: \"garments-test\", version: 1, instruments: [])\n");
    tar.extend(tar_entry("suit.dds", &dds));
    tar.extend_from_slice(&[0u8; 1024]);
    use sha2::{Digest, Sha256};
    let sha = hex::encode(Sha256::digest(&tar));
    let pack = GarmentPack { id: "garments-test".into(), version: 1, px: 2048, file: "humanity-garments-test-v1.tar".into(), bytes: tar.len() as u64, sha256: sha, garments: vec![PackGarment { id: "suit".into(), sha256: String::new() }] };
    (tar, pack)
}

/// A server on 127.0.0.1 serving `body` to `conns` GETs in turn: a `Range: bytes=N-` is answered 206
/// with the rest; when `cut` is Some(k), the FIRST answer declares the whole length but sends only k
/// bytes and closes (a connection dropping mid-download). Each request's Range header is recorded.
fn serve(body: Vec<u8>, cut: Option<usize>, conns: usize) -> (String, std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("its address");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for n in 0..conns {
            let Ok((mut s, _)) = listener.accept() else { return };
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => head.extend_from_slice(&buf[..k]),
                }
            }
            let text = String::from_utf8_lossy(&head).to_string();
            let range = text.lines().find_map(|l| l.strip_prefix("Range: ").or_else(|| l.strip_prefix("range: ")).map(|r| r.trim().to_string()));
            log.lock().unwrap().push(range.clone());
            let from = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.trim_end_matches('-').parse::<usize>().ok()).filter(|f| *f < body.len());
            let reply = match from {
                Some(f) => format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {f}-{}/{}\r\nConnection: close\r\n\r\n", body.len() - f, body.len() - 1, body.len()),
                None => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()),
            };
            let part = &body[from.unwrap_or(0)..];
            let _ = s.write_all(reply.as_bytes());
            let send = if n == 0 { cut.map_or(part.len(), |k| k.min(part.len())) } else { part.len() };
            let _ = s.write_all(&part[..send]);
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    });
    (format!("http://{addr}/"), seen)
}

/// A GARMENT PACK DOWNLOAD THAT DROPS RESUMES WHERE IT STOPPED, AND INSTALLS CHECKED: the first
/// connection drops after 20,000 bytes: the try fails as a network failure, those bytes are KEPT
/// (`partial_bytes`) and nothing is installed; the next try asks only for the rest ("bytes=20000-"),
/// the whole is checked against the catalog's SHA-256, and the pack is installed where the engine
/// reads a garment's file (`packs::garment_file`), with nothing left over. Removing it leaves nothing.
/// Seen red 2026-11-10 with the Range header left out of `download_and_install` (every try started
/// over): "resumed with a Range request" (left: [None, None], right: [None, Some("bytes=20000-")]).
#[test]
fn a_dropped_garment_pack_download_resumes_and_installs_checked() {
    let data = humanity_core::test_temp::dir("garment_pack_resume");
    let dir = packs::packs_dir(&data);
    let (tar, pack) = fixture_pack();
    let (base, seen) = serve(tar.clone(), Some(20_000), 2);
    assert!(base.starts_with("http://127.0.0.1:"), "served from this computer: {base}");
    let entry = pack.install_entry();
    let first = install::download_and_install(&pack.url(&base), &entry, &dir, &install::new_progress());
    assert!(matches!(first, Err(install::Failure::Network(_))), "a dropped connection is a network failure: {first:?}");
    assert_eq!(install::partial_bytes(&dir, &pack.id), 20_000, "what arrived is kept to resume");
    assert!(install::installed_packs(&dir).is_empty(), "nothing installed from half a pack");
    let progress = install::new_progress();
    install::download_and_install(&pack.url(&base), &entry, &dir, &progress).expect("the second try finishes and installs");
    assert_eq!(seen.lock().unwrap().clone(), vec![None, Some("bytes=20000-".to_string())], "resumed with a Range request");
    assert_eq!(progress.lock().unwrap().done, tar.len() as u64, "every byte counted");
    assert_eq!(install::installed_packs(&dir).get(&pack.id).map(|p| p.version), Some(1), "installed at its version");
    assert!(packs::garment_file(&data, &pack.id, "suit").is_file(), "the garment's file is where the engine reads it");
    assert_eq!(install::partial_bytes(&dir, &pack.id), 0, "nothing left to resume");
    install::remove_pack(&dir, &pack.id).expect("removes");
    assert!(std::fs::read_dir(&dir).map(|r| r.count()).unwrap_or(0) == 0, "removed cleanly");
}

/// A DAMAGED GARMENT PACK IS REFUSED: the archive with one byte changed (the same length) fails its
/// SHA-256, is deleted and installs nothing; a server whose file is another size is refused before
/// a byte is kept; and a partial left by a dropped download is not trusted past the check (it resumes
/// into the same refusal when the rest does not match).
/// Seen red 2026-11-10 with the SHA-256 comparison taken out of `download_and_install`: "a changed
/// byte is refused as damaged: Ok(())".
#[test]
fn a_damaged_garment_pack_is_refused() {
    let data = humanity_core::test_temp::dir("garment_pack_damaged");
    let dir = packs::packs_dir(&data);
    let (tar, pack) = fixture_pack();
    let entry = pack.install_entry();
    let mut bad = tar.clone();
    bad[30_000] ^= 0x5a;
    let (base, _) = serve(bad.clone(), None, 1);
    let got = install::download_and_install(&pack.url(&base), &entry, &dir, &install::new_progress());
    assert!(matches!(got, Err(install::Failure::Damaged(_))), "a changed byte is refused as damaged: {got:?}");
    assert!(install::installed_packs(&dir).is_empty() && install::partial_bytes(&dir, &pack.id) == 0, "nothing installed, nothing kept");
    let mut longer = tar.clone();
    longer.extend_from_slice(&[0u8; 512]);
    let (base, _) = serve(longer, None, 1);
    let got = install::download_and_install(&pack.url(&base), &entry, &dir, &install::new_progress());
    assert!(matches!(got, Err(install::Failure::Damaged(_))), "a file of another size is refused: {got:?}");
    assert_eq!(install::partial_bytes(&dir, &pack.id), 0);
    // Half of a damaged file arrives, the connection drops; the rest arrives on the next try and the
    // whole fails its check, deleted.
    let (base, _) = serve(bad, Some(40_000), 2);
    let _ = install::download_and_install(&pack.url(&base), &entry, &dir, &install::new_progress());
    assert_eq!(install::partial_bytes(&dir, &pack.id), 40_000);
    let got = install::download_and_install(&pack.url(&base), &entry, &dir, &install::new_progress());
    assert!(matches!(got, Err(install::Failure::Damaged(_))), "the resumed whole fails its check: {got:?}");
    assert!(install::installed_packs(&dir).is_empty() && install::partial_bytes(&dir, &pack.id) == 0);
}

/// THE COMPILED-IN CATALOG IS THE CONVERTER'S: it reads, names its release under a non-`v` tag, and
/// every pack is built (a size and a SHA-256 to check a download against) with garments at its size.
#[test]
fn the_compiled_in_catalog_reads() {
    let c = packs::catalog();
    assert!(!c.release.starts_with('v'), "an asset release never takes a game tag: {}", c.release);
    assert!(c.base_url.ends_with(&format!("/{}/", c.release)));
    for p in &c.packs {
        assert!(p.bytes > 0 && p.sha256.len() == 64, "{} is built", p.id);
        assert!(!p.garments.is_empty() && p.px > 1024, "{} holds garments above the shipped size", p.id);
        assert_eq!(p.install_entry().id, p.id);
    }
}
