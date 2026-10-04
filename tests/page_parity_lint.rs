//! Dual-UI data-parity lint (2026-07-30).
//!
//! `page_registry_lint` proves a page EXISTS on both sides and is listed in
//! docs/PAGES.md. It does not prove the two sides show the SAME THING, and that
//! gap is exactly how Library and Platform drifted: both were marked "both" in
//! the registry for months while the native and web versions read entirely
//! different data files. Library shipped 53 documents in the app and 17 on the
//! web; the external-resources list was two unrelated curations that shared 17
//! URLs out of 81. Nothing failed, because nothing was checking.
//!
//! This closes it. For each page in `PARITY_PAIRS`, both the native source and
//! the web source must reference the same data file. When you repoint one side
//! at a new data file, this test fails until you repoint the other.
//!
//! Std-only file scanner (no crate imports), compiled standalone like the other
//! lints so it never links the native bin (Windows LNK1318 gotcha):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/page_parity_lint.rs
//! or run it through `just lints`.
//!
//! Deliberately NOT checked: that the two renderers produce identical pixels
//! (native is egui, web is HTML, a literal mirror is not the goal) or that every
//! page has a web twin (some are legitimately native-only, see PAGES.md).

use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A page that exists on both sides, and the data file both must read.
struct ParityPair {
    /// Page name, for failure messages.
    page: &'static str,
    /// Native sources that must mention `data_file`. Any one of them counts:
    /// the loader often lives with the other data-file loaders while the
    /// renderer is the page. Those loaders moved out of `gui/mod.rs` into
    /// `gui/loaders.rs` under the file-size ratchet, so both paths are listed
    /// below; the list follows the code rather than pinning it in place.
    native: &'static [&'static str],
    /// Web sources that must mention `data_file`.
    web: &'static [&'static str],
    /// The shared data path, written as it appears in source (forward slashes).
    data_file: &'static str,
    /// The READS themselves: `(file, call)` pairs, every one of which must
    /// appear in its file on a line that is not a whole-line comment. Empty
    /// for the older pairs, which only check that the path is mentioned.
    ///
    /// Why it exists: a bare mention cannot fail once a comment names the file.
    /// The Donate pairs were first written with mentions only (2026-10-04), and
    /// a review the same day deleted all four `loadList` calls from the web
    /// page and all four `read_data_json` calls from native while the lint
    /// still passed 5 of 5: header comments, the doc comments on the GuiState
    /// fields, a theme-exempt note and a test's assert message all name the
    /// files. A call is what reading the file means, so a call is what is
    /// matched: the loader's read, the startup line that puts its result in
    /// the app state, and the web page's fetch.
    reads: &'static [(&'static str, &'static str)],
}

