// Load each platform's OWN embedded player in the offscreen browser and record
// what it did. Twice over: once from a page opened as a file on disk, and once
// from the same page served by a local HTTP server on 127.0.0.1.
//
// The second half is the one that matters. If a platform accepts a page served
// from localhost, the watch page can ship inside the app and be served by the
// app's own server, and our website is not in the path at all: no bandwidth, no
// uptime dependency, and it keeps working when the site is down.
//
// A refusal is a result, not a failure. It is recorded exactly as the platform
// phrased it.
//
// Evidence per scenario: two PNGs several seconds apart, the browser console,
// the page title (which the test page sets to whatever the player reported),
// and the share of pixels that changed between the two PNGs. A player that is
// really playing changes a large share of the picture; a still refusal notice
// changes almost none. That pixel share is the check that cannot be fooled by
// a page claiming success.
//
// Usage:  node run-embed-tests.js [--only=<name>] [--seconds=20]

const { spawn, spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const png = require("../../scripts/lib/png.js");

const HERE = __dirname;
const CEF_DIR = process.env.CEF_PATH || "C:/Users/Shaos/AppData/Local/cef-sdk/cef";
const EXE = path.join(HERE, "target", "release", "cef-probe.exe");
const RUNS = process.env.PROBE_RUNS || "C:/Users/Shaos/AppData/Local/cef-sdk/runs";
const PORT = 8788;

const argv = Object.fromEntries(
  process.argv.slice(2).map((a) => {
    const m = a.replace(/^--/, "").split("=");
    return [m[0], m[1] ?? "1"];
  })
);
const SECONDS = Number(argv.seconds || 20);

// A file:// URL for the test page, and the same page over local HTTP.
const fileBase = "file:///" + path.join(HERE, "pages", "embed.html").replace(/\\/g, "/");
const httpBase = `http://localhost:${PORT}/embed.html`;

// The three platforms, each with its official embed. The Rumble embed id came
// from Rumble's own oEmbed endpoint; the YouTube id is the first video ever
// uploaded to the site, which has been publicly embeddable for twenty years.
const PLATFORMS = [
  { name: "youtube", query: "platform=youtube&id=jNQXAC9IVRw" },
  { name: "twitch", query: "platform=twitch&channel=twitch" },
  { name: "rumble", query: "platform=rumble&id=v17j3tt" },
];

const SCENARIOS = [];
for (const p of PLATFORMS) {
  // From a file on disk. Twitch needs a host name to declare as the embedding
  // parent and a file has none, so "localhost" is offered anyway: that makes
  // the refusal, if it comes, about the page's ORIGIN and not about a missing
  // parameter.
  SCENARIOS.push({
    name: `${p.name}-file`,
    url: `${fileBase}?${p.query}${p.name === "twitch" ? "&parent=localhost" : ""}`,
  });
}
for (const p of PLATFORMS) {
  SCENARIOS.push({
    name: `${p.name}-localhost`,
    url: `${httpBase}?${p.query}${p.name === "twitch" ? "&parent=localhost" : ""}`,
  });
}

function runProbe(scenario) {
  const out = path.join(RUNS, "embed-" + scenario.name);
  fs.rmSync(out, { recursive: true, force: true });
  fs.mkdirSync(out, { recursive: true });
  const args = [
    `--url=${scenario.url}`,
    "--width=1280",
    "--height=720",
    "--fps=60",
    `--seconds=${SECONDS}`,
    "--warmup=12",
    "--settle=8",
    "--png-at=4",
    `--out=${out}`,
    `--cef-path=${CEF_DIR}`,
    `--label=${scenario.name}`,
  ];
  const env = { ...process.env, PATH: CEF_DIR + path.delimiter + process.env.PATH };
  const r = spawnSync(EXE, args, { env, encoding: "utf8", timeout: (SECONDS + 90) * 1000 });
  fs.writeFileSync(path.join(out, "stdout.txt"), (r.stdout || "") + "\n---stderr---\n" + (r.stderr || ""));
  return out;
}

function pixelChange(dir) {
  try {
    const a = png.decode(fs.readFileSync(path.join(dir, "frame.png")));
    const b = png.decode(fs.readFileSync(path.join(dir, "frame_end.png")));
    const d = png.diffPixels(a, b);
    const colours = png.colourStats(b);
    return {
      changedShare: d.differing / d.total,
      distinctColours: colours.distinct,
      dominantShare: colours.dominantShare,
    };
  } catch (e) {
    return { error: String(e) };
  }
}

function main() {
  if (!fs.existsSync(EXE)) {
    console.error("build the probe first: cargo build --release in tools/cef-probe");
    process.exit(1);
  }

  // The local server stands in for the app's own HTTP server.
  const server = spawn(process.execPath, [path.join(HERE, "serve.js"), String(PORT)], {
    stdio: ["ignore", "inherit", "inherit"],
  });

  const results = [];
  const finish = () => {
    server.kill();
    const summary = path.join(RUNS, "embed-summary.json");
    fs.writeFileSync(summary, JSON.stringify(results, null, 2));
    console.log("\n================ EMBED RESULTS ================");
    for (const r of results) {
      console.log(
        `${r.name.padEnd(20)} title=${JSON.stringify(r.title).padEnd(28)} ` +
          `pixels changed ${(r.pixels.changedShare * 100 || 0).toFixed(1)}%  ` +
          `colours ${r.pixels.distinctColours}`
      );
      for (const line of r.verdictLines) console.log("    " + line);
    }
    console.log("\nwrote " + summary);
  };

  // Give the server a moment to bind, then run the scenarios one at a time:
  // two browsers at once would contend for the GPU and muddy every number.
  setTimeout(() => {
    for (const s of SCENARIOS) {
      if (argv.only && s.name !== argv.only) continue;
      console.log(`\n---- ${s.name} ----\n${s.url}`);
      const out = runProbe(s);
      let json = {};
      try {
        json = JSON.parse(fs.readFileSync(path.join(out, "result.json"), "utf8"));
      } catch (e) {
        json = { title: "(no result.json)", console: [], load_errors: [] };
      }
      const pixels = pixelChange(out);
      // Pull out the console lines that say what the platform decided.
      const interesting = (json.console || []).filter((l) =>
        /\[embed\]|Refused|frame-ancestors|X-Frame-Options|ERR_|blocked|CORS|error/i.test(l)
      );
      results.push({
        name: s.name,
        url: s.url,
        title: json.title,
        fps: json.fps,
        pixels,
        loadErrors: json.load_errors || [],
        console: json.console || [],
        verdictLines: interesting.slice(0, 12),
        dir: out,
      });
      console.log(
        `  -> title ${JSON.stringify(json.title)}, ${(pixels.changedShare * 100 || 0).toFixed(1)}% of pixels changed`
      );
    }
    finish();
  }, 1200);
}

main();
