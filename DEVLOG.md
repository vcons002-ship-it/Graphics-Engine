# DEVLOG

## Entry #1 — 2026-06-12 — Workspace scaffold + First Light demo

### Stack (exact resolved versions from Cargo.lock)

- rustc 1.94.1, edition 2024
- bevy **0.18.1** (wgpu 27.0.1, winit 0.30.13)
- avian3d **0.6.1** (parry3d 0.26.1) — Avian's table confirms Bevy 0.18 ↔ Avian 0.5–0.6

### What was built

- **Cargo workspace** for multi-game development: `crates/engine` (reusable
  library: camera/rendering defaults, lighting, physics, first-person
  controller, debug tools) + `games/first_light` (this demo: world, props,
  throw mechanic). Rule: code starts in the game crate and is extracted to
  the engine only once proven reusable.
- **First Light demo**: borderless-fullscreen window, HDR camera with
  TonyMcMapface + `Bloom::NATURAL` + FXAA, procedural atmosphere
  (late-afternoon sun, `lux::RAW_SUNLIGHT`, ev100 13), shadow-casting sun
  with cascades tuned to scene scale, 400 m ground slab, 15-crate toppleable
  pyramid, 30 scattered PBR props (metallic/rough/emissive mix), first-person
  kinematic controller (WASD + mouse look + Space jump + Shift sprint, click
  grabs cursor / Esc releases), left-click throws glowing cubes (30 s
  lifetime), F2 screenshot → `./screenshots/`, F3 FPS overlay (dev builds),
  F4 vsync toggle.

### Key decisions

- **Multi-game architecture** (user request): engine = game-agnostic plugin
  library; games are thin binaries. The engine never wraps or hides Bevy, so
  it imposes no capability ceiling — games can always use Bevy/Avian APIs
  directly.
- **Character controller** ported from Avian v0.6.1's official
  `kinematic_character_3d` example (move-and-slide via the `MoveAndSlide`
  system param, shape-cast ground detection, impulses pushed into dynamic
  bodies). One deliberate change: per-frame input is accumulated into a
  `MoveInput` component consumed in `FixedUpdate`, instead of the example's
  per-frame messages, so acceleration is independent of render frame rate
  (matters at the 5090's frame rates).
- **0.18 atmosphere API**: `Atmosphere::earthlike(handle)` now takes a
  `ScatteringMedium` asset (changed from 0.17's `Atmosphere::EARTH`).
  `Bloom` and `Atmosphere` `#[require(Hdr)]`, so HDR is automatic.
  `GlobalAmbientLight::NONE` + `AtmosphereEnvironmentMapLight` on the camera
  for physically based ambient, per the official atmosphere example.
- **AA path**: `Msaa::Off` + FXAA (one path only, matching the atmosphere
  example; MSAA/TAA/DLSS would conflict).
- **Emissives** are scaled ×60 000 — daylight at ev100 13 needs huge
  emissive values to bloom.
- **`ENGINE_AUTO_SCREENSHOT=<frame>`** env var: capture at frame N and exit;
  used for headless visual verification (Xvfb + Mesa lavapipe) since this
  session ran in a Linux container without GPU/display.
- This session ran on Linux; the Windows-specific Phase 0 (winget/MSVC) was
  skipped. Code is platform-neutral; nothing Windows-specific was needed.

### Open items

- Real-hardware verification: FPS numbers, controller feel, physics weight —
  needs a playtest on the target Windows/RTX 5090 machine.
- The FPS overlay needs `--features dev` or `--features dev_tools`; plain
  release has no overlay (F4 vsync + F2 screenshot still work).
- Thrown cubes use a fixed 30 s lifetime; consider a max-count cap instead.
- No `CollisionLayers` yet — introduce when a third interaction category
  appears (per CLAUDE.md).
