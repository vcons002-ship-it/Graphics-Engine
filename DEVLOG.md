# DEVLOG

## Entry #11 — 2026-06-13 — Spiral stairs and the attacker swarm

- **Spiral staircases** inside every manned tower: a doorway is cut in the
  shaft's bailey-facing base (`ring_tower` skips those voussoirs), a central
  newel post rises to the platform, and a contiguous helix of stone treads
  (angular step sized so ~1.2 m treads touch — no gaps to fall through)
  climbs around it. A pure `castle::tower_navs()` generates the climb path,
  and `build_spiral` lays the geometry from that same path, so steps and
  navigation can't drift apart.
- **Attacker swarm**: through the breached gate (or off a ladder) attackers
  enter `Hunting` — seek the nearest foe in 3D; chase a ground-level enemy
  directly, or for one up on a tower/wall route to the nearest tower base
  and `ClimbSpiral` up the helix tread by tread (geometry-following keeps
  them on the steps) to engage the archers on the platform.
- Cached the spiral paths in a `CastleNav` resource; shared `walk_toward`
  locomotion helper.

Verified (FL_AUTO_FIRE siege, FL_BATTLE_LOG): the trebuchet broke the gate,
the lead element pushed to z=-174 inside the walls, and defenders fell
182 -> ~84 as the courtyard was overrun — no panic. The elevated wall-walk
and tower archers are the last holdouts (towers do their job); fully
clearing them via the spirals is slow and is the next tuning target.


## Entry #10 — 2026-06-13 — Castle defences: gate, voussoirs, stairs, ladders

Playtest gaps fixed across architecture and battle AI.

- **New block shape**: a trapezoidal voussoir (`masonry::spawn_wedge`) —
  a cuboid deformed wider on its outer face — tiles tower rings tightly,
  killing the faceted gaps and reading as hewn-to-fit curves. Walls now
  mix long bond stones and short fillers for stone-size variety.
- **Open fighting platforms**: wall/corner/mural/gatehouse/barbican towers
  lost their cone roofs for stone floor caps + crenellated parapet rings,
  with outward arrowslits up the field-facing arc and a short stair from
  the wall-walk; the great tower keeps its spire.
- **Closed gate**: twin oak leaves + a lowered iron portcullis fill the
  passage — this is what `gate_passage()` detects, so the assault really
  stalls until the player breaches it (confirmed: lead element reaches the
  gate line z=-156 and holds while both sides take casualties).
- **Bailey stairs** climb to the wall-walk.
- **Soldier locomotion** now follows real geometry near the castle: a
  downward ray finds the static surface underfoot (stairs/ramps/wall-walk),
  clamped to a climbable step so a sheer wall can't be scaled without a
  ladder. One mechanic makes stairs, ramps, and walls all walkable.
- **Ladder assault**: one crew per company carries a plank, peels off to an
  assigned wall spot, plants a walkable ladder ramp, and scales the wall to
  fight defenders on the walk.
- Wall archers now fire on the approaching column from 135 m; attacker
  archers volley with no minimum range.

Honest limits: defenders garrison their posts at spawn (they don't path up
the bailey stairs); the demonstrated AI climb is the ladder scale. Tower
interiors are open platforms, not full spiral-stair interiors.


## Entry #9 — 2026-06-13 — The battle at scale (~1,150 soldiers)

Twelve companies of 81 attackers (~970) stage in waves and march the
causeway; defenders densified to ~180 (wall posts every 3 m, four
archers per tower, courtyard reserve ranks). Engineering that made the
5x scale-up free:

- **BattleGrid**: a uniform 6 m spatial grid rebuilt each frame; melee
  engagement, marching separation, and arrow hit-tests all query cells
  instead of scanning every soldier (the old O(N^2) targeting would have
  cost ~4M distance checks per frame at this count).
- Long-range archery targets from a quarter-strided snapshot; defender
  facing scans staggered to one frame in six.
