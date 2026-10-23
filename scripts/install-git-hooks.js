#!/usr/bin/env node
/**
 * Install GIT pre-commit + pre-push hooks (distinct from scripts/install-hooks.js,
 * which installs CLAUDE CODE hooks). Node-based, not a `just` shebang recipe, because
 * shebang recipes need `cygpath` -- absent on the Windows dev host, so the old shebang
 * installer silently did nothing here. The HOOK BODIES are bash; git-for-windows runs
 * hooks through its bundled sh, so bash is correct on every platform.
 *
 * The hooks turn the documented recurring CI gotchas into fast, self-explaining local
 * gates. CI (.github/workflows/verify.yml) is the unconditional backstop; these are the
 * fast feedback layer. Bypass in a real emergency: `git commit --no-verify` /
 * `git push --no-verify`.
 *
 *   node scripts/install-git-hooks.js      (or: just install-hooks)
 *
 * Set GIT_HOOKS_DIR to write elsewhere (used to validate the generated hooks without
 * installing them).
 *
 * The folder comes from `git rev-parse --git-path hooks` (2026-10-19): it honours
 * core.hooksPath, and in a worktree, where `.git` is a FILE pointing at the main
 * repository, it names the shared hooks folder. The old `<checkout>/.git/hooks` guess
 * failed in every worktree.
 */
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

function gitHooksDir() {
  const root = path.join(__dirname, '..');
  try {
    const p = execFileSync('git', ['rev-parse', '--git-path', 'hooks'], { cwd: root }).toString().trim();
    return path.resolve(root, p);
  } catch {
    return path.join(root, '.git', 'hooks');
  }
}

const hooksDir = process.env.GIT_HOOKS_DIR || gitHooksDir();
if (!fs.existsSync(hooksDir)) {
  fs.mkdirSync(hooksDir, { recursive: true });
}

const preCommit = `#!/usr/bin/env bash
# Auto-installed by: node scripts/install-git-hooks.js (just install-hooks)
# 1. Stale-worktree dead paths (context rot): the unified binary deleted these.
if git diff --cached --name-only | grep -qE '^(native/src|server/src|crates)/'; then
  echo "x Staged edits under native/src | server/src | crates/ -- those paths are GONE."
  echo "  An AI agent likely found a stale worktree. Run: just clean-worktrees, redo vs src/."
  exit 1
fi
# 1b. Text garbled by a PowerShell round trip (Get-Content/Set-Content, > or Out-File):
#     a byte-order mark the file did not have, UTF-16, or Windows-1252 mojibake in an
#     added line. One git diff, well under a second. It prints its own fix command.
if ! node scripts/check-encoding.js --staged; then
  exit 1
fi
# 2. cargo-fmt blast radius: a real change never touches 40+ .rs files at once.
RSN=$(git diff --cached --name-only -- '*.rs' | wc -l | tr -d ' ')
if [ "\${RSN:-0}" -gt 40 ]; then
  echo "x \${RSN} .rs files staged -- looks like a whole-crate cargo fmt (BANNED, Incident v0.390)."
  echo "  Revert the fmt-only churn. (bypass: git commit --no-verify)"
  exit 1
fi
# 3. Relay build = the feature set CI deploys with; an ungated native module breaks it
#    (kept Deploy red for 25 releases). The load-bearing check.
echo "-> pre-commit: cargo check (relay)..."
if ! cargo check --features relay --no-default-features -q 2>&1; then
  echo "x Relay build failed (likely an ungated native module). Fix before committing."
  exit 1
fi
# 4. The src/gui + engine lints (standalone rustc; no native link, PDB-safe, fast), and
#    the two that guard startup: the schema ALTER order (BUG-046: an index before its ALTER
#    stopped the live relay) and the bind group entry counts (v0.1029-1038: a lazy bind
#    group one binding short panicked every world entry).
#    Through scripts/run-lints.js (2026-10-23): each compiled once per content and cached in
#    target/lint-cache/, run four at a time, every failure reported.
node scripts/run-lints.js emdash_lint theme_token_lint theme_editor_coverage icon_glyph_lint engine_wiring_lint schema_alter_order_lint bind_group_entry_lint || { echo "x lint failed (above)"; exit 1; }
echo "ok pre-commit"
`;

const prePush = `#!/usr/bin/env bash
# Auto-installed by: node scripts/install-git-hooks.js (just install-hooks)
# Untracked source compiles locally but FAILS a fresh CI checkout (a committed
# 'mod x;' referencing an un-added x.rs). Hard-stop before push.
UNTRACKED=$(git ls-files --others --exclude-standard -- '*.rs' '*.ron' '*.csv')
if [ -n "\${UNTRACKED}" ]; then
  echo "x Untracked source files would fail a fresh CI checkout -- git add them first:"
  echo "\${UNTRACKED}" | sed 's/^/    /'
  echo "  (bypass: git push --no-verify)"
  exit 1
fi
`;

function writeHook(name, body) {
  const p = path.join(hooksDir, name);
  fs.writeFileSync(p, body, { mode: 0o755 });
  try { fs.chmodSync(p, 0o755); } catch { /* chmod is a no-op on Windows */ }
  console.log('ok wrote ' + path.relative(path.join(__dirname, '..'), p));
}

writeHook('pre-commit', preCommit);
writeHook('pre-push', prePush);
console.log('Git hooks installed. Bypass with --no-verify in an emergency.');
console.log('CI (.github/workflows/verify.yml) is the unconditional backstop.');
