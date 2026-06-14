//! A mannable counterweight trebuchet.
//!
//! Walk up (E to man it), aim by looking — the machine slews to follow
//! your view and a trajectory arc shows where the stone will land,
//! updating as you crank. Hold left click to wind, release to loose; the
//! camera follows the shot downrange until you click again. The beam
//! swing is kinematic with substepped integration (frame-rate-independent
//! release speed), paced like a gravity-driven counterweight machine —
//! slow, ponderous, enormous. The counterweight box hangs plumb from the
//! short arm throughout the swing. The stone is a fully dynamic
//! ~4.6-tonne granite sphere with continuous collision detection; heavy
//! impacts feed camera shake and dust (`masonry.rs`).

use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use super::audio::{SoundEvent, SoundKind};
use super::masonry::{BrittleStone, PreTickVelocity, Projectile};
use super::terrain::{CASTLE_CENTER, KNOLL_CENTER, terrain_height};
use super::world::Respawnable;
use engine::prelude::*;

pub struct TrebuchetPlugin;

impl Plugin for TrebuchetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Manning>()
            .init_resource::<ShotCamera>()
            .add_systems(Startup, (setup_assets, spawn_trebuchet).chain());

        // Headless verification: `FL_AUTO_MAN=<frame>` mans the trebuchet so
        // screenshots show the aiming view, trajectory arc, and chase camera.
        if let Some(at_frame) = std::env::var("FL_AUTO_MAN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.add_systems(
                Update,
                move |mut frame: Local<u32>,
                      mut manning: ResMut<Manning>,
                      trebuchets: Query<Entity, With<Trebuchet>>| {
                    *frame += 1;
                    if *frame == at_frame
                        && let Some(trebuchet) = trebuchets.iter().next()
                    {
                        manning.0 = Some(trebuchet);
                    }
                },
            );
        }

        // Headless verification: `FL_AUTO_FIRE=<frame>[:<charge>]`.
        if let Ok(var) = std::env::var("FL_AUTO_FIRE") {
            let (frame_str, charge_str) = var.split_once(':').unwrap_or((var.as_str(), "1.0"));
            let charge: f32 = charge_str.parse().unwrap_or(1.0);
            if let Ok(at_frame) = frame_str.parse::<u32>() {
                app.add_systems(
                    Update,
                    move |mut frame: Local<u32>, mut trebuchets: Query<&mut Trebuchet>| {
                        *frame += 1;
                        // Fire twice: the second shot proves the reload.
                        if *frame == at_frame || *frame == at_frame + 45 {
                            for mut trebuchet in &mut trebuchets {
                                trebuchet.phase = Phase::Swinging { released: false };
                                trebuchet.charge = charge;
                                trebuchet.angular_velocity = 0.0;
                            }
                        }
                    },
                );
            }
        }
        app.add_systems(
            Update,
            (
                man_toggle,
                aim,
                wind_and_loose,
                seat_stone,
                trajectory_preview,
                trebuchet_camera,
                hint_text,
            )
                .chain()
                .run_if(in_state(MenuState::Closed)),
        );
    }
}

/// Which trebuchet the player is manning, if any. Checked by the throw
/// system so left click doesn't also hurl cubes.
#[derive(Resource, Default)]
pub struct Manning(pub Option<Entity>);

/// While a shot is in the air the camera chases it; clicking returns to
/// aiming (and starts the next wind). Near the castle the camera stops
/// advancing and watches the destruction from a held position, lingering
/// a few seconds after the stone comes to rest.
#[derive(Resource, Default)]
struct ShotCamera {
    following: Option<Entity>,
    held_eye: Option<Vec3>,
    rest_timer: f32,
    /// Last known stone position, so the camera can keep watching the spot
    /// for a beat if the stone shatters (its entity despawns) on impact.
    last_pos: Vec3,
}

/// Root entity (kinematic body; carries the frame colliders).
#[derive(Component)]
struct Trebuchet {
    phase: Phase,
    charge: f32,
    /// Winch-creak cadence accumulator while winding.
    creak: f32,
    /// Current arm angle in radians (see [`ARM_COCKED`]).
    angle: f32,
    /// Angular velocity during the swing (rad/s, negative = firing).
    angular_velocity: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Cocked and loaded, waiting.
    Ready,
    /// Player holding the wind (charge grows).
    Winding,
    /// Arm swinging; the stone releases at [`ARM_RELEASE`].
    Swinging { released: bool },
    /// Arm returning to cocked; reloads when done.
    Resetting,
}

