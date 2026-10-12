// ── chat-file-transfer.js ─────────────────────────────────────────────────
// Device-to-device big files (2026-10-11, docs/design/media-showcase.md,
// "Device-to-device big files, exact" and "...: the shapes, exact"; the
// operator's answer 10). In a direct message, a file bigger than my private
// file limit never goes to the server's disk: it goes from this device to the
// other person's, over a WebRTC data channel, once they have said yes.
//
// What this file does, in the order it happens:
//  1. The attach step (chat-messages.js sendEncryptedAttachment calls
//     offerFileDirect for a file over the limit in a DM) says why it goes this
//     way ("Files over 6 MB go straight to Ann's device instead of through the
//     server."), the file is cleaned as every file is, then hashed a piece at a
//     time (/shared/blake3.js), and I mint a file pass for this one transfer
//     (Dilithium3 over hum/file-pass/v1\n{server}\n{me}\n{them}\n{id}\n{expiry}).
//  2. The offer goes as a sealed DM note, [[hum:offer:v1]]{...}, never shown
//     as a message (/shared/file-transfer.js holds its exact text).
//  3. Their app (this file, on their side) applies "Files in messages" first:
//     a sender it does not accept files from is never asked about (a quiet row
//     says so; no answer goes back, which would tell the sender they were
//     online). Otherwise: "Ann wants to send you clip.mp4 (2.3 GB). Accept?"
//     Accept or Decline goes back as [[hum:offer-answer:v1]]{id, accept}.
//  4. On Accept the receiver dials the sender: a peer connection with one data
//     channel labelled file:<id>, its dc_offer carrying the pass (the relay
//     checks it and lets the offer through). Both ends use only the server's
//     call relay (iceTransportPolicy 'relay', credentials from
//     `call_credentials` with {file, peer, file_pass}) unless the two are
//     friends and the person turned direct connections on (Safety > Files in
//     messages), so neither sees the other's network address by default.
//  5. The channel protocol (/shared/file-transfer.js Sender and Receiver):
//     hashes, get windows, 64 KiB messages, piece checks, resume, done, pause,
//     resume, cancel. Pieces land in IndexedDB as they are checked; the file is
//     saved (through the File System Access API where the browser has it,
//     otherwise as a Blob download) only after the whole file's BLAKE3 matches.
//  6. A row per transfer in the conversation, on both sides: the name, the
//     size, a progress bar, Pause, Resume and Cancel, and words for each state.
//
// One connection with each person at a time, carrying several transfers (rule 2
// of "Device-to-device big files: rules settled by the first review",
// 2026-10-12): one data channel per transfer, labelled file:<id>, each with its
// own engine, on the dialling side and the answering side alike. A transfer's
// channel is opened by its receiver, whichever side dialled, so a file each way
// rides one connection. A channel ends without touching the others, and the
// connection closes when its last channel has ended. Each channel's label says
// which file it is.
//
// The receiver's answer and cancel notes carry the offer's file pass on their
// dm_put (`file_pass`, rule 4), so they reach a sender who takes messages only
// from friends.
//
// Settled by the second review (2026-10-12), rules 7 to 9:
//  7. A dial names its transfer: the relay passes a dc_offer it let in on a
//     file pass to the sender with that pass's id as `file`. The sender asks
//     the server's call relay for that one transfer's credentials and makes ONE
//     allocation for the connection; every other transfer with that person
//     rides it as its own channel. (Before, the sender asked for each of its
//     live offers to that person, up to four: idle allocations used up the
//     person's share, and a dial for a fifth offer matched none.) A dc_offer
//     with no `file` is no file dial.
//  8. An end is always said: a transfer that ends here for any reason other
//     than done says cancel on the channel and sends the cancel note (ftFail).
//     A channel the other side closes while the connection stays up ends a
//     sending transfer ("Ben's side stopped it"); a receiving transfer still
//     live opens its channel again after a wait (ftScheduleRetry), and a pause
//     the other side held is cleared.
//  9. A dial is an answer: a sender still waiting for the answer note takes its
//     channel opening as Accept and done as Sent (ftBindChannel).
//
// Kept on this device: each transfer's record (encrypted with the DM store's
// key, /chat/chat-dm-store.js sealBox) and the pieces received so far (in
// IndexedDB, `humanity_file_transfers`, until the file is saved or the
// transfer ends). The sender's file itself is never kept: after a reload the
// row asks for it to be chosen again, and checks it is the same file.
//
// Depends on: app.js (ws, myKey, myDilithiumSecret, peerData, activeDmPartner,
//   appendMessage, addSystemMessage, shortKey, handleMessage), crypto.js
//   (pqBuildDmPuts, window.pqSignMessage, window.pqVerifyMessage), chat-dms.js
//   (privateFilesSettings, privateFileSender, privateFileHidden,
//   upsertDmConversation), chat-voice-rooms.js (requestCallCredentials,
//   relayOnlyRtcConfig), chat-p2p.js (ownDeviceRtcConfig), chat-messages.js
//   (prepareAttachment, attachmentLimit, acceptsFilesKnown,
//   privateConversationNow), /shared/file-transfer.js, /shared/blake3.js,
//   /shared/files-in-messages.js. Loaded after chat-p2p.js.
// Tests: scripts/tests/file-transfer-web.test.js (the protocol),
//        scripts/tests/file-transfer-chat-web.test.js (this file).
// ──────────────────────────────────────────────────────────────────────────

const FT_DB_NAME = 'humanity_file_transfers';
const FT_DB_VERSION = 1;
const FT_DIRECT_KEY = 'hos_file_direct';
const FT_STALL_MS = 30000;           // nothing for this long while a window is out: ask again
const FT_CONNECT_WAIT_MS = 45000;    // a dial not open by then is given up (and tried again later)
const FT_DIAL_RETRY_MS = 60000;      // while waiting for the sender, try again this often
const FT_STOPPED_WAIT_MS = 15000;    // a sending channel closed by the other side: how long to tell a stop from a drop
const FT_PUT_WAIT_MS = 30000;        // how long a note's put waits for the server's answer
const FT_PIECES_LINGER_MS = 120000;  // pieces stay this long after a save, while the browser copies them
const FT_KEEP_ENDED_MS = 30 * 24 * 3600 * 1000; // a finished transfer's row is kept this long
const FT_RECORD_EVERY = 16;          // a receiving transfer's record is saved every this many pieces

const FT_ACTIVE_SEND = ['preparing', 'offered', 'accepted', 'sending'];
const FT_ACTIVE_RECEIVE = ['asked', 'accepted', 'receiving', 'checking'];

/** id -> the transfer (see ftNewTransfer). */
const ftTransfers = new Map();
/** lower-case key of the other person -> the one connection with them. */
const ftLinks = new Map();
/** id -> the row drawn for it in the conversation on screen. */
const ftRows = new Map();
/** Keys the member list says are online. */
const ftOnline = new Set();
/** A note's put waiting for the server's answer: ref -> {id}. */
const ftPuts = new Map();
let ftLoadPromise = null;
let ftRetryTimer = 0;

function ftApi() { return window.HosFileTransfer || null; }
function ftWords() { const F = ftApi(); return F ? F.WORDS : null; }
function ftLower(k) { return typeof k === 'string' ? k.toLowerCase() : ''; }
function ftSameKey(a, b) { return typeof a === 'string' && typeof b === 'string' && a !== '' && a.toLowerCase() === b.toLowerCase(); }
function ftNowSecs() { return Math.floor(Date.now() / 1000); }
function ftStore() { return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null; }

/** The name a person goes by here. */
function ftName(key) {
  if (typeof reachDisplayName === 'function') return reachDisplayName(key);
  const p = (typeof peerData !== 'undefined' && peerData) ? peerData[key] : null;
  if (p && p.display_name) return p.display_name;
  return typeof shortKey === 'function' ? shortKey(key) : String(key || '').slice(0, 8);
}

/** A file's name fit to offer and to save: no control characters, no path, not too long. */
function ftSafeName(name) {
  // eslint-disable-next-line no-control-regex
  let s = String(name || '').replace(/[\u0000-\u001f\u007f]/g, ' ').replace(/[\\/]/g, '_').trim();
  if (!s) s = 'file';
  if (s.length > 200) {
    const dot = s.lastIndexOf('.');
    const ext = dot > 0 && s.length - dot <= 16 ? s.slice(dot) : '';
    s = s.slice(0, 200 - ext.length) + ext;
  }
  return s;
}

function ftHexBytes(hex) {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
  return out;
}

function ftB64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function ftFinished(t) {
  return !(t.role === 'send' ? FT_ACTIVE_SEND : FT_ACTIVE_RECEIVE).includes(t.state);
}

// ── Direct connections (Safety > Files in messages) ──

/** Has the person turned on direct connections for files with friends? Never with the protected setup on. */
function ftDirectSetting() {
  if (typeof protectedIsOn === 'function' && protectedIsOn()) return false;
  try { return window.localStorage.getItem(FT_DIRECT_KEY) === '1'; } catch (e) { return false; }
}

/** May a file between me and `peer` go straight between our devices: friends both ways, and the setting on. */
function ftDirectAllowed(peer) {
  if (!ftDirectSetting()) return false;
  const store = ftStore();
  if (!store) return false;
  // Friends: we follow each other, I hold their pass and they hold mine.
  return !!(store.isFriendPeer(peer) && store.certFor(peer) && store.certSentTo(peer));
}

