// The play journal's bridge tools (docs/design/play-journal.md, rung 1): dev_play_marks and
// dev_play_journal in scripts/mcp-server.js read a journal folder the game wrote
// (src/engine/play_journal.rs: <kind>.jsonl, one JSON line per event, marks/<n>.png).
// Run: node --test scripts/tests/play-journal-bridge.test.js
//
// A scratch journal stands in for the game's, in the game's own line format; the game side's
// format is pinned by its Rust tests (event_line). Seen red before the tools existed
// ("playJournal is exported").
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');

const bridge = require('../mcp-server.js');

function journal() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-play-journal-'));
  fs.mkdirSync(path.join(dir, 'marks'));
  const lines = (rows) => rows.map((r) => JSON.stringify(r)).join('\n') + '\n';
  fs.writeFileSync(path.join(dir, 'log.jsonl'), lines([
    { t: 1000, what: 'an early warning', level: 'WARN', module: 'net' },
    { t: 3000, what: 'after the first mark', level: 'ERROR', module: 'renderer' },
    { t: 5000, what: 'after the second mark', level: 'WARN', module: 'net' },
  ]) + '{"t": 6000, "what": "a half-wri');
  fs.writeFileSync(path.join(dir, 'mark.jsonl'), lines([
    { t: 2000, what: 'Mark 1', n: 1, picture: path.join(dir, 'marks', '1.png'), page: 'None', storey: { room: 'Kitchen' } },
    { t: 4000, what: 'Mark 2', n: 2, picture: null, page: 'Chat', storey: {} },
  ]));
  return dir;
}

const text = (r) => r.content[0].text;

test('the marks are listed with their time, page, room and picture', () => {
  assert.strictEqual(typeof bridge.playJournal, 'function', 'playJournal is exported');
  const dir = journal();
  const r = bridge.playMarks({ folder: dir });
  assert.strictEqual(r.structuredContent.marks.length, 2);
  const t = text(r);
  assert.match(t, /^2 marks in /);
  assert.match(t, /#1 1970-01-01T00:00:02\.000Z None Kitchen .*1\.png/);
  assert.match(t, /#2 1970-01-01T00:00:04\.000Z Chat \(no picture\)/);
});

test('since a mark: only what happened from it on, every kind, oldest first, with its picture', () => {
  const dir = journal();
  const r = bridge.playJournal({ folder: dir, since_mark: 1 });
  const ev = r.structuredContent.events;
  assert.deepStrictEqual(ev.map((e) => e.what), ['Mark 1', 'after the first mark', 'Mark 2', 'after the second mark'], 'the early warning and the half-written line are left out');
  assert.match(text(r), /since mark #1 \(1970-01-01T00:00:02\.000Z, picture .*1\.png\)/);
  assert.match(text(r), /log ERROR: after the first mark/);
});

test('one kind, the last few, and a mark that is not there', () => {
  const dir = journal();
  const logs = bridge.playJournal({ folder: dir, kind: 'log', last: 2 });
  assert.deepStrictEqual(logs.structuredContent.events.map((e) => e.what), ['after the first mark', 'after the second mark']);
  assert.match(text(logs), /the 1 older left out/);
  const none = bridge.playJournal({ folder: dir, since_mark: 9 });
  assert.match(text(none), /There is no mark #9/);
  const empty = bridge.playMarks({ folder: path.join(dir, 'nothing-here') });
  assert.match(text(empty), /No marks .* F8 marks a moment/);
});

test('the default folder is the operator install, beside its logs', () => {
  const was = process.env.APPDATA;
  process.env.APPDATA = path.join('C:', 'Users', 'someone', 'AppData', 'Roaming');
  try {
    assert.strictEqual(bridge.playJournalDir(), path.join(process.env.APPDATA, 'HumanityOS', 'journal'));
  } finally {
    if (was === undefined) delete process.env.APPDATA; else process.env.APPDATA = was;
  }
});

// Rung 4 (2026-10-24): the rigs mark each vantage and, when one fails, read what the game recorded
// since its mark, through the same reader (scripts/lib/play-journal.js).
test('a vantage finds its own mark and the events after it, of the kinds asked', () => {
  const PJ = require('../lib/play-journal.js');
  const dir = journal();
  fs.appendFileSync(path.join(dir, 'mark.jsonl'), JSON.stringify({ t: 4500, what: 'Mark 3: fuji-forest-ground', n: 3, label: 'fuji-forest-ground', picture: null }) + '\n');
  const mark = PJ.markFor(dir, 'fuji-forest-ground', 4000);
  assert.strictEqual(mark && mark.n, 3, 'the mark named for the vantage');
  assert.strictEqual(PJ.markFor(dir, 'fuji-forest-ground', 4600), null, 'not one from before the vantage started');
  assert.strictEqual(PJ.markFor(dir, 'ocean-storm-low', 0), null, 'another vantage has none');
  const after = PJ.eventsBetween(dir, mark.t, Infinity, ['log', 'panic']);
  assert.deepStrictEqual(after.map((e) => e.what), ['after the second mark'], 'only events from the mark on, of those kinds');
  assert.match(PJ.eventLine(after[0]), /^1970-01-01T00:00:05\.000Z log WARN: after the second mark$/);
});

test('the sweep marks each vantage and attaches the journal to a failed one', () => {
  const sweep = fs.readFileSync(path.join(__dirname, '..', 'probe-sweep.js'), 'utf8');
  assert.match(sweep, /req\("ui_request\.json", \{ action: "mark", label: v\.id, picture: false \}\)/);
  assert.match(sweep, /const mark = PJ\.markFor\(journalDir, v\.id, markSentAt - 5000\);/);
  assert.match(sweep, /if \(!rec\.ok\) \{\s*\n\s*const events = PJ\.eventsBetween\(journalDir, mark\.t, Infinity, \["log", "panic", "frame", "world", "net"\]\)/);
});

test('both tools are in the bridge tool list', () => {
  const src = fs.readFileSync(path.join(__dirname, '..', 'mcp-server.js'), 'utf8');
  assert.match(src, /name: "dev_play_journal"[\s\S]{0,1500}\}, playJournal\]/);
  assert.match(src, /name: "dev_play_marks"[\s\S]{0,600}\}, playMarks\]/);
});
