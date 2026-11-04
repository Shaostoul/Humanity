# Per-person controls: your settings for one person

**Status: design 2026-11-03; rung 1 built the same day (section 6).** Sections 2 and 3 list every
control; each says whether it existed before this design, what rung 1 built, or which later rung
builds it.

The operator's request, 2026-11-03, verbatim, with a screenshot of another person's profile card
in the desktop chat (avatar, a verified badge, Offline, Friends, the key, Send DM, Call, Unfollow,
Report, Block, then the Moderation and Admin buttons):

> "We need to add the various controls users can use on other users to the profile page. Like we
> need to set file sharing limits, allowed to call during quiet hours, etc."

The card in his screenshot also had no name: a "?" avatar and an empty line beside the badge
(section 7, fixed with rung 1).

**Why a document of its own.** The choices a person makes about one other person already lived in
four designs: who can reach me and the friend's ticks (`blocking-and-safe-mode.md` 10c, 10c-ii,
10n), the always-allow list and the conversation mute (`notifications.md` sections 5 and 11), the
files a person takes (`files-showcase.md`), and how calls connect (`blocking-and-safe-mode.md` 10p).
`blocking-and-safe-mode.md` is about protecting a person (it is over 160 KB); this one is about
the card where all of a person's choices about someone meet, so the card's code has one document
to cite and the next control has one place to be added. Each of the four designs keeps its own
rules; this one names them and adds only what is new.

---

## 1. The rules every control follows

1. **An override of a global setting, never a second copy of it.** Each control starts at "Use my
   setting" (or the global's own answer), and changes only what happens for this one person. With
   nothing chosen for anyone, the app behaves exactly as before.
