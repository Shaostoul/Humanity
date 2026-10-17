// ── chat-dms.js ───────────────────────────────────────────────────────────
// DM state, conversation list, and DM message rendering, and how a file in a
// private conversation (a DM or a P2P group) is drawn (10k, privateFileHtml).
// Depends on: app.js (ws, myKey, myName, activeChannel, esc, formatBody,
//   appendMessage, formatTime, generateIdenticon, shortKey, switchChannel,
//   renderChannelList, fillFileCards, fileCautionFill)
// chat-ui.js (isMobile, closeSidebars, switchSidebarTab)
// crypto.js (pqParseFileMarker, pqDecryptFile), chat-protected.js
//   (protectedHidesPicturesFrom, protectedPictureHiddenHtml)
// ─────────────────────────────────────────────────────────────────────────

// ── DM State ──
let activeDmPartner = null; // Public key of active DM partner, or null for channel view.
let activeDmPartnerName = '';
let dmConversations = []; // Array of { partner_key, partner_name, last_message, last_timestamp, unread_count }

function upsertDmConversation(partnerKey, partnerName, lastMessage, lastTimestamp, incoming) {
  if (!partnerKey) return;
  const idx = dmConversations.findIndex(c => c.partner_key === partnerKey);
  if (idx >= 0) {
    const row = dmConversations[idx];
    row.partner_name = partnerName || row.partner_name;
    row.last_message = String(lastMessage || row.last_message || '');
    row.last_timestamp = Number(lastTimestamp || row.last_timestamp || Date.now());
    if (incoming && activeDmPartner !== partnerKey) {
      row.unread_count = Number(row.unread_count || 0) + 1;
    }
  } else {
    dmConversations.push({
      partner_key: partnerKey,
      partner_name: partnerName || shortKey(partnerKey),
      last_message: String(lastMessage || ''),
      last_timestamp: Number(lastTimestamp || Date.now()),
      unread_count: (incoming && activeDmPartner !== partnerKey) ? 1 : 0,
    });
  }
  dmConversations.sort((a, b) => Number(b.last_timestamp || 0) - Number(a.last_timestamp || 0));
  renderDmList();
  // Persist recent DMs summary for dashboard widget.
  try {
    const recent = dmConversations.slice(0, 10).map(c => ({
      name: c.partner_name,
      preview: dmSafePreview(c.last_message).slice(0, 80),
      time: c.last_timestamp ? new Date(Number(c.last_timestamp) * (c.last_timestamp < 1e12 ? 1000 : 1)).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }) : '',
      unread: c.unread_count || 0,
    }));
    localStorage.setItem('hos_dm_recent', JSON.stringify(recent));
  } catch {}
}

/** Switch to DM conversation view. */
function openDmConversation(partnerKey, partnerName) {
  if (!partnerKey) return;
  // Resolve best display name: passed arg > peerData > short key
  const resolvedName = partnerName ||
    (window.peerData && window.peerData[partnerKey]?.display_name) ||
    (typeof shortKey === 'function' ? shortKey(partnerKey) : partnerKey.slice(0, 8));
  // A reply belongs to the view it was made in: a new view starts without one (app.js).
  if (typeof clearReplyTarget === 'function') clearReplyTarget();
  activeDmPartner = partnerKey;
  activeDmPartnerName = resolvedName;
  // Clear group context so sendMessage doesn't accidentally route to the active group.
  if (typeof activeGroupId !== 'undefined') { activeGroupId = null; activeGroupName = ''; }

  // Ensure conversation appears in DM list immediately, even before server confirms.
  // (A brand-new conversation won't be in dm_list yet, so we seed it locally.)
  upsertDmConversation(partnerKey, resolvedName, '', Date.now(), false);

  // Switch to DMs tab in sidebar.
  if (typeof switchSidebarTab === 'function') switchSidebarTab('dms', true);

  // Clear unread for this conversation and update sidebar highlighting.
  const row = dmConversations.find(c => c.partner_key === partnerKey);
  if (row) row.unread_count = 0;
  renderDmList();
  renderChannelList(); // Deselect server channels
  if (typeof renderGroupList === 'function') renderGroupList(); // Deselect groups

  // Hide pin bar in DM view.
  document.getElementById('pin-bar').style.display = 'none';
  document.getElementById('pin-list').classList.remove('open');

  // Update channel header.
  renderDmHeader();

  // Clear messages area and set DM context (crimson tint + red stripes).
  const msgsEl = document.getElementById('messages');
  msgsEl.innerHTML = '';
  msgsEl.dataset.ctx = 'dm';
  if (typeof resetMsgStripe === 'function') resetMsgStripe();

  // Enable input.
  const input = document.getElementById('msg-input');
  input.disabled = false;
  input.placeholder = `Message ${partnerName}…`;
  document.getElementById('send-btn').disabled = false;
  // The attach button follows Files in messages (chat-messages.js).
  if (typeof updateAttachButton === 'function') updateAttachButton();

  // History renders from the LOCAL encrypted store — the relay keeps no
  // DM history any more (sealed-sender: its mailbox is a sender-less
  // delivery window that expires).
  renderDmConversationFromStore(partnerKey);

  if (isMobile()) closeSidebars();
}

