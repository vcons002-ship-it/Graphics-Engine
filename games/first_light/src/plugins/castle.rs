//! A stone castle on the headwall terrace — built entirely from individual
//! mortared masonry blocks (see `masonry.rs`), so every wall, tower, and
//! merlon can be knocked down with physics.
//!
//! Layout: crenellated curtain wall with four round corner towers, a
//! twin-towered gatehouse over the causeway, an inner keep with corner
//! piers, rooftop turrets, a great tower with banner, and courtyard
//! buildings. Roof cones are single rigid pieces that topple when the
//! masonry under them is destroyed. Windows are wall blocks with a warm
//! emissive material. Everything is `Respawnable`: Restart rebuilds the
//! castle.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::TAU;

use super::masonry::{self, MasonryAssets, MasonryBlock, spawn_block};
use super::terrain::{CASTLE_CENTER, TERRACE_HEIGHT};
use super::world::Respawnable;
use engine::prelude::*;

pub struct CastlePlugin;

impl Plugin for CastlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (setup_castle_assets, spawn_castle)
                .chain()
                .after(masonry::setup_masonry_assets),
        )
        .add_systems(
            Update,
            spawn_castle.run_if(on_message::<RestartRequested>),
        );
    }
}

/// Curtain wall footprint (local space, castle centered at origin, gate
/// facing +Z toward the valley).
const WALL_HALF_X: f32 = 30.0;
const WALL_HALF_Z: f32 = 24.0;
const WALL_HEIGHT: f32 = 12.0;
const WALL_THICKNESS: f32 = 2.0;
const GATE_HALF_WIDTH: f32 = 4.0;

/// Nominal masonry course dimensions.
const BLOCK_L: f32 = 1.4;
const BLOCK_H: f32 = 0.7;

/// Wall/pier tops land on whole courses; everything that sits on a wall
/// must use this height.
fn course_top(height: f32) -> f32 {
    (height / BLOCK_H).round() * BLOCK_H
}

#[derive(Resource)]
struct CastleAssets {
    cone: Handle<Mesh>,
    sphere: Handle<Mesh>,
    cube: Handle<Mesh>,
    flame: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    banner: Handle<StandardMaterial>,
}

fn setup_castle_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CastleAssets {
        cone: meshes.add(Cone {
            radius: 0.5,
            height: 1.0,
        }),
        sphere: meshes.add(Sphere::new(0.14)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        flame: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.6, 0.2),
            emissive: LinearRgba::rgb(2.2, 1.0, 0.25) * 9_000.0,
            ..default()
        }),
        slate: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.20, 0.27),
            perceptual_roughness: 0.55,
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
    });
}

