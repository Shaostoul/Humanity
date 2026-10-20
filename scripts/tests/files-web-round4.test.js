// The Media page on the website, round 4 (2026-10-11): docs/design/files-showcase.md "Round 4: fixes
// from the round 3 review", its web items W4-1 to W4-3 and the decision at its top, a cancel means
// nothing stays. W4-4 (the chat's opened-picture cache) is at the end of
// scripts/tests/private-files-web.test.js, which loads the chat's own scripts.
//
// Run: node --test scripts/tests/files-web-round4.test.js   (in `just rig-tests` with the others)
//
// Against the stand-ins in scripts/tests/files-web-harness.js, whose stand-in relay
// (`resumableRelay`) answers a cancel the way the spec states round 4's relay does: 409
// `{busy: true}` while a piece or the finish is being written, `{ok: true, removed_stored: true}`
// on a finished upload not yet published (its stored file deleted with it), `{ok: true,
// published: true}` on one already published (its item left), else `{ok: true}`. Written before the
// relay's round 4 landed, against those shapes, then checked against it as built (a18ff7d63): the
// same, plus a 500 in plain words, everything kept, when the stored file cannot be deleted (W4-3f).
// Every wait goes through a stand-in sleep that
// records how long it was asked for and ends only when the test wakes it. HOS_WEB_DIR points the
// test at another copy of web/ (used to see each test red).
//
// What it proves:
//  W4-1. Every wait returns at once when the upload is stopping. Cancel or Pause pressed while the
//        start is on its way (it is not stopped: its answer names the session) and answered with
//        the hour's allowance, a server error, a proxy's rate limit or a dropped connection ends
//        the upload at once: nothing sleeps and the wait's words never show. The same for Pause on
//        a piece whose answer was already coming when it was pressed.
//  W4-2. A refused ticket renewal (a ban, the item gone, too many open) puts the relay's words
//        where the video was, instead of a dead video with no words; opening the item again asks
//        for a ticket afresh.
//  W4-3. Cancel uses the cancel answer: no status is asked first; the server is asked again while
//        it says busy, for as long as it does; `removed_stored` means the server deleted the file
//        the finish had stored, so nothing is removed by hand and the page says so; `published`
//        says the file stays on the Media page; a cancel the server never heard is said, not taken
//        for done, and one it answered with an error every time (its 500 when the stored file
//        cannot be deleted) is said in its words. The unfinished uploads' Cancel uses the same
//        answer.
//
// Seen red 2026-10-11: each with one break made in a copy of web/ under HOS_WEB_DIR, then green
// again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");

const H = require("./files-web-harness.js");
const {
  media, upload, MIB, byId, text, respond, path0, at, settle, item, listAnswer, mineAnswer, listRoutes, openPage,
  T0, aFile, sleeper, drive, resumableRelay, uploaderOn, NGINX_503, NGINX_429,
} = H;
const W = media.FILES_WORDS;

// The page's upload list (as in files-web-round3.test.js).
const entries = (doc) => byId(doc, "files-uploads").querySelectorAll("[data-upload]");
const entryButton = (entry, action) => entry.querySelector('[data-upload-action="' + action + '"]');
const notice = (doc) => text(byId(doc, "files-notice").querySelector(".files-notice-text"));

