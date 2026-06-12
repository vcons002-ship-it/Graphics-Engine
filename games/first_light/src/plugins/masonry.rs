//! Physics-based masonry destruction with per-stone damage and fracture.
//!
//! Castle structures are built from individual stone blocks that start as
//! `RigidBody::Static` — "mortared" in place. Destruction works on three
//! levels:
//!
//! 1. **Damage** ([`MasonryBlock::integrity`], joules): every energetic hit
//!    — projectile or a dynamic body crushing into a block — subtracts
//!    kinetic energy from the stones involved. At 50% integrity a block
//!    visibly cracks (material swap); at 0 it **fractures** into dynamic
//!    fragments that inherit its velocity.
//! 2. **Mortar failure**: blocks taking heavy but sub-fracture damage break
//!    loose (static → dynamic) and tumble.
//! 3. **Support collapse**: whenever a block wakes or fractures, neighbors
//!    are queued; queued static blocks shape-cast a thin box below their
//!    base and let go if nothing static holds them — breaches propagate
//!    and walls cave in progressively.
//!
//! Falling masonry carries collision events, so a collapsing tower crushes
//! and fractures whatever it lands on — chain reactions are real physics,
//! not scripts.
//!
//! Performance bounds (deliberate, all tunable):
//! - fragments are terminal — they never re-fracture;
//! - a global budget ([`FRAGMENT_BUDGET`]) despawns the oldest *sleeping*
//!   fragments when rubble piles up;
//! - collision events exist only on dynamic bodies — the 10k-block static
//!   castle generates none;
//! - support checks are amortized ([`CHECKS_PER_TICK`]) and damage events
//!   capped per tick ([`DAMAGE_EVENTS_PER_TICK`]).

use avian3d::prelude::*;
use bevy::ecs::query::Has;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::VecDeque;

use super::audio::{SoundEvent, SoundKind};
use super::scoring::DestructionTally;
use super::soldiers::BlastEvent;
use super::world::Respawnable;

pub struct MasonryPlugin;

impl Plugin for MasonryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SupportQueue>()
            .init_resource::<FragmentCounter>()
            .init_resource::<ImpactShake>()
            .add_systems(Startup, setup_masonry_assets)
            .add_systems(
                FixedUpdate,
                (
                    projectile_impacts,
                    crush_damage,
                    cache_pre_tick_velocities,
                    support_check,
                    fragment_budget,
                )
                    .chain(),
            )
            .add_systems(Update, dust_motes);
    }
}

/// A mortared stone block (or roof piece) with remaining integrity in
/// joules of absorbable impact energy.
#[derive(Component)]
pub struct MasonryBlock {
    pub integrity: f32,
    pub max_integrity: f32,
}

impl MasonryBlock {
    /// Integrity from volume (m^3) and material toughness (J/m^3).
    pub fn from_volume(volume: f32, toughness: f32) -> Self {
        let integrity = (volume * toughness).max(4_000.0);
        Self {
            integrity,
            max_integrity: integrity,
        }
    }
}

/// Impact-energy absorption per cubic meter before stone shatters.
pub const STONE_TOUGHNESS: f32 = 55_000.0;
pub const WOOD_TOUGHNESS: f32 = 25_000.0;
pub const SLATE_TOUGHNESS: f32 = 35_000.0;

/// Terminal rubble from a fractured block (sequence number for the budget).
#[derive(Component)]
pub struct Fragment(pub u64);

/// Marks cone-shaped masonry (tower roofs): fragments outside the cone
/// volume are skipped so debris forms a cone-shaped cloud, not a box.
#[derive(Component)]
pub struct ConeShape;

/// Screen-shake trauma fed by heavy impacts; consumed by the camera.
#[derive(Resource, Default)]
pub struct ImpactShake(pub f32);

/// Short-lived dust puff (no physics body — integrated by `dust_motes`).
#[derive(Component)]
struct DustMote {
    velocity: Vec3,
    life: f32,
}

/// Fast heavy objects that blast masonry on impact (catapult stones, thrown
/// cubes). Must also carry `CollisionEventsEnabled`.
#[derive(Component)]
pub struct Projectile;

