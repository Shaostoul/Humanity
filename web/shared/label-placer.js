// Map label placement: the web twin of src/gui/widgets/label_placer.rs.
// Keep the two in step (same rules, same candidate order).
//
// The greedy collision pass cartography uses for point features. Labels are
// offered in PRIORITY order (the brightest star first); each brings candidate
// rectangles in PREFERENCE order (right of the point, left, above, below).
// The first candidate that is inside the map and clear of everything already
// placed wins; a label with no clear candidate is dropped, so the more
// important name keeps its place. Fixed things a label must not cover (a
// scale label, the dot of a star) are reserved up front.
//
// Rectangles are plain objects {x0, y0, x1, y1} in canvas pixels.
(function (root) {
  'use strict';

  function intersects(a, b) {
    return a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1;
  }

  // An empty map. `bounds` is where labels may go (wholly inside it); `pad`
  // is the clear gap kept around each placed label.
  function LabelPlacer(bounds, pad) {
    this.bounds = bounds;
    this.pad = pad || 0;
    this.taken = [];
  }

  // Mark an area no label may cover.
  LabelPlacer.prototype.reserve = function (r) {
    const p = this.pad;
    this.taken.push({ x0: r.x0 - p, y0: r.y0 - p, x1: r.x1 + p, y1: r.y1 + p });
  };

  // Mark an obstacle held to its exact outline, with no gap: a star's dot.
  // Its own label already sits `gap` away; padding the dot too would block
  // every candidate of that label once the pad reached the gap.
  LabelPlacer.prototype.reserveExact = function (r) {
    this.taken.push({ x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y1 });
  };

  // Whether `r` is inside the bounds and clear of everything placed.
  LabelPlacer.prototype.isFree = function (r) {
    const b = this.bounds;
    if (r.x0 < b.x0 || r.y0 < b.y0 || r.x1 > b.x1 || r.y1 > b.y1) return false;
    for (const t of this.taken) {
      if (intersects(t, r)) return false;
    }
    return true;
  };

  // Place the label at the first free candidate and return where it went, or
  // null when every candidate is blocked (the label is dropped).
  LabelPlacer.prototype.place = function (candidates) {
    for (const r of candidates) {
      if (this.isFree(r)) {
        this.reserve(r);
        return r;
      }
    }
    return null;
  };

  // The four standard candidate positions around a point of radius `r` at
  // (x, y) for a label `w` x `h`, in order of preference: right, left,
  // centred above, centred below. `gap` is the space between dot and label.
  function pointCandidates(x, y, r, w, h, gap) {
    const d = r + gap;
    return [
      { x0: x + d, y0: y - h / 2, x1: x + d + w, y1: y + h / 2 },
      { x0: x - d - w, y0: y - h / 2, x1: x - d, y1: y + h / 2 },
      { x0: x - w / 2, y0: y - d - h, x1: x + w / 2, y1: y - d },
      { x0: x - w / 2, y0: y + d, x1: x + w / 2, y1: y + d + h },
    ];
  }

  // The clear gap kept between two names: a little wider than a space, so
  // two neighbouring names never read as one. Same as the app's LABEL_GAP.
  const LABEL_GAP = 3;

  const api = { LabelPlacer, pointCandidates, LABEL_GAP };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.HosLabels = api;
  }
})(typeof window !== 'undefined' ? window : this);
