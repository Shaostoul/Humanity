// ── First-Time Onboarding Wizard ──
// Goal: walk brand-new users through what just happened (a cryptographic identity
// was created for them), why they need to back it up, and what this platform is —
// all in plain language a non-technical person can follow.
// Called once from app.js when myIdentity.isNew is true, never shown again.

const ONBOARD_DONE_KEY = 'humanity_onboarding_done';

/**
 * Show the multi-step onboarding wizard to a first-time visitor.
 * Steps guide them through: identity creation, seed phrase backup,
 * what Humanity is, how to meet people, and a "you're ready" launch pad.
 * @param {string} [mnemonic] - Pre-generated 24-word phrase (pass so we don't call generateMnemonic twice).
 */
async function showOnboardingWizard(mnemonic) {
  if (localStorage.getItem(ONBOARD_DONE_KEY)) return;

  // Generate the mnemonic now if not provided.
  if (!mnemonic) {
    try { mnemonic = await generateMnemonic(); } catch(e) { mnemonic = null; }
  }

  let step = 0;
  const TOTAL = 5;

  const overlay = document.createElement('div');
  overlay.id = 'onboarding-overlay';
  overlay.style.cssText = `
    position:fixed;inset:0;background:rgba(0,0,0,.92);z-index:9000;
    display:flex;align-items:center;justify-content:center;
    padding:1rem;box-sizing:border-box;
  `;
  document.body.appendChild(overlay);

  function render() {
    overlay.innerHTML = buildStep(step, mnemonic);
    // Wire navigation buttons
    const prev = overlay.querySelector('#ob-prev');
    const next = overlay.querySelector('#ob-next');
    const skip = overlay.querySelector('#ob-skip');
    if (prev) prev.addEventListener('click', () => { if (step > 0) { step--; render(); } });
    if (next) next.addEventListener('click', () => { if (step < TOTAL - 1) { step++; render(); } else { finish(); } });
    if (skip) skip.addEventListener('click', finish);

    // Step-specific wiring
    if (step === 1) wireStep1(overlay, mnemonic);
    if (step === 4) wireStep4(overlay);
  }

  function finish() {
    localStorage.setItem(ONBOARD_DONE_KEY, '1');
    overlay.remove();
  }

  render();
}

// ── Step builders ────────────────────────────────────────────────────────────

