//! The desktop's item page and Edit window against the REAL relay's round 5 routes (Media round 5,
//! lane 1D; the relay half is lane 1A's src/relay/media_edit.rs): the relay's own route functions
//! served over HTTP on this machine's loopback address, over a temporary database and folders of
//! the test's own (nothing lands in the checkout's data folder, nothing leaves the machine), and
//! the app driven the way a person drives it: Save pressed in the Edit window, files picked for
//! the item uploaded through the app's own resumable client and added to it, a file's Remove
//! confirmed. relay_round3_tests.rs is the pattern. Seen failing once before it passed; what it saw
//! red is in its comment.

use super::item_tests::{click_top, until};
use super::*;
use crate::gui::pages::media::api_items::{AttachTo, FileRole};
use crate::gui::pages::media::tests::ctx_and_theme;
use crate::gui::pages::media::{edit, sync_server, uploading};
use crate::relay::media::{publish_core, Folders, PublishBody, VisibilityBody};
use crate::relay::media_edit::{files_add_in, files_remove_in, FileAddBody, FileRemoveBody};
use crate::relay::relay::RelayState;
use crate::relay::resumable as rr;
use crate::relay::storage::media::{OwnedFolders, MEDIA_SHELF};
use crate::relay::storage::Storage;
use axum::extract::{DefaultBodyLimit, Path as AxPath, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use std::sync::Arc;

/// The relay's four folders, of this test's own.
struct Dirs {
    uploads: crate::test_temp::TempPath,
    members: crate::test_temp::TempPath,
    resume: crate::test_temp::TempPath,
    waiting: crate::test_temp::TempPath,
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
}

/// A relay served on a loopback port: its state, its folders, its address, and the runtime serving
/// it (dropped with it).
struct Relay {
    state: Arc<RelayState>,
    dirs: Arc<Dirs>,
    base: String,
    _rt: tokio::runtime::Runtime,
}

/// The relay's routes an item's page uses, each the relay's own function: the edit as it is mounted
/// in relay/mod.rs (it touches no folder), and the rest over this test's folders (the mounted ones
/// use the live folders): the resumable upload, a file added and taken off, and who can see it (its
/// mounted handler's three steps, the signature, the item named by its file and the change).
fn relay(tag: &str) -> Relay {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let state = Arc::new(RelayState::new(Storage::open_temp(tag)));
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    let dirs = Arc::new(Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting") });
    for p in [&dirs.uploads, &dirs.members, &dirs.resume, &dirs.waiting] {
        std::fs::create_dir_all(p).unwrap();
    }
    let (d1, d2, d3, d4, d5) = (dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone());
    let app = axum::Router::new()
        .route(
            "/api/upload/resumable",
            post(move |State(s): State<Arc<RelayState>>, Query(q): Query<rr::SessionQuery>, h: HeaderMap, axum::Json(b): axum::Json<rr::StartBody>| {
                let d = d1.clone();
                async move { rr::start_core(&s, d.folders(), rr::session_token(&h, &q).as_deref(), b).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response) }
            }),
        )
        .route(
            "/api/upload/resumable/{id}",
            get(rr::status)
                .patch(move |State(s): State<Arc<RelayState>>, AxPath(id): AxPath<String>, Query(q): Query<rr::SessionQuery>, h: HeaderMap, body: axum::body::Bytes| {
                    let d = d2.clone();
                    async move {
                        rr::piece_core(&s, d.folders(), &id, rr::session_token(&h, &q).as_deref(), q.offset, body).await.map(IntoResponse::into_response).unwrap_or_else(IntoResponse::into_response)
                    }
                })
                .layer(DefaultBodyLimit::max(rr::PIECE_BODY_LIMIT)),
        )
        .route("/api/media/edit", post(crate::relay::media_edit::edit))
        .route(
            "/api/media/files/add",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<FileAddBody>| {
                let d = d3.clone();
                async move { files_add_in(s, OwnedFolders::of(d.folders()), b).await.into_response() }
            }),
        )
        .route(
            "/api/media/files/remove",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<FileRemoveBody>| {
                let d = d4.clone();
                async move { files_remove_in(&s, d.folders(), &b).await.into_response() }
            }),
        )
        .route(
            "/api/media/visibility",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<VisibilityBody>| {
                let d = d5.clone();
                async move {
                    let purpose = format!("media_visibility:{}:{}", b.id, b.visibility);
                    let answer = async {
                        crate::relay::media::check_signed(&s, &b.key, b.timestamp, &b.sig, &purpose).await?;
                        crate::relay::media::same_item(&s.db, b.id, &b.filename)?;
                        crate::relay::media::visibility_core(&s.db, d.folders(), b.id, &b.key, &b.visibility)
                    };
                    answer.await.into_response()
                }
            }),
        )
        .with_state(state.clone());
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    rt.spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Relay { state, dirs, base: format!("http://127.0.0.1:{port}"), _rt: rt }
}

