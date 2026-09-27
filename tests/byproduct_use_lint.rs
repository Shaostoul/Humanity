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
//!   2. every byproduct is USED: consumed by at least one recipe that does not
//!      also make it, or applied in the garden as a soil amendment
//!      (data/garden/soil_ph.ron `amendments`, 2026-09-27: wood ash limes soil,
//!      which is its real use and not a recipe), so no byproduct is a dead end;
//!   3. a recipe that yields a byproduct does not create mass: its outputs weigh
//!      no more than its inputs (items.csv weight_kg). The sawmill used to cut
//!      one 8 kg log into 10 kg of planks; adding sawdust on top of that would
//!      have hidden the defect instead of fixing it;
//!   4. no recipe hands back more of an item than it takes of that same item
//!      (tan_leather turned one hide into two of the same hide, so hides
//!      multiplied forever). No recipe does this any more: vulcanize_rubber, the
//!      last, now cures raw_rubber_0 into rubber_sheet_0 (2026-09-26), so
//!      KNOWN_MULTIPLIERS is empty and a new multiplier fails;
//!   5. (2026-09-27) a fire that burns charcoal as fuel leaves its ash, at the
//!      FAO ash share of the charcoal, and an ore smelt does not (its ash goes
//!      into the slag); paddy rice is milled at IRRI's split before anything
//!      cooks it and is not food as it is; the sunflower press gives no more
//!      oil than the seed holds and leaves the cake at its cited oil content.
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
    ("wood_slab_0", "sawing logs into planks at the sawmill (saw_planks)"),
    ("bark_0", "sawing logs into planks (saw_planks, saw_planks_hand)"),
    ("bran_0", "milling wheat into white flour (grind_flour)"),
    ("press_cake_0", "pressing oilseed (press_oil_rapeseed, _camelina, _safflower, _sunflower)"),
    ("olive_pomace_0", "pressing olives (press_oil_olive)"),
    ("rice_hulls_0", "milling paddy into white rice (mill_rice)"),
    ("rice_bran_0", "milling paddy into white rice (mill_rice)"),
    ("wood_ash_0", "burning charcoal as fuel in the kiln, the forge and the smelter's melts"),
];

/// The ash share of charcoal: "Good quality lump charcoal typically has ash
/// content of about 3%" (FAO Forestry Paper 41, "Simple technologies for
/// charcoal making", 1987, section 10.1.4; data/recipes.csv carries the
/// quote). coal_0 is charcoal.
const CHARCOAL_ASH_SHARE: f64 = 0.03;

/// The stations whose recipes burn their charcoal as fuel.
const FIRE_STATIONS: &[&str] = &["kiln_0", "forge_0", "smelter_0"];

/// IRRI Rice Knowledge Bank, "Milling": an ideal mill gives "20% husk, 8−12%
/// bran depending on the milling degree and 68−72% milled rice or white rice".
/// (item, low share, high share) of the paddy's mass.
const RICE_MILLING: &[(&str, f64, f64)] = &[("rice_0", 0.68, 0.72), ("rice_hulls_0", 0.20, 0.20), ("rice_bran_0", 0.08, 0.12)];

/// Whole oil-type sunflower seed is 48% ether extract in 92.8% dry matter
/// (Feedipedia node/40), and a screw press leaves "a "cake" containing 15-20%
/// of oil" (Feedipedia node/732).
const SUNFLOWER_SEED_OIL: f64 = 0.48 * 0.928;
const SUNFLOWER_CAKE_OIL: (f64, f64) = (0.15, 0.20);

/// Recipes that already hand back more of an input than they take, each with
/// why it is not fixed here. (2026-09-26: craft_battery_charge,
/// craft_titanium_alloy and craft_nanomaterial were removed; each only
/// multiplied an item and made nothing any recipe used.) Fixing one means removing its row; a NEW recipe
/// that does this fails. The honest fix is a second item for the changed state
/// (tan_leather now makes leather_0 from leather_hide_0), not a bigger number.
const KNOWN_MULTIPLIERS: &[(&str, &str)] = &[];

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
    category: String,
    station: String,
    inputs: Vec<(String, u32)>,
    outputs: Vec<(String, u32)>,
}

