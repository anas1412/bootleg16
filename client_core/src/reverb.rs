//! Room reverb's sound side: the room you're in now (set by the front end, which measures it)
//! and a small Freeverb that gives a sample's echo. The voice mixer echoes voices with it too.
use std::sync::Mutex;

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

/// The room changed (or is easing towards a new one).
pub fn set(p: Params) {
    if let Ok(mut room) = ROOM.lock() {
        *room = p;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_echo_dies_away_and_a_bigger_room_rings_longer() {
        let rate = 48_000;
        let small = Params { wet: 0.2, size: 0.6, feedback: 0.72 };
        let big = Params { wet: 0.28, size: 1.2, feedback: 0.9 };
        assert!(Reverb::new(rate, big).tail_secs(rate) > Reverb::new(rate, small).tail_secs(rate));
        // the echo of a click dies away instead of building up
        let mut r = Reverb::new(rate, big);
        r.process(1.0);
        let late: f32 = (0..rate as usize * 4).map(|_| r.process(0.0).abs()).skip(rate as usize * 3).fold(0.0, f32::max);
        assert!(late < 1e-3, "{late}");
    }
}
