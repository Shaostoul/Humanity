# Composing in a genre: what copyright protects in music, what it leaves free, and where a genre engine must stop

**Research date: 12 November 2026** (Pacific time). Every source below was read that day. Each one
carries its own published, amended or revised date beside it, which is often years older. If you
are reading this later, compare those dates with whatever has changed since.

**I am not a lawyer. This is a reading of public sources by an AI working on the project, not legal
advice.** Nobody qualified has reviewed it. Statutes, agency manuals and court opinions are texts
whose reading a court could disagree with, and the court opinions below bind only the courts below
them.

Related: [`2026-11-06-music-sources.md`](2026-11-06-music-sources.md) (which recordings and models
the app may ship) and the design, [`docs/design/music.md`](../../design/music.md), section 7a.

## The question

May HumanityOS's procedural music engine compose live music in the electro swing genre (the
operator named the band Caravan Palace as an example of it) from the genre's general
characteristics, and where exactly is the line it must not cross?

## Short answer

**Conclusion.** Yes, the genre may be composed from its general characteristics. US copyright
protects two things in music: the musical work (the composition, words included) and the sound
recording. It does not protect any "idea, procedure, process, system, method of operation, concept,
principle, or discovery" (17 U.S.C. 102(b)). The Copyright Office will not register scales,
arpeggios, "common chord progressions that merely consist of standard harmonies or common musical
phrases", or short musical phrases. The Ninth Circuit, sitting en banc (the federal appeals court
whose rulings bind the federal courts in Washington State), held that copyright does not reach
"commonplace elements that are firmly rooted
in the genre's tradition" and called them "building blocks" that "belong in the public domain". So
the genre's ingredients as the engine uses them (a tempo of 128 bpm, swung eighths, a
four-on-the-floor kick, a walking bass built from chord tones, ii-V-i minor harmony, brass stabs,
clarinet lines, call and response) are free for anyone to use. **No source read says in so many
words that "a genre" or "a tempo" cannot be owned;** that reading rests on 102(b), the Copyright
Office's common-property lists, and the court opinions below, and is a judgement, though a well
supported one.

**What is NOT allowed (the negative finding):**

- **Copying any recording's actual sounds** (a sample, a loop, a stem, a drum hit lifted from a
  track) without the owner's permission or a free licence such as CC0. The sound recording owner's
  right covers copies "that directly or indirectly recapture the actual sounds fixed in the
  recording" (17 U.S.C. 114(b)), and the Copyright Office notes that more than a de minimis
  recognizable sample "may be questioned for lawful inclusion".
- **Copying any band's melodies, lyrics, riffs or hooks**, even short ones and even re-styled.
  The Ninth Circuit: "a pattern of notes in a tune may earn copyright protection", and an
  "eight-note melody may be copyrightable". Re-doing an existing song in another style is still an
  adaptation of it, which the Copyright Office describes as a reworking "that changes the style or
  genre of that work".
- **Reproducing a particular song's distinctive combination of elements** (its bass line together
  with its keyboard part, its signature phrases, its arrangement). Elements that are each free can
  be protected in an original combination, and in *Williams v. Gaye* (the "Blurred Lines" case) a
  jury found infringement, largely upheld on appeal, although the dissenting judge found the two
  songs shared only a "groove" or genre. The engine must generate from rules, never from
  transcriptions or analyses of any one band's tracks.
- **Using a band's name in the app or its promotion in a way that suggests the band made, approved
  or is connected with the music.** The federal false-endorsement provision (15 U.S.C. 1125(a))
  reaches a name used in commerce that is likely to cause confusion "as to the affiliation,
  connection, or association" or "sponsorship, or approval". Naming the genre ("Electro swing") is
  fine; "sounds like Caravan Palace" in the interface or marketing is not worth the risk. Whether a
  free, donation-funded project acts "in commerce" was not researched.

**Worldwide.** The Berne Convention protects "musical compositions with or without words" in every
member country without registration ("shall not be subject to any formality"), so a band's songs
are protected wherever the app runs. The WIPO Copyright Treaty states the same line the US draws:
protection "extends to expressions and not to ideas, procedures, methods of operation or
mathematical concepts as such." National laws outside the US were not read.

## The findings

### 1. What copyright protects in music: 17 U.S.C. 102(a)

- **Source:** 17 U.S.C. 102, Cornell Legal Information Institute's copy of the official text,
  <https://www.law.cornell.edu/uscode/text/17/102> (the official House site could not be read; see
  below).
