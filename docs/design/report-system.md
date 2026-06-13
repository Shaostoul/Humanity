# Report System Design (v0.188+)

> **Status: design complete, implementation pending.** This document answers the operator's questions about how the chat 🚩 Report button should behave across DM / Group / Server contexts, who receives reports, and what stops the feature from being abused. Implementation is split across v0.189–v0.191 per the phasing at the bottom.

## Three contexts, three handlers

Reports must work cleanly in three distinct contexts because the trust geometry is different in each one.

### 1. Server channel (federated, public)

| | |
|---|---|
| **Recipient** | The reported message's server's MOD + ADMIN tier. Visible in the new "Reports" tab inside the server's settings cog. |
| **What the reporter sees** | "Reported. Mods will review." Status visible later in the user's own profile under "My filed reports". |
| **What the reportee sees** | Nothing at submission time. Only sees a system DM if a mod takes action ("Your message in #general was deleted by Mod {name} for {reason}"). |
| **Available actions** | Dismiss / Warn / Delete message / Mute (15m / 1h / 24h / 7d) / Kick / Ban / **Mark Bogus** (deducts trust from reporter). |

Same flow for groups, a group is just a small private server with its own admin tier.

### 2. DM (1:1, end-to-end encrypted)

DMs have no central authority. Two parallel actions on submit:

| | |
|---|---|
| **Always: local block** | The reportee is added to the reporter's local block list. Future DMs from them are silently dropped at the reporter's client. The reportee sees no error, they'll just notice no replies. |
| **Optionally: forward to a shared-server admin** | Modal asks: "Also notify admins of a server you both belong to?" → dropdown of shared servers. If the reporter picks one, a signed report is sent to that server's mod tier with the DM message snapshot (the reporter must have copy-pasted or quoted the message; relay never sees their DM contents because they're E2E encrypted). |
| **What the reportee sees** | Locally blocked: nothing immediately. Server-forwarded: only if the receiving admin takes action on a shared server. |

The forwarding path is **opt-in per report** so a reporter can quietly block someone without escalating, OR escalate when they want admin action.

### 3. Group (small, private, multi-member)

| | |
|---|---|
| **Recipient** | The group's admin tier. Same UX as a server channel report. |
| **Special case** | If you're reporting a message from a group ADMIN, the report routes to a SECOND-tier admin (group "owner" if defined) OR falls through to the group's federation host server's admins. The same admin who's misbehaving cannot dismiss reports against themselves. |

## Anti-abuse mechanisms

Eight overlapping defenses, ordered from cheapest to most invasive:

### 1. Rate limit per reporter
Max **5 reports per hour** per reporter pubkey. Same Fibonacci-backoff machinery as the existing message rate limiter in `src/relay/relay.rs`. Defeats drive-by spam.

### 2. Same-target cooldown
Max **1 report per 24 hours** per (reporter, target_user) pair. You can't spam-report the same person.

### 3. Self-report rejected
Server-side validation: `reporter_key != target_key`. Trivial check.

### 4. Reporter trust score weighting
Mods see each report alongside the reporter's trust score (already computed in `src/relay/storage/trust_score.rs`). Reports from low-trust accounts (new, no vouches, history of dismissals) are visually de-emphasized but never hidden. Trust score is INPUT to mod judgment, not a gate.

### 5. "Mark Bogus" → trust hit
When a mod reviews a report, decision options are:
- `Dismiss` (no fault), neutral
- `Action taken` (warn/mute/kick/ban), neutral for reporter, action for reportee
- **`Mark Bogus`**, explicit "this report was malicious", deducts -0.05 from reporter's trust score per occurrence, capped at -0.5/year

After a configurable threshold of bogus reports (default: 3 in 30 days), the reporter's report-submit ability is throttled to 1/day for 30 days. After 6, revoked entirely on that server (admin can manually reinstate).

### 6. Adversarial-mod escape valve
Reports against a mod CANNOT be dismissed by that same mod (server-side check on `reviewer_key != target_key`). If the entire mod tier is adversarial:
- Reporter can escalate to the ADMIN tier (separate review queue).
- Worst case: reporter still has the local block (DM context) and can leave the server.
- Federation: signed reports can theoretically be carried across servers; not auto-propagated but visible if other servers query.

### 7. Signed reports + transparent log
Every report is an Ed25519-signed object (canonical form `report_v1\n<reporter_key>\n<target_key>\n<context>\n<reason>\n<timestamp>`). Stored in SQLite with the signature. **Anyone can audit:** the reporter's profile page shows their filed-reports count + dismiss rate; the target's profile page shows reports-filed-against-me count (after a 7-day cooldown for reporter privacy). Public accountability deters frivolous reports because the reporter can be seen.

### 8. Federation handling
A signed report doesn't auto-propagate to other servers. Each server's mods make their own call. A target user banned on one server isn't auto-banned elsewhere. Servers that want to share moderation can opt into a shared-block-list federation extension (post-v1.0).

## UI flow (user-facing)