2. **Safe by default** (operator, 2026-10-09: "Security is definitely something we need to get
   right. We must protect the users."). The default of every control is the safe global default
   it overrides. A control that lets more through than the global setting (a file size taken
   without asking from someone who is not a friend) is the person's explicit choice for that one
   person, and the protected setup still comes first: nothing here widens what it holds back.
3. **Enforced on the person's own device**, where the decision is already made: THE notification
   rule (`src/notifications/decide.rs` and its web twin), the file's plan
   (`gui/pages/chat/attach_view.rs`), the file offer's question (`net/file_offer.rs`), the ring
   (`engine/call_relay.rs`). The relay enforces only what it already enforces from a pass (who
   may message, call or trade), never a new list.
4. **No server-side social edge** (CLAUDE.md, "The removed tables stay removed"). A choice about a
   person names them, so it is a social edge: it is kept on the person's devices and travels
   between them as a note to oneself, the way the block list, the choice for each friend and the
   always-allow list do (the two new controls from rung 2 on; until then each stays on the device
   it was set on, and the card says so). No server stores it, and no sender is told about it (a per-person file
   rule is never published in the profile's `accepts_files`; telling Ann "you are the one whose
   files are refused" would single her out, the way 4.7 of the safety design keeps a block
   unannounced).
5. **The card shows them in one plain section,** "Your settings for <name>", below the person's
   primary actions and above Report and Block, kept apart from the Moderation and Admin tools
   (section 4).

## 2. The whole list

Status: **existed** (built before this design, now also on the card), **rung 1** (built with this
design), or the rung that builds it (section 5).

| Control | Choices | Default | Overrides | Enforced where | Travels to my other devices | Status |
|---|---|---|---|---|---|---|
| May message you | tick | on | Who can reach me: Messages, when set to "People I choose" | the relay (the pass's `may`); my device lists a refused DM as a request | the choice note `[[hum:choice:v1]]` (10n) | existed (Safety > People I choose); on the card in rung 1, for friends |
| May call you | tick | off | Who can reach me: Calls | the relay (the ring and its signals) | the choice note | existed; on the card in rung 1 |
| May send you trade requests | tick | on | Who can reach me: Trades | the relay (`trade_request`) | the choice note | existed; on the card in rung 1 |
| Calls and messages come through quiet hours | switch | off | quiet hours and their two exceptions | my device: THE rule's `from_allowed` | the always-allow note `[[hum:quiet-allow:v1]]` | existed (a DM's menu, Settings); on the card in rung 1 |
| Notifications from them | Everything / Only calls, mentions and replies / Nothing | Everything | the synced switches, each room's level | my device: THE rule's `from_level` (both twins, the shared cases) | rung 2: the person note (section 3) | **rung 1** |
| Their conversation muted for a while | 1 hour / 8 hours / until I unmute | not muted | none | my device: THE rule's conversation mute | no (mutes are this device's, `notifications.md` section 7) | existed (a DM's menu); the card shows it with Unmute in rung 1 |
| Files from them | Use my setting / Always ask / Up to a size without asking (N MB) / Never | Use my setting | Files in messages: Accept files from | my device: the file's plan and the offer's question | rung 2: the person note | **rung 1** |
| Pictures from them without a click | Use my setting / Always ask | Use my setting | Show pictures without asking from | my device: the file's plan | rung 2 | rung 3 (until then "Always ask" for Files covers it) |
| What they see of you | a line, not a control | | Profile > Privacy | the relay filters my profile by my pass (friends' fields need one) | the pass | rung 3: the line on the card |
| Visiting your home | the Household card's permit | no permit | none | the relay and my device (permits) | the permit notes | existed (the Household card); the card names it in rung 3 |
| How calls with them connect | Use my setting / Always private | Use my setting | How my calls connect (10p) | my device (the call's plan) | rung 2's note | rung 4, a question for the operator (section 8) |
| A name of your own for them | text | none | the name the server lists | my device (every place their name is drawn) | rung 2's note | rung 4 |
| Their voice in rooms: volume, or silent for me | slider, switch | 100%, heard | none | my device (the voice mix) | no (a device's speakers) | rung 4 |
| A ring of their own | the sounds list | the kind's sound | the per-kind sound | my device | no (a device's speakers) | rung 4 |
| Block, Report | the existing buttons | | | | | existed: on the card, after this section |

**Not on the list, and why.** Hiding my presence from one person: presence reaches people through
the server's member list, so hiding it from one person would need the server to know who, a social
edge. The house answer would be presence told to friends in sealed notes instead of by the server;
that is a design of its own (section 8, question 5). Read receipts and typing marks per person:
neither exists globally yet.

## 3. The new controls, exactly

### 3.1 Notifications from them (rung 1)

Three levels, the room level's own words reused (`data/notifications/notifications.json`
`person_levels`), kept per person:

- **Everything** (the default): their messages, calls and mentions notify as your other settings
  allow. No entry is kept.
- **Only calls, mentions and replies**: a notice only when they call you, mention you (`@your
  name` in a room) or reply to a line of yours. Their direct messages and their other lines in
  rooms arrive quietly, their unread marks shown as always.
- **Nothing**: no notice from them at all, calls included. Their messages and calls still arrive;
  with the window open the ringing screen still shows (as with every mute: the rule holds back the
  notice and the sound, never the call).

**In THE rule.** Each kind in the data file gains `person_level`, the least a sender's level must
be for that kind to notify (absent: `all`): `mention`, `reply` and `call` are `mentions`; `dm`,
`room_line` and `contact_request` are `all`. A candidate carries `from_level`, the sender's level
(`null`: no choice, which counts as Everything). The check comes right after the mutes, before the
room level and quiet hours, so a person set to Nothing stays quiet even when they are on the
always-allow list (a mute holds over an exception the same way), and its answer is `person_level`.
A task and a game event carry no level: a task board is the server's, and the game is the world.
The shared cases (`scripts/tests/fixtures/notify-cases.json`) hold both twins to it.

**Where the level is read.** The desktop's candidates (`engine/notify.rs` `room_line`, `dm`,
`call`, `contact_request`) fill `from_level` from the sender's key; the website's
(`chat-notify.js` `decide`) from the conversation (`dm:<key>`) or the line's `from_key`.

### 3.2 Files from them (rung 1)

- **Use my setting** (the default): Files in messages decides, as before. No entry is kept.
- **Always ask**: every file from them waits for Accept (a friend's too): "You chose to be asked
  about every file from Ann. Accept this one?"
- **Up to a size without asking** (N MB, from 1 to 1,024, the largest private file a server can allow; 10 until
  changed): a file up to that size is taken as from someone you accept files from (pictures still
  follow "Show pictures without asking from"); a bigger one asks first: "This file is over the 10
  MB you take from Ann without asking. Accept this one?" Under the protected setup it only
  narrows: a file is taken without asking only when the protected setup's own rule would take it
  too.
- **Never**: nothing from them is fetched, and there is no Accept: "You chose not to take files
  from Ann." To take one, change the setting.

**Big files sent device to device always ask** (the offer's question, `files-showcase.md` answer
10). For an offer the rule adds: Never leaves it out with the same line and nothing sent back (as
the protected setup's leaving out does, so the sender's row simply runs out); Always ask, and a
size within the limit, go straight to the question; a size over the limit asks first with the
over-the-limit line, then the question.

**In code.** One pure verdict per file, both apps, held to shared cases
(`scripts/tests/fixtures/person-controls-cases.json`): `Mine` (the global rule decides), `Accept`,
`Ask` (with its line), `Never`. Desktop: `src/net/person_controls.rs` `file_verdict`, read by
`attach_view::plan` and `file_offer::question`. Website: `web/shared/person-controls.js`
`fileVerdict`, read by `chat-dms.js` (files in messages) and `chat-file-transfer.js` (offers).

### 3.3 Where the choices are kept (rung 1)

One record per person, by the identity it belongs to (so a second identity on the same device has
its own), only what differs from the defaults:

- Desktop: `config.json` `person_controls`: `{ "<my key>": { "<their key>": { "notices":
  "mentions", "files": "up_to", "files_mb": 10 } } }` (`AppConfig::person_controls`,
  `src/net/person_controls.rs`). Beside the always-allow list and the mutes, which are kept the
  same way (`notifications.md` section 7; rung 4 of that design moves all of them into the
  encrypted local store together).
- Website: localStorage `hos_person_controls`, the same shape.

### 3.4 Travelling between my devices (rung 2)

A note to myself per change, the always-allow list's way: a sealed DM from me to me whose text is
`[[hum:person:v1]]<their key>/<field>/<value>`, with `field` one of `notices`, `files` (its value
`mine`, `ask`, `never`, or `up_to:<MB>`), sent to my own mailbox on every server I am connected
to, kept for the next connection when none is. Per person and field the newest note (by its
signed time) wins, so a note delivered twice, or two devices' changes arriving out of order,
settle the same everywhere. A note from anyone else is dropped unread. Until rung 2 the card says
"Kept on this device."

## 4. The card

Native first (`src/gui/pages/chat/modals.rs` `draw_user_modal`, the section drawn by
`src/gui/pages/chat/person_controls.rs`); the website mirrors it in the profile overlay
(`web/chat/chat-profile.js` `showViewProfileCard`).

```
  (avatar)  Aria  [verified]
  Online   Friends
  Key  503c7aeb...acf2e1bf             Copy
  [ Send DM                          ]
  [ Call ]  [ Unfollow ]
  ── Your settings for Aria ──────────────
  Notifications from Aria
  [ Everything                     v ]
  [x] Calls and messages come through quiet hours
  Files from Aria
  [ Use my setting                 v ]   (Up to: [ 10 ] MB)
  What Aria may do            (friends only)
  [x] Message  [ ] Call  [x] Trade
  Their conversation is muted until 14:00   [Unmute]   (only while muted)
  Kept on this device.
  [ Report ]
  [ Block  ]
  ── Moderation ── (mods)   ── Admin ── (admins)
```

- **One plain section,** its heading "Your settings for <name>", every control with its hint by
  key. Each control is a label line above its control, so the section works at the card's
  narrowest (320 points on the desktop; a phone's 412-pixel viewport on the website, where the
  card is 90% of the screen's width and scrolls inside 85% of its height, `web/chat/modals.css`).
- **The friend's ticks** appear only for someone in "People I choose" (`people_to_choose`: anyone
  holding a pass from me, and mutual follows still owed one). For anyone else one line says what
  applies: "They reach you as Settings > Safety says for people who are not your friends." The
  ticks call the same `set_tick` as Settings > Safety (so the protected setup's PIN still guards
  them), and one line under them says when they count ("These ticks count where Settings > Safety
  is set to People I choose."); Safety's own "In use now" line stays there.
- **Not for yourself,** and not for a bot (`bot_` keys).
- **Moderation and Admin** stay below Report and Block, after a separator, unchanged.

## 5. The rungs

1. **(built 2026-11-03)** The card's section on both apps: Notifications from them (new, in THE
   rule's two twins and the shared cases), Files from them (new, in the file's plan and the
   offer's question on both apps, with shared cases), the quiet-hours switch (the existing
   always-allow list), the friend's three ticks (the existing choice), and the conversation's mute
   with Unmute (the existing mute). Kept on the device it was set on. The empty-name fix
   (section 7).
2. The record travels between my devices (3.4), both twins with one test vector, and Settings
   lists "People you have settings for" with Reset (the always-allow list's Settings row joins
   it). The card's "Kept on this device." goes.
3. Pictures from them; the line "What they see of you" (a friend: what you keep for friends in
   Profile > Privacy; anyone else: what you show everyone), with a link to Profile > Privacy; the
   Household permit named on the card.
4. A name of your own for them; their voice's volume and silence in rooms; a ring of their own;
   calls with them always private, if the operator wants it (question 1).

## 6. As built (rung 1, 2026-11-03)

- **Native.** `src/net/person_controls.rs` (the record, the levels and file choices, the verdict;
  no socket and no GUI, unit tested), `AppConfig::person_controls` (`src/config.rs`), THE rule's
  `from_level` check (`src/notifications/decide.rs`, the data file's `person_level` and
  `person_levels`), the candidates (`src/engine/notify.rs`), the file's plan
  (`src/gui/pages/chat/attach_view.rs`, `Show::Refused`), the offer's question
  (`src/net/file_offer.rs` `question`, which takes the verdict, `Question::Refused`; drawn by
  `src/gui/pages/chat/transfer_rows.rs`), the card's section
  (`src/gui/pages/chat/person_controls.rs`, drawn by `modals.rs` `draw_user_modal`, which is now
  a scrolling dialog since its height depends on the choices), and the openers through `modals.rs`
  `open_profile` and the name through `card_name` (section 7).
- **Web.** `web/shared/person-controls.js` (`HosPersonControls`: the record, `fileVerdict`, the
  words), THE rule's twin (`web/shared/notify-rules.js`), `chat-notify.js` (`personLevel`, filling
  `from_level` from a DM's conversation or a room line's `from_key`, which `chat-ui.js` now passes),
  `chat-dms.js` (`privateFilePersonVerdict`) and `chat-file-transfer.js` (an offer under Never is
  `refused` with `refusedPerson`; every other choice reaches the question, the website having no
  "Accept this one" step for offers), the overlay's section (`chat-profile.js` `personControlsHtml`,
  `wirePersonControls`, and `profileCardName` for the name).
