#!/usr/bin/env node
// check-updates.js: is a newer Rust, rustup or crate release out than what we build with?
// (operator, 2026-10-23: "What if we have a way of you getting automatically notified when an
// update lands so that you can evaluate the update's worth ... I want you to keep all components
// up to date in a reasonable not excessively zealous fashion.")
//
// It only REPORTS. Nothing is ever updated automatically, because an update can break work in
// progress and a new Rust version forces a full rebuild of every target folder. A session reads
// the row in `just brief`, reads the release notes, and decides (docs/SOP.md "Keeping components
// up to date").
//
// What it checks:
//   - Rust: the version rust-toolchain.toml pins against the newest stable (`rustup check`).
//   - rustup itself (same command).
//   - Crates: how many semver-compatible updates `cargo update --dry-run` would make, and how many
//     dependencies sit behind a newer incompatible (major) version. This needs the crate index,
//     so it runs at most once a day and is cached in %LOCALAPPDATA%\HumanityOS\updates-last.json.
//
//   node scripts/check-updates.js          one line for `just brief` (cached crate part)
//   node scripts/check-updates.js --fresh  ignore the cache
//   node scripts/check-updates.js --json   the whole result as JSON
'use strict';

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const root = path.resolve(__dirname, '..');
const cacheFile = path.join(process.env.LOCALAPPDATA || require('os').tmpdir(), 'HumanityOS', 'updates-last.json');
const DAY_MS = 24 * 3600 * 1000;

function run(cmd, args, timeoutMs) {
  const r = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', timeout: timeoutMs, windowsHide: true });
  return { ok: r.status === 0, out: `${r.stdout || ''}\n${r.stderr || ''}` };
}

function pinnedRust() {
  try {
    const m = fs.readFileSync(path.join(root, 'rust-toolchain.toml'), 'utf8').match(/^\s*channel\s*=\s*"([^"]+)"/m);
    return m ? m[1] : null;
  } catch {
    return null;
  }
}

/** Newest stable Rust and rustup from `rustup check` ("1.88.0 (...) -> 1.99.1 (...)" or "Up to date : 1.99.1"). */
function rustupCheck() {
  const r = run('rustup', ['check'], 30000);
  const line = (re) => r.out.split('\n').find((l) => re.test(l)) || '';
  const ver = (l) => {
    const to = l.match(/->\s*(\d+\.\d+\.\d+)/);
    if (to) return to[1];
    const same = l.match(/Up to date\s*:\s*(\d+\.\d+\.\d+)/i);
    return same ? same[1] : null;
  };
  const stable = line(/^stable-/);
  const self = line(/^rustup\s*-/);
  return {
    newest_stable: ver(stable),
    rustup_update: /->/.test(self) ? (self.match(/->\s*(\S+)/) || [])[1] : null,
    reachable: r.ok || /stable-/.test(r.out),
  };
}

function crateCheck() {
  const r = run('cargo', ['update', '--dry-run'], 180000);
  const compatible = (r.out.match(/^\s*(Updating|Adding|Removing|Downgrading)\s+\S+\s+v/gm) || []).length;
  const behind = r.out.match(/(\d+)\s+unchanged dependenc(?:y|ies) behind latest/);
  return { at: new Date().toISOString(), ok: r.ok, compatible, behind_latest: behind ? Number(behind[1]) : 0 };
}

function cmp(a, b) {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
  return 0;
}

function main() {
  const fresh = process.argv.includes('--fresh');
  const pin = pinnedRust();
  const rust = rustupCheck();
  let crates = null;
  try {
    crates = JSON.parse(fs.readFileSync(cacheFile, 'utf8'));
  } catch { /* none yet */ }
  if (fresh || !crates || Date.now() - Date.parse(crates.at) > DAY_MS) {
    crates = crateCheck();
    try {
      fs.mkdirSync(path.dirname(cacheFile), { recursive: true });
      fs.writeFileSync(cacheFile + '.tmp', JSON.stringify(crates, null, 2));
      fs.renameSync(cacheFile + '.tmp', cacheFile);
    } catch { /* cache is a convenience */ }
  }
  const result = { pinned_rust: pin, ...rust, crates };
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  const parts = [];
  if (!pin) parts.push('Rust is not pinned (no rust-toolchain.toml)');
  else if (!rust.newest_stable) parts.push(`Rust ${pin} pinned; newest stable unknown (offline?)`);
  else if (cmp(rust.newest_stable, pin) > 0) {
    parts.push(`NEWER RUST: ${rust.newest_stable} is out (pinned ${pin}); read https://blog.rust-lang.org and decide (docs/SOP.md "Keeping components up to date")`);
  } else parts.push(`Rust ${pin} pinned, the newest stable`);
  if (rust.rustup_update) parts.push(`rustup ${rust.rustup_update} available (rustup self update)`);
  if (crates && crates.ok) {
    const hours = Math.round((Date.now() - Date.parse(crates.at)) / 3600000);
    parts.push(`crates: ${crates.compatible} compatible update(s), ${crates.behind_latest} behind a newer major (checked ${hours} h ago)`);
  } else parts.push('crates: could not check (cargo update --dry-run failed)');
  console.log(parts.join('; '));
}

main();
