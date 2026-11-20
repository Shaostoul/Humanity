// Moved out of web/chat/index.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    (function preflightIdentity() {
      // If we already came back from the landing page (hash flag) skip
      // the redirect, the user just signed up and we don't want to
      // bounce them back.
      if (location.hash === '#joined') {
        history.replaceState(null, '', location.pathname);
        return;
      }
      // Opened from a scanned device-link QR (fragment URL): stay here so the
      // chat app can import that identity, even on a fresh device with no key
      // yet. Do NOT bounce to the welcome page (it would drop the fragment).
      if (location.hash.indexOf('devicelink=') !== -1) return;
      try {
        var hasBackup  = !!localStorage.getItem('humanity_key_backup');
        var hasLegacy  = !!localStorage.getItem('humanity_key');
        var hasWrapped = !!localStorage.getItem('humanity_key_wrapped');
        if (hasBackup || hasLegacy || hasWrapped) return;
        // A visitor with no key reads the channels anyone may read first (2026-11-02,
        // docs/design/chat-access.md section 7): /chat/read, whose one button, Choose a name
        // to join in, opens the welcome step and comes back here. A link to a channel keeps its
        // channel (2026-11-03): /chat#fire-disciples reads /chat/read#fire-disciples. A hash holding
        // '=' is no channel (#dm=<key>, a device link) and is not carried.
        var hash = location.hash && location.hash.indexOf('=') === -1 ? location.hash : '';
        location.replace('/chat/read' + hash);
      } catch (e) {
        // localStorage blocked, let chat handle it, no redirect.
      }
    })();