/// The rotating beam (child of the root, pivot at the axle).
#[derive(Component)]
struct TrebuchetArm;

/// The counterweight hanger (child of the arm): counter-rotated every
/// frame so the weight box hangs plumb while the beam swings.
#[derive(Component)]
struct CounterweightHanger;

/// The loaded stone (kinematic while seated in the spoon).
#[derive(Component)]
struct SeatedStone {
    trebuchet: Entity,
}

/// Arm angles, radians, rotation about the arm's local X axis. The spoon
/// arm extends +Z (the trebuchet's rear); negative rotation swings it up
/// and over toward the front (-Z, where the castle is).
const ARM_COCKED: f32 = 0.55; // long arm low at the rear, weight raised
const ARM_RELEASE: f32 = -0.80; // sling whips the stone out ~42 degrees up
const ARM_STOP: f32 = -1.45;
/// Extra wind-back at full charge.
const WIND_BACK: f32 = 0.18;
/// Beam pivot in the trebuchet's local space (top of the A-frames).
const PIVOT: Vec3 = Vec3::new(0.0, 7.2, 0.4);
/// Long-arm length to the sling attachment.
const ARM_LENGTH: f32 = 9.0;
/// Stone seat: arm tip plus the (rigidly modeled) sling extension.
const SEAT: Vec3 = Vec3::new(0.0, 0.55, ARM_LENGTH + 2.8);
/// Short arm to the counterweight hanger.
const SHORT_ARM: f32 = 2.8;
const STONE_RADIUS: f32 = 0.75;
/// Granite.
const STONE_DENSITY: f32 = 2600.0;
/// Joules of impact the stone can shed before it breaks apart. Tuned so a
/// full-power head-on hit on a thick, intact wall shatters it, a moderate
/// hit only cracks it, and field landings or glancing blows leave it whole.
const STONE_INTEGRITY: f32 = 800_000.0;
/// Effective angular acceleration (rad/s^2) by charge. Deliberately low:
/// a counterweight machine accelerates ponderously (~0.5 s swing), the
/// speed comes from the enormous 11.8 m effective arm. ~45–72 m/s.
fn spring_acceleration(charge: f32) -> f32 {
    2.0 + 9.0 * charge
}
/// Seconds of held click for full charge (cranking the weight up).
const WIND_TIME: f32 = 3.0;

/// The trebuchet stands on the siege knoll, overlooking the castle.
const POSITION: Vec2 = KNOLL_CENTER;

#[derive(Resource)]
struct TrebuchetAssets {
    wood: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    stone_material: Handle<StandardMaterial>,
    stone_cracked: Handle<StandardMaterial>,
    cube: Handle<Mesh>,
    wheel: Handle<Mesh>,
    stone_mesh: Handle<Mesh>,
}

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TrebuchetAssets {
        wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.30, 0.17),
            perceptual_roughness: 0.85,
            ..default()
        }),
        dark_wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.19, 0.11),
            perceptual_roughness: 0.9,
            ..default()
        }),
        iron: materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.25, 0.28),
            metallic: 0.9,
            perceptual_roughness: 0.45,
            ..default()
        }),
        stone_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.44, 0.42),
            perceptual_roughness: 0.9,
            ..default()
        }),
        // Darker, rougher, fissured granite for a cracked or shattered stone.
        stone_cracked: materials.add(StandardMaterial {
            base_color: Color::srgb(0.32, 0.30, 0.28),
            perceptual_roughness: 0.98,
            ..default()
        }),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        wheel: meshes.add(Cylinder::new(0.85, 0.45)),
        stone_mesh: meshes.add(Sphere::new(STONE_RADIUS)),
    });
}

