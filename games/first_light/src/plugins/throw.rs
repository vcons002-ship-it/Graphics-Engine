//! Left-click throws a glowing cube from the camera.

use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;
use engine::prelude::*;

use super::trebuchet::Manning;
use super::masonry::{PreTickVelocity, Projectile};
use super::world::Respawnable;

pub struct ThrowPlugin;

impl Plugin for ThrowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_throw_assets)
            .add_systems(Update, (throw_cube, despawn_expired));
    }
}

const CUBE_SIZE: f32 = 0.25;
const THROW_SPEED: f32 = 14.0;
const CUBE_LIFETIME_SECS: f32 = 30.0;

#[derive(Resource)]
struct ThrowAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

/// Despawn timer so thrown cubes don't accumulate forever.
#[derive(Component)]
struct Expires(Timer);

fn setup_throw_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ThrowAssets {
        mesh: meshes.add(Cuboid::new(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE)),
        material: materials.add(StandardMaterial {
            base_color: Color::BLACK,
            // Bright enough to bloom against full daylight.
            emissive: LinearRgba::rgb(0.4, 1.4, 2.0) * 450.0,
            ..default()
        }),
    });
}

fn throw_cube(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    locked: Res<CursorLocked>,
    manning: Res<Manning>,
    assets: Res<ThrowAssets>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    // `is_changed` filters out the click that grabbed the cursor this frame;
    // while manning the catapult, left click winds it instead.
    if !locked.0 || locked.is_changed() || manning.0.is_some() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(camera) = camera.single() else {
        return;
    };

    let direction = camera.forward();
    commands.spawn((
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_translation(camera.translation() + direction * 0.8)
            .with_rotation(camera.rotation()),
        (
            RigidBody::Dynamic,
            Collider::cuboid(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE),
            ColliderDensity(1500.0),
            Friction::new(0.5),
            Restitution::new(0.2),
            SweptCcd::default(),
        ),
        Projectile,
        CollisionEventsEnabled,
        PreTickVelocity(direction * THROW_SPEED),
        LinearVelocity((direction * THROW_SPEED).adjust_precision()),
        TransformInterpolation,
        Respawnable,
        Expires(Timer::from_seconds(CUBE_LIFETIME_SECS, TimerMode::Once)),
    ));
}

fn despawn_expired(
    mut commands: Commands,
    time: Res<Time>,
    mut expiring: Query<(Entity, &mut Expires)>,
) {
    for (entity, mut expires) in &mut expiring {
        if expires.0.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
