//! TEXT CUT lint (BUG-340, 2026-11-21): no text is cut by BYTES where a character may straddle the cut.
//!
//! Rust panics when a `&str` is sliced at a byte that is not a character boundary. A cut like
//! `&card.key[..12.min(card.key.len())]` looks careful (it minds the length) but counts bytes: when
//! byte 12 falls inside an accented letter, a CJK character or an emoji, the thread panics. The key
//! cards handler did exactly that on a card that FAILED its check, so on whatever text a server
//! sent, on the main frame thread with no catch: any connected server could crash the desktop app
//! with one frame, again on every reconnect. The same cut sat on the sender of a sealed message,
//! a contact request's sender, group authors, file ids and more. A `&s[..8]` on text shorter than 8
//! bytes panics the same way.
//!
//! Every such cut now goes through `humanity_core::text` (`prefix`, `suffix`, `prefix_bytes`,
//! `suffix_bytes`, `clip`, `ends`), which cuts by characters and never panics. This lint fails on a
//! new byte-range slice of text in src/ or crates/*/src/ whose bound is a literal (`[..12]`,
//! `[3..]`), a clamp (`[..12.min(s.len())]`) or arithmetic on a length (`[s.len() - 4..]`,
//! `[s.len().saturating_sub(4)..]`). A cut at a position the text itself gave (`find`, `char_indices`,
//! a matched prefix's `len()`) is on a boundary already and is left alone.
//!
//! WHAT COUNTS AS TEXT. A std-only scan has no types, so it reads the evidence in the source: the
//! slice is an argument of a formatting macro (`format!`, `log::warn!`, `println!`, ...), or is
//! followed by a method only text has (`.chars()`, `.parse`, `.trim()`, `.to_lowercase()`, ...), or
//! is compared with a string literal, or is handed to `from_str_radix`, `RichText::new`, `.label`,
//! `.starts_with`, ...; or its receiver is a text-making call (`to_hex()`, `to_string()`, `as_str()`,
//! `hex::encode(..)`) or a name declared as text: the nearest declaration above it in the file
//! (`x: &str`, `let x = y.to_string()`, ...) or, for a field `a.b`, a field `b` declared only with a
//! text type anywhere in the scanned code. A cut followed by a method of a list (`.join(`, `.iter(`)
//! is a list's. Measured against `clippy::string_slice` (which has the types) on 2026-11-21's tree,
//! before the fix: the scan found 171 of the 185 count-shaped string cuts clippy found in src/ and
//! crates/ (the misses were names declared out of its sight: a tuple pattern, a closure's or a
//! `for` loop's name, a copy declared 1,400 lines up), and flagged 2 slices that were not text (a
//! list of names, a byte buffer), now marked. Clippy is not a gate here (the tree is far from
//! clippy-clean); to measure again: `cargo clippy --workspace --all-targets -- -A clippy::all -W
//! clippy::string_slice`, and `TEXT_CUT_DUMP=<file>` makes this lint write every cut it read.
//!
//! A REVIEWED BYTE CUT that is truly meant (fixed-format ASCII, such as the hex of a hash or an
//! RFC 3339 date the code itself wrote) carries `text-cut-ok: <why it is ASCII>` in a comment on its
//! line or the line above, or is written `s.get(a..b)`, which never panics. So does a slice of bytes
//! or of a list that the scan mistook for text. A cut after a prefix the code checked is written
//! `&s["dm:".len()..]`: a position the text gave, not a count.
//!
//! Seen red on purpose 2026-11-21: run over the tree before the fix, it failed listing 184 cuts,
//! among them `src/engine/key_cards.rs:269 card.key[..12.min(card.key.len())]`; and the scanner is
//! run on a synthetic source first (a positive control), so a scanner that stopped seeing cuts
//! cannot pass by accident.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/text_cut_lint.rs

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file of the library code: under src/, and under each crates/<name>/src/.
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

