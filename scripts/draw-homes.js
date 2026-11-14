#!/usr/bin/env node
// draw-homes.js: draw home designs as floor plans, FROM THE DATA THE GAME BUILDS (2026-11-13,
// docs/design/home-room-program.md section 7: "The drafts should then be drawn from those files, so
// the drawings and the game can never disagree").
//
// Usage:
//   node scripts/draw-homes.js <out folder> [kind ...]        shipped designs (data/homes/shipped/<kind>.ron);
//                                                             no kind draws every shipped design but the cabin
//   node scripts/draw-homes.js <out folder> --file <x.ron> [--terrace N] [--prefix p]
//                                                             a draft in the same shape (a row house before it
//                                                             is data), N side by side as a terrace
//
// For each storey it writes <prefix><kind>-<storey>.svg and, when Chrome or Edge is installed, a
// .png of it (headless, --screenshot), plus <prefix><kind>-rooms.md, the room table. The street is
// always at the top of the drawing, whichever wall of the design its door is in (the door is where
// the plot's corridor from the street arrives). Everything drawn is read from the file: zones
// (rooms and areas, each one's label and size), walls with their doors and windows, the floor slabs
// (an upper floor, a lid, a glass rooflight), stairs, lights, and the household's machines (the
// start's first household, data/homes/starts.ron, when the design is a start's).
//
// The layout CHECKS are Rust tests over the same files (crates/humanity-ship/src/home_program_tests.rs);
// this script only draws. Its own test: scripts/tests/draw-homes.test.js.
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const EPS = 1e-6;

// ───────────────────────────────────────────────────────────── RON (the subset the data uses)

/** Parse RON: structs and named structs as objects, tuples and lists as arrays, maps as objects,
 * strings, numbers, true/false, Some(x) as x, None as null, enum variants as their name. */
function parseRon(text) {
  let i = 0;
  const ws = () => {
    for (;;) {
      while (i < text.length && /\s/.test(text[i])) i++;
      if (text.startsWith('//', i)) while (i < text.length && text[i] !== '\n') i++;
      else if (text.startsWith('/*', i)) i = text.indexOf('*/', i) + 2;
      else return;
    }
  };
  const fail = (m) => {
    throw new Error(`RON: ${m} at ${i}: ${JSON.stringify(text.slice(i, i + 40))}`);
  };
  const ident = () => {
    const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(text.slice(i, i + 64));
    return m ? m[0] : null;
  };
  const value = () => {
    ws();
    const c = text[i];
    if (c === '"') {
      let s = '';
      i++;
      while (text[i] !== '"') {
        if (text[i] === '\\') {
          const e = text[i + 1];
          s += e === 'n' ? '\n' : e === 't' ? '\t' : e;
          i += 2;
        } else s += text[i++];
      }
      i++;
      return s;
    }
    if (c === '[') return seq('[', ']');
    if (c === '{') {
      i++;
      const out = {};
      for (;;) {
        ws();
        if (text[i] === '}') { i++; return out; }
        const k = value();
        ws();
        if (text[i] !== ':') fail('a colon in a map');
        i++;
        out[k] = value();
        ws();
        if (text[i] === ',') i++;
      }
    }
    if (c === '(') return paren();
    const n = /^-?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?/.exec(text.slice(i, i + 40));
    if (n) { i += n[0].length; return Number(n[0]); }
    const w = ident();
    if (!w) fail('a value');
    i += w.length;
    if (w === 'true') return true;
    if (w === 'false') return false;
    if (w === 'None') return null;
    ws();
    if (text[i] === '(') {
      if (w === 'Some') { i++; const v = value(); ws(); if (text[i] !== ')') fail('the end of Some'); i++; return v; }
      return paren();
    }
    return w;
  };
  const seq = (open, close) => {
    i++;
    const out = [];
    for (;;) {
      ws();
      if (text[i] === close) { i++; return out; }
      out.push(value());
      ws();
      if (text[i] === ',') i++;
    }
  };
  const paren = () => {
    // A struct when it opens with `name:` (not `name::`), else a tuple.
    const save = i;
    i++;
    ws();
    const w = ident();
    let isStruct = false;
    if (w) {
      let j = i + w.length;
      while (/\s/.test(text[j])) j++;
      isStruct = text[j] === ':' && text[j + 1] !== ':';
    }
    if (text[i] === ')') { i++; return []; }
    if (!isStruct) { i = save; return seq('(', ')'); }
    const out = {};
    for (;;) {
      ws();
      if (text[i] === ')') { i++; return out; }
      const k = ident();
      if (!k) fail('a field name');
      i += k.length;
      ws();
      if (text[i] !== ':') fail('a colon');
      i++;
      out[k] = value();
      ws();
      if (text[i] === ',') i++;
    }
  };
  const v = value();
  ws();
  if (i < text.length) fail('trailing text');
  return v;
}

