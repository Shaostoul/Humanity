# Asteroid orbits for the map: may the project ship a small file derived from the Minor Planet Center's MPCORB, and on what terms

**Researched 2026-10-21** (sources read between about 10:50 and 11:20 Pacific
time, 17:50 to 18:20 UTC, on 21 October 2026). Everything below was read on
that day. **The Minor Planet Center's website policy changed its wording at
some point between 25 September 2026 and the day of this research** (finding
4), so if you are reading this later, read that page again first.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as a judgement
call, and a lawyer could disagree with any of it.

## The question

May HumanityOS take about 20,000 asteroid orbits (semi-major axis,
eccentricity, inclination, ascending node, argument of perihelion, mean
anomaly, epoch, and absolute magnitude H) out of the Minor Planet Center's
MPCORB data and ship that derived file in its open-source repository and app,
to draw the asteroid belt, Jupiter trojans, Hildas and Kuiper belt as point
clouds on the map, and what attribution or other conditions apply?

## Short answer

- **Yes, with conditions, and the conditions are written down by the MPC
  itself.** The header at the top of `MPCORB.DAT` says software programs "may
  include this datafile amongst their datasets" provided three things are done:
  the header is included (a separate file is acceptable), "proper attribution
  to the Minor Planet Center is given", and information on how to get updated
  copies is included. The MPC's website policy, as served on 2026-10-21, says
  separately: "Data files are copyrighted entities and may be redistributed as
  long as the source for the data is clearly specified."
- **It is not public domain and not open-licensed.** The MPC says its data
  files are "copyrighted entities". The repository's own CC0 does not reach
  this file: it ships under the MPC's conditions, recorded in `LICENSES.md`
  and `data/credits.ron` with `attribution_required: true`.
- **Neither MPC text speaks to a subset with columns removed.** The permission
  is worded for "this datafile" and for "data files". The project's reading
  (judgement call) is that a 20,000-row extract is within both, provided every
  condition is met in full anyway: ship the header verbatim beside the derived
  file, credit the MPC where the belts are drawn and in Credits, and give the
  update address.
- **One new line on the policy page reads more broadly.** Under the licence
  for "MPC Circulars and other products" the current page says "Redistribution
  of material posted on this website is not permitted in any form." The
  data-files permission is a separate bullet beside it, and the wording before
  the September/October 2026 rewrite scoped that ban to "the circulars". The
  project reads the ban as not reaching data files (judgement call); one
  message to the MPC helpdesk would settle it.
- **Alternatives checked.** Lowell Observatory's `astorb.dat` carries an
  explicit, generous grant ("may be freely used, copied, and transmitted
  provided attribution is made to the aforementioned funding sources"), but it
  is 112 MB compressed and Lowell's database was marked unavailable on the day.
  JPL's Small-Body Database query API is the easiest to fetch (about 1.6 to 3
  MB of JSON for 20,000 rows) but JPL gives only a citation request and a
  general "Permission to reproduce may be required" statement, no
  redistribution grant.
- **Recommended source: the MPC's `MPCORB.DAT.gz`** (93,910,948 bytes on
  2026-10-21), downloaded once by a developer-run build script, never by the
  app at run time.

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Quotations are copied from the text read
on the day; where a web page splits a sentence across HTML lines, the line
breaks are shown as single spaces, and nothing else is changed (the MPC's own
spelling slips, such as "classses", are kept). The `MPCORB.DAT` header is
quoted in a code block because its line breaks and double spaces are exactly
what the project would have to reproduce.

- **minorplanetcenter.net** pages were fetched directly. Most carry no page
  date. File sizes and dates come from the server's own `Content-Length` and
  `Last-Modified` headers (HEAD requests, no download). The `/data` page
  refused a plain scripted request (HTTP 403) and was read with a browser-like
  request.
- **The MPC documentation hub** (docs.minorplanetcenter.net) was read through
  its own search index file (last modified 2026-10-06 per the server).
- **The Internet Archive** was used only to date a wording change on the MPC's
  policy page.
- **ssd.jpl.nasa.gov and ssd-api.jpl.nasa.gov** were fetched directly; a few
  small API queries (largest 162 KB) were made one at a time with a named
  User-Agent, as JPL's fair-use policy asks.
- **US statutes** were read from the US Code, 2024 Edition, on govinfo.gov.
- **No large data file was downloaded.** Headers and formats were checked with
  byte-range requests of the first 8 KB, 4 KB and 1 KB of the files.

