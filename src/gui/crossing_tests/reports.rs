//! From relay/handlers/reports_tests.rs (`a_message_carrying_a_file_is_never_included_in_a_report`):
//! the one line of it that read the app's side. The relay refuses a report carrying an encrypted
//! file by the marker's prefix (`DM_FILE_MARKER_PREFIX`, "[[hum:file:"); the marker the desktop app
//! seals files under (`net::dm_pq::FILE_MARKER`) must start with it, or a file the app sends would
//! slip past the rule. The app's network side and the server are crates beside each other, so this
//! line is the GUI's; the rest of that test, the server's own rule, stays in the relay's tests and
//! its battery.

use crate::relay::handlers::reports::DM_FILE_MARKER_PREFIX;

#[test]
fn the_apps_file_marker_starts_with_the_prefix_the_report_rule_refuses() {
    assert!(crate::net::dm_pq::FILE_MARKER.starts_with(DM_FILE_MARKER_PREFIX), "the marker the desktop app seals files under");
}