/**
 * The open conversation's header: Back, their name, Block (Unblock while they
 * are blocked; step C, 2026-10-09) and Report (step D: their messages here can
 * go to the admins as evidence, chat-reports.js). Redrawn by chat-privacy.js
 * when a block changes.
 */
function renderDmHeader() {
  const header = document.getElementById('channel-header');
  if (!header || !activeDmPartner) return;
  const blocked = typeof isBlockedKey === 'function' && isBlockedKey(activeDmPartner);
  const blockTitle = blocked
    ? 'Unblock them: they can reach you again as your safety settings allow.'
    : 'Block them: you will not see anything from them, and they are not told.';
  header.innerHTML = `<span class="ch-name" style="cursor:pointer;" onclick="closeDmView()">← Back</span> <span class="ch-name">${hosIcon('chat', 16)} ${esc(activeDmPartnerName)}</span>`
    + `<button class="vr-btn dm-block-btn" onclick="toggleBlockActiveDm()" title="${esc(blockTitle)}" style="float:right;font-size:0.7rem;${blocked ? '' : 'color:var(--danger);'}">${blocked ? 'Unblock' : 'Block'}</button>`
    + `<button class="vr-btn dm-report-btn" onclick="reportActiveDm()" title="Report them to this server's admins, with their messages you choose as evidence. They are not told who reported them." style="float:right;font-size:0.7rem;margin-right:var(--space-xs);color:var(--danger);">Report</button>`;
  header.style.display = 'block';
}

/** The header's Block / Unblock button. */
function toggleBlockActiveDm() {
  const key = activeDmPartner;
  if (!key) return;
  if (typeof isBlockedKey === 'function' && isBlockedKey(key)) {
    if (typeof unblockKey === 'function') unblockKey(key);
  } else if (typeof blockKey === 'function') {
    blockKey(key);
  }
}

/** Render a DM conversation from the LOCAL history store into #messages. */
function renderDmConversationFromStore(partnerKey) {
  const msgsEl = document.getElementById('messages');
  msgsEl.innerHTML = '';
  const banner = document.createElement('div');
  banner.style.cssText = 'text-align:center;font-size:0.7rem;padding:var(--space-sm);color:var(--text-muted);';
  banner.innerHTML = hosIcon('lock', 14) + ' End-to-end encrypted, sender sealed inside (post-quantum). History lives on your devices; the server keeps no readable copy.';
  msgsEl.appendChild(banner);
  if (!(window.hosDmStore && hosDmStore.ready)) {
    if (typeof renderFileTransferRows === 'function') renderFileTransferRows(partnerKey);
    return;
  }
  const msgs = hosDmStore.conversation(partnerKey);
  if (msgs.length > 0) {
    const notice = document.createElement('div');
    notice.id = 'history-notice';
    notice.textContent = `── ${msgs.length} earlier messages ──`;
    msgsEl.appendChild(notice);
  }
  for (const m of msgs) {
    const isMe = m.from === myKey;
    const name = isMe
      ? myName
      : ((window.peerData && peerData[m.from]?.display_name) || activeDmPartnerName || shortKey(m.from));
    addDmMessage(name, m.text, m.ts, m.from, m.to, true);
  }
  const last = msgs[msgs.length - 1];
  if (last) hosDmStore.markRead(partnerKey, last.ts);
  // Device-to-device big files between us (2026-10-11): a row each, after the messages
  // (chat-file-transfer.js).
  if (typeof renderFileTransferRows === 'function') renderFileTransferRows(partnerKey);
}

/** Rebuild the sidebar conversation list from the local store. */
function loadDmListFromStore() {
  if (!(window.hosDmStore && hosDmStore.ready)) return;
  const summaries = hosDmStore.summaries();
  const known = new Set(summaries.map(s => s.peer));
  // Preserve locally-seeded entries (brand-new, still-empty conversations).
  const localOnly = dmConversations.filter(c => !known.has(c.partner_key));
  dmConversations = summaries.map(s => ({
    partner_key: s.peer,
    partner_name: (window.peerData && peerData[s.peer]?.display_name)
      || dmConversations.find(c => c.partner_key === s.peer)?.partner_name
      || shortKey(s.peer),
    last_message: (s.lastFromMe ? 'You: ' : '') + dmSafePreview(s.lastText),
    last_timestamp: s.lastTs,
    unread_count: s.unread ? 1 : 0,
  })).concat(localOnly);
  dmConversations.sort((a, b) => Number(b.last_timestamp || 0) - Number(a.last_timestamp || 0));
  renderDmList();
}

/** Sealed-sender privacy control: delete every envelope currently queued
 *  for us server-side (they auto-expire after the server TTL anyway; this
 *  is the immediate scrub). Local history on this device is untouched. */
function purgeServerMailbox() {
  // No confirm() dialog: the press-and-HOLD on the button IS the confirmation
  // (see holdToConfirm / the .purge-mailbox-row wiring), so this only fires
  // after a deliberate ~1.5s hold, never a stray click.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'dm_purge' }));
    if (typeof addSystemMessage === 'function') addSystemMessage('Clearing your server mailbox…');
  }
}

