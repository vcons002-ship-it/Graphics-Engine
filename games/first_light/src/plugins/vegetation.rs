//! Lush valley vegetation: a procedurally grown forest, boulders, and bushes.
//!
//! Trees are real branching structures — a tapered trunk that forks into
//! several generations of limbs with leaf clusters at the tips — generated
//! once into a small set of merged meshes per species, then instanced.
//! Building per-variant meshes (rather than one entity per branch) keeps the
//! draw count tiny: ~380 trees share ~24 meshes. Placement is deterministic
//! (hash-based rejection sampling against the terrain functions).

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use std::collections::hash_map::DefaultHasher;
use std::f32::consts::TAU;
use std::hash::{Hash, Hasher};

use super::terrain::{
    CASTLE_CENTER, KNOLL_CENTER, LAKE_CENTER, LAKE_RADIUS, PLAYGROUND_CENTER, terrain_height,
    terrain_normal,
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

/// A small stateful generator for procedural tree growth.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / u32::MAX as f32).clamp(0.0, 1.0)
    }
    fn signed(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
    fn unit(&mut self) -> Vec3 {
        Vec3::new(self.signed(), self.signed(), self.signed()).normalize_or_zero()
    }
}

/// Accumulates transformed primitive geometry into one merged mesh.
#[derive(Default)]
struct MeshAccum {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    idx: Vec<u32>,
}

impl MeshAccum {
    fn push(&mut self, src: &Mesh, xf: Mat4) {
        let base = self.pos.len() as u32;
        let nmat = Mat3::from_mat4(xf);
        if let Some(VertexAttributeValues::Float32x3(ps)) = src.attribute(Mesh::ATTRIBUTE_POSITION) {
            for p in ps {
                self.pos.push(xf.transform_point3(Vec3::from_array(*p)).to_array());
            }
        }
        if let Some(VertexAttributeValues::Float32x3(ns)) = src.attribute(Mesh::ATTRIBUTE_NORMAL) {
            for n in ns {
                self.nrm
                    .push((nmat * Vec3::from_array(*n)).normalize_or_zero().to_array());
            }
        }
        if let Some(indices) = src.indices() {
            for i in indices.iter() {
                self.idx.push(base + i as u32);
            }
        }
    }