/** The Safety page's row for it, drawn into Files in messages (chat-privacy.js drawFilesInMessages). */
function drawFileDirectRow(box, row) {
  const W = ftWords();
  if (!W || !box) return;
  const prot = typeof protectedIsOn === 'function' && protectedIsOn();
  if (prot) {
    box.appendChild(row(W.directSetting, null, W.directProtected));
    return;
  }
  const on = ftDirectSetting();
  const box2 = document.createElement('input');
  box2.type = 'checkbox';
  box2.checked = on;
  box2.dataset.filesSetting = 'direct';
  box2.setAttribute('aria-label', W.directSetting);
  box2.onchange = () => {
    try { window.localStorage.setItem(FT_DIRECT_KEY, box2.checked ? '1' : '0'); } catch (e) { /* not kept */ }
    if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
  };
  box.appendChild(row(W.directSetting, box2, on ? W.directSettingOn : W.directSettingOff));
}
window.drawFileDirectRow = drawFileDirectRow;

// ── Kept on this device ──

let ftDbPromise = null;
function ftDb() {
  if (ftDbPromise) return ftDbPromise;
  ftDbPromise = new Promise((resolve) => {
    let req;
    try { req = window.indexedDB.open(FT_DB_NAME, FT_DB_VERSION); } catch (e) { resolve(null); return; }
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('transfers')) db.createObjectStore('transfers', { keyPath: 'id' });
      if (!db.objectStoreNames.contains('pieces')) db.createObjectStore('pieces');
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => resolve(null);
  });
  return ftDbPromise;
}

/**
 * One request on one store; its result, or null when it failed. A write counts only once the
 * browser has committed it: a full disk is often reported when the transaction commits, after the
 * request itself succeeded, and a piece taken as kept then would only show at the end, as a file
 * that "does not match" (the review of 2026-10-12).
 */
async function ftIdb(store, mode, fn) {
  const db = await ftDb();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(store, mode);
      const r = fn(tx.objectStore(store));
      const result = () => (r.result === undefined ? null : r.result);
      if (mode === 'readwrite') {
        tx.oncomplete = () => resolve(result());
        tx.onerror = () => resolve(null);
        tx.onabort = () => resolve(null);
      } else {
        r.onsuccess = () => resolve(result());
        r.onerror = () => resolve(null);
      }
    } catch (e) {
      resolve(null);
    }
  });
}

function ftPieceKey(id, i) { return id + ':' + i; }

/** What of a transfer is kept (the file itself and the engines are not). */
function ftRecordOf(t) {
  return {
    id: t.id, role: t.role, peer: t.peer, offer: t.offer, offerTs: t.offerTs, state: t.state,
    reason: t.reason || null, kept: t.kept || 0, done: t.done || 0, pausedHere: !!t.pausedHere,
    refusedAccept: t.refusedAccept || null, name: t.name, size: t.size, type: t.type,
    pendingNotes: (t.pendingNotes || []).slice(0, 8), created: t.created, updated: Date.now(),
  };
}

/** Keep the transfer's record (encrypted with the DM store's key). */
async function ftSave(t) {
  const store = ftStore();
  if (!store || typeof store.sealBox !== 'function') return false;
  const box = await store.sealBox(ftRecordOf(t));
  if (!box) return false;
  return (await ftIdb('transfers', 'readwrite', (os) => os.put({ id: t.id, scope: store.scope, box }))) !== null;
}

async function ftForget(t) {
  await ftIdb('transfers', 'readwrite', (os) => os.delete(t.id));
}

/** The numbers of the pieces kept for a transfer (any order). */
async function ftHeldPieces(id) {
  const prefix = id + ':';
  // Only this transfer's keys where the browser can say so ("id:" up to, not including, "id;").
  const range = typeof IDBKeyRange !== 'undefined' ? IDBKeyRange.bound(prefix, id + ';', false, true) : undefined;
  const keys = (await ftIdb('pieces', 'readonly', (os) => (range ? os.getAllKeys(range) : os.getAllKeys()))) || [];
  const out = [];
  for (const k of keys) {
    if (typeof k !== 'string' || !k.startsWith(prefix)) continue;
    const i = Number(k.slice(prefix.length));
    if (Number.isSafeInteger(i) && i >= 0) out.push(i);
  }
  return out;
}

/** The pieces received for a transfer go (a cancel, a failure, or after the save). */
async function ftDropPieces(t) {
  for (const i of await ftHeldPieces(t.id)) await ftIdb('pieces', 'readwrite', (os) => os.delete(ftPieceKey(t.id, i)));
  t.kept = 0;
}

/**
 * The pieces store a Receiver uses: checked pieces, as Blobs, so nothing large is held in memory.
 * A piece is kept as soon as it checks, in whatever order it arrives (a piece asked again after a
 * bad hash comes after the ones asked before it).
 */
function ftPieceStore(t) {
  let puts = 0;
  return {
    async held() { return ftHeldPieces(t.id); },
    async put(i, bytes) {
      const ok = await ftIdb('pieces', 'readwrite', (os) => os.put(new Blob([bytes]), ftPieceKey(t.id, i)));
      if (ok === null) throw new Error('this browser could not keep a piece');
      // Ended (a cancel) while it was being written: it goes too, so nothing is left behind.
      if (ftFinished(t)) {
        await ftIdb('pieces', 'readwrite', (os) => os.delete(ftPieceKey(t.id, i)));
        return;
      }
      puts += 1;
      if (puts % FT_RECORD_EVERY === 0) ftSave(t);
    },
    async get(i) {
      const blob = await ftIdb('pieces', 'readonly', (os) => os.get(ftPieceKey(t.id, i)));
      return blob ? new Uint8Array(await blob.arrayBuffer()) : null;
    },
  };
}

/** Load this identity's transfers, once the DM store (and its key) is ready. */
function ftEnsureLoaded() {
  if (ftLoadPromise) return ftLoadPromise;
  const store = ftStore();
  if (!store || typeof store.openBox !== 'function') return Promise.resolve(false);
  ftLoadPromise = (async () => {
    const rows = (await ftIdb('transfers', 'readonly', (os) => os.getAll())) || [];
    const now = Date.now();
    for (const row of rows) {
      if (!row || row.scope !== store.scope || ftTransfers.has(row.id)) continue;
      const rec = await store.openBox(row.box);
      if (!rec || !rec.id) continue;
      if (rec.updated && now - rec.updated > FT_KEEP_ENDED_MS) { await ftForget(rec); continue; }
      const t = Object.assign(ftNewTransfer(rec.role, rec.peer, rec.id), rec);
      // A transfer is never "on its way" across a reload: the connection went with the page.
      if (t.state === 'sending') t.state = 'accepted';
      if (t.state === 'receiving') t.state = 'accepted';
      if (t.state === 'preparing') { t.state = 'failed'; t.reason = 'noFile'; }
      if (t.role === 'receive' && t.state === 'checking') t.state = 'accepted';
      ftTransfers.set(t.id, t);
    }
    ftExpireOld();
    return true;
  })();
  return ftLoadPromise;
}
window.loadFileTransfers = ftEnsureLoaded;

function ftNewTransfer(role, peer, id) {
  return {
    id, role, peer, offer: null, offerTs: 0, state: role === 'send' ? 'preparing' : 'asked', reason: null,
    done: 0, kept: 0, pausedHere: false, pausedThere: false, noServer: false, path: null,
    file: null, pieceHashes: null, hashing: null, receiver: null, saveHandle: null, pendingNotes: [],
    created: Date.now(), name: '', size: 0, type: '',
  };
}

// ── Notes (sealed DMs, never shown) ──

function ftPutRef() {
  const b = crypto.getRandomValues(new Uint8Array(12));
  return 'file-' + Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
}

/**
 * Seal and send a note to `peer`. opts.selfCopy: my other devices hear it too
 * (answers and cancels). opts.answerFor: the transfer whose row hears the
 * server's answer to the put. opts.filePass: the offer's file pass, carried on
 * the put to them as `file_pass` (the receiver's answer and cancel, rule 4: the
 * relay lets it through to a sender who takes messages only from friends,
 * because the pass is theirs, made out to me). Returns false when it could not
 * go now.
 */
async function ftSendNote(peer, text, opts) {
  const o = opts || {};
  if (!text || typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) return false;
  const built = await pqBuildDmPuts(text, peer, Date.now());
  if (!built || !built.recipientPut) return false;
  if (o.filePass) built.recipientPut.file_pass = o.filePass;
  if (o.answerFor) {
    const ref = ftPutRef();
    built.recipientPut.ref = ref;
    ftPuts.set(ref, { id: o.answerFor });
    setTimeout(() => ftPuts.delete(ref), FT_PUT_WAIT_MS);
  }
  ws.send(JSON.stringify(built.recipientPut));
  if (o.selfCopy && built.selfPut) ws.send(JSON.stringify(built.selfPut));
  return true;
}

/** Send a note now, or keep it to send when the connection is back. */
async function ftNote(t, text, opts) {
  if (await ftSendNote(t.peer, text, opts)) return true;
  t.pendingNotes.push({ text, opts: opts || {} });
  ftSave(t); // kept with the record, so a note that could not go survives the page closing
  return false;
}

/** Notes that could not go (no connection), sent now. */
async function ftFlushNotes() {
  for (const t of ftTransfers.values()) {
    while (t.pendingNotes.length) {
      const n = t.pendingNotes[0];
      if (!(await ftSendNote(t.peer, n.text, n.opts))) return;
      t.pendingNotes.shift();
    }
  }
}

