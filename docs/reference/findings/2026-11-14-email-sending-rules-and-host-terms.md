# Email: what the big mailbox providers and our two server hosts require

**Research date: 14 November 2026 (Pacific).** Every rule below was read that day from the
provider's or host's own page, never from memory. **This is not legal advice**, and these are terms
and technical rules, not law: it is an AI's reading of each page, written down with the words that
carry the meaning so a person can check them. The research notes, with the standards, blocklists,
software and costs, are in
[`../research/2026-11-14-microsoft-services/email-bridge.md`](../research/2026-11-14-microsoft-services/email-bridge.md).
The law is in the companion finding
[`2026-11-14-running-a-mail-service-law.md`](2026-11-14-running-a-mail-service-law.md).

Method: `curl -A "Claude-User (research tool)"`, page text extracted by script, quotations checked
against it; robots.txt read first for each host, in a step of its own. Quotations are exact (curly
quotes normalised to straight).

## The question

What do the big mailbox providers (Gmail, Yahoo, Outlook.com) and our two server hosts (Namecheap for
united-humanity.us, DreamHost for public.guide) require before HumanityOS servers may send and receive
standard email?

## Short answer

- **Receiving only** (an address that accepts mail and delivers it into the app): none of the three
  providers sets rules for receiving their mail. What matters is our side: a server reachable on port
  25, an MX record, and spam filtering.
- **Sending** to Gmail, Yahoo and Outlook.com, from **every** sender: SPF or DKIM on the sending
  domain, a valid reverse DNS (PTR) record for the server's address, an encrypted connection (Gmail),
  and spam complaints below 0.3% (Gmail and Yahoo). Outlook.com adds that connections "from dynamic
  IP space may not be accepted" and that servers must follow the internet mail standards.
