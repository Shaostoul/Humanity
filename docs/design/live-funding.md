# Live funding: what donations bring in against what the project costs

> The operator, 2026-11-22: "As far as I'm aware the Patreon, CashApp, and GitHub Sponsors are live
> and waiting for donations. Patreon is the only one with $20/month. Can we update the values live?
> So people can see what current funding is at least via other things not my second job?"

The Donate page already shows a server's funding GOAL and its addresses (`funding` in
`/api/server-info`, from `data/server-config.json`; see `docs/design/funding.md`). It shows no
money coming in, because nothing tracked it. This adds the figures: what each way of giving brings
in each month, what running the project costs each month, and one sentence putting them side by
side. It is never anyone's job or personal income: the page says so in words
(`donate.figures_note`).

## The whole plan, and the rung built now

| Rung | What | State |
|------|------|-------|
| 1 | The owner enters the figures by hand in Server Settings; the relay stores, stamps and serves them; the Donate page of both apps shows them. | Built (2026-11-22) |
| 2 | Automatic figures from GitHub Sponsors and Patreon. CashApp stays manual. | Design only, below |

### Rung 1

**Data shape (infinite-of-x: rows are data, no list in code).** A server's `FundingFigures`
(`src/relay/storage/funding_figures.rs`) is two lists:

- `channels`: `{ name, link, monthly_usd, as_of }`, one per way people give (Patreon, GitHub
  Sponsors, CashApp...).
- `costs`: `{ name, monthly_usd, as_of }`, one per monthly cost (the servers, the domain...).

The goal stays where it was (`funding.goal_usd` in `data/server-config.json`). `docs/outreach/
for_funders_and_sponsors.md` ("The books, itemized") holds the published cost figures; the
operator enters the live ones, and the two are kept consistent by him, since the document is dated
August 2026 and these are dated to the day.

**Shape validation (the relay, `FundingFigures::checked`).** At most 32 channels and 32 costs; a name
of 1 to 80 characters; a link empty or `https://` and at most 300 characters; an amount from 0 to
1,000,000 dollars (finite). Any bad row refuses the WHOLE update, the sender is told why, and the
saved figures stay as they were. The reader (`from_funding_json` native, `read` in
`web/pages/donate-figures.js`) holds a served response to the same bounds again, so a rogue server
cannot make the page draw a thousand rows or a link that is not https.

**The as-of date is the relay's.** A row is stamped with today's date (UTC) when it is new, when its
amount changed, or when its date arrives blank (the editor's "Still right today" button blanks it);
an unchanged row keeps its date. A date an owner types for a changed row is not believed.

**Storage and serving.** One JSON text column of the `server_settings` row, `funding_figures`
(empty = publishes nothing, the shipped default), added to a live database by a guarded ALTER
(`src/relay/storage/mod.rs`). `/api/server-info` serves them inside the existing `funding` object as
`channels` and `costs` (`funding_figures::served`); a server that publishes none serves exactly
what it did before, and one that publishes some serves them even when `server-config.json`'s funding
is off. Public by design: it is what a donor wants to see.

**Who may change them, and which path enforces it.** The existing settings update path:
`server_settings_update` (`src/relay/handlers/server_settings_update.rs`) takes an optional
`funding_figures` and, like every other field there, runs only for a sender whose role is `admin` or
`owner` (the relay checks the role in the handler; a member or moderator gets "Only admins can update
server settings."). The data-file route was not used: `data/server-config.json` is read once at boot
and an edit needs a restart, where this path is live and already enforces the role. The update is a
signed chat-socket message like the rest of the settings.

**The Donate page, both apps.** A "This month" card inside "More ways to give": the sentence
("Donations cover $20 of $241 a month."; "Donations cover all $241 a month." when they meet the costs;
"Donations bring in $20 a month." when no costs are published), each channel with its amount, its
date and an Open link, each cost with its amount and date, and the note. Native
`src/gui/pages/donate_figures.rs`, web `web/pages/donate-figures.js`; words by key (`donate.figures_*`
in `data/i18n/en.json`), theme tokens, a hint on the Open button. Nothing is drawn when a server
publishes none, and a reconnect to a server that publishes none clears the previous server's figures
(the money-routing rule of `apply_server_funding`).

**GUI-first.** Native: Server Settings > ADMIN > Funding figures (`src/gui/pages/funding_admin.rs`):
one row per channel and per cost (name, link, amount, its date, "Still right today", Remove), Add
buttons, and Save, which sends the one update. Web twin: the chat's Game Admin window has a Funding
figures section (`web/chat/chat-funding-admin.js`), the same place the gameplay rules were mirrored,
because the website has no Server Settings page of its own (the chat's admin windows are its
twin). `data/admin/ops_registry.json` lists the action so an AI can enumerate it.

**Where the operator enters the live figures.** United-humanity.us ships publishing none (the
operator's to enter): after the release, in the app connected to united-humanity.us as its owner,
Server Settings > ADMIN > Funding figures, then Add a way people give (Patreon, 20; GitHub Sponsors,
0; CashApp, 0, each with its link) and Add a cost (his own choice of lines: the servers, the domain,
the tools), then Save. The website's equivalent is the chat's Game Admin window.

### Rung 2 (design only, not built)

- **GitHub Sponsors.** Believed to be readable through GitHub's GraphQL API with a token the owner
  creates (the exact fields and scopes are to be confirmed from GitHub's own documentation in the
  dated finding below, not taken from memory). One poll a day by the relay, writing the channel's
  `monthly_usd` and `as_of` the way the owner's edit does.
- **Patreon.** Believed to be readable through Patreon's API with a creator access token (again
  confirmed from Patreon's documentation first). Same daily poll, same write.
- **CashApp.** No public API: stays a manual row.
- **Secrets.** The tokens are the owner's to add and stay on the server (`data/` files outside git,
  like `data/backup.key`), never in `server-config.json`, never in an API response, never read or
  printed by an agent. A channel row would gain `source: "manual" | "github-sponsors" | "patreon"`;
  an automatic row is not editable by hand, and its as-of date is the poll's.
- **Before building it:** read each platform's current terms and API documentation from its own
  pages and write a dated finding (`docs/reference/findings/`), since a platform's rules decide
  what may be fetched and displayed; and compare open alternatives for the poller. Not done now.
