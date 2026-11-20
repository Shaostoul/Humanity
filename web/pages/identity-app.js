// Moved out of web/pages/identity.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    'use strict';
    document.getElementById('lookup-btn').addEventListener('click', doLookup);
    // Show the reader their own DID, derived locally from their key. Nothing
    // is sent anywhere to compute it: a DID IS a hash of your public key.
    (async function showMyDid() {
      var box = document.getElementById('my-did-box');
      if (!box || typeof window.getPqIdentity !== 'function') return;
      var id = null;
      try { id = await window.getPqIdentity(); } catch (e) { /* no identity here */ }
      if (!id || !id.dilithiumPublicHex) return;
      var bytes = new Uint8Array(id.dilithiumPublicHex.length / 2);
      for (var i = 0; i < bytes.length; i++) {
        bytes[i] = parseInt(id.dilithiumPublicHex.substr(i * 2, 2), 16);
      }
      var did = await window.HumOS.pq.did_for_pubkey(bytes);
      document.getElementById('my-did-value').textContent = did;
      box.style.display = '';

      var btn = document.getElementById('my-did-copy');
      btn.addEventListener('click', function () {
        navigator.clipboard.writeText(did).then(function () {
          btn.textContent = 'Copied';
          setTimeout(function () { btn.textContent = 'Copy'; }, 1500);
        });
      });

      // Say plainly whether it resolves yet, because it usually does not, and a
      // DID that 404s looks broken rather than merely unused. A DID resolves
      // against signed objects you have published; a member who has not
      // published one has a perfectly valid DID that no server can look up.
      var note = document.getElementById('my-did-note');
      note.textContent = 'Checking whether this server can resolve it...';
      try {
        var r = await window.HumOS.pq.resolveDid(did);
        note.textContent = r
          ? 'This server can resolve your DID from the signed objects you have published.'
          : 'Not resolvable here yet. A DID is resolved from the signed objects you have published, and you have not published any. Your DID is still valid and still yours: it is a hash of your public key, so it exists whether or not any server has seen it.';
      } catch (e) {
        note.textContent = 'Not resolvable here yet. A DID is resolved from the signed objects you have published, and you have not published any. Your DID is still valid and still yours: it is a hash of your public key, so it exists whether or not any server has seen it.';
      }
    })();

    document.getElementById('did-input').addEventListener('keydown', function(e){
      if (e.key === 'Enter') doLookup();
    });

    async function doLookup() {
      var did = document.getElementById('did-input').value.trim();
      if (!did) return;
      var area = document.getElementById('result-area');
      area.innerHTML = '<div class="empty">Resolving \u2026</div>';
      try {
        var data = await window.HumOS.pq.enrichDid(did);
        renderResult(area, did, data);
      } catch (err) {
        area.innerHTML = '<div class="empty">Lookup failed: ' + escapeHtml(String(err && err.message || err)) + '</div>';
      }
    }

    function renderResult(area, did, data) {
      var res = data.resolution;
      var trust = data.trust;
      var ai = data.aiStatus;
      var creds = data.credentials || [];

      var didCard = '' +
        '<div class="card">' +
          '<h3>DID Resolution</h3>' +
          '<div class="kv"><span class="k">DID</span><span class="v">' + escapeHtml(did) + '</span></div>' +
          (res && !res.error
            ? '<div class="kv"><span class="k">Crypto suite</span><span class="v">' + escapeHtml(res.crypto_suite || '\u2014') + '</span></div>' +
              '<div class="kv"><span class="k">Public key (b64, head)</span><span class="v">' + escapeHtml((res.current_pubkey_b64 || '').slice(0, 32)) + '\u2026</span></div>' +
              '<div class="kv"><span class="k">First seen</span><span class="v">' + (res.first_seen ? new Date(res.first_seen).toLocaleString() : '\u2014') + '</span></div>' +
              '<div class="kv"><span class="k">Last seen</span><span class="v">' + (res.last_seen ? new Date(res.last_seen).toLocaleString() : '\u2014') + '</span></div>' +
              '<div class="kv"><span class="k">Object count</span><span class="v">' + (res.object_count || 0) + '</span></div>'
            : '<div class="empty">Not yet observed by this server.</div>') +
        '</div>';

      var trustCard = '' +
        '<div class="card">' +
          '<h3>Trust Score</h3>' +
          (trust && !trust.error
            ? '<div style="font-size:2rem;font-weight:700;color:var(--accent);text-align:center;">' + (trust.total != null ? trust.total.toFixed(2) : '0.00') + '</div>' +
              '<div class="trust-meter"><div class="fill" style="width:' + ((trust.total || 0) * 100) + '%"></div></div>' +
              '<div class="sub-scores">' + renderSubScores(trust.sub_scores || {}) + '</div>' +
              '<div style="margin-top:12px;font-size:0.78rem;color:var(--text-muted);">Inputs: ' + (trust.inputs ? JSON.stringify(trust.inputs) : '\u2014') + '</div>'
            : '<div class="empty">No trust data.</div>') +
        '</div>';

      var aiClass = (ai && !ai.error) ? (ai.subject_class || 'unknown') : 'unknown';
      var aiCard = '' +
        '<div class="card">' +
          '<h3>Subject Class</h3>' +
          '<div style="text-align:center;margin:8px 0;"><span class="ai-flag ' + aiClass + '">' + aiClass + '</span></div>' +
          (ai && !ai.error
            ? (ai.operator_did
                ? '<div class="kv"><span class="k">Controlled by</span><span class="v">' + escapeHtml(ai.operator_did) + '</span></div>'
                : (aiClass === 'ai_agent'
                    ? '<div class="empty" style="color:#ef4f4f;">AI without controlled_by_v1 binding \u2014 not compliant</div>'
                    : '<div class="empty">No operator binding</div>')) +
              '<div class="kv"><span class="k">Compliant</span><span class="v">' + (ai.compliant ? 'yes' : 'no') + '</span></div>'
            : '<div class="empty">No declaration.</div>') +
        '</div>';

      var credsCard = '' +
        '<div class="card">' +
          '<h3>Verifiable Credentials (' + creds.length + ')</h3>' +
          (creds.length
            ? '<div class="vc-list">' + creds.map(renderVcRow).join('') + '</div>'
            : '<div class="empty">No credentials issued to this DID yet.</div>') +
        '</div>';

      area.innerHTML = '<div class="grid">' + didCard + trustCard + aiCard + credsCard + '</div>';
    }

    function renderSubScores(sub) {
      return Object.entries(sub).map(function(entry){
        var label = entry[0].replace(/_/g, ' ');
        var v = entry[1] || 0;
        return '<div class="sub-row">' +
          '<span class="label">' + escapeHtml(label) + '</span>' +
          '<span class="bar"><span class="fill" style="width:' + (v * 100) + '%"></span></span>' +
          '<span class="pct">' + v.toFixed(2) + '</span>' +
        '</div>';
      }).join('');
    }

    function renderVcRow(c) {
      var cls = (c.revoked || c.withdrawn) ? 'vc-row revoked' : 'vc-row';
      return '<div class="' + cls + '">' +
        '<div class="schema">' + escapeHtml(c.schema_id) + '</div>' +
        '<div class="from">From: ' + escapeHtml(c.issuer_did) + '</div>' +
      '</div>';
    }

    function escapeHtml(s) {
      return String(s).replace(/[&<>"']/g, function(c){
        return ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[c];
      });
    }
