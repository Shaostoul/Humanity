# How your privacy is built: what we did, and what we can't

There is a question every online platform quietly answers for you, usually
without telling you: when someone powerful demands your data, what can they
get? Not "what will the company choose to hand over," but what physically
exists to be handed over. Those are very different questions, and the gap
between them is where almost every privacy disaster lives.

This is the story of spending a few days closing that gap on HumanityOS,
until the answer to "what can they get" is, for the things that matter,
nothing. It is also an honest account of the parts we cannot close, because
a privacy claim that hides its own limits is just marketing, and we would
rather you trust the architecture than trust us.

*Checked against the code on 6 November 2026, and again on 7 November 2026, the
day the fix to the order of stored messages went live on united-humanity.us.
Where an earlier version of this page claimed more than the code did (groups,
photos, the order of stored messages, the onion door, friendship passes, how
long logs are kept), it now says the narrower thing that is true. Its opening
story is now told from the court filings themselves, read on 7 November 2026:
[our dated research note](../reference/findings/2026-11-07-take-two-discord-subpoena.md)
links each one.*

## The moment that started it

In August 2026, Take-Two Interactive, the publisher of Grand Theft Auto VI,
went to a federal court in New York to find out who had posted leaked material
from the game. Under the law that lets a copyright owner ask a court for a
subpoena to identify someone it accuses of copying its work, it asked for one
subpoena to Discord and one to Microsoft, and both were approved on 21 August
2026.

The one to Discord named three accounts, and then went much further. It asked
about every account that had been a member of three Discord communities since 1
June 2026, one of them the community server of a Grand Theft Auto video creator,
which Discord's own count put at more than 117,000 members on 7 November 2026.
For each of those accounts it asked for email addresses, phone numbers, IP
access logs, linked accounts, device identifiers and "original message logs".
The one to Microsoft asked for much the same about members of the same
communities. Three accounts were named; nearly everyone else in those
communities had nothing to do with the leak. Their only mistake was being in the
wrong chat server at the wrong time.

Discord is not the villain of this story. It said it would "evaluate the
validity and scope before responding", and nothing public says that it or
Microsoft handed anything over. But a request like that is worth making only
because Discord, like almost every platform, had collected all of it. That is
the entire lesson. "We promise to protect your data" is a promise standing on a
foundation of having collected the data in the first place, and that foundation
is one subpoena, one breach, or one change of ownership away from crumbling. Every platform that holds your information is
holding a liability on your behalf, and you are the one who pays when it
spills.

We build HumanityOS on the opposite bet: collect nothing you do not
absolutely need, and for the things you do keep, arrange them so that a
stolen copy or a legal demand yields noise instead of people. The technical
term is data minimization. The plain version is: you cannot be forced to hand
over what you never had.

When the Discord news broke, we ran that same subpoena, in our heads, against
our own database. Most of it came back empty, because we never had emails or
phone numbers or device IDs to begin with. But not all of it. Someone made a
sharp argument that stuck with us: your messages may be encrypted, but if the
server still has a readable record of who talked to whom, a court order is
just a spreadsheet export. They were right. So we went looking for every
place that was still true, and we fixed all of them.

Here is what we found, what we did about it, and what remains.

## The problems we solved

### Who talks to whom

Direct messages on HumanityOS were already end to end encrypted, meaning the
words themselves were unreadable to us. But the server still kept a plain
record of every message's envelope: who sent it, who received it, and when.
That is metadata, and metadata is often more revealing than content. Knowing
that a person messaged a divorce lawyer, a clinic, and a rival company's
recruiter, in that order, tells you almost everything without reading a single
word. The subpoena to Discord asked for records of exactly this kind, each
member's "IP access logs" and "original message logs" among them.

We rebuilt the whole system so the server no longer records the envelope. The
sender's identity now travels sealed inside the encrypted message, signed so
the recipient can verify it, and the server stores each message addressed only
to its recipient with no record of who it came from. Messages expire off the
server after a short delivery window; your real history lives encrypted on
your own devices. What a subpoena of our database now returns is a pile of
scrambled blobs, each addressed to an anonymous key, padded to one of a few
fixed sizes, with the day it arrived and no sender. The rows are kept by
recipient rather than in the order they arrived, so nothing in the database
pairs a message with the person who sent it. And the app sends its own copy of
what you wrote (the one your other devices read) a random 20 seconds to 2
minutes after the recipient's, so the two are not written one right after the
other either: the website does this now, and the desktop app does from its next
update.

