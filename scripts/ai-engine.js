#!/usr/bin/env node
'use strict';
/**
 * ai-engine.js (`just ai-engine`, rung 2 of docs/design/embeddable-ai.md, 2026-11-09): build, test,
 * check and package the AI engine, humanity-ai.exe (crates/humanity-ai-engine/, a workspace of its
 * own that HumanityOS.exe never depends on).
 *
 *   node scripts/ai-engine.js build     the real engine (llama.cpp, CPU) into target/ai-engine/release/
 *   node scripts/ai-engine.js test      its tests: the pure-Rust ones, then the llama ones (a real
 *                                       model when one is on this PC, SKIPPED in words when not)
 *   node scripts/ai-engine.js check     type-check both builds, warnings as errors (budget 0)
 *   node scripts/ai-engine.js package   build, then the zip `assets-ai-1` carries, with its licences,
 *                                       and target/ai-engine/package/engine.json (its SHA-256 and size)
 *
 * What it adds around cargo, for the `llama` feature (build.rs):
 * - **The pinned llama.cpp source**: `cargo metadata` on crates/humanity-ai-engine/llama-src/ (which
 *   pins the crate llama-cpp-sys-2 =0.1.161 and its checksum) says where cargo unpacked it; its
 *   llama.cpp/ folder goes to the build as HUMANITY_LLAMA_SRC. Nothing is downloaded but crates,
 *   through cargo.
 * - **CMake**: the one on PATH, else the copy inside Visual Studio's Build Tools (the operator's PC
 *   has it there, not on PATH); handed to the build as CMAKE, never added to the system PATH.
 * - **Rust flags** for a repeatable program: its own C runtime linked in (+crt-static; it calls
 *   llama.cpp's libraries, which use the shared Visual C++ runtime, through C functions with plain
 *   values only: build.rs says why the libraries cannot link theirs in), no time stamp (/Brepro), no
 *   symbols, and the builder's folders written as neutral names (--remap-path-prefix), so the
 *   published engine carries nobody's user name.
 * - **Four compiles at a time** (CMAKE_BUILD_PARALLEL_LEVEL, CARGO_BUILD_JOBS): the operator keeps a
 *   share of his PC (CLAUDE.md).
 *
 * The app's client tests (src/ai/client_tests.rs) build the engine's debug copy WITHOUT `llama`
 * (plain cargo, no CMake) into target/ai-engine/debug/; this script's llama builds use the release
 * profile, so the two never overwrite each other.
 */

const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { spawnSync, execFileSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const ENGINE_DIR = path.join(ROOT, 'crates', 'humanity-ai-engine');
const MANIFEST = path.join(ENGINE_DIR, 'Cargo.toml');
const SRC_MANIFEST = path.join(ENGINE_DIR, 'llama-src', 'Cargo.toml');
/** Where cargo builds it. HUMANITY_AI_ENGINE_TARGET moves it, for the reproducibility check (the same
 * package built in a second, fresh folder must hash the same). */
const TARGET = process.env.HUMANITY_AI_ENGINE_TARGET ? path.resolve(process.env.HUMANITY_AI_ENGINE_TARGET) : path.join(ROOT, 'target', 'ai-engine');
const PACKAGE_DIR = path.join(TARGET, 'package');
/** The pinned crate whose llama.cpp/ folder the engine builds. */
const LLAMA_CRATE = 'llama-cpp-sys-2';
/** Where Visual Studio's Build Tools keep CMake (2022), when it is not on PATH. */
const VS_CMAKE = [
  'C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin\\cmake.exe',
  'C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin\\cmake.exe',
  'C:\\Program Files\\Microsoft Visual Studio\\2022\\Professional\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin\\cmake.exe',
];

function fail(words) {
  console.error(`ai-engine: ${words}`);
  process.exit(1);
}

/** The engine's version, from its Cargo.toml (the catalog and the zip's name carry it whole: since 1.1.0, 2026-11-12,
 * a new engine is a NEW file on the release, never a replacement for one an earlier app's catalog vouches for). */
function engineVersion() {
  const m = /^version\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(MANIFEST, 'utf8'));
  if (!m) fail(`no version in ${MANIFEST}`);
  return m[1];
}

