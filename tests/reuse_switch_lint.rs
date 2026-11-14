//! REUSE SWITCH lint (2026-11-13; CLAUDE.md, "No caches kept across runs", refined 2026-10-30).
//!
//! The rule: data kept in memory across frames is allowed only when it is rebuilt on a schedule or
//! tagged with a version its source bumps, "and EVERY such reuse obeys the one 'turn off all reuse'
//! switch on the Dev page ... A new reuse that does not check the switch is a bug." The switch is
//! `humanity_core::reuse::enabled()`. The first reuse wave (2026-11-13) found the sites by reading;
//! this lint is what stops the next one being added without anyone asking the question.
//!
//! WHAT IT LOOKS FOR, reuse-SHAPED state, by name (a judgement, so it is written down here):
//!
//! * a struct FIELD or a `static` whose name holds `cache`, `memo` or `dirty`: a cache keeps what
//!   it worked out, a memo the same, and a dirty flag is the gate of something kept until the flag
//!   says it is stale;
//! * a field or static named `last_*` whose type holds BUILT DATA (a `Vec`, a map or set, a `Box`
//!   or `Arc`, an `Option` of one, or a named type of the program's own): the shape of "what I
//!   built last time, kept to compare or to draw again". A `last_*` holding a number, a flag, a
//!   string or an `Instant` is bookkeeping (when, how many, what was said) and is not counted.
//!
//! Struct fields only inside a `struct ... {` body, so a function's parameters and a `let` with
//! the same names are not counted; test modules (a `mod` under `#[cfg(test)]`, `*_tests.rs`,
//! `tests.rs`) are skipped.
//!
//! WHAT PASSES. A site passes when it DECLARES ITSELF where it lives, with a comment on its own
//! line or in the comment and attribute block right above it, one of two ways:
//!
//! 1. `reuse: site: <what the switch does here>`: a reuse that obeys the switch, and how (built
//!    again every frame with the switch off, or on a short schedule, and where that is decided);
//! 2. any other `reuse:` (or `Reuse:`) comment: why it is not a reuse (a dirty flag that only asks
//!    for a rebuild of something rebuilt in full, a counter, data loaded once per run).
//!
//! THE DECLARATIONS ARE THE LIST (2026-11-13, the reuse switch, part B). Until then a site could
//! also pass by being named in one paragraph of `crates/humanity-core/src/reuse.rs`, and every
//! agent that added a reuse appended to that paragraph: it conflicted in two merges in one day. So
//! each site now says so where it is declared (two agents adding two sites touch two places), the
//! list of every site is `grep -rn "reuse: site:" src crates`, and reuse.rs keeps only the rule and
//! a pointer: `reuse_rs_holds_no_site_list` fails a site list written back into it.
//!
//! THE BACKLOG. When this lint was built, the sites in [`BACKLOG`] were reuse-shaped and not yet
//! answered: the first two waves answered the engine's and the renderer's known sites, and these
//! are the rest, found by this scan, each to be given the switch or a reason. The list only
//! shrinks: an entry that no longer matches a site (answered, renamed or removed) fails the lint
//! until it is taken off, and a site that is not answered and not on it fails as new.
//!
//! Seen red 2026-11-13 three ways: a deliberately unlisted `kept_cache: Vec<u8>` added to
//! `engine::home_reuse::StockKey` ("1 reuse-shaped site(s) neither in ... reuse.rs's list nor with
//! a `reuse:` reason comment ... src/engine/home_reuse.rs:49 `kept_cache`"), a made-up backlog entry
//! ("1 backlog entr(ies) no longer match an unanswered site"), and the real tree before the
//! backlog was written down (89 sites at first, 73 once `memory_mb` stopped counting as a memo and
//! an `Option<Instant>` as built data, 58 after this wave's reasons and list: the backlog).
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/reuse_switch_lint.rs

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("src"), &mut out);
    if let Ok(rd) = fs::read_dir(repo().join("crates")) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                walk(&e.path().join("src"), &mut out);
            }
        }
    }
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Is a `last_*` field's type built data (see the module doc)?
fn holds_built_data(ty: &str) -> bool {
    let t = ty.trim().trim_end_matches(',').trim();
    let scalar = [
        "bool", "f32", "f64", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "char",
        "String", "&'static str", "Instant", "Duration", "SystemTime",
    ];
    // Peel the wrappers a kept value sits in (an Option, a lock, a cell) down to what it holds.
    let mut inner = t;
    loop {
        let s = inner.trim().trim_start_matches("std::sync::").trim_start_matches("std::cell::").trim_start_matches("std::time::");
        let peeled = ["Option<", "Mutex<", "RwLock<", "OnceLock<", "Cell<", "RefCell<"]
            .iter()
            .find_map(|w| s.strip_prefix(w).and_then(|r| r.strip_suffix('>')));
        match peeled {
            Some(p) => inner = p,
            None => {
                inner = s;
                break;
            }
        }
    }
    let inner = inner.trim().trim_start_matches("atomic::");
    let atomics = ["AtomicBool", "AtomicU8", "AtomicU32", "AtomicU64", "AtomicUsize", "AtomicI32", "AtomicI64"];
    if scalar.contains(&inner) || atomics.contains(&inner) {
        return false;
    }
    // Arrays and tuples of numbers (a position, a colour, a size) are values, not built data.
    if (inner.starts_with('[') || inner.starts_with('(')) && !inner.contains('<') {
        let words: Vec<&str> = inner.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
        if words.iter().all(|w| scalar.contains(w) || w.chars().all(|c| c.is_ascii_digit())) {
            return false;
        }
    }
    // Plain glam values (a position, a direction, a rotation) are values too.
    let glam_value = ["Vec2", "Vec3", "Vec4", "DVec3", "DVec2", "Quat", "DQuat", "Mat4", "DMat4", "glam::Vec3", "glam::DVec3", "glam::Vec2", "glam::Quat"];
    !glam_value.contains(&inner)
}

