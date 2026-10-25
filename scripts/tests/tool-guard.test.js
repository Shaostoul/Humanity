// Tests for scripts/hooks/tool-guard.js (the PreToolUse guard). Run: node scripts/tests/tool-guard.test.js
'use strict';
const assert = require('assert');
const { review } = require('../hooks/tool-guard.js');
const MAIN = 'C:/Humanity';
const WT = 'C:/Humanity/.claude/worktrees/agent-x';
const bash = (command, cwd = MAIN) => review({ tool_name: 'Bash', tool_input: { command }, cwd });
const ps = (command, cwd = MAIN) => review({ tool_name: 'PowerShell', tool_input: { command }, cwd });
const refused = (r) => r.length > 0;
const cases = [
  // [description, result, expected refused?]
  ['git add -A in the shared checkout', bash('git add -A'), true],
  ['git add . in the shared checkout', bash('git add .'), true],
  ['git add -u', bash('git add -u'), true],
  ['git add --all', bash('git add --all'), true],
  ['git commit -am', bash('git commit -am "x"'), true],
  ['git commit -a -F', bash('git commit -a -F msg.txt'), true],
  ['pathless git stash', bash('git stash'), true],
  ['git stash push no paths', bash('git stash push -m wip'), true],
  ['git reset --hard in main', bash('git reset --hard origin/main'), true],
  ['git checkout -- .', bash('git checkout -- .'), true],
  ['git restore .', bash('git restore .'), true],
  ['git -C main add -A from a worktree', bash('git -C C:/Humanity add -A', WT), true],
  ['cargo fmt', bash('cargo fmt -p humanity-engine'), true],
  ['rustfmt', bash('rustfmt src/lib.rs'), true],
  ['backtick in node -e double quotes', bash('node -e "console.log(`x`)"'), true],
  ['backtick in unquoted heredoc', bash('cat <<EOF > f.md\nuse `just x`\nEOF'), true],
  ['PowerShell Set-Content to repo', ps('(Get-Content a.rs) | Set-Content a.rs'), true],
  ['PowerShell Out-File to repo', ps('"x" | Out-File docs/x.md'), true],
  ['PowerShell > to repo', ps('"x" > docs/x.md'), true],
  ['powershell from bash writing repo', bash("powershell -Command 'Set-Content -Path src/a.rs -Value x'"), true],
  // allowed
  ['git add by name', bash('git add src/a.rs docs/b.md'), false],
  ['scoped git add -A -- app/web', bash('git add -A -- app/web'), false],
  ['git add -A inside a worktree', bash('git add -A', WT), false],
  ['cd worktree && git add -A', bash('cd C:/Humanity/.claude/worktrees/agent-y && git add -A'), false],
  ['git reset --hard in a worktree', bash('git reset --hard origin/main', WT), false],
  ['git stash with paths', bash('git stash push -- src/a.rs'), false],
  ['git stash list', bash('git stash list'), false],
  ['git commit -F', bash('git commit -F msg.txt'), false],
  ['git checkout a file', bash('git checkout -- src/a.rs'), false],
  ['cargo build', bash('cargo build --features native'), false],
  ['backtick in single quotes', bash("node -e 'console.log(`x`)'"), false],
  ['backtick in quoted heredoc', bash("cat <<'EOF' > f.md\nuse `just x`\nEOF"), false],
  ['escaped backtick in double quotes', bash('echo "a \\`b\\`"'), false],
  ['$() in double quotes', bash('echo "$(date)"'), false],
  ['backtick in a comment', bash('ls # see `x`'), false],
  ['PowerShell 2>$null', ps('git status 2>$null'), false],
  ['PowerShell > $null', ps('Remove-Item x > $null'), false],
  ['PowerShell Set-Content to scratchpad', ps('Set-Content -Path "$env:TEMP/x.txt" -Value 1'), false],
  ['PowerShell Get-Content read only', ps('Get-Content a.txt -TotalCount 5'), false],
  ['empty', bash(''), false],
  ['commit message heredoc that mentions the banned commands', bash("cat > m.txt <<'EOF'\nrefuses git commit -a and cargo fmt\nEOF\ngit commit -F m.txt"), false],
  ['a real sweep after a heredoc is still refused', bash("cat > m.txt <<'EOF'\ntext\nEOF\ngit add -A"), true],
  ['the word PowerShell in a heredoc, bash > to temp', bash("cat > \"$TEMP/j.js\" <<'EOF'\nPowerShell 5.1 notes\nEOF"), false],
  ['bash > to a repo file is bash, not PowerShell', bash('echo hi > docs/x.md'), false],
  // A separator inside quotes is part of the argument (2026-10-25: a grep for the banned commands was refused).
  ['a quoted grep pattern naming git add -A', bash('grep -n -i "cargo fmt\\|git add -A\\|git add \\.\\|seed phrase" CONTRIBUTING.md'), false],
  ['a quoted pattern naming cargo fmt with a pipe', bash("grep -n 'x|cargo fmt' a.md"), false],
  ['a real sweep after a quoted pipe is still refused', bash('grep "a|b" x.txt && git add -A'), true],
  ['a real sweep after an unquoted pipe is still refused', bash('echo x | git add -A'), true],
  ['a real sweep after a semicolon in single quotes is still refused', bash("echo 'a;b'; git add ."), true],
  ['powershell call with a bash redirect of its output', bash("powershell -NoProfile -Command 'Get-Date' > out.txt"), false],
];
let failed = 0;
for (const [name, result, expected] of cases) {
  const ok = refused(result) === expected;
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${ok ? '' : ` (expected ${expected ? 'refused' : 'allowed'}, got ${JSON.stringify(result)})`}`);
}
console.log(failed ? `${failed} failed` : `all ${cases.length} passed`);
process.exit(failed ? 1 : 0);
