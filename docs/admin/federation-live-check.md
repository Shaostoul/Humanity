# Federation: the live check on two linked servers

Written 2026-10-22 for the operator's pair, united-humanity.us (called A below) and public.guide
(B), which already share #general and #announcements. It runs on the live servers the same check
the automated Phase 3 test runs on two relays on one machine
(`src/relay/handlers/federation_phase3_tests.rs`; what it proves and why is in
`docs/design/federation-activation.md`, Phases 2 and 3). Use a test room, never #general, so
nothing here lands in a room people read. Allow about 20 minutes.

## Before you start

1. **Both servers run a release from 2026-10-22 or later.** The words a server signs for a chat
   line it sends now name the author's key, so a server on an older release refuses the other's
   chat lines (and the other way round) until both are updated. public.guide updates itself when
   a release comes out; check both:
   `curl -s https://united-humanity.us/api/server-info` and
   `curl -s https://public.guide/api/server-info`, and compare `version`.
2. **The link is up.** On each server, the Admin page's federation list (or Server Settings >
   Federation in the desktop app) shows the other at trust tier 2 and online. Each row now carries
   a `link` object (the admin console's federation panel shows it): `sending.mode` should read
   `live` and `flow.state` `ok`. Until the panel shows it, the same data is in the answer to the
   panel's own request (the browser's developer tools, Network tab, the
   `/api/admin/federation` request).
3. **A test account** for the ban step, which has to be the SAME key on both servers: in a private
   browser window, sign up on united-humanity.us as `fedtest`, write down its recovery phrase,
   then open public.guide in the same private window and choose "Restore from recovery phrase".

## The check

Do each step in a room both servers share; the first step makes one.

1. **A test room on both.** On each server, Server Settings > Channels: add a channel with the
   same id on both, for example `fed-test`, and tick Federated. The id must match exactly; the
   name shown can differ.
2. **A message.** Post in #fed-test on A. It appears in #fed-test on B within a few seconds.
   Post on B: it appears on A.
3. **An edit.** Edit your message on A. B shows the new words (an app from before this release
   shows them after a reload).
4. **A delete.** Delete a message on A. It is gone on B.
5. **A profile change.** Change your bio on A. Within a few seconds
   `https://public.guide/api/profile/<your key>` shows it (your key: "Public Key" on the web Settings
   page).
6. **A ban, honoured in the shared room.** As admin on A, type `/ban fedtest` in chat.
   - On B, as fedtest, post in #fed-test: you are told "This room is shared with <A's name>,
     which has banned you, so you cannot post in it. This server's own rooms are still open to
     you." The post appears nowhere.
   - On B, as fedtest, post in a room of B's own (not federated): it works.
   - On B, the Admin page's row for A lists fedtest under `link.bans`.
7. **The ban lifted.** On A, type `/unban fedtest`. On B, fedtest posts in #fed-test: it appears
   on both servers.
8. **Nothing lost while one is away.** On B, run `sudo systemctl restart humanity-relay`. While
   it restarts, post three lines in #fed-test on A, edit the second and delete the third. Once B
   is back (the link comes up within about a minute), #fed-test on B holds the first line and the
   edited second line, each once, and the third never appeared there. On B,
   `journalctl -u humanity-relay --since "5 min ago" | grep Federation` shows the link coming
   back up.
9. **Tidy up.** Untick Federated on #fed-test on both servers (or delete the room), and erase the
   fedtest account on both if you like (chat's privacy panel, "Erase account", in that private
   window).

## If a step fails

Send the lines containing `Federation:` from both servers' journals for the minute around the
step (`journalctl -u humanity-relay --since "10 min ago" | grep Federation`), and the `link`
object of each server's row for the other from the Admin page's federation list. The lines to
look for:

- `refused ... not signed by it` or `rejected chat ... invalid or missing signature`: one server
  has not pinned the other's current key (Re-check key on the federation row), or one server is
  still on an older release (step 1 of "Before you start").
- `sent faster than its allowance for too long and is paused`: a flood, or allowances too small
  for real traffic. The pause lifts by itself and what was sent meanwhile is fetched again.
- `no longer had N frames this server missed`: the server was away longer than the other keeps
  its outbox (7 days, or 10,000 frames). Messages from that gap do not arrive; bans and profiles
  are re-sent whole.
