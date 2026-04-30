/* Resources page — context-aware curated links (Real = real-world help, Sim = in-game guides) */
(function() {
  'use strict';

  // ── Real-world resources (shown in Real mode) ──
  var realResources = {
    categories: [
      {
        name: 'Education',
        icon: 'book',
        resources: [
          { name: 'Khan Academy', url: 'https://www.khanacademy.org', desc: 'Free courses in math, science, computing, history, and more for all ages.' },
          { name: 'Coursera', url: 'https://www.coursera.org', desc: 'University-level courses from top institutions, many available for free.' },
          { name: 'MIT OpenCourseWare', url: 'https://ocw.mit.edu', desc: 'Free lecture notes, exams, and videos from MIT.' },
          { name: 'Library Genesis', url: 'https://libgen.is', desc: 'Search engine for free access to academic papers, books, and articles.' },
          { name: 'edX', url: 'https://www.edx.org', desc: 'Free online courses from Harvard, MIT, and other leading universities.' }
        ]
      },
      {
        name: 'Health',
        icon: 'heart',
        resources: [
          { name: 'CDC', url: 'https://www.cdc.gov', desc: 'Disease prevention, health information, and public health guidance.' },
          { name: 'WHO', url: 'https://www.who.int', desc: 'Global health information, disease outbreak tracking, and health guidelines.' },
          { name: 'MedlinePlus', url: 'https://medlineplus.gov', desc: 'Trusted health information from the National Library of Medicine.' },
          { name: 'GoodRx', url: 'https://www.goodrx.com', desc: 'Compare prescription drug prices and find coupons at local pharmacies.' },
          { name: 'Planned Parenthood', url: 'https://www.plannedparenthood.org', desc: 'Reproductive health care, sex education, and health information.' }
        ]
      },
      {
        name: 'Legal Aid',
        icon: 'shield',
        resources: [
          { name: 'Legal Aid Society', url: 'https://www.legalaidnyc.org', desc: 'Free legal services for low-income individuals and families.' },
          { name: 'LawHelp.org', url: 'https://www.lawhelp.org', desc: 'Find free legal help programs in your state.' },
          { name: 'ACLU', url: 'https://www.aclu.org', desc: 'Civil liberties defense, know your rights resources.' },
          { name: 'Nolo', url: 'https://www.nolo.com', desc: 'Free legal information, articles, and DIY legal guides.' }
        ]
      },
      {
        name: 'Housing',
        icon: 'home',
        resources: [
          { name: 'HUD.gov', url: 'https://www.hud.gov', desc: 'Federal housing assistance programs, rental help, and homebuyer resources.' },
          { name: 'National Low Income Housing Coalition', url: 'https://nlihc.org', desc: 'Affordable housing advocacy and rental assistance locator.' },
          { name: 'Habitat for Humanity', url: 'https://www.habitat.org', desc: 'Affordable homeownership programs and volunteer building opportunities.' },
          { name: '211.org', url: 'https://www.211.org', desc: 'Local resources for housing, utilities, food, and more. Dial 2-1-1.' }
        ]
      },
      {
        name: 'Food',
        icon: 'leaf',
        resources: [
          { name: 'Feeding America', url: 'https://www.feedingamerica.org', desc: 'Find local food banks, pantries, and meal programs near you.' },
          { name: 'SNAP Benefits', url: 'https://www.fns.usda.gov/snap', desc: 'Apply for food assistance (food stamps) through the USDA.' },
          { name: 'No Kid Hungry', url: 'https://www.nokidhungry.org', desc: 'Free meals for children, school breakfast programs, and summer meal sites.' },
          { name: 'World Food Programme', url: 'https://www.wfp.org', desc: 'Global hunger relief, emergency food assistance, and development programs.' }
        ]
      },
      {
        name: 'Employment',
        icon: 'tasklist',
        resources: [
          { name: 'Indeed', url: 'https://www.indeed.com', desc: 'Job search engine aggregating listings from thousands of sources.' },
          { name: 'USAJobs', url: 'https://www.usajobs.gov', desc: 'Official job site for the U.S. federal government.' },
          { name: 'LinkedIn', url: 'https://www.linkedin.com', desc: 'Professional networking, job search, and career development.' },
          { name: 'CareerOneStop', url: 'https://www.careeronestop.org', desc: 'Free career exploration, training finder, and job search tools from the DOL.' }
        ]
      },
      {
        name: 'Mental Health',
        icon: 'profile',
        resources: [
          { name: '988 Suicide & Crisis Lifeline', url: 'https://988lifeline.org', desc: 'Call or text 988 for free, confidential support 24/7.' },
          { name: 'NAMI', url: 'https://www.nami.org', desc: 'Mental health education, support groups, and advocacy resources.' },
          { name: 'Crisis Text Line', url: 'https://www.crisistextline.org', desc: 'Text HOME to 741741 for free crisis counseling via text message.' },
          { name: 'BetterHelp', url: 'https://www.betterhelp.com', desc: 'Online therapy and counseling with licensed professionals.' },
          { name: 'Open Path Collective', url: 'https://openpathcollective.org', desc: 'Affordable therapy sessions ($30-$80) with licensed therapists.' }
        ]
      },
      {
        name: 'Emergency',
        icon: 'warning',
        resources: [
          { name: 'FEMA', url: 'https://www.fema.gov', desc: 'Disaster assistance, emergency preparedness, and recovery programs.' },
          { name: 'Red Cross', url: 'https://www.redcross.org', desc: 'Emergency assistance, disaster relief, and blood donation.' },
          { name: 'Ready.gov', url: 'https://www.ready.gov', desc: 'Build an emergency kit, make a plan, and stay informed about threats.' },
          { name: 'Disaster Distress Helpline', url: 'https://www.samhsa.gov/find-help/disaster-distress-helpline', desc: 'Call 1-800-985-5990 for crisis counseling after disasters.' }
        ]
      },
      {
        name: 'Financial',
        icon: 'coin',
        resources: [
          { name: 'Consumer Financial Protection Bureau', url: 'https://www.consumerfinance.gov', desc: 'Financial education, complaint filing, and consumer protection.' },
          { name: 'IRS Free File', url: 'https://www.irs.gov/filing/free-file-do-your-federal-taxes-for-free', desc: 'Free federal tax filing for qualifying individuals.' },
          { name: 'Benefits.gov', url: 'https://www.benefits.gov', desc: 'Find government benefits you may be eligible for.' },
          { name: 'MyMoney.gov', url: 'https://www.mymoney.gov', desc: 'Federal financial literacy resources: earning, saving, investing, spending.' }
        ]
      },
      {
        name: 'Technology',
        icon: 'dev',
        resources: [
          { name: 'freeCodeCamp', url: 'https://www.freecodecamp.org', desc: 'Free coding bootcamp with certifications in web dev, data science, and more.' },
          { name: 'EveryoneOn', url: 'https://www.everyoneon.org', desc: 'Low-cost internet and affordable computers for qualifying households.' },
          { name: 'Digital Literacy Resources', url: 'https://www.digitallearn.org', desc: 'Free courses on using technology, the internet, and digital tools.' },
          { name: 'Open Source Guides', url: 'https://opensource.guide', desc: 'Learn how to contribute to and maintain open source projects.' },
          { name: 'The Odin Project', url: 'https://www.theodinproject.com', desc: 'Free full-stack web development curriculum with projects.' }
        ]
      }
    ]
  };

  // ── Sim-mode resources (in-game guides and wiki) ──
  var simResources = {
    categories: [
      {
        name: 'Getting Started',
        icon: 'home',
        resources: [
          { name: 'Tutorial Walkthrough', url: '#tutorial', desc: 'Step-by-step guide through the opening tutorial quests.' },
          { name: 'Controls Reference', url: '#controls', desc: 'Keyboard, mouse, and gamepad bindings for all actions.' },
          { name: 'UI Guide', url: '#ui-guide', desc: 'Understanding the HUD, inventory, and menu systems.' },
          { name: 'First Steps Checklist', url: '#first-steps', desc: 'What to do in your first 30 minutes: gather, build, survive.' }
        ]
      },
      {
        name: 'Survival',
        icon: 'heart',
        resources: [
          { name: 'Farming Guide', url: '#farming', desc: 'Planting, watering, harvesting, and crop rotation for 23 crops.' },
          { name: 'Crafting Recipes', url: '#crafting', desc: 'All 35 recipes: materials, tools, and advanced items.' },
          { name: 'Weather & Seasons', url: '#weather', desc: '7 weather conditions, seasonal effects on farming and travel.' },
          { name: 'Resource Gathering', url: '#resources', desc: 'Mining asteroids, harvesting planets, and material processing.' }
        ]
      },
      {
        name: 'Building',
        icon: 'inventory',
        resources: [
          { name: 'Construction System', url: '#construction', desc: 'Blueprints, snap grid placement, and structural integrity.' },
          { name: 'Ship Building', url: '#ships', desc: 'Ship layouts, room types, deck plans, and fleet management.' },
          { name: 'Base Planning', url: '#bases', desc: 'Optimal base layouts for defense, farming, and production.' },
          { name: 'Power & Systems', url: '#power', desc: 'Reactor types, power grids, and life support configuration.' }
        ]
      },
      {
        name: 'Exploration',
        icon: 'map',
        resources: [
          { name: 'Planet Guide', url: '#planets', desc: 'Earth, Mars, Moon, and procedural worlds: biomes, resources, hazards.' },
          { name: 'Asteroid Mining', url: '#asteroids', desc: 'C/S/M-type asteroids, ore veins, and mining techniques.' },
          { name: 'Navigation', url: '#navigation', desc: 'Star maps, waypoints, and orbital mechanics basics.' },
          { name: 'Vehicle Guide', url: '#vehicles', desc: 'Mechs, rovers, and ship piloting controls and upgrades.' }
        ]
      },
      {
        name: 'Combat & Skills',
        icon: 'shield',
        resources: [
          { name: 'Combat Guide', url: '#combat', desc: 'Weapons, armor, tactics, and enemy behavior patterns.' },
          { name: 'Skills Overview', url: '#skills', desc: '20 skills, XP curves, level-up rewards, and training tips.' },
          { name: 'AI Behaviors', url: '#ai', desc: 'Understanding NPC types: passive, aggressive, herd, predator, guard.' },
          { name: 'Quest Guide', url: '#quests', desc: 'Main quest chains, side quests, objectives, and rewards.' }
        ]
      },
      {
        name: 'Economy & Trading',
        icon: 'market',
        resources: [
          { name: 'Trading Guide', url: '#trading', desc: 'P2P trading, escrow system, and marketplace listings.' },
          { name: 'Economy Overview', url: '#economy', desc: 'Supply, demand, pricing, and resource flow between players.' },
          { name: 'Inventory Management', url: '#inventory', desc: 'Item stacking, storage optimization, and transfer systems.' }
        ]
      },
      {
        name: 'Modding',
        icon: 'dev',
        resources: [
          { name: 'Mod Getting Started', url: '#mod-intro', desc: 'How to create and install mods using the data directory.' },
          { name: 'Data File Reference', url: '#data-files', desc: 'CSV, TOML, RON, and JSON format specifications for game data.' },
          { name: 'Asset Creation', url: '#asset-creation', desc: 'Creating models (GLTF/GLB), textures, and audio for mods.' },
          { name: 'Mod Manifest', url: '#mod-manifest', desc: 'Manifest format, load order, and data override system.' }
        ]
      }
    ]
  };

  var activeCategory = null; // null = show all

  function getContext() {
    return window.hos_context || 'real';
  }

  function getData() {
    return getContext() === 'sim' ? simResources : realResources;
  }

  function updateHeader() {
    var ctx = getContext();
    var title = document.getElementById('res-title');
    var subtitle = document.getElementById('res-subtitle');
    if (ctx === 'sim') {
      title.textContent = 'Game Guides';
      subtitle.textContent = 'In-game wiki, tutorials, and reference guides for HumanityOS simulation.';
    } else {
      title.textContent = 'Resources';
      subtitle.textContent = 'Curated links to real-world help: education, health, legal, housing, and more.';
    }
  }

  function renderFilterBar() {
    var bar = document.getElementById('filter-bar');
    if (!bar) return;
    var data = getData();

    var html = '<button class="filter-btn' + (activeCategory === null ? ' active' : '') + '" onclick="window._resFilter(null)">All</button>';
    for (var i = 0; i < data.categories.length; i++) {
      var cat = data.categories[i];
      var isActive = activeCategory === cat.name;
      html += '<button class="filter-btn' + (isActive ? ' active' : '') + '" onclick="window._resFilter(\'' + cat.name.replace(/'/g, "\\'") + '\')">' + cat.name + '</button>';
    }
    bar.innerHTML = html;
  }

  function render(filter) {
    var container = document.getElementById('res-list');
    if (!container) return;

    var data = getData();
    var q = (filter || '').toLowerCase().trim();
    var html = '';
    var totalShown = 0;

    for (var i = 0; i < data.categories.length; i++) {
      var cat = data.categories[i];

      // Category filter
      if (activeCategory && cat.name !== activeCategory) continue;

      // Search filter
      var matching = cat.resources.filter(function(r) {
        if (!q) return true;
        return r.name.toLowerCase().indexOf(q) !== -1 ||
               r.desc.toLowerCase().indexOf(q) !== -1 ||
               cat.name.toLowerCase().indexOf(q) !== -1 ||
               r.url.toLowerCase().indexOf(q) !== -1;
      });

      if (matching.length === 0) continue;
      totalShown += matching.length;

      var iconHtml = window.hosIcon ? hosIcon(cat.icon || 'globe', 18) : '';

      html += '<div class="cat-section">';
      html += '<div class="cat-header" onclick="this.nextElementSibling.style.display=this.nextElementSibling.style.display===\'none\'?\'grid\':\'none\';this.querySelector(\'.cat-arrow\').classList.toggle(\'collapsed\')">';
      html += '<span class="cat-arrow">&#9660;</span> ';
      html += '<span>' + iconHtml + '</span>';
      html += '<h2>' + esc(cat.name) + '</h2>';
      html += '<span class="cat-count">(' + matching.length + ')</span>';
      html += '</div>';

      html += '<div class="res-grid">';
      for (var j = 0; j < matching.length; j++) {
        var r = matching[j];
        var isExternal = r.url.indexOf('http') === 0;
        var targetAttr = isExternal ? ' target="_blank" rel="noopener noreferrer"' : '';
        html += '<div class="res-card">';
        html += '<div class="res-name"><a href="' + esc(r.url) + '"' + targetAttr + '>' + esc(r.name) + '</a></div>';
        html += '<div class="res-desc">' + esc(r.desc) + '</div>';
        if (isExternal) {
          html += '<div class="res-tags"><span class="res-tag">' + esc(new URL(r.url).hostname.replace('www.', '')) + '</span></div>';
        }
        html += '</div>';
      }
      html += '</div></div>';
    }

    if (totalShown === 0) {
      html = '<div class="no-results">No resources found matching your search.</div>';
    }

    container.innerHTML = html;
  }

  function esc(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  // ── Public filter handler ──
  window._resFilter = function(cat) {
    activeCategory = cat;
    renderFilterBar();
    render(document.getElementById('res-search').value);
  };

  // ── Search input ──
  var searchEl = document.getElementById('res-search');
  if (searchEl) {
    searchEl.addEventListener('input', function() {
      render(this.value);
    });
  }

  // ── Context change listener ──
  window.addEventListener('hos-context-change', function() {
    activeCategory = null;
    updateHeader();
    renderFilterBar();
    render(searchEl ? searchEl.value : '');
  });

  // ── Initial render ──
  updateHeader();
  renderFilterBar();
  render('');

})();
