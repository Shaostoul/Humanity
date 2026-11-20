# Public-domain scores: which editions and score files of Grieg's Mountain King and Beethoven HumanityOS may use

**Research date: 20 November 2026** (UTC; the evening of 19 November in Pacific time). Every
statute, licence and catalogue entry below was read that day from its own page, never from memory.
**This is not legal advice**: it is an AI's reading of each source, written down with the quotation
that carries the meaning so a person, or a lawyer, can check it. No score file (PDF, LilyPond, MIDI,
MEI, MusicXML or MuseScore) was downloaded for this research: catalogue pages, licence files and
READMEs were read, and file sizes were taken from the catalogues or from a HEAD request, which sends
no file.

This is step 2 of the goal for the usage reset of 2026-11-20 ([`docs/PRIORITIES.md`](../../PRIORITIES.md),
"GOAL FOR THE NEXT USAGE RESET"). It settles the legal questions that section 4 of the score-format
research left open ([`mei-and-humdrum.md`](../research/2026-11-16-score-formats/mei-and-humdrum.md),
section 4, "What the dated legal finding (step 2) must settle", and section 8, question 7). The
project's own licences are in [`LICENSING.md`](../../../LICENSING.md) (since 2026-11-14: code AGPL-3.0,
Library and docs CC BY-SA 4.0, data CC0; the finding behind that is
[`2026-11-14-licence-change.md`](2026-11-14-licence-change.md)).

## The question

Which editions and ready-made score files of Grieg's "In the Hall of the Mountain King" and of
Beethoven's works may HumanityOS encode from or import, from where, and with what obligations?

## Short answer

- **The music itself is free everywhere.** Grieg died on 4 September 1907 and Beethoven on 26 March
  1827. The Berne Convention's minimum term is the author's life plus 50 years and the EU's is life
  plus 70; no term found in this research comes near 119 or 199 years.
- **Old editions are free too, in the United States without any doubt.** The US Copyright Office
  (Circular 15a, revised April 2026): "all works published in the United States before January 1,
  1931, are in the public domain." The orchestral Mountain King was first published by C.F. Peters,
  Leipzig, in 1888; the Breitkopf & Härtel complete Beethoven appeared in 1862 to 1890. A foreign
  edition that old cannot get a US term back: restoration only covers works not already in the public
  domain at home "through expiration of term", and only for "the remainder of the term".
- **Modern critical editions are the trap.** The EU lets member states protect "critical and
  scientific publications" of public-domain works for up to 30 years; Germany does so for 25 years
  (UrhG section 70). In the US, "musical editing" such as "fingering, accents, dynamics" can be
  registered. So HumanityOS encodes only from editions published before 1931, never from a modern
  Urtext (Henle 1976, Grieg's *Samlede verker* of 1985 and 1988, Breitkopf's 1995 Urtext), even when
  their own 25-year German right has run out.
- **Copying the notes by hand adds no rights and takes none.** The US Copyright Office: "Transcribing
  or fixing a musical work in and of itself does not constitute authorship." Encoding the 1888 Peters
  full score in MEI is lawful, and the result can be dedicated CC0 like the rest of `data/`.
- **Ready-made files:** Mutopia's piano "In the Hall of the Mountain King" (Grieg's own piano
  arrangement, The University Society 1918) is public domain; so are Mutopia's Für Elise and its full
  orchestral score of Beethoven's Symphony No. 5, first and second movements (both from Breitkopf &
  Härtel). OpenScore's quartets and songs are CC0 (credit requested, not required) and hold 11
  Beethoven string quartets and the Grosse Fuge, but no Mountain King. TROMPA's Beethoven MEI files are CC BY 4.0 (credit
  required), and five of them follow the 1976 Henle Urtext: avoid those five. **No orchestral Mountain
  King encoding with a usable licence was found**: that one is ours to encode, from the 1888 Peters
  first edition.
- **IMSLP's scans may be used, but not fetched by an agent.** IMSLP marks its 1888 Peters scans
  "Public Domain", but its robots.txt bars automated readers from its file download handler, so the
  operator (or another person) downloads the scan in a browser.
- **Recommended order:** (1) Mutopia's piano Mountain King, (2) Mutopia's Für Elise, cross-checked
  against TROMPA's MEI of the same 1888 edition, (3) the orchestral Mountain King, encoded by us from
  the 1888 Peters full score, (4) Mutopia's Symphony No. 5, first movement. All four can ship in the
  core set with no credit duty; only a TROMPA file would bring one (a row in `data/credits.ron` and in
  `LICENSES.md`, and its licence beside the file).

## The findings

### A. The compositions

**1. Grieg died on 4 September 1907.** Bergen Public Library, *Edvard Grieg 1843-1907*
(https://bergenbibliotek.no/grieg/english/bio2-eng; the page carries "sist endret 2016-09-17" and
"Oppdatert 08.02.07"): "The frail, exhausted body can take no more; on the 4th of September 1907,
Edvard Grieg’s life ends."

