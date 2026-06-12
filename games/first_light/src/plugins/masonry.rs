//! Physics-based masonry destruction.
//!
//! Castle structures are built from individual stone blocks that start as
//! `RigidBody::Static` — "mortared" in place. Two systems bring them to
//! life:
//!
//! 1. **Impact waking** ([`impact_wake`]): when a [`Projectile`] slams into
//!    static masonry, blocks within a radius scaled by the projectile's
//!    kinetic energy are converted to dynamic bodies.
//! 2. **Support collapse** ([`support_check`]): whenever a block wakes, its
//!    neighbors are queued; queued static blocks shape-cast a thin box just
//!    below their base, and any block with no static support left under it
//!    wakes too — so breaches propagate upward and walls cave in
//!    progressively, a few dozen checks per frame.
//!
//! Awakened blocks are ordinary dynamic rigid bodies: they tumble, slide,
//! pile into rubble, and go to sleep.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::VecDeque;

use super::world::Respawnable;

pub struct MasonryPlugin;

impl Plugin for MasonryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SupportQueue>()
            .add_systems(Startup, setup_masonry_assets)
            .add_systems(FixedUpdate, (impact_wake, support_check).chain());
    }
}

/// A mortared stone block (or roof piece) that can be knocked loose.
#[derive(Component)]
pub struct MasonryBlock;

/// Fast heavy objects that wake masonry on impact (catapult stones, thrown
/// cubes). Must also carry `CollisionEventsEnabled`.
#[derive(Component)]
pub struct Projectile;

/// Static masonry entities pending a support check.
#[derive(Resource, Default)]
pub struct SupportQueue(VecDeque<Entity>);

/// Shared masonry rendering assets: one unit cube, a subtle grain texture,
/// and a handful of stone tints.
#[derive(Resource)]
pub struct MasonryAssets {
    pub cube: Handle<Mesh>,
    pub tints: Vec<Handle<StandardMaterial>>,
}

/// Stone density (kg/m^3) used for all masonry.
pub const STONE_DENSITY: f32 = 2200.0;

pub fn setup_masonry_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let grain = images.add(grain_texture());
    let tints = [
        Color::srgb(0.55, 0.53, 0.50),
        Color::srgb(0.50, 0.48, 0.45),
        Color::srgb(0.59, 0.56, 0.51),
        Color::srgb(0.46, 0.45, 0.43),
        Color::srgb(0.53, 0.50, 0.44),
        Color::srgb(0.57, 0.55, 0.54),
    ]
    .map(|c| {
        materials.add(StandardMaterial {
            base_color: c,
            base_color_texture: Some(grain.clone()),
            perceptual_roughness: 0.94,
            ..default()
        })
    })
    .to_vec();

    commands.insert_resource(MasonryAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tints,
    });
}

/// Low-contrast value-noise grain so block faces aren't flat color.
fn grain_texture() -> Image {
    const SIZE: usize = 128;
    fn hash(a: u32, b: u32) -> f32 {
        let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
        h = (h ^ (h >> 13)).wrapping_mul(0xC2B2_AE35);
        ((h >> 16) & 0xFF) as f32 / 255.0
    }
    let mut data = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Two octaves of blocky noise, kept subtle.
            let v = 0.82
                + (hash((x / 16) as u32, (y / 16) as u32) - 0.5) * 0.10
                + (hash(x as u32, y as u32) - 0.5) * 0.10;
            let v = (v.clamp(0.0, 1.0) * 255.0) as u8;
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

/// Deterministic tint pick from a position.
pub fn tint_index(pos: Vec3, len: usize) -> usize {
    let h = (pos.x * 73.7 + pos.y * 179.3 + pos.z * 283.1).abs() as usize;
    h % len
}

/// Spawns one mortared stone block. `size` is full extents; rotation is
/// arbitrary (tower rings use tangents).
pub fn spawn_block(
    commands: &mut Commands,
    assets: &MasonryAssets,
    pos: Vec3,
    rotation: Quat,
    size: Vec3,
) -> Entity {
    commands
        .spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.tints[tint_index(pos, assets.tints.len())].clone()),
            Transform::from_translation(pos)
                .with_rotation(rotation)
                .with_scale(size),
            RigidBody::Static,
            Collider::cuboid(1.0, 1.0, 1.0),
            ColliderDensity(STONE_DENSITY),
            Friction::new(0.75),
            Restitution::new(0.05),
            MasonryBlock,
            Respawnable,
        ))
        .id()
}