// ───────────────────────────────────────────────────────────── the model, as the game reads it

const zrect = (z) => [z.origin[0], z.origin[2], z.origin[0] + z.size[0], z.origin[2] + z.size[2]];
const area = (r) => Math.max(0, r[2] - r[0]) * Math.max(0, r[3] - r[1]);
const roomOf = (id) => (id.includes('-part-') ? id.slice(0, id.indexOf('-part-')) : id);
const f2 = (n) => String(Math.round(n * 100) / 100);
const MATERIAL_GLASS = 4;

/** The storeys, as `HomeStructure::storeys` derives them: the ground, then each slab top. */
function storeys(body) {
  const tops = [...new Set((body.floors || []).map((s) => s.top))].filter((t) => t > EPS && t < body.height - EPS).sort((a, b) => a - b);
  const floors = [0, ...tops];
  return floors.map((fl, k) => ({ floor: fl, top: k + 1 < floors.length ? floors[k + 1] : body.height }));
}
const storeyIndex = (st, y) => {
  let k = 0;
  st.forEach((s, j) => { if (s.floor <= y + 1e-3) k = j; });
  return k;
};

/** Where the street is: the side of the shell the door point is on. */
function streetSide(design) {
  const [x, z] = design.door;
  const { width: w, depth: d } = design.body;
  if (Math.abs(x) < 1e-3) return 'west';
  if (Math.abs(x - w) < 1e-3) return 'east';
  if (Math.abs(z) < 1e-3) return 'north';
  return 'south';
}

/** Home-local (x, z) to drawing (u, v), the street at the top: [u, v, drawing width, height]. */
function frame(design) {
  const { width: w, depth: d } = design.body;
  switch (streetSide(design)) {
    case 'west': return { uv: (x, z) => [d - z, x], W: d, H: w };
    case 'east': return { uv: (x, z) => [z, w - x], W: d, H: w };
    case 'north': return { uv: (x, z) => [x, z], W: w, H: d };
    default: return { uv: (x, z) => [w - x, d - z], W: w, H: d };
  }
}

/** A zone's look: lidded rooms are the house and its sheds; open ones the garden. */
function category(body, z, glassEdge) {
  const r = zrect(z);
  const cx = (r[0] + r[2]) / 2, cz = (r[1] + r[3]) / 2;
  const lid = (body.floors || []).some((s) => z.origin[1] < s.top - EPS && cx >= Math.min(s.a[0], s.b[0]) && cx <= Math.max(s.a[0], s.b[0]) && cz >= Math.min(s.a[1], s.b[1]) && cz <= Math.max(s.a[1], s.b[1]));
  // A part of a room looks like its primary (the zone that carries the room type).
  const primary = body.zones.find((p) => p.id === roomOf(z.id)) || z;
  const t = primary.type_id, rt = primary.room_type;
  if (z.origin[1] > EPS) return 'house';
  if (lid) return t === 'room_workshop' ? 'work' : t === 'room_pantry' || t === 'room_garden' ? 'store' : t === 'agriculture' ? 'animal' : 'house';
  if (glassEdge) return 'garden';
  if (t === 'agriculture') return 'orchard';
  if (t === 'room_garden') return rt === 'garden' ? 'orchard' : 'yard';
  return 'path';
}

