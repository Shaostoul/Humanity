//! The Donate page's funding: what the connected server's `funding` sets on `GuiState`, and how
//! its addresses are read (`DonateAddress::from_funding_json`). Moved out of gui/mod.rs as it
//! stood (2026-10-12), so that file stays under its line budget (tests/file_size_ratchet.rs).

use super::{DonateAddress, GuiState};

#[test]
fn funding_result_from_the_connected_server_lands_and_absent_funding_clears() {
    let mut state = GuiState::default();
    state.server_url = "https://united-humanity.us/".to_string();
    let funding = serde_json::json!({
        "goal_usd": 100000.0, "goal_label": "Full-time development",
        "addresses": [{"network": "GitHub Sponsors", "type": "url",
                       "value": "https://github.com/sponsors/Shaostoul", "label": "x"}]
    });
    state.apply_server_funding("https://united-humanity.us", Some(&funding));
    assert_eq!(state.donate_addresses_server.len(), 1);
    assert_eq!(state.donate_funding_goal, Some((100000.0, "Full-time development".to_string())));

    // Reconnect-to-a-server-without-funding: the SAME url reporting no funding
    // must CLEAR the fields, not leave the old list showing (the adversarial
    // review's major finding: stale addresses = money to the wrong operator).
    state.apply_server_funding("https://united-humanity.us", None);
    assert!(state.donate_addresses_server.is_empty(), "absent funding clears the list");
    assert!(state.donate_funding_goal.is_none(), "absent funding clears the goal");
}

#[test]
fn late_result_from_a_previous_server_is_discarded() {
    let mut state = GuiState::default();
    state.server_url = "https://server-b.example".to_string();
    let funding_a = serde_json::json!({
        "addresses": [{"network": "Bitcoin (BTC)", "type": "address", "value": "bc1qA", "label": ""}]
    });
    // A stale fetch tagged with server A's url lands while connected to B.
    state.apply_server_funding("https://server-a.example", Some(&funding_a));
    assert!(state.donate_addresses_server.is_empty(), "server A's addresses must not display on server B");
    assert!(state.donate_funding_server.is_empty());
}

#[test]
fn parses_filled_entries_and_skips_blank_placeholders() {
    // Mirrors the real shipped server-config.json shape: GitHub Sponsors is
    // filled in, the crypto rows are blank placeholders awaiting the operator.
    let funding = serde_json::json!({
        "enabled": true,
        "goal_usd": 100000,
        "addresses": [
            {"network": "GitHub Sponsors", "type": "url",
             "value": "https://github.com/sponsors/Shaostoul", "label": "Recurring or one-time"},
            {"network": "Solana (SOL)", "type": "address", "value": "", "label": "Send SOL"},
            {"network": "Bitcoin (BTC)", "type": "address", "value": "", "label": "Send BTC"},
        ]
    });
    let parsed = DonateAddress::from_funding_json(&funding);
    assert_eq!(parsed.len(), 1, "blank-value placeholder rows are dropped");
    assert_eq!(parsed[0].network, "GitHub Sponsors");
    assert_eq!(parsed[0].addr_type, "url");
    assert_eq!(parsed[0].value, "https://github.com/sponsors/Shaostoul");
    assert_eq!(parsed[0].label, "Recurring or one-time");
}

#[test]
fn missing_or_malformed_addresses_yield_empty_not_panic() {
    assert!(DonateAddress::from_funding_json(&serde_json::json!({"enabled": true})).is_empty());
    assert!(DonateAddress::from_funding_json(&serde_json::json!({"addresses": "oops"})).is_empty());
    assert!(DonateAddress::from_funding_json(&serde_json::json!(null)).is_empty());
}

#[test]
fn missing_type_defaults_to_address() {
    let funding = serde_json::json!({"addresses": [{"network": "X", "value": "abc123"}]});
    let parsed = DonateAddress::from_funding_json(&funding);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].addr_type, "address");
    assert_eq!(parsed[0].label, "");
}
