//! Lush valley vegetation: pine forests on the lower slopes, boulders on the
//! scree, bushes across the meadow. All placement is deterministic
//! (hash-based rejection sampling against the terrain functions), so the
//! forest is identical on every run.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::terrain::{
    CASTLE_CENTER, LAKE_CENTER, LAKE_RADIUS, PLAYGROUND_CENTER, terrain_height, terrain_normal,
};

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_vegetation);
    }
}

fn hash01(seed: (u64, u64)) -> f32 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    (hasher.finish() % 10_000) as f32 / 10_000.0
}

/// Keeps vegetation off gameplay areas: lake, playground pad, castle terrace,
/// and the causeway.
fn clear_of_landmarks(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    if p.distance(LAKE_CENTER) < LAKE_RADIUS + 4.0 {
        return false;
    }
    if p.distance(PLAYGROUND_CENTER) < 30.0 {
        return false;
    }
    if p.distance(CASTLE_CENTER) < 105.0 {
        return false;
    }
    // Causeway corridor.
    if x.abs() < 22.0 && (-200.0..=-55.0).contains(&z) {
        return false;
    }
    true
}

fn spawn_vegetation(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Shared meshes; instances vary by transform scale.
    let trunk_mesh = meshes.add(Cone {
        radius: 0.3,
        height: 3.2,
    });
    let canopy_mesh = meshes.add(Cone {
        radius: 1.0,
        height: 1.0,
    });
    let rock_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let bush_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());

    let trunk_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.20, 0.12),
        perceptual_roughness: 0.95,
        ..default()
    });
    let canopy_materials: Vec<_> = [
        Color::srgb(0.07, 0.26, 0.09),
        Color::srgb(0.10, 0.31, 0.11),
        Color::srgb(0.06, 0.22, 0.10),
        Color::srgb(0.13, 0.34, 0.12),
        Color::srgb(0.16, 0.33, 0.08),
        Color::srgb(0.09, 0.28, 0.14),
    ]
    .map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.9,
            ..default()
        })
    })
    .to_vec();
    let rock_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.40, 0.38),
        perceptual_roughness: 0.95,
        ..default()
    });
    let bush_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.30, 0.10),
        perceptual_roughness: 0.95,
        ..default()
    });

    // --- Pines ---------------------------------------------------------------
    let mut planted = 0;
    for i in 0..1400u64 {
        if planted >= 380 {
            break;
        }
        let x = (hash01((i, 1)) - 0.5) * 580.0;
        let z = (hash01((i, 2)) - 0.5) * 580.0;
        let h = terrain_height(x, z);
        let normal = terrain_normal(x, z);
        // Trees: lower slopes and valley floor, gentle ground, below the rock line.
        if !(0.5..=58.0).contains(&h) || normal.y < 0.88 || !clear_of_landmarks(x, z) {
            continue;
        }
        planted += 1;

        let s = 0.8 + hash01((i, 3)) * 1.0;
        // A slight random lean makes the forest read as organic.
        let lean = Quat::from_euler(
            EulerRot::XYZ,
            (hash01((i, 5)) - 0.5) * 0.10,
            hash01((i, 6)) * std::f32::consts::TAU,
            (hash01((i, 7)) - 0.5) * 0.10,
        );
        let canopy = canopy_materials[(i % canopy_materials.len() as u64) as usize].clone();
        commands
            .spawn((
                Transform::from_xyz(x, h, z).with_rotation(lean),
                Visibility::default(),
                RigidBody::Static,
                Collider::cylinder(0.3 * s, 3.0 * s),
            ))
            .with_children(|tree| {
                tree.spawn((
                    Mesh3d(trunk_mesh.clone()),
                    MeshMaterial3d(trunk_material.clone()),
                    Transform::from_xyz(0.0, 1.6 * s, 0.0).with_scale(Vec3::splat(s)),
                ));
                for (layer, (radius, height, y)) in
                    [(1.6, 2.8, 2.6), (1.25, 2.4, 4.0), (0.85, 2.0, 5.2)]
                        .into_iter()
                        .enumerate()
                {
                    tree.spawn((
                        Mesh3d(canopy_mesh.clone()),
                        MeshMaterial3d(canopy.clone()),
                        Transform::from_xyz(0.0, y * s, 0.0).with_scale(Vec3::new(
                            radius * 2.0 * s,
                            height * s,
                            radius * 2.0 * s,
                        )),
                        // Slight per-layer twist so silhouettes vary.
                        Visibility::default(),
                    ));
                    let _ = layer;
                }
            });
    }

    // --- Boulders --------------------------------------------------------------
    let mut placed = 0;
    for i in 0..900u64 {
        if placed >= 80 {
            break;
        }
        let x = (hash01((i, 11)) - 0.5) * 580.0;
        let z = (hash01((i, 12)) - 0.5) * 580.0;
        let h = terrain_height(x, z);
        let normal = terrain_normal(x, z);
        let slope = 1.0 - normal.y;
        // Boulders favor the scree between meadow and cliffs.
        if !(0.0..=95.0).contains(&h) || !(0.06..=0.45).contains(&slope) || !clear_of_landmarks(x, z)
        {
            continue;
        }
        placed += 1;

        let s = 0.5 + hash01((i, 13)) * 1.6;
        let squash = 0.55 + hash01((i, 14)) * 0.4;
        commands.spawn((
            Mesh3d(rock_mesh.clone()),
            MeshMaterial3d(rock_material.clone()),
            Transform::from_xyz(x, h + s * squash * 0.45, z)
                .with_rotation(Quat::from_rotation_y(hash01((i, 15)) * std::f32::consts::TAU))
                .with_scale(Vec3::new(s, s * squash, s * 0.8)),
            RigidBody::Static,
            Collider::sphere(0.8),
        ));
    }

    // --- Bushes ------------------------------------------------------------------
    let mut placed = 0;
    for i in 0..900u64 {
        if placed >= 150 {
            break;
        }
        let x = (hash01((i, 21)) - 0.5) * 500.0;
        let z = (hash01((i, 22)) - 0.5) * 500.0;
        let h = terrain_height(x, z);
        let normal = terrain_normal(x, z);
        if !(0.2..=25.0).contains(&h) || normal.y < 0.94 || !clear_of_landmarks(x, z) {
            continue;
        }
        placed += 1;

        let s = 0.35 + hash01((i, 23)) * 0.6;
        commands.spawn((
            Mesh3d(bush_mesh.clone()),
            MeshMaterial3d(bush_material.clone()),
            Transform::from_xyz(x, h + s * 0.4, z).with_scale(Vec3::new(s, s * 0.7, s)),
        ));
    }
}
