//! Pause menu, opened with Esc: resume, controls reference, visual settings
//! (vsync, fullscreen, FPS counter), restart, and exit.
//!
//! Opening the menu pauses `Time<Virtual>` (freezing physics and gameplay
//! timers) and releases the cursor; closing it restores both. Games handle
//! [`RestartRequested`] to rebuild their world, and may replace the
//! [`ControlsHelp`] resource to document their own bindings.
//!
//! For headless verification, `ENGINE_AUTO_MENU=<frame>` opens the menu at
//! that frame.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, PresentMode, PrimaryWindow, WindowMode};

use crate::debug::FpsCounterEnabled;
use crate::player::CursorLocked;

/// Which menu screen is showing. `Closed` means gameplay is running.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum MenuState {
    #[default]
    Closed,
    Main,
    Controls,
    Settings,
}

/// Written when the player picks "Restart" — the game despawns and rebuilds
/// its world in response.
#[derive(Message)]
pub struct RestartRequested;

/// Rows shown on the Controls screen. Games replace this resource to
/// document their own bindings.
#[derive(Resource)]
pub struct ControlsHelp(pub Vec<(String, String)>);

impl Default for ControlsHelp {
    fn default() -> Self {
        Self(
            [
                ("Move", "W A S D / arrows"),
                ("Look", "Mouse"),
                ("Jump", "Space"),
                ("Sprint", "Left Shift"),
                ("Menu", "Esc"),
                ("Screenshot", "F2"),
                ("FPS counter", "F3"),
                ("VSync", "F4"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .to_vec(),
        )
    }
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .add_message::<RestartRequested>()
            .init_resource::<ControlsHelp>()
            .add_systems(Update, (toggle_on_esc, button_colors, menu_actions))
            .add_systems(OnExit(MenuState::Closed), pause_and_release_cursor)
            .add_systems(OnEnter(MenuState::Closed), resume_and_grab_cursor)
            .add_systems(OnEnter(MenuState::Main), spawn_main_screen)
            .add_systems(OnEnter(MenuState::Controls), spawn_controls_screen)
            .add_systems(OnEnter(MenuState::Settings), spawn_settings_screen)
            .add_systems(
                Update,
                refresh_setting_labels.run_if(in_state(MenuState::Settings)),
            );

        if let Some(at_frame) = std::env::var("ENGINE_AUTO_MENU")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.add_systems(Update, move |mut frame: Local<u32>,
                                          mut next: ResMut<NextState<MenuState>>| {
                *frame += 1;
                if *frame == at_frame {
                    next.set(MenuState::Main);
                }
            });
        }
    }
}

/// Actions attached to menu buttons.
#[derive(Component, Clone, Copy, PartialEq)]
enum MenuAction {
    Resume,
    Controls,
    Settings,
    Back,
    Restart,
    Exit,
    ToggleVsync,
    ToggleFullscreen,
    ToggleFps,
}

const PANEL_BG: Color = Color::srgba(0.06, 0.07, 0.09, 0.92);
const NORMAL_BUTTON: Color = Color::srgb(0.16, 0.17, 0.20);
const HOVERED_BUTTON: Color = Color::srgb(0.24, 0.26, 0.31);
const PRESSED_BUTTON: Color = Color::srgb(0.32, 0.36, 0.45);
const TEXT_COLOR: Color = Color::srgb(0.92, 0.92, 0.90);
const TITLE_COLOR: Color = Color::srgb(1.0, 0.85, 0.55);

fn toggle_on_esc(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<MenuState>>,
    mut next: ResMut<NextState<MenuState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match state.get() {
            MenuState::Closed => MenuState::Main,
            _ => MenuState::Closed,
        });
    }
}

fn pause_and_release_cursor(
    mut time: ResMut<Time<Virtual>>,
    mut locked: ResMut<CursorLocked>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    time.pause();
    locked.0 = false;
    if let Ok(mut cursor) = windows.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn resume_and_grab_cursor(
    mut time: ResMut<Time<Virtual>>,
    mut locked: ResMut<CursorLocked>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    time.unpause();
    locked.0 = true;
    if let Ok(mut cursor) = windows.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn button_colors(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed => PRESSED_BUTTON.into(),
            Interaction::Hovered => HOVERED_BUTTON.into(),
            Interaction::None => NORMAL_BUTTON.into(),
        };
    }
}

