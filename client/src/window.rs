//! The window: fullscreen from the settings (borderless on the current monitor; on by default,
//! Alt + Enter toggles it), and a test
//! hook, `BOOTLEG16_RESIZE=w,h`, that resizes the window a few seconds in (to check what a
//! big window does to the render targets).
use crate::menu::Settings;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (alt_enter, apply_fullscreen, resize_for_test).chain());
    }
}

/// Alt + Enter switches between fullscreen and a window, and is remembered.
fn alt_enter(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<Settings>) {
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if alt && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        settings.fullscreen = !settings.fullscreen;
        settings.save();
    }
}

/// (Not before the window has been up a moment: asking at creation can't pick the monitor.)
fn apply_fullscreen(settings: Res<Settings>, time: Res<Time>, mut windows: Query<&mut Window>) {
    if time.elapsed_secs() < 1.5 {
        return;
    }
    let mode = if settings.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Primary) } else { WindowMode::Windowed };
    for mut w in &mut windows {
        if w.mode != mode {
            w.mode = mode;
            if mode == WindowMode::Windowed {
                w.resolution.set(1920.0, 1080.0);
            }
        }
    }
}

fn resize_for_test(time: Res<Time>, mut windows: Query<&mut Window>, mut done: Local<bool>, mut frame: Local<u32>) {
    // `drag`: a size every frame, as dragging the window edge does: up to 1900 wide, then back.
    if std::env::var("BOOTLEG16_RESIZE").is_ok_and(|v| v == "drag") && time.elapsed_secs() > 4.0 {
        *frame += 1;
        let step = (*frame % 200) as f32;
        let k = if step < 100.0 { step } else { 200.0 - step };
        for mut win in &mut windows {
            win.resolution.set(1280.0 + 6.2 * k, 720.0 + 3.6 * k);
        }
        return;
    }
    let Some((w, h)) = std::env::var("BOOTLEG16_RESIZE").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?)))) else { return };
    if !*done && time.elapsed_secs() > 4.0 {
        *done = true;
        for mut win in &mut windows {
            win.resolution.set(w, h);
        }
    }
}
