# How quiet must a home be, and what else in a ship's layout decides its crew's health?

**Research date: 2026-11-06.** Every source below was read on 2026-11-06. This is a reading of
public sources, done so the mothership's layout can be checked against real health guidance. **It
is not medical, acoustic or planning advice**, and nobody who wrote it is a doctor, an acoustician
or a planner. Anyone designing a real building should go to the primary sources and to a
professional.

## The question

What noise levels do the World Health Organization's guidelines (and NASA's spacecraft standard)
set for a home by day and by night, how much does a building's shell take off, and what other
crew-health factors that a ship's layout decides (light and the day-night cycle, green space and
trees, walking distance, air quality, privacy and crowding) have a primary-source number a layout
can be checked against?

Asked by the operator on 2026-11-06: "We want to maximize crewmember health." It answers open
question 14 of [ship-homes-and-logistics.md](../../design/ship-homes-and-logistics.md) (how far
homes must be from loud places, open since 2026-10-03) and his 2026-07-01 observation that "if the
industrial area is too close, no one can sleep, the noise comes through the steel frame of the
ship" ([mothership-superstructure.md](../../design/mothership-superstructure.md)). The rules built
on it are in [mothership-master-plan.md](../../design/mothership-master-plan.md), section 3.13.

## Short answer

**Noise at a home.**

- **Inside a bedroom at night: 30 dB LAeq over the 8-hour night, and no single event above
  45 dB LAmax** (WHO *Guidelines for community noise*, 1999, F3), with "a still lower guideline
  value" for noise that is mostly low-frequency, such as ventilation (F3). The WHO's 2018 guidelines
  keep these indoor values valid (F2).
- **Inside the home by day and evening: 35 dB LAeq over 16 hours** (F3).
- **Outside the home at night: 40 dB Lnight,outside**, the WHO's health-based night noise guideline
  for Europe (2009); its 55 dB "interim target" is "not a health-based limit value by itself" (F4).
- **Outdoor living areas by day: 50 dB LAeq** for moderate annoyance, 55 dB for serious (F3).
- **By source, outdoors** (WHO 2018, F1): road traffic 53 dB Lden and 45 dB Lnight; railways 54
  and 44; aircraft 45 and 40; leisure noise 70 dB LAeq,24h.
- **NASA's spacecraft standard** (F5) limits continuous noise in crew quarters and sleep areas on
  missions over 30 days to the NC-40 curve, work areas to NC-50, and noise events during sleep to
  10 dB above the background. A real crew on a 1991 Shuttle mission lived at 68 to 75.5 dBA by day
  and reported irritation and headaches (F6).
- **What a building's shell takes off**: the WHO estimates the indoor level at "around 10 dB" below
  outdoors with windows open, 15 dB tilted and "about 25 dB for closed windows" (F2); its 2009 night
  guidelines use 21 dB as a year's average (F4). England requires at least 45 dB of airborne sound
  insulation between new flats (F7).

**The other factors.**

- **Light**: at least 250 lux melanopic equivalent daylight illuminance (melanopic EDI) at the eye
  by day, at most 10 lux in the 3 hours before bedtime, at most 1 lux in the sleeping room at night
  (Brown and others, 2022, F8). NASA's 1975 settlement study required "a diurnal cycle" in homes
  (F9).
- **Green space**: public green space "of at least 0.5–1 hectare within 300 metres' linear distance
  (around 5 minutes' walk)" of every home (WHO 2017, F10); the 3-30-300 rule adds three trees in
  view from every home and 30% tree canopy in every neighbourhood (F11), and its author says the
  "3" is a communication choice, not evidence. NASA's study gave 10 m2 of open space a person (F12).
- **Crowding and privacy**: the WHO strongly recommends strategies "to prevent and reduce household
  crowding" and lists the measures in use, from more than one person a room (American Crowding
  Index) to the British Bedroom Standard (F13); "an adequate proportion of the housing stock should
  be accessible" (F14).
- **Air**: the WHO's 2021 guideline levels, among them PM2.5 at 5 µg/m3 a year and 15 a day, and
  NO2 at 10 a year (F15); NASA's 1975 study kept carbon dioxide below 0.4 kPa (F16).

---

## How the sources were read

