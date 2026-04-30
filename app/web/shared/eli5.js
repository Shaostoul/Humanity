/**
 * ELI5 + Expert dual-depth explanation system
 *
 * Both simple and expert text show by default.
 * Users can hide either via Settings → Appearance.
 * localStorage keys: hos_show_simple ('1'/'0'), hos_show_expert ('1'/'0')
 *
 * Usage in any page:
 *   <div class="eli5-block">
 *     <div class="eli5-simple">Simple explanation.</div>
 *     <div class="eli5-expert">Technical details.</div>
 *   </div>
 */
(function () {
  if (window.__HOS_ELI5__) return;
  window.__HOS_ELI5__ = true;

  var showSimple = localStorage.getItem('hos_show_simple') !== '0';
  var showExpert = localStorage.getItem('hos_show_expert') !== '0';

  var style = document.createElement('style');
  style.id = 'eli5-styles';
  style.textContent =
    '.eli5-block{margin:0.75rem 0;padding:0.75rem 1rem;' +
    'border-radius:var(--radius,8px);background:var(--bg-card,rgba(30,30,35,0.6));' +
    'border:1px solid var(--border,rgba(80,80,90,0.3));}' +

    '.eli5-simple{font-size:0.92rem;line-height:1.5;color:var(--text,#ddd);}' +
    '.eli5-simple::before{content:"💡 ";font-size:1rem;}' +

    '.eli5-expert{font-size:0.85rem;line-height:1.55;color:var(--text-muted,#aaa);' +
    'margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid var(--border,rgba(80,80,90,0.2));}' +
    '.eli5-expert::before{content:"🔬 ";font-size:0.9rem;}' +

    '.eli5-block:not([data-show-simple="1"]) .eli5-simple{display:none;}' +
    '.eli5-block:not([data-show-expert="1"]) .eli5-expert{display:none;}' +
    '.eli5-block:not([data-show-simple="1"]):not([data-show-expert="1"]){display:none;}';
  document.head.appendChild(style);

  function initBlocks() {
    document.querySelectorAll('.eli5-block').forEach(function (block) {
      if (block.dataset.eli5Init) return;
      block.dataset.eli5Init = '1';
      block.dataset.showSimple = showSimple ? '1' : '0';
      block.dataset.showExpert = showExpert ? '1' : '0';
    });
  }

  // Apply preference changes from Settings (called globally)
  window.__hos_eli5_update = function () {
    showSimple = localStorage.getItem('hos_show_simple') !== '0';
    showExpert = localStorage.getItem('hos_show_expert') !== '0';
    document.querySelectorAll('.eli5-block').forEach(function (block) {
      block.dataset.showSimple = showSimple ? '1' : '0';
      block.dataset.showExpert = showExpert ? '1' : '0';
    });
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initBlocks);
  } else {
    initBlocks();
  }

  var observer = new MutationObserver(function () { initBlocks(); });
  observer.observe(document.body || document.documentElement, { childList: true, subtree: true });
})();
