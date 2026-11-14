# Open alternatives to Microsoft's everyday services, part 2: identity to AI

What this is: a scan of the open projects that meet the same everyday needs as Microsoft's
sign-in, password, device, browser, search, maps, translation, data, automation, malware,
code-hosting, AI-assistant and professional-network services, and how each could fit
HumanityOS. Part 1 (productivity and communication) is the file beside this one.

Read on 14 November 2026 (Pacific).

Method: GitHub facts (licence, licence file, README lines, latest release, last push, main
language) were read through the GitHub API with `gh api`, which reads the record and downloads no
program. crates.io facts came from its API at one request per second. Web pages were read with
curl as "Claude-User (research tool)", each site's robots.txt first, in a step of its own.
Quotations are exact (curly quotes may be normalised to straight ones); a quotation's source is the
file or page named beside it.

**This is an AI's reading of each licence, not legal advice.** Every licence is quoted so a person
can check it against the source.

Words used below:
- **Licence fit.** HumanityOS is CC0-1.0 (public domain) and ships as ONE program. Code under the
  GPL or AGPL ("copyleft": anything built from it must carry the same licence) cannot be linked
  inside that one program without changing the whole program's licence. Permissive licences (MIT,
  Apache-2.0, BSD, ISC, Zlib) can. MPL-2.0 is copyleft per file: its own files stay MPL (and their
  source stays available), while our files stay CC0.
- **Fit**: "embed" = a library inside our Rust app; "beside" = a separate program next to ours;
  "bridge" = we speak its open protocol or file format; "point" = we only link to it from our Apps
  page.
- **NOASSERTION** is what GitHub's licence detector prints when a repository does not hold one
  plain standard licence (mixed, dual or custom licensing). The quotation then says what it is.
- **Open core**: most code is open, but some folders are under a commercial licence.
- **Fair-code / source-available**: you can read the code, but the licence limits use (usually
  commercial use), so it is not open source.
- **Self-host / server you host**: a program run on a server (or a home PC) that people reach over
  the network.

## In plain words

- **Strongest "embed" fits are small permissive Rust libraries:** `totp-rs` (MIT) for sign-in
  codes, `passkey` from 1Password (MIT OR Apache-2.0) for passkeys, the `keepass` crate (MIT) to
  read and write KeePassXC files, and `candle` from Hugging Face (MIT OR Apache-2.0), which runs
  the OPUS-MT translation models in Rust.
- **MPL-2.0 Rust projects can sit inside our program with care:** Servo (an embeddable browser
  engine), Kanidm and `webauthn-rs` (passkeys). Their files stay MPL; ours stay CC0.
- **Copyleft servers fit "beside":** Vaultwarden (Rust, a Bitwarden-compatible password server),
  SearXNG (private search), LibreTranslate (offline translation), AdGuard Home (family DNS filter
  with a parental-control switch), Grafana, ClamAV, and Forgejo (which the project already runs as
  a pull mirror at git.united-humanity.us, per project memory).
