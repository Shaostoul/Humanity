//! What a file is, said before someone opens one they were sent (2026-10-17, from a suggestion in
//! the project's Discord: "something that says 'hey, this is an executable/script/zip/whatever,
//! make sure you scan anything before opening it'").
//!
//! Every file a person sees also gets its TYPE shown (`file_type`, drawn by
//! gui/widgets/file_badge.rs): a picture, a word and a colour, so that someone who reads little or
//! nothing still knows what they are looking at (the operator, 2026-10-17: "people inexperienced
//! with technology and possibly illiterate people"). Green to look at, amber to take care with, red
//! for what can run things, grey when the type is not known.
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
    unknown: Look,
    icons: std::collections::HashMap<String, Vec<Shape>>,
}

#[derive(Debug, Deserialize)]
struct Kind {
    id: String,
    name: String,
    label: String,
    icon: String,
    level: Level,
    words: String,
    extensions: Vec<String>,
    #[serde(default)]
    magic: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NamedAs {
    id: String,
    name: String,
    label: String,
    icon: String,
    level: Level,
    extensions: Vec<String>,
    disguised_if: Vec<String>,
}

/// How a type looks with no more to it: the `unknown` entry.
#[derive(Debug, Default, Deserialize)]
struct Look {
    id: String,
    label: String,
    icon: String,
    level: Level,
}

/// How much care a type of file needs, shown as its badge's colour.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Fine to look at: a picture, a video, a sound, a document, a 3D model (green).
    View,
    /// Take care: an archive, a document that can carry macros, a web page (amber).
    Care,
    /// Can run things on the computer, or pretends to be something it is not (red).
    Danger,
    /// Not a type the list knows (grey).
    #[default]
    Unknown,
}

/// One shape of a type's drawing, on a 48 by 48 grid (y down): exactly one of `rect` [x, y, w, h],
/// `line` [x1, y1, x2, y2], `poly` [x, y, ...] (open unless `closed`) or `circle` [cx, cy, r];
/// `fill` fills a rect, a closed poly or a circle.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Shape {
    #[serde(default)]
    pub rect: Option<Vec<f32>>,
    #[serde(default)]
    pub line: Option<Vec<f32>>,
    #[serde(default)]
    pub poly: Option<Vec<f32>>,
    #[serde(default)]
    pub circle: Option<Vec<f32>>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub fill: bool,
}

/// What type of file a file is, for its badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileType {
    /// The type's id in the list (`video`, `program`, `unknown`, ...).
    pub id: String,
    /// The word on the badge ("Video").
    pub label: String,
    /// The drawing's name in the list's `icons`.
    pub icon: String,
    pub level: Level,
    /// The file's own ending as shown (".mp4"), or empty when it has none.
    pub ending: String,
    /// Named like a harmless type while its bytes are a program, a script or an archive: the badge
    /// shows what it really is, in red.
    pub disguised: bool,
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
fn ending_of<'a>(name: &str, endings: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    endings.filter(|e| name.ends_with(&format!(".{e}"))).max_by_key(|e| e.len())
}

/// The file's own ending as shown in the words: ".exe", or "no ending".
fn shown_ending(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && !ext.contains(['/', '\\', ' ']) => format!(".{ext}"),
        _ => "no ending".to_string(),
    }
}

