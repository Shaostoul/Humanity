#!/usr/bin/env node
/**
 * HumanityOS Icon Generator
 *
 * WHY: Keeps all icons visually consistent — same stroke weight, padding,
 *      line caps, and color. Outputs both SVG (scalable, best format) and
 *      PNG (512×512, for legacy/fallback).
 *
 * STYLE SPEC:
 *   Canvas:       512 × 512 px
 *   Color:        #FFFFFF (white on transparent)
 *   Stroke width: 16 px (at 512px scale — renders as weight 3 at 48px display)
 *   Line cap:     round
 *   Line join:    round
 *   Padding:      ~48 px from edges (content within 48–464 box)
 *   Fill:         none (outline only) unless shape semantics require it
 *
 * USAGE:
 *   node tools/generate-icon.js <icon-name>     Generate one icon
 *   node tools/generate-icon.js --all           Generate ALL built-in icons
 *
 * SVG AUTHORING RULES:
 *   - ViewBox: 0 0 512 512
 *   - Use stroke="#FFF" stroke-width="16" stroke-linecap="round" stroke-linejoin="round" fill="none"
 *   - Keep shapes within the 48–464 safe zone
 *   - No background rectangles — transparency is the background
 */

const sharp = require('sharp');
const fs = require('fs');
const path = require('path');

const SIZE = 512;
const STROKE = 16;
const COLOR = '#FFFFFF';
const PAD = 48; // safe zone padding

