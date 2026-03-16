# HumanityOS Staged Download + Trusted Seeding Model

## Naming

- Product name: **HumanityOS**
- Accepted abbreviation: **HOS**

## Goals

1. Fast first install (small initial download)
2. Optional large game content download from in-app menu
3. Verifiable trust chain for malware-resistant distribution
4. Multi-source seeding (official + community-controlled)
5. User-controlled seeding with bandwidth limits, default **off**

---

## Two-Stage Install Strategy

## Stage 1 — Core Runtime (required)

Includes:
- desktop runtime shell
- account/profile/systems UI
- updater + verifier
- minimal baseline assets and docs
- no heavy game planet/content packs

Outcome:
- app is usable quickly
- user can reach Download Manager immediately

## Stage 2 — Expanded Content Packs (optional)

User opens **Download Manager** and chooses packs:
- base game environment pack
- high-res terrain/biome packs
- voice/audio/cinematic packs (optional)

Each pack includes:
- version
- hash
- signature metadata
- size and dependencies

---

## Trusted Distribution + Hash Model

Primary trust source:
- `united-humanity.us` (or updates subdomain) publishes signed manifests + hashes

Client verification flow:
1. fetch signed manifest
2. verify signature (trusted embedded pubkey)
3. download package from one or more mirrors/seeds
4. verify cryptographic hash and size
5. install only on exact hash match

If hash mismatch:
- block install
- show integrity warning
- retry from alternate source

---

## Multi-Source Seeding

Supported distribution sources:
- official VPS endpoints
- GitHub releases (fallback)
- other controlled mirrors (object storage/CDN)
- optional P2P swarm seeding for published hashes

Rule:
- transport source may vary; accepted package content is fixed by signed hash manifest

---

## In-App Seeding Controls

Setting: **"Seed verified HumanityOS packages"**
- default: **Off**
- user can enable manually
- per-source and global bandwidth caps
- upload schedule options (always / idle-only / night-only)
- show current seeding stats and source package hashes

Security constraints:
- seed only verified packages with known signed hashes
- never seed unknown local files

---

## UX Outline

Download menu should show:
- installed packs
- available packs
- required dependencies
- download size
- hash status (verified/unverified)
- seeding toggle and cap controls

---

## Immediate implementation steps

1. Add staged pack manifest schema (`core` vs `optional content packs`)
2. Add Download Manager UI tab in desktop/web shell
3. Add package verifier service (sig + hash)
4. Add seeding settings model (off by default)
5. Add source priority/fallback logic (official -> mirror -> github)
