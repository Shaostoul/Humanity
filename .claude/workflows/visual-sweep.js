// 3D visual-regression sweep: capture the canonical vantages (the running
// game, not the egui pages), then judge each screenshot against its golden
// spec in parallel. The perf half is deterministic (`just perf-sweep`); this
// is the half that needs eyes, so it is a workflow.
//
// Run it:  Workflow({ scriptPath: ".claude/workflows/visual-sweep.js" })
// Requires: a built release exe (target/release/HumanityOS.exe) and a GPU on
// the machine running it (the capture drives the real game).
//
// Shape: one capture agent runs scripts/probe-sweep.js and returns the
// manifest (which already carries each vantage's `expect` + `regressions`),
// then one judge per vantage reads its PNG and rules pass/fail/warn, then a
// synthesis agent produces the report. The workflow SCRIPT has no filesystem
// access, so the capture agent is what reads the manifest off disk.

export const meta = {
  name: 'visual-sweep',
  description: 'Capture the canonical 3D vantages then judge each screenshot against its golden spec (visual regression)',
  phases: [
    { title: 'Capture', detail: 'drive the probe through tests/visual/vantages.json' },
    { title: 'Judge', detail: 'one judge per screenshot vs its expect + regressions' },
    { title: 'Report', detail: 'pass/fail summary' },
  ],
}

const MANIFEST = {
  type: 'object',
  additionalProperties: false,
  required: ['dir', 'captured', 'total', 'panics', 'vantages'],
  properties: {
    dir: { type: 'string', description: 'absolute sweep output directory' },
    captured: { type: 'integer' },
    total: { type: 'integer' },
    panics: { type: 'integer' },
    vantages: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['id', 'ok'],
        properties: {
          id: { type: 'string' },
          ok: { type: 'boolean' },
          screenshot: { type: 'string', description: 'PNG filename inside dir (present when ok)' },
          expect: { type: 'string' },
          regressions: { type: 'array', items: { type: 'string' } },
          fps: { type: ['number', 'null'] },
          error: { type: 'string' },
        },
      },
    },
  },
}

const VERDICT = {
  type: 'object',
  additionalProperties: false,
  required: ['id', 'verdict', 'notes'],
  properties: {
    id: { type: 'string' },
    verdict: { type: 'string', enum: ['pass', 'warn', 'fail'] },
    notes: { type: 'string', description: 'what the frame shows and how it does or does not match; name any regression seen' },
  },
}

const REPORT = {
  type: 'object',
  additionalProperties: false,
  required: ['status', 'summary', 'verdicts'],
  properties: {
    status: { type: 'string', enum: ['clean', 'warnings', 'regressions'] },
    summary: { type: 'string' },
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['id', 'verdict', 'note'],
        properties: {
          id: { type: 'string' },
          verdict: { type: 'string', enum: ['pass', 'warn', 'fail'] },
          note: { type: 'string' },
        },
      },
    },
  },
}

phase('Capture')
const cap = await agent(
  'From C:\\Humanity run `node scripts/probe-sweep.js` (Bash) and wait for it to finish - it boots the release exe, drives the canonical vantage tour, writes screenshots + a manifest, and kills the game. Then Read the manifest: the absolute output dir is the content of .probe-rig/latest-sweep.txt, and the manifest is <dir>/manifest.json. Return the manifest fields exactly: dir, captured, total, panics, and for each vantage its id, ok, screenshot (the PNG filename), expect, regressions, fps, and error if any. Do not judge the images - only report what the sweep produced.',
  { label: 'capture', phase: 'Capture', schema: MANIFEST }
)

phase('Judge')
const shots = cap.vantages.filter((v) => v.ok && v.screenshot)
const verdicts = await parallel(
  shots.map((v) => () =>
    agent(
      `You are a visual-regression judge for the "${v.id}" vantage. Read the screenshot at "${cap.dir}\\${v.screenshot}" (use the Read tool - it renders the image).\n\nIt MUST match this expected appearance:\n${v.expect}\n\nSpecifically it must NOT show any of these known regressions:\n- ${(v.regressions || []).join('\n- ')}\n\nIgnore the small HUD text/overlay and the center crosshair dot - judge the rendered 3D scene only. Rule: verdict "pass" if the scene clearly matches the expectation and shows none of the regressions; "fail" if any listed regression is present or the scene is clearly wrong (e.g. black/undressed frame, missing subject); "warn" if it is plausibly off but ambiguous. When genuinely uncertain between pass and fail, choose warn. In notes, say concretely what the frame shows and cite the specific matching or failing detail.`,
      { label: `judge:${v.id}`, phase: 'Judge', schema: VERDICT }
    ).then((x) => x || { id: v.id, verdict: 'warn', notes: 'judge returned nothing' })
  )
)

phase('Report')
const failedCapture = cap.vantages.filter((v) => !v.ok).map((v) => ({ id: v.id, verdict: 'fail', note: `capture failed: ${v.error || 'unknown'}` }))
const report = await agent(
  `Summarize this 3D visual-regression sweep for the operator.\n\nCapture: ${cap.captured}/${cap.total} vantages, ${cap.panics} panic(s).\nCapture failures: ${JSON.stringify(failedCapture)}\nJudge verdicts: ${JSON.stringify(verdicts.filter(Boolean))}\n\nstatus = "regressions" if any verdict is fail or any capture failed or panics>0; "warnings" if any warn but no fail; else "clean". Give a one-line summary and the per-vantage verdict list (fold in the capture failures as fails). Lead with anything that failed.`,
  { label: 'report', phase: 'Report', schema: REPORT }
)
return report
