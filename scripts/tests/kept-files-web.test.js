// A copy, on this device, of each private file the person opened (2026-10-11,
// docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", part 4):
// web/shared/kept-files.js, and the Settings page's controls it draws.
//
// Run: node --test scripts/tests/kept-files-web.test.js
//
// What is REAL: the encryption (Node's WebCrypto AES-256-GCM, a key made like the DM store's:
// non-extractable), the hashing of record ids, and the module's own code. What is stood in:
// IndexedDB (a small one that keeps each store in a Map, with the upgrade step a browser runs) and
// the Settings page's document (elements that keep their children, text and listeners).
//
// What it proves:
//  1. A file kept opens back to its bytes with the same key, and with no other key; a copy that
//     does not open is removed. Nothing in the database names the file: no address, no key, no
//     byte of the file in the clear.
//  2. The cap: the oldest opened go first (opening one makes it the newest), a file larger than
//     the cap is not kept, a cap of 0 keeps nothing, and a copy left with no size beside it goes.
//  3. The cap is 200 MB unless chosen; Settings shows what is kept, a new cap trims at once, and
//     Clear kept files asks first, then removes every copy.
// The chat's own use (opening looks here first; a reload still opens a file the server let go)
// is in private-files-web.test.js.
//
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red). Red first: see
// the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const K = require(path.join(WEB, "shared", "kept-files.js"));

const MIB = 1024 * 1024;
const SCOPE = "a1".repeat(32);
const meta = (n) => ({ url: `/uploads/file-${n}.enc`, k: Buffer.alloc(32, n).toString("base64"), n: "AAAAAAAAAAAAAAAA", name: `photo-${n}.jpg`, mime: "image/jpeg", size: 0 });
const key = () => globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
const fill = (n, v) => new Uint8Array(n).fill(v);

/** A stand-in IndexedDB: each store a Map, the upgrade run when the database is new. */
function fakeIndexedDB() {
  const dbs = new Map();
  const later = (r, result, err) => setImmediate(() => { if (err) { r.error = err; if (r.onerror) r.onerror(); } else { r.result = result; if (r.onsuccess) r.onsuccess(); } });
  const req = (result) => { const r = {}; later(r, result); return r; };
  function makeDb() {
    const stores = new Map();
    const db = {
      stores,
      objectStoreNames: { contains: (n) => stores.has(n) },
      createObjectStore: (n, opts) => { stores.set(n, { keyPath: opts.keyPath, rows: new Map() }); },
      transaction: (n) => {
        if (!stores.has(n)) throw new Error("no store " + n);
        const st = stores.get(n);
        return {
          objectStore: () => ({
            get: (k) => req(st.rows.get(k)),
            put: (v) => { st.rows.set(v[st.keyPath], v); return req(undefined); },
            delete: (k) => { st.rows.delete(k); return req(undefined); },
            clear: () => { st.rows.clear(); return req(undefined); },
            getAll: () => req([...st.rows.values()]),
            getAllKeys: () => req([...st.rows.keys()]),
          }),
        };
      },
    };
    return db;
  }
  return {
    dbs,
    open(name, version) {
      const r = {};
      setImmediate(() => {
        let db = dbs.get(name);
        const fresh = !db;
        if (fresh) { db = makeDb(); dbs.set(name, db); db.version = version; }
        r.result = db;
        if (fresh && r.onupgradeneeded) r.onupgradeneeded();
        if (r.onsuccess) r.onsuccess();
      });
      return r;
    },
  };
}

