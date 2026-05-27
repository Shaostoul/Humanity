// view/timestampPill.js — clean view module for the message timestamp pill.
//
// Mirrors native's paint_timestamp_pill (src/gui/pages/chat.rs): a compact
// "HH:MM Þ" pill. Shape/sizing come from theme.css vars (--pill-radius etc.),
// which are generated from data/gui/theme.ron — the one source both platforms
// read. First module of the clean web-view rebuild; see docs/design/chat-layout.md.
//
// Options:
//   time  — preformatted HH:MM string (use formatTimePill in app.js)
//   thorn — true to render the Þ add-reaction affordance (main channel)
//   extra — extra inline HTML inside the pill (e.g. the DM e2ee lock)
function timestampPillHTML(opts) {
  opts = opts || {};
  const time = opts.time || '';
  const thorn = opts.thorn ? '<span class="ts-thorn" title="React">Þ</span>' : '';
  const extra = opts.extra || '';
  return '<span class="ts-pill"><span class="ts-time">' + time + '</span>' + thorn + extra + '</span>';
}
window.timestampPillHTML = timestampPillHTML;
