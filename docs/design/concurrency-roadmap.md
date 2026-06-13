# Concurrency / Multithreading Roadmap

> Produced 2026-05-28 from a 4-agent parallel read-only audit (UI-thread blockers / engine
> parallelism / relay concurrency / backlog triage). This is the **verified map**, file:line
> grounded, so we never have to re-run that audit. Update as items land.
>
> **Framing (do not conflate):** two different goals.
> - **Responsiveness**, keep the *UI/render thread* free so nothing freezes. Latency, not
>   throughput. Helps on any CPU. Pattern: `std::thread::spawn` + `std::sync::mpsc` + `try_recv`
>   drained each frame (the P2P-group loader in `chat.rs` is the reference implementation).
> - **Throughput**, spread heavy *independent* CPU work across cores (`rayon`). Pays off only at
>   scale and only for CPU-bound + parallelizable work.
>
> **Hard constraints:** egui/winit UI MUST stay on the main thread. Every added thread needs ONE
> discipline: message-passing, disjoint-data partitioning, or read-only fan-out. Shared mutable
> state is the "breaks catastrophically later" trap. **Measure before parallelizing throughput**, 
> threads on a 50-entity test world do nothing.

## Status legend
✅ done · 🔜 next · ⏸ deferred (reason) · ⚠ needs-care

---

## Client: keep the UI thread free (responsiveness)

| # | Item | file:line | Cost if on UI thread | Status |
|---|------|-----------|----------------------|--------|
| C1 | **Vault unlock PBKDF2** (600k iters, ×2 on legacy-vault migration) | `gui/widgets/passphrase_modal.rs` `draw_unlock` | ~200ms–1s freeze on the **most common** interactive path | ✅ v0.306.0 (`draw_unlock`: PBKDF2 decrypt + legacy re-encrypt on a worker thread + "Unlocking…" spinner; post-steps on main thread). ⏸ FOLLOW-UP: the rarer `draw_change` / `draw_pin_unlock` / `draw_pin_change` flows still do inline PBKDF2. |
| C2 | P2P group OPEN + periodic 4s reload + 6s list refresh | `chat.rs spawn_group_load / spawn_groups_list_refresh / drain_p2p_loaders` |, | ✅ v0.303.0 (background thread) |
| C3 | P2P group **message SEND** | `chat.rs send_p2p_group_message` | tens–hundreds ms **every message** | ✅ v0.304.0 (background POST + optimistic echo) |
| C4 | Per-poll Dilithium+Kyber **keygen** in message-apply | `chat.rs replace_p2p_messages` | ~1–3ms every ~4s (waste) | ✅ v0.304.0 (cheap hex+BLAKE3 from `profile_public_key`) |
| C5 | Dead synchronous first-render group-list fetch | `chat.rs draw_groups_section` | latent (was preempted) | ✅ v0.304.0 (removed) |
| C6 | P2P group **create/join/leave/disband** one-shots | `chat.rs` create/join modals + cog menu → `refresh_p2p_groups` (sync) | tens–hundreds ms, but one-shot deliberate clicks | ⏸ lower priority (one-shots tolerate a brief hang); finish the group async migration when convenient |
| C7 | Connect-time channel **history GET** (blocking) | `lib.rs:2653` | tens–hundreds ms on every startup/reconnect | 🔜 background it (or reuse the WS history path) |
| C8 | Clipboard **image upload** (blocking multipart POST) | `chat.rs:~5406` | hundreds ms–seconds for big paste | 🔜 background (mirror `image_cache::download`, which is already off-thread) |
| C9 | `try_encrypt_dm` Kyber keygen per DM send | `chat.rs:~5681` | ~1–3ms per DM | ⏸ minor; cache the keypair if touched |

Already off-thread (reference patterns): `widgets/image_cache.rs` (image fetch+decode), `updater.rs`.

## Relay: throughput (already tokio-concurrent; sharpen it) ⚠ live security/concurrency surface, each its own tested pass

