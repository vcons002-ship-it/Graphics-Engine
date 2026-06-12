//! First Light — a sunlit first-person physics playground.
//!
//! App builder only: engine plugins + this game's plugins.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use engine::prelude::*;

mod plugins;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "First Light".into(),
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EnginePlugins)
        .add_plugins((
            plugins::world::WorldPlugin,
            plugins::props::PropsPlugin,
            plugins::throw::ThrowPlugin,
        ))
        .run();
}
