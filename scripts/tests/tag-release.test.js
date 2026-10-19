// `just tag` (scripts/tag-release.js): every refusal, in throwaway repositories under the OS
// temp folder, never this checkout, and never a real push. The runner the script is given
// is an ALLOWLIST: only `git show`, `git rev-parse` and `git tag` reach real git (in the
// temp repo); `git push`, `git ls-remote`, `git merge-base` and `gh` are recorded and
// answered by the test; anything else is recorded as unexpected and fails the test. The
// temp repos have no remote at all, as a second wall. Runs in `just rig-tests`.
//
// Mutation checks, run 2026-10-19 when this was written (each guard removed from the
// script, the suite run, the script restored): see the PROVEN RED note in tag-release.js.

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const SCRIPT = path.join(__dirname, '..', 'tag-release.js');
const { tagRelease, packageVersion, defaultRun } = require(SCRIPT);

const toml = (version) =>
  `[package]\nname = "humanity-engine"\nversion = ${JSON.stringify(version)}\nedition = "2021"\n\n[dependencies.serde]\nversion = "9.9.9"\n`;

function repo(version = '1.2.3') {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tag-release-'));
  const g = (...args) => {
    const r = spawnSync('git', args, { cwd: dir, encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr}`);
    return r.stdout.trim();
  };
  g('init', '-q');
  g('config', 'user.name', 'test');
  g('config', 'user.email', 'test@example.invalid');
  g('config', 'commit.gpgsign', 'false');
  g('config', 'tag.gpgsign', 'false');
  fs.writeFileSync(path.join(dir, 'Cargo.toml'), toml(version));
  g('add', 'Cargo.toml');
  g('commit', '-q', '--no-verify', '-m', `v${version}`);
  const tags = () => g('tag', '--list');
  const cleanup = () => fs.rmSync(dir, { recursive: true, force: true });
  return { dir, g, tags, cleanup };
}

/**
 * The fake runner. `world` says how origin and gh answer.
 */
function fake(world = {}) {
  const w = { remoteTags: '', remoteStatus: 0, onMain: 0, push: 0, gh: 0, ghVersion: 0, ...world };
  const calls = [];
  const unexpected = [];
  const run = (cmd, args, opts) => {
    calls.push([cmd, ...args].join(' '));
    if (cmd === 'gh') {
      if (args[0] === '--version') return { status: w.ghVersion, stdout: 'gh version 2', stderr: '' };
      return { status: w.gh, stdout: '', stderr: w.gh ? 'HTTP 422: release exists' : '' };
    }
    if (cmd !== 'git') {
      unexpected.push(cmd);
      return { status: 127, stdout: '', stderr: 'not allowed in this test' };
    }
    switch (args[0]) {
      case 'push':
        return { status: w.push, stdout: '', stderr: w.push ? '! [remote rejected]' : '' };
      case 'ls-remote':
        return { status: w.remoteStatus, stdout: w.remoteTags, stderr: w.remoteStatus ? 'Could not resolve host: github.com' : '' };
      case 'merge-base':
        return { status: w.onMain, stdout: '', stderr: w.onMain > 1 ? 'fatal: Not a valid object name origin/main' : '' };
      case 'show':
      case 'rev-parse':
      case 'tag':
        return defaultRun('git', args, opts);
      default:
        unexpected.push(`git ${args.join(' ')}`);
        return { status: 128, stdout: '', stderr: 'not allowed in this test' };
    }
  };
  return { run, calls, unexpected };
}

function attempt(r, world, argv = []) {
  const f = fake(world);
  const res = tagRelease({ cwd: r.dir, argv, run: f.run });
  const out = res.lines.join('\n');
  assert.deepStrictEqual(f.unexpected, [], `unexpected commands: ${f.unexpected.join(', ')}`);
  assert.ok(!f.calls.some((c) => /\bcommit\b/.test(c)), `never git commit: ${f.calls.join(' | ')}`);
  return { ...res, out, calls: f.calls };
}

function refused(r, world, argv, why) {
  const before = r.tags();
  const res = attempt(r, world, argv);
  assert.strictEqual(res.code, 1, res.out);
  assert.match(res.out, /^x just tag refused: /);
  assert.match(res.out, why);
  assert.strictEqual(r.tags(), before, 'a refusal must not create a tag');
  assert.ok(!res.calls.some((c) => c.startsWith('git push') || c.startsWith('gh release')), res.calls.join(' | '));
  return res;
}

const withRepo = (version, fn) => {
  const r = repo(version);
  try {
    fn(r);
  } finally {
    r.cleanup();
  }
};

test('reads the [package] version, not a dependency table version', () => {
  assert.strictEqual(packageVersion(toml('0.1518.0')), '0.1518.0');
  assert.strictEqual(packageVersion('[dependencies.serde]\nversion = "1.0.0"\n[package]\nname = "x"\n'), null);
  assert.strictEqual(packageVersion('[package]\r\nname = "x"\r\nversion = "2.0.1"\r\n'), '2.0.1');
});

test('the happy path tags HEAD v<version> and pushes only that tag', () => {
  withRepo('1.2.3', (r) => {
    const res = attempt(r, {});
    assert.strictEqual(res.code, 0, res.out);
    assert.strictEqual(r.tags(), 'v1.2.3');
    assert.strictEqual(r.g('rev-parse', 'v1.2.3^{commit}'), r.g('rev-parse', 'HEAD'));
    assert.ok(res.calls.includes('git push origin refs/tags/v1.2.3'), res.calls.join(' | '));
    assert.ok(!res.calls.some((c) => c.startsWith('gh release')));
    assert.match(res.out, /gh release list --limit 3/);
  });
});

test('--release runs gh release create with the version as title and the notes file', () => {
  withRepo('1.2.3', (r) => {
    fs.writeFileSync(path.join(r.dir, 'notes.md'), 'What changed.\n');
    const res = attempt(r, {}, ['--release', 'notes.md']);
    assert.strictEqual(res.code, 0, res.out);
    const notes = path.resolve(r.dir, 'notes.md');
    assert.ok(res.calls.includes(`gh release create v1.2.3 --title v1.2.3 --notes-file ${notes}`), res.calls.join(' | '));
    // The tag is pushed before the release is made.
    assert.ok(res.calls.indexOf('git push origin refs/tags/v1.2.3') < res.calls.findIndex((c) => c.startsWith('gh release')));
  });
});

test('--dry-run checks everything and changes nothing', () => {
  withRepo('1.2.3', (r) => {
    const res = attempt(r, {}, ['--dry-run']);
    assert.strictEqual(res.code, 0, res.out);
    assert.match(res.out, /Would run:\n  git tag v1\.2\.3\n  git push origin refs\/tags\/v1\.2\.3/);
    assert.strictEqual(r.tags(), '');
    assert.ok(!res.calls.some((c) => c.startsWith('git push') || c === 'git tag v1.2.3'));
  });
});

test('an empty or missing version is refused (the tag named just "v", 2026-09-29)', () => {
  withRepo('', (r) => refused(r, {}, [], /no \[package\] version \(or it is empty\)/));
  withRepo('1.2.3', (r) => {
    fs.writeFileSync(path.join(r.dir, 'Cargo.toml'), '[package]\nname = "x"\n');
    refused(r, {}, [], /no \[package\] version/);
  });
});

test('a version that is not plain X.Y.Z is refused', () => {
  for (const v of ['1.2', 'v1.2.3', '1.2.3-rc1', '01.2.3', '1.2.3 ', 'latest']) {
    withRepo(v, (r) => refused(r, {}, [], /not a plain X\.Y\.Z version/));
  }
});

test('an uncommitted bump is refused, naming both numbers (the v0.1212.2 mislabel, 2026-08-25)', () => {
  withRepo('1.2.3', (r) => {
    fs.writeFileSync(path.join(r.dir, 'Cargo.toml'), toml('1.2.4'));
    refused(r, {}, [], /Cargo\.toml says 1\.2\.4 but the commit you would tag \(HEAD\) says 1\.2\.3/);
  });
});

test('a tag that already exists here is refused, and never moved', () => {
  withRepo('1.2.3', (r) => {
    r.g('tag', 'v1.2.3');
    const first = r.g('rev-parse', 'v1.2.3');
    refused(r, {}, [], /already exists in this checkout/);
    assert.strictEqual(r.g('rev-parse', 'v1.2.3'), first);
  });
});

test('a tag that already exists on origin is refused', () => {
  withRepo('1.2.3', (r) => {
    refused(r, { remoteTags: 'abc123\trefs/tags/v1.2.3\n' }, [], /origin already has the tag v1\.2\.3/);
  });
});

test('origin that cannot be asked is refused, not guessed', () => {
  withRepo('1.2.3', (r) => {
    refused(r, { remoteStatus: 128 }, [], /could not ask origin whether v1\.2\.3 exists \(Could not resolve host/);
  });
});

test('HEAD not on origin/main yet is refused; so is a missing origin/main', () => {
  withRepo('1.2.3', (r) => {
    refused(r, { onMain: 1 }, [], /HEAD is not on origin\/main yet/);
    refused(r, { onMain: 128 }, [], /could not compare HEAD with origin\/main/);
  });
});

test('--release with a missing or empty notes file, or no gh, is refused BEFORE tagging', () => {
  withRepo('1.2.3', (r) => {
    refused(r, {}, ['--release', 'nope.md'], /notes file nope\.md is missing/);
    fs.writeFileSync(path.join(r.dir, 'empty.md'), '');
    refused(r, {}, ['--release', 'empty.md'], /notes file empty\.md is empty/);
    fs.writeFileSync(path.join(r.dir, 'notes.md'), 'x\n');
    refused(r, { ghVersion: 127 }, ['--release', 'notes.md'], /needs the GitHub CLI/);
    refused(r, {}, ['--release'], /--release needs a notes file/);
    refused(r, {}, ['--sign'], /unknown argument "--sign"/);
  });
});

test('a failed push leaves the local tag, says how to push it again, and makes no release', () => {
  withRepo('1.2.3', (r) => {
    fs.writeFileSync(path.join(r.dir, 'notes.md'), 'x\n');
    const res = attempt(r, { push: 1 }, ['--release', 'notes.md']);
    assert.strictEqual(res.code, 1, res.out);
    assert.match(res.out, /exists here but the push failed/);
    assert.match(res.out, /git push origin refs\/tags\/v1\.2\.3/);
    assert.match(res.out, /Do not delete or move the tag/);
    assert.strictEqual(r.tags(), 'v1.2.3');
    assert.ok(!res.calls.some((c) => c.startsWith('gh release')));
  });
});

test('the real command, run in a repo with no origin at all, refuses and tags nothing', () => {
  withRepo('1.2.3', (r) => {
    const p = spawnSync(process.execPath, [SCRIPT], { cwd: r.dir, encoding: 'utf8' });
    assert.strictEqual(p.status, 1, p.stdout + p.stderr);
    assert.match(p.stdout, /could not ask origin whether v1\.2\.3 exists/);
    assert.strictEqual(r.tags(), '');
  });
});
