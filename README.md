# Graphics-Engine

A Bevy 0.18 + Avian 0.6 game workspace: a reusable, game-agnostic **engine**
library (`crates/engine`) and one binary crate per game (`games/`). Authored
entirely in code — see `CLAUDE.md` for the project conventions and `DEVLOG.md`
for session history.

## Games

### First Light (`games/first_light`)

A sunlit first-person physics playground: procedural atmosphere, real-time
shadows, a toppleable crate pyramid, and throwable glowing cubes.

```sh
# Dev loop (fast rebuilds via dynamic linking, FPS overlay included)
cargo run -p first_light --features dev

# Release (real performance; add --features dev_tools to keep the FPS overlay)
cargo run -p first_light --release
```

| Input        | Action                          |
| ------------ | ------------------------------- |
| Click        | Grab cursor (mouse look)        |
| WASD + mouse | Move + look                     |
| Space        | Jump                            |
| Left Shift   | Sprint                          |
| Left click   | Throw a glowing cube (when grabbed) |
| Esc          | Release cursor                  |
| F2           | Screenshot → `./screenshots/`   |
| F3           | Toggle FPS overlay (dev builds) |
| F4           | Toggle vsync                    |

Building on Windows requires the MSVC toolchain; on Linux:
`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