/** The server's answer to a note's put (dm_put_ok / dm_put_refused). True when it was one of these. */
function ftPutAnswered(msg) {
  const p = msg && typeof msg.ref === 'string' ? ftPuts.get(msg.ref) : null;
  if (!p) return false;
  ftPuts.delete(msg.ref);
  const t = ftTransfers.get(p.id);
  if (!t || msg.type !== 'dm_put_refused') return true;
  if (t.state === 'offered') {
    t.state = 'undelivered';
    t.reason = msg.reason === 'reach' ? 'reach' : (msg.reason === 'rate' ? 'rate' : 'other');
    t.file = null;
    t.pieceHashes = null;
    ftSave(t);
    ftDraw(t);
  }
  return true;
}

// ── The file pass ──

/** Mint the pass for transfer `id` to `peer`: {v, id, expiry, sig}, or null when the identity or server is not ready. */
async function ftMintPass(peer, id) {
  const F = ftApi();
  const server = window.hosServerDid;
  if (!F || !server || typeof myDilithiumSecret === 'undefined' || !myDilithiumSecret || !myKey) return null;
  if (![server, myKey, peer, id].every(F.filePassFieldOk)) return null;
  const expiry = F.filePassExpiry(ftNowSecs());
  const words = F.filePassPreimage(server, myKey, peer, id, expiry);
  const sig = await window.pqSignMessage(myDilithiumSecret, new TextEncoder().encode(words));
  if (!sig) return null;
  return F.filePassObject(id, expiry, ftB64(sig));
}

/** Did `issuer` give `grantee` this pass for its transfer on this server? */
async function ftPassChecks(issuer, grantee, pass) {
  const F = ftApi();
  const server = window.hosServerDid;
  try {
    if (!F || !server || !pass || ![server, issuer, grantee].every(F.filePassFieldOk)) return false;
    const words = F.filePassPreimage(server, issuer, grantee, pass.id, pass.expiry);
    const sig = Uint8Array.from(atob(pass.sig), (c) => c.charCodeAt(0));
    return !!(await window.pqVerifyMessage(ftHexBytes(issuer), new TextEncoder().encode(words), sig));
  } catch (e) {
    return false;
  }
}

// ── Sending: the attach step's way for a big file in a DM ──

/**
 * Offer `file` to `peer` device to device (chat-messages.js sendEncryptedAttachment, for a file
 * over my private file limit in a DM). Returns the transfer, or null when it did not start.
 */
async function offerFileDirect(file, peer) {
  const F = ftApi();
  const W = ftWords();
  if (!F || !window.HosBlake3) {
    addSystemMessage('The file was not sent: this page could not load its file sender. Reload the page and try again.');
    return null;
  }
  if (!file || !(Number(file.size) > 0) || !peer) return null;
  const target = 'dm:' + peer;
  // The file's name goes in the sealed offer: never my recovery phrase (step F, chat-warnings.js).
  if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(file.name, 'The file was not sent.')) return null;
  const name = ftName(peer);
  const lines = [W.directLine(typeof attachmentLimit === 'function' ? attachmentLimit() : 6 * 1024 * 1024, name), W.keepOpen];
  let refuse = false;
  const known = typeof acceptsFilesKnown === 'function' ? acceptsFilesKnown(peer) : null;
  if (known && (known.value === 'nobody' || (known.value === 'friends' && !known.friend))) {
    lines.push(W.refusedBefore(name, known.value));
    refuse = true;
  }
  // Cleaned on this device first, as every file is (part 1): the switch and the words are the same.
  const ready = await prepareAttachment(file, target, { direct: { lines, refuse } });
  if (!ready) return null;
  if (privateConversationNow() !== target) {
    addSystemMessage('The file was not sent because another conversation was opened while it was being prepared.');
    return null;
  }
  await ftEnsureLoaded();
  const sending = ready.file;
  const t = ftNewTransfer('send', peer, F.newId(crypto));
  t.file = sending;
  t.name = ftSafeName(file.name);
  t.size = Number(sending.size);
  t.type = String(file.type || '').slice(0, 255);
  t.hashing = { done: 0, total: F.piecesFor(t.size) };
  ftTransfers.set(t.id, t);
  ftDraw(t);
  const hashed = await F.hashFile(sending, {
    blake3: window.HosBlake3,
    onProgress: (done, total) => { t.hashing = { done, total }; ftDrawSoon(t); },
    stopped: () => t.state !== 'preparing',
  });
  if (!hashed || t.state !== 'preparing') return t;
  t.pieceHashes = hashed.pieceHashes;
  t.hashing = null;
  const pass = await ftMintPass(peer, t.id);
  const offer = pass ? {
    id: t.id, name: t.name, size: t.size, type: t.type, blake3: hashed.blake3,
    piece: F.PIECE_BYTES, pieces: hashed.pieces, hashes: hashed.hashes, pass,
  } : null;
  const text = offer ? F.offerNoteText(offer) : null;
  if (!text) {
    ftEnd(t, 'failed', 'notSigned');
    return t;
  }
  t.offer = F.offerChecked(offer);
  t.offerTs = Date.now();
  t.state = 'offered';
  await ftSave(t);
  ftDraw(t);
  // The offer goes to them alone: my other devices do not have the file, so they hear nothing of it.
  if (!(await ftSendNote(peer, text, { answerFor: t.id }))) {
    t.state = 'undelivered';
    t.reason = 'other';
    await ftSave(t);
    ftDraw(t);
  }
  return t;
}
window.offerFileDirect = offerFileDirect;

/** After a reload the page no longer has the file: the person chooses it again, and it must be the same file. */
async function ftChooseAgain(t, file) {
  const F = ftApi();
  if (!F || !file || t.role !== 'send' || ftFinished(t)) return false;
  if (Number(file.size) !== t.offer.size) {
    t.reason = 'wrongFile';
    ftDraw(t);
    return false;
  }
  t.hashing = { done: 0, total: t.offer.pieces };
  t.reason = 'checkingAgain';
  ftDraw(t);
  const hashed = await F.hashFile(file, { blake3: window.HosBlake3, onProgress: (done, total) => { t.hashing = { done, total }; ftDrawSoon(t); } });
  t.hashing = null;
  if (!hashed || hashed.blake3 !== t.offer.blake3 || hashed.hashes !== t.offer.hashes) {
    t.reason = 'wrongFile';
    ftDraw(t);
    return false;
  }
  t.file = file;
  t.pieceHashes = hashed.pieceHashes;
  t.reason = null;
  ftDraw(t);
  return true;
}

// ── Receiving: the notes ──

/**
 * A sealed note about a transfer ([[hum:offer:v1]], [[hum:offer-answer:v1]],
 * [[hum:offer-cancel:v1]]), already opened and signature-checked
 * (chat-social.js ingestDmControl hands it here). Never shown as a message.
 */
async function ingestFileNote(inner) {
  const F = ftApi();
  if (!F || !inner || typeof inner.text !== 'string') return true;
  await ftEnsureLoaded();
  const fromMe = ftSameKey(inner.from, myKey);
  const peer = fromMe ? inner.to : inner.from;
  const offer = F.offerNoteParse(inner.text);
  if (offer) {
    if (!fromMe) await ftTakeOffer(offer, inner);
    return true;
  }
  const answer = F.answerNoteParse(inner.text);
  if (answer) {
    const t = ftTransfers.get(answer.id);
    if (!t || !ftSameKey(t.peer, peer)) return true;
    if (fromMe) {
      // My own answer: from another of my devices (this one's own comes back too, and changes nothing).
      if (t.role === 'receive' && t.state === 'asked') {
        t.state = 'elsewhere';
        await ftSave(t);
        ftDraw(t);
      }
      return true;
    }
    if (t.role === 'send' && (t.state === 'offered' || t.state === 'undelivered')) {
      t.state = answer.accept ? 'accepted' : 'declined';
      if (!answer.accept) { t.file = null; t.pieceHashes = null; }
      await ftSave(t);
      ftDraw(t);
    }
    return true;
  }
  const cancel = F.cancelNoteParse(inner.text);
  if (cancel) {
    const t = ftTransfers.get(cancel.id);
    if (t && ftSameKey(t.peer, peer) && !ftFinished(t)) await ftEnd(t, 'cancelled');
    return true;
  }
  return true;
}
window.ingestFileNote = ingestFileNote;

