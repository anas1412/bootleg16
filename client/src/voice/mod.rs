//! Voice chat: two push-to-talk keys (bound in `binds`). Team voice reaches your team anywhere; proximity voice
//! reaches everyone alive near you (enemies too), fading with distance and panned to where
//! you are. Opus over the game's UDP connection; the server relays (net/src/server/voice.rs).
mod audio;
mod page;
mod rows;

pub use page::{apply, page, VoiceAction};

use crate::binds::{Action, Binds};
use crate::game::{FlyCam, Frame, Walker};
use rows::Talking;
use crate::menu::{overlay_open, AppState, Overlay};
use bevy::prelude::*;
use net::protocol::{VoiceChannel, VOICE_FRAME, VOICE_RATE};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoiceSettings::load())
            .insert_resource(Devices(audio::input_devices()))
            .init_resource::<MicLevel>()
            .add_systems(Startup, open_audio)
            .add_systems(OnEnter(AppState::InGame), rows::spawn)
            .add_systems(Update, page::mic_test.run_if(in_state(AppState::Menu)))
            .add_systems(Update, (talk, listen).chain().in_set(Frame::Sound))
            .add_systems(Update, rows::rows.in_set(Frame::Ui));
    }
}

/// Which channel you're talking on: a push-to-talk key held (`binds`), voice on, no menu open.
pub(super) fn pushed(s: &VoiceSettings, binds: &Binds, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, overlay: Option<Res<Overlay>>) -> Option<VoiceChannel> {
    if !s.on || overlay_open(overlay) {
        None
    } else if binds.held(Action::TeamVoice, keys, mouse) {
        Some(VoiceChannel::Team)
    } else if binds.held(Action::NearVoice, keys, mouse) {
        Some(VoiceChannel::Proximity)
    } else {
        None
    }
}

/// Voice settings, saved in ~/.config/bootleg16/voice.txt.
#[derive(Resource, Clone)]
pub struct VoiceSettings {
    pub on: bool,
    /// Microphone by name ("" = system default).
    pub mic: String,
    pub mic_volume: f32,
    /// Everyone else's voice.
    pub volume: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings { on: true, mic: String::new(), mic_volume: 1.0, volume: 1.0 }
    }
}

fn path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16/voice.txt")
}

impl VoiceSettings {
    fn load() -> VoiceSettings {
        let mut s = VoiceSettings::default();
        for line in std::fs::read_to_string(path()).unwrap_or_default().lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "on" => s.on = v == "true",
                "mic" => s.mic = v.to_string(),
                "mic_volume" => s.mic_volume = v.parse().unwrap_or(s.mic_volume),
                "volume" => s.volume = v.parse().unwrap_or(s.volume),
                _ => {}
            }
        }
        s
    }

    fn save(&self) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let text = format!(
            "on={}\nmic={}\nmic_volume={}\nvolume={}\n",
            self.on,
            self.mic,
            self.mic_volume,
            self.volume
        );
        let _ = std::fs::write(p, text);
    }
}

/// Microphones found at startup.
#[derive(Resource)]
pub struct Devices(pub(super) Vec<String>);

/// Loudness of the microphone (0..1), for the meter on the voice settings page.
#[derive(Resource, Default)]
pub struct MicLevel(pub(super) f32);

/// The sound card side (cpal streams aren't Send, so these live on the main thread).
pub(super) struct Audio {
    pub(super) mic: Option<audio::Mic>,
    mic_name: String,
    speakers: Option<audio::Speakers>,
    encoder: Option<opus::Encoder>,
    decoders: HashMap<u8, opus::Decoder>,
}

fn open_audio(world: &mut World) {
    let name = world.resource::<VoiceSettings>().mic.clone();
    let audio = Audio {
        mic: audio::Mic::open(&name),
        mic_name: name,
        speakers: audio::Speakers::open(),
        encoder: voice_encoder(),
        decoders: HashMap::new(),
    };
    if audio.mic.is_none() {
        warn!("voice: no microphone");
    }
    world.insert_non_send(audio);
}

/// Opus for speech at 24 kbit/s (~60 bytes per 20 ms frame; the default would be ~3x that).
fn voice_encoder() -> Option<opus::Encoder> {
    let mut e = opus::Encoder::new(VOICE_RATE, opus::Channels::Mono, opus::Application::Voip).ok()?;
    e.set_bitrate(opus::Bitrate::Bits(24_000)).ok()?;
    Some(e)
}

impl Audio {
    /// Switch microphones when the setting changed.
    pub(super) fn follow(&mut self, s: &VoiceSettings) {
        if self.mic_name != s.mic {
            self.mic = audio::Mic::open(&s.mic);
            self.mic_name = s.mic.clone();
        }
    }