/// Converts a static block to a dynamic body and queues its neighbors for
/// support checks.
fn wake_block(
    commands: &mut Commands,
    queue: &mut SupportQueue,
    spatial: &SpatialQuery,
    statics: &Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    entity: Entity,
) {
    let Ok((transform, body)) = statics.get(entity) else {
        return;
    };
    if *body != RigidBody::Static {
        return;
    }
    commands
        .entity(entity)
        .insert((RigidBody::Dynamic, TransformInterpolation));

    // Neighbors within ~1.5 block sizes get a support check.
    let reach = transform.scale.max_element() * 1.4 + 0.4;
    for neighbor in spatial.shape_intersections(
        &Collider::sphere(reach),
        transform.translation,
        Quat::IDENTITY,
        &SpatialQueryFilter::default(),
    ) {
        if neighbor != entity && statics.contains(neighbor) {
            queue.0.push_back(neighbor);
        }
    }
}

/// Wakes masonry around energetic projectile impacts.
fn impact_wake(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    mut queue: ResMut<SupportQueue>,
    spatial: SpatialQuery,
    projectiles: Query<(&LinearVelocity, &ComputedMass), With<Projectile>>,
    statics: Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    transforms: Query<&Transform>,
) {
    for event in events.read() {
        // Either side may be the projectile.
        let (projectile, _other) = if projectiles.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if projectiles.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };
        let Ok((velocity, mass)) = projectiles.get(projectile) else {
            continue;
        };
        let speed = velocity.length();
        if speed < 4.0 {
            continue;
        }
        let Ok(impact_at) = transforms.get(projectile).map(|t| t.translation) else {
            continue;
        };

        // Breach radius from kinetic energy: a thrown cube nudges one or
        // two stones loose, a catapult stone opens a hole meters wide.
        let energy = 0.5 * mass.value() * speed * speed;
        let radius = ((energy / 30_000.0).cbrt() * 2.0).clamp(0.9, 5.0);

        let mut woken = 0;
        for entity in spatial.shape_intersections(
            &Collider::sphere(radius),
            impact_at,
            Quat::IDENTITY,
            &SpatialQueryFilter::default(),
        ) {
            if matches!(statics.get(entity), Ok((_, RigidBody::Static))) {
                woken += 1;
            }
            wake_block(&mut commands, &mut queue, &spatial, &statics, entity);
        }
        if woken > 0 {
            info!("impact at {impact_at:.1}: {speed:.0} m/s, breach r={radius:.1}, woke {woken} blocks");
        }
    }
}

/// How many queued support checks run per physics tick.
const CHECKS_PER_TICK: usize = 48;

/// Wakes static masonry that has lost the support beneath it.
fn support_check(
    mut commands: Commands,
    mut queue: ResMut<SupportQueue>,
    spatial: SpatialQuery,
    statics: Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    bodies: Query<&RigidBody>,
) {
    for _ in 0..CHECKS_PER_TICK {
        let Some(entity) = queue.0.pop_front() else {
            return;
        };
        let Ok((transform, body)) = statics.get(entity) else {
            continue;
        };
        if *body != RigidBody::Static {
            continue;
        }

        // A thin box slightly below the block's base: is anything static
        // still holding it up?
        let probe = Vec3::new(
            (transform.scale.x * 0.7).max(0.2),
            0.12,
            (transform.scale.z * 0.7).max(0.2),
        );
        let below = transform.translation
            + transform.rotation * Vec3::new(0.0, -transform.scale.y / 2.0 - 0.08, 0.0);
        let supported = spatial
            .shape_intersections(
                &Collider::cuboid(probe.x, probe.y, probe.z),
                below,
                transform.rotation,
                &SpatialQueryFilter::from_excluded_entities([entity]),
            )
            .iter()
            .any(|&e| matches!(bodies.get(e), Ok(RigidBody::Static)));

        if !supported {
            wake_block(&mut commands, &mut queue, &spatial, &statics, entity);
        }
    }
}
