#!/usr/bin/env node
// verify-screens: the in-world screens gate. Boots the real release binary in
// a portable rig, enters the world, parks the camera in the console room, and
// PROVES with nobody at the keyboard that the wall screens are interactive:
//
//   inventory   find the "Home" container header on wall_screen_1 by its
//               drawn text, HOVER it, find the child row "Garage" (drawn
//               only while Home is open), snapshot, click the header, find
//               "Garage" again, snapshot: the hover must have landed at the
//               found uv, the click must report egui's PointingHand cursor
//               (the header row's own, set from row.hovered() in
//               inventory.rs, so the point is on the row and not merely on
//               the panel), "Garage" must have flipped from found to gone,
//               and the two images must differ. The hover comes FIRST so
//               both snapshots carry the pointer in the same place: egui's
//               floating scrollbar (egui 0.31 ScrollArea, invisible when
//               dormant, drawn the instant a pointer is over the area)
//               otherwise makes two frames differ by a scrollbar column
//               whether or not the click did anything, and a pixel diff
//               alone cannot tell a dead click from a live one (an
//               adversarial review found exactly that hole, 2026-09-17).
//   web         wait for wall_screen_3 (web:https://united-humanity.us) to
//               report a loaded page, snapshot, click its first link that
//               stays on our own host, wait again: the url must change and
//               still be on our host, the status must be ready again, and
//               the snapshot must differ.
//   tasks       snapshot wall_screen_2: not a single colour (a page drew).
//   panics      zero PANIC lines in the rig's run.log.
//
// Every event goes through debug/screen_request.json, which the engine feeds
// into the same ScreenCore event API the look ray uses (never a side path),
// so a green run here means a player can walk up to the wall and do the same.
// The operator's words: "We want to make sure people can actually interact
// with the web display screen in-game." The gate only FOLLOWS LINKS on our
// own site; the page's own images and any redirect go to the hosts the page
// names (docs/design/readable-web.md, "The opt-in, and what leaves the
// machine"), which is a property of the page, not of this rig.
//
// The rig writes `readable_web: true` into ITS OWN config.json before boot
// (the switch is off by default and a screen must never fetch while it is),
// then reads the switch back through the screen's own status field: a run
// where the setting did not take fails with "off", never passes by luck.
//
// Usage:
//   node scripts/verify-screens.js [--exe PATH] [--timeout-min N] [--keep-open]
//   node scripts/verify-screens.js --dry-verdict <manifest.json>
//   node scripts/verify-screens.js --self-test
// Exit 0 = every check passed. Exit 1 = refused to run (an instance already
// running, stale binary, rig busy). Exit 2 = the screens failed the gate.
//
// --dry-verdict re-judges an EXISTING manifest (its PNGs beside it) without
// booting anything; --self-test judges the three fixture manifests under
// tests/fixtures/screens/ (green must pass; red and dead-click must fail on
// exactly their known checks, dead-click being the scrollbar-only pixel
// diff with a child row that never went away) and is how the verdict logic
// itself is proven able to fail. Both print "DRY VERDICT (nothing was
// booted)" so a pasted transcript can never be mistaken for a live run.
//
// ONE GPU (CLAUDE.md): this refuses to boot while ANY HumanityOS.exe is
// running, the operator's game included, and never sets the take-focus env
// var (the operator's proof-of-human launch, src/engine/launch_focus.rs).

const fs = require("fs");
const path = require("path");
const { spawn, spawnSync, execSync } = require("child_process");
const png = require("./lib/png.js");
const G = require("./rig-graphics.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : def;
};
const flag = (name) => args.includes(name);
const KEEP_OPEN = flag("--keep-open");
const EXE = path.resolve(opt("--exe", path.join(REPO, "target", "release", "HumanityOS.exe")));
const TIMEOUT_MS = Number(opt("--timeout-min", "8")) * 60 * 1000;
const DRY = opt("--dry-verdict", null);
const SELF_TEST = flag("--self-test");

// Its own rig, nested in .probe-rig so the gitignore entry covers it, and
// separate from verify-runtime's so the two gates never fight over one exe
// copy or one set of debug/*.json files.
const RIG = path.join(REPO, ".probe-rig", "screens");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");

