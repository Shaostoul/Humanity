//! Byproduct use lint (2026-09-26).
//!
//! Real processing leaves material behind: smelting leaves slag, sawing leaves
//! sawdust, milling leaves bran, pressing leaves cake and pomace. On a homestead
//! those leftovers feed something else (concrete, compost), and that is the
//! point of modelling them. A byproduct nothing consumes is just clutter that
//! fills the backpack, so this file makes that fail the build.
//!
//! It checks, from the shipped data/recipes.csv and data/items.csv:
//!
//!   1. every byproduct listed in BYPRODUCTS is a real item AND is made by at
//!      least one recipe alongside a main product (a stale entry fails);
//!   2. every byproduct is CONSUMED by at least one recipe that does not also
//!      make it, so no byproduct is a dead end;
//!   3. a recipe that yields a byproduct does not create mass: its outputs weigh
//!      no more than its inputs (items.csv weight_kg). The sawmill used to cut
//!      one 8 kg log into 10 kg of planks; adding sawdust on top of that would
//!      have hidden the defect instead of fixing it;
//!   4. no recipe hands back more of an item than it takes of that same item
//!      (tan_leather turned one hide into two of the same hide, so hides
//!      multiplied forever). One older recipe still does this and is listed in
//!      KNOWN_MULTIPLIERS with the reason; the list may only shrink.
//!
//! The ratios themselves, and their sources, are in the `#` comments next to
//! each recipe in data/recipes.csv.
//!
//! Standalone by design: std only, imports nothing from the crate, reads the
//! repo files at runtime, so it runs without linking the native bin (see the
//! LNK1318 note in CLAUDE.md). Run it the way `just lints` does:
//!
//!   CARGO_MANIFEST_DIR="$(pwd)" rustc --test --edition 2021 -A warnings \
//!       tests/byproduct_use_lint.rs -o /tmp/bul.exe && /tmp/bul.exe

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

/// Every byproduct item a recipe makes on the side. Add a row when a recipe
/// starts leaving something behind; the tests below then insist it is used.
const BYPRODUCTS: &[(&str, &str)] = &[
    ("slag_0", "smelting copper and iron ore (smelt_copper, smelt_copper_graphite, smelt_iron, smelt_iron_graphite)"),
    ("whey_0", "making cheese (cook_cheese)"),
    ("apple_pomace_0", "pressing apple juice (cook_juice)"),
    ("sawdust_0", "sawing logs into planks (saw_planks, saw_planks_hand)"),
    ("bran_0", "milling wheat into white flour (grind_flour)"),
    ("press_cake_0", "pressing oilseed (press_oil_rapeseed, _camelina, _safflower)"),
    ("olive_pomace_0", "pressing olives (press_oil_olive)"),
];

/// Recipes that already hand back more of an input than they take, each with
/// why it is not fixed here. (2026-09-26: craft_battery_charge,
/// craft_titanium_alloy and craft_nanomaterial were removed; each only
/// multiplied an item and made nothing any recipe used.) Fixing one means removing its row; a NEW recipe
/// that does this fails. The honest fix is a second item for the changed state
/// (tan_leather now makes leather_0 from leather_hide_0), not a bigger number.
const KNOWN_MULTIPLIERS: &[(&str, &str)] = &[
    (
        "vulcanize_rubber",
        "2 rubber_sheet_0 in, 3 out: needs a raw (unvulcanized) rubber item",
    ),
];

// -------------------------------------------------------------------------
// File reading (same conventions as tests/recipe_sources_lint.rs)
// -------------------------------------------------------------------------

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = project_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Split one CSV line into trimmed fields, quote-aware like the csv crate.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// A parsed CSV: `#` comment lines and blank lines are dropped BEFORE the
/// header is read, exactly like `crate::assets::loader::parse_csv`.
struct Csv {
    rel: String,
    header: Vec<String>,
    /// (1-based line number in the file, fields)
    rows: Vec<(usize, Vec<String>)>,
}

impl Csv {
    fn load(rel: &str) -> Csv {
        let text = read(rel);
        let mut lines = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim_start().starts_with('#') && !l.trim().is_empty());
        let (_, header) = lines
            .next()
            .unwrap_or_else(|| panic!("{rel}: no header row"));
        let header = split_csv_line(header);
        let rows = lines.map(|(i, l)| (i + 1, split_csv_line(l))).collect();
        Csv {
            rel: rel.to_string(),
            header,
            rows,
        }
    }

    fn col(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("{}: no `{name}` column in {:?}", self.rel, self.header))
    }
}

