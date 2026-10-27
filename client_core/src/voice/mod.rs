//! Voice chat, engine-free. Team voice reaches your team anywhere; proximity voice reaches
//! everyone alive near you (enemies too), fading with distance, muffled through walls and
//! panned to where the speaker is. Opus at 48 kHz mono in 20 ms frames over the game's UDP
//! connection; the server relays (net/src/server/voice.rs).
//!
//! Each frame the front end calls `push` while a talk key is held, sends `take_outgoing` with
//! `net::Client::send_voice`, and hands `net::Client::take_voice` to `play`.
mod audio;

pub use audio::input_devices;
use net::client::Heard;
use net::protocol::{VoiceChannel, VOICE_FRAME, VOICE_RATE};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Voice settings, saved in ~/.config/bootleg16/voice.txt.
#[derive(Clone, Debug, PartialEq)]
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
    /// Each player's voice volume by name (missing = 1).
    players: HashMap<String, f32>,
    /// Muted for this match only (not saved; cleared by `new_match`).
    muted: HashSet<String>,
    /// Blocked: never heard again, team or proximity, in any match (saved).
    blocked: HashSet<String>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings { on: true, mic: String::new(), mic_volume: 1.0, volume: 1.0, open_mic: false, gate: 0.03, players: HashMap::new(), muted: HashSet::new(), blocked: HashSet::new() }
    }
}

/// After the voice drops under the gate, keep sending this many frames (300 ms) so the ends of
/// words aren't cut off.
const GATE_HOLD_FRAMES: u32 = 15;
/// Voice through a wall: this much quieter, and low-passed (one-pole, ~1.2 kHz at 48 kHz).
const MUFFLED_GAIN: f32 = 0.55;
const MUFFLE_ALPHA: f32 = 0.145;
/// Someone still counts as talking this long (seconds) after their last frame.
pub const LINGER: f32 = 0.3;

fn path() -> PathBuf {
    crate::config_dir().join("voice.txt")
}

impl VoiceSettings {
    /// The saved settings (defaults for anything missing).
    pub fn load() -> VoiceSettings {
        VoiceSettings::parse(&std::fs::read_to_string(path()).unwrap_or_default())
    }

    fn parse(src: &str) -> VoiceSettings {
        let mut s = VoiceSettings::default();
        for line in src.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "on" => s.on = v == "true",
                "mic" => s.mic = v.to_string(),
                "mic_volume" => s.mic_volume = v.parse().unwrap_or(s.mic_volume),
                "volume" => s.volume = v.parse().unwrap_or(s.volume),
                "open_mic" => s.open_mic = v == "true",
                "gate" => s.gate = v.parse().unwrap_or(s.gate),
                k if k.starts_with("block.") => {
                    s.blocked.insert(k["block.".len()..].to_string());
                }
                k if k.starts_with("player.") => {
                    let name = k["player.".len()..].to_string();
                    match v.parse::<f32>() {
                        // a mute saved before Block existed stays off for good
                        Ok(g) if g <= 0.0 => {
                            s.blocked.insert(name);
                        }
                        Ok(g) => {
                            s.players.insert(name, g);
                        }
                        Err(_) => {}
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn text(&self) -> String {
        let mut text = format!(
            "on={}\nmic={}\nmic_volume={}\nvolume={}\nopen_mic={}\ngate={}\n",
            self.on, self.mic, self.mic_volume, self.volume, self.open_mic, self.gate
        );
        for (name, g) in &self.players {
            text.push_str(&format!("player.{name}={g}\n"));
        }
        for name in &self.blocked {
            text.push_str(&format!("block.{name}=true\n"));
        }
        text
    }

    pub fn save(&self) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, self.text());
    }

    /// A new match: everyone muted for the last one is heard again.
    pub fn new_match(&mut self) {
        self.muted.clear();
    }

    /// How loud `name`'s voice plays for you (0: muted or blocked).
    pub fn player_gain(&self, name: &str) -> f32 {
        if self.is_muted(name) || self.is_blocked(name) {
            return 0.0;
        }
        self.player_volume(name)
    }

    /// The volume you set for `name` (whether or not they're muted).
    pub fn player_volume(&self, name: &str) -> f32 {
        self.players.get(name).copied().unwrap_or(1.0)
    }

    pub fn is_muted(&self, name: &str) -> bool {
        self.muted.contains(name)
    }

    pub fn is_blocked(&self, name: &str) -> bool {
        self.blocked.contains(name)
    }

    /// Mute or unmute `name` for this match.
    pub fn toggle_mute(&mut self, name: &str) {
        if !self.muted.remove(name) {
            self.muted.insert(name.to_string());
        }
    }

    /// Block or unblock `name` for good, and remember it.
    pub fn toggle_block(&mut self, name: &str) {
        if !self.blocked.remove(name) {
            self.blocked.insert(name.to_string());
        }
        self.save();
    }

    /// Set `name`'s voice volume (1 is normal, from a quarter up to 2), and remember it.
    pub fn set_player_gain(&mut self, name: &str, gain: f32) {
        let g = gain.clamp(0.25, 2.0);
        if (g - 1.0).abs() < 1e-3 {
            self.players.remove(name);
        } else {
            self.players.insert(name.to_string(), g);
        }
        self.save();
    }
}

/// Someone talking now, for the HUD's list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Talker {
    /// Their player id (None: you).
    pub from: Option<u8>,
    pub channel: VoiceChannel,
    /// Proximity voice from behind a wall.
    pub muffled: bool,
    /// How loud their last frame was (0..1), for the waveform.
    pub level: f32,
}

