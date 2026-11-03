/**
 * Links to chat channels on the website (2026-11-03). The operator: "Can we make it so I can link
 * people to specific channels like https://united-humanity.us/chat#global or
 * https://united-humanity.us/chat#fire-disciples"
 *
 * A channel's address is `https://<its server>/chat#<its id>`. These are the rules the chat
 * (web/chat/app.js: the address follows the channel open, a link opens its channel, Copy link) and
 * the read view for a visitor (web/chat/read.js) both follow: which word a hash names, which
 * channel a word names (its id first, then its shown name, either in any case: an admin can rename
 * what a channel shows), a channel's address, and whether a server's address reaches anyone off
 * this computer or its network. A direct message is never in the address (docs/design/navigation.md
 * section 10.1); `#dm=<key>` stays the explicit conversation link it was.
 *
 * The desktop's twin is src/gui/pages/chat/channel_link.rs. Both are held to the same answers by
 * scripts/tests/fixtures/chat-links-v1.json (scripts/tests/chat-links-web.test.js and the Rust
 * tests beside channel_link.rs).
 *
 * Loaded by /chat (before app.js) and /chat/read. window.HosChatLinks; module.exports for node.
 */
(function () {
  'use strict';

  /** The longest word a link may name: a channel's id is at most 24 characters, its shown name a
   *  little more. Anything longer is no channel. */
  const MAX_WORD = 64;
  /** Hashes the chat reads as something else: the Account button's `#profile`, and `#joined`, the
   *  welcome step's way back (web/chat/index.html). */
  const NOT_CHANNELS = new Set(['profile', 'joined']);

  /** The channel word an address's hash names ('#fire-disciples' gives 'fire-disciples'), or ''
   *  when it names none: no hash, an empty one, `#profile`, `#joined`, or anything holding '='
   *  (`#dm=<key>`, a device link). Percent-decoded and trimmed. */
  function wordOf(hash) {
    let h = String(hash == null ? '' : hash);
    if (h.charAt(0) === '#') h = h.slice(1);
    if (!h || h.indexOf('=') !== -1) return '';
    let w = '';
    try { w = decodeURIComponent(h); } catch (e) { return ''; }
    w = w.trim();
    if (!w || w.length > MAX_WORD || NOT_CHANNELS.has(w.toLowerCase())) return '';
    return w;
  }

  /** The channel of `channels` (each `{id, name}`) that `word` names: the one whose id it is, else
   *  the one whose shown name it is, each ignoring case; null when none does. */
  function find(word, channels) {
    const w = String(word == null ? '' : word).trim().toLowerCase();
    if (!w) return null;
    const list = Array.isArray(channels) ? channels.filter((c) => c && typeof c === 'object') : [];
    return list.find((c) => String(c.id).toLowerCase() === w)
      || list.find((c) => String(c.name == null ? '' : c.name).trim().toLowerCase() === w)
      || null;
  }

  /** The hash that shows channel `id`: '#' and the id, written so an address can carry it. */
  function hashOf(id) {
    return '#' + encodeURIComponent(String(id));
  }

  /** Channel `id`'s address on the server at `origin` ('https://united-humanity.us'). */
  function link(origin, id) {
    return String(origin == null ? '' : origin).replace(/\/+$/, '') + '/chat' + hashOf(id);
  }

  /** Whether `host` (a host name or address, a port allowed) can be reached by someone on another
   *  network: not this computer (localhost, 127.x, ::1) and not a home or office network (10.x,
   *  172.16 to 31.x, 192.168.x, 169.254.x, 100.64 to 127.x, fc00::/7, fe80::/10, a name ending
   *  .local, .lan, .internal, .home.arpa, .localhost or .test, or a bare machine name). A link to a
   *  server that is not public opens only for people on that computer or that network. */
  function isPublicHost(host) {
    let h = String(host == null ? '' : host).trim().toLowerCase();
    if (h.charAt(0) === '[') {
      const end = h.indexOf(']');
      h = end > 0 ? h.slice(1, end) : h.slice(1);
    } else if ((h.match(/:/g) || []).length === 1) {
      h = h.slice(0, h.indexOf(':'));
    }
    if (h.endsWith('.')) h = h.slice(0, -1);
    if (!h || h === 'localhost' || h === '0.0.0.0' || h === '::' || h === '::1') return false;
    for (const end of ['.localhost', '.local', '.lan', '.internal', '.home.arpa', '.test']) {
      if (h.endsWith(end)) return false;
    }
    const v4 = h.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
    if (v4) {
      const a = Number(v4[1]);
      const b = Number(v4[2]);
      if (a === 0 || a === 10 || a === 127) return false;
      if (a === 169 && b === 254) return false;
      if (a === 172 && b >= 16 && b <= 31) return false;
      if (a === 192 && b === 168) return false;
      if (a === 100 && b >= 64 && b <= 127) return false;
      return true;
    }
    if (h.indexOf(':') !== -1) return !/^f[cd]/.test(h) && !/^fe[89ab]/.test(h);
    return h.indexOf('.') !== -1;
  }

  /** What a link to a server's chat asks for: `{origin, channel}` for `/chat#<channel>` (or the
   *  read view's `/chat/read#<channel>`), `{origin,
   *  dm}` for `/chat#dm=<public key>`, or null for any other address (another page, no hash, a hash
   *  naming no channel, a scheme other than http and https). `origin` is the scheme and host (and
   *  a port that is not the scheme's own), lower case: 'https://united-humanity.us'. */
  function parse(url) {
    let u = null;
    try { u = new URL(String(url)); } catch (e) { return null; }
    if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
    const path = u.pathname.replace(/\/+$/, '');
    // The chat, and the read view a visitor is sent to (/chat/read#<channel>), name the same channel.
    if (path !== '/chat' && path !== '/chat/index.html' && path !== '/chat/read') return null;
    const origin = u.protocol + '//' + u.host.toLowerCase();
    const dm = u.hash.match(/^#dm=([0-9a-fA-F]{16,8192})$/);
    if (dm) return { origin, dm: dm[1] };
    const channel = wordOf(u.hash);
    return channel ? { origin, channel } : null;
  }

  const api = { MAX_WORD, wordOf, find, hashOf, link, isPublicHost, parse };
  if (typeof window !== 'undefined') window.HosChatLinks = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