const PARITY_PAIRS: &[ParityPair] = &[
    // Tools: the external catalog, free software + real-world help services.
    // Merged from three divergent files into one on 2026-07-30.
    ParityPair {
        page: "Tools",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/tools.rs"],
        web: &["web/pages/tools-app.js"],
        data_file: "external/catalog.json",
        reads: &[],
    },
    // Library: the documents. Both sides read the same manifest; the web page
    // fetches the markdown the manifest lists from the same directory.
    ParityPair {
        page: "Library",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/library.rs"],
        web: &["web/pages/library-app.js"],
        data_file: "library/index.json",
        reads: &[],
    },
    // Browser: the websites database. Until 2026-09-16 native read
    // data/browser/bookmarks.json and web.html carried its own DEFAULT_SITES
    // array (17 shared URLs out of 42). Both now read this one file, which
    // also records each site's embed-legality decision.
    ParityPair {
        page: "Browser",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/browser.rs"],
        web: &["web/pages/web.html"],
        data_file: "web/sites.json",
        reads: &[],
    },
    // Donate: four data files, and both pages must read all four, because this
    // page routes money and a drift here tells people the wrong thing about
    // where their gift goes or whether it is tax-deductible. Until 2026-10-04
    // the web page carried its own hardcoded FAQ (four entries) while native
    // read data/donate/faq.json (five different entries, two of them saying
    // things that were no longer true). Seen red first, 2026-10-04, before
    // the routes file existed and before either page read it:
    //   "Donate (giving routes): no native source mentions `donate/routes.json`"
    //   "Donate (giving routes): no web source mentions `donate/routes.json`"
    //   "Donate (FAQ): no web source mentions `donate/faq.json`"
    //   "Donate (giving routes): data/donate/routes.json does not exist"
    // Those mention checks could not fail once comments named the files (see
    // `reads` on ParityPair), so each pair also names its three read calls.
    // Seen red 2026-10-04 in a scratch copy with all four `loadList` calls
    // taken out of donate-app.js and all four `read_data_json` calls out of
    // loaders.rs, the change the review made (the mention checks stayed green):
    //   "Donate (giving routes): src/gui/loaders.rs no longer reads
    //    `donate/routes.json` (expected a line with `(data_dir, "donate/routes.json")`)"
    //   "Donate (giving routes): web/pages/donate-app.js no longer reads
    //    `donate/routes.json` (expected a line with
    //    `loadList('/data/donate/routes.json', 'routes')`)"
    //   ... and the same pair of lines for methods, charities and faq.
    // And with only the startup line for the FAQ taken out of lib.rs:
    //   "Donate (FAQ): src/lib.rs no longer reads `donate/faq.json` (expected a
    //    line with `crate::gui::load_donate_faq(&data_dir)`)"
    ParityPair {
        page: "Donate (giving routes)",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/donate.rs"],
        web: &["web/pages/donate-app.js"],
        data_file: "donate/routes.json",
        reads: &[
            ("src/gui/loaders.rs", "(data_dir, \"donate/routes.json\")"),
            ("src/lib.rs", "crate::gui::load_donate_routes(&data_dir)"),
            ("web/pages/donate-app.js", "loadList('/data/donate/routes.json', 'routes')"),
        ],
    },
    ParityPair {
        page: "Donate (direct links)",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/donate.rs"],
        web: &["web/pages/donate-app.js"],
        data_file: "donate/methods.json",
        reads: &[
            ("src/gui/loaders.rs", "(data_dir, \"donate/methods.json\")"),
            ("src/lib.rs", "crate::gui::load_donate_methods(&data_dir)"),
            ("web/pages/donate-app.js", "loadList('/data/donate/methods.json', 'methods')"),
        ],
    },
    ParityPair {
        page: "Donate (charities)",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/donate.rs"],
        web: &["web/pages/donate-app.js"],
        data_file: "donate/charities.json",
        reads: &[
            ("src/gui/loaders.rs", "(data_dir, \"donate/charities.json\")"),
            ("src/lib.rs", "crate::gui::load_donate_charities(&data_dir)"),
            ("web/pages/donate-app.js", "loadList('/data/donate/charities.json', 'charities')"),
        ],
    },
    ParityPair {
        page: "Donate (FAQ)",
        native: &["src/gui/loaders.rs", "src/gui/mod.rs", "src/gui/pages/donate.rs"],
        web: &["web/pages/donate-app.js"],
        data_file: "donate/faq.json",
        reads: &[
            ("src/gui/loaders.rs", "(data_dir, \"donate/faq.json\")"),
            ("src/lib.rs", "crate::gui::load_donate_faq(&data_dir)"),
            ("web/pages/donate-app.js", "loadList('/data/donate/faq.json', 'entries')"),
        ],
    },
];

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// A line that is only a comment, in Rust, JS or HTML: `//`, `///`, `//!`,
/// a `/*` opener, a ` * ` continuation, or `<!--`. A code line with a comment
/// after the code is NOT one, so a call followed by a note still counts.
fn is_comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.starts_with("<!--")
}

/// Does `needle` appear in `file` on a line that is not only a comment?
fn has_call(file: &str, needle: &str) -> bool {
    read(file).lines().any(|l| !is_comment_line(l) && l.contains(needle))
}

