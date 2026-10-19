//! A statement in `Storage::open` must not name a column before the ALTER TABLE that adds it.
//!
//! THE FAILURE (BUG-046, v0.675.0, the live relay down for about 25 minutes):
//! `src/relay/storage/mod.rs` builds the schema in one long function, `Storage::open`.
//! A table's `CREATE TABLE IF NOT EXISTS` lists every column, including the ones added
//! later, and a guarded `ALTER TABLE ... ADD COLUMN` further down adds each of those to a
//! database that is older than the column. On a FRESH database the CREATE makes the
//! column, so every unit test and every local smoke test passes. On the LIVE database the
//! table already exists without it, the CREATE is a no-op, and any statement that names
//! the column before its ALTER has run fails. v0.675.0 put
//! `CREATE INDEX idx_user_uploads_shared ON user_uploads(shared, id)` in the first schema
//! batch; the batch aborted and the relay exited at startup. The same class had already
//! crash-looped the relay once (v0.262.2, 2026-05-17), that time with the roles seed
//! INSERT naming columns whose ALTERs ran after it. Both times every test was green.
//!
//! WHAT THIS CHECKS. Every `ALTER TABLE <t> ADD [COLUMN] <c>` in `Storage::open` is
//! recorded with its position. Then every statement in `Storage::open` that names
//! `<t>.<c>` must sit AFTER that ALTER in the source (the function is straight-line code,
//! so source order is run order). The statements read:
//!   CREATE [UNIQUE] INDEX ... ON <t>(...) [WHERE ...]   every column and expression
//!   INSERT / REPLACE INTO <t> (cols)                     the column list (v0.262.2)
//!   UPDATE <t> SET ... [WHERE ...]                       names outside sub-queries
//!   DELETE FROM <t> WHERE ...                            names outside sub-queries
//!   CREATE TRIGGER ... ON <t>                            UPDATE OF list, new.x/old.x, and
//!                                                        the INSERT/UPDATE/DELETE in its body
//!   CREATE VIEW ... AS SELECT ... FROM <t>               any name that is a column of <t>
//! Measured with SQLite 3.51 (node:sqlite) on 2026-10-19 against a table missing the
//! column: an index (plain, partial or expression), an INSERT and an UPDATE are REJECTED
//! at once ("no such column: b", "table t has no column named b"), which aborts the batch.
//! A view and a trigger are accepted at CREATE time and fail when first used ("no such
//! column: new.b" on the first insert into the trigger's table). They are checked for
//! order all the same: a trigger fires on the first insert into its table, and the
//! seed INSERTs in `Storage::open` are exactly such inserts.
//!
//! NOT checked: a plain SELECT. The guarded ALTERs probe with `SELECT <c> FROM <t> LIMIT 0`
//! BEFORE the ALTER on purpose (the probe failing is how the code knows to add it).
//!
//! Any ALTER the scanner cannot read (a table or column built with `format!`) FAILS the
//! test rather than being skipped, so the check never goes quietly blind.
//!
//! Why a source scan and not a lib test that opens an old-shaped database: a lib test would
//! have to build the "before" shape of every table by deleting ALTER-added columns from the
//! CREATE TABLE text, which is the same parsing with a slower loop (the relay test build),
//! and could not run in `just lints` or the pre-commit hook. This runs in well under a
//! second, std only, compiled standalone with rustc so it never links the native bin
//! (the Windows LNK1318 limit). The per-change pre-migration test
//! (`opens_a_pre_v0675_database_and_migrates_it` in storage/uploads.rs) remains the
//! pattern for testing a specific migration's behaviour.
//!
//! PROVEN RED, 2026-10-19: `CREATE INDEX IF NOT EXISTS idx_user_uploads_shared ON
//! user_uploads(shared, id);` added back to the end of the first schema batch (the v0.675.0
//! shape). The run printed:
//!
//!   schema_alter_order_lint: 70 ALTER-added columns, 92 CREATE INDEX, 421 statements
//!     read in Storage::open, 40 places naming an ALTER-added column
//!   [FAIL] Storage::open names 1 column(s) before the ALTER TABLE that adds them.
//!     src/relay/storage/mod.rs:657  CREATE INDEX names user_uploads.shared
//!         but that column is added by the ALTER at src/relay/storage/mod.rs:854, which runs later.
//!
//! Restored, it passes (70 columns, 91 indexes, 420 statements, 39 places, 0 out of order). The in-file fixtures below (`fixture_*`) repeat the v0.675.0,
//! v0.262.2, trigger and view shapes on every run, so the scanner proves it can still
//! fail each time it passes.
//!
//! Run: `just lints` (or standalone:
//!   CARGO_MANIFEST_DIR="$(pwd)" rustc --test --edition 2021 -A warnings \
//!       tests/schema_alter_order_lint.rs -o /tmp/t.exe && /tmp/t.exe)