fn spawn_castle(
    mut commands: Commands,
    masonry: Res<MasonryAssets>,
    castle: Res<CastleAssets>,
) {
    let c = &mut commands;
    let ma = &*masonry;
    let ca = &*castle;
    let o = Vec3::new(CASTLE_CENTER.x, TERRACE_HEIGHT, CASTLE_CENTER.y);

    // --- Curtain walls (gate opening in the front wall) ----------------------
    // Corner towers are r=5.5; walls stop 6.0 short of corners so block
    // colliders never overlap the tower rings. The gate-side front segments
    // also stop clear of the gatehouse towers.
    let tower_gap = 6.0;
    let gate_clear = GATE_HALF_WIDTH + 2.4 + 3.4;
    wall_run(c, ma, o + Vec3::new(-WALL_HALF_X + tower_gap, 0.0, -WALL_HALF_Z), Vec3::X, (WALL_HALF_X - tower_gap) * 2.0, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(-WALL_HALF_X, 0.0, -WALL_HALF_Z + tower_gap), Vec3::Z, (WALL_HALF_Z - tower_gap) * 2.0, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(WALL_HALF_X, 0.0, -WALL_HALF_Z + tower_gap), Vec3::Z, (WALL_HALF_Z - tower_gap) * 2.0, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(-WALL_HALF_X + tower_gap, 0.0, WALL_HALF_Z), Vec3::X, WALL_HALF_X - tower_gap - gate_clear, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(gate_clear, 0.0, WALL_HALF_Z), Vec3::X, WALL_HALF_X - tower_gap - gate_clear, WALL_HEIGHT, WALL_THICKNESS);

    // Wall-top merlons (outer edge).
    let wall_top = course_top(WALL_HEIGHT);
    merlons(c, ma, o + Vec3::new(0.0, wall_top, -WALL_HALF_Z - WALL_THICKNESS / 2.0 + 0.35), Vec3::X, (WALL_HALF_X - tower_gap) * 2.0 - 1.5);
    for sx in [-1.0_f32, 1.0] {
        merlons(c, ma, o + Vec3::new(sx * (WALL_HALF_X + WALL_THICKNESS / 2.0 - 0.35), wall_top, 0.0), Vec3::Z, (WALL_HALF_Z - tower_gap) * 2.0 - 1.5);
        let mid = (gate_clear + WALL_HALF_X - tower_gap) / 2.0;
        merlons(c, ma, o + Vec3::new(sx * mid, wall_top, WALL_HALF_Z + WALL_THICKNESS / 2.0 - 0.35), Vec3::X, WALL_HALF_X - tower_gap - gate_clear - 1.5);
    }

    // --- Corner towers + roofs ------------------------------------------------
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = o + Vec3::new(sx * WALL_HALF_X, 0.0, sz * WALL_HALF_Z);
        let top = ring_tower(c, ma, base, 5.5, 20.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (5.5 + 0.9), 9.0, false);
    }

    // --- Gatehouse --------------------------------------------------------------
    for sx in [-1.0_f32, 1.0] {
        let base = o + Vec3::new(sx * (GATE_HALF_WIDTH + 2.4), 0.0, WALL_HALF_Z + 1.0);
        let top = ring_tower(c, ma, base, 3.4, 16.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (3.4 + 0.9), 6.5, false);
    }
    // Lintel bridging the gate (3 courses; no arch physics — it will fall
    // if the gate towers are destroyed, which is the point).
    for row in 0..3 {
        let y = 9.0 + (row as f32 + 0.5) * BLOCK_H;
        let offset = if row % 2 == 0 { 0.0 } else { BLOCK_L / 2.0 };
        let mut x = -GATE_HALF_WIDTH - 2.0 + offset;
        while x + BLOCK_L <= GATE_HALF_WIDTH + 2.0 + 0.01 {
            spawn_block(c, ma, o + Vec3::new(x + BLOCK_L / 2.0, y, WALL_HALF_Z), Quat::IDENTITY, Vec3::new(BLOCK_L - 0.02, BLOCK_H - 0.02, WALL_THICKNESS));
            x += BLOCK_L;
        }
    }
    // Raised wooden portcullis in the arch.
    let portcullis = c
        .spawn((
            Mesh3d(ma.cube.clone()),
            MeshMaterial3d(ca.wood.clone()),
            Transform::from_translation(o + Vec3::new(0.0, 7.7, WALL_HALF_Z + 0.1))
                .with_scale(Vec3::new(GATE_HALF_WIDTH * 2.0 - 0.4, 2.2, 0.3)),
            RigidBody::Static,
            Collider::cuboid(1.0, 1.0, 1.0),
            ColliderDensity(600.0),
            Friction::new(0.5),
            Restitution::new(0.1),
            MasonryBlock,
            Respawnable,
        ))
        .id();
    let _ = portcullis;

    // --- Keep --------------------------------------------------------------------
    let keep = o + Vec3::new(0.0, 0.0, -6.0);
    let (kx, kz, kh) = (12.0, 10.0, 20.0);
    let t = 1.6;
    // Corner piers (solid columns) carry the rooftop turrets.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        pier(c, ma, keep + Vec3::new(sx * (kx - 1.3), 0.0, sz * (kz - 1.3)), 2.6, kh);
    }
    // Walls between piers; the front (+Z) wall gets window slots and a door.
    wall_run(c, ma, keep + Vec3::new(-kx + 2.6, 0.0, -kz + t / 2.0), Vec3::X, kx * 2.0 - 5.2, kh, t);
    wall_run(c, ma, keep + Vec3::new(-kx + t / 2.0, 0.0, -kz + 2.6), Vec3::Z, kz * 2.0 - 5.2, kh, t);
    wall_run(c, ma, keep + Vec3::new(kx - t / 2.0, 0.0, -kz + 2.6), Vec3::Z, kz * 2.0 - 5.2, kh, t);
    keep_front_wall(c, ma, ca, keep + Vec3::new(0.0, 0.0, kz - t / 2.0), kx * 2.0 - 5.2, kh, t);
    // Keep-top merlons.
    let keep_top = course_top(kh);
    merlons(c, ma, keep + Vec3::new(0.0, keep_top, kz - 0.5), Vec3::X, kx * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(0.0, keep_top, -kz + 0.5), Vec3::X, kx * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(kx - 0.5, keep_top, 0.0), Vec3::Z, kz * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(-kx + 0.5, keep_top, 0.0), Vec3::Z, kz * 2.0 - 3.0);
    // Rooftop turrets on the piers.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = keep + Vec3::new(sx * (kx - 1.3), keep_top, sz * (kz - 1.3));
        let top = ring_tower(c, ma, base, 1.9, 11.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (1.9 + 0.9), 4.4, false);
    }

    // --- Great tower ----------------------------------------------------------------
    let great = o + Vec3::new(0.0, 0.0, -17.0);
    let (gx, gz, gh) = (5.0, 5.0, 26.0);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        pier(c, ma, great + Vec3::new(sx * (gx - 1.1), 0.0, sz * (gz - 1.1)), 2.2, gh);
    }
    wall_run(c, ma, great + Vec3::new(-gx + 2.2, 0.0, -gz + 0.75), Vec3::X, gx * 2.0 - 4.4, gh, 1.5);
    wall_run(c, ma, great + Vec3::new(-gx + 2.2, 0.0, gz - 0.75), Vec3::X, gx * 2.0 - 4.4, gh, 1.5);
    wall_run(c, ma, great + Vec3::new(-gx + 0.75, 0.0, -gz + 2.2), Vec3::Z, gz * 2.0 - 4.4, gh, 1.5);
    wall_run(c, ma, great + Vec3::new(gx - 0.75, 0.0, -gz + 2.2), Vec3::Z, gz * 2.0 - 4.4, gh, 1.5);
    let great_top = course_top(gh);
    // Slate spire with the banner mounted on it (children fall with it);
    // sized to rest on the wall ring.
    roof_cone(c, ma, ca, great + Vec3::Y * great_top, 2.0 * (gx + 1.2), 9.5, true);

    // --- Torches: gate flanks, courtyard, keep door --------------------------
    for pos in [
        Vec3::new(-GATE_HALF_WIDTH - 0.8, 0.0, WALL_HALF_Z + 2.6),
        Vec3::new(GATE_HALF_WIDTH + 0.8, 0.0, WALL_HALF_Z + 2.6),
        Vec3::new(-3.0, 0.0, 6.5),
        Vec3::new(3.0, 0.0, 6.5),
        Vec3::new(-10.0, 0.0, 12.0),
        Vec3::new(10.0, 0.0, 12.0),
    ] {
        torch(c, ca, o + pos);
    }

    // --- Courtyard buildings ------------------------------------------------------
    courtyard_building(c, ma, ca, o + Vec3::new(-WALL_HALF_X + 6.5, 0.0, 4.0), 9.0, 7.0, 14.0, ca.slate.clone());
    courtyard_building(c, ma, ca, o + Vec3::new(WALL_HALF_X - 5.5, 0.0, 10.0), 6.0, 4.5, 10.0, ca.wood.clone());
}

