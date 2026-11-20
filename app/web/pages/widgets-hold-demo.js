// Moved out of web/pages/widgets.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // Wire the inert hold-confirm demos. window.holdConfirm / holdToConfirm come
    // from /shared/hold-confirm.js; here they fire nothing but a status note.
    (function () {
      var note = document.getElementById('wg-hold-note');
      function say(msg, ok) {
        note.textContent = msg;
        note.classList.toggle('ok', !!ok);
      }
      async function demoModal(seconds) {
        if (!window.holdConfirm) return;
        var held = await window.holdConfirm(
          'This is a demo. In the real app this is where an irreversible action would fire. Nothing happens here.',
          { seconds: seconds, confirmLabel: 'Hold to confirm (demo)' });
        if (held) say('✓ You held it to the end. In the real app, the action would now run.', true);
        else say('Cancelled. Nothing happened.', false);
      }
      document.getElementById('wg-modal-3').addEventListener('click', function () { demoModal(3); });
      document.getElementById('wg-modal-5').addEventListener('click', function () { demoModal(5); });
      if (window.holdToConfirm) {
        window.holdToConfirm(document.getElementById('wg-inline'), {
          seconds: 3,
          onConfirm: function () { say('✓ Inline hold complete (demo). Nothing happened.', true); },
        });
      }
    })();
