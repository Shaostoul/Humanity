//! What a file is, said before someone opens one they were sent (2026-10-17, from a suggestion in
//! the project's Discord: "something that says 'hey, this is an executable/script/zip/whatever,
//! make sure you scan anything before opening it'").
//!
//! Not an antivirus. It reads the file's name and, when they are at hand, its first bytes, against
//! one list (`data/safety/file_risks.json`, which the website reads too: web/shared/file-risk.js),
//! and says what kind of thing the file is and what to be careful of: a program, a script, an
//! archive whose contents nobody checked, a document that can carry macros, a shortcut, a web page,
//! a Blender file that can carry scripts. And when the name says one thing (a picture, a video, a
//! song) while the bytes say a program or an archive, it says the file is disguised.
//!
//! Why the receiving app and not only the server: the server already refuses programs and scripts
//! by name, but a private file is encrypted before it is uploaded, so the server never sees its
//! real name or contents, and a device-to-device file never passes the server at all.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize)]
struct Data {
    kinds: Vec<Kind>,
    named_as: Vec<NamedAs>,
    disguised: String,
}

#[derive(Debug, Deserialize)]
struct Kind {
    id: String,
    name: String,
    words: String,
    extensions: Vec<String>,
    #[serde(default)]
    magic: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NamedAs {
    name: String,
    extensions: Vec<String>,
    disguised_if: Vec<String>,
}

/// A word of caution about one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caution {
    /// The kind's id in the list (`program`, `script`, `archive`, ...).
    pub kind: String,
    /// Named like something harmless while its bytes say otherwise.
    pub disguised: bool,
    /// What to show the person, whole.
    pub words: String,
}

/// Where the list lives in the data folder.
const FILE: &str = "safety/file_risks.json";

/// The list, read once: the data folder's copy (so it can be changed without a new build, like the
/// app's other data), else the copy built into the app, which says so in run.log. Neither readable
/// (never, while the tests hold the built-in copy well formed): an empty list, so nothing is said.
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        crate::embedded_data::load_data_or_embedded(&crate::data_dir(), FILE, |bytes| serde_json::from_slice::<Data>(bytes).map_err(|e| e.to_string())).unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy(FILE, format_args!("neither copy could be read ({e}); no file gets a word of caution"));
            Data::default()
        })
    })
}

/// Whether `head` starts with the bytes the hex text `magic` spells.
fn starts_with_hex(head: &[u8], magic: &str) -> bool {
    let want: Option<Vec<u8>> = (0..magic.len() / 2).map(|i| u8::from_str_radix(magic.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
    want.is_some_and(|w| !w.is_empty() && head.starts_with(&w))
}

/// The longest of `endings` that `name` ends with (after a dot), if any.
fn ending<'a>(name: &str, endings: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    endings.filter(|e| name.ends_with(&format!(".{e}"))).max_by_key(|e| e.len())
}

/// The file's own ending as shown in the words: ".exe", or "no ending".
fn shown_ending(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && !ext.contains(['/', '\\', ' ']) => format!(".{ext}"),
        _ => "no ending".to_string(),
    }
}

/// A word of caution about a file named `name` whose first bytes are `head` (None or empty: not
/// known), or None when there is nothing to warn about. The name decides first; when it says
/// nothing risky, the bytes are checked: a name that claims a picture, a video, a song, a document
/// or a 3D model over a program or an archive is a disguise, and a name the list does not know over
/// a program, a script or an archive is warned as that.
pub fn assess(name: &str, head: Option<&[u8]>) -> Option<Caution> {
    let d = data();
    let lower = name.trim().to_lowercase();
    let by_name = d
        .kinds
        .iter()
        .filter_map(|k| ending(&lower, k.extensions.iter()).map(|e| (k, e)))
        .max_by_key(|(_, e)| e.len());
    if let Some((kind, ext)) = by_name {
        return Some(Caution { kind: kind.id.clone(), disguised: false, words: kind.words.replace("{ext}", &format!(".{ext}")) });
    }
    let head = head.filter(|h| !h.is_empty())?;
    let actual = d.kinds.iter().find(|k| k.magic.iter().any(|m| starts_with_hex(head, m)))?;
    let claimed = d.named_as.iter().filter_map(|n| ending(&lower, n.extensions.iter()).map(|e| (n, e))).max_by_key(|(_, e)| e.len());
    match claimed {
        Some((named, ext)) if named.disguised_if.contains(&actual.id) => Some(Caution {
            kind: actual.id.clone(),
            disguised: true,
            words: d.disguised.replace("{claimed}", &named.name).replace("{ext}", &format!(".{ext}")).replace("{actual}", &actual.name),
        }),
        // A harmless kind whose bytes are allowed for it (a text file starting "#!").
        Some(_) => None,
        None => Some(Caution { kind: actual.id.clone(), disguised: false, words: actual.words.replace("{ext}", &shown_ending(&lower)) }),
    }
}

