  // ══════════════════════════════════════
  // FANTASY TAB
  // ══════════════════════════════════════

  // ── Card collapse (reuse pattern) ──
  function toggleFantasyCard(id) {
   const card = document.getElementById('fantasy-' + id);
   if (card) card.classList.toggle('collapsed');
   localStorage.setItem('fantasy_collapsed_' + id, card.classList.contains('collapsed'));
  }
  ['worldmap', 'character', 'lore', 'achievements', 'celestial'].forEach(id => {
   if (localStorage.getItem('fantasy_collapsed_' + id) === 'true') {
    const card = document.getElementById('fantasy-' + id);
    if (card) card.classList.add('collapsed');
   }
  });

  // ── World Map (procedural grid) ──
  (function drawWorldMap() {
   const canvas = document.getElementById('world-map-canvas');
   if (!canvas) return;
   const ctx = canvas.getContext('2d');
   const w = canvas.width, h = canvas.height;
   // Seed-based pseudo-random
   let seed = 42;
   function rng() { seed = (seed * 16807 + 0) % 2147483647; return (seed - 1) / 2147483646; }

   // Draw terrain grid
   const cellSize = 8;
   const cols = Math.ceil(w / cellSize), rows = Math.ceil(h / cellSize);
   for (let y = 0; y < rows; y++) {
    for (let x = 0; x < cols; x++) {
     const v = rng();
     const cx = x / cols, cy = y / rows;
     const dist = Math.sqrt((cx - 0.5) ** 2 + (cy - 0.5) ** 2);
     if (v < 0.15 + dist * 0.3) {
      // water
      const b = Math.floor(40 + rng() * 30);
      ctx.fillStyle = `rgb(${5+Math.floor(rng()*10)},${10+Math.floor(rng()*15)},${b})`;
     } else if (v < 0.5) {
      // land
      const g = Math.floor(25 + rng() * 35);
      ctx.fillStyle = `rgb(${8+Math.floor(rng()*12)},${g},${8+Math.floor(rng()*10)})`;
     } else if (v < 0.75) {
      // highlands
      const g = Math.floor(35 + rng() * 20);
      ctx.fillStyle = `rgb(${15+Math.floor(rng()*10)},${g},${20+Math.floor(rng()*10)})`;
     } else {
      // mountains
      const g = Math.floor(20 + rng() * 15);
      ctx.fillStyle = `rgb(${g},${g},${g+5})`;
     }
     ctx.fillRect(x * cellSize, y * cellSize, cellSize, cellSize);
    }
   }
   // Overlay fog
   const grad = ctx.createRadialGradient(w/2, h/2, w*0.15, w/2, h/2, w*0.5);
   grad.addColorStop(0, 'rgba(10,10,26,0)');
   grad.addColorStop(1, 'rgba(10,10,26,0.85)');
   ctx.fillStyle = grad;
   ctx.fillRect(0, 0, w, h);
   // Label
   ctx.font = '14px sans-serif';
   ctx.fillStyle = 'rgba(153,102,255,0.6)';
   ctx.textAlign = 'center';
   ctx.fillText('âš" Terra Incognita âš"', w/2, h/2);
  })();

  // ── Character Sheet ──
  function loadCharacter() {
   try { return JSON.parse(localStorage.getItem('humanity_character')); } catch { return null; }
  }
  function saveCharacter(c) { localStorage.setItem('humanity_character', JSON.stringify(c)); }

  function rollStats() {
   const roll = () => Math.floor(Math.random() * 16) + 3; // 3-18
   return { Strength: roll(), Intelligence: roll(), Dexterity: roll(), Charisma: roll() };
  }

  function renderCharacter() {
   const c = loadCharacter();
   const create = document.getElementById('char-create');
   const display = document.getElementById('char-display');
   if (!c) {
    create.style.display = 'block';
    display.style.display = 'none';
    return;
   }
   create.style.display = 'none';
   display.style.display = 'block';
   const classIcons = { Explorer: '🧭', Builder: '🔨', Scholar: '📚', Guardian: '🛡️' };
   document.getElementById('char-d-name').textContent = c.name;
   document.getElementById('char-d-class').textContent = (classIcons[c.class] || '') + ' ' + c.class;
   document.getElementById('char-d-level').textContent = 'Lv. ' + (c.level || 1);
   const statsEl = document.getElementById('char-stats');
   const statColors = { Strength: '#e55', Intelligence: '#58f', Dexterity: '#4c8', Charisma: '#eb4' };
   statsEl.innerHTML = Object.entries(c.stats).map(([k, v]) => {
    const pct = ((v - 3) / 15 * 100).toFixed(0);
    return `<div style="background:var(--bg-input);border-radius:6px;padding:0.35rem 0.5rem;">
     <div style="display:flex;justify-content:space-between;font-size:0.75rem;margin-bottom:0.2rem;">
      <span style="color:var(--text-muted);">${k}</span><span style="color:${statColors[k]||'var(--text)'};font-weight:700;">${v}</span>
     </div>
     <div style="background:var(--bg);border-radius:3px;height:4px;overflow:hidden;">
      <div style="width:${pct}%;height:100%;background:${statColors[k]||'var(--accent)'};border-radius:3px;"></div>
     </div>
    </div>`;
   }).join('');

   // Auto-unlock achievements
   checkAchievements();
  }

  function createCharacter() {
   const name = document.getElementById('char-name-input').value.trim();
   if (!name) { document.getElementById('char-name-input').style.borderColor = '#e55'; return; }
   const cls = document.getElementById('char-class-input').value;
   saveCharacter({ name, class: cls, level: 1, stats: rollStats(), createdAt: new Date().toISOString() });
   renderCharacter();
   // Unlock achievement
   unlockAchievement('character_created');
  }

  function rerollStats() {
   const c = loadCharacter();
   if (!c) return;
   c.stats = rollStats();
   saveCharacter(c);
   renderCharacter();
  }

  function deleteCharacter() {
   if (!confirm('Delete your character? This cannot be undone.')) return;
   localStorage.removeItem('humanity_character');
   renderCharacter();
  }

  renderCharacter();

  // ── Lore / Codex ──
  const LORE_ENTRIES = [
   {
    title: '🌅 The Awakening',
    text: 'Before the Network, humanity existed in silos — each mind an island, separated by distance, language, and power structures designed to keep them apart. The first spark came not from technology, but from a simple idea: what if we actually cooperated?'
   },
   {
    title: '🔗 The Accord',
    text: 'The Humanity Accord is not a contract — it\'s a promise. Every node in the network agrees to three principles: Transparency in governance, equity in access, and cooperation over competition. Those who sign the Accord don\'t just join a network — they join a movement.'
   },
   {
    title: '🌌 The Long Road',
    text: 'The destination was always the stars. But first, humanity had to learn to talk to itself. The Humanity Network is the foundation — a mesh of minds, resources, and purpose. From chat rooms to space stations, every great journey begins with "hello."'
   },
   {
    title: '🛡️ The Guardians',
    text: 'Not every force in the world welcomes cooperation. The Guardians are those who protect the network — not with weapons, but with code, with vigilance, with the simple act of refusing to let the dream die. They moderate, they build, they hold the line.'
   },
   {
    title: '📡 The Relay',
    text: 'Messages travel through the Relay — a decentralized web of servers that carry humanity\'s voice. No single point of failure, no single point of control. When one relay falls, others rise. The network remembers, even when individuals forget.'
   }
  ];

  function renderLore() {
   const container = document.getElementById('lore-entries');
   container.innerHTML = LORE_ENTRIES.map((entry, i) => `
    <details style="background:var(--bg-input);border-radius:6px;padding:0.1rem 0.6rem;" ${i === 0 ? 'open' : ''}>
     <summary style="cursor:pointer;font-weight:600;font-size:0.85rem;color:var(--text);padding:0.4rem 0;user-select:none;list-style:none;">
      ${entry.title}
     </summary>
     <p style="font-size:0.82rem;color:var(--text-muted);line-height:1.6;padding:0 0 0.5rem;">${entry.text}</p>
    </details>
   `).join('');
  }
  renderLore();

  // ── Achievements ──
  const ACHIEVEMENTS = [
   { id: 'first_steps', icon: '👣', name: 'First Steps', desc: 'Visited the Humanity Hub', auto: true },
   { id: 'character_created', icon: '⚔️', name: 'Born Anew', desc: 'Created a character' },
   { id: 'voice_of_people', icon: '📢', name: 'Voice of the People', desc: 'Sent your first message' },
   { id: 'gardener', icon: '🌱', name: 'Green Thumb', desc: 'Planted something in your garden' },
   { id: 'note_taker', icon: '📝', name: 'Scribe', desc: 'Created your first note' },
   { id: 'explorer', icon: '🧭', name: 'Explorer', desc: 'Visited all hub tabs' },
   { id: 'lore_reader', icon: '📜', name: 'Lorekeeper', desc: 'Read all codex entries' },
   { id: 'customizer', icon: '🎨', name: 'Fashionista', desc: 'Changed your accent color' },
   { id: 'night_owl', icon: '🦉', name: 'Night Owl', desc: 'Online past midnight' },
   { id: 'streamer', icon: '🎥', name: 'Camera Ready', desc: 'Tested screen/camera capture' },
   { id: 'completionist', icon: '💎', name: 'Completionist', desc: 'Unlock all other achievements' },
   { id: 'secret', icon: '🔮', name: '???', desc: 'A secret achievement…' },
  ];

  function loadAchievements() {
   try { return JSON.parse(localStorage.getItem('humanity_achievements')) || []; } catch { return []; }
  }
  function saveAchievements(a) { localStorage.setItem('humanity_achievements', JSON.stringify(a)); }

  function unlockAchievement(id) {
   const unlocked = loadAchievements();
   if (unlocked.includes(id)) return;
   unlocked.push(id);
   saveAchievements(unlocked);
   renderAchievements();
  }

  function checkAchievements() {
   // Auto-unlock based on conditions
   unlockAchievement('first_steps');
   if (localStorage.getItem('humanity_character')) unlockAchievement('character_created');
   if (localStorage.getItem('humanity_name')) unlockAchievement('voice_of_people');
   const garden = (() => { try { return JSON.parse(localStorage.getItem('humanity_garden')); } catch { return null; } })();
   if (garden && garden.plots && Object.keys(garden.plots).length > 0) unlockAchievement('gardener');
   const notes = (() => { try { return JSON.parse(localStorage.getItem('humanity_notes')) || []; } catch { return []; } })();
   if (notes && notes.length > 0) unlockAchievement('note_taker');
   const settings = (() => { try { return JSON.parse(localStorage.getItem('humanity_settings')) || {}; } catch { return {}; } })();
   if (settings && settings.accent && settings.accent !== '#FF8811') unlockAchievement('customizer');
   if (new Date().getHours() >= 0 && new Date().getHours() < 5) unlockAchievement('night_owl');
  }

  function renderAchievements() {
   const unlocked = loadAchievements();
   const grid = document.getElementById('achievement-grid');
   grid.innerHTML = ACHIEVEMENTS.map(a => {
    const isUnlocked = unlocked.includes(a.id);
    return `<div style="background:${isUnlocked ? 'rgba(153,102,255,0.1)' : 'var(--bg-input)'};border:1px solid ${isUnlocked ? 'rgba(153,102,255,0.3)' : 'var(--border)'};border-radius:8px;padding:0.5rem;text-align:center;transition:border-color 0.2s;" title="${a.desc}">
     <div style="font-size:1.5rem;${isUnlocked ? '' : 'filter:grayscale(1) brightness(0.4);'}">${a.icon}</div>
     <div style="font-size:0.72rem;font-weight:600;color:${isUnlocked ? 'var(--text)' : 'var(--text-muted)'};margin-top:0.2rem;">${isUnlocked ? a.name : '🔒 Locked'}</div>
     <div style="font-size:0.6rem;color:var(--text-muted);margin-top:0.1rem;">${a.desc}</div>
    </div>`;
   }).join('');
  }
  checkAchievements();
  renderAchievements();

