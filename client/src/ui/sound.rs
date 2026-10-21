//! UI sounds, made here instead of loaded (design/mockups/ui ai.js `tone`): short soft
//! triangle blips, quiet so footsteps stay clear. Send a `UiSound`; it plays at the game's
//! volume.
use crate::menu::Settings;
use bevy::audio::{PlaybackSettings, Volume};
use bevy::prelude::*;
use std::collections::HashMap;

/// A UI sound to play.
#[derive(Message, Clone, Copy, PartialEq)]
pub enum UiSound {
    /// The assistant's channel opening (rises), the Twins handing over (one blip), closing
    /// (falls); each assistant has its own pitch (Hz).
    Open(f32),
    Hand(f32),
    Close(f32),
    /// The round timer's last seconds.
    Tick,
    /// A match found: three rising notes.
    Found,
    /// Round events, where CS 1.6 plays a radio call; ours are short cues, no voice (and the
    /// assistant's line). Freeze time over, the round is live (CS: "Go go go").
    Go,
    /// The bomb is planted (CS: "The bomb has been planted"): high, low, high.
    Planted,
    /// The bomb is defused (CS: "Bomb has been defused"): two bright rising notes.
    Defused,
    /// The round is won or lost (CS: "Terrorists win" / "Counter-terrorists win"): a rising
    /// or a falling three notes.
    Won,
    Lost,
    /// A status icon appearing (buy zone, kit) or going.
    On,
    Off,
}

impl UiSound {
    /// (tones in Hz one after another, loudness)
    fn tones(self) -> (Vec<f32>, f32) {
        match self {
            UiSound::Open(f) => (vec![f, f * 1.5], 0.35),
            UiSound::Hand(f) => (vec![f * 1.5], 0.35),
            UiSound::Close(f) => (vec![f * 1.25, f * 0.84], 0.3),
            UiSound::Tick => (vec![1320.0], 0.2),
            UiSound::Found => (vec![660.0, 880.0, 1320.0], 0.5),
            UiSound::Go => (vec![587.0, 880.0], 0.5),
            UiSound::Planted => (vec![988.0, 740.0, 988.0], 0.5),
            UiSound::Defused => (vec![784.0, 1175.0], 0.5),
            UiSound::Won => (vec![523.0, 659.0, 784.0], 0.5),
            UiSound::Lost => (vec![523.0, 440.0, 349.0], 0.45),
            UiSound::On => (vec![880.0], 0.18),
            UiSound::Off => (vec![660.0], 0.18),
        }
    }
}

const RATE: u32 = 22050;
/// Each blip: 8 ms attack, then a fast fall over 90 ms; the next starts 60 ms later.
const GAP: f32 = 0.06;
const LEN: f32 = 0.09;

/// Blips as a 16-bit mono WAV file.
fn wav(tones: &[f32]) -> Vec<u8> {
    let total = GAP * (tones.len() - 1) as f32 + LEN + 0.02;
    let n = (total * RATE as f32) as usize;
    let mut pcm = vec![0f32; n];
    for (k, f) in tones.iter().enumerate() {
        let start = (k as f32 * GAP * RATE as f32) as usize;
        for i in 0..((LEN * RATE as f32) as usize) {
            let t = i as f32 / RATE as f32;
            let env = if t < 0.008 { t / 0.008 } else { (-(t - 0.008) / 0.018).exp() };
            // triangle wave
            let ph = (t * f).fract();
            let tri = 4.0 * (ph - 0.5).abs() - 1.0;
            if let Some(s) = pcm.get_mut(start + i) {
                *s += tri * env;
            }
        }
    }
    let data: Vec<u8> = pcm.iter().flat_map(|s| (((s * 0.5).clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes()).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    let u32le = |v: u32| v.to_le_bytes();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32le(36 + data.len() as u32));
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&u32le(16));
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&u32le(RATE));
    out.extend_from_slice(&u32le(RATE * 2));
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&u32le(data.len() as u32));
    out.extend_from_slice(&data);
    out
}

#[derive(Resource, Default)]
struct Made(HashMap<Vec<u32>, Handle<AudioSource>>);

fn play(mut commands: Commands, mut msgs: MessageReader<UiSound>, mut made: ResMut<Made>, mut assets: ResMut<Assets<AudioSource>>, settings: Option<Res<Settings>>) {
    let volume = settings.map(|s| s.volume).unwrap_or(1.0);
    for s in msgs.read() {
        let (tones, loud) = s.tones();
        let key: Vec<u32> = tones.iter().map(|f| f.round() as u32).collect();
        let handle = made.0.entry(key).or_insert_with(|| assets.add(AudioSource { bytes: wav(&tones).into() })).clone();
        commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume * loud))));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<UiSound>().init_resource::<Made>().add_systems(Update, play);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blips_are_a_valid_wav() {
        let w = wav(&[660.0, 990.0]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        let data = u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize;
        assert_eq!(w.len(), 44 + data);
        // 2 blips: 60 ms + 90 ms + 20 ms of tail
        assert_eq!(data / 2, ((GAP + LEN + 0.02) * RATE as f32) as usize);
    }
}
