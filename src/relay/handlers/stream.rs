//! LIVESTREAMING: going live, the WebRTC handshake that carries the picture,
//! the viewer roster, the stream's own chat, and pointing viewers at an
//! external platform instead.
//!
//! Extracted VERBATIM from `relay/handlers/msg_handlers.rs` (2026-09-19) under
//! the file-size ratchet, which msg_handlers.rs had outgrown at 4,936 lines
//! against a 4,800 budget.
//!
//! WHY THIS IS THE CLUSTER. msg_handlers.rs is a dozen unrelated feature
//! domains sharing one file because they all arrived as `match` arms of the
//! same connection loop. Livestreaming is the one that reads most like its own
//! subsystem: eleven handlers that talk to each other and to
//! `state.live_streams`, one WebRTC signalling triangle (offer, answer, ice)
//! that only makes sense as a set, and its own test module. Nothing outside
//! calls in except `relay.rs`'s match arms.
//!
//! The rule the whole file exists to hold: THE RELAY NEVER CARRIES THE VIDEO.
//! It introduces a viewer to a broadcaster and forwards the SDP and ICE
//! blobs; the media itself goes peer to peer. That is why every handler here
//! is a few lines of routing and the only state is a roster.
//!
//! Declared as a `#[path]` CHILD of `msg_handlers`, so one `use super::*`
//! brings in `RelayState`, the broadcast helpers and the storage traits, and
//! the parent's `pub use` keeps every `handle_stream_*` call in `relay.rs`
//! resolving through both `msg_handlers::` and `handlers::`. No call site
//! changed and `handlers/mod.rs` needed no edit.

use super::*;

pub async fn handle_stream_start(
    state: &Arc<RelayState>,
    my_key: &str,
    title: String,
    category: String,
) {
    // Capability composition (v0.239, docs/design/roles-system.md):
    //   effective_can_stream = server master switch AND role.can_stream
    // The server-wide video_streaming_enabled is the master kill-switch;
    // the per-role can_stream decides who, when it's on. Seed defaults:
    // mod + admin can_stream=1, so the operator opts other roles in by
    // editing/creating a role (e.g. a "Family" role with can_stream=1).
    let role = state.db.get_role(my_key).unwrap_or_default();
    let settings = state.db.get_server_settings().unwrap_or_default();
    let rd = state.db.role_def(&role);
    let may_stream = settings.video_streaming_enabled && rd.can_stream;
    if !may_stream {
        let private = RelayMessage::Private {
            to: my_key.to_string(),
            message: if !settings.video_streaming_enabled {
                "Streaming is disabled server-wide (enable it in Server Settings).".to_string()
            } else {
                format!("Your role \"{}\" is not permitted to stream on this server.", rd.label)
            },
        };
        let _ = state.broadcast_tx.send(private);
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let streamer_name = {
            let peers = state.peers.read().await;
            peers.get(my_key).and_then(|p| p.display_name.clone()).unwrap_or_else(|| "Unknown".to_string())
        };
        let db_id = state.db.create_stream(my_key, &title, &category).ok();
        let stream = ActiveStream {
            streamer_key: my_key.to_string(),
            streamer_name: streamer_name.clone(),
            title: title.clone(),
            category: category.clone(),
            started_at: now,
            viewer_keys: HashSet::new(),
            peak_viewers: 0,
            external_urls: Vec::new(),
            db_id,
        };
        *state.active_stream.write().await = Some(stream);
        tracing::info!("Stream started by {} ({}): {}", streamer_name, my_key, title);
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(streamer_name),
            streamer_key: Some(my_key.to_string()),
            title: Some(title),
            category: Some(category),
            viewer_count: 0,
            started_at: Some(now),
            external_urls: None,
        };
        let _ = state.broadcast_tx.send(info_msg);
    }
}

pub async fn handle_stream_stop(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref stream) = *stream_lock {
        let role = state.db.get_role(my_key).unwrap_or_default();
        if stream.streamer_key == my_key || role == "admin" {
            // The tracked high-water mark, not the live count at stop time --
            // by the time a stream ends most viewers have often already left,
            // so `viewer_keys.len()` here is frequently 0 or far below the
            // real peak (see ActiveStream::peak_viewers' doc comment).
            let viewer_peak = stream.peak_viewers as i64;
            if let Some(db_id) = stream.db_id {
                let _ = state.db.end_stream(db_id, viewer_peak);
            }
            tracing::info!("Stream stopped by {}", my_key);
            *stream_lock = None;
            drop(stream_lock);
            let info_msg = RelayMessage::StreamInfo {
                active: false,
                streamer_name: None,
                streamer_key: None,
                title: None,
                category: None,
                viewer_count: 0,
                started_at: None,
                external_urls: None,
            };
            let _ = state.broadcast_tx.send(info_msg);
        }
    }
}

