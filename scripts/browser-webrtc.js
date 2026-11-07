#!/usr/bin/env node
/**
 * The browser for `just verify-browser-webrtc`: a headless Chromium (Chrome, else Edge) on a fresh,
 * throwaway profile, driven over the DevTools protocol on a PIPE, pointed at the page the test
 * serves (src/gui/crossing_tests/webrtc_browser.rs starts this script and is the judge). It prints the
 * page's console and the browser's log lines prefixed `[page]` and `[browser]`, so a failure shows
 * what the website itself said, and when the test closes this script's stdin it closes the browser,
 * makes sure every process of it has gone, and deletes the profile.
 *
 * WHY (2026-11-07): the calls' and the device-to-device files' WebRTC moved from str0m 0.20 to 0.25
 * (commit 78072c92c), whose DTLS is a different stack (dimpl 0.9 on RustCrypto). Every other test
 * is str0m against str0m, so nothing had checked the app against a browser's DTLS (Chromium's
 * BoringSSL), SRTP and SCTP: what a person on the website calling, or sending a file to, a person
 * on the desktop app actually meets.
 *
 * THE MACHINE RULES IT KEEPS (CLAUDE.md, the operator's PC):
 *  - nothing listens beyond this computer: the DevTools protocol runs over a pipe
 *    (--remote-debugging-pipe), not a port; the page and its socket are the test's, on 127.0.0.1;
 *  - the browser is headless (--headless=new) and started hidden, so it never shows a window or
 *    takes focus;
 *  - a FRESH profile in the temp folder (hum_browser_webrtc_*, which `just clean-test-temp` also
 *    sweeps should a run be killed), never the operator's own, which holds his sign-ins; deleted
 *    afterwards;
 *  - a fake microphone and camera (--use-fake-device-for-media-stream, the browser's own beep) and
 *    the permission prompt answered by the fake UI, so no real device is ever opened; the sound out
 *    muted (--mute-audio);
 *  - no background traffic to the browser maker's services (--disable-background-networking and
 *    the rest of BROWSER_FLAGS);
 *  - every process it started is gone at the end: the browser closes itself (Browser.close), then
 *    its PID is killed with its tree if it is still there, then any process still running with
 *    this run's profile folder in its command line is killed by PID (never by name: the operator's
 *    own browser is never touched).
 * Before closing it lists the UDP sockets the browser's processes hold (Windows), for the report:
 * which addresses its WebRTC used.
 *
 * Usage (the test runs it; by hand only to look at the page):
 *   node scripts/browser-webrtc.js --url <page url> [--downloads <dir>] [--browser <exe>]
 * HOS_BROWSER names the browser exe to use instead of the installed Chrome or Edge.
 * Tested by scripts/tests/browser-webrtc.test.js (the flags, the browser chosen, the pipe's framing).
 */
'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn, execFileSync } = require('node:child_process');

/** Where Chrome and Edge install on Windows, in the order tried. */
const WINDOWS_BROWSERS = [
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
];
const OTHER_BROWSERS = ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'];

/** The temp-folder prefix of a run's profile (clean-test-temp.js sweeps `hum_`). */
const PROFILE_PREFIX = 'hum_browser_webrtc_';

/** The flags every run uses; see the header for why each is there. */
const BROWSER_FLAGS = [
  '--headless=new',
  '--remote-debugging-pipe',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-background-networking',
  '--disable-component-update',
  '--disable-sync',
  '--disable-default-apps',
  '--disable-extensions',
  '--disable-domain-reliability',
  '--no-pings',
  '--mute-audio',
  '--use-fake-device-for-media-stream',
  '--use-fake-ui-for-media-stream',
  '--autoplay-policy=no-user-gesture-required',
  // The forwarder this check runs is on 127.0.0.1, and Chromium leaves the loopback network out of
  // WebRTC by default: its TURN client then talks only from sockets on the computer's network
  // addresses, which Windows does not let reach 127.0.0.1, so no relayed address ever came (the
  // first runs, 2026-11-07: "candidates [], gathering complete"). With loopback let in, it also asks
  // the forwarder from 127.0.0.1. The product's forwarder is the server's public address, which
  // the network addresses reach, so this changes nothing the check is about.
  '--allow-loopback-in-peer-connection',
];

/** The browser's command line for a run on `profile`. */
function browserArgs(profile) {
  return [...BROWSER_FLAGS, `--user-data-dir=${profile}`, 'about:blank'];
}

