# Audio Engine Architecture

**Status:** Proposal
**Component:** Game / Desktop Audio
**Last updated:** 2026-03-17

---

## Goals

1. **Highest fidelity** -- no compromises on sample rate, bit depth, or spatial accuracy.
2. **Massive polyphony** -- hundreds of simultaneous sounds (gunfire volleys, crowd chaos, explosions overlapping) without dropouts.
3. **Zero clipping** -- per-voice limiting and a master limiter guarantee clean output regardless of voice count.
4. **Surround sound** -- 5.1, 7.1, and Dolby Atmos support out of the box.
5. **Spatial / 3D audio** -- every in-world sound positioned accurately with HRTF, occlusion, and distance attenuation.
6. **Clean mix separation** -- SFX, dialog, music, ambient, and UI on independent buses with per-bus volume, ducking, and effects.
7. **VR-ready** -- architecture must extend to head-tracked binaural rendering without a rewrite.

---

## Option Analysis

### kira (Rust, MIT)

| Aspect | Detail |
|--------|--------|
| Language | Pure Rust, no FFI |
| Voices | 256+ simultaneous, lock-free mixer thread |
| Spatial | Built-in spatial audio (distance attenuation, panning) |
| Features | Tweening, ducking, clock-synced playback, streaming |
| License | MIT -- no revenue restrictions |
| Maturity | Active development, growing game-engine adoption |
| **Con** | No built-in Atmos or surround output routing. Relies on CPAL for platform audio output, which supports multi-channel devices but has no Atmos renderer. |

**Verdict:** Excellent mixing and playback core. Needs a spatialization partner for physics-based HRTF and advanced surround.

### FMOD (C++, commercial)

| Aspect | Detail |
|--------|--------|
| Language | C/C++ with Rust FFI bindings (libfmod-sys) |
| Voices | Unlimited via virtual voice system (inaudible voices cost near zero) |
| Spatial | Built-in Atmos, 7.1.4, HRTF, object-based audio |
| Features | Occlusion, reverb zones, full DSP chain, profiler, live mixing tool |
| License | Free under $200k revenue; commercial license above that |
| Maturity | Industry standard -- used in thousands of shipped AAA titles |
| **Con** | Commercial license required at scale. C++ FFI adds build complexity and potential for unsafe boundary bugs. Proprietary -- no source access to core. |

**Verdict:** The safe industry pick. Maximum features, but licensing and FFI complexity work against our open-source, pure-Rust goals.

### Steam Audio (C, free/open-source)

| Aspect | Detail |
|--------|--------|
| Language | C API, straightforward Rust FFI via bindgen |
| Spatial | Physics-based HRTF, ambisonics up to third order, real-time occlusion and transmission through geometry |
| Performance | GPU-accelerated reverb (OpenCL/Radeon Rays/Embree), CPU fallback |
| License | Apache 2.0 -- fully free, no revenue cap |
| Maturity | Shipped in CS2, Half-Life: Alyx; maintained by Valve |
| **Con** | Spatialization only -- no mixer, no voice management, no bus routing. Must pair with a separate mixing engine. |

**Verdict:** Best-in-class spatialization at zero cost. Pairs naturally with kira to fill kira's surround gap.

### Web Audio API (browser)

| Aspect | Detail |
|--------|--------|
| Language | JavaScript, already available in WebView2 |
| Voices | Spec allows many; browsers practically cap useful polyphony around 32 sources before performance degrades |
| Spatial | PannerNode offers basic 3D panning; no HRTF customization, no occlusion |
| License | Free (platform API) |
| **Con** | No real-time thread priority -- GC pauses cause glitches under load. No surround output. No Atmos. Not viable for hundreds of simultaneous sounds. |

**Verdict:** Fine for browser-only UI sounds and notifications. Not viable as the primary game audio engine.

---

## Recommended Architecture

**kira (mixing + playback) + Steam Audio (spatialization)**

This combination delivers:

- Pure Rust mixing core (kira) -- lock-free, 256+ voices, built-in ducking and tweening.
- Physics-based spatialization (Steam Audio via C FFI) -- HRTF, occlusion, GPU reverb, ambisonics.
- Fully open-source stack -- MIT + Apache 2.0, no revenue cap.
- Minimal FFI surface -- only the Steam Audio C API crosses the boundary; kira is native Rust.
- VR path -- Steam Audio already supports head-tracked binaural rendering; kira feeds it pre-mixed stems.

