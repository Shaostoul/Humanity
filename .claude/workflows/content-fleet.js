// Content fleet: the data-only writers, all at once, each in its own worktree.
//
// This is the first deliberately ALL-PARALLEL run: botanist, lexicographer and
// homestead-engineer own disjoint data files and never touch src/, so they are
// the safest tier to fan out (docs/design/multi-agent-workflow.md, "Content
// agents"). Worktree isolation makes even a stray blanket-add harmless.
//
// Run: Workflow({ scriptPath: ".claude/workflows/content-fleet.js" })
//
// Nothing merges automatically. Each writer leaves its work staged in its own
// worktree and the workflow returns the three reports; the main session merges
// serially after reviewing them.

export const meta = {
  name: 'content-fleet',
  description: 'Botanist, lexicographer and homestead-engineer in parallel worktrees, data-only, no auto-merge',
  phases: [
    { title: 'Fleet', detail: 'three data-only writers in parallel, isolated worktrees' },
  ],
}

phase('Fleet')

const [plants, words, homestead] = await parallel([
  () => agent(
    'Add 12 to 15 new staple-food species to data/plants.csv, prioritised for real ' +
    'self-sufficiency: grains, legumes, roots and tubers, and oil crops the file does ' +
    'not yet have, with climate coverage beyond temperate gardens (arid and cold ' +
    'climates need options too). Follow your definition file exactly: check each id ' +
    'and its synonyms against the existing 134 rows first, source every number and ' +
    'name the source per species, keep the existing units and stage vocabulary, ' +
    'confirm every harvest_item id exists in data/items.csv before using it, and run ' +
    'just validate-data at the end (the worktree compiles cold, so it takes a few ' +
    'minutes; that is normal). Leave your work staged with just mine data/plants.csv ' +
    'and report per your output format.',
    { agentType: 'botanist', isolation: 'worktree', label: 'fleet:plants' }
  ),
  () => agent(
    'Sweep the places people actually meet words and add missing definitions to ' +
    'data/glossary.json. Work in your priority order: native UI labels and page copy ' +
    'in src/gui/pages/ first, then quest and onboarding copy in ' +
    'data/onboarding/quests.json, then the Accord documents in data/library/. Add ' +
    'the terms a newcomer would actually be stopped by; skip terms already among the ' +
    '201 present. Plain first sentence, concrete over abstract, verify any behaviour ' +
    'a definition describes against the code, and report any term you could not ' +
    'define confidently rather than guessing. Verify the JSON parses when done. ' +
    'Leave your work staged with just mine data/glossary.json and report per your ' +
    'output format.',
    { agentType: 'lexicographer', isolation: 'worktree', label: 'fleet:dictionary' }
  ),
  () => agent(
    'Audit the seven loop balances in data/home_outline.json for one resident: does ' +
    'each loop actually balance at the numbers given? Show the arithmetic per loop ' +
    'with units, cross-check food yields against data/plants.csv and air/water ' +
    'recovery against the real ECLSS numbers your definition file names, and state ' +
    'every assumption. Where a stated number does not balance, correct it in the ' +
    'data file with a source, keeping dual units (metric first, imperial in ' +
    'parentheses) and both tiers. Do NOT close any of the five cannot-close loops. ' +
    'A loop that fails to balance is a finding to report prominently, not to paper ' +
    'over. Leave your work staged with just mine on the files you changed and ' +
    'report per your output format.',
    { agentType: 'homestead-engineer', isolation: 'worktree', label: 'fleet:homestead' }
  ),
])

return {
  plants,
  dictionary: words,
  homestead,
  note: 'Each writer left staged work in its own worktree. Merge serially from the main session after review.',
}