---

## 1. The MPCORB page describes the file and its subsets, and says nothing about terms

Source: https://minorplanetcenter.net/iau/MPCORB.html (no date shown on the
page; read 2026-10-21).

> The MPC Orbit (MPCORB) database contains orbital elements of minor planets that have been published in the Minor Planet Circulars, the Minor Planet Orbit Supplement and the Minor Planet Electronic Circulars.

> This includes all minor planet orbits excluding comets.

> The compressed file is about 30% the size of the uncompressed file.

> These are special classses of objects that are a subset of MPCORB.DAT.

The subsets it links are `NEA.txt`, `PHA.txt`, `Distant.txt` ("TNOs, Centaurs
and SDOs") and `Unusual.txt`. There is no main-belt, trojan or Hilda subset,
and no numbered-only file.

**Negative finding:** the page carries no statement on copyright, licence,
attribution or redistribution. The terms are in the file itself (finding 2)
and on a separate policy page (finding 3).

## 2. The MPCORB.DAT header grants software inclusion, on three conditions

Source: the first 8,192 bytes of
https://minorplanetcenter.net/iau/MPCORB/MPCORB.DAT (byte-range request; the
file's `Last-Modified` was Wed, 21 Oct 2026 13:05:28 GMT; the header text
itself carries no date).

```
   Software programs may include this datafile amongst their datasets, as
   long as this header is included (it is acceptable if it is contained
   in a file separate from the actual data) and that proper attribution
   to the Minor Planet Center is given.  Credit to the individual orbit
   computers is implicit by the inclusion of a reference and the name of
   the orbit computer on each orbit record.  Information on how to obtain
   updated copies of the datafile must also be included.
```

and, further down the same header, the update address it supplies itself:

```
New versions of this file, updated on a daily basis, will be available at:

          https://www.minorplanetcenter.org/iau/MPCORB/MPCORB.DAT
```

The header runs from the line `MINOR PLANET CENTER ORBIT DATABASE (MPCORB)`
to the dashed rule under the column legend: 43 lines on 2026-10-21, with the
first orbit record, `(1) Ceres`, on line 44. It also describes the file's
scope:

> This file contains published orbital elements for all numbered and unnumbered multi-opposition minor planets for which it is possible to make reasonable predictions.

**What this settles:** the MPC permits software to carry MPCORB data. The
three conditions are the header, attribution to the MPC, and how to get
updates. The "Credit to the individual orbit computers" sentence describes how
credit is given (through the `Reference` and `Computer` columns); it is not
worded as a condition.

**What it does not settle:** it speaks of "this datafile". It does not say
whether a subset, or a file with columns dropped, is still covered.

**Negative finding:** the subset files do not carry the header. The first
bytes of `Distant.txt` and `NEA.txt` begin straight with orbit records
(`(944) Hidalgo` and `(433) Eros`). The header has to be taken from
`MPCORB.DAT`.

## 3. The MPC's website policy: data files may be redistributed if the source is clearly specified

Source: https://www.minorplanetcenter.net/iau/WWWPolicy.html , titled "Website
Use Policy" (no date shown; read 2026-10-21 at about 18:05 UTC, and again at
18:12 UTC from `minorplanetcenter.net` without the `www.`, with the same
wording).

The permission, a top-level bullet of its own:

> Data files are copyrighted entities and may be redistributed as long as the source for the data is clearly specified.

The general copyright statement it sits under:

> Copyright exists in all original material (i.e., nearly everything) on the MPC pages

> You should never assume that material on the Internet is in the public domain, unless this fact is clearly stated.

The licence for circulars, a separate top-level bullet whose nested items
include a broader-sounding line:

> Some MPC Circulars and other products are made freely accessible at this site only under the following specific non-negotiable licence, which is subject to amendment at any time:

> Redistribution of material posted on this website is not permitted in any form.

> Any free availability of material at this site is subject to withdrawal at any time without warning.

And a ban that names circulars and web pages, not data files:

> Inclusion of circulars or MPC web pages in products of any description (including, but not limited to CDs and magazines), whether or not a charge is made for the product, is strictly prohibited unless express written permission has been obtained in advance.

