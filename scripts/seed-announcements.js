#!/usr/bin/env node
/**
 * seed-announcements.js — re-seed the #announcements channel after a
 * full-PQ fresh-schema wipe (scripts/pq-wipe.sh).
 *
 * The operator wants the Deploy-Bot app-update history kept across the
 * wipe ("I only really care about the #announcements history since that
 * shows the app update"). data/announcements_archive.json is the durable
 * repo copy (lossless export taken pre-wipe). After the relay recreates
 * the fresh schema, this regenerates:
 *   1. the `announcements` channels row (the fresh schema only auto-
 *      creates `general`, so without this the messages are orphaned and
 *      the channel never appears in the UI), and
 *   2. every archived message, with a synthesized `raw_json` that
 *      deserializes as `RelayMessage::Chat` — the relay serves history
 *      via `serde_json::from_str::<RelayMessage>(raw_json)`, so a wrong
 *      shape would be silently dropped.
 *
 * Emits one SQL transaction on stdout. Usage (on the VPS, relay STOPPED):
 *   node scripts/seed-announcements.js | sqlite3 data/relay.db
 *
 * Dependency-free (Node core only); safe to run on a fresh empty schema.
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ARCHIVE = path.join(__dirname, '..', 'data', 'announcements_archive.json');

/** SQL string literal (single-quote escaped) or NULL for null/undefined. */
function sql(v) {
  if (v === null || v === undefined) return 'NULL';
  return "'" + String(v).replace(/'/g, "''") + "'";
}

let rows;
try {
  rows = JSON.parse(fs.readFileSync(ARCHIVE, 'utf8'));
} catch (e) {
  console.error('[seed-announcements] cannot read ' + ARCHIVE + ': ' + e.message);
  process.exit(1);
}
if (!Array.isArray(rows)) {
  console.error('[seed-announcements] archive is not a JSON array');
  process.exit(1);
}

const out = [];
out.push('BEGIN;');

// 1. Recreate the announcements channel (values mirror the pre-wipe prod
//    row: read-only, pinned near the top, voice disabled). INSERT OR
//    IGNORE so it is harmless if the relay ever auto-creates it.
out.push(
  "INSERT OR IGNORE INTO channels " +
  "(id, name, description, created_by, created_at, read_only, position, voice_enabled) " +
  "VALUES ('announcements', 'announcements', 'Project updates and news', " +
  "'system', 1772591873057, 1, 2, 0);"
);

let n = 0;
for (const r of rows) {
  // Tolerate either the export column names or already-normalized keys.
  const fromKey = r.from_key != null ? r.from_key : (r.from != null ? r.from : '');
  const fromName = r.from_name != null ? r.from_name : null;
  const content = r.content != null ? r.content : '';
  const ts = Number(r.timestamp);
  if (!Number.isFinite(ts)) continue; // skip corrupt rows rather than poison the import
  const signature = r.signature != null ? r.signature : null;

  // raw_json MUST round-trip as RelayMessage::Chat (serde tag "chat").
  // Optional fields (signature/reply_to/thread_count/message_id) are
  // omitted; `channel` is required for correct routing.
  const rawObj = {
    type: 'chat',
    from: fromKey,
    from_name: fromName,
    content: content,
    timestamp: ts,
    channel: 'announcements',
  };
  const rawJson = JSON.stringify(rawObj);

  out.push(
    'INSERT INTO messages ' +
    '(msg_type, from_key, from_name, content, timestamp, signature, raw_json, channel_id) ' +
    'VALUES (' +
    "'chat', " +
    sql(fromKey) + ', ' +
    sql(fromName) + ', ' +
    sql(content) + ', ' +
    ts + ', ' +
    sql(signature) + ', ' +
    sql(rawJson) + ", " +
    "'announcements');"
  );
  n++;
}

out.push('COMMIT;');
process.stdout.write(out.join('\n') + '\n');
console.error('[seed-announcements] generated SQL for ' + n + ' messages + 1 channel');
