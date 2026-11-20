// Moved out of web/pages/home.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
(async function () {
  function esc(s) {
    return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  }
  const T = (key, vars) => i18n.html(key, vars);
  let o;
  try {
    const res = await fetch('/data/home_outline.json');
    if (!res.ok) throw new Error(res.status);
    o = await res.json();
  } catch (e) {
    document.getElementById('o-loops').innerHTML = '<p class="load-err">' + T('home.load_error', { error: e.message }) + '</p>';
    return;
  }
  const loops = o.loops || [];
  if (o.title) document.getElementById('o-title').textContent = o.title;
  const closing = loops.filter(l => l.closes).length;
  document.getElementById('o-summary').innerHTML = T('home.outline_summary', { closed: closing, total: loops.length });
  document.getElementById('o-subtitle').textContent = o.subtitle || '';
  if (o.intro) {
    document.getElementById('o-intro').textContent = o.intro;
    document.getElementById('o-about').hidden = false;
  }

  // The two tiers of a loop (operator 2026-07-05): the bare minimum first, then the life-of-luxury
  // additions. An empty game_id is the honest "not in game yet" pill, so the luxury tier doubles
  // as the game-gap list.
  const tier = (l, t) => (l.requirements || []).filter(r => t === 'baseline' ? (!r.tier || r.tier === 'baseline') : r.tier === t);
  const missing = l => (l.requirements || []).filter(r => !r.game_id).length;
  const parts = n => esc(i18n.tn ? i18n.tn('home.parts', n) : (n + ''));
  const pill = l => l.closes
    ? '<span class="status-badge tone-success">' + T('home.outline_closes') + '</span>'
    : '<span class="status-badge tone-warning">' + T('home.outline_open') + '</span>';
  const list = lines => '<ul>' + lines.map(s => '<li>' + esc(s) + '</li>').join('') + '</ul>';

  function reqRow(r) {
    return '<div class="req"><span class="qty">' + esc(r.qty) + ' ' + esc(r.item) + '</span> ' +
      (r.game_id ? '<span class="gid">' + esc(r.game_id) + '</span>' : '<span class="status-badge tone-warning">' + T('home.not_in_game') + '</span>') +
      '<span class="why">' + esc(r.why) + '</span></div>';
  }

  // A loop's card, then its two detail panels (hidden until its fold buttons open them): each a
  // whole row of the grid, opening under the card's row (list-views.css, .card-detail).
  function card(l) {
    const id = esc(l.id);
    const base = tier(l, 'baseline');
    const lux = tier(l, 'luxury');
    const gap = missing(l);
    const heading = what => '<h3 class="detail-heading">' + esc(l.name) + '<span>' + T(what) + '</span></h3>';
    let takes = '';
    if (base.length) takes += '<div><div class="tier-label">' + T('home.bare_minimum') + '</div>' + base.map(reqRow).join('') + '</div>';
    if (lux.length) takes += '<div><div class="tier-label luxury">' + T('home.luxury') + '</div>' + lux.map(reqRow).join('') + '</div>';
    const how = '<div><h4>' + T('home.demand') + '</h4><p>' + esc(l.demand) + '</p><p>' + esc(l.closure_note) + '</p></div>' +
      ((l.balance || []).length ? '<div><h4>' + T('home.the_arithmetic') + '</h4>' + list(l.balance) + '</div>' : '') +
      ((l.assumptions || []).length ? '<div><h4>' + T('home.assumptions') + '</h4>' + list(l.assumptions) + '</div>' : '');
    return '<div class="card-cell" id="loop-' + id + '">' +
      '<div class="card-head"><span class="card-name">' + esc(l.name) + '</span>' + pill(l) + '</div>' +
      '<div class="card-meta">' + parts((l.requirements || []).length) +
        (gap ? ' <span class="status-badge tone-warning">' + T('home.not_in_game_count', { n: gap }) + '</span>' : '') + '</div>' +
      '<div class="labelled"><span class="label">' + T('home.demand') + '</span><span class="clamp-3" title="' + esc(l.demand) + '">' + esc(l.demand) + '</span></div>' +
      '<button type="button" class="fold-btn" aria-expanded="false" aria-controls="how-' + id + '" data-i18n="home.how_worked_out" data-i18n-tip="home.how_worked_out_hint">' + T('home.how_worked_out') + '</button>' +
      (takes ? '<button type="button" class="fold-btn" aria-expanded="false" aria-controls="takes-' + id + '" data-i18n="home.what_it_takes" data-i18n-tip="home.what_it_takes_hint">' + T('home.what_it_takes') + '</button>' : '') +
    '</div>' +
    '<div class="card-detail" id="how-' + id + '" hidden>' + heading('home.how_worked_out') + '<div class="detail-cols">' + how + '</div></div>' +
    (takes ? '<div class="card-detail" id="takes-' + id + '" hidden>' + heading('home.what_it_takes') + '<div class="detail-cols">' + takes + '</div></div>' : '');
  }

  function table() {
    const head = ['home.col_loop', 'home.col_status', 'home.demand', 'home.bare_minimum', 'home.luxury', 'home.col_missing']
      .map(k => '<th>' + T(k) + '</th>').join('');
    const rows = loops.map(l => {
      const gap = missing(l);
      const tone = l.closes ? 'tone-success' : 'tone-warning';
      return '<tr><td><strong>' + esc(l.name) + '</strong></td>' +
        '<td class="' + tone + '"><span class="dot"></span>' + T(l.closes ? 'home.outline_closes' : 'home.outline_open') + '</td>' +
        '<td title="' + esc(l.demand) + '">' + esc(l.demand) + '</td>' +
        '<td>' + parts(tier(l, 'baseline').length) + '</td>' +
        '<td class="tone-info">' + parts(tier(l, 'luxury').length) + '</td>' +
        '<td class="' + (gap ? 'tone-warning' : '') + '">' + gap + '</td></tr>';
    }).join('');
    return '<div class="data-table-wrap"><table class="data-table"><thead><tr>' + head + '</tr></thead><tbody>' + rows + '</tbody></table></div>';
  }

  const wrap = document.getElementById('o-loops');
  // A card's fold button opens or closes its detail panel.
  wrap.addEventListener('click', function (e) {
    const b = e.target.closest('.fold-btn');
    if (!b) return;
    const open = b.getAttribute('aria-expanded') !== 'true';
    b.setAttribute('aria-expanded', open ? 'true' : 'false');
    const panel = document.getElementById(b.getAttribute('aria-controls'));
    if (panel) panel.hidden = !open;
  });
  function draw(view) {
    wrap.innerHTML = view === 'table' ? table() : '<div class="card-grid" style="--card-min: 330px">' + loops.map(card).join('') + '</div>';
    // The summaries' words and hints are keys: fill them (and their hover hints) now.
    i18n.apply(wrap);
  }
  draw(HosViewToggle.render(document.getElementById('o-view'), 'home_outline', 'cards', draw));

  if (o.cannot_close && o.cannot_close.note) {
    document.getElementById('o-cannot-note').textContent = o.cannot_close.note;
    document.getElementById('o-cannot').hidden = false;
  }
  if (Array.isArray(o.in_game_next) && o.in_game_next.length) {
    document.getElementById('o-next-list').innerHTML = o.in_game_next.map(s => '<li>' + esc(s) + '</li>').join('');
    document.getElementById('o-next').hidden = false;
  }
  document.getElementById('o-footer').textContent = o.footer || '';
})();
