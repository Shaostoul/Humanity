// view/messageRow.js, clean view module for one chat message row.
//
// Mirrors native's message_row (src/gui/widgets/row.rs): a [avatar gutter |
// content column] row. Avatar + name show once per sender group; continuation
// rows drop them and show just the timestamp pill. Avatar size/gap come from
// theme.css vars (--avatar-size/--avatar-gap), generated from data/gui/theme.ron
//, one source for both platforms. See docs/design/chat-layout.md.
//
// The engine (app.js addChatMessage / chat-dms.js addDmMessage) builds the data
// + sub-HTML parts and wires event listeners on the returned element afterward;
// this module ONLY assembles the row structure, so the layout that must match
// native has exactly one home. Output is intentionally identical to the markup
// the two builders used to inline.
//
// parts (all optional except where noted):
//   isContinuation, true drops the avatar + name (grouped continuation row)
//   identiconHtml, gutter avatar HTML (shown on header rows)
//   metaHtml, the <div class="meta"> name + badges block (header rows)
//   pillHtml, timestampPillHTML(...) output
//   reactionsHtml, the <span class="reactions" ...> container (main channel)
//   bodyHtml, formatted message body
//   replyIndicatorHtml, threadBadgeHtml, actionsHtml, optional extras
function messageRowHTML(p) {
  p = p || {};
  const gutter = p.isContinuation ? '' : (p.identiconHtml || '');
  const meta = p.isContinuation ? '' : (p.metaHtml || '');
  return `
    <div class="msg-gutter">${gutter}</div>
    <div class="msg-main">
      ${p.replyIndicatorHtml || ''}
      ${meta}
      ${p.pillHtml || ''}
      ${p.reactionsHtml || ''}
      <div class="body">${p.bodyHtml || ''}</div>
      ${p.threadBadgeHtml || ''}
    </div>
    ${p.actionsHtml || ''}
  `;
}
window.messageRowHTML = messageRowHTML;