/** The room table: each room (a primary and its parts) with its label and size, a one-box room
 * measured along the street first, then deep (the drawings' frame). */
function roomTable(design) {
  const fr = frame(design);
  const rooms = new Map();
  for (const z of design.body.zones) {
    const id = roomOf(z.id);
    if (!rooms.has(id)) rooms.set(id, { id, label: z.label, rects: [], y: z.origin[1], room_type: null });
    const room = rooms.get(id);
    room.rects.push(zrect(z));
    if (z.room_type) room.room_type = z.room_type;
  }
  return [...rooms.values()].map((r) => {
    const a = r.rects.reduce((s, q) => s + area(q), 0);
    const [p, q] = r.rects.length === 1 ? [fr.uv(r.rects[0][0], r.rects[0][1]), fr.uv(r.rects[0][2], r.rects[0][3])] : [];
    const one = p ? `${f2(Math.abs(q[0] - p[0]))} x ${f2(Math.abs(q[1] - p[1]))} m, ` : '';
    return { ...r, area: a, size: `${one}${f2(a)} m2` };
  });
}

// ───────────────────────────────────────────────────────────── drawing

const FILL = { house: '#f4ead5', work: '#dde3ea', yard: '#e7f0da', path: '#efefec', garden: '#d3ecd2', store: '#e6def0', animal: '#f5dfc4', orchard: '#cde4bd' };
const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function wallStyle(w) {
  if (w.material === MATERIAL_GLASS) return ['#2f78c4', 2.2, ''];
  if (w.height < 1.5) return ['#2b2b2b', 1.2, ' stroke-dasharray="4 3"']; // a guard
  if (w.height < 2.5) return ['#7a8a5a', 1.2, ' stroke-dasharray="6 3"']; // a garden fence
  return ['#2b2b2b', (w.thickness || 0.1) >= 0.14 ? 3 : 2, ''];
}

function doorStyle(o, w) {
  if (o.kind === 'Window') return null;
  if (o.style === 'fold') return ['#e07b00', 'Folding vehicle door'];
  if (w.material === MATERIAL_GLASS || o.style === 'slide' && w.height >= 2.5) return ['#2f78c4', 'Sliding or glass door'];
  if (w.height < 2.5) return ['#3c8d2f', 'Gate'];
  return ['#7a4a1a', 'Door'];
}

/** The storey a light belongs to: the highest at or below it with a zone holding it (a light high in
 * a room that rises through two storeys, the greenhouse or the garden, is that room's). */
function lightStorey(body, st, l) {
  for (let k = storeyIndex(st, l.pos[1]); k >= 0; k--) {
    if (body.zones.some((z) => storeyIndex(st, z.origin[1]) === k && l.pos[0] > zrect(z)[0] && l.pos[0] < zrect(z)[2] && l.pos[2] > zrect(z)[1] && l.pos[2] < zrect(z)[3])) return k;
  }
  return storeyIndex(st, l.pos[1]);
}

