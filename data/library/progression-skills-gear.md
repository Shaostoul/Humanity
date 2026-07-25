# Progression: skills, abilities, gear

> Written 2026-07-07 against the verified codebase survey. Companion to
> [gameplay-loop-map.md](gameplay-loop-map.md) (X2 layer). Everything here
> extends shapes that already exist in code or data; nothing proposes a
> parallel system where one is half-built. Schemas are concrete so an
> implementing session writes loaders against them, not interpretations.

## Design pillars

1. **Learn by doing.** XP comes from performing the act (craft, harvest,
   mine, treat, trade), never from abstract kill-score. Already live via the
   xp_grants channel; every new system that adds a verb adds an XP call.
2. **Skills gate and scale, gear extends, abilities activate.** A skill is
   who you have become (permanent), gear is what you carry (situational),
   an ability is what you can DO on demand (actioned, costed). Three layers,
   one stat pipeline.
3. **One stat grammar everywhere.** status_effects.csv already speaks
   `stat:value:op` and net_stat_multiplier() already folds it. Gear
   modifiers, ability buffs, and skill perks all use the SAME grammar and
   the SAME fold. No second modifier system, ever.
4. **Real/Sim split is data, not code.** The relay's skill_dna table already
   separates reality_xp from fantasy_xp. Canonical model: every XP grant is
   tagged real|sim by its source; fantasy-flavored content (spells) simply
   does not exist in Real mode's data view.
5. **Infinite-of-X.** New skill/ability/gear = new row. The schemas below
   are the contract.

## Current state (verified)

- SkillSystem live: 20 skills (data/skills/skills.csv), XP curve
  base*level^1.5, level gates on recipes (level 2+), XP from crafting,
  farming, mining. 10 of 20 skills have no XP source (all combat, social,
  and most survival-category skills). Level-ups log only; no player
  feedback. Skill level does nothing but gate.
- Quests partial: Gather/Craft/Harvest objectives work; Build/Travel/Talk
  have no emitters; no accept UI; QuestTracker not saved; rewards are items
  only (no XP field).
- Equipment DORMANT: six slots (data/inventory/equipment_slots.json) render
  in the GUI; Equip copies a name string into page-local state with no slot
  matching, no persistence, no effect. Meanwhile the Outfit component
  (cosmetics) correctly models slot->item and persists. Two forks of the
  same idea; the cosmetic one is the better-built.
- Abilities ABSENT in engine; data/spells.csv (110 rows: mana, cast time,
  cooldown, range, AoE, damage, healing, DoT, skill gate) and
  data/enchantments.csv (107 rows) fully authored with schemas, zero
  loaders.
- Weapons in items.csv (23 rows) carry no damage stats; Armor component
  exists (per-damage-type resistance) read by dormant CombatSystem,
  inserted by nothing. Hardpoints component (mount slots) has no consumer.
- THREE progression stores: native PlayerSkills (canonical), relay
  game_state JSON xp/reputation (hardcoded quest chain), relay skill_dna
  (reality_xp/fantasy_xp per user). They share nothing.

## Part 1: Skills

### Schema (extend data/skills/skills.csv, add columns)

```
id,name,category,max_level,xp_per_level,description,scales,perk_levels
metalworking,Metalworking,crafting,25,150,"...",craft_speed:0.02|craft_quality:0.01,5:dual_smelt|15:master_alloys
```

- `scales`: pipe list of `stat:per_level` bonuses folded through
  net_stat_multiplier as `skill_<id>` pseudo-effects. Start with exactly
  three consumed stats: craft_speed (crafting tick), yield (harvest +
  mining quantity roll), craft_quality (reserved until quality exists).
- `perk_levels`: pipe list of `level:ability_id` unlocks granting rows from
  the abilities file (Part 2). This is the skill tree WITHOUT a tree
  widget: linear per-skill unlock lists, readable in one row.

### Rules

- Keep exactly 20 skills. Depth comes from perk unlocks, not skill count.
  A new skill requires retiring or splitting an old one (taxonomy budget).
- Every untrainable skill gets its trainer when its system activates
  (combat skills with CombatSystem, trading with the vendor, navigation
  with the unified map travel events, foraging with wild Harvestables).
  Rule for implementers: registering a system that adds a verb REQUIRES
  wiring its award_skill_xp call in the same increment.
