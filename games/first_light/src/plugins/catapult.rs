//! A mannable siege catapult.
//!
//! Walk up (E to man it), aim by looking — the catapult slews to follow
//! your view and a trajectory arc shows where the stone will land,
//! updating as you wind. Hold left click to wind, release to loose; the
//! camera follows the shot downrange until you click again to wind the
//! next one. The arm swing is kinematic with substepped integration
//! (frame-rate-independent release speed); the stone is a fully dynamic
//! ~1.8-tonne granite sphere with continuous collision detection. The
//! masonry system (`masonry.rs`) decides what shatters when it lands.

use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use super::masonry::{PreTickVelocity, Projectile};
use super::terrain::{CASTLE_CENTER, KNOLL_CENTER, terrain_height};
use super::world::Respawnable;
use engine::prelude::*;

pub struct CatapultPlugin;

impl Plugin for CatapultPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Manning>()
            .init_resource::<ShotCamera>()
            .add_systems(Startup, (setup_assets, spawn_catapult).chain());

        // Headless verification: `FL_AUTO_MAN=<frame>` mans the catapult so
        // screenshots show the aiming view, trajectory arc, and chase camera.
        if let Some(at_frame) = std::env::var("FL_AUTO_MAN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.add_systems(
                Update,
                move |mut frame: Local<u32>,
                      mut manning: ResMut<Manning>,
                      catapults: Query<Entity, With<Catapult>>| {
                    *frame += 1;
                    if *frame == at_frame
                        && let Some(catapult) = catapults.iter().next()
                    {
                        manning.0 = Some(catapult);
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
                    move |mut frame: Local<u32>, mut catapults: Query<&mut Catapult>| {
                        *frame += 1;
                        // Fire twice: the second shot proves the reload.
                        if *frame == at_frame || *frame == at_frame + 45 {
                            for mut catapult in &mut catapults {
                                catapult.phase = Phase::Swinging { released: false };
                                catapult.charge = charge;
                                catapult.angular_velocity = 0.0;
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
                catapult_camera,
                hint_text,
            )
                .chain()
                .run_if(in_state(MenuState::Closed)),
        );
    }
}

/// Which catapult the player is manning, if any. Checked by the throw
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
}

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
const ARM_COCKED: f32 = 0.17;
const ARM_RELEASE: f32 = -0.87; // launch elevation ~40 degrees
const ARM_STOP: f32 = -1.31; // padded stop
/// Extra wind-back at full charge.
const WIND_BACK: f32 = 0.12;
/// Arm pivot in the catapult's local space (top of the uprights).
const PIVOT: Vec3 = Vec3::new(0.0, 4.6, 0.3);
/// Spoon distance from the pivot.
const TIP_RADIUS: f32 = 6.3;
/// Stone seat offset above the spoon.
const SEAT: Vec3 = Vec3::new(0.0, 0.5, TIP_RADIUS - 0.15);
const STONE_RADIUS: f32 = 0.65;
/// Granite.
const STONE_DENSITY: f32 = 2600.0;
/// Spring angular acceleration (rad/s^2) by charge: ~43–81 m/s at the tip,
/// i.e. from "foot of the curtain wall" to "far over the castle".
fn spring_acceleration(charge: f32) -> f32 {
    6.0 + 64.0 * charge
}
/// Seconds of held click for full charge.
const WIND_TIME: f32 = 2.2;

/// The catapult stands on the siege knoll, overlooking the castle.
const POSITION: Vec2 = KNOLL_CENTER;

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
        wheel: meshes.add(Cylinder::new(0.85, 0.45)),
        stone_mesh: meshes.add(Sphere::new(STONE_RADIUS)),
    });
}

