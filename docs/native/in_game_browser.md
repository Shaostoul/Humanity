# In-Game Browser (Web-to-Texture)

## Purpose

Render live websites onto 3D surfaces inside the game world. Primary use case: interactive kiosks (e.g., an Amazon affiliate kiosk in a VR marketplace where players can browse and shop on the real Amazon website without leaving the game).

## Why Not iframes / OS Webviews

- iframes are HTML-only — the game engine renders via wgpu, not a browser
- OS webviews (like Tauri's WebView2) are windowed UI components — they can't render onto a 3D mesh
- Most websites block iframe embedding via X-Frame-Options / CSP
- In VR, there is no "system browser" to fall back to

## Architecture

```
KioskEntity (ECS)
  ├── Transform (position, rotation, scale in world)
  ├── Mesh (screen quad — flat panel or curved surface)
  ├── BrowserComponent
  │     ├── url: String
  │     ├── size: (u32, u32)          — pixel dimensions of offscreen buffer
  │     ├── browser: CEF instance     — headless Chromium
  │     └── input_focus: bool         — whether player is interacting
  └── Material
        └── texture: GPU texture      — updated each frame from CEF buffer
```

### Render Pipeline

1. CEF renders the webpage to an offscreen pixel buffer (shared memory or PBO)
2. Each frame, if the buffer is dirty, upload pixels to a GPU texture
3. The texture is bound to the kiosk's screen material
4. The engine's standard mesh renderer draws it like any other textured surface

### Input Pipeline

1. Player looks at / points at the kiosk (raycast from camera or VR controller)
2. Ray-mesh intersection gives a UV coordinate on the screen quad
3. UV coordinate maps to pixel coordinates in the browser viewport
4. Mouse move / click / scroll / keyboard events are forwarded to CEF
5. CEF processes the input and re-renders — the texture updates

### VR Considerations

- VR controllers emit a ray; intersection with the kiosk quad gives the "cursor" position
- Virtual keyboard overlay for text input (or pass-through to physical keyboard)
- Kiosk screens should be sized for comfortable reading at arm's length (~1m)
- Consider a "zoom in" mode that brings the browser panel closer to the player
- Stereo rendering: the browser texture is 2D, applied to both eye views via the mesh

## Technology Options

### CEF (Chromium Embedded Framework) — Recommended

- Industry standard: used by Unreal Engine, Unity (via plugins), Steam Overlay, Spotify, Discord
- Full Chromium browser — renders any website, runs JavaScript, handles cookies/auth
- Offscreen rendering mode designed exactly for this use case
- Rust bindings: `cef-rs` crate (or raw FFI to the C API)
- ~200MB runtime footprint (Chromium is heavy)

### Servo (Mozilla's Rust browser engine)

- Written in Rust — ideal language fit
- Much lighter than Chromium
- Designed for embedding from the start
- Less mature — may not render complex sites (Amazon, YouTube) correctly
- Worth evaluating once Servo stabilizes

### Ultralight

- Lightweight HTML renderer (~30MB vs CEF's ~200MB)
- Designed for game UI rendering
- May struggle with complex real-world websites
- Commercial license required for some uses

### WebView2 offscreen (Windows only)

- Microsoft's Chromium-based engine
- Has a "visual hosting" mode for offscreen rendering
- Windows-only — not viable for cross-platform

## Data-Driven Kiosk Definitions

Kiosks should be defined in data files, not hardcoded:

```toml
# data/kiosks/amazon.toml
[kiosk]
id = "amazon-store"
name = "Amazon"
url = "https://www.amazon.com/?tag=humanity-affiliate-20"
icon = "assets/icons/kiosks/amazon.png"
size = [1920, 1080]
allow_navigation = true       # can the player click links
allowed_domains = ["amazon.com", "*.amazon.com"]  # restrict navigation
category = "shopping"

[kiosk.placement]
# Default placement rules for procedural world generation
location = "marketplace"
min_distance_between = 50.0   # meters — don't cluster duplicates
```

## Affiliate / Revenue Integration

- Kiosk URLs include affiliate tags (e.g., `?tag=humanity-affiliate-20`)
- Revenue from affiliate links funds the cooperative
- Players see a small "Affiliate link — supports Humanity" badge on the kiosk frame
- Transparency: clicking the badge shows exactly what the affiliate tag does

## Security Considerations

- **Domain allowlist**: Each kiosk restricts navigation to specific domains
- **No access to game state**: The browser runs in a sandboxed process (CEF's multi-process architecture)
- **No localStorage sharing**: Each kiosk gets its own browser profile
- **Cookie isolation**: Kiosks don't share cookies with each other
- **Content filtering**: Block popups, redirects to non-allowlisted domains
- **Rate limiting**: Limit how many kiosks can be actively rendering (GPU/memory budget)

## Performance Budget

- Each active browser instance: ~100-200MB RAM, ~2-5% CPU
- Texture upload: negligible if using PBO (pixel buffer object) double-buffering
- Maximum simultaneous active kiosks: 3-5 (configurable, based on hardware)
- Inactive kiosks (player not nearby): suspend rendering, show a static screenshot
- LOD: at distance, show a low-res cached screenshot instead of live rendering

## Implementation Phases

1. **Phase 1**: CEF integration in engine — render a single URL to a texture, display on a quad
2. **Phase 2**: Input forwarding — mouse/keyboard from game input to CEF
3. **Phase 3**: Data-driven kiosks — load from TOML, place in world
4. **Phase 4**: VR interaction — controller ray input, virtual keyboard
5. **Phase 5**: Multi-kiosk management — LOD, suspend/resume, memory budget
6. **Phase 6**: Affiliate tracking dashboard — admin page showing kiosk revenue

## Files (future)

| Path | Role |
|------|------|
| `native/src/systems/browser.rs` | CEF lifecycle, offscreen rendering, texture upload |
| `native/src/components/kiosk.rs` | Kiosk ECS component |
| `native/src/input/browser_input.rs` | Input forwarding (mouse, keyboard, VR controller) |
| `data/kiosks/*.toml` | Kiosk definitions (URL, domain allowlist, placement) |
| `assets/icons/kiosks/` | Kiosk brand icons for world UI |