/// One reuse-shaped declaration: file, line (1-based), name, and whether it is answered by a
/// reason comment.
#[derive(Debug, Clone)]
struct Site {
    file: String,
    line: usize,
    name: String,
    reasoned: bool,
}

/// Is `name` reuse-shaped by itself? One of its `_`-separated words is a cache, a memo or dirty
/// (`decoration_mesh_cache`, `sel_dirty`, `CACHE`, `memo`), so `memory_mb` is not a memo.
fn shaped_name(name: &str) -> bool {
    name.to_ascii_lowercase().split('_').any(|w| {
        matches!(w, "cache" | "cached" | "caches" | "memo" | "memos" | "memoized" | "dirty")
    })
}

/// Does this line's COMMENT carry a reason (`reuse:` as a word, not a path's `home_reuse::`)?
fn has_reason(line: &str) -> bool {
    let Some(at) = line.find("//") else { return false };
    let c = &line[at..];
    ["reuse:", "Reuse:"].iter().any(|w| {
        c.match_indices(w).any(|(k, _)| {
            let before = c[..k].chars().next_back().map_or(true, |ch| !(ch.is_alphanumeric() || ch == '_'));
            let after = c[k + w.len()..].chars().next() != Some(':');
            before && after
        })
    })
}

/// Scan one source file's text for reuse-shaped declarations.
fn scan(file: &str, src: &str) -> Vec<Site> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    // Depth of brace nesting inside the struct bodies we are in (0 = not in one).
    let mut struct_depth: i32 = 0;
    let mut pending_struct = false;
    let mut cfg_test = false;
    // Brace depth inside a test module being skipped (0 = not in one).
    let mut test_mod_depth: i32 = 0;
    for (i, raw) in lines.iter().enumerate() {
        let code = raw.split("//").next().unwrap_or("");
        let t = code.trim();
        // A test module is skipped to its closing brace. (Until 2026-11-13 the scan stopped at the
        // first one, on the belief that it ran to the end of the file: settings.rs has one at line
        // 513 of 5,014, so its last 4,500 lines, the Media line's `CACHE` among them, went unseen.)
        if test_mod_depth > 0 {
            test_mod_depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
            continue;
        }
        if t.starts_with("#[cfg(test)]") {
            cfg_test = true;
            continue;
        }
        if cfg_test && (t.starts_with("mod ") || t.starts_with("pub mod ") || t.starts_with("pub(crate) mod ")) {
            if t.ends_with('{') {
                test_mod_depth = code.matches('{').count() as i32 - code.matches('}').count() as i32;
            }
            cfg_test = false;
            continue;
        }
        if cfg_test && !t.starts_with("#[") && !t.is_empty() {
            cfg_test = false;
        }
        let opens = code.matches('{').count() as i32;
        let closes = code.matches('}').count() as i32;
        if struct_depth == 0 {
            // `struct Name {` (a tuple or unit struct ends with `;` and has no field lines).
            let is_struct = t.split_whitespace().take(3).any(|w| w == "struct") && !t.ends_with(';');
            if is_struct || (pending_struct && t.starts_with('{')) || (pending_struct && t.starts_with("where")) {
                if opens > 0 {
                    struct_depth = opens - closes;
                    pending_struct = false;
                } else {
                    pending_struct = true;
                }
                continue;
            }
        } else {
            // A field line, at the struct's own depth.
            if struct_depth == 1 {
                if let Some(site) = field_site(file, i, t, &lines) {
                    out.push(site);
                }
            }
            struct_depth += opens - closes;
            if struct_depth < 0 {
                struct_depth = 0;
            }
            continue;
        }
        // Statics, anywhere outside a struct body.
        let s = t.trim_start_matches("pub(crate) ").trim_start_matches("pub ");
        if let Some(rest) = s.strip_prefix("static ") {
            let rest = rest.trim_start_matches("mut ");
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let ty = rest[name.len()..].trim_start_matches(':').split('=').next().unwrap_or("");
            let lname = name.to_ascii_lowercase();
            if shaped_name(&lname) || (lname.starts_with("last_") && holds_built_data(ty)) {
                out.push(Site { file: file.to_string(), line: i + 1, name, reasoned: reasoned_at(&lines, i) });
            }
        }
    }
    out
}

