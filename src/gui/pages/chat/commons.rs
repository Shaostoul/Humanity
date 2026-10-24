//! The Commons: rooms bridged across two or more of my servers, shown once (docs/design/
//! federation-ux.md, "The Commons"). Which of my servers make up a Commons room, how a line typed
//! there is routed, and the servers' federation links that decide it.
//!
//! Moved out of `gui/pages/chat.rs` on 2026-10-24 with the linked-servers rule (federation-ux.md,
//! build order 6). Before it, a room was matched across my servers BY NAME alone: the operator's
//! own "Super Secret Invite Only Server" (a node on his PC, `http://127.0.0.1:3210`) marks its
//! #general federated, so the app listed it as a third carrier of the public #general beside
//! united-humanity.us and public.guide, though it is linked to neither. Two separate rooms were
//! merged into one view, and a line typed there could be sent through the private server. Now a
//! Commons room is a group of LINKED servers, and every place that asks (the sidebar, the
//! per-server lists, the merged view, the header, the composer, read-only, send routing) takes its
//! servers from `commons_carriers`, so they cannot disagree. The sidebar section itself, which has
//! words, stays in chat.rs.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use super::{norm_server_url, viewer_role};
use crate::gui::GuiState;

/// One Commons room: a federated channel that TWO OR MORE of my LINKED servers carry, so it
/// renders once in the COMMONS sidebar section instead of once per server. Rooms are still matched
/// by name, but only among servers that bridge with each other (`commons_carriers`).
pub(crate) struct CommonsRoom {
    /// Channel id/name (the wire channel string).
    pub name: String,
    /// How many of my signed-in servers carry it.
    pub carriers: usize,
    /// Whether the ACTIVE connection is one of the carriers (send path:
    /// active if possible, else a background carrier).
    pub active_carries: bool,
    /// Any carrier holds unread activity in this room.
    pub unread: bool,
    /// The signed-in carriers' urls, for the at-a-glance identity-color dots on the room row
    /// (field test 6: WHICH servers relay a room, without menus) and for hiding the room from
    /// exactly those servers' own lists.
    pub carrier_urls: Vec<String>,
}

/// The room name behind a Commons view id ("commons:general" -> "general"),
/// or None when the active channel is not a Commons view. A Commons room and
/// a same-named LOCAL channel are DIFFERENT rooms: the qualified id keeps
/// them apart (the operator's private #general must never blend into the
/// bridged #general, and vice versa).
pub(crate) fn commons_room_of(active_channel: &str) -> Option<&str> {
    active_channel.strip_prefix("commons:")
}

/// Open a Commons room: qualified view id, fresh buffer, history refetch,
/// and the unread mark cleared on every carrier.
pub(crate) fn open_commons_room(state: &mut GuiState, name: &str) {
    state.chat_active_channel = format!("commons:{}", name);
    state.chat_messages.clear();
    state.history_fetched = false;
    if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == name) {
        c.unread = false;
    }
    for conn in state.connections.iter_mut() {
        if let Some(c) = conn.channels.iter_mut().find(|c| c.id == name) {
            c.unread = false;
        }
    }
}

/// Which of my servers make up one Commons room (`commons_carriers`).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct CommonsCarriers {
    /// The active server is one of them.
    pub active: bool,
    /// The parked servers that are, as indexes into `state.connections`.
    pub parked: Vec<usize>,
    /// How many of them are signed in now (the active one connected, a parked one identified).
    pub signed_in: usize,
}

impl CommonsCarriers {
    /// Whether this is a Commons room at all: two or more of them signed in.
    pub fn is_commons(&self) -> bool {
        self.signed_in >= 2
    }
}

/// Whether the active server's socket is up.
fn active_connected(state: &GuiState) -> bool {
    state.ws_client.as_ref().map_or(false, |c| c.is_connected())
}

/// A server's identity for links: the web address its federation list names it by
/// (`wss://host/ws` and `https://host/` are both `https://host`), in lower case.
fn link_key(url: &str) -> String {
    crate::gui::widgets::image_cache::http_base(url).to_lowercase()
}

