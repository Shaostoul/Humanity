# Rotating the TURN credential

> **Why this exists:** until v0.857 the TURN password was a static string committed in
> the clients (`src/net/webrtc.rs` and `web/chat/chat-voice-rooms.js`). Anyone who read
> the repo or the served JS could use the TURN relay as free bandwidth. v0.857 moved the
> clients to short-lived credentials issued by the relay's `/api/turn-credentials`, so no
> secret ships to clients anymore. This doc is the one-time migration that ALSO revokes
> the old committed password. After this, rotating is a one-line secret change.

## How it works now

coturn's REST-API auth (`use-auth-secret`): the relay hands each client a credential that
is `username = <expiry-unix-timestamp>` and `credential = base64(HMAC-SHA1(secret, username))`.
coturn recomputes the same HMAC from its `static-auth-secret` and accepts it until the
expiry. The **secret lives only on the server** (coturn's config + the relay's
`TURN_STATIC_SECRET` env, which must be the SAME value) and is never sent to a client.

If `TURN_STATIC_SECRET` is unset, `/api/turn-credentials` returns STUN-only and voice still
works for everyone except symmetric-NAT peers, so the migration cannot hard-break voice.

## The migration (one time, ~5 minutes, on the VPS)

All on `humanity-vps`. The secret is generated on the VPS and never leaves it.

```sh
# 1. Generate a strong shared secret.
SECRET=$(openssl rand -hex 32)
echo "$SECRET"   # note it; you set it in two places below

# 2. Point the relay at it (EnvironmentFile the systemd unit already loads).
echo "TURN_STATIC_SECRET=$SECRET" | sudo tee -a /opt/Humanity/.env
sudo systemctl restart humanity-relay

# 3. Switch coturn to secret-based auth AND revoke the old static user.
sudo sed -i \
  -e 's/^user=humanity:.*/# user= (revoked v0.857 - replaced by use-auth-secret)/' \
  -e "\$a use-auth-secret\nstatic-auth-secret=$SECRET" \
  /etc/turnserver.conf
sudo systemctl restart coturn
```

`realm=united-humanity.us` and `lt-cred-mech` stay as they are (the REST API is built on
the long-term-credential mechanism, so both are still needed).

## Verify

```sh
# The endpoint should now include turn:/turns: entries WITH a credential:
curl -s https://united-humanity.us/api/turn-credentials
```

Then confirm a real call still connects: start a voice room on the web chat AND from the
native app, and check both sides get audio. If a peer is behind symmetric NAT the call now
relays through TURN using the fresh credential.

## Rotating again later (routine)

Just change the secret in BOTH places and restart both services:

```sh
SECRET=$(openssl rand -hex 32)
sudo sed -i "s/^TURN_STATIC_SECRET=.*/TURN_STATIC_SECRET=$SECRET/" /opt/Humanity/.env
sudo sed -i "s/^static-auth-secret=.*/static-auth-secret=$SECRET/" /etc/turnserver.conf
sudo systemctl restart humanity-relay coturn
```

No code change, no deploy. Every previously issued credential stops working immediately.

## Note on git history

The old password (`turnRelay2026!secure`) remains in git history. Revoking it in coturn
(step 3 above) makes it worthless, which is the point. A history rewrite is not worth the
disruption for a credential that is now inert; if you ever want one, that is a separate,
deliberate `git filter-repo` exercise coordinated with every clone.