/// A straight masonry wall: `start` is the base of one end, `dir` the unit
/// run direction. Courses alternate by half a block like real ashlar.
fn wall_run(
    commands: &mut Commands,
    assets: &MasonryAssets,
    start: Vec3,
    dir: Vec3,
    length: f32,
    height: f32,
    thickness: f32,
) {
    let rows = (height / BLOCK_H).round() as usize;
    let yaw = if dir.x.abs() > 0.5 {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    };
    for row in 0..rows {
        let y = (row as f32 + 0.5) * BLOCK_H;
        let offset = if row % 2 == 0 { 0.0 } else { BLOCK_L / 2.0 };
        let mut s = -offset;
        while s < length - 0.05 {
            let e = (s + BLOCK_L).min(length);
            let cs = s.max(0.0);
            let w = e - cs;
            if w > 0.15 {
                let center = start + dir * (cs + w / 2.0) + Vec3::Y * y;
                spawn_block(
                    commands,
                    assets,
                    center,
                    yaw,
                    Vec3::new(w - 0.02, BLOCK_H - 0.02, thickness),
                );
            }
            s += BLOCK_L;
        }
    }
}

/// The keep's valley-facing wall: like [`wall_run`] but with a door opening
/// at the center and emissive window slots.
fn keep_front_wall(
    commands: &mut Commands,
    assets: &MasonryAssets,
    castle: &CastleAssets,
    center_base: Vec3,
    length: f32,
    height: f32,
    thickness: f32,
) {
    let rows = (height / BLOCK_H).round() as usize;
    let start = center_base - Vec3::X * (length / 2.0);
    for row in 0..rows {
        let y = (row as f32 + 0.5) * BLOCK_H;
        let offset = if row % 2 == 0 { 0.0 } else { BLOCK_L / 2.0 };
        let mut s = -offset;
        while s < length - 0.05 {
            let e = (s + BLOCK_L).min(length);
            let cs = s.max(0.0);
            let w = e - cs;
            s += BLOCK_L;
            if w <= 0.15 {
                continue;
            }
            let local_x = cs + w / 2.0 - length / 2.0;
            // Door opening: 3.2 m wide, 5 courses tall, centered.
            if row < 5 && local_x.abs() < 1.6 {
                continue;
            }
            let pos = start + Vec3::X * (cs + w / 2.0) + Vec3::Y * y;
            let block = spawn_block(
                commands,
                assets,
                pos,
                Quat::IDENTITY,
                Vec3::new(w - 0.02, BLOCK_H - 0.02, thickness),
            );
            // Window slots: two bands, every ~4.2 m.
            let banded = (6..9).contains(&row) || (14..17).contains(&row);
            if banded && (local_x.abs() % 4.2) < 0.7 && local_x.abs() > 1.0 {
                commands
                    .entity(block)
                    .insert(MeshMaterial3d(castle.window.clone()));
            }
        }
    }
}

