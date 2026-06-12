//! Debug tooling: built-in FPS counter (F3 toggles, on by default),
//! F2 screenshot, F4 vsync toggle.
//!
//! For headless/CI verification, set `ENGINE_AUTO_SCREENSHOT=<frame>` to
//! capture a screenshot at that frame and exit once it has been saved.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Capturing, Screenshot, save_to_disk};
use bevy::window::{PresentMode, PrimaryWindow};
use std::time::{SystemTime, UNIX_EPOCH};

/// Whether the FPS counter is shown. Toggled by F3 and the pause menu.
#[derive(Resource)]
pub struct FpsCounterEnabled(pub bool);

impl Default for FpsCounterEnabled {
    fn default() -> Self {
        Self(true)
    }
}

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<FpsCounterEnabled>()
            .add_systems(Startup, spawn_fps_counter)
            .add_systems(
                Update,
                (
                    update_fps_counter,
                    toggle_fps_on_f3,
                    screenshot_on_f2,
                    toggle_vsync_on_f4,
                ),
            );

        if let Some(at_frame) = std::env::var("ENGINE_AUTO_SCREENSHOT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.insert_resource(AutoScreenshot {
                at_frame,
                taken: false,
            });
            app.add_systems(Update, auto_screenshot);
        }
    }
}

#[derive(Component)]
struct FpsCounterText;

fn spawn_fps_counter(mut commands: Commands) {
    commands.spawn((
        FpsCounterText,
        Text::new("FPS --"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.4, 1.0, 0.5)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            left: px(8),
            ..default()
        },
        // Above any menu overlay.
        GlobalZIndex(i32::MAX),
    ));
}

fn update_fps_counter(
    diagnostics: Res<DiagnosticsStore>,
    enabled: Res<FpsCounterEnabled>,
    mut counter: Query<(&mut Text, &mut Visibility), With<FpsCounterText>>,
) {
    for (mut text, mut visibility) in &mut counter {
        *visibility = if enabled.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if !enabled.0 {
            continue;
        }
        if let Some(fps) = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
        {
            text.0 = format!("FPS {fps:.0}");
        }
    }
}

fn toggle_fps_on_f3(input: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<FpsCounterEnabled>) {
    if input.just_pressed(KeyCode::F3) {
        enabled.0 = !enabled.0;
    }
}

fn screenshot_on_f2(mut commands: Commands, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::F2) {
        take_screenshot(&mut commands);
    }
}

fn take_screenshot(commands: &mut Commands) {
    if let Err(err) = std::fs::create_dir_all("screenshots") {
        error!("could not create screenshots directory: {err}");
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = format!("screenshots/screenshot-{timestamp}.png");
    info!("saving screenshot to {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

#[derive(Resource)]
struct AutoScreenshot {
    at_frame: u32,
    taken: bool,
}

fn auto_screenshot(
    mut commands: Commands,
    mut state: ResMut<AutoScreenshot>,
    mut frame: Local<u32>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if !state.taken && *frame >= state.at_frame {
        take_screenshot(&mut commands);
        state.taken = true;
    } else if state.taken && capturing.is_empty() && *frame > state.at_frame + 5 {
        // The extra frames cover the gap before `Capturing` is attached.
        exit.write(AppExit::Success);
    }
}

fn toggle_vsync_on_f4(
    input: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if input.just_pressed(KeyCode::F4) {
        if let Ok(mut window) = windows.single_mut() {
            window.present_mode = match window.present_mode {
                PresentMode::AutoNoVsync => PresentMode::AutoVsync,
                _ => PresentMode::AutoNoVsync,
            };
            info!("present mode: {:?}", window.present_mode);
        }
    }
}