That last part was not true until November 2026, and we found it while checking
this page against the code. Rows used to be numbered in the order they arrived,
and your app sent your own copy right after the recipient's, so two neighbouring
rows of the same size could pair you with the person you wrote to. We fixed both
halves in the code on 6 November 2026; the server's half went live on
united-humanity.us on 7 November 2026, and so did the website's. Until the
desktop app's update, the server's
write log and its continuous backup still hold a desktop user's two copies one
right after the other, even though the database itself no longer pairs them.
Other servers get the fix when they update. Rows stored with the old numbers
are deleted when a server updates, fetched or not, rather than waiting out the
30-day window; backups taken before the fix keep them for as long as those
backups are kept.

What the database still cannot hide, said plainly: at the moment you send, the
server you are connected to knows it is you and whom the sealed envelope is
for, because it arrives on your signed-in connection. It does not write that
down, but a server changed to record it could. And the server's write log and
its continuous backup keep the order things were written in for as long as they
are kept, so on a server so quiet that nothing else happens in the minutes
between your message and your own copy, someone holding them could still guess
the pair.

### Who is friends with whom

This was the hard one, and the one the outside critique pointed at. Even after
the message envelopes were gone, the server still held a friends and followers
list. That is a social graph, and a social graph is one of the most sensitive
things a platform can own. It reveals your community, your associations, your
private support networks, the people you would never want mapped by a stranger.

The difficulty was that this list did real work: it decided who was allowed to
message you. We could not simply delete it without breaking a feature people
rely on. So we moved the entire concept off the server and into your own hands.
Following someone is now a private, encrypted message between your devices and
theirs. Friendship is a cryptographic certificate that your own device holds
and presents when you send a message; the server checks the certificate's
signature and immediately forgets it, storing nothing. A stranger with no
certificate can still send you a small, capped number of polite messages a day,
so someone can reach out to you cold without anyone being able to flood you.

The result is that our server can no longer answer "who follows whom" or "who
is friends with whom," because it does not know and never records it. A request
like the one sent to Discord would find no such list on our side to hand over.

### What you said in a shop or a group

Two more categories of conversation were still stored in plain, readable text.
Marketplace messages between buyers and sellers were kept unencrypted on the
server, and, worse, were broadcast to every connected client. The old group
chat system stored its membership rosters and every message in the clear too.
Both are now gone. In the desktop app, contacting a seller opens a normal end
to end encrypted direct message. The web marketplace has not been moved over
yet, and its old message box does nothing at all, which is a bug we are
fixing; until then, contact a seller from the app or by direct message in
chat. Both plaintext tables were dropped from the database outright.

Groups are now built differently, and here we have to be careful with the
words. What is said in a group is end-to-end encrypted: the server stores and
passes on messages it cannot read. But a group is a set of signed records, and
the server that carries one keeps those records where it can read them: the
group's name in plain text, the list of who is in it, and each message's author
and the time it was posted. So a copy of that server's database shows who talks
with whom in groups, even though it cannot show what they say. Hiding those too
is planned work, not done.

### Where your photos were taken

This one surprises people, and it is the kind of leak nobody consents to
because nobody knows it is happening. Every photo your phone takes carries
hidden metadata: the exact GPS coordinates of where you were standing, the
time, often the camera's serial number. Upload a nice picture of your garden
to share, and you may have just published your home address to anyone who
downloads it.

Now your app removes that hidden metadata on your own device before a photo
or video leaves it (a switch, "Remove location and camera details", on unless
you turn it off), and the server removes it again from the pictures, videos and
phone photos uploaded to it, before the file is stored. JPEG, PNG and WebP
pictures lose their EXIF, XMP and comment blocks; GIF pictures lose their
comments and the blocks programs write into them, XMP among them, while their
animation stays exactly as it was; HEIC and AVIF photos have their EXIF and XMP
blanked; MP4-family videos have their location boxes blanked.
The removal is lossless, so the picture itself is untouched.

