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
