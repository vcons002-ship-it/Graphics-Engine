//! Physics setup and conventions (avian3d).
//!
//! Conventions (see CLAUDE.md): dynamic objects use `RigidBody::Dynamic` with
//! primitive colliders sized to the visual mesh; static world geometry uses
//! `RigidBody::Static`. Add `TransformInterpolation` to anything that moves.

use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default());
    }
}
