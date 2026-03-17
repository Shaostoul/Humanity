#!/usr/bin/env node
/**
 * HumanityOS Icon Generator
 *
 * WHY: Keeps all icons visually consistent — same stroke weight, padding,
 *      line caps, and color. Outputs both SVG (scalable, best format) and
 *      PNG (512×512, for legacy/fallback).
 *
 * STYLE SPEC (derived from existing icon set):
 *   Canvas:       512 × 512 px
 *   Color:        #FFFFFF (white on transparent)
 *   Stroke width: 16 px (at 512px scale)
 *   Line cap:     round
 *   Line join:    round
 *   Padding:      ~48 px from edges (content within 48–464 box)
 *   Fill:         none (outline only) unless shape semantics require it
 *
 * USAGE:
 *   node tools/generate-icon.js <icon-name>
 *   — Reads  tools/icons/<icon-name>.svg  (hand-authored SVG path)
 *   — Writes assets/ui/icons/<icon-name>.png
 *   — Writes assets/ui/icons/<icon-name>.svg
 *
 * Or use the built-in icons:
 *   node tools/generate-icon.js download
 *   node tools/generate-icon.js refresh
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
const BUILTIN_ICONS = {
  // Download: downward arrow into a tray
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
      <!-- Arrow shaft -->
      <line x1="${cx}" y1="${arrowTop}" x2="${cx}" y2="${arrowBot}" />
      <!-- Arrow head -->
      <polyline points="${cx - arrowW},${arrowBot - arrowW} ${cx},${arrowBot} ${cx + arrowW},${arrowBot - arrowW}" />
      <!-- Tray -->
      <polyline points="${trayL},${trayY} ${trayL},${trayBot} ${trayR},${trayBot} ${trayR},${trayY}" />
    `;
  },

  // Refresh/update: circular arrow
  refresh: () => {
    return `
      <path d="M 400,256 A 144,144 0 1 1 256,112" fill="none" />
      <polyline points="256,48 256,112 320,112" />
    `;
  },
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
  const name = process.argv[2];
  if (!name) {
    console.log('Usage: node tools/generate-icon.js <icon-name>');
    console.log('Built-in icons:', Object.keys(BUILTIN_ICONS).join(', '));
    process.exit(1);
  }

  let svgContent;
  const customPath = path.join(__dirname, 'icons', `${name}.svg`);

  if (BUILTIN_ICONS[name]) {
    svgContent = wrapSVG(BUILTIN_ICONS[name]());
    console.log(`Using built-in definition for "${name}"`);
  } else if (fs.existsSync(customPath)) {
    svgContent = fs.readFileSync(customPath, 'utf8');
    console.log(`Using custom SVG from ${customPath}`);
  } else {
    console.error(`No built-in icon "${name}" and no file at ${customPath}`);
    process.exit(1);
  }

  const outDir = path.join(__dirname, '..', 'assets', 'ui', 'icons');
  fs.mkdirSync(outDir, { recursive: true });

  // Write SVG
  const svgPath = path.join(outDir, `${name}.svg`);
  fs.writeFileSync(svgPath, svgContent);
  console.log(`SVG → ${svgPath}`);

  // Write PNG (512×512)
  const pngPath = path.join(outDir, `${name}.png`);
  await sharp(Buffer.from(svgContent))
    .resize(SIZE, SIZE)
    .png()
    .toFile(pngPath);
  console.log(`PNG → ${pngPath}`);
}

main().catch(err => { console.error(err); process.exit(1); });
