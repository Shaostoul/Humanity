// key-pins-shim.js: run INSIDE a chat test page's node:vm context, after app.js and
// chat-key-cards.js (`installKeyPinsShim(ctx)` from a test file).
//
// Key cards (2026-10-22, docs/design/key-cards.md): the chat seals to a person only with a key
// from their key card, checked and pinned in hosDmStore.keyCards. The older chat test harnesses
// (reach, block, report, protected, warnings, friend passes, choices, group reports, files, admin
// erase) test other features with stand-in keys ("b2b2...", "kyber-b2b2"), which no real key card
// can carry, and hand those keys out in their member lists as `kyber_public`, the field a server
// used to send. This shim stands in for the step those features take for granted: each person in
// a member list given to the page has had their card checked and pinned. It pins the stand-in key
// into the page's REAL store (and pins again after the store loads for another identity or
// server), so every seal still goes through crypto.js getPeerEcdhPublic and chat-key-cards.js
// exactly as in the browser. A real server sends no `kyber_public` any more; the checking itself
// is tested with real cards in scripts/tests/key-card-web.test.js and key-cards-chat-web.test.js.
(function () {
  const standIns = new Map(); // person -> stand-in Kyber key
  function pinAll() {
    if (!(window.hosDmStore && hosDmStore.ready)) return;
    let dirty = false;
    for (const [person, kyber] of standIns) {
      const pin = hosDmStore.keyCards.pins[person];
      if (!pin || pin.kyber !== kyber) {
        hosDmStore.keyCards.pins[person] = { kyber, at: 1 };
        dirty = true;
      }
    }
    if (dirty) hosDmStore._persistPins();
  }
  const note = (person, kyber) => {
    if (typeof person === 'string' && person && typeof kyber === 'string' && kyber) standIns.set(person, kyber);
  };
  const realHandle = handleMessage;
  handleMessage = function (msg) {
    if (msg && msg.type === 'full_user_list') for (const u of msg.users || []) note(u && u.public_key, u && u.kyber_public);
    if (msg && msg.type === 'peer_list') for (const p of msg.peers || []) note(p && p.public_key, p && p.kyber_public);
    if (msg && msg.type === 'peer_joined') note(msg.public_key, msg.kyber_public);
    pinAll();
    return realHandle.apply(this, arguments);
  };
  const realInit = hosDmStore.init;
  hosDmStore.init = async function () {
    const ok = await realInit.apply(this, arguments);
    pinAll();
    return ok;
  };
  window.__pinStandInKeys = (pairs) => {
    for (const [person, kyber] of pairs || []) note(person, kyber);
    pinAll();
  };
})();
