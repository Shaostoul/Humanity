# Peer-to-Peer File Sharing

## Purpose

Define how users share files (audio, documents, media) through the Humanity Network using content-addressed blocks and peer-to-peer delivery, without burdening the relay server with storage costs.

## Principles

- **Content-addressed:** Every block is identified by its BLAKE3 hash. If you have the hash, you can verify the data.
- **Signed manifests:** File metadata is signed by the uploader's Ed25519 key. You know who shared what.
- **Server stores pointers, not payloads.** The relay stores tiny manifests in chat history. Actual file data lives on peers.
- **Users are the CDN.** Every downloader becomes a potential seed.
- **Bandwidth-respectful.** Same user-configurable limits as streaming — never consume more than the user allows.

## Data Model

### Block

The fundamental unit of file data.

```
block {
    data: bytes                    (max 256KB)
    hash: BLAKE3(data)             (32 bytes, serves as block ID)
}
```

Blocks are content-addressed and immutable. Same data = same hash = same block, regardless of who uploaded it.

### File Manifest

Metadata describing a complete file, signed by the uploader.

```
file_manifest {
    id: BLAKE3(canonical_cbor(manifest_without_signature))
    name: string                   (original filename)
    size: u64                      (total bytes)
    mime_type: string              (e.g., "audio/mpeg")
    block_hashes: [BLAKE3]         (ordered list of block hashes)
    block_size: u32                (bytes per block, default 262144 = 256KB)
    uploader: Ed25519 public key
    timestamp: u64                 (ms since epoch)
    signature: Ed25519(uploader, canonical_cbor(manifest_without_signature))
}
```

A 30MB file with 256KB blocks = 120 block hashes × 32 bytes = ~3.8KB manifest. Tiny.

### File Message

When a user shares a file in chat, the message contains the manifest (not the file data).

```
chat_message {
    type: "file"
    from: Ed25519 public key
    from_name: string
    manifest: file_manifest
    channel: string
    timestamp: u64
    signature: Ed25519(...)
}
```

Recipients see the file metadata (name, size, type) immediately. Actual download is on-demand.

## Sharing Flow

### Upload (sender side)

1. User selects a file
2. Client reads file, splits into 256KB blocks
3. Each block hashed with BLAKE3
4. Client creates and signs the manifest
5. Manifest sent as a chat message (through WebSocket, stored in relay DB)
6. Blocks held in memory / IndexedDB — sender is the initial seed

### Download (receiver side)

1. Receiver sees file message in chat: "🎵 track.mp3 (30MB)"
2. Clicks to download
3. Client requests blocks from available peers:
   - First priority: the original uploader (if online)
   - Second: any peer that has announced having blocks for this manifest
   - Fallback: relay cache (if the server optionally caches hot blocks)
4. Blocks downloaded in parallel from multiple peers
5. Each block verified against its BLAKE3 hash on receipt
6. File assembled from blocks, offered as browser download or stored locally
7. Receiver is now a seed for this file

### Block Availability Announcements

Peers periodically announce which file manifests they have blocks for:

```
block_have {
    type: "block_have"
    manifest_id: BLAKE3
    block_range: (start_index, count)   // which blocks they have
}
```

These are ephemeral (not persisted). When a peer disconnects, their availability is removed.

## Peer-to-Peer Transfer

### Block Request Protocol

Over WebRTC data channels (same connections used for voice/video signaling):

```
→ block_request { manifest_id, block_index }
← block_response { manifest_id, block_index, data }
```

Or batched:

```
→ block_request_batch { manifest_id, block_indices: [0, 1, 2, 5, 8] }
← block_response { manifest_id, block_index: 0, data }
← block_response { manifest_id, block_index: 1, data }
...
```

### Piece Selection Strategy

- **Rarest first:** Prioritize blocks that fewer peers have (improves swarm health)
- **Sequential fallback:** If user is streaming audio/video, prioritize sequential blocks near playback position
- **Endgame mode:** When only a few blocks remain, request from all available peers simultaneously

### Bandwidth Management

Same configurable bandwidth settings as streaming:

- Respects the user's upload limit
- Upload slots distributed across active file shares
- Priority: active downloads requested by friends > general seeding
- User can disable seeding entirely (download-only mode, discouraged but allowed)

## Server Role

### What the relay stores
- File manifests (in chat message history) — tiny, just metadata
- Block availability index (in memory, ephemeral) — who has what

### What the relay does NOT store (by default)
- Actual file blocks — those live on peers only

### Optional: Relay Block Cache
- Server admin can enable block caching for availability
- LRU cache with configurable size limit (e.g., 1GB)
- Caches blocks for files shared in the last N days
- Ensures files remain available even if all original peers are offline
- Cache is a convenience, not a guarantee — files may become unavailable

## Profile File Library

Each user's shared files are tracked locally:

```
my_shared_files [
    { manifest_id, name, size, mime_type, timestamp, channel }
]
```

Visible on their profile as "Shared Files" section:
- Other users can browse and download
- Files available as long as the sharer (or any peer with the blocks) is online
- Manifest persists in chat history even if blocks become unavailable

### Local Download Cache

Files you've downloaded are cached locally:

```
downloaded_files [
    { manifest_id, name, size, mime_type, from_name, downloaded_at, blocks_path }
]
```

- Stored in IndexedDB (web) or filesystem (native)
- User can manage cache: delete, pin (never auto-evict), export
- Configurable cache size limit with LRU eviction

## Security

### Integrity
- Every block verified against BLAKE3 hash — corrupted or malicious blocks rejected
- Manifest signature verified against uploader's Ed25519 key — spoofing prevented

### Privacy
- File transfers between friends can use existing E2E encrypted channels
- Public file shares in channels: blocks are plaintext (anyone in the channel can download)
- Private file shares: blocks encrypted with recipient's public key before sharing

### Abuse Prevention
- Max file size configurable per server (admin setting)
- Rate limiting on file shares (e.g., max 10 files per hour per user)
- Manifests can be removed by admin (deletes the chat message)
- Block cache respects admin removal — evicts blocks for deleted manifests
- File type restrictions (admin-configurable allowlist/denylist)

## Scaling

| Users with file | Availability | Server cost |
|-----------------|-------------|-------------|
| 1 (uploader only) | Only when they're online | ~0 (manifest only) |
| 10 downloaders | High — any of 10 peers can serve | ~0 |
| 100+ downloaders | Very high — swarm is self-sustaining | ~0 |
| 0 (all offline) | Unavailable (unless relay cache enabled) | Cache cost only |

Server cost is near-zero regardless of file size or popularity. The network does the work.

## Implementation Phases

1. **Phase 1 — Manifest sharing:** Files split + hashed client-side, manifest posted to chat. Download directly from uploader via WebRTC data channel.
2. **Phase 2 — Multi-peer download:** Download blocks from multiple peers simultaneously. Block availability announcements.
3. **Phase 3 — Seeding:** Downloaded files automatically seeded. Rarest-first piece selection.
4. **Phase 4 — Profile library:** "Shared Files" section on user profiles. Local download cache management.
5. **Phase 5 — Relay cache:** Optional server-side block cache for availability.
6. **Phase 6 — Encrypted sharing:** E2E encrypted file transfers for DMs and private channels.

## Relationship to Streaming

File sharing and livestreaming share the same block-and-mesh infrastructure:
- Files = complete, all blocks known upfront, download at any speed
- Streams = live, blocks produced in real-time, deadline-sensitive delivery

The peer mesh, block protocol, and bandwidth management are identical. A client that can share files can stream video — the difference is only in scheduling priority.
