//! A stone castle on the headwall terrace overlooking the valley.
//!
//! Built parametrically from shared primitive meshes: a crenellated curtain
//! wall with round corner towers, a twin-towered gatehouse opening onto the
//! causeway, an inner keep with corner turrets and a great tower, courtyard
//! buildings, and warm-lit windows. Large surfaces carry a procedurally
//! generated ashlar-block texture, tiled per piece so the masonry stays
//! ~1 m scale no matter the wall size. Walls, towers, and buildings have
//! colliders; decoration (merlons, roofs, windows) does not.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::TAU;

use super::terrain::{CASTLE_CENTER, TERRACE_HEIGHT};

pub struct CastlePlugin;

impl Plugin for CastlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_castle);
    }
}

/// Curtain wall footprint (local space, castle centered at origin, gate
/// facing +Z toward the valley).
const WALL_HALF_X: f32 = 34.0;
const WALL_HALF_Z: f32 = 27.0;
const WALL_HEIGHT: f32 = 13.0;
const WALL_THICKNESS: f32 = 2.6;
const GATE_HALF_WIDTH: f32 = 4.0;

/// Masonry block size in meters (texture tiling period).
const BRICK_W: f32 = 1.4;
const BRICK_H: f32 = 0.7;

struct CastleAssets {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    cone: Handle<Mesh>,
    stone_texture: Handle<Image>,
    /// Untextured stone for small decoration (merlons etc.).
    trim: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    banner: Handle<StandardMaterial>,
}

