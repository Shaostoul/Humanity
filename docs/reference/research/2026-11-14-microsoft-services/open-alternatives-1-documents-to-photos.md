# Open alternatives to Microsoft's everyday services, part 1: documents to photos

What this is: a scan of the open projects that meet the same everyday needs as Microsoft's services
(office documents, file sync, calls and chat, calendars, notes, whiteboards, forms, tasks and
booking, video, photos and backup), with each project's licence, publisher, platform, language,
latest release, and how it could fit HumanityOS.

Read on 14 November 2026 (Pacific), by an AI research helper.

Method: every host's robots.txt was read first, in a step of its own (list at the end). GitHub facts
(licence detected by GitHub as an SPDX id, the licence file's first lines, the repository's language
field, the latest GitHub release and the last commit) were read through `gh api` (GitHub's own
programming interface). crates.io facts came from its public API, one request per second or slower.
Web pages were read with curl identifying itself as "Claude-User (research tool)". Licence files in
repositories carry no date of their own unless noted: treat them as "the file as it stood on 14
November 2026". Release dates are GitHub's own "published" dates.

Quotations are exact (curly quotes normalised to straight). **This is an AI's reading of each
licence, not legal advice.** Every licence is quoted so a reader can check it; the file named in the
"Licence" column is in the repository named in the "Project" column
(`https://github.com/OWNER/REPO/blob/HEAD/<file>`).

Words used below:
- **SPDX id**: a standard short name for a licence (for example `MIT`, `AGPL-3.0`). `NOASSERTION`
  means GitHub's detector could not name one, usually because the file adds extra text.
- **Permissive** (MIT, Apache-2.0, BSD, ISC, Zlib): code can be built into our program.
- **Copyleft** (GPL, AGPL, LGPL): code linked into our program would force the whole program onto
  that licence. AGPL also covers people who use it over a network. MPL-2.0 is copyleft per file only.
- **Open core**: the main code is open, but some folders are under a paid, non-open licence.
- **Fit**: "embed" (a library that can live inside our Rust app), "beside" (a separate program run
  next to ours), "bridge" (we speak its open protocol or file format), "point" (we only link people
  to it from our Apps page). HumanityOS is CC0-1.0 and ships as one program, so copyleft projects
  fit only as beside, bridge or point.

## In plain words

- **Best "embed" fits, all permissive and written in Rust:** Typst for typeset documents
  (Apache-2.0); the `icalendar` and `calcard` crates for calendar (.ics) files; `rustic_core` for
  encrypted, deduplicated backups in restic's format; `matrix-rust-sdk` if we ever talk to Matrix.
- **Best "bridge" fits, through open standards rather than code:** ODF documents (LibreOffice),
  CalDAV/CardDAV calendars and contacts (Radicale, DAVx5), Matrix chat, ActivityPub video
  (PeerTube), and the Excalidraw and draw.io drawing files (MIT and Apache-2.0, so the web mirror
  could even embed those editors).
- **Licence trap 1: tldraw is not open source.** Its licence says "Not to use the Software in
  Production Environments." without a paid key, and the software checks that key and may report
  usage. Avoid it; use Excalidraw or draw.io.
- **Licence trap 2, open core:** AFFiNE's server code needs a paid subscription for production use,
  Formbricks has a paid "ee" folder, Joplin Server has a personal-use licence, PhotoPrism keeps some
  features for paying members and claims its brand assets, and Cal.com's open repository is now a
  separate "Cal.diy" with the enterprise code removed.
- **Most big self-hosted suites are AGPL** (Nextcloud, ONLYOFFICE, CryptPad, Synapse, Immich,
  PeerTube, Plane, Vikunja, Joplin, Logseq): good to run beside ours or link to, never to build in.
  ONLYOFFICE's current source headers say "No trademark rights are granted under this License."
  (the older "retain the logo" wording was not in the headers read today).
