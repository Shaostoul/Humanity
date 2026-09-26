//! Recipe sources lint (2026-09-25).
//!
//! THE DEFECT CLASS. A recipe is only real if a player can gather every one of
//! its inputs. A survey on 2026-09-25 found twelve recipes that could never be
//! crafted, and nothing had noticed, because every existing check asks whether
//! an input id EXISTS in items.csv, not whether anything in the game ever GIVES
//! the player one. Examples of what slipped through:
//!
//!   * `painkiller_0` was consumed by the medkit recipes, while the chemistry
//!     recipe made `painkillers_0` (a one-letter typo, both ids were items).
//!   * `animal_fat_0` dropped only from the pig and the polar bear, and neither
//!     animal is ever spawned.
//!   * `fiber_cotton_0` came only from harvesting cotton, and cotton had no seed
//!     item, so nobody could ever plant it.
//!   * `fiberglass` was not an item at all.
//!
//! This file makes that class fail the build. It checks, from the shipped data:
//!
//!   1. every recipe input has at least one SOURCE (below);
//!   2. every recipe can actually be reached from those sources, which also
//!      catches a loop where two recipes only make each other's inputs;
//!   3. no recipe id is defined twice (the loader keeps the LAST row, so the
//!      other definition silently disappears);
//!   4. every recipe `category` has exactly one Crafting-sidebar entry in
//!      data/crafting/categories.json, and no sidebar entry is empty;
//!   5. every plant in plants.csv has its `seed_<plant>_0` item, because
//!      src/systems/farming/mod.rs plants from and harvests back to that id;
//!   6. every data/world/showcase.ron `bed_crops` entry names a real machine
//!      type and a real plant, so no grow surface is silently left bare.
//!
//! WHAT COUNTS AS A SOURCE of an item:
//!
//!   * another recipe's output (a recipe does not source its own input);
//!   * a plant's harvest, but only when that plant has a seed item to plant;
//!     the harvest resolves the way farming's `harvest_item_for` does: the
//!     plants.csv `harvest_item` column when it names a real item, otherwise
//!     `vegetable_/fruit_/grain_<plant>_0`;
//!   * the seed item of any plant in plants.csv;
//!   * a loot drop or renewable product of a creature that is actually SPAWNED,
//!     meaning it is listed in data/entities/wild_spawns.ron or
//!     data/entities/livestock.ron (a creatures.csv row nobody spawns drops
//!     nothing); loot ids resolve the way `CreatureDef::loot_entries` does:
//!     the exact id, else `<id>_0`;
//!   * an item the vendor sells: a data/trade_goods.ron id that is also in
//!     items.csv (the vendor catalog in src/lib.rs filters to exactly that);
//!   * an ore from the three test asteroids in src/lib.rs (ASTEROID_ORES);
//!   * the player's starting items in data/world/player.ron.
//!
//! Standalone by design: std only, imports nothing from the crate, reads the
//! repo files at runtime, so it runs without linking the native bin (see the
//! LNK1318 note in CLAUDE.md). Run it the way `just lints` does:
//!
//!   CARGO_MANIFEST_DIR="$(pwd)" rustc --test --edition 2021 -A warnings \
//!       tests/recipe_sources_lint.rs -o /tmp/rsl.exe && /tmp/rsl.exe

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;

/// Ores the three test asteroids yield. COPIED from the `AsteroidBody { ..
/// ores: vec![..] }` spawns in src/lib.rs (grep "AsteroidBody {"): M-12
/// (metallic), S-7 (silicaceous) and C-3 (carbonaceous). The test
/// `asteroid_ore_list_matches_src_lib` fails if this list and src/lib.rs drift
/// apart, so update both together.
const ASTEROID_ORES: &[&str] = &[
    // M-12
    "iron_ore_0",
    "nickel_ore_0",
    "platinum_ore_0",
    "gold_ore_0",
    "silver_ore_0",
    // S-7
    "copper_ore_0",
    "bauxite_0",
    "rutile_0",
    // C-3
    "graphite_0",
];

