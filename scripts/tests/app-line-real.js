// The REAL app's-voice functions of web/chat/app.js, as source to run in a test's node:vm page that
// does not load the whole of app.js (owner-chat-web, claim-typed-web): `appLine` (the one way to the
// app's mark, the owner branch's eighth review, 2026-11-22), `appArg`, `appArgText`, `drawNotice`,
// `shownName`, `serverWords` and `formatCountdown`, cut out of app.js itself so a test never holds a
// copy that could drift from what the page runs. The page needs `document`, `appendMessage`,
// `i18n` and (for shownName) `window.HosKeyCard`.
'use strict';
const fs = require('fs');
const path = require('path');

/** The source of `function name(...) {...}` (or `const name = ...;` up to its closing `})();`). */
function cut(src, name) {
  let start = src.search(new RegExp('(^|\\n)(async )?function ' + name + '\\('));
  let isConst = false;
  if (start < 0) {
    start = src.search(new RegExp('(^|\\n)const ' + name + ' = '));
    isConst = true;
  }
  if (start < 0) throw new Error('app.js has no ' + name);
  if (src[start] === '\n') start += 1;
  if (isConst) {
    const end = src.indexOf('\n})();', start);
    if (end < 0) throw new Error('unclosed ' + name);
    return src.slice(start, end + '\n})();'.length);
  }
  let depth = 0;
  for (let i = src.indexOf('{', start); i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}' && --depth === 0) return src.slice(start, i + 1);
  }
  throw new Error('unclosed ' + name);
}

/** The functions, as one script. `webDir` defaults to the repo's web/ (HOS_WEB_DIR when set). */
function appVoiceSource(webDir) {
  const web = webDir || process.env.HOS_WEB_DIR || path.join(__dirname, '..', '..', 'web');
  const src = fs.readFileSync(path.join(web, 'chat', 'app.js'), 'utf8').replace(/\r\n/g, '\n');
  return ['appLine', 'appArg', 'appArgText', 'drawNotice', 'shownName', 'serverWords', 'formatCountdown'].map((n) => cut(src, n)).join('\n');
}

module.exports = { appVoiceSource };
