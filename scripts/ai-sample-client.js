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
let allRooms = [];           // [{id,name,room_type,position,size,center}, ...]
let visitedRooms = new Set(); // room ids the bot has visited
let tourQueue = [];          // remaining room ids to walk through

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
    allRooms = game.rooms || [];
    const quest = game.current_quest;
    const questLine = quest
      ? `Quest: ${quest.title} (${quest.visited?.length || 0}/${quest.total_rooms})`
      : 'No active quest.';
    if (quest?.visited) for (const r of quest.visited) visitedRooms.add(r);
    log('game_welcome', `You are entity ${myPlayerId}. World has ${entityCount} entities, ${allRooms.length} rooms. ${questLine}`);
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
    // Begin the explore_ship quest tour: teleport through every room we
    // haven't yet visited so the relay records the visits and emits
    // game_quest_progress events. Each move is well within the 100-unit
    // server-side teleport limit because the Pioneer is small (~30m).
    tourQueue = allRooms
      .map(r => r.id)
      .filter(id => !visitedRooms.has(id));
    if (tourQueue.length === 0) {
      console.log('\nQuest already complete — disconnecting.');
      setTimeout(() => ws.close(), 250);
      return;
    }
    state = 'touring';
    console.log(`→ Starting room tour for ${tourQueue.length} room(s)`);
    sendNextTourStep();
    return;
  }

  // Ambient chatter from the world (v0.165.0). Surfaced even after the
  // demo state machine completes, until the safety-net timeout fires.
  if (game.type === 'game_ambient_chatter') {
    log('Ambient chatter', `${game.speaker} (${game.room_id}): "${game.line}"`);
    return;
  }

  // NPC greeting on first room entry (v0.169.0). Resident NPC says hi.
  if (game.type === 'game_npc_greeting') {
    log('NPC greeting', `${game.speaker}: "${game.line}"`);
    return;
  }

  // Quest progress / completion events (v0.167.0). The relay sends private
  // game_quest_progress to the questing player on each new room visited and
  // a public game_quest_completed when all rooms have been visited.
  if (game.type === 'game_quest_progress' || game.type === 'game_quest_completed') {
    const tag = game.complete ? '✓ COMPLETE' : 'progress';
    log(`Quest ${tag}`, `${game.quest_id} → entered ${game.room_id} (${game.visited_count}/${game.total})`);
    visitedRooms.add(game.room_id);
    if (game.complete && state === 'touring') {
      console.log('\nQuest complete — disconnecting.');
      setTimeout(() => ws.close(), 500);
    } else if (state === 'touring') {
      // Move on to the next room after a short pause.
      setTimeout(sendNextTourStep, 300);
    }
    return;
  }

  if (game.type === 'game_error') {
    console.error(`\n[ERROR] ${game.error}: ${game.message}`);
  }
});

/** Pop the next room off the tour queue and teleport the bot into it. */
function sendNextTourStep() {
  while (tourQueue.length > 0) {
    const roomId = tourQueue.shift();
    if (visitedRooms.has(roomId)) continue;
    const room = allRooms.find(r => r.id === roomId);
    if (!room) continue;
    console.log(`→ Teleporting to ${room.name} (${roomId}) at [${room.center.map(n => n.toFixed(1)).join(', ')}]`);
    send({
      type: 'game_position_update',
      position: room.center,
      rotation: [0, 0, 0, 1],
      velocity: [0, 0, 0],
      timestamp: Date.now() / 1000,
    });
    return;
  }
  console.log('\nNo more rooms to visit — disconnecting.');
  setTimeout(() => ws.close(), 250);
}

// Safety net — anything stuck for >30 sec is a bug.
setTimeout(() => {
  console.log('\nTimeout — disconnecting (was in state:', state, ')');
  ws.close();
}, 30000);