// -------------------------------------------------------------------------
// File reading
// -------------------------------------------------------------------------

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = project_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Split one CSV line into trimmed fields. Quote-aware (a quoted field may hold
/// commas, and `""` is an escaped quote), matching what the csv crate does for
/// the shipped files. Fields are trimmed like `csv::Trim::All`.
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

/// A parsed id-style CSV: `#` comment lines and blank lines are dropped BEFORE
/// the header is read, exactly like `crate::assets::loader::parse_csv`.
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

/// Item ids named in a recipe inputs/outputs cell (`iron_ore_0:2|coal_0:1`).
/// Mirrors `Recipe::parse_ingredients`: split on `|`, the id is everything
/// before the first `:`, trimmed, and empty ids are skipped.
fn ingredient_ids(cell: &str) -> Vec<String> {
    cell.split('|')
        .filter_map(|pair| {
            let id = pair.split(':').next().unwrap_or("").trim();
            if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            }
        })
        .collect()
}

/// Remove `//` line comments from RON text so a commented-out row (the
/// wolves in wild_spawns.ron) does not count. The shipped RON files carry no
/// `//` inside string values; `strip_comments_keeps_strings` pins that.
fn strip_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut in_str = false;
        let mut prev = '\0';
        let mut cut = line.len();
        for (i, c) in line.char_indices() {
            if c == '"' && prev != '\\' {
                in_str = !in_str;
            }
            if !in_str && c == '/' && prev == '/' {
                cut = i - 1;
                break;
            }
            prev = c;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// Every string value that directly follows `key:` (RON) or `"key":` (JSON),
/// e.g. `creature: "chicken"` -> "chicken". Whitespace between is allowed.
fn quoted_after_key(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut search = 0;
    while let Some(off) = text[search..].find(key) {
        let start = search + off;
        search = start + key.len();
        // The key must not be the tail of a longer identifier (`id:` inside `def_id:`).
        if start > 0 {
            let p = bytes[start - 1] as char;
            if p.is_ascii_alphanumeric() || p == '_' {
                continue;
            }
        }
        let mut i = search;
        if bytes.get(i) == Some(&b'"') {
            i += 1; // JSON: "key" then ':'
        }
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            continue;
        }
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            continue;
        }
        i += 1;
        if let Some(end) = text[i..].find('"') {
            out.push(text[i..i + end].to_string());
        }
    }
    out
}

