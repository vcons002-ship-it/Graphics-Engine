//! Physics props: a toppleable crate stack and scattered primitives with
//! varied PBR materials.

use avian3d::prelude::*;
use bevy::prelude::*;
use engine::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::terrain::{PLAYGROUND_CENTER, terrain_height};
use super::world::Respawnable;

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_crate_stack, spawn_scattered_props))
            .add_systems(
                Update,
                (spawn_crate_stack, spawn_scattered_props)
                    .run_if(on_message::<RestartRequested>),
            );
    }
}

/// Deterministic pseudo-random value in [0, 1) so the scene is the same on
/// every run without pulling in a rand dependency (pattern from bevy's
/// `bloom_3d` example).
fn hash01(seed: (u64, u64)) -> f32 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    (hasher.finish() % 10_000) as f32 / 10_000.0
}

const CRATE_SIZE: f32 = 0.8;

fn spawn_crate_stack(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let crate_mesh = meshes.add(Cuboid::new(CRATE_SIZE, CRATE_SIZE, CRATE_SIZE));
    let crate_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.38, 0.2),
        perceptual_roughness: 0.8,
        ..default()
    });

    // A pyramid on the playground pad: rows of 5, 4, 3, 2, 1 crates, slightly
    // separated so the solver settles them rather than spawning in contact.
    let center_z = PLAYGROUND_CENTER.y - 12.0;
    let base_y = terrain_height(PLAYGROUND_CENTER.x, center_z);
    let gap = CRATE_SIZE + 0.02;
    for (row, width) in [5, 4, 3, 2, 1].into_iter().enumerate() {
        for i in 0..width {
            let x = (i as f32 - (width as f32 - 1.0) / 2.0) * gap;
            let y = base_y + CRATE_SIZE / 2.0 + row as f32 * gap;
            commands.spawn((
                Mesh3d(crate_mesh.clone()),
                MeshMaterial3d(crate_material.clone()),
                Transform::from_xyz(x, y + 0.05, center_z),
                RigidBody::Dynamic,
                Collider::cuboid(CRATE_SIZE, CRATE_SIZE, CRATE_SIZE),
                ColliderDensity(450.0),
                Friction::new(0.6),
                Restitution::new(0.1),
                TransformInterpolation,
                Respawnable,
            ));
        }
    }
}

fn spawn_scattered_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for i in 0..30u64 {
        let r1 = hash01((i, 1));
        let r2 = hash01((i, 2));
        let r3 = hash01((i, 3));
        let r4 = hash01((i, 4));

        // Scatter in a ring around the origin, keeping the crate-stack and
        // spawn areas clear. Emissive props go on the sun-facing (+X) side so
        // their bloom doesn't sit inside the crate stack's shadow.
        let emissive = i % 10 == 1;
        let angle = if emissive {
            (r1 - 0.5) * std::f32::consts::PI
        } else {
            r1 * std::f32::consts::TAU
        };
        let radius = 7.0 + r2 * 14.0;
        let x = PLAYGROUND_CENTER.x + angle.cos() * radius;
        let z = PLAYGROUND_CENTER.y + angle.sin() * radius;
        let position = Vec3::new(x, terrain_height(x, z) + 2.0 + r3 * 2.0, z);

        let base_color = Color::hsl(r4 * 360.0, 0.55, 0.5);
        let material = match i % 10 {
            // Polished metal.
            0 | 5 => StandardMaterial {
                base_color,
                metallic: 1.0,
                perceptual_roughness: 0.15,
                ..default()
            },
            // Emissive accent — glows through bloom in daylight without the
            // flare whiting out the ground around it.
            1 => StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(2.0, 1.1, 0.3) * 180.0,
                ..default()
            },
            // Rough diffuse.
            _ => StandardMaterial {
                base_color,
                metallic: 0.0,
                perceptual_roughness: 0.6 + r3 * 0.35,
                ..default()
            },
        };
        let material = materials.add(material);

        let size = 0.4 + r2 * 0.8;
        let (mesh, collider) = match i % 3 {
            0 => (
                meshes.add(Cuboid::new(size, size, size)),
                Collider::cuboid(size, size, size),
            ),
            1 => (
                meshes.add(Sphere::new(size / 2.0)),
                Collider::sphere(size / 2.0),
            ),
            _ => (
                meshes.add(Cylinder::new(size / 2.0, size)),
                Collider::cylinder(size / 2.0, size),
            ),
        };

        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(position),
            RigidBody::Dynamic,
            collider,
            // Rough physical materials by kind: metal is dense and slick,
            // the rest are mid-weight and grippy.
            ColliderDensity(if i % 10 == 0 || i % 10 == 5 { 2700.0 } else { 800.0 }),
            Friction::new(if i % 10 == 0 || i % 10 == 5 { 0.35 } else { 0.6 }),
            Restitution::new(0.15),
            TransformInterpolation,
            Respawnable,
        ));
    }
}