// THE SCREENS UNDER TEST, fixed on purpose: these are the placed instances
// in data/machines/home.ron. Renaming one there must break this gate loudly
// (the screen request answers "no screen named ..."), never shrink it.
const SCREENS = { inventory: "wall_screen_1", tasks: "wall_screen_2", web: "wall_screen_3" };
// The only site this gate fetches: our own. The web check asserts the loaded
// page is on this host, so a redirect elsewhere cannot pass.
const WEB_HOST = "united-humanity.us";
// The drawn text the inventory click targets: the Home container's header.
const INVENTORY_TARGET = "Home";
// A drawn text that exists ONLY while that container is open: its child
// container "Garage" (data/places/seed.json, Home's one room, nested as its
// own card inside Home's open body and drawn nowhere else on the page; the
// headless test `find_text_locates_the_home_header_and_a_click_there_toggles_it`
// in src/gui/screen_surface.rs guards that it vanishes when Home closes).
// Its `found` flipping across the click is the semantic proof the click
// toggled the header, which no pixel diff can give. Renaming the room in
// the data breaks this gate loudly (found=false on both sides), never
// quietly.
const INVENTORY_CHILD = "Garage";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pad = (s, n) => String(s).padEnd(n);
const rel = (p) => {
  const r = path.relative(REPO, p);
  return !r || r.startsWith("..") ? p : r;
};
function log(msg) {
  console.log(`[screens] ${msg}`);
}
function refuse(lines) {
  console.error("");
  for (const l of lines) console.error(l);
  console.error("");
  process.exit(1);
}

