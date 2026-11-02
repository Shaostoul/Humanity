# Unfinished software: what it means, and why the warning is there

At the bottom of every page, in the desktop app and on the website, there is a
thin amber line:

> Unfinished software: it can be wrong, break, or lose your work. Do not rely on
> it for health, safety, food or water. It comes as is, with no warranty.

This page says what that line means for you: what "unfinished" means here, what
can go wrong, what stays yours all the same, what is not promised, why the line
is there, how to mute it for a while, how to tell us about a problem, and where
to see what comes next.

## What "unfinished" means here

HumanityOS is built in the open, every day, by one person and the AI helpers he
works with. It changes many times a week. That is how it gets better quickly,
and it is also why it is not finished. Each version is a step on the way to
version 1.0, and each one is unfinished in its own ways.

- **It can be wrong.** A number in the game, a step in a guide, a fact in the
  Library: any of them can be a mistake nobody has caught yet. Mistakes are found
  and fixed in public, and some are serious. In October 2026 a review found that
  some of the game's recipes taught unsafe food handling, such as canning
  low-acid food in a pot of boiling water, which cannot kill the spores that cause
  botulism. In another check, 48 of 60 researched plant and mushroom records were
  found wrong, almost all about which ones are poisonous and which look like them.
- **It can break.** A page can stop working after an update, or a feature can
  vanish while it is rebuilt.
- **It can lose your work.** See the next section.

## What can go wrong

- **Your saves and settings may not carry over.** Until version 1.0 the project
  changes the way it writes things down (its file formats) outright, instead of
  keeping every old way working, so a newer version may not open what an older
  one saved: games, settings, notes. Keep your own copy of anything you cannot
  afford to lose.
- **A server's world can be reset.** The shared worlds people play in together
  are kept on servers, and while the game is being built a server's world can be
  wiped and started again from the beginning.
- **A server can go away.** A server is a computer someone runs, and anyone can
  run one. It can shut down, lose what it held, or change its rules. Whatever
  lived only there goes with it: the messages in its rooms, the files you put on
  it, your progress in its world, and any private message still waiting there
  for you. (Even while a server keeps running, it holds a waiting message for a
  limited time, 30 days unless it sets another; one your device has not
  collected by then is gone.)
- **Something can work today and not tomorrow.** If you rely on a feature, check
  it after each update.

## What stays yours all the same

Some things are built in from the start, and an unfinished version does not
change them:

- **Your account is yours, on your own device.** There is no sign-up and no
  password stored on any server. Your account is a key that lives on your own
  device, and your 24-word recovery phrase can rebuild it on any other. If a
  server goes away, your account does not: you can use the same key on another
  server. Write the 24 words down on paper and keep them where only you can find
  them, because nobody, the people who build HumanityOS included, can reset
  them for you.
- **Your direct messages are sealed.** A private message to one person is sealed
  on your device before it leaves it, so the servers that carry it cannot read
  it, and the server does not keep who sent it. Only the person you wrote to can
  open it. (For those who want the names: the seal uses Kyber768, also called
  ML-KEM-768, then BLAKE3 and AES-256-GCM, and the message is signed inside the
  seal with your Dilithium3, also called ML-DSA-65, key. The project's
  [cryptography table](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/cryptography.md) lists every method it
  uses.)
- **Messages in rooms are not sealed.** What you say in a server's rooms can be
  read by that server and by everyone in the room. Say there only what you would
  say in a public place.

These describe how HumanityOS is built. They are not promises: unfinished
software can have mistakes here too, which is one reason every line of its code
is public, for anyone to check.

## What is not promised

### As is, with no warranty

A warranty is a promise from whoever made something that it works and is fit for
its use, with a way to put it right if it is not. HumanityOS makes no such
promise. It is given away free, to everyone, and you take it as it is. If it
turns out to be wrong or broken, nobody has promised to fix it for you, though we
do want to hear about it and usually do fix it.

This is what the project's licence already says. HumanityOS is released into the
public domain under CC0 1.0, which says:

> Affirmer offers the Work as-is and makes no representations or warranties of
> any kind concerning the Work, express, implied, statutory or otherwise,
> including without limitation warranties of title, merchantability, fitness for
> a particular purpose, non infringement, or the absence of latent or other
> defects, accuracy, or the present or absence of errors, whether or not
> discoverable, all to the greatest extent permissible under applicable law.