- **Licence traps:** n8n and NocoDB use the "Sustainable Use License" (n8n's own docs ask "Why
  don't you use an open source license?"); Open WebUI forbids removing its branding above 50 users;
  Bitwarden's SDK and commercial modules are not open; Fleet, Metabase, authentik, Baserow and
  Activepieces are open core.
- **Local AI:** llama.cpp, Ollama and mistral.rs (Rust) are MIT, so the licence would allow
  embedding, but the operator's AI-optional rule makes any model runtime a separate, chosen
  download, so they fit "beside".
- **Gaps:** no mature open screen-time or parental-control app for Windows was found (TimeLimit is
  Android only); no mature open, federated LinkedIn alternative exists (only one-star projects).

## Passwords and sign-in codes

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| KeePassXC | KeePassXC Team | NOASSERTION; COPYING: "either version 2 or (at your option) version 3 of the License" (GNU GPL) | Desktop program: "You can run KeePassXC on Windows, macOS, and Linux systems." (README); hardware not stated | C++ | 2.8.0, 2026-10-21 | bridge: read and write its KDBX file with the MIT `keepass` crate; the app itself stays beside |
| Bitwarden server | Bitwarden, Inc. | NOASSERTION; LICENSE.txt: "covered by one of two licenses: (i) the GNU Affero General Public License (AGPL) v3.0 (ii) the Bitwarden License v1.0"; "Bitwarden Licensed code is found only in the /bitwarden_license directory."; LICENSE_BITWARDEN.txt: "GOVERNS YOUR USE OF THE COMMERCIAL MODULES" | Server you host; hardware not read | C# | v2026.10.1, 2026-10-28 | point (AGPL plus commercial modules) |
| Bitwarden clients | Bitwarden, Inc. | NOASSERTION; LICENSE.txt: "(i) the GNU General Public License (GPL) v3.0 (ii) the Bitwarden License v1.0" | Browser extension, desktop, web, command line; "all Bitwarden client applications except the mobile applications" (README; Android app repo is GPL-3.0) | TypeScript | desktop-v2026.10.4, 2026-11-05 | point |
| Bitwarden SDK (sdk-internal) | Bitwarden, Inc. | NOASSERTION; LICENSE: "your choice of GPL v3.0 OR BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE"; folders named bitwarden_license are "covered solely by the BITWARDEN SOFTWARE DEVELOPMENT KIT LICENSE" | Library | Rust | rust-v4.0.0, 2026-09-28 | point: Rust, but GPL or a proprietary SDK licence, so not embeddable in a CC0 program |
| Vaultwarden | Daniel García (dani-garcia) and contributors | AGPL-3.0; LICENSE.txt: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Server you host: "perfect for self-hosted deployment where running the official resource-heavy service might not be ideal." (README); hardware not stated | Rust | 1.37.5, 2026-11-01 | beside, or bridge through the Bitwarden client API it speaks; Rust but AGPL, so not embed |
| Aegis Authenticator | Beem Development | GPL-3.0; LICENSE: "GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007" | Phone app: "A free, secure and open source app for Android to manage your 2-step verification tokens." (repo description) | Java | v3.4.3, 2026-09-06 | point |
| ente Auth | Ente (repo github.com/ente/ente, the whole monorepo) | AGPL-3.0; LICENSE: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Phone app (App Store link in README); "Ente Auth is free, and will remain free forever." (README); other platforms not checked | Dart | auth-v4.4.26, 2026-10-19 | point |
| 2FAS Auth (added: a widely used open authenticator) | 2FAS | GPL-3.0; LICENSE: "GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007" | Android phone app ("Source code for 2FAS Auth Android app", repo description); iOS in a separate repo | Kotlin | 6.1.0, 2026-10-19 | point |
| totp-rs (crate) | Cléo Rebert (constantoine) | MIT; crates.io licence field "MIT"; LICENSE: "MIT License" | Library: "RFC-compliant TOTP implementation" (repo description) | Rust | 6.0.0, 2026-08-06 | embed (Rust) |
| totp-lite (crate) | Colin Woodbury (fosskers) | MIT; crates.io "MIT"; LICENSE: "MIT License" | Library | Rust | 2.0.1, 2023-10-30 (repo last push 2026-06-23) | embed (Rust; smaller, older) |
| keepass (crate) | Stefan Seemayer | MIT; crates.io "MIT"; LICENSE: "MIT License" | Library: "Rust KeePass database file parser for KDB, KDBX3 and KDBX4, with experimental support for KDBX4 writing." (repo description) | Rust | 0.17.0, 2026-10-19 | embed (Rust): the bridge to KeePassXC files |
| webauthn-rs (crate, passkeys) | Kanidm project | MPL-2.0; crates.io "MPL-2.0"; LICENSE.md: "Mozilla Public License Version 2.0" | Library | Rust | 0.6.2, 2026-10-31 | embed (Rust; its files stay MPL) |
| passkey (crate, passkeys) | 1Password | crates.io "MIT OR Apache-2.0"; LICENSE-APACHE: "Apache License Version 2.0, January 2004" | Library | Rust | 0.6.0, 2026-10-01 | embed (Rust) |

## Identity and single sign-on

"Single sign-on" means one login that many separate apps accept. These are all servers an
organisation runs; HumanityOS has no home server and no sign-up, so the realistic fit is to speak
their open standards (OpenID Connect, SAML, WebAuthn) where a person wants HumanityOS to talk to
them.

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Keycloak | Keycloak project (GitHub org keycloak) | Apache-2.0; LICENSE.txt: "Apache License Version 2.0, January 2004" | Server you host: "Open Source Identity and Access Management For Modern Applications and Services" (repo description); hardware not stated | Java | 26.8.2, 2026-11-05 | beside / bridge (OpenID Connect, SAML) |
| authentik | Authentik Security Inc. (licence: "Copyright (c) 2023 Jens Langhammer") | NOASSERTION, open core; LICENSE: "authentik/enterprise/" content "is licensed under the license defined in "authentik/enterprise/LICENSE"", and "Content outside of the above mentioned directories or restrictions above is available under the "MIT" license" | Server you host: "A host with at least 2 CPU cores and 2 GB of RAM" (docs.goauthentik.io/install-config/install/docker-compose/, undated page, docs version 2026.8) | Python | version/2026.8.5, 2026-10-21 | beside |
| Kanidm | Kanidm project | MPL-2.0; LICENSE.md: "Mozilla Public License Version 2.0" | Server you host; "Linux/Unix integration with TPM protected offline authentication" (README); hardware not stated | Rust | v1.12.1, 2026-11-13 | beside; its client crate `kanidm_client` (crates.io "MPL-2.0", 1.12.1) could embed |
| ZITADEL | ZITADEL (GitHub org zitadel) | AGPL-3.0; LICENSE: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Server you host: "Identity infrastructure, simplified for you." (repo description); hardware not stated | Go | v4.20.3, 2026-11-13 | beside |

## Devices and family safety

"MDM" (mobile device management) means a server that sets up, updates and locks an
organisation's phones and computers.

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Fleet | Fleet Device Management Inc | NOASSERTION, open core; LICENSE: content under "ee/" "is licensed under the license defined in "ee/LICENSE"", and "Content outside of the above mentioned directories or restrictions above is available under the "MIT Expat" license" | Server you host plus an agent on each device: "Fleet is lightweight and modular." (README) | Go | fleet-v4.94.1, 2026-11-06 | point |
| NanoMDM | MicroMDM project (licence: "Copyright 2021 Jesse Peterson") | MIT; LICENSE: "Permission is hereby granted, free of charge, to any person obtaining a copy of this software" | Server you host for Apple devices: "a minimalist Apple MDM server and library" (repo description) | Go | v0.9.0, 2025-12-25 | point |
| MicroMDM | MicroMDM project | MIT; LICENSE: "MIT License" | Server you host for Apple devices: "Mobile Device Management server" (repo description) | Go | v1.13.1, 2025-10-17 | point |
| Headwind MDM | Headwind Solutions (h-mdm) | Apache-2.0; LICENSE: "Apache License Version 2.0, January 2004" | Server plus Android agent: "Mobile Device Management System for Android: web control panel." (repo description) | Java | v5.41.4, 2026-11-06 | point |
| AdGuard Home (added: family filter) | AdGuard (AdguardTeam) | GPL-3.0; LICENSE.txt: "GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007" | Server on the home network: "Network-wide ads & trackers blocking DNS server" (repo description); README feature table lists "Parental control (blocking adult domains)" and "Force Safe search on search engines"; hardware not stated | TypeScript (GitHub's top language) | v0.107.80, 2026-11-03 | beside |
| Pi-hole (added) | Pi-hole, LLC | NOASSERTION; LICENSE: "This software is licensed under the European Union Public License (EUPL)", text "EUROPEAN UNION PUBLIC LICENCE v. 1.2" | Linux server: "runs smoothly with minimal hardware and software requirements" (README) | Shell | v6.6, 2026-11-11 | beside |
| TimeLimit | TimeLimit (timelimit.io; imprint at jolo.software) | Not read: the source is on Codeberg and the F-Droid listing, both barred by robots.txt. The site says: "You can find the source code of the Client here." (timelimit.io/en/, undated page) | Phone app: "Limit the usage of Android&trade; phones easily"; the linked mode is "14 days for free when using the connected mode; after that an automatically downgrade to the basic connected mode" (timelimit.io/en/) | Not read | Not read | point |

Honest note: TimeLimit is the only open screen-time app found, and it is Android only. On Windows
the open options found are network filters (AdGuard Home, Pi-hole), not per-child screen-time
limits; nothing mature was found for that.

## Browser, search and maps

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Firefox | Mozilla | NOASSERTION; LICENSE: "Please see the file toolkit/content/license.html"; that page's template (toolkit/content/license.html.mako): "most of the source code is available under the Mozilla Public License 2.0 (MPL)." | Browser program (platform list not read) | JavaScript (GitHub's top language) | No GitHub releases; last push 2026-11-14 | point |
| Chromium | The Chromium Authors | BSD-3-Clause; LICENSE: "// Copyright 2015 The Chromium Authors" then "Redistribution and use in source and binary forms, with or without" | Browser engine (platforms not read) | Not reported by GitHub for this mirror | No GitHub releases; last push 2026-11-14 | beside: already the agreed route for watching video platforms only, as a separate opt-in process (CLAUDE.md) |
| Ladybird | The Ladybird developers | BSD-2-Clause; LICENSE: "BSD 2-Clause License" | "Ladybird runs on Linux, macOS, Windows (with WSL2), and many other \*Nixes."; "Ladybird is in a pre-alpha state, and only suitable for use by developers" (README) | Rust and C++ (GitHub's byte count: Rust 42.0 MB, C++ 39.5 MB) | No releases; last push 2026-11-14 | point today (pre-alpha); permissive, so a future embed candidate |
| Servo | Servo project | MPL-2.0; LICENSE: "Mozilla Public License Version 2.0" | A library to embed: "a lightweight, high-performance alternative for embedding web technologies in applications" (repo description); "It is currently developed on 64-bit macOS, 64-bit Linux, 64-bit Windows, 64-bit OpenHarmony, and Android." (README) | Rust | v0.8.0, 2026-11-09 | embed (Rust; its files stay MPL); compare against our own web reader before adopting |
| SearXNG | SearXNG project | AGPL-3.0; LICENSE: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Server you host: "Users are neither tracked nor profiled." (repo description) | Python | No releases (rolling); last push 2026-11-14 | beside / bridge (ask an instance for results over HTTP) |
| YaCy | YaCy project | NOASSERTION; COPYRIGHT: "GNU Lesser General Public License (files in source/net/yacy/cora) or the GNU General Public License (all other files)", "either version 2 of the License, or (at your option)" later | Peer-to-peer search server: "Distributed Peer-to-Peer Web Search Engine and Intranet Search Appliance" (repo description) | Java | No releases; last push 2026-11-03 | beside |
| Stract (checked) | StractOrg | AGPL-3.0 | Was an open web search engine | Rust | Archived; last push 2025-03-24 | none: archived |
| OpenStreetMap (data) | OpenStreetMap Foundation | ODbL: "OpenStreetMap is open data, licensed under the Open Data Commons Open Database License (ODbL) by the OpenStreetMap Foundation (OSMF)."; "If you alter or build upon our data, you may distribute the result only under the same license." (openstreetmap.org/copyright, undated page) | Map data | n/a | Continuous | bridge: use the data with credit; a database built from it must stay ODbL |
| Organic Maps | Organic Maps community | NOASSERTION; LICENSE: "Apache License Version 2.0, January 2004" | Phone app: "a free Android & iOS offline maps app" that "uses crowd-sourced OpenStreetMap data" (repo description) | C++ | 2026.11.11-39-android, 2026-11-11 | point |

## Translation

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| LibreTranslate | LibreTranslate project | AGPL-3.0; LICENSE: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Server you host: "Self-hosted, offline capable and easy to setup." (repo description); hardware not stated in README | Python | v1.9.6, 2026-05-26 | beside / bridge (HTTP API) |
| Argos Translate | Argos Open Technologies, LLC | MIT; LICENSE: "MIT License" | "Open-source offline translation library" (repo description); CPU by default, GPU optional: "To enable GPU support, you need to set the `ARGOS_DEVICE_TYPE` env variable to `cuda` or `auto`." (README); model size not stated | Python | v1.4.0, 2021-08-24 (last push 2026-08-08) | beside (Python); LibreTranslate's engine: "This work is largely possible thanks to Argos Translate" (LibreTranslate README) |
| Bergamot translator | Bergamot project (browsermt) | MPL-2.0; LICENSE: "Mozilla Public License Version 2.0" | Library: "optimized machine translation on the consumer-grade device" (repo description); also builds to WebAssembly (README) | C++ | v0.4.5, 2022-06-21 (last push 2024-05-12) | embed possible through a C++ binding (MPL per file), but quiet since 2024 |
| Firefox Translations models and pipeline | Mozilla | MPL-2.0; LICENSE: "Mozilla Public License Version 2.0" (models repo; the pipeline mozilla/translations is MPL-2.0 too) | Models are "CPU-optimized"; sizes: "tiny" is "the fastest and the smallest models but with lower translation quality", "base" is "best quality but slower and larger" (models README) | Ruby (models repo), C++ (pipeline) | Models repo archived (last push 2025-12-15); pipeline 0.4.0, 2023-11-06, last push 2026-11-04 | bridge (Marian-format models) |
| OPUS-MT | Helsinki-NLP (University of Helsinki; licence: "Copyright (c) 2004-2020 Joerg Tiedemann") | MIT code (LICENSE: "Permission is hereby granted, free of charge"); models: "pre-trained downloadable translation models ... CC-BY 4.0 license" (README) | Server; GPU optional ("Option 2.1: Using Docker with CUDA GPU", README); model size not stated | Python | No releases; last push 2026-02-23 | bridge: its models run in a Rust engine (next row); CC-BY 4.0 means giving credit |
| candle (added: Rust engine for these models) | Hugging Face | crates.io candle-core "MIT OR Apache-2.0"; repo holds LICENSE-APACHE and LICENSE-MIT | Library; its example: "`marian-mt` is a neural machine translation model." (candle-examples/examples/marian-mt/README.md) | Rust | candle-core 0.11.1, 2026-10-31 | embed (Rust); the model stays a chosen download under the AI-optional rule |

## Data, dashboards, automation and low-code apps

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Apache Superset | Apache Software Foundation | Apache-2.0; LICENSE.txt: "Apache License Version 2.0, January 2004" | Server: "a Data Visualization and Data Exploration Platform" (repo description); hardware not stated | Python | 6.1.0, 2026-05-13 | beside |
| Metabase | Metabase, Inc. | NOASSERTION, open core; LICENSE.txt: "variously licensed under the GNU Affero General Public License (AGPL), or the Metabase Commercial License"; "Within the the top-level "enterprise" directory, source code in a given file is licensed under the Metabase Commercial License" | Server; hardware not stated | Clojure | v0.65.4, 2026-11-13 | point |
| Grafana | Grafana Labs | AGPL-3.0; LICENSE: "GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007" | Server: "The open and composable observability and data visualization platform." (repo description) | TypeScript | v13.3.1, 2026-11-10 | beside (for example, charts of our relay's health) |
| Node-RED | OpenJS Foundation and contributors | Apache-2.0; LICENSE: "Copyright OpenJS Foundation and other contributors" then "Apache License" | Server or desktop program: "Low-code programming for event-driven applications" (repo description); hardware not stated | JavaScript | 5.0.9, 2026-10-21 | beside |
| n8n | n8n (n8n-io) | NOASSERTION, fair-code; LICENSE.md, Sustainable Use License: "You may use or modify the software only for your own internal business purposes or for non-commercial or personal use."; files with ".ee." "are NOT licensed under the Sustainable Use License". n8n's docs: "These licenses are based on the [fair-code](https://faircode.io/) model." and a heading "Why don't you use an open source license?" (docs.n8n.io/n8n-community-license/community-license.md, undated page) | Server you host or n8n's cloud | TypeScript | n8n@2.47.7, 2026-11-13 | point. **Not open source by n8n's own words** (the OSI list itself was not read) |
| Huginn | Huginn project (licence: "Copyright (c) 2013, Andrew Cantino") | MIT; LICENSE: "The MIT License" | Server: "Create agents that monitor and act on your behalf." (repo description) | Ruby | v2026.11.09, 2026-11-09 | beside |
| Budibase | Budibase | NOASSERTION; LICENSE: "Each Budibase package has its own license"; "You can consider Budibase to be GPLv3 licensed overall." | Server | TypeScript | v4.0.0, 2026-11-10 | point |
| Appsmith | Appsmith | Apache-2.0; LICENSE: "Apache License Version 2.0, January 2004" | Server: "Platform to build admin panels, internal tools, and dashboards." (repo description) | TypeScript | v2.7.0, 2026-11-12 | beside |
| NocoDB | NocoDB | NOASSERTION, fair-code; LICENSE.md "Updated on: January 29, 2026": "Content in the "master" branch is available under the "Sustainable Use License""; "You may use or modify the software only for your own internal business purposes or for non-commercial or personal use." | Server: "A Free & Self-hostable Airtable Alternative" (repo description) | TypeScript | 2026.10.1, 2026-10-29 | point: now the same licence as n8n, so not open source |
| Baserow | Baserow B.V. | NOASSERTION, open core; LICENSE: "premium/" and "enterprise/" under their own licences; the rest "[Baserow Open Source Edition ("Baserow OSE")] is available under the "MIT Expat" license" | Server, self-hosted or their cloud | Python | 2.4.3, 2026-11-14 | beside (the MIT edition) |
| Activepieces (added: MIT-core automation, an n8n alternative) | Activepieces Inc. | NOASSERTION, open core; LICENSE: "packages/ee/" content under "packages/ee/LICENSE"; "Content outside of the above mentioned directories or restrictions above is available under the "MIT Expat" license" | Server | TypeScript | 0.93.0, 2026-11-08 | beside |

## Malware scanning

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| ClamAV | Cisco Talos | GPL-2.0; COPYING.txt: "GNU GENERAL PUBLIC LICENSE Version 2, June 1991" | Scanner program and scanning service (platform list and hardware not read) | C | clamav-1.5.4, 2026-08-07 | beside / bridge: run its scanning service and send it files to check |

## Code hosting

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Forgejo | Forgejo project | GPL-3.0-or-later: "Forgejo versions v9.0 and later are distributed under the terms of the GPL v3+." (forgejo.org/faq/, undated page; its repository on Codeberg could not be read) | Server you host: "a self-hosted lightweight software forge" (forgejo.org); no Windows builds: "there is no expertise in the Forgejo community to provide support for proprietary operating systems." (forgejo.org/faq/) | Not read | v17.0.1, "released on 26 October 2026" (forgejo.org/releases/) | beside: already in use as the project's pull mirror |
| Gitea | The Gitea Authors | MIT; LICENSE: "Copyright (c) 2016 The Gitea Authors" (MIT text) | Server: "As Gitea is written in Go, it works across all the platforms and architectures that are supported by Go, including Linux, macOS, FreeBSD/OpenBSD and Windows" (README) | Go | v28.3.0, 2026-11-01 | beside |

## AI assistant run on your own computer

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| llama.cpp | The ggml authors (ggml-org) | MIT; LICENSE: "MIT License" | "minimal setup and state-of-the-art performance on a wide range of hardware - locally and in the cloud"; "CPU+GPU hybrid inference to partially accelerate models larger than the total VRAM capacity" (README) | C++ | v1.0.0 (marked latest), 2026-11-13; daily build tags such as b12377, 2026-11-14 | beside: the licence allows embed (Rust binding `llama-cpp-2`, crates.io "MIT OR Apache-2.0"), but the AI-optional rule keeps the runtime a separate download |
| Ollama | Ollama | MIT; LICENSE: "MIT License" | Desktop program with a local server; hardware not stated in README | Go | v0.40.16, 2026-11-09 | beside / bridge (its local HTTP API) |
| Open WebUI | Open WebUI Inc. | NOASSERTION; LICENSE: "Open WebUI License"; clause 4: licensees are "strictly prohibited from altering, removing, obscuring, or replacing any "Open WebUI" branding", except where end users do "not exceed fifty (50) within any rolling thirty (30) day period". LICENSE_HISTORY: code before commit 60d84a3 (2025-04-18, "chore: license "branding" clause") keeps the earlier BSD-style licence | Server with a browser interface; GPU optional: "requires the Nvidia Container Toolkit" (README) | Svelte | v0.13.3, 2026-11-08 | point: the branding clause makes it more than a plain permissive licence |
| Jan | Menlo Research | NOASSERTION; LICENSE: "Licensed under the Apache License, Version 2.0" | Desktop program: "Windows: 10+ with GPU support for NVIDIA/AMD/Intel Arc"; "macOS: 13.6+ (8GB RAM for 3B models, 16GB for 7B, 32GB for 13B)" (README) | Rust and TypeScript | v0.8.8, 2026-10-28 | point (or beside) |
| mistral.rs (added: Rust) | Eric Buehler | MIT; LICENSE: "MIT License" | "Prebuilt CPU and CUDA images are published to GHCR." (README) | Rust | v0.9.5, 2026-10-19 | beside: Rust and MIT, so embeddable by licence, but the AI-optional rule applies |

## Professional network and job boards

No mature open, federated alternative to LinkedIn was found. A GitHub search on 14 November 2026
found only very early projects: ProFed ("Federated professional network", AGPL-3.0, 1 star, last
push 2026-10-15) and noombat ("The Fediverse professional networking platform.", AGPL-3.0, 1 star,
last push 2026-11-14). Bonfire (AGPL-3.0-only per its LICENSES folder, Elixir, v1.0.7, 2026-08-30)
is a general federated community app, not a professional network. Fit: none ready; HumanityOS's
own signed profiles and certificates are closer to the need than any of these.

## Standards

| Standard | Publisher, status and date | Quotation | Source | Fit |
|---|---|---|---|---|
| RFC 4226, HOTP (counter-based one-time codes) | IETF, Informational, December 2005 | "This document describes an algorithm to generate one-time password values, based on Hashed Message Authentication Code (HMAC)." | https://www.rfc-editor.org/rfc/rfc4226.txt | bridge |
| RFC 6238, TOTP (time-based one-time codes, the six digits in authenticator apps) | IETF, Informational, May 2011 | "This document describes an extension of the One-Time Password (OTP) algorithm, namely the HMAC-based One-Time Password (HOTP) algorithm, as defined in RFC 4226, to support the time-based moving factor." | https://www.rfc-editor.org/rfc/rfc6238.txt | bridge |
| WebAuthn Level 3 (passkeys) | W3C Recommendation, 25 August 2026 | Status line: "W3C Recommendation," dated "25 August 2026"; this version https://www.w3.org/TR/2026/REC-webauthn-3-20260825/ | https://www.w3.org/TR/webauthn-3/ | bridge |
| Verifiable Credentials Data Model 2.0 (signed claims a person carries) | W3C Recommendation, 15 May 2025 | "W3C Recommendation" with published date "15 May 2025" | https://www.w3.org/TR/vc-data-model-2.0/ | bridge: a possible shape for our signed profiles and certificates |
| ODbL (OpenStreetMap data licence) | OpenStreetMap Foundation | See the maps table | https://www.openstreetmap.org/copyright | bridge |

## Robots.txt read

- www.rfc-editor.org: one group "User-agent: *" barring only /rfc/authors/, /rfc/rerendered/ and /rfc/v3test/; RFC text allowed.
- www.w3.org: "User-agent: *" bars WordPress admin paths and similar; /TR/ allowed.
- crates.io: "User-agent: *" with an empty "Disallow:" (everything allowed).
- www.openstreetmap.org: "User-agent: *" bars user traces and history; /copyright allowed.
- codeberg.org: "User-agent: *" bars /api/, /*/*/src/, /*/*/raw/ and more; a later group naming ClaudeBot, Claude-Web and anthropic-ai says "Disallow: /". Treated as barred: nothing on Codeberg read.
- f-droid.org: groups for ClaudeBot, Claude-Web and anthropic-ai say "Disallow: /". Barred, not read.
- docs.n8n.io: "User-agent: *", "Allow: /", with "Content-Signal: ai-train=yes, search=yes, ai-input=yes".
- forgejo.org: "User-agent: *" with an empty "Disallow:" (everything allowed).
- timelimit.io: no robots.txt (404), so no restriction; two pages read.
- docs.goauthentik.io: no robots.txt (404 page), so no restriction; one page read.
- GitHub: facts read only through the API with `gh api`; no github.com web page was read.

## Could not read

- codeberg.org/forgejo/forgejo: Forgejo's repository language and licence file (barred; the licence came from forgejo.org instead).
- codeberg.org/timelimit/timelimit-android and f-droid.org/en/packages/io.timelimit.android.aosp.direct/: TimeLimit's licence (barred).
- Model sizes for Argos Translate, Firefox Translations and OPUS-MT: not stated in the READMEs read; Hugging Face and model-index pages not read.
- Hardware needs for Keycloak, Kanidm, ZITADEL, Vaultwarden, Superset, Metabase, Grafana, Node-RED, Ollama, LibreTranslate and ClamAV: not stated in the READMEs read; their documentation sites were not read.
- The OSI list of approved licences: the "not open source" notes for n8n and NocoDB rest on n8n's own docs and the licence text, not on the OSI list.
- docs.n8n.io/sustainable-use-license/: redirected to a "Page Not Found"; the community-license.md and license-faq.md pages were read instead.
- Firefox's and Chromium's platform lists, and ente Auth's non-phone platforms: not read.
