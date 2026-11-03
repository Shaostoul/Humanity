//! From systems/ship_meals_tests.rs: the mess hall counter's rules are the server's (the ship's
//! stores and the fleet ledger).

use humanity_systems::ship_meals::MealRules;

/// THE BUILT-IN RULES ARE THE SERVER'S: the counter reads the same two files the relay does, and
/// both read them the same (the meal time from ship_stores.ron, the meal from the ledger's meal
/// kind), so a meal on this computer is the meal a server would serve.
///
/// Seen red 2026-10-12 with the meal kind looked up as "meals": "the counter reads the ledger's
/// meal: Err(\"ship/fleet_ledger.ron: no meal kind\")".
#[cfg(feature = "relay")]
#[test]
fn the_counters_rules_are_the_servers() {
    let stores = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/food/ship_stores.ron"));
    let ledger = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/ship/fleet_ledger.ron"));
    let rules = MealRules::parse(stores, ledger);
    assert!(rules.is_ok(), "the counter reads the ledger's meal: {rules:?}");
    let rules = rules.unwrap();
    let relay_stores = crate::relay::handlers::ship_stores::Provisions::parse(stores).expect("the relay reads it");
    let relay_ledger = crate::relay::handlers::fleet_ledger::LedgerData { file: crate::relay::handlers::fleet_ledger::LedgerFile::parse(ledger).unwrap(), goods: Default::default() };
    assert_eq!(rules.interval_s, relay_stores.meal_interval_hours * 3600.0);
    assert_eq!(Some(rules.item.as_str()), relay_ledger.meal_item());
    assert_eq!(MealRules::built_in(), rules);
    assert!(MealRules::parse("(meal_interval_hours: 0.0)", ledger).is_err(), "a meal time of 0 is refused");
}
