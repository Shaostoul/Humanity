//! Settings > AI's downloads, against a server on this computer (127.0.0.1, never the internet):
//! a model and the engine fetched, checked and installed; a download stopped half way carried on
//! with an HTTP Range request; a file that does not match refused and deleted; parts joined and checked
//! whole; Remove and Remove all leaving nothing; the engine zip read strictly. And the catalog the app
//! is built with. What each test saw red first is at the test.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use super::catalog::{AiCatalog, EngineEntry, Extra, ModelEntry, Part};
use super::install::{self, Failure, Stage};

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// A stored zip, the way scripts/ai-engine.js writes the engine's (the reader's own format).
fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = super::zip::crc32(data);
        let mut local = Vec::new();
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&[20, 0, 0, 8, 0, 0, 0, 0, 33, 0]);
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(name.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&local);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 8, 0, 0, 0, 0, 33, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// How the test server behaves.
#[derive(Clone, Copy, PartialEq)]
enum Serve {
    Normal,
    /// The first request for the model file sends this many bytes, then hangs up.
    CutFirstAfter(usize),
    /// The model file arrives with one byte changed.
    Corrupt,
}

/// A server on 127.0.0.1 serving `files` by name, with `Range: bytes=N-` answered 206. Records each
/// request's path and Range header.
struct TestServer {
    base: String,
    seen: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

fn serve(files: Vec<(String, Vec<u8>)>, mode: Serve, model_file: String) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("its address");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let model_hits = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buf[..n]),
                }
            }
            let text = String::from_utf8_lossy(&head).to_string();
            let name = text.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
            let range = text.lines().find_map(|l| l.strip_prefix("Range: ").or_else(|| l.strip_prefix("range: ")).map(|r| r.trim().to_string()));
            log.lock().unwrap().push((name.clone(), range.clone()));
            let Some((_, body)) = files.iter().find(|(n, _)| *n == name) else {
                let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            };
            let mut body = body.clone();
            if name == model_file && mode == Serve::Corrupt {
                let mid = body.len() / 2;
                body[mid] ^= 0xff;
            }
            let from = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.trim_end_matches('-').parse::<usize>().ok()).filter(|f| *f < body.len());
            let (status, slice, extra) = match from {
                Some(f) => ("206 Partial Content", &body[f..], format!("Content-Range: bytes {f}-{}/{}\r\n", body.len() - 1, body.len())),
                None => ("200 OK", &body[..], String::new()),
            };
            let cut = match mode {
                Serve::CutFirstAfter(n) if name == model_file && model_hits.fetch_add(1, Ordering::SeqCst) == 0 => Some(n),
                _ => None,
            };
            let reply = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", slice.len());
            let _ = s.write_all(reply.as_bytes());
            let _ = s.write_all(&slice[..cut.unwrap_or(slice.len()).min(slice.len())]);
            let _ = s.flush();
        }
    });
    TestServer { base: format!("http://{addr}/"), seen }
}

