# The everyday suite: what Microsoft's services do for people, and our route to each

> Written 2026-11-14. A gap analysis, not a build log: every "HumanityOS now" cell was checked
> against `docs/FEATURES.md` and the source file it names on that date. The facts about Microsoft
> and about the open alternatives come from the dated research notes in
> [`../reference/research/2026-11-14-microsoft-services/`](../reference/research/2026-11-14-microsoft-services/README.md),
> where every claim carries its source, the source's date and a quotation. Legal and terms points
> are in three dated findings (listed at the end). The build order lives in
> [`../ROADMAP.md`](../ROADMAP.md), section "Everyday life suite".

## Why this exists

The operator, 2026-11-14: "I'm wondering what services they provide that our app is still missing.
We want to make HumanityOS a one stop shop holistic app for everyone's needs... eventually. Our chat
system is essentially email but, more convenient to use." He asked for the roadmap to be upgraded
"to incorporate everything possible".

This is the all-in-one direction of [`two-realities.md`](two-realities.md) ("the app's real-life
side is an all-in-one life suite") applied to one question: what does a person get from one big
provider that they cannot yet get here?

## In plain words

- **Much of the core is already here.** Messages (sealed so only the people in a conversation can
  read them), group chats, voice rooms, calls and screen sharing on the website, voice on the
  desktop, files shared on a server, tasks, signed voting, a readable web, maps, notifications, and
  an identity that needs no sign-up and comes back from a recovery phrase.
- **The biggest gaps are the ones people meet every day:** opening a Word or Excel file someone
  sent them, a calendar that follows them to their other devices and can be shared, notes that
  sync, forms and sign-up sheets, sign-in codes, and photo backup from a phone.
- **Email stays with other providers for a long time** (the operator's decision below). A bridge
  comes later, and when it does, the first rung keeps nobody's mail on our servers.
- **Almost every gap can be built today**, on the project's current small servers and the
  operator's PC, because the work is code and each person's own device does the heavy lifting.
  A short list needs real money: keeping everyone's mail, sending mail at scale, backing up
  everyone's photos, calling ordinary phone numbers, a web search index, and games or PCs run in a
  data centre. Each row below says which side it is on.
- **The order** puts what a group needs to run itself first (the operator: "This definitely sounds
  high priority"), then files, documents, forms, the calendar and notes; sign-in codes and the mail
  bridge come later; translation and captions come last.

## His answers (2026-11-14)

The operator answered a preliminary gap list the same day. His words, verbatim, and what each one
sets in this plan:

| Topic | His words | What it sets |
|---|---|---|
| Email | "We could do a bridge later. Hopefully we can make something better than the email app Thunderbird." and "I'm fine letting email be third party for a long time. To properly implement it would take away from the more important aspect of game dev." | Email stays third party for a long time; a bridge is a later rung. When it comes, the bar is "better than Thunderbird", which section 3 turns into concrete aims. |
| Email for everyone | "How would giving our own users an email address work for billions of users for our app?" | Answered in plain words in section 3.3. The decision above stands. |
| Office documents | "We'll have to fix that eventually. I'm sure we'll want to display those sort of documents on in-game displays. Being able to edit them in-app would be convenient." | Viewing comes first and is drawn by our own code, so the same view works on the Files page, on the website and on an in-world screen (section 4). Editing follows. |
| Sign-in codes and passwords | "This sounds like a big difficult task too." | Later, and built only to the security bar it needs (section 5). |
| Files | "Which fits with fleshing out of file explorer. Right now I can view GLBs and listen to music and move things around but, it's still far from perfect." | Version history, shared folders and sync between your own devices belong to the Files page work, not to a new page (section 6). |
| Photos | "You mean something different from the current file upload system?" | Answered in section 6.2: a photo library is a view and an automatic phone backup on top of Files, not a second store. |
| Smaller tools | "We definitely want to get these made eventually. Especially the forms and surveys. I guess a survey is kinda like a vote but, different. I figure translation and captions are probably among the last things to do. I guess we already have partial family safety controls." | Forms and surveys early, built beside the signed voting (section 7); translation and captions last; family safety builds on the Safety tools that already exist (section 8). |
| Phone apps | "It would be nice to have the native app on my one plus nine pro instead of the PWA version." | The native app on Android is noted in section 9; its platform work and costs are in the platforms design document another agent is writing (platforms.md, in this design folder). |
| Organisation tools | "This definitely sounds high priority." | First in the order (section 10). |
| Cheap hardware or funding | "I guess we need to separate things between what we can do with the cheap hardware today and what would need millions in funding to actually work." | Every row and every email option carries a "Today" or "Funding" tag, and the roadmap rungs run "Today" first. |

## 1. Today or funding: what the tags mean

- **Today**: doable now on the project's current small servers (two inexpensive rented servers) and
  the operator's PC. The work is code. Each person's own computer or phone does the computing, and
  a server only passes on or briefly holds small sealed items, the way it already holds sealed
  messages until they are fetched.
- **Funding: storage**: the feature means a server keeping large amounts of other people's data for
  a long time (mailboxes, photo libraries, whole-PC backups). For scale: Microsoft's free account
  gives 15 GB of mail and 5 GB of cloud storage (research notes, part 1), so a server holding
  1,000 people's free-tier mail alone would hold 15 TB (arithmetic, not a quote).
- **Funding: bandwidth**: serving large amounts of video or files to many people.
- **Funding: staff**: people paid to handle abuse, support, security audits or legal questions.
  Sending email at scale is mostly this cost (section 3).
- **Funding: compute**: data-centre computers doing the work for people (games streamed from a
  server, cloud PCs, a web search index, hosted AI).
- **Funding: carriers**: telephone networks charge to connect to ordinary phone numbers (not
  researched in detail today).

## 2. The gap table, by everyday need

State: **Live** (shipped and usable), **Partial** (some of it), **None**. Open alternatives are
named here; each one's publisher, licence (quoted) and fit are in the research notes
([part 1](../reference/research/2026-11-14-microsoft-services/open-alternatives-1-documents-to-photos.md),
[part 2](../reference/research/2026-11-14-microsoft-services/open-alternatives-2-identity-to-ai.md)).
Routes: **native** (build it in egui first, then mirror it on the website), **embed** (a permissively
licensed library inside our program), **beside** (a separate open program run next to ours),
**bridge** (we speak its open protocol or file format), **point** (the Apps page links people to it;
`data/external/catalog.json` already lists LibreOffice, ONLYOFFICE, CryptPad, Etherpad, KeePassXC,
Kdenlive, Shotcut and others). Licence rule behind the routes: HumanityOS is CC0 and ships as one
program, so GPL or AGPL code can only run beside it, never inside it.

### 2.1 Mail

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Outlook.com, the Outlook app, Exchange Online | A mail address, an inbox, contacts and a calendar; organisations' mailboxes | **Partial by design.** Between HumanityOS people the chat replaces mail: sealed messages (FEATURES "Post-Quantum Sealed-Sender DMs"; `src/net/dm_pq.rs`). **None** for mail from the rest of the world. | Thunderbird (client); Stalwart (Rust, AGPL-3.0 plus a proprietary enterprise licence), Maddy, Mox, Postfix and Dovecot (servers); Rust crates lettre, mail-parser, mail-auth, mail-builder, mail-send, async-imap (all MIT or Apache-2.0) | Third party for a long time (his answer). Later: section 3 | Section 3 |

### 2.2 Files and backup

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| OneDrive | Files reachable from every device, shared with others | **Partial.** The Files page with its places (FEATURES "Files Page"; `src/gui/pages/files/places.rs`), the server's shared library (`src/gui/pages/files/shared_files.rs`), uploads (`src/relay/storage/uploads.rs`), sealed attachments in messages. No sync between your own devices, no folders shared with chosen people. | Nextcloud, Seafile (beside); Syncthing (bridge or beside) | native: grow the Files page (section 6) | Today for your own devices and small shares; Funding: storage to keep everyone's files on servers |
| OneDrive Personal Vault | A locked folder for passports and insurance papers | **Partial.** Private files stay on your own computer; sealed attachments exist. | Cryptomator-style vaults (not researched) | native: a sealed folder in Files | Today |
| SharePoint, Lists, Access | An organisation's shared documents and pages; shared tables for tracking things | **None** (servers have channels and shared files, not team sites or shared tables) | Nextcloud, NocoDB and Baserow (both with licence traps, see part 2) | native: shared folders and shared tables for a group (section 10) | Today |
| Windows Backup | A new PC comes back the way the old one was | **Partial.** Your keys and settings are kept sealed on your server (FEATURES "Vault Sync"; `src/relay/storage/vault_sync.rs`); you can export everything a server holds about you (FEATURES "Account Export + Erase"; `src/relay/storage/account.rs`). The device mesh and personal encrypted backup are planned (ROADMAP). | restic, BorgBackup, Kopia; the `rustic_core` crate (Apache-2.0 OR MIT, Rust, restic's format) | native: device mesh first (your devices back each other up), embed `rustic_core` for the format | Today between your own devices; Funding: storage for server-side backup of everyone |

### 2.3 Documents

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Word | Letters, reports, CVs | **None.** Notes are Markdown; the Files page opens text files only. | LibreOffice and Collabora (MPL-2.0 core), ONLYOFFICE (AGPL), CryptPad, Etherpad; crates docx-rs, Typst | native viewer, then editor (section 4) | Today |
| Excel | Budgets, lists, sums, charts | **None** (a Calculator: FEATURES "Calculator"; `web/pages/calculator-app.js`) | LibreOffice Calc; crates calamine, rust_xlsxwriter, umya-spreadsheet, spreadsheet-ods (all permissive) | native viewer, then editor (section 4) | Today |
| PowerPoint | Slides for talks and lessons | **None** | LibreOffice Impress; no mature Rust library (research) | native viewer; our own writer crate later (section 4) | Today |
| Editor | Spelling and grammar help | **None** | Spelling dictionaries (not researched today) | native, later | Today |
| Visio | Flowcharts, floor plans, org charts | **None** | draw.io (Apache-2.0), Excalidraw (MIT) | native drawing with a bridge to those file formats (section 7) | Today |
| Sway, Publisher (retired October 2026), Designer | Newsletters, flyers, cards | **None** | Typst (Apache-2.0, Rust), Inkscape and Scribus-style tools (point) | embed Typst for printable flyers and newsletters | Today |

### 2.4 Calls and meetings

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Teams for personal use (Skype's replacement since 5 May 2025), Teams for work | Calls, video meetings, screen sharing | **Live on the website, partial on the desktop.** Calls with camera (FEATURES "Voice/Video Calls"; `web/chat/chat-voice-calls.js`), voice rooms on both apps (FEATURES "Voice Channels"; `web/chat/chat-voice-rooms.js`), screen sharing and streams (FEATURES "Screen Sharing / Streaming"), native voice. Native video and screen share are planned (ROADMAP). Big meetings through a forwarding unit are designed (`eight-billion.md`). | Jitsi Meet, BigBlueButton (beside); Matrix (bridge) | native: the existing ROADMAP rungs (native video, voice polish) | Today for small calls; Funding: bandwidth for very large meetings |
| Teams Phone (calling ordinary phone numbers) | A business phone line | **None** | Telephone gateways (not researched) | bridge, much later | Funding: carriers (and its own rules, not researched) |
| Teams Live Events (retired 30 June 2026), Stream | Broadcasts to many viewers | **Partial.** Streams on the website; the Studio page rehearses scenes (FEATURES "Studio Page"; `src/gui/pages/studio.rs`) | Owncast, PeerTube (beside) | native: the ROADMAP's self-hosted livestreaming rung | Today for small audiences; Funding: bandwidth for large ones |

### 2.5 Calendar and scheduling

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Outlook calendar | Appointments, reminders, shared family and team calendars, invitations | **Partial.** A calendar on each app, kept only on that device: the website's says "No server sync yet" (FEATURES "Calendar/Planner"; `web/pages/calendar.html`), the desktop's is kept in `personal.json` (FEATURES "Calendar Page"; `src/gui/pages/calendar.rs`, `src/gui/personal_store.rs`). No sharing, invitations or `.ics` files. | Radicale, Baikal (servers), DAVx5 (phone); crates `icalendar` (MIT/Apache-2.0) and `calcard` (archived) | native: sync between your own devices as sealed notes to yourself (the pattern Tasks of my own already uses: `src/gui/mine_tasks.rs`), shared calendars for groups, invitations with RSVP, `.ics` import and export (bridge, so a school's calendar file opens) | Today |
| Bookings | A page where customers or members pick a time | **None** | Cal.com (now "Cal.diy", personal use only per its README), Easy!Appointments | native, on the shared calendar (section 10) | Today |

### 2.6 Sign-in codes and passwords

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Microsoft Authenticator | Six-digit sign-in codes and approvals; Microsoft says its password autofill stopped: "Starting July 2025, the autofill feature in Authenticator will stop working, and from August 2025, passwords will no longer be accessible in Authenticator." | **None.** The vault holds only your own identity keys (FEATURES "Vault Sync"). | Aegis, ente Auth, 2FAS (phone); crate `totp-rs` (MIT); standards RFC 6238 and RFC 4226 | native, later (section 5) | Today in code; Funding: staff for an independent security audit before it is offered widely |
| Edge password manager, passkeys | Saving and filling passwords; signing in with a fingerprint instead | **None** | KeePassXC, Bitwarden, Vaultwarden; crates `keepass` (MIT), `passkey` (MIT OR Apache-2.0), `webauthn-rs` (MPL-2.0); WebAuthn Level 3 (a W3C Recommendation since 25 August 2026) | native, later, reading and writing KeePass files so people can leave (section 5) | Same as above |
| Entra ID (single sign-on), Entra Verified ID | One sign-in for many services; digital proofs a person keeps and shows | **Partial.** One identity across every server, no sign-up, proof of possession at connect (`docs/reference/cryptography.md`); signed credentials and vouches exist as signed objects (`storage-architecture.md`); the Skill Passport is on the ROADMAP. No "sign in with HumanityOS" for other websites. | Keycloak, Authentik, Kanidm (Rust, MPL-2.0), ZITADEL; W3C Verifiable Credentials 2.0 (a Recommendation since 15 May 2025) | bridge: speak the open standards so a HumanityOS identity can sign in elsewhere and a credential can be checked by others | Today |

### 2.7 Chat and communities

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Teams chat, GroupMe, Viva Engage (formerly Yammer) | Group chats, community spaces, announcements | **Live.** Channels, threads, reactions, pins, search, sealed DMs, peer-to-peer groups (FEATURES "Chat (Text Messaging)", "Threads", "Post-Quantum Sealed-Sender DMs"); guilds (FEATURES "Guild System"; `src/relay/storage/guilds.rs`) | Matrix, Mumble | native: keep growing (Track W parity) | Today |

### 2.8 Notes, boards and forms

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| OneNote, Loop | Notebooks; shared live lists and tables | **Partial.** Notes on each app, kept only on that device (FEATURES "Notes/Journal", "Notes Page"; `src/gui/pages/notes.rs`, `web/pages/notes-app.js`) | Joplin, Logseq, AppFlowy, Trilium (licence traps for some, see part 1) | native: sync as sealed notes to yourself, shared notebooks for groups through editing together (section 4.3) | Today |
| Whiteboard | A shared drawing board | **None** | Excalidraw (MIT), draw.io (Apache-2.0); tldraw is NOT open source (production use needs a paid key) | native canvas, Excalidraw file format as the bridge | Today |
| Forms | Surveys, quizzes, polls, sign-up sheets | **None** for forms. Signed voting is live (FEATURES "Native Governance page: live feed + signed voting + proposal form"; `src/gui/pages/governance.rs`) | LimeSurvey, Formbricks (open core) | native, beside the voting (section 7) | Today |

### 2.9 Tasks, projects and organisations

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| To Do, Planner | Personal to-do lists; team task boards | **Live.** Tasks page, Tasks of my own, comments, projects (FEATURES "Tasks of my own (Mine)", "Task CRUD", "Project Grouping"; `src/gui/pages/tasks.rs`, `src/gui/mine_tasks.rs`) | OpenProject, Vikunja, Kanboard (maintenance mode), Taiga | native: due dates and reminders through the notifications that exist (FEATURES "Desktop and browser notifications") | Today |
| Project | Schedules, dependencies, resources | **None** | OpenProject, Plane | native, later (a timeline view on Tasks) | Today |
| Exchange, SharePoint, Teams, Bookings, Entra as an organisation's toolkit; Dynamics 365 | Running a club, co-op or small business: members, roles, shared calendar, files, sign-ups | **Partial.** Servers, roles, guilds, tasks, a marketplace, signed governance and an admin action registry exist; there is no one place where a group runs itself | Nextcloud, OpenProject | native: organisation tools (section 10) | Today |

### 2.10 Photos and video

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Photos app, OneDrive photo backup | Looking through photos by date, albums, phone photos saved automatically | **Partial.** The Files page shows images; uploads strip hidden details such as location (FEATURES "Upload Metadata Stripping"). No library view, no phone backup. | Immich (AGPL), PhotoPrism (paid parts), digiKam | native: a view on Files plus phone backup (section 6.2) | Today to your own PC; Funding: storage to keep everyone's photos on servers |
| Clipchamp | Trimming and editing home videos | **None** for people's own videos (the Clip Maker films the game: FEATURES "Clip Maker (movie mode)") | Kdenlive, Shotcut, OpenShot (point, already on the Apps page) | point now; a native trimmer later | Today |

### 2.11 Family and devices

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Family Safety | Screen-time limits and content filters for children. Microsoft says: "Location and driving safety features in Microsoft Family Safety are going away." | **Partial.** Safety tools on both apps: who can reach me, Block, reports, warnings, help outside the server, and a protected setup a parent can lock with a PIN (FEATURES "Safety: who can reach me, Block, reports, warnings, protected setup"; `src/gui/pages/safety.rs`) | No mature open screen-time app for Windows was found (TimeLimit is Android only); AdGuard Home and Pi-hole filter a home network (beside) | native, on the Safety page (section 8) | Today |
| Intune, Defender | Managing an organisation's laptops; antivirus and identity monitoring | **None.** The "My Devices" dashboard is planned (ROADMAP, device mesh). | Fleet (open core), ClamAV (GPL, beside a server) | native for your own devices only; ClamAV beside a server for shared uploads; antivirus itself stays the operating system's job | Today |
| Phone Link | Texts and calls from the phone on the PC | **None** | KDE Connect-style tools (not researched) | after the native phone app | Today, later |

### 2.12 Browser, search, maps, translation, news

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Edge | Browsing | **Partial by design.** Our own readable web, no JavaScript, off by default (FEATURES "The Readable Web"; `src/web_reader/`, `src/gui/pages/browser.rs`); an embedded browser is agreed only for watching video platforms (CLAUDE.md, `embedded-browser.md`) | Firefox (point), Servo (Rust, MPL-2.0) | keep the reader; point to a full browser | Today |
| Bing search | Finding things on the web | **None** for the web (search inside chat and the market is live: FEATURES "Message Search", "Full-Text Search (FTS5)") | SearXNG (AGPL, beside), YaCy | beside: a server may run SearXNG; an index of our own is not proposed | Today with SearXNG; Funding: compute and storage for an index of our own |
| Bing Maps | Places and directions | **Partial.** The Maps page with real OpenStreetMap regions and whole-Earth map packs (FEATURES "Maps Page", "Whole-Earth Maps"; `src/gui/pages/maps/planet_view.rs`). No directions. | OpenStreetMap (data under ODbL, share-alike), Organic Maps | native: offline directions on the map packs (routing engines not researched today) | Today |
| Translator | Translating messages, signs, documents | **Partial.** The app's own words in 5 languages (FEATURES "Localization (i18n)"). No translation of what people write. | LibreTranslate, Argos Translate (beside); OPUS-MT models run by the `candle` crate (MIT OR Apache-2.0; the models need credit, CC BY 4.0) | native, on the person's own computer, an optional download (AI is optional): last in the order (his answer) | Today |
| MSN (news, weather) | One feed of news and weather | **None** for people (the game reads live weather for its sky) | RSS readers (point) | later; low priority | Today |

### 2.13 AI helper

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Copilot, Microsoft 365 Copilot | Asking questions, help writing, summaries | **Partial.** Optional downloadable models run on your own computer for talking characters (FEATURES "Embeddable AI, rung 2"; `src/ai/`, `src/gui/pages/settings/ai.rs`) | llama.cpp, Ollama, mistral.rs (MIT); Open WebUI has a branding clause above 50 users | native: an optional helper that uses the same chosen download; every feature still works without it (CLAUDE.md "AI is optional") | Today on the person's own computer; Funding: compute for hosted AI (not proposed) |

### 2.14 Data and automation

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| Power BI | Charts and dashboards from your data | **Partial.** Community dashboards (FEATURES "Civilization Dashboard", "Live Community Stats") | Apache Superset, Metabase (open core), Grafana | native charts on the spreadsheet rung | Today |
| Power Automate | Making repetitive steps happen by themselves | **None** for people. Server admins have an action registry an AI can list (FEATURES "Admin action registry") | Node-RED (Apache-2.0, beside); n8n is not OSI open source | native, later, on the same registry idea | Today |
| Power Apps, Power Pages | Simple apps and sign-up portals without programming | **None.** The schema-driven "Make-a-Thing" editor is on the ROADMAP. | Budibase, Appsmith (open core) | native: the Make-a-Thing rung, plus forms (section 7) | Today |

### 2.15 Learning, work, code, games, hosting

| Microsoft | What people use it for | HumanityOS now | Open alternatives | Route | Tag |
|---|---|---|---|---|---|
| LinkedIn | A work profile, contacts, jobs | **Partial.** Profiles, peer-endorsed signed skills (FEATURES "Reputation System"), a marketplace. No job listings. | No mature open federated alternative found | native: work listings in the market, the Skill Passport (ROADMAP) | Today |
| LinkedIn Learning, Microsoft Learn | Courses and certificates | **Partial.** The Library and its guides in the app (FEATURES "Library Tags"); learning paths are on the ROADMAP | Khan Academy, MIT OpenCourseWare (point, already in the catalog) | native: learning paths with peer-endorsed skills | Today |
| GitHub | Hosting code | **Partial** for the project itself (GitHub plus a Forgejo mirror). None for people. | Forgejo, Gitea (beside) | point | Today |
| Xbox, Game Pass, Minecraft | Games, a game identity, a game library | **Live, differently.** The game itself and the Apps page's games library (FEATURES "Apps Page"; `src/gui/pages/apps.rs`) | (our own) | native | Today; Funding: compute for streamed play |
| Microsoft Store | Finding and installing apps | **Partial.** The Apps page tracks installs and launches and recommends open programs | (our own) | native | Today |
| Azure, Windows 365 | Renting computers and storage; a PC run in a data centre | **Partial.** The desktop app is itself a server, hosted from inside the app (`in-app-ops.md`, "Hosting a node from inside the app") | Proxmox-style hosting (not researched) | native: community-run servers | Today for small servers; Funding: compute for hosted capacity |

### 2.16 Not proposed

Microsoft Rewards (points for using products), Surface and other hardware, Purview and the
enterprise compliance suites, and Microsoft's marketplace and licensing programmes. They serve a
company selling products, not a need a person has that HumanityOS lacks; the fleet ledger already
records what each person used and contributed (FEATURES "Fleet ledger").

## 3. Email

### 3.1 The decision

Email stays third party for a long time, and a bridge comes later (his answers above). This section
keeps the options and their costs on record so the later rung starts from facts, not memory.

What is already true: between HumanityOS people the chat does what email does, sealed so the server
cannot read it, with no address to sign up for. The gap is mail from banks, schools and employers.

### 3.2 The options

Sources: the research notes
[`email-bridge.md`](../reference/research/2026-11-14-microsoft-services/email-bridge.md) and the two
findings [`2026-11-14-email-sending-rules-and-host-terms.md`](../reference/findings/2026-11-14-email-sending-rules-and-host-terms.md)
and [`2026-11-14-running-a-mail-service-law.md`](../reference/findings/2026-11-14-running-a-mail-service-law.md).
Neither finding is legal advice.

| Option | What it means for a person | Costs and risks | Tag |
|---|---|---|---|
| 1. Stay separate | Keep using your current mail provider; HumanityOS links to it. | None for the project. People keep two places to check. | Today (this is the current state) |
| 2. Connect the app to your existing mailbox | The app reads and sends through the provider you already have (the open mail standards IMAP and JMAP), like Thunderbird does. Our servers never hold your mail. | Permissive Rust crates exist (async-imap, mail-parser, mail-send, lettre). The sending rules are your provider's job. Washington's breach law counts an email address with its password as personal information, so the password (or its sign-in token) stays on your device only. Big providers may require their own sign-in flow for third-party mail apps (not researched today). Exchange Web Services, one doorway to company mailboxes, is being switched off from 1 October 2026 and is gone on 1 April 2027 (research notes, part 1), so a client should speak IMAP and JMAP. | Today |
| 3. Receive only | An address at a HumanityOS server (for example `name@` a server's domain) that accepts mail and delivers it into the app. | None of Gmail, Yahoo or Outlook.com sets rules for receiving their mail. Needs port 25 open inbound, a mail-exchange record in the domain's settings, and spam filtering (Spamhaus's lists are "free of charge for low-volume, non-commercial users"). Replies would have to go out some other way. In law, a service that lets people "send or receive" mail is an "electronic communication service" (Stored Communications Act); whether a free volunteer server serves "the public" is for a lawyer. Namecheap's terms could not be read (every page refused automated reading), so whether united-humanity.us may run mail is unknown. | Today for a small server; Funding: staff once many people rely on it |
| 4. Send too | Replies and new mail go out from that address. | Every sender to Gmail and Yahoo needs SPF or DKIM, valid reverse DNS and spam complaints under 0.3%; above about 5,000 messages a day to one provider a sender is "bulk" and also needs DMARC and one-click unsubscribe, and Gmail counts all mail from one domain together, permanently. One abusive account on a shared domain can get everyone's mail rejected. A new server address can already be on a blocklist. This is mostly a cost in people: an abuse desk and someone watching delivery. | Funding: staff (abuse, deliverability), dedicated addresses |
| 5. A person's own domain | You buy a domain and point it at the app or your server, so your address survives a change of server. | The domain's yearly fee is the person's. The setup (DNS records) needs an in-app guide, per the GUI-first rule. Sending from it carries option 4's rules. | Today for receiving; sending as option 4 |
| 6. Forwarding alias | An address that passes mail straight on (into the app, or to another mailbox) and keeps nothing. | Almost no storage. Forwarding to another provider can break the sender's authentication; ARC (RFC 8617) exists for that case. | Today |

**Held mailboxes versus delivered and dropped.** Microsoft's free tier keeps 15 GB of mail per
person. A HumanityOS server can instead seal each arriving message to the person's key (the key card
every app already checks before sealing anything) and keep it only until their devices fetch it,
the way the sealed message mailbox already works. Then the server holds almost nothing for long.
Honest limit: standard email arrives at the receiving server readable (the encryption between
servers protects only the journey), so for the moment of arrival the server could read it. Sealing
on arrival protects it from then on.

### 3.3 "How would giving our own users an email address work for billions of users?"

In plain words:

- **Email is already federated, like our servers.** Every address names its server's domain
  (`you@example.org`), and servers find each other through the domain's records. So the natural
  shape is that each HumanityOS server that chooses to offer mail gives addresses on its own domain:
  `name@united-humanity.us`, `name@public.guide`, `name@` your town's server. Billions of people
  means millions of small mail servers, not one giant one, which is how email itself grew.
- **One shared domain for everyone would not work.** Gmail counts every message from one primary
  domain together and treats it as bulk once it passes about 5,000 a day, permanently (finding).
  Billions of people on one domain would share one reputation: one spammer would get everyone's mail
  rejected, and it would need one central operator with a large abuse team, which is the "home
  server" HumanityOS does not have.
- **A person's own domain is the most portable.** The address belongs to the person, follows them
  between servers, and its reputation is theirs. It costs them a small yearly fee and a setup the
  app would walk them through.
- **Forwarding or delivered-and-dropped keeps storage small at any scale.** If mail is sealed and
  handed to the person's devices, a server stores little per person, so storage is not what limits
  billions. Held mailboxes (keeping everyone's mail on servers for years) are what would need
  storage funding.
- **The real cost at scale is abuse, in both directions.** Receiving needs spam filtering; sending
  needs people who stop abusers and keep the servers' addresses off blocklists. That is staff, and
  it is why sending is the last email rung and tagged Funding.
- **Each server operator would carry the law for their server**: the Stored Communications Act's
  limits on disclosure and its warrant rule, the duty to report child sexual abuse material on
  actual knowledge, and breach notification (finding). A per-server design keeps that with whoever
  chose to offer mail.

### 3.4 When the bridge comes: the recommended order

1. **Option 2, the client, first.** No server holds anyone's mail, no deliverability work, no new
   legal exposure for the servers, and it lands the "better than Thunderbird" aim where it can be
   met: mail shown as conversations the way the chat shows them, one inbox beside HumanityOS
   messages, plain words everywhere, safe defaults (remote images off until asked, the warnings on
   messages that already exist checking links), and the password kept on the device.
2. **Option 6 then option 3**: receive-only and forwarding addresses per server, sealed on arrival,
   opt-in for each server's operator, after Namecheap's terms are read.
3. **Option 5**: a person's own domain, with an in-app guide for its records.
4. **Option 4, sending, last**, when there is funding for the people it needs.

## 4. Office documents

### 4.1 May we?

The finding [`2026-11-14-office-file-formats.md`](../reference/findings/2026-11-14-office-file-formats.md)
(not legal advice): Microsoft's Open Specification Promise is irrevocable, needs no signature, covers
open-source and partial implementations, and names ECMA-376, ISO/IEC 29500 and Microsoft's own
extension documents for .docx, .xlsx and .pptx. OpenDocument's committee works royalty-free, and ODF
1.4 has been an OASIS Standard since 6 October 2025. The remaining risk is patents held by other
companies, which no promise covers, and whether the promise reaches the newest editions, which is a
lawyer's reading. Trademarks ("Word", "Excel") were not researched, so the app should name formats
(".docx document") rather than products.

### 4.2 What it would take

From the research notes
[`office-formats-and-co-editing.md`](../reference/research/2026-11-14-microsoft-services/office-formats-and-co-editing.md):
spreadsheets are well served in Rust (calamine reads .xlsx and .ods, rust_xlsxwriter writes .xlsx,
umya-spreadsheet and spreadsheet-ods do both); docx-rs has read and written .docx since 2019;
nothing mature exists for .pptx, .odt or .odp; Typst, printpdf and lopdf make PDFs (permissive);
MuPDF is AGPL, so it could only run beside us; LibreOffice's headless conversion is the fallback,
also beside us. By the house rule "build what does not exist", the missing readers and writers
become HumanityOS crates.

The rungs:

1. **View** .docx, .xlsx, .pptx, .odt, .ods, .odp and PDF on the Files page, drawn by our own code
   into the same kind of readable document the web reader already makes (headings, paragraphs,
   tables, images; slides as pages). Because it is our own drawing, the same view goes onto an
   in-world screen (the operator: "display those sort of documents on in-game displays") and the
   website mirrors it, the way the readable web and the wall screens already share one page
   (`in-world-screens.md`, `readable-web.md`).
2. **Edit** spreadsheets and documents on both apps; save as .xlsx, .ods and .docx; print or export
   through Typst.
3. **Edit together** (4.3).
4. **Slides and OpenDocument text**: our own .pptx writer and .odt and .odp crates.

All along, LibreOffice stays on the Apps page for anything the app cannot do yet. Tag: Today for all
four rungs.

### 4.3 Editing together over the federated servers

A shared document is a CRDT (a data structure that merges everyone's changes on their own device, in
any order, and always arrives at the same result). Operational transformation, the other approach,
needs one server that sees every change in the clear (Etherpad's own design), so it cannot be sealed.
The research ranks Yrs first (MIT, Rust, and it speaks the protocol of Yjs, so the website could use
the JavaScript original and still merge with the desktop app), then Loro, then Automerge, all MIT.

How it fits [`storage-architecture.md`](storage-architecture.md): a document behaves like a
peer-to-peer group. Its members hold a document key; each batch of edits is a CRDT update sealed
under that key (AES-256-GCM, exactly as group messages are: `docs/reference/cryptography.md`, "P2P
group messages") and carried in a signed object by its author's Dilithium3 / ML-DSA-65 identity, so
every edit says who made it and no server can read or alter it. Servers store and pass on the sealed
updates like any other signed object; a member who was offline merges them on return; and a
snapshot now and then keeps the history from growing without end. No new algorithm is needed. Tag:
Today for documents of ordinary size.

## 5. Sign-in codes and passwords: later, to the bar they need

The operator: "This sounds like a big difficult task too." It is, because a mistake here hands
someone else every account a person has. So it waits, and when it comes it is built to this bar:

- Secrets are sealed on the device under the vault that already exists and travel only sealed
  (vault sync); no server ever sees one.
- Sign-in codes follow the published standards (RFC 6238, RFC 4226) with a small permissive
  library (`totp-rs`, MIT), and import from the open authenticator apps' export files.
- Passwords are kept in the KeePass file format (the `keepass` crate, MIT), so a person can open
  them in KeePassXC and leave at any time.
- A written threat model first, and an independent audit before it is offered to everyone (Funding:
  staff). Passkeys (WebAuthn Level 3) come after codes and passwords.

## 6. Files and photos

### 6.1 Files

The operator: "Which fits with fleshing out of file explorer. Right now I can view GLBs and listen to
music and move things around but, it's still far from perfect." So these belong to the Files page
(`files-showcase.md`, `file-transfer-at-scale.md`), not a new page:

- **Version history**: the HumanityOS folder already keeps one previous version (`<name>.bak`, Put
  back the previous version); extend it to every place and many versions.
- **Folders shared with chosen people**, sealed the way message attachments already are.
- **Sync between your own devices**: the device mesh rung on the ROADMAP.
- **Files at scale**: the existing plan (pieces under one fingerprint, sent device to device).

Tag: Today. Keeping everyone's files on servers for the long term is Funding: storage.

### 6.2 Photos: "You mean something different from the current file upload system?"

Yes, but built on top of it, not beside it. The upload system is for **sharing**: a picture posted in
a chat, at most 10 MB, with its hidden details stripped. A photo library is for **keeping and
finding** your own pictures, and it is two things layered on Files, never a second store:

1. **A view.** The same image files, shown by the date they were taken, in albums (which are
   folders or tags underneath), with a large viewer. Finding faces or things in pictures would be
   an optional on-device model, never required.
2. **Automatic backup from the phone.** Once the native phone app exists, it copies each new photo
   into the person's own sealed space: first to their own PC at home (the device mesh, free), later
   to a server with a fair share of space.

Tag: Today for the view and for backup to your own PC; Funding: storage to keep everyone's photos on
servers.

## 7. Forms and surveys, beside the signed voting

The operator: "Especially the forms and surveys. I guess a survey is kinda like a vote but,
different." The difference, in plain words:

- **A vote** decides one thing for a group. It needs one answer per member, a tally everyone can
  check, and rules (a voting window, quorum). That is `proposal_v1` and `vote_v1`, live today.
- **A survey or form** gathers information. It has many questions of different kinds (a choice, a
  number, a sentence, a date), it may be open to people outside the group (a sign-up sheet), the
  answers often go privately to whoever asked, and it decides nothing.

So forms reuse what voting already has (signed answers, a signed question set, the same page
patterns) and add question types, private answers sealed to the form's owner, an anonymous option
where the owner allows it, and a summary view. A sign-up sheet for an event is a form joined to the
calendar. Tag: Today.

## 8. Family safety, on the Safety tools

The operator: "I guess we already have partial family safety controls." True: Safety on both apps
has who can reach me, Block, reports, warnings, help outside the server, and a protected setup a
parent can lock with a PIN (`blocking-and-safe-mode.md`). Next, on the same page and still checking
no one's age:

- A **family group**: a parent's devices see the child's safety settings and can change them under
  the protected setup.
- **Time limits** inside HumanityOS, the game included (the app can limit itself; it cannot police
  the rest of the computer, and no mature open screen-time program for Windows was found).
- **Opt-in location sharing** within the family, sealed between their own devices. Microsoft is
  removing its own: "Location and driving safety features in Microsoft Family Safety are going
  away."

Tag: Today.

## 9. The native app on the phone

The operator: "It would be nice to have the native app on my one plus nine pro instead of the PWA
version." What it takes, in outline: building the same Rust app for Android, a touch layout for
every page (the website's phone layout shows the sizes), a simplified mode for the 3D world on a
phone's graphics, notifications that arrive while the app is closed, the identity key kept in the
phone's protected storage, and a way to install it. The platform work and its costs are in the
platforms design document another agent is writing (platforms.md, in this design folder). Photo
backup (6.2) and sign-in codes (5) both wait on it.

## 10. Organisation tools (high priority)

The operator: "This definitely sounds high priority." Here it means the tools a group (a volunteer
group, a club, a cooperative, a school class, a small business) uses to run itself, which is the
mission's own unit ("turn groups into cooperatives", ROADMAP). Most of the parts exist; what is
missing is one place where they come together for a group:

- **Members and roles**: who belongs, who may do what (servers and guilds already have members and
  roles).
- **A shared calendar with RSVP** and reminders (2.5).
- **Sign-up forms and surveys** (section 7) and a **booking page** for members or the public.
- **Shared folders** (6.1) and **shared tables** (the Lists idea: a member list, an inventory, a
  delivery route).
- **The task board** that exists, and **announcements** (a channel only the organisers post in).
- **Decisions** through the signed voting that exists.
- **Printable material** (flyers, newsletters) through Typst.

All of it runs on servers, groups and signed objects that already exist. Tag: Today.

## 11. The suggested order

The brief asked to lead with what a person would need most if their usual provider went down: mail,
files, documents, calls, calendar, sign-in codes. Of those, messages and calls already work, mail is
third party by decision, and sign-in codes wait for their security bar. With his answers and the
"Today first" rule, the order is:

1. **Organisation tools with the shared calendar** (section 10, 2.5). Today.
2. **Files**: version history, shared folders, sync between your own devices, the photo view (6). Today.
3. **Office documents**: view, then edit, then edit together (4). Today.
4. **Forms and surveys** (7). Today.
5. **Notes that sync and share, and a whiteboard** (2.8). Today.
6. **Calls**: the native video and voice rungs already on the ROADMAP (2.4). Today.
7. **Family safety** additions (8). Today.
8. **Sign-in codes and passwords**, to the bar in section 5. Code today; audit Funding.
9. **The mail bridge**, later: the client first (3.4). Today for the client; sending is Funding.
10. **An optional AI helper**, then **translation and captions**, last (2.12, 2.13). Today on the
    person's own computer.
11. **The Funding tier**, only with real funding: holding mailboxes and sending mail at scale,
    server backup of everyone's files and photos, calling ordinary phone numbers, a web search index
    of our own, streamed games and cloud PCs.

The phone app (9) runs on its own track (the platforms document) and unlocks photo backup and
sign-in codes on the phone.

## 12. Open questions for the operator (each with a recommendation)

1. **Which group goes first for organisation tools?** Recommendation: a real volunteer group you
   already help run, so every part is tested on a group that actually needs it.
2. **When the mail bridge comes, start with the client to people's existing mailboxes?**
   Recommendation: yes (section 3.4); it keeps mail off our servers and puts the "better than
   Thunderbird" aim where it can be met.
3. **Should the project's own servers ever offer addresses on their domains?** Recommendation: only
   receive-and-seal and forwarding, opt-in per server operator, never one shared domain for
   everyone, and only after Namecheap's terms are read (they refused automated reading).
4. **Keep passwords in the KeePass format?** Recommendation: yes, so people can always leave.
5. **Photo backup: to the person's own PC first, servers later?** Recommendation: yes; it costs the
   project nothing and keeps photos at home.

## Sources

- Research notes, read 2026-11-14:
  [README](../reference/research/2026-11-14-microsoft-services/README.md),
  [Microsoft part 1: productivity and communication](../reference/research/2026-11-14-microsoft-services/microsoft-1-productivity-and-communication.md),
  [Microsoft part 2: identity, security, consumer and platform](../reference/research/2026-11-14-microsoft-services/microsoft-2-identity-security-consumer-platform.md),
  [open alternatives part 1](../reference/research/2026-11-14-microsoft-services/open-alternatives-1-documents-to-photos.md),
  [open alternatives part 2](../reference/research/2026-11-14-microsoft-services/open-alternatives-2-identity-to-ai.md),
  [email bridge](../reference/research/2026-11-14-microsoft-services/email-bridge.md),
  [office formats and editing together](../reference/research/2026-11-14-microsoft-services/office-formats-and-co-editing.md).
- Dated findings:
  [email sending rules and host terms](../reference/findings/2026-11-14-email-sending-rules-and-host-terms.md),
  [running a mail service: the law](../reference/findings/2026-11-14-running-a-mail-service-law.md),
  [office file formats](../reference/findings/2026-11-14-office-file-formats.md).
- Related designs: [`two-realities.md`](two-realities.md), [`in-app-ops.md`](in-app-ops.md),
  [`storage-architecture.md`](storage-architecture.md), [`eight-billion.md`](eight-billion.md),
  [`file-transfer-at-scale.md`](file-transfer-at-scale.md), [`files-showcase.md`](files-showcase.md),
  [`blocking-and-safe-mode.md`](blocking-and-safe-mode.md), [`in-world-screens.md`](in-world-screens.md),
  [`readable-web.md`](readable-web.md), [`embeddable-ai.md`](embeddable-ai.md).
