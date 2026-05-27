#!/usr/bin/env node
// PostToolUse hook (matcher "Write|Edit") — wired in .claude/settings.json.
//
// When data/gui/theme.ron (the single source for design tokens) is edited,
// regenerate web/shared/theme.css so the web theme can NEVER silently drift
// from native. No-op for every other file. Reads the hook payload (which
// includes the edited file path) from stdin. Pure Node stdlib.
//
// This makes the single-source self-maintaining — see CLAUDE.md "Cross-session
// persistence" and the theme pipeline in docs/design/ui-system.md.
let input = '';
process.stdin.on('data', (d) => (input += d));
process.stdin.on('end', () => {
  try {
    const j = JSON.parse(input || '{}');
    const f = (
      (j.tool_input && j.tool_input.file_path) ||
      (j.tool_response && j.tool_response.filePath) ||
      ''
    ).replace(/\\/g, '/');
    if (!/theme\.ron$/.test(f)) return; // only theme.ron triggers a regen
    require('child_process').execSync('node scripts/gen-theme-css.js', { stdio: 'ignore' });
    process.stdout.write(JSON.stringify({
      systemMessage: 'theme.ron changed -> regenerated web/shared/theme.css (single-source kept in sync)',
    }));
  } catch (e) {
    // Advisory only — never fail the edit over a theme regen.
  }
});