/// `n` bytes that are not all the same.
fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
}

/// AN ITEM EDITED AND GIVEN FILES ON THE REAL RELAY: Ann's model, published on the relay, is edited
/// from her Edit window (Save: the relay checks her signature over the edit's text exactly as sent,
/// and keeps the new words); a file and a cover picked for it go up through the app's resumable
/// client and are added to it (the relay holds them on the item, in its public folder); made
/// members only from the same window, every one of its files moves out of the public folder; and
/// the added file's Remove, confirmed on its page, takes it off the item and deletes it.
///
/// Seen red 2026-10-12 with `edit_request` sending the edit re-serialised (pretty-printed) after
/// hashing the compact text: "the relay took the edit (Signature verification failed.)" (left:
/// ("Dusk", "") / right: ("Dusk at the beach", "First line\nSecond line")).
#[test]
fn an_item_edited_and_given_files_on_the_real_relay() {
    let r = relay("desk_r5_item");
    let seed = [5u8; 32];
    let key = crate::net::identity::derive_pq_identity(&seed).expect("an identity").dilithium_hex;
    r.state.db.join_server(&key, "Ann").unwrap();
    r.state.db.set_role(&key, "verified").unwrap();
    let mut role = r.state.db.role_def("verified");
    (role.can_image_share, role.can_file_share) = (true, true);
    r.state.db.upsert_role(&role).unwrap();
    r.state.upload_tokens.try_write().expect("test setup: nobody holds the tokens").insert("tok_ann".into(), key.clone());

    // Ann's model, uploaded for Media and published.
    std::fs::write(r.dirs.waiting.join("1_dusk.glb"), b"glTF model bytes").unwrap();
    r.state.db.record_upload(&key, "1_dusk.glb", 100, MEDIA_SHELF, "dusk.glb", 16).unwrap();
    let settings = r.state.db.get_server_settings().unwrap();
    let body = PublishBody { key: key.clone(), filename: "1_dusk.glb".into(), title: "Dusk".into(), ..Default::default() };
    let answer = publish_core(&r.state.db, &settings, &[], r.dirs.folders(), &body).expect("published").0;
    let item = api::Item::from_json(&answer["item"]).expect("the relay's item, as the app reads it");
    let id = item.id;

    // Her app, signed in to this relay, the item on its page.
    crate::config::keep_saves_off_disk();
    let mut state = GuiState::default();
    state.server_url = r.base.clone();
    state.profile_public_key = key.clone();
    sync_server(&mut state);
    state.media.requested = true;
    state.upload_token = Some((r.base.clone(), "tok_ann".into()));
    state.private_key_bytes = Some(seed.to_vec());
    state.settings.files_in_messages.remove_details = false;
    state.media.list.items = vec![item.clone()];
    let (ctx, theme) = ctx_and_theme();
    let page_item = |s: &GuiState| s.media.item.as_ref().and_then(|p| p.item.clone()).expect("the page's item");
    let window_done = |s: &GuiState| !saving(s, id) && s.media.item.as_ref().is_some_and(|p| p.edit.as_ref().map_or(true, |f| !f.problem.is_empty()));
    let problem = |s: &GuiState| s.media.item.as_ref().and_then(|p| p.edit.as_ref()).map(|f| f.problem.clone()).unwrap_or_default();

    // 1. The words, from the Edit window: Save.
    edit::open_edit(&mut state, &item);
    {
        let form = state.media.item.as_mut().and_then(|p| p.edit.as_mut()).expect("the Edit window");
        form.title = "Dusk at the beach".into();
        form.description = "First line\nSecond line".into();
    }
    click_top(&ctx, &theme, &mut state, edit::SAVE);
    until(&ctx, &theme, &mut state, "the edit is answered", window_done);
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!((held.title.as_str(), held.description.as_str()), ("Dusk at the beach", "First line\nSecond line"), "the relay took the edit ({})", problem(&state));
    assert_eq!(page_item(&state).title, "Dusk at the beach", "and the page shows it");
    assert_eq!(state.media.item.as_ref().map(|p| p.status.as_str()), Some(SAVED));

    // 2. A file for it: up through the relay's resumable routes, then added to the item.
    let zip = crate::test_temp::file("desk_r5_zip", "zip");
    std::fs::write(&zip, bytes(3000)).unwrap();
    let to = |role| AttachTo { id, filename: item.stored.clone(), role };
    uploading::start_attach(&mut state, &zip, "textures.zip", 3000, to(FileRole::File)).expect("it starts");
    until(&ctx, &theme, &mut state, "the file is up and added", |s| s.media.uploads.is_empty() && !s.media.jobs.iter().any(|(j, _)| matches!(j, Job::Publish { .. })));
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!(held.files.iter().map(|f| f.original_name.as_str()).collect::<Vec<_>>(), vec!["textures.zip"], "the relay holds it on the item ({})", state.media.status);
    let added = held.files[0].clone();
    assert!(r.dirs.uploads.join(&added.filename).exists(), "in the public folder, with the item");
    assert_eq!(page_item(&state).files.len(), 2, "the page lists both files");

    // 3. A cover: a real picture, as the relay checks it.
    let png = crate::test_temp::file("desk_r5_cover", "png");
    let mut picture = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 120, 40, 255]))).write_to(&mut picture, image::ImageFormat::Png).unwrap();
    let picture = picture.into_inner();
    std::fs::write(&png, &picture).unwrap();
    uploading::start_attach(&mut state, &png, "cover.png", picture.len() as u64, to(FileRole::Cover)).expect("it starts");
    until(&ctx, &theme, &mut state, "the cover is up and added", |s| s.media.uploads.is_empty() && !s.media.jobs.iter().any(|(j, _)| matches!(j, Job::Publish { .. })));
    let cover = r.state.db.media_item(id).unwrap().unwrap().cover.unwrap_or_else(|| panic!("the relay holds the cover ({})", state.media.status));
    assert_eq!(cover.kind, "picture");
    assert!(page_item(&state).cover.is_some(), "the page has its cover");

    // 4. Members only, from the same window: every file moves out of the public folder.
    let now = page_item(&state);
    edit::open_edit(&mut state, &now);
    state.media.item.as_mut().and_then(|p| p.edit.as_mut()).expect("the Edit window").members_only = true;
    click_top(&ctx, &theme, &mut state, edit::SAVE);
    until(&ctx, &theme, &mut state, "the visibility is answered", window_done);
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!(held.visibility, "members", "made members only ({})", problem(&state));
    for name in [held.upload_filename.as_str(), added.filename.as_str(), cover.filename.as_str()] {
        assert!(r.dirs.members.join(name).exists() && !r.dirs.uploads.join(name).exists(), "{name} moved with the item");
    }
    assert!(page_item(&state).members_only(), "the page says so");

    // 5. The added file's Remove, confirmed on the page: off the item, and deleted.
    let file = page_item(&state).files.into_iter().find(|f| f.filename == added.filename).expect("the page lists the added file");
    state.media.item.as_mut().unwrap().confirm_file = Some(file);
    click_top(&ctx, &theme, &mut state, "Remove the file");
    until(&ctx, &theme, &mut state, "the remove is answered", |s| !s.media.jobs.iter().any(|(j, _)| matches!(j, Job::FileRemove { .. })));
    assert!(r.state.db.media_item(id).unwrap().unwrap().files.is_empty(), "off the item ({:?})", state.media.item.as_ref().map(|p| &p.status));
    assert!(!r.dirs.members.join(&added.filename).exists(), "and deleted");
    assert_eq!(state.media.item.as_ref().map(|p| p.status.as_str()), Some(FILE_REMOVED));
    assert_eq!(page_item(&state).files.len(), 1, "the page lists the item's own file alone");
}