struct Data {
    /// item id -> weight_kg (None when the cell does not parse)
    items: BTreeMap<String, Option<f64>>,
    /// item id -> items.csv subcategory ("ore", "byproduct", ...)
    subcategory: BTreeMap<String, String>,
    recipes: Vec<Recipe>,
}

impl Data {
    fn weight(&self, id: &str) -> f64 {
        self.items
            .get(id)
            .copied()
            .flatten()
            .unwrap_or_else(|| panic!("{id} has no weight_kg in data/items.csv"))
    }

    fn mass(&self, list: &[(String, u32)]) -> f64 {
        list.iter().map(|(id, q)| self.weight(id) * f64::from(*q)).sum()
    }

    fn recipe(&self, id: &str) -> &Recipe {
        self.recipes
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("data/recipes.csv has no recipe {id}"))
    }
}

fn load() -> Data {
    let icsv = Csv::load("data/items.csv");
    let (iid, iw, isub) = (icsv.col("id"), icsv.col("weight_kg"), icsv.col("subcategory"));
    let named = || icsv.rows.iter().filter(|(_, r)| !field(r, iid).is_empty());
    let items: BTreeMap<String, Option<f64>> =
        named().map(|(_, r)| (field(r, iid).to_string(), field(r, iw).parse::<f64>().ok())).collect();
    let subcategory: BTreeMap<String, String> =
        named().map(|(_, r)| (field(r, iid).to_string(), field(r, isub).to_string())).collect();

    let rcsv = Csv::load("data/recipes.csv");
    let (rid, rcat, rst) = (rcsv.col("id"), rcsv.col("category"), rcsv.col("station_required"));
    let (rin, rout) = (rcsv.col("inputs"), rcsv.col("outputs"));
    let recipes: Vec<Recipe> = rcsv
        .rows
        .iter()
        .filter(|(_, r)| !field(r, rid).is_empty())
        .map(|(line, r)| Recipe {
            line: *line,
            id: field(r, rid).to_string(),
            category: field(r, rcat).to_string(),
            station: field(r, rst).to_string(),
            inputs: ingredients(field(r, rin)),
            outputs: ingredients(field(r, rout)),
        })
        .collect();

    // Non-vacuity: a parse that silently matched nothing would pass everything.
    assert!(items.len() >= 500, "items.csv parsed only {} ids", items.len());
    assert!(recipes.len() >= 300, "recipes.csv parsed only {} rows", recipes.len());
    Data { items, subcategory, recipes }
}

