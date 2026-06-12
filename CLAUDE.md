# CLAUDE.md — First Light

Native Windows 3D game project. Bevy is the engine; **Claude Code is the editor.** All scenes, materials, lighting, physics, and gameplay are authored in Rust code. The human describes features in plain English; you design and implement them end to end. Target hardware: NVIDIA RTX 5090 — assume GPU headroom, but keep the architecture clean enough to scale anyway.

## Pinned stack (do not change without checking compatibility)

- **Rust**: latest stable, **MSVC toolchain, native Windows** (never WSL — the game must open a real window and talk to the NVIDIA driver directly)
- **bevy = "0.18"** — renders natively via Vulkan/DX12 through wgpu
- **avian3d = "0.6"** — ECS-native physics
- Optional, only when needed and after re-verifying versions: `bevy-tnua` 0.11 + `bevy-tnua-avian3d` 0.11 (floating character controller), `bevy_locomotion` (FPS controller)
- Compatibility ground truth: the version table in https://github.com/avianphysics/avian and each crate's README

## Golden rules

1. **Never write Bevy or Avian API calls from memory.** Bevy ships breaking changes roughly every 3 months; your training data is likely at least one version behind, and 0.17–0.18 heavily reworked events/observers/messages, UI, and parts of rendering. Before using any API you have not verified **in this session**, fetch ground truth: docs.rs for the pinned version, the official examples at the pinned git tag, or the migration guides. Guessed APIs that "look right" are the #1 failure mode of this project.
2. **Compile constantly.** Run `cargo check` after every meaningful edit. Do not introduce new warnings. Never end a session with a broken build.
3. **Small vertical slices.** One feature → check → run/verify → commit → next. No big-bang changes.
4. **Plugin per feature.** Every gameplay/rendering feature is a Bevy `Plugin` in its own file under `src/plugins/`. `main.rs` stays thin (app builder + plugin registration only).
5. **Vet every new dependency.** Before adding a crate, confirm it supports Bevy 0.18 (README table or a release dated after Jan 2026). Prefer writing 100 lines ourselves over adopting an unmaintained crate.
6. **Release-quality visuals by default.** Every scene gets tonemapping, real shadows, a sky/atmosphere (never a flat gray void), deliberate sun angle and exposure, and bloom where tasteful. "Programmer art" geometry is fine; programmer *lighting* is not.
7. **Verify visually with screenshots.** The game writes screenshots to `./screenshots/` (F2). After visual changes, take one and view the image file yourself; adjust if it looks flat or wrong. Ask the human to playtest for *feel* (controls, physics weight).
8. **Keep DEVLOG.md current.** One entry per session: what changed, decisions made, exact crate versions touched, open issues. It is the project's long-term memory.

## Session startup ritual

1. Read this file. 2. Read the last ~2 entries of `DEVLOG.md`. 3. `cargo check` to confirm a clean baseline. 4. Only then start the task.

## Layout — multi-game workspace

The repo is a Cargo workspace: a reusable, game-agnostic **engine** library plus one binary crate per game. Engine = anything a second game would want unchanged (rendering defaults, lighting, character controller, physics conventions, debug tools). Game = scene content + mechanics. Start code in the game crate; extract upward to the engine only once it's proven reusable — never generalize prematurely. A new game is a new folder under `games/` plus a few lines of plugin registration.

```
Cargo.toml             # [workspace]: shared deps (bevy, avian3d), profiles, default-members
crates/engine/src/
  lib.rs               # EnginePlugins group + prelude
  camera.rs            # MainCamera marker → HDR, tonemapping, bloom, exposure, atmosphere, FXAA
  lighting.rs          # sun + shadows via SunSettings resource (games override before Startup)
  physics.rs           # avian3d setup + conventions
  player.rs            # first-person kinematic controller (ported from Avian's example) + spawn_player()
  debug.rs             # F2 screenshot, F3 FPS overlay (dev_tools feature), F4 vsync
games/first_light/
  src/main.rs          # app builder only
  src/plugins/         # world.rs, props.rs, throw.rs — game-specific content
  assets/              # per-game assets: models/ textures/ audio/
tools/
  blender/             # headless Blender python scripts that generate .glb files
DEVLOG.md
screenshots/           # gitignored
```

