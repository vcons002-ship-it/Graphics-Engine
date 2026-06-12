//! A stone castle on the headwall terrace overlooking the valley.
//!
//! Built parametrically from shared primitive meshes: a crenellated curtain
//! wall with round corner towers, a twin-towered gatehouse opening onto the
//! causeway, an inner keep with corner turrets and a great tower, courtyard
//! buildings, and warm-lit windows. Walls, towers, and buildings carry
//! colliders; decoration (merlons, roofs, windows) does not.

use avian3d::prelude::*;
use bevy::prelude::*;
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
const WALL_HALF_X: f32 = 32.0;
const WALL_HALF_Z: f32 = 26.0;
const WALL_HEIGHT: f32 = 11.0;
const WALL_THICKNESS: f32 = 2.5;
const GATE_HALF_WIDTH: f32 = 3.5;

struct CastleAssets {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    cone: Handle<Mesh>,
    stone: Vec<Handle<StandardMaterial>>,
    slate: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    banner: Handle<StandardMaterial>,
}

fn spawn_castle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stone = [
        Color::srgb(0.52, 0.50, 0.46),
        Color::srgb(0.47, 0.45, 0.42),
        Color::srgb(0.56, 0.53, 0.48),
    ]
    .map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.92,
            ..default()
        })
    })
    .to_vec();

    let assets = CastleAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(0.5, 1.0)),
        cone: meshes.add(Cone {
            radius: 0.5,
            height: 1.0,
        }),
        stone,
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
    info!("castle: spawning at {origin}");
    let c = &mut commands;
    let a = &assets;

    // --- Curtain walls ------------------------------------------------------
    // Back wall (full length) and two side walls.
    wall(c, a, origin, Vec3::new(0.0, 0.0, -WALL_HALF_Z), WALL_HALF_X * 2.0 + WALL_THICKNESS, 0.0);
    wall(c, a, origin, Vec3::new(-WALL_HALF_X, 0.0, 0.0), WALL_HALF_Z * 2.0, 90.0);
    wall(c, a, origin, Vec3::new(WALL_HALF_X, 0.0, 0.0), WALL_HALF_Z * 2.0, 90.0);
    // Front wall: two segments flanking the gate opening.
    let seg = WALL_HALF_X - GATE_HALF_WIDTH;
    wall(c, a, origin, Vec3::new(-(GATE_HALF_WIDTH + seg / 2.0), 0.0, WALL_HALF_Z), seg, 0.0);
    wall(c, a, origin, Vec3::new(GATE_HALF_WIDTH + seg / 2.0, 0.0, WALL_HALF_Z), seg, 0.0);

    // --- Corner towers ------------------------------------------------------
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        tower(
            c,
            a,
            origin + Vec3::new(sx * WALL_HALF_X, 0.0, sz * WALL_HALF_Z),
            5.5,
            17.0,
            true,
        );
    }

    // --- Gatehouse ----------------------------------------------------------
    for sx in [-1.0, 1.0] {
        tower(
            c,
            a,
            origin + Vec3::new(sx * (GATE_HALF_WIDTH + 2.8), 0.0, WALL_HALF_Z + 1.0),
            3.4,
            15.0,
            true,
        );
    }
    // Lintel bridging the gate towers, with merlons on top.
    block(
        c,
        a,
        1,
        origin + Vec3::new(0.0, 10.0, WALL_HALF_Z),
        Vec3::new(GATE_HALF_WIDTH * 2.0 + 5.6, 4.0, 4.5),
        true,
    );
    merlon_row(c, a, origin + Vec3::new(0.0, 12.0, WALL_HALF_Z + 1.8), GATE_HALF_WIDTH * 2.0 + 5.0, 0.0);
    // Raised portcullis visible in the gate arch.
    block(
        c,
        a,
        usize::MAX, // wood
        origin + Vec3::new(0.0, 9.2, WALL_HALF_Z + 0.2),
        Vec3::new(GATE_HALF_WIDTH * 2.0, 2.4, 0.4),
        false,
    );

    // --- Wall-top crenellations ---------------------------------------------
    merlon_row(c, a, origin + Vec3::new(0.0, WALL_HEIGHT, -WALL_HALF_Z - WALL_THICKNESS / 2.0 + 0.4), WALL_HALF_X * 2.0 - 8.0, 0.0);
    for sx in [-1.0, 1.0] {
        merlon_row(c, a, origin + Vec3::new(sx * (WALL_HALF_X + WALL_THICKNESS / 2.0 - 0.4), WALL_HEIGHT, 0.0), WALL_HALF_Z * 2.0 - 8.0, 90.0);
    }
    for sx in [-1.0, 1.0] {
        merlon_row(
            c,
            a,
            origin + Vec3::new(sx * (GATE_HALF_WIDTH + seg / 2.0), WALL_HEIGHT, WALL_HALF_Z + WALL_THICKNESS / 2.0 - 0.4),
            seg - 6.0,
            0.0,
        );
    }

    // --- Keep ----------------------------------------------------------------
    let keep_pos = origin + Vec3::new(0.0, 0.0, -8.0);
    let keep_size = Vec3::new(24.0, 20.0, 21.0);
    block(c, a, 0, keep_pos + Vec3::Y * keep_size.y / 2.0, keep_size, true);
    merlon_row(c, a, keep_pos + Vec3::new(0.0, keep_size.y, keep_size.z / 2.0 - 0.5), keep_size.x - 2.0, 0.0);
    merlon_row(c, a, keep_pos + Vec3::new(0.0, keep_size.y, -keep_size.z / 2.0 + 0.5), keep_size.x - 2.0, 0.0);
    merlon_row(c, a, keep_pos + Vec3::new(keep_size.x / 2.0 - 0.5, keep_size.y, 0.0), keep_size.z - 2.0, 90.0);
    merlon_row(c, a, keep_pos + Vec3::new(-keep_size.x / 2.0 + 0.5, keep_size.y, 0.0), keep_size.z - 2.0, 90.0);

    // Keep corner turrets.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let pos = keep_pos + Vec3::new(sx * (keep_size.x / 2.0 - 0.5), 8.0, sz * (keep_size.z / 2.0 - 0.5));
        turret(c, a, pos, 2.4, 16.0);
    }

    // Buttresses on the keep's valley-facing corners.
    for sx in [-1.0, 1.0] {
        block(
            c,
            a,
            2,
            keep_pos + Vec3::new(sx * (keep_size.x / 2.0 + 0.6), 5.0, keep_size.z / 2.0 + 0.6),
            Vec3::new(1.6, 10.0, 1.6),
            true,
        );
    }

    // Great tower rising from the back of the keep.
    let great_pos = keep_pos + Vec3::new(0.0, 0.0, -4.0);
    block(c, a, 1, great_pos + Vec3::Y * 15.0, Vec3::new(11.0, 30.0, 11.0), true);
    merlon_row(c, a, great_pos + Vec3::new(0.0, 30.0, 5.0), 9.0, 0.0);
    merlon_row(c, a, great_pos + Vec3::new(0.0, 30.0, -5.0), 9.0, 0.0);
    merlon_row(c, a, great_pos + Vec3::new(5.0, 30.0, 0.0), 9.0, 90.0);
    merlon_row(c, a, great_pos + Vec3::new(-5.0, 30.0, 0.0), 9.0, 90.0);
    // Slate pyramid roof + banner.
    c.spawn((
        Mesh3d(a.cone.clone()),
        MeshMaterial3d(a.slate.clone()),
        Transform::from_translation(great_pos + Vec3::Y * 32.5).with_scale(Vec3::new(13.0, 5.0, 13.0)),
    ));
    c.spawn((
        Mesh3d(a.cylinder.clone()),
        MeshMaterial3d(a.wood.clone()),
        Transform::from_translation(great_pos + Vec3::Y * 38.0).with_scale(Vec3::new(0.22, 6.0, 0.22)),
    ));
    c.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(a.banner.clone()),
        Transform::from_translation(great_pos + Vec3::new(1.6, 40.0, 0.0)).with_scale(Vec3::new(3.0, 1.6, 0.12)),
    ));

    // Windows on the keep's valley face and the great tower.
    for row in 0..3 {
        for col in 0..5 {
            let x = (col as f32 - 2.0) * 4.2;
            let y = 6.0 + row as f32 * 5.0;
            window(c, a, keep_pos + Vec3::new(x, y, keep_size.z / 2.0 + 0.05));
        }
    }
    for row in 0..4 {
        window(c, a, great_pos + Vec3::new(0.0, 18.0 + row as f32 * 3.4, 5.55));
    }

    // --- Courtyard buildings --------------------------------------------------
    // Great hall along the west wall.
    let hall = origin + Vec3::new(-WALL_HALF_X + 7.0, 0.0, 8.0);
    block(c, a, 2, hall + Vec3::Y * 3.5, Vec3::new(9.0, 7.0, 16.0), true);
    c.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(a.slate.clone()),
        Transform::from_translation(hall + Vec3::Y * 7.8)
            .with_scale(Vec3::new(10.4, 1.6, 17.4))
            .with_rotation(Quat::from_rotation_z(0.0)),
    ));
    for i in 0..3 {
        window(c, a, hall + Vec3::new(4.55, 4.0, (i as f32 - 1.0) * 5.0));
    }
    // Stables along the east wall.
    let stables = origin + Vec3::new(WALL_HALF_X - 6.0, 0.0, 12.0);
    block(c, a, 1, stables + Vec3::Y * 2.2, Vec3::new(7.0, 4.4, 12.0), true);
    c.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(a.wood.clone()),
        Transform::from_translation(stables + Vec3::Y * 4.9).with_scale(Vec3::new(8.0, 1.0, 13.0)),
    ));
}

