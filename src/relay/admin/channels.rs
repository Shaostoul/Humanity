//! The registry's channel access actions (2026-11-02, docs/design/chat-access.md): `channels.list`
//! (every channel with who can read it and who can post) and `channels.access` (set both). The
//! website's admin page draws its Channels panel from these, and an AI can list and run them; the
//! desktop app's Server Settings > Channels sets the same levels through `channel_update`.

use std::sync::Arc;

use serde_json::{json, Value};

use humanity_protocol::chat_access::{normalized_post, PostLevel, ReadLevel};

use super::{Caller, Inputs, Ran, Refusal};
use crate::relay::relay::{RelayMessage, RelayState};

/// What the Channels panel draws: each channel's id, name, read and post levels, in the sidebar's
/// order, and the words each level is sent as.
pub fn view(state: &RelayState) -> Value {
    let channels: Vec<Value> = state
        .db
        .list_channels_with_categories_and_voice()
        .unwrap_or_default()
        .into_iter()
        .map(|(id, name, _desc, (read, post), _cat, _voice)| {
            json!({ "id": id, "name": name, "read_level": read.as_str(), "post_level": normalized_post(read, post).as_str() })
        })
        .collect();
    json!({
        "channels": channels,
        "read_levels": ReadLevel::ALL.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
        "post_levels": PostLevel::ALL.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
    })
}

/// `channels.list`
pub async fn list(state: Arc<RelayState>, _caller: Caller, _inputs: Inputs) -> Result<Ran, Refusal> {
    let data = view(&state);
    let n = data["channels"].as_array().map_or(0, |c| c.len());
    Ok(Ran::said(format!("{n} channel{}.", if n == 1 { "" } else { "s" })).with_data(data))
}

/// `channels.access`: set who can read a channel and who can post in it.
pub async fn access(state: Arc<RelayState>, _caller: Caller, inputs: Inputs) -> Result<Ran, Refusal> {
    let id = inputs.text("channel").map(|c| c.trim_start_matches('#').to_lowercase()).unwrap_or_default();
    let read = inputs.text("read").as_deref().and_then(ReadLevel::parse);
    let post = inputs.text("post").as_deref().and_then(PostLevel::parse);
    let (Some(read), Some(post)) = (read, post) else {
        return Err(Refusal::bad("Who can read is anyone, members or verified; who can post is members, verified or mods."));
    };
    match state.db.set_channel_access(&id, read, post) {
        Ok(true) => {
            crate::relay::handlers::broadcast::broadcast_channel_list(&state);
            let post = normalized_post(read, post);
            let said = format!("#{id}: read by {}, posted in by {}.", read.as_str(), post.as_str());
            let _ = state.broadcast_tx.send(RelayMessage::System { message: format!("Channel {said}") });
            Ok(Ran::said(said.clone()).logged_as(said).with_data(view(&state)))
        }
        Ok(false) => Err(Refusal::not_found(format!("There is no channel #{id}."))),
        Err(e) => Err(Refusal::with(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {e}"))),
    }
}
