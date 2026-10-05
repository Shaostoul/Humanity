# A new player's first hour, with stakes on (audit, 2026-10-04)

> Read-only code audit commissioned after the operator asked "How goes progress
> towards a basic starting gameplay loop?" (2026-10-04). Scenario: a new player in
> Play mode Normal with "Start every session from the default home" OFF, single
> player, then what changes on a shared server. Every claim carries file:line as of
> v0.1458.1; lines drift. It is the evidence base for the first-hour arc in
> docs/PRIORITIES.md. Reproduced as the auditor wrote it.

HumanityOS: a new player's first hour with stakes on. Read-only code audit, 2026-10-04.

Scope: I read code and data only. Nothing was edited, built or booted. Two parallel tracers covered quests and the shared server. A third, covering tools, drone, smelting and building, never reported back, so I traced those myself. I re-read every load-bearing line the tracers cited before using it. Paths are relative to C:\Humanity. I did not check runtime state, such as whether the live server's plots are already taken.

FOUR THINGS FRAME EVERYTHING
- **The defaults are the opposite of this scenario.**
  - Play mode defaults to Dev (src/config.rs:127-131), where nothing is used up (config.rs:166).
  - "Start every session from the default home" defaults ON (src/gui/mod.rs:4528, src/config.rs:627-630).
  - A player who never opens Settings > Gameplay has no stakes, and loses all progress every launch.
- **The first session is usually not single player.**
  - If the player typed a display name and is online, the app connects to united-humanity.us by itself (src/gui/connections.rs:293-303, src/lib.rs:14275-14307). The default URL is set at src/gui/mod.rs:3222, and "Skip (stay offline)" keeps it (src/gui/pages/main_menu.rs:317-319).
  - Entering the world then joins the shared world (src/lib.rs:6739-6792), because the "solo" flag defaults to false and is never saved (src/gui/mod.rs:3732).
  - Only choosing the home row in the Characters picker sets solo (src/lib.rs:7287-7288).
- **In true single player at the default 1x clock, nothing can hurt the player in the first hour.** Anything that could hurt them later is undone by dying, quitting or relaunching.
- **The quest line is completable in a few minutes, but nothing tells the player where to go.** After the second quest it falls silent.

1. THE JOURNEY

**1. First boot.**
- Storage chooser, then Welcome (main_menu.rs:50-55, 67-122, 147-184).
- "Skip setup (offline mode)" finishes with no identity (main_menu.rs:175-179). That is the only truly offline path, because there is no auto-connect without a key (connections.rs:302).
- Discoverable: yes.

**2. Server step.**
- "Connect" only probes the server's /health address (main_menu.rs:293-310).
- "Skip (stay offline)" just moves on (main_menu.rs:317-319).

**3. Identity.**
- Display name, then "Generate New Identity" shows the 24 words in place and opens the passphrase prompt (main_menu.rs:453-537; lib.rs:15462-15465).
- Finish is blocked without an identity (main_menu.rs:625-636).

**4. "Enter HumanityOS".**
- This puts the player in the 3D world (main_menu.rs:669-678).
- The only instructions anywhere are "Press Escape anytime to open the menu. Press Enter to toggle chat." (main_menu.rs:681-684).
- Nothing mentions I (inventory), E (use), F1 (key list) or Alt (free the mouse).

**5. Spawn.**
- The home is assembled onto plot p1 (src/engine/world_load.rs:117-131).
- The player stands at the home's authored spawn, (53.5, 40.5) (data/homes/homestead.ron:1414), inside the Entry room (homestead.ron:1566-1573), just inside the front door, facing west (world_load.rs:211-214).
- data/world/spawn.ron, which says "bedroom", is dead data: only the embed list references it (src/embedded_data.rs:79, 295, 471).
- data/player.toml is dead too (embedded_data.rs:52, 276, 459).
- From data/world/player.ron, only starting_items is read (src/save_load.rs:596-611).
- What the player has:
  - Starter kit (player.ron:21-41): 35 seeds, 4 purified water, 2 empty bottles, 3 rations, 8 hand tools.
  - Health 100 (src/ecs/components.rs:66-73).
  - Food 80, Water 80, Energy 100 (components.rs:122-139).
  - 10,000 credits (components.rs:1007-1010).
  - The Barn holds 400 wheat, 200 oats, 200 corn, 100 rice, 100 flour, 100 seed potatoes, 40 fertilizer and 60 planks (data/places/seed.json:111-127).
  - 3 chickens, 2 goats and 2 sheep (data/entities/livestock.ron:13-15).
  - The whole garden is pre-planted for free at staggered stages, some already ripe (src/engine/ipc.rs:169-294; data/world/showcase.ron:8-9).
