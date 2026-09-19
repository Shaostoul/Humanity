//! THE MARKETPLACE's HTTP surface: what is for sale, what it looks like, what
//! people said about the seller, and the standing buy and sell orders.
//!
//! Extracted VERBATIM from `relay/api.rs` (2026-09-19) under the file-size
//! ratchet, which api.rs had outgrown at 4,480 lines against a 4,200 budget.
//!
//! WHY THIS IS THE CLUSTER. api.rs is one route table's worth of handlers for
//! about twenty unrelated features, and it grew the way such files do: a new
//! endpoint arrives under a new banner comment at the bottom. Trading is three
//! of those banners (Marketplace, Reviews, Order Book) that are plainly ONE
//! feature - a listing has images, a listing has reviews, a seller's rating
//! summarises those reviews, and the order book is the same goods offered
//! standing rather than one at a time. Nothing outside trading calls in, and
//! the wire types (`ListingEntry`, `ReviewEntry`, `OrderEntry`, ...) are used
//! nowhere else.
//!
//! THE ONE THING TO KNOW BEFORE EDITING THE REVIEWS: they are IDENTITY-SIGNED,
//! not session-trusted. A review carries a Dilithium3 signature over
//! `review\n{listing_id}` together with its timestamp, the handler verifies it
//! before storing, and a request older than five minutes is refused outright -
//! so the relay never takes a client's word for who wrote a review, and a
//! captured request cannot be replayed later. Deleting one runs the same check
//! against the original reviewer's key. The verification itself runs on
//! `spawn_blocking` because ML-DSA-65 verify is CPU-bound and would otherwise
//! stall the async executor, and it FAILS CLOSED if that task panics.
//!
//! Declared as a `#[path]` CHILD of `api`, so one `use super::*` brings in the
//! axum extractors, `RelayState` and the auth helpers, and the parent's
//! `pub use` keeps every `api::get_listings` / `api::create_trade_order` in
//! the router (`relay/mod.rs`) resolving unchanged. Not one route line moved.

use super::*;

// ── Marketplace API ──

/// Query params for GET /api/listings.
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub category: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Full-text search query (uses FTS5 when available, LIKE fallback).
    pub q: Option<String>,
}

/// Response for GET /api/listings.
#[derive(Debug, Serialize)]
pub struct ListingsResponse {
    pub listings: Vec<ListingEntry>,
}

#[derive(Debug, Serialize)]
pub struct ListingEntry {
    pub id: String,
    pub seller_key: String,
    pub seller_name: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub condition: Option<String>,
    pub price: Option<String>,
    pub payment_methods: Option<String>,
    pub location: Option<String>,
    pub images: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Image entry in API responses.
#[derive(Debug, Serialize)]
pub struct ListingImageEntry {
    pub id: i64,
    pub url: String,
    pub position: i32,
}

/// GET /api/listings — browse marketplace listings (public).
/// When `q` is provided, uses FTS5 full-text search (falls back to LIKE).
pub async fn get_listings(
    State(state): State<Arc<RelayState>>,
    Query(params): Query<ListingsQuery>,
) -> Json<ListingsResponse> {
    let limit = params.limit.unwrap_or(50).min(200);
    let listings = if let Some(ref q) = params.q {
        if q.trim().is_empty() {
            state.db.get_listings(
                params.category.as_deref(),
                params.status.as_deref().or(Some("active")),
                limit,
            ).unwrap_or_default()
        } else {
            state.db.search_listings(q.trim(), limit).unwrap_or_default()
        }
    } else {
        state.db.get_listings(
            params.category.as_deref(),
            params.status.as_deref().or(Some("active")),
            limit,
        ).unwrap_or_default()
    };
    let offset = params.offset.unwrap_or(0);
    let entries: Vec<ListingEntry> = listings.into_iter()
        .skip(offset)
        .map(|l| ListingEntry {
            id: l.id, seller_key: l.seller_key, seller_name: l.seller_name,
            title: l.title, description: l.description, category: l.category,
            condition: l.condition, price: l.price, payment_methods: l.payment_methods,
            location: l.location, images: l.images, status: l.status, created_at: l.created_at,
            updated_at: l.updated_at,
        }).collect();
    Json(ListingsResponse { listings: entries })
}

/// Request body for POST /api/listings.
#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub condition: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub payment_methods: String,
    #[serde(default)]
    pub location: String,
}

