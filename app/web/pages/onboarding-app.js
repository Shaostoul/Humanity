// Moved out of web/pages/onboarding.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    // ── Quest chain renderer ──
    // Loads data/onboarding/quests.json, tracks progress in localStorage. Each chain is a card
    // shaped like the native Quests page's quest card (src/gui/pages/quests.rs, 2026-10-30): its
    // name and one-line purpose, "Step 2 of 4" over a meter with a segment per step, the next step
    // as the card's focus. (On the desktop these chains are the Tasks page's suggested cards, the
    // website's Tasks page shows them the same way, and this page is their second home on the
    // website: docs/design/ui-system.md, its "Quest card and chain window" row.)
    // A click opens the chain's window (the native chain window, quests/chain_window.rs) with
    // every step, to tick off or to go to. These are real-life guides, with no story to spoil,
    // so every step shows, the ones after the next slightly dimmed (the native Dev view's look).
    // Data-driven: a new chain is a new entry in the JSON, not a change here.
    (function () {
      var STORAGE_KEY = 'humanity_quest_progress';
      var root = document.getElementById('quest-root');
      var resetBtn = document.getElementById('reset-quests');
      var data = null;
      // The chain whose window is open, by id.
      var openChain = null;

      function esc(s) {
        return String(s).replace(/[&<>"']/g, function (c) {
          return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
        });
      }
      function loadProgress() {
        try { return JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}'); }
        catch (e) { return {}; }
      }
      function saveProgress(p) {
        try { localStorage.setItem(STORAGE_KEY, JSON.stringify(p)); } catch (e) { /* private window: kept for this visit only */ }
      }
      function stepKey(chain, step) { return chain.id + ':' + step.id; }

      /** How many steps of a chain are done, and the first one not done (-1 when all are). */
      function standing(chain, progress) {
        var done = 0, next = -1;
        chain.steps.forEach(function (step, i) {
          if (progress[stepKey(chain, step)]) done++;
          else if (next < 0) next = i;
        });
        return { done: done, next: next };
      }

      /** A chain's card: its name, where it stands, the meter, the next step, "See the whole chain". */
      function card(chain, progress) {
        var st = standing(chain, progress);
        var allDone = st.next < 0;
        var count = allDone ? i18n.t('quests.state_done') : i18n.t('quests.step_count', { n: st.next + 1, total: chain.steps.length });
        var html = '<button type="button" class="chain-card' + (allDone ? ' all-done' : '') + '" data-chain="' + esc(chain.id) + '" data-i18n-tip="onboarding.chain_card_hint">';
        html += '<span class="chain-card-head"><span class="chain-card-title">' + esc(chain.title) + '</span>';
        html += '<span class="chain-card-count">' + esc(count) + '</span></span>';
        // The chain's one line, as the desktop's suggested card has it (Tasks page, quests.json purpose).
        if (chain.purpose) html += '<span class="chain-card-purpose">' + esc(chain.purpose) + '</span>';
        html += '<span class="chain-meter" aria-hidden="true">';
        chain.steps.forEach(function (step, i) {
          html += '<i class="' + (progress[stepKey(chain, step)] ? 'done' : (i === st.next ? 'now' : '')) + '"></i>';
        });
        html += '</span>';
        if (allDone) {
          html += '<span class="chain-now-desc">' + esc(i18n.t('onboarding.all_done')) + '</span>';
        } else {
          var next = chain.steps[st.next];
          html += '<span class="chain-now"><span class="chain-now-label">' + esc(i18n.t('onboarding.next')) + '</span>';
          html += '<span class="chain-now-title">' + esc(next.title) + '</span>';
          html += '<span class="chain-now-desc">' + esc(next.description) + '</span></span>';
        }
        html += '<span class="chain-card-more">' + esc(i18n.t('quests.see_chain')) + '</span>';
        return html + '</button>';
      }

      /** A chain's window: its name and description, then every step with its tick box and Go. */
      function chainWindow(chain, progress) {
        var st = standing(chain, progress);
        var html = '<div class="chain-modal" role="dialog" aria-modal="true" aria-labelledby="chain-modal-title">';
        html += '<div class="chain-modal-head"><h3 id="chain-modal-title">' + esc(chain.title) + '</h3>';
        html += '<button type="button" class="chain-close" data-close="1" aria-label="' + esc(i18n.t('common.close')) + '" data-i18n-tip="onboarding.close_hint">×</button></div>';
        if (chain.purpose) html += '<p class="chain-modal-purpose">' + esc(chain.purpose) + '</p>';
        html += '<p class="chain-modal-desc">' + esc(chain.description) + '</p>';
        html += '<p class="chain-modal-intro">' + esc(i18n.t('onboarding.chain_intro')) + '</p>';
        html += '<ol class="quest-steps">';
        chain.steps.forEach(function (step, i) {
          var key = stepKey(chain, step);
          var done = !!progress[key];
          html += '<li class="quest-step ' + (done ? 'done' : (i === st.next ? 'now' : 'ahead')) + '">';
          html += '<button type="button" class="quest-checkbox" data-step-key="' + esc(key) + '" aria-pressed="' + done + '" aria-label="' + esc(step.title) + '" data-i18n-tip="onboarding.tick_hint">' + (done ? '✓' : '') + '</button>';
          html += '<span class="quest-text"><span class="qs-title">' + esc(step.title) + '</span><br><span class="qs-desc">' + esc(step.description) + '</span></span>';
          // Go there, and Open the how-to where the step has a written one (quests.json how_to; the
          // desktop's suggested cards and the Tasks page offer the same two, with the same words).
          if (step.link || step.how_to) {
            html += '<span class="qs-actions">';
            if (step.link) {
              var outside = step.link.indexOf('http') === 0;
              html += '<a class="qs-action" href="' + esc(step.link) + '"' + (outside ? ' target="_blank" rel="noopener"' : '') + ' data-i18n-tip="onboarding.go_hint">' + esc(i18n.t('onboarding.go')) + ' →</a>';
            }
            if (step.how_to) {
              html += '<a class="qs-action" href="/how-to#' + encodeURIComponent(step.how_to) + '" data-i18n-tip="tasks.suggested_how_to_hint">' + esc(i18n.t('tasks.suggested_how_to')) + '</a>';
            }
            html += '</span>';
          }
          html += '</li>';
        });
        return html + '</ol></div>';
      }

      /** Draw the window for the open chain, or take it away when none is open. */
      function drawWindow(progress) {
        var existing = document.getElementById('chain-modal-root');
        var chain = openChain && data.chains.filter(function (c) { return c.id === openChain; })[0];
        if (!chain) {
          if (existing) existing.remove();
          return;
        }
        var backdrop = existing;
        if (!backdrop) {
          backdrop = document.createElement('div');
          backdrop.id = 'chain-modal-root';
          backdrop.className = 'chain-backdrop';
          backdrop.addEventListener('click', onWindowClick);
          document.body.appendChild(backdrop);
        }
        backdrop.innerHTML = chainWindow(chain, progress);
        i18n.apply(backdrop);
      }

      function render() {
        var progress = loadProgress();
        root.className = 'chain-grid';
        root.innerHTML = data.chains.map(function (c) { return card(c, progress); }).join('');
        i18n.apply(root);
        drawWindow(progress);
      }

      function closeWindow() {
        var was = openChain;
        openChain = null;
        drawWindow(loadProgress());
        var back = root.querySelector('.chain-card[data-chain="' + CSS.escape(was || '') + '"]');
        if (back) back.focus();
      }

      // A click on the dimmed screen or the close button closes the window; on a tick box it
      // marks its step done or not done, and the card under the window follows.
      function onWindowClick(e) {
        if (e.target.id === 'chain-modal-root' || e.target.closest('[data-close]')) {
          closeWindow();
          return;
        }
        var tick = e.target.closest('.quest-checkbox');
        if (!tick) return;
        var key = tick.dataset.stepKey;
        var progress = loadProgress();
        progress[key] = !progress[key];
        saveProgress(progress);
        render();
        var again = document.querySelector('#chain-modal-root .quest-checkbox[data-step-key="' + CSS.escape(key) + '"]');
        if (again) again.focus();
      }

      root.addEventListener('click', function (e) {
        var c = e.target.closest('.chain-card');
        if (!c || !data) return;
        openChain = c.dataset.chain;
        drawWindow(loadProgress());
        var close = document.querySelector('#chain-modal-root .chain-close');
        if (close) close.focus();
      });
      document.addEventListener('keydown', function (e) {
        if (e.key === 'Escape' && openChain) closeWindow();
      });

      fetch('/data/onboarding/quests.json')
        .then(function (r) {
          if (!r.ok) throw new Error('HTTP ' + r.status);
          return r.json();
        })
        .then(function (d) {
          data = d;
          render();
        })
        .catch(function (err) {
          root.innerHTML = '<div class="loading-state">' + esc(i18n.t('onboarding.load_failed', { error: err.message })) + '</div>';
        });

      if (resetBtn) {
        resetBtn.addEventListener('click', async function () {
          if (await holdConfirm('Clear all quest progress in this browser?', { seconds: 3 })) {
            localStorage.removeItem(STORAGE_KEY);
            location.reload();
          }
        });
      }
    })();
