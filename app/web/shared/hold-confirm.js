/* hold-confirm.js — press-and-HOLD confirmation for destructive actions.
 *
 * The web twin of native `widgets::hold_to_confirm`. A single misclick must
 * never fire something irreversible (delete account, disband group, wipe data),
 * so the user has to HOLD the confirm control for a few seconds while a progress
 * indicator fills. Releasing early cancels.
 *
 * Two entry points, both global (plain scripts, no modules):
 *
 *   holdToConfirm(el, {seconds, onConfirm})
 *     Attach a hold gate to an existing element. Drives the element's `--hold-p`
 *     CSS var 0 -> 1 over `seconds` while the primary pointer is held, toggles a
 *     `.holding` class, and fires `onConfirm` once on completion. The element's
 *     LOOK (ring, bar, ...) is the caller's CSS, keyed off `--hold-p`.
 *
 *   holdConfirm(message, {seconds, confirmLabel, cancelLabel}) -> Promise<bool>
 *     A modal, drop-in async replacement for window.confirm(): shows `message`
 *     with a Cancel button and a HOLD-to-confirm button (a danger bar fills as
 *     you hold). Resolves true only after a full hold, false on cancel / Esc /
 *     backdrop click. Usage: `if (!await holdConfirm('Delete X?')) return;`
 *
 * Loading this script also injects the modal's CSS once, so a page needs only
 * the one <script src="/shared/hold-confirm.js"> include, nothing else.
 */
