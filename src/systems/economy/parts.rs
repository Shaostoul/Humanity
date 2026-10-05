//! What the trading post pays for a better grade, and the parts price that
//! limits it (BUG-146, 2026-10-05).
//!
//! THE LOOP. The trading post sells goods at 1.25 times their base value and
//! buys them back at 0.5 times (`TradeGoodsRegistry::vendor_sell_price` and
//! `vendor_buy_price`). Until this fix it paid a hand-made good's grade
//! multiple on the whole price (data/manufacturing.ron: good 1.5, excellent
//! 2.5, masterwork 5.0), so a skilled crafter could buy a hammer's parts at the
//! post for 16.50, make a masterwork hammer and sell it back for 35, as often as
//! they liked. BUG-145 had closed the same loop for standard goods only.
//!
//! THE RULE. A better grade fetches its multiple of the standard price, but the
//! post never pays more for a made good than the parts to make it would cost
//! there. At a grade whose multiple is `m` it pays the smaller of:
//!
//! ```text
//!   m x standard                         (the grade's own multiple)
//!   parts - (parts - standard) / m       (the parts price, less a shortfall)
//! ```
//!
//! `standard` is what the post pays for a standard one, and `parts` the least
//! its parts cost bought at the post, or made from goods bought there
//! (`parts_prices`). Made from bought parts and sold at standard grade, a good
//! comes back `parts - standard` short of what its parts cost. A better grade
//! shrinks that shortfall in proportion to its multiple: a good one comes back
//! two thirds of it short, an excellent one two fifths, a masterwork a fifth.
//! So buying the parts, making the good and selling it back never makes money,
//! however many times it is done, and wherever the parts cost more than a
//! standard one fetches, every grade fetches more than the grade below it and
//! none reaches the parts price. Where the parts cost far more than the good
//! sells for (most hand tools), the grade's own multiple is the smaller of the
//! two and is paid in full. Where they cost no more than a standard one fetches
//! (the BUG-145 limit, which the large backpack sits on), no grade fetches more
//! than standard. A good that no recipe makes from goods the post sells has no
//! parts price: nothing bought there can be turned into it, so there is no
//! loop to stop, and its grade is paid in full. Below standard nothing
//! changes: a poor good fetches 0.4 of the standard price and a defective one
//! is not bought.
//!
//! WHY THIS RULE, and not the two the bug itself suggested:
//! - A price that falls as the post's stock of a good grows (a demand curve)
//!   limits how much repeated selling makes, but the first sales still pay the
//!   full price. A masterwork vehicle's first sale paid about four times what
//!   its parts cost, so a demand curve turns an endless loop into a windfall
//!   at every restock, not into no loop.
//! - Paying the grade bonus on the labour share (the price less the parts)
//!   pays no bonus at all for most hand-made goods, because the post prices
//!   them below their parts (a hammer is worth 15 and its parts cost 16.50),
//!   and still loops for the vehicles, priced at up to twice their parts: five
//!   times a light mech's labour share is far more than its parts cost.
//!
//! Never bidding more for a thing than its own asking price for the parts is
//! what any dealer that both buys and sells has to keep to, or people are paid
//! to carry its goods round in a circle (the "no arbitrage" condition of market
//! making). The rule needs no memory of what the post holds, so nothing new is
//! saved per game. A demand curve could still sit on top of it later, and would
//! need it underneath: a curve's first sale must not loop either.
//!
//! The grade multiples are the data's (data/manufacturing.ron); the parts
//! prices are worked out once, when the data loads (`engine::registries`, via
//! `TradeGoodsRegistry::with_parts_prices`), from the same three files the game
//! trades and crafts by: data/trade_goods.ron (what the post sells and pays),
//! data/items.csv (which of those goods it stocks) and data/recipes.csv (what
//! makes what). The walk that finds the cheapest way to each item is the one
//! BUG-145's check uses (`cheapest_costs`).
//!
//! The check `no_grade_sells_back_for_more_than_its_parts_cost` leaves out the
//! tools a craft wears, a small further cost of every craft, so it is if
//! anything stricter than play.
//!
//! TAP WATER COSTS NOTHING (2026-10-05). A measure of tap water (the `tap`
//! items of data/containers/fluids.ron, a litre of Purified Water today) is
//! priced the way a craft really gets it, not at the post's 2 credits a litre:
//! a hand craft at home draws it from the home's tanks when the backpack has
//! none (`crafting::plan_inputs`, `fluids::draw_from_tanks`), and so does an
//! automated machine, and nobody pays credits for what the tanks hold. The
//! tanks are filled by the home's own well pump and rain catchment
//! (data/machines/home.ron) and by the air handlers' condensate; the power
//! that runs them comes from the home's panels or, past those, the ship's
//! reactor, which the fleet ledger meters as worth on its own balance
//! (data/ship/fleet_ledger.ron, 1.5 CR a kWh), never against the player's
//! credits, and the ledger lists no water at all. So the walk starts every
//! tap item at 0 (`cheapest_costs`), and a recipe that turns tap water into
//! something the post buys must cost more than it fetches through its other
//! inputs. Until this the walk priced tap water as bought, and three recipes
//! sold back for more than they cost: Harvest Honey made a jar of honey from
//! a litre of tap water, Culture Antibiotics five antibiotics from flour,
//! sugar and three litres, and Brew Healing Potion a medkit from wheat seed
//! and that honey (the review of BUG-146; data/recipes.csv says what the
//! first two became).

