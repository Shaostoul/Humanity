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
    /// the loader often lives in gui/mod.rs while the renderer is the page.
    native: &'static [&'static str],
    /// Web sources that must mention `data_file`.
    web: &'static [&'static str],
    /// The shared data path, written as it appears in source (forward slashes).
    data_file: &'static str,
}

const PARITY_PAIRS: &[ParityPair] = &[
    // Tools: the external catalog, free software + real-world help services.
    // Merged from three divergent files into one on 2026-07-30.
    ParityPair {
        page: "Tools",
        native: &["src/gui/mod.rs", "src/gui/pages/tools.rs"],
        web: &["web/pages/tools-app.js"],
        data_file: "external/catalog.json",
    },
    // Library: the documents. Both sides read the same manifest; the web page
    // fetches the markdown the manifest lists from the same directory.
    ParityPair {
        page: "Library",
        native: &["src/gui/mod.rs", "src/gui/pages/library.rs"],
        web: &["web/pages/library-app.js"],
        data_file: "library/index.json",
    },
];

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
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
