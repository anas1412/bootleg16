//! Room reverb, automatic (CS 1.6 only has it where a map places `env_sound`, and none of our
//! maps do; Source and CS2 work the room out themselves, so do we): a few times a second rays
//! from your ears measure the space around you (how much of it is open sky, how far the walls
//! are), and every sound you hear, voice chat included, gets that room's echo. Open air stays
//! dry, a corridor short and close, a big hall longer.
use crate::game::{FlyCam, Walker};
use bevy::audio::{AddAudioSource, AudioSource, Decodable, Source};
use bevy::prelude::*;
use std::sync::Mutex;
use std::time::Duration;

/// How the room sounds: `wet` how loud the echo is, `size` how far apart its reflections
/// (0.6 small .. 1.2 large), `feedback` how long it rings.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Params {
    pub wet: f32,
    pub size: f32,
    pub feedback: f32,
}

/// The room you're in now (read by the game sounds and by the voice mixer's thread).
static ROOM: Mutex<Params> = Mutex::new(Params { wet: 0.0, size: 1.0, feedback: 0.0 });

pub fn current() -> Params {
    ROOM.lock().map(|p| *p).unwrap_or_default()
}

/// Freeverb's comb and all-pass delays (samples at 44.1 kHz) and how much the combs damp.
const COMBS: [usize; 4] = [1116, 1188, 1277, 1356];
const ALLPASSES: [usize; 2] = [556, 441];
const DAMP: f32 = 0.35;

/// A mono reverb: feed it one sample at a time, it gives the echo back.
pub struct Reverb {
    combs: Vec<(Vec<f32>, usize, f32)>,
    allpasses: Vec<(Vec<f32>, usize)>,
    pub params: Params,
}

impl Reverb {
    pub fn new(rate: u32, params: Params) -> Reverb {
        let scale = rate as f32 / 44_100.0 * params.size.clamp(0.3, 1.5);
        let len = |d: usize| ((d as f32 * scale) as usize).max(1);
        Reverb { combs: COMBS.iter().map(|&d| (vec![0.0; len(d)], 0, 0.0)).collect(), allpasses: ALLPASSES.iter().map(|&d| (vec![0.0; len(d)], 0)).collect(), params }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let fb = self.params.feedback;
        let mut out = 0.0;
        for (buf, i, low) in &mut self.combs {
            let y = buf[*i];
            *low = y * (1.0 - DAMP) + *low * DAMP;
            buf[*i] = input * 0.015 + *low * fb;
            *i = (*i + 1) % buf.len();
            out += y;
        }
        for (buf, i) in &mut self.allpasses {
            let b = buf[*i];
            buf[*i] = out + b * 0.5;
            out = b - out;
            *i = (*i + 1) % buf.len();
        }
        out * self.params.wet * 4.0
    }

    /// Seconds until the echo has died down (-60 dB).
    pub fn tail_secs(&self, rate: u32) -> f32 {
        let fb = self.params.feedback.clamp(0.01, 0.98);
        let longest = self.combs.iter().map(|c| c.0.len()).max().unwrap_or(1) as f32;
        (0.001f32.ln() / fb.ln()) * longest / rate as f32
    }
}

/// A sound played through the room's reverb (as it was when the sound started).
#[derive(Asset, TypePath, Clone)]
pub struct Reverbed {
    pub source: AudioSource,
    pub params: Params,
}

impl Decodable for Reverbed {
    type Decoder = Wet<<AudioSource as Decodable>::Decoder>;

    fn decoder(&self) -> Self::Decoder {
        let inner = self.source.decoder();
        let (rate, channels) = (inner.sample_rate().get(), inner.channels().get() as usize);
        let reverb = Reverb::new(rate, self.params);
        let tail = (reverb.tail_secs(rate) * rate as f32) as usize;
        Wet { inner: Some(inner), reverb, channels, rate, frame: vec![0.0; channels], at: channels, tail }
    }
}

/// The dry sound with the echo mixed into every channel, then the echo's tail on its own.
pub struct Wet<I> {
    inner: Option<I>,
    reverb: Reverb,
    channels: usize,
    rate: u32,
    frame: Vec<f32>,
    at: usize,
    tail: usize,
}

