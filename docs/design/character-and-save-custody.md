# Characters, profiles, and save custody

Status: the decoupling is shipped (v0.448); the cloud/server-custody modes are designed here
and build on the existing `WorldSave.kind` axis. Operator direction, 2026-06-14.

## Three separate things (do not conflate)

1. **Chat / network identity** -- WHO YOU ARE online. The Dilithium3 keypair derived from
   your BIP39 seed, plus the network profile (display name, bio, avatar URL). Self-custodial
   by construction: it lives in your seed, no account, no server owns it. (See the
   Cryptography section in CLAUDE.md.)
2. **Game character** -- WHO YOU PLAY. A `character_name` + `Appearance` + `Outfit`, stored in
   the `WorldSave`. As of v0.448 this is DECOUPLED from the chat profile: the character has
   its own name (the player's ECS `Name`, edited in the showroom), and its appearance/outfit
   were never part of the network profile (the relay + profile page never reference them).
   You can be "Shaostoul" in chat and play a character named "Astra" who looks however you
   like. One identity, many characters.
3. **Home / save** -- WHERE YOU PLAY + the progress. The `WorldSave` (name, inventory, skills,
   constructions, the character above). A home IS a save (homes-as-saves, v0.380).

## Custody model (the `WorldSave.kind` axis)

`kind` already encodes who owns the truth of a save: `offline` | `server` | `real`.

- **offline (local, self-custodial)** -- you own the save file; it lives on your disk. Full
  self-custody of your character + progress. The only live mode today. Single-player and
  listen-host. No server can take it; no server can verify it (so not for competitive MMO).
- **server (server-authoritative, "forced cloud")** -- a relay owns the truth of the save.
  For MMORPG servers: the server stores the character + progress so it can validate actions
  and resist cheaters/hackers (a client cannot fabricate its own progress). This is the
  operator's "forced cloud saves" -- a deliberate trade of self-custody for integrity, chosen
  per-server. Your IDENTITY stays self-custodial (your seed/key); only the world-state custody
  moves to the server.
- **real (sensor-owned)** -- physical sensors own the truth (the real-life homestead mirror).
  Deferred.

## Self-custodial accounts on servers (the goal)

You should be able to bring your CHARACTER to a server without creating a heavy account: the
server authenticates you by your Dilithium key (already how the relay identifies clients --
two-phase identify challenge, see CLAUDE.md), accepts your chosen character (name + appearance
+ outfit), and stores **as little as possible** -- ideally only the world-authoritative state
it must own to prevent cheating (your progress on THAT server), keyed by your public key. No
email, no password, minimal PII. The character is yours (self-custodial); the server hosts the
world and owns only what integrity requires.

## The resolution of the tension

- **Identity** is ALWAYS self-custodial (your seed -> Dilithium key). No exceptions.
- **Character look + name** are self-custodial data you present; a server may cache them to
  render you to others (the public half could ride the signed-profile gossip), but you own the
  source.
- **Progress/world-state custody** is per-save-kind: `offline` = you own it; `server` = the
  server owns it (for anti-cheat). The player chooses per home/server which trade they want.

## What is built vs next

- Built (v0.448): character decoupled from chat profile (own name + appearance + outfit in the
  save); `offline` self-custodial saves round-trip all of it.
- Next: the `server` kind (cloud/server-authoritative saves) -- the relay storing a player's
  WorldSave keyed by public key, with the client syncing to it; the per-server "bring your
  character, minimal storage" flow; and the in-app picker to choose offline vs a server when
  creating/loading a character. Cheater-resistance comes from the server validating against
  its owned state, not the client's.
