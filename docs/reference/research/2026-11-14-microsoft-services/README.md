# What Microsoft's services do for people, and the open alternatives (read 2026-11-14)

**Why this exists.** On 2026-11-14 the operator asked: "I'm wondering what services they provide that
our app is still missing. We want to make HumanityOS a one stop shop holistic app for everyone's
needs... eventually. Our chat system is essentially email but, more convenient to use." Six research
helpers read primary sources the same day: Microsoft's own product and support pages, each open
project's own licence file and repository, the RFCs, the mail providers' sender rules, the hosts'
terms and the statutes. Every file quotes its sources exactly, with each source's own date, and ends
with the robots.txt files read first and the pages that could not be read.

**What this is not.** A scan, not a decision. The gap analysis, the routes, the "Today or Funding"
tags, the operator's answers and the suggested order are in
[`../../../design/everyday-suite.md`](../../../design/everyday-suite.md); the build order is in
[`../../../ROADMAP.md`](../../../ROADMAP.md), section "Everyday life suite". A tool marked as a
candidate here still needs its own dated finding before it is downloaded, bundled or built on.

| File | What it covers |
|---|---|
| [microsoft-1-productivity-and-communication.md](microsoft-1-productivity-and-communication.md) | Outlook, Exchange, Teams, OneDrive, SharePoint, Lists, Word, Excel, PowerPoint, OneNote, Loop, Whiteboard, Forms, To Do, Planner, Project, Visio, Bookings, Clipchamp, Stream, Sway, Access, Editor, Designer, the Microsoft 365 plans, and what Microsoft has retired (Skype, Publisher, Lens, Project Online, Windows Mail and Calendar, Teams Live Events, Exchange Web Services). www.microsoft.com refused automated reading, so every fact comes from support.microsoft.com and learn.microsoft.com. |
| [microsoft-2-identity-security-consumer-platform.md](microsoft-2-identity-security-consumer-platform.md) | Copilot, Defender, Authenticator (its password autofill retired), Entra ID and Verified ID, Intune, Family Safety (its location features going away), Edge, Bing, Translator, the Power Platform, Windows Backup, Photos, Phone Link, the Store, Windows 365, Xbox and Game Pass, LinkedIn, GitHub, Azure, MSN, Learn, Rewards, Dynamics 365, Purview, Minecraft, and everything else Microsoft's own menus list. LinkedIn and MSN bar Claude in robots.txt. |
| [open-alternatives-1-documents-to-photos.md](open-alternatives-1-documents-to-photos.md) | Open alternatives for documents, files, calls, calendars, notes, whiteboards, forms, tasks, video, photos and backup: publisher, licence quoted, hardware, language, latest release, fit (embed, beside, bridge, point). tldraw is not open source; several projects are open core. |
| [open-alternatives-2-identity-to-ai.md](open-alternatives-2-identity-to-ai.md) | Open alternatives for passwords and sign-in codes, identity, devices and family safety, browsers, search, maps, translation, dashboards and automation, malware scanning, code hosting, local AI and job networks, plus the standards (RFC 6238, RFC 4226, WebAuthn Level 3, Verifiable Credentials 2.0, the OpenStreetMap licence). n8n and NocoDB are not open source. |
| [email-bridge.md](email-bridge.md) | The mail standards (DMARC's RFC 7489 was replaced by RFCs 9989 to 9991 in May 2026), blocklists, the open mail servers and Rust mail crates with their licences, and one published price as an example. Its legal and terms points are in two findings. |
| [office-formats-and-co-editing.md](office-formats-and-co-editing.md) | Office Open XML and OpenDocument, the Rust readers and writers for each format, LibreOffice as a separate converter, and the libraries for editing together (Yrs, Loro, Automerge and others), ranked by fit. Its permission question is in a finding. |

**The dated findings this research produced:**

- [`../../findings/2026-11-14-email-sending-rules-and-host-terms.md`](../../findings/2026-11-14-email-sending-rules-and-host-terms.md)
- [`../../findings/2026-11-14-running-a-mail-service-law.md`](../../findings/2026-11-14-running-a-mail-service-law.md)
- [`../../findings/2026-11-14-office-file-formats.md`](../../findings/2026-11-14-office-file-formats.md)

**Method notes for the next researcher.** www.microsoft.com answered an honestly identified reader
with "Your request has been blocked"; Namecheap's pages, ISO's pages, app.leg.wa.gov (the Washington
statutes) and EUR-Lex refused or barred automated reading on this date, and Codeberg and F-Droid bar
Claude in robots.txt. None of these was worked around. One helper's crates.io requests named the
project in their user agent, against the privacy rule; future requests should use a generic tool name.
