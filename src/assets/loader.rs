//! Asset loader — type identification and data format parsing.
//!
//! Supports CSV, TOML, RON, and raw byte loading.
//! All parse functions are pure (no I/O) for cross-platform use.

use serde::de::DeserializeOwned;
use std::cell::Cell;

/// Supported asset types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetType {
    Mesh,    // .glb, .gltf
    Texture, // .png, .jpg, .ktx2
    Shader,  // .wgsl
    Audio,   // .ogg, .wav
    Data,    // .ron, .csv, .toml, .json
}

/// Identify asset type from a file extension string.
pub fn asset_type_from_ext(ext: &str) -> AssetType {
    match ext {
        "glb" | "gltf" => AssetType::Mesh,
        "png" | "jpg" | "ktx2" => AssetType::Texture,
        "wgsl" => AssetType::Shader,
        "ogg" | "wav" => AssetType::Audio,
        _ => AssetType::Data,
    }
}

/// Load raw asset bytes from disk and identify type (native only).
#[cfg(feature = "native")]
pub fn load_asset_bytes(path: &std::path::Path) -> Result<(AssetType, Vec<u8>), std::io::Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let asset_type = asset_type_from_ext(ext);
    let bytes = std::fs::read(path)?;
    Ok((asset_type, bytes))
}

thread_local! {
    /// Whether `parse_csv` refuses a file that has a row it cannot read, instead of
    /// skipping the row: true only inside `refusing_rows`, on this thread (BUG-163).
    static REFUSE_ROWS: Cell<bool> = const { Cell::new(false) };
}

/// Run `f` with every `parse_csv` in it REFUSING a file that has a row it cannot
/// read: the parse returns an error naming the row (and how many there were)
/// instead of skipping it.
///
/// `embedded_data::load_data_or_embedded` builds a data folder's copy of a file
/// this way, so a file this version cannot read in full is never half used: the
/// copy built into the exe is used instead, and the log says so once (BUG-163).
/// Anything else keeps `parse_csv`'s row-by-row leniency.
///
/// Scoped to this thread and to `f`: the setting before it comes back when `f`
/// returns or unwinds.
pub fn refusing_rows<R>(f: impl FnOnce() -> R) -> R {
    struct PutBack(bool);
    impl Drop for PutBack {
        fn drop(&mut self) {
            REFUSE_ROWS.with(|r| r.set(self.0));
        }
    }
    let _put_back = PutBack(REFUSE_ROWS.with(|r| r.replace(true)));
    f()
}

/// Parse a CSV file into a Vec of deserialized records.
/// Skips comment lines (starting with #). Flexible: handles headers automatically.
///
/// A row the record type cannot read is skipped, with a warning naming its line in
/// the file; inside `refusing_rows` the whole parse is refused instead.
pub fn parse_csv<T: DeserializeOwned>(data: &[u8]) -> Result<Vec<T>, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    // A byte-order mark, which some editors (older Notepad among them) put at the
    // start of a file they save, is not part of the first line: left in, it hides
    // a first comment line or the header, and now that one row the code cannot
    // read sets the whole file aside (BUG-163), a modder's file would be refused
    // for a character nobody can see.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // Comment lines go, and blank ones (never rows: the reader skipped them anyway,
    // but a record read after one is positioned at the blank line). Each line kept
    // remembers its line in the file, so a row the code cannot read is named by
    // the line a person finds it on.
    let mut file_line = Vec::new();
    let filtered: String = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .map(|(i, line)| {
            file_line.push(i + 1);
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(filtered.as_bytes());
    let headers = reader.headers().map_err(|e| format!("the header cannot be read: {e}"))?.clone();

    // ROW-RESILIENT: a row that fails serde is skipped, not fatal. That resilience once
    // silently ate data (plants.csv `saffron` failed a too-narrow u32 field for months),
    // so every skip is logged per-row AND summarized with a count. Registries that must
    // be complete should assert count == source-row count (see farming's zero-drop test).
    // A data folder's file is parsed inside `refusing_rows` (BUG-163), where one such
    // row refuses the whole file and the built-in copy is used instead.
    let refuse = REFUSE_ROWS.with(|r| r.get());
    let mut records = Vec::new();
    let mut skipped = 0usize;
    let mut first: Option<String> = None;
    for row in reader.records() {
        let problem = match row {
            Ok(row) => match row.deserialize::<T>(Some(&headers)) {
                Ok(record) => {
                    records.push(record);
                    continue;
                }
                Err(e) => row_problem(&e, Some(&row), &headers, &file_line),
            },
            Err(e) => row_problem(&e, None, &headers, &file_line),
        };
        skipped += 1;
        if refuse {
            first.get_or_insert(problem);
        } else {
            log::warn!("CSV parse warning (skipping row): {problem}");
        }
    }
    if skipped > 0 {
        let total = records.len() + skipped;
        if let Some(first) = first {
            return Err(if skipped == 1 {
                first
            } else {
                format!("{skipped} of its {total} rows, the first at {first}")
            });
        }
        log::warn!(
            "CSV parse: skipped {skipped} malformed row(s) of {total} total -- data rows are \
             being silently dropped; check the schema/struct fields against the file"
        );
    }
    Ok(records)
}

/// Where a CSV row is and why it cannot be read, as a person looking at the file
/// would want it: "line 31 (well_fed), column duration_s: invalid float literal".
fn row_problem(
    e: &csv::Error,
    row: Option<&csv::StringRecord>,
    headers: &csv::StringRecord,
    file_line: &[usize],
) -> String {
    let line = e
        .position()
        .or_else(|| row.and_then(|r| r.position()))
        .and_then(|p| file_line.get((p.line() as usize).wrapping_sub(1)).copied());
    let mut place = line.map_or_else(|| "a row".to_string(), |l| format!("line {l}"));
    if let Some(id) = row.and_then(|r| r.get(0)).filter(|id| !id.is_empty()) {
        place.push_str(&format!(" ({id})"));
    }
    let why = match e.kind() {
        csv::ErrorKind::Deserialize { err, .. } => {
            if let Some(column) = err.field().and_then(|f| headers.get(f as usize)) {
                place.push_str(&format!(", column {column}"));
            }
            err.kind().to_string()
        }
        _ => e.to_string(),
    };
    format!("{place}: {why}")
}

/// Parse a TOML string into a deserialized struct.
pub fn parse_toml<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    toml::from_str(text).map_err(|e| format!("TOML parse error: {e}"))
}