/** One storey of one design as an SVG (`copies` side by side for a terrace). */
function drawStorey(design, k, opts = {}) {
  const body = design.body;
  const st = storeys(body);
  const fr = frame(design);
  const copies = opts.copies || 1;
  const s = opts.scale || Math.max(8, Math.min(24, 900 / (fr.W * copies)));
  const m = 30, top = 92;
  const plotW = fr.W * s * copies, plotH = fr.H * s;
  const width = Math.max(plotW + 2 * m, 760), height = top + plotH + 170;
  const out = [];
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${f2(width)}" height="${f2(height)}" viewBox="0 0 ${f2(width)} ${f2(height)}" font-family="Segoe UI, Arial, sans-serif">`);
  out.push('<defs><marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#2b2b2b"/></marker><pattern id="hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="6" stroke="#bbb" stroke-width="1.5"/></pattern></defs>');
  out.push(`<rect width="${f2(width)}" height="${f2(height)}" fill="#ffffff"/>`);
  const title = opts.title || design.kind;
  const storeyName = k === 0 ? 'Ground floor' : `Floor ${k} (at ${f2(st[k].floor)} m)`;
  out.push(`<text x="${m}" y="28" font-size="18" font-weight="700">${esc(title)}: ${esc(storeyName)}</text>`);
  out.push(`<text x="${m}" y="48" font-size="11" fill="#555">${esc(opts.source || `Drawn from data/homes/shipped/${design.kind}.ron by scripts/draw-homes.js`)}: every size is the file's own.</text>`);
  out.push(`<text x="${m}" y="64" font-size="11" fill="#555">Plot ${f2(fr.W)} m along the street, ${f2(fr.H)} m deep${copies > 1 ? `; ${copies} homes side by side` : ''}. Top of the drawing = the street.</text>`);
  out.push(`<rect x="${m}" y="${top - 22}" width="${f2(plotW)}" height="18" fill="#cfcfcf"/><text x="${m + plotW / 2}" y="${top - 9}" font-size="11" text-anchor="middle">STREET</text>`);
  for (let c = 0; c < copies; c++) {
    const P = (x, z) => {
      const [u, v] = fr.uv(x, z);
      return [m + (c * fr.W + u) * s, top + v * s];
    };
    const R = (r) => {
      const a = P(r[0], r[1]), b = P(r[2], r[3]);
      return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1])];
    };
    const onK = (y) => storeyIndex(st, y) === k;
    // A greenhouse: more than half of its room's edges inside the plot are glass walls.
    const glassOn = (z) => {
      const rects = body.zones.filter((p) => roomOf(p.id) === roomOf(z.id)).map(zrect);
      let edge = 0, glass = 0;
      for (const r of rects) {
        for (const [ax, az, bx, bz] of [[r[0], r[1], r[2], r[1]], [r[0], r[3], r[2], r[3]], [r[0], r[1], r[0], r[3]], [r[2], r[1], r[2], r[3]]]) {
          const vertical = Math.abs(ax - bx) < EPS;
          const at = vertical ? ax : az, lo = vertical ? az : ax, hi = vertical ? bz : bx;
          if (vertical ? at < EPS || at > body.width - EPS : at < EPS || at > body.depth - EPS) continue; // the shell
          // Skip the stretches shared with another box of the same room (no wall there).
          const shared = rects.filter((q) => q !== r).reduce((sum, q) => sum + (vertical ? (Math.abs(q[0] - at) < EPS || Math.abs(q[2] - at) < EPS ? Math.max(0, Math.min(hi, q[3]) - Math.max(lo, q[1])) : 0) : (Math.abs(q[1] - at) < EPS || Math.abs(q[3] - at) < EPS ? Math.max(0, Math.min(hi, q[2]) - Math.max(lo, q[0])) : 0)), 0);
          edge += hi - lo - shared;
          for (const w of body.walls.filter((w) => w.material === MATERIAL_GLASS && onK(w.base || 0))) {
            const wv = Math.abs(w.a[0] - w.b[0]) < EPS;
            if (wv !== vertical || Math.abs((wv ? w.a[0] : w.a[1]) - at) > EPS) continue;
            const [w0, w1] = wv ? [Math.min(w.a[1], w.b[1]), Math.max(w.a[1], w.b[1])] : [Math.min(w.a[0], w.b[0]), Math.max(w.a[0], w.b[0])];
            glass += Math.max(0, Math.min(hi, w1) - Math.max(lo, w0));
          }
        }
      }
      return edge > 0 && glass > edge * 0.5;
    };
    // The storey below, ghosted, under an upper floor.
    if (k > 0) for (const z of body.zones.filter((z) => storeyIndex(st, z.origin[1]) === k - 1)) {
      const [x, y, w, h] = R(zrect(z));
      out.push(`<rect x="${f2(x)}" y="${f2(y)}" width="${f2(w)}" height="${f2(h)}" fill="#f6f6f6" stroke="#d0d0d0" stroke-width="0.6"/>`);
    }
    // Slabs this storey stands on (upstairs): solid floor shaded, glass outlined.
    if (k > 0) for (const sl of (body.floors || []).filter((f) => Math.abs(f.top - st[k].floor) < 1e-3)) {
      const [x, y, w, h] = R([Math.min(sl.a[0], sl.b[0]), Math.min(sl.a[1], sl.b[1]), Math.max(sl.a[0], sl.b[0]), Math.max(sl.a[1], sl.b[1])]);
      out.push(`<rect x="${f2(x)}" y="${f2(y)}" width="${f2(w)}" height="${f2(h)}" fill="${sl.material === 'Glass' ? 'none' : '#ebe4d6'}" stroke="#c8bfae" stroke-width="0.5"/>`);
    }
    const zones = body.zones.filter((z) => onK(z.origin[1]));
    for (const z of zones) {
      const [x, y, w, h] = R(zrect(z));
      const cat = category(body, z, glassOn(z));
      const ink = k > 0 ? FILL.house : FILL[cat];
      out.push(`<rect x="${f2(x)}" y="${f2(y)}" width="${f2(w)}" height="${f2(h)}" fill="${ink}" fill-opacity="${k > 0 ? 0.9 : 1}"/>`);
    }
    // Where an upper zone has no floor: the stairwell.
    if (k > 0) for (const sp of body.structures || []) {
      const ty = (opts.structureTypes || {})[sp.type_id];
      if (!ty || ty.kind !== 'Stairs') continue;
      const [hw, hd] = sp.rot_deg % 180 === 0 ? [ty.size[0] / 2, ty.size[2] / 2] : [ty.size[2] / 2, ty.size[0] / 2];
      const [x, y, w, h] = R([sp.pos[0] - hw, sp.pos[2] - hd, sp.pos[0] + hw, sp.pos[2] + hd]);
      out.push(`<rect x="${f2(x)}" y="${f2(y)}" width="${f2(w)}" height="${f2(h)}" fill="url(#hatch)"/>`);
    }
    // Walls of this storey, then their openings.
    for (const w of body.walls.filter((w) => onK(w.base || 0))) {
      const [col, wt, dash] = wallStyle(w);
      const a = P(...w.a), b = P(...w.b);
      out.push(`<line x1="${f2(a[0])}" y1="${f2(a[1])}" x2="${f2(b[0])}" y2="${f2(b[1])}" stroke="${col}" stroke-width="${wt}" stroke-linecap="square"${dash}/>`);
      const len = Math.hypot(w.b[0] - w.a[0], w.b[1] - w.a[1]);
      for (const o of w.openings || []) {
        const t0 = o.at / len, t1 = (o.at + o.width) / len;
        const p = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0], q = [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
        out.push(`<line x1="${f2(p[0])}" y1="${f2(p[1])}" x2="${f2(q[0])}" y2="${f2(q[1])}" stroke="#fff" stroke-width="${wt + 2}"/>`);
        const ds = doorStyle(o, w);
        if (!ds) {
          out.push(`<line x1="${f2(p[0])}" y1="${f2(p[1])}" x2="${f2(q[0])}" y2="${f2(q[1])}" stroke="#2f78c4" stroke-width="1" stroke-dasharray="1 1.5"/>`);
          continue;
        }
        out.push(`<line x1="${f2(p[0])}" y1="${f2(p[1])}" x2="${f2(q[0])}" y2="${f2(q[1])}" stroke="${ds[0]}" stroke-width="1.6"/>`);
      }
    }
    // The plot's front gate: where the corridor from the street arrives.
    if (k === 0) {
      const g = P(...design.door);
      out.push(`<circle cx="${f2(g[0])}" cy="${f2(g[1])}" r="4" fill="#c0392b"/>`);
    }
    // Stairs (on the storey they start from), lights, machines.
    for (const sp of body.structures || []) {
      const ty = (opts.structureTypes || {})[sp.type_id];
      if (!ty || ty.kind !== 'Stairs' || !onK(sp.pos[1])) continue;
      const rad = (sp.rot_deg * Math.PI) / 180, sn = Math.sin(rad), cs = Math.cos(rad);
      const L = (lx, lz) => P(sp.pos[0] + lx * cs + lz * sn, sp.pos[2] - lx * sn + lz * cs);
      const [hw, hd] = [ty.size[0] / 2, ty.size[2] / 2];
      for (let i = 0; i <= ty.steps; i++) {
        const lz = -hd + (2 * hd * i) / ty.steps;
        const [a, b] = [L(-hw, lz), L(hw, lz)];
        out.push(`<line x1="${f2(a[0])}" y1="${f2(a[1])}" x2="${f2(b[0])}" y2="${f2(b[1])}" stroke="#555" stroke-width="0.6"/>`);
      }
      const [a, b] = [L(0, -hd + 0.1), L(0, hd - 0.2)];
      out.push(`<line x1="${f2(a[0])}" y1="${f2(a[1])}" x2="${f2(b[0])}" y2="${f2(b[1])}" stroke="#2b2b2b" stroke-width="1.2" marker-end="url(#arrow)"/>`);
    }
    for (const l of (body.lights || []).filter((l) => lightStorey(body, st, l) === k)) {
      const p = P(l.pos[0], l.pos[2]);
      out.push(`<circle cx="${f2(p[0])}" cy="${f2(p[1])}" r="2.2" fill="#f5c400" stroke="#8a6d00" stroke-width="0.5"/>`);
    }
    for (const mi of (opts.machines || []).filter((mi) => mi.zone === 'home' && onK(mi.offset[1]))) {
      const p = P(mi.offset[0], mi.offset[2]);
      if (mi.machine.includes('fan')) out.push(`<circle cx="${f2(p[0])}" cy="${f2(p[1])}" r="2.6" fill="#fff" stroke="#5a7d9a"/>`);
      else out.push(`<rect x="${f2(p[0] - 3)}" y="${f2(p[1] - 3)}" width="6" height="6" fill="#8d8d8d"/>`);
    }
    // Labels, the primary zone of each room: name and size.
    const table = roomTable(design);
    for (const room of table.filter((r) => storeyIndex(st, r.y) === k)) {
      const big = room.rects.slice().sort((p, q) => area(q) - area(p))[0];
      const [x, y, w, h] = R(big);
      const name = room.label.split(':')[0];
      const fit = (fs0, ww, hh) => Math.max(name.length, room.size.length) * fs0 * 0.55 < ww - 3 && fs0 * 2.4 < hh;
      let fs0 = [11, 9.5, 8.5, 7.5, 6.5].find((n) => fit(n, w, h));
      let rot = 0;
      if (!fs0) { fs0 = [11, 9.5, 8.5, 7.5, 6.5].find((n) => fit(n, h, w)); rot = fs0 ? -90 : 0; }
      fs0 = fs0 || 6;
      const cx = x + w / 2, cy = y + h / 2;
      const tr = rot ? ` transform="rotate(${rot} ${f2(cx)} ${f2(cy)})"` : '';
      out.push(`<g${tr}><text x="${f2(cx)}" y="${f2(cy - fs0 * 0.1)}" font-size="${fs0}" text-anchor="middle" font-weight="600">${esc(name)}</text><text x="${f2(cx)}" y="${f2(cy + fs0 * 1.0)}" font-size="${f2(fs0 * 0.85)}" text-anchor="middle" fill="#444">${esc(room.size)}</text></g>`);
    }
  }
  // Each plot's shell: a terrace's party walls between neighbours.
  for (let c = 0; c < copies; c++) out.push(`<rect x="${f2(m + c * fr.W * s)}" y="${top}" width="${f2(fr.W * s)}" height="${f2(plotH)}" fill="none" stroke="#000" stroke-width="1.5"/>`);
  // Scale bar and key.
  const ky = top + plotH + 28;
  out.push('<g font-size="10" fill="#333">');
  for (let i = 0; i < 10; i += 5) out.push(`<rect x="${m + i * s}" y="${ky}" width="${5 * s}" height="5" fill="${i ? '#fff' : '#333'}" stroke="#333" stroke-width="0.6"/>`);
  out.push(`<text x="${m}" y="${ky + 16}">0</text><text x="${m + 10 * s}" y="${ky + 16}" text-anchor="middle">10 m</text></g>`);
  const keys = [['#2b2b2b', '', 'Wall (3 m)'], ['#2f78c4', '', 'Glass wall'], ['#7a8a5a', '6 3', 'Garden fence (2 m)'], ['#2b2b2b', '4 3', 'Guard rail (1 m)'], ['#7a4a1a', '', 'Door'], ['#2f78c4', '', 'Sliding or glass door'], ['#3c8d2f', '', 'Gate'], ['#e07b00', '', 'Folding vehicle door']];
  keys.forEach(([c, d, t], i) => {
    const lx = m + (i % 4) * 185, ly = ky + 34 + Math.floor(i / 4) * 18;
    out.push(`<line x1="${lx}" y1="${ly}" x2="${lx + 22}" y2="${ly}" stroke="${c}" stroke-width="2.2" stroke-dasharray="${d}"/><text x="${lx + 28}" y="${ly + 4}" font-size="11">${t}</text>`);
  });
  const ly = ky + 34 + 2 * 18;
  out.push(`<circle cx="${m + 6}" cy="${ly}" r="4" fill="#c0392b"/><text x="${m + 16}" y="${ly + 4}" font-size="11">Front gate (from the street)</text>`);
  out.push(`<circle cx="${m + 206}" cy="${ly}" r="2.4" fill="#f5c400" stroke="#8a6d00" stroke-width="0.5"/><text x="${m + 214}" y="${ly + 4}" font-size="11">Light</text>`);
  out.push(`<circle cx="${m + 276}" cy="${ly}" r="2.6" fill="#fff" stroke="#5a7d9a"/><text x="${m + 284}" y="${ly + 4}" font-size="11">Fan</text><rect x="${m + 322}" y="${ly - 3}" width="6" height="6" fill="#8d8d8d"/><text x="${m + 334}" y="${ly + 4}" font-size="11">Furniture</text>`);
  out.push('</svg>');
  return out.join('\n');
}

