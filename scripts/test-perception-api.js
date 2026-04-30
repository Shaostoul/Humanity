#!/usr/bin/env node
// Test the AI Perception API end-to-end via WebSocket.
// Connects, identifies, joins the game world, and queries perception.
//
// Usage:
//   node scripts/test-perception-api.js              # against live VPS
//   node scripts/test-perception-api.js localhost    # against local relay (port 3210)

const url = process.argv[2] === 'localhost'
  ? 'ws://127.0.0.1:3210/ws'
  : 'wss://united-humanity.us/ws';

const crypto = require('crypto');
const pubKey = crypto.randomBytes(32).toString('hex');
const name = 'PerceptBot' + Math.floor(Math.random() * 1000);

console.log(`Connecting to ${url}`);
console.log(`Identity: ${name} (${pubKey.slice(0, 16)}...)`);

const ws = new WebSocket(url);

const log = (label, obj) => {
  console.log(`\n[${label}]`);
  console.log(JSON.stringify(obj, null, 2).slice(0, 2000));
};

let step = 0;

ws.addEventListener('open', () => {
  console.log('\n✓ WebSocket open');
  // Step 1: identify
  ws.send(JSON.stringify({
    type: 'identify',
    public_key: pubKey,
    display_name: name,
  }));
});

ws.addEventListener('message', (ev) => {
  const text = typeof ev.data === 'string' ? ev.data : ev.data.toString();
  let msg;
  try { msg = JSON.parse(text); } catch { msg = { raw: text }; }

  // Game messages come prefixed __game__:
  let gameMsg = null;
  if (msg.type === 'private' && msg.message?.startsWith('__game__:')) {
    gameMsg = JSON.parse(msg.message.slice('__game__:'.length));
  } else if (msg.type === 'system' && msg.message?.startsWith('__game__:')) {
    gameMsg = JSON.parse(msg.message.slice('__game__:'.length));
  }

  if (gameMsg) {
    log(`game_msg: ${gameMsg.type}`, gameMsg);

    if (gameMsg.type === 'game_welcome' && step === 0) {
      step = 1;
      console.log('\n→ Sending game_perceive');
      ws.send(JSON.stringify({ type: 'game_perceive', radius: 25 }));
    } else if (gameMsg.type === 'game_perception' && step === 1) {
      step = 2;
      // Try to interact with the first interactable nearby entity
      const target = (gameMsg.nearby_entities || []).find(e => e.interactable);
      if (target) {
        console.log(`\n→ Sending game_interact for entity ${target.entity_id} (${target.entity_type})`);
        ws.send(JSON.stringify({
          type: 'game_interact',
          entity_id: target.entity_id,
          action: 'inspect',
        }));
      } else {
        console.log('\n(no interactable entities nearby — ending test)');
        ws.close();
      }
    } else if (gameMsg.type === 'game_interact_result') {
      console.log('\n✓ Test complete — disconnecting');
      setTimeout(() => ws.close(), 500);
    }
  } else if (msg.type === 'identified' || msg.type === 'welcome' || msg.type === 'history') {
    if (step === 0 && (msg.type === 'identified' || msg.type === 'welcome')) {
      console.log(`\n[identify ack: ${msg.type}]`);
      console.log('→ Sending game_join');
      ws.send(JSON.stringify({ type: 'game_join', player_name: name }));
    }
  } else if (msg.type === 'system' || msg.type === 'name_taken') {
    log(`relay_msg: ${msg.type}`, msg);
    if (msg.type === 'name_taken') {
      console.log('Name conflict — exiting');
      ws.close();
    }
  }
});

ws.addEventListener('error', (e) => {
  console.error('\n✗ WebSocket error:', e.message || e);
});

ws.addEventListener('close', (ev) => {
  console.log(`\n✓ Closed (code: ${ev.code})`);
  process.exit(0);
});

// Safety timeout
setTimeout(() => {
  console.log('\n✗ Timeout — closing');
  ws.close();
}, 15000);
