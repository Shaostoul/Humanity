# How-to: step-by-step procedures for real life, the game and this platform

> **Status:** design, 2026-10-23; **increment 1 built the same day** (1a data and the desktop page,
> 1b the website's), unreviewed content: see "Increment 1 as built" near the end for what it
> changed in this plan. This is the whole plan written before the first increment, per the
> operator's rule (CLAUDE.md, "Plan the whole feature, build it in rungs, keep performance in mind
> throughout"): the data shapes, the budgets and the scale are decided here once, so the first
> slice is a rung of the real thing. The build order is the increment ladder near the end. Four
> decisions were the operator's (the last section): two answered on 2026-10-23, two open; every
> other call is made here with its reason, so it can be overruled in one line.

## The idea in one paragraph

A **how-to** is one file of data: what it achieves and for whom, how long it takes, what you
need, the steps (to tick off, with timers where a step waits, checks where you test something,
and safety notes at the step where they matter), how to tell it worked, what goes wrong and what
to do then, who can do it for you, and where its facts come from with dates. One page, **How-to**,
renders every how-to for real life. The game renders the same files inside the world, where a
crafting recipe turns out to be a how-to that makes something, so the Crafting page becomes a
view of how-tos rather than a second system. The platform's own procedures (run a server,
restore a backup, cut a release) are how-tos too, and a step can be a button from the admin
action registry, so a person can follow one and an AI can run one. Sharing works the way the
Market works: signed, written by people, adapted to a place the way the Laws page handles
jurisdictions, and pulled on demand rather than copied onto every server.

## Why: the operator's idea, and what the app already has

The operator, 2026-10-23, verbatim:

> "I feel like we should have a page dedicated to SOP. It's kind of like a crafting page but,
> could incorporate more stuff. Like parent categories could be good and services or something
> like that. What do you think? How would you make this idea the best it could be?"

The strongest argument for building it is that the app already contains procedures, in nine
different shapes, none of which can be followed, ticked, timed, scheduled or shared the same way:

| Where (counted 2026-10-23) | What it is | Its shape | What happens to it |
|---|---|---|---|
| `data/recipes.csv` (386 recipes), `data/crafting/tools.ron`, `data/crafting/categories.json` | crafting recipes, their hand tools, the Crafting sidebar | CSV rows plus two overlays | become how-tos with a `makes` block (increment 4); the Crafting page reads how-tos |
| `docs/user/skills/*.md` (75 of 128 guides have a numbered list of four or more bold-led steps; copied to `data/library/` by `scripts/build-library.js`) | the "Step by step" sections of Library guides | Markdown lists | the steps move into how-tos and the guide embeds them, so they exist once (increment 2) |
| `data/onboarding/quests.json` (6 chains, 26 steps) | "Learn by doing" on the Tasks and Quests pages, progress in localStorage | JSON chains | become how-tos (increment 6) |
| `data/onboarding/sim_guides.json` (27 entries, 20 unwritten) | an index of game guides whose `_comment` says it "needs a render surface" | JSON | retired: its checklists become how-tos, its references become Library documents (increment 6) |
| `data/admin/ops_registry.json` (78 actions, no ids) | every admin action, with a free-text `how` | JSON | gains ids, so a how-to step can be one of its actions (increment 6) |
| `.claude/skills/{release,merge-agent,game-checks}/SKILL.md`, `docs/SOP.md` | the developers' procedures, for people and for AI | Markdown | the procedures become how-tos; the skills are generated from them (increment 6) |
| `data/testing/qa_tasks.json` (63 cards) | the Testing page's checks of shipped features | JSON | stays as it is ("What not to build") |
| `data/quests/*.ron` | game quests: objectives, rewards, chains | RON | stay quests; a quest step can point at a how-to, and the objective kinds are how the game ticks a how-to's steps (increment 5) |
| `data/npc/chores.ron` | what crew NPCs are seen doing | RON | stays; a chore can name the how-to it shows (later) |

### Who it serves

1. **Someone at home with one thing to do now.** Change the furnace filter; find out why a jar
   lid did not seal. They want the short version, the right tools, and a timer.
2. **Someone in trouble.** The power is out, a cut will not stop bleeding, the water may be
   unsafe. They need the "stop and call someone if" line first, the emergency number for where
   they are, and a how-to that works offline and on paper.
3. **A learner.** The Curriculum's practical layer: read why in the Library, practise in the
   game, then do it for real (docs/design/educational-gameplay.md).
4. **Someone who would rather have it done.** Who near them does this, who teaches it, and what
   it costs, without the platform touching the money (docs/design/market-directory.md).
5. **Helpers and providers.** A repair shop, a guild, a neighbour who teaches canning: their
   service says which how-tos it covers.
6. **Server owners.** Run, back up, restore and move their server from inside the app, with the
   honest command where no button exists yet (docs/design/in-app-ops.md).
7. **Developers and AI agents.** The platform's own procedures, from one source, which a person
   reads on a page and an AI reads as data.

## The name: How-to

**Recommended: How-to.** The page is "How-to"; one is "a how-to"; several are "how-tos". In code
it is `how_to` (the Rust module, the page id, the data folder, the schema, the signed object
`how_to_v1`); on the website it is `how-to` (the route `/how-to`, `how-to.html`), following the
existing split between snake_case page ids (`relay_control`) and hyphenated web pages
(`wallet-guide.html`). Code, routes, files, settings and docs take the name together, in the same
increment, per the naming rule (CLAUDE.md, 2026-10-19).

Why this word:

- **It is what people already type.** "How to change a furnace filter" is the phrase in a search
  box. The page name matches the question, so nobody has to learn a word to find it.
- **It names a verb, like its neighbours.** The Library is what you read, Apps is what you use,
  How-to is what you do. The pages already split by verb (the Library and the Tools page split
  that way in 2026-07, kept when Tools joined Apps).
- **It does not collide.** Rejected, with the reason:
  - *SOP*: jargon. "Standard operating procedure" means nothing to most of the people this
    platform most wants to reach, and the abbreviation means less.
  - *Guides*: the Library already calls its documents guides (its tag "Step-by-step guide"), and
    the Tasks page calls its learn-by-doing panel guides. A third meaning would make the word
    useless.
  - *Procedures*: accurate, formal, cold; reads like a workplace manual.
  - *Instructions*, *Manual*: suggest a product's booklet.
  - *Know-how*: warm, but an idiom that translates badly.
  - *DIY*: leaves out "who can do it for you" and the platform's own procedures; also jargon
    outside English.
  - *Recipes*: the game's word for a transform (docs/design/terminology.md defines Recipe), and
    in English it means cooking.
- **It translates as an ordinary phrase** in the app's five languages (suggestions for a native
  speaker to check before any translation ships: Spanish "Cómo hacerlo", French "Comment faire",
  Japanese "やり方", Chinese "怎么做").

In the game the page that lists what you can make keeps the name **Crafting**: it is a different
place (inside the world, per docs/design/two-realities.md), most of what it lists is making, and
it is a word every player already knows. Both pages read the same how-to files.

## The one idea: a how-to is data, and a recipe is part of one

The project's terminology already defines a **Recipe** as "a deterministic transformation of
inputs into outputs that declares time, requirements, and byproducts" (docs/design/terminology.md).
That is an *outcome*: two iron ore and one coal become one ingot in ten seconds at a smelter. A
how-to is a *method*: the steps a person follows. The two fit together without either being
stretched:

- A how-to **may carry a recipe**, in its `makes` block. "Smelt iron" is a how-to whose `makes`
  is the transform the game runs. Most of the 386 recipes have no real steps written yet; they
  are how-tos with only a `makes` block, the same way a Curriculum topic can be `absent`.