/** The pinned llama.cpp source: { dir, version, checksum, vcs }. */
function llamaSource() {
  let meta;
  try {
    meta = JSON.parse(execFileSync('cargo', ['metadata', '--manifest-path', SRC_MANIFEST, '--format-version', '1', '--locked'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20, stdio: ['ignore', 'pipe', 'inherit'] }));
  } catch (e) {
    fail(`cargo metadata on ${SRC_MANIFEST} failed: ${e.message}`);
  }
  const pkg = meta.packages.find((p) => p.name === LLAMA_CRATE);
  if (!pkg) fail(`${SRC_MANIFEST} does not pin ${LLAMA_CRATE}`);
  const crateDir = path.dirname(pkg.manifest_path);
  const dir = path.join(crateDir, 'llama.cpp');
  if (!fs.existsSync(path.join(dir, 'CMakeLists.txt'))) fail(`${dir} is not a llama.cpp source folder`);
  const lock = fs.readFileSync(path.join(path.dirname(SRC_MANIFEST), 'Cargo.lock'), 'utf8');
  const block = lock.split('[[package]]').find((b) => b.includes(`name = "${LLAMA_CRATE}"`)) || '';
  const checksum = (/checksum = "([0-9a-f]{64})"/.exec(block) || [])[1] || '';
  let vcs = '';
  try {
    vcs = JSON.parse(fs.readFileSync(path.join(crateDir, '.cargo_vcs_info.json'), 'utf8')).git.sha1;
  } catch {
    /* not every crate records it */
  }
  return { dir, version: pkg.version, checksum, vcs, crateDir };
}

function findCmake() {
  if (process.env.CMAKE && fs.existsSync(process.env.CMAKE)) return process.env.CMAKE;
  const onPath = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['cmake'], { encoding: 'utf8' });
  if (onPath.status === 0 && onPath.stdout.trim()) return onPath.stdout.trim().split(/\r?\n/)[0];
  const vs = VS_CMAKE.find((p) => fs.existsSync(p));
  if (vs) return vs;
  fail('CMake was not found on PATH or in Visual Studio 2022 Build Tools (install the "C++ CMake tools for Windows" part of the Build Tools)');
}

/** The environment of a llama build. */
function llamaEnv() {
  const src = llamaSource();
  const cargoHome = process.env.CARGO_HOME || path.join(os.homedir(), '.cargo');
  const flags = [
    '-Ctarget-feature=+crt-static',
    '-Clink-arg=/Brepro',
    '-Cstrip=symbols',
    `--remap-path-prefix=${cargoHome}=cargo-home`,
    `--remap-path-prefix=${ROOT}=humanity`,
  ];
  return {
    src,
    env: {
      ...process.env,
      HUMANITY_LLAMA_SRC: src.dir,
      CMAKE: findCmake(),
      CMAKE_BUILD_PARALLEL_LEVEL: process.env.CMAKE_BUILD_PARALLEL_LEVEL || '4',
      CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '4',
      CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: flags.join(' '),
    },
  };
}

function cargo(args, env, opts = {}) {
  console.log(`> cargo ${args.join(' ')}`);
  const r = spawnSync('cargo', args, { cwd: ROOT, env, stdio: opts.capture ? ['ignore', 'pipe', 'pipe'] : 'inherit', encoding: 'utf8', maxBuffer: 256 << 20 });
  if (r.error) fail(`could not run cargo: ${r.error.message}`);
  return r;
}

const COMMON = ['--manifest-path', MANIFEST, '--target-dir', TARGET];

function build() {
  const { env, src } = llamaEnv();
  console.log(`llama.cpp source: ${LLAMA_CRATE} ${src.version} (crate checksum ${src.checksum}), ${src.dir}`);
  console.log(`CMake: ${env.CMAKE}`);
  const r = cargo(['build', '--release', '--features', 'llama', ...COMMON], env);
  if (r.status !== 0) fail('the engine did not build');
  console.log(`Built ${path.join(TARGET, 'release', exeName())}`);
  return src;
}

