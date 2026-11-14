// scripts/i18n-add.js puts a new key at its SORTED place in its group (2026-11-14), so two agents
// adding words to the same group of data/i18n/en.json merge in git without a conflict.
//
// Run: node --test scripts/tests/i18n-add-merge.test.js   (in `just rig-tests`)
//
// Why: until then a new key went at the END of its group. Two branches that each added a word to
// one group both edited the group's last line (its comma) and both inserted after it, so git
// stopped every such merge on a conflict in en.json, and in web/shared/i18n-en.js generated from
// it (docs/ai/orchestrator.md, section 8: a generated file is regenerated, never merged by hand).
//
// What it proves, with REAL git in a throwaway repository and the REAL en.json:
//   1. the CONTROL: two additions appended at a group's end (the old way) conflict. Without this
//      the merge check below could pass because the harness never sees a conflict at all;
//   2. the same two additions made by i18n-add's merge() merge cleanly, and the result holds both;
//   3. one added key changes en.json by exactly one inserted line: nothing already there moves;
//   4. insertKey keeps an unsorted group's order and puts the key before its sorted successor.
//
// Seen red 2026-11-14: with insertKey reverted to the old `obj[key] = value` (append), test 2
// failed: "two additions to one group conflicted in git" (CONFLICT (content): Merge conflict in
// en.json), and test 3 failed (the new key landed after the group's last line, which changed too).
"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const ADD = require(path.join(ROOT, "scripts", "i18n-add.js"));
const EN_TEXT = fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8").replace(/\r\n/g, "\n");

/** The group of en.json with the most plain-word keys: where two agents most often meet. */
function busiestGroup(en) {
  let best = null;
  for (const [g, v] of Object.entries(en)) {
    if (!v || typeof v !== "object") continue;
    const n = Object.values(v).filter((x) => typeof x === "string").length;
    if (!best || n > best.n) best = { g, n };
  }
  return best.g;
}

/** The smallest key of `obj` that sorts after `key` (where insertKey puts it before). */
function successor(obj, key) {
  let s = null;
  for (const e of Object.keys(obj)) if (e > key && (s === null || e < s)) s = e;
  return s;
}

function git(cwd, ...args) {
  const r = spawnSync("git", ["-c", "core.autocrlf=false", "-c", "core.hooksPath=" + path.join(cwd, ".no-hooks"),
    "-c", "user.name=i18n-test", "-c", "user.email=i18n-test@localhost", "-c", "commit.gpgsign=false", ...args],
  { cwd, encoding: "utf8", windowsHide: true });
  return { status: r.status, out: (r.stdout || "") + (r.stderr || "") };
}

/**
 * A throwaway repository with en.json as its first commit, branch `a` and branch `b` each changed
 * by `editA` / `editB` (functions of the parsed file), then `b` merged into `a`.
 * Returns { status, out, merged } (merged = the file after the merge, or null on a conflict).
 */
