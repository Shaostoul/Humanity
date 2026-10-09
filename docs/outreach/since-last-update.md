# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the week update of 5 October 2026
(`docs/outreach/posts/2026-10-05-week-update.md`, v0.1421.1 to v0.1464.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

- **v0.1464.2: the server got its footing back.** A web crawler filled the server's disk on 5 October by asking the git mirror for hundreds of half-gigabyte copies of the code. Found and fixed on 9 October: the chat server is current again, the git mirror syncs again and now builds at most one downloadable copy a minute, the disk alarm posts once instead of every 20 minutes, and the server can no longer lose its own program while clearing space.
