#!/usr/bin/env node
// PostToolUse hook for Edit and Write (wired in .claude/settings.json), 2026-10-22, approved by
// the operator with the tool guard. After a file is edited it checks that file for the damage a
// PowerShell round trip does (a byte-order mark it did not have, or mojibake), using the same
// checker as the pre-commit and `just lints` gate (scripts/check-encoding.js). Exit 2 tells the
// agent at once, while the edit is fresh, rather than at commit time. Fails open on any error.
'use strict';

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const { checkFile } = require('../check-encoding.js');

function headBytes(file) {
  try {
    const dir = path.dirname(file);
    const root = execFileSync('git', ['-C', dir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    const rel = path.relative(root, file).split(path.sep).join('/');
    return execFileSync('git', ['-C', root, 'show', `HEAD:${rel}`], { stdio: ['ignore', 'pipe', 'ignore'], maxBuffer: 64 * 1024 * 1024 });
  } catch {
    return null; // untracked or outside git: a new file
  }
}

let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (d) => (raw += d));
process.stdin.on('end', () => {
  try {
    const input = JSON.parse(raw || '{}');
    const file = input.tool_input && input.tool_input.file_path;
    if (!file || !fs.existsSync(file)) process.exit(0);
    const buf = fs.readFileSync(file);
    if (buf.length > 32 * 1024 * 1024) process.exit(0);
    const problems = checkFile(file, buf, headBytes(file), null);
    if (problems.length) {
      const lines = problems.slice(0, 10).map((p) =>
        typeof p === 'string' ? ` - ${p}` : ` - line ${p.line}, column ${p.col}: "${p.seq}" is garbled "${p.original}"`,
      );
      process.stderr.write(
        `Encoding problem in ${file} after this edit (scripts/hooks/post-edit-encoding.js):\n${lines.join('\n')}\n` +
          `Repair it with: node scripts/check-encoding.js --fix "${file}"\n`,
      );
      process.exit(2);
    }
  } catch {
    // Fail open.
  }
  process.exit(0);
});
