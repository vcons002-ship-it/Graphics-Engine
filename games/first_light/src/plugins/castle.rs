//! A stone castle on the headwall terrace — built entirely from individual
//! mortared masonry blocks (see `masonry.rs`), so every wall, tower, and
//! merlon can be brought down with physics.
//!
//! The architecture follows real concentric-castle conventions:
//! - curtain walls with a **battered plinth** (stepped, thicker base
//!   courses), a protruding **string course** at two-thirds height, and a
//!   corbelled **machicolation collar** under the wall head;
//! - four round corner towers plus **mural (interval) towers** mid-wall;
//! - a twin-towered **gatehouse** behind a forward **barbican** that
//!   funnels the causeway approach;
//! - an inner **keep** with solid corner piers and rooftop turrets, and a
//!   taller **great tower** with the banner;
//! - courtyard buildings: a **gabled great hall**, stables, and a well.
//!
//! Roof cones are single rigid pieces (marked [`ConeShape`] so they
//! fracture into cone-shaped debris). Everything is `Respawnable`:
//! Restart rebuilds the castle.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::TAU;

use super::masonry::{
    self, ConeShape, MasonryAssets, MasonryBlock, SLATE_TOUGHNESS, WOOD_TOUGHNESS, spawn_block,
};
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
        .add_systems(Update, spawn_castle.run_if(on_message::<RestartRequested>));
    }
}

/// Curtain wall footprint (local space, castle centered at origin, gate
/// facing +Z toward the valley).
pub const WALL_HALF_X: f32 = 42.0;
pub const WALL_HALF_Z: f32 = 34.0;
pub const WALL_HEIGHT: f32 = 14.0;
const WALL_THICKNESS: f32 = 2.2;
pub const GATE_HALF_WIDTH: f32 = 4.5;
/// Corner tower radius and clearance the walls keep from tower centers.
const CORNER_TOWER_R: f32 = 6.5;
const TOWER_GAP: f32 = 7.0;
/// Mural (interval) towers midway along the long walls.
const MURAL_TOWER_R: f32 = 4.2;
const MURAL_GAP: f32 = 4.8;

/// Nominal masonry course dimensions.
const BLOCK_L: f32 = 1.4;
const BLOCK_H: f32 = 0.7;

/// Wall/pier tops land on whole courses; everything that sits on a wall
/// must use this height.
fn course_top(height: f32) -> f32 {
    (height / BLOCK_H).round() * BLOCK_H
}

/// World-space point in the middle of the gate passage (soldiers test it
/// for blockage) at chest height.
pub fn gate_passage() -> Vec3 {
    Vec3::new(
        CASTLE_CENTER.x,
        TERRACE_HEIGHT + 1.6,
        CASTLE_CENTER.y + WALL_HALF_Z,
    )
}

