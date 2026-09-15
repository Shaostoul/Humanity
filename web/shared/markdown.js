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
    // An underscore INSIDE a word never opens or closes emphasis. This is the
    // GFM rule, and without it the Library mangles its own subject matter:
    // `what_soil_is.md`, `silverdale_wa`, `poison_hemlock`, `game_join` and
    // every other snake_case identifier lost its underscores and went italic
    // from the second underscore to the next one. 82 spans across 23 shipped
    // documents, and the native reader showed all of them correctly, so this
    // was a web-only divergence a reader could only find by comparing clients.
    // Guarded by scripts/check-library-render.js.
    s = s.replace(/(^|[^A-Za-z0-9_])__([^_\s](?:[^_]*[^_\s])?)__(?![A-Za-z0-9_])/g,
      '$1<strong>$2</strong>');
    // Italic.
    s = s.replace(/\*(.+?)\*/g, '<em>$1</em>');
    s = s.replace(/(^|[^A-Za-z0-9_])_([^_\s](?:[^_]*[^_\s])?)_(?![A-Za-z0-9_])/g,
      '$1<em>$2</em>');
    // Inline code.
    s = s.replace(/`([^`]+)`/g, '<code>$1</code>');
    // Links.
    s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
    return s;
  }

  /** Reduce inline markdown to the plain text a reader sees. Used for heading
      anchors and outlines, where markup in the source must not reach the slug.
      Mirrors strip_md in src/gui/widgets/markdown.rs. */
  function stripInline(text) {
    return String(text)
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/\*\*/g, '')
      .replace(/__/g, '')
      .replace(/`/g, '')
      .replace(/\*/g, '')
      .trim();
  }

  /** GitHub's heading-anchor grammar: lowercase, every run of non-alphanumerics
      becomes one hyphen, ends trimmed. MUST match heading_slug in
      src/gui/widgets/markdown.rs, because a link written in one client is
      followed in the other. */
  function headingSlug(text) {
    let out = '', dash = false;
    const s = String(text);
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (/[\p{L}\p{N}]/u.test(c)) { out += c.toLowerCase(); dash = false; }
      else if (!dash && out.length) { out += '-'; dash = true; }
    }
    return out.replace(/-+$/, '');
  }

  /** GitHub's duplicate-anchor rule: the second heading that slugs to
      `section-1` becomes `section-1-1`, the third `section-1-2`. The US
      Constitution has nine repeated "Section N" headings, one set per Article;
      without this the outline sends a reader looking for Article III's Section 1
      to Article I's. Mirrors dedupe_slug in src/gui/widgets/markdown.rs. */
  function dedupeSlug(seen, base) {
    const n = seen[base] || 0;
    seen[base] = n + 1;
    return n === 0 ? base : base + '-' + n;
  }

  /** Every heading in `md` as {level, text, slug, line}, in document order. Code
      fences are skipped, because a `# comment` inside a shell example is not a
      section. */
  function headings(md) {
    const out = [];
    const seen = {};
    let inCode = false;
    String(md).split('\n').forEach(function (line, n) {
      const t = line.trim();
      if (t.startsWith('```') || t.startsWith('~~~')) { inCode = !inCode; return; }
      if (inCode) return;
      const m = t.match(/^(#{1,6})\s+(.*)/);
      if (!m) return;
      const text = stripInline(m[2]);
      const base = headingSlug(text);
      if (base) out.push({ level: m[1].length, text: text, slug: dedupeSlug(seen, base), line: n });
    });
    return out;
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
    // An HTML comment is not content and must never reach the reader. Library
    // documents carry machine-readable markers in comments (the `quote-ok`
    // licensing marker, for one), and with nothing stripping them a reader
    // opening the guide saw the literal marker line sitting in the Sources
    // section. Stripped here rather than in the build, so it holds for every
    // document from every source.
    md = String(md).replace(/<!--[\s\S]*?-->/g, '');

    // Per-render duplicate-anchor counter, so the Nth repeat of a heading
    // name gets the same id here that headings() gives it in the outline.
    const seenSlugs = {};
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
    let quote = [];

    // A block quote spans consecutive "> " lines and is ONE quote, not one per
    // line. The Constitution's ratification notes are four-line quotes, which
    // the per-line version rendered as four stacked quote boxes.
    function flushQuote() {
      if (!quote.length) return;
      html += '<blockquote>' + inlineMarkdown(quote.join(' ').trim()) + '</blockquote>';
      quote = [];
    }
    function flushPara() {
      // Every branch that ends a paragraph also ends a quote, so folding the
      // quote flush in here means no other call site needs to change.
      flushQuote();
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
      const fenced = line.trim();
      if (fenced.startsWith('```') || fenced.startsWith('~~~')) {
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
      const hMatch = line.trim().match(/^(#{1,6})\s+(.*)/);
      if (hMatch) {
        flushPara();
        const level = hMatch[1].length;
        // The anchor is what makes a section addressable: the Library's
        // Contents outline, a search hit that knows which section it matched,
        // and a /library#doc/heading deep link all resolve to this id.
        const slug = dedupeSlug(seenSlugs, headingSlug(stripInline(hMatch[2])));
        const idAttr = slug ? ' id="' + escapeHtml(slug) + '"' : '';
        html += '<h' + level + idAttr + '>' + inlineMarkdown(hMatch[2]) + '</h' + level + '>';
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

      // Blockquote. Accumulate; a run of "> " lines is one quote. Matches a
      // bare ">" too, which is how a blank line inside a quote is written.
      if (line.startsWith('>')) {
        // End a pending paragraph, but do NOT flush the quote being built, so
        // this cannot go through flushPara().
        if (para.length) {
          html += '<p>' + inlineMarkdown(para.join(' ')) + '</p>';
          para = [];
        }
        quote.push(line.replace(/^>\s?/, ''));
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


  window.hosMarkdown = { render, escapeHtml, inlineMarkdown, headings, headingSlug, stripInline };
})();