    fn finish(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Shared unit primitives, transformed and merged into trees.
struct Parts {
    cyl: Mesh,
    blob: Mesh,
}

/// A tapered limb (cylinder) from `a` to `b` with bottom radius `r`.
fn limb(acc: &mut MeshAccum, parts: &Parts, a: Vec3, b: Vec3, r: f32) {
    let along = b - a;
    let len = along.length();
    if len < 1e-3 {
        return;
    }
    let rot = Quat::from_rotation_arc(Vec3::Y, along / len);
    let xf = Mat4::from_scale_rotation_translation(Vec3::new(r, len, r), rot, (a + b) * 0.5);
    acc.push(&parts.cyl, xf);
}

/// A leaf/foliage clump (squashed sphere) at `center`.
fn blob(acc: &mut MeshAccum, parts: &Parts, center: Vec3, scale: Vec3, rng: &mut Rng) {
    let rot = Quat::from_euler(EulerRot::XYZ, rng.signed(), rng.signed(), rng.signed());
    acc.push(&parts.blob, Mat4::from_scale_rotation_translation(scale, rot, center));
}

/// A broadleaf (oak) limb that recursively forks, ending in leaf clumps.
#[allow(clippy::too_many_arguments)]
fn oak_branch(
    wood: &mut MeshAccum,
    leaf: &mut MeshAccum,
    parts: &Parts,
    rng: &mut Rng,
    base: Vec3,
    dir: Vec3,
    len: f32,
    rad: f32,
    depth: u32,
) {
    let tip = base + dir * len;
    limb(wood, parts, base, tip, rad);
    if depth == 0 || rad < 0.05 {
        for _ in 0..3 {
            let off = rng.unit() * len * 0.35;
            blob(leaf, parts, tip + off, Vec3::splat(0.5 + rad * 9.0), rng);
        }
    } else {
        let n = 2 + (rng.next() > 0.45) as u32;
        for _ in 0..n {
            let new_dir = (dir * 0.55 + rng.unit() * 0.6 + Vec3::Y * 0.18).normalize_or_zero();
            oak_branch(wood, leaf, parts, rng, tip, new_dir, len * 0.72, rad * 0.62, depth - 1);
        }
    }
}

/// Grows a broadleaf tree into the wood/leaf accumulators.
fn grow_oak(wood: &mut MeshAccum, leaf: &mut MeshAccum, parts: &Parts, rng: &mut Rng) {
    let trunk = 2.6 + rng.next() * 0.8;
    oak_branch(wood, leaf, parts, rng, Vec3::ZERO, Vec3::Y, trunk, 0.34, 4);
}

/// Grows a conifer: straight tapered trunk, whorls of drooping branches with
/// needle clumps, and a leafy spire.
fn grow_pine(wood: &mut MeshAccum, leaf: &mut MeshAccum, parts: &Parts, rng: &mut Rng) {
    let h = 7.0 + rng.next() * 3.0;
    let segs = 8;
    for k in 0..segs {
        let y0 = h * k as f32 / segs as f32;
        let y1 = h * (k + 1) as f32 / segs as f32;
        let r = 0.30 * (1.0 - k as f32 / segs as f32) + 0.04;
        limb(wood, parts, Vec3::new(0.0, y0, 0.0), Vec3::new(0.0, y1, 0.0), r);
    }
    let whorls = 7;
    for w in 0..whorls {
        let t = (w as f32 + 1.0) / (whorls as f32 + 1.0);
        let y = h * (0.22 + 0.72 * t);
        let branch_len = (1.0 - t) * 2.3 + 0.6;
        let droop = -0.22 - 0.22 * (1.0 - t);
        let count = 5;
        for b in 0..count {
            let ang = (b as f32 / count as f32) * TAU + w as f32 * 0.5;
            let dir = Vec3::new(ang.cos(), droop, ang.sin()).normalize_or_zero();
            let start = Vec3::new(0.0, y, 0.0);
            let tip = start + dir * branch_len;
            limb(wood, parts, start, tip, 0.05);
            blob(leaf, parts, start + dir * branch_len * 0.55, Vec3::new(branch_len * 0.55, 0.32, branch_len * 0.55), rng);
            blob(leaf, parts, tip, Vec3::new(0.5, 0.28, 0.5), rng);
        }
    }
    blob(leaf, parts, Vec3::new(0.0, h + 0.4, 0.0), Vec3::new(0.7, 1.3, 0.7), rng);
}

/// A grown tree variant: merged wood + leaf meshes and its leaf material.
struct TreeVariant {
    wood: Handle<Mesh>,
    leaf: Handle<Mesh>,
    leaf_material: Handle<StandardMaterial>,
}

fn spawn_vegetation(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let parts = Parts {
        cyl: Cylinder::new(1.0, 1.0).mesh().resolution(6).build(),
        blob: Sphere::new(1.0).mesh().ico(1).unwrap(),
    };

    let trunk_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.20, 0.12),
        perceptual_roughness: 0.95,
        ..default()
    });
    let make_leaf = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let pine_greens = [
        Color::srgb(0.07, 0.26, 0.09),
        Color::srgb(0.10, 0.31, 0.11),
        Color::srgb(0.06, 0.22, 0.10),
        Color::srgb(0.13, 0.34, 0.12),
    ];
    let oak_greens = [
        Color::srgb(0.22, 0.40, 0.11),
        Color::srgb(0.28, 0.44, 0.14),
        Color::srgb(0.18, 0.35, 0.10),
    ];

    // Pre-grow a handful of variants of each species.
    let mut pine_variants = Vec::new();
    for v in 0..6u64 {
        let mut rng = Rng(0x51E_D + v.wrapping_mul(2654435761));
        let (mut wood, mut leaf) = (MeshAccum::default(), MeshAccum::default());
        grow_pine(&mut wood, &mut leaf, &parts, &mut rng);
        pine_variants.push(TreeVariant {
            wood: meshes.add(wood.finish()),
            leaf: meshes.add(leaf.finish()),
            leaf_material: make_leaf(&mut materials, pine_greens[v as usize % pine_greens.len()]),
        });
    }
    let mut oak_variants = Vec::new();
    for v in 0..4u64 {
        let mut rng = Rng(0x0A4 + v.wrapping_mul(40503));
        let (mut wood, mut leaf) = (MeshAccum::default(), MeshAccum::default());
        grow_oak(&mut wood, &mut leaf, &parts, &mut rng);
        oak_variants.push(TreeVariant {
            wood: meshes.add(wood.finish()),
            leaf: meshes.add(leaf.finish()),
            leaf_material: make_leaf(&mut materials, oak_greens[v as usize % oak_greens.len()]),
        });
    }

    let rock_mesh = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let bush_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
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

    // --- Trees ---------------------------------------------------------------
    let mut planted = 0;
    for i in 0..1400u64 {
        if planted >= 380 {
            break;
        }
        let x = (hash01((i, 1)) - 0.5) * 580.0;
        let z = (hash01((i, 2)) - 0.5) * 580.0;
        let h = terrain_height(x, z);
        let normal = terrain_normal(x, z);
        if !(0.5..=58.0).contains(&h) || normal.y < 0.88 || !clear_of_landmarks(x, z) {
            continue;
        }
        planted += 1;

        let s = 0.8 + hash01((i, 3)) * 0.9;
        let lean = Quat::from_euler(
            EulerRot::XYZ,
            (hash01((i, 5)) - 0.5) * 0.10,
            hash01((i, 6)) * TAU,
            (hash01((i, 7)) - 0.5) * 0.10,
        );
        // Oaks on the warm valley floor, pines everywhere (and up the slopes).
        let oak = i % 3 == 1 && h < 25.0;
        let variant = if oak {
            &oak_variants[(i as usize) % oak_variants.len()]
        } else {
            &pine_variants[(i as usize) % pine_variants.len()]
        };

        commands
            .spawn((
                Transform::from_xyz(x, h, z)
                    .with_rotation(lean)
                    .with_scale(Vec3::splat(s)),
                Visibility::default(),
                RigidBody::Static,
                Collider::cylinder(0.3, 3.0),
                children![
                    (
                        Mesh3d(variant.wood.clone()),
                        MeshMaterial3d(trunk_material.clone()),
                    ),
                    (
                        Mesh3d(variant.leaf.clone()),
                        MeshMaterial3d(variant.leaf_material.clone()),
                    ),
                ],
            ));
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
                .with_rotation(Quat::from_rotation_y(hash01((i, 15)) * TAU))
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

/// Keeps vegetation off gameplay areas: lake, playground pad, castle terrace,
/// the knoll, and the causeway.
fn clear_of_landmarks(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    if p.distance(LAKE_CENTER) < LAKE_RADIUS + 4.0 {
        return false;
    }
    if p.distance(PLAYGROUND_CENTER) < 30.0 {
        return false;
    }
    if p.distance(KNOLL_CENTER) < 38.0 {
        return false;
    }
    if p.distance(CASTLE_CENTER) < 120.0 {
        return false;
    }
    if x.abs() < 22.0 && (-200.0..=-55.0).contains(&z) {
        return false;
    }
    true
}
