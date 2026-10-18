//! The window, as most games do it: a window at a chosen size (Settings, Window size), or
//! borderless (no frame, the size of the screen, Settings or Alt + Enter). Never the
//! compositor's own fullscreen mode: on the Intel + NVIDIA laptop that took KWin down (see
//! docs/GOTCHAS.md). Also a test hook, `BOOTLEG16_RESIZE=w,h` (or `drag`), that resizes the
//! window a few seconds in.
use crate::menu::Settings;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor};

/// The sizes the window can take, and the one it starts at.
pub const RESOLUTIONS: [(u32, u32); 5] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];
pub const DEFAULT_RESOLUTION: u8 = 2;

pub struct WindowPlugin;

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (alt_enter, apply, resize_for_test).chain());
    }
}

/// What the window should be: its frame, and its size (the screen's when borderless).
pub fn target(settings: &Settings, screen: Option<(u32, u32)>) -> (bool, (u32, u32)) {
    match (settings.borderless, screen) {
        (true, Some(size)) => (false, size),
        _ => (true, RESOLUTIONS[settings.resolution as usize]),
    }
}

/// Alt + Enter switches between the window and borderless, and is remembered.
fn alt_enter(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<Settings>) {
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if alt && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        settings.borderless = !settings.borderless;
        settings.save();
    }
}

/// Make the window what the settings say, when that changes (and once at the start); a window
/// you resized by hand is left alone until then.
fn apply(settings: Res<Settings>, monitor: Option<Single<&Monitor, With<PrimaryMonitor>>>, mut windows: Query<&mut Window>, mut last: Local<Option<(bool, (u32, u32))>>) {
    let screen = monitor.map(|m| (m.physical_width, m.physical_height));
    if settings.borderless && screen.is_none() {
        return;
    }
    let want = target(&settings, screen);
    if *last == Some(want) {
        return;
    }
    *last = Some(want);
    let (frame, (w, h)) = want;
    for mut win in &mut windows {
        if win.decorations != frame {
            win.decorations = frame;
        }
        let scale = win.scale_factor();
        win.resolution.set(w as f32 / scale, h as f32 / scale);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_takes_its_size_and_borderless_takes_the_screen() {
        let mut s = Settings::default();
        assert_eq!(target(&s, Some((2560, 1440))), (true, (1920, 1080)));
        s.resolution = 0;
        assert_eq!(target(&s, None), (true, (1280, 720)));
        s.borderless = true;
        assert_eq!(target(&s, Some((2560, 1440))), (false, (2560, 1440)));
        // no screen known yet: stay a window rather than guess
        assert_eq!(target(&s, None).0, true);
    }
}
