//! The voice settings page (menu -> Voice): on/off, both push-to-talk keys (rebind by
//! pressing one), microphone, volumes, and a live mic meter to test with.
use super::{Audio, Bind, Devices, MicLevel, VoiceSettings, KEYS, MICE};
use crate::ui::{button, text, UiAction, MUTED, TEXT};
use bevy::prelude::*;
use net::protocol::VoiceChannel;

/// A change made on the voice settings page.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceAction {
    Toggle,
    Rebind(VoiceChannel),
    NextMic,
    MicVolume(f32),
    Volume(f32),
}

/// Apply a voice settings change (called from the menu).
pub fn apply(s: &mut VoiceSettings, devices: &Devices, a: VoiceAction) {
    match a {
        VoiceAction::Toggle => s.on = !s.on,
        VoiceAction::Rebind(ch) => s.capture = Some(ch),
        VoiceAction::NextMic => {
            let i = devices.0.iter().position(|d| *d == s.mic).unwrap_or(0);
            s.mic = devices.0.get((i + 1) % devices.0.len().max(1)).cloned().unwrap_or_default();
        }
        VoiceAction::MicVolume(d) => s.mic_volume = (s.mic_volume + d).clamp(0.0, 3.0),
        VoiceAction::Volume(d) => s.volume = (s.volume + d).clamp(0.0, 2.0),
    }
    s.save();
}

/// Waiting for a key to bind: the next key or mouse button press takes it (Esc cancels).
pub(super) fn rebind(mut s: ResMut<VoiceSettings>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, mut st: ResMut<crate::menu::MenuDirty>) {
    let Some(ch) = s.capture else { return };
    let pressed = keys.get_just_pressed().find_map(|k| KEYS.contains(k).then_some(Bind::Key(*k))).or_else(|| mouse.get_just_pressed().find_map(|m| MICE.contains(m).then_some(Bind::Mouse(*m))));
    if keys.just_pressed(KeyCode::Escape) {
        s.capture = None;
        st.0 = true;
        return;
    }
    let Some(b) = pressed else { return };
    match ch {
        VoiceChannel::Team => s.team_key = b,
        VoiceChannel::Proximity => s.near_key = b,
    }
    s.capture = None;
    s.save();
    st.0 = true;
}

#[derive(Component)]
pub(super) struct MicMeter;

/// On the voice settings page the mic is live, so you can see it working.
pub(super) fn mic_test(mut audio: NonSendMut<Audio>, settings: Res<VoiceSettings>, mut level: ResMut<MicLevel>, mut meter: Query<&mut Node, With<MicMeter>>) {
    audio.follow(&settings);
    let on = !meter.is_empty() && settings.on;
    if let Some(mic) = &mut audio.mic {
        mic.set_running(on);
    }
    let (_, peak) = audio.captured(&settings);
    level.0 = if on { level.0 * 0.85 + peak * 0.15 } else { 0.0 };
    for mut n in &mut meter {
        n.width = percent((level.0 * 140.0).min(100.0));
    }
}

/// The voice settings page (menu → Voice).
pub fn page(p: &mut ChildSpawnerCommands, s: &VoiceSettings) {
    p.spawn(text("Voice", 30.0, TEXT));
    p.spawn((text("Team voice: your team hears you anywhere. Proximity voice: everyone near you hears you, enemies too.", 16.0, MUTED), Node { margin: UiRect::vertical(px(8)), ..default() }));
    let a = |v: VoiceAction| UiAction::Voice(v);
    button(p, if s.on { "Voice chat: on" } else { "Voice chat: off" }, a(VoiceAction::Toggle), s.on);
    let key = |ch: VoiceChannel, b: Bind| if s.capture == Some(ch) { "press a key or mouse button... (Esc cancels)".to_string() } else { b.label() };
    button(p, &format!("Team voice key: {}", key(VoiceChannel::Team, s.team_key)), a(VoiceAction::Rebind(VoiceChannel::Team)), s.capture == Some(VoiceChannel::Team));
    button(p, &format!("Proximity voice key: {}", key(VoiceChannel::Proximity, s.near_key)), a(VoiceAction::Rebind(VoiceChannel::Proximity)), s.capture == Some(VoiceChannel::Proximity));
    let mic = if s.mic.is_empty() { "system default" } else { s.mic.as_str() };
    button(p, &format!("Microphone: {mic}"), a(VoiceAction::NextMic), false);
    crate::menu::setting_row(p, &format!("Mic volume: {:.0}%", s.mic_volume * 100.0), a(VoiceAction::MicVolume(-0.1)), a(VoiceAction::MicVolume(0.1)));
    crate::menu::setting_row(p, &format!("Voice volume: {:.0}%", s.volume * 100.0), a(VoiceAction::Volume(-0.1)), a(VoiceAction::Volume(0.1)));
    p.spawn((text("Mic test (talk now):", 16.0, MUTED), Node { margin: UiRect::top(px(10)), ..default() }));
    p.spawn((Node { width: px(320), height: px(10), border_radius: BorderRadius::all(px(5)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))).with_children(|b| {
        b.spawn((MicMeter, Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(5)), ..default() }, BackgroundColor(Color::srgb(0.45, 0.95, 0.45))));
    });
}
