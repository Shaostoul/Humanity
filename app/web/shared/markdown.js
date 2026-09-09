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

  /** A GFM table separator row: only pipes, dashes, colons and space, and it
      must contain at least one of each of a dash and a pipe. A bare row of
      dashes is a horizontal rule, not a separator, so the pipe test matters. */
  function isSeparatorRow(s) {
    return !!s && s.indexOf('-') >= 0 && s.indexOf('|') >= 0 && /^[\s|:-]+$/.test(s);
  }

  /** Split one pipe row into trimmed cells, tolerating optional outer pipes. */
  function tableCells(row) {
    let s = row.trim();
    if (s.startsWith('|')) s = s.slice(1);
    if (s.endsWith('|')) s = s.slice(0, -1);
    return s.split('|').map(function (c) { return c.trim(); });
  }

  /* Table styling ships with the renderer rather than with each page. The
     .md-viewer rules are already duplicated across accord.html, library.html
     and files.html; adding a fourth copy of table CSS to each would guarantee
     drift, and any page that renders markdown needs these the moment the
     document contains a table. Uses theme variables, so it themes normally. */
  let stylesInjected = false;
  function injectTableStyles() {
    if (stylesInjected || typeof document === 'undefined') return;
    stylesInjected = true;
    const style = document.createElement('style');
    style.id = 'hos-md-table-styles';
    style.textContent =
      '.md-table-wrap{overflow-x:auto;margin:var(--space-md) 0;}' +
      '.md-table-wrap table{border-collapse:collapse;width:100%;font-size:var(--text-sm);}' +
      '.md-table-wrap th,.md-table-wrap td{border:1px solid var(--border);padding:6px 10px;text-align:left;vertical-align:top;}' +
      '.md-table-wrap th{background:var(--bg-card);color:var(--text);font-weight:600;}' +
      '.md-table-wrap td{color:var(--text-muted);}' +
      '.md-table-wrap tr:nth-child(even) td{background:rgba(127,127,127,0.05);}';
    document.head.appendChild(style);
  }

  function render(md) {
    let html = '';
    const lines = md.split('\n');
    let inCode = false;
    let inList = false;
    let listType = '';
    let codeBlock = '';

    /* Two buffers, because markdown joins wrapped source lines and this
       renderer used to emit one element per LINE. That broke over half the
       library: consecutive prose lines became separate ragged paragraphs, an
       indented continuation line silently ended its bullet (42 of 81 documents
       wrap list items, 250 times in ROADMAP.md alone), and inline markup that
       spanned a line break never matched, so humanity_accord.md showed seven
       bold spans as literal asterisks. Flush before emitting anything else. */
    let para = [];
    let liBuf = null;

    function flushPara() {
      if (!para.length) return;
      html += '<p>' + inlineMarkdown(para.join(' ')) + '</p>';
      para = [];
    }
    function flushLi() {
      if (liBuf === null) return;
      html += '<li>' + inlineMarkdown(liBuf.join(' ')) + '</li>';
      liBuf = null;
    }
    function closeList() {
      flushLi();
      if (inList) { html += '</' + listType + '>'; inList = false; }
    }

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // Code blocks.
      if (line.startsWith('```')) {
        flushPara();
        if (inCode) {
          html += '<pre><code>' + escapeHtml(codeBlock) + '</code></pre>';
          codeBlock = '';
          inCode = false;
        } else {
          closeList();
          inCode = true;
        }
        continue;
      }
      if (inCode) {
        codeBlock += (codeBlock ? '\n' : '') + line;
        continue;
      }

      const ulMatch = line.match(/^\s*[-*+]\s+(.*)/);
      const olMatch = line.match(/^\s*\d+\.\s+(.*)/);

      // A wrapped list item: indented, non-blank, and not itself a new bullet.
      // Belongs to the bullet above it, not to a paragraph of its own.
      if (inList && liBuf !== null && !ulMatch && !olMatch && line.trim() && /^\s/.test(line)) {
        liBuf.push(line.trim());
        continue;
      }

      // Anything that is not a list item ends the list.
      if (inList && !ulMatch && !olMatch) closeList();

      // Headers.
      const hMatch = line.match(/^(#{1,6})\s+(.*)/);
      if (hMatch) {
        flushPara();
        const level = hMatch[1].length;
        html += '<h' + level + '>' + inlineMarkdown(hMatch[2]) + '</h' + level + '>';
        continue;
      }

      // Tables (GitHub pipe syntax). A row containing a pipe, immediately
      // followed by a separator row, opens a table that runs until the first
      // line that is blank or carries no pipe. 14 library documents already
      // used tables before this existed and rendered as raw pipe text.
      if (line.indexOf('|') >= 0 && isSeparatorRow(lines[i + 1])) {
        flushPara();
        injectTableStyles();
        // Column alignment from the separator: :--- left, ---: right, :--: center.
        const aligns = tableCells(lines[i + 1]).map(function (s) {
          const l = s.startsWith(':'), r = s.endsWith(':');
          return (l && r) ? 'center' : r ? 'right' : l ? 'left' : '';
        });
        const cell = function (tag, v, k) {
          const a = aligns[k] ? ' style="text-align:' + aligns[k] + '"' : '';
          return '<' + tag + a + '>' + inlineMarkdown(v) + '</' + tag + '>';
        };
        const head = '<tr>' + tableCells(line).map(function (v, k) {
          return cell('th', v, k);
        }).join('') + '</tr>';

        let body = '';
        let n = i + 2;
        for (; n < lines.length; n++) {
          if (!lines[n].trim() || lines[n].indexOf('|') < 0) break;
          body += '<tr>' + tableCells(lines[n]).map(function (v, k) {
            return cell('td', v, k);
          }).join('') + '</tr>';
        }

        html += '<div class="md-table-wrap"><table><thead>' + head +
          '</thead><tbody>' + body + '</tbody></table></div>';
        i = n - 1;   // the outer loop's i++ moves past the last consumed row
        continue;
      }

      // Blockquote.
      if (line.startsWith('> ')) {
        flushPara();
        html += '<blockquote>' + inlineMarkdown(line.slice(2)) + '</blockquote>';
        continue;
      }

      // Horizontal rule. Checked before the list branches so that a "---"
      // is never mistaken for a bullet.
      if (line.match(/^[-*_]{3,}\s*$/)) {
        flushPara();
        closeList();
        html += '<hr>';
        continue;
      }

      // Unordered list.
      if (ulMatch) {
        flushPara();
        flushLi();
        if (!inList || listType !== 'ul') {
          closeList();
          html += '<ul>';
          inList = true;
          listType = 'ul';
        }
        liBuf = [ulMatch[1]];
        continue;
      }

      // Ordered list.
      if (olMatch) {
        flushPara();
        flushLi();
        if (!inList || listType !== 'ol') {
          closeList();
          html += '<ol>';
          inList = true;
          listType = 'ol';
        }
        liBuf = [olMatch[1]];
        continue;
      }

      // Empty line: ends the current paragraph.
      if (!line.trim()) {
        flushPara();
        continue;
      }

      // Prose: accumulate into the paragraph buffer, do not emit yet.
      para.push(line.trim());
    }

    flushPara();
    closeList();
    if (inCode) html += '<pre><code>' + escapeHtml(codeBlock) + '</code></pre>';

    return html;
  }


  window.hosMarkdown = { render, escapeHtml, inlineMarkdown };
})();