/** The browser to run: HOS_BROWSER, else the first installed of Chrome and Edge. */
function findBrowser(env = process.env, exists = fs.existsSync, platform = process.platform) {
  if (env.HOS_BROWSER) return exists(env.HOS_BROWSER) ? env.HOS_BROWSER : null;
  const list = platform === 'win32' ? WINDOWS_BROWSERS : OTHER_BROWSERS;
  return list.find((p) => exists(p)) || null;
}

/**
 * The DevTools protocol's pipe framing: each message is JSON ended by a NUL byte. Feed it the bytes
 * as they come; it calls `onMessage` with each whole message, keeping a partial one for the next.
 */
function frameReader(onMessage) {
  let pending = Buffer.alloc(0);
  return (chunk) => {
    pending = pending.length ? Buffer.concat([pending, chunk]) : chunk;
    let at;
    while ((at = pending.indexOf(0)) >= 0) {
      const text = pending.subarray(0, at).toString('utf8');
      pending = pending.subarray(at + 1);
      if (text) onMessage(JSON.parse(text));
    }
  };
}

/** A console call's arguments as one line. */
function consoleLine(args) {
  return (args || []).map((a) => (a.value !== undefined ? (typeof a.value === 'string' ? a.value : JSON.stringify(a.value)) : a.description || a.type)).join(' ');
}

/** The PIDs of the processes whose command line holds `needle` (Windows; [] elsewhere or on error). */
function pidsWith(needle) {
  if (process.platform !== 'win32') return [];
  try {
    const ps = `Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and $_.CommandLine.Contains('${needle.replace(/'/g, "''")}') } | ForEach-Object { $_.ProcessId }`;
    const out = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
    return out.split(/\r?\n/).map((s) => Number(s.trim())).filter((n) => Number.isInteger(n) && n > 0 && n !== process.pid);
  } catch {
    return [];
  }
}

