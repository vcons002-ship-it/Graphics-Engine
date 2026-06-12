//! Minimal control scene for shadow diagnosis: plane + cube + shadow-casting
//! directional light, no engine plugins/atmosphere/physics. Saves
//! `screenshots/probe.png` at frame 60 and exits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, shoot)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.7, 0.7, 0.7))),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.6, 0.4))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.5, -1.0, -0.3), Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-3.0, 4.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn shoot(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    *frame += 1;
    if *frame == 60 {
        std::fs::create_dir_all("screenshots").ok();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("screenshots/probe.png".to_string()));
    }
    if *frame == 120 {
        exit.write(AppExit::Success);
    }
}