/** An offer from someone else. */
async function ftTakeOffer(offer, inner) {
  const F = ftApi();
  const from = inner.from;
  if (ftTransfers.has(offer.id)) return;
  // From someone my "who can reach me" settings refuse: the server should not have passed it on.
  if (typeof reachAllowsFrom === 'function' && typeof reachStore === 'function' && reachStore() && !reachAllowsFrom(from, 'message')) return;
  // The pass must be theirs, for me, for this file, on this server: the relay checks the same.
  if (!(await ftPassChecks(from, myKey, offer.pass))) {
    console.warn('A file offer whose pass does not check was dropped.');
    return;
  }
  const t = ftNewTransfer('receive', from, offer.id);
  t.offer = offer;
  t.offerTs = Number(inner.ts) || Date.now();
  t.name = ftSafeName(offer.name);
  t.size = offer.size;
  t.type = offer.type;
  // Files in messages first (part 3): a sender this device does not accept files from is never asked about.
  const settings = typeof privateFilesSettings === 'function' ? privateFilesSettings() : null;
  const who = typeof privateFileSender === 'function' ? privateFileSender(from) : { isMe: false, isFriend: false };
  const Fm = window.HosFilesInMessages;
  const accepted = Fm ? Fm.acceptsFrom(settings, who) : false;
  const hidden = typeof privateFileHidden === 'function' && privateFileHidden(from);
  if (!accepted || hidden) {
    t.state = 'refused';
    t.refusedAccept = settings ? settings.accept : 'friends';
  } else if (!F.filePassTimeOk(offer.pass, ftNowSecs())) {
    t.state = 'expired';
  } else {
    t.state = 'asked';
  }
  ftTransfers.set(t.id, t);
  await ftSave(t);
  if (t.state === 'asked') {
    const name = ftName(from);
    if (typeof upsertDmConversation === 'function') upsertDmConversation(from, name, 'Wants to send you a file', t.offerTs, true);
    if (typeof notifyNewMessage === 'function') notifyNewMessage(name, `Wants to send you ${t.name} (${F.sizeWords(t.size)})`, true);
  }
  ftDraw(t);
}

/**
 * Accept (the question's button). One click accepts, and the row moves on at once.
 *
 * Where to save is asked in the same click, since a browser opens its save dialog only straight
 * after one, but the answer only decides where the finished file goes: the place chosen, or (the
 * dialog closed, or a browser without one) the browser's own download. Until 2026-10-12 Accept
 * waited for the dialog first and did nothing when it was closed. A browser that cannot show the
 * dialog (a headless one) closes it at once, measured 2026-10-12 in Edge: "AbortError ... The user
 * aborted a request." So a real click there left the question on the row with nothing sent, while
 * a script's click, which a browser lets open no dialog ("NotAllowedError ... User activation
 * required."), went on to the transfer: the review's real-browser run saw exactly that.
 */
function ftAccept(t) {
  const F = ftApi();
  if (!t || t.role !== 'receive' || t.state !== 'asked') return Promise.resolve();
  if (!F.filePassTimeOk(t.offer.pass, ftNowSecs())) return ftEnd(t, 'expired');
  let picking = null;
  if (typeof window.showSaveFilePicker === 'function') {
    try { picking = Promise.resolve(window.showSaveFilePicker({ suggestedName: t.name })); } catch (e) { picking = null; }
  }
  t.state = 'accepted';
  t.reason = null;
  t.saveHandle = null;
  if (picking) {
    t.savePicking = picking.then((h) => { t.saveHandle = h || null; }, () => { t.saveHandle = null; })
      .then(() => { t.savePicking = null; ftDraw(t); });
  }
  ftDraw(t);
  return (async () => {
    await ftSave(t);
    await ftNote(t, F.answerNoteText(t.id, true), { selfCopy: true, filePass: t.offer.pass });
    ftDial(t);
  })();
}

/** Decline (the question's button): the row says so at once. */
async function ftDecline(t) {
  const F = ftApi();
  if (!t || t.role !== 'receive' || t.state !== 'asked') return;
  t.state = 'declined';
  ftDraw(t);
  await ftSave(t);
  await ftNote(t, F.answerNoteText(t.id, false), { selfCopy: true, filePass: t.offer.pass });
}

/** Cancel, from either side: the channel hears it if it is open, and the note goes either way. */
async function ftCancel(t) {
  if (!t || ftFinished(t)) return;
  const chan = ftChanFor(t);
  if (chan && chan.engine && chan.open) chan.engine.cancel();
  else if (t.receiver && t.receiver.io) t.receiver.cancel();
  const offered = t.offer && t.state !== 'preparing';
  await ftEnd(t, 'cancelled');
  if (offered) await ftCancelNote(t);
}

/** The cancel note to the other side (and my other devices). The receiver's carries the pass, as its answer does (rule 4); the sender's needs none. */
async function ftCancelNote(t) {
  const F = ftApi();
  await ftNote(t, F.cancelNoteText(t.id), { selfCopy: true, filePass: t.role === 'receive' ? t.offer.pass : null });
}

/**
 * A transfer that cannot go on ends here, and the other side is told (rule 8 of the second review,
 * "An end is always said"): its engine has said {"t":"cancel"} on the channel when there was one,
 * and the cancel note goes too, so the other side's row ends even with the connection gone. The
 * row here says why (`reason`). Until 2026-10-12 a damaged, mismatched or unstorable file ended
 * here without a word to anyone, and a desktop sender waited for it until the pass ran out, up to
 * 7 days.
 */
async function ftFail(t, reason) {
  if (!t || ftFinished(t)) return;
  await ftEnd(t, 'failed', reason);
  if (t.offer) await ftCancelNote(t);
}

function ftPause(t) {
  if (!t || ftFinished(t) || t.pausedHere) return;
  t.pausedHere = true;
  const chan = ftChanFor(t);
  if (chan && chan.engine && chan.open) chan.engine.pause();
  else if (t.receiver) t.receiver.pause();
  ftDraw(t);
  ftSave(t);
}

function ftResume(t) {
  if (!t || ftFinished(t)) return;
  const was = t.pausedHere;
  t.pausedHere = false;
  t.noServer = false;
  const chan = ftChanFor(t);
  if (was && chan && chan.engine && chan.open) chan.engine.resume();
  else if (was && t.receiver && t.receiver.io) t.receiver.resume();
  else if (t.receiver) t.receiver.pausedHere = false;
  ftDraw(t);
  ftSave(t);
  if (t.role === 'receive') ftDial(t);
}

/**
 * End a transfer: its state and words (on the row at once), its channel (the others on the
 * connection go on), and (when it can never finish) its pieces.
 */
async function ftEnd(t, state, reason) {
  t.state = state;
  t.reason = reason || null;
  t.hashing = null;
  ftDropChannel(t);
  if (t.role === 'send') { t.file = null; t.pieceHashes = null; }
  ftDraw(t);
  if (t.role === 'receive' && state !== 'saved') {
    t.receiver = null;
    await ftDropPieces(t);
  }
  await ftSave(t);
  ftDraw(t);
}

/** Offers past their pass's end: the sender's "did not answer", the receiver's "ran out". */
function ftExpireOld() {
  const F = ftApi();
  const now = ftNowSecs();
  for (const t of ftTransfers.values()) {
    if (ftFinished(t) || !t.offer || F.filePassTimeOk(t.offer.pass, now)) continue;
    if (t.role === 'send' && t.state === 'offered') ftEnd(t, 'expired');
    else if (t.role === 'receive' && (t.state === 'asked' || t.state === 'accepted')) ftEnd(t, 'expired');
    else if (t.role === 'send' && t.state === 'accepted') ftEnd(t, 'expired', 'afterAccept');
  }
}
window.expireFileTransfers = ftExpireOld;

// ── The connection ──
//
// Rule 2 of "Device-to-device big files: rules settled by the first review" (2026-10-12): one
// connection with each person, several transfers on it, one data channel and one engine each.
// Until then this page kept one engine per connection: a second transfer between the same two
// waited for the first, and on the answering side every channel was handed to the first
// transfer's Sender (the review found it at the seams with the desktop app, which already kept
// several).
//
// A link: {peer, role ('dial' or 'answer'), pc, chans (transfer id -> its channel), toOpen (ids of
// my receiving transfers whose channels open once the connection is up), pendingIce, remoteSet,
// open (a channel has opened), closed, timer, path}. A channel: {id, t, dc, engine, open, closed}.

function ftNewLink(peer, role) {
  return { peer, role, pc: null, chans: new Map(), toOpen: [], pendingIce: [], remoteSet: false, open: false, closed: false, timer: 0, path: null };
}

/** The connection a transfer rides, or waits to ride, or null. */
function ftLinkFor(t) {
  const link = t ? ftLinks.get(ftLower(t.peer)) : null;
  if (!link || link.closed) return null;
  return (link.chans.has(t.id) || link.toOpen.includes(t.id)) ? link : null;
}

/** A transfer's channel, or null. */
function ftChanFor(t) {
  const link = ftLinkFor(t);
  return link ? (link.chans.get(t.id) || null) : null;
}

function ftSignal(peer, type, data, extra) {
  if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify(Object.assign({ type: 'webrtc_signal', to: peer, signal_type: type, data }, extra || {})));
}

/**
 * The peer-connection settings for a connection with `peer` set up for transfer `t`: the server's
 * call relay only, with ONE allocation, that transfer's credentials (rule 7: the forwarder passes
 * packets only between the two people's allocations for one transfer, so both ends ask for the
 * same one, the dialler for the transfer it dials for and the answerer for the dial's `file`;
 * every other transfer rides the connection as its own channel). Or, when direct connections are
 * allowed for us, that and this server's own address lookup. Null when neither is possible: the
 * server cannot carry it, and it may not go direct.
 */
async function ftRtcConfig(t, peer) {
  const direct = ftDirectAllowed(peer);
  const servers = [];
  if (typeof requestCallCredentials === 'function') {
    const creds = await requestCallCredentials({ file: t.id, peer, file_pass: t.offer.pass });
    if (creds) servers.push({ urls: creds.urls.slice(), username: creds.username, credential: creds.credential });
  }
  if (!direct) {
    if (!servers.length) return null;
    return { config: { iceServers: servers, iceTransportPolicy: 'relay' }, path: 'relay' };
  }
  let stun = [];
  if (typeof ownDeviceRtcConfig === 'function') {
    try { stun = (await ownDeviceRtcConfig()).iceServers || []; } catch (e) { stun = []; }
  }
  return { config: { iceServers: stun.concat(servers), iceTransportPolicy: 'all' }, path: 'direct' };
}

