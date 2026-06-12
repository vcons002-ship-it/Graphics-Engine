# DEVLOG

## Entry #2 — 2026-06-12 — Pause menu, mountain valley, castle

Same stack as Entry #1 (bevy 0.18.1, avian3d 0.6.1). Verified on the
user's Windows machine via the installer; FPS counter was missing in their
build because the overlay was behind the removed `dev_tools` feature.

### What was built

- **Engine `MenuPlugin`** (`crates/engine/src/menu.rs`): Esc opens a pause
  menu (pauses `Time<Virtual>`, releases the cursor). Screens: Main
  (Resume/Controls/Settings/Restart/Exit), Controls (rows from the
  `ControlsHelp` resource — games override it), Settings (VSync,
  Fullscreen, FPS counter toggles with live labels). `RestartRequested`
  message lets the game rebuild its world; Exit writes `AppExit`. Esc
  handling moved out of the player controller; the menu owns cursor state.
  Bevy `States` + `DespawnOnExit` per screen. Note: `BorderRadius` is a
  `Node` field in 0.18, not a component.
- **Built-in FPS counter** in `DebugPlugin` (FrameTimeDiagnosticsPlugin +
  small text overlay, F3 toggle, on by default). The `dev_tools` cargo
  feature is gone; `dev` is just dynamic linking now.
- **Mountain valley terrain** (`terrain.rs`): hand-rolled value-noise fBm
  (no new deps), U-shaped valley with headwall, castle terrace + spawn-pad
  flattening, walkable ~25° causeway carved into the slope, lake basin,
  vertex-colored mesh (grass/rock/snow/shore/cobble road) at 257², and a
  **trimesh collider built from the exact visual mesh**.
- **Castle** (`castle.rs`): parametric — crenellated curtain walls, four
  corner towers with slate cone roofs, twin-tower gatehouse over the
  causeway, keep with corner turrets, 38 m great tower with banner, warm
  emissive windows, courtyard hall/stables. Large surfaces use a
  procedurally generated tileable ashlar texture (256² Image, repeat
  sampler) with per-piece `uv_transform` scaling so blocks stay ~1 m.
- **Vegetation** (`vegetation.rs`): ~320 pines (shared cone/cylinder
  meshes, 4 canopy materials), 80 boulders, 150 bushes, deterministic
  rejection sampling honoring slope/height bands and keep-clear zones.
- Playground (crates/props/throw) relocated to a flat pad on the valley
  floor; props sit on `terrain_height`. Restart despawns `Respawnable`
  entities (player + dynamics) and respawns them.

### Hard-won lessons

- **avian3d 0.6.1 `Collider::heightfield` produced no contacts at all**
  (player and crates fell straight through; diagnosed with a probe system
  logging positions). Switched to `Collider::trimesh_from_mesh` on the
  visual mesh — works, and guarantees collision/visual parity. Worth
  re-testing heightfield on the next avian upgrade.
- parry's heightfield convention is `heights_zx` (rows advance along Z) —
  avian 0.6.1's doc comment claims the opposite; trust parry's source.
- Headless verification hooks now in the engine: `ENGINE_AUTO_SCREENSHOT`,
  `ENGINE_AUTO_MENU=<frame>[:screen]`, and the game's `FL_SPAWN="x,z,yaw"`.

### Open issues

- Restart and menu interactions verified by code review + screenshots of
  every screen; needs a real playtest (clicking buttons headless isn't
  wired).
- Castle gate-level vantage screenshots kept hitting terrain occlusion;
  verify the causeway approach feel in playtest.
- llvmpipe renders the full scene at ~1 FPS at 720p in this container —
  meaningless for the 5090, but slow for iteration.

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
- **Emissives**: daylight at ev100 13 needs large emissive values to bloom;
  ×5 000 glows tastefully, ×60 000 produces flares that white out meters of
  ground. Emissive props are scattered on the sun-facing side so their bloom
  never sits inside the crate stack's shadow.
- **`ENGINE_AUTO_SCREENSHOT=<frame>`** env var: capture at frame N and exit;
  used for headless visual verification (Xvfb + Mesa lavapipe) since this
  session ran in a Linux container without GPU/display.
- This session ran on Linux; the Windows-specific Phase 0 (winget/MSVC) was
  skipped. Code is platform-neutral; nothing Windows-specific was needed.

### The missing-shadows investigation (lesson for future sessions)

Headless screenshots initially showed **no cast shadows**. A long bisection
(crates/engine/examples/shadow_probe.rs, kept for reuse) cleared every
suspect one by one: llvmpipe, cascade config, atmosphere, HDR/bloom/exposure,
late camera decoration, Avian + TransformInterpolation, the player hierarchy,
AtmosphereEnvironmentMapLight, and the FPS overlay — shadows rendered
correctly in *every* probe. The real cause: two over-bright emissive props
(×60 000) had deterministically scattered into the exact shadow corridor of
the crate pyramid, and their bloom flares painted over the shadow in every
screenshot; sun-angle diagnostics (near-overhead, backlit) also hid shadows
geometrically. Confirmed by zeroing emissives → long, crisp golden-hour
shadow. Morals: (1) verify with pixel measurements, not just eyeballs —
PIL luminance bands settled it; (2) when bisecting visuals, change the
*composition* (remove occluders) before suspecting the renderer; (3) a
near-overhead sun is a useless shadow test — shadows hide under objects.

### Open items

- `firstlight_install.bat` / `firstlight_update.bat` / `firstlight_start.bat`
  (repo root) handle Windows setup, updates, and launching — double-click to
  run. Each checks for existing installs (vswhere for the MSVC workload,
  `where` for rustup/cargo/blender) before installing anything. CRLF endings
  enforced via .gitattributes. Not yet executed on a real Windows machine.

- Real-hardware verification: FPS numbers, controller feel, physics weight —
  needs a playtest on the target Windows/RTX 5090 machine.
- The FPS overlay needs `--features dev` or `--features dev_tools`; plain
  release has no overlay (F4 vsync + F2 screenshot still work).
- Thrown cubes use a fixed 30 s lifetime; consider a max-count cap instead.
- No `CollisionLayers` yet — introduce when a third interaction category
  appears (per CLAUDE.md).
