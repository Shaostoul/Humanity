# Real-world food/drink/product items — schema

> **Last updated:** v0.116.0
>
> Lets you look up any consumer item (Oreos, Coca Cola, a household cleaner)
> and see its ingredient list, what each ingredient is, and what its
> toxicology profile looks like for humans + common pets per exposure route.

## Files

- `ingredients.csv` — flat catalog of every ingredient name we know about,
  optionally linked to `data/chemistry/compounds.csv` so the chemistry layer
  fills in molecular detail.
- `<item>.ron` — one file per real-world item (e.g., `oreos.ron`,
  `coca-cola-classic.ron`).

## Item RON shape

```ron
(
    id: "oreos_original",
    name: "Oreo Original Sandwich Cookies",
    brand: "Mondelēz International (Nabisco)",
    category: "food/cookies",
    description: "Chocolate sandwich cookies with cream filling.",
    serving_size_g: 34.0,
    sources: [
        // Where the ingredient list was pulled from (manufacturer label, FDA, etc.)
        "https://www.oreo.com/", "https://www.mondelezinternational.com/",
    ],
    last_verified: "2026-04-25",

    ingredients: [
        // Each entry references ingredients.csv by id.
        // `note` is free-text — quantity hints, allergen flags, processing notes.
        ( id: "sugar",                       note: "primary sweetener" ),
        ( id: "wheat_flour_unbleached",      note: "" ),
        ( id: "palm_and_canola_oil",         note: "shortening blend" ),
        ( id: "cocoa_alkalized",             note: "Dutch process" ),
        ( id: "high_fructose_corn_syrup",    note: "" ),
        ( id: "leavening_baking_soda",       note: "" ),
        ( id: "salt",                        note: "" ),
        ( id: "soy_lecithin",                note: "emulsifier" ),
        ( id: "vanillin_artificial",         note: "synthetic vanilla flavor" ),
        ( id: "chocolate",                   note: "" ),
    ],

    // Toxicology by species and exposure route.
    // Routes: "ingested" | "topical" | "inhaled" | "injected"
    // Verdict per (species, route):
    //   safe / caution / toxic / lethal / unknown
    // `details` is human-readable explanation; `lethal_dose_mg_kg` if known.
    toxicology: {
        "human_adult": {
            "ingested": (
                verdict: "safe_in_moderation",
                details: "Standard processed cookie. High in sugar and refined carbs; no acute toxicity at normal serving sizes. Long-term overconsumption contributes to metabolic disease.",
                lethal_dose_mg_kg: None,
                notes: ["allergens: wheat, soy", "may contain milk traces"],
            ),
            "topical": ( verdict: "safe", details: "No skin reaction expected.", lethal_dose_mg_kg: None, notes: [] ),
            "inhaled": ( verdict: "caution", details: "Choking hazard for crumbs; not designed for inhalation.", lethal_dose_mg_kg: None, notes: [] ),
        },
        "human_child": {
            "ingested": (
                verdict: "safe_in_moderation",
                details: "Same as adult but smaller body weight means sugar load is proportionally larger; choking risk for whole cookies under age 4.",
                lethal_dose_mg_kg: None,
                notes: ["age 4+ recommended for whole cookies"],
            ),
        },
        "dog": {
            "ingested": (
                verdict: "toxic",
                details: "Cocoa contains theobromine and caffeine, both toxic to dogs. A standard 3-cookie serving (~10g cocoa) is mildly toxic to a 10kg dog and potentially lethal to a 5kg dog. Sugar + fat content separately risks pancreatitis.",
                lethal_dose_mg_kg: Some(100.0),
                notes: ["theobromine LD50 in dogs ~100-200 mg/kg", "call vet immediately on ingestion"],
            ),
        },
        "cat": {
            "ingested": (
                verdict: "toxic",
                details: "Same theobromine toxicity as dogs but cats are MORE sensitive due to slower metabolism. Even one cookie is dangerous for an average cat.",
                lethal_dose_mg_kg: Some(80.0),
                notes: ["cats lack glucuronyl transferase to detoxify methylxanthines"],
            ),
        },
    },

    accord_constraints: ["transparency", "epistemic_integrity"],
)
```

## Ingredient CSV columns

```
id,name,category,chemistry_compound_id,common_use,allergen_class,description
```

- `id`: kebab-case key referenced by item RON
- `name`: human-readable
- `category`: sweetener / fat / leavening / emulsifier / colorant / flavoring /
  preservative / acidulant / etc.
- `chemistry_compound_id`: foreign key into `data/chemistry/compounds.csv` (or
  empty if no exact compound mapping — e.g., "wheat flour" is a mixture)
- `common_use`: brief 1-line role
- `allergen_class`: empty | "wheat" | "soy" | "milk" | "egg" | "tree_nut" |
  "peanut" | "fish" | "shellfish" | "sesame" (US Big-9 allergens)
- `description`: longer context

## Toxicology verdict scale

| Verdict | Meaning |
|---|---|
| `safe` | No documented harm at any reasonable exposure |
| `safe_in_moderation` | Safe at normal exposures; harmful at extreme/chronic excess |
| `caution` | Safe for most but risk groups exist (e.g., allergens, pregnancy) |
| `toxic` | Documented harm at typical exposures for this species/route |
| `lethal` | Documented mortality at typical exposures |
| `unknown` | No reliable data |

## Why this lives here

- The `data/chemistry/` files cover individual compounds and pure substances
- Real-world items are mostly *mixtures* of compounds + biologically-derived
  ingredients (flour, oils, herbs, etc.)
- Pet poisoning is a leading cause of avoidable harm; surfacing it inline with
  ingredient lookup serves the Accord's "harm minimization" constraint
- Cross-species comparison (human/dog/cat) is a query that no consumer label
  currently provides — closing that gap is high user value

## Adding a new item

1. Verify ingredients from the manufacturer's label or an authoritative source
2. For each ingredient, ensure it exists in `ingredients.csv`; add if missing
3. Write the item RON with toxicology entries for at least `human_adult` + any
   common-pet species the ingredients are known to affect
4. Cite sources and `last_verified` date — toxicology data ages
5. Run `cargo check` and a quick parse test

## Links

- `data/chemistry/compounds.csv` — pure-compound molecular data
- `data/chemistry/toxins.csv` — known acute toxins
- ASPCA Animal Poison Control https://www.aspca.org/pet-care/animal-poison-control
- FDA Food Allergens https://www.fda.gov/food/food-labeling-nutrition/food-allergies
- USDA FoodData Central https://fdc.nal.usda.gov/
