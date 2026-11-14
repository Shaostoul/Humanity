# A bridge to standard email: standards, blocklists, software, costs

What this file is: research notes for the question of whether, and how, HumanityOS could let its
people reach standard email (the kind banks, schools and employers send) while their chat with each
other stays on HumanityOS's own sealed messages. It lists the standards a bridge would have to speak,
what the big blocklist says about a new server's address, the software that could do the work (with
licences), and one published price as an example. The rules of the big mailbox providers, our two
server hosts' terms and the law are in two dated findings, summarised here:
[`../../findings/2026-11-14-email-sending-rules-and-host-terms.md`](../../findings/2026-11-14-email-sending-rules-and-host-terms.md)
and [`../../findings/2026-11-14-running-a-mail-service-law.md`](../../findings/2026-11-14-running-a-mail-service-law.md).
It says nothing about what we should build; that is a design decision.

The five options weighed: (1) stay separate, no email at all; (2) receive only (an address at a
HumanityOS server that accepts mail and delivers it into the app, sealed to the person on arrival);
(3) send too; (4) a person's own domain; (5) connect the app to the person's EXISTING mailbox (the
app is a mail client; the server never holds their mail).

Read on 14 November 2026 (Pacific).

Method:
- Readers: `curl -A "Claude-User (research tool)"` (an honest identity), with each page's text
  extracted by a small script and quotations checked against that text; WebFetch only where curl was
  refused (the FTC, marked). GitHub facts through `gh api`; crate facts through the crates.io API, one
  request a second. Each host's robots.txt was read first, in a step of its own (list at the end).
- Primary sources only: the RFC Editor, each provider's own page, each host's own legal pages, each
  project's own licence file or registry entry.
- Quotations are exact (curly quotes normalised to straight). Where a quotation is part of a sentence,
  it is still the source's exact words.
- **Nothing here is legal advice.** It is an AI's reading of what the sources say.

Words used below, in plain terms:
- **SMTP** is how mail servers hand messages to each other (on network **port 25**), and how a mail
  app hands a new message to its own server to send (**submission**, ports 465 or 587).
- An **MX record** is the line in a domain's address book (DNS) that says which server receives its
  mail. A **PTR record** (reverse DNS) says which name belongs to a server's numeric address.
- **IMAP** and **JMAP** are how a mail app reads a mailbox that lives on a server.
- **SPF**, **DKIM** and **DMARC** are the three checks receivers use to tell real mail from forged
  mail: SPF lists the servers allowed to send for a domain, DKIM is a signature on each message, DMARC
  is the domain's published rule tying the visible From address to the other two. **ARC** keeps those
  results intact when a message is forwarded.
- A **blocklist** (DNSBL) is a published list of server addresses that receivers refuse or distrust.

## Part A. The standards a bridge must speak

Source for every row: the RFC's own text at `https://www.rfc-editor.org/rfc/rfcNNNN.txt` and its
metadata at `https://www.rfc-editor.org/rfc/rfcNNNN.json` (read 14 November 2026). The date is the
RFC's publication date on its title page. The quotation is the first sentence of its Abstract.

