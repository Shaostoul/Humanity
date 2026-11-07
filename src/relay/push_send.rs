//! Web Push to a person's devices with no socket open: `RelayState::send_push_notification`,
//! moved out of relay.rs unchanged (2026-11-01, notifications rung 3, to keep relay.rs within its
//! file-size budget when the task notice joined). Who gets a push, and with what words, is the
//! caller's (the DM, the mention and the task notice: `storage::notification_prefs` `wants_push`).
//! The encryption and the VAPID token are `relay::web_push` (our own since 2026-11-06).
//!
//! WHERE A PUSH GOES (2026-11-06, BUG-255): every endpoint is checked again here, at send, and
//! sent only through `outbound`'s guarded client: its shape first (a host written as an IP
//! address would skip any resolver), then the client's own resolver, which connects only to
//! public addresses it has just checked, follows no redirect and uses no proxy. A subscription
//! that fails is skipped and kept, never removed: a fault in the rule must not silently forget
//! people's subscriptions while their browsers think they are still subscribed.

use std::sync::Arc;

use super::RelayState;
use crate::relay::outbound::Schemes;
use crate::relay::storage::PushSubscriptionRecord;
use crate::relay::web_push;

/// The VAPID token's subject: who a push service contacts about our pushes (RFC 8292 section 2.1).
const CONTACT: &str = "mailto:admin@united-humanity.us";

impl RelayState {
    /// Send push notifications to all offline devices of a user.
    /// Fire-and-forget: spawns a tokio task, never blocks message routing.
    /// Automatically deletes stale subscriptions on 404 Not Found and 410 Gone.
    pub fn send_push_notification(
        self: &Arc<Self>,
        to_key: &str,
        title: &str,
        body: &str,
        tag: &str,
        url_path: &str,
    ) {
        if self.vapid_key.is_none() { return; }

        let subs = self.db.get_push_subscriptions(to_key);
        if subs.is_empty() { return; }

        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "tag": tag,
            "url": url_path,
        }).to_string();

        let state = self.clone();
        tokio::spawn(async move { state.deliver_pushes(subs, payload).await });
    }

    /// Send `payload` to each of `subs`, one after another, returning when all were tried (the
    /// tests await it to know nothing more will be sent).
    pub(crate) async fn deliver_pushes(&self, subs: Vec<PushSubscriptionRecord>, payload: String) {
        let Some(vapid) = self.vapid_key.as_ref() else { return };
        let Some(client) = self.outbound.client(Schemes::HttpsOnly) else { return };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for sub in subs {
            // Never the endpoint itself in the log: it is the subscription's secret address.
            if let Err(why) = self.outbound.shape(&sub.endpoint, Schemes::HttpsOnly) {
                tracing::warn!("Push skipped: the subscription's address is refused ({})", why.reason());
                continue;
            }
            let push = match web_push::build_request(&sub.endpoint, &sub.p256dh, &sub.auth, payload.as_bytes(), vapid, CONTACT, now) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Failed to build push message: {e}");
                    continue;
                }
            };
            let mut req = client.post(&push.url).body(push.body);
            for (k, v) in &push.headers {
                req = req.header(*k, v.as_str());
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if status == 410 || status == 404 {
                        // Subscription expired — remove it.
                        let _ = self.db.remove_push_subscription(&sub.endpoint);
                        tracing::info!("Removed stale push subscription ({})", status);
                    } else if status >= 400 {
                        tracing::warn!("Push failed for endpoint: HTTP {status}");
                    }
                }
                Err(e) => {
                    // A name leading to a non-public address is refused here, by the resolver.
                    tracing::warn!("Push request failed: {e}");
                }
            }
        }
    }
}
