//! The window: fullscreen from the settings (borderless on the current monitor), and a test
//! hook, `BOOTLEG16_RESIZE=w,h`, that resizes the window a few seconds in (to check what a
//! big window does to the render targets).
use crate::menu::Settings;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_fullscreen, resize_for_test));
    }
}

/// (Not before the window has been up a moment: asking at creation can't pick the monitor.)
fn apply_fullscreen(settings: Res<Settings>, time: Res<Time>, mut windows: Query<&mut Window>) {
    if time.elapsed_secs() < 1.5 {
        return;
    }
    let mode = if settings.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
    for mut w in &mut windows {
        if w.mode != mode {
            w.mode = mode;
        }
    }
}

fn resize_for_test(time: Res<Time>, mut windows: Query<&mut Window>, mut done: Local<bool>) {
    let Some((w, h)) = std::env::var("BOOTLEG16_RESIZE").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?)))) else { return };
    if !*done && time.elapsed_secs() > 4.0 {
        *done = true;
        for mut win in &mut windows {
            win.resolution.set(w, h);
        }
    }
}
