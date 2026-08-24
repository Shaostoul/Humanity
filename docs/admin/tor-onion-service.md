# Tor Onion Service for a HumanityOS relay

**Status:** optional operator infrastructure (2026-08-24). Closes the one
privacy gap the application layer cannot: transport-level IP address exposure.

## Why

Everything above the socket is now sealed: messages are end-to-end encrypted,
the server stores no DM social graph, no follows graph, no readable marketplace
or group chat, and presence can be hidden. But any server on the internet sees
the IP address of a socket connected to it in the moment. We don't tie IPs to
identity or retain them (nginx logs are cut to 2 days for fail2ban only), and
that is a real minimization, but it is not zero.

A Tor onion service makes it zero for users who want it. When a user reaches the
relay over its `.onion` address, the relay literally never learns their IP:
Tor's rendezvous circuit means neither end knows the other's network location.
This is the honest answer to "can a wiretap or a hostile operator see who
connected" — over the onion service, there is nothing to see.

This is opt-in and additive. The clearnet `https://` endpoint keeps working
exactly as before; the onion address is a second door for people who want
transport anonymity, and it costs the operator nothing but a few minutes.

## Setup (Debian VPS, ~10 minutes)

```bash
sudo apt-get install -y tor
```

Add to `/etc/tor/torrc`:

```
HiddenServiceDir /var/lib/tor/humanity/
HiddenServicePort 80 127.0.0.1:8080
HiddenServiceVersion 3
```

(Port 8080 is the relay's local listener. If you terminate TLS at nginx,
point the onion service at nginx's plain-HTTP upstream instead; onion
services are already end-to-end encrypted by Tor, so a second TLS layer is
optional.)

```bash
sudo systemctl restart tor
sudo cat /var/lib/tor/humanity/hostname   # your <56-char>.onion address
```

Publish that `.onion` address alongside the clearnet URL. The
`scripts/tor-onion-setup.sh` helper does the above and prints the address.

## What the user does

Open the relay's `.onion` address in Tor Browser (or any Tor-enabled client).
Identity, keys, DMs, and everything else work identically; the only difference
is the relay cannot see where they are. A user can keep the same identity across
the clearnet and onion doors, because identity is a key, not an account tied to
an address.

## Honest limits

- The onion service protects the connection to THIS relay. Federated servers a
  user's data reaches by other paths are separate.
- Running Tor on the VPS adds a background daemon; it is lightweight but it is a
  moving part to keep patched.
- Onion latency is higher than clearnet. That is the trade for location privacy;
  most chat interactions are well within tolerance.
- This does not anonymize the operator or the server, only the users connecting
  in over the onion address.

## GUI-first debt

Surfacing the `.onion` address in the app (Server Settings, and the launcher's
server-detail pane) so users can copy it without SSH is tracked in
`docs/design/in-app-ops.md`. The relay could read
`/var/lib/tor/humanity/hostname` through the sudo-gated system bridge and expose
it as a read-only server-info field.