/** Chrome or Edge, for the PNGs; null when neither is installed. */
function browser() {
  const tries = [
    process.env.CHROME_PATH,
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ].filter(Boolean);
  return tries.find((p) => fs.existsSync(p)) || null;
}

function toPng(svgPath, exe, profile) {
  const svg = fs.readFileSync(svgPath, 'utf8');
  const m = /<svg[^>]*width="([0-9.]+)" height="([0-9.]+)"/.exec(svg);
  const png = svgPath.replace(/\.svg$/, '.png');
  const r = spawnSync(exe, ['--headless=new', '--disable-gpu', '--hide-scrollbars', `--user-data-dir=${profile}`, `--window-size=${Math.ceil(m[1])},${Math.ceil(m[2])}`, `--screenshot=${png}`, `file:///${svgPath.replace(/\\/g, '/')}`], { timeout: 60000, windowsHide: true });
  return r.status === 0 && fs.existsSync(png) ? png : null;
}

/** The household machines of the start whose design this is (its first household), if any. */
function machinesFor(kind) {
  try {
    const starts = parseRon(fs.readFileSync(path.join(ROOT, 'data/homes/starts.ron'), 'utf8'));
    const start = starts.starts.find((s) => s.design === kind);
    if (!start) return [];
    const file = parseRon(fs.readFileSync(path.join(ROOT, 'data/machines', start.households[0].machines), 'utf8'));
    return file.instances || [];
  } catch {
    return [];
  }
}