- Discoverable: the room card says "Entry ... Here: Personal Storage / Review Tasks". That card is text only (src/gui/pages/hud.rs:438-473; data/rooms.ron:153-156).

**6. First seconds online.**
- A "Choose your privacy" window appears in the middle of the 3D view (lib.rs:15377-15391; src/gui/pages/privacy.rs:201-236). It has no close button.
- The mouse stays captured for looking around (src/engine/input.rs:65-73 does not count this window), so the player must hold Alt or press Esc to click it.

**7. HUD.**
- Shows the health bar, "10000 CR" and the quest (hud.rs:101-155).
- No survival bars at the start. The default "When low" mode shows a need only below half (hud.rs:1160-1201; config.rs:89-99). At 1x, Energy is the first to appear, after about 10.7 hours (src/systems/food.rs:181).
- No help button in the world (src/gui/pages/keymap.rs:245-258). F1 lists the keys (data/keymaps.ron:25-57), but nothing on screen says F1 exists.

**8. Quest "First Steps", step 1: "Acquire 3 iron ore (mine it with a drone, or stock it) (1/2)".**
- Accepted automatically (lib.rs:1366-1367; data/quests/getting_started.ron:26-27). It counts the backpack only (src/systems/quests/mod.rs:273-278).
- **Drone route:**
  - Press I, open Mining, click an asteroid, set up to 10 units, press "Launch drone" (src/gui/pages/inventory.rs:1150-1207).
  - Free in every mode: no item, power, fuel or hangar check, and one drone at a time (src/systems/mining.rs:79-93, 190-235, 356-357).
  - A trip takes about 16 seconds (components.rs:1437-1443). The ore lands in the backpack (mining.rs:263-303).
  - Ore is finite: 185 iron across three asteroids (lib.rs:1404-1447).
- **Vendor route:** E on the home trading post (data/machines/home.ron:3814), hold Alt, "Trade". Iron ore costs 7 credits (data/trade_goods.ron:7-9, 72).
- "Or stock it" means a Dev-only button (src/gui/pages/crafting.rs:305-309), so in Normal that half of the hint points nowhere.
- Discoverable: barely; nothing says the drone lives on the Inventory page. Completable: yes, in about a minute. Persists: yes.

**9. Step 2: "Smelt an iron ingot at a smelter (with coal or graphite)".**
- It counts any craft that makes an ingot (src/systems/crafting/mod.rs:791-803).
- By hand: Crafting page, "Smelt Iron (graphite)", which takes 2 ore + 1 graphite (graphite comes from asteroid C-3) (data/recipes.csv:643).
  - It is free-tier (crafting/mod.rs:805-831), and hand crafts aboard also draw from the Barn (src/engine/built_uses.rs:72-81).
- The home smelter also smelts by itself whenever 2 ore and 1 coal are anywhere (home.ron:2088; crafting/mod.rs:1083-1214). Coal costs 4 credits (trade_goods.ron:67).
- Reward: 2 ingots and 30 XP (getting_started.ron:34-39). Completion is only written to the log; the player sees nothing (quests/mod.rs:431).

**10. "Toolsmith" ("Forge a hammer").**
- It completes itself. The home's three workbenches build hammers whenever fewer than 2 exist (home.ron:2568-2569; instances at 3305, 3313, 3927).
- They take ingots from the backpack first (crafting/mod.rs:1186-1214). So the player's 3 ingots, quest reward included, become hammers, and the quest finishes with no notice.
- After that the HUD quest line is blank. "Build First Habitat" must be found and accepted on the Quests page (src/gui/pages/quests.rs:104-140; lib.rs:12599-12619).

**11. Eating and drinking.**
- Press I, choose a backpack item, press Eat or Drink (inventory.rs:936-985; food.rs:354-456).
- Values:
  - A ration gives +30 food.
  - Water gives +30 water.
  - A raw vegetable gives +3.75 food, with a 2% chance of food poisoning (data/food_system.ron:106-117, 542-555, 931-944).
  - Barn grain has a 5% chance (food_system.ron:156-165).
- Barn food must be moved into the backpack before it can be eaten (inventory.rs:936-941).
- At 1x, water falls about 2 points an hour and food about 0.6 (food.rs:144-147), so eating is never needed in the first hour.

