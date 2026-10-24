// The LEVEL PICKER for the web (Maps increment 3, docs/design/maps-zoom-to-anything.md 5.7):
// the web twin of the app's src/gui/widgets/level_picker.rs. A multi-level place's levels are
// listed TOP TO BOTTOM (the highest first, as a building's directory and a lift's panel read),
// the level you are on marked "you", the level shown highlighted, then "Whole" for the place
// as a whole. `pickerRows` is the same rule as the app's `picker_rows`; keep the two in step.
(function (root) {
  'use strict';

  /** The picker's rows: `levels` ({ordinal, short, name}) from the top one down, then Whole.
   *  `you` is the ordinal of the level you are on (or null), `shown` the one shown (null: Whole). */
  function pickerRows(levels, you, shown) {
    const rows = levels
      .slice()
      .sort((a, b) => b.ordinal - a.ordinal)
      .map((l) => {
        const here = you === l.ordinal;
        return {
          ordinal: l.ordinal,
          label: here ? `${l.short}  · you` : l.short,
          tooltip: here ? `${l.name} (you are on this level)` : l.name,
          selected: shown === l.ordinal,
          you: here,
        };
      });
    rows.push({ ordinal: null, label: 'Whole', tooltip: 'The whole place, nothing cut away', selected: shown === null || shown === undefined, you: false });
    return rows;
  }

  /** Draw the rows into `el` (emptied first) as buttons; `onPick(ordinal | null)` on a click. */
  function renderPicker(el, title, rows, onPick) {
    el.textContent = '';
    const head = document.createElement('div');
    head.className = 'level-picker-title';
    head.textContent = title;
    el.appendChild(head);
    for (const r of rows) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'level-picker-row' + (r.selected ? ' active' : '') + (r.you ? ' you' : '');
      b.textContent = r.label;
      b.title = r.tooltip;
      b.setAttribute('aria-pressed', r.selected ? 'true' : 'false');
      b.addEventListener('click', () => onPick(r.ordinal));
      el.appendChild(b);
    }
  }

  const api = { pickerRows, renderPicker };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.HosLevelPicker = api;
  }
})(typeof window !== 'undefined' ? window : this);
