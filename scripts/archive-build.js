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
const EXE_PATTERN = /^v(\d+)\.(\d+)\.(\d+)_HumanityOS\.exe$/;
const MAX_VERSIONS = 5;
// Stable, unversioned copy in the repo root. The operator pins THIS to the
// Windows taskbar once; every build refreshes it in place so the shortcut
// always launches the latest build, and purgeOld never touches it (it does not
// match EXE_PATTERN). gitignored (/HumanityOS.exe). (v0.476)
const STABLE_EXE = path.join(BINDIR, "HumanityOS.exe");

// Semver-aware comparison for filenames like "v0.105.1_HumanityOS.exe".
// Lexicographic sort is wrong for multi-digit components: "v0.105.1" < "v0.97.0"
// alphabetically because '1' < '9'. Compare numeric components instead.
function compareSemverDesc(a, b) {
  const ma = a.match(EXE_PATTERN);
  const mb = b.match(EXE_PATTERN);
  if (!ma || !mb) return a < b ? 1 : a > b ? -1 : 0;
  for (let i = 1; i <= 3; i++) {
    const da = parseInt(ma[i], 10);
    const db = parseInt(mb[i], 10);
    if (da !== db) return db - da; // newest first
  }
  return 0;
}

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
    .sort(compareSemverDesc); // newest first by semver, not lexicographic
}

function getLatestExe() {
  const exes = listExes();
  return exes.length ? path.join(BINDIR, exes[0]) : null;
}

function killRunning() {
  // Kill any running build whose exe we are about to overwrite: the versioned
  // archives (v*HumanityOS*) AND the stable taskbar copy (HumanityOS.exe in the
  // root). Without releasing the stable one, a build can't refresh it while the
  // operator has it open from the taskbar. target/release/HumanityOS.exe is NOT
  // matched (the build owns that; cargo handles its own lock).
  const stableForPs = STABLE_EXE.replace(/'/g, "''"); // PS single-quote escape
  try {
    execSync(
      `powershell -Command "Get-Process | Where-Object { $_.Path -like 'C:\\Humanity\\v*HumanityOS*' -or $_.Path -eq '${stableForPs}' } | Stop-Process -Force -ErrorAction SilentlyContinue"`,
      { stdio: "ignore" }
    );
  } catch (e) {
    // no process to kill
  }
  // Brief pause so Windows releases the file lock
  execSync("ping -n 2 127.0.0.1 > nul", { stdio: "ignore" });
}

// Refresh the stable, unversioned HumanityOS.exe (+ its signature sidecar) so
// the operator's pinned taskbar shortcut always points at the latest build.
// Best-effort: a lock (exe still running) is warned, not fatal.
function refreshStable(dest) {
  try {
    // killRunning() above already stopped a running stable copy, so the fast
    // path normally works. Fallback for a still-locked target: a running exe on
    // Windows can be RENAMED even while locked, so move it aside to
    // HumanityOS.old.exe (gitignored: *.old.exe) and copy the new one in. The
    // old instance keeps running off the renamed file until the user closes it.
    try {
      fs.copyFileSync(dest, STABLE_EXE);
    } catch (lockErr) {
      const old = STABLE_EXE.replace(/\.exe$/, ".old.exe");
      try {
        fs.unlinkSync(old);
      } catch (e) {
        /* none */
      }
      fs.renameSync(STABLE_EXE, old);
      fs.copyFileSync(dest, STABLE_EXE);
    }
    // Carry the signature sidecar so the stable copy is trusted by the updater
    // too (the sig is over file content, which the copy preserves byte-for-byte).
    const sidecar = dest + ".sig.json";
    if (fs.existsSync(sidecar)) {
      fs.copyFileSync(sidecar, STABLE_EXE + ".sig.json");
    } else {
      // No signature for this build - drop any stale sidecar so the stable copy
      // isn't paired with an old, now-invalid signature.
      try {
        fs.unlinkSync(STABLE_EXE + ".sig.json");
      } catch (e) {
        /* none */
      }
    }
    console.log(`  Stable: HumanityOS.exe refreshed (pin this to your taskbar)`);
  } catch (e) {
    console.warn(
      `  Could not refresh HumanityOS.exe (is it still running?): ${e.message}`
    );
  }
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
    // Remove the orphaned signature sidecar too, if present.
    try {
      fs.unlinkSync(full + ".sig.json");
    } catch (e) {
      /* no sidecar */
    }
  }
}

// Sign the archived build so the local launcher (find_newer_exe) will trust it
// (audit 2026-06-12). Opt-in + zero-friction: only signs when the operator's
// encrypted signing key is present AND HUMANITY_SIGNING_PASSPHRASE is set in
// the environment. Unsigned builds still launch directly via `just launch`;
// they just won't be auto-delegated-to from an older running build.
function signArchive(dest) {
  const keyFile = path.join(BINDIR, "release-signing-key.enc");
  if (!fs.existsSync(keyFile)) return; // signing not provisioned; nothing to do
  if (!process.env.HUMANITY_SIGNING_PASSPHRASE) {
    console.log("  (unsigned — set HUMANITY_SIGNING_PASSPHRASE to sign this build)");
    return;
  }
  try {
    execSync(`"${dest}" --sign-file "${dest}" "${keyFile}"`, { stdio: "ignore" });
    if (fs.existsSync(dest + ".sig.json")) {
      console.log(`  Signed: ${path.basename(dest)}.sig.json`);
    } else {
      console.warn("  Signing produced no sidecar (wrong passphrase?)");
    }
  } catch (e) {
    console.warn(`  Could not sign build: ${e.message}`);
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

  // DXC shader compiler DLLs (v0.865): the exe prefers DXC when
  // dxcompiler.dll + dxil.dll sit beside it (boot drops from ~25 s to ~5 s;
  // FXC fallback otherwise). Keep the repo-root copies fresh so the taskbar
  // exe gets the fast path too. Sourced from target/release (put there from
  // the Windows SDK bin dir or a DirectXShaderCompiler release).
  for (const dll of ["dxcompiler.dll", "dxil.dll"]) {
    const dsrc = path.join(BINDIR, "target", "release", dll);
    if (fs.existsSync(dsrc)) {
      try {
        fs.copyFileSync(dsrc, path.join(BINDIR, dll));
      } catch (e) {
        console.warn(`(warn) could not refresh ${dll}: ${e.message}`);
      }
    }
  }

  const mb = (fs.statSync(dest).size / 1048576).toFixed(1);
  console.log(`Archived: ${dest} (${mb} MB)`);

  signArchive(dest);
  refreshStable(dest);
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