| RFC | What it is | Date, status | First sentence of the Abstract | What it means for the bridge |
|---|---|---|---|---|
| 5321 | SMTP | October 2008, Draft Standard; updated by RFC 7504 | "This document is a specification of the basic protocol for Internet electronic mail transport." | Options 2, 3 and 4: any server with a receiving address must speak it on port 25; sending uses it too. |
| 5322 | Internet Message Format | October 2008, Draft Standard; updated by RFC 6854 | "This document specifies the Internet Message Format (IMF), a syntax for text messages that are sent between computer users, within the framework of "electronic mail" messages." | Every option that touches mail: how headers (From, To, Date, Subject) and the body are laid out. Yahoo requires it of senders (finding). |
| 9051 | IMAP version 4rev2 | August 2021, Proposed Standard; obsoletes RFC 3501 | "The Internet Message Access Protocol Version 4rev2 (IMAP4rev2) allows a client to access and manipulate electronic mail messages on a server." | Option 5: how the app would read the person's existing mailbox. |
| 8620 | JMAP (the core protocol) | July 2019, Proposed Standard; updated by RFC 9404 and RFC 9670 | "This document specifies a protocol for clients to efficiently query, fetch, and modify JSON-based data objects, with support for push notification of changes and fast resynchronisation and for out-of- band binary data upload/download." | Option 5, for mailboxes that offer it: a newer, web-style alternative to IMAP. |
| 8621 | JMAP for Mail | August 2019, Proposed Standard | "This document specifies a data model for synchronising email data with a server using the JSON Meta Application Protocol (JMAP)." | Option 5, as above; also how the app could talk to a HumanityOS mail server in options 2 to 4. |
| 6376 | DKIM signatures | September 2011, Draft Standard; updated by RFC 8301, 8463, 8553, 8616 | "DomainKeys Identified Mail (DKIM) permits a person, role, or organization that owns the signing domain to claim some responsibility for a message by associating the domain with the message." | Options 3 and 4: outgoing mail must be signed (Google, Yahoo, Microsoft). Options 2 to 4: incoming signatures are checked to judge forgery. |
| 7208 | SPF | April 2014, Proposed Standard; obsoletes RFC 4408; updated by RFC 7372, 8553, 8616 | "Email on the Internet can be forged in a number of ways." (the next sentence: "existing protocols place no restriction on what a sending host can use as the "MAIL FROM" of a message") | Options 3 and 4: publish which servers may send; options 2 to 4: check it on arrival. |
| 7489 | DMARC (first version) | March 2015, Informational; **obsoleted by RFC 9989, 9990 and 9991** | "Domain-based Message Authentication, Reporting, and Conformance (DMARC) is a scalable mechanism by which a mail-originating organization can express domain-level policies and preferences for message validation, disposition, and reporting, that a mail-receiving organization can use to improve mail handling." | The providers' rules still say "DMARC"; the standard itself was replaced in May 2026 (next row). |
| 9989 | DMARC (the standard that replaced 7489) | May 2026, Proposed Standard; obsoletes RFC 7489 and 9091 | "This document describes the Domain-based Message Authentication, Reporting, and Conformance (DMARC) protocol." | Options 3 and 4: publish a DMARC policy (required of bulk senders); options 2 to 4: apply senders' policies on arrival. |
| 9990 | DMARC aggregate reports | May 2026, Proposed Standard; obsoletes RFC 7489 | "Domain-based Message Authentication, Reporting, and Conformance (DMARC) allows for Domain Owners to request aggregate reports from receivers." | Optional: daily reports of who sends as our domain. |
| 9991 | DMARC failure reports | May 2026, Proposed Standard; obsoletes RFC 7489; updates RFC 6591 | "Domain-based Message Authentication, Reporting, and Conformance (DMARC) is a mechanism by which a Domain Owner can request feedback about email messages using their domain in the From: address field." | Optional; failure reports can carry message details, a privacy question. |
| 8617 | ARC | July 2019, **Experimental** ("published for examination, experimental implementation, and evaluation") | "The Authenticated Received Chain (ARC) protocol provides an authenticated "chain of custody" for a message, allowing each entity that handles the message to see what entities handled it before and what the message's authentication assessment was at each step in the handling." | Only if a server forwards mail onward (Microsoft's FAQ names it for forwarding; finding). Not needed for options 2 and 5. |
| 8461 | MTA-STS | September 2018, Proposed Standard | "SMTP MTA Strict Transport Security (MTA-STS) is a mechanism enabling mail service providers (SPs) to declare their ability to receive Transport Layer Security (TLS) secure SMTP connections and to specify whether sending SMTP servers should refuse to deliver to MX hosts that do not offer TLS with a trusted server certificate." | Options 2 to 4: lets a receiving server insist that senders encrypt the connection (a "_mta-sts" DNS record plus a policy file). |
| 8314 | TLS for mail submission and access | January 2018, Proposed Standard; updated by RFC 8997 | "This specification outlines current recommendations for the use of Transport Layer Security (TLS) to provide confidentiality of email traffic between a Mail User Agent (MUA) and a Mail Submission Server or Mail Access Server." | Option 5 above all: the app must connect encrypted ("imaps" on "default port 993", "submissions" on "default port 465"). |
| 9580 | OpenPGP | July 2024, Proposed Standard; obsoletes RFC 4880, 5581, 6637 | "This document specifies the message formats used in OpenPGP." | End-to-end encryption inside email itself; one candidate format for the copy sealed on arrival, and how people who already use PGP would write to us. |
| 8551 | S/MIME 4.0 | April 2019 on its title page (the RFC Editor's metadata says May 2019), Proposed Standard; obsoletes RFC 5751; updated by RFC 9788 | "This document defines Secure/Multipurpose Internet Mail Extensions (S/MIME) version 4.0." | The other end-to-end email encryption standard, common in organisations; the same two uses as OpenPGP. |

## Part B. What the big mailbox providers require (summary; the finding has the quotations)

Read from Google's sender guidelines and FAQ, Yahoo's sender best practices, and Microsoft's
announcement and Outlook.com postmaster policies. In short:

- **Every sender** to Gmail and Yahoo (from February 2024): SPF or DKIM, valid forward and reverse DNS
  (PTR), spam complaints below 0.3%; Gmail also requires TLS. Outlook.com's policies ask every server
  for valid reverse DNS and say connections "from dynamic IP space may not be accepted".
- **Bulk senders** (more than 5,000 messages a day to that provider): SPF and DKIM and DMARC (at least
  `p=none`), the From domain aligned with SPF or DKIM, and one-click unsubscribe for marketing and
  subscribed mail (Yahoo: honoured within 2 days). Microsoft enforces this from 5 May 2025 and rejects
  failures with error "550; 5.7.515".
- Gmail ramped up enforcement in November 2025 ("temporary and permanent rejections").
- A server that only **receives** (option 2) is not bound by these rules, and option 5 sends through
  the person's own provider, which carries them.

## Part C. Blocklists and a new server's address (Spamhaus)

| Source (URL, date) | Quotation | What it means |
|---|---|---|
| Spamhaus, Policy Blocklist, https://www.spamhaus.org/blocklists/policy-blocklist/ (undated page) | "The Policy Blocklist is a dataset containing end-user IP address ranges from which email should never be sent directly to the final destination." | If a server's address sits in a range its network marks as end-user space, receivers using the PBL refuse its mail outright. |
| Same page | "The IPs in this dataset are not necessarily "bad" - simply, they should never be sending email." | A PBL listing is about the kind of address, not misbehaviour. |
| Same page | "If the Policy for a particular network allows it, end users can have their IP excluded from PBL." | Whether a listed address can be removed is the network owner's (our host's) policy. |
| Spamhaus, Spamhaus Blocklist, https://www.spamhaus.org/blocklists/spamhaus-blocklist/ (undated page) | "The Spamhaus Blocklist contains IP addresses that have been identified as malicious." | The SBL is for abuse. A new address can inherit a bad history from whoever held it before. |
| Same page | "IP addresses are listed on the SBL because they appear to Spamhaus to be under the control of, used by, or made available for use by spammers and abusers in unsolicited bulk email or other types of Internet-based abuse that threatens networks or users." | Listing follows observed abuse, including abuse by others on the same network. |
| Same page, its advice to avoid listing | "Configuration – ensure that your hostname and your HELO match, and that your reverse DNS (PTR record) is defined and pointing to the same hostname." | Matches the providers' reverse DNS rule. |
| Same page | "Use of the Spamhaus DNSBLs is free of charge for low-volume, non-commercial users." | For receiving (options 2 to 4): a small non-commercial server may use Spamhaus to filter incoming spam for free, within Spamhaus's usage criteria. |

Not checked: whether either of our two servers' addresses is listed today. That is a lookup on
`check.spamhaus.org`, which the PBL page names; it was not run for this research.

## Part D. Our two hosts' terms (summary; the finding has the quotations)

- **DreamHost** (public.guide): its Acceptable Use Policy (last updated 26 August 2026) strictly
  prohibits spam in any form and spoofing "email or network packet headers, whether or not it is done
  for malicious purposes"; its Anti-Spam Policy requires confirmed opt-in for mailing lists and says
  its mail equipment is in California and bound by California's anti-spam law. Its VPS pages
  (September and November 2026) describe full root access and no included email, and say nothing about
  port 25. No DreamHost page read answers whether a VPS may send on port 25.
- **Namecheap** (united-humanity.us): every page refused automated reading (a "Just a moment..."
  challenge, HTTP 403), so neither its Acceptable Use Policy nor any VPS port 25 rule was read.

## Part E. The law (summary; the finding has the quotations)

- **CAN-SPAM** (15 U.S.C. 7701 and after) governs "commercial electronic mail" and the person who
  "initiates" it; "initiate" excludes "routine conveyance", the automatic relaying of mail whose
  recipients someone else chose. On its words, a server that only carries its people's personal mail
  is not the initiator. It would bind the project's OWN commercial mail (none planned).
- **Stored Communications Act**: a mail bridge fits the words "electronic communication service".
  If it is "to the public", 18 U.S.C. 2702 forbids knowingly divulging stored contents except as
  listed, and 2703 sets what the government needs to compel them (a warrant for mail stored 180 days
  or less) and a 90-day preservation duty on request.
- **18 U.S.C. 2258A**: a "provider" must report apparent child sexual abuse material to NCMEC's
  CyberTipline on actual knowledge, with no duty to monitor or scan.
- **47 U.S.C. 230(c)**: a service is not treated as the publisher of others' content and is protected
  when it blocks in good faith; it does not touch federal criminal law or the privacy statutes.
- **Washington, RCW 19.255** (read through the Attorney General's pages; the statute's own site
  barred automated reading): notify affected residents within 30 days of a breach, and the Attorney
  General when more than 500 Washington residents are affected. "Personal information" includes a
  username or email address with a password, and private keys for electronic signature.
- **EU GDPR**: reaches a controller outside the EU that offers services to people in the EU (Article
  3(2)); Article 32 asks for security appropriate to the risk. EUR-Lex barred automated reading today.

## Part F. Software that could build or run it

Fit rule (from the brief): HumanityOS is CC0-1.0 and ships as ONE program, so GPL or AGPL code fits
only as a separate program running beside it, never linked in; permissively licensed crates (MIT,
Apache-2.0) can live inside it.

Sources: `gh api repos/OWNER/REPO` (SPDX field, description, last push) and the licence file's first
lines, read 14 November 2026; crates through `https://crates.io/api/v1/crates/NAME` the same day.

### Mail servers and filters (separate programs)

| Project (repository) | What it is (its own description) | Licence, quoted | Fit |
|---|---|---|---|
| Stalwart (stalwartlabs/stalwart; last push 10 Nov 2026; Rust) | "All-in-one Mail & Collaboration server. Secure, scalable and fluent in every protocol (IMAP, JMAP, SMTP, CalDAV, CardDAV, WebDAV)." | **Dual-licensed.** README: "This project is dual-licensed under the **GNU Affero General Public License v3.0** (AGPL-3.0; as published by the Free Software Foundation) and the **Stalwart Enterprise License v2 (SELv2)**". The SELv2 file: "*Last Update: March 29, 2026*" and "THIS AGREEMENT CONSTITUTES A LEGALLY BINDING AGREEMENT BETWEEN YOU AND STALWART LABS LLC". The README says "About 85% of Stalwart's features are free and open source under the AGPL-3.0." GitHub's SPDX field reads "none" (several licences in a LICENSES folder). | Separate program only. **Its default build includes the enterprise code**: `crates/main/Cargo.toml` has `default = ["rocks", "enterprise", "keep"]`. A build that must stay AGPL-only would leave out the `enterprise` feature; which features that removes is on Stalwart's comparison page (not read). |
| Maddy (foxcpp/maddy; last push 12 Nov 2026; Go) | "Composable all-in-one mail server." | GPL-3.0: "GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007" | Separate program only. |
| Mox (mjl-/mox; last push 27 Oct 2026; Go) | "modern full-featured open source secure mail server for low-maintenance self-hosted email" | MIT: "Copyright 2021 Mechiel Lukkien" and "Permission is hereby granted, free of charge, to any person obtaining a copy of this software" | Any use; written in Go, so in practice a separate program. |
| Postfix (vdukhovni/postfix, a Postfix developer's repository; postfix.org itself not read; C) | "Postfix MTA by Wietse Venema" (sends and receives on SMTP only; no mailbox reading) | "This software is dual-licensed under both the Eclipse Public License version 2.0 and the IBM Public License version 1.0" | Separate program. |
| Dovecot (dovecot/core; last push 14 Nov 2026; C) | "Dovecot mail server" (an IMAP server; it does not receive from the internet by itself) | Mixed: "Everything in src/lib/, src/auth/, and src/lib-sql/ is under MIT license" and "Everything else is LGPLv2.1 (see COPYING.LGPL) unless otherwise mentioned at the beginning of the file." | Separate program. |
| Rspamd (rspamd/rspamd; last push 13 Nov 2026; C) | "Rapid spam filtering system." | Apache-2.0: "Apache License" "Version 2.0, January 2004" | Separate program (a spam filter beside a mail server). |
| mailcow (mailcow/mailcow-dockerized; last push 12 Nov 2026) | "mailcow: dockerized" (a ready-made mail suite run in Docker containers) | GPL-3.0: "GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007" | Separate program; its parts carry their own licences (not read). |

### Rust crates (could live inside the one program)

| Crate (repository) | What it is | Licence (crates.io, newest version) | Fit |
|---|---|---|---|
| lettre 0.11.23 (lettre/lettre; 3 Aug 2026) | "a mailer library for Rust" (sends mail over SMTP) | MIT | Inside the app or a relay: option 3 and 4 sending. |
| mail-parser 1.0.1 (stalwartlabs/mail-parser; 2 Nov 2026) | "Fast and robust e-mail parsing library for Rust" | Apache-2.0 OR MIT | Inside: read any incoming message (options 2 to 5). |
| mail-auth 0.15.1 on crates.io; 0.16.0 in its repository (stalwartlabs/mail-auth; 30 Oct 2026) | "DKIM, SPF and DMARC library for Rust" | Apache-2.0 OR MIT | Inside: check incoming mail, sign outgoing. |
| mail-builder 2.0.1 (stalwartlabs/mail-builder; 2 Nov 2026) | "E-mail builder library for Rust" | Apache-2.0 OR MIT | Inside: compose messages (options 3 to 5). |
| mail-send 0.7.0 (stalwartlabs/mail-send; 1 Nov 2026) | "E-mail delivery library for Rust with DKIM support" | Apache-2.0 OR MIT | Inside: send with DKIM. |
| async-imap 0.12.0 (crates.io names async-email/async-imap; GitHub now serves chatmail/async-imap; 30 Sep 2026) | "Async IMAP implementation in Rust" | MIT OR Apache-2.0 (the repository holds LICENSE-APACHE and LICENSE-MIT) | Inside: option 5, the app as an IMAP client. |

Note: the four `mail-*` crates come from Stalwart Labs, but unlike the Stalwart server they are
permissively licensed (their `Cargo.toml`: `license = "Apache-2.0 OR MIT"`).

## Part G. A published cost, as an example only

Not a recommendation: one hosted sending service's own price list, to show the order of magnitude.

Amazon SES pricing, https://aws.amazon.com/ses/pricing/ (undated page):
- "Outbound email" "$0.10 / 1,000 emails" (the à la carte price), plus "$0.12 per GB of attachment
  data sent".
- "Inbound email" "$0.10 / 1,000 emails" plus "$0.09 for every 1,000 incoming email chunks" (a chunk
  is "256 kilobytes (KB) of incoming data").
- "Dedicated IPs – Standard" "$24.95 / month / IP".
- Plans: "0 – 10M emails" a month at "$0.16 per 1,000 emails" (Essentials), and the page says "New SES
  accounts and account x region combinations with no metered SES activity since June 1, 2025 will
  start on the Essentials plan beginning July 21, 2026. You can upgrade or switch to à-la-carte pricing
  at any time."

Worked example from those figures (arithmetic, not a quote): 1,000 people each receiving 30 messages
a month is 30,000 inbound messages, about $3 a month at the à la carte inbound price before chunk
charges.

## Robots.txt read

- www.rfc-editor.org: one group for every reader; bars only /rfc/authors/, /rfc/rerendered/, /rfc/v3test/.
- support.google.com: one group for every reader; bars search, API and forum-attachment paths. The
  /a/answer/ pages redirected to /mail/answer/ on the same host, which is allowed.
- senders.yahooinc.com: no robots.txt (HTTP 404); nothing barred.
- techcommunity.microsoft.com: one group for every reader; bars user, settings and help paths; /blog/ allowed.
- sendersupport.olc.protection.outlook.com: no robots.txt (it serves an HTML page); its policies page
  redirected to substrate.office.com, whose robots.txt request returned HTTP 417 (none served). curl
  followed that redirect before the robots check could be made; recorded here.
- learn.microsoft.com: read; bars question-and-answer user paths only; no page used.
- www.spamhaus.org: "Allow: /" for every reader.
- www.namecheap.com: one group for every reader, /legal/ not barred; but every request got a
  challenge page (HTTP 403), so nothing was read.
- www.dreamhost.com: bars /legal/terms-of-service/unstyled/ and some tool pages; /legal/ pages allowed.
- help.dreamhost.com: allows all but /admin, /api and search-suggest files.
- www.ftc.gov: curl received a "Page Not Found" page; WebFetch read it: one group for every reader,
  "Crawl-delay: 5", bars /core/, /profiles/, /admin/, /search/ and login pages; /business-guidance/ allowed.
- uscode.house.gov: bars only the reader "Slurp"; but the site is "currently operating in a temporary
  format" and every section page returned 404, so the US Code was read from govinfo.gov.
- www.govinfo.gov: bars system paths and /app/search/*; /content/pkg/ allowed.
- www.law.cornell.edu: read ("Crawl-delay: 10"); no page used.
- app.leg.wa.gov: "User-agent: *" "Disallow: /": **barred**; the RCW was not read there.
- www.atg.wa.gov: bars system paths only.
- eur-lex.europa.eu: "User-agent: *" "Disallow: /": **barred**; the GDPR was not read there today.
- commission.europa.eu: bars system and /search/ paths; the page tried answered HTTP 429 (too many requests).
- www.edpb.europa.eu: bars system paths only.
- crates.io: "allow crawling everything by default".
- api.github.com: no robots.txt (HTTP 404); used through `gh api`.
- stalw.art: allows every reader and names ClaudeBot and Claude-User as allowed; no page read.
- aws.amazon.com: bars old news, tag and sign-up pages; /ses/pricing/ allowed.
- lawfilesext.leg.wa.gov and leg.wa.gov: no robots.txt; no page read.

## Could not read

- https://www.namecheap.com/legal/ (and so Namecheap's Acceptable Use Policy and any knowledge-base
  article on VPS port 25): a challenge page, "Just a moment... Enable JavaScript and cookies to
  continue", HTTP 403 to curl and to WebFetch. Hoped for: whether a Namecheap VPS may run a mail
  server and send on port 25, and its spam rules.
- DreamHost's rule on port 25 for a self-managed VPS: not found. The help centre's search is not
  served as readable text, and its API is barred by robots.txt. Hoped for: whether outbound port 25 is
  open, limited, or opened on request.
- https://app.leg.wa.gov/RCW/default.aspx?cite=19.255.010 (and RCW 19.255.005, the definitions, and
  RCW chapter 19.190): barred by robots.txt. Hoped for: the statute's exact words.
- https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32016R0679 (GDPR Articles 3 and 32):
  barred by robots.txt. Article 3 is quoted from our 2026-11-07 finding, read when EUR-Lex allowed it.
- https://commission.europa.eu/law/law-topic/data-protection/rules-business-and-organisations/obligations_en:
  HTTP 429. Hoped for: the Commission's own summary of security duties.
- https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title15-section7702&num=0&edition=prelim
  (and the other sections there): 404 in the site's temporary format. Read from govinfo's 2024
  edition instead, so amendments after that edition were not checked.
- https://www.postfix.org/ : not read; Postfix's licence was read from a Postfix developer's
  GitHub repository.
- Stalwart's comparison page (https://stalw.art/compare/): not read; it lists which features are
  enterprise-only.
- Spamhaus listings for our two servers' addresses (check.spamhaus.org): not looked up.