async function pickFile(doc, file) {
  const input = byId(doc, "files-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

/** An AbortController whose abort comes too late: the answer was already on its way. */
class TooLate {
  constructor() { this.signal = { aborted: false, addEventListener() {} }; }
  abort() { /* the request had already been answered */ }
}

// The answers a wait follows from, as the relay and nginx send them.
const BUSY = { error: "Another piece of this upload is being written. Wait for its answer, then ask where the upload is (GET) and carry on from there.", busy: true };
const WAITS = {
  "the hour's allowance": () => respond(429, { error: "This server takes 3 uploads an hour from your role.", allowance: "hour", resets_at: T0 + 3600000, resets_in_s: 3600, left_bytes: 0 }),
  "a server error": () => respond(503, NGINX_503),
  "a proxy's rate limit": () => respond(429, NGINX_429),
  "a dropped connection": () => { throw new TypeError("Failed to fetch"); },
};

// ── W4-1. Every wait returns at once when stopping ───────────────────────

// Seen red 2026-10-11 with nap's first line (`if (stopping()) return Promise.resolve();`) taken
// out: "the hour's allowance: cancelled at once / actual 'allowance', expected 'cancelled'" (it
// went to sleep for the server's 3,600,000 ms, the review's proof).
test("W4-1a. Cancel while the start is on its way: whatever wait its answer asks for, the upload ends at once and nothing sleeps", async () => {
  for (const [name, answer] of Object.entries(WAITS)) {
    let release;
    const relay = resumableRelay({ start: async () => { await new Promise((r) => { release = r; }); return answer(); } });
    const up = uploaderOn(relay, aFile("clip.mp4", 20 * MIB));
    up.u.start();
    await settle();
    assert.equal(up.u.state.phase, "starting", name + ": the start is on its way");
    up.u.cancel();
    release();
    await settle();
    assert.equal(up.u.state.phase, "cancelled", name + ": cancelled at once");
    assert.deepEqual(up.sl.waits, [], name + ": nothing slept");
    assert.deepEqual(up.phases.slice(up.phases.indexOf("cancelling")), ["cancelling", "cancelled"], name + ": the wait's words never shown");
    assert.equal(relay.log.length, 1, name + ": no request after the start (it named no session)");
  }
});

// Seen red 2026-10-11 with the same break: "the hour's allowance: paused at once / actual
// 'allowance', expected 'paused'" (the start; the piece's cases come after it in the test).
test("W4-1b. Pause while the start is on its way, or on a piece whose answer was already coming: the wait never begins, and Resume carries on", async () => {
  for (const [name, answer] of Object.entries(WAITS)) {
    let release = null;
    let held = true;
    const relay = resumableRelay({ start: async () => {
      if (!held) return null;
      held = false;
      await new Promise((r) => { release = r; });
      return answer();
    } });
    const up = uploaderOn(relay, aFile("clip.mp4", 20 * MIB));
    up.u.start();
    await settle();
    up.u.pause();
    release();
    await settle();
    assert.equal(up.u.state.phase, "paused", name + ": paused at once");
    assert.deepEqual(up.sl.waits, [], name + ": nothing slept");
    up.u.resume();
    await drive(up.sl, () => up.u.state.phase === "done");
    assert.equal(up.u.state.phase, "done", name + ": Resume carries on");
  }
  // A piece: Pause pressed as its answer was already coming (the abort too late to stop it).
  const pieceWaits = {
    "the piece's allowance": () => respond(429, { error: "Over the day's allowance.", allowance: "day", resets_at: T0 + 1500000, resets_in_s: 1500, left_bytes: 0, offset: 8 * MIB, size: 20 * MIB }),
    "the piece's server error": () => respond(502, "The relay is restarting."),
    "the piece's busy answer": () => respond(409, BUSY),
  };
  for (const [name, answer] of Object.entries(pieceWaits)) {
    let release = null;
    let held = true;
    const relay = resumableRelay({ piece: async (_c, _s, off) => {
      if (off !== 8 * MIB || !held) return null;
      held = false;
      await new Promise((r) => { release = r; });
      return answer();
    } });
    const up = uploaderOn(relay, aFile("clip.mp4", 20 * MIB), { AbortController: TooLate });
    up.u.start();
    await settle();
    assert.equal(typeof release, "function", name + ": the second piece is on its way");
    up.u.pause();
    release();
    await settle();
    assert.equal(up.u.state.phase, "paused", name + ": paused at once");
    assert.deepEqual(up.sl.waits, [], name + ": nothing slept");
    up.u.resume();
    await drive(up.sl, () => up.u.state.phase === "done");
    assert.equal(up.u.state.phase, "done", name + ": Resume carries on");
  }
});

// ── W4-2. A refused ticket renewal says so on the video ──────────────────

// Seen red 2026-10-11 with renewTicket's refusal put back to `() => { e.renewing = false; }`
// (round 3's): "403: no dead video left / actual [FakeElement], expected null" (the dead video
// stayed, with no words); and with openViewer's deletion of a refused ticket taken out: "403:
// opening it again asks afresh" (the refusal was shown again without asking).
test("W4-2. a ticket renewal the relay refuses puts its words where the video was; opening the item again asks afresh", async () => {
  const refusals = [
    [403, "Only members of this server who are not banned can open members-only files on the Files page."],
    [404, "That item is not there any more."],
    [429, "Too many members-only files have been opened from this account in the last ten minutes. Wait a few minutes and open it again."],
  ];
  for (const [status, words] of refusals) {
    let t = T0;
    let n = 0;
    let refuse = false;
    const clip = item({ id: 4, url: "/api/files/file/4", visibility: "members", kind: "video", title: "Fire show", filename: "4_fire.mp4" });
    const { doc, net } = await openPage({ now: () => t, routes: listRoutes(listAnswer([clip]), [
      [at("/api/files/ticket", "POST"), () => (refuse ? respond(status, words) : respond(200, { url: "/api/files/file/4?ticket=t" + (++n), expires_in_s: 600 }))],
    ]) });
    const tickets = () => net.calls.filter((c) => c.url === "/api/files/ticket").length;
    byId(doc, "files-grid").querySelector(".files-thumb").click();
    await settle();
    const video = byId(doc, "files-viewer-open").querySelector("video");
    assert.equal(video.src, "/api/files/file/4?ticket=t1", status + ": playing through its ticket");
    // Twelve minutes in, the old ticket has run out and the new one is refused.
    t += 12 * 60 * 1000;
    refuse = true;
    video.dispatch("error");
    await settle();
    assert.equal(tickets(), 2, status + ": a new ticket was asked for");
    assert.equal(byId(doc, "files-viewer-open").querySelector("video"), null, status + ": no dead video left");
    assert.equal(text(byId(doc, "files-viewer-wait")), W.fileFailed + words, status + ": the relay's words where the video was");
    await settle();
    assert.equal(tickets(), 2, status + ": and not asked again by itself");
    // Opened again (as the relay's words for too many open say): a ticket afresh, and it plays.
    refuse = false;
    byId(doc, "files-viewer-close").click();
    await settle();
    byId(doc, "files-grid").querySelector(".files-thumb").click();
    await settle();
    const again = byId(doc, "files-viewer-open").querySelector("video");
    assert.ok(again, status + ": opening it again asks afresh");
    assert.equal(again.src, "/api/files/file/4?ticket=t2", status + ": with a ticket of its own");
  }
});

// ── W4-3. The cancel answer ──────────────────────────────────────────────

// Seen red 2026-10-11 with cancelSession ignoring `removed_stored` (every ok answer taken as
// 'deleted'): "the uploader knows the server deleted the stored file / actual 'deleted', expected
// 'removed_stored'".
test("W4-3a. Cancel during the finish: asked again while the server is busy, and the server deletes the file the finish stored, with no status asked", async () => {
  let finishing;
  const relay = resumableRelay({ finish: async () => { await new Promise((r) => { finishing = r; }); return null; } });
  const up = uploaderOn(relay, aFile("clip.mp4", 10 * MIB));
  up.u.start();
  await settle();
  assert.equal(typeof finishing, "function", "the server is storing the last piece");
  up.u.cancel();
  await settle();
  const dels = () => relay.log.filter((e) => e.kind === "DELETE");
  assert.equal(dels().length, 1, "asked once while busy");
  assert.deepEqual(up.sl.waits, [2000], "then a wait");
  assert.equal(up.u.state.phase, "cancelling");
  finishing();
  await drive(up.sl, () => up.u.state.phase === "cancelled");
  assert.equal(dels().length, 2, "asked again once the server is no longer busy");
  assert.equal(up.u.state.cancelAnswer, "removed_stored", "the uploader knows the server deleted the stored file");
  const sess = relay.sessions.get(dels()[0].id);
  assert.ok(sess.done && sess.storedRemoved && sess.deleted, "nothing stays on the server");
  assert.equal(relay.log.filter((e) => e.kind === "GET").length, 0, "no status asked first");
});

// The review's timing (finding 4): the finish stored the file in the moment between round 3's
// status ("not done") and its DELETE, so round 3 removed nothing and the stored file stayed.
// Seen red 2026-10-11 against round 3's web/ (`git archive 3077155ed web` into a copy): "the page
// says the stored file was deleted too / actual null" (round 3 asked its status first, heard "not
// done", and took no notice of the cancel's answer); and with the same `removed_stored` break as
// W4-3a: the same words.
test("W4-3b. the review's timing: the finish stores the file just before the cancel arrives; the server deletes it with the cancel and the page says so, removing nothing by hand", async () => {
  let finishing = null;
  const relay = resumableRelay({
    finish: async () => { await new Promise((r) => { finishing = r; }); return null; },
    // The cancel arrives the moment after the finish stored the file.
    del: async () => {
      if (finishing) { const f = finishing; finishing = null; f(); await settle(); }
      return null;
    },
  });
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([]), [
    ...relay.routes,
    [at("/api/files/remove", "POST"), () => respond(200, { ok: true, file_removed: true, file_kept: false })],
  ]) });
  await pickFile(doc, aFile("clip.mp4", 10 * MIB));
  byId(doc, "files-form-go").click();
  await settle();
  assert.equal(typeof finishing, "function", "the server is storing the last piece");
  entryButton(entries(doc)[0], "cancel").click();
  for (let i = 0; i < 5 && entries(doc).length; i++) await settle();
  await settle();
  const del = relay.log.filter((e) => e.kind === "DELETE");
  assert.equal(del.length, 1, "one cancel");
  const sess = relay.sessions.get(del[0].id);
  assert.ok(sess.done && sess.storedRemoved, "the stored file is gone: nothing stays");
  assert.equal(notice(doc), W.cancelledStored, "the page says the stored file was deleted too");
  assert.equal(net.calls.filter((c) => c.url === "/api/files/remove").length, 0, "nothing is removed by hand");
  assert.equal(net.calls.filter((c) => c.url === "/api/files" && c.init.method === "POST").length, 0, "and nothing is published");
  assert.equal(entries(doc).length, 0, "and it leaves the list");
});

