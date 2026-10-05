# A new player's first hour, with stakes on (audit, 2026-10-04)

> Read-only code audit commissioned after the operator asked "How goes progress
> towards a basic starting gameplay loop?" (2026-10-04). Scenario: a new player in
> Play mode Normal with "Start every session from the default home" OFF, single
> player, then what changes on a shared server. Every claim carries file:line as of
> v0.1458.1; lines drift. This is the auditor's final report, after three read-only
> sub-audits (quests; tools, drone, smelting and building; the shared server), and it
> replaces a shorter first version. It is the evidence base for the first-hour arc
> in docs/PRIORITIES.md. Reproduced as the auditor wrote it.

New-player first-hour audit of HumanityOS, play mode Normal, progress kept between launches

I only read code and data. Nothing was edited, built or booted. Paths are under C:\Humanity\ (so src\lib.rs means C:\Humanity\src\lib.rs).

I traced the core path myself. Three read-only sub-audits ran alongside: one on quests, one on tools, drone, smelting and building, and one on the shared server. I re-read the code behind every claim of theirs used below and corrected one of them: the stuck drone in Friction 5 does have an escape.

## Headlines
- **The first session is not single player by default.** It silently joins the public shared world (72x clock), and may arrive as a guest with no home.
- **First Steps is done in about five minutes with the stated settings, but almost nothing in hour one is at risk.** A ripe garden appears for free, the B editor places any machine for free, the drone is free, and both death and relaunch refill you.
- **The defaults a real downloader gets remove progress and stakes entirely.** Play mode is Dev, and "start every session from the default home" is on.
- All seven known gaps are confirmed. The home marker is broader than reported: it never draws anywhere, not just from the ground.

## 1. The journey, step by step

1. **Storage chooser.** Code: main_menu.rs:50-55, 67-122. Discoverable: yes. Persists: the choice.

2. **Welcome.** Code: main_menu.rs:147-184.
   - "Skip setup (offline mode)" completes onboarding with no identity at all (:175-179). Without an identity the app never connects (connections.rs:302).

3. **Server step.** "Skip (stay offline)" only moves to the next page (main_menu.rs:317-319). The server address stays https://united-humanity.us (gui\mod.rs:3222).

4. **Identity.** Display name, Generate New Identity, the 24 words, then a passphrase.
   - Code: main_menu.rs:432-536; passphrase modal at lib.rs:15462-15465. You cannot finish without an identity (:625-636).
   - Discoverable: yes; the guidance is good and in place.

5. **"Enter HumanityOS"** takes you to the 3D world, or to Chat if the server probe succeeded (main_menu.rs:669-678).
   - The only tips: "Press Escape anytime to open the menu. Press Enter to toggle chat." (:681-684). Nothing about I, E, F1 or Alt.

6. **Spawn.** You stand just inside the front door, in the Entry room, facing west.
   - Code: homestead.ron:52 (door), :1414 (spawn), :1566-1573 (Entry); world_load.rs:211-214.
   - data\world\spawn.ron says "bedroom" (:6-7), but nothing reads it; it is only embedded (embedded_data.rs:79, 295, 471).
   - Starting state:
     - Health 100/100 (components.rs:66-73).
     - Food 80, Water 80, Energy 100 (components.rs:122-139).
     - 10,000 CR (components.rs:1007-1011).
     - Kit of 35 seeds, 4 waters, 2 empty bottles, 3 rations and 8 hand tools (player.ron:21-41). It fills 17.5 L of the 65 L pack (inventory\mod.rs:181-184).
     - The Barn holds 400 wheat, 200 oats, 200 corn, 100 rice, 100 flour, 60 planks, 40 fertilizer and 100 seed potatoes (seed.json:111-128).
     - The Garage holds 58 climbing-kit items that are labels only and do nothing (loaders.rs:335-353).
   - Two files are never read:
     - data\player.toml (hunger 50, thirst 50, :15-20).
     - player.ron's health and speed fields. Only `starting_items` is read (save_load.rs:596-611).

