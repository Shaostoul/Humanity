#!/usr/bin/env node
// PreToolUse hook (matcher "Bash", filtered to git-push via the hook's `if`)
// — wired in .claude/settings.json.
//
// Before a `git push`, WARN (never block) if there are UNTRACKED .rs/.ron/.csv
// files. CI builds from a fresh checkout and fails when code/data files were
// committed-but-never-`git add`ed (CLAUDE.md item 12). Advisory only — it
// prints a heads-up and lets the push proceed. Reads tool_input.command from
// stdin. Pure Node stdlib.
let input = '';
process.stdin.on('data', (d) => (input += d));
process.stdin.on('end', () => {
  try {
    const j = JSON.parse(input || '{}');
    const cmd = (j.tool_input && j.tool_input.command) || '';
    if (!/\bgit\s+push\b/.test(cmd)) return; // belt-and-suspenders (the hook `if` also filters)
    const out = require('child_process').execSync('git status --porcelain', { encoding: 'utf8' });
    const untracked = out
      .split('\n')
      .filter((l) => l.startsWith('??') && /\.(rs|ron|csv)$/.test(l.trim()))
      .map((l) => l.slice(3).trim());
    if (untracked.length) {
      process.stdout.write(JSON.stringify({
        systemMessage:
          'PUSH GUARD: untracked code/data files are NOT staged — CI builds from a fresh checkout and will fail without them: ' +
          untracked.join(', ') +
          '. Stage them (git add) before this push, or confirm they are intentionally ignored.',
      }));
    }
  } catch (e) {
    // Advisory only — never block a push because the check itself errored.
  }
});