/** cancelSession against a list of answers (a Response, or an Error thrown as a dropped connection). */
async function cancelWith(answers, token) {
  const calls = [];
  let i = 0;
  const fetch = async (url, init) => {
    calls.push({ url, init });
    const a = answers[Math.min(i++, answers.length - 1)];
    if (a instanceof Error) throw a;
    return a;
  };
  const sl = sleeper();
  let told = null;
  upload.cancelSession({ fetch, token: token || (() => "tok-1"), sleep: sl.sleep }, "a".repeat(32)).then((t) => { told = t; });
  await drive(sl, () => told !== null, 60);
  return { told, calls, waits: sl.waits };
}

// Seen red 2026-10-11 with a busy answer counted as a try (`if (++tries >= CANCEL_TRIES) return
// { answer: 'unconfirmed', words }` in the busy branch): "busy for longer than the tries: still
// asked until it is not / actual { answer: 'unconfirmed' }, expected { answer: 'removed_stored' }".
test("W4-3c. the cancel answer one by one: busy is asked again for as long as it lasts; published, gone, a stale token, a refusal and a server never heard", async () => {
  const busy = respond(409, BUSY);
  const n = upload.CANCEL_TRIES + 3;
  const r1 = await cancelWith([...Array(n).fill(busy), respond(200, { ok: true, removed_stored: true })]);
  assert.deepEqual(r1.told, { answer: "removed_stored", words: "" }, "busy for longer than the tries: still asked until it is not");
  assert.equal(r1.calls.length, n + 1);
  assert.deepEqual(r1.waits.slice(0, 6), [2000, 4000, 8000, 16000, 30000, 30000], "after a growing wait each time");
  for (const c of r1.calls) {
    assert.equal(c.init.method, "DELETE");
    assert.equal(c.url, "/api/upload/resumable/" + "a".repeat(32));
    assert.equal(c.init.headers["X-Upload-Token"], "tok-1", "the token in the header");
  }
  assert.deepEqual((await cancelWith([respond(200, { ok: true, published: true })])).told, { answer: "published", words: "" }, "already published: its item stays");
  assert.deepEqual((await cancelWith([respond(200, { ok: true })])).told, { answer: "deleted", words: "" });
  assert.deepEqual((await cancelWith([respond(404, "There is no upload with that id.")])).told, { answer: "gone", words: "" });
  const stale = await cancelWith([respond(403, "Invalid upload token."), respond(503, NGINX_503), respond(200, { ok: true })]);
  assert.deepEqual(stale.told, { answer: "deleted", words: "" }, "a stale token and a server error are tried again");
  assert.deepEqual(stale.waits, [2000, 4000]);
  const tokens = ["", ""];
  const signingIn = await cancelWith([respond(200, { ok: true })], () => (tokens.length ? tokens.shift() : "tok-2"));
  assert.deepEqual(signingIn.waits, [1000, 1000], "no token yet: it waits for the page to sign in again");
  assert.equal(signingIn.calls[0].init.headers["X-Upload-Token"], "tok-2");
  assert.deepEqual((await cancelWith([respond(400, "That is not an upload id.")])).told, { answer: "refused", words: "That is not an upload id." });
  const never = await cancelWith([new TypeError("Failed to fetch")]);
  assert.equal(never.told.answer, "unconfirmed", "a server never heard is not taken for done");
  assert.equal(never.calls.length, upload.CANCEL_TRIES);
  assert.deepEqual(never.waits, [2000, 4000, 8000, 16000, 30000, 30000, 30000, 30000, 30000]);
  const failing = await cancelWith([respond(503, NGINX_503)]);
  assert.deepEqual(failing.told, { answer: "unconfirmed", words: "The server answered 503." });
  // The relay's 500 when the stored file could not be deleted keeps everything: asked again.
  const CANNOT = "The uploaded file could not be removed from the server (Access is denied. (os error 5)). Try again in a moment.";
  const once = await cancelWith([respond(500, CANNOT), respond(200, { ok: true, removed_stored: true })]);
  assert.deepEqual(once.told, { answer: "removed_stored", words: "" }, "a 500 is asked again");
  assert.deepEqual(once.waits, [2000]);
});

