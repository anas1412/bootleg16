//! The voice settings page (menu -> Voice): on/off, microphone, volumes, open mic and the noise
//! gate, and a live mic meter (with the gate's line) to test with. The push-to-talk keys are in Settings · Keys.
use super::{Audio, Devices, MicLevel, VoiceSettings};
use crate::ui::{chips, Choice, UiMemory, chip, page_title, setting_row, stepper, t, vh, Kit, Type, UiAction, AMBER, BG1, OK, TX2};
use bevy::prelude::*;

/// A change made on the voice settings page.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceAction {
    Toggle,
    NextMic,
    MicVolume(f32),
    Volume(f32),
    /// Open mic on/off (talks to your team whenever the gate opens).
    OpenMic,
    /// Noise gate threshold step (0 = off).
    Gate(f32),
}

/// Apply a voice settings change (called from the menu).
pub fn apply(s: &mut VoiceSettings, devices: &Devices, a: VoiceAction) {
    match a {
        VoiceAction::Toggle => s.on = !s.on,
        VoiceAction::NextMic => {
            let i = devices.0.iter().position(|d| *d == s.mic).unwrap_or(0);
            s.mic = devices.0.get((i + 1) % devices.0.len().max(1)).cloned().unwrap_or_default();
        }
        VoiceAction::MicVolume(d) => s.mic_volume = (s.mic_volume + d).clamp(0.0, 3.0),
        VoiceAction::Volume(d) => s.volume = (s.volume + d).clamp(0.0, 2.0),
        VoiceAction::OpenMic => s.open_mic = !s.open_mic,
        VoiceAction::Gate(d) => s.gate = (s.gate + d).clamp(0.0, 0.5),
    }
    s.save();
}

#[derive(Component)]
pub(super) struct MicMeter;

/// How the mic level fills the meter (level 1/GAUGE fills it); the gate line uses the same scale.
const GAUGE: f32 = 1.4;

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
        n.width = percent((level.0 * GAUGE * 100.0).min(100.0));
    }
}

/// The voice settings page (menu → Voice).
pub fn page(p: &mut ChildSpawnerCommands, kit: &Kit, memory: &mut UiMemory, s: &VoiceSettings, tune: Option<f32>) {
    page_title(p, kit, "Team hears you anywhere, proximity reaches anyone near · keys in Settings", "Voice", "音声");
    let a = |v: VoiceAction| UiAction::Voice(v);

    let mic = if s.mic.is_empty() { "System default" } else { s.mic.as_str() };
    crate::menu_pages::rows(p, tune, |rows| {
        vec![
            setting_row(rows, kit, "Voice chat", |r| chips(r, kit, "voice.on", vec![Choice::new("On", a(VoiceAction::Toggle), s.on), Choice::new("Off", a(VoiceAction::Toggle), !s.on)])),
            setting_row(rows, kit, "Microphone", |r| chip(r, kit, mic, a(VoiceAction::NextMic), false)),
            setting_row(rows, kit, "Mic volume", |r| stepper(r, kit, memory, "voice.mic", &format!("{:.0}%", s.mic_volume * 100.0), a(VoiceAction::MicVolume(-0.1)), a(VoiceAction::MicVolume(0.1)))),
            setting_row(rows, kit, "Voice volume", |r| stepper(r, kit, memory, "voice.vol", &format!("{:.0}%", s.volume * 100.0), a(VoiceAction::Volume(-0.1)), a(VoiceAction::Volume(0.1)))),
            // open mic: your team hears you whenever you're louder than the gate (no key)
            setting_row(rows, kit, "Open mic", |r| chips(r, kit, "voice.open", vec![Choice::new("On", a(VoiceAction::OpenMic), s.open_mic), Choice::new("Off", a(VoiceAction::OpenMic), !s.open_mic)])),
            setting_row(rows, kit, "Noise gate", |r| stepper(r, kit, memory, "voice.gate", &if s.gate <= 0.0 { "Off".to_string() } else { format!("{:.0}", s.gate * 100.0) }, a(VoiceAction::Gate(-0.01)), a(VoiceAction::Gate(0.01)))),
            setting_row(rows, kit, "Mic test", |r| {
                r.spawn((Node { width: vh(320.0), height: vh(8.0), ..default() }, BackgroundColor(BG1))).with_children(|b| {
                    b.spawn((MicMeter, Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(OK)));
                    // the gate: sound under this line isn't sent
                    if s.gate > 0.0 {
                        b.spawn((Node { position_type: PositionType::Absolute, left: percent((s.gate * GAUGE * 100.0).min(100.0)), top: vh(-4.0), bottom: vh(-4.0), width: vh(2.0), ..default() }, BackgroundColor(AMBER)));
                    }
                });
                r.spawn(t(kit, Type::Label, if s.gate > 0.0 { "Talk now · louder than the line goes through" } else { "Talk now" }, TX2));
            }),
        ]
    });
}
