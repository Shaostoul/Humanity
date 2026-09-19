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

So NPC behaviour is downstream of every loop it would perform. A crew member
cannot really farm until farming is finished.

**But cosmetic first is explicitly welcome, and that was a correction to an
earlier reading of this page.** The operator, the same night: "I don't mind
cosmetic at first to give the illusion of a living world though the actual
living NPCs would be the ideal end result. Simple cosmetic actions are at least
the least processor demanding. Calculating NPCs actually doing things could
rapidly tank performance if we don't do it right considering the mothership has
a population of over a billion."

There is no contradiction between that and the dependency rule, and the
distinction is worth holding precisely:

- **A cosmetic NPC performs no loop.** It walks a corridor, tends a plot,
  carries a crate. It is animation, it costs almost nothing, and it can ship
  today because it depends on nothing.
- **A real NPC performs the loop the player performs**, using the same systems,
  and cannot exist before that system does.

So the order is: cosmetic crew whenever it makes the ship feel inhabited, real
crew per loop as each loop is finished, and never a cosmetic stand-in
pretending to be the real thing where a player could tell the difference.

**Performance is the reason this is staged, not just sequencing.** A billion
residents cannot each be simulated. Whatever the eventual design, it will be
tiers: a handful genuinely simulated near the player, a larger ring of cheap
approximations, and a statistical population beyond that. Building the cheap
tier first is therefore not a shortcut being paid off later, it is the tier the
overwhelming majority of the population will always use.

## Homes: one good design, then many

The operator, 2026-09-19: "feel free to redesign the whole initial player home
to better utilize the full space we have. You can base it on the fibonacci
sequence or you can figure out something better."

The reason it matters beyond the player's own front door is the reuse: "When we
make a great player home then we can mirror that to all the NPC homes so they're
aesthetically pleasing should the player enter one for whatever reason." So the
starting home is not one room to decorate, it is the template the inhabited ship
is made of. A player who walks into a neighbour's quarters should find a place
someone lives, not an empty shell, and that comes free if the design is good and
the data is reusable.

Two things follow that should shape how it is built:

- **It must be data, not code.** Premade homes are wanted in variety, and
  beyond that: "We could even introduce the ability for players to submit home
  designs so it's not just us making stuff." A layout that only a programmer can
  author cannot become a library that players contribute to. This is the
  infinite-of-x rule applied to architecture.
- **Every page wants a place.** Screens should sit where a person would go to do
  that thing. The operator's own example: "the player bedroom could have like a
  standing mirror like display that also doubles as a touchscreen for changing
  character appearance." That is the pattern to follow, and it is better than a
  wall of identical monitors, because it makes the interface part of the room.

The honest constraint today is that multi-storey interiors do not work yet, so a
first redesign is single floor. What a second storey needs is small and known,
and it is the same set of fixes the construction editor needs, which is another
reason the tool is upstream of everything here.

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
