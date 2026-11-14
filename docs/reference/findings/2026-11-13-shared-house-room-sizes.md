# Room sizes for shared houses and wider homes (England)

**Research date: 2026-11-13.** Not legal advice; the writer is not a lawyer. Read for the row houses
of [the home room program](../../design/home-room-program.md), section 10.

## The question

What does England's law and national guidance say about the sizes of bedrooms in a home, about the
widths of double bedrooms, and about bathrooms and toilets in a house shared by people who are not
one household (a house for couples sharing, a house for friends sharing)?

## The short answer

- The **Nationally Described Space Standard** (the "space standard") gives whole-home floor areas
  by bedrooms and people, a single bedroom 7.5 m2 and 2.15 m wide, a double 11.5 m2, and a width
  rule for doubles the room program had not used before: one double at least 2.75 m wide and every
  other at least 2.55 m. Its table has no row for five bedrooms and five people.
- A licensed **house in multiple occupation** must give a room slept in by one adult at least
  6.51 m2 and by two adults at least 10.22 m2 (SI 2018/616). The space standard is stricter, so a
  design meeting it meets these.
- Where sharers do not each have their own bathroom and toilet, five or more sharers need a
  bathroom and a separate toilet with a basin for every five, and a basin in every room let (SI
  2006/373, Schedule 3). Four or fewer need one bathroom with a toilet.

## The findings

### 1. The space standard's table and bedroom rules

- Source: Technical housing standards: nationally described space standard, UK Government,
  gov.uk/government/publications/technical-housing-standards-nationally-described-space-standard/technical-housing-standards-nationally-described-space-standard
- Source date: "Published 27 March 2015"; the notes to Table 1 were added on 19 May 2016 (the
  page's own history). Read 2026-11-13.
- Table 1, two-storey homes (minimum gross internal area, m2; read from the table): 2b4p 79; 3b5p
  93 (three storeys 99); 3b6p 102; 4b5p 97; 5b6p 110. Built-in storage 2.0 to 3.5 m2.
- Quotations, the bedroom rules:
  - "in order to provide 1 bedspace, a single bedroom has a floor area of at least 7.5m2 and is at
    least 2.15m wide"
  - "in order to provide 2 bedspaces, a double (or twin bedroom) has a floor area of at least
    11.5m2"
  - "1 double (or twin bedroom) is at least 2.75m wide and every other double (or twin) bedroom is
    at least 2.55m wide"
- Also on the page: a minimum floor-to-ceiling height of 2.3 m for at least 75% of the area, and
  area under 1.5 m of headroom left out of the floor area unless it is storage.

### 2. Sleeping rooms in a licensed shared house

- Source: The Licensing of Houses in Multiple Occupation (Mandatory Conditions of Licences)
  (England) Regulations 2018, SI 2018/616, regulation 2 as made,
  legislation.gov.uk/uksi/2018/616/regulation/2/made
- Source date: the "as made" text; it refers to "1st October 2018" for when compliance periods
  apply. Read 2026-11-13.
- Quotations: a room "used as sleeping accommodation by one person aged over 10 years is not less
  than 6.51 square metres"; "used as sleeping accommodation by two persons aged over 10 years is not
  less than 10.22 square metres".

### 3. Bathrooms and toilets for sharers

- Source: The Licensing and Management of Houses in Multiple Occupation and Other Houses
  (Miscellaneous Provisions) (England) Regulations 2006, SI 2006/373, Schedule 3 as made,
  legislation.gov.uk/uksi/2006/373/schedule/3/made
- Source date: the "as made" text. Read 2026-11-13.
- Quotations (paragraph 2, where units "do not contain bathing and toilet facilities for the
  exclusive use of each individual household"): four or fewer sharing, "there must be at least one
  bathroom with a fixed bath or shower and a toilet"; five or more, "one separate toilet with wash
  hand basin with appropriate splash back for every five sharing occupiers" and "at least one
  bathroom (which may contain a toilet) with a fixed bath or shower for every five sharing
  occupiers"; and with five or more occupiers "every unit of living accommodation must contain a
  wash hand basin with appropriate splash back". Paragraph 3 asks that a shared kitchen be "suitably
  located in relation to the living accommodation", with its equipment "in a sufficient quantity for
  the number of those sharing the facilities"; it sets no size.

## What is still unknown, and what would settle it

- **Kitchen and living room sizes for sharers.** Neither regulation gives one; English local
  authorities publish their own amenity standards. Settled by a sample of three or four councils'
  published HMO amenity standards, dated.
- **Whether these regulations have been amended since they were made.** The "as made" pages were
  read, not the latest revised versions. Settled by reading each instrument's "latest available"
  version on legislation.gov.uk.
- **Other countries.** Section 6 of the room program still wants the US International Residential
  Code and Japan's Building Standards Act read for room minimums and fire rules.

## Sources not reached

None this time.

## What it means for this project

Certain:
- The room program's bedroom checks now include the double-width rule
  (crates/humanity-ship/src/home_program_tests.rs, `bedroom_problems`), and every shipped home
  passes it.
- The friends' house (five singles sharing) has a bathroom and a separate WC with a basin, as five
  sharers need; the basins in the bedrooms are a later rung of its machine file.
- The couples' house gives each couple a shower room of their own, so the shared-bathroom rule does
  not apply to it.

Judgement:
- The space standard, written for self-contained homes, is used for the shared houses' bedrooms too,
  because it is stricter than the shared-house minimums and the game's homes are meant to be
  comfortable, not minimal.
- Five bedrooms for five people is drawn against the nearest table rows (97 m2 for 4b5p, 110 m2 for
  5b6p); the friends' house is larger than both.

Not legal advice; the writer is not a lawyer.