// Seen red 2026-10-11 with the page's onChange for 'cancelled' not saying the answer (the
// `sayCancelled` line taken out): "a cancel the server never heard is said / actual null, expected
// 'The cancel could not reach the server, ...'".
test("W4-3d. Cancel on the page: a cancel the server never heard is said, not taken for done", async () => {
  const relay = resumableRelay({
    piece: async (_c, _s, off) => { if (off === 8 * MIB) await new Promise(() => {}); return null; },
    del: async () => { throw new TypeError("Failed to fetch"); },
  });
  const sl = sleeper();
  const { doc } = await openPage({ sleep: sl.sleep, routes: listRoutes(listAnswer([]), relay.routes) });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "files-form-go").click();
  await settle();
  entryButton(entries(doc)[0], "cancel").click();
  await drive(sl, () => entries(doc).length === 0, 30);
  assert.equal(relay.log.filter((e) => e.kind === "DELETE").length, upload.CANCEL_TRIES, "tried that many times");
  assert.equal(notice(doc), W.cancelUnconfirmed, "a cancel the server never heard is said");
  assert.ok(byId(doc, "files-notice").className.includes("files-warn"), "as a warning");
});

// The relay as built (a18ff7d63): a finished upload whose stored file cannot be deleted is answered
// 500 in plain words, and every row is kept so the app may ask again.
// Seen red 2026-10-11 with sayCancelled saying round 4's first words for every unconfirmed cancel
// (`say(FILES_WORDS.cancelUnconfirmed, true)`, "could not reach the server" for a server that
// answered): "the server's own words, not 'could not reach' / actual 'The cancel could not reach
// the server, ...'".
test("W4-3f. a stored file the server cannot delete: the cancel is asked again, then said in the server's words, never taken for done", async () => {
  const CANNOT = "The uploaded file could not be removed from the server (Access is denied. (os error 5)). Try again in a moment.";
  let finishing = null;
  const relay = resumableRelay({
    finish: async () => { await new Promise((r) => { finishing = r; }); return null; },
    // The cancel arrives just after the finish stored the file, which the server cannot delete.
    del: async (_c, sess) => {
      if (finishing) { const f = finishing; finishing = null; f(); await settle(); sess.cannotDelete = true; }
      return null;
    },
  });
  const sl = sleeper();
  const { doc } = await openPage({ sleep: sl.sleep, routes: listRoutes(listAnswer([]), relay.routes) });
  await pickFile(doc, aFile("clip.mp4", 10 * MIB));
  byId(doc, "files-form-go").click();
  await settle();
  assert.equal(typeof finishing, "function", "the server is storing the last piece");
  entryButton(entries(doc)[0], "cancel").click();
  await drive(sl, () => entries(doc).length === 0, 30);
  const del = relay.log.filter((e) => e.kind === "DELETE");
  assert.equal(del.length, upload.CANCEL_TRIES, "asked again, that many times");
  const sess = relay.sessions.get(del[0].id);
  assert.ok(sess.done && !sess.deleted && !sess.storedRemoved, "everything stays on the server, to try again");
  assert.equal(notice(doc), W.cancelNotFinished + CANNOT, "the server's own words, not 'could not reach'");
  assert.ok(byId(doc, "files-notice").className.includes("files-warn"), "as a warning");
});