use std::collections::HashMap;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const SCHEMA_FILE: &str = "src/relay/storage/mod.rs";
const OPEN_SIGNATURE: &str = "pub fn open(path: &Path)";

// ---------------------------------------------------------------------------------------
// Rust lexing: just enough to find string literals and the body of one function, while
// never mistaking a `"` inside a comment, a char literal or a raw string for a delimiter.
// ---------------------------------------------------------------------------------------

/// A Rust string literal: the byte offset of its first content byte, and its raw text
/// (escapes left as written; SQL needs none of them decoded).
struct Lit {
    start: usize,
    text: String,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Every string literal in `src`, plus a mask that is true for bytes that are code
/// (not comment, not inside a literal).
fn lex_rust(src: &str) -> (Vec<Lit>, Vec<bool>) {
    let b = src.as_bytes();
    let mut code = vec![false; b.len()];
    let mut lits = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Line comment.
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment (nesting, as Rust's are).
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw string: r"..." / r#"..."# (also br), only at a token boundary, so the
        // `r"` at the end of `"Moderator",` is not one.
        let boundary = i == 0 || !is_ident(b[i - 1]);
        let raw_at = if c == b'r' && boundary {
            Some(i + 1)
        } else if c == b'b' && boundary && i + 1 < b.len() && b[i + 1] == b'r' {
            Some(i + 2)
        } else {
            None
        };
        if let Some(mut j) = raw_at {
            let mut hashes = 0;
            while j < b.len() && b[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let start = j + 1;
                let mut k = start;
                'raw: while k < b.len() {
                    if b[k] == b'"' {
                        let mut n = 0;
                        while n < hashes && k + 1 + n < b.len() && b[k + 1 + n] == b'#' {
                            n += 1;
                        }
                        if n == hashes {
                            break 'raw;
                        }
                    }
                    k += 1;
                }
                lits.push(Lit { start, text: src[start..k.min(b.len())].to_string() });
                i = k + 1 + hashes;
                continue;
            }
        }
        // Normal (or byte) string.
        if c == b'"' {
            let start = i + 1;
            let mut k = start;
            while k < b.len() && b[k] != b'"' {
                if b[k] == b'\\' {
                    k += 1;
                }
                k += 1;
            }
            lits.push(Lit { start, text: src[start..k.min(b.len())].to_string() });
            i = k + 1;
            continue;
        }
        // Char literal or lifetime: '"' must not open a string.
        if c == b'\'' {
            if i + 1 < b.len() && b[i + 1] == b'\\' {
                let mut k = i + 2;
                while k < b.len() && b[k] != b'\'' {
                    k += 1;
                }
                i = k + 1;
                continue;
            }
            if i + 2 < b.len() && b[i + 2] == b'\'' {
                i += 3;
                continue;
            }
            code[i] = true;
            i += 1;
            continue;
        }
        code[i] = true;
        i += 1;
    }
    (lits, code)
}