- **Proof.** Each seen red once: the shared notification cases (both twins), the shared file
  cases (both twins), the record's round trip through `config.json`, the card drawing its section
  (a headless frame) and the snapshot `user_profile_modal`, a call during quiet hours ringing only
  once the card's switch is on (the card clicked headless through the app's own egui), a person
  set to Nothing making no notice, every candidate from a person carrying their level
  (`engine/notify.rs`), the setting persistence lint scanning the card, and the empty-name fix.
  The snapshots `user_profile_modal` and `user_profile_modal_narrow` (380 points wide) show the
  section with choices made.

## 7. The card with no name (fixed with rung 1)

**Cause.** The card draws `GuiState::chat_user_modal_name`, which only its opener sets, and one
opener did not: a direct message's right-click "View Profile" (`gui/pages/chat/left_panel.rs`) set
the key and left the name as it was, empty at the start of a run (the operator's screenshot: a "?"
avatar, no name) or the name of whoever's card was opened last (a wrong name). **Fix (BUG-244).**
The openers outside the message list go through one function, `modals.rs` `open_profile(state,
key, name)`, which sets the key and the name together (the two inside the message list set the
same fields by hand: the list holds a borrow of the state), and the card never draws an empty name: an empty one is found from the key (the member list, friends, the DM list,
then the start of the key, `engine/dm.rs` `dm_display_name`), each frame, so a name learned after
the card opened shows too. The website's card (`requestViewProfile`, `showViewProfileCard`) finds a
missing name from the key the same way (`profileCardName`: the member list, the DM list, the start
of the key).

