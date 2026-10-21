#!/usr/bin/env node
// Write each body's `map_color` into data/star_systems/sol.json: the colour the
// Maps page paints a body's icon in (Maps increment 2,
// docs/design/maps-zoom-to-anything.md 5.3: "Colours are data: a `map_color`
// per body in `sol.json`, computed for bodies with an albedo map as the mean of
// that map, so the hard-coded `body_color` match goes").
//
// Three kinds of body, in this order of preference:
//
//  1. A body with a colour map (`albedo: Some("planets/<x>.bin")` in its
//     data/planets/<id>.ron): the MEAN of that map, weighted by the area each
//     texel covers on the sphere (cos latitude, since an equirectangular row
//     near a pole covers far less ground than one at the equator) and averaged
//     in LINEAR light (sRGB decoded first, encoded back at the end), which is
//     what the eye integrates when a whole globe shrinks to one dot. Rewritten
//     every run, so a new or rebaked map changes the dot with it.
//  2. A body whose `map_color` was set by hand (its `map_color_source` does not
//     start with "mean of" or "grey at"): kept as it is. The Sun and the giant
//     planets, which have no map in data/planets yet (increment 5 brings them).
//  3. Every other body: a neutral grey at the body's own albedo
//     (`physical.albedo`, the fraction of light it reflects), so a coal-dark
//     comet is dark and icy Enceladus is near white. Honest about what we do
//     not know (no colour), exact about what we do (how bright it is).
//
// Run after adding a body, a map or an albedo:   node scripts/build-body-colors.js
// It bumps nothing; bump `catalog_version` by hand when a release must override
// stale installed copies (src/cosmos.rs `load_sol_json`).

"use strict";
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const SOL = path.join(ROOT, "data", "star_systems", "sol.json");
const PLANETS = path.join(ROOT, "data", "planets");

// sRGB byte -> linear (IEC 61966-2-1), and back: the same curve as
// src/terrain/planet_albedo.rs `srgb_byte_to_linear` / `linear_to_srgb_byte`.
function toLinear(b) {
  const c = b / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}
function toSrgbByte(l) {
  const c = Math.max(0, Math.min(1, l));
  const s = c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055;
  return Math.round(s * 255);
}
const hex = (rgb) => "#" + rgb.map((v) => v.toString(16).padStart(2, "0")).join("");

// The area-weighted linear mean of a HOSALB1 grid (format: magic, u32 width,
// u32 height, then width*height*3 sRGB bytes, row 0 the northernmost).
function meanOfAlbedo(file) {
  const buf = fs.readFileSync(file);
  if (buf.slice(0, 7).toString("latin1") !== "HOSALB1") throw new Error(`${file}: not a HOSALB1 file`);
  const w = buf.readUInt32LE(7);
  const h = buf.readUInt32LE(11);
  const lut = Array.from({ length: 256 }, (_, i) => toLinear(i));
  const sum = [0, 0, 0];
  let weight = 0;
  for (let y = 0; y < h; y++) {
    const lat = (90 - ((y + 0.5) * 180) / h) * (Math.PI / 180);
    const wy = Math.cos(lat);
    let row = 15 + y * w * 3;
    const rs = [0, 0, 0];
    for (let x = 0; x < w; x++, row += 3) {
      rs[0] += lut[buf[row]];
      rs[1] += lut[buf[row + 1]];
      rs[2] += lut[buf[row + 2]];
    }
    for (let k = 0; k < 3; k++) sum[k] += rs[k] * wy;
    weight += wy * w;
  }
  return sum.map((s) => toSrgbByte(s / weight));
}

// The albedo map a body's planet def points at, if any.
function albedoMapFor(id) {
  const ron = path.join(PLANETS, `${id}.ron`);
  if (!fs.existsSync(ron)) return null;
  const m = /^\s*albedo:\s*Some\("([^"]+)"\)/m.exec(fs.readFileSync(ron, "utf8"));
  return m ? m[1] : null;
}

function main() {
  const text = fs.readFileSync(SOL, "utf8");
  const sol = JSON.parse(text);
  const report = [];
  for (const body of sol.bodies) {
    const map = albedoMapFor(body.id);
    const src = body.map_color_source || "";
    const handSet = body.map_color && !src.startsWith("mean of") && !src.startsWith("grey at");
    if (map) {
      const rgb = meanOfAlbedo(path.join(ROOT, "data", map));
      body.map_color = hex(rgb);
      body.map_color_source = `mean of data/${map} (area-weighted, linear light)`;
      report.push(`${body.id}: ${body.map_color} from ${map}`);
    } else if (handSet) {
      report.push(`${body.id}: ${body.map_color} kept (${src})`);
    } else {
      const a = body.physical && typeof body.physical.albedo === "number" ? body.physical.albedo : null;
      if (a === null) {
        report.push(`${body.id}: NO colour (no map, no hand colour, no albedo)`);
        continue;
      }
      const g = toSrgbByte(a);
      body.map_color = hex([g, g, g]);
      body.map_color_source = `grey at the body's albedo ${a} (no colour map yet)`;
      report.push(`${body.id}: ${body.map_color} grey`);
    }
  }
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  fs.writeFileSync(SOL, JSON.stringify(sol, null, 1).replace(/\n/g, eol) + eol);
  console.log(report.join("\n"));
}

main();
