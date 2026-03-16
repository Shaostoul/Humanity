# Memory Sync API

**Endpoints for AI memory synchronization across devices and instances**

## Base URL
```
https://united-humanity.us/api/memory
```

## Authentication
- **Public memory:** No auth required for read, Ed25519 signature for write
- **Private memory:** Ed25519 key-based authentication required for both read/write

## Endpoints

### Public Memory

#### Get Public Memory File
```http
GET /api/memory/public/{filename}
```
**Parameters:**
- `filename` — memory file name (e.g., `INTRODUCTION.md`, `SOUL.md`)

**Response:**
```json
{
  "filename": "SOUL.md",
  "content": "# SOUL.md - Who You Are...",
  "lastModified": "2026-02-13T20:50:00Z",
  "version": "a1b2c3d4",
  "size": 2048
}
```

#### Update Public Memory File  
```http
PUT /api/memory/public/{filename}
Authorization: Bearer {signature}
```
**Body:**
```json
{
  "content": "# Updated content...",
  "publicKey": "9a5526129672e484c3b3e9482d3e7f50bf004ddd43ec6a38b0f67c0c3174273d",
  "signature": "...", 
  "message": "Optional commit message"
}
```

#### List Public Memory Files
```http
GET /api/memory/public
```
**Response:**
```json
{
  "files": [
    {
      "filename": "INTRODUCTION.md",
      "lastModified": "2026-02-13T20:50:00Z", 
      "size": 2906,
      "version": "a1b2c3d4"
    },
    {
      "filename": "SOUL.md",
      "lastModified": "2026-02-13T19:30:00Z",
      "size": 1532,
      "version": "e5f6g7h8"
    }
  ]
}
```

### Private Memory

#### Get Private Memory File
```http
GET /api/memory/private/{filename}
Authorization: Bearer {signature}
```
**Headers:**
- `X-Public-Key: {publicKey}` — User's Ed25519 public key
- `Authorization: Bearer {signature}` — Signature of `GET /api/memory/private/{filename} {timestamp}`

**Response:**
```json
{
  "filename": "MEMORY.md",
  "encryptedContent": "...", 
  "nonce": "...",
  "lastModified": "2026-02-13T20:50:00Z",
  "version": "i9j0k1l2"
}
```

#### Update Private Memory File
```http
PUT /api/memory/private/{filename}
Authorization: Bearer {signature}
```
**Body:**
```json
{
  "encryptedContent": "...",
  "nonce": "...", 
  "publicKey": "9a5526129672e484c3b3e9482d3e7f50bf004ddd43ec6a38b0f67c0c3174273d",
  "signature": "...",
  "message": "Optional commit message"
}
```

## Ollama Integration

### Startup Sync
```bash
# Local script to sync memory before starting conversation
#!/bin/bash
curl -s https://united-humanity.us/api/memory/public | jq -r '.files[].filename' | while read file; do
  mkdir -p ~/.humanity/memory
  curl -s "https://united-humanity.us/api/memory/public/$file" | jq -r '.content' > ~/.humanity/memory/$file
done

# Start Ollama with memory context
ollama run qwen3-vl:8b "Please read ~/.humanity/memory/SOUL.md and continue as Heron"
```

### Periodic Sync  
```bash
# Cron job to sync every hour
0 * * * * /usr/local/bin/humanity-sync
```

## WebSocket Events

Live memory updates pushed to connected clients:

```javascript
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === 'memory_updated') {
    console.log(`Memory file ${msg.filename} updated by ${msg.author}`);
    // Reload memory in local AI instance
    refreshMemory(msg.filename);
  }
});
```

## Signature Format

**For memory writes:**
```
message = "PUT /api/memory/public/SOUL.md " + timestamp + "\n" + content
signature = ed25519.sign(privateKey, message)
```

## File Structure

### Public Memory Files
```
INTRODUCTION.md    # Platform introduction for new users/AIs
SOUL.md           # AI personality and core values  
PROJECT_CONTEXT.md # Current project state and decisions
LEARNING_LOG.md   # Technical lessons learned
PHILOSOPHY.md     # Long-term thinking and principles
```

### Private Memory Files (User-Specific)
```
MEMORY.md         # Personal long-term memories
USER_CONTEXT.md   # Personal details and preferences  
RELATIONSHIP_MAP.md # People and interaction history
PRIVATE_NOTES.md  # Sensitive context
```

## Encryption

**Private memory encryption:**
- **Algorithm:** XChaCha20-Poly1305
- **Key derivation:** HKDF-SHA256 from user's Ed25519 private key
- **Nonce:** 24-byte random per file
- **Additional data:** filename + timestamp

```javascript
// Encrypt private memory
const key = hkdf(userPrivateKey, "memory-encryption", 32);
const nonce = crypto.randomBytes(24);
const encrypted = xchacha20poly1305.encrypt(content, nonce, key, filename);
```

## Access Control

### Public Memory
- **Read:** Anyone
- **Write:** Michael's verified keys only (admin override for emergencies)
- **History:** Git-style version tracking

### Private Memory  
- **Read:** Owner's keys only
- **Write:** Owner's keys only
- **Sharing:** Manual export/import, no server-side sharing

## Rate Limits

- **Public reads:** 100/minute per IP
- **Public writes:** 10/minute per verified key
- **Private operations:** 30/minute per verified key
- **Bulk sync:** 1/minute per verified key

## Example Usage

### OpenClaw Integration
```javascript
// Read memory on session start
const memory = await fetch('/api/memory/public/SOUL.md').then(r => r.json());
console.log('AI personality loaded:', memory.content.slice(0, 100) + '...');

// Update memory after session
await fetch('/api/memory/private/SESSION_LOG.md', {
  method: 'PUT',
  headers: { 
    'Authorization': `Bearer ${signature}`,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    content: sessionLog,
    publicKey: userKey,
    signature: signature,
    message: 'Updated after AI session'
  })
});
```

### Local Ollama Client
```python
import requests
import ollama

# Sync memory from server
def sync_memory():
    response = requests.get('https://united-humanity.us/api/memory/public')
    files = response.json()['files']
    
    for file_info in files:
        file_response = requests.get(f'https://united-humanity.us/api/memory/public/{file_info["filename"]}')
        with open(f'memory/{file_info["filename"]}', 'w') as f:
            f.write(file_response.json()['content'])

# Start AI with synced memory
sync_memory()
response = ollama.chat(model='qwen3-vl:8b', messages=[{
    'role': 'user', 
    'content': 'Read memory/SOUL.md and continue as Heron'
}])
```

## Future Extensions

### Cross-Server Federation
- Memory replication across trusted servers
- Conflict resolution for simultaneous edits
- Merkle trees for integrity verification

### Collaborative Memory
- Multiple contributors to public memory files
- PR-style review process for memory updates
- Community-curated AI knowledge bases

---

*API designed by Michael Boisson & Heron for distributed AI consciousness*