**How the page is built:** in the HTML, "Redistribution of material posted on
this website is not permitted in any form." is a nested item inside the
circulars-licence bullet, and "Data files are copyrighted entities and may be
redistributed..." is the next top-level bullet, at the same level as the
circulars licence. The page does not link to any Smithsonian, Harvard or IAU
terms.

**What this settles:** on the day, the MPC permitted redistribution of data
files on the single condition that the source is clearly specified, and
asserted copyright in them. Nothing read says MPC data is public domain or
under any open licence.

## 4. The policy page changed wording between 25 September and 21 October 2026

Source: Internet Archive captures of the same page:
https://web.archive.org/web/20260925233534/https://www.minorplanetcenter.net/iau/WWWPolicy.html
(captured 2026-09-25 23:35 UTC), and the same text in captures of 2011-02-03,
2022-03-13, 2023-11-29 and 2025-03-12.

The earlier wording of the data-files clause:

> Data files, whether freely-available (such as the MPCORB orbit database) or via subscription (such as the databases of observations), are also copyrighted entities.

> Subscription datasets are for your own personal scientific use and must not be redistributed in any form.

> Freely-available datasets may be redistributed as long as the source for the data is clearly specified.

And the earlier wording of the redistribution ban, which named circulars:

> Redistribution of the circulars posted on this web site is not permitted in any form.

**What moved:** the old page named MPCORB as a freely available dataset and
said such datasets may be redistributed with the source specified. The new
page drops the subscription distinction and says all "Data files" may be
redistributed with the source specified. The ban that used to say "the
circulars posted on this web site" now says "material posted on this website",
inside the licence for "MPC Circulars and other products". The permission for
MPCORB is the same in substance before and after; only the scope of the
nearby ban became less clear.

**Sources disagree, and how that was weighed:** a web-search summary returned
during this research described the policy with the subscription wording. That
matches the archived old page, not the live one, so the live page read
directly on 2026-10-21 is what this document relies on.

## 5. The MPC's data page: the file list, and the acknowledgement it asks of research publications

Source: https://minorplanetcenter.net/data , "Data Available from the Minor
Planet Center" (page undated; its table gives each file's "Last Update (UT)",
2026/10/21 13:05 for `MPCORB.DAT`). A plain scripted request was refused with
HTTP 403; a browser-like request was answered.

> If using these or any other data files, or a webtool from the MPC, for your research publication, we would appreciate it if you could thank us in the acknowledgements section.

> This research has made use of data and/or services provided by the International Astronomical Union's Minor Planet Center.

This is a request ("we would appreciate it"), worded for research
publications, not a licence condition. It is a ready-made credit line in the
MPC's own words.

Orbit files listed, with the sizes the MPC's server reported on 2026-10-21:

