//! Dense, full-map grass that sways in the wind entirely on the GPU.
//!
//! Blades are scattered deterministically across the grassy valley floor and
//! baked, in world space, into a handful of large merged meshes (one per ~24 m
//! chunk, so the renderer can frustum-cull them). Every blade carries a "bend"
//! weight in `uv.y` (0 at the root, 1 at the tip) and a per-blade phase in
//! `uv.x`. A custom vertex shader — layered on top of `StandardMaterial` via
//! [`ExtendedMaterial`] so the grass keeps full PBR shadows, atmosphere ambient
//! and fog — pushes each vertex along a travelling wind wave weighted by that
//! bend. There is zero per-frame CPU work: the whole meadow ripples on the GPU,
//! and the blades cost only a few draw calls no matter how many there are.

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use std::f32::consts::TAU;

use super::terrain::{
    CASTLE_CENTER, KNOLL_CENTER, LAKE_CENTER, LAKE_RADIUS, terrain_height, terrain_normal,
};

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/grass.wgsl");
        embedded_asset!(app, "shaders/grass_prepass.wgsl");
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
            .add_systems(Startup, spawn_grass)
            .add_systems(Update, drive_wind);
    }
}

/// `StandardMaterial` extended with a wind-displacement vertex shader.
type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassWind>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct GrassWind {
    /// x = sway strength (metres at the tip), y = wind speed multiplier.
    #[uniform(100)]
    params: Vec4,
}

/// Holds the one grass material so [`drive_wind`] can advance its clock.
#[derive(Resource)]
struct GrassMat(Handle<GrassMaterial>);

impl MaterialExtension for GrassWind {
    fn vertex_shader() -> ShaderRef {
        "embedded://first_light/plugins/shaders/grass.wgsl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://first_light/plugins/shaders/grass_prepass.wgsl".into()
    }
}

