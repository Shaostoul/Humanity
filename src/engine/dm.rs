use crate::gui::GuiState;

/// Helper: decrypt an encrypted DM content if we have the keys.
/// Returns the decrypted plaintext, or the original content with a marker if decryption fails.
pub(crate) fn decrypt_dm_if_encrypted(
    raw_content: &str,
    encrypted: bool,
    nonce: &str,
    peer_key: &str,
    gui_state: &GuiState,
) -> String {
    let _ = (nonce, peer_key); // full-PQ: envelope is self-contained; KEM needs no peer key
    if !encrypted {
        return raw_content.to_string();
    }
    // Full-PQ: decapsulate with OUR OWN Kyber768 secret (deterministic
    // from the BIP39 seed). The {v:1,r,s} dual-seal envelope means this
    // opens both received messages and our own from history, on any
    // device with the seed. No peer key needed (ML-KEM).
    let seed = match gui_state.private_key_bytes.as_ref() {
        Some(s) => s,
        None => return "[encrypted — unlock your identity to read]".to_string(),
    };
    let me = match crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed) {
        Ok(k) => k,
        Err(_) => return "[encrypted — key derivation failed]".to_string(),
    };
    match crate::net::dm_pq::open_envelope(&me, raw_content) {
        Ok(plain) => plain,
        Err(e) => {
            log::warn!("PQ DM decryption failed for {}: {}", peer_key, e);
            "[encrypted — decryption failed]".to_string()
        }
    }
}