/// All `"..."` string values inside the `[ ... ]` that follows `key:`.
fn quoted_in_list_after(text: &str, key: &str) -> Vec<String> {
    let Some(k) = text.find(key) else {
        return Vec::new();
    };
    let rest = &text[k + key.len()..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let Some(close) = rest[open..].find(']') else {
        return Vec::new();
    };
    let body = &rest[open + 1..open + close];
    body.split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect()
}

// -------------------------------------------------------------------------
// The shipped data, loaded once per test
// -------------------------------------------------------------------------

struct Recipe {
    line: usize,
    id: String,
    category: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

struct Data {
    items: BTreeSet<String>,
    recipes: Vec<Recipe>,
    plant_ids: Vec<String>,
    /// item id -> every non-recipe way the game hands one to the player.
    base_sources: BTreeMap<String, Vec<String>>,
}

fn load() -> Data {
    let items_csv = Csv::load("data/items.csv");
    let id = items_csv.col("id");
    let items: BTreeSet<String> = items_csv
        .rows
        .iter()
        .map(|(_, r)| field(r, id).to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let rcsv = Csv::load("data/recipes.csv");
    let (rid, rcat, rin, rout) = (
        rcsv.col("id"),
        rcsv.col("category"),
        rcsv.col("inputs"),
        rcsv.col("outputs"),
    );
    let recipes: Vec<Recipe> = rcsv
        .rows
        .iter()
        .filter(|(_, r)| !field(r, rid).is_empty())
        .map(|(line, r)| Recipe {
            line: *line,
            id: field(r, rid).to_string(),
            category: field(r, rcat).to_string(),
            inputs: ingredient_ids(field(r, rin)),
            outputs: ingredient_ids(field(r, rout)),
        })
        .collect();

    let mut base_sources: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut add = |item: &str, why: String| {
        base_sources.entry(item.to_string()).or_default().push(why);
    };

    // Plants: the seed item, and the harvest when the plant can be planted.
    let pcsv = Csv::load("data/plants.csv");
    let (pid, phv) = (pcsv.col("id"), pcsv.col("harvest_item"));
    let mut plant_ids = Vec::new();
    for (_, r) in &pcsv.rows {
        let plant = field(r, pid);
        if plant.is_empty() {
            continue;
        }
        plant_ids.push(plant.to_string());
        let seed = format!("seed_{plant}_0");
        if !items.contains(&seed) {
            continue; // unplantable: its harvest reaches no one
        }
        add(&seed, format!("seed of plant `{plant}`"));
        let explicit = field(r, phv);
        let harvest = if !explicit.is_empty() && items.contains(explicit) {
            Some(explicit.to_string())
        } else {
            ["vegetable", "fruit", "grain"]
                .iter()
                .map(|p| format!("{p}_{plant}_0"))
                .find(|c| items.contains(c))
        };
        if let Some(h) = harvest {
            add(&h, format!("harvest of plant `{plant}`"));
        }
    }

    // Creatures that are actually spawned: loot drops + renewable products.
    let mut spawned: BTreeSet<String> = BTreeSet::new();
    for rel in ["data/entities/wild_spawns.ron", "data/entities/livestock.ron"] {
        let text = strip_line_comments(&read(rel));
        for c in quoted_after_key(&text, "creature") {
            spawned.insert(c);
        }
    }
    let ccsv = Csv::load("data/creatures.csv");
    let (cid, cloot, crenew) = (
        ccsv.col("id"),
        ccsv.col("loot_table"),
        ccsv.col("renewable_product"),
    );
    for (_, r) in &ccsv.rows {
        let creature = field(r, cid);
        if !spawned.contains(creature) {
            continue;
        }
        for seg in field(r, cloot).split('|') {
            let raw = seg.split(':').next().unwrap_or("").trim();
            if raw.is_empty() {
                continue;
            }
            let suffixed = format!("{raw}_0");
            let resolved = if items.contains(raw) {
                raw.to_string()
            } else if items.contains(&suffixed) {
                suffixed
            } else {
                raw.to_string()
            };
            add(&resolved, format!("drop from spawned creature `{creature}`"));
        }
        let product = field(r, crenew).split(':').next().unwrap_or("").trim().to_string();
        if !product.is_empty() {
            add(&product, format!("renewable product of spawned creature `{creature}`"));
        }
    }

    // The vendor: trade goods that are also items (the catalog filter in src/lib.rs).
    let goods = strip_line_comments(&read("data/trade_goods.ron"));
    for g in quoted_after_key(&goods, "id") {
        if items.contains(&g) {
            add(&g, "sold by the vendor (trade_goods.ron)".to_string());
        }
    }

    for ore in ASTEROID_ORES {
        add(ore, "mined from a test asteroid (src/lib.rs)".to_string());
    }

    let player = strip_line_comments(&read("data/world/player.ron"));
    for s in quoted_in_list_after(&player, "starting_items") {
        add(&s, "player starting item (world/player.ron)".to_string());
    }

    // Non-vacuity: a scan that silently matched nothing would pass everything.
    assert!(items.len() >= 500, "items.csv parsed only {} ids", items.len());
    assert!(recipes.len() >= 300, "recipes.csv parsed only {} rows", recipes.len());
    assert!(plant_ids.len() >= 100, "plants.csv parsed only {} rows", plant_ids.len());
    assert!(
        spawned.len() >= 3,
        "found only {} spawned creatures in wild_spawns.ron + livestock.ron: {spawned:?}",
        spawned.len()
    );
    assert!(
        base_sources.values().flatten().any(|w| w.starts_with("sold by the vendor")),
        "no vendor goods matched items.csv: trade_goods.ron parse is broken"
    );

    Data {
        items,
        recipes,
        plant_ids,
        base_sources,
    }
}

// -------------------------------------------------------------------------
// The checks
// -------------------------------------------------------------------------

#[test]
fn every_recipe_input_has_a_source() {
    let d = load();
    // item -> ids of the recipes that output it
    let mut made_by: HashMap<&str, Vec<&str>> = HashMap::new();
    for r in &d.recipes {
        for o in &r.outputs {
            made_by.entry(o.as_str()).or_default().push(r.id.as_str());
        }
    }
    // unsourced item -> recipes that need it
    let mut missing: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for r in &d.recipes {
        for i in &r.inputs {
            let by_another = made_by
                .get(i.as_str())
                .map(|v| v.iter().any(|x| *x != r.id))
                .unwrap_or(false);
            if by_another || d.base_sources.contains_key(i) {
                continue;
            }
            missing.entry(i.as_str()).or_default().insert(r.id.as_str());
        }
    }
    if !missing.is_empty() {
        let mut msg = format!(
            "\n\n[FAIL] {} recipe input(s) have NO source anywhere in the game, so every \
             recipe that needs them can never be crafted.\n\
             A source is: another recipe's output, a plantable plant's harvest or a \
             plant's seed item, a drop or product of a SPAWNED creature, a vendor \
             good that is also in items.csv, a test-asteroid ore, or a player starting \
             item (see the header of tests/recipe_sources_lint.rs).\n\n",
            missing.len()
        );
        for (item, needed_by) in &missing {
            let note = if d.items.contains(*item) {
                ""
            } else {
                "  (NOT EVEN AN ITEM in items.csv)"
            };
            msg.push_str(&format!(
                "  {item}{note}\n    needed by: {}\n",
                needed_by.iter().copied().collect::<Vec<_>>().join(", ")
            ));
        }
        msg.push_str(
            "\nFix the DATA the realistic way: correct a typo to the item that really \
             exists, or add the missing real-world item plus a recipe (or a creature, \
             plant or vendor row) that produces it.\n",
        );
        panic!("{msg}");
    }
}

#[test]
fn every_recipe_is_reachable_from_base_sources() {
    let d = load();
    let mut have: BTreeSet<String> = d.base_sources.keys().cloned().collect();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    loop {
        let mut changed = false;
        for r in &d.recipes {
            if done.contains(r.id.as_str()) {
                continue;
            }
            if r.inputs.iter().all(|i| have.contains(i)) {
                done.insert(r.id.as_str());
                have.extend(r.outputs.iter().cloned());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let blocked: Vec<String> = d
        .recipes
        .iter()
        .filter(|r| !done.contains(r.id.as_str()))
        .map(|r| {
            let lacking: Vec<&str> = r
                .inputs
                .iter()
                .filter(|i| !have.contains(*i))
                .map(|s| s.as_str())
                .collect();
            format!(
                "  data/recipes.csv:{} {} lacks: {}",
                r.line,
                r.id,
                lacking.join(", ")
            )
        })
        .collect();
    assert!(
        blocked.is_empty(),
        "\n\n[FAIL] {} recipe(s) can never be crafted starting from what the world \
         hands out (harvests, seeds, spawned creatures, the vendor, asteroid ores, \
         starting items), even following every recipe chain. Either an input has no \
         source at all (see every_recipe_input_has_a_source) or the recipes form a \
         loop that only feeds itself.\n\n{}\n",
        blocked.len(),
        blocked.join("\n")
    );
}

/// Item ids NPC shops sell are real items (2026-09-26): the Farming Elder
/// sold `seed_bag_0`, and a dozen other shop lines named items that did not
/// exist. data/tech_tree.ron is left out on purpose: the game does not read it
/// yet and it names future items (antimatter cells, stone tools) that are
/// design intent, not stale references.
#[test]
fn npc_shops_sell_real_items() {
    let d = load();
    let mut missing = Vec::new();
    let npcs = strip_line_comments(&read("data/npcs.ron"));
    let mut rest = npcs.as_str();
    while let Some(k) = rest.find("shop_items:") {
        rest = &rest[k + "shop_items:".len()..];
        let Some(close) = rest.find(']') else { break };
        for (i, part) in rest[..close].split('"').enumerate() {
            if i % 2 == 1 && !d.items.contains(part) {
                missing.push(format!("npcs.ron shop item {part}"));
            }
        }
    }
    assert!(missing.is_empty(), "names no item in data/items.csv: {missing:#?}");
}

#[test]
fn recipe_ids_are_unique() {
    let d = load();
    let mut first: HashMap<&str, usize> = HashMap::new();
    let mut dups = Vec::new();
    for r in &d.recipes {
        if let Some(prev) = first.insert(r.id.as_str(), r.line) {
            dups.push(format!(
                "  {} defined at data/recipes.csv:{prev} AND data/recipes.csv:{}",
                r.id, r.line
            ));
        }
    }
    assert!(
        dups.is_empty(),
        "\n\n[FAIL] duplicate recipe ids. RecipeRegistry::from_csv keeps the LAST row, so \
         the other definition silently vanishes from the game. Keep one row per id.\n\n{}\n",
        dups.join("\n")
    );
}

/// The Crafting page sidebar (src/gui/pages/crafting.rs `category_matches`)
/// selects recipes whose `category` equals a sidebar entry, ignoring ASCII
/// case. A category with no entry is reachable only under "All"; an entry that
/// matches nothing is an empty list.
#[test]
fn every_recipe_category_has_exactly_one_sidebar_entry() {
    let d = load();
    let json = read("data/crafting/categories.json");
    let groups_at = json
        .find("\"groups\"")
        .expect("data/crafting/categories.json has no \"groups\" key");
    let groups = &json[groups_at..];
    let names = quoted_after_key(groups, "name");
    // Each group object: its "categories" list, in order.
    let mut entries: Vec<(String, String)> = Vec::new(); // (group, category)
    let mut rest = groups;
    let mut gi = 0;
    while let Some(k) = rest.find("\"categories\"") {
        let after = &rest[k..];
        let list = quoted_in_list_after(after, "\"categories\"");
        let group = names.get(gi).cloned().unwrap_or_else(|| format!("group #{gi}"));
        for c in list {
            entries.push((group.clone(), c));
        }
        gi += 1;
        rest = &after["\"categories\"".len()..];
    }
    assert!(
        !entries.is_empty(),
        "parsed no sidebar categories from data/crafting/categories.json"
    );

    let recipe_cats: BTreeMap<String, usize> =
        d.recipes.iter().fold(BTreeMap::new(), |mut m, r| {
            *m.entry(r.category.to_ascii_lowercase()).or_default() += 1;
            m
        });

    let mut problems = Vec::new();
    for (cat, n) in &recipe_cats {
        let hits: Vec<&(String, String)> = entries
            .iter()
            .filter(|(_, c)| c.eq_ignore_ascii_case(cat))
            .collect();
        match hits.len() {
            0 => problems.push(format!(
                "  recipe category `{cat}` ({n} recipes) has NO sidebar entry, so those \
                 recipes only show under All"
            )),
            1 => {}
            k => problems.push(format!(
                "  recipe category `{cat}` appears in {k} sidebar entries: {:?}",
                hits.iter().map(|(g, c)| format!("{g}/{c}")).collect::<Vec<_>>()
            )),
        }
    }
    for (group, c) in &entries {
        if !recipe_cats.contains_key(&c.to_ascii_lowercase()) {
            problems.push(format!(
                "  sidebar entry `{group}/{c}` matches no recipe category (an empty list)"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "\n\n[FAIL] data/crafting/categories.json does not match the recipe categories in \
         data/recipes.csv.\n\n{}\n",
        problems.join("\n")
    );
}

/// src/systems/farming/mod.rs plants from `seed_<plant>_0` and a survival
/// harvest gives two of it back, so a plant without that item can never be
/// planted and its harvest never reaches anyone.
#[test]
fn every_plant_has_a_seed_item() {
    let d = load();
    let missing: Vec<&str> = d
        .plant_ids
        .iter()
        .filter(|p| !d.items.contains(&format!("seed_{p}_0")))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "\n\n[FAIL] {} plant(s) in data/plants.csv have no `seed_<plant>_0` row in \
         data/items.csv, so they can never be planted:\n  {}\n\n\
         Add one per plant (keep the id even for cuttings, tubers or spawn, and name \
         the item honestly, e.g. \"Seed Potatoes\", \"Oyster Mushroom Spawn\").\n",
        missing.len(),
        missing.join(", ")
    );
}

/// data/world/showcase.ron `bed_crops` maps a machine TYPE to the plant the
/// perpetual showcase sows on it (src/engine/ipc.rs looks the key up by the
/// placed machine's type). A key naming no machine type in the
/// data/machines/home.ron catalog is silently never planted: "grain_tray" and
/// "potato_bed" sat there unplanted until 2026-09-25. A value naming no plant
/// is skipped the same way.
#[test]
fn showcase_bed_crops_name_real_machines_and_plants() {
    let d = load();
    let home = read("data/machines/home.ron");
    let start = home
        .find("catalog: {")
        .expect("data/machines/home.ron has no `catalog: {`");
    let end = home[start..]
        .find("instances:")
        .map(|e| start + e)
        .unwrap_or(home.len());
    // Catalog keys sit at exactly eight spaces: `        "potato_grow_bed": (`.
    let machine_types: BTreeSet<String> = home[start..end]
        .lines()
        .filter(|l| l.starts_with("        \"") && !l.starts_with("         "))
        .filter_map(|l| {
            let t = l.trim_start();
            let rest = t.strip_prefix('"')?;
            let (key, after) = rest.split_once('"')?;
            after.trim_start().starts_with(':').then(|| key.to_string())
        })
        .collect();
    assert!(
        machine_types.len() >= 10,
        "parsed only {} machine types from the home.ron catalog",
        machine_types.len()
    );

    let showcase = strip_line_comments(&read("data/world/showcase.ron"));
    let s = showcase
        .find("bed_crops:")
        .expect("data/world/showcase.ron has no bed_crops");
    let open = s + showcase[s..].find('{').expect("bed_crops has no `{`");
    let close = open + showcase[open..].find('}').expect("bed_crops has no `}`");
    let strings: Vec<&str> = showcase[open + 1..close]
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect();
    assert!(
        strings.len() >= 2 && strings.len() % 2 == 0,
        "could not parse bed_crops pairs: {strings:?}"
    );
    let plants: BTreeSet<&str> = d.plant_ids.iter().map(|s| s.as_str()).collect();
    let mut problems = Vec::new();
    for pair in strings.chunks(2) {
        let (machine, plant) = (pair[0], pair[1]);
        if !machine_types.contains(machine) {
            problems.push(format!(
                "  bed_crops key `{machine}` is not a machine type in the data/machines/home.ron catalog"
            ));
        }
        if !plants.contains(plant) {
            problems.push(format!(
                "  bed_crops `{machine}` -> `{plant}`: `{plant}` is not a plant in data/plants.csv"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "\n\n[FAIL] data/world/showcase.ron bed_crops entries that can never be planted:\n\n{}\n",
        problems.join("\n")
    );
}

/// ASTEROID_ORES is a copy; this keeps it honest against src/lib.rs.
#[test]
fn asteroid_ore_list_matches_src_lib() {
    let lib = read("src/lib.rs");
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut blocks = 0;
    let mut rest = lib.as_str();
    while let Some(k) = rest.find("AsteroidBody {") {
        blocks += 1;
        let body = &rest[k..];
        let end = body.find("position:").unwrap_or(body.len());
        let ores = &body[..end];
        if let Some(v) = ores.find("ores:") {
            for (i, s) in ores[v..].split('"').enumerate() {
                if i % 2 == 1 {
                    found.insert(s.to_string());
                }
            }
        }
        rest = &body["AsteroidBody {".len()..];
    }
    let listed: BTreeSet<String> = ASTEROID_ORES.iter().map(|s| s.to_string()).collect();
    assert!(blocks >= 3, "found only {blocks} `AsteroidBody {{` spawns in src/lib.rs");
    assert_eq!(
        listed, found,
        "\n\nASTEROID_ORES in tests/recipe_sources_lint.rs no longer matches the ores the \
         `AsteroidBody {{` spawns in src/lib.rs yield. Update the list.\n"
    );
}

// -------------------------------------------------------------------------
// The scanners must not be silent no-ops.
// -------------------------------------------------------------------------

#[test]
fn csv_splitter_handles_quotes_and_trims() {
    assert_eq!(
        split_csv_line(r#"a, "b,c" ,"say ""hi""",d"#),
        vec!["a", "b,c", "say \"hi\"", "d"]
    );
    assert_eq!(ingredient_ids("iron_ore_0:2| coal_0 :1||x"), vec!["iron_ore_0", "coal_0", "x"]);
}

#[test]
fn strip_comments_keeps_strings() {
    let ron = "(creature: \"a\", n: 1),\n// (creature: \"wolf\"),\n(creature: \"b//c\"), // tail";
    let got = quoted_after_key(&strip_line_comments(ron), "creature");
    assert_eq!(got, vec!["a", "b//c"]);
    // `id:` must not match inside `def_id:`.
    assert_eq!(quoted_after_key("(def_id: \"x\", id: \"y\")", "id"), vec!["y"]);
    // JSON keys.
    assert_eq!(quoted_after_key("{\"name\": \"Tools\"}", "name"), vec!["Tools"]);
    assert_eq!(
        quoted_in_list_after("starting_items: [(\"a\", 2), \"b\"],", "starting_items"),
        vec!["a", "b"]
    );
    assert!(quoted_in_list_after("starting_items: [],", "starting_items").is_empty());
}