fn spawn_trebuchet(mut commands: Commands, assets: Res<TrebuchetAssets>) {
    let y = terrain_height(POSITION.x, POSITION.y);
    let to_castle = Vec2::new(0.0, -166.0) - POSITION;
    let yaw = (-to_castle.x).atan2(-to_castle.y);

    let root = commands
        .spawn((
            Trebuchet {
                phase: Phase::Ready,
                charge: 0.0,
                creak: 0.0,
                angle: ARM_COCKED,
                angular_velocity: 0.0,
            },
            Transform::from_xyz(POSITION.x, y, POSITION.y)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();

    let arm = commands
        .spawn((
            TrebuchetArm,
            ChildOf(root),
            Transform::from_translation(PIVOT).with_rotation(Quat::from_rotation_x(ARM_COCKED)),
            Visibility::default(),
        ))
        .id();

    // Frame: long deck, two A-frame trusses, axle, crossbraces.
    let parts: &[(Vec3, Vec3, Quat, bool, &Handle<StandardMaterial>)] = &[
        // deck
        (Vec3::new(0.0, 0.9, 0.5), Vec3::new(6.0, 0.8, 11.0), Quat::IDENTITY, true, &assets.wood),
        // A-frame legs (two slanted pairs)
        (Vec3::new(-2.3, 4.05, -1.9), Vec3::new(0.55, 7.4, 0.7), Quat::from_rotation_x(-0.33), true, &assets.dark_wood),
        (Vec3::new(2.3, 4.05, -1.9), Vec3::new(0.55, 7.4, 0.7), Quat::from_rotation_x(-0.33), true, &assets.dark_wood),
        (Vec3::new(-2.3, 4.05, 2.8), Vec3::new(0.55, 7.4, 0.7), Quat::from_rotation_x(0.33), true, &assets.dark_wood),
        (Vec3::new(2.3, 4.05, 2.8), Vec3::new(0.55, 7.4, 0.7), Quat::from_rotation_x(0.33), true, &assets.dark_wood),
        // crossbraces between the trusses
        (Vec3::new(0.0, 5.4, -2.4), Vec3::new(4.6, 0.4, 0.4), Quat::IDENTITY, false, &assets.dark_wood),
        (Vec3::new(0.0, 5.4, 3.3), Vec3::new(4.6, 0.4, 0.4), Quat::IDENTITY, false, &assets.dark_wood),
        // axle through the apex
        (PIVOT, Vec3::new(5.2, 0.4, 0.4), Quat::IDENTITY, false, &assets.iron),
    ];
    for (pos, size, rot, collide, material) in parts {
        let mut part = commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d((*material).clone()),
            Transform::from_translation(*pos).with_rotation(*rot).with_scale(*size),
            ChildOf(root),
        ));
        if *collide {
            part.insert(Collider::cuboid(1.0, 1.0, 1.0));
        }
    }
    // Wheels.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        commands.spawn((
            Mesh3d(assets.wheel.clone()),
            MeshMaterial3d(assets.dark_wood.clone()),
            Transform::from_xyz(sx * 3.2, 0.9, sz * 4.2)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ChildOf(root),
        ));
    }

    // Beam: tapered main spar, sling extension, pouch.
    for (pos, size, material) in [
        (Vec3::new(0.0, 0.0, (ARM_LENGTH - SHORT_ARM) / 2.0), Vec3::new(0.6, 0.55, ARM_LENGTH + SHORT_ARM), &assets.wood),
        // sling (rendered rigid)
        (Vec3::new(0.0, 0.3, ARM_LENGTH + 1.4), Vec3::new(0.12, 0.12, 2.9), &assets.dark_wood),
        // pouch under the seat
        (Vec3::new(0.0, 0.25, ARM_LENGTH + 2.8), Vec3::new(1.3, 0.3, 1.4), &assets.dark_wood),
    ] {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos).with_scale(size),
            ChildOf(arm),
        ));
    }
    // Counterweight: hanger pivot at the short-arm end; the box swings
    // plumb beneath it.
    let hanger = commands
        .spawn((
            CounterweightHanger,
            ChildOf(arm),
            Transform::from_xyz(0.0, 0.0, -SHORT_ARM)
                .with_rotation(Quat::from_rotation_x(-ARM_COCKED)),
            Visibility::default(),
        ))
        .id();
    for (pos, size, material) in [
        (Vec3::new(0.0, -1.3, 0.0), Vec3::new(0.35, 2.2, 0.35), &assets.iron),
        (Vec3::new(0.0, -3.5, 0.0), Vec3::new(3.0, 2.6, 3.0), &assets.iron),
    ] {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos).with_scale(size),
            ChildOf(hanger),
        ));
    }

    load_stone(&mut commands, &assets, root);
}