/// The source with every comment and every string literal's inside turned to spaces (newlines and
/// the quotes kept), so code is read as code. Raw strings and nested block comments are handled; a
/// char literal's quote is not mistaken for a string, nor a lifetime for a char.
fn lex(src: &str) -> Vec<char> {
    let b: Vec<char> = src.chars().collect();
    let mut code: Vec<char> = b.clone();
    let n = b.len();
    let blank = |code: &mut Vec<char>, a: usize, z: usize| {
        for k in a..z.min(n) {
            if code[k] != '\n' && code[k] != '\r' {
                code[k] = ' ';
            }
        }
    };
    let mut i = 0;
    while i < n {
        let c = b[i];
        let d = if i + 1 < n { b[i + 1] } else { '\0' };
        if c == '/' && d == '/' {
            let mut j = i;
            while j < n && b[j] != '\n' {
                j += 1;
            }
            blank(&mut code, i, j);
            i = j;
            continue;
        }
        if c == '/' && d == '*' {
            let (mut depth, mut j) = (1, i + 2);
            while j < n && depth > 0 {
                if b[j] == '/' && j + 1 < n && b[j + 1] == '*' {
                    depth += 1;
                    j += 2;
                } else if b[j] == '*' && j + 1 < n && b[j + 1] == '/' {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            i = j;
            continue;
        }
        let ident_before = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
        if (c == 'r' || (c == 'b' && d == 'r')) && !ident_before {
            let mut j = if c == 'b' { i + 2 } else { i + 1 };
            let mut hashes = 0;
            while j < n && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && b[j] == '"' {
                let open = j;
                let mut k = j + 1;
                while k < n {
                    if b[k] == '"' && (0..hashes).all(|h| k + 1 + h < n && b[k + 1 + h] == '#') {
                        break;
                    }
                    k += 1;
                }
                blank(&mut code, open + 1, k);
                i = k + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            let mut j = i + 1;
            while j < n && b[j] != '"' {
                if b[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            blank(&mut code, i + 1, j);
            i = j + 1;
            continue;
        }
        if c == '\'' {
            // 'x', '\n', '\'' and '\u{..}' are char literals; 'a (a lifetime) is not.
            if i + 2 < n && b[i + 1] != '\\' && b[i + 2] == '\'' {
                blank(&mut code, i + 1, i + 2);
                i += 3;
                continue;
            }
            if i + 1 < n && b[i + 1] == '\\' {
                let mut j = i + 2;
                while j < n && j < i + 12 && b[j] != '\'' {
                    j += 1;
                }
                if j < n && b[j] == '\'' {
                    blank(&mut code, i + 1, j);
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    code
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// What the source says a receiver or a use is.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Text,
    NotText,
    Unknown,
}

/// A type written in the source, read as text or not.
fn type_kind(t: &str) -> Kind {
    let mut t = t.trim();
    loop {
        let before = t;
        t = t.trim_start_matches('&').trim_start();
        if t.starts_with('\'') {
            t = t.trim_start_matches(|c: char| c == '\'' || is_ident(c)).trim_start();
        }
        t = t.strip_prefix("mut ").unwrap_or(t).trim_start();
        t = t.strip_prefix("Option<").unwrap_or(t).trim_start();
        if t == before {
            break;
        }
    }
    let word: String = t.chars().take_while(|&c| is_ident(c)).collect();
    match word.as_str() {
        "" if t.starts_with('[') || t.starts_with('(') => Kind::NotText,
        "str" | "String" | "ArrayString" => Kind::Text,
        "Cow" if t.contains("str") => Kind::Text,
        "" => Kind::Unknown,
        _ => Kind::NotText,
    }
}

/// What a `let`'s initialiser makes, by the calls in it.
fn init_kind(init: &str) -> Kind {
    // The first statement, without the unwrapping at its end: what is left ends in the call that
    // made the value (`x.trim().split(' ').collect()` is a list, not text).
    let mut e = init.split(';').next().unwrap_or("").trim().to_string();
    loop {
        let before = e.len();
        for tail in ["?", ".unwrap()", ".unwrap_or_default()", ".ok()"] {
            if let Some(t) = e.strip_suffix(tail) {
                e = t.trim_end().to_string();
            }
        }
        if e.ends_with(')') && e.contains(".expect(") {
            e = e[..e.rfind(".expect(").unwrap_or(e.len())].to_string();
        }
        if e.len() == before {
            break;
        }
    }
    const TEXT_ENDS: &[&str] = &[
        ".to_string()", ".to_hex()", ".to_lowercase()", ".to_uppercase()", ".trim()", ".trim_start()", ".trim_end()", ".as_str()",
        ".to_string_lossy()", ".into_owned()",
    ];
    const TEXT_STARTS: &[&str] = &["format!(", "String::new", "String::from", "String::with_capacity", "hex::encode(", "std::fs::read_to_string(", "fs::read_to_string(", "std::str::from_utf8(", "str::from_utf8("];
    const NOT_ENDS: &[&str] = &[".as_bytes()", ".to_vec()", ".finalize()", ".collect()", ".into_bytes()"];
    const NOT_STARTS: &[&str] = &["vec![", "[0u8;", "b\"", "Vec::", "std::fs::read(", "fs::read("];
    if TEXT_ENDS.iter().any(|m| e.ends_with(m)) || TEXT_STARTS.iter().any(|m| e.starts_with(m)) || (e.ends_with(')') && e.contains(".join(\"")) {
        return Kind::Text;
    }
    if NOT_ENDS.iter().any(|m| e.ends_with(m)) || NOT_STARTS.iter().any(|m| e.starts_with(m)) || e.contains("collect::<Vec") {
        return Kind::NotText;
    }
    Kind::Unknown
}

/// Every struct field's declared types, by field name, over all the scanned files: (text, other).
fn field_types(sources: &[(String, Vec<char>)]) -> HashMap<String, (u32, u32)> {
    let mut out: HashMap<String, (u32, u32)> = HashMap::new();
    for (_, code) in sources {
        let text: String = code.iter().collect();
        for line in text.lines() {
            // `    pub name: Type,` or `    name: Type,` alone on its line.
            let l = line.trim();
            let l = l.strip_prefix("pub(crate) ").or_else(|| l.strip_prefix("pub(super) ")).or_else(|| l.strip_prefix("pub ")).unwrap_or(l);
            let Some(colon) = l.find(':') else { continue };
            let name = &l[..colon];
            let rest = &l[colon + 1..];
            if name.is_empty() || !name.chars().all(is_ident) || rest.starts_with(':') || !l.ends_with(',') {
                continue;
            }
            // A struct literal's `name: value,` is not a declaration: a value has a call, a field
            // access, a literal or a macro in it, or is a bare lower-case name; a type has none.
            let ty = rest.trim_end_matches(',').trim();
            let first = ty.trim_start_matches(|c: char| c == '&' || c == '(' || c == '[').chars().next().unwrap_or(' ');
            const PRIMITIVES: &[&str] = &["u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64", "bool", "char", "str", "mut", "dyn", "impl"];
            let head: String = ty.trim_start_matches(|c: char| c == '&' || c == '(' || c == '[' || c == '\'').chars().take_while(|&c| is_ident(c)).collect();
            if ty.contains('.') || ty.contains('"') || ty.contains('!') || (ty.contains('(') && !ty.starts_with('(')) || !(first.is_uppercase() || first == '\'' || PRIMITIVES.contains(&head.as_str())) {
                continue;
            }
            let e = out.entry(name.to_string()).or_default();
            match type_kind(ty) {
                Kind::Text => e.0 += 1,
                _ => e.1 += 1,
            }
        }
    }
    out
}

/// Does this line start a fn (its signature)?
fn is_signature(line: &str) -> bool {
    let t = line.trim_start();
    (t.starts_with("fn ") || t.contains(" fn ")) && t.contains('(')
}

/// The nearest declaration of `name` at or above line `ln` (0-based) of `lines`: what it says.
fn nearest_decl(lines: &[&str], ln: usize, name: &str) -> Kind {
    nearest_decl_from(lines, ln, name, 3)
}

/// `nearest_decl`, following `let a = b.clone();` (and `to_owned()`, `as_ref()`, a bare `b`) to
/// `b`'s own declaration, `hops` deep.
fn nearest_decl_from(lines: &[&str], ln: usize, name: &str, hops: u32) -> Kind {
    // What an initialiser says, or, when it only copies another name, what that name's says.
    let init_or_copy = |init: &str, at_line: usize| -> Kind {
        let first = init.split(';').next().unwrap_or("").trim();
        let copied = first.strip_suffix(".clone()").or_else(|| first.strip_suffix(".to_owned()")).or_else(|| first.strip_suffix(".as_ref()")).unwrap_or(first);
        if hops > 0 && !copied.is_empty() && copied != name && copied.chars().all(is_ident) {
            return nearest_decl_from(lines, at_line.saturating_sub(1), copied, hops - 1);
        }
        init_kind(init)
    };
    let lo = ln.saturating_sub(400);
    for i in (lo..=ln).rev() {
        let line = lines[i];
        // Past the signature of the fn the cut is in (read already: it holds the parameters), a
        // name belongs to another fn: stop.
        if i < ln && is_signature(lines[i + 1]) {
            break;
        }
        let lb: Vec<char> = line.chars().collect();
        let nb: Vec<char> = name.chars().collect();
        let mut p = 0;
        while p + nb.len() <= lb.len() {
            if lb[p..p + nb.len()] != nb[..] || (p > 0 && is_ident(lb[p - 1])) || (p + nb.len() < lb.len() && is_ident(lb[p + nb.len()])) {
                p += 1;
                continue;
            }
            let before: String = lb[..p].iter().collect();
            let after: String = lb[p + nb.len()..].iter().collect();
            let bt = before.trim_end();
            let at = after.trim_start();
            // What comes after an `=` that is an assignment (not `==` or `=>`), with the next lines.
            let init_after = |s: &str| -> Option<String> {
                let s = s.trim_start();
                let s = s.strip_prefix('=')?;
                if s.starts_with('=') || s.starts_with('>') {
                    return None;
                }
                let mut init = s.to_string();
                for more in lines.iter().skip(i + 1).take(2) {
                    init.push_str(more);
                }
                Some(init)
            };
            if bt.ends_with("let") || bt.ends_with("let mut") {
                if let Some(t) = at.strip_prefix(':') {
                    if !t.starts_with(':') {
                        return type_kind(t.split('=').next().unwrap_or(""));
                    }
                }
                return init_after(at).map_or(Kind::Unknown, |init| init_or_copy(&init, i));
            }
            if bt.ends_with("Some(") || bt.ends_with("Ok(") || bt.ends_with("Some(ref") {
                if let Some(rest) = at.strip_prefix(')') {
                    return init_after(rest).map_or(Kind::Unknown, |init| init_or_copy(&init, i));
                }
            }
            if let Some(t) = at.strip_prefix(':') {
                if !t.starts_with(':') && (bt.is_empty() || bt.ends_with('(') || bt.ends_with(',') || bt.ends_with('|') || bt.ends_with("mut")) {
                    return type_kind(t.split(|c| c == ',' || c == ')' || c == '|' || c == '=').next().unwrap_or(""));
                }
            }
            if bt.ends_with("for") && at.starts_with("in ") {
                return Kind::Unknown;
            }
            p += nb.len();
        }
    }
    Kind::Unknown
}

/// What the receiver of a cut is, by what makes it or how it is declared.
fn receiver_kind(recv: &str, lines: &[&str], ln: usize, fields: &HashMap<String, (u32, u32)>) -> Kind {
    let r: String = recv.chars().filter(|c| !c.is_whitespace()).collect();
    const TEXT_CALLS: &[&str] = &[
        "to_hex()", "to_string()", "as_str()", "to_lowercase()", "to_uppercase()", "trim()", "trim_start()", "trim_end()",
        "to_string_lossy()",
    ];
    const NOT_CALLS: &[&str] = &["as_bytes()", "as_slice()", "to_vec()", "finalize()", "public_key()"];
    if TEXT_CALLS.iter().any(|m| r.ends_with(m)) || r.starts_with("hex::encode(") || r.starts_with("format!(") {
        return Kind::Text;
    }
    if NOT_CALLS.iter().any(|m| r.ends_with(m)) || r.ends_with(')') || r.ends_with(']') {
        return if NOT_CALLS.iter().any(|m| r.ends_with(m)) { Kind::NotText } else { Kind::Unknown };
    }
    if let Some(dot) = r.rfind('.') {
        let field = &r[dot + 1..];
        return match fields.get(field) {
            Some(&(t, 0)) if t > 0 => Kind::Text,
            Some(_) => Kind::NotText,
            None => Kind::Unknown,
        };
    }
    if r.chars().all(is_ident) && !r.is_empty() {
        return nearest_decl(lines, ln, &r);
    }
    Kind::Unknown
}

/// The formatting macros whose arguments are text when a slice is one of them.
const FORMAT_MACROS: &[&str] = &[
    "format", "println", "eprintln", "print", "eprint", "write", "writeln", "panic", "unreachable", "todo", "format_args", "trace",
    "debug", "info", "warn", "error",
];

/// The checking macros, whose arguments are text only after their message's format string.
const ASSERT_MACROS: &[&str] = &["assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne"];

/// The format spec (what follows `:` inside the braces) of the placeholder that shows positional
/// argument `arg` of a format string: `{}` takes the next, `{2}` the third, `{name}` none.
fn placeholder_spec(fmt: &str, arg: usize) -> Option<String> {
    let chars: Vec<char> = fmt.chars().collect();
    let mut next = 0usize;
    let mut k = 0;
    while k < chars.len() {
        if chars[k] == '{' && chars.get(k + 1) == Some(&'{') {
            k += 2;
            continue;
        }
        if chars[k] == '{' {
            let inside: String = chars[k + 1..].iter().take_while(|&&c| c != '}').collect();
            let (name, spec) = inside.split_once(':').unwrap_or((inside.as_str(), ""));
            let index = if name.is_empty() {
                next += 1;
                Some(next - 1)
            } else {
                name.parse::<usize>().ok()
            };
            if index == Some(arg) {
                return Some(spec.to_string());
            }
            k += inside.chars().count() + 2;
            continue;
        }
        k += 1;
    }
    None
}

/// What the use of a cut says: (start of the receiver, the `]` that closes the cut) in `code`, whose
/// string literals `raw` (the source, position for position) still holds.
fn use_kind(code: &[char], raw: &[char], recv_at: usize, close: usize) -> Kind {
    let after: String = code[close + 1..code.len().min(close + 40)].iter().collect();
    let after = after.trim_start();
    const LIST_METHODS: &[&str] = &[".join(", ".iter(", ".concat(", ".to_vec(", ".copy_from_slice(", ".into_iter(", ".windows(", ".chunks("];
    if LIST_METHODS.iter().any(|m| after.starts_with(m)) {
        return Kind::NotText;
    }
    const TEXT_METHODS: &[&str] = &[
        ".to_string(", ".parse", ".chars(", ".char_indices(", ".trim", ".split", ".lines(", ".starts_with(\"", ".ends_with(\"",
        ".contains(\"", ".to_lowercase(", ".to_uppercase(", ".eq_ignore_ascii_case(", ".as_bytes(", ".to_ascii",
    ];
    if TEXT_METHODS.iter().any(|m| after.starts_with(m)) {
        return Kind::Text;
    }
    if (after.starts_with("==") || after.starts_with("!=")) && after[2..].trim_start().starts_with('"') {
        return Kind::Text;
    }
    let mut k = recv_at;
    while k > 0 && (code[k - 1].is_whitespace() || code[k - 1] == '&') {
        k -= 1;
    }
    let before: String = code[k.saturating_sub(40)..k].iter().collect();
    const TEXT_TAKERS: &[&str] = &[
        "from_str_radix(", "RichText::new(", ".label(", "push_str(", "String::from(", ".starts_with(", ".ends_with(", ".contains(",
        "strip_prefix(", "strip_suffix(",
    ];
    if TEXT_TAKERS.iter().any(|m| before.ends_with(m)) {
        return Kind::Text;
    }
    // Inside a formatting macro's arguments: the first unclosed `(` before the receiver, within the
    // statement, follows `name!`.
    let mut depth = 0i32;
    let mut j = recv_at;
    while j > 0 {
        j -= 1;
        match code[j] {
            ')' | ']' => depth += 1,
            '(' | '[' if depth > 0 => depth -= 1,
            '(' => {
                let mut e = j;
                while e > 0 && code[e - 1].is_whitespace() {
                    e -= 1;
                }
                // The cut is an argument of a call: what the call makes is shown, not the cut.
                if e == 0 || code[e - 1] != '!' {
                    return Kind::Unknown;
                }
                if e > 0 && code[e - 1] == '!' {
                    let mut s = e - 1;
                    while s > 0 && is_ident(code[s - 1]) {
                        s -= 1;
                    }
                    let name: String = code[s..e - 1].iter().collect();
                    let format = FORMAT_MACROS.contains(&name.as_str());
                    // In a check, only an argument after the message's format string is shown.
                    let check = ASSERT_MACROS.contains(&name.as_str());
                    if !format && !check {
                        // Another macro (vec!, matches!, ...): keep looking outward.
                        continue;
                    }
                    // assert! takes one argument before its message, assert_eq!/assert_ne! two.
                    let skip = if !check { 0 } else if name.ends_with("_eq") || name.ends_with("_ne") { 2 } else { 1 };
                    let mut commas = 0;
                    let mut dep = 0i32;
                    let mut open_q = None;
                    for q in j + 1..recv_at {
                        match code[q] {
                            '(' | '[' | '{' => dep += 1,
                            ')' | ']' | '}' => dep -= 1,
                            ',' if dep == 0 => commas += 1,
                            '"' if dep == 0 && commas >= skip => {
                                open_q = Some(q);
                                break;
                            }
                            _ => {}
                        }
                    }
                    let Some(open_q) = open_q else { return Kind::Unknown };
                    let Some(close_q) = (open_q + 1..recv_at).find(|&q| code[q] == '"') else { return Kind::Unknown };
                    // The source's format string, and which argument after it the cut is in.
                    let fmt: String = raw[open_q + 1..close_q].iter().collect();
                    let mut arg = None::<usize>;
                    let mut dep = 0i32;
                    for q in close_q + 1..recv_at {
                        match code[q] {
                            '(' | '[' | '{' => dep += 1,
                            ')' | ']' | '}' => dep -= 1,
                            ',' if dep == 0 => arg = Some(arg.map_or(0, |a| a + 1)),
                            _ => {}
                        }
                    }
                    let Some(arg) = arg else { continue };
                    // A cut shown with `{:?}` or `{:x}` may well be bytes or a list: then the
                    // receiver decides. Shown with `{}`, it is text.
                    return match placeholder_spec(&fmt, arg) {
                        Some(spec) if spec.contains('?') || spec.contains('x') || spec.contains('X') => Kind::Unknown,
                        _ => Kind::Text,
                    };
                }
            }
            '[' => {}
            '{' | '}' | ';' if depth == 0 => break,
            _ => {}
        }
    }
    Kind::Unknown
}

/// A cut the scan reports: its line (1-based), column of the receiver (1-based), the receiver and
/// the range as written, and whether it is flagged (text evidence and no `text-cut-ok:` mark).
#[derive(Debug)]
struct Cut {
    line: usize,
    col: usize,
    recv: String,
    range: String,
    flagged: bool,
}

/// Every count-shaped range cut in one file's source, flagged or not.
fn cuts_in(src: &str, code: &[char], fields: &HashMap<String, (u32, u32)>) -> Vec<Cut> {
    let n = code.len();
    let code_text: String = code.iter().collect();
    let code_lines: Vec<&str> = code_text.lines().collect();
    let raw_lines: Vec<&str> = src.lines().collect();
    let raw: Vec<char> = src.chars().collect();
    let mut line_starts = vec![0usize];
    for (k, &c) in code.iter().enumerate() {
        if c == '\n' {
            line_starts.push(k + 1);
        }
    }
    let line_col = |at: usize| -> (usize, usize) {
        let l = line_starts.partition_point(|&s| s <= at) - 1;
        (l + 1, at - line_starts[l] + 1)
    };
    const KEYWORDS: &[&str] = &["in", "return", "break", "else", "match", "if", "let", "mut", "as", "yield", "move", "for", "while", "loop", "where", "impl", "dyn", "const", "static", "type", "ref"];
    let mut out = Vec::new();
    for i in 0..n {
        if code[i] != '[' {
            continue;
        }
        // An index follows a name, a call's `)`, an index's `]` or a `?`; anything else opens an
        // array, a type, a pattern or an attribute.
        let mut p = i;
        while p > 0 && code[p - 1].is_whitespace() {
            p -= 1;
        }
        if p == 0 {
            continue;
        }
        let pc = code[p - 1];
        if !(is_ident(pc) || pc == ')' || pc == ']' || pc == '?') {
            continue;
        }
        if is_ident(pc) {
            let mut q = p - 1;
            while q > 0 && is_ident(code[q - 1]) {
                q -= 1;
            }
            let word: String = code[q..p].iter().collect();
            if KEYWORDS.contains(&word.as_str()) || (q > 0 && code[q - 1] == '\'') {
                continue;
            }
        }
        // The matching `]`.
        let mut depth = 0i32;
        let mut close = i;
        while close < n {
            match code[close] {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            close += 1;
        }
        if close >= n {
            continue;
        }
        let inner: Vec<char> = code[i + 1..close].to_vec();
        let mut dd = None;
        let mut d = 0i32;
        for k in 0..inner.len().saturating_sub(1) {
            match inner[k] {
                '(' | '[' | '{' => d += 1,
                ')' | ']' | '}' => d -= 1,
                '.' if d == 0 && inner[k + 1] == '.' => {
                    dd = Some(k);
                    break;
                }
                _ => {}
            }
        }
        let Some(dd) = dd else { continue };
        let start: String = inner[..dd].iter().collect::<String>().trim().to_string();
        let end: String = inner[dd + 2..].iter().collect::<String>().trim_start_matches('=').trim().to_string();
        if start.is_empty() && end.is_empty() {
            continue;
        }
        let literal = |x: &str| !x.is_empty() && x.trim_end_matches("usize").chars().all(|c| c.is_ascii_digit() || c == '_');
        let clamp = |x: &str| x.contains("min(");
        let from_len = |x: &str| x.contains("len()") && (x.contains('-') || x.contains("saturating_sub") || x.contains("checked_sub"));
        // A window of a counted width at a computed start (`[i..i + 2]`, the hex reader's cut).
        let window = !start.is_empty() && end.strip_prefix(start.as_str()).map(str::trim_start).and_then(|r| r.strip_prefix('+')).is_some_and(|w| literal(w.trim()));
        if !(literal(&start) || literal(&end) || clamp(&start) || clamp(&end) || from_len(&start) || from_len(&end) || window) {
            continue;
        }
        // The receiver: back over a postfix expression (names, paths, fields, calls, indexes, `?`).
        let mut r = p;
        let mut rd = 0i32;
        while r > 0 {
            let c = code[r - 1];
            if rd > 0 {
                if c == ')' || c == ']' {
                    rd += 1;
                } else if c == '(' || c == '[' {
                    rd -= 1;
                }
                r -= 1;
                continue;
            }
            if c == ')' || c == ']' {
                rd += 1;
                r -= 1;
            } else if is_ident(c) || c == '.' || c == '?' || c == '!' {
                r -= 1;
            } else if c == ':' && r >= 2 && code[r - 2] == ':' {
                r -= 2;
            } else {
                break;
            }
        }
        let recv: String = code[r..p].iter().collect();
        let (line, col) = line_col(r);
        let kind = match use_kind(code, &raw, r, close) {
            Kind::Text => Kind::Text,
            Kind::NotText => Kind::NotText,
            Kind::Unknown => receiver_kind(&recv, &code_lines, line - 1, fields),
        };
        let marked = [line.checked_sub(1), line.checked_sub(2)]
            .into_iter()
            .flatten()
            .filter_map(|l| raw_lines.get(l))
            .any(|l| l.contains("text-cut-ok:"));
        out.push(Cut { line, col, recv, range: format!("[{start}..{end}]"), flagged: kind == Kind::Text && !marked });
    }
    out
}

/// The scanned files' sources, lexed.
fn lexed_sources() -> Vec<(String, String, Vec<char>)> {
    rust_sources()
        .into_iter()
        .filter_map(|p| {
            let src = fs::read_to_string(&p).ok()?;
            let code = lex(&src);
            Some((rel(&p), src, code))
        })
        .collect()
}

#[test]
fn the_scanner_catches_what_it_is_for() {
    let src = "fn f(key: &str, bytes: &[u8], names: Vec<String>) {\n\
               let a = &key[..12.min(key.len())];\n\
               let b = &bytes[..4];\n\
               log::warn!(\"from {}\", &peer.id[..8]);\n\
               let c = &key[key.len().saturating_sub(4)..];\n\
               let d = names[..2].join(\", \");\n\
               let e = &key[..pos];\n\
               // text-cut-ok: the hex of a hash is ASCII\n\
               let f = &h.to_hex()[..16];\n\
               let g = h.to_hex()[..16].to_string();\n\
               let s = \"a [..4] in a string\"; // and &key[..4] in a comment\n\
               let t: &str = \"abc\"; let u = &t[1..];\n\
               let v = u8::from_str_radix(&h[i..i + 2], 16); let w = &buf[i..i + 4];\n\
               }\n";
    let code = lex(src);
    let fields = HashMap::new();
    let got: Vec<(usize, String)> = cuts_in(src, &code, &fields).into_iter().filter(|c| c.flagged).map(|c| (c.line, c.recv)).collect();
    let want: Vec<(usize, String)> =
        vec![(2, "key".into()), (4, "peer.id".into()), (5, "key".into()), (10, "h.to_hex()".into()), (12, "t".into()), (13, "h".into())];
    assert_eq!(got, want);
}

#[test]
fn no_text_is_cut_by_bytes() {
    let files = lexed_sources();
    // The walk must see both halves of the library code: a walk that stopped reading src/ or
    // crates/ would pass on nothing.
    let in_src = files.iter().filter(|f| f.0.starts_with("src/")).count();
    let in_crates = files.iter().filter(|f| f.0.starts_with("crates/")).count();
    assert!(in_src > 500, "the walk found only {in_src} files under src/");
    assert!(in_crates > 50, "the walk found only {in_crates} files under crates/*/src/");
    let fields = field_types(&files.iter().map(|f| (f.0.clone(), f.2.clone())).collect::<Vec<_>>());
    let mut problems = Vec::new();
    let mut dump = Vec::new();
    for (path, src, code) in &files {
        if !src.contains("..") {
            continue;
        }
        for c in cuts_in(src, code, &fields) {
            dump.push(format!("{}\t{path}\t{}\t{}\t{}\t{}", c.flagged, c.line, c.col, c.recv, c.range));
            if c.flagged {
                problems.push(format!("  {path}:{}: {}{}", c.line, c.recv.trim(), c.range));
            }
        }
    }
    // For measuring the scan against clippy's typed check: TEXT_CUT_DUMP=<file> writes every
    // count-shaped cut it read, flagged or not.
    if let Ok(out) = std::env::var("TEXT_CUT_DUMP") {
        fs::write(out, dump.join("\n")).ok();
    }
    assert!(
        problems.is_empty(),
        "\n\nText cut by bytes, which panics when the cut falls inside a character ({} places):\n{}\n\n\
         Cut by characters with humanity_core::text::prefix(s, n) / suffix(s, n) (or prefix_bytes / \
         suffix_bytes for a limit in bytes, clip / ends for the short form of a long key). A byte cut \
         truly meant on ASCII, or a slice of bytes or of a list the scan took for text, gets a \
         `text-cut-ok: <why>` comment on its line or the line above. tests/text_cut_lint.rs says how \
         the scan decides what is text.\n",
        problems.len(),
        problems.join("\n")
    );
}
