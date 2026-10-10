# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the week update of 5 October 2026
(`docs/outreach/posts/2026-10-05-week-update.md`, v0.1421.1 to v0.1464.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

- **v0.1464.2: the server got its footing back.** A web crawler filled the server's disk on 5 October by asking the git mirror for over a hundred half-gigabyte copies of the code. Found and fixed on 9 October: the chat server is current again, the git mirror syncs again and now builds at most one downloadable copy a minute, the disk alarm posts once instead of every 20 minutes, and the server can no longer lose its own program while clearing space.
- **v0.1464.3: your data syncs only between your own devices.** Web chat's data sync (calendar, home records, notes, map pins) answered anyone who connected to you directly; now it answers only your other devices and asks before merging. And #announcements became one post per release: 5,325 bot notices became 2,354, each named by its release.
- **v0.1465.0: your network address stays yours.** Both apps now connect directly only to people you know (friends, your groups, your call); the web chat no longer starts a call before you press Accept; Google is asked for your address only when a call starts; pictures from other websites wait for a click; hiding your online status really hides it; nobody can add you to a group without you; and strangers' trade requests share the daily limit messages have.
- **v0.1466.0: friendships you can end, and that never run out.** Unfollowing someone now withdraws their pass at once, so they can no longer message you without limit; a friendship never expires on its own; and each pass says what a friend may do (message, voice message, invite, trade, call), with calls left for people you choose.
- **v0.1467.0: you choose who can reach you.** A new Safety section in Settings, in the desktop app and the web chat: for messages, calls and trades, choose Nobody, People I choose, Friends, Friends and people in my groups, or Anyone. Safe by default: messages and trades from friends, calls only from people you pick. A stranger can send a contact request that shows you only their name, and you accept or ignore it. The server enforces your choices for everyone, admins included.
- **v0.1468.0: Block.** In both apps, one click blocks someone: their messages, posts, calls, trades and requests disappear for you, the friendship pass you gave them is taken back, and they are not told. Your other devices learn it too. Settings > Safety lists everyone you blocked, with Unblock.
- **v0.1469.0: reports the admins can check.** Report someone with the messages they sent you as evidence; because every message carries its sender's signature, the server can prove they wrote it, and nobody can forge a report or pin it on someone else. Admins and moderators review reports in the app; the reported person is never told who reported them. For a child or anyone in danger, the dialog says to contact your local emergency number first.
- **v0.1470.0: calls that keep your address private.** Voice calls and voice rooms now go through the server, so nobody in a call sees another person's network address, and the apps no longer ask Google for anything. The forwarder can only pass audio between people in the same call or room. (Calls switch on once the server's port is opened.)
- **v0.1471.0: warnings that explain the trick, and a guard on your recovery phrase.** When someone who is not your friend sends a message asking for gift cards or a wire transfer, asking for your recovery phrase, pushing you to move the chat to another app, rushing you or telling you to keep it secret, or claiming to be staff, a short note under it explains the trick and what to do (the money and recovery-phrase notes show under friends' messages too, because an account can be taken over). Their links wait until you press Open. Nothing is read anywhere but on your own device, and nothing is reported. And neither app will send a message holding your own recovery phrase, even pasted as a numbered list.
