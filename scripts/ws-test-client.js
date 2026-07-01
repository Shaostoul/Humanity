#!/usr/bin/env node
// Lightweight relay WS protocol test client (2026-07-01 overnight loop infra).
//
// Authenticates via the bot_secret fastpath (src/relay/relay.rs ~2542, requires
// API_SECRET set on the relay process) so protocol-level tests don't need the
// full Dilithium identify handshake -- this is for testing message ROUTING and
// STORAGE against a local relay, not for exercising the real crypto path.
//
// Usage (against a local headless relay only -- never point RELAY_URL at
// production, this test key has no real identity and bot_ keys get special
// server-side treatment):
//   API_SECRET=<same value the relay was started with> \
//   node scripts/ws-test-client.js ws://127.0.0.1:<port> bot_test1 \
//     '{"type":"get_notification_prefs"}' \
//     '{"type":"update_notification_prefs","dm":false,"mentions":true,"tasks":true}' \
//     '{"type":"get_notification_prefs"}'
//
// Prints every message received (one JSON object per line) for 2 seconds after
// the last send, then exits. Exit code 0 always (this is an observation tool,
// not a pass/fail assertion -- pipe stdout through your own check).

const url = process.argv[2];
const botKey = process.argv[3];
const sends = process.argv.slice(4);
const apiSecret = process.env.API_SECRET || '';

if (!url || !botKey) {
  console.error('Usage: node scripts/ws-test-client.js <ws-url> <bot_key> [json-message]...');
  process.exit(1);
}

const ws = new WebSocket(url);

ws.addEventListener('open', () => {
  ws.send(JSON.stringify({
    type: 'identify',
    public_key: botKey,
    display_name: botKey,
    bot_secret: apiSecret,
  }));
  let i = 0;
  const sendNext = () => {
    if (i >= sends.length) return;
    ws.send(sends[i]);
    i += 1;
    setTimeout(sendNext, 150);
  };
  setTimeout(sendNext, 150);
});

ws.addEventListener('message', (ev) => {
  console.log(typeof ev.data === 'string' ? ev.data : ev.data.toString());
});

ws.addEventListener('error', (ev) => {
  console.error('ws error:', ev.message || ev);
});

let lastActivity = Date.now();
const origLog = console.log;
console.log = (...args) => { lastActivity = Date.now(); origLog(...args); };

const timer = setInterval(() => {
  if (Date.now() - lastActivity > 2000) {
    clearInterval(timer);
    ws.close();
    process.exit(0);
  }
}, 200);

setTimeout(() => {
  clearInterval(timer);
  ws.close();
  process.exit(0);
}, 10000);
