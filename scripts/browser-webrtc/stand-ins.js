// The parts of the chat page (web/chat/app.js, crypto.js, chat-messages.js, chat-dms.js) that the
// website's call and file scripts lean on, stood in for `just verify-browser-webrtc`
// (src/gui/crossing_tests/webrtc_browser.rs, scripts/browser-webrtc.js). Loaded where index.html loads
// crypto.js and app.js, so chat-voice-rooms.js, chat-voice-calls.js and chat-file-transfer.js load
// over it exactly as they load over the real page.
//
// What is stood in, and why that is fair: this check is about WebRTC between a real browser and the
// desktop app (the DTLS handshake, the SRTP keys, the SCTP data channel, the forwarder), not about
// signing in or the sealed DMs, which have their own tests (scripts/tests/sealed-dm.test.js and the
// rest). So:
//  - the identity is a real Dilithium3 key (pq.js pqDeriveIdentity, harness.js), and the file
//    passes are really signed and really checked (pq.js pqSignMessage / pqVerifyMessage);
//  - a DM is not sealed: pqBuildDmPuts hands the note's text to the test as it is (the relay
//    stand-in passes it to the desktop side, which reads it with the app's own parse_note);
//  - a file is not cleaned of its metadata before it is offered (prepareAttachment), since the
//    check's files are made-up bytes with none;
//  - the person's "Files in messages" choice is "from anyone", so an offer is asked about;
//  - no rows are drawn (no conversation is open, so chat-file-transfer.js draws none).
// Anything else a script reaches for that is not here shows as an error in the page, which
// harness.js reports to the test, so a missing stand-in is never silent.

/* eslint-disable no-unused-vars */

// app.js's state the scripts read.
let ws = null;
let myKey = '';
let myName = 'Browser';
let myDilithiumPublicHex = '';
let myDilithiumSecret = null;
let activeDmPartner = null;
const peerData = {};

/** The lines the chat would have shown, for the report. */
const harnessSystemLines = [];

// app.js's message handler, which the voice and file scripts wrap. Whatever reaches the bottom of
// the chain is something this check did not expect; it is kept for the report.
const harnessUnhandled = [];
function handleMessage(msg) {
  harnessUnhandled.push(msg && msg.type);
}

function updateUserList() {}
function openSocket() {}
function addSystemMessage(text) {
  harnessSystemLines.push(String(text));
  console.log('[chat line] ' + String(text));
}
function esc(s) { return String(s == null ? '' : s); }
function hosIcon() { return ''; }
function shortKey(k) { return String(k || '').slice(0, 8); }
function resolveSenderName(name, key) { return name || shortKey(key); }

// crypto.js: the sealed DM layer, not sealed here (see above).
async function pqBuildDmPuts(text, peer, ts) {
  return { recipientPut: { type: 'dm_put', to: peer, ts, harness_note: String(text) }, selfPut: null };
}

// chat-messages.js: the attach step's cleaning and the conversation on screen.
async function prepareAttachment(file) { return { file }; }
function privateConversationNow() { return 'dm:' + (window.harnessPeerKey || ''); }

// chat-dms.js: the person's "Files in messages" choice (Safety): from anyone.
function privateFilesSettings() { return { accept: 'anyone' }; }