/// Velocity captured before the physics step. `CollisionStart` events are
/// emitted after the solver has already absorbed the impact, so reading
/// `LinearVelocity` in a handler gives the rebound speed — roughly an
/// order of magnitude too little energy. Every body that can deal impact
/// damage carries this cache.
#[derive(Component, Default)]
pub struct PreTickVelocity(pub Vec3);

/// Runs after the damage handlers each tick: stores the current (not yet
/// re-solved) velocity for next tick's events.
fn cache_pre_tick_velocities(mut query: Query<(&LinearVelocity, &mut PreTickVelocity)>) {
    for (velocity, mut cache) in &mut query {
        cache.0 = Vec3::new(velocity.x, velocity.y, velocity.z);
    }
}

/// Static masonry entities pending a support check.
#[derive(Resource, Default)]
pub struct SupportQueue(VecDeque<Entity>);

#[derive(Resource, Default)]
pub struct FragmentCounter {
    next_seq: u64,
}

/// Shared masonry rendering assets: a plain unit cube, chiseled unit-cube
/// variants (jittered corners, flat shading) so stone reads rough-hewn,
/// grain textures, and parallel intact/cracked stone tints.
#[derive(Resource)]
pub struct MasonryAssets {
    pub cube: Handle<Mesh>,
    pub chiseled: Vec<Handle<Mesh>>,
    pub tints: Vec<Handle<StandardMaterial>>,
    pub cracked: Vec<Handle<StandardMaterial>>,
}

/// A unit cube whose corners are jittered (consistently across the faces
/// sharing each corner) and flat-shaded: a rough-hewn stone block.
fn chiseled_cube(seed: u32) -> Mesh {
    fn corner_hash(seed: u32, p: Vec3, axis: u32) -> f32 {
        let q = |v: f32| (v * 4.0).round() as i32 as u32;
        let mut h = seed
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(q(p.x).wrapping_mul(0x85EB_CA6B))
            .wrapping_add(q(p.y).wrapping_mul(0xC2B2_AE35))
            .wrapping_add(q(p.z).wrapping_mul(0x27D4_EB2F))
            .wrapping_add(axis.wrapping_mul(0x1656_67B1));
        h = (h ^ (h >> 13)).wrapping_mul(0x6849_5FB7);
        ((h >> 16) & 0xFF) as f32 / 255.0 - 0.5
    }

    let mut mesh: Mesh = Cuboid::new(1.0, 1.0, 1.0).into();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for p in positions.iter_mut() {
            let v = Vec3::from_array(*p);
            for axis in 0..3 {
                p[axis as usize] += corner_hash(seed, v, axis) * 0.12;
            }
        }
    }
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

/// Stone density (kg/m^3) used for all masonry.
pub const STONE_DENSITY: f32 = 2200.0;

pub fn setup_masonry_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let grain = images.add(grain_texture(false));
    let cracked_grain = images.add(grain_texture(true));
    let colors = [
        Color::srgb(0.55, 0.53, 0.50),
        Color::srgb(0.50, 0.48, 0.45),
        Color::srgb(0.59, 0.56, 0.51),
        Color::srgb(0.46, 0.45, 0.43),
        Color::srgb(0.53, 0.50, 0.44),
        Color::srgb(0.57, 0.55, 0.54),
    ];
    let tints = colors
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: c,
                base_color_texture: Some(grain.clone()),
                perceptual_roughness: 0.94,
                ..default()
            })
        })
        .to_vec();
    let cracked = colors
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: c.darker(0.06),
                base_color_texture: Some(cracked_grain.clone()),
                perceptual_roughness: 0.97,
                ..default()
            })
        })
        .to_vec();

    commands.insert_resource(MasonryAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        chiseled: (0..6).map(|seed| meshes.add(chiseled_cube(seed))).collect(),
        tints,
        cracked,
    });
}

