//! The siege battle: an attacking army assaults the castle while defenders
//! man the walls and towers.
//!
//! Soldiers are lightweight kinematic agents (no character controllers):
//! they steer over the pure [`terrain_height`] function, so hundreds cost
//! almost nothing. Defenders hold wall-walk and tower posts computed from
//! the castle layout; archers on both sides loose visual-ballistic arrows;
//! melee breaks out wherever the lines meet.
//!
//! The battle stalls by design: the gate holds, and defenders out-shoot
//! the attackers from cover. The player's trebuchet is the decisive force
//! — blast radii ([`BlastEvent`]) and fast debris kill soldiers (ragdoll),
//! and once the gate passage is breached the attackers pour into the
//! courtyard. When the last defender falls, the castle falls.
use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use super::audio::{SoundEvent, SoundKind};
use super::castle::{self, gate_passage};
use super::masonry::{MasonryBlock, PreTickVelocity};
use super::terrain::{CASTLE_CENTER, terrain_height};
use super::world::Respawnable;
use engine::prelude::*;

pub struct SoldiersPlugin;

impl Plugin for SoldiersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlastEvent>()
            .init_resource::<Battle>()
            .init_resource::<BattleGrid>()
            .add_systems(Startup, (setup_soldier_assets, spawn_armies).chain())
            .add_systems(
                Update,
                (
                    rebuild_grid,
                    soldier_ai,
                    arrows,
                    blast_kills,
                    debris_kills,
                    battle_state,
                )
                    .chain()
                    .run_if(in_state(MenuState::Closed)),
            )
            .add_systems(
                Update,
                (reset_battle, spawn_armies)
                    .chain()
                    .run_if(on_message::<RestartRequested>),
            );
        if std::env::var("FL_BATTLE_LOG").is_ok() {
            app.add_systems(Update, battle_log);
        }
    }
}

/// Written by the masonry system on big projectile impacts: soldiers caught
/// in the radius are blown off their feet.
#[derive(Message)]
pub struct BlastEvent {
    pub position: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Attacker,
    Defender,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Attackers advancing on the castle.
    Marching { waypoint: usize },
    /// Defenders at their station.
    Holding,
    /// Trading blows with a nearby enemy.
    Fighting,
    /// Down; despawns after the timer.
    Dead { timer: f32, ragdoll: bool },
}

#[derive(Component)]
struct Soldier {
    side: Side,
    archer: bool,
    state: State,
    /// Station for defenders; loose-formation offset for attackers.
    post: Vec3,
    cooldown: f32,
    seed: f32,
}

/// A visual-ballistic arrow (no physics body).
#[derive(Component)]
struct Arrow {
    velocity: Vec3,
    from: Side,
    life: f32,
}

#[derive(Resource, Default)]
struct Battle {
    horn_blown: bool,
    victory_announced: bool,
}

/// Uniform spatial grid over the live soldiers, rebuilt every frame.
/// Everything that needs neighbors (melee, separation, arrow hits) asks
/// the grid instead of scanning all ~1,500 soldiers.
#[derive(Resource, Default)]
struct BattleGrid {
    cells: bevy::platform::collections::HashMap<IVec2, Vec<(Entity, Vec3, Side)>>,
}

const GRID_CELL: f32 = 6.0;

impl BattleGrid {
    fn key(p: Vec3) -> IVec2 {
        IVec2::new((p.x / GRID_CELL).floor() as i32, (p.z / GRID_CELL).floor() as i32)
    }

    fn rebuild(&mut self, soldiers: impl Iterator<Item = (Entity, Vec3, Side)>) {
        self.cells.clear();
        for (entity, pos, side) in soldiers {
            self.cells.entry(Self::key(pos)).or_default().push((entity, pos, side));
        }
    }

    /// Visits every live soldier within `radius` of `from`.
    fn near(&self, from: Vec3, radius: f32, mut f: impl FnMut(Entity, Vec3, Side)) {
        let r = (radius / GRID_CELL).ceil() as i32;
        let center = Self::key(from);
        let r_sq = radius * radius;
        for dx in -r..=r {
            for dz in -r..=r {
                if let Some(cell) = self.cells.get(&(center + IVec2::new(dx, dz))) {
                    for &(entity, pos, side) in cell {
                        if from.distance_squared(pos) <= r_sq {
                            f(entity, pos, side);
                        }
                    }
                }
            }
        }
    }