/** May this receiving transfer open its channel now? */
function ftWantsChannel(t) {
  return !!t && t.role === 'receive' && t.state === 'accepted' && !t.pausedHere;
}

/**
 * The receiver dials the sender for transfer `t` (Accept, Resume, the sender coming online, a
 * retry). With a connection to them already there (either side's dial), the transfer rides it:
 * its channel opens on it, now or as soon as it is up.
 */
async function ftDial(t) {
  const F = ftApi();
  if (!ftWantsChannel(t)) return;
  if (!F.filePassTimeOk(t.offer.pass, ftNowSecs())) { await ftEnd(t, 'expired'); return; }
  const key = ftLower(t.peer);
  const have = ftLinks.get(key);
  if (have && !have.closed) { ftJoin(have, t); return; }
  if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) return;
  const link = ftNewLink(t.peer, 'dial');
  link.toOpen.push(t.id);
  ftLinks.set(key, link);
  ftDraw(t);
  const cfg = await ftRtcConfig(t, t.peer);
  if (link.closed) return;
  if (!cfg) {
    // The server cannot carry it: every transfer waiting for this connection says so.
    const waiting = link.toOpen.map((id) => ftTransfers.get(id)).filter(Boolean);
    ftCloseLink(link);
    for (const w of waiting) { w.noServer = true; ftDraw(w); }
    return;
  }
  t.noServer = false;
  link.path = cfg.path;
  try {
    const pc = new RTCPeerConnection(cfg.config);
    link.pc = pc;
    // A channel the other side opens: one of my offers to them, which they are receiving.
    pc.ondatachannel = (ev) => ftChannelArrived(link, ev && ev.channel);
    // The channels of every transfer waiting for this connection, made before the offer so it
    // carries them (one more joining later opens on the live connection, no new offer needed).
    for (const id of link.toOpen.splice(0)) ftOpenChannel(link, id);
    if (!link.chans.size) { ftCloseLink(link); return; }
    pc.onicecandidate = (ev) => { if (ev && ev.candidate) ftSignal(t.peer, 'dc_ice', JSON.stringify(ev.candidate)); };
    pc.onconnectionstatechange = () => { if (pc.connectionState === 'failed' || pc.connectionState === 'closed') ftLinkGone(link); };
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    if (link.closed) return;
    // The pass rides on the offer: the relay lets it through to the sender because of it, and keeps it.
    ftSignal(t.peer, 'dc_offer', JSON.stringify(offer), { file_pass: t.offer.pass });
    link.timer = setTimeout(() => { if (!link.open) ftLinkGone(link); }, FT_CONNECT_WAIT_MS);
  } catch (e) {
    ftLinkGone(link);
  }
}

/** A receiving transfer of mine rides a connection already there: its channel opens now, or once it is up. */
function ftJoin(link, t) {
  if (link.closed || !ftWantsChannel(t) || link.chans.has(t.id) || link.toOpen.includes(t.id)) return;
  if (link.open && link.pc) ftOpenChannel(link, t.id);
  else link.toOpen.push(t.id);
  ftDraw(t);
}

/** Open a receiving transfer's channel on the connection (the receiver opens it, whichever side dialled). */
function ftOpenChannel(link, id) {
  const F = ftApi();
  const t = ftTransfers.get(id);
  if (!ftWantsChannel(t) || link.chans.has(id) || !link.pc) return;
  let dc;
  try { dc = link.pc.createDataChannel(F.channelLabel(id), { ordered: true }); } catch (e) { return; }
  ftBindChannel(link, t, dc);
}

/** The connection is up: the channels waiting for it open. */
function ftOpenWaiting(link) {
  for (const id of link.toOpen.splice(0)) ftOpenChannel(link, id);
}

/**
 * A channel the other side opened: for one of my live offers to them (their receiving end), served
 * by a Sender of its own. Anything else is closed.
 */
function ftChannelArrived(link, ch) {
  const F = ftApi();
  const id = ch ? F.channelLabelId(ch.label) : null;
  const t = id ? ftTransfers.get(id) : null;
  const ok = t && t.role === 'send' && ftSameKey(t.peer, link.peer) && !ftFinished(t) && t.state !== 'preparing'
    && t.file && t.pieceHashes && !link.chans.has(id) && !link.closed;
  if (!ok) {
    // A channel for an offer of mine that ended here (cancelled, failed): an end is always said
    // (rule 8). Cancel goes on the channel before it closes, and the cancel note once more: the
    // receiver never heard the first, or it would not be opening this (the review of 2026-10-12).
    if (ch && ftEndedOfferTo(t, link.peer)) {
      ftSayCancelAndClose(ch);
      ftCancelAgainOnce(t);
      return;
    }
    try { if (ch) ch.close(); } catch (e) { /* gone */ }
    return;
  }
  ftBindChannel(link, t, ch);
}

/** Is `t` an offer of mine to `peer` that ended here without being sent (cancelled or failed)? */
function ftEndedOfferTo(t, peer) {
  return !!t && t.role === 'send' && !!t.offer && ftSameKey(t.peer, peer) && ['cancelled', 'failed'].includes(t.state);
}

/** The cancel note for an ended offer of mine, said again once in this page's life. */
function ftCancelAgainOnce(t) {
  if (t.cancelSaidAgain) return;
  t.cancelSaidAgain = true;
  ftCancelNote(t);
}

/** Say {"t":"cancel"} on a channel (once it is open), then close it: a browser sends what is queued first. */
function ftSayCancelAndClose(ch) {
  const go = () => {
    try { ch.send(JSON.stringify({ t: 'cancel' })); } catch (e) { /* gone */ }
    try { ch.close(); } catch (e) { /* gone */ }
  };
  if (ch.readyState === 'open') go();
  else ch.onopen = go;
}

/** Is `t` a live offer of mine to `peer`: offered, accepted or sending (a dial may be for it)? */
function ftLiveOfferTo(t, peer) {
  return !!t && t.role === 'send' && ftSameKey(t.peer, peer) && !!t.offer && ['offered', 'accepted', 'sending'].includes(t.state);
}

/** The sender's live offers to `peer`. */
function ftLiveOffersTo(peer) {
  const out = [];
  for (const t of ftTransfers.values()) if (ftLiveOfferTo(t, peer)) out.push(t);
  return out.sort((a, b) => (a.offerTs || 0) - (b.offerTs || 0));
}

/**
 * The sender answers a dial for `named`, the live offer the dial's `file` names (rule 7): one
 * allocation, that transfer's credentials, the one the dialler asked for too. Every other transfer
 * with them rides the same connection as its own channel, which each channel's label names. It is
 * answered while something can go on it: an offer to them whose file this page holds, or a
 * transfer of theirs to me waiting to open its channel.
 */
async function ftAnswerDial(msg, named, link) {
  const going = ftLiveOffersTo(msg.from).filter((t) => t.file && t.pieceHashes);
  if (!going.length && !ftWaitingFrom(msg.from).length) { ftCloseLink(link); return; }
  const cfg = await ftRtcConfig(named, msg.from);
  if (link.closed) return;
  if (!cfg) {
    for (const t of going) { t.noServer = true; ftDraw(t); }
    ftCloseLink(link);
    return;
  }
  for (const t of going) t.noServer = false;
  link.path = cfg.path;
  try {
    const pc = new RTCPeerConnection(cfg.config);
    link.pc = pc;
    pc.ondatachannel = (ev) => ftChannelArrived(link, ev && ev.channel);
    pc.onicecandidate = (ev) => { if (ev && ev.candidate) ftSignal(msg.from, 'dc_ice', JSON.stringify(ev.candidate)); };
    pc.onconnectionstatechange = () => { if (pc.connectionState === 'failed' || pc.connectionState === 'closed') ftLinkGone(link); };
    await pc.setRemoteDescription(new RTCSessionDescription(JSON.parse(msg.data)));
    link.remoteSet = true;
    await ftApplyIce(link);
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);
    if (link.closed) return;
    ftSignal(msg.from, 'dc_answer', JSON.stringify(answer));
    link.timer = setTimeout(() => { if (!link.open) ftLinkGone(link); }, FT_CONNECT_WAIT_MS);
  } catch (e) {
    ftLinkGone(link);
  }
}

async function ftApplyIce(link) {
  const list = link.pendingIce.splice(0);
  for (const c of list) {
    try { await link.pc.addIceCandidate(new RTCIceCandidate(c)); } catch (e) { /* one bad candidate */ }
  }
}

/** My receiving transfers with `peer` waiting to ride a connection to them. */
function ftWaitingFrom(peer) {
  return Array.from(ftTransfers.values()).filter((t) => ftWantsChannel(t) && ftSameKey(t.peer, peer));
}

/**
 * A `webrtc_signal` from the relay: true when it belongs to a file transfer
 * (chat-voice-calls.js handleWebrtcSignalMessage asks here first). My own
 * devices' links stay chat-p2p.js's.
 */