// ── Built-in icon definitions (SVG path data) ───────────────────────────────
// Each icon is a function returning SVG content inside the viewBox.
// Coordinate space: 512×512, safe zone 48–464.
const BUILTIN_ICONS = {

  // ── Nav icons (replace existing PNGs) ──────────────────────────────────

  // Network: chat bubble with signal waves
  network: () => `
    <path d="M80,80 H360 V300 H200 L140,360 V300 H80 Z" />
    <line x1="170" y1="170" x2="270" y2="170" />
    <line x1="170" y1="220" x2="240" y2="220" />
    <path d="M360,160 H432 V360 H370 V420 L310,360 H260" />
  `,

  // Profile: person silhouette
  profile: () => `
    <circle cx="256" cy="180" r="100" />
    <path d="M80,464 A176,140 0 0 1 432,464" />
  `,

  // Home: house with door
  home: () => `
    <polyline points="64,270 256,80 448,270" />
    <polyline points="120,240 120,440 392,440 392,240" />
    <rect x="210" y="300" width="92" height="140" rx="4" />
  `,

  // Inventory/Gear: backpack
  inventory: () => `
    <rect x="120" y="180" width="272" height="270" rx="24" />
    <path d="M180,180 V120 A76,76 0 0 1 332,120 V180" />
    <rect x="180" y="260" width="152" height="80" rx="10" />
    <line x1="256" y1="280" x2="256" y2="320" />
  `,

  // Tasks: checklist on clipboard
  tasklist: () => `
    <rect x="100" y="80" width="312" height="380" rx="20" />
    <path d="M200,80 V60 A56,56 0 0 1 312,60 V80" />
    <polyline points="160,200 190,230 230,180" />
    <line x1="260" y1="205" x2="380" y2="205" />
    <polyline points="160,290 190,320 230,270" />
    <line x1="260" y1="295" x2="380" y2="295" />
    <line x1="160" y1="385" x2="230" y2="385" />
    <line x1="260" y1="385" x2="360" y2="385" />
  `,

  // Calendar: monthly calendar grid
  calendar: () => `
    <rect x="72" y="100" width="368" height="360" rx="20" />
    <line x1="72" y1="190" x2="440" y2="190" />
    <line x1="180" y1="100" x2="180" y2="60" />
    <line x1="332" y1="100" x2="332" y2="60" />
    <line x1="196" y1="190" x2="196" y2="460" />
    <line x1="316" y1="190" x2="316" y2="460" />
    <line x1="72" y1="280" x2="440" y2="280" />
    <line x1="72" y1="370" x2="440" y2="370" />
  `,

  // Map: folded map with pin
  map: () => `
    <polygon points="64,100 200,160 320,100 448,160 448,420 320,360 200,420 64,360" />
    <line x1="200" y1="160" x2="200" y2="420" />
    <line x1="320" y1="100" x2="320" y2="360" />
    <circle cx="320" cy="200" r="8" fill="#FFF" stroke="none" />
  `,

  // Market: storefront with awning
  market: () => `
    <rect x="80" y="220" width="352" height="240" rx="4" />
    <polyline points="80,220 80,120 432,120 432,220" />
    <path d="M80,120 L80,220 Q140,180 200,220 Q260,180 320,220 Q380,180 432,220 V120" fill="none" />
    <rect x="180" y="320" width="152" height="140" rx="4" />
  `,

  // Website/Web: globe on stand
  website: () => `
    <circle cx="256" cy="220" r="160" />
    <ellipse cx="256" cy="220" rx="70" ry="160" />
    <line x1="96" y1="220" x2="416" y2="220" />
    <path d="M112,150 Q256,180 400,150" fill="none" />
    <path d="M112,290 Q256,260 400,290" fill="none" />
    <line x1="256" y1="380" x2="256" y2="440" />
    <line x1="180" y1="440" x2="332" y2="440" />
  `,

  // Download: downward arrow into tray
  download: () => {
    const cx = 256;
    const arrowTop = PAD + 20;
    const arrowBot = 320;
    const arrowW = 80;
    const trayY = 380;
    const trayBot = SIZE - PAD;
    const trayL = PAD + 40;
    const trayR = SIZE - PAD - 40;
    return `
      <line x1="${cx}" y1="${arrowTop}" x2="${cx}" y2="${arrowBot}" />
      <polyline points="${cx - arrowW},${arrowBot - arrowW} ${cx},${arrowBot} ${cx + arrowW},${arrowBot - arrowW}" />
      <polyline points="${trayL},${trayY} ${trayL},${trayBot} ${trayR},${trayBot} ${trayR},${trayY}" />
    `;
  },

  // Dev: code brackets </>
  dev: () => `
    <polyline points="200,140 100,256 200,372" />
    <polyline points="312,140 412,256 312,372" />
    <line x1="290" y1="100" x2="222" y2="412" />
  `,

  // Settings/Gear: cog wheel
  settings: () => `
    <circle cx="256" cy="256" r="72" />
    <path d="M256,68 L276,130 A130,130 0 0 1 340,164 L398,138 L372,206
             A130,130 0 0 1 380,256 L444,264 L420,328 A130,130 0 0 1 372,370
             L398,424 L340,412 A130,130 0 0 1 276,444 L256,444 L236,444
             A130,130 0 0 1 172,412 L114,424 L140,370 A130,130 0 0 1 92,328
             L68,264 L132,256 A130,130 0 0 1 140,206 L114,138 L172,164
             A130,130 0 0 1 236,130 Z" />
  `,

  // Ops/Wrench: wrench tool
  ops: () => `
    <path d="M340,80 A120,120 0 0 0 200,180 L108,360 A56,56 0 1 0 172,412
             L340,280 A120,120 0 0 0 432,172 L360,172 L360,100 L300,100 L300,140" fill="none" />
    <circle cx="140" cy="390" r="16" />
  `,

  // Games/Gamepad: controller
  games: () => `
    <path d="M120,180 A80,80 0 0 0 80,340 A60,60 0 0 0 160,380 L200,280
             H312 L352,380 A60,60 0 0 0 432,340 A80,80 0 0 0 392,180 Z" fill="none" />
    <line x1="170" y1="240" x2="170" y2="300" />
    <line x1="140" y1="270" x2="200" y2="270" />
    <circle cx="330" cy="240" r="12" />
    <circle cx="370" cy="280" r="12" />
  `,

  // Journal/Notes: open book
  journal: () => `
    <path d="M256,120 V420" />
    <path d="M256,120 C200,100 120,100 72,120 V420 C120,400 200,400 256,420" fill="none" />
    <path d="M256,120 C312,100 392,100 440,120 V420 C392,400 312,400 256,420" fill="none" />
    <line x1="130" y1="200" x2="210" y2="200" />
    <line x1="130" y1="260" x2="210" y2="260" />
    <line x1="130" y1="320" x2="190" y2="320" />
    <line x1="302" y1="200" x2="382" y2="200" />
    <line x1="302" y1="260" x2="382" y2="260" />
  `,

  // Refresh/update: circular arrow
  refresh: () => `
    <path d="M 400,256 A 144,144 0 1 1 256,112" fill="none" />
    <polyline points="256,48 256,112 320,112" />
  `,

  // ── Download page platform icons ───────────────────────────────────────

  // Monitor: desktop computer
  monitor: () => `
    <rect x="72" y="72" width="368" height="280" rx="16" />
    <line x1="256" y1="352" x2="256" y2="420" />
    <line x1="160" y1="420" x2="352" y2="420" />
  `,

  // Laptop: notebook computer
  laptop: () => `
    <rect x="112" y="100" width="288" height="220" rx="12" />
    <path d="M64,320 H448 A0,0 0 0 1 448,320 L420,380 H92 L64,320 Z" />
  `,

  // Phone: mobile device
  phone: () => `
    <rect x="152" y="60" width="208" height="392" rx="24" />
    <line x1="224" y1="400" x2="288" y2="400" />
  `,

  // Penguin: Linux logo
  penguin: () => `
    <ellipse cx="256" cy="300" rx="120" ry="150" />
    <ellipse cx="256" cy="180" rx="80" ry="90" />
    <circle cx="228" cy="168" r="12" fill="#FFF" />
    <circle cx="284" cy="168" r="12" fill="#FFF" />
    <path d="M240,200 L256,216 L272,200" fill="none" />
    <path d="M136,240 Q80,320 100,420" fill="none" />
    <path d="M376,240 Q432,320 412,420" fill="none" />
    <line x1="200" y1="430" x2="200" y2="460" />
    <line x1="312" y1="430" x2="312" y2="460" />
    <line x1="172" y1="460" x2="228" y2="460" />
    <line x1="284" y1="460" x2="340" y2="460" />
  `,

  // Globe: web/internet
  globe: () => `
    <circle cx="256" cy="256" r="200" />
    <ellipse cx="256" cy="256" rx="80" ry="200" />
    <line x1="56" y1="256" x2="456" y2="256" />
    <path d="M80,160 Q256,200 432,160" fill="none" />
    <path d="M80,352 Q256,312 432,352" fill="none" />
  `,

  // Robot: android face
  robot: () => `
    <rect x="112" y="180" width="288" height="240" rx="24" />
    <line x1="200" y1="180" x2="180" y2="120" />
    <line x1="312" y1="180" x2="332" y2="120" />
    <circle cx="200" cy="280" r="24" />
    <circle cx="312" cy="280" r="24" />
    <line x1="200" y1="360" x2="312" y2="360" />
    <line x1="80" y1="260" x2="80" y2="340" />
    <line x1="432" y1="260" x2="432" y2="340" />
    <line x1="180" y1="440" x2="180" y2="460" />
    <line x1="332" y1="440" x2="332" y2="460" />
  `,

  // ── Download page module icons ─────────────────────────────────────────

  // Chat: speech bubble
  chat: () => `
    <path d="M80,80 H432 V340 H200 L140,400 V340 H80 Z" />
    <line x1="160" y1="170" x2="352" y2="170" />
    <line x1="160" y1="240" x2="300" y2="240" />
  `,

  // Dashboard: bar chart
  dashboard: () => `
    <rect x="80" y="280" width="80" height="160" rx="4" />
    <rect x="216" y="180" width="80" height="260" rx="4" />
    <rect x="352" y="100" width="80" height="340" rx="4" />
    <line x1="60" y1="460" x2="452" y2="460" />
  `,

  // Map pin: location marker
  mappin: () => `
    <path d="M256,440 C180,340 96,260 96,192 A160,160 0 0 1 416,192 C416,260 332,340 256,440 Z" fill="none" />
    <circle cx="256" cy="200" r="56" />
  `,

  // Cart: shopping cart
  cart: () => `
    <circle cx="200" cy="420" r="28" />
    <circle cx="380" cy="420" r="28" />
    <path d="M60,80 H120 L180,340 H400 L440,160 H160" fill="none" />
  `,

  // Storefront: marketplace
  storefront: () => `
    <rect x="80" y="240" width="352" height="220" rx="4" />
    <polyline points="80,240 80,140 432,140 432,240" />
    <path d="M80,140 Q140,200 200,140 Q260,200 320,140 Q380,200 432,140" fill="none" />
    <rect x="200" y="340" width="112" height="120" rx="4" />
  `,

  // Hammer: build tool for World Builder
  hammer: () => `
    <rect x="100" y="60" width="180" height="100" rx="12" />
    <line x1="256" y1="160" x2="256" y2="440" />
    <line x1="200" y1="440" x2="312" y2="440" />
  `,

  // Steam: stylized S-valve shape
  steam: () => `
    <circle cx="256" cy="256" r="196" />
    <path d="M160,320 Q160,240 256,240 Q352,240 352,180 Q352,120 256,120" fill="none" />
    <circle cx="256" cy="240" r="24" />
    <line x1="160" y1="320" x2="160" y2="400" />
  `,

  // Epic: storefront with E
  epic: () => `
    <circle cx="256" cy="256" r="196" />
    <polyline points="320,160 200,160 200,352 320,352" />
    <line x1="200" y1="256" x2="300" y2="256" />
  `,

  // PlayStation: circled gamepad
  playstation: () => `
    <circle cx="256" cy="256" r="196" />
    <path d="M180,300 Q180,200 256,160 Q332,200 332,260 Q332,300 256,300 H180" fill="none" />
    <line x1="256" y1="160" x2="256" y2="380" />
  `,

  // Xbox: circled X
  xbox: () => `
    <circle cx="256" cy="256" r="196" />
    <line x1="160" y1="160" x2="352" y2="352" />
    <line x1="352" y1="160" x2="160" y2="352" />
  `,

  // GOG: galaxy swirl
  gog: () => `
    <circle cx="256" cy="256" r="196" />
    <path d="M300,120 A140,140 0 1 1 160,300" fill="none" />
    <circle cx="256" cy="256" r="40" />
  `,

  // Puzzle: community modules
  puzzle: () => `
    <path d="M80,200 H180 Q180,160 210,160 Q240,160 240,200 H360 V300
             Q320,300 320,330 Q320,360 360,360 V460 H80 Z" fill="none" />
    <path d="M360,200 V100 H200 Q200,140 170,140 Q140,140 140,100" fill="none" />
    <path d="M360,200 H440 Q440,240 470,240 Q470,200 440,200" fill="none" />
  `,

  // Rocket: launch / open app
  rocket: () => `
    <path d="M256,80 Q180,200 180,340 L256,400 L332,340 Q332,200 256,80 Z" fill="none" />
    <circle cx="256" cy="240" r="32" />
    <path d="M180,300 Q120,320 100,380" fill="none" />
    <path d="M332,300 Q392,320 412,380" fill="none" />
    <path d="M220,400 L256,460 L292,400" fill="none" />
  `,
};