- **Health warnings:** Kanboard says it is "in maintenance mode"; the digiKam GitHub copy is archived
  (the project lives on KDE's own server); Seafile's GitHub releases stop in 2019 though its code
  is still changing; draw.io says "We do not accept pull requests."; Collabora's GitHub is now
  "Issue tracker only".

## Documents and office suites

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| LibreOffice (`LibreOffice/core`) | The Document Foundation: README: "Libreoffice is backed by The Document Foundation" | GitHub detects `GPL-3.0` (from `COPYING`), but the project's own statement says MPL: "This product is made available subject to the terms of the Mozilla Public License, v. 2.0." (`readlicense_oo/license/license.xml`); some files also under LGPL ("either the LGPL or the MPL") | Desktop program for Windows. "256 MB RAM (512 MB RAM recommended)", "Up to 1.5 GB available hard disk space" (https://www.libreoffice.org/get-help/system-requirements/, undated page) | C++ | No GitHub releases; last commit 2026-11-14 | **bridge** (ODF files); can also run beside us, headless, to convert documents |
| Collabora Online (`CollaboraOnline/online`) | Collabora (GitHub org "Collabora Online") | `NOASSERTION`; `COPYING`: "This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0." (MPL-2.0) | A server you host; documents edited in the browser. Hardware: not stated in the files read. Repo description: "Issue tracker only. Active development is on Gerrit" | Shell (field reflects the GitHub copy) | GitHub release `25.04.7-mobile` 2025-12-05 is a phone build; server releases not on GitHub; last commit 2026-11-13 | **beside** |
| ONLYOFFICE Docs (`ONLYOFFICE/DocumentServer`) | Ascensio System SIA (source headers: "Copyright (C) Ascensio System SIA, 2009-2026") | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE". Source headers (e.g. `ONLYOFFICE/server` `Common/sources/utils.js`) add: "No trademark rights are granted under this License." and non-code elements are under "Creative Commons Attribution-ShareAlike 4.0". `Readme.md`: the free Community edition is "free and open source for non-critical use."; "White Label" ("Your logo, name, and interface settings.") is offered only for the paid Developer edition | A server you host; browser editors. Hardware: not stated in the files read | Shell (wrapper repo) | v10.0.0, 2026-10-12 | **point** (could run beside, but AGPL, and the free edition is pitched for "non-critical use") |
| CryptPad (`cryptpad/cryptpad`) | CryptPad (GitHub org) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | A server you host; web app. Description: "Collaborative office suite, end-to-end encrypted". Hardware: not stated in the files read | JavaScript | 2026.10.1, 2026-11-10 | **point** (closest to our end-to-end encryption values) |
| Etherpad (`ether/etherpad-lite`) | The Etherpad Foundation | `Apache-2.0`; `LICENSE`: "Apache License" "Version 2.0, January 2004" | A server you host; web editor ("really-real-time collaborative document editor"). Hardware: not stated | TypeScript | v3.3.17, 2026-11-11 | **beside** (permissive, so a relay operator could run it next to ours) |
| Typst (`typst/typst`) | Typst (GitHub org) | `Apache-2.0`; `LICENSE`: "Apache License" "Version 2.0, January 2004" | A library and command-line program; "A markup-based typesetting system"; README: "Windows: `winget install --id Typst.Typst`". Hardware: not stated | **Rust** | v0.15.1, 2026-07-17 | **embed** (Rust, permissive: printable documents and PDFs inside our app) |
| Pandoc (`jgm/pandoc`) | John MacFarlane (`COPYRIGHT`: "Copyright (C) 2006-2024 John MacFarlane") | `GPL-2.0`; `COPYRIGHT`: "either version 2 of the License, or (at your option) any later version." | Command-line program: "Universal markup converter". Hardware: not stated | Haskell | 3.14, 2026-11-12 | **beside** (call the separate pandoc program; never link it) |

## Files and sync

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Nextcloud (`nextcloud/server`) | Nextcloud GmbH (README: "Nextcloud Enterprise ... provided by Nextcloud GmbH") | `AGPL-3.0`; README: "SPDX-License-Identifier: AGPL-3.0-or-later"; `COPYING`: "GNU AFFERO GENERAL PUBLIC LICENSE" | A server you host. "Nextcloud needs a minimum of **128MB** RAM per process, and we recommend a minimum of **512MB** RAM per process." (`nextcloud/documentation` `admin_manual/installation/system_requirements.rst`) | PHP | v33.0.11, 2026-11-12 | **bridge** (WebDAV files) |
| Seafile (`haiwen/seafile-server`, `haiwen/seafile`) | Seafile (GitHub org `haiwen`, site seafile.com) | `NOASSERTION`. Server `LICENSE.txt`: "This program is released under Affero GPLv3, with the following additional permission to link with OpenSSL library." Client: "released under GPLv2". README: "Seahub (Seafile server Web UI): Apache License v2" | A server you host plus a desktop sync client (README: "Desktop syncing client (this repository): GPLv2"). Hardware: not stated in the files read | C | GitHub releases stale (server v7.1.1, 2019-12-17); last server commit 2026-11-13 | **point** |
| Syncthing (`syncthing/syncthing`) | The Syncthing Project | `MPL-2.0`; `LICENSE`: "Mozilla Public License Version 2.0" | A program on each computer that syncs straight to your other computers: "Open Source Continuous File Synchronization"; README mentions "macOS and Windows binaries". Hardware: not stated | Go | v2.1.7, 2026-10-21 | **beside** (MPL is per file, but Go code cannot live in our Rust program anyway) |
| ownCloud Infinite Scale (`owncloud/ocis`) | "ownCloud - a Kiteworks Company" (GitHub org name) | `Apache-2.0`; `LICENSE`: "Apache License"; README: "oCIS does not require a Contributor License Agreement (CLA) as it is Apache 2.0 licensed" | A server you host. Hardware: not stated in the files read | Go | v8.2.3, 2026-11-06 | **beside** (permissive; or bridge by WebDAV) |

## Calls, meetings and chat

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Jitsi Meet (`jitsi/jitsi-meet`) | Jitsi (GitHub org) | `Apache-2.0`; `LICENSE`: "Apache License" | A server you host; meetings in the browser; "use as a standalone app or embed in your web application". "RAM: it's usually suggested to get 8 GB." "For small meetings you can get away with 4 GB" (`jitsi/handbook` `docs/devops-guide/requirements.md`) | TypeScript | stable/jitsi-meet_11248, 2026-09-14 | **beside** (permissive; the web mirror could embed a meeting) |
| BigBlueButton (`bigbluebutton/bigbluebutton`) | BigBlueButton (GitHub org) | `LGPL-3.0`; `LICENSE`: "GNU LESSER GENERAL PUBLIC LICENSE" | A dedicated Ubuntu 24.04 server: "16 GB of memory with swap enabled", "8 CPU cores, with high single-thread performance", "500 GB of free disk space" (`docs/docs/administration/install.md`) | TypeScript | v4.0.2, 2026-11-06 | **point** (heavy, built for classes) |
| Matrix: Synapse server (`element-hq/synapse`) | Element Creations Ltd (README: "This software is dual-licensed by Element Creations Ltd (Element).") | `AGPL-3.0`; README: "available under an AGPL license (or alternatively under a commercial license from Element)." | A server you host. "At least 1GB of free RAM if you want to join large public rooms" (`docs/setup/installation.md`) | Python (description: "Python/Twisted + Rust") | v1.165.0, 2026-11-10 | **bridge** (the Matrix protocol is open; our relay could federate with it) |
| Matrix: Element client (`element-hq/element-web`) | New Vector Ltd (Element) (README) | `AGPL-3.0`; README: "multi licensed by New Vector Ltd (Element)": AGPL-3.0 or later, GPL-3.0 or later, or "a paid-for Element Commercial License" | Web app: "A glossy Matrix collaboration client for the web." Hardware: not stated | TypeScript | v1.12.35, 2026-11-12 | **point** |
| Added: matrix-rust-sdk (`matrix-org/matrix-rust-sdk`) | The Matrix.org Foundation | `Apache-2.0`; `LICENSE`: "Apache License" "Version 2.0, January 2004" | A library. "Matrix Client-Server SDK for Rust" | **Rust** | matrix-sdk-0.19.1, 2026-09-18 | **embed** (if we add a Matrix bridge) |
| Added: tuwunel (`matrix-construct/tuwunel`) | Matrix Construct (GitHub org) | `Apache-2.0`; `LICENSE`: "Apache License" | A Matrix server you host; "Official successor to conduwuit". Hardware: not stated | **Rust** | last commit 2026-11-10 (latest GitHub release is an asset bundle, "v2.0.0-assets") | **beside** (a Rust Matrix server exists) |
| Mumble (`mumble-voip/mumble`) | The Mumble Developers | `NOASSERTION`, but a 3-clause BSD-style permissive licence; `LICENSE`: "Copyright (C) The Mumble Developers" "Redistribution and use in source and binary forms, with or without modification, are permitted" | Voice-chat program plus a server you host: "The client works on Windows, Linux, FreeBSD, OpenBSD, and macOS". Hardware: not stated | C++ | v1.5.939, 2026-10-11 | **beside** (permissive, but C++ and Qt; a model for low-delay voice) |

## Calendar and contacts

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Radicale (`Kozea/Radicale`) | Kozea | `GPL-3.0`; `COPYING.md`: "GNU GENERAL PUBLIC LICENSE" "Version 3, 29 June 2007" | A small server you host; "A simple CalDAV (calendar) and CardDAV (contact) server." Hardware: not stated | Python | v3.9.1, 2026-11-12 | **bridge** (CalDAV and CardDAV are open standards) |
| Baikal (`sabre-io/Baikal`) | sabre.io | `GPL-3.0`; `LICENSE`: "GNU GENERAL PUBLIC LICENSE" | A server you host; "Baïkal is a Calendar+Contacts server". Hardware: not stated | PHP | 0.13.0, 2026-10-19 | **bridge** (CalDAV and CardDAV) |
| DAVx5 (`bitfireAT/davx5-ose`) | bitfire web engineering | `GPL-3.0`; `LICENSE`: "GNU GENERAL PUBLIC LICENSE" | Android phone app; "CalDAV/CardDAV suite and sync app for Android". Hardware: not stated | Kotlin | v4.5.21-ose, 2026-10-15 | **point** (would sync a phone to us if we served CalDAV) |
| Rust crate `icalendar` (`hoodie/icalendar`) | GitHub user hoodie | crates.io licence field: "MIT/Apache-2.0" (https://crates.io/api/v1/crates/icalendar); repo `LICENSE-Apache`: "Apache License" | A library: "Strongly typed iCalendar builder and parser." | **Rust** | 0.17.17, 2026-10-26 | **embed** |
| Added: Rust crate `calcard` (`stalwartlabs/calcard`) | Stalwart Labs | crates.io licence field: "Apache-2.0 OR MIT"; README: "Licensed under either of" Apache 2.0 or MIT | A library: "iCalendar/JSCalendar and vCard/JSContact parsing, building and conversion library for Rust". GitHub marks the repository archived | **Rust** | 0.3.14, 2026-09-15 | **embed** (covers contacts too; watch where it is maintained now) |
| Rust crate `ical` (`Peltoche/ical-rs`) | GitHub user Peltoche | crates.io licence field: "non-standard"; repo `LICENSE` detected `Apache-2.0`: "Apache License" | A library: "Rust parser for ics (rfc5545) and vcard (rfc6350)". Repository archived | **Rust** | 0.11.0, 2024-03-13 | embed possible, but archived; prefer the two above |

## Notes

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Joplin (`laurent22/joplin`) | Laurent Cozic (`laurent22`); the server licence names "JOPLIN registered under the laws of France" | `NOASSERTION`; `LICENSE`: "All code in this repository is licensed under the AGPL-3.0-or-later License" unless a folder has its own. Trap: `packages/server/LICENSE.md`: "JOPLIN SERVER PERSONAL USE LICENSE" (dated "v.1: 21 December 2021") | Windows desktop, phone apps; description: "for Windows, macOS, Linux, Android and iOS". Hardware: not stated | TypeScript | v3.7.22, 2026-10-28 | **point** |
| Logseq (`logseq/logseq`) | Logseq (GitHub org) | `AGPL-3.0`; `LICENSE.md`: "GNU AFFERO GENERAL PUBLIC LICENSE" | Web and desktop app (README: "the Logseq web or desktop app"). Hardware: not stated | Clojure | 2.0.2, 2026-10-07 | **point** |
| AppFlowy (`AppFlowy-IO/AppFlowy`) | AppFlowy (site appflowy.com) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | README: "AppFlowy Desktop (macOS, Windows, and Linux)", and Android ("Android 10 or above"). Hardware: not stated | Dart | 0.15.0, 2026-11-02 | **point** |
| AFFiNE (`toeverything/AFFiNE`) | TOEVERYTHING PTE. LTD. | `NOASSERTION`, open core. `LICENSE`: content outside two folders "is available under the "MIT" license". Trap: `packages/backend` and `packages/common/native` fall under `packages/backend/server/LICENSE`: "may only be used in production, if you ... have a valid AFFiNE Enterprise Edition subscription" | A knowledge-base app (platforms not checked); its server code is the non-open part. Hardware: not stated | TypeScript | v0.27.4, 2026-08-18 | **point** (only the MIT parts could ever be reused) |
| Standard Notes (`standardnotes/app`) | Standard Notes (GitHub org) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | README links Windows, iOS and Android apps; "end-to-end encrypted notes and files". Hardware: not stated | TypeScript | desktop 3.202.11, 2026-10-23 | **point** |
| Trilium Notes (`TriliumNext/Trilium`) | Trilium Next community (GitHub org) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | Desktop program (README section "Windows / MacOS") or a server you host (README: "your self-hosted Trilium / TriliumNext server"). Hardware: not stated | TypeScript | v0.107.0, 2026-11-08 | **point** |

## Whiteboards and diagrams

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Excalidraw (`excalidraw/excalidraw`) | Excalidraw (`LICENSE`: "Copyright (c) 2020 Excalidraw") | `MIT`; `LICENSE`: "MIT License" | Web app; "Virtual whiteboard for sketching hand-drawn like diagrams". Hardware: not stated | TypeScript | v0.18.1, 2026-04-21 | **bridge** (its drawing files); MIT, so the web mirror could embed the editor |
| tldraw (`tldraw/tldraw`) | tldraw, Inc. | `NOASSERTION`; **not an open-source licence.** `LICENSE.md`: "# tldraw license"; conditions include "Not to use the Software in Production Environments." and "Not to disable, change, or interfere with the Software's License Key enforcement."; also "The Software may collect and transmit usage data to tldraw for license compliance purposes." | A software kit for building canvas apps in React (web). Hardware: not stated | TypeScript | v5.6.0, 2026-10-28 | **point at most; do not build on it** |
| draw.io (`jgraph/drawio`) | JGraph (GitHub org "draw.io") | `Apache-2.0`; README: "The source code in this repository is licensed under the [Apache License 2.0]" and "We do not accept pull requests." | Runs in the browser ("a JavaScript, client-side editor for general diagramming"; README lists "Edge 123+" among supported browsers). README: "does not support real-time collaborative editing in this version". Hardware: not stated | JavaScript | v32.6.10, 2026-11-04 | **bridge** (.drawio files); permissive, so the web mirror could embed it |
| Penpot (`penpot/penpot`) | Kaleidos (README: "Copyright (c) KALEIDOS SUBSIDIARY SL") | `MPL-2.0`; `LICENSE`: "Mozilla Public License Version 2.0" | A server you host, used in the browser; "open-source design platform". Hardware: not stated in the files read | Clojure | 2.19.1, 2026-11-11 | **point** (a design tool, a Figma alternative) |

## Forms and surveys

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| LimeSurvey (`LimeSurvey/LimeSurvey`) | LimeSurvey GmbH | `NOASSERTION`, GPL version 2 or later; `LICENSE`: "LimeSurvey - The free & open-source survey software tool" ... "either version 2 of the License, or (at your option) any later version." | A server you host; surveys in the browser. Hardware: not stated in the files read | JavaScript (GitHub's field) | No GitHub releases; last commit 2026-11-12 | **point** |
| Formbricks (`formbricks/formbricks`) | Formbricks GmbH | `NOASSERTION`, open core. `LICENSE`: the rest "is available under the "AGPLv3" license"; SDK folders under MIT; trap: `apps/web/modules/ee/LICENSE`: "may only be used in production, if you ... have a valid Formbricks Enterprise license" | A server you host; "Open Source Qualtrics Alternative". Hardware: not stated in the files read | TypeScript | 6.2.0, 2026-11-13 | **point** |

## Tasks, projects and booking

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| OpenProject (`opf/openproject`) | OpenProject (GitHub org) | `GPL-3.0`; README: "OpenProject is licensed under the terms of the GNU General Public License version 3." It also offers an "Enterprise Edition" with "Enterprise add-ons" | A server you host. Minimum: "Quad Core CPU (>= 2ghz)", "Memory: 4096 MB", "Free disk space: 20 GB" (`docs/installation-and-operations/system-requirements/README.md`) | Ruby | v17.9.7, 2026-11-09 | **point** |
| Taiga (`taigaio/taiga-back`, `taigaio/taiga-front`) | Taiga.io (GitHub org) | Back end `MPL-2.0` ("Mozilla Public License Version 2.0"); front end `AGPL-3.0` ("GNU AFFERO GENERAL PUBLIC LICENSE") | A server you host. Hardware: not stated | Python and CoffeeScript | No GitHub releases; last commits 2026-11-03 and 2026-10-27 | **point** |
| Kanboard (`kanboard/kanboard`) | Frédéric Guillot (`LICENSE`: "Copyright (c) Frédéric Guillot") | `MIT`; `LICENSE`: "The MIT License (MIT)" | A small server you host. README: "**This application is in maintenance mode**." Hardware: not stated | PHP | v1.2.57, 2026-10-31 | **point** |
| Vikunja (`go-vikunja/vikunja`) | Vikunja (GitHub org) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | A server you host; "The task manager you actually own." Hardware: not stated | Go | v2.8.0, 2026-11-05 | **point** |
| Cal.com, now "Cal.diy" (`calcom/cal.com` redirects to `calcom/cal.diy`) | Cal.com, Inc. | `MIT`; `LICENSE`: "MIT License" "Copyright (c) 2020-present Cal.com, Inc."; README: "a fork of [Cal.com](https://cal.com) with all enterprise/commercial code removed" | A server you host. README warning: "It is strictly recommended for personal, non-production use." Hardware: not stated | TypeScript | v6.2.0, 2026-03-01; last commit 2026-11-13 | **beside** (permissive; booking pages next to ours) |
| Easy!Appointments (`alextselegidis/easyappointments`) | Alex Tselegidis | `GPL-3.0`; `LICENSE`: "GNU GENERAL PUBLIC LICENSE" | A server you host; "Self Hosted Appointment Scheduler". Hardware: not stated | PHP | 1.6.1, 2026-10-19 | **point** |
| Added: Plane (`makeplane/plane`) | Plane (GitHub org) | `AGPL-3.0`; README: "This project is licensed under the [GNU Affero General Public License v3.0]" | A server you host; "Open-source Jira, Linear, Monday, and ClickUp alternative". Hardware: not stated | TypeScript | v1.4.3, 2026-10-28 | **point** (added as a widely used project tool) |

## Video

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Kdenlive (`KDE/kdenlive`, a copy of invent.kde.org/multimedia/kdenlive) | KDE | `GPL-3.0`; `COPYING`: "GNU GENERAL PUBLIC LICENSE" "Version 3, 29 June 2007" | Desktop program: "Free and open source video editor, based on MLT Framework and KDE Frameworks". Hardware: not stated in the files read | C++ | Tag v26.08.3 dated 2026-11-03 on invent.kde.org (v26.11.80, 2026-11-12, is a test build) | **point** |
| Shotcut (`mltframework/shotcut`) | MLT Framework (GitHub org) | `GPL-3.0`; `COPYING`: "GNU GENERAL PUBLIC LICENSE"; description: "open-source (GPLv3) video editor" | Desktop video editor, "cross-platform (Qt)". Hardware: not stated in the files read | C++ | v26.9.27, 2026-09-28 | **point** |
| OpenShot (`OpenShot/openshot-qt`) | OpenShot Studios, LLC | `NOASSERTION`; `COPYING`: "GNU GENERAL PUBLIC LICENSE" "Version 3, 29 June 2007" | Desktop video editor "for Linux, Mac, and Windows". Hardware: not stated | Python | v4.1.1, 2026-11-02 | **point** |
| PeerTube (`Chocobozzz/PeerTube`) | Framasoft (README: "Developed with ... by Framasoft") | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | A server you host. "Minimum hardware requirements: 1 vCore 1.5 GB of RAM" (https://joinpeertube.org/faq, undated page) | TypeScript | v8.3.2, 2026-10-26 | **bridge** (description: "ActivityPub-federated video streaming platform") |
| Owncast (`owncast/owncast`) | Gabe Kangas (`LICENSE`: "Copyright (c) 2020-2023 Gabe Kangas") | `MIT`; `LICENSE`: "MIT License" | A server you host for your own live stream: "Streaming + chat out of the box." Hardware: not stated in the files read | Go | v0.3.0, 2026-09-03 | **beside** (permissive; live streaming next to ours) |

## Photos and backup

| Project | Publisher | Licence (SPDX, quotation, source) | Runs on and hardware | Language | Latest release | Fit |
|---|---|---|---|---|---|---|
| Immich (`immich-app/immich`) | Immich (GitHub org) | `AGPL-3.0`; `LICENSE`: "GNU AFFERO GENERAL PUBLIC LICENSE" | A server you host. "RAM: Minimum 6GB, recommended 8GB." "CPU: Minimum 2 cores, recommended 4 cores." (`docs/docs/install/requirements.md`) | TypeScript | v3.4.1, 2026-11-11 | **point** |
| PhotoPrism (`photoprism/photoprism`) | PhotoPrism UG | `NOASSERTION`: AGPL-3.0 plus an added section. `LICENSE` section 18: brand assets are "proprietary assets owned exclusively by PhotoPrism UG". Paid part: README: "Our members [enjoy additional features]"; it also sells "a commercial license" | A server you host. "at least 2 cores, 3 GB of physical memory" (`photoprism/photoprism-docs` `docs/getting-started/index.md`) | Go | 261029-963b58c50, 2026-10-29 | **point** |
| digiKam (`KDE/digikam`, archived copy; project at invent.kde.org/graphics/digikam) | KDE | `GPL-2.0`; `COPYING`: "GNU GENERAL PUBLIC LICENSE" "Version 2, June 1991" | Desktop program that "runs on Linux, Windows, and MacOS". Hardware: not stated | C++ | Tag v9.1.0 dated 2026-06-06 on invent.kde.org (GitHub copy archived, last commit 2019-08-07) | **point** |
| restic (`restic/restic`) | The restic authors (`LICENSE`: "Copyright (c) 2014, Alexander Neumann") | `BSD-2-Clause`; `LICENSE`: "BSD 2-Clause License" | Command-line backup program: "It supports the three major operating systems (Linux, macOS, Windows)". Hardware: not stated | Go | v0.19.1, 2026-07-05 | **beside**; a Rust version exists (next row) |
| Added: rustic and `rustic_core` (`rustic-rs/rustic`) | rustic (GitHub org) | Program `Apache-2.0` (`LICENSE-APACHE`: "Apache License"); library crate `rustic_core` licence field: "Apache-2.0 OR MIT" (https://crates.io/api/v1/crates/rustic_core) | "fast, encrypted, and deduplicated backups powered by Rust"; README: it "can be used as a `restic` replacement in most cases." Hardware: not stated | **Rust** | rustic v0.11.4, 2026-08-18; rustic_core 0.13.0, 2026-08-16 | **embed** (backups inside our app) |
| BorgBackup (`borgbackup/borg`) | The Borg Collective (`LICENSE`: "Copyright (C) 2015-2026 The Borg Collective") | `NOASSERTION`, a 3-clause BSD-style licence: "Redistribution and use in source and binary forms, with or without modification" | Command-line backup program; "Deduplicating archiver with compression and authenticated encryption."; on Windows only "experimental" builds (README). Hardware: not stated | Python | 1.4.5, 2026-07-18 | **beside** |
| Kopia (`kopia/kopia`) | Kopia (GitHub org) | `Apache-2.0`; `LICENSE`: "Apache License" | "Cross-platform backup tool for Windows, macOS & Linux" with "client-side end-to-end encryption". Hardware: not stated | Go | v0.23.1, 2026-06-16 | **beside** |

## Robots.txt read

- github.com: has a group for "ClaudeBot", "Claude-User" and other AI readers with "Crawl-delay: 1" and path rules (for example "Disallow: /search$", "Disallow: /*?tab=*"); repository pages are not barred, and the file points readers to "an extensive API". All GitHub facts were read through that API.
- api.github.com: no robots.txt (HTTP 404), so no rules; read through `gh api`.
- crates.io: "User-agent: *" with an empty "Disallow:" (everything allowed); four API requests, spaced over a second apart.
- invent.kde.org: "User-Agent: *" bars "/api/v*" and "/*/-/commits/" among others; project tag pages are not barred, so only the digiKam and Kdenlive tag pages were read.
- joinpeertube.org: no robots.txt (HTTP 404); one page read (the FAQ).
- www.libreoffice.org: no robots.txt (HTTP 404); one page read (system requirements).

## Could not read

- invent.kde.org API (`/api/v4/...`): barred by robots.txt; I wanted exact release dates for digiKam and Kdenlive and used the tag pages instead.
- Hardware statements for Collabora Online, ONLYOFFICE Docs, CryptPad, Seafile, ownCloud Infinite Scale, Penpot, LimeSurvey, Formbricks, Plane, Owncast and the desktop video editors: not in the READMEs or repository documents read; their own websites were not checked.
- Collabora Online and LibreOffice server or desktop release numbers: not published as GitHub releases; the last commit date is given instead. The same applies to LimeSurvey and Taiga.
- Whether `calcard` is now maintained somewhere other than its archived GitHub repository: not checked.
