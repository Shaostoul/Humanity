# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the update of 16 November 2026
(`docs/outreach/posts/2026-11-16-update.md`, v0.1558.0 to v0.1564.0, with v0.1557.0 in one line).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

### v0.1565.0 (20 November 2026)

- The family home's beds stand against their head walls, nightstands flush beside them; living rooms and studies get a picture rail.
- A mining drone's haul no longer trains Mining.
- Every website page checked on a small phone: Calendar, Inventory, Tasks, Settings, Maps and the Calculator no longer cut controls off.
- Both public servers open their call forwarder, and each names itself as the place to connect.

### Next release

- A server that was never told its own name no longer sends callers to another server's call forwarder; it says plainly which setting to add.
- A server that was never told which websites may use it now allows only its own pages, never another server's.
- The website no longer runs any script written inside its pages, so a script slipped into a page cannot run.
- Emoji pictures come from our own server (OpenMoji), so no outside company sees who is chatting; the reaction picker credits OpenMoji, and the heart reaction shows as a picture again.
- A written content policy says what is never allowed and how to report it, with the six steps both servers follow for child sexual abuse material; it is in the Library and linked from Settings > Safety and the website's footer.
- The wallet's key is made from your recovery phrase under its own label, so exporting it can never hand over your identity or your messages.
- Sealed messages written with accented or non-Latin letters are padded to the same sizes as plain English ones, so their size says nothing about the language.
- The post-quantum code is now checked against the official US government test vectors, in the app and on the website.
- Written music: the Music page can play written pieces on our own instruments, starting with the Ode to Joy.
- Developers on Linux can run the project's test tools; the checks run on Linux for every change.
- Moderators can "hide and keep" a report about child abuse material: it disappears everywhere at once, a copy is sealed so only the server's owner can open it, kept a year and then deleted, and every step is signed and logged. Nobody, the owner included, gets a button to view it.
- Even if someone slipped markup into a page, the website now ignores any button action the page did not write itself.
- The Music page plays Für Elise, Grieg's In the Hall of the Mountain King (his piano arrangement) and the first movement of Beethoven's Fifth, with repeats, grace notes and dynamics.
- Every open-source library inside the app now has its licence and copyright notice in the download and on the Credits page.
- The simulation gives the same result every time for the same starting point, so the game's checks can trust it.

### Outreach

- HumanityOS is listed on AlternativeTo (approved 18 November 2026), next to Discord, Element, Signal, Telegram and other chat apps: https://alternativeto.net/software/humanityos/about/
- The project now says who holds the copyright: Shaostoul and the HumanityOS contributors, each keeping what they wrote, so nobody can take it private.
