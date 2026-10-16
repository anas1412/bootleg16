//! Voice chat: two push-to-talk keys (bound in `binds`). Team voice reaches your team anywhere; proximity voice
//! reaches everyone alive near you (enemies too), fading with distance and panned to where
//! you are. Opus over the game's UDP connection; the server relays (net/src/server/voice.rs).
mod audio;
mod page;
mod rows;

pub use page::{apply, page, VoiceAction};

use crate::binds::{Action, Binds};
use crate::game::{FlyCam, Frame, Walker};
pub use rows::{MicIcon, Talking};
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
    /// Open mic: talk to your team whenever you speak (the gate decides), no key needed.
    pub open_mic: bool,
    /// Noise gate: frames quieter than this peak (0..1) aren't sent (0 = off). Open mic needs it.
    pub gate: f32,
    /// Each player's voice volume by name (0 = muted; missing = 1).
    players: HashMap<String, f32>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings { on: true, mic: String::new(), mic_volume: 1.0, volume: 1.0, open_mic: false, gate: 0.03, players: HashMap::new() }
    }
}

/// After the voice drops under the gate, keep sending this many frames (300 ms) so the ends of
/// words aren't cut off.
const GATE_HOLD_FRAMES: u32 = 15;
/// Voice through a wall: this much quieter, and low-passed (one-pole, ~1.2 kHz at 48 kHz).
const MUFFLED_GAIN: f32 = 0.55;
const MUFFLE_ALPHA: f32 = 0.145;

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
                "open_mic" => s.open_mic = v == "true",
                "gate" => s.gate = v.parse().unwrap_or(s.gate),
                k if k.starts_with("player.") => {
                    if let Ok(g) = v.parse() {
                        s.players.insert(k["player.".len()..].to_string(), g);
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn save(&self) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let mut text = format!(
            "on={}\nmic={}\nmic_volume={}\nvolume={}\nopen_mic={}\ngate={}\n",
            self.on, self.mic, self.mic_volume, self.volume, self.open_mic, self.gate
        );
        for (name, g) in &self.players {
            text.push_str(&format!("player.{name}={g}\n"));
        }
        let _ = std::fs::write(p, text);
    }

    /// How loud `name`'s voice plays for you (0 = muted).
    pub fn player_gain(&self, name: &str) -> f32 {
        self.players.get(name).copied().unwrap_or(1.0)
    }

    /// Set `name`'s voice volume (0 mutes them; 1 is normal, up to 2), and remember it.
    pub fn set_player_gain(&mut self, name: &str, gain: f32) {
        let g = gain.clamp(0.0, 2.0);
        if (g - 1.0).abs() < 1e-3 {
            self.players.remove(name);
        } else {
            self.players.insert(name.to_string(), g);
        }
        self.save();
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
    /// Low-pass state per speaker (for voice through walls).
    muffle: HashMap<u8, f32>,
    /// Frames left to send after the voice fell under the gate.
    gate_hold: u32,
}

fn open_audio(world: &mut World) {
    let name = world.resource::<VoiceSettings>().mic.clone();
    let audio = Audio {
        mic: audio::Mic::open(&name),
        mic_name: name,
        speakers: audio::Speakers::open(),
        encoder: voice_encoder(),
        decoders: HashMap::new(),
        muffle: HashMap::new(),
        gate_hold: 0,
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

/// While a push-to-talk key is held, or always with open mic (team voice), and no menu is open:
/// send what the mic hears, once it's louder than the noise gate.
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
    let open = settings.on && settings.open_mic && !auto && overlay.as_ref().is_none_or(|o| **o == Overlay::None);
    let channel = if auto { None } else { pushed(&settings, &binds, &keys, &mouse, overlay) }.or(open.then_some(VoiceChannel::Team));
    if let Some(mic) = &mut audio.mic {
        mic.set_running(channel.is_some());
    }
    let (frames, peak) = audio.captured(&settings);
    level.0 = peak;
    *me_talking = channel;
    let Some(channel) = channel else { return };
    for f in frames {
        // Noise gate: send while it's louder than the gate, and a moment after.
        let loud = f.iter().fold(0.0f32, |m, x| m.max(x.abs())) >= settings.gate;
        audio.gate_hold = if loud { GATE_HOLD_FRAMES } else { audio.gate_hold.saturating_sub(1) };
        if !loud && audio.gate_hold == 0 {
            continue;
        }
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
    for net::client::Heard { from, channel, gain, muffled, data } in frames {
        let name = w.client.scores.iter().find(|s| s.id == from).map(|s| s.name.as_str()).unwrap_or("");
        let player = settings.player_gain(name);
        if player <= 0.0 {
            continue; // muted: not even decoded
        }
        let dec = audio.decoders.entry(from).or_insert_with(|| opus::Decoder::new(VOICE_RATE, opus::Channels::Mono).expect("opus decoder"));
        let mut pcm = vec![0.0f32; VOICE_FRAME * 6];
        let Ok(n) = dec.decode_float(&data, &mut pcm, false) else { continue };
        // Through a wall: duller (low-pass) and quieter. The filter keeps its state per speaker
        // so it doesn't click between frames.
        let y = audio.muffle.entry(from).or_insert(0.0);
        for x in pcm[..n].iter_mut() {
            *y += MUFFLE_ALPHA * (*x - *y);
            if muffled {
                *x = *y * MUFFLED_GAIN;
            }
        }
        let peak = pcm[..n].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        talking.0.insert(from, (channel, time.elapsed_secs(), peak));
        let vol = settings.volume * player * gain as f32 / 255.0;
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
