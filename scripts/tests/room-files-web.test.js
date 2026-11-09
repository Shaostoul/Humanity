// A members-only room's files on the website (BUG-282, 2026-11-07): the relay serves a file posted
// in a room that does not read at Anyone only to that room's readers, and a picture, a player or a
// Download link cannot send a header, so the chat adds the room's ticket (?t=) to each such file's
// address (web/chat/app.js `withRoomTicket`, `ticketRoomFiles`); and the name a file card shows
// drops the 128 random bits the server now puts in every stored name (`fileCardName`).
// Run: node --test scripts/tests/room-files-web.test.js
//
// Seen red 2026-11-08 against the page before the fix (git show HEAD:web/chat/app.js): no
// withRoomTicket at all, and fileCardName showed "0123456789abcdef0123456789abcdef_plan.png".
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FILE = process.env.ROOM_FILES_APP || path.join(__dirname, '..', '..', 'web', 'chat', 'app.js');

/** The text of `function name(...) {...}` in `src`, to its closing brace at the start of a line. */
function fnText(src, name) {
  const start = src.indexOf('function ' + name + '(');
  assert.ok(start >= 0, name + ' is in app.js');
  const end = src.indexOf('\n}', start);
  return src.slice(start, end + 2);
}

function load() {
  const src = fs.readFileSync(FILE, 'utf8').replace(/\r\n/g, '\n');
  const ctx = vm.createContext({ URL, location: { href: 'https://united-humanity.us/chat', origin: 'https://united-humanity.us' } });
  vm.runInContext(fnText(src, 'fileCardName') + '\n' + fnText(src, 'withRoomTicket') + '\n;globalThis.__f = { fileCardName, withRoomTicket };', ctx);
  return ctx.__f;
}

test('a file card shows the name, without the time and the random bits', () => {
  const { fileCardName } = load();
  assert.strictEqual(fileCardName('/uploads/1794203031248_0123456789abcdef0123456789abcdef_plan.png'), 'plan.png');
  assert.strictEqual(fileCardName('/uploads/1794203031248_0123456789abcdef0123456789abcdef_plan.png?t=1.ab.cd'), 'plan.png');
  assert.strictEqual(fileCardName('/uploads/1760000000000_WH40K_Marriah_Carey.mp4'), 'WH40K_Marriah_Carey.mp4', 'an older name still reads');
});

test("a file of this server's gets the room's ticket once, and nothing else is touched", () => {
  const { withRoomTicket } = load();
  const t = '1794206631.0123456789abcdef0123456789abcdef.ffff';
  assert.strictEqual(withRoomTicket('/uploads/1_ab_x.png', t), '/uploads/1_ab_x.png?t=' + encodeURIComponent(t).replace(/%2E/g, '.'));
  assert.strictEqual(withRoomTicket('https://united-humanity.us/uploads/1_ab_x.mp4', t), 'https://united-humanity.us/uploads/1_ab_x.mp4?t=' + t);
  assert.strictEqual(withRoomTicket('/uploads/1_ab_x.png?t=old', t), '/uploads/1_ab_x.png?t=old', 'never twice');
  assert.strictEqual(withRoomTicket('https://elsewhere.example/uploads/x.png', t), 'https://elsewhere.example/uploads/x.png', 'never to another site');
  assert.strictEqual(withRoomTicket('/api/files/1', t), '/api/files/1', 'only /uploads/');
  assert.strictEqual(withRoomTicket('/uploads/1_ab_x.png', null), '/uploads/1_ab_x.png', 'no ticket, no change');
});