7. **A few seconds later, if a name was typed and you are online:**
   - The app connects to united-humanity.us on its own (connections.rs:293-303; lib.rs:14275-14307).
   - `copresence_solo` defaults to false and is never saved (gui\mod.rs:3732), so the join gate puts you in the shared world (lib.rs:6739-6792; home_plot.rs:229-240).
   - The host's clock takes over (time.rs:539-564) at 72x by default (relay\storage\mod.rs:1107). If the server refuses the join because your ship file differs from its own, a "Not in the shared world" note stays on the HUD instead (hud.rs:184-191).
   - A "Choose your privacy" window opens mid-screen with no close button (privacy.rs:201-236; lib.rs:15383-15391). The mouse stays captured because `cursor_want_free` has no term for it (engine\input.rs:65-73). Only Esc or holding Alt makes it clickable.
   - Real single player needs one of: picking "My Homestead" in the Characters picker (lib.rs:7287-7288), an empty display name, or no network.

8. **HUD.**
   - Health bar and "10000 CR" (hud.rs:101-127).
   - No survival bars, because the default is "When low" (config.rs:89-99; hud.rs:1160-1201). At 1x the first bar, Energy, appears after about 10.7 h; Water after about 14.4 h (food.rs:144-147, 181).
   - Quest line "First Steps / Acquire 3 iron ore (mine it with a drone, or stock it) (1/2)" (hud.rs:144-155; getting_started.ron:26-27).
   - Room card "Entry ... Here: Personal Storage / Review Tasks". It is text only (hud.rs:438-473; rooms.ron:153-156).
   - The public chat feed is on by default (gui\mod.rs:3148).
   - There is no help button in the world (keymap.rs:249-258). F1 lists the keys (keymaps.ron:25-57), but nothing says to press it.

9. **Using things.** E opens a machine's card (hud.rs:623-639).
   - The card's buttons (Fill bottle, Take, Store, Trade) can only be clicked while holding Alt (hud.rs:1470-1598; engine\input.rs:65-73).
   - Alt is mentioned only in the F1 list (keymaps.ron:46).

10. **Quest step 1: get 3 iron ore.**
    - Drone: Inventory (I) > Mining > asteroid card > Launch drone (inventory.rs:2587-2626, 1121-1225).
      - It is free (no item, power or fuel), one at a time, about 16 s per trip, and ore lands in the backpack (mining.rs:76-94, 190-235, 263-303, 356-357; components.rs:1437-1443).
    - Vendor: the trading post in the vehicle bay (home.ron:3812-3816) > hold Alt > Trade. Iron ore is 7 CR (trade_goods.ron:72).
    - The step counts only what is in the backpack (quests\mod.rs:273-278).
    - Discoverable: partly. "Or stock it" means the Dev-only stock button (crafting.rs:305-309), and nothing says where the drone is.
    - Completable: yes. Persists: ore, asteroids as mined, a drone in flight (save_load.rs:132-141, 267-278).

11. **Quest step 2: smelt an iron ingot.**
    - The smelter auto-runs the coal recipe (home.ron:2067-2091; recipes.csv:20).
    - No coal or graphite at start (player.ron:21-41; seed.json:111-128), so it just waits.
    - Coal: 4 CR from the vendor (trade_goods.ron:67).
    - Graphite: only asteroid C-3 (lib.rs:1438-1447), then hand-craft "Smelt Iron (graphite)" (recipes.csv:643).
    - Any ingot counts, including one made automatically (crafting\mod.rs:791-803; quests\mod.rs:361-374).
    - Reward: 2 ingots and 30 XP (getting_started.ron:34-39). There is no completion message; it only goes to the log (quests\mod.rs:425-432).
    - Completable: yes in the Family home, never in the Solo home (Blocker 3).

