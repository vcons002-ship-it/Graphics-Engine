//! Environment: ground and player spawn.

use avian3d::prelude::*;
use bevy::prelude::*;
use engine::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // The player spawns facing -Z. Late-afternoon sun low in the west,
        // to the player's right and slightly behind: faces the player sees
        // are cross-lit and long shadows rake left across the view.
        app.insert_resource(SunSettings {
            // TEMP DIAGNOSTIC: near-overhead sun to make cast shadows unmissable.
            direction: Vec3::new(-0.25, -1.0, -0.25),
            ..default()
        })
        .add_systems(Startup, setup_world);
    }
}

const GROUND_SIZE: f32 = 400.0;
const GROUND_THICKNESS: f32 = 0.2;

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground: a thin slab whose top face sits at y = 0, with a collider that
    // matches the visual mesh exactly.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(GROUND_SIZE, GROUND_THICKNESS, GROUND_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.38, 0.31),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -GROUND_THICKNESS / 2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(GROUND_SIZE, GROUND_THICKNESS, GROUND_SIZE),
    ));

    // Player spawns facing -Z, toward the crate stack and props.
    spawn_player(&mut commands, Vec3::new(0.0, 1.5, 10.0));
}
