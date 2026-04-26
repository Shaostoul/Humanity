#!/usr/bin/env node
/**
 * Aggregate agent_sessions JSON files into a single coordinator-friendly
 * status report. Reads `data/coordination/sessions/*.json` and produces
 * either a Markdown summary (default) or raw JSON (--json flag).
 *
 * Usage:
 *   node scripts/agent-status.js          # markdown summary
 *   node scripts/agent-status.js --json   # raw JSON
 *   node scripts/agent-status.js --scope pq-substrate   # single scope
 *
 * Lets a freshly-spun-up Claude Code session check the global state of
 * every audited scope without re-running the audit. Pair with
 * `data/coordination/agent_registry.ron` for the canonical scope list.
 */

const fs = require('fs');
const path = require('path');

const SESSIONS_DIR = path.join(__dirname, '..', 'data', 'coordination', 'sessions');
const REGISTRY_PATH = path.join(__dirname, '..', 'data', 'coordination', 'agent_registry.ron');

function readSessions() {
  if (!fs.existsSync(SESSIONS_DIR)) {
    return [];
  }
  return fs.readdirSync(SESSIONS_DIR)
    .filter(f => f.endsWith('.json'))
    .map(f => {
      const raw = fs.readFileSync(path.join(SESSIONS_DIR, f), 'utf8');
      try {
        return JSON.parse(raw);
      } catch (e) {
        return { scope_id: f.replace(/\.json$/, ''), parse_error: e.message };
      }
    });
}

function readRegistryScopeIds() {
  // Cheap parse — extract `id: "..."` lines from the RON file.
  if (!fs.existsSync(REGISTRY_PATH)) return [];
  const raw = fs.readFileSync(REGISTRY_PATH, 'utf8');
  const ids = [];
  for (const line of raw.split('\n')) {
    const m = line.match(/^\s*id:\s*"([^"]+)"/);
    if (m) ids.push(m[1]);
  }
  return ids;
}

function statusEmoji(status) {
  return ({
    complete: '✅',
    partial: '🟡',
    scaffold: '🟠',
    missing: '❌',
  })[status] || '❓';
}

function recommendationEmoji(rec) {
  return ({
    active: '🔄 active',
    passive: '😴 passive',
    blocked: '🚧 blocked',
  })[rec] || '❓';
}

function renderMarkdown(sessions, registryIds) {
  const auditedIds = new Set(sessions.map(s => s.scope_id));
  const unaudited = registryIds.filter(id => !auditedIds.has(id));

  let md = '# Agent coordination status\n\n';
  md += `Generated: ${new Date().toISOString()}\n\n`;
  md += `Audited scopes: **${sessions.length} / ${registryIds.length}**\n\n`;

  // Summary table
  md += '| Scope | Status | Recommendation | Audited | Summary |\n';
  md += '|---|---|---|---|---|\n';
  for (const s of sessions.sort((a, b) => a.scope_id.localeCompare(b.scope_id))) {
    md += `| **${s.scope_id}** | ${statusEmoji(s.implementation_status)} ${s.implementation_status || '?'} | ${recommendationEmoji(s.recommended_status)} | ${s.audited_at || '?'} | ${(s.summary || '').replace(/\|/g, '\\|').slice(0, 100)} |\n`;
  }

  if (unaudited.length) {
    md += `\n## Unaudited scopes (${unaudited.length})\n\n`;
    for (const id of unaudited) {
      md += `- **${id}** — no session JSON yet. Spin up an audit agent for this scope.\n`;
    }
  }

  // Gaps + TODOs across all audited scopes
  const allGaps = sessions.flatMap(s => (s.gaps || []).map(g => ({scope: s.scope_id, gap: g})));
  if (allGaps.length) {
    md += `\n## Gaps and missing work (${allGaps.length})\n\n`;
    for (const g of allGaps) {
      md += `- **${g.scope}**: ${g.gap}\n`;
    }
  }

  const allTodos = sessions.flatMap(s => (s.todos_found || []).map(t => ({scope: s.scope_id, todo: t})));
  if (allTodos.length) {
    md += `\n## TODOs found in code (${allTodos.length})\n\n`;
    for (const t of allTodos) {
      md += `- **${t.scope}**: ${t.todo}\n`;
    }
  }

  // Per-scope detail
  md += '\n## Per-scope detail\n\n';
  for (const s of sessions) {
    md += `### ${s.scope_id}\n`;
    md += `- Status: **${s.implementation_status || '?'}**\n`;
    md += `- Recommendation: **${s.recommended_status || '?'}**\n`;
    md += `- Audited: ${s.audited_at || '?'} by ${s.agent_id || '?'}\n`;
    md += `- Completion check holds: ${s.completion_check_holds === true ? '✅' : s.completion_check_holds === false ? '❌' : '?'}\n`;
    if (s.data_coverage_estimate) {
      md += `- Data coverage estimate: ${s.data_coverage_estimate}\n`;
    }
    if (s.owned_files) {
      md += '- Owned files:\n';
      for (const f of s.owned_files) {
        md += `  - \`${f.path}\` ${f.exists ? '✅' : '❌'} ${f.row_count != null ? `${f.row_count} rows` : f.loc != null ? `${f.loc} LOC` : ''} ${f.test_count != null ? `${f.test_count} tests` : ''}\n`;
      }
    }
    md += `- Summary: ${s.summary || ''}\n\n`;
  }

  return md;
}

function main() {
  const args = process.argv.slice(2);
  const json = args.includes('--json');
  const scope = (args.indexOf('--scope') !== -1) ? args[args.indexOf('--scope') + 1] : null;

  let sessions = readSessions();
  if (scope) sessions = sessions.filter(s => s.scope_id === scope);

  if (json) {
    process.stdout.write(JSON.stringify(sessions, null, 2));
    return;
  }
  const registryIds = readRegistryScopeIds();
  process.stdout.write(renderMarkdown(sessions, registryIds));
}

main();