function fileTransferSignal(msg) {
  if (!msg || typeof msg.from !== 'string' || !msg.from || ftSameKey(msg.from, typeof myKey === 'string' ? myKey : '')) return false;
  const key = ftLower(msg.from);
  const link = ftLinks.get(key);
  if (msg.signal_type === 'dc_offer') {
    // Rule 7: the relay names the transfer whose file pass let the dial in (`file`, set by the relay
    // alone). A dc_offer without it came in some other way (a friend's own-device link), so it is no
    // file dial: chat-p2p.js decides on it, and answers only my own devices.
    const F = ftApi();
    if (!F || !F.idOk(msg.file)) return false;
    const named = ftTransfers.get(msg.file);
    // A dial for a file of mine that is no longer live: not answered. When I cancelled it or it
    // failed here, they are dialling because my cancel note never reached them, and their other
    // files with me wait behind this dial: say it again, once in this page's life (the review of
    // 2026-10-12).
    if (!ftLiveOfferTo(named, msg.from)) {
      if (ftEndedOfferTo(named, msg.from)) ftCancelAgainOnce(named);
      return true;
    }
    if (link && link.role === 'dial' && !link.open) {
      // Both of us dialing at once (each has a file for the other): the larger key's dial goes on,
      // as the desktop app's offerer rule has it; the other side answers it, and its own receiving
      // transfers ride the connection it answers.
      if (String(myKey).toLowerCase() > key) return true;
      ftCloseLink(link);
    } else if (link) {
      ftLinkGone(link); // they dialed again: the old connection is gone on their side
    }
    const fresh = ftNewLink(msg.from, 'answer');
    ftLinks.set(key, fresh);
    // My own receiving transfers with them ride this connection too: I open their channels once it is up.
    for (const t of ftWaitingFrom(msg.from)) ftJoin(fresh, t);
    ftAnswerDial(msg, named, fresh);
    return true;
  }
  if (!link) return false;
  if (msg.signal_type === 'dc_answer' && link.role === 'dial' && link.pc) {
    (async () => {
      try {
        await link.pc.setRemoteDescription(new RTCSessionDescription(JSON.parse(msg.data)));
        link.remoteSet = true;
        await ftApplyIce(link);
      } catch (e) { ftLinkGone(link); }
    })();
    return true;
  }
  if (msg.signal_type === 'dc_ice') {
    let c = null;
    try { c = JSON.parse(msg.data); } catch (e) { return true; }
    if (link.pc && link.remoteSet) link.pc.addIceCandidate(new RTCIceCandidate(c)).catch(() => {});
    else link.pendingIce.push(c);
    return true;
  }
  return false;
}
window.fileTransferSignal = fileTransferSignal;

/** Wire a data channel to its transfer's own engine. */
function ftBindChannel(link, t, dc) {
  const F = ftApi();
  const chan = { id: t.id, t, dc, engine: null, open: false, closed: false };
  link.chans.set(t.id, chan);
  try { dc.binaryType = 'arraybuffer'; } catch (e) { /* read-only in some fakes */ }
  try { dc.bufferedAmountLowThreshold = 262144; } catch (e) { /* not every browser */ }
  const io = {
    send: (d) => dc.send(d),
    buffered: () => Number(dc.bufferedAmount) || 0,
    drained: () => new Promise((resolve) => {
      let done = false;
      const fin = () => { if (!done) { done = true; resolve(); } };
      dc.onbufferedamountlow = fin;
      setTimeout(fin, 500); // a browser that never says so still goes on
    }),
  };
  const opened = () => {
    if (chan.open || chan.closed || link.closed) return;
    chan.open = true;
    if (!link.open) {
      link.open = true;
      clearTimeout(link.timer);
      ftOpenWaiting(link);
    }
    t.path = link.path;
    t.noServer = false;
    if (t.role === 'send') {
      // Rule 9 (second review), a dial is an answer: a record still waiting for the answer note
      // (lost, or slower than the dial) takes the channel opening as Accept, and its done (the
      // Sender's 'end' below) as Sent, so a lost note never leaves the row waiting.
      if (t.state === 'offered') t.state = 'accepted';
      t.state = 'sending';
      chan.engine = new F.Sender({
        offer: t.offer, file: t.file, pieceHashes: t.pieceHashes, io, paused: t.pausedHere,
        onEvent: (type, d) => ftSenderEvent(t, chan, type, d),
      });
    } else {
      t.state = 'receiving';
      if (!t.receiver) {
        t.receiver = new F.Receiver({
          offer: t.offer, store: ftPieceStore(t), blake3: window.HosBlake3, stallMs: FT_STALL_MS, paused: t.pausedHere,
          onEvent: (type, d) => ftReceiverEvent(t, type, d),
        });
      }
      chan.engine = t.receiver;
      t.receiver.attach(io);
    }
    ftSave(t);
    ftDraw(t);
  };
  dc.onopen = opened;
  dc.onmessage = (ev) => { if (chan.engine && !chan.closed) chan.engine.receive(ev.data); };
  dc.onclose = () => ftChanGone(link, chan);
  if (dc.readyState === 'open') opened();
}

function ftSenderEvent(t, chan, type, d) {
  if (chan.closed) return;
  if (type === 'progress') {
    t.done = Math.max(t.done || 0, d.piece + 1);
    ftDrawSoon(t);
  } else if (type === 'paused-there' || type === 'resumed-there') {
    t.pausedThere = type === 'paused-there';
    ftDraw(t);
  } else if (type === 'end') {
    if (d.reason === 'done') {
      t.done = t.offer.pieces;
      ftEnd(t, 'sent');
    } else if (d.reason === 'cancelled-there') {
      ftEnd(t, 'cancelled');
    } else if (d.reason === 'unreadable') {
      t.file = null;
      t.pieceHashes = null;
      t.reason = 'unreadable';
      ftDropChannel(t);
      t.state = 'accepted';
      ftSave(t);
      ftDraw(t);
    }
  }
}

function ftReceiverEvent(t, type, d) {
  if (type === 'progress') {
    t.done = d.done;
    t.kept = d.low;
    ftDrawSoon(t);
  } else if (type === 'paused-there' || type === 'resumed-there') {
    t.pausedThere = type === 'paused-there';
    ftDraw(t);
  } else if (type === 'checking') {
    t.state = 'checking';
    ftDraw(t);
  } else if (type === 'complete') {
    ftSaveFile(t);
  } else if (type === 'end') {
    // A cancel pressed here is ended by ftCancel itself; the engine's 'cancelled-here' only repeats it.
    if (d.reason === 'cancelled-here') return;
    if (d.reason === 'cancelled-there') ftEnd(t, 'cancelled');
    // Rule 8: the Receiver has said cancel on the channel; ftFail sends the cancel note and the row
    // says why.
    else ftFail(t, { mismatch: 'mismatch', 'mismatch-file': 'mismatch', damaged: 'damaged', storage: 'storage' }[d.reason] || 'failed');
  }
}

/** Stop a channel's engine without a word (the Receiver stays with its transfer, for the next channel). */
function ftStopEngine(chan) {
  if (!chan.engine) return;
  if (typeof chan.engine.detach === 'function') chan.engine.detach();
  else if (typeof chan.engine.close === 'function') chan.engine.close();
}

/**
 * Close the connection when its last channel has ended (and nothing waits to open on it, no channel
 * closed here is still closing, and no channel the other side closed is still being judged, see
 * ftSendChannelGone).
 */
function ftCloseIfIdle(link) {
  if (!link.closed && link.chans.size === 0 && link.toOpen.length === 0 && !link.closingHere && !link.judging) ftCloseLink(link);
}

/**
 * A transfer ended here: its channel closes, and the others on the connection go on. When it was
 * the last, the connection closes once the channel has (its close event, ftChanGone): a browser
 * sends what was queued on a channel before closing it, but closing the whole connection drops
 * that at once, and the last thing said on an ending transfer's channel is its cancel (rule 8).
 */
function ftDropChannel(t) {
  const link = t ? ftLinks.get(ftLower(t.peer)) : null;
  if (!link || link.closed) return;
  const q = link.toOpen.indexOf(t.id);
  if (q >= 0) link.toOpen.splice(q, 1);
  const chan = link.chans.get(t.id);
  if (chan) {
    chan.closed = true;
    link.chans.delete(t.id);
    ftStopEngine(chan);
    if (chan.dc.readyState !== 'closed') {
      chan.closingHere = true;
      link.closingHere = (link.closingHere || 0) + 1;
    }
    try { chan.dc.close(); } catch (e) { ftClosedHere(link, chan); }
  }
  ftCloseIfIdle(link);
}

/** A channel closed here has finished closing. */
function ftClosedHere(link, chan) {
  if (!chan.closingHere) return;
  chan.closingHere = false;
  link.closingHere -= 1;
}

/**
 * One channel closed (the other side closed it, or it failed): that transfer is off the connection
 * and the others go on. A receiving transfer that did not end opens its channel again (or dials
 * again) at the next retry, and a pause the other side held is cleared (rule 8; its Receiver clears
 * its own on the next channel). A sending transfer is judged by ftSendChannelGone.
 */
function ftChanGone(link, chan) {
  if (chan.closed) {
    // One this side closed has finished closing (ftDropChannel): the connection may close now.
    if (chan.closingHere) { ftClosedHere(link, chan); ftCloseIfIdle(link); }
    return;
  }
  chan.closed = true;
  if (link.chans.get(chan.id) === chan) link.chans.delete(chan.id);
  ftStopEngine(chan);
  const t = chan.t;
  if (!ftFinished(t) && t.state !== 'checking' && !(t.receiver && t.receiver.completed)) {
    if (t.state === 'sending' || t.state === 'receiving') t.state = 'accepted';
    t.pausedThere = false;
    ftSave(t);
    ftDraw(t);
    if (t.role === 'send') ftSendChannelGone(link, t);
    else ftScheduleRetry();
  }
  ftCloseIfIdle(link);
}