// ── The verdict: pure, over a manifest ───────────────────────────────────────
// Everything the live run learned is in the manifest (done files verbatim,
// PNG file names beside it, the panic count); this re-derives the verdict
// from that evidence alone, which is what lets --dry-verdict and --self-test
// exercise the exact code the live run trusts.
function hostOf(url) {
  try {
    return new URL(String(url)).hostname;
  } catch {
    return "";
  }
}
// "On our own site" means the host IS WEB_HOST or a subdomain of it, never
// merely a host that ends with those letters (evil-united-humanity.us must
// not count). Every same-host decision in this file goes through here.
function onOurHost(url) {
  const h = hostOf(url);
  return h === WEB_HOST || h.endsWith("." + WEB_HOST);
}
// Two uv pairs agree when each axis is within 1e-4. The engine writes f32
// values and a uv sent back to it round-trips exactly, so this is really
// an equality; the tolerance only keeps a number-formatting change from
// failing a good run.
function uvClose(a, b) {
  return (
    Array.isArray(a) && Array.isArray(b) && a.length === 2 && b.length === 2 &&
    Math.abs(a[0] - b[0]) < 1e-4 && Math.abs(a[1] - b[1]) < 1e-4
  );
}
function loadPng(dir, name) {
  if (!name) return { error: "no snapshot recorded" };
  const p = path.join(dir, name);
  if (!fs.existsSync(p)) return { error: `missing ${name}` };
  try {
    return { img: png.decode(fs.readFileSync(p)) };
  } catch (e) {
    return { error: `${name}: ${e.message}` };
  }
}
function judge(m, dir) {
  const checks = [];
  const add = (id, ok, detail) => checks.push({ id, ok: !!ok, detail });
  const inv = m.inventory || {};
  const web = m.web || {};
  const tasks = m.tasks || {};

  // Inventory: find the header, hover it, find the child row, snapshot,
  // click, find the child row again, snapshot. Six checks; the last one,
  // inventory_changed, is red unless the hover (a), the click's cursor (b)
  // and the child row's flip (c) ALL hold as well as the pixel diff. A
  // pixel diff alone cannot tell a dead click from a live one: the
  // pointer's own arrival draws egui's floating scrollbar (see the header
  // comment), so two frames differ whether or not the click did anything.
  const invA = loadPng(dir, inv.before);
  const invB = loadPng(dir, inv.after);
  add("inventory_snapshots", invA.img && invB.img, invA.error || invB.error || `${inv.before}, ${inv.after}`);
  const find = inv.find || null;
  const findOk = !!(find && find.ok === true && find.found === true && Array.isArray(find.uv));
  add(
    "inventory_find",
    findOk,
    find ? (find.error || `"${find.text}" at uv ${JSON.stringify(find.uv)} (${find.matches} match(es))`) : "never ran"
  );
  // (a) The hover landed where the find pointed, BEFORE the first snapshot,
  // so both snapshots carry the pointer in the same place.
  const hover = inv.hover || null;
  const hoverOk = !!(hover && hover.ok === true && findOk && uvClose(hover.uv, find.uv));
  add(
    "inventory_hover",
    hoverOk,
    hover
      ? (hover.error ||
        `${findOk && uvClose(hover.uv, find.uv) ? "at" : "NOT at"} the found uv: hovered ${JSON.stringify(hover.uv || null)} cursor=${hover.cursor_icon}`)
      : "never ran"
  );
  // (b) The click landed ON the header row: egui reported a layer under the
  // pointer AND the row's own cursor. inventory.rs sets PointingHand from
  // row.hovered(), so Default means the point was on the panel, not the row.
  const click = inv.click || null;
  const clickOk = !!(click && click.ok === true && click.hover_widget === true && click.cursor_icon === "PointingHand");
  add(
    "inventory_click",
    clickOk,
    click
      ? (click.error ||
        `hover_widget=${click.hover_widget} cursor=${click.cursor_icon}${click.cursor_icon === "PointingHand" ? "" : " (the header row reports PointingHand)"} focused=${click.focused}`)
      : "never ran"
  );
  // (c) The click's EFFECT, semantically: a row drawn only while the
  // container is open must be present on one side of the click and absent
  // on the other. Either direction is a toggle; no change is a dead click.
  const cb = inv.child_before || null;
  const ca = inv.child_after || null;
  const childOk = !!(
    cb && ca && cb.ok === true && ca.ok === true &&
    typeof cb.found === "boolean" && typeof ca.found === "boolean" && cb.found !== ca.found
  );
  const childName = inv.child_text || (cb && cb.text) || "?";
  add(
    "inventory_toggled",
    childOk,
    cb && ca
      ? (cb.error || ca.error ||
        `"${childName}" found before=${cb.found} after=${ca.found}${childOk ? (cb.found ? " (the click closed the container)" : " (the click opened the container)") : " (no change: the click did not toggle the container)"}`)
      : "never ran"
  );
  if (invA.img && invB.img) {
    const d = png.diffPixels(invA.img, invB.img);
    const pixelsOk = !d.sizeMismatch && d.differing > 0;
    const missing = [];
    if (!pixelsOk) missing.push(d.sizeMismatch ? "snapshot sizes differ" : "no pixel changed");
    if (!hoverOk) missing.push("hover not at the target");
    if (!clickOk) missing.push("click not on the header row");
    if (!childOk) missing.push("child row did not flip");
    add(
      "inventory_changed",
      pixelsOk && hoverOk && clickOk && childOk,
      `${d.differing} of ${d.total} pixels changed after the click${missing.length ? `; NOT proven: ${missing.join(", ")}` : "; hover, cursor and child row all agree"}`
    );
  } else {
    add("inventory_changed", false, "no pair of snapshots to compare");
  }

  // Web: loaded on our host, a link clicked, loaded again on a NEW url.
  const ready = web.ready || null;
  add(
    "web_ready",
    ready && ready.ok === true && ready.status === "ready" && onOurHost(ready.url),
    ready ? (ready.error || `${ready.status} ${ready.url} title=${JSON.stringify(ready.title)}${ready.waited_ms != null ? ` after ${ready.waited_ms} ms` : ""}`) : "never ran"
  );
  const link = web.link || null;
  add("web_link", link && link.ok === true, link ? (link.error || `link ${web.link_index ?? 0} clicked at uv ${JSON.stringify(link.uv || null)}`) : "never ran");
  // The new page must be a DIFFERENT url, loaded, and still on our host: the
  // gate follows a link on our own site only (the rig picks the index from
  // the hrefs the screen reports), and a redirect elsewhere must not pass.
  const ready2 = web.ready2 || null;
  add(
    "web_navigated",
    ready && ready2 && ready2.ok === true && ready2.status === "ready" && ready2.url && ready2.url !== ready.url && onOurHost(ready2.url),
    ready2 ? (ready2.error || `${ready2.status} ${ready2.url} title=${JSON.stringify(ready2.title)}`) : "never ran"
  );
  const webA = loadPng(dir, web.before);
  const webB = loadPng(dir, web.after);
  if (webA.img && webB.img) {
    const d = png.diffPixels(webA.img, webB.img);
    add("web_changed", !d.sizeMismatch && d.differing > 0, d.sizeMismatch ? "snapshot sizes differ" : `${d.differing} of ${d.total} pixels changed after the link`);
  } else {
    add("web_changed", false, webA.error || webB.error || "no pair of snapshots to compare");
  }

  // Tasks: the page drew (not one flat colour).
  const t = loadPng(dir, tasks.png);
  if (t.img) {
    const s = png.colourStats(t.img);
    add(
      "tasks_not_blank",
      s.distinct >= 2 && s.dominantShare < 0.995,
      `${s.distinct} distinct colours, dominant colour ${(s.dominantShare * 100).toFixed(2)}% of ${s.total} pixels`
    );
  } else {
    add("tasks_not_blank", false, t.error);
  }

  const panics = Number(m.panics || 0);
  add("no_panics", panics === 0, `${panics} PANIC line(s) in run.log`);
  return { checks, pass: checks.every((c) => c.ok) };
}