/// The last frame heard from someone.
struct LastHeard {
    channel: VoiceChannel,
    muffled: bool,
    at: Instant,
    level: f32,
}

/// The sound card side and the codec. cpal streams aren't `Send` on every platform, so this
/// stays on the thread that made it.
pub struct Voice {
    settings: VoiceSettings,
    mic: Option<audio::Mic>,
    mic_name: String,
    speakers: Option<audio::Speakers>,
    encoder: Option<opus::Encoder>,
    decoders: HashMap<u8, opus::Decoder>,
    /// Low-pass state per speaker (for voice through walls).
    muffle: HashMap<u8, f32>,
    /// Frames left to send after the voice fell under the gate.
    gate_hold: u32,
    /// When to try the speakers again after they failed.
    reopen_at: Instant,
    /// The talk key held this frame (`push`).
    pushed: Option<VoiceChannel>,
    /// What the last `take_outgoing` talked on, and how loud the mic was.
    sending: Option<VoiceChannel>,
    mic_level: f32,
    heard: HashMap<u8, LastHeard>,
}

/// Opus for speech at 24 kbit/s (~60 bytes per 20 ms frame; the default would be ~3x that).
fn voice_encoder() -> Option<opus::Encoder> {
    let mut e = opus::Encoder::new(VOICE_RATE, opus::Channels::Mono, opus::Application::Voip).ok()?;
    e.set_bitrate(opus::Bitrate::Bits(24_000)).ok()?;
    Some(e)
}

impl Voice {
    /// Open the microphone named in `settings` and the default speakers.
    pub fn new(settings: VoiceSettings) -> Voice {
        Voice {
            mic: audio::Mic::open(&settings.mic),
            mic_name: settings.mic.clone(),
            settings,
            speakers: audio::Speakers::open(),
            encoder: voice_encoder(),
            decoders: HashMap::new(),
            muffle: HashMap::new(),
            gate_hold: 0,
            reopen_at: Instant::now(),
            pushed: None,
            sending: None,
            mic_level: 0.0,
            heard: HashMap::new(),
        }
    }

    pub fn has_mic(&self) -> bool {
        self.mic.is_some()
    }

    pub fn settings(&self) -> &VoiceSettings {
        &self.settings
    }

    /// Change the settings (a new microphone opens on the next `take_outgoing` or `mic_test`).
    /// Call `save` on them to keep the change.
    pub fn settings_mut(&mut self) -> &mut VoiceSettings {
        &mut self.settings
    }

