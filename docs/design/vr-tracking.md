# VR Integration & Indoor Positioning

Status: Draft
Created: 2026-03-17

## Overview

This document covers two related capabilities for HumanityOS: VR headset integration via OpenXR, and indoor positioning for placing users in virtual or augmented environments. The goal is to let people inhabit shared 3D spaces — starting with what headsets already provide, then expanding to room-aware experiences and non-VR device tracking.

---

## 1. VR Integration via OpenXR

### Why OpenXR

OpenXR is the Khronos Group standard for VR/AR runtime access. Every major platform supports it:

| Platform | OpenXR Support |
|----------|---------------|
| SteamVR | Native runtime |
| Meta Quest | Native runtime (standalone + Link) |
| Windows Mixed Reality | Native runtime |
| Monado (Linux) | Open-source runtime |

Using OpenXR means one integration path covers all headsets. No vendor-specific SDKs required.

### Rust Integration

- **Crate:** [`openxr`](https://crates.io/crates/openxr) — well-maintained Rust bindings to the OpenXR loader
- **Rendering:** `wgpu` for cross-platform GPU access (Vulkan, DX12, Metal)
- **Frame loop:** Application renders stereo frames via wgpu, submits them to the OpenXR compositor for display

### Capabilities Provided by OpenXR

| Feature | Description |
|---------|-------------|
| Head tracking | 6DoF position + orientation of the headset |
| Controller input | Button state, trigger values, thumbstick axes, 6DoF pose |
| Hand tracking | Per-joint skeleton data (26 joints per hand) |
| Eye tracking | Gaze direction vector (hardware-dependent, requires user opt-in) |
| Reference spaces | LOCAL, STAGE, VIEW — coordinate systems for seated, standing, or head-relative use |
| Passthrough | Camera feed composited with rendered content (Quest 3, Varjo, etc.) |

### Architecture

```
HumanityOS Client (Tauri / Standalone)
  └─ wgpu renderer
       ├─ 2D UI layer (existing HTML/JS via WebView)
       └─ 3D world layer
            └─ OpenXR session
                 ├─ Swapchain (stereo frame submission)
                 ├─ Action sets (input bindings)
                 └─ Spatial anchors (room-locked objects)
```

The 3D layer runs alongside the existing 2D client. In VR mode, the 2D interface can be projected onto a virtual panel within the 3D scene.

---

## 2. Indoor Positioning Technologies

For placing non-VR devices (phones, tablets, laptops) in a shared spatial context, several technologies exist — each with significant trade-offs.

### WiFi RTT (802.11mc / Fine Time Measurement)

- **Accuracy:** 1-2 meters with 3+ compatible access points
- **How it works:** Measures round-trip time of WiFi frames to triangulate position
- **Support:**
  - Android 9+ with compatible chipsets
  - Some Intel WiFi chips on Windows (limited driver support)
  - NOT widely supported on Windows or iOS
- **Verdict:** Promising on Android, but requires compatible APs and client hardware. Not cross-platform enough today.

### WiFi Fingerprinting

- **Accuracy:** 2-5 meters (room-level)
- **How it works:** ML model trained on signal strength patterns at known positions
- **Drawback:** Requires collecting training data for every room. Signal patterns change when furniture moves or doors open.
- **Verdict:** High maintenance burden. Not practical for home use.

### UWB (Ultra-Wideband)

- **Accuracy:** 10-30 centimeters
- **How it works:** Time-of-flight measurement using very short radio pulses
- **Hardware required:**
  - Phones: iPhone 11+, Samsung S21+, Pixel 7 Pro+
  - Fixed anchors: UWB tags/beacons (e.g., Decawave DW1000 modules)
- **Verdict:** Best radio-based accuracy by far. Requires UWB anchors installed in the space. Good fit for dedicated installations (offices, warehouses) but adds hardware cost for home users.

### Bluetooth RSSI

- **Accuracy:** 2-5 meters
- **How it works:** Signal strength from BLE beacons, mapped to approximate distance
- **Support:** Universal — every phone and most laptops have BLE
- **Drawback:** Extremely noisy. Human bodies, walls, and furniture cause multipath interference. Not reliable for sub-room positioning.
- **Verdict:** Useful for room-level presence detection ("user is in the kitchen"), not for spatial placement.

### Camera-Based SLAM (Simultaneous Localization and Mapping)

- **Accuracy:** Centimeter-level
- **How it works:** Visual features from camera frames are tracked to build a 3D map and localize the device within it
- **Support:**
  - Quest 2/3/Pro: Built-in SLAM is the primary tracking method
  - ARKit (iOS): World tracking with 6DoF
  - ARCore (Android): World tracking with 6DoF
  - Laptops/desktops: Possible with webcam + OpenCV, but CPU-intensive and less robust
- **Verdict:** Best accuracy. Already the foundation of all VR headset tracking. Phones get this for free via ARKit/ARCore.

---

## 3. 3D Home Recreation

Three approaches to getting room geometry into HumanityOS:

### Option A: LiDAR Scan

- **Hardware:** iPhone Pro (12+), iPad Pro (2020+) — these have built-in LiDAR
- **Workflow:** User scans room with phone app, exports point cloud or mesh (OBJ/USDZ/GLB)
- **Quality:** Good geometry, ~1cm depth accuracy at room scale
- **Export tools:** Polycam, 3D Scanner App, RoomPlan API (iOS 16+, outputs parametric room model)

### Option B: Manual Room Editor

- **Hardware:** None required
- **Workflow:** User places walls, doors, windows, and furniture in a 2D/3D editor within HumanityOS
- **Quality:** As accurate as the user makes it
- **Advantage:** Works on any device, no special sensors needed
- **Disadvantage:** Tedious for complex rooms

### Option C: Quest 3 Spatial Mesh API

- **Hardware:** Quest 3 (or Quest Pro with v62+ firmware)
- **Workflow:** Automatic. The headset continuously maps the room and exposes geometry via the Scene API
- **Quality:** Room-scale mesh with wall/floor/ceiling/furniture classification
- **Advantage:** Zero user effort — the headset does it during normal use
- **Integration:** Access via OpenXR `XR_META_spatial_entity_mesh` extension

### Mixed Reality / Passthrough

VR headsets with color passthrough cameras (Quest 3, Apple Vision Pro, Varjo XR-4) already solve the "position real person in virtual room" problem. The headset's SLAM tracking places virtual objects in the physical room. This is not something HumanityOS needs to build — it is a runtime capability provided by the headset.

HumanityOS just needs to:
1. Define virtual objects and their spatial anchors
2. Submit them to the OpenXR compositor
3. The headset handles camera feed + overlay rendering

---

## 4. Phased Approach

### Phase 1: OpenXR for Standard VR Headsets

- Integrate the `openxr` crate into the Tauri/standalone client
- Render a basic 3D environment via wgpu with stereo output
- Support head tracking, controller input, and hand tracking
- Project the existing 2D chat/task UI onto a virtual panel
- **Tracking is handled entirely by the headset** — no external hardware needed

### Phase 2: Room Scanning

- Support importing room meshes from:
  - Quest 3 spatial mesh API (automatic, via OpenXR extensions)
  - LiDAR scans exported as GLB/OBJ (manual import)
  - iOS RoomPlan parametric models
- Build a simple manual room editor as fallback
- Use room geometry to place shared virtual objects (whiteboards, task boards, screens)

### Phase 3: Non-VR Indoor Positioning

- For phones/tablets: leverage ARKit/ARCore for camera-based 6DoF tracking
- For dedicated spaces (offices, community centers): UWB anchor installation for centimeter-level tracking of any device
- WiFi/Bluetooth as fallback for coarse room-level presence detection only
- **Do not invest in WiFi fingerprinting** — maintenance cost exceeds value for home/small-space use

---

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| OpenXR over vendor SDKs | One integration covers all headsets. Future-proof. |
| wgpu for rendering | Cross-platform GPU abstraction. Matches Rust stack. |
| Headset-first tracking | VR headsets already have the best tracking. Don't rebuild what exists. |
| Skip WiFi fingerprinting | Requires per-room training data. Too fragile for general use. |
| UWB for dedicated spaces only | Great accuracy but requires hardware investment. Not a default path. |
| Camera SLAM for phones | ARKit/ARCore are free and accurate. Natural bridge between phone and headset experiences. |

---

## Dependencies

- `openxr` crate (Rust bindings)
- `wgpu` crate (GPU rendering)
- OpenXR loader installed on user's system (comes with SteamVR, Quest Link, etc.)
- For Phase 2: Quest 3 firmware with Scene API, or iOS device with LiDAR
- For Phase 3: ARKit/ARCore-capable phone, or UWB hardware