function buildStep(step, mnemonic) {
  const dots = Array.from({length: 5}, (_, i) =>
    `<span style="width:8px;height:8px;border-radius:50%;display:inline-block;background:${i === step ? '#f0a500' : '#333'};margin:0 3px"></span>`
  ).join('');

  const content = [step0, step1, step2, step3, step4][step](mnemonic);
  const isLast  = step === 4;
  const isFirst = step === 0;

  return `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:16px;padding:2rem;
                width:100%;max-width:580px;font-family:'Segoe UI',system-ui,sans-serif;
                color:#e0e0e0;max-height:92vh;overflow-y:auto;box-sizing:border-box;">

      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1.5rem">
        <div>${dots}</div>
        <span style="font-size:.7rem;color:#444">${step + 1} of 5</span>
      </div>

      ${content}

      <div style="display:flex;gap:.75rem;justify-content:space-between;align-items:center;margin-top:1.75rem;flex-wrap:wrap">
        <button id="ob-skip" style="background:none;border:none;color:#444;font-size:.75rem;cursor:pointer;padding:.25rem .5rem;text-decoration:underline">
          ${isLast ? '' : 'Skip intro'}
        </button>
        <div style="display:flex;gap:.75rem">
          ${!isFirst ? `<button id="ob-prev"
            style="background:none;border:1px solid #333;color:#888;border-radius:8px;
                   padding:.5rem 1.1rem;font-size:.85rem;cursor:pointer">← Back</button>` : ''}
          <button id="ob-next"
            style="background:#f0a500;color:#000;border:none;border-radius:8px;
                   padding:.5rem 1.4rem;font-size:.85rem;font-weight:700;cursor:pointer">
            ${isLast ? '🚀 Let\'s go!' : 'Next →'}
          </button>
        </div>
      </div>
    </div>
  `;
}

// ── Step 0: Welcome ───────────────────────────────────────────────────────────
function step0() {
  return `
    <h2 style="font-size:1.4rem;font-weight:800;color:#f0a500;margin:0 0 .5rem">👋 Welcome to Humanity!</h2>
    <p style="font-size:.9rem;line-height:1.65;color:#ccc;margin:0 0 1rem">
      We just created a <strong style="color:#e0e0e0">unique digital identity</strong> for you — and we want to explain what that means
      in plain language before you dive in.
    </p>

    <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:1.1rem 1.2rem;margin-bottom:1rem">
      <p style="font-size:.85rem;color:#ccc;margin:0 0 .6rem;font-weight:600">🤔 Wait — no account? No password?</p>
      <p style="font-size:.82rem;color:#888;line-height:1.6;margin:0">
        That's right. Instead of a username and password stored on a server somewhere, we generated a
        <strong style="color:#e0e0e0">secret key</strong> that lives right here in your browser.
        It's like getting a house key cut — it's yours, it's unique, and nobody else has one like it.
      </p>
    </div>

    <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:1.1rem 1.2rem">
      <p style="font-size:.85rem;color:#ccc;margin:0 0 .6rem;font-weight:600">✅ What this means for you</p>
      <ul style="font-size:.82rem;color:#888;line-height:1.8;margin:0;padding-left:1.2rem">
        <li>No company holds your account — not us, not anyone.</li>
        <li>You can't be banned, shadowbanned, or deplatformed.</li>
        <li>Nobody reads your messages — they're encrypted.</li>
        <li>Your identity is the same on every device, as long as you back it up (we'll show you how).</li>
      </ul>
    </div>
  `;
}

// ── Step 1: Seed Phrase ───────────────────────────────────────────────────────
function step1(mnemonic) {
  const words = mnemonic ? mnemonic.trim().split(/\s+/) : [];
  const wordGrid = words.length === 24
    ? `<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:.4rem;margin:1rem 0">
        ${words.map((w, i) => `
          <div style="background:#0a0a0a;border:1px solid #2a2a2a;border-radius:7px;
                      padding:.4rem .5rem;display:flex;align-items:baseline;gap:.3rem">
            <span style="font-size:.6rem;color:#444;min-width:16px;text-align:right">${i+1}.</span>
            <span style="font-size:.82rem;color:#f0a500;font-weight:600">${w}</span>
          </div>`).join('')}
      </div>
      <div id="ob-copy-row" style="display:flex;align-items:center;gap:.75rem;margin-bottom:.6rem">
        <button id="ob-copy-btn"
          style="background:none;border:1px solid #333;color:#888;border-radius:7px;
                 padding:.35rem .9rem;font-size:.78rem;cursor:pointer">📋 Copy all 24 words</button>
        <span id="ob-copy-msg" style="font-size:.72rem;color:#4ec87a"></span>
      </div>`
    : `<div style="background:#1a1a1a;border:1px dashed #333;border-radius:8px;padding:1rem;
                   font-size:.8rem;color:#666;margin:1rem 0;text-align:center">
         Seed phrase unavailable — your key may not be extractable in this browser.<br>
         Use <strong>Encrypted Backup</strong> from the profile menu instead.
       </div>`;

  return `
    <h2 style="font-size:1.2rem;font-weight:800;color:#f0a500;margin:0 0 .4rem">🌱 Your 24-Word Recovery Phrase</h2>
    <p style="font-size:.83rem;line-height:1.6;color:#888;margin:0 0 .75rem">
      Here's the most important thing we'll tell you today:
    </p>

    <div style="background:#1c1200;border:1px solid #5a3800;border-radius:10px;padding:1rem 1.1rem;margin-bottom:.75rem">
      <p style="font-size:.85rem;color:#f0a500;font-weight:700;margin:0 0 .4rem">⚠️ Think of this as your master key</p>
      <p style="font-size:.8rem;color:#ccc;line-height:1.6;margin:0">
        These 24 words can restore your entire identity on any device, forever.
        Anyone who has them <em>is</em> you — so keep them offline, on paper,
        somewhere only you can find. <strong>Don't screenshot. Don't email. Don't type them into any website.</strong>
      </p>
    </div>

    ${wordGrid}

    <label style="display:flex;align-items:flex-start;gap:.6rem;cursor:pointer;font-size:.8rem;color:#888;line-height:1.5" id="ob-wrote-label">
      <input type="checkbox" id="ob-wrote-check" style="margin-top:.15rem;accent-color:#f0a500">
      I've written these 24 words on paper and stored them somewhere safe.
    </label>
  `;
}

function wireStep1(overlay, mnemonic) {
  const copyBtn = overlay.querySelector('#ob-copy-btn');
  const copyMsg = overlay.querySelector('#ob-copy-msg');
  if (copyBtn && mnemonic) {
    copyBtn.addEventListener('click', () => {
      navigator.clipboard.writeText(mnemonic).then(() => {
        copyMsg.textContent = '✓ Copied — paste it somewhere offline, then clear your clipboard.';
        copyBtn.textContent = 'Copied!';
      }).catch(() => { copyMsg.textContent = 'Copy failed — select the words above manually.'; });
    });
  }
}

// ── Step 2: What is Humanity ─────────────────────────────────────────────────
function step2() {
  return `
    <h2 style="font-size:1.2rem;font-weight:800;color:#f0a500;margin:0 0 .4rem">🌍 What is Humanity?</h2>
    <p style="font-size:.83rem;line-height:1.6;color:#888;margin:0 0 1rem">
      Humanity is a cooperative platform — not owned by any company, not funded by ads, and not watching you.
    </p>

    <div style="display:grid;gap:.6rem">
      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem">
        <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .3rem">💬 Chat that's actually private</p>
        <p style="font-size:.8rem;color:#888;line-height:1.5;margin:0">
          Messages between you and another person are encrypted end-to-end — meaning only you two can read them.
          Not us, not the server, not your ISP. It's like passing a note in an envelope no one else can open.
        </p>
      </div>

      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem">
        <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .3rem">🚫 No algorithm. No ads. No engagement bait.</p>
        <p style="font-size:.8rem;color:#888;line-height:1.5;margin:0">
          There's no feed tuned to keep you angry or scrolling. You see what you choose to see.
          The platform doesn't profit from your attention.
        </p>
      </div>

      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem">
        <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .3rem">🤝 Owned by everyone, run by the community</p>
        <p style="font-size:.8rem;color:#888;line-height:1.5;margin:0">
          The code is open source. Anyone can check it. Anyone can run a copy.
          Decisions about the platform's direction are made cooperatively.
        </p>
      </div>
    </div>
  `;
}

// ── Step 3: How to Connect ───────────────────────────────────────────────────
function step3() {
  return `
    <h2 style="font-size:1.2rem;font-weight:800;color:#f0a500;margin:0 0 .4rem">🤝 Meeting People</h2>
    <p style="font-size:.83rem;line-height:1.6;color:#888;margin:0 0 1rem">
      Unlike social media, you build your network intentionally — no followers game, no public follower counts.
    </p>

    <div style="display:grid;gap:.6rem">
      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem;display:flex;gap:.75rem">
        <span style="font-size:1.4rem;flex-shrink:0">💬</span>
        <div>
          <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .25rem">Jump into a channel</p>
          <p style="font-size:.79rem;color:#888;line-height:1.5;margin:0">
            Channels on the left sidebar are open rooms — like a coffee shop with a topic.
            Just start talking. Nobody requires an introduction.
          </p>
        </div>
      </div>

      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem;display:flex;gap:.75rem">
        <span style="font-size:1.4rem;flex-shrink:0">📮</span>
        <div>
          <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .25rem">Direct Messages</p>
          <p style="font-size:.79rem;color:#888;line-height:1.5;margin:0">
            Click any username to open a private, encrypted conversation with that person. Only the two of you can read it.
          </p>
        </div>
      </div>

      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem;display:flex;gap:.75rem">
        <span style="font-size:1.4rem;flex-shrink:0">🪪</span>
        <div>
          <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .25rem">Contact Cards (P2P)</p>
          <p style="font-size:.79rem;color:#888;line-height:1.5;margin:0">
            In the Contacts tab you can share a QR code or a short link.
            Scanning it lets two people connect directly, even without the relay server in the middle.
          </p>
        </div>
      </div>

      <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:10px;padding:.9rem 1rem;display:flex;gap:.75rem">
        <span style="font-size:1.4rem;flex-shrink:0">✏️</span>
        <div>
          <p style="font-size:.85rem;color:#e0e0e0;font-weight:600;margin:0 0 .25rem">Set up your profile</p>
          <p style="font-size:.79rem;color:#888;line-height:1.5;margin:0">
            Tap your name in the sidebar to add a bio, pronouns, social links, and a profile picture.
            You control what's public and what's friends-only.
          </p>
        </div>
      </div>
    </div>
  `;
}

// ── Step 4: Launch Pad ───────────────────────────────────────────────────────
function step4() {
  const pubKey = (window.myIdentity && myIdentity.publicKeyHex || '').slice(0, 20);
  return `
    <h2 style="font-size:1.2rem;font-weight:800;color:#4ec87a;margin:0 0 .4rem">🚀 You're all set!</h2>
    <p style="font-size:.83rem;line-height:1.6;color:#888;margin:0 0 1rem">
      Your identity is live. Here's a quick reference you can always come back to.
    </p>

    ${pubKey ? `<div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:8px;padding:.7rem 1rem;margin-bottom:1rem;font-size:.75rem;color:#555">
      Your public ID: <code style="color:#888">${pubKey}…</code>
      <br><span style="font-size:.68rem;color:#3a3a3a">This is your address — share it freely. Your private key never leaves your device.</span>
    </div>` : ''}

    <div style="display:grid;grid-template-columns:1fr 1fr;gap:.5rem;margin-bottom:1rem">
      <button onclick="openSeedPhraseModal()" id="ob-seed-btn"
        style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:9px;padding:.75rem;
               color:#f0a500;font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🌱 View Seed Phrase<br>
        <span style="font-size:.7rem;color:#666;font-weight:400">24-word paper backup</span>
      </button>
      <button onclick="openEncryptedBackupModal()" id="ob-bkp-btn"
        style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:9px;padding:.75rem;
               color:#e0e0e0;font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🔑 Encrypted Backup<br>
        <span style="font-size:.7rem;color:#666;font-weight:400">Download a backup file</span>
      </button>
      <button onclick="openEditProfileModal()" id="ob-prof-btn"
        style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:9px;padding:.75rem;
               color:#e0e0e0;font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        ✏️ Edit Profile<br>
        <span style="font-size:.7rem;color:#666;font-weight:400">Name, bio, avatar…</span>
      </button>
      <button onclick="openKeyProtectionModal()" id="ob-kp-btn"
        style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:9px;padding:.75rem;
               color:#e0e0e0;font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🔒 Protect Your Key<br>
        <span style="font-size:.7rem;color:#666;font-weight:400">Add a passphrase lock</span>
      </button>
    </div>

    <p style="font-size:.75rem;color:#444;line-height:1.5;margin:0">
      💡 You can reopen this guide anytime via <strong style="color:#555">Help → Getting Started</strong> in the sidebar.
      Your seed phrase is always available under <strong style="color:#555">Profile → Seed Phrase</strong>.
    </p>
  `;
}

function wireStep4(overlay) {
  // Buttons on the launch pad close the wizard before opening their modals
  // so the modals don't stack on top of the wizard.
  ['ob-seed-btn','ob-bkp-btn','ob-prof-btn','ob-kp-btn'].forEach(id => {
    const btn = overlay.querySelector('#' + id);
    if (btn) {
      const originalClick = btn.getAttribute('onclick');
      btn.removeAttribute('onclick');
      btn.addEventListener('click', () => {
        localStorage.setItem(ONBOARD_DONE_KEY, '1');
        overlay.remove();
        // Small delay so overlay is removed before modal opens
        setTimeout(() => {
          if (originalClick) (new Function(originalClick))();
        }, 80);
      });
    }
  });
}

/**
 * Re-open the onboarding wizard manually (e.g. from a Help menu).
 * Clears the done flag so the full flow shows again.
 */
function reopenOnboardingWizard() {
  localStorage.removeItem(ONBOARD_DONE_KEY);
  showOnboardingWizard();
}