function storage(init) {
  const m = new Map(Object.entries(init || {}));
  return { m, getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}

// ── 1. Kept, and only for the key ────────────────────────────────────────

test("a kept file opens back with the same key and no other; nothing in the database names it", async () => {
  const idb = fakeIndexedDB();
  const mine = await key();
  const bytes = Uint8Array.from(Buffer.from("a picture of a dog on a beach, pixel by pixel"));
  const m = meta(1);
  assert.equal(await K.put({ indexedDB: idb, key: mine, scope: SCOPE, meta: m, bytes, cap: MIB }), true, "kept");
  const back = await K.get({ indexedDB: idb, key: mine, scope: SCOPE, meta: m });
  assert.ok(back && Buffer.from(back).equals(Buffer.from(bytes)), "opens back to its bytes");
  assert.equal(await K.get({ indexedDB: idb, key: mine, scope: "b2".repeat(32), meta: m }), null, "not under another identity or server");
  assert.equal(await K.get({ indexedDB: idb, key: mine, scope: SCOPE, meta: Object.assign({}, m, { k: meta(9).k }) }), null, "not for another key in a marker");

  // Everything kept, as text: no address, no key, no byte run of the file.
  const db = idb.dbs.get(K.DB_NAME);
  const dump = JSON.stringify([...db.stores.values()].map((s) => [...s.rows.entries()].map(([k, v]) => [k, Object.fromEntries(Object.entries(v).map(([a, b]) => [a, b instanceof Uint8Array ? Buffer.from(b).toString("latin1") : b]))])));
  assert.ok(!dump.includes(m.url) && !dump.includes("file-1"), "the address is not kept in the clear");
  assert.ok(!dump.includes(m.k), "nor the file's key");
  assert.ok(!dump.includes("a picture of a dog"), "nor its bytes");

  // A copy that does not open (another key, or altered) is removed.
  const other = await key();
  assert.equal(await K.get({ indexedDB: idb, key: other, scope: SCOPE, meta: m }), null, "another key opens nothing");
  assert.equal((await K.usage({ indexedDB: idb })).count, 0, "and the copy that did not open is gone");
});

// ── 2. The cap ───────────────────────────────────────────────────────────

test("the cap: the oldest opened go first, a file over the cap is not kept, 0 keeps nothing", async () => {
  const idb = fakeIndexedDB();
  const k = await key();
  // Room for three: each copy is its bytes and the 16 of its seal.
  const cap = 3 * (100 * 1024 + 16);
  for (const n of [1, 2, 3]) assert.equal(await K.put({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(n), bytes: fill(100 * 1024, n), cap }), true);
  // Opening 1 makes it the newest: when 4 comes, 2 goes (the oldest opened), not 1.
  assert.ok(await K.get({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(1) }), "1 opened again");
  await K.put({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(4), bytes: fill(100 * 1024, 4), cap });
  const still = async (n) => !!(await K.get({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(n) }));
  assert.equal(await still(2), false, "the oldest opened went");
  assert.equal(await still(1), true, "the one opened again stayed");
  assert.equal(await still(3), true);
  assert.equal(await still(4), true);
  const u = await K.usage({ indexedDB: idb });
  assert.ok(u.bytes <= cap && u.count === 3, "within the cap");

  assert.equal(await K.put({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(5), bytes: fill(cap + 1, 5), cap }), false, "a file larger than the cap is not kept");
  assert.equal(await K.put({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(6), bytes: fill(10, 6), cap: 0 }), false, "a cap of 0 keeps nothing");
  assert.equal((await K.usage({ indexedDB: idb })).count, 3, "and nothing was pushed out by either");

  // A copy left with no size beside it (a write that did not finish) goes on the next trim.
  const db = idb.dbs.get(K.DB_NAME);
  db.stores.get("files").rows.set("orphan", { k: "orphan", iv: new Uint8Array(12), ct: new Uint8Array(40) });
  await K.trim({ indexedDB: idb, cap });
  assert.equal(db.stores.get("files").rows.has("orphan"), false, "the orphan went");
  assert.equal(db.stores.get("files").rows.size, 3, "the others stayed");
});

// ── 3. Settings > Storage ────────────────────────────────────────────────

/** A small document: elements keep their children, text, value and listeners. */
function fakeDocument() {
  function el(tag) {
    const e = {
      tagName: tag.toUpperCase(), children: [], dataset: {}, style: {}, listeners: {}, className: "", textContent: "",
      value: "", selected: false, disabled: false, type: "",
      appendChild(c) { this.children.push(c); if (this.tagName === "SELECT" && c.selected) this.value = c.value; return c; },
      addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); },
      async fire(t) { for (const f of this.listeners[t] || []) await f({ target: this }); },
      replaceChildren() { this.children = []; },
    };
    return e;
  }
  return { createElement: el };
}
const walk = (n, out = []) => { out.push(n); for (const c of n.children || []) walk(c, out); return out; };
const findBy = (root, pred) => walk(root).find(pred);

