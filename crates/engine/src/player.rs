//! First-person kinematic character controller.
//!
//! The core (ground detection, gravity, damping, move-and-slide, pushing
//! dynamic bodies) is ported from Avian's official `kinematic_character_3d`
//! example at tag v0.6.1. On top of that: first-person mouse look (yaw on the
//! body, pitch on the camera child), sprint, camera-relative movement, and
//! cursor grab (click to lock, Esc to release).
//!
//! One deliberate change from the example: input is accumulated per frame
//! into [`MoveInput`] state and consumed in `FixedUpdate`, instead of sending
//! per-frame messages. This keeps acceleration independent of the render
//! frame rate.

#![allow(clippy::type_complexity)]

use avian3d::{math::*, prelude::*};
use bevy::ecs::query::Has;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::f32::consts::FRAC_PI_2;

use crate::camera::MainCamera;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorLocked>()
            .init_resource::<LookSensitivity>()
            .add_systems(PreUpdate, keyboard_input)
            .add_systems(
                Update,
                (
                    cursor_grab.run_if(in_state(crate::menu::MenuState::Closed)),
                    mouse_look,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                (
                    update_grounded,
                    apply_gravity,
                    movement,
                    apply_movement_damping,
                    move_and_slide,
                    apply_forces_to_dynamic_bodies,
                )
                    .chain(),
            );
    }
}

/// Whether the cursor is currently grabbed for mouse look. Game systems that
/// react to clicks (e.g. throwing) should check `locked.0 &&
/// !locked.is_changed()` so the click that grabs the cursor isn't also a
/// gameplay action.
#[derive(Resource, Default)]
pub struct CursorLocked(pub bool);

/// Mouse look sensitivity (radians per pixel of motion), horizontal/vertical.
#[derive(Resource, Deref)]
pub struct LookSensitivity(pub Vec2);

impl Default for LookSensitivity {
    fn default() -> Self {
        Self(Vec2::new(0.003, 0.002))
    }
}

/// Marker for the player entity.
#[derive(Component)]
pub struct Player;

/// A marker component indicating that an entity is using a character
/// controller. `CustomPositionIntegration` prevents Avian from applying the
/// character's velocity to its position automatically, since move-and-slide
/// handles movement manually.
#[derive(Component)]
#[require(
    RigidBody::Kinematic,
    CustomPositionIntegration,
    // We don't want to impart speculative collision impulses in this case.
    SpeculativeMargin(0.0)
)]
pub struct CharacterController;

/// Movement tuning for a character controller.
#[derive(Component)]
pub struct CharacterMovementSettings {
    /// The acceleration used for character movement.
    pub acceleration: Scalar,
    /// The damping coefficient used for slowing down movement.
    pub damping: Scalar,
    /// The strength of a jump.
    pub jump_impulse: Scalar,
    /// Movement speed multiplier while sprinting.
    pub sprint_multiplier: Scalar,
    /// The gravitational acceleration used for the character.
    pub gravity: Vector,
    /// The maximum speed that gravity can accelerate the character to.
    pub terminal_velocity: Scalar,
}

impl Default for CharacterMovementSettings {
    fn default() -> Self {
        Self {
            acceleration: 50.0,
            damping: 10.0,
            jump_impulse: 7.0,
            sprint_multiplier: 1.6,
            gravity: Vector::new(0.0, -9.81 * 2.0, 0.0),
            terminal_velocity: 50.0,
        }
    }
}

/// Per-frame movement intent, written by input systems in `PreUpdate` and
/// consumed by [`movement`] in `FixedUpdate`.
#[derive(Component, Default)]
pub struct MoveInput {
    /// Local-space direction: x = strafe right, y = forward.
    pub direction: Vec2,
    pub sprinting: bool,
    /// Set on the frame jump is pressed; cleared when consumed so jumps
    /// aren't lost when `FixedUpdate` doesn't run that frame.
    pub jump_queued: bool,
}

/// Ground detection configuration for a character controller.
#[derive(Component)]
pub struct GroundDetection {
    /// The maximum angle (in radians) where a surface is considered
    /// ground/ceiling relative to the up-direction.
    pub max_angle: Scalar,
    /// The maximum distance for ground detection.
    pub max_distance: Scalar,
    /// The shape cast collider used for ground detection.
    pub cast_shape: Option<Collider>,
}

impl Default for GroundDetection {
    fn default() -> Self {
        Self {
            max_angle: PI / 6.0,
            max_distance: 0.2,
            cast_shape: None,
        }
    }
}

/// A marker component indicating that an entity is on ground steeper than
/// [`GroundDetection::max_angle`]. Grounded characters can jump.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// Current collisions for a character controller, used to apply forces to
/// dynamic rigid bodies hit by the character.
#[derive(Component, Default, Deref)]
pub struct CharacterCollisions(Vec<CharacterCollision>);