When a file cannot be read all the way through, the metadata cannot be removed
with certainty, and until 7 November 2026 the server on united-humanity.us then
stored the picture as it came, location and all. Now it stores nothing unless
you say so: the app asks you, "Send it anyway, or don't send?", with "Don't
send" first, and only your "Send anyway", signed with your own key for that one
file, lets the server keep it as it is. The website asks now; the desktop app
asks from its next update, on the Files page as in the chat.
One limit we know of: a location recorded inside a video's own picture stream,
as some action cameras and drones do, is not caught.

### Photos and files inside a private message

Here is a subtle one we caught late, and it is worth telling you about because
it is exactly the kind of gap that hides behind a feature that looks finished.
Messages were encrypted. But a photo you attached to a private message was not
encrypted the same way; only the link to it was hidden inside the encrypted
message, while the file itself sat readable at a web address on the server.
Someone sending a sensitive photo in a DM believed it was as private as the
words, and it was not.

So we fixed it to match the promise. A file shared in a private message is now
encrypted on your device before it is uploaded, with a fresh key each time. The
server stores only scrambled bytes; the key to unlock them travels sealed
inside the message, so only the person you sent it to can turn it back into a
photo. To the server operator, and to anyone who finds the web address, it is
noise. Public channels are unchanged, because public is public. But a private
photo is now genuinely private, file and all.

### Whether you appear online

Being visible online is a real safety issue for some people. Knowing when a
specific person is awake, active, or reachable is a stalking tool. So presence
is now something you fully control, and it is enforced by the server, not just
hidden in your interface. If you choose to be invisible, you never show as
online, you generate no "is typing" or "joined" signals, and your last seen
time is not merely hidden from others, it is never written down in the first
place. New accounts start invisible until their owner chooses otherwise, so the
default protects you before you have configured anything.

### The copies you forget: backups and logs

Backups are where privacy quietly dies. They are the copy of everything,
sitting in a folder that gets synced to other machines, and they are usually an
afterthought. Ours are now encrypted at rest, with the key deliberately kept
somewhere the backups themselves never travel. A backup that gets copied off
the machine is unreadable scrambled data without that key. Separately, the web
server used to keep two weeks of visitor IP logs; we cut that to at most three
days (it starts a new log each day and keeps the two before it), just enough
for the automatic abuse blocking to function, and deleted the backlog.

### How much you said, and for how long

Two subtler leaks. First, the size of an encrypted message can betray its
length; a one word "ok" and a long confession look different on the wire even
when both are unreadable. We now pad every message up to a fixed size bucket
before encrypting, so a short reply and a full paragraph produce identical
sized ciphertext. Second, server operators can now set public messages to
automatically expire after a chosen number of days, so even public history does
not linger forever, with pinned messages always kept.

### You choose, and the default is maximum privacy

All of this control means nothing if it is buried in settings nobody finds. So
the very first time you join, you choose how visible you want to be, and the
default we start you on is maximum privacy: invisible, unlisted, discoverable
by no one unless you decide otherwise. At the other end of the dial is a
setting we built on purpose for streamers and creators who want the opposite,
maximum discoverability, so they can be found the moment they go live. Privacy
should be a deliberate choice with a safe default, not a maze you have to solve
before you are protected.

### Your data is actually yours

Finally, the two buttons every platform should have and almost none do. You can
export everything the server holds about you as a single file, and you can
erase your account and all of its data permanently, yourself, with no support
ticket and no admin approval, on any HumanityOS server. We made these
impossible for a server operator to switch off, because a right that the host
can disable is not a right.

## How we did it, in one idea

If there is a single technique underneath all of this, it is this: replace
"the server remembers, and promises to be careful" with "the server verifies,
and forgets." Again and again, the pattern was to take something the server
used to store, a friends list, a sender's identity, an online status, and turn
it into something a client proves in the moment with a cryptographic signature.
The server checks the proof, acts on it, and keeps no record. The math does the
work that trust used to do.

