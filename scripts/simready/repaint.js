// repaint.js - the brand check's repaints (docs/design/simready-props.md, section 8): a copy of a
// source texture with a maker's name, logo or a protected emblem painted out, which the bake then
// uses in place of the original. The original download is never changed.
//
// A repaint is recorded in starter-set.json under the asset's `brand_check.repaints`:
//   { "texture": "<file name in simready_usd/Textures/>", "kind": "fill",
//     "rects": [[x0, y0, x1, y1], ...], "sample": [x, y] }
//       every pixel in each rectangle (x0 <= x < x1, y0 <= y < y1, the texture's own pixels, row 0
//       at the top) takes the colour and alpha of the sample pixel: the plain surface beside the mark;
//   { "texture": "...", "kind": "red_to_green" }
//       every red pixel becomes the green of the ISO 7010 first aid sign (a red cross becomes a
//       green one): new = (g, 0.71 r, 0.46 r), blended by how red the pixel is, so edges stay soft.
// Done with ffmpeg (the instrument pipeline's dependency already); the bake reads the result's
// pixels, so its PNG bytes need not be stable, only its pixels.

'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

function ffmpeg(args) {
  const r = spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args], { windowsHide: true, encoding: 'buffer', maxBuffer: 1 << 26 });
  if (r.status !== 0) throw new Error(`ffmpeg ${args.join(' ')}: ${r.stderr.toString()}`);
  return r.stdout;
}

/** One pixel's RGBA. */
function samplePixel(file, x, y) {
  const raw = ffmpeg(['-i', file, '-vf', `crop=1:1:${x}:${y},format=rgba`, '-f', 'rawvideo', '-']);
  if (raw.length < 4) throw new Error(`could not read pixel ${x},${y} of ${file}`);
  return [raw[0], raw[1], raw[2], raw[3]];
}

function fillExpr(rects, channel, value) {
  const inside = rects.map(([x0, y0, x1, y1]) => `between(X,${x0},${x1 - 1})*between(Y,${y0},${y1 - 1})`).join('+');
  const src = { r: 'r(X,Y)', g: 'g(X,Y)', b: 'b(X,Y)', a: 'alpha(X,Y)' }[channel];
  return `if(gt(${inside},0),${value},${src})`;
}

/** Write the repainted copy of one texture and return its path. */
function repaintTexture(srcFile, outFile, spec) {
  fs.mkdirSync(path.dirname(outFile), { recursive: true });
  let vf;
  if (spec.kind === 'fill') {
    const [r, g, b, a] = samplePixel(srcFile, spec.sample[0], spec.sample[1]);
    vf = `format=rgba,geq=r='${fillExpr(spec.rects, 'r', r)}':g='${fillExpr(spec.rects, 'g', g)}':b='${fillExpr(spec.rects, 'b', b)}':a='${fillExpr(spec.rects, 'a', a)}'`;
  } else if (spec.kind === 'red_to_green') {
    const w = 'clip((r(X,Y)-max(g(X,Y),b(X,Y)))/100,0,1)';
    vf = `format=rgba,geq=r='r(X,Y)+(g(X,Y)-r(X,Y))*${w}':g='g(X,Y)+(0.71*r(X,Y)-g(X,Y))*${w}':b='b(X,Y)+(0.46*r(X,Y)-b(X,Y))*${w}':a='alpha(X,Y)'`;
  } else {
    throw new Error(`unknown repaint kind ${spec.kind}`);
  }
  ffmpeg(['-i', srcFile, '-vf', vf, outFile]);
  return outFile;
}

/** Apply every repaint an asset's brand check lists; returns { textureFileName: repaintedPath }. */
function applyRepaints(asset, srcDir, outDir) {
  const out = {};
  // A `"stage": "baked"` repaint paints the bake's output, not a source texture (pack.js,
  // applyBakedRepaints: a name moulded into the geometry is in no texture).
  const specs = ((asset.brand_check && asset.brand_check.repaints) || []).filter(s => s.stage !== 'baked');
  // Several repaints of one texture apply in order, each to the previous result.
  for (const spec of specs) {
    const original = path.join(srcDir, 'simready_usd', 'Textures', spec.texture);
    const input = out[spec.texture] || original;
    const target = path.join(outDir, `${spec.texture.replace(/\.[^.]+$/, '')}.${Object.keys(out).length}.${specs.indexOf(spec)}.png`);
    out[spec.texture] = repaintTexture(input, target, spec);
  }
  return out;
}

module.exports = { applyRepaints, repaintTexture, samplePixel };