    /// Captured frames with the mic volume applied, and their loudness.
    pub(super) fn captured(&mut self, s: &VoiceSettings) -> (Vec<Vec<f32>>, f32) {
        let Some(mic) = &self.mic else { return (Vec::new(), 0.0) };
        let mut frames = mic.frames(VOICE_FRAME);
        let mut peak = 0.0f32;
        for f in &mut frames {
            for x in f.iter_mut() {
                *x = (*x * s.mic_volume).clamp(-1.0, 1.0);
                peak = peak.max(x.abs());
            }
        }
        (frames, peak)
    }
}

/// While a push-to-talk key is held (and no menu is open), send what the mic hears.
#[allow(clippy::too_many_arguments)]
fn talk(
    mut audio: NonSendMut<Audio>,
    settings: Res<VoiceSettings>,
    binds: Res<Binds>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Option<Res<Overlay>>,
    mut w: ResMut<Walker>,
    mut level: ResMut<MicLevel>,
    mut me_talking: Local<Option<VoiceChannel>>,
) {
    audio.follow(&settings);
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    let channel = if auto { None } else { pushed(&settings, &binds, &keys, &mouse, overlay) };
    *me_talking = channel;
    if let Some(mic) = &mut audio.mic {
        mic.set_running(channel.is_some());
    }
    let (frames, peak) = audio.captured(&settings);
    level.0 = peak;
    let Some(channel) = channel else { return };
    for f in frames {
        let mut out = [0u8; 400];
        if let Some(n) = audio.encoder.as_mut().and_then(|e| e.encode_float(&f, &mut out).ok()) {
            w.client.send_voice(channel, out[..n].to_vec());
        }
    }
}

/// Play what others say: team voice in both ears; proximity voice at the server's loudness,
/// panned towards the speaker when you can see them.
fn listen(
    mut audio: NonSendMut<Audio>,
    settings: Res<VoiceSettings>,
    mut w: ResMut<Walker>,
    cam: Single<&GlobalTransform, With<FlyCam>>,
    mut talking: ResMut<Talking>,
    time: Res<Time>,
) {
    let frames = w.client.take_voice();
    if !settings.on || frames.is_empty() {
        return;
    }
    let right = cam.right().as_vec3();
    let ear = cam.translation();
    let others = w.client.others();
    for (from, channel, gain, data) in frames {
        let dec = audio.decoders.entry(from).or_insert_with(|| opus::Decoder::new(VOICE_RATE, opus::Channels::Mono).expect("opus decoder"));
        let mut pcm = vec![0.0f32; VOICE_FRAME * 6];
        let Ok(n) = dec.decode_float(&data, &mut pcm, false) else { continue };
        let peak = pcm[..n].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        talking.0.insert(from, (channel, time.elapsed_secs(), peak));
        let vol = settings.volume * gain as f32 / 255.0;
        // Pan: -1 left .. 1 right, only for proximity voice from someone in view.
        let pan = match (channel, others.iter().find(|o| o.id == from)) {
            (VoiceChannel::Proximity, Some(o)) => {
                let at = Vec3::from(crate::map::to_y_up(o.origin));
                (at - ear).normalize_or_zero().dot(right).clamp(-1.0, 1.0) * 0.8
            }
            _ => 0.0,
        };
        if let Some(sp) = &audio.speakers {
            sp.push(from, &pcm[..n], vol * (1.0 - pan.max(0.0)), vol * (1.0 + pan.min(0.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_round_trip_keeps_the_voice() {
        let mut enc = voice_encoder().unwrap();
        let mut dec = opus::Decoder::new(VOICE_RATE, opus::Channels::Mono).unwrap();
        // 440 Hz tone, a few frames (Opus needs a moment to settle).
        let mut energy = (0.0, 0.0);
        for f in 0..10 {
            let pcm: Vec<f32> = (0..VOICE_FRAME).map(|i| ((f * VOICE_FRAME + i) as f32 * 440.0 * std::f32::consts::TAU / VOICE_RATE as f32).sin() * 0.5).collect();
            let mut packet = [0u8; 400];
            let n = enc.encode_float(&pcm, &mut packet).unwrap();
            assert!(n < 100, "{n} bytes for 20 ms");
            let mut out = vec![0.0f32; VOICE_FRAME];
            let m = dec.decode_float(&packet[..n], &mut out, false).unwrap();
            assert_eq!(m, VOICE_FRAME);
            if f > 3 {
                energy.0 += pcm.iter().map(|x| x * x).sum::<f32>();
                energy.1 += out.iter().map(|x| x * x).sum::<f32>();
            }
        }
        assert!((energy.1 / energy.0 - 1.0).abs() < 0.3, "{energy:?}");
    }

}
