# HumanityOS — Project History

A timeline of the project from its origins as Project Universe to its current form as HumanityOS.

---

## Timeline

### January 2019 — Project Universe begins

The project launches as **Project Universe**, an open-source cooperative platform aiming to unite humanity and end poverty through technology and gaming. The core vision from day one: build a platform that replicates everything in real life AND allows fantasy/imagination experiences, all for maximally enjoyable educational purposes.

### January 2019 – January 2026 — Seven years as Project Universe

Throughout this period the project develops under the Project Universe name. The foundational ideas take shape:

- **Real-world parity.** Every real-world skill, system, and process should have a faithful in-game counterpart — farming, welding, electronics, construction, medicine, cooking, navigation, and more.
- **Fantasy as a teaching tool.** Realistic simulation alone is not enough. Fantasy and sci-fi scenarios (alien invasions, space expeditions, cooperative base-building) provide the excitement that makes players *want* to learn.
- **Cooperative by design.** The platform is built around cooperation, not competition. Players help each other, share knowledge, and build together.
- **Open-source everything.** Code, design documents, governance — all open and transparent.

### January 2026 — Renamed to HumanityOS

The project is renamed from Project Universe to **HumanityOS** (also referred to simply as "Humanity"). The vision remains the same; the new name better reflects the project's scope as a full operating layer for human cooperation, not just a game.

- **Repository:** [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)
- **Live instance:** [united-humanity.us](https://united-humanity.us)

### March 2026 — Current state

The platform consists of:

- A **Rust relay server** (axum/tokio, SQLite) handling real-time communication, task management, and data sync.
- A **web client** (plain HTML/JS, no build step) for chat, tasks, notes, profiles, and more.
- A **Tauri desktop app** wrapping the web client with native capabilities.
- A **custom game engine** built on wgpu (Rust), designed to coexist with the Tauri webview overlay.
- Extensive **design documentation** covering game systems, educational modules, governance (the Humanity Accord), and technical architecture.
- A **gardening minigame** as the first playable game module, grounded in real botanical data.

---

## Related documents

- [01-VISION](01-VISION.md) — Mission statement and product shape
- [Product vision](product/vision.md) — Detailed product vision
- [Project Universe integration](product/project_universe_integration.md) — Integration notes from the original Project Universe
- [Educational gameplay](design/educational-gameplay.md) — The educational game design philosophy
- [Game engine decision](design/game-engine.md) — Why a custom engine on wgpu
- [Roadmap](roadmap.md) — Current feature priority list