    fn nearest_enemy(&self, from: Vec3, side: Side, radius: f32) -> Option<(Entity, Vec3, f32)> {
        let mut best: Option<(Entity, Vec3, f32)> = None;
        self.near(from, radius, |entity, pos, other_side| {
            if other_side != side {
                let d = from.distance_squared(pos);
                if best.is_none_or(|(_, _, b)| d < b) {
                    best = Some((entity, pos, d));
                }
            }
        });
        best
    }
}

#[derive(Component)]
struct VictoryBanner;

#[derive(Resource)]
struct SoldierAssets {
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    helmet: Handle<Mesh>,
    stick: Handle<Mesh>,
    shield: Handle<Mesh>,
    arrow: Handle<Mesh>,
    attacker_tunics: Vec<Handle<StandardMaterial>>,
    defender_tunics: Vec<Handle<StandardMaterial>>,
    skin: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
}

fn setup_soldier_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let tunic = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    commands.insert_resource(SoldierAssets {
        torso: meshes.add(Capsule3d::new(0.26, 0.7)),
        head: meshes.add(Sphere::new(0.16)),
        helmet: meshes.add(Cone {
            radius: 0.18,
            height: 0.22,
        }),
        stick: meshes.add(Cuboid::new(0.05, 1.7, 0.05)),
        shield: meshes.add(Cuboid::new(0.45, 0.62, 0.07)),
        arrow: meshes.add(Cuboid::new(0.03, 0.03, 0.6)),
        attacker_tunics: [
            Color::srgb(0.55, 0.10, 0.10),
            Color::srgb(0.48, 0.13, 0.08),
            Color::srgb(0.62, 0.16, 0.12),
        ]
        .map(|c| tunic(&mut materials, c))
        .to_vec(),
        defender_tunics: [
            Color::srgb(0.12, 0.20, 0.52),
            Color::srgb(0.10, 0.26, 0.46),
            Color::srgb(0.16, 0.18, 0.58),
        ]
        .map(|c| tunic(&mut materials, c))
        .to_vec(),
        skin: tunic(&mut materials, Color::srgb(0.75, 0.58, 0.45)),
        iron: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.46, 0.5),
            metallic: 0.8,
            perceptual_roughness: 0.5,
            ..default()
        }),
        wood: tunic(&mut materials, Color::srgb(0.35, 0.24, 0.13)),
    });
}

fn hash01(seed: u64) -> f32 {
    let mut h = seed.wrapping_mul(0x9E3779B97F4A7C15);
    h ^= h >> 31;
    (h % 10_000) as f32 / 10_000.0
}

/// Attack route: staging meadow, causeway foot, mid-ramp, barbican, gate,
/// courtyard, keep door.
fn waypoints() -> [Vec3; 6] {
    let o = Vec3::new(CASTLE_CENTER.x, 0.0, CASTLE_CENTER.y);
    [
        Vec3::new(0.0, 0.0, -55.0),
        Vec3::new(0.0, 0.0, -90.0),
        o + Vec3::new(0.0, 0.0, castle::WALL_HALF_Z + 16.0),
        o + Vec3::new(0.0, 0.0, castle::WALL_HALF_Z + 4.0),
        o + Vec3::new(0.0, 0.0, castle::WALL_HALF_Z - 8.0),
        o + Vec3::new(0.0, 0.0, 10.0),
    ]
}

