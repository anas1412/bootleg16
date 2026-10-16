//! Voice chat: two push-to-talk keys. Team voice reaches your team anywhere; proximity voice
//! reaches everyone alive near you (enemies too), fading with distance and panned to where
//! you are. Opus over the game's UDP connection; the server relays (net/src/server/voice.rs).
mod audio;

use crate::game::{FlyCam, Frame, Walker};
use crate::menu::{overlay_open, AppState, Overlay};
use crate::ui::{button, text, UiAction, CT_COLOR, MUTED, TEXT, T_COLOR};
use bevy::prelude::*;
use net::protocol::{Team, VoiceChannel, VOICE_FRAME, VOICE_RATE};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VoiceSettings::load())
            .insert_resource(Devices(audio::input_devices()))
            .init_resource::<MicLevel>()
            .add_systems(Startup, open_audio)
            .add_systems(OnEnter(AppState::InGame), spawn_talkers)
            .add_systems(Update, (rebind, mic_test).run_if(in_state(AppState::Menu)))
            .add_systems(Update, (talk, listen, talkers).chain().in_set(Frame::Sound));
    }
}

/// A push-to-talk binding: a key or a mouse button, saved by name ("KeyK", "MouseBack").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Keys that can be bound (by their saved names).
const KEYS: [KeyCode; 62] = {
    use KeyCode::*;
    [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Digit0, Digit1,
        Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, Space, Tab, CapsLock, ShiftLeft, ShiftRight,
        ControlLeft, ControlRight, AltLeft, AltRight, Backquote, Comma, Period, Slash, Semicolon,
    ]
};
const MICE: [MouseButton; 4] = [MouseButton::Middle, MouseButton::Back, MouseButton::Forward, MouseButton::Right];

impl Bind {
    fn name(self) -> String {
        match self {
            Bind::Key(k) => format!("{k:?}"),
            Bind::Mouse(m) => format!("Mouse{m:?}"),
        }
    }

    fn parse(s: &str) -> Option<Bind> {
        KEYS.iter().find(|k| format!("{k:?}") == s).map(|k| Bind::Key(*k)).or_else(|| MICE.iter().find(|m| format!("Mouse{m:?}") == s).map(|m| Bind::Mouse(*m)))
    }

    /// As shown to the player: "K", "5", "Mouse Back".
    fn label(self) -> String {
        let n = self.name();
        n.strip_prefix("Key").or_else(|| n.strip_prefix("Digit")).map(str::to_string).unwrap_or_else(|| n.replace("Mouse", "Mouse "))
    }

    fn held(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Bind::Key(k) => keys.pressed(k),
            Bind::Mouse(m) => mouse.pressed(m),
        }
    }
}

/// Voice settings, saved in ~/.config/bootleg16/voice.txt.
#[derive(Resource, Clone)]
pub struct VoiceSettings {
    pub on: bool,
    pub team_key: Bind,
    pub near_key: Bind,
    /// Microphone by name ("" = system default).
    pub mic: String,
    pub mic_volume: f32,
    /// Everyone else's voice.
    pub volume: f32,
    /// Waiting for a key to bind (on the voice settings page).
    pub capture: Option<VoiceChannel>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        // K is CS's voice key; X sits next to the movement keys.
        VoiceSettings { on: true, team_key: Bind::Key(KeyCode::KeyK), near_key: Bind::Key(KeyCode::KeyX), mic: String::new(), mic_volume: 1.0, volume: 1.0, capture: None }
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
                "team_key" => s.team_key = Bind::parse(v).unwrap_or(s.team_key),
                "near_key" => s.near_key = Bind::parse(v).unwrap_or(s.near_key),
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
            "on={}\nteam_key={}\nnear_key={}\nmic={}\nmic_volume={}\nvolume={}\n",
            self.on,
            self.team_key.name(),
            self.near_key.name(),
            self.mic,
            self.mic_volume,
            self.volume
        );
        let _ = std::fs::write(p, text);
    }
}

/// Microphones found at startup.
#[derive(Resource)]
pub struct Devices(Vec<String>);

/// Loudness of the microphone (0..1), for the meter on the voice settings page.
#[derive(Resource, Default)]
pub struct MicLevel(f32);

/// The sound card side (cpal streams aren't Send, so these live on the main thread).
struct Audio {
    mic: Option<audio::Mic>,
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
    fn follow(&mut self, s: &VoiceSettings) {
        if self.mic_name != s.mic {
            self.mic = audio::Mic::open(&s.mic);
            self.mic_name = s.mic.clone();
        }
    }