use std::collections::HashMap;

use super::TradeGoodsRegistry;
use crate::systems::crafting::{Recipe, RecipeRegistry};
use crate::systems::fluids::FluidTable;
use crate::systems::inventory::ItemRegistry;

/// The items a craft draws from the home's water tanks (data/containers/fluids.ron
/// `tap`), sorted: the walk counts each at nothing. See the module notes.
pub fn tap_water(fluids: &FluidTable) -> Vec<String> {
    let mut tap: Vec<String> = fluids.tap.keys().filter(|id| fluids.tap_litres(id).is_some()).cloned().collect();
    tap.sort();
    tap
}

/// Rounds of the cheapest walk before it gives up: costs that are still
/// falling by then mean a cycle of recipes makes goods from nothing.
const MAX_ROUNDS: usize = 100;

/// What the post charges for an item it stocks: a trade good that is also an
/// items.csv item, which is what its catalog offers (src/lib.rs builds the
/// catalog with the same filter). None when it does not stock it.
pub fn vendor_charge(items: &ItemRegistry, goods: &TradeGoodsRegistry, id: &str) -> Option<f64> {
    if items.items.contains_key(id) {
        goods.vendor_sell_price(id).map(|p| p as f64)
    } else {
        None
    }
}

/// The recipes that take inputs, sorted by id so a walk over them, and its
/// messages, are the same on every run. A recipe with no inputs is gathering,
/// not buying, and is left out.
pub fn recipe_book(recipes: &RecipeRegistry) -> Vec<&Recipe> {
    let mut book: Vec<&Recipe> = recipes.recipes.values().filter(|r| !r.inputs.is_empty()).collect();
    book.sort_by(|a, b| a.id.cmp(&b.id));
    book
}

/// What a recipe's inputs cost at `costs`; None when one has no cost.
pub fn inputs_cost(r: &Recipe, costs: &HashMap<String, f64>) -> Option<f64> {
    r.inputs.iter().map(|(id, q)| costs.get(id).map(|c| c * *q as f64)).sum()
}

/// The result of `cheapest_costs`.
#[derive(Debug, Clone, Default)]
pub struct Cheapest {
    /// The least credits it takes to get one of each item, by item id.
    pub costs: HashMap<String, f64>,
    /// False when the costs were still falling after `MAX_ROUNDS` rounds: a
    /// cycle of recipes makes goods from nothing, itself an endless loop.
    pub settled: bool,
}

