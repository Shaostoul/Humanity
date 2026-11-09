# Ingredients and labels: a real-life ingredient record, and a label reader that runs on your own device

**Status (2026-11-08): PLANNED, nothing built.** The whole feature is planned here with all its
parts (CLAUDE.md, "Plan the whole feature, build it in rungs"), then cut into rungs (section 13),
smallest first. What the project may ship, under which terms, and what it must never claim, rests on
the dated finding
[`docs/reference/findings/2026-11-08-food-data-sources.md`](../reference/findings/2026-11-08-food-data-sources.md)
(cited below as "the finding", with its F-numbers).

## 0. In plain words

- HumanityOS gets **one record per ingredient**, the way it has one row per plant and per item:
  its names in every language the app carries, what it is made of, which allergen group the law in
  each country puts it in, its additive code (E 102, INS 102, "Yellow 5"), its nutrition, how to
  keep it and how it spoils, what can stand in for it, which crop in the game it comes from, and
  **the known risks**: who is at risk (pregnancy, young children, a condition, an allergy), at what
  amount, and which authority says so, with the date we read it.
- **A label reader.** Point a phone at an ingredient list, or give the desktop app a photo. The words
  are read **on your own device** (nothing is uploaded), matched against the records, and anything on
  your own avoid list or in an allergen group is pointed out, with what the words were and what we
  could not read.
- **It never says a food is safe.** It says what the words on the label contain, as far as it could
  read them, and what an authority says about each ingredient. Labels can be wrong, recipes change,
  "may contain" means different things in different countries, and a camera can misread. The screen
  says so every time, in plain words.
- **Every health claim is checked twice before it ships.** The project's own research once had 48 of
  60 records refuted on review, almost all on the lines that could hurt someone. So each risk line
  carries its source, quoted, and passes a second, adversarial check against that source before
  anyone sees it.
- **Plain words first, everything on request.** "Contains sesame. Sesame is on your avoid list." is
  the default; the law in each country, the source's words, the amounts and the review history are
  one click away.

## 1. Why, in the operator's words (2026-11-08)

On reading about OCR: "Oh, is this how they do those ingredient list checkers for like making sure
food in a store is safe? This is something we want to consider since we're actually putting all the
various ingredients in our app so people can cook in real life. Might as well let people know all
the data, including health risks for all ingredients."