12. **"Toolsmith: Forge a hammer" completes itself.**
    - Three workbenches auto-craft hammers until 2 are on hand (home.ron:2552-2572; instances at 3304-3316 and 3926-3927).
    - They take ingots from the backpack first and planks from the Barn (crafting\mod.rs:1083-1233). The kit holds 1 hammer.
    - So the next ingots, up to three and including the reward, become hammers, and the automatic craft fires the quest event (crafting\mod.rs:795).
    - The chain then ends. Other quests need Accept on the Quests page (lib.rs:12599-12632; quests.rs:104-140).

13. **Eating and drinking.** Inventory > click the item > Eat or Drink (inventory.rs:936-985).
    - Only backpack items get the buttons (:937-941), so Barn food must be moved into the backpack first.
    - A ration gives +30 food and a water +30 water (food.rs:155-160, 374-423).
    - Not needed in hour one at 1x: water drops about 2 points an hour (food.rs:147).
    - Food poisoning risk:
      - Each raw bite has a 2-5% chance (food_system.ron:117, 165; food.rs:431-437).
      - It drains 0.2 HP/s for 90 minutes, and there is no cure (status_effects.csv:76).
      - The only healing in the game is "well fed": 1 HP/s for 30 minutes after a filling meal (status_effects.csv:33; food.rs:441-444, 792-819).
      - So one unlucky bite kills you about 38 minutes later unless you eat again.

14. **Garden.**
    - On entry the whole garden is planted for you, free, at staggered stages including ripe (ipc.rs:169-294; called every frame from hot_reload\mod.rs:92-95; showcase.ron:9).
    - "Harvest N ready" gives produce plus 2 seeds per plant (inventory.rs:2191-2197; farming\mod.rs:1779-1799).
    - The backpack's Plant button makes a "loose" crop with no bed (farming\mod.rs:1209-1252). It is never drawn (home_meshes.rs:973), gets no automatic watering, and dies about 10 game hours after its last hand-watering (farming\mod.rs:489-498, 2191-2199).
    - At the default 1x speed (time.rs:191-195), lettuce takes 45 real days (plants.csv:142); potato 90 and wheat 120 (plants.csv:140-141).
    - Persists: crops, aged by the time you were away (save_load.rs:943-985).

15. **Animals.** 3 chickens, 2 goats and 2 sheep (entities\livestock.ron:13-15).
    - Press E for an egg every 300 game seconds, milk every 400, wool every 600 (creatures.csv:29, 32-33).
    - They need nothing (livestock.rs:467-487).

16. **Building.**
    - Crafting > Structures > Build, then E. Materials come from the backpack, then the Barn (construction\mod.rs:435-508).
    - With too few materials, E silently does nothing in the world. The reason shows only on the Crafting page, in green, with raw ids (build_place.rs:149-153; construction\mod.rs:477-489; crafting.rs:339-345).
    - The B editor places any machine (smelter, trading post, solar) for free in Normal (editor.rs:504-544; config.rs:164).
    - Persists: blueprint builds go in the save (save_load.rs:213-261). Editor edits are written into the data files themselves (editor.rs:265-302), so they outlive every save.

17. **Sleep.**
    - Only a bed you built works (built_uses.rs:182-210, 238). The home's own "Bed" is a machine card you cannot lie in (home.ron:1388-1400, 2764).
    - A bed costs 6 planks and 4 fiber (basic.ron:46). Fiber comes from a log or from flax (recipes.csv:118, 648), and neither is in the kit or the Barn.
    - "Short rest" is a 10-minute nap (inventory.rs:1538; sleep.rs:97-117).
    - Not needed in hour one: fatigue starts after about 16 h (food.rs:179-183).

18. **What can hurt you in hour one** (single player, 1x):
    - No hunger, thirst, cold or air danger indoors (survival_env.rs:183-209). Hostile wildlife is off by default (gui\mod.rs:4565).
    - Possible deaths: the raw-bite poisoning above, or about 52 s of vacuum outside the sealed rooms (food.rs:186-191; survival_env.rs:210-249). I did not confirm a walkable way out of the sealed rooms.
    - Death costs nothing (lib.rs:12381-12436).