/// Spawns a fresh stone seated in the spoon.
fn load_stone(commands: &mut Commands, assets: &TrebuchetAssets, trebuchet: Entity) {
    commands.spawn((
        SeatedStone { trebuchet },
        Mesh3d(assets.stone_mesh.clone()),
        MeshMaterial3d(assets.stone_material.clone()),
        Transform::from_xyz(0.0, -1000.0, 0.0), // placed by `seat_stone`
        RigidBody::Kinematic,
        Respawnable,
    ));
}

/// Keeps the seated stone in the spoon (world-space, follows arm swing).
fn seat_stone(
    trebuchets: Query<(&Transform, &Trebuchet), Without<SeatedStone>>,
    mut stones: Query<(&SeatedStone, &mut Transform)>,
) {
    for (seat, mut transform) in &mut stones {
        let Ok((root, trebuchet)) = trebuchets.get(seat.trebuchet) else {
            continue;
        };
        let arm_rot = Quat::from_rotation_x(trebuchet.angle);
        let local = PIVOT + arm_rot * SEAT;
        transform.translation = root.translation + root.rotation * local;
        transform.rotation = root.rotation;
    }
}

const MAN_RANGE: f32 = 7.0;

fn man_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut manning: ResMut<Manning>,
    mut shot_camera: ResMut<ShotCamera>,
    players: Query<&Transform, With<Player>>,
    trebuchets: Query<(Entity, &Transform), With<Trebuchet>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    if !keys.just_pressed(KeyCode::KeyE) {
        if let Some(active) = manning.0
            && let Ok((_, root)) = trebuchets.get(active)
            && root.translation.distance(player.translation) > MAN_RANGE + 2.0
        {
            manning.0 = None;
            shot_camera.following = None;
        }
        return;
    }
    if manning.0.is_some() {
        manning.0 = None;
        shot_camera.following = None;
        return;
    }
    for (entity, root) in &trebuchets {
        if root.translation.distance(player.translation) <= MAN_RANGE {
            manning.0 = Some(entity);
            return;
        }
    }
}

/// While manned, the trebuchet slews to follow the player's view yaw.
fn aim(
    time: Res<Time>,
    manning: Res<Manning>,
    players: Query<&Transform, (With<Player>, Without<Trebuchet>)>,
    mut trebuchets: Query<&mut Transform, With<Trebuchet>>,
) {
    let (Some(active), Ok(player)) = (manning.0, players.single()) else {
        return;
    };
    let Ok(mut root) = trebuchets.get_mut(active) else {
        return;
    };
    let (player_yaw, ..) = player.rotation.to_euler(EulerRot::YXZ);
    let (current, ..) = root.rotation.to_euler(EulerRot::YXZ);
    let mut delta = player_yaw - current;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    let max_step = 1.2 * time.delta_secs();
    root.rotation = Quat::from_rotation_y(current + delta.clamp(-max_step, max_step));
}

/// World-space release point and velocity for a given charge. Used by both
/// the trajectory preview and the actual release, so the arc is exact: the
/// stone leaves from this pose with v = omega x r at the seat point.
fn release_state(root: &Transform, charge: f32) -> (Vec3, Vec3) {
    let sweep = (ARM_COCKED + charge * WIND_BACK) - ARM_RELEASE;
    let angular_velocity = -(2.0 * spring_acceleration(charge) * sweep).sqrt();
    let arm_rot = Quat::from_rotation_x(ARM_RELEASE);
    let r = arm_rot * SEAT;
    let position = root.translation + root.rotation * (PIVOT + r);
    let velocity = root.rotation * (Vec3::X * angular_velocity).cross(r);
    (position, velocity)
}

