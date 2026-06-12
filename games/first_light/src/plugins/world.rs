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
                ("Menu", "Esc"),
                ("Screenshot", "F2"),
                ("FPS counter", "F3"),
                ("VSync", "F4"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .to_vec(),
        ))
        .add_systems(Startup, (spawn_player_on_terrain, spawn_valley_fog))
        .add_systems(
            Update,
            (despawn_respawnables, spawn_player_on_terrain)
                .chain()
                .run_if(on_message::<RestartRequested>),
        );
    }
}

fn spawn_valley_fog(mut commands: Commands) {
    // A thin haze layer hugging the valley floor: catches god rays where
    // the sun cuts past the castle and the mountain shoulders.
    commands.spawn((
        FogVolume {
            density_factor: 0.045,
            ..default()
        },
        Transform::from_xyz(0.0, 28.0, -40.0).with_scale(Vec3::new(560.0, 90.0, 520.0)),
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
