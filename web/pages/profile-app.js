// Moved out of web/pages/profile.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    var STORE_KEY = 'humanity_profile_full_v1';

    // ── Tab navigation ──
    function showTab(id) {
      document.querySelectorAll('.prof-tab').forEach(function(t) { t.classList.remove('active'); });
      document.querySelectorAll('.prof-section-btn').forEach(function(b) { b.classList.remove('active'); });
      var tab = document.getElementById('tab-' + id);
      if (tab) tab.classList.add('active');
      var btn = document.querySelector('[data-tab="' + id + '"]');
      if (btn) btn.classList.add('active');
    }

    // ── Field map: id → key ──
    var FIELDS = [
      'height','weight','eye-color','blood-type',
      'hair-color','hair-length','hair-style','hair-texture',
      'neck','shoulders','chest','waist','hips','thighs','inseam','shoe-size','shirt-size','pants-size',
      'first-name','last-name','dob','timezone','city','country',
      'emergency-name','emergency-rel','emergency-phone',
      'allergies','medical','private-notes',
      'display-name','pronouns','bio','location','website','avatar-url','banner-url',
      'directory-listed',
      'interests','skills','looking-for',
      'twitter','youtube','twitch','github','discord','instagram','tiktok','mastodon','linkedin','portfolio',
      'kofi','patreon',
      'yt-enabled','yt-key','yt-rtmp',
      'twitch-enabled','twitch-key','twitch-rtmp',
      'rumble-enabled','rumble-key','rumble-rtmp',
      'facebook-enabled','facebook-key','facebook-rtmp',
      'x-enabled','x-key','x-rtmp',
      'kick-enabled','kick-key','kick-rtmp',
      'tiktok-enabled','tiktok-key','tiktok-rtmp',
      'linkedin-enabled','linkedin-key','linkedin-rtmp',
      'custom-enabled','custom-key','custom-rtmp',
      'stream-title','stream-category'
    ];

    // Checkbox fields that use .checked instead of .value
    var CHECKBOX_FIELDS = ['yt-enabled','twitch-enabled','rumble-enabled','facebook-enabled',
      'x-enabled','kick-enabled','tiktok-enabled','linkedin-enabled','custom-enabled',
      'directory-listed'];

    // ── Load from localStorage ──
    function loadProfile() {
      var data = {};
      try { data = JSON.parse(localStorage.getItem(STORE_KEY) || '{}') || {}; } catch (_) {}
      FIELDS.forEach(function(f) {
        var el = document.getElementById('f-' + f);
        if (!el) return;
        if (CHECKBOX_FIELDS.indexOf(f) >= 0) {
          // Rumble defaults to enabled; others default to false if not yet saved
          if (data[f] !== undefined) el.checked = !!data[f];
        } else {
          if (data[f] !== undefined) el.value = data[f];
        }
      });
    }

    // ── Save all to localStorage ──
    function saveAll() {
      var data = {};
      FIELDS.forEach(function(f) {
        var el = document.getElementById('f-' + f);
        if (!el) return;
        data[f] = CHECKBOX_FIELDS.indexOf(f) >= 0 ? el.checked : el.value;
      });
      localStorage.setItem(STORE_KEY, JSON.stringify(data));
      var st = document.getElementById('save-status');
      st.textContent = '✓ Saved';
      st.className = 'saved';
      setTimeout(function() { st.textContent = ''; st.className = ''; }, 2500);
    }

    // Auto-save on field change
    var saveTimer = null;
    document.getElementById('profile-main').addEventListener('input', function() {
      clearTimeout(saveTimer);
      saveTimer = setTimeout(saveAll, 800);
    });

    // ── Per-field privacy (public 🌐 <-> friends only) ──
    // Persisted in its own key so it round-trips on page reload; folded into the
    // canonical `humanity_profile` privacy map on sync.
    var PRIVACY_KEY = 'humanity_profile_privacy';
    var PRIVACY_FIELDS = ['pronouns', 'location', 'website'];
    var profilePrivacy = {};

    function loadPrivacy() {
      try { profilePrivacy = JSON.parse(localStorage.getItem(PRIVACY_KEY) || '{}') || {}; }
      catch (_) { profilePrivacy = {}; }
      PRIVACY_FIELDS.forEach(function(f) { renderPrivacyLock(f); });
    }

    function renderPrivacyLock(field) {
      var btn = document.getElementById('privacy-' + field);
      if (!btn) return;
      var isPrivate = profilePrivacy[field] === 'private';
      btn.classList.toggle('is-private', isPrivate);
      btn.setAttribute('aria-pressed', isPrivate ? 'true' : 'false');
      var name = field.charAt(0).toUpperCase() + field.slice(1);
      if (isPrivate) {
        btn.textContent = '🔒 Friends';
        btn.title = 'Visible to friends only, click to make it public';
        btn.setAttribute('aria-label', name + ' visibility: friends only. Activate to make it public.');
      } else {
        btn.textContent = '🌐 Public';
        btn.title = 'Visible to everyone, click to make it friends only';
        btn.setAttribute('aria-label', name + ' visibility: public. Activate to make it visible to friends only.');
      }
    }

    function togglePrivacy(field) {
      profilePrivacy[field] = profilePrivacy[field] === 'private' ? 'public' : 'private';
      localStorage.setItem(PRIVACY_KEY, JSON.stringify(profilePrivacy));
      renderPrivacyLock(field);
    }

    // ── Bio character counter, and the characters past the limit in red ──
    // A soft limit (2026-10-24): the bio no longer stops at 280 (maxlength cut a longer paste
    // without a word, and the save cut it again); what is past it shows in red on dark red
    // (shared/over-limit.js), counted in characters as the server counts, and Save says to
    // shorten it. The desktop app's Profile page does the same.
    var BIO_MAX = 280;
    function updateBioCount() {
      var bio = document.getElementById('f-bio');
      var counter = document.getElementById('bio-count');
      if (!bio || !counter) return;
      var len = Array.from(bio.value).length;
      counter.textContent = len + ' / ' + BIO_MAX;
      counter.className = 'bio-count' + (len > BIO_MAX ? ' over' : len > BIO_MAX - 40 ? ' warn' : '');
      if (window.hosOverLimit) window.hosOverLimit(bio, BIO_MAX);
    }

    /**
     * Derive a bare handle from a full URL or an already-bare handle, so the
     * chat View-Profile card (which builds x.com/{handle}, github.com/{handle})
     * links correctly. `x.com/foo` -> `foo`; `@foo` -> `foo`; `foo` -> `foo`.
     */
    function bareHandle(v) {
      if (!v) return '';
      return String(v).trim()
        .replace(/^https?:\/\//i, '')
        .replace(/^(www\.)?[^\/]+\//, '')
        .replace(/^@/, '')
        .replace(/\/+$/, '');
    }

    // ── Save the network-facing profile to the canonical `humanity_profile`
    // store, the SAME object web/chat/chat-profile.js reads and pushes to the
    // relay. Writing it fires a cross-tab `storage` event, so an already-open
    // Chat tab syncs to the relay immediately; otherwise it syncs on next connect.
    // (This replaces the old `humanity_profile_pending_sync` hand-off, which only
    // applied after a full reconnect.) ──
    function syncNetworkProfile() {
      var val = function(id) { var el = document.getElementById('f-' + id); return el ? el.value.trim() : ''; };

      // Social handles, shaped to what the chat profile card renders.
      var socials = {};
      var portfolio = val('portfolio');
      if (portfolio)      socials.website = portfolio;              // rendered as a link
      if (val('discord')) socials.discord = val('discord');        // plain text
      if (val('twitter')) socials.twitter = bareHandle(val('twitter'));
      if (val('youtube')) socials.youtube = val('youtube');        // card accepts full URL or @handle
      if (val('github'))  socials.github  = bareHandle(val('github'));

      // Privacy map: only record fields the user explicitly set to private, plus
      // the member-directory choice as a word, always: "listed" (in /api/members) or
      // "unlisted". Only "listed" lists, so no word at all is never choosing, and a server
      // leaves that person out (BUG-241, 2026-11-03).
      var privacy = {};
      PRIVACY_FIELDS.forEach(function(f) { if (profilePrivacy[f] === 'private') privacy[f] = 'private'; });
      var dir = document.getElementById('f-directory-listed');
      if (dir) privacy.directory = dir.checked ? 'listed' : 'unlisted';

      // Never cut: a bio past the limit is not saved, and the page says why (its excess is red).
      var bioChars = Array.from(val('bio')).length;
      if (bioChars > BIO_MAX) {
        var refused = document.getElementById('sync-status');
        refused.textContent = i18n.t('profile.bio_over_limit', { n: bioChars, limit: BIO_MAX });
        refused.style.color = 'var(--danger)';
        var bioBox = document.getElementById('f-bio');
        if (bioBox) bioBox.focus();
        return;
      }

      var profile = {
        bio: val('bio'),
        socials: socials,
        avatar_url: val('avatar-url').substring(0, 512),
        banner_url: val('banner-url').substring(0, 512),
        pronouns: val('pronouns').substring(0, 64),
        location: val('location').substring(0, 128),
        website: val('website').substring(0, 256),
        privacy: privacy,
      };

      // Persist the full page state too, so nothing typed is lost on reload.
      saveAll();
      // Which fields this save changes, recorded BEFORE the profile is written, so the Chat tab the
      // write wakes (storage event) sends them; Chat sends only fields changed in this browser, on
      // top of the server's copy (BUG-195, /shared/profile-changes.js). Every other key of the
      // stored profile is kept.
      var before = {};
      try { before = JSON.parse(localStorage.getItem('humanity_profile') || '{}') || {}; } catch (_) { before = {}; }
      if (window.HosProfileChanges) {
        window.HosProfileChanges.add(localStorage, window.HosProfileChanges.changedFields(before, profile));
      }
      localStorage.setItem('humanity_profile', JSON.stringify(Object.assign({}, before, profile)));

      var status = document.getElementById('sync-status');
      // If a Chat tab is open in this browser it will catch the `storage` event
      // and push to the relay right away; if not, it syncs on next connect.
      status.textContent = '✓ Saved. Syncing to the network via Chat…';
      status.style.color = 'var(--success)';
      setTimeout(function() { status.textContent = ''; status.style.color = ''; }, 5000);
    }

    // ── Copy OBS multistream config ──
    function copyObsConfig() {
      var allPlatforms = [
        { id: 'yt',       name: 'YouTube' },
        { id: 'twitch',   name: 'Twitch' },
        { id: 'rumble',   name: 'Rumble' },
        { id: 'facebook', name: 'Facebook/Instagram' },
        { id: 'x',        name: 'X (Twitter)' },
        { id: 'kick',     name: 'Kick' },
        { id: 'tiktok',   name: 'TikTok' },
        { id: 'linkedin', name: 'LinkedIn' },
        { id: 'custom',   name: 'Custom' },
      ];
      var platforms = allPlatforms.map(function(p) {
        var enabled = document.getElementById('f-' + p.id + '-enabled');
        var key  = document.getElementById('f-' + p.id + '-key');
        var rtmp = document.getElementById('f-' + p.id + '-rtmp');
        return { name: p.name, enabled: enabled && enabled.checked, key: key && key.value, rtmp: rtmp && rtmp.value };
      }).filter(function(p) { return p.enabled && p.key && p.rtmp; });
      if (platforms.length === 0) {
        alert('Enter at least one stream key + RTMP server first.');
        return;
      }
      var config = platforms.map(function(p) {
        return '# ' + p.name + '\n[rtmp]\nServer=' + p.rtmp + '\nKey=' + p.key + '\n';
      }).join('\n');
      navigator.clipboard.writeText(config).then(function() {
        alert('OBS multistream config copied to clipboard!\n\nPaste into OBS → Tools → obs-multi-rtmp or your multistream plugin config.');
      }).catch(function() {
        var textarea = document.createElement('textarea');
        textarea.value = config;
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
        alert('Copied!');
      });
    }

    // Backfill the network-profile fields from the canonical `humanity_profile`
    // store (what Chat reads/pushes) for anything the page's own store doesn't
    // already hold, e.g. a profile first set from the chat client. The page's
    // typed state wins; this only fills blanks so the page reflects what will sync.
    function backfillFromCanonical() {
      var canon;
      try { canon = JSON.parse(localStorage.getItem('humanity_profile') || '{}') || {}; }
      catch (_) { return; }
      var map = {
        'bio': canon.bio, 'pronouns': canon.pronouns, 'location': canon.location,
        'website': canon.website, 'avatar-url': canon.avatar_url, 'banner-url': canon.banner_url,
      };
      Object.keys(map).forEach(function(id) {
        var el = document.getElementById('f-' + id);
        if (el && !el.value && map[id]) el.value = map[id];
      });
      // The directory choice from the canonical privacy map (BUG-241): only "listed" ticks the
      // box. Not listed there always unticks it, as "unlisted" did before, so the page never says
      // listed while the server leaves the person out; listed there ticks it unless the page's
      // own store holds a choice of its own (typed here, not synced yet).
      var dir = document.getElementById('f-directory-listed');
      if (dir) {
        var canonListed = !!(canon.privacy && canon.privacy.directory === 'listed');
        var pageChose = false;
        try { pageChose = (JSON.parse(localStorage.getItem(STORE_KEY) || '{}') || {})['directory-listed'] !== undefined; } catch (_) {}
        if (!canonListed) dir.checked = false;
        else if (!pageChose) dir.checked = true;
      }
      // Seed the privacy locks if the page has no privacy state of its own yet.
      if (!localStorage.getItem(PRIVACY_KEY) && canon.privacy) {
        PRIVACY_FIELDS.forEach(function(f) {
          if (canon.privacy[f] === 'private') profilePrivacy[f] = 'private';
        });
      }
    }

    // Chat updates this page's stores when my own profile changes on another device (BUG-195,
    // chat-profile.js followOnProfilePage): show it here at once, as it would show on a reload.
    window.addEventListener('storage', function(e) {
      if (e.key === STORE_KEY) { loadProfile(); updateBioCount(); }
      if (e.key === PRIVACY_KEY) loadPrivacy();
    });

    // ── Init ──
    loadProfile();
    loadPrivacy();
    backfillFromCanonical();
    PRIVACY_FIELDS.forEach(renderPrivacyLock);
    updateBioCount();
    var bioEl = document.getElementById('f-bio');
    if (bioEl) bioEl.addEventListener('input', updateBioCount);

    // ═══════════════════════════════════════════════════════
    // Skills tab, domains, XP, endorsements
    // ═══════════════════════════════════════════════════════

    // ── The skills base set (skills rung 1, 2026-11-14, docs/design/skills.md) ──
    // The same skills, families and named stages as the game, read from data/skills/ (skills.csv,
    // families.json, stages.json); every word comes from data/i18n/en.json by key. This tab is the
    // real-life twin (skills.md section 5): your own record of where you stand, kept in this browser.
    // The native twin is the Skills section of Profile in the app (src/gui/pages/profile_skills.rs).
    var SKILLS = [];          // [{ id, family, base, ladder: [{ stage, from_level, can: [keys] }] }]
    var SKILL_FAMILIES = [];  // family ids, in page order
    var SKILL_STAGES = [];    // stage ids, Untrained first

    // XP curve lives in data/skills/xp_curve.json (shared with the tasks page, and the game's
    // rule, BUG-248): its first levels as a table, and past the table the same formula, base x
    // level^1.5, because there is no top level. The literals below are only the offline fallback,
    // held to the file by scripts/tests/xp-curve-web.test.js.
    var SKILL_XP_PER_LEVEL = [100, 282, 519, 800, 1118, 1469, 1852, 2262, 2700, 3162];
    var SKILL_XP_BASE = 100;
    fetch('/data/skills/xp_curve.json')
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (d) {
        if (d && Array.isArray(d.xp_per_level) && d.xp_per_level.length) {
          SKILL_XP_PER_LEVEL = d.xp_per_level;
          SKILL_XP_BASE = d.base || SKILL_XP_BASE;
        }
      })
      .catch(function () { /* offline: keep the fallback */ });
    var SKILL_STORAGE_KEY = 'hos_skills_v1';

    var skillData = {};
    var skillEndorsements = {};
    var skill_activeFamily = null;

    // ── Practice quality (skills rung 2, 2026-11-15, docs/design/skills.md section 3) ──
    // The game weighs each practice by data/skills/practice_quality.json (crates/humanity-systems/
    // src/skills/quality.rs). Here each "Practised" press is one try at that skill, weighed by the
    // file's Simplified level (the game's default; the game's Settings > Gameplay > Skill learning
    // chooses another there), and the card says why it gave what it gave in the game's words. The
    // Library guide for the stage reached is linked from data/skills/guides.json. Held to the file by
    // scripts/tests/skills-practice-web.test.js.
    var SKILL_QUALITY = null;      // practice_quality.json's Simplified level { variety, guide, mentor }
    var SKILL_GUIDES = {};         // skill id -> stage id -> Library slug
    var SKILL_GUIDE_TITLES = {};   // Library slug -> title
    var skillWhy = {};             // skill id -> the lines saying why its last press gave what it gave

    /** What try number `attempt` (1 = the first today) is worth at the variety rules `v`. */
    function skill_varietyFactor(v, attempt) {
      var past = Math.max(0, attempt - Math.max(1, v.full_attempts));
      return Math.max(v.floor, Math.pow(v.per_repeat, past));
    }

    /** Weigh one try at the variety rules `v` for the skill's saved record `s` at `now` (ms), and
     *  remember it: its number today, what repeating made it worth, and the first-time bonus. */
    function skill_weighTry(v, s, now) {
      var rest = v.rest_days * 86400000;
      s.log = (s.log || []).filter(function (t) { return now - t < rest && t <= now; });
      var attempt = s.log.length + 1;
      var full = Math.max(1, v.full_attempts);
      var past = Math.max(0, attempt - full);
      var variety = Math.max(v.floor, Math.pow(v.per_repeat, past));
      var first = s.tried ? 1 : Math.max(1, v.first_time);
      s.log.push(now);
      if (s.log.length > 200) s.log.splice(0, s.log.length - 200);
      s.tried = true;
      return { attempt: attempt, full: full, variety: variety, first: first };
    }

    /** A multiplier in words: 1.5 is "1.5", 2 is "2". */
    function skill_times(x) { return String(Math.round(x * 100) / 100); }
    /** A share as a whole percent: 0.85 is "85". */
    function skill_percent(x) { return String(Math.round(x * 100)); }

    /** "How practice counts" at the website's level, from the file's numbers. */
    function skill_renderHow() {
      var box = document.getElementById('skill-how');
      var rules = document.getElementById('skill-how-rules');
      if (!box || !rules || !SKILL_QUALITY) return;
      var lines = [];
      var v = SKILL_QUALITY.variety, g = SKILL_QUALITY.guide, m = SKILL_QUALITY.mentor;
      if (v) lines.push(skill_t('skills_page.how_variety', { full: v.full_attempts, percent: skill_percent(v.per_repeat), floor: skill_percent(v.floor), first: skill_times(v.first_time) }));
      if (g) lines.push(skill_t('skills_page.how_guide', { times: skill_times(g.multiplier), hours: skill_times(g.hours) }));
      if (m) lines.push(skill_t('skills_page.how_mentor', { reach: skill_times(m.reach_m), above: m.stages_above, times: skill_times(m.multiplier) }));
      if (!lines.length) lines.push(skill_t('skills_page.how_off'));
      rules.innerHTML = lines.map(function (l) { return '<p>' + l + '</p>'; }).join('');
      box.hidden = false;
    }

    function skill_t(key, vars) { return (window.i18n && i18n.t) ? i18n.t(key, vars) : key; }

    function skill_loadData() {
      try {
        var raw = localStorage.getItem(SKILL_STORAGE_KEY);
        if (raw) skillData = JSON.parse(raw);
      } catch (e) { skillData = {}; }
    }

    function skill_saveData() {
      try { localStorage.setItem(SKILL_STORAGE_KEY, JSON.stringify(skillData)); } catch (e) { /* storage blocked */ }
    }

    function skill_get(id) {
      if (!skillData[id]) skillData[id] = { level: 0, xp: 0 };
      return skillData[id];
    }

    function skill_getById(id) {
      for (var i = 0; i < SKILLS.length; i++) if (SKILLS[i].id === id) return SKILLS[i];
      return null;
    }

    /** XP to go from `level` to `level + 1` for a skill of XP base `base` (no top level). */
    function skill_xpForLevel(level, base) {
      var b = base || SKILL_XP_BASE;
      if (b === SKILL_XP_BASE && SKILL_XP_PER_LEVEL[level]) return SKILL_XP_PER_LEVEL[level];
      return Math.floor(b * Math.pow(level + 1, 1.5));
    }

    /** How many stages of the skill's ladder `level` has reached (0 = Untrained). */
    function skill_stageIndex(sk, level) {
      var n = 0;
      for (var i = 0; i < sk.ladder.length; i++) if (level >= sk.ladder[i].from_level) n++;
      return n;
    }

    /** Progress 0..1 from the start of the stage `level` is in to the next stage. */
    function skill_stageProgress(sk, level, xp) {
      var i = skill_stageIndex(sk, level);
      var next = sk.ladder[i];
      if (!next) return 1;
      var start = i > 0 ? sk.ladder[i - 1].from_level : 0;
      var need = 0, done = xp;
      for (var l = start; l < next.from_level; l++) need += skill_xpForLevel(l, sk.base);
      for (var m = start; m < level; m++) done += skill_xpForLevel(m, sk.base);
      return need > 0 ? Math.min(1, done / need) : 1;
    }

    function skill_stageName(stage) { return skill_t('skill_stages.' + stage + '.name'); }

    function skill_addXp(skillId, amount, cardEl) {
      var sk = skill_getById(skillId);
      if (!sk) return;
      var s = skill_get(skillId);
      var before = skill_stageIndex(sk, s.level);
      // Weighed the way the game weighs practice (rung 2): repeats today count for less.
      if (SKILL_QUALITY && SKILL_QUALITY.variety) {
        var w = skill_weighTry(SKILL_QUALITY.variety, s, Date.now());
        var gained = Math.max(1, Math.round(amount * w.variety * w.first));
        var lines = [skill_t('skills_page.web_logged', { gained: gained, base: amount })];
        if (w.first > 1) lines.push(skill_t('skills_page.why_first_time', { times: skill_times(w.first) }));
        lines.push(w.variety < 0.999
          ? skill_t('skills_page.why_repeat', { attempt: w.attempt, percent: skill_percent(w.variety) })
          : skill_t('skills_page.why_full', { attempt: w.attempt, full: w.full }));
        skillWhy[skillId] = lines;
        amount = gained;
      }
      s.xp += amount;
      var leveled = false;
      while (s.xp >= skill_xpForLevel(s.level, sk.base)) {
        s.xp -= skill_xpForLevel(s.level, sk.base);
        s.level++;
        leveled = true;
      }
      skill_saveData();
      skill_refreshCard(skillId, cardEl);
      if (leveled) {
        cardEl = document.getElementById('card-' + skillId);
        if (cardEl) { cardEl.classList.remove('levelup'); void cardEl.offsetWidth; cardEl.classList.add('levelup'); }
        var after = skill_stageIndex(sk, s.level);
        skill_showToast(after > before
          ? skill_t('skills_page.stage_up', { skill: skill_t('skills.' + skillId + '.name'), stage: skill_stageName(sk.ladder[after - 1].stage) })
          : skill_t('skills_page.level_up', { skill: skill_t('skills.' + skillId + '.name'), level: s.level }));
      }
      skill_renderSidebar();
    }

    /** Self-assessment: put the skill at the start of the chosen stage. */
    function skill_setStage(skillId, stageIdx, cardEl) {
      var sk = skill_getById(skillId);
      if (!sk) return;
      var s = skill_get(skillId);
      s.level = stageIdx > 0 && sk.ladder[stageIdx - 1] ? sk.ladder[stageIdx - 1].from_level : 0;
      s.xp = 0;
      skill_saveData();
      skill_refreshCard(skillId, cardEl);
      skill_renderSidebar();
    }

    function skill_familySkills(fam) { return SKILLS.filter(function (s) { return s.family === fam; }); }

    function skill_practised(fam) {
      return skill_familySkills(fam).filter(function (s) { var p = skill_get(s.id); return p.level > 0 || p.xp > 0; }).length;
    }

    function skill_renderSidebar() {
      var list = document.getElementById('domain-list');
      if (!list) return;
      list.innerHTML = SKILL_FAMILIES.map(function (f) {
        var total = skill_familySkills(f).length;
        var done = skill_practised(f);
        var pct = total ? (done / total * 100).toFixed(1) : 0;
        var isActive = skill_activeFamily === f;
        return '<div class="domain-item' + (isActive ? ' active' : '') + '" style="--domain-color:var(--accent)" data-id="' + f + '" role="button" tabindex="0"' +
          ' data-i18n-tip="skills_page.family_hint" data-tip-desc="' + skill_t('skills_page.family_hint') + '" data-on-click="skill_selectFamily(' + hosArg(f) + ')">' +
          '<div class="domain-dot"></div>' +
          '<div style="flex:1;min-width:0">' +
            '<div class="domain-name">' + skill_t('skill_families.' + f + '.name') + '</div>' +
            '<div class="domain-bar-wrap"><div class="domain-bar-fill" style="width:' + pct + '%"></div></div>' +
          '</div>' +
          '<div class="domain-avg">' + done + '/' + total + '</div>' +
        '</div>';
      }).join('');
      if (skill_activeFamily) {
        var countEl = document.getElementById('panel-avg-level');
        if (countEl) countEl.textContent = skill_practised(skill_activeFamily) + '/' + skill_familySkills(skill_activeFamily).length;
      }
    }

    function skill_selectFamily(fam) {
      if (SKILL_FAMILIES.indexOf(fam) < 0) return;
      skill_activeFamily = fam;
      var emptyEl = document.getElementById('empty-state');
      var contentEl = document.getElementById('domain-content');
      if (emptyEl) emptyEl.style.display = 'none';
      if (contentEl) contentEl.style.display = 'block';
      document.getElementById('panel-domain-title').textContent = skill_t('skill_families.' + fam + '.name');
      document.getElementById('panel-domain-desc').textContent = skill_t('skill_families.' + fam + '.desc');
      skill_renderGrid(fam);
      skill_renderSidebar();
      try { localStorage.setItem('hos_skills_last_family', fam); } catch (e) { /* storage blocked */ }
    }

    function skill_renderGrid(fam) {
      var grid = document.getElementById('skill-grid');
      if (!grid) return;
      grid.innerHTML = skill_familySkills(fam).map(skill_buildCard).join('');
    }

    function skill_list(keys, cls) {
      return '<ul class="skill-can ' + cls + '">' + keys.map(function (k) { return '<li>' + skill_t(k) + '</li>'; }).join('') + '</ul>';
    }

    function skill_buildCard(sk) {
      var s = skill_get(sk.id);
      var reached = skill_stageIndex(sk, s.level);
      var stage = reached > 0 ? sk.ladder[reached - 1].stage : 'untrained';
      var next = sk.ladder[reached];
      var pct = (skill_stageProgress(sk, s.level, s.xp) * 100).toFixed(1);
      var name = skill_t('skills.' + sk.id + '.name');
      var done = [];
      for (var i = 0; i < reached; i++) done = done.concat(sk.ladder[i].can);
      var endorsement = skillEndorsements[sk.id];
      var endorseBadge = endorsement && endorsement.count > 0
        ? '<span style="font-size:0.7rem;color:var(--pg-gold-ink);margin-top:var(--space-xs);display:block;">🏅 ' + endorsement.count + '</span>'
        : '';
      var guideSlug = (SKILL_GUIDES[sk.id] || {})[stage];
      var stageOptions = SKILL_STAGES.map(function (st, idx) {
        return '<option value="' + idx + '"' + (idx === reached ? ' selected' : '') + '>' + skill_stageName(st) + '</option>';
      }).join('');
      var practise = function (n) {
        return '<button class="xp-btn" data-i18n-tip="skills_page.log_practice_hint" data-tip-desc="' + skill_t('skills_page.log_practice_hint') + '"' +
          ' data-on-click="skill_addXp(' + hosArg(sk.id) + ', ' + hosNum(n) + ', document.getElementById(' + hosArg('card-' + sk.id) + '))">' + skill_t('skills_page.log_practice', { xp: n }) + '</button>';
      };
      return '<div class="skill-card" id="card-' + sk.id + '">' +
        '<div class="skill-card-header">' +
          '<div>' +
            '<div class="skill-name">' + name + '</div>' +
            '<div class="skill-tags">' + skill_t('skills_page.stage_level', { stage: skill_stageName(stage), level: s.level }) + '</div>' +
          '</div>' +
          '<div style="text-align:center">' + endorseBadge + '</div>' +
        '</div>' +
        '<div class="skill-desc">' + skill_t('skills.' + sk.id + '.desc') + '</div>' +
        '<div class="xp-bar-wrap" data-i18n-tip="skills_page.progress_hint" data-tip-desc="' + skill_t('skills_page.progress_hint') + '">' +
          '<div class="xp-bar-fill" style="width:' + pct + '%"></div>' +
          '<div class="xp-bar-text">' + (next ? skill_t('skills_page.next_stage', { stage: skill_stageName(next.stage), level: next.from_level }) : skill_t('skills_page.at_master')) + '</div>' +
        '</div>' +
        (skillWhy[sk.id]
          ? '<div class="skill-why" data-i18n-tip="skills_page.last_practice_hint" data-tip-desc="' + skill_t('skills_page.last_practice_hint') + '">' +
            skillWhy[sk.id].map(function (l) { return '<div>' + l + '</div>'; }).join('') + '</div>'
          : '') +
        (guideSlug
          ? '<div class="skill-guide">' + skill_t('skills_page.guide_for_stage', { title: SKILL_GUIDE_TITLES[guideSlug] || guideSlug }) +
            ' <a class="xp-btn" href="/library#' + guideSlug + '" data-i18n-tip="skills_page.guide_open_web_hint" data-tip-desc="' + skill_t('skills_page.guide_open_web_hint') + '">' +
            skill_t('skills_page.guide_open') + '</a></div>'
          : '') +
        (done.length
          ? '<div class="skill-can-head">' + skill_t('skills_page.can_now') + '</div>' + skill_list(done, 'done')
          : '<div class="skill-can-head">' + skill_t('skills_page.untrained_web') + '</div>') +
        (next ? '<div class="skill-can-head">' + skill_t('skills_page.can_next', { stage: skill_stageName(next.stage) }) + '</div>' + skill_list(next.can, 'next') : '') +
        '<div class="xp-actions">' +
          '<label class="xp-current">' + skill_t('skills_page.my_stage') + ' ' +
            '<select data-i18n-tip="skills_page.my_stage_hint" data-tip-desc="' + skill_t('skills_page.my_stage_hint') + '"' +
            ' data-on-change="skill_setStage(' + hosArg(sk.id) + ', Number(this.value), document.getElementById(' + hosArg('card-' + sk.id) + '))">' + stageOptions + '</select>' +
          '</label>' +
          practise(10) + practise(50) +
          '<button class="xp-btn" style="margin-left:auto;font-size:0.65rem" data-i18n-tip="skills_page.find_hint" data-tip-desc="' + skill_t('skills_page.find_hint') + '"' +
          ' data-on-click="skill_findUsers(' + hosArg(sk.id) + ', ' + hosArg(name) + ')">' + hosIcon('users', 14) + ' ' + skill_t('skills_page.find') + '</button>' +
        '</div>' +
      '</div>';
    }

    function skill_refreshCard(skillId, cardEl) {
      var sk = skill_getById(skillId);
      if (!sk || !cardEl) return;
      cardEl.outerHTML = skill_buildCard(sk);
    }

    /** Read the base set: the rows, the family order and every skill's ladder. */
    function skill_loadBaseSet() {
      var text = function (u) { return fetch(u).then(function (r) { return r.ok ? r.text() : ''; }); };
      return Promise.all([text('/data/skills/skills.csv'), text('/data/skills/families.json'), text('/data/skills/stages.json')])
        .then(function (parts) {
          var stages = JSON.parse(parts[2] || '{}');
          SKILL_FAMILIES = (JSON.parse(parts[1] || '{}').families || []).map(function (f) { return f.id; });
          SKILL_STAGES = (stages.stages || []).map(function (s) { return s.id; });
          var lines = parts[0].split(/\r?\n/).filter(function (l) { return l && l.charAt(0) !== '#'; });
          var head = (lines.shift() || '').split(',');
          var col = function (name) { return head.indexOf(name); };
          SKILLS = lines.map(function (l) {
            var c = l.split(',');
            return { id: c[col('id')], family: c[col('family')], base: Number(c[col('xp_per_level')]) || SKILL_XP_BASE,
                     ladder: (stages.ladders || {})[c[col('id')]] || [] };
          }).sort(function (a, b) { return skill_t('skills.' + a.id + '.name').localeCompare(skill_t('skills.' + b.id + '.name')); });
          // Practice quality and each stage's guide (rung 2); the Library's titles for the guides.
          return Promise.all([text('/data/skills/practice_quality.json'), text('/data/skills/guides.json'), text('/data/library/index.json')]);
        })
        .then(function (more) {
          if (!more) return;
          try { SKILL_QUALITY = (JSON.parse(more[0] || '{}').levels || {}).simplified || null; } catch (e) { SKILL_QUALITY = null; }
          try { SKILL_GUIDES = JSON.parse(more[1] || '{}').guides || {}; } catch (e) { SKILL_GUIDES = {}; }
          try {
            var walk = function (o) {
              if (Array.isArray(o)) { o.forEach(walk); return; }
              if (!o || typeof o !== 'object') return;
              if (o.file && o.title) SKILL_GUIDE_TITLES[o.file.replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase()] = o.title;
              Object.keys(o).forEach(function (k) { if (typeof o[k] === 'object') walk(o[k]); });
            };
            walk(JSON.parse(more[2] || '{}'));
          } catch (e) { /* no titles: the slug stands in */ }
          skill_renderHow();
        })
        .catch(function () { /* offline: the tab stays on its empty state */ });
    }

    var skillToastTimer = null;
    function skill_showToast(msg) {
      var t = document.getElementById('skill-toast');
      if (!t) return;
      t.textContent = msg;
      t.classList.add('show');
      clearTimeout(skillToastTimer);
      skillToastTimer = setTimeout(function() { t.classList.remove('show'); }, 2500);
    }

    /** Show a modal listing community members who have this skill */
    function skill_findUsers(skillId, skillName) {
      var overlay = document.createElement('div');
      overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.8);z-index:var(--z-shade);display:flex;align-items:center;justify-content:center;padding:var(--space-xl);';
      overlay.innerHTML =
        '<div style="background:var(--pg-card-bg);border:1px solid var(--pg-line);border-radius:12px;padding:var(--space-2xl);width:100%;max-width:420px;max-height:80vh;overflow-y:auto;font-family:\'Segoe UI\',system-ui,sans-serif;color:var(--pg-card-ink);">' +
          '<h2 style="font-size:0.95rem;font-weight:700;color:var(--accent);margin-bottom:var(--space-sm)">' + hosIcon('users', 16) + ' ' + skillName + '</h2>' +
          '<p style="font-size:0.73rem;color:var(--pg-dim-ink);margin-bottom:var(--space-xl)">Community members with this skill (from relay database):</p>' +
          '<div id="skill-find-results" style="font-size:0.82rem;">Loading…</div>' +
          '<div style="display:flex;justify-content:flex-end;margin-top:var(--space-xl);">' +
            '<button data-on-click="this.closest(\'[style*=fixed]\').remove()" style="background:none;border:1px solid var(--pg-quiet-line);color:var(--pg-quiet-ink);border-radius:7px;padding:var(--space-md) var(--space-xl);font-size:0.82rem;cursor:pointer;">Close</button>' +
          '</div>' +
        '</div>';
      document.body.appendChild(overlay);
      overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });

      var resultsEl = document.getElementById('skill-find-results');
      fetch('/api/skills/search?skill=' + encodeURIComponent(skillId) + '&min_level=1&limit=20')
        .then(function(res) { if (!res.ok) throw new Error('HTTP ' + res.status); return res.json(); })
        .then(function(users) {
          if (!users || users.length === 0) {
            resultsEl.innerHTML = '<span style="color:var(--pg-faint-ink)">No community members found with this skill yet. Be the first to level it up!</span>';
            return;
          }
          var lvColors = ['#555','#888','#aaa','#6ab0f5','#4ec87a','#f0c040','#f0a500','#e57c00','#e53935','#c62828','#9c27b0'];
          resultsEl.innerHTML = users.map(function(u) {
            var lv = u.level || 0;
            var c = lvColors[Math.min(lv, lvColors.length-1)];
            return '<div style="display:flex;align-items:center;gap:var(--space-md);padding:var(--space-sm) 0;border-bottom:1px solid var(--pg-row-line);">' +
              '<div style="width:28px;height:28px;border-radius:50%;background:var(--pg-chip-bg);display:flex;align-items:center;justify-content:center;font-size:0.75rem;font-weight:700;flex-shrink:0;color:' + c + '">' + lv + '</div>' +
              '<span style="flex:1;font-weight:600">' + (u.display_name || u.public_key.slice(0,12) + '…') + '</span>' +
              '<span style="font-size:0.68rem;color:var(--pg-faint-ink)">' + (u.reality_xp|0) + ' XP</span>' +
            '</div>';
          }).join('');
        })
        .catch(function(e) {
          resultsEl.innerHTML = '<span style="color:#e55">Error: ' + e.message + '</span>';
        });
    }

    /** Hook called by app.js when relay sends back endorsement counts */
    window.onSkillEndorsementsReceived = function(data) {
      if (!data || !Array.isArray(data.endorsements)) return;
      skillEndorsements = {};
      for (var i = 0; i < data.endorsements.length; i++) {
        var e = data.endorsements[i];
        skillEndorsements[e.skill_id.toLowerCase()] = { count: e.count, last_endorser: e.last_endorser };
      }
      if (skill_activeFamily) skill_renderGrid(skill_activeFamily);
    };

    /** Request endorsement counts from relay if connected */
    function skill_loadEndorsements() {
      if (typeof requestSkillEndorsements === 'function') {
        requestSkillEndorsements();
      }
    }

    // ── Relay sync, push skill levels to server when user is signed in ──
    (function initSkillRelaySync() {
      var raw = localStorage.getItem('humanity_key_backup');
      if (!raw) return;
      var pub, name;
      try { var b = JSON.parse(raw); pub = b.publicKeyHex; name = localStorage.getItem('humanity_name') || 'user'; }
      catch(e) { return; }
      if (!pub) return;

      var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
      var ws = new WebSocket(proto + '//' + location.host + '/ws');

      ws.addEventListener('open', function() {
        ws.send(JSON.stringify({ type: 'identify', public_key: pub, display_name: name }));
      });

      ws.addEventListener('message', function(e) {
        try {
          var m = JSON.parse(e.data);
          if (m.type === 'peer_list') {
            var data = skillData || {};
            var entries = Object.entries(data);
            for (var i = 0; i < entries.length; i++) {
              var sid = entries[i][0], sk = entries[i][1];
              if (!sk || !sk.level) continue;
              ws.send(JSON.stringify({
                type: 'skill_update',
                skill_id: sid,
                reality_xp: sk.xp || 0,
                fantasy_xp: 0,
                level: sk.level,
              }));
            }
            setTimeout(function() { ws.close(); }, 500);
          }
        } catch(err) {}
      });

      ws.addEventListener('error', function() { ws.close(); });
    })();

    // ── Skills init ──
    skill_loadData();
    skill_loadBaseSet().then(function () {
      skill_renderSidebar();
      var last = null;
      try { last = localStorage.getItem('hos_skills_last_family'); } catch (e) { /* storage blocked */ }
      if (SKILL_FAMILIES.length) skill_selectFamily(last && SKILL_FAMILIES.indexOf(last) >= 0 ? last : SKILL_FAMILIES[0]);
    });

    // Try loading endorsements after connection delay
    setTimeout(skill_loadEndorsements, 3000);
