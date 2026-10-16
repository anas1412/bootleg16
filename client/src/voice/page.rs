//! The voice settings page (menu -> Voice): on/off, both push-to-talk keys (rebind by
//! pressing one), microphone, volumes, and a live mic meter to test with.
use super::{Audio, Bind, Devices, MicLevel, VoiceSettings, KEYS, MICE};
use crate::ui::{chips, Choice, UiMemory, chip, page_title, setting_row, stepper, t, vh, Kit, Type, UiAction, BG1, OK, TX2};
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
pub fn page(p: &mut ChildSpawnerCommands, kit: &Kit, memory: &mut UiMemory, s: &VoiceSettings, tune: Option<f32>) {
    page_title(p, kit, "Team hears you anywhere, proximity reaches anyone near", "Voice", "音声");
    let a = |v: VoiceAction| UiAction::Voice(v);
    let key = |ch: VoiceChannel, b: Bind| if s.capture == Some(ch) { "Press a key (Esc cancels)".to_string() } else { b.label() };
    let mic = if s.mic.is_empty() { "System default" } else { s.mic.as_str() };
    crate::menu_pages::rows(p, tune, |rows| {
        vec![
            setting_row(rows, kit, "Voice chat", |r| chips(r, kit, "voice.on", vec![Choice::new("On", a(VoiceAction::Toggle), s.on), Choice::new("Off", a(VoiceAction::Toggle), !s.on)])),
            setting_row(rows, kit, "Team voice key", |r| chip(r, kit, &key(VoiceChannel::Team, s.team_key), a(VoiceAction::Rebind(VoiceChannel::Team)), s.capture == Some(VoiceChannel::Team))),
            setting_row(rows, kit, "Proximity voice key", |r| chip(r, kit, &key(VoiceChannel::Proximity, s.near_key), a(VoiceAction::Rebind(VoiceChannel::Proximity)), s.capture == Some(VoiceChannel::Proximity))),
            setting_row(rows, kit, "Microphone", |r| chip(r, kit, mic, a(VoiceAction::NextMic), false)),
            setting_row(rows, kit, "Mic volume", |r| stepper(r, kit, memory, "voice.mic", &format!("{:.0}%", s.mic_volume * 100.0), a(VoiceAction::MicVolume(-0.1)), a(VoiceAction::MicVolume(0.1)))),
            setting_row(rows, kit, "Voice volume", |r| stepper(r, kit, memory, "voice.vol", &format!("{:.0}%", s.volume * 100.0), a(VoiceAction::Volume(-0.1)), a(VoiceAction::Volume(0.1)))),
            setting_row(rows, kit, "Mic test", |r| {
                r.spawn((Node { width: vh(320.0), height: vh(8.0), ..default() }, BackgroundColor(BG1))).with_children(|b| {
                    b.spawn((MicMeter, Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(OK)));
                });
                r.spawn(t(kit, Type::Label, "Talk now", TX2));
            }),
        ]
    });
}
