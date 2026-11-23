// Moved out of web/pages/ops.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.

(function() {
  // Populate system info
  const el = id => document.getElementById(id);
  if (el('debug-version')) el('debug-version').textContent = 'v0.1571.0';
  if (el('debug-server'))  el('debug-server').textContent  = window.location.origin;
  if (el('debug-ua'))      el('debug-ua').textContent      = navigator.userAgent.slice(0, 120);
  if (el('debug-screen'))  el('debug-screen').textContent  = screen.width + '×' + screen.height + ' · ' + (window.devicePixelRatio||1) + 'x DPR';
  if (el('debug-loaded'))  el('debug-loaded').textContent  = new Date().toISOString();
  fetch('/api/stats').then(r => r.ok ? r.json() : null).then(data => {
    if (!data) { if(el('debug-connection')) el('debug-connection').textContent = '⚠️ Server unreachable'; return; }
    if(el('debug-connection')) el('debug-connection').textContent = '✅ Connected, ' + (data.online_users||0) + ' online';
    if(el('debug-version') && data.version) el('debug-version').textContent = data.version;
  }).catch(() => { if(el('debug-connection')) el('debug-connection').textContent = '❌ Cannot reach server'; });
})();

  const states = ['btn-clickable','btn-channeling','btn-activated','btn-cooldown'];

  function testBtnClick(btn) {
   const orig = btn.textContent;
   btn.textContent = '✓ Clicked!';
   setTimeout(() => { btn.textContent = orig; }, 800);
  }
  function cycleState(btn) {
   const current = states.find(s => btn.classList.contains(s)) || 'btn-clickable';
   const idx = states.indexOf(current);
   const next = states[(idx + 1) % states.length];
   states.forEach(s => btn.classList.remove(s));
   btn.classList.add(next);
   btn.querySelectorAll('.btn-cooldown-timer').forEach(t => t.remove());
   if (next === 'btn-cooldown') {
    btn.innerHTML = btn.textContent.replace(/[\d.]+s/g, '').trim();
    const topT = document.createElement('span');
    topT.className = 'btn-cooldown-timer top';
    topT.textContent = '∞';
    const botT = document.createElement('span');
    botT.className = 'btn-cooldown-timer bottom';
    botT.textContent = '∞';
    btn.appendChild(topT);
    btn.appendChild(botT);
   }
  }
  function toggleChannel(btn) {
   if (btn.classList.contains('btn-channeling')) {
    btn.classList.remove('btn-channeling');
    btn.classList.add('btn-clickable');
   } else {
    btn.classList.remove('btn-clickable');
    btn.classList.add('btn-channeling');
   }
  }

  function startCooldown(btn, seconds) {
   if (btn.classList.contains('btn-cooldown')) return;
   const originalText = btn.textContent;
   btn.classList.remove('btn-clickable');
   btn.classList.add('btn-cooldown');

   const topT = document.createElement('span');
   topT.className = 'btn-cooldown-timer top';
   const botT = document.createElement('span');
   botT.className = 'btn-cooldown-timer bottom';
   btn.appendChild(topT);
   btn.appendChild(botT);

   let remaining = seconds;
   const update = () => {
    topT.textContent = remaining.toFixed(1) + 's';
    botT.textContent = remaining.toFixed(1) + 's';
   };
   update();

   const interval = setInterval(() => {
    remaining -= 0.1;
    if (remaining <= 0) {
     clearInterval(interval);
     btn.classList.remove('btn-cooldown');
     btn.classList.add('btn-clickable');
     topT.remove();
     botT.remove();
     btn.textContent = originalText;
    } else {
     update();
    }
   }, 100);
  }