(function () {
  'use strict';
  if (window.holdConfirm && window.holdToConfirm) return; // already loaded

  // ---- generic inline hold gate -------------------------------------------
  function holdToConfirm(el, opts) {
    opts = opts || {};
    const seconds = opts.seconds != null ? opts.seconds : 5;
    const onConfirm = opts.onConfirm;
    let raf = null, start = null;
    const stop = () => {
      if (raf) { cancelAnimationFrame(raf); raf = null; }
      start = null;
      el.style.setProperty('--hold-p', 0);
      el.classList.remove('holding');
    };
    const tick = (now) => {
      if (start === null) start = now;
      const p = Math.min(1, (now - start) / (seconds * 1000));
      el.style.setProperty('--hold-p', p);
      if (p >= 1) { stop(); if (onConfirm) onConfirm(); return; }
      raf = requestAnimationFrame(tick);
    };
    el.addEventListener('pointerdown', (e) => {
      if (e.button !== undefined && e.button !== 0) return; // primary button only
      e.preventDefault();
      try { el.setPointerCapture(e.pointerId); } catch (_) {}
      el.classList.add('holding');
      start = null;
      if (!raf) raf = requestAnimationFrame(tick);
    });
    ['pointerup', 'pointercancel', 'lostpointercapture'].forEach((ev) =>
      el.addEventListener(ev, stop)
    );
  }

  // ---- modal drop-in for confirm() ----------------------------------------
  function holdConfirm(message, opts) {
    opts = opts || {};
    const seconds = opts.seconds != null ? opts.seconds : 5;
    const confirmLabel = opts.confirmLabel || 'Hold to confirm';
    const cancelLabel = opts.cancelLabel || 'Cancel';
    return new Promise((resolve) => {
      const overlay = document.createElement('div');
      overlay.className = 'hold-confirm-overlay';
      const modal = document.createElement('div');
      modal.className = 'hold-confirm-modal';
      modal.setAttribute('role', 'alertdialog');
      modal.setAttribute('aria-modal', 'true');

      const msg = document.createElement('div');
      msg.className = 'hold-confirm-msg';
      msg.textContent = message; // textContent: never interpret message as HTML

      const actions = document.createElement('div');
      actions.className = 'hold-confirm-actions';

      const cancelBtn = document.createElement('button');
      cancelBtn.type = 'button';
      cancelBtn.className = 'hold-confirm-cancel';
      cancelBtn.textContent = cancelLabel;

      const goBtn = document.createElement('button');
      goBtn.type = 'button';
      goBtn.className = 'hold-confirm-go';
      const goRing = document.createElement('span');
      goRing.className = 'hold-confirm-ring';
      goRing.setAttribute('aria-hidden', 'true');
      const goLabel = document.createElement('span');
      goLabel.className = 'hold-confirm-golabel';
      goLabel.textContent = confirmLabel;
      goBtn.appendChild(goRing);
      goBtn.appendChild(goLabel);

      actions.appendChild(cancelBtn);
      actions.appendChild(goBtn);
      modal.appendChild(msg);
      modal.appendChild(actions);
      overlay.appendChild(modal);
      document.body.appendChild(overlay);

      let done = false;
      const onKey = (e) => { if (e.key === 'Escape') close(false); };
      const close = (val) => {
        if (done) return;
        done = true;
        document.removeEventListener('keydown', onKey);
        overlay.remove();
        resolve(val);
      };
      document.addEventListener('keydown', onKey);
      cancelBtn.addEventListener('click', () => close(false));
      overlay.addEventListener('click', (e) => { if (e.target === overlay) close(false); });
      holdToConfirm(goBtn, { seconds, onConfirm: () => close(true) });
      cancelBtn.focus(); // safe default: the non-destructive control is focused
    });
  }

  // ---- one-time CSS injection ---------------------------------------------
  function injectStyle() {
    if (document.getElementById('hold-confirm-style')) return;
    const style = document.createElement('style');
    style.id = 'hold-confirm-style';
    style.textContent = [
      '.hold-confirm-overlay{position:fixed;inset:0;z-index:100000;display:flex;',
      'align-items:center;justify-content:center;background:rgba(0,0,0,0.55);',
      'backdrop-filter:blur(2px);padding:20px;}',
      '.hold-confirm-modal{background:var(--bg-secondary,#1c1c1c);color:var(--text,#eee);',
      'border:1px solid var(--border,#444);border-radius:var(--radius,8px);',
      'max-width:420px;width:100%;padding:20px;box-shadow:0 10px 40px rgba(0,0,0,0.5);}',
      '.hold-confirm-msg{font-size:0.9rem;line-height:1.45;white-space:pre-wrap;',
      'margin-bottom:18px;color:var(--text,#eee);}',
      '.hold-confirm-actions{display:flex;gap:10px;justify-content:flex-end;}',
      '.hold-confirm-cancel{background:var(--bg-card,#2a2a2a);color:var(--text,#eee);',
      'border:1px solid var(--border,#444);border-radius:var(--radius,6px);',
      'padding:9px 16px;font-size:0.8rem;cursor:pointer;}',
      '.hold-confirm-cancel:hover{background:var(--bg-hover,#333);}',
      '.hold-confirm-go{display:inline-flex;align-items:center;justify-content:center;gap:8px;',
      'border:1px solid var(--danger,#e05555);border-radius:var(--radius,6px);',
      'background:transparent;color:var(--danger,#e05555);',
      'padding:9px 16px;font-size:0.8rem;font-weight:600;cursor:pointer;user-select:none;',
      '-webkit-user-select:none;touch-action:none;min-width:170px;}',
      '.hold-confirm-go:hover{background:rgba(224,85,85,0.12);}',
      // Clock ring that fills clockwise with the hold; while holding, the arc
      // cycles RGB FAST (hue-rotate) so the wheel is as visible as possible.
      '.hold-confirm-ring{width:20px;height:20px;flex:0 0 auto;border-radius:50%;',
      'background:conic-gradient(var(--danger,#e05555) calc(var(--hold-p,0)*1turn),var(--border,#555) 0);',
      '-webkit-mask:radial-gradient(circle,transparent 5px,#000 5.5px);',
      'mask:radial-gradient(circle,transparent 5px,#000 5.5px);}',
      '.hold-confirm-go.holding .hold-confirm-ring{animation:hos-hold-rgb 0.8s linear infinite;}',
      '@keyframes hos-hold-rgb{to{filter:hue-rotate(360deg);}}',
    ].join('');
    (document.head || document.documentElement).appendChild(style);
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', injectStyle);
  } else {
    injectStyle();
  }

  window.holdToConfirm = holdToConfirm;
  window.holdConfirm = holdConfirm;
})();