/// Generates a tileable ashlar-masonry texture: offset courses of blocks
/// with darker mortar lines and per-block value variation.
fn stone_texture() -> Image {
    const SIZE: usize = 256;
    /// Pixels per block course (8 courses per tile).
    const COURSE: usize = SIZE / 8;
    const BLOCK: usize = SIZE / 4;
    const MORTAR: usize = 2;

    fn hash(a: u32, b: u32) -> f32 {
        let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
        h = (h ^ (h >> 13)).wrapping_mul(0xC2B2_AE35);
        ((h >> 16) & 0xFF) as f32 / 255.0
    }

    let mut data = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        let course = y / COURSE;
        // Alternate courses are offset by half a block.
        let offset = if course % 2 == 0 { 0 } else { BLOCK / 2 };
        for x in 0..SIZE {
            let bx = (x + offset) % SIZE / BLOCK;
            let in_mortar = y % COURSE < MORTAR || (x + offset) % BLOCK < MORTAR;
            let value = if in_mortar {
                0.42
            } else {
                // Per-block tone + a little pixel grain.
                0.78 + (hash(bx as u32, course as u32) - 0.5) * 0.18
                    + (hash(x as u32, y as u32) - 0.5) * 0.06
            };
            let v = (value.clamp(0.0, 1.0) * 255.0) as u8;
            data.extend_from_slice(&[v, v, v, 255]);
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    image
}

fn spawn_castle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let assets = CastleAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(0.5, 1.0)),
        cone: meshes.add(Cone {
            radius: 0.5,
            height: 1.0,
        }),
        stone_texture: images.add(stone_texture()),
        trim: materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.51, 0.48),
            perceptual_roughness: 0.92,
            ..default()
        }),
        slate: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.20, 0.27),
            perceptual_roughness: 0.6,
            ..default()
        }),
        wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.32, 0.22, 0.12),
            perceptual_roughness: 0.85,
            ..default()
        }),
        window: materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.04, 0.03),
            emissive: LinearRgba::rgb(2.0, 1.2, 0.5) * 1_500.0,
            ..default()
        }),
        banner: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.08, 0.08),
            perceptual_roughness: 0.8,
            ..default()
        }),
    };

    let origin = Vec3::new(CASTLE_CENTER.x, TERRACE_HEIGHT, CASTLE_CENTER.y);
    let c = &mut commands;
    let m = &mut materials;
    let a = &assets;

    // --- Curtain walls -------------------------------------------------------
    wall(c, m, a, origin, Vec3::new(0.0, 0.0, -WALL_HALF_Z), WALL_HALF_X * 2.0 + WALL_THICKNESS, 0.0);
    wall(c, m, a, origin, Vec3::new(-WALL_HALF_X, 0.0, 0.0), WALL_HALF_Z * 2.0, 90.0);
    wall(c, m, a, origin, Vec3::new(WALL_HALF_X, 0.0, 0.0), WALL_HALF_Z * 2.0, 90.0);
    let seg = WALL_HALF_X - GATE_HALF_WIDTH;
    wall(c, m, a, origin, Vec3::new(-(GATE_HALF_WIDTH + seg / 2.0), 0.0, WALL_HALF_Z), seg, 0.0);
    wall(c, m, a, origin, Vec3::new(GATE_HALF_WIDTH + seg / 2.0, 0.0, WALL_HALF_Z), seg, 0.0);

    // --- Corner towers --------------------------------------------------------
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        tower(c, m, a, origin + Vec3::new(sx * WALL_HALF_X, 0.0, sz * WALL_HALF_Z), 6.5, 22.0);
    }

    // --- Gatehouse --------------------------------------------------------------
    for sx in [-1.0, 1.0] {
        tower(c, m, a, origin + Vec3::new(sx * (GATE_HALF_WIDTH + 3.0), 0.0, WALL_HALF_Z + 1.2), 3.8, 18.0);
    }
    textured_block(c, m, a, origin + Vec3::new(0.0, 12.5, WALL_HALF_Z), Vec3::new(GATE_HALF_WIDTH * 2.0 + 6.0, 5.0, 4.8), true);
    merlon_row(c, a, origin + Vec3::new(0.0, 15.0, WALL_HALF_Z + 1.9), GATE_HALF_WIDTH * 2.0 + 5.4, 0.0);
    // Raised portcullis visible in the gate arch.
    plain_block(c, a, a.wood.clone(), origin + Vec3::new(0.0, 11.2, WALL_HALF_Z + 0.2), Vec3::new(GATE_HALF_WIDTH * 2.0, 2.8, 0.4));

    // --- Wall-top crenellations ---------------------------------------------------
    merlon_row(c, a, origin + Vec3::new(0.0, WALL_HEIGHT, -WALL_HALF_Z - WALL_THICKNESS / 2.0 + 0.4), WALL_HALF_X * 2.0 - 10.0, 0.0);
    for sx in [-1.0, 1.0] {
        merlon_row(c, a, origin + Vec3::new(sx * (WALL_HALF_X + WALL_THICKNESS / 2.0 - 0.4), WALL_HEIGHT, 0.0), WALL_HALF_Z * 2.0 - 10.0, 90.0);
    }
    for sx in [-1.0, 1.0] {
        merlon_row(c, a, origin + Vec3::new(sx * (GATE_HALF_WIDTH + seg / 2.0), WALL_HEIGHT, WALL_HALF_Z + WALL_THICKNESS / 2.0 - 0.4), seg - 7.0, 0.0);
    }

    // --- Keep -----------------------------------------------------------------------
    let keep_pos = origin + Vec3::new(0.0, 0.0, -7.0);
    let keep_size = Vec3::new(28.0, 24.0, 24.0);
    textured_block(c, m, a, keep_pos + Vec3::Y * keep_size.y / 2.0, keep_size, true);
    merlon_row(c, a, keep_pos + Vec3::new(0.0, keep_size.y, keep_size.z / 2.0 - 0.5), keep_size.x - 2.0, 0.0);
    merlon_row(c, a, keep_pos + Vec3::new(0.0, keep_size.y, -keep_size.z / 2.0 + 0.5), keep_size.x - 2.0, 0.0);
    merlon_row(c, a, keep_pos + Vec3::new(keep_size.x / 2.0 - 0.5, keep_size.y, 0.0), keep_size.z - 2.0, 90.0);
    merlon_row(c, a, keep_pos + Vec3::new(-keep_size.x / 2.0 + 0.5, keep_size.y, 0.0), keep_size.z - 2.0, 90.0);

    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let pos = keep_pos + Vec3::new(sx * (keep_size.x / 2.0 - 0.5), 10.0, sz * (keep_size.z / 2.0 - 0.5));
        turret(c, m, a, pos, 2.8, 20.0);
    }

    // Buttresses on the keep's valley-facing corners.
    for sx in [-1.0, 1.0] {
        textured_block(c, m, a, keep_pos + Vec3::new(sx * (keep_size.x / 2.0 + 0.7), 6.0, keep_size.z / 2.0 + 0.7), Vec3::new(1.8, 12.0, 1.8), true);
    }

    // Great tower rising behind the keep.
    let great_pos = keep_pos + Vec3::new(0.0, 0.0, -5.0);
    textured_block(c, m, a, great_pos + Vec3::Y * 19.0, Vec3::new(13.0, 38.0, 13.0), true);
    for (dx, dz, yaw) in [(0.0, 6.0, 0.0), (0.0, -6.0, 0.0), (6.0, 0.0, 90.0), (-6.0, 0.0, 90.0)] {
        merlon_row(c, a, great_pos + Vec3::new(dx, 38.0, dz), 11.0, yaw);
    }
    c.spawn((
        Mesh3d(a.cone.clone()),
        MeshMaterial3d(a.slate.clone()),
        Transform::from_translation(great_pos + Vec3::Y * 41.0).with_scale(Vec3::new(15.5, 6.5, 15.5)),
    ));
    c.spawn((
        Mesh3d(a.cylinder.clone()),
        MeshMaterial3d(a.wood.clone()),
        Transform::from_translation(great_pos + Vec3::Y * 47.5).with_scale(Vec3::new(0.22, 7.0, 0.22)),
    ));
    c.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(a.banner.clone()),
        Transform::from_translation(great_pos + Vec3::new(1.8, 50.2, 0.0)).with_scale(Vec3::new(3.4, 1.8, 0.12)),
    ));

    // Windows: keep's valley face and the great tower.
    for row in 0..3 {
        for col in 0..5 {
            let x = (col as f32 - 2.0) * 4.8;
            let y = 7.0 + row as f32 * 6.0;
            window(c, a, keep_pos + Vec3::new(x, y, keep_size.z / 2.0 + 0.05));
        }
    }
    for row in 0..5 {
        window(c, a, great_pos + Vec3::new(0.0, 22.0 + row as f32 * 3.6, 6.55));
    }

    // --- Courtyard buildings -----------------------------------------------------------
    let hall = origin + Vec3::new(-WALL_HALF_X + 7.5, 0.0, 9.0);
    textured_block(c, m, a, hall + Vec3::Y * 4.0, Vec3::new(9.5, 8.0, 17.0), true);
    plain_block(c, a, a.slate.clone(), hall + Vec3::Y * 8.8, Vec3::new(11.0, 1.8, 18.4));
    for i in 0..3 {
        window(c, a, hall + Vec3::new(4.8, 4.5, (i as f32 - 1.0) * 5.2));
    }
    let stables = origin + Vec3::new(WALL_HALF_X - 6.5, 0.0, 13.0);
    textured_block(c, m, a, stables + Vec3::Y * 2.5, Vec3::new(7.5, 5.0, 13.0), true);
    plain_block(c, a, a.wood.clone(), stables + Vec3::Y * 5.5, Vec3::new(8.6, 1.0, 14.0));
}

