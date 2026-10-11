//! Uploads honour the uploader's own role limit (operator, 2026-10-10: "as admin my upload limit
//! is still only 6MB", and a video failed with "Error parsing multipart/form-data request"). The
//! route's body limit is the 1 GB ceiling (src/relay/mod.rs), not axum's 2 MB default; a request
//! that declares more than the role allows is refused before it is read; and the file streams to
//! a part file, refused the moment it passes the limit, never held in memory whole.

use super::*;
use axum::extract::Query;

/// A multipart request carrying one file field of `len` bytes, as a browser builds it.
fn form(len: usize) -> axum::http::Request<axum::body::Body> {
    let boundary = "humanityTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"clip.mp4\"\r\nContent-Type: video/mp4\r\n\r\n").as_bytes());
    body.extend(std::iter::repeat(7u8).take(len));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    axum::http::Request::builder()
        .method("POST")
        .uri("/api/upload")
        .header(axum::http::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .header(axum::http::header::CONTENT_LENGTH, body.len().to_string())
        .body(axum::body::Body::from(body))
        .unwrap()
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("hos-upload-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

/// THE ROLE'S LIMIT IS THE LIMIT: a role set to 2 GB gets the 1 GB ceiling, one set to 0 or less
/// gets 1 MB, anything between gets exactly what was set.
#[test]
fn a_roles_upload_limit_is_what_server_settings_says_up_to_the_ceiling() {
    let mut role = crate::relay::storage::RoleDef::default();
    role.max_upload_mb = 300;
    assert_eq!(role_upload_limit(&role), 300 * 1024 * 1024);
    role.max_upload_mb = 2048;
    assert_eq!(role_upload_limit(&role), UPLOAD_HARD_CEILING, "held to the 1 GB ceiling");
    role.max_upload_mb = 0;
    assert_eq!(role_upload_limit(&role), 1024 * 1024, "never below 1 MB");
    assert!(UPLOAD_BODY_LIMIT as u64 > UPLOAD_HARD_CEILING, "the route's body limit leaves room for the form around a full-size file");
}

/// A REQUEST THAT SAYS IT IS TOO BIG IS REFUSED BEFORE IT IS READ, with the person's limit in the
/// words; one within the limit (form overhead included) is not. Seen red 2026-10-10 with the
/// check returning None: "a 50 MB request against a 25 MB limit is refused up front".
#[test]
fn a_request_declared_over_the_limit_is_refused_before_it_is_read() {
    let limit = 25 * 1024 * 1024;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_LENGTH, (50u64 * 1024 * 1024).to_string().parse().unwrap());
    let refused = declared_too_large(&headers, limit).expect("a 50 MB request against a 25 MB limit is refused up front");
    assert_eq!(refused.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(refused.1.contains("your limit on this server is 25 MB"), "{}", refused.1);
    headers.insert(axum::http::header::CONTENT_LENGTH, (limit + 1000).to_string().parse().unwrap());
    assert!(declared_too_large(&headers, limit).is_none(), "the form's own bytes around a full-size file are allowed");
    assert!(declared_too_large(&axum::http::HeaderMap::new(), limit).is_none(), "no declared size: checked while streaming instead");
}

/// A FILE BIGGER THAN AXUM'S OLD 2 MB DEFAULT STREAMS TO A PART FILE WHOLE (the operator's video
/// failed there), through the route's own body limit (`DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)`,
/// as src/relay/mod.rs sets it), and one over the limit is refused mid-stream with nothing left
/// behind.
/// Served for real on a loopback port. Seen red 2026-10-10 two ways: with the route's layer left
/// off (axum's 2 MB default), "a 3 MB file is taken" failed; with the size check taken out of the
/// stream loop, "a file over the limit is refused while it streams" failed.
#[test]
fn a_file_streams_to_a_part_file_and_is_refused_mid_stream_past_the_limit() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rt = runtime();
    let dir = scratch("stream");
    let served = dir.clone();
    rt.block_on(async move {
        // The handler: the field streamed with a limit taken from the query (?max=bytes).
        let app = axum::Router::new()
            .route(
                "/api/upload",
                axum::routing::post(move |Query(q): Query<std::collections::HashMap<String, usize>>, mut multipart: axum::extract::Multipart| {
                    let dir = served.clone();
                    async move {
                        let mut field = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.expect("one field");
                        let part = stream_to_part(&mut field, q["max"], &dir).await?;
                        Ok::<_, (StatusCode, String)>(part.len.to_string())
                    }
                }),
            )
            .layer(axum::extract::DefaultBodyLimit::max(UPLOAD_BODY_LIMIT));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // POST `len` bytes with a `max` limit; the status line and body of the answer.
        let post = |len: usize, max: usize| async move {
            let req = form(len);
            let (parts, body) = req.into_parts();
            let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
            let mut head = format!("POST /api/upload?max={max} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
            for (k, v) in parts.headers.iter() {
                head.push_str(&format!("{}: {}\r\n", k, v.to_str().unwrap()));
            }
            head.push_str("\r\n");
            let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
            s.write_all(head.as_bytes()).await.unwrap();
            let _ = s.write_all(&body).await; // the server may stop reading once it refuses
            let mut out = Vec::new();
            let _ = s.read_to_end(&mut out).await;
            String::from_utf8_lossy(&out).to_string()
        };

        let three_mb = 3 * 1024 * 1024;
        let answer = post(three_mb, 25 * 1024 * 1024).await;
        assert!(answer.starts_with("HTTP/1.1 200") && answer.ends_with(&three_mb.to_string()), "a 3 MB file is taken: {answer}");

        let answer = post(three_mb, 2 * 1024 * 1024).await;
        assert!(answer.starts_with("HTTP/1.1 413"), "a file over the limit is refused while it streams: {answer}");
        assert!(answer.contains("your limit on this server is 2 MB"), "{answer}");
    });
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 1, "only the accepted file's part is there; the refused one was removed: {left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
