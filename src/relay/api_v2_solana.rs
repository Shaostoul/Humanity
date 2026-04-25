//! HTTP API v2: Solana balance proxy (Phase 6a — read-only).
//!
//! `GET /api/v2/solana/balance/{base58_address}` — returns SOL balance in
//! lamports + sol formatted. The relay proxies this to the Solana mainnet-beta
//! JSON-RPC endpoint so clients don't all hammer it directly.
//!
//! Per the strategic plan (decision 4): Solana stays as an opt-in payment
//! substrate decoupled from identity. Transaction signing happens client-side;
//! the relay only proxies read-only queries.

use axum::{
    Json,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
};

use crate::relay::core::solana_rpc::{LAMPORTS_PER_SOL, SolanaRpcClient};

/// `GET /api/v2/solana/balance/{address}`
pub async fn get_solana_balance(Path(address): Path<String>) -> impl IntoResponse {
    // Choose endpoint via env var (defaults to mainnet-beta). Set
    // SOLANA_RPC_URL=https://api.devnet.solana.com for devnet testing.
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| crate::relay::core::solana_rpc::MAINNET_BETA_URL.to_string());
    let client = SolanaRpcClient::new(rpc_url);

    match client.get_balance(&address).await {
        Ok(lamports) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "address": address,
                "lamports": lamports,
                "sol": lamports as f64 / LAMPORTS_PER_SOL as f64,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}
