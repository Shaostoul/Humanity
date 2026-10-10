// Child of net/protected.rs (#[path]): the protected setup's rules with no app state and no
// screen (step G of docs/design/blocking-and-safe-mode.md, 10h's proof list). Each test was seen
// failing once on purpose, recorded at the test.

use super::*;

/// The preset built into the exe (the same bytes as data/gui/safety_presets.json).
fn preset() -> Preset {
    parse_presets(crate::embedded_data::SAFETY_PRESETS_JSON.as_bytes()).expect("the built-in presets parse")
}

/// A verifier at few iterations, so the tests of what is done WITH a verifier stay quick; one
/// test holds `PinVerifier::new` to the real 600,000.
fn quick(pin: &str) -> PinVerifier {
    PinVerifier::with_salt(pin, &[7u8; 16], 1_000)
}

/// THE PRESET IS THE REAL FILE (10h): the copy built into the exe is the file in data/, and it
/// reads as 10h says: Messages from friends, Calls from people I choose, Trades from friends,
/// warnings on friends' messages, pictures from non-friends never shown, only read-only rooms,
/// a PIN of 4 to 12 digits, a 60-second wait after 3 wrong ones, seven sentences in order. Its
/// `reach_set` is the ordinary frame with exactly those three rows. A data folder without the
/// file falls back to the built-in copy.
/// Seen red 2026-10-10 with `reach_set_frame` built from `ReachSettings::default()` instead of
/// the preset's rows after the call row was changed in a copy: "the preset's call row" failed.
#[test]
fn the_preset_is_read_from_the_real_file() {
    let on_disk = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join(PRESETS_FILE)).expect("data/gui/safety_presets.json");
    assert_eq!(crate::embedded_data::SAFETY_PRESETS_JSON.as_bytes(), on_disk.as_slice(), "the built-in copy is the file in data/");
    assert_eq!(crate::embedded_data::get_embedded(PRESETS_FILE), Some(crate::embedded_data::SAFETY_PRESETS_JSON), "and the lookup finds it");
    let p = preset();
    assert_eq!(p.id, "protected");
    assert_eq!(p.reach_settings(), Some(ReachSettings { message: Audience::Friends, call: Audience::Chosen, trade: Audience::Friends }), "10h's rows");
    assert_eq!(
        p.reach_set_frame(),
        Some(serde_json::json!({ "type": "reach_set", "settings": { "message": "friends", "call": "chosen", "trade": "friends" } })),
        "the ordinary reach_set, nothing added"
    );
    assert!(p.warnings_on_friends);
    assert_eq!((p.pictures_from_non_friends.as_str(), p.public_rooms.as_str()), (PICTURES_NEVER, ROOMS_READ_ONLY_ONLY));
    assert_eq!((p.pin_digits_min, p.pin_digits_max, p.pin_wrong_tries_before_wait, p.pin_wait_seconds), (4, 12, 3, 60));
    assert_eq!(p.sentences.len(), 7);
    assert_eq!(p.sentences[0], p.summary, "the reading step opens with the finding's sentence 1, the summary");
    assert_eq!(p.button, "Turn on the protected setup");
    assert_eq!(p.labels.pin_rule(4, 12), "Use 4 to 12 digits, the same both times.", "the shared labels, with their blanks filled");
    assert_eq!(p.labels.pin_wait(60), "Too many wrong tries. Try again in 60 seconds.");
    assert_eq!(p.picture_hidden_line, "A picture or file from someone who is not a friend is not shown.");

    let applied = ProtectedSetup::applied(&p, quick("1234"), "me", vec!["ann".into()]);
    assert!(applied.on && applied.warnings_on_friends && applied.pictures_hidden && applied.public_rooms_hidden, "the preset's rules turn on");
    let mut changed = serde_json::from_slice::<serde_json::Value>(&on_disk).unwrap();
    changed["presets"][0]["reach"]["call"] = serde_json::json!("anyone");
    let copy = parse_presets(changed.to_string().as_bytes()).unwrap();
    assert_eq!(copy.reach_set_frame().unwrap()["settings"]["call"], "anyone", "the preset's call row");

    let empty = std::env::temp_dir().join(format!("protected-no-data-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&empty);
    assert_eq!(load_preset(&empty).map(|p| p.id), Ok("protected".to_string()), "no file in the data folder: the built-in copy");
}

/// A file this version cannot use is refused, so the app never applies a guess: an unknown
/// audience word, an unknown pictures or rooms rule, no `protected` preset.
/// Seen red 2026-10-10 with the reach check taken out of `parse_presets`: "an unknown audience
/// word" parsed.
#[test]
fn a_preset_this_version_cannot_use_is_refused() {
    let file: serde_json::Value = serde_json::from_str(crate::embedded_data::SAFETY_PRESETS_JSON).unwrap();
    let with = |path: &[&str], value: serde_json::Value| {
        let mut f = file.clone();
        let mut at = &mut f["presets"][0];
        for p in &path[..path.len() - 1] {
            at = &mut at[*p];
        }
        at[path[path.len() - 1]] = value;
        parse_presets(f.to_string().as_bytes())
    };
    assert!(with(&["reach", "message"], serde_json::json!("everyone")).is_err(), "an unknown audience word");
    assert!(with(&["pictures_from_non_friends"], serde_json::json!("sometimes")).is_err(), "an unknown pictures rule");
    assert!(with(&["public_rooms"], serde_json::json!("some")).is_err(), "an unknown rooms rule");
    assert!(with(&["status_line"], serde_json::json!("  ")).is_err(), "an empty word");
    assert!(with(&["labels", "pin_wrong"], serde_json::json!("")).is_err(), "an empty label");
    let mut no_label = file.clone();
    no_label["presets"][0]["labels"].as_object_mut().unwrap().remove("not_connected");
    assert!(parse_presets(no_label.to_string().as_bytes()).is_err(), "a missing label");
    assert!(with(&["id"], serde_json::json!("other")).is_err(), "no protected preset");
}

/// THE PIN VERIFIER (10h): the right PIN opens it, a wrong one does not, the same PIN under
/// another salt gives another hash, and `new` makes a random 16-byte salt and a 32-byte hash at
/// the vault's 600,000 iterations. What is stored holds neither the PIN nor its digits.
/// Seen red 2026-10-10 with `matches` answering true without comparing the hashes: "\"482138\"
/// is refused" failed.
#[test]
fn the_pin_verifier_opens_with_the_right_pin_only() {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let v = quick("482139");
    assert!(v.matches("482139"), "the right PIN opens it");
    for wrong in ["482138", "48213", "4821390", "", "000000"] {
        assert!(!v.matches(wrong), "{wrong:?} is refused");
    }
    let other_salt = PinVerifier::with_salt("482139", &[8u8; 16], 1_000);
    assert_ne!(v.hash, other_salt.hash, "the salt changes the hash");

    let real = PinVerifier::new("907153").expect("a verifier");
    assert_eq!(real.iterations, 600_000, "the vault's 600,000 iterations");
    assert_eq!(PIN_ITERATIONS, crate::config::PBKDF2_ITERATIONS_NEW);
    assert_eq!(b64.decode(&real.salt).unwrap().len(), 16, "a 16-byte salt");
    assert_eq!(b64.decode(&real.hash).unwrap().len(), 32);
    assert_ne!(PinVerifier::new("907153").unwrap().salt, real.salt, "a fresh salt each time");
    assert!(real.matches("907153") && !real.matches("907154"));
    let stored = serde_json::to_string(&real).unwrap();
    assert!(!stored.contains("907153"), "the PIN is not stored: {stored}");
}

/// THE WAIT (10h): three wrong PINs in a row start a 60-second wait, during which not even the
/// right PIN is looked at; after it the right PIN opens, and a right PIN starts the count again.
/// Seen red 2026-10-10 with `try_pin` checking the PIN before the wait: "while waiting, the
/// right PIN is not looked at" got Right.
#[test]
fn three_wrong_pins_in_a_row_wait_sixty_seconds() {
    let p = preset();
    let mut s = ProtectedSetup::applied(&p, quick("2468"), "me", Vec::new());
    let (max, wait) = (p.pin_wrong_tries_before_wait, p.pin_wait_seconds);
    assert_eq!(s.try_pin("1111", 1_000, max, wait), PinTry::Wrong);
    assert_eq!(s.try_pin("2468", 1_001, max, wait), PinTry::Right, "a right PIN starts the count again");
    assert_eq!(s.try_pin("1111", 1_002, max, wait), PinTry::Wrong);
    assert_eq!(s.try_pin("2222", 1_003, max, wait), PinTry::Wrong);
    assert_eq!(s.try_pin("3333", 1_004, max, wait), PinTry::WaitStarted(60), "the third wrong one in a row waits 60 seconds");
    assert_eq!(s.wait_until, 1_064, "the wait is kept with the setup, so closing the app does not end it");
    assert_eq!(s.try_pin("2468", 1_005, max, wait), PinTry::Waiting(59), "while waiting, the right PIN is not looked at");
    assert_eq!(s.try_pin("2468", 1_063, max, wait), PinTry::Waiting(1));
    assert_eq!(s.try_pin("2468", 1_064, max, wait), PinTry::Right, "after the wait it opens");
    assert_eq!(ProtectedSetup::default().try_pin("2468", 0, max, wait), PinTry::NoPin, "off: nothing to try");
}

/// The PIN's shape (10h): 4 to 12 digits, digits only (ASCII: what a keypad types).
/// Seen red 2026-10-10 with `pin_digits_max` compared with `<`: "twelve digits" refused.
#[test]
fn a_pin_is_four_to_twelve_digits() {
    let p = preset();
    assert!(!p.pin_shape_ok("123"), "three digits");
    assert!(p.pin_shape_ok("1234"), "four digits");
    assert!(p.pin_shape_ok("123456789012"), "twelve digits");
    assert!(!p.pin_shape_ok("1234567890123"), "thirteen digits");
    assert!(!p.pin_shape_ok("12a4"), "a letter");
    assert!(!p.pin_shape_ok("12 34"), "a space");
    assert!(!p.pin_shape_ok("\u{0661}\u{0662}\u{0663}\u{0664}"), "digits of another script");
}

/// What needs the PIN (10h): every action on the locked list does, every action on the
/// never-locked list does not, and a typed `/redeem` is a locked action.
/// Seen red 2026-10-10 with `Unfollow` moved into the locked arm: "Unfollow never needs it".
#[test]
fn locked_and_never_locked_actions() {
    use ProtectedAction::*;
    let locked = [
        ReachRow(ReachKind::Message, Audience::Anyone),
        ReachRow(ReachKind::Call, Audience::Nobody),
        Tick("ben".into(), ReachKind::Call, true),
        Tick("ben".into(), ReachKind::Message, false),
        Follow("ben".into()),
        AcceptRequest("ben".into()),
        SendRequest("ben".into()),
        RedeemFriendCode("ABCD1234".into()),
        JoinGroup("ticket".into()),
        JoinVoice("lounge".into()),
        WarningsOff,
        ShowPictures,
        ShowPublicRooms,
        ChangePin,
        TurnOff,
        ShowRecoveryPhrase,
    ];
    for a in &locked {
        assert!(a.needs_pin(), "{a:?} needs the PIN");
    }
    for a in [Block("ben".into()), Report("ben".into()), Unfollow("ben".into()), LeaveGroup("g".into()), LeaveRoom("lounge".into())] {
        assert!(!a.needs_pin(), "{a:?} never needs it");
    }
    assert_eq!(typed_command("/redeem ABCD1234"), Some(RedeemFriendCode("ABCD1234".into())));
    assert_eq!(typed_command("  /REDEEM   AB12  "), Some(RedeemFriendCode("AB12".into())), "any case, any spacing");
    assert_eq!(typed_command("/friend-code"), None, "making a code is not making a friend");
    assert_eq!(typed_command("redeem this"), None, "an ordinary message");
}

/// THE CHANNEL FILTER (10h): with the setup on, only read-only rooms are listed; showing public
/// rooms with the PIN lists the rest; with it off everything is listed.
/// Seen red 2026-10-10 with `lists_room` answering true for every room: "a public room is not
/// listed" failed.
#[test]
fn only_read_only_rooms_are_listed_while_it_is_on() {
    let off = ProtectedSetup::default();
    assert!(off.lists_room(false) && off.lists_room(true), "off: every room");
    let mut on = ProtectedSetup::applied(&preset(), quick("1234"), "me", Vec::new());
    assert!(on.lists_room(true), "a read-only room is listed");
    assert!(!on.lists_room(false), "a public room is not listed");
    on.public_rooms_hidden = false; // shown with the PIN
    assert!(on.lists_room(false), "once shown, it is");
}

/// Pictures and files (10h): from someone who is not a friend, not shown while the setup's
/// pictures rule is on; a friend's always follow the click-to-load rule; off changes nothing.
/// Seen red 2026-10-10 with `hides_pictures` ignoring `from_friend`: "a friend's pictures still
/// follow the click-to-load rule" failed.
#[test]
fn a_non_friends_pictures_are_not_shown() {
    let mut on = ProtectedSetup::applied(&preset(), quick("1234"), "me", Vec::new());
    assert!(on.hides_pictures(false), "a non-friend's pictures are not shown");
    assert!(!on.hides_pictures(true), "a friend's pictures still follow the click-to-load rule");
    on.pictures_hidden = false; // turned down with the PIN
    assert!(!on.hides_pictures(false));
    assert!(!ProtectedSetup::default().hides_pictures(false), "off: the ordinary rule");
}

/// "Forgot the PIN?" (10h): the typed phrase matches when its words are the phrase's, in order,
/// whatever the case, the punctuation, or a numbered list; a wrong word, a missing word, the
/// words in another order or nothing at all do not.
/// Seen red 2026-10-10 with `phrase_words` keeping number-only words: "a numbered list" failed.
#[test]
fn the_phrase_check_compares_the_words() {
    let derived = crate::net::identity::mnemonic_from_seed(&[5u8; 32]).unwrap();
    let words: Vec<&str> = derived.split(' ').collect();
    assert!(phrase_matches(&derived, &derived), "the phrase itself");
    assert!(phrase_matches(&derived.to_uppercase().replace(' ', ",  "), &derived), "any case, any separators");
    let numbered: String = words.iter().enumerate().map(|(i, w)| format!("{}. {w}\n", i + 1)).collect();
    assert!(phrase_matches(&numbered, &derived), "a numbered list");
    let mut swapped = words.clone();
    swapped.swap(0, 1);
    assert!(!phrase_matches(&swapped.join(" "), &derived), "the words in another order");
    assert!(!phrase_matches(&words[..23].join(" "), &derived), "a word missing");
    let other = crate::net::identity::mnemonic_from_seed(&[6u8; 32]).unwrap();
    assert!(!phrase_matches(&other, &derived), "another identity's phrase");
    assert!(!phrase_matches("", &derived) && !phrase_matches(" 1. 2. ", &derived), "nothing typed");
}

/// The string literals in a Rust source file (between unescaped double quotes), outside line
/// comments. Enough for the files below, which hold no raw strings.
fn string_literals(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '\'' {
            // A char literal ('x' or '\x') is skipped whole; a lifetime ('a) is left alone.
            if chars.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 2;
            }
            i += 1;
        } else if c == '"' {
            let mut s = String::new();
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                if let Some(&ch) = chars.get(i) {
                    s.push(ch);
                }
                i += 1;
            }
            out.push(s);
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

/// THE WORDS TEST (10h, "Words"): every string this feature shows is held to the preset's
/// `avoid_words`: the preset's own lines, sentences and labels, and every string literal in the
/// feature's files (the section, the chat's lines, the engine, the rules), so a word typed
/// straight into one of them is caught too.
/// Seen red 2026-10-10 with `accept_label`'s "Accept" in safety_protected.rs changed to "Accept,
/// verified": it failed naming that file, the string and the word.
#[test]
fn every_string_the_feature_shows_avoids_the_words() {
    let p = preset();
    assert!(p.avoid_words.len() >= 20, "the finding's list");
    assert_eq!(p.shown_words().len(), 12 + p.sentences.len() + 24, "every line, sentence and label");
    let mut checked: Vec<(String, String)> = Vec::new();
    for w in p.shown_words() {
        checked.push(("the preset".into(), w.to_string()));
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in ["src/gui/pages/safety_protected.rs", "src/gui/pages/chat/protected.rs", "src/engine/protected.rs", "src/net/protected.rs"] {
        let src = std::fs::read_to_string(root.join(file)).expect(file);
        let literals = string_literals(&src);
        assert!(!literals.is_empty(), "{file}: no strings found, so the scan broke");
        checked.extend(literals.into_iter().map(|s| (file.to_string(), s)));
    }
    for (from, text) in &checked {
        if let Some(word) = avoided_word(text, &p.avoid_words) {
            panic!("{from}: {text:?} avoids {word:?} (10h, Words)");
        }
    }
    assert_eq!(avoided_word("This is Child-Proof", &p.avoid_words), Some("child-proof"), "the check itself, any case");
    let mut changed = p.clone();
    changed.labels.pin_wrong = "That PIN is not verified.".into();
    assert!(changed.shown_words().iter().any(|w| avoided_word(w, &p.avoid_words) == Some("verified")), "a label is held to the list too");
    assert!(string_literals("let a = \"x\\\"y\"; // \"not this\"").contains(&"x\"y".to_string()), "the scanner reads escapes");
}

/// The setup's saved form: absent in an older config it is off with nothing stored, and it
/// round-trips through JSON as itself.
/// Seen red 2026-10-10 with `from_stored` not honouring `"on": false`: "a setup that says it is
/// off is off" failed (it read back as on).
#[test]
fn the_saved_setup_reads_back() {
    let on = ProtectedSetup::applied(&preset(), quick("1234"), "me", vec!["ann".into()]);
    let json = serde_json::to_string(&on).unwrap();
    assert_eq!(serde_json::from_str::<ProtectedSetup>(&json).unwrap(), on);
    let off = serde_json::to_value(ProtectedSetup::default()).unwrap();
    assert_eq!(ProtectedSetup::from_stored(off), ProtectedSetup::default(), "a setup that says it is off is off");
    assert_eq!(ProtectedSetup::from_stored(serde_json::Value::Null), ProtectedSetup::default(), "nothing stored: off");
}

/// FAIL CLOSED (as on the web chat): a stored setup that cannot be read counts as ON with no PIN
/// that matches and the safe rules, so a damaged config.json is never a way to turn it off; a
/// verifier that is not one this app makes counts as no PIN; every PIN is then wrong (and counts
/// toward the wait), and only the recovery phrase can set a new one. Through config.rs's own
/// reader, so a damaged field does not make the whole config unreadable (which reads as off).
/// Seen red 2026-10-10 with `from_stored` returning `Self::default()` when the value did not
/// parse: "a damaged setup counts as on" failed.
#[test]
fn a_damaged_stored_setup_fails_closed() {
    let p = preset();
    for damaged in [serde_json::json!("garbage"), serde_json::json!({ "on": "yes" }), serde_json::json!({ "pin": 5 }), serde_json::json!({})] {
        let s = ProtectedSetup::from_stored(damaged.clone());
        assert!(s.on && s.pin.is_none(), "a damaged setup counts as on, with no PIN: {damaged}");
        assert!(s.pictures_hidden && s.public_rooms_hidden && s.warnings_on_friends, "with the safe rules: {damaged}");
    }
    let mut weak = serde_json::to_value(ProtectedSetup::applied(&p, quick("1234"), "me", Vec::new())).unwrap();
    assert!(ProtectedSetup::from_stored(weak.clone()).pin.is_none(), "a verifier at 1,000 iterations is not one this app makes");
    weak["pin"]["iterations"] = serde_json::json!(PIN_ITERATIONS);
    weak["pin"]["salt"] = serde_json::json!("not base64!");
    assert!(ProtectedSetup::from_stored(weak).pin.is_none(), "nor is one with a salt that does not read");

    let mut locked = ProtectedSetup::from_stored(serde_json::json!("garbage"));
    for (i, pin) in ["1234", "0000"].iter().enumerate() {
        assert_eq!(locked.try_pin(pin, 10 + i as u64, 3, 60), PinTry::Wrong, "no PIN opens it");
    }
    assert_eq!(locked.try_pin("", 12, 3, 60), PinTry::WaitStarted(60), "and wrong tries still count toward the wait");

    let config: crate::config::AppConfig = serde_json::from_str(r#"{"protected_setup":"garbage","user_name":"Ann"}"#).expect("the rest of the config still reads");
    assert!(config.protected_setup.on && config.user_name == "Ann", "through config.rs's reader");
    let fresh: crate::config::AppConfig = serde_json::from_str(r#"{"user_name":"Ann"}"#).unwrap();
    assert!(!fresh.protected_setup.on, "a config from before the setup existed: off");
}

/// THE APPROVED LIST (as on the web chat): while it is on, a pass may go only to a friend the PIN
/// holder let be one; off, to anyone. A befriending action names its person; nothing else does.
/// Seen red 2026-10-10 with `pass_allowed` ignoring `on`: "off: anyone" failed.
#[test]
fn passes_go_only_to_approved_friends_while_it_is_on() {
    let on = ProtectedSetup::applied(&preset(), quick("1234"), "me", vec!["Ann".into()]);
    assert!(on.pass_allowed("ann") && on.pass_allowed("ANN"), "an approved friend, whatever the case");
    assert!(!on.pass_allowed("ben"), "anyone else needs the PIN first");
    assert!(ProtectedSetup::default().pass_allowed("ben"), "off: anyone");
    assert_eq!(ProtectedAction::Follow("ben".into()).befriends(), Some("ben"));
    assert_eq!(ProtectedAction::AcceptRequest("ben".into()).befriends(), Some("ben"));
    assert_eq!(ProtectedAction::SendRequest("ben".into()).befriends(), Some("ben"));
    assert_eq!(ProtectedAction::JoinGroup("t".into()).befriends(), None);
    assert_eq!(ProtectedAction::RedeemFriendCode("c".into()).befriends(), None, "a code names no one yet");
}

/// NEVER ASKS THE OPERATING SYSTEM FOR AN AGE (10h; California Civil Code 1798.501(e), see
/// docs/reference/findings/2026-10-10-california-ab-1043-age-signals.md): no file under src/
/// names Windows' age-range call or Apple's declared-age-range interface. The names are built
/// from parts here, so this file does not match itself.
/// Seen red 2026-10-10 on its first run, against this file's own doc comment, which then named
/// Apple's interface in full.
#[test]
fn nothing_asks_the_operating_system_for_an_age() {
    let needles = [["GetUserAge", "Range"].concat(), ["Declared", "AgeRange"].concat(), ["Declared", "Age", "Range"].join(" ")];
    let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut seen = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src/ reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                seen += 1;
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                for needle in &needles {
                    assert!(!text.contains(needle.as_str()), "{} names {needle:?}", path.display());
                }
            }
        }
    }
    assert!(seen > 100, "the walk found only {seen} files");
}
