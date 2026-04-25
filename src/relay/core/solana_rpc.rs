//! Solana JSON-RPC client (Phase 6a — read-only balance lookup).
//!
//! Lightweight JSON-RPC over reqwest — avoids pulling in the full `solana-client`
//! crate with its C dependencies. Read-only for now: balance queries only.
//! Transaction signing stays client-side per the plan (the relay must never see
//! private keys).
//!
//! Usage:
//! ```ignore
//! let client = SolanaRpcClient::mainnet();
//! let lamports = client.get_balance("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").await?;
//! ```
//!
//! The server's identity remains Dilithium3 — Solana support is purely an
//! optional payment substrate. Users derive a Solana keypair from the same
//! BIP39 seed via a separate KDF path (`hum/solana/v1`) — the chat client
//! handles signing entirely. The relay only proxies balance lookups so the UI
//! can show "your wallet holds X SOL" without each client hammering an RPC.

use serde::{Deserialize, Serialize};

use crate::relay::core::error::{Error, Result};

/// Default Solana mainnet-beta JSON-RPC endpoint.
pub const MAINNET_BETA_URL: &str = "https://api.mainnet-beta.solana.com";

/// Default Solana devnet JSON-RPC endpoint (used in tests/dev).
pub const DEVNET_URL: &str = "https://api.devnet.solana.com";

/// Lamports per SOL (1 SOL = 1_000_000_000 lamports).
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Lightweight Solana JSON-RPC client.
#[derive(Debug, Clone)]
pub struct SolanaRpcClient {
    rpc_url: String,
    timeout_ms: u64,
}

impl SolanaRpcClient {
    pub fn mainnet() -> Self {
        Self::new(MAINNET_BETA_URL.to_string())
    }

    pub fn devnet() -> Self {
        Self::new(DEVNET_URL.to_string())
    }

    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url, timeout_ms: 5000 }
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Get the SOL balance (in lamports) for an address.
    /// `address` is the base58-encoded Solana public key (32 bytes Ed25519).
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        // Validate the address shape before sending.
        // A Solana base58 pubkey is 32-44 chars. Reject obviously bad input early
        // so we don't burn an RPC round-trip.
        if address.is_empty() || address.len() > 64 || !is_base58(address) {
            return Err(Error::InvalidField {
                field: "solana_address".into(),
                reason: "must be base58-encoded".into(),
            });
        }

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBalance".to_string(),
            params: vec![serde_json::Value::String(address.to_string())],
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build()
            .map_err(|e| Error::InvalidField {
                field: "reqwest_client".into(),
                reason: e.to_string(),
            })?;

        let resp: JsonRpcResponse<BalanceResult> = client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::InvalidField {
                field: "solana_rpc".into(),
                reason: format!("network: {e}"),
            })?
            .json()
            .await
            .map_err(|e| Error::InvalidField {
                field: "solana_rpc_decode".into(),
                reason: e.to_string(),
            })?;

        if let Some(err) = resp.error {
            return Err(Error::InvalidField {
                field: "solana_rpc".into(),
                reason: format!("rpc error {}: {}", err.code, err.message),
            });
        }
        Ok(resp.result.map(|r| r.value).unwrap_or(0))
    }
}

/// Cheap base58 validator: alphabet check only (not a full decode).
fn is_base58(s: &str) -> bool {
    const ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    s.chars().all(|c| ALPHABET.contains(c))
}

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    // Option fields without #[serde(default)] — serde maps missing-key to None
    // natively without requiring T: Default. The default attribute would force
    // T: Default which we don't need.
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
struct BalanceResult {
    value: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_addresses_rejected_offline() {
        let client = SolanaRpcClient::devnet();

        // We can't actually hit the network in unit tests. But we can verify
        // that address validation rejects junk before any RPC call.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // Empty
        let r = rt.block_on(client.get_balance(""));
        assert!(r.is_err());

        // Too long
        let r = rt.block_on(client.get_balance(&"a".repeat(100)));
        assert!(r.is_err());

        // Contains '0' (not in base58 alphabet)
        let r = rt.block_on(client.get_balance("0xinvalid"));
        assert!(r.is_err());
    }

    #[test]
    fn lamports_per_sol_constant() {
        assert_eq!(LAMPORTS_PER_SOL, 1_000_000_000);
    }
}