/// A solid square pier (column) of stacked blocks.
fn pier(commands: &mut Commands, assets: &MasonryAssets, base: Vec3, side: f32, height: f32) {
    let rows = (height / BLOCK_H).round() as usize;
    for row in 0..rows {
        let y = (row as f32 + 0.5) * BLOCK_H;
        // Alternate course orientation for an interlocked look.
        let yaw = if row % 2 == 0 {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        };
        spawn_block(
            commands,
            assets,
            base + Vec3::Y * y,
            yaw,
            Vec3::new(side - 0.02, BLOCK_H - 0.02, side - 0.02),
        );
    }
}

/// A round tower of tangent blocks; returns the actual top height (whole
/// rows). Roofed towers have no separate battlement ring — the cone covers
/// the wall head.
fn ring_tower(commands: &mut Commands, assets: &MasonryAssets, base: Vec3, radius: f32, height: f32) -> f32 {
    const ROW_H: f32 = 0.75;
    const ARC: f32 = 1.5;
    let radial = 1.4_f32.min(radius * 0.7);
    let rows = (height / ROW_H).round() as usize;
    let r_mid = radius - radial / 2.0;
    let n = ((TAU * r_mid) / ARC).round().max(6.0) as usize;
    let chord = TAU * r_mid / n as f32;
    for row in 0..rows {
        let y = (row as f32 + 0.5) * ROW_H;
        let offset = if row % 2 == 0 { 0.0 } else { 0.5 };
        for k in 0..n {
            let angle = (k as f32 + offset) / n as f32 * TAU;
            let pos = base + Vec3::new(angle.cos() * r_mid, y, angle.sin() * r_mid);
            let rot = Quat::from_rotation_y(-(angle + std::f32::consts::FRAC_PI_2));
            spawn_block(
                commands,
                assets,
                pos,
                rot,
                Vec3::new(chord - 0.04, ROW_H - 0.02, radial),
            );
        }
    }
    rows as f32 * ROW_H
}

/// A row of merlons along `dir`, centered at `center` (top-of-wall height).
fn merlons(commands: &mut Commands, assets: &MasonryAssets, center: Vec3, dir: Vec3, length: f32) {
    let yaw = if dir.x.abs() > 0.5 {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    };
    let count = (length / 2.3).floor().max(1.0) as usize;
    for k in 0..=count {
        let offset = (k as f32 / count as f32 - 0.5) * length;
        spawn_block(
            commands,
            assets,
            center + dir * offset + Vec3::Y * 0.65,
            yaw,
            Vec3::new(1.1, 1.3, 0.7),
        );
    }
}