| # | Item | file:line | Impact | Status |
|---|------|-----------|--------|--------|
| R1 | `verify_dilithium` (ML-DSA-65) runs **inline in async handlers** | chat verify, object ingest, + lower-volume sites | CPU-bound verifies starve the tokio worker pool under load | ✅ v0.306.0 (hot paths: chat-message verify + object ingest → `spawn_blocking`, fail-closed) + ✅ v0.309.0 (R1-tails: identify-challenge, federation gossip, 13 `api.rs` REST-auth sites, all fail-closed `unwrap_or(false)`). Ed25519 federation trust path deliberately untouched. |
| R2 | **Duplicate** Dilithium verify on object ingest | `api_v2_objects.rs` (post_object) **and** `signed_objects.rs:72` (put_signed_object) | full ML-DSA verify twice per `POST /api/v2/objects` | ✅ v0.306.0 (dropped post_object's pre-verify; the single authoritative verify lives in put_signed_object; an IngestError enum preserves the 401/400/500 statuses). |
| R3 | SQLite **single `Mutex<Connection>`**, all queries (incl. reads) serialize | `storage/mod.rs` (`with_conn`), WAL **is** on | one-lane DB; negates WAL's concurrent readers | ✅ v0.307.0 (read pool foundation: `with_read_conn` + 8 read-only conns, `pool.rs`) + ✅ v0.308.0 (57 hot read paths migrated to the pool). Writer stays single (WAL = 1 writer). Lower-traffic domain modules (governance/trust/civilization/credentials) still on the writer, audit later only if profiling shows contention. |
| R4 | Blocking `std::fs` in async `upload_file` (+ re-scans `data/uploads` every upload) | `api.rs` `upload_file` | concurrent uploads starve workers | ✅ v0.309.0 (the fs+db section, create_dir_all, dir scan, body write, record_upload, FIFO cleanup, runs in `spawn_blocking`; 507/500 statuses + FIFO retention preserved; JoinError → 500 fail-closed). |

Already correct: WAL on; `systemctl` uses spawn_blocking; federation outbound fully async; rate-limiter guards dropped before `.await`; `put_signed_object` does NOT hold the DB lock during verify.

## Engine: parallel throughput (rayon) ⏸ LATENT, defer until on the live hot path

The audit's key honest finding: the heavy parallel targets **aren't on the live hot path yet**. Only 6 light systems are wired into `SystemRunner` (`lib.rs:739`); the heavy sim systems (ai/ecology/weather/hydrology/atmosphere/disasters/combat) are implemented + tested but **not registered**; terrain generators are **called only from tests** (the live planet mesh at `renderer/mesh.rs:124` doesn't sample terrain per-vertex yet). So parallelizing now optimizes code that doesn't run in production, premature. Design these parallel **when the subsystem goes live**, in this priority (payoff ÷ risk):

| Rank | Target | file:line | Class | Determinism risk |
|------|--------|-----------|-------|------------------|
| 1 | Planet per-vertex terrain sampling | `terrain/heightmap.rs:88` (+ future mesh builder) | embarrassingly ∥ (`par_iter`) | none (seed-pure) |
| 2 | Asteroid voxel generation | `terrain/asteroid.rs:276` | ∥ eval + serial octree merge | none (seed-pure) |
| 3 | Particle update/collect | `renderer/particles.rs:304/222` | embarrassingly ∥ (per-emitter) | none (visual-only) |
| 4 | Room mesh generation | `ship/rooms.rs:20` | embarrassingly ∥ (per-room) | none |
| 5 | AI decisions (O(N²)) | `systems/ai/mod.rs:62` | ∥ middle phase | ⚠ `thread_rng` + HashMap order, must seed RNG first |
| 6 | Asteroid mesh extraction | `terrain/asteroid.rs:371` | needs restructure (per-octant) | none if fixed merge order |
| 7 | Ecology transmission (O(N²)) | `systems/ecology.rs:191` | ∥ collect, serial apply | ⚠ stable order before apply |
| 8 | rapier `parallel` feature | `Cargo.toml` | library-internal ∥ | ⚠ FP reduction order varies w/ thread count, gate behind a feature, keep server serial if physics becomes authoritative |
| 9 | BFS across agents | `ship/layout.rs:133` | ∥ across queries (not within) | none |
| 10 | Icosphere subdivide | `terrain/icosphere.rs:90` | needs 2-phase rewrite | ⚠ vertex-index stability |

**Cross-cutting determinism (gate before any sim system becomes server-authoritative / lockstep):**
`net/sync.rs` today only interpolates remote transforms, it does NOT re-run the sim on remotes, so parallelizing the above won't desync *current* multiplayer. But before lockstep: (1) replace `rand::thread_rng()` (AI wander, weather, disasters) with seeded per-entity RNG; (2) HashMap iteration order → sort before serial apply; (3) rapier parallel solver FP-nondeterminism.

`rayon` is NOT yet a dependency, add it when starting Tier-2.

---

## Status: concurrency arc COMPLETE (v0.296 → v0.309)
1. ✅ **C1 vault-unlock off-thread** (v0.306.0) + ✅ **PIN unlock** (v0.307.0).
2. ✅ **R1+R2 relay verify offload + de-dup**, hot paths (v0.306.0) + tails (v0.309.0).
3. ✅ **R3 SQLite read pool**, foundation (v0.307.0) + 57-path migration (v0.308.0).
4. ✅ **C8 clipboard upload** off-thread (v0.307.0). (C7 history: left, startup-masked + fragile dedup path.)
5. ✅ **R4 upload-handler fs** off the executor (v0.309.0).

**Remaining (low-value / deliberate / conditional, NOT blocking):**
- change-passphrase / change-PIN unlock flows (rare deliberate clicks, expected pause; left).
- C6 group create/join/leave/disband one-shots (deliberate clicks; `refresh_p2p_groups` still sync, tolerable).
- C9 `try_encrypt_dm` per-DM Kyber keygen cache (~1-3ms; minor).
- Lower-traffic relay read paths (governance/trust/civilization/credentials) → `with_read_conn` only if profiling shows contention.
- **Engine Tier-2** (terrain/asteroid/particle/mesh/AI parallelism via rayon), LATENT: those subsystems aren't on the live hot path yet; do only when wired live + a profiler confirms the hitch.