/// Every parity page must reference its shared data file on BOTH sides.
#[test]
fn both_uis_read_the_same_data_file() {
    let mut failures = Vec::new();

    for pair in PARITY_PAIRS {
        let native_hit = pair.native.iter().any(|f| read(f).contains(pair.data_file));
        let web_hit = pair.web.iter().any(|f| read(f).contains(pair.data_file));

        if !native_hit {
            failures.push(format!(
                "{}: no native source mentions `{}`\n    looked in: {}",
                pair.page,
                pair.data_file,
                pair.native.join(", ")
            ));
        }
        if !web_hit {
            failures.push(format!(
                "{}: no web source mentions `{}`\n    looked in: {}",
                pair.page,
                pair.data_file,
                pair.web.join(", ")
            ));
        }
        for (file, call) in pair.reads {
            if !has_call(file, call) {
                failures.push(format!(
                    "{}: {} no longer reads `{}` (expected a line with `{}`)",
                    pair.page, file, pair.data_file, call
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\n\nDual-UI data parity broken. Native and web must read the SAME data \
         file for these pages, or they silently drift into different products.\n\n{}\n\n\
         Fix by repointing the other side at the same file, or by updating \
         PARITY_PAIRS in tests/page_parity_lint.rs if the split is deliberate \
         (and say why in docs/PAGES.md).\n",
        failures.join("\n")
    );
}

/// The data files the pairs name must actually exist. Catches a rename that
/// updated both sides but not the file itself, which would otherwise leave both
/// clients agreeing on a path that 404s.
#[test]
fn parity_data_files_exist() {
    let mut missing = Vec::new();
    for pair in PARITY_PAIRS {
        let path = repo().join("data").join(pair.data_file);
        if !path.exists() {
            missing.push(format!("{}: data/{} does not exist", pair.page, pair.data_file));
        }
    }
    assert!(
        missing.is_empty(),
        "\n\nParity data file(s) missing:\n{}\n",
        missing.join("\n")
    );
}

/// The constitution must have exactly ONE document pipeline. /accord is a
/// focused public permalink over the same `data/library/` manifest that
/// /library and the native Library read (the Accord subset is flagged
/// `accord: true` by scripts/build-library.js). It used to fetch a separate
/// relay endpoint, which gave the constitution two independent sources that
/// could drift and made a static document unreadable whenever the relay was
/// down.
#[test]
fn accord_page_reads_the_shared_library_manifest() {
    let js = read("web/pages/accord-app.js");
    assert!(
        js.contains("library/index.json"),
        "\n\nweb/pages/accord-app.js must read the shared data/library manifest, \
         not a separate document source.\n"
    );
    // Match a STRING LITERAL, not prose: the header comment legitimately names
    // the retired endpoint to explain why it is retired, and a bare substring
    // check flags that comment as a violation.
    assert!(
        !js.contains("'/api/docs/accord") && !js.contains("\"/api/docs/accord"),
        "\n\nweb/pages/accord-app.js is back on the relay endpoint /api/docs/accord. \
         That is a SECOND source for the constitution; it can drift from \
         data/library/ and it breaks when the relay is down. Read the manifest \
         instead.\n"
    );
    // The flag the page filters on must actually exist in the generated data,
    // otherwise the page renders an empty Accord.
    let manifest = read("data/library/index.json");
    assert!(
        manifest.contains("\"accord\""),
        "\n\ndata/library/index.json has no `accord` flag, so /accord would show \
         nothing. Re-run `node scripts/build-library.js`.\n"
    );
}

/// Every file the library manifest names must exist on disk with the EXACT
/// byte-for-byte name. Path::exists() is case-insensitive on Windows, so a
/// manifest entry differing only in case passes every dev-machine check and
/// 404s on the case-sensitive live server: exactly how the v0.1064 Library
/// shipped with a live 404 (ONBOARDING.md vs onboarding.md, two source docs
/// whose basenames collided only case-insensitively; found 2026-07-31).
#[test]
fn library_manifest_files_exist_case_exact() {
    let manifest = read("data/library/index.json");
    let dir = repo().join("data/library");
    let on_disk: std::collections::HashSet<String> = std::fs::read_dir(&dir)
        .expect("read data/library")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    // Pull every "file": "..." value out of the manifest.
    let mut missing = Vec::new();
    let mut seen_lower = std::collections::HashMap::new();
    let mut from = 0usize;
    while let Some(i) = manifest[from..].find("\"file\":") {
        let start = from + i + 7;
        let rest = manifest[start..].trim_start();
        if let Some(stripped) = rest.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                let name = &stripped[..end];
                if !on_disk.contains(name) {
                    missing.push(name.to_string());
                }
                if let Some(prev) = seen_lower.insert(name.to_lowercase(), name.to_string()) {
                    if prev != name {
                        missing.push(format!(
                            "case-collision: {prev} vs {name} (same file on Windows)"
                        ));
                    }
                }
            }
        }
        from = start;
    }

    assert!(
        missing.is_empty(),
        "\n\ndata/library/index.json names files that do not exist CASE-EXACTLY on \
         disk (they will 404 on the live Linux server even though Windows finds \
         them):\n  {}\n\nRe-run `node scripts/build-library.js` (its collision check \
         is case-insensitive as of 2026-07-31) or fix the manifest.\n",
        missing.join("\n  ")
    );
}

/// The retired files must stay retired. They were merged into
/// data/external/catalog.json on 2026-07-30; if one reappears, the three-way
/// split is back and the pages will drift again.
#[test]
fn retired_catalogs_are_not_resurrected() {
    let retired = [
        "data/resources.json",
        "data/resources/catalog.json",
        "data/tools/catalog.json",
    ];
    let mut back = Vec::new();
    for r in retired {
        if repo().join(r).exists() {
            back.push(r);
        }
    }
    assert!(
        back.is_empty(),
        "\n\nThese catalogs were merged into data/external/catalog.json and must \
         not come back as separate sources:\n  {}\n\nAdd entries to \
         data/external/catalog.json instead.\n",
        back.join("\n  ")
    );
}
