/**
 * Humanity Network — Client-side Ed25519 cryptography (inlined).
 * Uses Web Crypto API. Keys persisted in IndexedDB.
 */
const DB_NAME = 'humanity-keys';
const DB_VERSION = 1;
const STORE_NAME = 'identity';
const KEY_ID = 'primary';

function openKeyDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => { req.result.createObjectStore(STORE_NAME, { keyPath: 'id' }); };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function storeKeypair(db, publicKeyHex, keypair) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    tx.objectStore(STORE_NAME).put({ id: KEY_ID, publicKeyHex, privateKey: keypair.privateKey, publicKey: keypair.publicKey });
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function loadKeypair(db) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const req = tx.objectStore(STORE_NAME).get(KEY_ID);
    req.onsuccess = () => resolve(req.result || null);
    req.onerror = () => reject(req.error);
  });
}

function bufToHex(buf) {
  return Array.from(new Uint8Array(buf)).map(b => b.toString(16).padStart(2, '0')).join('');
}

function hexToBuf(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  return bytes;
}

async function supportsEd25519() {
  try { await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']); return true; } catch (e) { return false; }
}

async function generateKeypair() {
  const keypair = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
  const rawPub = await crypto.subtle.exportKey('raw', keypair.publicKey);
  return { publicKeyHex: bufToHex(rawPub), privateKey: keypair.privateKey, publicKey: keypair.publicKey, isNew: true };
}

async function requestPersistentStorage() {
  try {
    if (navigator.storage && navigator.storage.persist) {
      const granted = await navigator.storage.persist();
      console.log('Persistent storage:', granted ? 'granted' : 'denied');
      return granted;
    }
  } catch (e) { console.warn('Persistent storage request failed:', e); }
  return false;
}

/** Save key backup to localStorage as redundancy (IndexedDB can be evicted). */
async function saveKeyBackupToLocalStorage(publicKeyHex, privateKey) {
  try {
    if (!privateKey) return;
    const exported = await crypto.subtle.exportKey('pkcs8', privateKey);
    const b64 = btoa(String.fromCharCode(...new Uint8Array(exported)));
    localStorage.setItem('humanity_key_backup', JSON.stringify({ publicKeyHex, privateKeyPkcs8: b64 }));
    console.log('Key backup saved to localStorage');
  } catch (e) { console.warn('Key backup to localStorage failed:', e); }
}

/** Try to restore key from localStorage backup. */
async function restoreKeyFromLocalStorage() {
  try {
    const raw = localStorage.getItem('humanity_key_backup');
    if (!raw) return null;
    const { publicKeyHex, privateKeyPkcs8 } = JSON.parse(raw);
    if (!publicKeyHex || !privateKeyPkcs8) return null;
    const pkcs8Buf = Uint8Array.from(atob(privateKeyPkcs8), c => c.charCodeAt(0));
    const privateKey = await crypto.subtle.importKey('pkcs8', pkcs8Buf, 'Ed25519', true, ['sign']);
    const publicKey = await crypto.subtle.importKey('raw', hexToBuf(publicKeyHex), 'Ed25519', true, ['verify']);
    console.log('Restored identity from localStorage backup:', publicKeyHex.substring(0, 16) + '…');
    // Re-save to IndexedDB
    try {
      const db = await openKeyDB();
      await storeKeypair(db, publicKeyHex, { privateKey, publicKey });
      console.log('Re-saved restored key to IndexedDB');
    } catch (e) { console.warn('Could not re-save to IndexedDB:', e); }
    return { publicKeyHex, privateKey, publicKey, canSign: true, isNew: false, restored: true };
  } catch (e) { console.warn('localStorage key restore failed:', e); return null; }
}

async function getOrCreateIdentity() {
  const hasEd25519 = await supportsEd25519();
  if (!hasEd25519) {
    console.warn('Ed25519 not supported — falling back to random key');
    let key = localStorage.getItem('humanity_key');
    if (!key) { const bytes = new Uint8Array(32); crypto.getRandomValues(bytes); key = bufToHex(bytes); localStorage.setItem('humanity_key', key); }
    return { publicKeyHex: key, privateKey: null, publicKey: null, canSign: false };
  }
  try {
    const db = await openKeyDB();
    const stored = await loadKeypair(db);
    if (stored && stored.privateKey && stored.publicKeyHex) {
      console.log('Loaded existing identity:', stored.publicKeyHex.substring(0, 16) + '…');
      // Ensure localStorage backup exists
      saveKeyBackupToLocalStorage(stored.publicKeyHex, stored.privateKey);
      return { publicKeyHex: stored.publicKeyHex, privateKey: stored.privateKey, publicKey: stored.publicKey, canSign: true, isNew: false };
    }
    // IndexedDB empty — try localStorage backup before generating new key
    const restored = await restoreKeyFromLocalStorage();
    if (restored) return restored;
    // No plaintext backup — check for passphrase-wrapped key
    if (isKeyWrapped()) {
      const pp = window.prompt('Your identity is passphrase-protected.\nEnter your passphrase to unlock it:');
      if (pp) {
        try {
          const wrapped = await loadWrappedKey(pp);
          if (wrapped) {
            const db2 = await openKeyDB();
            await storeKeypair(db2, wrapped.publicKeyHex, wrapped);
            await saveKeyBackupToLocalStorage(wrapped.publicKeyHex, wrapped.privateKey);
            console.log('Unlocked identity from wrapped key:', wrapped.publicKeyHex.substring(0, 16) + '…');
            return wrapped;
          }
        } catch (e) {
          alert('Could not unlock identity: ' + e.message + '\nA new identity will be generated.');
        }
      }
    }
    // Generate new identity
    const kp = await generateKeypair();
    await storeKeypair(db, kp.publicKeyHex, kp);
    // Request persistent storage for new identities
    await requestPersistentStorage();
    // Save backup to localStorage
    await saveKeyBackupToLocalStorage(kp.publicKeyHex, kp.privateKey);
    console.log('Generated new identity:', kp.publicKeyHex.substring(0, 16) + '…');
    return { publicKeyHex: kp.publicKeyHex, privateKey: kp.privateKey, publicKey: kp.publicKey, canSign: true, isNew: true };
  } catch (e) {
    console.error('Identity setup failed:', e);
    // Try localStorage backup as last resort
    const restored = await restoreKeyFromLocalStorage();
    if (restored) return restored;
    let key = localStorage.getItem('humanity_key');
    if (!key) { const bytes = new Uint8Array(32); crypto.getRandomValues(bytes); key = bufToHex(bytes); localStorage.setItem('humanity_key', key); }
    return { publicKeyHex: key, privateKey: null, publicKey: null, canSign: false };
  }
}

async function signMessage(privateKey, content, timestamp) {
  if (!privateKey) return null;
  try {
    const payload = `${content}\n${timestamp}`;
    const sig = await crypto.subtle.sign('Ed25519', privateKey, new TextEncoder().encode(payload));
    return bufToHex(sig);
  } catch (e) { console.error('Signing failed:', e); return null; }
}

async function verifyMessage(publicKeyHex, signatureHex, content, timestamp) {
  try {
    const pubKey = await crypto.subtle.importKey('raw', hexToBuf(publicKeyHex), 'Ed25519', true, ['verify']);
    const payload = `${content}\n${timestamp}`;
    return await crypto.subtle.verify('Ed25519', pubKey, hexToBuf(signatureHex), new TextEncoder().encode(payload));
  } catch (e) { console.error('Verification failed:', e); return false; }
}

// ── Identity Export/Import ──

/** Extract the 32-byte Ed25519 seed from a PKCS8 export. */
function extractSeedFromPkcs8(pkcs8Buf) {
  // PKCS8 for Ed25519: 48 bytes total. The 32-byte seed starts at offset 16.
  const bytes = new Uint8Array(pkcs8Buf);
  if (bytes.length === 48) {
    return bytes.slice(16, 48);
  }
  // Fallback: try last 32 bytes
  return bytes.slice(bytes.length - 32);
}

/** Export the current identity as a JSON backup object. Returns null if non-extractable. */
async function exportIdentityJSON(name) {
  if (!myIdentity || !myIdentity.privateKey) return null;
  try {
    const pkcs8 = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
    const seed = extractSeedFromPkcs8(pkcs8);
    const exportData = {
      name: name || myName,
      publicKey: myIdentity.publicKeyHex,
      privateKey: bufToHex(seed),
      exportedAt: new Date().toISOString(),
      note: "Keep this file safe. Anyone with it can impersonate you."
    };
    // Include ECDH key for E2EE DMs if available.
    if (myEcdhKeyPair) {
      try {
        const ecdhPkcs8 = await crypto.subtle.exportKey('pkcs8', myEcdhKeyPair.privateKey);
        exportData.ecdhPrivateKey = btoa(String.fromCharCode(...new Uint8Array(ecdhPkcs8)));
        exportData.ecdhPublicKey = myEcdhPublicBase64;
      } catch (e) { console.warn('ECDH export failed:', e); }
    }
    return exportData;
  } catch (e) {
    console.error('Export failed (key may be non-extractable):', e);
    return null;
  }
}

/** Download identity as a JSON file. */
function openDevicePanel() {
  document.getElementById('device-panel-overlay').classList.add('open');
  requestDeviceList();
}

function requestDeviceList() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'device_list_request' }));
  }
}

