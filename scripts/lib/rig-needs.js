// WHAT EACH RIG NEEDS FROM THE MACHINE (2026-10-22): one table, so the split is explicit.
//
// The operator, 2026-10-22: "I'd rather development testing continue if possible. Is there any
// way we could minimize the impact of the game running in the background for all your testing
// purposes? Like if you set all of the graphics to minimum/off. ... If certain parts of the app
// aren't necessary to a test can unnecessary resources hogs be disabled? Like minimum graphics vs
// max."
//
// A GeForce cannot be split between him and the rigs, so the question for each rig is what its
// verdict is made of:
//
//   behaviour  the verdict is what the game DID (where the body went, what the HUD named, which
//              messages crossed the relay, whether it panicked). Minimum graphics change none of
//              that. While one of his GPU-heavy programs runs (scripts/lib/operator-apps.json, or
//              his own HumanityOS: machine-guard.js isOperatorGame),
//              the rig does NOT wait for it: it boots its game in LOW IMPACT (src/engine/
//              low_impact.rs: minimum graphics, a low frame cap, a small window, never saved) and
//              runs beside it. When none runs, the rig boots at full graphics as it always has.
//   picture    the verdict is the picture (a vantage compared, a photograph, a clip, a golden).
//              Low impact would make it meaningless, so the rig WAITS for his program to close,
//              and REFUSES to run in low impact at all (asked by flag or by an inherited
//              HUMANITY_LOW_IMPACT).
//   timing     the verdict is a time (frame costs, boot time). The same as picture: it waits, and
//              refuses low impact.
//   no-game    boots no game at all (a headless relay, no GPU): his program is nothing to it.
//
// Every rig script that starts the game names itself here (scripts/tests/rig-needs.test.js reads
// every spawnGame call's rigName and fails on one missing from this table), so a new rig cannot
// be added without saying which it is. One booted HumanityOS at a time still holds for every rig:
// the boot lock (lib/boot-lock.js) and the wait for other games are unchanged.
//
// `lowImpact` is the HUMANITY_LOW_IMPACT value a behaviour rig boots with (src/engine/
// low_impact.rs parse): "1" for the defaults (15 frames a second, a 960 x 540 window), or a
// comma list of fps=N and window=WxH / window=keep for a rig whose judgement needs more.

"use strict";

const path = require("path");
const MG = require("./machine-guard.js");

/// The kinds, and what each does about the operator's programs.
const KINDS = Object.freeze({
  behaviour: "runs beside his program, in low impact",
  picture: "waits for his program to close; never low impact",
  timing: "waits for his program to close; never low impact",
  "no-game": "boots no game; his program is nothing to it",
});

/// Every rig, by the name it passes spawnGame as rigName (or its script name for one that boots
/// no game). Each with the reason it is that kind.
const RIGS = Object.freeze({
  "verify-runtime": {
    kind: "behaviour",
    lowImpact: "1",
    why: "boot, world entry and panics: its three vantages must capture, but no capture is compared",
    // Low impact turns passes off, so a pass in it does not cover them; the verdict says so.
    notCovered: "the passes low impact turns off (clouds, sun shadows, bounced light probes, god rays, grass, most 3D tree models)",
  },
  "verify-storeys": {
    kind: "behaviour",
    lowImpact: "1",
    why: "where the walk, the jump, the lift and the ladder took the body, and what the HUD named; its screenshots are kept for a person, never judged",
  },
  "verify-falls": {
    kind: "behaviour",
    lowImpact: "1",
    why: "the landings, wounds and health the falls left (landing speed comes from the height fallen, not the frame rate)",
  },
  "verify-copresence": {
    kind: "behaviour",
    // window=keep: it counts the walker's teal (and the crew's amber) pixels in its screenshots
    // against thresholds sized for the full window (lib/copresence-judge.js FIGURE_MIN_PX,
    // CREW_MIN_PX). fps=30: its steady-speed band is reasoned for the 30 frames a second a
    // background game draws (copresence-judge.js SPEED_BAND).
    lowImpact: "fps=30,window=keep",
    why: "where the game drew the other player frame by frame, what the relay accepted, and whether the figure is in the picture at all (--build, --starts, --household: the same rig)",
  },
  "verify-screens": {
    kind: "behaviour",
    lowImpact: "1",
    why: "whether a click changed the wall screen: its snapshots are the screen's own texture, compared before and after, not the window",
  },
  "verify-live-screen": {
    kind: "behaviour",
    lowImpact: "1",
    why: "whether the wall screen's own picture moves, stops and says why: the screen's texture, not the window",
  },
  "second-player": {
    kind: "no-game",
    why: "a scripted player against a headless relay: no window, no GPU",
  },
  "probe-sweep": {
    kind: "picture",
    why: "vantage captures compared and timed (verify-runtime runs it as a boot check, --boot-check, under verify-runtime's own entry)",
  },
  "photograph-home": {
    kind: "picture",
    why: "photographs of the home for a person to judge",
  },
  "make-clips": {
    kind: "picture",
    why: "clips for social media, at the operator's own graphics",
  },
  "boot-timing": {
    kind: "timing",
    why: "how long the game takes to boot",
  },
  "perf-report": {
    kind: "timing",
    why: "grades a sweep's frame times (refuses a sweep that ran in low impact)",
  },
});

