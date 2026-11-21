// The start's two times, read from a run.log (2026-11-20, the desktop's pages-first start;
// engine/world_prep.rs): when the PAGES were first drawn and when the 3D WORLD was ready, which
// since that day are separate moments (the world is prepared beside the pages after the first
// frame), plus how long a Play pressed before it was ready waited, and how long loading the world
// took once it could. scripts/boot-timing.js reports them beside its time to playable;
// scripts/tests/startup-times.test.js holds the parsing. Pure: no files, no game.

"use strict";

/** { first_page_frame_ms, pages_only, world_ready_ms, world_prep_ms, play_waited_ms, world_load_ms } from a run.log's
 *  text, each null when its line is not there (an older build, a run that ended first). */
function parseStartup(text) {
  const t = String(text || "");
  const num = (re) => {
    const m = t.match(re);
    return m ? Number(m[1]) : null;
  };
  const first = t.match(/\[Boot\] first frame drawn (\d+) ms after the window opened \(pages-only start: (yes|no)\)/);
  return {
    first_page_frame_ms: first ? Number(first[1]) : null,
    pages_only: first ? first[2] === "yes" : null,
    world_ready_ms: num(/\[WorldPrep\] the 3D world is ready: (\d+) ms after the window opened/),
    // From its preparation's start: on a start that waits for Play (the default), the world ready after Play.
    world_prep_ms: num(/\[WorldPrep\] the 3D world is ready: \d+ ms after the window opened, (\d+) ms after its preparation began/),
    play_waited_ms: num(/\[WorldPrep\] opening the world (\d+) ms after Play was pressed/),
    world_load_ms: num(/3D world loaded in (\d+)ms/),
  };
}

module.exports = { parseStartup };
