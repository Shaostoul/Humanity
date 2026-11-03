// `just feedback` (scripts/feedback.js, 2026-11-02, docs/design/chat-channels.md section 3), run
// under Node with the server's answers stood in:
//
//   node --test scripts/tests/feedback.test.js        (in `just rig-tests`)
//
// What it proves: a server without a #feedback anyone can read says so in words, not with an error
// or an empty screen; a server with one prints each line's time, name and words, a linked
// server's line saying which server; a refusal names the status and the server's words.
//
// Seen red 2026-11-02 with `report` reading the lines without asking for the channel list first:
// "a server without #feedback says so" (actual: "https://u.example refused #feedback's lines (HTTP 401): This
// channel is for people with a name on this server: sign in to the chat to read it.").

const test = require('node:test');
const assert = require('node:assert');
const F = require('../feedback.js');

const answers = (routes) => async (url) => {
  const path = url.replace(/^https?:\/\/[^/]+/, '');
  const hit = Object.keys(routes).find((p) => path.startsWith(p));
  return hit ? routes[hit] : { status: 404, body: null };
};

test('a server without #feedback says so', async () => {
  const lines = await F.report('https://u.example/', 20, answers({
    '/api/channels': { status: 200, body: { channels: [{ id: 'announcements' }, { id: 'global' }] } },
    '/api/messages': { status: 401, body: { error: 'This channel is for people with a name on this server: sign in to the chat to read it.' } },
  }));
  assert.strictEqual(lines.length, 1);
  assert.match(lines[0], /^https:\/\/u\.example has no #feedback that anyone can read yet\./, 'a server without #feedback says so');
});

test('the lines print time, name and words; a linked server says which', async () => {
  const t0 = new Date(2026, 10, 2, 21, 14).getTime();
  const lines = await F.report('https://u.example', 2, answers({
    '/api/channels': { status: 200, body: { channels: [{ id: 'global' }, { id: 'feedback' }] } },
    '/api/messages?channel=feedback&limit=2': { status: 200, body: { cursor: 9, messages: [
      { type: 'chat', from: '', from_name: 'Ann', content: 'the map is hard\nto find', timestamp: t0, channel: 'feedback' },
      { type: 'federated_chat', server_name: 'public.guide', from_name: 'Cy', content: 'love the garden', timestamp: t0 + 60_000, channel: 'feedback' },
    ] } },
  }));
  assert.deepStrictEqual(lines, [
    '#feedback on https://u.example, the newest 2:',
    '2026-11-02 21:14  Ann: the map is hard / to find',
    '2026-11-02 21:15  Cy (public.guide): love the garden',
  ]);
});

test('a refusal names the status and the server\'s words', async () => {
  const lines = await F.report('https://u.example', 5, answers({
    '/api/channels': { status: 200, body: { channels: [{ id: 'feedback' }] } },
    '/api/messages': { status: 503, body: { error: 'The messages could not be read just now.' } },
  }));
  assert.deepStrictEqual(lines, ['https://u.example refused #feedback\'s lines (HTTP 503): The messages could not be read just now.']);
});