The two-realities rule (`docs/design/two-realities.md`) puts this on the real-life side: the app's
pages are your real life, and the game is a place you walk into. The ingredient records serve real
cooking (How-to's `food_cooking` recipes name what they use) and link out to the game's crops, so a
player who grows sesame in the game can read what sesame is in real life.

## 2. What exists today (read 2026-11-08)

Enhance, never rebuild. What is already in the repository:

| What | Where | What it gives this feature |
|---|---|---|
| **A prototype ingredient and product schema** (v0.116 to v0.117, April 2026) | `data/items/foods/SCHEMA.md`, `ingredients.csv` (33 rows), `ingredients/` (sugar, caffeine, cocoa), `items/` (two branded products) | The right shape in three layers (compound, ingredient, product with a dated formulation history, risks derived at query time), and the right idea that improving "caffeine" once improves every product containing it. No code reads it. Its health lines fail today's standard: sources such as "FDA caffeine guidance" cannot be opened, and its Yellow 5 line is refuted by EFSA's own list (the finding, "A check the project's own prototype fails"). Rung 1 replaces it. |
| **Crop nutrition from USDA** | `data/food/crop_nutrition.ron` (energy and macros, FDC ids per row), `data/food/crop_micronutrients.json` (the rest, SR Legacy April 2018, with refuse and cooking retention) | The nutrition numbers for every food crop, keyed by `data/plants.csv` id with the FoodData Central row named. The ingredient record points at the same FDC rows rather than copying numbers. |
| **What a person needs a day** | `data/food/nutrient_requirements.json` (US DRIs, ULs and which apply to food) | The upper limits a risk line about a nutrient can cite. |
| **The game's food** | `data/food_system.ron` (nutrition profiles, FoodKeeper spoilage, raw-eating risk), `data/food/item_profiles.ron` (which items are food), `crates/humanity-systems/src/food.rs` | The game's side. Its FoodKeeper figures and FDC rows are the same sources the real-life record uses, so the two never disagree. |
| **Chemistry** | `data/chemistry/compounds.csv`, `toxins.csv` | The compound layer the prototype linked to (`chemistry_compound_id`). |
| **The source registry** | `data/sources/registry.json`, `schemas/source.toml` | One list of authorities with `licence` and `use` (`verbatim` may ship as text, `facts` is restated in our words). `fda`, `usda`, `usda-ars`, `usda-fsis`, `who`, `fao`, `health-canada` are there; FoodData Central, Open Food Facts, EFSA, the European Commission, Codex and ANSES are not yet. |
| **How-to's review pattern** | `docs/design/how-to.md`, "Sources and review" | The review scale (`absent`, `stub`, `sourced`, `verified`), a review hash that binds the badge to the words it checked, and a person's review for anything `lethal`. This feature reuses it exactly. |
| **Real cooking** | `data/how_to/` (`food_cooking` category; "Bake a loaf of bread" planned), `materials` naming `data/items.csv` ids | Where real recipes live. An ingredient record links to the items it is, and a how-to's "What you need" links to the record. |
| **The Real Skills curriculum** | `data/curriculum/syllabus.json` ("Keeping Food", "Nutrition that keeps you working", "Waterborne and foodborne illness", "Botulism, specifically") | Lessons that can cite the records. |
| **Private health fields** | Website Profile's free-text "Allergies" box (`web/pages/profile.html`, kept in the browser only); the desktop's "Body & Measurements" (memory only, `src/gui/pages/profile.rs`); `docs/design/privacy-notice.md` ("No health or body data leaves the device") | The avoid list grows from the website's Allergies box, and the desktop gains the same (it has no allergies field: a parity gap today). |
| **The 2026-11-08 AI tools scan** | `docs/reference/research/2026-11-08-ai-tools/B-speech-text-vision-agents.md` | Unlimited OCR 2 (MIT, about 3B parameters, NVIDIA only): right for whole scanned documents, too heavy for a label. |

`docs/FEATURES.md` and `docs/STATUS.md` list no allergen, label or real-life ingredient feature.

## 3. The rules this follows

- **Infinite-of-x** (`docs/design/infinite-of-x.md`): every ingredient, allergen group, country's
  law, risk line and substitute is data. Adding the ten-thousandth ingredient is a new file, not
  code.
- **Two modes** (CLAUDE.md, 2026-09-24): plain by default, full detail on request (section 7).
- **One word source** (CLAUDE.md, 2026-10-22): every word on screen by key, every control with a
  hint, jargon marked `[[term]]` (section 8).
- **Web mirrors native, both gated** (memory `feedback_gate_both_mirrors_or_neither`): the same
  golden label texts and photos run against the desktop and the website; a gate on one is not a gate
  on the other.
- **Protect the user, safe by default** (CLAUDE.md, 2026-10-09): the avoid list never leaves the
  device unless the person seals it into their own backup; the reader errs toward pointing something
  out, never toward silence.
- **Say plainly when the hardware cannot run it** (CLAUDE.md, 2026-11-08): section 5.9.
- **Open source only** (CLAUDE.md, 2026-09-24): open models and engines only (the finding, F14).
- **Teaching data needs a refutation pass** (memory `project_teaching_data_needs_refutation`):
  section 6.
- **Saying what we will not build, and why** (CLAUDE.md, 2026-09-18): section 12.
- **No backwards-compatibility debt before v1.0**: the prototype's files are replaced outright.
- **No caches across runs; in-memory reuse obeys the off switch** (CLAUDE.md, 2026-10-30): the
  matcher's alias index is rebuilt when the data's version changes and is rebuilt every time when
  `HUMANITY_NO_REUSE=1` or the Dev page's "turn off all reuse" is on.

## 4. The ingredient record

### 4.1 Files

```
data/food/
  ingredients/<id>.json        one ingredient per file (snake_case id, like plants.csv and items.csv)
  ingredients/index.json       GENERATED: id, names, aliases, codes, allergen classes, review state;
                               what the matcher and the website read first (like data/how_to/index.json)
  allergen_classes.json        the allergen groups, and which country's law lists which (4.4)
  regimes.json                 the countries and bodies whose law or advice the records cite (us, eu,
                               ca, codex, later jp, cn, uk, au/nz)
schemas/ingredient.toml        the shape, which the checker enforces
scripts/build-ingredients.js   writes index.json; refuses a record the checker refuses
scripts/check-ingredients.js   the checker (section 11); in `just lints` and `just preflight`
```

JSON, not RON, because the website reads these files directly (the label reader runs in the
browser too) and How-to already binds a review hash to a JSON record's canonical CBOR. One file per
ingredient keeps each review bound to one small file and keeps diffs readable.

**Retired in rung 1:** `data/items/foods/` (the prototype). Its three layers survive: the compound
layer stays `data/chemistry/`, the ingredient layer becomes `data/food/ingredients/`, and the product
layer moves to the person's own device (4.11). Its two branded product files and its unsourced
toxicology lines are deleted, not migrated.

### 4.2 The shape, by example

An illustration of the shape, not data: the numbers and wording here are not reviewed and must not be
copied into a record.

```json
{
  "id": "sesame_seed",
  "kind": "food",
  "names": {
    "en": { "name": "Sesame seeds",
            "aliases": ["sesame", "sesame seed", "benne seed", "gingelly"],
            "plural": "sesame seeds" },
    "fr": { "name": "Graines de sésame", "aliases": ["sésame"] },
    "es": { "name": "Semillas de sésamo", "aliases": ["sésamo", "ajonjolí"] },
    "ja": { "name": "ごま", "aliases": ["胡麻", "ゴマ"] },
    "zh": { "name": "芝麻", "aliases": [] }
  },
  "made_from": [],
  "forms": ["sesame_oil", "tahini", "sesame_flour"],
  "codes": { "fdc": [170150], "e": null, "ins": null, "cas": null, "cnf": null, "ciqual": null },
  "allergens": [
    { "class": "sesame", "how": "is",
      "cites": "allergen_classes.json#sesame" }
  ],
  "status": [],
  "nutrition": { "fdc": 170150, "basis": "per 100 g, raw" },
  "risks": [
    {
      "id": "sesame_allergy",
      "kind": "allergy",
      "who": [{ "group": "allergy", "class": "sesame" }],
      "amount": { "kind": "no_threshold_set", "note_key": "risk.amount.no_threshold_us" },
      "severity": "lethal",
      "words": {
        "en": { "plain": "Can cause a severe allergic reaction in people allergic to [[sesame]].",
                "detail": "..." }
      },
      "sources": [
        { "id": "fda",
          "where": "https://www.fda.gov/food/food-labeling-nutrition/food-allergies",
          "published": "2026-03-11", "read": "2026-11-08",
          "quote": "the words that carry the meaning, exactly" }
      ],
      "review": { "status": "sourced", "hash": "<BLAKE3>", "checks": [], "person": null }
    }
  ],
  "storage": {
    "foodkeeper": null,
    "keeps": [ { "where": "pantry", "days": { "min": null, "max": null } } ],
    "spoils_by": ["rancid smell"],
    "sources": []
  },
  "substitutes": [
    { "id": "sunflower_seed", "for": ["topping", "texture"], "ratio": "1:1",
      "note_key": "substitute.sunflower_for_sesame" }
  ],
  "game": { "plants": ["sesame"], "items": [], "food_profile": null },
  "review": { "status": "sourced", "hash": "<BLAKE3>", "checked": "2026-11-08",
              "by": "refutation pass", "person": null },
  "written_by": "ai",
  "updated": "2026-11-08"
}
```

What each part decides:

### 4.3 Names and aliases in every language

- `names.<lang>` for every language `data/i18n/` carries (today `en`, `es`, `fr`, `ja`, `zh`). A
  missing language falls back to English per field, the same rule as the app's words.
- **`name` is what the screen shows; `aliases` are what a label may print.** The matcher reads both,
  plus `plural`. An alias that maps an ingredient to an allergen group (whey to milk, casein to milk,
  semolina to wheat) is itself a claim that can hurt someone if wrong or missing, so a derived name
  carries its own source and passes the refutation pass (section 6) like a risk line.
- Names in the records are data about real things, in the way `data/items.csv` and
  `data/plants.csv` carry their names; the words of the interface around them come from
  `data/i18n/` by key (section 8).
- Sources for names in other languages: the Canadian Nutrient File is bilingual English and French,
  Ciqual is French, and both may be redistributed with credit (the finding, F12 and F13). Open Food
  Facts' multilingual taxonomies are share-alike and stay out of these CC0 files (F8, F9).

### 4.4 Allergen classes, per country's law

`data/food/allergen_classes.json` holds the groups and, for each, which law lists it:

```json
{
  "classes": [
    { "id": "sesame",
      "law": [
        { "regime": "us", "listed": true, "since": "2023-01-01", "source": { "id": "fda", "...": "..." } },
        { "regime": "eu", "listed": true, "source": { "id": "ec-food", "...": "..." } },
        { "regime": "ca", "listed": true, "source": { "id": "health-canada", "...": "..." } },
        { "regime": "codex", "listed": "unconfirmed", "note": "CXS 1-1985 not read (the finding)" }
      ] },
    { "id": "tree_nuts",
      "members": ["almond", "hazelnut", "walnut", "cashew", "pecan", "brazil_nut", "pistachio", "macadamia"],
      "law": [
        { "regime": "us", "listed": true, "name_the_type": true },
        { "regime": "eu", "listed": true, "members_named": ["almond", "hazelnut", "walnut", "cashew",
          "pecan", "brazil_nut", "pistachio", "macadamia"] }
      ] },
    { "id": "sulphites",
      "law": [
        { "regime": "eu", "listed": true,
          "label_threshold": { "value": 10, "unit": "mg/kg", "or": "10 mg/L",
                               "meaning": "labelling threshold, not a safe dose" } },
        { "regime": "ca", "listed": true },
        { "regime": "us", "listed": false, "note_key": "allergen.us_sulfites_monitored" }
      ] }
  ]
}
```

From the finding: the United States lists nine (F4), the European Union fourteen (celery, mustard,
lupin, molluscs and sulphites among them, F10), Canada its own list (mustard, sulphites, molluscs,
wheat and triticale, F12), Codex the "big-8" plus sulphites as FAO described it in 2022 (F11). The
**default view shows the union** (anything any carried regime lists is pointed out), with the
person's own country marked, because an imported label follows another country's law (question 3).

Rules the matcher takes from the law:

- **Derived ingredients.** "whey (milk)", "lecithin (soy)", "flour (wheat)" are FDA's own examples of
  a declaration (F4). An ingredient `made_from` an allergen source inherits its class unless a
  regime's law exempts it.
- **Exemptions are pointed out, not hidden.** US law exempts highly refined oils from the major
  allergen definition (F4); the record says "made from soy; highly refined oil is exempt from US
  allergen labelling" rather than staying silent, because the person, not the law, decides what
  they avoid.
- **"May contain" is shown, always, as the maker's warning**, never suppressed and never read as
  "contains" or as "does not contain". It is voluntary in the US, risk-assessed in the EU and
  "unregulated in many parts of the world" (F4, F10, F11). The avoid list has a setting, on by
  default, "Treat 'may contain' as 'contains'".

### 4.5 Status, per country, dated

`status` records what a regulator has decided, never what is true about health (an illustration of
the shape; the EU line is not checked):

```json
"status": [
  { "regime": "us", "what": "color_certified", "since": null, "until": "2027-01-15",
    "note_key": "status.red3_revoked_food", "source": { "id": "fda", "...": "..." } },
  { "regime": "eu", "what": "authorised", "code": "E 127", "source": { "id": "ec-food", "...": "..." } }
]
```

`what` comes from a fixed list per regime (US: `food_additive`, `color_certified`,
`color_exempt`, `gras_listed`, `gras_notice_no_questions`, `prior_sanctioned`, `prohibited`,
`delisted`; EU: `authorised`, `not_authorised`, `withdrawn`). An entry with `until` changes on its
own date. **Status and risk are separate on the screen**: FDA revoked Red No. 3 for food from
15 January 2027 "as a matter of law" while saying claims that it puts people at risk "are not
supported by the available scientific information" (F6). A record that showed "banned" as a danger,
or "approved" as a guarantee, would mislead in both directions. GRAS itself is changing (FDA's
proposed rule of 10 August 2026, F5), which is why every entry carries a date.

### 4.6 Nutrition

`nutrition.fdc` names the FoodData Central row (and the basis: raw, cooked, dried); the numbers come
from the same snapshot the crop files already use (`data/food/crop_micronutrients.json` and its
builder), so the real-life page and the game read one source. A food FDC does not carry (a regional
dish, a Canadian or French food) names its Canadian Nutrient File or Ciqual row instead, with credit
(F12, F13). Retention after cooking comes from the USDA retention table already used.

### 4.7 Known risks

Each risk line answers four questions and cannot ship without all four:

- **What** (`kind`): `allergy`, `intolerance` (lactose), `coeliac` (gluten), `toxicity` (a natural
  toxin, as in raw kidney beans), `pathogen` (raw sprouts, unpasteurised milk), `contaminant`
  (mercury in some fish), `interaction` (a food with a medicine; see question 4), `choking` (whole
  nuts for small children), `excess` (a nutrient above its upper limit).
- **Who** (`who`): everyone, or a named group: `pregnancy`, `breastfeeding`, `infant` with an age
  (`under_months: 12`), `child` with an age, `older_adult`, a condition (`kidney_disease`,
  `phenylketonuria`), a medicine class, or an allergy class. The person's own avoid list (5.5) can
  hold the groups that are theirs, on their device only.
- **How much** (`amount`): a number with a unit and what it means (`ADI` with `mg/kg body weight/day`,
  `UL` per day, a labelling threshold, "no threshold set"), or "any amount". A labelling threshold is
  never shown as a safe dose (the EU's 10 mg/kg for sulphites is a labelling rule).
- **Says who, when** (`sources`): a registry id, the exact page, its published date, the date read,
  and the quotation that carries the meaning. A `use = "facts"` source (EFSA, FAO, WHO) is restated
  in our own words with the quote kept short (the registry's rule).

And one more field: `severity` on How-to's hazard scale (`caution`, `serious`, `lethal`), which
decides who must review it (section 6).

### 4.8 Storage and spoilage

`storage.keeps`: where (pantry, refrigerator, freezer, after opening), for how long (a range), from
FoodKeeper (CC0, F2) or the National Center for Home Food Preservation (`use = "facts"`). `spoils_by`:
the signs a person can see or smell, each sourced. The game's `spoilage_rate_hours` already takes
FoodKeeper's lower bound; the record keeps the whole range and the game keeps citing it.

### 4.9 Substitutes

`substitutes` name other ingredient ids, what for (baking, binding, thickening, an allergy), the
ratio and a note. **A substitute is checked against the avoid list too**: suggesting almond flour to a
person avoiding tree nuts would be the worst kind of help, so the screen derives each substitute's
classes from its own record and never offers one the person avoids.

### 4.10 Links to the game

`game.plants` (ids in `data/plants.csv`), `game.items` (ids in `data/items.csv`), `game.food_profile`
(a row of `data/food_system.ron`). The checker fails a link to an id that does not exist. Both ways
on screen: the ingredient page says "Grow it in the game", and the game's crop card can say "In real
life". The game's own food stays the game character's (the privacy notice says so); the link is for
learning.

### 4.11 Products and formulations (the prototype's history idea, kept)

A product (a box of crackers) is a list of ingredient ids with a dated history: recipes change, and
the prototype's `history: [{ as_of, note, ingredients }]` was right. Products are **not shipped in
the bundle**: they come from the person's own scans (kept on their device, in their pantry, later
linked to Possessions) and, only if the person turns it on, from an Open Food Facts lookup at
runtime (5.8). Risks are never written on a product; they are derived from its current ingredients,
as the prototype said.

### 4.12 Scale

Thousands of records is the target (FDC's Foundation Foods, the EU's additives, Canada's 5,993
foods). The website reads `index.json` (names, aliases, codes, classes) once when the reader opens,
then fetches a record only when its page opens. Budget: the index stays under 2 MB compressed at
5,000 records (to be measured as it grows); the matcher builds its lookup in memory from it.

## 5. The label reader

### 5.1 The flow

1. **Take or choose a picture** of the ingredient list (and, if there is one, the "Contains" line).
   Or type or paste the words: the reader works without a camera, and rung 1 is exactly that.
2. **Read it on the device** (5.2). Show the words read, with the doubtful ones marked, and let the
   person fix any word before matching.
3. **Split it** into ingredients (5.3).
4. **Match** each one to a record (5.4).
5. **Show the result** (5.6): what is on the person's avoid list, which allergen groups appear, any
   "may contain" warning, what could not be recognised, and the reasons the result can be wrong.

### 5.2 Reading: OCR on the device

The candidates, from the finding (F14) and the 2026-11-08 AI tools scan:

| Option | Licence | Size and hardware | Languages | Fit |
|---|---|---|---|---|
| **PaddleOCR PP-OCRv6, tiny or small** (detection + recognition), through ONNX Runtime | Apache 2.0, code and models (ONNX Runtime MIT) | 1.5M or 7.7M parameters; CPU | Chinese, English, Japanese and 46 Latin-script languages in one model | **Chosen.** Every language the app carries, small enough for a phone browser, no GPU |
| Tesseract (with `tessdata_fast`), or tesseract.js in a browser | Apache 2.0 | CPU; one model file per language | 100+ | The proven fallback; weaker on curved packaging photos |
| ocrs on RTen (Rust) | Apache 2.0 code; its models tagged CC BY-SA 4.0 | CPU, WebAssembly | Latin alphabet only, "early preview" | Not now: no Japanese or Chinese, share-alike models |
| Windows' own OCR (`Windows.Media.Ocr`) | part of Windows | on-device | the languages installed on the PC | A possible fast path on Windows, as the app already asks Windows to play video (the 2026-10-17 finding); not the core |
| Unlimited OCR 2 | MIT | about 3B parameters, NVIDIA only | many | Whole documents, not labels: an optional extra for the Library and Files pages (rung 11) |
| Surya | weights revenue-capped | GPU-friendly | 90+ | Excluded: not open source |

**One inference runtime for the whole app.** Paradee's read-aloud, MotionBricks' animation and this
reader all run ONNX models. The runtime is chosen once (ONNX Runtime through a Rust binding on the
desktop, onnxruntime-web on the website, both MIT, is the natural fit) and licence-checked in its own
finding before the first of the three is built.

The models ship with the app, or download on first use from the project's own release assets, never
fetched from a third party at runtime. Their training data is unexamined (the finding); a check
before shipping is part of rung 2.

### 5.3 Parsing a label

Label grammar the parser must handle (golden tests for each, section 11):

- Ingredients separated by commas or semicolons at the top level; parentheses and brackets hold
  sub-ingredients (`chocolate (sugar, cocoa butter, milk)`), nested.
- A declaration in parentheses after a name (`whey (milk)`, `lecithin (soy)`) or a "Contains:" line
  (FDA's two forms, F4); in the EU, allergens emphasised by font, which OCR cannot always see.
- "Contains 2% or less of", "and/or", percentages, "*organic", "may contain", "produced in a
  facility that also uses" (kept as the maker's warning, never as an ingredient).
- Additive codes in all their spellings: `E102`, `E-102`, `E 102`, `INS 102`, `(102)`, `FD&C Yellow
  No. 5`, `Yellow 5`, `Yellow 5 Lake`, colour and class names before the code
  (`colour (E 102)`), as EFSA describes labels naming the function and the substance (F10).
- Line breaks and hyphens across lines; OCR's usual confusions (`0`/`O`, `1`/`l`, `rn`/`m`).
- The label's language, detected per line, so a bilingual Canadian label matches both halves.

### 5.4 Matching

- Normalise case, accents and spacing; try exact alias matches first, then codes, then a careful
  fuzzy match (one edit for words of five letters or more, two for nine or more).
- **A fuzzy match is shown as fuzzy** ("probably *sesame*, read as *sesarne*") and still raises
  every flag the exact word would. Nothing doubtful ever silences a flag.
- **Unspecific words are pointed out**: "natural flavors", "spices", "artificial colors", "vegetable
  oil", "starch". In the US a flavouring, colouring or incidental additive that contains a major
  allergen must still declare it (section 403(w)(4) as FALCPA wrote it, the finding F4), but other
  allergens can sit inside such words, and in other countries the rules differ. The screen says
  "this word can hide ingredients".
- Every token ends in one of four states: matched, probably matched, unspecific, or not recognised.
  The counts are on the result screen.

### 5.5 The avoid list (private)

The person's own list, on their device:

- allergen groups (`sesame`, `tree_nuts`, or one nut), single ingredients (`carmine`), additive
  groups (`sulphites`), and free words for anything the data lacks;
- the groups that are theirs (pregnant, a child in the family, a condition), only if they choose, so
  risk lines for those groups are raised first;
- "Treat 'may contain' as 'contains'": **on by default**.

Where it lives: Profile's private section, on both apps. On the website it grows out of the existing
free-text "Allergies" box (kept in the browser, never on the server); the desktop gains the same
section, closing today's parity gap. It is never part of the server's readable profile. If the person
turns it on, it travels inside their own sealed vault backup and nowhere else (question 5). This keeps
the reading in the 2026-11-07 privacy finding: Washington's My Health My Data Act is not reached while
no health data reaches the server.

### 5.6 The result screen: what it says and never says

In plain mode, top to bottom:

1. **Your avoid list**: each match, in the person's words ("Sesame: on your avoid list"), with the
   label word it came from.
2. **Allergen groups** found, each with the countries whose law lists it.
3. **"May contain"** warnings, as the maker's words.
4. **Words we could not recognise** and **words that can hide ingredients**, with counts.
5. **Always, in the same place**: "We read the words on this label as best we could. We can misread
   them, labels can be wrong or out of date, and 'may contain' rules differ by country. If an allergy
   matters, read the label yourself and ask the maker." (wording by key, reviewed once and fixed).

When nothing on the avoid list was found, the screen says exactly that and no more: "We did not find
anything on your avoid list **in the words we could read**." It never says "safe", "safe for you",
"allergen-free", "gluten-free", "OK to eat" or "no allergens" (the finding, last section). A test
fails the build if any of those phrases appears in the reader's words in any language (section 11).

Detail mode adds: the per-country law for each class with its source; each matched ingredient's
record (status, risk lines with amounts and who, sources quoted, review state and date); the OCR's
confidence per word; the raw text read.

### 5.7 Desktop and website

- **Desktop** (native first): a photo from a file, the clipboard or a drag; a webcam later. OCR on a
  worker thread, never in the game's frame. The Library's new Ingredients face holds the records and
  a "Check a label" button (question 6).
- **Website** (the mirror): on a phone, a file input with the `capture` attribute (the HTML Media
  Capture way of asking for the camera) lets the phone's own picker take the photo, and the photo
  stays in the page; a live camera view (getUserMedia) is a later option and needs HTTPS (MDN, F14),
  which the site has. Which phones honour `capture` is measured in rung 3, not assumed. OCR runs in
  a Web Worker (onnxruntime-web, WebAssembly). **Nothing is uploaded**: the image and the text never
  leave the browser; only the app's own model files and the ingredient index come down from the
  project's server.
- Both run the same golden tests (section 11); the website's matcher is a port gated by the same
  vectors until a shared WebAssembly build of the Rust matcher replaces it (the website loads no
  WebAssembly today, so rung 1 does not add that pipeline; rung 3 does, for the OCR).

### 5.8 Barcodes and Open Food Facts (opt-in, later)

A barcode can look up a product's ingredient list in Open Food Facts. The finding settles how
(F8, F9):

- **Off by default.** Turned on by the person, with the plain words "This sends the barcode you scan
  to Open Food Facts, a free food database run by a non-profit in France. Nothing else is sent."
- **One request per real scan**, as its API asks; no bulk download, no scraping.
- **The ODbL notice on the screen that shows the result** ("Contains information from Open Food
  Facts, which is made available here under the Open Database License (ODbL)", with both links),
  not only on the Credits page: the Produced Work obligation, the same one `LICENSES.md` records for
  OpenStreetMap.
- **No Open Food Facts data in the bundle.** If a derived file is ever published (an offline subset),
  it goes in its own folder under the ODbL with its notices, offered whole, as `LICENSES.md` does for
  the map regions.
- **Its own warning carried over**: its data "must not be used for medical purposes", so a product
  found there is shown with "from Open Food Facts, entered by volunteers; check the package".
- Optional, later: let the person send a correction or a photo back, which Open Food Facts asks for
  and which is good citizenship.
- Barcode reading: zxing-cpp (Apache 2.0) on the desktop; the browser's own BarcodeDetector where it
  exists ("not Baseline", F14), the same library compiled to WebAssembly elsewhere.

### 5.9 Hardware, and saying so plainly

The rule (CLAUDE.md, 2026-11-08): check, and say what it needs, what the person has, and what works
instead, on the screen where they meet it.

| Situation | What the screen says (by key) | What works instead |
|---|---|---|
| No camera on this computer | "This computer has no camera we can use." | Choose a photo, paste one, or type the words |
| The browser blocks the camera (not HTTPS, or permission refused) | "This browser is not letting us use the camera here." | Choose a photo from the phone's gallery, or type the words |
| Not enough memory to load the reading model (low-end phone) | "This device does not have enough free memory to read photos here. It needs about N MB; it has M MB free." (measured in rung 3) | Type the words; or use the desktop app |
| No WebAssembly SIMD, so reading will be slow | "Reading a photo may take up to N seconds on this browser." | Wait, or type the words |
| The optional document reader (Unlimited OCR 2, rung 11) on a PC without a suitable NVIDIA GPU | "Reading whole documents needs an NVIDIA graphics card with at least N GB. This PC has: <what it found>." | The label reader, which runs on any processor |

The core reader (PP-OCRv6 tiny on the CPU) is chosen so that the first four rows are the only
hardware notices most people ever see.

### 5.10 Budgets

Measured, not assumed, when each rung lands; these are the targets the design is held to:

- **Matching**: under 20 ms for a 1,000-character list on a mid-range phone browser.
- **Reading a photo**: under 2 s on a 2018 laptop's CPU for a cropped ingredient list (PP-OCRv6
  tiny); the small model where the person picks accuracy over speed.
- **Download for the website**: the reading model and the ingredient index together under 10 MB,
  fetched only when the reader is opened, never on page load.
- **Network**: none after that. The Open Food Facts lookup, when turned on, is one small request per
  scan.
- **The game**: zero cost per frame; the reader is a page, on its own thread.

## 6. The refutation pass: no health line ships unchecked

The record: the Silverdale species research of 2026-09-15 had 48 of 60 plant and fungus records
refuted on adversarial review, almost all on toxicity and lookalikes; the 2026-10-30 review of 61 survival facts confirmed 38,
corrected 19, refuted 1 in part and left 3 unconfirmed; and today the prototype's own Yellow 5 line
fails against EFSA's list. Food health claims fail in the same way. So:

**What is checked:** every risk line, every alias or `made_from` link that puts an ingredient in an
allergen class, every status entry and every substitute's claim about what it replaces.

**The states** (How-to's scale, exactly): `stub` (written, not sourced), `sourced` (every claim
cites a source with its quote and dates), `verified` (sourced, then survived an independent attempt
to refute it, then merged). Each check is kept: `review.checks: [{ date, by, verdict, notes }]`,
where `verdict` is `confirmed`, `corrected`, `refuted` or `unconfirmed`.

**The procedure, per batch:**

1. **Write** from the primary source: the writer opens the page, quotes the words, records the
   published and read dates, and restates the fact in plain words.
2. **Refute**, by a separate agent (the `critic`) given only the line, the source link and the
   quote, not the writer's reasoning. Its brief: is the quote exact; does the source say this amount,
   this unit, this group; is the source current, or has it been replaced (a newer EFSA opinion, a
   revoked FDA authorisation); does another authority disagree (then both are shown, never one
   silently); is a regulator's status being passed off as a health risk, or a labelling threshold as
   a safe dose; does the plain sentence say more than the source.
3. **Record** the verdict on the line; correct or drop what failed; a refuted line is deleted with a
   note in the batch's findings file, not kept hidden.
4. **A person reviews** every line of severity `serious` or `lethal` (`review.person`, with the date
   and who) before it shows by default, extending the operator's 2026-09-16 rule for lethal topics
   (who: question 2).
5. **Publish a dated findings file** for the batch, as the 2026-10-30 quest-chain review did: how
   many confirmed, corrected, refuted, unconfirmed, and why.

**The binding:** `review.hash` is the BLAKE3 of the line's canonical CBOR without its `review`
block, the encoding How-to uses. Change one word and the badge is gone until it is checked again; in
a data folder a person edited, the screen says "Changed since it was checked". A badge cannot outlive
the words it vouched for (memory `feedback_checks_that_cannot_fail`).

**What shows:** plain mode shows only `verified` lines (and the statutory allergen classes, which are
read straight from the law and verified the same way). Detail mode lists the rest as "Not checked
yet", with their sources, so nothing is hidden from a person who looks.

**Staying current:** every source carries the date it was read. The checker warns when a status
line's source is older than six months or a risk line's older than a year, and the screen shows
"Last checked on <date>". Status entries with an `until` date change on that date by themselves.

## 7. Two modes

- **Plain (the default):** one line per finding, in everyday words. "Contains sesame. Sesame is on
  your avoid list." "Carmine: a red colour made from insects; some people are allergic to it."
- **Full detail (on request, per card, or "Show everything" in Settings):** the law in each country,
  the sources quoted with their dates, the amounts and who they apply to, the review history, the raw
  OCR text with its confidence.
- Two modes for the data too: an ingredient page opens on its plain summary (what it is, whether it is
  in an allergen group, the verified risks in one line each) and expands to everything.

## 8. Words by key, and the glossary

- Every interface word by key: `ingredients.*` (the Library face), `label_reader.*` (the reader),
  `avoid_list.*` (Profile), `allergen.*` (class names), `risk.*` (amount kinds, groups),
  `status.*` (status words), added with `node scripts/i18n-add.js`. Every control has its hint.
- Record text (names, the plain and detail sentences of a risk line) is data in the record, per
  language, falling back to English per field like the interface words.
- Jargon marked for the glossary, in interface words and record sentences alike. Terms
  `data/glossary.json` lacks today, to be written by the lexicographer in the rung that first shows
  them: allergen, anaphylaxis, intolerance, coeliac disease, cross-contact, precautionary allergen
  label ("may contain"), E number, INS number, GRAS, acceptable daily intake (ADI), tolerable upper
  intake level, highly refined oil, sulphites, OCR, barcode. (`calorie`, `botulism` and `CC0` exist.)
- `tests/terminology_lint.rs` gains the reader's forbidden phrases (section 11).

## 9. Privacy and safety

- **The photo, the text read and the avoid list never leave the device.** No server stores them; no
  log line contains them; the website does its reading in the browser.
- **The only thing that ever leaves**, and only when the person turns it on: a barcode, to Open Food
  Facts, one per scan (5.8).
- **Health groups the person adds** (pregnant, a condition) are the most sensitive data the app
  holds; they sit with the avoid list, under the same rule.
- **Errs toward pointing out.** False positives (a flag that was not needed) cost a second look;
  false negatives (a missed allergen) can kill. Every design choice above (fuzzy matches raise flags,
  exemptions are shown, "may contain" counts by default, unknown words are counted) leans the same
  way.
- **No medical advice.** The reader never sets a dose for a person, never says what to eat, never
  tells anyone to change a medicine (the finding, F7 and the last section).

## 10. Licences and credits

From the finding:

| Source | May ship | Conditions | Registry `use` |
|---|---|---|---|
| USDA FoodData Central | yes, in the bundle | CC0; cite as requested | `verbatim` (exists as `usda-ars`; add `usda-fdc`) |
| USDA FSIS FoodKeeper | yes | CC0 on data.gov | `verbatim` (`usda-fsis`) |
| FDA pages (allergens, additives, Red No. 3) | yes, as text | public domain; copies dated and linked | `verbatim` (`fda`) |
| European Commission content (allergen factsheet, additives database) | yes | CC BY 4.0: credit, mark changes; no logos | add `ec-food`; `facts` for the regulations until EUR-Lex is read |
| EFSA | facts | re-use with acknowledgement; individual documents may differ; no logo | add `efsa`, `facts` |
| Codex / FAO | facts | non-commercial reproduction only | `fao` exists; add `codex`, `facts` |
| WHO / JECFA | facts | CC BY-NC-SA 3.0 IGO; database use needs permission | `who` exists, `facts` |
| Canadian Nutrient File 2026 | yes | OGL-Canada 2.0 attribution line | add `health-canada-cnf`, `verbatim` |
| ANSES Ciqual | yes | Licence Ouverte 2.0: source and its last update date | add `anses-ciqual`, `verbatim` |
| Open Food Facts | runtime only, opt-in | ODbL notice on screen; derived files under ODbL in their own folder | add `openfoodfacts`, `facts`, `bundle: false` |
| PP-OCRv6 models, ONNX Runtime, zxing-cpp | yes | Apache 2.0 / MIT notices beside the files | in `LICENSES.md` and `data/credits.ron` |

Every attribution-required source gets its row in `data/credits.ron` naming the surface where its
notice is shown, which the credits test enforces.

## 11. Dev tooling (permanent)

Never trimmed as cleanup (CLAUDE.md, "Forever development"):

- **`scripts/check-ingredients.js`** (in `just lints` and `just preflight`): the schema; every source
  id in the registry; every risk line with a quote, both dates, a unit on every number and a group on
  every amount; a `facts` source's quote kept short; every game link resolving; every class member
  resolving; review hashes current; status and risks never mixed; the forbidden phrases absent from
  the reader's words in every language.
- **Golden labels** (`tests/fixtures/labels/`): typed label texts (rung 1) and photos (rung 2), each
  with the tokens, matches, flags and unknowns expected. The same set runs against the desktop and the
  website; each new check is seen red first, with the red evidence written in the test's comment.
- **A false-negative test set**: labels where an allergen hides behind a derived name, a misread, a
  line break, a code or a second language. Missing one fails the build.
- **The review board** (Dev page): every line by review state, age and severity, the batch findings
  files, and the lines waiting for a person.
- **The reader's debug view**: the photo with the detected text boxes and per-word confidence, the
  parse tree, and each token's match path, on both apps.

## 12. What we will not build, and why

Said plainly, on the screen where a person would look for it (CLAUDE.md, 2026-09-18), naming which
kind of reason it is:

- **A "safe to eat" verdict.** A choice of this project, backed by every source read: labels can be
  wrong, the reader can misread, and no source we can cite will stand behind such a verdict (Open Food
  Facts: "must not be used for medical purposes"). Not a law, a patent or a platform rule.
- **Personal medical advice** (doses, what to eat for a condition, medicine changes). A choice of this
  project; in the United States FDA describes education and reference apps as outside its device rules
  and decision-making about a specific patient as inside them (F7). It is a line the project chooses to
  stay well behind.
- **Product photos from Open Food Facts in the app.** The photos carry the packaging's copyright and
  trademarks, which their licence does not cover (F8): a law reason (copyright and trademark).
- **Codex, FAO and WHO texts quoted at length.** Their terms allow non-commercial use only (F11): a
  licence reason; the facts are cited instead.
- **Uploading photos to a server to read them.** A choice: it would put what people eat, and so their
  health, on a server, which the project's privacy promise rules out.

## 13. The rungs

Each rung is a slice of the real thing, not a throwaway.

1. **The allergen spine and the typed-list checker. (Rung 1.)**
   - Data: `allergen_classes.json` and `regimes.json` (US, EU, Canada; Codex marked partial),
     `schemas/ingredient.toml`, and the records for every member of every class and their label
     names (milk and whey, casein; egg; fish and crustaceans by kind; molluscs; each tree nut; peanut;
     wheat, rye, barley, oats and triticale; soy; sesame; mustard; celery; lupin; sulphites and their
     E-numbers). English names; other languages as empty fields. Every class membership and every
     derived name sourced from the law and passed through the refutation pass. No risk lines yet.
     Game links where the crop exists (wheat, soybean, peanut, sesame, celery, lupin, hazelnut,
     walnut, oat, barley, rye).
   - Code: a feature-neutral matcher (normalise, split, match, flag) with its golden typed labels and
     the false-negative set; the website's port gated by the same vectors.
   - Screens: the Library's Ingredients face (browse, plain and detail) with "Check a label" that
     takes typed or pasted words; the result screen with its fixed honest wording; the avoid list in
     Profile on both apps (the website's Allergies box grows into it).
   - Retire `data/items/foods/`. Add the sources to the registry and `data/credits.ron`.
   - Why first: the OCR only ever produces text. The matcher, the classes, the avoid list and the
     honest screen are the parts that can hurt someone, and proving them on typed words keeps their
     errors apart from a camera's.
2. **A photo on the desktop**: PP-OCRv6 through the shared ONNX runtime (its finding first), from a
   file, the clipboard or a drag; the word-fixing step; golden photos; the debug view.
3. **A photo on the website**: the phone's camera through a file input, onnxruntime-web in a Web
   Worker, the same models, nothing uploaded, the hardware notices measured and worded.
4. **Additives**: E and INS numbers and the US colour names, each with dated status per regime (the
   European Commission's database, FDA's Substances Added to Food); status shown apart from risk.
5. **Risk lines, first batch**: the regulators' own consumer statements (FDA, EFSA, Health Canada),
   through the refutation pass and a person's review for `serious` and `lethal`, with the batch's
   findings file.
6. **Nutrition, storage, substitutes**: FDC rows linked (the crop files' snapshot), FoodKeeper ranges,
   substitutes checked against the avoid list; Canadian Nutrient File and Ciqual for foods FDC lacks.
7. **The game, both ways**: "In real life" on crop cards, "Grow it in the game" on ingredient pages;
   the game's food profiles cite the same rows.
8. **Every language**: names and aliases in French, Spanish, Japanese and Chinese; Japan's and
   China's allergen laws after their own finding.
9. **Barcodes and Open Food Facts**: opt-in, one request per scan, the ODbL notice on screen;
   contributions back as an option.
10. **Your pantry**: products you scanned, with their dated ingredient history, on your device,
    linked to Possessions when it exists.
11. **The optional document reader** (Unlimited OCR 2) for whole pages in the Library and Files, with
    its GPU notice; never needed for labels.
12. **Pets**, if the operator wants it (question 7): foods dangerous to dogs and cats, through the
    same gate.

## 14. Questions for the operator, each with a recommendation

1. **Use Open Food Facts at all?** It is the only open source of product ingredient lists, but it is
   share-alike, volunteer-entered, and a lookup tells it what you scanned. *Recommendation: yes, at
   rung 9, off by default, one lookup per scan with the plain notice; nothing from it in the bundle.*
2. **Who reviews the serious and lethal health lines?** An AI refutation pass catches misquotes and
   stale sources; a person should still read anything that can kill. *Recommendation: no serious or
   lethal risk line shows by default until a named, qualified volunteer (a dietitian, pharmacist or
   allergist) has read it; ask the community for one by adding a "Health reviewer" role to
   [`docs/outreach/volunteers-wanted.md`](../outreach/volunteers-wanted.md) (not added here: that
   page is public copy and the role is the operator's to open). Until then, ship the allergen
   classes, the regulatory status and the regulators' own consumer statements, which are the
   lowest-risk lines.*
3. **Which country's allergen list by default?** *Recommendation: the union of every country the data
   carries, with the person's own country marked, because a label in the shop may follow another
   country's law; a setting narrows it.*
4. **Food and medicine interactions** (grapefruit with some heart medicines, vitamin K with blood
   thinners): useful, and the closest this feature comes to medical advice. *Recommendation: only the
   warnings a regulator publishes for consumers, worded "ask your pharmacist", never a dose; and none
   until question 2 has an answer.*
5. **Should the avoid list ever leave the device?** *Recommendation: device-only by default; included
   in the person's own sealed vault backup only if they turn that on; never in the server's readable
   profile.*
6. **Where does it live?** *Recommendation: an Ingredients face in the Library (beside Documents,
   Dictionary and Curriculum) with "Check a label" in it, the avoid list in Profile's private
   section, and links from How-to recipes; no new top-level page.*
7. **Pets?** The prototype carried dog and cat toxicity, which suggests people want it.
   *Recommendation: a later rung (12), through the same refutation gate, because the dangerous
   foods for pets are well documented and often surprising.*

## 15. Not done here

- No code, no data and no registry entries were written; this document and the finding are the whole
  of rung 0.
- The ONNX runtime, Japan's and China's allergen laws, the EU's medical device question and the Codex
  texts each need their own research before the rung that uses them (the finding's open questions).
- `data/items/foods/` is still in the tree until rung 1 replaces it.