19. **Saving.**
    - Every 120 s (lib.rs:6849-6855) and when the window closes (lib.rs:2134-2148).
    - The hub's Quit button and the updater restart exit without saving (main_menu.rs:721-723; settings.rs:4602; lib.rs:15663-15665).

20. **Relaunch.**
    - You land on the Humanity page, not in the world (lib.rs:1757-1764).
    - "Play" opens the Characters picker the first time, because onboarding recorded no last choice (gui\mod.rs:2943-2966).
    - Esc drops you straight back into the shared world (lib.rs:2520-2527).
    - Kept: backpack, Barn, skills, wallet, quests, crops, builds, asteroids, drone, machine levels, crafts in progress, clock (save_load.rs:115-294, 358-591).
    - Reset:
      - health (to 100), food and water (to 80), energy, oxygen, body temperature and waste;
      - all status effects, including poisoning;
      - spoilage timers and the urine tank;
      - death state and position;
      - any smelter recipe switch.
    - Sources for the resets: lib.rs:1375-1389; persistence.rs:21 (written nowhere); food.rs:221-223, 279-286; home_spawn.rs:275-280; machine_levels.rs:43-56.

## Known gaps: all confirmed
- **Health and vitals are never saved.** The save's player_health field is never written or read (persistence.rs:21; save_load.rs:15-18, 355-357). Each launch spawns fresh defaults (lib.rs:1379-1382).
- **data\medical.ron is never applied.** MedicalSystem is never registered (lib.rs:1062-1363). It keeps the file as raw RON values nothing reads (medical.rs:24-52), and `apply_condition` is never called.
- **Stored food never spoils, and the spoilage clock is not saved.**
  - Spoilage scans only ECS inventories (food.rs:857-918), and only the player has one (lib.rs:1371).
  - Home storage is a GUI list (save_load.rs:709).
  - The timer is kept in memory per slot (food.rs:246-255, 279-280). Moving a stack to another slot, stashing it and taking it back, or relaunching all reset it.
- **Farm animals have no needs** (livestock.rs:467-487).
- **The home marker never shows from the ground, nor anywhere else.**
  - It is added only beyond 1 km (lib.rs:3482-3490).
  - The HUD drops any point beyond the camera's far plane (hud.rs:1113-1121; reverse-Z camera, camera.rs:493).
  - The far plane is Render Distance: 500 m by default, 2,000 m at most (lib.rs:16390; gui\mod.rs:4496; settings.rs:2749).
- **Dev travel leaves fly mode on while the HUD reads WALK** (Dev mode only).
  - Travel sets the fly flags but not `dev_hover` (lib.rs:5965-5966, 6082-6083). The HUD reads `dev_hover` (hud.rs:374-387).
  - Fly mode suspends vacuum and cold harm (survival_env.rs:178-181).
  - Normal mode forces fly off (lib.rs:3560-3561). But if you travel in Dev (the default mode) and then switch to Normal, you are stranded away from home: Return home is only on the Dev page (dev.rs:330; lib.rs:5881).
- **The sawmill takes logs from the backpack**, though the Barn is also used.
  - It takes from the backpack first, then the Barn (crafting\mod.rs:1186-1213).
  - It has no stock limit and no "player is home" check (home.ron:2023-2042; crafting\mod.rs:1037-1233).
  - So any 2 logs you carry become planks, sawdust, slabs and bark within 5 s (recipes.csv:68).

## 2. Ranked lists

### BLOCKERS
1. **The first session silently joins the shared world** (step 7).
   - The ship has exactly two home plots. They are first come, first served, and never freed automatically (ship_structure.ron:996-1026; relay\storage\plots.rs:89-95).
   - From the third player who ever joins onward, you are a guest:
     - no building and no B editor (home_plot.rs:155; editor.rs:1878-1884);
     - no home stations or home storage (built_uses.rs:30-37);
     - the Commons has no smelter or water tank (machines\ship.ron).
   - So step 2 is impossible there, on a 72x clock. Becoming a guest shows no message (home_plot.rs:478-486, 747).
   - Whether the live server's two plots are already taken is runtime state I cannot see.
   - **Fix:** make onboarding's Enter (and Skip offline) set `copresence_solo = true` and record a "home" pairing (`record_pairing`, gui\mod.rs:2924-2937). Also save `copresence_solo` in AppConfig.
