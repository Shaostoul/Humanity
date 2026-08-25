// Rebuild web/shared/social/og-banner.png.
//
// The old one led with the RETIRED "Project Universe" ring logo across the left
// half of the frame, and carried a tagline ("Free water. Free energy. Free food.
// Forever.") that no longer matched the page. This is the first and often only
// image anyone sees when the link is posted anywhere, so it should say what the
// homepage says.
//
// Colours come from web/shared/theme.css, which is itself generated from
// data/gui/theme.ron. Do not hand-pick colours here.
const fs = require('fs');
const path = require('path');
const sharp = require('sharp');

const T = {
  bg: '#000000',
  card: '#040404',
  text: '#e8e8ea',
  secondary: '#b4b4be',
  muted: '#94949f',
  accent: '#ed8c24',
  border: '#2a2a35',
};

const W = 1200, H = 630;

// Escape anything that would break the SVG.
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// A generic family list rather than one font name: this renders through
// librsvg/resvg using whatever the machine has, and a missing family silently
// falls back to something ugly (or nothing).
const FONT = "Segoe UI, Selawik, DejaVu Sans, Arial, Helvetica, sans-serif";

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <linearGradient id="warm" x1="0" y1="1" x2="1" y2="0">
      <stop offset="0%" stop-color="${T.accent}" stop-opacity="0.16"/>
      <stop offset="55%" stop-color="${T.accent}" stop-opacity="0.03"/>
      <stop offset="100%" stop-color="${T.accent}" stop-opacity="0"/>
    </linearGradient>
  </defs>

  <rect width="${W}" height="${H}" fill="${T.bg}"/>
  <rect width="${W}" height="${H}" fill="url(#warm)"/>
  <rect x="0" y="0" width="10" height="${H}" fill="${T.accent}"/>

  <text x="72" y="112" font-family="${FONT}" font-size="26" font-weight="700"
        letter-spacing="3" fill="${T.accent}">HUMANITYOS</text>

  <text x="72" y="214" font-family="${FONT}" font-size="60" font-weight="800" fill="${T.text}">Free tools for growing food,</text>
  <text x="72" y="286" font-family="${FONT}" font-size="60" font-weight="800" fill="${T.text}">collecting water, and</text>
  <text x="72" y="358" font-family="${FONT}" font-size="60" font-weight="800" fill="${T.text}">making power.</text>

  <text x="72" y="428" font-family="${FONT}" font-size="30" fill="${T.secondary}">Plus a space game built on the same rules.</text>

  <text x="72" y="502" font-family="${FONT}" font-size="26" fill="${T.muted}">No ads. No email. No password. Open source.</text>

  <text x="72" y="566" font-family="${FONT}" font-size="26" font-weight="700" fill="${T.accent}">united-humanity.us</text>
</svg>`;

const OUT = path.join('web', 'shared', 'social', 'og-banner.png');

(async () => {
  const base = await sharp(Buffer.from(svg)).png().toBuffer();

  // The app mark, right-hand side. Kept modest: the words carry the message,
  // and the old banner's mistake was letting a logo occupy half the frame.
  const markSize = 260;
  const mark = await sharp(path.join('web', 'shared', 'icons', 'icon-512.png'))
    .resize(markSize, markSize, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .toBuffer();

  await sharp(base)
    .composite([{ input: mark, left: W - markSize - 80, top: Math.round((H - markSize) / 2) }])
    .png({ compressionLevel: 9 })
    .toFile(OUT);

  const meta = await sharp(OUT).metadata();
  const bytes = fs.statSync(OUT).size;
  console.log('wrote', OUT, meta.width + 'x' + meta.height, Math.round(bytes / 1024) + ' KB');
  if (meta.width !== W || meta.height !== H) {
    console.error('!! size does not match the declared og:image:width/height');
    process.exit(1);
  }
})();