/// Wind-up, release, swing, stone hand-off, reset, reload.
fn wind_and_loose(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    manning: Res<Manning>,
    mut shot_camera: ResMut<ShotCamera>,
    mut sounds: MessageWriter<SoundEvent>,
    assets: Res<TrebuchetAssets>,
    mut trebuchets: Query<(Entity, &Transform, &mut Trebuchet)>,
    mut arms: Query<(&ChildOf, &mut Transform), (With<TrebuchetArm>, Without<Trebuchet>, Without<CounterweightHanger>)>,
    mut hangers: Query<(&ChildOf, &mut Transform), (With<CounterweightHanger>, Without<Trebuchet>, Without<TrebuchetArm>)>,
    stones: Query<(Entity, &SeatedStone), Without<Trebuchet>>,
) {
    let dt = time.delta_secs();
    for (entity, root, mut trebuchet) in &mut trebuchets {
        let manned = manning.0 == Some(entity);
        // A click while watching the previous shot returns to aiming
        // (right-click works too and never starts a wind).
        let watching = manned && shot_camera.following.is_some();
        if watching
            && (buttons.just_pressed(MouseButton::Left)
                || buttons.just_pressed(MouseButton::Right))
        {
            shot_camera.following = None;
            shot_camera.held_eye = None;
            shot_camera.rest_timer = 0.0;
        }

        match trebuchet.phase {
            Phase::Ready => {
                if manned && !watching && buttons.pressed(MouseButton::Left) {
                    trebuchet.phase = Phase::Winding;
                    trebuchet.charge = 0.25;
                }
            }
            Phase::Winding => {
                trebuchet.charge = (trebuchet.charge + dt / WIND_TIME).min(1.0);
                trebuchet.angle = ARM_COCKED + trebuchet.charge * WIND_BACK;
                trebuchet.creak += dt;
                if trebuchet.creak > 0.38 {
                    trebuchet.creak = 0.0;
                    sounds.write(SoundEvent {
                        kind: SoundKind::Creak,
                        position: root.translation + Vec3::Y * 5.0,
                        intensity: 0.8,
                    });
                }
                if !manned || !buttons.pressed(MouseButton::Left) {
                    trebuchet.phase = Phase::Swinging { released: false };
                    trebuchet.angular_velocity = 0.0;
                }
            }
            Phase::Swinging { mut released } => {
                // Substepped so release speed is frame-rate independent.
                let acceleration = spring_acceleration(trebuchet.charge);
                let substeps = (dt / 0.004).ceil().max(1.0) as u32;
                let sub_dt = dt / substeps as f32;
                for _ in 0..substeps {
                    if trebuchet.angle <= ARM_STOP {
                        break;
                    }
                    trebuchet.angular_velocity -= acceleration * sub_dt;
                    let previous = trebuchet.angle;
                    trebuchet.angle += trebuchet.angular_velocity * sub_dt;

                    if !released && previous > ARM_RELEASE && trebuchet.angle <= ARM_RELEASE {
                        released = true;
                        // Snap the stone to the exact release pose so the
                        // flight matches the previewed arc at any frame rate.
                        let (position, velocity) = release_state(root, trebuchet.charge);
                        for (stone_entity, seat) in &stones {
                            if seat.trebuchet != entity {
                                continue;
                            }
                            commands.entity(stone_entity).remove::<SeatedStone>().insert((
                                Transform::from_translation(position)
                                    .with_rotation(root.rotation),
                                (
                                    RigidBody::Dynamic,
                                    Collider::sphere(STONE_RADIUS),
                                    ColliderDensity(STONE_DENSITY),
                                    Friction::new(0.7),
                                    Restitution::new(0.1),
                                    SweptCcd::default(),
                                ),
                                Projectile,
                                BrittleStone {
                                    integrity: STONE_INTEGRITY,
                                    max_integrity: STONE_INTEGRITY,
                                    radius: STONE_RADIUS,
                                    cracked: assets.stone_cracked.clone(),
                                    cracked_applied: false,
                                },
                                CollisionEventsEnabled,
                                PreTickVelocity(velocity),
                                TransformInterpolation,
                                LinearVelocity(velocity.adjust_precision()),
                            ));
                            if manned {
                                shot_camera.following = Some(stone_entity);
                                shot_camera.held_eye = None;
                                shot_camera.rest_timer = 0.0;
                            }
                            sounds.write(SoundEvent {
                                kind: SoundKind::Whoosh,
                                position: root.translation + Vec3::Y * 8.0,
                                intensity: 0.5 + trebuchet.charge * 0.5,
                            });
                            info!("trebuchet: loosed stone at {:.1} m/s", velocity.length());
                        }
                    }
                }
                if trebuchet.angle <= ARM_STOP {
                    trebuchet.angle = ARM_STOP;
                    trebuchet.phase = Phase::Resetting;
                    sounds.write(SoundEvent {
                        kind: SoundKind::FrameThunk,
                        position: root.translation + Vec3::Y * 4.0,
                        intensity: 0.6 + trebuchet.charge * 0.4,
                    });
                } else {
                    trebuchet.phase = Phase::Swinging { released };
                }
            }
            Phase::Resetting => {
                // Heavy recrank: about 1.8 s to haul the weight back up.
                trebuchet.angle += 1.15 * dt;
                if trebuchet.angle >= ARM_COCKED {
                    trebuchet.angle = ARM_COCKED;
                    trebuchet.charge = 0.0;
                    trebuchet.phase = Phase::Ready;
                    load_stone(&mut commands, &assets, entity);
                }
            }
        }

        for (parent, mut arm_transform) in &mut arms {
            if parent.parent() == entity {
                arm_transform.rotation = Quat::from_rotation_x(trebuchet.angle);
            }
        }
        for (parent, mut hanger_transform) in &mut hangers {
            if arms.get(parent.parent()).is_ok_and(|(p, _)| p.parent() == entity) {
                hanger_transform.rotation = Quat::from_rotation_x(-trebuchet.angle);
            }
        }
    }
}

