# Humanity Desktop App

A native desktop wrapper for [united-humanity.us](https://united-humanity.us) built with [Tauri v2](https://tauri.app/).

The app loads the web client directly — updates to the web platform are instant, no app update needed.

## Download

Pre-built binaries for Windows, macOS, and Linux are available on the [Releases page](https://github.com/Shaostoul/Humanity/releases/latest).

## Building from Source

### Prerequisites
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Platform-specific dependencies:
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
  - **macOS:** Xcode Command Line Tools
  - **Windows:** Visual Studio Build Tools with C++ workload

### Build

```bash
cd desktop
npm install
npx tauri build
```

The built binary will be in `src-tauri/target/release/bundle/`.

## Development

```bash
cd desktop
npm install
npx tauri dev
```

## Icons

To regenerate icons from the source SVG (`crates/humanity-relay/client/favicon.svg`):

```bash
cd desktop
npx tauri icon ../crates/humanity-relay/client/favicon.svg
```

This requires the SVG to be converted to a 1024x1024 PNG first. You can use:

```bash
# Install rsvg-convert if needed: sudo apt install librsvg2-bin
rsvg-convert -w 1024 -h 1024 ../crates/humanity-relay/client/favicon.svg > /tmp/icon.png
npx tauri icon /tmp/icon.png
```

## License

CC0 — Public Domain