/// Defender stations computed from the castle layout: wall-walk posts along
/// every curtain segment, archer perches on tower tops, and courtyard
/// reserves. `true` = archer.
pub fn defender_posts() -> Vec<(Vec3, bool)> {
    let o = Vec3::new(CASTLE_CENTER.x, TERRACE_HEIGHT, CASTLE_CENTER.y);
    let wall_top = course_top(WALL_HEIGHT);
    let ring_top = |h: f32| (h / 0.75_f32).round() * 0.75;
    let mut posts = Vec::new();

    // Wall-walks: spaced posts along each wall line (atop the wall).
    let mut wall_line = |from: Vec3, to: Vec3, spacing: f32| {
        let length = from.distance(to);
        let n = (length / spacing).floor().max(1.0) as usize;
        for k in 0..=n {
            let p = from.lerp(to, k as f32 / n as f32);
            posts.push((o + p + Vec3::Y * wall_top, false));
        }
    };
    wall_line(Vec3::new(-WALL_HALF_X + 8.0, 0.0, -WALL_HALF_Z), Vec3::new(WALL_HALF_X - 8.0, 0.0, -WALL_HALF_Z), 3.2);
    wall_line(Vec3::new(-WALL_HALF_X, 0.0, -WALL_HALF_Z + 8.0), Vec3::new(-WALL_HALF_X, 0.0, WALL_HALF_Z - 8.0), 3.2);
    wall_line(Vec3::new(WALL_HALF_X, 0.0, -WALL_HALF_Z + 8.0), Vec3::new(WALL_HALF_X, 0.0, WALL_HALF_Z - 8.0), 3.2);
    wall_line(Vec3::new(-WALL_HALF_X + 8.0, 0.0, WALL_HALF_Z), Vec3::new(-12.0, 0.0, WALL_HALF_Z), 2.6);
    wall_line(Vec3::new(12.0, 0.0, WALL_HALF_Z), Vec3::new(WALL_HALF_X - 8.0, 0.0, WALL_HALF_Z), 2.6);

    // Archers on the tower tops.
    for (pos, r, h) in [
        (Vec3::new(-WALL_HALF_X, 0.0, -WALL_HALF_Z), CORNER_TOWER_R, 24.0),
        (Vec3::new(WALL_HALF_X, 0.0, -WALL_HALF_Z), CORNER_TOWER_R, 24.0),
        (Vec3::new(-WALL_HALF_X, 0.0, WALL_HALF_Z), CORNER_TOWER_R, 24.0),
        (Vec3::new(WALL_HALF_X, 0.0, WALL_HALF_Z), CORNER_TOWER_R, 24.0),
        (Vec3::new(0.0, 0.0, -WALL_HALF_Z), MURAL_TOWER_R, 18.0),
        (Vec3::new(-WALL_HALF_X, 0.0, 0.0), MURAL_TOWER_R, 18.0),
        (Vec3::new(WALL_HALF_X, 0.0, 0.0), MURAL_TOWER_R, 18.0),
        (Vec3::new(-(GATE_HALF_WIDTH + 2.6), 0.0, WALL_HALF_Z + 1.2), 4.0, 20.0),
        (Vec3::new(GATE_HALF_WIDTH + 2.6, 0.0, WALL_HALF_Z + 1.2), 4.0, 20.0),
    ] {
        let top = ring_top(h);
        posts.push((o + pos + Vec3::new(r * 0.3, top, 0.0), true));
        posts.push((o + pos + Vec3::new(-r * 0.3, top, -r * 0.2), true));
        posts.push((o + pos + Vec3::new(0.0, top, r * 0.25), true));
        posts.push((o + pos + Vec3::new(-r * 0.2, top, r * 0.15), true));
    }

    // Courtyard reserves in loose ranks before the keep.
    for row in 0..6 {
        for col in 0..12 {
            posts.push((
                o + Vec3::new((col as f32 - 5.5) * 2.6, 0.0, 14.0 + row as f32 * 2.4),
                false,
            ));
        }
    }
    posts
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
            emissive: LinearRgba::rgb(2.2, 1.0, 0.25) * 700.0,
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
            emissive: LinearRgba::rgb(2.0, 1.2, 0.5) * 140.0,
            ..default()
        }),
        banner: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.08, 0.08),
            perceptual_roughness: 0.8,
            ..default()
        }),
    });
}