/// A test catalog: an engine zip holding a stand-in program, a model of `model_len` bytes (in `parts`
/// pieces when more than one), its licence and README; and the files to serve.
fn fixture(model_len: usize, parts: usize) -> (AiCatalog, Vec<(String, Vec<u8>)>) {
    let zip = stored_zip(&[(super::paths::ENGINE_EXE, b"not really a program"), ("licenses/llama.cpp-LICENSE.txt", b"MIT")]);
    let model: Vec<u8> = (0..model_len).map(|i| (i * 31 % 251) as u8).collect();
    let licence = b"the licence".to_vec();
    let readme = b"the credit".to_vec();
    let mut files = vec![("engine.zip".to_string(), zip.clone()), ("model-LICENSE.txt".to_string(), licence.clone()), ("model-README.md".to_string(), readme.clone())];
    let mut part_list = Vec::new();
    if parts > 1 {
        let size = model_len.div_ceil(parts);
        for (i, chunk) in model.chunks(size).enumerate() {
            let name = format!("model.gguf.part{}of{parts}", i + 1);
            part_list.push(Part { file: name.clone(), bytes: chunk.len() as u64, sha256: sha(chunk) });
            files.push((name, chunk.to_vec()));
        }
    } else {
        files.push(("model.gguf".to_string(), model.clone()));
    }
    let cat = AiCatalog {
        release: "assets-ai-test".into(),
        base_url: "https://example.invalid/".into(),
        engine: EngineEntry {
            version: super::paths::ENGINE_VERSION.into(),
            platform: "windows-x64".into(),
            file: "engine.zip".into(),
            bytes: zip.len() as u64,
            sha256: sha(&zip),
            parts: vec![],
            sources: vec!["test".into()],
            licence: "ai.licence.engine".into(),
            licence_text: "ai/licences/llama.cpp-mit.txt".into(),
            credits: vec!["llama_cpp".into()],
        },
        models: vec![ModelEntry {
            id: "test-model".into(),
            choice: "characters-quick".into(),
            kind: "brain".into(),
            model: "Test Model".into(),
            maker: "Nobody".into(),
            licence: "ai.licence.apache2".into(),
            licence_text: "ai/licences/apache-2.0.txt".into(),
            finding: "docs/reference/findings/2026-11-08-granite-4-0-1b.md".into(),
            credits: vec!["granite_4_0_1b".into()],
            upstream: "https://example.invalid".into(),
            upstream_sha256: sha(&model),
            file: "model.gguf".into(),
            bytes: model.len() as u64,
            sha256: sha(&model),
            parts: part_list,
            extras: vec![
                Extra { file: "model-LICENSE.txt".into(), install_as: "LICENSE.txt".into(), bytes: licence.len() as u64, sha256: sha(&licence) },
                Extra { file: "model-README.md".into(), install_as: "README.md".into(), bytes: readme.len() as u64, sha256: sha(&readme) },
            ],
            context: 4096,
        }],
    };
    (cat, files)
}