function structureTypes() {
  const list = parseRon(fs.readFileSync(path.join(ROOT, 'data/blueprints/structure_types.ron'), 'utf8'));
  return Object.fromEntries(list.map((t) => [t.id, t]));
}

function draw(design, outDir, opts) {
  const st = storeys(design.body);
  const written = [];
  const prefix = opts.prefix || '';
  const types = structureTypes();
  for (let k = 0; k < st.length; k++) {
    if (k > 0 && !design.body.zones.some((z) => storeyIndex(st, z.origin[1]) === k)) continue;
    const name = `${prefix}${design.kind}-${k === 0 ? 'ground' : `floor${k}`}.svg`;
    const file = path.join(outDir, name);
    fs.writeFileSync(file, drawStorey(design, k, { ...opts, structureTypes: types }));
    written.push(file);
  }
  const table = roomTable(design);
  const md = [`# ${opts.title || design.kind}: rooms`, '', `${opts.source || `Drawn from data/homes/shipped/${design.kind}.ron by scripts/draw-homes.js`}.`, ''];
  for (let k = 0; k < st.length; k++) {
    const rows = table.filter((r) => storeyIndex(st, r.y) === k);
    if (!rows.length) continue;
    md.push(`## ${k === 0 ? 'Ground floor' : `Floor ${k}`}`, '', '| Room or area | Room type | Size |', '|---|---|---|', ...rows.map((r) => `| ${r.label} | ${r.room_type || ''} | ${r.size} |`), '');
  }
  const mdFile = path.join(outDir, `${prefix}${design.kind}-rooms.md`);
  fs.writeFileSync(mdFile, md.join('\n'));
  written.push(mdFile);
  return written;
}

