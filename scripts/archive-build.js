#!/usr/bin/env node
// Archive the release build to C:\Humanity\v{version}_HumanityOS.exe
// Keeps last 5 versioned exes, purges oldest when a 6th is created.
//
// Usage:
//   node archive-build.js             — archive only (kill running, copy exe)
//   node archive-build.js --launch    — archive then launch
//   node archive-build.js --launch-only — launch latest without archiving

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

// Repo root — exe lives next to data/ so no duplication
const BINDIR = path.join(__dirname, "..");
const EXE_PATTERN = /^v[\d.]+_HumanityOS\.exe$/;
const MAX_VERSIONS = 5;

function getVersion() {
  const cargo = fs.readFileSync(path.join(BINDIR, "Cargo.toml"), "utf8");
  const match = cargo.match(/^version = "(.+)"/m);
  if (!match) throw new Error("Could not read version from Cargo.toml");
  return match[1];
}

function listExes() {
  return fs
    .readdirSync(BINDIR)
    .filter((f) => EXE_PATTERN.test(f))
    .sort()
    .reverse(); // newest first
}

function getLatestExe() {
  const exes = listExes();
  return exes.length ? path.join(BINDIR, exes[0]) : null;
}

function killRunning() {
  try {
    execSync(
      'powershell -Command "Get-Process | Where-Object { $_.Path -like \'C:\\Humanity\\v*HumanityOS*\' } | Stop-Process -Force -ErrorAction SilentlyContinue"',
      { stdio: "ignore" }
    );
  } catch (e) {
    // no process to kill
  }
  // Brief pause so Windows releases the file lock
  execSync("ping -n 2 127.0.0.1 > nul", { stdio: "ignore" });
}

function purgeOld() {
  const exes = listExes(); // newest first
  if (exes.length <= MAX_VERSIONS) return;
  const toDelete = exes.slice(MAX_VERSIONS);
  for (const name of toDelete) {
    const full = path.join(BINDIR, name);
    try {
      fs.unlinkSync(full);
      console.log(`  Purged: ${name}`);
    } catch (e) {
      console.warn(`  Could not purge ${name}: ${e.message}`);
    }
  }
}

function archive() {
  const ver = getVersion();
  const dest = path.join(BINDIR, `v${ver}_HumanityOS.exe`);
  const src = path.join(BINDIR, "target", "release", "HumanityOS.exe");

  if (!fs.existsSync(src)) {
    console.error(
      `ERROR: ${src} not found. Run 'cargo build --features native --release' first.`
    );
    process.exit(1);
  }

  killRunning();
  fs.copyFileSync(src, dest);

  const mb = (fs.statSync(dest).size / 1048576).toFixed(1);
  console.log(`Archived: ${dest} (${mb} MB)`);

  purgeOld();
  return dest;
}

function launch(exePath) {
  if (!exePath) {
    exePath = getLatestExe();
  }
  if (!exePath || !fs.existsSync(exePath)) {
    console.error("No builds found. Run: just build-game");
    process.exit(1);
  }
  console.log(`Launching ${path.basename(exePath)}`);
  execSync(`start "" "${exePath}"`, { shell: true });
}

// CLI
const args = process.argv.slice(2);
if (args.includes("--launch-only")) {
  launch();
} else if (args.includes("--launch")) {
  const dest = archive();
  launch(dest);
} else {
  archive();
}
