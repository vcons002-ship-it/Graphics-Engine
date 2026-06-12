//! Sun and global lighting.
//!
//! One shadow-casting directional sun whose direction/strength comes from
//! [`SunSettings`]. Games override the defaults by inserting their own
//! `SunSettings` resource before `Startup` runs. Global flat ambient is
//! disabled because the camera's `AtmosphereEnvironmentMapLight` provides
//! physically based ambient instead.

use bevy::light::{CascadeShadowConfigBuilder, light_consts::lux};
use bevy::prelude::*;

/// Sun configuration, applied once at startup.
#[derive(Resource, Clone)]
pub struct SunSettings {
    /// Direction the sunlight travels (normalized on use).
    pub direction: Vec3,
    /// Illuminance in lux. `lux::RAW_SUNLIGHT` is the correct input when the
    /// atmosphere does the scattering.
    pub illuminance: f32,
    /// Furthest distance (meters) that receives shadows.
    pub shadow_distance: f32,
    /// Far bound of the first (sharpest) shadow cascade.
    pub first_cascade_far_bound: f32,
}

impl Default for SunSettings {
    fn default() -> Self {
        Self {
            // Late afternoon: low sun in the west, ~19 degrees above horizon.
            direction: Vec3::new(-1.0, -0.35, 0.25),
            illuminance: lux::RAW_SUNLIGHT,
            shadow_distance: 150.0,
            first_cascade_far_bound: 12.0,
        }
    }
}

/// Marker for the sun entity.
#[derive(Component)]
pub struct Sun;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SunSettings>()
            .insert_resource(GlobalAmbientLight::NONE)
            .add_systems(Startup, spawn_sun);
    }
}

fn spawn_sun(mut commands: Commands, settings: Res<SunSettings>) {
    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: settings.illuminance,
            shadows_enabled: true,
            ..default()
        },
        Transform::default().looking_to(settings.direction.normalize(), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: settings.first_cascade_far_bound,
            maximum_distance: settings.shadow_distance,
            ..default()
        }
        .build(),
    ));
}
