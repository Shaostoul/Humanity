#!/usr/bin/env node
// PreToolUse hook for the Bash and PowerShell tools (wired in .claude/settings.json), 2026-10-22.
//
// The operator approved this on 2026-10-22 ("Sounds like a great idea. Do that, please."):
// the project's standing rules, which until now lived only as sentences in CLAUDE.md and were
// broken often enough to cost real work, are refused mechanically before the command runs.
//
//   1. Sweeping the shared checkout: `git add -A`, `git add .`, `git add -u`, `git commit -a`,
//      a pathless `git stash`, `git reset --hard`, `git checkout -- .`, `git restore .`.
//      Two or three Claude sessions share C:\Humanity; a sweep took another session's work
//      into the wrong commit three times on 2026-07-30. A worktree under .claude/worktrees is
//      one agent's alone, so these are allowed there. A sweep with explicit paths
//      (`git add -A -- app/web`) is not a sweep and is allowed everywhere.
//   2. `cargo fmt` and `rustfmt` in any form: the repo is not rustfmt-clean and a whole-crate
//      fmt rewrites ~240 files and breaks the theme lint.
//   3. PowerShell writing a repo file (Set-Content, Add-Content, Out-File, `>`): PowerShell 5.1
//      reads BOM-less UTF-8 as Windows-1252 and writes a byte-order mark back, garbling every
//      non-ASCII character. Writes into the temp or scratchpad folders are allowed.
//   4. Backticks inside a double-quoted string in a Bash command, or in an unquoted heredoc:
//      bash runs them as commands. Markdown in a `node -e "..."` string ran `just launch-bg`
//      and emptied text in PRIORITIES.md twice on 2026-10-22.
//
// Exit 2 refuses the command and shows the reason to the agent; exit 0 lets it run. Any error
// inside this script lets the command run (fail open): a broken guard must never stop work.
// Test it with `node scripts/tests/tool-guard.test.js`.
'use strict';

const path = require('path');

/** The shell separators that end one simple command. */
const SPLIT = /\|\||&&|;|\||\n/;

/** True when `dir` is inside an agent worktree (one agent's alone) rather than the shared checkout. */
function inWorktree(dir) {
  return /[\\/]\.claude[\\/]worktrees[\\/]/i.test(String(dir || '') + path.sep);
}

/** The folder a git command acts on: `git -C <dir>`, else a leading `cd <dir>`, else the hook's cwd. */
function gitDir(segment, fullCommand, cwd) {
  const c = segment.match(/\bgit\s+-C\s+("([^"]+)"|'([^']+)'|(\S+))/);
  if (c) return c[2] || c[3] || c[4];
  const cd = fullCommand.match(/(?:^|&&|;)\s*cd\s+("([^"]+)"|'([^']+)'|(\S+))/);
  if (cd) return cd[2] || cd[3] || cd[4];
  return cwd;
}

/** Tokens of one simple command, quotes removed (good enough for flags and paths). */
function tokens(segment) {
  const out = [];
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let m;
  while ((m = re.exec(segment))) out.push(m[1] !== undefined ? m[1] : m[2] !== undefined ? m[2] : m[3]);
  return out;
}

/** The arguments after `git [-C dir] [-c k=v]... <sub>`, or null when this is not that git subcommand. */
function gitArgs(segment, sub) {
  const t = tokens(segment);
  let i = t.findIndex((x) => x === 'git' || /[\\/]git(\.exe)?$/i.test(x));
  if (i < 0) return null;
  i++;
  while (i < t.length && (t[i] === '-C' || t[i] === '-c' || t[i] === '--git-dir' || t[i] === '--work-tree')) i += 2;
  if (t[i] !== sub) return null;
  return t.slice(i + 1);
}

const WHOLE_TREE = new Set(['.', ':/', ':', '*', './', ':/*']);