/// A struct field declaration line (`pub(crate) name: Type,`), if it is reuse-shaped.
fn field_site(file: &str, i: usize, t: &str, lines: &[&str]) -> Option<Site> {
    let s = t.trim_start_matches("pub(crate) ").trim_start_matches("pub(super) ").trim_start_matches("pub ");
    let name: String = s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if name.is_empty() {
        return None;
    }
    let rest = s[name.len()..].trim_start();
    let ty = rest.strip_prefix(':')?;
    if rest.starts_with("::") {
        return None;
    }
    let lname = name.to_ascii_lowercase();
    let shaped = shaped_name(&lname) || (lname.starts_with("last_") && holds_built_data(ty));
    shaped.then(|| Site { file: file.to_string(), line: i + 1, name, reasoned: reasoned_at(lines, i) })
}

/// A reason on the declaration's own line, or in the comment and attribute block right above it.
fn reasoned_at(lines: &[&str], i: usize) -> bool {
    if has_reason(lines[i]) {
        return true;
    }
    let mut k = i;
    while k > 0 {
        k -= 1;
        let t = lines[k].trim();
        if t.starts_with("//") || t.starts_with("#[") {
            if has_reason(t) {
                return true;
            }
        } else {
            break;
        }
    }
    false
}

/// Is this line's comment a SITE declaration (`reuse: site: ...`), and what does it say?
fn site_declaration(line: &str) -> Option<&str> {
    let at = line.find("//")?;
    let c = &line[at..];
    ["reuse: site:", "Reuse: site:"].iter().find_map(|w| c.find(w).map(|k| c[k + w.len()..].trim()))
}