/// Information about a collision between a character controller and another
/// collider.
pub struct CharacterCollision {
    /// The collider that was hit by the character.
    pub collider: Entity,
    /// The point of contact in world space.
    pub point: Vector,
    /// The normal of the contact surface, pointing away from the character.
    pub normal: Dir3,
    /// The velocity of the character at the point of contact.
    pub character_velocity: Vector,
}

/// Capsule dimensions for the player (radius, length between hemisphere
/// centers): total height 1.8 m.
const CAPSULE_RADIUS: f32 = 0.4;
const CAPSULE_LENGTH: f32 = 1.0;
/// Eye height above the capsule center.
const EYE_HEIGHT: f32 = 0.6;

/// Spawns a fully wired first-person player (controller + camera child) at
/// `position` and returns its entity.
pub fn spawn_player(commands: &mut Commands, position: Vec3) -> Entity {
    commands
        .spawn((
            Player,
            CharacterController,
            CharacterMovementSettings::default(),
            CharacterCollisions::default(),
            MoveInput::default(),
            GroundDetection {
                // Slightly smaller capsule for the ground-detection shape cast.
                cast_shape: Some(Collider::capsule(CAPSULE_RADIUS - 0.001, CAPSULE_LENGTH)),
                ..default()
            },
            Collider::capsule(CAPSULE_RADIUS, CAPSULE_LENGTH),
            Transform::from_translation(position),
            TransformInterpolation,
            Visibility::default(),
            children![(MainCamera, Transform::from_xyz(0.0, EYE_HEIGHT, 0.0))],
        ))
        .id()
}

/// Writes per-frame movement intent from the keyboard.
fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut players: Query<&mut MoveInput, With<Player>>,
) {
    let up = keyboard_input.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    let down = keyboard_input.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);
    let left = keyboard_input.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);
    let right = keyboard_input.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);

    let horizontal = right as i8 - left as i8;
    let vertical = up as i8 - down as i8;
    let direction = Vec2::new(horizontal as f32, vertical as f32).clamp_length_max(1.0);

    for mut input in &mut players {
        input.direction = direction;
        input.sprinting = keyboard_input.pressed(KeyCode::ShiftLeft);
        if keyboard_input.just_pressed(KeyCode::Space) {
            input.jump_queued = true;
        }
    }
}

/// Click grabs the cursor for mouse look while no menu is open. Esc is owned
/// by the pause menu, which releases the cursor on open and re-grabs it on
/// resume.
fn cursor_grab(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut locked: ResMut<CursorLocked>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    if !locked.0 && mouse_buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        locked.0 = true;
    }
}