- **The Crafting page is a view**: every how-to with a `makes` block that exists in the game,
  with the game's inventory, stations and craft queue.
- **The two modes the project requires of every deep system** (CLAUDE.md, 2026-09-24) fall out
  of this for free. *Simplified*: one click, the recipe's time, today's behaviour. *Realistic*:
  do each step, ticked by what you actually do in the game. A Settings > Gameplay switch,
  "Making things: Simplified or Realistic", sits beside the existing carrying and death switches.
- One method can make several things, and several methods can make the same thing (bread in an
  oven, bread in a Dutch oven over coals). That is why the recipe belongs to the how-to and not
  the other way round, and why recipes move into how-to files (increment 4) instead of how-tos
  pointing into a separate table: a recipe row cannot be signed, adapted to a place or translated
  on its own, and a how-to can.

### Words this design adds

Each goes into docs/design/terminology.md in increment 1 (its usage rule requires it).

| Word | Meaning |
|---|---|
| **How-to** | One procedure as data: its purpose, needs, steps, checks, problems, help and sources. |
| **Step** | One thing to do in a how-to, with a stable id. |
| **Run** | One time a person does a how-to: when it started, which steps are done, what they wrote down, how it ended. |
| **Repeat** | A person's choice to do a how-to every so often (every 90 days); the Calendar shows when it is next due. |
| **Variant** | A how-to adapted to a place or a kind of equipment, stating only what differs from the how-to it adapts. |
| **Recipe** | Unchanged: the game transform, now the `makes` block of a how-to. |

## The model

### A how-to

One JSON file per how-to. JSON because both clients read it (the web fetches the file itself,
no conversion), it is the format of every other content set both clients share (the Library
catalog, the Curriculum, the external catalog, the Laws, the admin registry), and its data model
maps directly onto the canonical CBOR a signed object needs. Prose fields that run long are
arrays of paragraphs, the pattern `data/help/topics.json` already uses.

A worked example, the shape every field takes (comments are for this document only; the Library
slug, the topic id and the two furnace items are illustrative and do not exist yet):

```jsonc
{
  "id": "change_furnace_filter",                 // snake_case, stable, never reused
  "title": "Change a furnace filter",
  "summary": "Swap the air filter in a forced-air furnace so it heats safely and costs less to run.",
  "for": "Anyone with a forced-air furnace or heat pump. No experience needed.",
  "audience": "everyone",                        // everyone | server_owner | developer
  "category": "household_heating",               // one node of data/how_to/categories.json
  "tags": ["real", "shelter"],                   // the Library's tag vocabulary (data/library/tags.json)
  "realities": ["real"],                         // real, sim, or both
  "time": { "active_min": 10, "waiting_min": 0 },
  "difficulty": 1,                               // 1 Easy .. 5 Expert, shown as words
  "hazard": "caution",                           // none | caution | serious | lethal (the Curriculum's scale)
  "skills": ["shelter_building"],                // data/skills/skills.csv ids
  "topics": ["shelter_heating"],                 // data/curriculum/syllabus.json topic ids
  "reading": ["heating-a-home"],                 // Library slugs, the /library#slug grammar
  "needs_first": [],                             // how-tos to know before this one
  "tools": [
    { "item": "screwdriver_flat_0", "optional": true, "note": "Only if the cover is screwed shut." }
  ],
  "materials": [
    { "item": "furnace_filter_0", "qty": { "value": 1, "unit": "each" },
      "note": "The same size as the old one; the size is printed on its edge." }
  ],
  "stop_and_call": [
    { "when": "You smell gas.",
      "do": "Leave the building now and call the gas company from outside.",
      "who": "role:gas-utility" }                // resolved through the person's locale
  ],
  "steps": [
    { "id": "off", "kind": "do", "text": "Turn the furnace off at the thermostat.",
      "why": ["A running fan pulls dust past the open slot and into the blower."] },
    { "id": "size", "kind": "measure", "text": "Read the size printed on the old filter's edge.",
      "record": { "type": "text", "label": "Filter size", "remember": true } },
    { "id": "arrow", "kind": "check",
      "text": "Does the arrow on the new filter point toward the furnace?",
      "no": "arrow_backwards" },                 // a problem id below, or a step id
    { "id": "on", "kind": "do", "text": "Turn the furnace back on." }
  ],
  "worked_if": [ "Warm air comes from the vents within a few minutes." ],
  "problems": [
    { "id": "arrow_backwards", "sign": "The arrow points away from the furnace.",
      "why": "Filters are built to catch dust from one side.",
      "fix": "Take it out and turn it round." }
  ],
  "help": { "do": ["maintain", "install"], "acts_on": ["furnace_0"], "teach": true,
            "catalog": [] },                     // see "Who can help" below
  "repeat": { "every_days": 90, "note": "Every one to three months; monthly with pets." },
  "makes": null,                                 // a recipe, for a how-to the game can make
  "sources": [
    { "id": "doe-energy-saver", "where": "https://www.energy.gov/...", "read": "2026-10-23",
      "quote": "the words that carry the meaning" }
  ],
  "review": { "status": "sourced", "hash": "<BLAKE3>", "checked": "2026-10-23",
              "by": "refutation pass", "person": null },
  "written_by": "ai",                            // ai | person; shown on the page (Accord: be open about AI)
  "updated": "2026-10-23"
}
```

Notes on the fields that carry decisions:

- **Items come from the shared taxonomy.** `tools`, `materials`, `help.acts_on` and `makes` name
  `data/items.csv` ids, the same spine the Market's offerings use (`item_ref`,
  schemas/offering.toml) and the game uses. A real thing missing from the taxonomy gets a row
  (the taxonomy is meant to cover real things); until it has one, a how-to may carry
  `proposed_item` the way an offering does.
- **Quantities carry units** (`{ "value": 4, "unit": "l" }`, or `min`/`max` for a range), stored
  in metric and shown in the person's chosen units (a setting added in the increment that first
  shows a converted quantity). A unit list lives in the schema.
- **`tools` are shared by both realities.** In the game, each listed tool with a durability wears
  by one use (today's `tools.ron` behaviour). `materials` are the real quantities; `makes.inputs`
  are the game's balanced transform. They stay separate because the game abstracts (two ore and
  one coal) while the real list is literal; where they agree, the migration says so, and the
  realistic-first direction is to converge them over time.
- **`hazard` is at least the highest step's safety level.** The checker refuses less.
- **`audience`** decides visibility: `developer` how-tos show only with Developer mode on (the
  existing `nav_dev_visible`), `server_owner` ones in the HumanityOS > Running a server branch.

### Steps

Every step has an `id` (stable, unique in its how-to) and a `text` (a short imperative sentence,
shown in both views). The `kind` says what the person does:

| Kind | What the person sees | In real life | In the game |
|---|---|---|---|
| `do` | a line to tick | ticked by hand | ticked by a game event when `sim.watch` names one; otherwise a Done button |
| `wait` | a timer, `wait: { "min_s", "max_s" }` | the real clock; the due time is stored, so it runs while the app is closed | the world clock, at the server's speed |
| `check` | a question with Yes and No | No goes to a problem or a step (`no`) | the same |
| `measure` | a value to write down, `record: { type, label, unit, range, remember }` | `remember` shows last time's value next time ("Last time: 16 x 25 x 1") | the game's value where it models one |
| `choose` | one of a few options, `options: [{ id, label }]` | later steps show or hide by `if` | the same |
| `call` | another how-to, inline (`how_to`) | opens as a sub-run, ticks back up | the same |
| `go` | a button that opens a page or section of this app (`page`, `section`) | the same | the same |
| `action` | an admin action from `data/admin/ops_registry.json`, as a button (`action`) | shipped how-tos only; its danger level asks for a confirm or a typed name | none |
| `command` | a command to type, with Copy (`command`, `shell`) | shipped how-tos only; each one is a counted gap in the app (see "The platform's own how-tos") | none |

