//! Valve's BINARY KeyValues, the shape of Steam's local achievement records (2026-10-19):
//! `appcache\stats\UserGameStatsSchema_<appid>.bin` (a game's achievements: their names and
//! descriptions) and `appcache\stats\UserGameStats_<account id>_<appid>.bin` (which of them this
//! person has unlocked, and when). The same tree as the text format (keyvalues.rs), written as
//! bytes: each entry is one type byte, its name as a zero-ended string, then its value:
//!
//! ```text
//! 00 name 00   a block: entries follow, up to an 08 byte
//! 01 name 00   text, zero-ended (UTF-8)
//! 02 name 00   a 32-bit whole number, little-endian
//! 03 name 00   a 32-bit decimal number
//! 04 name 00   a 32-bit pointer (read as a number)
//! 05 name 00   text in UTF-16, ended by two zero bytes
//! 06 name 00   a colour, four bytes (read as a number)
//! 07 name 00   a 64-bit whole number (unsigned)
//! 0A name 00   a 64-bit whole number (signed)
//! 08 or 0B     the end of the block
//! ```
//!
//! Read into keyvalues.rs's own `Value`, numbers written as their decimal text, so the readers
//! look things up with the same `find` and `find_text` as the text files. Read only, never written.
//!
//! A FILE THAT DOES NOT READ EXACTLY IS REFUSED, not guessed at: a type byte not in the table, a
//! string or number cut off by the end of the file, a block (the whole file's included) not
//! closed, nesting past the limit or bytes after the end. The achievement reader then shows nothing for that game, never a wrong
//! count. On 2026-10-19 every one of the 102 such files on the operator's computer read exactly
//! (types 00, 01, 02 and 08 were all they used; the others are read because Valve's format has
//! them).

use super::keyvalues::Value;

/// How deep blocks may nest. Steam's stats files nest five or six deep.
const MAX_DEPTH: usize = 64;

