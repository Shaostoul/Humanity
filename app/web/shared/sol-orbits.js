// The Sun's family for the web Maps page (Maps increment 2, "a solar system
// that looks real", docs/design/maps-zoom-to-anything.md): the web twin of the
// app's orbit model (src/cosmos.rs) and of the Maps page's symbols
// (src/gui/pages/maps/bodies.rs, belts.rs). Keep the two in step.
//
// It reads the SAME data files the app does, served beside the site:
//   /data/star_systems/sol.json  every body's orbit, size and `map_color`
//   /data/maps/belts.json        20,000 real minor-planet orbits (the Minor
//                                Planet Center's MPCORB; terms in
//                                docs/reference/findings/2026-10-21-mpc-orbit-data.md)
// and moves every body along its real ellipse to the page's date, so the
// planets stand where they really are and the belts are the real ones (the
// page used to place the planets at a golden-angle spread and draw the belts
// as a random ring).
//
// Positions are heliocentric, in AU, in the ecliptic frame of J2000: x toward
// the March equinox, z toward ecliptic north (the frame JPL Horizons quotes).
(function (root) {
  'use strict';

  const DEG = Math.PI / 180;
  const TWO_PI = Math.PI * 2;
  const KM_PER_AU = 149597870.7;
  const J2000_MS = Date.UTC(2000, 0, 1, 12, 0, 0);
  const JULIAN_YEAR_S = 365.25 * 86400;
  // The Sun's mean-motion law the MPC's own elements use: k / a^1.5 rad a day.
  const GAUSS_K = 0.01720209895;
  const JD_J2000 = 2451545.0;

  /** Seconds since J2000.0 for a time in ms (now by default). */
  function secondsSinceJ2000(ms) {
    return ((ms === undefined ? Date.now() : ms) - J2000_MS) / 1000;
  }

  /** Kepler's equation M = E - e sin E, solved for E (as cosmos::kepler_solve). */
  function kepler(M, e) {
    let E = M;
    for (let i = 0; i < 12; i++) {
      const d = (E - e * Math.sin(E) - M) / (1 - e * Math.cos(E));
      E -= d;
      if (Math.abs(d) < 1e-12) break;
    }
    return E;
  }

  /** The perifocal axes P (toward periapsis) and Q in the ecliptic frame. */
  function perifocal(nodeDeg, incDeg, periDeg) {
    const so = Math.sin(nodeDeg * DEG), co = Math.cos(nodeDeg * DEG);
    const si = Math.sin(incDeg * DEG), ci = Math.cos(incDeg * DEG);
    const sw = Math.sin(periDeg * DEG), cw = Math.cos(periDeg * DEG);
    return {
      P: [co * cw - so * sw * ci, so * cw + co * sw * ci, sw * si],
      Q: [-co * sw - so * cw * ci, -so * sw + co * cw * ci, cw * si],
    };
  }

  function precessionDegPerSec(years) {
    return years ? 360 / (years * JULIAN_YEAR_S) : 0;
  }

  /** A sol.json body's orbit, ready to move (null for the Sun). */
  function bodyOrbit(b) {
    const o = b.orbit || {};
    const a = o.semi_major_axis_au > 0 ? o.semi_major_axis_au : o.semi_major_axis_km > 0 ? o.semi_major_axis_km / KM_PER_AU : 0;
    if (!(a > 0)) return null;
    const nodeRate = precessionDegPerSec(o.longitude_ascending_node_precession_years || 0);
    const periRate = precessionDegPerSec(o.argument_perihelion_precession_years || 0);
    // As cosmos::mean_anomaly_deg_at: a body with no mean anomaly in the data
    // starts at one hashed from its name, so they do not all sit at periapsis.
    let m0 = o.mean_anomaly_deg || 0;
    if (!m0) {
      let s = 0;
      for (const ch of new TextEncoder().encode(b.name || b.id)) s += ch;
      m0 = s % 360;
    }
    const period = o.orbital_period_days || 0;
    return {
      id: b.id,
      parent: b.parent,
      a,
      e: Math.min(Math.max(o.eccentricity || 0, 0), 0.99),
      inc: o.inclination_deg || 0,
      node0: o.longitude_ascending_node_deg || 0,
      peri0: o.argument_perihelion_deg || 0,
      nodeRate,
      periRate,
      m0,
      mRate: period > 0 ? 360 / (period * 86400) - nodeRate - periRate : 0,
    };
  }

  /** Where on its ellipse (eccentric anomaly, radians) an orbit is at t. */
  function eccentricAnomalyAt(orb, t) {
    const M = ((((orb.m0 + orb.mRate * t) % 360) + 360) % 360) * DEG;
    return kepler(M, orb.e);
  }

  /** The point at eccentric anomaly E on the orbit as it lies at t, AU,
   *  relative to its parent. */
  function orbitPoint(orb, E, t) {
    const { P, Q } = perifocal(orb.node0 + orb.nodeRate * t, orb.inc, orb.peri0 + orb.periRate * t);
    const x = orb.a * (Math.cos(E) - orb.e);
    const y = orb.a * Math.sqrt(1 - orb.e * orb.e) * Math.sin(E);
    return [P[0] * x + Q[0] * y, P[1] * x + Q[1] * y, P[2] * x + Q[2] * y];
  }

  /** The body's place at t relative to its parent, AU. */
  function positionAU(orb, t) {
    return orbitPoint(orb, eccentricAnomalyAt(orb, t), t);
  }

  /** "#rrggbb" to [r, g, b] bytes, or null. */
  function hexRgb(s) {
    const m = /^#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(s || '');
    return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : null;
  }

  function toLinear(b) {
    const c = b / 255;
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  }

  function toSrgb(l) {
    const c = Math.min(Math.max(l, 0), 1);
    return Math.round((c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055) * 255);
  }

  /** A body's lit colour, as the app paints its icon (bodies::icon_lit_linear):
   *  its map colour through a camera's exposure curve, hue kept, so a 4%
   *  comet still shows. Returns {lit: 'rgb()', night: 'rgb()'}. */
  function iconColours(hex) {
    const c = hexRgb(hex) || [140, 140, 140];
    const l = c.map(toLinear);
    const m = Math.max(l[0], l[1], l[2]);
    const k = m > 1e-6 ? Math.pow(m, 0.4) / m : 0;
    const lit = l.map((v) => (m > 1e-6 ? v * k : 0.25));
    const rgb = (f) => `rgb(${lit.map((v) => toSrgb(v * f)).join(',')})`;
    return { lit: rgb(1), night: rgb(0.1) };
  }

  /** The Sun's glare at `deg` from its centre, 0 to 1: the two far terms of
   *  Spencer, Shirley, Zimmerman and Greenberg's point-spread function (1995),
   *  at the app's exposure (bodies::glare_at). */
  function glareAt(deg) {
    const t = Math.max(deg, 0) + 0.02;
    const psf = (0.478 * 20.91) / (t * t * t) + (0.138 * 72.37) / (t * t);
    return 1 - Math.exp(-0.1 * psf);
  }

  /** Load sol.json: {orbits: Map id -> orbit, bodies: Map id -> body}. */
  function loadSol() {
    return fetch('/data/star_systems/sol.json')
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (!data || !Array.isArray(data.bodies)) return null;
        const bodies = new Map();
        const orbits = new Map();
        for (const b of data.bodies) {
          if (b.type === 'region') continue;
          bodies.set(b.id, b);
          const orb = bodyOrbit(b);
          if (orb) orbits.set(b.id, orb);
        }
        return { bodies, orbits };
      });
  }

  /** Load belts.json: {classes, credit, n, cls (Uint8Array), and per orbit
   *  P, Q, a, b, e, m0, n, epoch in typed arrays}. */
  function loadBelts() {
    return fetch('/data/maps/belts.json')
      .then((r) => (r.ok ? r.json() : null))
      .then((f) => {
        if (!f || f.format !== 'humanityos-belts-1' || !Array.isArray(f.rows)) return null;
        const n = f.rows.length;
        const set = {
          classes: f.classes || [],
          credit: f.credit || '',
          n,
          cls: new Uint8Array(n),
          axes: new Float64Array(n * 6),
          ab: new Float64Array(n * 3),
          time: new Float64Array(n * 3),
        };
        f.rows.forEach((row, i) => {
          const [, cls, , epochJd, a, e, inc, node, peri, M] = row;
          const { P, Q } = perifocal(node, inc, peri);
          set.cls[i] = cls;
          set.axes.set([P[0], P[1], P[2], Q[0], Q[1], Q[2]], i * 6);
          set.ab.set([a, a * Math.sqrt(1 - e * e), e], i * 3);
          set.time.set([M * DEG, GAUSS_K / Math.pow(a, 1.5) / 86400, (epochJd - JD_J2000) * 86400], i * 3);
        });
        return set;
      });
  }

  /** Every belt orbit's place at t, AU: a Float64Array of x, y, z. */
  function propagateBelts(set, t) {
    const out = new Float64Array(set.n * 3);
    for (let i = 0; i < set.n; i++) {
      const [m0, n, epoch] = [set.time[i * 3], set.time[i * 3 + 1], set.time[i * 3 + 2]];
      const [a, b, e] = [set.ab[i * 3], set.ab[i * 3 + 1], set.ab[i * 3 + 2]];
      const M = (((m0 + n * (t - epoch)) % TWO_PI) + TWO_PI) % TWO_PI;
      const E = kepler(M, e);
      const x = a * (Math.cos(E) - e);
      const y = b * Math.sin(E);
      const k = i * 6;
      out[i * 3] = set.axes[k] * x + set.axes[k + 3] * y;
      out[i * 3 + 1] = set.axes[k + 1] * x + set.axes[k + 4] * y;
      out[i * 3 + 2] = set.axes[k + 2] * x + set.axes[k + 5] * y;
    }
    return out;
  }

  const api = {
    secondsSinceJ2000,
    kepler,
    bodyOrbit,
    eccentricAnomalyAt,
    orbitPoint,
    positionAU,
    iconColours,
    glareAt,
    loadSol,
    loadBelts,
    propagateBelts,
    hexRgb,
  };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.HosSol = api;
  }
})(typeof window !== 'undefined' ? window : this);
