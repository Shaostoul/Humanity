/**
 * Humanity Network — Client-side Ed25519 cryptography.
 *
 * Uses the Web Crypto API for Ed25519 key generation and signing.
 * Keys are persisted in IndexedDB for stable identity across sessions.
 *
 * Per the Humanity design spec:
 * - Identity = Ed25519 keypair (client-side only)
 * - Server never sees private keys
 * - All messages will be signed (MVP: sign content + timestamp)
 */

const DB_NAME = 'humanity-keys';
const DB_VERSION = 1;
const STORE_NAME = 'identity';
const KEY_ID = 'primary';

/**
 * Open the IndexedDB for key storage.
 */
function openKeyDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      req.result.createObjectStore(STORE_NAME, { keyPath: 'id' });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * Store a keypair in IndexedDB.
 */
async function storeKeypair(db, publicKeyHex, keypair) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    tx.objectStore(STORE_NAME).put({
      id: KEY_ID,
      publicKeyHex,
      privateKey: keypair.privateKey,
      publicKey: keypair.publicKey,
    });
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/**
 * Load a keypair from IndexedDB.
 */
async function loadKeypair(db) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const req = tx.objectStore(STORE_NAME).get(KEY_ID);
    req.onsuccess = () => resolve(req.result || null);
    req.onerror = () => reject(req.error);
  });
}

/**
 * Convert an ArrayBuffer to hex string.
 */
function bufToHex(buf) {
  return Array.from(new Uint8Array(buf))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Convert a hex string to Uint8Array.
 */
function hexToBuf(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

/**
 * Check if the browser supports Ed25519 via WebCrypto.
 */
async function supportsEd25519() {
  try {
    const key = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
    return true;
  } catch (e) {
    return false;
  }
}

/**
 * Generate a new Ed25519 keypair.
 * Returns { publicKeyHex, privateKey (CryptoKey), publicKey (CryptoKey) }
 */
async function generateKeypair() {
  const keypair = await crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']);

  // Export public key to get hex representation.
  const rawPub = await crypto.subtle.exportKey('raw', keypair.publicKey);
  const publicKeyHex = bufToHex(rawPub);

  return {
    publicKeyHex,
    privateKey: keypair.privateKey,
    publicKey: keypair.publicKey,
  };
}

/**
 * Get or create the user's identity keypair.
 * Persists in IndexedDB so the key survives across sessions.
 *
 * Falls back to random hex key if Ed25519 is not supported.
 */
async function getOrCreateIdentity() {
  // Check Ed25519 support.
  const hasEd25519 = await supportsEd25519();

  if (!hasEd25519) {
    console.warn('Ed25519 not supported — falling back to random key (no signing)');
    let key = localStorage.getItem('humanity_key');
    if (!key) {
      const bytes = new Uint8Array(32);
      crypto.getRandomValues(bytes);
      key = bufToHex(bytes);
      localStorage.setItem('humanity_key', key);
    }
    return { publicKeyHex: key, privateKey: null, publicKey: null, canSign: false };
  }

  try {
    const db = await openKeyDB();
    const stored = await loadKeypair(db);

    if (stored && stored.privateKey && stored.publicKeyHex) {
      console.log('Loaded existing identity:', stored.publicKeyHex.substring(0, 16) + '…');
      return {
        publicKeyHex: stored.publicKeyHex,
        privateKey: stored.privateKey,
        publicKey: stored.publicKey,
        canSign: true,
      };
    }

    // Generate new keypair.
    const kp = await generateKeypair();
    await storeKeypair(db, kp.publicKeyHex, kp);
    console.log('Generated new identity:', kp.publicKeyHex.substring(0, 16) + '…');

    return {
      publicKeyHex: kp.publicKeyHex,
      privateKey: kp.privateKey,
      publicKey: kp.publicKey,
      canSign: true,
    };
  } catch (e) {
    console.error('Identity setup failed:', e);
    // Fallback to random key.
    let key = localStorage.getItem('humanity_key');
    if (!key) {
      const bytes = new Uint8Array(32);
      crypto.getRandomValues(bytes);
      key = bufToHex(bytes);
      localStorage.setItem('humanity_key', key);
    }
    return { publicKeyHex: key, privateKey: null, publicKey: null, canSign: false };
  }
}

/**
 * Sign a message payload.
 *
 * Signs the canonical string: `${content}\n${timestamp}`
 * Returns the signature as a hex string, or null if signing is unavailable.
 */
async function signMessage(privateKey, content, timestamp) {
  if (!privateKey) return null;

  try {
    const payload = `${content}\n${timestamp}`;
    const encoded = new TextEncoder().encode(payload);
    const sig = await crypto.subtle.sign('Ed25519', privateKey, encoded);
    return bufToHex(sig);
  } catch (e) {
    console.error('Signing failed:', e);
    return null;
  }
}

/**
 * Verify a message signature.
 *
 * @param {string} publicKeyHex - The signer's public key (hex).
 * @param {string} signatureHex - The Ed25519 signature (hex).
 * @param {string} content - Message content.
 * @param {number} timestamp - Message timestamp.
 * @returns {boolean} True if signature is valid.
 */
async function verifyMessage(publicKeyHex, signatureHex, content, timestamp) {
  try {
    const pubKeyBytes = hexToBuf(publicKeyHex);
    const pubKey = await crypto.subtle.importKey(
      'raw', pubKeyBytes, 'Ed25519', true, ['verify']
    );

    const payload = `${content}\n${timestamp}`;
    const encoded = new TextEncoder().encode(payload);
    const sig = hexToBuf(signatureHex);

    return await crypto.subtle.verify('Ed25519', pubKey, sig, encoded);
  } catch (e) {
    console.error('Verification failed:', e);
    return false;
  }
}