2. **The shipped defaults erase progress and stakes.**
   - "Start every session from the default home" is on (gui\mod.rs:4528; config.rs:627-630). Each relaunch keeps only name and look (lib.rs:1683-1689; save_load.rs:613-638, 697-705).
   - Play mode defaults to Dev, which turns free resources on (config.rs:127-131, 161-172).
   - **Fix:** flip both defaults, or ask on first run.
3. **The Solo home design can never finish First Steps.**
   - data\machines\home_solo.ron has no smelter, workbench, trading post or ship machines, and its catalog has no smelter to place in the B editor.
   - The only buildable smelter, the Furnace, costs 3 iron ingots (basic.ron:19).
   - **Fix:** add a smelter and a trading post to home_solo.ron, or make the Furnace stone-only.
4. **Past hour one, two quest chains are dead.**
   - `exploration_first_survey` needs 5 `ore_sample_0`, which only drops from a creature nothing spawns (exploration.ron:27; creatures.csv:124; wild_spawns.ron:15-34). That also locks its child quest.
   - Travel steps read the player's position from a component that walking never updates (quests\mod.rs:323-341; player.rs:35-38; "physics_world" is never inserted).
   - **Fix:** give `ore_sample_0` a source, and copy the camera position into the player's position each frame.
5. **Two quit paths skip the save**, so up to 2 minutes are lost (main_menu.rs:721-723; settings.rs:4602; lib.rs:15663-15665).
   - **Fix:** run the same save as window close (lib.rs:2134-2148) before exiting.

### FRICTION
1. **No in-game guidance.**
   - The only tips are Esc and Enter (main_menu.rs:681-684), and there is no help button in the world.
   - The Quests page repeats the step text without saying where to go (quests.rs:78-102).
   - Quest completion is silent (quests\mod.rs:425-432).
   - **Fix:** name the place in each step ("Inventory > Mining"), and show a toast when a step or quest completes.
2. **Alt is needed for every in-world button**: Fill, Take, Trade, the vendor and the privacy window (engine\input.rs:65-73; hud.rs:1470-1598).
   - **Fix:** add `vendor_open` and `privacy_tier_prompt_open` to `cursor_want_free`, and print "hold Alt to use" under the card.
3. **The quest hint points at a Dev-only button** (getting_started.ron:26; crafting.rs:305-309).
   - **Fix:** reword it to mention Inventory > Mining and the trading post.
4. **Smelting fuel is never explained.**
   - There is no coal or graphite at start, and the smelter just waits.
   - Switching the smelter to graphite is lost on every world load (home_spawn.rs:275-280; machine_levels.rs:43-56).
   - **Fix:** put a few coal in the Barn, and save each machine's recipe choice.
5. **"Keep mining" traps the drone.**
   - Once the chosen ore runs out, the asteroid survives (it is removed only when every ore is gone, mining.rs:306-317), and empty trips relaunch forever (mining.rs:96-125, 219-231).
   - The asteroid cards ignore clicks while a drone is out (inventory.rs:2612). The checkbox (inventory.rs:1181-1186) is reachable only via Maps > the asteroid marker (cosmos.rs:1618-1620; inventory.rs:1297-1298).
   - The order is also saved (save_load.rs:721, 1060).
   - **Fix:** end the order when a trip comes back empty, and put a Stop button on the drone row.
6. **The workbenches eat your first ingots, the quest reward included** (home.ron:2568-2569; crafting\mod.rs:1083-1233).
   - **Fix:** automatic machines draw only from the Barn, not the backpack.
7. **Two logs from a fallen log are refused as "Your pack is full" with 47.5 L free**, because each log is 26.1 L (lib.rs:11820-11858; items.csv:210). Logs you do carry get sawn. Fiber for a bed is therefore a puzzle.
   - **Fix:** accept part of a forage yield, and give the sawmill a stock limit.