const BLOCK: u8 = 0x00;
const STRING: u8 = 0x01;
const INT32: u8 = 0x02;
const FLOAT32: u8 = 0x03;
const POINTER: u8 = 0x04;
const WIDE_STRING: u8 = 0x05;
const COLOR: u8 = 0x06;
const UINT64: u8 = 0x07;
const END: u8 = 0x08;
const INT64: u8 = 0x0A;
const END_ALT: u8 = 0x0B;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(b)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.bytes.len()).ok_or("a value is cut off by the end of the file")?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    /// A zero-ended string (its bytes read as UTF-8, any that are not replaced).
    fn cstring(&mut self) -> Result<String, String> {
        let rest = &self.bytes[self.at..];
        let len = rest.iter().position(|b| *b == 0).ok_or("a string is not ended")?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.at += len + 1;
        Ok(s)
    }

    /// UTF-16 (little-endian) up to two zero bytes.
    fn wide_string(&mut self) -> Result<String, String> {
        let mut units = Vec::new();
        loop {
            let pair = self.take(2)?;
            let unit = u16::from_le_bytes([pair[0], pair[1]]);
            if unit == 0 {
                return Ok(String::from_utf16_lossy(&units));
            }
            units.push(unit);
        }
    }

    fn four(&mut self) -> Result<[u8; 4], String> {
        let b = self.take(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    fn eight(&mut self) -> Result<[u8; 8], String> {
        let b = self.take(8)?;
        Ok([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    }

    /// The entries up to this block's end byte. The top level has one too (every file Steam
    /// writes ends with it), so a file cut off between two entries is refused like any other.
    fn block(&mut self, depth: usize) -> Result<Vec<(String, Value)>, String> {
        if depth > MAX_DEPTH {
            return Err(format!("blocks nest more than {MAX_DEPTH} deep"));
        }
        let mut out = Vec::new();
        loop {
            let Some(kind) = self.byte() else {
                return Err("a block is not closed".to_string());
            };
            if kind == END || kind == END_ALT {
                return Ok(out);
            }
            let name = self.cstring()?;
            let value = match kind {
                BLOCK => Value::Block(self.block(depth + 1)?),
                STRING => Value::Text(self.cstring()?),
                WIDE_STRING => Value::Text(self.wide_string()?),
                INT32 => Value::Text(i32::from_le_bytes(self.four()?).to_string()),
                FLOAT32 => Value::Text(f32::from_le_bytes(self.four()?).to_string()),
                POINTER | COLOR => Value::Text(u32::from_le_bytes(self.four()?).to_string()),
                UINT64 => Value::Text(u64::from_le_bytes(self.eight()?).to_string()),
                INT64 => Value::Text(i64::from_le_bytes(self.eight()?).to_string()),
                other => return Err(format!("type byte {other:#04x} is not one this reader knows")),
            };
            out.push((name, value));
        }
    }
}

/// Read a whole binary KeyValues file: its top-level entries, in order.
pub fn parse(bytes: &[u8]) -> Result<Vec<(String, Value)>, String> {
    let mut r = Reader { bytes, at: 0 };
    let out = r.block(0)?;
    if r.at != bytes.len() {
        return Err(format!("{} bytes follow the end", bytes.len() - r.at));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::fixtures::{bkv, B};
    use crate::games::keyvalues::{find, find_text};

    /// EVERY TYPE STEAM'S FORMAT HAS, NESTED, READ BACK AS TEXT: a block in a block, text (UTF-8,
    /// with a letter outside ASCII), a negative 32-bit number (a bitfield with its top bit set), a
    /// decimal, a 64-bit number, UTF-16 text and both end bytes.
    ///
    /// Seen red 2026-10-19 with INT32 read as unsigned (`u32::from_le_bytes`): "a 32-bit number is
    /// signed, as Valve writes it", left: Some("2147483649"), right: Some("-2147483647").
    #[test]
    fn every_type_nested_and_read_back() {
        let mut bytes = bkv(&[(
            "275850",
            B::Block(vec![
                ("gamename", B::Str("No Man\u{2019}s Sky")),
                ("stats", B::Block(vec![("2", B::Block(vec![("data", B::Int(i32::MIN + 1))]))])),
                ("version", B::Int(5)),
            ]),
        )]);
        // Splice in the rarer types by hand, before the root block's end byte: a decimal, a 64-bit
        // number, UTF-16 text, and a block closed by the other end byte (0x0B).
        let end = bytes.len() - 2;
        let mut extra = vec![FLOAT32];
        extra.extend_from_slice(b"ratio\0");
        extra.extend_from_slice(&0.5f32.to_le_bytes());
        extra.push(UINT64);
        extra.extend_from_slice(b"big\0");
        extra.extend_from_slice(&76561198000000001u64.to_le_bytes());
        extra.push(WIDE_STRING);
        extra.extend_from_slice(b"wide\0");
        for u in "Hi".encode_utf16() {
            extra.extend_from_slice(&u.to_le_bytes());
        }
        extra.extend_from_slice(&[0, 0]);
        extra.push(BLOCK);
        extra.extend_from_slice(b"alt\0");
        extra.push(END_ALT);
        bytes.splice(end..end, extra);

        let kv = parse(&bytes).expect("it reads");
        let root = find(&kv, "275850").and_then(Value::block).expect("the root block");
        assert_eq!(find_text(root, "gamename"), Some("No Man\u{2019}s Sky"), "UTF-8 text");
        let stat = find(root, "stats").and_then(Value::block).and_then(|s| find(s, "2")).and_then(Value::block).expect("three deep");
        assert_eq!(find_text(stat, "data"), Some("-2147483647"), "a 32-bit number is signed, as Valve writes it");
        assert_eq!(find_text(root, "version"), Some("5"));
        assert_eq!(find_text(root, "ratio"), Some("0.5"), "a decimal");
        assert_eq!(find_text(root, "big"), Some("76561198000000001"), "a 64-bit number");
        assert_eq!(find_text(root, "wide"), Some("Hi"), "UTF-16 text");
        assert_eq!(find(root, "alt"), Some(&Value::Block(Vec::new())), "a block closed by 0x0B");
    }

    /// A DAMAGED FILE IS REFUSED: cut short inside a number, a string or a block; an unknown type
    /// byte; bytes after the end; nesting past the limit.
    ///
    /// Seen red 2026-10-19 with the trailing-bytes check left out of `parse`: "bytes after the end
    /// are refused" got Ok([("a", Text("b"))]).
    #[test]
    fn a_damaged_file_is_refused() {
        let good = bkv(&[("cache", B::Block(vec![("crc", B::Int(7)), ("note", B::Str("x"))]))]);
        assert!(parse(&good).is_ok());
        for cut in [good.len() - 1, good.len() - 4, 9, 3] {
            assert!(parse(&good[..cut]).is_err(), "cut at {cut} is refused: {:?}", parse(&good[..cut]));
        }
        let mut unknown = good.clone();
        unknown[0] = 0x09;
        assert!(parse(&unknown).is_err(), "an unknown type byte is refused");
        let mut trailing = bkv(&[("a", B::Str("b"))]);
        trailing.extend_from_slice(&[1, 2, 3]);
        assert!(parse(&trailing).is_err(), "bytes after the end are refused: {:?}", parse(&trailing));
        let mut deep = Vec::new();
        for _ in 0..MAX_DEPTH + 2 {
            deep.extend_from_slice(&[BLOCK, b'a', 0]);
        }
        assert!(parse(&deep).is_err(), "nesting past the limit is refused, not followed");
    }
}
