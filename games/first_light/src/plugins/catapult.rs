//! A mannable catapult for sieging the castle.
//!
//! Walk up (E to man it), aim by looking — the catapult slews to follow
//! your view — hold left click to wind the arm, release to loose. The arm
//! swing is kinematic (reliable and tunable); the stone is a fully dynamic
//! ~1-tonne granite sphere released with the arm-tip velocity, flying with
//! continuous collision detection so it never tunnels through walls. The
//! masonry system (`masonry.rs`) handles what happens when it lands.

use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use super::masonry::Projectile;
use super::terrain::terrain_height;
use super::world::Respawnable;
use engine::prelude::*;

pub struct CatapultPlugin;

impl Plugin for CatapultPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Manning>()
            .add_systems(Startup, (setup_assets, spawn_catapult).chain());

        // Headless verification: `FL_AUTO_FIRE=<frame>[:<charge>]` fires at
        // the given frame with the given charge (default full).
        if let Ok(var) = std::env::var("FL_AUTO_FIRE") {
            let (frame_str, charge_str) = var.split_once(':').unwrap_or((var.as_str(), "1.0"));
            let charge: f32 = charge_str.parse().unwrap_or(1.0);
            if let Ok(at_frame) = frame_str.parse::<u32>() {
                app.add_systems(
                    Update,
                    move |mut frame: Local<u32>, mut catapults: Query<&mut Catapult>| {
                        *frame += 1;
                        if *frame == at_frame {
                            for mut catapult in &mut catapults {
                                catapult.phase = Phase::Swinging;
                                catapult.charge = charge;
                                catapult.angular_velocity = 0.0;
                            }
                        }
                    },
                );
            }
        }
        app
            .add_systems(
                Update,
                (man_toggle, aim, wind_and_loose, seat_stone, hint_text)
                    .chain()
                    .run_if(in_state(MenuState::Closed)),
            );
    }
}

/// Which catapult the player is manning, if any. Checked by the throw
/// system so left click doesn't also hurl cubes.
#[derive(Resource, Default)]
pub struct Manning(pub Option<Entity>);

/// Root entity (kinematic body; carries the frame colliders).
#[derive(Component)]
struct Catapult {
    phase: Phase,
    charge: f32,
    /// Current arm angle in radians (see [`ARM_COCKED`]).
    angle: f32,
    /// Angular velocity during the swing (rad/s, negative = firing).
    angular_velocity: f32,
}

enum Phase {
    /// Cocked and loaded, waiting.
    Ready,
    /// Player holding the wind (charge grows).
    Winding,
    /// Arm swinging; stone releases at [`ARM_RELEASE`].
    Swinging,
    /// Arm returning to cocked; reloads when done.
    Resetting,
}

/// The rotating arm (child of the root, pivot at the axle).
#[derive(Component)]
struct CatapultArm;

/// The loaded stone (kinematic while seated in the spoon).
#[derive(Component)]
struct SeatedStone {
    catapult: Entity,
}

/// Arm angles, radians, rotation about the arm's local X axis. The spoon
/// arm extends +Z (the catapult's rear); negative rotation swings it up
/// and over toward the front (-Z, where the castle is).
const ARM_COCKED: f32 = 0.17; // ~10 deg below horizontal, spoon at the rear
const ARM_RELEASE: f32 = -0.87; // ~50 deg past vertical start: stone flies ~50 deg up
const ARM_STOP: f32 = -1.31; // padded stop ~75 deg
/// Spoon distance from the pivot.
const TIP_RADIUS: f32 = 4.0;
const STONE_RADIUS: f32 = 0.45;
/// Granite.
const STONE_DENSITY: f32 = 2600.0;

/// Where the catapult stands (meadow edge, clear shot at the castle).
const POSITION: Vec2 = Vec2::new(16.0, 34.0);

#[derive(Resource)]
struct CatapultAssets {
    wood: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    stone_material: Handle<StandardMaterial>,
    cube: Handle<Mesh>,
    wheel: Handle<Mesh>,
    stone_mesh: Handle<Mesh>,
}

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CatapultAssets {
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
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        wheel: meshes.add(Cylinder::new(0.55, 0.3)),
        stone_mesh: meshes.add(Sphere::new(STONE_RADIUS)),
    });
}

