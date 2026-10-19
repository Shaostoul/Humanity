#!/usr/bin/env node
/**
 * tag-release.js: tag HEAD with the version in Cargo.toml and push the tag, refusing every
 * way that has gone wrong before. This is `just tag`.
 *
 *   node scripts/tag-release.js                       tag HEAD v<version>, push the tag
 *   node scripts/tag-release.js --release notes.md    ... then gh release create v<version>
 *                                                     --title v<version> --notes-file notes.md
 *   node scripts/tag-release.js --dry-run [...]       every check, then say what it would run
 *
 * WHY (CLAUDE.md "Sharing this checkout" and "git commit -F, never -m"):
 *   - 2026-08-25: a session tagged v0.1212.2 from its own arithmetic while Cargo.toml said
 *     0.1214.2 (another session had shipped three releases meanwhile).
 *   - 2026-09-29: a grep with no matches stopped a chain before the version was read, and
 *     the tag line after it still ran: a tag named just "v", and a release marked Latest.
 *   - 2026-06-11 and 06-12: a commit silently failed (PowerShell split its -m message) and
 *     the tag and release that followed landed on the PREVIOUS commit (v0.415.0, v0.421.1).
 * So the number is READ, never typed; nothing is tagged unless every check passes first;
 * and nothing here ever runs `git commit`.
 *
 * REFUSES (each before anything is changed), with exit 1:
 *   1. Cargo.toml's [package] version is missing, empty or not plain X.Y.Z.
 *   2. HEAD's committed Cargo.toml says a different version (a bump not committed yet, or
 *      a commit that failed and left HEAD on the previous release: the v0.415.0 case).
 *   3. The tag v<version> already exists here.
 *   4. The tag already exists on origin, or origin cannot be asked (offline: nothing is
 *      tagged rather than guessing).
 *   5. HEAD is not on origin/main yet: the safe order pushes main first, so the release
 *      builds a commit that main has (CLAUDE.md's pattern: commit, push main, then tag).
 *   6. With --release: the notes file is missing or empty, or `gh` is not installed.
 * Never deletes or moves a tag (the never-delete, never-re-tag rule).
 *
 * The commands go through one `run(cmd, args)` function, which scripts/tests/
 * tag-release.test.js replaces so it can record pushes and gh calls instead of making them.
 *
 * PROVEN RED, 2026-10-19: each guard removed in turn, scripts/tests/tag-release.test.js
 * run, the guard restored. Every removal failed at least one test. With guard 2 gone, the
 * test's repo (HEAD committed at 1.2.3, Cargo.toml edited to 1.2.4) printed
 *   tagged v1.2.4 at 4a3d1bc / pushed v1.2.4 to origin
 * a tag naming a version the tagged commit does not contain: the 2026-08-25 mislabel.
 * The others: the X.Y.Z shape, the empty version, the local tag, the origin tag, origin
 * unreachable (also failing the real-command test), HEAD not on origin/main, the notes
 * file, gh missing, and the push-failure message.
 */
'use strict';

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const VERSION_SHAPE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/** The [package] version in a Cargo.toml's text, or null. */
function packageVersion(toml) {
  const text = String(toml).replace(/\r\n/g, '\n');
  const start = text.search(/^\[package\]\s*$/m);
  if (start < 0) return null;
  const rest = text.slice(start).split('\n').slice(1);
  for (const line of rest) {
    if (/^\s*\[/.test(line)) break; // the next table
    const m = /^\s*version\s*=\s*"([^"]*)"\s*(#.*)?$/.exec(line);
    if (m) return m[1];
  }
  return null;
}

function defaultRun(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: opts.cwd, encoding: 'utf8', shell: false });
  if (r.error) return { status: 127, stdout: '', stderr: String(r.error.message || r.error) };
  return { status: r.status, stdout: r.stdout || '', stderr: r.stderr || '' };
}

class Refusal extends Error {}

function parseArgs(argv) {
  const out = { release: null, dryRun: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--release') {
      out.release = argv[++i];
      if (!out.release) throw new Refusal('--release needs a notes file: just tag --release notes.md');
    } else if (a === '--dry-run') {
      out.dryRun = true;
    } else {
      throw new Refusal(`unknown argument "${a}". Usage: just tag [--release <notes-file>] [--dry-run]`);
    }
  }
  return out;
}

/**
 * Do it. Returns { code, lines, tag }. `run` is the command runner; `cwd` the checkout.
 */