- The WHO's 2018 and 2009 guidelines, its 2017 green space brief, its 2018 housing guidelines and
  its 2021 air quality guidelines were downloaded from WHO's repository (IRIS) through the links on
  each publication's who.int page, and turned into text with `pdftotext -layout`.
- **The WHO's own PDF of the 1999 *Guidelines for community noise* is a scan with no text layer**
  (who.int/publications/i/item/a68672, its download from IRIS). Its text was read from an identical
  copy of the WHO document, title page "Edited by Birgitta Berglund, Thomas Lindvall, Dietrich H
  Schwela", World Health Organization, Geneva, hosted by the London Borough of Camden
  ([camdocs.camden.gov.uk record 8372262](https://camdocs.camden.gov.uk/CMWebDrawer/Record/8372262/file/document)).
  Its sleep sentences are confirmed by the WHO's own 2009 guidelines, which quote them in full (F4).
- The NASA standard was read through NASA's technical brief that reproduces its requirements word
  for word (F5); the standard itself was not opened.
- **Quotations reproduce the source's words exactly.** Where PDF text extraction garbled a range
  dash, it is written as an en dash (–). Table cells that the extraction scrambled were matched to
  their rows by position and checked against the same table printed in another chapter of the same
  document.
- dB: decibels, A-weighted. LAeq: the average level over the stated time. LAmax: the loudest single
  event. Lnight: the average over the 8-hour night across a whole year, outdoors at the facade.
  Lden: a 24-hour average with evening and night weighted more heavily.

---

## Noise

### F1. WHO 2018: guideline levels by source (outdoors)

WHO Regional Office for Europe, *Environmental noise guidelines for the European Region*, ISBN 978
92 890 5356 3 (the publication page,
[who.int/europe/publications/i/item/9789289053563](https://www.who.int/europe/publications/i/item/9789289053563),
dates it 30 January 2019; the document itself is dated 2018). Executive summary, pages xv to xvii:

> "For average noise exposure, the GDG strongly recommends reducing noise levels produced by road
> traffic below 53 decibels (dB) Lden, as road traffic noise above this level is associated with
> adverse health effects."

> "For night noise exposure, the GDG strongly recommends reducing noise levels produced by road
> traffic during night time below 45 dB Lnight, as night-time road traffic noise above this level
> is associated with adverse effects on sleep."

The same pair for railways is 54 dB Lden and 44 dB Lnight; for aircraft 45 dB Lden and 40 dB
Lnight; for wind turbines 45 dB Lden (conditional), with "No recommendation is made for average
night noise exposure Lnight of wind turbines." For leisure noise:

> "For average noise exposure, the GDG conditionally recommends reducing the yearly average from
> all leisure noise sources combined to 70 dB LAeq,24h as leisure noise above this level is
> associated with adverse health effects."

On where it applies: "the recommended exposure levels can be considered applicable in other
regions and suitable for a global audience".

### F2. WHO 2018: why no indoor values, what a shell takes off, and the 1999 values kept

Same document, section 2.2.2, "Environmental noise indicators used in the guidelines":

> "These are the practical reasons why the GDG decided not to recommend any guideline values for
> noise indoors. Nevertheless, in certain cases it could be helpful to estimate indoor levels based
> on outdoor values. The differences between indoor and outdoor levels are usually estimated at
> around 10 dB for open, 15 dB for tilted or half-open and about 25 dB for closed windows."

Section 2.6.3, "Differences from the prior noise guidelines":

> "The current environmental noise guidelines for the European Region supersede the CNG from 1999.
> Nevertheless, the GDG recommends that all CNG indoor guideline values and any values not covered
> by the current guidelines (such as industrial noise and shopping areas) should remain valid."

And on industry, section 2.2, "Scope of the guidelines": "The GDG agreed not to develop specific recommendations for occupational
and industrial noise."

### F3. WHO 1999: the indoor values, and lower ones for low-frequency noise

World Health Organization, *Guidelines for community noise*, edited by Birgitta Berglund, Thomas
Lindvall and Dietrich H Schwela (the who.int page dates it 10 February 1999; the document records
the expert task force meeting of April 1999). Table 4.1 (page 65) and Table 1 of the executive
summary give, for each environment, the critical health effect, LAeq, its time base and LAmax:

| Specific environment | Critical health effect(s) | LAeq [dB] | Time base [hours] | LAmax, fast [dB] |
|---|---|---|---|---|
| Outdoor living area | Serious annoyance, daytime and evening | 55 | 16 | - |
| | Moderate annoyance, daytime and evening | 50 | 16 | - |
| Dwelling, indoors | Speech intelligibility and moderate annoyance, daytime and evening | 35 | 16 | - |
| Inside bedrooms | Sleep disturbance, night-time | 30 | 8 | 45 |
| Outside bedrooms | Sleep disturbance, window open (outdoor values) | 45 | 8 | 60 |
| Industrial, commercial shopping and traffic areas, indoors and outdoors | Hearing impairment | 70 | 24 | 110 |

On sleep, executive summary, page xii:

> "When noise is continuous, the equivalent sound pressure level should not exceed 30 dB(A)
> indoors, if negative effects on sleep are to be avoided. For noise with a large proportion of
> low-frequency sound a still lower guideline value is recommended. When the background noise is
> low, noise exceeding 45 dB LAmax should be limited, if possible, and for sensitive persons an even
> lower limit is preferred."

And just before it: "It should be noted that low-frequency noise, for example, from ventilation
systems, can disturb rest and sleep even at low sound pressure levels."

### F4. WHO 2009: 40 dB at night outdoors, and the shell's yearly average

WHO Regional Office for Europe, *Night noise guidelines for Europe*, ISBN 9789289041737 (the
publication page,
[who.int/europe/publications/i/item/9789289041737](https://www.who.int/europe/publications/i/item/9789289041737),
dates it 15 October 2009). Executive summary, Table 4:

> "Night noise guideline (NNG) Lnight,outside = 40 dB"
> "Interim target (IT) Lnight,outside = 55 dB"

> "It should be emphasized that IT is not a health-based limit value by itself. Vulnerable groups
> cannot be protected at this level."

Its footnote defines the night: "the night is eight hours (usually 23.00 – 07.00 local time)".

On what the shell takes off (chapter 1, "Methods and criteria"):

> "Y is the year average insulation value of the (bedroom) facade. In this report a default value
> of 21 dB is used (see also section 1.3.5). It should be stressed that this conversion is thought
> to be highly dependent on local building habits, climate and window opening behaviour."

It quotes the 1999 guidelines' sleep statement in full ("If negative effects on sleep are to be
avoided the equivalent sound pressure level should not exceed 30 dBA indoors for continuous noise.
If the noise is not continuous, sleep disturbance correlates best with LAmax and effects have been
observed at 45 dB or less.") and adds: "The thresholds are now known to be lower than LAmax of 45
dB for a number of effects."

### F5. NASA's spacecraft standard: NC-40 where the crew sleeps

NASA Office of the Chief Health and Medical Officer, *NASA-STD-3001 Technical Brief: Acoustics*,
OCHMO-TB-035 Rev C, dated 10/06/2023, which states "This Technical Brief is derived from NASA-STD-3001
and is for reference only" and reproduces the requirements of NASA-STD-3001 Volume 2, Revision D
([nasa.gov, acoustics_technical_brief_ochmo_0.pdf](https://www.nasa.gov/sites/default/files/atoms/files/acoustics_technical_brief_ochmo_0.pdf)):

> "[V2 6079] Crew Sleep Continuous Noise Limits For missions greater than 30 days, SPLs of
> continuous noise shall be limited to the values given by the NC-40 curve (see Figure 6.6-1--NC
> Curves, and Table 6.6-3--Octave Band SPL Limits for Continuous Noise, dB re 20 Pa) in crew
> quarters and sleep areas. Hearing protection cannot be used to satisfy this requirement."

> "[V2 6082] Annoyance Noise Limits for Crew Sleep With the exception of communications and alarms,
> the system shall limit impulse and intermittent noise levels at the crewmember's head location to
> 10 dB above background noise levels during crew sleep periods. Hearing protection cannot be used
> to satisfy this requirement."

Work areas: "[V2 6078] Continuous Noise Limits In spacecraft work areas, where good voice
communications and habitability are required, SPLs of continuous noise (not including impulse or
intermittent noise sources) shall be limited to the values given by the Noise Criterion (NC)-50
curve". And everywhere outside launch and entry, "[V2 6077]": "the A-weighted sound level
(excluding impulse noise and alarm signals) shall be less than 85 dBA, regardless of time
duration".

### F6. What a noisy crew module did to a real crew

Same brief, "Reference Data":

> "Sound levels in the Spacelab Module, a research module located in the Shuttle's cargo bay during
> STS-40, were measured to be 68 dBA, on a daily average basis, but rose on some days to be as high
> as 75.5 dBA and up to 84 dBA during ergometer operations (Goodman, 1991b)."

> "The crew was very irritated during operations and sleep periods, and reported headaches due to
> the high noise levels experienced (Goodman, 1991a)."

### F7. England's minimum sound insulation between flats

HM Government, *Approved Document E: Resistance to the passage of sound*, 2003 edition incorporating
the 2004, 2010, 2013 and 2015 amendments (online version; the gov.uk page,
[gov.uk/government/publications/resistance-to-sound-approved-document-e](https://www.gov.uk/government/publications/resistance-to-sound-approved-document-e),
is dated 4 March 2015). Table 0.1a, "Dwelling-houses and flats – performance standards for
separating walls, separating floors, and stairs that have a separating function", purpose built
dwelling-houses and flats: walls, airborne sound insulation DnT,w + Ctr of at least **45 dB**;
floors and stairs, at least **45 dB** airborne and an impact sound level L'nT,w of at most
**62 dB**. (Values read from the table; Crown copyright text, numbers cited.)

---

## Light and the day-night cycle

### F8. How much light, and when (Brown and others, 2022)

Timothy M. Brown and others, "Recommendations for daytime, evening, and nighttime indoor light
exposure to best support physiology, sleep, and wakefulness in healthy adults", *PLoS Biology*
20(3): e3001571, published 17 March 2022
([journals.plos.org/plosbiology/article?id=10.1371/journal.pbio.3001571](https://journals.plos.org/plosbiology/article?id=10.1371/journal.pbio.3001571)):

> Daytime: "the recommended minimum melanopic EDI is 250 lux at the eye measured in the vertical
> plane"

> Evening: "starting at least 3 hours before bedtime, the recommended maximum melanopic EDI is 10
> lux"

> Night (the sleep environment): "The recommended maximum ambient melanopic EDI is 1 lux measured
> at the eye."

(Melanopic EDI weights light by how strongly it drives the eye's non-visual, clock-setting
response; the paper measures at about 1.2 m.)

### F9. NASA 1975: homes need a day and a night

NASA SP-413, *Space Settlements: A Design Study* (1977,
[ntrs.nasa.gov/citations/19770014162](https://ntrs.nasa.gov/citations/19770014162)), chapter 5,
pages 88 to 89, on the torus's mirrors (the scan's stray spaces removed):

> "The flux of light in space is 1400 W/m2, but requirements in the torus vary from 200 W/m2 in the
> residential areas to 1000 W/m2 in the agricultural areas. Furthermore, a diurnal cycle is
> required in residential and some agricultural areas, while other agricultural regions require
> continuous solar radiation."

Its Table 3-5, "Summary of qualitative criteria of environmental design", lists among others
"Natural light", "Parts of interior out of sight of others" and "Availability of privacy".

---

## Green space and trees

### F10. WHO 2017: green space within 300 metres of every home

WHO Regional Office for Europe, *Urban green spaces: a brief for action*, ISBN 9789289052498 (the
publication page,
[who.int/europe/publications/i/item/9789289052498](https://www.who.int/europe/publications/i/item/9789289052498),
dates it 1 October 2017), under "Put the green space close to people":

> "As a rule of thumb, urban residents should be able to access public green spaces of at least
> 0.5–1 hectare within 300 metres' linear distance (around 5 minutes' walk) of their homes."

### F11. The 3-30-300 rule (Konijnendijk, 2022)

Cecil C. Konijnendijk, "Evidence-based guidelines for greener, healthier, more resilient
neighbourhoods: Introducing the 3–30–300 rule", *Journal of Forestry Research* 34 (2023), 821 to
830, published online 26 August 2022, open access under a Creative Commons licence
([link.springer.com, 10.1007/s11676-022-01523-z](https://link.springer.com/content/pdf/10.1007/s11676-022-01523-z.pdf)).
Abstract:

> "This guideline aims to provide equitable access to trees and green spaces and their benefits by
> setting the thresholds of having at least 3 well-established trees in view from every home,
> school, and place of work, no less than a 30% tree canopy in every neighbourhood; and no more
> than 300 m to the nearest public green space from every residence."

On the "3": "The specific number '3' is not supported by scientific evidence but was chosen to
connect with the numbers 30 and 300 from a communication and 'stickiness' perspective." On the
"300": "WHO suggests a public green space of at least 1-ha, but this may sometimes be difficult to
achieve." (F10's WHO brief says 0.5–1 hectare; the two sources differ on the lower bound.)

### F12. NASA 1975: 10 m2 of open space a person

NASA SP-413, chapter 3, Appendix B (see the
[space program finding](2026-11-06-space-program-sources.md), S7): "a lower value of open space in
the residential area is adopted, namely 10 m2/person", against 18 m2 averaged over 53 US cities and
the 14 m2 DeChiara and Koppelman recommend, because the settlement's farms partly serve as open
space.

---

## Crowding, privacy and access

### F13. WHO 2018: prevent and reduce household crowding

World Health Organization, *WHO Housing and health guidelines*, ISBN 978 92 4 155037 6 (the
publication page,
[who.int/publications/i/item/9789241550376](https://www.who.int/publications/i/item/9789241550376),
dates it 23 November 2018). The recommendation, rated "Strong":

> "Strategies should be developed and implemented to prevent and reduce household crowding."

The definition, chapter 3: "Household crowding is a condition where the number of occupants exceeds
the capacity of the dwelling space available, whether measured as rooms, bedrooms or floor area,
resulting in adverse physical and mental health outcomes". Its Table 3.1 lists measures in use,
among them:

> "UN-Habitat Overcrowding occurs if there are more than three people per habitable room"

> "American Crowding Index Crowding occurs if there is more than one person per room; severe
> crowding occurs if there are more than 1.5 persons per room (excluding bathrooms, balconies,
> porches, foyers, hall-ways and half-rooms)"

and the British Bedroom Standard, by which a household is overcrowded unless each "cohabiting adult
couple", each "person aged over 21 years", each "same sex pair of children aged 10–20 years" and
each "two children aged less than 10 years" (among others) has its own bedroom.

### F14. WHO 2018: accessible homes, and warmth

Same guidelines, Table 1, both rated "Strong":

> "Based on the current and projected national prevalence of populations with functional
> impairments and taking into account trends of ageing, an adequate proportion of the housing stock
> should be accessible to people with functional impairments."

> "Indoor housing temperatures should be high enough to protect residents from the harmful health
> effects of cold."

(It adds, conditionally: "For countries with temperate or colder climates, 18 °C has been proposed
as a safe and well-balanced indoor temperature to protect the health of general populations during
cold seasons.")

---

## Air

### F15. WHO 2021: the air quality guideline levels

World Health Organization, *WHO global air quality guidelines: particulate matter (PM2.5 and
PM10), ozone, nitrogen dioxide, sulfur dioxide and carbon monoxide*, ISBN 9789240034228 (the
publication page,
[who.int/publications/i/item/9789240034228](https://www.who.int/publications/i/item/9789240034228),
dates it 22 September 2021). Executive summary, "Table 0.1. Recommended AQG levels and interim
targets", the guideline (AQG) levels:

| Pollutant | Averaging time | AQG level |
|---|---|---|
| PM2.5, µg/m3 | Annual | 5 |
| PM2.5, µg/m3 | 24-hour | 15 |
| PM10, µg/m3 | Annual | 15 |
| PM10, µg/m3 | 24-hour | 45 |
| O3, µg/m3 | Peak season | 60 |
| O3, µg/m3 | 8-hour | 100 |
| NO2, µg/m3 | Annual | 10 |
| NO2, µg/m3 | 24-hour | 25 |
| SO2, µg/m3 | 24-hour | 40 |
| CO, mg/m3 | 24-hour | 4 |

The 24-hour values are the "99th percentile (i.e. 3–4 exceedance days per year)". And: "all
recommendations are classified as strong".

### F16. NASA 1975: carbon dioxide below 0.4 kPa

NASA SP-413, chapter 3, page 27 (Table 3-3 gives the same "<0.4 kPa (<3 mm Hg)"):

> "The level of carbon dioxide should be maintained below the OSHA standard (ref. 32), which
> specifies that pCO2 be less than 0.4 kPa (3 mm Hg)."

---

## What is still unknown, and what would settle it

| Unknown | What would settle it | Cost |
|---|---|---|
| How much sound a ship's steel frame carries from a reactor or a factory to a home (structure-borne noise, the operator's 2026-07-01 concern) | Published transmission loss of ship structures per frame and per deck (marine noise standards, for example the IMO's code on noise levels on board ships) | A research pass |
| The WHO's "still lower" value for low-frequency noise has no number | National low-frequency criteria (several European countries publish curves) | A research pass |
| The A-weighted level an NC-40 spectrum comes to, to compare NASA's sleep limit with the WHO's 30 dB | The NC curve table in NASA-STD-3001 Volume 2 (Table 6.6-3) and an A-weighting sum | An hour |
| The sound power of each loud machine the ship will carry (reactor halls, pumps, fans, the axial rail) | Manufacturers' data per machine class, or measured values from comparable plants | Per machine, as each is designed |
| Escape distances and lift sizes for multi-storey homes | A building code's means-of-egress and accessibility chapters (the US International Building Code refused automated reading, below) | An hour, with a readable code |
| Rotation comfort above 1 rpm (NASA's 1975 criterion was "≤1 rpm"; Sector 1 is 1.89 rpm) | The later literature on Coriolis adaptation | A research pass |

## Sources that could not be reached

- The WHO's own PDF of the 1999 community noise guidelines has no text layer (a scan); the text was
  read from Camden's copy of the same document, as said above.
- PubMed Central's copy of the 3-30-300 paper (pmc.ncbi.nlm.nih.gov/articles/PMC9415244) answered
  with a reCAPTCHA page; Springer's open-access PDF was read instead.
- The WHO fact sheet on ambient air quality (who.int/news-room/fact-sheets, dated 24 October 2024)
  shows its guideline levels only in an image; the 2021 guidelines themselves were read instead.
- The International Code Council's International Building Code 2021, chapter 12 (sound
  transmission between dwelling units), at codes.iccsafe.org, returned 403 Forbidden.
- `https://www.who.int/docstore/peh/noise/Comnoise-4.pdf` (an old WHO path) returned 404.

## What it means for this project

### Certain (from the sources)

- The WHO's health-based targets for a home are 30 dB LAeq inside a bedroom over the night with
  events under 45 dB LAmax, 35 dB inside by day, and 40 dB Lnight outside at night; its 55 dB
  interim target is not health-based (F3, F4, F2).
- Low-frequency noise disturbs sleep at lower levels and earns a lower value, unnumbered (F3).
- NASA limits sleep areas on long missions to NC-40 and sleep-time events to 10 dB above background
  (F5), and a noisy module gave a real crew headaches (F6).
- A closed shell takes off about 25 dB, a year's average with windows sometimes open about 21 dB
  (F2, F4); England requires 45 dB between flats (F7).
- Daytime light at the eye should reach 250 lux melanopic EDI, evenings stay under 10, sleeping
  rooms under 1 (F8).
- Green space of half a hectare to a hectare within 300 m of every home is the WHO's rule of thumb
  (F10); the 3-30-300 rule's "3" is not evidence-based by its author's own account (F11).

### Judgement calls (ours; a reader may weigh them differently)

- **"Maximize crewmember health" picks the strictest value wherever sources differ**: the WHO's 30
  dB bedroom over NASA's NC-40, the WHO's 40 dB night outdoors over any interim target, the 1 ha end
  of the WHO's green space range, every home accessible rather than "an adequate proportion".
- **The ship's homes have sealed windows** (the drum's air is shared and conditioned), so the
  closed-window 25 dB is the planning default for a home's shell; a quieter wall is a design
  choice, never an assumption a layout leans on.
- **The deliberate ship hum** (the operator's 2026-11-06 idea, [music.md](../../design/music.md))
  is two things: an audio effect the player hears, set by the player's own volume, and a sound the
  simulated crew lives with. Only the second is a health exposure, and it is low-frequency, so the
  rule for it is stricter than for other sound.
- These are guidelines for Earth's cities and for spacecraft. A mothership is both; the rules in the
  master plan apply the city values to the drum (homes, streets, parks) and NASA's to workplaces in
  the spine.

This is a reading of public sources, not medical, acoustic or planning advice.