/// Every `reuse: site:` declaration in src/ and crates/: (file, line, what it says).
fn declarations() -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    for p in rust_sources() {
        let Ok(src) = fs::read_to_string(&p) else { continue };
        for (i, l) in src.lines().enumerate() {
            if let Some(said) = site_declaration(l) {
                out.push((rel(&p), i + 1, said.to_string()));
            }
        }
    }
    out
}

/// The sites that were reuse-shaped and unanswered when this lint was built (2026-11-13), as
/// "file name" pairs, each to be given the switch or a reason: 58 declarations in 56 pairs, after
/// the second wave's part A answered the engine's own. Only shrinks. Some are surely not reuses
/// (`last_status`, a dirty flag that asks for a save): a `reuse:` comment saying so takes them off.
/// Part B (2026-11-13) answered 53 of the 54: the one left is local_shadows.rs's log throttle (a
/// clock and the last stats, surely a reason), left while another agent works in that file.
/// How many `reuse: site:` declarations there were when the central list was retired (part B,
/// 2026-11-13); raise it as sites are declared, never lower it to hide a deletion.
const DECLARED_FLOOR: usize = 59;

const BACKLOG: &[&str] = &[
    "src/renderer/local_shadows.rs last_log",
];

fn unanswered() -> (Vec<Site>, usize, usize) {
    let mut all = Vec::new();
    let files = rust_sources();
    let crates_seen = files.iter().filter(|p| rel(p).starts_with("crates/")).count();
    for p in &files {
        let f = rel(p);
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "tests.rs" || name.ends_with("_tests.rs") {
            continue;
        }
        let Ok(src) = fs::read_to_string(p) else { continue };
        all.extend(scan(&f, &src));
    }
    let total = all.len();
    let open: Vec<Site> = all.into_iter().filter(|s| !s.reasoned).collect();
    (open, total, crates_seen)
}

/// The scanner finds what it should and nothing it should not (a positive and a negative
/// control): an unlisted cache field fails, a reasoned one passes, a function parameter and a
/// test module are not counted, and reuse.rs's list format is read.
#[test]
fn the_scanner_sees_a_reuse_shaped_field() {
    let src = "\
struct Kept {
    pub(crate) last_cache: Vec<u8>,
    /// reuse: rebuilt every frame with the switch off (positive control).
    pub(crate) mesh_cache: HashMap<String, usize>,
    last_seen: Option<Instant>,
    last_pose: glam::DVec3,
    last_selection: Option<Selection>,
    sel_dirty: bool, // reuse: asks for a rebuild only
    pile_cache: (home_reuse::StockKey, Vec<u8>),
    memory_mb: u32,
}
fn f(decoration_mesh_cache: &mut HashMap<String, usize>) {}
static NAME_CACHE: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(test)]
mod tests {
    struct T { kept_cache: Vec<u8> }
}
struct After {
    after_cache: Vec<u8>,
}
";
    let sites = scan("src/x.rs", src);
    let names: Vec<(&str, bool)> = sites.iter().map(|s| (s.name.as_str(), s.reasoned)).collect();
    assert_eq!(
        names,
        vec![
            ("last_cache", false),
            ("mesh_cache", true),
            ("last_selection", false),
            ("sel_dirty", true),
            ("pile_cache", false),
            ("NAME_CACHE", false),
            // After a test module in the middle of a file the scan goes on (seen red 2026-11-13 with
            // the old stop at the first test module: the picks ended at NAME_CACHE).
            ("after_cache", false)
        ],
        "the scanner's picks"
    );
    // A site declaration answers the site it sits on, and says what the switch does.
    let declared = scan("src/x.rs", "struct K {\n    /// reuse: site: built again every frame with the switch off.\n    tile_cache: Vec<u8>,\n}\n");
    assert!(declared.len() == 1 && declared[0].reasoned, "a `reuse: site:` declaration answers its site");
    assert_eq!(site_declaration("    /// reuse: site: built again every frame"), Some("built again every frame"));
    assert_eq!(site_declaration("    // reuse: a counter, not a reuse"), None, "a reason is not a site");
    assert_eq!(site_declaration("    let reuse: bool = true; // no comment here"), None);
}

