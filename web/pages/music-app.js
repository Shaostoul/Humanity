// The Music page's web mirror (docs/design/music-page.md section 11): favourites kept as share
// codes in this browser (localStorage `hos_music_favourites`), each read to show its name, genre,
// mood and seed, with Copy code and Remove. The reader follows the desktop app's
// crates/humanity-music/src/favourite.rs exactly: "HOSM1-", then Crockford base32 of the seed as a
// variable-length number, the tuning, the genre, the mood and the name, and a 16-bit checksum.
(function () {
  'use strict';
  const KEY = 'hos_music_favourites';
  const PREFIX = 'HOSM1-';
  const B32 = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

  function b32decode(text) {
    const out = [];
    let acc = 0, bits = 0;
    for (let c of text.toUpperCase()) {
      if (c === '-' || /\s/.test(c)) continue;
      if (c === 'I' || c === 'L') c = '1';
      if (c === 'O') c = '0';
      const v = B32.indexOf(c);
      if (v < 0) return null;
      acc = ((acc << 5) | v) & 0xffff;
      bits += 5;
      if (bits >= 8) { bits -= 8; out.push((acc >> bits) & 0xff); }
    }
    return out;
  }

  function check16(bytes) {
    let h = 0x811c9dc5;
    for (const b of bytes) { h ^= b; h = Math.imul(h, 0x01000193) >>> 0; }
    return ((h ^ (h >>> 16)) & 0xffff) >>> 0;
  }

  // Read a code to {name, genre, mood, seed} (seed as a string: it may pass 2^53), or null.
  function read(code) {
    code = (code || '').trim();
    if (code.slice(0, PREFIX.length).toUpperCase() !== PREFIX) return null;
    const bytes = b32decode(code.slice(PREFIX.length));
    if (!bytes || bytes.length < 12) return null;
    const data = bytes.slice(0, -2);
    const sum = bytes[bytes.length - 2] | (bytes[bytes.length - 1] << 8);
    if (check16(data) !== sum) return null;
    let i = 0;
    const u8 = () => { if (i >= data.length) throw new Error('short'); return data[i++]; };
    const str = () => { const n = u8(); if (i + n > data.length) throw new Error('short'); const s = new TextDecoder().decode(new Uint8Array(data.slice(i, i + n))); i += n; return s; };
    try {
      let seed = 0n, shift = 0n;
      for (;;) { const b = u8(); seed |= BigInt(b & 0x7f) << shift; shift += 7n; if (!(b & 0x80)) break; }
      i += 1 + 2 + 1 + 1 + 1; // transpose, tempo, swing, muted, soloed
      const levels = u8(); i += levels * 2;
      const leads = u8();
      for (let k = 0; k < leads; k++) { u8(); str(); }
      const genre = str(), mood = str(), name = str();
      return { name, genre, mood, seed: seed.toString() };
    } catch (e) { return null; }
  }

  function load() {
    try { const v = JSON.parse(localStorage.getItem(KEY) || '[]'); return Array.isArray(v) ? v.filter((c) => typeof c === 'string') : []; } catch (e) { return []; }
  }
  function save(codes) {
    try { localStorage.setItem(KEY, JSON.stringify(codes)); } catch (e) { /* private window: kept for this visit only */ }
  }

  const words = (id) => String(id || '').replace(/_/g, ' ');
  const genreWords = (id) => { const k = 'music.genre.' + id; const w = i18n.t(k); return w && w !== k ? w : words(id); };
  const moodWords = (id) => { const k = 'music.mood.' + id; const w = i18n.t(k); return w && w !== k ? w : words(id); };

  let codes = load();
  const list = document.getElementById('mp-list');
  const msg = document.getElementById('mp-msg');
  const input = document.getElementById('mp-code');

  function render() {
    list.textContent = '';
    if (!codes.length) {
      const li = document.createElement('li');
      li.className = 'muted';
      li.textContent = i18n.t('music_page.web_none');
      list.appendChild(li);
      return;
    }
    codes.forEach((code, idx) => {
      const f = read(code);
      if (!f) return;
      const li = document.createElement('li');
      const name = document.createElement('div');
      name.className = 'name';
      name.textContent = f.name || i18n.t('music_page.default_name', { seed: f.seed });
      const line = document.createElement('div');
      line.className = 'line';
      line.textContent = i18n.t('music_page.fav_line', { genre: genreWords(f.genre), mood: moodWords(f.mood), seed: f.seed });
      const row = document.createElement('div');
      row.className = 'row';
      const copy = document.createElement('button');
      copy.type = 'button';
      copy.textContent = i18n.t('music_page.web_copy');
      copy.setAttribute('data-i18n-tip', 'music_page.copy_code_hint');
      copy.addEventListener('click', () => {
        (navigator.clipboard ? navigator.clipboard.writeText(code) : Promise.reject()).then(() => { msg.textContent = i18n.t('music_page.code_copied'); }).catch(() => { input.value = code; input.select(); });
      });
      const remove = document.createElement('button');
      remove.type = 'button';
      remove.textContent = i18n.t('music_page.web_remove');
      remove.setAttribute('data-i18n-tip', 'music_page.remove_hint');
      remove.addEventListener('click', () => { codes.splice(idx, 1); save(codes); render(); });
      row.appendChild(copy);
      row.appendChild(remove);
      li.appendChild(name);
      li.appendChild(line);
      li.appendChild(row);
      list.appendChild(li);
    });
  }

  document.getElementById('mp-add').addEventListener('click', () => {
    const code = input.value.trim();
    const f = read(code);
    if (!f) { msg.textContent = i18n.t('music_page.web_bad_code'); return; }
    if (!codes.includes(code)) { codes.push(code); save(codes); }
    input.value = '';
    msg.textContent = i18n.t('music_page.paste_ok', { name: f.name || f.seed });
    render();
  });

  render();
})();