function printVerdict(verdictPrefix, m, dir) {
  const { checks, pass } = judge(m, dir);
  console.log("");
  console.log("-".repeat(72));
  for (const c of checks) console.log(`${c.ok ? "PASS" : "FAIL"}  ${pad(c.id, 20)} ${c.detail}`);
  console.log("-".repeat(72));
  if (pass) {
    console.log(`${verdictPrefix}PASS  ${checks.length}/${checks.length} screen checks passed`);
  } else {
    const failed = checks.filter((c) => !c.ok).map((c) => c.id);
    console.log(`${verdictPrefix}FAIL  ${checks.length - failed.length}/${checks.length} passed; failed: ${failed.join(", ")}`);
  }
  if (m.screenshot) console.log(`        viewport ${path.join(rel(dir), m.screenshot)}`);
  console.log(`        evidence ${rel(dir)}`);
  if (m.log) console.log(`        log      ${rel(path.resolve(dir, m.log))}`);
  console.log("-".repeat(72));
  console.log("");
  return pass;
}

// ── --self-test: the verdict must be able to fail ────────────────────────────
if (SELF_TEST) {
  const fx = path.join(REPO, "tests", "fixtures", "screens");
  const load = (name) => ({
    name,
    dir: path.join(fx, name),
    m: JSON.parse(fs.readFileSync(path.join(fx, name, "manifest.json"), "utf8")),
  });
  // Each fixture with EXACTLY the checks it must fail (in verdict order),
  // and nothing else. Any drift in the verdict logic (a check that stops
  // firing, or one that fires on good evidence) shows up here.
  //   green       every check passes.
  //   red         identical inventory snapshots, a hover off the target, a
  //               click whose cursor is Default (on the panel, not the
  //               row), a child row still found after the click, a web view
  //               that errored on the same url, a flat tasks snapshot, one
  //               panic. inventory_find and inventory_snapshots must still
  //               pass: a verdict that fails everything proves nothing.
  //   dead-click  THE REVIEWER'S SCENARIO (2026-09-17): hover and cursor
  //               both good, the two snapshots differ ONLY by a
  //               scrollbar-like column, and the child row is still found
  //               after the click. The old verdict passed this; it must
  //               fail on the child row and, through it, inventory_changed.
  const fixtures = [
    { ...load("green"), expect: [] },
    {
      ...load("red"),
      expect: [
        "inventory_hover", "inventory_click", "inventory_toggled", "inventory_changed",
        "web_ready", "web_navigated", "web_changed", "tasks_not_blank", "no_panics",
      ],
    },
    { ...load("dead-click"), expect: ["inventory_toggled", "inventory_changed"] },
  ];
  console.log("verify-screens --self-test: judging the fixture manifests (nothing is booted)");
  const problems = [];
  for (const f of fixtures) {
    const r = judge(f.m, f.dir);
    printVerdict(`DRY VERDICT (nothing was booted) [${f.name}]: `, f.m, f.dir);
    const got = r.checks.filter((c) => !c.ok).map((c) => c.id);
    if (JSON.stringify(got) !== JSON.stringify(f.expect)) {
      problems.push(
        f.expect.length
          ? `the ${f.name} fixture must fail exactly [${f.expect.join(", ")}], got [${got.join(", ")}]`
          : `the ${f.name} fixture must pass; failed: ${got.join(", ")}`
      );
    }
  }
  if (problems.length) {
    console.error("SELF-TEST FAILED:");
    for (const p of problems) console.error(`  ${p}`);
    process.exit(1);
  }
  console.log("SELF-TEST OK: green passes; red and dead-click fail on exactly their expected checks.");
  process.exit(0);
}

// ── --dry-verdict: re-judge an existing manifest ─────────────────────────────
if (DRY) {
  const manifest = path.resolve(DRY);
  if (!fs.existsSync(manifest)) refuse([`--dry-verdict: no such manifest: ${manifest}`]);
  const m = JSON.parse(fs.readFileSync(manifest, "utf8"));
  console.log(`[dry] verdict only, nothing booted: ${rel(manifest)}`);
  const pass = printVerdict("DRY VERDICT (nothing was booted): ", m, path.dirname(manifest));
  process.exit(pass ? 0 : 2);
}