- **Source date:** enacted by Pub. L. 94-553, 19 October 1976; last amended by Pub. L. 101-650,
  1 December 1990 (the page's source credit). The page shows no "current through" date.
- **Quotations:**
  - "Copyright protection subsists, in accordance with this title, in original works of authorship
    fixed in any tangible medium of expression"
  - Among the listed categories: "(2) musical works, including any accompanying words;" and
    "(7) sound recordings; and"

So there are two separate rights in every released song: the composition (melody, harmony, rhythm,
words) and the recording of it.

### 2. What copyright does not protect: 17 U.S.C. 102(b)

- **Source:** the same page, <https://www.law.cornell.edu/uscode/text/17/102>.
- **Source date:** as above (1976, last amended 1990).
- **Quotation:** "In no case does copyright protection for an original work of authorship extend
  to any idea, procedure, process, system, method of operation, concept, principle, or discovery"
  (the sentence continues: "regardless of the form in which it is described, explained,
  illustrated, or embodied in such work.")

### 3. The Copyright Office on musical works: Compendium, Third Edition, chapter 800

- **Source:** Compendium of U.S. Copyright Office Practices, Third Edition, Chapter 800, "Works of
  the Performing Arts", <https://www.copyright.gov/comp3/chap800/ch800-performing-arts.pdf> (a
  116-page PDF; its text was extracted locally to read it).
- **Source date:** every page is footed **01/28/2021**.
- **Quotations, by section:**
  - 802.2(B), two works: "A musical work and a sound recording of that musical composition are
    separate works."
  - 802.3, what authorship in music is made of: "The main elements of copyrightable musical work
    authorship include melody, rhythm, harmony, and lyrics, if any."
  - 802.5(A), common property: "a musical work consisting entirely of common property material
    would not constitute original authorship." Its examples: "Diatonic or chromatic scales.",
    "Arpeggios." and "Chord symbols based on standard chord progressions."
  - 802.5(B), short phrases: "short musical phrases are not copyrightable because they lack a
    sufficient amount of authorship"
  - 802.6(C)(1), harmonizing a melody must be original, "not simply standard chords in common
    sequences such as C, F, G, C."
  - 802.6(D), adaptations of an existing work (the negative side): an adaptation "may involve a
    reworking of the melody, rhythm, harmony, and/or lyrics in a preexisting musical work that
    changes the style or genre of that work."
  - 802.6(G), samples: "use of more than a de minimis amount of recognizable sample may be
    questioned for lawful inclusion."
  - 802.5(C), a side note for this engine: "Nor could a musical composition created solely by a
    computer algorithm be registered."

Chapter 800 nowhere says that a musical style, a genre or a tempo is unprotectable; "style" and
"genre" appear only in 802.6(D) above and in the compulsory-licence passage. Rhythm is listed as an
element of authorship (802.3), so a rhythm can be part of a protected melody; no passage protects a
common rhythmic pattern on its own.

### 4. The Copyright Office on ideas and common property: Compendium chapter 300

- **Source:** Compendium, Third Edition, Chapter 300, "Copyrightable Authorship",
  <https://www.copyright.gov/comp3/chap300/ch300-copyrightable-authorship.pdf> (text extracted
  locally).
- **Source date:** every page is footed **01/28/2021**.
- **Quotations:**
  - 313.4(D): "the Office cannot register a claim in common sayings, diatonic and chromatic musical
    scales, or common chord progressions that merely consist of standard harmonies or common
    musical phrases."
  - 313.3(A), examples of what cannot be registered: "Ideas, procedures, processes, or methods for
    doing, making, or building things." and "Mathematical principles, formulas, algorithms, or
    equations."
  - 312.2 (compilations), an example of a routine arrangement: "arranging notes in standard scales
    or in standard melodic or harmonic intervals, or a musical work consisting solely of a musical
    scale(s)."

### 5. Copyright Office circulars

- **Circular 33, "Works Not Protected by Copyright"**, <https://www.copyright.gov/circs/circ33.pdf>,
  **revised 03/2021**. Its worked example, about a book describing a food-processing system: the
  copyright "does not give the author the right to prevent others from employing the system or from
  using any procedures, processes, or methods described in the book." Circular 33 does not mention
  music except to list "Musical notation" among familiar symbols.
- **Circular 50, "Copyright Registration for Musical Compositions"**,
  <https://www.copyright.gov/circs/circ50.pdf>, **revised 08/2023**: "A musical composition and a
  sound recording are two separate works." Also: "Do not include elements that are not protected by
  copyright, such as ideas, concepts, names, or titles."