fn spawn_catapult(mut commands: Commands, assets: Res<CatapultAssets>) {
    let y = terrain_height(POSITION.x, POSITION.y);
    // Face the castle gate.
    let to_castle = Vec2::new(0.0, -166.0) - POSITION;
    let yaw = (-to_castle.x).atan2(-to_castle.y);

    let root = commands
        .spawn((
            Catapult {
                phase: Phase::Ready,
                charge: 0.0,
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
            CatapultArm,
            ChildOf(root),
            Transform::from_xyz(0.0, 3.0, 0.2)
                .with_rotation(Quat::from_rotation_x(ARM_COCKED)),
            Visibility::default(),
        ))
        .id();

    // Frame (colliders so the player can bump into it).
    let parts: &[(Vec3, Vec3, bool, &Handle<StandardMaterial>)] = &[
        // platform
        (Vec3::new(0.0, 0.55, 0.0), Vec3::new(3.0, 0.5, 5.0), true, &assets.wood),
        // uprights
        (Vec3::new(-1.05, 1.9, 0.2), Vec3::new(0.35, 2.6, 0.5), true, &assets.dark_wood),
        (Vec3::new(1.05, 1.9, 0.2), Vec3::new(0.35, 2.6, 0.5), true, &assets.dark_wood),
        // axle
        (Vec3::new(0.0, 3.0, 0.2), Vec3::new(2.4, 0.22, 0.22), false, &assets.iron),
        // padded stop bar at the front
        (Vec3::new(0.0, 2.4, -1.3), Vec3::new(2.2, 0.3, 0.3), false, &assets.dark_wood),
    ];
    for (pos, size, collide, material) in parts {
        let mut part = commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d((*material).clone()),
            Transform::from_translation(*pos).with_scale(*size),
            ChildOf(root),
        ));
        if *collide {
            part.insert(Collider::cuboid(1.0, 1.0, 1.0));
        }
    }
    // Wheels (decor).
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        commands.spawn((
            Mesh3d(assets.wheel.clone()),
            MeshMaterial3d(assets.dark_wood.clone()),
            Transform::from_xyz(sx * 1.65, 0.55, sz * 1.8)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ChildOf(root),
        ));
    }

    // Arm: beam, counterweight, spoon.
    for (pos, size, material) in [
        (Vec3::new(0.0, 0.0, 1.2), Vec3::new(0.3, 0.26, 5.8), &assets.wood),
        (Vec3::new(0.0, -0.2, -1.7), Vec3::new(1.0, 1.0, 1.0), &assets.iron),
        (Vec3::new(0.0, 0.16, TIP_RADIUS - 0.1), Vec3::new(0.7, 0.18, 0.8), &assets.dark_wood),
    ] {
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos).with_scale(size),
            ChildOf(arm),
        ));
    }

    load_stone(&mut commands, &assets, root);
}

/// Spawns a fresh stone seated in the spoon.
fn load_stone(commands: &mut Commands, assets: &CatapultAssets, catapult: Entity) {
    commands.spawn((
        SeatedStone { catapult },
        Mesh3d(assets.stone_mesh.clone()),
        MeshMaterial3d(assets.stone_material.clone()),
        Transform::from_xyz(0.0, -1000.0, 0.0), // placed by `seat_stone`
        RigidBody::Kinematic,
        Respawnable,
    ));
}

/// Keeps the seated stone in the spoon (world-space, follows arm swing).
fn seat_stone(
    catapults: Query<(&Transform, &Catapult), Without<SeatedStone>>,
    arms: Query<&ChildOf, With<CatapultArm>>,
    mut stones: Query<(&SeatedStone, &mut Transform)>,
) {
    let _ = arms;
    for (seat, mut transform) in &mut stones {
        let Ok((root, catapult)) = catapults.get(seat.catapult) else {
            continue;
        };
        // Arm pivot is at (0, 3.0, 0.2) local; spoon at TIP_RADIUS along
        // the arm's +Z, rotated by the current angle about local X.
        let pivot = Vec3::new(0.0, 3.0, 0.2);
        let arm_rot = Quat::from_rotation_x(catapult.angle);
        let local = pivot + arm_rot * Vec3::new(0.0, 0.35, TIP_RADIUS - 0.1);
        transform.translation = root.translation + root.rotation * local;
        transform.rotation = root.rotation;
    }
}

const MAN_RANGE: f32 = 6.0;

fn man_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut manning: ResMut<Manning>,
    players: Query<&Transform, With<Player>>,
    catapults: Query<(Entity, &Transform), With<Catapult>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    if !keys.just_pressed(KeyCode::KeyE) {
        // Auto-dismount when wandering off.
        if let Some(active) = manning.0
            && let Ok((_, root)) = catapults.get(active)
            && root.translation.distance(player.translation) > MAN_RANGE + 2.0
        {
            manning.0 = None;
        }
        return;
    }
    if manning.0.is_some() {
        manning.0 = None;
        return;
    }
    for (entity, root) in &catapults {
        if root.translation.distance(player.translation) <= MAN_RANGE {
            manning.0 = Some(entity);
            return;
        }
    }
}