/** Close DM view and return to channel view. */
function closeDmView() {
  activeDmPartner = null;
  activeDmPartnerName = '';
  renderDmList();
  switchChannel(activeChannel);
}

/** Add a DM message to the message area. */
function addDmMessage(author, body, timestamp, fromKey, toKey, isEncrypted) {
  const el = document.createElement('div');
  const stripe = (typeof getStripeClass === 'function') ? getStripeClass(fromKey || author) : '';
  el.className = 'message dm-message' + (stripe ? ' ' + stripe : '');
  el.dataset.from = fromKey;
  el.dataset.timestamp = timestamp;

  // Native-parity sender grouping (mirrors the main-channel builder).
  const isContinuation = (typeof isMessageContinuation === 'function') && isMessageContinuation(fromKey, timestamp);
  if (isContinuation) el.classList.add('continuation');

  const isMe = fromKey === myKey;

  const isBotMsg2 = fromKey && fromKey.startsWith('bot_');
  const identiconSrc = (!isBotMsg2 && fromKey) ? generateIdenticon(fromKey, 32) : '';
  const identiconHtml = isBotMsg2 ? '<span class="identicon" style="font-size:calc(var(--avatar-size) * 0.75);line-height:var(--avatar-size);text-align:center;">🤖</span>' : (identiconSrc ? `<img src="${identiconSrc}" class="identicon" alt="">` : '');
  const e2eeBadge = isEncrypted ? '<span class="dm-e2ee" title="End-to-end encrypted" style="opacity:0.6;margin-left:var(--space-xs);">' + hosIcon('lock', 12) + '</span>' : '';

  const metaHtml = `<div class="meta"><span class="author${isMe ? ' you' : ''}">${esc(author)}</span></div>`;

  // Encrypted attachment (2026-08-24): a [[hum:file:v1]] marker renders as a
  // card, never as its text (the text holds the file's key). The file's
  // ciphertext is public but useless; the key rode in this sealed message.
  // Drawn the same way as one in a P2P group (10k, privateFileHtml below);
  // with the protected setup on, one from someone who is not a friend is not
  // shown and never fetched or opened: one line in its place (10h).
  const fileMeta = (typeof pqParseFileMarker === 'function') ? pqParseFileMarker(body) : null;
  const bodyHtml = fileMeta ? privateFileHtml(fileMeta, fromKey) : formatBody(body, fromKey);

  // Step F (2026-10-10, chat-warnings.js): a stranger's links are held until
  // the line under the message's Open is pressed, and the warnings the message
  // matches show under it. Never on my own messages.
  const received = !!fromKey && !isMe;
  const holdLinks = received && !fileMeta && typeof dmLinksHeld === 'function' && typeof holdLinksHtml === 'function'
    && dmLinksHeld(fromKey, timestamp, body);

  el.innerHTML = messageRowHTML({
    isContinuation,
    identiconHtml,
    metaHtml,
    pillHtml: timestampPillHTML({ time: formatTimePill(timestamp), extra: e2eeBadge }),
    bodyHtml: holdLinks ? holdLinksHtml(bodyHtml) : bodyHtml,
  });

  appendMessage(el);
  if (received && typeof addMessageSafetyLines === 'function') {
    addMessageSafetyLines(el, { text: body, from: fromKey, ts: timestamp, context: 'dm', name: author, liveBodyHtml: holdLinks ? bodyHtml : null });
  }
  if (fileMeta) hydratePrivateFile(el, fileMeta, fromKey);
  // A file's address in the words: its card's size and what kind of file it is (app.js).
  else if (typeof fillFileCards === 'function') fillFileCards(el);
  if (window.twemoji) twemoji.parse(el);
  return el;
}

// ── Files in private conversations (10k, 2026-10-10) ────────────────────────
// docs/design/blocking-and-safe-mode.md 10k. A [[hum:file:v1]] marker in a
// direct message (addDmMessage above) or in a P2P group message (app.js
// addChatMessage, for chat-groups-p2p.js) is drawn the same way, here:
//  - with the protected setup hiding pictures from someone who is not a
//    friend (10h, chat-protected.js), its one line, and nothing is fetched or
//    opened;
//  - a picture (by its `mime`: png, jpeg, gif or webp) is fetched, opened with
//    the key from the marker and shown inline: at once when it is mine or a
//    friend's (a mutual follow, the test the warnings use), and after a click
//    on "Image (click to load)" when the sender is not a friend, as any picture
//    link from them is (app.js formatBody);
//  - anything else is a card with its name and size and a Save button, which
//    fetches and opens it only when pressed;
//  - by size (round 3, review 7): a picture is shown in place only up to
//    16 MB (larger, it is a Save card saying so), Save opens a file up to
//    512 MB, and a larger file is a card saying to open it in the desktop app,
//    with nothing fetched; a picture opened once is kept for this page, so a
//    conversation drawn again does not download it again;
//  - a fetch or an opening that fails says "This file could not be opened.",
//    and the ciphertext is never shown.
// Only a file on this server is fetched (a `/uploads/` path, which is what the
// encrypted upload answers): a marker naming any other address is not
// followed, so a message cannot make this device reach another site.
//
// Files in messages (2026-10-11, docs/design/media-showcase.md, "Files on the
// sender's device and in messages, exact", parts 2 to 4; the settings and
// their words are /shared/files-in-messages.js, drawn in Safety by
// chat-privacy.js). THIS app enforces them, never the server:
//  - "Accept files from" (Nobody / Friends / Anyone I talk to, default
//    Friends): a file from someone it does not allow is not fetched at all;
//    its card says "Ann sent a file. You accept files only from friends.
//    Accept this one?", and Accept lets that one file through (a picture is
//    then fetched and shown; any other file gets its Save);
//  - "Show pictures without asking from" (Friends / Nobody; the protected
//    setup is always Nobody) is the click-to-load rule above, made a setting;
//    my own pictures always show;
//  - a file whose marker says `"unchecked": true` (its sender's app could not
//    check it for hidden location, and they sent it anyway) carries one line
//    under its name saying so;
//  - a file opened here is kept on this device, encrypted with the DM
//    history's key (/shared/kept-files.js through chat-dm-store.js), and
//    opening a file looks there first, so it still opens after the server
//    removes its copy.
// Without /shared/files-in-messages.js loaded, every file but mine asks first.
// Sending is chat-messages.js (sendEncryptedAttachment).
// Test: scripts/tests/private-files-web.test.js