/// The type of a file named `name` whose first bytes are `head` (None or empty: not known), for its
/// badge: by its name (the longest matching ending; a kind that needs care before a harmless one),
/// and, when the bytes are known, what they say: a harmless name over a program's, a script's or an
/// archive's bytes is that, disguised and red; a name the list does not know over such bytes is
/// that. Anything else is the list's `unknown` ("File", grey). Always an answer.
/// A file's name as it is shown and judged (2026-10-17): its direction marks taken out (U+200E,
/// U+200F, U+061C, U+202A to U+202E, U+2066 to U+2069; a right-to-left override makes a browser
/// show `photo<RLO>gnp.exe` as "photoexe.png", and a mark after ".exe" hid the ending from the
/// check), every other control character made a space, and trimmed. The website's twin is
/// web/shared/file-risk.js `plainName`; both are held to the shared cases' `plain_names`.
pub fn plain_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(*c, '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn file_type(name: &str, head: Option<&[u8]>) -> FileType {
    let d = data();
    let lower = plain_name(name).to_lowercase();
    let ending = match shown_ending(&lower) {
        e if e.starts_with('.') => e,
        _ => String::new(),
    };
    let of_kind = |k: &Kind, disguised: bool| FileType {
        id: k.id.clone(),
        label: k.label.clone(),
        icon: k.icon.clone(),
        level: if disguised { Level::Danger } else { k.level },
        ending: ending.clone(),
        disguised,
    };
    if let Some((kind, _)) = d.kinds.iter().filter_map(|k| ending_of(&lower, k.extensions.iter()).map(|e| (k, e))).max_by_key(|(_, e)| e.len()) {
        return of_kind(kind, false);
    }
    let head = head.filter(|h| !h.is_empty());
    let actual = head.and_then(|h| d.kinds.iter().find(|k| k.magic.iter().any(|m| starts_with_hex(h, m))));
    let claimed = d.named_as.iter().filter_map(|n| ending_of(&lower, n.extensions.iter()).map(|e| (n, e))).max_by_key(|(_, e)| e.len()).map(|(n, _)| n);
    match (claimed, actual) {
        (Some(named), Some(kind)) if named.disguised_if.contains(&kind.id) => of_kind(kind, true),
        (Some(named), _) => FileType { id: named.id.clone(), label: named.label.clone(), icon: named.icon.clone(), level: named.level, ending, disguised: false },
        (None, Some(kind)) => of_kind(kind, false),
        (None, None) => FileType { id: d.unknown.id.clone(), label: d.unknown.label.clone(), icon: d.unknown.icon.clone(), level: d.unknown.level, ending, disguised: false },
    }
}

/// The drawing named `icon` (the list's `icons`), or the unknown type's when there is none.
pub fn icon(icon: &str) -> &'static [Shape] {
    let d = data();
    d.icons.get(icon).or_else(|| d.icons.get(&d.unknown.icon)).map(|v| v.as_slice()).unwrap_or(&[])
}

/// A word of caution about a file named `name` whose first bytes are `head` (None or empty: not
/// known), or None when there is nothing to warn about. The name decides first; when it says
/// nothing risky, the bytes are checked: a name that claims a picture, a video, a song, a document
/// or a 3D model over a program or an archive is a disguise, and a name the list does not know over
/// a program, a script or an archive is warned as that.
pub fn assess(name: &str, head: Option<&[u8]>) -> Option<Caution> {
    let d = data();
    let lower = plain_name(name).to_lowercase();
    let by_name = d
        .kinds
        .iter()
        .filter_map(|k| ending_of(&lower, k.extensions.iter()).map(|e| (k, e)))
        .max_by_key(|(_, e)| e.len());
    if let Some((kind, ext)) = by_name {
        return Some(Caution { kind: kind.id.clone(), disguised: false, words: kind.words.replace("{ext}", &format!(".{ext}")) });
    }
    let head = head.filter(|h| !h.is_empty())?;
    let actual = d.kinds.iter().find(|k| k.magic.iter().any(|m| starts_with_hex(head, m)))?;
    let claimed = d.named_as.iter().filter_map(|n| ending_of(&lower, n.extensions.iter()).map(|e| (n, e))).max_by_key(|(_, e)| e.len());
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

/// The first bytes of a file this app just saved, read once per path for the rest of the run, so a
/// card drawn every frame does not read the disk every frame. Empty when it cannot be read.
fn saved_head(path: &std::path::Path) -> Vec<u8> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static SEEN: OnceLock<Mutex<HashMap<std::path::PathBuf, Vec<u8>>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(known) = seen.lock().ok().and_then(|m| m.get(path).cloned()) {
        return known;
    }
    let mut head = [0u8; 16];
    let n = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut head)).unwrap_or(0);
    let head = head[..n].to_vec();
    if let Ok(mut m) = seen.lock() {
        m.insert(path.to_path_buf(), head.clone());
    }
    head
}

