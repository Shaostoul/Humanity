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
    div.style.cssText = 'padding:0.5rem;margin-bottom:0.4rem;background:var(--bg-secondary);border-radius:6px;border:1px solid var(--border);';
    const keyShort = d.public_key.substring(0, 16) + '…';
    const statusDot = d.is_online ? '🟢' : '⚫';
    const currentBadge = d.is_current ? ' <span style="color:var(--accent);font-size:0.7rem">(this device)</span>' : '';
    const date = new Date(d.registered_at).toLocaleDateString();
    div.innerHTML = `
      <div style="display:flex;align-items:center;gap:0.4rem;margin-bottom:0.3rem">
        <span>${statusDot}</span>
        <code style="font-size:0.75rem;color:var(--text-muted)">${keyShort}</code>
        ${currentBadge}
      </div>
      <div style="display:flex;align-items:center;gap:0.3rem;margin-bottom:0.3rem">
        <input type="text" value="${(d.label || '').replace(/"/g, '&quot;')}"
               placeholder="Label (e.g. PC, Phone, Laptop)"
               maxlength="32"
               style="flex:1;font-size:0.8rem;padding:0.2rem 0.4rem;background:var(--bg);border:1px solid var(--border);border-radius:4px;color:var(--text)"
               onchange="labelDevice('${d.public_key}', this.value)">
        ${!d.is_current ? `<button onclick="revokeDevice('${d.public_key.substring(0, 8)}')"
                style="font-size:0.75rem;padding:0.2rem 0.5rem;background:#c0392b;border:none;border-radius:4px;color:#fff;cursor:pointer"
                title="Remove this device">✕</button>` : ''}
      </div>
      <div style="font-size:0.7rem;color:var(--text-muted)">Registered: ${date}</div>
    `;
    container.appendChild(div);
  });
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
      const eCt      = await crypto.subtle.encrypt({ name: 'AES-GCM', eIv }, eWrapKey, ePkcs8);
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