/// First-person look: yaw rotates the player body, pitch rotates the camera
/// child (clamped just short of straight up/down).
fn mouse_look(
    locked: Res<CursorLocked>,
    sensitivity: Res<LookSensitivity>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut players: Query<&mut Transform, With<Player>>,
    mut cameras: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let delta = mouse_motion.delta;
    if !locked.0 || delta == Vec2::ZERO {
        return;
    }

    for mut transform in &mut players {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_rotation_y(yaw - delta.x * sensitivity.x);
    }

    const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
    for mut transform in &mut cameras {
        let (_, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        let pitch = (pitch - delta.y * sensitivity.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, pitch, 0.0);
    }
}

/// Updates the [`Grounded`] status for character controllers.
fn update_grounded(
    mut commands: Commands,
    mut query: Query<(Entity, &GroundDetection, &GlobalTransform)>,
    spatial_query: SpatialQuery,
) {
    for (entity, ground_detection, global_transform) in &mut query {
        let Some(collider) = &ground_detection.cast_shape else {
            continue;
        };

        let translation = global_transform.translation().adjust_precision();
        let rotation = global_transform.rotation().adjust_precision();

        // Cast the shape downward to check for ground.
        let hit = spatial_query.cast_shape(
            collider,
            translation,
            rotation,
            global_transform.down(),
            &ShapeCastConfig::from_max_distance(ground_detection.max_distance),
            &SpatialQueryFilter::from_excluded_entities([entity]),
        );

        // The character is grounded if we hit a surface that isn't too steep.
        let is_grounded = hit.is_some_and(|hit| {
            let up = global_transform.up().adjust_precision();
            (rotation * hit.normal1).angle_between(up) <= ground_detection.max_angle
        });

        if is_grounded {
            commands.entity(entity).insert(Grounded);
        } else {
            commands.entity(entity).remove::<Grounded>();
        }
    }
}

/// Accelerates character controllers according to their [`MoveInput`],
/// relative to the body's yaw.
fn movement(
    time: Res<Time>,
    mut controllers: Query<(
        &CharacterMovementSettings,
        &mut MoveInput,
        &Transform,
        &mut LinearVelocity,
        Has<Grounded>,
    )>,
) {
    let delta_secs = time.delta_secs_f64().adjust_precision();

    for (settings, mut input, transform, mut linear_velocity, is_grounded) in &mut controllers {
        let speed = if input.sprinting {
            settings.sprint_multiplier
        } else {
            1.0
        };
        let local = Vec3::new(input.direction.x, 0.0, -input.direction.y);
        let world_direction = (transform.rotation * local).adjust_precision() * speed;
        linear_velocity.0 += world_direction * settings.acceleration * delta_secs;

        if input.jump_queued {
            if is_grounded {
                linear_velocity.y = settings.jump_impulse;
            }
            input.jump_queued = false;
        }
    }
}

/// Applies gravity to character controllers.
fn apply_gravity(
    time: Res<Time>,
    mut controllers: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
) {
    let delta_secs = time.delta_secs_f64().adjust_precision();

    for (movement, mut linear_velocity) in &mut controllers {
        let gravity_direction = movement.gravity.normalize_or_zero();

        let velocity_along_gravity = linear_velocity.dot(gravity_direction);
        if velocity_along_gravity > movement.terminal_velocity {
            // Don't apply more gravity if we're already at terminal velocity.
            continue;
        }

        let new_velocity = linear_velocity.0 + movement.gravity * delta_secs;

        // Don't exceed terminal velocity.
        let new_velocity_along_gravity = new_velocity.dot(gravity_direction);
        if new_velocity_along_gravity < movement.terminal_velocity {
            linear_velocity.0 = new_velocity;
        } else {
            linear_velocity.0 = gravity_direction * movement.terminal_velocity;
        }
    }
}

/// Slows down movement in the XZ plane.
fn apply_movement_damping(
    mut query: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs_f64().adjust_precision();

    for (movement, mut linear_velocity) in &mut query {
        // Approximate exponential decay. We could use `LinearDamping` for
        // this, but we don't want to dampen movement along the Y axis.
        linear_velocity.x *= 1.0 / (1.0 + delta_secs * movement.damping);
        linear_velocity.z *= 1.0 / (1.0 + delta_secs * movement.damping);
    }
}

/// Performs move-and-slide for character controllers, moving them according
/// to their velocity and sliding along any contact surfaces.
///
/// For simplicity, we assume that the character is not a child entity, and
/// its collider is on the same entity as the `CharacterController`.
fn move_and_slide(
    mut query: Query<
        (
            Entity,
            Option<&GroundDetection>,
            Option<&mut CharacterCollisions>,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
        ),
        With<CharacterController>,
    >,
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
) {
    for (entity, ground_detection, mut collisions, mut transform, mut lin_vel, collider) in
        &mut query
    {
        let mut hit_ground_or_ceiling = false;

        if let Some(collisions) = &mut collisions {
            // Clear previous collisions.
            collisions.0.clear();
        }

        let up = transform.up().adjust_precision();

        // Perform move-and-slide.
        let MoveAndSlideOutput {
            position: new_position,
            projected_velocity,
        } = move_and_slide.move_and_slide(
            collider,
            transform.translation.adjust_precision(),
            transform.rotation.adjust_precision(),
            lin_vel.0,
            time.delta(),
            &MoveAndSlideConfig::default(),
            &SpatialQueryFilter::from_excluded_entities([entity]),
            |hit| {
                // Called for each surface hit during move-and-slide. Used to
                // prevent sliding down gentle slopes while grounded and
                // climbing up steep ones.

                let Some(ground_detection) = ground_detection else {
                    return MoveAndSlideHitResponse::Accept;
                };

                // Is the surface ground, based on the angle between the
                // up-vector and the hit normal?
                let angle = up.angle_between(hit.normal.adjust_precision());
                let is_ground = angle <= ground_detection.max_angle;
                let is_ceiling = is_ground && up.dot(hit.normal.adjust_precision()) < 0.0;

                // Decompose the original input velocity into components
                // relative to the hit normal and the up direction.
                let [horizontal_component, vertical_component] =
                    split_into_components(lin_vel.0, up);

                let horizontal_velocity_decomposition =
                    decompose_hit_velocity(horizontal_component, *hit.normal, up);
                let decomposition = decompose_hit_velocity(*hit.velocity, *hit.normal, up);

                // An object is trying to slip if the tangential movement
                // induced by its vertical movement points downward.
                let slipping_intent =
                    up.dot(horizontal_velocity_decomposition.vertical_tangent) < -0.001;

                // An object is slipping if its vertical movement points downward.
                let slipping = up.dot(decomposition.vertical_tangent) < -0.001;

                // An object is trying to climb if its vertical input motion
                // points upward.
                let climbing_intent = up.dot(vertical_component) > 0.0;

                // An object is climbing if the tangential movement induced by
                // its vertical movement points upward.
                let climbing = up.dot(decomposition.vertical_tangent) > 0.0;

                let projected_velocity = if !is_ground && climbing && !climbing_intent {
                    // Can't climb the slope; remove the vertical tangent
                    // motion induced by the forward motion.
                    decomposition.horizontal_tangent + decomposition.normal_part
                } else if is_ground && slipping && !slipping_intent {
                    // Prevent the vertical movement from sliding down.
                    decomposition.horizontal_tangent + decomposition.normal_part
                } else {
                    // Otherwise, allow full movement.
                    decomposition.horizontal_tangent
                        + decomposition.vertical_tangent
                        + decomposition.normal_part
                };

                // Update the current velocity used by the algorithm.
                *hit.velocity = projected_velocity;

                if is_ground || is_ceiling {
                    hit_ground_or_ceiling = true;
                }

                if let Some(collisions) = &mut collisions {
                    // Record the collision for use in other systems, such as
                    // applying forces to dynamic bodies.
                    collisions.0.push(CharacterCollision {
                        collider: hit.entity,
                        point: hit.point,
                        normal: *hit.normal,
                        character_velocity: *hit.velocity,
                    });
                }

                MoveAndSlideHitResponse::Accept
            },
        );

        // Update position to the final position calculated by move-and-slide.
        transform.translation = new_position.f32();

        // If we hit the ground or a ceiling, update the velocity along the
        // up-direction to prevent accumulating velocity along the ground
        // normal when hitting slopes, and to prevent sticking to ceilings
        // when jumping.
        if hit_ground_or_ceiling {
            let up = up.adjust_precision();
            let velocity_along_up = lin_vel.dot(up);
            let new_velocity_along_up = projected_velocity.dot(up);
            lin_vel.0 += (new_velocity_along_up - velocity_along_up) * up;
        }
    }
}

/// The decomposition of a velocity vector into parts relative to a collision
/// normal and an up-direction.
#[derive(Debug)]
struct VelocityDecomposition {
    /// The part of the velocity that is directly against the collision normal.
    normal_part: Vector,
    /// The part tangent to the collision surface and perpendicular to up.
    horizontal_tangent: Vector,
    /// The part tangent to the collision surface and parallel to up.
    vertical_tangent: Vector,
}

/// Decomposes a velocity vector into parts relative to a collision `normal`
/// and an `up` direction.
fn decompose_hit_velocity(velocity: Vector, normal: Dir, up: Vector) -> VelocityDecomposition {
    let normal = normal.adjust_precision();
    let normal_part = normal * normal.dot(velocity);
    let tangent_part = velocity - normal_part;

    let horizontal_tangent_dir = normal.cross(up).normalize_or_zero();
    let horizontal_tangent = tangent_part.dot(horizontal_tangent_dir) * horizontal_tangent_dir;
    let vertical_tangent = tangent_part - horizontal_tangent;

    VelocityDecomposition {
        normal_part,
        horizontal_tangent,
        vertical_tangent,
    }
}

/// Splits a vector into horizontal and vertical components relative to `up`.
fn split_into_components(v: Vector, up: Vector) -> [Vector; 2] {
    let vertical_component = up * v.dot(up);
    let horizontal_component = v - vertical_component;
    [horizontal_component, vertical_component]
}

/// Applies forces to dynamic rigid bodies hit by character controllers.
fn apply_forces_to_dynamic_bodies(
    characters: Query<(&ComputedMass, &CharacterCollisions)>,
    colliders: Query<&ColliderOf>,
    mut rigid_bodies: Query<(&RigidBody, Forces)>,
) {
    for (mass, collisions) in &characters {
        let mass = mass.value();
        for collision in &collisions.0 {
            let Ok(collider_of) = colliders.get(collision.collider) else {
                continue;
            };
            let Ok((rigid_body, mut forces)) = rigid_bodies.get_mut(collider_of.body) else {
                continue;
            };
            if !rigid_body.is_dynamic() {
                continue;
            }

            let touch_dir = -collision.normal.adjust_precision();
            let relative_velocity = collision.character_velocity - forces.linear_velocity();
            let touch_velocity = touch_dir.dot(relative_velocity) * touch_dir;
            let impulse = touch_velocity * mass;

            forces.apply_linear_impulse_at_point(impulse, collision.point);
        }
    }
}