function exeName() {
  return process.platform === 'win32' ? 'humanity-ai.exe' : 'humanity-ai';
}

function test() {
  // The pure-Rust half: what every build has (the fake, the protocol loop, the command line).
  let r = cargo(['test', ...COMMON], process.env);
  if (r.status !== 0) fail('the engine\'s pure-Rust tests failed');
  // The llama half, in the release profile beside `build`'s program. --nocapture so the real model's
  // test says SKIPPED out loud when no model file is on this PC, instead of passing in silence.
  const { env } = llamaEnv();
  r = cargo(['test', '--release', '--features', 'llama', ...COMMON, '--', '--nocapture', '--test-threads', '1'], env);
  if (r.status !== 0) fail('the engine\'s llama tests failed');
}

function check() {
  const runs = [
    { label: 'pure Rust', args: ['check', '--all-targets', '--message-format', 'short', ...COMMON], env: process.env },
    { label: 'llama', args: ['check', '--all-targets', '--release', '--features', 'llama', '--message-format', 'short', ...COMMON], env: llamaEnv().env },
  ];
  let bad = false;
  for (const run of runs) {
    const r = cargo(run.args, run.env, { capture: true });
    const out = `${r.stdout || ''}${r.stderr || ''}`;
    process.stdout.write(out);
    // Our own crate's warnings only (a build script's own `cargo:warning` included): the line names
    // a file under the engine's folder, or says it comes from the engine package.
    const ours = out.split(/\r?\n/).filter((l) => /warning/.test(l) && (/humanity-ai-engine|crates[\\/]humanity-ai|src[\\/]/.test(l)) && !/generated \d+ warning/.test(l));
    if (r.status !== 0) {
      console.error(`FAILED: the ${run.label} build does not type-check`);
      bad = true;
    } else if (ours.length) {
      console.error(`FAILED: the ${run.label} build has ${ours.length} warning line(s) (budget 0)`);
      bad = true;
    } else {
      console.log(`ok: the ${run.label} build type-checks with no warnings`);
    }
  }
  if (bad) process.exit(1);
}

// ---------------------------------------------------------------------------------------------
// The zip: stored (no compression), sorted, fixed dates, so the same files make the same bytes.
// ---------------------------------------------------------------------------------------------

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * A zip of `entries` ([{ name, data }], names with forward slashes), every entry STORED (method 0):
 * the app reads it without a decompressor (src/ai/zip.rs), and no zlib version can change the bytes.
 * Dates are 1980-01-01 00:00 (the earliest a zip can say) and the order is by name.
 */