const PRIVATE_FILE_FAILED = 'This file could not be opened.';
// The picture types shown inline, the ones formatBody shows from a link. Any
// other type (an SVG, which can carry a script) is a card with Save instead.
const PRIVATE_FILE_PICTURE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];
// The files the reader chose to load, by address, for this page only, so a
// conversation drawn again (a group redraws on each new message) keeps them.
const privateFilesLoaded = new Set();

// What a browser can hold (docs/design/media-showcase.md round 3, review 7). A
// private file is fetched whole and opened at once (WebCrypto's AES-GCM takes
// the whole ciphertext and gives the whole file), so the page holds it about
// twice while it opens: pictures are shown in place up to 16 MB, any file is
// saved up to 512 MB, and a larger one is a card saying to open it in the
// desktop app, which reads it in pieces. The sizes are the marker's (the
// sender's word), and the server's own length is held to them as well.
const PRIVATE_PICTURE_INLINE_MAX = 16 * 1024 * 1024;
const PRIVATE_FILE_SAVE_MAX = 512 * 1024 * 1024;
// AES-GCM adds its 16-byte tag; a little more is allowed for nothing else.
const PRIVATE_FILE_SEAL_BYTES = 1024;
// Opened pictures kept for redraws, at most this much of them.
const PRIVATE_OPENED_KEEP_BYTES = 128 * 1024 * 1024;
const _mbWords = (n) => Math.round(n / (1024 * 1024)) + ' MB';

/** The file's size as its marker says it, 0 when it does not. */
function privateFileSize(meta) {
  const n = Number(meta && meta.size);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

/**
 * How a private file is offered: 'picture' (shown in place, at once for a
 * friend, else on a click), 'save' (a card with Save: any other file, and a
 * picture over 16 MB) or 'desktop' (over 512 MB: a card saying to open it in
 * the desktop app, nothing fetched).
 */
function privateFileWay(meta) {
  const size = privateFileSize(meta);
  if (size > PRIVATE_FILE_SAVE_MAX) return 'desktop';
  if (privateFileIsPicture(meta) && size <= PRIVATE_PICTURE_INLINE_MAX) return 'picture';
  return 'save';
}

/** The card's note for a file too large to show or to open here, or ''. */
function privateFileNote(meta) {
  const way = privateFileWay(meta);
  if (way === 'desktop') {
    return 'This file is ' + _fmtBytes(privateFileSize(meta)) + ', more than a browser can open (up to ' + _mbWords(PRIVATE_FILE_SAVE_MAX) + '). Open it in the HumanityOS desktop app.';
  }
  if (way === 'save' && privateFileIsPicture(meta)) {
    return 'This picture is ' + _fmtBytes(privateFileSize(meta)) + ', larger than is shown here (up to ' + _mbWords(PRIVATE_PICTURE_INLINE_MAX) + '). Save it to see it.';
  }
  return '';
}

function _fmtBytes(n) {
  n = Number(n) || 0;
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(0) + ' KB';
  return (n / 1024 / 1024).toFixed(1) + ' MB';
}

/** Is this file a picture shown inline? */
function privateFileIsPicture(meta) {
  return PRIVATE_FILE_PICTURE_TYPES.includes(String((meta && meta.mime) || '').toLowerCase());
}

/**
 * Is a file from `fromKey` shown without a click: mine, or a friend's? A
 * friend here is the desktop app's: a mutual follow who also holds a pass from
 * me (someone I let through under "Friends"), not a mutual follow alone. Before
 * the local store loads nobody is, so nothing from anyone opens early.
 */
function privateFileFromFriend(fromKey) {
  if (!fromKey || typeof fromKey !== 'string') return false;
  if (typeof myKey === 'string' && myKey && fromKey.toLowerCase() === myKey.toLowerCase()) return true;
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store) return false;
  return !!(store.isFriendPeer(fromKey) && store.certSentTo(fromKey));
}