/// Stone material with the ashlar texture tiled to keep blocks ~`BRICK` size
/// on a face `width_m` by `height_m`.
fn stone_material(
    materials: &mut Assets<StandardMaterial>,
    assets: &CastleAssets,
    width_m: f32,
    height_m: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.58, 0.56, 0.52),
        base_color_texture: Some(assets.stone_texture.clone()),
        uv_transform: bevy::math::Affine2::from_scale(Vec2::new(
            (width_m / (BRICK_W * 4.0)).max(0.25),
            (height_m / (BRICK_H * 8.0)).max(0.25),
        )),
        perceptual_roughness: 0.92,
        ..default()
    })
}

/// A curtain-wall segment with collider and tiled masonry.
fn wall(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    assets: &CastleAssets,
    origin: Vec3,
    offset: Vec3,
    length: f32,
    yaw_deg: f32,
) {
    let material = stone_material(materials, assets, length, WALL_HEIGHT);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(origin + offset + Vec3::Y * WALL_HEIGHT / 2.0)
            .with_rotation(Quat::from_rotation_y(yaw_deg.to_radians()))
            .with_scale(Vec3::new(length, WALL_HEIGHT, WALL_THICKNESS)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
    ));
}

/// A round tower: textured shaft, battlement collar, merlon ring, slate roof.
fn tower(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    assets: &CastleAssets,
    base: Vec3,
    radius: f32,
    height: f32,
) {
    let material = stone_material(materials, assets, TAU * radius, height);
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(base + Vec3::Y * height / 2.0)
            .with_scale(Vec3::new(radius * 2.0, height, radius * 2.0)),
        RigidBody::Static,
        Collider::cylinder(0.5, 1.0),
    ));
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(assets.trim.clone()),
        Transform::from_translation(base + Vec3::Y * (height + 0.6))
            .with_scale(Vec3::new(radius * 2.4, 1.2, radius * 2.4)),
    ));
    let rim = radius * 1.2 - 0.45;
    let count = (rim * TAU / 2.4).round() as usize;
    for k in 0..count {
        let angle = k as f32 / count as f32 * TAU;
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.trim.clone()),
            Transform::from_translation(base + Vec3::new(angle.cos() * rim, height + 1.9, angle.sin() * rim))
                .with_rotation(Quat::from_rotation_y(-angle))
                .with_scale(Vec3::new(0.8, 1.4, 1.2)),
        ));
    }
    commands.spawn((
        Mesh3d(assets.cone.clone()),
        MeshMaterial3d(assets.slate.clone()),
        Transform::from_translation(base + Vec3::Y * (height + 2.5 + 2.6))
            .with_scale(Vec3::new(radius * 2.6, 5.6 + radius, radius * 2.6)),
    ));
}

