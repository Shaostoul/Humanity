/**
 * HumanityOS Shared Icon System
 *
 * WHY: Consistent SVG icons everywhere, with user-adjustable stroke weight.
 *      No more emoji rendering inconsistencies across platforms.
 *
 * USAGE:
 *   <script src="/shared/icons.js"></script>
 *   hosIcon('chat')              → returns SVG string at default size (20px)
 *   hosIcon('chat', 24)          → returns SVG string at 24px
 *   hosIcon('lock', 16, '#f80')  → 16px, custom color override
 *
 * User preference stored in localStorage as 'hos_icon_weight' (default: 3).
 * Weight range: 1–6, applied as stroke-width on all icons.
 *
 * Icons use viewBox="0 0 48 48" for easy authoring. Stroke-width is set
 * via CSS custom property --icon-weight so all icons update in real-time.
 */
(function() {
  // ── Read user preference ──
  var defaultWeight = 3;
  var stored = localStorage.getItem('hos_icon_weight');
  var weight = stored ? parseFloat(stored) : defaultWeight;
  if (isNaN(weight) || weight < 1 || weight > 6) weight = defaultWeight;

  // Apply as CSS custom property on :root so all icons inherit
  document.documentElement.style.setProperty('--icon-weight', weight);

  // ── Icon paths (viewBox 0 0 48 48) ──
  // Each value is the SVG inner content (no <svg> wrapper).
  var PATHS = {
    // ── Navigation / Core ──
    network: '<path d="M6,6 H36 V30 H18 L12,36 V30 H6 Z"/><line x1="12" y1="14" x2="30" y2="14"/><line x1="12" y1="22" x2="24" y2="22"/><path d="M36,14 H42 V36 H36 V42 L30,36 H24"/>',
    chat: '<path d="M6,6 H42 V32 H18 L12,38 V32 H6 Z"/><line x1="14" y1="16" x2="34" y2="16"/><line x1="14" y1="24" x2="28" y2="24"/>',
    profile: '<circle cx="24" cy="16" r="10"/><path d="M6,44 A18,18 0 0 1 42,44"/>',
    user: '<circle cx="24" cy="16" r="10"/><path d="M6,44 A18,18 0 0 1 42,44"/>',
    users: '<circle cx="18" cy="16" r="8"/><path d="M4,42 A14,14 0 0 1 32,42"/><circle cx="34" cy="18" r="6"/><path d="M32,42 A10,10 0 0 1 44,42"/>',
    home: '<polyline points="6,24 24,6 42,24"/><polyline points="10,22 10,42 38,42 38,22"/><rect x="18" y="28" width="12" height="14" rx="1"/>',
    inventory: '<rect x="10" y="16" width="28" height="26" rx="3"/><path d="M16,16 V10 A8,8 0 0 1 32,10 V16"/><rect x="16" y="24" width="16" height="8" rx="1"/><line x1="24" y1="26" x2="24" y2="30"/>',
    tasklist: '<rect x="8" y="6" width="32" height="38" rx="3"/><path d="M18,6 V4 A6,6 0 0 1 30,4 V6"/><polyline points="14,18 17,21 22,16"/><line x1="26" y1="19" x2="36" y2="19"/><polyline points="14,28 17,31 22,26"/><line x1="26" y1="29" x2="36" y2="29"/><line x1="14" y1="38" x2="22" y2="38"/><line x1="26" y1="38" x2="34" y2="38"/>',
    calendar: '<rect x="6" y="8" width="36" height="36" rx="3"/><line x1="6" y1="18" x2="42" y2="18"/><line x1="16" y1="8" x2="16" y2="4"/><line x1="32" y1="8" x2="32" y2="4"/><line x1="18" y1="18" x2="18" y2="44"/><line x1="30" y1="18" x2="30" y2="44"/><line x1="6" y1="28" x2="42" y2="28"/><line x1="6" y1="36" x2="42" y2="36"/>',
    map: '<polygon points="4,8 16,14 32,8 44,14 44,40 32,34 16,40 4,34"/><line x1="16" y1="14" x2="16" y2="40"/><line x1="32" y1="8" x2="32" y2="34"/>',
    mappin: '<path d="M24,42 C18,34 8,26 8,18 A16,16 0 0 1 40,18 C40,26 30,34 24,42 Z" fill="none"/><circle cx="24" cy="18" r="6"/>',
    market: '<rect x="6" y="22" width="36" height="22" rx="1"/><polyline points="6,22 6,12 42,12 42,22"/><path d="M6,12 Q14,20 22,12 Q30,20 38,12 Q42,16 42,22" fill="none"/><rect x="18" y="30" width="12" height="14" rx="1"/>',
    storefront: '<rect x="4" y="22" width="40" height="22" rx="1"/><polyline points="4,22 4,10 44,10 44,22"/><path d="M4,10 Q12,20 20,10 Q28,20 36,10 Q44,20 44,22" fill="none"/><rect x="16" y="30" width="16" height="14" rx="1"/><line x1="24" y1="30" x2="24" y2="44"/>',
    website: '<circle cx="24" cy="20" r="16"/><ellipse cx="24" cy="20" rx="7" ry="16"/><line x1="8" y1="20" x2="40" y2="20"/><path d="M10,12 Q24,16 38,12" fill="none"/><path d="M10,28 Q24,24 38,28" fill="none"/><line x1="24" y1="36" x2="24" y2="42"/><line x1="16" y1="42" x2="32" y2="42"/>',
    download: '<line x1="24" y1="6" x2="24" y2="30"/><polyline points="16,22 24,30 32,22"/><polyline points="8,36 8,42 40,42 40,36"/>',
    dev: '<polyline points="18,10 6,24 18,38"/><polyline points="30,10 42,24 30,38"/><line x1="28" y1="6" x2="20" y2="42"/>',
    settings: '<circle cx="24" cy="24" r="7"/><path d="M24,6 L26,12 A12,12 0 0 1 34,16 L38,12 L36,20 A12,12 0 0 1 38,24 L42,24 L38,28 A12,12 0 0 1 36,32 L38,36 L34,34 A12,12 0 0 1 26,38 L24,42 L22,38 A12,12 0 0 1 14,34 L10,36 L12,32 A12,12 0 0 1 10,28 L6,24 L10,20 A12,12 0 0 1 12,16 L10,12 L14,14 A12,12 0 0 1 22,12 Z"/>',
    ops: '<path d="M34,8 A12,12 0 0 0 18,16 L10,34 A6,6 0 1 0 16,40 L34,26 A12,12 0 0 0 42,16 L36,16 L36,10 L30,10 L30,14" fill="none"/><circle cx="13" cy="37" r="2"/>',
    games: '<path d="M10,16 A8,8 0 0 0 6,30 A6,6 0 0 0 14,34 L18,26 H30 L34,34 A6,6 0 0 0 42,30 A8,8 0 0 0 38,16 Z" fill="none"/><line x1="15" y1="20" x2="15" y2="26"/><line x1="12" y1="23" x2="18" y2="23"/><circle cx="31" cy="20" r="1.5"/><circle cx="35" cy="24" r="1.5"/>',
    journal: '<path d="M24,10 V40"/><path d="M24,10 C20,8 12,8 6,10 V40 C12,38 20,38 24,40" fill="none"/><path d="M24,10 C28,8 36,8 42,10 V40 C36,38 28,38 24,40" fill="none"/><line x1="12" y1="18" x2="20" y2="18"/><line x1="12" y1="24" x2="20" y2="24"/><line x1="12" y1="30" x2="18" y2="30"/><line x1="28" y1="18" x2="36" y2="18"/><line x1="28" y1="24" x2="36" y2="24"/>',

    // ── State / Action icons ──
    lock: '<rect x="10" y="22" width="28" height="22" rx="3"/><path d="M16,22 V14 A8,8 0 0 1 32,14 V22"/><circle cx="24" cy="33" r="3"/>',
    unlock: '<rect x="10" y="22" width="28" height="22" rx="3"/><path d="M16,22 V14 A8,8 0 0 1 32,14 V10"/><circle cx="24" cy="33" r="3"/>',
    shield: '<path d="M24,4 L42,12 V24 C42,36 24,44 24,44 C24,44 6,36 6,24 V12 Z" fill="none"/><polyline points="16,24 22,30 32,18"/>',
    key: '<circle cx="16" cy="20" r="10"/><line x1="24" y1="24" x2="42" y2="24"/><line x1="36" y1="20" x2="36" y2="28"/><line x1="42" y1="20" x2="42" y2="28"/>',
    bell: '<path d="M24,6 A12,12 0 0 1 36,18 V26 L40,32 H8 L12,26 V18 A12,12 0 0 1 24,6 Z"/><path d="M18,32 A6,6 0 0 0 30,32"/>',
    search: '<circle cx="20" cy="20" r="14"/><line x1="30" y1="30" x2="42" y2="42"/>',
    star: '<polygon points="24,4 29,18 44,18 32,28 36,42 24,34 12,42 16,28 4,18 19,18"/>',
    heart: '<path d="M24,40 L6,24 A10,10 0 0 1 24,12 A10,10 0 0 1 42,24 Z" fill="none"/>',
    save: '<rect x="6" y="6" width="36" height="36" rx="3"/><rect x="14" y="6" width="16" height="14" rx="1"/><rect x="14" y="28" width="20" height="14" rx="1"/><line x1="28" y1="8" x2="28" y2="18"/>',
    copy: '<rect x="14" y="14" width="24" height="28" rx="2"/><path d="M14,34 H10 A2,2 0 0 1 8,32 V8 A2,2 0 0 1 10,6 H32 A2,2 0 0 1 34,8 V14"/>',
    trash: '<polyline points="8,14 40,14"/><path d="M16,14 V8 H32 V14"/><path d="M12,14 L14,42 H34 L36,14"/><line x1="20" y1="20" x2="20" y2="36"/><line x1="28" y1="20" x2="28" y2="36"/>',
    edit: '<path d="M6,42 L12,26 L36,4 L44,12 L20,36 Z" fill="none"/><line x1="30" y1="10" x2="38" y2="18"/>',
    pin: '<path d="M20,28 L8,40"/><path d="M30,6 L42,18 L34,26 L22,26 L22,14 Z" fill="none"/>',
    clip: '<path d="M20,8 A6,6 0 0 1 32,8 V30 A10,10 0 0 1 12,30 V14" fill="none"/>',
    upload: '<line x1="24" y1="30" x2="24" y2="6"/><polyline points="16,14 24,6 32,14"/><polyline points="8,36 8,42 40,42 40,36"/>',
    refresh: '<path d="M38,24 A14,14 0 1 1 24,10" fill="none"/><polyline points="24,4 24,10 30,10"/>',
    close: '<line x1="12" y1="12" x2="36" y2="36"/><line x1="36" y1="12" x2="12" y2="36"/>',
    check: '<polyline points="8,24 18,34 40,12"/>',
    plus: '<line x1="24" y1="8" x2="24" y2="40"/><line x1="8" y1="24" x2="40" y2="24"/>',
    minus: '<line x1="8" y1="24" x2="40" y2="24"/>',
    menu: '<line x1="6" y1="12" x2="42" y2="12"/><line x1="6" y1="24" x2="42" y2="24"/><line x1="6" y1="36" x2="42" y2="36"/>',
    eye: '<path d="M4,24 Q24,6 44,24 Q24,42 4,24 Z" fill="none"/><circle cx="24" cy="24" r="6"/>',
    'eye-off': '<path d="M4,24 Q24,6 44,24 Q24,42 4,24 Z" fill="none"/><circle cx="24" cy="24" r="6"/><line x1="8" y1="40" x2="40" y2="8"/>',
    info: '<circle cx="24" cy="24" r="18"/><line x1="24" y1="20" x2="24" y2="34"/><circle cx="24" cy="14" r="2" fill="currentColor" stroke="none"/>',
    warning: '<path d="M24,6 L44,42 H4 Z" fill="none"/><line x1="24" y1="18" x2="24" y2="30"/><circle cx="24" cy="36" r="2" fill="currentColor" stroke="none"/>',
    block: '<circle cx="24" cy="24" r="18"/><line x1="12" y1="12" x2="36" y2="36"/>',

    // ── Media / Communication ──
    mic: '<rect x="16" y="4" width="16" height="24" rx="8"/><path d="M10,24 A14,14 0 0 0 38,24" fill="none"/><line x1="24" y1="38" x2="24" y2="44"/>',
    'mic-off': '<rect x="16" y="4" width="16" height="24" rx="8"/><path d="M10,24 A14,14 0 0 0 38,24" fill="none"/><line x1="24" y1="38" x2="24" y2="44"/><line x1="6" y1="6" x2="42" y2="42"/>',
    video: '<rect x="4" y="12" width="26" height="24" rx="2"/><polyline points="30,20 44,12 44,36 30,28"/>',
    'video-off': '<rect x="4" y="12" width="26" height="24" rx="2"/><polyline points="30,20 44,12 44,36 30,28"/><line x1="6" y1="6" x2="42" y2="42"/>',
    phone: '<rect x="14" y="4" width="20" height="40" rx="3"/><line x1="20" y1="38" x2="28" y2="38"/>',
    'phone-call': '<path d="M8,14 A24,24 0 0 0 34,40 L40,36 L34,28 L30,30 A14,14 0 0 1 18,18 L20,14 L12,8 Z" fill="none"/>',
    monitor: '<rect x="6" y="6" width="36" height="28" rx="2"/><line x1="24" y1="34" x2="24" y2="42"/><line x1="14" y1="42" x2="34" y2="42"/>',
    image: '<rect x="6" y="10" width="36" height="28" rx="2"/><circle cx="16" cy="20" r="4"/><polyline points="42,30 32,22 20,34 14,28 6,36"/>',
    film: '<rect x="6" y="8" width="36" height="32" rx="2"/><line x1="6" y1="16" x2="42" y2="16"/><line x1="6" y1="32" x2="42" y2="32"/><line x1="16" y1="8" x2="16" y2="16"/><line x1="32" y1="8" x2="32" y2="16"/><line x1="16" y1="32" x2="16" y2="40"/><line x1="32" y1="32" x2="32" y2="40"/>',
    speaker: '<path d="M22,14 L14,20 H6 V28 H14 L22,34 Z" fill="none"/><path d="M30,16 A10,10 0 0 1 30,32" fill="none"/><path d="M34,10 A16,16 0 0 1 34,38" fill="none"/>',
    music: '<circle cx="14" cy="36" r="6"/><circle cx="38" cy="32" r="6"/><line x1="20" y1="36" x2="20" y2="8"/><line x1="44" y1="32" x2="44" y2="4"/><line x1="20" y1="8" x2="44" y2="4"/>',

    // ── Objects / Categories ──
    globe: '<circle cx="24" cy="24" r="20"/><ellipse cx="24" cy="24" rx="8" ry="20"/><line x1="4" y1="24" x2="44" y2="24"/><path d="M8,14 Q24,18 40,14" fill="none"/><path d="M8,34 Q24,30 40,34" fill="none"/>',
    rocket: '<path d="M24,8 Q18,18 18,32 L24,38 L30,32 Q30,18 24,8 Z" fill="none"/><circle cx="24" cy="22" r="3"/><path d="M18,28 Q12,30 10,36" fill="none"/><path d="M30,28 Q36,30 38,36" fill="none"/><path d="M20,38 L24,44 L28,38" fill="none"/>',
    box: '<path d="M6,16 L24,6 L42,16 V36 L24,44 L6,36 Z" fill="none"/><line x1="24" y1="24" x2="24" y2="44"/><line x1="6" y1="16" x2="24" y2="24"/><line x1="42" y1="16" x2="24" y2="24"/>',
    seed: '<path d="M12,40 Q12,20 24,10 Q36,20 36,40" fill="none"/><line x1="24" y1="10" x2="24" y2="40"/><path d="M24,20 Q18,22 16,28" fill="none"/><path d="M24,26 Q30,28 32,34" fill="none"/>',
    coin: '<ellipse cx="24" cy="24" rx="16" ry="18"/><ellipse cx="24" cy="24" rx="10" ry="12"/><line x1="24" y1="16" x2="24" y2="32"/>',
    cloud: '<path d="M12,36 A10,10 0 0 1 12,20 A12,12 0 0 1 36,16 A8,8 0 0 1 40,30 A6,6 0 0 1 36,36 Z" fill="none"/>',
    link: '<path d="M18,30 L30,18" fill="none"/><path d="M14,34 A8,8 0 0 1 14,22 L20,16" fill="none"/><path d="M34,14 A8,8 0 0 1 34,26 L28,32" fill="none"/>',
    compass: '<circle cx="24" cy="24" r="18"/><polygon points="24,10 28,24 24,38 20,24" fill="none"/>',
    grid: '<rect x="6" y="6" width="36" height="36" rx="2"/><line x1="6" y1="18" x2="42" y2="18"/><line x1="6" y1="30" x2="42" y2="30"/><line x1="18" y1="6" x2="18" y2="42"/><line x1="30" y1="6" x2="30" y2="42"/>',
    weather: '<circle cx="20" cy="18" r="8"/><path d="M14,32 A10,10 0 0 1 14,20" fill="none"/><path d="M34,28 A8,8 0 0 1 16,32 H36 A6,6 0 0 1 34,28 Z" fill="none"/>',
    tool: '<path d="M28,20 L42,6" fill="none"/><circle cx="18" cy="30" r="12"/><line x1="18" y1="24" x2="18" y2="36"/><line x1="12" y1="30" x2="24" y2="30"/>',
    cube: '<path d="M24,4 L44,16 V32 L24,44 L4,32 V16 Z" fill="none"/><line x1="24" y1="24" x2="24" y2="44"/><line x1="4" y1="16" x2="24" y2="24"/><line x1="44" y1="16" x2="24" y2="24"/>',
    car: '<path d="M8,30 H40 V38 H8 Z" fill="none"/><path d="M10,30 L16,20 H32 L38,30" fill="none"/><circle cx="14" cy="38" r="4"/><circle cx="34" cy="38" r="4"/>',
    blueprint: '<rect x="6" y="8" width="36" height="32" rx="2"/><line x1="14" y1="16" x2="34" y2="16"/><line x1="14" y1="24" x2="28" y2="24"/><line x1="14" y1="32" x2="22" y2="32"/>',
    storage: '<rect x="6" y="8" width="36" height="12" rx="2"/><rect x="6" y="24" width="36" height="12" rx="2"/><circle cx="34" cy="14" r="2" fill="currentColor" stroke="none"/><circle cx="34" cy="30" r="2" fill="currentColor" stroke="none"/>',

    // ── Status dots (filled circles, no stroke) ──
    'dot-green': '<circle cx="24" cy="24" r="8" fill="#4a9" stroke="none"/>',
    'dot-red': '<circle cx="24" cy="24" r="8" fill="#c44" stroke="none"/>',
    'dot-yellow': '<circle cx="24" cy="24" r="8" fill="#ca3" stroke="none"/>',
    'dot-black': '<circle cx="24" cy="24" r="8" fill="#555" stroke="none"/>',
    'dot-blue': '<circle cx="24" cy="24" r="8" fill="#48f" stroke="none"/>',

    // ── Social / Brand (simple geometric representations) ──
    discord: '<path d="M14,12 Q24,8 34,12 V34 Q24,40 14,34 Z" fill="none"/><circle cx="18" cy="24" r="3"/><circle cx="30" cy="24" r="3"/>',
    github: '<circle cx="24" cy="24" r="18"/><path d="M18,42 V34 Q18,30 24,28 Q30,30 30,34 V42" fill="none"/><circle cx="24" cy="18" r="6"/>',
    twitch: '<path d="M8,6 H40 V30 L32,38 H24 L18,44 V38 H8 Z" fill="none"/><line x1="20" y1="16" x2="20" y2="26"/><line x1="28" y1="16" x2="28" y2="26"/>',
    youtube: '<rect x="4" y="10" width="40" height="28" rx="4"/><polygon points="20,18 34,24 20,30" fill="none"/>',
  };

  // ── Public API ──
  window.hosIcon = function(name, size, colorOverride) {
    size = size || 20;
    var pathData = PATHS[name];
    if (!pathData) return '<span style="display:inline-block;width:' + size + 'px;height:' + size + 'px"></span>';
    var style = 'stroke:' + (colorOverride || 'currentColor') +
      ';fill:none;stroke-linecap:round;stroke-linejoin:round;stroke-width:var(--icon-weight,' + weight + ')';
    return '<svg viewBox="0 0 48 48" width="' + size + '" height="' + size +
      '" style="' + style + ';display:inline-block;vertical-align:middle">' + pathData + '</svg>';
  };

  // ── Expose weight setter for settings page ──
  window.hosSetIconWeight = function(w) {
    w = parseFloat(w);
    if (isNaN(w) || w < 1 || w > 6) return;
    weight = w;
    localStorage.setItem('hos_icon_weight', w);
    document.documentElement.style.setProperty('--icon-weight', w);
  };

  window.hosGetIconWeight = function() { return weight; };

  // ── Expose icon names for dev page ──
  window.hosIconNames = function() { return Object.keys(PATHS); };
})();