/** Are files from `fromKey` not shown now (the protected setup's pictures rule)? */
function privateFileHidden(fromKey) {
  return typeof protectedHidesPicturesFrom === 'function' && protectedHidesPicturesFrom(fromKey);
}

// ── Files in messages (part 3) ──
// The files the person pressed Accept on, by address, for this page.
const privateFilesAccepted = new Set();
// The line under a file whose sender's app could not check it (part 2): /shared/upload-clean.js's words.
const PRIVATE_FILE_UNCHECKED = (window.HosUploadClean && HosUploadClean.WORDS && HosUploadClean.WORDS.recipientLine)
  || "The sender's app could not check this file for hidden location.";

/** This device's Files in messages settings, or null when /shared/files-in-messages.js is not loaded. */
function privateFilesSettings() {
  const F = window.HosFilesInMessages;
  if (!F) return null;
  let storage = null;
  try { storage = window.localStorage; } catch (e) { storage = null; }
  return F.readSettings(storage);
}

/** The sender as the Files in messages rules see them: me, or a friend (privateFileFromFriend). */
function privateFileSender(fromKey) {
  const isMe = typeof myKey === 'string' && !!myKey && typeof fromKey === 'string' && fromKey.toLowerCase() === myKey.toLowerCase();
  return { isMe, isFriend: isMe || privateFileFromFriend(fromKey) };
}

/** Is this file downloaded without asking ("Accept files from", or Accept pressed here)? */
function privateFileAccepted(meta, fromKey) {
  if (meta && privateFilesAccepted.has(meta.url)) return true;
  const who = privateFileSender(fromKey);
  const settings = privateFilesSettings();
  if (!settings) return who.isMe;
  return HosFilesInMessages.acceptsFrom(settings, who);
}

/** Is this picture shown at once ("Show pictures without asking from", or a click here)? */
function privateFilePictureNow(meta, fromKey) {
  if (meta && privateFilesLoaded.has(meta.url)) return true;
  const who = privateFileSender(fromKey);
  const settings = privateFilesSettings();
  if (!settings) return who.isMe;
  const prot = typeof protectedIsOn === 'function' && protectedIsOn();
  return HosFilesInMessages.picturesWithoutAsking(settings, who, prot);
}

/** The words on a file that was not downloaded: "Ann sent a file. You accept files only from friends. Accept this one?" */
function privateFileAskWords(fromKey) {
  const name = typeof reachDisplayName === 'function' ? reachDisplayName(fromKey)
    : ((typeof peerData !== 'undefined' && peerData && peerData[fromKey] && peerData[fromKey].display_name) || 'Someone');
  const settings = privateFilesSettings();
  return window.HosFilesInMessages ? HosFilesInMessages.askWords(name, settings) : `${name} sent a file. Accept this one?`;
}

/** The body of a message that is a file: the protected setup's line, or the card. */
function privateFileHtml(meta, fromKey) {
  if (privateFileHidden(fromKey) && typeof protectedPictureHiddenHtml === 'function') return protectedPictureHiddenHtml();
  // The line under a file its sender's app could not check, written by hydratePrivateFile with textContent.
  const unchecked = meta && meta.unchecked === true ? '<div class="enc-attach-unchecked"></div>' : '';
  // What kind of file it is, when that deserves a word (a program, a script, an archive, a file
  // named like a picture over a program's bytes: /shared/file-risk.js). Written by
  // hydratePrivateFile with textContent, by its name, and again with its first bytes once it is
  // opened. The file is encrypted end to end, so the server never saw its real name or contents:
  // this check, on the receiving side, is the only one.
  const caution = '<div class="file-caution enc-attach-caution" hidden></div>';
  // Its type, just before its name, always (app.js fileBadgeSlotHtml): by its name, or from its
  // first bytes when it was opened here before; drawn again by privateFileCaution once it is opened.
  const named = (meta && meta.name) || 'file';
  const badge = fileBadgeSlotHtml(named, privateFileHeads.get(privateFileHeadKey(meta)) || null, 'enc-attach-badge');
  return `<div class="enc-attach" data-enc="1">
    <div class="enc-attach-head">${hosIcon('lock', 12)} ${badge}<span class="enc-attach-name">${esc(named)}</span>
      <span class="enc-attach-size">${_fmtBytes(meta.size)}</span></div>
    ${caution}${unchecked}<div class="enc-attach-body">${privateFileBodyHtml(meta, fromKey)}</div>
  </div>`;
}

// The first bytes of each private file opened on this page (address + key -> up to 16 bytes),
// so its caution line can be said again from what is inside it, not only its name.
const privateFileHeads = new Map();
const PRIVATE_FILE_HEAD_BYTES = 16;
const privateFileHeadKey = (meta) => String(meta && meta.url) + '\n' + String(meta && meta.k);