fn spawn_castle(mut commands: Commands, masonry: Res<MasonryAssets>, castle: Res<CastleAssets>) {
    let c = &mut commands;
    let ma = &*masonry;
    let ca = &*castle;
    let o = Vec3::new(CASTLE_CENTER.x, TERRACE_HEIGHT, CASTLE_CENTER.y);

    // --- Curtain walls, split around corner and mural towers ----------------
    // Back wall (mural tower at its center).
    let back_len = WALL_HALF_X - TOWER_GAP - MURAL_GAP;
    wall_run(c, ma, o + Vec3::new(-WALL_HALF_X + TOWER_GAP, 0.0, -WALL_HALF_Z), Vec3::X, back_len, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(MURAL_GAP, 0.0, -WALL_HALF_Z), Vec3::X, back_len, WALL_HEIGHT, WALL_THICKNESS);
    // Side walls (mural tower at each center).
    let side_len = WALL_HALF_Z - TOWER_GAP - MURAL_GAP;
    for sx in [-1.0_f32, 1.0] {
        wall_run(c, ma, o + Vec3::new(sx * WALL_HALF_X, 0.0, -WALL_HALF_Z + TOWER_GAP), Vec3::Z, side_len, WALL_HEIGHT, WALL_THICKNESS);
        wall_run(c, ma, o + Vec3::new(sx * WALL_HALF_X, 0.0, MURAL_GAP), Vec3::Z, side_len, WALL_HEIGHT, WALL_THICKNESS);
    }
    // Front wall segments flanking the gatehouse.
    let gate_clear = GATE_HALF_WIDTH + 2.6 + 4.0;
    let front_len = WALL_HALF_X - TOWER_GAP - gate_clear;
    wall_run(c, ma, o + Vec3::new(-WALL_HALF_X + TOWER_GAP, 0.0, WALL_HALF_Z), Vec3::X, front_len, WALL_HEIGHT, WALL_THICKNESS);
    wall_run(c, ma, o + Vec3::new(gate_clear, 0.0, WALL_HALF_Z), Vec3::X, front_len, WALL_HEIGHT, WALL_THICKNESS);

    // Wall-head merlons.
    let wall_top = course_top(WALL_HEIGHT);
    merlons(c, ma, o + Vec3::new(-WALL_HALF_X + TOWER_GAP + back_len / 2.0, wall_top, -WALL_HALF_Z - WALL_THICKNESS / 2.0 + 0.2), Vec3::X, back_len - 1.5);
    merlons(c, ma, o + Vec3::new(MURAL_GAP + back_len / 2.0, wall_top, -WALL_HALF_Z - WALL_THICKNESS / 2.0 + 0.2), Vec3::X, back_len - 1.5);
    for sx in [-1.0_f32, 1.0] {
        merlons(c, ma, o + Vec3::new(sx * (WALL_HALF_X + WALL_THICKNESS / 2.0 - 0.2), wall_top, -WALL_HALF_Z + TOWER_GAP + side_len / 2.0), Vec3::Z, side_len - 1.5);
        merlons(c, ma, o + Vec3::new(sx * (WALL_HALF_X + WALL_THICKNESS / 2.0 - 0.2), wall_top, MURAL_GAP + side_len / 2.0), Vec3::Z, side_len - 1.5);
        merlons(c, ma, o + Vec3::new(sx * (gate_clear + front_len / 2.0), wall_top, WALL_HALF_Z + WALL_THICKNESS / 2.0 - 0.2), Vec3::X, front_len - 1.5);
    }

    // --- Corner towers --------------------------------------------------------
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = o + Vec3::new(sx * WALL_HALF_X, 0.0, sz * WALL_HALF_Z);
        let top = ring_tower(c, ma, base, CORNER_TOWER_R, 24.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (CORNER_TOWER_R + 0.9), 10.0, false);
    }
    // --- Mural towers -----------------------------------------------------------
    for base in [
        o + Vec3::new(0.0, 0.0, -WALL_HALF_Z),
        o + Vec3::new(-WALL_HALF_X, 0.0, 0.0),
        o + Vec3::new(WALL_HALF_X, 0.0, 0.0),
    ] {
        let top = ring_tower(c, ma, base, MURAL_TOWER_R, 18.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (MURAL_TOWER_R + 0.9), 7.5, false);
    }

    // --- Gatehouse ----------------------------------------------------------------
    for sx in [-1.0_f32, 1.0] {
        let base = o + Vec3::new(sx * (GATE_HALF_WIDTH + 2.6), 0.0, WALL_HALF_Z + 1.2);
        let top = ring_tower(c, ma, base, 4.0, 20.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (4.0 + 0.9), 7.5, false);
    }
    // Gate lintel (3 courses spanning the opening).
    lintel(c, ma, o + Vec3::new(0.0, 11.2, WALL_HALF_Z), GATE_HALF_WIDTH - 0.8, WALL_THICKNESS, 3);
    // Raised wooden portcullis.
    wood_slab(c, ma, ca, o + Vec3::new(0.0, 9.4, WALL_HALF_Z + 0.1), Vec3::new(GATE_HALF_WIDTH * 2.0 - 0.6, 2.6, 0.3));

    // --- Barbican: forward gate guarding the causeway ---------------------------
    let barbican_z = WALL_HALF_Z + 14.0;
    for sx in [-1.0_f32, 1.0] {
        let base = o + Vec3::new(sx * (GATE_HALF_WIDTH + 1.4), 0.0, barbican_z);
        let top = ring_tower(c, ma, base, 2.8, 11.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (2.8 + 0.8), 5.0, false);
        // Flank walls connecting back toward the gatehouse.
        wall_run(c, ma, o + Vec3::new(sx * 6.6, 0.0, WALL_HALF_Z + 5.6), Vec3::Z, barbican_z - WALL_HALF_Z - 8.6, 7.0, 1.6);
    }
    lintel(c, ma, o + Vec3::new(0.0, 8.0, barbican_z), GATE_HALF_WIDTH + 0.4, 2.2, 2);

    // --- Keep -------------------------------------------------------------------------
    let keep = o + Vec3::new(0.0, 0.0, -8.0);
    let (kx, kz, kh) = (14.0, 12.0, 24.0);
    let t = 1.8;
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        pier(c, ma, keep + Vec3::new(sx * (kx - 1.5), 0.0, sz * (kz - 1.5)), 3.0, kh);
    }
    wall_run(c, ma, keep + Vec3::new(-kx + 3.0, 0.0, -kz + t / 2.0), Vec3::X, kx * 2.0 - 6.0, kh, t);
    wall_run(c, ma, keep + Vec3::new(-kx + t / 2.0, 0.0, -kz + 3.0), Vec3::Z, kz * 2.0 - 6.0, kh, t);
    wall_run(c, ma, keep + Vec3::new(kx - t / 2.0, 0.0, -kz + 3.0), Vec3::Z, kz * 2.0 - 6.0, kh, t);
    keep_front_wall(c, ma, ca, keep + Vec3::new(0.0, 0.0, kz - t / 2.0), kx * 2.0 - 6.0, kh, t);
    let keep_top = course_top(kh);
    merlons(c, ma, keep + Vec3::new(0.0, keep_top, kz - 0.5), Vec3::X, kx * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(0.0, keep_top, -kz + 0.5), Vec3::X, kx * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(kx - 0.5, keep_top, 0.0), Vec3::Z, kz * 2.0 - 3.0);
    merlons(c, ma, keep + Vec3::new(-kx + 0.5, keep_top, 0.0), Vec3::Z, kz * 2.0 - 3.0);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = keep + Vec3::new(sx * (kx - 1.5), keep_top, sz * (kz - 1.5));
        let top = ring_tower(c, ma, base, 2.2, 12.0);
        roof_cone(c, ma, ca, base + Vec3::Y * top, 2.0 * (2.2 + 0.8), 5.0, false);
    }

    // --- Great tower -----------------------------------------------------------------
    let great = o + Vec3::new(0.0, 0.0, -26.5);
    let (gx, gz, gh) = (6.0, 6.0, 34.0);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        pier(c, ma, great + Vec3::new(sx * (gx - 1.3), 0.0, sz * (gz - 1.3)), 2.6, gh);
    }
    wall_run(c, ma, great + Vec3::new(-gx + 2.6, 0.0, -gz + 0.8), Vec3::X, gx * 2.0 - 5.2, gh, 1.6);
    wall_run(c, ma, great + Vec3::new(-gx + 2.6, 0.0, gz - 0.8), Vec3::X, gx * 2.0 - 5.2, gh, 1.6);
    wall_run(c, ma, great + Vec3::new(-gx + 0.8, 0.0, -gz + 2.6), Vec3::Z, gz * 2.0 - 5.2, gh, 1.6);
    wall_run(c, ma, great + Vec3::new(gx - 0.8, 0.0, -gz + 2.6), Vec3::Z, gz * 2.0 - 5.2, gh, 1.6);
    let great_top = course_top(gh);
    roof_cone(c, ma, ca, great + Vec3::Y * great_top, 2.0 * (gx + 1.4), 11.0, true);

    // --- Courtyard ----------------------------------------------------------------------
    // Great hall with a gabled roof, along the west wall.
    let hall = o + Vec3::new(-WALL_HALF_X + 9.5, 0.0, 8.0);
    let (hx, hh, hd) = (5.5, 8.0, 19.0);
    wall_run(c, ma, hall + Vec3::new(-hx, 0.0, -hd / 2.0 + 0.6), Vec3::X, hx * 2.0, hh, 1.2);
    wall_run(c, ma, hall + Vec3::new(-hx, 0.0, hd / 2.0 - 0.6), Vec3::X, hx * 2.0, hh, 1.2);
    wall_run(c, ma, hall + Vec3::new(-hx + 0.6, 0.0, -hd / 2.0 + 1.4), Vec3::Z, hd - 2.8, hh, 1.2);
    wall_run(c, ma, hall + Vec3::new(hx - 0.6, 0.0, -hd / 2.0 + 1.4), Vec3::Z, hd - 2.8, hh, 1.2);
    gable_roof(c, ma, ca, hall + Vec3::Y * course_top(hh), hx, hd + 1.2);

    // Stables along the east wall (flat wooden roof).
    let stables = o + Vec3::new(WALL_HALF_X - 7.0, 0.0, 14.0);
    wall_run(c, ma, stables + Vec3::new(-3.5, 0.0, -5.5), Vec3::X, 7.0, 4.5, 1.0);
    wall_run(c, ma, stables + Vec3::new(-3.5, 0.0, 5.5), Vec3::X, 7.0, 4.5, 1.0);
    wall_run(c, ma, stables + Vec3::new(-3.0, 0.0, -4.8), Vec3::Z, 9.6, 4.5, 1.0);
    wall_run(c, ma, stables + Vec3::new(3.0, 0.0, -4.8), Vec3::Z, 9.6, 4.5, 1.0);
    wood_slab(c, ma, ca, stables + Vec3::Y * (course_top(4.5) + 0.4), Vec3::new(8.6, 0.8, 12.6));

    // Courtyard well.
    ring_tower(c, ma, o + Vec3::new(12.0, 0.0, 16.0), 1.4, 1.4);

    // --- Torches: barbican, gate, courtyard, keep door ---------------------------
    for pos in [
        Vec3::new(-GATE_HALF_WIDTH - 1.0, 0.0, barbican_z + 2.0),
        Vec3::new(GATE_HALF_WIDTH + 1.0, 0.0, barbican_z + 2.0),
        Vec3::new(-GATE_HALF_WIDTH - 1.0, 0.0, WALL_HALF_Z + 3.0),
        Vec3::new(GATE_HALF_WIDTH + 1.0, 0.0, WALL_HALF_Z + 3.0),
        Vec3::new(-4.0, 0.0, 8.0),
        Vec3::new(4.0, 0.0, 8.0),
    ] {
        torch(c, ca, o + pos);
    }
}