pub async fn handle_stream_offer(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamOffer from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamOffer {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_answer(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamAnswer from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamAnswer {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_ice(
    state: &Arc<RelayState>,
    my_key: &str,
    to: String,
    data: serde_json::Value,
) {
    tracing::info!("StreamIce from {} to {}", &my_key[..8], &to[..8]);
    let _ = state.broadcast_tx.send(RelayMessage::StreamIce {
        from: my_key.to_string(),
        to,
        data,
    });
}

pub async fn handle_stream_viewer_join(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    tracing::info!("Stream viewer join from {}", my_key);
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref mut stream) = *stream_lock {
        stream.viewer_keys.insert(my_key.to_string());
        let count = stream.viewer_keys.len();
        // The count is highest right here, at a join -- this is the ONLY
        // place the true peak can be observed, so record it now rather than
        // waiting to read a (by-then-lower) count at leave/stop time.
        stream.peak_viewers = stream.peak_viewers.max(count);
        let count = count as u32;
        let streamer_key = stream.streamer_key.clone();
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(stream.streamer_name.clone()),
            streamer_key: Some(stream.streamer_key.clone()),
            title: Some(stream.title.clone()),
            category: Some(stream.category.clone()),
            viewer_count: count,
            started_at: Some(stream.started_at),
            external_urls: Some(stream.external_urls.clone()),
        };
        drop(stream_lock);
        let _ = state.broadcast_tx.send(info_msg);
        tracing::info!("Sending __stream_viewer_ready__ to streamer {} for viewer {}", streamer_key, my_key);
        let notify = RelayMessage::Private {
            to: streamer_key,
            message: format!("__stream_viewer_ready__:{}", my_key),
        };
        let _ = state.broadcast_tx.send(notify);
    }
}

pub async fn handle_stream_viewer_leave(
    state: &Arc<RelayState>,
    my_key: &str,
) {
    let mut stream_lock = state.active_stream.write().await;
    if let Some(ref mut stream) = *stream_lock {
        stream.viewer_keys.remove(my_key);
        let count = stream.viewer_keys.len() as u32;
        // Persist the tracked high-water mark, NOT the post-leave live count
        // (which just decreased and is never the actual peak -- see
        // ActiveStream::peak_viewers' doc comment).
        let peak = stream.peak_viewers as i64;
        if let Some(db_id) = stream.db_id {
            let _ = state.db.update_stream_viewer_peak(db_id, peak);
        }
        let info_msg = RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(stream.streamer_name.clone()),
            streamer_key: Some(stream.streamer_key.clone()),
            title: Some(stream.title.clone()),
            category: Some(stream.category.clone()),
            viewer_count: count,
            started_at: Some(stream.started_at),
            external_urls: Some(stream.external_urls.clone()),
        };
        drop(stream_lock);
        let _ = state.broadcast_tx.send(info_msg);
    }
}

pub async fn handle_stream_chat(
    state: &Arc<RelayState>,
    my_key: &str,
    content: String,
    source: String,
    source_user: Option<String>,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let from_name = {
        let peers = state.peers.read().await;
        peers.get(my_key).and_then(|p| p.display_name.clone())
    };
    {
        let stream = state.active_stream.read().await;
        if let Some(ref s) = *stream {
            if let Some(db_id) = s.db_id {
                let _ = state.db.store_stream_chat(
                    db_id, &content,
                    from_name.as_deref().unwrap_or("Unknown"),
                    &source,
                );
            }
        }
    }
    let chat_msg = RelayMessage::StreamChat {
        content,
        source,
        source_user,
        from: Some(my_key.to_string()),
        from_name,
        timestamp: now,
    };
    let _ = state.broadcast_tx.send(chat_msg);
}

pub async fn handle_stream_info_request(
    state: &Arc<RelayState>,
) {
    let stream = state.active_stream.read().await;
    let info_msg = if let Some(ref s) = *stream {
        RelayMessage::StreamInfo {
            active: true,
            streamer_name: Some(s.streamer_name.clone()),
            streamer_key: Some(s.streamer_key.clone()),
            title: Some(s.title.clone()),
            category: Some(s.category.clone()),
            viewer_count: s.viewer_keys.len() as u32,
            started_at: Some(s.started_at),
            external_urls: Some(s.external_urls.clone()),
        }
    } else {
        RelayMessage::StreamInfo {
            active: false,
            streamer_name: None,
            streamer_key: None,
            title: None,
            category: None,
            viewer_count: 0,
            started_at: None,
            external_urls: None,
        }
    };
    let _ = state.broadcast_tx.send(info_msg);
}

pub async fn handle_stream_set_external(
    state: &Arc<RelayState>,
    my_key: &str,
    urls: Vec<StreamExternalUrl>,
) {
    let role = state.db.get_role(my_key).unwrap_or_default();
    if role == "admin" {
        let mut stream_lock = state.active_stream.write().await;
        if let Some(ref mut stream) = *stream_lock {
            stream.external_urls = urls;
            let info_msg = RelayMessage::StreamInfo {
                active: true,
                streamer_name: Some(stream.streamer_name.clone()),
                streamer_key: Some(stream.streamer_key.clone()),
                title: Some(stream.title.clone()),
                category: Some(stream.category.clone()),
                viewer_count: stream.viewer_keys.len() as u32,
                started_at: Some(stream.started_at),
                external_urls: Some(stream.external_urls.clone()),
            };
            drop(stream_lock);
            let _ = state.broadcast_tx.send(info_msg);
        }
    }
}

// ── Livestream handler tests (v0.645, overnight-loop priority #2 verification) ──
#[cfg(test)]
mod stream_tests {
    use super::*;
    use crate::relay::relay::RelayState;

    fn fresh_state() -> Arc<RelayState> {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_stream_{pid}_{nanos}.db"));
        let db = Storage::open(&path).expect("open test db");
        Arc::new(RelayState::new(db))
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
    }

    /// Enable the server-wide streaming switch (off by default) and grant the
    /// streamer's role can_stream (mirrors the real admin/mod seed defaults).
    fn allow_streaming(st: &Arc<RelayState>, streamer_key: &str) {
        st.db.set_role(streamer_key, "admin").unwrap();
        let mut settings = st.db.get_server_settings().unwrap_or_default();
        settings.video_streaming_enabled = true;
        st.db.set_server_settings(&settings, streamer_key).unwrap();
    }

    /// The exact bug this test guards: `viewer_peak` used to be fed the LIVE
    /// `viewer_keys.len()` at leave/stop time, which is only ever highest
    /// right at a join and monotonically decreases from there -- so by the
    /// time a stream actually ends (viewers usually already trickled out),
    /// the persisted peak was frequently 0 or far below the real maximum.
    /// Two viewers join (peak momentarily 2), both leave (live count back to
    /// 0), THEN the stream stops -- the persisted viewer_peak must still be
    /// the true historical 2, not the live-at-stop-time 0.
    #[test]
    fn viewer_peak_survives_viewers_leaving_before_stream_stops() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Test Stream".to_string(), "testing".to_string()));
        block(handle_stream_viewer_join(&st, "viewer1_key"));
        block(handle_stream_viewer_join(&st, "viewer2_key"));
        // Peak is 2 right now -- both viewers leave before anyone checks.
        block(handle_stream_viewer_leave(&st, "viewer1_key"));
        block(handle_stream_viewer_leave(&st, "viewer2_key"));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        assert_eq!(streams.len(), 1, "expected exactly one recorded stream");
        let (_, streamer_key, _, _, _, ended_at, viewer_peak) = &streams[0];
        assert_eq!(streamer_key, "streamer_key");
        assert!(ended_at.is_some(), "stream_stop must set ended_at");
        assert_eq!(*viewer_peak, 2, "the persisted peak must be the true historical max (2), not the live count at stop time (0)");
    }

    /// A single viewer who joins then leaves (peak 1) must not be recorded
    /// as 0 just because they left before the stream ended -- the simplest
    /// case of the same bug class, kept separate so a regression here is
    /// unambiguous about which scenario broke.
    #[test]
    fn viewer_peak_of_one_is_not_lost_when_that_viewer_leaves() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Solo Viewer Stream".to_string(), "testing".to_string()));
        block(handle_stream_viewer_join(&st, "viewer1_key"));
        block(handle_stream_viewer_leave(&st, "viewer1_key"));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        let (_, _, _, _, _, _, viewer_peak) = &streams[0];
        assert_eq!(*viewer_peak, 1);
    }

    /// A stream nobody ever watches must record a peak of 0, not error or
    /// panic -- the zero-viewer path through the same code.
    #[test]
    fn a_stream_with_no_viewers_records_zero_peak() {
        let st = fresh_state();
        allow_streaming(&st, "streamer_key");

        block(handle_stream_start(&st, "streamer_key", "Empty Room".to_string(), "testing".to_string()));
        block(handle_stream_stop(&st, "streamer_key"));

        let streams = st.db.get_recent_streams(1).unwrap();
        let (_, _, _, _, _, _, viewer_peak) = &streams[0];
        assert_eq!(*viewer_peak, 0);
    }

    /// Streaming is refused when the server-wide switch is off, even for an
    /// admin -- the master-kill-switch half of the authorization check
    /// (`may_stream = settings.video_streaming_enabled && rd.can_stream`).
    #[test]
    fn streaming_disabled_server_wide_blocks_even_an_admin() {
        let st = fresh_state();
        st.db.set_role("streamer_key", "admin").unwrap();
        // Deliberately do NOT enable video_streaming_enabled.
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_stream_start(&st, "streamer_key", "Should Not Start".to_string(), "testing".to_string()));
        let mut saw_refusal = false;
        while let Ok(msg) = rx.try_recv() {
            if let RelayMessage::Private { message, .. } = msg {
                if message.contains("disabled server-wide") {
                    saw_refusal = true;
                }
            }
        }
        assert!(saw_refusal, "expected a server-wide-disabled refusal message");
        assert_eq!(st.db.get_recent_streams(10).unwrap().len(), 0, "no stream row should be created");
    }
}
