// Moved out of web/pages/dev.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // ═══════════════════════════════════════════════════════════════════════
    // TAB SWITCHING
    // ═══════════════════════════════════════════════════════════════════════
    document.getElementById('tab-bar').addEventListener('click', function(e) {
      var btn = e.target.closest('.tab-btn');
      if (!btn) return;
      document.querySelectorAll('.tab-btn').forEach(function(b) { b.classList.remove('active'); });
      document.querySelectorAll('.tab-panel').forEach(function(p) { p.classList.remove('active'); });
      btn.classList.add('active');
      document.getElementById('tab-' + btn.dataset.tab).classList.add('active');
    });

    // ═══════════════════════════════════════════════════════════════════════
    // ICONS TAB, All hosIcon icons
    // ═══════════════════════════════════════════════════════════════════════
    (function() {
      var names = typeof hosIconNames === 'function' ? hosIconNames() : [];
      document.getElementById('icon-count').textContent = '(' + names.length + ')';

      var grid = document.getElementById('all-icons-grid');
      var html = '';
      names.forEach(function(name) {
        html += '<div class="all-icon-cell" data-name="' + name + '">' +
          hosIcon(name, 32) +
          '<span class="icon-label">' + name + '</span></div>';
      });
      grid.innerHTML = html;

      grid.addEventListener('click', function(e) {
        var cell = e.target.closest('.all-icon-cell');
        if (!cell) return;
        var name = cell.dataset.name;
        var text = "hosIcon('" + name + "')";
        navigator.clipboard.writeText(text).then(function() {
          cell.classList.add('copied');
          cell.querySelector('.icon-label').textContent = 'Copied!';
          setTimeout(function() {
            cell.classList.remove('copied');
            cell.querySelector('.icon-label').textContent = name;
          }, 1200);
        });
      });
    })();

    // ── Stroke weight comparison (same as before) ──
    var WEIGHT_ICONS = {
      download: '<line x1="24" y1="6" x2="24" y2="32"/><polyline points="16,24 24,32 32,24"/><polyline points="8,36 8,42 40,42 40,36"/>',
      cart: '<circle cx="18" cy="42" r="3"/><circle cx="36" cy="42" r="3"/><path d="M2,4 H10 L16,32 H38 L44,12 H14"/>',
      chat: '<path d="M6,6 H42 V32 H18 L10,40 V32 H6 Z"/>',
      home: '<polyline points="6,24 24,6 42,24"/><rect x="14" y="24" width="20" height="18"/><rect x="20" y="30" width="8" height="12"/>',
      gear: '<circle cx="24" cy="24" r="8"/><path d="M24,4 L26,10 A14,14 0 0 1 34,14 L40,12 L36,20 A14,14 0 0 1 38,24 L44,24 L38,28 A14,14 0 0 1 36,32 L40,36 L34,34 A14,14 0 0 1 26,38 L24,44 L22,38 A14,14 0 0 1 14,34 L8,36 L12,32 A14,14 0 0 1 10,28 L4,24 L10,20 A14,14 0 0 1 12,14 L8,12 L14,14 A14,14 0 0 1 22,10 Z"/>',
      map: '<polygon points="4,8 16,14 32,8 44,14 44,40 32,34 16,40 4,34"/><line x1="16" y1="14" x2="16" y2="40"/><line x1="32" y1="8" x2="32" y2="34"/>',
      globe: '<circle cx="24" cy="24" r="20"/><ellipse cx="24" cy="24" rx="10" ry="20"/><line x1="4" y1="24" x2="44" y2="24"/>',
      search: '<circle cx="20" cy="20" r="14"/><line x1="30" y1="30" x2="42" y2="42"/>',
    };
    var weightIconNames = Object.keys(WEIGHT_ICONS);

    function makeSVG(name, size, weight) {
      return '<svg viewBox="0 0 48 48" width="' + size + '" height="' + size +
        '" style="stroke-width:' + weight + ';stroke:currentColor;fill:none;stroke-linecap:round;stroke-linejoin:round">' +
        WEIGHT_ICONS[name] + '</svg>';
    }

    function populateRow(row, size, weight) {
      var label = row.querySelector('.row-label');
      var html = label.outerHTML;
      weightIconNames.forEach(function(name) {
        html += '<div class="icon-cell">' + makeSVG(name, size, weight) + '<span class="icon-name">' + name + '</span></div>';
      });
      row.innerHTML = html;
    }

    function renderWeightRows() {
      var slider = document.getElementById('weight-slider');
      var sizeSelect = document.getElementById('size-select');
      var liveWeight = parseFloat(slider.value);
      var size = parseInt(sizeSelect.value);
      document.getElementById('weight-val').textContent = liveWeight;
      document.getElementById('live-weight-label').textContent = liveWeight;
      populateRow(document.getElementById('live-row'), size, liveWeight);
      document.querySelectorAll('.icon-row[data-weight]').forEach(function(row) {
        populateRow(row, size, parseFloat(row.dataset.weight));
      });
    }
    document.getElementById('weight-slider').addEventListener('input', renderWeightRows);
    document.getElementById('size-select').addEventListener('change', renderWeightRows);
    renderWeightRows();

    // ═══════════════════════════════════════════════════════════════════════
    // ASSETS TAB, Dynamic asset browser (fetches from /api/asset-manifest)
    // ═══════════════════════════════════════════════════════════════════════
    (function() {
      /* Assets loaded dynamically from GET /api/asset-manifest.
         Categories and file lists come from the server. */
      var ASSETS = [];
      var assetCategories = {};

      // ── State ──
      var activeCat = 'all';
      var activeSubFilter = 'all';
      var searchQuery = '';
      var viewMode = 'grid';
      var thumbSize = 120;

      // Shared audio element for previews
      var sharedAudio = new Audio();
      var currentlyPlayingPath = null;

      // ── Sub-filter definitions per category (file-type splits) ──
      var SUB_FILTERS = {
        'icons': { 'all': 'All', 'svg': 'SVG', 'png': 'PNG' },
        'audio': { 'all': 'All', 'sfx': 'SFX', 'music': 'Music', 'dialog': 'Dialog' },
      };

      // ── Helpers ──
      // An asset's path, copied by a press on it (was an inline onclick): it reads Copied!, and with
      // `restore` shows the path again after 1.2 s.
      window.devCopyPath = function (el, path, restore) {
        navigator.clipboard.writeText(path);
        el.textContent = window.i18n ? i18n.t('dev.asset_path_copied') : '';
        if (restore) setTimeout(function () { el.textContent = path; }, 1200);
      };
      function formatSize(bytes) {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / 1048576).toFixed(1) + ' MB';
      }

      function getExt(name) {
        return name.split('.').pop().toLowerCase();
      }

      function showToast(msg) {
        var t = document.getElementById('toast');
        t.textContent = msg;
        t.classList.add('show');
        setTimeout(function() { t.classList.remove('show'); }, 2000);
      }

      function copyToClipboard(text) {
        navigator.clipboard.writeText(text).then(function() {
          showToast('Copied: ' + text);
        });
      }

      // ── File type classification ──
      var IMAGE_EXTS = ['png','jpg','jpeg','gif','webp','svg','ico'];
      var AUDIO_EXTS = ['ogg','flac','wav','mp3'];
      var MODEL_EXTS = ['glb','gltf'];
      var SHADER_EXTS = ['wgsl'];
      var FONT_EXTS = ['ttf','woff2','woff','otf'];

      function isImageExt(ext) { return IMAGE_EXTS.indexOf(ext) !== -1; }
      function isAudioExt(ext) { return AUDIO_EXTS.indexOf(ext) !== -1; }
      function isModelExt(ext) { return MODEL_EXTS.indexOf(ext) !== -1; }
      function isShaderExt(ext) { return SHADER_EXTS.indexOf(ext) !== -1; }
      function isFontExt(ext) { return FONT_EXTS.indexOf(ext) !== -1; }

      // ── Counts per category ──
      function catCounts() {
        var counts = { all: ASSETS.length };
        ASSETS.forEach(function(a) {
          counts[a.c] = (counts[a.c] || 0) + 1;
        });
        return counts;
      }

      // ── Filter logic ──
      function getFiltered() {
        var q = searchQuery.toLowerCase();
        return ASSETS.filter(function(a) {
          var catMatch = activeCat === 'all' || a.c === activeCat;
          var searchMatch = !q || a.n.toLowerCase().indexOf(q) !== -1 || a.p.toLowerCase().indexOf(q) !== -1;
          if (!catMatch || !searchMatch) return false;

          // Sub-filter logic
          if (activeSubFilter !== 'all') {
            var ext = getExt(a.n);
            if (activeCat === 'icons') {
              if (activeSubFilter === 'svg' && ext !== 'svg') return false;
              if (activeSubFilter === 'png' && ext !== 'png') return false;
            } else if (activeCat === 'audio') {
              var sub = (a.sub_category || '').toLowerCase();
              if (activeSubFilter !== ext && activeSubFilter !== sub) return false;
            }
          }

          return true;
        });
      }

      // ── Total size of filtered set ──
      function totalSize(list) {
        var sum = 0;
        list.forEach(function(a) { sum += a.s; });
        return sum;
      }

      // ── Update tab count display ──
      function updateAssetCount() {
        document.getElementById('asset-count').textContent = '(' + ASSETS.length + ')';
      }

      // ── Build category pills from API response ──
      function buildCategoryPills() {
        var counts = catCounts();
        var filterEl = document.getElementById('asset-filter');
        var html = '<button class="filter-pill' + (activeCat === 'all' ? ' active' : '') +
          '" data-cat="all">All <span class="pill-count">' + ASSETS.length + '</span></button>';

        var catIds = Object.keys(assetCategories);
        catIds.forEach(function(catId) {
          var cnt = counts[catId] || 0;
          if (cnt === 0) return;
          var label = assetCategories[catId] || catId;
          html += '<button class="filter-pill' + (catId === activeCat ? ' active' : '') +
            '" data-cat="' + catId + '">' + label +
            ' <span class="pill-count">' + cnt + '</span></button>';
        });

        filterEl.innerHTML = html;
      }

      // ── Build sub-filter pills (SVG/PNG for icons, SFX/Music/Dialog for audio) ──
      function renderSubFilters() {
        var container = document.getElementById('asset-sub-filter');
        if (!container) return;
        var subs = SUB_FILTERS[activeCat];
        if (!subs) {
          container.innerHTML = '';
          return;
        }
        var html = '';
        Object.keys(subs).forEach(function(key) {
          html += '<button class="sub-pill' + (key === activeSubFilter ? ' active' : '') +
            '" data-sub="' + key + '">' + subs[key] + '</button>';
        });
        container.innerHTML = html;
      }

      // ── Build preview HTML per asset type ──
      function buildPreviewHTML(asset, previewH) {
        var ext = getExt(asset.n);

        if (isImageExt(ext)) {
          return '<img src="' + asset.p + '" alt="' + asset.n + '" loading="lazy">';
        }

        if (isAudioExt(ext)) {
          var playing = currentlyPlayingPath === asset.p;
          return '<button class="audio-play-btn" data-audio-path="' + asset.p + '" title="Play audio">' +
            (playing ? '<svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>'
                     : '<svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><polygon points="5,3 19,12 5,21"/></svg>') +
            '</button>';
        }

        if (isModelExt(ext)) {
          return '<div class="model-icon">' +
            '<svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">' +
            '<path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>' +
            '<span>3D Model</span></div>';
        }

        if (isShaderExt(ext)) {
          return '<div class="code-icon" data-shader-path="' + asset.p + '" style="cursor:pointer" title="Click to view source">' +
            '<svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">' +
            '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>' +
            '<span>Shader</span></div>';
        }

        if (isFontExt(ext)) {
          var fontId = 'font-preview-' + asset.n.replace(/[^a-zA-Z0-9]/g, '_');
          var style = document.getElementById(fontId);
          if (!style) {
            style = document.createElement('style');
            style.id = fontId;
            style.textContent = '@font-face { font-family: "' + fontId + '"; src: url("' + asset.p + '"); }';
            document.head.appendChild(style);
          }
          return '<span class="font-preview" style="font-family:\'' + fontId + '\',sans-serif">AaBb</span>';
        }

        return '<span style="font-size:0.7rem;color:var(--text-muted)">' + ext.toUpperCase() + '</span>';
      }

      // ── Render asset grid/list ──
      function renderAssets() {
        var grid = document.getElementById('asset-grid');
        var filtered = getFiltered();

        document.getElementById('asset-stats').textContent =
          filtered.length + ' assets / ' + formatSize(totalSize(filtered));

        grid.className = 'asset-grid' + (viewMode === 'list' ? ' list-view' : '');
        if (viewMode === 'grid') {
          grid.style.gridTemplateColumns = 'repeat(auto-fill, minmax(' + thumbSize + 'px, 1fr))';
        } else {
          grid.style.gridTemplateColumns = '';
        }

        var previewH = viewMode === 'list' ? 48 : Math.round(thumbSize * 0.7);

        var html = '';
        filtered.forEach(function(a, idx) {
          var ext = getExt(a.n);

          html += '<div class="asset-card" data-idx="' + idx + '" data-path="' + a.p + '">';

          // File type badge
          html += '<span class="ext-badge">' + ext.toUpperCase() + '</span>';

          // Actions overlay
          html += '<div class="asset-actions">' +
            '<button class="asset-act-btn" data-action="copy" title="Copy path">C</button>' +
            '<button class="asset-act-btn" data-action="download" title="Download">D</button>' +
            '<button class="asset-act-btn del" data-action="delete" title="Delete">X</button>' +
            '</div>';

          // Preview
          html += '<div class="asset-preview" style="height:' + previewH + 'px">';
          html += buildPreviewHTML(a, previewH);
          html += '</div>';

          // Info
          html += '<div class="asset-info">' +
            '<div class="asset-name">' + a.n + '</div>' +
            '<div class="asset-meta">' +
            '<span>' + (assetCategories[a.c] || a.c) + '</span>' +
            '<span>' + formatSize(a.s) + '</span>' +
            '</div></div></div>';
        });

        grid.innerHTML = html || '<p style="color:var(--text-muted);font-size:0.85rem;padding:16px">No assets match your search.</p>';

        renderSubFilters();
      }

      // ── Detail panel ──
      function showDetail(asset) {
        var overlay = document.getElementById('asset-detail-overlay');
        var panel = document.getElementById('asset-detail-panel');
        var ext = getExt(asset.n);

        var html = '<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:16px">' +
          '<h2 style="margin:0;font-size:1.1rem">' + asset.n + '</h2>' +
          '<button class="demo-btn" data-on-click="document.getElementById(\'asset-detail-overlay\').classList.remove(\'open\')" style="padding:4px 12px;font-size:0.8rem">Close</button>' +
          '</div>';

        if (isImageExt(ext)) {
          html += '<div class="detail-preview"><img src="' + asset.p + '" alt="' + asset.n + '"></div>';
        } else if (isAudioExt(ext)) {
          html += '<div class="detail-preview"><audio controls src="' + asset.p + '" style="width:100%"></audio></div>';
        } else if (isFontExt(ext)) {
          var fontId = 'font-preview-' + asset.n.replace(/[^a-zA-Z0-9]/g, '_');
          html += '<div class="detail-preview"><p style="font-family:\'' + fontId + '\',sans-serif;font-size:2rem">AaBbCcDdEeFf 0123456789</p>' +
            '<p style="font-family:\'' + fontId + '\',sans-serif;font-size:1rem">The quick brown fox jumps over the lazy dog.</p></div>';
        } else if (isShaderExt(ext)) {
          html += '<div class="detail-preview" id="shader-detail-code" style="text-align:left;font-family:monospace;font-size:0.75rem;white-space:pre-wrap;max-height:300px;overflow-y:auto;color:var(--text-muted)">Loading shader source...</div>';
          fetch(asset.p).then(function(r) { return r.text(); }).then(function(code) {
            var el = document.getElementById('shader-detail-code');
            if (el) el.textContent = code;
          }).catch(function() {
            var el = document.getElementById('shader-detail-code');
            if (el) el.textContent = 'Could not load shader source.';
          });
        }

        html += '<div class="detail-row"><span class="detail-label">Filename</span><span class="detail-val">' + asset.n + '</span></div>';
        html += '<div class="detail-row"><span class="detail-label">Path</span><span role="button" tabindex="0" class="detail-val" style="cursor:pointer;color:var(--accent)" data-on-click="devCopyPath(this, ' + hosArg(asset.p) + ', true)">' + asset.p + '</span></div>';
        html += '<div class="detail-row"><span class="detail-label">Category</span><span class="detail-val">' + (assetCategories[asset.c] || asset.c) + '</span></div>';
        html += '<div class="detail-row"><span class="detail-label">File Size</span><span class="detail-val">' + formatSize(asset.s) + ' (' + asset.s.toLocaleString() + ' bytes)</span></div>';
        html += '<div class="detail-row"><span class="detail-label">Type</span><span class="detail-val">' + ext.toUpperCase() + '</span></div>';

        html += '<div class="detail-actions">' +
          '<button class="demo-btn primary" data-on-click="window.open(' + hosArg(asset.p) + ',\'_blank\')">Open Full Size</button>' +
          '<a class="demo-btn" href="' + asset.p + '" download="' + asset.n + '">Download</a>' +
          '<button class="demo-btn" data-on-click="devCopyPath(this, ' + hosArg(asset.p) + ', false)">Copy Path</button>' +
          '</div>';

        panel.innerHTML = html;
        overlay.classList.add('open');
      }

      // ── Wire up filter pills ──
      document.getElementById('asset-filter').addEventListener('click', function(e) {
        var pill = e.target.closest('.filter-pill');
        if (!pill) return;
        activeCat = pill.dataset.cat;
        activeSubFilter = 'all';
        buildCategoryPills();
        renderAssets();
      });

      // ── Wire up sub-filter pills ──
      document.getElementById('asset-sub-filter').addEventListener('click', function(e) {
        var pill = e.target.closest('.sub-pill');
        if (!pill) return;
        activeSubFilter = pill.dataset.sub;
        renderAssets();
      });

      // ── Wire up search ──
      document.getElementById('asset-search').addEventListener('input', function(e) {
        searchQuery = e.target.value;
        renderAssets();
      });

      // ── Wire up view toggle ──
      document.getElementById('view-grid').addEventListener('click', function() {
        viewMode = 'grid';
        this.classList.add('active');
        document.getElementById('view-list').classList.remove('active');
        renderAssets();
      });
      document.getElementById('view-list').addEventListener('click', function() {
        viewMode = 'list';
        this.classList.add('active');
        document.getElementById('view-grid').classList.remove('active');
        renderAssets();
      });

      // ── Wire up size slider ──
      var sizeSlider = document.getElementById('asset-size-slider');
      var sizeVal = document.getElementById('asset-size-val');
      sizeSlider.addEventListener('input', function() {
        thumbSize = parseInt(this.value);
        sizeVal.textContent = thumbSize;
        renderAssets();
      });

      // ── Wire up upload toggle ──
      var uploadZone = document.getElementById('upload-zone');
      document.getElementById('asset-upload-btn').addEventListener('click', function() {
        uploadZone.classList.toggle('visible');
      });

      // ── Upload drop zone interactions ──
      uploadZone.addEventListener('click', function() {
        document.getElementById('upload-file-input').click();
      });
      uploadZone.addEventListener('dragover', function(e) {
        e.preventDefault();
        this.classList.add('dragover');
      });
      uploadZone.addEventListener('dragleave', function() {
        this.classList.remove('dragover');
      });
      uploadZone.addEventListener('drop', function(e) {
        e.preventDefault();
        this.classList.remove('dragover');
        handleUpload(e.dataTransfer.files);
      });
      document.getElementById('upload-file-input').addEventListener('change', function(e) {
        handleUpload(e.target.files);
        e.target.value = '';
      });

      function handleUpload(files) {
        if (!files || !files.length) return;
        Array.from(files).forEach(function(file) {
          var formData = new FormData();
          formData.append('file', file);
          showToast('Uploading ' + file.name + '...');
          fetch('/api/upload', { method: 'POST', body: formData })
            .then(function(r) {
              if (!r.ok) throw new Error('Upload failed: ' + r.status);
              return r.json();
            })
            .then(function(data) {
              showToast('Uploaded: ' + file.name);
            })
            .catch(function(err) {
              showToast('Upload failed: ' + err.message);
            });
        });
      }

      // ── Wire up asset grid clicks (detail, copy, download, delete, audio play, shader view) ──
      document.getElementById('asset-grid').addEventListener('click', async function(e) {
        // Audio play button
        var audioBtn = e.target.closest('.audio-play-btn');
        if (audioBtn) {
          e.stopPropagation();
          var audioPath = audioBtn.dataset.audioPath;
          if (currentlyPlayingPath === audioPath) {
            sharedAudio.pause();
            sharedAudio.currentTime = 0;
            currentlyPlayingPath = null;
            renderAssets();
          } else {
            sharedAudio.src = audioPath;
            sharedAudio.play();
            currentlyPlayingPath = audioPath;
            sharedAudio.onended = function() {
              currentlyPlayingPath = null;
              renderAssets();
            };
            renderAssets();
          }
          return;
        }

        // Shader code view
        var shaderIcon = e.target.closest('.code-icon[data-shader-path]');
        if (shaderIcon) {
          e.stopPropagation();
          var path = shaderIcon.dataset.shaderPath;
          var asset = ASSETS.find(function(a) { return a.p === path; });
          if (asset) showDetail(asset);
          return;
        }

        var actBtn = e.target.closest('.asset-act-btn');
        if (actBtn) {
          e.stopPropagation();
          var card = actBtn.closest('.asset-card');
          var assetPath = card.dataset.path;
          var asset = ASSETS.find(function(a) { return a.p === assetPath; });
          if (!asset) return;

          var action = actBtn.dataset.action;
          if (action === 'copy') {
            copyToClipboard(asset.p);
          } else if (action === 'download') {
            var a = document.createElement('a');
            a.href = asset.p;
            a.download = asset.n;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
          } else if (action === 'delete') {
            if (await holdConfirm('Delete ' + asset.n + '? This removes the asset record from the server.', { seconds: 3 })) {
              fetch('/api/assets/' + encodeURIComponent(asset.n) + '?key=local', { method: 'DELETE' })
                .then(function(r) {
                  if (r.ok) showToast('Deleted: ' + asset.n);
                  else showToast('Delete failed (may need auth)');
                })
                .catch(function() { showToast('Delete failed'); });
            }
          }
          return;
        }

        var card = e.target.closest('.asset-card');
        if (card) {
          var assetPath = card.dataset.path;
          var asset = ASSETS.find(function(a) { return a.p === assetPath; });
          if (asset) showDetail(asset);
        }
      });

      // ── Close detail overlay on background click or Escape ──
      document.getElementById('asset-detail-overlay').addEventListener('click', function(e) {
        if (e.target === this) this.classList.remove('open');
      });
      document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape') {
          document.getElementById('asset-detail-overlay').classList.remove('open');
        }
      });

      // ── Build file tree from current ASSETS array ──
      function buildFileTree() {
        var tree = document.getElementById('file-tree');
        var dirs = {};
        ASSETS.forEach(function(a) {
          var parts = a.p.split('/');
          parts.pop();
          var dir = parts.join('/') + '/';
          if (!dirs[dir]) dirs[dir] = [];
          dirs[dir].push(a.n);
        });

        var treeHTML = '';
        var sortedDirs = Object.keys(dirs).sort();
        sortedDirs.forEach(function(dir) {
          var files = dirs[dir];
          treeHTML += '<div><span class="tree-folder">' + dir + '</span> <span class="tree-count">(' + files.length + ' files)</span></div>';
          files.forEach(function(f) {
            treeHTML += '<div class="tree-file">' + f + '</div>';
          });
        });
        tree.innerHTML = treeHTML || '<p style="color:var(--text-muted)">No assets loaded.</p>';
      }

      // ── Fetch manifest from API ──
      fetch('/api/asset-manifest')
        .then(function(r) { return r.json(); })
        .then(function(data) {
          ASSETS = data.assets;
          assetCategories = data.categories;
          buildCategoryPills();
          renderAssets();
          updateAssetCount();
          buildFileTree();
        })
        .catch(function() {
          document.getElementById('asset-grid').innerHTML =
            '<p style="color:var(--text-muted);padding:var(--space-xl)">Could not load asset manifest. Is the server running?</p>';
          document.getElementById('asset-count').textContent = '(0)';
        });
    })();

    // ═══════════════════════════════════════════════════════════════════════
    // COMPONENTS TAB, Spacing, radius, typography demos
    // ═══════════════════════════════════════════════════════════════════════
    (function() {
      var cs = getComputedStyle(document.documentElement);

      // Spacing scale
      var spacingVars = [
        ['--space-xs', 'space-xs'], ['--space-sm', 'space-sm'], ['--space-md', 'space-md'],
        ['--space-lg', 'space-lg'], ['--space-xl', 'space-xl'], ['--space-2xl', 'space-2xl'],
        ['--space-3xl', 'space-3xl'],
      ];
      var spacingHTML = '';
      spacingVars.forEach(function(v) {
        var val = cs.getPropertyValue(v[0]).trim();
        var px = parseFloat(val) * 16;
        spacingHTML += '<div class="spacing-row">' +
          '<span class="spacing-label">' + v[1] + '</span>' +
          '<div class="spacing-bar" style="width:' + (px * 3) + 'px"></div>' +
          '<span class="spacing-val">' + val + ' (' + px.toFixed(1) + 'px)</span></div>';
      });
      document.getElementById('spacing-demo').innerHTML = spacingHTML;

      // Border radius
      var radiusVars = [['--radius-sm', 'sm'], ['--radius', 'default'], ['--radius-lg', 'lg']];
      var radiusHTML = '';
      radiusVars.forEach(function(v) {
        var val = cs.getPropertyValue(v[0]).trim();
        radiusHTML += '<div style="width:64px;height:64px;background:var(--accent-dim);border:2px solid var(--accent);' +
          'border-radius:' + val + ';display:flex;align-items:center;justify-content:center;font-size:0.7rem;font-weight:600;color:var(--accent)">' +
          v[1] + '<br>' + val + '</div>';
      });
      document.getElementById('radius-demo').innerHTML = radiusHTML;

      // Typography scale
      var typeVars = [
        ['--text-xs', 'text-xs'], ['--text-sm', 'text-sm'], ['--text-base', 'text-base'],
        ['--text-lg', 'text-lg'], ['--text-xl', 'text-xl'], ['--text-2xl', 'text-2xl'],
        ['--text-3xl', 'text-3xl'],
      ];
      var typeHTML = '';
      typeVars.forEach(function(v) {
        var val = cs.getPropertyValue(v[0]).trim();
        typeHTML += '<div style="font-size:' + val + ';margin-bottom:4px;display:flex;align-items:baseline;gap:12px">' +
          '<span style="min-width:80px;font-size:0.7rem;color:var(--text-muted)">' + v[1] + ' (' + val + ')</span>' +
          '<span>The quick brown fox jumps over the lazy dog</span></div>';
      });
      document.getElementById('type-demo').innerHTML = typeHTML;
    })();

    // ═══════════════════════════════════════════════════════════════════════
    // THEME TAB, Color palette + variable coverage
    // ═══════════════════════════════════════════════════════════════════════
    (function() {
      var cs = getComputedStyle(document.documentElement);

      // Color palette
      var vars = [
        ['--bg', 'bg'], ['--bg-secondary', 'bg-secondary'], ['--bg-card', 'bg-card'],
        ['--bg-card-hover', 'bg-card-hover'], ['--bg-input', 'bg-input'], ['--bg-hover', 'bg-hover'],
        ['--text', 'text'], ['--text-muted', 'text-muted'],
        ['--accent', 'accent'], ['--accent-hover', 'accent-hover'], ['--accent-dim', 'accent-dim'],
        ['--border', 'border'], ['--success', 'success'], ['--danger', 'danger'], ['--warning', 'warning'],
      ];
      var paletteHTML = '';
      vars.forEach(function(v) {
        var val = cs.getPropertyValue(v[0]).trim();
        paletteHTML += '<div class="swatch">' +
          '<div class="swatch-color" style="background:' + val + '"></div>' +
          '<div class="swatch-info"><div class="swatch-name">' + v[1] + '</div><div class="swatch-val">' + val + '</div></div></div>';
      });
      document.getElementById('palette-grid').innerHTML = paletteHTML;

      // Variable coverage table
      var coverage = [
        { name: '--accent', widget: 'Color picker + presets', has: true },
        { name: '--accent-hover', widget: 'Auto-derived from accent', has: true },
        { name: '--accent-dim', widget: 'Auto-derived from accent', has: true },
        { name: '--icon-weight', widget: 'Icon Weight slider', has: true },
        { name: '--icon-size', widget: 'Icon Size slider', has: true },
        { name: '--radius / sm / lg', widget: 'Border Radius slider', has: true },
        { name: '--content-width', widget: 'Content Width slider', has: true },
        { name: '--line-height', widget: 'Line Height slider', has: true },
        { name: '--space-* (all 7)', widget: 'UI Spacing slider (50-200%)', has: true },
        { name: '--font-size-base', widget: 'Font Size pills', has: true },
        { name: '--success', widget: 'Success Color override', has: true },
        { name: '--danger', widget: 'Danger Color override', has: true },
        { name: '--warning', widget: 'Warning Color override', has: true },
        { name: '--bg', widget: 'Theme toggle (dark/light)', has: false },
        { name: '--bg-secondary', widget: 'Theme toggle', has: false },
        { name: '--bg-card', widget: 'Theme toggle', has: false },
        { name: '--bg-card-hover', widget: 'Theme toggle', has: false },
        { name: '--bg-input', widget: 'Theme toggle', has: false },
        { name: '--bg-hover', widget: 'Theme toggle', has: false },
        { name: '--text', widget: 'Theme toggle', has: false },
        { name: '--text-muted', widget: 'Theme toggle', has: false },
        { name: '--border', widget: 'Theme toggle', has: false },
        { name: '--text-xs to --text-3xl', widget: 'Proportional to --font-size-base', has: false },
      ];

      var tbody = document.getElementById('var-table-body');
      var tbodyHTML = '';
      coverage.forEach(function(v) {
        var val = '';
        var cleanName = v.name.replace(/ \/.*/,'').replace(/ \(.*/,'').replace(/ to .*/,'');
        if (cleanName.startsWith('--')) val = cs.getPropertyValue(cleanName).trim();
        var swatchColor = val && (val.startsWith('#') || val.startsWith('rgb')) ?
          '<span class="swatch-inline" style="background:' + val + '"></span>' : '';
        tbodyHTML += '<tr><td><code>' + v.name + '</code></td>' +
          '<td>' + swatchColor + (val || '-') + '</td>' +
          '<td class="' + (v.has ? 'has-widget' : 'no-widget') + '">' + (v.has ? '✓ ' : '') + v.widget + '</td></tr>';
      });
      tbody.innerHTML = tbodyHTML;
    })();
