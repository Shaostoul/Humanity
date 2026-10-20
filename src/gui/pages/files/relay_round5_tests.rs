//! The desktop's item page and edit form against the REAL relay's round 5 routes (Media round 5,
//! lane 1D; the relay half is lane 1A's src/relay/files_edit.rs): the relay's own route functions
//! served over HTTP on this machine's loopback address, over a temporary database and folders of
//! the test's own (nothing lands in the checkout's data folder, nothing leaves the machine), and
//! the app driven the way a person drives it: Save pressed in the edit form, files picked for
//! the item uploaded through the app's own resumable client and added to it, a file's Remove
//! confirmed. relay_round3_tests.rs is the pattern. Seen failing once before it passed; what it saw
//! red is in its comment.

use super::item_tests::{click_at, click_top, every_exact, find_text, until};
use super::*;
use crate::gui::pages::files::api_items::{AttachTo, FileRole};
use crate::gui::pages::files::tests::ctx_and_theme;
use crate::gui::pages::files::{edit, sync_server, uploading};
use crate::relay::files::{publish_core, Folders, PublishBody, VisibilityBody};
use crate::relay::files_edit::{files_add_in, files_remove_in, FileAddBody, FileRemoveBody};
use crate::relay::relay::RelayState;
use crate::relay::resumable as rr;
use crate::relay::storage::files::{OwnedFolders, MEDIA_SHELF};
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
    let (d1, d2, d3, d4, d5, d6) = (dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone(), dirs.clone());
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
        .route("/api/files/edit", post(crate::relay::files_edit::edit))
        // Lane 1B's counting routes: a view as mounted (it touches no folder), a ticket as mounted,
        // and the counted download over this test's folders.
        .route("/api/files/viewed", post(crate::relay::files_counts::viewed))
        .route("/api/files/ticket", post(crate::relay::files_ticket::ticket))
        .route(
            "/api/files/download/{id}",
            get(move |State(s): State<Arc<RelayState>>, AxPath(id): AxPath<i64>, Query(q): Query<crate::relay::files::TokenQuery>, req: axum::extract::Request| {
                let d = d6.clone();
                async move {
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
                    crate::relay::files_counts::download_from(&s, d.folders(), id, q, req, now).await
                }
            }),
        )
        .route(
            "/api/files/item-files/add",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<FileAddBody>| {
                let d = d3.clone();
                async move { files_add_in(s, OwnedFolders::of(d.folders()), b).await.into_response() }
            }),
        )
        .route(
            "/api/files/item-files/remove",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<FileRemoveBody>| {
                let d = d4.clone();
                async move { files_remove_in(&s, d.folders(), &b).await.into_response() }
            }),
        )
        .route(
            "/api/files/visibility",
            post(move |State(s): State<Arc<RelayState>>, axum::Json(b): axum::Json<VisibilityBody>| {
                let d = d5.clone();
                async move {
                    let purpose = format!("media_visibility:{}:{}", b.id, b.visibility);
                    let answer = async {
                        crate::relay::files::check_signed(&s, &b.key, b.timestamp, &b.sig, &purpose).await?;
                        crate::relay::files::same_item(&s.db, b.id, &b.filename)?;
                        crate::relay::files::visibility_core(&s.db, d.folders(), b.id, &b.key, &b.visibility)
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

/// Press Download on the item page's file row holding `row`, wait for it to end, and return what
/// it saved (the file is deleted after it is read).
fn download_row(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, row: &str, file: i64) -> Vec<u8> {
    let out = crate::gui::pages::files::tests::settled(ctx, theme, state);
    let at = find_text(&out, row).unwrap_or_else(|| panic!("the row {row:?} is drawn"));
    let press = every_exact(&out, "Download").into_iter().min_by(|a, b| (a.center().y - at.center().y).abs().total_cmp(&(b.center().y - at.center().y).abs())).expect("its Download");
    click_at(ctx, theme, state, press.center());
    until(ctx, theme, state, "the download ends", |s| s.files.open.as_ref().is_some_and(|o| o.saved_words(file).is_some()));
    let words = state.files.open.as_ref().and_then(|o| o.saved_words(file)).unwrap_or_default().to_string();
    let path = words.strip_prefix("Saved to ").unwrap_or_else(|| panic!("saved: {words}")).to_string();
    let got = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    got
}

/// AN ITEM EDITED AND GIVEN FILES ON THE REAL RELAY: Ann's model, published on the relay, is edited
/// from her edit form (Save: the relay checks her signature over the edit's text exactly as sent,
/// and keeps the new words), and opening the page counted a view; a file and a cover picked for it
/// go up through the app's resumable client and are added to it (the relay holds them on the item,
/// in its public folder), and the file's Download on the page comes through the relay's counting
/// route and is counted; made members only from the same window, every one of its files moves out
/// of the public folder, and the cover's Download comes through a ticket made for the counting
/// route, counted for the member; and the added file's Remove, confirmed on its page, takes it off
/// the item and deletes it.
///
/// Seen red 2026-10-12 with `edit_request` sending the edit re-serialised (pretty-printed) after
/// hashing the compact text: "the relay took the edit (Signature verification failed.)" (left:
/// ("Dusk", "") / right: ("Dusk at the beach", "First line\nSecond line")). Its download legs
/// (added once lane 1B's routes merged) seen red with `download_origin` fetching the item's
/// `/uploads/` address, which nothing serves here: "saved: Not saved: The server refused it (404).".
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
    state.files.requested = true;
    state.upload_token = Some((r.base.clone(), "tok_ann".into()));
    state.private_key_bytes = Some(seed.to_vec());
    state.settings.files_in_messages.remove_details = false;
    state.files.list.items = vec![item.clone()];
    let (ctx, theme) = ctx_and_theme();
    let page_item = |s: &GuiState| s.files.item.as_ref().and_then(|p| p.item.clone()).expect("the page's item");
    let window_done = |s: &GuiState| !saving(s, id) && s.files.item.as_ref().is_some_and(|p| p.edit.as_ref().map_or(true, |f| !f.problem.is_empty()));
    let problem = |s: &GuiState| s.files.item.as_ref().and_then(|p| p.edit.as_ref()).map(|f| f.problem.clone()).unwrap_or_default();

    // 1. The words, from the edit form: Save.
    edit::open_edit(&mut state, &item);
    {
        let form = state.files.item.as_mut().and_then(|p| p.edit.as_mut()).expect("the edit form");
        form.title = "Dusk at the beach".into();
        form.description = "First line\nSecond line".into();
    }
    click_top(&ctx, &theme, &mut state, edit::SAVE);
    until(&ctx, &theme, &mut state, "the edit is answered", window_done);
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!((held.title.as_str(), held.description.as_str()), ("Dusk at the beach", "First line\nSecond line"), "the relay took the edit ({})", problem(&state));
    assert_eq!(page_item(&state).title, "Dusk at the beach", "and the page shows it");
    assert_eq!(state.files.item.as_ref().map(|p| p.status.as_str()), Some(SAVED));
    // Opening the page counted one view, on the relay's own counter.
    until(&ctx, &theme, &mut state, "the view is counted", |_| r.state.db.media_item(id).unwrap().unwrap().views == 1);

    // 2. A file for it: up through the relay's resumable routes, then added to the item.
    let zip = crate::test_temp::file("desk_r5_zip", "zip");
    std::fs::write(&zip, bytes(3000)).unwrap();
    let to = |role| AttachTo { id, filename: item.stored.clone(), role };
    uploading::start_attach(&mut state, &zip, "textures.zip", 3000, to(FileRole::File)).expect("it starts");
    until(&ctx, &theme, &mut state, "the file is up and added", |s| s.files.uploads.is_empty() && !s.files.jobs.iter().any(|(j, _)| matches!(j, Job::Publish { .. })));
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!(held.files.iter().map(|f| f.original_name.as_str()).collect::<Vec<_>>(), vec!["textures.zip"], "the relay holds it on the item ({})", state.files.status);
    let added = held.files[0].clone();
    assert!(r.dirs.uploads.join(&added.filename).exists(), "in the public folder, with the item");
    assert_eq!(page_item(&state).files.len(), 2, "the page lists both files");
    // Its Download goes through the relay's counting route, and is counted.
    assert_eq!(download_row(&ctx, &theme, &mut state, "textures.zip, ", added.file), bytes(3000), "the file, downloaded from its page");
    assert_eq!(r.state.db.media_item(id).unwrap().unwrap().downloads, 1, "counted once");

    // 3. A cover: a real picture, as the relay checks it.
    let png = crate::test_temp::file("desk_r5_cover", "png");
    let mut picture = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 120, 40, 255]))).write_to(&mut picture, image::ImageFormat::Png).unwrap();
    let picture = picture.into_inner();
    std::fs::write(&png, &picture).unwrap();
    uploading::start_attach(&mut state, &png, "cover.png", picture.len() as u64, to(FileRole::Cover)).expect("it starts");
    until(&ctx, &theme, &mut state, "the cover is up and added", |s| s.files.uploads.is_empty() && !s.files.jobs.iter().any(|(j, _)| matches!(j, Job::Publish { .. })));
    let cover = r.state.db.media_item(id).unwrap().unwrap().cover.unwrap_or_else(|| panic!("the relay holds the cover ({})", state.files.status));
    assert_eq!(cover.kind, "picture");
    assert!(page_item(&state).cover.is_some(), "the page has its cover");

    // 4. Members only, from the same window: every file moves out of the public folder.
    let now = page_item(&state);
    edit::open_edit(&mut state, &now);
    state.files.item.as_mut().and_then(|p| p.edit.as_mut()).expect("the edit form").members_only = true;
    click_top(&ctx, &theme, &mut state, edit::SAVE);
    until(&ctx, &theme, &mut state, "the visibility is answered", window_done);
    let held = r.state.db.media_item(id).unwrap().unwrap();
    assert_eq!(held.visibility, "members", "made members only ({})", problem(&state));
    for name in [held.upload_filename.as_str(), added.filename.as_str(), cover.filename.as_str()] {
        assert!(r.dirs.members.join(name).exists() && !r.dirs.uploads.join(name).exists(), "{name} moved with the item");
    }
    assert!(page_item(&state).members_only(), "the page says so");
    // A members-only file's Download: a ticket for the counting route, for that file, counted by
    // the member's key (a person apart from the address that downloaded before).
    let stored_cover = std::fs::read(r.dirs.members.join(&cover.filename)).unwrap();
    assert_eq!(download_row(&ctx, &theme, &mut state, ", cover picture", cover.file), stored_cover, "the cover, through its ticket");
    assert_eq!(r.state.db.media_item(id).unwrap().unwrap().downloads, 2, "counted for the member");

    // 5. The added file's Remove, confirmed on the page: off the item, and deleted.
    let file = page_item(&state).files.into_iter().find(|f| f.filename == added.filename).expect("the page lists the added file");
    state.files.item.as_mut().unwrap().confirm_file = Some(file);
    click_top(&ctx, &theme, &mut state, "Remove the file");
    until(&ctx, &theme, &mut state, "the remove is answered", |s| !s.files.jobs.iter().any(|(j, _)| matches!(j, Job::FileRemove { .. })));
    assert!(r.state.db.media_item(id).unwrap().unwrap().files.is_empty(), "off the item ({:?})", state.files.item.as_ref().map(|p| &p.status));
    assert!(!r.dirs.members.join(&added.filename).exists(), "and deleted");
    assert_eq!(state.files.item.as_ref().map(|p| p.status.as_str()), Some(FILE_REMOVED));
    assert_eq!(page_item(&state).files.len(), 1, "the page lists the item's own file alone");
}
