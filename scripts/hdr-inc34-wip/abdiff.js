// Increment-3 A/B analysis: four captures of one park, d1 h1 d2 h2
// (display, hdr, display, hdr). A pixel the SWITCH changed differs on both
// flips (d1/h1 and d2/h2) and on neither same-arm pair (d1/d2, h1/h2);
// scene motion lands somewhere new each capture.
// usage: node abdiff.js <dir> <vantage> [...]
const path = require('path');
const sharp = require(require.resolve('sharp', { paths: ['C:/Humanity'] }));
const dir = process.argv[2];
const ids = process.argv.slice(3);
async function load(arm, id) {
  const { data, info } = await sharp(path.join(dir, `${arm}__${id}.png`)).raw().toBuffer({ resolveWithObject: true });
  return { data, W: info.width, H: info.height, C: info.channels };
}
function pxDiff(a, b, i, C) {
  let m = 0;
  for (let c = 0; c < 3; c++) m = Math.max(m, Math.abs(a[i * C + c] - b[i * C + c]));
  return m;
}
(async () => {
  for (const id of ids) {
    let d1, h1, d2, h2;
    try {
      [d1, h1, d2, h2] = await Promise.all(['d1', 'h1', 'd2', 'h2'].map((a) => load(a, id)));
    } catch (e) { console.log(id, 'missing', e.message); continue; }
    const { W, H, C } = d1;
    const N = W * H;
    let sameD = 0, sameH = 0, flip1 = 0, flip2 = 0, sw = 0, maxSw = 0;
    const hist = [0, 0, 0, 0, 0]; // 1, 2, 3..8, 9..32, >32
    const big = [];
    let flipMax = 0;
    for (let i = 0; i < N; i++) {
      const dd = pxDiff(d1.data, d2.data, i, C);
      const hh = pxDiff(h1.data, h2.data, i, C);
      const f1 = pxDiff(d1.data, h1.data, i, C);
      const f2 = pxDiff(d2.data, h2.data, i, C);
      if (dd) sameD++;
      if (hh) sameH++;
      if (f1) flip1++;
      if (f2) flip2++;
      flipMax = Math.max(flipMax, f1, f2);
      if (f1 && f2 && !dd && !hh) {
        sw++;
        maxSw = Math.max(maxSw, f1);
        const k = f1 === 1 ? 0 : f1 === 2 ? 1 : f1 <= 8 ? 2 : f1 <= 32 ? 3 : 4;
        hist[k]++;
        if (f1 > 2) {
          const o = i * C;
          big.push({ x: i % W, y: Math.floor(i / W), d: f1, disp: [d1.data[o], d1.data[o + 1], d1.data[o + 2]], hdr: [h1.data[o], h1.data[o + 1], h1.data[o + 2]] });
        }
      }
    }
    console.log(`${id} ${W}x${H}: same-arm floor d ${sameD} px, h ${sameH} px; flips ${flip1} / ${flip2} px (max ${flipMax}); SWITCH px ${sw} max ${maxSw}; by size 1:${hist[0]} 2:${hist[1]} 3-8:${hist[2]} 9-32:${hist[3]} >32:${hist[4]}`);
    big.sort((a, b) => b.d - a.d);
    for (const b of big.slice(0, 6)) console.log(`   ${b.x},${b.y} d${b.d} display ${b.disp} hdr ${b.hdr}`);
    if (big.length) {
      // Are the >2 pixels star-like (a dark neighbourhood) or on surfaces?
      let darkNb = 0;
      for (const b of big) {
        let s = 0, n = 0;
        for (let dy = -3; dy <= 3; dy++) for (let dx = -3; dx <= 3; dx++) {
          const x = b.x + dx, y = b.y + dy;
          if (x < 0 || y < 0 || x >= W || y >= H) continue;
          const o = (y * W + x) * C;
          s += (d1.data[o] + d1.data[o + 1] + d1.data[o + 2]) / 3; n++;
        }
        if (s / n < 40) darkNb++;
      }
      console.log(`   >2-code switch px: ${big.length}, of which ${darkNb} sit in a dark 7x7 neighbourhood (star-like)`);
    }
  }
})();