/// The entry for `rig`, or throws: a rig that does not say what it needs is a programming error,
/// found at its first run rather than guessed at.
function needsOf(rig) {
  const e = RIGS[rig];
  if (!e) throw new Error(`rig-needs: "${rig}" is not in scripts/lib/rig-needs.js RIGS; add it with its kind and why`);
  return e;
}

/// How one of his programs is named in a line: its exe name, or for his own HumanityOS its path
/// ("HumanityOS.exe" alone would read as a rig).
const appName = (p) => (MG.isOperatorGame(p) ? `his own game ${p.exe}` : p.name);

/// Is a HUMANITY_LOW_IMPACT value on? (The engine's own reading, src/engine/low_impact.rs parse.)
const isOn = (v) => {
  const s = String(v == null ? "" : v).trim().toLowerCase();
  return !!s && !["0", "off", "false", "no"].includes(s);
};

/// Decide how `rig` boots right now. Pure given its inputs, so it is tested without a machine.
///   opt.argv   the rig's arguments: --low-impact (always low impact; a behaviour rig only),
///              --full-graphics (never low impact: a behaviour rig then waits for his program)
///   opt.env    the rig's environment (process.env): an inherited HUMANITY_LOW_IMPACT is refused
///              by a picture or timing rig
///   opt.apps   the operator's programs running now ([{ pid, name, exe }]; default: looked up)
/// Returns {
///   rig, kind, lowImpact   the HUMANITY_LOW_IMPACT value to boot with, or null for full graphics
///   apps                   his programs running now
///   own                    extra fields for machine-guard's `own` (lowImpact: his programs are
///                          then not in the way)
///   why                    one line for the log and the manifest
///   refuse                 null, or the lines to refuse with (exit 1, nothing booted)
/// }
function planBoot(rig, opt = {}) {
  const e = needsOf(rig);
  const argv = opt.argv || [];
  const env = opt.env || {};
  const apps = opt.apps !== undefined ? opt.apps : MG.operatorApps();
  const asked = argv.includes("--low-impact");
  const full = argv.includes("--full-graphics");
  const inherited = isOn(env.HUMANITY_LOW_IMPACT);
  const names = apps.map(appName).join(", ");
  const base = { rig, kind: e.kind, apps, lowImpact: null, own: {}, why: "", refuse: null };
  if (asked && full) {
    return { ...base, refuse: [`${rig}: REFUSED - --low-impact and --full-graphics together ask for both. Nothing was booted.`] };
  }
  if (e.kind === "picture" || e.kind === "timing") {
    if (asked || inherited) {
      return {
        ...base,
        refuse: [
          `${rig}: REFUSED - this rig judges ${e.kind === "picture" ? "the picture" : "times"} (${e.why}), and low impact`,
          `(${asked ? "--low-impact" : `HUMANITY_LOW_IMPACT=${env.HUMANITY_LOW_IMPACT} in this shell`}) would make that meaningless: minimum graphics, a capped frame rate and`,
          "a small window are not the game it is meant to judge. Nothing was booted.",
          ...(inherited ? ["Unset HUMANITY_LOW_IMPACT in this shell (only the rigs set it, for the game they start)."] : []),
          apps.length
            ? `It waits instead for ${names} to close (scripts/lib/rig-needs.js).`
            : "Run it without that, at full graphics.",
        ],
      };
    }
    return { ...base, why: apps.length ? `${e.kind} rig: waits for ${names} to close` : `${e.kind} rig: full graphics` };
  }
  if (e.kind === "no-game") return { ...base, why: "boots no game" };
  // behaviour
  if (full) {
    return { ...base, why: apps.length ? `behaviour rig at full graphics (--full-graphics): waits for ${names} to close` : "behaviour rig at full graphics (--full-graphics)" };
  }
  if (asked || apps.length) {
    return {
      ...base,
      lowImpact: e.lowImpact || "1",
      own: { lowImpact: true },
      why: asked
        ? `LOW IMPACT (--low-impact): HUMANITY_LOW_IMPACT=${e.lowImpact || "1"}`
        : `LOW IMPACT beside ${names} (the operator's program): HUMANITY_LOW_IMPACT=${e.lowImpact || "1"}, no wait for it`,
    };
  }
  return { ...base, why: "behaviour rig at full graphics (none of the operator's programs is running)" };
}