fn spawn_catapult(mut commands: Commands, assets: Res<CatapultAssets>) {
    let y = terrain_height(POSITION.x, POSITION.y);
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
            Transform::from_translation(PIVOT).with_rotation(Quat::from_rotation_x(ARM_COCKED)),
            Visibility::default(),
        ))
        .id();

    // Frame (colliders so the player can bump into it).
    let parts: &[(Vec3, Vec3, bool, &Handle<StandardMaterial>)] = &[
        // platform
        (Vec3::new(0.0, 0.85, 0.0), Vec3::new(4.6, 0.7, 7.6), true, &assets.wood),
        // uprights
        (Vec3::new(-1.6, 2.9, 0.3), Vec3::new(0.5, 4.0, 0.7), true, &assets.dark_wood),
        (Vec3::new(1.6, 2.9, 0.3), Vec3::new(0.5, 4.0, 0.7), true, &assets.dark_wood),
        // diagonal-ish braces (simple posts front and back)
        (Vec3::new(-1.6, 1.9, 2.4), Vec3::new(0.4, 2.4, 0.4), false, &assets.dark_wood),
        (Vec3::new(1.6, 1.9, 2.4), Vec3::new(0.4, 2.4, 0.4), false, &assets.dark_wood),
        // axle
        (PIVOT, Vec3::new(3.6, 0.3, 0.3), false, &assets.iron),
        // padded stop bar at the front
        (Vec3::new(0.0, 3.6, -2.0), Vec3::new(3.4, 0.45, 0.45), false, &assets.dark_wood),
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
            Transform::from_xyz(sx * 2.5, 0.85, sz * 2.8)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ChildOf(root),
        ));
    }

    // Arm: beam, counterweight, spoon.
    for (pos, size, material) in [
        (Vec3::new(0.0, 0.0, 1.9), Vec3::new(0.45, 0.38, 9.0), &assets.wood),
        (Vec3::new(0.0, -0.35, -2.4), Vec3::new(1.5, 1.5, 1.5), &assets.iron),
        (Vec3::new(0.0, 0.24, TIP_RADIUS - 0.15), Vec3::new(1.1, 0.26, 1.2), &assets.dark_wood),
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
    mut stones: Query<(&SeatedStone, &mut Transform)>,
) {
    for (seat, mut transform) in &mut stones {
        let Ok((root, catapult)) = catapults.get(seat.catapult) else {
            continue;
        };
        let arm_rot = Quat::from_rotation_x(catapult.angle);
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
    catapults: Query<(Entity, &Transform), With<Catapult>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    if !keys.just_pressed(KeyCode::KeyE) {
        if let Some(active) = manning.0
            && let Ok((_, root)) = catapults.get(active)
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
    assets: Res<CatapultAssets>,
    mut catapults: Query<(Entity, &Transform, &mut Catapult)>,
    mut arms: Query<(&ChildOf, &mut Transform), (With<CatapultArm>, Without<Catapult>)>,
    stones: Query<(Entity, &SeatedStone), Without<Catapult>>,
) {
    let dt = time.delta_secs();
    for (entity, root, mut catapult) in &mut catapults {
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

        match catapult.phase {
            Phase::Ready => {
                if manned && !watching && buttons.pressed(MouseButton::Left) {
                    catapult.phase = Phase::Winding;
                    catapult.charge = 0.25;
                }
            }
            Phase::Winding => {
                catapult.charge = (catapult.charge + dt / WIND_TIME).min(1.0);
                catapult.angle = ARM_COCKED + catapult.charge * WIND_BACK;
                if !manned || !buttons.pressed(MouseButton::Left) {
                    catapult.phase = Phase::Swinging { released: false };
                    catapult.angular_velocity = 0.0;
                }
            }
            Phase::Swinging { mut released } => {
                // Substepped so release speed is frame-rate independent.
                let acceleration = spring_acceleration(catapult.charge);
                let substeps = (dt / 0.004).ceil().max(1.0) as u32;
                let sub_dt = dt / substeps as f32;
                for _ in 0..substeps {
                    if catapult.angle <= ARM_STOP {
                        break;
                    }
                    catapult.angular_velocity -= acceleration * sub_dt;
                    let previous = catapult.angle;
                    catapult.angle += catapult.angular_velocity * sub_dt;

                    if !released && previous > ARM_RELEASE && catapult.angle <= ARM_RELEASE {
                        released = true;
                        // Snap the stone to the exact release pose so the
                        // flight matches the previewed arc at any frame rate.
                        let (position, velocity) = release_state(root, catapult.charge);
                        for (stone_entity, seat) in &stones {
                            if seat.catapult != entity {
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
                            info!("catapult: loosed stone at {:.1} m/s", velocity.length());
                        }
                    }
                }
                if catapult.angle <= ARM_STOP {
                    catapult.angle = ARM_STOP;
                    catapult.phase = Phase::Resetting;
                } else {
                    catapult.phase = Phase::Swinging { released };
                }
            }
            Phase::Resetting => {
                // Fast reset: ready to wind again in about half a second.
                catapult.angle += 3.2 * dt;
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

/// Dotted arc from the spoon to the predicted landing point, live while
/// aiming and winding.
fn trajectory_preview(
    mut gizmos: Gizmos,
    manning: Res<Manning>,
    shot_camera: Res<ShotCamera>,
    catapults: Query<(&Transform, &Catapult)>,
) {
    let Some(active) = manning.0 else {
        return;
    };
    if shot_camera.following.is_some() {
        return;
    }
    let Ok((root, catapult)) = catapults.get(active) else {
        return;
    };
    let charge = match catapult.phase {
        Phase::Winding => catapult.charge,
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

/// While manning: an elevated chase view behind the catapult; after firing
/// the camera follows the stone downrange until the player clicks.
fn catapult_camera(
    manning: Res<Manning>,
    mut shot_camera: ResMut<ShotCamera>,
    time: Res<Time>,
    catapults: Query<&Transform, (With<Catapult>, Without<MainCamera>)>,
    players: Query<&Transform, (With<Player>, Without<Catapult>, Without<MainCamera>)>,
    stones: Query<(&Transform, Option<&LinearVelocity>), (With<Projectile>, Without<Player>, Without<Catapult>, Without<MainCamera>)>,
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
                    shot_camera.following = None;
                    shot_camera.held_eye = None;
                    None
                }
            }
        } else if let Ok(root) = catapults.get(active) {
            // Behind and above the machine, looking down the aim line.
            let eye = root.translation + root.rotation * Vec3::new(0.0, 7.5, 13.0);
            let target = root.translation + root.rotation * Vec3::new(0.0, 3.0, -30.0);
            Some((eye, target))
        } else {
            None
        }
    } else {
        None
    };

    match desired {
        Some((eye, target)) => {
            // Convert the desired world pose into the camera's local frame
            // (it is a child of the player).
            let world = Transform::from_translation(eye).looking_at(target, Vec3::Y);
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

/// Bottom-center prompt when near or manning the catapult.
fn hint_text(
    mut commands: Commands,
    manning: Res<Manning>,
    shot_camera: Res<ShotCamera>,
    players: Query<&Transform, With<Player>>,
    catapults: Query<&Transform, With<Catapult>>,
    mut hints: Query<(Entity, &mut Text), With<HintText>>,
) {
    let message = if manning.0.is_some() {
        if shot_camera.following.is_some() {
            "Click to wind the next shot — E to step off"
        } else {
            "Aim with the mouse — hold Left Click to wind, release to loose — E to step off"
        }
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