fn field(row: &[String], idx: usize) -> &str {
    row.get(idx).map(|s| s.as_str()).unwrap_or("")
}

/// `iron_ore_0:2|coal_0:1` -> [("iron_ore_0", 2), ("coal_0", 1)]. Mirrors
/// `Recipe::parse_ingredients`: the id is everything before the first `:`, a
/// missing or unparsable quantity counts as 1, and empty ids are skipped.
fn ingredients(cell: &str) -> Vec<(String, u32)> {
    cell.split('|')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, ':');
            let id = parts.next().unwrap_or("").trim();
            if id.is_empty() {
                return None;
            }
            let qty = parts
                .next()
                .and_then(|q| q.trim().parse::<u32>().ok())
                .unwrap_or(1);
            Some((id.to_string(), qty))
        })
        .collect()
}

struct Recipe {
    line: usize,
    id: String,
    inputs: Vec<(String, u32)>,
    outputs: Vec<(String, u32)>,
}

struct Data {
    /// item id -> weight_kg (None when the cell does not parse)
    items: BTreeMap<String, Option<f64>>,
    recipes: Vec<Recipe>,
}

fn load() -> Data {
    let icsv = Csv::load("data/items.csv");
    let (iid, iw) = (icsv.col("id"), icsv.col("weight_kg"));
    let items: BTreeMap<String, Option<f64>> = icsv
        .rows
        .iter()
        .filter(|(_, r)| !field(r, iid).is_empty())
        .map(|(_, r)| (field(r, iid).to_string(), field(r, iw).parse::<f64>().ok()))
        .collect();

    let rcsv = Csv::load("data/recipes.csv");
    let (rid, rin, rout) = (rcsv.col("id"), rcsv.col("inputs"), rcsv.col("outputs"));
    let recipes: Vec<Recipe> = rcsv
        .rows
        .iter()
        .filter(|(_, r)| !field(r, rid).is_empty())
        .map(|(line, r)| Recipe {
            line: *line,
            id: field(r, rid).to_string(),
            inputs: ingredients(field(r, rin)),
            outputs: ingredients(field(r, rout)),
        })
        .collect();

    // Non-vacuity: a parse that silently matched nothing would pass everything.
    assert!(items.len() >= 500, "items.csv parsed only {} ids", items.len());
    assert!(recipes.len() >= 300, "recipes.csv parsed only {} rows", recipes.len());
    Data { items, recipes }
}

fn has(list: &[(String, u32)], id: &str) -> bool {
    list.iter().any(|(i, _)| i == id)
}

fn qty(list: &[(String, u32)], id: &str) -> u32 {
    list.iter().filter(|(i, _)| i == id).map(|(_, q)| *q).sum()
}

// -------------------------------------------------------------------------
// The checks
// -------------------------------------------------------------------------

