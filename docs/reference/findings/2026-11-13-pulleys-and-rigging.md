# Pulleys and rigging: friction per sheave, rope strength, block sizes, who can pull

**Researched 2026-11-13.** Every source below was opened and read on that date. This is
engineering reference research for a game and a learning tool, not professional engineering or
legal advice: a real lift is planned by a competent person with the maker's ratings in hand.

## The question

What do primary engineering sources say about a block and tackle's friction, the strength and
weight of its rope, the size of its blocks and sheaves, its safety factors, and how hard a person
or a winch can pull, so the Pulleys configurator (`docs/design/pulleys.md`) can use their numbers?

## The short answer

- Friction compounds per sheave. Crosby publishes the load-to-line-pull ratio for 1 to 20 parts of
  line, for bronze-bushed and anti-friction sheaves (F6); the sum of e^k with e = 0.96 and 0.98
  reproduces both columns within 2 percent. The US Army's field rule is "about 10 percent of the
  weight of the load" per sheave (F1).
- Rope strength scales with the square of its diameter in both of the Army's rules of thumb:
  fibre rope's safe working capacity in tons is D squared, wire rope's is 8 D squared (D in inches;
  F3, F4). Fibre rope takes a factor of safety of 4 (F3); blocks are "Generally expressed" at 4:1
  (F10); a running wire rope on a crane takes five (F9).
- Sheaves: for fibre line about twice the line's circumference, the block about three times it
  (F5); for wire rope "never less than 20 times the wire-rope diameter" (F4, F5).
- A person pulls about 100 pounds on a vertical pull and 60 on a horizontal one (F7). A winch's
  rating holds on the first layer of rope only (F8). Geared chain hoists run from about 85 down to
  35 percent efficient (F11).

## Findings

**F1. The Army's friction rule.** US Army FM 5-125, *Rigging Techniques, Procedures, and
Applications* (October 1995), pp. 3-21 to 3-22, read in the Internet Archive's text of the scan:
https://archive.org/details/fm-5-125-rigging-techniques-procedures-and-applications-1995 (the
item's publication date 1995-10; uploaded 2026-03-19).
> "Each sheave in the tackle system can be expected to create a resistance equal to about 10
> percent of the weight of the load."

The pull is "the sum of the weight of the load and the friction in the tackle system divided by
the theoretical" mechanical advantage. The rule adds 10 percent of the load per sheave, linearly.
Taken per sheave and compounded it is an efficiency of 1 / 1.10 = 0.909 per sheave (our
derivation), which is the `plain_bore` row in `data/pulleys/pulleys.ron`.

**F2. The bearing classes.** The Crosby Group, catalog page 438 ("Applications & Warnings",
blocks), © 2023, https://www.thecrosbygroup.com/wp-content/uploads/catalog/2022/en/438.pdf, its
selection guide:
> "A. Common (Plain) Bore for very slow line speeds and very infrequent use (high bearing
> friction)."

and, of the fourth class, "D. Anti-friction Bearings for faster line speeds and more frequent use
at greater loads (minimum bearing friction)." Between them, self-lubricating and pressure-lubricated
bronze bushings, "(moderate bearing friction)".

**F3. Fibre rope.** FM 5-125, Chapter 1, Section I ("Strength", p. 1-3):
> "The rule of thumb for the SWC, in tons, for fiber rope is equal to the square of the rope
> diameter (D) in inches"

Table 1-1, note 2: "Safe working load may be computed using a safety factor of 4". The text: "A
new 1-inch diameter, Number 1 Manila rope has a BS of 9,000 pounds" (the scan's OCR prints the
"1" as "I|"). Table 1-1's 1 1/2 inch row reads "0.600" pounds a foot and "18,500" pounds breaking
strength; most of the table's other rows are garbled in the OCR text and were not used.

**F4. Wire rope.** FM 5-125, Chapter 1, Section II ("Strength", p. 1-9):
> "As a rule of thumb, you can square the diameter of wire rope in, inches, and multiply by 8 to
> obtain the SWC in tons"

followed at once by "A value obtained in this manner will not always agree with the FS given in
Table 1-3." Table 1-3 itself is illegible in the OCR text. "Drums and Sheaves" (p. 1-15): the
sheave diameter "always should be as large as possible" and "never less than 20 times the
wire-rope diameter."

**F5. Block to line ratio.** US Navy, *Steelworker Volume 2* (NAVEDTRA 14251, per the reprint's
path), section "Block To Line Ratio", read in the reprint at
https://constructionmanuals.tpub.com/14251/css/Block-to-Line-Ratio-151.htm (no date on the page):
> "With fiber line, the length of the block used should be about three times the circumference of
> the line."

Then: "However, an inch or so either way does not matter too much;" and "the sheave diameter should
be about twice the size of the circumference of the line used." For wire rope, "a rough rule of
thumb is that the sheave diameter should be about 20 times the diameter of the wire."