/// Low-contrast value-noise grain; the cracked variant adds dark fissure
/// lines wandering across the face.
fn grain_texture(cracked: bool) -> Image {
    const SIZE: usize = 128;
    fn hash(a: u32, b: u32) -> f32 {
        let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
        h = (h ^ (h >> 13)).wrapping_mul(0xC2B2_AE35);
        ((h >> 16) & 0xFF) as f32 / 255.0
    }
    let mut values = vec![0.0f32; SIZE * SIZE];
    for y in 0..SIZE {
        for x in 0..SIZE {
            values[y * SIZE + x] = 0.82
                + (hash((x / 16) as u32, (y / 16) as u32) - 0.5) * 0.10
                + (hash(x as u32, y as u32) - 0.5) * 0.10;
        }
    }
    if cracked {
        // A few random-walk fissures.
        for c in 0..5u32 {
            let mut x = (hash(c, 1) * SIZE as f32) as i32;
            let mut y = (hash(c, 2) * SIZE as f32) as i32;
            let mut dir = hash(c, 3) * std::f32::consts::TAU;
            for step in 0..90 {
                dir += (hash(c * 91 + step, 4) - 0.5) * 1.2;
                x = (x + dir.cos().round() as i32).rem_euclid(SIZE as i32);
                y = (y + dir.sin().round() as i32).rem_euclid(SIZE as i32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
                    let xi = (x + dx).rem_euclid(SIZE as i32) as usize;
                    let yi = (y + dy).rem_euclid(SIZE as i32) as usize;
                    values[yi * SIZE + xi] *= 0.45;
                }
            }
        }
    }
    let mut data = Vec::with_capacity(SIZE * SIZE * 4);
    for v in values {
        let v = (v.clamp(0.0, 1.0) * 255.0) as u8;
        data.extend_from_slice(&[v, v, v, 255]);
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

/// Spawns one mortared stone block. `size` is full extents.
pub fn spawn_block(
    commands: &mut Commands,
    assets: &MasonryAssets,
    pos: Vec3,
    rotation: Quat,
    size: Vec3,
) -> Entity {
    commands
        .spawn((
            Mesh3d(assets.chiseled[tint_index(pos + Vec3::splat(7.7), assets.chiseled.len())].clone()),
            MeshMaterial3d(assets.tints[tint_index(pos, assets.tints.len())].clone()),
            Transform::from_translation(pos)
                .with_rotation(rotation)
                .with_scale(size),
            RigidBody::Static,
            Collider::cuboid(1.0, 1.0, 1.0),
            ColliderDensity(STONE_DENSITY),
            Friction::new(0.75),
            Restitution::new(0.05),
            MasonryBlock::from_volume(size.x * size.y * size.z, STONE_TOUGHNESS),
            Respawnable,
        ))
        .id()
}

/// Queues every masonry neighbor of `position` for a support check.
fn enqueue_neighbors(
    queue: &mut SupportQueue,
    spatial: &SpatialQuery,
    blocks: &Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    position: Vec3,
    reach: f32,
) {
    for neighbor in spatial.shape_intersections(
        &Collider::sphere(reach),
        position,
        Quat::IDENTITY,
        &SpatialQueryFilter::default(),
    ) {
        if blocks.contains(neighbor) {
            queue.0.push_back(neighbor);
        }
    }
}

/// Converts a static block to a dynamic body ("mortar failure") and queues
/// its neighbors. Dynamic masonry carries collision events so it can crush
/// what it lands on.
fn wake_block(
    commands: &mut Commands,
    queue: &mut SupportQueue,
    tally: &mut DestructionTally,
    spatial: &SpatialQuery,
    blocks: &Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    entity: Entity,
) {
    let Ok((transform, body)) = blocks.get(entity) else {
        return;
    };
    if *body != RigidBody::Static {
        return;
    }
    commands.entity(entity).try_insert((
        RigidBody::Dynamic,
        TransformInterpolation,
        CollisionEventsEnabled,
        PreTickVelocity::default(),
        // Aggressive sleep ("Jenga" culling): settled rubble leaves the
        // solver quickly instead of grinding and re-triggering crush.
        SleepThreshold {
            linear: 0.6,
            angular: 0.7,
        },
    ));
    tally.add(1);
    let reach = transform.scale.max_element() * 1.4 + 0.4;
    enqueue_neighbors(queue, spatial, blocks, transform.translation, reach);
}

/// Replaces a block with dynamic fragments (pieces no larger than ~1.1 m)
/// that inherit its motion. Cone-shaped pieces (tower roofs) only spawn
/// fragments inside the cone volume.
#[allow(clippy::too_many_arguments)]
fn fracture(
    commands: &mut Commands,
    assets: &MasonryAssets,
    counter: &mut FragmentCounter,
    queue: &mut SupportQueue,
    tally: &mut DestructionTally,
    sounds: &mut MessageWriter<SoundEvent>,
    spatial: &SpatialQuery,
    blocks: &Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    cones: &Query<(), With<ConeShape>>,
    entity: Entity,
    transform: &Transform,
    velocity: Vec3,
) {
    let size = transform.scale;
    let cracked = &assets.cracked[tint_index(transform.translation, assets.cracked.len())];
    let is_cone = cones.contains(entity);

    let splits = |s: f32| ((s / 1.1).ceil() as usize).clamp(1, 4);
    let (nx, ny, nz) = (splits(size.x), splits(size.y), splits(size.z));
    let piece = size / Vec3::new(nx as f32, ny as f32, nz as f32);

    for ix in 0..nx {
        for iy in 0..ny {
            for iz in 0..nz {
                let normalized = Vec3::new(
                    (ix as f32 + 0.5) / nx as f32 - 0.5,
                    (iy as f32 + 0.5) / ny as f32 - 0.5,
                    (iz as f32 + 0.5) / nz as f32 - 0.5,
                );
                // Cone volume check: radius shrinks linearly with height.
                if is_cone {
                    let allowed = 0.5 - normalized.y - 0.05;
                    if Vec2::new(normalized.x, normalized.z).length() > allowed.max(0.0) {
                        continue;
                    }
                }
                counter.next_seq += 1;
                let jitter = 0.72 + ((counter.next_seq * 37 % 23) as f32 / 23.0) * 0.22;
                let local = normalized * size;
                let offset = transform.rotation * local;
                let spray = offset.normalize_or_zero() * 1.5;
                commands.spawn((
                    Fragment(counter.next_seq),
                    Mesh3d(assets.cube.clone()),
                    MeshMaterial3d(cracked.clone()),
                    Transform::from_translation(transform.translation + offset)
                        .with_rotation(transform.rotation)
                        .with_scale(piece * jitter),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    ColliderDensity(STONE_DENSITY),
                    Friction::new(0.8),
                    Restitution::new(0.05),
                    LinearVelocity((velocity + spray).into()),
                    SleepThreshold {
                        linear: 0.6,
                        angular: 0.7,
                    },
                    TransformInterpolation,
                    Respawnable,
                ));
            }
        }
    }

    tally.add(3);
    sounds.write(SoundEvent {
        kind: SoundKind::RockCrack,
        position: transform.translation,
        intensity: (size.x * size.y * size.z / 3.0).clamp(0.3, 1.0),
    });
    let reach = size.max_element() * 1.4 + 0.4;
    let position = transform.translation;
    commands.entity(entity).try_despawn();
    enqueue_neighbors(queue, spatial, blocks, position, reach);
}

/// Applies `energy` joules to a block: cracks it visually past 50%, wakes
/// it on heavy damage, fractures it at zero. Returns true if it fractured.
#[allow(clippy::too_many_arguments)]
fn apply_damage(
    commands: &mut Commands,
    assets: &MasonryAssets,
    counter: &mut FragmentCounter,
    queue: &mut SupportQueue,
    tally: &mut DestructionTally,
    sounds: &mut MessageWriter<SoundEvent>,
    spatial: &SpatialQuery,
    blocks: &Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    cones: &Query<(), With<ConeShape>>,
    damageable: &mut Query<(&mut MasonryBlock, Option<&LinearVelocity>)>,
    entity: Entity,
    energy: f32,
) -> bool {
    let Ok((mut block, velocity)) = damageable.get_mut(entity) else {
        return false;
    };
    // Sentinel: this block already fractured this tick (its despawn is
    // queued); damaging it again would queue commands on a dead entity.
    if block.integrity == f32::NEG_INFINITY {
        return false;
    }
    let was = block.integrity;
    block.integrity -= energy;

    if block.integrity <= 0.0 {
        block.integrity = f32::NEG_INFINITY;
        if let Ok((transform, _)) = blocks.get(entity) {
            let v = velocity.map(|v| Vec3::new(v.x, v.y, v.z)).unwrap_or(Vec3::ZERO);
            let transform = *transform;
            fracture(
                commands, assets, counter, queue, tally, sounds, spatial, blocks, cones, entity,
                &transform, v,
            );
        }
        return true;
    }
    // Visible crack at half integrity.
    if was > block.max_integrity * 0.5 && block.integrity <= block.max_integrity * 0.5 {
        if let Ok((transform, _)) = blocks.get(entity) {
            commands.entity(entity).try_insert(MeshMaterial3d(
                assets.cracked[tint_index(transform.translation, assets.cracked.len())].clone(),
            ));
        }
    }
    // Heavy single hits blow the mortar even when the stone survives.
    if energy > 0.28 * block.max_integrity {
        wake_block(commands, queue, tally, spatial, blocks, entity);
    }
    false
}

/// Projectile strikes: kinetic energy is shared over blocks near the impact
/// with linear falloff — the closest stones shatter, the next ring breaks
/// loose, the outer ring cracks.
#[allow(clippy::too_many_arguments)]
fn projectile_impacts(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    mut queue: ResMut<SupportQueue>,
    mut counter: ResMut<FragmentCounter>,
    mut tally: ResMut<DestructionTally>,
    mut shake: ResMut<ImpactShake>,
    mut sounds: MessageWriter<SoundEvent>,
    mut blasts: MessageWriter<BlastEvent>,
    assets: Res<MasonryAssets>,
    spatial: SpatialQuery,
    cones: Query<(), With<ConeShape>>,
    projectiles: Query<(&PreTickVelocity, &ComputedMass), With<Projectile>>,
    blocks: Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    mut damageable: Query<(&mut MasonryBlock, Option<&LinearVelocity>)>,
    transforms: Query<&Transform>,
    mut seen: Local<bevy::platform::collections::HashSet<Entity>>,
) {
    // One projectile touching several blocks fires one event per pair;
    // process each projectile once per tick or the energy multiplies.
    seen.clear();
    for event in events.read() {
        let (projectile, struck) = if projectiles.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if projectiles.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };
        if !seen.insert(projectile) {
            continue;
        }
        let Ok((velocity, mass)) = projectiles.get(projectile) else {
            continue;
        };
        let speed = velocity.0.length();
        if speed < 6.0 {
            continue;
        }
        let Ok(impact_at) = transforms.get(projectile).map(|t| t.translation) else {
            continue;
        };

        let energy = 0.5 * mass.value() * speed * speed;
        let radius = ((energy / 22_000.0).cbrt() * 2.2).clamp(0.9, 7.0);

        // The stone actually struck one block: it bears the brunt (55%) —
        // a tonne of granite at 20 m/s genuinely shatters its contact
        // stone. 30% radiates into the surrounding masonry with falloff;
        // the rest is heat and noise.
        let mut direct_shattered = 0;
        if blocks.contains(struck)
            && apply_damage(
                &mut commands, &assets, &mut counter, &mut queue, &mut tally, &mut sounds,
                &spatial, &blocks, &cones, &mut damageable, struck, energy * 0.55,
            )
        {
            direct_shattered += 1;
        }

        let hits: Vec<(Entity, f32)> = spatial
            .shape_intersections(
                &Collider::sphere(radius),
                impact_at,
                Quat::IDENTITY,
                &SpatialQueryFilter::default(),
            )
            .into_iter()
            .filter_map(|e| {
                let (transform, _) = blocks.get(e).ok()?;
                let d = transform.translation.distance(impact_at);
                Some((e, (1.0 - d / radius).max(0.05)))
            })
            .collect();
        let total_weight: f32 = hits.iter().map(|(_, w)| w).sum();
        if total_weight <= 0.0 {
            continue;
        }

        let mut shattered = direct_shattered;
        for (target, weight) in &hits {
            if *target == struck {
                continue;
            }
            let share = energy * 0.30 * weight / total_weight;
            if apply_damage(
                &mut commands, &assets, &mut counter, &mut queue, &mut tally, &mut sounds,
                &spatial, &blocks, &cones, &mut damageable, *target, share,
            ) {
                shattered += 1;
            }
        }
        // Stone-on-stone weight: screen shake, dust, and a deep thud.
        shake.0 = (shake.0 + (energy / 3.0e6).clamp(0.15, 1.0)).min(1.2);
        spawn_dust(&mut commands, &assets, impact_at, energy);
        sounds.write(SoundEvent {
            kind: SoundKind::StoneImpact,
            position: impact_at,
            intensity: (energy / 4.0e6).clamp(0.25, 1.0),
        });
        blasts.write(BlastEvent {
            position: impact_at,
            radius,
        });
        info!(
            "impact at {impact_at:.1}: {speed:.0} m/s, {energy:.0} J over {} blocks (r={radius:.1}), {shattered} shattered",
            hits.len()
        );
    }
}

/// Damage events handled per tick (the rest of the messages are still
/// drained, just cheaply).
const DAMAGE_EVENTS_PER_TICK: usize = 192;

/// Dynamic masonry (and fragments) crushing into blocks: both sides absorb
/// a share of the relative kinetic energy. This is what lets a collapsing
/// tower smash the wall it falls on.
#[allow(clippy::too_many_arguments)]
fn crush_damage(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    mut queue: ResMut<SupportQueue>,
    mut counter: ResMut<FragmentCounter>,
    mut tally: ResMut<DestructionTally>,
    mut sounds: MessageWriter<SoundEvent>,
    assets: Res<MasonryAssets>,
    spatial: SpatialQuery,
    cones: Query<(), With<ConeShape>>,
    projectiles: Query<(), With<Projectile>>,
    movers: Query<(&PreTickVelocity, &ComputedMass)>,
    blocks: Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    mut damageable: Query<(&mut MasonryBlock, Option<&LinearVelocity>)>,
) {
    let mut handled = 0;
    for event in events.read() {
        if handled >= DAMAGE_EVENTS_PER_TICK {
            break;
        }
        // Projectile hits are handled (with falloff) by projectile_impacts.
        if projectiles.contains(event.collider1) || projectiles.contains(event.collider2) {
            continue;
        }
        let (a, b) = (event.collider1, event.collider2);
        let va = movers.get(a).map(|(v, m)| (v.0, m.value())).ok();
        let vb = movers.get(b).map(|(v, m)| (v.0, m.value())).ok();

        let relative = match (&va, &vb) {
            (Some((va, _)), Some((vb, _))) => (*va - *vb).length(),
            (Some((va, _)), None) => va.length(),
            (None, Some((vb, _))) => vb.length(),
            (None, None) => continue,
        };
        // Damped against settling-pile feedback: only genuinely fast hits
        // crush (a block falling ~1.5 m barely qualifies).
        if relative < 5.0 {
            continue;
        }
        // Effective mass: the lighter participant limits transferred energy.
        let mass = match (&va, &vb) {
            (Some((_, ma)), Some((_, mb))) => ma.min(*mb),
            (Some((_, ma)), None) => *ma,
            (None, Some((_, mb))) => *mb,
            _ => continue,
        };
        let energy = 0.5 * mass * relative * relative * 0.5;
        if energy < 8_000.0 {
            continue;
        }
        handled += 1;
        if let Ok((t, _)) = blocks.get(a).or(blocks.get(b)) {
            sounds.write(SoundEvent {
                kind: SoundKind::StoneImpact,
                position: t.translation,
                intensity: (energy / 1.5e6).clamp(0.1, 0.7),
            });
        }

        for target in [a, b] {
            if damageable.contains(target) {
                apply_damage(
                    &mut commands, &assets, &mut counter, &mut queue, &mut tally, &mut sounds,
                    &spatial, &blocks, &cones, &mut damageable, target, energy * 0.5,
                );
            }
        }
    }
}

/// A burst of unphysical dust motes at an impact point (count scales with
/// energy; they fly out, fall, shrink, and despawn).
fn spawn_dust(commands: &mut Commands, assets: &MasonryAssets, at: Vec3, energy: f32) {
    let count = ((energy / 80_000.0) as usize).clamp(8, 36);
    for k in 0..count {
        let h = |n: u64| {
            let mut v = (k as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ n.wrapping_mul(0xBF58476D1CE4E5B9);
            v ^= v >> 31;
            (v % 1000) as f32 / 1000.0
        };
        let dir = Vec3::new(h(1) - 0.5, h(2) * 0.7 + 0.15, h(3) - 0.5).normalize_or_zero();
        commands.spawn((
            DustMote {
                velocity: dir * (4.0 + h(4) * 7.0),
                life: 1.2 + h(5) * 0.6,
            },
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.tints[k % assets.tints.len()].clone()),
            Transform::from_translation(at + dir * 0.8)
                .with_scale(Vec3::splat(0.25 + h(6) * 0.4)),
        ));
    }
}

fn dust_motes(
    mut commands: Commands,
    time: Res<Time>,
    mut motes: Query<(Entity, &mut DustMote, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut mote, mut transform) in &mut motes {
        mote.life -= dt;
        if mote.life <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        mote.velocity.y -= 6.0 * dt;
        mote.velocity *= 1.0 - 1.6 * dt;
        let velocity = mote.velocity;
        transform.translation += velocity * dt;
        transform.scale *= 1.0 - 0.8 * dt;
    }
}

/// How many queued support checks run per physics tick.
const CHECKS_PER_TICK: usize = 48;

/// Wakes static masonry that has genuinely lost its support. Three checks,
/// cheap to expensive (the "forgiving archway" model):
///
/// 1. Direct bearing: anything static right under the base? Stays.
/// 2. Arch rule: two or more adjacent static blocks? It is bridged —
///    jagged archways survive over breaches instead of unzipping the
///    whole castle.
/// 3. Deep anchor: bridged blocks still need static mass somewhere within
///    8 m below, or the whole floating chunk lets go.
fn support_check(
    mut commands: Commands,
    mut queue: ResMut<SupportQueue>,
    mut tally: ResMut<DestructionTally>,
    spatial: SpatialQuery,
    blocks: Query<(&Transform, &RigidBody), With<MasonryBlock>>,
    bodies: Query<&RigidBody>,
) {
    for _ in 0..CHECKS_PER_TICK {
        let Some(entity) = queue.0.pop_front() else {
            return;
        };
        let Ok((transform, body)) = blocks.get(entity) else {
            continue;
        };
        if *body != RigidBody::Static {
            continue;
        }
        let is_static = |e: Entity| matches!(bodies.get(e), Ok(RigidBody::Static));

        // 1. Direct bearing under the base.
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
            .copied()
            .any(is_static);
        if supported {
            continue;
        }

        // 2. Arch rule: bridged by at least two static masonry neighbors.
        let tight = transform.scale.max_element() * 0.8 + 0.15;
        let neighbors = spatial
            .shape_intersections(
                &Collider::sphere(tight),
                transform.translation,
                Quat::IDENTITY,
                &SpatialQueryFilter::from_excluded_entities([entity]),
            )
            .iter()
            .filter(|&&e| blocks.contains(e) && is_static(e))
            .count();

        if neighbors >= 2 {
            // 3. Deep anchor: static mass anywhere within 8 m below?
            let deep = Vec3::new(
                (transform.scale.x * 0.5).max(0.2),
                8.0,
                (transform.scale.z * 0.5).max(0.2),
            );
            let anchored = spatial
                .shape_intersections(
                    &Collider::cuboid(deep.x, deep.y, deep.z),
                    below + Vec3::new(0.0, -4.0, 0.0),
                    Quat::IDENTITY,
                    &SpatialQueryFilter::from_excluded_entities([entity]),
                )
                .iter()
                .copied()
                .any(is_static);
            if anchored {
                continue;
            }
        }

        wake_block(&mut commands, &mut queue, &mut tally, &spatial, &blocks, entity);
    }
}

/// Hard cap on rubble. Above the budget, the oldest sleeping fragments are
/// recycled; in a runaway pileup (>150% of budget) even awake ones go.
const FRAGMENT_BUDGET: usize = 1_500;

fn fragment_budget(
    mut commands: Commands,
    fragments: Query<(Entity, &Fragment, Has<Sleeping>)>,
) {
    let count = fragments.iter().count();
    if count <= FRAGMENT_BUDGET {
        return;
    }
    let hard_over = count > FRAGMENT_BUDGET * 3 / 2;
    let mut candidates: Vec<(u64, Entity)> = fragments
        .iter()
        .filter(|(_, _, sleeping)| *sleeping || hard_over)
        .map(|(e, f, _)| (f.0, e))
        .collect();
    candidates.sort_unstable_by_key(|(seq, _)| *seq);
    for (_, entity) in candidates.into_iter().take(count - FRAGMENT_BUDGET) {
        commands.entity(entity).try_despawn();
    }
}