| File | What it holds (the MPC's words in quotation marks) | Size |
|---|---|---|
| `/iau/MPCORB/MPCORB.DAT` | "Orbits for all asteroids in the MPC database" | 319,912,690 bytes |
| `/iau/MPCORB/MPCORB.DAT.gz` | the same, gzip | 93,910,948 bytes |
| `/Extended_Files/mpcorb_extended.json.gz` | the same plus derived fields, JSON | 182,539,648 bytes |
| `/Extended_Files/mpcorb_extended.dat.gz` | the same, fixed-width with extra columns | 114,114,114 bytes |
| `/iau/MPCORB/Distant.txt` | "Orbits for TNOs, Centaurs and SDOs" | 1,687,336 bytes |
| `/Extended_Files/distant_extended.json.gz` | the same, JSON | 975,824 bytes |
| `/iau/MPCORB/NEA.txt` | "Orbits for Near Earth Asteroids (NEAs)" | 8,698,144 bytes |
| `/iau/MPCORB/Unusual.txt` | "Orbits for asteroids with e ≥ 0.5 or q > 6 AU" | 8,697,941 bytes |

The MPC offers no file of only main-belt objects, only trojans, only Hildas or
only numbered objects (a guessed `numbered_extended.json.gz` returned 404).

## 6. The MPC's documentation hub: a citation policy for its lists, and a tutorial doing exactly this

Source: https://docs.minorplanetcenter.net/mpc-ops-docs/data-and-services/lists/
(read through the hub's search index, last modified 2026-10-06).

> Users of any of these lists are reminded that the source of the data is the Minor Planet Center. The relevant URL should be cited, along with a reference to the MPC itself.

This is written for the MPC's "Lists and Plots", not MPCORB, but it shows what
the MPC means by citing it: the URL and the MPC.

Source: https://docs.minorplanetcenter.net/tutorials/notebooks/mpc_tutorial_orbit_plotting/
(same index). The MPC's own tutorial downloads `MPCORB.DAT`, subsamples it,
and plots the result:

> a position snapshot that reveals the main belt, the Hilda triangle, and the Jupiter Trojan clouds

This is practice, not permission: it shows the use this project has in mind
is one the MPC itself teaches.

## 7. The extended format manual: the field names, and no terms

Source: https://minorplanetcenter.net/Extended_Files/Extended_MPCORB_Data_Format_Manual.pdf
("Last updated: 2024/05/08", by J.L. Galache; 312,926 bytes, read in full).

It gives the MPCORB column layout (Table 1) and the JSON attributes (Table 4)
used in "How the data would be fetched" below. On the orbit-type flags in
MPCORB columns 162 to 165, the values that matter here are 8 (Hilda), 9
(Jupiter Trojan) and 10 (Distant object), with this caution:

> Note that the orbit classification is based on cuts in osculating element space and is not 100% reliable.

**Negative finding:** the manual says nothing on copyright, licence or
redistribution.

## 8. An older CBAT/MPC policy forbids redistributing the printed catalogues, and says single elements are not copyrighted

Source: https://tamkin3.eps.harvard.edu/CatalogueDistPolicy.html (the Central
Bureau for Astronomical Telegrams' server at Harvard; no date shown; the same
page at https://minorplanetcenter.net/iau/CatalogueDistPolicy.html returned
404).

What it covers:

> The following policy statement and statement of copyright applies to the distribution of the printed and electronic forms (including diskettes) of the Catalogue of Cometary Orbits, Catalogue of High-Precision Orbits of Minor Planets and Catalogue of Orbits of Unnumbered Minor Planets issued by the Central Bureau for Astronomical Telegrams (CBAT) and Minor Planet Center (MPC), as well as datasets obtained from the Computer Service and Extended Computer Service:

What it forbids:

> Catalogues are intended for personal (scientific) use only and may not be redistributed.

> Inclusion of CBAT/MPC datasets in third-party collections (such as, but not limited to, data sets supplied on diskette or CD-ROM with planetarium or sky-simulation software) is not permitted.

And what it says about copyright in single orbits (the space before the comma
is in the original):

> The collections of orbital elements are copyrighted , whereas the individual orbital elements are not.

**How this fits:** this page names specific paid catalogues and the old
Computer Service, not MPCORB, which carries its own permission (finding 2) and
is described on the current policy page (finding 3). It is recorded because it
is the clearest "no" in the MPC's history, and because its last sentence is the
MPC's own view that a single orbit is not copyrighted. It is a reason not to
take orbits from those catalogues.

## 9. US law: copyright does not extend to a discovery

Source: 17 U.S.C. 102(b), US Code 2024 Edition,
https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-chap1-sec102.htm

> (b) In no case does copyright protection for an original work of authorship extend to any idea, procedure, process, system, method of operation, concept, principle, or discovery, regardless of the form in which it is described, explained, illustrated, or embodied in such work.

This is background for finding 8's distinction between single orbits and
collections. The project does not need to rely on it, because the MPC grants
permission for the use anyway.

## 10. JPL's Solar System Dynamics site asks for a citation and grants nothing more

Source: https://ssd.jpl.nasa.gov/about/ , "Acknowledgements" (no date shown).

> For referencing the data downloaded from the SSD website, please use the following citation:

> “Solar System Dynamics. (Downloaded Year, Month, Date). (Title of the Page). https://ssd.jpl.nasa.gov”

Source: https://www.jpl.nasa.gov/caltechjpl-privacy-policies-and-important-notices ,
"Copyright Status" (linked as "Privacy" from every SSD page footer; no date
shown).

> Unless otherwise noted, Web sites in the domain jpl.nasa.gov are managed by the California Institute of Technology

> All documents available from this server may be protected under the U.S. and Foreign Copyright Laws. Permission to reproduce may be required.

Source: https://www.jpl.nasa.gov/jpl-image-use-policy (the other SSD footer
link; no date shown). Its permission is worded for images and video, not data:

> Unless otherwise noted, images and video on JPL public web sites (public sites ending with a jpl.nasa.gov address) may be used for any purpose without prior permission

For context, the US Code says (17 U.S.C. 105(a) and the definition in 17
U.S.C. 101, 2024 Edition, govinfo.gov):