/// Parse a RON string into a deserialized struct.
pub fn parse_ron<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 error: {e}"))?;
    ron::from_str(text).map_err(|e| format!("RON parse error: {e}"))
}

/// Parse a JSON string into a deserialized struct.
pub fn parse_json<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| format!("JSON parse error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row {
        id: String,
        #[serde(default)]
        n: f32,
    }

    const FILE: &str = "# a comment\n# another\nid,n\nfirst,1\n\nsecond,two\nthird,3\n";

    /// Outside `refusing_rows` a row that cannot be read is skipped and the rest load,
    /// as they always have; blank lines and comments are not rows.
    #[test]
    fn a_row_that_cannot_be_read_is_skipped_by_default() {
        let rows: Vec<Row> = parse_csv(FILE.as_bytes()).expect("parses");
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["first", "third"]);
        assert_eq!(rows[1].n, 3.0);
    }

    /// Inside `refusing_rows` the same file is refused, naming the row by its line in the
    /// file (comments counted), its id and its column; and the leniency comes back after.
    #[test]
    fn inside_refusing_rows_one_bad_row_refuses_the_file_and_names_it() {
        let e = refusing_rows(|| parse_csv::<Row>(FILE.as_bytes())).expect_err("refused");
        assert!(e.starts_with("line 6 (second), column n: "), "{e}");
        assert!(parse_csv::<Row>(FILE.as_bytes()).is_ok(), "lenient again outside the scope");
    }

    /// A column the record does not declare refuses every row, and the message counts them.
    #[test]
    fn an_unknown_column_refuses_every_row_and_says_how_many() {
        let file = "id,n,dispel_type\na,1,none\nb,2,medicine\n";
        let e = refusing_rows(|| parse_csv::<Row>(file.as_bytes())).expect_err("refused");
        assert!(e.starts_with("2 of its 2 rows, the first at line 2 (a)"), "{e}");
        assert!(e.contains("dispel_type"), "{e}");
    }

    /// A file saved with a byte-order mark reads like one without.
    #[test]
    fn a_byte_order_mark_is_not_part_of_the_first_line() {
        let with_mark = format!("\u{feff}{FILE}");
        let rows = refusing_rows(|| parse_csv::<Row>("\u{feff}# a comment\nid,n\nfirst,1\n".as_bytes())).expect("reads");
        assert_eq!(rows.len(), 1);
        let lenient: Vec<Row> = parse_csv(with_mark.as_bytes()).expect("parses");
        assert_eq!(lenient.len(), 2);
    }

    /// The setting is put back even when the code inside panics.
    #[test]
    fn refusing_rows_is_undone_by_a_panic_inside_it() {
        let caught = std::panic::catch_unwind(|| refusing_rows(|| -> () { panic!("a test's own panic, caught") }));
        assert!(caught.is_err());
        assert!(parse_csv::<Row>(FILE.as_bytes()).is_ok(), "lenient again after the panic");
    }
}
