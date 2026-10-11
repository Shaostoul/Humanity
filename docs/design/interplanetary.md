# Between planets: designing for light speed, and what it gives Earth now

> Written 2026-10-11 from the operator: "There's also the general Human desire to colonize other
> planets. Assuming we're stuck with the limits of light speed for file sharing. We also want to
> consider how we might handle interplanetary comms/file sharing. It's possible that thinking about
> this could lead to more significant evolutions of the software that could significantly benefit
> the Earth limited software until interplanetary is a thing. I guess this is the epitome of the
> infinite-of-x rule taken to the extreme."
>
> Status: direction. The companion documents are [eight-billion.md](eight-billion.md) (one planet,
> everyone on it) and [file-transfer-at-scale.md](file-transfer-at-scale.md).

## The limits, in numbers

- **The Moon:** about 1.3 seconds each way. Talking works; it feels like a bad phone line.
- **Mars:** from about 3 minutes each way at closest to about 22 minutes at farthest, so a question
  and its answer take between 6 and 44 minutes. No conversation, no live call, no "loading...".
- **Mars behind the Sun:** for about two weeks every 26 months (solar conjunction) there is no link
  at all. Everything has to wait, or go around through a relay spacecraft if one exists.
- **The link is narrow and shared:** a whole colony shares a few deep-space antennas' time.

Space agencies already solved the shape of this: delay- and disruption-tolerant networking
(DTN). Its Bundle Protocol (version 7, RFC 9171) sends self-contained "bundles" that each hop
stores until it can pass them on, instead of the internet's assumption that a path exists end to
end right now. NASA has run it on the International Space Station.

## What it means for HumanityOS: six rules

Each one is required between planets, and each one already pays off on Earth.

1. **Every thing is a self-contained signed bundle.** A message, a piece of a file, a profile, a
   vote: each carries who made it, its signature, and everything needed to check it, so any path
   can carry it and any hop can hold it until the next one opens. *Between planets:* the only way
   anything crosses. *On Earth now:* the same bundle travels over the internet, a LAN, LoRa radio,
   or a USB stick carried across a disaster zone (the planned FederationBundle), and a server that
   relays it never has to be trusted. The signed objects HumanityOS already uses are most of this.
2. **No protocol may need a round trip to work.** A conversation of "hello", "prove it", "here",
   "ok" takes 44 minutes per exchange at Mars. Anything that must work between planets is one-way:
   a signed bundle that proves itself on arrival, checked statelessly. *On Earth now:* fewer round
   trips is faster on phone and satellite links, and works through flaky connections. HumanityOS
   already has the pattern: friendship passes and plot permits are checked statelessly, with no
   lookup and no session. The signed-in socket's challenge stays for live use; it is just never the
   only way in.
3. **There is no single "now".** Clocks disagree, and a message written later can arrive first.
   So the order of things comes from what each author had seen when they wrote (logical clocks),
   and edits made apart merge without asking anyone (the family of techniques called CRDTs, data
   that merges by rule). *On Earth now:* this is the two-devices problem the friendship choices hit
   (blocking-and-safe-mode.md 10m and 10n): the same person on a phone and a PC, each offline for a
   while. The choice notes already use the core of a hybrid logical clock: a new choice is dated
   after the newest one the device has seen for that friend, even when its own clock is behind
   (`DmStore::choose` in src/net/dm_store.rs, and the web's chat-dm-store.js). Signed profiles do
   not: the relay keeps the one with the latest timestamp and accepts timestamps up to 24 hours
   ahead (src/relay/storage/signed_profiles.rs), so a device whose clock runs two hours fast
   publishes a profile that silently beats every edit from a correct device for those two hours.
4. **Read near you, sync across the gap.** Each planet (on Earth: each region, each community)
   keeps its own servers with copies of what its people read, and syncs with the others by
   exchanging bundles. Nobody on Mars waits 44 minutes for a page that is already on Mars. *On
   Earth now:* local-first, the rule DM history already follows: fast, private, and it works when
   the internet does not.
5. **Send only what the other side is missing, most important first.** Over a narrow link, two
   sides compare fingerprints of what they hold and send the difference (set reconciliation), and
   bundles carry a priority: a text and a vote before a preview, a preview before the original, an
   emergency before everything. *On Earth now:* less mobile data, faster sync after a phone was
   offline, and a disaster zone's urgent messages getting out first.
6. **Say honestly where a thing is.** Not "sent" and "failed", but each step: written, left your
   device, reached your server, crossed to theirs, reached their device, read; and between planets,
   "arrives about 10:42 Mars time". *On Earth now:* people stop guessing whether a message got
   through, which is most of what makes chat feel unreliable.

## What else changes between planets

- **Calls and streams become recordings.** Live only works within a planet (the Moon at a push);
  across the gap, a "call" is a video message and a stream is a broadcast that arrives late and is
  watched when it arrives, in the right order.
- **Identity needs no trip home.** A Martian's key is made on Mars; everything about it checks on
  Mars. Withdrawing something (a friendship pass, a stolen key) travels as a signed bundle too, and
  until it arrives each side acts on what it knows; the documents say so plainly.
- **Self-government by distance.** A vote across planets needs a window long enough for the
  farthest voter, and most decisions belong to the people they affect: the Humanity Accord's nested
  jurisdictions (Humanity, then a world, then a place) already describe this. Each world runs its
  own servers under its own rules.
- **The game can teach it.** The simulation already models space; messages between ships and
  worlds can arrive at the speed of light, so players learn the limit by living with it.

## What to do now (cheap now, expensive later)

1. Keep every new protocol in the signed-bundle shape (rule 1) and one-way where it can be
   (rule 2). This is already the house pattern; it becomes a written rule here.
2. Date every "latest wins" record after the newest one already seen (rule 3), as the choice
   notes do: first signed profiles (the app reads its own current profile's timestamp before
   signing an edit), then anything else a person edits from more than one device.
3. Show delivery honestly (rule 6), starting with DMs and file transfers.
4. Build sync as "compare fingerprints, send what is missing" (rule 5) when cross-server mailboxes
   and file pieces are built (eight-billion.md and file-transfer-at-scale.md).
5. Build the FederationBundle (a file of signed bundles that any medium can carry) as the first
   step, because it is the same thing a Mars relay would carry, and it helps a flooded town today.
