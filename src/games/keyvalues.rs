//! Valve's KeyValues text format, the shape of Steam's own install records
//! (`steamapps\libraryfolders.vdf` and each `steamapps\appmanifest_<appid>.acf`). Valve's
//! Steamworks documentation names the appmanifest "(KeyValues)"; the format itself is a list of
//! names, each followed by either a value or a `{ ... }` block of more of the same:
//!
//! ```text
//! "AppState"
//! {
//!     "appid"      "620"
//!     "name"       "Portal 2"
//!     "StateFlags" "4"
//! }
//! ```
//!
//! Read only, never written: nothing in HumanityOS changes a store's files. What is handled is what
//! Steam writes: quoted strings with the escapes `\\`, `\"`, `\n` and `\t` (an unknown escape keeps
//! its backslash, so an unescaped Windows path still reads right), unquoted words, `//` comments to
//! the end of the line, and the `[$WIN32]`-style conditions some Valve files put after a value,
//! which are skipped. Names are not case-sensitive (`find`): Steam has written both
//! "LibraryFolders" and "libraryfolders".

/// A value: text, or a block of named values in the order the file gives them.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Block(Vec<(String, Value)>),
}

impl Value {
    /// The text, when this is text.
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            Value::Block(_) => None,
        }
    }

    /// The block's entries, when this is a block.
    pub fn block(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Block(b) => Some(b),
            Value::Text(_) => None,
        }
    }
}

/// The first entry named `key`, whatever its letters' case.
pub fn find<'a>(entries: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    entries.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// The text of the first entry named `key`, when it is text.
pub fn find_text<'a>(entries: &'a [(String, Value)], key: &str) -> Option<&'a str> {
    find(entries, key).and_then(Value::text)
}

/// How deep blocks may nest. Steam's files nest three or four deep; a file nested far deeper is
/// not one of them, and reading it must not run the program out of stack.
const MAX_DEPTH: usize = 64;

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

/// Split `text` into strings and braces.
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.trim_start_matches('\u{feff}').chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '{' {
            out.push(Token::Open);
            i += 1;
        } else if c == '}' {
            out.push(Token::Close);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            // A comment, to the end of the line.
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '[' {
            // A condition such as [$WIN32] after a value: skipped.
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            i += 1;
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return Err("a quoted string is not closed".to_string()),
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => {
                        match chars.get(i + 1) {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('\\') => s.push('\\'),
                            Some('"') => s.push('"'),
                            Some(other) => {
                                s.push('\\');
                                s.push(*other);
                            }
                            None => return Err("a quoted string is not closed".to_string()),
                        }
                        i += 2;
                    }
                    Some(other) => {
                        s.push(*other);
                        i += 1;
                    }
                }
            }
            out.push(Token::Str(s));
        } else {
            // An unquoted word, up to whitespace, a brace or a quote.
            let mut s = String::new();
            while i < chars.len() && !chars[i].is_whitespace() && !matches!(chars[i], '{' | '}' | '"') {
                s.push(chars[i]);
                i += 1;
            }
            out.push(Token::Str(s));
        }
    }
    Ok(out)
}

/// Read a whole KeyValues text: its top-level entries, in order.
pub fn parse(text: &str) -> Result<Vec<(String, Value)>, String> {
    let toks = tokens(text)?;
    let mut i = 0;
    block(&toks, &mut i, 0)
}

/// The entries from `t[*i]` to the `}` that closes this block (or the end, at the top level).
fn block(t: &[Token], i: &mut usize, depth: usize) -> Result<Vec<(String, Value)>, String> {
    if depth > MAX_DEPTH {
        return Err(format!("blocks nest more than {MAX_DEPTH} deep"));
    }
    let mut out = Vec::new();
    loop {
        match t.get(*i) {
            None if depth == 0 => return Ok(out),
            None => return Err("a { is not closed".to_string()),
            Some(Token::Close) if depth == 0 => return Err("a } has no { before it".to_string()),
            Some(Token::Close) => {
                *i += 1;
                return Ok(out);
            }
            Some(Token::Open) => return Err("a { has no name before it".to_string()),
            Some(Token::Str(key)) => {
                *i += 1;
                match t.get(*i) {
                    Some(Token::Str(v)) => {
                        out.push((key.clone(), Value::Text(v.clone())));
                        *i += 1;
                    }
                    Some(Token::Open) => {
                        *i += 1;
                        let inner = block(t, i, depth + 1)?;
                        out.push((key.clone(), Value::Block(inner)));
                    }
                    _ => return Err(format!("\"{key}\" has no value")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QUOTED STRINGS, NESTING AND ESCAPES, as Steam writes them: a block inside a block, a Windows
    /// path written with doubled backslashes, an escaped quote, a tab and a newline, a comment, an
    /// unquoted word, a [$WIN32] condition, and names read whatever their case.
    ///
    /// Seen red 2026-10-18 with the `\\` escape arm taken out (it fell through to the unknown-escape
    /// arm): "a doubled backslash is one backslash", left: Some("C:\\\\Program Files (x86)\\\\Steam")
    /// right: Some("C:\\Program Files (x86)\\Steam").
    #[test]
    fn quoted_strings_nesting_and_escapes() {
        let text = "\u{feff}// written by Steam\n\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"say \\\"hi\\\"\\tthen\\nmore\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"620\"\t\t\"123\"\n\t\t}\n\t}\n\tunquoted word [$WIN32]\n\t\"raw\" \"D:\\Games\\Steam\"\n}\n";
        let kv = parse(text).expect("it reads");
        let folders = find(&kv, "LibraryFolders").and_then(Value::block).expect("names are not case-sensitive");
        let zero = find(folders, "0").and_then(Value::block).expect("a block inside a block");
        assert_eq!(find_text(zero, "path"), Some("C:\\Program Files (x86)\\Steam"), "a doubled backslash is one backslash");
        assert_eq!(find_text(zero, "label"), Some("say \"hi\"\tthen\nmore"), "an escaped quote, a tab and a newline");
        let apps = find(zero, "apps").and_then(Value::block).expect("three deep");
        assert_eq!(find_text(apps, "620"), Some("123"));
        assert_eq!(find_text(folders, "unquoted"), Some("word"), "an unquoted word, the condition after it skipped");
        assert_eq!(find_text(folders, "raw"), Some("D:\\Games\\Steam"), "an unknown escape keeps its backslash");
    }

    /// A DAMAGED FILE IS REFUSED, NOT GUESSED AT: an unclosed quote, an unclosed block, a stray `}`
    /// and a name with no value each give an error (the reader then skips that file).
    ///
    /// Seen red 2026-10-18 with the nested end-of-text arm returning what it had: "an unclosed block
    /// is refused" got Ok([("AppState", Block([("appid", Text("620"))]))]).
    #[test]
    fn a_damaged_file_is_refused() {
        assert!(parse("\"AppState\" { \"name\" \"Portal").is_err(), "an unclosed quote is refused");
        assert!(parse("\"AppState\" { \"appid\" \"620\"").is_err(), "an unclosed block is refused: {:?}", parse("\"AppState\" { \"appid\" \"620\""));
        assert!(parse("\"a\" \"b\" }").is_err(), "a stray }} is refused");
        assert!(parse("\"a\" { \"b\" }").is_err(), "a name with no value is refused");
        let deep = "\"a\" {".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep).is_err(), "nesting past the limit is refused, not followed");
    }
}