fn hash(x: i32, z: i32, s: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add((z as u32).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(s.wrapping_mul(0xC2B2_AE35));
    h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    ((h >> 16) & 0xFFFF) as f32 / 65536.0
}

/// Keep grass off water, the castle terrace, and the knoll.
fn grassy(x: f32, z: f32) -> bool {
    let p = Vec2::new(x, z);
    p.distance(LAKE_CENTER) > LAKE_RADIUS + 2.0
        && p.distance(CASTLE_CENTER) > 70.0
        && p.distance(KNOLL_CENTER) > 14.0
}

// Scatter parameters. The map is large, so density and spacing are the dials
// to trade lushness against vertex count.
const SPACING: f32 = 0.6; // metres between blade clumps (before jitter)
const BLADES_PER_CLUMP: usize = 3;
const CHUNK: f32 = 24.0; // merged-mesh / frustum-cull granularity
const REGION: (f32, f32, f32, f32) = (-150.0, 150.0, -110.0, 210.0);

/// Accumulates blade geometry for one chunk before it becomes a mesh.
#[derive(Default)]
struct ChunkMesh {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl ChunkMesh {
    /// Append one blade rooted at `(x, y, z)`. `phase` (0..1) offsets its place
    /// in the wind wave; `shade` is its base colour (linear RGB).
    fn add_blade(&mut self, x: f32, y: f32, z: f32, yaw: f32, h: f32, phase: f32, shade: Vec3) {
        let dir = Vec2::new(yaw.cos(), yaw.sin());
        let perp = Vec2::new(-dir.y, dir.x);
        let w = 0.045;
        let lean = 0.10 + h * 0.25;

        let base = self.pos.len() as u32;
        // base-left, base-right, mid-left, mid-right, tip — with the bend weight
        // (uv.y) rising from root to tip and a touch of ambient darkening low down.
        let verts: [([f32; 3], f32); 5] = [
            ([x - perp.x * w, y, z - perp.y * w], 0.0),
            ([x + perp.x * w, y, z + perp.y * w], 0.0),
            (
                [
                    x - perp.x * w * 0.6 + dir.x * lean * 0.5,
                    y + h * 0.55,
                    z - perp.y * w * 0.6 + dir.y * lean * 0.5,
                ],
                0.55,
            ),
            (
                [
                    x + perp.x * w * 0.6 + dir.x * lean * 0.5,
                    y + h * 0.55,
                    z + perp.y * w * 0.6 + dir.y * lean * 0.5,
                ],
                0.55,
            ),
            ([x + dir.x * lean, y + h, z + dir.y * lean], 1.0),
        ];
        for (p, bend) in verts {
            self.pos.push(p);
            // Mostly-up normal so blades catch sky/sun light evenly.
            self.nrm.push([perp.x * 0.2, 1.0, perp.y * 0.2]);
            self.uv.push([phase, bend]);
            // Darker at the root (ambient occlusion), full colour at the tip.
            let ao = 0.5 + 0.5 * bend;
            self.col.push([shade.x * ao, shade.y * ao, shade.z * ao, 1.0]);
        }
        self.idx.extend_from_slice(&[
            base, base + 1, base + 3,
            base, base + 3, base + 2,
            base + 2, base + 3, base + 4,
        ]);
    }

    fn into_mesh(self) -> Mesh {
        use bevy::asset::RenderAssetUsages;
        use bevy::mesh::{Indices, Mesh as M};
        use bevy::render::render_resource::PrimitiveTopology;
        M::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(M::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(M::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(M::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(M::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

fn spawn_grass(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    // One shared material; per-blade colour variation rides in the vertex colors,
    // so base_color stays white and just lets those through.
    let material = materials.add(GrassMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: GrassWind {
            params: Vec4::new(0.35, 1.0, 0.0, 0.0),
        },
    });
    commands.insert_resource(GrassMat(material.clone()));

    // A few green shades, stored linear so the baked vertex colours match.
    let shades: Vec<Vec3> = [
        Color::srgb(0.20, 0.40, 0.12),
        Color::srgb(0.26, 0.46, 0.15),
        Color::srgb(0.16, 0.34, 0.10),
        Color::srgb(0.30, 0.48, 0.18),
    ]
    .iter()
    .map(|c| {
        let l = c.to_linear();
        Vec3::new(l.red, l.green, l.blue)
    })
    .collect();

    let mut chunks: HashMap<(i32, i32), ChunkMesh> = HashMap::default();
    let (x0, x1, z0, z1) = REGION;

    let mut col = 0i32;
    let mut gx = x0;
    while gx < x1 {
        let mut row = 0i32;
        let mut gz = z0;
        while gz < z1 {
            let cx = gx + (hash(col, row, 5) - 0.5) * SPACING;
            let cz = gz + (hash(col, row, 6) - 0.5) * SPACING;
            let height = terrain_height(cx, cz);
            let n = terrain_normal(cx, cz);
            if (0.1..=40.0).contains(&height) && n.y > 0.88 && grassy(cx, cz) {
                let key = ((cx / CHUNK).floor() as i32, (cz / CHUNK).floor() as i32);
                let chunk = chunks.entry(key).or_default();
                for b in 0..BLADES_PER_CLUMP {
                    let bi = b as i32;
                    // Spread the clump's blades around its point.
                    let off_a = hash(col * 7 + bi, row, 9) * TAU;
                    let off_r = hash(col, row * 7 + bi, 10) * 0.22;
                    let bx = cx + off_a.cos() * off_r;
                    let bz = cz + off_a.sin() * off_r;
                    let by = terrain_height(bx, bz);
                    let yaw = hash(col + bi, row, 7) * TAU;
                    let h = 0.32 + hash(col, row + bi, 8) * 0.36;
                    let phase = hash(col * 3 + bi, row * 5, 11);
                    let shade = shades[(hash(col + bi, row, 12) * shades.len() as f32) as usize
                        % shades.len()];
                    chunk.add_blade(bx, by, bz, yaw, h, phase, shade);
                }
            }
            gz += SPACING;
            row += 1;
        }
        gx += SPACING;
        col += 1;
    }

    let mut blades = 0usize;
    let chunk_count = chunks.len();
    for (_key, chunk) in chunks {
        blades += chunk.pos.len() / 5;
        commands.spawn((
            Mesh3d(meshes.add(chunk.into_mesh())),
            MeshMaterial3d(material.clone()),
            // World-space positions baked in, so spawn at the origin.
            Transform::IDENTITY,
        ));
    }
    info!("grass: {blades} blades across {chunk_count} chunks");
}

/// Feed elapsed time into the wind uniform so the GPU wave keeps travelling.
fn drive_wind(
    time: Res<Time>,
    mat: Option<Res<GrassMat>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    let Some(mat) = mat else { return };
    if let Some(m) = materials.get_mut(&mat.0) {
        m.extension.params.z = time.elapsed_secs();
    }
}
