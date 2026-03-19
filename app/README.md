# HumanityOS Desktop App

Native desktop wrapper built with [Tauri v2](https://tauri.app/). Uses a local-first architecture: web assets are bundled into the binary so the app works offline, then syncs with the server when connected.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Platform-specific dependencies:
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
  - **macOS:** Xcode Command Line Tools
  - **Windows:** Visual Studio Build Tools with C++ workload

## Building

From the repo root:

```bash
just bundle-web          # copies web assets into app/web/
just build-desktop       # runs bundle-web then compiles Tauri (release)
```

Or manually:

```bash
node scripts/bundle-web.js
cd app
npm install
npx tauri build
```

The built binary is in `target/release/bundle/`.

## Development

```bash
cd app
npm install
npx tauri dev
```

## How It Works

### Local-first architecture

The desktop app ships with all web assets (HTML, JS, CSS, images) bundled directly into the binary. This means:

- **Offline-capable** -- the app loads instantly from local files, no network required.
- **Deterministic** -- the exact version of the UI is pinned to the release, no CDN surprises.
- **Syncable** -- when online, the app connects to `united-humanity.us` via WebSocket to sync data (messages, tasks, vault, etc.).

### Build pipeline

1. `scripts/bundle-web.js` copies all web assets (chat client, shared files, pages, etc.) into `app/web/`.
2. Tauri's `frontendDist` in `tauri.conf.json` points to `../web`, so the bundled assets become the app's UI.
3. The `web/` directory is gitignored -- it's regenerated on every build.

### Web sync

The app connects to the relay server (`united-humanity.us/ws`) for real-time data. If the server is unreachable, the app continues working with cached local data. When connectivity returns, it reconnects and syncs automatically.

### Version management

`just bump` (or `node scripts/bump-version.js`) updates version strings across all locations, including the local manifest in `web/manifest.json` if it exists.

## Full Release

```bash
just release             # patch bump + bundle + build + ship
just release minor       # minor bump + bundle + build + ship
```

## Icons

To regenerate icons from the source SVG (`ui/chat/favicon.svg`):

```bash
cd app
npx tauri icon ../ui/chat/favicon.svg
```

## License

CC0 -- Public Domain