/// [`assess`] for a file on this computer: its first bytes read from `path`.
pub fn assess_file(name: &str, path: &std::path::Path) -> Option<Caution> {
    let mut head = [0u8; 16];
    let n = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    assess(name, Some(&head[..n]))
}

/// [`assess_file`] for a file this app just saved, remembered per path for the rest of the run, so
/// a card drawn every frame reads the file's first bytes once.
pub fn assess_saved(name: &str, path: &std::path::Path) -> Option<Caution> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static SEEN: OnceLock<Mutex<HashMap<std::path::PathBuf, Option<Caution>>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(known) = seen.lock().ok().and_then(|m| m.get(path).cloned()) {
        return known;
    }
    let caution = assess_file(name, path);
    if let Ok(mut m) = seen.lock() {
        m.insert(path.to_path_buf(), caution.clone());
    }
    caution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Cases {
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    struct Case {
        name: String,
        head: String,
        kind: Option<String>,
        disguised: bool,
        words: Option<String>,
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap()).collect()
    }

    /// EVERY SHARED CASE, AS THE WEBSITE ANSWERS IT (scripts/tests/fixtures/file-risk-cases.json,
    /// which scripts/tests/file-risk-web.test.js runs too): programs, scripts, archives, macro
    /// documents, shortcuts, web pages and Blender files by name; a picture, a song, a video or a text
    /// file whose bytes are a program or an archive called out as disguised; harmless files left
    /// alone (a text file starting "#!" included).
    ///
    /// Seen red 2026-10-17 with the disguise check left out: "holiday.png" over a program's bytes got
    /// no word at all.
    #[test]
    fn every_shared_case_gets_the_same_word_as_on_the_website() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/file-risk-cases.json");
        let cases: Cases = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(cases.cases.len() >= 15);
        for c in cases.cases {
            let head = hex(&c.head);
            let got = assess(&c.name, Some(&head));
            assert_eq!(got.as_ref().map(|g| g.kind.as_str()), c.kind.as_deref(), "{:?} over {}", c.name, c.head);
            assert_eq!(got.as_ref().is_some_and(|g| g.disguised), c.disguised, "{:?} over {}: disguised", c.name, c.head);
            if let Some(words) = c.words {
                assert_eq!(got.map(|g| g.words), Some(words), "{:?}: the words", c.name);
            }
        }
    }

    /// The list is well formed and every kind names its words; nothing in it is malformed hex.
    #[test]
    fn the_list_reads_and_every_signature_is_hex() {
        let d = data();
        assert!(d.kinds.len() >= 7 && !d.named_as.is_empty() && d.disguised.contains("{claimed}"));
        for k in &d.kinds {
            assert!(!k.words.is_empty() && !k.extensions.is_empty(), "{}", k.id);
            for m in &k.magic {
                assert!(m.len() % 2 == 0 && m.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{}: {m}", k.id);
            }
        }
    }

    /// A file on this computer is read for its first bytes.
    #[test]
    fn a_file_on_disk_is_read_for_its_first_bytes() {
        let dir = crate::test_temp::dir("file_risk_disk");
        let p = dir.join("cat.png");
        std::fs::write(&p, [0x4d, 0x5a, 0x90, 0x00, 0x03]).unwrap();
        assert!(assess_file("cat.png", &p).is_some_and(|c| c.disguised && c.kind == "program"));
        assert_eq!(assess_file("cat.png", &dir.join("missing.png")), None, "a file that is not there: no word");
    }
}