/// Every path under a folder, relative, sorted (files and folders).
fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                out.push(p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/"));
                if p.is_dir() {
                    walk(base, &p, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn ai_root(t: &Path) -> PathBuf {
    t.join("ai")
}

/// THE BUILT-IN CATALOG reads, keeps every rule, and lists Granite 4.0 1B as IBM's own file (its hash
/// is the publisher's) for the Quick choice, and the engine of this app's version.
#[test]
fn the_built_in_catalog_reads_and_lists_granite_and_the_engine() {
    let cat = AiCatalog::parse(super::catalog::BUILT_IN).expect("the built-in catalog");
    let g = cat.model("granite-4-0-1b").expect("Granite is listed");
    assert_eq!((g.bytes, g.sha256.as_str()), (1_023_645_440, "22ec0f9cc99a90185312de3c882c84e7bd6789bdd050389844380a01a831d7f1"));
    assert_eq!(g.sha256, g.upstream_sha256, "IBM's own file, unchanged");
    assert_eq!(cat.for_choice("characters-quick").map(|m| m.id.as_str()), Some("granite-4-0-1b"));
    assert_eq!(cat.engine.version, super::paths::ENGINE_VERSION);
    assert!(cat.engine.file.ends_with("-windows-x64.zip"));
    let choices = super::choices::load(&humanity_core::storage::data_dir()).expect("choices.ron");
    for m in &cat.models {
        assert!(choices.choice(&m.choice).is_some(), "{} fills a choice of data/ai/choices.ron", m.id);
        assert!(humanity_core::storage::data_dir().join(&m.licence_text).is_file(), "{}'s licence text ships with the app", m.id);
    }
}

/// The catalog's rules refuse a missing hash, a file of 2 GiB or more not in parts, parts that do not add
/// up, a name that reaches outside its folder. Seen red 2026-11-09 with the parts' sum left unchecked
/// in `problems`: "parts that do not add up are refused: []".
#[test]
fn the_catalog_refuses_what_it_cannot_vouch_for() {
    let (good, _) = fixture(1000, 1);
    assert!(good.problems().is_empty(), "{:?}", good.problems());
    let mut no_hash = good.clone();
    no_hash.models[0].sha256 = String::new();
    assert!(no_hash.problems().iter().any(|p| p.contains("not a SHA-256")));
    let mut big = good.clone();
    big.models[0].bytes = 3 << 30;
    assert!(big.problems().iter().any(|p| p.contains("must come in parts")), "{:?}", big.problems());
    let (mut parted, _) = fixture(1000, 3);
    parted.models[0].bytes = 3 << 30;
    let p = parted.problems();
    assert!(p.iter().any(|x| x.contains("do not add up")), "parts that do not add up are refused: {p:?}");
    let mut escape = good.clone();
    escape.models[0].extras[0].install_as = "..\\evil.dll".into();
    assert!(escape.problems().iter().any(|p| p.contains("not a plain file name")));
}

/// A MODEL AND THE ENGINE INSTALL FROM A LOOPBACK SERVER: the engine first (it comes with the first
/// model), unpacked into engine/<version>/; the model with its licence and README into models/<id>/;
/// nothing left in downloads/; the progress counts every byte of both. Seen red 2026-11-09 with the
/// engine step left out of `install_model` (the total counted the engine, which never came): "every
/// byte counted" (left: (Done, 300021), right: (Done, 300308)).
#[test]
fn a_model_and_the_engine_install_from_a_loopback_server() {
    let t = humanity_core::test_temp::dir("ai_install");
    let root = ai_root(&t);
    let (cat, files) = fixture(300_000, 1);
    let server = serve(files, Serve::Normal, "model.gguf".into());
    assert!(server.base.starts_with("http://127.0.0.1:"), "served from this computer: {}", server.base);
    let total = install::download_total(&cat, &cat.models[0], &root);
    assert_eq!(total, cat.engine.bytes + cat.models[0].download_bytes(), "the engine's size is in the total before the click");
    let job = install::new_job(total);
    install::install_model(&server.base, &cat, "test-model", &root, &job).expect("it installs");
    let p = job.snapshot();
    assert_eq!((p.stage, p.done), (Stage::Done, total), "every byte counted");
    let have = install::installed(&cat, &root);
    assert!(have.engine.is_some(), "the engine came with the first model");
    assert!(have.models.contains_key("test-model"));
    assert_eq!(tree(&root), ["engine", "engine/1.0.0", "engine/1.0.0/humanity-ai.exe", "engine/1.0.0/licenses", "engine/1.0.0/licenses/llama.cpp-LICENSE.txt", "models", "models/test-model", "models/test-model/LICENSE.txt", "models/test-model/README.md", "models/test-model/model.gguf"].map(String::from).to_vec(), "nothing else, no downloads folder left");
    assert_eq!(install::download_total(&cat, &cat.models[0], &root), cat.models[0].download_bytes(), "the engine is not counted again");
}

/// A DOWNLOAD STOPPED HALF WAY CARRIES ON: the server hangs up after 100 KB; the part stays; Get again
/// asks for "bytes=102400-" and only the rest arrives; the file checks whole. Seen red 2026-11-09 with
/// the Range header left out of `fetch` (every Get started over): "resumed with a Range request"
/// (left: [None, None], right: [None, Some("bytes=102400-")]).
#[test]
fn a_stopped_download_carries_on_with_a_range_request() {
    let t = humanity_core::test_temp::dir("ai_resume");
    let root = ai_root(&t);
    let (cat, files) = fixture(300_000, 1);
    let server = serve(files, Serve::CutFirstAfter(102_400), "model.gguf".into());
    let job = install::new_job(install::download_total(&cat, &cat.models[0], &root));
    let first = install::install_model(&server.base, &cat, "test-model", &root, &job);
    assert!(matches!(first, Err(Failure::Network(_))), "the hang-up is a network failure: {first:?}");
    let part = install::downloads_dir(&root).join("model.gguf.part");
    assert_eq!(std::fs::metadata(&part).map(|m| m.len()).ok(), Some(102_400), "what arrived is kept");
    let job = install::new_job(install::download_total(&cat, &cat.models[0], &root));
    install::install_model(&server.base, &cat, "test-model", &root, &job).expect("the second Get finishes");
    let model_requests: Vec<Option<String>> = server.seen.lock().unwrap().iter().filter(|(n, _)| n == "model.gguf").map(|(_, r)| r.clone()).collect();
    assert_eq!(model_requests, vec![None, Some("bytes=102400-".to_string())], "resumed with a Range request");
    assert!(install::installed(&cat, &root).models.contains_key("test-model"));
    install::verify(&install::model_file(&root, &cat.models[0]), cat.models[0].bytes, &cat.models[0].sha256).expect("the joined-up file is the published one");
}

/// A FILE THAT DOES NOT MATCH IS REFUSED AND DELETED (5.2): one byte changed on the way; the failure
/// is Damaged, its words the plain ones, the half file gone, the model not installed. Seen red
/// 2026-11-09 with the SHA-256 comparison left out of `fetch`: "a changed byte is refused" (Ok(())).
#[test]
fn a_download_that_does_not_match_is_refused_and_deleted() {
    let t = humanity_core::test_temp::dir("ai_damaged");
    let root = ai_root(&t);
    let (cat, files) = fixture(200_000, 1);
    let server = serve(files, Serve::Corrupt, "model.gguf".into());
    let job = install::new_job(install::download_total(&cat, &cat.models[0], &root));
    let r = install::install_model(&server.base, &cat, "test-model", &root, &job);
    assert!(matches!(&r, Err(Failure::Damaged(d)) if d.contains("SHA-256")), "a changed byte is refused: {r:?}");
    assert_eq!(r.unwrap_err().words_key(), "ai.download.damaged");
    assert!(!install::downloads_dir(&root).join("model.gguf.part").exists(), "deleted");
    assert!(!install::installed(&cat, &root).models.contains_key("test-model"));
    assert!(matches!(job.snapshot().stage, Stage::Failed(Failure::Damaged(_))));
    let words = humanity_core::i18n::t("ai.download.damaged");
    assert!(words.contains("did not match what we published"), "{words}");
}

/// PARTS ARE JOINED AND CHECKED WHOLE (6.2): a model published in three parts downloads part by part,
/// each checked, then joined and checked against the whole file's hash; the parts are gone after.
/// And joining parts in the wrong order is refused and leaves nothing. Seen red 2026-11-09 with the
/// whole-file comparison weakened in `join_parts`: "parts in the wrong order are refused:
/// Ok("...\join\model.gguf.joined")".
#[test]
fn parts_are_joined_and_checked_whole() {
    let t = humanity_core::test_temp::dir("ai_parts");
    let root = ai_root(&t);
    let (cat, files) = fixture(250_000, 3);
    assert_eq!(cat.models[0].parts.len(), 3);
    let server = serve(files, Serve::Normal, "model.gguf".into());
    let job = install::new_job(install::download_total(&cat, &cat.models[0], &root));
    install::install_model(&server.base, &cat, "test-model", &root, &job).expect("the parts install");
    install::verify(&install::model_file(&root, &cat.models[0]), cat.models[0].bytes, &cat.models[0].sha256).expect("joined whole");
    let fetched: Vec<String> = server.seen.lock().unwrap().iter().map(|(n, _)| n.clone()).filter(|n| n.starts_with("model.gguf")).collect();
    assert_eq!(fetched, ["model.gguf.part1of3", "model.gguf.part2of3", "model.gguf.part3of3"], "part by part, never the whole");

    let scratch = t.join("join");
    std::fs::create_dir_all(&scratch).unwrap();
    let m = &cat.models[0];
    let model: Vec<u8> = (0..250_000usize).map(|i| (i * 31 % 251) as u8).collect();
    let size = model.len().div_ceil(3);
    let mut paths: Vec<PathBuf> = model
        .chunks(size)
        .enumerate()
        .map(|(i, c)| {
            let p = scratch.join(format!("p{i}"));
            std::fs::write(&p, c).unwrap();
            p
        })
        .collect();
    paths.swap(0, 1);
    let r = install::join_parts(&paths, "model.gguf", m.bytes, &m.sha256, &scratch, &install::new_job(0));
    assert!(matches!(r, Err(Failure::Damaged(_))), "parts in the wrong order are refused: {r:?}");
    assert!(tree(&scratch).is_empty(), "nothing is left: {:?}", tree(&scratch));
}

/// MINISTRAL 3 3B INSTALLS IN ITS PARTS (2026-11-11): the built-in catalog lists Mistral AI's own file
/// (its hash is Mistral's) for the Rich choice, in two parts because it is over PART_LIMIT; a small
/// stand-in with the catalog's own file name, part names and part count, its licence and README under
/// their release names, downloads from a 127.0.0.1 server part by part in the catalog's order, is
/// joined and checked whole, and lands in models/ministral-3-3b/ with LICENSE.txt and README.md and
/// nothing left in downloads/. Never the real 2 GB. Seen red 2026-11-11 against the catalog before
/// Ministral's row: "Ministral is listed".
#[test]
fn ministral_installs_in_its_parts_from_a_loopback_server() {
    let real = AiCatalog::parse(super::catalog::BUILT_IN).expect("the built-in catalog");
    let m = real.model("ministral-3-3b").expect("Ministral is listed").clone();
    assert_eq!((m.bytes, m.sha256.as_str()), (2_147_023_008, "9ed150d4367e68df0ac8e1540f6ddc65b42d0ee26378329d1ecbca60f93fc5f8"));
    assert_eq!(m.sha256, m.upstream_sha256, "Mistral AI's own file, unchanged");
    assert_eq!((m.maker.as_str(), m.licence.as_str()), ("Mistral AI", "ai.licence.apache2"));
    assert_eq!(real.for_choice("characters-rich").map(|x| x.id.as_str()), Some("ministral-3-3b"), "the Rich choice, beside Granite's Quick");
    assert_eq!(m.parts.len(), 2, "over PART_LIMIT, so in parts: {:?}", m.parts);
    assert!(m.parts.iter().all(|p| p.bytes <= super::catalog::PART_LIMIT) && m.parts.iter().map(|p| p.bytes).sum::<u64>() == m.bytes);
    // The stand-in: the test fixture split into Ministral's part count, renamed to its release names.
    let (mut cat, files) = fixture(250_000, m.parts.len());
    let rename = |n: &str| -> String {
        if let Some(i) = n.strip_prefix("model.gguf.part").and_then(|r| r.split("of").next()).and_then(|i| i.parse::<usize>().ok()) {
            m.parts[i - 1].file.clone()
        } else if n == "model-LICENSE.txt" {
            m.extras[0].file.clone()
        } else if n == "model-README.md" {
            m.extras[1].file.clone()
        } else {
            n.to_string()
        }
    };
    let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|(n, b)| (rename(&n), b)).collect();
    let t = &mut cat.models[0];
    t.id = m.id.clone();
    t.choice = m.choice.clone();
    t.file = m.file.clone();
    for p in &mut t.parts {
        p.file = rename(&p.file);
    }
    for x in &mut t.extras {
        x.file = rename(&x.file);
    }
    // (The stand-in breaks one catalog rule on purpose: it is far under PART_LIMIT yet in parts, so
    // `problems()` would refuse it; the installer takes the parts as listed, which is what is tested.)
    let t = humanity_core::test_temp::dir("ai_ministral");
    let root = ai_root(&t);
    let server = serve(files, Serve::Normal, m.file.clone());
    let job = install::new_job(install::download_total(&cat, &cat.models[0], &root));
    install::install_model(&server.base, &cat, &m.id, &root, &job).expect("Ministral's parts install");
    let got = &cat.models[0];
    install::verify(&install::model_file(&root, got), got.bytes, &got.sha256).expect("joined and checked whole");
    let fetched: Vec<String> = server.seen.lock().unwrap().iter().map(|(n, _)| n.clone()).filter(|n| n.starts_with(&m.file)).collect();
    let want: Vec<String> = m.parts.iter().map(|p| p.file.clone()).collect();
    assert_eq!(fetched, want, "part by part in the catalog's order, never the whole");
    let dir = install::model_dir(&root, &m.id);
    assert!(dir.join(&m.file).is_file() && dir.join("LICENSE.txt").is_file() && dir.join("README.md").is_file(), "{:?}", tree(&root));
    assert!(tree(&install::downloads_dir(&root)).is_empty(), "nothing left in downloads: {:?}", tree(&root));
}

/// REMOVE AND REMOVE ALL LEAVE NOTHING (5.3): removing the last model takes the engine and the `ai`
/// folder with it; installing again and removing all deletes the folder whole; a half-finished download
/// goes with Remove too. Seen red 2026-11-09 with the engine step left out of `remove_model`: "the last
/// model takes the engine and the folder with it" (left: ["ai", "ai/engine", "ai/engine/1.0.0", ...]).
#[test]
fn remove_and_remove_all_leave_nothing() {
    let t = humanity_core::test_temp::dir("ai_remove");
    let root = ai_root(&t);
    let (cat, files) = fixture(120_000, 1);
    let server = serve(files, Serve::Normal, "model.gguf".into());
    install::install_model(&server.base, &cat, "test-model", &root, &install::new_job(0)).expect("installs");
    // A stray half download of the same model, as a stopped Get leaves.
    std::fs::create_dir_all(install::downloads_dir(&root)).unwrap();
    std::fs::write(install::downloads_dir(&root).join("model.gguf.part"), b"half").unwrap();
    let gone = install::remove_model(&cat, &root, "test-model").expect("removes");
    assert!(!gone.is_empty());
    assert_eq!(tree(&t), Vec::<String>::new(), "the last model takes the engine and the folder with it");
    assert!(install::installed(&cat, &root).nothing());

    install::install_model(&server.base, &cat, "test-model", &root, &install::new_job(0)).expect("installs again");
    assert!(root.is_dir());
    install::remove_all(&root).expect("removes all");
    assert!(!root.exists(), "Remove all AI deletes the ai folder");
    assert!(install::installed(&cat, &root).nothing());
}

/// THE ENGINE ZIP IS READ STRICTLY: a path that reaches outside, a compressed entry and a changed byte
/// are each refused and nothing is written; the real package, when `just ai-engine package` has made
/// one on this computer, reads whole with the engine program in it (the Node writer and this reader
/// agree). Seen red 2026-11-09 with `safe_relative` accepting any name: "a path outside is refused".
#[test]
fn the_engine_zip_is_read_strictly() {
    let t = humanity_core::test_temp::dir("ai_zip");
    let good = stored_zip(&[("humanity-ai.exe", b"x"), ("licenses/a.txt", b"y")]);
    assert_eq!(super::zip::unpack(&good, &t.join("ok")).expect("a good zip"), 2);
    assert!(super::zip::unpack(&stored_zip(&[("../escape.dll", b"x")]), &t.join("bad1")).is_err(), "a path outside is refused");
    assert!(super::zip::unpack(&stored_zip(&[("C:\\Windows\\x.dll", b"x")]), &t.join("bad2")).is_err(), "a drive is refused");
    let mut compressed = good.clone();
    compressed[8] = 8; // the first local header's method: deflate
    let dir_at = u32::from_le_bytes(good[good.len() - 6..good.len() - 2].try_into().unwrap()) as usize;
    compressed[dir_at + 10] = 8;
    assert!(super::zip::entries(&compressed).is_err_and(|e| e.contains("compressed")));
    let mut changed = good.clone();
    changed[30 + "humanity-ai.exe".len()] ^= 1;
    assert!(super::zip::entries(&changed).is_err_and(|e| e.contains("CRC")));
    assert!(!t.join("bad1").join("..").join("escape.dll").exists());
    let real = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("target/ai-engine/package/humanity-ai-engine-v1-windows-x64.zip");
    match std::fs::read(&real) {
        Ok(bytes) => {
            let names: Vec<String> = super::zip::entries(&bytes).expect("the real package reads").iter().map(|e| e.path.to_string_lossy().replace('\\', "/")).collect();
            assert!(names.contains(&"humanity-ai.exe".to_string()) && names.iter().any(|n| n.starts_with("ggml-cpu-")), "{names:?}");
        }
        Err(_) => eprintln!("NOTE: no engine package at {} (run `just ai-engine package`); the real zip was not read this time", real.display()),
    }
}

/// The threads a brain gets (9.2): physical cores minus two, never more than four, at least one.
#[test]
fn a_brain_gets_cores_minus_two_up_to_four_threads() {
    assert_eq!(install::brain_threads(Some(6), 12), 4);
    assert_eq!(install::brain_threads(Some(4), 8), 2);
    assert_eq!(install::brain_threads(Some(2), 4), 1);
    assert_eq!(install::brain_threads(Some(16), 32), 4);
    assert_eq!(install::brain_threads(None, 3), 1);
}

/// A server on 127.0.0.1 that streams files from disk by name (a model is a gigabyte: never held in
/// memory), with Range answered 206.
fn serve_files(files: Vec<(String, PathBuf)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { break };
            let files = files.clone();
            std::thread::spawn(move || {
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&head).to_string();
                let name = text.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
                let Some((_, path)) = files.iter().find(|(n, _)| *n == name) else {
                    let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    return;
                };
                let Ok(mut f) = std::fs::File::open(path) else { return };
                let len = f.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n").as_bytes());
                let _ = std::io::copy(&mut f, &mut s);
            });
        }
    });
    format!("http://{addr}/")
}

