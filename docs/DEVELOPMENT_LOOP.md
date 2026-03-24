# Development Loop

Autonomous development cycle. Repeat until v1.0.0.

## The Loop

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  1. EVALUATE                                    │
│     Read: FEATURES.md, STATUS.md, BUGS.md       │
│     Check: GitHub Issues, bug reports page       │
│     Audit: compile check, broken pages, stale    │
│            docs, version mismatches              │
│                                                 │
│  2. PLAN                                        │
│     Compare features list against vision         │
│     Identify: bugs > polish > new features       │
│     Priority: security > stability > UX > new    │
│     Check: does this already exist? (FEATURES)   │
│     Check: was this already fixed? (BUGS)        │
│                                                 │
│  3. BUILD                                       │
│     Fix bugs first (update BUGS.md)              │
│     Build features (update FEATURES.md)          │
│     Use subagents for parallel work              │
│     Compile check after every change             │
│     No backward compat hacks                     │
│                                                 │
│  4. SYNC                                        │
│     Run SYNC.md checklist                        │
│     Version bump (patch or minor)                │
│     Update: STATUS.md, FEATURES.md, BUGS.md      │
│     Update: CHANGELOG.md                         │
│     Update: CLAUDE.md if architecture changed    │
│     Commit, push, verify CI passes               │
│     Tag release if significant                   │
│                                                 │
│  5. VERIFY                                      │
│     Check live site (united-humanity.us)         │
│     Check CI deploy succeeded                    │
│     Test new pages load correctly                │
│     Check for console errors via Chrome          │
│     Read bug reports for regressions             │
│                                                 │
│  6. ASK (if needed)                             │
│     Design decisions that affect UX              │
│     Architecture choices with tradeoffs          │
│     Priorities when multiple paths exist         │
│     Anything that changes the user experience    │
│     If no questions: go to step 1                │
│                                                 │
└──────────── repeat ─────────────────────────────┘
```

## Priority Order

Always fix in this order:
1. **Security vulnerabilities** (immediately)
2. **Data loss bugs** (immediately)
3. **Crashes / white screens** (same session)
4. **Broken features** (same session)
5. **Polish / UX improvements** (next available)
6. **New features** (after stability)
7. **Optimization** (after feature complete)

## When to Ask the User

Ask before:
- Changing nav structure or page layout
- Renaming concepts (like Game to Sim)
- Removing features or pages
- Architecture decisions with multiple valid approaches
- Spending significant time on something speculative

Don't ask:
- Bug fixes (just fix them)
- Data file additions (items, recipes, materials)
- Documentation updates
- Code cleanup that doesn't change behavior
- Adding tests

## Autonomous Development Rules

1. Never rebuild an existing feature (check FEATURES.md)
2. Never re-fix a resolved bug (check BUGS.md)
3. Never create backward compatibility hacks
4. Always compile check before committing
5. Always use absolute paths for web script sources
6. Always update FEATURES.md when adding features
7. Always update BUGS.md when fixing bugs
8. Run the SYNC.md checklist before pushing
9. Use subagents for independent parallel work
10. Commit frequently with descriptive messages
