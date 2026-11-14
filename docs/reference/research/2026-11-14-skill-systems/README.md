# Skill systems research (2026-11-14)

The evidence behind [docs/design/skills.md](../../../design/skills.md), the proposed base skill set.
The operator asked, 2026-11-14: "We need to figure out what skills we should bother including. Then
we need to make sure we understand how they are leveled and what leveling them does for us." So this
compares how real-world frameworks and other games organise skills, how a level is earned, and what a
level gives, and says which ideas fit a game where a person learns the REAL skill.

Two research helpers read the sources on 2026-11-14. Every entry in the two notes files carries its
source and link, the source's own date (or "no date on page"), one exact quotation of under 15 words,
the helper's paraphrase, a judgement of fit and a confidence. Each site's robots.txt was read first,
in a fetch of its own; sites that refused or barred Claude are listed at the end of each file and were
not worked around (a re-hosted copy of a refused paper was found once and deliberately not read).

| File | What it covers |
|---|---|
| [A-real-world-frameworks.md](A-real-world-frameworks.md) | A1 O*NET's skills and knowledge (US Department of Labor); A2 ESCO (European Commission); A3 England's apprenticeship occupational standards and the farrier end-point assessment; A4 the Dreyfus five-stage model; A5 Scouts Victoria's Outdoor Adventure Skills (Scouting America refused); A6 British Judo Association kyu and dan grading; A7 juggling, baton twirling and flow arts grading; A8 certificates that expire (HSE first aid, Washington State electricians) |
| [B-games.md](B-games.md) | B1 RuneScape; B2 Skyrim; B3 Wurm Online; B4 Vintage Story; B5 Project Zomboid; B6 Kenshi; B7 Mortal Online 2; B8 EVE Online; B9 Starfield's challenges and Fallout 4's perks; B10 Valheim and Stardew Valley as a simple contrast |

## The comparison in one table

| Source | How skills are organised | How a level is earned | What a level gives |
|---|---|---|---|
| O*NET (A1) | Skills kept apart from knowledge; a few essential skills under everything | Not earned: occupations are rated, Level 0 to 7 anchored by example tasks | A description of what the work needs |
| ESCO (A2) | Transversal roots, cross-sector skills, then sector and specialist skills | No grades at all | A shared vocabulary |
| Apprenticeship standards (A3) | Knowledge, skills and behaviours per occupation | Practice to a gateway, then separate methods (test, discussion, practicals, a portfolio of things you made), every one must pass | Pass or distinction; the right to work in the trade |
| Dreyfus (A4) | Five stages, novice to expert | Experience; no test | Rules and step-by-step guidance fall away as you rise |
| Scouts (A5) | Stages per skill, written as "I can" statements | Demonstrate every statement; a verifier at least two stages above signs off; prior real skill counts with evidence | Stages roll up into broader awards |
| Judo (A6) | Kyu then dan grades | Demonstrate named techniques and explain choices; minimum time in grade that lengthens; several routes (contest, technical, service) | Rank, and the move from learning technique to applying it |
| Juggling and twirling (A7) | Levels of countable feats | All or nothing, judged on clear faults, by video; feedback on failure | A certified level; competition on top. No grading body found for poi or staff |
| Certificates that expire (A8) | A competence plus its currency | A course, then refreshers or continuing education | Permission to act, until a date; after a long lapse, a retest |
| RuneScape (B1) | Combat, gathering, artisan and support skills | XP per action, plus quest rewards | Each level opens named recipes, items and places |
| Skyrim (B2) | Skills with perk trees | Use fills the skill's bar; skill-ups raise the character level | Perks chosen by the player shape the specialist |
| Wurm Online (B3) | Parent and child skills (child practice feeds the parent) | A chance per action, by difficulty against your skill and tool quality; improving teaches more than creating | Success chance and quality |
| Vintage Story (B4) | No skill numbers | The player learns real-ish procedures (knapping, smithing a voxel grid) from a handbook | Nothing visible: mastery is the player's own |
| Project Zomboid (B5) | About two dozen skills | Doing; books multiply practice XP, only within their level range | Recipes and better results |
| Kenshi (B6) | Stats by use | Faster gain when the challenge exceeds you | Better odds and speed; also invited exploit-training |
| Mortal Online 2 (B7) | Skills under a points cap | Books bring a skill to a base, practice to mastery; child skills cannot pass their parent | Breadth traded against depth |
| EVE Online (B8) | Skill books, ranks, mastery lists | Real time, online or off | Ship and module access; mastery lists as curricula |
| Starfield (B9) | Skills with four ranks | A point buys rank 1; each later rank needs its challenge done first | Each rank's perk |

