//! Arcade-style destruction tally: individual wakes and shatters aggregate
//! into one "+N STONES" banner per volley instead of UI spam, with a
//! running total. Masonry systems increment [`DestructionTally`]; the
//! banner appears once the burst quiets down.

use bevy::prelude::*;

pub struct ScoringPlugin;

impl Plugin for ScoringPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DestructionTally>()
            .add_systems(Update, destruction_banner);
    }
}

/// Aggregated destruction counters. Broken-loose stones count 1,
/// shattered stones 3.
#[derive(Resource, Default)]
pub struct DestructionTally {
    pub pending: u32,
    pub quiet: f32,
    pub total: u64,
}

impl DestructionTally {
    pub fn add(&mut self, points: u32) {
        self.pending += points;
        self.quiet = 0.0;
    }
}

#[derive(Component)]
struct Banner {
    fade: Timer,
}

fn destruction_banner(
    mut commands: Commands,
    time: Res<Time>,
    mut tally: ResMut<DestructionTally>,
    mut banners: Query<(Entity, &mut Banner, &mut TextColor)>,
) {
    // Wait for the burst to quiet down (~0.8 s without new destruction)
    // before showing one aggregated banner.
    if tally.pending > 0 {
        tally.quiet += time.delta_secs();
        if tally.quiet > 0.8 {
            let points = tally.pending;
            tally.total += points as u64;
            let total = tally.total;
            tally.pending = 0;

            for (entity, ..) in &banners {
                commands.entity(entity).try_despawn();
            }
            commands.spawn((
                Banner {
                    fade: Timer::from_seconds(2.8, TimerMode::Once),
                },
                Text::new(format!("DESTRUCTION  +{points}   (total {total})")),
                TextFont {
                    font_size: 30.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.78, 0.25)),
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(18),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ));
        }
    }

    for (entity, mut banner, mut color) in &mut banners {
        banner.fade.tick(time.delta());
        let remaining = banner.fade.fraction_remaining();
        color.0.set_alpha(remaining.min(0.4) / 0.4);
        if banner.fade.just_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}