/// POST /api/listings — create a listing (requires API auth for bots).
pub async fn create_listing(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(req): Json<CreateListingRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;
    if req.title.trim().is_empty() || req.title.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Title must be 1-100 characters.".into()));
    }
    let id = format!("api_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
    match state.db.create_listing(&id, "bot_api", "API", req.title.trim(), &req.description, &req.category, &req.condition, &req.price, &req.payment_methods, &req.location) {
        Ok(()) => {
            if let Ok(Some(listing)) = state.db.get_listing_by_id(&id) {
                let _ = state.broadcast_tx.send(crate::relay::relay::RelayMessage::ListingNew {
                    listing: crate::relay::relay::ListingData {
                        id: listing.id.clone(), seller_key: listing.seller_key, seller_name: listing.seller_name,
                        title: listing.title, description: listing.description, category: listing.category,
                        condition: listing.condition, price: listing.price, payment_methods: listing.payment_methods,
                        location: listing.location, images: listing.images, status: listing.status,
                        created_at: listing.created_at, updated_at: listing.updated_at,
                    },
                });
            }
            Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed: {e}"))),
    }
}

/// POST /api/listings/{id}/images — upload an image to a listing.
/// Reuses the existing upload infrastructure: caller uploads via /api/upload first,
/// then registers the resulting URL here.
pub async fn add_listing_image(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Query(query): Query<UploadQuery>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Auth: resolve upload token or key to public key.
    let public_key = resolve_upload_key(&state, &query).await?;

    // Verify the caller owns this listing (or is admin).
    let seller_key = state.db.get_listing_seller_key(&listing_id)
        .ok_or((StatusCode::NOT_FOUND, "Listing not found.".into()))?;
    let role = state.db.get_role(&public_key).unwrap_or_default();
    let is_admin = role == "admin" || role == "mod";
    if seller_key != public_key && !is_admin {
        return Err((StatusCode::FORBIDDEN, "You can only add images to your own listings.".into()));
    }

    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing 'url' field.".into()));
    }
    let position = body.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    match state.db.add_listing_image(&listing_id, url, position) {
        Ok(image_id) => Ok(Json(serde_json::json!({
            "id": image_id,
            "listing_id": listing_id,
            "url": url,
            "position": position,
            "status": "created"
        }))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

/// GET /api/listings/{id}/images — get images for a listing.
pub async fn get_listing_images(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let images = state.db.get_listing_images(&listing_id);
    let entries: Vec<ListingImageEntry> = images.into_iter().map(|img| ListingImageEntry {
        id: img.id,
        url: img.url,
        position: img.position,
    }).collect();
    Json(serde_json::json!({ "images": entries }))
}

/// DELETE /api/listings/{listing_id}/images/{image_id} — remove an image from a listing.
pub async fn delete_listing_image(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path((listing_id, image_id)): axum::extract::Path<(String, i64)>,
    Query(query): Query<UploadQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let public_key = resolve_upload_key(&state, &query).await?;

    let seller_key = state.db.get_listing_seller_key(&listing_id)
        .ok_or((StatusCode::NOT_FOUND, "Listing not found.".into()))?;
    let role = state.db.get_role(&public_key).unwrap_or_default();
    let is_admin = role == "admin" || role == "mod";
    if seller_key != public_key && !is_admin {
        return Err((StatusCode::FORBIDDEN, "You can only delete images from your own listings.".into()));
    }

    match state.db.delete_listing_image(image_id, &listing_id) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Image not found.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// Resolve upload token/key to a public key (shared helper for listing image endpoints).
async fn resolve_upload_key(state: &Arc<RelayState>, query: &UploadQuery) -> Result<String, (StatusCode, String)> {
    if let Some(ref token) = query.token {
        if token.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Empty upload token.".into()));
        }
        let tokens = state.upload_tokens.read().await;
        match tokens.get(token) {
            Some(key) => Ok(key.clone()),
            None => Err((StatusCode::FORBIDDEN, "Invalid upload token.".into())),
        }
    } else if let Some(ref k) = query.key {
        if k.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Missing upload token or key.".into()));
        }
        let peers = state.peers.read().await;
        if !peers.contains_key(k) {
            return Err((StatusCode::FORBIDDEN, "Key is not connected.".into()));
        }
        Ok(k.clone())
    } else {
        Err((StatusCode::BAD_REQUEST, "Missing required 'token' or 'key' query parameter.".into()))
    }
}

// ── Reviews API ────────────────────────────────────────────────────────────

/// Query params for GET /api/listings/{id}/reviews.
#[derive(Debug, Deserialize)]
pub struct ReviewsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

/// GET /api/listings/{id}/reviews — get reviews for a listing (public).
pub async fn get_listing_reviews(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Query(params): Query<ReviewsQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let reviews = state.db.get_reviews(&listing_id, limit + offset).unwrap_or_default();
    let data: Vec<serde_json::Value> = reviews.iter().skip(offset).map(|r| {
        serde_json::json!({
            "id": r.id,
            "listing_id": r.listing_id,
            "reviewer_key": r.reviewer_key,
            "reviewer_name": r.reviewer_name,
            "rating": r.rating,
            "comment": r.comment,
            "created_at": r.created_at,
        })
    }).collect();

    // Also return aggregate info.
    let listing = state.db.get_listing_by_id(&listing_id);
    let (avg, count) = if let Ok(Some(ref l)) = listing {
        state.db.get_seller_rating(&l.seller_key)
    } else {
        (0.0, 0)
    };

    Json(serde_json::json!({
        "reviews": data,
        "avg_rating": avg,
        "review_count": count,
    }))
}

/// Request body for POST /api/listings/{id}/reviews.
#[derive(Debug, Deserialize)]
pub struct CreateReviewRequest {
    pub rating: i32,
    #[serde(default)]
    pub comment: String,
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
}

/// POST /api/listings/{id}/reviews — create a review (authenticated via Ed25519 sig).
pub async fn create_listing_review(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(listing_id): axum::extract::Path<String>,
    Json(body): Json<CreateReviewRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    // Reject requests older than 5 minutes.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    // Verify signature over "review\n" + listing_id + "\n" + timestamp.
    // ML-DSA-65 verify is CPU-bound — run it off the async executor (clone owned
    // inputs in; fail-closed on panic → reject). Decision + 401 status unchanged.
    let sig_content = format!("review\n{}", listing_id);
    let vk = body.public_key.clone();
    let vsig = body.signature.clone();
    let vts = body.timestamp;
    let sig_ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&vk, &sig_content, vts, &vsig)
    }).await.unwrap_or(false);
    if !sig_ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    let reviewer_name = state.db.name_for_key(&body.public_key)
        .ok().flatten().unwrap_or_else(|| "Anonymous".to_string());

    match state.db.create_review(&listing_id, &body.public_key, &reviewer_name, body.rating, &body.comment) {
        Ok(review_id) => {
            // Broadcast via WebSocket.
            if let Ok(Some(review)) = state.db.get_review_by_id(review_id) {
                let _ = state.broadcast_tx.send(crate::relay::relay::RelayMessage::ReviewCreated {
                    review: crate::relay::relay::review_from_db(&review),
                });
            }
            Ok(Json(serde_json::json!({ "id": review_id, "status": "created" })))
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

/// DELETE /api/listings/{id}/reviews/{review_id} — delete a review.
pub async fn delete_listing_review(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path((listing_id, review_id)): axum::extract::Path<(String, i64)>,
    Query(q): Query<VaultSyncQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    // ML-DSA-65 verify off the async executor (fail-closed → reject). 401 unchanged.
    let sig_content = format!("review_delete\n{}", review_id);
    let vk = q.key.clone();
    let vsig = q.sig.clone();
    let vts = q.timestamp;
    let sig_ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&vk, &sig_content, vts, &vsig)
    }).await.unwrap_or(false);
    if !sig_ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    let user_role = state.db.get_role(&q.key).unwrap_or_default();
    let is_admin = user_role == "admin" || user_role == "mod";

    match state.db.delete_review(review_id, &q.key, is_admin) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(crate::relay::relay::RelayMessage::ReviewDeleted {
                listing_id,
                review_id,
            });
            Ok(Json(serde_json::json!({ "status": "deleted" })))
        }
        Ok(false) => Err((StatusCode::NOT_FOUND, "Review not found.".into())),
        Err(e) => Err((StatusCode::FORBIDDEN, e)),
    }
}