**12. Garden.**
- The showcase crops are ripe on arrival. "Harvest N ready" (inventory.rs:2191-2198) gives produce plus 2 seeds each (src/systems/farming/mod.rs:1779-1799).
- Their own seeds take real days: lettuce 45, bean 60, tomato 70, carrot 75, potato 90, wheat 120 (data/plants.csv:138-145). The default clock is real time (src/systems/time.rs:191-195). At the "Simplified" 72x preset (time.rs:196-207), lettuce takes 15 hours.
- The backpack "Plant" button makes a crop that belongs to no bed:
  - It is never drawn in the world (src/engine/home_meshes.rs:973).
  - It is never irrigated, so it dies about 10 game hours later unless hand-watered (farming/mod.rs:1209-1245, 489-498, 2191-2199).

**13. Building.**
- Crafting page, Structures, Build, then a ghost of the piece appears; E builds it.
- Materials are real: backpack first, then the Barn (src/systems/construction/mod.rs:435-508).
- The crosshair hints are good (src/engine/build_place.rs:220-280).
- Separately, B opens the construction editor. It places any home machine for free in every mode (src/engine/editor.rs:504-544; config.rs:164) and writes the result into the data files, not the save (editor.rs:265-301).

**14. Sleeping.**
- Only a BUILT bed works (src/systems/sleep.rs:19-21; built_uses.rs:238). It costs 6 planks + 4 fiber (data/blueprints/basic.ron:46).
- The bedroom's furnished bed card says "sleep here" (home.ron:1388-1400, 2764), but E only opens the card.
- Fiber comes from logs (recipes.csv:118). The home sawmill turns any 2 logs in the backpack into planks first (home.ron:2039).
- "Short rest" is in the Inventory status section (inventory.rs:1538; sleep.rs:97-117).
- Not needed in the first hour at 1x.

**15. Saving.**
- The game saves every 120 seconds (lib.rs:6846-6855; save_load.rs:845-864) and when the window is closed (lib.rs:2134-2148).
- The hub's Quit button and the updater's restart exit without saving (main_menu.rs:721-723; settings.rs:4602; lib.rs:15663-15665).

**16. Relaunch.**
- The player lands on the Humanity page, not in the world (lib.rs:1757-1764; gui/mod.rs:3230).
- "Play" with no saved pairing opens the character picker (src/gui/pages/escape_menu.rs:612-630). Esc goes straight into the world as shared (lib.rs:2520-2526).
- Restored when progress is kept (lib.rs:1683-1712; save_load.rs:358-591): backpack, skills, credits, quests, crops (aged by the time away; save_load.rs:943-985), builds, storage, machine levels, asteroids, drone, animal timers, clock.
- Lost on every relaunch:
  - health, all vitals, status effects and position (save_load.rs:15-18, 355-357; persistence.rs:19-21 fields are never written);
  - spoilage timers (food.rs:279-280);
  - the waste meter and urine tank (food.rs:221-223, 283-286).
- The player always returns at full health, Food 80, Water 80, at the front door.

2. RANKED LISTS

**BLOCKERS**

**B1. The default "fresh home" setting wipes progress on every launch.**
- Evidence: gui/mod.rs:4528; lib.rs:1683-1689; save_load.rs:697-705. With it on, quests restart too (save_load.rs:646-663).
- Smallest fix: default it off, and keep it on only in the operator's config.

**B2. Unasked auto-join to the live shared world.**
- Evidence: connections.rs:293-303; lib.rs:6739-6792; gui/mod.rs:3732.
- Consequences:
  - The host clock runs at 72x (src/relay/storage/mod.rs:1107; time.rs:546-564). Water empties about 32 real minutes after the starting 80 and you are fatigued after about 13 (food.rs:144-147, 181), from my arithmetic.
  - Sleeping is refused there (sleep.rs:163-170).
  - The ship has only two plots (data/blueprints/ship_structure.ron:996-1026). Nothing frees a plot (src/relay/storage/plots.rs:88-95), so once two identities have ever joined, every newcomer is a guest whose home is put away and who cannot build (src/engine/home_plot.rs:155).
- Smallest fix: onboarding's Enter sets solo, or records a "home:" pairing, unless the player picked a server; save the solo intent.

**B3. Quit from the hub or the updater skips the save.**
- Evidence: lib.rs:15663-15665.
- Smallest fix: call save_active_home before exiting there.