// Seen red 2026-10-11 with cancelUnfinished put back to round 3's single DELETE (busy shown as a
// refusal, never asked again): "the row says it is cancelling / actual 'Cancel', expected
// 'Cancelling...'" (it had already given up); and with its row not marked while it goes, the same.
test("W4-3e. the unfinished uploads' Cancel uses the same answer: asked again while busy, its row saying so; a published file is said to stay", async () => {
  for (const [answer, words] of [[{ ok: true }, W.unfinishedCancelled], [{ ok: true, published: true }, W.cancelledPublished]]) {
    const id = "f".repeat(32);
    let busy = 2;
    const deletes = [];
    let mine = mineAnswer({ uploads_in_progress: [{ id, filename: "Old clip.mp4", size: 900 * MIB, offset: 312 * MIB, touched_at: T0 - 3600000 }] });
    const sl = sleeper();
    const { doc } = await openPage({ sleep: sl.sleep, routes: listRoutes(listAnswer([]), [
      [at("/api/files/mine", "POST"), () => respond(200, mine)],
      [(c) => path0(c.url) === "/api/upload/resumable/" + id && c.init.method === "DELETE", (c) => {
        deletes.push(c);
        if (busy-- > 0) return respond(409, BUSY);
        mine = mineAnswer();
        return respond(200, answer);
      }],
    ]) });
    const button = () => byId(doc, "files-unfinished-list").querySelector("[data-unfinished-cancel]");
    button().click();
    await settle();
    assert.equal(deletes.length, 1, "asked once, busy");
    assert.equal(text(button()), W.cancellingUnfinished, "the row says it is cancelling");
    assert.ok(button().disabled);
    button().dispatch("click");
    await settle();
    assert.equal(deletes.length, 1, "a second press while it goes sends nothing more");
    await drive(sl, () => notice(doc), 10);
    await settle();
    assert.equal(deletes.length, 3, "asked again while busy");
    assert.equal(notice(doc), words);
    assert.equal(byId(doc, "files-unfinished-list"), null, "and the list follows the server");
  }
});