/// The byte range of the body of the function whose signature is `signature`.
fn function_body(src: &str, code: &[bool], signature: &str) -> Result<(usize, usize), String> {
    let at = src
        .find(signature)
        .ok_or_else(|| format!("could not find `{signature}` in {SCHEMA_FILE}; the lint must be pointed at the function that builds the schema"))?;
    let b = src.as_bytes();
    let mut i = at;
    while i < b.len() && !(b[i] == b'{' && code[i]) {
        i += 1;
    }
    let open = i;
    let mut depth = 0i32;
    while i < b.len() {
        if code[i] {
            if b[i] == b'{' {
                depth += 1;
            } else if b[i] == b'}' {
                depth -= 1;
                if depth == 0 {
                    return Ok((open, i));
                }
            }
        }
        i += 1;
    }
    Err(format!("`{signature}` has no closing brace; the lexer is probably broken"))
}

// ---------------------------------------------------------------------------------------
// SQL tokens and statements.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    /// An identifier, keyword or number, lowercased. Quoted identifiers unquoted.
    Word(String),
    /// A '...' string literal (its contents never name a column).
    Str,
    Punct(char),
}

#[derive(Clone, Debug)]
struct T {
    tok: Tok,
    /// Byte offset within the literal's text.
    off: usize,
}