- **Circular 56A, "Copyright Registration of Musical Compositions and Sound Recordings"**,
  <https://www.copyright.gov/circs/circ56a.pdf>, **revised 03/2021**: "Sound recordings and musical
  compositions are considered two separate works for copyright purposes."
- Circular 56 (sound recordings alone) was not read; 56A covered the point needed.

### 6. The sound recording: 17 U.S.C. 114(b)

- **Source:** 17 U.S.C. 114, <https://www.law.cornell.edu/uscode/text/17/114>.
- **Source date:** the section's latest amendment is Pub. L. 115-264, 11 October 2018 (the page's
  statutory history). No "current through" date shown.
- **Quotations (subsection (b)):**
  - The owner's reproduction right "is limited to the right to duplicate the sound recording in the
    form of phonorecords or copies that directly or indirectly recapture the actual sounds fixed in
    the recording."
  - The owner's rights "do not extend to the making or duplication of another sound recording that
    consists entirely of an independent fixation of other sounds, even though such sounds imitate
    or simulate those in the copyrighted sound recording."

So a sample copies the recording. Sounds the engine makes itself are not a copy of any recording,
even when they resemble a recorded timbre; the composition's copyright is a separate question
(findings 3, 8 to 10).

### 7. The genre's building blocks: *Skidmore v. Led Zeppelin* (Ninth Circuit, en banc)

- **Source:** *Skidmore v. Led Zeppelin*, No. 16-56057, 952 F.3d 1051 (9th Cir. 2020) (en banc),
  <https://cdn.ca9.uscourts.gov/datastore/opinions/2020/03/09/16-56057.pdf> (text extracted
  locally).
- **Source date:** filed **9 March 2020**. Its later history was not checked.
- **Quotations:**
  - Copyright does not extend to "commonplace elements that are firmly rooted in the genre's
    tradition," (the court takes these words from Judge Nguyen's dissent in *Williams v. Gaye*,
    finding 9).
  - "These building blocks belong in the public domain and cannot be exclusively appropriated by
    any particular author."
  - The jury instruction the court upheld: copyright "does not protect ideas, themes or common
    musical elements, such as descending chromatic scales, arpeggios or short sequences of three
    notes."
  - "Jury Instruction No. 16 correctly listed non-protectable musical building blocks that no
    individual may own"

### 8. Common elements and where protection starts: *Gray v. Hudson* (the "Dark Horse" case)

- **Source:** *Gray v. Hudson*, No. 20-55401 (9th Cir. 2022),
  <https://cdn.ca9.uscourts.gov/datastore/opinions/2022/03/10/20-55401.pdf> (text extracted
  locally).
- **Source date:** filed **10 March 2022**. Its later history was not checked.
- **Quotations (free elements):**
  - "Plainly, no person may copyright the minor scale, as such scales are common musical building
    blocks belonging to the public."
  - Quoting an earlier case: "chord progressions may not be individually protected"
  - Quoting a district court: "the use of a long-short-long rhythm is too general to be
    protectable"
- **Quotations (the line not to cross):**
  - "While an eight-note melody may be copyrightable, the abstract eight-note pitch sequence that
    is a component of the melody is not."
  - "the fact that the ostinatos here are only eight notes long does not foreclose the possibility
    of a protected arrangement of commonplace musical elements."
  - Quoting an earlier case: "Each note in a scale . . . is not protectable, but a pattern of notes
    in a tune may earn copyright protection."

### 9. The caution: *Williams v. Gaye* (the "Blurred Lines" case)

- **Source:** *Williams v. Gaye*, No. 15-56880, 895 F.3d (9th Cir. 2018), order and amended
  opinion, <https://cdn.ca9.uscourts.gov/datastore/opinions/2018/07/11/15-56880.pdf> (text
  extracted locally).
- **Source date:** filed **21 March 2018, amended 11 July 2018**.
- **What happened (from the opinion):** a jury found that "Blurred Lines" infringed Marvin Gaye's
  "Got To Give It Up"; the panel affirmed most of the judgment on narrow, procedural grounds. The
  work was a 1909 Act composition judged by its deposited sheet music, not its recording.
- **Quotations, majority:**
  - "Our decision does not grant license to copyright a musical style or "groove.""
  - "Musical compositions are not confined to a narrow range of expression." (So a later song need
    only be substantially similar to infringe, not virtually identical.)
