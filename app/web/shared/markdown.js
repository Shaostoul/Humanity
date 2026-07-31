/**
 * HumanityOS shared markdown renderer.
 * Extracted from web/pages/files-app.js (the Files viewer's markdown mode) so
 * a second consumer (the Accord doc browser, web/pages/accord-app.js) can
 * reuse it instead of duplicating the logic. Exposes window.hosMarkdown.render.
 *
 * Known caveat (pre-existing, not introduced here): inlineMarkdown's link
 * handling splices the URL straight into href="..." with no scheme
 * allowlist, so a javascript: URI or an embedded double-quote can execute
 * script or inject an attribute. Both current callers (Files viewer, Accord
 * browser) only render trusted, repo-committed markdown, so this is not an
 * active issue -- but do not reuse this renderer for user-submitted content
 * without fixing that first.
 */
(function () {
  'use strict';

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  function inlineMarkdown(text) {
    let s = escapeHtml(text);
    // Bold.
    s = s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
    s = s.replace(/__(.+?)__/g, '<strong>$1</strong>');
    // Italic.
    s = s.replace(/\*(.+?)\*/g, '<em>$1</em>');
    s = s.replace(/_(.+?)_/g, '<em>$1</em>');
    // Inline code.
    s = s.replace(/`([^`]+)`/g, '<code>$1</code>');
    // Links.
    s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
    return s;
  }

  function render(md) {
    let html = '';
    const lines = md.split('\n');
    let inCode = false;
    let inList = false;
    let listType = '';
    let codeBlock = '';

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // Code blocks.
      if (line.startsWith('```')) {
        if (inCode) {
          html += '<pre><code>' + escapeHtml(codeBlock) + '</code></pre>';
          codeBlock = '';
          inCode = false;
        } else {
          if (inList) { html += '</' + listType + '>'; inList = false; }
          inCode = true;
        }
        continue;
      }
      if (inCode) {
        codeBlock += (codeBlock ? '\n' : '') + line;
        continue;
      }

      // Close list if needed.
      if (inList && !line.match(/^(\s*[-*+]|\s*\d+\.)\s/)) {
        html += '</' + listType + '>';
        inList = false;
      }

      // Headers.
      const hMatch = line.match(/^(#{1,6})\s+(.*)/);
      if (hMatch) {
        const level = hMatch[1].length;
        html += '<h' + level + '>' + inlineMarkdown(hMatch[2]) + '</h' + level + '>';
        continue;
      }

      // Blockquote.
      if (line.startsWith('> ')) {
        html += '<blockquote>' + inlineMarkdown(line.slice(2)) + '</blockquote>';
        continue;
      }

      // Unordered list.
      const ulMatch = line.match(/^\s*[-*+]\s+(.*)/);
      if (ulMatch) {
        if (!inList || listType !== 'ul') {
          if (inList) html += '</' + listType + '>';
          html += '<ul>';
          inList = true;
          listType = 'ul';
        }
        html += '<li>' + inlineMarkdown(ulMatch[1]) + '</li>';
        continue;
      }

      // Ordered list.
      const olMatch = line.match(/^\s*\d+\.\s+(.*)/);
      if (olMatch) {
        if (!inList || listType !== 'ol') {
          if (inList) html += '</' + listType + '>';
          html += '<ol>';
          inList = true;
          listType = 'ol';
        }
        html += '<li>' + inlineMarkdown(olMatch[1]) + '</li>';
        continue;
      }

      // Horizontal rule.
      if (line.match(/^[-*_]{3,}\s*$/)) {
        html += '<hr>';
        continue;
      }

      // Empty line.
      if (!line.trim()) {
        continue;
      }

      // Paragraph.
      html += '<p>' + inlineMarkdown(line) + '</p>';
    }

    if (inCode) html += '<pre><code>' + escapeHtml(codeBlock) + '</code></pre>';
    if (inList) html += '</' + listType + '>';

    return html;
  }

  window.hosMarkdown = { render, escapeHtml, inlineMarkdown };
})();
