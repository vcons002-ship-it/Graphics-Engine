//! Minimal control scene for shadow diagnosis: plane + cube + shadow-casting
//! directional light, no engine plugins/atmosphere/physics. Saves
//! `screenshots/probe.png` at frame 60 and exits.

use avian3d::prelude::*;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{CascadeShadowConfigBuilder, light_consts::lux};
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, (late_decorate, shoot))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
) {
    // Demo-scale geometry: huge ground slab, cube 14 m from the camera.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(400.0, 0.2, 400.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.7, 0.7, 0.7))),
        Transform::from_xyz(0.0, -0.1, 0.0),
        RigidBody::Static,
        Collider::cuboid(400.0, 0.2, 400.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.6, 0.4))),
        Transform::from_xyz(0.0, 0.6, -4.0),
        RigidBody::Dynamic,
        Collider::cuboid(1.0, 1.0, 1.0),
        TransformInterpolation,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: lux::RAW_SUNLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-1.0, -0.3, -0.25), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 12.0,
            maximum_distance: 150.0,
            ..default()
        }
        .build(),
    ));
    // Spawn the camera bare; decorations are inserted a few frames later by
    // `late_decorate`, mimicking the engine's CameraPlugin.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.1, 10.0).looking_at(Vec3::new(0.0, 1.0, -4.0), Vec3::Y),
    ));
}

fn late_decorate(
    mut commands: Commands,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
    cameras: Query<Entity, (With<Camera3d>, Without<Atmosphere>)>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if *frame < 5 {
        return;
    }
    for entity in &cameras {
        commands.entity(entity).insert((
            Tonemapping::TonyMcMapface,
            Bloom::NATURAL,
            Exposure { ev100: 13.0 },
            Atmosphere::earthlike(scattering_mediums.add(ScatteringMedium::default())),
            AtmosphereSettings::default(),
            Msaa::Off,
            Fxaa::default(),
        ));
    }
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