    /// Captured frames with the mic volume applied, and their loudness.
    fn captured(&mut self, s: &VoiceSettings) -> (Vec<Vec<f32>>, f32) {
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
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Option<Res<Overlay>>,
    mut w: ResMut<Walker>,
    mut level: ResMut<MicLevel>,
    mut me_talking: Local<Option<VoiceChannel>>,
) {
    audio.follow(&settings);
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    let channel = if !settings.on || auto || overlay_open(overlay) {
        None
    } else if settings.team_key.held(&keys, &mouse) {
        Some(VoiceChannel::Team)
    } else if settings.near_key.held(&keys, &mouse) {
        Some(VoiceChannel::Proximity)
    } else {
        None
    };
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

/// Who is talking now (for the list on screen): channel and when we last heard them.
#[derive(Resource, Default)]
struct Talking(HashMap<u8, (VoiceChannel, f32)>);

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
        talking.0.insert(from, (channel, time.elapsed_secs()));
        let dec = audio.decoders.entry(from).or_insert_with(|| opus::Decoder::new(VOICE_RATE, opus::Channels::Mono).expect("opus decoder"));
        let mut pcm = vec![0.0f32; VOICE_FRAME * 6];
        let Ok(n) = dec.decode_float(&data, &mut pcm, false) else { continue };
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

#[derive(Component)]
struct TalkersList;

fn spawn_talkers(mut commands: Commands) {
    commands.insert_resource(Talking::default());
    commands.spawn((
        TalkersList,
        Node { position_type: PositionType::Absolute, left: px(12), top: px(120), flex_direction: FlexDirection::Column, row_gap: px(2), ..default() },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Names of who's talking, left side (like CS), and yourself while you hold a key.
#[allow(clippy::too_many_arguments)]
fn talkers(
    mut commands: Commands,
    w: Res<Walker>,
    settings: Res<VoiceSettings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    talking: Res<Talking>,
    time: Res<Time>,
    root: Single<(Entity, Option<&Children>), With<TalkersList>>,
    mut shown: Local<Vec<String>>,
) {
    let now = time.elapsed_secs();
    let name = |id: u8| w.client.scores.iter().find(|s| s.id == id).map(|s| (s.name.clone(), s.team));
    let mut lines: Vec<(String, Color)> = talking
        .0
        .iter()
        .filter(|(_, (_, at))| now - at < 0.3)
        .filter_map(|(id, (ch, _))| {
            let (n, t) = name(*id)?;
            let c = if t == Team::Ct { CT_COLOR } else { T_COLOR };
            Some((format!("{n}  ({})", if *ch == VoiceChannel::Team { "team" } else { "near" }), c))
        })
        .collect();
    if settings.on && settings.team_key.held(&keys, &mouse) {
        lines.insert(0, ("You  (team)".into(), TEXT));
    } else if settings.on && settings.near_key.held(&keys, &mouse) {
        lines.insert(0, ("You  (near)".into(), TEXT));
    }
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    let key: Vec<String> = lines.iter().map(|l| l.0.clone()).collect();
    if *shown == key {
        return;
    }
    *shown = key;
    let (root, children) = *root;
    for &c in children.into_iter().flatten() {
        commands.entity(c).despawn();
    }
    commands.entity(root).with_children(|col| {
        for (l, c) in lines {
            col.spawn(text(format!("[voice] {l}"), 15.0, c));
        }
    });
}

/// A change made on the voice settings page.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceAction {
    Toggle,
    Rebind(VoiceChannel),
    NextMic,
    MicVolume(f32),
    Volume(f32),
}

/// Apply a voice settings change (called from the menu).
pub fn apply(s: &mut VoiceSettings, devices: &Devices, a: VoiceAction) {
    match a {
        VoiceAction::Toggle => s.on = !s.on,
        VoiceAction::Rebind(ch) => s.capture = Some(ch),
        VoiceAction::NextMic => {
            let i = devices.0.iter().position(|d| *d == s.mic).unwrap_or(0);
            s.mic = devices.0.get((i + 1) % devices.0.len().max(1)).cloned().unwrap_or_default();
        }
        VoiceAction::MicVolume(d) => s.mic_volume = (s.mic_volume + d).clamp(0.0, 3.0),
        VoiceAction::Volume(d) => s.volume = (s.volume + d).clamp(0.0, 2.0),
    }
    s.save();
}

/// Waiting for a key to bind: the next key or mouse button press takes it (Esc cancels).
fn rebind(mut s: ResMut<VoiceSettings>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, mut st: ResMut<crate::menu::MenuDirty>) {
    let Some(ch) = s.capture else { return };
    let pressed = keys.get_just_pressed().find_map(|k| KEYS.contains(k).then_some(Bind::Key(*k))).or_else(|| mouse.get_just_pressed().find_map(|m| MICE.contains(m).then_some(Bind::Mouse(*m))));
    if keys.just_pressed(KeyCode::Escape) {
        s.capture = None;
        st.0 = true;
        return;
    }
    let Some(b) = pressed else { return };
    match ch {
        VoiceChannel::Team => s.team_key = b,
        VoiceChannel::Proximity => s.near_key = b,
    }
    s.capture = None;
    s.save();
    st.0 = true;
}

#[derive(Component)]
struct MicMeter;

/// On the voice settings page the mic is live, so you can see it working.
fn mic_test(mut audio: NonSendMut<Audio>, settings: Res<VoiceSettings>, mut level: ResMut<MicLevel>, mut meter: Query<&mut Node, With<MicMeter>>) {
    audio.follow(&settings);
    let on = !meter.is_empty() && settings.on;
    if let Some(mic) = &mut audio.mic {
        mic.set_running(on);
    }
    let (_, peak) = audio.captured(&settings);
    level.0 = if on { level.0 * 0.85 + peak * 0.15 } else { 0.0 };
    for mut n in &mut meter {
        n.width = percent((level.0 * 140.0).min(100.0));
    }
}

/// The voice settings page (menu → Voice).
pub fn page(p: &mut ChildSpawnerCommands, s: &VoiceSettings) {
    p.spawn(text("Voice", 30.0, TEXT));
    p.spawn((text("Team voice: your team hears you anywhere. Proximity voice: everyone near you hears you, enemies too.", 16.0, MUTED), Node { margin: UiRect::vertical(px(8)), ..default() }));
    let a = |v: VoiceAction| UiAction::Voice(v);
    button(p, if s.on { "Voice chat: on" } else { "Voice chat: off" }, a(VoiceAction::Toggle), s.on);
    let key = |ch: VoiceChannel, b: Bind| if s.capture == Some(ch) { "press a key or mouse button... (Esc cancels)".to_string() } else { b.label() };
    button(p, &format!("Team voice key: {}", key(VoiceChannel::Team, s.team_key)), a(VoiceAction::Rebind(VoiceChannel::Team)), s.capture == Some(VoiceChannel::Team));
    button(p, &format!("Proximity voice key: {}", key(VoiceChannel::Proximity, s.near_key)), a(VoiceAction::Rebind(VoiceChannel::Proximity)), s.capture == Some(VoiceChannel::Proximity));
    let mic = if s.mic.is_empty() { "system default" } else { s.mic.as_str() };
    button(p, &format!("Microphone: {mic}"), a(VoiceAction::NextMic), false);
    crate::menu::setting_row(p, &format!("Mic volume: {:.0}%", s.mic_volume * 100.0), a(VoiceAction::MicVolume(-0.1)), a(VoiceAction::MicVolume(0.1)));
    crate::menu::setting_row(p, &format!("Voice volume: {:.0}%", s.volume * 100.0), a(VoiceAction::Volume(-0.1)), a(VoiceAction::Volume(0.1)));
    p.spawn((text("Mic test (talk now):", 16.0, MUTED), Node { margin: UiRect::top(px(10)), ..default() }));
    p.spawn((Node { width: px(320), height: px(10), border_radius: BorderRadius::all(px(5)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))).with_children(|b| {
        b.spawn((MicMeter, Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(5)), ..default() }, BackgroundColor(Color::srgb(0.45, 0.95, 0.45))));
    });
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
    fn binds_save_and_load_by_name() {
        for b in [Bind::Key(KeyCode::KeyK), Bind::Key(KeyCode::F5), Bind::Mouse(MouseButton::Back)] {
            assert_eq!(Bind::parse(&b.name()), Some(b));
        }
        assert_eq!(Bind::Key(KeyCode::KeyX).label(), "X");
        assert_eq!(Bind::Mouse(MouseButton::Forward).label(), "Mouse Forward");
    }
}