function checkGit(command, cwd) {
  const problems = [];
  for (const segment of command.split(SPLIT)) {
    const dir = gitDir(segment, command, cwd);
    if (inWorktree(dir)) continue;

    const add = gitArgs(segment, 'add');
    if (add) {
      const flags = add.filter((a) => a.startsWith('-') && a !== '--');
      const paths = add.filter((a) => !a.startsWith('-'));
      const sweepFlag = flags.some((f) => /^(-A|--all|-u|--update|--no-ignore-removal)$/.test(f) || /^-[a-zA-Z]*[Au]/.test(f));
      if ((sweepFlag && paths.length === 0) || paths.some((p) => WHOLE_TREE.has(p))) {
        problems.push(
          'git add sweeps the whole shared checkout. Other sessions have work in progress here; stage your own files by name (`just mine <paths>` or `git add <paths>`).',
        );
      }
    }

    const commit = gitArgs(segment, 'commit');
    if (commit && commit.some((f) => f === '--all' || /^-[a-zA-Z]*a/.test(f) && !f.startsWith('--'))) {
      problems.push('git commit -a commits every modified file in the shared checkout, including other sessions\' work. Stage your files by name, then commit.');
    }

    const stash = gitArgs(segment, 'stash');
    if (stash) {
      const sub = stash[0] && !stash[0].startsWith('-') ? stash[0] : 'push';
      const sep = stash.indexOf('--');
      const hasPaths = sep >= 0 && stash.length > sep + 1;
      if ((sub === 'push' || sub === 'save') && !hasPaths) {
        problems.push('git stash would hide every session\'s uncommitted work in the shared checkout. Never stash here; if you must set your own change aside, copy your files somewhere first.');
      }
    }

    const reset = gitArgs(segment, 'reset');
    if (reset && reset.includes('--hard')) {
      problems.push('git reset --hard in the shared checkout destroys other sessions\' uncommitted work. Re-sync only inside your own worktree.');
    }

    for (const sub of ['checkout', 'restore']) {
      const a = gitArgs(segment, sub);
      if (a && a.some((p) => WHOLE_TREE.has(p))) {
        problems.push(`git ${sub} of the whole tree discards other sessions' work in the shared checkout. Name the files that are yours.`);
      }
    }
  }
  return problems;
}

function checkFmt(command) {
  for (const segment of command.split(SPLIT)) {
    const t = tokens(segment);
    const i = t.findIndex((x) => /(^|[\\/])cargo(\.exe)?$/i.test(x));
    if (i >= 0 && t.slice(i + 1).some((x) => x === 'fmt')) {
      return ['cargo fmt is never run in this repo: it is not rustfmt-clean, and a whole-crate fmt rewrites about 240 files and breaks theme_token_lint. Match the surrounding style by hand.'];
    }
    if (t.some((x) => /(^|[\\/])(rustfmt|cargo-fmt)(\.exe)?$/i.test(x))) {
      return ['rustfmt is never run in this repo (see CLAUDE.md, "NEVER run cargo fmt"). Match the surrounding style by hand.'];
    }
  }
  return [];
}

/** A path or variable that points at a temp or scratchpad folder, where a PowerShell write is harmless. */
const TEMP_TARGET = /\$env:TEMP|\$env:TMP|\$TEMP\b|\$TMP\b|\$\{TEMP\}|%TEMP%|\\Temp\\|\/Temp\/|\/tmp\/|scratchpad|\$null|\bNUL\b/i;