**2. Beethoven died on 26 March 1827.** Beethoven-Haus Bonn, *Chronology Beethoven in Vienna*
(https://www.beethoven.de/en/g/zeittafel-beethoven-in-wien; no date shown), under the year 1827:
"Beethoven dies on 26 March in the "Schwarzspanierhaus" in Vienna." Its page *Beethoven*
(https://www.beethoven.de/en/beethoven): "Ludwig van Beethoven was born in Bonn in 1770. He lived in
his hometown till 1792. The rest of his life he spent in Vienna, where he died in 1827."

**3. The Berne Convention's term**, Paris Act 1971 as amended 1979, Article 7, WIPO Lex
(https://www.wipo.int/wipolex/en/text/283698): "(1) The term of protection granted by this Convention
shall be the life of the author and fifty years after his death." And Article 7(8): "In any case, the
term shall be governed by the legislation of the country where protection is claimed; however, unless
the legislation of that country otherwise provides, the term shall not exceed the term fixed in the
country of origin of the work."

**4. The EU (and EEA) term**, Directive 2006/116/EC, consolidated text of 31 October 2011, EUR-Lex
(https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:02006L0116-20111031), Article 1(1):
"The rights of an author of a literary or artistic work within the meaning of Article 2 of the Berne
Convention shall run for the life of the author and for 70 years after his death, irrespective of the
date when the work is lawfully made available to the public." Article 8: "The terms laid down in this
Directive shall be calculated from the first day of January of the year following the event which
gives rise to them." Germany's own Act, section 64 (https://www.gesetze-im-internet.de/urhg/__64.html):
"Das Urheberrecht erlischt siebzig Jahre nach dem Tode des Urhebers."

### B. Editions, engravings and scans

**5. The US cutoff is 1 January 1931.** US Copyright Office, Circular 15a, *Duration of Copyright*
(https://www.copyright.gov/circs/circ15a.pdf; "REVIEWED: 08/2011 REVISED: 04/2026"): "Thus the maximum
total term of copyright protection for works already protected by January 1, 1978, has been increased
from 56 years (a first term of 28 years plus a renewal term of 28 years) to 95 years (a first term of
28 years plus a renewal term of 67 years). Applying these standards, all works published in the United
States before January 1, 1931, are in the public domain." The statute behind it, 17 U.S.C. 304(b)
(https://www.law.cornell.edu/uscode/text/17/304, LII page dated 23 April 2026): "Any copyright still in
its renewal term at the time that the Sonny Bono Copyright Term Extension Act becomes effective shall
have a copyright term of 95 years from the date copyright was originally secured." And 17 U.S.C. 305
(https://www.law.cornell.edu/uscode/text/17/305): "All terms of copyright provided by sections 302
through 304 run to the end of the calendar year in which they would otherwise expire."

**6. A foreign edition published that long ago cannot be restored.** 17 U.S.C. 104A
(https://www.law.cornell.edu/uscode/text/17/104A), subsection (h)(6): a "restored work" is one that
"(B) is not in the public domain in its source country through expiration of term of protection;"
and subsection (a)(1)(B): "Any work in which copyright is restored under this section shall subsist
for the remainder of the term of copyright that the work would have otherwise been granted in the
United States if the work never entered the public domain in the United States ." (sic, the space
before the full stop is LII's.) For an 1888 Leipzig edition the remainder of a term of at most 95
years is nothing.

**7. A new edition is protected only in what it adds.** 17 U.S.C. 103(b)
(https://www.law.cornell.edu/uscode/text/17/103): "The copyright in a compilation or derivative work
extends only to the material contributed by the author of such work, as distinguished from the
preexisting material employed in the work, and does not imply any exclusive right in the preexisting
material."

**8. In the US, editorial markings can be protected, and copying notes cannot.** *Compendium of U.S.
Copyright Office Practices*, Third Edition, chapter 800 (https://www.copyright.gov/comp3/chap800/ch800-performing-arts.pdf;
chapter dated 01/28/2021), section 802.6(I), *Editorial Authorship*: "Musical editing generally
consists of adding markings for the performance of a musical composition, such as additional or
altered fingering, accents, dynamics, and the like." And: "A work consisting of editorial revisions,
annotations, elaborations, or other modifications which as a whole represent an original work of
authorship, is a derivative work." Section 802.8(B): "The author of a musical work (including any
lyrics) is the creator of the music (and/or lyrics), not the person who merely transcribes the work.
Transcribing or fixing a musical work in and of itself does not constitute authorship." Section
802.9(E)(6): "The term "transcription" is unclear, because it could refer to the act of notating or
writing down music that someone else created. In this situation, the transcriber's contribution is
not registrable."

**9. The EU allows a right in critical editions, for at most 30 years, and one for first publication
of an unpublished work, for 25.** Directive 2006/116/EC (consolidated, as finding 4), Article 5,
*Critical and scientific publications*: "Member States may protect critical and scientific
publications of works which have come into the public domain. The maximum term of protection of such
rights shall be 30 years from the time when the publication was first lawfully published." Article 4,
*Protection of previously unpublished works*: "Any person who, after the expiry of copyright
protection, for the first time lawfully publishes or lawfully communicates to the public a previously
unpublished work, shall benefit from a protection equivalent to the economic rights of the author. The
term of protection of such rights shall be 25 years from the time when the work was first lawfully
published or lawfully communicated to the public."

**10. Germany's edition right lasts 25 years.** Urheberrechtsgesetz section 70, *Wissenschaftliche
Ausgaben* (https://www.gesetze-im-internet.de/urhg/__70.html, the current text): "(1) Ausgaben
urheberrechtlich nicht geschützter Werke oder Texte werden in entsprechender Anwendung der Vorschriften
des Teils 1 geschützt, wenn sie das Ergebnis wissenschaftlich sichtender Tätigkeit darstellen und sich
wesentlich von den bisher bekannten Ausgaben der Werke oder Texte unterscheiden." "(3) Das Recht
erlischt fünfundzwanzig Jahre nach dem Erscheinen der Ausgabe, jedoch bereits fünfundzwanzig Jahre
nach der Herstellung, wenn die Ausgabe innerhalb dieser Frist nicht erschienen ist." The Federal
Ministry's English translation (https://www.gesetze-im-internet.de/englisch_urhg/englisch_urhg.html,
"Translation provided by Ute Reusch", including amendments to 23 October 2024): "(1) Editions of works
or texts which are not protected by copyright are protected accordingly under the provisions of Part 1
if they represent the result of scientifically organised activity and differ substantially from
previously known editions of the works or texts." "(3) The right expires 25 years after publication of
the edition, but 25 years after its production if the edition was not released within that period."
Section 71 gives the first publisher of a posthumous work the same 25 years ("Das Recht erlischt
fünfundzwanzig Jahre nach dem Erscheinen des Werkes").

IMSLP's own note on its Urtext files (on the Peer Gynt Suite page, finding 14) says the same in plain
words, a secondary source: "In most European Union countries, these editions (except new original
material) are generally protected for no more than 25 years from publication (30 years in Poland), and
only if the edition is published after the copyrights of the original creator(s) have expired."

**11. The EU's 2019 rule on reproductions covers visual art, not scores.** Directive (EU) 2019/790
(https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32019L0790, OJ L 130, 17 May 2019),
Article 14, *Works of visual art in the public domain*: "Member States shall provide that, when the
term of protection of a work of visual art has expired, any material resulting from an act of
reproduction of that work is not subject to copyright or related rights, unless the material resulting
from that act of reproduction is original in the sense that it is the author's own intellectual
creation." Germany implemented it as section 68, in the English translation: "Reproductions of works
of visual arts in the public domain are not protected by related rights under Parts 2 and 3." A score
is a musical work, so Article 14 does not by its words reach a scan of one. Germany keeps a 50-year
right in simple photographs (section 72, in translation: "The provisions of Part 1 applicable to
photographic works apply accordingly to photographs and products manufactured in a similar manner to
photographs."; the German subsection (3): "Das Recht nach Absatz 1 erlischt fünfzig Jahre nach dem
Erscheinen des Lichtbildes"). The Term Directive's Article 6 leaves that to each state: "Member States
may provide for the protection of other photographs."

**12. IMSLP treats a scan of a public-domain edition as public domain.** *IMSLP:Licensing Policy and
Guidelines* (https://imslp.org/wiki/IMSLP:Licensing_Policy_and_Guidelines, revision 4434734): "Note
that scanning or transferring an existing edition or recording does not create a new copyright (as
Canadian law does not allow for new copyright in mechanical/procedural/literal copying)." *IMSLP:FAQ*
(https://imslp.org/wiki/IMSLP:FAQ, revision 3115408): "As the IMSLP servers are physically hosted in
Canada, IMSLP has to follow Canadian copyright law." *IMSLP:General disclaimer*
(https://imslp.org/wiki/IMSLP:General_disclaimer, revision 4581898): "IMSLP reviewers check the
copyright status of each item in Canada, the US and the European Union." The wiki's own text, not the
scans, is "available under the Creative Commons Attribution-ShareAlike 4.0 License" (the footer of
every page).

**13. IMSLP's robots.txt bars agents from its files** (https://imslp.org/robots.txt, read 20 November
2026), for every user agent: "Disallow: /wiki/Special:ImagefromIndex", "Disallow: /images/",
"Disallow: /imglnks/", with the comment "ImagefromIndex = the file download/disclaimer handler." The
work pages under `/wiki/` may be read; the files may not be fetched by an automated reader.

### C. The Mountain King: what exists

**14. The orchestral first edition is C.F. Peters, Leipzig, 1888, and IMSLP has four scans of it plus
Grieg's manuscript.** *Peer Gynt Suite No.1, Op.46 (Grieg, Edvard)*
(https://imslp.org/wiki/Peer_Gynt_Suite_No.1,_Op.46_(Grieg,_Edvard), revision 4858207): "First
Publication. 1888 - Leipzig: C.F. Peters, Plate 7188-89."; composition "1874-75"; movement 4 "In the
Hall of the Mountain King. Alla marcia e molto marcato" in "B minor"; instrumentation "piccolo, 2
flutes, 2 oboes, 2 clarinets, 2 bassoons / 4 horns, 2 trumpets, 3 trombones, tuba / timpani, cymbals,
bass drum, triangle, strings" (the page's line breaks shown as " / "). The full scores, each tagged
"Public Domain" unless the table says otherwise:

| IMSLP file | What | Size |
|---|---|---|
| #912194 | "First edition (reprint)", "Leipzig: Edition Peters , No. 2433, n.d. [1888] . Plate 7188.", reprinted Frankfurt, scanned by archive.org | 13.65 MB, 44 pp. |
| #893887 | Peters [1888], Plate 7188, reprinted "New York: Edwin F. Kalmus , n.d.[1932]." (note: "The 1932 copyright claim is utterly fraudulent") | 6.00 MB, 41 pp. |
| #521719 | Peters [1888], Plate 7188, reprinted in "Peer Gynt Suites Nos.1 and 2", "Mineola: Dover Publications , 1997." | 3.60 MB, 37 pp. |
| #810456 | "Holograph manuscript, 1888. (January 20)." (Grieg's own hand) | 6.68 MB, 42 pp. |
| #363545 | *Samlede verker*, Band 12, ed. Finn Benestad (1929-2012), "Frankfurt: C.F. Peters , 1985. Plate 31306. URTEXT EDITION", hosted by IMSLP-EU, tagged "Public Domain - See notes on copyright status for urtext editions" | 5.31 MB, 37 pp. **Do not use** (finding 8, 10) |

The page's "Source Files" section lists 0 files. The editable files elsewhere on the page are string
parts and arrangements under "Creative Commons Attribution Non-commercial Share Alike 3.0" or
"Creative Commons Attribution-NonCommercial 4.0" (NonCommercial: not usable, as the score-format
research found). The page also lists Musopen recordings of the suite (2012) under "Creative Commons
Attribution 3.0"; HumanityOS plays its own instruments, so they are not needed.

**15. Grieg's own piano arrangement exists in three free editions**, from the same IMSLP page: his
holograph "1888. (January 21)." (#810457, 2.40 MB, 15 pp.), the Peters edition "n.d.(ca.1890). Plate
7190." edited by "Adolf Ruthardt (1849–1934)" (#36764, 4.14 MB, 15 pp.), and No. 4 alone in "New York:
The University Society , 1918. Plate 1136-4-CB.", editor "Alexander Pero" (#00581, 0.23 MB, 4 pp.).
All tagged "Public Domain".

**16. Mutopia's piano Mountain King is public domain**, piece 1888
(https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=1888): "In the Hall of the Mountain King",
"Arranger(s): Edvard Grieg", "Opus: Op. 46, No. 4", "Source: The University Society, 1918",
"Copyright: Public Domain", "Last updated: 2013/Dec/07.", "Typeset using: LilyPond , version 2.16.1",
"Maintainer: Coyau", and its scan reference "http://imslp.org/wiki/Special:ReverseLookup/581" (IMSLP
#00581, finding 15). Its "Public Domain" links to Creative Commons' old dedication, which now reads
(https://creativecommons.org/publicdomain/certification/1.0/us/): "2010-10-11: Creative Commons has
retired this legal tool and does not recommend that it be applied to works." The tool itself: the
dedicator "hereby either (a) certifies that, to the best of his knowledge, the work of authorship
identified is in the public domain of the country from which the work is published, or (b) hereby
dedicates whatever copyright the dedicators holds in the work of authorship identified below (the
"Work") to the public domain."

**17. The original incidental music, with its troll chorus, is free too.** *Peer Gynt, Op.23 (Grieg,
Edvard)* (https://imslp.org/wiki/Peer_Gynt,_Op.23_(Grieg,_Edvard), revision 4835074): full score
"Leipzig: C.F. Peters , n.d.[1908]. Plate 9355.", editor "Johan Halvorsen (1864–1935) not credited",
"First edition" (#521723, 36.06 MB, 258 pp., "Public Domain"); No. 8 "In the Hall of the Mountain King
/ I Dovregubbens hall / In der Halle des Bergkönigs" (in Benestad's numbering). This is the play's
version, in which the trolls sing; the 1988 *Samlede verker* edition (#288910) is a modern critical
edition and is not used.

**18. No other usable Mountain King encoding was found.** OpenScore's two corpora hold no Grieg but
his String Quartet Op. 27 (finding 19). The two MuseScore.com community uploads of an orchestral
Mountain King that a web search showed (user 127153, score 193505; user 48122211, score 19980562)
could not be read (MuseScore.com answered with a robot check), so their licences are unknown and they
are not used. The score-format research's Sao Mai file was already excluded (provenance unclear).

### D. Ready-made encodings and their licences

**19. OpenScore: CC0, credit requested.** Read through the code host's API (openscore.cc did not
resolve). *OpenScore/StringQuartets* (README and `LICENSE.txt`, repository last pushed 2026-10-26):
"These scores are released under Creative Commons Zero (CC0). See LICENSE.txt." and "We kindly ask
that you credit OpenScore String Quartets and provide a link to [OSSQ] or this repository for any
public-facing use of these scores." Its Beethoven folder holds String Quartets Op. 18 Nos. 1 to 6,
Op. 59 No. 3, Op. 127, Op. 130, Op. 131, Op. 135 and the Grosse Fuge Op. 133, each as MuseScore
(`.mscx`), MusicXML (`.mxl`) and PDF, with the IMSLP scan it was made from recorded in
`data/scores.tsv` (for example Op. 18 No. 1: score 8071278, IMSLP "#04755", `sq8071278.mxl` 240,096
bytes). Its Grieg folder holds only "String_Quartet_No.1,_Op.27". *OpenScore/Lieder* (last pushed
2026-09-09): "These scores are released under Creative Commons Zero (CC0)." and "We kindly ask that
you credit OpenScore Lieder"; Beethoven's "6_Lieder,_Op.48" and "8_Lieder,_Op.52" are there.

**20. Mutopia: per piece; Beethoven is 70 of 77 public domain.** *License details*
(https://www.mutopiaproject.org/legal.html, file dated 10 March 2016): "All music on Mutopia may be
freely downloaded, printed, copied, distributed, modified, performed and recorded." It lists three
kinds: "Creative Commons Attribution-ShareAlike ( 4.0 / 3.0 / 2.5 / 2.0 / 1.0 )", "Creative Commons
Attribution ( 4.0 / 3.0 / 2.5 )" and "Public Domain": "The contributor of this music has dedicated
their contribution into the public domain. You can do what you like with this music - print it out,
sell it, change it, distribute it, record it, and perform it, etc." Its Beethoven listing (all eight
pages of `make-table.cgi?Composer=BeethovenLv`, read 20 November 2026) has 77 pieces: 70 "Public
Domain", 2 "Creative Commons Attribution-ShareAlike 4.0", 3 "... 3.0" (among them the Eroica
Symphony from Litolff), 2 "... 2.5" (among them the "Moonlight" Sonata and the String Quartet Op. 131).
The candidates for HumanityOS, each "Copyright: Public Domain" on its own page:

| Mutopia piece | Source edition | Files (size from a HEAD request) |
|---|---|---|
| 931, "Für Elise", WoO 59, piano, maintainer Stelios Samelis, LilyPond 2.18.2, updated 2015/Aug/18 | "Breitkopf & Härtel, 1888" | `fur_Elise_WoO59.ly` 8,800 bytes; `.mid` 7,590 bytes |
| 941, Symphony No. 5, Op. 67, first movement, orchestra, LilyPond 2.16.2, updated 2017/Nov/05 | "Breitkopf and Härtel (1862-1865)" | `Symphony5_1-lys.zip` 94,967 bytes; `.mid` 74,649 bytes |
| 1437, Symphony No. 5, second movement, orchestra, updated 2017/Dec/08 | "Breitkopf and Härtel (1862-1865)" | `Symphony5_2-lys.zip` 85,923 bytes |
| 891, Symphony No. 7, first movement; 595, second movement (Allegretto) | Breitkopf 1862-1865; "Litolff Verlag" | not sized |
| 547 Egmont Overture; 377 Fidelio Overture; Coriolan Overture | Breitkopf; "Eulenburg, 1919"; Breitkopf | not sized |
| 528, "Ode to Joy", SATB hymn | "Various" | not sized |

Mutopia's own disclaimer: "No claim is made as to the accuracy or the factual, editorial or musical
correctness of any of the material provided here." Its robots.txt allows every reader ("User-agent: *"
with an empty "Disallow:").

**21. TROMPA: CC BY 4.0, each repository its own; five follow a 1976 Henle edition.** The
*trompamusic-encodings* organisation (read through the code host's API) has 27 repositories, 25 of
them Beethoven MEI encodings (the other two are Clara Schumann and a Mahler piano-duet arrangement). Für Elise, *Beethoven_WoO59_BreitkopfHaertel* (last pushed 2023-08-08), README: "This
repository contains an MEI encoding of Clavierstück „Für Elise“ (WoO 59) by Ludwig van Beethoven,
Breitkopf und Härtel edition, 1862–90. Plate B.298." and "No assurances are made regarding the
validity or consistency of this encoding or its structure."; `LICENSE.md`: "This work is released by
the licensor, Werner Goebl, University of Music and Performing Arts Vienna, on behalf of the TROMPA
Project under the Creative Commons Attribution 4.0 International (CC-BY-4.0) license." The file is
`Beethoven_WoO59-Breitkopf.mei`, 242,272 bytes. The same edition is IMSLP #993345, "Ludwig van
Beethovens Werke , Serie 25, No.298 (pp.354-56)", "Leipzig: Breitkopf und Härtel , n.d.[1888]. Plate
B.298.", "Public Domain" (https://imslp.org/wiki/F%C3%BCr_Elise,_WoO_59_(Beethoven,_Ludwig_van),
revision 4702855), which gives the first publication as "1867 - in Neue Briefe Beethovens (ed. Ludwig
Nohl, pp.28-33)". **Five repositories follow a modern edition**: *Beethoven_Op28_HenleUrtext* ("Henle
Urtext edition, 1976, Plate HN1032"), *Beethoven_Op31_No2_HenleUrtext*, *Beethoven_Op31_No3_HenleUrtext*,
*Beethoven_Op53_HenleUrtext* (README: "following the Henle Urtext edition, 1976. Plate HN1034") and
*Beethoven_Op27_No1_HenleUrtext*. The other 20 follow the Breitkopf edition of 1862 to 1890 (variations,
bagatelles, rondos, Für Elise; no symphony).

What CC BY 4.0 asks, from its legal code (https://creativecommons.org/licenses/by/4.0/legalcode.en),
section 3(a)(1): "If You Share the Licensed Material (including in modified form), You must: retain
the following if it is supplied by the Licensor with the Licensed Material: identification of the
creator(s) of the Licensed Material and any others designated to receive attribution, in any
reasonable manner requested by the Licensor (including by pseudonym if designated); a copyright
notice; a notice that refers to this Public License; a notice that refers to the disclaimer of
warranties; a URI or hyperlink to the Licensed Material to the extent reasonably practicable; indicate
if You modified the Licensed Material and retain an indication of any previous modifications; and
indicate the Licensed Material is licensed under this Public License, and include the text of, or the
URI or hyperlink to, this Public License."

**22. Beethoven's Symphony No. 5 and its Breitkopf score.** *Symphony No.5, Op.67 (Beethoven, Ludwig
van)* (https://imslp.org/wiki/Symphony_No.5,_Op.67_(Beethoven,_Ludwig_van), revision 4839721):
"First Publication. 1809 (April) - parts; 1826 (March) - score"; the complete-edition score IMSLP
#52624, "Ludwig van Beethovens Werke , Serie 1, No.5", "Leipzig: Breitkopf und Härtel , n.d.[1862].
Plate B.5.", 16.52 MB, 106 pp., "Public Domain"; and a modern Urtext not to use: #862873, "Clive Brown
(b. 1947)", "Wiesbaden: Breitkopf & Härtel , No. PB 5235 , 1995. Plate Wb. 2312. URTEXT EDITION".

**23. KernScores and Sapp's Beethoven: still unknown.** kern.humdrum.org answered robots.txt with HTTP
503 again (as on 2026-11-16), and the two other hosts the README of
*craigsapp/beethoven-piano-sonatas* names (kernscores.stanford.edu, kern.ccarh.org) did not answer. The
repository has no licence ("license": null in the API) and its README states none; it describes itself
as "based on the Durand 1915 edition edited by Paul Dukas". The per-file `!!!YEM` records were not
read, because reading them means opening a score file (open question 2).

## The recommendation

The rule that makes everything simple: **encode from, or import, only material whose source edition
was published before 1931, and prefer files that are public domain or CC0, so they can sit in `data/`
under its CC0 licence with no credit duty.** Record each score's provenance in the file itself (source
edition, publisher, plate, year, the scan used, and the encoding it started from).

**(a) In the Hall of the Mountain King**

1. **Piano, first: Mutopia piece 1888** (Grieg's own arrangement, The University Society 1918),
   public domain. Convert its LilyPond to our MEI. No obligation; credit Mutopia and its maintainer as
   a courtesy (`attribution_required: false`). Check it against the 1918 scan (IMSLP #00581) and drop
   any editorial marking (fingering) that is not in the Peters edition of ca. 1890 (IMSLP #36764),
   which keeps out anything Alexander Pero might have added in 1918.
2. **Orchestral, next: encode it ourselves from the 1888 Peters first edition**, from IMSLP #912194
   (the clearest scan, a reprint of the first edition), with Grieg's 1888 manuscript (#810456) as the
   check where the print is unclear. Never from the 1985 *Samlede verker* (#363545). The encoding is
   our own work on a public-domain source: dedicate it CC0 in `data/`. No credit duty; name the
   edition in the file.
3. **Later, if wanted: the troll chorus**, from the 1908 Peters full score of Op. 23 (#521723), which
   the five-vowel choir could sing.

**(b) A first Beethoven piece**

1. **Für Elise, first: Mutopia piece 931** (public domain, Breitkopf & Härtel 1888). It is the
   smallest complete piece, and TROMPA's MEI of the very same edition (CC BY 4.0) is an independent
   second encoding to compare ours against. Comparing does not copy; if we import TROMPA's file
   instead, it takes a credit row and its licence beside it (below).
2. **The first orchestral Beethoven: Mutopia piece 941**, Symphony No. 5, first movement (public
   domain, Breitkopf 1862-1865), checked against IMSLP #52624; the second movement (1437) follows.
3. **Never** a TROMPA file from the 1976 Henle edition (Op. 27 No. 1, Op. 28, Op. 31 Nos. 2 and 3,
   Op. 53), the 1995 Breitkopf Urtext, or any NonCommercial file (IMSLP's CC BY-NC typesets, Sapp's
   Mozart, DCML).

**What a CC BY file would oblige** (a TROMPA MEI, a Mutopia CC BY piece): the licence text or its
link beside the file (LICENSING.md: "with its licence file beside it"); a row in `data/credits.ron`
with `attribution_required: true` naming the creator as the licensor asks ("Werner Goebl, University
of Music and Performing Arts Vienna, on behalf of the TROMPA Project"), the licence, a link to the
source, and "converted and modified by HumanityOS" when we change it; a section in `LICENSES.md`; and
the file keeps its own licence, never relabelled CC0. A CC BY-SA piece (five of Mutopia's Beethoven)
would also make our changed version CC BY-SA: avoid while a public-domain alternative exists.

## Files to download, for the operator's permission

None of these was downloaded. The IMSLP scans must be downloaded by a person in a browser (finding
13); Mutopia's and the code host's files may be fetched by an agent once permitted.

| # | File | Where | Size | Licence | Why |
|---|---|---|---|---|---|
| 1 | `Dans_l_antre_du_roi_de_la_montagne.ly` | Mutopia piece 1888, `/ftp/GriegE/O46/Dans_l_antre_du_roi_de_la_montagne/` | 14,420 bytes | Public domain | Piano Mountain King, step (a)1 |
| 2 | IMSLP #00581 (University Society 1918, No. 4) | IMSLP Peer Gynt Suite No. 1 page | 0.23 MB, 4 pp. | Public domain | Proofread file 1 |
| 3 | IMSLP #912194 (Peters 1888 first edition, full score) | same page | 13.65 MB, 44 pp. | Public domain | Encode the orchestral version, (a)2 |
| 4 | IMSLP #810456 (Grieg's 1888 manuscript of the suite) | same page | 6.68 MB, 42 pp. | Public domain | Check unclear passages of file 3 (optional) |
| 5 | `fur_Elise_WoO59.ly` | Mutopia piece 931, `/ftp/BeethovenLv/WoO59/fur_Elise_WoO59/` | 8,800 bytes | Public domain | Für Elise, (b)1 |
| 6 | `Beethoven_WoO59-Breitkopf.mei` | trompamusic-encodings/Beethoven_WoO59_BreitkopfHaertel | 242,272 bytes | CC BY 4.0 | Second encoding to compare against (optional) |
| 7 | IMSLP #993345 (Breitkopf 1888, Serie 25 No. 298) | IMSLP Für Elise page | 0.19 MB, 3 pp. | Public domain | Proofread file 5 |
| 8 | `Symphony5_1-lys.zip` | Mutopia piece 941, `/ftp/BeethovenLv/O67/Symphony5_1/` | 94,967 bytes | Public domain | Symphony No. 5 i, (b)2 |
| 9 | IMSLP #52624 (Breitkopf [1862], Serie 1 No. 5, full score) | IMSLP Symphony No. 5 page | 16.52 MB, 106 pp. | Public domain | Proofread file 8 |

## What is still unknown, and what would settle it

1. **How the orchestral Mountain King gets encoded**: a person in MuseScore, optical music recognition
   (an open-source reader, compared first under CLAUDE.md's compare-before-adopting rule) with a
   proofread, or an agent reading the page images. Legally all three are the same (finding 8); the
   choice is step 3's. Settled by a short trial on one page of file 3.
2. **The licence of Craig Sapp's Beethoven sonatas and quartets** (the `!!!YEM` records) and
   KernScores' own terms. Settled by KernScores coming back (re-check its robots.txt), or by the
   operator permitting the reading of one file's header (`kern/sonata01-1.krn`, 16,790 bytes per the
   score-format research). Not needed for the recommended pieces.
3. **Whether Alexander Pero's 1918 markings carry any right in the EU**: only if they are original and
   he died after 1955; his dates were not found. Free in the US either way (finding 5). Settled by
   dropping his markings (recommendation (a)1), which costs nothing.
4. **Whether a TROMPA or other CC BY file belongs in the core set** or only in an optional pack, as the
   operator ruled for instruments on 2026-11-13 ("credit-only CC BY libraries allowed in OPTIONAL
   packs, never in the core set", recorded in `data/credits.ron`). A question for the operator; the
   recommended order needs no CC BY file.
5. **Whether the US status of the 1976 Henle edition's editorial content** (and so of TROMPA's five
   Henle-based files) is free: a lawyer's question (restoration under 17 U.S.C. 104A for a 1976 German
   edition). Moot if they are simply not used.
6. **Norway's own Act** (Grieg's country) was not read: lovdata.no bars every robot. Norway's term
   would matter only if it exceeded 119 years after death, and nothing found suggests that.
7. **The EU's database right** (Directive 96/9/EC) on a large encoded corpus was not researched. It
   could matter for importing a whole corpus in bulk, not for one hand-checked piece.
8. **Converting LilyPond to MEI** (Mutopia's format to ours) is a technical question for step 3, not
   a legal one; Mutopia's MIDI files (public domain too) are a fallback for the notes alone.

## Sources that could not be reached

| URL | What happened (20 November 2026) | Hoped for |
|---|---|---|
| https://id.loc.gov/authorities/names/n79046495.html and n79107741.html | "Just a moment..." robot check (HTTP 403), not worked around | The Library of Congress authority records with Grieg's and Beethoven's dates (the Bergen library and the Beethoven-Haus were used) |
| https://snl.no/ | robots.txt: "User-agent: ClaudeBot" "Disallow: /"; not read | Store norske leksikon on Grieg |
| https://lovdata.no/ | robots.txt: "User-agent: *" "Disallow: /"; not read | Norway's Copyright Act |
| https://musescore.com/user/127153/scores/193505 | Robot check (HTTP 403), not worked around | The licence of a community orchestral Mountain King |
| https://openscore.cc/ | The name did not resolve | OpenScore's own licence page (the code host's copies were read) |
| https://kern.humdrum.org/robots.txt | HTTP 503 | KernScores' terms |
| https://kernscores.stanford.edu/, https://kern.ccarh.org/ | No answer | The same |
| https://www.cpdl.org/ | robots.txt: "User-agent: ClaudeBot" "Disallow: /"; not read | Choral editions of the troll chorus |

## What it means for this project

**Certain** (from the quotations above):

- The compositions of Grieg and Beethoven are in the public domain in the US, the EU and every Berne
  country; terms of life plus 50 or 70 years ended long ago (findings 1 to 4).
- In the US, every edition published before 1 January 1931 is in the public domain, and an 1888 or
  1862 German edition cannot be restored (findings 5, 6). The 1888 Peters full score, Grieg's own
  piano arrangement of ca. 1890 and 1918, and Breitkopf's Beethoven of 1862 to 1890 may be encoded.
- Faithfully transcribing notes is not authorship in the US (finding 8): our encoding of a
  public-domain edition carries no one else's rights, and we may dedicate it CC0.
- Modern editions can be protected: editorial markings in the US (finding 8), critical editions for up
  to 30 years in the EU and 25 in Germany (findings 9, 10).
- Mutopia's piano Mountain King, Für Elise and Symphony No. 5 (first two movements) are public domain;
  OpenScore is CC0 with a credit request; TROMPA is CC BY 4.0 with a required credit (findings 16, 19,
  20, 21).
- IMSLP's robots.txt bars automated downloads of its files (finding 13).

**Judgement calls** (an AI's reading; a reader may weigh them differently):

- Treating Mutopia's retired Creative Commons dedication as enough. It was a valid dedication when made
  and the music under it is public domain anyway; the retirement notice concerns new use of the tool.
- Using a scan made recently (archive.org's of #912194) as the source to copy notes from. Article 14
  does not cover scores and Germany keeps a 50-year photograph right (finding 11), but copying the
  notes does not copy the photograph's own expression; we never ship the scan itself.
- Honouring TROMPA's CC BY 4.0 in full even where it may not bite (a faithful MEI transcription may
  hold no copyright in the US, per finding 8). Crediting costs nothing and is what the licensor asks.
- Avoiding modern Urtext editions even where their German 25-year right has run out (Henle 1976,
  *Samlede verker* 1985 and 1988), because their US status is uncertain and a pre-1931 source exists
  for every piece recommended here.
- Crediting the CC0 and public-domain sources (OpenScore, Mutopia and its maintainers) on the Credits
  page although nothing requires it.

**This is not legal advice.** It records what the sources said on 20 November 2026 and how an AI read
them; anything that turns on the open questions above is a question for a lawyer.