**B4. Solo home soft-lock.**
- Evidence: Settings > Home Design = Solo (settings.rs:3683-3692) loads data/machines/home_solo.ron, which has no smelter, workbench or trading post. The only buildable smelter needs 3 iron ingots (basic.ron:19). First Steps step 2 is then impossible.
- Smallest fix: add a smelter to home_solo.ron.

**B5. Accepted quests that can never finish.**
- "ore_sample_0 ×5" and "rare_ore_0 ×3" (data/quests/exploration.ron:27, 67) come only from creatures that are never spawned (data/creatures.csv:92, 124; data/entities/wild_spawns.ron:15-34).
- Travel steps read the player's position from a record that walking never moves (quests/mod.rs:327-341; src/systems/player.rs:35-38, where "physics_world" is never inserted).
- Smallest fix: copy the camera position into the player's position every frame; give ore_sample_0 a source.

**B6. Dev-only: Dev travel then switching to Normal strands the player.**
- Evidence: "Return home" exists only on the Dev page (src/gui/pages/dev.rs:330), which Normal hides (dev.rs:74), and is the only reset (lib.rs:5881). Relaunching fixes it.

**FRICTION**

**F1. Nothing teaches the game.**
- Evidence: onboarding mentions only Esc and Enter (main_menu.rs:681-684); no help button in the world (keymap.rs:245-258); data/help/topics.json has no gameplay topic; no tutorial.
- Smallest fix: a first-entry notice naming I, E, F1 and Alt.

**F2. Clickable panels need Alt held, and nothing on screen says so.**
- Machine-card buttons (Fill, Take, Trade), the vendor window and the privacy window all need Alt held, because the captured-mouse rule ignores them (engine/input.rs:65-73; hud.rs:1470-1598). Only F1 mentions Alt (keymaps.ron:46).
- Smallest fix: add the vendor window, the privacy prompt and a pinned card to that rule.

**F3. Quest text and feedback.**
- The step text sends players to a Dev-only button and never names a place (getting_started.ron:27).
- The HUD shows no counts (hud.rs:144-155). Completions are silent (quests/mod.rs:431). The HUD goes blank after Toolsmith.
- Smallest fix: say "Inventory > Mining, or the trading post"; post a notice on completion; accept "Build First Habitat" automatically.

**F4. Automated machines take from the backpack before the Barn.**
- Evidence: crafting/mod.rs:1186-1214.
- The workbenches eat the quest's reward ingots; the sawmill eats logs bought for fiber; the smelter takes ore in pairs once coal is around.
- Smallest fix: automated machines draw from home storage only.

**F5. The bedroom bed cannot be slept in, though its card says "sleep here".**
- Evidence: home.ron:1388-1400 versus built_uses.rs:238.
- Smallest fix: route E on the home's bed machine to sleep.

**F6. Relaunch flow.**
- Evidence: Humanity page first, then a picker (lib.rs:1757-1764); Esc bypasses the picker and goes shared (lib.rs:2520-2526).

**F7. Nothing a new player plants ripens in a session.**
- Evidence: real-time clock (time.rs:191-195). The backpack "Plant" crop is invisible and goes unwatered (home_meshes.rs:973; farming/mod.rs:2191-2199).

**F8. The only first-hour indoor death is food poisoning, and its cure is undocumented.**
- Evidence: 2% chance per raw vegetable; 0.2 HP/s for 90 minutes (data/status_effects.csv:76; food_system.ron:117).
- Health never regenerates except through "well fed", +1 HP/s for 30 minutes after a meal that leaves food at 70 or more (status_effects.csv:33; food.rs:438-444, 786-826).
- So a poisoned player dies about 38 minutes after eating unless they eat again. Nothing says so, and there is no medicine.

**F9. Misleading names.**
- Two different "First Steps": the HUD quest (getting_started.ron:21-22) and the Tasks page checklist (data/onboarding/quests.json:6-7).
- The picker hint "Gear and skills stay in the world you earn them in." is false; solo and server share one save (src/gui/pages/showroom.rs:262-267; save_load.rs:89-93).

**F10. Server confusions.**
- No arrival message.
- Relay quests are ignored by the desktop app (src/engine/net_route.rs:6-10, 224).
- The fleet store is invisible in the world.
- A game ban only shows on an admin status line, and the HUD keeps saying "Shared world" (net_route.rs:208-215).

**MISSING STAKES**

**S1. Quitting heals and refills.**
- Evidence: vitals, health and effects are never saved (save_load.rs:15-18, 355-357; lib.rs:1375-1389).
- Smallest fix: add them to the save.

