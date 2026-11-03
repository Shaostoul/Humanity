// ── settings-privacy.js ───────────────────────────────────────────────────
// Settings > Privacy on the website (BUG-237, 2026-11-03): the privacy levels and the two
// switches, as the desktop app's Settings > Privacy has them (src/gui/pages/privacy.rs
// draw_privacy_content), in the same words by key. Until then the website showed the level only
// once, at the first chat, and had no presence switch, so a person on the website could not
// become harder to see later. Protect the users (CLAUDE.md, 2026-10-09): the way back to Private
// is always this one page.
//
// What each control does (the levels and how a choice is kept: /shared/privacy-tiers.js):
//  - a level card picks the level; "Use this privacy level" uses it: both switches set from it,
//    and it is kept as the level last used, exactly as the chat's first-connect chooser does;
//  - "Show others when I am online" keeps the presence choice in this browser and tells this
//    server at once ({"type":"privacy_update","hide_presence":b}) on this page's own signed-in
//    connection (/shared/relay-session.js), when this browser holds a readable key; with the key
//    locked an open chat tab tells its server (chat-privacy.js, the storage event), and the chat
//    tells every server the next time it connects to it;
//  - "List me in the public member directory" records the choice in the profile the chat sends
//    (as the website's Profile page does): at once if the chat is open in another tab,
//    otherwise the next time it is opened.
// The line under the switches says which of these happened, then the server's own answer.
//
// A classic script in the browser (it mounts itself on #sec-privacy) and a CommonJS module under
// Node, where scripts/tests/privacy-web.test.js drives createPrivacySection with a stand-in page.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const P = root.HosPrivacyTiers || (typeof module === 'object' && module && module.exports ? require('../shared/privacy-tiers.js') : null);

  /**
   * The section. `d`: doc (with getElementById and createElement), storage (localStorage), fetch,
   * i18n (t, has, hint; or null), changes (HosProfileChanges, or null) and makeSession() ->
   * Promise<a relay session, or null when this browser has no readable key>.
   */
  function createPrivacySection(d) {
    const doc = d.doc;
    const el = (id) => doc.getElementById(id);
    const words = (key, vars) => (d.i18n ? d.i18n.t(key, vars) : key);
    const state = {
      tiers: [],
      defaultTier: 'private',
      selected: '',
      cards: [],      // [{ id, card, radio }]
      session: null,
      sessionAsked: false,
      pending: null,  // the presence frame waiting for the sign-in, the latest choice only
      status: '',     // the key the status line shows
    };

    function say(key, vars) {
      state.status = key || '';
      const line = el('privacy-status');
      if (line) line.textContent = key ? words(key, vars) : '';
    }

    function showSelected() {
      for (const c of state.cards) {
        c.card.className = 'privacy-tier-card' + (c.id === state.selected ? ' selected' : '');
        c.radio.checked = c.id === state.selected;
      }
    }

    function renderCards() {
      const host = el('privacy-tier-cards');
      if (!host) return;
      host.textContent = '';
      state.cards = [];
      for (const tier of state.tiers) {
        const card = doc.createElement('label');
        card.setAttribute('data-tier', tier.id);
        const head = doc.createElement('div');
        head.className = 'tier-head';
        const radio = doc.createElement('input');
        radio.type = 'radio';
        radio.name = 'privacy-tier';
        radio.value = tier.id;
        if (d.i18n) d.i18n.hint(radio, 'privacy.tier_card_hint');
        radio.addEventListener('change', () => { state.selected = tier.id; showSelected(); });
        const name = doc.createElement('span');
        name.className = 'tier-name';
        name.textContent = words(tier.name);
        const tagline = doc.createElement('span');
        tagline.className = 'tier-tagline';
        tagline.textContent = words(tier.tagline);
        const desc = doc.createElement('div');
        desc.className = 'tier-desc';
        desc.textContent = words(tier.description);
        head.appendChild(radio);
        head.appendChild(name);
        head.appendChild(tagline);
        card.appendChild(head);
        card.appendChild(desc);
        host.appendChild(card);
        state.cards.push({ id: tier.id, card, radio });
      }
      showSelected();
    }

    /** The level last used, in its words ("Privacy level last used: Private"); empty when none. */
    function renderCurrent() {
      const line = el('privacy-current');
      if (!line) return;
      const tier = P.tierById(state.tiers, P.chosenTierId(d.storage));
      line.textContent = tier ? words('privacy.current_tier', { tier: words(tier.name) }) : '';
    }

    /** Both switches as this browser keeps them; presence never chosen shows as hidden, as a
     *  server keeps an account that never chose (fail-private). */
    function renderSwitches() {
      const presence = el('privacy-presence');
      if (presence) presence.checked = P.presenceHidden(d.storage) === false;
      const directory = el('privacy-directory');
      if (directory) directory.checked = P.directoryListed(d.storage);
    }

    function render() {
      renderCards();
      renderCurrent();
      renderSwitches();
    }

    function onSessionEvent(ev, st) {
      if (ev.type === 'state') {
        if (st.status === 'bound' && state.pending) {
          const frame = state.pending;
          state.pending = null;
          state.session.send(frame);
        } else if (st.status === 'refused' && state.pending) {
          state.pending = null;
          say('privacy.web_not_sent', { why: st.refusal });
        } else if (st.status === 'no_identity' && state.pending) {
          state.pending = null;
          say('privacy.web_no_identity');
        }
      } else if (ev.type === 'frame') {
        const answer = P.presenceAnswer(ev.msg);
        if (answer === 'hidden') say('privacy.web_presence_hidden_done');
        else if (answer === 'shown') say('privacy.web_presence_shown_done');
      }
    }

    async function session() {
      if (!state.sessionAsked) {
        state.sessionAsked = true;
        try { state.session = d.makeSession ? await d.makeSession() : null; } catch (e) { state.session = null; }
        if (state.session) state.session.subscribe(onSessionEvent);
      }
      return state.session;
    }

    /** Tell this server the presence choice, on this page's own signed-in connection. */
    async function tellServer(frame) {
      say('privacy.web_saved_sending');
      const s = await session();
      if (!s) {
        say('privacy.web_no_identity');
        return;
      }
      if (s.state.status === 'bound' && s.send(frame)) return;
      state.pending = frame;
      await s.start();
    }

    /** "Use this privacy level". */
    async function useSelected() {
      const tier = P.tierById(state.tiers, state.selected);
      if (!tier) return;
      const frame = P.applyTier(d.storage, tier, d.changes || null);
      render();
      await tellServer(frame);
    }

    /** "Show others when I am online", changed. */
    async function setPresenceVisible(visible) {
      const frame = P.setPresence(d.storage, !visible);
      await tellServer(frame);
    }

    /** "List me in the public member directory", changed. */
    function setListed(listed) {
      P.setDirectory(d.storage, !!listed, d.changes || null);
      say('privacy.web_directory_saved');
    }

    async function mount() {
      const loaded = await P.loadTiers(d.fetch, d.i18n && d.i18n.has ? (k) => d.i18n.has(k) : null);
      state.tiers = loaded.tiers;
      state.defaultTier = loaded.defaultTier;
      // The level last used, if the file still has it (Spotlight was taken out, BUG-238); else the
      // default is picked, and nothing changes until the button is pressed.
      const chosen = P.chosenTierId(d.storage);
      state.selected = P.tierById(state.tiers, chosen) ? chosen : state.defaultTier;
      render();
      const apply = el('privacy-apply');
      if (apply) apply.addEventListener('click', () => { useSelected(); });
      const presence = el('privacy-presence');
      if (presence) presence.addEventListener('change', () => { setPresenceVisible(!!presence.checked); });
      const directory = el('privacy-directory');
      if (directory) directory.addEventListener('change', () => { setListed(!!directory.checked); });
      return state;
    }

    return { mount, render, useSelected, setPresenceVisible, setListed, state };
  }

  /** The browser's own page: a signed-in session made only when a change is sent. */
  function mountInBrowser() {
    if (!root.document || !root.document.getElementById('sec-privacy')) return null;
    const section = createPrivacySection({
      doc: root.document,
      storage: root.localStorage,
      fetch: (url) => root.fetch(url),
      i18n: root.i18n || null,
      changes: root.HosProfileChanges || null,
      makeSession: async () => {
        let identity = null;
        try { identity = typeof root.getPqIdentity === 'function' ? await root.getPqIdentity() : null; } catch (e) { identity = null; }
        if (!identity || typeof root.createRelaySession !== 'function') return null;
        return root.createRelaySession({
          getIdentity: async () => identity,
          signBytes: (secret, bytes) => root.pqSignMessage(secret, bytes),
          WebSocket: root.WebSocket,
          location: root.location,
          storage: root.localStorage,
        });
      },
    });
    section.mount();
    // The chat in another tab used a level or adopted the server's copy of the profile: show it.
    if (typeof root.addEventListener === 'function') {
      root.addEventListener('storage', (e) => {
        if (e && (e.key === P.TIER_KEY || e.key === P.PRESENCE_KEY || e.key === P.PROFILE_KEY)) section.render();
      });
    }
    return section;
  }

  const api = { createPrivacySection };
  if (typeof module === 'object' && module && module.exports) {
    module.exports = api;
  } else {
    root.createPrivacySection = createPrivacySection;
    if (root.document && root.document.readyState === 'loading') root.document.addEventListener('DOMContentLoaded', mountInBrowser);
    else mountInBrowser();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
