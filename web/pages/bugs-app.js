/**
 * HumanityOS Bug Reporter, Client logic
 * Submits bug reports to POST /api/bugs and lists known bugs from GET /api/bugs.
 */
(function () {
  'use strict';

  var API_BASE = '';
  var form = document.getElementById('bug-form');
  var btnSubmit = document.getElementById('btn-submit');
  var formStatus = document.getElementById('form-status');
  var bugList = document.getElementById('bug-list');

  var filterStatus = document.getElementById('filter-status');
  var filterSeverity = document.getElementById('filter-severity');
  var filterCategory = document.getElementById('filter-category');

  // ---- Auto-capture browser info ----
  function getBrowserInfo() {
    var ua = navigator.userAgent;
    var w = window.innerWidth || screen.width;
    var h = window.innerHeight || screen.height;
    return ua + ' | ' + w + 'x' + h;
  }

  function getVersion() {
    // Try to read version from shell.js or page meta
    var el = document.querySelector('.site-footer .version, [data-version]');
    if (el) return el.textContent || el.getAttribute('data-version') || '';
    // Fallback: scan for version pattern in nav
    var nav = document.querySelector('.hub-nav');
    if (nav) {
      var match = nav.textContent.match(/v\d+\.\d+\.\d+/);
      if (match) return match[0];
    }
    return '';
  }

  function getPublicKey() {
    // myIdentity is set by chat/app.js if user is logged in
    if (window.myIdentity && window.myIdentity.publicKeyHex) {
      return window.myIdentity.publicKeyHex;
    }
    // Try localStorage
    var stored = localStorage.getItem('publicKeyHex');
    return stored || '';
  }

  function getDisplayName() {
    if (window.myIdentity && window.myIdentity.displayName) {
      return window.myIdentity.displayName;
    }
    return localStorage.getItem('displayName') || '';
  }

  // ---- Submit form ----
  form.addEventListener('submit', function (e) {
    e.preventDefault();

    var title = document.getElementById('bug-title').value.trim();
    var description = document.getElementById('bug-description').value.trim();
    if (!title || !description) {
      showStatus('Title and description are required.', 'error');
      return;
    }

    btnSubmit.disabled = true;
    showStatus('Submitting...', '');

    var body = {
      title: title,
      description: description,
      steps: document.getElementById('bug-steps').value,
      expected: document.getElementById('bug-expected').value,
      actual: document.getElementById('bug-actual').value,
      severity: document.getElementById('bug-severity').value,
      category: document.getElementById('bug-category').value,
      reporter_key: getPublicKey(),
      reporter_name: getDisplayName(),
      browser_info: getBrowserInfo(),
      page_url: window.location.href,
      version: getVersion()
    };

    fetch(API_BASE + '/api/bugs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
      .then(function (res) {
        if (!res.ok) return res.text().then(function (t) { throw new Error(t); });
        return res.json();
      })
      .then(function (data) {
        showStatus('Bug #' + data.id + ' submitted. Thank you!', 'success');
        form.reset();
        loadBugs();
        setTimeout(function () { btnSubmit.disabled = false; }, 2000);
      })
      .catch(function (err) {
        showStatus('Failed: ' + err.message, 'error');
        btnSubmit.disabled = false;
      });
  });

  function showStatus(msg, type) {
    formStatus.textContent = msg;
    formStatus.className = 'form-status' + (type ? ' ' + type : '');
  }

  // ---- Load and display bugs ----
  function loadBugs() {
    var params = new URLSearchParams();
    if (filterStatus.value) params.set('status', filterStatus.value);
    if (filterSeverity.value) params.set('severity', filterSeverity.value);
    if (filterCategory.value) params.set('category', filterCategory.value);
    params.set('limit', '100');

    fetch(API_BASE + '/api/bugs?' + params.toString())
      .then(function (res) { return res.json(); })
      .then(function (data) {
        renderBugs(data.bugs || []);
      })
      .catch(function () {
        bugList.innerHTML = '<div class="empty-state">Could not load bug reports.</div>';
      });
  }

  function renderBugs(bugs) {
    if (!bugs.length) {
      bugList.innerHTML = '<div class="empty-state">No bug reports found. Everything looks good!</div>';
      return;
    }

    bugList.innerHTML = bugs.map(function (b) {
      var statusLabel = b.status.replace(/_/g, ' ');
      var ago = timeAgo(b.created_at);
      var desc = b.description.length > 150 ? b.description.substring(0, 150) + '...' : b.description;

      return '<div class="bug-card" data-id="' + b.id + '">' +
        '<div class="bug-card-top">' +
          '<span class="badge badge-' + b.severity + '">' + b.severity + '</span>' +
          '<span class="badge badge-' + b.status + '">' + statusLabel + '</span>' +
          '<span class="bug-card-title">' + escapeHtml(b.title) + '</span>' +
        '</div>' +
        '<div class="bug-card-desc">' + escapeHtml(desc) + '</div>' +
        '<div class="bug-card-meta">' +
          '<span>' + (b.category || 'other') + '</span>' +
          '<span>' + ago + '</span>' +
          (b.reporter_name ? '<span>by ' + escapeHtml(b.reporter_name) + '</span>' : '') +
          '<button class="vote-btn" onclick="window.__voteBug(' + b.id + ', this)">' +
            '\u25B2 ' + b.votes +
          '</button>' +
        '</div>' +
      '</div>';
    }).join('');
  }

  // ---- Voting ----
  //
  // POST /api/bugs/{id}/vote takes { voter_key, timestamp, sig } and checks
  // them with verify_signed_actor (src/relay/api.rs): the timestamp must be
  // under 5 minutes old, and sig must be a Dilithium3 signature by voter_key
  // over the bytes "bug_vote\n<timestamp>". getPqSignedAuth('bug_vote')
  // (web/shared/pq-relay-auth.js, loaded by bugs.html together with
  // /chat/pq.js) builds exactly that preimage and returns { key, timestamp,
  // sig }, with key the Dilithium public key hex the relay knows people by.
  // It reads the seed from the backup Chat leaves in localStorage, so it
  // works on browsers without WebCrypto Ed25519 too. It returns null when
  // this device has no such backup.
  var NO_IDENTITY_MSG = 'Open Chat once on this device to sign in, then vote.';

  function signVote() {
    if (typeof window.getPqSignedAuth !== 'function') {
      console.warn('Bug vote: pq-relay-auth.js not loaded');
      return Promise.resolve(null);
    }
    return window.getPqSignedAuth('bug_vote');
  }

  window.__voteBug = function (id, btn) {
    if (btn.disabled) return;
    btn.disabled = true;

    signVote()
      .then(function (auth) {
        if (!auth) {
          alert(NO_IDENTITY_MSG);
          return null;
        }
        return fetch(API_BASE + '/api/bugs/' + id + '/vote', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            voter_key: auth.key,
            timestamp: auth.timestamp,
            sig: auth.sig
          })
        }).then(function (res) {
          // Errors come back as plain text, not JSON.
          if (!res.ok) return res.text().then(function (t) { throw new Error(t || ('HTTP ' + res.status)); });
          return res.json();
        });
      })
      .then(function (data) {
        if (!data) return;
        // voted is false only when this key had already voted for the bug;
        // either way this person's vote is counted, so show it as theirs.
        btn.classList.add('voted');
        btn.innerHTML = '\u25B2 ' + data.votes;
      })
      .catch(function (err) {
        alert('Vote failed: ' + (err && err.message ? err.message : 'network error'));
      })
      .then(function () { btn.disabled = false; });
  };

  // ---- Utilities ----
  function escapeHtml(str) {
    var div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  function timeAgo(ms) {
    var diff = Date.now() - ms;
    var s = Math.floor(diff / 1000);
    if (s < 60) return 'just now';
    var m = Math.floor(s / 60);
    if (m < 60) return m + 'm ago';
    var h = Math.floor(m / 60);
    if (h < 24) return h + 'h ago';
    var d = Math.floor(h / 24);
    if (d < 30) return d + 'd ago';
    return Math.floor(d / 30) + 'mo ago';
  }

  // ---- Filter listeners ----
  filterStatus.addEventListener('change', loadBugs);
  filterSeverity.addEventListener('change', loadBugs);
  filterCategory.addEventListener('change', loadBugs);

  // ---- Init ----
  loadBugs();
})();
