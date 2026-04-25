/* HumanityOS — PQ Identity Bridge (Phase 0–8 client integration)
 *
 * This module is the JS-side bridge to the v0.98+ post-quantum substrate.
 * It does NOT replace the existing Ed25519 chat in web/chat/ — it augments
 * the experience by giving any page on the site read access to the new
 * /api/v2/* endpoints: DID resolution, Verifiable Credentials, trust score,
 * governance proposals, AI status, recovery setup, Solana balance, etc.
 *
 * Usage (no build step — plain script tag):
 *   <script src="/shared/pq-identity.js"></script>
 *   const trust = await HumOS.pq.getTrustScore('did:hum:abc...');
 *   const vcs = await HumOS.pq.listCredentials({ subject: 'did:hum:...' });
 *
 * A full Dilithium3 / Kyber768 keygen + signing path requires the
 * `@noble/post-quantum` library; that lands in a follow-up when we vendor a
 * UMD bundle (~200 KB) or pick a CDN. Until then, this module:
 *   - Reads everything from the v2 API (Cargo.toml: 14 endpoints exposed)
 *   - Provides the type-safe API surface for client devs
 *   - Documents the recommended migration path
 */
(function () {
  'use strict';

  // Build a base URL from the current origin. Same-origin keeps cookies and
  // CORS happy; the relay listens on the same host as the website.
  const API_BASE = window.location.origin;

  /* ─── Low-level fetch helpers ──────────────────────────────────────── */

  async function getJson(path) {
    const res = await fetch(`${API_BASE}${path}`, {
      method: 'GET',
      credentials: 'same-origin',
    });
    if (!res.ok && res.status !== 404) {
      throw new Error(`GET ${path}: ${res.status} ${res.statusText}`);
    }
    return res.json();
  }

  async function postJson(path, body) {
    const res = await fetch(`${API_BASE}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'same-origin',
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      throw new Error(`POST ${path}: ${res.status} ${res.statusText}`);
    }
    return res.json();
  }

  /* ─── DID layer (Phase 1 PR 1) ─────────────────────────────────────── */

  async function resolveDid(did) {
    return getJson(`/api/v2/did/${encodeURIComponent(did)}`);
  }

  /* ─── Verifiable Credentials (Phase 1 PR 2) ────────────────────────── */

  async function listCredentials({ subject, issuer, schema, includeRevoked, includeWithdrawn, limit } = {}) {
    const q = new URLSearchParams();
    if (subject) q.set('subject', subject);
    if (issuer) q.set('issuer', issuer);
    if (schema) q.set('schema', schema);
    if (includeRevoked) q.set('include_revoked', 'true');
    if (includeWithdrawn) q.set('include_withdrawn', 'true');
    if (limit) q.set('limit', String(limit));
    return getJson(`/api/v2/credentials?${q.toString()}`);
  }

  async function getCredential(vcObjectId) {
    return getJson(`/api/v2/credentials/${encodeURIComponent(vcObjectId)}`);
  }

  /* ─── Trust score (Phase 2) ────────────────────────────────────────── */

  async function getTrustScore(did, { fresh = false } = {}) {
    const q = fresh ? '?fresh=true' : '';
    return getJson(`/api/v2/trust/${encodeURIComponent(did)}${q}`);
  }

  /* ─── Governance (Phase 5) ─────────────────────────────────────────── */

  async function listProposals({ scope, type, spaceId, onlyOpen, limit } = {}) {
    const q = new URLSearchParams();
    if (scope) q.set('scope', scope);
    if (type) q.set('proposal_type', type);
    if (spaceId) q.set('space_id', spaceId);
    if (onlyOpen) q.set('only_open', 'true');
    if (limit) q.set('limit', String(limit));
    return getJson(`/api/v2/proposals?${q.toString()}`);
  }

  async function getProposal(id) {
    return getJson(`/api/v2/proposals/${encodeURIComponent(id)}`);
  }

  async function getProposalTally(id) {
    return getJson(`/api/v2/proposals/${encodeURIComponent(id)}/tally`);
  }

  /* ─── AI status (Phase 8) ──────────────────────────────────────────── */

  async function getAiStatus(did) {
    return getJson(`/api/v2/ai-status/${encodeURIComponent(did)}`);
  }

  /* ─── Social key recovery (Phase 4) ────────────────────────────────── */

  async function getRecoverySetup(holderDid) {
    return getJson(`/api/v2/recovery/setup/${encodeURIComponent(holderDid)}`);
  }

  async function getSharesHeldBy(guardianDid) {
    return getJson(`/api/v2/recovery/shares-held-by/${encodeURIComponent(guardianDid)}`);
  }

  async function getRecoveryRequest(requestObjectId) {
    return getJson(`/api/v2/recovery/request/${encodeURIComponent(requestObjectId)}`);
  }

  /* ─── Federation v2 ────────────────────────────────────────────────── */

  async function listObjects({ type, space, authorFp, sinceReceived, limit } = {}) {
    const q = new URLSearchParams();
    if (type) q.set('object_type', type);
    if (space) q.set('space_id', space);
    if (authorFp) q.set('author_fp', authorFp);
    if (sinceReceived) q.set('since_received', String(sinceReceived));
    if (limit) q.set('limit', String(limit));
    return getJson(`/api/v2/objects?${q.toString()}`);
  }

  async function getObject(objectId) {
    return getJson(`/api/v2/objects/${encodeURIComponent(objectId)}`);
  }

  /**
   * Submit a pre-signed PQ object. The caller MUST sign the canonical CBOR
   * with their Dilithium3 key; this function only handles the HTTP layer.
   *
   * `submission` shape (all binary fields base64-encoded):
   *   {
   *     protocol_version: 1,
   *     object_type: "vouch_v1",
   *     space_id: optional,
   *     channel_id: optional,
   *     author_public_key_b64: "...",     // 1952-byte Dilithium3 pubkey
   *     created_at: optional u64,
   *     references: [],
   *     payload_schema_version: 1,
   *     payload_encoding: "cbor_canonical_v1",
   *     payload_b64: "...",
   *     signature_b64: "..."              // 3309-byte Dilithium3 sig
   *   }
   */
  async function postObject(submission) {
    return postJson('/api/v2/objects', submission);
  }

  /* ─── Solana opt-in (Phase 6a) ─────────────────────────────────────── */

  async function getSolanaBalance(base58Address) {
    return getJson(`/api/v2/solana/balance/${encodeURIComponent(base58Address)}`);
  }

  /* ─── Schema registry endpoints ────────────────────────────────────── */

  async function getZkSchema() {
    return getJson('/api/v2/zk/schema');
  }

  async function getLivenessSchema() {
    return getJson('/api/v2/liveness/schema');
  }

  /* ─── Convenience: enrich a profile view ───────────────────────────── */

  /**
   * One-shot fetch: resolve DID + trust score + AI status + recent VCs.
   * Useful for profile pages so they don't have to chain N calls in a loop.
   */
  async function enrichDid(did) {
    const [resolution, trust, aiStatus, credentials] = await Promise.all([
      resolveDid(did).catch((e) => ({ error: e.message })),
      getTrustScore(did).catch((e) => ({ error: e.message })),
      getAiStatus(did).catch((e) => ({ error: e.message })),
      listCredentials({ subject: did, limit: 50 }).catch((e) => ({ error: e.message })),
    ]);
    return { did, resolution, trust, aiStatus, credentials };
  }

  /* ─── PQ keygen + signing — TODO marker ────────────────────────────── */

  /**
   * STUB: full PQ key derivation and signing requires the @noble/post-quantum
   * library (Dilithium3 + Kyber768). When that lands as a vendored UMD bundle
   * or CDN load, the implementation will be:
   *
   *   import { ml_dsa65 } from '@noble/post-quantum/ml-dsa';
   *   import { ml_kem768 } from '@noble/post-quantum/ml-kem';
   *   const seed = bip39ToSeed(mnemonic);  // existing crypto.js helper
   *   const dilithiumSeed = blake3DeriveKey(seed, "hum/dilithium3/v1");
   *   const { secretKey, publicKey } = ml_dsa65.keygen(dilithiumSeed);
   *   const sig = ml_dsa65.sign(secretKey, canonicalCborBytes);
   *
   * For now, this throws so callers know the gap.
   */
  function pqKeygenFromSeed(_seedBytes32) {
    throw new Error(
      'PQ keygen not yet wired in JS. ' +
      'Track plan: ~/.claude/plans/okay-claude-here-s-a-floating-wozniak.md ' +
      '(Phase 0 PR 3b — pending @noble/post-quantum bundle integration).'
    );
  }

  function pqSign(_signingKey, _message) {
    throw new Error('PQ sign not yet wired in JS. See pqKeygenFromSeed for status.');
  }

  function pqVerify(_publicKey, _message, _signature) {
    throw new Error('PQ verify not yet wired in JS. See pqKeygenFromSeed for status.');
  }

  /* ─── Public API ───────────────────────────────────────────────────── */

  window.HumOS = window.HumOS || {};
  window.HumOS.pq = {
    // DID
    resolveDid,
    // VCs
    listCredentials,
    getCredential,
    // Trust score
    getTrustScore,
    // Governance
    listProposals,
    getProposal,
    getProposalTally,
    // AI
    getAiStatus,
    // Recovery
    getRecoverySetup,
    getSharesHeldBy,
    getRecoveryRequest,
    // Generic objects (federation substrate)
    listObjects,
    getObject,
    postObject,
    // Solana
    getSolanaBalance,
    // Schema docs
    getZkSchema,
    getLivenessSchema,
    // Convenience
    enrichDid,
    // PQ crypto stubs (TODO)
    pqKeygenFromSeed,
    pqSign,
    pqVerify,
  };
})();