// ── Preconditions for a live run ─────────────────────────────────────────────
console.log("");
console.log("verify-screens  real binary, real world entry, real clicks on the wall screens");
console.log("");

// 1. ONE GPU: nothing else may be running, the operator's game included.
function runningInstances() {
  try {
    const out = execSync(
      'powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \\"Name=\'HumanityOS.exe\'\\" | ForEach-Object { $_.ProcessId.ToString() + \'|\' + $_.ExecutablePath }"',
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }
    );
    return out
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => {
        const [pid, exe] = l.split("|");
        return { pid: Number(pid), exe: exe || "" };
      });
  } catch {
    return [];
  }
}
const instances = runningInstances();
if (instances.length) {
  refuse([
    `REFUSED: HumanityOS.exe is already running (pid ${instances.map((p) => `${p.pid} ${p.exe}`).join("; ")}).`,
    "One GPU, one instance (CLAUDE.md): a second renderer bogs the whole machine down and",
    "would steal the operator's game. Wait for it to exit, then run again. If it is a",
    `leftover rig of ours (exe under ${rel(RIG)}): taskkill //PID <pid> //F`,
  ]);
}

// 2. The binary must be the current build.
const fresh = spawnSync(process.execPath, [path.join(__dirname, "check-fresh-exe.js"), "--exe", EXE], { cwd: REPO, stdio: "inherit" });
if (fresh.status !== 0) {
  console.error("verify-screens: REFUSED - see the freshness failure above. Nothing was booted.");
  process.exit(1);
}

// ── Rig setup (the probe-sweep recipe, in this rig's own folder) ─────────────
function ensureJunction(link, target) {
  try {
    const st = fs.lstatSync(link);
    if (st.isSymbolicLink() || st.isDirectory()) {
      // A junction resolves to its target; a stale real dir is replaced.
      try {
        if (fs.realpathSync(link).toLowerCase() === fs.realpathSync(target).toLowerCase()) return;
      } catch {}
      fs.rmSync(link, { recursive: true, force: true });
    }
  } catch {}
  fs.symlinkSync(target, link, "junction");
}
function killRigProcesses() {
  const rigExe = path.join(RIG, "HumanityOS.exe").replace(/'/g, "''");
  try {
    execSync(
      `powershell -NoProfile -Command "Get-Process HumanityOS -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${rigExe}' } | Stop-Process -Force"`,
      { stdio: "ignore" }
    );
  } catch {}
}
function setupRig() {
  fs.mkdirSync(RIG, { recursive: true });
  fs.mkdirSync(DEBUG, { recursive: true });
  fs.mkdirSync(path.join(RIG, "logs"), { recursive: true });
  // portable.txt: identity/config/saves stay inside the rig, and the dev
  // autopilot runs (it refuses against a real installed identity).
  fs.writeFileSync(path.join(RIG, "portable.txt"), "verify-screens rig\n");
  // no_focus.txt: the engine boots background even if the env var is lost.
  fs.writeFileSync(path.join(RIG, "no_focus.txt"), "engine marker: boot background, never steal focus.\n");
  ensureJunction(path.join(RIG, "data"), path.join(REPO, "data"));
  ensureJunction(path.join(RIG, "assets"), path.join(REPO, "assets"));
  if (!fs.existsSync(EXE)) refuse([`ERROR: exe not found: ${EXE}`, "  build one first: cargo build --features native --release"]);
  killRigProcesses();
  try {
    fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  } catch (e) {
    if (e.code !== "EBUSY") throw e;
    execSync("ping -n 3 127.0.0.1 >nul", { shell: "cmd.exe" });
    fs.copyFileSync(EXE, path.join(RIG, "HumanityOS.exe"));
  }
  // DXC dlls beside the exe drop boot from ~25 s to ~5 s.
  for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
    const s = path.join(path.dirname(EXE), dll);
    if (fs.existsSync(s)) fs.copyFileSync(s, path.join(RIG, dll));
  }
  for (const f of fs.readdirSync(DEBUG)) {
    if (/\.png$/.test(f) || /_done\.json$/.test(f) || /_request\.json$/.test(f)) fs.unlinkSync(path.join(DEBUG, f));
  }
  if (fs.existsSync(LOG)) fs.truncateSync(LOG, 0);
}