8. **Building with too few materials gives no feedback in the world** (build_place.rs:149-153, 385-387).
   - **Fix:** send the refusal through `set_placing_note`, with item names.
9. **The home's bed cannot be slept in** (built_uses.rs:182-210).
   - **Fix:** make it a sleep target.
10. **Relaunch lands on the Humanity page**; Play opens a picker the first time; Esc puts you in the shared world (lib.rs:1757-1764, 2520-2527).
11. **Planting your own seed makes an invisible, unwatered crop**, and the fastest takes 45 real days.
12. **Contradictory dead data:** spawn.ron, player.toml, the label-only climbing kit, and a second, unrelated "First Steps" in data\onboarding\quests.json:7.

### MISSING STAKES
1. **The B editor places any machine free in Normal and saves it into the data files** (editor.rs:504-544, 265-302).
   - **Fix:** require and consume the machine's item outside Creative and Dev.
2. **A free garden that refills itself** in every play mode (ipc.rs:169-294).
   - **Fix:** gate it on free resources, or run it once for a new save only.
3. **Quitting heals:** health, vitals and effects are not saved.
   - **Fix:** add them to the save.
4. **Death is free:** full health, food, water and energy raised to at least 60, effects cleared, nothing dropped (lib.rs:12381-12436).
   - **Fix:** any cost.
5. **The drone is free and unlimited** (mining.rs:356-357).
6. **Animals need nothing and lay an egg every 5 minutes.**
7. **Stored food never spoils, and the carried-food clock resets.**
8. **No medicine and no natural regeneration** (medical.rs; status_effects.csv:76).
9. **At 1x nothing needs attention for over 10 hours.** The Barn holds years of grain, and 10,000 CR buys anything.

### On a shared server instead
- You get a plot or become a guest (above).
- Your backpack, skills and quests are your local save's. The relay holds a fresh empty entity (relay\handlers\game_state.rs:729-763). What you do there is written into your offline save.
- **72x clock, with real stakes:**
  - the Water bar appears at about 12 minutes, the empty point at 32, and death at about 52 without drinking;
  - you become fatigued at about 13 minutes;
  - sleep is refused in a shared world (sleep.rs:164-170).
  - Death is still free.
- **Fleet supply is unlimited by default** (ship_stores.rs:40, 174-179). You get one meal per 8 game hours, about 6.7 real minutes, at the mess-hall store (food\ship_stores.ron:58-68). The store has no object in the world.
- **The desktop app ignores relay quests** (net_route.rs:6-10, 224).
- **The picker's hint is wrong.** It says "Gear and skills stay in the world you earn them in" (showroom.rs:264-265), which the code does not do.

## 3. The first ten minutes, plainly
You pick a name, write down 24 words, press Enter, and wake up just inside the front door of a huge, fully built home on a space station, with full health, 10,000 credits, seeds, hand tools, three rations and four bottles of water. The only instruction on screen is a quest line, "First Steps: Acquire 3 iron ore (mine it with a drone, or stock it)"; nothing tells you that I opens your inventory, E uses things, or that you must hold Alt to click the boxes that pop up. If you typed a name and are online, the game quietly puts you on the public shared server, where time runs 72 times faster, and shows a privacy window you cannot click until you press Esc. If you find the Mining tab, a free drone brings ore in about 16 seconds, and with graphite from asteroid C-3 or coal from the trading post you smelt an ingot and finish the quest; the workbenches then quietly turn your new ingots into hammers, which finishes the next quest by itself, and after that nothing tells you what to do. Playing alone at normal speed, nothing gets hungry, thirsty or tired all hour, the garden is already full of ripe food you never planted, the chickens lay an egg every five minutes without being fed, and pressing B lets you place any machine for free. Your own seeds would take 45 real days to grow, and if you die, or simply quit and start again, you come back at full health with nothing lost.