    /// Open mic is on: push `Team` every frame you'd let it talk (not with a menu open).
    pub fn open_mic(&self) -> bool {
        self.settings.on && self.settings.open_mic
    }

    /// A talk key is held this frame (call every frame it is).
    pub fn push(&mut self, channel: VoiceChannel) {
        self.pushed = Some(channel);
    }

    /// Switch microphones when the setting changed.
    fn follow(&mut self) {
        if self.mic_name != self.settings.mic {
            self.mic = audio::Mic::open(&self.settings.mic);
            self.mic_name = self.settings.mic.clone();
        }
    }

    /// Captured frames with the mic volume applied, and their loudness.
    fn captured(&mut self) -> (Vec<Vec<f32>>, f32) {
        let Some(mic) = &self.mic else { return (Vec::new(), 0.0) };
        let mut frames = mic.frames(VOICE_FRAME);
        let mut peak = 0.0f32;
        for f in &mut frames {
            for x in f.iter_mut() {
                *x = (*x * self.settings.mic_volume).clamp(-1.0, 1.0);
                peak = peak.max(x.abs());
            }
        }
        (frames, peak)
    }

    /// Once a frame: the Opus packets to send (`net::Client::send_voice`). While a talk key was
    /// pushed since the last call and voice is on, the mic runs, and what it hears goes out
    /// once it's louder than the noise gate.
    pub fn take_outgoing(&mut self) -> Vec<(VoiceChannel, Vec<u8>)> {
        self.follow();
        let channel = self.pushed.take().filter(|_| self.settings.on);
        if let Some(mic) = &mut self.mic {
            mic.set_running(channel.is_some());
        }
        let (frames, peak) = self.captured();
        self.mic_level = peak;
        self.sending = channel;
        let Some(channel) = channel else { return Vec::new() };
        let mut out = Vec::new();
        for f in frames {
            // Noise gate: send while it's louder than the gate, and a moment after.
            let loud = f.iter().fold(0.0f32, |m, x| m.max(x.abs())) >= self.settings.gate;
            self.gate_hold = if loud { GATE_HOLD_FRAMES } else { self.gate_hold.saturating_sub(1) };
            if !loud && self.gate_hold == 0 {
                continue;
            }
            let mut packet = [0u8; 400];
            if let Some(n) = self.encoder.as_mut().and_then(|e| e.encode_float(&f, &mut packet).ok()) {
                out.push((channel, packet[..n].to_vec()));
            }
        }
        out
    }

    /// The settings page's mic test: runs the mic while `on` (nothing is sent) and gives its
    /// loudness this frame (0..1).
    pub fn mic_test(&mut self, on: bool) -> f32 {
        self.follow();
        if let Some(mic) = &mut self.mic {
            mic.set_running(on);
        }
        self.captured().1
    }

    /// Your mic's loudness on the last `take_outgoing` (0..1).
    pub fn mic_level(&self) -> f32 {
        self.mic_level
    }

    /// Play a frame someone said (`net::Client::take_voice`). `name` is theirs (for your
    /// per-player volume, mute and block), `pan` where they are (-1 left .. 1 right, `pan()`).
    /// Gives how loud the frame was, or None if it wasn't played (voice off, muted, bad data).
    pub fn play(&mut self, heard: Heard, name: &str, pan: f32) -> Option<f32> {
        let Heard { from, channel, gain, muffled, data } = heard;
        // The speaker stream died (it said so once): open a new one, at most every 2 s.
        if self.speakers.as_ref().is_some_and(|s| s.failed()) && Instant::now() >= self.reopen_at {
            self.reopen_at = Instant::now() + Duration::from_secs(2);
            self.speakers = audio::Speakers::open();
        }
        if !self.settings.on {
            return None;
        }
        let player = self.settings.player_gain(name);
        if player <= 0.0 {
            return None; // muted: not even decoded
        }
        let dec = self.decoders.entry(from).or_insert_with(|| opus::Decoder::new(VOICE_RATE, opus::Channels::Mono).expect("opus decoder"));
        let mut pcm = vec![0.0f32; VOICE_FRAME * 6];
        let n = dec.decode_float(&data, &mut pcm, false).ok()?;
        // Through a wall: duller (low-pass) and quieter. The filter keeps its state per speaker
        // so it doesn't click between frames.
        let y = self.muffle.entry(from).or_insert(0.0);
        for x in pcm[..n].iter_mut() {
            *y += MUFFLE_ALPHA * (*x - *y);
            if muffled {
                *x = *y * MUFFLED_GAIN;
            }
        }
        let peak = pcm[..n].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        self.heard.insert(from, LastHeard { channel, muffled, at: Instant::now(), level: peak });
        let vol = self.settings.volume * player * gain as f32 / 255.0;
        if let Some(sp) = &self.speakers {
            sp.push(from, &pcm[..n], vol * (1.0 - pan.max(0.0)), vol * (1.0 + pan.min(0.0)));
        }
        Some(peak)
    }