/// GET /api/sellers/{key}/rating — get aggregate seller rating.
pub async fn get_seller_rating(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(seller_key): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let (avg, count) = state.db.get_seller_rating(&seller_key);
    Json(serde_json::json!({
        "seller_key": seller_key,
        "avg_rating": avg,
        "review_count": count,
    }))
}

// ── Order Book API ──

#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    pub item_type: Option<String>,
}

/// GET /api/trade/orders?item_type=wood — get open sell orders for an item type.
pub async fn get_trade_orders(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<OrderBookQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let item_type = query.item_type.unwrap_or_default();
    if item_type.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "item_type is required.".into()));
    }
    match state.db.get_open_orders(&item_type) {
        Ok(orders) => {
            let market_price = state.db.get_market_price(&item_type).unwrap_or(None);
            Ok(Json(serde_json::json!({
                "item_type": item_type,
                "orders": orders,
                "market_price": market_price,
            })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTradeOrderRequest {
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
    pub item_type: String,
    pub item_id: Option<String>,
    pub quantity: i64,
    pub price_per_unit: f64,
    pub currency: Option<String>,
}

/// POST /api/trade/orders — create a sell order (authenticated via Ed25519 sig).
pub async fn create_trade_order(
    State(state): State<Arc<RelayState>>,
    Json(body): Json<CreateTradeOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    // ML-DSA-65 verify off the async executor (fail-closed → reject). 401 unchanged.
    let sig_content = format!("trade_order\n{}\n{}\n{}", body.item_type, body.quantity, body.price_per_unit);
    let vk = body.public_key.clone();
    let vsig = body.signature.clone();
    let vts = body.timestamp;
    let sig_ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&vk, &sig_content, vts, &vsig)
    }).await.unwrap_or(false);
    if !sig_ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    if body.quantity <= 0 {
        return Err((StatusCode::BAD_REQUEST, "Quantity must be positive.".into()));
    }
    if body.price_per_unit <= 0.0 {
        return Err((StatusCode::BAD_REQUEST, "Price must be positive.".into()));
    }
    if body.item_type.is_empty() || body.item_type.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Invalid item_type.".into()));
    }

    let currency = body.currency.as_deref().unwrap_or("credits");
    if currency != "credits" && currency != "SOL" {
        return Err((StatusCode::BAD_REQUEST, "Currency must be 'credits' or 'SOL'.".into()));
    }

    let item_id = body.item_id.as_deref().unwrap_or("");
    match state.db.create_trade_order(
        &body.public_key,
        &body.item_type,
        item_id,
        body.quantity,
        body.price_per_unit,
        currency,
    ) {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id, "status": "open" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// DELETE /api/trade/orders/{id} — cancel a sell order (authenticated via Ed25519 sig).
pub async fn cancel_trade_order(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
    Query(q): Query<VaultSyncQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(q.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old.".into()));
    }

    // ML-DSA-65 verify off the async executor (fail-closed → reject). 401 unchanged.
    let sig_content = format!("cancel_order\n{}", order_id);
    let vk = q.key.clone();
    let vsig = q.sig.clone();
    let vts = q.timestamp;
    let sig_ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&vk, &sig_content, vts, &vsig)
    }).await.unwrap_or(false);
    if !sig_ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.cancel_trade_order(order_id, &q.key) {
        Ok(true) => Ok(Json(serde_json::json!({ "status": "cancelled" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Order not found or not yours.".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

#[derive(Debug, Deserialize)]
pub struct FillOrderRequest {
    pub public_key: String,
    pub timestamp: u64,
    pub signature: String,
    pub quantity: i64,
}

/// POST /api/trade/orders/{id}/fill — buy from a sell order (authenticated via Ed25519 sig).
pub async fn fill_trade_order(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(order_id): axum::extract::Path<i64>,
    Json(body): Json<FillOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::relay::handlers::broadcast::verify_dilithium_signature;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if now_ms.saturating_sub(body.timestamp) > 5 * 60 * 1000 {
        return Err((StatusCode::BAD_REQUEST, "Timestamp too old (> 5 min).".into()));
    }

    // ML-DSA-65 verify off the async executor (fail-closed → reject). 401 unchanged.
    let sig_content = format!("fill_order\n{}\n{}", order_id, body.quantity);
    let vk = body.public_key.clone();
    let vsig = body.signature.clone();
    let vts = body.timestamp;
    let sig_ok = tokio::task::spawn_blocking(move || {
        verify_dilithium_signature(&vk, &sig_content, vts, &vsig)
    }).await.unwrap_or(false);
    if !sig_ok {
        return Err((StatusCode::UNAUTHORIZED, "Signature verification failed.".into()));
    }

    match state.db.fill_trade_order(order_id, &body.public_key, body.quantity) {
        Ok(history_id) => Ok(Json(serde_json::json!({
            "status": "filled",
            "history_id": history_id,
        }))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub key: Option<String>,
    pub limit: Option<usize>,
}

/// GET /api/trade/history?key=...&limit=50 — get trade history for a user.
pub async fn get_trade_history(
    State(state): State<Arc<RelayState>>,
    Query(query): Query<TradeHistoryQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let key = query.key.unwrap_or_default();
    if key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "key is required.".into()));
    }
    let limit = query.limit.unwrap_or(50);
    match state.db.get_trade_history(&key, limit) {
        Ok(history) => Ok(Json(serde_json::json!(history))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))),
    }
}

/// GET /api/federation/servers — list federated servers (public).
pub async fn list_federation_servers(
    State(state): State<Arc<RelayState>>,
) -> Json<Vec<FederatedServerEntry>> {
    let servers = state.db.list_federated_servers().unwrap_or_default();
    let entries: Vec<FederatedServerEntry> = servers.into_iter().map(|s| FederatedServerEntry {
        server_id: s.server_id,
        name: s.name,
        url: s.url,
        public_key: s.public_key,
        trust_tier: s.trust_tier,
        accord_compliant: s.accord_compliant,
        status: s.status,
        last_seen: s.last_seen,
    }).collect();
    Json(entries)
}

/// Query parameters for GET /api/search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub channel: Option<String>,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/search?q=hello&channel=general&from=Michael&limit=20
pub async fn search_messages(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Query(params): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_api_auth(&headers)?;

    if params.q.len() < 2 || params.q.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "Query must be 2-200 characters".into()));
    }

    let limit = params.limit.unwrap_or(50).min(100) as usize;
    // API search doesn't include DMs (no requester context); pass empty key to exclude DM results.
    match state.db.search_messages_full(&params.q, params.channel.as_deref(), params.from.as_deref(), limit, "") {
        Ok(results) => {
            let search_results: Vec<SearchResultData> = results.into_iter().map(|(id, ch, msg)| {
                if let RelayMessage::Chat { from, from_name, content, timestamp, .. } = msg {
                    SearchResultData {
                        message_id: id,
                        channel: ch,
                        from: from.clone(),
                        from_name: from_name.unwrap_or_default(),
                        content,
                        timestamp,
                    }
                } else {
                    SearchResultData {
                        message_id: id, channel: ch, from: String::new(),
                        from_name: String::new(), content: String::new(), timestamp: 0,
                    }
                }
            }).collect();
            let total = search_results.len() as u32;
            Ok(Json(serde_json::json!({
                "query": params.q,
                "results": search_results,
                "total": total,
                "syntax": {
                    "description": "FTS5 full-text search",
                    "examples": {
                        "boolean": "hello AND world",
                        "phrase": "\"exact phrase\"",
                        "prefix": "hel*",
                        "exclude": "hello NOT goodbye",
                        "or": "hello OR hi",
                    }
                }
            })))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Search error: {e}"))),
    }
}
