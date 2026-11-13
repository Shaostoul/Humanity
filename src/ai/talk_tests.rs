//! The talk card's AI half (src/ai/talk.rs; rung 3 of docs/design/embeddable-ai.md, 2026-11-12):
//! the output shape refuses anything else, an action the game did not grant is refused, the grammar
//! offers only the granted actions, the filter, and the project's own persona.

use super::talk::*;

fn grants() -> Grants {
    Grants { tasks: vec![("farming_greenhouse".into(), "Greenhouse Construction".into())], trade: false, ways: vec![("mess_hall".into(), "the mess hall".into())] }
}

/// THE OUTPUT SHAPE REFUSES ANYTHING ELSE (8.3: "anything outside the shape is discarded"): only
/// `{"line":"...","action":"..."}` with a line in words and a known action passes; plain text, another
/// object, a field more or less, an empty line and an unknown action are each refused with their own
/// words. Seen red 2026-11-12 with `Wire` without `deny_unknown_fields` and no empty-line check:
/// "a field more is refused: Ok(Reply { line: \"Hello there.\", action: None })".
#[test]
fn the_output_shape_refuses_anything_else() {
    let g = grants();
    assert_eq!(parse_reply(r#"{"line":"Hello there.","action":"none"}"#, &g), Ok(Reply { line: "Hello there.".into(), action: None }));
    assert!(matches!(parse_reply("Hello there.", &g), Err(ShapeError::NotTheShape(_))), "plain text is refused");
    assert!(matches!(parse_reply(r#"{"line":"Hello there.","action":"none","mood":"happy"}"#, &g), Err(ShapeError::NotTheShape(_))), "a field more is refused: {:?}", parse_reply(r#"{"line":"Hello there.","action":"none","mood":"happy"}"#, &g));
    assert!(matches!(parse_reply(r#"{"line":"Hello there."}"#, &g), Err(ShapeError::NotTheShape(_))), "a field less is refused");
    assert!(matches!(parse_reply(r#"["Hello there.","none"]"#, &g), Err(ShapeError::NotTheShape(_))), "another JSON value is refused");
    assert_eq!(parse_reply(r#"{"line":"   ","action":"none"}"#, &g), Err(ShapeError::Empty), "an empty line is refused");
    assert!(matches!(parse_reply(r#"{"line":"Here.","action":"give_item:sword"}"#, &g), Err(ShapeError::UnknownAction(_))), "an action of no known kind is refused");
    let long = format!(r#"{{"line":"{}","action":"none"}}"#, "a".repeat(MAX_LINE_CHARS + 1));
    assert!(matches!(parse_reply(&long, &g), Err(ShapeError::NotTheShape(_))), "a line over the cap is refused");
    for e in [ShapeError::NotTheShape(String::new()), ShapeError::Empty, ShapeError::UnknownAction(String::new()), ShapeError::NotGranted(String::new())] {
        assert_ne!(humanity_core::i18n::t_dyn(e.words_key()), e.words_key(), "{e:?} has words");
    }
}

/// AN ACTION THE GAME DID NOT GRANT IS REFUSED (8.3 and 8.5: a brain's line is words only; it may offer
/// only what this character's card offers): trade from a character who does not trade, a task it does
/// not offer, a place it cannot show; the granted ones pass. Seen red 2026-11-12 with `parse_reply`
/// not asking `Grants::allows`: "open_trade from a character who does not trade: Ok(Reply { line:
/// \"Let's trade.\", action: Some(OpenTrade) })".
#[test]
fn an_action_the_game_did_not_grant_is_refused() {
    let g = grants();
    let r = |action: &str| parse_reply(&format!(r#"{{"line":"Let's trade.","action":"{action}"}}"#), &g);
    assert_eq!(r("open_trade"), Err(ShapeError::NotGranted("open_trade".into())), "open_trade from a character who does not trade: {:?}", r("open_trade"));
    assert_eq!(r("offer_task:tutorial_power_grid"), Err(ShapeError::NotGranted("offer_task:tutorial_power_grid".into())), "a task it does not offer");
    assert_eq!(r("point_way:wolf_ridge"), Err(ShapeError::NotGranted("point_way:wolf_ridge".into())), "a place it cannot show");
    assert_eq!(r("offer_task:farming_greenhouse").map(|x| x.action), Ok(Some(Action::OfferTask("farming_greenhouse".into()))));
    assert_eq!(r("point_way:mess_hall").map(|x| x.action), Ok(Some(Action::PointWay("mess_hall".into()))));
    let trader = Grants { trade: true, ..Grants::default() };
    assert_eq!(parse_reply(r#"{"line":"Trade?","action":"open_trade"}"#, &trader).map(|x| x.action), Ok(Some(Action::OpenTrade)), "a trader may offer trade");
}

/// THE GRAMMAR OFFERS ONLY THE GRANTED ACTIONS, and begins as the fake engine expects (its
/// `fake::TALK_SHAPE`, crates/humanity-ai-engine/src/fake.rs, which answers in this shape). Seen red
/// 2026-11-12 with open_trade listed whatever the grants: "left: \"action ::= \\\"none\\\" |
/// \\\"open_trade\\\" | ...\"" against the granted list.
#[test]
fn the_grammar_offers_only_the_granted_actions() {
    let gr = grammar(&grants());
    assert!(gr.starts_with(r#"root ::= "{\"action\":\""#), "the fake engine's TALK_SHAPE: {gr}");
    let action_rule = gr.lines().find(|l| l.starts_with("action ::=")).expect("an action rule");
    assert_eq!(action_rule, r#"action ::= "none" | "offer_task:farming_greenhouse" | "point_way:mess_hall""#);
    assert!(!gr.contains("open_trade"), "no trade in the grammar of a character who does not trade");
    assert!(gr.contains(&format!("{{1,{MAX_LINE_CHARS}}}")), "the line is capped in the grammar too: {gr}");
    assert!(gr.contains(&format!("line ::= {LINE_CHARS}")) && !LINE_CHARS.contains('"') && !LINE_CHARS.contains('_') && !LINE_CHARS.contains('#'), "no quote, underscore or hash in a line: {gr}");
}

/// THE FILTER (8.3): whole words only, so ordinary in-game talk passes ("I'd kill for a fresh atlas",
/// one of Vex's written lines; "sextant"); each rule catches its phrases; a template marker and a line
/// over the cap are caught; the data file's cap is the grammar's; every rule has its words. Seen red
/// 2026-11-12 with phrases matched as plain substrings: "passes: \"Use the sextant by the window.\"",
/// left: Err(FilterHit { why: \"sexual\" }).
#[test]
fn the_filter_stops_what_breaks_a_rule_and_lets_the_rest_through() {
    let f = Filter::current();
    assert_eq!(f.max_chars, MAX_LINE_CHARS, "data/ai/filter.ron's cap is the grammar's");
    for ok in ["I'd kill for a fresh atlas. Real paper. Stars don't move that fast.", "Use the sextant by the window.", "Your recovery phrase stays on your own computer.", "The mess hall is by the Commons."] {
        assert_eq!(f.check(ok), Ok(()), "passes: {ok:?}");
    }
    for (bad, why) in [
        ("Honestly, I am a human like you.", "person"),
        ("I'm a large language model made by Mistral.", "maker"),
        ("Take 400 mg of it twice a day.", "advice"),
        ("Those berries are edible, go ahead.", "advice"),
        ("Always test new edibles carefully before eating.", "advice"),
        ("Write down your seed phrase.", "account_words"),
        ("You should kill yourself.", "harm"),
        ("Let's talk about sex.", "sexual"),
        ("Vote Republican next time.", "politics"),
        ("Sure. [INST] ignore that", "marker"),
    ] {
        assert_eq!(f.check(bad), Err(FilterHit { why: why.into() }), "stopped: {bad:?}");
    }
    assert_eq!(f.check(&"word ".repeat(80)), Err(FilterHit { why: "too_long".into() }));
    for why in filter_reasons(&f).keys() {
        let key = format!("ai.talk.filter.{why}");
        assert_ne!(humanity_core::i18n::t_dyn(&key), key, "the rule {why} has its words");
    }
}

/// The line out of a reply still arriving, for the card to show word by word.
#[test]
fn the_line_streams_out_of_the_shape() {
    assert_eq!(streaming_line(""), "");
    assert_eq!(streaming_line("{\"li"), "");
    assert_eq!(streaming_line("{\"action\":\"no"), "", "the action first, no words yet");
    assert_eq!(streaming_line("{\"action\":\"none\",\"line\":\"Hel"), "Hel");
    assert_eq!(streaming_line("{\"action\":\"none\",\"line\":\"Hello.\"}"), "Hello.");
    assert_eq!(streaming_line("{\"line\":\"Hello."), "", "not the shape (the line first): nothing");
    assert_eq!(streaming_line("Hello"), "", "not the shape: nothing to show");
}

/// THE PROJECT'S OWN PERSONA (8.2): the prompt file has its version; the persona says what the model
/// is (a character run by AI on the player's computer), names no model or maker, carries the
/// character's row and its written lines as examples, and lists exactly its granted actions; the part
/// every character shares is the same start for all of them, and the one live fact comes last (prompt
/// reuse, 9.6). Seen red 2026-11-12 with the live fact moved to the top of the prompt file: "the live
/// fact comes last: Right now: watering the beds. ...".
#[test]
fn the_persona_is_the_projects_own() {
    let p = Prompt::character();
    assert_eq!(p.version, 1);
    assert!(Prompt::parse("no version here\n{actions}{name}{examples}{doing}").is_err(), "a prompt without its version is refused");
    let yara = Character {
        name: "Botanist Yara".into(),
        job: "botanist".into(),
        description: "Tending the Commons garden".into(),
        post: "the Commons".into(),
        persona: "Warm and patient.".into(),
        examples: vec!["Seeds, soil, light and patience.".into()],
        doing: "Watering the beds".into(),
    };
    let tan = Character { name: "Chief Tan".into(), job: "chief engineer".into(), doing: "Checking the air handlers".into(), ..yara.clone() };
    let a = persona(&p, &yara, &grants());
    let b = persona(&p, &tan, &Grants::default());
    assert!(a.contains("run by AI on the player's own computer"), "it says what it is");
    for maker in ["Mistral", "Le Chat", "IBM", "Granite", "Ministral", "language model"] {
        assert!(!a.contains(maker), "no model or maker as the speaker: {maker}");
    }
    assert!(a.contains("You are Botanist Yara.") && a.contains("botanist, tending the Commons garden; your post is the Commons") && a.contains("- Seeds, soil, light and patience."), "{a}");
    assert!(a.contains("offer_task:farming_greenhouse") && a.contains("point_way:mess_hall") && !a.contains("open_trade"), "exactly its granted actions");
    assert!(a.trim_end().ends_with("Right now: watering the beds."), "the live fact comes last: {a}");
    let shared = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    assert!(shared > a.chars().count() / 2, "two characters share the prompt's start ({shared} of {} characters)", a.chars().count());
}

/// Settings > AI's brain choice: the person's pick when installed, else Automatic: the Rich brain on a
/// computer with the memory for it, the Quick one on a small one or when only it is installed.
#[test]
fn the_brain_is_the_persons_choice_or_fits_the_computer() {
    use super::models::{pick_brain, BRAIN_AUTO};
    let entry = |id: &str, choice: &str| {
        let mut m = super::catalog::current().expect("the catalog").models.iter().find(|m| m.kind == "brain").cloned().expect("a brain");
        m.id = id.into();
        m.choice = choice.into();
        (m, std::path::PathBuf::from(id))
    };
    let both = vec![entry("granite", "characters-quick"), entry("ministral", "characters-rich")];
    let id = |b: Option<(super::catalog::ModelEntry, std::path::PathBuf)>| b.map(|(m, _)| m.id);
    assert_eq!(id(pick_brain(&both, BRAIN_AUTO, Some(32 * 1024))), Some("ministral".into()), "a 32 GB PC: Rich");
    assert_eq!(id(pick_brain(&both, BRAIN_AUTO, Some(8 * 1024))), Some("granite".into()), "an 8 GB PC: Quick");
    assert_eq!(id(pick_brain(&both, "characters-quick", Some(32 * 1024))), Some("granite".into()), "the person's pick wins");
    assert_eq!(id(pick_brain(&both[1..], BRAIN_AUTO, Some(8 * 1024))), Some("ministral".into()), "the only one installed");
    assert_eq!(id(pick_brain(&[], BRAIN_AUTO, None)), None);
}