## Rendering defaults (verify exact 0.18 names before use)

- `Camera3d` with HDR enabled; default tonemapping (TonyMcMapface); subtle bloom.
- One `DirectionalLight` sun with shadows on; tune cascade config to scene scale.
- Outdoor scenes use Bevy's procedural atmosphere for sky + ambient instead of a clear color.
- Anti-aliasing: pick ONE path (MSAA, or TAA, or later DLSS) — they conflict.
- Default to vsync; expose a toggle in the debug plugin. Target 4K-native on the 5090.

## Physics conventions (avian3d)

- Dynamic objects: `RigidBody::Dynamic` + primitive `Collider`s sized to the visual mesh. Trimesh colliders only for static geometry.
- Static world: `RigidBody::Static`.
- Player: kinematic controller, ported from Avian's official kinematic character controller example (fetch the example source for the pinned version as ground truth — do not reinvent ground detection).
- Introduce `CollisionLayers` as soon as there are more than two interaction categories.
- If movement looks jittery, enable Avian's transform interpolation (verify the current feature/component name) before touching timesteps.

## Asset pipeline

- Interchange format: **glTF (.glb) only.**
- Custom models are *generated, not hand-made*: write Python scripts in `tools/blender/` and run `blender --background --python tools/blender/<script>.py` to emit `.glb` into `assets/models/`. Procedural modeling via `bpy` keeps every asset reviewable and regenerable. (If Blender is missing: `winget install BlenderFoundation.Blender`.)
- Textures/HDRIs: PolyHaven (CC0). Download via a script in `tools/`, record source URLs in `assets/CREDITS.md`.
- Never hand-edit binary assets; fix the generating script and regenerate.

## Build profiles & performance

- Dev loop: `cargo run -p first_light --features dev` — the `dev` feature enables `bevy/dynamic_linking` + dev tooling; the workspace Cargo.toml sets `opt-level = 1` for our code and `opt-level = 3` for dependencies. **Never** benchmark or distribute a dynamic-linking build.
- Real performance checks: `cargo run -p first_light --release` (add `--features dev_tools` to keep the FPS overlay without dynamic linking).
- Measure before optimizing: FPS overlay + Bevy diagnostics first; deeper profiling only if a real problem shows up.

## NVIDIA experimental track (opt-in, own branch)

DLSS (upscaling/AA) and **Solari** (raytraced direct + indirect lighting; denoised by DLSS Ray Reconstruction, so NVIDIA-only in practice) are supported and a 5090 is the ideal card for them — but both are experimental and DLSS setup requires accepting NVIDIA's SDK license. When the human asks for "the Solari experiment": create a `solari` branch, read the Solari/DLSS sections of the Bevy 0.17 and 0.18 release notes at bevy.org/news plus any linked setup docs, and follow them exactly. Never attempt this from memory. Keep `main` on the standard rasterized path.

## Windows notes

- Native PowerShell. Toolchain = rustup + MSVC ("Desktop development with C++" workload of VS Build Tools).
- Linker errors about missing libs usually mean the C++ workload is incomplete.
- wgpu picks the high-performance adapter by default; if the wrong GPU is ever selected, configure the render plugin's adapter/power preference (verify current API).

## Ground-truth URLs

- Release notes: https://bevy.org/news/bevy-0-18/
- Migration guides: https://bevy.org/learn/migration-guides/
- Bevy examples at the pinned tag: https://github.com/bevyengine/bevy/tree/v0.18.1/examples
- Bevy API docs: https://docs.rs/bevy/0.18
- Avian repo + examples: https://github.com/avianphysics/avian
- Avian API docs: https://docs.rs/avian3d/0.6

## How the human works with you

Plain-English requests: "add a grappling hook," "make dusk lighting," "rope bridge that actually swings." You: plan briefly, verify any unfamiliar API, implement as a plugin, compile, screenshot, devlog. When a request is ambiguous, choose the grounded, physically plausible interpretation and say what you chose. The world should always feel *real*: things have weight, light has direction, materials respond to it.