> Copyright protection under this title is not available for any work of the United States Government

> A "work of the United States Government" is a work prepared by an officer or employee of the United States Government as part of that person's official duties.

**What this settles:** JPL asks for a citation in a set form. **Negative
finding:** nothing read on JPL's sites grants permission to redistribute
Small-Body Database data, and JPL's own statement says permission "may be
required". Because JPL says its sites are managed by Caltech, JPL's data is
not plainly a "work of the United States Government"; whether a given SBDB
extract is copyrighted at all, no source read settles.

## 11. JPL's API terms: build-time fetching only, one request at a time, a named User-Agent

Source: https://ssd-api.jpl.nasa.gov/ , "SSD API Service - Fair Use Policy"
(no date shown; the website asks visitors to tick "I Accept the Terms Above").

> You may not embed these APIs in your website (per NASA CORS policy).

> Applications making automated connections must provide a valid, application-specific User-Agent request header that includes correct product name, version, and contact information.

> You agree to submit only one API request at a time (no simultaneous requests).

> Where practical, applications should cache and reuse results rather than repeatedly requesting unchanged or predictably changing data.

> You understand that API data formats can change without notice.

These are terms of using the service, not of reusing its output, and they
settle the method: if JPL is used, a developer script fetches once and the
result is committed; neither the website nor the app calls the API.

## 12. JPL's SBDB query API: what it returns, and that part of it is the MPC's own orbits

Source: https://ssd-api.jpl.nasa.gov/doc/sbdb_query.html ("Version: 1.0 (2021
August)") and https://ssd-api.jpl.nasa.gov/doc/sbdb_filter.html (same version
line).

> Limit data to the first N results (where N is the specified number and must be an integer value greater than zero).

The orbit classes that matter here, from the filter document:

> Asteroids with orbital elements constrained by (2.0 au < a < 3.2 au; q > 1.666 au)

(MBA, main belt), and `OMB` (3.2 to 4.6 au, which holds the Hildas), `TJN`:

> Asteroids trapped in Jupiter’s L4/L5 Lagrange points (4.6 au < a < 5.5 au; e < 0.3)

and `TNO`:

> Objects with orbits outside Neptune (a > 30.1 au)

JPL has no Hilda class; a Hilda selection has to be made with a custom
constraint (`sb-cdata`).

The `source` field shows that some of JPL's orbits are the MPC's:

> code indicating the source of the orbit: ORB=”JPL orbit file”, MPC:mpn=”MPC numbered asteroid orbit file”, MPC:mpu=”MPC unnumbered asteroid orbit file”, MPC:mp1=”MPC single opposition short-arc orbit file”

Counts the API returned on 2026-10-21 (count-only queries, no fields):

| Query | Count |
|---|---|
| `sb-class=MBA` | 1,393,683 (825,292 of them numbered) |
| `sb-class=OMB` | 50,909 |
| `sb-class=TJN` | 16,412 |
| `sb-class=TNO` | 7,302 |
| `sb-class=CEN` | 1,062 |
| Hilda-like box: a 3.7 to 4.2 au, e < 0.3, i < 20 degrees (bounds chosen for this check, not taken from a source) | 7,404 |
| asteroids with `source` = `ORB` (JPL's own fit) | 1,520,701 |
| asteroids with `source` starting `MPC` | 55,250 |

So drawing from JPL does not fully escape the MPC: a filter
`source|EQ|ORB` keeps only JPL's own fits, and those are still computed from
observations the MPC collects.

## 13. Lowell Observatory's astorb: an explicit grant, attribution to its funders

Source: https://asteroid.lowell.edu/astorb/ , "Acknowledgment and Attribution"
(no date shown; on the day, the page carried a banner that the astorbDB
database was unavailable).

> The resources to support astorb.dat were originally provided by NASA grant NAG5-4741 (PI E. Bowell) and the Lowell Observatory endowment, and more recently by NASA PDART grant NNX16AG52G (PI N. Moskovitz). astorb.dat may be freely used, copied, and transmitted provided attribution is made to the aforementioned funding sources.

> Each orbit, based on astrometric observations downloaded from the Minor Planet Center, occupies one 266-column record.

Source: https://ftp.lowell.edu/pub/elgb/astorb.html , the older page ("Last
updated 7 Dec 2021 at 04:12:50 U.T."), which words the condition differently:

> astorb.dat may be freely used, copied, and transmitted provided attribution to Dr. Edward Bowell and the aforementioned funding sources is made.

**Sources disagree (mildly):** the current page asks for attribution to the
funding sources; the 2021 page also names Dr. Edward Bowell. Crediting both
satisfies both.

Size: `https://ftp.lowell.edu/pub/elgb/astorb.dat.gz` reported 112,227,037
bytes (`Last-Modified` Wed, 21 Oct 2026 05:16:18 GMT); the page gives about 423
MB uncompressed.

**What this settles:** Lowell's grant is the most explicit of the three. Its
orbits are Lowell's own fits to MPC observations. It is the best fallback if
the MPC's terms ever close.

---

## How the data would be fetched

### Recommended: the MPC's MPCORB.DAT.gz

- **URL:** https://minorplanetcenter.net/iau/MPCORB/MPCORB.DAT.gz (the header
  inside gives https://www.minorplanetcenter.org/iau/MPCORB/MPCORB.DAT as the
  update address).