/** Keep the first bytes of a private file just opened (`bytes`: its plain bytes, a Uint8Array). */
function privateFileKeepHead(meta, bytes) {
  try {
    if (bytes && typeof bytes.length === 'number') {
      privateFileHeads.set(privateFileHeadKey(meta), new Uint8Array(bytes.slice(0, PRIVATE_FILE_HEAD_BYTES)));
    }
  } catch (e) { /* no head: the name's word stands */ }
}

/**
 * Say on a private file's card what kind of file it is: its type's badge, just before its name
 * (app.js fileBadgeFill), and its word of caution when it deserves one (app.js fileCautionFill). By
 * its name, and with its first bytes once it has been opened here, so a file whose name says a
 * picture over the bytes of a program shows red and is called out as disguised.
 */
function privateFileCaution(card, meta) {
  if (!card || typeof card.querySelector !== 'function') return;
  const head = privateFileHeads.get(privateFileHeadKey(meta)) || null;
  if (typeof fileBadgeFill === 'function') fileBadgeFill(card.querySelector('.enc-attach-badge'), (meta && meta.name) || 'file', head);
  const line = card.querySelector('.enc-attach-caution');
  if (!line || typeof fileCautionFill !== 'function') return;
  fileCautionFill(line, (meta && meta.name) || '', head);
}

/** The inside of a file card: Accept (a file not allowed yet), the note, Save, the picture or its click. */
function privateFileBodyHtml(meta, fromKey) {
  // Not allowed by "Accept files from": nothing is fetched until Accept. Its words are written
  // by hydratePrivateFile with textContent.
  if (!privateFileAccepted(meta, fromKey)) {
    return '<div class="enc-attach-ask"></div><button type="button" class="enc-attach-accept">Accept</button>';
  }
  const way = privateFileWay(meta);
  const now = way === 'picture' && privateFilePictureNow(meta, fromKey);
  // The note's words (a picture too large to show here, a file too large for a browser) are
  // written by hydratePrivateFile with textContent.
  const note = privateFileNote(meta) ? '<div class="enc-attach-note"></div>' : '';
  return way === 'desktop'
    ? note
    : way === 'save'
      ? note + '<button type="button" class="enc-attach-dl">Save</button>'
      : now
        ? '<div class="enc-attach-loading">Opening image…</div>'
        : '<button type="button" class="enc-attach-load">' + hosIcon('image', 14) + ' Image (click to load)</button>';
}

/**
 * Fetch a file's ciphertext from this server and open it with the key in its
 * marker. A Blob of the opened bytes, or null when the address is not a file
 * on this server, the fetch fails, the file is larger than `max` (the marker's
 * size is the sender's word, so the server's own length is checked too), or the
 * bytes do not open (a wrong key or altered bytes): the caller then says the
 * file could not be opened. The ciphertext is let go as soon as it is opened.
 */
