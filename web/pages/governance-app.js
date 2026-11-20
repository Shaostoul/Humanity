// Moved out of web/pages/governance.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    'use strict';
    let activeScope = '';
    let activeFilter = 'open';

    document.querySelectorAll('.scope-tab').forEach(function(btn){
      btn.addEventListener('click', function(){
        document.querySelectorAll('.scope-tab').forEach(function(b){ b.classList.remove('active'); });
        btn.classList.add('active');
        activeScope = btn.dataset.scope;
        loadProposals();
      });
    });

    document.querySelectorAll('.filter-pill').forEach(function(btn){
      btn.addEventListener('click', function(){
        document.querySelectorAll('.filter-pill').forEach(function(b){ b.classList.remove('active'); });
        btn.classList.add('active');
        activeFilter = btn.dataset.filter;
        loadProposals();
      });
    });

    function fmtTime(ms) {
      try { return new Date(ms).toLocaleString(); } catch(_) { return String(ms); }
    }

    function tallyHtml(tally) {
      const total = tally.total_weight || 1;
      return '<div class="tally-bar">' +
          '<div class="yes" style="width:' + (tally.yes_weight / total * 100) + '%"></div>' +
          '<div class="no" style="width:' + (tally.no_weight / total * 100) + '%"></div>' +
          '<div class="abstain" style="width:' + (tally.abstain_weight / total * 100) + '%"></div>' +
        '</div>' +
        '<div class="tally-numbers">' +
          '<span class="yes">' + i18n.t('governance.tally_yes', { n: tally.yes_weight.toFixed(2) }) + '</span>' +
          '<span class="no">' + i18n.t('governance.tally_no', { n: tally.no_weight.toFixed(2) }) + '</span>' +
          '<span>' + i18n.t('governance.tally_abstain', { n: tally.abstain_weight.toFixed(2) }) + '</span>' +
          '<span>' + i18n.tn('governance.voters', tally.vote_count) + '</span>' +
        '</div>';
    }

    /* ── Proposal titles ──────────────────────────────────────────────────
     * /api/v2/proposals returns the INDEX row: type, scope, dates, DID and
     * object id. The title and body a person actually reads live only inside
     * the signed CBOR payload, so this page used to head each card with the
     * proposal_type ("local_rule") and fill its body with an object id. Native
     * has always decoded them (payload_texts in src/gui/pages/governance.rs);
     * only this page did not.
     *
     * Fixed on the CLIENT with ONE extra bulk request, not by teaching the
     * relay to decode payloads in its proposals query: that query runs on the
     * single writer connection, so a blob read plus CBOR decode per proposal
     * would sit on the relay's only write path behind a public endpoint.
     * The market pages already join signed payloads exactly this way.
     */
    var _payloadCbor = null;
    async function cborMod() {
      if (!_payloadCbor) _payloadCbor = await import('/shared/canonical-cbor.js');
      return _payloadCbor;
    }
    function b64Bytes(b64) {
      var bin = atob(String(b64 || ''));
      var out = new Uint8Array(bin.length);
      for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
      return out;
    }
    /** object_id -> {title, body} for every proposal object, best effort. */
    async function loadProposalTexts() {
      var texts = {};
      try {
        var cbor = await cborMod();
        var rows = await window.HumOS.pq.listObjects({ type: 'proposal_v1', limit: 500 });
        (rows || []).forEach(function (row) {
          var id = row.object_id || row.id;
          if (!id || !row.payload_b64) return;
          try {
            var m = cbor.decodeCanonicalCbor(b64Bytes(row.payload_b64));
            texts[id] = {
              title: m && typeof m.title === 'string' ? m.title : '',
              body: m && typeof m.body === 'string' ? m.body : '',
            };
          } catch (_e) { /* one unreadable payload must not blank the page */ }
        });
      } catch (e) {
        // A federated relay may not serve the objects query. Cards then fall
        // back to the type heading, which is what they showed before.
        console.warn('governance: could not load proposal texts', e);
      }
      return texts;
    }

    async function loadProposals() {
      var list = document.getElementById('proposal-list');
      list.innerHTML = '<div class="empty-state">' + i18n.t('governance.loading') + '</div>';
      try {
        var opts = { limit: 100 };
        if (activeScope) opts.scope = activeScope;
        if (activeFilter === 'open') opts.onlyOpen = true;
        var propsAndTexts = await Promise.all([
          window.HumOS.pq.listProposals(opts),
          loadProposalTexts(),
        ]);
        var props = propsAndTexts[0];
        var texts = propsAndTexts[1];
        if (!props || !props.length) {
          list.innerHTML = '<div class="empty-state">' +
            i18n.t('governance.web_empty', { button: '<strong>' + escapeHtml(i18n.t('governance.new_proposal')) + '</strong>' }) +
            '</div>';
          return;
        }
        list.innerHTML = '';
        for (const p of props) {
          let tally = { yes_weight: 0, no_weight: 0, abstain_weight: 0, total_weight: 0, vote_count: 0 };
          try { tally = await window.HumOS.pq.getProposalTally(p.proposal_object_id); } catch(_) {}
          const isOpen = Number(p.closes_at) > Date.now();
          const card = document.createElement('div');
          card.className = 'proposal-card';
          card.dataset.pid = p.proposal_object_id;
          const t = texts[p.proposal_object_id] || {};
          // Fall back to the type heading when a payload could not be read, so
          // a card is never blank; the type then reads as the label it always was.
          const heading = t.title ? t.title : p.proposal_type;
          card.innerHTML =
            '<h3>' + escapeHtml(heading) + '</h3>' +
            '<div class="proposal-meta">' +
              '<span class="scope-badge ' + p.scope + '">' + p.scope + '</span>' +
              (t.title ? ' <span class="prop-type-tag">' + escapeHtml(p.proposal_type) + '</span>' : '') +
              ' ' + i18n.t('governance.proposer_label') + ' <code>' + escapeHtml(p.proposer_did.slice(0, 24)) + '...</code>' +
              ' \u2022 ' + i18n.t('governance.closes_at', { time: fmtTime(p.closes_at) }) +
            '</div>' +
            '<div class="proposal-body">' +
              (t.body
                ? escapeHtml(t.body)
                : '<em>' + i18n.t('governance.no_text') + '</em>') +
            '</div>' +
            '<div class="proposal-id">object_id: <code>' + // i18n-exempt: the wire format's field name, shown as code
              escapeHtml(p.proposal_object_id.slice(0, 16)) + '...</code></div>' +
            '<div class="tally-wrap">' + tallyHtml(tally) + '</div>' +
            (isOpen
              ? '<div class="vote-actions">' +
                  '<button class="btn-vote" data-choice="yes" data-id="' + escapeHtml(p.proposal_object_id) + '" data-i18n-tip="governance.vote_yes_hint">' + i18n.t('governance.vote_yes') + '</button>' +
                  '<button class="btn-vote" data-choice="no" data-id="' + escapeHtml(p.proposal_object_id) + '" data-i18n-tip="governance.vote_no_hint">' + i18n.t('governance.vote_no') + '</button>' +
                  '<button class="btn-vote" data-choice="abstain" data-id="' + escapeHtml(p.proposal_object_id) + '" data-i18n-tip="governance.abstain_hint">' + i18n.t('governance.abstain') + '</button>' +
                '</div>' +
                '<div class="vote-status">' + i18n.t('governance.votes_final') + '</div>'
              : '<div class="vote-status">' + i18n.t('governance.voting_closed') + '</div>');
          i18n.apply(card);
          list.appendChild(card);
        }
        list.querySelectorAll('.btn-vote').forEach(function(b){
          b.addEventListener('click', function(){
            castVote(b.dataset.id, b.dataset.choice, b.closest('.proposal-card'));
          });
        });
        applyMyVotes();
      } catch (err) {
        list.innerHTML = '<div class="empty-state">' +
          escapeHtml(i18n.t('governance.load_failed', { error: String(err && err.message || err) })) + '</div>';
      }
    }

    /* \u2500\u2500 Real voting: Dilithium3-signed vote_v1 objects \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
     * Path: seed from localStorage (pq-relay-auth.js getPqIdentity) \u2192
     * KAT-locked Dilithium3 derivation (/chat/pq.js) \u2192 canonical CBOR
     * object build (pq-object.js, locked byte-for-byte to the Rust relay
     * by `just vote-kat`) \u2192 POST /api/v2/objects. The relay verifies the
     * signature and tallies the FIRST vote per (proposal, voter). */

    let _pqmods = null;
    async function pqMods() {
      if (_pqmods) return _pqmods;
      const [obj, noble] = await Promise.all([
        import('/shared/pq-object.js'),
        import('/shared/vendor/noble-pq.bundle.js'),
      ]);
      if (!noble.blake3) throw new Error('post-quantum bundle failed to load');
      const blake3 = function(d){ return noble.blake3.create({ dkLen: 32 }).update(d).digest(); };
      _pqmods = { obj: obj, blake3: blake3 };
      return _pqmods;
    }

    function hexToBytes(hex) {
      const a = new Uint8Array(hex.length / 2);
      for (let i = 0; i < a.length; i++) a[i] = parseInt(hex.substr(i * 2, 2), 16);
      return a;
    }

    // Cached identity promise: resolves { id, pub, fp } or null. fp = first
    // 16 bytes of BLAKE3(Dilithium pubkey) hex, the same author fingerprint
    // the relay stores, so my-vote tracking is keyed per identity. Only a
    // SUCCESSFUL load is cached: if the user signs in via Chat in another
    // tab and clicks vote again, we re-check instead of staying stuck on
    // "sign in first".
    let _me = null;
    function myIdentity() {
      if (!_me) {
        _me = (async function(){
          if (typeof window.getPqIdentity !== 'function') return null;
          const id = await window.getPqIdentity();
          if (!id) return null;
          const mods = await pqMods();
          const pub = hexToBytes(id.dilithiumPublicHex);
          const h = mods.blake3(pub);
          let fp = '';
          for (let i = 0; i < 16; i++) fp += h[i].toString(16).padStart(2, '0');
          return { id: id, pub: pub, fp: fp };
        })().catch(function(e){
          console.warn('governance: identity load failed', e);
          return null;
        });
        _me.then(function(r){ if (!r) _me = null; });
      }
      return _me;
    }

    // Votes are final; remember mine (per identity) so a reload keeps the label.
    function readMyVotes(fp) {
      try { return JSON.parse(localStorage.getItem('hum_gov_votes_' + fp) || '{}'); }
      catch (_e) { return {}; }
    }
    function recordMyVote(fp, proposalId, choice) {
      const m = readMyVotes(fp);
      m[proposalId] = { choice: choice, at: Date.now() };
      try { localStorage.setItem('hum_gov_votes_' + fp, JSON.stringify(m)); } catch (_e) {}
    }

    /** The words for a vote choice ("yes", "no", "abstain" on the wire). */
    function choiceWord(choice) {
      if (choice === 'yes') return i18n.t('governance.choice_yes');
      if (choice === 'no') return i18n.t('governance.choice_no');
      if (choice === 'abstain') return i18n.t('governance.choice_abstain');
      return String(choice);
    }

    function markVoted(card, choice) {
      card.querySelectorAll('.btn-vote').forEach(function(b){
        b.disabled = true;
        if (b.dataset.choice === choice) b.classList.add('voted-' + choice);
      });
      const st = card.querySelector('.vote-status');
      if (st) {
        st.textContent = i18n.t('governance.web_your_vote', { choice: choiceWord(choice) });
        st.className = 'vote-status ok';
      }
    }

    async function applyMyVotes() {
      const me = await myIdentity();
      if (!me) return;
      const votes = readMyVotes(me.fp);
      document.querySelectorAll('.proposal-card').forEach(function(card){
        const v = votes[card.dataset.pid];
        if (v && v.choice) markVoted(card, v.choice);
      });
    }

    async function castVote(proposalId, choice, card) {
      const st = card.querySelector('.vote-status');
      const buttons = card.querySelectorAll('.btn-vote');
      buttons.forEach(function(b){ b.disabled = true; });
      st.className = 'vote-status';
      st.textContent = i18n.t('governance.checking_identity');
      try {
        const me = await myIdentity();
        if (!me) {
          st.textContent = i18n.t('governance.web_sign_in_vote');
          st.className = 'vote-status err';
          buttons.forEach(function(b){ b.disabled = false; });
          return;
        }
        st.textContent = i18n.t('governance.signing_vote');
        const mods = await pqMods();
        const built = await mods.obj.buildVoteV1({
          proposalId: proposalId,
          choice: choice,
          authorPublicKey: me.pub,
          sign: async function(bytes){
            const sig = await window.pqSignMessage(me.id.dilithiumSecret, bytes);
            if (!sig) throw new Error('Dilithium signing failed (post-quantum bundle unavailable)');
            return sig;
          },
          blake3: mods.blake3,
        });
        st.textContent = i18n.t('governance.submitting_vote');
        await window.HumOS.pq.postObject(built.submission);
        recordMyVote(me.fp, proposalId, choice);
        markVoted(card, choice);
        refreshTally(proposalId, card);
      } catch (err) {
        st.textContent = i18n.t('governance.vote_failed', { error: String(err && err.message || err) });
        st.className = 'vote-status err';
        buttons.forEach(function(b){ b.disabled = false; });
      }
    }

    async function refreshTally(proposalId, card) {
      try {
        const t = await window.HumOS.pq.getProposalTally(proposalId);
        const wrap = card.querySelector('.tally-wrap');
        if (wrap && t) wrap.innerHTML = tallyHtml(t);
      } catch (_e) { /* tally refresh is best-effort; the vote itself succeeded */ }
    }

    /* ── New proposal: Dilithium3-signed proposal_v1 objects ────────
     * Web mirror of the native form (src/gui/pages/governance.rs, the
     * "New proposal" card + build_proposal). Same identity path, same builder
     * shape and the same canonical CBOR payload the relay indexes in
     * storage/governance.rs::index_proposal, so a proposal made here is
     * indistinguishable from one made in the app. */

    // Mirrors src/gui/pages/governance.rs::PROPOSAL_TYPES. The relay's full
    // authored set is data/governance/proposal_types.ron (9 kinds, each with
    // its own quorum and pass thresholds), but no endpoint serves that file to
    // a browser, so both clients offer the same curated three. If a
    // proposal-types endpoint ever lands, both clients should read it instead.
    const PROPOSAL_TYPES = [
      ['local_rule', i18n.t('governance.type_local_rule')],
      ['parameter_change', i18n.t('governance.type_parameter_change')],
      ['accord_amendment', i18n.t('governance.type_accord_amendment')],
    ];
    // Mirrors src/gui/pages/governance.rs::SCOPES (and the scope tabs above).
    const PROPOSAL_SCOPES = [
      ['local', i18n.t('governance.scope_local')],
      ['civilization', i18n.t('governance.scope_civilization')],
    ];
    const TITLE_MAX = 200;
    const BODY_MAX = 8000;
    // The days input offers 1 to 30, the same range as the native slider. This
    // is the backstop for a value typed past the spinner, not the normal range.
    const WINDOW_MAX_DAYS = 365;

    const proposeForm = document.getElementById('propose-form');
    const proposeToggle = document.getElementById('toggle-propose');
    const proposeStatus = document.getElementById('propose-status');
    const proposeNote = document.getElementById('propose-identity-note');
    const proposeSubmit = document.getElementById('prop-submit');
    const fTitle = document.getElementById('prop-title');
    const fBody = document.getElementById('prop-body');
    const fType = document.getElementById('prop-type');
    const fScope = document.getElementById('prop-scope');
    const fDays = document.getElementById('prop-days');

    function fillSelect(sel, pairs) {
      pairs.forEach(function(pair){
        const opt = document.createElement('option');
        opt.value = pair[0];
        opt.textContent = pair[1];
        sel.appendChild(opt);
      });
    }
    fillSelect(fType, PROPOSAL_TYPES);
    fillSelect(fScope, PROPOSAL_SCOPES);

    function setProposeStatus(text, kind) {
      proposeStatus.textContent = text || '';
      proposeStatus.className = 'vote-status' + (kind ? ' ' + kind : '');
    }

    // Gate the submit button on an identity, using the SAME cached myIdentity()
    // path the vote buttons use. Re-run every time the form is opened: a user
    // who signs in via Chat in another tab should not have to reload this one
    // (myIdentity only caches successes, so a re-check is cheap and truthful).
    async function refreshProposeIdentity() {
      proposeSubmit.disabled = true;
      proposeNote.hidden = false;
      proposeNote.className = 'propose-note';
      proposeNote.textContent = i18n.t('governance.checking_identity');
      const me = await myIdentity();
      if (!me) {
        proposeNote.className = 'propose-note warn';
        proposeNote.textContent = i18n.t('governance.web_sign_in_propose');
        return false;
      }
      proposeNote.hidden = true;
      proposeNote.textContent = '';
      proposeSubmit.disabled = false;
      return true;
    }

    proposeToggle.addEventListener('click', function(){
      const opening = proposeForm.hidden;
      proposeForm.hidden = !opening;
      proposeToggle.textContent = opening ? i18n.t('governance.close_form') : i18n.t('governance.new_proposal');
      if (opening) {
        setProposeStatus('');
        refreshProposeIdentity();
        fTitle.focus();
      }
    });

    // Returns { title, body, type, scope, days } or null (after showing why).
    function readProposalForm() {
      const title = fTitle.value.trim();
      const body = fBody.value.trim();
      const days = Number(fDays.value);
      if (!title) { setProposeStatus(i18n.t('governance.err_title_missing'), 'err'); fTitle.focus(); return null; }
      if (title.length > TITLE_MAX) {
        setProposeStatus(i18n.t('governance.err_title_long', { n: title.length, max: TITLE_MAX }), 'err');
        fTitle.focus();
        return null;
      }
      if (!body) { setProposeStatus(i18n.t('governance.err_body_missing'), 'err'); fBody.focus(); return null; }
      if (body.length > BODY_MAX) {
        setProposeStatus(i18n.t('governance.err_body_long', { n: body.length, max: BODY_MAX }), 'err');
        fBody.focus();
        return null;
      }
      if (!Number.isFinite(days) || days <= 0) {
        setProposeStatus(i18n.t('governance.err_days'), 'err');
        fDays.focus();
        return null;
      }
      if (days > WINDOW_MAX_DAYS) {
        setProposeStatus(i18n.t('governance.err_days_max', { max: WINDOW_MAX_DAYS }), 'err');
        fDays.focus();
        return null;
      }
      return { title: title, body: body, type: fType.value, scope: fScope.value, days: days };
    }

    proposeForm.addEventListener('submit', function(ev){
      ev.preventDefault();
      submitProposal();
    });

    async function submitProposal() {
      const fields = readProposalForm();
      if (!fields) return;
      proposeSubmit.disabled = true;
      setProposeStatus(i18n.t('governance.checking_identity'));
      try {
        const me = await myIdentity();
        if (!me) {
          setProposeStatus(i18n.t('governance.web_sign_in_propose'), 'err');
          // Leaves the button disabled with the sign-in note: there is nothing
          // to retry until an identity exists.
          await refreshProposeIdentity();
          return;
        }
        setProposeStatus(i18n.t('governance.signing_proposal'));
        const mods = await pqMods();
        const built = await mods.obj.buildProposalV1({
          proposalType: fields.type,
          scope: fields.scope,
          title: fields.title,
          body: fields.body,
          windowDays: fields.days,
          authorPublicKey: me.pub,
          sign: async function(bytes){
            const sig = await window.pqSignMessage(me.id.dilithiumSecret, bytes);
            if (!sig) throw new Error('Dilithium signing failed (post-quantum bundle unavailable)');
            return sig;
          },
          blake3: mods.blake3,
        });
        setProposeStatus(i18n.t('governance.submitting_proposal'));
        await window.HumOS.pq.postObject(built.submission);
        setProposeStatus(i18n.t('governance.proposal_submitted'), 'ok');
        fTitle.value = '';
        fBody.value = '';
        proposeForm.hidden = true;
        proposeToggle.textContent = i18n.t('governance.new_proposal');
        proposeSubmit.disabled = false;
        // The new proposal is open, so it shows under either filter.
        loadProposals();
      } catch (err) {
        setProposeStatus(i18n.t('governance.proposal_failed', { error: String(err && err.message || err) }), 'err');
        proposeSubmit.disabled = false;
      }
    }

    function escapeHtml(s) {
      return String(s).replace(/[&<>"']/g, function(c){
        return ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[c];
      });
    }

    loadProposals();
