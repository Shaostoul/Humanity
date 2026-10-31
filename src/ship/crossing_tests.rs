//! TESTS OF THE CORE, TOGETHER WITH THE SHIP (the crate split, 2026-10-31,
//! docs/design/crate-split.md): written beside the core's code, they reach into this crate to do it,
//! so each moved up WHOLE to the lowest crate that may use everything it reaches. Nothing in a test
//! changed but its paths. From storage.rs: a fresh install writes its own home without the shipped
//! header.

use crate::storage::extract_embedded_to;
use crate::storage::tests::tmp;

/// A FRESH INSTALL'S OWN HOME does not claim to be the shipped default (the final review of
/// ship homes increment 2). The first run writes data/homes/homestead.ron, the
/// player's own home that every editor Save rewrites, from the copy built into the exe. That
/// copy is the shipped default (data/homes/shipped/), whose header says it is "never written
/// by the game", and `HomeDesign::save` keeps a file's leading comment block, so every
/// installed player's own home said so forever. Now the extracted file is the shipped design
/// under the own-home header, before a Save and after one. Seen red 2026-10-04 with
/// `get_embedded` handing out the shipped text as it is: "the extracted own home claims to be
/// the shipped default: Some(\"// THE SHIPPED DEFAULT of the homestead (ship homes increment
/// 2 review, finding 5). Compiled\")".
#[test]
fn a_fresh_install_writes_its_own_home_without_the_shipped_header() {
    use crate::ship::ship_structure::HomeDesign;
    let data = tmp("own_home");
    extract_embedded_to(&data);
    let path = data.join("homes").join("homestead.ron");
    // What the file's leading comment block (the part a Save keeps) says.
    let claims_shipped = |text: &str| {
        text.lines()
            .take_while(|l| l.trim_start().starts_with("//") || l.trim().is_empty())
            .any(|l| l.contains("SHIPPED DEFAULT") || l.contains("never written by the game"))
    };
    let text = std::fs::read_to_string(&path).expect("the first run writes the own home");
    assert!(!claims_shipped(&text), "the extracted own home claims to be the shipped default: {:?}", text.lines().next());
    // It is the shipped design all the same, and loads as the player's own home.
    let own = HomeDesign::load(&data, "homestead").expect("the own home loads");
    let shipped = HomeDesign::built_in("homestead").expect("the homestead is built in");
    assert_eq!(ron::ser::to_string(&own).unwrap(), ron::ser::to_string(&shipped).unwrap(), "the own home starts as the shipped design");
    // An editor Save keeps the header it found, and that header still does not claim it.
    own.save(&path).expect("a Save writes the home");
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!claims_shipped(&saved), "after a Save the own home claims to be the shipped default: {:?}", saved.lines().next());
    assert!(saved.starts_with("// HumanityOS home design"), "a Save keeps the own-home header: {:?}", saved.lines().next());
}