/// Every reuse-shaped site in src/ and crates/ is answered: in reuse.rs's list, with a reason
/// comment, or (from before this lint) in the backlog. And the backlog holds only sites still
/// unanswered, so it shrinks as they are answered.
#[test]
fn every_reuse_shaped_site_asks_the_switch_or_says_why_not() {
    let (open, total, crates_seen) = unanswered();
    assert!(total > 50, "the walk found only {total} reuse-shaped sites: it stopped seeing a folder");
    assert!(crates_seen > 50, "the walk found only {crates_seen} files under crates/");
    let backlog: BTreeSet<&str> = BACKLOG.iter().copied().collect();
    let open_keys: BTreeSet<String> = open.iter().map(|s| format!("{} {}", s.file, s.name)).collect();
    let new: Vec<String> = open
        .iter()
        .filter(|s| !backlog.contains(format!("{} {}", s.file, s.name).as_str()))
        .map(|s| format!("  {}:{} `{}`", s.file, s.line, s.name))
        .collect();
    let stale: Vec<&&str> = backlog.iter().filter(|b| !open_keys.contains(**b)).collect();
    assert!(
        new.is_empty(),
        "{} reuse-shaped site(s) declaring neither `reuse: site:` nor a `reuse:` reason (CLAUDE.md: \"A new reuse \
         that does not check the switch is a bug\"). Make it obey humanity_core::reuse::enabled() and declare it where it \
         lives with a `// reuse: site: <what the switch does here>` comment, or say in a `// reuse: <reason>` comment \
         why it is not a reuse:\n{}",
        new.len(),
        new.join("\n")
    );
    assert!(
        stale.is_empty(),
        "{} backlog entr(ies) no longer match an unanswered site (answered, renamed or removed): take them off BACKLOG \
         in tests/reuse_switch_lint.rs, the list only shrinks:\n{:?}",
        stale.len(),
        stale
    );
}

/// Every `reuse: site:` declaration says what the switch does (a few words at least), and there are
/// as many as the reuse waves declared, so a sweep that deletes them is noticed (2026-11-13, part B:
/// 45 when the central list was retired, 59 once part B answered the backlog). Seen red with the floor one above the count: "only 45
/// `reuse: site:` declarations (at least 46 expected): were some deleted?".
#[test]
fn every_declared_site_says_what_the_switch_does() {
    let decl = declarations();
    let thin: Vec<String> = decl
        .iter()
        .filter(|(_, _, said)| said.split_whitespace().count() < 4)
        .map(|(f, l, said)| format!("  {f}:{l} \"{said}\""))
        .collect();
    assert!(thin.is_empty(), "a `reuse: site:` declaration must say what the switch does there:\n{}", thin.join("\n"));
    assert!(
        decl.len() >= DECLARED_FLOOR,
        "only {} `reuse: site:` declarations (at least {DECLARED_FLOOR} expected): were some deleted?",
        decl.len()
    );
}

/// The sites are declared where they live, never listed in reuse.rs again (the paragraph every
/// agent appended to conflicted twice in one day). Seen red 2026-11-13 against reuse.rs as it was
/// before this change: "crates/humanity-core/src/reuse.rs lists reuse sites again ... reuse.rs:51
/// //! - `src/engine/frame_water.rs`: the sea's two shells ..." and 13 more items.
#[test]
fn reuse_rs_holds_no_site_list() {
    let reuse_rs = fs::read_to_string(repo().join("crates/humanity-core/src/reuse.rs")).expect("reuse.rs");
    let items: Vec<String> = reuse_rs
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            t.starts_with("//! - `") && t["//! - `".len()..].split('`').next().is_some_and(|f| f.ends_with(".rs"))
        })
        .map(|(i, l)| format!("  reuse.rs:{} {}", i + 1, l.trim()))
        .collect();
    assert!(
        items.is_empty(),
        "crates/humanity-core/src/reuse.rs lists reuse sites again; declare each where it lives instead, with a \
         `// reuse: site: <what the switch does>` comment on the field or static:\n{}",
        items.join("\n")
    );
}