impl<I: Source> Iterator for Wet<I> {
    type Item = bevy::audio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at == self.channels {
            let mut got = 0;
            if let Some(inner) = &mut self.inner {
                for s in self.frame.iter_mut() {
                    match inner.next() {
                        Some(x) => {
                            *s = x;
                            got += 1;
                        }
                        None => *s = 0.0,
                    }
                }
            }
            if got == 0 {
                self.inner = None;
                if self.tail == 0 {
                    return None;
                }
                self.tail -= 1;
                self.frame.fill(0.0);
            }
            let mono = self.frame.iter().sum::<f32>() / self.channels as f32;
            let wet = self.reverb.process(mono);
            self.frame.iter_mut().for_each(|s| *s += wet);
            self.at = 0;
        }
        self.at += 1;
        Some(self.frame[self.at - 1])
    }
}

impl<I: Source> Source for Wet<I> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> bevy::audio::ChannelCount {
        std::num::NonZero::new(self.channels as u16).unwrap_or(std::num::NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> bevy::audio::SampleRate {
        std::num::NonZero::new(self.rate).unwrap_or(std::num::NonZero::<u32>::MIN)
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// What the rays found: the share of them that reached open sky (or nothing), and how far
/// the rest went.
fn room_of(open: f32, distance: f32) -> Params {
    let enclosed = ((1.0 - open - 0.3) / 0.6).clamp(0.0, 1.0);
    let s = ((distance - 128.0) / 896.0).clamp(0.0, 1.0);
    Params { wet: enclosed * (0.16 + 0.12 * s), size: 0.6 + 0.6 * s, feedback: 0.72 + 0.18 * s }
}

/// Measures the room around the camera every 0.2 s and eases the reverb towards it.
fn probe(time: Res<Time>, w: Res<Walker>, cam: Single<&Transform, With<FlyCam>>, mut target: Local<Params>, mut next: Local<f32>) {
    let now = time.elapsed_secs();
    if now >= *next {
        *next = now + 0.2;
        let t = cam.translation;
        let ear = [t.x, -t.z, t.y];
        let mut dirs: Vec<[f32; 3]> = vec![[0.0, 0.0, 1.0]];
        for k in 0..8 {
            let (s, c) = (k as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
            dirs.push([c, s, 0.0]);
            dirs.push([c * 0.7, s * 0.7, 0.7]);
        }
        const REACH: f32 = 2048.0;
        let (mut open, mut far, mut walls) = (0, 0.0, 0);
        for d in &dirs {
            let hit = w.client.world.trace_line(ear, std::array::from_fn(|i| ear[i] + d[i] * REACH));
            if hit.fraction >= 1.0 || hit.hit_sky {
                open += 1;
            } else {
                far += hit.fraction * REACH;
                walls += 1;
            }
        }
        *target = room_of(open as f32 / dirs.len() as f32, if walls > 0 { far / walls as f32 } else { REACH });
    }
    if let Ok(mut p) = ROOM.lock() {
        let k = (time.delta_secs() * 2.0).min(1.0);
        p.wet += (target.wet - p.wet) * k;
        p.size += (target.size - p.size) * k;
        p.feedback += (target.feedback - p.feedback) * k;
    }
}

pub struct ReverbPlugin;

impl Plugin for ReverbPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Reverbed>().add_systems(Update, probe.run_if(resource_exists::<Walker>).run_if(in_state(crate::menu::AppState::InGame)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sky_is_dry_and_a_hall_rings_longer_than_a_corridor() {
        assert_eq!(room_of(0.8, 300.0).wet, 0.0);
        let (corridor, hall) = (room_of(0.0, 150.0), room_of(0.0, 900.0));
        assert!(corridor.wet > 0.05 && hall.wet > corridor.wet);
        let rate = 48_000;
        assert!(Reverb::new(rate, hall).tail_secs(rate) > Reverb::new(rate, corridor).tail_secs(rate));
        // the echo of a click dies away instead of building up
        let mut r = Reverb::new(rate, hall);
        r.process(1.0);
        let late: f32 = (0..rate as usize * 4).map(|_| r.process(0.0).abs()).skip(rate as usize * 3).fold(0.0, f32::max);
        assert!(late < 1e-3, "{late}");
    }
}
