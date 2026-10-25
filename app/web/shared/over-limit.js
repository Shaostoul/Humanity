// Characters past a field's limit, shown as they are typed: the part within the limit as usual,
// the rest in --danger on --danger-bg. The website's half of the desktop app's
// src/gui/widgets/over_limit.rs (operator, 2026-10-24: "highlight the character spots that go
// beyond the character limit have their background highlight dark red and the text change from
// white to red. We need to improve the visual ques for some errors."). Used by the chat's message
// box (chat/chat-ui.js) and the profile page's bio (pages/profile.html); its styles are
// shared/over-limit.css.
//
// A text box draws all its words in one colour, so while the text is too long a mirror layer
// behind the box draws them instead. The box's own words go transparent meanwhile (its caret
// stays), the mirror takes the box's background, and the two scroll together. Counted in
// characters, as the servers count them (an emoji is one), never in JavaScript's UTF-16 units.
(function () {
  'use strict';

  var COPIED = ['font', 'letterSpacing', 'lineHeight', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
    'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'borderRadius', 'textIndent', 'tabSize'];

  function mirrorOf(input) {
    var parent = input.parentElement;
    var m = parent.querySelector(':scope > .over-limit-mirror');
    if (!m) {
      m = document.createElement('div');
      m.className = 'over-limit-mirror';
      m.setAttribute('aria-hidden', 'true');
      parent.style.position = parent.style.position || 'relative';
      parent.insertBefore(m, input);
      input.addEventListener('scroll', function () { m.scrollTop = input.scrollTop; });
    }
    return m;
  }

  // Shows (or hides) the characters of `input` past `limit`. Returns whether it is over.
  function hosOverLimit(input, limit) {
    if (!input || !input.parentElement) return false;
    var chars = Array.from(input.value);
    var over = chars.length > limit;
    var existing = input.parentElement.querySelector(':scope > .over-limit-mirror');
    if (!over) {
      input.classList.remove('over-limit-typing');
      if (existing) existing.style.display = 'none';
      return false;
    }
    var m = mirrorOf(input);
    // The box's own background, read before it goes transparent, so the field keeps its look.
    if (!input.classList.contains('over-limit-typing')) m.style.backgroundColor = getComputedStyle(input).backgroundColor;
    input.classList.add('over-limit-typing');
    var cs = getComputedStyle(input);
    COPIED.forEach(function (p) { m.style[p] = cs[p]; });
    m.style.left = input.offsetLeft + 'px';
    m.style.top = input.offsetTop + 'px';
    m.style.width = input.offsetWidth + 'px';
    m.style.height = input.offsetHeight + 'px';
    m.style.display = 'block';
    m.textContent = '';
    m.appendChild(document.createTextNode(chars.slice(0, limit).join('')));
    var mark = document.createElement('mark');
    mark.className = 'over-limit-chars';
    mark.textContent = chars.slice(limit).join('');
    m.appendChild(mark);
    // A trailing line break, so a final empty line in the box has its height in the mirror too.
    m.appendChild(document.createTextNode(String.fromCharCode(10)));
    m.scrollTop = input.scrollTop;
    return true;
  }

  window.hosOverLimit = hosOverLimit;
})();