### Data flow

```
Game World (positions, velocities, materials)
       |
       v
Steam Audio           kira
+------------------+  +-----------------------------+
| Per-voice HRTF   |  | Voice pool (256+)           |
| Occlusion query  |->| Per-voice gain/filter apply |
| Reverb (GPU/CPU) |  | Bus routing + ducking       |
+------------------+  | Master limiter              |
                       | CPAL output (stereo/5.1/7.1)|
                       +-----------------------------+
                                  |
                                  v
                          Hardware output
                       (speakers / headphones)
```

**Atmos path (future):** When CPAL gains object-based output support, or via platform-specific backends (Windows Spatial Sound API), kira's output can route through the OS Atmos renderer. Steam Audio's ambisonics output maps directly to Atmos bed channels.

---

## Mix Bus Architecture

```
                         +----------------+
                         |   Master Bus   |
                         |  master limiter|
                         |  volume: 1.0   |
                         +-------+--------+
                                 |
          +----------+-----------+-----------+----------+
          |          |           |           |          |
    +-----+--+ +----+---+ +----+----+ +----+---+ +----+---+
    | Music  | |  SFX   | | Dialog  | | Ambient| |   UI   |
    | Bus    | |  Bus   | |  Bus    | |  Bus   | |  Bus   |
    +--------+ +--------+ +---------+ +--------+ +--------+
    |vol: 0.8| |vol: 1.0| |vol: 1.0 | |vol: 0.6| |vol: 0.7|
    |spatial:| |spatial:| |spatial: | |spatial:| |spatial:|
    | no     | | yes    | | yes     | | yes    | | no     |
    |duck by:| |duck by:| |duck by: | |duck by:| |duck by:|
    | Dialog | | --     | | --      | | Dialog | | --     |
    +--------+ +--------+ +---------+ +--------+ +--------+
```

### Bus definitions

| Bus | Spatial | Duck trigger | Duck amount | Notes |
|-----|---------|-------------|-------------|-------|
| **Master** | -- | -- | -- | Final output. Brick-wall limiter at -0.5 dBFS prevents clipping under all conditions. |
| **Music** | No (pre-mixed stereo/5.1) | Ducked by Dialog | -12 dB, 200ms attack, 800ms release | Background score. Crossfade support for track transitions. |
| **SFX** | Yes (3D positioned) | Never ducked | -- | Gunshots, explosions, impacts. Per-voice compressor + bus limiter. Highest polyphony demand. |
| **Dialog** | Yes (3D positioned) | Ducks Music + Ambient | -- | Voice lines, NPC speech. Priority voice allocation -- dialog never gets virtualized. |
| **Ambient** | Yes (3D positioned) | Ducked by Dialog | -8 dB, 300ms attack, 1200ms release | Wind, rain, crowd murmur, environmental loops. |
| **UI** | No (screen-space) | Never ducked | -- | Menu clicks, notifications, HUD feedback. Bypasses spatial pipeline entirely. |

### Anti-clipping strategy

1. **Per-voice gain staging** -- each voice is normalized at load time; runtime gain never exceeds 0 dB.
2. **Bus-level compressor** -- soft-knee compressor on the SFX bus (threshold -6 dB, ratio 4:1) tames peaks when dozens of sounds fire simultaneously.
3. **Ducking automation** -- dialog presence automatically lowers music and ambient, reducing mix density at the loudest moments.
4. **Master limiter** -- brick-wall look-ahead limiter on the master bus. Ceiling at -0.5 dBFS. This is the final safety net; the upstream stages should prevent it from engaging often.
5. **Virtual voice system** -- voices below an audibility threshold (distance + occlusion + bus level) are virtualized (tracked but not mixed), freeing DSP budget. When they become audible again, they resume seamlessly.

---

## Voice Management

```
Voice request
     |
     v
Priority check (dialog > SFX > ambient > music)
     |
     v
Voice pool full?
  no  --> allocate voice, apply spatial, route to bus
  yes --> steal lowest-priority voice below threshold
            |
            v
          stolen voice virtualized (position tracked, no DSP cost)
```

