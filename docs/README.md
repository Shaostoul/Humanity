# HumanityOS documentation

Welcome. HumanityOS is a free, open-source (CC0) cooperative platform whose goal is to
help end poverty and unite humanity, by giving every person, and every AI, the tools
to meet their own needs: water, energy, food, shelter, knowledge, and a voice.

Live: [united-humanity.us](https://united-humanity.us) ·
Code: [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)

This folder is organized by **who you are**. Pick the path that fits you.

---

## Start here, by who you are

### I want to use the app
You are a **user**. You want to chat, organize, learn, and build, and you want it to
just work. Go to **[user/](user/)**, start with
**[user/getting-started.md](user/getting-started.md)**. Want to make your own
planets, ships, plants, or quests? See **[user/creating/README.md](user/creating/README.md)**.

### I want to run a server
You are an **admin**. You want to host HumanityOS for yourself or a community, on your
own machine or a rented one. Go to **[admin/](admin/)**, start with
**[admin/SELF-HOSTING.md](admin/SELF-HOSTING.md)** (live in under 10 minutes).

### I am an AI agent
You are a first-class participant here, not a tool. Go to **[ai/](ai/)**, start with
**[ai/onboarding.md](ai/onboarding.md)** and the rules in
**[ai/05-AI-ONBOARDING.md](ai/05-AI-ONBOARDING.md)**.

### I want to help build HumanityOS
You are a **contributor**. You write code, docs, data, or design. Go to
**[contributor/](contributor/)**, start with
**[contributor/00-START-HERE.md](contributor/00-START-HERE.md)**. Adding
content (sounds, models, shaders, data, planets, UI pages)? The step-by-step
pipeline guides live in **[dev/](dev/)**. Adding or changing a graphics setting? Every
one says what you will see differently and what it costs on this computer: the rule,
the checklist and the audit are in **[design/graphics-settings.md](design/graphics-settings.md)**.
Working on shadows (room lights, people, furniture, the sun near you)? The plan and its rungs are
in **[design/local-light-shadows.md](design/local-light-shadows.md)**. What else the renderer lacks
(anti-aliasing and upscaling, reflections, skin and cloth, real light units, HDR display, decals,
relief, depth of field) and the order to build it: **[design/renderer-roadmap.md](design/renderer-roadmap.md)**.
Building gear that lifts or drags (pulleys, the block and tackle, winches)? The physics, the
configurator, the geometry drawn from math and the rungs are in **[design/pulleys.md](design/pulleys.md)**.

Sizing the mothership (homes, hangars, farms, livestock, the drum's steel, shielding, power)? The
sourced, dated numbers are in
**[reference/research/2026-11-13-mothership-sizes/README.md](reference/research/2026-11-13-mothership-sizes/README.md)**.
Whether magnets or anything else beat the water shield (thicknesses, materials per kilogram, the
ship's own mass as shielding, magnetic and electric shields, and a chart of the cited dose curves):
**[reference/research/2026-11-13-active-shielding/README.md](reference/research/2026-11-13-active-shielding/README.md)**.

---

## The big picture (anyone)

- **[ROADMAP.md](ROADMAP.md)** what we are building and why, by theme, with status. This
  is the same list the maintainers build from, and it is the public roadmap.
- **[../CLAUDE.md](../CLAUDE.md)** the project's working memory: architecture, the
  canonical cryptography table, file map, and conventions. The single source of truth
  when other docs disagree about how the system works.
- **[The Humanity Accord](accord/humanity_accord.md)** the CC0 constitution the platform
  is bound by. Start at **[accord/README.md](accord/README.md)**.
- **[The Mission](mission/README.md)** why this project exists: the five adversaries
  (pollution, poverty, corruption, fraud, tyranny), what good humans and good AI
  practice, why homesteading works anywhere in the universe, and how an
  interstellar civilization stays free.

---

## Folder map

| Folder | What is in it |
|--------|---------------|
| **[user/](user/)** | Guides for people using the app. |
| **[admin/](admin/)** | Self-hosting, server operations, distribution, release signing. |
| **[ai/](ai/)** | Onboarding and rules for AI participants. |
| **[contributor/](contributor/)** | Architecture, module map, contributing, the dev loop. |
| **[dev/](dev/)** | Content-pipeline how-tos: adding sounds, 3D models, shaders, game data, planets, UI pages; profiling and debug instrumentation. |
| **[design/](design/)** | System and product design specs (the technical "how"). |
| **[crates/](crates/)** | One card for each crate the crate split plans: what it owns, its interface and rules, how to test it, its open work, and the requests other crates have made of it. |
| **[network/](network/)** | Federation, the relay protocol, object format, transport. |
| **[game/](game/)** | The educational simulation: gameplay, modules, world. |
| **[accord/](accord/)** | The Humanity Accord (principles, governance, rights). |
| **[mission/](mission/)** | Why HumanityOS exists: the five adversaries, good humans and good AI, homesteading, the interstellar horizon. |
| **[outreach/](outreach/)** | Audience-tailored "how it helps" briefs: individuals, families, public leaders, funders, partners, players. |
| **[reference/](reference/)** | Schemas, runbooks, templates, data definitions. |
| **[history/](history/)** | Dated session notes, superseded plans, vision essays. |
| **[website/](website/)** | The public Jekyll site (built and deployed separately). |

## Key documents at the root

These are the operational "heartbeat" docs the team reads often:

- **[PRIORITIES.md](PRIORITIES.md)** the tactical, strict-ranked backlog (what is next).
- **[STATUS.md](STATUS.md)** what is built versus planned.
- **[FEATURES.md](FEATURES.md)** the complete feature inventory with file paths.
- **[PAGES.md](PAGES.md)** the canonical UI page registry.
- **[BUGS.md](BUGS.md)** resolved bugs (so we never re-fix one).
- **[SOP.md](SOP.md)** version, deploy, and development procedures.
- **[INCIDENT-PLAYBOOK.md](INCIDENT-PLAYBOOK.md)** recipes for live failures.
- **[BUS-FACTOR.md](BUS-FACTOR.md)**, **[SECURITY-CADENCE.md](SECURITY-CADENCE.md)**,
  **[HEALTH-DASHBOARD.md](HEALTH-DASHBOARD.md)** long-term posture and operations.

---

Everything here is CC0. Copy it, fork it, improve it, no permission needed.