/// A straight masonry wall with real fortress detailing: a battered (stepped,
/// thicker) plinth on the bottom three courses, a protruding string course at
/// two-thirds height, and a corbelled machicolation collar at the head.
/// Courses alternate by half a block like real ashlar.
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
    let string_row = rows * 2 / 3;
    let yaw = if dir.x.abs() > 0.5 {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    };
    for row in 0..rows {
        let y = (row as f32 + 0.5) * BLOCK_H;
        let t = match row {
            0 => thickness + 0.66,
            1 => thickness + 0.44,
            2 => thickness + 0.22,
            r if r == string_row => thickness + 0.3,
            r if r + 1 == rows => thickness + 0.55,
            _ => thickness,
        };
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
                    Vec3::new(w - 0.02, BLOCK_H - 0.02, t),
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
            // Door opening: 3.6 m wide, 6 courses tall, centered.
            if row < 6 && local_x.abs() < 1.8 {
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
            let banded = (9..12).contains(&row) || (19..22).contains(&row);
            if banded && (local_x.abs() % 4.4) < 0.7 && local_x.abs() > 1.2 {
                commands
                    .entity(block)
                    .try_insert(MeshMaterial3d(castle.window.clone()));
            }
        }
    }
}

/// A solid square pier (column) of stacked blocks.
fn pier(commands: &mut Commands, assets: &MasonryAssets, base: Vec3, side: f32, height: f32) {
    let rows = (height / BLOCK_H).round() as usize;
    for row in 0..rows {
        let y = (row as f32 + 0.5) * BLOCK_H;
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

/// A round tower of tangent blocks with a battered base and a corbelled top
/// ring; returns the actual top height (whole rows).
fn ring_tower(
    commands: &mut Commands,
    assets: &MasonryAssets,
    base: Vec3,
    radius: f32,
    height: f32,
) -> f32 {
    const ROW_H: f32 = 0.75;
    const ARC: f32 = 1.5;
    let radial = 1.4_f32.min(radius * 0.7);
    let rows = (height / ROW_H).round() as usize;
    for row in 0..rows {
        // Battered base and corbelled head.
        let flare = match row {
            0 => 0.45,
            1 => 0.3,
            2 => 0.15,
            r if r + 1 == rows && rows > 4 => 0.35,
            _ => 0.0,
        };
        let r_mid = radius - radial / 2.0 + flare;
        let n = ((TAU * r_mid) / ARC).round().max(6.0) as usize;
        let chord = TAU * r_mid / n as f32;
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
                Vec3::new(chord - 0.04, ROW_H - 0.02, radial + flare),
            );
        }
    }
    rows as f32 * ROW_H
}