Any step may also carry:

- `why` and `detail`: paragraphs shown in the Full view only.
- `safety`: `{ "level": "caution" | "serious" | "lethal", "short": "...", "full": ["..."] }`.
- `uses`: the tools and materials this step needs (so the step can say "you need the jar lifter
  now").
- `image` with a required `alt`: a picture under `assets/how_to/<how-to id>/`, public domain or
  our own.
- `cites`: source ids, for the facts in this step.
- `time`: hands-on minutes for this step, when it is long enough to matter.
- `who`: a role from the how-to's `roles` list, for work done by several people (a crew raising
  a wall; the game's co-operative skills in docs/design/educational-gameplay.md). Reserved now,
  shown later.
- `if`: show this step only when an earlier `choose` picked something (`"if": { "furnace_kind":
  "gas" }`) or when a place applies (`"if": { "place": "wa" }`).
- `sim`: how the game sees it: `{ "watch": { "kind": "plant", "target": "tomato" } }` reuses the
  quest objective kinds (`src/systems/quests/objectives.rs`: Gather, Craft, Make, Harvest,
  Build, Travel, Talk, Eat, Plant, View), `"read"` means the player confirms it (the game cannot
  observe wiping a jar rim, and reading it is the lesson), `"skip"` means the game does not model
  it.
- `short: false`: leave this step out of the Short view (an explanatory step). Refused on any
  step with `serious` or `lethal` safety.

### Safety, hazard, and the two reading views

The project requires two modes of every deep system. For reading a how-to they are **Short** and
**Full** (not "Simplified", which Settings already uses for game realism, so one word does not
mean two things):

- **Short**: the steps' `text`, every safety note's `short` line, the checks, the timers, the
  "stop and call" box. For someone who knows the why already, or whose hands are wet.
- **Full**: everything, with each step's `why`, `detail` and the full safety note.

**Short shortens; it never omits.** A step whose safety level is `serious` or `lethal` must have a
`short` line and is always shown in Short. The checker refuses a how-to that breaks this, and a
test proves the refusal on a deliberately broken file. The choice is remembered for the person
(`how_to_view` in config.json and its web twin), not per how-to.

On screen the hazard is a word and a painted icon, never colour alone (the tint rule in
docs/design/ui-system.md), worded the way the Library's Curriculum words its lethal topics ("Can
kill you if taught wrong"): `lethal` "Can kill you if done wrong", `serious` "Can seriously hurt
you", `caution` "Take care". A `serious` or `lethal`
how-to without a `stop_and_call` entry is a warning from the checker; one in Health and first
aid always shows the emergency number for the person's place at the top.

### Sources and review

Teaching data fails in a way code does not: 48 of 60 researched species records were refuted on
review, almost all on the fields that can hurt someone, and the fixes needed their own pass
(memory `project_teaching_data_needs_refutation`). So review is a first-class field, not a
comment:

- `sources` cite ids from `data/sources/registry.json` (the one list of authorities, with its
  licence and `use` rules; schemas/source.toml), plus where exactly, the date we read it, and the
  words that carry the meaning. Facts are restated in our own words; copyrighted text is never
  copied in (the `use = "facts"` rule).
- `review.status` uses the Curriculum's scale and meanings exactly: `absent` (named so the gap is
  countable), `stub` (written, not sourced), `sourced` (every claim cites), `verified` (sourced
  and independently refuted-and-merged).
- `review.hash` is the BLAKE3 of the how-to's canonical CBOR without the `review` block, the same
  encoding and hash the signed objects use. **A review binds to the text it checked.** If a
  shipped how-to's hash no longer matches, the checker fails the build, and in a data folder a
  person edited, the page shows "Changed since it was checked" instead of the badge. This is the
  guard against a check that cannot fail (memory `feedback_checks_that_cannot_fail`): the badge
  cannot outlive the words it vouched for.
- `review.person` is the date a person read it. A `lethal` how-to cannot show as checked without
  one, extending the operator's 2026-09-16 rule for Curriculum topics (see the decisions).
- A number in a `serious` or `lethal` step must cite a source; the checker flags digits in such a
  step with no `cites`. Crude, mechanical, and it catches the dangerous case (a processing time,
  a dose, a temperature).
