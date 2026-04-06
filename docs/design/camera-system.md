# Camera System Architecture

Three unified camera modes sharing a single ECS `Camera` component. All modes work identically in native desktop and WASM browser builds.

## Camera Modes

### 1. First-Person — Primary Gameplay Mode

WASD movement with mouselook. This is the default mode for most activities: farming, building, cooking, exploring, crafting.

Features:
- Mouse controls pitch/yaw, WASD moves relative to facing direction
- Head bob (subtle sinusoidal offset tied to movement speed)
- Adjustable FOV (default 90, range 60-120)
- Crosshair overlay (context-sensitive: changes for interact, harvest, combat)
- Collision with world geometry prevents clipping through walls/terrain

Reference games: Minecraft, Valheim, Satisfactory.

Partially implemented in `crates/core-firstperson-controller/`.

### 2. Third-Person — Social and Action Mode

Camera follows behind the player at an adjustable distance. Better spatial awareness for combat and group activities. Shows the player's character, gear, and animations — important for social interaction and showing off builds.

Features:
- Adjustable follow distance (1.5m close, 3m default, 8m max)
- Shoulder swap: toggle camera offset left/right (useful for aiming, doorways)
- Zoom in/out with scroll wheel
- Camera collision: pulls forward when blocked by walls to prevent clipping
- Soft-lock targeting in combat (camera biases toward locked target)

Toggle between first and third person: `F` or `V` key.

Reference games: Skyrim, GTA, Fortnite.

### 3. Orbit/Free Camera — Maps, Planning, Spectating

Free rotation around a point of interest. Used for build planning, ship navigation, map exploration, and spectating other players.

Features:
- Left-click drag to orbit (rotate around focal point)
- Scroll to zoom in/out
- Middle-click drag to pan (shifts focal point)
- Smooth zoom transitions across scale levels (see Scale Transitions below)
- Orthographic projection toggle for isometric-style view (useful for building layouts)
- Optional grid overlay in orthographic mode

Enter orbit mode: `M` key (map/orbit toggle).

Reference games: Cities Skylines, Kerbal Space Program, Google Earth.

## Camera Transitions

No jarring cuts between modes. All transitions interpolate smoothly.

- **Rotation**: quaternion slerp over 0.3-0.5 seconds
- **Position**: cubic ease-in-out lerp over the same duration
- **FOV**: linear lerp (first-person FOV to orbit FOV)
- **Context-sensitive defaults**: entering a building switches to first-person; opening the map switches to orbit; entering a vehicle may switch to third-person
- **State memory**: each context (building interior, overworld, vehicle) remembers its last camera state and restores it on return

## Scale Transitions (Orbit Mode)

Orbit mode supports seamless zoom from character level to galactic scale. Each level loads progressively different LOD assets.

| Level | Range | What's Visible | LOD Strategy |
|-------|-------|----------------|--------------|
| Player | 1m - 100m | Character, homestead, nearby objects | Full detail meshes |
| Local | 100m - 10km | Town, terrain features, roads | Simplified meshes, billboard trees |
| Regional | 10km - 1,000km | Mountain ranges, biomes, cities as dots | Terrain heightmap, landmark icons |
| Planetary | 1,000km - 50,000km | Planet surface from orbit, atmosphere | Spherical terrain tiles, cloud layer |
| Solar System | 50,000km - 10 AU | Planets orbiting star, orbital paths | Point sprites, orbit lines |
| Galactic | 10 AU+ | Star field, nebulae, travel routes | Procedural star particles, route overlays |

Transitions between levels are continuous — no loading screens. Asset streaming loads the next LOD level in the background as the camera moves.

## Input Mapping

| Input | First-Person | Third-Person | Orbit |
|-------|-------------|-------------|-------|
| Mouse move | Look (pitch/yaw) | Look (pitch/yaw) | — |
| Left-click drag | — | — | Rotate around focal point |
| Middle-click drag | — | — | Pan (shift focal point) |
| Right-click | Aim/block | Aim/block | — |
| Scroll wheel | — | Zoom in/out | Zoom in/out |
| WASD | Move character | Move character | Pan camera |
| F or V | Switch to third-person | Switch to first-person | — |
| M | Enter orbit mode | Enter orbit mode | Exit orbit mode |
| Tab | Cycle: first -> third -> orbit -> first | Cycle | Cycle |
| O (in orbit) | — | — | Toggle orthographic/perspective |

## Technical Details

### ECS Camera Component

All three modes share one `Camera` component. The mode determines how input updates the component fields.

```
Camera {
    position: Vec3,          // world-space camera position
    target: Vec3,            // look-at point (character pos in first/third, focal point in orbit)
    up: Vec3,                // usually (0, 1, 0)
    fov: f32,                // vertical FOV in radians (perspective mode)
    near: f32,               // near clip plane (0.1 default)
    far: f32,                // far clip plane (scales with zoom level)
    projection: Projection,  // Perspective or Orthographic
    mode: CameraMode,        // FirstPerson, ThirdPerson, Orbit
    transition: Option<CameraTransition>,  // active interpolation state
}
```

### View/Projection Matrices

Updated every frame in the camera system:
- `view_matrix` = `look_at(position, target, up)`
- `projection_matrix` = `perspective(fov, aspect, near, far)` or `orthographic(...)` depending on projection type
- Both matrices uploaded to a shared GPU uniform buffer used by all shaders

### Transition System

```
CameraTransition {
    from: CameraState,      // snapshot of camera at transition start
    to: CameraState,        // target state
    elapsed: f32,            // seconds since transition began
    duration: f32,           // total transition time
    easing: EasingFn,       // cubic ease-in-out by default
}
```

When `transition` is `Some`, the system interpolates between `from` and `to` each frame. When `elapsed >= duration`, the transition completes and the field is set to `None`.

### Platform Compatibility

The camera system is pure Rust math (vectors, matrices, quaternions) with no platform-specific code. Works identically in:
- Native desktop via Tauri
- WASM browser via WebGPU
- Future VR headsets (override view matrices with headset pose data)

Input abstraction through `winit` handles the platform differences for mouse/keyboard events.

## Related Files

- `crates/core-firstperson-controller/` — existing first-person controller
- `src/systems/` — game systems that interact with camera
- `docs/design/engine-architecture.md` — master engine reference
- `docs/design/engine-wasm.md` — WASM compilation (same camera code runs in browser)
- `docs/design/maps-multi-scale.md` — multi-scale map rendering (orbit mode zoom levels)