**F6. Crosby's line-pull ratios and wire rope weights.** The Crosby Group, catalog page 443
("Blocks, Section 15, Applications & Warnings"), © 2023,
https://www.thecrosbygroup.com/wp-content/uploads/catalog/2022/en-US/443.pdf. Its ratio table,
"Ratio A or B = Total Load to be Lifted / Single Line Pull (lb)", Ratio A for bronze-bushed and
Ratio B for anti-friction bearing sheaves, by number of line parts:

| Parts | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| Ratio A (bronze) | .96 | 1.87 | 2.75 | 3.59 | 4.39 | 5.16 | 5.90 | 6.60 |
| Ratio B (anti-friction) | .98 | 1.94 | 2.88 | 3.81 | 4.71 | 5.60 | 6.47 | 7.32 |

(the table runs on to 20 parts, 16.40 for Ratio B). Its worked example: "9,290 lbs single line pull
required to lift this load on 8 parts of line" (68,000 lb over Ratio B 7.32). "Factor A, Wireline
Weight (lbs per ft) 6 x 19 IWRC": 1/2 inch .46, 3/4 inch 1.04, 1 inch 1.85.

**F7. How hard people pull.** FM 5-125, p. 3-22:
> "On a vertical pull, men of average weight can pull about 100 pounds per man and about 60 pounds
> per man on a horizontal."

and "More men can pull on a single horizontal line along the ground than on a single vertical line."

**F8. A winch's rating.** FM 5-125, p. 3-24:
> "When a winch is rated at a capacity, that rating applies only as the first layer of cable is
> wound onto the drum."

and "The capacity of the winch may be reduced by as much as 50 percent when the last layer is being
wound onto the drum."

**F9. A running rope's design factor.** US OSHA, 29 CFR 1926.1414, "Wire rope: selection and
installation criteria", paragraph (e)(4)(ii)(E), read at
https://www.osha.gov/laws-regs/regulations/standardnumber/1926/1926.1414 (rule published
75 FR 48149, August 9, 2010):
> "The operating design factor for the boom hoist reeving system must be not less than five."

The paragraph is about cranes' boom hoist reeving; we use five as the design factor paired with
F4's rule (our pairing, said so in the data file).

**F10. A block's design factor.** Crosby, page 438 (as F2), "DESIGN (SAFETY) FACTOR":
> "Generally expressed for blocks as a ratio of 4:1."

**F11. Geared chain hoists.** FM 5-125, pp. 3-22 to 3-23: a spur-gear chain hoist "is about 85
percent efficient"; "The screw-gear chain hoist is about 50 percent efficient."; "The differential
chain hoist is only about 35 percent efficient." And "Chain hoists differ widely in their MA,
depending on their rated capacity."

**F12. Simple and compound tackles; chain.** FM 5-125, pp. 3-14 to 3-20: "A simple tackle system
uses one rope and one or more blocks." "A compound tackle system uses more than one rope with two
or more blocks." Of chain: "The SWC ordinarily is assumed to be about one-sixth of the BS, giving a
FS of 6."

## What is still unknown, and what would settle it

- **Synthetic rope** (polyester, nylon, HMPE): breaking strength, weight and stretch per diameter
  from a maker's published table, so the configurator can offer the ropes people actually buy.
- **Rope stretch** for the Realistic mode: each rope's elongation at its working load, from the
  same tables.
- **A person's pulling speed and a vehicle winch's rated speed**: the `person` row's 0.25 m/s and
  the `truck_winch` row's numbers are design values until a primary source is read.
- **FM 5-125 Tables 1-2 and 1-3** (wire rope strength and factors of safety by use): a clean scan.

## Sources we could not reach

- https://www.ecfr.gov/current/title-29/subtitle-B/chapter-XVII/part-1926/subpart-CC/section-1926.1414
  redirected to a robot check (unblock.federalregister.gov); OSHA's own copy was read instead (F9).
- https://www.warn.com/products/zeon-xd-12-s-110012 answered HTTP 429 twice; the
  literature.warn.com brochure viewer serves no text to an automated reader. The truck winch row
  stays a design value.
- https://www.globalsecurity.org/ (FM 55-501 and other field manuals): its robots.txt disallows
  ClaudeBot, so it was not read.

## What it means for this project

**Certain:** the friction model (the sum of e^k) matches a maker's published ratios; rope strength
by the square of the diameter is the Army's own rule; the sheave and block size rules and the
factors of safety above are what the sources print.

**Judgement, labelled in the data:** the plain-bore efficiency (F1 compounded), the wire rope's
design factor of five (F9 paired with F4), the steel's strength from `data/materials.csv`, and the
strength-of-materials sizing of pins, plates and shackles (our derivation, in the design doc).

This is engineering reference research, not professional engineering or legal advice.