/// The `item: "..."` of every soil amendment in data/garden/soil_ph.ron's
/// `amendments: [ ... ]` list (the Garden panel's Lime, Sulfur and Wood ash
/// buttons, which spend that item). Read as text between the list's opening
/// and its closing `],` at the same indent; the file's other `item:` fields
/// (fertilizer_acidity) sit outside it.
fn amendment_items() -> Vec<String> {
    let text = read("data/garden/soil_ph.ron");
    let start = text
        .find("\n    amendments: [")
        .expect("data/garden/soil_ph.ron has an `amendments: [` list");
    let body = &text[start..];
    let end = body.find("\n    ],").expect("the amendments list closes with `    ],`");
    body[..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter_map(|l| l.split("item: \"").nth(1)?.split('"').next().map(str::to_string))
        .collect()
}

/// The ids listed in one `name: [ ... ]` block of data/food/item_profiles.ron.
fn profile_block(name: &str) -> BTreeSet<String> {
    let text = read("data/food/item_profiles.ron");
    let start = text
        .find(&format!("\n    {name}: ["))
        .unwrap_or_else(|| panic!("item_profiles.ron has no `{name}: [` block"));
    let body = &text[start..];
    let end = body.find("\n    ],").expect("the block closes with `    ],`");
    body[..end]
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("(\""))
        .filter_map(|l| l.strip_prefix("(\"")?.split('"').next().map(str::to_string))
        .collect()
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

/// A byproduct is used when a recipe consumes it (compost, concrete, fuel) or
/// the Garden panel spends it as a soil amendment: wood ash (2026-09-27) is
/// the case, and liming is what gardeners really do with it. Seen red by
/// renaming wood ash's amendment item in soil_ph.ron to "wood_ash_1" (wood ash
/// then had no use).
#[test]
fn every_byproduct_is_used_by_a_recipe_or_the_garden() {
    let d = load();
    let amendments = amendment_items();
    // Non-vacuity: the text scan must find the amendments that exist.
    assert!(
        amendments.iter().any(|a| a == "garden_lime_0") && amendments.iter().any(|a| a == "garden_sulfur_0"),
        "soil_ph.ron amendments read as {amendments:?}"
    );
    let mut dead_ends = Vec::new();
    for (id, from) in BYPRODUCTS {
        let users: Vec<&str> = d
            .recipes
            .iter()
            .filter(|r| has(&r.inputs, id) && !has(&r.outputs, id))
            .map(|r| r.id.as_str())
            .collect();
        if users.is_empty() && !amendments.iter().any(|a| a == id) {
            dead_ends.push(format!("  {id}, left by {from}"));
        }
    }
    assert!(
        dead_ends.is_empty(),
        "\n\n[FAIL] {} byproduct(s) are made but nothing uses them, so they only pile up \
         in the backpack:\n\n{}\n\n\
         Give each a real use the way a homestead would (compost, feed, fuel, aggregate), \
         as a recipe in data/recipes.csv that consumes it, or as a soil amendment in \
         data/garden/soil_ph.ron if that is what it is for.\n",
        dead_ends.len(),
        dead_ends.join("\n")
    );
}

/// Charcoal burnt as FUEL leaves its ash: every kiln, forge and smelter recipe
/// that burns coal_0 (charcoal) and charges no ore leaves one wood_ash_0 per
/// lump burnt, and wood_ash_0 weighs FAO's 3% of a lump. A recipe that charges
/// ore leaves none, because there the charcoal is the ore's reducing agent and
/// its ash melts into the slag (data/recipes.csv carries both sources). Seen
/// red two ways: fire_porcelain's wood_ash_0 removed (the fuel-only check named
/// it), and wood_ash_0:1 added to smelt_nickel (the ore check named it).
#[test]
fn charcoal_fires_leave_their_ash_and_ore_smelts_do_not() {
    let d = load();
    let per_lump = d.weight("coal_0") * CHARCOAL_ASH_SHARE;
    assert!(
        (d.weight("wood_ash_0") - per_lump).abs() < 1e-9,
        "wood_ash_0 weighs {} kg; 3% of a {} kg lump is {per_lump}",
        d.weight("wood_ash_0"),
        d.weight("coal_0")
    );
    let mut problems = Vec::new();
    let (mut fuel_only, mut ore_smelts) = (0, 0);
    for r in d.recipes.iter().filter(|r| FIRE_STATIONS.contains(&r.station.as_str())) {
        let burnt = i64::from(qty(&r.inputs, "coal_0")) - i64::from(qty(&r.outputs, "coal_0"));
        if burnt <= 0 {
            continue; // make_charcoal: charcoal comes out of the kiln
        }
        let ash = i64::from(qty(&r.outputs, "wood_ash_0"));
        let charges_ore = r.inputs.iter().any(|(id, _)| d.subcategory.get(id).map(String::as_str) == Some("ore"));
        if charges_ore {
            ore_smelts += 1;
            if ash != 0 {
                problems.push(format!("  {} charges ore, so its ash is in its slag, but it leaves {ash} wood_ash_0", r.id));
            }
        } else {
            fuel_only += 1;
            if ash != burnt {
                problems.push(format!("  {} burns {burnt} coal_0 as fuel and leaves {ash} wood_ash_0, not {burnt}", r.id));
            }
        }
    }
    assert!(fuel_only >= 8 && ore_smelts >= 6, "checked {fuel_only} fuel fires and {ore_smelts} ore smelts");
    assert!(problems.is_empty(), "\n\n[FAIL] charcoal ash:\n\n{}\n", problems.join("\n"));
}

/// Paddy rice is milled before it is eaten: mill_rice splits the paddy's mass
/// into white rice, hulls and bran inside IRRI's ranges, no cooking recipe
/// takes paddy, the porridge takes milled rice, and item_profiles.ron files
/// the paddy as not food and the white rice as food. Seen red by setting
/// cook_porridge back to grain_rice_0 (the cooking check named it).
#[test]
fn paddy_is_milled_at_irris_split_before_it_is_eaten() {
    let d = load();
    let mill = d.recipe("mill_rice");
    assert_eq!(mill.inputs.len(), 1, "mill_rice takes only paddy: {:?}", mill.inputs);
    assert_eq!(mill.inputs[0].0, "grain_rice_0");
    let paddy = d.mass(&mill.inputs);
    assert!((d.mass(&mill.outputs) - paddy).abs() < 1e-9, "milling neither makes nor loses mass");
    for (item, lo, hi) in RICE_MILLING {
        let share = d.weight(item) * f64::from(qty(&mill.outputs, item)) / paddy;
        assert!(
            (lo - 1e-9..=hi + 1e-9).contains(&share),
            "mill_rice gives {item} at {share:.3} of the paddy; IRRI gives {lo} to {hi}"
        );
    }
    let cooks_paddy: Vec<&str> = d
        .recipes
        .iter()
        .filter(|r| r.category == "cooking" && has(&r.inputs, "grain_rice_0"))
        .map(|r| r.id.as_str())
        .collect();
    assert!(cooks_paddy.is_empty(), "these cook paddy rice, hull and all: {cooks_paddy:?}");
    assert!(has(&d.recipe("cook_porridge").inputs, "rice_0"), "porridge is milled rice in milk");
    let (food, not_food) = (profile_block("items"), profile_block("not_food"));
    assert!(food.len() > 100 && not_food.len() > 5, "item_profiles.ron read: {} and {}", food.len(), not_food.len());
    assert!(not_food.contains("grain_rice_0") && !food.contains("grain_rice_0"), "paddy is not eaten as it is");
    assert!(food.contains("rice_0"), "white rice is food");
}

/// The sunflower press takes no more oil out than the seed holds, and leaves
/// the cake at the oil a screw press leaves (Feedipedia's 15 to 20%). Seen red
/// by making it 3 oil and 3 cake (1.50 kg of oil pressed out of seed holding
/// 1.34 kg; the check read the cake at -10.9%).
#[test]
fn the_sunflower_press_matches_the_seed_it_presses() {
    let d = load();
    let press = d.recipe("press_oil_sunflower");
    let seed = d.mass(&press.inputs);
    assert_eq!(press.inputs, vec![("fruit_sunflower_0".to_string(), qty(&press.inputs, "fruit_sunflower_0"))]);
    let oil_in = seed * SUNFLOWER_SEED_OIL;
    let oil_out = d.weight("cooking_oil_0") * f64::from(qty(&press.outputs, "cooking_oil_0"));
    let cake = d.weight("press_cake_0") * f64::from(qty(&press.outputs, "press_cake_0"));
    assert!((oil_out + cake - seed).abs() < 1e-9, "oil and cake are the seed");
    let left = (oil_in - oil_out) / cake;
    let (lo, hi) = SUNFLOWER_CAKE_OIL;
    assert!(
        (lo..=hi).contains(&left),
        "the cake keeps {:.1}% oil ({oil_in:.2} kg in the seed, {oil_out:.2} kg pressed out); a screw press leaves {lo} to {hi}",
        left * 100.0
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
