# Eight billion people: how HumanityOS works at the scale of everyone

> Written 2026-10-11 from the operator: "There's roughly 8 billion humans on Earth. If all of them
> adopted the HumanityOS software we obviously can't have any single server handle that many.
> However we still want 8 billion people to be able to use the software, chat, share files,
> stream, etc."
>
> Status: direction. Each part below says what is built, what is planned, and what is not designed
> yet. Files have their own document, [file-transfer-at-scale.md](/library#file-transfer-at-scale).

## The rule that makes it possible

**No feature may need one particular server, or a big one.** Every capability has to work in
three ways, and the app picks the best one available:

1. **Device to device**, with no server at all (two people in a room, a village with no internet).
2. **Through any small server**, the way it works today: a community, a household, one person's PC.
3. **Across many servers**, federated: each run by someone who chose to, none in charge of the rest.

A server is a meeting place and a mailbox, never the owner of anyone's identity or history. Then
eight billion people is not one enormous system; it is millions of small ones that can talk to
each other. If one community server carries about a thousand active people, everyone on Earth is
about eight million servers. That is a large number of servers and a small number for each of
them to carry.

**The lever that makes millions of servers real: the desktop app already IS a server.** One
program is the app and the relay (`HumanityOS --headless` is the same exe). So anyone who installs
it can host a household, a club or a town, and the in-app server settings (no command line, the
GUI-first rule) are what turn "can" into "does".

## Each capability at that scale

| Capability | How it scales | Status |
|---|---|---|
| **Who you are** | Your identity is your key, made from your recovery phrase on your own device (`did:hum:` from the key). No server issues it or can take it away; it works on any server. | Built. |
| **Finding someone** | Each person publishes a small signed record: which servers carry their mailbox, and their devices' keys. Records live in a directory spread across servers (a distributed hash table, the technique behind BitTorrent's trackerless swarms), found by the person's key. No global phone book exists. Today: signed profiles gossip between federated servers. | Profile gossip built; the directory not designed. |
| **Messages to a person** | Sealed mailboxes on the servers that person chose, reached across servers by federation. The sender's server delivers to the recipient's; neither learns more than today (a sender-less sealed box). | Same-server mailbox built (sealed sender); cross-server delivery planned. |
| **Communities and rooms** | Each server hosts its own rooms; a room can be federated so people on other servers join it. A huge room is many servers carrying one conversation, each serving its own members. | Federated channels flag built; federation activation is `[next]` on the roadmap. |
| **Groups** | Peer-to-peer groups: signed, encrypted objects passed between members, servers optional. They survive any one server going away. | Phases 1-2 built; phases 3-5 planned. |
| **Files** | Pieces under one fingerprint, sealed before they are cut, shared among a group's members and seeded by the people who watch them; servers are mailboxes, not archives. See [file-transfer-at-scale.md](/library#file-transfer-at-scale). | Resumable pieces built; the rest planned. |
| **Calls** | Two to a few people: direct between devices, through a server's forwarder to keep addresses private. Big calls: a forwarding unit (an SFU) on a server receives each speaker once and sends to each listener, so one call costs one server, not everyone ([voice_video_streaming.md](https://github.com/Shaostoul/Humanity/blob/main/docs/network/voice_video_streaming.md)). | Mesh built; the forwarder's server half built (its port opens when the operator opens it, and the apps' half is separate work); the SFU planned. |
| **Live streams** | A fan-out tree: the streamer sends to one server, which serves its own viewers and forwards one copy to each other server that has viewers, which does the same. A million viewers on a thousand servers costs each server one incoming copy. Viewers may also pass the stream on, opt-in. | One relay fans out today (`/ws/live`); the tree not designed. |
| **Search** | Each server indexes its own public content. A search asks the servers the person chose, or shared indexes built from what servers already gossip. No single index of everything. | Per-server search built. |
| **Rules and moderation** | Each server's operator sets its rules, as the law where they live requires. People move freely because their identity is not tied to any server; blocklists and warnings can be shared between servers as signed lists. | Per-server rules, roles, reports built. |
| **The game world** | Each server runs its own mothership (ship homes); many servers, many ships, travel between them later. | One ship per server built. |
| **No internet** | Signed objects travel by radio (LoRa) or carried by hand (a "FederationBundle" on a USB stick) and sync when a connection returns. | Planned. |

## What it costs, and who pays

Nobody pays a company. Each server is run by someone who chose to, on hardware they have, and
the cost per server stays small because no server carries more than its own people. Three rules
keep it fair:

- **Shares follow the pool.** The operator says how much disk and bandwidth the server has; each
  person's share is worked out from that and from how many people are using it, so a crowded
  server stays usable and nobody can crowd anyone else out.
- **The busiest things share their own load.** Popular files and streams are carried by the
  people using them (opt-in, addresses private by default), so popularity does not bankrupt the
  server that hosts it.
- **People can give back, and it is counted.** A person can offer disk and bandwidth to their
  community, and a ledger shows what each used and contributed, as the fleet ledger does for
  supplies. Donations fund the operators who want help.

## What has to be true in the code from now on

These are cheap now and expensive to retrofit, so new work follows them:

1. Every protocol names the server it is on (its own `did:hum:`), as friendship passes and plot
   permits already do, so nothing breaks when the same person meets the same thing on another
   server.
2. Nothing stores "the" server: the apps can be connected to several at once, and a person's
   records say which servers carry them.
3. Every limit comes from the operator's pool and the role, never a constant a big server would
   outgrow.
4. Anything a person creates is signed by them and can be checked by anyone, so it can travel
   between servers without trusting the one that delivered it.
5. Every new feature answers: does it work with no server, with one small one, and across many?

## Beyond Earth

Light speed adds minutes to hours between worlds, and a fortnight of silence when Mars is behind
the Sun. The six rules that follow from it (self-contained signed bundles, no round trips, no
single "now", read near you, send only what is missing, say honestly where a thing is) each pay
off on Earth first: [interplanetary.md](/library#interplanetary).

## Order of work (the roadmap carries the dates)

1. Federation activation (the roadmap's `[next]`): servers that find, trust and talk to each other.
2. Cross-server delivery of sealed mailboxes, and the signed "where to reach me" record.
3. The distributed directory, so finding a person needs no particular server.
4. The live-stream fan-out tree and the call SFU.
5. Files at scale, in its own order.