function renderDeviceList(devices) {
  const container = document.getElementById('device-list');
  if (!container) return;
  container.innerHTML = '';
  devices.forEach(d => {
    const div = document.createElement('div');
    div.style.cssText = 'padding:var(--space-xl);margin-bottom:var(--space-md);background:var(--bg-secondary);border-radius:var(--radius);border:1px solid var(--border);';
    const keyShort = d.public_key.substring(0, 16) + '…';
    const statusDot = d.is_online ? '🟢' : '⚫';
    const currentBadge = d.is_current ? ' <span style="color:var(--accent);font-size:0.7rem">(this device)</span>' : '';
    const date = new Date(d.registered_at).toLocaleDateString();
    div.innerHTML = `
      <div style="display:flex;align-items:center;gap:var(--space-md);margin-bottom:var(--space-sm)">
        <span>${statusDot}</span>
        <code style="font-size:0.75rem;color:var(--text-muted)">${keyShort}</code>
        ${currentBadge}
      </div>
      <div style="display:flex;align-items:center;gap:var(--space-sm);margin-bottom:var(--space-sm)">
        <input type="text" value="${(d.label || '').replace(/"/g, '&quot;')}"
               placeholder="Label (e.g. PC, Phone, Laptop)"
               maxlength="32"
               style="flex:1;font-size:0.8rem;padding:var(--space-xs) var(--space-md);background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--text)"
               onchange="labelDevice('${d.public_key}', this.value)">
        ${!d.is_current ? `<button onclick="revokeDevice('${d.public_key.substring(0, 8)}')"
                style="font-size:0.75rem;padding:var(--space-xs) var(--space-xl);background:var(--danger);border:none;border-radius:var(--radius-sm);color:#fff;cursor:pointer"
                title="Remove this device">✕</button>` : ''}
      </div>
      <div style="font-size:0.7rem;color:var(--text-muted)">Registered: ${date}</div>
    `;
    container.appendChild(div);
  });
}