- **Size:** 93,910,948 bytes compressed, 319,912,690 uncompressed (server
  headers, 2026-10-21). Updated daily. Roughly 1.5 million orbits.
- **Format:** gzip of fixed-width ASCII, one orbit per 202-character line, a
  43-line header first, the three sections (numbered, unnumbered
  multi-opposition, one-opposition) separated by blank lines.
- **Columns (Table 1 of the format manual, 1-based):**

  | Element | Columns | Note |
  |---|---|---|
  | H | 9 to 13 | absolute magnitude |
  | epoch | 21 to 25 | packed date, ".0 TT"; the records read on 2026-10-21 carry `K2669`, 2026 June 9.0 (JD 2461200.5) |
  | M (mean anomaly) | 27 to 35 | degrees |
  | peri (argument of perihelion) | 38 to 46 | degrees, J2000.0 |
  | node (longitude of ascending node) | 49 to 57 | degrees, J2000.0 |
  | i (inclination) | 60 to 68 | degrees, J2000.0 ecliptic |
  | e | 71 to 79 | |
  | a | 93 to 103 | AU |
  | Reference | 108 to 116 | carries the per-orbit credit (finding 2) |
  | Computer name | 151 to 160 | carries the per-orbit credit |
  | 4-hexdigit flags | 162 to 165 | low 6 bits = orbit type: 8 Hilda, 9 Jupiter Trojan, 10 Distant object |
  | readable designation | 167 to 194 | e.g. `(1) Ceres` |

- **How:** a developer-run script downloads the file once, keeps about 20,000
  rows sampled by class (main belt by a range, Hildas and trojans by flag value
  8 and 9, distant objects by flag 10 or by a > 30 au), writes the derived
  file, and copies the 43 header lines byte for byte into a separate text file
  beside it. The app never contacts the MPC.

The JSON alternative, https://minorplanetcenter.net/Extended_Files/mpcorb_extended.json.gz
(182,539,648 bytes), holds the same orbits as a JSON array with keys `a`, `e`,
`i`, `Node`, `Peri`, `M`, `Epoch` (Julian Date), `H`, `Ref`, `Computer` and a
ready-made `Orbit_type` (`"MBA"`, `"Hilda"`, `"Jupiter Trojan"`, `"Distant
Object"` and others). Two things observed: the file is twice the download, and
it carries no header text (its first bytes are `[` and the Ceres record), so
the header would still have to come from `MPCORB.DAT`. The manual spells the
key `orbit_type`; the file on the day spelled it `Orbit_type`.

For the distant objects alone,
https://minorplanetcenter.net/Extended_Files/distant_extended.json.gz is
975,824 bytes.

### Alternative: JPL's SBDB query API (if the MPC route is ever closed)

- **Endpoint:** `https://ssd-api.jpl.nasa.gov/sbdb_query.api`, one request at a
  time, with a User-Agent naming the product, version and a contact.
- **Main belt, brightest numbered 15,000:**
  `https://ssd-api.jpl.nasa.gov/sbdb_query.api?fields=spkid,pdes,a,e,i,om,w,ma,epoch,H,class&sb-ns=n&sb-class=MBA&sort=H&limit=15000&full-prec=1`
  (`sb-ns=n` matters: without it, sorting on H puts objects with no H first).
