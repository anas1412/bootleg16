//! Room reverb, automatic (CS 1.6 only has it where a map places `env_sound`, and none of our
//! maps do; Source and CS2 work the room out themselves, so do we): a few times a second rays
//! from your ears measure the space around you (how much of it is open sky, how far the walls
//! are), and every sound you hear, voice chat included, gets that room's echo. Open air stays
//! dry, a corridor short and close, a big hall longer.
use crate::game::{FlyCam, Walker};
use bevy::audio::{AddAudioSource, AudioSource, Decodable, Source};
use bevy::prelude::*;
use std::time::Duration;

pub use client_core::reverb::{current, Params, Reverb};

/// A sound played through the room's reverb (as it was when the sound started), and placed
/// left or right GoldSrc's way: `pan` is each ear's gain ([left, right]; None: as recorded).
#[derive(Asset, TypePath, Clone)]
pub struct Reverbed {
    pub source: AudioSource,
    pub params: Params,
    pub pan: Option<[f32; 2]>,
    /// Ear gains that change while it plays (a looping map sound as you move): f32 bits,
    /// written by the game, read by the sound thread. Wins over `pan`.
    pub live: Option<LivePan>,
}

/// Each ear's gain, live (`Reverbed::live`).
pub type LivePan = std::sync::Arc<[std::sync::atomic::AtomicU32; 2]>;

pub fn live_pan() -> LivePan {
    std::sync::Arc::new([std::sync::atomic::AtomicU32::new(0), std::sync::atomic::AtomicU32::new(0)])
}

pub fn set_live(l: &LivePan, gains: [f32; 2]) {
    for (a, g) in l.iter().zip(gains) {
        a.store(g.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

impl Decodable for Reverbed {
    type Decoder = Wet<<AudioSource as Decodable>::Decoder>;

    fn decoder(&self) -> Self::Decoder {
        let inner = self.source.decoder();
        let (rate, channels) = (inner.sample_rate().get(), inner.channels().get() as usize);
        let reverb = Reverb::new(rate, self.params);
        // (no echo, no tail: a loop must not gain a gap)
        let tail = if self.params.wet > 0.01 { (reverb.tail_secs(rate) * rate as f32) as usize } else { 0 };
        let out = if self.pan.is_some() || self.live.is_some() { 2 } else { channels };
        Wet { inner: Some(inner), reverb, channels, rate, input: vec![0.0; channels], frame: vec![0.0; out], at: out, tail, pan: self.pan, live: self.live.clone() }
    }
}

/// The dry sound with the echo mixed into every channel, then the echo's tail on its own.
pub struct Wet<I> {
    inner: Option<I>,
    reverb: Reverb,
    /// Channels in the recording; `frame` is what goes out (2 when panned).
    channels: usize,
    rate: u32,
    input: Vec<f32>,
    frame: Vec<f32>,
    at: usize,
    tail: usize,
    pan: Option<[f32; 2]>,
    live: Option<LivePan>,
}

impl<I: Source> Iterator for Wet<I> {
    type Item = bevy::audio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at == self.frame.len() {
            let mut got = 0;
            if let Some(inner) = &mut self.inner {
                for s in self.input.iter_mut() {
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
                self.input.fill(0.0);
            }
            let mono = self.input.iter().sum::<f32>() / self.channels as f32;
            let wet = if self.reverb.params.wet > 0.01 { self.reverb.process(mono) } else { 0.0 };
            let pan = self.live.as_ref().map(|l| l.each_ref().map(|a| f32::from_bits(a.load(std::sync::atomic::Ordering::Relaxed)))).or(self.pan);
            match pan {
                // the room's echo comes from all round: both ears, at the sound's loudness
                Some([l, r]) => {
                    let echo = wet * (l + r) * 0.5;
                    self.frame[0] = mono * l + echo;
                    self.frame[1] = mono * r + echo;
                }
                None => {
                    for (o, i) in self.frame.iter_mut().zip(&self.input) {
                        *o = i + wet;
                    }
                }
            }
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
        std::num::NonZero::new(self.frame.len() as u16).unwrap_or(std::num::NonZero::<u16>::MIN)
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
    let mut p = current();
    let k = (time.delta_secs() * 2.0).min(1.0);
    p.wet += (target.wet - p.wet) * k;
    p.size += (target.size - p.size) * k;
    p.feedback += (target.feedback - p.feedback) * k;
    client_core::reverb::set(p);
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