#[test]
fn every_byproduct_is_a_real_item_made_beside_a_main_product() {
    let d = load();
    let mut problems = Vec::new();
    for (id, from) in BYPRODUCTS {
        if !d.items.contains_key(*id) {
            problems.push(format!("  {id} ({from}) is not an item in data/items.csv"));
            continue;
        }
        let made = d
            .recipes
            .iter()
            .any(|r| has(&r.outputs, id) && r.outputs.iter().any(|(o, _)| o != id));
        if !made {
            problems.push(format!(
                "  {id} ({from}) is made by no recipe as a side output. Either a recipe \
                 lost its byproduct or this BYPRODUCTS row is stale."
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "\n\n[FAIL] BYPRODUCTS in tests/byproduct_use_lint.rs does not match the data:\n\n{}\n",
        problems.join("\n")
    );
}

#[test]
fn every_byproduct_is_consumed_by_a_recipe() {
    let d = load();
    let mut dead_ends = Vec::new();
    for (id, from) in BYPRODUCTS {
        let users: Vec<&str> = d
            .recipes
            .iter()
            .filter(|r| has(&r.inputs, id) && !has(&r.outputs, id))
            .map(|r| r.id.as_str())
            .collect();
        if users.is_empty() {
            dead_ends.push(format!("  {id}, left by {from}"));
        }
    }
    assert!(
        dead_ends.is_empty(),
        "\n\n[FAIL] {} byproduct(s) are made but no recipe uses them, so they only pile up \
         in the backpack:\n\n{}\n\n\
         Give each a real use the way a homestead would (compost, feed, fuel, aggregate), \
         as a recipe in data/recipes.csv that consumes it.\n",
        dead_ends.len(),
        dead_ends.join("\n")
    );
}

#[test]
fn byproduct_recipes_do_not_create_mass() {
    let d = load();
    let ids: BTreeSet<&str> = BYPRODUCTS.iter().map(|(id, _)| *id).collect();
    let mass = |list: &[(String, u32)], problems: &mut Vec<String>, rid: &str| -> f64 {
        list.iter()
            .map(|(id, q)| match d.items.get(id) {
                Some(Some(w)) => w * f64::from(*q),
                _ => {
                    problems.push(format!("  {rid}: {id} has no weight_kg in data/items.csv"));
                    0.0
                }
            })
            .sum()
    };
    let mut problems = Vec::new();
    let mut checked = 0;
    for r in d
        .recipes
        .iter()
        .filter(|r| r.outputs.iter().any(|(o, _)| ids.contains(o.as_str())))
    {
        checked += 1;
        let m_in = mass(&r.inputs, &mut problems, &r.id);
        let m_out = mass(&r.outputs, &mut problems, &r.id);
        if m_out > m_in + 1e-6 {
            problems.push(format!(
                "  data/recipes.csv:{} {}: {m_in:.2} kg in, {m_out:.2} kg out",
                r.line, r.id
            ));
        }
    }
    assert!(checked >= BYPRODUCTS.len(), "checked only {checked} byproduct recipes");
    assert!(
        problems.is_empty(),
        "\n\n[FAIL] recipes that leave a byproduct must not make matter out of nothing: \
         the main product plus the byproduct can weigh at most what went in (the rest \
         leaves as gas, water or waste that is not modelled).\n\n{}\n",
        problems.join("\n")
    );
}

#[test]
fn no_recipe_multiplies_its_own_input() {
    let d = load();
    let known: BTreeMap<&str, &str> = KNOWN_MULTIPLIERS.iter().copied().collect();
    let mut offenders: BTreeSet<&str> = BTreeSet::new();
    let mut new_ones = Vec::new();
    for r in &d.recipes {
        for (id, q_in) in &r.inputs {
            let q_out = qty(&r.outputs, id);
            if q_out > qty(&r.inputs, id) {
                offenders.insert(r.id.as_str());
                if !known.contains_key(r.id.as_str()) {
                    new_ones.push(format!(
                        "  data/recipes.csv:{} {}: takes {q_in} {id} and gives back {q_out}",
                        r.line, r.id
                    ));
                }
            }
        }
    }
    let stale: Vec<&str> = known
        .keys()
        .copied()
        .filter(|k| !offenders.contains(k))
        .collect();
    assert!(
        new_ones.is_empty(),
        "\n\n[FAIL] a recipe hands back more of an item than it takes, so crafting it over \
         and over multiplies the item for free:\n\n{}\n\n\
         Make the changed state its own item (a raw hide tans into leather, not into two \
         raw hides).\n",
        new_ones.join("\n")
    );
    assert!(
        stale.is_empty(),
        "\n\n[FAIL] fixed, so remove from KNOWN_MULTIPLIERS in tests/byproduct_use_lint.rs: \
         {stale:?}\n"
    );
}

// -------------------------------------------------------------------------
// The parsers must not be silent no-ops.
// -------------------------------------------------------------------------

#[test]
fn ingredient_parser_matches_the_crafting_loader() {
    assert_eq!(
        ingredients("iron_ore_0:2| coal_0 :1||x"),
        vec![
            ("iron_ore_0".to_string(), 2),
            ("coal_0".to_string(), 1),
            ("x".to_string(), 1)
        ]
    );
    assert_eq!(
        split_csv_line(r#"a, "b,c" ,"say ""hi""",d"#),
        vec!["a", "b,c", "say \"hi\"", "d"]
    );
}