/// Dotted arc from the spoon to the predicted landing point, live while
/// aiming and winding.
fn trajectory_preview(
    mut gizmos: Gizmos,
    manning: Res<Manning>,
    shot_camera: Res<ShotCamera>,
    trebuchets: Query<(&Transform, &Trebuchet)>,
) {
    let Some(active) = manning.0 else {
        return;
    };
    if shot_camera.following.is_some() {
        return;
    }
    let Ok((root, trebuchet)) = trebuchets.get(active) else {
        return;
    };
    let charge = match trebuchet.phase {
        Phase::Winding => trebuchet.charge,
        _ => 0.25,
    };
    let (mut position, mut velocity) = release_state(root, charge);

    let mut points = vec![position];
    for _ in 0..400 {
        velocity.y -= 9.81 * 0.05;
        position += velocity * 0.05;
        points.push(position);
        if position.y < terrain_height(position.x, position.z) {
            break;
        }
    }
    let intensity = 0.4 + charge * 0.6;
    gizmos.linestrip(points.iter().copied(), Color::srgb(1.0, 0.85 * intensity, 0.2));
    if let Some(landing) = points.last() {
        gizmos.sphere(Isometry3d::from_translation(*landing), 1.2, Color::srgb(1.0, 0.3, 0.15));
    }
}