/// A slim keep turret with conical roof.
fn turret(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    assets: &CastleAssets,
    base: Vec3,
    radius: f32,
    height: f32,
) {
    let material = stone_material(materials, assets, TAU * radius, height);
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(base + Vec3::Y * height / 2.0)
            .with_scale(Vec3::new(radius * 2.0, height, radius * 2.0)),
        RigidBody::Static,
        Collider::cylinder(0.5, 1.0),
    ));
    commands.spawn((
        Mesh3d(assets.cone.clone()),
        MeshMaterial3d(assets.slate.clone()),
        Transform::from_translation(base + Vec3::Y * (height + 2.1))
            .with_scale(Vec3::new(radius * 2.5, 4.6, radius * 2.5)),
    ));
}

/// A textured stone block with collider.
fn textured_block(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    assets: &CastleAssets,
    center: Vec3,
    size: Vec3,
    collider: bool,
) {
    let material = stone_material(materials, assets, size.x.max(size.z), size.y);
    let mut entity = commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(center).with_scale(size),
    ));
    if collider {
        entity.insert((RigidBody::Static, Collider::cuboid(1.0, 1.0, 1.0)));
    }
}

/// An untextured block (wood, slate, trim).
fn plain_block(
    commands: &mut Commands,
    assets: &CastleAssets,
    material: Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
) {
    let _ = assets;
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(center).with_scale(size),
    ));
}

/// A row of merlons (crenellation teeth) along the given axis.
fn merlon_row(
    commands: &mut Commands,
    assets: &CastleAssets,
    center: Vec3,
    length: f32,
    yaw_deg: f32,
) {
    let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    let axis = rotation * Vec3::X;
    let count = (length / 2.6).floor().max(1.0) as usize;
    for k in 0..=count {
        let offset = (k as f32 / count as f32 - 0.5) * length;
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.trim.clone()),
            Transform::from_translation(center + axis * offset + Vec3::Y * 0.8)
                .with_rotation(rotation)
                .with_scale(Vec3::new(1.3, 1.6, 0.9)),
        ));
    }
}

/// A recessed, warmly lit window pane.
fn window(commands: &mut Commands, assets: &CastleAssets, center: Vec3) {
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.window.clone()),
        Transform::from_translation(center).with_scale(Vec3::new(1.2, 2.2, 0.25)),
    ));
}