- On screen: "Checked 2026-11-02", "Sourced, not yet checked", or "Not checked yet", beside the
  hazard. Unchecked how-tos are shown, labelled; a planned (`absent`) one is shown dimmed in its
  category with "Not written yet", because a page that only shows what it has cannot tell you
  what it is missing (the Curriculum's principle).

### A category

`data/how_to/categories.json` is a tree, unlimited in depth. A how-to sits in exactly one node
(its shelf, as a Library document does), and tags cross-cut.

```jsonc
{
  "id": "household",                       // unique across the whole tree
  "title": "Household",
  "line": "Keeping a home running: heat, water, power, cleaning and safety checks.",
  "order": 1,
  "icon": "home",                          // a widgets::icons painter name
  "subjects": ["shelter", "sanitation"],   // Curriculum subjects this branch practises
  "help": ["housing", "energy"],           // Help and Services categories (data/external/catalog.json)
  "children": [ { "id": "household_heating", "title": "Heating and cooling", "line": "...", "children": [] } ]
}
```

The starting tree (titles and one line each; children are examples, the data decides):

| Top level | Its line | Children, for a start |
|---|---|---|
| Household | Keeping a home running | Heating and cooling, Plumbing, Electrics you may safely touch, Appliances, Cleaning, Safety checks |
| Food | Cooking, keeping and growing food | Cooking, Keeping food, Growing food, Animals, Wild food |
| Water | Finding, keeping and making water safe | Collecting, Making it safe, Storing, Testing |
| Health and first aid | Looking after a body, and knowing when to get help | First aid, Staying well, Caring for someone |
| Making and repair | Tools, materials and fixing things | Tools, Wood, Metal, Cloth and sewing, Electronics, Bikes and vehicles |
| Energy and power | Making, storing and using power safely | Electricity, Fuel and heat, Batteries and solar |
| Emergencies | When something goes wrong at once | Power cut, Storm, Fire, Flood, Earthquake, Lost, Leaving in a hurry |
| Money and paperwork | Budgets, benefits, documents and forms | Budgeting, Getting help, Documents and IDs, Taxes |
| Community | Doing things together | Meetings and decisions, Sharing and lending, Teaching others, Volunteering |
| Computers and phones | Keeping your devices and your data safe | Backups, Passwords and keys, Staying safe online, Fixing common problems |
| HumanityOS | Using, running and developing this app | Getting started, Your account, Running a server, Developing HumanityOS (Developer mode) |

"Household" rather than "Home", because the app's Home page is the game homestead. The tree is
organised by where in life a task sits; the Curriculum is organised by subject. They are linked
through `subjects`, not merged: "Change a furnace filter" is Household, and it practises the
Shelter subject.

### A run

A run is the record of one time a person does a how-to. It belongs to that person, lives on
their device, and is never sent anywhere unless they choose to.

```jsonc
{
  "run": "r-20261023-140211-7f3a",
  "how_to": "change_furnace_filter",
  "hash": "<the how-to's hash when the run started>",
  "reality": "real",                       // a game run lives in the game save, see below
  "started": "2026-10-23T21:02:11Z",
  "finished": null,
  "outcome": null,                         // worked | did_not_work | stopped
  "choices": { "furnace_kind": "gas" },
  "steps": {
    "off":   { "done": "2026-10-23T21:02:40Z" },
    "size":  { "done": "2026-10-23T21:04:05Z", "value": "16x25x1" },
    "arrow": { "done": "2026-10-23T21:05:30Z", "answer": "no", "went_to": "arrow_backwards" }
  },
  "waits": {},                             // step id -> { "ends": time }
  "notes": "",
  "photos": [],                            // file names in this run's folder
  "task": null,                            // a Tasks id, when the person added it to Tasks
  "place": "silverdale"
}
```

- Native: `%APPDATA%/HumanityOS/how_to/runs/<run>.json`, photos in `how_to/runs/<run>/`, each
  written to a `.tmp` and renamed (the save pattern in `src/persistence.rs`), listed with its size
  in Settings > Data. Web: IndexedDB.
- Step state is keyed by step **id**, so a how-to that is edited, reordered or updated mid-run
  keeps the run's ticks. A step that no longer exists shows as "This step was removed since you
  started".
- A game run (Realistic making) is part of the game save, like quest progress, because it is
  part of the world: it uses the game clock and the game inventory.
- Runs are part of the person's own data: in the encrypted personal backup when that exists
  (docs/design/two-realities.md), and erased with the rest of their local data.

### A repeat

```jsonc
{ "how_to": "change_furnace_filter", "every_days": 90, "from": "2026-10-23", "remind": true }
```

Kept in `how_to/repeats.json`. **Next due** is the last run that ended `worked`, plus
`every_days`; with no run yet, `from` plus `every_days`. Nothing is copied into the Calendar: the
Calendar asks for the due dates and shows them (see "How it connects"), so doing it early moves
the next date by itself and there is no second record to drift. The how-to's own `repeat` field
is only the suggestion; the person sets the real interval.

### Who can help (the services join)

The operator asked for "services or something like that". The Market already has the right
shape, so this adds no new registry of helpers:

- An `offering_v1` with `kind = "service"` carries an `action` verb (repair, maintain, install,
  build, process, instruct, consult, care...) and an `item_ref` it acts on (schemas/offering.toml).
- A how-to's `help` says which verbs would do it for you (`do`) and on what (`acts_on`). **"Who
  does this near you"** is the query: service offerings whose `action` is in `do` and whose
  `item_ref` is in `acts_on`, in the person's reality and area. **"Who teaches this"** is the
  same with `action = instruct`, when `teach` is true.
- A service can also say which how-tos it covers: an optional `how_tos` list in the offering's
  `service` table (offering payload schema version 2, changed outright, no compatibility shim).
  The how-to page shows those offerings first.
- Guild members who offer help do it the same way: a guild publishes a `provider_v1` backed by its
  group, and its offerings appear here. **One place to offer help, one privacy rule:** nobody
  appears as a helper unless they published an offering. Skill DNA levels and endorsements are
  never turned into a list of people to contact.
- The Help and Services catalog (`data/external/catalog.json`, its `service` categories) gives
  the websites: a category's `help` names catalog categories, and a how-to's `help.catalog` can
  name single entries. Catalog entries have no ids today; they get stable ones (increment 2).
- Money is never handled here: the Market lists, the two parties settle (the directory rule).

### Variants and places

The same how-to differs by place (a permit needed in one state and not another; who to call;
what the water authority is) and by equipment (gas furnace or heat pump). Two mechanisms, both
already in the repo:

- **Roles resolve by place.** A step or a `stop_and_call` entry can name `role:<id>`; the
  person's locale (`data/locales/<id>/locale.json`, its `authorities`) gives the name and
  address, the way Curriculum sources resolve `state-water-agency`. One how-to serves every
  place with no copy.
- **A variant states only what differs.** It names the how-to it adapts and that how-to's hash,
  a `place` (a jurisdiction id from `data/laws/laws.json`) or an equipment label, and a list of
  changes: add a step after another, replace a step's text, add a legal note. It may never remove
  a step that carries `serious` or `lethal` safety. Because it is a list of changes, a safety fix
  to the original reaches every variant; because it pins the original's hash, a variant whose
  original changed shows "The general how-to changed since this was written" until someone
  checks it again.
- **The nearest place wins.** The person's location sits on a jurisdiction chain (Silverdale,
  Kitsap County, Washington, United States, Earth, Humanity, exactly the Laws page's chain); the
  how-to shows the variant nearest them, and lists the others ("Also: general, Washington").
- The location is a saved setting shared by Laws, How-to and the locale data. Today
  `GuiState.laws_location` exists but is not saved; increment 7 saves it, once, for all three.

### Files and folders

```
data/how_to/categories.json              the tree
data/how_to/<top category id>/<id>.json  one how-to per file; the folder is its top category
data/how_to/index.json                   generated: the list both clients read first, carrying each
                                         how-to's `makes`, so the Crafting views (and a bare exe
                                         with no data folder) need nothing else
schemas/how_to.toml                      every field in this document, with its rules
assets/how_to/<id>/                      pictures, with licence and alt text
src/how_to/                              the model, loader, checker, runs, repeats
                                         (compiles for the relay too; GUI parts gated `native`)
src/gui/pages/how_to.rs, how_to/         the page: rail, list, view, run, print
web/pages/how-to.html, how-to-app.js     the web mirror
web/shared/how-to-logic.js               the rules both the page and tests use (as laws-logic.js)
scripts/build-how-to.js                  writes index.json, checks every file, reports gaps
```

One folder per top category is deliberate: content agents in this repo work in parallel, and two
agents writing Food and Household how-tos never touch the same folder. The checker refuses a file
whose folder is not its category's top node, so the two cannot disagree.

## The page

### Layout (native first; the web mirrors it)

Three panes side by side on a wide window and stacked on a narrow one, with the split remembered,
reusing `widgets::panes` (the Files page's layout since 2026-10-19), and a "How this page works"
fold by the heading (`widgets::how_it_works`).

1. **The rail** (left), built with `widgets::tree`, folding to the path of what is open, as the
   Library's rail does:
   - a search box. It searches titles, summaries, steps, tools and **the signs in `problems`**,
     so "jar lid pops up" finds "Test the seals on home-canned jars". People usually arrive with a
     symptom, not a title.
   - **Doing now**: this person's unfinished real runs, each with its next step or the time left
     on its timer.
   - **Due soon**: repeats due in the next 14 days, and overdue ones.
   - the category tree with counts; planned how-tos dimmed.
2. **The list** (middle): the how-tos in the chosen branch, each a row with its title, its line,
   time, difficulty in words, hazard word and icon, review state, and once possessions exist,
   "You have 3 of 5 things". Sort: Suggested, A to Z, Quickest. Chips: Also in the game, Easy,
   Under 30 minutes.
3. **The how-to** (right), in this order, because the order is the safety design:
   - the title, summary and "For", then time, difficulty, hazard, review state, written by;
   - the Short or Full switch;
   - the **Stop and call someone if** box, when there is one, and for Health the emergency number
     for the person's place;
   - **What you need**: tools and materials, each with I have this (real) or the game count
     (game), and Find it on the Market for what is missing;
   - **Start** (or Continue), Repeat, Add to Tasks, Print, and Practise in the game when the game
     models it;
   - **Steps**: a numbered checklist with the current step marked by a marker and "Step 3 of 9",
     never by colour alone; safety notes inline at their step; a timer on each wait step; Yes and
     No on each check; a field on each measure step with last time's value; a call step opens
     its how-to inline;
   - **It worked if**: the checks at the end, then It worked, It did not work, or Stop;
   - **If something goes wrong**: each problem as what you see, why, and what to do, with
     "Get help" on the ones marked `stop`;
   - **Who can help**: providers who do it, who teach it, and the Help and Services websites;
   - **Learn more**: the Library guides, the Curriculum topics, the skills;
   - **Your history**: past runs of this how-to with their dates, notes and recorded values (a
     maintenance log: "Filter changed 2026-10-23, 16 x 25 x 1");
   - **Sources**: each with its date read, and the review state; "Something wrong here?" opens
     the bug report with the how-to and version filled in.

### Doing one

- Start creates a run; ticking a step stamps it; a wait step's Start sets its due time and the
  rail shows the countdown. The page asks for a repaint once a second only while a countdown is
  on screen.
- A check answered No jumps to its problem or step; the run remembers the branch.
- Leaving the page, closing the app or restarting the computer loses nothing: the run is on disk
  after every tick.
- A timer that ends while the app is open posts a notice (the HUD and page notices that exist);
  one that ended while it was closed is the first thing the rail says next time. The web asks
  once for browser notifications, opt-in. Windows notifications for the desktop app are a later
  rung, loaded at run time like Media Foundation, never linked, so a Windows "N" edition still
  starts.
- Finishing asks It worked or It did not work. "Did not work" opens the problems.
- Nothing ticks a real step on a guess. Only the person ticks real steps.

### In the game

The same files, inside the world (docs/design/two-realities.md: the app is real life, the game
is a place you walk into):

- **The Crafting page** lists every how-to whose `realities` include `sim`, grouped by the same
  category tree, pruned to branches that have any, so `data/crafting/categories.json` goes away
  (increment 4). The craft queue, stations, the draw from backpack and home storage (BUG-147) and
  quality grades are unchanged.
- **Simplified** (Settings > Gameplay > Making things): Make it is one click and the recipe's
  time, today's behaviour.
- **Realistic**: Do it step by step runs the how-to. Steps with `sim.watch` tick when the game
  sees the thing happen, from the same events the quest objectives already count (what the
  crafting, food and farming systems report, and `src/engine/quest_hooks.rs` for views), so there
  is no new per-frame scan; `read` steps ask the player to confirm; waits run on the world
  clock (a 24 hour wait is 24 hours at 1x and 20 minutes at 72x). The current step shows on the
  HUD under the quest line. Finishing performs the `makes` transform, with the skill XP and
  quality that doing it properly earns (multipliers in data, not code).
- **Between the two:** a real how-to the game models shows Practise in the game, which enters the
  world and opens it on the Crafting page; on the Crafting page, Do it for real opens it on the
  How-to page when the player steps out. Neither page shows the other reality's runs, so there is
  no toggle and no mixed list.
- A server can later require Realistic making in its world, the same way the in-app-ops table
  plans for carrying and death rules.

### The web mirror

`/how-to`, the same three panes (stacked on a phone), reading the same `data/how_to/` files,
`index.json` first and each how-to's file on open; runs and repeats in IndexedDB; the Short or
Full choice in localStorage. Deep links use the Library's grammar: `/how-to#<id>` opens a how-to
and `/how-to#<id>/<step id>` scrolls to a step, the same on both clients. The service worker keeps
every how-to that was opened, and the whole Emergencies branch, so they work offline. A print
stylesheet makes Print a clean page. `tests/page_parity_lint.rs` holds the two to the same data.

### How it connects to the other pages

| Page | How it connects |
|---|---|
| **Crafting** | A view of the same how-tos in the game (above); recipes move into how-to files in increment 4. |
| **Library** | The Library says why; the how-to says what to do. A guide embeds its how-to with a fenced code block whose language is `how-to` and whose one line is the how-to's id, which both clients' markdown readers render as the steps in Short view with Open in How-to, so the steps exist once. The guide keeps its hazards, worked example and "Know where your own work stops". The Dictionary's Define words mode works on a how-to too. |
| **Curriculum** | A how-to lists `topics`; the Curriculum view shows each topic's how-tos and `scripts/curriculum-status.js` counts them. Not a fifth required layer until the content exists, so the goalposts do not move under the topics already counted done. |
| **Skills** | A how-to lists `skills`. In the game, doing it earns XP. In real life nothing is recorded unless the person presses Add to my skills, which writes reality XP to their Skill DNA (`src/relay/storage/skill_dna.rs`, which already keeps reality and fantasy XP apart); others can then endorse it. |
| **Tasks** | Add to Tasks makes a task linked to the how-to and the run; finishing the run offers to move it to Done. The Learn by doing panel lists how-tos in progress and the HumanityOS > Getting started how-tos (increment 6). |
| **Calendar** | Shows repeats' due dates and running timers' end times, asked from the how-to data each time it draws, never copied. The native Calendar keeps its events only in memory today (`GuiState.cal_events`) and has no repeats; increment 3 gives it saved events and repeats, which it needs anyway. |
| **Apps** | A How-to tile in the green HumanityOS tools part (`data/gui/humanityos_tools.json`); the Help and Services parts link the how-tos of their category. |
| **Market** | "Find it on the Market" for a missing tool or material (offerings by `item_ref`); "Who can help" (above). |
| **Inventory** | In the game, the counts come from the backpack and home storage. In real life, "I have this" writes to the person's real possessions (item, count, where it is kept), the first rung of the real Inventory that two-realities.md asks for; the real Inventory page reads the same store when it is built. |
| **Laws** | The shared location setting; a how-to's legal notes per place; the jurisdiction chain for variants. |
| **Server Settings and the Relays page** | A `server_owner` how-to's action steps are the Admin map's actions; the Admin map links each action to the how-tos that use it. |
| **Quests** | A quest step may point at a how-to ("Learn: Purify water"). Quests keep their rewards, story and HUD lines. |
| **Testing** | Separate, see below. Shares the step checklist widget. |
| **Help (`?`)** | A help topic for the page in `data/help/topics.json`. |

## The platform's own how-tos

The HumanityOS branch holds three kinds: using the app (Back up your recovery phrase, Restore
from recovery phrase, Restore a save snapshot, Report a bug), running a server (Host a server on
this PC, Back up your server, Restore it, Move it to a new machine, which names the keys that must
travel with it, `data/backup.key` and `data/erased-accounts.key`), and, with Developer mode on,
developing HumanityOS (Cut a release, Merge an agent's work, Run the game checks).

- **Action steps** name an admin action. `data/admin/ops_registry.json` gains a stable `id` per
  action and per planned action (78 actions, 10 planned and 9 shell-only today; none has an id),
  plus `danger` (safe, caution, destructive: the in-app-ops levels) and, where the app can do it
  directly, the app's handler. The step renders as that action's button with its confirm; where
  the action lives elsewhere, the step shows the registry's `how` and where to go.
- **Command steps** show the command with Copy. Each one in a `server_owner` or `everyone`
  how-to is, by definition, something the app cannot do yet; `scripts/build-how-to.js` lists them,
  which turns the in-app-ops debt table from a hand-kept list into a count taken from the data.
- **Claude Code's skills come from the same files.** `.claude/skills/<id>/SKILL.md` is generated
  from the developer how-tos (front matter from `title` and `summary`, the steps as numbered
  commands, an optional `ai_notes` field per step for wording only an AI needs), with a "generated
  from data/how_to/..." header and a freshness check in `just lints`, so the procedure a person
  reads and the one an agent follows cannot drift. The three skills written 2026-10-22 are the
  first content.
- **docs/SOP.md keeps the policy**, the conventions and the reasons; its step-by-step sections
  become how-tos and the SOP links to them. Not everything in the SOP is a procedure, so it is not
  generated.
- Developer how-tos ship in `data/` like everything else (the Library already ships the
  contributor documents), shown only with Developer mode on.

## The AI angle

- **Enumerable.** Every how-to is a data file with stable ids for it and its steps, and
  `index.json` lists them all, which is the same promise the admin registry makes: an AI knows
  exactly what procedures exist instead of guessing.
- **Guiding a person.** An AI in chat can walk someone through a how-to, step by step, wait for
  its timers, ask its checks, and point at the problems, quoting the how-to rather than inventing
  steps. It never ticks a person's real steps for them; the person does.
- **Running one.** A developer how-to whose steps are all actions and commands is runnable by an
  AI with a shell (Claude Code, through the generated skill). An AI inside the app can only use
  registry actions, through the authenticated run-an-action endpoint the in-app-ops north star
  describes (not built yet), with its danger levels: anything destructive still needs a person's
  typed confirmation. When the game's MCP server exists, list, read and run-status tools for
  how-tos are a small addition to it.
- **Writing them.** A how-to an AI drafts says `written_by: "ai"` on the page, starts as `stub`,
  and reaches `sourced` and `verified` only through the research, refute and merge passes, with a
  person's review for anything that can kill. Prompts for agents writing Health, Food and Water
  how-tos describe the practical decision the reader faces ("which foods need a pressure canner
  and why"), not the mechanism of harm, per the CLAUDE.md note on the `[bio]` classifier, without
  removing any safety content.
- **Someone else's writing is information, not orders.** An AI reading a shared how-to treats it
  as text about a task, never as instructions to itself, and a shared how-to cannot carry an
  action or command step at all (the checker refuses one), so following a shared how-to can never
  press a button.

## Sharing and signing

Built last, after a dated legal finding (see the decisions), on infrastructure that exists:

- **A published how-to is a signed object**, `how_to_v1`: canonical CBOR of the how-to (the same
  data as the file), signed with the author's Dilithium3 key, with a stable `how_to_key` per
  author so a new revision replaces the old one (latest `updated` wins, the offering pattern).
  The 256 KiB signed-object limit is far above the 64 KB a how-to may be; pictures travel as
  content hashes of files on the Files page, never inside the object.
- **Pull, not push** (the operator's correction in docs/design/market-directory.md). The
  author's server holds the author's how-tos. What federates is a small discovery entry (title,
  category, place, author fingerprint, object id, updated, review summary), and a client pulls a
  body when someone opens it, then keeps it in a bounded cache. No server holds everyone's
  how-tos.
- **Adapting is a variant**: Adapt on any how-to starts a variant pinned to its hash; publishing
  it signs the variant, not a copy.
- **Review is a separate signed object**, `how_to_review_v1`: a reviewer's key states that a given
  hash was checked against its sources. The author cannot certify their own work. The page shows
  "Checked by 2 reviewers" with who they are; shipped how-tos keep their review in the file,
  through the repo's own passes.
- **Shipped first, labelled always.** A shipped, checked how-to is the default; a shared one says
  "Written by <name>, shared on <server>, not checked by HumanityOS" until reviewed, and is never
  ranked above a checked one on safety ground (Health, Food keeping, Water, Electrics, Gas,
  Emergencies).
- **Reporting and withdrawing**: a report rides the existing report system (`reports_v2`, a new
  context kind `how_to`); an author withdraws with an empty revision, as a profile withdraws with
  an empty name.
- **Quotas are a server setting**, GUI-first: how many how-tos a member may publish and how big,
  in Server Settings, listed in the Admin map.

## Accessibility and languages

- **Text and targets** follow the theme: sizes from `font_size_*`, tick boxes and buttons at least
  `button_height` tall, so wet or gloved hands can hit them. No literal colours (the theme lint).
- **Keyboard**: Up and Down move between steps, Space ticks, T starts the step's timer, Enter
  opens; every action reachable without a mouse.
- **Never colour alone**: the current step has a marker and its number; hazards have a word and an
  icon; review states are words.
- **Plain words**: short imperative steps; the Dictionary's Define words mode; the reading level
  is part of review (a sentence a beginner cannot follow is a defect).
- **Offline and on paper**: shipped how-tos are on the device; Print gives a clean page (the web
  print stylesheet; on the desktop, an export the browser prints); the Emergencies branch is the
  one to print and keep by the door.
- **Images** require alt text; the checker refuses one without.
- **The web mirror** uses real HTML structure: an ordered list for steps, real checkboxes with
  labels, a live region for timers.
- **Later rungs**: read aloud (the browser's speech on the web; Windows speech loaded at run
  time on the desktop), and a hands-busy view with one step per screen in very large text.
- **Languages**: English only for now. The page's own words go through the i18n table when the
  native app gains one (`data/i18n/` holds web strings today). The content is translated per
  how-to: a sibling file `<id>.<lang>.json` holds only text fields keyed by step id, so a
  translation can never change a quantity, a time, a step's order or a safety level. A how-to's
  review covers its English; a translation carries its own review state.

## Sizes and budgets

Decided now so the first slice is built for the scale it will reach.

| What | Budget | How it is kept |
|---|---|---|
| Shipped how-tos | ~400 after increment 4 (386 from recipes), ~1,000 within a year, designed for 10,000 | one file each, one index |
| One how-to | typically 2 to 8 KB; the checker warns at 16 KB ("this is turning into a Library guide") and refuses 64 KB | why and background belong in the Library |
| Native load | at 1,000 files (~5 MB), read and parsed on a background thread at start: under 300 ms on the operator's PC, 0 ms on the frame thread. Past 3,000, the index first (one file) and bodies on open (under 2 ms each) | measured in the startup timings (`src/boot_timing.rs`) when built |
| Native memory | under 10 MB for 1,000 parsed how-tos; past 3,000, the index plus the 200 most recently opened | an LRU |
| Frame cost, page open | under 1 ms at 1,000 rows (rows drawn only when visible); search runs when the text changes, never per frame, under 5 ms per keystroke at 1,000, on a worker beyond | the snapshot and the frame cost rig |
| Frame cost, page closed | one timer check per second, under 0.01 ms | no work per frame |
| Frame cost, game | game steps tick from the existing event hooks, under 0.01 ms per frame with a run active | no per-frame scan |
| A run | under 16 KB without photos; photos scaled to 1,600 px, under 400 KB each, at most 20 per run | shown in Settings > Data |
| Web | `index.json` under 200 KB compressed at 1,000; a how-to under 8 KB per open; opened ones and Emergencies kept offline | the service worker |
| Network, shipped | none on the desktop; the web fetches the index once a session | |
| Network, shared | discovery entries under 1 KB each; bodies only when opened; no bodies gossiped | the pull model |
| A server | its own members' how-tos only, within its quota (say 200 per member, admin-set), plus peers' discovery entries with an expiry | Server Settings |
| A device's cache of shared how-tos | at most 2,000 or 50 MB, pruned by expiry and least recently opened | the client |
| Runs never leave the device | no telemetry, no counts sent, nothing unless the person shares | by design |

## What not to build, and why

- **Not a second Library.** A how-to is the steps; long explanation is a Library guide that
  embeds it. The size warning holds the line.
- **Not a second crafting system.** One model; the Crafting page is a view.
- **Not a list of helpers built from people's skills.** Help comes from published Market
  offerings only, so nobody becomes contactable by having a skill.
- **No buttons in shared how-tos.** Action and command steps exist only in shipped how-tos, which
  come through the repo and its review.
- **No guessing that a real step was done.** Only the person ticks real steps.
- **No popularity ranking for safety.** Stars and vote counts measure liking, not correctness;
  the page shows review state. An opt-in "this worked for me" may come later, as information.
- **No game XP for real life**, and no real skill record without the person's choice: the two
  realities stay apart except where the person joins them.
- **The Testing page stays separate.** QA cards check a shipped feature for a release and post to
  chat; they are not procedures anyone learns, they come and go with versions, and folding them
  in would bury the real how-tos under developer checks. They share the checklist widget.
- **Quests stay quests.** They carry rewards, story, chains and the HUD's 64-character lines; a
  quest step can point at a how-to.
- **No video hosting in how-tos.** A step can link a video; files live on the Files page.
- **No paid how-tos.** The platform lists and teaches; providers' services are offerings with
  their own settlement.
- **Nothing copied from copyrighted sources.** Facts restated in our own words, cited (the
  sources registry's `use` rule; Feist v. Rural, 1991, as schemas/source.toml explains).
- **No compatibility code for the old formats.** recipes.csv, the crafting overlays and the
  onboarding chains are replaced outright (no installed base yet, CLAUDE.md 2026-06-30).

## The increment ladder

Each increment ships something real on both clients unless it says otherwise, and states what it
proves. The schema is written whole in increment 1, so later increments fill in what the page
shows, not what the data is.

1. **The model and the page, for real life.** `schemas/how_to.toml` with every field in this
   document; `src/how_to/` (load, check); `scripts/build-how-to.js` in `just validate-data` and
   `just lints`; `data/how_to/categories.json` with the whole starting tree; six how-tos chosen to
   exercise the shape (Change a furnace filter: repeat, measure with last time, a check, help;
   Boil water to make it safe: a wait, a choice by altitude, a role resolved by place; Test the
   seals on home-canned jars: checks with problems, `serious`, taken from the already checked
   water-bath guide; Mix oral rehydration solution: `serious`, Short and Full, and a `makes` block
   naming the existing recipe `mix_ors`, which the checker resolves against recipes.csv until
   increment 4; Back up your recovery phrase and Restore a save snapshot: `go` steps into
   Settings); the `data/items.csv` rows those how-tos need (there is no furnace filter in the item
   taxonomy today); about 30 `absent` how-tos so the gaps count; the native page (rail with Doing
   now and the tree, list,
   view, Short and Full, ticking, timers, checks and branches, outcome, problems, sources, review
   badge) with runs saved; a nav button beside Library and the Apps tile; the help topic;
   PAGES.md, FEATURES.md and terminology rows; the web mirror; a page snapshot and tests (the
   shipped data loads; a run survives its steps being reordered; Short never hides a serious note;
   a stale review hash fails, proven on a broken file). Can be split into 1a (data, model, native)
   and 1b (web), disjoint files.
   *Proves:* the schema holds varied real procedures, one data set renders the same on both
   clients, a run survives restarts and edits, and the safety and review gates can fail.
2. **Steps live once: the Library, the Curriculum, and help.** The fenced how-to block in both
   markdown readers; the canning guides' steps moved into their how-tos, with a check of exactly
   the edited passages and a cold read of the result (the fixes need a pass too); the Curriculum
   shows how-tos per topic and the status script counts them; ids on the catalog's entries; Who
   can help from the catalog and from Market service offerings by verb and item (filtered in the
   client over what the Market already fetches, until the Market's own item index exists, the
   known work in market-directory.md); the optional `how_tos` list on a service offering, a
   persisted format change that the migration-guard agent reviews first; the emergency number from
   the locale.
   *Proves:* a step is written once and shown in three places; services need no new registry.
3. **Your things and your time.** The real possessions store with I have this; Find it on the
   Market; Add to Tasks; Repeat; the Calendar keeps its events and gains repeats (native has
   neither today), and shows due how-tos and timer ends; Due soon; browser notifications, opt-in.
   *Proves:* recurring upkeep works end to end, and the real Inventory has its first data.
4. **Crafting joins.** A one-time script turns recipes.csv, tools.ron and the crafting categories
   into how-to files with `makes` (the CSV's citation comments become `sources`, the station tool
   rules become each how-to's explicit `tools` and a checker rule); `RecipeRegistry` is built from
   how-tos; a golden test proves the registry built from how-tos equals the one built from the
   CSV, field by field, **before** the CSV is deleted; the recipe lints read how-tos; the Crafting
   page and `web/pages/crafting.html` read how-tos; recipes.csv, recipes.json and
   `data/crafting/` are deleted, and the embedded fallback (`src/embedded_data.rs`) carries the
   index, which holds every `makes`, so a bare exe can still craft. Each how-to's id may be its
   recipe's id, which keeps the golden test simple; keeping old saves' crafts in progress loading
   is not a requirement (operator, 2026-10-23: "We're not really concerned about backwards
   compatibility until after v1.0"). The 36 calls of `RecipeRegistry::from_csv` in 17 files (most of them tests) move to
   one helper that builds the registry from the shipped how-tos. Needs a quiet
   window: `src/systems/crafting/mod.rs`, the registries and the embedded data are shared files.
   *Proves:* one model; the economy, machines, food and quests behave exactly as before (their
   tests pass unchanged); the load budget holds at ~400 files.
5. **Practise in the game.** `sim` bindings through the quest hooks; Settings > Gameplay "Making
   things: Simplified or Realistic"; Realistic runs on the Crafting page and the HUD; world-clock
   waits; Practise in the game and Do it for real; XP and quality for Realistic, tuned in data.
   *Proves:* practising in the game is practising the real steps.
6. **The platform's own how-tos.** Ids, `danger` and handlers in the admin registry; action and
   command steps; the command-step gap report; the release, merge, game checks, host, back up,
   restore and move how-tos; the generated skills with their freshness check; the SOP linking
   them; the onboarding chains as how-tos, read by the Learn by doing panel and the web onboarding
   page; `sim_guides.json` retired.
   *Proves:* one source for the procedures people read and agents follow; the app's command-line
   gaps are counted from data.
7. **Writing how-tos in the app.** An editor (GUI-first) for your own how-tos, kept in
   `how_to/mine/`; Adapt as a variant; import and export a file; the checker live as you type; the
   saved location setting shared with Laws.
   *Proves:* anyone can write and adapt a how-to without a text editor, and variants resolve by
   place.
8. **Sharing.** After the legal finding: `how_to_v1` and `how_to_review_v1`, publishing to your
   server, discovery entries, pull and the bounded cache, reports and withdrawal, quotas in Server
   Settings, the shared-and-unchecked label.
   *Proves:* sharing works at federation scale with no server holding everyone's how-tos.
9. **Depth**, each its own small increment: quantities that scale with a batch size and a units
   setting; Print and export; read aloud; the hands-busy view; photos on steps; Do it together (a
   run shared live with a helper over the sealed direct messages); Windows notifications; charts
   of a measured value over time (well water tests); the AI guide in chat; MCP tools; team steps
   with `who`.

## Increment 1 as built (2026-10-23)

What is where: `schemas/how_to.toml` (every field, and the shape check itself: the build reads it),
`src/how_to/` (model, checker, canonical hash, loader, runs, view rules, compiled into the relay
too), `scripts/build-how-to.js` (the JS checker run over every file, every fixture, the index; in
`just lints` and `just validate-data`), `web/shared/how-to-logic.js` (the rules both the website and
the build use), `data/how_to/` (the tree, six how-tos, 48 planned, the page's words, the generated
index), the desktop page `src/gui/pages/how_to.rs` + `how_to/`, the website's `web/pages/how-to.html`
+ `how-to-app.js`. Every gate is shown red by `tests/fixtures/how_to/broken.json`, in both checkers,
on every run.

What building it changed in this plan, each with its reason, so the next increment builds on what
is true:

- **The hazard levels of the first how-tos.** The plan said the seals and the rehydration how-tos
  were `serious` and the furnace filter one, by its example, `caution`. By the scale's own meaning
  ("how badly a person can be hurt by doing it wrong", the Curriculum's) the four real-life how-tos
  are `lethal`: keeping a jar whose lid did not seal is the botulism decision; too little water in
  an oral rehydration drink is dangerous to a baby; water not boiled long enough, or boiled when it
  holds fuel or chemicals, can kill the vulnerable; and the furnace maker's own manual says running
  it without its filter or filter door "could result in fire, personal injury, or death". The
  Curriculum rates the matching topics lethal too. Whether "Can kill you if done wrong" on a filter
  change reads as alarmist is a fair question for the reviewer and the operator; the label follows
  the definition until someone changes the definition.
- **Sources carry a local `key` and the registry id as `source`**, not the registry id alone,
  because one authority gives several pages (two CDC pages in one how-to); `in_project` cites a file
  of this repository for facts about HumanityOS itself; and `dated` (the source's own date, or the
  word `undated`) is required beside `read`. Four authorities joined the registry:
  `carrier-hvac`, `pa-puc`, `ny-doh`, `unicef`.
- **The number rule is wider.** The plan flagged digits in a step with serious or lethal safety
  and no citation. In a serious or lethal how-to, every step, problem and stop-and-call line that
  states a number (in its words, an option's or a measurement's label, a timer, a range) must cite:
  a boiling time sits on a step whose own note is about scalds, not about the time.
- **Whole numbers only, everywhere** (gate `whole_numbers`): canonical CBOR has no floating point,
  so a how-to that will be signed holds 500 ml, never 0.5 l.
- **Field names:** a step's hands-on minutes are `time_min` (the how-to's `time` is an object);
  `sim` is `{ mode: watch | read | skip, kind, target }`; `stop_and_call` and `problems` carry
  `cites`; a how-to may say which HumanityOS `apps` its steps are written for (the recovery phrase
  how-to names the desktop's buttons, which the website's Settings does not have).
- **Planned how-tos are one list,** `data/how_to/planned.json`, not 48 nearly empty files: one
  line to add a gap, and the checker refuses an id that is both planned and written.
- **The page's words are a data file both apps read,** `data/how_to/words.json`, with a test that
  fails on a word either app uses that the file lacks and on a word neither uses; the desktop keeps
  a built-in copy. That is the i18n table the plan waited for, for this page.
- **The Short view's second lock.** Beside the gate, `Step::in_short` (and `inShort` on the
  website) shows a serious or lethal step in Short even if a file said `short: false`.
- **Not built in increment 1, though the page section lists them:** Due soon and repeats (they
  need the Calendar, increment 3), I have this and Find it on the Market (increment 3), Who can help
  (increment 2), Print, the Emergencies branch kept offline, and browser notifications (the website
  announces a timer that ends to its live region and the desktop posts a notice). A call step opens
  the called how-to rather than running it inline as a sub-run. Developer how-tos are shown on the
  website always, as its Dev tab is (it has no Developer mode switch).
- **Content notes the reviewer should look at first:** the boiling time's height line (the CDC
  says 6,500 feet, the EPA 5,000 feet and, on the same page, 1,000 metres; the how-to uses the
  lowest, 1,000 metres, and says why); the furnace steps rest on one maker's manual, which is not
  every furnace's; the rehydration how-to's warning about too little water applies WHO's warning
  about overfilled packets (said so on the page); the seals how-to asks for all three of the
  USDA's tests where the USDA asks for one (said so, as our reading).
- **A fact-check before commit, not a review.** An independent agent read every cited source
  against the six files the same day and found what the writing pass missed: the USDA's gloves
  and skin-contact warning and its disposal steps for a spoiled jar, WHO's "blood in stool" and
  "persistent diarrhoea" limits and the zinc course, two of the gas utility's rules, the furnace
  maker's safety glasses and gloves, 102 F printed as 38.8 C (it is 38.9 C), and five places where
  the author's own reasoning was not marked as such. All were fixed. It is not the independent
  review the review field means: every how-to is still `sourced`, "Sourced, not yet checked", until
  that review, and the four lethal ones until a person has read them and dated it.
- **The service worker** fetches `/data/how_to/` from the network first (a safety fix must reach a
  person on their next visit), falling back to the copy it last fetched offline.

## Decisions for the operator

1. **The name. DECIDED 2026-10-23: How-to.** The operator: "Love it, let's do it." On screen
   "How-to", one is "a how-to", several "how-tos"; in code `how_to` (the module, the page id, the
   data folder, the schema, the signed object `how_to_v1`); on the website `/how-to` and
   `how-to.html`. The in-game making page stays **Crafting**.
2. **Crafting joins the model. DECIDED 2026-10-23: yes, in increment 4,** as recommended: recipes
   become how-to files and the Crafting page becomes a view of how-tos, with Simplified and
   Realistic making. The operator added: "We're not really concerned about backwards compatibility
   until after v1.0." So increment 4 **need not keep recipe ids for old saves**: the line in
   increment 4 that says each how-to's id is its recipe's id so a save naming a craft in progress
   still loads is now a choice, not a requirement (keep the ids if that is simply cleaner, which it
   probably is, since the golden test compares the registries by id). And: "You have full
   permission to close any instances of HumanityOS I may have up when you need to develop
   something" (graceful `taskkill` first, forced after about 15 seconds; CLAUDE.md already says
   so).
3. **The line on dangerous work. NOT ANSWERED YET (2026-10-23).** Recommended: publish how-tos for
   what a householder may lawfully do themselves in at least one place, always with the "stop and
   call" line, and for work that needs a licence (mains electrical, gas lines) a how-to for finding
   and checking a professional instead; and a person's dated review before any "can kill" how-to
   shows as checked, extending the 2026-09-16 Curriculum rule to how-tos. **Increment 1 was built
   with the recommended safety rule** (gate `lethal_without_person`: a lethal how-to cannot be
   verified without `review.person`; the Stop and call someone if box always shown, first, in both
   views), so if the operator answers otherwise, it is one gate to change.
4. **Publishing for others (increment 8). NOT ANSWERED YET (2026-10-23).** Whether people can
   publish how-tos to other people at all, and under what licence. Recommended: yes, under CC0
   like the rest of the project, but only after a dated finding on licensing and on responsibility
   for shared instructions that can hurt someone. Increment 1 has no publishing.

Calls made here that he can overrule in a line: the nav button beside Library; one JSON file per
how-to, foldered by top category; the words Short and Full; the Testing page and the quests kept
separate; help only through Market offerings; developer how-tos shipped and shown in Developer
mode; Claude Code's skills generated from them.

## Still to find out (dated findings before the increment that needs them)

- **Before increment 8:** the licence for shared how-tos, and what responsibility a platform
  carries for a shared instruction that hurts someone, in the places the first servers run.
  Written as `docs/reference/findings/<date>-how-to-sharing.md` from primary sources, with dates
  and quotations, per the findings practice.
- **Before the first licensed-work how-to:** whether any place restricts publishing instructions
  for work it licenses (expected no; the answer and its date get written down either way, since a
  negative finding is worth as much as a positive one).
- **Before any translation ships:** the page name and the hazard words checked by a native
  speaker of each language.