- Marching separation (grid-based push-away) keeps the column from
  collapsing into a blob in the causeway funnel.

Verified: 972/182 spawn, lead element marched z=-82 to the gate line at
-164, then 49 casualties as the gate fight opened. The column reads as a
river of red on the causeway from the trebuchet position.

## Entry #8 — 2026-06-12 — The siege battle + spatial audio fix

- **Audio was silent beyond footsteps**: bevy's default spatial scale
  treats 1 m as 1 audio unit, so a 200 m impact played at ~1/200
  amplitude. Spatial one-shots now use `SpatialScale(0.045)`. (Footsteps
  were the only non-spatial sound — that's why they survived.)
- **Armies** (`soldiers.rs`): ~196 attackers + ~90 defenders as
  lightweight kinematic agents steered over the pure terrain function
  (no character controllers). Defenders man wall-walks, tower tops
  (archers), and courtyard ranks computed from the castle layout;
  attackers stage in companies and march the causeway in loose formation.
  Arrows are visual-ballistic; melee trades synthesized clanks; a war
  horn opens the battle.
- **The player is the decisive force**: the gate holds while masonry
  blocks the passage (live shape-test at the gate point), so the assault
  stalls under defending arrow fire. Trebuchet `BlastEvent`s ragdoll
  everyone in the breach radius, fast debris plows through ranks
  (`CollisionStart` vs the soldiers' kinematic capsules), and when the
  last defender falls: victory horn + "THE CASTLE HAS FALLEN" banner.
- Verified headless via `FL_BATTLE_LOG=1`: lead element advanced
  z=-54 -> -150 (the gate) over ~75 sim-seconds, then casualties began
  (191/89 alive, 6 down); screenshot shows the red column massing at the
  gate. Battle resets with Restart.

## Entry #7 — 2026-06-12 — Trebuchet, expanded fortress, heavy impacts

- Roof cones no longer fracture into four giant cubes: fragments cap at
  ~1.1 m (up to 4 splits/axis) and `ConeShape` pieces skip fragments
  outside the cone volume.
- **Castle expanded** with real fortress conventions: 84x68 m curtain
  (14 m high) with battered plinths, string courses, and machicolation
  collars baked into `wall_run`/`ring_tower`; three mural interval
  towers; a forward barbican with flank walls astride the causeway;
  bigger keep (24 m) and great tower (34 m); gabled great hall; well.
  ~13.5k blocks. Terrace flat radius 66 / blend 112; causeway top at
  z=-124; footprint tests updated.
- **Catapult became a counterweight trebuchet**: 7.2 m pivot atop A-frame
  trusses, 11.8 m effective arm with rigid sling extension, counterweight
  box counter-rotated every frame so it hangs plumb, gravity-paced
  ~0.5 s swing (accel 2+9c rad/s^2), 3 s crank, 1.8 s recrank; 0.75 m
  granite ball (~4.6 t) at 45–72 m/s.
- **Heavy stone-on-stone feel**: impact camera shake (energy-scaled
  trauma joggle in the chase camera) and dust-mote bursts at the impact
  point (no physics bodies, integrated manually, ~1.5 s life).
- Verified: 0.55-charge shot smashed through a gatehouse tower top at
  45 m/s (4.7 MJ, 12+7 shattered), carried on, and hit the keep at
  5.6 MJ. Deferred still: procedural impact audio (next big feel win).

## Entry #6 — 2026-06-12 — Structural integrity overhaul (post-mortem adoption)

Playtest: one hit collapsed the entire castle. Root cause was a positive
feedback loop — any block without direct under-base support fell, falling
columns crushed neighbors' support, repeat until nothing stood. The user
shared a post-mortem from a prior engine build of this concept; adopted
from it:

- **Forgiving archways**: blocks bridged by >=2 adjacent static neighbors
  hold, so breaches leave jagged arches instead of unzipping walls.
- **Volumetric deep anchor**: bridged blocks also need static mass within
  8 m below the base, or the floating chunk falls (no hovering islands).
- **Jenga sleep culling**: debris carries SleepThreshold(0.6/0.7) and
  drops out of the solver quickly once settled.
- **Crush damping**: 5 m/s relative-speed gate and 8 kJ minimum, so
  settling piles stop grinding each other into new collapses.
- **Chiseled stones**: six corner-jittered flat-shaded cube variants
  (rough-hewn look, still instanced).
- **Destruction tally** (scoring.rs): one aggregated "DESTRUCTION +N"
  banner per volley, with running total.

Verified: two 3.9 MJ keep hits + long settle, panorama shows the castle
standing with local damage only.

Deferred post-mortem ideas worth revisiting: procedural audio synthesis
(generated impact thuds), historical castle layout variants, brief
slow-motion on big impacts, free orbit camera, strict object pooling
(fragment budget covers the worst of it today).

## Entry #5 — 2026-06-12 — Catapult overhaul + feedback round

Playtest feedback: catapult never reloaded (the swing substep loop broke
at the release angle, so the arm never reached its stop and the state
machine hung in `Swinging` — also why testing caught it only via the
double-fire headless check). Now: release mid-swing, follow through to
the stop, ~0.5 s reset, auto-reload.

- Catapult ~1.6× bigger (pivot at 4.6 m, 6.3 m arm, 1.8 t stone), spring
  spans ~43–81 m/s tip speed (charge 2.2 s) — wall-base to far-overshoot.
- **Trajectory preview**: gizmo arc + landing marker, live while winding.
- **Cameras**: elevated chase view behind the machine while aiming; after
  loosing, the camera follows the stone downrange until the player clicks
  (which also starts the next wind). Camera override = rewriting the
  camera child's local transform from a desired world pose; restore on
  dismount. Watch for B0001: every `&Transform` query in a system that
  also takes `&mut Transform` on the camera needs `Without<MainCamera>`.
- Emissives cut ~10× across props/cubes/windows/torches (they read like
  suns at ev100 13 with bloom).
- Trees v3: mixed forest — pines with tapered trunks and four jittered
  canopy tiers, broadleaf oaks from clustered ico-spheres; smoother rocks.
- Ground bake: cavity shading (curvature-based), grass-blade speckle,
  dirt patches.
- Verified headless: double auto-fire (reload), low-charge direct hit on
  the gate battlements at 43 m/s / 1.68 MJ — shatter, punch-through,
  secondary impacts, second stone loosed.

## Entry #4 — 2026-06-12 — Per-stone damage, cracking, and fracture

Stones now take damage and break apart, giving destruction depth beyond
support collapse:

- `MasonryBlock` carries integrity in joules (volume × toughness; granite
  55 kJ/m³, slate 35, wood 25). At 50% blocks visibly crack (material
  swap to a fissured texture); at 0 they **fracture into 4–12 dynamic
  fragments** that inherit velocity plus radial spray.
- **Contact-brunt model**: the directly struck block takes 55% of a
  projectile's kinetic energy (the collision event identifies it), 30%
  radiates into the radius with falloff, 15% lost. Mid-ring stones break
  loose (mortar failure), the edge cracks in place.
- **Crush damage**: hard dynamic-vs-block collisions damage both sides
  (relative KE × 0.4, split) — collapsing masonry pulverizes what it
  lands on; chain reactions are emergent.
- Performance bounds: fragments are terminal (never re-fracture); oldest
  sleeping rubble recycled above 1,500 pieces; collision events only on
  dynamic bodies; damage events capped per tick; support checks amortized.

### Bugs that mattered

- **`CollisionStart` arrives after the solver** — reading `LinearVelocity`
  in a handler gives the *rebound* speed (~8× energy underestimate).
  Damage-dealers now cache velocity each tick before the physics step
  (`PreTickVelocity`); note the cache system must be ordered against
  `PhysicsSystems` or the scheduler may interleave.
- One landing fires one event per touching block — projectiles dedup per
  tick or breach energy multiplies.
- Bevy `Bundle` tuples cap at 15 elements; nest sub-tuples.
- Siege geometry is real: from 200 m with a 40° launch the stone plunges
  steeply — it crushes courtyards (and the keep) rather than breaching
  wall faces. Wall-face breaching needs flatter, closer shots. Verified:
  plunging hit at 20 m/s shatters its contact stone in the keep wall;
  `FL_AUTO_FIRE=<frame>[:<charge>]` drives headless siege tests.

### Open items

- Catapult stones are indestructible; making them shatter on hard impacts
  would be a nice touch.
- A movable/redeployable catapult would enable flat-trajectory breaching
  shots against the curtain wall face.

## Entry #3 — 2026-06-12 — Destructible castle, catapult, fidelity pass

Playtest feedback fixed first: the causeway's smoothstep profile peaked at
~35° (above the 30° climb limit) and its carve region undercut the
gatehouse — that was the "floating castle". Now: linear ~24° grade
topping out at the terrace edge, terrace lowered to 44 m, climb limit
raised to 40°, and the project's first unit tests pin all of it
(causeway grade, castle footings, playground flatness — `cargo test`).

### Destructible masonry (the headline)

- Castle rebuilt from **~10k individual mortared stone blocks**
  (`masonry.rs` + `castle.rs`): walls with alternating courses, ring
  towers from tangent blocks, keep with solid corner piers, merlons,
  emissive window blocks. All start `RigidBody::Static`.
- **Impact waking**: `CollisionStart` events from `Projectile`s (thrown
  cubes, catapult stones) wake static blocks within a breach radius
  scaled by kinetic energy (`(E/30k)^(1/3) * 2`, clamped 0.9–5 m).
- **Support cascade**: woken blocks queue their neighbors; queued static
  blocks shape-cast a thin box under their base and wake if nothing
  static holds them up (48 checks/tick, amortized). Walls cave
  progressively; roof cones are single rigid pieces that topple.
- Verified end-to-end headless: auto-fired stone at 46 m/s, impact log
  shows breach r=2.6 m waking 21 blocks at the front wall.

### Catapult (`catapult.rs`)

E to man, view-yaw slews the aim, hold LMB winds (charge), release
looses. Kinematic arm with **substepped swing integration** (4 ms) —
naive per-frame Euler released at 62 m/s under llvmpipe's 0.25 s frames.
Stone: 0.45 m granite sphere (~1 t), `SweptCcd`, fully dynamic from
release with arm-tip velocity. Full charge ≈ 49 m/s reaches the gate
uphill at ~200 m. Auto-reload; `FL_AUTO_FIRE=<frame>` fires headless.

### Physics realism

`SubstepCount(8)`; per-material `Friction`/`Restitution`/
`ColliderDensity` everywhere (granite 2600, masonry 2200, wood 450–600,
metal props 2700 & slick); projectiles use swept CCD.

### Visual fidelity

- **SSAO** (`ScreenSpaceAmbientOcclusion`) + **volumetric god rays**
  (`VolumetricFog` on camera, `VolumetricLight` on the sun, a thin
  `FogVolume` hugging the valley floor).
- **Terrain albedo megatexture**: 1024² baked ground-color (~0.6 m/texel
  + micro-variation) replaces vertex colors.
- Masonry grain texture + 6 stone tints; warm sun color; tapered leaning
  pines (380, 6 canopy tints); 6 torches (emissive flame + point light)
  at gate/courtyard/keep.

### Open issues

- The masonry support model has no lateral bridging (a lintel over a gap
  survives only until disturbed) — acceptable, reads as mortar failure.
- llvmpipe renders the 10k-block scene at ~1 FPS (stills only); real
  performance needs the 5090 playtest.
- Catapult arm is kinematic by design (reliability); the stone and all
  destruction are fully dynamic.

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
