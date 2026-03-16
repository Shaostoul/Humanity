/**
 * ELI5 + Expert dual-depth explanation system
 *
 * Usage in any page:
 *   <div class="eli5-block">
 *     <div class="eli5-simple">Simple explanation visible by default.</div>
 *     <div class="eli5-expert">Detailed expert explanation, collapsed by default.</div>
 *   </div>
 *
 * The toggle state persists in localStorage. Users who prefer expert mode
 * get it everywhere, automatically.
 */
(function () {
  if (window.__HOS_ELI5__) return;
  window.__HOS_ELI5__ = true;

  var LS_KEY = 'hos_eli5_mode'; // 'simple' or 'expert'
  var mode = localStorage.getItem(LS_KEY) || 'simple';

  // Inject styles once
  var style = document.createElement('style');
  style.textContent =
    '.eli5-block{position:relative;margin:0.75rem 0;padding:0.75rem 1rem;' +
    'border-radius:var(--radius,8px);background:var(--bg-card,rgba(30,30,35,0.6));' +
    'border:1px solid var(--border,rgba(80,80,90,0.3));}' +

    '.eli5-toggle{position:absolute;top:0.5rem;right:0.5rem;background:none;border:1px solid var(--border,#444);' +
    'color:var(--text-muted,#999);font-size:0.7rem;padding:2px 8px;border-radius:12px;cursor:pointer;' +
    'transition:all 0.2s;z-index:1;}' +
    '.eli5-toggle:hover{color:var(--text,#eee);border-color:var(--accent,#4af);}' +

    '.eli5-simple{font-size:0.92rem;line-height:1.5;color:var(--text,#ddd);}' +
    '.eli5-simple::before{content:"💡 ";font-size:1rem;}' +

    '.eli5-expert{font-size:0.85rem;line-height:1.55;color:var(--text-muted,#aaa);' +
    'margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid var(--border,rgba(80,80,90,0.2));}' +
    '.eli5-expert::before{content:"🔬 ";font-size:0.9rem;}' +

    '.eli5-block[data-mode="simple"] .eli5-expert{display:none;}' +
    '.eli5-block[data-mode="expert"] .eli5-simple{display:none;}';
  document.head.appendChild(style);

  function initBlocks() {
    var blocks = document.querySelectorAll('.eli5-block');
    blocks.forEach(function (block) {
      if (block.dataset.eli5Init) return;
      block.dataset.eli5Init = '1';
      block.dataset.mode = mode;

      // Add toggle button
      var btn = document.createElement('button');
      btn.className = 'eli5-toggle';
      btn.type = 'button';
      btn.textContent = mode === 'simple' ? 'Expert ▸' : '◂ Simple';
      btn.setAttribute('aria-label', 'Toggle explanation depth');
      block.prepend(btn);

      btn.addEventListener('click', function () {
        var current = block.dataset.mode;
        var next = current === 'simple' ? 'expert' : 'simple';
        // Update all blocks on the page
        document.querySelectorAll('.eli5-block').forEach(function (b) {
          b.dataset.mode = next;
          var t = b.querySelector('.eli5-toggle');
          if (t) t.textContent = next === 'simple' ? 'Expert ▸' : '◂ Simple';
        });
        mode = next;
        localStorage.setItem(LS_KEY, next);
      });
    });
  }

  // Run on load and observe for dynamically added blocks
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initBlocks);
  } else {
    initBlocks();
  }

  // Watch for blocks added later (SPA-style)
  var observer = new MutationObserver(function () { initBlocks(); });
  observer.observe(document.body || document.documentElement, { childList: true, subtree: true });
})();