fn sql_tokens(text: &str) -> Vec<T> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() || c == b'\\' {
            // A backslash is a Rust escape in the raw literal text (\" or a line
            // continuation); it never matters to the SQL meaning we read.
            i += 1;
            continue;
        }
        if c == b'-' && i + 1 < b.len() && b[i + 1] == b'-' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == b'\'' {
            let start = i;
            i += 1;
            while i < b.len() {
                if b[i] == b'\'' {
                    if i + 1 < b.len() && b[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            i += 1;
            out.push(T { tok: Tok::Str, off: start });
            continue;
        }
        if c == b'"' || c == b'`' || c == b'[' {
            let close = if c == b'[' { b']' } else { c };
            let start = i;
            i += 1;
            let s = i;
            while i < b.len() && b[i] != close && b[i] != b'\\' {
                i += 1;
            }
            let word = text[s..i].to_ascii_lowercase();
            // Skip the closing quote, written \" inside a Rust string.
            if i < b.len() && b[i] == b'\\' {
                i += 1;
            }
            i += 1;
            out.push(T { tok: Tok::Word(word), off: start });
            continue;
        }
        if is_ident(c) {
            let start = i;
            while i < b.len() && is_ident(b[i]) {
                i += 1;
            }
            out.push(T { tok: Tok::Word(text[start..i].to_ascii_lowercase()), off: start });
            continue;
        }
        // Skip a whole UTF-8 sequence for non-ASCII bytes (an emoji DEFAULT, say).
        let len = if c < 0x80 { 1 } else if c >= 0xF0 { 4 } else if c >= 0xE0 { 3 } else { 2 };
        out.push(T { tok: Tok::Punct(c as char), off: i });
        i += len;
    }
    out
}

fn word(t: &T) -> Option<&str> {
    match &t.tok {
        Tok::Word(w) => Some(w.as_str()),
        _ => None,
    }
}

fn is_word(t: Option<&T>, w: &str) -> bool {
    t.and_then(word) == Some(w)
}

/// Split a token list into statements on `;`, keeping a trigger's BEGIN ... END body
/// (whose inner statements end in `;` too) inside its CREATE TRIGGER statement.
fn split_statements(toks: &[T]) -> Vec<Vec<T>> {
    let mut out = Vec::new();
    let mut cur: Vec<T> = Vec::new();
    let mut in_body = false;
    let mut case_depth = 0;
    for t in toks {
        let is_trigger = {
            let mut w = cur.iter().filter_map(word);
            matches!(w.next(), Some("create"))
                && cur.iter().filter_map(word).take(4).any(|x| x == "trigger")
        };
        match &t.tok {
            Tok::Punct(';') if !in_body => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                continue;
            }
            Tok::Word(w) if is_trigger => {
                if !in_body && w == "begin" {
                    in_body = true;
                    case_depth = 0;
                } else if in_body && w == "case" {
                    case_depth += 1;
                } else if in_body && w == "end" {
                    if case_depth > 0 {
                        case_depth -= 1;
                    } else {
                        in_body = false;
                    }
                }
            }
            _ => {}
        }
        cur.push(t.clone());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// One place a statement names `table.column`, at a byte offset in the literal.
#[derive(Debug)]
struct Ref {
    table: String,
    column: String,
    off: usize,
    kind: &'static str,
}

/// Index of the token after `create [unique|temp|temporary] <kind> [if not exists] <name>`.
fn after_create_name(s: &[T], kind: &str) -> Option<usize> {
    let mut i = 1;
    while i < s.len() && word(&s[i]) != Some(kind) {
        i += 1;
        if i > 3 {
            return None;
        }
    }
    i += 1;
    if is_word(s.get(i), "if") && is_word(s.get(i + 1), "not") && is_word(s.get(i + 2), "exists") {
        i += 3;
    }
    // The name, possibly schema-qualified.
    i += 1;
    if matches!(s.get(i).map(|t| &t.tok), Some(Tok::Punct('.'))) {
        i += 2;
    }
    Some(i)
}

/// Words at paren depth 0 in `s[from..]` (names in sub-queries belong to other tables).
fn depth0_words(s: &[T], from: usize) -> Vec<&T> {
    let mut depth = 0;
    let mut out = Vec::new();
    for t in s.iter().skip(from) {
        match &t.tok {
            Tok::Punct('(') => depth += 1,
            Tok::Punct(')') => depth -= 1,
            Tok::Word(_) if depth == 0 => out.push(t),
            _ => {}
        }
    }
    out
}

/// What this statement names. `Err` for an ALTER the scanner cannot read.
enum Stmt {
    Alter { table: String, column: String, off: usize },
    Names(Vec<Ref>),
}

fn classify(s: &[T]) -> Result<Stmt, String> {
    let w0 = s.first().and_then(word).unwrap_or("");
    let w = |i: usize| s.get(i).and_then(word);
    let mut refs = Vec::new();
    match w0 {
        "alter" if w(1) == Some("table") => {
            let table = w(2).ok_or("an ALTER TABLE whose table is not a plain name (built with format!?)")?;
            let mut i = 3;
            if matches!(s.get(3).map(|t| &t.tok), Some(Tok::Punct('.'))) {
                i = 5;
            }
            let table = if i == 5 { w(4).unwrap_or(table) } else { table };
            if w(i) != Some("add") {
                return Ok(Stmt::Names(refs)); // RENAME / DROP COLUMN: nothing is added
            }
            let mut j = i + 1;
            if w(j) == Some("column") {
                j += 1;
            }
            let column = w(j).ok_or("an ALTER TABLE ... ADD COLUMN whose column is not a plain name (built with format!?)")?;
            return Ok(Stmt::Alter { table: table.to_string(), column: column.to_string(), off: s[0].off });
        }
        "create" => {
            let kinds: Vec<&str> = s.iter().take(4).filter_map(word).collect();
            if kinds.contains(&"index") {
                let i = after_create_name(s, "index").unwrap_or(s.len());
                if w(i) == Some("on") {
                    if let Some(table) = w(i + 1) {
                        for t in s.iter().skip(i + 2) {
                            if let Some(c) = word(t) {
                                refs.push(Ref { table: table.into(), column: c.into(), off: t.off, kind: "CREATE INDEX" });
                            }
                        }
                    }
                }
            } else if kinds.contains(&"trigger") {
                let start = after_create_name(s, "trigger").unwrap_or(s.len());
                // Find `ON <table>` at depth 0 before BEGIN.
                let mut i = start;
                let mut of_from = None;
                while i < s.len() && w(i) != Some("on") && w(i) != Some("begin") {
                    if w(i) == Some("of") {
                        of_from = Some(i + 1);
                    }
                    i += 1;
                }
                if w(i) == Some("on") {
                    let table = w(i + 1).unwrap_or("").to_string();
                    if let Some(of) = of_from {
                        for t in &s[of..i] {
                            if let Some(c) = word(t) {
                                refs.push(Ref { table: table.clone(), column: c.into(), off: t.off, kind: "CREATE TRIGGER (UPDATE OF)" });
                            }
                        }
                    }
                    // new.x / old.x anywhere after ON.
                    for k in i..s.len().saturating_sub(2) {
                        if matches!(w(k), Some("new") | Some("old")) && matches!(s[k + 1].tok, Tok::Punct('.')) {
                            if let Some(c) = w(k + 2) {
                                refs.push(Ref { table: table.clone(), column: c.into(), off: s[k + 2].off, kind: "CREATE TRIGGER (new./old.)" });
                            }
                        }
                    }
                    // The statements in its body, each against its own table.
                    if let Some(b) = (i..s.len()).find(|&k| w(k) == Some("begin")) {
                        let body: Vec<T> = s[b + 1..].to_vec();
                        let mut inner: Vec<T> = Vec::new();
                        for t in body.into_iter().chain(std::iter::once(T { tok: Tok::Punct(';'), off: 0 })) {
                            if matches!(t.tok, Tok::Punct(';')) {
                                if let Ok(Stmt::Names(r)) = classify(&inner) {
                                    for mut x in r {
                                        x.kind = "CREATE TRIGGER (body)";
                                        refs.push(x);
                                    }
                                }
                                inner.clear();
                            } else {
                                inner.push(t);
                            }
                        }
                    }
                }
            } else if kinds.contains(&"view") {
                let tables: Vec<String> = s
                    .windows(2)
                    .filter(|p| matches!(word(&p[0]), Some("from") | Some("join")))
                    .filter_map(|p| word(&p[1]).map(String::from))
                    .collect();
                for t in s.iter() {
                    if let Some(c) = word(t) {
                        for table in &tables {
                            refs.push(Ref { table: table.clone(), column: c.into(), off: t.off, kind: "CREATE VIEW" });
                        }
                    }
                }
            }
        }
        "insert" | "replace" => {
            let into = (1..s.len().min(5)).find(|&k| w(k) == Some("into"));
            if let Some(k) = into {
                if let Some(table) = w(k + 1) {
                    if matches!(s.get(k + 2).map(|t| &t.tok), Some(Tok::Punct('('))) {
                        for t in s.iter().skip(k + 3) {
                            if matches!(t.tok, Tok::Punct(')')) {
                                break;
                            }
                            if let Some(c) = word(t) {
                                refs.push(Ref { table: table.into(), column: c.into(), off: t.off, kind: "INSERT" });
                            }
                        }
                    }
                }
            }
        }
        "update" => {
            let mut k = 1;
            if w(1) == Some("or") {
                k = 3;
            }
            if let Some(table) = w(k) {
                for t in depth0_words(s, k + 1) {
                    refs.push(Ref { table: table.into(), column: word(t).unwrap().into(), off: t.off, kind: "UPDATE" });
                }
            }
        }
        "delete" if w(1) == Some("from") => {
            if let Some(table) = w(2) {
                for t in depth0_words(s, 3) {
                    refs.push(Ref { table: table.into(), column: word(t).unwrap().into(), off: t.off, kind: "DELETE" });
                }
            }
        }
        _ => {}
    }
    Ok(Stmt::Names(refs))
}

// ---------------------------------------------------------------------------------------
// The check.
// ---------------------------------------------------------------------------------------

struct Report {
    alters: usize,
    statements: usize,
    indexes: usize,
    /// How many places name an ALTER-added column at all (in either order): proof the
    /// matcher is matching, not just that nothing was out of order.
    checked: usize,
    violations: Vec<String>,
}

fn line_of(src: &str, off: usize) -> usize {
    src[..off.min(src.len())].bytes().filter(|&b| b == b'\n').count() + 1
}

fn check(src: &str, file: &str) -> Result<Report, String> {
    let (lits, code) = lex_rust(src);
    let (lo, hi) = function_body(src, &code, OPEN_SIGNATURE)?;

    // Every statement in the function, with its absolute byte offset.
    let mut stmts: Vec<(usize, Vec<T>)> = Vec::new();
    for lit in lits.iter().filter(|l| l.start > lo && l.start < hi) {
        let toks = sql_tokens(&lit.text);
        for s in split_statements(&toks) {
            let abs = lit.start + s[0].off;
            let rebased: Vec<T> = s.into_iter().map(|t| T { off: lit.start + t.off, tok: t.tok }).collect();
            stmts.push((abs, rebased));
        }
    }

    // First ALTER that adds each (table, column).
    let mut added: HashMap<(String, String), usize> = HashMap::new();
    let mut unreadable = Vec::new();
    let mut named = Vec::new();
    let mut indexes = 0;
    for (abs, s) in &stmts {
        if s.iter().take(4).filter_map(word).any(|x| x == "index") && word(&s[0]) == Some("create") {
            indexes += 1;
        }
        match classify(s) {
            Ok(Stmt::Alter { table, column, off }) => {
                let e = added.entry((table, column)).or_insert(off);
                *e = (*e).min(off);
            }
            Ok(Stmt::Names(r)) => named.extend(r),
            Err(why) => unreadable.push(format!("  {file}:{}  {why}", line_of(src, *abs))),
        }
    }
    if !unreadable.is_empty() {
        return Err(format!(
            "\n\n[FAIL] The schema lint cannot read {} ALTER TABLE statement(s) in Storage::open:\n\n{}\n\n\
             Write each ALTER TABLE ... ADD COLUMN as a plain string literal (as every other one in\n\
             the file is), so this lint can see which column it adds. An ALTER it cannot read is a\n\
             column it cannot protect, and a lint that skips it quietly is worse than none.\n",
            unreadable.len(),
            unreadable.join("\n")
        ));
    }

    let mut violations: Vec<String> = Vec::new();
    let mut checked = 0;
    for r in &named {
        if let Some(&alter_at) = added.get(&(r.table.clone(), r.column.clone())) {
            checked += 1;
            if r.off < alter_at {
                violations.push(format!(
                    "  {file}:{}  {} names {}.{}\n      but that column is added by the ALTER at {file}:{}, which runs later.",
                    line_of(src, r.off),
                    r.kind,
                    r.table,
                    r.column,
                    line_of(src, alter_at)
                ));
            }
        }
    }
    violations.sort();
    violations.dedup();
    Ok(Report { alters: added.len(), statements: stmts.len(), indexes, checked, violations })
}

fn failure_message(file: &str, v: &[String]) -> String {
    format!(
        "\n\n[FAIL] Storage::open names {} column(s) before the ALTER TABLE that adds them.\n\n{}\n\n\
         Why this matters: on a FRESH database the CREATE TABLE already has the column, so\n\
         every test passes. On the LIVE database the table exists without it until its\n\
         ALTER runs, so an index, INSERT or UPDATE that names it first fails, the whole\n\
         batch aborts, and the relay exits at startup. That is BUG-046 (v0.675.0, the live\n\
         relay down about 25 minutes) and the v0.262.2 crash loop (2026-05-17, the roles\n\
         seed INSERT). A trigger or view is accepted at CREATE time but fails on first use.\n\n\
         Fix: move the statement into its own conn.execute_batch(...) AFTER the ALTER (see\n\
         idx_user_uploads_shared, right after the profiles/user_uploads ALTER loop in\n\
         {file}), and add a test that opens a database shaped like the live one\n\
         (pattern: opens_a_pre_v0675_database_and_migrates_it in src/relay/storage/uploads.rs).\n\
         Do not silence this test: it is the only thing that sees the live database's shape.\n",
        v.len(),
        v.join("\n")
    )
}

#[test]
fn storage_open_names_no_column_before_its_alter() {
    let path = repo_root().join(SCHEMA_FILE);
    let src = std::fs::read_to_string(&path).expect("read src/relay/storage/mod.rs");
    let report = match check(&src, SCHEMA_FILE) {
        Ok(r) => r,
        Err(msg) => panic!("{msg}"),
    };
    println!(
        "schema_alter_order_lint: {} ALTER-added columns, {} CREATE INDEX, {} statements read in Storage::open, {} places naming an ALTER-added column",
        report.alters, report.indexes, report.statements, report.checked
    );
    // A scanner that finds nothing passes everything. These floors sit well below the
    // 2026-10-19 counts (64 ALTER-added columns, 90 indexes, 310 statements), so they
    // only trip if the scanner breaks, not when the schema shrinks a little.
    assert!(
        report.alters >= 40,
        "only {} ALTER TABLE ... ADD COLUMN found in Storage::open; the scanner is probably broken rather than the schema being fine",
        report.alters
    );
    assert!(
        report.indexes >= 50,
        "only {} CREATE INDEX statements found in Storage::open; the scanner is probably broken",
        report.indexes
    );
    assert!(
        report.statements >= 150,
        "only {} SQL statements found in Storage::open; the scanner is probably broken",
        report.statements
    );
    assert!(
        report.checked >= 20,
        "only {} statements name an ALTER-added column (39 on 2026-10-19: the post-ALTER indexes, the FTS \
         triggers' new.channel_id, the role backfills); the matcher is probably broken",
        report.checked
    );
    assert!(report.violations.is_empty(), "{}", failure_message(SCHEMA_FILE, &report.violations));
}

// ---------------------------------------------------------------------------------------
// Fixtures: the past mistakes, re-run on every pass so the scanner proves it still fails.
// ---------------------------------------------------------------------------------------

fn fixture(body: &str) -> Result<Report, String> {
    let src = format!("impl Storage {{\n    pub fn open(path: &Path) -> Result<Self, E> {{\n{body}\n    }}\n}}\n");
    check(&src, "fixture.rs")
}

/// The v0.675.0 shape: the index over `shared` in the first batch, its ALTER in a loop below.
const V0675_BROKEN: &str = r#"
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_uploads (
                id INTEGER PRIMARY KEY, -- a comment; with a semicolon
                shared INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_user_uploads_shared
                ON user_uploads(shared, id);"
        )?;
        for alter in &[
            "ALTER TABLE user_uploads ADD COLUMN shared INTEGER NOT NULL DEFAULT 0",
        ] {
            let _ = conn.execute(alter, []);
        }
"#;

/// The v0.675.1 fix: the same index after the ALTER.
const V0675_FIXED: &str = r#"
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_uploads (
                id INTEGER PRIMARY KEY,
                shared INTEGER NOT NULL DEFAULT 0
            );"
        )?;
        for alter in &[
            "ALTER TABLE user_uploads ADD COLUMN shared INTEGER NOT NULL DEFAULT 0",
        ] {
            let _ = conn.execute(alter, []);
        }
        // An index named in a comment must not count: CREATE INDEX x ON user_uploads(shared);
        let label = '"'; // a char literal holding a quote must not open a string
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_user_uploads_shared ON user_uploads(shared, id);")?;
"#;

#[test]
fn fixture_v0675_index_before_its_alter_fails() {
    let r = fixture(V0675_BROKEN).expect("fixture reads");
    assert_eq!(r.violations.len(), 1, "the v0.675.0 shape must fail: {:?}", r.violations);
    assert!(r.violations[0].contains("CREATE INDEX names user_uploads.shared"), "{:?}", r.violations);
}

#[test]
fn fixture_v0675_fix_passes() {
    let r = fixture(V0675_FIXED).expect("fixture reads");
    assert!(r.violations.is_empty(), "the v0.675.1 fix must pass: {:?}", r.violations);
    assert_eq!(r.alters, 1);
    assert_eq!(r.indexes, 1, "the commented-out index must not be counted");
}

#[test]
fn fixture_v0262_seed_insert_before_its_alter_fails() {
    // v0.262.2 (2026-05-17): the roles seed named a column whose ALTER ran after it.
    let r = fixture(
        r#"
        conn.execute_batch("CREATE TABLE IF NOT EXISTS roles (id TEXT PRIMARY KEY, max_chars INTEGER NOT NULL DEFAULT 280);")?;
        conn.execute("INSERT OR IGNORE INTO roles (id, max_chars) VALUES (?1, ?2)", params)?;
        if conn.prepare("SELECT max_chars FROM roles LIMIT 0").is_err() {
            conn.execute_batch("ALTER TABLE roles ADD COLUMN max_chars INTEGER NOT NULL DEFAULT 280;")?;
        }
        conn.execute_batch("BEGIN; ALTER TABLE roles ADD COLUMN ship INTEGER; UPDATE roles SET ship = 1 WHERE id = 'admin'; COMMIT;")?;
"#,
    )
    .expect("fixture reads");
    assert_eq!(r.violations.len(), 1, "the seed INSERT must fail, the UPDATE after its ALTER must not: {:?}", r.violations);
    assert!(r.violations[0].contains("INSERT names roles.max_chars"), "{:?}", r.violations);
}

#[test]
fn fixture_trigger_view_update_delete_before_their_alter_fail() {
    let r = fixture(
        r#"
        conn.execute_batch("
            CREATE TRIGGER IF NOT EXISTS m_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, channel_id) VALUES (new.id, COALESCE(new.channel_id, 'general'));
                SELECT CASE WHEN 1 THEN 1 END;
            END;
            CREATE VIEW IF NOT EXISTS v AS SELECT reply_to FROM messages;
            UPDATE messages SET topic = 'x' WHERE id = (SELECT 1 FROM other WHERE topic = 2);
            DELETE FROM messages WHERE flagged = 1;
        ")?;
        conn.execute_batch("ALTER TABLE messages ADD COLUMN channel_id TEXT;
                            ALTER TABLE messages ADD COLUMN reply_to TEXT;
                            ALTER TABLE messages ADD COLUMN topic TEXT;
                            ALTER TABLE messages ADD COLUMN flagged INTEGER;")?;
"#,
    )
    .expect("fixture reads");
    let all = r.violations.join("\n");
    assert!(all.contains("CREATE TRIGGER (new./old.) names messages.channel_id"), "{all}");
    assert!(all.contains("CREATE VIEW names messages.reply_to"), "{all}");
    assert!(all.contains("UPDATE names messages.topic"), "{all}");
    assert!(all.contains("DELETE names messages.flagged"), "{all}");
    assert_eq!(r.violations.len(), 4, "the sub-query's `topic` belongs to `other` and must not count: {all}");
}

#[test]
fn fixture_an_alter_built_with_format_fails_loudly() {
    let r = fixture(
        r#"
        let add = format!("ALTER TABLE {table} ADD COLUMN {col} TEXT");
"#,
    );
    match r {
        Err(msg) => assert!(msg.contains("cannot read"), "{msg}"),
        Ok(r) => panic!("an unreadable ALTER must fail, not be skipped (found {} alters)", r.alters),
    }
}
