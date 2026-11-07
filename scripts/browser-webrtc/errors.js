// Loaded FIRST by harness.html, so an error while the website's scripts load (a stand-in that
// collides with one of their names, a script that throws at its top) is kept, and harness.js sends
// every one to the test once the socket is open. The test fails on any of them: a page that did not
// load as the real chat page loads proves nothing about it.
(function () {
  'use strict';
  const kept = [];
  let sink = null;
  window.harnessError = function (text) {
    if (sink) sink(String(text));
    else kept.push(String(text));
  };
  /** harness.js hands over its sender; what was kept goes first. */
  window.harnessErrorSink = function (send) {
    sink = send;
    for (const t of kept.splice(0)) send(t);
  };
  window.addEventListener('error', (ev) => {
    window.harnessError(`${ev.message || ev.error || 'error'} (${ev.filename || ''}:${ev.lineno || 0})`);
  });
  window.addEventListener('unhandledrejection', (ev) => {
    const r = ev.reason;
    window.harnessError('unhandled rejection: ' + String((r && (r.stack || r.message)) || r));
  });
})();
