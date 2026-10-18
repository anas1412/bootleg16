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

/// The screen's size, once known (the menu lists only the sizes that fit on it).
static SCREEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The indexes of `RESOLUTIONS` that fit on the screen (all of them until it is known).
pub fn sizes_for_screen() -> Vec<u8> {
    let s = SCREEN.load(std::sync::atomic::Ordering::Relaxed);
    let (sw, sh) = ((s >> 32) as u32, s as u32);
    let fits: Vec<u8> = (0..RESOLUTIONS.len() as u8).filter(|&i| s == 0 || (RESOLUTIONS[i as usize].0 <= sw && RESOLUTIONS[i as usize].1 <= sh)).collect();
    if fits.is_empty() { vec![0] } else { fits }
}

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
fn alt_enter(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<Settings>, mut dirty: ResMut<crate::menu::MenuDirty>) {
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if alt && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        settings.borderless = !settings.borderless;
        settings.save();
        dirty.0 = true; // an open Settings page shows the new mode
    }
}

/// Make the window what the settings say, when that changes (and once at the start); a window
/// you resized by hand is left alone until then.
fn apply(settings: Res<Settings>, monitors: Query<(&Monitor, Has<PrimaryMonitor>)>, mut windows: Query<&mut Window>, mut last: Local<Option<(bool, (u32, u32))>>) {
    // (Wayland has no primary monitor: then the biggest one, which on one screen is the screen.)
    let screen = monitors.iter().max_by_key(|(m, primary)| (*primary, m.physical_width * m.physical_height)).map(|(m, _)| (m.physical_width, m.physical_height));
    if let Some((sw, sh)) = screen {
        SCREEN.store((sw as u64) << 32 | sh as u64, std::sync::atomic::Ordering::Relaxed);
    }
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
    fn only_sizes_that_fit_the_screen_are_offered() {
        SCREEN.store(1920 << 32 | 1080, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(sizes_for_screen(), vec![0, 1, 2]);
        SCREEN.store(0, std::sync::atomic::Ordering::Relaxed);
    }

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
