// view/timestampPill.js, clean view module for the message timestamp pill.
//
// Mirrors native's paint_timestamp_pill (src/gui/pages/chat.rs): ONE small rounded frame holding,
// left to right, the time, the Þ, and the message's reactions, each its emoji and its count
// ("👍1"), yours in the accent on a tinted ground. Clicking a reaction adds or takes back yours
// (app.js, the click on [data-target-from]). The Þ is the pull tab: hovering, clicking or pressing
// it opens the expanded pill (view/pillPopup.js), native's `pill_expand`. Shape and sizing come
// from theme.css vars (--pill-radius etc.), which are generated from data/gui/theme.ron, the one
// source both platforms read. See docs/design/chat-layout.md and docs/design/web-native-parity.md.
//
// Until 2026-10-22 the reactions sat in their own bordered pills beside this one, and the Þ was a
// bare span a keyboard could not reach (Track W step 3).
//
// Options:
//   time, preformatted HH:MM string (use formatTimePill in app.js)
//   iso, the message's time as an ISO string, for the <time> element's datetime
//   thorn, true to render the Þ (the main channel's rows; a DM row has none yet)
//   extra, extra inline HTML inside the pill (e.g. the DM e2ee lock)
//   reactionsHtml, the row's reactions container, drawn last inside the pill (app.js)
function timestampPillHTML(opts) {
  opts = opts || {};
  const time = opts.time || '';
  const when = opts.iso ? ' datetime="' + opts.iso + '"' : '';
  const thorn = opts.thorn
    ? '<button type="button" class="ts-thorn" aria-haspopup="true" aria-expanded="false" aria-label="Reply or react" data-no-tip>Þ</button>'
    : '';
  const extra = opts.extra || '';
  return '<span class="ts-pill"><time class="ts-time"' + when + '>' + time + '</time>' + thorn + extra + (opts.reactionsHtml || '') + '</span>';
}
window.timestampPillHTML = timestampPillHTML;

// The reactions inside the pill, as native paints them: sorted by emoji, each "emoji count",
// counted without anyone `o.hidden(key)` says to leave out (someone I blocked, step C); mine
// marked (`.mine`, aria-pressed). `reactions` maps an emoji to the keys that reacted with it (a
// Set or an array). `o`: { myKey, from, ts, esc, hidden }. A reaction nobody shown is left out.
function reactionBadgesHTML(reactions, o) {
  o = o || {};
  const esc = o.esc || ((s) => String(s));
  return Object.keys(reactions || {}).sort().map((emoji) => {
    const keys = Array.from(reactions[emoji] || []);
    const shown = keys.filter((k) => !(o.hidden && o.hidden(k))).length;
    if (shown === 0) return '';
    const mine = keys.includes(o.myKey);
    return '<button type="button" class="reaction-badge' + (mine ? ' mine' : '') + '"'
      + ' data-target-from="' + esc(o.from) + '" data-target-ts="' + o.ts + '" data-emoji="' + esc(emoji) + '"'
      + ' aria-pressed="' + mine + '" aria-label="' + esc(emoji) + ' ' + shown + (mine ? ', yours' : '') + '" data-no-tip>'
      + esc(emoji) + '<span class="count">' + shown + '</span></button>';
  }).join('');
}
window.reactionBadgesHTML = reactionBadgesHTML;
