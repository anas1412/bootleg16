//! Frame rate: VSync on or off and an fps cap (CS's `fps_max`), from the settings.
use crate::menu::Settings;
use bevy::prelude::*;
use bevy::window::PresentMode;
use std::time::{Duration, Instant};

pub struct FrameratePlugin;

impl Plugin for FrameratePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_vsync).add_systems(Last, cap);
    }
}

fn apply_vsync(settings: Res<Settings>, mut windows: Query<&mut Window>) {
    // Not AutoNoVsync: on KDE Wayland it ended up capped at 60; Mailbox runs free without tearing.
    let mode = if settings.vsync { PresentMode::AutoVsync } else { PresentMode::Mailbox };
    for mut w in &mut windows {
        if w.present_mode != mode {
            w.present_mode = mode;
        }
    }
}

/// Wait out the rest of the frame when we're faster than the cap.
fn cap(settings: Res<Settings>, mut last: Local<Option<Instant>>) {
    if settings.fps_max > 0 {
        let frame = Duration::from_secs_f64(1.0 / settings.fps_max as f64);
        if let Some(t) = *last {
            let left = frame.saturating_sub(t.elapsed());
            // ponytail: plain sleep overshoots ~0.1 ms on Linux; spin the last bit if the cap feels uneven.
            if !left.is_zero() {
                std::thread::sleep(left);
            }
        }
    }
    *last = Some(Instant::now());
}