test("Settings: 200 MB unless chosen, what is kept, a new cap trims at once, Clear asks then removes every copy", async () => {
  assert.equal(K.capMb(storage()), K.DEFAULT_CAP_MB, "the default");
  assert.equal(K.DEFAULT_CAP_MB, 200, "200 MB on the web");
  assert.equal(K.capMb(storage({ [K.CAP_KEY]: "0" })), 0, "0 is a choice");
  assert.equal(K.capMb(storage({ [K.CAP_KEY]: "bad" })), 200, "an unreadable one is the default");
  assert.equal(K.capBytes(storage({ [K.CAP_KEY]: "50" })), 50 * MIB);

  const idb = fakeIndexedDB();
  const k = await key();
  for (const n of [1, 2, 3]) await K.put({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(n), bytes: fill(30 * MIB, n), cap: 200 * MIB });
  const st = storage();
  const doc = fakeDocument();
  const box = doc.createElement("div");
  const asked = [];
  let answer = false;
  const ui = K.mountControls(box, { document: doc, storage: st, indexedDB: idb, confirm: async (w) => { asked.push(w); return answer; } });
  await ui.ready;
  const usage = findBy(box, (e) => e.dataset.keptUsage);
  assert.ok(/^Kept on this device: 3 files, 90\.0 MB\.$/.test(usage.textContent), "what is kept: " + usage.textContent);
  const sel = findBy(box, (e) => e.dataset.keptCap);
  assert.equal(sel.value, "200", "the cap shown is 200 MB");
  assert.ok(sel.children.some((o) => o.value === "0" && o.textContent === K.WORDS.capNone), "and Nothing is offered");

  // 50 MB: kept at once to the newest that fit.
  sel.value = "50";
  await sel.fire("change");
  assert.equal(st.m.get(K.CAP_KEY), "50", "the new cap is kept");
  assert.ok(/^Kept on this device: 1 file, 30\.0 MB\.$/.test(usage.textContent), "trimmed at once: " + usage.textContent);
  assert.ok(await K.get({ indexedDB: idb, key: k, scope: SCOPE, meta: meta(3) }), "the newest stayed");

  // Clear asks first; No changes nothing; Yes removes every copy.
  const clear = findBy(box, (e) => e.dataset.keptClear);
  assert.equal(clear.textContent, "Clear kept files");
  await clear.fire("click");
  assert.deepEqual(asked, [K.WORDS.clearConfirm], "it asked");
  assert.equal((await K.usage({ indexedDB: idb })).count, 1, "No: nothing removed");
  answer = true;
  await clear.fire("click");
  assert.equal((await K.usage({ indexedDB: idb })).count, 0, "Yes: every copy removed");
  assert.equal(usage.textContent, K.WORDS.nothing, "and it says so");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  K1 kept-files.js, recordId over the address alone (no scope, no key): test 1 "not under another
//     identity or server".
//  K2 kept-files.js, get() not making the copy it opened the newest: test 2 "the oldest opened
//     went" (the one opened again went in its place).
//  K3 kept-files.js, put() keeping a file larger than the cap: test 2 "a file larger than the cap is
//     not kept".
//  K4 kept-files.js, mountControls' Clear not asking first: test 3 "it asked".
//  K5 kept-files.js, the default cap 1000 MB: test 3 "200 MB on the web".