- **Bulk senders** (about 5,000 or more messages a day to that provider's users) must also have SPF
  AND DKIM, a DMARC policy (at least `p=none`), the From domain aligned with SPF or DKIM, and one-click
  unsubscribe for marketing and subscribed mail. **Gmail counts every message from the same primary
  domain together**, and the status is permanent once reached: if all HumanityOS people sent from
  one shared domain, their personal mail together could make the project a bulk sender.
- **Enforcement is live**: Gmail and Yahoo from February 2024 (Gmail ramped up "temporary and
  permanent rejections" from November 2025); Outlook.com from 5 May 2025, with error "550; 5.7.515".
- **DreamHost** forbids spam and header spoofing outright (policy of 26 August 2026), asks confirmed
  opt-in for any list, and its pages read say nothing about port 25 on a self-managed VPS.
- **Namecheap** refused automated reading of every page, so its terms are **unknown**, including
  whether its VPS may send on port 25. That is the first thing to settle before choosing a host for
  sending.
- **Spamhaus** lists end-user address ranges that "should never be sending email" (the PBL) and
  addresses used for abuse (the SBL); a new server's address can arrive already listed.

## The findings

### 1. Google (Gmail): requirements for every sender

- Source: Google, "Email sender guidelines", https://support.google.com/mail/answer/81126?hl=en
  (the /a/answer/81126 address redirects here; undated page, footer "©2026 Google").
- "Starting February 1, 2024, all email senders who send email to Gmail accounts must meet the
  requirements in this section."
- The requirements, each a line of the page: "Set up SPF or DKIM email authentication for your
  sending domains."; "Ensure that sending domains or IPs have valid forward and reverse DNS records,
  also referred to as PTR records."; "Use a TLS connection for transmitting email."; "Keep spam rates
  reported in Postmaster Tools below 0.3%."
- Reverse DNS in detail: "The sending IP address must match the IP address of the hostname specified
  in the Pointer (PTR) record."
- DKIM key size: "Sending to personal Gmail accounts requires a DKIM key of 1024 bits or longer."
- The page applies to personal accounts: "A personal Gmail account is an account that ends in
  @gmail.com or @googlemail.com."

### 2. Google (Gmail): requirements for bulk senders

- Same page: "Starting February 1, 2024, email senders who send more than 5,000 messages per day to
  Gmail accounts must meet the requirements in this section."
- "Set up SPF and DKIM email authentication for your domain."; "Set up DMARC email authentication
  for your sending domain. Your DMARC enforcement policy can be set to none."; "Keep spam rates
  reported in Postmaster Tools below 0.30%."
- Alignment: "For direct email, the domain in the sender's From: header must be aligned with either
  the SPF domain or the DKIM domain."
- Unsubscribe: "Marketing messages and subscribed messages must support one-click unsubscribe, and
  include a clearly visible unsubscribe link in the message body."
- Its best-practice section asks for less: "Keep spam rates reported in Postmaster Tools below 0.10%
  and avoid ever reaching a spam rate of 0.30% or higher."

### 3. Google (Gmail): who counts as bulk, and enforcement

- Source: Google, "Email sender guidelines FAQ", https://support.google.com/mail/answer/14229414?hl=en
  (undated page; it says "We update this FAQ periodically").
- "A bulk sender is any email sender that sends close to 5,000 messages or more to personal Gmail
  accounts within a 24-hour period. Messages sent from the same primary domain count toward the
  5,000 limit."
- "Senders who meet the above criteria at least once are permanently considered bulk senders."
- "Starting November 2025, Gmail is ramping up its enforcement on non-compliant traffic. Messages
  that fail to meet the email sender requirements will experience disruptions, including temporary
  and permanent rejections."
- "The Email sender guidelines don't apply to messages sent to Google Workspace accounts."

### 4. Yahoo: requirements for every sender and for bulk senders

- Source: Yahoo, "Sender Best Practices", https://senders.yahooinc.com/best-practices/ (undated page).
- "Beginning in February 2024, enforcement of the following sending standards will take effect."
- All senders: "Implement SPF or DKIM at a minimum"; "Keep your spam rate below 0.3%"; "Have a valid
  forward and reverse DNS record for your sending IPs".
- Bulk senders: "Implement both SPF & DKIM"; "Publish a valid DMARC policy with at least p=none";
  "Ensure the domain in the From: header is aligned with either the SPF domain or the DKIM domain.";
  "Implement a functioning list-unsubscribe header, which supports one-click unsubscribe for
  marketing and subscribed messages"; "Honor unsubscribes within 2 days".
- Standards: "All email must be compliant with RFC 5321 and RFC 5322."
- Reverse DNS: "Do not use a reverse DNS that looks like a dynamically-assigned IP instead of a
  static mail server."
- Yahoo's page gives no number of messages that makes a "bulk sender".

### 5. Microsoft (Outlook.com): high-volume senders

- Source: Microsoft Tech Community, post titled Strengthening Email Ecosystem: Outlook's New
  Requirements for High-Volume Senders,
  https://techcommunity.microsoft.com/blog/microsoftdefenderforoffice365blog/strengthening-email-ecosystem-outlook%E2%80%99s-new-requirements-for-high%E2%80%90volume-senders/4399730
  (posted Apr 02, 2025; "Updated Apr 30, 2025").
- "For domains sending over 5,000 emails per day, Outlook will soon require compliance with SPF,
  DKIM, DMARC."
- SPF: "Must Pass for the sending domain." DKIM: "Must Pass to validate email integrity and
  authenticity." DMARC: "At least p=none and align with either SPF or DKIM (preferably both)."
- **The post disagrees with itself on what happens to failures.** Its "April 29th Update" says: "we
  have made a decision to reject messages that don't pass the required authentication requirements
  detailed above" with "550; 5.7.515 Access denied, sending domain [SendingDomain] does not meet the
  required authentication level." and "This change will state taking effect on May 5th as originally
  stated." The original text below it still says: "After May 5th, 2025, Outlook will begin routing
  messages from high volume non‐compliant domains to the Junk folder" with rejection "in the future
  (date to be announced)". Finding 6 (the policies page) sides with Junk first, then rejection.
- Below 5,000 a day: "While enforcement first targets large senders, all senders benefit from these
  best practices."
- Forwarding: "Forwarding can break DMARC alignment. ARC preserves the original authentication
  checks, preventing legitimate forwarded mail from being wrongfully flagged."

### 6. Microsoft (Outlook.com): rules for every server that connects

- Source: Outlook.com Postmaster, "Policies",
  https://substrate.office.com/ip-domain-management-snds/Postmaster/Policies (reached by redirect from
  sendersupport.olc.protection.outlook.com; page footer "2025").
- "Sender is expected to comply with all technical standards for the transmission of Internet email,
  as published by The Internet Society's Internet Engineering Task Force (IETF), including RFC 2821,
  RFC 2822, and others." (RFC 2821 and 2822 are the predecessors of RFC 5321 and 5322.)
- "Connections from dynamic IP space may not be accepted."
- "Email servers must have valid reverse DNS records."
- "Messages must not be transmitted through insecure email relay or proxy servers."
- "Sender must not open more than 500 simultaneous connections to Outlook.com inbound email servers
  without making prior arrangements."
- "The mechanism for unsubscribing, either from individual lists or all lists hosted by the sender,
  must be clearly documented and easy for recipients to find and use."
- High volume: "Effective May 5th, 2025, specific authentication requirements are in place for
  high-volume senders: Domains sending more than 5,000 emails per day to Outlook.com accounts must
  fully be compliant with SPF, DKIM, and DMARC." Failures "will be sent to the junk folder. If
  issues remain unresolved, messages may be rejected".
- Microsoft's pages read give no spam-complaint rate.

### 7. Spamhaus: blocklists a new server's address may already be on

- Source: https://www.spamhaus.org/blocklists/policy-blocklist/ (undated page): "The Policy Blocklist
  is a dataset containing end-user IP address ranges from which email should never be sent directly
  to the final destination." and "If the Policy for a particular network allows it, end users can have
  their IP excluded from PBL."
- Source: https://www.spamhaus.org/blocklists/spamhaus-blocklist/ (undated page): "The Spamhaus
  Blocklist contains IP addresses that have been identified as malicious." Its advice: "ensure that
  your hostname and your HELO match, and that your reverse DNS (PTR record) is defined and pointing to
  the same hostname."

### 8. DreamHost (public.guide): Acceptable Use Policy

- Source: https://www.dreamhost.com/legal/acceptable-use-policy/ ("Last Updated: August 26, 2026").
- "Spamming in any form is strictly prohibited. Customer agrees to not utilize, distribute, or in any
  way make available tools or methods to send unsolicited e-mail or usenet postings (spam), or to use
  DreamHost infrastructure for relaying unsolicited e-mail or usenet postings".
- "Use of DreamHost infrastructure to impersonate another person or entity, be it through Email,
  Internet Forums, or any other means, is strictly prohibited. This includes spoofing email or network
  packet headers, whether or not it is done for malicious purposes."
- The rule that servers be used "primarily for the purpose of hosting a website and associated email
  functions" begins "With the exception of Hosting plans with defined resource allocations at signup,
  including but not limited to Web Hosting (Launch, Growth, Scale), Shared Starter, DreamPress, VPS,
  Managed VPS, and Dedicated Servers", so on its words it does not limit a VPS.

### 9. DreamHost: Anti-Spam Policy

- Source: https://www.dreamhost.com/legal/anti-spam-policy/ (undated page).
- "Spam is strictly against our Terms of Service, and will result in the immediate disablement
  without refund of any customers who are found to have used spam in conjunction with their DreamHost
  account."
- "Mailing list subscribers must specifically opt-into the list they are subscribed to." and
  "Mailing list subscriber information must include the date and time that the subscription was
  confirmed, as well as the IP address of the subscriber at the time of the confirmation."
- "All mailings must provide clear and simple to follow instructions for opting-out of future
  mailings".
- "The user is advised and acknowledges that DreamHost's electronic mail service equipment is located
  in California. In no event shall the user use DreamHost's equipment in violation of California's
  anti-spam laws including California Business & Professions Code sections 17538.4 and 17538.45."
  (This names DreamHost's own mail equipment; whether it reaches a self-run server on a VPS is not
  said.)
- "DreamHost reserves the right to disable any account at any time should it feel there is a
  reasonable suspicion that it is being used in conjunction with the practice of spamming."

### 10. DreamHost: the self-managed VPS, and its mail limits

- Source: "Migrating between a VPS and a managed hosting plan",
  https://help.dreamhost.com/hc/en-us/articles/17876957977515-migrating-between-a-vps-and-a-managed-hosting-plan
  (modified 27 August 2026): "A VPS gives you full root access to a Linux environment that DreamHost
  does not manage" and "VPS does not include Professional Email."
- Source: "VPS overview", https://help.dreamhost.com/hc/en-us/articles/50476238203028-VPS-overview
  (modified 15 September 2026): "You configure, maintain, and secure the server."
- Source: "SMTP quota limits", https://help.dreamhost.com/hc/en-us/articles/215730437-SMTP-quota-limits
  (modified 29 July 2026): "SMTP quota limits are set for all DreamHost email addresses and cannot be
  adjusted." These limits (100 recipients an hour by email client) are for DreamHost's own hosted
  email; a server we run ourselves on the VPS is not a DreamHost email address.
- None of the DreamHost pages read mentions port 25 for a VPS.

### 11. Namecheap (united-humanity.us): not read

- https://www.namecheap.com/legal/ answered every request with "Just a moment... Enable JavaScript
  and cookies to continue" (HTTP 403), to curl and to WebFetch. Its robots.txt does not bar /legal/;
  the refusal is a bot challenge, which this research does not work around. So nothing is known from
  Namecheap's own words about mail, spam or port 25.

## What is still unknown, and what would settle it

- **Namecheap's terms and its port 25 rule.** Settle: the operator opens Namecheap's Acceptable Use
  Policy in his browser and searches for "mail", "SMTP" and "port 25" (free, about ten minutes), or
  asks Namecheap's support chat whether outbound port 25 is open on his VPS plan.
- **DreamHost's port 25 rule for a self-managed VPS.** Settle: ask DreamHost support (free), or, with
  the operator's approval, a one-line connection test from each server to a receiving mail server on
  port 25 (it opens a connection and sends nothing; free).
- **Whether each host lets us set the reverse DNS (PTR) record** for our server's address, which every
  provider requires. Settle: each host's control panel or support (free).
- **Whether our two addresses are already on a Spamhaus list.** Settle: a lookup at
  check.spamhaus.org, which the PBL page names (free).
- **When Outlook.com rejects rather than junks** (finding 5 disagrees with itself). Settle: a test
  message after setup, or Microsoft's later post (free).
- **Yahoo's bulk threshold** is not stated on the page read. Settle: Yahoo's sender FAQ (not read).
- **The providers' rules for forwarded mail** (Gmail has a separate forwarding guidance page, not
  read). Only matters if a HumanityOS server ever forwards mail onward.