/**
 * Show a QR code + copyable JSON so the user can transfer their identity
 * to a new device by scanning or pasting. The JSON is the same format
 * used by downloadIdentityBackup() / importIdentityBackup().
 */
async function openLinkDeviceModal() {
  const name = (typeof myName !== 'undefined' && myName) || localStorage.getItem('humanity_name') || 'user';
  const data = await exportIdentityJSON(name);
  if (!data) {
    alert('Cannot export identity — this key was created before backup support was added. Please download a backup from the 🔐 Backup button instead.');
    return;
  }
  const json = JSON.stringify(data, null, 2);
  const overlay = document.createElement('div');
  overlay.id = 'link-device-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:7000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);';
  overlay.innerHTML = `
    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-2xl);width:100%;max-width:440px;font-family:'Segoe UI',system-ui,sans-serif;color:var(--text);max-height:90vh;overflow-y:auto;">
      <h2 style="font-size:1rem;font-weight:700;color:var(--accent);margin-bottom:var(--space-md)">📱 Link New Device</h2>
      <p style="font-size:.75rem;color:var(--accent);background:rgba(240,165,0,.08);border:1px solid rgba(240,165,0,.2);border-radius:var(--radius);padding:var(--space-xl) var(--space-lg);margin-bottom:var(--space-xl);line-height:1.55">
        ⚠️ <strong>Private:</strong> this QR code contains your private key. Only scan it in a physically private location. Close this modal immediately after use.
      </p>
      <canvas id="link-device-qr" style="display:block;margin:0 auto var(--space-lg);border-radius:var(--radius);background:#fff;padding:6px;max-width:220px;width:100%;height:auto;"></canvas>
      <p style="font-size:.72rem;color:var(--text-muted);text-align:center;margin-bottom:var(--space-lg)">Scan with your new device's camera. Or copy the JSON below.</p>
      <textarea id="link-device-json" readonly rows="5"
        style="width:100%;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-xl);font-size:.7rem;color:var(--text-muted);font-family:monospace;resize:none;margin-bottom:var(--space-lg)">${json.replace(/</g,'&lt;')}</textarea>
      <div style="display:flex;gap:var(--space-xl);justify-content:flex-end">
        <button onclick="navigator.clipboard.writeText(document.getElementById('link-device-json').value).then(()=>this.textContent='Copied!')"
          style="background:var(--bg-input);border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer;">📋 Copy JSON</button>
        <button onclick="document.getElementById('link-device-overlay').remove()"
          style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer;">Close</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  // Render QR code using the shared qrcode.js helper from chat-p2p.js.
  // Falls back to text if qrcode.js isn't available (e.g. pages other than /chat).
  if (typeof renderQrCode === 'function') {
    renderQrCode('link-device-qr', json);
  } else if (typeof qrcode !== 'undefined') {
    try {
      const qr = qrcode(0, 'L');
      qr.addData(json);
      qr.make();
      const canvas = document.getElementById('link-device-qr');
      const size = Math.min(canvas.parentElement.offsetWidth - 50, 220);
      canvas.width = canvas.height = size;
      const ctx = canvas.getContext('2d');
      const cells = qr.getModuleCount();
      const cell = size / cells;
      for (let r = 0; r < cells; r++) {
        for (let c = 0; c < cells; c++) {
          ctx.fillStyle = qr.isDark(r, c) ? '#000' : '#fff';
          ctx.fillRect(c * cell, r * cell, cell, cell);
        }
      }
    } catch (e) {
      document.getElementById('link-device-qr').style.display = 'none';
    }
  } else {
    document.getElementById('link-device-qr').style.display = 'none';
  }
}

function labelDevice(publicKey, label) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'device_label', public_key: publicKey, label: label }));
  }
}

function revokeDevice(keyPrefix) {
  if (!confirm('Revoke this device? It will be disconnected and removed from your account.')) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'device_revoke', key_prefix: keyPrefix }));
  }
}

async function downloadIdentityBackup(name) {
  const data = await exportIdentityJSON(name);
  if (!data) {
    addSystemMessage("⚠️ This key was created before backup support. Register a new name to get an exportable identity.");
    return false;
  }
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `humanity-identity-${data.name}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return true;
}

