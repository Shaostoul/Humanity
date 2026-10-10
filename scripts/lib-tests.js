#!/usr/bin/env node
// Run the library's unit tests: `cargo test --features native --lib`, except on
// Windows, where the test program is linked WITHOUT a debug file (BUG-174).
//
// Usage:  node scripts/lib-tests.js [test-filter and test-program options]
//         just test-lib [same]
// e.g.    node scripts/lib-tests.js net::webrtc --nocapture
// Everything after the script name goes to the test program itself, the way the
// words after `--` do for `cargo test`: a name filter, --nocapture, --exact.
//
// WHY (2026-10-09): Microsoft's linker refuses to write the library test
// program's debug file (PDB) with "LNK1318: Unexpected PDB error; LIMIT (12)",
// which stopped `just verify` on Windows (Linux CI has no PDB and was never
// affected; builds of 2026-10-05 still linked). Three things were measured that
// day and did NOT help: `strip = "debuginfo"` (on MSVC rustc passes /DEBUG
// whatever strip says), line-tables-only debug info everywhere (the PDB fell
// from 1.08 GB to 487 MB and the link still failed, so it is not the file's
// size), and the object count (about 2,300 objects, far under any module cap).
// Linking with /DEBUG:NONE works, and `cargo rustc` can pass that to this one
// link without changing the build of any dependency, so nothing else rebuilds.
// The cost: a panicking test's stack trace shows addresses instead of function
// names. A failing assertion still names its file and line (that comes from the
// panic location, not the debug file). Release builds keep their PDB.
//
// Use this, not a bare `cargo test --features native --lib`, on Windows: the
// bare form links with /DEBUG again and fails the same way.

'use strict';
const { spawnSync } = require('child_process');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const passthrough = process.argv.slice(2);

function exitWith(r) {
  if (r.error) { console.error('lib-tests:', r.error.message); process.exit(1); }
  process.exit(r.status === null ? 1 : r.status);
}

if (process.platform !== 'win32') {
  exitWith(spawnSync('cargo', ['test', '--features', 'native', '--lib', '--', ...passthrough], { cwd: ROOT, stdio: 'inherit' }));
}

// Build the test program (the library compiled with --test), linked without a PDB.
const build = spawnSync(
  'cargo',
  ['rustc', '--features', 'native', '--lib', '--profile', 'test',
    '--message-format=json-render-diagnostics', '--', '-C', 'link-arg=/DEBUG:NONE'],
  { cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 1 << 30 },
);
if (build.error) exitWith(build);
// Cargo's own messages go to stdout as JSON; the program is the artifact with an executable.
let exe = null;
for (const line of (build.stdout || '').split('\n')) {
  if (!line.startsWith('{')) continue;
  try {
    const m = JSON.parse(line);
    if (m.reason === 'compiler-artifact' && m.executable && m.profile && m.profile.test) exe = m.executable;
  } catch { /* not a cargo message */ }
}
if (build.status !== 0) exitWith(build);
if (!exe) {
  console.error('lib-tests: cargo built nothing runnable (no test program in its messages)');
  process.exit(1);
}

// Run it as `cargo test` would: from the package root, with CARGO_MANIFEST_DIR set.
exitWith(spawnSync(exe, passthrough, {
  cwd: ROOT,
  stdio: 'inherit',
  env: { ...process.env, CARGO_MANIFEST_DIR: ROOT },
}));