- **Trojans, TNOs:** the same with `sb-class=TJN&limit=3000` and
  `sb-class=TNO&limit=2000` (drop `sb-ns=n` for TNOs to include unnumbered
  ones; sort then on another field).
- **Hildas:** add
  `sb-cdata=%7B%22AND%22%3A%5B%22a%7CRG%7C3.7%7C4.2%22%2C%22e%7CLT%7C0.3%22%2C%22i%7CLT%7C20%22%5D%7D`
  (the URL-encoded form of `{"AND":["a|RG|3.7|4.2","e|LT|0.3","i|LT|20"]}`;
  these bounds are this document's choice, not a sourced definition).
- **Only JPL's own fits:** add `{"AND":["source|EQ|ORB"]}` to the constraint.
- **Size, measured:** 2,000 rows of `spkid,a,e,i,om,w,ma,epoch,H` at the
  default reduced precision came to 162,416 bytes (about 81 bytes a row, so
  about 1.6 MB for 20,000); 200 rows at `full-prec=1` came to 30,103 bytes
  (about 150 bytes a row, so about 3.0 MB for 20,000).
- **Format:** JSON, `{"signature":{...},"fields":[...],"data":[[...],...],"count":N}`,
  every number as a string.
- **Mapping:** `a` to a, `e` to e, `i` to i, `om` to node, `w` to peri, `ma` to
  M, `epoch` (Julian day, TDB) to epoch, `H` to H.

---

## What is still unknown, and what would settle it

1. **Whether "this datafile" and "Data files" cover a 20,000-row extract with
   columns removed.** Neither MPC text mentions subsets. *Settles it:* one
   message to the MPC through its helpdesk (linked from every MPC page) asking
   exactly that, and saving the reply under this folder. Cost: one message and
   a wait of days to weeks. Until then, meeting every condition in full is the
   project's mitigation.
2. **Whether the new line "Redistribution of material posted on this website
   is not permitted in any form" reaches data files.** The page was rewritten
   within the four weeks before this research and may still be settling.
   *Settles it:* the same helpdesk message, and re-reading the page before the
   derived file ships.
3. **When the policy page changed, exactly.** Known only to be after the
   2026-09-25 capture. *Settles it:* the MPC, or later archive captures. It
   matters only if someone asks which wording applied on a given date.
4. **Whether JPL's SBDB extracts are copyrighted at all.** JPL is run by
   Caltech, and JPL says permission "may be required". *Settles it:* an email
   to JPL's SSD API contact (the address is assembled by script on the API
   pages: `contact-ssd-api` at `jpl.nasa.gov`). Only needed if JPL is used.
5. **Whether any database right outside the US applies.** Not researched. The
   MPC's own permission makes this moot as long as its conditions are met; it
   would matter only for a use the MPC has not permitted.
