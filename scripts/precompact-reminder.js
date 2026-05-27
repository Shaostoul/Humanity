#!/usr/bin/env node
// PreCompact hook — wired in .claude/settings.json.
//
// Fires right before Claude Code compacts (summarizes + drops) the
// conversation context. It injects a reminder back into the model's context
// (via hookSpecificOutput.additionalContext) to flush any un-journaled durable
// state to DISK first — because on-disk state survives compaction and a fresh
// session reloads it, while anything only held in-conversation is lost.
//
// This is the automated backstop for the "Cross-session persistence (perpetual)"
// practice in CLAUDE.md. Edit the reminder text here; it's plain Node (stdlib).
const reminder =
  'PreCompact: context is about to be compacted. Flush any un-journaled durable state to disk NOW, ' +
  'before it is lost — decisions + the WHY behind them -> data/coordination/orchestrator_state.json (recent_decisions); ' +
  'what to work on next -> docs/PRIORITIES.md; operator corrections/preferences -> CLAUDE.md + memory; ' +
  'lessons -> the relevant docs/ file. On-disk survives compaction; in-context does not.';

process.stdout.write(JSON.stringify({
  hookSpecificOutput: {
    hookEventName: 'PreCompact',
    additionalContext: reminder,
  },
}));
