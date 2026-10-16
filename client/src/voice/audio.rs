//! Sound card side of voice: the microphone (captured, mixed to mono, resampled to 48 kHz) and
//! a little mixer that plays everyone talking, each with its own volume and left/right pan.
//! Both run on cpal's audio threads; the game talks to them through shared buffers.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use net::protocol::VOICE_RATE;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// Names of the microphones on this machine ("" first = the system default).
pub fn input_devices() -> Vec<String> {
    let mut v = vec![String::new()];
    if let Ok(devs) = cpal::default_host().input_devices() {
        v.extend(devs.filter_map(|d| d.description().ok().map(|d| d.name().to_string())));
    }
    v.dedup();
    v
}

/// Linear resampling of a mono stream, keeping its position between calls.
#[derive(Default)]
struct Resampler {
    pos: f64,
    last: f32,
}

impl Resampler {
    fn run(&mut self, input: &[f32], from: u32, to: u32, out: &mut Vec<f32>) {
        if from == to {
            out.extend_from_slice(input);
            return;
        }
        let step = from as f64 / to as f64;
        while self.pos < input.len() as f64 {
            let i = self.pos.floor() as usize;
            let t = (self.pos - i as f64) as f32;
            let a = if i == 0 { self.last } else { input[i - 1] };
            out.push(a + (input[i] - a) * t);
            self.pos += step;
        }
        self.pos -= input.len() as f64;
        self.last = input.last().copied().unwrap_or(self.last);
    }
}

/// An open microphone. Paused while nobody holds push-to-talk.
pub struct Mic {
    stream: cpal::Stream,
    /// 48 kHz mono samples not yet sent.
    buf: Arc<Mutex<VecDeque<f32>>>,
    running: bool,
}

impl Mic {
    /// Open `name` ("" or missing = the default microphone).
    pub fn open(name: &str) -> Option<Mic> {
        let host = cpal::default_host();
        let dev = host
            .input_devices()
            .ok()
            .and_then(|mut d| d.find(|d| !name.is_empty() && d.description().is_ok_and(|x| x.name() == name)))
            .or_else(|| host.default_input_device())?;
        let cfg = dev.default_input_config().ok()?;
        let (rate, channels) = (cfg.sample_rate(), cfg.channels() as usize);
        let buf: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let b = buf.clone();
        let mut rs = Resampler::default();
        let mut mono = Vec::new();
        let mut out = Vec::new();
        let mut push = move |data: &mut dyn Iterator<Item = f32>| {
            mono.clear();
            let all: Vec<f32> = data.collect();
            mono.extend(all.chunks(channels.max(1)).map(|f| f.iter().sum::<f32>() / f.len() as f32));
            out.clear();
            rs.run(&mono, rate, VOICE_RATE, &mut out);
            if let Ok(mut q) = b.lock() {
                q.extend(out.iter());
                let max = VOICE_RATE as usize; // never more than a second behind
                while q.len() > max {
                    q.pop_front();
                }
            }
        };
        let err = |e| eprintln!("microphone: {e}");
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_input_stream(&cfg.into(), move |d: &[f32], _: &_| push(&mut d.iter().copied()), err, None),
            cpal::SampleFormat::I16 => dev.build_input_stream(&cfg.into(), move |d: &[i16], _: &_| push(&mut d.iter().map(|s| *s as f32 / 32768.0)), err, None),
            _ => return None,
        }
        .ok()?;
        let _ = stream.pause();
        Some(Mic { stream, buf, running: false })
    }

    /// Start or stop capturing (stopping also drops what's left, so a new press starts fresh).
    pub fn set_running(&mut self, on: bool) {
        if on == self.running {
            return;
        }
        self.running = on;
        if on {
            let _ = self.stream.play();
        } else {
            let _ = self.stream.pause();
            if let Ok(mut q) = self.buf.lock() {
                q.clear();
            }
        }
    }

    /// Whole 20 ms frames captured so far.
    pub fn frames(&self, frame: usize) -> Vec<Vec<f32>> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.buf.lock() {
            while q.len() >= frame {
                out.push(q.drain(..frame).collect());
            }
        }
        out
    }
}

/// One person talking: their decoded samples waiting to play, and how loud in each ear.
#[derive(Default)]
struct Voice {
    queue: VecDeque<f32>,
    left: f32,
    right: f32,
    /// Waiting for a little audio before starting, so network jitter doesn't stutter.
    primed: bool,
}

/// Start playing a voice once this much is buffered (60 ms), and never let it lag more than 400 ms.
const PRIME: usize = VOICE_RATE as usize * 6 / 100;
const MAX_LAG: usize = VOICE_RATE as usize * 4 / 10;

#[derive(Default)]
struct Mixer {
    voices: HashMap<u8, Voice>,
}

/// The output stream that plays everyone's voice.
pub struct Speakers {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
}

impl Speakers {
    pub fn open() -> Option<Speakers> {
        let dev = cpal::default_host().default_output_device()?;
        let cfg = dev.default_output_config().ok()?;
        let (rate, channels) = (cfg.sample_rate(), cfg.channels() as usize);
        let mixer: Arc<Mutex<Mixer>> = Arc::default();
        // Device rate differs from 48 kHz: step through each voice at 48k/rate per output frame.
        let step = VOICE_RATE as f64 / rate as f64;
        let err = |e| eprintln!("voice output: {e}");
        let mx2 = mixer.clone();
        let mut acc = 0.0f64;
        let mut render = move |len: usize, write: &mut dyn FnMut(usize, f32)| {
            let Ok(mut mx) = mx2.lock() else { return };
            for f in 0..len / channels.max(1) {
                let (mut l, mut r) = (0.0f32, 0.0f32);
                for v in mx.voices.values_mut() {
                    if !v.primed {
                        v.primed = v.queue.len() >= PRIME;
                    }
                    if v.primed {
                        let s = v.queue.front().copied().unwrap_or(0.0);
                        l += s * v.left;
                        r += s * v.right;
                    }
                }
                acc += step;
                while acc >= 1.0 {
                    acc -= 1.0;
                    for v in mx.voices.values_mut().filter(|v| v.primed) {
                        if v.queue.pop_front().is_none() {
                            v.primed = false; // ran dry: wait for more before starting again
                        }
                    }
                }
                let (l, r) = (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0));
                for c in 0..channels {
                    write(f * channels + c, if channels == 1 { (l + r) * 0.5 } else if c % 2 == 0 { l } else { r });
                }
            }
        };
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_output_stream(&cfg.into(), move |d: &mut [f32], _: &_| { let n = d.len(); render(n, &mut |i, s| d[i] = s) }, err, None),
            cpal::SampleFormat::I16 => dev.build_output_stream(&cfg.into(), move |d: &mut [i16], _: &_| { let n = d.len(); render(n, &mut |i, s| d[i] = (s * 32767.0) as i16) }, err, None),
            _ => return None,
        }
        .ok()?;
        stream.play().ok()?;
        Some(Speakers { _stream: stream, mixer })
    }

    /// Queue decoded 48 kHz samples from `who`, at this volume in each ear.
    pub fn push(&self, who: u8, samples: &[f32], left: f32, right: f32) {
        if let Ok(mut mx) = self.mixer.lock() {
            let v = mx.voices.entry(who).or_default();
            v.queue.extend(samples.iter());
            while v.queue.len() > MAX_LAG {
                v.queue.pop_front();
            }
            (v.left, v.right) = (left, right);
        }
    }
}
