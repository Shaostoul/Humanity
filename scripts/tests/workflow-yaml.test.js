// Every GitHub workflow file parses as YAML where the common slip lives: a colon and a space
// inside an unquoted one-line value.
//
// WHY (2026-11-15): build-android.yml line 108 read
//   run: echo "::warning::This release has no APK: the signing secrets ..."
// The ": " after "APK" ends a YAML plain scalar ("mapping values are not allowed here"), so GitHub
// refused the whole file ("You have an error in your yaml syntax on line 108") and logged a failed
// run on every push to main, while every local check passed: nothing here parsed the workflows.
// The repo has no YAML parser dependency, so this checks the one rule that bit, by line: an
// unquoted value after `key: ` may not itself contain ": " (or end in ":"). Values that open a
// quote, a block scalar (| or >), a flow list or map, or are comments are skipped.
//
// Seen red 2026-11-15 on the unfixed build-android.yml: "build-android.yml:108".
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');

const DIR = path.join(__dirname, '..', '..', '.github', 'workflows');

/** The lines of `text` whose unquoted one-line value holds ": ", as "file:line". */
function badPlainScalars(name, text) {
  const bad = [];
  let inBlock = -1; // indent of a `key: |` / `key: >` whose body we are inside
  text.split(/\r?\n/).forEach((line, i) => {
    const indent = line.match(/^ */)[0].length;
    if (inBlock >= 0) {
      if (line.trim() === '' || indent > inBlock) return;
      inBlock = -1;
    }
    if (/^\s*#/.test(line)) return;
    const m = line.match(/^(\s*)(?:-\s+)?([A-Za-z0-9_.-]+):\s+(.*)$/);
    if (!m) return;
    const value = m[3].replace(/\s+#.*$/, '');
    if (/^[|>]/.test(value)) {
      inBlock = indent;
      return;
    }
    if (/^["'[{&*!]/.test(value)) return;
    // A `${{ }}` expression can hold "a: b" only inside a string literal; strip them first.
    const bare = value.replace(/\$\{\{[\s\S]*?\}\}/g, 'X');
    if (/: /.test(bare) || /:$/.test(bare)) bad.push(`${name}:${i + 1}`);
  });
  return bad;
}

test('every workflow file keeps ": " out of unquoted one-line values', () => {
  const files = fs.readdirSync(DIR).filter((f) => /\.ya?ml$/.test(f));
  assert.ok(files.length >= 5, `expected the workflows in ${DIR}, found ${files.length}`);
  const bad = files.flatMap((f) => badPlainScalars(f, fs.readFileSync(path.join(DIR, f), 'utf8')));
  assert.deepStrictEqual(bad, [], 'quote the value or use `run: |` on these lines');
});

test('the check catches the line that broke build-android.yml', () => {
  const broken = '    steps:\n      - name: Say why\n        run: echo "::warning::This release has no APK: the secrets are not set."\n';
  assert.deepStrictEqual(badPlainScalars('x.yml', broken), ['x.yml:3']);
  const fixed = '    steps:\n      - name: Say why\n        run: |\n          echo "no APK: the secrets"\n';
  assert.deepStrictEqual(badPlainScalars('x.yml', fixed), []);
});