/// A standing torch: wooden post, emissive flame, warm point light.
fn torch(commands: &mut Commands, castle: &CastleAssets, base: Vec3) {
    commands
        .spawn((
            Mesh3d(castle.cube.clone()),
            MeshMaterial3d(castle.wood.clone()),
            Transform::from_translation(base + Vec3::Y * 0.9).with_scale(Vec3::new(0.12, 1.8, 0.12)),
            Respawnable,
        ))
        .with_children(|t| {
            // Children of a scaled parent: counter the scale.
            t.spawn((
                Mesh3d(castle.sphere.clone()),
                MeshMaterial3d(castle.flame.clone()),
                Transform::from_xyz(0.0, 0.56, 0.0).with_scale(Vec3::new(1.0 / 0.12, 1.0 / 1.8, 1.0 / 0.12)),
            ));
            t.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.62, 0.28),
                    intensity: 600_000.0,
                    range: 22.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
        });
}

/// A slate roof cone: one rigid piece with a cone collider, masonry-managed
/// so it topples when its tower is destroyed. Optionally carries the banner.
fn roof_cone(
    commands: &mut Commands,
    masonry: &MasonryAssets,
    castle: &CastleAssets,
    base: Vec3,
    diameter: f32,
    height: f32,
    banner: bool,
) {
    let mut roof = commands.spawn((
        Mesh3d(castle.cone.clone()),
        MeshMaterial3d(castle.slate.clone()),
        Transform::from_translation(base + Vec3::Y * (height / 2.0))
            .with_scale(Vec3::new(diameter, height, diameter)),
        RigidBody::Static,
        Collider::cone(0.5, 1.0),
        ColliderDensity(900.0),
        Friction::new(0.6),
        Restitution::new(0.05),
        MasonryBlock,
        Respawnable,
    ));
    if banner {
        // Children ride along when the roof falls. Note: scales are in the
        // roof's (non-uniform) local space.
        roof.with_children(|r| {
            r.spawn((
                Mesh3d(masonry.cube.clone()),
                MeshMaterial3d(castle.wood.clone()),
                Transform::from_xyz(0.0, 0.75, 0.0).with_scale(Vec3::new(0.02, 0.6, 0.02)),
            ));
            r.spawn((
                Mesh3d(masonry.cube.clone()),
                MeshMaterial3d(castle.banner.clone()),
                Transform::from_xyz(0.08, 0.95, 0.0).with_scale(Vec3::new(0.18, 0.18, 0.012)),
            ));
        });
    }
}

/// A small courtyard building: four masonry walls and a one-piece roof slab.
fn courtyard_building(
    commands: &mut Commands,
    assets: &MasonryAssets,
    castle: &CastleAssets,
    center: Vec3,
    half_x: f32,
    height: f32,
    depth: f32,
    roof_material: Handle<StandardMaterial>,
) {
    let hz = depth / 2.0;
    let t = 1.0;
    wall_run(commands, assets, center + Vec3::new(-half_x / 2.0, 0.0, -hz + t / 2.0), Vec3::X, half_x, height, t);
    wall_run(commands, assets, center + Vec3::new(-half_x / 2.0, 0.0, hz - t / 2.0), Vec3::X, half_x, height, t);
    wall_run(commands, assets, center + Vec3::new(-half_x / 2.0 + t / 2.0, 0.0, -hz + t), Vec3::Z, depth - 2.0 * t, height, t);
    wall_run(commands, assets, center + Vec3::new(half_x / 2.0 - t / 2.0, 0.0, -hz + t), Vec3::Z, depth - 2.0 * t, height, t);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(roof_material),
        Transform::from_translation(center + Vec3::Y * (height + 0.4))
            .with_scale(Vec3::new(half_x + 1.0, 0.8, depth + 1.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
        ColliderDensity(700.0),
        Friction::new(0.6),
        Restitution::new(0.05),
        MasonryBlock,
        Respawnable,
    ));
    let _ = castle;
}
