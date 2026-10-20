# Files at scale: pieces, swarms and fair shares

> Written 2026-10-11 from the operator's question: "Is this essentially how torrents are handled?
> Essentially it seems like we're prepping for torrent sharing which makes sense. How could we make
> this even better? We also have to think, what happens when I get a server with a million users.
> Obviously a million users can't store 512MB on my limited server. The bandwidth usage could become
> a usage for DMs/Groups. How do we handle this eventuality?"
>
> Status: direction, not built. The pieces already built are listed under "What exists". The near
> increments live in [files-showcase.md](files-showcase.md) (answers 9 and 10, resumable uploads);
> this document is the shape they grow into.

## Is it a torrent?

Partly. What is planned for big private files (one sender, one receiver, both online, the receiver
accepting) is a direct transfer, one to one. A torrent is many to many: a file is cut into pieces,
everyone who has a piece passes it on, and the more people want a file the more people are serving
it. That last property is the one worth having, because it is the only way serving cost falls as a
file gets popular instead of rising. So yes: the right destination is a swarm, and the one-to-one
transfer is its smallest case (a swarm of two).

## The design, layer by layer

1. **Every file is pieces with one fingerprint.** A file is cut into fixed pieces under a BLAKE3
   tree (the verified-streaming layout BLAKE3's authors published for exactly this, "Bao"), so a
   piece from anyone can be checked on arrival against the file's one fingerprint. Resuming from a
   different source, fetching pieces from several people at once, and storing identical files once
   (the same fingerprint is the same file) all fall out of that. The resumable upload already sends
   pieces and checks a BLAKE3 hash at the end; this moves the check to every piece.
2. **Private files are sealed before they are cut.** A DM or group file is encrypted on the
   sender's device first (as today), and the pieces are pieces of the sealed file. So a helper who
   carries pieces (a group member, a server, a friend's device) never sees what is in them.
3. **Groups share among themselves.** A group file is sent once. Members who are online fetch
   pieces from each other; the server keeps a copy only for members who were offline when it was
   sent, and only for the mailbox lifetime (30 days by default). A ten-person group sending a video
   costs the server one copy at most, not ten downloads of it.
4. **Public Media seeds itself.** The server seeds its public files; viewers who opt in seed what
   they have watched, while the page is open (browsers can take part through WebRTC data channels,
   the way WebTorrent does; the desktop app does it in the background with a bandwidth cap the
   person sets). A popular clip gets cheaper for the server, not dearer.
5. **Addresses stay private by default.** A swarm shows every member's network address to the
   others. So swarming is among friends and in groups whose members chose it; between strangers,
   pieces go through the server's call forwarder (bandwidth, never disk), or not at all. Nobody
   joins a public swarm without saying yes.

## A million people on one server

They should not be. The honest answer has three parts.

- **The server is a mailbox, not an archive.** Each person's devices are where their files live
  (the same local-first rule DM history already follows); a server holds files in transit, for the
  mailbox lifetime, and Media the operator chooses to keep. That bounds the disk by how much moves
  in 30 days, not by how much everyone has ever sent.
- **Fair shares that shrink as the server fills.** Today's limits are fixed numbers per role (the
  Media share, the private files held, the daily allowance). At scale the operator sets the pool
  (disk and monthly bandwidth) and the server works out each person's share from it: the pool
  divided among the people actually using it, times their role's weight, never more than the
  role's own limit. A quiet server gives generous shares; a crowded one gives smaller ones, and
  nobody can crowd anyone else out. The meter shows both the pool and the current fair share.
- **Many servers, not one big one.** HumanityOS is federated
  ([storage-architecture.md](storage-architecture.md), "Scaling story"): communities run their own
  servers, each with its own disk, limits and rules, and a person's files live where they are
  active. A million people on one operator's VPS is not a plan; a million people across thousands
  of servers, each run by someone who chose to, is.

And one cooperative layer on top: **people can give back.** A person can offer disk and bandwidth
(the existing `vault_backup` feature already lets people store others' encrypted backups), and a
ledger shows what each person used and contributed, in the red or the black, as the fleet ledger
does for supplies. Someone who sends a lot can balance it by seeding. Nothing is paid; it is the
same "contribute what you can" rule as the rest of the project.

## What exists

- Resumable Media uploads in 8 MiB pieces with a BLAKE3 check at the end (2026-10-11,
  `src/relay/resumable.rs`).
- Encrypted DM and group files, sealed on the sender's device (CLAUDE.md, Cryptography,
  "Encrypted attachments").
- WebRTC data channels for chat between browsers (`web/chat/chat-p2p.js`) and P2P groups
  ([p2p-groups.md](p2p-groups.md)).
- The call forwarder that keeps addresses private (`src/relay/call_forwarder.rs`; its port opens
  when the operator opens it).
- An earlier forward design for content-addressed P2P file sharing,
  [network/file_sharing.md](../network/file_sharing.md) (`file_manifest`, `block_have`,
  `block_request`): not built. It is the first draft of layers 1 to 3 here; when they are built,
  reconcile the two (its Ed25519 signing becomes Dilithium3, its hashes BLAKE3).
- A BitTorrent seeder for release downloads ([torrent-infrastructure.md](../admin/torrent-infrastructure.md)),
  built but not serving today: the release manifest carries no magnet links (checked 2026-09-14).

## Order of work

1. The near increments in files-showcase.md: per-person caps, the 30-day expiry, the
   device-to-device transfer with the receiver accepting (a swarm of two, through the forwarder
   between strangers).
2. Pieces checked one by one (layer 1), so a transfer can resume from anyone.
3. Group swarms (layer 3), the biggest saving for the server.
4. Fair shares that follow the pool (the operator's million-user question).
5. Public Media seeding (layer 4) and the contribution ledger.