This is slower and harder to build than just keeping a database of everything.
It means the server genuinely cannot offer certain conveniences, because it no
longer holds the data those conveniences would need. We think that is the
point. A platform that cannot betray you is worth more than one that promises
not to.

## The things we cannot control, honestly

A privacy post that stops here would be dishonest, so it does not stop here.
Some things are outside what any application can fix, and pretending otherwise
would be exactly the kind of overpromise we are trying to get away from.

**Your IP address on a live connection.** Any server on the internet, ours
included, can see the network address of a computer currently connected to it.
That is how the internet routes packets; it is physics, not policy. We do not
tie those addresses to your identity and we do not keep them, but in the moment
of connection they are visible. The honest fix is not something we can do for
you inside the app; it is something you layer underneath it. So server
operators can now run what is called a Tor onion service, which lets you reach
the server through an anonymizing network where neither end learns the other's
location. Over that door, there is genuinely nothing to see. But it is opt in,
it is a bit slower, and it depends on the operator setting it up. Two more
limits, as of 6 November 2026: the door reaches the server's chat and the
connection the apps use, not yet its website; and united-humanity.us itself does
not run one yet. We can open the door; we cannot walk through it for you.

**Timing and traffic patterns.** Even when a watcher cannot see who you are or
what you said, someone monitoring the raw connection in real time might notice
that traffic flowed to your recipient right after traffic came from you. Fully
defeating that kind of timing analysis requires what is called a mix network,
which deliberately shuffles and delays messages. We have not built that, and it
is a genuinely hard, unsolved-in-general problem. The onion service helps for
users who opt into it; universal protection against a patient wire watcher is
honestly beyond where we are today. Hiding when you send among dummy messages,
and keeping your own copies of what you send off the server altogether, are
planned, not built.

**Public is public.** Anything you post in a public channel is public, exactly
like a public post anywhere else. The architecture protects what you did not
choose to broadcast. It cannot un-ring the bell on what you did.

**Replication means no global undo.** HumanityOS is designed so anyone can run
a server and so data can move between them, which is a strength: no single
company owns your world. But it means that once something has been shared to
independent servers or other people's devices, we cannot reach across the
whole network and guarantee it is deleted everywhere. Deletion is real on the
servers we control and honest about its limits beyond them.

**Friendship passes do not expire.** The friendship passes that let a friend
message you have no end date. Since version 0.1466.0 you can withdraw one you
gave: your app tells the server, which then refuses that pass, and keeps only a
scrambled tag of your key, the pass's number and the day, not whom you gave it
to. A pass
still lasts until you withdraw it, and an end date that renews on its own would
be better; we would rather tell you that now than let you assume otherwise.

**Your own device.** None of this protects you from a compromised device. If
someone has malware on your computer or physical access to your unlocked
machine, they can read what you can read. Encrypting the server's copy does not
help if the attacker is sitting where the decrypted copy lives. Protect your
device, use the passphrase protection we offer for your keys, and understand
that the endpoint is always the last line, and it is yours to hold.

**Old backups age out, they do not vanish instantly.** When we dropped those
plaintext tables, backups taken before the change still contained them until
they rotate out over the following days. We do not pretend a change is
retroactive across every snapshot ever made. It becomes true going forward, and
the old copies expire on their normal schedule.

## Why this is verifiable, not just claimed

Everything above is open source. That is not a slogan; it is the whole
argument. You do not have to believe a word of this. You can read the code that
refuses to accept an unencrypted message. You can look at the database schema
and see, with your own eyes, that the columns for sender identity and friend
lists are not there. You can run your own server and trust no one but yourself.
There is even a test in our codebase whose only job is to fail the build if
anyone ever tries to add a sender column back to the message store.

The standard we want to be held to is not "we promise we protect your data." It
is "come check that we never took it in the first place." Those are different
promises, and only one of them survives a subpoena.

Greed has consequences. So does architecture. The difference is that greed's
consequences land on you, and good architecture's land on the person who comes
asking for your data and finds there is nothing to take. We chose ours on
purpose, and we will keep choosing it, in the open, where you can watch.
