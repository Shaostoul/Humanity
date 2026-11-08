/*
 * The website's privacy page (/privacy, 2026-11-07, docs/design/privacy-notice.md).
 *
 * It draws data/library/privacy_policy.md, the same file the desktop app's Library shows on its
 * "Your Privacy" shelf (the source is docs/outreach/privacy_policy.md; scripts/build-library.js
 * copies it), with the website Library's markdown renderer (/shared/markdown.js). One text, two
 * windows onto it: the policy can only be true in one place.
 */
(function () {
  'use strict';
  var SRC = '/data/library/privacy_policy.md';
  var box = document.getElementById('privacy-text');
  if (!box) return;
  var t = function (key) { return window.i18n && window.i18n.t ? window.i18n.t(key) : key; };

  function failed() {
    box.textContent = '';
    var p = document.createElement('p');
    p.className = 'pp-note';
    p.textContent = t('privacy_page.load_failed') + ' ';
    var a = document.createElement('a');
    a.href = 'https://github.com/Shaostoul/Humanity/blob/main/docs/outreach/privacy_policy.md';
    a.target = '_blank';
    a.rel = 'noopener';
    a.textContent = t('privacy_page.on_github');
    a.setAttribute('data-tip-desc', t('privacy_page.on_github_hint'));
    p.appendChild(a);
    box.appendChild(p);
  }

  fetch(SRC, { cache: 'no-cache' })
    .then(function (r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.text(); })
    .then(function (md) {
      if (!window.hosMarkdown || !window.hosMarkdown.render) throw new Error('no renderer');
      // Trusted text: a repo-committed file, never anything a visitor wrote (markdown.js's header).
      box.innerHTML = window.hosMarkdown.render(md);
    })
    .catch(failed);
})();