/** Import an identity from a JSON backup file. Returns { publicKeyHex, privateKey, publicKey, name } or throws. */
async function importIdentityFromJSON(jsonData) {
  // Validate required fields
  if (!jsonData.publicKey || !jsonData.privateKey || !jsonData.name) {
    throw new Error("Invalid backup file: missing required fields (name, publicKey, privateKey).");
  }
  if (jsonData.publicKey.length !== 64 || jsonData.privateKey.length !== 64) {
    throw new Error("Invalid backup file: keys must be 64-character hex strings.");
  }

  // Reconstruct the Ed25519 keypair from the seed
  const seedBytes = hexToBuf(jsonData.privateKey);

  // Build PKCS8 wrapper around the 32-byte seed
  const pkcs8Prefix = new Uint8Array([
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06,
    0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20
  ]);
  const pkcs8 = new Uint8Array(48);
  pkcs8.set(pkcs8Prefix, 0);
  pkcs8.set(seedBytes, 16);

  const privateKey = await crypto.subtle.importKey(
    'pkcs8', pkcs8, 'Ed25519', true, ['sign']
  );

  // Import the public key
  const pubBytes = hexToBuf(jsonData.publicKey);
  const publicKey = await crypto.subtle.importKey(
    'raw', pubBytes, 'Ed25519', true, ['verify']
  );

  // Verify the imported keypair works by signing and verifying a test message
  const testMsg = new TextEncoder().encode('test');
  const testSig = await crypto.subtle.sign('Ed25519', privateKey, testMsg);
  const valid = await crypto.subtle.verify('Ed25519', publicKey, testSig, testMsg);
  if (!valid) {
    throw new Error("Key verification failed: the public and private keys don't match.");
  }

  // Store in IndexedDB
  const db = await openKeyDB();
  await storeKeypair(db, jsonData.publicKey, { privateKey, publicKey });

  // Set name in localStorage
  localStorage.setItem('humanity_name', jsonData.name);

  return {
    publicKeyHex: jsonData.publicKey,
    privateKey,
    publicKey,
    canSign: true,
    name: jsonData.name
  };
}

// ── Seed Phrase Display (paper backup) ──
// Goal: let users write their identity key on paper in a readable format.
// Displays the 32-byte Ed25519 seed as 8 groups of 4 hex chars (like a PIN
// sheet) — easy to write down, hard to misread. No word list required.

/**
 * Return the current identity seed as a formatted "paper phrase":
 * 8 groups of 4 hex chars separated by dashes.
 * Example: a1b2-c3d4-e5f6-a7b8-c9d0-e1f2-a3b4-c5d6
 * @returns {Promise<string|null>}
 */
async function getSeedPhrase() {
  if (!myIdentity || !myIdentity.privateKey) return null;
  try {
    const pkcs8 = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
    const seed = extractSeedFromPkcs8(pkcs8);
    const hex = bufToHex(seed);
    // Split into 8 groups of 4 hex chars
    return hex.match(/.{1,8}/g).map(g => g.match(/.{1,4}/g).join('-')).join('  ');
  } catch (e) { return null; }
}

// ── Passphrase-Encrypted Identity Backup ──
// Goal: let users protect their identity backup file with a passphrase so that
// losing the file to an attacker doesn't compromise their identity.

/**
 * Derive an AES-256-GCM key from a user passphrase + random salt using PBKDF2.
 * Uses 600,000 iterations (OWASP 2023 recommendation for SHA-256).
 * @param {string} passphrase
 * @param {Uint8Array} salt
 * @returns {Promise<CryptoKey>}
 */
async function deriveKeyFromPassphrase(passphrase, salt) {
  const enc = new TextEncoder();
  const keyMaterial = await crypto.subtle.importKey(
    'raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']
  );
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 600000, hash: 'SHA-256' },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt']
  );
}

/**
 * Export the current identity as a passphrase-encrypted JSON file download.
 * The file is safe to store in cloud drives — it's useless without the passphrase.
 * @param {string} passphrase - User-chosen passphrase to protect the backup.
 */