/// While manned, the catapult slews to follow the player's view yaw.
fn aim(
    time: Res<Time>,
    manning: Res<Manning>,
    players: Query<&Transform, (With<Player>, Without<Catapult>)>,
    mut catapults: Query<&mut Transform, With<Catapult>>,
) {
    let (Some(active), Ok(player)) = (manning.0, players.single()) else {
        return;
    };
    let Ok(mut root) = catapults.get_mut(active) else {
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

/// Wind-up, release, swing, stone hand-off, reset, reload.
fn wind_and_loose(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    manning: Res<Manning>,
    assets: Res<CatapultAssets>,
    mut catapults: Query<(Entity, &Transform, &mut Catapult)>,
    mut arms: Query<(&ChildOf, &mut Transform), (With<CatapultArm>, Without<Catapult>)>,
    stones: Query<(Entity, &SeatedStone, &Transform), (Without<Catapult>, Without<CatapultArm>)>,
) {
    let dt = time.delta_secs();
    for (entity, root, mut catapult) in &mut catapults {
        let manned = manning.0 == Some(entity);
        match catapult.phase {
            Phase::Ready => {
                if manned && buttons.pressed(MouseButton::Left) {
                    catapult.phase = Phase::Winding;
                    catapult.charge = 0.3;
                }
            }
            Phase::Winding => {
                catapult.charge = (catapult.charge + dt / 1.6).min(1.0);
                // Creak a few degrees further back as it winds.
                catapult.angle = ARM_COCKED + catapult.charge * 0.12;
                if !manned || buttons.just_released(MouseButton::Left) || !buttons.pressed(MouseButton::Left) {
                    catapult.phase = Phase::Swinging;
                    catapult.angular_velocity = 0.0;
                }
            }
            Phase::Swinging => {
                // Constant torque from the spring: ~snappy 0.2 s swing.
                // Integrated in small substeps so the release speed is
                // frame-rate independent (one big Update step would blow
                // far past the release angle).
                let acceleration = 20.0 + 53.0 * catapult.charge;
                let substeps = (dt / 0.004).ceil().max(1.0) as u32;
                let sub_dt = dt / substeps as f32;
                let mut previous = catapult.angle;
                for _ in 0..substeps {
                    if catapult.angle <= ARM_RELEASE {
                        break;
                    }
                    catapult.angular_velocity -= acceleration * sub_dt;
                    previous = catapult.angle;
                    catapult.angle += catapult.angular_velocity * sub_dt;
                }

                // Stone releases the moment the arm passes ARM_RELEASE.
                if previous > ARM_RELEASE && catapult.angle <= ARM_RELEASE {
                    for (stone_entity, seat, stone_transform) in &stones {
                        if seat.catapult != entity {
                            continue;
                        }
                        // Tip velocity: omega x r, in the catapult's frame.
                        let speed = catapult.angular_velocity.abs() * TIP_RADIUS;
                        let a = ARM_RELEASE;
                        let local = Vec3::new(0.0, a.cos(), a.sin()) * speed;
                        let velocity = root.rotation * local;
                        info!("catapult: loosed stone at {:.1} m/s", velocity.length());
                        commands.entity(stone_entity).remove::<SeatedStone>().insert((
                            RigidBody::Dynamic,
                            Collider::sphere(STONE_RADIUS),
                            ColliderDensity(STONE_DENSITY),
                            Friction::new(0.7),
                            Restitution::new(0.1),
                            SweptCcd::default(),
                            Projectile,
                            CollisionEventsEnabled,
                            TransformInterpolation,
                            LinearVelocity(velocity.adjust_precision()),
                        ));
                        let _ = stone_transform;
                    }
                }
                if catapult.angle <= ARM_STOP {
                    catapult.angle = ARM_STOP;
                    catapult.phase = Phase::Resetting;
                }
            }
            Phase::Resetting => {
                catapult.angle += 1.4 * dt;
                if catapult.angle >= ARM_COCKED {
                    catapult.angle = ARM_COCKED;
                    catapult.charge = 0.0;
                    catapult.phase = Phase::Ready;
                    load_stone(&mut commands, &assets, entity);
                }
            }
        }

        for (parent, mut arm_transform) in &mut arms {
            if parent.parent() == entity {
                arm_transform.rotation = Quat::from_rotation_x(catapult.angle);
            }
        }
    }
}

#[derive(Component)]
struct HintText;

/// Bottom-center prompt when near or manning the catapult.
fn hint_text(
    mut commands: Commands,
    manning: Res<Manning>,
    players: Query<&Transform, With<Player>>,
    catapults: Query<&Transform, With<Catapult>>,
    mut hints: Query<(Entity, &mut Text), With<HintText>>,
) {
    let message = if manning.0.is_some() {
        "Aim with the mouse — hold Left Click to wind, release to loose — E to step off"
    } else if players.single().is_ok_and(|player| {
        catapults
            .iter()
            .any(|c| c.translation.distance(player.translation) <= MAN_RANGE)
    }) {
        "E — man the catapult"
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
