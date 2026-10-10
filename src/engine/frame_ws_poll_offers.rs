// Child of frame_ws_poll.rs (#[path]): who this device answers a direct-connection offer from,
// and which voice-room offers it takes. Moved out under the file-size ratchet on 2026-10-09
// (tests/file_size_ratchet.rs); behaviour unchanged.

/// Why this device may answer a direct-connection offer from `from`, or `None`
/// to ignore it (2026-10-09, defect 3.7.2 of docs/design/blocking-and-safe-mode.md).
/// Gathers what the app knows about the sender; the decision itself is the
/// pure `direct_offer_reason` in src/net/webrtc.rs.
///
/// - Friend: we follow them and hold the certificate they gave us. The DM store
///   holds both and is keyed by server, so it is loaded the way the DM path
///   loads it (nothing to check while the identity is locked).
/// - Group member: anyone in any P2P group we are in, from the group list the
///   relay gives us plus the open group's roster (the mesh's own source).
/// - Call or room: the other person in our current call, and everyone in the
///   voice room we are in (its roster, plus anyone connected to us in it).
/// - Asked by us: the person the Dev tools "P2P test" was pressed for.
pub(crate) fn dc_offer_reason(
    gs: &mut crate::gui::GuiState,
    from: &str,
) -> Option<crate::net::webrtc::OfferReason> {
    use crate::net::webrtc::{direct_offer_reason, holds_friendship, OfferFacts};
    let me = gs.profile_public_key.clone();
    let sender_is_friend = crate::engine::dm::ensure_dm_store(gs)
        && gs.dm_store.as_ref().map_or(false, |s| {
            holds_friendship(s.is_following(from), s.cert_for(from), from, &me)
        });
    let group_members: Vec<&str> = gs
        .p2p_groups
        .iter()
        .flat_map(|g| g.members.iter().map(String::as_str))
        .chain(gs.p2p_group_fp_to_key.values().map(String::as_str))
        .collect();
    let voice_room_peers: Vec<&str> = match gs.voice_active_room.as_deref() {
        Some(room) => gs
            .chat_channels
            .iter()
            .filter(|c| c.id == room)
            .flat_map(|c| c.voice_participants.iter().map(|(k, _)| k.as_str()))
            .chain(gs.voice_connected_peers.iter().map(String::as_str))
            .collect(),
        None => Vec::new(),
    };
    let facts = OfferFacts {
        my_key: &me,
        sender_is_friend,
        group_members,
        call_peer: gs.call_active.as_ref().map(|(k, _)| k.as_str()),
        voice_room_peers,
        asked_peer: gs.webrtc_test_peer.as_deref(),
    };
    direct_offer_reason(from, &facts)
}

/// Whether to pass a `voice_room_signal` to the WebRTC manager (2026-10-09). An
/// offer opens a connection and our answer carries this device's address, so
/// it is taken only for the voice room we are in. The relay already forwards
/// room signals only between two people in the same room; this also covers an
/// offer still on its way when we leave. Answers and candidates only touch a
/// connection we already made, so they pass.
pub(super) fn voice_signal_wanted(signal_type: &str, room_id: &str, active_room: Option<&str>) -> bool {
    signal_type != "offer" || active_room == Some(room_id)
}

/// Who this app answers a direct-connection offer from, gathered from the app's
/// own state (2026-10-09, defect 3.7.2 of docs/design/blocking-and-safe-mode.md).
/// The friend half needs the DM store on disk, so it is tested on its own in
/// src/net/webrtc.rs (`holds_friendship`); here no identity is unlocked, so no
/// store loads and nobody is a friend.
#[cfg(test)]
mod dc_offer_tests {
    use crate::gui::{ChatChannel, GuiState};
    use crate::net::webrtc::OfferReason;

    fn app() -> GuiState {
        let mut gs = GuiState::default();
        gs.profile_public_key = "me".into();
        gs
    }

    /// Seen red 2026-10-09 with `direct_offer_reason` (src/net/webrtc.rs)
    /// answering everyone left over: "a stranger gets no answer", left:
    /// Some(Friend).
    #[test]
    fn a_stranger_is_ignored_and_each_reason_lets_in_only_its_people() {
        let mut gs = app();
        assert_eq!(super::dc_offer_reason(&mut gs, "stranger"), None, "a stranger gets no answer");
        assert_eq!(super::dc_offer_reason(&mut gs, "me"), Some(OfferReason::OwnDevice));

        gs.p2p_groups.push(crate::net::api_v2::P2pGroupInfo {
            group_id: "g".into(),
            name: "Garden".into(),
            members: vec!["me".into(), "g1".into()],
            is_creator: false,
        });
        gs.p2p_group_fp_to_key.insert("fp".into(), "g2".into());
        assert_eq!(super::dc_offer_reason(&mut gs, "g1"), Some(OfferReason::GroupMember), "a member of a group we are in");
        assert_eq!(super::dc_offer_reason(&mut gs, "g2"), Some(OfferReason::GroupMember), "on the open group's roster");

        gs.call_active = Some(("c1".into(), "Cee".into()));
        assert_eq!(super::dc_offer_reason(&mut gs, "c1"), Some(OfferReason::CallOrRoom));

        // Someone in a voice room counts only while we are in that room.
        gs.chat_channels.push(ChatChannel {
            id: "lounge".into(),
            voice_participants: vec![("r1".into(), "Ar".into())],
            ..Default::default()
        });
        assert_eq!(super::dc_offer_reason(&mut gs, "r1"), None, "not in the room ourselves");
        gs.voice_active_room = Some("lounge".into());
        assert_eq!(super::dc_offer_reason(&mut gs, "r1"), Some(OfferReason::CallOrRoom));

        gs.webrtc_test_peer = Some("t1".into());
        assert_eq!(super::dc_offer_reason(&mut gs, "t1"), Some(OfferReason::AskedByUs));

        assert_eq!(super::dc_offer_reason(&mut gs, "stranger"), None, "still nobody else");
    }

    /// Seen red 2026-10-09 with `voice_signal_wanted` passing everything, as
    /// the arm did before: "another room".
    #[test]
    fn a_voice_offer_is_taken_only_for_the_room_we_are_in() {
        assert!(super::voice_signal_wanted("offer", "lounge", Some("lounge")));
        assert!(!super::voice_signal_wanted("offer", "attic", Some("lounge")), "another room");
        assert!(!super::voice_signal_wanted("offer", "lounge", None), "not in a room");
        assert!(super::voice_signal_wanted("answer", "lounge", None), "answers only touch our own offers");
        assert!(super::voice_signal_wanted("ice", "attic", Some("lounge")));
    }
}