function makeZip(entries) {
  const sorted = [...entries].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const locals = [];
  const centrals = [];
  let offset = 0;
  const DOS_TIME = 0;
  const DOS_DATE = (0 << 9) | (1 << 5) | 1; // 1980-01-01
  for (const e of sorted) {
    const name = Buffer.from(e.name, 'utf8');
    const crc = crc32(e.data);
    const size = e.data.length;
    if (size >= 0xffffffff) fail(`${e.name} is too large for a plain zip`);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed
    local.writeUInt16LE(0x0800, 6); // flags: UTF-8 names
    local.writeUInt16LE(0, 8); // stored
    local.writeUInt16LE(DOS_TIME, 10);
    local.writeUInt16LE(DOS_DATE, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(size, 18);
    local.writeUInt32LE(size, 22);
    local.writeUInt16LE(name.length, 26);
    local.writeUInt16LE(0, 28);
    locals.push(local, name, e.data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4); // made by
    central.writeUInt16LE(20, 6); // needed
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(0, 10);
    central.writeUInt16LE(DOS_TIME, 12);
    central.writeUInt16LE(DOS_DATE, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(size, 20);
    central.writeUInt32LE(size, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt16LE(0, 30); // extra
    central.writeUInt16LE(0, 32); // comment
    central.writeUInt16LE(0, 34); // disk
    central.writeUInt16LE(0, 36); // internal attributes
    central.writeUInt32LE(0, 38); // external attributes
    central.writeUInt32LE(offset, 42);
    centrals.push(central, name);
    offset += local.length + name.length + size;
  }
  const centralBuf = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(sorted.length, 8);
  end.writeUInt16LE(sorted.length, 10);
  end.writeUInt32LE(centralBuf.length, 12);
  end.writeUInt32LE(offset, 16);
  end.writeUInt16LE(0, 20);
  return Buffer.concat([...locals, centralBuf, end]);
}

/** The Rust crates compiled INTO the engine (its normal dependencies, not build scripts' or proc
 * macros'), each with its licence files, from `cargo metadata`. */
function linkedCrates(env) {
  const meta = JSON.parse(execFileSync('cargo', ['metadata', '--manifest-path', MANIFEST, '--format-version', '1', '--features', 'llama', '--locked'], { cwd: ROOT, env, encoding: 'utf8', maxBuffer: 64 << 20, stdio: ['ignore', 'pipe', 'inherit'] }));
  const byId = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const root = meta.packages.find((p) => p.name === 'humanity-ai-engine');
  const seen = new Set();
  const walk = (id) => {
    for (const d of nodes.get(id).deps) {
      const normal = d.dep_kinds.some((k) => k.kind === null);
      const pkg = byId.get(d.pkg);
      const procMacro = pkg.targets.some((t) => t.kind.includes('proc-macro'));
      if (!normal || procMacro || seen.has(d.pkg)) continue;
      seen.add(d.pkg);
      walk(d.pkg);
    }
  };
  walk(root.id);
  return [...seen].map((id) => byId.get(id)).sort((a, b) => a.name.localeCompare(b.name));
}

function licenceFiles(dir) {
  return fs
    .readdirSync(dir)
    .filter((f) => /^(LICEN[CS]E|COPYING|UNLICENSE|NOTICE)/i.test(f) && fs.statSync(path.join(dir, f)).isFile())
    .sort();
}

function sha256(buf) {
  return crypto.createHash('sha256').update(buf).digest('hex');
}

function pack() {
  const src = build();
  const { env } = llamaEnv();
  const version = engineVersion();
  const release = path.join(TARGET, 'release');
  const entries = [];
  const exe = exeName();
  entries.push({ name: exe, data: fs.readFileSync(path.join(release, exe)) });
  const libs = fs.readdirSync(release).filter((f) => /\.dll$/i.test(f) && /^(llama|ggml)/i.test(f)).sort();
  if (!libs.some((f) => /^ggml-cpu-/i.test(f))) fail(`no ggml-cpu-*.dll beside ${exe} in ${release}`);
  for (const f of libs) entries.push({ name: f, data: fs.readFileSync(path.join(release, f)) });

  // Licences: llama.cpp's (MIT, which covers ggml, the same authors), each linked crate's, ours.
  const notices = [];
  entries.push({ name: 'licenses/llama.cpp-LICENSE.txt', data: fs.readFileSync(path.join(src.dir, 'LICENSE')) });
  notices.push(`llama.cpp and ggml (the "ggml authors"), MIT: licenses/llama.cpp-LICENSE.txt. The source is the llama.cpp folder of the crate ${LLAMA_CRATE} ${src.version} (crates.io checksum ${src.checksum}${src.vcs ? `, utilityai/llama-cpp-rs commit ${src.vcs}` : ''}).`);
  entries.push({ name: 'licenses/humanity-ai-engine-LICENSE.txt', data: fs.readFileSync(path.join(ROOT, 'LICENSE')) });
  notices.push(`humanity-ai-engine ${version} (HumanityOS, this program and its C door src/llama/shim.c), CC0 1.0: licenses/humanity-ai-engine-LICENSE.txt`);
  for (const c of linkedCrates(env)) {
    const dir = path.dirname(c.manifest_path);
    const files = licenceFiles(dir);
    const ours = c.name.startsWith('humanity-');
    if (ours) {
      entries.push({ name: `licenses/${c.name}-LICENSE.txt`, data: fs.readFileSync(path.join(ROOT, 'LICENSE')) });
      notices.push(`${c.name} ${c.version} (HumanityOS), CC0 1.0: licenses/${c.name}-LICENSE.txt`);
      continue;
    }
    if (!files.length) fail(`the crate ${c.name} ${c.version} has no licence file in ${dir}`);
    for (const f of files) entries.push({ name: `licenses/${c.name}-${f}${path.extname(f) ? '' : '.txt'}`, data: fs.readFileSync(path.join(dir, f)) });
    notices.push(`${c.name} ${c.version}, ${c.license}: ${files.map((f) => `licenses/${c.name}-${f}${path.extname(f) ? '' : '.txt'}`).join(', ')}`);
  }
  const readme = [
    `HumanityOS AI engine ${version} (humanity-ai.exe), Windows x64`,
    '',
    'The separate program HumanityOS starts, only while an AI feature you chose is in use, to run AI',
    'models on this computer. It talks to HumanityOS through its standard input and output only: no',
    'network, no listener. It loads libraries only from this folder. Remove it from Settings > AI.',
    'Design: https://github.com/Shaostoul/Humanity/blob/main/docs/design/embeddable-ai.md',
    '',
    'Inside: llama.cpp on the processor (one ggml-cpu-*.dll per processor generation; the best one',
    'for this computer is chosen when it starts). Built by the HumanityOS project from pinned source.',
    '',
    'Licences of everything inside:',
    ...notices.map((n) => `- ${n}`),
    '',
    'Needs the Microsoft Visual C++ 2015-2022 runtime (MSVCP140.dll, VCRUNTIME140.dll,',
    'VCRUNTIME140_1.dll), the same runtime HumanityOS itself uses; it is not part of this zip.',
    '',
  ].join('\r\n');
  entries.push({ name: 'README.txt', data: Buffer.from(readme, 'utf8') });

  const zip = makeZip(entries);
  const file = `humanity-ai-engine-v${version}-windows-x64.zip`;
  fs.mkdirSync(PACKAGE_DIR, { recursive: true });
  const out = path.join(PACKAGE_DIR, file);
  fs.writeFileSync(out, zip);
  const record = {
    file,
    version,
    bytes: zip.length,
    sha256: sha256(zip),
    sources: [`llama.cpp from ${LLAMA_CRATE} ${src.version}`],
    llama_crate_checksum: src.checksum,
    llama_cpp_rs_commit: src.vcs,
    entries: entries.map((e) => ({ name: e.name, bytes: e.data.length, sha256: sha256(e.data) })).sort((a, b) => (a.name < b.name ? -1 : 1)),
  };
  fs.writeFileSync(path.join(PACKAGE_DIR, 'engine.json'), `${JSON.stringify(record, null, 2)}\n`);
  console.log(`\n${out}\n  ${zip.length} bytes\n  SHA-256 ${record.sha256}\n  ${entries.length} files (${libs.length} libraries)\nRecorded in ${path.join(PACKAGE_DIR, 'engine.json')}`);
}

function main(argv) {
  const mode = argv[0] || 'build';
  switch (mode) {
    case 'build':
      return build();
    case 'test':
      return test();
    case 'check':
      return check();
    case 'package':
      return pack();
    case 'measure': {
      // The design's 9.1 to 9.3 on this computer: load time, memory, prefill and writing rates at 4
      // threads (HUMANITY_AI_MEASURE_THREADS to change) and below-normal priority, on the model
      // HUMANITY_AI_TEST_GGUF names (else the operator's Granite on E:). Prints numbers, never fails on
      // them.
      const { env } = llamaEnv();
      const r = cargo(['test', '--release', '--features', 'llama', ...COMMON, '--lib', 'measure_the_model_on_this_computer', '--', '--ignored', '--nocapture'], env);
      if (r.status !== 0) fail('the measurement did not run');
      return undefined;
    }
    case 'llama-src':
      return console.log(JSON.stringify(llamaSource(), null, 2));
    default:
      fail(`build, test, check, package or llama-src, not '${mode}'`);
  }
}

if (require.main === module) main(process.argv.slice(2));

module.exports = { makeZip, crc32 };
