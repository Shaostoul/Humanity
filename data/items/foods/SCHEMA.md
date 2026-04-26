# Real-world food/drink/product items — schema (v0.117.0)

> **Last updated:** v0.117.0
>
> Lets you look up any consumer item (Oreos, Coca Cola, a household cleaner)
> and compute its toxicology profile per species + exposure route from its
> ingredient list. Ingredient lists change over time — Oreos in 1990 ≠ Oreos
> in 2026 — so items carry a versioned `history` array and the toxicology is
> always derived from the current ingredients, not stored on the item.

## Layout

```
data/items/foods/
  ingredients.csv           — flat catalog: id, name, category, links to chemistry
  ingredients/              — one RON file per ingredient with toxicology
    sugar.ron
    caffeine.ron
    cocoa_alkalized.ron
    ...
  items/                    — one RON file per real-world item
    oreos_original.ron
    coca_cola_classic.ron
    ...
  SCHEMA.md                 — this file
```

## Three layers, no duplication

```
data/chemistry/compounds.csv
   ↓ (foreign key by `chemistry_compound_id` when ingredient is a single compound)
data/items/foods/ingredients/<id>.ron
   ↓ (ingredient list reference)
data/items/foods/items/<id>.ron
```

- **Compound** (`data/chemistry/`): single molecule. Single source of truth for
  pure-substance toxicity (LD50 etc.). Used by chemistry simulations.
- **Ingredient** (`data/items/foods/ingredients/`): consumer-label-level
  ingredient. May be a single compound (sugar = sucrose) or a complex mixture
  (wheat flour). Holds its own toxicology per (species, route) — in the
  mixture case this captures the worst-relevant compound's contribution.
- **Item** (`data/items/foods/items/`): a real-world product. Pure ingredient
  list with version history. **No embedded toxicology** — derived at query
  time by aggregating its current ingredients' toxicities.

This means:
- A new Oreo formulation = new entry in `history` array; never rewrite tox
- 1000 new items don't duplicate ingredient tox; they reference shared sidecars
- Improving the toxicology data for "caffeine" automatically improves every
  item that contains caffeine

## Item RON shape

```ron
(
    id: "oreos_original",
    name: "Oreo Original Sandwich Cookies",
    brand: "Mondelēz International (Nabisco)",
    category: "food/cookies",
    description: "Chocolate sandwich cookies with cream filling. Iconic American snack since 1912.",
    serving_size_g: 34.0,
    sources: [
        "https://www.oreo.com/products/original",
        "USDA FoodData Central FDC ID 392880",
    ],

    // Versioned ingredient history. The CURRENT ingredient list is the entry
    // with the latest `as_of` date. Older entries are kept so toxicology can
    // be re-derived for any historical formulation (useful for medical
    // exposure inquiries: "I ate Oreos as a kid in 1985 — what was in them?").
    history: [
        (
            as_of: "1912-03-06",
            note: "Original launch by National Biscuit Company",
            ingredients: [
                ( id: "wheat_flour_unbleached", note: "" ),
                ( id: "lard",                   note: "original shortening" ),
                ( id: "sugar",                  note: "" ),
                ( id: "cocoa_alkalized",        note: "" ),
                ( id: "salt",                   note: "" ),
                ( id: "leavening_baking_soda",  note: "" ),
                ( id: "vanilla_extract_natural", note: "" ),
            ],
        ),
        (
            as_of: "1990-01-01",
            note: "Switched lard → partially hydrogenated vegetable oil",
            ingredients: [/* ... */],
        ),
        (
            as_of: "2006-01-01",
            note: "Removed trans fats per FDA guidance",
            ingredients: [/* ... */],
        ),
        (
            as_of: "2024-03-01",
            note: "Current US formulation",
            ingredients: [
                ( id: "sugar",                       note: "primary sweetener" ),
                ( id: "wheat_flour_unbleached",      note: "" ),
                ( id: "palm_and_canola_oil",         note: "shortening blend" ),
                ( id: "cocoa_alkalized",             note: "" ),
                ( id: "high_fructose_corn_syrup",    note: "" ),
                ( id: "leavening_baking_soda",       note: "" ),
                ( id: "salt",                        note: "" ),
                ( id: "soy_lecithin",                note: "emulsifier" ),
                ( id: "vanillin_artificial",         note: "synthetic vanilla flavor" ),
                ( id: "chocolate",                   note: "" ),
            ],
        ),
    ],

    accord_constraints: ["transparency", "epistemic_integrity", "harm_minimization"],
)
```

## Ingredient RON shape

