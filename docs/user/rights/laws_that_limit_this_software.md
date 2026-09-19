# The laws that limit what this software can do

Some things people ask us for, we will not build. Not because we disagree, and
usually not because they are hard. Because a law says no, or because a company
that controls a platform says no. This page names each one, so you can read it
and know where you stand instead of wondering whether we simply did not care.

It is also here for a second reason. If you think one of these rules is wrong,
this page is written so you can hand it to a lawyer, a representative or a
campaign and say: here is the rule, here is what it costs an ordinary person,
and here is what a better rule would look like.

**This is not legal advice.** It is a plain-language summary written by people
building software, and it leans on the law of the United States, where this
project's releases are hosted. Your country may differ, sometimes a great deal.
If something here matters to you, talk to a lawyer in your own jurisdiction.

## The one that bites hardest: you may own it and still not be allowed to open it

Copyright law says you may watch a film you bought. A separate law, written in
1998 and called the anti-circumvention rule, says that getting past a technical
lock is itself an offence, and it does not care whether what you do next is
perfectly legal. There is no exception for owning the thing.

The result is a rule most people do not believe when they first hear it. You
buy a DVD. You own that disc. You may lawfully watch it. The disc is encrypted,
so playing it means undoing the encryption, and undoing the encryption is
prohibited regardless of the fact that you own the disc and are only watching
it in your own home.

Two consequences follow, and the difference between them matters:

- **Using** a tool that does this is, in the United States, technically an
  offence with no personal-use exception, and it is essentially never enforced
  against an individual watching their own film.
- **Distributing** such a tool is where enforcement actually happens, and it is
  the reason this project will not ship one. Our releases live in a public
  repository under a named person. The realistic outcome of getting this wrong
  is not a courtroom, it is a takedown notice that removes every release we have
  ever published.

Where the same idea appears elsewhere: the European Union has a similar rule in
its 2001 copyright directive, though member states differ in how they apply it,
and France in particular has interoperability provisions that are part of why
certain well-known media software is developed there rather than here.

## Features people ask for, and which rule stops us

| What people ask for | Why we do not do it | Whose rule |
|---|---|---|
| Play a protected commercial DVD or Blu-ray in the app | Requires undoing the disc's encryption | Law, anti-circumvention |
| Download a video from a streaming platform | Breaks that platform's terms, and strips the advertising that makes free viewing possible | Contract, the platform's terms |
| Show a platform's video with the adverts removed | Same, and it is the advertising that makes embedding permitted at all | Contract |
| Ship decoders for some common video formats | Patent licensing we cannot afford and will not impose on people who redistribute us | Law, patents |
| Play a disc on a game console | Consoles do not let any third-party app touch the drive | Company policy |
| Run on iPhone, Xbox, PlayStation or Nintendo | We have no build yet; each needs approval, and their kits come with secrecy terms that clash with open source | Company policy, plus our budget |

Read the right-hand column carefully, because the three kinds are not the same.
A law we must obey. A contract we chose to accept by using someone's service, and
could in principle negotiate. A company policy is simply a business decision by a
platform holder, and those change when enough people push.

## Patents on video formats, and the good news

For twenty years, playing ordinary video meant paying patent holders. That is
why free software so often could not play the files everyone else could.

The good news is that this is expiring. The patents covering DVD video and its
audio ran out around 2017 and 2018, which is why a disc you burned yourself now
plays in free software with nothing owed to anyone. Some newer formats are still
encumbered, which is why we prefer the free ones and convert into them rather
than shipping a decoder we would have to license.

This one is fixing itself, slowly, through the simple passage of time.

## What is NOT stopping us

Worth stating, so nobody assumes the worst:

- Nothing stops you playing any file you already have, in any format, on any
  screen in the world. The app does that today.
- Nothing stops you playing a disc you made yourself.
- Nothing stops us showing a video platform's own player, with its own
  advertising, on a screen inside the world. That is permitted and we intend to
  do it.
- Nothing stops anyone streaming to a server you or we run, and being watched
  in world by anyone you invite. No third party is involved in that at all, and
  it is the direction we would rather grow.

## If you think a rule is wrong

The anti-circumvention rule is the one worth fighting, and there is a real
mechanism rather than just complaining.

**In the United States, the law has a built-in escape valve.** Every three
years, the Copyright Office runs a public process and can grant exemptions for
specific uses. Anyone may file, including a person with no lawyer, and it does
not cost money. Exemptions have been won this way for repairing tractors and
medical devices, for preserving video games, for accessibility, and for
educators making short excerpts. The process is slow, the exemptions are narrow
and must be re-won each cycle, and that structure is itself a fair criticism.

**What makes a filing land** is not indignation, it is evidence. A specific
class of works, a specific lawful use, a specific harm, and a real person it
happened to. "I own the discs, I paid for them, and I cannot watch them in
software I wrote myself" is exactly that shape of story, and it is stronger than
an abstract argument about freedom.

**What a better rule would look like**, if you are asked for a proposal:
circumvention should be unlawful when it is a step toward infringing, and lawful
when the underlying use is lawful. That single change would preserve every
protection rights holders actually need while ending the absurdity of a person
being forbidden from watching a film they own on a screen they own. Versions of
this have been introduced in Congress before and have not passed.

**Where to find allies:** the digital rights organisations that run the
exemption filings each cycle do this work for free and are usually glad of a
concrete case, and the right to repair movement has spent a decade making
exactly this argument about tractors and phones. It is the same rule and the
same fight.

## How to use this page

If you are a person who just wanted to watch a film: the short version is that
the app plays anything you can hand it as a file, and will not break a lock on a
disc, and that is a legal line rather than a technical one.

If you are talking to a lawyer or a representative: the sections above are
ordered so you can point at the rule, the harm, the mechanism and the proposed
change. The project operator is a real person with a real collection of discs he
paid for and cannot watch in his own software, which is the kind of specific,
documented case these processes are built around.

If you are a contributor: our own position, in detail, is in
[media-stance.md](../../reference/media-stance.md). When you decline to build
something for a legal reason, say which rule and say it plainly, here and on the
screen where someone hits it. Being told "no, and here is exactly why" is a
thing people can accept. Silence is not.
