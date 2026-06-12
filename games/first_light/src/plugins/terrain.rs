//! Procedural mountain valley: a U-shaped glacial valley walled by mountains,
//! closed at the north (-Z) end by a headwall that carries the castle
//! terrace. One vertex-colored mesh (grass / rock / snow by height and
//! slope), an exactly matching heightfield collider, and a small lake.
//!
//! [`terrain_height`] is pure and deterministic — every other plugin
//! (castle, vegetation, props, player spawn) samples it so the whole scene
//! always sits on the ground.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh};
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain);
    }
}

/// Full terrain extent in meters (centered on the origin).
pub const TERRAIN_SIZE: f32 = 600.0;
/// Castle terrace: center, flat radius, blend-out radius, floor height.
pub const CASTLE_CENTER: Vec2 = Vec2::new(0.0, -190.0);
const TERRACE_FLAT_RADIUS: f32 = 58.0;
const TERRACE_BLEND_RADIUS: f32 = 95.0;
pub const TERRACE_HEIGHT: f32 = 58.0;
/// Spawn-area pad on the valley floor so the physics playground sits flat.
pub const PLAYGROUND_CENTER: Vec2 = Vec2::new(0.0, 60.0);
const PLAYGROUND_FLAT_RADIUS: f32 = 22.0;
const PLAYGROUND_BLEND_RADIUS: f32 = 40.0;
/// Lake basin on the valley floor.
pub const LAKE_CENTER: Vec2 = Vec2::new(-52.0, -10.0);
pub const LAKE_RADIUS: f32 = 38.0;
pub const WATER_LEVEL: f32 = -1.2;

/// Deterministic hash noise in [0, 1).
fn hash2(ix: i32, iz: i32) -> f32 {
    let mut h = (ix as i64).wrapping_mul(374_761_393) ^ (iz as i64).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65536.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Bilinear value noise in [0, 1).
fn value_noise(x: f32, z: f32) -> f32 {
    let (ix, iz) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - x.floor(), z - z.floor());
    let (sx, sz) = (smooth(fx), smooth(fz));
    let a = hash2(ix, iz);
    let b = hash2(ix + 1, iz);
    let c = hash2(ix, iz + 1);
    let d = hash2(ix + 1, iz + 1);
    a + (b - a) * sx + (c - a) * sz + (a - b - c + d) * sx * sz
}

/// Fractal noise, roughly in [-1, 1].
fn fbm(x: f32, z: f32, octaves: u32) -> f32 {
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        sum += amplitude * (value_noise(x * frequency, z * frequency) * 2.0 - 1.0);
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    sum / norm
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    smooth(((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0))
}

/// Terrain height before terraces are blended in.
fn raw_height(x: f32, z: f32) -> f32 {
    // U-shaped valley: side walls rise with |x|.
    let walls = smoothstep(45.0, 290.0, x.abs()).powf(1.4) * 150.0;
    // Headwall closing the valley to the north (-Z), carrying the castle.
    let headwall = smoothstep(-60.0, -300.0, z).powf(1.3) * 160.0;
    // Gentle rise toward the open south end too, so the valley feels held.
    let tail = smoothstep(180.0, 320.0, z) * 40.0;
    let base = walls + headwall + tail;

    // Rolling detail, rougher on the mountains than the meadow floor.
    let detail_amp = 2.0 + base * 0.16;
    let detail = fbm(x * 0.013 + 7.3, z * 0.013 - 3.1, 5) * detail_amp;

    // Meadow undulation.
    let meadow = fbm(x * 0.05 + 21.0, z * 0.05 + 13.0, 3) * 1.2;

    // Lake basin carved into the floor.
    let lake = (1.0 - smoothstep(0.0, LAKE_RADIUS, Vec2::new(x, z).distance(LAKE_CENTER))) * 5.0;

    base + detail + meadow - lake
}

/// Blends a flat disc into the height function (terrace / playground pad).
fn flatten(height: f32, x: f32, z: f32, center: Vec2, flat_r: f32, blend_r: f32, to: f32) -> f32 {
    let d = Vec2::new(x, z).distance(center);
    let t = 1.0 - smoothstep(flat_r, blend_r, d);
    height + (to - height) * t
}

/// Causeway: a broad processional ramp climbing from the valley floor to the
/// castle gate, carved into the headwall slope at a walkable ~25 degrees.
const RAMP_TOP_Z: f32 = -180.0;
const RAMP_BOTTOM_Z: f32 = -70.0;
const RAMP_HALF_WIDTH: f32 = 9.0;

fn ramp(height: f32, x: f32, z: f32) -> f32 {
    let along = smoothstep(RAMP_TOP_Z, RAMP_BOTTOM_Z, z);
    let target = TERRACE_HEIGHT + (2.0 - TERRACE_HEIGHT) * along;
    let lateral = 1.0 - smoothstep(RAMP_HALF_WIDTH, RAMP_HALF_WIDTH + 10.0, x.abs());
    let in_z = smoothstep(RAMP_BOTTOM_Z + 15.0, RAMP_BOTTOM_Z, z)
        * (1.0 - smoothstep(RAMP_TOP_Z, RAMP_TOP_Z - 15.0, z));
    height + (target - height) * lateral * in_z
}

/// World-space terrain height at (x, z). Pure and deterministic.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    let mut h = raw_height(x, z);
    h = flatten(
        h,
        x,
        z,
        CASTLE_CENTER,
        TERRACE_FLAT_RADIUS,
        TERRACE_BLEND_RADIUS,
        TERRACE_HEIGHT,
    );
    h = flatten(
        h,
        x,
        z,
        PLAYGROUND_CENTER,
        PLAYGROUND_FLAT_RADIUS,
        PLAYGROUND_BLEND_RADIUS,
        0.0,
    );
    ramp(h, x, z)
}