/// Plan, print the plan line, and refuse (exit 1) when the plan says so. What a rig calls first.
function planOrRefuse(rig, opt = {}) {
  const p = planBoot(rig, { argv: process.argv.slice(2), env: process.env, ...opt });
  if (p.refuse) {
    console.error("");
    for (const l of p.refuse) console.error(l);
    console.error("");
    process.exit(1);
  }
  (opt.log || console.log)(`[${rig}] ${p.why}`);
  return p;
}

/// The flags every rig reads through planBoot, for a rig that refuses unknown arguments.
const FLAGS = Object.freeze(["--low-impact", "--full-graphics"]);

/// The manifest's record of a plan and of what the game confirmed (game-launch.js spawnGame's
/// `low_impact`). Null for a full-graphics run.
function manifestRecord(plan, confirmed) {
  if (!plan || !plan.lowImpact) return null;
  return {
    value: plan.lowImpact,
    because: plan.apps.length ? plan.apps.map(appName) : ["--low-impact"],
    confirmed: confirmed || null,
    not_covered: needsOf(plan.rig).notCovered || null,
  };
}

/// The manifest record, with what the game wrote to confirm it (its rig folder's
/// debug/low_impact.json, which spawnGame deletes before every boot, so it is this run's).
function recordFromRig(plan, rigDir) {
  if (!plan || !plan.lowImpact) return null;
  const GL = require("./game-launch.js");
  return manifestRecord(plan, GL.readLowImpact(path.join(rigDir, "debug", GL.LOW_IMPACT_FILE)));
}

/// For a rig with no machine wait of its own (it refuses on another game, and spawnGame takes the
/// boot lock): when this run is NOT light and one of the operator's programs runs, wait for it to
/// close, bounded, and refuse (exit 1, nothing booted) if it never does. A low-impact plan needs no
/// wait: it runs beside his program.
function waitForOperator(plan, opt = {}) {
  if (!plan || plan.lowImpact || plan.kind === "no-game" || !plan.apps.length) return null;
  const w = MG.waitForFree({
    own: { gameOnly: true, exe: opt.exe || "" },
    lock: false, // spawnGame takes the boot lock right before the spawn
    label: `${plan.rig}: the operator's program`,
    rig: plan.rig,
    timeoutMs: opt.timeoutMs != null ? opt.timeoutMs : MG.WAIT_MIN_DEFAULT * 60 * 1000,
    log: opt.log || ((m) => console.log(m)),
  });
  if (!w.free) {
    MG.refuseBusy(w, {
      rig: plan.rig,
      hint: [
        plan.kind === "behaviour"
          ? "This run asked for full graphics (--full-graphics); without that flag it would have run light beside his program."
          : `This rig judges ${plan.kind === "picture" ? "the picture" : "times"}, which his program would make meaningless; run it when he is done.`,
      ],
    });
  }
  return w;
}

/// The lines a verdict prints about a low-impact run (none for a full-graphics one), so nobody
/// reads its result as a full-graphics run's.
function verdictLines(rec) {
  if (!rec) return [];
  const c = rec.confirmed;
  const win = c ? (Array.isArray(c.window) ? c.window.join("x") : c.window) : null;
  const lines = [
    `LOW IMPACT: the game ran at minimum graphics (HUMANITY_LOW_IMPACT=${rec.value}) because of ${rec.because.join(", ")}; ` +
      (c ? `the game confirmed ${c.fps} fps, window ${win}.` : "the game NEVER CONFIRMED it (see the run's log)."),
    "What the game did is judged exactly as at full graphics; how it looked is not this run's to say.",
  ];
  if (rec.not_covered) lines.push(`Not covered by this run: ${rec.not_covered}. Run it again at full graphics (no --low-impact, his program closed) before a release.`);
  return lines;
}

module.exports = { KINDS, RIGS, needsOf, isOn, planBoot, planOrRefuse, FLAGS, manifestRecord, recordFromRig, waitForOperator, verdictLines };
