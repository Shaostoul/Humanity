#!/usr/bin/env node
//
// HumanityOS — AI sample client
//
// Demonstrates how an AI agent uses the Perception API end-to-end:
//   1. Connect via WebSocket
//   2. Identify with a fresh Ed25519-shaped public key + display name
//   3. Wait for the post-identify ack (`peer_list`)
//   4. Send `game_join` to spawn into the world
//   5. Decode the `game_welcome` to learn our entity id
//   6. Send `game_perceive` to see the room + nearby entities
//   7. Send `game_interact` on the first interactable entity
//   8. Send `game_perceive` again to confirm the world reacted
//   9. Disconnect cleanly
//
// All game messages travel through the relay's chat WebSocket prefixed with
// `__game__:` inside the `private`/`system` message envelope. This client
// hides that detail behind `decodeGameMsg()` so the demo logic reads naturally.
//
// Usage:
//   node scripts/ai-sample-client.js              # against live VPS
//   node scripts/ai-sample-client.js localhost    # against local relay (port 3210)
//
// No npm dependencies — uses Node's built-in `WebSocket` (Node 22+).

'use strict';

const URL = process.argv[2] === 'localhost'
  ? 'ws://127.0.0.1:3210/ws'
  : 'wss://united-humanity.us/ws';

const crypto = require('crypto');
const PUBKEY = crypto.randomBytes(32).toString('hex');
const NAME = 'AISampleBot' + Math.floor(Math.random() * 10000);

console.log(`AI sample client → ${URL}`);
console.log(`Identity:        ${NAME}`);
console.log(`Public key:      ${PUBKEY.slice(0, 16)}…`);
console.log('');

// ── Helpers ────────────────────────────────────────────────────────────────

const ws = new WebSocket(URL);
const send = (obj) => ws.send(JSON.stringify(obj));
const log  = (label, data) => {
  console.log(`\n┌─ ${label}`);
  if (data !== undefined) {
    const s = typeof data === 'string' ? data : JSON.stringify(data, null, 2);
    s.split('\n').slice(0, 30).forEach(line => console.log(`│ ${line}`));
  }
  console.log('└───');
};

/**
 * Decode a server-sent message into either a regular relay event or a game
 * event. Game events arrive wrapped: `{type:"private"|"system", message:"__game__:{...}"}`.
 */
function decodeGameMsg(msg) {
  const wrap = msg && (msg.type === 'private' || msg.type === 'system');
  if (!wrap || typeof msg.message !== 'string') return null;
  if (!msg.message.startsWith('__game__:')) return null;
  try {
    return JSON.parse(msg.message.slice('__game__:'.length));
  } catch {
    return null;
  }
}

// ── State machine ──────────────────────────────────────────────────────────

let state = 'awaiting_identify_ack';
let myPlayerId = null;

ws.addEventListener('open', () => {
  console.log('WebSocket open. Sending identify…');
  send({ type: 'identify', public_key: PUBKEY, display_name: NAME });
});

ws.addEventListener('error', (e) => {
  console.error('WebSocket error:', e.message || e);
});

ws.addEventListener('close', (ev) => {
  console.log(`\nWebSocket closed (code ${ev.code})`);
  process.exit(0);
});

ws.addEventListener('message', (ev) => {
  const text = typeof ev.data === 'string' ? ev.data : ev.data.toString();
  let msg;
  try { msg = JSON.parse(text); } catch { return; }

  // ── Phase 1: get past identify, send game_join ──
  // Relay's first response after identify is always `peer_list`.
  if (state === 'awaiting_identify_ack' && msg.type === 'peer_list') {
    log('Identify ack', `Server welcomed us (${msg.peers?.length || 0} peers online)`);
    state = 'awaiting_game_welcome';
    console.log('→ Sending game_join');
    send({ type: 'game_join', player_name: NAME });
    return;
  }

  // ── Phase 2+: game messages ──
  const game = decodeGameMsg(msg);
  if (!game) return;

  if (game.type === 'game_welcome' && state === 'awaiting_game_welcome') {
    myPlayerId = game.player_id;
    const entityCount = (game.world_snapshot || []).length;
    log('game_welcome', `You are entity ${myPlayerId}. World has ${entityCount} entities.`);
    state = 'awaiting_perception';
    console.log('→ Sending game_perceive');
    send({ type: 'game_perceive', radius: 25 });
    return;
  }

  if (game.type === 'game_perception' && state === 'awaiting_perception') {
    const room = game.location;
    const nearby = game.nearby_entities || [];
    const env = game.environment || {};
    log('game_perception', {
      location: room ? `${room.name} (${room.deck} of ${room.ship})` : 'open space',
      exits: room?.exits?.map(e => `${e.direction} → ${e.room_name}`) || [],
      environment: env,
      nearby_count: nearby.length,
      nearby_sample: nearby.slice(0, 5).map(e => `${e.entity_type} (${e.distance.toFixed(1)}m, interactable=${e.interactable})`),
    });

    // Prefer talking to a crew NPC — bridge/medbay/engineering/cargo/hydroponics/quarters
    // each spawn a role-tagged NPC with dialog lines (v0.162.0).
    const npcTypes = ['navigator', 'medic', 'engineer', 'maintenance_bot', 'botanist', 'crewmate'];
    const npc = nearby.find(e => npcTypes.includes(e.entity_type) && e.interactable);
    const target = npc || nearby.find(e => e.interactable);
    if (!target) {
      console.log('\nNo interactable entities nearby — disconnecting.');
      ws.close();
      return;
    }
    const action = npc ? 'talk' : 'inspect';
    state = 'awaiting_interact';
    console.log(`→ Sending game_interact on entity ${target.entity_id} (${target.entity_type}) action=${action}`);
    send({ type: 'game_interact', entity_id: target.entity_id, action });
    return;
  }

  if (game.type === 'game_interact_result' && state === 'awaiting_interact') {
    if (game.dialog_line) {
      log('NPC dialog', `${game.speaker || 'NPC'}: "${game.dialog_line}"`);
    } else {
      log('game_interact_result', game);
    }
    state = 'awaiting_final_perception';
    console.log('→ Sending game_perceive again to confirm world state');
    send({ type: 'game_perceive', radius: 25 });
    return;
  }

  if (game.type === 'game_perception' && state === 'awaiting_final_perception') {
    log('game_perception (final)', `Saw ${(game.nearby_entities || []).length} nearby entities`);
    console.log('\nDemo complete — disconnecting.');
    setTimeout(() => ws.close(), 250);
    return;
  }

  if (game.type === 'game_error') {
    console.error(`\n[ERROR] ${game.error}: ${game.message}`);
  }
});

// Safety net — anything stuck for >20 sec is a bug.
setTimeout(() => {
  console.log('\nTimeout — disconnecting (was in state:', state, ')');
  ws.close();
}, 20000);
