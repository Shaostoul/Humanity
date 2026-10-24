// The web admin page's Backups panel (web/pages/backups-admin.js, 2026-10-22), the
// mirror of the desktop app's Server Settings > ADMIN > Backups. Pure node, no
// browser: runs in `just rig-tests`.
//
// Run:  node --test scripts/tests/backups-admin-web.test.js
//
// What it holds:
// - the words an admin signs are exactly the relay's (`backup_stream::api::purpose`
//   in Rust, whose test asserts the same string): a mismatch means every request
//   from the page is refused as a bad signature;
// - a Save sends what the relay expects: Off sends no destination (the relay keeps
//   the saved one), an empty secret stays empty (the relay keeps the saved one),
//   numbers must be whole;
// - the status shown never carries anything but the destination's summary;
// - every element the script reaches for is on the page (a renamed id would leave
//   the panel half-wired with no error).
//
// Seen red 2026-10-22, each fault on its own: hashing the action instead of the
// params ("actual: 'admin_backups\nstatus\n073c1634...'"); an empty secret sent as
// something else ("actual: 'kept', expected: ''"); the drill button's id renamed
// in the page ("ids the script uses that the page does not have ... actual:
// [ 'bk-drill-btn' ]").

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "..", "..");
const B = require("../../web/pages/backups-admin.js");

test("the signed words are the relay's, byte for byte", async () => {
  assert.strictEqual(
    await B.purpose("status", ""),
    "admin_backups\nstatus\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  );
  // The same pair the Rust test `the_signed_words_match_the_apps` checks.
  assert.strictEqual(
    await B.purpose("set_destination", '{"enabled":false}'),
    "admin_backups\nset_destination\n5acf3ff77b4420677b5923071f303facaba7a9273a346284a667a275df325146"
  );
});

test("a Save sends what the relay keeps or changes, and nothing it would misread", () => {
  const base = { retention_hours: "72", snapshot_every_hours: "24", alert_after_minutes: "5" };
  const off = JSON.parse(B.settingsParams(Object.assign({ kind: "off" }, base)).params);
  assert.deepStrictEqual(off, { enabled: false, destination: null, retention_hours: 72, snapshot_every_hours: 24, alert_after_minutes: 5 });

  const s3 = JSON.parse(
    B.settingsParams(
      Object.assign(
        { kind: "s3", endpoint: " https://s3.example.com ", region: "eu-1", bucket: "b", prefix: "relay", access_key_id: "", secret_access_key: "", path_style: true },
        base
      )
    ).params
  );
  assert.strictEqual(s3.enabled, true);
  assert.strictEqual(s3.destination.kind, "s3");
  assert.strictEqual(s3.destination.endpoint, "https://s3.example.com", "spaces trimmed");
  assert.strictEqual(s3.destination.secret_access_key, "", "an empty secret is sent empty, so the relay keeps the saved one");
  assert.strictEqual(s3.destination.access_key_id, "");

  const server = JSON.parse(B.settingsParams(Object.assign({ kind: "server", url: "https://public.guide/" }, base)).params);
  assert.deepStrictEqual(server.destination, { kind: "server", url: "https://public.guide/" });

  assert.ok(B.settingsParams(Object.assign({ kind: "folder", path: "/x" }, base, { retention_hours: "3.5" })).error);
  assert.ok(B.settingsParams(Object.assign({ kind: "folder", path: "/x" }, base, { alert_after_minutes: "" })).error);
});

test("the status shown names the destination by its summary and nothing more", () => {
  const now = Date.UTC(2026, 9, 22, 14, 0, 0);
  const status = {
    stream: {
      state: "up_to_date",
      words: "Every change is at the HumanityOS server https://public.guide.",
      restorable_from_ms: now - 72 * 3600e3,
      restorable_to_ms: now - 5000,
      lag_secs: 0,
      dest_bytes: 5 * 1048576,
      dest_objects: 40,
      snapshots: 3,
      pending_objects: 0,
      key_fingerprint: "ab12 cd34 ef56 7890",
      retention_hours: 72,
      snapshot_every_hours: 24,
      alert_after_minutes: 5,
    },
    config: { destination: { kind: "s3", summary: "relay/ in the bucket b at https://s3.example.com", access_key_hint: "...WXYZ", secret_set: true } },
  };
  const d = B.describe(status, now);
  assert.strictEqual(d.label, "Up to date");
  assert.strictEqual(d.tone, "ok");
  const text = JSON.stringify(d.rows);
  assert.ok(text.includes("relay/ in the bucket b at https://s3.example.com"));
  assert.ok(text.includes("5 s ago"), text);
  assert.ok(text.includes("2026-10-19 14:00:00 UTC"), "the restore window starts 72 h back: " + text);
  assert.ok(!text.includes("WXYZ") && !text.includes("secret"), "only the summary of the destination is shown: " + text);
  assert.deepStrictEqual(B.describe({ stream: { state: "off" } }, now).rows, [], "nothing but the off sentence when off");
  assert.strictEqual(B.describe({ stream: { state: "behind" } }, now).tone, "err");
});

test("every element the panel script uses is on the admin page", () => {
  const js = fs.readFileSync(path.join(ROOT, "web/pages/backups-admin.js"), "utf8");
  const html = fs.readFileSync(path.join(ROOT, "web/pages/admin.html"), "utf8");
  const ids = new Set();
  for (const m of js.matchAll(/\$\('([a-z0-9-]+)'\)/g)) ids.add(m[1]);
  for (const m of js.matchAll(/getElementById\('([a-z0-9-]+)'\)/g)) ids.add(m[1]);
  assert.ok(ids.size > 20, "the scan found the ids: " + [...ids].join(", "));
  const missing = [...ids].filter((id) => !html.includes('id="' + id + '"'));
  assert.deepStrictEqual(missing, [], "ids the script uses that the page does not have");
  for (const kind of ["off", "folder", "s3", "server"]) {
    assert.ok(html.includes('name="bk-kind" value="' + kind + '"'), "the " + kind + " choice");
    assert.ok(html.includes('id="bk-fields-' + kind + '"'), "the " + kind + " fields");
  }
  assert.ok(/<script src="\/pages\/backups-admin\.js\?v=\d+"><\/script>/.test(html), "the page loads the script");
});
