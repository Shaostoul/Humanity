//! From how_to/tests.rs: the how-to checker's tests that read the app's list of pages, run again
//! WITH it. The list is the GUI's (`gui::pages::how_to::app_pages`), which the relay's crate cannot
//! reach, so the relay's crate runs them without it (its page gate skipped, as the server build
//! always ran them) and these run the same tests with it, as `native` ran them until the crate split
//! (2026-10-31). Each twin is the test itself, called with the list handed over on its own thread.

use crate::how_to::tests as how;

/// Run `test` with the app's own list of pages, and take it back after (a panic included).
fn with_the_apps_pages(test: fn()) {
    struct TakeBack;
    impl Drop for TakeBack {
        fn drop(&mut self) {
            how::PAGES.with(|p| *p.borrow_mut() = None);
        }
    }
    how::PAGES.with(|p| *p.borrow_mut() = Some(crate::gui::pages::how_to::app_pages()));
    let _back = TakeBack;
    test();
}

#[test]
fn the_shipped_how_tos_load_and_pass_every_gate() {
    with_the_apps_pages(how::the_shipped_how_tos_load_and_pass_every_gate);
}

#[test]
fn every_gate_refuses_its_broken_fixture() {
    with_the_apps_pages(how::every_gate_refuses_its_broken_fixture);
}

#[test]
fn the_base_fixture_and_the_allowed_edits_pass() {
    with_the_apps_pages(how::the_base_fixture_and_the_allowed_edits_pass);
}

#[test]
fn short_never_hides_a_serious_or_lethal_note() {
    with_the_apps_pages(how::short_never_hides_a_serious_or_lethal_note);
}

#[test]
fn a_stale_review_hash_fails_on_a_broken_file() {
    with_the_apps_pages(how::a_stale_review_hash_fails_on_a_broken_file);
}

#[test]
fn a_run_survives_its_steps_being_reordered_and_edited() {
    with_the_apps_pages(how::a_run_survives_its_steps_being_reordered_and_edited);
}

#[test]
fn a_no_goes_to_its_problem_or_back_to_its_step() {
    with_the_apps_pages(how::a_no_goes_to_its_problem_or_back_to_its_step);
}

#[test]
fn search_finds_a_how_to_by_the_sign_of_its_problem() {
    with_the_apps_pages(how::search_finds_a_how_to_by_the_sign_of_its_problem);
}

#[test]
fn the_two_hashes_agree_on_every_shipped_how_to() {
    with_the_apps_pages(how::the_two_hashes_agree_on_every_shipped_how_to);
}
