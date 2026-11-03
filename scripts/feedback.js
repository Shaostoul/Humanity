#!/usr/bin/env node
/**
 * feedback.js: the newest lines of #feedback, read the way any visitor reads them (2026-11-02,
 * docs/design/chat-channels.md section 3). The operator: "Then we can make a #feedback channel
 * that people can talk in directly and I can say check that channel to you."
 *
 *   just feedback            the newest 20 lines from https://united-humanity.us
 *   just feedback 50         the newest 50 (the server answers at most 200)
 *   node scripts/feedback.js 20 https://public.guide     another server
 *
 * #feedback reads at Anyone, so this needs no key: GET /api/channels says whether the server has a
 * #feedback anyone may read, and GET /api/messages?channel=feedback gives its lines since it
 * opened, names only (src/relay/chat_access.rs). Lines from linked servers say which one. Times
 * are this computer's. Node core only.
 */
'use strict';

const DEFAULT_SERVER = 'https://united-humanity.us';
const CHANNEL = 'feedback';

/** "2026-11-02 21:14", in this computer's time zone. */
function when(ms) {
  const d = new Date(Number(ms) || 0);
  const two = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** One line of the answer as printed: its time, who said it (and from which linked server), the words. */
function lineOf(m) {
  const who = m.from_name || 'someone';
  const where = m.type === 'federated_chat' && m.server_name ? ` (${m.server_name})` : '';
  return `${when(m.timestamp)}  ${who}${where}: ${String(m.content || '').replace(/\s*\n\s*/g, ' / ')}`;
}

/**
 * Everything to print, as lines: the channel's newest `count` lines, or why there are none. `get`
 * is `(url) => Promise<{ status, body }>`, so the tests can answer for the server.
 */
async function report(server, count, get) {
  const base = server.replace(/\/+$/, '');
  const channels = await get(`${base}/api/channels`);
  if (channels.status !== 200) {
    return [`${base} did not list its channels (HTTP ${channels.status}).`];
  }
  const listed = (channels.body && channels.body.channels) || [];
  if (!listed.some((c) => c.id === CHANNEL)) {
    return [`${base} has no #feedback that anyone can read yet. A server makes it at its first start with the code that adds it (relay/default_channels.rs); until then there is nothing to read.`];
  }
  const res = await get(`${base}/api/messages?channel=${CHANNEL}&limit=${count}`);
  if (res.status !== 200) {
    const why = res.body && res.body.error ? `: ${res.body.error}` : '';
    return [`${base} refused #feedback's lines (HTTP ${res.status})${why}`];
  }
  const lines = ((res.body && res.body.messages) || []).filter((m) => m.type === 'chat' || m.type === 'federated_chat');
  if (!lines.length) return [`#feedback on ${base} has no lines yet.`];
  return [`#feedback on ${base}, the newest ${lines.length}:`, ...lines.map(lineOf)];
}

async function httpGet(url) {
  const res = await fetch(url, { headers: { accept: 'application/json' } });
  let body = null;
  try { body = await res.json(); } catch (_) { /* a page that is not JSON */ }
  return { status: res.status, body };
}

async function main() {
  const count = Math.max(1, Math.min(200, parseInt(process.argv[2] || '20', 10) || 20));
  const server = process.argv[3] || DEFAULT_SERVER;
  try {
    for (const line of await report(server, count, httpGet)) console.log(line);
  } catch (e) {
    console.log(`Could not reach ${server}: ${e.message}`);
    process.exitCode = 1;
  }
}

if (require.main === module) main();

module.exports = { report, lineOf, when };
