# Federation UX: servers, the Commons, and how multi-server chat should feel

> Created 2026-08-14, the night the first two production servers federated
> (united-humanity.us <-> public.guide, BUG-071 repair) and the operator
> field-tested the client against a real multi-server world for the first
> time. Every confusion they hit is recorded here with its cause, because
> the design falls straight out of them. Decisions below were made jointly
> (operator taste calls, 2026-08-14).

## The mental model (canonical wording, reuse everywhere)

- A **server** is a place: its own channels, members, settings, operator.
- **Federation** is a bridge between the same room on two servers, opted
  into by BOTH operators (peer trust + per-channel toggle on each side).
- A **federated room** belongs to its mesh, not to any one server.

## Field-test findings (2026-08-14) and their causes

1. "united-humanity.us disappeared when I connected to public.guide."
   The client holds ONE live connection; switching disconnects the rest,
   and nothing says so.
2. "The channels seem universal across servers." Every server seeds
   default channels with the same names (#general, #announcements);
   same-named but independent rooms, with no per-server grouping cues.
3. "The servers inherit each other's settings." Server Settings silently
   edits whichever server is CURRENTLY connected; with switching itself
   invisible, it reads as one shared pane.
4. "The federation buttons don't do anything." The toggle marks a channel,
   but replication needs a peer relationship, and no UI anywhere showed
   peers, their status, or what a toggle was connected to. A control whose
   effect depends on an invisible precondition is a broken promise.
5. "Saved servers vanished on relaunch." chat_servers was runtime-only
   memory; nothing persisted it. FIXED v0.1125.0: AppConfig.saved_servers
   round-trips identity (name + url); connection state stays runtime.
6. "Host a node on this PC looks like a setting of whichever server is
   selected." It rendered at the bottom of every remote relay's page.
   FIXED v0.1125.0: permanent THIS PC rail entry with its own panel.

## Decisions (operator, 2026-08-14)

1. **Connected to all saved servers at once** (Discord-shaped). No more
   disappearing; the sidebar becomes per-server sections.
2. **A federated room renders ONCE, merged**, regardless of how many of
   your servers carry it, with a bridge badge listing carriers.
3. **One "Servers" concept everywhere**: the chat sidebar lists places you
   visit; the header page (rename from "Relays") manages the ones you run
   and administer, including a first-class Federation panel.

## The Commons (operator's insight, adopted)

A federated room does not belong exclusively to any server, so it does not
live under one in the sidebar. Structure:

```
DMs
Groups
COMMONS                        <- rooms bridged across >= 2 of YOUR servers
  # general        (bridge badge: carriers + live status on hover)
  # announcements
SERVERS
  united-humanity.us           <- each server: ONLY its local-only rooms
    # ops
  public.guide
  This PC (when hosting)
```

Rules:
- A room appears in the Commons when it is federated and at least two of
  the user's connected servers carry it; it then leaves the per-server
  lists (never duplicated).
- Federated on only ONE of your servers: stays under that server with a
  small bridge badge (bridged somewhere, but you have one door).
- **Send routing = resilience made visible**: a Commons message is sent
  via any healthy carrying server; if one carrier dies mid-conversation,
  the next message routes through another. The room outlives any single
  server, which is the entire point of federation expressed at the level
  where users live.
- Dedup by (origin_server, sender, timestamp), the existing federated
  dedup key.
- KNOWN LIMIT (accepted for the operator-curated era): rooms are matched
  across the mesh BY NAME. Two unrelated meshes both federating a
  "#general" would collide if a user joined both. Real cross-mesh channel
  identity belongs to the signed-object space_id system; revisit when
  strangers can federate.

## Discovery, not auto-add (operator question answered, 2026-08-14)

The operator asked: should federated servers CHAIN into every user's list
automatically ("if Target.com joins, everyone's list grows"), since making
each user hand-add servers feels like a Discord server list when the
federation is meant to be a nexus?

Position adopted: **the nexus is delivered by the Commons, not by the
server list.** A federated room reaches you through ANY one door: connect
to a single server and every room it bridges is already in your Commons.
Users never need their server list to grow to get the mesh's content.
What auto-adding servers would actually do at scale:

- turn one operator's peering decision into every user's sidebar noise
  (fifty corporate servers materializing unasked is the exact pattern this
  project exists to oppose);
- conflate operator-to-operator trust with user choice;
- cost a live connection per listed server under the multi-connection
  model (a socket per row does not survive a 50-server mesh);
- hand any peer that reaches tier 2 anywhere a free advertising slot in
  every client.

What users DO need is **discoverability**: a Federation Directory. The
data already exists (GET /api/federation/servers); the app renders each
connected server's peer graph as a browsable directory: "united-humanity
.us federates with: public.guide, ..." with one-click "Add to my list",
including the extended graph ("via public.guide: ...") for depth. Autoyou
DISCOVER, you CHOOSE to join. Plus an opt-in per-user setting, default
OFF: "automatically follow my home server's federation" for people who
genuinely want the growing list.

Server-side trust stays **non-transitive** on purpose: A trusting B and B
trusting C does not make A trust C. Every operator picks their own peers;
one compromised hub cannot chain into everything. The directory shows the
graph; it never walks it for you.

## Build order

1. DONE v0.1125.0: saved-server persistence; THIS PC rail entry; the
   add-by-key pairing path (v0.1124.0) and the Federation config section
   opener already on the Relays page.
2. DONE v0.1128.0: **Multi-connection foundation**, as the hybrid the
   state survey recommended: the legacy active fields stay the ACTIVE
   connection (zero churn on the router and UI); `ServerConnection`
   parks every other server WITH its live socket; switching = park +
   unpark, not teardown; `engine/bg_connections.rs` dials all saved
   servers, answers their identify challenges, stores their traffic,
   marks unread, and redials with backoff. Sidebar rows show live
   link + unread dots.
3. DONE v0.1129.0: **Sidebar Commons**: bridged rooms (federated flag +
   carried by 2 or more of my servers, matched by name) render once in
   a COMMONS section with the both-ways badge and carrier count; they
   leave the per-server lists; background servers list their remaining
   channels, clickable (switch + open). The center view MERGES every
   carrier's copy of a Commons room: native lines beat federated
   echoes on (timestamp, content); federated copies collapse on
   (origin, timestamp, content). Still to do from the sidebar spec:
   the server settings header naming its server loudly.
4. DONE v0.1129.0/v0.1130.0: **Send routing + failover** for Commons
   rooms: a room the active server does not carry sends through a
   background carrier (never onto the active relay, which would fork a
   local unbridged room). Failover is inherent in the routing: carrier
   choice re-evaluates on EVERY send against live link state, and an
   offline active server automatically fails over to a healthy carrier.
   Per-connection link state + message counters live in the F3 overlay.
5. DONE v0.1130.0: **Servers admin**: nav renamed Relays -> Servers;
   Federation panel has peer list with status + trust tiers + remove,
   add-by-URL, and ADD BY KEY (NAT pairing, validated 64-hex + Pair
   button); the channel editor's Federated toggle names its live bridge
   peers ("Federated channels here bridge with: ...") or says there are
   no peers yet; the Server Settings header names its server loudly
   ("Server: <name>" + URL). Remaining niceties for later: per-channel
   per-peer bridge scoping, and a merged per-server admin panel layout.

## Related

- docs/BUGS.md BUG-071 (the seven-defect federation repair)
- docs/admin/site-resilience.md (why federation is the anti-single-point
  strategy; Cloudflare declined)
- tests/federation_two_relays.rs (the definition of federation working)
