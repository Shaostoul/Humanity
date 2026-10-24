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

// The time beside a message, written as the desktop app writes it (src/gui/pages/chat/timestamps.rs
// `format_timestamp` and `format_full_timestamp`), 2026-10-24. Settings > Chat > Timestamp format
// and Time zone (`timestamp_format`, `timestamp_zone` in humanity_settings; the defaults are in
// shared/defaults.js), this computer's time by default. In UTC a time says so (" UTC"). Until then
// the pill was always this computer's time and its expanded form always UTC.
function timestampSettings() {
  const d = window.HOS_DEFAULTS || {};
  let s = {};
  try { s = JSON.parse(window.localStorage.getItem(window.HOS_STORAGE_KEY || 'humanity_settings')) || {}; } catch (e) { s = {}; }
  return { format: s.timestamp_format || d.timestamp_format || 'hour_min', zone: s.timestamp_zone || d.timestamp_zone || 'local' };
}

// The clock reading for `ms` in `zone` ('utc', or this computer's time), and the offset east of
// UTC in minutes (null in UTC).
function stampParts(ms, zone) {
  const t = new Date(Number(ms));
  const u = zone === 'utc';
  return {
    y: u ? t.getUTCFullYear() : t.getFullYear(), mo: (u ? t.getUTCMonth() : t.getMonth()) + 1,
    d: u ? t.getUTCDate() : t.getDate(), h: u ? t.getUTCHours() : t.getHours(),
    mi: u ? t.getUTCMinutes() : t.getMinutes(), s: u ? t.getUTCSeconds() : t.getSeconds(),
    ms: u ? t.getUTCMilliseconds() : t.getMilliseconds(), off: u ? null : -t.getTimezoneOffset(),
  };
}
function stampPad(n, w) { return String(n).padStart(w || 2, '0'); }

// `format`: hour_min (17:42), hour_min_sec, date_hour_min, date_hour_min_sec, full (with ms).
function formatStamp(ms, format, zone) {
  const p = stampParts(ms, zone);
  const date = stampPad(p.y, 4) + '-' + stampPad(p.mo) + '-' + stampPad(p.d);
  const hm = stampPad(p.h) + ':' + stampPad(p.mi);
  const tail = p.off === null ? ' UTC' : '';
  switch (format) {
    case 'hour_min_sec': return hm + ':' + stampPad(p.s) + tail;
    case 'date_hour_min': return date + ' ' + hm + tail;
    case 'date_hour_min_sec': return date + ' ' + hm + ':' + stampPad(p.s) + tail;
    case 'full': return date + ' ' + hm + ':' + stampPad(p.s) + '.' + stampPad(p.ms, 3) + tail;
    default: return hm + tail;
  }
}

// Always the full form, whatever the display format: "2026-10-24 12:48:24 (UTC-07:00)" in this
// computer's time, "2026-10-24 19:48:24 UTC" in UTC.
function formatFullStamp(ms, zone) {
  const p = stampParts(ms, zone);
  const base = stampPad(p.y, 4) + '-' + stampPad(p.mo) + '-' + stampPad(p.d) + ' ' + stampPad(p.h) + ':' + stampPad(p.mi) + ':' + stampPad(p.s);
  if (p.off === null) return base + ' UTC';
  const a = Math.abs(p.off);
  return base + ' (UTC' + (p.off < 0 ? '-' : '+') + stampPad(Math.floor(a / 60)) + ':' + stampPad(a % 60) + ')';
}
window.timestampSettings = timestampSettings;
window.formatStamp = formatStamp;
window.formatFullStamp = formatFullStamp;

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