async function exportEncryptedIdentityBackup(passphrase) {
  if (!myIdentity || !myIdentity.privateKey) throw new Error('No identity loaded.');
  if (!passphrase || passphrase.length < 8) throw new Error('Passphrase must be at least 8 characters.');

  const pkcs8 = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
  const seed = extractSeedFromPkcs8(pkcs8);
  const plain = JSON.stringify({ v: 1, name: myName, publicKey: myIdentity.publicKeyHex, privateKey: bufToHex(seed) });

  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv   = crypto.getRandomValues(new Uint8Array(12));
  const wrapKey = await deriveKeyFromPassphrase(passphrase, salt);
  const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, wrapKey, new TextEncoder().encode(plain));

  const bundle = {
    v: 1, encrypted: true,
    kdf: 'PBKDF2-SHA256-600k',
    cipher: 'AES-256-GCM',
    salt: bufToHex(salt),
    iv:   bufToHex(iv),
    ct:   btoa(String.fromCharCode(...new Uint8Array(ct))),
    exportedAt: new Date().toISOString(),
  };

  const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `humanity-backup-${(myName || 'identity').replace(/[^a-z0-9]/gi, '_')}-encrypted.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return true;
}

/**
 * Import an identity from a passphrase-encrypted backup file.
 * Handles both encrypted bundles (v1 with `encrypted: true`) and
 * plain JSON backups created by downloadIdentityBackup().
 * @param {object} parsed - Parsed JSON from the backup file.
 * @param {string} [passphrase] - Required only for encrypted backups.
 * @returns {Promise<object>} Identity object ready to use.
 */
async function importIdentityBackup(parsed, passphrase) {
  if (parsed.encrypted) {
    if (!passphrase) throw new Error('This backup is encrypted. Please enter your passphrase.');
    const salt = hexToBuf(parsed.salt);
    const iv   = hexToBuf(parsed.iv);
    const ct   = Uint8Array.from(atob(parsed.ct), c => c.charCodeAt(0));
    const wrapKey = await deriveKeyFromPassphrase(passphrase, salt);
    let plainBuf;
    try {
      plainBuf = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, wrapKey, ct);
    } catch (e) {
      throw new Error('Wrong passphrase or corrupted backup file.');
    }
    const inner = JSON.parse(new TextDecoder().decode(plainBuf));
    return importIdentityFromJSON(inner);
  }
  // Plain backup
  return importIdentityFromJSON(parsed);
}

// ══════════════════════════════════════════════════════════════════════════════
// Passphrase-Protected Key Storage (keys at rest)
// ══════════════════════════════════════════════════════════════════════════════

const WRAPPED_KEY_LS   = 'humanity_key_wrapped';
const WRAPPED_ECDH_LS  = 'humanity_ecdh_wrapped';

/**
 * Encrypt the current Ed25519 (and optionally ECDH) private keys with a
 * passphrase and persist them in localStorage as AES-256-GCM blobs.
 * Wrapped keys are safe to leave in localStorage even if DevTools are open —
 * they are useless without the passphrase.
 * @param {string} passphrase - User-chosen passphrase (≥ 8 chars)
 */
async function wrapAndStoreKey(passphrase) {
  if (!myIdentity || !myIdentity.privateKey) throw new Error('No identity loaded.');
  if (!passphrase || passphrase.length < 8)  throw new Error('Passphrase must be at least 8 characters.');

  // Wrap Ed25519 key
  const pkcs8    = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
  const salt     = crypto.getRandomValues(new Uint8Array(16));
  const iv       = crypto.getRandomValues(new Uint8Array(12));
  const wrapKey  = await deriveKeyFromPassphrase(passphrase, salt);
  const ct       = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, wrapKey, pkcs8);
  localStorage.setItem(WRAPPED_KEY_LS, JSON.stringify({
    v: 1, publicKeyHex: myIdentity.publicKeyHex,
    salt: bufToHex(salt), iv: bufToHex(iv),
    ct: btoa(String.fromCharCode(...new Uint8Array(ct))),
    wrappedAt: new Date().toISOString(),
  }));

  // Wrap ECDH key if available
  if (myEcdhKeyPair) {
    try {
      const ePkcs8   = await crypto.subtle.exportKey('pkcs8', myEcdhKeyPair.privateKey);
      const eSalt    = crypto.getRandomValues(new Uint8Array(16));
      const eIv      = crypto.getRandomValues(new Uint8Array(12));
      const eWrapKey = await deriveKeyFromPassphrase(passphrase, eSalt);
      const eCt      = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: eIv }, eWrapKey, ePkcs8);
      localStorage.setItem(WRAPPED_ECDH_LS, JSON.stringify({
        v: 1, publicKeyRaw: myEcdhPublicBase64,
        salt: bufToHex(eSalt), iv: bufToHex(eIv),
        ct: btoa(String.fromCharCode(...new Uint8Array(eCt))),
      }));
    } catch (e) { console.warn('ECDH wrap failed:', e); }
  }
  return true;
}

/**
 * Decrypt a wrapped Ed25519 keypair from localStorage.
 * Throws 'Wrong passphrase.' if the passphrase is incorrect.
 * @param {string} passphrase
 * @returns {Promise<object|null>} Identity object or null if no wrapped key exists.
 */
async function loadWrappedKey(passphrase) {
  const raw = localStorage.getItem(WRAPPED_KEY_LS);
  if (!raw) return null;
  const b = JSON.parse(raw);
  const wrapKey = await deriveKeyFromPassphrase(passphrase, hexToBuf(b.salt));
  let pkcs8;
  try {
    pkcs8 = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: hexToBuf(b.iv) },
      wrapKey,
      Uint8Array.from(atob(b.ct), c => c.charCodeAt(0))
    );
  } catch { throw new Error('Wrong passphrase.'); }
  const privateKey = await crypto.subtle.importKey('pkcs8', pkcs8, 'Ed25519', true, ['sign']);
  const publicKey  = await crypto.subtle.importKey('raw', hexToBuf(b.publicKeyHex), 'Ed25519', true, ['verify']);
  // Also restore ECDH key if wrapped
  try {
    const er = localStorage.getItem(WRAPPED_ECDH_LS);
    if (er) {
      const eb      = JSON.parse(er);
      const eWrapKey = await deriveKeyFromPassphrase(passphrase, hexToBuf(eb.salt));
      const ePkcs8  = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: hexToBuf(eb.iv) },
        eWrapKey,
        Uint8Array.from(atob(eb.ct), c => c.charCodeAt(0))
      );
      const ePriv = await crypto.subtle.importKey('pkcs8', ePkcs8, { name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveKey']);
      const ePub  = await crypto.subtle.importKey('raw', Uint8Array.from(atob(eb.publicKeyRaw), c => c.charCodeAt(0)), { name: 'ECDH', namedCurve: 'P-256' }, true, []);
      myEcdhKeyPair = { privateKey: ePriv, publicKey: ePub };
      myEcdhPublicBase64 = eb.publicKeyRaw;
    }
  } catch (e) { console.warn('ECDH unwrap failed:', e); }
  return { publicKeyHex: b.publicKeyHex, privateKey, publicKey, canSign: true, isNew: false };
}

/** Returns true if the identity is currently protected by a passphrase. */
function isKeyWrapped() { return !!localStorage.getItem(WRAPPED_KEY_LS); }

/**
 * Remove the plaintext localStorage backup after confirming the wrapped copy
 * exists. Call only after wrapAndStoreKey() succeeds.
 */
function removeUnwrappedKey() {
  if (!isKeyWrapped()) throw new Error('Enable key protection before removing the plain backup.');
  localStorage.removeItem('humanity_key_backup');
  localStorage.removeItem('humanity_ecdh_backup');
}

// ══════════════════════════════════════════════════════════════════════════════
// E2EE: ECDH P-256 + AES-256-GCM for end-to-end encrypted DMs
// ══════════════════════════════════════════════════════════════════════════════

const ECDH_DB_STORE = 'ecdh_identity';
let myEcdhKeyPair = null; // { publicKey, privateKey }
let myEcdhPublicBase64 = null; // base64-encoded raw public key for transmission

/** Generate or load ECDH P-256 keypair for E2EE DMs. */
async function getOrCreateEcdhKeypair() {
  try {
    // Try loading from IndexedDB
    const db = await openKeyDB();
    const stored = await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const req = tx.objectStore(STORE_NAME).get('ecdh');
      req.onsuccess = () => resolve(req.result || null);
      req.onerror = () => reject(req.error);
    });
    if (stored && stored.privateKey && stored.publicKey) {
      myEcdhKeyPair = { privateKey: stored.privateKey, publicKey: stored.publicKey };
      const raw = await crypto.subtle.exportKey('raw', stored.publicKey);
      myEcdhPublicBase64 = btoa(String.fromCharCode(...new Uint8Array(raw)));
      console.log('Loaded existing ECDH key');
      return;
    }
  } catch (e) { console.warn('ECDH IndexedDB load failed:', e); }

  // Try localStorage backup
  try {
    const backup = localStorage.getItem('humanity_ecdh_backup');
    if (backup) {
      const { publicKeyRaw, privateKeyPkcs8 } = JSON.parse(backup);
      const privBuf = Uint8Array.from(atob(privateKeyPkcs8), c => c.charCodeAt(0));
      const pubBuf = Uint8Array.from(atob(publicKeyRaw), c => c.charCodeAt(0));
      const privateKey = await crypto.subtle.importKey('pkcs8', privBuf, { name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveKey']);
      const publicKey = await crypto.subtle.importKey('raw', pubBuf, { name: 'ECDH', namedCurve: 'P-256' }, true, []);
      myEcdhKeyPair = { privateKey, publicKey };
      myEcdhPublicBase64 = publicKeyRaw;
      console.log('Restored ECDH key from localStorage');
      // Re-save to IndexedDB
      try {
        const db = await openKeyDB();
        const tx = db.transaction(STORE_NAME, 'readwrite');
        tx.objectStore(STORE_NAME).put({ id: 'ecdh', privateKey, publicKey });
      } catch (e) {}
      return;
    }
  } catch (e) { console.warn('ECDH localStorage restore failed:', e); }

  // Generate new
  try {
    const kp = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveKey']);
    myEcdhKeyPair = { privateKey: kp.privateKey, publicKey: kp.publicKey };
    const raw = await crypto.subtle.exportKey('raw', kp.publicKey);
    myEcdhPublicBase64 = btoa(String.fromCharCode(...new Uint8Array(raw)));

    // Store in IndexedDB
    try {
      const db = await openKeyDB();
      const tx = db.transaction(STORE_NAME, 'readwrite');
      tx.objectStore(STORE_NAME).put({ id: 'ecdh', privateKey: kp.privateKey, publicKey: kp.publicKey });
    } catch (e) {}

    // Backup to localStorage
    try {
      const pkcs8 = await crypto.subtle.exportKey('pkcs8', kp.privateKey);
      const pkcs8B64 = btoa(String.fromCharCode(...new Uint8Array(pkcs8)));
      localStorage.setItem('humanity_ecdh_backup', JSON.stringify({ publicKeyRaw: myEcdhPublicBase64, privateKeyPkcs8: pkcs8B64 }));
    } catch (e) {}

    console.log('Generated new ECDH P-256 keypair');
  } catch (e) {
    console.error('ECDH key generation failed:', e);
  }
}

/** Derive an AES-GCM-256 key from our ECDH private key and peer's ECDH public key. */
async function deriveSharedKey(peerEcdhPublicBase64) {
  if (!myEcdhKeyPair || !peerEcdhPublicBase64) return null;
  try {
    const peerRaw = Uint8Array.from(atob(peerEcdhPublicBase64), c => c.charCodeAt(0));
    const peerKey = await crypto.subtle.importKey('raw', peerRaw, { name: 'ECDH', namedCurve: 'P-256' }, false, []);
    return await crypto.subtle.deriveKey(
      { name: 'ECDH', public: peerKey },
      myEcdhKeyPair.privateKey,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt', 'decrypt']
    );
  } catch (e) {
    console.error('ECDH key derivation failed:', e);
    return null;
  }
}

/** Encrypt a plaintext string for a peer. Returns { content, nonce } (both base64) or null. */
async function encryptDmContent(plaintext, peerEcdhPublicBase64) {
  const sharedKey = await deriveSharedKey(peerEcdhPublicBase64);
  if (!sharedKey) return null;
  try {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encoded = new TextEncoder().encode(plaintext);
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, sharedKey, encoded);
    return {
      content: btoa(String.fromCharCode(...new Uint8Array(ciphertext))),
      nonce: btoa(String.fromCharCode(...iv))
    };
  } catch (e) {
    console.error('DM encryption failed:', e);
    return null;
  }
}

/** Decrypt an encrypted DM. Returns plaintext string or null. */
async function decryptDmContent(ciphertextBase64, nonceBase64, peerEcdhPublicBase64) {
  const sharedKey = await deriveSharedKey(peerEcdhPublicBase64);
  if (!sharedKey) return null;
  try {
    const iv = Uint8Array.from(atob(nonceBase64), c => c.charCodeAt(0));
    const ciphertext = Uint8Array.from(atob(ciphertextBase64), c => c.charCodeAt(0));
    const plainBuf = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, sharedKey, ciphertext);
    return new TextDecoder().decode(plainBuf);
  } catch (e) {
    console.error('DM decryption failed:', e);
    return null;
  }
}

/** Look up ECDH public key for a peer by their Ed25519 public key. */
function getPeerEcdhPublic(peerKey) {
  const peer = peerData[peerKey];
  return peer ? peer.ecdh_public : null;
}

// ══════════════════════════════════════════════════════════════════════════════
// End E2EE
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// BIP39 Seed Phrase — 24-word paper backup for the Ed25519 identity key
// ══════════════════════════════════════════════════════════════════════════════
// Goal: let users write 24 words on paper and fully restore their identity
// on a new device with no cloud, no server, no QR code required.
//
// Encoding: 256-bit entropy (32-byte Ed25519 seed) + 8-bit SHA-256 checksum
//           = 264 bits → 24 × 11 bits → 24 BIP39 words.
//
// The wordlist is loaded from window.BIP39_ENGLISH (bip39-english.js must be
// loaded before crypto.js).  If unavailable the functions return null gracefully.

/** @returns {string[]|null} The 2048-word BIP39 English list, or null. */
function bip39Words() {
  return (typeof window !== 'undefined' && Array.isArray(window.BIP39_ENGLISH) &&
          window.BIP39_ENGLISH.length === 2048)
    ? window.BIP39_ENGLISH : null;
}

/**
 * Encode a 32-byte Uint8Array as a 24-word BIP39 mnemonic.
 * 256-bit entropy + 8-bit checksum → 264 bits → 24 × 11 bits.
 * @param {Uint8Array} seed32
 * @returns {Promise<string|null>} Space-separated 24 words, or null.
 */
async function mnemonicFromSeed(seed32) {
  const words = bip39Words();
  if (!words || seed32.length !== 32) return null;
  // Compute 8-bit checksum: first byte of SHA-256(seed)
  const hashBuf = await crypto.subtle.digest('SHA-256', seed32);
  const checksum = new Uint8Array(hashBuf)[0];
  // Build bit array: 256 entropy bits + 8 checksum bits = 264 bits
  const bits = [];
  for (const byte of seed32) for (let i = 7; i >= 0; i--) bits.push((byte >> i) & 1);
  for (let i = 7; i >= 0; i--) bits.push((checksum >> i) & 1);
  // Group into 24 × 11-bit chunks → word indices
  const result = [];
  for (let i = 0; i < 24; i++) {
    let idx = 0;
    for (let j = 0; j < 11; j++) idx = (idx << 1) | bits[i * 11 + j];
    result.push(words[idx]);
  }
  return result.join(' ');
}

/**
 * Decode a 24-word BIP39 mnemonic back to the 32-byte seed.
 * Validates word membership and SHA-256 checksum.
 * @param {string} mnemonic - Space-separated 24 words (case-insensitive).
 * @returns {Promise<Uint8Array>} 32-byte seed.
 * @throws {Error} If words are invalid or checksum fails.
 */
async function seedFromMnemonic(mnemonic) {
  const words = bip39Words();
  if (!words) throw new Error('BIP39 wordlist not loaded.');
  const list = mnemonic.toLowerCase().trim().split(/\s+/);
  if (list.length !== 24) throw new Error('Recovery phrase must be exactly 24 words.');
  // Decode words → bit array (264 bits)
  const bits = [];
  for (const word of list) {
    const idx = words.indexOf(word);
    if (idx < 0) throw new Error(`Unknown word: "${word}". Check spelling.`);
    for (let j = 10; j >= 0; j--) bits.push((idx >> j) & 1);
  }
  // Extract 256 entropy bits → 32 bytes
  const seed = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    for (let j = 0; j < 8; j++) seed[i] = (seed[i] << 1) | bits[i * 8 + j];
  }
  // Verify 8-bit checksum
  const hashBuf = await crypto.subtle.digest('SHA-256', seed);
  const expectedChecksum = new Uint8Array(hashBuf)[0];
  let actualChecksum = 0;
  for (let i = 0; i < 8; i++) actualChecksum = (actualChecksum << 1) | bits[256 + i];
  if (actualChecksum !== expectedChecksum) {
    throw new Error('Invalid recovery phrase — checksum failed. Check for typos.');
  }
  return seed;
}

/**
 * Return the current identity as a 24-word BIP39 mnemonic.
 * @returns {Promise<string|null>} Mnemonic string or null if unavailable.
 */
async function generateMnemonic() {
  if (!myIdentity || !myIdentity.privateKey) return null;
  try {
    const pkcs8 = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
    const seed = extractSeedFromPkcs8(pkcs8);
    return mnemonicFromSeed(seed);
  } catch (e) { console.error('generateMnemonic failed:', e); return null; }
}

/**
 * Restore identity from a 24-word BIP39 mnemonic.
 * Derives the Ed25519 keypair, stores it to IndexedDB + localStorage,
 * and returns an identity object ready to pass to reconnect().
 * @param {string} mnemonic - 24-word recovery phrase.
 * @returns {Promise<object>} Identity: { publicKeyHex, privateKey, publicKey, canSign }
 * @throws {Error} If the mnemonic is invalid or the browser lacks Ed25519 support.
 */
async function restoreIdentityFromMnemonic(mnemonic) {
  // Decode + checksum-validate the 24 words → 32-byte seed
  const seed = await seedFromMnemonic(mnemonic);

  // Wrap in Ed25519 PKCS8 structure (same prefix used by importIdentityFromJSON)
  const pkcs8Prefix = new Uint8Array([
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06,
    0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20
  ]);
  const pkcs8 = new Uint8Array(48);
  pkcs8.set(pkcs8Prefix, 0);
  pkcs8.set(seed, 16);

  // Import private key
  const privateKey = await crypto.subtle.importKey('pkcs8', pkcs8, 'Ed25519', true, ['sign']);

  // Derive public key via JWK export (Chrome 80+/Firefox 79+: JWK includes `x` = public key)
  let publicKeyHex;
  let publicKey;
  try {
    const jwk = await crypto.subtle.exportKey('jwk', privateKey);
    if (!jwk.x) throw new Error('JWK missing public key field');
    // base64url → bytes → hex
    const pubBytes = Uint8Array.from(atob(jwk.x.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0));
    publicKeyHex = bufToHex(pubBytes);
    publicKey = await crypto.subtle.importKey('raw', pubBytes, 'Ed25519', true, ['verify']);
  } catch (e) {
    throw new Error('Could not derive public key from seed. Your browser may not fully support Ed25519. Error: ' + e.message);
  }

  // Sanity-check: sign + verify a test message
  const test = new TextEncoder().encode('humanity-identity-verify');
  const sig = await crypto.subtle.sign('Ed25519', privateKey, test);
  const ok  = await crypto.subtle.verify('Ed25519', publicKey, sig, test);
  if (!ok) throw new Error('Key self-verification failed — seed may be corrupted.');

  // Persist to IndexedDB and localStorage backup
  const db = await openKeyDB();
  await storeKeypair(db, publicKeyHex, { privateKey, publicKey });
  await saveKeyBackupToLocalStorage(publicKeyHex, privateKey);

  console.log('Identity restored from mnemonic:', publicKeyHex.substring(0, 16) + '…');
  return { publicKeyHex, privateKey, publicKey, canSign: true, isNew: false, restored: true };
}

/**
 * Encrypt a BIP39 mnemonic with a user passphrase and return a small portable
 * JSON blob that can be saved anywhere (file, password manager note, cloud).
 * Uses PBKDF2-SHA256 (600k iterations) → AES-256-GCM, same as identity backup.
 * The output contains everything needed to decrypt — no server, no account.
 * @param {string} mnemonic  - 24-word space-separated BIP39 phrase.
 * @param {string} passphrase - User-chosen encryption passphrase.
 * @returns {Promise<object>} Blob: { v, enc, iv, salt } (all base64).
 */
async function encryptMnemonic(mnemonic, passphrase) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv   = crypto.getRandomValues(new Uint8Array(12));
  const key  = await deriveKeyFromPassphrase(passphrase, salt);
  const data = new TextEncoder().encode(mnemonic);
  const encBuf = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, data);
  const b64 = buf => btoa(String.fromCharCode(...new Uint8Array(buf)));
  return {
    v:    1,
    type: 'humanity-mnemonic',
    enc:  b64(encBuf),
    iv:   b64(iv),
    salt: b64(salt),
  };
}

/**
 * Decrypt a mnemonic blob produced by encryptMnemonic().
 * @param {object} blob       - The { v, enc, iv, salt } object.
 * @param {string} passphrase - The passphrase used when encrypting.
 * @returns {Promise<string>} The 24-word mnemonic.
 * @throws {Error} If the passphrase is wrong or the blob is malformed.
 */
async function decryptMnemonic(blob, passphrase) {
  if (!blob || blob.type !== 'humanity-mnemonic') throw new Error('Not a mnemonic backup file.');
  const b64dec = s => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const salt = b64dec(blob.salt);
  const iv   = b64dec(blob.iv);
  const enc  = b64dec(blob.enc);
  const key  = await deriveKeyFromPassphrase(passphrase, salt);
  let plain;
  try {
    plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, enc);
  } catch {
    throw new Error('Wrong passphrase or corrupted file.');
  }
  return new TextDecoder().decode(plain);
}

/**
 * Encrypt a mnemonic and trigger a browser download of a tiny JSON file the
 * user can store anywhere — cloud, USB, password manager attachment.
 * Accepts an already-generated mnemonic string, or generates one if omitted.
 * @param {string|null} mnemonic  - Pre-generated mnemonic, or null to generate.
 * @param {string}      passphrase - User-chosen passphrase.
 */
async function downloadEncryptedMnemonic(mnemonic, passphrase) {
  if (!mnemonic) mnemonic = await generateMnemonic();
  if (!mnemonic) throw new Error('Key is not extractable — use Encrypted Backup instead.');
  const blob = await encryptMnemonic(mnemonic, passphrase);
  const json = JSON.stringify(blob, null, 2);
  const a = document.createElement('a');
  a.href = 'data:application/json;charset=utf-8,' + encodeURIComponent(json);
  a.download = 'humanity-phrase-backup.json';
  a.click();
}