/// While manning: an elevated chase view behind the trebuchet; after firing
/// the camera follows the stone downrange until the player clicks.
fn trebuchet_camera(
    manning: Res<Manning>,
    mut shot_camera: ResMut<ShotCamera>,
    mut shake: ResMut<super::masonry::ImpactShake>,
    time: Res<Time>,
    trebuchets: Query<&Transform, (With<Trebuchet>, Without<MainCamera>)>,
    players: Query<&Transform, (With<Player>, Without<Trebuchet>, Without<MainCamera>)>,
    stones: Query<(&Transform, Option<&LinearVelocity>), (With<Projectile>, Without<Player>, Without<Trebuchet>, Without<MainCamera>)>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
    mut was_overriding: Local<bool>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let desired: Option<(Vec3, Vec3)> = if let Some(active) = manning.0 {
        if let Some(stone) = shot_camera.following {
            match stones.get(stone) {
                Ok((stone_transform, velocity)) => {
                    let stone_pos = stone_transform.translation;
                    shot_camera.last_pos = stone_pos;
                    // Linger after the stone slows so the collapse plays
                    // out; the timer never resets (rubble nudging the ball
                    // must not re-capture the camera).
                    let speed = velocity.map(|v| v.length()).unwrap_or(0.0);
                    if speed < 2.5 || shot_camera.rest_timer > 0.0 {
                        shot_camera.rest_timer += time.delta_secs();
                        if shot_camera.rest_timer > 4.0 {
                            shot_camera.following = None;
                            shot_camera.held_eye = None;
                            shot_camera.rest_timer = 0.0;
                        }
                    }

                    // Hold position approaching the castle: watch the hit
                    // from outside instead of riding into the wall.
                    let near_castle = Vec2::new(stone_pos.x, stone_pos.z)
                        .distance(CASTLE_CENTER)
                        < 85.0;
                    let chase_eye = stone_pos
                        + velocity
                            .map(|v| Vec3::new(v.x, 0.0, v.z).normalize_or_zero())
                            .unwrap_or(Vec3::Z)
                            * -14.0
                        + Vec3::Y * 6.0;
                    let eye = if near_castle {
                        *shot_camera.held_eye.get_or_insert(chase_eye)
                    } else {
                        shot_camera.held_eye = None;
                        chase_eye
                    };
                    Some((eye, stone_pos))
                }
                Err(_) => {
                    // The stone is gone — most likely it shattered on a hard
                    // hit. Hold on the impact spot for a beat so the break and
                    // any collapse register, then release back to aiming.
                    shot_camera.rest_timer += time.delta_secs();
                    if shot_camera.rest_timer > 3.0 {
                        shot_camera.following = None;
                        shot_camera.held_eye = None;
                        shot_camera.rest_timer = 0.0;
                        None
                    } else {
                        let target = shot_camera.last_pos;
                        let eye = shot_camera
                            .held_eye
                            .unwrap_or(target + Vec3::new(0.0, 7.0, 16.0));
                        Some((eye, target))
                    }
                }
            }
        } else if let Ok(root) = trebuchets.get(active) {
            // Behind, above, and a step to the side so the trajectory arc
            // reads as a curve instead of an edge-on line.
            let eye = root.translation + root.rotation * Vec3::new(4.5, 10.0, 17.0);
            let target = root.translation + root.rotation * Vec3::new(0.0, 4.0, -30.0);
            Some((eye, target))
        } else {
            None
        }
    } else {
        None
    };

    // Impact trauma decays; while overriding, it joggles the camera —
    // stone-on-stone should be felt.
    shake.0 = (shake.0 - time.delta_secs() * 0.9).max(0.0);
    let t = time.elapsed_secs();
    let joggle = Vec3::new(
        (t * 47.0).sin(),
        (t * 53.0 + 1.7).sin(),
        (t * 41.0 + 3.1).sin(),
    ) * shake.0
        * shake.0
        * 0.5;

    match desired {
        Some((eye, target)) => {
            // Convert the desired world pose into the camera's local frame
            // (it is a child of the player).
            let world = Transform::from_translation(eye + joggle).looking_at(target, Vec3::Y);
            let inv = player.rotation.inverse();
            let goal_translation = inv * (world.translation - player.translation);
            let goal_rotation = inv * world.rotation;
            let blend = (time.delta_secs() * 5.0).min(1.0);
            camera.translation = camera.translation.lerp(goal_translation, blend);
            camera.rotation = camera.rotation.slerp(goal_rotation, blend);
            *was_overriding = true;
        }
        None => {
            if *was_overriding {
                // Back to first-person eye height.
                camera.translation = Vec3::new(0.0, 0.6, 0.0);
                camera.rotation = Quat::IDENTITY;
                *was_overriding = false;
            }
        }
    }
}

#[derive(Component)]
struct HintText;

/// Bottom-center prompt when near or manning the trebuchet.
fn hint_text(
    mut commands: Commands,
    manning: Res<Manning>,
    shot_camera: Res<ShotCamera>,
    players: Query<&Transform, With<Player>>,
    trebuchets: Query<&Transform, With<Trebuchet>>,
    mut hints: Query<(Entity, &mut Text), With<HintText>>,
) {
    let message = if manning.0.is_some() {
        if shot_camera.following.is_some() {
            "Click to wind the next shot — E to step off"
        } else {
            "Aim with the mouse — hold Left Click to wind, release to loose — E to step off"
        }
    } else if players.single().is_ok_and(|player| {
        trebuchets
            .iter()
            .any(|c| c.translation.distance(player.translation) <= MAN_RANGE)
    }) {
        "E — man the trebuchet"
    } else {
        ""
    };

    match hints.single_mut() {
        Ok((entity, mut text)) => {
            if message.is_empty() {
                commands.entity(entity).despawn();
            } else if text.0 != message {
                text.0 = message.to_string();
            }
        }
        Err(_) if !message.is_empty() => {
            commands.spawn((
                HintText,
                Text::new(message),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.92, 0.8)),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(48),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ));
        }
        _ => {}
    }
}
