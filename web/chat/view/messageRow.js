// view/messageRow.js, clean view module for one chat message row.
//
// Mirrors native's message_row (src/gui/widgets/row.rs): a [avatar gutter |
// content column] row. Avatar + name show once per sender group; continuation
// rows drop them and show just the timestamp pill. Avatar size/gap come from
// theme.css vars (--avatar-size/--avatar-gap), generated from data/gui/theme.ron
//, one source for both platforms. See docs/design/chat-layout.md.
//
// A reply's "↩ Name: words" line runs across the top of the row, above the
// avatar, the way native draws it as its own short row above the message
// (chat.rs, "Reply-to context"); it is indented to the content column.
//
// The engine (app.js addChatMessage / chat-dms.js addDmMessage) builds the data
// + sub-HTML parts and wires event listeners on the returned element afterward;
// this module ONLY assembles the row structure, so the layout that must match
// native has exactly one home.
//
// parts (all optional except where noted):
//   isContinuation, true drops the avatar + name (grouped continuation row)
//   identiconHtml, gutter avatar HTML (shown on header rows)
//   metaHtml, the <div class="meta"> name + badges block (header rows)
//   pillHtml, timestampPillHTML(...) output (the reactions ride inside it)
//   bodyHtml, formatted message body
//   replyIndicatorHtml, threadBadgeHtml, actionsHtml, optional extras
function messageRowHTML(p) {
  p = p || {};
  const gutter = p.isContinuation ? '' : (p.identiconHtml || '');
  const meta = p.isContinuation ? '' : (p.metaHtml || '');
  return `
    ${p.replyIndicatorHtml || ''}
    <div class="msg-gutter">${gutter}</div>
    <div class="msg-main">
      ${meta}
      ${p.pillHtml || ''}
      <div class="body">${p.bodyHtml || ''}</div>
      ${p.threadBadgeHtml || ''}
    </div>
    ${p.actionsHtml || ''}
  `;
}
window.messageRowHTML = messageRowHTML;
