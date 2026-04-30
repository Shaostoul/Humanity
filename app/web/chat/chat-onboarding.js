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
    padding:var(--space-xl);box-sizing:border-box;
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
    `<span style="width:8px;height:8px;border-radius:50%;display:inline-block;background:${i === step ? 'var(--accent)' : 'var(--border)'};margin:0 3px"></span>`
  ).join('');

  const content = [step0, step1, step2, step3, step4][step](mnemonic);
  const isLast  = step === 4;
  const isFirst = step === 0;

  return `
    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-3xl);
                width:100%;max-width:580px;font-family:'Segoe UI',system-ui,sans-serif;
                color:var(--text);max-height:92vh;overflow-y:auto;box-sizing:border-box;">

      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-2xl)">
        <div>${dots}</div>
        <span style="font-size:.7rem;color:#444">${step + 1} of 5</span>
      </div>

      ${content}

      <div style="display:flex;gap:var(--space-lg);justify-content:space-between;align-items:center;margin-top:var(--space-2xl);flex-wrap:wrap">
        <button id="ob-skip" style="background:none;border:none;color:#444;font-size:.75rem;cursor:pointer;padding:var(--space-sm) var(--space-md);text-decoration:underline">
          ${isLast ? '' : 'Skip intro'}
        </button>
        <div style="display:flex;gap:var(--space-lg)">
          ${!isFirst ? `<button id="ob-prev"
            style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);
                   padding:var(--space-md) var(--space-xl);font-size:.85rem;cursor:pointer">← Back</button>` : ''}
          <button id="ob-next"
            style="background:var(--accent);color:#000;border:none;border-radius:var(--radius);
                   padding:var(--space-md) var(--space-xl);font-size:.85rem;font-weight:700;cursor:pointer">
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
    <h2 style="font-size:1.4rem;font-weight:800;color:var(--accent);margin:0 0 var(--space-md)">👋 Welcome to Humanity!</h2>
    <p style="font-size:.9rem;line-height:1.65;color:#ccc;margin:0 0 var(--space-xl)">
      We just created a <strong style="color:var(--text)">unique digital identity</strong> for you — and we want to explain what that means
      in plain language before you dive in.
    </p>

    <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-2xl);margin-bottom:var(--space-xl)">
      <p style="font-size:.85rem;color:#ccc;margin:0 0 var(--space-lg);font-weight:600">🤔 Wait — no account? No password?</p>
      <p style="font-size:.82rem;color:var(--text-muted);line-height:1.6;margin:0">
        That's right. Instead of a username and password stored on a server somewhere, we generated a
        <strong style="color:var(--text)">secret key</strong> that lives right here in your browser.
        It's like getting a house key cut — it's yours, it's unique, and nobody else has one like it.
      </p>
    </div>

    <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-2xl)">
      <p style="font-size:.85rem;color:#ccc;margin:0 0 var(--space-lg);font-weight:600">✅ What this means for you</p>
      <ul style="font-size:.82rem;color:var(--text-muted);line-height:1.8;margin:0;padding-left:var(--space-2xl)">
        <li>No company holds your account — not us, not anyone.</li>
        <li>You can't be banned, shadowbanned, or deplatformed.</li>
        <li>Nobody reads your messages — they're encrypted.</li>
        <li>Your identity is the same on every device, as long as you back it up (we'll show you how).</li>
      </ul>
    </div>
  `;
}

// ── Step 1: Seed Phrase + Storage Options ────────────────────────────────────
function step1(mnemonic) {
  const words = mnemonic ? mnemonic.trim().split(/\s+/) : [];
  const wordGrid = words.length === 24
    ? `<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-sm);margin:var(--space-lg) 0 var(--space-lg)">
        ${words.map((w, i) => `
          <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);
                      padding:var(--space-sm) var(--space-md);display:flex;align-items:baseline;gap:var(--space-sm)">
            <span style="font-size:.58rem;color:#444;min-width:15px;text-align:right">${i+1}.</span>
            <span style="font-size:.8rem;color:var(--accent);font-weight:600">${w}</span>
          </div>`).join('')}
      </div>`
    : `<div style="background:var(--bg-secondary);border:1px dashed var(--border);border-radius:var(--radius);padding:var(--space-xl);
                   font-size:.8rem;color:var(--text-muted);margin:var(--space-lg) 0;text-align:center">
         Seed phrase unavailable in this browser. Use <strong>Encrypted Backup</strong> instead.
       </div>`;

  return `
    <h2 style="font-size:1.15rem;font-weight:800;color:var(--accent);margin:0 0 var(--space-sm)">🌱 Your 24-Word Recovery Phrase</h2>
    <p style="font-size:.8rem;line-height:1.5;color:var(--text-muted);margin:0 0 var(--space-md)">
      These 24 words <em>are</em> your identity — they can recreate your account on any device, forever.
      Think of them as a master key. <strong style="color:#ccc">Anyone who has them is you.</strong>
    </p>

    ${wordGrid}

    <p style="font-size:.75rem;color:#555;margin:0 0 var(--space-lg)">Choose at least one backup method below. Two is better.</p>

    <!-- Option A: Paper -->
    <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);margin-bottom:var(--space-md)">
      <p style="font-size:.82rem;color:var(--text);font-weight:700;margin:0 0 var(--space-sm)">📝 Paper (most secure)</p>
      <p style="font-size:.76rem;color:var(--text-muted);line-height:1.5;margin:0 0 var(--space-md)">
        Write the 24 words by hand. Store the paper somewhere safe — a fireproof box, a safe, a trusted person's home.
        Paper can't be hacked. Just don't lose it or get it wet.
      </p>
      <div style="display:flex;align-items:center;gap:var(--space-lg)">
        <button id="ob-copy-btn"
          style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);
                 padding:var(--space-sm) var(--space-xl);font-size:.75rem;cursor:pointer">📋 Copy words</button>
        <span id="ob-copy-msg" style="font-size:.7rem;color:var(--success)"></span>
      </div>
    </div>

    <!-- Option B: Encrypted file -->
    <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);margin-bottom:var(--space-md)">
      <p style="font-size:.82rem;color:var(--text);font-weight:700;margin:0 0 var(--space-sm)">💾 Encrypted file (easiest digital)</p>
      <p style="font-size:.76rem;color:var(--text-muted);line-height:1.5;margin:0 0 var(--space-md)">
        We lock the 24 words with a passphrase you choose, then download a tiny file (~1 KB).
        Store that file in your cloud (Google Drive, Dropbox, iCloud) — it's useless without the passphrase,
        so keep the passphrase in your head or a password manager. <strong style="color:var(--text-muted)">Never store the file and passphrase in the same place.</strong>
      </p>
      <div style="display:flex;gap:var(--space-md);align-items:center;flex-wrap:wrap">
        <input id="ob-enc-pass" type="password" placeholder="Choose a passphrase…" autocomplete="new-password"
          style="flex:1;min-width:140px;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);
                 padding:var(--space-sm) var(--space-lg);color:var(--text);font-size:.78rem;outline:none">
        <button id="ob-enc-btn"
          style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);
                 padding:var(--space-sm) var(--space-xl);font-size:.75rem;cursor:pointer;white-space:nowrap">💾 Download</button>
        <span id="ob-enc-msg" style="font-size:.7rem;color:var(--success);width:100%"></span>
      </div>
    </div>

    <!-- Option C: Password manager -->
    <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl)">
      <p style="font-size:.82rem;color:var(--text);font-weight:700;margin:0 0 var(--space-sm)">🔐 Password manager (most accessible)</p>
      <p style="font-size:.76rem;color:var(--text-muted);line-height:1.5;margin:0 0 var(--space-md)">
        Open <a href="https://bitwarden.com" target="_blank" rel="noopener"
          style="color:var(--accent)">Bitwarden</a>, <strong style="color:var(--text-muted)">1Password</strong>, or any password manager.
        Create a new <em>Secure Note</em> called "Humanity seed phrase" and paste the 24 words there.
        Password managers are encrypted, sync across devices, and survive losing your phone or laptop.
        <br><strong style="color:var(--text-muted)">Bitwarden is free and open source.</strong>
      </p>
      <div style="display:flex;align-items:center;gap:var(--space-lg)">
        <button id="ob-pm-btn"
          style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);
                 padding:var(--space-sm) var(--space-xl);font-size:.75rem;cursor:pointer">📋 Copy for password manager</button>
        <span id="ob-pm-msg" style="font-size:.7rem;color:var(--success)"></span>
      </div>
    </div>
  `;
}

function wireStep1(overlay, mnemonic) {
  // Paper copy
  const copyBtn = overlay.querySelector('#ob-copy-btn');
  const copyMsg = overlay.querySelector('#ob-copy-msg');
  if (copyBtn && mnemonic) {
    copyBtn.addEventListener('click', () => {
      navigator.clipboard.writeText(mnemonic).then(() => {
        copyMsg.textContent = '✓ Copied — write them down, then clear your clipboard.';
        copyBtn.textContent = 'Copied!';
      }).catch(() => { copyMsg.textContent = 'Copy failed — select the words manually.'; });
    });
  }

  // Encrypted file download
  const encBtn  = overlay.querySelector('#ob-enc-btn');
  const encPass = overlay.querySelector('#ob-enc-pass');
  const encMsg  = overlay.querySelector('#ob-enc-msg');
  if (encBtn && mnemonic) {
    encBtn.addEventListener('click', async () => {
      const pass = encPass ? encPass.value.trim() : '';
      if (pass.length < 8) { encMsg.innerHTML = '<span style="color:var(--danger)">Passphrase must be at least 8 characters.</span>'; return; }
      encBtn.disabled = true; encBtn.textContent = 'Encrypting…'; encMsg.textContent = '';
      try {
        await downloadEncryptedMnemonic(mnemonic, pass);
        encMsg.textContent = '✓ File downloaded — store it in your cloud, keep the passphrase separate.';
        encBtn.textContent = 'Downloaded!';
      } catch(e) {
        encMsg.innerHTML = `<span style="color:var(--danger)">${e.message}</span>`;
        encBtn.disabled = false; encBtn.textContent = '💾 Download';
      }
    });
  }

  // Password manager copy
  const pmBtn = overlay.querySelector('#ob-pm-btn');
  const pmMsg = overlay.querySelector('#ob-pm-msg');
  if (pmBtn && mnemonic) {
    pmBtn.addEventListener('click', () => {
      navigator.clipboard.writeText(mnemonic).then(() => {
        pmMsg.textContent = '✓ Copied — paste into a Secure Note in your password manager.';
        pmBtn.textContent = 'Copied!';
      }).catch(() => { pmMsg.textContent = 'Copy failed — select the words manually.'; });
    });
  }
}

// ── Step 2: What is Humanity ─────────────────────────────────────────────────
function step2() {
  return `
    <h2 style="font-size:1.2rem;font-weight:800;color:var(--accent);margin:0 0 var(--space-md)">🌍 What is Humanity?</h2>
    <p style="font-size:.83rem;line-height:1.6;color:var(--text-muted);margin:0 0 var(--space-xl)">
      Humanity is a cooperative platform — not owned by any company, not funded by ads, and not watching you.
    </p>

    <div style="display:grid;gap:var(--space-lg)">
      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl)">
        <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">💬 Chat that's actually private</p>
        <p style="font-size:.8rem;color:var(--text-muted);line-height:1.5;margin:0">
          Messages between you and another person are encrypted end-to-end — meaning only you two can read them.
          Not us, not the server, not your ISP. It's like passing a note in an envelope no one else can open.
        </p>
      </div>

      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl)">
        <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">🚫 No algorithm. No ads. No engagement bait.</p>
        <p style="font-size:.8rem;color:var(--text-muted);line-height:1.5;margin:0">
          There's no feed tuned to keep you angry or scrolling. You see what you choose to see.
          The platform doesn't profit from your attention.
        </p>
      </div>

      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl)">
        <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">🤝 Owned by everyone, run by the community</p>
        <p style="font-size:.8rem;color:var(--text-muted);line-height:1.5;margin:0">
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
    <h2 style="font-size:1.2rem;font-weight:800;color:var(--accent);margin:0 0 var(--space-md)">🤝 Meeting People</h2>
    <p style="font-size:.83rem;line-height:1.6;color:var(--text-muted);margin:0 0 var(--space-xl)">
      Unlike social media, you build your network intentionally — no followers game, no public follower counts.
    </p>

    <div style="display:grid;gap:var(--space-lg)">
      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);display:flex;gap:var(--space-lg)">
        <span style="font-size:1.4rem;flex-shrink:0">💬</span>
        <div>
          <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">Jump into a channel</p>
          <p style="font-size:.79rem;color:var(--text-muted);line-height:1.5;margin:0">
            Channels on the left sidebar are open rooms — like a coffee shop with a topic.
            Just start talking. Nobody requires an introduction.
          </p>
        </div>
      </div>

      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);display:flex;gap:var(--space-lg)">
        <span style="font-size:1.4rem;flex-shrink:0">📮</span>
        <div>
          <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">Direct Messages</p>
          <p style="font-size:.79rem;color:var(--text-muted);line-height:1.5;margin:0">
            Click any username to open a private, encrypted conversation with that person. Only the two of you can read it.
          </p>
        </div>
      </div>

      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);display:flex;gap:var(--space-lg)">
        <span style="font-size:1.4rem;flex-shrink:0">🪪</span>
        <div>
          <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">Contact Cards (P2P)</p>
          <p style="font-size:.79rem;color:var(--text-muted);line-height:1.5;margin:0">
            In the Contacts tab you can share a QR code or a short link.
            Scanning it lets two people connect directly, even without the relay server in the middle.
          </p>
        </div>
      </div>

      <div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-xl) var(--space-xl);display:flex;gap:var(--space-lg)">
        <span style="font-size:1.4rem;flex-shrink:0">✏️</span>
        <div>
          <p style="font-size:.85rem;color:var(--text);font-weight:600;margin:0 0 var(--space-sm)">Set up your profile</p>
          <p style="font-size:.79rem;color:var(--text-muted);line-height:1.5;margin:0">
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
    <h2 style="font-size:1.2rem;font-weight:800;color:var(--success);margin:0 0 var(--space-md)">🚀 You're all set!</h2>
    <p style="font-size:.83rem;line-height:1.6;color:var(--text-muted);margin:0 0 var(--space-xl)">
      Your identity is live. Here's a quick reference you can always come back to.
    </p>

    ${pubKey ? `<div style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-lg) var(--space-xl);margin-bottom:var(--space-xl);font-size:.75rem;color:#555">
      Your public ID: <code style="color:var(--text-muted)">${pubKey}…</code>
      <br><span style="font-size:.68rem;color:#3a3a3a">This is your address — share it freely. Your private key never leaves your device.</span>
    </div>` : ''}

    <div style="display:grid;grid-template-columns:1fr 1fr;gap:var(--space-md);margin-bottom:var(--space-xl)">
      <button onclick="openSeedPhraseModal()" id="ob-seed-btn"
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-lg);
               color:var(--accent);font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🌱 View Seed Phrase<br>
        <span style="font-size:.7rem;color:var(--text-muted);font-weight:400">24-word paper backup</span>
      </button>
      <button onclick="openEncryptedBackupModal()" id="ob-bkp-btn"
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-lg);
               color:var(--text);font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🔑 Encrypted Backup<br>
        <span style="font-size:.7rem;color:var(--text-muted);font-weight:400">Download a backup file</span>
      </button>
      <button onclick="openEditProfileModal()" id="ob-prof-btn"
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-lg);
               color:var(--text);font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        ✏️ Edit Profile<br>
        <span style="font-size:.7rem;color:var(--text-muted);font-weight:400">Name, bio, avatar…</span>
      </button>
      <button onclick="openKeyProtectionModal()" id="ob-kp-btn"
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-lg);
               color:var(--text);font-size:.8rem;font-weight:600;cursor:pointer;text-align:left">
        🔒 Protect Your Key<br>
        <span style="font-size:.7rem;color:var(--text-muted);font-weight:400">Add a passphrase lock</span>
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