/// A row of merlons (crenellation teeth) along `dir`, centered at `center`.
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

/// Courses of stone bridging an opening (gate/barbican arch substitute).
fn lintel(
    commands: &mut Commands,
    assets: &MasonryAssets,
    center: Vec3,
    half_span: f32,
    thickness: f32,
    courses: usize,
) {
    for row in 0..courses {
        let y = center.y + (row as f32 + 0.5) * BLOCK_H;
        let offset = if row % 2 == 0 { 0.0 } else { BLOCK_L / 2.0 };
        let mut x = -half_span - 1.8 + offset;
        while x + BLOCK_L <= half_span + 1.8 + 0.01 {
            spawn_block(
                commands,
                assets,
                Vec3::new(center.x + x + BLOCK_L / 2.0, y, center.z),
                Quat::IDENTITY,
                Vec3::new(BLOCK_L - 0.02, BLOCK_H - 0.02, thickness),
            );
            x += BLOCK_L;
        }
    }
}

/// A destructible wooden slab (portcullis, flat roofs).
fn wood_slab(
    commands: &mut Commands,
    masonry: &MasonryAssets,
    castle: &CastleAssets,
    center: Vec3,
    size: Vec3,
) {
    commands.spawn((
        Mesh3d(masonry.cube.clone()),
        MeshMaterial3d(castle.wood.clone()),
        Transform::from_translation(center).with_scale(size),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
        ColliderDensity(600.0),
        Friction::new(0.5),
        Restitution::new(0.1),
        MasonryBlock::from_volume(size.x * size.y * size.z, WOOD_TOUGHNESS),
        Respawnable,
    ));
}