async function openPrivateFile(meta, max) {
  try {
    const url = meta && typeof meta.url === 'string' ? meta.url : '';
    if (!/^\/uploads\/[^\s?#\\]+$/.test(url) || url.includes('..')) return null;
    const type = privateFileWay(meta) === 'picture' ? String(meta.mime).toLowerCase() : 'application/octet-stream';
    // A copy kept on this device (part 4) opens without asking the server, which may have let
    // the file go: the DM mailbox's lifetime, 30 days by default.
    const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
    if (store && typeof store.keptFileGet === 'function') {
      const kept = await store.keptFileGet(meta);
      if (kept && kept.length <= (Number(max) || PRIVATE_FILE_SAVE_MAX)) {
        privateFileKeepHead(meta, kept);
        return new Blob([kept], { type });
      }
    }
    const limit = (Number(max) || PRIVATE_FILE_SAVE_MAX) + PRIVATE_FILE_SEAL_BYTES;
    const resp = await fetch(url);
    if (!resp || !resp.ok) return null;
    const said = resp.headers && typeof resp.headers.get === 'function' ? Number(resp.headers.get('content-length')) : NaN;
    if (Number.isFinite(said) && said > limit) {
      try { if (resp.body && typeof resp.body.cancel === 'function') resp.body.cancel(); } catch (e) { /* already closed */ }
      return null;
    }
    let ct = new Uint8Array(await resp.arrayBuffer());
    if (ct.length > limit) return null;
    const plain = await pqDecryptFile(ct, meta.k, meta.n);
    ct = null;
    if (!plain) return null;
    // Its first bytes, for the card's caution line (privateFileCaution).
    privateFileKeepHead(meta, plain);
    // Opened: kept on this device for when the server no longer has it (within the cap,
    // Settings > Storage). Never waited for: the file is shown or saved at once.
    if (store && typeof store.keptFilePut === 'function') store.keptFilePut(meta, plain).catch(() => {});
    return new Blob([plain], { type });
  } catch (e) {
    return null;
  }
}

/**
 * A picture opened once is kept for this page (its object URL, by address and
 * key), so a conversation drawn again (a group redraws on every new message,
 * a DM on every open) shows it without downloading it again. At most
 * PRIVATE_OPENED_KEEP_BYTES of them; the oldest beyond that are let go.
 */
const privateFilesOpened = new Map(); // address + '\n' + key -> {url, size} or {pending: Promise}
let privateOpenedBytes = 0;

async function openPrivatePicture(meta) {
  const key = String(meta.url) + '\n' + String(meta.k);
  const have = privateFilesOpened.get(key);
  if (have && have.pending) return have.pending;
  if (have) {
    // Kept as the newest: the one just shown is the last to go.
    privateFilesOpened.delete(key);
    privateFilesOpened.set(key, have);
    return have.url;
  }
  const pending = (async () => {
    const blob = await openPrivateFile(meta, PRIVATE_PICTURE_INLINE_MAX);
    if (!blob) { privateFilesOpened.delete(key); return null; }
    const url = URL.createObjectURL(blob);
    // Taken out and put back, so it is the newest: a Map keeps a key where it was first set, and
    // its placeholder was set when it was asked for. Left there, a picture that finished after
    // later ones sat before them, the sweep below stopped at it, and nothing was let go (round 4,
    // W4-4: 20 pictures finishing in reverse kept 320 MB against the 128 MB bound).
    privateFilesOpened.delete(key);
    privateFilesOpened.set(key, { url, size: blob.size });
    privateOpenedBytes += blob.size;
    for (const [k, v] of privateFilesOpened) {
      if (privateOpenedBytes <= PRIVATE_OPENED_KEEP_BYTES || k === key) break;
      if (v.pending) continue;
      privateFilesOpened.delete(k);
      privateOpenedBytes -= v.size;
      try { URL.revokeObjectURL(v.url); } catch (e) { /* already gone */ }
    }
    return url;
  })();
  privateFilesOpened.set(key, { pending });
  return pending;
}

/**
 * Wire a drawn file card: the unchecked line, Accept (a file "Accept files from" does not
 * allow yet), then a picture (now, or on its click) or Save. Never runs while the file is hidden.
 */
async function hydratePrivateFile(el, meta, fromKey) {
  if (privateFileHidden(fromKey)) return;
  const card = el && el.querySelector('.enc-attach');
  const bodyEl = card && card.querySelector('.enc-attach-body');
  if (!bodyEl) return;
  if (meta && meta.unchecked === true) {
    const line = card.querySelector('.enc-attach-unchecked');
    if (line) line.textContent = PRIVATE_FILE_UNCHECKED;
  }
  // What kind of file it is, by its name (and its first bytes, when it was opened here before).
  privateFileCaution(card, meta);
  if (!privateFileAccepted(meta, fromKey)) {
    const ask = bodyEl.querySelector('.enc-attach-ask');
    if (ask) ask.textContent = privateFileAskWords(fromKey);
    const accept = bodyEl.querySelector('.enc-attach-accept');
    if (!accept) return;
    accept.onclick = async (e) => {
      if (e && typeof e.stopPropagation === 'function') e.stopPropagation();
      // This one file, for this page. Accept is the person's own click, so a picture is
      // fetched and shown now; any other file gets its Save, which fetches it.
      privateFilesAccepted.add(meta.url);
      if (privateFileWay(meta) === 'picture') privateFilesLoaded.add(meta.url);
      bodyEl.innerHTML = privateFileBodyHtml(meta, fromKey);
      await hydratePrivateFileBody(bodyEl, meta, fromKey, card);
    };
    return;
  }
  await hydratePrivateFileBody(bodyEl, meta, fromKey, card);
}

/**
 * Wire a card's body once the file is allowed: the note, Save, or the picture (now, or on its
 * click). `card`: the whole card, whose caution line is said again from the file's first bytes
 * once it is opened.
 */
async function hydratePrivateFileBody(bodyEl, meta, fromKey, card) {
  const failed = () => { bodyEl.innerHTML = '<div class="enc-attach-failed">' + esc(PRIVATE_FILE_FAILED) + '</div>'; };
  const way = privateFileWay(meta);
  const words = privateFileNote(meta);
  if (words) {
    const note = bodyEl.querySelector('.enc-attach-note');
    if (note) note.textContent = words;
  }

  // Too large for a browser to open: the note says where it can be opened, and nothing is fetched.
  if (way === 'desktop') return;

  if (way === 'save') {
    const btn = bodyEl.querySelector('.enc-attach-dl');
    if (!btn) return;
    btn.onclick = async (e) => {
      if (e && typeof e.stopPropagation === 'function') e.stopPropagation();
      btn.disabled = true;
      btn.textContent = 'Saving…';
      const blob = await openPrivateFile(meta, PRIVATE_FILE_SAVE_MAX);
      if (!blob) { failed(); return; }
      // Opened: what is inside it may say more than its name did (a "picture" that is a program).
      privateFileCaution(card, meta);
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = meta.name || 'file';
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 4000);
      btn.disabled = false;
      btn.textContent = 'Save';
    };
    return;
  }

  const show = async () => {
    bodyEl.innerHTML = '<div class="enc-attach-loading">Opening image…</div>';
    const url = await openPrivatePicture(meta);
    if (!url) { failed(); return; }
    privateFileCaution(card, meta);
    const img = document.createElement('img');
    img.src = url;
    img.alt = meta.name || 'image';
    img.className = 'enc-attach-img';
    img.loading = 'lazy';
    img.onclick = () => window.open(url, '_blank');
    bodyEl.innerHTML = '';
    bodyEl.appendChild(img);
  };
  if (privateFilePictureNow(meta, fromKey)) {
    await show();
    return;
  }
  const load = bodyEl.querySelector('.enc-attach-load');
  if (!load) return;
  load.onclick = async (e) => {
    if (e && typeof e.stopPropagation === 'function') e.stopPropagation();
    privateFilesLoaded.add(meta.url);
    await show();
  };
}

window.privateFileHtml = privateFileHtml;
window.hydratePrivateFile = hydratePrivateFile;
window.PRIVATE_FILE_FAILED = PRIVATE_FILE_FAILED;

// DM previews loaded from the zero-knowledge relay arrive as the raw E2EE
// envelope ({"v":1,"r":{...}}), the relay can't decrypt them. Never show that
// ciphertext; collapse it to a lock placeholder (matches the incoming-DM
// handler in app.js and native's clean DM list).
function dmSafePreview(raw) {
  raw = String(raw || '');
  if (/^\s*\{\s*"v"\s*:\s*\d/.test(raw) || raw.includes('"ek_ct') || /"r"\s*:\s*\{/.test(raw)) {
    return '🔒 Encrypted message';
  }
  // Encrypted attachment marker: show a friendly label, never the base64.
  const fm = (typeof pqParseFileMarker === 'function') ? pqParseFileMarker(raw) : null;
  if (fm) return ((fm.mime || '').startsWith('image/') ? '🔒 Photo' : '🔒 ' + (fm.name || 'File'));
  return raw;
}

/** Render the DM conversation list in the sidebar. */
function renderDmList() {
  const list = document.getElementById('dm-list');
  // Sealed-sender scrub control, always available at the foot of the list.
  // PRESS-AND-HOLD, not a click: this scrubs the server mailbox for every
  // device you haven't synced yet, so a single fat-fingered tap must not fire
  // it (operator 2026-08-24). The .hold-ring fills over the hold; see
  // holdToConfirm below. Mirrors native widgets::hold_to_confirm.
  const purgeRow = '<div class="dm-item purge-mailbox-row" id="purge-mailbox-btn" '
    + 'title="Press and HOLD to delete the encrypted envelopes queued for you on the server. Local history on your devices stays. Holding prevents an accidental misclick.">'
    + '<span class="hold-ring" aria-hidden="true"></span>'
    + '<span class="dm-name">Delete my server mailbox</span>'
    + '<span class="hold-hint">hold</span></div>';
  // Contact requests ("who can reach me", step B): name only, Accept and
  // Ignore, above the conversations (chat-privacy.js draws and wires them).
  // Above them, while the protected setup is on, its always-visible line
  // (10h, chat-protected.js): the person it protects is told.
  const statusHtml = (typeof protectedStatusLineHtml === 'function') ? protectedStatusLineHtml('dm') : '';
  const requestsHtml = statusHtml + ((typeof contactRequestsSidebarHtml === 'function') ? contactRequestsSidebarHtml() : '');
  if (dmConversations.length === 0) {
    list.innerHTML = requestsHtml + '<div style="font-size:0.7rem;color:var(--text-muted);padding:var(--space-sm) var(--space-md);">No conversations yet</div>' + purgeRow;
    if (typeof wireContactRequestButtons === 'function') wireContactRequestButtons(list);
    wirePurgeMailboxHold();
    return;
  }

  list.innerHTML = requestsHtml + dmConversations.map(c => {
    const isActive = activeDmPartner === c.partner_key;
    const unread = c.unread_count > 0 ? '<span class="dm-unread"></span>' : '';
    const timeStr = formatTime(c.last_timestamp);
    // Web keeps name-only (+ unread dot) with a right-aligned time, NO
    // message preview: the relay-stored DM body is an opaque E2EE envelope
    // here, so a sidebar preview would mostly render the lock placeholder
    // (original decision: operator, 2026-05-27). NOTE the native app DOES
    // show a decrypted last-message preview under each DM name as of
    // v0.715 (operator-approved, 2026-07-06) — it decrypts on arrival, so
    // its preview is real text. Don't "fix" native back to name-only for
    // parity; the two clients intentionally differ until web can decrypt
    // at list-render time.
    return `<div class="dm-item${isActive ? ' active' : ''}" onclick="openDmConversation('${esc(c.partner_key)}', '${esc(c.partner_name)}')">
      <span class="dm-name">${esc(c.partner_name)} ${unread}</span>
      <span class="dm-time">${timeStr}</span>
    </div>`;
  }).join('') + purgeRow;
  if (window.twemoji) twemoji.parse(list);
  if (typeof wireContactRequestButtons === 'function') wireContactRequestButtons(list);
  wirePurgeMailboxHold();
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}

/** Attach the press-and-hold gate to the mailbox-scrub row after each render.
 *  5s hold (operator 2026-08-25: max protection on the catastrophic scrub).
 *  holdToConfirm is the shared gate from /shared/hold-confirm.js. */
function wirePurgeMailboxHold() {
  const el = document.getElementById('purge-mailbox-btn');
  if (el && window.holdToConfirm) holdToConfirm(el, { seconds: 5, onConfirm: purgeServerMailbox });
}
