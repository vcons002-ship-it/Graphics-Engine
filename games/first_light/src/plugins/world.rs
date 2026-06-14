//! World wiring: sun, player spawn, controls help, and restart handling.

use bevy::light::FogVolume;
use bevy::prelude::*;
use engine::prelude::*;

use super::terrain::{PLAYGROUND_CENTER, terrain_height};

pub struct WorldPlugin;

/// Everything that should be despawned and rebuilt on "Restart" (the player
/// and all dynamic props). Static scenery stays.
#[derive(Component)]
pub struct Respawnable;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // The valley runs along Z with the castle on the north headwall; the
        // player spawns facing it. Late-afternoon sun from the west-southwest
        // cross-lights the castle face and rakes long shadows down the valley.
        app.insert_resource(SunSettings {
            direction: Vec3::new(-0.9, -0.32, -0.45),
            shadow_distance: 420.0,
            first_cascade_far_bound: 18.0,
            ..default()
        })
        .insert_resource(ControlsHelp(
            [
                ("Move", "W A S D / arrows"),
                ("Look", "Mouse"),
                ("Jump", "Space"),
                ("Sprint", "Left Shift"),
                ("Throw cube", "Left click"),
                ("Man trebuchet", "E (when near)"),
                ("Wind / loose", "Hold / release Left click"),
                ("Menu", "Esc"),
                ("Screenshot", "F2"),
                ("FPS counter", "F3"),
                ("VSync", "F4"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .to_vec(),
        ))
        .add_systems(Startup, (spawn_player_on_terrain, spawn_valley_fog, spawn_birds))
        .add_systems(Update, fly_birds)
        .add_systems(
            Update,
            (despawn_respawnables, spawn_player_on_terrain)
                .chain()
                .run_if(on_message::<RestartRequested>),
        );
    }
}

/// A bird wheeling high over the valley.
#[derive(Component)]
struct Bird {
    center: Vec2,
    radius: f32,
    height: f32,
    speed: f32,
    phase: f32,
}

/// A simple swept-wing silhouette (one double-sided triangle).
fn bird_mesh() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::Indices;
    use bevy::render::render_resource::PrimitiveTopology;
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0, 0.0, 0.35], [-0.6, 0.06, -0.2], [0.6, 0.06, -0.2]],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 3])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn spawn_birds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(bird_mesh());
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.07, 0.09),
        perceptual_roughness: 1.0,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    for i in 0..18u32 {
        let f = i as f32;
        let phase = f * 0.7;
        commands.spawn((
            Bird {
                center: Vec2::new(((f * 53.0).sin()) * 120.0, -70.0 + (f * 31.0).cos() * 90.0),
                radius: 22.0 + (f * 17.0).sin().abs() * 30.0,
                height: 72.0 + (f * 11.0).cos() * 14.0,
                speed: 0.12 + (f * 7.0).sin().abs() * 0.08,
                phase,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::splat(2.6)),
        ));
    }
}

fn fly_birds(time: Res<Time>, mut birds: Query<(&Bird, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (bird, mut transform) in &mut birds {
        let a = t * bird.speed + bird.phase;
        transform.translation = Vec3::new(
            bird.center.x + a.cos() * bird.radius,
            bird.height + (t * 1.3 + bird.phase).sin() * 2.5,
            bird.center.y + a.sin() * bird.radius,
        );
        // Face the direction of travel, with a gentle wing-bank flap.
        let yaw = (a.cos()).atan2(-a.sin());
        let bank = (t * 5.0 + bird.phase).sin() * 0.25;
        transform.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(bank);
        transform.scale = Vec3::splat(2.6);
    }
}

fn spawn_valley_fog(mut commands: Commands) {
    // A thin haze layer hugging the valley floor: catches god rays where
    // the sun cuts past the castle and the mountain shoulders.
    // Density is per-meter optical depth: across a 500 m valley even small
    // values add up, so keep it thin or the sky goes black.
    commands.spawn((
        FogVolume {
            density_factor: 0.0025,
            ..default()
        },
        Transform::from_xyz(0.0, 12.0, -40.0).with_scale(Vec3::new(560.0, 36.0, 520.0)),
    ));
}

fn spawn_player_on_terrain(mut commands: Commands) {
    // On the playground pad, looking north (-Z) toward the castle. Overridable
    // for headless verification: FL_SPAWN="x,z,yaw_degrees".
    let (x, z, yaw) = std::env::var("FL_SPAWN")
        .ok()
        .and_then(|v| {
            let p: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            (p.len() == 3).then(|| (p[0], p[1], p[2]))
        })
        .unwrap_or((6.0, PLAYGROUND_CENTER.y + 20.0, 0.0));

    let y = terrain_height(x, z) + 1.5;
    let player = spawn_player(&mut commands, Vec3::new(x, y, z));
    commands
        .entity(player)
        .insert((
            Respawnable,
            Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(yaw.to_radians())),
        ));
}

fn despawn_respawnables(mut commands: Commands, entities: Query<Entity, With<Respawnable>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
