# Beethoven's own metronome mark for the first movement of his Fifth Symphony

**Researched: 2026-11-20.** A question of historical fact, not of law; nothing here is legal advice.

**The question.** What metronome mark did Beethoven himself give the first movement (Allegro con
brio) of his Symphony No. 5, Op. 67, which the score player plays from
`data/music/scores/beethoven_symphony5_1.mei`?

## The short answer

- **A half note = 108.** Beethoven's table of metronome marks for all his symphonies so far, printed
  in the Leipzig *Allgemeine musikalische Zeitung* of 17 December 1817, gives the Fifth's first
  movement "Allº con brio" at 108 with a half note as the beat. The movement is in 2/4, so a half
  note is the whole bar: 108 bars, or 216 quarter notes, a minute. The score player's file already
  plays at that tempo (`midi.bpm="216"`, quarter notes a minute), so nothing in it changes.
- **The oboe's Adagio cadenza (bar 268) has no mark of Beethoven's.** The table gives the first
  movement one figure only; its next line is the second movement. The cadenza's 60 quarter notes a
  minute in the file is our own playback choice, not his.

## Findings

### 1. The table in the Allgemeine musikalische Zeitung, 17 December 1817

- **Source:** *Allgemeine musikalische Zeitung*, Neunzehnter Jahrgang (Leipzig, Breitkopf und
  Härtel), No. 51, 17 December 1817, columns 873 and 874, a full-page engraved table. Read as the
  Bavarian State Library's copy, digitised by Google and held by the Internet Archive:
  <https://archive.org/details/bub_gb_qRNDAAAAcAAJ> (the volume for 1817; the table is leaf 482 of
  500, between column 872 and column 875).
- **Source date:** printed 17 December 1817 (the issue's head dates it 17 December 1817 and numbers
  it 51; the volume's title page runs it from January to 24 December 1817). The scan was added to the Internet Archive on 2015-04-14; its catalogue record marks it
  with the Public Domain Mark 1.0.
- **Quotation.** The table's heading, as printed:

  > Die Tempo's sämmtlicher Sätze aller Symphonien des Hrn L. v. Beethoven, vom Verf. selbst nach
  > Maelzels Metronom bestimmt.

  ("The tempos of every movement of all the symphonies of Mr L. v. Beethoven, set by the composer
  himself by Maelzel's metronome.") Under "Symph. V. Op. 67." the first line reads:

  > Allº con brio 108 = [a half note]

  The note after the equals sign is engraved as an open note head with a stem: a half note. (The
  same table writes other beats as other notes, for instance the second movement's "Andante con
  moto. 92 =" with an eighth note, so the note value is a deliberate part of each mark.) Beside the
  mark the table engraves the movement's opening bars, the four-note motto in 2/4 with its two
  fermatas, which identifies the movement beyond doubt.

  How it was read: the Internet Archive's robots.txt (archive.org) was read first, in its own step;
  it disallows only `/control/` and `/report/`. The volume's plain-text recognition
  (`bub_gb_qRNDAAAAcAAJ_djvu.txt`) located the heading at column 873 (the recognition garbles the
  engraved table itself), and the page image of leaf 482, served by the Archive's book reader at
  `ia600605.us.archive.org`, was read to see the numbers and the note value. That image host has
  no robots.txt (HTTP 404, checked the same day, after the image was read).

## What is still unknown, and what would settle it

- **Whether a later Beethoven source revises the figure.** The 1817 table is the composer's own
  publication; the critical report of a modern scholarly edition of the symphonies would say
  whether any other source of his gives a different mark. Not needed for the score player: we follow the 1817 publication, which is his.
- **The separate leaflet.** The marks were also issued apart from the newspaper; that leaflet was
  not looked for, because the newspaper's own printing settles the figure.

## Sources you could not reach

None: no request failed. The separate leaflet and a critical edition's report were not tried
(above).

## What it means for this project

**Certain:**

- Beethoven's own mark for the first movement of the Fifth is a half note = 108, as printed on
  17 December 1817.
- In the movement's 2/4 that is 216 quarter notes a minute, which is what
  `beethoven_symphony5_1.mei` already gives its "Allegro con brio" (`midi.bpm="216"`) and what
  `data/music/scores/index.ron` gives the piece (`qpm: 216.0`). Nothing changes in the file, and
  the length test in `crates/humanity-music/src/tests_scores.rs` (worked out by hand at 216) stands.
- Beethoven's table gives no figure for the oboe's Adagio cadenza.

**Judgement call:**

- The cadenza's 60 quarter notes a minute (and the return to 216 after it) is a playback choice made
  by the project, and the file, the index and the design document say so in those words. Anyone may
  change it in the MEI file; there is no authority to follow there.
- The score player plays the Allegro at the composer's own figure because it is the documented
  source; any other tempo for it would be a choice of ours and would have to say so the same way.
