# CONTINUE.md – united-humanity.us / Humanity Project

## Project Overview
united-humanity.us (also called Humanity / Project Universe) is a fully open-source, public-domain, privacy-first ecosystem:
- Federated, account-less, E2E-encrypted chat & networking platform (desktop + PWA)
- Project Universe survival/homesteading game with custom Rust engine (no Bevy)
- No central servers, no user accounts — everything uses Ed25519 cryptographic identities

Core stack:
- Rust (Cargo workspace, nightly where needed)
- humanity-core crate: identity, signing, hashing, CBOR encoding
- humanity-relay crate: p2p relay server, realtime protocol, storage
- Tauri desktop app (Rust backend + web frontend)
- Website: Jekyll-style static site (lots of Markdown)
- Data: RON serialized entities/plots/resources
- Design docs: 200+ Markdown files covering Accord, game systems, SurrealDB schemas, threat models, etc.

## Exact Project Structure (2026)
    united-humanity.us/
    ├── Cargo.toml                  # Workspace root
    ├── crates/
    │   ├── humanity-core/          # Core crypto & identity (identity.rs, signing.rs, object.rs, etc.)
    │   └── humanity-relay/         # Relay server + client web (api.rs, relay.rs, storage.rs + client/ HTML/JS)
    ├── desktop/                    # Tauri app (src-tauri/)
    ├── website/                    # Static site with _layouts, accord/, design/, etc.
    ├── design/                     # 200+ .md files (architecture, database_concepts/*.surql, game, security…)
    ├── data/                       # RON files (entities, plots, resources)
    ├── assets/                     # Game art, UI icons, etc.
    ├── .continue/rules/CONTINUE.md # This file (auto-loaded)
    └── Humanity.code-workspace     # Your VS Code workspace

## Getting Started
    git clone <your-repo>
    cd united-humanity.us

    rustup default nightly
    cargo build --release

    # Run relay server
    cargo run -p humanity-relay --bin humanity-relay

    # Run desktop app
    cd desktop && cargo tauri dev

    # Build website (Jekyll-style)
    cd website && jekyll serve

## Key Rust Conventions in This Project
- Zero-copy where possible (bytemuck, Cow, arenas)
- Ed25519 identities everywhere (no passwords)
- Constant-time crypto
- Heavy use of RON for data, CBOR for network objects
- SurrealDB-inspired schemas in design/database_concepts/
- humanity-relay handles realtime transport, federation, storage

## Common Tasks
- Improve identity handling → edit crates/humanity-core/src/identity.rs
- Fix relay protocol → crates/humanity-relay/src/relay.rs
- Add new game entity → edit RON files in data/entities/ and update design schemas
- Update desktop UI → desktop/src-tauri/src/main.rs or web files
- Refactor borrow checker issue → look in hot paths in relay or core

## Development Workflow
- cargo check -p humanity-core -p humanity-relay
- cargo clippy --all-targets -- -D warnings
- cargo test --all
- Always run cargo fmt
- For large refactors: use Continue with Agent mode