fn spawn_soldier(
    commands: &mut Commands,
    assets: &SoldierAssets,
    position: Vec3,
    side: Side,
    archer: bool,
    post: Vec3,
    seed: u64,
) {
    let tunics = match side {
        Side::Attacker => &assets.attacker_tunics,
        Side::Defender => &assets.defender_tunics,
    };
    let tunic = tunics[(seed % tunics.len() as u64) as usize].clone();
    let state = match side {
        Side::Attacker => State::Marching { waypoint: 0 },
        Side::Defender => State::Holding,
    };
    commands
        .spawn((
            Soldier {
                side,
                archer,
                state,
                post,
                cooldown: hash01(seed.wrapping_add(7)) * 2.0,
                seed: hash01(seed),
            },
            Transform::from_translation(position),
            Visibility::default(),
            RigidBody::Kinematic,
            Collider::capsule(0.26, 0.9),
            Respawnable,
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(assets.torso.clone()),
                MeshMaterial3d(tunic),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            s.spawn((
                Mesh3d(assets.head.clone()),
                MeshMaterial3d(assets.skin.clone()),
                Transform::from_xyz(0.0, 0.72, 0.0),
            ));
            s.spawn((
                Mesh3d(assets.helmet.clone()),
                MeshMaterial3d(assets.iron.clone()),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            if archer {
                // Bow: a slim stave held across the body.
                s.spawn((
                    Mesh3d(assets.stick.clone()),
                    MeshMaterial3d(assets.wood.clone()),
                    Transform::from_xyz(0.32, 0.2, 0.0)
                        .with_rotation(Quat::from_rotation_z(0.35)),
                ));
            } else {
                // Spear and shield.
                s.spawn((
                    Mesh3d(assets.stick.clone()),
                    MeshMaterial3d(assets.wood.clone()),
                    Transform::from_xyz(0.34, 0.35, 0.0),
                ));
                s.spawn((
                    Mesh3d(assets.shield.clone()),
                    MeshMaterial3d(assets.iron.clone()),
                    Transform::from_xyz(-0.34, 0.2, 0.12),
                ));
            }
        });
}

fn spawn_armies(mut commands: Commands, assets: Res<SoldierAssets>) {
    // Defenders at their computed stations (every third is an archer even
    // off the towers).
    for (k, (post, tower_archer)) in castle::defender_posts().into_iter().enumerate() {
        let archer = tower_archer || k % 3 == 0;
        spawn_soldier(&mut commands, &assets, post, Side::Defender, archer, post, k as u64);
    }

    // A host: twelve companies of 81 staged in waves across the meadow;
    // 20% carry bows. Later waves start farther back and march in behind
    // the van.
    let mut n = 0u64;
    for company in 0..12u32 {
        let wave = company / 4;
        let cx = ((company % 4) as f32 - 1.5) * 22.0;
        let cz = -16.0 - wave as f32 * 22.0;
        for rank in 0..9 {
            for file in 0..9 {
                n += 1;
                let x = cx + (file as f32 - 4.0) * 1.9 + (hash01(n) - 0.5) * 1.2;
                let z = cz + rank as f32 * 1.9 + (hash01(n + 999) - 0.5) * 1.2;
                let position = Vec3::new(x, terrain_height(x, z) + 0.85, z);
                let lateral = Vec3::new(
                    (hash01(n + 5) - 0.5) * 12.0,
                    0.0,
                    (hash01(n + 11) - 0.5) * 6.0,
                );
                spawn_soldier(
                    &mut commands,
                    &assets,
                    position,
                    Side::Attacker,
                    n % 5 == 0,
                    lateral,
                    n + 10_000,
                );
            }
        }
    }
}

fn reset_battle(mut battle: ResMut<Battle>) {
    *battle = Battle::default();
}

fn rebuild_grid(mut grid: ResMut<BattleGrid>, soldiers: Query<(Entity, &Soldier, &Transform)>) {
    grid.rebuild(
        soldiers
            .iter()
            .filter(|(_, s, _)| !matches!(s.state, State::Dead { .. }))
            .map(|(e, s, t)| (e, t.translation, s.side)),
    );
}

/// Squared-distance helper over a soldier snapshot.
fn nearest_enemy(
    snapshot: &[(Entity, Vec3, Side)],
    from: Vec3,
    side: Side,
    max: f32,
) -> Option<(Entity, Vec3, f32)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;
    let max_sq = max * max;
    for &(entity, pos, other_side) in snapshot {
        if other_side == side {
            continue;
        }
        let d = from.distance_squared(pos);
        if d < max_sq && best.is_none_or(|(_, _, b)| d < b) {
            best = Some((entity, pos, d));
        }
    }
    best
}

const WALK_SPEED: f32 = 3.1;

#[allow(clippy::too_many_arguments)]
fn soldier_ai(
    mut commands: Commands,
    time: Res<Time>,
    spatial: SpatialQuery,
    grid: Res<BattleGrid>,
    assets: Res<SoldierAssets>,
    mut sounds: MessageWriter<SoundEvent>,
    mut soldiers: Query<(Entity, &mut Soldier, &mut Transform)>,
    bodies: Query<&RigidBody>,
    masonry: Query<(), With<MasonryBlock>>,
    mut kill_list: Local<Vec<Entity>>,
    mut frame: Local<u32>,
) {
    let dt = time.delta_secs();
    let route = waypoints();
    *frame = frame.wrapping_add(1);

    // Strided snapshot for long-range targeting: a quarter of the live
    // soldiers is plenty to pick an archery target from.
    let snapshot: Vec<(Entity, Vec3, Side)> = soldiers
        .iter()
        .filter(|(_, s, _)| !matches!(s.state, State::Dead { .. }))
        .map(|(e, s, t)| (e, t.translation, s.side))
        .enumerate()
        .filter(|(k, _)| k % 4 == 0)
        .map(|(_, v)| v)
        .collect();

    // Is the gate passage still blocked by masonry?
    let gate_blocked = spatial
        .shape_intersections(
            &Collider::sphere(2.6),
            gate_passage(),
            Quat::IDENTITY,
            &SpatialQueryFilter::default(),
        )
        .iter()
        .any(|&e| masonry.contains(e) && matches!(bodies.get(e), Ok(RigidBody::Static)));

    for (entity, mut soldier, mut transform) in &mut soldiers {
        soldier.cooldown -= dt;
        let side = soldier.side;
        let archer = soldier.archer;
        match soldier.state {
            State::Dead { timer, ragdoll } => {
                let t = timer + dt;
                if !ragdoll {
                    // Topple over where they stood.
                    let lean = (t * 3.0).min(std::f32::consts::FRAC_PI_2);
                    transform.rotation = Quat::from_rotation_z(lean);
                }
                if t > 7.0 {
                    commands.entity(entity).despawn();
                } else {
                    soldier.state = State::Dead { timer: t, ragdoll };
                }
                continue;
            }
            State::Marching { waypoint } => {
                let target = if waypoint < route.len() {
                    route[waypoint] + soldier.post * (1.0 - waypoint as f32 * 0.15).max(0.3)
                } else {
                    route[route.len() - 1]
                };
                let mut to = target - transform.translation;
                to.y = 0.0;
                let distance = to.length();

                // Engage anything close.
                if grid.nearest_enemy(transform.translation, side, 2.4).is_some() {
                    soldier.state = State::Fighting;
                } else if distance < 3.0 {
                    let next = waypoint + 1;
                    // Hold at the gate while it is blocked.
                    if next == 5 && gate_blocked {
                        // Mill about; archers shoot from here.
                    } else if next < route.len() {
                        soldier.state = State::Marching { waypoint: next };
                    } else {
                        soldier.state = State::Fighting;
                    }
                } else {
                    // Separation: ease away from packed neighbors so the
                    // column doesn't collapse into a blob.
                    let mut push = Vec3::ZERO;
                    let here = transform.translation;
                    grid.near(here, 1.3, |other, pos, _| {
                        if other != entity {
                            let away = here - pos;
                            push += Vec3::new(away.x, 0.0, away.z).normalize_or_zero();
                        }
                    });
                    let step = (to.normalize_or_zero() + push.clamp_length_max(1.2) * 0.6)
                        .normalize_or_zero()
                        * WALK_SPEED
                        * dt;
                    transform.translation += step;
                    transform.translation.y = terrain_height(
                        transform.translation.x,
                        transform.translation.z,
                    ) + 0.85
                        + (time.elapsed_secs() * 9.0 + soldier.seed * 20.0).sin().abs() * 0.06;
                    if step.length_squared() > 0.0 {
                        let yaw = (-step.x).atan2(-step.z);
                        transform.rotation = Quat::from_rotation_y(yaw);
                    }
                }

                // Attacker archers volley at the walls while advancing.
                if archer
                    && soldier.cooldown <= 0.0
                    && let Some((_, target_pos, d)) =
                        nearest_enemy(&snapshot, transform.translation, side, 90.0)
                    && d > 36.0
                {
                    soldier.cooldown = 3.0 + soldier.seed * 2.5;
                    loose_arrow(&mut commands, &assets, transform.translation, target_pos, side);
                }
            }
            State::Holding => {
                // Snap to post (defenders stand on masonry, not terrain).
                transform.translation = soldier.post;
                // Stagger the long-range scan: each defender re-targets a
                // few times a second, offset by its seed.
                if (frame.wrapping_add((soldier.seed * 64.0) as u32)) % 6 != 0 {
                    continue;
                }
                if let Some((_, target_pos, d)) =
                    nearest_enemy(&snapshot, transform.translation, side, 95.0)
                {
                    let to = target_pos - transform.translation;
                    transform.rotation = Quat::from_rotation_y((-to.x).atan2(-to.z));
                    if d < 2.4 * 2.4 {
                        soldier.state = State::Fighting;
                    } else if archer && soldier.cooldown <= 0.0 {
                        soldier.cooldown = 2.8 + soldier.seed * 2.2;
                        loose_arrow(&mut commands, &assets, transform.translation, target_pos, side);
                    }
                }
            }
            State::Fighting => {
                match grid.nearest_enemy(transform.translation, side, 2.6) {
                    Some((enemy, enemy_pos, _)) => {
                        let to = enemy_pos - transform.translation;
                        transform.rotation = Quat::from_rotation_y((-to.x).atan2(-to.z));
                        if soldier.cooldown <= 0.0 {
                            soldier.cooldown = 1.1 + soldier.seed * 0.8;
                            sounds.write(SoundEvent {
                                kind: SoundKind::Clank,
                                position: transform.translation,
                                intensity: 0.5 + soldier.seed * 0.4,
                            });
                            if soldier.seed + soldier.cooldown.fract() > 0.85 {
                                kill_list.push(enemy);
                            }
                        }
                    }
                    None => {
                        soldier.state = match side {
                            Side::Attacker => State::Marching { waypoint: 4 },
                            Side::Defender => State::Holding,
                        };
                    }
                }
            }
        }
    }

    for enemy in kill_list.drain(..) {
        if let Ok((_, mut s, _)) = soldiers.get_mut(enemy)
            && !matches!(s.state, State::Dead { .. })
        {
            s.state = State::Dead {
                timer: 0.0,
                ragdoll: false,
            };
        }
    }
}

fn loose_arrow(
    commands: &mut Commands,
    assets: &SoldierAssets,
    from: Vec3,
    target: Vec3,
    side: Side,
) {
    let origin = from + Vec3::Y * 1.2;
    let to = target - origin;
    let flat = Vec2::new(to.x, to.z).length();
    // Simple ballistic lead: aim up proportionally to distance.
    let direction = (to + Vec3::Y * (flat * 0.18)).normalize_or_zero();
    let velocity = direction * 30.0;
    commands.spawn((
        Arrow {
            velocity,
            from: side,
            life: 5.0,
        },
        Mesh3d(assets.arrow.clone()),
        MeshMaterial3d(assets.wood.clone()),
        Transform::from_translation(origin).looking_to(direction, Vec3::Y),
        Respawnable,
    ));
}

fn arrows(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<BattleGrid>,
    mut arrows: Query<(Entity, &mut Arrow, &mut Transform), Without<Soldier>>,
    mut soldiers: Query<&mut Soldier>,
) {
    let dt = time.delta_secs();
    for (entity, mut arrow, mut transform) in &mut arrows {
        arrow.life -= dt;
        arrow.velocity.y -= 9.81 * dt;
        let velocity = arrow.velocity;
        transform.translation += velocity * dt;
        if velocity.length_squared() > 0.1 {
            let dir = velocity.normalize();
            transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
        }
        let pos = transform.translation;
        let grounded = pos.y < terrain_height(pos.x, pos.z);
        if arrow.life <= 0.0 || grounded {
            commands.entity(entity).despawn();
            continue;
        }
        // Hit check against nearby enemies only (grid lookup).
        let mut hit: Option<Entity> = None;
        grid.near(pos, 0.95, |candidate, _, side| {
            if hit.is_none() && side != arrow.from {
                hit = Some(candidate);
            }
        });
        if let Some(candidate) = hit
            && let Ok(mut soldier) = soldiers.get_mut(candidate)
            && !matches!(soldier.state, State::Dead { .. })
        {
            soldier.state = State::Dead {
                timer: 0.0,
                ragdoll: false,
            };
            commands.entity(entity).despawn();
        }
    }
}

/// Trebuchet blasts: anyone in the radius is thrown.
fn blast_kills(
    mut commands: Commands,
    mut events: MessageReader<BlastEvent>,
    mut soldiers: Query<(Entity, &mut Soldier, &Transform)>,
) {
    for event in events.read() {
        for (entity, mut soldier, transform) in &mut soldiers {
            if matches!(soldier.state, State::Dead { .. }) {
                continue;
            }
            let offset = transform.translation - event.position;
            if offset.length() < event.radius * 1.25 {
                soldier.state = State::Dead {
                    timer: 0.0,
                    ragdoll: true,
                };
                commands.entity(entity).try_insert((
                    RigidBody::Dynamic,
                    LinearVelocity(
                        (offset.normalize_or_zero() * 9.0 + Vec3::Y * 7.0).adjust_precision(),
                    ),
                ));
            }
        }
    }
}

/// Fast debris and stones plow through soldiers.
fn debris_kills(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    movers: Query<&PreTickVelocity>,
    mut soldiers: Query<(Entity, &mut Soldier)>,
) {
    for event in events.read() {
        let (soldier_entity, other) = if soldiers.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if soldiers.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };
        let Ok(velocity) = movers.get(other) else {
            continue;
        };
        if velocity.0.length() < 5.0 {
            continue;
        }
        if let Ok((entity, mut soldier)) = soldiers.get_mut(soldier_entity)
            && !matches!(soldier.state, State::Dead { .. })
        {
            soldier.state = State::Dead {
                timer: 0.0,
                ragdoll: true,
            };
            commands.entity(entity).try_insert((
                RigidBody::Dynamic,
                LinearVelocity((velocity.0 * 0.5 + Vec3::Y * 4.0).adjust_precision()),
            ));
        }
    }
}