function main(argv) {
  const out = argv[0];
  if (!out) {
    console.error('usage: node scripts/draw-homes.js <out folder> [kind ...] | --file <design.ron> [--terrace N] [--prefix p] [--title t]');
    process.exit(2);
  }
  fs.mkdirSync(out, { recursive: true });
  const rest = argv.slice(1);
  const opt = (name) => {
    const i = rest.indexOf(name);
    return i >= 0 ? rest.splice(i, 2)[1] : undefined;
  };
  const file = opt('--file');
  const copies = Number(opt('--terrace') || 1);
  const prefix = opt('--prefix') || '';
  const title = opt('--title');
  const jobs = [];
  if (file) {
    const design = parseRon(fs.readFileSync(file, 'utf8'));
    jobs.push([design, { copies, prefix, title, source: `Drawn from ${path.basename(file)} (a draft, not yet game data) by scripts/draw-homes.js` }]);
  } else {
    const dir = path.join(ROOT, 'data/homes/shipped');
    const kinds = rest.length ? rest : fs.readdirSync(dir).filter((n) => n.endsWith('.ron')).map((n) => n.slice(0, -4)).filter((k) => k !== 'cabin');
    for (const kind of kinds) {
      const design = parseRon(fs.readFileSync(path.join(dir, `${kind}.ron`), 'utf8'));
      jobs.push([design, { copies, prefix, title, machines: machinesFor(kind) }]);
    }
  }
  const exe = browser();
  const profile = path.join(require('os').tmpdir(), 'draw-homes-browser');
  for (const [design, o] of jobs) {
    for (const w of draw(design, out, o)) {
      console.log(w);
      if (w.endsWith('.svg') && exe) {
        const png = toPng(path.resolve(w), exe, profile);
        console.log(png || `  (no PNG: ${path.basename(exe)} did not write one)`);
      }
    }
  }
  if (!exe) console.log('No Chrome or Edge found (set CHROME_PATH): SVGs only.');
}

if (require.main === module) main(process.argv.slice(2));
module.exports = { parseRon, storeys, streetSide, frame, roomTable, drawStorey };