```ron
(
    id: "cocoa_alkalized",
    name: "Cocoa (Dutch-processed)",
    category: "flavoring",
    chemistry_compound_id: None,  // Some("theobromine") would link primary
    common_use: "Dark cocoa powder treated with alkali. Source of theobromine.",
    allergen_class: "",
    description: "Cacao solids treated with potassium carbonate to reduce bitterness. Contains theobromine + caffeine + small amounts of phenethylamine.",
    composed_of_compounds: ["theobromine", "caffeine"],
    sources: [
        "https://www.fda.gov/...",
        "https://en.wikipedia.org/wiki/Theobromine",
    ],
    last_verified: "2026-04-25",

    // Toxicology by species and exposure route. ROUTES:
    //   ingested | topical | inhaled | injected
    // VERDICTS:
    //   safe / safe_in_moderation / caution / toxic / lethal / unknown
    //
    // For mixtures (like cocoa), the verdict reflects the worst contributing
    // compound at typical exposure for THAT INGREDIENT (e.g. cocoa = ~1-2%
    // theobromine by mass). For single compounds, copy from compounds.csv
    // and elaborate on species differences.
    toxicology: {
        "human_adult": {
            "ingested": (
                verdict: "safe_in_moderation",
                details: "Bitter alkaloids in cocoa are mood-altering at high dose; 1g/kg can cause headaches and insomnia.",
                lethal_dose_mg_kg: Some(1000.0),
                notes: ["lethal at multi-kilogram doses, never reached in food"],
            ),
        },
        "dog": {
            "ingested": (
                verdict: "toxic",
                details: "Theobromine in cocoa is metabolized 4× slower in dogs. ~10g cocoa is mildly toxic to a 10kg dog (vomiting, restlessness), potentially lethal to a 5kg dog.",
                lethal_dose_mg_kg: Some(100.0),
                notes: [
                    "theobromine LD50 in dogs ~100-200 mg/kg",
                    "ASPCA Animal Poison Control: 1-888-426-4435",
                ],
            ),
        },
        "cat": {
            "ingested": (
                verdict: "toxic",
                details: "More sensitive than dogs due to slower hepatic glucuronyl transferase.",
                lethal_dose_mg_kg: Some(80.0),
                notes: ["cats lack the enzyme to detoxify methylxanthines efficiently"],
            ),
        },
    },
)
```

## Deriving an item's toxicology

Aggregation rule: for each (species, route) pair, take the **worst verdict
across all ingredients in the current formulation**. Combine details into
a list of "ingredient → verdict, details" so the user sees what's driving
the warning.

Pseudocode:
```
fn item_toxicology(item, species, route):
    current_ingredients = item.history.last().ingredients
    verdicts = []
    for ing in current_ingredients:
        ingredient = load_ingredient(ing.id)
        if let tox = ingredient.toxicology[species][route]:
            verdicts.push((ing.id, tox))
    # Reduce: worst verdict wins. Collect details from all toxic+ ingredients.
    return aggregate(verdicts)
```

Verdict ordering (worst → best):
```
lethal > toxic > caution > safe_in_moderation > safe > unknown
```

## Versioning protocol

When a manufacturer changes a formulation:

1. Verify the change from the manufacturer's current label or an authoritative
   source
2. Append a new entry to the item's `history` array with the change date and a
   short `note` describing what changed
3. Cite sources (label, press release, FOIA, etc.)
4. Bump `last_verified` on any newly-touched ingredient sidecars
5. The aggregator automatically uses the latest `as_of` entry; old entries
   stay queryable for historical exposure inquiries

## Ingredient catalog CSV columns

```
id,name,category,chemistry_compound_id,common_use,allergen_class,description
```

This stays as the **flat index** for browsing. Each row should also have a
sidecar RON file under `ingredients/<id>.ron` with full toxicology. The CSV
is the cheap-to-search index; the RON files are the rich data.

## Toxicology verdict scale

| Verdict | Meaning |
|---|---|
| `safe` | No documented harm at any reasonable exposure |
| `safe_in_moderation` | Safe at normal exposures; harmful at extreme/chronic excess |
| `caution` | Safe for most but risk groups exist (allergens, pregnancy, etc.) |
| `toxic` | Documented harm at typical exposures for this species/route |
| `lethal` | Documented mortality at typical exposures |
| `unknown` | No reliable data |

## Performance / scaling

See `docs/design/storage-architecture.md` for the full search-performance
section. Quick numbers: at 1B items in SQLite + FTS5, indexed lookups by ID
or exact name are sub-millisecond; full-text search is sub-100ms; aggregate
toxicology computation per item is O(n_ingredients) ≈ <1ms per item.

## Adding a new item

1. Verify ingredients from manufacturer's current label
2. For each ingredient, ensure it exists in `ingredients.csv` AND in
   `ingredients/<id>.ron` with toxicology — add if missing
3. Write the item RON with a single `history` entry (current formulation)
4. Cite sources and `last_verified` date
5. The toxicology is auto-derived; you do not write it on the item

## Related

- `data/chemistry/compounds.csv` — pure-compound molecular data (FK target)
- `data/chemistry/toxins.csv` — known acute toxins (cross-reference)
- ASPCA Animal Poison Control https://www.aspca.org/pet-care/animal-poison-control
- FDA Food Allergens https://www.fda.gov/food/food-labeling-nutrition/food-allergies
- USDA FoodData Central https://fdc.nal.usda.gov/