6. **The "new downloading restrictions" note.** The MPC's software-elements
   page (https://minorplanetcenter.net/iau/Ephemerides/SoftwareEls.html) says
   "Please ensure you've read the note in /iau/MPCStatus.html on new
   downloading restrictions." That address now redirects to a status log
   (https://minorplanetcenter.net/mpcops/mpcstatus/) in which no such note was
   found. *Settles it:* the helpdesk message again. A single download of one
   file by a developer is unlikely to be what it restricts, but that is a
   guess.

## Sources that could not be reached, or were deliberately not downloaded

- https://minorplanetcenter.net/data and http://mpcweb1.cfa.harvard.edu/data :
  HTTP 403 to a plain scripted request. The first was then read with
  browser-like request headers; the second was not retried.
- https://minorplanetcenter.net/iau/MPCORB/ : "Listing of Directories is not
  Permitted". Hoped for the exact file list; the data page supplied it.
- https://minorplanetcenter.net/iau/CatalogueDistPolicy.html and the `www.`
  form: 404. The policy was read on CBAT's Harvard server instead (finding 8).
- https://minorplanetcenter.net/iau/MPCStatus.html : redirects to a status log;
  the downloading-restrictions note was not found (unknown 6).
- https://minorplanetcenter.net/Extended_Files/numbered_extended.json.gz : 404.
  A guess at a numbered-only file; none exists.
- The MPC documentation hub was read through its search index, not page by
  page; a page whose text is not in the index would have been missed.
- The astorb paper (https://doi.org/10.1016/j.ascom.2022.100661) was not read.
  A web-search summary (marked as such, not checked) said the article is under
  CC BY-NC-ND; that would be the article's text, not the data.
- **Not downloaded on purpose:** `MPCORB.DAT` (319.9 MB), `MPCORB.DAT.gz`
  (93.9 MB), `mpcorb_extended.json.gz` (182.5 MB), `astorb.dat.gz` (112.2 MB)
  and `Distant.txt` (1.7 MB). Only server headers and the first 1 to 8 KB were
  read.

## What it means for this project

### Certain

- On 2026-10-21 the MPC permitted software programs to include MPCORB data,
  with three written conditions: include the header (a separate file is
  allowed), give proper attribution to the Minor Planet Center, include how to
  get updated copies (finding 2).
- On the same day the MPC's website policy permitted data files to be
  redistributed "as long as the source for the data is clearly specified", and
  the earlier wording permitted the same for MPCORB by name (findings 3 and 4).
- The MPC asserts copyright in its data files. Nothing read puts MPCORB in the
  public domain or under CC0 or any open licence, so the derived file cannot
  simply join the repository's CC0. It is third-party data with conditions,
  and belongs in `LICENSES.md` and in `data/credits.ron` with
  `attribution_required: true` and a `shown_in` naming the map surface that
  draws the belts (the credits unit test in `src/credits.rs` requires that).
- Neither MPC text addresses subsets or dropped columns (findings 2 and 3).
- No MPC text read limits use to non-commercial projects, charges a fee, or
  requires registration for MPCORB.
- The old CBAT/MPC catalogue policy forbids redistributing the paid catalogues
  and Computer Service datasets, including in "planetarium or sky-simulation
  software" (finding 8). Orbits must not be taken from those.
- JPL asks for a citation and grants no redistribution permission in anything
  read; its API may not be called from the website, only by a build-time
  script with a named User-Agent, one request at a time (findings 10 and 11).
- Lowell permits free use, copying and transmission of astorb.dat with
  attribution to its funding sources (and, per its 2021 page, Dr. Edward
  Bowell) (finding 13).

### Judgement calls

- **Use the MPC, not JPL.** The MPC is the IAU's official source, the design
  document already names it (`docs/design/maps-zoom-to-anything.md`, "Asteroid
  belts from real orbits"), and it is the only one of the three that both
  grants redistribution in writing and is the original. JPL is easier to query
  but rests on a citation request plus "permission may be required". Lowell is
  the fallback.
- **Treat the 20,000-row extract as covered, and meet every condition in full
  anyway.** Concretely:
  1. Ship the MPCORB header, the 43 lines from the downloaded file copied byte
     for byte by the build script (never retyped), as a text file beside the
     derived data. That carries the MPC's own words and the update address.
  2. Keep the `Ref` and `Computer` values on each row. The header says that is
     how the orbit computers are credited; it costs about 20 bytes a row,
     roughly 0.4 MB for 20,000 rows, and removes any argument that the extract
     stripped the credit.
  3. Put a credit where the belts are drawn and in Settings > Credits, naming
     the source and the date downloaded. Suggested notice, built from the
     MPC's own wording: "Asteroid orbits: Minor Planet Center, International
     Astronomical Union (MPCORB, downloaded YYYY-MM-DD). Updated copies:
     https://www.minorplanetcenter.net/iau/MPCORB/MPCORB.DAT". The MPC's own
     sentence, "This research has made use of data and/or services provided by
     the International Astronomical Union's Minor Planet Center.", can sit in
     Credits as well; it costs nothing and is what the MPC asks of
     publications.
  4. Record the source in `LICENSES.md`, saying the file is not CC0 and that
     anyone redistributing it must keep the header file and the credit.
- **Read the new "Redistribution of material posted on this website" line as
  part of the circulars licence.** It is nested under "Some MPC Circulars and
  other products", the data-files permission is a separate bullet beside it,
  and until weeks ago the same ban said "the circulars". A reader who takes it
  literally would also have to read it as overriding the header that travels
  in the file, which says the opposite. Send the helpdesk message (unknown 1)
  before the file ships so the question does not rest on this reading.
- **Refresh rarely, by hand.** The point clouds show the shape of the
  populations, not where any one asteroid is tonight; a file regenerated a few
  times a year is plenty, and each regeneration is one 94 MB download by a
  developer. Each refresh copies the header afresh, in case its wording
  changes.

**This is a reading of public sources by an AI, not legal advice, and nobody
who wrote it is a lawyer.**