- Level-up feedback: consume SkillSystem::drain_level_ups (already written,
  zero callers) into a HUD toast + a chat-style log line.

## Part 2: Abilities

Abilities are the activation layer: castable/usable actions with costs and
cooldowns. data/spells.csv is the authored content; the engine work is one
loader + one component + one action pipeline.

### Canonical file: rename data/spells.csv -> data/abilities.csv

The rename matters: rows already include tech abilities and powers, not
just fantasy spells, and Real mode must be able to filter by flavor.
Add/confirm columns:

```
id,name,flavor,school,skill_required,skill_level,cost_stat,cost,cast_time_s,cooldown_s,range_m,aoe_shape,aoe_radius_m,effects,description
heal_minor,Minor Mend,fantasy,restoration,medicine,3,energy,15,1.5,8,3,single,0,heal:20|apply:regeneration,"..."
scan_ore,Ore Scan,tech,sensors,mining,2,energy,10,0.5,30,50,sphere,50,mark:ore_nodes,"..."
```

- `flavor`: real|tech|fantasy. Real mode shows real+tech. Sim settings
  choose whether fantasy loads at all (a data VIEW, not a code path).
- `cost_stat`: which Vitals field pays (energy is the default mana; this
  makes abilities part of the survival economy: casting makes you hungry).
- `effects`: pipe list of typed verbs. v1 verbs: damage:<n>:<type>,
  heal:<n>, apply:<status_effect_id>, mark:<what>. Every verb resolves
  through EXISTING pipelines (DamageEvent queue, Health, StatusEffects
  component). New verbs are added to one match statement.

### Engine pieces (one session)

1. AbilityRegistry loader (CSV -> DataStore, mirror ItemRegistry).
2. KnownAbilities component (Vec<ability_id>) granted by skill perk_levels
   and/or quest rewards; persisted in WorldSave.
3. ability_request channel (GUI -> system), cast validation (cost, cooldown,
   range), effect resolution through the verb match.
4. Hotbar UI: 1-9 slots on the HUD, assignable from a Known Abilities panel
   (reuse the item-tile widget family).

Cooldowns/costs make abilities the SAME shape as machine automation
(validate, consume, timed effect), so implementers should crib from
CraftingSystem's request pattern, not invent a new one.

## Part 3: Gear

### Schema: new file data/equipment.csv (join on items.csv id)

Keep items.csv lean (it is 500+ rows of mass/volume physics); equipment
stats are a sparse property of few items:

```
item_id,slot,armor_kinetic,armor_thermal,armor_energy,armor_chemical,armor_radiation,damage,damage_type,range_m,stat_modifiers,hardpoint_slots
jacket_insulated_0,chest,0.05,0.40,0,0.10,0,0,,,cold_resist:0.5:add,
pickaxe_steel_0,hands,0,0,0,0,0,12,kinetic,2,mine_speed:0.15:add,
exo_frame_0,back,0.10,0,0.05,0,0,0,,,carry_capacity:25:add,2
```

