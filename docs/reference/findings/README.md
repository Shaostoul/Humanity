# Findings

Dated research notes on questions with a legal, regulatory or licensing edge.
One file per question, written the day the research was done, never quietly
updated afterwards.

## Why this folder exists

The operator's reason, in his own words:

> If there's something legal or similar then we should research it and create a
> document of what we find, whether it's ok or not. That way when we inevitably
> have to discuss the subject again we can see where we found the information and
> what we found. We should also date our findings as to clearly identify the last
> time we looked. That way if something changes and we're approached we can see
> the date we researched and compare it to the date that the change occurred.

The point is not to be right forever. It is to make it possible, later, to tell
**which kind of wrong we were**. When a question comes back around, a reader
with one of these files in hand can separate two very different situations:

- **The world changed.** The patent expired, the pool restructured its rates, a
  platform changed its terms. Our finding was correct when written and is now
  stale. The date proves it, and the fix is to research again.
- **We were wrong.** The source said something we misread, or we never found the
  source that mattered. The quotations prove it, because a later reader can go
  and read the same words we read.

Without the date, those two look identical, and every old conclusion has to be
re-derived from nothing. With it, most of them can just be re-checked.

There is a practical edge to this too. If someone ever writes to the project
about a codec, a format or a protection scheme, the reply can say exactly what
we looked at, exactly when, and what we concluded. That is a much better
position than remembering.

## File naming

```
docs/reference/findings/YYYY-MM-DD-short-topic.md
```

The date is the date the **research** was done, not the date of any later edit.
It goes first so the folder sorts chronologically.

Do not overwrite an old finding when the answer changes. Write a new file with
the new date and, in it, link back to the one it supersedes and say plainly what
moved. The old file stays: it is the record of what we believed and why, and it
is the thing that lets a later reader date a change.

## Required parts, in this order

1. **The question**, in one sentence. If it takes a paragraph, it is more than
   one finding.
2. **The research date, stated prominently at the top**, before anything else a
   reader might act on.
3. **The short answer**, up front. A reader in a hurry should be able to stop
   after this section and not be misled.
4. **Each finding, separately**, carrying three things every time: the source
   link, the date that source was published or last updated, and a quotation of
   the words that carry the meaning.
5. **What is still unknown, and what would settle it.** Name the specific thing
   that would answer each open question, and what it would cost. "Ask a lawyer"
   is a legitimate answer; "unclear" on its own is not.
6. **Sources you could not reach.** Every 404, paywall and block, by URL, with
   what you were hoping to get from it. This is not an apology, it is a
   work-list for whoever researches next.
7. **What it means for this project**, split into what is **certain** and what
   is a **judgement call**. Keep these apart. A later reader must be able to
   accept the facts and reject the opinion.

## The rules

**Quote, do not paraphrase, and never from memory.** Open the source, read the
words, copy them exactly. A summary is your reading of a source; a quotation is
the source. When a finding turns out to be wrong, the quotation is what lets a
reader see whether the source was wrong or we were. Copyright limits apply, so
quote the passage that carries the meaning and no more, and reproduce quoted
text exactly as written, punctuation included.

**Date every source, not just the document.** "Retrieved 2026-09-19" is not the
same as "published 2013" and the gap between them is often the whole story. A
licence written in 2014 and a patent table edited three days ago carry very
different weight.

**Where sources disagree, show both.** Do not resolve a conflict silently in
favour of the one you prefer. Quote each side, say which you find more credible
and why, and label that as a judgement. A reader who disagrees with your
weighting can then reach their own conclusion from the same material.

**Say you are not a lawyer, plainly, and more than once.** These documents get
read later, out of context, by people who did not commission them. The
disclaimer belongs at the top and at the bottom, not buried.

**Record the answer even when it is "no".** A researched "no, and here is
exactly why" is worth as much as a yes, because it stops the question being
re-opened from scratch every year.

**Do not edit a finding to make it look better later.** Correct an outright
factual error, with a dated note saying what was corrected. Everything else goes
in a new file.

## What belongs here, and what does not

Findings answer *may we?* and *what is actually true out there?*. They are
research about the world outside the repository.

They are not design documents, which answer *how shall we build it?* and live in
`docs/design/`. They are not public stances, which answer *what do we tell
people?* and live in `docs/reference/` proper, such as
[`media-stance.md`](../media-stance.md). A finding usually comes first and feeds
the other two: research the ground, then decide what to build, then write down
what we will say about it.

## Index

- [`2026-09-19-video-codec-licensing.md`](2026-09-19-video-codec-licensing.md),
  whether a free, open-source, donation-funded project can lawfully play H.264
  video and AAC audio. Covers the Cisco OpenH264 grant, remaining patent expiry
  dates, calling the operating system's own decoders, Via LA's published rates,
  and what other free projects did.
- [`2026-09-25-site-embed-terms.md`](2026-09-25-site-embed-terms.md),
  whether the 34 sites awaiting review in `data/web/sites.json` allow the
  readable web to fetch, reformat and show their pages. One allowed, 21
  conditional (mostly attribution), seven forbidden as written, four silent,
  one gone. Evidence for the person who records each site's `embed.status`.
- [`2026-09-25-permission-routes-and-launchers.md`](2026-09-25-permission-routes-and-launchers.md),
  where to ask each site that forbids the reader for permission (GOG,
  Examine, Coursera, Discord, Project Gutenberg, Khan Academy, Instructables,
  Element, iFixit), and whether HumanityOS could act as a game launcher
  (GOG and Steam: contacts, APIs, affiliate programme, Playnite and Heroic).