/// A curtain-wall segment centered at `offset` (local), `length` along its
/// axis, rotated `yaw_deg` around Y. Includes the collider.
fn wall(
    commands: &mut Commands,
    assets: &CastleAssets,
    origin: Vec3,
    offset: Vec3,
    length: f32,
    yaw_deg: f32,
) {
    let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.stone[0].clone()),
        Transform::from_translation(origin + offset + Vec3::Y * WALL_HEIGHT / 2.0)
            .with_rotation(rotation)
            .with_scale(Vec3::new(length, WALL_HEIGHT, WALL_THICKNESS)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
    ));
    // Crenellations ride on the outer edge via merlon_row at the call site.
}

/// A round tower with battlement ring and optional conical roof.
fn tower(
    commands: &mut Commands,
    assets: &CastleAssets,
    base: Vec3,
    radius: f32,
    height: f32,
    roof: bool,
) {
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(assets.stone[1].clone()),
        Transform::from_translation(base + Vec3::Y * height / 2.0)
            .with_scale(Vec3::new(radius * 2.0, height, radius * 2.0)),
        RigidBody::Static,
        Collider::cylinder(0.5, 1.0),
    ));
    // Slightly wider battlement collar at the top.
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(assets.stone[2].clone()),
        Transform::from_translation(base + Vec3::Y * (height + 0.6))
            .with_scale(Vec3::new(radius * 2.4, 1.2, radius * 2.4)),
    ));
    // Merlons around the rim.
    let rim = radius * 1.2 - 0.45;
    let count = (rim * TAU / 2.4).round() as usize;
    for k in 0..count {
        let angle = k as f32 / count as f32 * TAU;
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.stone[2].clone()),
            Transform::from_translation(
                base + Vec3::new(angle.cos() * rim, height + 1.9, angle.sin() * rim),
            )
            .with_rotation(Quat::from_rotation_y(-angle))
            .with_scale(Vec3::new(0.8, 1.4, 1.2)),
        ));
    }
    if roof {
        commands.spawn((
            Mesh3d(assets.cone.clone()),
            MeshMaterial3d(assets.slate.clone()),
            Transform::from_translation(base + Vec3::Y * (height + 2.5 + 2.6))
                .with_scale(Vec3::new(radius * 2.6, 5.2 + radius, radius * 2.6)),
        ));
    }
}