function tagRelease({ cwd, argv = [], run = defaultRun, log = () => {} }) {
  const lines = [];
  const say = (s) => {
    lines.push(s);
    log(s);
  };
  const git = (...args) => run('git', args, { cwd });
  try {
    const opts = parseArgs(argv);

    // 1. The version, read from the file.
    let toml;
    try {
      toml = fs.readFileSync(path.join(cwd, 'Cargo.toml'), 'utf8');
    } catch {
      throw new Refusal('there is no Cargo.toml here; run `just tag` from the repository root.');
    }
    const version = packageVersion(toml);
    if (version === null || version === '') {
      throw new Refusal('Cargo.toml has no [package] version (or it is empty), so there is no number to tag. Nothing was tagged.');
    }
    if (!VERSION_SHAPE.test(version)) {
      throw new Refusal(`Cargo.toml's version is "${version}", which is not a plain X.Y.Z version. Nothing was tagged.`);
    }
    const tag = `v${version}`;

    // 2. HEAD must carry the same number: the bump is committed, and HEAD is the commit
    //    that was meant (not the one before a commit that silently failed).
    const head = git('show', 'HEAD:Cargo.toml');
    if (head.status !== 0) {
      throw new Refusal(`could not read Cargo.toml at HEAD (${head.stderr.trim() || 'no commit?'}). Nothing was tagged.`);
    }
    const headVersion = packageVersion(head.stdout);
    if (headVersion !== version) {
      throw new Refusal(
        `Cargo.toml says ${version} but the commit you would tag (HEAD) says ${headVersion || 'nothing'}.\n` +
          '  Either the version bump is not committed yet, or the last commit failed and HEAD is\n' +
          '  still the previous release (how v0.415.0 and v0.421.1 landed on the wrong commit).\n' +
          '  Commit the bump first (stage Cargo.toml, Cargo.lock and the stamp files by name,\n' +
          '  git commit -F <file>), push main, then run `just tag` again. Nothing was tagged.',
      );
    }

    // 3. Not already a tag here.
    if (git('rev-parse', '-q', '--verify', `refs/tags/${tag}`).status === 0) {
      throw new Refusal(
        `the tag ${tag} already exists in this checkout. Tags are never moved or re-made here:\n` +
          '  bump to the next version (node scripts/bump-version.js patch), commit, push, tag that.',
      );
    }

    // 4. Not already a tag on origin (and origin must answer).
    const remote = git('ls-remote', '--tags', 'origin', `refs/tags/${tag}`);
    if (remote.status !== 0) {
      throw new Refusal(
        `could not ask origin whether ${tag} exists (${remote.stderr.trim() || `exit ${remote.status}`}).\n` +
          '  Nothing was tagged. Check the network and `git remote -v`, then run `just tag` again.',
      );
    }
    if (remote.stdout.trim()) {
      throw new Refusal(
        `origin already has the tag ${tag} (another session shipped this number first?).\n` +
          '  Bump to the next version, commit, push main, and tag that. Nothing was tagged.',
      );
    }

    // 5. HEAD is on origin/main.
    const onMain = git('merge-base', '--is-ancestor', 'HEAD', 'origin/main');
    if (onMain.status !== 0) {
      throw new Refusal(
        (onMain.status === 1
          ? 'HEAD is not on origin/main yet.'
          : `could not compare HEAD with origin/main (${onMain.stderr.trim() || `exit ${onMain.status}`}).`) +
          '\n  Push main first (git push origin main), so the release builds a commit main has. Nothing was tagged.',
      );
    }

    // 6. The release notes and gh, before anything is pushed.
    let notes = null;
    if (opts.release) {
      notes = path.resolve(cwd, opts.release);
      let size = -1;
      try {
        size = fs.statSync(notes).size;
      } catch {
        /* missing */
      }
      if (size <= 0) {
        throw new Refusal(`the release notes file ${opts.release} is ${size < 0 ? 'missing' : 'empty'}. Write it first. Nothing was tagged.`);
      }
      if (run('gh', ['--version'], { cwd }).status !== 0) {
        throw new Refusal('--release needs the GitHub CLI (gh), which did not run. Nothing was tagged.');
      }
    }

    const sha = git('rev-parse', '--short', 'HEAD').stdout.trim();
    const steps = [
      ['git', ['tag', tag]],
      ['git', ['push', 'origin', `refs/tags/${tag}`]],
    ];
    if (notes) steps.push(['gh', ['release', 'create', tag, '--title', tag, '--notes-file', notes]]);

    if (opts.dryRun) {
      say(`dry run: every check passed for ${tag} at ${sha}. Would run:`);
      for (const [c, a] of steps) say(`  ${c} ${a.join(' ')}`);
      return { code: 0, lines, tag };
    }

    // Tag, push, release. Each failure says exactly what state it left.
    let r = git('tag', tag);
    if (r.status !== 0) {
      throw new Refusal(`git tag ${tag} failed: ${r.stderr.trim()}. Nothing was pushed.`);
    }
    say(`tagged ${tag} at ${sha}`);
    r = git('push', 'origin', `refs/tags/${tag}`);
    if (r.status !== 0) {
      throw new Refusal(
        `the tag ${tag} exists here but the push failed: ${r.stderr.trim()}\n` +
          `  Push it again with: git push origin refs/tags/${tag}\n` +
          '  Do not delete or move the tag.',
      );
    }
    say(`pushed ${tag} to origin`);
    if (notes) {
      r = run('gh', ['release', 'create', tag, '--title', tag, '--notes-file', notes], { cwd });
      if (r.status !== 0) {
        throw new Refusal(
          `${tag} is tagged and pushed, but gh release create failed: ${r.stderr.trim()}\n` +
            `  Retry with: gh release create ${tag} --title ${tag} --notes-file ${opts.release}`,
        );
      }
      say(`released ${tag}`);
    }
    say('Next: gh release list --limit 3  (check that Latest points at the release you meant)');
    return { code: 0, lines, tag };
  } catch (e) {
    if (!(e instanceof Refusal)) throw e;
    say(`x just tag refused: ${e.message}`);
    return { code: 1, lines, tag: null };
  }
}

module.exports = { tagRelease, packageVersion, defaultRun };

if (require.main === module) {
  const root = (() => {
    const r = defaultRun('git', ['rev-parse', '--show-toplevel'], { cwd: process.cwd() });
    return r.status === 0 ? r.stdout.trim() : process.cwd();
  })();
  const { code } = tagRelease({ cwd: root, argv: process.argv.slice(2), log: (s) => console.log(s) });
  process.exitCode = code;
}
