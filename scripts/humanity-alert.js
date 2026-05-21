#!/usr/bin/env node
/*
 * humanity-alert.js -- multi-channel external alert fanout.
 * =====================================================================
 * WHY: the relay watchdog + disk-guard can detect trouble, but until now
 * they could only post to #announcements (in-app), which is useless when
 * the relay itself is down or the admin isn't watching chat. This script
 * fans a single alert message out to every alert channel the server
 * admin has configured -- ntfy (phone push), Discord, Telegram, or any
 * generic webhook (Slack, custom, etc.) -- so the admin actually hears
 * about it.
 *
 * CONFIGURABLE PER SERVER ADMIN: each self-hosted relay has its own
 * data/alert-channels.secrets.json (gitignored -- it holds tokens/URLs).
 * Copy data/alert-channels.example.json to that name and fill in the
 * channels you want. No channels configured = this script is a silent
 * no-op (safe default; nothing breaks).
 *
 * USAGE:
 *   node humanity-alert.js "<message>" [severity]
 *     severity: info | warn | critical   (default: warn)
 *
 * ENV:
 *   HUMANITY_ALERT_CONFIG   override config path
 *   HUMANITY_ALERT_DRYRUN=1 print what WOULD be sent, send nothing
 *
 * Dependency-free (node core only: https/http/fs). Best-effort: a failing
 * channel never blocks the others, and the script always exits 0 so a
 * caller (watchdog/disk-guard) is never wedged by an alerting hiccup.
 * =====================================================================
 */
'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');
const { URL } = require('url');

const DRYRUN = process.env.HUMANITY_ALERT_DRYRUN === '1';
const message = process.argv[2];
const severity = (process.argv[3] || 'warn').toLowerCase();

if (!message) {
  console.error('usage: node humanity-alert.js "<message>" [info|warn|critical]');
  process.exit(0); // exit 0 -- never wedge the caller
}

// Resolve config path: env override, else data/alert-channels.secrets.json
// next to this script's repo.
const configPath = process.env.HUMANITY_ALERT_CONFIG
  || path.join(__dirname, '..', 'data', 'alert-channels.secrets.json');

let channels = [];
try {
  const raw = fs.readFileSync(configPath, 'utf8');
  const parsed = JSON.parse(raw);
  channels = Array.isArray(parsed) ? parsed : (parsed.channels || []);
} catch (e) {
  // No config (or unreadable) = nothing to do. This is the safe default
  // for a fresh relay: alerting is opt-in, absence is not an error.
  if (e.code !== 'ENOENT') {
    console.error(`humanity-alert: config at ${configPath} unreadable: ${e.message}`);
  }
  process.exit(0);
}

const enabled = channels.filter(c => c && c.enabled !== false && c.type);
if (enabled.length === 0) {
  // Configured but nothing enabled -> silent no-op.
  process.exit(0);
}

// Severity decoration (used by channels that support it, e.g. ntfy).
const sevPrefix = { info: '', warn: '[WARN] ', critical: '[CRITICAL] ' }[severity] || '';
const text = `${sevPrefix}${message}`;

// --- HTTP helper: POST (or GET) a request, resolve with status code. ---
function send(urlStr, { method = 'POST', headers = {}, body = null } = {}) {
  return new Promise((resolve) => {
    let u;
    try { u = new URL(urlStr); } catch (e) { return resolve({ ok: false, err: 'bad url' }); }
    const lib = u.protocol === 'http:' ? http : https;
    const req = lib.request(u, { method, headers, timeout: 10000 }, (res) => {
      // Drain the response so the socket frees; we only care about status.
      res.on('data', () => {});
      res.on('end', () => resolve({ ok: res.statusCode >= 200 && res.statusCode < 300, status: res.statusCode }));
    });
    req.on('error', (e) => resolve({ ok: false, err: e.message }));
    req.on('timeout', () => { req.destroy(); resolve({ ok: false, err: 'timeout' }); });
    if (body) req.write(body);
    req.end();
  });
}

// --- Per-channel-type senders. Each returns a Promise<result>. ---
async function deliver(ch) {
  const tag = ch.label ? `${ch.type}(${ch.label})` : ch.type;
  try {
    switch (ch.type) {
      case 'ntfy': {
        // ntfy: POST the message body to the topic URL. Title + priority
        // via headers. topic can be "https://ntfy.sh/mytopic" or a
        // self-hosted base+topic.
        if (!ch.topic) return { tag, ok: false, err: 'missing topic' };
        const headers = { 'Content-Type': 'text/plain' };
        if (ch.title) headers['Title'] = ch.title;
        if (severity === 'critical') { headers['Priority'] = 'urgent'; headers['Tags'] = 'rotating_light'; }
        else if (severity === 'warn') { headers['Priority'] = 'high'; }
        if (DRYRUN) return { tag, ok: true, dry: `POST ${ch.topic} <- "${text}"` };
        return { tag, ...(await send(ch.topic, { method: 'POST', headers, body: text })) };
      }
      case 'discord': {
        // Discord incoming webhook: JSON {content}.
        if (!ch.webhook_url) return { tag, ok: false, err: 'missing webhook_url' };
        const body = JSON.stringify({ content: text.slice(0, 1900) });
        if (DRYRUN) return { tag, ok: true, dry: `POST discord webhook <- "${text}"` };
        return { tag, ...(await send(ch.webhook_url, { headers: { 'Content-Type': 'application/json' }, body })) };
      }
      case 'slack':
      case 'webhook': {
        // Slack incoming webhook + generic JSON webhook both accept {text}.
        // A generic receiver can also read "message"/"content"; send all three.
        if (!ch.url && !ch.webhook_url) return { tag, ok: false, err: 'missing url' };
        const url = ch.url || ch.webhook_url;
        const body = JSON.stringify({ text, message: text, content: text, severity });
        if (DRYRUN) return { tag, ok: true, dry: `POST ${url} <- ${body}` };
        return { tag, ...(await send(url, { method: ch.method || 'POST', headers: { 'Content-Type': 'application/json' }, body })) };
      }
      case 'telegram': {
        // Telegram Bot API sendMessage.
        if (!ch.bot_token || !ch.chat_id) return { tag, ok: false, err: 'missing bot_token/chat_id' };
        const url = `https://api.telegram.org/bot${ch.bot_token}/sendMessage`;
        const body = JSON.stringify({ chat_id: ch.chat_id, text });
        if (DRYRUN) return { tag, ok: true, dry: `POST telegram sendMessage chat=${ch.chat_id} <- "${text}"` };
        return { tag, ...(await send(url, { headers: { 'Content-Type': 'application/json' }, body })) };
      }
      default:
        return { tag, ok: false, err: `unknown channel type '${ch.type}'` };
    }
  } catch (e) {
    return { tag, ok: false, err: e.message };
  }
}

(async () => {
  const results = await Promise.all(enabled.map(deliver));
  for (const r of results) {
    if (r.dry) console.log(`humanity-alert DRYRUN ${r.tag}: ${r.dry}`);
    else if (r.ok) console.log(`humanity-alert ${r.tag}: sent (status ${r.status})`);
    else console.error(`humanity-alert ${r.tag}: FAILED (${r.err || 'status ' + r.status})`);
  }
  // Always exit 0 -- alerting is best-effort, never fatal to the caller.
  process.exit(0);
})();
