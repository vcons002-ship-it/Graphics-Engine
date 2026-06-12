//! Rendering defaults for the main gameplay camera.
//!
//! Spawn an entity with [`MainCamera`] and this plugin decorates it with the
//! engine's release-quality defaults: HDR rendering (required by `Bloom` and
//! `Atmosphere`), TonyMcMapface tonemapping, subtle bloom, physical exposure,
//! a procedural atmosphere, and FXAA as the single anti-aliasing path.

use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

/// Marker for the primary gameplay camera. Spawning this is all a game needs
/// to do; rendering defaults are applied by [`CameraPlugin`].
#[derive(Component)]
#[require(Camera3d)]
pub struct MainCamera;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_render_defaults);
    }
}

/// The atmosphere's `RAW_SUNLIGHT` sun is bright, so compensate exposure
/// upward (see the bevy v0.18.1 `atmosphere` example).
const DEFAULT_EV100: f32 = 13.0;

fn apply_render_defaults(
    mut commands: Commands,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
    cameras: Query<Entity, (With<MainCamera>, Without<Atmosphere>)>,
) {
    for entity in &cameras {
        commands.entity(entity).insert((
            Tonemapping::TonyMcMapface,
            Bloom::NATURAL,
            Exposure { ev100: DEFAULT_EV100 },
            Atmosphere::earthlike(scattering_mediums.add(ScatteringMedium::default())),
            AtmosphereSettings::default(),
            // Atmosphere drives ambient light and reflections (IBL).
            AtmosphereEnvironmentMapLight::default(),
            Msaa::Off,
            Fxaa::default(),
        ));
    }
}