### When the reporter clicks 🚩 Report

A modal opens with:
1. **Snapshot of the offending message** (verbatim, so it's clear what's being reported).
2. **Reason dropdown** with a fixed list:
   - Harassment
   - Hate speech / discrimination
   - Threats / violence
   - Spam
   - NSFW / off-topic
   - Impersonation
   - Other (requires note)
3. **Optional note** (textarea, max 500 chars).
4. **Context-aware notice + extra control:**
   - **Server channel:** "This will be sent to {server_name} mods for review."
   - **Group:** "This will be sent to {group_name} admins for review."
   - **DM:** Two-line: "**This will block this user locally, you'll stop receiving their DMs.**" + checkbox `[ ] Also notify admins of a shared server` with dropdown of shared servers.
5. **Submit / Cancel.**

### When mod opens "Reports" tab

A spreadsheet-style list (matches the channel/member editor design):

| Reporter | Trust | Target | Reason | When | Message | Decision |
|---|---|---|---|---|---|---|
| @alice (0.62) | bar | @bob | Harassment | 2h ago | "you suck"... | [Dismiss] [Warn] [Mute▾] [Kick] [Ban] [Bogus] |

Click row → expands to show reporter note + full message + context. Click decision button → action applied + mod note required for "Mark Bogus".

### When user opens their own profile

New section under Identity:
- **Reports I've filed** (last 30 days): N total, M dismissed → low M/N reflects well on the user, high M/N is a red flag.
- **Reports filed against me** (after 7-day cooldown, redacted reporter identity): N total, M dismissed, K resulted in action.

This is the public accountability layer, both directions visible.

## Data model

```sql
CREATE TABLE reports (
    id INTEGER PRIMARY KEY,
    reporter_key TEXT NOT NULL,         -- Ed25519 pubkey hex
    target_key TEXT NOT NULL,
    target_message_id INTEGER,          -- nullable; null = user-level report
    target_message_content TEXT,        -- snapshot at report time
    target_message_channel TEXT,
    context TEXT NOT NULL,              -- 'dm' | 'group:<id>' | 'server:<channel>'
    reason TEXT NOT NULL,               -- enum string from reason list
    note TEXT,                          -- optional reporter note (max 500)
    timestamp INTEGER NOT NULL,         -- millis
    signature TEXT NOT NULL,            -- Ed25519 sig over canonical form

    state TEXT DEFAULT 'pending',       -- 'pending' | 'dismissed' | 'actioned' | 'bogus'
    reviewer_key TEXT,                  -- mod who reviewed
    reviewed_at INTEGER,                -- millis
    decision TEXT,                      -- 'no_action' | 'warn' | 'delete' | 'mute_*' | 'kick' | 'ban' | 'bogus'
    decision_note TEXT
);

CREATE INDEX reports_target ON reports(target_key);
CREATE INDEX reports_reporter ON reports(reporter_key);
CREATE INDEX reports_state ON reports(state);
```

## WebSocket message types

| Direction | Type | Payload |
|---|---|---|
| client → relay | `report_submit` | `{target_key, target_message_id?, target_message_content, context, reason, note?, timestamp, signature}` |
| client → relay | `report_list` | `{scope: "channel"|"group"|"server", scope_id, state_filter?}` (admin/mod only) |
| client → relay | `report_decide` | `{report_id, decision, decision_note?}` (admin/mod only) |
| client → relay | `report_my_filed` | `{}` (returns reports the calling user filed in last 30d) |
| client → relay | `report_against_me` | `{}` (returns reports filed against caller, redacted, after 7d cooldown) |
| relay → client | `report_submitted_ack` | `{report_id, status}` |
| relay → client | `report_list_response` | `{reports: [...]}` |

## Phasing

| Release | Scope |
|---|---|
| **v0.189.0** | Storage table + WebSocket handlers (`report_submit` only). Client modal triggered from chat 🚩 Report → submits + closes. DM context auto-blocks locally. No mod review surface yet, reports just accumulate in SQLite. |
| **v0.190.0** | Mod review tab in server settings. Decision buttons + enforcement (warn/mute/kick/ban via existing slash-command machinery). "Mark Bogus" → trust score hit. Adversarial-mod escape valve (reports against a mod hide their dismiss button). |
| **v0.191.0** | Self-transparency: profile page sections for "reports I filed" and "reports against me" (redacted). Federation propagation opt-in. |
| **v0.192.0+** | Cross-server shared-block-list federation extension (post-v1.0 candidate). |

## Why the per-context split matters

The simplest possible design, "all reports go to a central HumanityOS moderation team", is exactly what we don't want. It puts a single authority on top of a federated system. The per-context split keeps moderation **scoped to where the moderation authority actually lives**: server admins moderate their server, group admins moderate their group, and DMs are governed by the participants themselves with optional escalation to a shared trust context.

This matches the rest of the project's federation philosophy: no central authority, signed objects, transparent logs, and user-controlled blocks as the always-available baseline.