/// GRANITE THROUGH THE APP, END TO END (ignored: it copies a gigabyte; run by hand with
/// `just test-lib granite_through_the_app -- --ignored --nocapture`, or the dev MCP's dev_lib_test):
/// the real engine package (`just ai-engine package`) and IBM's Granite file served from 127.0.0.1,
/// downloaded, checked and installed by the app's installer into a test folder; the installed engine
/// started by the app's client in its job object at below-normal priority with the cap raised to
/// Granite's need plus the margin; the model checked again before its load and loaded with this
/// computer's brain threads; the Dev page's test question answered, word by word. Prints the times and
/// the memory (embeddable-ai.md, 9.1 to 9.3). SKIPPED, saying so, when the package or the model file is
/// not on this computer.
#[test]
#[ignore = "installs the real engine and Granite (1 GB) into a test folder; run by hand"]
fn granite_through_the_app_end_to_end() {
    use super::client::{AiClient, ClientConfig, Outcome};
    use std::time::{Duration, Instant};
    let repo = Path::new(env!("HUMANITY_REPO_ROOT"));
    let built = AiCatalog::parse(super::catalog::BUILT_IN).expect("the built-in catalog");
    let pkg = repo.join("target/ai-engine/package").join(&built.engine.file);
    let gguf = std::env::var_os("HUMANITY_AI_TEST_GGUF").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"E:\HumanityOS\ai-src\granite-4.0-1b\granite-4.0-1b-Q4_K_M.gguf"));
    if !pkg.is_file() || !gguf.is_file() {
        eprintln!("SKIPPED: needs {} (just ai-engine package) and {}; it did not run, it did not pass", pkg.display(), gguf.display());
        return;
    }
    let t = humanity_core::test_temp::dir("ai_granite");
    let root = ai_root(&t);
    let mut cat = built.clone();
    let zip = std::fs::read(&pkg).expect("the package");
    if sha(&zip) != cat.engine.sha256 {
        eprintln!("NOTE: the engine package on this computer is not the one the catalog names (rebuilt since?); using its own size and hash");
        cat.engine.sha256 = sha(&zip);
        cat.engine.bytes = zip.len() as u64;
    }
    let granite = cat.model("granite-4-0-1b").expect("Granite").clone();
    let recipe: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(repo.join("scripts/ai-catalog-recipe.json")).unwrap()).unwrap();
    let readme = t.join("README.md");
    std::fs::write(&readme, recipe["models"][0]["extras"][1]["text"].as_str().expect("the README's text")).unwrap();
    let files = vec![
        (cat.engine.file.clone(), pkg.clone()),
        (granite.file.clone(), gguf.clone()),
        (granite.extras[0].file.clone(), repo.join("data/ai/licences/apache-2.0.txt")),
        (granite.extras[1].file.clone(), readme),
    ];
    cat.base_url = serve_files(files);
    let started = Instant::now();
    let job = install::new_job(install::download_total(&cat, &granite, &root));
    install::install_model(&cat.base_url, &cat, "granite-4-0-1b", &root, &job).expect("Granite and the engine install");
    let install_ms = started.elapsed().as_millis();
    let started = Instant::now();
    install::verify(&install::model_file(&root, &granite), granite.bytes, &granite.sha256).expect("the check before the load");
    let check_ms = started.elapsed().as_millis();

    let mut c = AiClient::new(Some(super::paths::engine_path_in(&root)), ClientConfig::default());
    let cap = super::models::memory_cap_mb(&granite);
    c.set_memory_cap(cap);
    let hw = super::hardware::read();
    let threads = install::brain_threads(hw.cores, hw.threads);
    let started = Instant::now();
    c.load(&granite.id, &install::model_file(&root, &granite).display().to_string(), &granite.sha256, humanity_ai_protocol::ModelKind::Brain, granite.context, threads);
    let deadline = Instant::now() + Duration::from_secs(120);
    while !c.is_loaded(&granite.id) && Instant::now() < deadline {
        c.poll();
        assert!(c.last_error.is_none(), "the load was refused: {:?}", c.last_error);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(c.is_loaded(&granite.id), "Granite loaded within 120 s; the state is {:?}", c.state());
    let load_ms = started.elapsed().as_millis();
    #[cfg(windows)]
    assert_eq!(c.containment(), Some((true, super::job::BELOW_NORMAL)), "in its job object at below-normal priority");
    let (after_load, _) = c.memory_mb();
    let question = humanity_core::i18n::t("dev.ai.test_question");
    let r = c.generate(&granite.id, "You are a crew member in HumanityOS, run by AI on the player's computer. Answer in one or two sentences.", question, 64);
    let mut pieces = 0;
    let mut last = 0;
    let outcome = loop {
        c.poll();
        if let Some(p) = c.partial(r) {
            if p.len() != last {
                pieces += 1;
                last = p.len();
            }
        }
        if let Some(o) = c.take_outcomes().into_iter().find(|o| o.request() == r) {
            break o;
        }
        assert!(Instant::now() < deadline + Duration::from_secs(60), "no answer");
        std::thread::sleep(Duration::from_millis(5));
    };
    let (now, peak) = c.memory_mb();
    match outcome {
        Outcome::Answered { text, tokens, ms_prefill, ms_generate, .. } => {
            eprintln!(
                "GRANITE THROUGH THE APP: installed in {install_ms} ms (engine {} bytes + model {} bytes over 127.0.0.1), checked in {check_ms} ms, loaded in {load_ms} ms on {threads} threads under a {cap} MB cap\n  committed {:.0} MB after the load, {:.0} MB now, {:.0} MB at its peak\n  asked {question:?}\n  answered {text:?}\n  {tokens} tokens, arriving in {pieces} pieces; prefill {ms_prefill} ms, writing {ms_generate} ms ({:.1} tokens a second)",
                cat.engine.bytes,
                granite.bytes,
                after_load.unwrap_or(0.0),
                now.unwrap_or(0.0),
                peak.unwrap_or(0.0),
                tokens as f64 * 1000.0 / ms_generate.max(1) as f64
            );
            assert!(text.chars().any(char::is_alphabetic) && pieces > 3, "a streamed answer: {text:?} in {pieces} pieces");
        }
        other => panic!("Granite did not answer: {other:?} (last refusal {:?})", c.last_error),
    }
    c.shutdown_now();
    drop(c);

    // THE DEV PAGE'S OWN PATH: Dev > AI's "Send a test question" with the fake switched off, through the
    // app's one client (src/ai/shared.rs) and the same function its button calls: the installed engine
    // found in the (test) ai folder, the model checked on its thread (the first call says "checking"),
    // then loaded and asked; its answer arrives through the shared client.
    super::paths::set_test_root(Some(root.clone()));
    super::shared::set_use_dev_fake(false);
    super::models::forget_checks();
    let started = Instant::now();
    let request = loop {
        let mut sc = super::shared::client();
        if let Some(r) = crate::gui::pages::dev_ai::ask_test_question(&mut sc) {
            break r;
        }
        drop(sc);
        assert!(started.elapsed() < Duration::from_secs(60), "the Dev page's question was never sent (the model check did not finish)");
        std::thread::sleep(Duration::from_millis(50));
    };
    let answer = loop {
        let mut sc = super::shared::client();
        sc.poll();
        if let Some(o) = sc.take_outcomes().into_iter().find(|o| o.request() == request) {
            break o;
        }
        drop(sc);
        assert!(started.elapsed() < Duration::from_secs(180), "the Dev page's question was never answered");
        std::thread::sleep(Duration::from_millis(10));
    };
    super::shared::client().shutdown_now();
    super::paths::set_test_root(None);
    match answer {
        Outcome::Answered { text, .. } => eprintln!("DEV > AI, SEND A TEST QUESTION: answered by the installed Granite in {} ms (the check, the load and the reply): {text:?}", started.elapsed().as_millis()),
        other => panic!("the Dev page's question fell back: {other:?}"),
    }
}
