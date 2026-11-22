//! The funding figures through the relay (2026-11-22; storage/funding_figures.rs holds the shape,
//! docs/design/live-funding.md the plan): who may change them, what is saved, and what
//! `/api/server-info` then serves everyone.

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::Storage;

fn frame(v: serde_json::Value) -> RelayMessage {
    serde_json::from_value::<RelayMessage>(v).expect("the frame parses")
}

fn update(figures: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "type": "server_settings_update", "funding_figures": figures })
}

fn patreon_20() -> serde_json::Value {
    serde_json::json!({
        "channels": [{ "name": "Patreon", "link": "https://www.patreon.com/x", "monthly_usd": 20.0, "as_of": "" }],
        "costs": [{ "name": "Servers", "monthly_usd": 45.0, "as_of": "" }]
    })
}

/// A MEMBER WHO IS NOT AN ADMIN CANNOT CHANGE THE FIGURES (2026-11-22): a member's and a
/// moderator's `server_settings_update` naming figures leaves the server publishing nothing, and
/// `/api/server-info` still serves no figures.
///
/// Seen red 2026-11-22 with the update handler's role check switched off: "member, who is not an
/// admin, published figures".
#[tokio::test]
async fn a_member_who_is_not_an_admin_cannot_change_the_figures() {
    let state = std::sync::Arc::new(RelayState::new(Storage::open_temp("funding_figures_member")));
    state.db.set_role("moddy", "mod").unwrap();
    for who in ["member", "moddy"] {
        super::server_settings_update::handle(&state, who, frame(update(patreon_20()))).await;
        assert!(state.db.get_server_settings().unwrap().funding_figures.is_empty(), "{who}, who is not an admin, published figures");
    }
    let info = crate::relay::api::get_server_info(axum::extract::State(state.clone())).await.0;
    assert!(info.funding.as_ref().map_or(true, |f| f.get("channels").is_none() && f.get("costs").is_none()), "nothing published, nothing served");
}

/// AN ADMIN'S FIGURES ARE SAVED, STAMPED AND SERVED (2026-11-22): the owner's update is saved with
/// today as each row's date (the relay's, not the sender's), `/api/server-info` serves the
/// channels and costs to anyone, and a bad row (an http link) refuses the whole update, so the
/// saved figures stay as they were.
///
/// Seen red 2026-11-22 with `served` returning the config's funding untouched: the served `channels`
/// were `Null` ("assertion `left == right` failed / left: Null").
#[tokio::test]
async fn an_admins_figures_are_saved_stamped_and_served() {
    let state = std::sync::Arc::new(RelayState::new(Storage::open_temp("funding_figures_admin")));
    // Only a claim or a handover makes an owner (the owner branch: set_role refuses the role).
    assert!(state.db.claim_owner("op", 0).unwrap(), "op claims the empty owner role");
    super::server_settings_update::handle(&state, "op", frame(update(patreon_20()))).await;
    let saved = state.db.get_server_settings().unwrap().funding_figures;
    assert_eq!(saved.channels.len(), 1, "the owner's figures are saved");
    assert_eq!(saved.channels[0].as_of, crate::relay::storage::funding_figures::today(), "the relay stamps today");
    let info = crate::relay::api::get_server_info(axum::extract::State(state.clone())).await.0;
    let f = info.funding.expect("published figures are served");
    assert_eq!(f["channels"][0]["monthly_usd"], 20.0);
    assert_eq!(f["costs"][0]["name"], "Servers");
    // A bad row refuses everything and keeps what was saved.
    let bad = serde_json::json!({ "channels": [{ "name": "Evil", "link": "http://x.example", "monthly_usd": 1.0 }], "costs": [] });
    super::server_settings_update::handle(&state, "op", frame(update(bad))).await;
    assert_eq!(state.db.get_server_settings().unwrap().funding_figures, saved, "a bad row changed nothing");
    // Emptying the figures publishes nothing again.
    super::server_settings_update::handle(&state, "op", frame(update(serde_json::json!({ "channels": [], "costs": [] })))).await;
    let info = crate::relay::api::get_server_info(axum::extract::State(state.clone())).await.0;
    assert!(info.funding.as_ref().map_or(true, |f| f.get("channels").is_none() && f.get("costs").is_none()), "emptied figures serve nothing");
}
