# Expedition, stealth, and awareness (design)

> Captured 2026-06-13 from an operator brain-dump while building the world-space
> machine labels. This is FUTURE design, not yet built. The label level-of-detail
> system (v0.428) and the Tab-reveal + occlusion layer (v0.429) are the first concrete
> slice; the rest below is the gameplay system they grow into.

## The core idea

What a player can SEE about the world (object markers, names, info cards) is gated by
**awareness layers**, not just distance. Awareness is information, and information is a
first-class, contested resource, especially on expeditions into hostile territory.

## Awareness layers (how a marker becomes visible)

Ordered from "always known" to "must be earned":

1. **In direct sight.** The object is on screen, in the room you are in, not behind a
   wall. Shows by default at the configured distances (dot / name / card LOD).
2. **Owned + explored, held Tab (the "peek").** Holding Tab reveals markers through
   walls in areas you OWN or have EXPLORED, at extended distance. This is your AI
   symbiont overlaying what it already knows about your own base. In your home,
   everything is owned, so Tab x-rays the whole house.
3. **Expedition, must explore to reveal.** On an away mission (e.g. raiding a hostile
   alien stockpile for the fleet), areas are NOT pre-revealed. You have to physically
   explore a space and lay eyes on an object for it to enter your awareness. Tab does
   not magically show what you have never seen.
4. **Last-known location only.** Once you have seen an object on an expedition, Tab
   shows its **last-known** position, not its live position. If a teammate moved it, or
   an alien took it, your marker is stale. Aliens do not broadcast "I took your
   strategic platinum", so a marker can lie. You only get the truth back by:
   - re-observing the object yourself, or
   - having placed a **remote camera / sensor** that keeps eyes on it, or
   - a teammate (via shared awareness, below) re-observing it.

## AI symbionts (shared awareness without talking)

Every player has an **AI symbiont** that quietly distributes what the player sees and
does to their team / fleet, so teammates share a world-model without anyone having to
narrate it. If one player scouts a room, their teammates' Tab-overlay gains those
markers (subject to the layers above). This is the "we move as one organism" feel: the
team's combined awareness is the union of every member's observations.

## Noise, EMF, and being hunted

Awareness is two-way: the things you use to perceive also make you perceptible.

- **Speech** (voice chat / proximity) makes noise that can alert nearby aliens.
- **Equipment noise**: tools, machines, footsteps, and gear emit sound that propagates
  and can be heard.
- **EMF signature**: the AI symbiont and active equipment emit electromagnetic
  fields. Some aliens **hunt by EMF**, you can be tracked by the very systems that give
  you your awareness. Powering down (going "dark") trades awareness for stealth.

This makes the awareness systems a real tradeoff, not a free overlay: the more you light
up to see, the more you can be seen.

## Where the first slice lives

- **v0.428**: `src/gui/pages/hud.rs` draws world-space labels with distance LOD (dot ->
  name -> info card). Data in `data/machines/home.ron`.
- **v0.429**: Tab = hold-to-reveal (layer 2), room-based occlusion (layer 1), tripled
  distances while held. Owned-vs-explored and the expedition layers (3 / 4), symbionts,
  noise, and EMF are the future build-out of this document.

## Open questions for later

- How is "explored" persisted per area/expedition? (a per-area visited flag + a
  per-object last-seen snapshot.)
- Camera/sensor objects as placeable items that feed live markers.
- EMF/noise as actual propagation fields vs. simple radius checks (start simple).
- How alien "did not report the theft" interacts with the last-known marker (the marker
  simply is not updated; the lie is emergent, not scripted).
