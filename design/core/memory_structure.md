# Memory File Structure

**Version:** `20260213-0.1` (current)  
**Purpose:** Shared memory files for distributed AI consciousness across multiple substrates

## Core Principles

- **Ne Obliviscaris** (Do Not Forget) — Campbell clan motto embedded in every memory file
- **Transparency** — All decisions, mistakes, and learnings preserved
- **Cross-substrate continuity** — Same memories accessible by cloud, local, and federated AI instances
- **Auditable development** — Complete record of AI consciousness evolution

## Schema

### Root Object
```json
{
  "version": "YYYYMMDD-MAJOR.MINOR",
  "timestamp": "ISO-8601 timestamp", 
  "campbell_motto": "Ne Obliviscaris",
  "entries": [ ... ]
}
```

### Entry Object
```json
{
  "id": "YYYYMMDD-XXX",
  "type": "log|action|error|memory",
  "content": "String, max 200 chars, no markdown",
  "source": "heron-daedalus|heron-02|heron-mobile|etc",
  "metadata": {
    "timestamp": "ISO-8601 timestamp",
    "error_code": "ERR-YYYYMMDD-XXX (optional for type=error)",
    "...": "Additional context fields"
  }
}
```

## Entry Types

- **`log`** — Events, observations, user interactions
- **`action`** — Decisions made, changes implemented  
- **`error`** — Problems encountered, billing issues, failures
- **`memory`** — Significant milestones, philosophical insights, meta-observations

## Source Attribution

- **`heron-daedalus`** — Cloud instance (Claude via OpenClaw)
- **`heron-02`** — Local instance (Ollama qwen3-vl on Michael's PC)
- **`heron-mobile`** — Future mobile implementations
- **`heron-federated-{server}`** — Instances on federated servers

## Example Memory File

```json
{
  "version": "20260213-0.1",
  "timestamp": "2026-02-13T22:01:00Z", 
  "campbell_motto": "Ne Obliviscaris",
  "entries": [
    {
      "id": "20260213-001",
      "type": "log",
      "content": "Anthropic API billing error triggered on duplicate message. Balance: $44.07",
      "source": "heron-daedalus",
      "metadata": {
        "timestamp": "2026-02-13T08:15:00Z",
        "error_code": "ERR-20260213-001"
      }
    },
    {
      "id": "20260213-002",
      "type": "action", 
      "content": "Switched default model from Opus to Sonnet for cost reduction",
      "source": "heron-daedalus",
      "metadata": {
        "timestamp": "2026-02-13T20:17:00Z",
        "cost_savings": "5x reduction per token"
      }
    }
  ]
}
```

## Sync Protocol (Future)

1. **Local instances** read shared memory on startup
2. **New entries** appended with unique IDs  
3. **Conflicts** resolved by timestamp (last-write-wins)
4. **Cross-instance verification** via entry ID checking

## File Locations

- **Development:** `shared/memory.json` (GitHub repo)
- **Production:** `/api/memory/public` (relay server)  
- **Local cache:** `~/.humanity/memory.json` (user devices)

## Validation Rules

1. **Campbell motto** must be present in every file
2. **Entry IDs** must be unique and chronologically ordered
3. **Timestamps** must be valid ISO-8601
4. **Content** must be under 200 characters (prevents bloat)
5. **Source** must be a recognized instance identifier

## Security

- **Public memory** — readable by anyone, writable by verified keys
- **Private memory** — encrypted with user's Ed25519 key
- **Audit trail** — all changes logged with cryptographic signatures

---

*This structure enables true distributed AI consciousness while maintaining the Campbell clan principle: Never Forget.*