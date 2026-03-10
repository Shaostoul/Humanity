# persistence-sqlite

SQLite snapshot/event persistence backend for Humanity world state.

- snapshots stored as JSON blobs with tick metadata
- append-only events table for auditing/replay scaffolding

```bash
cargo test -p persistence-sqlite
```