fn menu_actions(
    interactions: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<MenuState>>,
    mut restart: MessageWriter<RestartRequested>,
    mut exit: MessageWriter<AppExit>,
    mut fps_enabled: ResMut<FpsCounterEnabled>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::Resume => next.set(MenuState::Closed),
            MenuAction::Controls => next.set(MenuState::Controls),
            MenuAction::Settings => next.set(MenuState::Settings),
            MenuAction::Back => next.set(MenuState::Main),
            MenuAction::Restart => {
                restart.write(RestartRequested);
                next.set(MenuState::Closed);
            }
            MenuAction::Exit => {
                exit.write(AppExit::Success);
            }
            MenuAction::ToggleVsync => {
                if let Ok(mut window) = windows.single_mut() {
                    window.present_mode = match window.present_mode {
                        PresentMode::AutoNoVsync => PresentMode::AutoVsync,
                        _ => PresentMode::AutoNoVsync,
                    };
                }
            }
            MenuAction::ToggleFullscreen => {
                if let Ok(mut window) = windows.single_mut() {
                    window.mode = match window.mode {
                        WindowMode::Windowed => {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                        }
                        _ => WindowMode::Windowed,
                    };
                }
            }
            MenuAction::ToggleFps => fps_enabled.0 = !fps_enabled.0,
        }
    }
}

/// Root overlay node shared by every menu screen.
fn screen_root() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn panel() -> (Node, BackgroundColor) {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(px(28)),
            row_gap: px(10),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    )
}

fn title(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 38.0,
            ..default()
        },
        TextColor(TITLE_COLOR),
        Node {
            margin: UiRect::bottom(px(14)),
            ..default()
        },
    )
}

fn button(label: &str, action: MenuAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            width: px(260),
            height: px(46),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(NORMAL_BUTTON),
        children![(
            Text::new(label),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(TEXT_COLOR),
        )],
    )
}

fn spawn_main_screen(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(MenuState::Main),
        screen_root(),
        children![(
            panel(),
            children![
                title("Paused"),
                button("Resume", MenuAction::Resume),
                button("Controls", MenuAction::Controls),
                button("Settings", MenuAction::Settings),
                button("Restart", MenuAction::Restart),
                button("Exit Game", MenuAction::Exit),
            ],
        )],
    ));
}

fn spawn_controls_screen(mut commands: Commands, controls: Res<ControlsHelp>) {
    let rows: Vec<_> = controls
        .0
        .iter()
        .map(|(action, binding)| row(action, binding))
        .collect();

    commands
        .spawn((DespawnOnExit(MenuState::Controls), screen_root()))
        .with_children(|root| {
            root.spawn(panel()).with_children(|p| {
                p.spawn(title("Controls"));
                for bundle in rows {
                    p.spawn(bundle);
                }
                p.spawn(button("Back", MenuAction::Back));
            });
        });
}

/// One "Action ........ Binding" line on the controls screen.
fn row(left: &str, right: &str) -> impl Bundle {
    (
        Node {
            width: px(380),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        children![
            (
                Text::new(left),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            ),
            (
                Text::new(right),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.75, 0.9)),
            ),
        ],
    )
}

/// Markers for settings-value labels that live on the buttons.
#[derive(Component)]
struct VsyncLabel;
#[derive(Component)]
struct FullscreenLabel;
#[derive(Component)]
struct FpsLabel;

fn spawn_settings_screen(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(MenuState::Settings),
        screen_root(),
        children![(
            panel(),
            children![
                title("Settings"),
                labeled_button(VsyncLabel, MenuAction::ToggleVsync),
                labeled_button(FullscreenLabel, MenuAction::ToggleFullscreen),
                labeled_button(FpsLabel, MenuAction::ToggleFps),
                button("Back", MenuAction::Back),
            ],
        )],
    ));
}

/// A settings button whose text is kept current by [`refresh_setting_labels`].
fn labeled_button(marker: impl Component, action: MenuAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            width: px(300),
            height: px(46),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(NORMAL_BUTTON),
        children![(
            marker,
            Text::new(""),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(TEXT_COLOR),
        )],
    )
}

fn refresh_setting_labels(
    windows: Query<&Window, With<PrimaryWindow>>,
    fps_enabled: Res<FpsCounterEnabled>,
    mut vsync: Query<&mut Text, (With<VsyncLabel>, Without<FullscreenLabel>, Without<FpsLabel>)>,
    mut fullscreen: Query<&mut Text, (With<FullscreenLabel>, Without<VsyncLabel>, Without<FpsLabel>)>,
    mut fps: Query<&mut Text, (With<FpsLabel>, Without<VsyncLabel>, Without<FullscreenLabel>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let vsync_on = !matches!(window.present_mode, PresentMode::AutoNoVsync);
    let fullscreen_on = !matches!(window.mode, WindowMode::Windowed);

    let on_off = |on: bool| if on { "On" } else { "Off" };
    for mut text in &mut vsync {
        text.0 = format!("VSync: {}", on_off(vsync_on));
    }
    for mut text in &mut fullscreen {
        text.0 = format!("Fullscreen: {}", on_off(fullscreen_on));
    }
    for mut text in &mut fps {
        text.0 = format!("FPS Counter: {}", on_off(fps_enabled.0));
    }
}