/** Is this connection still up: not closed here, and the browser says it is connected? */
function ftLinkUp(link) {
  return !!link && !link.closed && !!link.pc && link.pc.connectionState === 'connected';
}

/**
 * A sending transfer's channel closed by the other side before done (rule 8 of the second review):
 * their side stopped it, and it ends here with words ("Ben's side stopped it") and the cancel note,
 * rather than waiting for the pass to run out. A DROP is not a stop, though: a receiver whose page
 * reloads, or whose network goes, comes back and dials again with the same pass, and the spec's
 * resume needs this side to still answer. The two look alike at the moment a channel closes (a
 * closing page's channels close one event at a time, while its connection still reads connected
 * here), so the close is judged after FT_STOPPED_WAIT_MS:
 *  - the connection gone by then (a new dial from them replaced it, or the browser no longer says
 *    connected: a page that closed or reloaded stops answering ICE's checks, which the browser is
 *    expected to notice within the wait; not yet measured in a real browser, media-showcase.md):
 *    a drop, and the transfer waits for their next dial, as before;
 *  - still connected, and the transfer not back on a channel since: a stop, and it ends.
 * A connection already gone when the channel closes is a drop at once. The connection is kept open
 * while a close is judged (ftCloseIfIdle waits for it).
 */
function ftSendChannelGone(link, t) {
  if (!ftLinkUp(link)) return; // a drop: it waits for their next dial
  link.judging = (link.judging || 0) + 1;
  setTimeout(() => {
    link.judging -= 1;
    if (ftLinkUp(link) && t.state === 'accepted' && !ftChanFor(t)) ftFail(t, 'stoppedThere');
    ftCloseIfIdle(link);
  }, FT_STOPPED_WAIT_MS);
}

/** The connection went (closed, failed, or never opened in time): every transfer on it waits again. */
function ftLinkGone(link) {
  if (link.closed) return;
  const riding = Array.from(link.chans.values()).map((c) => c.t)
    .concat(link.toOpen.map((id) => ftTransfers.get(id)).filter(Boolean));
  ftCloseLink(link);
  for (const t of riding) {
    if (ftFinished(t)) continue;
    // Every piece in and being checked (or saved): that needs no connection, and goes on.
    if (t.state === 'checking' || (t.receiver && t.receiver.completed)) continue;
    if (t.state === 'sending' || t.state === 'receiving') t.state = 'accepted';
    t.pausedThere = false;
    ftSave(t);
    ftDraw(t);
  }
  ftScheduleRetry();
}

function ftCloseLink(link) {
  if (link.closed) return;
  link.closed = true;
  clearTimeout(link.timer);
  const key = ftLower(link.peer);
  if (ftLinks.get(key) === link) ftLinks.delete(key);
  for (const chan of link.chans.values()) {
    chan.closed = true;
    ftStopEngine(chan);
    try { chan.dc.close(); } catch (e) { /* gone */ }
  }
  link.chans.clear();
  link.toOpen.length = 0;
  try { if (link.pc) link.pc.close(); } catch (e) { /* gone */ }
}

/** Every receiving transfer waiting for its sender dials again (when the sender may be there). */
function ftRetryDials() {
  ftRetryTimer = 0;
  for (const t of ftTransfers.values()) {
    if (t.role === 'receive' && t.state === 'accepted' && !t.pausedHere) ftDial(t);
  }
}

function ftScheduleRetry() {
  if (ftRetryTimer) return;
  ftRetryTimer = setTimeout(ftRetryDials, FT_DIAL_RETRY_MS) || 0;
}

// ── Saving what was received ──

