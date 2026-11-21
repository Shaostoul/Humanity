// Moved out of web/pages/download.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // ── SVG icon templates (viewBox 0 0 48 48, stroke-width inherited from CSS) ──
    var PLATFORM_SVGS = {
      windows: '<svg viewBox="0 0 48 48" fill="currentColor" stroke="none"><path d="M6,10 L22,7.5 V23 H6 Z M25,7 L42,4.5 V23 H25 Z M6,25.5 H22 V40.5 L6,38 Z M25,25.5 H42 V43.5 L25,41 Z"/></svg>',
      macos: '<svg viewBox="0 0 48 48" fill="currentColor" stroke="none"><path d="M33.7,6c-2.8,0.2-6.1,2-8,4.3c-1.7,2.1-3.2,5.2-2.6,8.2c3,0.1,6.2-1.7,8-4C32.8,12.4,34.1,9.3,33.7,6z M39.5,17.5c-2.3,1.3-3.8,3.8-3.8,6.5c0,4,3,7.4,6.8,7.4c-0.5,1.7-1.2,3.3-2.2,4.8c-1.5,2.2-3.1,4.4-5.5,4.4c-2.4,0-3.1-1.4-5.8-1.4c-2.7,0-3.5,1.4-5.8,1.5c-2.3,0.1-4.1-2.4-5.6-4.6c-3.1-4.5-5.4-12.6-2.3-18.1c1.6-2.7,4.3-4.5,7.3-4.5c2.3,0,4.4,1.5,5.8,1.5c1.4,0,4-1.9,6.7-1.6C36.5,13.5,38.5,15,39.5,17.5z"/></svg>',
      linux: '<svg viewBox="0 0 48 48" fill="currentColor" stroke="none"><path d="M24,4c-4.4,0-8,4.5-8,10c0,3,1,5.6,2.5,7.5C16,23.5,12,28,12,33c0,2,0.5,4,2,5.5c-1.5,0.8-2,2-2,3.5c0,1.1,2,2,5,2h14c3,0,5-0.9,5-2c0-1.5-0.5-2.7-2-3.5C35.5,37,36,35,36,33c0-5-4-9.5-6.5-11.5C31,19.6,32,17,32,14C32,8.5,28.4,4,24,4z M21,13a2,2 0 1,1 0,0.1 M27,13a2,2 0 1,1 0,0.1"/></svg>',
      android: '<svg viewBox="0 0 48 48" fill="currentColor" stroke="none"><rect x="10" y="20" width="28" height="18" rx="2"/><rect x="12" y="38" width="6" height="6" rx="1"/><rect x="30" y="38" width="6" height="6" rx="1"/><rect x="6" y="22" width="4" height="12" rx="2"/><rect x="38" y="22" width="4" height="12" rx="2"/><path d="M14,20 Q14,12 24,12 Q34,12 34,20 Z"/><circle cx="19" cy="16" r="1.5" fill="var(--bg, #1a1a1a)"/><circle cx="29" cy="16" r="1.5" fill="var(--bg, #1a1a1a)"/><line x1="17" y1="12" x2="14" y2="7" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><line x1="31" y1="12" x2="34" y2="7" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
      ios: '<svg viewBox="0 0 48 48" fill="currentColor" stroke="none"><rect x="14" y="4" width="20" height="40" rx="3" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="24" cy="39" r="2"/></svg>',
      web: '<svg viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2"><circle cx="24" cy="24" r="18"/><ellipse cx="24" cy="24" rx="7" ry="18"/><line x1="6" y1="24" x2="42" y2="24"/><path d="M9,15 Q24,18 39,15"/><path d="M9,33 Q24,30 39,33"/></svg>',
    };

    // ── Platform Detection ──
    function detectPlatform() {
      var ua = navigator.userAgent || '';
      var uaData = navigator.userAgentData;
      var os = 'unknown', arch = '', svgKey = 'web', cardId = 'card-web';

      if (uaData) {
        var p = (uaData.platform || '').toLowerCase();
        if (p === 'windows') { os = 'windows'; svgKey = 'windows'; cardId = 'card-windows'; }
        else if (p === 'macos') { os = 'macos'; svgKey = 'macos'; cardId = 'card-macos-arm'; }
        else if (p === 'linux') { os = 'linux'; svgKey = 'linux'; cardId = 'card-linux'; }
        else if (p === 'android') { os = 'android'; svgKey = 'android'; cardId = 'card-android'; }
        else if (p === 'ios') { os = 'ios'; svgKey = 'ios'; cardId = 'card-ios'; }

        if (uaData.getHighEntropyValues) {
          uaData.getHighEntropyValues(['architecture', 'bitness'])
            .then(function(h) {
              arch = h.architecture || '';
              if (os === 'macos') cardId = (arch === 'arm' || arch === 'arm64') ? 'card-macos-arm' : 'card-macos-intel';
              updateUI(os, arch, svgKey, cardId);
            })
            .catch(function() { updateUI(os, arch, svgKey, cardId); });
          return;
        }
      }

      if (/Windows/.test(ua)) { os = 'windows'; svgKey = 'windows'; cardId = 'card-windows'; }
      else if (/Macintosh/.test(ua)) {
        os = 'macos'; svgKey = 'macos';
        cardId = /ARM64|arm64/.test(ua) ? 'card-macos-arm' : (/Intel/.test(ua) ? 'card-macos-intel' : 'card-macos-arm');
      }
      else if (/Android/.test(ua)) { os = 'android'; svgKey = 'android'; cardId = 'card-android'; }
      else if (/iPhone|iPad|iPod/.test(ua)) { os = 'ios'; svgKey = 'ios'; cardId = 'card-ios'; }
      else if (/Linux/.test(ua)) { os = 'linux'; svgKey = 'linux'; cardId = 'card-linux'; }

      if (/arm64|aarch64/i.test(ua)) arch = 'arm64';
      else if (/x86_64|x64|Win64|WOW64|amd64/i.test(ua) || /Intel Mac/.test(ua)) arch = 'x64';

      updateUI(os, arch, svgKey, cardId);
    }

    /** The platform updateUI last showed ('android' moves the main button to the phone app). */
    var DETECTED_OS = null;
    function updateUI(os, arch, svgKey, cardId) {
      DETECTED_OS = os;
      var names = { windows: 'Windows', macos: 'macOS', linux: 'Linux', android: 'Android', ios: 'iOS', unknown: 'your device' };
      var archLabel = arch === 'arm64' || arch === 'arm' ? 'ARM64' : arch === 'x64' ? 'x64' : '';
      var detail = (names[os] || 'Unknown') + (archLabel ? ' · ' + archLabel : '');

      document.getElementById('detected-icon').innerHTML = PLATFORM_SVGS[svgKey] || PLATFORM_SVGS.web;
      document.getElementById('detected-name').textContent = 'Detected: ' + (names[os] || 'Unknown');
      document.getElementById('detected-detail').textContent = detail;

      document.querySelectorAll('.platform-card').forEach(function(c) { c.classList.remove('detected'); });
      var card = document.getElementById(cardId);
      if (card) card.classList.add('detected');

      var btn = document.getElementById('detected-download');
      var sub = document.getElementById('detected-sub');

      if (['windows', 'macos', 'linux'].includes(os)) {
        btn.textContent = 'Download for ' + names[os];
        btn.href = 'https://github.com/Shaostoul/Humanity/releases/tag/v0.1568.0';
        sub.textContent = 'Downloads from GitHub Releases · v0.1568.0';
        // The installer's own address, from this server's release list, so clicks download the file
        fetchDirectDownload(os, arch, btn, sub);
      } else if (['android', 'ios'].includes(os)) {
        btn.textContent = 'Install as PWA';
        btn.href = '/chat';
        sub.textContent = 'Open in browser, then install to homescreen';
      } else {
        btn.textContent = 'Open Web App';
        btn.href = '/chat';
        sub.textContent = 'Works in any modern browser, no download needed';
      }
    }

    /** The newest release this server holds, from its own list (written by
     *  scripts/vps/regen-releases-manifest when a release lands): { tag, assets: [{ name, size }] },
     *  or null. Never GitHub's API, which sent every visitor's address to GitHub (BUG-281,
     *  2026-11-07); GitHub sees a visitor only when they press a download button. */
    function newestRelease() {
      return fetch('/releases/manifest.json')
        .then(function(r) { return r.json(); })
        .then(function(m) {
          var rel = m && m.releases && m.releases[0];
          return rel && rel.tag ? { tag: rel.tag, assets: rel.assets || [] } : null;
        });
    }

    /** A release file's own address on GitHub Releases, where the downloads are. */
    function releaseFileUrl(tag, name) {
      return 'https://github.com/Shaostoul/Humanity/releases/download/' + encodeURIComponent(tag) + '/' + encodeURIComponent(name);
    }

    /** Point the button at the installer for this platform, from this server's release list. */
    function fetchDirectDownload(os, arch, btn, sub) {
      newestRelease()
        .then(function(d) {
          if (!d) return;
          var ver = d.tag.replace(/^v/, '');
          // Match the right asset for the detected platform.
          // Ordered preference per platform: every platform prefers the .zip,
          // because the zip is the FULL game (program + models + textures +
          // data) and it opens natively in Explorer, Finder and Linux file
          // managers. The bare Windows .exe is only the program: run on its
          // own it draws machines as plain boxes on flat ground, so it is a
          // fallback for old releases that shipped no zip, never the first
          // pick. (The in-app update check further down still takes the .exe
          // on purpose: it updates an install that already has the files.)
          // mac/linux fall back to .tar.gz on releases from before zips shipped.
          var patterns = {
            windows: [/windows-x64\.zip$/i, /windows-x64\.exe$/i],
            macos:   arch === 'arm64'
                       ? [/macos-arm64\.zip$/i, /macos-arm64\.tar\.gz$/i]
                       : [/macos-x64\.zip$/i, /macos-x64\.tar\.gz$/i],
            linux:   [/linux-x64\.zip$/i, /linux-x64\.tar\.gz$/i],
          };
          var pats = patterns[os];
          if (!pats) return;
          var asset = null;
          for (var pi = 0; pi < pats.length && !asset; pi++) {
            for (var i = 0; i < d.assets.length; i++) {
              if (pats[pi].test(d.assets[i].name)) { asset = d.assets[i]; break; }
            }
          }
          if (asset) {
            btn.href = releaseFileUrl(d.tag, asset.name);
            // A release file's own address is a direct download link: the browser downloads it.
            // Name the size when it is the full game, so a ~380 MB download
            // is not a surprise. (A bare exe fallback is not the full game.)
            var full = /\.(zip|tar\.gz)$/i.test(asset.name) && asset.size;
            sub.textContent = (full ? 'Full game, ' + Math.round(asset.size / 1e6) + ' MB · ' : '') +
              'Downloads from GitHub Releases · v' + ver;
          } else {
            // No matching asset, fall back to releases page
            sub.textContent = 'Downloads from GitHub Releases · v' + ver;
          }
        })
        .catch(function() { /* use fallback link to releases page */ });
    }

    // ── The phone app (Android rung A2, docs/design/platforms.md) ──
    // Its card shows the download and the install steps only when this server's newest release
    // carries the APK; otherwise the card says the phone app is not published yet (the page's
    // default, so a failed fetch says the same). The words are the page's own, by key
    // (data-i18n); this only picks which are shown, and the main button borrows them.
    var ANDROID_APK = 'HumanityOS-android-arm64.apk';

    /** The release's APK, by its exact name (the workflow publishes only one signed with the
     *  project's key, never a test-signed one), or null. */
    function androidApk(assets) {
      for (var i = 0; i < (assets || []).length; i++) {
        if (assets[i] && assets[i].name === ANDROID_APK) return assets[i];
      }
      return null;
    }

    /** Show the Android card's state for release `rel` ({ tag, assets } or null); the APK's
     *  download address when there is one, else null. */
    function showAndroid(rel) {
      var apk = rel ? androidApk(rel.assets) : null;
      function show(id, on) {
        var el = document.getElementById(id);
        if (el) el.hidden = !on;
      }
      ['android-arch-none', 'android-detail-none', 'android-use-web'].forEach(function(id) { show(id, !apk); });
      ['android-arch-apk', 'android-detail-apk', 'android-apk-link', 'android-steps-link', 'android-install'].forEach(function(id) { show(id, !!apk); });
      if (!apk) return null;
      var url = releaseFileUrl(rel.tag, apk.name);
      var link = document.getElementById('android-apk-link');
      if (link) link.href = url;
      return url;
    }

    detectPlatform();

    newestRelease()
      .then(function(d) {
        var url = showAndroid(d);
        if (!url || DETECTED_OS !== 'android') return;
        // On an Android phone the main button is the phone app's, in the card's own words.
        var btn = document.getElementById('detected-download');
        var sub = document.getElementById('detected-sub');
        var words = document.getElementById('android-apk-link');
        var then = document.getElementById('android-get-sub');
        if (btn && words) { btn.textContent = words.textContent; btn.href = url; }
        if (sub && then) sub.textContent = then.textContent;
      })
      .catch(function() { /* the card keeps saying the phone app is not published yet */ });

    // ── Native desktop app: check for updates against this server's release list ──
    (function desktopUpdateCheck() {
      if (!window.__HOS_DESKTOP__) return;

      var btn = document.getElementById('detected-download');
      var sub = document.getElementById('detected-sub');
      if (!btn) return;

      var appVer = window.__HOS_APP_VERSION || '0.1568.0';
      btn.textContent = 'Checking for updates...';
      btn.style.opacity = '0.7';
      if (sub) sub.textContent = 'Currently running v' + appVer;

      // The newest release this server holds (its own list, never GitHub's API: BUG-281).
      newestRelease()
        .then(function(d) {
          if (!d) throw new Error('No release data');
          var latestVer = d.tag.replace(/^v/, '');

          // Find the Windows installer asset
          var asset = null;
          for (var i = 0; i < d.assets.length; i++) {
            if (/\.exe$/i.test(d.assets[i].name)) { asset = d.assets[i]; break; }
          }

          // Compare versions
          function versionNewer(a, b) {
            var pa = a.split('.').map(Number), pb = b.split('.').map(Number);
            for (var i = 0; i < 3; i++) { if ((pa[i]||0) > (pb[i]||0)) return true; if ((pa[i]||0) < (pb[i]||0)) return false; }
            return false;
          }

          if (versionNewer(latestVer, appVer) && asset) {
            // Newer version available, link to download
            btn.textContent = 'Download v' + latestVer;
            btn.style.opacity = '1';
            btn.style.pointerEvents = 'auto';
            btn.href = releaseFileUrl(d.tag, asset.name);
            if (sub) sub.textContent = 'v' + latestVer + ' available · You have v' + appVer;
          } else {
            // Up to date
            btn.textContent = "You're up to date! (v" + appVer + ')';
            btn.style.opacity = '0.8';
            btn.style.background = 'var(--success, #2a6)';
            btn.style.pointerEvents = 'none';
            if (sub) sub.textContent = 'Running the latest version';
          }

          btn.href = btn.href || '#';
          btn.removeAttribute('target');
        })
        .catch(function(err) {
          // Can't check, show manual download link
          btn.textContent = 'Download Latest';
          btn.style.opacity = '1';
          btn.style.pointerEvents = 'auto';
          btn.href = 'https://github.com/Shaostoul/Humanity/releases/latest';
          if (sub) sub.textContent = 'Currently running v' + appVer + ' · Could not check for updates';
        });
    })();

    // ── The newest version, from this server's release list (web only; BUG-281) ──
    if (!window.__HOS_DESKTOP__) {
      newestRelease()
        .then(function(d) {
          if (d) {
            var v = d.tag.replace(/^v/, '');
            document.getElementById('version-badge').textContent = 'Desktop: v' + v + ' · Web: always latest';
          }
        })
        .catch(function() { /* use fallback version */ });
    }
