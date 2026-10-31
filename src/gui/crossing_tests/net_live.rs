//! From net/live.rs's tests: a real frame survives the whole pipeline to a viewer, through a real
//! relay's live handlers.

use crate::net::live::*;
use crate::net::live::TAG_KEYFRAME;
use crate::raw_frame::RawFrame;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// THE end-to-end test: a real `LivePublisher` (real identity, real JPEG encoder,
/// real WebSocket) against a real relay, with a real viewer on the other side that
/// DECODES what arrives and checks the pixels.
///
/// Unit-testing the envelope and the downscale separately proves neither of them
/// is wired to anything. This proves the whole chain: a frame in the GPU's exact
/// layout (BGRA, 256-padded rows) goes in one end, and a correctly-sized,
/// correctly-coloured, decodable image comes out the other.
///
/// Native-only because that is where the publisher lives; the `native` feature
/// includes `relay`, so both halves are available here.
#[tokio::test]
async fn a_real_frame_survives_the_whole_pipeline_to_a_viewer() {
    use axum::routing::get;
    use futures::StreamExt;

    // --- Relay with our test identity's name registered.
    let db = crate::relay::storage::Storage::open_temp("live_e2e");

    let seed = [7u8; 32];
    let pq = crate::net::identity::derive_pq_identity(&seed).expect("derive identity");
    db.register_name("streamer", &pq.dilithium_hex).expect("register name");

    let state = Arc::new(crate::relay::relay::RelayState::new(db));
    let app = axum::Router::new()
        .route("/ws/live/pub", get(crate::relay::live::pub_handler))
        .route("/ws/live/sub/{stream}", get(crate::relay::live::sub_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // --- Start the real publisher.
    let cfg = LiveConfig {
        server: format!("http://127.0.0.1:{port}"),
        title: "E2E".into(),
        chat: String::new(),
        target_height: 36, // tiny, so the test is fast
        quality: 90,
        fps: 30,
    };
    let mut publisher = LivePublisher::start(cfg, &seed);

    // Let the worker connect and authenticate before a viewer subscribes.
    // Waits for the connection, up to 30 s, rather than a fixed 400 ms:
    // the whole lib suite beside several agent builds missed 400 ms once
    // (2026-09-27, the BUG-099 class). A quiet machine connects at once.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !publisher.stats().connected.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        publisher.stats().connected.load(Ordering::Relaxed),
        "publisher failed to authenticate: {}",
        publisher.stats().error.lock().unwrap()
    );

    let (mut viewer, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws/live/sub/streamer"))
            .await
            .expect("viewer connects");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // --- Feed it a frame in the GPU's real layout: BGRA, 256-aligned row padding.
    let (w, h) = (128u32, 72u32);
    let bpr = ((w * 4).div_ceil(256)) * 256;
    let mut pixels = vec![0u8; (bpr * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (y * bpr + x * 4) as usize;
            pixels[i] = 32; // B
            pixels[i + 1] = 64; // G
            pixels[i + 2] = 200; // R  (a distinctly red-ish pixel)
            pixels[i + 3] = 255;
        }
    }
    // --- Receive it, and actually DECODE it. The frame is offered again
    // every 200 ms until the viewer has one, up to 30 s: the 150 ms
    // settle above is a hint, not a guarantee, and a loaded machine
    // (several agent builds, 2026-09-27) can register the viewer after a
    // single frame has already gone out to nobody.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let msg = loop {
        publisher.submit_frame(RawFrame {
            pixels: pixels.clone(),
            width: w,
            height: h,
            bytes_per_row: bpr,
            bgra: true,
        });
        match tokio::time::timeout(Duration::from_millis(200), viewer.next()).await {
            Ok(next) => break next.expect("the stream stays open").expect("a frame, not an error"),
            Err(_) => assert!(std::time::Instant::now() < deadline, "a frame should have arrived within 30 s"),
        }
    };
    let data = msg.into_data();

    assert!(data.len() > 9, "frame must carry a payload, not just a header");
    assert_eq!(data[0], TAG_KEYFRAME, "MJPEG frames are self-contained keyframes");

    let jpeg = &data[9..];
    let img = image::load_from_memory(jpeg).expect("the payload must be a decodable JPEG");
    assert_eq!(
        (img.width(), img.height()),
        (64, 36),
        "128x72 downscaled to a 36px target height, aspect ratio preserved"
    );

    // The pixels must have survived the BGRA un-swizzle. If R and B were swapped,
    // this red-ish frame would decode blue-ish.
    let px = img.to_rgb8();
    let p = px.get_pixel(32, 18);
    assert!(
        p[0] > 150 && p[2] < 100,
        "expected a red-ish pixel (BGRA un-swizzled), got {p:?}"
    );

    assert_eq!(publisher.stats().sent.load(Ordering::Relaxed), 1);
    assert!(publisher.stats().bytes.load(Ordering::Relaxed) > 0, "bitrate must be measured");
}
