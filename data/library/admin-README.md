# For admins

This folder is for **people who run a HumanityOS server** (a "relay"), whether for
yourself, a family, or a community. You do not have to be a professional sysadmin. If
you can follow a short recipe in a terminal, you can host one.

## What hosting means here

A server is a **meeting place, not an owner**. It relays messages and stores public
content, but it cannot read anyone's private messages, and people keep their identity
across every server. So running one is low-stakes and high-value: you give your
community a home without becoming its gatekeeper.

## Start here

- **[SELF-HOSTING.md](SELF-HOSTING.md)** run your own relay in under 10 minutes. One
  binary, SQLite built in, no external services required.

## Operations and infrastructure

- **[security-hardening-tasks.md](security-hardening-tasks.md)** the operator-only
  security items from the 2026-06-12 audit (nginx edge rate limit, GitHub branch
  protection, release signing). The code-side fixes are already shipped; these live in
  GitHub settings or the VPS config.
- **[release-signing.md](release-signing.md)** how releases are cryptographically signed
  so the auto-updater only ever installs trusted code. Read this before you publish
  builds.
- **[distribution-mirrors.md](distribution-mirrors.md)** distributing HumanityOS beyond
  GitHub (mirrors, torrents, archives) so it survives any single host going down.
- **[torrent-infrastructure.md](torrent-infrastructure.md)** the BitTorrent seeder setup
  for sovereign, censorship-resistant distribution.
- **[forgejo-setup.md](forgejo-setup.md)** running the self-hosted Forgejo git mirror.

## Related

- Live-ops notes (backup replication and the like) live in
  **[../operations/](../operations/)**.
- Incident recipes for when something breaks are in
  **[../INCIDENT-PLAYBOOK.md](../INCIDENT-PLAYBOOK.md)**.
- The team's operational posture: **[../SECURITY-CADENCE.md](../SECURITY-CADENCE.md)**,
  **[../HEALTH-DASHBOARD.md](../HEALTH-DASHBOARD.md)**, **[../BUS-FACTOR.md](../BUS-FACTOR.md)**.

## A note on the GUI-first goal

A core rule of HumanityOS is that **anything an admin can do should eventually be doable
from inside the app, not only from a terminal**. The CLI recipes here are the ground
truth today; the in-app ops console is being built to put a button on each of them. If
you hit something that forces you into a shell, that is tracked debt, not the intended
end state.