/// Periodic battle status in the log (`FL_BATTLE_LOG=1`), for headless
/// verification and tuning.
fn battle_log(
    time: Res<Time>,
    soldiers: Query<(&Soldier, &Transform)>,
    mut last: Local<f32>,
) {
    if time.elapsed_secs() - *last < 8.0 {
        return;
    }
    *last = time.elapsed_secs();
    let (mut attackers, mut defenders, mut dead) = (0, 0, 0);
    let mut lead_z = f32::MAX;
    for (soldier, transform) in &soldiers {
        if matches!(soldier.state, State::Dead { .. }) {
            dead += 1;
            continue;
        }
        match soldier.side {
            Side::Attacker => {
                attackers += 1;
                lead_z = lead_z.min(transform.translation.z);
            }
            Side::Defender => defenders += 1,
        }
    }
    info!("battle: {attackers} attackers (lead z={lead_z:.0}), {defenders} defenders, {dead} down");
}

/// War horn at the start; victory banner when the last defender falls.
fn battle_state(
    mut commands: Commands,
    mut battle: ResMut<Battle>,
    mut sounds: MessageWriter<SoundEvent>,
    soldiers: Query<&Soldier>,
    players: Query<&Transform, With<Player>>,
) {
    if !battle.horn_blown {
        battle.horn_blown = true;
        sounds.write(SoundEvent {
            kind: SoundKind::Horn,
            position: Vec3::new(0.0, 6.0, -30.0),
            intensity: 1.0,
        });
    }

    if battle.victory_announced {
        return;
    }
    let mut any_defender = false;
    let mut any_soldier = false;
    for soldier in &soldiers {
        if matches!(soldier.state, State::Dead { .. }) {
            continue;
        }
        any_soldier = true;
        if soldier.side == Side::Defender {
            any_defender = true;
            break;
        }
    }
    if any_soldier && !any_defender {
        battle.victory_announced = true;
        let position = players
            .single()
            .map(|t| t.translation)
            .unwrap_or(Vec3::ZERO);
        sounds.write(SoundEvent {
            kind: SoundKind::Horn,
            position: position + Vec3::Y * 4.0,
            intensity: 1.0,
        });
        commands.spawn((
            VictoryBanner,
            Respawnable,
            Text::new("THE CASTLE HAS FALLEN"),
            TextFont {
                font_size: 44.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.82, 0.3)),
            Node {
                position_type: PositionType::Absolute,
                top: percent(30),
                justify_self: JustifySelf::Center,
                ..default()
            },
        ));
    }
}