## Sources not reached

- https://www.namecheap.com/legal/ and every other Namecheap page: bot challenge, HTTP 403. Hoped
  for: the Acceptable Use Policy and any VPS mail or port 25 rule.
- DreamHost's port 25 rule for a VPS: not found; the help centre's search is not readable text and its
  API is barred by robots.txt.
- Yahoo's sender FAQ: not read. Hoped for: the bulk threshold.
- https://sendersupport.olc.protection.outlook.com/pm/policies.aspx redirected to substrate.office.com,
  whose robots.txt request returned HTTP 417; the redirect was followed automatically before that
  check. Recorded here; the page itself is Microsoft's own.

## What it means for this project

**Certain (from the words quoted):**
- Receiving mail from Gmail, Yahoo and Outlook.com users needs no permission from those providers.
- Any HumanityOS server that sends to them must have SPF or DKIM, valid forward and reverse DNS, and
  (for Gmail) encrypted connections, and must keep complaints under 0.3%; beyond about 5,000 a day to
  one provider, SPF and DKIM and DMARC and alignment too, and one-click unsubscribe on marketing or
  subscribed mail.
- Gmail adds up all mail from one primary domain, and a domain once bulk stays bulk.
- DreamHost forbids spam and header spoofing on any of its services, and can disable an account on
  reasonable suspicion of spam.
- Option 5 (the app as a client of the person's existing mailbox) sends through the person's own
  provider, so none of these sender rules falls on HumanityOS's servers.

**Judgement calls (an AI's reading, for the operator to accept or reject):**
- Receiving only (option 2) is the low-risk first rung: it touches none of the sender rules and
  neither host's spam clauses, provided the server accepts mail only for its own people and never
  relays for anyone else.
- Sending (option 3) from a shared project domain makes every person's reputation everyone's: one
  person's spam complaints count against the domain all share. Option 4 (each person's own domain)
  isolates that, at the cost of each person's DNS setup.
- DreamHost's ban on "spoofing email or network packet headers, whether or not it is done for
  malicious purposes" could be read to reach a server that re-sends a message under its original sender's From address (classic
  forwarding). A bridge that never forwards avoids the question.
- Do not choose Namecheap or DreamHost for sending until the port 25 question is answered in writing
  by the host.

**This is not legal advice.** It records what the providers' and hosts' pages said on 14 November
2026. Terms change without notice; re-read them before relying on them.
