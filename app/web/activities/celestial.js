  // ══════════════════════════════════════
  // CELESTIAL NAVIGATION
  // ══════════════════════════════════════
 (async function initCelestialMap() {
  const canvas = document.getElementById('celestial-canvas');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');


   // ── Solar System + Star Catalog data loaded from JSON ──
   // Data fetch replaces inline const SUN, PLANETS, MOONS, STAR_CATALOG declarations.
   let SUN, PLANETS, MOONS, STAR_CATALOG;

   async function celLoadData() {
    const [solarRes, starsRes] = await Promise.all([
     fetch('/data/solar-system.json'),
     fetch('/data/stars-nearby.json')
    ]);
    const solar = await solarRes.json();
    const starsArr = await starsRes.json();
    SUN = solar.sun;
    PLANETS = solar.planets;
    MOONS = solar.moons;
    STAR_CATALOG = starsArr;
   }

   // Spectral type → color
   function spectralColor(sp) {
    if (!sp) return '#ffffff';
    const c = sp.charAt(0).toUpperCase();
    const map = {O:'#9bb0ff',B:'#aabfff',A:'#cad7ff',F:'#f8f7ff',G:'#fff4ea',K:'#ffd2a1',M:'#ffcc6f',D:'#ffffff',L:'#ff6633',T:'#cc3300'};
    return map[c] || '#ffffff';
   }

   // ── State ──
   let celMode = 'reality'; // reality | fantasy
   let celLevel = 'sector'; // sector | system | planet
   let celTarget = null; // current star/planet/moon id
   let celPan = {x:0, y:0};
   let celZoom = 1.0;
   let celDragging = false;
   let celDragStart = {x:0,y:0};
   let celPanStart = {x:0,y:0};
   let celHover = null;
   let celSelected = null;
   let celAnimate = false;

   // Load saved state
   try {
    const saved = JSON.parse(localStorage.getItem('humanity_celestial'));
    if (saved) {
     celMode = saved.mode || 'reality';
     celLevel = saved.viewState?.level || 'sector';
     celTarget = saved.viewState?.target || null;
     celPan = saved.viewState?.pan || {x:0,y:0};
     celZoom = saved.viewState?.zoom || 1.0;
    }
   } catch {}

   function celSave() {
    const state = {
     mode: celMode,
     viewState: { level: celLevel, target: celTarget, pan: {...celPan}, zoom: celZoom },
     homePlanet: 'earth',
     bookmarks: [],
     visited: []
    };
    localStorage.setItem('humanity_celestial', JSON.stringify(state));
   }

   // ── Mode Toggle ──
   window.celSetMode = function(mode) {
    celMode = mode;
    document.getElementById('cel-mode-reality').style.background = mode === 'reality' ? 'rgba(100,150,255,0.4)' : 'rgba(20,20,40,0.8)';
    document.getElementById('cel-mode-fantasy').style.background = mode === 'fantasy' ? 'rgba(150,100,255,0.4)' : 'rgba(20,20,40,0.8)';
    celSave();
    celRender();
   };

   // ── Zoom Controls ──
   window.celZoomIn = function() { celZoom = Math.min(celZoom * 1.4, 50); celSave(); celRender(); };
   window.celZoomOut = function() { celZoom = Math.max(celZoom / 1.4, 0.1); celSave(); celRender(); };

   // ── Navigation ──
   function celNavigate(level, target) {
    celLevel = level;
    celTarget = target;
    celPan = {x:0, y:0};
    if (level === 'sector') celZoom = 1.0;
    else if (level === 'system') celZoom = 1.0;
    else if (level === 'planet') celZoom = 1.0;
    celSelected = null;
    celSave();
    celRender();
    celRenderBreadcrumb();
    celRenderInfo();
   }

   // ── Breadcrumb ──
   function celRenderBreadcrumb() {
    const bc = document.getElementById('cel-breadcrumb');
    if (!bc) return;
    let parts = [];
    const mkLink = (text, fn) => `<span style="cursor:pointer;color:#6699ff;text-decoration:underline;" onclick="${fn}">${text}</span>`;
    parts.push(mkLink('🌌 Universe', ""));
    parts.push(mkLink('Milky Way', ""));
    if (celLevel === 'sector') {
     parts.push('<span style="color:var(--text);">Local Stars</span>');
    } else if (celLevel === 'system') {
     parts.push(mkLink('Local Stars', "celNavigate('sector',null)"));
     const star = celTarget === 'SOL' ? SUN : findStar(celTarget);
     parts.push(`<span style="color:var(--text);">${star ? (star.properName || star.name) : celTarget}</span>`);
    } else if (celLevel === 'planet') {
     parts.push(mkLink('Local Stars', "celNavigate('sector',null)"));
     const planet = PLANETS.find(p => p.id === celTarget);
     if (planet) {
      parts.push(mkLink('Sol', "celNavigate('system','SOL')"));
      parts.push(`<span style="color:var(--text);">${planet.name}</span>`);
     }
    }
    bc.innerHTML = parts.join(' <span style="color:var(--text-muted);">›</span> ');
   }

   function findStar(id) {
    if (id === 'SOL') return SUN;
    const entry = STAR_CATALOG.find(s => s[0] === id || s[7] === id);
    if (!entry) return null;
    return {
     id: entry[0], name: entry[0], properName: entry[7] || entry[0],
     position: {x:entry[1],y:entry[2],z:entry[3]},
     spectralType: entry[4], magnitude: entry[5], absMagnitude: entry[6],
     distance: Math.sqrt(entry[1]**2+entry[2]**2+entry[3]**2),
     color: spectralColor(entry[4]),
     planets: id === 'SOL' ? SUN.planets : []
    };
   }

   // ── Rendering ──
   function celRender() {
    const w = canvas.width, h = canvas.height;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvas.clientWidth * dpr;
    canvas.height = canvas.clientHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const cw = canvas.clientWidth, ch = canvas.clientHeight;

    // Clear
    ctx.fillStyle = '#05050f';
    ctx.fillRect(0, 0, cw, ch);

    // Draw faint background stars
    ctx.save();
    let bgSeed = 12345;
    const bgRng = () => { bgSeed = (bgSeed * 16807) % 2147483647; return (bgSeed - 1) / 2147483646; };
    for (let i = 0; i < 200; i++) {
     const bx = bgRng() * cw, by = bgRng() * ch;
     const br = bgRng() * 0.8 + 0.2;
     const ba = bgRng() * 0.4 + 0.1;
     ctx.fillStyle = `rgba(200,210,255,${ba})`;
     ctx.beginPath();
     ctx.arc(bx, by, br, 0, Math.PI * 2);
     ctx.fill();
    }
    ctx.restore();

    if (celLevel === 'sector') celRenderSector(cw, ch);
    else if (celLevel === 'system') celRenderSystem(cw, ch);
    else if (celLevel === 'planet') celRenderPlanet(cw, ch);
   }

   // ── Sector View (Stellar Neighborhood) ──
   let sectorStarPositions = []; // [{name, sx, sy, star, distPc}]
   function celRenderSector(cw, ch) {
    sectorStarPositions = [];
    const cx = cw / 2 + celPan.x;
    const cy = ch / 2 + celPan.y;
    const scale = 25 * celZoom; // pixels per parsec

    // Grid
    ctx.strokeStyle = 'rgba(50,70,120,0.15)';
    ctx.lineWidth = 0.5;
    const gridStep = 5; // parsecs
    const gridPx = gridStep * scale;
    if (gridPx > 20) {
     for (let gx = cx % gridPx; gx < cw; gx += gridPx) {
      ctx.beginPath(); ctx.moveTo(gx, 0); ctx.lineTo(gx, ch); ctx.stroke();
     }
     for (let gy = cy % gridPx; gy < ch; gy += gridPx) {
      ctx.beginPath(); ctx.moveTo(0, gy); ctx.lineTo(cw, gy); ctx.stroke();
     }
    }

    // Sol
    const solSx = cx, solSy = cy;
    drawStar(solSx, solSy, '#FFF5E0', 4, 'Sol');
    sectorStarPositions.push({name:'SOL',sx:solSx,sy:solSy,star:SUN,distPc:0});

    // Catalog stars (project x,z onto screen — top-down galactic plane)
    for (const s of STAR_CATALOG) {
     const sx = cx + s[1] * scale;
     const sy = cy - s[3] * scale; // z → up
     const col = spectralColor(s[4]);
     const mag = s[5];
     const r = Math.max(1.5, Math.min(4, (8 - mag) * 0.3)) * Math.min(celZoom, 2);
     const distPc = Math.sqrt(s[1]**2+s[2]**2+s[3]**2);

     if (sx > -20 && sx < cw + 20 && sy > -20 && sy < ch + 20) {
      drawStar(sx, sy, col, r, celZoom > 0.8 ? (s[7] || '') : '');
      sectorStarPositions.push({name:s[0],sx,sy,star:{name:s[0],properName:s[7]||s[0],spectralType:s[4],magnitude:s[5],absMagnitude:s[6],distance:distPc,color:col,position:{x:s[1],y:s[2],z:s[3]}},distPc});
     }
    }

    // Scale indicator
    ctx.fillStyle = 'rgba(100,150,255,0.5)';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'left';
    const scaleBarPc = 5;
    const scaleBarPx = scaleBarPc * scale;
    if (scaleBarPx > 20 && scaleBarPx < cw * 0.6) {
     ctx.fillRect(10, ch - 20, scaleBarPx, 2);
     ctx.fillText(scaleBarPc + ' pc', 10, ch - 25);
    }
   }

   function drawStar(x, y, color, radius, label) {
    // Glow
    const grad = ctx.createRadialGradient(x, y, 0, x, y, radius * 3);
    grad.addColorStop(0, color);
    grad.addColorStop(1, 'transparent');
    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(x, y, radius * 3, 0, Math.PI * 2);
    ctx.fill();
    // Core
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y, radius, 0, Math.PI * 2);
    ctx.fill();
    // Label
    if (label) {
     ctx.fillStyle = 'rgba(200,210,255,0.7)';
     ctx.font = '9px sans-serif';
     ctx.textAlign = 'center';
     ctx.fillText(label, x, y - radius - 4);
    }
   }

   // ── System View (Solar System) ──
   let systemPlanetPositions = []; // [{id, sx, sy, planet, r}]
   function celRenderSystem(cw, ch) {
    systemPlanetPositions = [];
    if (celTarget !== 'SOL') {
     // Non-Sol stars — just show info, no planets
     ctx.fillStyle = 'rgba(200,210,255,0.5)';
     ctx.font = '14px sans-serif';
     ctx.textAlign = 'center';
     const star = findStar(celTarget);
     ctx.fillText(star ? star.properName || star.name : celTarget, cw/2, ch/2 - 20);
     ctx.font = '11px sans-serif';
     ctx.fillText('No detailed planet data available (yet)', cw/2, ch/2 + 10);
     ctx.fillText('Spectral Type: ' + (star ? star.spectralType : '?'), cw/2, ch/2 + 30);
     return;
    }

    const cx = cw / 2 + celPan.x;
    const cy = ch / 2 + celPan.y;

    // Scale: AU to pixels — logarithmic for visibility
    function auToR(au) {
     return (30 + Math.log2(au + 0.1) * 35) * celZoom;
    }

    // Draw Sun
    ctx.fillStyle = '#FFF5E0';
    ctx.beginPath();
    ctx.arc(cx, cy, 8 * Math.min(celZoom, 2), 0, Math.PI * 2);
    ctx.fill();
    const sunGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, 20 * Math.min(celZoom, 2));
    sunGrad.addColorStop(0, 'rgba(255,245,224,0.3)');
    sunGrad.addColorStop(1, 'transparent');
    ctx.fillStyle = sunGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, 20 * Math.min(celZoom, 2), 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = 'rgba(255,245,224,0.8)';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('☀ Sol', cx, cy - 14 * Math.min(celZoom, 2));

    // Draw planets
    const now = Date.now();
    const J2000 = Date.UTC(2000, 0, 1, 12, 0, 0);
    const daysSinceJ2000 = (now - J2000) / 86400000;

    for (const p of PLANETS) {
     const orbitR = auToR(p.orbit.semiMajor);
     // Draw orbit ellipse
     ctx.strokeStyle = 'rgba(100,150,255,0.12)';
     ctx.lineWidth = 0.8;
     ctx.beginPath();
     ctx.ellipse(cx, cy, orbitR, orbitR * (1 - p.orbit.eccentricity * 0.3), 0, 0, Math.PI * 2);
     ctx.stroke();

     // Calculate position (simplified mean anomaly)
     const meanAnomaly = ((p.orbit.meanLongitude || 0) + (360 / p.orbit.period) * daysSinceJ2000) % 360;
     const angle = meanAnomaly * Math.PI / 180;
     const px = cx + Math.cos(angle) * orbitR;
     const py = cy + Math.sin(angle) * orbitR * (1 - p.orbit.eccentricity * 0.3);

     // Planet size (artistic, not to scale)
     let pr;
     if (p.type === 'gas_giant') pr = 6;
     else if (p.type === 'ice_giant') pr = 5;
     else if (p.type === 'dwarf') pr = 2.5;
     else pr = 3.5;
     pr *= Math.min(celZoom, 2);

     // Draw planet
     ctx.fillStyle = p.color;
     ctx.beginPath();
     ctx.arc(px, py, pr, 0, Math.PI * 2);
     ctx.fill();

     // Rings
     if (p.rings) {
      ctx.strokeStyle = p.color + '66';
      ctx.lineWidth = 1.2;
      ctx.beginPath();
      ctx.ellipse(px, py, pr * 1.8, pr * 0.5, -0.3, 0, Math.PI * 2);
      ctx.stroke();
     }

     // Label
     ctx.fillStyle = 'rgba(200,210,255,0.7)';
     ctx.font = '9px sans-serif';
     ctx.textAlign = 'center';
     ctx.fillText(p.symbol + ' ' + p.name, px, py - pr - 4);

     systemPlanetPositions.push({id:p.id, sx:px, sy:py, planet:p, r:pr});
    }
   }

   // ── Planet View ──
   function celRenderPlanet(cw, ch) {
    const planet = PLANETS.find(p => p.id === celTarget);
    if (!planet) return;

    const cx = cw / 2;
    const cy = ch / 2;
    const pr = Math.min(cw, ch) * 0.3;

    // Draw planet as a sphere with simple shading
    const grad = ctx.createRadialGradient(cx - pr * 0.3, cy - pr * 0.3, pr * 0.1, cx, cy, pr);
    grad.addColorStop(0, planet.color);
    grad.addColorStop(1, '#111122');
    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(cx, cy, pr, 0, Math.PI * 2);
    ctx.fill();

    // Atmosphere glow
    if (planet.atmosphere && planet.atmosphere.pressure > 0.1) {
     const atmoGrad = ctx.createRadialGradient(cx, cy, pr * 0.95, cx, cy, pr * 1.15);
     atmoGrad.addColorStop(0, planet.color + '33');
     atmoGrad.addColorStop(1, 'transparent');
     ctx.fillStyle = atmoGrad;
     ctx.beginPath();
     ctx.arc(cx, cy, pr * 1.15, 0, Math.PI * 2);
     ctx.fill();
    }

    // Icosphere grid overlay (level 0 — 20 triangles projected)
    ctx.strokeStyle = 'rgba(100,150,255,0.15)';
    ctx.lineWidth = 0.5;
    // Draw latitude/longitude lines as approximation
    for (let lat = -60; lat <= 60; lat += 30) {
     const latR = Math.cos(lat * Math.PI / 180) * pr;
     const latY = cy - Math.sin(lat * Math.PI / 180) * pr;
     ctx.beginPath();
     ctx.ellipse(cx, latY, latR, latR * 0.15, 0, 0, Math.PI * 2);
     ctx.stroke();
    }
    for (let lon = 0; lon < 180; lon += 30) {
     ctx.beginPath();
     ctx.ellipse(cx, cy, pr * Math.cos(lon * Math.PI / 180), pr, 0, 0, Math.PI * 2);
     ctx.stroke();
    }

    // Name
    ctx.fillStyle = planet.color;
    ctx.font = 'bold 16px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(planet.symbol + ' ' + planet.name, cx, cy - pr - 20);

    // Draw moons
    const moons = MOONS.filter(m => m.planetId === planet.id);
    moons.forEach((m, i) => {
     const mAngle = (Date.now() / (m.orbit.period * 86400) * Math.PI * 2) % (Math.PI * 2);
     const mOrbitR = pr * 1.3 + i * 25;
     const mx = cx + Math.cos(mAngle) * mOrbitR;
     const my = cy + Math.sin(mAngle) * mOrbitR * 0.4;
     // orbit
     ctx.strokeStyle = 'rgba(150,150,200,0.1)';
     ctx.beginPath();
     ctx.ellipse(cx, cy, mOrbitR, mOrbitR * 0.4, 0, 0, Math.PI * 2);
     ctx.stroke();
     // moon
     ctx.fillStyle = m.color || '#aaa';
     ctx.beginPath();
     ctx.arc(mx, my, Math.max(2, Math.min(m.radius / 500, 6)), 0, Math.PI * 2);
     ctx.fill();
     ctx.fillStyle = 'rgba(200,210,255,0.6)';
     ctx.font = '8px sans-serif';
     ctx.textAlign = 'center';
     ctx.fillText(m.name, mx, my - 8);
    });
   }

   // ── Info Panel ──
   function celRenderInfo() {
    const panel = document.getElementById('cel-info');
    if (!panel) return;

    if (celLevel === 'system' && celTarget !== 'SOL' && celTarget) {
     const star = findStar(celTarget);
     if (star) {
      panel.style.display = 'block';
      panel.innerHTML = `
       <div style="display:flex;align-items:center;gap:0.8rem;margin-bottom:0.6rem;">
        <span style="font-size:2rem;">⭐</span>
        <div>
         <div style="font-weight:700;font-size:1rem;color:${star.color};">${star.properName || star.name}</div>
         <div style="font-size:0.75rem;color:var(--text-muted);">${star.spectralType} · ${star.distance.toFixed(2)} pc</div>
        </div>
       </div>
       <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.4rem;font-size:0.8rem;">
        <div style="color:var(--text-muted);">Apparent Mag: <span style="color:var(--text);">${star.magnitude.toFixed(2)}</span></div>
        <div style="color:var(--text-muted);">Absolute Mag: <span style="color:var(--text);">${star.absMagnitude.toFixed(2)}</span></div>
        <div style="color:var(--text-muted);">Distance: <span style="color:var(--text);">${(star.distance * 3.262).toFixed(2)} ly</span></div>
        <div style="color:var(--text-muted);">Position: <span style="color:var(--text);">${star.position.x.toFixed(1)}, ${star.position.y.toFixed(1)}, ${star.position.z.toFixed(1)} pc</span></div>
       </div>`;
      return;
     }
    }

    if (celSelected && celLevel === 'system') {
     const planet = PLANETS.find(p => p.id === celSelected);
     if (planet) {
      panel.style.display = 'block';
      const atmoStr = planet.atmosphere && planet.atmosphere.composition
       ? Object.entries(planet.atmosphere.composition).map(([k,v]) => `${k} ${v}%`).join(', ')
       : 'None';
      const moonNames = MOONS.filter(m => m.planetId === planet.id).map(m => m.name).join(', ') || 'None';
      panel.innerHTML = `
       <div style="display:flex;align-items:center;gap:0.8rem;margin-bottom:0.6rem;">
        <span style="font-size:2rem;">${planet.symbol}</span>
        <div>
         <div style="font-weight:700;font-size:1rem;color:${planet.color};">${planet.name}</div>
         <div style="font-size:0.75rem;color:var(--text-muted);">${planet.type.replace('_',' ')} · ${planet.orbit.semiMajor.toFixed(2)} AU from Sun</div>
        </div>
       </div>
       <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.3rem 1rem;font-size:0.8rem;">
        <div style="color:var(--text-muted);">Radius: <span style="color:var(--text);">${planet.radius.toLocaleString()} km</span></div>
        <div style="color:var(--text-muted);">Mass: <span style="color:var(--text);">${planet.mass.toExponential(2)} kg</span></div>
        <div style="color:var(--text-muted);">Gravity: <span style="color:var(--text);">${planet.gravity} m/s²</span></div>
        <div style="color:var(--text-muted);">Day Length: <span style="color:var(--text);">${planet.dayLength} hours</span></div>
        <div style="color:var(--text-muted);">Year Length: <span style="color:var(--text);">${planet.orbit.period.toFixed(1)} days</span></div>
        <div style="color:var(--text-muted);">🌡️ Temp: <span style="color:var(--text);">${planet.temperature.min}°C to ${planet.temperature.max}°C (avg ${planet.temperature.avg}°C)</span></div>
        <div style="color:var(--text-muted);">🌬️ Atmosphere: <span style="color:var(--text);">${atmoStr}</span></div>
        <div style="color:var(--text-muted);">💧 Water: <span style="color:var(--text);">${planet.water ? 'Yes' : 'No'}</span></div>
        <div style="color:var(--text-muted);">🧬 Life: <span style="color:var(--text);">${planet.life ? 'Yes' : 'No'}</span></div>
        <div style="color:var(--text-muted);">🌙 Moons: <span style="color:var(--text);">${moonNames}</span></div>
        <div style="color:var(--text-muted);">📦 Resources: <span style="color:var(--text);">${planet.resources.join(', ')}</span></div>
       </div>
       <div style="margin-top:0.6rem;display:flex;gap:0.4rem;">
        <button onclick="celNavigate('planet','${planet.id}')" style="padding:4px 10px;font-size:0.75rem;background:rgba(100,150,255,0.2);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">🌐 View Surface</button>
       </div>`;
      return;
     }
    }

    // Planet view detail
    if (celLevel === 'planet') {
     const planet = PLANETS.find(p => p.id === celTarget);
     if (planet) {
      panel.style.display = 'block';
      const atmoStr = planet.atmosphere && planet.atmosphere.composition
       ? Object.entries(planet.atmosphere.composition).map(([k,v]) => `${k} ${v}%`).join(', ')
       : 'None';
      const moonNames = MOONS.filter(m => m.planetId === planet.id).map(m => m.name).join(', ') || 'None';
      panel.innerHTML = `
       <div style="font-weight:700;font-size:1.1rem;color:${planet.color};margin-bottom:0.5rem;">${planet.symbol} ${planet.name}</div>
       <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.3rem 1rem;font-size:0.8rem;">
        <div style="color:var(--text-muted);">Type: <span style="color:var(--text);">${planet.type.replace('_',' ')}</span></div>
        <div style="color:var(--text-muted);">Radius: <span style="color:var(--text);">${planet.radius.toLocaleString()} km</span></div>
        <div style="color:var(--text-muted);">Mass: <span style="color:var(--text);">${planet.mass.toExponential(2)} kg</span></div>
        <div style="color:var(--text-muted);">Gravity: <span style="color:var(--text);">${planet.gravity} m/s²</span></div>
        <div style="color:var(--text-muted);">Day: <span style="color:var(--text);">${planet.dayLength} hours</span></div>
        <div style="color:var(--text-muted);">Year: <span style="color:var(--text);">${planet.orbit.period.toFixed(1)} days</span></div>
        <div style="color:var(--text-muted);">🌡️ Temp: <span style="color:var(--text);">${planet.temperature.min}°C to ${planet.temperature.max}°C</span></div>
        <div style="color:var(--text-muted);">🌬️ Atmo: <span style="color:var(--text);">${atmoStr}</span></div>
        <div style="color:var(--text-muted);">💧 Water: <span style="color:var(--text);">${planet.water ? 'Yes' : 'No'}</span></div>
        <div style="color:var(--text-muted);">🧬 Life: <span style="color:var(--text);">${planet.life ? 'Yes' : 'No'}</span></div>
        <div style="color:var(--text-muted);">🌙 Moons: <span style="color:var(--text);">${moonNames}</span></div>
        <div style="color:var(--text-muted);">📦 Resources: <span style="color:var(--text);">${planet.resources.join(', ')}</span></div>
       </div>
       <p style="font-size:0.75rem;color:var(--text-muted);margin-top:0.6rem;font-style:italic;">
        Icosphere address: F0.A0 — Surface detail coming soon
       </p>`;
      return;
     }
    }

    panel.style.display = 'none';
   }

   // ── Nearby Stars ──
   function celRenderNearby() {
    const el = document.getElementById('cel-nearby');
    if (!el) return;
    if (celLevel === 'sector') {
     const sorted = STAR_CATALOG.map(s => ({name: s[7] || s[0], dist: Math.sqrt(s[1]**2+s[2]**2+s[3]**2)}))
      .sort((a,b) => a.dist - b.dist).slice(0, 8);
     el.innerHTML = '✨ Nearest: ' + sorted.map(s => `<span style="color:#6699ff;cursor:pointer;" title="${s.dist.toFixed(2)} pc">${s.name} (${s.dist.toFixed(2)} pc)</span>`).join(' · ');
    } else {
     el.innerHTML = '';
    }
   }

   // ── Mouse Interaction ──
   canvas.addEventListener('mousedown', e => {
    celDragging = true;
    const rect = canvas.getBoundingClientRect();
    celDragStart = {x: e.clientX - rect.left, y: e.clientY - rect.top};
    celPanStart = {...celPan};
   });

   canvas.addEventListener('mousemove', e => {
    if (celDragging) {
     const rect = canvas.getBoundingClientRect();
     const mx = e.clientX - rect.left;
     const my = e.clientY - rect.top;
     celPan.x = celPanStart.x + (mx - celDragStart.x);
     celPan.y = celPanStart.y + (my - celDragStart.y);
     celRender();
    }
   });

   canvas.addEventListener('mouseup', e => {
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    const wasDrag = Math.abs(mx - celDragStart.x) > 5 || Math.abs(my - celDragStart.y) > 5;
    celDragging = false;

    if (!wasDrag) {
     // Click detection
     if (celLevel === 'sector') {
      // Find nearest star
      let best = null, bestD = 20;
      for (const sp of sectorStarPositions) {
       const d = Math.sqrt((sp.sx - mx)**2 + (sp.sy - my)**2);
       if (d < bestD) { bestD = d; best = sp; }
      }
      if (best) {
       celSelected = best.name;
       celNavigate('system', best.name);
      }
     } else if (celLevel === 'system') {
      // Find nearest planet
      let best = null, bestD = 20;
      for (const pp of systemPlanetPositions) {
       const d = Math.sqrt((pp.sx - mx)**2 + (pp.sy - my)**2);
       if (d < bestD) { bestD = d; best = pp; }
      }
      if (best) {
       celSelected = best.id;
       celRenderInfo();
      } else {
       celSelected = null;
       celRenderInfo();
      }
     }
    }
    celSave();
   });

   canvas.addEventListener('dblclick', e => {
    if (celLevel === 'system' && celSelected) {
     const planet = PLANETS.find(p => p.id === celSelected);
     if (planet) celNavigate('planet', planet.id);
    }
   });

   canvas.addEventListener('wheel', e => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.15 : 0.87;
    celZoom = Math.max(0.1, Math.min(50, celZoom * factor));
    celSave();
    celRender();
   }, {passive: false});

   // Touch support
   let celTouchDist = 0;
   canvas.addEventListener('touchstart', e => {
    if (e.touches.length === 1) {
     celDragging = true;
     celDragStart = {x: e.touches[0].clientX, y: e.touches[0].clientY};
     celPanStart = {...celPan};
    } else if (e.touches.length === 2) {
     celTouchDist = Math.hypot(e.touches[0].clientX - e.touches[1].clientX, e.touches[0].clientY - e.touches[1].clientY);
    }
   }, {passive:true});
   canvas.addEventListener('touchmove', e => {
    if (e.touches.length === 1 && celDragging) {
     celPan.x = celPanStart.x + (e.touches[0].clientX - celDragStart.x);
     celPan.y = celPanStart.y + (e.touches[0].clientY - celDragStart.y);
     celRender();
    } else if (e.touches.length === 2) {
     const newDist = Math.hypot(e.touches[0].clientX - e.touches[1].clientX, e.touches[0].clientY - e.touches[1].clientY);
     const factor = newDist / celTouchDist;
     celZoom = Math.max(0.1, Math.min(50, celZoom * factor));
     celTouchDist = newDist;
     celRender();
    }
   }, {passive:true});
   canvas.addEventListener('touchend', () => { celDragging = false; celSave(); }, {passive:true});

   // Make celNavigate global for breadcrumbs
   window.celNavigate = celNavigate;


   // Load data then render
   await celLoadData();

   // Initial render
   celSetMode(celMode);
   celRender();
   celRenderBreadcrumb();
   celRenderInfo();
   celRenderNearby();

   // Resize handler
   window.addEventListener('resize', () => { celRender(); });


 })();
