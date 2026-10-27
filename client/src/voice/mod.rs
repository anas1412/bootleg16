//! Voice chat: two push-to-talk keys (bound in `binds`). Team voice reaches your team anywhere; proximity voice
//! reaches everyone alive near you (enemies too), fading with distance and panned to where
//! you are. The sound card and codec side is `client_core::voice`; this is the keys, the
//! settings page and the HUD rows.
mod page;
mod rows;

pub use page::{apply, page, VoiceAction};

use crate::binds::{Action, Binds};
use crate::game::{FlyCam, Frame, Walker};
pub use rows::{MicIcon, Talking};
use crate::menu::{menu_has_keys, AppState, Overlay};
use bevy::prelude::*;
use client_core::voice::Voice;
use net::protocol::VoiceChannel;

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoiceSettings(client_core::voice::VoiceSettings::load()))
            .insert_resource(Devices(client_core::voice::input_devices()))
            .init_resource::<MicLevel>()
            .add_systems(Startup, open_audio)
            .add_systems(OnEnter(AppState::InGame), (rows::spawn, |mut s: ResMut<VoiceSettings>| s.new_match()))
            .add_systems(Update, page::mic_test.run_if(in_state(AppState::Menu)))
            .add_systems(Update, (talk, listen).chain().in_set(Frame::Sound))
            .add_systems(Update, rows::rows.in_set(Frame::Ui));
    }
}

/// Which channel you're talking on: a push-to-talk key held (`binds`), voice on, no menu open.
pub(super) fn pushed(s: &VoiceSettings, binds: &Binds, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, overlay: Option<Res<Overlay>>) -> Option<VoiceChannel> {
    if !s.on || menu_has_keys(overlay) {
        None
    } else if binds.held(Action::TeamVoice, keys, mouse) {
        Some(VoiceChannel::Team)
    } else if binds.held(Action::NearVoice, keys, mouse) {
        Some(VoiceChannel::Proximity)
    } else {
        None
    }
}

/// Voice settings (`client_core::voice::VoiceSettings`, saved in ~/.config/bootleg16/voice.txt).
#[derive(Resource, Clone, Deref, DerefMut)]
pub struct VoiceSettings(pub client_core::voice::VoiceSettings);

/// Microphones found at startup.
#[derive(Resource)]
pub struct Devices(pub(super) Vec<String>);

/// Loudness of the microphone (0..1), for the meter on the voice settings page.
#[derive(Resource, Default)]
pub struct MicLevel(pub(super) f32);

/// Hand settings changes to the sound card side.
fn sync(voice: &mut Voice, s: &Res<VoiceSettings>) {
    if s.is_changed() {
        *voice.settings_mut() = s.0.clone();
    }
}

fn open_audio(world: &mut World) {
    let voice = Voice::new(world.resource::<VoiceSettings>().0.clone());
    if !voice.has_mic() {
        warn!("voice: no microphone");
    }
    world.insert_non_send(voice);
}

/// While a push-to-talk key is held, or always with open mic (team voice), and no menu is open:
/// send what the mic hears, once it's louder than the noise gate.
#[allow(clippy::too_many_arguments)]
fn talk(
    mut voice: NonSendMut<Voice>,
    settings: Res<VoiceSettings>,
    binds: Res<Binds>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Option<Res<Overlay>>,
    mut w: ResMut<Walker>,
    mut level: ResMut<MicLevel>,
) {
    sync(&mut voice, &settings);
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    let open = voice.open_mic() && !auto && overlay.as_ref().is_none_or(|o| !o.takes_keys());
    let channel = if auto { None } else { pushed(&settings, &binds, &keys, &mouse, overlay) }.or(open.then_some(VoiceChannel::Team));
    if let Some(ch) = channel {
        voice.push(ch);
    }
    for (ch, data) in voice.take_outgoing() {
        w.client.send_voice(ch, data);
    }
    level.0 = voice.mic_level();
}

/// Play what others say: team voice in both ears; proximity voice at the server's loudness,
/// panned towards the speaker when you can see them.
fn listen(mut voice: NonSendMut<Voice>, settings: Res<VoiceSettings>, mut w: ResMut<Walker>, cam: Single<&GlobalTransform, With<FlyCam>>, mut talking: ResMut<Talking>, time: Res<Time>) {
    sync(&mut voice, &settings);
    let frames = w.client.take_voice();
    let (right, ear) = (cam.right().as_vec3().to_array(), cam.translation().to_array());
    let others = w.client.others();
    for heard in frames {
        let (from, channel) = (heard.from, heard.channel);
        let name = w.client.scores.iter().find(|s| s.id == from).map(|s| s.name.as_str()).unwrap_or("");
        let at = others.iter().find(|o| o.id == from).map(|o| crate::map::to_y_up(o.origin));
        let pan = client_core::voice::pan(channel, ear, right, at);
        if let Some(peak) = voice.play(heard, name, pan) {
            talking.0.insert(from, (channel, time.elapsed_secs(), peak));
        }
    }
}