- **Quotations, dissent (Judge Nguyen; a dissent is not the court's holding):**
  - "The majority allows the Gayes to accomplish what no one has before: copyright a musical
    style."
  - ""Blurred Lines" clearly shares the same "groove" or musical genre as "Got to Give It Up,"
    which everyone agrees is an unprotectable idea."
  - "two works in the same genre must share at least some protectable expression in order to run
    afoul of copyright law."

Both sides agree a genre or groove is not protected. The case still shows the practical risk: when
a new piece is built while studying one particular song, a jury can find that its combination of
elements took protected expression, and an appeals court may not disturb that.

### 10. The international baseline: the Berne Convention and the WIPO Copyright Treaty

- **Berne Convention for the Protection of Literary and Artistic Works**, WIPO Lex,
  <https://www.wipo.int/wipolex/en/text/283698>. **Source date:** the Paris Act of 24 July 1971 as
  amended on 28 September 1979 (the page's title). **Quotations:** Article 2(1) counts as protected
  works "every production in the literary, scientific and artistic domain, whatever may be the mode
  or form of its expression," including "musical compositions with or without words;". Article
  5(2): "The enjoyment and the exercise of these rights shall not be subject to any formality;"
- **WIPO Copyright Treaty**, WIPO Lex, <https://www.wipo.int/wipolex/en/text/295166>. **Source
  date:** "adopted in Geneva on December 20, 1996". **Quotation,** Article 2: "Copyright
  protection extends to expressions and not to ideas, procedures, methods of operation or
  mathematical concepts as such."

### 11. Naming a band: 15 U.S.C. 1125(a) (false designation and endorsement)

- **Source:** 15 U.S.C. 1125, <https://www.law.cornell.edu/uscode/text/15/1125>.
- **Source date:** latest amendment Pub. L. 112-190, 5 October 2012 (the page's source credit).
- **Quotations (subsection (a)(1)):** "Any person who, on or in connection with any goods or
  services, or any container for goods, uses in commerce any word, term, name, symbol, or device,
  or any combination thereof," and, at (A), "is likely to cause confusion, or to cause mistake, or
  to deceive as to the affiliation, connection, or association of such person with another person,"
  then "or as to the origin, sponsorship, or approval of his or her goods, services, or commercial
  activities by another person"

### The electro swing genre's elements, one by one

| Element in `data/music/genres/electro_swing.ron` | What the sources say | Standing |
| --- | --- | --- |
| Tempo, 128 bpm | No source read mentions tempo at all | Free by 102(b) reasoning: a judgement |
| Swung eighths, four-on-the-floor kick, clap on 2 and 4, off-beat open hat | Common rhythms not protected on their own (*Gray*, quoting a district court; *Skidmore* "building blocks"); rhythm IS an element of a protected melody (Compendium 802.3) | Free as patterns; a judgement for "common", certain that a melody's own rhythm can be protected |
| Minor key, harmonic minor, blues scale | Scales are common property (Compendium 802.5(A), 313.4(D)); "no person may copyright the minor scale" (*Gray*) | Free: certain |
| ii-V-i, diminished passing chords, secondary dominants | "common chord progressions that merely consist of standard harmonies" (313.4(D)); "chord progressions may not be individually protected" (*Gray*) | Free: certain for standard progressions |
| Walking bass from chord tones and passing notes | No source on walking bass as such; the *Williams* dissent calls a standard bass line one that "merges with the idea" (a dissent) | Free as a method: a judgement; a specific band's bass line is not free |
| Brass stabs, clarinet lines, gypsy-jazz comping (instrument choice) | Not addressed for genre conventions; the Office protects new instrumentation of an existing work (802.6(C)(2)) | Choosing genre-typical instruments: a judgement, free; orchestrating someone's song: not free |
| Call and response | Not addressed; a form or technique is a "method" or "system" (102(b)) | Free: a judgement |
| The melodies the engine improvises | Algorithm output is not registrable (802.5(C)); any melody too close to an existing one infringes it (*Gray*, *Williams*) | The engine's rules must never encode an existing tune |

## What is still unknown, and what would settle it

- **Whether a genre or tempo is "unprotectable" in so many words.** No statute, Copyright Office
  text or holding read says it. The *Williams* majority disclaims protecting "a musical style or
  "groove"", and *Skidmore*'s "genre's tradition" line is borrowed from a dissent. Settled only by a
  holding that says it; an IP lawyer's opinion letter would say how much weight the present sources
  carry. Cost: a lawyer's hour or two.
- **How close the engine's improvised melodies might come to an existing song by chance.** The law
  protects against copying, and independent creation is a defence, but chance resemblance is not
  measured. Settled by an engineering check, not a legal one (for example, never seeding the engine
  with any existing melody, and keeping the generating rules documented in the repository as
  evidence of independent creation).
- **Imitating a particular singer's voice or a performer's signature sound.** That is state law
  (right of publicity) and not copyright; it was not researched. The engine has no vocals today.
  A separate finding before any vocal or voice feature imitates a person.
- **Whether a free, donation-funded project uses a name "in commerce"** for 15 U.S.C. 1125(a). Not
  researched; avoiding band names in shipped words makes it moot.
- **Fair use and the de minimis doctrine for samples.** Not researched, and not relied on: the
  project's rule is to sample nothing that is not free to share.
- **Laws outside the US** (the EU, the UK, France and others), including neighbouring rights in
  recordings (the Rome Convention, the WIPO Performances and Phonograms Treaty). Not read. Berne and
  the WCT give the shared baseline above.
- **Later history of the three court cases** (any Supreme Court petition). Not checked; a docket
  search settles it in minutes.

## Sources that could not be reached

- <https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title17-section102&num=0&edition=prelim>,
  the same for section 114, and
  <https://uscode.house.gov/view.xhtml?req=(title:17%20section:102%20edition:prelim)>: each
  returned HTTP 404 (its `robots.txt` allows reading). Hoped for: the official text with its
  "current through" line. Cornell's copy was used instead; a later reader should compare it with the
  House site once the right URL is found.
- <https://www.copyright.gov/comp3/chap800/> (the chapter's index page): HTTP 403. The chapter PDF
  itself was read. copyright.gov has no `robots.txt` (HTTP 404).
- Circular 56 (sound recordings alone): not attempted; 56A covered the point.

## What it means for this project

**Certain (from the sources):**

- A released song carries two copyrights: its composition and its recording (102(a), Compendium
  802.2(B), Circulars 50 and 56A).
- Ideas, methods, systems and concepts are not protected (102(b); WCT Article 2 internationally).
- Scales, arpeggios, standard chord progressions and short musical phrases cannot be registered
  (Compendium 802.5(A), 802.5(B), 313.4(D)); the Ninth Circuit treats common musical elements as
  public-domain building blocks (*Skidmore*, *Gray*).
- Copying a recording's actual sounds is a matter for the recording's owner (114(b)); making new
  sounds that imitate a recorded sound is not a copy of the recording (114(b)).
- A melody, even a short one, and an original arrangement of common elements can be protected
  (*Gray*), and re-styling an existing song is an adaptation of it (Compendium 802.6(D)).
- A band's songs are protected worldwide without registration (Berne Article 5(2)).
- The player-facing words for this genre (`music.genre.electro_swing` and `_about` in
  `data/i18n/en.json`) name the genre and its ingredients, not a band. The band's name appears only
  in a source comment in `data/music/genres/electro_swing.ron` that quotes the operator, which is a
  record, not a claim of connection.

**Judgement calls (accept the facts above and reject these if you disagree):**

- The electro swing genre as written in `electro_swing.ron` (128 bpm, swing at 0.64 of the beat,
  house drums, walking or two-feel bass, la pompe comping, brass stabs, clarinet call and brass
  response, minor-key jazz harmony) is a description of shared conventions, and composing from it
  is lawful. The data file's own comment, "a style of music is not" copyrighted, is a fair summary of
  the sources, though no source states it that bluntly.
- The engine's safety lies in HOW it composes: from rules and probabilities written in our data,
  never from transcriptions, MIDI files, stems or analyses of any band's tracks. Keep it that way,
  and keep any future genre's data free of quoted melodies or riffs, however short.
- Keep band names out of shipped words and promotion ("electro swing", not "like Caravan Palace").
  Writing the band's name in research notes and design history, as here, is fine.
- `docs/design/music.md` section 7a says the app "never samples copyrighted recordings, because
  copyright law forbids it". More precisely, copyright law forbids copying a recording's sounds
  without the owner's permission (114(b)); with a free licence such as CC0 it is allowed, which is
  exactly the project's practice. The sentence is a fair plain-words version; a later edit could
  add "without permission".
- That the engine's own output cannot be registered (Compendium 802.5(C)) changes nothing for a CC0
  project; see the 2026-11-06 music-sources finding on CC licences and machine-made music.

**Again: I am not a lawyer, and this is not legal advice.** It is a dated reading of the sources
quoted above, written so a later reader can check every claim against the same words.
