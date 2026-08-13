# Site resilience: staying reachable when the host goes down

> Created 2026-08-13, the day Namecheap took the VPS (and the site) dark for
> maintenance, one week after the TURN null-route outage. Decision (operator):
> three layers, strongest-together: a free full-site mirror on GitHub Pages
> (LIVE, automated), Cloudflare free tier in front of the domain (operator
> setup below), and a second VPS at a different provider (purchase + provision
> below). The honest scope note: these keep the WEBSITE reachable; live chat
> and the API need a second running relay, which is the federation-repair work
> ranked in PRIORITIES.md. These layers buy availability while that matures.

## Layer 1: GitHub Pages mirror (LIVE since 2026-08-13, zero maintenance)

- What: the full web frontend auto-published to https://shaostoul.github.io/
  from the [Shaostoul/shaostoul.github.io](https://github.com/Shaostoul/shaostoul.github.io) repo.
- How: an hourly PULL-based workflow there checks out the public Humanity
  repo and runs `scripts/sync-web-root.sh` (the ONE web-layout definition CI
  and the provisioner also use, so the mirror cannot drift structurally),
  then publishes via Pages. No cross-repo secrets exist.
- Instant refresh after a big release: Actions tab in that repo, "Mirror
  united-humanity.us", Run workflow. (Or `gh workflow run mirror.yml
  --repo Shaostoul/shaostoul.github.io`.)
- On the mirror, `web/shared/shell.js` shows a one-line banner (gated on
  `*.github.io` hostnames) telling visitors live features need the primary.
  Downloads work fully (they point at GitHub Releases).
- Failure independence: GitHub Pages shares nothing with Namecheap. If
  GitHub itself is down, releases and the mirror go together; that is what
  the torrent/WebSeed layer (docs/admin/torrent-infrastructure.md) and the
  second VPS are for.

## Layer 2: Cloudflare free tier (DECLINED, operator decision 2026-08-13)

> The operator declined this layer on principle: "I'm avoiding cloudflare.
> I don't want that single point of failure." The answer to one company
> being able to take the site down should not be a different company in
> front of everything. The section below is KEPT for reference in case the
> tradeoff is ever revisited; the chosen path instead is federation (both
> relays replicate; the app's server picker fails over) plus the mirror,
> with DNS diversity as a possible future step (moving united-humanity.us
> DNS to a host independent of the VPS provider, e.g. DreamHost's free DNS,
> so registrar-panel outages cannot block record changes mid-incident).

### Reference: what the Cloudflare layer would have provided (~30 minutes)

Why: today's outage was survivable by no DNS trick alone; Cloudflare's
anycast DNS + edge cache means (a) DNS stops depending on Namecheap, (b)
static pages keep serving FROM CACHE ("Always Online") while the origin is
dark, (c) the origin IP is hidden, (d) failover to the mirror becomes a
one-call origin swap the uptime monitor can automate later.

Steps (all in the operator's accounts; an AI cannot and must not do these):

1. Create the free account at cloudflare.com (Free plan is enough).
2. "Add a site": united-humanity.us. Cloudflare scans existing DNS records;
   VERIFY against the real set before continuing:
   - `A @ 203.161.61.222` (the VPS) - PROXIED (orange cloud)
   - `CNAME www -> united-humanity.us` - PROXIED
   - `A git 203.161.61.222` (Forgejo) - **DNS ONLY (grey cloud)**: the forge
     remote uses SSH on this hostname and Cloudflare's proxy does not carry
     SSH. Leave grey or `git push forge` breaks.
   - Any mail records (MX/TXT/SPF): copy exactly, DNS only.
3. At Namecheap (Domain > Nameservers): switch to "Custom DNS" and paste the
   two Cloudflare nameservers it assigns. Propagation: minutes to hours.
4. In Cloudflare, set: SSL/TLS mode **Full (strict)** (the origin has real
   Let's Encrypt certs; "Flexible" would silently break the API); Speed >
   Optimization defaults are fine; Caching > Configuration > enable
   **Always Online**; SSL/TLS > Edge Certificates > Always Use HTTPS on.
5. Nothing on the VPS changes. WebSockets (/ws chat) pass through the proxy
   automatically. Certbot HTTP-01 renewal keeps working under Full (strict).
6. Tell the session AI it is live; the uptime workflow's site probe then
   watches the edge, and a follow-up increment can add automated
   origin-failover to the mirror via the Cloudflare API (needs an API token
   stored as a GitHub Actions secret, operator-created, scope: zone DNS +
   page rules for this zone only).

Philosophy note, recorded on purpose: this puts a centralized third party
in front of a decentralization project. It is a pragmatic availability
layer while federation matures, not the destination; the second VPS +
repaired federation is the real answer, and Cloudflare can be removed by
switching nameservers back at any time.

## Layer 3: second VPS at a different provider (OPERATOR PURCHASE, then one command)

- Pick a provider with no shared failure domain with Namecheap: Hetzner
  (CX22, ~4.5 EUR/mo, Falkenstein/Helsinki), OVH, or Racknerd all fit. Any
  Debian 12 x64 box with 2 GB RAM works (the relay idles at ~20 MB RSS).
- Boot Debian 12, add the operator SSH key, then from this repo:
  1. Copy `ops/vps/node.env.example` to `node.env` on the box and set the
     3 domain lines (this is the domain-parametric seam built 2026-08-11;
     for a pure mirror, a subdomain like `mirror.united-humanity.us` or a
     sibling domain both work).
  2. Run `scripts/provision-vps.sh`. It installs nginx, certs, the relay,
     backups, and the assertions; it is idempotent and fetches the prebuilt
     relay binary (sha256-verified) instead of compiling.
- Roles, in order of ambition:
  a. STATIC MIRROR + off-site backup target now (rsync the web root +
     receive the DB backup pulls, so backups stop being Windows-PC-only).
  b. SECOND RELAY once federation's three never-ran-live defects are fixed
     (the ranked federation item in PRIORITIES.md): then chat/profiles
     replicate and the platform itself, not just the website, survives a
     provider outage.
- DNS: with Cloudflare in front, the second origin is a standby A record /
  load-balancing rule; without it, a manual record flip at the registrar.

## Failover procedures (until automation exists)

- PRIMARY DOWN, Cloudflare live: nothing to do for static pages (Always
  Online serves cache). For a long outage, point the `@` A record at the
  second VPS (or a Pages-redirect worker) in the Cloudflare dashboard;
  revert when the primary returns.
- PRIMARY DOWN, no Cloudflare yet (today's world): the mirror is always at
  https://shaostoul.github.io/ ; post that link (Discord, X) during
  outages. DNS flips at Namecheap are NOT worth it mid-outage (slow TTL,
  manual, and their panel may be part of the maintenance).
- Detection: the uptime workflow (.github/workflows/uptime.yml) probes
  every ~15 min from GitHub's network and is how both 2026-08 outages were
  pinned to the minute. Its red runs are the signal; the session AI arms a
  recovery watch and re-runs the failed deploy legs when the box returns.

## Related

- docs/INCIDENT-PLAYBOOK.md (the TURN abuse + this maintenance outage)
- docs/admin/distribution-mirrors.md + torrent-infrastructure.md (release
  file availability, WebSeed fallback)
- PRIORITIES.md "self-host / federation" block (the real long-term fix)
