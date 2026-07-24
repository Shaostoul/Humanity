/**
 * Wallet Guide, renders step-by-step wallet education sections.
 * Content lives in data/wallet/guide.json (infinite-of-x: edit the JSON, not
 * this file; nginx serves it at /data/wallet/guide.json). This file is only
 * the renderer: table of contents, sections, and the glossary grid composed
 * from the terms pairs.
 */
(function () {
  'use strict';

  function esc(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function render(data) {
    var tocList = document.getElementById('toc-list');
    var container = document.getElementById('guide-sections');
    if (!tocList || !container) return;

    var sections = data.sections || [];
    var tocHtml = '';
    var sectionHtml = '';

    for (var i = 0; i < sections.length; i++) {
      var s = sections[i];
      tocHtml += '<li><a href="#' + s.id + '"><span class="toc-num">' + (i + 1) + '.</span>' + s.title.replace(/^\d+\.\s*/, '') + '</a></li>';
      sectionHtml += '<div class="guide-section" id="' + s.id + '">';
      sectionHtml += '<h2>' + s.title + '</h2>';
      sectionHtml += s.html; // trusted first-party content from our own data file
      sectionHtml += '</div>';
    }

    // Glossary section, composed from the terms pairs (kept as data so they
    // can later merge into data/glossary.json).
    var g = data.glossary;
    var terms = data.terms || [];
    if (g && terms.length) {
      var n = sections.length + 1;
      tocHtml += '<li><a href="#' + g.id + '"><span class="toc-num">' + n + '.</span>' + g.title.replace(/^\d+\.\s*/, '') + '</a></li>';
      var gh = '<div class="guide-section" id="' + g.id + '"><h2>' + g.title + '</h2>';
      gh += g.intro_html;
      gh += '<div class="glossary-grid">';
      for (var t = 0; t < terms.length; t++) {
        gh += '<dl class="glossary-term"><dt>' + esc(terms[t][0]) + '</dt><dd>' + esc(terms[t][1]) + '</dd></dl>';
      }
      gh += '</div></div>';
      sectionHtml += gh;
    }

    tocList.innerHTML = tocHtml;
    container.innerHTML = sectionHtml;

    // Smooth scroll for TOC links
    tocList.addEventListener('click', function (e) {
      var link = e.target.closest('a');
      if (!link) return;
      e.preventDefault();
      var target = document.querySelector(link.getAttribute('href'));
      if (target) {
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  }

  function boot() {
    fetch('/data/wallet/guide.json')
      .then(function (r) {
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.json();
      })
      .then(render)
      .catch(function (err) {
        var container = document.getElementById('guide-sections');
        if (container) {
          container.innerHTML = '<p class="muted">Could not load the guide content (' + esc(err.message) + '). It lives at /data/wallet/guide.json; try reloading.</p>';
        }
      });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
})();