/// Terrain normal from central differences of the height function.
pub fn terrain_normal(x: f32, z: f32) -> Vec3 {
    const EPS: f32 = 1.0;
    let dx = terrain_height(x + EPS, z) - terrain_height(x - EPS, z);
    let dz = terrain_height(x, z + EPS) - terrain_height(x, z - EPS);
    Vec3::new(-dx, 2.0 * EPS, -dz).normalize()
}

/// Ground color: lush grass on the floor, rock on steep slopes, snow on the
/// peaks, sandy shore near the lake.
fn ground_color(x: f32, z: f32, height: f32, normal: Vec3) -> [f32; 4] {
    let slope = 1.0 - normal.y;
    let tint = fbm(x * 0.11 + 53.0, z * 0.11 - 41.0, 3);

    let grass = Vec3::new(0.13 + tint * 0.05, 0.36 + tint * 0.08, 0.10 + tint * 0.03);
    let rock = Vec3::new(0.38 + tint * 0.06, 0.35 + tint * 0.05, 0.33 + tint * 0.05);
    let snow = Vec3::new(0.92, 0.93, 0.96);
    let sand = Vec3::new(0.55, 0.5, 0.36);

    // Grass -> rock with slope (rock also creeps in with altitude).
    let rockiness =
        (smoothstep(0.18, 0.45, slope) + smoothstep(45.0, 110.0, height) * 0.6).min(1.0);
    let mut color = grass.lerp(rock, rockiness);

    // Snow above the snow line, but not on cliffs.
    let snowiness = smoothstep(95.0, 130.0, height) * (1.0 - smoothstep(0.35, 0.6, slope));
    color = color.lerp(snow, snowiness);

    // Sandy shoreline ring around the lake.
    let shore = Vec2::new(x, z).distance(LAKE_CENTER);
    let sandiness = (1.0 - smoothstep(0.0, LAKE_RADIUS * 0.85, shore))
        * (1.0 - smoothstep(0.18, 0.4, slope));
    color = color.lerp(sand, sandiness);

    [color.x, color.y, color.z, 1.0]
}

/// Mesh resolution (vertices per side) and collider resolution.
const MESH_RES: usize = 257;
const COLLIDER_RES: usize = 129;

fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // --- Visual mesh -------------------------------------------------------
    let n = MESH_RES;
    let step = TERRAIN_SIZE / (n - 1) as f32;
    let half = TERRAIN_SIZE / 2.0;

    let mut positions = Vec::with_capacity(n * n);
    let mut normals = Vec::with_capacity(n * n);
    let mut colors = Vec::with_capacity(n * n);
    let mut uvs = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = -half + i as f32 * step;
            let z = -half + j as f32 * step;
            let y = terrain_height(x, z);
            let normal = terrain_normal(x, z);
            positions.push([x, y, z]);
            normals.push([normal.x, normal.y, normal.z]);
            colors.push(ground_color(x, z, y, normal));
            uvs.push([i as f32 / (n - 1) as f32, j as f32 / (n - 1) as f32]);
        }
    }
    let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let i00 = (j * n + i) as u32;
            let i10 = i00 + 1;
            let i01 = i00 + n as u32;
            let i11 = i01 + 1;
            indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
        }
    }

    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));

    // --- Collider (parry heightfield: rows advance along Z, columns along X,
    // centered, spanning scale.x by scale.z) --------------------------------
    let cn = COLLIDER_RES;
    let cstep = TERRAIN_SIZE / (cn - 1) as f32;
    let heights: Vec<Vec<f32>> = (0..cn)
        .map(|j| {
            (0..cn)
                .map(|i| terrain_height(-half + i as f32 * cstep, -half + j as f32 * cstep))
                .collect()
        })
        .collect();

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.96,
            ..default()
        })),
        Transform::IDENTITY,
        RigidBody::Static,
        Collider::heightfield(heights, Vec3::new(TERRAIN_SIZE, 1.0, TERRAIN_SIZE)),
    ));

    // --- Lake water --------------------------------------------------------
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(LAKE_RADIUS * 2.2, LAKE_RADIUS * 2.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.3, 0.42, 0.72),
            perceptual_roughness: 0.08,
            metallic: 0.2,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(LAKE_CENTER.x, WATER_LEVEL, LAKE_CENTER.y),
    ));
}
