// Domain pass: improve one visual domain (water, clouds, plants, terrain,
// atmosphere...) end to end, with fidelity and performance held in tension the
// way the operator wants them: max quality FIRST, then make that same image
// cheaper, never trading the quality away to buy frames.
//
// Run it:
//   Workflow({ scriptPath: ".claude/workflows/domain-pass.js",
//              args: { domain: "clouds",
//                      owns: ["src/renderer/clouds.rs", "assets/shaders/pbr/40-clouds.wgsl"],
//                      vantages: ["ocean-storm-low", "limb-400km"] } })
//
// Requires a built release exe and a GPU: the fidelity and perf phases both
// drive the real game through scripts/probe-sweep.js.
//
// Shape, and why:
//   1. Historian gate     - has this already been done? Cheapest agent, and this
//                           repo's most common waste is rebuilding a shipped pass.
//                           If it says EXISTS, the workflow stops rather than
//                           spending a fleet on redoing it.
//   2. Fidelity + perf    - in PARALLEL, both read-only. Fidelity finds the
//                           missing realism cue; perf measures where the cost
//                           actually is. Neither can corrupt anything.
//   3. Challenger         - is this the right thing to build at all, given both
//                           reports? Cheap insurance before any writing starts.
//   4. Implement          - ONE domain-writer, in its own worktree. Single writer
//                           by design: this workflow improves one domain, and
//                           parallel writers only pay off across DIFFERENT domains.
//   5. Verify             - runtime-verifier (does it still boot and enter the
//                           world) and critic (does the evidence actually support
//                           the claim), in parallel.
//
// Nothing here commits or merges. The writer leaves work staged in its worktree
// and the workflow returns a report; landing it is the operator's call, because
// agent work in this repo has been wrong in ways that passed local checks.

export const meta = {
  name: 'domain-pass',
  description: 'Fidelity then performance pass over one visual domain, verified, without auto-merging',
  phases: [
    { title: 'Prior art', detail: 'has this already been built or tried?' },
    { title: 'Analyse', detail: 'fidelity gaps and perf measurements, in parallel' },
    { title: 'Decide', detail: 'challenge the approach before writing code' },
    { title: 'Implement', detail: 'one domain-writer in an isolated worktree' },
    { title: 'Verify', detail: 'runtime boot + adversarial check of the evidence' },
  ],
}

const PRIOR_ART = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict', 'evidence'],
  properties: {
    verdict: { type: 'string', enum: ['EXISTS', 'EXISTED', 'TRIED', 'NEW'] },
    evidence: { type: 'string', description: 'paths, versions, commits' },
    remaining_gap: { type: 'string', description: 'the real gap if EXISTS' },
  },
}

const FINDINGS = {
  type: 'object',
  additionalProperties: false,
  required: ['findings'],
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['summary', 'detail'],
        properties: {
          summary: { type: 'string', description: 'one line' },
          detail: { type: 'string' },
          cost: { type: 'string', description: 'rough cost or expected saving' },
          acceptance: { type: 'string', description: 'expect/regressions line or fps target' },
        },
      },
    },
    measurement: { type: 'string', description: 'perf only: what was measured and the numbers' },
  },
}

const DECISION = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict', 'reasoning', 'do_this'],
  properties: {
    verdict: { type: 'string', enum: ['KEEP', 'ADJUST', 'RECONSIDER'] },
    reasoning: { type: 'string' },
    do_this: { type: 'string', description: 'the concrete task for the writer' },
  },
}

const VERDICT = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict', 'evidence'],
  properties: {
    verdict: { type: 'string', enum: ['RUNS', 'PANICS', 'UNVERIFIED', 'CONFIRMED', 'REFUTED'] },
    evidence: { type: 'string' },
  },
}

const domain = (args && args.domain) || null
const owns = (args && args.owns) || []
const vantages = (args && args.vantages) || []

if (!domain || !owns.length) {
  log('domain-pass needs { domain, owns: [...paths], vantages: [...ids] }')
  return { error: 'missing domain or owns' }
}

const vantageList = vantages.length ? vantages.join(', ') : '(pick the relevant ones from tests/visual/vantages.json)'

// ── 1. Prior art gate ────────────────────────────────────────────────────────
phase('Prior art')
const prior = await agent(
  `Has a fidelity or performance pass over ${domain} already been done in this repo?\n` +
  `Owned files: ${owns.join(', ')}\n` +
  `Check docs/FEATURES.md, docs/STATUS.md, docs/BUGS.md, docs/PRIORITIES.md, docs/history/, ` +
  `and git log (release titles here are descriptive, so grep them).\n` +
  `If it EXISTS, say what the remaining genuine gap is.`,
  { agentType: 'historian', schema: PRIOR_ART, label: `prior-art:${domain}` }
)

if (prior && prior.verdict === 'EXISTS' && !(prior.remaining_gap || '').trim()) {
  log(`STOP: ${domain} already ships and no remaining gap was identified.`)
  return { stopped: 'already exists', prior }
}
log(`prior art: ${prior ? prior.verdict : 'unknown'}`)