"Affirmer" is the person who released it. The whole licence is the project's
[LICENSE](https://github.com/Shaostoul/Humanity/blob/main/LICENSE) file.

To the extent the law allows, the people who make HumanityOS are not responsible
for loss or harm that comes from using it. That has a limit, and it is right
that it does: in some places the law does not let anyone escape responsibility
for death or injury caused by carelessness, and nothing on this page tries to.

### What not to rely on it for

Use what you learn here as a starting point, and check it against someone who
knows before you act on it. Never rely on HumanityOS alone for:

- **Your health:** medicine, first aid, symptoms, what a body needs.
- **Food:** whether something is fit to eat, how to preserve it, wild plants and
  mushrooms.
- **Water:** whether water is fit to drink, and how to make it so.
- **Your safety:** fire, electricity, gas, heating, building work, anything that
  can hurt you or someone else if it goes wrong.
- **Money**, and **your only copy** of anything that matters.

**In an emergency, call your local emergency number.** This app is not an
emergency service and cannot call one for you.

## Why the warning is there

Because it is true, and you should know it before you lean on anything here.
The warning is not a trick to dodge blame. It is there so you know, before you
lean on something, how much weight it can take. The licence already says the
software comes as is; the amber line makes sure nobody has to read a licence to
find that out.

It says what the software is and what not to rely on it for, and it does not
say "we cannot be held responsible for anything", because in some places that
would not be true. The research behind its words, with every law and licence it
read and the date it read them, is in the project's finding
[An unfinished-software warning](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-10-30-wip-warning-and-liability.md).
**It is not legal advice**, and it says a lawyer should read the warning before
version 1.0.

## Muting it for a while

Once you have read the line, you may not want it on every page. You can mute it:

- **Where:** press **Mute**, beside "What this means" on the line itself, or go to
  **Settings > Notifications > Muted**, in the desktop app or on the website.
- **For how long:** for this session (until you close HumanityOS; on the website,
  until you close the tab), for one day, or for one week.

What muting does: the line is hidden on every page for that long, and the room it
took at the bottom of the screen is given back to the page.

What muting does not do:

- **It changes nothing else.** Everything the line says is still true while it is
  hidden.
- **It does not last past a new version.** Each version is unfinished in its own
  ways, so the first time you open a new version the line is back.
- **It does not hide it everywhere.** The title screen of the desktop app (on the
  website, the front page) and this page always show it, mute or not. The title
  screen is the first thing you see each time you open HumanityOS, and keeping it
  there keeps things honest.
- **It is not shared.** A mute is kept on this device only: in the desktop app's
  own settings, or in this browser. It is never sent to a server.

To bring it back sooner, press **Muted** on the line where it still shows (the
title screen, the front page or this page) and choose **Show it again**, or press
**Show it again** in **Settings > Notifications > Muted**.

## When it goes away

At version 1.0. The line shows on every version before that, and the app decides
it from its own version number, so it cannot be left switched on, or switched
off for good, by a setting someone forgot. The version you are running is at the
end of the line.

Version 1.0 will not mean perfect. What it does promise, and what it still does
not, will be said plainly when it comes.

## How to tell us about a problem

The **Bugs** page, under Dev in the app and on the website, files a report on the
server you use; reports filed on united-humanity.us reach the people building
HumanityOS. Say what you did, what you expected and what happened instead. A
wrong fact about food, water, health or safety is the most useful report of all.

## What comes next, and the road to 1.0

- **The roadmap** says what is built, what is being built now and what is still
  to come before 1.0: the [Roadmap](/library#roadmap) in the Library, and
  [united-humanity.us/roadmap](https://united-humanity.us/roadmap) on the website.
- **Everything is public.** The code, the plans and every change are on
  [GitHub](https://github.com/Shaostoul/Humanity), and the project's own copy is
  at git.united-humanity.us.
- **Shortfalls are written down.** The places the software still falls short of
  its own promises are listed in
  [Where the Software Falls Short](/library#conformance-gaps), and the
  things it will not do because a law or a platform forbids them are in
  [The Laws That Limit This Software](/library#laws-that-limit-this-software).