function mergeTwoBranches(editA, editB) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hos-i18n-merge-"));
  try {
    const file = path.join(dir, "en.json");
    assert.strictEqual(git(dir, "init", "-q", "-b", "main").status, 0, "git init");
    fs.writeFileSync(file, EN_TEXT);
    git(dir, "add", "en.json");
    assert.strictEqual(git(dir, "commit", "-q", "-m", "base").status, 0, "base commit");
    for (const [branch, edit] of [["a", editA], ["b", editB]]) {
      git(dir, "checkout", "-q", "-b", branch, "main");
      const en = JSON.parse(EN_TEXT);
      edit(en);
      fs.writeFileSync(file, ADD.serialize(en));
      git(dir, "add", "en.json");
      assert.strictEqual(git(dir, "commit", "-q", "-m", branch).status, 0, branch + " commit");
    }
    git(dir, "checkout", "-q", "a");
    const m = git(dir, "merge", "--no-edit", "-q", "b");
    const merged = m.status === 0 ? fs.readFileSync(file, "utf8") : null;
    return { status: m.status, out: m.out, merged };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const EN = JSON.parse(EN_TEXT);
const GROUP = busiestGroup(EN);
// Two new keys in two different gaps of the group: after its first third and after its second.
const sorted = Object.keys(EN[GROUP]).filter((k) => typeof EN[GROUP][k] === "string").sort((x, y) => (x < y ? -1 : x > y ? 1 : 0));
const KEY_A = sorted[Math.floor(sorted.length / 3)] + "_merge_test_a";
const KEY_B = sorted[Math.floor((2 * sorted.length) / 3)] + "_merge_test_b";

test("the file round-trips through i18n-add unchanged (else every add would rewrite it)", () => {
  assert.strictEqual(ADD.serialize(JSON.parse(EN_TEXT)), EN_TEXT, "serialize(parse(en.json)) differs from en.json");
});

test("the setup: the two keys are new and land before different existing keys", () => {
  assert.ok(sorted.length >= 6, `the busiest group ${GROUP} has ${sorted.length} words`);
  assert.strictEqual(EN[GROUP][KEY_A], undefined);
  assert.strictEqual(EN[GROUP][KEY_B], undefined);
  assert.notStrictEqual(successor(EN[GROUP], KEY_A), successor(EN[GROUP], KEY_B), "both keys would land at one place");
});

test("CONTROL: two additions appended at a group's end conflict in git (the old way)", () => {
  const r = mergeTwoBranches((en) => { en[GROUP][KEY_A] = "A"; }, (en) => { en[GROUP][KEY_B] = "B"; });
  assert.notStrictEqual(r.status, 0, "appending at the end merged cleanly, so this harness cannot see a conflict:\n" + r.out);
  assert.match(r.out, /CONFLICT/, r.out);
});

test("two additions to one group by i18n-add merge in git without a conflict", () => {
  const add = (key, words) => (en) => {
    const res = ADD.merge(en, { [GROUP]: { [key]: words } }, false);
    assert.deepStrictEqual(res.conflicts, []);
    assert.deepStrictEqual(res.added, [GROUP + "." + key]);
  };
  const r = mergeTwoBranches(add(KEY_A, "A"), add(KEY_B, "B"));
  assert.strictEqual(r.status, 0, "two additions to one group conflicted in git:\n" + r.out);
  const merged = JSON.parse(r.merged);
  assert.strictEqual(merged[GROUP][KEY_A], "A");
  assert.strictEqual(merged[GROUP][KEY_B], "B");
});

test("one added key is one inserted line: nothing already in en.json moves or changes", () => {
  const en = JSON.parse(EN_TEXT);
  ADD.merge(en, { [GROUP]: { [KEY_A]: "A" } }, false);
  const before = EN_TEXT.split("\n");
  const after = ADD.serialize(en).split("\n");
  assert.strictEqual(after.length, before.length + 1);
  const at = after.findIndex((l, i) => l !== before[i]);
  assert.strictEqual(after[at].trim(), `${JSON.stringify(KEY_A)}: "A",`, "the inserted line");
  assert.deepStrictEqual(after.slice(0, at).concat(after.slice(at + 1)), before, "a line other than the new one changed");
});

test("insertKey: sorted place among the keys there, an unsorted group's order kept", () => {
  const sortedGroup = { apple: 1, melon: 2, zucchini: 3 };
  ADD.insertKey(sortedGroup, "banana", 9);
  assert.deepStrictEqual(Object.keys(sortedGroup), ["apple", "banana", "melon", "zucchini"]);
  ADD.insertKey(sortedGroup, "aardvark", 9);
  ADD.insertKey(sortedGroup, "zzz", 9);
  assert.deepStrictEqual(Object.keys(sortedGroup), ["aardvark", "apple", "banana", "melon", "zucchini", "zzz"]);
  const unsorted = { title: 1, alpha: 2, zeta: 3, mid: 4 };
  ADD.insertKey(unsorted, "beta", 9); // its successor among the keys there is "mid"
  assert.deepStrictEqual(Object.keys(unsorted), ["title", "alpha", "zeta", "beta", "mid"]);
  ADD.insertKey(unsorted, "zz", 9); // sorts after everything: straight after the largest, "zeta"
  assert.deepStrictEqual(Object.keys(unsorted), ["title", "alpha", "zeta", "zz", "beta", "mid"]);
  // A new group goes in the same way, and the words it brings are in it.
  const top = { a: { x: "1" }, c: { y: "2" } };
  ADD.merge(top, { b: { z: "3" } }, false);
  assert.deepStrictEqual(Object.keys(top), ["a", "b", "c"]);
  assert.strictEqual(top.b.z, "3");
});