/// A gabled slate roof: two tilted slabs meeting at a ridge.
fn gable_roof(
    commands: &mut Commands,
    masonry: &MasonryAssets,
    castle: &CastleAssets,
    eaves_center: Vec3,
    half_width: f32,
    length: f32,
) {
    let pitch = 0.55_f32;
    let slope = half_width / pitch.cos();
    for side in [-1.0_f32, 1.0] {
        let size = Vec3::new(slope + 0.6, 0.45, length);
        let center = eaves_center
            + Vec3::new(
                side * half_width / 2.0,
                (half_width / 2.0) * pitch.tan() + 0.2,
                0.0,
            );
        commands.spawn((
            Mesh3d(masonry.cube.clone()),
            MeshMaterial3d(castle.slate.clone()),
            Transform::from_translation(center)
                .with_rotation(Quat::from_rotation_z(-side * pitch))
                .with_scale(size),
            RigidBody::Static,
            Collider::cuboid(1.0, 1.0, 1.0),
            ColliderDensity(800.0),
            Friction::new(0.6),
            Restitution::new(0.05),
            MasonryBlock::from_volume(size.x * size.y * size.z, SLATE_TOUGHNESS),
            Respawnable,
        ));
    }
}

/// A slate roof cone: one rigid piece with a cone collider, masonry-managed
/// (and [`ConeShape`]-marked so it fractures into cone-shaped debris).
/// Optionally carries the banner.
#[allow(clippy::too_many_arguments)]
fn roof_cone(
    commands: &mut Commands,
    masonry: &MasonryAssets,
    castle: &CastleAssets,
    base: Vec3,
    diameter: f32,
    height: f32,
    banner: bool,
) {
    let volume = std::f32::consts::PI * (diameter / 2.0).powi(2) * height / 3.0;
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
        MasonryBlock::from_volume(volume, SLATE_TOUGHNESS),
        ConeShape,
        Respawnable,
    ));
    if banner {
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
            t.spawn((
                Mesh3d(castle.sphere.clone()),
                MeshMaterial3d(castle.flame.clone()),
                Transform::from_xyz(0.0, 0.56, 0.0)
                    .with_scale(Vec3::new(1.0 / 0.12, 1.0 / 1.8, 1.0 / 0.12)),
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