## Which ideas fit a game where you learn the real skill, and why

Ranked by how well each serves the goal (a person who levels a skill in the game should actually be
better at it, and should never be able to level it without learning).

1. **The procedure is the skill (Vintage Story, B4).** When the interaction is the real procedure,
   learning the game is learning the craft. This is already the activities catalog's first rule
   (docs/design/activities.md section 2). Its weakness, mastery leaving no record, is what the rest
   of this list fixes.
2. **A stage is earned by demonstrating it (Starfield B9, Scouts A5, juggling A7, apprenticeship
   A3).** Practice alone never passes a stage; showing the competence does. The demonstration must be
   a real competence check judged by the simulation (a weld that holds, a loaf that rose, a splint
   that immobilises), never a count of kills or metres, which teaches nothing. Apprenticeships add
   the weakest-link rule (every part must pass) and the portfolio (things you actually made).
3. **"I can" statements, all lower ones still required (Scouts A5).** Each stage of each skill is a
   short list of things a person can do, which a player reads, a page shows, and an assessment checks.
   They double as the curriculum and as the plain-words answer to "what is this level?".
4. **Practice teaches most just beyond your level (Wurm B3, Kenshi B6).** Gain depends on difficulty
   against your skill: easy work teaches little, work slightly beyond you teaches most. That is
   deliberate practice, and it is the cure for grinding. Kenshi shows the trap to avoid: when the
   only lever is "harder", players farm it with nonsense (getting beaten up on purpose).
5. **Study multiplies practice but never replaces it (Project Zomboid B5, Mortal Online 2 B7).**
   Reading the right Library guide makes the next attempts count for more, and only at the stage it
   is written for. Theory brings a base; only practice brings mastery.
6. **Guidance fades as you rise (Dreyfus A4).** What a stage gives is not only numbers: a novice
   gets step-by-step prompts, an expert none. Dreyfus is a teaching heuristic, not proven science
   (A4 cites the critique), so use its stage names as plain meanings, not as a formula.
7. **Competence is permanent; currency runs out (first aid and electricians, A8).** Separate the
   stage (earned once) from recent practice (a "current until" date a refresher renews). Losing a
   stage after a lapse is real (the 90-day retest) but punishing, so it belongs only in the full
   realism mode.
8. **Teach and verify two stages down (Scouts A5, judo A6).** A person two stages above can sign a
   learner off, and higher stages need a qualified examiner. That is a ready-made multiplayer loop and
   it makes teaching a way to advance (judo's service route).
9. **Parents and transfers (Wurm B3, ESCO A2, Mortal Online 2 B7).** Related skills help each other:
   practising a specialty builds the shared fundamentals. Use it only where the overlap is real; the
   operator's own test was "why would fishing make me better at finding mushrooms?"
10. **Knowledge is not a skill (O*NET A1).** Knowing soil chemistry is not growing a crop. Knowledge
    areas (maths, physics, biology) belong to the curriculum (data/curriculum/syllabus.json) and are
    checked by questions; skills are earned by doing, and a stage may require its knowledge topics.
11. **A visible unlock ladder (RuneScape B1).** Every stage says exactly what it opens. Borrow the
    ladder, not the per-action XP loop that rewards repetition.

## Ideas to avoid, and why

- **Time-based training (EVE Online, B8).** Skill that grows while you are away is the opposite of
  learning by doing. Its mastery lists are worth borrowing as curricula; the clock is not.
- **Kill counts and distance counts as the proof (Starfield's challenge examples, B9).** The gate is
  right, the evidence is wrong: a count says nothing about technique.
- **Use-counting with no difficulty or quality term (Skyrim, B2; RuneScape, B1).** Players repeat the
  cheapest action; the level then measures patience, not skill.
- **A hard points cap that forces forgetting (Mortal Online 2, B7).** It makes players depend on each
  other, which suits a cooperative game, but deleting a skill a person really learned teaches the
  wrong thing; the operator has also said no cap for now.

## Gaps

The notes could not read: the Old School RuneScape wiki, pzwiki, the Stardew wiki and several others
(robots.txt bars Claude); UESP and the Fandom wikis (robots.txt refused); scouting.org and redcross.org
(robots.txt refused), so Scouts Victoria and the UK Health and Safety Executive stood in; the 1980
Dreyfus report (DTIC refused), so a 2010 peer-reviewed article that cites it stood in. Because of
that, the notes leave out RuneScape's level-99 XP figure, Kenshi's multiplier, Wurm's skill count,
and the Valheim and Stardew level rules; nothing in skills.md rests on those.