/// A slim keep turret (no separate battlements, conical roof).
fn turret(commands: &mut Commands, assets: &CastleAssets, base: Vec3, radius: f32, height: f32) {
    commands.spawn((
        Mesh3d(assets.cylinder.clone()),
        MeshMaterial3d(assets.stone[2].clone()),
        Transform::from_translation(base + Vec3::Y * height / 2.0)
            .with_scale(Vec3::new(radius * 2.0, height, radius * 2.0)),
        RigidBody::Static,
        Collider::cylinder(0.5, 1.0),
    ));
    commands.spawn((
        Mesh3d(assets.cone.clone()),
        MeshMaterial3d(assets.slate.clone()),
        Transform::from_translation(base + Vec3::Y * (height + 1.9))
            .with_scale(Vec3::new(radius * 2.5, 4.2, radius * 2.5)),
    ));
}

/// A stone block; `material` indexes the stone variants (`usize::MAX` =
/// wood). `collider` adds a matching static cuboid.
fn block(
    commands: &mut Commands,
    assets: &CastleAssets,
    material: usize,
    center: Vec3,
    size: Vec3,
    collider: bool,
) {
    let material = if material == usize::MAX {
        assets.wood.clone()
    } else {
        assets.stone[material % assets.stone.len()].clone()
    };
    let mut entity = commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(center).with_scale(size),
    ));
    if collider {
        entity.insert((RigidBody::Static, Collider::cuboid(1.0, 1.0, 1.0)));
    }
}

/// A row of merlons (crenellation teeth) centered at `center`, spread along
/// the row's axis (`yaw_deg` 0 = X axis), spaced ~2.6 m.
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
            MeshMaterial3d(assets.stone[2].clone()),
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
        Transform::from_translation(center).with_scale(Vec3::new(1.1, 2.0, 0.25)),
    ));
}