/// The cheapest credits-to-item cost of every item a player can get for
/// credits (BUG-145): buy it from the post, or make it from cheaper inputs,
/// crediting what else the recipe makes at `credit(id)` each. `tap` is the
/// tap water a craft draws from the home's tanks (`tap_water`), which costs
/// nothing. Settles in a few rounds on sound data. Moved here from BUG-145's
/// test (2026-10-05) so the game's parts prices and the checks share one walk.
pub fn cheapest_costs(
    items: &ItemRegistry,
    goods: &TradeGoodsRegistry,
    recipes: &RecipeRegistry,
    tap: &[String],
    credit: &dyn Fn(&str) -> f64,
) -> Cheapest {
    let book = recipe_book(recipes);
    let mut costs: HashMap<String, f64> = goods
        .goods
        .keys()
        .filter_map(|id| vendor_charge(items, goods, id).map(|c| (id.clone(), c)))
        .collect();
    // Drawn from the tanks, not bought (the module notes).
    for id in tap {
        costs.insert(id.clone(), 0.0);
    }
    for _ in 0..MAX_ROUNDS {
        let mut changed = false;
        for r in &book {
            let Some(cost) = inputs_cost(r, &costs) else { continue };
            for (i, (out, q)) in r.outputs.iter().enumerate() {
                if *q == 0 {
                    continue;
                }
                let others: f64 = r
                    .outputs
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, (id, qq))| credit(id) * *qq as f64)
                    .sum();
                let unit = (cost - others) / *q as f64;
                if costs.get(out).map_or(true, |c| unit < c - 1e-9) {
                    costs.insert(out.clone(), unit);
                    changed = true;
                }
            }
        }
        if !changed {
            return Cheapest { costs, settled: true };
        }
    }
    Cheapest { costs, settled: false }
}

/// The parts price of every trade good some recipe makes from goods the post
/// sells: the least it takes to MAKE one, over every recipe that makes it, with
/// each input at its cheapest (bought, or made from goods bought) and what else
/// the recipe makes credited at the post's standard price. Buying the good
/// itself does not count: the post sells ungraded goods only, so a bought good
/// can never be sold back at a grade.
///
/// Where a recipe makes a graded good, it makes nothing else (true of every
/// recipe on the 2026-10-05 data), so crediting the rest at the standard price
/// is exact. A recipe that made two graded goods, or a graded good beside a
/// material, would want the other output at its graded price instead; the
/// whole-data check `no_grade_sells_back_for_more_than_its_parts_cost` credits
/// by-products at the most the post pays for them, so it would name any loop
/// such a recipe opened. `tap` is the tap water a craft draws from the home's
/// tanks (`tap_water`), at nothing.
pub fn parts_prices(
    items: &ItemRegistry,
    goods: &TradeGoodsRegistry,
    recipes: &RecipeRegistry,
    tap: &[String],
) -> HashMap<String, f64> {
    let standard = |id: &str| goods.vendor_buy_price(id).unwrap_or(0) as f64;
    let walk = cheapest_costs(items, goods, recipes, tap, &standard);
    if !walk.settled {
        log::warn!(
            "trade goods: the cheapest cost of some items never settled (a cycle of recipes in data/recipes.csv \
             makes goods from nothing); the trading post's parts prices may be too high"
        );
    }
    let mut parts: HashMap<String, f64> = HashMap::new();
    for r in recipe_book(recipes) {
        let Some(cost) = inputs_cost(r, &walk.costs) else { continue };
        for (i, (out, q)) in r.outputs.iter().enumerate() {
            if *q == 0 || goods.get(out).is_none() {
                continue;
            }
            let others: f64 = r
                .outputs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, (id, qq))| standard(id) * *qq as f64)
                .sum();
            let unit = (cost - others) / *q as f64;
            let slot = parts.entry(out.clone()).or_insert(unit);
            if unit < *slot {
                *slot = unit;
            }
        }
    }
    parts
}