- **Pool size:** 256 real voices default, configurable up to hardware limits.
- **Virtual voices:** Unlimited. Track position and parameters; re-activate on audibility.
- **Priority classes:** Dialog (highest) > SFX > Ambient > Music > UI.
- **Steal policy:** Lowest audibility score = distance attenuation * occlusion * bus volume. Same-priority ties broken by age (oldest stolen first).

---

## Spatial Audio Pipeline (Steam Audio)

For each spatially-enabled voice per frame:

1. **Direct path** -- compute HRTF-filtered direct sound based on source and listener positions.
2. **Occlusion** -- ray-cast against scene geometry. Partial occlusion applies frequency-dependent transmission filter (e.g., wall muffles highs).
3. **Reflections** -- early reflections from nearby surfaces (real-time ray tracing, GPU-accelerated when available).
4. **Reverb** -- late reverb tail from scene geometry. Baked for static scenes, real-time for dynamic.
5. **Output** -- HRTF-convolved stereo (headphones) or channel-mapped surround (speakers). Ambisonics intermediate for Atmos bed.

### Surround output modes

| Mode | Channels | Method |
|------|----------|--------|
| Stereo | 2 | HRTF binaural (headphones) or stereo downmix (speakers) |
| 5.1 | 6 | Channel-based panning from Steam Audio output |
| 7.1 | 8 | Channel-based panning from Steam Audio output |
| Atmos | 7.1.4 + objects | Ambisonics to Atmos bed via Windows Spatial Sound API |
| VR | 2 (binaural) | Head-tracked HRTF via Steam Audio + HMD orientation feed |

---

## VR Audio Path (Future)

Steam Audio already supports head-tracked binaural rendering. The integration path:

1. HMD reports head orientation per frame (quaternion from OpenXR/SteamVR).
2. Steam Audio listener orientation updated before spatial processing.
3. HRTF convolution produces binaural output matched to head pose.
4. kira routes binaural output to HMD audio device via CPAL.

No architectural changes required -- only a head-tracking input feed and device routing.

---

## File Formats and Loading

| Format | Use case | Decode |
|--------|----------|--------|
| Ogg Vorbis (.ogg) | SFX, dialog, ambient | Decoded to PCM on load (small files) or streamed (long files) |
| Opus (.opus) | Voice chat, compressed dialog | Real-time decode, lowest bitrate for speech |
| FLAC (.flac) | Music, high-fidelity SFX | Streamed from disk, lossless |
| WAV (.wav) | Dev/debug, very short one-shots | Direct PCM, no decode overhead |

- **Sample rate:** 48 kHz (matches Steam Audio HRTF dataset and most output devices).
- **Bit depth:** 32-bit float internal processing; 16-bit or 24-bit for storage.
- **Streaming threshold:** Files over 256 KB are streamed from disk rather than fully decoded into memory.

---

## Crate Dependencies

| Crate | Role | License |
|-------|------|---------|
| `kira` | Mixing engine, voice management, bus routing, tweening | MIT |
| `steam-audio-sys` | Steam Audio C bindings (bindgen) | Apache 2.0 |
| `cpal` | Cross-platform audio output (WASAPI/CoreAudio/ALSA/PulseAudio) | Apache 2.0 |
| `symphonia` | Audio file decoding (Vorbis, FLAC, WAV) | MPL 2.0 / Apache 2.0 |

---

## Open Questions

1. **Atmos timeline** -- Windows Spatial Sound API integration depends on CPAL multi-channel maturity. Monitor `cpal` and `windows-rs` progress.
2. **GPU reverb availability** -- Steam Audio GPU path requires OpenCL or Embree. Fallback CPU path works but costs more per frame. Profile on target min-spec hardware.
3. **Hot-reload in dev** -- kira supports replacing sounds at runtime. Define a dev workflow for sound designers to iterate without restarting the game.
4. **Console ports** -- kira + CPAL targets desktop OSes. Console audio backends (XAudio2 on Xbox, AAudio on Switch) would need platform-specific CPAL backends or a different output layer.
