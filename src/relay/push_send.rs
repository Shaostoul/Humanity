//! Web Push to a person's devices with no socket open: `RelayState::send_push_notification`,
//! moved out of relay.rs unchanged (2026-11-01, notifications rung 3, to keep relay.rs within its
//! file-size budget when the task notice joined). Who gets a push, and with what words, is the
//! caller's (the DM, the mention and the task notice: `storage::notification_prefs` `wants_push`).

use std::sync::Arc;

use web_push_native::jwt_simple::prelude::ES256KeyPair;

use super::RelayState;

impl RelayState {
    /// Send push notifications to all offline devices of a user.
    /// Fire-and-forget: spawns a tokio task, never blocks message routing.
    /// Automatically deletes stale subscriptions on 410 Gone.
    pub fn send_push_notification(
        self: &Arc<Self>,
        to_key: &str,
        title: &str,
        body: &str,
        tag: &str,
        url_path: &str,
    ) {
        use base64ct::{Base64UrlUnpadded, Encoding};

        let Some(ref vapid_kp) = self.vapid_key else { return };

        let subs = self.db.get_push_subscriptions(to_key);
        if subs.is_empty() { return; }

        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "tag": tag,
            "url": url_path,
        }).to_string();

        let client = self.http_client.clone();
        let vapid_bytes = vapid_kp.to_bytes();
        let state = self.clone();

        tokio::spawn(async move {
            let Ok(vapid_kp) = ES256KeyPair::from_bytes(&vapid_bytes) else { return };

            for sub in subs {
                // Decode subscription keys from base64url.
                let Ok(p256dh_bytes) = Base64UrlUnpadded::decode_vec(&sub.p256dh) else { continue };
                let Ok(auth_bytes) = Base64UrlUnpadded::decode_vec(&sub.auth) else { continue };
                let Ok(endpoint) = sub.endpoint.parse::<axum::http::Uri>() else { continue };
                let Ok(ua_public) = web_push_native::p256::PublicKey::from_sec1_bytes(&p256dh_bytes) else { continue };
                let ua_auth = web_push_native::Auth::clone_from_slice(&auth_bytes);

                let builder = web_push_native::WebPushBuilder::new(endpoint, ua_public, ua_auth)
                    .with_vapid(&vapid_kp, "mailto:admin@united-humanity.us");

                let http_req = match builder.build(payload.as_bytes()) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("Failed to build push message: {e}");
                        continue;
                    }
                };

                // Convert web_push_native http::Request to reqwest.
                let (parts, body_bytes) = http_req.into_parts();
                let url = parts.uri.to_string();
                let mut req = client.post(&url).body(body_bytes);
                for (k, v) in &parts.headers {
                    req = req.header(k.as_str(), v.to_str().unwrap_or_default());
                }

                match req.send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        if status == 410 || status == 404 {
                            // Subscription expired — remove it.
                            let _ = state.db.remove_push_subscription(&sub.endpoint);
                            tracing::info!("Removed stale push subscription ({})", status);
                        } else if status >= 400 {
                            tracing::warn!("Push failed for endpoint: HTTP {status}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Push request failed: {e}");
                    }
                }
            }
        });
    }
}
