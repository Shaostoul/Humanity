# The mothership, the player's acre, and the crew

Status: decided by the operator 2026-09-19, in conversation, and written down
the same night because it existed nowhere else. These answers set the order of
a lot of work, so they belong on disk rather than in a chat log.

## What the mothership is

A premade place. The operator's words: "the homeship is meant to be premade.
Kind of like a giant flying lived in city." It is authored content, not
something players build. It exists before any player arrives and it carries on
without them.

**The player does not own it and is not responsible for it.** "The mothership is
meant to be self-sufficient without the player because the entire crew is taking
care of it. The player just happens to be one of the crew members." Nothing in
the world should stall, spoil or break because a particular person logged off.
A solo player is a resident, not a caretaker.

## What a player may change

Their own home area, and only that. "In normal single-player and most
definitely in the MMO mode the only place an individual player should modify is
their specific home area in the spaceship. Like the 1 acre or whatever it is for
that specific ship design."

So the sandbox is the acre and the ship around it is the level. Two consequences
worth stating plainly, because they are easy to get backwards:

- Ship-wide construction is a DEVELOPER activity, not a player one.
- A player's acre is genuine construction, not decoration, and it is the only
  place their building skill is exercised.

**The Fibonacci homestead is not a rule.** It was "mostly just me trying to
figure out how to lay things out and scale things. Not like a fixed all players
or ships follow this rule. Just some way to try to make sense of such a complex
concept." Treat that work as an exploration of scale, not a layout every ship
must obey.

## The construction tools serve both, and that is the leverage

This is the most consequential line in the whole conversation:

> "Ideally we make the construction tools so good that whether we're a player
> messing with our one home or a dev (human or AI) creating the mother ship we
> have the easiest time building. A bad building experience for player or dev
> means we don't get as good as looking spaceships."

The same tool builds a player's kitchen and the ship's residential module. That
means improving it pays twice, and it means the quality ceiling of the entire
world is set by how good that tool is. A developer fighting a clumsy editor
produces a clumsy ship, and no amount of art fixes it afterwards.

It also reframes several complaints that look like separate problems. A badly
arranged home, a residential module that is "just kinda scattered about", and
the inability to build multi-storey interiors are all downstream of the same
question: how good is the thing we build with. Fixing the tool addresses all
three; hand-authoring each area in data files addresses one at a time and leaves
players with the same bad experience.

## Crew, and why they come later

The ambition is real: NPCs with their own lives and their own homesteads aboard,
wandering, repairing, transporting, flying. Not decoration. "It'd be cool for
NPCs to actually wander the ship, repair stuff, transport materials, fly ships,
etc. and have it all actually be doing something instead of cosmetic."

Two constraints shape when that can happen.

**Scale.** The population is meant to be in the billions, so the simulation must
be cheap. The design he describes is a felt abundance over a real ledger: the
player "would feel like they have unlimited resources but, realistically we can
have finite resources." Start simplified, go advanced later.

**Dependency, which is the scheduling point.** An NPC can only do what the game
can already do. His reasoning, verbatim: "the NPCs actions would depend on all
the noncombat (and eventually combat) gameplay loops being done. IE: They can't
farm if the farming minigame is incomplete. They can't repair a vehicle if the
vehicle system is incomplete."

So NPC behaviour is downstream of every loop it would perform. Building NPCs
before the loops means building animations of work rather than work, which is
exactly the cosmetic outcome he does not want. **The loops are the critical
path; the crew is what makes them visible at scale afterwards.**

## Explicitly deferred

Named by the operator as wanted but too large while core features are missing:

- Combat.
- NPCs living full simulated lives (the advanced mode above).
- Player roles such as captain, designer or architect, which would let a player
  affect the mothership itself rather than their acre.

Deferred is not rejected. Each is a real goal with a reason it is not now.

## What this decides

- The ship is level design; the acre is systems design.
- Construction tooling is upstream of the world's visual quality, for players
  and developers alike.
- Nothing may depend on a player being present.
- Loops first, crew second, and an NPC never performs a loop the game has not
  finished.