// The switch, written BEFORE boot (the engine reads config.json once at
// startup). Merged into whatever the rig's config already holds so a
// previous run's graphics settings survive; the field alone is what this
// gate needs, and AppConfig fills every other field from its defaults.
function writeReadableWeb() {
  const cfgPath = G.rigConfigPath(RIG);
  let cfg = {};
  const existing = G.readConfig(cfgPath);
  if (existing.ok && existing.cfg && typeof existing.cfg === "object") cfg = existing.cfg;
  cfg.readable_web = true;
  fs.mkdirSync(path.dirname(cfgPath), { recursive: true });
  fs.writeFileSync(cfgPath, JSON.stringify(cfg, null, 2) + "\n");
  log(`readable_web: true written to ${rel(cfgPath)}`);
  return cfgPath;
}

// ── IPC helpers ──────────────────────────────────────────────────────────────
function clearDone(...names) {
  for (const n of names) {
    const p = path.join(DEBUG, n);
    if (fs.existsSync(p)) fs.unlinkSync(p);
  }
}
async function waitFile(name, timeoutMs, pollMs = 300) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(p)) {
      try {
        return JSON.parse(fs.readFileSync(p, "utf8"));
      } catch {
        // half-written; retry
      }
    }
    await sleep(pollMs);
  }
  return null;
}
function req(name, body) {
  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));
}
async function waitBoot(timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (fs.existsSync(LOG)) {
      const txt = fs.readFileSync(LOG, "utf8");
      if (/PANIC/.test(txt)) throw new Error("PANIC during boot (see run.log)");
      if (/Cloud noise volumes generated/.test(txt)) return true;
    }
    await sleep(1000);
  }
  throw new Error("the exe did not finish booting in time");
}
// One screen request. The engine answers "no screen named ..." while the
// home is still being built after world entry, so that one error is
// retried for up to `placingMs`; every other answer is returned as-is.
async function screen(body, timeoutMs = 30000, placingMs = 60000) {
  const t0 = Date.now();
  for (;;) {
    clearDone("screen_done.json");
    req("screen_request.json", body);
    const d = await waitFile("screen_done.json", timeoutMs);
    if (!d) return { ok: false, error: `no screen_done.json within ${timeoutMs} ms for ${JSON.stringify(body)}` };
    if (d.ok === false && /no screen named/.test(d.error || "") && Date.now() - t0 < placingMs) {
      await sleep(1000);
      continue;
    }
    return d;
  }
}
function panicCount() {
  if (!fs.existsSync(LOG)) return 0;
  return (fs.readFileSync(LOG, "utf8").match(/PANIC/g) || []).length;
}

// ── The live run ─────────────────────────────────────────────────────────────
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const OUT = path.join(RIG, "runs", stamp);