// ── SVG wrapper ──────────────────────────────────────────────────────────────
function wrapSVG(innerContent) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${SIZE} ${SIZE}" width="${SIZE}" height="${SIZE}">
  <g stroke="${COLOR}" stroke-width="${STROKE}" stroke-linecap="round" stroke-linejoin="round" fill="none">
    ${innerContent.trim()}
  </g>
</svg>`;
}

// ── Main ─────────────────────────────────────────────────────────────────────
async function main() {
  const arg = process.argv[2];
  if (!arg) {
    console.log('Usage: node tools/generate-icon.js <icon-name>');
    console.log('       node tools/generate-icon.js --all');
    console.log('\nBuilt-in icons:', Object.keys(BUILTIN_ICONS).join(', '));
    process.exit(1);
  }

  const outDir = path.join(__dirname, '..', 'assets', 'ui', 'icons');
  fs.mkdirSync(outDir, { recursive: true });

  // --all: generate every built-in icon
  const names = arg === '--all' ? Object.keys(BUILTIN_ICONS) : [arg];

  for (const name of names) {
    let svgContent;
    const customPath = path.join(__dirname, 'icons', `${name}.svg`);

    if (BUILTIN_ICONS[name]) {
      svgContent = wrapSVG(BUILTIN_ICONS[name]());
    } else if (fs.existsSync(customPath)) {
      svgContent = fs.readFileSync(customPath, 'utf8');
    } else {
      console.error(`No built-in icon "${name}" and no file at ${customPath}`);
      process.exit(1);
    }

    // Write SVG
    const svgPath = path.join(outDir, `${name}.svg`);
    fs.writeFileSync(svgPath, svgContent);

    // Write PNG (512×512)
    const pngPath = path.join(outDir, `${name}.png`);
    await sharp(Buffer.from(svgContent))
      .resize(SIZE, SIZE)
      .png()
      .toFile(pngPath);

    console.log(`✓ ${name} → SVG + PNG`);
  }

  console.log(`\nDone: ${names.length} icon(s) generated.`);
}

main().catch(err => { console.error(err); process.exit(1); });
