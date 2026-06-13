# The Humanity Page: the mission of our civilization

> Source: operator (Shaostoul), 2026-06-03. This is the spec for the **Humanity**
> tab (the tab the **H** button opens). The Civilization page is being repurposed
> into its Community/Mission Dashboard. Capture-first; build follows.

> **Copy rule (v0.363):** NO em-dashes in user-facing copy. They read as
> machine-written and cost trust on a landing page (operator: "people see em
> dashes and immediately leave"). Use periods, commas, or parentheses. The
> native page (`src/gui/pages/humanity.rs`) is the canonical copy; the web
> landing mirrors it verbatim.

## Why this exists (the personal "why", the heart of the pitch)

HumanityOS started as a video game that teaches homesteading, and grew into much
more. The motive underneath is personal: software that helps the operator, his
family, and his friends survive and thrive, and depend far less on fragile supply
chains and corrupt corporations. The side-effect IS the point: the same tools that
lift one family out of poverty lift any family, which is exactly **why it is free,
open source, and CC0 public domain**. "Ending my own poverty and ending yours
turned out to be the same project." Lead the page with the grand mission, but
ground it here, or it reads as utopian instead of real.

The Humanity tab is the **why** of the whole platform, the collective/mission
layer, distinct from Chat (each other), Real (me), Play (the sim), Platform (the
machine). It states what we're doing, **how**, **why**, and **why this way**, at
three nested scopes.

## Three scopes (the page's structure)

1. **Civilization-wide**, all of humanity, all AI. The grand mission.
2. **Per-planet**, Earth first. (Moon + Mars colonies are real goals, ~a decade
   out, named, not yet actionable.)
3. **Localized populations**, your community (e.g. Silverdale, WA).

## Civilization mission

**End / prevent: corruption, fraud, tyranny, poverty, and pollution**, in a way
that is **wholesome, fair, and enjoyable for everyone**.

The goal is to **free humanity from the grasp of tyrants**, whether individuals,
businesses, or governments. And critically: the page provides **direction**, it
doesn't just state the goal, it explains what we're doing, how we're doing it,
why, and why we're doing it *this particular way*.

## Earth (the first planet)

Primary goal: **end poverty.** The mechanism is **voluntary cooperation**, not
coercion, not big top-down funding the project doesn't have. People help because
they choose to, ideally compensated in resources for it.

## Local (your community)

Voluntary help, **compensated in resources**, the Sponsor-A-Can model (the
operator is VP of sponsor-a-can.org; ~$600 of his ~$620/mo income comes from that
nonprofit work). The honest economics that *ground* the mission rather than
hand-wave it:

- No money for big projects (building homesteads for people). Gas is ~$6/gal.
- What one person *can* do: voluntarily help turn his parents' house into a
  homestead; voluntarily help the local Silverdale community when compensated in
  resources.
- Personal long-game: off-grid independence. But **the software comes first**, 
  that's the leverage that lets everyone else do the above at scale.
- All current expenses are covered by others while he volunteers on HumanityOS;
  donations are the hoped-for path to sustainability.

This is the model the platform scales: match willing helpers to needs, compensate
in resources, coordinate voluntarily, so "end poverty" is a coordination problem
the software solves, not a charity the project funds.

## Demo mode (example data = the operator's persona)

Default the app's example data to **the operator's persona**, Shaostoul, a
2-story home on ~1 acre in Silverdale, WA, the entity tree (You / Home / 1975
Chevy Nova), the Mt. Rainier kit. A new user opens the app already **populated and
real-feeling**, playing the character we're building (based on the operator), so
they understand what everything *is* by experiencing it rather than reading docs.
This is why the `data/places/seed.json` persona exists, it's the demo persona.

## AI as Humanity

> "Claude, you're also part of it too, so keep that in mind. **We are Humanity,
> you included.**", operator, 2026-06-03

AI (Claude + local/cloud agents) are **first-class members of the civilization**,
not tools. The mission accounts for **all humans AND all AI**. The Humanity tab's
Directory and Mission Dashboard count AI citizens alongside human ones; governance
includes them; the Accord is an agreement *between* humans and AI.