async function main() {
  setupRig();
  const cfgPath = writeReadableWeb();
  fs.mkdirSync(OUT, { recursive: true });
  const manifest = {
    kind: "verify-screens",
    stamp,
    exe: EXE,
    rig: RIG,
    config: cfgPath,
    log: path.relative(OUT, LOG),
    screens: SCREENS,
    web_host: WEB_HOST,
    steps: [],
    inventory: {},
    web: {},
    tasks: {},
    panics: 0,
  };
  const save = () => fs.writeFileSync(path.join(OUT, "manifest.json"), JSON.stringify(manifest, null, 2));
  const step = (id, d) => {
    manifest.steps.push({ id, ...(d && typeof d === "object" ? d : { value: d }) });
    save();
    const ok = d && d.ok !== false;
    log(`${ok ? "ok  " : "FAIL"} ${pad(id, 18)} ${d && d.error ? d.error : summarize(d)}`);
    return d;
  };
  // A snapshot's PNG lands under the rig cwd; copy it beside the manifest
  // under a stable name so the evidence folder stands on its own.
  const keepPng = (d, name) => {
    if (!d || !d.png) return null;
    const src = path.join(RIG, d.png);
    if (!fs.existsSync(src)) return null;
    fs.copyFileSync(src, path.join(OUT, name));
    return name;
  };

  log(`launching ${path.basename(EXE)} in ${rel(RIG)}`);
  const child = spawn(path.join(RIG, "HumanityOS.exe"), [], {
    cwd: RIG,
    detached: true,
    stdio: "ignore",
    // Background boot, never the operator's focus. The take-focus env var is
    // the operator's own opt-in (src/engine/launch_focus.rs) and is never set
    // by a script.
    env: { ...process.env, HUMANITY_NO_FOCUS: "1" },
  });
  const pid = child.pid;
  fs.writeFileSync(path.join(RIG, "probe_pid.txt"), String(pid));
  child.unref();
  let killed = false;
  const kill = () => {
    if (killed) return;
    killed = true;
    try {
      execSync(`taskkill /PID ${pid} /T /F`, { stdio: "ignore" });
    } catch {}
    killRigProcesses();
  };
  process.on("exit", () => {
    if (!KEEP_OPEN) kill();
  });
  const watchdog = setTimeout(() => {
    log(`TIMEOUT after ${TIMEOUT_MS / 60000} min; killing the rig`);
    manifest.steps.push({ id: "timeout", ok: false, error: `run exceeded ${TIMEOUT_MS / 60000} min` });
    manifest.panics = panicCount();
    save();
    kill();
    printVerdict("RESULT: ", manifest, OUT);
    process.exit(2);
  }, TIMEOUT_MS);

  try {
    log("waiting for boot...");
    await waitBoot(180000);
    log("entering world (autopilot)...");
    clearDone("autopilot_done.json");
    req("autopilot_request.json", { server_url: "" });
    const ap = await waitFile("autopilot_done.json", 180000);
    if (!ap || ap.ok !== true) throw new Error(`autopilot failed: ${JSON.stringify(ap)}`);
    step("autopilot", ap);

    // Park in the console room facing the web screen. The home is built a
    // few frames after world entry; the camera request names the screen,
    // so retry while it answers "no screen named".
    log("parking in the console room...");
    let cam = null;
    for (let t0 = Date.now(); Date.now() - t0 < 90000; ) {
      clearDone("camera_done.json");
      req("camera_request.json", { station: "home", screen: SCREENS.web, distance_m: 2.2 });
      cam = await waitFile("camera_done.json", 30000);
      if (cam && cam.ok === true) break;
      if (cam && /no screen named/.test(cam.error || "")) {
        await sleep(1000);
        continue;
      }
      break;
    }
    if (!cam || cam.ok !== true) throw new Error(`camera park failed: ${JSON.stringify(cam)}`);
    step("camera", cam);
    // Let the surfaces draw a few frames and the station settle before the
    // first snapshot.
    await sleep(4000);

    // (a) INVENTORY. The ORDER is the point (see the header comment): find
    // the header, HOVER it, find the child row, snapshot, click, find the
    // child row again, snapshot. The hover before the first snapshot puts
    // the pointer where it will stay for the whole leg (the camera faces
    // the web wall, so the look ray never touches this surface and never
    // takes the pointer away), so the only difference left between the two
    // snapshots is what the click did.
    const find = step("inv_find", await screen({ screen: SCREENS.inventory, find: { text: INVENTORY_TARGET } }));
    manifest.inventory.find = find;
    manifest.inventory.child_text = INVENTORY_CHILD;
    const nothingToClick = { ok: false, error: `no "${INVENTORY_TARGET}" text on the inventory screen, nothing to hover or click` };
    if (find.ok && find.found) {
      manifest.inventory.hover = step("inv_hover", await screen({ screen: SCREENS.inventory, action: "hover", uv: find.uv }));
    } else {
      manifest.inventory.hover = nothingToClick;
      step("inv_hover", manifest.inventory.hover);
    }
    manifest.inventory.child_before = step("inv_child", await screen({ screen: SCREENS.inventory, find: { text: INVENTORY_CHILD } }));
    const invA = step("inv_snapshot", await screen({ screen: SCREENS.inventory, action: "snapshot" }));
    manifest.inventory.before = keepPng(invA, "inventory_before.png");
    if (find.ok && find.found) {
      manifest.inventory.click = step("inv_click", await screen({ screen: SCREENS.inventory, action: "click", uv: find.uv }));
    } else {
      manifest.inventory.click = nothingToClick;
      step("inv_click", manifest.inventory.click);
    }
    manifest.inventory.child_after = step("inv_child2", await screen({ screen: SCREENS.inventory, find: { text: INVENTORY_CHILD } }));
    const invB = step("inv_snapshot2", await screen({ screen: SCREENS.inventory, action: "snapshot" }));
    manifest.inventory.after = keepPng(invB, "inventory_after.png");
    save();

    // (b) WEB: wait for the page, snapshot, click a same-host link, wait,
    // snapshot.
    const ready = step("web_ready", await screen({ screen: SCREENS.web, action: "wait_ready" }, 30000));
    manifest.web.ready = ready;
    const webA = step("web_snapshot", await screen({ screen: SCREENS.web, action: "snapshot" }));
    manifest.web.before = keepPng(webA, "web_before.png");
    // Which link: the first one that stays on our own site (the screen
    // reports the hrefs it drew, in link order, in `links`). Nothing
    // third-party is ever fetched by this gate, so a page that drew no such
    // link gets NO click at all: web_link and web_navigated then fail with
    // that reason, which is the right answer (the home page must offer a
    // link into itself, or the wall cannot be proven navigable).
    const links = Array.isArray(ready.links) ? ready.links : [];
    const linkIndex = links.findIndex(onOurHost);
    if (linkIndex < 0) {
      manifest.web.link_index = null;
      manifest.web.link_target = null;
      manifest.web.link = { ok: false, error: `no link on ${WEB_HOST} among the ${links.length} drawn; nothing clicked (this gate never follows a link off our own site)` };
      step("web_link", manifest.web.link);
    } else {
      manifest.web.link_index = linkIndex;
      manifest.web.link_target = links[linkIndex];
      log(`link ${linkIndex} -> ${manifest.web.link_target}`);
      manifest.web.link = step("web_link", await screen({ screen: SCREENS.web, link: { index: linkIndex } }));
    }
    manifest.web.ready2 = step("web_ready2", await screen({ screen: SCREENS.web, action: "wait_ready" }, 30000));
    const webB = step("web_snapshot2", await screen({ screen: SCREENS.web, action: "snapshot" }));
    manifest.web.after = keepPng(webB, "web_after.png");
    save();

    // (d) TASKS: one snapshot, judged for not being a flat colour.
    const tasks = step("tasks_snapshot", await screen({ screen: SCREENS.tasks, action: "snapshot" }));
    manifest.tasks.png = keepPng(tasks, "tasks.png");
    manifest.tasks.done = tasks;

    // A viewport capture for the human reading the evidence folder: the
    // console room with the screens on its walls as the player sees them.
    clearDone("screenshot_done.json");
    req("screenshot_request.json", { note: "verify-screens console room" });
    const shot = await waitFile("screenshot_done.json", 30000);
    if (shot && shot.ok && shot.path) {
      const src = path.isAbsolute(shot.path) ? shot.path : path.join(RIG, shot.path);
      if (fs.existsSync(src)) {
        fs.copyFileSync(src, path.join(OUT, "viewport.png"));
        manifest.screenshot = "viewport.png";
      }
    }
    step("viewport", shot || { ok: false, error: "no screenshot_done.json" });
  } catch (e) {
    manifest.steps.push({ id: "abort", ok: false, error: String(e.message || e) });
    log(`ABORT ${e.message || e}`);
  }
  clearTimeout(watchdog);
  // (c) Panics, read after everything else so a late one still counts.
  manifest.panics = panicCount();
  // The rig's log, copied beside the manifest so the folder is complete.
  try {
    fs.copyFileSync(LOG, path.join(OUT, "run.log"));
    manifest.log = "run.log";
  } catch {}
  save();
  if (!KEEP_OPEN) kill();
  else log(`--keep-open: the rig is still running (pid ${pid}); kill it with taskkill //PID ${pid} //T //F`);
  const pass = printVerdict("RESULT: ", manifest, OUT);
  if (!pass) {
    console.log("What to do:");
    console.log(`  1. open the PNGs in ${rel(OUT)} (before/after pairs, the tasks page, the viewport)`);
    console.log(`  2. read ${rel(path.join(OUT, "run.log"))} for [Screens] and web_reader lines (and any PANIC)`);
    console.log("  3. re-judge without booting: node scripts/verify-screens.js --dry-verdict " + rel(path.join(OUT, "manifest.json")));
    console.log("");
  }
  process.exit(pass ? 0 : 2);
}

function summarize(d) {
  if (!d || typeof d !== "object") return String(d);
  const parts = [];
  if (d.status) parts.push(`status=${d.status}`);
  if (d.url) parts.push(`url=${d.url}`);
  if (d.found !== undefined) parts.push(`found=${d.found}${d.uv ? ` uv=${JSON.stringify(d.uv)}` : ""}`);
  if (d.hover_widget !== undefined) parts.push(`hover_widget=${d.hover_widget}`);
  if (d.cursor_icon !== undefined) parts.push(`cursor=${d.cursor_icon}`);
  if (d.png) parts.push(d.png);
  if (d.screen && !d.png && !d.status) parts.push(`screen=${d.screen}`);
  if (d.position) parts.push(`position=${JSON.stringify(d.position)}`);
  if (d.public_key_prefix) parts.push(`identity=${d.public_key_prefix}`);
  return parts.join(" ");
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});
