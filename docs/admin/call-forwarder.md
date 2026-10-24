# The call forwarder and our own STUN

Since 2026-10-09 the relay answers STUN itself and carries calls and voice rooms through
itself, on one UDP port (3478 by default). People in a call then see only the server's
address, never each other's, and nobody asks Google for their own address any more. The
design is `docs/design/blocking-and-safe-mode.md` sections 7 and 10f; the code is
`src/relay/call_forwarder.rs`, `src/relay/call_credentials.rs` and `src/relay/stun_wire.rs`.

## Opening the port (the operator's decision)

**Until you open it, nothing listens publicly.** The relay listens on UDP 3478, but the
firewall that `scripts/provision-vps.sh` writes lets in only TCP 22, 80 and 443, so the
port is closed to the internet and calls cannot use the forwarder yet (the apps say "this
server is not set up for that yet").

The exact command, on the VPS as root:

```bash
echo 'HUMANITY_OPEN_CALL_PORT=1' >> /opt/Humanity/node.env && bash /opt/Humanity/scripts/provision-vps.sh
```

The provision script then writes `udp dport 3478 accept` into `/etc/nftables.conf`, loads
it, and checks at the end that the rule is there and that the relay, and nothing else,
listens on UDP 3478. Because the rule lives in the script's own firewall definition, it
survives a reboot and every later provision run.

To open it at once without re-running the whole script (it lasts until the next reboot or
provision run, so do the line above as well):

```bash
nft add rule inet filter input udp dport 3478 accept
```

To close it again:

```bash
sed -i '/^HUMANITY_OPEN_CALL_PORT=/d' /opt/Humanity/node.env && bash /opt/Humanity/scripts/provision-vps.sh
```

Before opening it, the refusal checks should pass on the code being deployed:
`cargo test --features relay --no-default-features --lib call_` (the forwarder, its
credentials, and the test that listens at outside addresses and proves nothing is sent to
them). On the box, `ss -Hulnp 'sport = :3478'` should show one line, the HumanityOS relay.

## Why this is not the 2026-08-07 incident again

That incident was coturn with a static password from the public repo, which let anyone
relay traffic to any address on the internet (`docs/INCIDENT-PLAYBOOK.md`). This forwarder
cannot send to an address a client names:

- Relayed addresses are virtual: the server's IP with a port that is never bound.
- A permission, a channel or a Send is accepted only toward another live allocation of the
  same voice room or call. Anything else is refused, and the data goes to that allocation's
  own client, inside the relay.
- Credentials come only over the signed-in socket, to someone in that room or call, for
  that room only, from a secret made when the relay starts and never stored. A restart ends
  every credential.
- Answers to strangers (STUN, challenges, errors) are limited to 20 a second per address
  and 1,000 a second in total, and are never much larger than the request.

## Settings

| Setting | Default | What it is |
|---|---|---|
| `TURN_PORT` | 3478 | The UDP port. The provision script sets it from `HUMANITY_CALL_PORT`. |
| `TURN_BIND` | the relay's `BIND_ADDRESS` | The address the port is bound to. A development relay on 127.0.0.1 keeps the forwarder on 127.0.0.1 too (no firewall prompt on Windows). A value that is not an IP address stops the forwarder (never widens it). |
| `TURN_PUBLIC_HOST` | `TURN_SERVER_HOST`, else the bind address if it is one address, else united-humanity.us | The host the apps are told to reach. Its IPv4 address is the IP of every relayed address. The provision script sets it from `HUMANITY_CALL_HOST` (default the node's domain). |
| `HUMANITY_OPEN_CALL_PORT` | 0 | In `node.env`, read by the provision script only: 1 lets UDP `TURN_PORT` through the firewall. |

The forwarder starts only when the owner offers voice (`features.voice` in
`data/server-config.json`); with voice off the port is not opened at all and
`call_credentials` is refused like any voice message. If the port is taken by another
program, the relay keeps running without the forwarder and says so in its log.

Limits (constants in `src/relay/call_forwarder.rs`): 2,000 allocations in total, 16 per
credential (a browser makes one per person it is connected to), 32 per source address,
lifetimes of 10 minutes by default and an hour at most (refreshed by the apps), 8 KiB a
packet and 1.25 MB a second per allocation.

A self-hoster opens the same UDP port on their own firewall or router, the same way.

## Direct calls and the STUN responder (2026-10-22)

Since 2026-10-22 each person chooses how their own calls and voice rooms connect (Settings >
Privacy in the desktop app, Settings > Safety in the web chat; the design is
`docs/design/blocking-and-safe-mode.md` 10p):

- **Private** (everyone's default): only the forwarder's relayed address is offered, as above.
- **Direct**: the person's own addresses are offered too, so two Direct people can talk device to
  device (less of the server's bandwidth, a little less delay). Their app learns its public address
  from **this server's own STUN responder**, which is the same UDP port as the forwarder.
- **Direct, also asking a public service**: as Direct, and a public STUN server the person picks
  (`data/net/call_connection.json`: Google's or Cloudflare's) is asked as well. Your server is not
  involved in that; the person chose to let that company see their address.

Each person's choice covers only their own address. A Direct person still asks this server for
call credentials and still gets an allocation, so a call between a Private person and a Direct
person connects through the forwarder on the Private side. Nothing about the forwarder's rules
changed: it still passes data only between its own allocations of one room or call.

**What it needs from the server: the same one port, nothing more.** The STUN responder answers on
`TURN_PORT` (3478) whenever the forwarder runs, and `/api/turn-credentials` names it
(`stun:<TURN_PUBLIC_HOST>:3478`) only while it does. It answers a Binding request with the address
it came from and nothing else, never larger than the request plus a few dozen bytes, at most 20 a
second per address and 1,000 a second in total. There is no separate switch for STUN: opening the
call port opens both.

Until the port is open, Direct people can still reach each other device to device where their
networks allow it (on one home network, say, or when they also ask a public service), and their
apps tell them the server cannot carry calls yet. Private people's calls still cannot connect.

### The operator's steps to open it (your call; nothing here touches the live servers)

1. Deploy a release that has this change (the version after v0.1543.0 that carries 10p). Check
   the refusal tests on that code first:
   `cargo test --features relay --no-default-features --lib call_`
2. On the VPS, as root, open the port the way the section above describes (it survives reboots
   and later provision runs):
   ```bash
   echo 'HUMANITY_OPEN_CALL_PORT=1' >> /opt/Humanity/node.env && bash /opt/Humanity/scripts/provision-vps.sh
   ```
   The provision script ends by checking that the firewall lets UDP 3478 in and that the relay,
   and nothing else, listens on it; it says `!!` and fails if not.
3. Check on the box: `ss -Hulnp 'sport = :3478'` shows one line, the HumanityOS relay, and
   `curl -s https://united-humanity.us/api/turn-credentials` lists
   `{"urls":"stun:united-humanity.us:3478"}`.
4. Check from outside, from your own PC: in the desktop app choose Settings > Privacy > Calls and
   voice rooms > Direct, join a voice room with someone, and look in
   `%APPDATA%\HumanityOS\logs\run.log` for `WebRTC: learned server-reflexive address ... (via STUN`
   naming the server: that is the STUN responder answering through the firewall. Set it back to
   Private afterwards if you prefer.

To close it again, the same line as above: remove `HUMANITY_OPEN_CALL_PORT` from `node.env` and
run the provision script. Closing it stops calls for Private people and the server's STUN for
Direct people; Direct people on networks that allow it keep reaching each other.

A self-hoster does the same: open UDP `TURN_PORT` on their own firewall or router.

## Files sent device to device

Since 2026-10-12 the same forwarder carries big files sent straight from one person's device to
another's (`docs/design/files-showcase.md`, "Device-to-device big files"). The file never lands on
the server: it passes through the forwarder encrypted, as a call does, and both people see only
the server's address (unless they are friends who both turned on direct connections, when it may
go straight between them). What that means for an operator:

- **It runs only with voice on.** With `features.voice` off the forwarder does not start, a file's
  credentials are refused, and the person sending it is told so in plain words ("its owner turned
  off voice, and the same call relay carries both").
- **Who can get credentials for a file.** Someone holding a file pass for that one transfer (a
  signed permission the SENDER's app puts in its offer, letting the receiver reach the sender for
  that one file, good for at most 7 days), for
  a person this server knows (a registered name, or signed in now). The credentials work for that
  transfer only, and the forwarder passes packets only between the two people's allocations of it.
  A pass is not the other person's consent to be sent anything: what bounds use is the next two
  limits.
- **A day amount per person.** Each role has `max_forwarded_mb_per_day` ("Forwarded a day" in the desktop
  app's Server settings, Roles; the website has no Roles editor). Default 2048 MB for every
  role; 0 = no limit, the built-in Admin role's default. The forwarder counts what each person's file
  allocations send in a UTC day and stops passing their file traffic past it until the next day.
  Calls and voice rooms are never counted. The count lives in memory: a restart starts the day
  again.
- **At most 16 file allocations per person at once** (`MAX_FILE_PER_PERSON`), across all their
  devices. One connection between two people carries any number of files, so this is 16 people
  sending to or receiving from them at the same moment.
- Asking for credentials of any kind counts toward 20 a minute per person, refused requests
  included.
