//! Game-agnostic engine layer: rendering defaults, lighting, physics,
//! first-person player controller, and debug tooling. Games compose these
//! plugins and add their own scene content on top.

pub mod camera;
pub mod debug;
pub mod lighting;
pub mod menu;
pub mod physics;
pub mod player;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

/// Everything a game needs from the engine. Games can disable individual
/// plugins via `EnginePlugins.build().disable::<...>()` if needed.
pub struct EnginePlugins;

impl PluginGroup for EnginePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(physics::PhysicsPlugin)
            .add(camera::CameraPlugin)
            .add(lighting::LightingPlugin)
            .add(player::PlayerPlugin)
            .add(menu::MenuPlugin)
            .add(debug::DebugPlugin)
    }
}

pub mod prelude {
    pub use crate::EnginePlugins;
    pub use crate::camera::MainCamera;
    pub use crate::debug::FpsCounterEnabled;
    pub use crate::lighting::SunSettings;
    pub use crate::menu::{ControlsHelp, MenuState, RestartRequested};
    pub use crate::player::{CursorLocked, Player, spawn_player};
}