/// The servers that carry `room` as a bridged room and are linked to each other: my servers that
/// mark it federated, grouped by their federation links (either one's list naming the other), the
/// group holding the active server if two or more of it are signed in, else the largest such
/// group. A server whose list has not arrived is linked to nobody. Servers in no group of two
/// keep the room in their own list (federation-ux.md, build order 6).
pub(crate) fn commons_carriers(state: &GuiState, room: &str) -> CommonsCarriers {
    // (link key, signed in, None = the active server or Some(parked index)).
    let mut nodes: Vec<(String, bool, Option<usize>)> = Vec::new();
    if state.chat_channels.iter().any(|c| c.id == room && c.federated) {
        nodes.push((link_key(&state.server_url), active_connected(state), None));
    }
    for (i, c) in state.connections.iter().enumerate() {
        if c.channels.iter().any(|ch| ch.id == room && ch.federated) {
            nodes.push((link_key(&c.url), c.identified, Some(i)));
        }
    }
    // Groups by links: a union-find over a handful of servers.
    let n = nodes.len();
    let mut up: Vec<usize> = (0..n).collect();
    fn root(up: &mut [usize], mut i: usize) -> usize {
        while up[i] != i {
            up[i] = up[up[i]];
            i = up[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            if state.federation_links.linked(&nodes[a].0, &nodes[b].0) {
                let (ra, rb) = (root(&mut up, a), root(&mut up, b));
                up[ra] = rb;
            }
        }
    }
    let roots: Vec<usize> = (0..n).map(|i| root(&mut up, i)).collect();
    let signed_in = |r: usize| (0..n).filter(|&i| roots[i] == r && nodes[i].1).count();
    let active_group = (0..n).find(|&i| nodes[i].2.is_none()).map(|i| roots[i]).filter(|&r| signed_in(r) >= 2);
    let chosen = active_group.or_else(|| {
        // The largest group of two or more signed in; on a tie, the first listed.
        let mut best: Option<(usize, usize)> = None;
        for i in 0..n {
            let count = signed_in(roots[i]);
            if count >= 2 && best.map_or(true, |(_, c)| count > c) {
                best = Some((roots[i], count));
            }
        }
        best.map(|(r, _)| r)
    });
    let Some(chosen) = chosen else { return CommonsCarriers::default() };
    let mut out = CommonsCarriers::default();
    for i in (0..n).filter(|&i| roots[i] == chosen) {
        match nodes[i].2 {
            None => out.active = true,
            Some(p) => out.parked.push(p),
        }
        if nodes[i].1 {
            out.signed_in += 1;
        }
    }
    out
}

/// How a line typed in a Commons room is sent: through the active server when it is one of the
/// room's carriers and connected, else through the first parked carrier with a live socket. The
/// send (chat.rs `send_composed_content`), the message box's limit (line_limit.rs), the composer
/// hint and read-only all ask this, so the server shown is the one the line goes to. (false,
/// None): no carrier is connected.
pub(crate) fn commons_route(state: &GuiState, room: &str) -> (bool, Option<usize>) {
    let c = commons_carriers(state, room);
    if c.active && active_connected(state) {
        return (true, None);
    }
    let parked = c.parked.into_iter().find(|&i| {
        let conn = &state.connections[i];
        conn.identified && conn.ws.is_some()
    });
    (false, parked)
}

/// Whether the resolved Commons send carrier treats `room` as read-only for
/// THIS user (admins/mods still post -- e.g. #announcements). None = no
/// connected carrier at all. Field test 5: bridged read-only rooms must not
/// accept posts through the side door of a different carrier.
pub(crate) fn commons_room_read_only(state: &GuiState, room: &str) -> Option<bool> {
    let is_mod = |role: &str| role == "admin" || role == "owner" || role == "moderator" || role == "mod";
    match commons_route(state, room) {
        (true, _) => {
            let ro = state.chat_channels.iter().find(|c| c.id == room).map(|c| c.read_only).unwrap_or(false);
            Some(ro && !is_mod(&viewer_role(state)))
        }
        (false, Some(i)) => {
            let conn = &state.connections[i];
            let ro = conn.channels.iter().find(|ch| ch.id == room).map(|ch| ch.read_only).unwrap_or(false);
            let role = conn
                .users
                .iter()
                .find(|u| u.public_key == state.profile_public_key)
                .map(|u| u.role.clone())
                .unwrap_or_default();
            Some(ro && !is_mod(&role))
        }
        (false, None) => None,
    }
}

/// The Commons rooms: every room some server of mine marks federated whose linked carriers
/// (`commons_carriers`) number two or more signed in.
pub(crate) fn commons_rooms(state: &GuiState) -> Vec<CommonsRoom> {
    let mut names: Vec<String> = state.chat_channels.iter().filter(|c| c.federated).map(|c| c.id.clone()).collect();
    for conn in &state.connections {
        names.extend(conn.channels.iter().filter(|c| c.federated).map(|c| c.id.clone()));
    }
    names.sort();
    names.dedup();
    let connected = active_connected(state);
    names
        .into_iter()
        .filter_map(|name| {
            let c = commons_carriers(state, &name);
            if !c.is_commons() {
                return None;
            }
            let unread_in = |chs: &Vec<crate::gui::ChatChannel>| chs.iter().any(|ch| ch.id == name && ch.unread);
            let mut unread = false;
            let mut carrier_urls = Vec::new();
            if c.active {
                unread |= unread_in(&state.chat_channels);
                if connected {
                    carrier_urls.push(norm_server_url(&state.server_url));
                }
            }
            for &i in &c.parked {
                let conn = &state.connections[i];
                unread |= unread_in(&conn.channels);
                if conn.identified {
                    carrier_urls.push(conn.url.clone());
                }
            }
            Some(CommonsRoom { carriers: c.signed_in, active_carries: c.active && connected, unread, carrier_urls, name })
        })
        .collect()
}

/// The Commons rooms `server_url` is a carrier of: the rooms that list moves to the COMMONS
/// section. A room another group of my servers bridges, but this server is not linked to, stays in
/// this server's own list.
pub(crate) fn commons_rooms_carried_by(state: &GuiState, server_url: &str) -> std::collections::HashSet<String> {
    let me = norm_server_url(server_url);
    commons_rooms(state).into_iter().filter(|r| r.carrier_urls.iter().any(|u| *u == me)).map(|r| r.name).collect()
}

/// When a server's federation list is asked for again: links change by an operator's decision,
/// so rarely; a failed answer is tried again sooner.
const LINKS_REFRESH: Duration = Duration::from_secs(600);
const LINKS_RETRY: Duration = Duration::from_secs(30);

/// Each of my servers' federation links, from its own GET /api/federation/servers, asked when the
/// app is signed in to it and again every ten minutes (`pump_federation_links`). Not a cache of
/// anything the app could compute: it is what each server says it bridges with.
#[derive(Default)]
pub struct FederationLinks {
    /// Server (`link_key`) -> the servers its list names (`link_key`).
    known: HashMap<String, Vec<String>>,
    /// When each server's list is asked for next.
    due: HashMap<String, Instant>,
    /// Lists being fetched: (server, the answer).
    in_flight: Vec<(String, Receiver<Result<Vec<String>, String>>)>,
}

impl FederationLinks {
    /// Whether two of my servers bridge with each other: either one's list names the other.
    pub fn linked(&self, a: &str, b: &str) -> bool {
        let names = |x: &str, y: &str| self.known.get(x).map_or(false, |l| l.iter().any(|p| p == y));
        a == b || names(a, b) || names(b, a)
    }
}

/// The servers a GET /api/federation/servers answer names (each entry's `url`), as `link_key`s.
fn parse_links(body: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let list = v.as_array().ok_or("not a list")?;
    Ok(list.iter().filter_map(|e| e.get("url").and_then(|u| u.as_str())).filter(|u| !u.is_empty()).map(link_key).collect())
}

/// Once a frame: take in the federation lists that arrived, and ask each server I am signed in
/// to for its list when it is due, on a worker thread (never the render thread). Called from the
/// background-connection pump (engine/bg_connections.rs).
pub(crate) fn pump_federation_links(state: &mut GuiState) {
    let now = Instant::now();
    let links = &mut state.federation_links;
    let mut i = 0;
    while i < links.in_flight.len() {
        let answer = match links.in_flight[i].1.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                i += 1;
                continue;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the fetch ended without an answer".to_string()),
        };
        let (server, _) = links.in_flight.remove(i);
        match answer {
            Ok(peers) => {
                links.known.insert(server.clone(), peers);
                links.due.insert(server, now + LINKS_REFRESH);
            }
            Err(e) => {
                // Kept as it was (nothing, the first time): a slow or failed answer never links.
                log::warn!("federation list of {server} not read: {e}");
                links.due.insert(server, now + LINKS_RETRY);
            }
        }
    }
    let mut signed_in: Vec<String> = Vec::new();
    if active_connected(state) {
        signed_in.push(state.server_url.clone());
    }
    signed_in.extend(state.connections.iter().filter(|c| c.identified && c.ws.is_some()).map(|c| c.display_url.clone()));
    let links = &mut state.federation_links;
    for url in signed_in {
        let key = link_key(&url);
        if key.is_empty() || links.in_flight.iter().any(|(s, _)| *s == key) || links.due.get(&key).map_or(false, |d| now < *d) {
            continue;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let api = format!("{key}/api/federation/servers");
        std::thread::spawn(move || {
            let answer = ureq::get(&api)
                .timeout(Duration::from_secs(8))
                .call()
                .map_err(|e| e.to_string())
                .and_then(|r| r.into_string().map_err(|e| e.to_string()))
                .and_then(|body| parse_links(&body));
            let _ = tx.send(answer);
        });
        links.in_flight.push((key, rx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::{ChatChannel, ServerConnection};

    fn conn(url: &str, identified: bool, channels: &[(&str, bool, bool)]) -> ServerConnection {
        ServerConnection {
            url: url.to_string(),
            identified,
            channels: channels
                .iter()
                .map(|(id, federated, unread)| ChatChannel {
                    id: id.to_string(),
                    name: id.to_string(),
                    federated: *federated,
                    unread: *unread,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// `a`'s federation list names `b` (as the two public servers' lists name each other).
    fn link(state: &mut GuiState, a: &str, b: &str) {
        state.federation_links.known.entry(link_key(a)).or_default().push(link_key(b));
    }

    #[test]
    fn a_room_two_linked_servers_bridge_is_commons_and_one_server_is_not() {
        let mut state = GuiState::default();
        // Active connection is offline (no ws), so carriers come from the
        // two background connections.
        state.connections.push(conn("https://a", true, &[("general", true, false), ("ops", true, false)]));
        state.connections.push(conn("https://b", true, &[("general", true, true), ("lounge", false, false)]));
        link(&mut state, "https://a", "https://b");
        let rooms = commons_rooms(&state);
        assert_eq!(rooms.len(), 1, "only the room BOTH servers federate qualifies");
        assert_eq!(rooms[0].name, "general");
        assert_eq!(rooms[0].carriers, 2);
        assert!(!rooms[0].active_carries, "active connection is offline");
        assert!(rooms[0].unread, "unread on ANY carrier lights the room");
    }

    #[test]
    fn same_name_without_the_federated_flag_never_merges() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", false, false)]));
        state.connections.push(conn("https://b", true, &[("general", false, false)]));
        link(&mut state, "https://a", "https://b");
        assert!(commons_rooms(&state).is_empty(), "two plain #general rooms stay separate");
    }

    /// The operator's case (2026-10-24): two linked public servers and a private node on this PC,
    /// all three marking #general federated. The private node is not linked to either, so it is no
    /// carrier, keeps its #general in its own list, and is never routed through. Seen red before
    /// the rule: the private node was the room's third carrier.
    #[test]
    fn a_server_linked_to_neither_is_no_carrier_of_their_room() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://united-humanity.us", true, &[("general", true, false)]));
        state.connections.push(conn("http://127.0.0.1:3210", true, &[("general", true, false)]));
        state.connections.push(conn("https://public.guide", true, &[("general", true, false)]));
        link(&mut state, "https://united-humanity.us", "https://public.guide");
        link(&mut state, "https://public.guide", "https://united-humanity.us");
        let c = commons_carriers(&state, "general");
        assert_eq!(c.parked, vec![0, 2], "the two linked servers, not the private node");
        assert!(c.is_commons());
        let rooms = commons_rooms(&state);
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].carrier_urls, vec!["https://united-humanity.us".to_string(), "https://public.guide".to_string()]);
        assert!(commons_rooms_carried_by(&state, "https://public.guide").contains("general"));
        assert!(!commons_rooms_carried_by(&state, "http://127.0.0.1:3210").contains("general"), "it stays in the private node's own list");
    }

    #[test]
    fn a_server_whose_list_has_not_arrived_is_linked_to_nobody() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", true, false)]));
        state.connections.push(conn("https://b", true, &[("general", true, false)]));
        assert!(commons_rooms(&state).is_empty(), "no lists yet, no Commons: safe by default");
        link(&mut state, "https://b", "https://a");
        assert_eq!(commons_rooms(&state).len(), 1, "one list naming the other is enough");
    }

    #[test]
    fn a_link_is_the_same_server_however_its_address_is_written() {
        let mut state = GuiState::default();
        state.connections.push(conn("wss://a.example/ws", true, &[("general", true, false)]));
        state.connections.push(conn("https://B.example/", true, &[("general", true, false)]));
        link(&mut state, "https://a.example", "https://b.example");
        assert_eq!(commons_carriers(&state, "general").parked, vec![0, 1]);
        assert_eq!(parse_links(r#"[{"server_id":"x","url":"https://Public.Guide/","status":"offline"},{"url":""}]"#), Ok(vec!["https://public.guide".to_string()]));
    }

    #[test]
    fn opening_a_commons_room_uses_the_qualified_id_and_clears_carrier_unread() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", true, true)]));
        state.connections.push(conn("https://b", true, &[("general", true, true)]));
        open_commons_room(&mut state, "general");
        // The qualified id keeps the bridged room distinct from any
        // same-named LOCAL channel (the private #general must never blend
        // into the Commons view).
        assert_eq!(state.chat_active_channel, "commons:general");
        assert_eq!(commons_room_of(&state.chat_active_channel), Some("general"));
        assert!(
            state.connections.iter().all(|c| c.channels.iter().all(|ch| !ch.unread)),
            "opening the room clears unread on every carrier"
        );
    }

    #[test]
    fn unidentified_connections_do_not_count_as_carriers() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", true, false)]));
        state.connections.push(conn("https://b", false, &[("general", true, false)]));
        link(&mut state, "https://a", "https://b");
        assert!(commons_rooms(&state).is_empty(), "a half-connected server is not a carrier yet");
    }
}
