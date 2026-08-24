# Privacy by architecture: what we built, and what we can't

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

## The moment that started it

A court order landed that forced Discord and Microsoft to hand over personal
information on roughly 100,000 users, real names, emails, phone numbers, IP
addresses, device identifiers, linked accounts, to help hunt down a single
person who leaked a video game. Most of those hundred thousand people had
nothing to do with the leak. Their only mistake was being in the wrong chat
server at the wrong time.

Discord is not uniquely evil here. It did what almost every platform is built
to do: it collected everything, and so when the order came, it had everything
to give. That is the entire lesson. "We promise to protect your data" is a
promise standing on a foundation of having collected the data in the first
place, and that foundation is one subpoena, one breach, or one change of
ownership away from crumbling. Every platform that holds your information is
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
word. In the Discord case, the envelope was precisely what the order wanted.

We rebuilt the whole system so the server no longer records the envelope. The
sender's identity now travels sealed inside the encrypted message, signed so
the recipient can verify it, and the server stores each message addressed only
to its recipient with no record of who it came from. Messages expire off the
server after a short delivery window; your real history lives encrypted on
your own devices. What a subpoena of our database now returns is a pile of
scrambled blobs addressed to anonymous keys, with day level timestamps and no
senders. There is no conversation graph to reconstruct, because we stopped
writing it down.

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
is friends with whom," because it does not know and never records it. That
list, the one Discord had to hand over for a hundred thousand people, does not
exist on our side to hand over.

### What you said in a shop or a group

Two more categories of conversation were still stored in plain, readable text.
Marketplace messages between buyers and sellers were kept unencrypted on the
server, and, worse, were broadcast to every connected client. The old group
chat system stored its membership rosters and every message in the clear too.
Both are now gone. Contacting a seller opens a normal end to end encrypted
direct message. Groups run entirely on encrypted, signed data the server cannot
read. Both plaintext tables were dropped from the database outright.

### Where your photos were taken

This one surprises people, and it is the kind of leak nobody consents to
because nobody knows it is happening. Every photo your phone takes carries
hidden metadata: the exact GPS coordinates of where you were standing, the
time, often the camera's serial number. Upload a nice picture of your garden
to share, and you may have just published your home address to anyone who
downloads it.

Now, every image uploaded to a HumanityOS server is stripped of that hidden
metadata before the file ever touches our disk. The stripping is lossless, so
your image quality is untouched; it just quietly removes the coordinates and
the serial number. You do not have to remember to do anything. It is simply
gone.

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
server used to keep two weeks of visitor IP logs; we cut that to two days, just
enough for the automatic abuse blocking to function, and deleted the backlog.

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
it is a bit slower, and it depends on the operator setting it up. We can open
the door; we cannot walk through it for you.

**Timing and traffic patterns.** Even when a watcher cannot see who you are or
what you said, someone monitoring the raw connection in real time might notice
that traffic flowed to your recipient right after traffic came from you. Fully
defeating that kind of timing analysis requires what is called a mix network,
which deliberately shuffles and delays messages. We have not built that, and it
is a genuinely hard, unsolved-in-general problem. The onion service helps for
users who opt into it; universal protection against a patient wire watcher is
honestly beyond where we are today.

**Public is public.** Anything you post in a public channel is public, exactly
like a public post anywhere else. The architecture protects what you did not
choose to broadcast. It cannot un-ring the bell on what you did.

**Replication means no global undo.** HumanityOS is designed so anyone can run
a server and so data can move between them, which is a strength: no single
company owns your world. But it means that once something has been shared to
independent servers or other people's devices, we cannot reach across the
whole network and guarantee it is deleted everywhere. Deletion is real on the
servers we control and honest about its limits beyond them.

**Certificates do not yet expire.** In this first version, the friendship
certificates that grant messaging access do not have an expiration date and
cannot be revoked by the server. Unfriending someone is handled on your side:
your client stops treating them as a friend, and their messages fall back to
the limited stranger allowance. It works, but a future version should add
proper expiration and rotation, and we would rather tell you that now than let
you assume otherwise.

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
