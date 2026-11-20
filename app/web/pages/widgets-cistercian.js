// Moved out of web/pages/widgets.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // Cistercian numeral widget. A single glyph encodes 0-9999 (one digit per
    // corner: units top-right, tens top-left, hundreds bottom-right, thousands
    // bottom-left; digit shapes 1-9 by the standard construction, 0 = blank).
    // Past 9999 we chain glyphs in base-10000. Alongside we show the same number
    // in decimal and binary, and a 0-9 symbol key, so the glyph is learned next
    // to systems the reader already knows.
    (function () {
      var SVG = 'http://www.w3.org/2000/svg';
      var input = document.getElementById('cis-input');
      if (!input) return;
      var range = document.getElementById('cis-range');
      var glyphsEl = document.getElementById('cis-glyphs');
      var decEl = document.getElementById('cis-dec');
      var binEl = document.getElementById('cis-bin');
      var partsEl = document.getElementById('cis-parts');
      var legendEl = document.getElementById('cis-legend');
      var tDec = document.getElementById('cis-t-dec');
      var tBin = document.getElementById('cis-t-bin');
      var tKey = document.getElementById('cis-t-key');

      var MAX = 999999999999; // up to three base-10000 glyphs
      var cx = 70, cy = 90, H = 60, A = 42, B = 40; // stave centre, half-height, arm, depth
      var SEG = {
        1: ['h_top'], 2: ['h_bot'], 3: ['d_tl_br'], 4: ['d_bl_tr'],
        5: ['h_top', 'd_bl_tr'], 6: ['v_right'], 7: ['h_top', 'v_right'],
        8: ['h_bot', 'v_right'], 9: ['h_top', 'h_bot', 'v_right'],
      };
      function segPoints(name) {
        if (name === 'h_top') return [0, 0, A, 0];
        if (name === 'h_bot') return [0, B, A, B];
        if (name === 'v_right') return [A, 0, A, B];
        if (name === 'd_tl_br') return [0, 0, A, B];
        if (name === 'd_bl_tr') return [0, B, A, 0];
        return [0, 0, 0, 0];
      }
      var QUAD = {
        units:     { ox: cx, oy: cy - H, sx: 1,  sy: 1 },
        tens:      { ox: cx, oy: cy - H, sx: -1, sy: 1 },
        hundreds:  { ox: cx, oy: cy + H, sx: 1,  sy: -1 },
        thousands: { ox: cx, oy: cy + H, sx: -1, sy: -1 },
      };
      function line(x1, y1, x2, y2, cls) {
        var l = document.createElementNS(SVG, 'line');
        l.setAttribute('x1', x1); l.setAttribute('y1', y1);
        l.setAttribute('x2', x2); l.setAttribute('y2', y2);
        if (cls) l.setAttribute('class', cls);
        return l;
      }
      // Build an <svg> glyph for a value 0-9999 at the given pixel width.
      function glyphSvg(value, px) {
        value = Math.max(0, Math.min(9999, value | 0));
        var svg = document.createElementNS(SVG, 'svg');
        svg.setAttribute('viewBox', '0 0 140 180');
        svg.setAttribute('width', px);
        svg.setAttribute('height', Math.round(px * 180 / 140));
        svg.setAttribute('class', 'cis-svg');
        svg.setAttribute('role', 'img');
        svg.setAttribute('aria-label', 'Cistercian ' + value);
        svg.appendChild(line(cx, cy - H, cx, cy + H, 'stave'));
        var digits = {
          units: value % 10, tens: (value / 10 | 0) % 10,
          hundreds: (value / 100 | 0) % 10, thousands: (value / 1000 | 0) % 10,
        };
        ['units', 'tens', 'hundreds', 'thousands'].forEach(function (q) {
          var d = digits[q]; if (!d) return; var t = QUAD[q];
          SEG[d].forEach(function (s) {
            var p = segPoints(s);
            svg.appendChild(line(t.ox + t.sx * p[0], t.oy + t.sy * p[1],
                                  t.ox + t.sx * p[2], t.oy + t.sy * p[3]));
          });
        });
        return svg;
      }
      // Split n into base-10000 groups, most-significant first.
      function groups10000(n) {
        var g = []; if (n === 0) return [0];
        while (n > 0) { g.unshift(n % 10000); n = Math.floor(n / 10000); }
        return g;
      }
      function fmt(n) { return n.toLocaleString('en-US'); }
      function clear(el) { while (el.firstChild) el.removeChild(el.firstChild); }

      function applyToggles() {
        decEl.style.display = tDec.checked ? '' : 'none';
        binEl.style.display = tBin.checked ? '' : 'none';
        legendEl.style.display = tKey.checked ? '' : 'none';
      }

      function render(n) {
        n = Math.max(0, Math.min(MAX, n | 0));
        // Glyph chain (base-10000, most-significant first).
        clear(glyphsEl);
        var g = groups10000(n);
        g.forEach(function (val, i) {
          var wrap = document.createElement('div'); wrap.className = 'cis-glyph';
          wrap.appendChild(glyphSvg(val, 88));
          if (g.length > 1) {
            var place = document.createElement('div'); place.className = 'place';
            var pow = g.length - 1 - i;
            place.textContent = '×' + fmt(Math.pow(10000, pow)); // x place value
            wrap.appendChild(place);
          }
          glyphsEl.appendChild(wrap);
        });
        // Same number, other systems.
        decEl.innerHTML = 'Decimal <b>' + fmt(n) + '</b>';
        binEl.innerHTML = 'Binary <b>' + n.toString(2) + '</b>';
        // Per-place breakdown of the lowest glyph.
        var low = n % 10000;
        partsEl.innerHTML =
          '<span class="cis-part">thousands <b>' + ((low / 1000 | 0) % 10) + '</b></span>' +
          '<span class="cis-part">hundreds <b>' + ((low / 100 | 0) % 10) + '</b></span>' +
          '<span class="cis-part">tens <b>' + ((low / 10 | 0) % 10) + '</b></span>' +
          '<span class="cis-part">units <b>' + (low % 10) + '</b></span>';
        input.value = n;
        range.value = Math.min(9999, n);
        applyToggles();
      }

      // Symbol key: digits 0-9 as small glyphs (0 = the bare stave).
      function buildLegend() {
        clear(legendEl);
        for (var d = 0; d <= 9; d++) {
          var k = document.createElement('div'); k.className = 'cis-key';
          k.appendChild(glyphSvg(d, 38));
          var lbl = document.createElement('div'); lbl.className = 'd'; lbl.textContent = d;
          k.appendChild(lbl);
          legendEl.appendChild(k);
        }
      }

      input.addEventListener('input', function () { render(parseInt(input.value, 10) || 0); });
      range.addEventListener('input', function () { render(parseInt(range.value, 10) || 0); });
      [tDec, tBin, tKey].forEach(function (t) { t.addEventListener('change', applyToggles); });
      buildLegend();
      render(1988);
    })();
