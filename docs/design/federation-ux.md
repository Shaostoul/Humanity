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

## Build order

1. DONE v0.1125.0: saved-server persistence; THIS PC rail entry; the
   add-by-key pairing path (v0.1124.0) and the Federation config section
   opener already on the Relays page.
2. **Multi-connection foundation** (the heavy lift): per-server
   connections (Vec of ws_client + per-server message/channel state),
   message routing by server, reconnect per server. Everything else
   stacks on this.
3. **Sidebar restructure**: Commons + per-server sections per the rules
   above; server settings header names its server loudly.
4. **Send-failover** for Commons rooms.
5. **Servers admin page**: rename Relays -> Servers; per-server panel
   keeps Health/Control/Console/Config; Federation panel grows peer list
   with live status dots, add-by-URL, add-by-key, per-channel toggles that
   SAY what they are bridged to ("bridged with public.guide") or "no
   peers yet - add one first" instead of a mute checkbox.

## Related

- docs/BUGS.md BUG-071 (the seven-defect federation repair)
- docs/admin/site-resilience.md (why federation is the anti-single-point
  strategy; Cloudflare declined)
- tests/federation_two_relays.rs (the definition of federation working)
