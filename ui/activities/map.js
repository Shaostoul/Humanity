  // ══════════════════════════════════════
  // MAP TAB
  // ══════════════════════════════════════
  (function initMapSystem() {
   // Fix display style for map tab (needs flex, not block)
   const mapTabEl = document.getElementById('tab-map');
   if (!mapTabEl) return;
   const origActive = mapTabEl.classList.contains('active');
   // Override .active display for map tab
   const mapStyle = document.createElement('style');
   mapStyle.textContent = '#tab-map.active { display:flex !important; } .map-mode-btn.active { background:rgba(100,200,150,0.3)!important; border-color:rgba(100,200,150,0.6)!important; color:#fff!important; } @media(max-width:768px){#map-sidebar{display:none!important;}}';
   document.head.appendChild(mapStyle);

   const canvas = document.getElementById('map-canvas');
   if (!canvas) return;
   const ctx = canvas.getContext('2d');

   // ── State ──
   let mapView = 'surface'; // surface | system | sector | galaxy
   let mapPan = {x:0, y:0};
   let mapZoom = 1.0;
   let mapDragging = false;
   let mapDragStart = {x:0,y:0};
   let mapPanStart = {x:0,y:0};
   let mapShowGrid = false;
   let mapShowWeather = false;
   let mapSelectedLocation = null; // {lat, lng}
   let mapInitialized = false;
   let mapWeatherCache = {};
   let mapWeatherTimer = null;
   let mapAnimating = false;

   // ── Home & Pins ──
   function loadHome() {
    try { return JSON.parse(localStorage.getItem('humanity_map_home')); } catch { return null; }
   }
   function saveHome(h) { localStorage.setItem('humanity_map_home', JSON.stringify(h)); }
   function loadPins() {
    try { return JSON.parse(localStorage.getItem('humanity_map_pins')) || []; } catch { return []; }
   }
   function savePins(p) { localStorage.setItem('humanity_map_pins', JSON.stringify(p)); }

   // Default home: Paradise, Mount Rainier
   if (!loadHome()) {
    saveHome({lat:46.7853,lng:-121.7365,name:'Paradise, Mount Rainier',icosphere:'F7.L5.T142'});
   }

   // ── GPS ↔ Icosphere Conversion ──
   // Icosahedron base vertices
   const PHI = (1 + Math.sqrt(5)) / 2;
   const ICO_VERTS = [
    [0,1,PHI],[0,-1,PHI],[0,1,-PHI],[0,-1,-PHI],
    [1,PHI,0],[-1,PHI,0],[1,-PHI,0],[-1,-PHI,0],
    [PHI,0,1],[-PHI,0,1],[PHI,0,-1],[-PHI,0,-1]
   ].map(v => { const l=Math.sqrt(v[0]**2+v[1]**2+v[2]**2); return [v[0]/l,v[1]/l,v[2]/l]; });

   const ICO_FACES = [
    [0,1,8],[0,8,4],[0,4,5],[0,5,9],[0,9,1],
    [1,6,8],[8,6,10],[8,10,4],[4,10,2],[4,2,5],
    [5,2,11],[5,11,9],[9,11,7],[9,7,1],[1,7,6],
    [3,6,7],[3,7,11],[3,11,2],[3,2,10],[3,10,6]
   ];

   function gpsTo3D(lat, lng) {
    const phi = (90 - lat) * Math.PI / 180;
    const theta = (lng + 180) * Math.PI / 180;
    return {
     x: Math.sin(phi) * Math.cos(theta),
     y: Math.cos(phi),
     z: Math.sin(phi) * Math.sin(theta)
    };
   }

   function dot3(a, b) { return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]; }
   function cross3(a, b) { return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]; }
   function norm3(v) { const l=Math.sqrt(v[0]**2+v[1]**2+v[2]**2); return [v[0]/l,v[1]/l,v[2]/l]; }
   function mid3(a,b) { return norm3([(a[0]+b[0])/2,(a[1]+b[1])/2,(a[2]+b[2])/2]); }

   function pointInTriangle3D(p, a, b, c) {
    const n = cross3([b[0]-a[0],b[1]-a[1],b[2]-a[2]],[c[0]-a[0],c[1]-a[1],c[2]-a[2]]);
    const d1 = dot3(cross3([b[0]-a[0],b[1]-a[1],b[2]-a[2]],[p[0]-a[0],p[1]-a[1],p[2]-a[2]]),n);
    const d2 = dot3(cross3([c[0]-b[0],c[1]-b[1],c[2]-b[2]],[p[0]-b[0],p[1]-b[1],p[2]-b[2]]),n);
    const d3 = dot3(cross3([a[0]-c[0],a[1]-c[1],a[2]-c[2]],[p[0]-c[0],p[1]-c[1],p[2]-c[2]]),n);
    return (d1>=0&&d2>=0&&d3>=0)||(d1<=0&&d2<=0&&d3<=0);
   }

   function pointToIcosphere(lat, lng, level) {
    const p3 = gpsTo3D(lat, lng);
    const pt = [p3.x, p3.y, p3.z];
    // Find base face
    let face = 0;
    for (let f = 0; f < 20; f++) {
     const a = ICO_VERTS[ICO_FACES[f][0]];
     const b = ICO_VERTS[ICO_FACES[f][1]];
     const c = ICO_VERTS[ICO_FACES[f][2]];
     if (pointInTriangle3D(pt, a, b, c)) { face = f; break; }
    }
    // Subdivide to find triangle path
    let a = ICO_VERTS[ICO_FACES[face][0]];
    let b = ICO_VERTS[ICO_FACES[face][1]];
    let c = ICO_VERTS[ICO_FACES[face][2]];
    let triIdx = 0;
    let path = [];
    for (let lv = 0; lv < Math.min(level, 8); lv++) {
     const ab = mid3(a,b), bc = mid3(b,c), ca = mid3(c,a);
     // 4 sub-triangles: 0=a-ab-ca, 1=ab-b-bc, 2=ca-bc-c, 3=ab-bc-ca (center)
     if (pointInTriangle3D(pt, a, ab, ca)) { b=ab; c=ca; path.push(0); }
     else if (pointInTriangle3D(pt, ab, b, bc)) { a=ab; c=bc; path.push(1); }
     else if (pointInTriangle3D(pt, ca, bc, c)) { a=ca; b=bc; path.push(2); }
     else { a=ab; b=bc; c=ca; path.push(3); }
    }
    // Compute triangle index from path
    for (let i = 0; i < path.length; i++) triIdx = triIdx * 4 + path[i];
    return { face, level: Math.min(level, 8), triangle: triIdx, address: 'F'+face+'.L'+Math.min(level,8)+'.T'+triIdx };
   }

   function icosphereToGPS(face, level, triangle) {
    let a = ICO_VERTS[ICO_FACES[face][0]];
    let b = ICO_VERTS[ICO_FACES[face][1]];
    let c = ICO_VERTS[ICO_FACES[face][2]];
    // Decode path from triangle index
    let path = [];
    let t = triangle;
    for (let i = 0; i < level; i++) { path.unshift(t % 4); t = Math.floor(t / 4); }
    for (const p of path) {
     const ab = mid3(a,b), bc = mid3(b,c), ca = mid3(c,a);
     if (p===0) { b=ab; c=ca; }
     else if (p===1) { a=ab; c=bc; }
     else if (p===2) { a=ca; b=bc; }
     else { a=ab; b=bc; c=ca; }
    }
    const centroid = norm3([(a[0]+b[0]+c[0])/3,(a[1]+b[1]+c[1])/3,(a[2]+b[2]+c[2])/3]);
    const lat = 90 - Math.acos(centroid[1]) * 180 / Math.PI;
    const lng = Math.atan2(centroid[2], centroid[0]) * 180 / Math.PI - 180;
    return {lat, lng: lng < -180 ? lng + 360 : lng};
   }


   // ── Map data loaded from JSON (replaces inline CITIES, COASTLINES, SKY_STARS, SKY_CONSTELLATIONS, MILKY_WAY_POINTS) ──
   let CITIES = [], COASTLINES = [], SKY_STARS = [], SKY_CONSTELLATIONS = [], MILKY_WAY_POINTS = [];
   let mapDataLoaded = false;

   async function mapLoadData() {
    if (mapDataLoaded) return;
    const [citiesRes, coastRes, starsRes, constsRes, mwRes] = await Promise.all([
     fetch('/data/cities.json'),
     fetch('/data/coastlines.json'),
     fetch('/data/stars-catalog.json'),
     fetch('/data/constellations.json'),
     fetch('/data/milky-way.json')
    ]);
    CITIES = await citiesRes.json();
    COASTLINES = await coastRes.json();
    SKY_STARS = await starsRes.json();
    SKY_CONSTELLATIONS = await constsRes.json();
    MILKY_WAY_POINTS = await mwRes.json();
    // Build name→index lookup for sky stars (used by constellation rendering)
    SKY_STARS.forEach((s, i) => { if (!skyStarMap[s[0]]) skyStarMap[s[0]] = i; });
    mapDataLoaded = true;
   }

   const skyStarMap = {};


   // ── Weather ──
   function weatherEmoji(code) {
    if (code === 0) return '☀️';
    if (code <= 3) return 'â›…';
    if (code <= 48) return '🌫️';
    if (code <= 57) return '🌧️';
    if (code <= 67) return '🌧️';
    if (code <= 77) return '🌨️';
    if (code <= 82) return '⛈️';
    if (code <= 86) return '🌨️';
    if (code >= 95) return '⛈️';
    return '🌡️';
   }

   function weatherDesc(code) {
    if (code===0) return 'Clear sky';
    if (code<=3) return 'Partly cloudy';
    if (code<=48) return 'Fog';
    if (code<=57) return 'Drizzle';
    if (code<=67) return 'Rain';
    if (code<=77) return 'Snow';
    if (code<=82) return 'Showers';
    if (code<=86) return 'Snow showers';
    if (code>=95) return 'Thunderstorm';
    return 'Unknown';
   }

   async function getWeather(lat, lng) {
    const key = lat.toFixed(2)+','+lng.toFixed(2);
    if (mapWeatherCache[key] && Date.now() - mapWeatherCache[key].ts < 900000) return mapWeatherCache[key].data;
    try {
     const url = `https://api.open-meteo.com/v1/forecast?latitude=${lat}&longitude=${lng}&current=temperature_2m,relative_humidity_2m,wind_speed_10m,weather_code,precipitation&daily=temperature_2m_max,temperature_2m_min,weather_code&timezone=auto&forecast_days=3`;
     const res = await fetch(url);
     const data = await res.json();
     mapWeatherCache[key] = {data, ts: Date.now()};
     return data;
    } catch { return null; }
   }

   // ── Equirectangular projection helpers ──
   function latLngToScreen(lat, lng, cw, ch) {
    const x = (lng + 180) / 360 * cw * mapZoom + mapPan.x + cw/2 * (1 - mapZoom);
    const y = (90 - lat) / 180 * ch * mapZoom + mapPan.y + ch/2 * (1 - mapZoom);
    return {x, y};
   }

   function screenToLatLng(sx, sy, cw, ch) {
    const lng = (sx - mapPan.x - cw/2*(1-mapZoom)) / (cw * mapZoom) * 360 - 180;
    const lat = 90 - (sy - mapPan.y - ch/2*(1-mapZoom)) / (ch * mapZoom) * 180;
    return {lat: Math.max(-90,Math.min(90,lat)), lng: ((lng+540)%360)-180};
   }

   // ── Render Surface (Earth Map) ──
   function renderSurface(cw, ch) {
    // Ocean
    ctx.fillStyle = '#0c1a3a';
    ctx.fillRect(0, 0, cw, ch);

    // Draw coastlines
    ctx.strokeStyle = 'rgba(100,200,150,0.5)';
    ctx.fillStyle = 'rgba(30,80,50,0.4)';
    ctx.lineWidth = 1;
    for (const coast of COASTLINES) {
     ctx.beginPath();
     for (let i = 0; i < coast.length; i++) {
      const p = latLngToScreen(coast[i][0], coast[i][1], cw, ch);
      if (i === 0) ctx.moveTo(p.x, p.y);
      else ctx.lineTo(p.x, p.y);
     }
     ctx.closePath();
     ctx.fill();
     ctx.stroke();
    }

    // Lat/lng grid
    ctx.strokeStyle = 'rgba(100,150,200,0.1)';
    ctx.lineWidth = 0.5;
    for (let lat = -60; lat <= 60; lat += 30) {
     const p1 = latLngToScreen(lat, -180, cw, ch);
     const p2 = latLngToScreen(lat, 180, cw, ch);
     ctx.beginPath(); ctx.moveTo(p1.x,p1.y); ctx.lineTo(p2.x,p2.y); ctx.stroke();
    }
    for (let lng = -180; lng <= 180; lng += 30) {
     const p1 = latLngToScreen(90, lng, cw, ch);
     const p2 = latLngToScreen(-90, lng, cw, ch);
     ctx.beginPath(); ctx.moveTo(p1.x,p1.y); ctx.lineTo(p2.x,p2.y); ctx.stroke();
    }

    // Icosphere grid overlay
    if (mapShowGrid) {
     ctx.strokeStyle = 'rgba(150,100,255,0.2)';
     ctx.lineWidth = 0.8;
     // Draw base icosahedron edges projected
     for (const face of ICO_FACES) {
      for (let i = 0; i < 3; i++) {
       const v1 = ICO_VERTS[face[i]];
       const v2 = ICO_VERTS[face[(i+1)%3]];
       // Convert 3D to lat/lng
       const lat1 = 90 - Math.acos(Math.max(-1,Math.min(1,v1[1]))) * 180/Math.PI;
       const lng1 = Math.atan2(v1[2],v1[0]) * 180/Math.PI - 180;
       const lat2 = 90 - Math.acos(Math.max(-1,Math.min(1,v2[1]))) * 180/Math.PI;
       const lng2 = Math.atan2(v2[2],v2[0]) * 180/Math.PI - 180;
       const p1 = latLngToScreen(lat1, lng1 < -180 ? lng1+360 : lng1, cw, ch);
       const p2 = latLngToScreen(lat2, lng2 < -180 ? lng2+360 : lng2, cw, ch);
       // Skip wrap-around edges
       if (Math.abs(p1.x - p2.x) < cw * 0.5) {
        ctx.beginPath(); ctx.moveTo(p1.x,p1.y); ctx.lineTo(p2.x,p2.y); ctx.stroke();
       }
      }
     }
    }

    // Cities
    ctx.font = (mapZoom > 1.5 ? '9' : '7') + 'px sans-serif';
    for (const city of CITIES) {
     const p = latLngToScreen(city.lat, city.lng, cw, ch);
     if (p.x < -10 || p.x > cw+10 || p.y < -10 || p.y > ch+10) continue;
     ctx.fillStyle = 'rgba(255,200,100,0.8)';
     ctx.beginPath(); ctx.arc(p.x, p.y, 2, 0, Math.PI*2); ctx.fill();
     if (mapZoom > 0.8) {
      ctx.fillStyle = 'rgba(255,200,100,0.6)';
      ctx.textAlign = 'center';
      ctx.fillText(city.name, p.x, p.y - 5);
     }
    }

    // Pins
    const pins = loadPins();
    for (const pin of pins) {
     const p = latLngToScreen(pin.lat, pin.lng, cw, ch);
     ctx.fillStyle = '#ff4444';
     ctx.beginPath(); ctx.arc(p.x, p.y, 4, 0, Math.PI*2); ctx.fill();
     ctx.fillStyle = '#fff';
     ctx.textAlign = 'center';
     ctx.font = '8px sans-serif';
     ctx.fillText(pin.name || 'Pin', p.x, p.y - 7);
    }

    // Home marker
    const home = loadHome();
    if (home) {
     const p = latLngToScreen(home.lat, home.lng, cw, ch);
     ctx.font = '14px sans-serif';
     ctx.textAlign = 'center';
     ctx.fillText('🏠', p.x, p.y + 5);
     ctx.font = '8px sans-serif';
     ctx.fillStyle = 'rgba(100,200,150,0.8)';
     ctx.fillText(home.name || 'Home', p.x, p.y - 10);
    }

    // Selected location
    if (mapSelectedLocation) {
     const p = latLngToScreen(mapSelectedLocation.lat, mapSelectedLocation.lng, cw, ch);
     ctx.strokeStyle = '#ff8811';
     ctx.lineWidth = 2;
     ctx.beginPath(); ctx.arc(p.x, p.y, 8, 0, Math.PI*2); ctx.stroke();
     ctx.beginPath(); ctx.moveTo(p.x-12,p.y); ctx.lineTo(p.x+12,p.y); ctx.stroke();
     ctx.beginPath(); ctx.moveTo(p.x,p.y-12); ctx.lineTo(p.x,p.y+12); ctx.stroke();
    }

    // Scale bar
    ctx.fillStyle = 'rgba(200,210,255,0.5)';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'left';
    const kmPerPx = 40075 / (cw * mapZoom);
    let scaleKm = 1000;
    if (kmPerPx < 5) scaleKm = 500;
    if (kmPerPx < 2) scaleKm = 200;
    if (kmPerPx < 1) scaleKm = 100;
    const scalePx = scaleKm / kmPerPx;
    if (scalePx > 20 && scalePx < cw*0.5) {
     ctx.fillRect(10, ch-20, scalePx, 2);
     ctx.fillText(scaleKm + ' km', 10, ch-25);
    }
   }

   // ── Celestial views (reuse from Fantasy tab data via globals) ──
   // We re-implement simplified versions here since the data is in an IIFE

   // Solar System Data (compact inline)
   const MAP_PLANETS = [
    {id:'mercury',name:'Mercury',symbol:'☿',semiMajor:0.387,eccentricity:0.2056,period:87.97,color:'#8c7e6d',type:'terrestrial',meanLongitude:252.25},
    {id:'venus',name:'Venus',symbol:'♀',semiMajor:0.723,eccentricity:0.0068,period:224.7,color:'#e8cda0',type:'terrestrial',meanLongitude:181.98},
    {id:'earth',name:'Earth',symbol:'🌍',semiMajor:1.0,eccentricity:0.0167,period:365.25,color:'#4488ff',type:'terrestrial',meanLongitude:100.46},
    {id:'mars',name:'Mars',symbol:'♂',semiMajor:1.524,eccentricity:0.0934,period:686.97,color:'#cc5533',type:'terrestrial',meanLongitude:355.45},
    {id:'jupiter',name:'Jupiter',symbol:'♃',semiMajor:5.203,eccentricity:0.0489,period:4332.59,color:'#c8a55a',type:'gas_giant',meanLongitude:34.40},
    {id:'saturn',name:'Saturn',symbol:'♄',semiMajor:9.537,eccentricity:0.0565,period:10759.22,color:'#e0c878',type:'gas_giant',meanLongitude:49.94},
    {id:'uranus',name:'Uranus',symbol:'⛢',semiMajor:19.19,eccentricity:0.0457,period:30688.5,color:'#7ec8c8',type:'ice_giant',meanLongitude:313.23},
    {id:'neptune',name:'Neptune',symbol:'♆',semiMajor:30.07,eccentricity:0.0113,period:60182,color:'#4466dd',type:'ice_giant',meanLongitude:304.88}
   ];

   const MAP_STARS = [
    ['Proxima Centauri',-1.55,-1.18,-0.77,'M5.5V',11.13,'Proxima Centauri'],
    ['Alpha Centauri A',-1.55,-1.18,-0.77,'G2V',0.01,'Rigil Kentaurus'],
    ["Barnard's Star",-0.06,-1.82,0.15,'M4V',9.51,"Barnard's Star"],
    ['Wolf 359',1.94,0.99,0.59,'M6V',13.44,'Wolf 359'],
    ['Sirius',1.68,0.25,-1.33,'A1V',-1.46,'Sirius'],
    ['Luyten 726-8',-0.52,-0.37,-2.27,'M5.5V',12.54,'BL Ceti'],
    ['Epsilon Eridani',-2.13,-0.36,-1.90,'K2V',3.73,'Epsilon Eridani'],
    ['Procyon',2.83,-0.87,-2.11,'F5IV-V',0.34,'Procyon'],
    ['61 Cygni',2.17,3.02,1.62,'K5V',5.21,'61 Cygni'],
    ['Tau Ceti',-3.36,0.47,-1.40,'G8.5V',3.49,'Tau Ceti'],
    ['Altair',2.29,4.07,-2.40,'A7V',0.76,'Altair'],
    ['Vega',0.69,6.19,0.21,'A0Va',0.03,'Vega'],
    ['Fomalhaut',-4.23,1.30,-5.01,'A3V',1.16,'Fomalhaut'],
    ['Gliese 581',-1.75,-3.29,-3.21,'M3V',10.56,'Gliese 581'],
    ['Gliese 667C',-3.68,-2.53,-2.57,'M1.5V',10.22,'Gliese 667C'],
    ['40 Eridani',-3.50,-0.29,-3.79,'K0.5V',4.43,'40 Eridani'],
    ['Sigma Draconis',2.44,4.82,1.20,'G9V',4.68,'Sigma Draconis'],
    ['Eta Cassiopeiae',5.11,1.54,1.64,'G3V',3.44,'Eta Cassiopeiae'],
    ['82 Eridani',-5.03,0.71,-2.28,'G8V',4.27,'82 Eridani'],
    ['Delta Pavonis',-2.48,0.69,-5.27,'G8IV',3.55,'Delta Pavonis']
   ];

   function mapSpectralColor(sp) {
    if (!sp) return '#fff';
    const c = sp[0].toUpperCase();
    return {O:'#9bb0ff',B:'#aabfff',A:'#cad7ff',F:'#f8f7ff',G:'#fff4ea',K:'#ffd2a1',M:'#ffcc6f',D:'#fff'}[c]||'#fff';
   }

   function renderSystem(cw, ch) {
    ctx.fillStyle = '#05050f';
    ctx.fillRect(0, 0, cw, ch);
    // Background stars
    let bgSeed = 99;
    const bgRng = () => { bgSeed=(bgSeed*16807)%2147483647; return (bgSeed-1)/2147483646; };
    for (let i=0;i<150;i++){
     ctx.fillStyle = `rgba(200,210,255,${bgRng()*0.3+0.1})`;
     ctx.beginPath(); ctx.arc(bgRng()*cw,bgRng()*ch,bgRng()*0.8+0.2,0,Math.PI*2); ctx.fill();
    }

    const cx = cw/2 + mapPan.x;
    const cy = ch/2 + mapPan.y;
    function auToR(au){ return (30+Math.log2(au+0.1)*35)*mapZoom; }

    // Sun
    ctx.fillStyle='#FFF5E0';
    ctx.beginPath(); ctx.arc(cx,cy,8*Math.min(mapZoom,2),0,Math.PI*2); ctx.fill();
    const sg=ctx.createRadialGradient(cx,cy,0,cx,cy,20*Math.min(mapZoom,2));
    sg.addColorStop(0,'rgba(255,245,224,0.3)'); sg.addColorStop(1,'transparent');
    ctx.fillStyle=sg; ctx.beginPath(); ctx.arc(cx,cy,20*Math.min(mapZoom,2),0,Math.PI*2); ctx.fill();
    ctx.fillStyle='rgba(255,245,224,0.8)'; ctx.font='10px sans-serif'; ctx.textAlign='center'; ctx.fillText('☀ Sol',cx,cy-14*Math.min(mapZoom,2));

    const now=Date.now();
    const J2000=Date.UTC(2000,0,1,12,0,0);
    const days=(now-J2000)/86400000;
    for (const p of MAP_PLANETS) {
     const orbitR=auToR(p.semiMajor);
     ctx.strokeStyle='rgba(100,150,255,0.12)'; ctx.lineWidth=0.8;
     ctx.beginPath(); ctx.ellipse(cx,cy,orbitR,orbitR*(1-p.eccentricity*0.3),0,0,Math.PI*2); ctx.stroke();
     const angle=((p.meanLongitude||0)+(360/p.period)*days)%360*Math.PI/180;
     const px=cx+Math.cos(angle)*orbitR;
     const py=cy+Math.sin(angle)*orbitR*(1-p.eccentricity*0.3);
     let pr=p.type==='gas_giant'?6:p.type==='ice_giant'?5:3.5;
     pr*=Math.min(mapZoom,2);
     ctx.fillStyle=p.color; ctx.beginPath(); ctx.arc(px,py,pr,0,Math.PI*2); ctx.fill();
     ctx.fillStyle='rgba(200,210,255,0.7)'; ctx.font='9px sans-serif'; ctx.textAlign='center';
     ctx.fillText(p.symbol+' '+p.name,px,py-pr-4);
    }
   }

   function renderSector(cw, ch) {
    ctx.fillStyle='#05050f'; ctx.fillRect(0,0,cw,ch);
    let bgSeed=12345;
    const bgRng=()=>{bgSeed=(bgSeed*16807)%2147483647;return(bgSeed-1)/2147483646;};
    for(let i=0;i<200;i++){ctx.fillStyle=`rgba(200,210,255,${bgRng()*0.4+0.1})`;ctx.beginPath();ctx.arc(bgRng()*cw,bgRng()*ch,bgRng()*0.8+0.2,0,Math.PI*2);ctx.fill();}

    const cx=cw/2+mapPan.x, cy=ch/2+mapPan.y;
    const scale=25*mapZoom;

    // Grid
    ctx.strokeStyle='rgba(50,70,120,0.15)'; ctx.lineWidth=0.5;
    const gPx=5*scale;
    if(gPx>20){for(let gx=cx%gPx;gx<cw;gx+=gPx){ctx.beginPath();ctx.moveTo(gx,0);ctx.lineTo(gx,ch);ctx.stroke();}for(let gy=cy%gPx;gy<ch;gy+=gPx){ctx.beginPath();ctx.moveTo(0,gy);ctx.lineTo(cw,gy);ctx.stroke();}}

    // Sol
    function drawStar(x,y,color,radius,label){
     const g=ctx.createRadialGradient(x,y,0,x,y,radius*3);g.addColorStop(0,color);g.addColorStop(1,'transparent');
     ctx.fillStyle=g;ctx.beginPath();ctx.arc(x,y,radius*3,0,Math.PI*2);ctx.fill();
     ctx.fillStyle=color;ctx.beginPath();ctx.arc(x,y,radius,0,Math.PI*2);ctx.fill();
     if(label){ctx.fillStyle='rgba(200,210,255,0.7)';ctx.font='9px sans-serif';ctx.textAlign='center';ctx.fillText(label,x,y-radius-4);}
    }
    drawStar(cx,cy,'#FFF5E0',4,'Sol');

    for(const s of MAP_STARS){
     const sx=cx+s[1]*scale, sy=cy-s[3]*scale;
     const col=mapSpectralColor(s[4]);
     const r=Math.max(1.5,Math.min(4,(8-s[5])*0.3))*Math.min(mapZoom,2);
     if(sx>-20&&sx<cw+20&&sy>-20&&sy<ch+20) drawStar(sx,sy,col,r,mapZoom>0.8?(s[6]||''):'');
    }

    ctx.fillStyle='rgba(100,150,255,0.5)'; ctx.font='10px sans-serif'; ctx.textAlign='left';
    const sBarPx=5*scale;
    if(sBarPx>20&&sBarPx<cw*0.6){ctx.fillRect(10,ch-20,sBarPx,2);ctx.fillText('5 pc',10,ch-25);}
   }

   function renderGalaxy(cw, ch) {
    ctx.fillStyle='#020208'; ctx.fillRect(0,0,cw,ch);
    // Background stars
    let bgSeed=777;
    const bgRng=()=>{bgSeed=(bgSeed*16807)%2147483647;return(bgSeed-1)/2147483646;};
    for(let i=0;i<300;i++){ctx.fillStyle=`rgba(200,210,255,${bgRng()*0.2+0.05})`;ctx.beginPath();ctx.arc(bgRng()*cw,bgRng()*ch,bgRng()*0.6+0.1,0,Math.PI*2);ctx.fill();}

    const cx=cw/2+mapPan.x, cy=ch/2+mapPan.y;
    // Stylized spiral galaxy
    ctx.save();
    for(let arm=0;arm<4;arm++){
     const armAngle=arm*Math.PI/2;
     for(let t=0;t<200;t++){
      const r=(t/200)*Math.min(cw,ch)*0.4*mapZoom;
      const a=armAngle+t*0.05;
      const x=cx+Math.cos(a)*r;
      const y=cy+Math.sin(a)*r*0.4; // edge-on tilt
      const spread=bgRng()*20-10;
      const alpha=Math.max(0.02,(1-t/200)*0.4);
      ctx.fillStyle=`rgba(180,160,220,${alpha})`;
      ctx.beginPath();ctx.arc(x+spread,y+spread*0.4,bgRng()*2+0.5,0,Math.PI*2);ctx.fill();
     }
    }
    // Central bulge
    const bulge=ctx.createRadialGradient(cx,cy,0,cx,cy,30*mapZoom);
    bulge.addColorStop(0,'rgba(255,240,200,0.5)');bulge.addColorStop(1,'transparent');
    ctx.fillStyle=bulge;ctx.beginPath();ctx.arc(cx,cy,30*mapZoom,0,Math.PI*2);ctx.fill();
    // "You are here" marker
    const solX=cx+60*mapZoom, solY=cy+5*mapZoom;
    ctx.fillStyle='#ff4444';ctx.beginPath();ctx.arc(solX,solY,3,0,Math.PI*2);ctx.fill();
    ctx.fillStyle='rgba(255,100,100,0.8)';ctx.font='9px sans-serif';ctx.textAlign='left';
    ctx.fillText('← You are here (Sol)',solX+6,solY+3);
    ctx.restore();

    // Label
    ctx.fillStyle='rgba(200,210,255,0.4)';ctx.font='12px sans-serif';ctx.textAlign='center';
    ctx.fillText('Milky Way Galaxy',cx,ch-15);
    ctx.fillStyle='rgba(200,210,255,0.25)';ctx.font='10px sans-serif';
    ctx.fillText('~200 billion stars · ~100,000 ly diameter',cx,ch-3);
   }

   // ══════════════════════════════════════════════════════════════
   // ██ SKY VIEW — Real night sky with 88 IAU constellations ██
   // ══════════════════════════════════════════════════════════════

   // ── Sky View State ──
   let skyTime = new Date();
   let skyTimeOffset = 0; // ms offset from real time
   let skyTimeSpeed = 0; // 0=paused at offset, or multiplier
   let skyShowConstellations = true;
   let skyShowMilkyWay = true;
   let skyDarkSky = true; // false = city sky (only bright stars)
   let skySelectedConstellation = null;
   let skyHoveredConstellation = null;
   let skyAnimFrame = null;

   function skyGetTime() {
    if (skyTimeSpeed === 0) return new Date(Date.now() + skyTimeOffset);
    return new Date(Date.now() + skyTimeOffset);
   }

   // ── Astronomical Math ──
   function julianDate(date) { return date.getTime() / 86400000 + 2440587.5; }

   function raDecToAltAz(ra, dec, lat, lng, date) {
    const jd = julianDate(date);
    const T = (jd - 2451545.0) / 36525.0;
    const GMST = (280.46061837 + 360.98564736629 * (jd - 2451545.0) + 0.000387933 * T * T) % 360;
    const LST = ((GMST + lng) % 360 + 360) % 360;
    const HA = ((LST - ra * 15) % 360 + 360) % 360;
    const haRad = HA * Math.PI / 180;
    const decRad = dec * Math.PI / 180;
    const latRad = lat * Math.PI / 180;
    const sinAlt = Math.sin(decRad) * Math.sin(latRad) + Math.cos(decRad) * Math.cos(latRad) * Math.cos(haRad);
    const alt = Math.asin(sinAlt) * 180 / Math.PI;
    const cosAz = (Math.sin(decRad) - Math.sin(alt * Math.PI / 180) * Math.sin(latRad)) / (Math.cos(alt * Math.PI / 180) * Math.cos(latRad));
    let az = Math.acos(Math.max(-1, Math.min(1, cosAz))) * 180 / Math.PI;
    if (Math.sin(haRad) > 0) az = 360 - az;
    return { altitude: alt, azimuth: az };
   }

   function altAzToCanvas(alt, az, cx, cy, radius) {
    const r = radius * (1 - alt / 90);
    const theta = (az - 180) * Math.PI / 180;
    return { x: cx + r * Math.sin(theta), y: cy - r * Math.cos(theta) };
   }

   function getSunRADec(date) {
    const jd = julianDate(date);
    const n = jd - 2451545.0;
    const L = (280.460 + 0.9856474 * n) % 360;
    const g = ((357.528 + 0.9856003 * n) % 360) * Math.PI / 180;
    const lambda = (L + 1.915 * Math.sin(g) + 0.020 * Math.sin(2*g)) % 360;
    const eps = 23.439 - 0.0000004 * n;
    const lRad = lambda * Math.PI / 180;
    const eRad = eps * Math.PI / 180;
    const ra = Math.atan2(Math.cos(eRad) * Math.sin(lRad), Math.cos(lRad)) * 180 / Math.PI / 15;
    const dec = Math.asin(Math.sin(eRad) * Math.sin(lRad)) * 180 / Math.PI;
    return { ra: ((ra % 24) + 24) % 24, dec };
   }

   function getMoonRADec(date) {
    const jd = julianDate(date);
    const T = (jd - 2451545.0) / 36525.0;
    const L0 = (218.3165 + 481267.8813 * T) % 360;
    const M = (134.9634 + 477198.8676 * T) % 360 * Math.PI / 180;
    const F = (93.2721 + 483202.0175 * T) % 360 * Math.PI / 180;
    const lng = L0 + 6.289 * Math.sin(M);
    const lat = 5.128 * Math.sin(F);
    const eps = 23.439 - 0.0000004 * (jd - 2451545.0);
    const lRad = lng * Math.PI / 180;
    const bRad = lat * Math.PI / 180;
    const eRad = eps * Math.PI / 180;
    const ra = Math.atan2(Math.sin(lRad)*Math.cos(eRad) - Math.tan(bRad)*Math.sin(eRad), Math.cos(lRad)) * 180 / Math.PI / 15;
    const dec = Math.asin(Math.sin(bRad)*Math.cos(eRad) + Math.cos(bRad)*Math.sin(eRad)*Math.sin(lRad)) * 180 / Math.PI;
    return { ra: ((ra % 24) + 24) % 24, dec };
   }

   function getMoonPhase(date) {
    const knownNewMoon = new Date('2024-01-11T11:57:00Z').getTime();
    const cycle = 29.53058867;
    const daysSince = (date.getTime() - knownNewMoon) / 86400000;
    const phase = ((daysSince % cycle) + cycle) % cycle;
    return phase / cycle;
   }

   // ── Star Catalog: ~300 brightest stars (mag < 4.0) ──
   // Format: [name, RA(hours), Dec(degrees), magnitude, spectral]

   // ── Stargazer XP System ──
   function loadStargazer() { try { return JSON.parse(localStorage.getItem('humanity_stargazer')) || { identified: [], xp: 0 }; } catch { return { identified: [], xp: 0 }; } }
   function saveStargazer(data) { localStorage.setItem('humanity_stargazer', JSON.stringify(data)); }

   window.skyIdentifyConstellation = function(name) {
    const sg = loadStargazer();
    if (sg.identified.includes(name)) return;
    sg.identified.push(name);
    sg.xp += 10;
    saveStargazer(sg);
    skySelectedConstellation = name;
    updateSidebar();
   };

   // ── Sky View Rendering ──
   function renderSkyView(cw, ch) {
    const date = skyGetTime();
    const home = loadHome();
    const lat = home ? home.lat : 46.7853;
    const lng = home ? home.lng : -121.7365;

    // Sun position for day/night
    const sunRD = getSunRADec(date);
    const sunPos = raDecToAltAz(sunRD.ra, sunRD.dec, lat, lng, date);
    const sunAlt = sunPos.altitude;

    // Sky background based on sun altitude
    let skyDarkness = 1; // 1 = full dark
    if (sunAlt > 0) skyDarkness = 0;
    else if (sunAlt > -6) skyDarkness = (-sunAlt) / 6 * 0.5; // civil twilight
    else if (sunAlt > -12) skyDarkness = 0.5 + (-sunAlt - 6) / 6 * 0.3; // nautical
    else if (sunAlt > -18) skyDarkness = 0.8 + (-sunAlt - 12) / 6 * 0.2; // astronomical
    else skyDarkness = 1;

    // Background gradient
    const cx = cw / 2, cy = ch / 2;
    const radius = Math.min(cw, ch) * 0.45;

    // Sky gradient from zenith to horizon
    if (sunAlt > -6) {
     // Twilight/day colors
     const dayFrac = Math.max(0, Math.min(1, (sunAlt + 6) / 20));
     const r1 = Math.round(10 + dayFrac * 100);
     const g1 = Math.round(10 + dayFrac * 140);
     const b1 = Math.round(30 + dayFrac * 200);
     const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius * 1.1);
     grad.addColorStop(0, `rgb(${r1},${g1},${b1})`);
     const r2 = Math.round(20 + dayFrac * 180);
     const g2 = Math.round(15 + dayFrac * 120);
     const b2 = Math.round(40 + dayFrac * 80);
     grad.addColorStop(1, `rgb(${r2},${g2},${b2})`);
     ctx.fillStyle = grad;
    } else {
     const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius * 1.1);
     grad.addColorStop(0, '#0a0a1a');
     grad.addColorStop(0.7, '#080818');
     grad.addColorStop(1, '#0c0c25');
     ctx.fillStyle = grad;
    }
    ctx.fillRect(0, 0, cw, ch);

    // Clip to circular sky area
    ctx.save();
    ctx.beginPath();
    ctx.arc(cx, cy, radius + 2, 0, Math.PI * 2);
    ctx.clip();

    // Milky Way band
    if (skyShowMilkyWay && skyDarkness > 0.5) {
     ctx.save();
     const mwAlpha = (skyDarkness - 0.5) * 0.15;
     for (let i = 0; i < MILKY_WAY_POINTS.length - 1; i++) {
      const p1 = MILKY_WAY_POINTS[i];
      const p2 = MILKY_WAY_POINTS[i + 1];
      const pos1 = raDecToAltAz(p1[0], p1[1], lat, lng, date);
      const pos2 = raDecToAltAz(p2[0], p2[1], lat, lng, date);
      if (pos1.altitude < -5 && pos2.altitude < -5) continue;
      const c1 = altAzToCanvas(Math.max(0, pos1.altitude), pos1.azimuth, cx, cy, radius);
      const c2 = altAzToCanvas(Math.max(0, pos2.altitude), pos2.azimuth, cx, cy, radius);
      const w = (p1[2] + p2[2]) * 0.5 * radius / 90;
      const dx = c2.x - c1.x, dy = c2.y - c1.y;
      const len = Math.sqrt(dx * dx + dy * dy);
      if (len < 1) continue;
      const nx = -dy / len * w, ny = dx / len * w;
      const mwGrad = ctx.createLinearGradient(c1.x + nx, c1.y + ny, c1.x - nx, c1.y - ny);
      mwGrad.addColorStop(0, 'transparent');
      mwGrad.addColorStop(0.3, `rgba(180,190,220,${mwAlpha})`);
      mwGrad.addColorStop(0.5, `rgba(200,210,240,${mwAlpha * 1.3})`);
      mwGrad.addColorStop(0.7, `rgba(180,190,220,${mwAlpha})`);
      mwGrad.addColorStop(1, 'transparent');
      ctx.fillStyle = mwGrad;
      ctx.beginPath();
      ctx.moveTo(c1.x + nx, c1.y + ny);
      ctx.lineTo(c2.x + nx, c2.y + ny);
      ctx.lineTo(c2.x - nx, c2.y - ny);
      ctx.lineTo(c1.x - nx, c1.y - ny);
      ctx.closePath();
      ctx.fill();
     }
     ctx.restore();
    }

    // Procedural faint background stars (galactic plane weighted)
    if (skyDarkness > 0.3 && skyDarkSky) {
     let seed = 42;
     const rng = () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
     const count = Math.round(800 * skyDarkness);
     for (let i = 0; i < count; i++) {
      const fakeRA = rng() * 24;
      // Weight toward galactic plane (dec ~-30 to +60 band)
      let fakeDec = (rng() - 0.3) * 180;
      if (rng() < 0.4) fakeDec = rng() * 60 - 30; // galactic plane bias
      fakeDec = Math.max(-90, Math.min(90, fakeDec));
      const pos = raDecToAltAz(fakeRA, fakeDec, lat, lng, date);
      if (pos.altitude < 2) continue;
      const pt = altAzToCanvas(pos.altitude, pos.azimuth, cx, cy, radius);
      const mag = 4 + rng() * 2;
      const sz = Math.max(0.3, 2 - mag * 0.3);
      const alpha = skyDarkness * (0.1 + rng() * 0.25);
      ctx.fillStyle = `rgba(200,210,230,${alpha})`;
      ctx.beginPath();
      ctx.arc(pt.x, pt.y, sz, 0, Math.PI * 2);
      ctx.fill();
     }
    }

    // Named stars
    const magLimit = skyDarkSky ? 6.0 : 3.0;
    const starScreenPos = []; // for constellation lines
    const spectralColors = {O:'#9bb0ff',B:'#aabfff',A:'#cad7ff',F:'#f8f7ff',G:'#fff4ea',K:'#ffd2a1',M:'#ffcc6f'};

    for (let i = 0; i < SKY_STARS.length; i++) {
     const s = SKY_STARS[i];
     if (s[3] > magLimit) continue;
     if (s[3] > 2 && skyDarkness < 0.3) continue;
     if (s[3] > 0 && skyDarkness < 0.1) continue;

     const pos = raDecToAltAz(s[1], s[2], lat, lng, date);
     if (pos.altitude < 0) { starScreenPos[i] = null; continue; }

     const pt = altAzToCanvas(pos.altitude, pos.azimuth, cx, cy, radius);
     const sz = Math.max(0.5, 4 - s[3] * 0.5) * Math.min(skyDarkness + 0.3, 1);
     const col = spectralColors[s[4]] || '#fff';
     starScreenPos[i] = pt;

     // Glow for bright stars
     if (s[3] < 2 && skyDarkness > 0.3) {
      const glowR = sz * 4;
      const glow = ctx.createRadialGradient(pt.x, pt.y, 0, pt.x, pt.y, glowR);
      glow.addColorStop(0, col.replace(')', `,${0.3 * skyDarkness})`).replace('rgb', 'rgba'));
      glow.addColorStop(1, 'transparent');
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(pt.x, pt.y, glowR, 0, Math.PI * 2);
      ctx.fill();
     }

     ctx.fillStyle = col;
     ctx.beginPath();
     ctx.arc(pt.x, pt.y, sz, 0, Math.PI * 2);
     ctx.fill();

     // Label bright stars
     if (s[3] < 2 && skyDarkness > 0.2) {
      ctx.fillStyle = `rgba(200,210,230,${0.5 * skyDarkness})`;
      ctx.font = '9px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(s[0], pt.x + sz + 3, pt.y + 3);
     }
    }

    // Constellation lines
    if (skyShowConstellations && skyDarkness > 0.2) {
     for (const con of SKY_CONSTELLATIONS) {
      if (!con.lines || con.lines.length === 0) continue;
      const isHovered = skyHoveredConstellation === con.name;
      const isSelected = skySelectedConstellation === con.name;
      const alpha = isHovered || isSelected ? 0.6 : 0.25;
      ctx.strokeStyle = `rgba(100,180,255,${alpha * skyDarkness})`;
      ctx.lineWidth = isHovered || isSelected ? 1.5 : 0.8;

      let conCx = 0, conCy = 0, conCount = 0;
      let anyVisible = false;

      for (const line of con.lines) {
       if (line.length < 2) continue;
       const i1 = skyStarMap[line[0]];
       const i2 = skyStarMap[line[1]];
       if (i1 === undefined || i2 === undefined) continue;

       // Calculate positions on-the-fly if not cached
       let p1 = starScreenPos[i1];
       let p2 = starScreenPos[i2];
       if (!p1) {
        const s = SKY_STARS[i1];
        const pos = raDecToAltAz(s[1], s[2], lat, lng, date);
        if (pos.altitude > 0) { p1 = altAzToCanvas(pos.altitude, pos.azimuth, cx, cy, radius); starScreenPos[i1] = p1; }
       }
       if (!p2) {
        const s = SKY_STARS[i2];
        const pos = raDecToAltAz(s[1], s[2], lat, lng, date);
        if (pos.altitude > 0) { p2 = altAzToCanvas(pos.altitude, pos.azimuth, cx, cy, radius); starScreenPos[i2] = p2; }
       }
       if (!p1 || !p2) continue;

       anyVisible = true;
       ctx.beginPath();
       ctx.moveTo(p1.x, p1.y);
       ctx.lineTo(p2.x, p2.y);
       ctx.stroke();

       conCx += p1.x + p2.x;
       conCy += p1.y + p2.y;
       conCount += 2;
      }

      // Also handle single-star constellations for label
      if (con.lines.length === 1 && con.lines[0].length === 1) {
       const idx = skyStarMap[con.lines[0][0]];
       if (idx !== undefined && starScreenPos[idx]) {
        anyVisible = true;
        conCx = starScreenPos[idx].x;
        conCy = starScreenPos[idx].y;
        conCount = 1;
       }
      }

      // Constellation label
      if (anyVisible && conCount > 0) {
       const lx = conCx / conCount, ly = conCy / conCount;
       ctx.fillStyle = `rgba(100,180,255,${(isHovered || isSelected ? 0.7 : 0.3) * skyDarkness})`;
       ctx.font = `${isHovered || isSelected ? '11px' : '9px'} sans-serif`;
       ctx.textAlign = 'center';
       ctx.fillText(con.name, lx, ly - 8);
      }
     }
    }

    // Sun
    if (sunAlt > -5) {
     const sunPt = altAzToCanvas(Math.max(0, sunAlt), sunPos.azimuth, cx, cy, radius);
     if (sunAlt > 0) {
      const sunGlow = ctx.createRadialGradient(sunPt.x, sunPt.y, 0, sunPt.x, sunPt.y, 30);
      sunGlow.addColorStop(0, 'rgba(255,240,180,0.9)');
      sunGlow.addColorStop(0.3, 'rgba(255,200,100,0.4)');
      sunGlow.addColorStop(1, 'transparent');
      ctx.fillStyle = sunGlow;
      ctx.beginPath(); ctx.arc(sunPt.x, sunPt.y, 30, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = '#FFF5E0';
      ctx.beginPath(); ctx.arc(sunPt.x, sunPt.y, 8, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = 'rgba(255,240,200,0.8)'; ctx.font = '10px sans-serif'; ctx.textAlign = 'center';
      ctx.fillText('☀ Sun', sunPt.x, sunPt.y - 14);
     }
    }

    // Moon
    const moonRD = getMoonRADec(date);
    const moonPos = raDecToAltAz(moonRD.ra, moonRD.dec, lat, lng, date);
    if (moonPos.altitude > 0) {
     const moonPt = altAzToCanvas(moonPos.altitude, moonPos.azimuth, cx, cy, radius);
     const phase = getMoonPhase(date);
     const moonR = 8;
     // Draw moon body
     ctx.fillStyle = '#e8e0c8';
     ctx.beginPath(); ctx.arc(moonPt.x, moonPt.y, moonR, 0, Math.PI * 2); ctx.fill();
     // Phase shadow
     ctx.fillStyle = 'rgba(10,10,26,0.85)';
     ctx.beginPath();
     const illumination = phase < 0.5 ? phase * 2 : (1 - phase) * 2;
     const shadowX = (phase < 0.5) ? moonR * (1 - illumination * 2) : -moonR * (1 - illumination * 2);
     ctx.ellipse(moonPt.x, moonPt.y, Math.abs(shadowX) || 0.1, moonR, 0, -Math.PI / 2, Math.PI / 2, phase > 0.5);
     ctx.arc(moonPt.x, moonPt.y, moonR, Math.PI / 2, -Math.PI / 2, phase < 0.25 || phase > 0.75);
     ctx.fill();
     // Glow
     const moonGlow = ctx.createRadialGradient(moonPt.x, moonPt.y, moonR, moonPt.x, moonPt.y, moonR * 3);
     moonGlow.addColorStop(0, `rgba(200,200,180,${0.15 * illumination})`);
     moonGlow.addColorStop(1, 'transparent');
     ctx.fillStyle = moonGlow;
     ctx.beginPath(); ctx.arc(moonPt.x, moonPt.y, moonR * 3, 0, Math.PI * 2); ctx.fill();
     ctx.fillStyle = 'rgba(220,215,200,0.7)'; ctx.font = '9px sans-serif'; ctx.textAlign = 'center';
     ctx.fillText('☽ Moon', moonPt.x, moonPt.y - moonR - 4);
    }

    ctx.restore(); // End clip

    // Horizon circle
    ctx.strokeStyle = 'rgba(100,200,150,0.4)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(cx, cy, radius, 0, Math.PI * 2);
    ctx.stroke();

    // Horizon gradient (ground below)
    const hGrad = ctx.createRadialGradient(cx, cy, radius - 2, cx, cy, radius + 20);
    hGrad.addColorStop(0, 'transparent');
    hGrad.addColorStop(0.5, 'rgba(30,50,30,0.3)');
    hGrad.addColorStop(1, 'rgba(20,30,20,0.5)');
    ctx.fillStyle = hGrad;
    ctx.beginPath();
    // Fill outside the circle
    ctx.rect(0, 0, cw, ch);
    ctx.arc(cx, cy, radius, 0, Math.PI * 2, true);
    ctx.fill();

    // Cardinal directions
    ctx.fillStyle = 'rgba(100,200,150,0.7)';
    ctx.font = 'bold 12px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('N', cx, cy - radius - 6);
    ctx.fillText('S', cx, cy + radius + 15);
    ctx.fillText('E', cx + radius + 10, cy + 4);
    ctx.fillText('W', cx - radius - 10, cy + 4);

    // Zenith marker
    ctx.fillStyle = 'rgba(200,210,255,0.3)';
    ctx.font = '8px sans-serif';
    ctx.fillText('zenith', cx, cy - 5);
    ctx.fillStyle = 'rgba(200,210,255,0.15)';
    ctx.beginPath(); ctx.arc(cx, cy, 2, 0, Math.PI * 2); ctx.fill();

    // Time display
    const timeStr = date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    const dateStr = date.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });
    ctx.fillStyle = 'rgba(200,210,255,0.7)';
    ctx.font = '11px monospace';
    ctx.textAlign = 'left';
    ctx.fillText(`${dateStr} ${timeStr}`, 10, ch - 25);
    ctx.fillStyle = 'rgba(200,210,255,0.4)';
    ctx.font = '9px sans-serif';
    const locName = home ? home.name : 'Default';
    ctx.fillText(`📍 ${locName} (${lat.toFixed(2)}°, ${lng.toFixed(2)}°)`, 10, ch - 10);

    // Sun altitude indicator
    const sunLabel = sunAlt > 0 ? '☀ Daytime' : sunAlt > -6 ? '🌅 Civil Twilight' : sunAlt > -12 ? '🌆 Nautical Twilight' : sunAlt > -18 ? '🌃 Astronomical Twilight' : '🌑 Full Night';
    ctx.fillStyle = 'rgba(200,210,255,0.5)';
    ctx.font = '9px sans-serif';
    ctx.textAlign = 'right';
    ctx.fillText(sunLabel, cw - 10, ch - 10);

    // Stargazer progress
    const sg = loadStargazer();
    ctx.fillText(`⭐ ${sg.identified.length}/88 constellations`, cw - 10, ch - 25);
   }

   // ── Sky View mouse interaction ──
   canvas.addEventListener('mousemove', function skyMouseMove(e) {
    if (mapView !== 'skyview') return;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left, my = e.clientY - rect.top;
    const cw = canvas.clientWidth, ch = canvas.clientHeight;
    const cx = cw / 2, cy = ch / 2;
    const radius = Math.min(cw, ch) * 0.45;
    const date = skyGetTime();
    const home = loadHome();
    const lat = home ? home.lat : 46.7853;
    const lng = home ? home.lng : -121.7365;

    let closest = null, closestDist = 30;
    for (const con of SKY_CONSTELLATIONS) {
     if (!con.lines || con.lines.length === 0) continue;
     let conCx = 0, conCy = 0, count = 0;
     for (const line of con.lines) {
      for (const starName of line) {
       const idx = skyStarMap[starName];
       if (idx === undefined) continue;
       const s = SKY_STARS[idx];
       const pos = raDecToAltAz(s[1], s[2], lat, lng, date);
       if (pos.altitude <= 0) continue;
       const pt = altAzToCanvas(pos.altitude, pos.azimuth, cx, cy, radius);
       conCx += pt.x; conCy += pt.y; count++;
      }
     }
     if (count === 0) continue;
     conCx /= count; conCy /= count;
     const d = Math.sqrt((mx - conCx) ** 2 + (my - conCy) ** 2);
     if (d < closestDist) { closest = con.name; closestDist = d; }
    }
    if (closest !== skyHoveredConstellation) {
     skyHoveredConstellation = closest;
     mapRender();
    }
   });

   canvas.addEventListener('click', function skyClick(e) {
    if (mapView !== 'skyview') return;
    if (skyHoveredConstellation) {
     skySelectedConstellation = skyHoveredConstellation;
     updateSidebar();
     mapRender();
    }
   });

   // ── Sky View time controls ──
   window.skySetTimeOffset = function(hours) {
    skyTimeOffset += hours * 3600000;
    mapRender();
   };
   window.skyResetTime = function() {
    skyTimeOffset = 0;
    mapRender();
   };
   window.skyToggleConstellations = function() {
    skyShowConstellations = !skyShowConstellations;
    mapRender();
   };
   window.skyToggleMilkyWay = function() {
    skyShowMilkyWay = !skyShowMilkyWay;
    mapRender();
   };
   window.skyToggleDarkSky = function() {
    skyDarkSky = !skyDarkSky;
    mapRender();
   };

   // Auto-animate sky (update every 30s for real-time movement)
   setInterval(() => {
    if (mapView === 'skyview' && document.getElementById('tab-map').style.display !== 'none') {
     mapRender();
    }
   }, 30000);

   // ══════════════════════════════════════════════════════════════
   // ██ END SKY VIEW                      ██
   // ══════════════════════════════════════════════════════════════

   // ── Main render ──
   function mapRender() {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvas.clientWidth * dpr;
    canvas.height = canvas.clientHeight * dpr;
    ctx.setTransform(dpr,0,0,dpr,0,0);
    const cw = canvas.clientWidth, ch = canvas.clientHeight;

    if (mapView === 'surface') renderSurface(cw, ch);
    else if (mapView === 'system') renderSystem(cw, ch);
    else if (mapView === 'sector') renderSector(cw, ch);
    else if (mapView === 'galaxy') renderGalaxy(cw, ch);
    else if (mapView === 'skyview') renderSkyView(cw, ch);
   }

   // ── View switching with animation ──
   const viewNames = {surface:'🌍 Earth Surface',system:'☀️ Solar System',sector:'⭐ Stellar Neighborhood',galaxy:'🌌 Milky Way Galaxy',skyview:'🔭 Sky View'};
   window.mapSetView = function(view) {
    if (view === mapView) return;
    // Animation: brief darken
    mapAnimating = true;
    const canvasEl = canvas;
    canvasEl.style.transition = 'opacity 0.2s';
    canvasEl.style.opacity = '0.3';
    setTimeout(() => {
     mapView = view;
     mapPan = {x:0,y:0};
     mapZoom = 1.0;
     // Update mode buttons
     document.querySelectorAll('.map-mode-btn').forEach(b => b.classList.toggle('active', b.dataset.mode === view));
     mapRender();
     updateSidebar();
     canvasEl.style.opacity = '1';
     setTimeout(() => { canvasEl.style.transition = ''; mapAnimating = false; }, 200);
    }, 200);
   };

   // ── Zoom ──
   window.mapZoomIn = function() { mapZoom = Math.min(mapZoom*1.4, 50); mapRender(); };
   window.mapZoomOut = function() { mapZoom = Math.max(mapZoom/1.4, 0.3); mapRender(); };

   // ── Home ──
   window.mapGoHome = function() {
    const home = loadHome();
    if (!home) return;
    if (mapView !== 'surface') { mapSetView('surface'); setTimeout(() => mapGoHome(), 500); return; }
    const container = document.getElementById('map-canvas-container');
    const cw = container.clientWidth, ch = container.clientHeight;
    mapZoom = 4;
    const target = latLngToScreen(home.lat, home.lng, cw, ch);
    mapPan.x = cw/2 - (home.lng+180)/360*cw*mapZoom - cw/2*(1-mapZoom) + mapPan.x + cw/2 - target.x;
    mapPan.y = ch/2 - (90-home.lat)/180*ch*mapZoom - ch/2*(1-mapZoom) + mapPan.y + ch/2 - target.y;
    mapSelectedLocation = {lat:home.lat,lng:home.lng};
    mapRender();
    updateCoordsBar(home.lat, home.lng);
    showLocationInfo(home.lat, home.lng, home.name);
   };

   // ── GPS ──
   window.mapLocateMe = function() {
    if (!navigator.geolocation) { alert('Geolocation not available'); return; }
    navigator.geolocation.getCurrentPosition(pos => {
     const lat = pos.coords.latitude, lng = pos.coords.longitude;
     if (mapView !== 'surface') mapSetView('surface');
     setTimeout(() => {
      const container = document.getElementById('map-canvas-container');
      const cw = container.clientWidth, ch = container.clientHeight;
      mapZoom = 6;
      mapPan = {x:0,y:0};
      const target = latLngToScreen(lat, lng, cw, ch);
      mapPan.x = cw/2 - target.x;
      mapPan.y = ch/2 - target.y;
      mapSelectedLocation = {lat, lng};
      mapRender();
      updateCoordsBar(lat, lng);
      showLocationInfo(lat, lng, 'My Location');
     }, mapView !== 'surface' ? 500 : 0);
    }, err => { alert('Could not get location: ' + err.message); });
   };

   // ── Toggle Weather ──
   window.mapToggleWeather = function() {
    mapShowWeather = !mapShowWeather;
    if (mapShowWeather && mapSelectedLocation) {
     showLocationInfo(mapSelectedLocation.lat, mapSelectedLocation.lng);
    }
    updateSidebar();
   };

   // ── Toggle Grid ──
   window.mapToggleGrid = function() {
    mapShowGrid = !mapShowGrid;
    mapRender();
   };

   // ── Coords bar ──
   function updateCoordsBar(lat, lng) {
    const el = document.getElementById('map-coords');
    if (!el) return;
    const ico = pointToIcosphere(lat, lng, 5);
    el.textContent = `GPS: ${lat.toFixed(4)}°, ${lng.toFixed(4)}° | Ico: ${ico.address}`;
   }

   // ── Sidebar ──
   function updateSidebar() {
    const panel = document.getElementById('map-info-panel');
    if (!panel) return;

    let html = `<div style="margin-bottom:0.8rem;">
     <div style="font-size:1rem;font-weight:700;color:var(--accent);margin-bottom:0.3rem;">🧘 Astral Projection Active</div>
     <div style="font-size:0.75rem;color:var(--text-muted);font-style:italic;margin-bottom:0.5rem;">Your consciousness expands beyond your physical form...</div>
     <div style="font-size:0.82rem;color:var(--text);">Currently viewing: <strong>${viewNames[mapView]||mapView}</strong></div>
    </div>`;

    if (mapView === 'surface') {
     const home = loadHome();
     if (home) {
      html += `<div style="background:rgba(100,200,150,0.1);border:1px solid rgba(100,200,150,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
       <div style="font-weight:700;font-size:0.85rem;">🏠 ${home.name||'Home'}</div>
       <div style="font-size:0.72rem;color:var(--text-muted);">${home.lat.toFixed(4)}°, ${home.lng.toFixed(4)}° · ${home.icosphere||pointToIcosphere(home.lat,home.lng,5).address}</div>
      </div>`;
     }
     const pins = loadPins();
     if (pins.length) {
      html += `<div style="margin-bottom:0.8rem;"><div style="font-weight:600;font-size:0.82rem;margin-bottom:0.3rem;">📌 Pins (${pins.length})</div>`;
      for (const pin of pins) {
       html += `<div style="font-size:0.75rem;padding:0.2rem 0;cursor:pointer;color:var(--text-muted);" onclick="mapGoToPin(${pin.lat},${pin.lng},'${(pin.name||'').replace(/'/g,"\\'")}')">${pin.name||'Pin'} — ${pin.lat.toFixed(2)}°, ${pin.lng.toFixed(2)}°</div>`;
      }
      html += '</div>';
     }
     html += `<div style="font-size:0.72rem;color:var(--text-muted);margin-bottom:0.5rem;">Click map to select location · Right-click to pin</div>`;
    }

    if (mapView === 'skyview') {
     const sg = loadStargazer();
     const date = skyGetTime();
     const home = loadHome();
     const locName = home ? home.name : 'Paradise, Mount Rainier';

     html += `<div style="background:rgba(180,140,255,0.08);border:1px solid rgba(180,140,255,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
      <div style="font-weight:700;font-size:0.9rem;color:#b48cff;">🔭 Sky View</div>
      <div style="font-size:0.72rem;color:var(--text-muted);margin-top:0.3rem;">Viewing from: ${locName}</div>
      <div style="font-size:0.72rem;color:var(--text-muted);">${date.toLocaleString()}</div>
     </div>`;

     // Time controls
     html += `<div style="background:rgba(50,50,80,0.3);border:1px solid rgba(100,100,150,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
      <div style="font-weight:600;font-size:0.8rem;margin-bottom:0.4rem;">⏰ Time Controls</div>
      <div style="display:flex;gap:0.3rem;flex-wrap:wrap;">
       <button onclick="skySetTimeOffset(-6)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">-6h</button>
       <button onclick="skySetTimeOffset(-1)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">-1h</button>
       <button onclick="skySetTimeOffset(-0.25)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">-15m</button>
       <button onclick="skyResetTime()" style="font-size:0.65rem;padding:2px 8px;background:rgba(100,200,150,0.2);border:1px solid rgba(100,200,150,0.3);color:#6cc;border-radius:4px;cursor:pointer;font-weight:700;">Now</button>
       <button onclick="skySetTimeOffset(0.25)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">+15m</button>
       <button onclick="skySetTimeOffset(1)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">+1h</button>
       <button onclick="skySetTimeOffset(6)" style="font-size:0.65rem;padding:2px 6px;background:rgba(100,150,255,0.15);border:1px solid rgba(100,150,255,0.3);color:#6699ff;border-radius:4px;cursor:pointer;">+6h</button>
      </div>
     </div>`;

     // Toggles
     html += `<div style="background:rgba(50,50,80,0.3);border:1px solid rgba(100,100,150,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
      <div style="font-weight:600;font-size:0.8rem;margin-bottom:0.4rem;">🎛️ Display</div>
      <div style="display:flex;flex-direction:column;gap:0.3rem;">
       <label style="font-size:0.72rem;color:var(--text-muted);cursor:pointer;display:flex;align-items:center;gap:0.3rem;">
        <input type="checkbox" ${skyShowConstellations?'checked':''} onchange="skyToggleConstellations()"> Constellation lines
       </label>
       <label style="font-size:0.72rem;color:var(--text-muted);cursor:pointer;display:flex;align-items:center;gap:0.3rem;">
        <input type="checkbox" ${skyShowMilkyWay?'checked':''} onchange="skyToggleMilkyWay()"> Milky Way band
       </label>
       <label style="font-size:0.72rem;color:var(--text-muted);cursor:pointer;display:flex;align-items:center;gap:0.3rem;">
        <input type="checkbox" ${skyDarkSky?'checked':''} onchange="skyToggleDarkSky()"> Dark sky (no light pollution)
       </label>
      </div>
     </div>`;

     // Stargazer progress
     html += `<div style="background:rgba(255,200,50,0.05);border:1px solid rgba(255,200,50,0.15);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
      <div style="font-weight:700;font-size:0.85rem;color:#ffc832;">⭐ Stargazer</div>
      <div style="font-size:0.75rem;color:var(--text-muted);margin-top:0.3rem;">${sg.identified.length}/88 constellations identified</div>
      <div style="font-size:0.72rem;color:var(--text-muted);">XP: ${sg.xp}</div>
      <div style="width:100%;height:6px;background:rgba(255,255,255,0.05);border-radius:3px;margin-top:0.4rem;overflow:hidden;">
       <div style="width:${(sg.identified.length/88*100).toFixed(1)}%;height:100%;background:linear-gradient(90deg,#ffc832,#ff8800);border-radius:3px;"></div>
      </div>`;
     // Achievements
     const achievements = [];
     if (sg.identified.length >= 1) achievements.push('🌟 First Star');
     const zodiac = ['Aries','Taurus','Gemini','Cancer','Leo','Virgo','Libra','Scorpius','Sagittarius','Capricornus','Aquarius','Pisces'];
     if (zodiac.every(z => sg.identified.includes(z))) achievements.push('♈ All Zodiac');
     if (sg.identified.length >= 88) achievements.push('🏆 All 88!');
     if (achievements.length) {
      html += `<div style="margin-top:0.4rem;font-size:0.7rem;color:#ffc832;">${achievements.join(' · ')}</div>`;
     }
     html += '</div>';

     // Selected constellation detail
     if (skySelectedConstellation) {
      const con = SKY_CONSTELLATIONS.find(c => c.name === skySelectedConstellation);
      if (con) {
       const isIdentified = sg.identified.includes(con.name);
       html += `<div style="background:rgba(100,180,255,0.08);border:1px solid rgba(100,180,255,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
        <div style="font-weight:700;font-size:0.95rem;color:#6cf;">⭐ ${con.name} — ${con.myth ? con.myth.split('—')[0] : con.abbr}</div>
        <div style="font-size:0.72rem;color:var(--text-muted);margin-top:0.3rem;line-height:1.5;">${con.myth || ''}</div>`;
       if (con.season) html += `<div style="font-size:0.72rem;color:var(--text-muted);margin-top:0.3rem;">Best viewing: ${con.season}</div>`;
       if (con.keyStars && con.keyStars.length) {
        html += `<div style="font-size:0.72rem;margin-top:0.4rem;font-weight:600;color:var(--text);">Key Stars:</div>`;
        for (const ks of con.keyStars) html += `<div style="font-size:0.7rem;color:var(--text-muted);padding-left:0.5rem;">⭐ ${ks}</div>`;
       }
       if (con.objects && con.objects.length) {
        html += `<div style="font-size:0.72rem;margin-top:0.4rem;font-weight:600;color:var(--text);">Notable Objects:</div>`;
        for (const obj of con.objects) html += `<div style="font-size:0.7rem;color:var(--text-muted);padding-left:0.5rem;">🌌 ${obj}</div>`;
       }
       if (con.funFact) html += `<div style="font-size:0.72rem;color:#b48cff;margin-top:0.4rem;font-style:italic;">💡 ${con.funFact}</div>`;
       if (!isIdentified) {
        html += `<button onclick="skyIdentifyConstellation('${con.name}')" style="margin-top:0.5rem;font-size:0.75rem;padding:4px 12px;background:rgba(255,200,50,0.2);border:1px solid rgba(255,200,50,0.4);color:#ffc832;border-radius:6px;cursor:pointer;font-weight:600;">Mark as Identified ✓ (+10 XP)</button>`;
       } else {
        html += `<div style="margin-top:0.5rem;font-size:0.72rem;color:#6cc;">✓ Identified</div>`;
       }
       html += '</div>';
      }
     }

     html += `<div style="font-size:0.72rem;color:var(--text-muted);margin-bottom:0.5rem;">Click a constellation to learn about it · Hover to highlight</div>`;
    }

    html += `<details style="margin-top:0.8rem;"><summary style="cursor:pointer;font-size:0.78rem;font-weight:600;color:var(--accent);">Quality Data Sources</summary>
     <div style="font-size:0.7rem;color:var(--text-muted);line-height:1.8;padding:0.3rem 0;">
      ⭐ Stars: HYG Database v4.1<br>
      <span style="font-size:0.65rem;">github.com/astronexus/HYG-Database</span><br>
      🪐 Planets: JPL Solar System Dynamics<br>
      🌤️ Weather: Open-Meteo (free, no tracking)<br>
      📐 Coordinates: WGS84 GPS + Icosphere<br>
      🌍 Coastlines: Natural Earth (simplified)
     </div>
    </details>`;

    panel.innerHTML = html;
   }

   // ── Show location info with weather ──
   async function showLocationInfo(lat, lng, name) {
    const panel = document.getElementById('map-info-panel');
    if (!panel) return;
    const ico = pointToIcosphere(lat, lng, 5);
    let html = `<div style="margin-bottom:0.8rem;">
     <div style="font-size:1rem;font-weight:700;color:var(--accent);margin-bottom:0.3rem;">🧘 Astral Projection Active</div>
     <div style="font-size:0.75rem;color:var(--text-muted);font-style:italic;">Your consciousness expands beyond your physical form...</div>
    </div>`;

    html += `<div style="background:rgba(255,136,17,0.08);border:1px solid rgba(255,136,17,0.2);border-radius:8px;padding:0.6rem;margin-bottom:0.8rem;">
     <div style="font-weight:700;font-size:0.9rem;margin-bottom:0.3rem;">📍 ${name || 'Selected Location'}</div>
     <div style="font-size:0.75rem;color:var(--text-muted);font-family:monospace;">
      GPS: ${lat.toFixed(4)}°N, ${lng.toFixed(4)}°E<br>
      Icosphere: ${ico.address}
     </div>
     <div style="display:flex;gap:0.3rem;margin-top:0.4rem;flex-wrap:wrap;">
      <button onclick="mapSetAsHome(${lat},${lng},'${(name||'').replace(/'/g,"\\'")}')" style="font-size:0.7rem;padding:2px 8px;background:rgba(100,200,150,0.2);border:1px solid rgba(100,200,150,0.3);color:#6cc;border-radius:4px;cursor:pointer;">🏠 Set Home</button>
      <button onclick="mapAddPin(${lat},${lng})" style="font-size:0.7rem;padding:2px 8px;background:rgba(255,68,68,0.2);border:1px solid rgba(255,68,68,0.3);color:#f66;border-radius:4px;cursor:pointer;">📌 Add Pin</button>
     </div>
    </div>`;

    // Weather
    if (mapShowWeather || true) {
     html += `<div id="map-weather-section" style="margin-bottom:0.8rem;"><div style="font-size:0.75rem;color:var(--text-muted);">Loading weather...</div></div>`;
     panel.innerHTML = html + buildDataSources();
     // Fetch weather async
     const weather = await getWeather(lat, lng);
     const ws = document.getElementById('map-weather-section');
     if (ws && weather && weather.current) {
      const c = weather.current;
      let whtml = `<div style="background:rgba(50,100,200,0.1);border:1px solid rgba(50,100,200,0.2);border-radius:8px;padding:0.6rem;">
       <div style="font-weight:700;font-size:0.85rem;margin-bottom:0.3rem;">🌤️ Current Weather</div>
       <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.4rem;">
        <span style="font-size:2rem;">${weatherEmoji(c.weather_code)}</span>
        <div>
         <div style="font-size:1.2rem;font-weight:700;">${c.temperature_2m}°C</div>
         <div style="font-size:0.72rem;color:var(--text-muted);">${weatherDesc(c.weather_code)}</div>
        </div>
       </div>
       <div style="font-size:0.75rem;display:grid;grid-template-columns:1fr 1fr;gap:0.2rem;">
        <div>💧 Humidity: ${c.relative_humidity_2m}%</div>
        <div>💨 Wind: ${c.wind_speed_10m} km/h</div>
        <div>🌧️ Precip: ${c.precipitation} mm</div>
       </div>`;
      if (weather.daily) {
       whtml += `<div style="margin-top:0.5rem;font-weight:600;font-size:0.78rem;">3-Day Forecast</div>
       <div style="display:flex;gap:0.5rem;margin-top:0.3rem;">`;
       for (let i = 0; i < Math.min(3, weather.daily.time.length); i++) {
        const d = weather.daily;
        whtml += `<div style="flex:1;text-align:center;background:rgba(255,255,255,0.03);border-radius:6px;padding:0.3rem;">
         <div style="font-size:0.65rem;color:var(--text-muted);">${d.time[i].slice(5)}</div>
         <div style="font-size:1.2rem;">${weatherEmoji(d.weather_code[i])}</div>
         <div style="font-size:0.7rem;">${d.temperature_2m_max[i]}°/${d.temperature_2m_min[i]}°</div>
        </div>`;
       }
       whtml += '</div>';
      }
      whtml += '</div>';
      ws.innerHTML = whtml;
     } else if (ws) {
      ws.innerHTML = '<div style="font-size:0.72rem;color:var(--text-muted);">Weather unavailable</div>';
     }
    } else {
     panel.innerHTML = html + buildDataSources();
    }
   }

   function buildDataSources() {
    return `<details style="margin-top:0.8rem;"><summary style="cursor:pointer;font-size:0.78rem;font-weight:600;color:var(--accent);">Quality Data Sources</summary>
     <div style="font-size:0.7rem;color:var(--text-muted);line-height:1.8;padding:0.3rem 0;">
      ⭐ Stars: HYG Database v4.1<br>
      <span style="font-size:0.65rem;">github.com/astronexus/HYG-Database</span><br>
      🪐 Planets: JPL Solar System Dynamics<br>
      🌤️ Weather: Open-Meteo (free, no tracking)<br>
      📐 Coordinates: WGS84 GPS + Icosphere<br>
      🌍 Coastlines: Natural Earth (simplified)
     </div>
    </details>`;
   }

   // ── Pin management ──
   window.mapSetAsHome = function(lat, lng, name) {
    const ico = pointToIcosphere(lat, lng, 5);
    saveHome({lat, lng, name: name || prompt('Name this location:', '') || 'Home', icosphere: ico.address});
    mapRender();
    updateSidebar();
   };

   window.mapAddPin = function(lat, lng) {
    const name = prompt('Pin name:', '');
    if (name === null) return;
    const pins = loadPins();
    pins.push({lat, lng, name: name || 'Pin', notes: '', ts: Date.now()});
    savePins(pins);
    mapRender();
    updateSidebar();
   };

   window.mapGoToPin = function(lat, lng, name) {
    if (mapView !== 'surface') mapSetView('surface');
    const container = document.getElementById('map-canvas-container');
    const cw = container.clientWidth, ch = container.clientHeight;
    mapZoom = 4;
    mapPan = {x:0,y:0};
    const target = latLngToScreen(lat, lng, cw, ch);
    mapPan.x = cw/2 - target.x;
    mapPan.y = ch/2 - target.y;
    mapSelectedLocation = {lat, lng};
    mapRender();
    updateCoordsBar(lat, lng);
    showLocationInfo(lat, lng, name);
   };

   // ── Search ──
   window.mapDoSearch = function() {
    const q = document.getElementById('map-search').value.trim().toLowerCase();
    if (!q) return;
    // Check GPS coords format
    const gpsMatch = q.match(/^(-?\d+\.?\d*)\s*[,\s]\s*(-?\d+\.?\d*)$/);
    if (gpsMatch) {
     const lat = parseFloat(gpsMatch[1]), lng = parseFloat(gpsMatch[2]);
     if (lat >= -90 && lat <= 90 && lng >= -180 && lng <= 180) {
      mapSetView('surface');
      setTimeout(() => mapGoToPin(lat, lng, `${lat.toFixed(4)}, ${lng.toFixed(4)}`), 300);
      return;
     }
    }
    // Check icosphere format
    const icoMatch = q.match(/^f(\d+)\.l(\d+)\.t(\d+)$/i);
    if (icoMatch) {
     const face = parseInt(icoMatch[1]), level = parseInt(icoMatch[2]), tri = parseInt(icoMatch[3]);
     const gps = icosphereToGPS(face, level, tri);
     mapSetView('surface');
     setTimeout(() => mapGoToPin(gps.lat, gps.lng, `${q.toUpperCase()}`), 300);
     return;
    }
    // Search cities
    const city = CITIES.find(c => c.name.toLowerCase().includes(q));
    if (city) { mapSetView('surface'); setTimeout(() => mapGoToPin(city.lat, city.lng, city.name), 300); return; }
    // Search planets
    const planet = MAP_PLANETS.find(p => p.name.toLowerCase().includes(q));
    if (planet) { mapSetView('system'); return; }
    // Search stars
    const star = MAP_STARS.find(s => (s[6]||s[0]).toLowerCase().includes(q));
    if (star) { mapSetView('sector'); return; }
    // Galaxy
    if (q.includes('galaxy') || q.includes('milky')) { mapSetView('galaxy'); return; }
    if (q.includes('sky') || q.includes('constellation') || q.includes('stargazer') || q.includes('night sky')) { mapSetView('skyview'); return; }
    const skyConMatch = SKY_CONSTELLATIONS && SKY_CONSTELLATIONS.find(c => c.name.toLowerCase().includes(q));
    if (skyConMatch) { mapSetView('skyview'); setTimeout(() => { skySelectedConstellation = skyConMatch.name; updateSidebar(); mapRender(); }, 500); return; }
   };

   // ── Mouse Events ──
   canvas.addEventListener('mousedown', e => {
    if (mapAnimating) return;
    mapDragging = true;
    const rect = canvas.getBoundingClientRect();
    mapDragStart = {x:e.clientX-rect.left, y:e.clientY-rect.top};
    mapPanStart = {...mapPan};
   });

   canvas.addEventListener('mousemove', e => {
    if (!mapDragging || mapAnimating) return;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX-rect.left, my = e.clientY-rect.top;
    mapPan.x = mapPanStart.x + (mx - mapDragStart.x);
    mapPan.y = mapPanStart.y + (my - mapDragStart.y);
    mapRender();
    // Update coords on surface
    if (mapView === 'surface') {
     const ll = screenToLatLng(mx, my, canvas.clientWidth, canvas.clientHeight);
     updateCoordsBar(ll.lat, ll.lng);
    }
   });

   canvas.addEventListener('mouseup', e => {
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX-rect.left, my = e.clientY-rect.top;
    const wasDrag = Math.abs(mx-mapDragStart.x)>5 || Math.abs(my-mapDragStart.y)>5;
    mapDragging = false;
    if (!wasDrag && mapView === 'surface') {
     const ll = screenToLatLng(mx, my, canvas.clientWidth, canvas.clientHeight);
     mapSelectedLocation = ll;
     updateCoordsBar(ll.lat, ll.lng);
     mapRender();
     showLocationInfo(ll.lat, ll.lng);
    }
   });

   canvas.addEventListener('contextmenu', e => {
    e.preventDefault();
    if (mapView !== 'surface') return;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX-rect.left, my = e.clientY-rect.top;
    const ll = screenToLatLng(mx, my, canvas.clientWidth, canvas.clientHeight);
    const choice = prompt(`${ll.lat.toFixed(4)}°, ${ll.lng.toFixed(4)}°\n\n1 = Set as Home\n2 = Add Pin\n\nEnter choice:`, '2');
    if (choice === '1') mapSetAsHome(ll.lat, ll.lng, '');
    else if (choice === '2') mapAddPin(ll.lat, ll.lng);
   });

   canvas.addEventListener('wheel', e => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.15 : 0.87;
    mapZoom = Math.max(0.3, Math.min(50, mapZoom * factor));
    mapRender();
    if (mapView === 'surface') {
     const rect = canvas.getBoundingClientRect();
     const mx = e.clientX-rect.left, my = e.clientY-rect.top;
     const ll = screenToLatLng(mx, my, canvas.clientWidth, canvas.clientHeight);
     updateCoordsBar(ll.lat, ll.lng);
    }
   }, {passive:false});

   // Touch support
   let mapTouchDist = 0;
   canvas.addEventListener('touchstart', e => {
    if (e.touches.length===1){ mapDragging=true; mapDragStart={x:e.touches[0].clientX,y:e.touches[0].clientY}; mapPanStart={...mapPan}; }
    else if(e.touches.length===2){ mapTouchDist=Math.hypot(e.touches[0].clientX-e.touches[1].clientX,e.touches[0].clientY-e.touches[1].clientY); }
   },{passive:true});
   canvas.addEventListener('touchmove', e => {
    if(e.touches.length===1&&mapDragging){ mapPan.x=mapPanStart.x+(e.touches[0].clientX-mapDragStart.x); mapPan.y=mapPanStart.y+(e.touches[0].clientY-mapDragStart.y); mapRender(); }
    else if(e.touches.length===2){ const nd=Math.hypot(e.touches[0].clientX-e.touches[1].clientX,e.touches[0].clientY-e.touches[1].clientY); mapZoom=Math.max(0.3,Math.min(50,mapZoom*(nd/mapTouchDist))); mapTouchDist=nd; mapRender(); }
   },{passive:true});
   canvas.addEventListener('touchend',()=>{mapDragging=false;},{passive:true});


   // ── Init function (called on tab switch) ──
   window.initMapTab = async function() {
    if (mapInitialized) { mapRender(); return; }
    mapInitialized = true;
    await mapLoadData();
    setTimeout(() => { mapRender(); updateSidebar(); }, 50);
   };

   // Resize
   window.addEventListener('resize', () => { if (document.getElementById('tab-map').classList.contains('active')) mapRender(); });

   // Also handle mousemove on surface for live coords
   canvas.addEventListener('mousemove', e => {
    if (mapView === 'surface' && !mapDragging) {
     const rect = canvas.getBoundingClientRect();
     const mx = e.clientX-rect.left, my = e.clientY-rect.top;
     const ll = screenToLatLng(mx, my, canvas.clientWidth, canvas.clientHeight);
     updateCoordsBar(ll.lat, ll.lng);
    }
   });
  })();

  // ── Map tab activation ──
  {
   const origSwitchTabMap = switchTab;
   switchTab = function(tabId, pushState) {
    origSwitchTabMap(tabId, pushState);
    if (tabId === 'map' && typeof initMapTab === 'function') initMapTab();
   };
   if (initialTab === 'map' && typeof initMapTab === 'function') initMapTab();
  }

  // ── Auto-refresh on server update ──
  (function() {
   let knownVersion = null;
   async function checkVersion() {
    try {
     const res = await fetch('/api/stats');
     if (!res.ok) return;
     const data = await res.json();
     if (!data.version) return;
     if (knownVersion === null) {
      knownVersion = data.version;
     } else if (knownVersion !== data.version) {
      console.log('Server updated (' + knownVersion + ' → ' + data.version + '), reloading…');
      location.reload();
     }
    } catch (e) { /* network hiccup, try again next cycle */ }
   }
   checkVersion();
   setInterval(checkVersion, 30000);
  })();
 </script>
