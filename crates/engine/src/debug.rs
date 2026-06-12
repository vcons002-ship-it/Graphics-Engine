//! Debug tooling: F2 screenshot, F3 FPS overlay toggle (with the `dev_tools`
//! feature), F4 vsync toggle.
//!
//! For headless/CI verification, set `ENGINE_AUTO_SCREENSHOT=<frame>` to
//! capture a screenshot at that frame and exit once it has been saved.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Capturing, Screenshot, save_to_disk};
use bevy::window::{PresentMode, PrimaryWindow};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (screenshot_on_f2, toggle_vsync_on_f4));

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

        #[cfg(feature = "dev_tools")]
        {
            use bevy::dev_tools::fps_overlay::{
                FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig,
            };
            app.add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: false,
                        min_fps: 30.0,
                        target_fps: 144.0,
                    },
                    ..default()
                },
            });
            app.add_systems(Update, toggle_fps_overlay_on_f3);
        }
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

#[cfg(feature = "dev_tools")]
fn toggle_fps_overlay_on_f3(
    input: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<bevy::dev_tools::fps_overlay::FpsOverlayConfig>,
) {
    if input.just_pressed(KeyCode::F3) {
        overlay.enabled = !overlay.enabled;
    }
}