    /// Who is talking now: you first (while you send), then everyone heard in the last `LINGER`
    /// seconds, by id.
    pub fn talking(&self) -> Vec<Talker> {
        let mut out: Vec<Talker> = self.sending.map(|channel| Talker { from: None, channel, muffled: false, level: self.mic_level }).into_iter().collect();
        let mut others: Vec<Talker> = self
            .heard
            .iter()
            .filter(|(_, h)| h.at.elapsed().as_secs_f32() < LINGER)
            .map(|(&id, h)| Talker { from: Some(id), channel: h.channel, muffled: h.muffled, level: h.level })
            .collect();
        others.sort_by_key(|t| t.from);
        out.extend(others);
        out
    }
}

/// Where a voice sits left to right (-1 left .. 1 right, at most 0.8 off centre). Only
/// proximity voice from someone you can see (`at`) is panned: towards them from your `ear`,
/// `right` being your view's right. Any one coordinate frame will do.
pub fn pan(channel: VoiceChannel, ear: [f32; 3], right: [f32; 3], at: Option<[f32; 3]>) -> f32 {
    let (VoiceChannel::Proximity, Some(at)) = (channel, at) else { return 0.0 };
    let d: [f32; 3] = std::array::from_fn(|i| at[i] - ear[i]);
    let len = d.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !(len.is_finite() && len > 0.0) {
        return 0.0;
    }
    (d.iter().zip(right).map(|(x, r)| x * r).sum::<f32>() / len).clamp(-1.0, 1.0) * 0.8
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

    #[test]
    fn settings_survive_a_save_and_mutes_do_not() {
        let mut s = VoiceSettings { on: false, mic: "USB Mic".into(), mic_volume: 1.5, volume: 0.8, open_mic: true, gate: 0.05, ..Default::default() };
        s.players.insert("loud".into(), 0.5);
        s.blocked.insert("troll".into());
        s.toggle_mute("chatty");
        let back = VoiceSettings::parse(&s.text());
        assert!(back.is_blocked("troll") && !back.is_muted("chatty"));
        s.new_match();
        assert_eq!(back, s);
        // a mute saved before Block existed
        assert!(VoiceSettings::parse("player.old=0\n").is_blocked("old"));
    }

    #[test]
    fn proximity_voice_pans_towards_the_speaker() {
        let (ear, right) = ([0.0; 3], [1.0, 0.0, 0.0]);
        assert!((pan(VoiceChannel::Proximity, ear, right, Some([10.0, 0.0, 0.0])) - 0.8).abs() < 1e-6);
        assert!((pan(VoiceChannel::Proximity, ear, right, Some([-5.0, 0.0, 5.0])) + 0.8 * 0.5f32.sqrt()).abs() < 1e-6);
        assert_eq!(pan(VoiceChannel::Team, ear, right, Some([10.0, 0.0, 0.0])), 0.0);
        assert_eq!(pan(VoiceChannel::Proximity, ear, right, None), 0.0);
        assert_eq!(pan(VoiceChannel::Proximity, ear, right, Some(ear)), 0.0);
    }
}