// ── 2. Fidelity and perf, in parallel (both read-only) ───────────────────────
phase('Analyse')
const [fidelity, perf] = await parallel([
  () => agent(
    `Why does ${domain} not look real, and what specifically is missing?\n` +
    `Owned files: ${owns.join(', ')}\nVantages: ${vantageList}\n` +
    `Capture the current output with the probe rig and look at it before reasoning about source. ` +
    `Ground every finding in real-world behaviour. Rank by realism gained per unit of work. ` +
    `Give each finding an acceptance test as an expect/regressions line.` +
    (prior && prior.remaining_gap ? `\nKnown remaining gap: ${prior.remaining_gap}` : ''),
    { agentType: 'fidelity-expert', schema: FINDINGS, label: `fidelity:${domain}`, phase: 'Analyse' }
  ),
  () => agent(
    `Where is the actual cost of ${domain}, and how would you make the SAME image cheaper?\n` +
    `Owned files: ${owns.join(', ')}\nVantages: ${vantageList}\n` +
    `MEASURE FIRST with just perf-sweep or probe-sweep and say what the limit really is ` +
    `(GPU compute, bandwidth, draw submission, overdraw, CPU). Then cover BOTH axes: the cost ` +
    `of one instance, and the cost at infinite-of-x scale (instancing, culling, LOD, impostors, ` +
    `shared work, memory layout). Quantify every proposal. Do not propose anything that changes ` +
    `how it looks.`,
    { agentType: 'perf-expert', schema: FINDINGS, label: `perf:${domain}`, phase: 'Analyse' }
  ),
])

const fidCount = fidelity && fidelity.findings ? fidelity.findings.length : 0
const perfCount = perf && perf.findings ? perf.findings.length : 0
log(`fidelity findings: ${fidCount} | perf findings: ${perfCount}`)
if (!fidCount && !perfCount) {
  log(`STOP: nothing to do for ${domain}.`)
  return { stopped: 'no findings', prior }
}

// ── 3. Challenge before writing anything ─────────────────────────────────────
phase('Decide')
const decision = await agent(
  `A ${domain} pass is proposed. Is this the right thing to do next, and in this shape?\n\n` +
  `FIDELITY:\n${JSON.stringify(fidelity, null, 1)}\n\n` +
  `PERF:\n${JSON.stringify(perf, null, 1)}\n\n` +
  `The operator's rule is maximum quality first, then make that same image cheaper, never ` +
  `trading fidelity for frames. Return the single concrete task the writer should do. ` +
  `KEEP is a fine verdict.`,
  { agentType: 'challenger', schema: DECISION, label: `challenge:${domain}` }
)
log(`challenger: ${decision ? decision.verdict : 'unknown'}`)
if (decision && decision.verdict === 'RECONSIDER') {
  log('STOP: challenger says the framing is wrong. Not writing code.')
  return { stopped: 'reconsider', prior, fidelity, perf, decision }
}

// ── 4. Implement, isolated ───────────────────────────────────────────────────
phase('Implement')
const work = await agent(
  `DOMAIN: ${domain}\nOWNED PATHS: ${owns.join(', ')}\n\n` +
  `TASK: ${decision ? decision.do_this : 'apply the highest-ranked fidelity finding'}\n\n` +
  `FIDELITY FINDINGS:\n${JSON.stringify(fidelity, null, 1)}\n\n` +
  `PERF FINDINGS:\n${JSON.stringify(perf, null, 1)}\n\n` +
  `Edit ONLY the owned paths. Return WIRING REQUESTS for anything needing a shared file. ` +
  `Update the relevant vantage's expect/regressions in tests/visual/vantages.json so the ` +
  `change is verifiable later. Do not commit, do not push.`,
  { agentType: 'domain-writer', isolation: 'worktree', label: `write:${domain}` }
)

// ── 5. Verify: does it run, and does the evidence hold ───────────────────────
phase('Verify')
const [runs, checked] = await parallel([
  () => agent(
    `Does the ${domain} change still boot AND enter the world?\n\n${work}\n\n` +
    `Build release and drive the probe rig through: ${vantageList}. ` +
    `A green menu is not sufficient; world entry is the bar. Report the real output.`,
    { agentType: 'runtime-verifier', schema: VERDICT, label: `runtime:${domain}`, phase: 'Verify' }
  ),
  () => agent(
    `Try to REFUTE this ${domain} work.\n\n${work}\n\n` +
    `Check the VERIFICATION first: could the checks it ran have failed if the change were ` +
    `broken? Then trace the change end to end for a narrowing or a missed call site.`,
    { agentType: 'critic', schema: VERDICT, label: `critic:${domain}`, phase: 'Verify' }
  ),
])

return {
  domain,
  prior,
  fidelity,
  perf,
  decision,
  work,
  runtime: runs,
  critique: checked,
  note: 'Nothing was committed or merged. The writer left its work staged in an isolated worktree.',
}