/// [`assess`] for a file this app just saved: its first bytes read once (`saved_head`).
pub fn assess_saved(name: &str, path: &std::path::Path) -> Option<Caution> {
    assess(name, Some(&saved_head(path)))
}

/// [`file_type`] for a file this app just saved: its first bytes read once (`saved_head`).
pub fn file_type_saved(name: &str, path: &std::path::Path) -> FileType {
    file_type(name, Some(&saved_head(path)))
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
        #[serde(rename = "type")]
        file_type: String,
        level: Level,
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
    /// no word at all. Its badge half (every file's type and level), seen red the same day with
    /// `file_type`'s disguise check left out: "\"holiday.png\" over 4d5a90000300: its badge".
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
            let t = file_type(&c.name, Some(&head));
            assert_eq!((t.id.as_str(), t.level), (c.file_type.as_str(), c.level), "{:?} over {}: its badge", c.name, c.head);
            assert_eq!(t.disguised, c.disguised, "{:?} over {}: the badge says disguised", c.name, c.head);
        }
    }

    /// A NAME CANNOT HIDE ITS ENDING (2026-10-17): every shared `plain_names` pair, the website's too;
    /// and the shared cases above hold a program whose ".exe" is followed by a direction mark.
    /// Seen red 2026-10-17 with `assess` judging the name as it came: the shared case for "evil.exe"
    /// followed by U+200F, in the test above, got no word (left: None, right: Some("program")).
    #[test]
    fn a_name_cannot_hide_its_ending_behind_direction_marks() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/file-risk-cases.json");
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let pairs = v["plain_names"].as_array().expect("the shared plain_names");
        assert!(pairs.len() >= 5);
        for p in pairs {
            assert_eq!(plain_name(p[0].as_str().unwrap()), p[1].as_str().unwrap(), "{:?}", p[0]);
        }
    }

    /// EVERY TYPE HAS A WORD AND A DRAWING THAT CAN BE DRAWN (the badge, 2026-10-17): every kind,
    /// every name a file can claim and the unknown type name a label and an icon the list draws, and
    /// every shape of every drawing is one shape with the right count of numbers, on the 48 by 48
    /// grid. Seen red 2026-10-17 with the archive's icon renamed "crate" in the list: "archive: its
    /// icon is drawn".
    #[test]
    fn every_type_has_a_word_and_a_drawing_that_can_be_drawn() {
        let d = data();
        let looks = d.kinds.iter().map(|k| (&k.id, &k.label, &k.icon)).chain(d.named_as.iter().map(|n| (&n.id, &n.label, &n.icon))).chain([(&d.unknown.id, &d.unknown.label, &d.unknown.icon)]);
        for (id, label, icon) in looks {
            assert!(!label.is_empty() && label.chars().count() <= 14, "{id}: a short word for the badge: {label:?}");
            assert!(d.icons.contains_key(icon), "{id}: its icon is drawn ({icon:?})");
        }
        for (name, shapes) in &d.icons {
            assert!(!shapes.is_empty(), "{name}: a drawing with something in it");
            for s in shapes {
                let given: Vec<(&str, &Vec<f32>)> = [("rect", &s.rect), ("line", &s.line), ("poly", &s.poly), ("circle", &s.circle)].into_iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v))).collect();
                assert_eq!(given.len(), 1, "{name}: each shape is exactly one of rect, line, poly, circle: {s:?}");
                let (k, v) = given[0];
                let right = match k {
                    "rect" | "line" => v.len() == 4,
                    "circle" => v.len() == 3,
                    _ => v.len() >= 4 && v.len() % 2 == 0,
                };
                assert!(right, "{name}: a {k} with the right count of numbers: {v:?}");
                assert!(v.iter().all(|n| (0.0..=48.0).contains(n)), "{name}: on the 48 by 48 grid: {v:?}");
            }
        }
        assert_eq!(icon("no such drawing"), icon(&d.unknown.icon), "a missing drawing falls back to the unknown type's");
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