/// What the post pays for one good at a grade whose price multiplier is
/// `multiplier` (data/manufacturing.ron), given what it pays for a standard
/// one and the good's parts price (`parts_prices`; None when nothing the post
/// sells can be made into it). See the module notes for the rule and why.
/// None when the grade is not bought at all (defective, multiplier 0).
///
/// Ungraded goods take multiplier 1 and so the standard price, exactly as
/// before; the trading post window shows these same numbers
/// (gui::pages::vendor), so what it lists is what a sale pays.
pub fn graded_pay(standard: i64, parts: Option<f64>, multiplier: f32) -> Option<i64> {
    if !(multiplier > 0.0) {
        return None;
    }
    let s = standard.max(0) as f64;
    let m = f64::from(multiplier);
    let pay = if m <= 1.0 {
        // Standard, ungraded and the grades below: the multiple, as before.
        s * m
    } else {
        match parts {
            // The grade's multiple, or the parts price less the standard
            // shortfall divided by the multiple, whichever is less.
            Some(p) if p > s => (s * m).min(p - (p - s) / m),
            // The parts already cost no more than a standard one fetches (the
            // BUG-145 limit): no room above the standard price.
            Some(_) => s,
            // Nothing the post sells makes it: no loop to stop.
            None => s * m,
        }
    };
    Some(pay.floor() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::crafting::quality::QualityLevels;
    use crate::systems::inventory::Inventory;

    fn read(rel: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join(rel)).unwrap()
    }

    /// The shipped items, recipes, grades and trade goods, each through the
    /// runtime's own loader, and the trade goods given their parts prices the
    /// way `engine::registries` does at load.
    fn shipped() -> (ItemRegistry, RecipeRegistry, TradeGoodsRegistry, QualityLevels) {
        let items = ItemRegistry::from_csv(&read("items.csv")).unwrap();
        let recipes = RecipeRegistry::from_csv(&read("recipes.csv")).unwrap();
        let goods = TradeGoodsRegistry::from_ron(&read("trade_goods.ron")).unwrap().with_parts_prices(&items, &recipes);
        let levels = QualityLevels::from_ron(&read("manufacturing.ron")).unwrap();
        (items, recipes, goods, levels)
    }

    /// The shipped tap water items (data/containers/fluids.ron), which a craft
    /// draws from the home's tanks.
    fn shipped_tap() -> Vec<String> {
        tap_water(&FluidTable::from_ron(&read(FluidTable::FILE)).unwrap())
    }

    /// The grade a hand craft gives an output (crafting::deliver_outputs):
    /// the crafter's grade for a durable good, none for anything else.
    fn grade_of(items: &ItemRegistry, id: &str, crafted: u8) -> u8 {
        if items.durability_for(id) > 0 {
            crafted
        } else {
            0
        }
    }

    /// The most the post pays for one `id` at any grade a craft can give it.
    fn best_pay(goods: &TradeGoodsRegistry, levels: &QualityLevels, items: &ItemRegistry, id: &str) -> f64 {
        (0..=levels.levels.len() as u8)
            .filter_map(|q| goods.vendor_buy_price_graded(id, grade_of(items, id, q), Some(levels)))
            .max()
            .unwrap_or(0) as f64
    }

    /// Sell everything one craft of `r` at grade `q` makes back to the post,
    /// through `vendor_sell`, the function the trading post window settles
    /// with. Returns the credits paid; what the post does not buy pays nothing.
    fn sell_back(goods: &TradeGoodsRegistry, levels: &QualityLevels, items: &ItemRegistry, r: &Recipe, q: u8) -> i64 {
        let mut credits = 0i64;
        for (id, n) in &r.outputs {
            if *n == 0 {
                continue;
            }
            let grade = grade_of(items, id, q);
            let mut inv = Inventory::new(4);
            inv.add_item_q(id, *n, *n, grade);
            let _ = super::super::vendor_sell(&mut inv, &mut credits, goods, id, *n, grade, Some(levels));
        }
        credits
    }

    /// BUG-146 (2026-10-05): at no grade does buying a recipe's parts at the
    /// trading post, making it and selling what it makes back there make money,
    /// however many times it is done. For every recipe with inputs and every
    /// grade (ungraded, then each grade in data/manufacturing.ron), a player
    /// with credits for 50 rounds buys the parts at the cheapest (bought, or
    /// made from goods bought: the BUG-145 walk), makes the recipe, sells all it
    /// makes back through `vendor_sell` (durable goods at the craft's grade, the
    /// rest ungraded, as a hand craft delivers them), and goes round again while
    /// they can pay. Their balance must never rise above what they started
    /// with, at any point, so stopping early cannot win either. What else a
    /// recipe makes is credited in the walk at the MOST the post pays for it at
    /// any grade, so a graded by-product could not hide a loop. Tap water is
    /// priced as a craft gets it, drawn from the home's tanks for nothing
    /// (`tap_water`, 2026-10-05).
    ///
    /// Seen red with `vendor_sell`'s body before the fix, verbatim (each
    /// grade's multiple on the whole price): 133 (recipe, grade) loops, 25 at
    /// Good, 43 at Excellent and 65 at Masterwork, among them all twelve
    /// vehicle recipes at Excellent and Masterwork and ten of them at Good:
    ///   craft_hammer at Masterwork: parts cost 16.50 at the cheapest, the post pays 35 for what it makes (18.50 ahead after one round, 925.00 after 50)
    ///   build_motorcycle_full at Good: parts cost 1560.79 at the cheapest, the post pays 1875 for what it makes (314.21 ahead after one round, 15710.42 after 50)
    ///   build_mech_light at Masterwork: parts cost 74960.55 at the cheapest, the post pays 300000 for what it makes (225039.45 ahead after one round, 11251972.50 after 50)
    ///
    /// Seen red again with tap water priced as a craft gets it, on the recipes
    /// before the review's fix (2026-10-05): 19 (recipe, grade) loops, 3 at
    /// ungraded, 2 at Defective, 2 at Poor, 3 at Standard, 3 at Good, 3 at
    /// Excellent and 3 at Masterwork, from three recipes:
    ///   cook_honey at ungraded: parts cost 0.00 at the cheapest, the post pays 2 for what it makes (2.00 ahead after one round, 100.00 after 50)
    ///   craft_antibiotics at Standard: parts cost 6.67 at the cheapest, the post pays 10 for what it makes (3.33 ahead after one round, 166.67 after 50)
    ///   craft_healing_potion at ungraded: parts cost 6.00 at the cheapest, the post pays 7 for what it makes (1.00 ahead after one round, 50.00 after 50)
    /// (the potion through honey made from tap water).
    #[test]
    fn no_grade_sells_back_for_more_than_its_parts_cost() {
        const ROUNDS: u32 = 50;
        let (items, recipes, goods, levels) = shipped();
        assert!(levels.levels.len() >= 6, "expected the six grades of data/manufacturing.ron, got {}", levels.levels.len());
        let best = |id: &str| best_pay(&goods, &levels, &items, id);
        let tap = shipped_tap();
        let walk = cheapest_costs(&items, &goods, &recipes, &tap, &best);
        assert!(walk.settled, "item costs never settle: a cycle of recipes makes goods from nothing");
        // Proof tap water is priced as a craft gets it: a litre of Purified
        // Water is a tap item, and the walk counts it at nothing, not the 2
        // credits the post charges.
        assert!(tap.iter().any(|id| id == "water_purified_0"), "fluids.ron no longer lists purified water as tap water: {tap:?}");
        for id in &tap {
            assert_eq!(walk.costs.get(id), Some(&0.0), "tap water {id} is drawn from the tanks for nothing");
        }

        let grades: Vec<u8> = (0..=levels.levels.len() as u8).collect();
        let grade_name = |q: u8| levels.name(q).unwrap_or("ungraded").to_string();
        let (mut checked, mut graded) = (0usize, 0usize);
        let mut loops: Vec<(String, u8, String)> = Vec::new();
        for r in recipe_book(&recipes) {
            let Some(cost) = inputs_cost(r, &walk.costs) else { continue };
            checked += 1;
            if r.outputs.iter().any(|(id, _)| items.durability_for(id) > 0 && goods.get(id).is_some()) {
                graded += 1;
            }
            for &q in &grades {
                let start = cost * f64::from(ROUNDS);
                let (mut balance, mut highest, mut rounds) = (start, start, 0u32);
                let mut paid = 0i64;
                while rounds < ROUNDS && balance >= cost {
                    balance -= cost;
                    paid = sell_back(&goods, &levels, &items, r, q);
                    balance += paid as f64;
                    rounds += 1;
                    highest = highest.max(balance);
                }
                if highest > start + 1e-6 {
                    loops.push((
                        r.id.clone(),
                        q,
                        format!(
                            "{} at {}: parts cost {cost:.2} at the cheapest, the post pays {paid} for what it makes \
                             ({:.2} ahead after one round, {:.2} after {rounds})",
                            r.id,
                            grade_name(q),
                            paid as f64 - cost,
                            highest - start
                        ),
                    ));
                }
            }
        }
        // Proof the walk reached the recipe book and the graded goods, so a
        // green run is not an empty one: 350 recipes have inputs the post
        // sells or can be made from them, and 78 of those make a durable good
        // the post buys (the 2026-10-05 data).
        assert!(checked >= 300, "only {checked} recipes have inputs the post sells or can be made from them");
        assert!(graded >= 70, "only {graded} of them make a graded good the post buys");
        if !loops.is_empty() {
            let per_grade: Vec<String> = grades
                .iter()
                .filter_map(|q| {
                    let n = loops.iter().filter(|(_, g, _)| g == q).count();
                    (n > 0).then(|| format!("{n} at {}", grade_name(*q)))
                })
                .collect();
            panic!(
                "{} (recipe, grade) rounds of buying the parts at the trading post, making the recipe and selling \
                 what it makes back come out ahead, an endless money loop (BUG-146): {}\n  {}",
                loops.len(),
                per_grade.join(", "),
                loops.iter().map(|(_, _, m)| m.as_str()).collect::<Vec<_>>().join("\n  ")
            );
        }
    }

    /// BUG-146 (2026-10-05): skill still pays at the trading post. A
    /// masterwork hammer sold through `vendor_sell` fetches more than a
    /// standard one. For every durable good the post buys, each grade from
    /// poor up fetches at least what the grade below it does; and wherever the
    /// parts leave room above the standard price (or no recipe makes the good
    /// from goods the post sells), a masterwork fetches more than a standard
    /// one. The rule could otherwise pass the loop check above by paying no
    /// premium at all.
    ///
    /// The loop check cannot fail on the old pricing here (a masterwork paid
    /// five times the standard price); this one was seen red on a variant of
    /// `graded_pay` that paid every grade above standard the standard price:
    ///   a masterwork Hammer fetches 7, no more than a standard one (7)
    #[test]
    fn a_masterwork_still_fetches_more_than_a_standard_good() {
        let (items, _recipes, goods, levels) = shipped();
        let standard = levels.standard();
        let top = levels.levels.len() as u8;
        assert_eq!(levels.name(top), Some("Masterwork"));
        let sell_one = |id: &str, q: u8| {
            let mut inv = Inventory::new(4);
            inv.add_item_q(id, 1, 1, q);
            let mut credits = 0i64;
            super::super::vendor_sell(&mut inv, &mut credits, &goods, id, 1, q, Some(&levels)).unwrap();
            credits
        };
        let (std_hammer, master_hammer) = (sell_one("hammer_0", standard), sell_one("hammer_0", top));
        assert!(
            master_hammer > std_hammer,
            "a masterwork Hammer fetches {master_hammer}, no more than a standard one ({std_hammer})"
        );

        let price = |id: &str, q: u8| goods.vendor_buy_price_graded(id, q, Some(&levels)).unwrap_or(0);
        let mut ids: Vec<&String> = goods.goods.keys().filter(|id| items.durability_for(id) > 0).collect();
        ids.sort();
        let (mut with_room, mut with_premium) = (0usize, 0usize);
        let mut wrong = Vec::new();
        for id in ids {
            // Poor (grade 2) up to masterwork: defective is not bought.
            let pays: Vec<i64> = (2..=top).map(|q| price(id, q)).collect();
            if pays.windows(2).any(|w| w[1] < w[0]) {
                wrong.push(format!("{id}: a better grade fetches less, poor to masterwork {pays:?}"));
            }
            let s = price(id, standard);
            let room = s >= 1 && goods.parts_price(id).map_or(true, |p| p - s as f64 >= 1.25);
            if room {
                with_room += 1;
                if price(id, top) <= s {
                    wrong.push(format!("{id}: a masterwork fetches {}, no more than a standard one ({s})", price(id, top)));
                }
            }
            if price(id, top) > s {
                with_premium += 1;
            }
        }
        assert!(wrong.is_empty(), "{} goods break the grade ladder:\n  {}", wrong.len(), wrong.join("\n  "));
        // Proof the ladder was walked over the real goods: 87 durable goods
        // are traded, and 80 of them leave room for a premium and fetch one
        // (the 2026-10-05 data). For the other 7 the parts cost under a
        // credit and a quarter more than a standard one fetches (the large
        // backpack's no more at all), or a standard one fetches nothing.
        assert!(with_room >= 75 && with_premium >= 75, "only {with_room} goods leave room, {with_premium} fetch a premium");
    }

    /// The game's own parts prices count tap water the way a craft gets it
    /// (2026-10-05, the review of BUG-146): the registry built the way
    /// `engine::registries` builds it (`with_parts_prices`, which reads the
    /// tap items from data/containers/fluids.ron itself) holds exactly the
    /// parts prices of the walk with tap water at nothing, and they are not
    /// the ones with tap water bought: a sterile bandage, made from cloth and
    /// a litre of tap water, costs its cloth alone. The loop check above prices
    /// its own walk, so without this the game's ceiling could drift from it:
    /// with the tap items left out of `with_parts_prices` the loop check stayed
    /// green, and this was red:
    ///   bandage_0: the game's parts price is not the walk's with tap water
    ///   free (left: Some(0.4), right: Some(0.25))
    #[test]
    fn the_games_parts_prices_count_tap_water_as_a_craft_gets_it() {
        let (items, recipes, goods, _levels) = shipped();
        let tap = shipped_tap();
        let free = parts_prices(&items, &goods, &recipes, &tap);
        let bought = parts_prices(&items, &goods, &recipes, &[]);
        let mut ids: Vec<&String> = free.keys().chain(goods.parts.keys()).collect();
        ids.sort();
        ids.dedup();
        for id in ids {
            assert_eq!(goods.parts_price(id), free.get(id).copied(), "{id}: the game's parts price is not the walk's with tap water free");
        }
        let (bandage_free, bandage_bought) = (free["bandage_0"], bought["bandage_0"]);
        assert!(
            bandage_free < bandage_bought,
            "a sterile bandage's parts cost {bandage_free} with tap water free, {bandage_bought} with it bought"
        );
    }

    /// The rule on hand numbers: a standard price of 7 and parts at 16.50
    /// (the hammer's, on the 2026-10-05 data).
    #[test]
    fn the_grade_rule_on_hand_numbers() {
        assert_eq!(graded_pay(7, Some(16.5), 1.0), Some(7), "standard: the standard price");
        assert_eq!(graded_pay(7, Some(16.5), 0.4), Some(2), "poor: 0.4 of it, as before");
        assert_eq!(graded_pay(7, Some(16.5), 0.0), None, "defective: not bought");
        // Good: min(10.5, 16.5 - 9.5 / 1.5 = 10.17).
        assert_eq!(graded_pay(7, Some(16.5), 1.5), Some(10));
        // Excellent: min(17.5, 16.5 - 9.5 / 2.5 = 12.7).
        assert_eq!(graded_pay(7, Some(16.5), 2.5), Some(12));
        // Masterwork: min(35, 16.5 - 9.5 / 5 = 14.6), never the 16.50 the parts cost.
        assert_eq!(graded_pay(7, Some(16.5), 5.0), Some(14));
        // Nothing the post sells makes it: the full multiple.
        assert_eq!(graded_pay(7, None, 5.0), Some(35));
        // Parts far dearer than the good (an anvil: 25 standard, 250 of parts): the full multiple.
        assert_eq!(graded_pay(25, Some(250.0), 5.0), Some(125));
        // Parts that cost what a standard one fetches (the BUG-145 limit): no room above it.
        assert_eq!(graded_pay(20, Some(20.0), 5.0), Some(20));
        // An ungraded good: the standard price, whatever its parts.
        assert_eq!(graded_pay(20, Some(20.0), 1.0), Some(20));
    }
}