/** The UDP sockets `pids` hold, from netstat (Windows), e.g. ["127.0.0.1:53121", ...]. */
function udpSocketsOf(pids) {
  if (process.platform !== 'win32' || !pids.length) return [];
  try {
    const out = execFileSync('netstat', ['-ano', '-p', 'UDP'], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
    const want = new Set(pids.map(String));
    const v6 = execFileSync('netstat', ['-ano', '-p', 'UDPv6'], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
    return (out + '\n' + v6).split(/\r?\n/)
      .map((l) => l.trim().split(/\s+/))
      .filter((f) => f[0] === 'UDP' && want.has(f[f.length - 1]))
      .map((f) => f[1]);
  } catch {
    return [];
  }
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function parseArgs(argv) {
  const opts = { url: null, downloads: null, browser: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--url') opts.url = argv[++i];
    else if (a === '--downloads') opts.downloads = argv[++i];
    else if (a === '--browser') opts.browser = argv[++i];
    else throw new Error(`browser-webrtc: unknown argument ${a}`);
  }
  if (!opts.url) throw new Error('browser-webrtc: --url is needed');
  return opts;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const exe = opts.browser || findBrowser();
  if (!exe) {
    console.log('[browser] FAILED: no Chromium browser found (Chrome or Edge); set HOS_BROWSER to one');
    process.exit(3);
  }
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), `${PROFILE_PREFIX}${process.pid}_`));
  console.log(`[browser] ${path.basename(exe)}, profile ${profile}`);
  const child = spawn(exe, browserArgs(profile), { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'], windowsHide: true });
  let exited = false;
  child.on('exit', () => { exited = true; });
  child.stderr.on('data', () => {}); // the browser's own chatter: not wanted, but must be read

  let nextId = 1;
  const waiting = new Map();
  const toBrowser = child.stdio[3];
  // A pipe the browser closed (it exited) must not crash this script before it has cleaned up.
  toBrowser.on('error', () => {});
  child.stdio[4].on('error', () => {});
  function cdp(method, params = {}, sessionId) {
    const id = nextId++;
    const msg = { id, method, params };
    if (sessionId) msg.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      waiting.set(id, { resolve, reject, method });
      toBrowser.write(JSON.stringify(msg) + '\0');
    });
  }
  child.stdio[4].on('data', frameReader((msg) => {
    if (msg.id && waiting.has(msg.id)) {
      const w = waiting.get(msg.id);
      waiting.delete(msg.id);
      if (msg.error) w.reject(new Error(`${w.method}: ${msg.error.message}`));
      else w.resolve(msg.result);
      return;
    }
    const p = msg.params || {};
    if (msg.method === 'Runtime.consoleAPICalled') console.log(`[page] ${p.type}: ${consoleLine(p.args)}`);
    else if (msg.method === 'Runtime.exceptionThrown') {
      const d = p.exceptionDetails || {};
      console.log(`[page] EXCEPTION: ${(d.exception && d.exception.description) || d.text} (${d.url || ''}:${d.lineNumber || 0})`);
    } else if (msg.method === 'Log.entryAdded') console.log(`[browser] ${p.entry.level}: ${p.entry.text}${p.entry.url ? ` (${p.entry.url})` : ''}`);
    else if (msg.method === 'Browser.downloadProgress' && p.state !== 'inProgress') console.log(`[browser] download ${p.state}: ${p.receivedBytes} bytes`);
  }));

  let closing = false;
  async function finish(code) {
    if (closing) return;
    closing = true;
    const pids = pidsWith(profile);
    const udp = udpSocketsOf(pids);
    console.log(`[browser] UDP sockets the browser held: ${udp.length ? udp.join(', ') : '(none listed)'}`);
    try {
      await Promise.race([cdp('Browser.close'), sleep(3000)]);
    } catch { /* already going */ }
    for (let i = 0; i < 50 && !exited; i++) await sleep(100);
    if (!exited) {
      console.log(`[browser] did not close in 5 s: killing PID ${child.pid} and its tree`);
      try {
        if (process.platform === 'win32') execFileSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
        else child.kill('SIGKILL');
      } catch { /* gone meanwhile */ }
    }
    // Anything still running on this run's profile is this run's (a helper process that outlived
    // the browser): killed by PID. The operator's own browser never has this folder in its command line.
    for (let round = 0; round < 3; round++) {
      const left = pidsWith(profile);
      if (!left.length) break;
      console.log(`[browser] killing leftover process(es) of this run: ${left.join(', ')}`);
      for (const pid of left) {
        try { execFileSync('taskkill', ['/PID', String(pid), '/F'], { stdio: 'ignore', windowsHide: true }); } catch { /* gone */ }
      }
      await sleep(500);
    }
    for (let i = 0; i < 20; i++) {
      try { fs.rmSync(profile, { recursive: true, force: true }); break; } catch { await sleep(250); }
    }
    console.log(`[browser] closed; profile ${fs.existsSync(profile) ? 'NOT removed: ' + profile : 'removed'}`);
    process.exit(code);
  }

  // The test closes stdin when it is done (or dies); a lifetime cap covers a test that hangs.
  process.stdin.on('data', () => {});
  process.stdin.on('end', () => finish(0));
  process.stdin.on('error', () => finish(0));
  setTimeout(() => { console.log('[browser] FAILED: still open after 10 minutes'); finish(4); }, 10 * 60 * 1000).unref();
  child.on('exit', (c) => { if (!closing) { console.log(`[browser] FAILED: the browser exited by itself (${c})`); finish(5); } });

  try {
    if (opts.downloads) {
      fs.mkdirSync(opts.downloads, { recursive: true });
      await cdp('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: opts.downloads, eventsEnabled: true });
    }
    const { targetInfos } = await cdp('Target.getTargets');
    const page = targetInfos.find((t) => t.type === 'page');
    if (!page) throw new Error('the browser opened no page');
    const { sessionId } = await cdp('Target.attachToTarget', { targetId: page.targetId, flatten: true });
    await cdp('Runtime.enable', {}, sessionId);
    await cdp('Log.enable', {}, sessionId);
    await cdp('Page.enable', {}, sessionId);
    const nav = await cdp('Page.navigate', { url: opts.url }, sessionId);
    if (nav.errorText) throw new Error(`the page did not load: ${nav.errorText}`);
    console.log(`[browser] opened ${opts.url}`);
  } catch (e) {
    console.log(`[browser] FAILED: ${e.message}`);
    finish(6);
  }
}

if (require.main === module) {
  main().catch((e) => {
    console.log(`[browser] FAILED: ${e && e.stack ? e.stack : e}`);
    process.exit(7);
  });
}

module.exports = { BROWSER_FLAGS, PROFILE_PREFIX, WINDOWS_BROWSERS, browserArgs, findBrowser, frameReader, consoleLine, parseArgs };
