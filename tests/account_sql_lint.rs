//! Every table named by account export/erase must actually exist.
//!
//! Why this test exists, in one paragraph, because the failure it catches is
//! invisible by construction:
//!
//! `src/relay/storage/account.rs` implements "download everything this server
//! holds about you" and "erase it all". Its `grab()` helper swallows a failed
//! `conn.prepare` into an empty array (`.ok()?`), and its `del()` helper used to
//! swallow a failed `execute` into a log line. So a query against a table that
//! DOES NOT EXIST produced exactly the same user-visible result as a query that
//! found nothing: `"uploads": []` in the export, and no mention at all in the
//! erase receipt. Two such queries lived in that file for months. `uploads` and
//! `tasks` are not tables in this schema; the real names are `user_uploads` and
//! `project_tasks`. The consequence was that every file a user had ever uploaded
//! survived "erase everything", on disk and in the database, while the app told
//! them their data was gone.
//!
//! No unit test could catch it, because the code did not error. Nothing was
//! wrong except the words. So the check has to be a lint over the source text.
//!
//! Deliberately NOT checked here: column names, whether the WHERE clause uses
//! the right key column, or whether the coverage is complete. Those need real
//! judgment. This test answers only the one question that has a mechanical
//! answer and that went wrong anyway: does the table exist?

use std::collections::HashSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Table names declared by the schema, from every `CREATE TABLE IF NOT EXISTS`.
fn schema_tables() -> HashSet<String> {
    let mut out = HashSet::new();
    let schema = std::fs::read_to_string(repo_root().join("src/relay/storage/mod.rs"))
        .expect("read src/relay/storage/mod.rs");
    let needle = "CREATE TABLE IF NOT EXISTS ";
    let mut rest = schema.as_str();
    while let Some(i) = rest.find(needle) {
        rest = &rest[i + needle.len()..];
        let name: String = rest
            .chars()
            .skip_while(|c| *c == '"')
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            out.insert(name);
        }
    }
    assert!(
        out.len() > 50,
        "only found {} tables in the schema, the scanner is probably broken rather than the code being fine",
        out.len()
    );
    out
}

/// Table names referenced after FROM / DELETE FROM / UPDATE / INSERT INTO in a source file.
fn referenced_tables(src: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (lineno, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        // Skip comment lines: this file explains the bug in prose that names
        // the phantom tables, and the lint must not trip over its own footnote.
        if trimmed.starts_with("//") {
            continue;
        }
        for kw in ["DELETE FROM ", "INSERT INTO ", "FROM ", "UPDATE "] {
            let mut rest = line;
            while let Some(i) = rest.find(kw) {
                rest = &rest[i + kw.len()..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                // "FROM (" subqueries yield an empty name. `ON CONFLICT DO
                // UPDATE SET ...` yields "SET", which is a keyword and not a
                // table; the same goes for the other words that can legally
                // follow one of these keywords.
                const NOT_TABLES: [&str; 6] = ["SET", "SELECT", "VALUES", "OR", "INTO", "WHERE"];
                if !name.is_empty() && !NOT_TABLES.contains(&name.as_str()) {
                    out.push((name, lineno + 1));
                }
            }
        }
    }
    out
}

#[test]
fn account_export_and_erase_only_name_tables_that_exist() {
    let tables = schema_tables();
    let path = repo_root().join("src/relay/storage/account.rs");
    let src = std::fs::read_to_string(&path).expect("read account.rs");

    let refs = referenced_tables(&src);
    assert!(
        refs.len() > 10,
        "found only {} table references in account.rs, the scanner is probably broken",
        refs.len()
    );

    let mut phantom: Vec<String> = refs
        .iter()
        .filter(|(name, _)| !tables.contains(name))
        .map(|(name, line)| format!("  account.rs:{line}  ->  no table named `{name}`"))
        .collect();
    phantom.sort();
    phantom.dedup();

    assert!(
        phantom.is_empty(),
        "\n\n[FAIL] Account export/erase names {} table(s) that do not exist in the schema.\n\n{}\n\n\
         This is not a harmless typo. `grab()` turns a missing table into an empty\n\
         array and the export then tells the user they have no such data, and a\n\
         failed `del()` is reported as a row count of zero. So the feature keeps\n\
         claiming success while silently doing nothing, which is exactly the false\n\
         privacy promise the Accord treats as a harm.\n\n\
         Fix the table name against src/relay/storage/mod.rs. Do not silence this test.\n",
        phantom.len(),
        phantom.join("\n"),
    );
}

/// Anything erased must first have been offered for download.
///
/// `export_account` and `delete_account` are two hand-maintained lists in the
/// same file, and they drifted: six tables were being DELETED with no matching
/// `grab()`, so the server erased data it had never let the user see. That is
/// backwards from the promise the Settings page makes, and it is the kind of
/// gap that only ever grows, because nothing connects the two lists.
///
/// This is the invariant, and it only runs one way on purpose: export may
/// legitimately be WIDER than delete (a ban is shown to you and never deleted,
/// because a record that exists to constrain someone must not be erasable by
/// that someone). Delete being wider than export is always a bug.
#[test]
fn everything_deleted_is_also_exported() {
    let path = repo_root().join("src/relay/storage/account.rs");
    let src = std::fs::read_to_string(&path).expect("read account.rs");

    // Split at delete_account so each half is scanned separately.
    let split = src
        .find("pub fn delete_account")
        .expect("delete_account not found; did the file get restructured?");
    let (export_half, delete_half) = src.split_at(split);

    let exported: HashSet<String> = referenced_tables(export_half)
        .into_iter()
        .map(|(t, _)| t)
        .collect();

    let mut missing: Vec<String> = referenced_tables(delete_half)
        .into_iter()
        .map(|(t, _)| t)
        .filter(|t| !exported.contains(t))
        .collect();
    missing.sort();
    missing.dedup();

    assert!(
        missing.is_empty(),
        "\n\n[FAIL] delete_account erases {} table(s) that export_account never offers:\n\n{}\n\n\
         A user is told they can download everything before erasing it. Deleting a\n\
         table the export omits breaks that in the one direction that cannot be\n\
         undone. Add a grab() for each, then delete it.\n\n\
         (Export MAY be wider than delete. Sanction records are exported and never\n\
         deleted on purpose. Only the reverse is a bug.)\n",
        missing.len(),
        missing
            .iter()
            .map(|t| format!("  {t}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// The same check for the other file that deletes user data in bulk. A wrong
/// table name there fails loudly rather than silently, but it is the same class
/// of mistake and the scan is free.
#[test]
fn channel_bulk_wipes_only_name_tables_that_exist() {
    let tables = schema_tables();
    let path = repo_root().join("src/relay/storage/channels.rs");
    let Ok(src) = std::fs::read_to_string(&path) else {
        return; // file moved; the account test above is the load-bearing one
    };

    let phantom: Vec<String> = referenced_tables(&src)
        .iter()
        .filter(|(name, _)| !tables.contains(name))
        // sqlite_master and pragma pseudo-tables are legitimate.
        .filter(|(name, _)| !name.starts_with("sqlite_") && !name.starts_with("pragma_"))
        .map(|(name, line)| format!("  channels.rs:{line}  ->  no table named `{name}`"))
        .collect();

    assert!(
        phantom.is_empty(),
        "\n\n[FAIL] channels.rs names {} table(s) that do not exist:\n\n{}\n",
        phantom.len(),
        phantom.join("\n"),
    );
}
