// Chat view — shared structural constants.
//
// Mirrored from the native app's `src/gui/widgets/row.rs` so the web view and
// the native view can't drift on the numbers that define the layout. Native
// is canonical: if you change one of these, change the matching const in
// row.rs (and vice-versa). See docs/design/chat-layout.md.
//
// Everything VISUAL (colors, spacing scale, radii, fonts) comes from theme
// tokens in theme.css (generated from data/gui/theme.ron) — NOT here. This
// file is only for the few structural numbers native hardcodes in layout
// logic that the DOM needs to match.
window.CHAT_LAYOUT = Object.freeze({
  AVATAR_SIZE: 32,                 // row.rs USERBOX_SIZE — gutter avatar square
  AVATAR_GAP: 8,                   // row.rs USERBOX_GAP — gutter→content gap
  PILL_RADIUS: 9,                  // paint_timestamp_pill rounding
  GROUP_WINDOW_MS: 5 * 60 * 1000,  // sender-grouping window (collapse same-sender)
});