## 8. Questions only the operator can answer

1. **Calls with one person always private?** A per-person "Always private" for how calls connect
   (10p) is possible on the device. Recommendation: not until someone asks; the global choice is
   already private by default.
2. **Should "Up to N MB" also accept big files sent device to device without a click?**
   Recommendation: no. They land on the disk straight from the other device, and one click is
   cheap safety; the hint says they always ask.
3. **Should "Nothing" also hide a ringing call while the window is open?** Recommendation: no. To
   stop someone calling, untick their Call; "Nothing" is about notices, like a mute.
4. **One always-allow switch, or separate ones for calls and for messages?** Recommendation: one,
   as decided on 2026-11-01 ("unless I specifically add users to the always allow list"): someone
   trusted to ring at 3 in the morning is trusted to write then.
5. **Hide your presence from one person?** It needs presence sent to friends in sealed notes
   instead of by the server's member list (section 2). Recommendation: a later design, after
   federation's presence work.

### 8.1 His answers (2026-11-03)

To questions 2, 3 and 5, as put to him ("Should open without asking up to N MB also apply to big files sent straight from their device? No: those land on your disk directly.", "Should Nothing also hide the ringing screen? No: untick their Call instead.", "Should hiding your online status from one person get its own design later? Yes, because the server would have to know who you are hiding from."): "Your recommendations sound good." So: big files sent device to device always ask, whatever the size limit; Nothing is about notices and never hides a ringing call; hiding presence from one person waits for its own design. Questions 1 and 4 keep their recommendations until he says otherwise.
