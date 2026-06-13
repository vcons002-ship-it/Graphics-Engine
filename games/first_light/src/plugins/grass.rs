//! A field of grass tufts over the valley floor that bend in the wind.
//!
//! Each tuft is a small cluster of tapered blades sharing one mesh, scattered
//! deterministically on the grassy, gently sloped ground and instanced. A
//! travelling wind wave tilts each tuft from its base so the whole meadow
//! ripples — cheap (one transform write per tuft) and entirely GPU-instanced.

use bevy::prelude::*;
use std::f32::consts::TAU;

use super::terrain::{
    CASTLE_CENTER, KNOLL_CENTER, LAKE_CENTER, LAKE_RADIUS, terrain_height, terrain_normal,
};

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_grass)
            .add_systems(Update, sway_grass);
    }
}

/// A swaying tuft: `phase` offsets its place in the travelling wind wave;
/// `yaw` is its fixed facing.
#[derive(Component)]
struct GrassTuft {
    phase: f32,
    yaw: f32,
}

fn hash(x: i32, z: i32, s: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add((z as u32).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(s.wrapping_mul(0xC2B2_AE35));
    h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    ((h >> 16) & 0xFFFF) as f32 / 65536.0
}

/// One tuft of `blades` tapered blades fanned around the base, rooted at y=0
/// so tilting the entity bends them from the ground.
fn tuft_mesh() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, Mesh as M};
    use bevy::render::render_resource::PrimitiveTopology;

    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let blades = 7;
    for b in 0..blades {
        let yaw = b as f32 / blades as f32 * TAU + hash(b, 0, 1) * 0.8;
        let dir = Vec2::new(yaw.cos(), yaw.sin());
        let perp = Vec2::new(-dir.y, dir.x);
        let bw = 0.05;
        let h = 0.28 + hash(b, 0, 2) * 0.22;
        let lean = 0.10 + hash(b, 0, 3) * 0.08;
        let c = dir * (0.02 + hash(b, 0, 4) * 0.06);
        let bl = [c.x - perp.x * bw, 0.0, c.y - perp.y * bw];
        let br = [c.x + perp.x * bw, 0.0, c.y + perp.y * bw];
        let mid_l = [c.x - perp.x * bw * 0.5 + dir.x * lean * 0.5, h * 0.55, c.y - perp.y * bw * 0.5 + dir.y * lean * 0.5];
        let mid_r = [c.x + perp.x * bw * 0.5 + dir.x * lean * 0.5, h * 0.55, c.y + perp.y * bw * 0.5 + dir.y * lean * 0.5];
        let tip = [c.x + dir.x * lean, h, c.y + dir.y * lean];
        let base = pos.len() as u32;
        for p in [bl, br, mid_l, mid_r, tip] {
            pos.push(p);
            nrm.push([0.0, 1.0, 0.0]);
        }
        // base quad (bl,br,mid_r,mid_l) + tip triangle (mid_l,mid_r,tip)
        idx.extend_from_slice(&[
            base, base + 1, base + 3,
            base, base + 3, base + 2,
            base + 2, base + 3, base + 4,
        ]);
    }
    M::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(M::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(M::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_indices(Indices::U32(idx))
}

/// Keep grass off water, the castle terrace, and the knoll.
fn grassy(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    p.distance(LAKE_CENTER) > LAKE_RADIUS + 2.0
        && p.distance(CASTLE_CENTER) > 70.0
        && p.distance(KNOLL_CENTER) > 14.0
}

const GRASS_MAX: usize = 5500;

fn spawn_grass(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(tuft_mesh());
    // A few green shades; blades are thin so render both faces.
    let mats: Vec<_> = [
        Color::srgb(0.20, 0.40, 0.12),
        Color::srgb(0.26, 0.46, 0.15),
        Color::srgb(0.16, 0.34, 0.10),
        Color::srgb(0.30, 0.48, 0.18),
    ]
    .map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.95,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    })
    .to_vec();

    // Jittered grid over the playable valley floor.
    let spacing = 2.1_f32;
    let (x0, x1, z0, z1) = (-95.0, 95.0, -55.0, 150.0);
    let mut planted = 0;
    let mut gx = x0;
    let mut col = 0i32;
    while gx < x1 && planted < GRASS_MAX {
        let mut gz = z0;
        let mut row = 0i32;
        while gz < z1 && planted < GRASS_MAX {
            let x = gx + (hash(col, row, 5) - 0.5) * spacing;
            let z = gz + (hash(col, row, 6) - 0.5) * spacing;
            let h = terrain_height(x, z);
            let n = terrain_normal(x, z);
            if (0.1..=40.0).contains(&h) && n.y > 0.9 && grassy(x, z) {
                let yaw = hash(col, row, 7) * TAU;
                let s = 0.8 + hash(col, row, 8) * 0.7;
                commands.spawn((
                    GrassTuft {
                        phase: (x + z) * 0.12,
                        yaw,
                    },
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mats[(planted) % mats.len()].clone()),
                    Transform::from_xyz(x, h, z)
                        .with_rotation(Quat::from_rotation_y(yaw))
                        .with_scale(Vec3::splat(s)),
                ));
                planted += 1;
            }
            gz += spacing;
            row += 1;
        }
        gx += spacing;
        col += 1;
    }
}

fn sway_grass(time: Res<Time>, mut tufts: Query<(&GrassTuft, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (tuft, mut transform) in &mut tufts {
        // Travelling gust wave plus a faster flutter; tilt about Z (wind
        // blows along +X) bending the blades over from their roots.
        let gust = (t * 1.3 + tuft.phase).sin() * 0.5 + (t * 3.1 + tuft.phase * 1.7).sin() * 0.18;
        let tilt = 0.16 + 0.13 * gust;
        transform.rotation = Quat::from_rotation_z(-tilt) * Quat::from_rotation_y(tuft.yaw);
    }
}