- Slots come from data/inventory/equipment_slots.json (unchanged).
- Armor columns populate the EXISTING Armor component's resistance map on
  equip (per damage type, matching CombatSystem's mitigation math).
- `stat_modifiers` uses the status_effects grammar verbatim and folds
  through net_stat_multiplier as `equip_<slot>` pseudo-effects. First
  consumed stats: speed (already consumed today), carry_capacity (weight
  cap), cold_resist/heat_resist (feed the FoodSystem temperature damage
  gate), mine_speed/craft_speed (their systems' tick multipliers).
- `damage`/`damage_type`/`range_m`: the held-tool columns CombatSystem's
  DamageEvent needs; also how the pickaxe eventually differentiates mining.
- `hardpoint_slots`: count of mount points, consuming the orphaned
  Hardpoints component later (mech/turret era; column reserved now so the
  schema does not churn).

### Unify the equip forks (the load-bearing decision)

ONE ECS component owns worn state: extend the existing, persisted Outfit
component into `Equipped { slots: HashMap<slot_id, item_id> }` covering
BOTH cosmetic tint and mechanical stats (a cosmetic-only item is simply a
row with no equipment.csv entry). The Inventory page Equip button and the
Showroom wardrobe write to the SAME component. Rules: slot must match
equipment_slots.json + the item's declared slot; equipping moves the item
out of inventory (unequip returns it); persisted in WorldSave where Outfit
already persists. Delete the page-local equipped Vec.

### Enchantments (later, cheap)

data/enchantments.csv rows are additive deltas over an equipment.csv row
(bonus_damage, bonus_armor, stat_modifiers in the same grammar). An
enchanted item is item_id + enchant_id stored on the ItemStack (needs a
per-stack metadata field; defer until the crafting-quality decision).
Do NOT build an enchanting system before base gear works.

## Part 4: Quests (repair, then grow)

1. Persist QuestTracker in WorldSave (save_load.rs currently resets it).
2. Accept/browse UI on the Quests page (quests with prerequisite None are
   currently unreachable authored content).
3. Emitters: ConstructionSystem completion pushes build_<blueprint_id>
   (rung 2 of the loop-map ladder), travel_<place> from the map/zone entry,
   talk_<npc> from the dialogue interaction.
4. Add `xp_rewards: [(skill_id, amount)]` to QuestDef (native quests
   currently reward items only, while the page header promises XP).
5. Unify the relay fork: the relay's hardcoded explore-ship chain becomes
   data (a quests RON the relay loads), and its per-player quest/XP store
   adopts the native schema (quest ids + skill XP grants), reality/fantasy
   tagged per pillar 4. One quest format, two authorities (native = home
   world, relay = shared world), zero hardcoded chains.

## Implementation ladder (each rung shippable + testable)

1. **Equipped component + equipment.csv loader** (slots enforce, persist,
   no stats yet). Test: equip jacket, restart, still worn; potato refuses
   the head slot.
2. **Stat pipeline hookup**: equip modifiers + skill `scales` fold through
   net_stat_multiplier; carry_capacity + cold_resist consumed. Test: jacket
   halves outdoor body-temp drain; exo frame raises the weight cap.
3. **EffectTick pass** (shared with loop-map rung 1): damage/healing per
   tick from status_effects.csv. Test: food_poisoning drains, regeneration
   heals.
4. **Armor on equip -> Armor component** (pre-combat; inert until
   CombatSystem registers, then free mitigation).
5. **Level-up toasts** (drain_level_ups consumer) + skills.csv `scales`
   column with craft_speed/yield live. Test: level 10 metalworking smelts
   measurably faster.
6. **AbilityRegistry + KnownAbilities + hotbar** with the four v1 verbs;
   grant scan_ore at mining 2 as the proof ability (tech flavor, useful,
   no combat dependency).
7. **Quest repair** (persistence, accept UI, xp_rewards, emitters as their
   systems land).
8. **skills.csv perk_levels** granting abilities; retire the dev-only
   feeling of skills by making levels visibly unlock things.
9. **Weapon damage columns consumed** when CombatSystem registers
   (loop-map rung 7); enchantments after that.

Rungs 1-3 need no new systems at all: they connect existing components to
an existing fold function. That is deliberate: progression should feel real
within days of work, not after a combat epic.

## Resolved open questions from engagement-modes.md

- Mode granularity: per-DOMAIN (food, water, energy, fabrication), not per
  sub-resource. Sub-resource granularity multiplies UI without adding
  choice.
- Mode transitions: changing mode never deletes state; Direct->Automation
  keeps your garden and staffs it (automation machines or NPC tenders when
  they exist); Background requires T3 credits or T4 cooperative membership
  paying the implied cost. Retaking Direct is always one click.
- Background mode cost: yes, always a visible upkeep (credits or coop
  contribution). Post-scarcity is earned infrastructure, not a toggle.
- Mods declare mode by content type automatically (a recipe extends modes
  1/3; a drone extends 3; a market good extends 2) - no mode manifest field
  needed.
- Onboarding: default everyone to Direct for food + energy (the teaching
  loop), Trade for everything else; the engagement choice surfaces the
  first time a domain's upkeep bites, not as an upfront quiz.