/** Every piece is in and the whole file's BLAKE3 matched: save it, then let the pieces go. */
async function ftSaveFile(t) {
  const n = t.offer.pieces;
  const piece = async (i) => ftIdb('pieces', 'readonly', (os) => os.get(ftPieceKey(t.id, i)));
  if (t.savePicking) {
    // The save dialog Accept opened is still open: the file waits for the place (the row says so).
    t.awaitingPlace = true;
    ftDraw(t);
    await t.savePicking;
    t.awaitingPlace = false;
  }
  try {
    if (t.saveHandle && typeof t.saveHandle.createWritable === 'function') {
      // Straight into the file the person chose, a piece at a time.
      const w = await t.saveHandle.createWritable();
      for (let i = 0; i < n; i++) {
        const b = await piece(i);
        if (!b) throw new Error('a piece is missing');
        await w.write(b);
      }
      await w.close();
    } else {
      // A Blob made of the kept pieces (each one a Blob held by the browser's storage, not in this
      // page's memory), saved through the browser's own download.
      const parts = [];
      for (let i = 0; i < n; i++) {
        const b = await piece(i);
        if (!b) throw new Error('a piece is missing');
        parts.push(b);
      }
      const blob = new Blob(parts, { type: t.offer.type || 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = t.name;
      a.rel = 'noopener';
      if (document.body && typeof document.body.appendChild === 'function') document.body.appendChild(a);
      if (typeof a.click === 'function') a.click();
      if (typeof a.remove === 'function') a.remove();
      // Kept for a "Save again" link (a browser that held the first download back), until the
      // pieces go.
      t.savedUrl = url;
      setTimeout(() => {
        try { URL.revokeObjectURL(url); } catch (e) { /* gone */ }
        t.savedUrl = null;
        ftDraw(t);
      }, FT_PIECES_LINGER_MS);
    }
  } catch (e) {
    addSystemMessage(`${t.name} arrived whole, but this browser could not save it: ${(e && e.message) || 'unknown'}.`);
    t.reason = 'notSaved';
    ftDraw(t);
    return;
  }
  t.receiver = null;
  t.done = n;
  t.state = 'saved';
  await ftSave(t);
  ftDraw(t);
  // Its channel closes a moment later ("done" has gone by then); the others on the connection go on.
  setTimeout(() => ftDropChannel(t), 2000);
  setTimeout(() => ftDropPieces(t), FT_PIECES_LINGER_MS);
}

// ── The rows ──

/** The words for a transfer's row as it stands. */
function ftStateWords(t) {
  const W = ftWords();
  const name = ftName(t.peer);
  const online = ftOnline.has(ftLower(t.peer));
  if (t.reason === 'wrongFile') return W.wrongFile;
  if (t.reason === 'checkingAgain') return W.checking;
  if (t.reason === 'notSaved') return W.notSaved;
  if (t.role === 'send') {
    switch (t.state) {
      case 'preparing': return W.preparing;
      case 'offered': return t.file ? W.waitingAccept(name) : W.noFile;
      case 'accepted':
        if (!t.file) return t.reason === 'unreadable' ? W.unreadable : W.noFile;
        if (t.noServer) return W.notSetUp;
        if (t.pausedHere) return W.paused;
        return online ? W.connecting : W.waitingOnline(name);
      case 'sending':
        if (t.pausedHere) return W.paused;
        if (t.pausedThere) return W.pausedThere(name);
        return t.path === 'direct' ? W.direct : W.throughRelay;
      case 'sent': return W.sent;
      case 'declined': return W.declined(name);
      case 'cancelled': return W.cancelled;
      case 'expired': return t.reason === 'afterAccept' ? W.ranOutSender : W.noAnswer(name);
      case 'undelivered':
        return t.reason === 'reach' ? W.notDeliveredReach(name) : (t.reason === 'rate' ? W.notDeliveredRate : W.notDelivered);
      default:
        if (t.reason === 'noFile') return W.noFile;
        if (t.reason === 'notSigned') return W.notSigned;
        if (t.reason === 'stoppedThere') return W.stoppedThere(name);
        return W.failed;
    }
  }
  switch (t.state) {
    case 'asked': return W.question(name, t.name, t.size);
    case 'accepted':
      if (t.noServer) return W.notSetUp;
      if (t.pausedHere) return W.paused;
      return online ? W.connecting : W.waitingOnline(name);
    case 'receiving':
      if (t.pausedHere) return W.paused;
      if (t.pausedThere) return W.pausedThere(name);
      return t.path === 'direct' ? W.receivingDirect : W.receivingThroughRelay;
    case 'checking': return t.awaitingPlace ? W.choosePlace : W.checking;
    case 'saved': return W.saved;
    case 'declined': return W.youDeclined;
    case 'refused': return W.refused(name, t.name, t.size, t.refusedAccept);
    case 'elsewhere': return W.answeredElsewhere;
    case 'cancelled': return W.cancelled;
    case 'expired': return W.ranOut;
    default:
      if (t.reason === 'mismatch') return W.mismatch(name);
      if (t.reason === 'damaged') return W.damaged;
      if (t.reason === 'storage') return W.storage;
      return W.failed;
  }
}

/** How far along, 0 to 1. */
function ftFraction(t) {
  if (t.hashing && t.hashing.total) return t.hashing.done / t.hashing.total;
  const total = (t.offer && t.offer.pieces) || 0;
  if (!total) return 0;
  if (t.state === 'sent' || t.state === 'saved') return 1;
  return Math.max(0, Math.min(1, (t.done || 0) / total));
}

/** Draw (or redraw) a transfer's row, when its conversation is on screen. */
function ftDraw(t) {
  if (!t) return;
  const open = typeof activeDmPartner !== 'undefined' && activeDmPartner && ftSameKey(activeDmPartner, t.peer);
  let el = ftRows.get(t.id);
  if (!open) return;
  if (!el) {
    el = document.createElement('div');
    el.className = 'message ft-row';
    el.dataset.fileTransfer = t.id;
    ftRows.set(t.id, el);
    ftFill(el, t);
    if (typeof appendMessage === 'function') appendMessage(el);
    return;
  }
  ftFill(el, t);
}

const ftDrawPending = new Set();
/** Redraw soon (progress comes many times a second; one redraw a frame is plenty). */
function ftDrawSoon(t) {
  if (ftDrawPending.has(t.id)) return;
  ftDrawPending.add(t.id);
  const run = () => { ftDrawPending.delete(t.id); ftDraw(t); };
  if (typeof requestAnimationFrame === 'function' && requestAnimationFrame(run)) return;
  Promise.resolve().then(run);
}

/** A row element -> its parts (built once, then changed in place; see ftFill). */
const ftRowParts = new WeakMap();

/** The buttons a row shows as the transfer stands, by their data-ft names. */
function ftButtonsFor(t) {
  const show = new Set();
  const finished = ftFinished(t);
  if (t.role === 'receive' && t.state === 'asked') {
    show.add('accept');
    show.add('decline');
  } else if (!finished) {
    const canPause = t.state === 'accepted' || t.state === 'sending' || t.state === 'receiving';
    if (canPause && !t.pausedHere && !t.noServer) show.add('pause');
    if (canPause && (t.pausedHere || t.noServer)) show.add('resume');
    if (t.role === 'send' && !t.file && t.state !== 'preparing') show.add('choose');
    // A file that arrived whole but could not be saved: try again, or give it up (it used to sit
    // at "Checking the file" with no buttons at all).
    if (t.role === 'receive' && t.reason === 'notSaved') show.add('try-save');
    if (t.state !== 'checking' || t.reason === 'notSaved') show.add('cancel');
  }
  // Saved through the browser's download: the same file once more, while the browser still holds it.
  if (t.role === 'receive' && t.state === 'saved' && t.savedUrl) show.add('save-again');
  return show;
}

/**
 * Fill a row's contents (createElement and textContent only).
 *
 * The row is built once and then only changed in place: its words, its bar, which buttons show.
 * Until 2026-10-12 every redraw emptied the row and made new buttons, and a redraw comes with every
 * piece (one a frame). A browser sends a click only to the element the button went down AND came
 * up on, so a button replaced between the two lost the click: Pause and Cancel could hardly be
 * pressed while a file was moving. Each button is now the same element for the row's whole life,
 * hidden when it does not apply, so a redraw never takes one from under the pointer.
 */
function ftFill(el, t) {
  const F = ftApi();
  const W = ftWords();
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  };
  const show = (e, on) => { e.style.display = on ? '' : 'none'; };
  let p = ftRowParts.get(el);
  if (!p) {
    p = { buttons: new Map() };
    el.innerHTML = ''; // emptied once (no markup is ever written here: everything below is textContent)
    const card = mk('div', 'enc-attach ft-card');
    const head = mk('div', 'enc-attach-head');
    p.name = head.appendChild(mk('span', 'ft-name', ''));
    p.size = head.appendChild(mk('span', 'enc-attach-size ft-size', ''));
    card.appendChild(head);
    const body = mk('div', 'enc-attach-body');
    p.bar = body.appendChild(mk('div', 'ft-bar'));
    p.fill = p.bar.appendChild(mk('div', 'ft-bar-fill'));
    p.words = body.appendChild(mk('div', 'ft-state', ''));
    p.words.setAttribute('role', 'status');
    p.note = body.appendChild(mk('div', 'enc-attach-note ft-note', W.sendKeepOpen));
    p.row = body.appendChild(mk('div', 'ft-buttons'));
    const name = ftName(t.peer);
    const button = (label, what, tip, fn) => {
      const b = mk('button', 'enc-attach-dl ft-btn', label);
      b.type = 'button';
      b.dataset.ft = what;
      b.setAttribute('data-tip-desc', tip);
      b.onclick = () => { fn(); };
      p.row.appendChild(b);
      p.buttons.set(what, b);
      return b;
    };
    // Every button this row can ever show, in their order, made now; which show is decided below.
    if (t.role === 'receive') {
      button(W.accept, 'accept', W.tipAccept(name), () => ftAccept(t));
      button(W.decline, 'decline', W.tipDecline(name), () => ftDecline(t));
    }
    button(W.pause, 'pause', W.tipPause, () => ftPause(t));
    button(W.resume, 'resume', W.tipResume, () => ftResume(t));
    if (t.role === 'send') {
      const pick = document.createElement('input');
      pick.type = 'file';
      pick.style.display = 'none';
      pick.onchange = () => { const f = pick.files && pick.files[0]; if (f) ftChooseAgain(t, f); };
      p.row.appendChild(pick);
      t.choosePicker = pick;
      button(W.chooseAgain, 'choose', W.tipChoose, () => { if (typeof pick.click === 'function') pick.click(); });
    }
    if (t.role === 'receive') {
      button(W.trySaveAgain, 'try-save', W.tipTrySaveAgain, () => {
        // Again through the browser's own download (a chosen place may be what failed).
        t.reason = null;
        t.saveHandle = null;
        ftDraw(t);
        ftSaveFile(t);
      });
    }
    button(W.cancel, 'cancel', W.tipCancel(name), () => ftCancel(t));
    if (t.role === 'receive') {
      const again = mk('a', 'enc-attach-dl ft-btn', W.saveAgain);
      again.dataset.ft = 'save-again';
      again.setAttribute('data-tip-desc', W.tipSaveAgain);
      p.row.appendChild(again);
      p.buttons.set('save-again', again);
    }
    card.appendChild(body);
    el.appendChild(card);
    ftRowParts.set(el, p);
  }
  el.dataset.state = t.state;
  p.name.textContent = t.name;
  p.size.textContent = F.sizeWords(t.size);
  const finished = ftFinished(t);
  show(p.bar, !finished && t.state !== 'asked');
  p.fill.style.width = Math.round(ftFraction(t) * 100) + '%';
  p.words.textContent = ftStateWords(t);
  show(p.note, t.role === 'send' && !finished && t.state !== 'preparing' && !!t.file);
  const wanted = ftButtonsFor(t);
  for (const [what, b] of p.buttons) show(b, wanted.has(what));
  const again = p.buttons.get('save-again');
  if (again && wanted.has('save-again')) {
    again.href = t.savedUrl;
    again.download = t.name;
  }
  show(p.row, wanted.size > 0);
}

/** The open conversation's rows, drawn after its messages (chat-dms.js renderDmConversationFromStore). */
function renderFileTransferRows(partnerKey) {
  for (const id of Array.from(ftRows.keys())) ftRows.delete(id);
  if (!ftLoadPromise) {
    const p = ftEnsureLoaded();
    p.then((loaded) => {
      if (loaded && typeof activeDmPartner !== 'undefined' && ftSameKey(activeDmPartner, partnerKey)) renderFileTransferRows(partnerKey);
    });
    if (!ftLoadPromise) return;
  }
  const mine = Array.from(ftTransfers.values()).filter((t) => ftSameKey(t.peer, partnerKey));
  mine.sort((a, b) => (a.offerTs || a.created || 0) - (b.offerTs || b.created || 0));
  for (const t of mine) ftDraw(t);
}
window.renderFileTransferRows = renderFileTransferRows;

// ── The frames this file listens to ──

/** The transfers, for the tests and for debugging from the console. */
function fileTransfers() { return Array.from(ftTransfers.values()); }
window.fileTransfers = fileTransfers;
window.fileTransferActions = { accept: ftAccept, decline: ftDecline, cancel: ftCancel, pause: ftPause, resume: ftResume, chooseAgain: ftChooseAgain, dial: ftDial };

const _origHandleMessageFT = handleMessage;
handleMessage = function (msg) {
  if (msg && (msg.type === 'dm_put_ok' || msg.type === 'dm_put_refused') && ftPutAnswered(msg)) return undefined;
  const out = _origHandleMessageFT(msg);
  if (msg && msg.type === 'full_user_list' && Array.isArray(msg.users)) {
    ftOnline.clear();
    for (const u of msg.users) if (u && u.online && u.public_key) ftOnline.add(ftLower(u.public_key));
    ftAfterPresence();
  } else if (msg && msg.type === 'peer_joined' && msg.public_key) {
    ftOnline.add(ftLower(msg.public_key));
    ftAfterPresence();
  } else if (msg && msg.type === 'peer_left' && msg.public_key) {
    ftOnline.delete(ftLower(msg.public_key));
    for (const t of ftTransfers.values()) if (ftSameKey(t.peer, msg.public_key)) ftDraw(t);
  }
  return out;
};

/** Someone came online (or the member list arrived after a connect): send what waited, dial whom I wait for. */
function ftAfterPresence() {
  if (!ftStore()) return;
  ftEnsureLoaded().then(() => {
    ftExpireOld();
    ftFlushNotes();
    for (const t of ftTransfers.values()) {
      if (t.role === 'receive' && t.state === 'accepted' && !t.pausedHere && ftOnline.has(ftLower(t.peer))) ftDial(t);
      if (!ftFinished(t)) ftDraw(t);
    }
  });
}