/** `powershell -Command ...` or `pwsh -File ...` run from bash: the word alone (in a message, a heredoc) is not a call. */
const POWERSHELL_CALL = /(^|[\s;&|(])(powershell|pwsh)(\.exe)?\s+-/i;

function checkPowerShellWrites(command, redirects = true) {
  const problems = [];
  // Look at each statement on its own so a temp write in one does not excuse a repo write in another.
  for (const stmt of command.split(/;|\n|\|\||&&/)) {
    if (/\b(Set-Content|Add-Content|Out-File)\b|(^|\|)\s*(sc|ac)\s/i.test(stmt) && !TEMP_TARGET.test(stmt)) {
      problems.push(
        'PowerShell Set-Content / Add-Content / Out-File would rewrite a file with Windows-1252 text and a byte-order mark (CLAUDE.md, "NEVER round-trip a repo source file through PowerShell"). Use the Edit or Write tool, or a byte-exact node script. Writes into the temp or scratchpad folder are allowed.',
      );
      break;
    }
    // `> file` or `>> file` (not 2>, not > $null), to a path that is not a temp folder.
    const redirect = redirects && stmt.match(/(^|[^0-9>])>>?\s*("[^"]+"|'[^']+'|[^\s;|]+)/);
    if (redirect &&!/^\$null$|^NUL$/i.test(redirect[2].replace(/["']/g, '')) && !TEMP_TARGET.test(redirect[2])) {
      problems.push(
        'PowerShell `>` writes UTF-16 or a byte-order-marked file, which garbles a repo file. Use the Write tool or a node script, or write into the temp or scratchpad folder.',
      );
      break;
    }
  }
  return problems;
}

/**
 * Backticks bash would execute: an unescaped backtick inside a double-quoted string, or inside
 * the body of an unquoted heredoc. Single-quoted text and quoted heredocs (<<'EOF') are literal.
 */
function checkBackticks(command) {
  const lines = command.split('\n');
  let body = null; // { tag, literal }
  const scan = [];
  const bodies = [];
  for (const line of lines) {
    if (body) {
      if (line.replace(/^\t+/, '') === body.tag) {
        body = null;
      } else if (!body.literal) {
        bodies.push(line);
      }
      continue;
    }
    scan.push(line);
    const h = line.match(/<<-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\1/);
    if (h) body = { tag: h[2], literal: h[1] !== '' };
  }

  const problems = [];
  if (bodies.some((l) => /(^|[^\\])`/.test(l))) {
    problems.push(
      'An unquoted heredoc (<<EOF) runs its backticks as commands. Quote the tag (<<\'EOF\') so the text stays literal.',
    );
  }

  const text = scan.join('\n');
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inSingle) {
      if (ch === "'") inSingle = false;
      continue;
    }
    if (ch === '\\') {
      i++;
      continue;
    }
    if (inDouble) {
      if (ch === '"') inDouble = false;
      else if (ch === '`') {
        problems.push(
          'A backtick inside a double-quoted string is run by bash as a command (this ran `just launch-bg` from markdown in a node -e string on 2026-10-22). For literal text use single quotes, a quoted heredoc (<<\'EOF\'), or the Write/Edit tool; for command substitution write $(...).',
        );
        break;
      }
      continue;
    }
    if (ch === "'") inSingle = true;
    else if (ch === '"') inDouble = true;
    else if (ch === '#' && (i === 0 || /\s/.test(text[i - 1]))) {
      // A comment runs to the end of the line.
      const nl = text.indexOf('\n', i);
      if (nl < 0) break;
      i = nl;
    }
  }
  return problems;
}

/**
 * The command without its heredoc bodies: text fed to a program (a commit message, a script) is
 * data, so a body that mentions "git commit -a" or "cargo fmt" is not a command. Bash only.
 */
function withoutHeredocBodies(command) {
  const kept = [];
  let tag = null;
  for (const line of command.split('\n')) {
    if (tag) {
      if (line.replace(/^\t+/, '') === tag) tag = null;
      continue;
    }
    kept.push(line);
    const h = line.match(/<<-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\1/);
    if (h) tag = h[2];
  }
  return kept.join('\n');
}

/** Every problem with one tool call: [] means it may run. */
function review(input) {
  const tool = input && input.tool_name;
  const command = String((input && input.tool_input && input.tool_input.command) || '');
  const cwd = (input && input.cwd) || process.cwd();
  if (!command) return [];
  const commandsOnly = tool === 'Bash' ? withoutHeredocBodies(command) : command;
  const problems = [...checkGit(commandsOnly, cwd), ...checkFmt(commandsOnly)];
  // In the PowerShell tool every write counts. From bash, only an actual powershell call does, and
  // only its cmdlets: a `>` in that command line is bash's own, which writes bytes unchanged.
  if (tool === 'PowerShell') problems.push(...checkPowerShellWrites(command, true));
  else if (POWERSHELL_CALL.test(command)) problems.push(...checkPowerShellWrites(command, false));
  if (tool === 'Bash') problems.push(...checkBackticks(command));
  return [...new Set(problems)];
}

module.exports = { review, checkGit, checkFmt, checkPowerShellWrites, checkBackticks };

if (require.main === module) {
  let raw = '';
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (d) => (raw += d));
  process.stdin.on('end', () => {
    try {
      const problems = review(JSON.parse(raw || '{}'));
      if (problems.length) {
        process.stderr.write(
          'Refused by the project tool guard (scripts/hooks/tool-guard.js):\n' + problems.map((p) => ' - ' + p).join('\n') + '\n',
        );
        process.exit(2);
      }
    } catch (e) {
      // Fail open: a broken guard must never stop work.
    }
    process.exit(0);
  });
}