**S2. Death costs nothing.**
- Respawn gives full health and refills needs to 60 (lib.rs:12381-12436). The death screen says "Nothing was lost" (hud.rs:51-53).

**S3. The B editor is free in Normal and persists outside the save.**
- Evidence: editor.rs:504-544, 265-301.
- Smallest fix: in Normal, consume the machine's item when it is placed.

**S4. The free garden refills itself.**
- The showcase replants the whole garden for free whenever it is empty, in every play mode (ipc.rs:177-196; src/hot_reload/mod.rs:92-95).
- Smallest fix: gate it to Dev, or to a brand-new world only.

**S5. Free resources.**
- The drone has no cost (mining.rs:356-357).
- 10,000 credits against 7-credit ore (trade_goods.ron:72).
- A year's grain sits in the Barn (seed.json:115).

**S6. Stored food never spoils.**
- Only an inventory on an entity is aged (food.rs:847-919), and only the player has one (lib.rs:1371).
- Timers are not saved and reset when a stack moves to another slot (food.rs:246-255, 279-280).

**S7. Animals have no needs, and a hen lays an egg every 5 real minutes at 1x.**
- Evidence: src/systems/livestock.rs:467-487; creatures.csv:29.

**S8. No treatment system.**
- data/medical.ron is never applied: MedicalSystem is never registered (src/systems/medical.rs:38-104 against the registration list at lib.rs:1062-1363), and apply_condition has no callers.

KNOWN GAPS: CONFIRM OR REFUTE

1. **Health and vitals never saved: CONFIRMED** (S1).
2. **medical.ron never applied: CONFIRMED** (S8).
3. **Stored food never spoils, and the spoilage clock is not saved: CONFIRMED.** Moving a stack to another slot also resets it (S6).
4. **Farm animals have no needs: CONFIRMED** (S7).
5. **Home marker never shows from the ground: CONFIRMED, and broader: it never shows anywhere at default settings.**
   - The marker is only added beyond 1 km (lib.rs:3482-3490).
   - The HUD drops anything past the camera's far plane (hud.rs:1113-1121).
   - The far plane is the render distance: 500 m by default, 2000 m maximum (lib.rs:16390; settings.rs:2749).
6. **Dev travel leaves fly mode on while the HUD reads WALK: CONFIRMED, Dev mode only.**
   - Travel sets fly mode on (lib.rs:5965-5966, 6082-6083), but the HUD reads the separate hover flag (hud.rs:374-387).
   - In Normal, fly mode is forced off every frame (lib.rs:3560-3571).
7. **The sawmill takes logs from the backpack: CONFIRMED** (home.ron:2039; crafting/mod.rs:1186-1214). The same rule applies to every automated machine.

IF THEY JOIN A SHARED SERVER INSTEAD (server tracer, key lines spot-checked)
- **Fresh only on the server's side.** The relay starts each join empty: health 100, nothing carried (src/relay/handlers/game_state.rs:729-763).
- **One local save for both.** The game keeps one save and plays the server session with the same backpack and home (save_load.rs:89-93). Server results flow back into that save.
- **Plots.**
  - A holder spawns at their own door.
  - Plot 1's door opens onto the Commons.
  - Guests spawn in the Commons.
- **Crew** live on the relay and exist only there.
- **Fleet supply is unlimited by default.** It still gives one meal per 8 game hours (data/food/ship_stores.ron:68). The per-player ledger records meals and power used against items and power given.
- **No server tutorial.** Nothing exists for a newcomer on a server (the tracer searched data/onboarding, data/help and data/quests).

3. THE FIRST TEN MINUTES, PLAINLY
You wake inside the front door of a large orbiting homestead that is already built and stocked. You carry seeds, water, three ration packs, hand tools and 10,000 credits, and the barn and garden hold more food than you could eat in a year. The screen gives you one job, "Acquire 3 iron ore", but never says where. If you press I and find Mining, a free drone fetches it in about 15 seconds, then one click smelts it, and the next quest finishes on its own while the home's workbenches quietly turn your new iron into hammers. Hunger, thirst and tiredness are real but move at real-world speed, so nothing appears on screen for about ten hours, and nothing you plant will ripen for at least 45 days. Indoors nothing can hurt you except a small chance of food poisoning from raw vegetables, and dying, quitting or relaunching all return you to full health with nothing lost. If you typed a name and are online, those ten minutes actually happen in the shared world at 72x speed: you get thirsty in about 20 minutes and cannot sleep there.
